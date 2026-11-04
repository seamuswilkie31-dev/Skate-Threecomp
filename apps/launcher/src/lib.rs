use std::io::Write;

use threecomp_engine_core::{DiscInventory, SourceInfo};

/// Write only disc metadata; no game payloads or host source path are included.
pub fn write_inventory_json(
    output: &mut impl Write,
    info: &SourceInfo,
    inventory: &DiscInventory,
) -> Result<(), String> {
    let entries: Vec<_> = inventory
        .entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "path": entry.path,
                "is_directory": entry.is_directory,
                "size": entry.size,
                "image_offset": entry.image_offset,
            })
        })
        .collect();
    let report = serde_json::json!({
        "schema_version": 1,
        "source_status": info.status(),
        "file_count": inventory.file_count(),
        "directory_count": inventory.directory_count(),
        "total_file_bytes": inventory.total_file_bytes,
        "entries": entries,
    });
    serde_json::to_writer_pretty(&mut *output, &report).map_err(|error| error.to_string())?;
    writeln!(output).map_err(|error| error.to_string())
}
