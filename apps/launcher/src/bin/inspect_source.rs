use std::{
    env,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

use threecomp_engine_core::{
    ArchiveIndex, StreamIndex, inspect_disc_archive, inspect_source, inventory_disc,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("Inspection failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.is_empty() || args.len() > 5 {
        return Err("Usage: skate-threecomp-inspect <source> [--list|--json|--archives|--streams|--mesh-sample|--texture-sample|--archive <disc-path>|--prefix <archive> <member>|--stream <archive> <member>|--decode-block <archive> <member> <block-index>|--rw4-block <archive> <member> <block-index>|--texture-block <archive> <member> <block-index>]".into());
    }
    let mode = args.get(1).map(|arg| arg.to_string_lossy().into_owned());
    if mode.as_deref().is_some_and(|mode| {
        ![
            "--list",
            "--json",
            "--archives",
            "--archive",
            "--prefix",
            "--streams",
            "--stream",
            "--decode-block",
            "--rw4-block",
            "--mesh-sample",
            "--texture-sample",
            "--texture-block",
        ]
        .contains(&mode)
    }) {
        return Err("Unknown inspection option.".into());
    }
    let expected_args = match mode.as_deref() {
        Some("--archive") => 3,
        Some("--prefix" | "--stream") => 4,
        Some("--decode-block" | "--rw4-block" | "--texture-block") => 5,
        Some(_) => 2,
        None => 1,
    };
    if args.len() != expected_args {
        return Err("Wrong number of arguments for the requested inspection mode.".into());
    }
    let path = PathBuf::from(&args[0]);
    let info = inspect_source(&path)?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    if mode.as_deref() == Some("--texture-sample") {
        let bytes = threecomp_engine_core::geometry::load_retail_block(&path, 0)?;
        let report = texture_report(&bytes)?;
        serde_json::to_writer_pretty(&mut output, &report).map_err(|error| error.to_string())?;
        return writeln!(output)
            .and_then(|_| output.flush())
            .map_err(|error| error.to_string());
    }
    if mode.as_deref() == Some("--mesh-sample") {
        let sample = threecomp_engine_core::geometry::load_retail_sample(&path)?;
        let collision = threecomp_engine_core::collision::TriangleMesh::from_sample(&sample)?;
        let placement = collision.placement().map(|p| [p.x, p.y, p.z]);
        let report = serde_json::json!({"schema_version":1,"mesh_count":sample.meshes.len(),
            "vertex_count":sample.meshes.iter().map(|mesh|mesh.positions.len()).sum::<usize>(),
            "triangle_count":sample.meshes.iter().map(|mesh|mesh.indices.len()/3).sum::<usize>(),
            "bounds_min":[sample.min.x,sample.min.y,sample.min.z],"bounds_max":[sample.max.x,sample.max.y,sample.max.z],
            "prototype_collision":true,"collision_triangle_count":collision.triangle_count(),
            "rideable_triangle_count":collision.rideable_count,"placement":placement});
        serde_json::to_writer_pretty(&mut output, &report).map_err(|error| error.to_string())?;
        return writeln!(output)
            .and_then(|_| output.flush())
            .map_err(|error| error.to_string());
    }
    if matches!(
        mode.as_deref(),
        Some("--stream" | "--decode-block" | "--rw4-block" | "--texture-block")
    ) {
        let archive_path = args[2].to_str().ok_or("Archive path must be UTF-8.")?;
        let member_path = args[3].to_str().ok_or("Member path must be UTF-8.")?;
        let (mut file, base, length) = open_stream_member(&path, archive_path, member_path)?;
        let stream = StreamIndex::read(&mut file, base, length)?;
        let report = if matches!(
            mode.as_deref(),
            Some("--decode-block" | "--rw4-block" | "--texture-block")
        ) {
            let block: usize = args[4]
                .to_str()
                .ok_or("Block index must be a number.")?
                .parse()
                .map_err(|_| "Block index must be a nonnegative integer.")?;
            let bytes = stream.decode_block(&mut file, base, block, 64 * 1024 * 1024)?;
            if mode.as_deref() == Some("--texture-block") {
                let mut report = texture_report(&bytes)?;
                report["archive_path"] = serde_json::json!(archive_path);
                report["member_path"] = serde_json::json!(member_path);
                report["block_index"] = serde_json::json!(block);
                serde_json::to_writer_pretty(&mut output, &report)
                    .map_err(|error| error.to_string())?;
                return writeln!(output)
                    .and_then(|_| output.flush())
                    .map_err(|error| error.to_string());
            }
            if mode.as_deref() == Some("--rw4-block") {
                let arena = threecomp_engine_core::rw4::Arena::read(&bytes)?;
                let mut type_counts = std::collections::BTreeMap::<String, usize>::new();
                let objects:Vec<_> = arena.objects.iter().enumerate().map(|(index,object)| {
                    let data=arena.object_data(&bytes,index)?;
                    let prefix:String=data.iter().take(16).map(|byte|format!("{byte:02X}")).collect();
                    *type_counts.entry(format!("0x{:08X}",object.type_id)).or_default()+=1;
                    Ok(serde_json::json!({
                    "offset_field": object.offset_field, "resolved_offset": object.resolved_offset,
                    "size": object.size, "alignment": object.alignment, "type_index": object.type_index,
                    "type_id": format!("0x{:08X}", object.type_id), "resource_slot": object.resource_slot,
                    "prefix_hex":prefix,
                })) }).collect::<Result<Vec<_>,String>>()?;
                let report = serde_json::json!({"schema_version": 1, "archive_path": archive_path,
                    "member_path": member_path, "block_index": block, "asset_id": format!("0x{:08X}", arena.asset_id),
                    "resource_sizes": arena.resource_sizes, "dictionary_offset": arena.dictionary_offset,
                    "used_object_count": arena.used_object_count, "objects": objects, "type_counts":type_counts});
                serde_json::to_writer_pretty(&mut output, &report)
                    .map_err(|error| error.to_string())?;
                return writeln!(output)
                    .and_then(|_| output.flush())
                    .map_err(|error| error.to_string());
            }
            let hex: String = bytes
                .iter()
                .take(32)
                .map(|byte| format!("{byte:02X}"))
                .collect();
            serde_json::json!({"schema_version": 1, "archive_path": archive_path, "member_path": member_path,
                "block_index": block, "decoded_bytes": bytes.len(), "decoded_prefix_hex": hex})
        } else {
            let blocks: Vec<_> = stream.blocks.iter().map(|block| {
                let segments: Vec<_> = block.segments.iter().map(|segment| serde_json::json!({
                    "offset": segment.offset, "decoded_size": segment.decoded_size,
                    "stored_size": segment.stored_size, "codec_flags": segment.codec_flags,
                })).collect();
                serde_json::json!({"offset": block.offset, "identifier_words": block.identifier_words,
                    "payload_size": block.payload_size, "padded_span": block.padded_span, "segments": segments})
            }).collect();
            serde_json::json!({"schema_version": 1, "archive_path": archive_path, "member_path": member_path,
                "length": length, "header_words": stream.header_words, "block_count": blocks.len(), "blocks": blocks})
        };
        serde_json::to_writer_pretty(&mut output, &report).map_err(|error| error.to_string())?;
        return writeln!(output)
            .and_then(|_| output.flush())
            .map_err(|error| error.to_string());
    }
    if mode.as_deref() == Some("--streams") {
        let inventory = inventory_disc(&path)?;
        let mut file = File::open(&path).map_err(|error| error.to_string())?;
        let mut streams = Vec::new();
        for archive in inventory.entries.iter().filter(|entry| {
            !entry.is_directory && entry.path.to_ascii_lowercase().ends_with(".big")
        }) {
            let index =
                ArchiveIndex::read(&mut file, archive.image_offset, u64::from(archive.size))?;
            for member in index
                .entries
                .iter()
                .filter(|entry| entry.path.to_ascii_lowercase().ends_with(".xsf"))
            {
                if member.compressed_size_field.unwrap_or(0) != 0
                    || u64::from(member.size_field) > member.available_bytes
                {
                    return Err(format!(
                        "Outer compression or invalid raw size in {} / {}",
                        archive.path, member.path
                    ));
                }
                let base = archive
                    .image_offset
                    .checked_add(member.offset)
                    .ok_or("Stream offset overflow.")?;
                let stream = StreamIndex::read(&mut file, base, u64::from(member.size_field))
                    .map_err(|error| format!("{} / {}: {error}", archive.path, member.path))?;
                let compressed = stream
                    .blocks
                    .iter()
                    .flat_map(|block| &block.segments)
                    .filter(|segment| segment.codec_flags == 1)
                    .count();
                streams.push(serde_json::json!({"archive_path": archive.path, "member_path": member.path,
                    "length": stream.length, "block_count": stream.blocks.len(), "refpack_segment_count": compressed}));
            }
        }
        serde_json::to_writer_pretty(
            &mut output,
            &serde_json::json!({"schema_version": 1, "streams": streams}),
        )
        .map_err(|error| error.to_string())?;
        return writeln!(output)
            .and_then(|_| output.flush())
            .map_err(|error| error.to_string());
    }
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

fn open_stream_member(
    image: &Path,
    archive_path: &str,
    member_path: &str,
) -> Result<(File, u64, u64), String> {
    let inventory = inventory_disc(image)?;
    let archive = inventory
        .entries
        .iter()
        .find(|entry| !entry.is_directory && entry.path.eq_ignore_ascii_case(archive_path))
        .ok_or("Archive not found on disc.")?;
    let mut file = File::open(image).map_err(|error| error.to_string())?;
    let index = ArchiveIndex::read(&mut file, archive.image_offset, u64::from(archive.size))?;
    let member = index
        .entries
        .iter()
        .find(|entry| entry.path.eq_ignore_ascii_case(member_path))
        .ok_or("Member not found in archive.")?;
    if member.compressed_size_field.unwrap_or(0) != 0
        || u64::from(member.size_field) > member.available_bytes
    {
        return Err("SFIL inspection requires a raw member with an exact bounded length.".into());
    }
    let base = archive
        .image_offset
        .checked_add(member.offset)
        .ok_or("Stream offset overflow.")?;
    Ok((file, base, u64::from(member.size_field)))
}

fn texture_report(bytes: &[u8]) -> Result<serde_json::Value, String> {
    let textures = threecomp_engine_core::texture::inspect_textures(bytes)?;
    let records:Vec<_>=textures.iter().map(|t|serde_json::json!({
        "object_index":t.object_index,"resource_object":t.resource_object,"resource_bytes":t.resource_bytes,
        "width":t.width,"height":t.height,"pitch_pixels":t.pitch_pixels,"format_id":t.format_id,
        "endian_mode":t.endian_mode,"tiled":t.tiled,"packed_mips":t.packed_mips,
        "fetch_words":t.fetch_words,
    })).collect();
    Ok(
        serde_json::json!({"schema_version":1,"metadata_only":true,"texture_count":records.len(),"textures":records}),
    )
}
