# Imported sample collision

The original Rust collision layer uses the same scaled positions and triangles as the visible sample. The initial retail sample supplies 413 nondegenerate triangles and 131 shallow riding triangles. Collision construction runs in the mesh loader worker; the original course remains available. No collision data or assets are distributed.

After **Load game mesh sample**, click **Place on mesh** to reset the skater onto the highest substantial shallow triangle's centroid. **R** or controller Start returns to the original-course spawn while retaining the loaded sample. The mesh placement is a prototype convenience, not a verified spawn point from the game.

## Contact behavior

Riding surfaces have an absolute normalized Y component of at least 0.64 (roughly a 50-degree maximum slope). A barycentric XZ query interpolates their height, accepting either winding. The highest eligible triangle below the query ceiling supports the board. The ceiling is limited to the board's previous height plus the prototype's 0.2-metre step allowance, so a surface far overhead does not pull the skater upward. The procedural course supplies the baseline floor.

Steeper triangles act as walls. Horizontal movement is swept from the old position to the proposed position using three body-height probes at 0.25, 0.9, and 1.5 metres above the riding plane, with a 0.18-metre forward extension. A hit stops horizontal motion and speed. Sweeps are two-sided and detect a thin wall even when the proposed end position lies beyond it. Moving away from a wall is permitted.

The simulation still advances at 120 Hz. Landing and trick evaluation use the queried imported surface as well as the original course. The shadow uses the same support query. Rendering and collision call one shared sample-position transform rather than maintaining separate offsets.

## Scope and validation

This is point support plus horizontal probes, not a capsule solver, a skateboard rigid-body model, or the game's original collision representation. Vertical head/ceiling contact, wall sliding, rounded edge contact, moving objects, rails, and continuous full-body collision remain pending. Narrow surfaces and corners can behave approximately. Imported render geometry can include surfaces that the original game did not intend to be collidable.

Construction rejects invalid coordinates/indexes, discards degenerate triangles, and caps the sample at 20,000 triangles. Queries use bounding checks followed by linear traversal, suitable for the small current sample; a full world needs spatial acceleration.

Synthetic tests cover slopes, both windings, support ceilings, falling onto a roof, remaining below that roof, crossing thin walls in either direction, moving away from a wall, and invalid geometry. An explicit real-image test successfully placed the skater on the imported sample and completed an ollie/landing back onto the same surface. Manual interaction with the latest UI remains pending.

```powershell
$env:THREECOMP_TEST_ISO = "D:\Games\Skate 3.iso"
cargo test -p threecomp-engine-core retail_sample_placement -- --ignored --nocapture
```
