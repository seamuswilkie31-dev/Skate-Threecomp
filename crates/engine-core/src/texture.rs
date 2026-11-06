//! Observed texture metadata, canonical keys, and narrow tiled base-level decoders.
use crate::rw4::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureInfo {
    pub object_index: usize,
    pub resource_object: usize,
    pub resource_bytes: usize,
    pub width: u32,
    pub height: u32,
    pub pitch_pixels: u32,
    pub format_id: u32,
    pub endian_mode: u32,
    pub tiled: bool,
    pub packed_mips: bool,
    pub fetch_words: [u32; 6],
}

#[derive(Debug, Clone)]
pub struct DecodedTexture {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}
pub struct TextureAsset {
    pub key: u64,
    pub pixels: DecodedTexture,
}
pub fn decode_rgb565_mip0(bytes: &[u8], object_index: usize) -> Result<DecodedTexture, String> {
    let textures = inspect_textures(bytes)?;
    let t = textures
        .iter()
        .find(|t| t.object_index == object_index)
        .ok_or("Texture descriptor not found.")?;
    if t.format_id != 4
        || t.endian_mode != 1
        || !t.tiled
        || t.fetch_words[0] & 0x3fc != 0
        || t.fetch_words[1] >> 12 != 0
        || t.fetch_words[3] & 0x7ffff != 0x1414
        || (t.fetch_words[4] >> 2) & 15 != 0
    {
        return Err("Unsupported RGB565 base-level fetch profile.".into());
    }
    let arena = Arena::read(bytes)?;
    let rgba8 = crate::rgb565::decode(
        arena.object_data(bytes, t.resource_object)?,
        t.width,
        t.height,
        t.pitch_pixels,
    )?;
    Ok(DecodedTexture {
        width: t.width,
        height: t.height,
        rgba8,
    })
}
/// Canonical key is explicitly named in the texture definition; a single
/// observed storage flag may differ in the numeric key. Other bits are retained.
pub(crate) fn texture_identity(bytes: &[u8]) -> Result<(u64, usize), String> {
    let arena = Arena::read(bytes)?;
    let tables: Vec<_> = arena
        .objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.type_id == 0xeb000b)
        .collect();
    if tables.len() != 1 {
        return Err("Expected one texture definition table.".into());
    }
    let d = arena.object_data(bytes, tables[0].0)?;
    definition_identity(d, &inspect_textures(bytes)?)
}
fn definition_identity(d: &[u8], textures: &[TextureInfo]) -> Result<(u64, usize), String> {
    let key = named_texture_key(d)?;
    let object_index = word(d, 40) as usize;
    if textures.len() != 1 || textures[0].object_index != object_index {
        return Err("Texture definition does not reference the sole texture descriptor.".into());
    }
    Ok((key, object_index))
}
fn named_texture_key(d: &[u8]) -> Result<u64, String> {
    if d.len() < 44
        || word(d, 0) != 1
        || word(d, 4) != 20
        || word(d, 8) != 44
        || word(d, 20) != 44
        || word(d, 24) != 0x9b0f1678
        || word(d, 36) != 0xac462e4a
        || word(d, 16) as usize != d.len()
    {
        return Err("Unsupported texture key table.".into());
    }
    let end = d[44..]
        .iter()
        .position(|&b| b == 0)
        .ok_or("Unterminated texture key name.")?;
    let name = std::str::from_utf8(&d[44..44 + end]).map_err(|_| "Invalid texture key name.")?;
    if name.len() != 26 || !name.starts_with("0x") || !name.ends_with(".Texture") {
        return Err("Unsupported canonical texture key name.".into());
    }
    let key =
        u64::from_str_radix(&name[2..18], 16).map_err(|_| "Invalid hexadecimal texture key.")?;
    let raw = (u64::from(word(d, 28)) << 32) | u64::from(word(d, 32));
    if raw != key && raw != (key | 0x4000000000000000) {
        return Err("Numeric and named texture keys disagree.".into());
    }
    Ok(key)
}
pub fn load_retail_diffuse_textures(image: &std::path::Path) -> Result<Vec<TextureAsset>, String> {
    let mut assets = Vec::new();
    for (block, expected) in [
        (4, 0x00008cfe03e3870au64),
        (5, 0x00008d0703e3870a),
        (6, 0x00008d1f03e3870a),
        (7, 0x2c70170a00050227),
    ] {
        let bytes = crate::geometry::load_retail_block(image, block)?;
        let (key, object_index) = texture_identity(&bytes)?;
        if key != expected {
            return Err("Retail diffuse sample key differs from the researched profile.".into());
        }
        assets.push(TextureAsset {
            key,
            pixels: decode_rgb565_mip0(&bytes, object_index)?,
        });
    }
    Ok(assets)
}

/// Decode the bounded base level only. Packed lower mips are permitted when
/// their page address describes the verified contiguous region after a complete
/// base surface. Mip-tail pixels and GPU host pointers are never followed.
pub fn decode_bc1_mip0(bytes: &[u8], object_index: usize) -> Result<DecodedTexture, String> {
    let textures = inspect_textures(bytes)?;
    let info = textures
        .iter()
        .find(|t| t.object_index == object_index)
        .ok_or("Texture descriptor not found.")?;
    if info.format_id != 18
        || info.endian_mode != 1
        || !info.tiled
        || info.fetch_words[0] & 0x3fc != 0
        || info.fetch_words[1] >> 12 != 0
        || info.fetch_words[5] & 4 != 0
        || info.fetch_words[3] & 0x7ffff != 0xd10
        || (info.fetch_words[4] >> 2) & 15 != 0
    {
        return Err("Unsupported BC1 preview profile (requires tiled mip zero, identity swizzle, unsigned channels, endian mode 1, and a zero base offset).".into());
    }
    let arena = Arena::read(bytes)?;
    let resource = arena.object_data(bytes, info.resource_object)?;
    validate_bc1_mip_region(info, resource.len())?;
    let rgba8 = crate::bc1::decode(resource, info.width, info.height, info.pitch_pixels)?;
    Ok(DecodedTexture {
        width: info.width,
        height: info.height,
        rgba8,
    })
}

fn validate_bc1_mip_region(info: &TextureInfo, resource_bytes: usize) -> Result<(), String> {
    // Xenos fetch word 5 stores packed_mips at bit 11 and the mip page address
    // at bits 12..31. These serialized assets use a zero base and a relative
    // mip offset; accept only an offset exactly after the local base footprint.
    let mip_offset = (info.fetch_words[5] & 0xfffff000) as usize;
    if !info.packed_mips && mip_offset == 0 {
        return Ok(());
    }
    let unsupported = || {
        "Unsupported BC1 preview profile (mip region must follow a complete, page-aligned local base surface).".to_string()
    };
    if !(128..=4096).contains(&info.width)
        || !(128..=4096).contains(&info.height)
        || !info.width.is_power_of_two()
        || !info.height.is_power_of_two()
        || info.pitch_pixels != info.width
    {
        return Err(unsupported());
    }
    // At this size each axis contains whole 32x32 BC1-block tiles, so the
    // tiled base footprint equals width*height/2 and is outside the packed tail.
    let base_bytes = info.width as usize * info.height as usize / 2;
    let max_mip = (info.fetch_words[4] >> 6) & 15;
    let first_mip_bytes = ((info.width / 8).div_ceil(32) as usize)
        * ((info.height / 8).div_ceil(32) as usize)
        * 32
        * 32
        * 8;
    let remaining = resource_bytes
        .checked_sub(mip_offset)
        .ok_or_else(unsupported)?;
    if mip_offset != base_bytes
        || !mip_offset.is_multiple_of(4096)
        || !resource_bytes.is_multiple_of(4096)
        || max_mip == 0
        || max_mip > info.width.max(info.height).ilog2()
        || remaining < first_mip_bytes
    {
        return Err(unsupported());
    }
    Ok(())
}

pub fn load_retail_texture(image: &std::path::Path) -> Result<DecodedTexture, String> {
    let bytes = crate::geometry::load_retail_block(image, 0)?;
    let object_index = inspect_textures(&bytes)?
        .first()
        .ok_or("Sample block contains no texture.")?
        .object_index;
    decode_bc1_mip0(&bytes, object_index)
}
pub fn inspect_textures(bytes: &[u8]) -> Result<Vec<TextureInfo>, String> {
    let arena = Arena::read(bytes)?;
    let mut textures = Vec::new();
    for (object_index, _) in arena
        .objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.type_id == 0x200e8)
    {
        let data = arena.object_data(bytes, object_index)?;
        if data.len() != 52 {
            return Err("Unsupported RW4 texture descriptor length.".into());
        }
        let reference = word(data, 0);
        if reference & 3 != 3 {
            return Err("Unsupported texture resource reference encoding.".into());
        }
        let resource_object = (reference >> 2) as usize;
        let resource = arena
            .objects
            .get(resource_object)
            .ok_or("Texture resource index is out of range.")?;
        if resource.type_id != 0x10031 {
            return Err("Texture resource has an unexpected type.".into());
        }
        let fetch_words = std::array::from_fn(|index| word(data, 28 + index * 4));
        if fetch_words[0] & 3 != 2
            || (fetch_words[5] >> 9) & 3 != 1
            || fetch_words[1] & (1 << 10) != 0
        {
            return Err(
                "Only the observed non-stacked 2D texture metadata profile is recognized.".into(),
            );
        }
        let width = (fetch_words[2] & 0x1fff) + 1;
        let height = ((fetch_words[2] >> 13) & 0x1fff) + 1;
        textures.push(TextureInfo {
            object_index,
            resource_object,
            resource_bytes: arena.object_data(bytes, resource_object)?.len(),
            width,
            height,
            pitch_pixels: ((fetch_words[0] >> 22) & 0x1ff) * 32,
            format_id: fetch_words[1] & 63,
            endian_mode: (fetch_words[1] >> 6) & 3,
            tiled: fetch_words[0] >> 31 != 0,
            packed_mips: fetch_words[5] & (1 << 11) != 0,
            fetch_words,
        });
    }
    Ok(textures)
}
fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_names_preserve_key_bits_and_validate_storage_flag() {
        for (name_key, stored) in [
            (0x00008cfe03e3870au64, 0x40008cfe03e3870au64),
            (0x8cfffed701ee37f8, 0x8cfffed701ee37f8),
        ] {
            let mut blob = vec![0; 72];
            for (offset, value) in [
                (0, 1),
                (4, 20),
                (8, 44),
                (16, 72),
                (20, 44),
                (24, 0x9b0f1678),
                (28, (stored >> 32) as u32),
                (32, stored as u32),
                (36, 0xac462e4a),
            ] {
                put(&mut blob, offset, value);
            }
            let name = format!("0x{name_key:016x}.Texture");
            blob[44..70].copy_from_slice(name.as_bytes());
            assert_eq!(named_texture_key(&blob).unwrap(), name_key);
            let textures = inspect_textures(&fixture()).unwrap();
            assert_eq!(
                definition_identity(&blob, &textures).unwrap(),
                (name_key, 0)
            );
            let mut bad_link = blob.clone();
            put(&mut bad_link, 40, 99);
            assert!(definition_identity(&bad_link, &textures).is_err());
            for field in [24, 36] {
                let mut bad_kind = blob.clone();
                put(&mut bad_kind, field, 0);
                assert!(named_texture_key(&bad_kind).is_err());
            }
            blob[32] ^= 1;
            assert!(named_texture_key(&blob).is_err());
        }
    }
    fn put(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 336];
        bytes[..24].copy_from_slice(b"\x89RW4xb2\0\r\n\x1a\n\x01\x20\x04\x00454\x00000\x00");
        for (o, v) in [
            (0x20, 2),
            (0x24, 2),
            (0x30, 224),
            (0x44, 272),
            (0x48, 4),
            (0x50, 1),
            (0x54, 64),
            (0x58, 4),
            (0x60, 1),
            (0x68, 1),
            (224, 172),
            (232, 52),
            (236, 4),
            (244, 0x200e8),
            (256, 64),
            (260, 4),
            (268, 0x10031),
            (172, 7),
            (172 + 28, 0x82000002),
            (172 + 32, 0x52),
            (172 + 36, 0x001fe0ff),
            (172 + 48, 0x8a00),
        ] {
            put(&mut bytes, o, v);
        }
        bytes
    }
    #[test]
    fn decodes_synthetic_texture_fields_without_reading_pixels() {
        let textures = inspect_textures(&fixture()).unwrap();
        assert_eq!(textures.len(), 1);
        let t = &textures[0];
        assert_eq!((t.width, t.height, t.pitch_pixels), (256, 256, 256));
        assert_eq!((t.format_id, t.endian_mode), (0x12, 1));
        assert!(t.tiled && t.packed_mips);
        assert_eq!(t.resource_bytes, 64); // Metadata does not promise a complete pixel footprint.
    }
    #[test]
    fn rejects_invalid_links_and_dimension_profile() {
        for (offset, value) in [(172, 4003), (172, 4), (172 + 48, 0), (232, 48)] {
            let mut bytes = fixture();
            put(&mut bytes, offset, value);
            assert!(inspect_textures(&bytes).is_err());
        }
    }
    #[test]
    fn pixel_decoder_rejects_unsupported_modes_before_using_resource() {
        let mut bytes = fixture();
        put(&mut bytes, 172 + 48, 0x200);
        for (offset, value) in [
            (172 + 48, 0x8a00),
            (172 + 48, 0x204),
            (172 + 32, 0x54),
            (172 + 28, 0x02000002),
            (172 + 32, 0x92),
            (172 + 40, 0),
        ] {
            let mut candidate = bytes.clone();
            put(&mut candidate, offset, value);
            assert!(
                decode_bc1_mip0(&candidate, 0)
                    .unwrap_err()
                    .contains("Unsupported BC1 preview profile")
            );
        }
        // Even the supported shape cannot decode a deliberately tiny resource.
        put(&mut bytes, 172 + 48, 0x200);
        assert!(decode_bc1_mip0(&bytes, 0).is_err());
    }
    fn mip_fixture(width: u32, height: u32, mip_offset: u32, resource_bytes: usize) -> Vec<u8> {
        let mut bytes = fixture();
        bytes.resize(272 + resource_bytes, 0);
        for (offset, value) in [
            (0x54, resource_bytes as u32),
            (256, resource_bytes as u32),
            (172 + 28, 0x80000002 | ((width / 32) << 22)),
            (172 + 36, (width - 1) | ((height - 1) << 13)),
            (172 + 40, 0xd10),
            (172 + 44, width.max(height).ilog2() << 6),
            (172 + 48, mip_offset | 0xa00),
        ] {
            put(&mut bytes, offset, value);
        }
        // A red BC1 block at local base offset zero. Appended mips use green
        // markers, which must never contribute pixels to the base-level image.
        bytes[272..280].copy_from_slice(&[0xf8, 0, 7, 0xe0, 0, 0, 0, 0]);
        if mip_offset as usize + 8 <= resource_bytes {
            let offset = 272 + mip_offset as usize;
            bytes[offset..offset + 8].copy_from_slice(&[7, 0xe0, 0xf8, 0, 0, 0, 0, 0]);
        }
        bytes
    }
    #[test]
    fn bc1_mip_region_is_local_bounded_and_not_sampled() {
        let mut bytes = mip_fixture(512, 256, 0x10000, 106496);
        let decoded = decode_bc1_mip0(&bytes, 0).unwrap();
        assert_eq!((decoded.width, decoded.height), (512, 256));
        assert_eq!(&decoded.rgba8[..4], &[255, 0, 0, 255]);
        // Changing every byte in the bounded mip region cannot change mip zero.
        bytes[272 + 0x10000..].fill(0xff);
        assert_eq!(decode_bc1_mip0(&bytes, 0).unwrap().rgba8, decoded.rgba8);
        // A complete no-mips texture has the same base output.
        put(&mut bytes, 172 + 48, 0x200);
        put(&mut bytes, 172 + 44, 0);
        assert_eq!(decode_bc1_mip0(&bytes, 0).unwrap().rgba8, decoded.rgba8);
    }
    #[test]
    fn bc1_rejects_overlapping_external_or_truncated_mip_regions() {
        for (mip_offset, resource_bytes) in [
            (0, 106496),
            (0xf000, 106496),
            (0x11000, 106496),
            (0xfffff000, 106496),
            (0x10000, 65536),
            (0x10000, 65536 + 4096),
            (0x10000, 106495),
        ] {
            let bytes = mip_fixture(512, 256, mip_offset, resource_bytes);
            assert!(decode_bc1_mip0(&bytes, 0).is_err());
        }
        for (width, height) in [(64, 64), (384, 256), (512, 64)] {
            assert!(decode_bc1_mip0(&mip_fixture(width, height, 0x10000, 106496), 0).is_err());
        }
        let mut bytes = mip_fixture(512, 256, 0x10000, 106496);
        for (offset, value) in [
            (172 + 32, 0x1052),  // Nonzero base address remains unsupported.
            (172 + 44, 0),       // Mip bytes with no declared mip level.
            (172 + 44, 15 << 6), // More levels than the dimensions can contain.
        ] {
            let old = word(&bytes, offset);
            put(&mut bytes, offset, value);
            assert!(decode_bc1_mip0(&bytes, 0).is_err());
            put(&mut bytes, offset, old);
        }
    }
    #[test]
    #[ignore = "Requires THREECOMP_TEST_ISO pointing to the local retail disc image"]
    fn retail_bc1_base_level_decodes_with_varied_opaque_pixels() {
        let path = std::env::var_os("THREECOMP_TEST_ISO").expect("Set THREECOMP_TEST_ISO");
        let texture = load_retail_texture(std::path::Path::new(&path)).unwrap();
        assert_eq!((texture.width, texture.height), (512, 512));
        assert_eq!(texture.rgba8.len(), 512 * 512 * 4);
        let colors: std::collections::HashSet<_> = texture
            .rgba8
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect();
        assert!(colors.len() > 256);
        assert!(texture.rgba8.chunks_exact(4).all(|p| p[3] == 255));
        let checksum = texture
            .rgba8
            .iter()
            .fold(0xcbf29ce484222325u64, |hash, &byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        assert_eq!(checksum, 0xe1eae908fdff7c68);
        eprintln!(
            "Decoded retail lightmap: {} distinct RGBA colors",
            colors.len()
        );
    }
    #[test]
    #[ignore = "Requires THREECOMP_TEST_ISO pointing to the local retail disc image"]
    fn retail_bc1_base_with_separate_mips_resolves_three_local_meshes() {
        let path = std::env::var_os("THREECOMP_TEST_ISO").expect("Set THREECOMP_TEST_ISO");
        let patch = crate::scene::load_retail_neighborhood(std::path::Path::new(&path)).unwrap();
        assert_eq!(
            patch.mesh_layers.iter().filter(|&&layer| layer > 0).count(),
            57
        );
        assert_eq!(patch.assets.len(), 15);
        for (key, checksum) in [
            (0x2c70170a000d01b1, 0xe0d0c50b76449046),
            (0x2c70170a001d0183, 0xaa5fa63c9da517c7),
        ] {
            let asset = patch.assets.iter().find(|asset| asset.key == key).unwrap();
            assert_eq!((asset.pixels.width, asset.pixels.height), (512, 256));
            let actual = asset
                .pixels
                .rgba8
                .iter()
                .fold(0xcbf29ce484222325u64, |hash, &byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
                });
            assert_eq!(actual, checksum);
        }
    }
}
