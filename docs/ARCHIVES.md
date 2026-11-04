# Archive index readers

## Scope and evidence

This project's original Rust readers identify two layouts observed directly in the user-supplied retail-profile image: 50 `EB` version 3 files and one `BIG4` file. All 51 indexes were read successfully, with 45,217 member records in total. These counts include repeated assets across archives. No archive payloads are distributed here.

The header organization was cross-checked against the original format research in the [SSX Modding Wiki](https://ssx.computernewb.com/wiki/Formats/Common%3ABIG) and the [OpenSAGE BIG specification](https://github.com/OpenSAGE/Docs/blob/master/file-formats/big/index.rst). The implementation was written for this project from format descriptions and direct inspection. No implementation code from those projects or Skate 3 Recompiled was copied.

## EB version 3 profile

Observed integers use big-endian byte order. The fixed header and padding occupy 48 bytes. The reader uses these fields:

| Offset | Width | Meaning used by the reader |
|---|---:|---|
| `0x00` | 4 | `45 42 00 03` signature and version |
| `0x04` | 4 | Member count |
| `0x0A` | 1 | Offset alignment exponent |
| `0x0C` | 4 | Base header size / start of path records |
| `0x10` | 4 | Name-table size |
| `0x14` | 1 | Path-record width |
| `0x15` | 1 | Directory-record width |
| `0x16` | 2 | Directory count |
| `0x18` | 8 | Declared archive size |

Starting at `0x30`, each 16-byte member record contains an offset unit value, two size fields, and a name hash. Byte offsets are calculated by shifting the offset unit value by the alignment exponent. The two size fields are preserved as `compressed_size_field` and `size_field`; they are not sufficient here to prove a member's exact stored or decoded length across compression modes.

One flag byte per member follows the member records. Path and directory tables begin at successive 16-byte boundaries. Each path record has a big-endian directory index followed by a terminated filename; each directory record holds a terminated directory string. Layout calculations must match the declared table sizes before any slices are taken.

For EB3, `available_bytes` is the span from a member offset to the next distinct member offset, or to the archive end for the final member. It can include padding. Index order is preserved; members may be ordered by hash rather than by offset. Duplicate payload offsets are allowed, while duplicate normalized member paths are rejected. Compression flags and hashes are retained without claiming to verify their meanings or hash algorithm.

## BIG4 profile

The observed file uses a 16-byte header: signature, little-endian total size, big-endian member count, and big-endian index end. Each member record contains a big-endian byte offset and length, followed by a terminated name. For this layout, `available_bytes` is the recorded member length. Payloads may still have their own encoding or compression.

The reader deliberately supports the observed `BIG4` and EB3 profiles. Other BIG signatures and layout variants return errors until they have separate evidence and coverage.

## Boundaries and previews

Archives are bounded regions within the ISO, so archive-relative offsets never refer to the host filesystem. The reader checks the physical source bounds, declared archive size, metadata budget (64 MiB), member count (100,000), names, directory indexes, and payload offsets. Member paths normalize separators and reject traversal or absolute paths. The API never uses member paths to write host files.

`ArchiveIndex::read_member_prefix` reads at most 64 KiB of raw bytes inside the member's available span; the CLI uses 32 bytes. An EB3 preview may include padding. It does not decompress, extract, render, or execute the data.

Synthetic tests cover both layouts inside a larger source with a nonzero base, prefix boundaries, malformed names and indexes, invalid extents, excessive counts, overflow, and path traversal. Real-image validation covers every `.big` index and sample raw prefixes from both layouts.

## Next evidence needed

The sampled Start Park member `cPres_50_-50_high.xsf` begins with `SFIL`. Its structure, the role of external stream data, and compression modes need investigation before implementing a complete world-asset loader. GPU shader translation and geometry decoding are also still pending.
