use std::{
    env,
    fs::File,
    io::{self, Write},
    path::PathBuf,
};

use threecomp_engine_core::{ArchiveIndex, inspect_disc_archive, inspect_source, inventory_disc};

fn main() {
    if let Err(error) = run() {
        eprintln!("Inspection failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.is_empty() || args.len() > 4 {
        return Err("Usage: skate-threecomp-inspect <source> [--list|--json|--archives|--archive <disc-path>|--prefix <archive-path> <member-path>]".into());
    }
    let mode = args.get(1).map(|arg| arg.to_string_lossy().into_owned());
    if mode.as_deref().is_some_and(|mode| {
        !["--list", "--json", "--archives", "--archive", "--prefix"].contains(&mode)
    }) {
        return Err("Unknown inspection option.".into());
    }
    let expected_args = match mode.as_deref() {
        Some("--archive") => 3,
        Some("--prefix") => 4,
        Some(_) => 2,
        None => 1,
    };
    if args.len() != expected_args {
        return Err("Wrong number of arguments for the requested inspection mode.".into());
    }
    let path = PathBuf::from(&args[0]);
    let info = inspect_source(&path)?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    if mode.as_deref() == Some("--prefix") {
        let archive_path = args[2].to_str().ok_or("Archive path must be UTF-8.")?;
        let member_path = args[3].to_str().ok_or("Member path must be UTF-8.")?;
        let inventory = inventory_disc(&path)?;
        let entry = inventory
            .entries
            .iter()
            .find(|entry| !entry.is_directory && entry.path.eq_ignore_ascii_case(archive_path))
            .ok_or("Archive path not found on disc.")?;
        let mut file = File::open(&path).map_err(|error| error.to_string())?;
        let index = ArchiveIndex::read(&mut file, entry.image_offset, u64::from(entry.size))?;
        let member = index
            .entries
            .iter()
            .position(|entry| entry.path.eq_ignore_ascii_case(member_path))
            .ok_or("Member path not found in archive.")?;
        let bytes = index.read_member_prefix(&mut file, entry.image_offset, member, 32)?;
        let hex: String = bytes.iter().map(|byte| format!("{byte:02X}")).collect();
        serde_json::to_writer_pretty(
            &mut output,
            &serde_json::json!({
                "schema_version": 1, "archive_path": archive_path, "member_path": member_path,
                "format": index.format, "raw_prefix_hex": hex, "bytes_read": bytes.len(),
            }),
        )
        .map_err(|error| error.to_string())?;
        return writeln!(output)
            .and_then(|_| output.flush())
            .map_err(|error| error.to_string());
    }
    if mode.as_deref() == Some("--archive") {
        let archive_path = args[2].to_str().ok_or("Archive path must be UTF-8.")?;
        let archive = inspect_disc_archive(&path, archive_path)?;
        let entries: Vec<_> = archive.entries.iter().map(|entry| serde_json::json!({
            "path": entry.path, "offset": entry.offset, "available_bytes": entry.available_bytes,
            "size_field": entry.size_field, "compressed_size_field": entry.compressed_size_field,
            "name_hash": entry.name_hash, "index_flags": entry.index_flags,
        })).collect();
        let report = serde_json::json!({ "schema_version": 1, "archive_path": archive_path,
            "format": archive.format, "declared_size": archive.declared_size,
            "entry_count": entries.len(), "entries": entries });
        serde_json::to_writer_pretty(&mut output, &report).map_err(|error| error.to_string())?;
        return writeln!(output)
            .and_then(|_| output.flush())
            .map_err(|error| error.to_string());
    }
    if mode.as_deref() == Some("--archives") {
        let inventory = inventory_disc(&path)?;
        let mut file = File::open(&path).map_err(|error| error.to_string())?;
        let mut archives = Vec::new();
        for entry in inventory.entries.iter().filter(|entry| {
            !entry.is_directory && entry.path.to_ascii_lowercase().ends_with(".big")
        }) {
            let archive = ArchiveIndex::read(&mut file, entry.image_offset, u64::from(entry.size))
                .map_err(|error| format!("{}: {error}", entry.path))?;
            archives.push(
                serde_json::json!({"path": entry.path, "format": archive.format,
                "entry_count": archive.entries.len(), "declared_size": archive.declared_size}),
            );
        }
        serde_json::to_writer_pretty(
            &mut output,
            &serde_json::json!({"schema_version": 1, "archives": archives}),
        )
        .map_err(|error| error.to_string())?;
        return writeln!(output)
            .and_then(|_| output.flush())
            .map_err(|error| error.to_string());
    }
    let result = if let Some(mode) = mode {
        let inventory = inventory_disc(&path)?;
        if mode == "--json" {
            skate_threecomp_launcher::write_inventory_json(&mut output, &info, &inventory)?;
            Ok(())
        } else {
            writeln!(output, "{}", info.status()).map_err(|error| error.to_string())?;
            writeln!(
                output,
                "{} files, {} directories, {} file bytes",
                inventory.file_count(),
                inventory.directory_count(),
                inventory.total_file_bytes
            )
            .map_err(|error| error.to_string())?;
            writeln!(output, "kind\tsize\timage_offset\tpath")
                .map_err(|error| error.to_string())?;
            for entry in &inventory.entries {
                writeln!(
                    output,
                    "{}\t{}\t0x{:X}\t{}",
                    if entry.is_directory { "dir" } else { "file" },
                    entry.size,
                    entry.image_offset,
                    entry.path
                )
                .map_err(|error| error.to_string())?;
            }
            Ok(())
        }
    } else {
        writeln!(output, "{}\n{}", info.status(), info.summary())
    };
    result
        .and_then(|_| output.flush())
        .map_err(|error| error.to_string())
}
