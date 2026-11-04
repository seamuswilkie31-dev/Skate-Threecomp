# Skate Threecomp

A clean-room Rust project exploring a native PC port of Skate 3. The project aims to reconstruct game systems in Rust and render through a native graphics backend. The launcher uses `egui`/`wgpu`, which can select platform-native graphics backends such as Direct3D 12 or Vulkan through wgpu.

This repository is an independent project. It does not contain or use code from Skate 3 Recompiled. The Skate 2 Rust Port is an architectural reference only; no source code has been copied from it. No copyrighted game code, assets, disc images, or executables are distributed here. Users must supply their own legally obtained game files.

## Current phase

Phase 2 provides read-only source inspection for Xbox 360 `.iso` and `.xex` files. The launcher checks the XEX2 header or the XDVDFS volume descriptor, walks the root directory tree, locates `default.xex`, and reads its execution metadata (title ID, version, base version, and media ID). It recognizes one Skate 3 retail metadata profile; this is an identifier match, not a cryptographic executable check or proof of runtime compatibility. The game executable is not loaded and Play remains disabled. See [the profile notes](docs/SUPPORTED_BUILDS.md).

The `skate-threecomp-inspect` command-line tool runs the same checks without opening the launcher:

```sh
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso"
```

Save data is stored under the platform's local application data directory:

- Windows: `%LOCALAPPDATA%\Threecomp\Skate Threecomp\saves\skater.p`
- macOS: `~/Library/Application Support/com.Threecomp.Skate Threecomp/saves/skater.p`
- Linux: `$XDG_DATA_HOME/Threecomp/Skate Threecomp/saves/skater.p` (or `~/.local/share/...`)

## Build

Install the stable Rust toolchain, then run from the repository root:

```sh
cargo run -p skate-threecomp-launcher
```

The `wgpu` backend uses Direct3D 12 on supported Windows systems and Vulkan where available. Backend selection and platform dependencies can vary by driver and operating system.

## Phases

1. **Launcher foundation** — native shell, source selection, settings, and save path discovery. Complete.
2. **Source inspection** — read-only ISO/XEX identification, root directory traversal, executable metadata, and an initial retail metadata profile. Current phase; executable identity verification and runtime compatibility remain open.
3. **Engine map** — reverse engineer file/container formats and the major runtime systems from a user-supplied copy; record findings and create format fixtures that contain no copyrighted data.
4. **Playable slice** — load one small environment, a skater, a camera, and controller input; establish the graphics abstraction and core physics loop.
5. **Gameplay systems** — expand animation, tricks, collisions, audio, menus, saves, and additional world content in independently testable slices.
6. **Release quality** — performance, compatibility, settings, packaging, crash reporting, and documentation.

The project is at the launcher stage. A complete Skate 3 port is a large reverse-engineering effort and is not implied by this initial scaffold.

## Independence and contributions

Keep all implementation work original. Do not copy or adapt code from Skate 3 Recompiled. Record technical observations and implement them independently. Contributions must not include game files or assets.

## License

The original project code is licensed under MIT. Game names and any referenced game materials remain the property of their respective owners.
