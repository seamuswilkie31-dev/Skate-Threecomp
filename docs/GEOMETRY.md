# First static geometry and native renderer

## Evidence and source independence

The new mesh reader was written from direct observations of the user's own retail-profile image. General RW4 mesh/buffer terminology was compared with [original RX2 research](https://soccergaming.com/forums/threads/fifa-11-pc-file-formats-resarch-renderware-4-5-assets.6468020/page-2); the compact type IDs and reference encoding below were investigated independently in this game. No code from Skate 3 Recompiled or other implementations was copied. The repository contains synthetic fixtures and original shaders, with no game payloads.

The explicit research sample is archive `data/content/worldDIST_DownTown.big`, member `data/content/world/stream/DIST_DownTown/cPres_550_150_high.xsf`, block 15 (zero-based). The exact sample is intentionally fixed while the first loader is developed. It successfully decodes 9 meshes, 647 vertices, and 413 triangles. Original bounds are approximately `(500, 51.344, 100)` through `(600, 137.541, 200)`.

## Supported narrow layout

Draw objects with type `0x00EB0023` link a vertex declaration at byte 40 and a mesh object at byte 44. The observed mesh type `0x00020081` holds index-buffer and vertex-buffer object indexes. Its vertex record (`0x000200EA`) refers to a raw `0x00010031` resource via the observed `(object_index << 2) | 3` encoding; its index record (`0x000200EB`) uses a direct raw-object index. These conventions are specific to the verified profile and are not treated as universal RW4 rules.

The reader now matches six complete observed declarations (`0x000200E9`): two 20-byte stride variants and one each at 24, 28, 32, and 36 bytes. Their declaration lengths are 67, 84, or 101 bytes: a 16-byte header, 16 bytes per attribute, and one repeated stride byte per attribute. Position remains the first three big-endian float32 values, token `0x002A23B9`. First UV data at byte 12 is half2 (`0x002C235F`) or half4 (`0x001A2360`), never float2. Half4's first two components feed the preview; all four must be finite, and the remaining two stay uninterpreted. Complete attribute offsets, tokens, semantic words, header flags, and stride markers must match an observed profile. Other attributes remain uninterpreted. The accepted index profile is a big-endian uint16 triangle list. Explicit draw ranges must be contiguous, have zero base vertex, use observed primitive value 4, and cover exactly the recorded index count. Buffer padding is excluded from geometry.

All linked object types, lengths, reference indexes, finite coordinates, and triangle indexes are checked. Limits are 4,096 meshes, 1,000,000 vertices, and 3,000,000 indexes within the existing 64 MiB decoded-arena budget. Unknown layouts, topologies, transforms requiring nonzero base vertices, and invalid data return errors. This first path does not apply instance transforms or load all surrounding scene objects.

## Preview behavior

Select the inspected ISO in the launcher, enter the skating prototype, then click **Load game mesh sample**. Loading runs in a worker while the original course stays usable. The geometry is scaled to fit an eight-metre preview and placed left of the course. Rendering and simplified collision share the exact same position transform. Six meshes use four locally decoded diffuse maps with the verified first UV set; three unresolved meshes remain plain. Simple face shading is retained. Optional material errors fall back to plain rendering while preserving geometry and collision. See [material scope](MATERIALS.md). Click **Place on mesh** to use a riding surface. The sample has 413 collision triangles, of which 131 pass the prototype slope threshold. This is a small sample with collision derived from render triangles, not a complete map or original collision database. Geometry and pixels remain in memory and no extraction output is written.

The inspector's `--mesh-sample` option invokes the same loader and emits counts and bounds as JSON, without exporting vertices.

The separate [neighborhood preview](NEIGHBORHOOD.md) combines seven selected batches using one placement, preserving their native border coordinates. It contains 110 meshes and 11,602 render triangles, with 57 meshes using fifteen local diffuse payloads. A separate source-skating mode keeps scale 1.0 and shares translation-only placement between rendering and source-only collision. `--neighborhood-sample` reports its coverage and warnings. It does not imply complete scene assembly or support for all nearby batches.

## Native rendering path

Original WGSL shaders project world vertices and sample a diffuse texture array on the GPU. Up to 64 diffuse layers are normalized to 64 × 64 with nearest duplication and sampled with repeating UVs and linear filtering. A white layer preserves the original course and plain fallback. Uploads are cached per loaded sample. A dedicated offscreen RGBA8 color attachment and `Depth32Float` depth attachment use depth writes and the `Less` comparison. The camera uses a 0.15-metre near plane and 500-metre far plane. A second pipeline composites the viewport through egui's callback mechanism, preserving the existing launcher and HUD. Viewport attachments resize with display scale, capped at 4,096 pixels per dimension. GPU buffers grow when needed and are reused between frames. The older projected view remains an untextured fallback if wgpu render state is absent.

## Validation

Synthetic tests cover a known triangle, padding, object links, unsupported declarations/topologies, half-float UVs, non-finite coordinates, truncated draw records, and invalid indexes. Four native GPU tests were run explicitly on an NVIDIA GeForce RTX 5060 Ti. They verify texture-layer selection, repeating UVs, near-over-far occlusion, compositing, the nine-mesh retail sample, the 110-mesh neighborhood preview, and the source-scale scene from its collision-adjusted follow camera. Both neighborhood views resolve 57 textured meshes. All read back visible pixels with no wgpu validation errors. These checks do not require desktop capture or distribute game data.

The GPU tests are ignored in ordinary `cargo test --workspace` runs because they require hardware, and the retail test also requires `THREECOMP_TEST_ISO` pointing to the local image. To run them on a development machine:

```powershell
$env:THREECOMP_TEST_ISO = "D:\Games\Skate 3.iso"
cargo test -p skate-threecomp-launcher --bin skate-threecomp graphics::tests -- --ignored --nocapture
```

An explicit real-image simulation test also verifies that the selected placement supports the skater, and an ollie lands back on that surface. See [collision notes](COLLISION.md). Manual validation of the latest UI and preview composition is still pending. Next work includes broader mesh layouts, unresolved textures, original lighting, scene transforms, animation, and more complete collision behavior.
