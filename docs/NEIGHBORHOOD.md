# Selected Downtown neighborhood

Select the inspected ISO and open the prototype. **Load neighborhood preview** creates the miniature beside the original course; **Place on mesh** uses an upper riding surface. **Skate neighborhood** loads the same selected geometry at its source spacing and starts on a verified route. **Test course** returns to the original course. See [source-scale skating controls](SOURCE_SKATING.md).

All seven static batches come from `data/content/worldDIST_DownTown.big`, under `data/content/world/stream/DIST_DownTown/`:

| Member | Block (zero-based) | Meshes | Vertices | Render triangles |
|---|---:|---:|---:|---:|
| `cPres_450_50_high.xsf` | 43 | 46 | 10,612 | 8,121 |
| `cPres_450_150_high.xsf` | 39 | 31 | 1,673 | 1,079 |
| `cPres_550_150_high.xsf` | 15 | 9 | 647 | 413 |
| `cPres_550_50_high.xsf` | 14 | 16 | 1,249 | 888 |
| `cPres_550_250_high.xsf` | 1 | 1 | 33 | 33 |
| `cPres_650_150_high.xsf` | 1 | 1 | 5 | 3 |
| `cPres_650_50_high.xsf` | 7 | 6 | 1,484 | 1,065 |
| **Total** | | **110** | **15,703** | **11,602** |

These are selected batches, not every object in those cells or a complete Downtown map. Additional batches, dynamic objects, instance transforms, original collision, animation, lighting, and streaming remain pending. Geometry stays in native coordinates while merging. Several neighboring batches share bit-identical XYZ border vertices. The additional western/southern batch shares 151 positions with `450_150` and 59 with `550_50`. A bounded two-unit grid below native height 100 in its 100 × 100 footprint found support at 2,495 of 2,500 points after adding that batch, compared with two points beforehand. This measures sampled geometry coverage, not full gameplay accessibility.

The miniature uses one uniform placement fitting the combined bounds into 16 units beside the original course. Source-scale skating applies translation only: scale is exactly 1.0, XZ is recentered, and the lowest Y moves to 0.02. Rendering and collision use the same active placement. Source collision retains 11,593 nondegenerate faces; the miniature retains 11,591 because two additional tiny faces fall below the collision-area threshold after downscaling. Source units have not been calibrated against real-world metres or original-game physics.

## Materials and loading limits

Thirteen supported diffuse payloads cover 54 of the 110 meshes. The other 56 remain plain because their local texture is absent or unsupported. Five optional decoder warnings and the selected member/block list appear under **Loaded source details**. Lightmap pixels remain available in the separate texture preview. Their shader coordinate interpretation is unverified, so lightmaps are not applied to lighting.

References resolve within each source member. Identical key/dimension/pixel payloads may share a texture-array layer; same-key local variants remain separate. Unresolved references do not borrow unrelated cells' textures. Duplicate local definitions are ambiguous and kept plain. Up to 64 diffuse assets plus the white fallback layer fit the renderer, with the existing 64 × 64 normalized preview layers.

The worker indexes the disc/archive once. Required geometry errors abort the new load; optional texture decoding and arena errors produce warnings and plain materials. Limits are 512 scanned blocks, 128 MiB aggregate block work charged by the larger of stored/decoded sizes (including failures), 64 MiB per block, 20,000 render triangles, 100,000 vertices, 4,096 meshes, 64 textures, 32 MiB decoded pixels, and 32 detailed warnings. Budget failures remain hard errors. No geometry or pixels are exported.

## Verification

Synthetic checks cover declarations, padded material tables, native seams, shared placement, texture variants, optional codec failures and budgets, source-only holes and bounds, stale grounded state, recovery scoring, and exclusion of invisible course rails. ISO tests check selected counts, texture-layer bounds, placement, a 20.024-source-unit ride across a cell boundary, and an ollie landing. Native GPU tests render both the miniature and source-scale neighborhood from a skating camera. A synthetic layer-10 test also verifies repeating UVs, depth, and compositing. Visual UI interaction is recorded separately from these GPU checks.

```powershell
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --neighborhood-sample
$env:THREECOMP_TEST_ISO = "D:\Games\Skate 3.iso"
cargo test --workspace -- --ignored --nocapture
```

The JSON report includes selected batches, counts, bounds, texture coverage, warnings, and source-scale availability/spawn/collision data. It emits no vertices or pixels.
