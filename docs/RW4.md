# Observed Xbox 360 RW4 dictionary profile

The original Rust reader accepts the `89 52 57 34` signature, Xbox 360 platform tag, big-endian 32-bit pointer profile, and observed `454`/`000` version strings. The general header and six-word dictionary structure were cross-checked against [Dmitri's original RW4 format research](https://soccergaming.com/forums/threads/fifa-11-pc-file-formats-resarch-renderware-4-5-assets.6468020/). No implementation source was copied, including from Skate 3 Recompiled.

Direct observations from the inspected game establish the following profile. The object count and used count occur at `0x20` and `0x24`; the dictionary pointer occurs at `0x30`. Each 24-byte record supplies a data offset, a currently zero word, a size, alignment, type-table index, and type ID. The parser preserves type IDs without assigning unverified game-specific meanings.

Five size/alignment pairs beginning at `0x44` describe the metadata region followed by four resource regions. Their sizes must total the concatenated decoded SFIL block length. Dictionary records with type IDs `0x10030` through `0x10033` use offsets relative to the corresponding resource region after metadata. Other records in the observed samples use metadata-relative offsets. The resolved byte offset is reported separately from the original offset field; alignment is checked on the original field.

The dictionary must fit inside metadata. Object extents must fit inside the region to which they resolve. Resource and object alignments must be powers of two. Counts are capped at 100,000 and decoded arena size at 64 MiB. Unsupported platform/version fields and nonzero second dictionary words return errors. Type indices, pointer fixups, and object contents are not interpreted or executed.

Real-image validation successfully indexed 32 sampled arenas across 16 archives, totalling 1,519 object records. This extends the prior first/last-block sample set; it does not establish compatibility with every arena in the disc. Synthetic tests cover region resolution and malformed dictionary counts, extents, and alignments. No game payloads are stored in the repository.

The next asset work is to identify vertex/index descriptors and texture records, decode their platform-specific layouts, and connect them to a world renderer. The original gameplay prototype develops movement independently while that research continues.
