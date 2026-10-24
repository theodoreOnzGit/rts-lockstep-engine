// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/DataChunk.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/DataChunk.cpp (the output
//   side: DataChunkOutput, DataChunkTableOfContents::write)
//   GeneralsMD/Code/GameEngine/Include/Common/MapReaderWriterInfo.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The writing side of the chunk file format of maps ([`super::data_chunk`]
//! reads it): chunks opened and closed in a stack, a table of contents that
//! fills as names are used, and the file assembled when the writer is
//! finished.
//!
//! Upstream's `DataChunkOutput` writes the body to a temporary file
//! (`<user data path>/_tmpChunk.dat`) and, when destroyed, writes the table
//! of contents to the real output stream and then copies the body after it.
//! Here the body is a byte vector, [`DataChunkOutput::finish`] returns the
//! assembled file, and nothing touches a file system (wasm has none).
//!
//! # Where this differs from upstream
//!
//! Nowhere in what it writes: the layout is that of the C++ writer
//! (`tests/differential_sides_scripts.rs` compares whole files byte for
//! byte). A name longer than 255 bytes has its length written modulo 256
//! (`unsigned char len = getLength()`) and a string longer than 65,535 its
//! length modulo 65,536 (`UnsignedShort`) with that many bytes, as upstream.
//! `closeDataChunk` with no chunk open returns without doing anything
//! (upstream: "TODO: Throw exception").
//!
//! # What is left out
//!
//! `writeArrayOfBytes` is the only primitive upstream has and this port
//! lacks none; the stream interface (`OutputStream`) is the returned vector.

use super::data_chunk::{DataChunkTableOfContents, DataChunkVersionType, TOC_TAG};
use crate::common::dict::{DataType, Dict};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};

/// `class OutputChunk`: an open chunk.
#[derive(Clone, Copy, Debug)]
struct OutputChunk {
    /// Where the chunk's data size is stored (`filepos`).
    filepos: i32,
}

/// `class DataChunkOutput`.
#[derive(Clone, Debug)]
pub struct DataChunkOutput {
    /// `m_tmp_file`: the body so far.
    body: Vec<u8>,
    /// `m_contents`.
    contents: DataChunkTableOfContents,
    /// `m_chunkStack`, the innermost last.
    chunk_stack: Vec<OutputChunk>,
}

impl Default for DataChunkOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl DataChunkOutput {
    /// `DataChunkOutput(pOut)`: nothing written yet.
    pub fn new() -> Self {
        DataChunkOutput {
            body: Vec::new(),
            contents: DataChunkTableOfContents::new(),
            chunk_stack: Vec::new(),
        }
    }

    /// `openDataChunk(name, ver)`: the chunk's id (allocated in the table
    /// of contents), its version, and a placeholder size of `0xffff`.
    pub fn open_data_chunk(&mut self, name: &[u8], ver: DataChunkVersionType) {
        let id = self.contents.allocate_id(name);
        self.body.extend_from_slice(&id.to_le_bytes());
        self.body.extend_from_slice(&ver.to_le_bytes());
        self.chunk_stack.push(OutputChunk {
            filepos: self.body.len() as i32,
        });
        self.body.extend_from_slice(&0xffffi32.to_le_bytes());
    }

    /// `closeDataChunk()`: store the size of the chunk (not counting the
    /// size field) and pop it. Nothing happens with no chunk open.
    pub fn close_data_chunk(&mut self) {
        let Some(c) = self.chunk_stack.pop() else {
            return;
        };
        let here = self.body.len() as i32;
        let size = here - c.filepos - 4;
        let at = c.filepos as usize;
        self.body[at..at + 4].copy_from_slice(&size.to_le_bytes());
    }

    /// `writeReal`.
    pub fn write_real(&mut self, r: f32) {
        self.body.extend_from_slice(&r.to_le_bytes());
    }

    /// `writeInt`.
    pub fn write_int(&mut self, i: i32) {
        self.body.extend_from_slice(&i.to_le_bytes());
    }

    /// `writeByte` (`Byte` is a signed `char`).
    pub fn write_byte(&mut self, b: i8) {
        self.body.push(b as u8);
    }

    /// `writeArrayOfBytes(ptr, len)`.
    pub fn write_array_of_bytes(&mut self, bytes: &[u8]) {
        self.body.extend_from_slice(bytes);
    }

    /// `writeAsciiString`: the length as an `UnsignedShort`, then that many
    /// bytes.
    pub fn write_ascii_string(&mut self, s: &[u8]) {
        let len = s.len() as u16;
        self.body.extend_from_slice(&len.to_le_bytes());
        self.body.extend_from_slice(&s[..usize::from(len)]);
    }

    /// `writeUnicodeString`: the length in units as an `UnsignedShort`, then
    /// that many 16-bit units.
    pub fn write_unicode_string(&mut self, s: &[u16]) {
        let len = s.len() as u16;
        self.body.extend_from_slice(&len.to_le_bytes());
        for u in &s[..usize::from(len)] {
            self.body.extend_from_slice(&u.to_le_bytes());
        }
    }

    /// The `keyAndType` of a name: its id in the table of contents shifted
    /// left 8, with the type in the low byte.
    fn key_and_type(&mut self, key: NameKeyType, t: i32, names: &NameKeyGenerator) -> i32 {
        let id = self.contents.allocate_id(names.key_to_name(key));
        ((id as i32) << 8) | (t & 0xff)
    }

    /// `writeNameKey(key)`: the key's name goes to the table of contents; an
    /// `Int` of its id and the type `DICT_ASCIISTRING` is written.
    pub fn write_name_key(&mut self, key: NameKeyType, names: &NameKeyGenerator) {
        let v = self.key_and_type(key, DataType::AsciiString as i32, names);
        self.write_int(v);
    }

    /// `writeDict(d)`: the pair count (`UnsignedShort`), then per pair its
    /// `keyAndType` (the name allocated in the table of contents before the
    /// value is written) and its value.
    pub fn write_dict(&mut self, d: &Dict, names: &NameKeyGenerator) {
        let len = d.get_pair_count() as u16;
        self.body.extend_from_slice(&len.to_le_bytes());
        for i in 0..i32::from(len) {
            let t = d.get_nth_type(i);
            let v = self.key_and_type(d.get_nth_key(i), t as i32, names);
            self.write_int(v);
            match t {
                DataType::Bool => {
                    self.write_byte(i8::from(d.try_get_nth_bool(i).expect("pair in range")))
                }
                DataType::Int => self.write_int(d.try_get_nth_int(i).expect("pair in range")),
                DataType::Real => self.write_real(d.try_get_nth_real(i).expect("pair in range")),
                DataType::AsciiString => {
                    self.write_ascii_string(&d.try_get_nth_ascii_string(i).expect("pair in range"))
                }
                DataType::UnicodeString => self
                    .write_unicode_string(&d.try_get_nth_unicode_string(i).expect("pair in range")),
                DataType::None => {} // upstream: DEBUG_CRASH("impossible"), nothing written
            }
        }
    }

    /// How many chunks are open.
    pub fn open_chunks(&self) -> usize {
        self.chunk_stack.len()
    }

    /// The destructor: the table of contents (`write`: `"CkMp"`, the number
    /// of names, then each as length byte, bytes and id, the newest first)
    /// followed by the body. Chunks still open keep the placeholder size.
    pub fn finish(self) -> Vec<u8> {
        let mut out = TOC_TAG.to_vec();
        let entries: Vec<_> = self.contents.mappings().collect();
        out.extend_from_slice(&(entries.len() as i32).to_le_bytes());
        for m in entries {
            let len = m.name.len() as u8;
            out.push(len);
            out.extend_from_slice(&m.name[..usize::from(len)]);
            out.extend_from_slice(&m.id.to_le_bytes());
        }
        out.extend_from_slice(&self.body);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::data_chunk::{
        CachedFileInputStream, ChunkResult, DataChunkInfo, DataChunkInput,
    };

    #[derive(Default)]
    struct Seen {
        names: NameKeyGenerator,
        out: Vec<String>,
    }

    fn parse_a(
        file: &mut DataChunkInput<Seen>,
        info: &DataChunkInfo,
        s: &mut Seen,
    ) -> ChunkResult<bool> {
        let i = file.read_int()?;
        let str = file.read_ascii_string()?;
        let k = file.read_name_key(&mut s.names)?;
        let d = file.read_dict(&mut s.names)?;
        s.out.push(format!(
            "{} v{} size{} int{} {} key={} dict={}",
            String::from_utf8_lossy(&info.label),
            info.version,
            info.data_size,
            i,
            String::from_utf8_lossy(&str),
            String::from_utf8_lossy(s.names.key_to_name(k)),
            d.get_pair_count()
        ));
        Ok(true)
    }

    /// What the writer writes is what the reader reads back: nested chunks,
    /// sizes patched on close, names in the table of contents.
    #[test]
    fn written_file_reads_back() {
        let mut names = NameKeyGenerator::new();
        names.init();
        let key = names.name_to_key(b"alpha");
        let key2 = names.name_to_key(b"beta");
        let mut d = Dict::new();
        d.set_int(key, 7).unwrap();
        d.set_ascii_string(key2, b"x").unwrap();
        let mut w = DataChunkOutput::new();
        w.open_data_chunk(b"A", 3);
        w.write_int(-4);
        w.write_ascii_string(b"hey");
        w.write_name_key(key2, &names);
        w.write_dict(&d, &names);
        assert_eq!(w.open_chunks(), 1);
        w.close_data_chunk();
        w.close_data_chunk(); // nothing open: ignored, as upstream
        assert_eq!(w.open_chunks(), 0);
        let bytes = w.finish();
        assert_eq!(&bytes[..4], b"CkMp");

        let mut seen = Seen::default();
        seen.names.init();
        let mut input = DataChunkInput::new(CachedFileInputStream::new(bytes).unwrap()).unwrap();
        input.register_parser(b"A", b"", parse_a);
        assert!(input.parse(&mut seen).unwrap());
        assert_eq!(
            seen.out,
            vec!["A v3 size30 int-4 hey key=beta dict=2".to_string()]
        );
    }

    /// The table lists the newest name first and the sizes count what a
    /// chunk holds, nested chunks included; the primitives have upstream's
    /// widths.
    #[test]
    fn layout_and_primitives() {
        let mut w = DataChunkOutput::new();
        w.open_data_chunk(b"Out", 1);
        w.open_data_chunk(b"In", 2);
        w.write_real(1.5);
        w.write_byte(-1);
        w.write_array_of_bytes(&[9, 8]);
        w.write_unicode_string(&[0x41, 0x42]);
        w.close_data_chunk();
        w.close_data_chunk();
        let b = w.finish();
        let mut want = b"CkMp".to_vec();
        want.extend_from_slice(&2i32.to_le_bytes());
        want.extend_from_slice(&[2, b'I', b'n', 2, 0, 0, 0]);
        want.extend_from_slice(&[3, b'O', b'u', b't', 1, 0, 0, 0]);
        // Out: id 1 version 1 size = In header (10) + 4 + 1 + 2 + 2 + 4 = 23
        want.extend_from_slice(&[1, 0, 0, 0, 1, 0, 23, 0, 0, 0]);
        want.extend_from_slice(&[2, 0, 0, 0, 2, 0, 13, 0, 0, 0]);
        want.extend_from_slice(&1.5f32.to_le_bytes());
        want.extend_from_slice(&[0xff, 9, 8, 2, 0, 0x41, 0, 0x42, 0]);
        assert_eq!(b, want);
    }

    /// A chunk left open keeps upstream's placeholder size; an over-long
    /// string has its length truncated to 16 bits.
    #[test]
    fn open_chunk_and_long_string() {
        let mut w = DataChunkOutput::new();
        w.open_data_chunk(b"X", 0);
        w.write_ascii_string(&vec![b'a'; 65_537]);
        let b = w.finish();
        let body = &b[b.len() - (10 + 2 + 1)..];
        assert_eq!(&body[6..10], &0xffffi32.to_le_bytes());
        assert_eq!(&body[10..12], &1u16.to_le_bytes());
        assert_eq!(body[12], b'a');
    }
}
