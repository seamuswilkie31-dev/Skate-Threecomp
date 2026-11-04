# Skate Threecomp

A clean-room Rust project exploring a native PC port of Skate 3. The project aims to reconstruct game systems in Rust and render through a native graphics backend. The launcher uses `egui`/`wgpu`, which can select platform-native graphics backends such as Direct3D 12 or Vulkan through wgpu.

This repository is an independent project. It does not contain or use code from Skate 3 Recompiled. The Skate 2 Rust Port is an architectural reference only; no source code has been copied from it. No copyrighted game code, assets, disc images, or executables are distributed here. Users must supply their own legally obtained game files.

## Current phase

Phase 1 provides a Windows/Linux/macOS native launcher shell with a source file picker for Xbox 360 `.iso` and `.xex` files, remembered source selection, and a save folder control showing the expected `skater.p` path. The launcher does not yet parse either format or start the game. The Play control stays disabled until a game runtime exists.

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

1. **Launcher foundation** — native shell, source selection, settings, and save path discovery. Current phase.
2. **Source inspection** — document the legally supplied formats, implement read-only ISO/XEX identification and integrity checks, and agree on the supported retail build.
3. **Engine map** — reverse engineer file/container formats and the major runtime systems from a user-supplied copy; record findings and create format fixtures that contain no copyrighted data.
4. **Playable slice** — load one small environment, a skater, a camera, and controller input; establish the graphics abstraction and core physics loop.
5. **Gameplay systems** — expand animation, tricks, collisions, audio, menus, saves, and additional world content in independently testable slices.
6. **Release quality** — performance, compatibility, settings, packaging, crash reporting, and documentation.

The project is at the launcher stage. A complete Skate 3 port is a large reverse-engineering effort and is not implied by this initial scaffold.

## Independence and contributions

Keep all implementation work original. Do not copy or adapt code from Skate 3 Recompiled. Record technical observations and implement them independently. Contributions must not include game files or assets.

## License

The original project code is licensed under MIT. Game names and any referenced game materials remain the property of their respective owners.
