# SFIL v3 stream research

## Evidence and current results

This original Rust implementation is based on direct read-only observations of the user-supplied retail image. The census successfully indexed 4,385 archived `.xsf` members, containing 60,495 blocks and 60,412 segments with codec flag 1. Counts include repeated records across archives, not globally unique world objects. The census checks metadata; it does not decompress every block.

Decoder validation separately covered 32 sampled blocks: first and last blocks from the member with the most blocks in each of 16 archives containing nonempty streams. Every sampled decode completed with output lengths matching its segment descriptors. Additional initial Downtown samples produced decoded data beginning with `89 52 57 34` (RW4). This identifies a candidate container; no geometry or texture interpretation is implemented yet.

The readers and decoder contain no code from Skate 3 Recompiled. Game payloads and generated reports are kept out of the repository.

## Observed layout

Integers below are big-endian. A stream starts with a 128-byte header, `SFIL` at offset 0, version 3 at offset 4, and header size 128 at offset `0x10`. The six words at offsets `0x08` through `0x1C` are preserved without assigning unverified semantics. A member exactly 128 bytes long is a valid header-only stream.

Blocks follow sequentially. Each has a 128-byte header:

| Block-relative offset | Width | Reader interpretation |
|---|---:|---|
| `0x00`, `0x04` | 4 each | Uninterpreted identifier words |
| `0x08` | 4 | Payload length, excluding this header and trailing padding |
| `0x0C` | 4 | Header length, observed as 128 |
| `0x10` | 4 | Padded span including header, payload, and padding |

The padded span is a nonzero multiple of 128 and advances to the next block or the exact member end. A payload starts with an 80-byte control area. Two 32-byte segment descriptors begin at offsets 0 and `0x20` within that area; each starts with decoded length, stored length, and codec flags. Remaining control bytes are currently uninterpreted.

Segment bytes immediately follow the control area, in descriptor order. Their stored lengths plus 80 must equal the block payload length. Segment offsets in reports are relative to the SFIL member. To reach them in an ISO, add the disc archive offset and archive member offset. Outer archive padding is excluded from the exact member length used by this reader.

## Decode profile and limits

Observed codec flag 0 means raw bytes with equal stored and decoded lengths. Flag 1 is handled as RefPack with the five-byte `10 FB` header. Other flag values remain indexable but return an explicit error when decoding is requested. RefPack header variants and outer EB3 member compression are not handled by this path.

The decoder was written from the command-format description in [OpenReliant's RefPack documentation](https://github.com/OpenReliant/openreliant/blob/main/docs/formats/refpack.md), cross-checked against the observed headers. It implements literal runs, the three back-reference encodings, overlapping copies, and the final command. It requires exact input consumption and agreement between the RefPack header length, segment descriptor, and decoded output. No implementation source was copied.

The stream index permits at most 100,000 blocks per member. Input reads, individual stored segments, and the total decoded block each have a 64 MiB hard cap; callers can set a smaller decoded budget. RefPack's supported three-byte size field further limits each RefPack segment's declared output. All offsets are checked against the containing member before reads. Decoding returns bytes in memory and never writes game assets to disk.

## Validation and next work

Synthetic tests cover container-relative offsets, raw and compressed segment concatenation, header-only streams, malformed spans and lengths, unsupported codecs, RefPack literal and match commands, overlapping copies, invalid back-references, truncated input, output budgets, and trailing bytes. Fixtures contain only original synthetic data.

The [RW4 object-table reader](RW4.md) now indexes the observed decoded profile. Vertex layouts, index buffers, texture tiling, shader translation, and the relationships among simulation, presentation, and texture streams still need evidence. A successful block decode does not establish a playable world or complete format compatibility.
