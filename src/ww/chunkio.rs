// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/chunkio.h
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/chunkio.cpp (the reader:
//   ChunkHeader, MicroChunkHeader, ChunkLoadClass)
//   (and the file the game reads W3D data through: GameFileClass::Read,
//   Seek and Tell, from GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/
//   GameClient/W3DFileSystem.cpp and WWLib/WWFILE.H, over the RAMFile::read
//   and RAMFile::seek of GeneralsMD/Code/GameEngine/Source/Common/System/
//   RAMFile.cpp)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWLib "Tiberian Sun / Commando / G Library",
//   chunkio by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The Westwood chunk file reader (`ChunkLoadClass`), which every W3D file
//! (meshes, hierarchies, animations, ...) is read through, over an
//! in-memory byte buffer (wasm has no file system).
//!
//! # The format, as upstream reads it
//!
//! ```text
//! chunk:       ChunkType (uint32), ChunkSize (uint32), ChunkSize & 0x7FFFFFFF bytes
//!              (the MSB of ChunkSize is the "contains sub-chunks" flag)
//! micro-chunk: ChunkType (uint8), ChunkSize (uint8), ChunkSize bytes
//! ```
//!
//! Little-endian. A chunk holds data or further chunks; a micro-chunk lives
//! inside a chunk's data. The reader keeps a stack of open chunks (at most
//! 256) with the number of bytes consumed in each, and refuses reads past
//! the end of the current chunk (and of the current micro-chunk).
//!
//! # The file underneath
//!
//! In the game a W3D file comes from a `.big` archive as a `RAMFile` behind a
//! `GameFileClass`, so [`GameFile`] has their semantics: `Read` copies what is
//! left (a short read still copies its bytes) and moves the position by the
//! count it returns, so a negative length moves it **backwards** without
//! copying; `Seek` clamps the new position to the file; `Tell` is `Seek(0)`,
//! which therefore also clamps a negative position back to 0.
//!
//! # Upstream quirks that are kept
//!
//! - `Open_Chunk` stops when the parent's consumed count **equals** its
//!   size, so a parent overrun by a child that claimed more than the parent
//!   holds keeps opening "children" from the bytes after it.
//! - Opening a chunk does not count its 8-byte header against the parent;
//!   `Close_Chunk` adds the child's size plus 8.
//! - The bounds checks are `uint32` sums, `PositionStack + nbytes >
//!   (int)Size` with both sides converted to unsigned: a length near 2^32
//!   wraps the sum and passes, and is then given to the file as a negative
//!   `int` (which moves the file position back and "succeeds").
//! - `Contains_Chunks` returns the flag bit itself as an `int`, so a chunk
//!   with sub-chunks gives `i32::MIN`, not 1.
//! - `Read(IOVector2Struct*)`, `Read(IOVector3Struct*)`,
//!   `Read(IOVector4Struct*)` and `Read(IOQuaternionStruct*)` read
//!   `sizeof(v)` bytes, the size of the **pointer**: 4 on the Win32 target the
//!   game was built for, so only the first component is read (the
//!   `read_vector*` methods here). Built for 64 bits, upstream would read 8;
//!   these four are therefore not compared with the C++ (built for x86_64
//!   here), only unit-tested.
//! - `Close_Micro_Chunk` adds the bytes it skips to the chunk's count even
//!   when the seek was clamped at the end of the file; with no micro-chunk
//!   open it still closes the last one again (its assert is compiled out).
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`ChunkIoError::Undefined`]: a chunk query, read, seek or close with no
//! chunk open (`HeaderStack[-1]`); opening a 257th nested chunk (a write past
//! the 256-entry stacks); a read that would copy more bytes than the
//! destination holds; a read that would copy from a negative file position
//! (left by a negative read). Upstream's asserts are compiled out in the
//! release build, as in the shipped game, so everything else they guard is
//! defined and reproduced.
//!
//! # Where upstream does not end
//!
//! A length of 2^31 or more that passes the wrapping bounds check reaches
//! the file as a negative `int` and moves the position **backwards**; a
//! loop over sub-chunks (as every W3D loader has) can then read the same
//! chunks forever. Once the position has moved backwards, more than
//! [`HANG_OPEN_LIMIT`] (2^20) further `Open_Chunk` calls give
//! [`ChunkIoError::NonTerminating`] instead of looping on: an input that
//! made upstream open that many chunks after rewinding and still end would
//! be reported wrongly, which needs files far beyond any game asset.
//!
//! # What is left out
//!
//! `ChunkSaveClass` (the writer) and the `READ_*`/`WRITE_*` helper macros.

/// `ChunkLoadClass::MAX_STACK_DEPTH`.
pub const MAX_STACK_DEPTH: usize = 256;
/// `sizeof(ChunkHeader)`.
pub const CHUNK_HEADER_SIZE: u32 = 8;
/// `sizeof(MicroChunkHeader)`.
pub const MICRO_CHUNK_HEADER_SIZE: u32 = 2;
/// `sizeof(void*)` on the Win32 target (see the module docs).
pub const WIN32_POINTER_SIZE: u32 = 4;

/// Where upstream is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChunkIoUndefined {
    /// A chunk operation with no chunk open (`HeaderStack[StackIndex-1]`
    /// with `StackIndex == 0`).
    NoChunkOpen,
    /// A 257th nested chunk (past the end of the 256-entry stacks).
    StackOverflow,
    /// More bytes than the destination buffer holds.
    BufferOverrun,
    /// A copy from a negative file position.
    ReadBeforeStart,
}

/// A chunk-reading error: where upstream is undefined, or does not end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkIoError {
    /// Upstream is undefined here.
    Undefined(ChunkIoUndefined),
    /// The file position has moved backwards and more than
    /// [`HANG_OPEN_LIMIT`] chunks have been opened since: upstream re-reads
    /// the same chunks and does not end (see the module docs).
    NonTerminating,
}

/// See [`ChunkIoError::NonTerminating`].
pub const HANG_OPEN_LIMIT: u64 = 1 << 20;

impl From<ChunkIoUndefined> for ChunkIoError {
    fn from(u: ChunkIoUndefined) -> Self {
        ChunkIoError::Undefined(u)
    }
}

impl std::fmt::Display for ChunkIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChunkIoError::Undefined(u) => write!(
                f,
                "chunk file outside what the original reader defines: {u:?}"
            ),
            ChunkIoError::NonTerminating => {
                write!(f, "chunk file on which the original reader loops forever")
            }
        }
    }
}

impl std::error::Error for ChunkIoError {}

/// Result of the chunk functions.
pub type ChunkIoResult<T> = Result<T, ChunkIoError>;

/// `SEEK_SET`, `SEEK_CUR`, `SEEK_END` (`GameFileClass::Seek` maps any other
/// value to `SEEK_CUR`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekFrom {
    Set,
    Cur,
    End,
}

/// The file a W3D is read through: `GameFileClass` over a `RAMFile` holding
/// `data` (see the module docs).
#[derive(Clone, Debug)]
pub struct GameFile<'a> {
    data: &'a [u8],
    pos: i32,
    rewound: bool,
}

impl<'a> GameFile<'a> {
    /// The file over `data`. `None` if it is larger than an `Int` can size.
    pub fn new(data: &'a [u8]) -> Option<Self> {
        i32::try_from(data.len()).ok()?;
        Some(GameFile {
            data,
            pos: 0,
            rewound: false,
        })
    }

    fn size(&self) -> i32 {
        self.data.len() as i32
    }

    /// The current position (`m_pos`; may be negative after a negative read).
    pub fn position(&self) -> i32 {
        self.pos
    }

    /// `Read(buffer, len)` (`RAMFile::read`): copies `min(len, left)` bytes
    /// into `dst` if that is positive, moves the position by that count
    /// (backwards for a negative `len`), returns it.
    pub fn read(&mut self, dst: &mut [u8], len: i32) -> ChunkIoResult<i32> {
        let left = self.size().wrapping_sub(self.pos);
        let n = if len > left { left } else { len };
        if n > 0 {
            if self.pos < 0 {
                return Err(ChunkIoUndefined::ReadBeforeStart.into());
            }
            if n as usize > dst.len() {
                return Err(ChunkIoUndefined::BufferOverrun.into());
            }
            let p = self.pos as usize;
            dst[..n as usize].copy_from_slice(&self.data[p..p + n as usize]);
        }
        let new_pos = self.pos.wrapping_add(n);
        self.rewound |= new_pos < self.pos;
        self.pos = new_pos;
        Ok(n)
    }

    /// `Seek(pos, dir)` (`RAMFile::seek`): the new position clamped to
    /// `0..=size`, returned.
    pub fn seek(&mut self, pos: i32, dir: SeekFrom) -> i32 {
        let new_pos = match dir {
            SeekFrom::Set => pos,
            SeekFrom::Cur => self.pos.wrapping_add(pos),
            SeekFrom::End => self.size().wrapping_add(pos),
        };
        let new_pos = new_pos.clamp(0, self.size());
        self.rewound |= new_pos < self.pos;
        self.pos = new_pos;
        self.pos
    }

    /// Whether the position has ever moved backwards (a negative read or
    /// seek length, or `Tell` clamping a negative position).
    pub fn has_rewound(&self) -> bool {
        self.rewound
    }

    /// `Tell()`: `Seek(0)` (`SEEK_CUR`).
    pub fn tell(&mut self) -> i32 {
        self.seek(0, SeekFrom::Cur)
    }

    /// `Size()`.
    pub fn file_size(&self) -> i32 {
        self.size()
    }
}

/// `struct ChunkHeader`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkHeader {
    pub chunk_type: u32,
    /// The size, with the "contains sub-chunks" flag in the MSB.
    pub chunk_size: u32,
}

impl ChunkHeader {
    /// `Get_Type()`.
    pub fn get_type(&self) -> u32 {
        self.chunk_type
    }
    /// `Get_Size()`: without the flag.
    pub fn get_size(&self) -> u32 {
        self.chunk_size & 0x7FFF_FFFF
    }
    /// `Get_Sub_Chunk_Flag()`: the flag bit, as the `int` upstream returns.
    pub fn get_sub_chunk_flag(&self) -> i32 {
        (self.chunk_size & 0x8000_0000) as i32
    }
    fn to_bytes(self) -> [u8; 8] {
        let mut b = [0u8; 8];
        b[..4].copy_from_slice(&self.chunk_type.to_le_bytes());
        b[4..].copy_from_slice(&self.chunk_size.to_le_bytes());
        b
    }
    fn from_bytes(b: [u8; 8]) -> Self {
        ChunkHeader {
            chunk_type: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            chunk_size: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        }
    }
}

/// `struct MicroChunkHeader`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MicroChunkHeader {
    pub chunk_type: u8,
    pub chunk_size: u8,
}

/// `class ChunkLoadClass`, over a [`GameFile`].
#[derive(Clone, Debug)]
pub struct ChunkLoad<'a> {
    file: GameFile<'a>,
    stack_index: usize,
    position_stack: Vec<u32>,
    header_stack: Vec<ChunkHeader>,
    in_micro_chunk: bool,
    micro_chunk_position: i32,
    mc_header: MicroChunkHeader,
    opens_since_rewind: u64,
}

impl<'a> ChunkLoad<'a> {
    /// `ChunkLoadClass(file)`, with the stacks zeroed.
    pub fn new(file: GameFile<'a>) -> Self {
        ChunkLoad {
            file,
            stack_index: 0,
            position_stack: vec![0; MAX_STACK_DEPTH],
            header_stack: vec![ChunkHeader::default(); MAX_STACK_DEPTH],
            in_micro_chunk: false,
            micro_chunk_position: 0,
            mc_header: MicroChunkHeader::default(),
            opens_since_rewind: 0,
        }
    }

    /// The file underneath.
    pub fn file(&self) -> &GameFile<'a> {
        &self.file
    }

    /// The file underneath, mutably (upstream callers share the `FileClass`).
    pub fn file_mut(&mut self) -> &mut GameFile<'a> {
        &mut self.file
    }

    fn top(&self) -> ChunkIoResult<usize> {
        self.stack_index
            .checked_sub(1)
            .ok_or(ChunkIoUndefined::NoChunkOpen.into())
    }

    /// `Open_Chunk()`.
    pub fn open_chunk(&mut self) -> ChunkIoResult<bool> {
        if self.file.has_rewound() {
            self.opens_since_rewind += 1;
            if self.opens_since_rewind > HANG_OPEN_LIMIT {
                return Err(ChunkIoError::NonTerminating);
            }
        }
        // if the parent chunk has been completely eaten, return false
        if self.stack_index > 0 {
            let t = self.stack_index - 1;
            if self.position_stack[t] == self.header_stack[t].get_size() {
                return Ok(false);
            }
        }
        // read the chunk header (a short read still lands in the slot)
        let mut b = [0u8; 8];
        let would_copy = self.file.size().wrapping_sub(self.file.pos) > 0;
        if self.stack_index >= MAX_STACK_DEPTH {
            if would_copy {
                return Err(ChunkIoUndefined::StackOverflow.into());
            }
            return Ok(false);
        }
        b.copy_from_slice(&self.header_stack[self.stack_index].to_bytes());
        let n = self.file.read(&mut b, CHUNK_HEADER_SIZE as i32)?;
        self.header_stack[self.stack_index] = ChunkHeader::from_bytes(b);
        if n != CHUNK_HEADER_SIZE as i32 {
            return Ok(false);
        }
        self.position_stack[self.stack_index] = 0;
        self.stack_index += 1;
        Ok(true)
    }

    /// `Close_Chunk()`.
    pub fn close_chunk(&mut self) -> ChunkIoResult<bool> {
        let t = self.top()?;
        let csize = self.header_stack[t].get_size() as i32;
        let pos = self.position_stack[t] as i32;
        if pos < csize {
            self.file.seek(csize.wrapping_sub(pos), SeekFrom::Cur);
        }
        self.stack_index -= 1;
        if self.stack_index > 0 {
            let p = &mut self.position_stack[self.stack_index - 1];
            *p = p.wrapping_add((csize as u32).wrapping_add(CHUNK_HEADER_SIZE));
        }
        Ok(true)
    }

    /// `Cur_Chunk_ID()`.
    pub fn cur_chunk_id(&self) -> ChunkIoResult<u32> {
        Ok(self.header_stack[self.top()?].get_type())
    }

    /// `Cur_Chunk_Length()`.
    pub fn cur_chunk_length(&self) -> ChunkIoResult<u32> {
        Ok(self.header_stack[self.top()?].get_size())
    }

    /// `Cur_Chunk_Depth()`.
    pub fn cur_chunk_depth(&self) -> i32 {
        self.stack_index as i32
    }

    /// `Contains_Chunks()`: the flag bit as an `int` (`i32::MIN` or 0).
    pub fn contains_chunks(&self) -> ChunkIoResult<i32> {
        Ok(self.header_stack[self.top()?].get_sub_chunk_flag())
    }

    /// `Open_Micro_Chunk()`: reads the micro-chunk header through
    /// [`ChunkLoad::read`], so it fails at the end of the chunk.
    pub fn open_micro_chunk(&mut self) -> ChunkIoResult<bool> {
        let mut b = [self.mc_header.chunk_type, self.mc_header.chunk_size];
        let n = self.read(&mut b, MICRO_CHUNK_HEADER_SIZE);
        // a short file read copies its bytes into MCHeader even on failure
        self.mc_header = MicroChunkHeader {
            chunk_type: b[0],
            chunk_size: b[1],
        };
        if n? != MICRO_CHUNK_HEADER_SIZE {
            return Ok(false);
        }
        self.in_micro_chunk = true;
        self.micro_chunk_position = 0;
        Ok(true)
    }

    /// `Close_Micro_Chunk()`.
    pub fn close_micro_chunk(&mut self) -> ChunkIoResult<bool> {
        self.in_micro_chunk = false;
        let csize = i32::from(self.mc_header.chunk_size);
        let pos = self.micro_chunk_position;
        if pos < csize {
            self.file.seek(csize.wrapping_sub(pos), SeekFrom::Cur);
            if self.stack_index > 0 {
                let p = &mut self.position_stack[self.stack_index - 1];
                *p = p.wrapping_add(csize.wrapping_sub(pos) as u32);
            }
        }
        Ok(true)
    }

    /// `Cur_Micro_Chunk_ID()`.
    pub fn cur_micro_chunk_id(&self) -> u32 {
        u32::from(self.mc_header.chunk_type)
    }

    /// `Cur_Micro_Chunk_Length()`.
    pub fn cur_micro_chunk_length(&self) -> u32 {
        u32::from(self.mc_header.chunk_size)
    }

    /// Whether a micro-chunk is open (`InMicroChunk`).
    pub fn in_micro_chunk(&self) -> bool {
        self.in_micro_chunk
    }

    /// The bytes consumed in the innermost open chunk
    /// (`PositionStack[StackIndex-1]`), if any.
    pub fn cur_chunk_position(&self) -> Option<u32> {
        self.top().ok().map(|t| self.position_stack[t])
    }

    /// The upstream bounds checks of `Read` and `Seek`: false if `nbytes`
    /// would pass the end of the chunk or of the micro-chunk.
    fn fits(&self, t: usize, nbytes: u32) -> bool {
        if self.position_stack[t].wrapping_add(nbytes) > self.header_stack[t].get_size() {
            return false;
        }
        if self.in_micro_chunk
            && (self.micro_chunk_position as u32).wrapping_add(nbytes)
                > u32::from(self.mc_header.chunk_size)
        {
            return false;
        }
        true
    }

    fn advance(&mut self, t: usize, nbytes: u32) {
        self.position_stack[t] = self.position_stack[t].wrapping_add(nbytes);
        if self.in_micro_chunk {
            self.micro_chunk_position = self.micro_chunk_position.wrapping_add(nbytes as i32);
        }
    }

    /// `Read(buf, nbytes)` into `dst`: returns `nbytes`, or 0 if it would
    /// pass the end of the chunk (or micro-chunk) or the file has fewer bytes
    /// (whatever the file had is still copied into `dst`).
    pub fn read(&mut self, dst: &mut [u8], nbytes: u32) -> ChunkIoResult<u32> {
        let t = self.top()?;
        if !self.fits(t, nbytes) {
            return Ok(0);
        }
        if self.file.read(dst, nbytes as i32)? != nbytes as i32 {
            return Ok(0);
        }
        self.advance(t, nbytes);
        Ok(nbytes)
    }

    /// `Read(&value, sizeof(value))` for a value of `N` bytes held as `bytes`
    /// (its current bytes stay where a short read does not reach).
    pub fn read_into<const N: usize>(&mut self, bytes: &mut [u8; N]) -> ChunkIoResult<u32> {
        self.read(bytes, N as u32)
    }

    /// `Seek(nbytes)`: skips `nbytes` within the chunk; returns `nbytes`, or
    /// 0 if it would pass the end of the chunk (or micro-chunk) or the file
    /// (the file position still moves, clamped).
    pub fn seek(&mut self, nbytes: u32) -> ChunkIoResult<u32> {
        let t = self.top()?;
        if !self.fits(t, nbytes) {
            return Ok(0);
        }
        let curpos = self.file.tell();
        if self
            .file
            .seek(nbytes as i32, SeekFrom::Cur)
            .wrapping_sub(curpos)
            != nbytes as i32
        {
            return Ok(0);
        }
        self.advance(t, nbytes);
        Ok(nbytes)
    }

    /// `Read(IOVector2Struct *)`, `Read(IOVector3Struct *)`,
    /// `Read(IOVector4Struct *)`, `Read(IOQuaternionStruct *)`: `sizeof(v)`
    /// is the Win32 pointer size, so only the first component (4 bytes) of
    /// `v` is read. Returns 4 or 0.
    pub fn read_vector<const N: usize>(&mut self, v: &mut [f32; N]) -> ChunkIoResult<u32> {
        let mut b = v[0].to_le_bytes();
        let n = self.read(&mut b, WIN32_POINTER_SIZE)?;
        v[0] = f32::from_le_bytes(b);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: u32, sub: bool, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        let size = body.len() as u32 | if sub { 0x8000_0000 } else { 0 };
        v.extend_from_slice(&size.to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn nested_chunks_and_micro_chunks() {
        let micro = [7u8, 2, 0xAA, 0xBB, 9, 1, 0xCC];
        let inner = chunk(0x10, false, &micro);
        let outer = chunk(0x20, true, &inner);
        let mut data = outer.clone();
        data.extend(chunk(0x30, false, &[1, 2, 3]));
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        assert_eq!(c.cur_chunk_id().unwrap(), 0x20);
        assert_eq!(c.contains_chunks().unwrap(), i32::MIN);
        assert!(c.open_chunk().unwrap());
        assert_eq!(c.cur_chunk_length().unwrap(), 7);
        assert!(c.open_micro_chunk().unwrap());
        assert_eq!((c.cur_micro_chunk_id(), c.cur_micro_chunk_length()), (7, 2));
        let mut b = [0u8; 4];
        assert_eq!(c.read(&mut b, 3).unwrap(), 0, "past the micro-chunk");
        assert_eq!(c.read(&mut b, 1).unwrap(), 1);
        c.close_micro_chunk().unwrap();
        assert!(c.open_micro_chunk().unwrap());
        assert_eq!(c.cur_micro_chunk_id(), 9);
        c.close_micro_chunk().unwrap();
        assert!(!c.open_micro_chunk().unwrap(), "chunk exhausted");
        c.close_chunk().unwrap();
        assert!(!c.open_chunk().unwrap(), "parent eaten");
        c.close_chunk().unwrap();
        assert!(c.open_chunk().unwrap());
        assert_eq!(c.cur_chunk_id().unwrap(), 0x30);
        c.close_chunk().unwrap();
        assert!(!c.open_chunk().unwrap(), "end of file");
        assert_eq!(
            c.close_chunk(),
            Err(ChunkIoError::Undefined(ChunkIoUndefined::NoChunkOpen))
        );
    }

    #[test]
    fn wrapping_length_moves_the_file_back() {
        let data = chunk(1, false, &[0u8; 16]);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let mut b = [0u8; 8];
        assert_eq!(c.read(&mut b, 8).unwrap(), 8);
        // 8 + 0xFFFFFFFC wraps to 4 <= 16: passes, reads -4 bytes.
        assert_eq!(c.read(&mut b, 0xFFFF_FFFC).unwrap(), 0xFFFF_FFFC);
        assert_eq!(c.cur_chunk_position(), Some(4));
        assert_eq!(c.file().position(), 12);
    }

    #[test]
    fn vector_reads_take_the_win32_pointer_size() {
        let mut body = 1.5f32.to_le_bytes().to_vec();
        body.extend_from_slice(&2.5f32.to_le_bytes());
        let data = chunk(1, false, &body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        c.open_chunk().unwrap();
        let mut v = [0.0f32, 9.0, 9.0];
        assert_eq!(c.read_vector(&mut v).unwrap(), 4);
        assert_eq!(v, [1.5, 9.0, 9.0]);
        assert_eq!(c.cur_chunk_position(), Some(4));
    }

    #[test]
    fn stack_overflow_is_undefined() {
        let mut data = Vec::new();
        for _ in 0..257 {
            data.extend_from_slice(&1u32.to_le_bytes());
            data.extend_from_slice(&0x7FFF_FFFFu32.to_le_bytes());
        }
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        for _ in 0..256 {
            assert!(c.open_chunk().unwrap());
        }
        assert_eq!(
            c.open_chunk(),
            Err(ChunkIoError::Undefined(ChunkIoUndefined::StackOverflow))
        );
    }
}
