//! Original mip-zero decoder for the observed tiled, 8-in-16-endian BC1 profile.

pub fn decode(data: &[u8], width: u32, height: u32, pitch_pixels: u32) -> Result<Vec<u8>, String> {
    if !(128..=4096).contains(&width)
        || !(128..=4096).contains(&height)
        || !width.is_power_of_two()
        || !height.is_power_of_two()
        || pitch_pixels != width
    {
        return Err(
            "BC1 preview requires power-of-two dimensions from 128 to 4096 and matching pitch."
                .into(),
        );
    }
    let required = width as usize * height as usize / 2;
    if data.len() < required {
        return Err("Tiled BC1 base level is truncated.".into());
    }
    let mut output = vec![0; width as usize * height as usize * 4];
    let pitch_blocks = (pitch_pixels / 4).div_ceil(32) * 32;
    for y in 0..height / 4 {
        for x in 0..width / 4 {
            let offset = block_address(x, y, pitch_blocks);
            let bytes = data
                .get(offset..offset + 8)
                .ok_or("Tiled BC1 block extends outside its resource.")?;
            // Endian mode 1 reverses the bytes of each 16-bit word independently.
            let block = std::array::from_fn(|i| bytes[i ^ 1]);
            let pixels = decode_block(block);
            for row in 0..4 {
                for column in 0..4 {
                    let destination = (((y * 4 + row) * width + x * 4 + column) * 4) as usize;
                    output[destination..destination + 4]
                        .copy_from_slice(&pixels[(row * 4 + column) as usize]);
                }
            }
        }
    }
    Ok(output)
}

// Deposit coordinate bits according to the 8-byte-block hardware address map.
// This is deliberately specialized to BC1, not a generalized console untile tool.
fn block_address(x: u32, y: u32, pitch_blocks: u32) -> usize {
    let tile = (y / 32) * (pitch_blocks / 32) + x / 32;
    let mut address = (tile as usize) << 13;
    for (coordinate, source, destination) in [
        (x, 0, 3),
        (y, 0, 4),
        (x, 1, 5),
        (x, 3, 6),
        (x, 2, 8),
        (y, 1, 9),
        (y, 2, 10),
        (y, 4, 11),
        (y, 3, 12),
    ] {
        address |= (((coordinate >> source) & 1) as usize) << destination;
    }
    address |= ((((x >> 4) ^ (y >> 3)) & 1) as usize) << 7;
    address
}

fn decode_block(bytes: [u8; 8]) -> [[u8; 4]; 16] {
    let endpoint0 = u16::from_le_bytes([bytes[0], bytes[1]]);
    let endpoint1 = u16::from_le_bytes([bytes[2], bytes[3]]);
    let unpack = |value: u16| {
        let r = ((value >> 11) & 31) as u8;
        let g = ((value >> 5) & 63) as u8;
        let b = (value & 31) as u8;
        [
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
            255,
        ]
    };
    let mut palette = [[0; 4]; 4];
    palette[0] = unpack(endpoint0);
    palette[1] = unpack(endpoint1);
    let first = palette[0];
    let second = palette[1];
    if endpoint0 > endpoint1 {
        for (channel, (a, b)) in first.into_iter().zip(second).take(3).enumerate() {
            palette[2][channel] = ((2 * u16::from(a) + u16::from(b)) / 3) as u8;
            palette[3][channel] = ((u16::from(a) + 2 * u16::from(b)) / 3) as u8;
        }
        palette[2][3] = 255;
        palette[3][3] = 255;
    } else {
        for (channel, (a, b)) in first.into_iter().zip(second).take(3).enumerate() {
            palette[2][channel] = ((u16::from(a) + u16::from(b)) / 2) as u8;
        }
        palette[2][3] = 255;
    }
    let selectors = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    std::array::from_fn(|pixel| palette[((selectors >> (pixel * 2)) & 3) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_bc1_palette_and_transparency() {
        let pixels = decode_block([0, 0xf8, 0xe0, 7, 0xe4, 0xe4, 0xe4, 0xe4]);
        assert_eq!(
            &pixels[..4],
            &[
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [170, 85, 0, 255],
                [85, 170, 0, 255]
            ]
        );
        let transparent = decode_block([0, 0, 255, 255, 0xe4, 0, 0, 0]);
        assert_eq!(
            &transparent[..4],
            &[
                [0, 0, 0, 255],
                [255, 255, 255, 255],
                [127, 127, 127, 255],
                [0, 0, 0, 0]
            ]
        );
    }
    #[test]
    fn golden_tiled_block_offsets_and_complete_bijection() {
        for (x, y, expected) in [
            (1, 0, 0x8),
            (2, 0, 0x20),
            (4, 0, 0x100),
            (8, 0, 0x40),
            (16, 0, 0x80),
            (0, 8, 0x1080),
            (16, 8, 0x1000),
            (15, 31, 0x1ff8),
            (31, 31, 0x1f78),
            (32, 0, 0x2000),
            (0, 32, 0x8000),
            (32, 32, 0xa000),
            (127, 127, 0x1ff78),
            (111, 127, 0x1fff8),
        ] {
            assert_eq!(block_address(x, y, 128), expected);
        }
        let mut addresses = std::collections::HashSet::new();
        for y in 0..128 {
            for x in 0..128 {
                let offset = block_address(x, y, 128);
                assert!(offset.is_multiple_of(8) && offset + 8 <= 131072);
                assert!(addresses.insert(offset));
            }
        }
        assert_eq!(addresses.len(), 16384);
    }
    #[test]
    fn fixed_marker_addresses_verify_endian_and_tile_boundaries() {
        let mut data = vec![0; 131072];
        // Explicit independently tabulated byte offsets, not generated by block_address.
        for offset in [0x8, 0x1080, 0xa000, 0x1fff8] {
            data[offset..offset + 8].copy_from_slice(&[0xf8, 0, 0, 0, 0, 0, 0, 0]);
        }
        let pixels = decode(&data, 512, 512, 512).unwrap();
        for (x, y) in [(1, 0), (0, 8), (32, 32), (111, 127)] {
            let offset = ((y * 4 * 512 + x * 4) * 4) as usize;
            assert_eq!(&pixels[offset..offset + 4], &[255, 0, 0, 255]);
        }
        assert!(decode(&data[..100], 512, 512, 512).is_err());
        assert!(decode(&data, 8192, 512, 8192).is_err());
        assert!(decode(&data, 512, 512, 1024).is_err());
    }
}
