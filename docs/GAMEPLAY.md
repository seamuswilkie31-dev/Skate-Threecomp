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
| A / Left, D / Right | Steer; balance while grinding |
| Space | Ollie while grounded; pop off a rail |
| F | Start a board flip while airborne |
| Q / E | Hold to spin left/right while airborne |
| G (hold) | Catch and hold a grind |
| R | Reset position and statistics |
| Escape | Return to launcher |

The course contains a two-metre ramp, a platform, a descending slope, two practice rails, boundary markers, and speed-reference stripes. The camera follows the direction of travel and stays steady during spins. Speed, riding/air/grind state, contact count, airtime, banked score, pending chain, last trick, and regular/switch stance are displayed. Losing window focus pauses the simulation.

## Controller and tricks

The optional native [gilrs controller backend](https://docs.rs/gilrs/latest/gilrs/) supports connection and disconnection during play. The HUD displays the selected controller name or backend status. Keyboard input works alongside it. Xbox-style labels describe the default layout:

| Controller input | Action |
|---|---|
| Left stick | Steer or grind balance (18% dead zone) |
| A / South | Hold to push |
| B / East | Hold to brake |
| X / West | Ollie or rail pop-off |
| Y / North | Flip |
| LB / RB | Hold for left/right air spin |
| LT (hold) | Catch and hold a grind |
| Start | Reset |
| Back / Select | Return to launcher |

Try an ollie followed immediately by a flip. A board flip takes 0.45 seconds. For a spin, release the spin control near 180 or 360 degrees before landing. A landing is clean only when a flip has finished and spin is within 30 degrees of a half-turn multiple. The orientation snaps to that multiple after a clean landing. Odd half-turns change stance while preserving travel direction. Body rotation in the air does not redirect horizontal velocity.

A clean ollie contributes 100 prototype base points, plus 300 per completed flip and 180 per half-turn. Simply dropping from an edge earns no points. Points remain in a pending chain. Linking another trick before banking increases the chain multiplier, up to ×8; its value is total base points times that multiplier. Ride continuously for 1.5 seconds to bank the chain. Beginning another air segment restarts that grounded window, and grinding keeps the chain pending. A bail clears pending points, stops the skater, and starts a 0.8-second recovery while retaining previously banked points. Contact count includes rotational landing bails. This is an original prototype rule set, not Skate 3's scoring or animation system. Reset clears these statistics. One-shot controller actions require a fresh press after focus or active-controller changes.

## Rail practice

Click **Rail practice** to start moving toward the first yellow rail. Hold **G** (pad **LT**) and tap **Space** (pad **X**) to ollie. An immediate ollie from this position is calibrated to catch the first rail on descent. A flip during the approach can link into the grind. Use **A/D** or the stick to keep the balance marker near the center. Reaching either edge of the meter causes a bail.

The two original rails are horizontal: one at x=7, top height 0.65 m, z=-2 through 8; the other at x=11, height 0.9 m, z=12 through 24. Capturing requires a descending crossing of the rail top, a lateral distance at most 0.32 m, speed at least 2 m/s, a travel heading within 25 degrees of the rail in either direction, and a completed/aligned rotation. Passing underneath or alongside a rail does not attach the board. Switch stance preserves the signed direction of travel.

Hold the grind button to follow the rail. Push does not accelerate a grind; friction and braking reduce speed. A/D or stick input affects deterministic balance drift. Space/X pops off, releasing the hold button drops off, and the rail endpoint or speed below 0.75 m/s also dismounts. Every exit has a 0.25-second recapture cooldown. Pop takes precedence over releasing the hold button in the same tick. Rail movement checks the existing wall probes and course bounds.

A completed rail segment contributes 200 base points plus 40 per metre, calculated from cumulative distance once on exit. At least 0.25 m of travel is required, so an immediate stationary capture/release cannot award grind points. A flip → grind → pop → clean landing can form one chain. Unfinished flips reject capture, and an unfinished landing rotation or failed balance loses the pending chain. The current grind is a simple 50-50 constraint on authored practice rails; other grind types, rail extraction from game data, board rigid-body contact, and original-game behavior remain future work.

## Movement and rendering scope

Movement advances at a fixed 120 Hz. Frame time is bounded to avoid long catch-up loops after a pause. The prototype models forward acceleration, rolling resistance, braking, speed-dependent steering, a vertical ollie impulse, gravity, ramp contact, and stopping at course bounds. It is an original tuning model rather than measured Skate 3 behavior. It uses a point contact against a height surface, not full skateboard rigid-body or swept collision physics.

The course and placeholder skater use a dedicated native wgpu pass with vertex buffers, a camera uniform, perspective projection, and a `Depth32Float` attachment. A compositing pass places the result inside the launcher. The earlier projected/face-sorted view remains a plain fallback when no native wgpu render state is supplied. Board and wheels roll together during flips. Rails highlight during grinding and the rider leans with balance. A narrow diffuse-texture path is implemented for the optional sample. Full material behavior, character animation, complex collision, ragdoll bails, and a full trick vocabulary remain needed.

With the initial retail ISO selected in the launcher, **Load game mesh sample** loads a narrow static geometry profile, builds simplified collision, and loads verified material associations in a background worker. Six of the nine meshes use four diffuse textures; three unresolved meshes stay plain. The scaled sample appears on the left side of the course. **Place on mesh** resets the skater and places it on an upper riding surface; **R** returns to the original course. Riding surfaces support the board and steep triangles stop horizontal body movement. **Load texture sample** opens a separate 512 × 512 lightmap preview; **Show texture** reopens it after closing. Applying lightmaps to mesh lighting is pending. The sample is not a complete map or the game's original collision data. See [collision behavior and limits](COLLISION.md) and [material notes](MATERIALS.md). Source bytes and decoded assets stay in memory. Unsupported images/layouts return a status message while the original course remains usable.

Automated checks cover movement, ollies, ramp following, clean-flip scoring, unfinished-flip bails, switch landings, ledge drops without points, swept rail capture/rejection, reverse/switch rail travel, endpoint/release/pop exits, wall obstruction, stationary-grind rejection, balance bails, chain loss/banking, the exact 180-tick banking boundary, controller dead zones, and focus-safe button edges. Explicit native GPU tests verify occlusion/compositing and actual rendering of the retail geometry sample. The native executable is built on Windows. Manual visual interaction with the latest UI remains to be checked; Windows capture attempts timed out. Linux builds with gilrs may need libudev development files, as described in its documentation.

**Load neighborhood preview** combines seven selected Downtown static batches beside the original course, with 110 meshes and shared render/collision placement. **Place on mesh** also works for this preview. Thirteen supported diffuse payloads cover 54 meshes; 56 stay plain. **Loaded source details** shows selected batches and optional warnings. This is a scaled, partial preview; see [neighborhood scope](NEIGHBORHOOD.md).

**Skate neighborhood** enters the [source-scale gameplay slice](SOURCE_SKATING.md). It keeps original vertex distances, uses only imported surfaces, omits the synthetic course/rails, and recovers to a verified start after falling through missing geometry. Speed is shown in source units/s. **Test course** returns to the original prototype; R/Start restarts whichever mode is active.
