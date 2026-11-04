//! Bounded, read-only file discovery for future asset loaders.

use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use crate::{
    MAX_ROOT_DIRECTORY_SIZE, SourceInfo, XDVDFS_SECTOR_SIZE, inspect_iso, read_directory_entries,
};

const MAX_ENTRIES: usize = 100_000;
const MAX_DEPTH: usize = 64;
const MAX_TABLE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscEntry {
    /// Disc-relative path, using forward slashes. Never a host extraction path.
    pub path: String,
    pub is_directory: bool,
    pub size: u32,
    /// Absolute byte position in the selected image, including its partition base.
    pub image_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscInventory {
    pub entries: Vec<DiscEntry>,
    /// Sum of file lengths. Directory tables and disc padding are excluded.
    pub total_file_bytes: u64,
}

impl DiscInventory {
    pub fn file_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| !entry.is_directory)
            .count()
    }

    pub fn directory_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.is_directory)
            .count()
    }
}

/// Inventory a disc image without extracting files or reading asset payloads.
/// A standalone XEX does not contain the surrounding disc filesystem.
pub fn inventory_disc(path: &Path) -> Result<DiscInventory, String> {
    let mut file = File::open(path).map_err(|error| format!("Could not open image: {error}"))?;
    let file_size = file
        .metadata()
        .map_err(|error| format!("Could not read image size: {error}"))?
        .len();
    let SourceInfo::XboxDiscImage {
        game_partition_offset,
        root_directory_sector,
        root_directory_size,
        ..
    } = inspect_iso(&mut file, file_size)?
    else {
        return Err("Inventory requires an Xbox disc image.".into());
    };
    walk_disc(
        &mut file,
        file_size,
        game_partition_offset,
        root_directory_sector,
        root_directory_size,
    )
}

fn walk_disc(
    file: &mut (impl Read + Seek),
    image_size: u64,
    partition: u64,
    root_sector: u32,
    root_size: u32,
) -> Result<DiscInventory, String> {
    let mut pending = vec![(String::new(), root_sector, root_size, 0usize)];
    let mut visited = HashSet::new();
    let mut paths = HashSet::new();
    let mut entries = Vec::new();
    let mut table_bytes = 0u64;
    let mut total_file_bytes = 0u64;

    while let Some((parent, sector, size, depth)) = pending.pop() {
        if size == 0 {
            continue;
        }
        if !visited.insert(sector) {
            return Err("Disc directory graph contains a cycle or shared table.".into());
        }
        if depth > MAX_DEPTH || size > MAX_ROOT_DIRECTORY_SIZE {
            return Err("Disc directory depth or table size exceeds the inventory limit.".into());
        }
        table_bytes += u64::from(size);
        if table_bytes > MAX_TABLE_BYTES {
            return Err("Disc directory data exceeds the inventory budget.".into());
        }
        let offset = checked_extent(partition, sector, size, image_size)?;
        let mut table = vec![0; size as usize];
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.read_exact(&mut table))
            .map_err(|error| format!("Could not read directory {parent:?}: {error}"))?;
        for entry in read_directory_entries(&table)? {
            if entries.len() >= MAX_ENTRIES {
                return Err("Disc contains too many entries to inventory.".into());
            }
            let path = if parent.is_empty() {
                entry.name
            } else {
                format!("{parent}/{}", entry.name)
            };
            if path.len() > MAX_PATH_BYTES || !paths.insert(path.to_ascii_lowercase()) {
                return Err("Disc contains an excessive path length or duplicate path.".into());
            }
            let image_offset =
                checked_extent(partition, entry.start_sector, entry.size, image_size)?;
            if entry.is_directory {
                pending.push((path.clone(), entry.start_sector, entry.size, depth + 1));
            } else {
                total_file_bytes += u64::from(entry.size);
            }
            entries.push(DiscEntry {
                path,
                is_directory: entry.is_directory,
                size: entry.size,
                image_offset,
            });
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(DiscInventory {
        entries,
        total_file_bytes,
    })
}

fn checked_extent(partition: u64, sector: u32, size: u32, image_size: u64) -> Result<u64, String> {
    let offset = partition
        .checked_add(u64::from(sector) * XDVDFS_SECTOR_SIZE)
        .ok_or("Disc entry offset overflow.")?;
    let end = offset
        .checked_add(u64::from(size))
        .ok_or("Disc entry length overflow.")?;
    if end > image_size {
        return Err("Disc entry points outside the selected image.".into());
    }
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn entry(name: &str, sector: u32, size: u32, directory: bool) -> Vec<u8> {
        let mut bytes = vec![0; 14];
        bytes[4..8].copy_from_slice(&sector.to_le_bytes());
        bytes[8..12].copy_from_slice(&size.to_le_bytes());
        bytes[12] = if directory { 0x10 } else { 0x20 };
        bytes[13] = name.len() as u8;
        bytes.extend_from_slice(name.as_bytes());
        bytes
    }

    #[test]
    fn inventories_nested_file_at_partition_relative_offset() {
        let root = entry("world", 2, 24, true);
        let child = entry("map.bin", 3, 7, false);
        let mut bytes = vec![0; 9000];
        bytes[2048..2048 + root.len()].copy_from_slice(&root);
        bytes[6144..6144 + child.len()].copy_from_slice(&child);
        let result = walk_disc(&mut Cursor::new(bytes), 9000, 2048, 0, root.len() as u32).unwrap();
        assert_eq!(result.file_count(), 1);
        assert_eq!(result.directory_count(), 1);
        assert_eq!(result.total_file_bytes, 7);
        assert_eq!(result.entries[1].path, "world/map.bin");
        assert_eq!(result.entries[1].image_offset, 8192);
    }

    #[test]
    fn rejects_directory_cycle() {
        let root = entry("loop", 0, 18, true);
        assert!(
            walk_disc(&mut Cursor::new(root), 18, 0, 0, 18)
                .unwrap_err()
                .contains("cycle")
        );
    }

    #[test]
    fn rejects_file_outside_image() {
        let root = entry("bad", 10, 100, false);
        assert!(
            walk_disc(
                &mut Cursor::new(root.clone()),
                root.len() as u64,
                0,
                0,
                root.len() as u32
            )
            .unwrap_err()
            .contains("outside")
        );
    }

    #[test]
    fn rejects_directory_tree_cycle_and_path_components() {
        let mut cyclic = entry("file", 1, 0, false);
        cyclic[0..2].copy_from_slice(&1u16.to_le_bytes());
        assert!(read_directory_entries(&cyclic).is_err());
        assert!(read_directory_entries(&entry("../file", 1, 0, false)).is_err());
    }
}
