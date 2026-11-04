# Development phases

## Phase 1 — Launcher foundation

- [x] Native Rust GUI shell
- [x] Select and remember `.iso` or `.xex` source path
- [x] Show expected `skater.p` path and open its folder
- [ ] Build and inspect on Windows, Linux, and macOS

## Phase 2 — Source inspection

- Define supported retail builds and expected source layouts.
- Inspect user-provided input read-only.
- Detect ISO filesystem metadata and XEX headers without distributing source content.
- Report missing or unsupported files clearly.

## Phase 3 — Engine map

- Keep an evidence log for file formats and engine behaviors.
- Design original Rust interfaces around observed behavior.
- Add synthetic fixtures and unit coverage without copyrighted payloads.

## Phase 4 — Playable slice

- Render a test scene with a native `wgpu` backend.
- Add camera controls and controller input.
- Implement a small original test environment before game content integration.

## Phase 5 — Game systems

Expand physics, animation, trick logic, audio, save handling, menus, and world streaming in small slices, validating each against the user-supplied game copy.

## Phase 6 — Release

Package the launcher and runtime, document supported hardware and source requirements, and provide clear logs for failures.
