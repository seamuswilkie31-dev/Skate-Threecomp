//! Original index reader for the observed Xbox 360 RW4 arena profile.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub offset_field: u32,
    pub resolved_offset: usize,
    pub size: u32,
    pub alignment: u32,
    pub type_index: u32,
    pub type_id: u32,
    pub resource_slot: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arena {
    pub asset_id: u32,
    pub used_object_count: u32,
    pub dictionary_offset: u32,
    pub resource_sizes: [u32; 5],
    pub objects: Vec<Object>,
}

impl Arena {
    /// Input is the concatenation of the two decoded SFIL block segments.
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 0xac || bytes.len() > 64 * 1024 * 1024 {
            return Err("RW4 arena size is outside the observed profile.".into());
        }
        if &bytes[..12] != b"\x89RW4xb2\0\r\n\x1a\n"
            || bytes[12..16] != [1, 32, 4, 0]
            || &bytes[16..24] != b"454\x00000\x00"
        {
            return Err("Unsupported RW4 platform, byte order, pointer width, or version.".into());
        }
        let count = word(bytes, 0x20) as usize;
        let used = word(bytes, 0x24);
        if count > 100_000 || used as usize > count {
            return Err("RW4 object count exceeds its bounds or inspection budget.".into());
        }
        let resource_sizes = std::array::from_fn(|slot| word(bytes, 0x44 + slot * 8));
        let total: u64 = resource_sizes.iter().map(|&size| u64::from(size)).sum();
        if total != bytes.len() as u64 {
            return Err("RW4 resource lengths do not match the decoded arena.".into());
        }
        for slot in 0..5 {
            if !word(bytes, 0x48 + slot * 8).is_power_of_two() {
                return Err("RW4 resource alignment is invalid.".into());
            }
        }
        let metadata_size = resource_sizes[0] as usize;
        let table = word(bytes, 0x30) as usize;
        if table < 0xac
            || table
                .checked_add(count * 24)
                .is_none_or(|end| end > metadata_size)
        {
            return Err("RW4 object dictionary extends outside metadata.".into());
        }
        let mut objects = Vec::with_capacity(count);
        for index in 0..count {
            let entry = table + index * 24;
            let offset = word(bytes, entry);
            let size = word(bytes, entry + 8);
            let alignment = word(bytes, entry + 12);
            let type_id = word(bytes, entry + 20);
            if word(bytes, entry + 4) != 0
                || !alignment.is_power_of_two()
                || offset % alignment != 0
            {
                return Err(
                    "RW4 dictionary record has unsupported offset or alignment fields.".into(),
                );
            }
            let slot = if (0x10030..=0x10033).contains(&type_id) {
                (type_id - 0x10030) as usize + 1
            } else {
                0
            };
            if u64::from(offset) + u64::from(size) > u64::from(resource_sizes[slot]) {
                return Err("RW4 object points outside its resource region.".into());
            }
            let resource_base: usize = resource_sizes[..slot]
                .iter()
                .map(|&size| size as usize)
                .sum();
            objects.push(Object {
                offset_field: offset,
                resolved_offset: resource_base + offset as usize,
                size,
                alignment,
                type_index: word(bytes, entry + 16),
                type_id,
                resource_slot: slot,
            });
        }
        Ok(Self {
            asset_id: word(bytes, 0x1c),
            used_object_count: used,
            dictionary_offset: table as u32,
            resource_sizes,
            objects,
        })
    }
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 260];
        bytes[..24].copy_from_slice(b"\x89RW4xb2\0\r\n\x1a\n\x01\x20\x04\x00454\x00000\x00");
        for (offset, value) in [
            (0x20, 2u32),
            (0x24, 2),
            (0x30, 0xb0),
            (0x44, 256),
            (0x48, 16),
            (0x50, 1),
            (0x54, 4),
            (0x58, 4),
            (0x60, 1),
            (0x68, 1),
            (0xb0, 240),
            (0xb8, 4),
            (0xbc, 16),
            (0xc4, 0x20005),
            (0xd0, 4),
            (0xd4, 4),
            (0xdc, 0x10031),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }
        bytes
    }
    #[test]
    fn resolves_metadata_and_separate_resource_offsets() {
        let arena = Arena::read(&fixture()).unwrap();
        assert_eq!(arena.objects[0].resolved_offset, 240);
        assert_eq!(arena.objects[1].resolved_offset, 256);
        assert_eq!(arena.objects[1].resource_slot, 2);
    }
    #[test]
    fn rejects_dictionary_resource_and_alignment_errors() {
        for (offset, value) in [
            (0x30, u32::MAX),
            (0x44, 400),
            (0x20, u32::MAX),
            (0xbc, 3),
            (0xd0, 8),
        ] {
            let mut bytes = fixture();
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            assert!(Arena::read(&bytes).is_err());
        }
        assert!(Arena::read(&fixture()[..128]).is_err());
    }
}
