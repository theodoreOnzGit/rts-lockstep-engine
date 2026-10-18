// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/DataChunk.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/DataChunk.cpp (the input
//   side: CachedFileInputStream's reads, DataChunkTableOfContents,
//   DataChunkInput)
//   GeneralsMD/Code/GameEngine/Include/Common/MapReaderWriterInfo.h
//   (and, for readDict, the set semantics of
//   GeneralsMD/Code/GameEngine/Source/Common/Dict.cpp)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The chunk file format of maps (and of some other data): a table of
//! contents mapping chunk names to integer ids, then nested chunks, read
//! through registered per-chunk parsers. Reads from an in-memory byte buffer
//! (wasm has no file system).
//!
//! # The format, as upstream reads it
//!
//! All integers little-endian.
//!
//! ```text
//! "CkMp", count (Int), count times: len (u8), len bytes of name, id (UnsignedInt)
//! then chunks:  id (UnsignedInt), version (UnsignedShort), dataSize (Int),
//!               dataSize bytes of data (values, or nested chunks)
//! ```
//!
//! [`DataChunkInput::parse`] reads chunks until the end of the file (or of
//! the enclosing chunk), looks each chunk's name up in the table and calls
//! the newest registered parser with that name and the enclosing chunk's
//! name as scope (`""` at the top), then skips whatever the parser left.
//! A parser reads values with `readInt`, `readReal`, ... and may call
//! `parse` again for nested chunks. Every read is counted against every open
//! chunk, so reading past a chunk's end makes its `dataLeft` negative (the
//! check is a debug assertion upstream).
//!
//! # What is left out
//!
//! - **Decompression.** `CachedFileInputStream::open` decompresses data that
//!   `CompressionManager` recognises (RefPack, Nox LZH, zlib, BTree, Huff;
//!   most codecs are missing from the released source). This takes bytes
//!   that are already uncompressed.
//! - The writer, `DataChunkOutput` (it goes through a temporary file), and
//!   `DataChunkTableOfContents::write`, `allocateID`'s use by it.
//! - `m_currentObject` and `m_userData` (`void*` hooks for parsers): a Rust
//!   parser gets `&mut C`. The `userData` given to `registerParser` is not
//!   kept: upstream never reads it (each parser gets `parse`'s argument).
//! - `NameKeyType`: upstream's `TheNameKeyGenerator` is process-wide, so
//!   `readNameKey` and `readDict` take it as `names` (the caller owns it, and
//!   one generator can serve several readers). Keys come from
//!   [`NameKeyGenerator::try_name_to_key`] in first-seen order, and a
//!   [`Dict`] (`common::dict`) is keyed and sorted by them as upstream's is.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`ChunkError::Undefined`]: an `Int`, `Real`, `Byte` or string length read
//! that the end of the file cuts off (they are uninitialised locals; the
//! table of contents' `count`, and its first name length, likewise); a
//! negative byte count for `readArrayOfBytes` (a `memcpy` of a negative
//! size); a byte count whose end overflows `Int`; more bytes than the
//! caller's buffer holds; a `dataLeft` that overflows `Int`; and a name key
//! past `0x7fffffff` ([`ChunkUndefined::NameKeysExhausted`]).
//!
//! # Upstream quirks that are kept
//!
//! - `parse` stops opening chunks inside a chunk with fewer than 4 bytes
//!   left (`CHUNK_HEADER_BYTES` is 4, but a chunk header is 10 bytes, so a
//!   chunk with 4 to 9 bytes left reads its "sub-chunk" header from the
//!   bytes after it).
//! - When opening a chunk reaches the end of the file, `parse` returns
//!   without closing it: it stays on the chunk stack, and the enclosing
//!   parser's `closeDataChunk` pops it instead of its own chunk. A parser
//!   that returns false leaves its chunk open the same way.
//! - The table of contents counts as valid only with at least one name and
//!   with data after it (`m_headerOpened = count > 0 && !eof()`); otherwise
//!   `parse` returns false at once. A wrong tag leaves it empty, not an
//!   error ("may happen with legacy files").
//! - Names are looked up newest first; an unknown id has the name `""`.
//! - A table of contents whose count runs past the end of the file adds the
//!   missing entries as `("", 0)` (zero-filled, nothing read); upstream
//!   allocates each of them, here they are counted
//!   ([`DataChunkTableOfContents::eof_mappings`]).
//! - Strings are C strings: an `AsciiString` or `UnicodeString` read ends at
//!   its first 0, and a string cut off by the end of the file keeps the
//!   zero-filled rest of its buffer.
//! - `readDict` throws `ERROR_OUT_OF_MEMORY` for more than 32767 pairs
//!   (before reading them) and `ERROR_CORRUPT_FILE_FORMAT` for an unknown
//!   type; the exception passes through `parse` to its caller.

use crate::common::dict::{Dict, DictError};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};

/// `typedef unsigned short DataChunkVersionType`.
pub type DataChunkVersionType = u16;
/// `DataChunkInput::CHUNK_HEADER_BYTES` (the header is in fact 10 bytes).
pub const CHUNK_HEADER_BYTES: i32 = 4;
/// The table-of-contents tag.
pub const TOC_TAG: &[u8; 4] = b"CkMp";
/// `ERROR_OUT_OF_MEMORY` (Common/Errors.h).
pub const ERROR_OUT_OF_MEMORY: u32 = 0xdead_0002;
/// `ERROR_CORRUPT_FILE_FORMAT` (Common/Errors.h).
pub const ERROR_CORRUPT_FILE_FORMAT: u32 = 0xdead_0005;
/// `Dict::MAX_LEN`.
pub const DICT_MAX_LEN: u16 = crate::common::dict::MAX_LEN as u16;

/// Where upstream is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChunkUndefined {
    /// A name key past `0x7fffffff` (`NameKeyGenerator`'s next key).
    NameKeysExhausted,
    /// A value read only partly, or not at all, before its use.
    UninitialisedRead,
    /// A read of a negative number of bytes (`memcpy` of a negative size).
    NegativeRead,
    /// `numBytes + m_pos` overflows `Int`.
    ReadOverflow,
    /// More bytes than the destination buffer holds.
    BufferOverrun,
    /// A chunk's `dataLeft` overflows `Int`.
    DataLeftOverflow,
}

/// A chunk-reading error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkError {
    /// Upstream throws this `ErrorCode` (from `readDict`).
    Thrown(u32),
    /// Upstream is undefined here.
    Undefined(ChunkUndefined),
}

impl From<ChunkUndefined> for ChunkError {
    fn from(u: ChunkUndefined) -> Self {
        ChunkError::Undefined(u)
    }
}

impl std::fmt::Display for ChunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChunkError::Thrown(c) => write!(f, "chunk file error {c:#010x}"),
            ChunkError::Undefined(u) => write!(f, "chunk file outside what the original reader defines: {u:?}"),
        }
    }
}

impl std::error::Error for ChunkError {}

/// Result of the chunk functions.
pub type ChunkResult<T> = Result<T, ChunkError>;

fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `class CachedFileInputStream`, over bytes already in memory (and already
/// decompressed).
#[derive(Clone, Debug)]
pub struct CachedFileInputStream {
    buffer: Vec<u8>,
    pos: i32,
}

impl CachedFileInputStream {
    /// The stream over `data` (`open` without the file system or
    /// decompression). `None` for empty data, where `open` returns false.
    pub fn new(data: Vec<u8>) -> Option<Self> {
        if data.is_empty() || i32::try_from(data.len()).is_err() {
            return None;
        }
        Some(CachedFileInputStream { buffer: data, pos: 0 })
    }

    fn size(&self) -> i32 {
        self.buffer.len() as i32
    }

    /// `read(pData, numBytes)` into `dst`: clamps to what is left, returns
    /// the count read.
    pub fn read(&mut self, dst: &mut [u8], num_bytes: i32) -> ChunkResult<i32> {
        let mut n = num_bytes;
        let end = n.checked_add(self.pos).ok_or(ChunkUndefined::ReadOverflow)?;
        if end > self.size() {
            n = self.size() - self.pos;
        }
        if n != 0 {
            if n < 0 {
                return Err(ChunkUndefined::NegativeRead.into());
            }
            if n as usize > dst.len() {
                return Err(ChunkUndefined::BufferOverrun.into());
            }
            let p = self.pos as usize;
            dst[..n as usize].copy_from_slice(&self.buffer[p..p + n as usize]);
            self.pos += n;
        }
        Ok(n)
    }

    /// `tell`.
    pub fn tell(&self) -> u32 {
        self.pos as u32
    }

    /// `absoluteSeek(pos)`: clamped to the size.
    pub fn absolute_seek(&mut self, pos: u32) -> bool {
        let p = pos.min(self.size() as u32);
        self.pos = p as i32;
        true
    }

    /// `eof`: at the end exactly.
    pub fn eof(&self) -> bool {
        self.size() == self.pos
    }

    /// `rewind`.
    pub fn rewind(&mut self) {
        self.pos = 0;
    }

    /// A read of exactly `N` bytes into an uninitialised value.
    fn read_exact<const N: usize>(&mut self) -> ChunkResult<[u8; N]> {
        let mut b = [0u8; N];
        if self.read(&mut b, N as i32)? != N as i32 {
            return Err(ChunkUndefined::UninitialisedRead.into());
        }
        Ok(b)
    }
}

/// `class Mapping`: a name and its id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
    pub name: Vec<u8>,
    pub id: u32,
}

/// `class DataChunkTableOfContents`.
#[derive(Clone, Debug)]
pub struct DataChunkTableOfContents {
    /// The mappings in the order they were added (upstream prepends, so
    /// lookups go from the end).
    list: Vec<Mapping>,
    /// Mappings that `read` added at the end of the file, all `("", 0)`
    /// (newer than `list`), counted rather than stored.
    eof_mappings: u64,
    list_length: i32,
    next_id: u32,
    header_opened: bool,
}

impl Default for DataChunkTableOfContents {
    fn default() -> Self {
        Self::new()
    }
}

impl DataChunkTableOfContents {
    /// `DataChunkTableOfContents()`.
    pub fn new() -> Self {
        DataChunkTableOfContents { list: Vec::new(), eof_mappings: 0, list_length: 0, next_id: 1, header_opened: false }
    }

    /// `findMapping(name)`: newest first.
    fn find_mapping(&self, name: &[u8]) -> Option<Mapping> {
        if self.eof_mappings > 0 && name.is_empty() {
            return Some(Mapping { name: Vec::new(), id: 0 });
        }
        self.list.iter().rev().find(|m| m.name == name).cloned()
    }

    /// `getID(name)`: 0 if unknown.
    pub fn get_id(&self, name: &[u8]) -> u32 {
        self.find_mapping(name).map_or(0, |m| m.id)
    }

    /// `getName(id)`: `""` if unknown.
    pub fn get_name(&self, id: u32) -> Vec<u8> {
        if self.eof_mappings > 0 && id == 0 {
            return Vec::new();
        }
        self.list.iter().rev().find(|m| m.id == id).map_or_else(Vec::new, |m| m.name.clone())
    }

    /// `allocateID(name)`: the existing id, or a new one.
    pub fn allocate_id(&mut self, name: &[u8]) -> u32 {
        if let Some(m) = self.find_mapping(name) {
            return m.id;
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.list.push(Mapping { name: name.to_vec(), id });
        self.list_length = self.list_length.wrapping_add(1);
        id
    }

    /// `isOpenedForRead`.
    pub fn is_opened_for_read(&self) -> bool {
        self.header_opened
    }

    /// How many mappings `read` added at the end of the file, each `("", 0)`
    /// (a count the file does not hold; upstream allocates every one, up to
    /// 2^31 for a damaged count, which the Rust only counts).
    pub fn eof_mappings(&self) -> u64 {
        self.eof_mappings
    }

    /// The mappings, newest first (`m_list` order).
    pub fn mappings(&self) -> impl Iterator<Item = &Mapping> {
        self.list.iter().rev()
    }

    /// `read(stream)`: append the table at the stream's position.
    pub fn read(&mut self, s: &mut CachedFileInputStream) -> ChunkResult<()> {
        let mut tag = *b"xxxx";
        s.read(&mut tag, 4)?;
        if &tag != TOC_TAG {
            return Ok(()); // "Don't throw, may happen with legacy files."
        }
        let count = i32::from_le_bytes(s.read_exact::<4>()?);
        let mut max_id = 0u32;
        let mut len: Option<u8> = None;
        let mut i = 0;
        while i < count {
            if s.eof() && len.is_some() {
                // every remaining mapping reads nothing: ("", 0), zero-filled
                let rest = (count - i) as u64;
                self.eof_mappings += rest;
                self.list_length = self.list_length.wrapping_add(count - i);
                break;
            }
            let mut b = [0u8; 1];
            if s.read(&mut b, 1)? == 1 {
                len = Some(b[0]);
            }
            let l = len.ok_or(ChunkUndefined::UninitialisedRead)?;
            let mut name = Vec::new();
            if l > 0 {
                let mut buf = vec![0u8; usize::from(l)];
                s.read(&mut buf, i32::from(l))?;
                name = c_str(&buf).to_vec();
            }
            let mut id = [0u8; 4]; // the zero-filled Mapping keeps unread bytes 0
            s.read(&mut id, 4)?;
            let id = u32::from_le_bytes(id);
            self.list.push(Mapping { name, id });
            self.list_length = self.list_length.wrapping_add(1);
            if id > max_id {
                max_id = id;
            }
            i += 1;
        }
        self.header_opened = count > 0 && !s.eof();
        self.next_id = self.next_id.max(max_id.wrapping_add(1));
        Ok(())
    }
}

/// `class InputChunk`: an open chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputChunk {
    pub id: u32,
    pub version: DataChunkVersionType,
    pub chunk_start: i32,
    pub data_size: i32,
    pub data_left: i32,
}

/// `struct DataChunkInfo`: what a parser is told about its chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataChunkInfo {
    pub label: Vec<u8>,
    pub parent_label: Vec<u8>,
    pub version: DataChunkVersionType,
    pub data_size: i32,
}

/// `DataChunkParserPtr`: parse one chunk; `false` stops the parse.
pub type DataChunkParser<C> = fn(&mut DataChunkInput<C>, &DataChunkInfo, &mut C) -> ChunkResult<bool>;

/// `class UserParser` (without its unused `userData`).
pub struct UserParser<C> {
    pub parser: DataChunkParser<C>,
    pub label: Vec<u8>,
    pub parent_label: Vec<u8>,
}

/// `nameToKey(name)` on the caller's generator. Its only failure is a key
/// past `0x7fffffff`, undefined upstream (`NameKeyError::KeysExhausted`).
fn name_to_key(names: &mut NameKeyGenerator, name: &[u8]) -> ChunkResult<NameKeyType> {
    names.try_name_to_key(name).map_err(|_| ChunkError::Undefined(ChunkUndefined::NameKeysExhausted))
}

/// A `Dict` setter's failure: `ERROR_OUT_OF_MEMORY`, the throw of
/// `Dict::Dict(n)` and of the setters past `MAX_LEN` pairs. `readDict` has
/// already refused more than `MAX_LEN` pairs, so this is not reached.
fn dict_error(_: DictError) -> ChunkError {
    ChunkError::Thrown(ERROR_OUT_OF_MEMORY)
}

/// `class DataChunkInput`, with parsers that fill a `C`.
pub struct DataChunkInput<C> {
    file: CachedFileInputStream,
    contents: DataChunkTableOfContents,
    filepos_of_first_chunk: u32,
    /// In registration order (upstream prepends: lookups go from the end).
    parsers: Vec<UserParser<C>>,
    /// Open chunks, the innermost last.
    chunk_stack: Vec<InputChunk>,
}

impl<C> DataChunkInput<C> {
    /// `DataChunkInput(stream)`: reads the table of contents.
    pub fn new(mut file: CachedFileInputStream) -> ChunkResult<Self> {
        let mut contents = DataChunkTableOfContents::new();
        contents.read(&mut file)?;
        let filepos_of_first_chunk = file.tell();
        Ok(DataChunkInput { file, contents, filepos_of_first_chunk, parsers: Vec::new(), chunk_stack: Vec::new() })
    }

    /// The table of contents.
    pub fn contents(&self) -> &DataChunkTableOfContents {
        &self.contents
    }

    /// `registerParser(label, parentLabel, parser)`: `parentLabel` `""` is
    /// the top level.
    pub fn register_parser(&mut self, label: &[u8], parent_label: &[u8], parser: DataChunkParser<C>) {
        self.parsers.push(UserParser { parser, label: label.to_vec(), parent_label: parent_label.to_vec() });
    }

    /// `parse(userData)`: parse chunks from here to the end of the file or
    /// of the open chunk (see the module docs).
    pub fn parse(&mut self, user: &mut C) -> ChunkResult<bool> {
        if !self.contents.is_opened_for_read() {
            return Ok(false);
        }
        let parent_label = match self.chunk_stack.last() {
            Some(c) => self.contents.get_name(c.id),
            None => Vec::new(),
        };
        while !self.at_end_of_file() {
            if let Some(top) = self.chunk_stack.last() {
                if top.data_left < CHUNK_HEADER_BYTES {
                    break;
                }
            }
            let (label, ver) = self.open_data_chunk()?;
            if self.at_end_of_file() {
                break;
            }
            let found = self
                .parsers
                .iter()
                .rev()
                .find(|p| p.label == label && p.parent_label == parent_label)
                .map(|p| p.parser);
            if let Some(parser) = found {
                let info = DataChunkInfo {
                    label,
                    parent_label: parent_label.clone(),
                    version: ver,
                    data_size: self.get_chunk_data_size() as i32,
                };
                if !parser(self, &info, user)? {
                    return Ok(false);
                }
            }
            self.close_data_chunk()?;
        }
        Ok(true)
    }

    /// `reset`: back to the first chunk, no chunk open.
    pub fn reset(&mut self) {
        self.chunk_stack.clear();
        self.file.absolute_seek(self.filepos_of_first_chunk);
    }

    /// `isValidFileType`.
    pub fn is_valid_file_type(&self) -> bool {
        self.contents.is_opened_for_read()
    }

    /// `decrementDataLeft(size)` on every open chunk.
    fn decrement_data_left(&mut self, size: i32) -> ChunkResult<()> {
        for c in &mut self.chunk_stack {
            c.data_left = c.data_left.checked_sub(size).ok_or(ChunkUndefined::DataLeftOverflow)?;
        }
        Ok(())
    }

    /// `openDataChunk(&ver)`: read a chunk header and push the chunk; its
    /// name (`""` at the end of the file) and version.
    pub fn open_data_chunk(&mut self) -> ChunkResult<(Vec<u8>, DataChunkVersionType)> {
        // the chunk's fields start at 0; partial reads keep the zero bytes
        let mut id = [0u8; 4];
        self.file.read(&mut id, 4)?;
        self.decrement_data_left(4)?;
        let mut version = [0u8; 2];
        self.file.read(&mut version, 2)?;
        self.decrement_data_left(2)?;
        let mut size = [0u8; 4];
        self.file.read(&mut size, 4)?;
        self.decrement_data_left(4)?;
        let c = InputChunk {
            id: u32::from_le_bytes(id),
            version: u16::from_le_bytes(version),
            chunk_start: self.file.tell() as i32,
            data_size: i32::from_le_bytes(size),
            data_left: i32::from_le_bytes(size),
        };
        self.chunk_stack.push(c);
        if self.at_end_of_file() {
            return Ok((Vec::new(), c.version));
        }
        Ok((self.contents.get_name(c.id), c.version))
    }

    /// `closeDataChunk`: skip the rest of the open chunk and pop it.
    pub fn close_data_chunk(&mut self) -> ChunkResult<()> {
        let Some(&top) = self.chunk_stack.last() else {
            return Ok(());
        };
        if top.data_left > 0 {
            self.file.absolute_seek(self.file.tell().wrapping_add(top.data_left as u32));
            self.decrement_data_left(top.data_left)?;
        }
        self.chunk_stack.pop();
        Ok(())
    }

    /// `atEndOfFile`.
    pub fn at_end_of_file(&self) -> bool {
        self.file.eof()
    }

    /// `atEndOfChunk`: no data left in the open chunk (true if none is open).
    pub fn at_end_of_chunk(&self) -> bool {
        self.chunk_stack.last().is_none_or(|c| c.data_left <= 0)
    }

    /// `getChunkLabel` (`""` if no chunk is open).
    pub fn get_chunk_label(&self) -> Vec<u8> {
        self.chunk_stack.last().map_or_else(Vec::new, |c| self.contents.get_name(c.id))
    }
    /// `getChunkVersion` (0 if no chunk is open).
    pub fn get_chunk_version(&self) -> DataChunkVersionType {
        self.chunk_stack.last().map_or(0, |c| c.version)
    }
    /// `getChunkDataSize` (0 if no chunk is open).
    pub fn get_chunk_data_size(&self) -> u32 {
        self.chunk_stack.last().map_or(0, |c| c.data_size as u32)
    }
    /// `getChunkDataSizeLeft` (0 if no chunk is open).
    pub fn get_chunk_data_size_left(&self) -> u32 {
        self.chunk_stack.last().map_or(0, |c| c.data_left as u32)
    }

    /// `readReal`.
    pub fn read_real(&mut self) -> ChunkResult<f32> {
        let b = self.file.read_exact::<4>()?;
        self.decrement_data_left(4)?;
        Ok(f32::from_le_bytes(b))
    }

    /// `readInt`.
    pub fn read_int(&mut self) -> ChunkResult<i32> {
        let b = self.file.read_exact::<4>()?;
        self.decrement_data_left(4)?;
        Ok(i32::from_le_bytes(b))
    }

    /// `readByte` (`Byte` is a signed `char`).
    pub fn read_byte(&mut self) -> ChunkResult<i8> {
        let b = self.file.read_exact::<1>()?;
        self.decrement_data_left(1)?;
        Ok(b[0] as i8)
    }

    /// `readArrayOfBytes(ptr, len)` into `dst`; bytes past the end of the
    /// file keep their values.
    pub fn read_array_of_bytes(&mut self, dst: &mut [u8], len: i32) -> ChunkResult<()> {
        self.file.read(dst, len)?;
        self.decrement_data_left(len)
    }

    fn read_len(&mut self) -> ChunkResult<u16> {
        let b = self.file.read_exact::<2>()?;
        self.decrement_data_left(2)?;
        Ok(u16::from_le_bytes(b))
    }

    /// `readAsciiString`: length (`UnsignedShort`) and bytes, as a C string.
    pub fn read_ascii_string(&mut self) -> ChunkResult<Vec<u8>> {
        let len = self.read_len()?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let mut buf = vec![0u8; usize::from(len)];
        self.file.read(&mut buf, i32::from(len))?;
        self.decrement_data_left(i32::from(len))?;
        Ok(c_str(&buf).to_vec())
    }

    /// `readUnicodeString`: length and 16-bit units, up to the first 0.
    pub fn read_unicode_string(&mut self) -> ChunkResult<Vec<u16>> {
        let len = self.read_len()?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let mut buf = vec![0u8; usize::from(len) * 2];
        self.file.read(&mut buf, i32::from(len) * 2)?;
        self.decrement_data_left(i32::from(len) * 2)?;
        let units: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let n = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        Ok(units[..n].to_vec())
    }

    /// `readNameKey`: an `Int` whose upper 24 bits are a table id; the key
    /// of that id's name in `names` (`nameToKey`).
    pub fn read_name_key(&mut self, names: &mut NameKeyGenerator) -> ChunkResult<NameKeyType> {
        let key_and_type = self.read_int()?;
        let name = self.contents.get_name((key_and_type >> 8) as u32);
        name_to_key(names, &name)
    }

    /// `readDict`: a count (`UnsignedShort`), then per pair an `Int` of
    /// table id (upper 24 bits) and type (low 8), and the value. The key of
    /// each pair is taken before its value is read, as upstream does.
    pub fn read_dict(&mut self, names: &mut NameKeyGenerator) -> ChunkResult<Dict> {
        let len = self.read_len()?;
        if len > DICT_MAX_LEN {
            return Err(ChunkError::Thrown(ERROR_OUT_OF_MEMORY));
        }
        let mut d = Dict::with_preallocated(i32::from(len)).map_err(dict_error)?;
        for _ in 0..len {
            let key_and_type = self.read_int()?;
            let t = key_and_type & 0xff;
            let name = self.contents.get_name((key_and_type >> 8) as u32);
            let k = name_to_key(names, &name)?;
            let set = match t {
                0 => d.set_bool(k, self.read_byte()? != 0),
                1 => d.set_int(k, self.read_int()?),
                2 => d.set_real(k, self.read_real()?),
                3 => d.set_ascii_string(k, &self.read_ascii_string()?),
                4 => d.set_unicode_string(k, &self.read_unicode_string()?),
                _ => return Err(ChunkError::Thrown(ERROR_CORRUPT_FILE_FORMAT)),
            };
            set.map_err(dict_error)?;
        }
        Ok(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::dict::DictValue;

    fn w(v: &mut Vec<u8>, x: &[u8]) {
        v.extend_from_slice(x);
    }

    fn file() -> Vec<u8> {
        let mut v = b"CkMp".to_vec();
        w(&mut v, &2i32.to_le_bytes());
        for (name, id) in [(&b"Outer"[..], 1u32), (b"Inner", 2)] {
            v.push(name.len() as u8);
            w(&mut v, name);
            w(&mut v, &id.to_le_bytes());
        }
        let mut inner = Vec::new();
        w(&mut inner, &2u32.to_le_bytes());
        w(&mut inner, &7u16.to_le_bytes());
        w(&mut inner, &8i32.to_le_bytes());
        w(&mut inner, &(-5i32).to_le_bytes());
        w(&mut inner, &1.5f32.to_le_bytes());
        w(&mut v, &1u32.to_le_bytes());
        w(&mut v, &3u16.to_le_bytes());
        w(&mut v, &(inner.len() as i32).to_le_bytes());
        w(&mut v, &inner);
        v
    }

    #[derive(Default)]
    struct Out {
        seen: Vec<(Vec<u8>, Vec<u8>, u16, i32)>,
        values: (i32, f32),
    }

    fn outer(f: &mut DataChunkInput<Out>, info: &DataChunkInfo, out: &mut Out) -> ChunkResult<bool> {
        out.seen.push((info.label.clone(), info.parent_label.clone(), info.version, info.data_size));
        f.parse(out)
    }

    fn inner(f: &mut DataChunkInput<Out>, info: &DataChunkInfo, out: &mut Out) -> ChunkResult<bool> {
        out.seen.push((info.label.clone(), info.parent_label.clone(), info.version, info.data_size));
        out.values = (f.read_int()?, f.read_real()?);
        Ok(f.at_end_of_chunk())
    }

    #[test]
    fn nested_chunks() {
        let mut input = DataChunkInput::new(CachedFileInputStream::new(file()).unwrap()).unwrap();
        assert!(input.is_valid_file_type());
        input.register_parser(b"Outer", b"", outer);
        input.register_parser(b"Inner", b"Outer", inner);
        let mut out = Out::default();
        assert_eq!(input.parse(&mut out), Ok(true));
        assert_eq!(out.seen, vec![(b"Outer".to_vec(), Vec::new(), 3, 18), (b"Inner".to_vec(), b"Outer".to_vec(), 7, 8)]);
        assert_eq!(out.values, (-5, 1.5));
        assert!(input.at_end_of_file());
        input.reset();
        assert!(!input.at_end_of_file());
    }

    #[test]
    fn toc_rules() {
        let mut s = CachedFileInputStream::new(b"CkMp\x00\x00\x00\x00rest".to_vec()).unwrap();
        let mut t = DataChunkTableOfContents::new();
        t.read(&mut s).unwrap();
        assert!(!t.is_opened_for_read()); // no names
        let mut s = CachedFileInputStream::new(b"CkMp\x05".to_vec()).unwrap();
        assert_eq!(t.read(&mut s), Err(ChunkError::Undefined(ChunkUndefined::UninitialisedRead)));
        let a = NameKeyType(1);
        let mut d = Dict::new();
        d.set_int(a, 1).unwrap();
        d.set_bool(a, true).unwrap();
        assert_eq!(d.get_pair_count(), 1);
        assert_eq!(d.get(a), Some(&DictValue::Bool(true)));
    }

    /// `readDict` and `readNameKey` with a table of contents of four names:
    /// the keys come from the generator in first-seen order, and the pairs
    /// are filled in upstream's `Dict` (here `common::dict`).
    #[derive(Default)]
    struct DictOut {
        names: NameKeyGenerator,
        dict: Option<Dict>,
        key: Option<NameKeyType>,
    }

    fn dict_parser(f: &mut DataChunkInput<DictOut>, _: &DataChunkInfo, out: &mut DictOut) -> ChunkResult<bool> {
        out.dict = Some(f.read_dict(&mut out.names)?);
        out.key = Some(f.read_name_key(&mut out.names)?);
        Ok(true)
    }

    #[test]
    fn read_dict_and_name_key_fill_common_dict() {
        let mut v = b"CkMp".to_vec();
        w(&mut v, &4i32.to_le_bytes());
        for (name, id) in [(&b"Ints"[..], 1u32), (b"Text", 2), (b"Real", 3), (b"Outer", 4)] {
            v.push(name.len() as u8);
            w(&mut v, name);
            w(&mut v, &id.to_le_bytes());
        }
        let mut data = Vec::new();
        w(&mut data, &3u16.to_le_bytes());
        w(&mut data, &((1 << 8) | 1i32).to_le_bytes());
        w(&mut data, &7i32.to_le_bytes());
        w(&mut data, &((2 << 8) | 3i32).to_le_bytes());
        w(&mut data, &3u16.to_le_bytes());
        w(&mut data, b"abc");
        w(&mut data, &((3 << 8) | 2i32).to_le_bytes());
        w(&mut data, &1.5f32.to_le_bytes());
        w(&mut data, &((1 << 8) | 1i32).to_le_bytes()); // the name key that follows
        w(&mut v, &4u32.to_le_bytes());
        w(&mut v, &3u16.to_le_bytes());
        w(&mut v, &(data.len() as i32).to_le_bytes());
        w(&mut v, &data);
        let mut input = DataChunkInput::new(CachedFileInputStream::new(v).unwrap()).unwrap();
        input.register_parser(b"Outer", b"", dict_parser);
        let mut out = DictOut::default();
        out.names.init();
        assert_eq!(input.parse(&mut out), Ok(true));
        let (ints, text, real) = (out.names.name_to_key(b"Ints"), out.names.name_to_key(b"Text"), out.names.name_to_key(b"Real"));
        assert_eq!((ints, text, real), (NameKeyType(1), NameKeyType(2), NameKeyType(3)));
        assert_eq!(out.names.key_to_name(text), b"Text");
        let d = out.dict.expect("read_dict ran");
        assert_eq!(d.get_pair_count(), 3);
        assert_eq!(d.get_int(ints), (7, true));
        assert_eq!(d.get_ascii_string(text), (b"abc".to_vec(), true));
        assert_eq!(d.get_real(real), (1.5, true));
        assert_eq!(out.key, Some(ints));
    }
}
