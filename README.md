# Skate Threecomp

A clean-room Rust project exploring a native PC port of Skate 3. The project aims to reconstruct game systems in Rust and render through a native graphics backend. The launcher uses `egui`/`wgpu`, which can select platform-native graphics backends such as Direct3D 12 or Vulkan through wgpu.

This repository is an independent project. It does not contain or use code from Skate 3 Recompiled. The Skate 2 Rust Port is an architectural reference only; no source code has been copied from it. No copyrighted game code, assets, disc images, or executables are distributed here. Users must supply their own legally obtained game files.

## Current phase

An early native skating prototype is now available. Click **Open skating prototype** in the launcher, or run `cargo run -p skate-threecomp-launcher -- --prototype`. Push, steer, brake, and ollie around an original ramp course with a follow camera. It uses placeholder shapes and an original movement model; game-world loading and Skate 3 gameplay reconstruction remain in development. The prototype needs no game files. See [controls and scope](docs/GAMEPLAY.md).

The prototype now accepts controller input and includes board flips, airborne spins, clean-landing scoring, switch stance, and a simple bail/recovery state. On keyboard, use **F** to flip and **Q/E** to spin after an ollie. The HUD shows controller status and the latest trick result.

The prototype now uses a dedicated native 3D pass with depth testing. With the inspected retail ISO selected, click **Load game mesh sample** inside the prototype to load a small original game-geometry sample beside the course. It is an untextured visual preview, scaled for inspection, with no imported collision. The sample stays in memory. See [geometry and renderer notes](docs/GEOMETRY.md).

Phase 2 provides read-only source inspection for Xbox 360 `.iso` and `.xex` files. The launcher checks the XEX2 header or the XDVDFS volume descriptor, walks the root directory tree, locates `default.xex`, and reads its execution metadata (title ID, version, base version, and media ID). It recognizes one Skate 3 retail metadata profile; this is an identifier match, not a cryptographic executable check or proof of runtime compatibility. The game executable is not loaded and Play remains disabled. See [the profile notes](docs/SUPPORTED_BUILDS.md).

The `skate-threecomp-inspect` command-line tool runs the same checks without opening the launcher:

```sh
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso"
```

Use **Export file inventory** in the launcher to save a JSON list of disc paths, sizes, directory flags, and absolute image offsets. The scan runs in the background. It reads directory metadata and does not extract assets. A standalone `.xex` has no disc filesystem to inventory.

The command-line tool also supports a recursive text listing or JSON output:

```powershell
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --list
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --json > inventory.json
```

The inventory includes file metadata only. Keep generated reports local while investigating your copy. Initial asset research is recorded in [the engine map](docs/ENGINE_MAP.md).

The Rust archive reader can index the `EB` version 3 and `BIG4` layouts observed on the initial disc. List archive counts, inspect one archive's members, or read a 32-byte raw member prefix:

```powershell
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --archives
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --archive "data/content/worldDIST_StartPark.big"
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --prefix "data/content/worldDIST_StartPark.big" "data/content/world/stream/DIST_StartPark/cPres_50_-50_high.xsf"
```

These modes output JSON and keep all game files in place. Indexing and raw previews do not decode or decompress assets. See [archive reader notes](docs/ARCHIVES.md) for bounds, offset units, and compression limitations.

The next loading layer indexes the observed `SFIL` v3 stream layout and decodes raw or `10 FB` RefPack block segments in memory. Inspect the stream census, one member's blocks, or a decoded block's size and first 32 bytes:

```powershell
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --streams
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --stream "data/content/worldDIST_DownTown.big" "data/content/world/stream/DIST_DownTown/cSim_50_450_high.xsf"
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --decode-block "data/content/worldDIST_DownTown.big" "data/content/world/stream/DIST_DownTown/cSim_50_450_high.xsf" 0
```

These diagnostic modes do not write decoded game assets to disk. Decoded `RW4` data still needs geometry and texture readers before it can be rendered. See [stream notes](docs/STREAMS.md) for the observed layout and validation scope.

The observed Xbox 360 RW4 object dictionary can also be inspected after decoding a stream block:

```powershell
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --rw4-block "data/content/worldDIST_DownTown.big" "data/content/world/stream/DIST_DownTown/cSim_50_450_high.xsf" 0
```

This reports object types and bounded resource offsets. It does not decode object-specific geometry or textures; see [RW4 notes](docs/RW4.md).

`--mesh-sample` runs the same narrow sample loader as the prototype and reports mesh, vertex, triangle, and bounding-box counts as JSON. The initial retail sample contains 9 meshes, 647 vertices, and 413 triangles.

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

The project currently includes a launcher, original skating prototype, and asset inspection and decompression tools. A complete Skate 3 port requires substantial additional reverse engineering and implementation.

## Independence and contributions

Keep all implementation work original. Do not copy or adapt code from Skate 3 Recompiled. Record technical observations and implement them independently. Contributions must not include game files or assets.

## License

The original project code is licensed under MIT. Game names and any referenced game materials remain the property of their respective owners.
