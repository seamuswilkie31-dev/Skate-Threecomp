//! Clean-room foundations for reading user-supplied Xbox 360 game media.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const XDVDFS_MAGIC: &[u8; 20] = b"MICROSOFT*XBOX*MEDIA";
const XDVDFS_DESCRIPTOR_OFFSET: u64 = 0x10000;
const XDVDFS_SECTOR_SIZE: u64 = 2048;

// Some full-disc Xbox 360 images store the game partition after a video area.
// These bases cover the common raw and trimmed layouts used by disc tools.
const GAME_PARTITION_BASES: &[u64] = &[0, 0x0FD90000, 0x02080000, 0x18300000];
const MAX_ROOT_DIRECTORY_SIZE: u32 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceInfo {
    XboxExecutable {
        magic: String,
        file_size: u64,
    },
    XboxDiscImage {
        file_size: u64,
        game_partition_offset: u64,
        root_directory_sector: u32,
        root_directory_size: u32,
        root_directory_offset: u64,
    },
}

impl SourceInfo {
    pub fn summary(&self) -> String {
        match self {
            Self::XboxExecutable { magic, file_size } => {
                format!("Xbox executable ({magic}), {}", format_bytes(*file_size))
            }
            Self::XboxDiscImage {
                file_size,
                game_partition_offset,
                root_directory_sector,
                root_directory_size,
                ..
            } => format!(
                "Xbox disc image, {}, game data at 0x{game_partition_offset:X}, root table: sector {root_directory_sector} ({})",
                format_bytes(*file_size),
                format_bytes(u64::from(*root_directory_size))
            ),
        }
    }
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
        "xex" => inspect_xex(&mut file, file_size),
        "iso" => inspect_iso(&mut file, file_size),
        _ => Err("Choose an .iso or .xex file.".into()),
    }
}

fn inspect_xex(file: &mut File, file_size: u64) -> Result<SourceInfo, String> {
    if file_size < 24 {
        return Err("File is too small to contain an Xbox executable header.".into());
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("Could not read executable header: {error}"))?;
    let mut header = [0; 24];
    file.read_exact(&mut header)
        .map_err(|error| format!("Could not read executable header: {error}"))?;
    if &header[0..4] != b"XEX2" {
        return Err("Xbox executable signature not found (expected XEX2).".into());
    }
    let header_size = u32::from_le_bytes(header[8..12].try_into().unwrap());
    if header_size < 24 || u64::from(header_size) > file_size {
        return Err("Xbox executable header has an invalid size.".into());
    }

    Ok(SourceInfo::XboxExecutable {
        magic: String::from_utf8_lossy(&header[0..4]).into_owned(),
        file_size,
    })
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

        return Ok(SourceInfo::XboxDiscImage {
            file_size,
            game_partition_offset: partition_base,
            root_directory_sector,
            root_directory_size,
            root_directory_offset,
        });
    }

    Err("Xbox disc filesystem signature not found in the supported image layouts.".into())
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
