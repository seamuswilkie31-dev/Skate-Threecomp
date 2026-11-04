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
4. Completed for the observed profile: index RW4 dictionaries and resolve resource-region offsets. Object-specific geometry and texture decoding remain next.

The initial archive scan successfully indexed 50 EB3 archives and one BIG4 archive, totalling 45,217 member records. Counts include all records across archives and are not a count of globally unique assets. Member paths in the Start Park archive include `.xsf` stream files; a sampled member begins with `SFIL`. This is a signature observation rather than a decoded world representation. See [archive details](ARCHIVES.md).

The SFIL census indexes 4,385 members and 60,495 blocks, with 60,412 segments marked with the observed RefPack flag. A separate decoder check passed 32 sampled blocks across 16 archives. Sample decoded payloads begin with the RW4 signature. The complete block corpus has not been decoded or compared against another implementation.

The directory, archive, stream, and RW4 readers support discovery and the initial in-memory decode path. The original [skating prototype](GAMEPLAY.md) provides basic movement, ollies, ramp contact, and a rendered test course. Game-world loading, production rendering, accurate gameplay reconstruction, and animation remain pending.

## Directory reader reference and limits

The implementation extends this project's existing XDVDFS reader using the sector size, volume descriptor layout, and directory flag documented by [XboxDev](https://xboxdevwiki.net/XDVDFS). It contains no code from Skate 3 Recompiled.

Traversal rejects out-of-image extents, repeated directory sectors, repeated tree nodes, unsafe path components, and duplicate paths. Limits are 100,000 entries, 64 nested directory levels, 64 MiB per directory table, 128 MiB total directory-table reads, and 4,096 bytes per path. Results are sorted by disc path for reproducible reports. A limit failure returns an error rather than a partial inventory.

JSON schema version 1 records `source_status`, `file_count`, `directory_count`, `total_file_bytes`, and `entries`. Each entry contains `path`, `is_directory`, `size`, and `image_offset`. Offsets are absolute byte positions within the selected image; they include its game-partition base. Directory `size` values describe table lengths rather than recursive content sizes.
