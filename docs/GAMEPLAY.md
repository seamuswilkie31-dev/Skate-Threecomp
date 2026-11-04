# Original skating prototype

Open **Open skating prototype** in the launcher. For a direct start, run:

```powershell
cargo run -p skate-threecomp-launcher -- --prototype
```

The prototype runs in the native launcher window with the existing `eframe`/`wgpu` renderer. Its original course, skater placeholder, and movement code require no game assets or ISO. This is an early development environment for the new Rust gameplay systems. The optional imported geometry sample requires the selected ISO. Skate 3's complete world, tricks, animation, physics behavior, and saves have not been reconstructed.

| Input | Action |
|---|---|
| W / Up | Push and accelerate |
| S / Down | Brake |
| A / Left, D / Right | Steer |
| Space | Ollie while grounded |
| F | Start a board flip while airborne |
| Q / E | Hold to spin left/right while airborne |
| R | Reset position and statistics |
| Escape | Return to launcher |

The course contains a two-metre ramp, a platform, a descending slope, boundary markers, and speed-reference stripes. The camera follows the direction of travel and stays steady during spins. Speed, grounded state, contact count, airtime, score, last trick, and regular/switch stance are displayed. Losing window focus pauses the simulation.

## Controller and tricks

The optional native [gilrs controller backend](https://docs.rs/gilrs/latest/gilrs/) supports connection and disconnection during play. The HUD displays the selected controller name or backend status. Keyboard input works alongside it. Xbox-style labels describe the default layout:

| Controller input | Action |
|---|---|
| Left stick | Steer (18% dead zone) |
| A / South | Hold to push |
| B / East | Hold to brake |
| X / West | Ollie |
| Y / North | Flip |
| LB / RB | Hold for left/right air spin |
| Start | Reset |
| Back / Select | Return to launcher |

Try an ollie followed immediately by a flip. A board flip takes 0.45 seconds. For a spin, release the spin control near 180 or 360 degrees before landing. A landing is clean only when a flip has finished and spin is within 30 degrees of a half-turn multiple. The orientation snaps to that multiple after a clean landing. Odd half-turns change stance while preserving travel direction. Body rotation in the air does not redirect horizontal velocity.

A clean ollie earns 100 prototype points, plus 300 per completed flip and 180 per half-turn. Simply dropping from an edge earns no points. An unfinished rotation causes a bail: no points, zero speed, and a brief 0.8-second recovery. Contact count includes bails. This is an original prototype rule set, not Skate 3's scoring or animation system. Reset clears these statistics. One-shot controller actions require a fresh press after focus or active-controller changes.

## Movement and rendering scope

Movement advances at a fixed 120 Hz. Frame time is bounded to avoid long catch-up loops after a pause. The prototype models forward acceleration, rolling resistance, braking, speed-dependent steering, a vertical ollie impulse, gravity, ramp contact, and stopping at course bounds. It is an original tuning model rather than measured Skate 3 behavior. It uses a point contact against a height surface, not full skateboard rigid-body or swept collision physics.

The course and placeholder skater now use a dedicated native wgpu pass with vertex buffers, a camera uniform, perspective projection, and a `Depth32Float` attachment. A compositing pass places the result inside the launcher. The earlier projected/face-sorted view remains available as a fallback when no native wgpu render state is supplied. Board and wheels roll together during flips. Materials, texture decoding, full character animation, and complex collision remain needed. Grinds, ragdoll bails, trick chains, and a full trick vocabulary are pending.

With the initial retail ISO selected in the launcher, **Load game mesh sample** loads a narrow static geometry profile and builds simplified collision in a background worker. The scaled, untextured sample appears on the left side of the course. **Place on mesh** resets the skater and places it on an upper riding surface; **R** returns to the original course. Riding surfaces support the board and steep triangles stop horizontal body movement. The sample is not a complete map or the game's original collision data. See [collision behavior and limits](COLLISION.md). Source bytes and decoded geometry stay in memory. Unsupported images/layouts return a status message while the original course remains usable.

Automated checks cover movement, ollies, ramp following, clean-flip scoring, unfinished-flip bails, switch landings that preserve travel direction, ledge drops without points, controller dead zones, and focus-safe button edges. Explicit native GPU tests verify occlusion/compositing and actual rendering of the retail geometry sample. The native executable is built on Windows. Manual visual interaction with the latest mesh-preview UI remains to be checked; earlier desktop capture attempts timed out. Linux builds with gilrs may need libudev development files, as described in its documentation.
