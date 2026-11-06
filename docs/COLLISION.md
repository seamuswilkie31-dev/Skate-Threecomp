# Imported sample collision

The original Rust collision layer uses the same scaled positions and triangles as the visible sample. The initial retail sample supplies 413 nondegenerate triangles and 131 shallow riding triangles. Collision construction runs in the mesh loader worker; the original course remains available. No collision data or assets are distributed.

After **Load game mesh sample**, click **Place on mesh** to reset the skater onto the highest substantial shallow triangle's centroid. **R** or controller Start returns to the original-course spawn while retaining the loaded sample. The mesh placement is a prototype convenience, not a verified spawn point from the game.

## Contact behavior

Riding surfaces have an absolute normalized Y component of at least 0.64 (roughly a 50-degree maximum slope). A barycentric XZ query interpolates their height, accepting either winding. The highest eligible triangle below the query ceiling supports the board. The ceiling is limited to the board's previous height plus the prototype's 0.2-metre step allowance, so a surface far overhead does not pull the skater upward. The procedural course supplies the baseline floor.

Steeper triangles act as walls. Horizontal movement uses three body probes at nominal heights of 0.25, 0.9, and 1.5 units above the support surface (offset from the board by subtracting its 0.12-unit riding height), with a 0.18-unit horizontal wall-plane radius. The resolver advances to contact and projects remaining movement along the wall, handling at most two contacts per step. It limits total displacement so sliding cannot add speed. Grounded heading follows the retained tangent; an airborne slide adjusts travel direction while preserving trick rotation. A clean landing reconciles body orientation and stance with that travel direction. Sweeps are two-sided and detect a thin wall even when the proposed endpoint lies beyond it. Tangential motion and moving away remain permitted.

An upward head sweep, 1.65 units above the board, stops vertical ascent at the nearest ceiling. Gravity then resumes descent. Grounded movement also checks body clearance at the proposed support point and sweeps the head during an uphill rise; an obstructed step is rejected rather than placing the board beneath its required floor. Airborne horizontal moves also reject new body overlap with low overhangs before gravity and landing. If a rider already overlaps geometry, equal or improved nearest vertical clearance permits small escape steps; this center-line approximation cannot resolve every overlapping shape. These distances follow prototype units, or uncalibrated source units in source mode. Arbitrary segment raycasts additionally support the [camera's clearance probes](CAMERA.md).

The simulation still advances at 120 Hz. Landing and trick evaluation use the queried imported surface as well as the original course. The shadow uses the same support query. Rendering and collision call one shared sample-position transform rather than maintaining separate offsets.

## Scope and validation

This is point support plus body/head probes, not a capsule solver, a skateboard rigid-body model, or the game's original collision representation. Rounded edge contact, moving objects, game-data rails, and continuous full-body collision remain pending. There is no general depenetration solver. Authored [practice rails](GAMEPLAY.md) use a separate descending-capture and rail-follow constraint, with the earlier stop-on-wall probes guarding their motion. Narrow surfaces and corners can behave approximately, and gaps between probe heights can miss thin obstacles. Imported render geometry can include surfaces that the original game did not intend to be collidable.

Construction rejects invalid coordinates/indexes, discards degenerate triangles, and caps the sample at 20,000 triangles. Queries use bounding checks followed by linear traversal, suitable for the small current sample; a full world needs spatial acceleration.

Synthetic tests cover slopes, both windings, support ceilings, nearest bounded ray hits, falling onto a roof, remaining below it, upward ceiling contact, grounded/airborne low-overhang entry, escape from an existing overlap, improving/worsening clearance, uphill headroom, wall sliding, radius contact, corners, stationary/tangent/away motion, displacement limits, airborne slide/landing, and invalid geometry. An explicit real-image test placed the skater on the imported sample and completed an ollie/landing back onto the same surface. A second rides across a source cell seam before an ollie/landing. Manual interaction with the latest UI remains pending.

```powershell
$env:THREECOMP_TEST_ISO = "D:\Games\Skate 3.iso"
cargo test -p threecomp-engine-core retail_sample_placement -- --ignored --nocapture
```

## Source-only mode

The [source skating mode](SOURCE_SKATING.md) uses `SourceWorld`: support is optional and comes only from imported triangles. It has independent finite XZ movement bounds and an empty rail list. The original `World` keeps the test-course baseline and practice rails. Skating clears stale grounded state before banking or bail recovery when support disappears; missing support never becomes a numeric floor. The pre-step support ceiling still prevents snapping up through a roof. Recovery tries a revalidated [session spot](SPOTS.md), otherwise a separately verified source start, and clears only pending/transient state while preserving banked points. Tests cover holes, bounds beyond the original course, source-only falls, stale grounding, rail exclusion, safe lower-surface candidates, and invalid checkpoint rejection.
