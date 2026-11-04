//! Narrow, original reader for observed static float3 presentation meshes.
//! Only the researched 20/24/28/32/36-byte declarations are accepted. Half4
//! first texture coordinates retain their first two components for preview;
//! the other two components are checked for finiteness but remain uninterpreted.
//! Other vertex and primitive layouts error rather than silently dropping draws.
use crate::{ArchiveIndex, StreamIndex, inventory_disc, rw4::Arena, skating::Vec3};
use std::{fs::File, path::Path};

#[derive(Debug, Clone)]
pub struct Mesh {
    pub draw_index: usize,
    pub positions: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub uv: Vec<[f32; 2]>,
}
#[derive(Debug, Clone)]
pub struct Sample {
    pub meshes: Vec<Mesh>,
    pub min: Vec3,
    pub max: Vec3,
}
impl Sample {
    /// Shared placement for rendering and prototype collision.
    pub fn preview_position(&self, p: Vec3) -> Vec3 {
        let span = self.max - self.min;
        let scale = 8.0 / span.x.max(span.y).max(span.z).max(0.001);
        let center = (self.min + self.max) * 0.5;
        Vec3::new(
            (p.x - center.x) * scale - 7.0,
            (p.y - self.min.y) * scale + 0.02,
            (p.z - center.z) * scale - 2.0,
        )
    }
}

pub fn read_meshes(bytes: &[u8]) -> Result<Sample, String> {
    let arena = Arena::read(bytes)?;
    let mut meshes = Vec::new();
    let mut vertex_total = 0usize;
    let mut index_total = 0usize;
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (draw_index, _) in arena
        .objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.type_id == 0xeb0023)
    {
        let d = arena.object_data(bytes, draw_index)?;
        if d.len() < 60 {
            return Err("Truncated static draw record.".into());
        }
        let layout = vertex_layout(object(&arena, bytes, word(d, 40) as usize, 0x200e9, 16)?)?;
        let mesh = object(&arena, bytes, word(d, 44) as usize, 0x20081, 16)?;
        if mesh.len() != 16 || word(mesh, 0) != 1 || word(mesh, 4) != 1 {
            return Err("Unsupported static mesh links.".into());
        }
        let vertex = object(&arena, bytes, word(mesh, 12) as usize, 0x200ea, 40)?;
        let index = object(&arena, bytes, word(mesh, 8) as usize, 0x200eb, 36)?;
        if vertex.len() != 40
            || index.len() != 36
            || word(index, 0) != 0x20000002
            || word(vertex, 24) & 3 != 3
        {
            return Err("Unsupported static vertex/index buffer profile.".into());
        }
        let vertex_bytes = object(&arena, bytes, (word(vertex, 24) >> 2) as usize, 0x10031, 0)?;
        let index_bytes = object(&arena, bytes, word(index, 24) as usize, 0x10031, 0)?;
        let length = word(vertex, 32) as usize;
        let index_count = word(index, 32) as usize;
        let draw_count = word(d, 56) as usize;
        if draw_count == 0
            || draw_count > 4096
            || 96usize
                .checked_add(draw_count * 16)
                .is_none_or(|end| end > d.len())
        {
            return Err("Static draw ranges are invalid.".into());
        }
        let mut consumed = 0usize;
        for range in 0..draw_count {
            let offset = 96 + range * 16;
            let count = word(d, offset + 12) as usize;
            if word(d, offset) != 4
                || word(d, offset + 4) != 0
                || word(d, offset + 8) as usize != consumed
                || count % 3 != 0
            {
                return Err("Unsupported primitive topology, base vertex, or draw range.".into());
            }
            consumed = consumed.checked_add(count).ok_or("Draw count overflow.")?;
        }
        if consumed != index_count {
            return Err("Draw ranges do not cover the index buffer.".into());
        }
        if length == 0
            || length % layout.stride != 0
            || length > vertex_bytes.len()
            || index_count == 0
            || index_count % 3 != 0
            || index_count
                .checked_mul(2)
                .is_none_or(|end| end > index_bytes.len())
            || word(index, 28) as usize > index_bytes.len()
        {
            return Err("Mesh buffer lengths or triangle count are invalid.".into());
        }
        vertex_total += length / layout.stride;
        index_total += index_count;
        if vertex_total > 1_000_000 || index_total > 3_000_000 || meshes.len() >= 4096 {
            return Err("Geometry exceeds the preview budget.".into());
        }
        let mut positions = Vec::with_capacity(length / layout.stride);
        let mut uv = Vec::with_capacity(length / layout.stride);
        for record in vertex_bytes[..length].chunks_exact(layout.stride) {
            let p = Vec3::new(
                f32::from_bits(word(record, 0)),
                f32::from_bits(word(record, 4)),
                f32::from_bits(word(record, 8)),
            );
            if ![p.x, p.y, p.z]
                .iter()
                .all(|v| v.is_finite() && v.abs() < 1_000_000.0)
            {
                return Err("Non-finite or excessive vertex coordinate.".into());
            }
            min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
            max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
            positions.push(p);
            let mut coords = [0.0; 4];
            for (destination, source) in coords
                .iter_mut()
                .zip(record[12..12 + layout.uv_components * 2].chunks_exact(2))
            {
                *destination = half(u16::from_be_bytes(source.try_into().unwrap()));
            }
            if !coords[..layout.uv_components].iter().all(|v| v.is_finite()) {
                return Err("Non-finite mesh UV coordinate.".into());
            }
            uv.push([coords[0], coords[1]]);
        }
        let indices: Vec<u32> = index_bytes[..index_count * 2]
            .chunks_exact(2)
            .map(|p| u16::from_be_bytes(p.try_into().unwrap()) as u32)
            .collect();
        if indices.iter().any(|&i| i as usize >= positions.len()) {
            return Err("Triangle index exceeds its vertex buffer.".into());
        }
        meshes.push(Mesh {
            draw_index,
            positions,
            indices,
            uv,
        });
    }
    if meshes.is_empty() {
        return Err("No supported static draw records found in this arena.".into());
    }
    Ok(Sample { meshes, min, max })
}

#[derive(Debug, Clone, Copy)]
struct VertexLayout {
    stride: usize,
    uv_components: usize,
}

/// Entire declarations, including unused attribute tokens, are matched against
/// the small set read directly from neighboring Downtown presentation arenas.
/// In particular 0x001A2360 contains four half values, not two f32 values.
fn vertex_layout(bytes: &[u8]) -> Result<VertexLayout, String> {
    const POSITION: [u32; 4] = [0, 0x002a23b9, 1, 1];
    const UV2: [u32; 4] = [12, 0x002c235f, 0x00050006, 1];
    const UV4: [u32; 4] = [12, 0x001a2360, 0x00050006, 1];
    const LIGHTMAP2: [u32; 4] = [16, 0x002c2159, 0x00050107, 1];
    const LIGHTMAP4: [u32; 4] = [16, 0x001a215a, 0x00050107, 1];
    const WIDE_LIGHTMAP4: [u32; 4] = [20, 0x001a215a, 0x00050107, 1];
    const COLOR20: [u32; 4] = [16, 0x002a2190, 0x00060015, 1];
    const COLOR28: [u32; 4] = [24, 0x002a2190, 0x00060015, 1];
    const COLOR32: [u32; 4] = [28, 0x002a2190, 0x00060015, 1];
    const COLOR36: [u32; 4] = [32, 0x002a2190, 0x00060015, 1];
    const EXTRA_UV: [u32; 4] = [28, 0x002c235f, 0x00050309, 1];
    let unsupported = || "Unsupported static vertex declaration.".to_string();
    if bytes.len() < 16 || word(bytes, 0) != 0 || word(bytes, 12) != 0 {
        return Err(unsupported());
    }
    let (attributes, stride, uv_components): (&[[u32; 4]], usize, usize) = match (
        bytes.len(),
        word(bytes, 4),
        word(bytes, 8),
        bytes.last().copied(),
    ) {
        (67, 0xc2, 0x00030001, Some(20)) => (&[POSITION, UV2, LIGHTMAP2], 20, 2),
        (67, 0x00200042, 0x00030001, Some(20)) => (&[POSITION, UV2, COLOR20], 20, 2),
        (67, 0xc2, 0x00030001, Some(24)) => (&[POSITION, UV2, LIGHTMAP4], 24, 2),
        (84, 0x002000c2, 0x00040001, Some(28)) => (&[POSITION, UV2, LIGHTMAP4, COLOR28], 28, 2),
        (84, 0x002000c2, 0x00040001, Some(32)) => {
            (&[POSITION, UV4, WIDE_LIGHTMAP4, COLOR32], 32, 4)
        }
        (101, 0x002002c2, 0x00050001, Some(36)) => {
            (&[POSITION, UV4, WIDE_LIGHTMAP4, EXTRA_UV, COLOR36], 36, 4)
        }
        _ => return Err(unsupported()),
    };
    let tail = 16 + attributes.len() * 16;
    if bytes.len() != tail + attributes.len()
        || bytes[tail..].iter().any(|&value| value as usize != stride)
        || attributes.iter().enumerate().any(|(index, attribute)| {
            attribute.iter().enumerate().any(|(component, &expected)| {
                word(bytes, 16 + index * 16 + component * 4) != expected
            })
        })
    {
        return Err(unsupported());
    }
    Ok(VertexLayout {
        stride,
        uv_components,
    })
}

/// Read a small, explicitly selected research sample from the user's own image.
/// The result stays in memory and is not a complete level or collision mesh.
pub fn load_retail_sample(image: &Path) -> Result<Sample, String> {
    read_meshes(&load_retail_block(image, 15)?)
}

/// Decode a block from the explicit research member, capped by the stream reader.
pub fn load_retail_block(image: &Path, block: usize) -> Result<Vec<u8>, String> {
    let disc = inventory_disc(image)?;
    let archive = disc
        .entries
        .iter()
        .find(|e| {
            e.path
                .eq_ignore_ascii_case("data/content/worldDIST_DownTown.big")
                && !e.is_directory
        })
        .ok_or("Retail sample archive not found.")?;
    let mut file = File::open(image).map_err(|e| e.to_string())?;
    let index = ArchiveIndex::read(&mut file, archive.image_offset, archive.size as u64)?;
    let member = index
        .entries
        .iter()
        .find(|e| {
            e.path.eq_ignore_ascii_case(
                "data/content/world/stream/DIST_DownTown/cPres_550_150_high.xsf",
            )
        })
        .ok_or("Retail sample member not found.")?;
    if member.compressed_size_field != Some(0) || member.size_field as u64 > member.available_bytes
    {
        return Err("Unsupported outer sample compression.".into());
    }
    let base = archive
        .image_offset
        .checked_add(member.offset)
        .ok_or("Sample offset overflow.")?;
    let stream = StreamIndex::read(&mut file, base, member.size_field as u64)?;
    stream.decode_block(&mut file, base, block, 64 * 1024 * 1024)
}
fn object<'a>(
    arena: &Arena,
    bytes: &'a [u8],
    index: usize,
    expected: u32,
    min: usize,
) -> Result<&'a [u8], String> {
    let o = arena
        .objects
        .get(index)
        .ok_or("Mesh object reference is out of range.")?;
    if o.type_id != expected {
        return Err(format!(
            "Mesh link has type 0x{:X}; expected 0x{expected:X}.",
            o.type_id
        ));
    }
    let data = arena.object_data(bytes, index)?;
    if data.len() < min {
        return Err("Mesh object is truncated.".into());
    }
    Ok(data)
}
fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = (bits >> 10) & 31;
    let fraction = bits & 1023;
    match exponent {
        0 => sign * (fraction as f32) * 2.0f32.powi(-24),
        31 => {
            if fraction == 0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => sign * (1.0 + fraction as f32 / 1024.0) * 2.0f32.powi(exponent as i32 - 15),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn half_coordinates_cover_sign_subnormals_and_nonfinite_values() {
        assert_eq!(half(0x3c00), 1.0);
        assert_eq!(half(0xc000), -2.0);
        assert_eq!(half(1), 2.0f32.powi(-24));
        assert_eq!(half(0x3555), 0.33325195);
        assert!(half(0x7c00).is_infinite() && half(0x7fff).is_nan());
    }
    fn put(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 740];
        bytes[..24].copy_from_slice(b"\x89RW4xb2\0\r\n\x1a\n\x01\x20\x04\x00454\x00000\x00");
        for (offset, value) in [
            (0x20, 7),
            (0x24, 7),
            (0x30, 464),
            (0x44, 640),
            (0x48, 4),
            (0x50, 1),
            (0x54, 100),
            (0x58, 4),
            (0x60, 1),
            (0x68, 1),
        ] {
            put(&mut bytes, offset, value);
        }
        let records = [
            (172, 116, 0xeb0023),
            (288, 84, 0x200e9),
            (372, 16, 0x20081),
            (388, 40, 0x200ea),
            (428, 36, 0x200eb),
            (0, 84, 0x10031),
            (84, 16, 0x10031),
        ];
        for (index, (offset, size, kind)) in records.into_iter().enumerate() {
            let p = 464 + index * 24;
            for (o, v) in [(0, offset), (8, size), (12, 4), (20, kind)] {
                put(&mut bytes, p + o, v);
            }
        }
        for (offset, value) in [
            (172 + 40, 1),
            (172 + 44, 2),
            (172 + 56, 1),
            (172 + 96, 4),
            (172 + 108, 3),
            (288 + 4, 0x002000c2),
            (288 + 8, 0x00040001),
            (288 + 20, 0x002a23b9),
            (288 + 24, 1),
            (288 + 28, 1),
            (288 + 32, 12),
            (288 + 36, 0x002c235f),
            (288 + 40, 0x00050006),
            (288 + 44, 1),
            (288 + 48, 16),
            (288 + 52, 0x001a215a),
            (288 + 56, 0x00050107),
            (288 + 60, 1),
            (288 + 64, 24),
            (288 + 68, 0x002a2190),
            (288 + 72, 0x00060015),
            (288 + 76, 1),
            (372, 1),
            (376, 1),
            (380, 4),
            (384, 3),
            (388 + 24, 23),
            (388 + 32, 84),
            (428, 0x20000002),
            (428 + 24, 6),
            (428 + 28, 16),
            (428 + 32, 3),
        ] {
            put(&mut bytes, offset, value);
        }
        bytes[368..372].fill(28);
        for (index, p) in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            .iter()
            .enumerate()
        {
            for (component, value) in p.iter().enumerate() {
                put(
                    &mut bytes,
                    640 + index * 28 + component * 4,
                    value.to_bits(),
                );
            }
        }
        bytes[724..730].copy_from_slice(&[0, 0, 0, 1, 0, 2]);
        bytes
    }
    #[test]
    fn reads_known_triangle_and_excludes_index_padding() {
        let sample = read_meshes(&fixture()).unwrap();
        assert_eq!(sample.meshes.len(), 1);
        assert_eq!(sample.meshes[0].positions.len(), 3);
        assert_eq!(sample.meshes[0].indices, [0, 1, 2]);
        assert_eq!(sample.max, Vec3::new(1.0, 1.0, 0.0));
    }
    #[test]
    fn rejects_invalid_indices_coordinates_links_and_layouts() {
        for (offset, value) in [
            (640, f32::NAN.to_bits()),
            (388 + 24, 4003),
            (288 + 20, 0),
            (428 + 32, 4),
            (172 + 96, 5),
            (472, 56),
            (640 + 12, 0x7c000000),
        ] {
            let mut bytes = fixture();
            put(&mut bytes, offset, value);
            assert!(read_meshes(&bytes).is_err());
        }
        let mut bytes = fixture();
        bytes[729] = 3;
        assert!(read_meshes(&bytes).is_err());
    }

    fn profile_fixture(stride: usize, color_only: bool) -> Vec<u8> {
        let wide = stride >= 32;
        let mut attributes = vec![
            [0, 0x002a23b9, 1, 1],
            [
                12,
                if wide { 0x001a2360 } else { 0x002c235f },
                0x00050006,
                1,
            ],
        ];
        match (stride, color_only) {
            (20, false) => attributes.push([16, 0x002c2159, 0x00050107, 1]),
            (20, true) => attributes.push([16, 0x002a2190, 0x00060015, 1]),
            (24, false) => attributes.push([16, 0x001a215a, 0x00050107, 1]),
            (28, false) => attributes.extend([
                [16, 0x001a215a, 0x00050107, 1],
                [24, 0x002a2190, 0x00060015, 1],
            ]),
            (32, false) => attributes.extend([
                [20, 0x001a215a, 0x00050107, 1],
                [28, 0x002a2190, 0x00060015, 1],
            ]),
            (36, false) => attributes.extend([
                [20, 0x001a215a, 0x00050107, 1],
                [28, 0x002c235f, 0x00050309, 1],
                [32, 0x002a2190, 0x00060015, 1],
            ]),
            _ => panic!("Unsupported fixture"),
        }
        let mut declaration = vec![0; 16 + attributes.len() * 17];
        let flags = match (stride, color_only) {
            (20, true) => 0x00200042,
            (20 | 24, false) => 0xc2,
            (36, false) => 0x002002c2,
            _ => 0x002000c2,
        };
        put(&mut declaration, 4, flags);
        put(&mut declaration, 8, (attributes.len() as u32) << 16 | 1);
        for (index, attribute) in attributes.iter().enumerate() {
            for (component, &value) in attribute.iter().enumerate() {
                put(&mut declaration, 16 + index * 16 + component * 4, value);
            }
        }
        declaration[16 + attributes.len() * 16..].fill(stride as u8);
        let mut draw = vec![0; 116];
        for (offset, value) in [(40, 1), (44, 2), (56, 1), (96, 4), (108, 3)] {
            put(&mut draw, offset, value);
        }
        let mut mesh = vec![0; 16];
        for (offset, value) in [(0, 1), (4, 1), (8, 4), (12, 3)] {
            put(&mut mesh, offset, value);
        }
        let mut vertex = vec![0; 40];
        put(&mut vertex, 24, 23);
        put(&mut vertex, 32, (stride * 3) as u32);
        let mut index = vec![0; 36];
        for (offset, value) in [(0, 0x20000002), (24, 6), (28, 16), (32, 3)] {
            put(&mut index, offset, value);
        }
        let metadata_objects = [
            (draw, 0xeb0023),
            (declaration, 0x200e9),
            (mesh, 0x20081),
            (vertex, 0x200ea),
            (index, 0x200eb),
        ];
        let mut bytes = vec![0; 172];
        bytes[..24].copy_from_slice(b"\x89RW4xb2\0\r\n\x1a\n\x01\x20\x04\x00454\x00000\x00");
        let mut records = Vec::new();
        for (object, kind) in metadata_objects {
            let offset = bytes.len();
            records.push((offset as u32, object.len() as u32, kind));
            bytes.extend(object);
            bytes.resize((bytes.len() + 3) & !3, 0);
        }
        let table = bytes.len();
        records.extend([
            (0, (stride * 3) as u32, 0x10031),
            ((stride * 3) as u32, 16, 0x10031),
        ]);
        bytes.resize(table + records.len() * 24, 0);
        let resource = bytes.len();
        bytes.resize(resource + stride * 3 + 16, 0);
        for (offset, value) in [
            (0x20, 7),
            (0x24, 7),
            (0x30, table as u32),
            (0x44, resource as u32),
            (0x48, 4),
            (0x50, 1),
            (0x54, (stride * 3 + 16) as u32),
            (0x58, 4),
            (0x60, 1),
            (0x68, 1),
        ] {
            put(&mut bytes, offset, value);
        }
        for (index, (offset, size, kind)) in records.into_iter().enumerate() {
            for (field, value) in [(0, offset), (8, size), (12, 4), (20, kind)] {
                put(&mut bytes, table + index * 24 + field, value);
            }
        }
        for (index, position) in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            .iter()
            .enumerate()
        {
            for (component, value) in position.iter().enumerate() {
                put(
                    &mut bytes,
                    resource + index * stride + component * 4,
                    value.to_bits(),
                );
            }
            bytes[resource + index * stride + 12..resource + index * stride + 16]
                .copy_from_slice(&[0x3c, 0x00, 0xc0, 0x00]);
            if wide {
                bytes[resource + index * stride + 16..resource + index * stride + 20]
                    .copy_from_slice(&[0x38, 0x00, 0x40, 0x00]);
            }
        }
        bytes[resource + stride * 3..resource + stride * 3 + 6]
            .copy_from_slice(&[0, 0, 0, 1, 0, 2]);
        bytes
    }

    #[test]
    fn observed_declarations_decode_correct_stride_and_half_uv_components() {
        for (stride, color_only) in [
            (20, false),
            (20, true),
            (24, false),
            (28, false),
            (32, false),
            (36, false),
        ] {
            let sample = read_meshes(&profile_fixture(stride, color_only)).unwrap();
            assert_eq!(
                sample.meshes[0].positions,
                [
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0)
                ]
            );
            assert_eq!(sample.meshes[0].indices, [0, 1, 2]);
            assert_eq!(sample.meshes[0].uv, [[1.0, -2.0]; 3]);
        }
    }

    #[test]
    fn rejects_mixed_stride_unknown_attributes_and_nonfinite_unused_half4() {
        for (stride, color_only) in [
            (20, false),
            (20, true),
            (24, false),
            (28, false),
            (32, false),
            (36, false),
        ] {
            let bytes = profile_fixture(stride, color_only);
            let arena = Arena::read(&bytes).unwrap();
            let declaration = &arena.objects[1];
            for offset in [
                declaration.resolved_offset + 4,
                declaration.resolved_offset + 8,
                declaration.resolved_offset + 40,
                declaration.resolved_offset + declaration.size as usize - 2,
            ] {
                let mut invalid = bytes.clone();
                invalid[offset] ^= 1;
                assert!(read_meshes(&invalid).is_err());
            }
            let mut invalid = bytes.clone();
            let vertex = arena.objects[5].resolved_offset;
            let component = if stride >= 32 { 16 } else { 12 };
            invalid[vertex + component..vertex + component + 2]
                .copy_from_slice(&0x7c00u16.to_be_bytes());
            assert!(read_meshes(&invalid).is_err());
        }
    }
}
