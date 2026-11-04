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

1. Inspect archive headers from the user-supplied image and record signatures, byte order, and index bounds.
2. Implement an original archive index reader against observed structures. Use synthetic buffers for malformed offset and size checks.
3. Read an individual asset through the disc and archive indexes with explicit size limits.
4. Identify geometry and texture formats before connecting them to a native Rust renderer.

The directory inventory is a discovery tool. Runtime loading, world rendering, physics, animation, and gameplay are still pending.

## Directory reader reference and limits

The implementation extends this project's existing XDVDFS reader using the sector size, volume descriptor layout, and directory flag documented by [XboxDev](https://xboxdevwiki.net/XDVDFS). It contains no code from Skate 3 Recompiled.

Traversal rejects out-of-image extents, repeated directory sectors, repeated tree nodes, unsafe path components, and duplicate paths. Limits are 100,000 entries, 64 nested directory levels, 64 MiB per directory table, 128 MiB total directory-table reads, and 4,096 bytes per path. Results are sorted by disc path for reproducible reports. A limit failure returns an error rather than a partial inventory.

JSON schema version 1 records `source_status`, `file_count`, `directory_count`, `total_file_bytes`, and `entries`. Each entry contains `path`, `is_directory`, `size`, and `image_offset`. Offsets are absolute byte positions within the selected image; they include its game-partition base. Directory `size` values describe table lengths rather than recursive content sizes.
