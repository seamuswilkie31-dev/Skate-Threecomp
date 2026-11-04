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
- [x] Recursively inventory disc directories with bounded traversal and text/JSON reports.
- [x] Record an initial retail metadata profile (identification only; runtime compatibility unverified).
- [ ] Cryptographically identify the executable and verify additional retail or regional builds.

## Phase 3 — Engine map

- [x] Inventory the initial retail disc and record candidate asset families.
- [x] Identify observed EB3/BIG4 archive signatures and implement bounded index readers.
- [x] Read capped raw member prefixes through disc and archive indexes.
- [x] Index the observed SFIL v3 block and segment layout with exact member bounds.
- [x] Decode raw and five-byte-header RefPack segments in memory with explicit size budgets.
- [ ] Investigate decoded RW4 object tables, geometry, and textures.
- [x] Read observed Xbox 360 RW4 dictionaries and resolve bounded resource regions.
- [ ] Verify remaining SFIL variants and codec flags as evidence becomes available.
- Keep an evidence log for file formats and engine behaviors.
- Design original Rust interfaces around observed behavior.
- Add synthetic fixtures and unit coverage without copyrighted payloads.

## Phase 4 — Playable slice (prototype started alongside asset research)

- [x] Render an original test course through the launcher's native `wgpu` backend.
- [x] Add a follow camera, keyboard push/brake/steering, and ollies.
- [x] Implement fixed-step movement, ramp contact, gravity, and landing tracking.
- [ ] Add controller input, board rotation, and trick detection.
- [ ] Build a depth-buffered world renderer and improve the movement/collision model.
- [ ] Connect decoded geometry and textures to the scene.

## Phase 5 — Game systems

Expand physics, animation, trick logic, audio, save handling, menus, and world streaming in small slices, validating each against the user-supplied game copy.

## Phase 6 — Release

Package the launcher and runtime, document supported hardware and source requirements, and provide clear logs for failures.
