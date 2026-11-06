# Selected Downtown neighborhood

Select the inspected ISO and open the prototype. **Load neighborhood preview** creates the miniature beside the original course; **Place on mesh** uses an upper riding surface. **Skate neighborhood** loads the same selected geometry at its source spacing and starts on a verified route. **Test course** returns to the original course. See [source-scale skating controls](SOURCE_SKATING.md).

All nine static batches come from `data/content/worldDIST_DownTown.big`, under `data/content/world/stream/DIST_DownTown/`:

| Member | Block (zero-based) | Meshes | Vertices | Render triangles |
|---|---:|---:|---:|---:|
| `cPres_450_50_high.xsf` | 43 | 46 | 10,612 | 8,121 |
| `cPres_450_150_high.xsf` | 39 | 31 | 1,673 | 1,079 |
| `cPres_550_150_high.xsf` | 15 | 9 | 647 | 413 |
| `cPres_550_50_high.xsf` | 14 | 16 | 1,249 | 888 |
| `cPres_550_250_high.xsf` | 1 | 1 | 33 | 33 |
| `cPres_550_350_high.xsf` | 1 | 1 | 33 | 33 |
| `cPres_550_450_high.xsf` | 1 | 1 | 22 | 24 |
| `cPres_650_150_high.xsf` | 1 | 1 | 5 | 3 |
| `cPres_650_50_high.xsf` | 7 | 6 | 1,484 | 1,065 |
| **Total** | | **112** | **15,758** | **11,659** |

These are selected batches, not every object in those cells or a complete Downtown map. Additional batches, dynamic objects, instance transforms, original collision, animation, lighting, and streaming remain pending. Geometry stays in native coordinates while merging. Several neighboring batches share bit-identical XYZ border vertices. The additional western/southern batch shares 151 positions with `450_150` and 59 with `550_50`. A bounded two-unit grid below native height 100 in its 100 × 100 footprint found support at 2,495 of 2,500 points after adding that batch, compared with two points beforehand. This measures sampled geometry coverage, not full gameplay accessibility.

The miniature uses one uniform placement fitting the combined bounds into 16 units beside the original course. Source-scale skating applies translation only: scale is exactly 1.0, XZ is recentered, and the lowest Y moves to 0.02. Rendering and collision use the same active placement. Source collision retains 11,650 nondegenerate faces, of which 3,552 pass the shallow-surface threshold. The expanded miniature retains 11,624 collision faces: its smaller scale puts 35 render faces below the collision-area threshold, compared with nine at source scale. Source units have not been calibrated against real-world metres or original-game physics.

## Verified forward route

The two forward batches at `550_350` and `550_450` use the existing 84-byte declaration with a 28-byte vertex stride and half2 first UVs. They extend the selected geometry from native Z 300 through 495.026. At both native Z 300 and Z 400, adjacent terrain batches share seven bit-identical XYZ vertices and six complete triangle edges. No inferred instance transform or synthetic bridge is added.

The native start stays `(512.82666, 84.623, 88.72301)`, facing +Z. With the current combined bounds, translation places it at approximately `(-14.432373, 55.872, -156.9045)`. Expanding the bounds changes XZ recentering, while native vertex positions, unit scale, and source-start Y remain unchanged. Native bounds are approximately `(400, 28.771, -3.771)` through `(654.518, 165.744, 495.026)`.

An explicit 120 Hz simulation holds push from that start and crosses native Z 300, 400, and 490 while grounded, covering about 401.329 source units in 4,210 ticks with no recovery. The known transition near native Z 198 leaves an upper surface and lands once on lower source geometry. The route therefore includes a drop; it is not continuously grounded or a claim of original-game collision behavior. The earlier 20.024-source-unit seam ride and stationary ollie/landing remain separately checked.

## Materials and loading limits

Fifteen supported diffuse payloads cover 57 of the 112 meshes. The other 55 remain plain because their local texture is absent or unsupported. The two forward terrain meshes reference unresolved diffuse key `2C70170A000A000D` and stay plain. Two optional decoder warnings and the selected member/block list appear under **Loaded source details**. Lightmap pixels remain available in the separate texture preview. Their shader coordinate interpretation is unverified, so lightmaps are not applied to lighting.

Three meshes use the newly verified 512 × 256 BC1 bases: key `2C70170A000D01B1` in `450_50` block 3 and `450_150` block 7, and key `2C70170A001D0183` in `450_150` block 2. Their identical base/resource layouts keep mip zero separate from a bounded local mip region. Only base pixels are decoded; the first lower mip's allocation is checked for capacity, and the complete mip chain remains unvalidated. See [exact records, checksums, and limits](TEXTURES.md#neighborhood-bc1-bases-with-separate-mip-data).

The two remaining warnings identify RGBA8 records in `cPres_450_50_high.xsf`, blocks 7 and 8. Their byte order, packed-base placement, and alpha/material behavior need separate decoder work. Other unresolved references remain plain without a decoder warning.

References resolve within each source member. Identical key/dimension/pixel payloads may share a texture-array layer; same-key local variants remain separate. Unresolved references do not borrow unrelated cells' textures. Duplicate local definitions are ambiguous and kept plain. Up to 64 diffuse assets plus the white fallback layer fit the renderer, with the existing 64 × 64 normalized preview layers.

The worker indexes the disc/archive once. The selected nine members contain 168 scanned blocks and charge 13,115,200 bytes of declared block work. Required geometry errors abort the new load; optional texture decoding and arena errors produce warnings and plain materials. Limits are 512 scanned blocks, 128 MiB aggregate block work charged by the larger of stored/decoded sizes (including failures), 64 MiB per block, 20,000 render triangles, 100,000 vertices, 4,096 meshes, 64 textures, 32 MiB decoded pixels, and 32 detailed warnings. Budget failures remain hard errors. No geometry or pixels are exported.

## Verification

Synthetic checks cover declarations, padded material tables, native seams, shared placement, texture variants, bounded BC1 mip regions, optional codec failures and budgets, source-only holes and bounds, stale grounded state, recovery scoring, and exclusion of invisible course rails. ISO tests check selected counts, 57 textured meshes and 15 assets, both new BC1 base checksums, texture-layer bounds, placement, a 20.024-source-unit ride across a cell boundary, and an ollie landing. The extended-route test checks exact forward seam edges, supported Z 300/400/490 crossings, the known drop/landing, unchanged native spawn and source Y, and absence of recovery. The original lightmap checksum remains unchanged. Native GPU tests render both the miniature and source-scale neighborhood from a skating camera. A synthetic layer-10 test also verifies repeating UVs, depth, and compositing. Visual UI interaction is recorded separately from these GPU checks.

```powershell
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --neighborhood-sample
$env:THREECOMP_TEST_ISO = "D:\Games\Skate 3.iso"
cargo test --workspace -- --ignored --nocapture
```

The JSON report includes selected batches, counts, bounds, texture coverage, warnings, and source-scale availability/spawn/collision data. It emits no vertices or pixels.
