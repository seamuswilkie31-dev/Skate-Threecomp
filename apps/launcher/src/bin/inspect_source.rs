use std::{env, path::Path};

use threecomp_engine_core::inspect_source;

fn main() {
    let Some(path) = env::args_os().nth(1) else {
        eprintln!("Usage: skate-threecomp-inspect <game.iso|default.xex>");
        std::process::exit(2);
    };

    match inspect_source(Path::new(&path)) {
        Ok(info) => println!("Recognized: {}", info.summary()),
        Err(error) => {
            eprintln!("Source not recognized: {error}");
            std::process::exit(1);
        }
    }
}
