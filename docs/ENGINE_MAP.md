# Engine and asset research

## Initial disc inventory

The original Rust directory reader scanned the selected retail profile image read-only. It found 103 files in 30 directories (excluding the root), totalling 6,404,940,920 bytes of file payload lengths. This sum excludes directory tables and disc padding. No game payloads are included in this repository.

| Filename extension | File count |
|---|---:|
| `.big` | 51 |
| `.rx2` | 23 |
| `.vp6` | 11 |
| `.mpf` | 3 |
| `.mus` | 3 |
| `.abin` | 2 |
| `.loc` | 2 |
| `.xex` | 2 |
| No extension | 2 |
| `.bh`, `.bin`, `.manifest`, `.mxb` | 1 each |

These are filename observations, not confirmed format identifications. Extensions alone do not establish an encoding, compression method, or runtime purpose.

Files named `worldDIST_DownTown.big`, `worldDIST_University.big`, and `worldDIST_Industrial.big` occur under `data/content`. Their names suggest world content; this remains an inference until their structures are examined. Other names under `data/audio` and `data/movies` provide candidates for later audio and video research.

## Next loader work

1. Completed: inspect archive headers and implement original, bounded EB3/BIG4 index readers with synthetic malformed-input checks.
2. Completed: locate archive members and read raw prefixes capped at 64 KiB in the library (32 bytes in the CLI).
3. Completed for the observed SFIL profile: index blocks and decode raw/RefPack segments into bounded memory. See [stream research](STREAMS.md).
4. Completed for a narrow profile: index RW4 dictionaries, resolve resources, and decode static float3/28-byte geometry and half-float UVs into a native depth-tested preview.
5. Completed for the initial sample: decode tiled BC1/RGB565 base levels, validate nine material associations, and bind four local diffuse textures to six meshes. The nine-batch neighborhood below extends those readers; further layouts, missing texture keys, original lighting, and full scene assembly remain open.

The initial archive scan successfully indexed 50 EB3 archives and one BIG4 archive, totalling 45,217 member records. Counts include all records across archives and are not a count of globally unique assets. Member paths in the Start Park archive include `.xsf` stream files; a sampled member begins with `SFIL`. This is a signature observation rather than a decoded world representation. See [archive details](ARCHIVES.md).

The SFIL census indexes 4,385 members and 60,495 blocks, with 60,412 segments marked with the observed RefPack flag. A separate decoder check passed 32 sampled blocks across 16 archives. Sample decoded payloads begin with the RW4 signature. The complete block corpus has not been decoded or compared against another implementation.

The directory, archive, stream, and RW4 readers support discovery and the initial in-memory decode path. The [skating prototype](GAMEPLAY.md) connects one imported geometry sample to simplified riding, wall contact, and six verified diffuse-texture bindings, alongside the original course. A separate lightmap preview is available. Complete game-world loading, original lighting, production collision, accurate gameplay reconstruction, and animation remain pending.

## Shared texture research

Read-only research also identified non-SFIL, RefPack-wrapped RW4 members in `data/big/miscboot.big` and `data/big/miscload.big`. A bounded in-memory check decoded 104 RX2 members from those two archives (31,323,540 decoded bytes) and found 318 texture descriptors. Examples in miscboot include `data/content/textures/globaltextures.rx2` (10 descriptors), `vitextures.rx2` (16), `particletextures.rx2`, `cubemap.rx2`, and `downtownlightmap.rx2`. Miscload includes model texture packs under `world/models/`. These families provide candidates for broader asset loading; their multi-asset definition tables are not yet integrated into the runtime sample loader.

The inspected shared-pack definition header is `[count, 20, 20 + 24 × count, 0, total_length]`. Each 24-byte row holds a name offset, observed type hash `0x9B0F1678`, 64-bit asset key, observed type hash `0xAC462E4A`, and descriptor object index. Unlike the sample's single hex-named definition, these packs use descriptive texture names and share resource objects across descriptors. Nonzero fetch base addresses, mixed formats 4/6/18/20, and packed mips need additional resource handling; they must not be decoded under the initial sample's single-resource assumptions.

The three unresolved diffuse keys in the initial presentation sample were absent from those decoded RX2 checks, all 45,217 archive member filenames, and the 23 standalone disc RX2 files. The standalone files are locator metadata arenas with no texture descriptors. A further bounded search of 200 nearby Downtown high-detail presentation/texture members examined 17,821 blocks and 17,247 texture arenas, without finding the three canonical names. The worldmisc/worlddmo stream checks also found none. These are negative findings within the inspected sets, not proof that the assets are absent from the game. Other quality tiers, shared/resident references, and additional world archives remain candidates. No payloads were exported or added to the repository.

## Directory reader reference and limits

The implementation extends this project's existing XDVDFS reader using the sector size, volume descriptor layout, and directory flag documented by [XboxDev](https://xboxdevwiki.net/XDVDFS). It contains no code from Skate 3 Recompiled.

Traversal rejects out-of-image extents, repeated directory sectors, repeated tree nodes, unsafe path components, and duplicate paths. Limits are 100,000 entries, 64 nested directory levels, 64 MiB per directory table, 128 MiB total directory-table reads, and 4,096 bytes per path. Results are sorted by disc path for reproducible reports. A limit failure returns an error rather than a partial inventory.

JSON schema version 1 records `source_status`, `file_count`, `directory_count`, `total_file_bytes`, and `entries`. Each entry contains `path`, `is_directory`, `size`, and `image_offset`. Offsets are absolute byte positions within the selected image; they include its game-partition base. Directory `size` values describe table lengths rather than recursive content sizes.

## Neighborhood assembly

The [neighborhood preview](NEIGHBORHOOD.md) now loads nine selected Downtown static batches: 112 meshes, 15,758 vertices, and 11,659 render triangles. Six exact vertex declarations cover strides 20/24/28/32/36; first UV attributes are half2 or half4. Adjacent source coordinates remain unchanged while merging and use one shared render/collision placement. Material group tables now accept verified zero padding to a 16-byte parameter boundary. Fifteen supported local diffuse payloads cover 57 meshes, including three meshes whose BC1 base levels precede a separately bounded mip region. The other 55 meshes remain plain, and two optional RGBA8 decoder warnings remain.

The two forward terrain batches use an already accepted 28-byte stride, extend native Z through 495.026, and match six complete border edges at each Z 300/400 seam. Source-scale collision retains 11,650 faces, while the expanded miniature retains 11,624 under its scale-dependent area threshold. A 120 Hz simulation from the unchanged native start crosses Z 300, 400, and 490 grounded after a known earlier drop and one landing, with no recovery. Source-scale skating also adds wall sliding, simplified head contacts, and camera clearance on these triangles. Other batches, complete scene placement/streaming, original lighting and collision, and animation remain open.
