//! Original, bounded readers for the archive indexes observed on the retail disc.
//! Indexing does not decompress or interpret member payloads.

use crate::inventory_disc;
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const MAX_INDEX: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub path: String,
    /// Byte offset relative to the containing archive.
    pub offset: u64,
    /// BIG4: stored length. EB3: allocation span through the next distinct offset,
    /// including any padding. This is not an EB3 decompressed length.
    pub available_bytes: u64,
    pub size_field: u32,
    pub compressed_size_field: Option<u32>,
    pub name_hash: Option<u32>,
    pub index_flags: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveIndex {
    pub format: &'static str,
    pub declared_size: u64,
    pub entries: Vec<ArchiveEntry>,
}

/// Locate an archive by its disc path, then inspect its index without extraction.
pub fn inspect_disc_archive(image: &Path, archive_path: &str) -> Result<ArchiveIndex, String> {
    let inventory = inventory_disc(image)?;
    let entry = inventory
        .entries
        .iter()
        .find(|entry| !entry.is_directory && entry.path.eq_ignore_ascii_case(archive_path))
        .ok_or_else(|| format!("Archive not found on disc: {archive_path}"))?;
    let mut file = File::open(image).map_err(|error| error.to_string())?;
    ArchiveIndex::read(&mut file, entry.image_offset, u64::from(entry.size))
}

impl ArchiveIndex {
    /// Read a small raw prefix for format identification. EB3 spans can include
    /// padding; these bytes are neither decompressed nor interpreted as assets.
    pub fn read_member_prefix(
        &self,
        reader: &mut (impl Read + Seek),
        base: u64,
        member: usize,
        limit: usize,
    ) -> Result<Vec<u8>, String> {
        if limit > 64 * 1024 {
            return Err("Member preview exceeds the 64 KiB limit.".into());
        }
        let entry = self
            .entries
            .get(member)
            .ok_or("Member index is out of range.")?;
        let size = entry.available_bytes.min(limit as u64);
        if entry
            .offset
            .checked_add(size)
            .is_none_or(|end| end > self.declared_size)
        {
            return Err("Member preview exceeds its archive.".into());
        }
        let mut region = Region {
            reader,
            base,
            length: self.declared_size,
        };
        region.read(entry.offset, size)
    }

    /// Read an archive contained in a bounded region of a seekable source.
    pub fn read(reader: &mut (impl Read + Seek), base: u64, length: u64) -> Result<Self, String> {
        let mut region = Region {
            reader,
            base,
            length,
        };
        // Check physical bounds before trusting any index fields.
        let end = base.checked_add(length).ok_or("Archive region overflow.")?;
        if end
            > region
                .reader
                .seek(SeekFrom::End(0))
                .map_err(|error| error.to_string())?
        {
            return Err("Archive region exceeds its source.".into());
        }
        let header = region.read(0, 16)?;
        match &header[..4] {
            b"BIG4" => read_big4(&mut region, &header),
            b"EB\0\x03" => read_eb3(&mut region),
            _ => Err("Unsupported archive signature (expected BIG4 or EB version 3).".into()),
        }
    }
}

struct Region<'a, R> {
    reader: &'a mut R,
    base: u64,
    length: u64,
}

impl<R: Read + Seek> Region<'_, R> {
    fn read(&mut self, offset: u64, size: u64) -> Result<Vec<u8>, String> {
        if size > MAX_INDEX || offset.checked_add(size).is_none_or(|end| end > self.length) {
            return Err("Archive index read exceeds its region or memory limit.".into());
        }
        let position = self
            .base
            .checked_add(offset)
            .ok_or("Archive offset overflow.")?;
        let mut bytes = vec![0; size as usize];
        self.reader
            .seek(SeekFrom::Start(position))
            .and_then(|_| self.reader.read_exact(&mut bytes))
            .map_err(|error| format!("Could not read archive index: {error}"))?;
        Ok(bytes)
    }
}

fn read_big4(
    region: &mut Region<'_, impl Read + Seek>,
    header: &[u8],
) -> Result<ArchiveIndex, String> {
    let declared_size = u64::from(u32::from_le_bytes(header[4..8].try_into().unwrap()));
    let count = be32(&header[8..12]) as usize;
    let index_end = u64::from(be32(&header[12..16]));
    check_count(count)?;
    if declared_size != region.length || index_end < 16 || index_end > declared_size {
        return Err("BIG4 size or index bounds are invalid for the observed profile.".into());
    }
    let bytes = region.read(16, index_end - 16)?;
    let mut cursor = 0usize;
    let mut entries = Vec::with_capacity(count);
    let mut paths = HashSet::new();
    for _ in 0..count {
        let record = bytes
            .get(cursor..cursor + 8)
            .ok_or("Truncated BIG4 index record.")?;
        let offset = u64::from(be32(&record[..4]));
        let size = be32(&record[4..8]);
        cursor += 8;
        let remaining = bytes.get(cursor..).ok_or("Truncated BIG4 filename.")?;
        let name_end = remaining
            .iter()
            .position(|&byte| byte == 0)
            .ok_or("Unterminated BIG4 filename.")?;
        let path = member_path(&cstring(&remaining[..=name_end])?)?;
        cursor += name_end + 1;
        if !paths.insert(path.to_ascii_lowercase())
            || offset < index_end
            || offset + u64::from(size) > declared_size
        {
            return Err("BIG4 member has duplicate path or invalid extent.".into());
        }
        entries.push(ArchiveEntry {
            path,
            offset,
            available_bytes: u64::from(size),
            size_field: size,
            compressed_size_field: None,
            name_hash: None,
            index_flags: None,
        });
    }
    Ok(ArchiveIndex {
        format: "BIG4",
        declared_size,
        entries,
    })
}

fn read_eb3(region: &mut Region<'_, impl Read + Seek>) -> Result<ArchiveIndex, String> {
    let header = region.read(0, 48)?;
    let count = be32(&header[4..8]) as usize;
    check_count(count)?;
    let alignment = header[10];
    let base_header_size = u64::from(be32(&header[12..16]));
    let name_header_size = u64::from(be32(&header[16..20]));
    let path_size = usize::from(header[20]);
    let directory_size = usize::from(header[21]);
    let directory_count = usize::from(u16::from_be_bytes(header[22..24].try_into().unwrap()));
    let declared_size = u64::from_be_bytes(header[24..32].try_into().unwrap());
    let names_end = base_header_size
        .checked_add(name_header_size)
        .ok_or("EB3 names size overflow.")?;
    let hash_end = 48 + count as u64 * 16;
    let expected_base = align16(hash_end + count as u64);
    let directory_start = align16(base_header_size + count as u64 * path_size as u64);
    let expected_end = align16(directory_start + directory_count as u64 * directory_size as u64);
    if declared_size != region.length
        || alignment > 16
        || path_size < 3
        || directory_size == 0
        || base_header_size != expected_base
        || names_end != expected_end
        || names_end > declared_size
        || names_end > MAX_INDEX
    {
        return Err("EB3 header or name-table layout is outside the observed profile.".into());
    }
    let metadata = region.read(0, names_end)?;
    let mut entries = Vec::with_capacity(count);
    let mut paths = HashSet::new();
    for index in 0..count {
        let record = &metadata[48 + index * 16..48 + (index + 1) * 16];
        let offset = u64::from(be32(&record[..4])) << alignment;
        let path_offset = base_header_size as usize + index * path_size;
        let path_record = &metadata[path_offset..path_offset + path_size];
        let directory_index = usize::from(u16::from_be_bytes(path_record[..2].try_into().unwrap()));
        if directory_index >= directory_count {
            return Err("EB3 directory index is out of range.".into());
        }
        let directory_offset = directory_start as usize + directory_index * directory_size;
        let directory = cstring(&metadata[directory_offset..directory_offset + directory_size])?;
        let name = cstring(&path_record[2..])?;
        if name.is_empty() {
            return Err("EB3 member filename is empty.".into());
        }
        let path = member_path(&format!("{directory}/{name}"))?;
        if !paths.insert(path.to_ascii_lowercase()) || offset < names_end || offset > declared_size
        {
            return Err("EB3 member has duplicate path or invalid offset.".into());
        }
        entries.push(ArchiveEntry {
            path,
            offset,
            available_bytes: 0,
            size_field: be32(&record[8..12]),
            compressed_size_field: Some(be32(&record[4..8])),
            name_hash: Some(be32(&record[12..16])),
            index_flags: Some(metadata[hash_end as usize + index]),
        });
    }
    let mut offsets: Vec<_> = entries.iter().map(|entry| entry.offset).collect();
    offsets.push(declared_size);
    offsets.sort_unstable();
    offsets.dedup();
    for entry in &mut entries {
        let next = offsets.partition_point(|&offset| offset <= entry.offset);
        entry.available_bytes = offsets.get(next).copied().unwrap_or(declared_size) - entry.offset;
    }
    Ok(ArchiveIndex {
        format: "EB3",
        declared_size,
        entries,
    })
}

fn check_count(count: usize) -> Result<(), String> {
    if count > MAX_ENTRIES {
        return Err("Archive entry count exceeds the inspection limit.".into());
    }
    Ok(())
}
fn align16(value: u64) -> u64 {
    (value + 15) & !15
}
fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().unwrap())
}
fn cstring(bytes: &[u8]) -> Result<String, String> {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .ok_or("Archive name is not terminated.")?;
    if end > 4096 {
        return Err("Archive name is too long.".into());
    }
    std::str::from_utf8(&bytes[..end])
        .map(str::to_owned)
        .map_err(|_| "Archive name is not UTF-8.".into())
}
fn member_path(value: &str) -> Result<String, String> {
    let value = value.replace('\\', "/");
    if value.starts_with('/') || value.contains(':') || value.chars().any(char::is_control) {
        return Err("Archive member path is invalid.".into());
    }
    let mut parts = Vec::new();
    for part in value.split('/') {
        if part == ".." {
            return Err("Archive member path traverses its root.".into());
        }
        if !part.is_empty() && part != "." {
            parts.push(part);
        }
    }
    let result = parts.join("/");
    if result.is_empty() || result.len() > 4096 {
        return Err("Archive member path is empty or too long.".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn eb3() -> Vec<u8> {
        let mut bytes = vec![0; 128];
        bytes[..4].copy_from_slice(b"EB\0\x03");
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        bytes[8..10].copy_from_slice(&16u16.to_be_bytes());
        bytes[10] = 4;
        bytes[12..16].copy_from_slice(&80u32.to_be_bytes());
        bytes[16..20].copy_from_slice(&32u32.to_be_bytes());
        bytes[20] = 8;
        bytes[21] = 2;
        bytes[22..24].copy_from_slice(&1u16.to_be_bytes());
        bytes[24..32].copy_from_slice(&128u64.to_be_bytes());
        bytes[48..52].copy_from_slice(&7u32.to_be_bytes());
        bytes[56..60].copy_from_slice(&4u32.to_be_bytes());
        bytes[60..64].copy_from_slice(&123u32.to_be_bytes());
        bytes[64] = 9;
        bytes[82..88].copy_from_slice(b"a.bin\0");
        bytes[96..98].copy_from_slice(b".\0");
        bytes[112..116].copy_from_slice(b"TEST");
        bytes
    }

    fn big4() -> Vec<u8> {
        let mut bytes = vec![0; 36];
        bytes[..4].copy_from_slice(b"BIG4");
        bytes[4..8].copy_from_slice(&36u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&1u32.to_be_bytes());
        bytes[12..16].copy_from_slice(&32u32.to_be_bytes());
        bytes[16..20].copy_from_slice(&32u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&4u32.to_be_bytes());
        bytes[24..30].copy_from_slice(b"a.bin\0");
        bytes[32..36].copy_from_slice(b"TEST");
        bytes
    }

    #[test]
    fn indexes_both_layouts_with_nonzero_container_base_and_reads_bounded_prefix() {
        for bytes in [eb3(), big4()] {
            let length = bytes.len() as u64;
            let mut container = vec![0; 19];
            container.extend_from_slice(&bytes);
            container.extend_from_slice(b"NEIGHBOR");
            let mut reader = Cursor::new(container);
            let index = ArchiveIndex::read(&mut reader, 19, length).unwrap();
            assert_eq!(index.entries[0].path, "a.bin");
            assert_eq!(index.entries[0].size_field, 4);
            assert_eq!(
                index.read_member_prefix(&mut reader, 19, 0, 4).unwrap(),
                b"TEST"
            );
            assert!(index.read_member_prefix(&mut reader, 19, 0, 65537).is_err());
            let prefix = index.read_member_prefix(&mut reader, 19, 0, 65536).unwrap();
            assert_eq!(prefix.len(), if index.format == "EB3" { 16 } else { 4 });
        }
    }

    #[test]
    fn preserves_eb3_fields_without_treating_allocation_as_stored_length() {
        let mut reader = Cursor::new(eb3());
        let index = ArchiveIndex::read(&mut reader, 0, 128).unwrap();
        assert_eq!(index.entries[0].available_bytes, 16);
        assert_eq!(index.entries[0].name_hash, Some(123));
        assert_eq!(index.entries[0].index_flags, Some(9));
        assert_eq!(index.entries[0].compressed_size_field, Some(0));
    }

    #[test]
    fn rejects_eb3_bad_directory_index_name_and_payload_offset() {
        let mut bad_directory = eb3();
        bad_directory[80..82].copy_from_slice(&1u16.to_be_bytes());
        let mut bad_name = eb3();
        bad_name[82..88].fill(b'x');
        let mut bad_offset = eb3();
        bad_offset[48..52].fill(0);
        let mut bad_tables = eb3();
        bad_tables[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        for bytes in [bad_directory, bad_name, bad_offset, bad_tables] {
            assert!(ArchiveIndex::read(&mut Cursor::new(bytes), 0, 128).is_err());
        }
    }

    #[test]
    fn rejects_big4_extent_and_filename_outside_index() {
        let mut bad_extent = big4();
        bad_extent[20..24].copy_from_slice(&100u32.to_be_bytes());
        let mut bad_name = big4();
        bad_name[24..32].fill(b'x');
        for bytes in [bad_extent, bad_name] {
            assert!(ArchiveIndex::read(&mut Cursor::new(bytes), 0, 36).is_err());
        }
    }

    #[test]
    fn rejects_physical_bounds_excessive_count_and_unsafe_paths() {
        assert!(ArchiveIndex::read(&mut Cursor::new(eb3()), u64::MAX, 128).is_err());
        assert!(ArchiveIndex::read(&mut Cursor::new(eb3()), 1, 128).is_err());
        let mut bytes = eb3();
        bytes[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(ArchiveIndex::read(&mut Cursor::new(bytes), 0, 128).is_err());
        for path in [
            "../file",
            "C:/file",
            "/absolute",
            "dir/../../file",
            "bad\nname",
        ] {
            assert!(member_path(path).is_err());
        }
    }
}
