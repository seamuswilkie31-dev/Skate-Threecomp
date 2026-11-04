# Development phases

## Phase 1 — Launcher foundation (complete)

- [x] Native Rust GUI shell
- [x] Select and remember `.iso` or `.xex` source path
- [x] Show expected `skater.p` path and open its folder
- [ ] Build and inspect on Windows, Linux, and macOS

## Phase 2 — Source inspection (current)

- [x] Inspect user-provided input read-only.
- [x] Detect XEX2 headers and common XDVDFS image layouts.
- [x] Check volume descriptor and root-directory bounds against the image size.
- [x] Report unsupported or malformed sources in the launcher.
- [x] Parse the XDVDFS root directory tree and locate `default.xex` without extracting it.
- [x] Read title ID, version, base version, and media ID from executable metadata.
- [ ] Recursively inspect nested directories when the executable is not at the root.
- [x] Record an initial retail metadata profile (identification only; runtime compatibility unverified).
- [ ] Cryptographically identify the executable and verify additional retail or regional builds.

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
