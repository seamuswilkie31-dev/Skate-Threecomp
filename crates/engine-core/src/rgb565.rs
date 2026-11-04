//! Tiled RGB565 base levels whose minimum dimension is outside the packed tail.
pub fn decode(data: &[u8], width: u32, height: u32, pitch: u32) -> Result<Vec<u8>, String> {
    if !(32..=4096).contains(&width)
        || !(32..=4096).contains(&height)
        || !width.is_power_of_two()
        || !height.is_power_of_two()
        || pitch != width
    {
        return Err(
            "RGB565 base preview requires matching power-of-two dimensions/pitch from 32 to 4096."
                .into(),
        );
    }
    if data.len() < width as usize * height as usize * 2 {
        return Err("RGB565 resource is truncated.".into());
    }
    let mut output = vec![0; width as usize * height as usize * 4];
    for y in 0..height {
        for x in 0..width {
            let offset = address(x, y, pitch);
            let pair = data
                .get(offset..offset + 2)
                .ok_or("Tiled RGB565 address exceeds its resource (including tile padding).")?;
            let value = u16::from_be_bytes(pair.try_into().unwrap());
            // Observed fetch swizzle Z,Y,X,1 gives high-five red, middle-six green,
            // and low-five blue; endian mode 1 makes the stored word big-endian.
            let r = (value >> 11) as u8;
            let g = ((value >> 5) & 63) as u8;
            let b = (value & 31) as u8;
            let pixel = ((y * width + x) * 4) as usize;
            output[pixel..pixel + 4].copy_from_slice(&[
                (r << 3) | (r >> 2),
                (g << 2) | (g >> 4),
                (b << 3) | (b >> 2),
                255,
            ]);
        }
    }
    Ok(output)
}
fn address(x: u32, y: u32, pitch: u32) -> usize {
    let tile = (y / 32) * (pitch / 32) + x / 32;
    let mut address = (((tile & 1) as usize) << 10) | (((tile >> 1) as usize) << 12);
    for (coordinate, source, destination) in [
        (x, 0, 1),
        (x, 1, 2),
        (x, 2, 3),
        (y, 0, 4),
        (y, 1, 5),
        (x, 3, 6),
        (y, 2, 8),
        (y, 3, 9),
        (y, 4, 11),
    ] {
        address |= (((coordinate >> source) & 1) as usize) << destination;
    }
    address |= ((((x >> 4) ^ (y >> 3)) & 1) as usize) << 7;
    address
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn golden_addresses_and_padding_holes() {
        for (x, y, a) in [
            (1, 0, 2),
            (8, 0, 0x40),
            (0, 8, 0x280),
            (16, 8, 0x200),
            (31, 31, 0xb7e),
            (32, 0, 0x400),
            (0, 32, 0x1000),
            (32, 32, 0x1400),
            (47, 63, 0x1ffe),
        ] {
            assert_eq!(address(x, y, 64), a);
        }
        for (width, height, extent) in [(32, 32, 3072), (64, 32, 4096), (64, 64, 8192)] {
            let mut unique = std::collections::HashSet::new();
            for y in 0..height {
                for x in 0..width {
                    assert!(unique.insert(address(x, y, width)));
                }
            }
            assert_eq!(unique.len(), (width * height) as usize);
            assert_eq!(unique.into_iter().max().unwrap() + 2, extent);
        }
    }
    #[test]
    fn markers_check_word_endian_channel_order_and_holes() {
        let mut data = vec![0; 4096];
        for (offset, color) in [(2, 0xf800u16), (0x280, 0x07e0), (0xbfe, 0x001f)] {
            data[offset..offset + 2].copy_from_slice(&color.to_be_bytes());
        }
        let output = decode(&data, 32, 32, 32).unwrap();
        for (x, y, color) in [
            (1, 0, [255, 0, 0, 255]),
            (0, 8, [0, 255, 0, 255]),
            (15, 31, [0, 0, 255, 255]),
        ] {
            let offset = ((y * 32 + x) * 4) as usize;
            assert_eq!(&output[offset..offset + 4], &color);
        }
        assert!(decode(&data[..2048], 32, 32, 32).is_err());
        assert!(decode(&data, 16, 16, 32).is_err());
    }
}
