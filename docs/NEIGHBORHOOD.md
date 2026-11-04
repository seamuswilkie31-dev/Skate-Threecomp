# Selected Downtown neighborhood preview

Select the inspected ISO, enter the prototype, and click **Load neighborhood preview**. The worker loads six explicit static batches from `data/content/worldDIST_DownTown.big`. **Place on mesh** uses an upper riding surface in the assembled preview. **Load game mesh sample** returns to the earlier single-batch sample. Loading errors retain the previously loaded preview and leave the original course usable.

| Member under `data/content/world/stream/DIST_DownTown/` | Block (zero-based) | Meshes | Vertices | Render triangles |
|---|---:|---:|---:|---:|
| `cPres_450_150_high.xsf` | 39 | 31 | 1,673 | 1,079 |
| `cPres_550_150_high.xsf` | 15 | 9 | 647 | 413 |
| `cPres_550_50_high.xsf` | 14 | 16 | 1,249 | 888 |
| `cPres_550_250_high.xsf` | 1 | 1 | 33 | 33 |
| `cPres_650_150_high.xsf` | 1 | 1 | 5 | 3 |
| `cPres_650_50_high.xsf` | 7 | 6 | 1,484 | 1,065 |
| **Total** | | **64** | **5,091** | **3,481** |

These are selected batches, not every object in those cells or a complete Downtown map. Additional batches, dynamic objects, instance transforms, original collision, animation, lighting, and streaming remain pending. Geometry remains in its native coordinate system while merging. Border vertices match exactly between several neighboring batches: the center shares 43 unique XYZ positions with the western batch and 45 with the southern batch; the southern batch shares 26 with the southeast batch.

One uniform placement fits the combined bounds into a 16-unit preview centered beside the original course. Individual cells are never normalized separately. Rendering and collision call the same placement. Four zero-area render faces are discarded by collision construction, leaving 3,477 collision triangles. This is still a scaled development preview; distances and movement are not a reconstruction of the original game's world scale or physics.

## Materials and loading limits

The current inspected image supplies ten supported diffuse payloads for 32 of the 64 meshes. The other 32 remain plain because their local texture is absent or uses an unsupported profile. Two optional decoder warnings are exposed in **Loaded source details**, along with the selected member/block list. Lightmap pixels remain available through the separate texture preview; shader interpretation of the packed lightmap-related vertex attributes is unverified, so lighting is not applied.

Material references are resolved within each source member. Identical key/dimension/pixel payloads can share a texture-array layer. Same-key payload variants remain separate; the loader does not borrow an unrelated cell's texture when a local reference is unresolved. Duplicate local definitions are treated as ambiguous and kept plain. The renderer accommodates up to 64 diffuse assets plus the white fallback layer, retaining its current 64 × 64 normalized preview layers.

The worker indexes the disc and archive once. Required geometry errors abort the new load. Optional texture decoding and arena failures produce warnings and plain materials. Limits are 512 scanned blocks, 128 MiB aggregate block work charged by the larger of stored/decoded sizes (including failed attempts), 64 MiB per block, 20,000 render triangles, 100,000 vertices, 4,096 meshes, 64 textures, 32 MiB of decoded texture pixels, and 32 detailed warnings. Size-budget failures remain hard errors. No geometry or pixels are exported.

## Verification

Synthetic checks cover the expanded exact vertex declarations, padded material tables, preserving native seam vertices, shared render/collision placement, identical versus variant texture payloads, and optional codec failure without bypassing budgets. The opt-in ISO test checks all six selected batches, their counts, texture-layer bounds, collision placement, and an ollie/landing on the assembled preview. Native GPU checks render all 64 meshes and 3,481 triangles with 32 linked diffuse maps, and a synthetic layer-10 test checks higher texture-array indices, repeating UVs, occlusion, and compositing. Manual visual interaction with the latest UI remains pending.

```powershell
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --neighborhood-sample
$env:THREECOMP_TEST_ISO = "D:\Games\Skate 3.iso"
cargo test --workspace -- --ignored --nocapture
```

The inspector reports counts, source batches, bounds, collision statistics, texture coverage, and warnings as JSON. It does not emit vertices or pixels.
