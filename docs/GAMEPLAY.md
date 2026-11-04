# Original skating prototype

Open **Open skating prototype** in the launcher. For a direct start, run:

```powershell
cargo run -p skate-threecomp-launcher -- --prototype
```

The prototype runs in the native launcher window with the existing `eframe`/`wgpu` renderer. Its original course, skater placeholder, and movement code require no game assets or ISO. This is an early development environment for the new Rust gameplay systems. Skate 3's world, tricks, animation, physics behavior, and saves have not been reconstructed.

| Input | Action |
|---|---|
| W / Up | Push and accelerate |
| S / Down | Brake |
| A / Left, D / Right | Steer |
| Space | Ollie while grounded |
| R | Reset position and statistics |
| Escape | Return to launcher |

The course contains a two-metre ramp, a platform, a descending slope, boundary markers, and speed-reference stripes. The camera follows behind the skater. Speed, grounded state, landing count, and last airtime are displayed. Losing window focus pauses the simulation.

## Movement and rendering scope

Movement advances at a fixed 120 Hz. Frame time is bounded to avoid long catch-up loops after a pause. The prototype models forward acceleration, rolling resistance, braking, speed-dependent steering, a vertical ollie impulse, gravity, ramp contact, and stopping at course bounds. It is an original tuning model rather than measured Skate 3 behavior. It uses a point contact against a height surface, not full skateboard rigid-body or swept collision physics.

The course and placeholder skater are perspective-projected triangles submitted through egui's native wgpu mesh renderer. Near-plane clipping and face sorting provide an initial view without a separate graphics dependency. A depth-buffered world renderer, lighting, textures, board animation, and proper complex collision remain needed for game content. Controller input, grinds, flips, bails, and trick scoring are not implemented yet.

Automated movement checks cover push/brake/steering, an ollie returning to ground with one landing, ramp following, and course boundaries. The native executable was built on Windows. Desktop capture timed out during manual visual validation, so appearance and keyboard interaction in the running window still need a manual check.
