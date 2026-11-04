//! Metadata only: observed RW4 texture records and Xenos fetch words.
//! No tiling, mip-tail, pixel, or material-binding decoder is claimed here.
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
}
