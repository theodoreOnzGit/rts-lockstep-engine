//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/chunkio.cpp (the `ChunkSaveClass` half and the
//!   `WRITE_MICRO_CHUNK` family of helper macros, as plain functions)
//!
//! The file is an in-memory `Vec<u8>` standing in for `FileClass *File`; the stack depth limit is
//! `MAX_CHUNK_DEPTH` (256) as upstream.

const MAX_CHUNK_DEPTH: usize = 256;

/// `ChunkHeader` as `ChunkSaveClass` keeps it: type and size (bit 31 of size is the sub-chunk flag).
#[derive(Clone, Copy, Default)]
struct SaveChunkHeader {
    chunk_type: u32,
    chunk_size: u32,
}

impl SaveChunkHeader {
    fn set_sub_chunk_flag(&mut self, onoff: bool) {
        if onoff {
            self.chunk_size |= 0x8000_0000;
        } else {
            self.chunk_size &= 0x7FFF_FFFF;
        }
    }
    fn get_sub_chunk_flag(&self) -> u32 {
        self.chunk_size & 0x8000_0000
    }
    fn add_size(&mut self, add: u32) {
        self.chunk_size = self.chunk_size.wrapping_add(add);
    }
    fn get_size(&self) -> u32 {
        self.chunk_size & 0x7FFF_FFFF
    }
    fn bytes(&self) -> [u8; 8] {
        let mut b = [0u8; 8];
        b[0..4].copy_from_slice(&self.chunk_type.to_le_bytes());
        b[4..8].copy_from_slice(&self.chunk_size.to_le_bytes());
        b
    }
}

/// `class ChunkSaveClass`.
pub struct ChunkSaveClass {
    /// `FileClass * File` (an in-memory file; `file_pos` is its current position).
    pub file: Vec<u8>,
    file_pos: usize,
    stack_index: usize,
    position_stack: [i32; MAX_CHUNK_DEPTH],
    header_stack: [SaveChunkHeader; MAX_CHUNK_DEPTH],
    in_micro_chunk: bool,
    micro_chunk_position: i32,
    mc_type: u8,
    mc_size: u8,
}

impl Default for ChunkSaveClass {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkSaveClass {
    /// `ChunkSaveClass::ChunkSaveClass(FileClass * file)` (chunkio.cpp:83).
    pub fn new() -> Self {
        ChunkSaveClass {
            file: Vec::new(),
            file_pos: 0,
            stack_index: 0,
            position_stack: [0; MAX_CHUNK_DEPTH],
            header_stack: [SaveChunkHeader::default(); MAX_CHUNK_DEPTH],
            in_micro_chunk: false,
            micro_chunk_position: 0,
            mc_type: 0,
            mc_size: 0,
        }
    }

    /// `FileClass::Write` on the in-memory file: overwrites then extends.
    fn file_write(&mut self, buf: &[u8]) -> usize {
        for &b in buf {
            if self.file_pos < self.file.len() {
                self.file[self.file_pos] = b;
            } else {
                self.file.push(b);
            }
            self.file_pos += 1;
        }
        buf.len()
    }

    /// `File->Seek(0)` (current position).
    fn file_tell(&self) -> i32 {
        self.file_pos as i32
    }

    /// `File->Seek(pos, SEEK_SET)`.
    fn file_seek_set(&mut self, pos: i32) {
        self.file_pos = pos as usize;
    }

    /// `ChunkSaveClass::Begin_Chunk` (chunkio.cpp:108).
    pub fn begin_chunk(&mut self, id: u32) -> bool {
        // If we have a parent chunk, set its 'Contains_Chunks' flag
        if self.stack_index > 0 {
            self.header_stack[self.stack_index - 1].set_sub_chunk_flag(true);
        }
        let chunkh = SaveChunkHeader { chunk_type: id, chunk_size: 0 };
        let filepos = self.file_tell();
        self.position_stack[self.stack_index] = filepos;
        self.header_stack[self.stack_index] = chunkh;
        self.stack_index += 1;
        // write a temporary chunk header (size = 0)
        let bytes = chunkh.bytes();
        self.file_write(&bytes) == bytes.len()
    }

    /// `ChunkSaveClass::End_Chunk` (chunkio.cpp:148).
    pub fn end_chunk(&mut self) -> bool {
        debug_assert!(!self.in_micro_chunk);
        let curpos = self.file_tell();
        self.stack_index -= 1;
        let chunkpos = self.position_stack[self.stack_index];
        let chunkh = self.header_stack[self.stack_index];
        self.file_seek_set(chunkpos);
        let bytes = chunkh.bytes();
        if self.file_write(&bytes) != bytes.len() {
            return false;
        }
        // Add the total bytes written to any encompasing chunk
        if self.stack_index != 0 {
            self.header_stack[self.stack_index - 1].add_size(chunkh.get_size() + 8);
        }
        self.file_seek_set(curpos);
        true
    }

    /// `ChunkSaveClass::Begin_Micro_Chunk` (chunkio.cpp:197).
    pub fn begin_micro_chunk(&mut self, id: u32) -> bool {
        debug_assert!(id < 256);
        debug_assert!(!self.in_micro_chunk);
        self.mc_type = id as u8;
        self.mc_size = 0;
        self.micro_chunk_position = self.file_tell();
        // upstream calls Write() so the header bytes are tracked in the wrapping chunk
        let hdr = [self.mc_type, self.mc_size];
        if self.write(&hdr) != 2 {
            return false;
        }
        self.in_micro_chunk = true;
        true
    }

    /// `ChunkSaveClass::End_Micro_Chunk` (chunkio.cpp:233).
    pub fn end_micro_chunk(&mut self) -> bool {
        debug_assert!(self.in_micro_chunk);
        let curpos = self.file_tell();
        self.file_seek_set(self.micro_chunk_position);
        let hdr = [self.mc_type, self.mc_size];
        if self.file_write(&hdr) != 2 {
            return false;
        }
        self.file_seek_set(curpos);
        self.in_micro_chunk = false;
        true
    }

    /// `ChunkSaveClass::Write(const void * buf, uint32 nbytes)` (chunkio.cpp:264). Returns bytes written or 0.
    pub fn write(&mut self, buf: &[u8]) -> u32 {
        let nbytes = buf.len() as u32;
        // you mixed data and chunks within the same chunk NO NO! / you didnt open any chunks yet
        debug_assert!(self.stack_index > 0);
        debug_assert!(self.header_stack[self.stack_index - 1].get_sub_chunk_flag() == 0);
        if self.file_write(buf) != buf.len() {
            return 0;
        }
        self.header_stack[self.stack_index - 1].add_size(nbytes);
        if self.in_micro_chunk {
            debug_assert!((self.mc_size as u32) < 255 - nbytes);
            self.mc_size = self.mc_size.wrapping_add(nbytes as u8);
        }
        nbytes
    }

    /// `Write(const IOVector2Struct &)`, `Write(const IOVector3Struct &)`, `Write(const IOVector4Struct &)`,
    /// `Write(const IOQuaternionStruct &)`: the whole struct (`sizeof(v)`), written as little-endian floats.
    pub fn write_floats(&mut self, v: &[f32]) -> u32 {
        let mut bytes = Vec::with_capacity(v.len() * 4);
        for f in v {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        self.write(&bytes)
    }

    /// `ChunkSaveClass::Cur_Chunk_Depth` (chunkio.cpp).
    pub fn cur_chunk_depth(&self) -> i32 {
        self.stack_index as i32
    }

    /// `WRITE_MICRO_CHUNK(csave, id, var)` for a value already as bytes (chunkio.h:293).
    pub fn write_micro_chunk(&mut self, id: u32, var: &[u8]) {
        self.begin_micro_chunk(id);
        self.write(var);
        self.end_micro_chunk();
    }

    /// `WRITE_MICRO_CHUNK_STRING(csave, id, var)` (chunkio.h:304): the string and its NUL.
    pub fn write_micro_chunk_string(&mut self, id: u32, var: &str) {
        self.begin_micro_chunk(id);
        let mut b = var.as_bytes().to_vec();
        b.push(0);
        self.write(&b);
        self.end_micro_chunk();
    }

    /// `WRITE_WWSTRING_CHUNK(csave, id, var)` (chunkio.h:249): a whole chunk holding the string and its NUL.
    pub fn write_wwstring_chunk(&mut self, id: u32, var: &str) {
        self.begin_chunk(id);
        let mut b = var.as_bytes().to_vec();
        b.push(0);
        self.write(&b);
        self.end_chunk();
    }
}
