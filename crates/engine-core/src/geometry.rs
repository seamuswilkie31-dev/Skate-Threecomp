//! Narrow, original reader for the static float3/28-byte mesh profile observed
//! in a retail presentation block. Other vertex and primitive layouts error.
use crate::{ArchiveIndex, StreamIndex, inventory_disc, rw4::Arena, skating::Vec3};
use std::{fs::File, path::Path};

#[derive(Debug, Clone)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub indices: Vec<u32>,
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
        let layout = object(&arena, bytes, word(d, 40) as usize, 0x200e9, 84)?;
        if layout.len() != 84
            || word(layout, 16) != 0
            || word(layout, 20) != 0x002a23b9
            || layout[80..84] != [28; 4]
        {
            return Err(
                "Unsupported vertex declaration (expected observed float3/28-byte profile).".into(),
            );
        }
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
            || length % 28 != 0
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
        vertex_total += length / 28;
        index_total += index_count;
        if vertex_total > 1_000_000 || index_total > 3_000_000 || meshes.len() >= 4096 {
            return Err("Geometry exceeds the preview budget.".into());
        }
        let mut positions = Vec::with_capacity(length / 28);
        for record in vertex_bytes[..length].chunks_exact(28) {
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
        }
        let indices: Vec<u32> = index_bytes[..index_count * 2]
            .chunks_exact(2)
            .map(|p| u16::from_be_bytes(p.try_into().unwrap()) as u32)
            .collect();
        if indices.iter().any(|&i| i as usize >= positions.len()) {
            return Err("Triangle index exceeds its vertex buffer.".into());
        }
        meshes.push(Mesh { positions, indices });
    }
    if meshes.is_empty() {
        return Err("No supported static draw records found in this arena.".into());
    }
    Ok(Sample { meshes, min, max })
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

#[cfg(test)]
mod tests {
    use super::*;
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
            (288 + 20, 0x002a23b9),
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
        ] {
            let mut bytes = fixture();
            put(&mut bytes, offset, value);
            assert!(read_meshes(&bytes).is_err());
        }
        let mut bytes = fixture();
        bytes[729] = 3;
        assert!(read_meshes(&bytes).is_err());
    }
}
