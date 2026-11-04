//! Selected static Downtown batches assembled in their shared native coordinates.
//! This is a bounded neighborhood preview, not a complete level/streaming system.
use crate::{
    ArchiveIndex, StreamIndex,
    geometry::{self, Sample},
    inventory_disc, materials,
    rw4::Arena,
    skating::Vec3,
    texture::{self, TextureAsset},
};
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Seek},
    path::Path,
};

pub const MAX_TEXTURES: usize = 64;
const MAX_TRIANGLES: usize = 20_000;
const DECODE_BUDGET: usize = 128 * 1024 * 1024;
const TEXTURE_BUDGET: usize = 32 * 1024 * 1024;

// Explicit observed batches; other batches in these cells may remain unsupported.
const PATCH: &[(&str, &[usize])] = &[
    ("cPres_450_150_high.xsf", &[39]),
    ("cPres_550_150_high.xsf", &[15]),
    ("cPres_550_50_high.xsf", &[14]),
    ("cPres_550_250_high.xsf", &[1]),
    ("cPres_650_150_high.xsf", &[1]),
    ("cPres_650_50_high.xsf", &[7]),
];

#[derive(Debug, Copy, Clone)]
pub struct Placement {
    center: Vec3,
    floor: f32,
    scale: f32,
    destination: Vec3,
}
impl Placement {
    pub fn new(sample: &Sample, extent: f32, destination: Vec3) -> Result<Self, String> {
        if ![sample.min, sample.max, destination]
            .iter()
            .all(|p| [p.x, p.y, p.z].iter().all(|v| v.is_finite()))
            || !extent.is_finite()
            || !(0.1..=32.0).contains(&extent)
            || sample.min.x > sample.max.x
            || sample.min.y > sample.max.y
            || sample.min.z > sample.max.z
        {
            return Err("Invalid scene preview placement.".into());
        }
        let span = sample.max - sample.min;
        Ok(Self {
            center: (sample.min + sample.max) * 0.5,
            floor: sample.min.y,
            scale: extent / span.x.max(span.y).max(span.z).max(0.001),
            destination,
        })
    }
    pub fn sample(sample: &Sample) -> Result<Self, String> {
        Self::new(sample, 8.0, Vec3::new(-7.0, 0.02, -2.0))
    }
    pub fn apply(self, p: Vec3) -> Vec3 {
        Vec3::new(
            (p.x - self.center.x) * self.scale + self.destination.x,
            (p.y - self.floor) * self.scale + self.destination.y,
            (p.z - self.center.z) * self.scale + self.destination.z,
        )
    }
}

pub struct CellSummary {
    pub member: String,
    pub blocks: Vec<usize>,
    pub mesh_count: usize,
    pub triangle_count: usize,
}
pub struct Neighborhood {
    pub geometry: Sample,
    pub placement: Placement,
    /// One-based texture-array layers, with zero reserved for plain white.
    pub mesh_layers: Vec<u32>,
    pub assets: Vec<TextureAsset>,
    pub cells: Vec<CellSummary>,
    pub warnings: Vec<String>,
}

/// Required geometry errors abort the load. Optional texture/material failures
/// are reported and keep affected meshes plain. No source bytes are exported.
pub fn load_retail_neighborhood(image: &Path) -> Result<Neighborhood, String> {
    let disc = inventory_disc(image)?;
    let archive = disc
        .entries
        .iter()
        .find(|e| {
            !e.is_directory
                && e.path
                    .eq_ignore_ascii_case("data/content/worldDIST_DownTown.big")
        })
        .ok_or("Downtown archive not found.")?;
    let mut file = File::open(image).map_err(|e| e.to_string())?;
    let index = ArchiveIndex::read(&mut file, archive.image_offset, archive.size as u64)?;
    let mut geometry = Sample {
        meshes: Vec::new(),
        min: Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
        max: Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
    };
    let mut assets = Vec::new();
    let mut mesh_layers = Vec::new();
    let mut cells = Vec::new();
    let mut warnings = Vec::new();
    let mut decoded_total = 0;
    let mut texture_bytes = 0;
    let mut scanned_blocks = 0;
    for &(filename, blocks) in PATCH {
        let member_path = format!("data/content/world/stream/DIST_DownTown/{filename}");
        let member = index
            .entries
            .iter()
            .find(|e| e.path.eq_ignore_ascii_case(&member_path))
            .ok_or_else(|| format!("Neighborhood member missing: {filename}"))?;
        if member.compressed_size_field != Some(0)
            || member.size_field as u64 > member.available_bytes
        {
            return Err("Unsupported outer neighborhood compression.".into());
        }
        let base = archive
            .image_offset
            .checked_add(member.offset)
            .ok_or("Neighborhood offset overflow.")?;
        let stream = StreamIndex::read(&mut file, base, member.size_field as u64)?;
        let first_mesh = geometry.meshes.len();
        let mut diffuse_keys = Vec::new();
        for &block in blocks {
            let bytes = decode(&stream, &mut file, base, block, &mut decoded_total)?;
            let part = geometry::read_meshes(&bytes)
                .map_err(|e| format!("{filename} block {block}: {e}"))?;
            let keys = match materials::inspect_materials(&bytes) {
                Ok(bindings) => part
                    .meshes
                    .iter()
                    .map(|m| {
                        bindings
                            .iter()
                            .find(|b| b.draw_index == m.draw_index)
                            .and_then(|b| b.diffuse_key)
                    })
                    .collect(),
                Err(error) => {
                    warning(
                        &mut warnings,
                        format!("{filename} block {block}: plain materials ({error})"),
                    );
                    vec![None; part.meshes.len()]
                }
            };
            append_geometry(&mut geometry, part)?;
            diffuse_keys.extend(keys);
        }
        let wanted: HashSet<u64> = diffuse_keys.iter().flatten().copied().collect();
        let mut local = Vec::new();
        let mut ambiguous = HashSet::new();
        let mut seen_definitions = HashSet::new();
        for block in 0..stream.blocks.len() {
            scanned_blocks += 1;
            if scanned_blocks > 512 {
                return Err("Neighborhood texture scan exceeds its block budget.".into());
            }
            if blocks.contains(&block) {
                continue;
            }
            let Some(bytes) = decode_optional(
                &stream,
                &mut file,
                base,
                block,
                &mut decoded_total,
                &mut warnings,
            )?
            else {
                continue;
            };
            let arena = match Arena::read(&bytes) {
                Ok(arena) => arena,
                Err(error) => {
                    warning(
                        &mut warnings,
                        format!("{filename} block {block}: optional arena skipped ({error})"),
                    );
                    continue;
                }
            };
            if !arena.objects.iter().any(|o| o.type_id == 0x200e8) {
                continue;
            }
            let Ok((key, object)) = texture::texture_identity(&bytes) else {
                continue;
            };
            if !wanted.contains(&key) || ambiguous.contains(&key) {
                continue;
            }
            if !seen_definitions.insert(key) {
                local.retain(|&(previous, _)| previous != key);
                ambiguous.insert(key);
                warning(
                    &mut warnings,
                    format!(
                        "{filename} texture {key:016X}: duplicate local definition; kept plain"
                    ),
                );
                continue;
            }
            let pixels = texture::decode_rgb565_mip0(&bytes, object)
                .or_else(|_| texture::decode_bc1_mip0(&bytes, object));
            match pixels {
                Ok(pixels) => {
                    let layer = insert_asset(
                        &mut assets,
                        TextureAsset { key, pixels },
                        &mut texture_bytes,
                    )?;
                    local.push((key, layer));
                }
                Err(error) => warning(
                    &mut warnings,
                    format!("{filename} texture {key:016X}: plain fallback ({error})"),
                ),
            }
        }
        mesh_layers.extend(diffuse_keys.iter().map(|key| {
            key.and_then(|key| {
                local
                    .iter()
                    .find(|&&(candidate, _)| candidate == key)
                    .map(|&(_, layer)| layer)
            })
            .unwrap_or(0)
        }));
        let new_meshes = &geometry.meshes[first_mesh..];
        cells.push(CellSummary {
            member: member_path,
            blocks: blocks.to_vec(),
            mesh_count: new_meshes.len(),
            triangle_count: new_meshes.iter().map(|m| m.indices.len() / 3).sum(),
        });
    }
    if geometry.meshes.len() != mesh_layers.len() {
        return Err("Neighborhood material layer count mismatch.".into());
    }
    let placement = Placement::new(&geometry, 16.0, Vec3::new(-10.0, 0.02, -2.0))?;
    Ok(Neighborhood {
        geometry,
        placement,
        mesh_layers,
        assets,
        cells,
        warnings,
    })
}

fn decode(
    stream: &StreamIndex,
    file: &mut (impl Read + Seek),
    base: u64,
    block: usize,
    total: &mut usize,
) -> Result<Vec<u8>, String> {
    let expected = reserve_decode(stream, block, total)?;
    stream.decode_block(file, base, block, expected)
}
fn reserve_decode(stream: &StreamIndex, block: usize, total: &mut usize) -> Result<usize, String> {
    let descriptor = stream
        .blocks
        .get(block)
        .ok_or("Neighborhood block index is out of range.")?;
    let expected: u64 = descriptor
        .segments
        .iter()
        .map(|s| u64::from(s.decoded_size))
        .sum();
    let expected = usize::try_from(expected).map_err(|_| "Neighborhood decode size overflow.")?;
    let stored: u64 = descriptor
        .segments
        .iter()
        .map(|s| u64::from(s.stored_size))
        .sum();
    let charge =
        expected.max(usize::try_from(stored).map_err(|_| "Neighborhood stored size overflow.")?);
    if charge > 64 * 1024 * 1024 || total.checked_add(charge).is_none_or(|n| n > DECODE_BUDGET) {
        return Err("Neighborhood decode budget exhausted.".into());
    }
    // Failed optional decodes still consume their declared budget, preventing
    // repeated malformed inputs from avoiding the aggregate work limit.
    *total += charge;
    Ok(expected)
}
fn decode_optional(
    stream: &StreamIndex,
    file: &mut (impl Read + Seek),
    base: u64,
    block: usize,
    total: &mut usize,
    warnings: &mut Vec<String>,
) -> Result<Option<Vec<u8>>, String> {
    let expected = reserve_decode(stream, block, total)?;
    match stream.decode_block(file, base, block, expected) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) => {
            warning(
                warnings,
                format!("Optional texture block {block} skipped ({error})"),
            );
            Ok(None)
        }
    }
}
fn warning(warnings: &mut Vec<String>, message: String) {
    if warnings.len() < 32 {
        warnings.push(message);
    }
}
fn append_geometry(target: &mut Sample, part: Sample) -> Result<(), String> {
    let triangles: usize = target
        .meshes
        .iter()
        .chain(&part.meshes)
        .map(|m| m.indices.len() / 3)
        .sum();
    let vertices: usize = target
        .meshes
        .iter()
        .chain(&part.meshes)
        .map(|m| m.positions.len())
        .sum();
    if triangles > MAX_TRIANGLES
        || vertices > 100_000
        || target.meshes.len() + part.meshes.len() > 4096
    {
        return Err("Neighborhood geometry exceeds its preview/collision budget.".into());
    }
    target.min = Vec3::new(
        target.min.x.min(part.min.x),
        target.min.y.min(part.min.y),
        target.min.z.min(part.min.z),
    );
    target.max = Vec3::new(
        target.max.x.max(part.max.x),
        target.max.y.max(part.max.y),
        target.max.z.max(part.max.z),
    );
    target.meshes.extend(part.meshes);
    Ok(())
}
fn insert_asset(
    assets: &mut Vec<TextureAsset>,
    asset: TextureAsset,
    total: &mut usize,
) -> Result<u32, String> {
    // Only identical payloads are deduplicated. Same-key local LOD variants may
    // retain separate layers; references never borrow an unrelated cell's data.
    if let Some(index) = assets.iter().position(|a| {
        a.key == asset.key
            && a.pixels.width == asset.pixels.width
            && a.pixels.height == asset.pixels.height
            && a.pixels.rgba8 == asset.pixels.rgba8
    }) {
        return Ok(index as u32 + 1);
    }
    if assets.len() >= MAX_TEXTURES
        || total
            .checked_add(asset.pixels.rgba8.len())
            .is_none_or(|size| size > TEXTURE_BUDGET)
    {
        return Err("Neighborhood texture budget exceeded.".into());
    }
    *total += asset.pixels.rgba8.len();
    assets.push(asset);
    Ok(assets.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(min: Vec3, max: Vec3) -> Sample {
        Sample {
            meshes: Vec::new(),
            min,
            max,
        }
    }
    #[test]
    fn placement_preserves_distances_and_original_sample_mapping() {
        let a = sample(Vec3::new(500., 50., 100.), Vec3::new(600., 150., 200.));
        let placement = Placement::sample(&a).unwrap();
        for p in [a.min, a.max, Vec3::new(550., 70., 125.)] {
            assert_eq!(placement.apply(p), a.preview_position(p));
        }
        let joined = sample(Vec3::new(400., 30., 0.), Vec3::new(700., 160., 300.));
        let placement = Placement::new(&joined, 16., Vec3::new(-10., 0.02, -2.)).unwrap();
        let seam = Vec3::new(600., 100., 100.);
        let delta = placement.apply(seam + Vec3::new(100., 0., 0.)) - placement.apply(seam);
        assert!((delta.x - 16. / 3.).abs() < 0.00001 && delta.y == 0. && delta.z == 0.);
        assert!(Placement::new(&joined, f32::NAN, Vec3::default()).is_err());
    }
    #[test]
    fn adjacent_batches_keep_native_vertices_and_share_render_collision_placement() {
        let make = |positions: Vec<Vec3>, min, max| Sample {
            meshes: vec![geometry::Mesh {
                draw_index: 0,
                uv: vec![[0.; 2]; 3],
                indices: vec![0, 1, 2],
                positions,
            }],
            min,
            max,
        };
        let mut joined = make(
            vec![
                Vec3::new(0., 2., 0.),
                Vec3::new(1., 2., 0.),
                Vec3::new(1., 2., 1.),
            ],
            Vec3::new(0., 2., 0.),
            Vec3::new(1., 2., 1.),
        );
        let right = make(
            vec![
                Vec3::new(1., 2., 0.),
                Vec3::new(2., 2., 0.),
                Vec3::new(1., 2., 1.),
            ],
            Vec3::new(1., 2., 0.),
            Vec3::new(2., 2., 1.),
        );
        append_geometry(&mut joined, right).unwrap();
        assert_eq!(joined.meshes[0].positions[1], Vec3::new(1., 2., 0.));
        assert_eq!(joined.meshes[1].positions[0], Vec3::new(1., 2., 0.));
        let placement = Placement::new(&joined, 16., Vec3::new(-10., 0.02, -2.)).unwrap();
        let left_seam = placement.apply(joined.meshes[0].positions[1]);
        let right_seam = placement.apply(joined.meshes[1].positions[0]);
        assert_eq!(left_seam, right_seam);
        assert_eq!(left_seam, Vec3::new(-10., 0.02, -6.));
        let collision =
            crate::collision::TriangleMesh::from_sample_at(&joined, |p| placement.apply(p))
                .unwrap();
        let center = placement.apply(Vec3::new(2. / 3., 2., 1. / 3.));
        assert!((collision.height(center.x, center.z, 1.).unwrap() - center.y).abs() < 0.00001);
    }
    #[test]
    fn optional_codec_failure_keeps_fallback_but_cannot_bypass_work_budget() {
        let empty = crate::StreamSegment {
            offset: 0,
            decoded_size: 0,
            stored_size: 0,
            codec_flags: 0,
        };
        let stream = StreamIndex {
            length: 0,
            header_words: [0; 6],
            blocks: vec![crate::StreamBlock {
                offset: 0,
                identifier_words: [0; 2],
                payload_size: 0,
                padded_span: 0,
                segments: [
                    crate::StreamSegment {
                        decoded_size: 8,
                        codec_flags: 99,
                        ..empty.clone()
                    },
                    empty,
                ],
            }],
        };
        let mut input = std::io::Cursor::new(Vec::<u8>::new());
        let mut total = 0;
        let mut warnings = Vec::new();
        assert!(
            decode_optional(&stream, &mut input, 0, 0, &mut total, &mut warnings)
                .unwrap()
                .is_none()
        );
        assert_eq!(total, 8);
        assert_eq!(warnings.len(), 1);
        assert!(decode(&stream, &mut input, 0, 0, &mut total).is_err());
        total = DECODE_BUDGET - 4;
        assert!(decode_optional(&stream, &mut input, 0, 0, &mut total, &mut warnings).is_err());
        assert_eq!(warnings.len(), 1);
    }
    #[test]
    fn identical_textures_deduplicate_but_local_variants_keep_distinct_layers() {
        let mut assets = Vec::new();
        let mut total = 0;
        let asset = |v| TextureAsset {
            key: 1,
            pixels: texture::DecodedTexture {
                width: 1,
                height: 1,
                rgba8: vec![v; 4],
            },
        };
        assert_eq!(insert_asset(&mut assets, asset(1), &mut total).unwrap(), 1);
        assert_eq!(insert_asset(&mut assets, asset(1), &mut total).unwrap(), 1);
        assert_eq!(insert_asset(&mut assets, asset(2), &mut total).unwrap(), 2);
        assert_eq!(total, 8);
    }
    #[test]
    #[ignore = "Requires THREECOMP_TEST_ISO pointing to the local retail disc image"]
    fn retail_neighborhood_preserves_all_selected_batches_and_collision() {
        let path = std::env::var_os("THREECOMP_TEST_ISO").expect("Set THREECOMP_TEST_ISO");
        let patch = load_retail_neighborhood(Path::new(&path)).unwrap();
        assert_eq!(patch.cells.len(), 6);
        assert_eq!(patch.geometry.meshes.len(), 64);
        assert_eq!(
            patch
                .geometry
                .meshes
                .iter()
                .map(|m| m.indices.len() / 3)
                .sum::<usize>(),
            3481
        );
        let collision = crate::collision::TriangleMesh::from_sample_at(&patch.geometry, |p| {
            patch.placement.apply(p)
        })
        .unwrap();
        // Four zero-area render faces are discarded by collision construction.
        assert_eq!(collision.triangle_count(), 3477);
        let placement = collision.placement().unwrap();
        let world = crate::collision::World {
            mesh: Some(&collision),
        };
        let mut skater = crate::skating::Skater::default();
        skater.position = placement;
        skater.step_on(
            crate::skating::Input {
                ollie: true,
                ..Default::default()
            },
            crate::skating::FIXED_DT,
            &world,
        );
        for _ in 0..120 {
            skater.step_on(Default::default(), crate::skating::FIXED_DT, &world);
        }
        assert!(skater.grounded && (skater.position.y - placement.y).abs() < 0.001);
        let textured = patch.mesh_layers.iter().filter(|&&layer| layer > 0).count();
        assert!(textured >= 6 && patch.assets.len() >= 4);
        assert!(
            patch
                .mesh_layers
                .iter()
                .all(|&l| l as usize <= patch.assets.len())
        );
        eprintln!(
            "Neighborhood: {} cells / {} meshes /3481 triangles /{textured} textured /{} textures /{} warnings",
            patch.cells.len(),
            patch.geometry.meshes.len(),
            patch.assets.len(),
            patch.warnings.len()
        );
    }
}
