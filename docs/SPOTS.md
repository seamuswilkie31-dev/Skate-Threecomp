# Session spot markers

Press **M** or controller **D-pad Up**, or click **Set spot**, while riding on a broad supported surface. A cyan pole and travel-direction arrow mark the position. Press **Enter** or **D-pad Down**, or click **Return to spot**, to retry from there at rest. Captured body heading and regular/switch stance are preserved. Camera position and pending button events reset so a return does not immediately launch a queued ollie or flip.

Returning keeps banked score, last banked chain, landing count, and last completed airtime. It clears an unbanked chain, current air/flip/grind/bail state, and speed. Setting a spot does not change rider state or score. If placement fails, the previous valid spot remains available and the HUD explains the rejection.

Source-mode gap recovery first tries the current spot. If no valid spot exists, it uses the researched source start. The recovery counter includes automatic returns; manual spot returns do not increment it. **R** or Start, Reset, Source start, Place on mesh, Rail practice, and mode switches clear the marker and restart prototype statistics. Replacing loaded geometry also clears it. A failed source load keeps the existing scene and its marker.

Markers exist only in memory for the current prototype session and terrain. They are not original-game spawn records, save-file support, or edits to `skater.p`. Returning to the launcher ends the prototype session. No marker files or game assets are written.

## Placement and return checks

The original Rust checkpoint module requires a finite, stable, grounded rider outside a bail or grind. Board height must agree with support. Nine support/headroom samples check a small footprint, while eight short radial movement probes and body-height rays reject nearby walls, edges, gaps, and bounds. Placement is deliberately conservative and can reject a narrow surface that can still be ridden. These probes are not a complete capsule or board collision volume.

Return validates the stored pose against active terrain before changing the rider. A failed return leaves rider state unchanged and removes the unavailable marker. Mode/source identity belongs to the launcher; it discards spots when that identity changes. Automatic source recovery falls back to the researched start when validation fails.

Reset/mode changes suppress spot commands in the same frame. Otherwise return takes precedence over placement when both arrive together. Focused UI buttons consume their keyboard activation without also triggering global Enter/Space gameplay actions. Controller one-shot actions require a fresh press after focus or active-controller changes, as with ollies and resets.

Synthetic tests cover capture without mutation, return state and switch stance, unsupported/unstable poses, footprint edges, low roofs, nearby and exactly intersecting walls, failed-return immutability, and launcher queue/reset behavior. An opt-in ISO test captures the researched source start, rides twenty source units, and returns from an ollie while keeping banked points. Manual visual interaction with the latest UI remains pending.
