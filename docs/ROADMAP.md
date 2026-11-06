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
- [x] Decode one observed static float3/28-byte vertex and uint16 triangle-list profile.
- [x] Expand to six exact observed declarations with 20/24/28/32/36-byte strides and half2/half4 first UVs.
- [x] Decode the first half-float diffuse UV set and validate draw-to-material associations.
- [x] Decode narrow tiled BC1 and RGB565 base levels with verified byte order and channel profiles.
- [ ] Investigate other geometry layouts, textures, transforms, and full scene assembly.
- [x] Read observed Xbox 360 RW4 dictionaries and resolve bounded resource regions.
- [ ] Verify remaining SFIL variants and codec flags as evidence becomes available.
- Keep an evidence log for file formats and engine behaviors.
- Design original Rust interfaces around observed behavior.
- Add synthetic fixtures and unit coverage without copyrighted payloads.

## Phase 4 — Playable slice (prototype started alongside asset research)

- [x] Render an original test course through the launcher's native `wgpu` backend.
- [x] Add a follow camera, keyboard push/brake/steering, and ollies.
- [x] Add mouse/controller camera orbit, wheel zoom, recentering, and bounded obstacle-clearance probes.
- [x] Implement fixed-step movement, ramp contact, gravity, and landing tracking.
- [x] Add native controller input with hotplugging, dead zones, and focus-safe button edges.
- [x] Animate board flips and detect clean flip/spin landings, switch stance, scores, and simple bails.
- [x] Add authored practice rails with 50-50 capture, balance, pop-offs, and distance scoring.
- [x] Add pending trick chains, multipliers, clean-riding banking, and bail loss.
- [ ] Add other grind types, game-data rails, richer character animation, and a broader trick vocabulary.
- [x] Build a dedicated depth-buffered 3D pass and composite it into the native launcher.
- [x] Connect a bounded real-game mesh sample to a native visual preview.
- [x] Connect the imported sample to shallow-surface support and swept wall probes with shared render transforms.
- [x] Add a mesh placement action and verify an ollie/landing on the real imported sample.
- [x] Add wall sliding, upward head/ceiling contact, and grounded headroom checks to the probe-based collision model.
- [ ] Improve full-body collision, rounded edge contacts, and original game collision-format support.
- [x] Inspect observed 2D texture descriptors and Xenos fetch metadata.
- [x] Bind four local diffuse textures to six sample meshes and preview a decoded lightmap.
- [x] Assemble seven selected neighboring static batches with shared placement (110 meshes, 11,602 render triangles, 57 textured meshes).
- [x] Decode verified BC1 base levels with a separately bounded local mip region; retain unsupported packed-tail fallbacks.
- [x] Add source-scale skating with translation-only geometry, source-only support/bounds, verified route start, and gap recovery that preserves banked points.
- [ ] Resolve the remaining material keys, packed base tails, other formats, and mip levels.
- [ ] Verify lightmap coordinates, original lighting, scene transforms, and complete scene assembly.

## Phase 5 — Game systems

Expand physics, animation, trick logic, audio, save handling, menus, and world streaming in small slices, validating each against the user-supplied game copy.

## Phase 6 — Release

Package the launcher and runtime, document supported hardware and source requirements, and provide clear logs for failures.
