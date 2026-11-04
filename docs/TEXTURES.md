# Observed texture metadata

The original Rust reader recognizes the observed 52-byte RW4 texture record (`0x000200E8`). It validates the encoded resource-object reference and resolves it through the existing bounded arena reader. Six big-endian fetch words begin at descriptor byte 28. Texture dimensions and flags are metadata only; resource length is not proof that every mip or pixel can be decoded.

Hardware field positions were cross-checked with [Xenia's Xenos hardware definitions](https://github.com/xenia-project/xenia/blob/master/src/xenia/gpu/xenos.h). Only the hardware data layout was used as a reference; the reader was implemented independently. No implementation code was copied, and no code from Skate 3 Recompiled is used.

The supported metadata profile is a non-stacked 2D fetch. It reports width and height from the two 13-bit size-minus-one fields, pitch from the row-pitch field, the six-bit format ID, two-bit byte-order mode, tiling flag, and packed-mip flag. It preserves all six fetch words for investigation. Encoded GPU addresses are never treated as host pointers or followed.

## Actual image observations

| Research member, block 0 | Width × height | Format ID | Endian mode | Tiled | Packed mips | Resource bytes |
|---|---|---:|---:|---|---|---:|
| `cPres_550_150_high.xsf` | 512 × 512 | 18 (`0x12`, hardware BC1/DXT1) | 1 | Yes | No | 131,072 |
| `cTex_-400_-300_high.xsf` | 256 × 256 | 18 (`0x12`, hardware BC1/DXT1) | 1 | Yes | Yes | 65,536 |

Both members are in `data/content/worldDIST_DownTown.big`, under `data/content/world/stream/DIST_DownTown/`. The renderer still uses a plain material: tiling conversion, byte swapping, block decompression, mip-tail layout, swizzles, and material-to-texture bindings must be verified before pixels are applied to geometry. A metadata match alone does not establish the rendered appearance.

`--texture-sample` inspects the fixed presentation member's block 0. `--texture-block <archive> <member> <block-index>` inspects an explicit decoded block. Both emit JSON without writing game assets. Unsupported descriptor lengths, resource links, dimensions, and stacked profiles return errors.

```powershell
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --texture-sample
```

Synthetic tests check field extraction and malformed links/profile fields; the two real-image observations above were inspected successfully. No pixel correctness claim is made yet.
