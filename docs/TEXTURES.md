# Observed textures and pixel decoding

The original Rust reader recognizes the observed 52-byte RW4 texture record (`0x000200E8`). It checks its resource-object reference through the bounded arena reader. Six big-endian fetch words begin at descriptor byte 28. Encoded GPU addresses are never treated as host pointers or followed.

The non-stacked 2D metadata reader reports dimensions, pitch, format, byte order, tiling, packed-mip flag, resource size, and all six fetch words. Hardware fields were checked against [Xenia's Xenos definitions](https://github.com/xenia-project/xenia/blob/master/src/xenia/gpu/xenos.h). The specialized address bit maps were checked against [the documented hardware address layout](https://github.com/xenia-project/xenia/blob/master/src/xenia/gpu/texture_address.h), and BC1 palette rules against [Microsoft's block-compression documentation](https://learn.microsoft.com/en-us/windows/desktop/direct3d10/d3d10-graphics-programming-guide-resources-block-compression). These are format references; all implementation and fixtures here are original. No code from Skate 3 Recompiled is used.

## Verified base levels

All blocks below are zero-based in `data/content/worldDIST_DownTown.big`, member `data/content/world/stream/DIST_DownTown/cPres_550_150_high.xsf`.

| Block | Canonical texture key | Dimensions | Hardware format | Resource bytes | Current use |
|---:|---|---|---|---:|---|
| 0 | `163899680E898770` | 512 × 512 | 18 / BC1 | 131,072 | Lightmap preview |
| 4 | `00008CFE03E3870A` | 32 × 32 | 4 / RGB565 | 8,192 | Diffuse |
| 5 | `00008D0703E3870A` | 64 × 32 | 4 / RGB565 | 8,192 | Diffuse |
| 6 | `00008D1F03E3870A` | 32 × 32 | 4 / RGB565 | 8,192 | Diffuse |
| 7 | `2C70170A00050227` | 64 × 64 | 4 / RGB565 | 16,384 | Diffuse |

These profiles are tiled with byte-order mode 1 (swap bytes within each 16-bit word). The BC1 sample has no packed mips and uses identity fetch swizzle. Its decoded base level has 1,048,576 RGBA bytes, 4,215 distinct colors, and FNV-1a-64 checksum `E1EAE908FDFF7C68`. This checksum records an implementation regression value; it is not an independent proof of every pixel. The decoded atlas was also visually inspected locally.

The four RGB565 samples use observed fetch swizzle Z,Y,X,1. They set the packed-mip flag, but their smallest base dimension is at least 32, outside the packed tail. The distinction is documented in [the packed-level hardware notes](https://github.com/xenia-project/xenia/blob/master/src/xenia/gpu/texture_util.h#L69-L90). Only the base level is decoded; lower mip addresses are not followed. Resource padding is checked by each computed address, including the holes in a 32 × 32 tiled surface.

Texture keys are read from explicit `0x<16 hex digits>.Texture` names and compared with their numeric entries. An observed storage flag at bit 62 may be set in the numeric value. Other key bits are preserved. [Material associations](MATERIALS.md) link four local textures to six of the sample's nine meshes; three diffuse keys remain unresolved.

## Accepted profiles and limits

- BC1: power-of-two width and height from 128 through 4,096, matching pitch, tiled, byte-order mode 1, unsigned identity channels, no forced-alpha override, packed-mip flag, or nonzero base offsets.
- RGB565: power-of-two width and height from 32 through 4,096, matching pitch, tiled, byte-order mode 1, unsigned observed channel swizzle, zero base level and base resource offset. A packed-mip flag is accepted only because these base dimensions are outside the tail.
- RGBA output is at most 64 MiB per decoded image. Every input read must fit the checked resource. Unsupported profiles return an error.

The metadata reader also inspected a 256 × 256, format-18, packed-mip texture in `cTex_-400_-300_high.xsf` block 0. That observation remains metadata only: the strict BC1 pixel path rejects its packed flag. Packed base levels of 16 pixels or smaller, other formats, other swizzles, lower mips, sRGB behavior, and complete lighting are pending.

## Use and validation

In the prototype, **Load texture sample** opens the decoded 512 × 512 lightmap. **Show texture** reopens it after closing. The lighting atlas is not applied to diffuse UVs. **Load game mesh sample** loads the separately verified diffuse maps for six meshes in the native renderer. Decoded pixels stay in memory; no game assets are exported.

The inspector emits metadata or decoder summaries as JSON:

```powershell
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --texture-sample
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --decode-texture-sample
cargo run -p skate-threecomp-launcher --bin skate-threecomp-inspect -- "D:\Games\Skate 3.iso" --material-sample
```

`--texture-block <archive> <member> <block-index>` inspects an explicit block. Synthetic tests check descriptor bounds, key validation, BC1 palettes/transparency, independently tabulated tiled addresses, tile bijections, fixed color markers, byte order, and truncated padded resources. An opt-in real-image test checks the lightmap decoder; native GPU tests check diffuse layer selection, repeating UVs, depth/compositing, and the actual textured geometry sample. No retail payloads or decoded images are stored in this repository.
