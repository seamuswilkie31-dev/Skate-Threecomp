//! Original decoder for the five-byte 10 FB RefPack header profile.

/// Decode one complete 10 FB stream with a caller-supplied output budget.
/// Extended headers and trailing bytes are rejected for this initial profile.
pub fn decode_refpack(input: &[u8], budget: usize) -> Result<Vec<u8>, String> {
    if input.len() < 5 || input[..2] != [0x10, 0xfb] {
        return Err("Unsupported or truncated RefPack header.".into());
    }
    let expected =
        (usize::from(input[2]) << 16) | (usize::from(input[3]) << 8) | usize::from(input[4]);
    if expected > budget.min(64 * 1024 * 1024) {
        return Err("RefPack decoded size exceeds the output budget.".into());
    }
    let mut cursor = 5;
    let mut output = Vec::with_capacity(expected);
    loop {
        let opcode = take(input, &mut cursor, 1)?[0];
        let code = usize::from(opcode);
        let (literals, length, distance) = match opcode {
            0x00..=0x7f => {
                let next = usize::from(take(input, &mut cursor, 1)?[0]);
                (
                    code & 3,
                    ((code >> 2) & 7) + 3,
                    ((code & 0x60) << 3) + next + 1,
                )
            }
            0x80..=0xbf => {
                let bytes = take(input, &mut cursor, 2)?;
                (
                    (usize::from(bytes[0]) >> 6) & 3,
                    (code & 0x3f) + 4,
                    ((usize::from(bytes[0]) & 0x3f) << 8) + usize::from(bytes[1]) + 1,
                )
            }
            0xc0..=0xdf => {
                let bytes = take(input, &mut cursor, 3)?;
                (
                    code & 3,
                    ((code & 12) << 6) + usize::from(bytes[2]) + 5,
                    ((code & 16) << 12) + (usize::from(bytes[0]) << 8) + usize::from(bytes[1]) + 1,
                )
            }
            0xe0..=0xfb => (((code & 31) << 2) + 4, 0, 0),
            0xfc..=0xff => (code & 3, 0, 0),
        };
        if literals + length > expected - output.len() {
            return Err("RefPack command exceeds its declared decoded size.".into());
        }
        output.extend_from_slice(take(input, &mut cursor, literals)?);
        if length != 0 {
            if distance > output.len() {
                return Err("RefPack back-reference precedes the decoded output.".into());
            }
            // Bytewise copying deliberately supports overlapping references.
            for _ in 0..length {
                let byte = output[output.len() - distance];
                output.push(byte);
            }
        }
        if opcode >= 0xfc {
            if output.len() != expected || cursor != input.len() {
                return Err("RefPack final size or input consumption does not match.".into());
            }
            return Ok(output);
        }
    }
}

fn take<'a>(input: &'a [u8], cursor: &mut usize, size: usize) -> Result<&'a [u8], String> {
    let end = cursor
        .checked_add(size)
        .ok_or("RefPack input offset overflow.")?;
    let bytes = input
        .get(*cursor..end)
        .ok_or("Truncated RefPack command or literals.")?;
    *cursor = end;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stream(size: usize, commands: &[u8]) -> Vec<u8> {
        let mut bytes = vec![
            0x10,
            0xfb,
            (size >> 16) as u8,
            (size >> 8) as u8,
            size as u8,
        ];
        bytes.extend_from_slice(commands);
        bytes
    }
    #[test]
    fn literal_and_final_runs() {
        assert_eq!(
            decode_refpack(&stream(7, b"\xE0TEST\xFFend"), 7).unwrap(),
            b"TESTend"
        );
        assert_eq!(decode_refpack(&stream(0, &[0xfc]), 0).unwrap(), b"");
    }
    #[test]
    fn all_match_encodings_support_overlap() {
        for (command, count) in [
            (&[0x05, 0, b'A'][..], 5),
            (&[0x81, 0x40, 0, b'A'][..], 6),
            (&[0xc1, 0, 0, 0, b'A'][..], 6),
        ] {
            let mut commands = command.to_vec();
            commands.push(0xfc);
            assert_eq!(
                decode_refpack(&stream(count, &commands), 100).unwrap(),
                vec![b'A'; count as usize]
            );
        }
    }

    #[test]
    fn decodes_high_distance_and_length_bits_against_known_bytes() {
        let seed: Vec<u8> = (0..131072).map(|index| (index % 251) as u8).collect();
        let mut literals = Vec::new();
        for chunk in seed.chunks(112) {
            literals.push(0xe0 | ((chunk.len() / 4 - 1) as u8));
            literals.extend_from_slice(chunk);
        }
        for (command, distance, length) in [
            (&[0x7c, 0xff][..], 1024usize, 10usize),
            (&[0xbf, 0x3f, 0xff][..], 16384, 67),
            (&[0xdc, 0xff, 0xff, 0xff][..], 131072, 1028),
        ] {
            let mut commands = literals.clone();
            commands.extend_from_slice(command);
            commands.push(0xfc);
            let mut expected = seed.clone();
            expected
                .extend_from_slice(&seed[seed.len() - distance..seed.len() - distance + length]);
            assert_eq!(
                decode_refpack(&stream(expected.len(), &commands), expected.len()).unwrap(),
                expected
            );
        }
    }
    #[test]
    fn rejects_bad_distance_truncation_budget_overrun_and_trailing_bytes() {
        for bytes in [
            stream(3, &[0, 0, 0xfc]),
            stream(4, b"\xE0TES"),
            stream(2, b"\xE0TEST\xFC"),
            stream(1, &[0xfc]),
            stream(0, &[0xfc, 0]),
        ] {
            assert!(decode_refpack(&bytes, 100).is_err());
        }
        assert!(decode_refpack(&stream(4, b"\xE0TEST\xFC"), 3).is_err());
    }
}
