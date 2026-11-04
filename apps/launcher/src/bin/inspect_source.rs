use std::{
    env,
    io::{self, Write},
    path::PathBuf,
};

use threecomp_engine_core::{inspect_source, inventory_disc};

fn main() {
    if let Err(error) = run() {
        eprintln!("Inspection failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.is_empty() || args.len() > 2 {
        return Err("Usage: skate-threecomp-inspect <game.iso|default.xex> [--list|--json]".into());
    }
    let mode = args.get(1).map(|arg| arg.to_string_lossy().into_owned());
    if mode
        .as_deref()
        .is_some_and(|mode| mode != "--list" && mode != "--json")
    {
        return Err("Unknown option. Use --list for text or --json for a disc inventory.".into());
    }
    let path = PathBuf::from(&args[0]);
    let info = inspect_source(&path)?;
    let mut output = io::BufWriter::new(io::stdout().lock());
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
