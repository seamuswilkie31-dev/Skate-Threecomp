//! Bounded SFIL v3 block indexing based on direct observations of retail files.

use crate::decode_refpack;
use std::io::{Read, Seek, SeekFrom};

const HEADER_SIZE: u64 = 128;
const MAX_BLOCKS: usize = 100_000;
const MAX_DECODED: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSegment {
    /// Relative to the SFIL member, not the disc or archive.
    pub offset: u64,
    pub decoded_size: u32,
    pub stored_size: u32,
    /// Observed 0 = raw, 1 = RefPack. Other values remain indexable.
    pub codec_flags: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamBlock {
    pub offset: u64,
    pub identifier_words: [u32; 2],
    pub payload_size: u32,
    pub padded_span: u32,
    pub segments: [StreamSegment; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamIndex {
    pub length: u64,
    /// Uninterpreted big-endian words from header offsets 0x08 through 0x1C.
    pub header_words: [u32; 6],
    pub blocks: Vec<StreamBlock>,
}

impl StreamIndex {
    /// Inspect an exact raw member extent. Outer archive compression must be
    /// resolved first; callers must not pass an EB3 padding span as member length.
    pub fn read(reader: &mut (impl Read + Seek), base: u64, length: u64) -> Result<Self, String> {
        let end = base.checked_add(length).ok_or("SFIL region overflow.")?;
        if end
            > reader
                .seek(SeekFrom::End(0))
                .map_err(|error| error.to_string())?
        {
            return Err("SFIL region exceeds its source.".into());
        }
        let header = read_at(reader, base, length, 0, 128)?;
        if &header[..4] != b"SFIL" || be32(&header[4..8]) != 3 || be32(&header[16..20]) != 128 {
            return Err("Unsupported SFIL header (expected observed v3/128-byte profile).".into());
        }
        let header_words =
            std::array::from_fn(|index| be32(&header[8 + index * 4..12 + index * 4]));
        let mut blocks = Vec::new();
        let mut offset = HEADER_SIZE;
        while offset < length {
            if blocks.len() >= MAX_BLOCKS {
                return Err("SFIL block count exceeds the inspection budget.".into());
            }
            let bytes = read_at(reader, base, length, offset, 128)?;
            let payload_size = be32(&bytes[8..12]);
            let block_header_size = u64::from(be32(&bytes[12..16]));
            let padded_span = be32(&bytes[16..20]);
            let span = u64::from(padded_span);
            if block_header_size != HEADER_SIZE
                || payload_size < 80
                || span % 128 != 0
                || span < HEADER_SIZE + u64::from(payload_size)
                || offset.checked_add(span).is_none_or(|end| end > length)
            {
                return Err("SFIL block header, span, or payload bounds are invalid.".into());
            }
            let control = read_at(reader, base, length, offset + HEADER_SIZE, 80)?;
            let first_stored = be32(&control[4..8]);
            let second_stored = be32(&control[36..40]);
            if 80 + u64::from(first_stored) + u64::from(second_stored) != u64::from(payload_size) {
                return Err("SFIL segment lengths do not match the block payload.".into());
            }
            let segments = std::array::from_fn(|index| {
                let start = index * 32;
                StreamSegment {
                    offset: offset
                        + HEADER_SIZE
                        + 80
                        + if index == 0 {
                            0
                        } else {
                            u64::from(first_stored)
                        },
                    decoded_size: be32(&control[start..start + 4]),
                    stored_size: be32(&control[start + 4..start + 8]),
                    codec_flags: be32(&control[start + 8..start + 12]),
                }
            });
            blocks.push(StreamBlock {
                offset,
                identifier_words: [be32(&bytes[..4]), be32(&bytes[4..8])],
                payload_size,
                padded_span,
                segments,
            });
            offset += span;
        }
        Ok(Self {
            length,
            header_words,
            blocks,
        })
    }

    /// Decode one block into memory; no payload is written to disk. Segment
    /// sizes, RefPack header sizes, and actual decoded lengths must all agree.
    pub fn decode_block(
        &self,
        reader: &mut (impl Read + Seek),
        base: u64,
        block: usize,
        budget: usize,
    ) -> Result<Vec<u8>, String> {
        let block = self
            .blocks
            .get(block)
            .ok_or("SFIL block index is out of range.")?;
        let expected: u64 = block
            .segments
            .iter()
            .map(|segment| u64::from(segment.decoded_size))
            .sum();
        if expected > budget.min(MAX_DECODED) as u64 {
            return Err("SFIL block exceeds the decoded output budget.".into());
        }
        let mut output = Vec::with_capacity(expected as usize);
        for segment in &block.segments {
            if segment.stored_size as usize > MAX_DECODED {
                return Err("SFIL stored segment exceeds the input budget.".into());
            }
            let bytes = read_at(
                reader,
                base,
                self.length,
                segment.offset,
                segment.stored_size as usize,
            )?;
            let decoded = match segment.codec_flags {
                0 if segment.stored_size == segment.decoded_size => bytes,
                0 => return Err("Raw SFIL segment stored and decoded sizes disagree.".into()),
                1 => decode_refpack(&bytes, segment.decoded_size as usize)?,
                _ => return Err("Unsupported SFIL segment codec flags.".into()),
            };
            if decoded.len() != segment.decoded_size as usize {
                return Err("SFIL decoded segment length does not match its descriptor.".into());
            }
            output.extend_from_slice(&decoded);
        }
        Ok(output)
    }
}

fn read_at(
    reader: &mut (impl Read + Seek),
    base: u64,
    length: u64,
    offset: u64,
    size: usize,
) -> Result<Vec<u8>, String> {
    if offset
        .checked_add(size as u64)
        .is_none_or(|end| end > length)
        || size > MAX_DECODED
    {
        return Err("SFIL read exceeds its region or input budget.".into());
    }
    let position = base
        .checked_add(offset)
        .ok_or("SFIL read offset overflow.")?;
    let mut bytes = vec![0; size];
    reader
        .seek(SeekFrom::Start(position))
        .and_then(|_| reader.read_exact(&mut bytes))
        .map_err(|error| format!("Could not read SFIL data: {error}"))?;
    Ok(bytes)
}
fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 384];
        bytes[..4].copy_from_slice(b"SFIL");
        for (offset, value) in [
            (4, 3u32),
            (16, 128),
            (136, 91),
            (140, 128),
            (144, 256),
            (256, 4),
            (260, 4),
            (288, 1),
            (292, 7),
            (296, 1),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }
        bytes[336..340].copy_from_slice(b"TEST");
        bytes[340..347].copy_from_slice(&[0x10, 0xfb, 0, 0, 1, 0xfd, b'!']);
        bytes
    }
    #[test]
    fn indexes_and_decodes_raw_plus_refpack_inside_a_container() {
        let mut bytes = vec![0; 13];
        bytes.extend_from_slice(&fixture());
        bytes.extend_from_slice(b"neighbor");
        let mut reader = Cursor::new(bytes);
        let index = StreamIndex::read(&mut reader, 13, 384).unwrap();
        assert_eq!(index.blocks.len(), 1);
        assert_eq!(index.blocks[0].segments[1].offset, 340);
        assert_eq!(index.decode_block(&mut reader, 13, 0, 5).unwrap(), b"TEST!");
        assert!(index.decode_block(&mut reader, 13, 0, 4).is_err());
    }
    #[test]
    fn accepts_header_only_stream_and_rejects_truncation() {
        let mut empty = fixture()[..128].to_vec();
        assert!(
            StreamIndex::read(&mut Cursor::new(empty.clone()), 0, 128)
                .unwrap()
                .blocks
                .is_empty()
        );
        empty.pop();
        assert!(StreamIndex::read(&mut Cursor::new(empty), 0, 127).is_err());
        assert!(StreamIndex::read(&mut Cursor::new(fixture()), u64::MAX, 384).is_err());
    }
    #[test]
    fn rejects_bad_spans_segment_lengths_and_unsupported_codecs() {
        for (offset, value) in [
            (144, 0u32),
            (144, 512),
            (140, 64),
            (136, 92),
            (260, u32::MAX),
        ] {
            let mut bytes = fixture();
            bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            assert!(StreamIndex::read(&mut Cursor::new(bytes), 0, 384).is_err());
        }
        let mut bytes = fixture();
        bytes[296..300].copy_from_slice(&2u32.to_be_bytes());
        let mut reader = Cursor::new(bytes);
        let index = StreamIndex::read(&mut reader, 0, 384).unwrap();
        assert!(index.decode_block(&mut reader, 0, 0, 100).is_err());
    }
}
