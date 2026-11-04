//! Clean-room foundations for reading user-supplied Xbox 360 game media.

mod inventory;
pub use inventory::{DiscEntry, DiscInventory, inventory_disc};
mod archive;
pub use archive::{ArchiveEntry, ArchiveIndex, inspect_disc_archive};
mod refpack;
pub use refpack::decode_refpack;
mod stream;
pub use stream::{StreamBlock, StreamIndex, StreamSegment};
pub mod collision;
pub mod geometry;
pub mod rw4;
pub mod skating;
pub mod texture;

use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const XDVDFS_MAGIC: &[u8; 20] = b"MICROSOFT*XBOX*MEDIA";
const XDVDFS_DESCRIPTOR_OFFSET: u64 = 0x10000;
const XDVDFS_SECTOR_SIZE: u64 = 2048;
const XEX_EXECUTION_INFO_KEY: u32 = 0x0004_0006;
const SKATE_3_TITLE_ID: u32 = 0x4541_08E6;
const SKATE_3_RETAIL_MEDIA_ID: u32 = 0x5C08_7C2C;
const SKATE_3_RETAIL_VERSION: u32 = 3;
const SKATE_3_RETAIL_BASE_VERSION: u32 = 3;

// Some full-disc Xbox 360 images store the game partition after a video area.
// These bases cover the common raw and trimmed layouts used by disc tools.
const GAME_PARTITION_BASES: &[u64] = &[0, 0x0FD90000, 0x02080000, 0x18300000];
const MAX_ROOT_DIRECTORY_SIZE: u32 = 64 * 1024 * 1024;
const MAX_OPTIONAL_HEADER_COUNT: u32 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XexInfo {
    pub magic: String,
    pub file_size: u64,
    pub title_id: Option<u32>,
    pub version: Option<u32>,
    pub base_version: Option<u32>,
    pub media_id: Option<u32>,
}

impl XexInfo {
    fn title_name(&self) -> Option<&'static str> {
        match self.title_id? {
            SKATE_3_TITLE_ID => Some("Skate 3"),
            _ => None,
        }
    }

    fn matches_known_skate_3_retail_metadata(&self) -> bool {
        self.title_id == Some(SKATE_3_TITLE_ID)
            && self.media_id == Some(SKATE_3_RETAIL_MEDIA_ID)
            && self.version == Some(SKATE_3_RETAIL_VERSION)
            && self.base_version == Some(SKATE_3_RETAIL_BASE_VERSION)
    }

    fn summary(&self) -> String {
        let mut parts = vec![
            format!("{} executable", self.magic),
            format_bytes(self.file_size),
        ];
        if let Some(title_id) = self.title_id {
            let title = self
                .title_name()
                .map(|name| format!("{name} (title ID 0x{title_id:08X})"))
                .unwrap_or_else(|| format!("title ID 0x{title_id:08X}"));
            parts.push(title);
        } else {
            parts.push("title metadata unavailable".into());
        }
        if let Some(version) = self.version {
            parts.push(format!("version 0x{version:08X}"));
        }
        if let Some(base_version) = self.base_version {
            parts.push(format!("base 0x{base_version:08X}"));
        }
        if let Some(media_id) = self.media_id {
            parts.push(format!("media ID 0x{media_id:08X}"));
        }
        parts.join(" · ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceInfo {
    XboxExecutable(XexInfo),
    XboxDiscImage {
        file_size: u64,
        game_partition_offset: u64,
        root_directory_sector: u32,
        root_directory_size: u32,
        executable: Option<XexInfo>,
    },
}

impl SourceInfo {
    pub fn is_skate_3(&self) -> bool {
        match self {
            Self::XboxExecutable(info) => info.title_name().is_some(),
            Self::XboxDiscImage { executable, .. } => executable
                .as_ref()
                .is_some_and(|info| info.title_name().is_some()),
        }
    }

    pub fn matches_known_retail_metadata(&self) -> bool {
        match self {
            Self::XboxExecutable(info) => info.matches_known_skate_3_retail_metadata(),
            Self::XboxDiscImage { executable, .. } => executable
                .as_ref()
                .is_some_and(XexInfo::matches_known_skate_3_retail_metadata),
        }
    }

    pub fn status(&self) -> &'static str {
        match self {
            Self::XboxExecutable(info) if info.matches_known_skate_3_retail_metadata() => {
                "SKATE 3 RETAIL METADATA PROFILE MATCHED"
            }
            Self::XboxExecutable(info) if info.title_name().is_some() => {
                "SKATE 3 IDENTIFIED · UNLISTED EXECUTABLE PROFILE"
            }
            Self::XboxExecutable(_) => "XBOX EXECUTABLE RECOGNIZED · TITLE NOT IDENTIFIED",
            Self::XboxDiscImage {
                executable: Some(info),
                ..
            } if info.matches_known_skate_3_retail_metadata() => {
                "SKATE 3 RETAIL METADATA PROFILE MATCHED"
            }
            Self::XboxDiscImage {
                executable: Some(info),
                ..
            } if info.title_name().is_some() => "SKATE 3 IDENTIFIED · UNLISTED DISC PROFILE",
            Self::XboxDiscImage {
                executable: Some(_),
                ..
            } => "XBOX DISC RECOGNIZED · DIFFERENT TITLE ID",
            Self::XboxDiscImage {
                executable: None, ..
            } => "XBOX DISC RECOGNIZED · DEFAULT.XEX NOT FOUND",
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::XboxExecutable(info) => info.summary(),
            Self::XboxDiscImage {
                file_size,
                game_partition_offset,
                root_directory_sector,
                root_directory_size,
                executable,
            } => {
                let executable_summary = executable
                    .as_ref()
                    .map(|info| format!("default.xex: {}", info.summary()))
                    .unwrap_or_else(|| "default.xex was not found in the root table".into());
                format!(
                    "Xbox disc image, {}, game data at 0x{game_partition_offset:X}, root table: sector {root_directory_sector} ({}); {executable_summary}",
                    format_bytes(*file_size),
                    format_bytes(u64::from(*root_directory_size))
                )
            }
        }
    }
}

#[derive(Debug)]
struct DirectoryEntry {
    name: String,
    start_sector: u32,
    size: u32,
    is_directory: bool,
}

/// Read only enough source metadata to identify a supported Xbox media header.
/// No game files are extracted or modified.
pub fn inspect_source(path: &Path) -> Result<SourceInfo, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut file = File::open(path).map_err(|error| format!("Could not open file: {error}"))?;
    let file_size = file
        .metadata()
        .map_err(|error| format!("Could not read file details: {error}"))?
        .len();

    match extension.as_str() {
        "xex" => Ok(SourceInfo::XboxExecutable(inspect_xex_at(
            &mut file, 0, file_size,
        )?)),
        "iso" => inspect_iso(&mut file, file_size),
        _ => Err("Choose an .iso or .xex file.".into()),
    }
}

fn inspect_xex_at(file: &mut File, offset: u64, file_size: u64) -> Result<XexInfo, String> {
    if file_size < 24 {
        return Err("File is too small to contain an Xbox executable header.".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("Could not read executable header: {error}"))?;
    let mut header = [0; 24];
    file.read_exact(&mut header)
        .map_err(|error| format!("Could not read executable header: {error}"))?;
    if &header[0..4] != b"XEX2" {
        return Err("Xbox executable signature not found (expected XEX2).".into());
    }

    let header_size = read_be_u32(&header[8..12]);
    let optional_header_count = read_be_u32(&header[20..24]);
    if optional_header_count > MAX_OPTIONAL_HEADER_COUNT {
        return Err("Xbox executable declares too many optional headers.".into());
    }
    let optional_headers_size = u64::from(optional_header_count)
        .checked_mul(8)
        .ok_or_else(|| "Executable optional-header table size overflow.".to_string())?;
    if header_size < 24
        || u64::from(header_size) > file_size
        || 24 + optional_headers_size > u64::from(header_size)
    {
        return Err("Xbox executable header has invalid bounds.".into());
    }

    let mut optional_headers = vec![0; optional_headers_size as usize];
    file.read_exact(&mut optional_headers)
        .map_err(|error| format!("Could not read executable metadata: {error}"))?;
    let mut info = XexInfo {
        magic: String::from_utf8_lossy(&header[0..4]).into_owned(),
        file_size,
        title_id: None,
        version: None,
        base_version: None,
        media_id: None,
    };

    for record in optional_headers.chunks_exact(8) {
        if read_be_u32(&record[0..4]) != XEX_EXECUTION_INFO_KEY {
            continue;
        }
        let execution_info_offset = u64::from(read_be_u32(&record[4..8]));
        let execution_info_end = execution_info_offset
            .checked_add(24)
            .ok_or_else(|| "Executable metadata offset overflow.".to_string())?;
        if execution_info_end > u64::from(header_size) || execution_info_end > file_size {
            return Err("Xbox executable metadata points outside its header.".into());
        }
        file.seek(SeekFrom::Start(offset + execution_info_offset))
            .map_err(|error| format!("Could not read executable metadata: {error}"))?;
        let mut execution_info = [0; 24];
        file.read_exact(&mut execution_info)
            .map_err(|error| format!("Could not read executable metadata: {error}"))?;
        info.media_id = Some(read_be_u32(&execution_info[0..4]));
        info.version = Some(read_be_u32(&execution_info[4..8]));
        info.base_version = Some(read_be_u32(&execution_info[8..12]));
        info.title_id = Some(read_be_u32(&execution_info[12..16]));
        break;
    }

    Ok(info)
}

fn inspect_iso(file: &mut File, file_size: u64) -> Result<SourceInfo, String> {
    for &partition_base in GAME_PARTITION_BASES {
        let descriptor_offset = partition_base
            .checked_add(XDVDFS_DESCRIPTOR_OFFSET)
            .ok_or_else(|| "Disc image offset overflow.".to_string())?;
        if descriptor_offset + XDVDFS_MAGIC.len() as u64 > file_size {
            continue;
        }

        file.seek(SeekFrom::Start(descriptor_offset))
            .map_err(|error| format!("Could not inspect disc image: {error}"))?;
        let mut magic = [0; 20];
        file.read_exact(&mut magic)
            .map_err(|error| format!("Could not inspect disc image: {error}"))?;
        if &magic != XDVDFS_MAGIC {
            continue;
        }

        file.seek(SeekFrom::Start(descriptor_offset + 0x7EC))
            .map_err(|error| format!("Could not verify disc descriptor: {error}"))?;
        let mut closing_magic = [0; 20];
        file.read_exact(&mut closing_magic)
            .map_err(|error| format!("Could not verify disc descriptor: {error}"))?;
        if &closing_magic != XDVDFS_MAGIC {
            return Err(
                "Xbox disc signature is present, but its volume descriptor is incomplete.".into(),
            );
        }

        file.seek(SeekFrom::Start(descriptor_offset + 20))
            .map_err(|error| format!("Could not read disc directory details: {error}"))?;
        let mut root_fields = [0; 8];
        file.read_exact(&mut root_fields)
            .map_err(|error| format!("Could not read disc directory details: {error}"))?;
        let root_directory_sector = u32::from_le_bytes(root_fields[0..4].try_into().unwrap());
        let root_directory_size = u32::from_le_bytes(root_fields[4..8].try_into().unwrap());
        if root_directory_sector == 0 || root_directory_size == 0 {
            return Err("Xbox disc signature is present, but its root directory is empty.".into());
        }
        if root_directory_size > MAX_ROOT_DIRECTORY_SIZE {
            return Err("Disc directory table exceeds the inspection size limit.".into());
        }

        let root_directory_offset = partition_base
            .checked_add(u64::from(root_directory_sector) * XDVDFS_SECTOR_SIZE)
            .ok_or_else(|| "Disc directory offset overflow.".to_string())?;
        let root_end = root_directory_offset
            .checked_add(u64::from(root_directory_size))
            .ok_or_else(|| "Disc directory size overflow.".to_string())?;
        if root_end > file_size {
            return Err("Disc root directory points outside the selected image.".into());
        }

        let mut root_table = vec![0; root_directory_size as usize];
        file.seek(SeekFrom::Start(root_directory_offset))
            .map_err(|error| format!("Could not read disc root directory: {error}"))?;
        file.read_exact(&mut root_table)
            .map_err(|error| format!("Could not read disc root directory: {error}"))?;
        let executable = find_directory_entry(&root_table, "default.xex")?
            .map(|entry| {
                if entry.is_directory {
                    return Err("The root entry named default.xex is a directory.".into());
                }
                let xex_offset = partition_base
                    .checked_add(u64::from(entry.start_sector) * XDVDFS_SECTOR_SIZE)
                    .ok_or_else(|| "Executable offset overflow.".to_string())?;
                let xex_end = xex_offset
                    .checked_add(u64::from(entry.size))
                    .ok_or_else(|| "Executable size overflow.".to_string())?;
                if xex_end > file_size {
                    return Err("default.xex points outside the selected disc image.".into());
                }
                inspect_xex_at(file, xex_offset, u64::from(entry.size))
            })
            .transpose()?;

        return Ok(SourceInfo::XboxDiscImage {
            file_size,
            game_partition_offset: partition_base,
            root_directory_sector,
            root_directory_size,
            executable,
        });
    }

    Err("Xbox disc filesystem signature not found in the supported image layouts.".into())
}

fn find_directory_entry(table: &[u8], target: &str) -> Result<Option<DirectoryEntry>, String> {
    Ok(read_directory_entries(table)?
        .into_iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(target)))
}

fn read_directory_entries(table: &[u8]) -> Result<Vec<DirectoryEntry>, String> {
    if table.is_empty() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    let mut pending = vec![0usize];
    let mut visited = HashSet::new();

    while let Some(offset) = pending.pop() {
        if !visited.insert(offset) {
            return Err("Disc directory tree contains a cycle or shared node.".into());
        }
        let header_end = offset
            .checked_add(14)
            .ok_or_else(|| "Directory entry offset overflow.".to_string())?;
        if header_end > table.len() {
            return Err("Disc directory tree points outside its table.".into());
        }

        let left = u16::from_le_bytes(table[offset..offset + 2].try_into().unwrap());
        let right = u16::from_le_bytes(table[offset + 2..offset + 4].try_into().unwrap());
        let start_sector = u32::from_le_bytes(table[offset + 4..offset + 8].try_into().unwrap());
        let size = u32::from_le_bytes(table[offset + 8..offset + 12].try_into().unwrap());
        let attributes = table[offset + 12];
        let name_size = usize::from(table[offset + 13]);
        let name_end = header_end
            .checked_add(name_size)
            .ok_or_else(|| "Directory filename size overflow.".to_string())?;
        if name_end > table.len() {
            return Err("Disc directory filename extends past its table.".into());
        }
        let name = String::from_utf8_lossy(&table[header_end..name_end]).into_owned();
        if entries.len() >= 100_000 {
            return Err("Disc directory contains too many entries to inspect.".into());
        }
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains(['/', '\\', '\0'])
            || name.chars().any(char::is_control)
        {
            return Err("Disc directory contains an invalid filename.".into());
        }
        entries.push(DirectoryEntry {
            name,
            start_sector,
            size,
            is_directory: attributes & 0x10 != 0,
        });

        for pointer in [left, right] {
            if pointer == 0 || pointer == u16::MAX {
                continue;
            }
            let child_offset = usize::from(pointer) * 4;
            if child_offset >= table.len() {
                return Err("Disc directory child pointer is outside its table.".into());
            }
            pending.push(child_offset);
        }
    }

    Ok(entries)
}

fn read_be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().unwrap())
}

fn format_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const KIB: f64 = 1024.0;
    if bytes >= GIB as u64 {
        format!("{:.2} GiB", bytes as f64 / GIB)
    } else if bytes >= MIB as u64 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes >= KIB as u64 {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}
