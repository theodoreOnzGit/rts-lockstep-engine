//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/TagBlock.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/tagblock.cpp
//!
//! `TagBlockFile`: a file of named (tagged) variable-size blocks, "a .MIX file that can have files added to it at any
//! time" (Write Once / Read Many). Upstream derives from `RawFileClass` (replaced by `std::fs` in the port, per the
//! previous pass's note) and hands out `TagBlockHandle *`s; here the handle is a small value ([`TagBlockHandle`]) whose
//! operations take the owning file (`&mut TagBlockFile`), the creation privilege is a handle id, and `delete handle`
//! is [`TagBlockFile::close_tag`]. The `IndexList` (an `SList` kept ordered by DESCENDING crc: a new index is inserted
//! before the first with a smaller crc) is a `Vec` in the same order. On-disk layout (little-endian): `FileHeader`
//! {u32 Version = 20000626, i32 NumBlocks, i32 FileSize}, then per block `BlockHeader` {i32 Index, i32 TagSize,
//! i32 DataSize}, the NUL-terminated tag name and the data. Upstream notes kept: `Read` of a tag name goes into a
//! `MAX_TAG_NAME_SIZE = 1024` buffer without checking `TagSize`; a block whose `Index` is not its ordinal ends the scan
//! and the header is repaired; `Create_Tag` writes the new block header with `Index = -1` (set when the write access
//! ends); I/O results of the `RawFileClass` calls are not checked.

use crate::ww::realcrc::crc_stringi;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

/// `FILE_VERSION = 20000626` ("Put in date when format is changed").
pub const FILE_VERSION: u32 = 20000626;
/// `MAX_TAG_NAME_SIZE = 1024`.
pub const MAX_TAG_NAME_SIZE: usize = 1024;
/// `sizeof(FileHeader)`, `sizeof(BlockHeader)`.
const FILE_HEADER_SIZE: usize = 12;
const BLOCK_HEADER_SIZE: usize = 12;

/// `TagBlockFile::FileHeader`.
#[derive(Clone, Copy, Debug, Default)]
struct FileHeader {
    version: u32,
    num_blocks: i32,
    file_size: i32,
}

impl FileHeader {
    fn to_bytes(&self) -> [u8; FILE_HEADER_SIZE] {
        let mut b = [0u8; FILE_HEADER_SIZE];
        b[0..4].copy_from_slice(&self.version.to_le_bytes());
        b[4..8].copy_from_slice(&self.num_blocks.to_le_bytes());
        b[8..12].copy_from_slice(&self.file_size.to_le_bytes());
        b
    }
    fn from_bytes(b: &[u8; FILE_HEADER_SIZE]) -> Self {
        FileHeader { version: u32::from_le_bytes([b[0], b[1], b[2], b[3]]), num_blocks: i32::from_le_bytes([b[4], b[5], b[6], b[7]]), file_size: i32::from_le_bytes([b[8], b[9], b[10], b[11]]) }
    }
}

/// `TagBlockFile::BlockHeader`.
#[derive(Clone, Copy, Debug, Default)]
struct BlockHeader {
    /// "Used to verify file integrity."
    index: i32,
    /// "Size of tagname (including NULL) that follows this block."
    tag_size: i32,
    /// "Size of block not including header."
    data_size: i32,
}

impl BlockHeader {
    fn to_bytes(&self) -> [u8; BLOCK_HEADER_SIZE] {
        let mut b = [0u8; BLOCK_HEADER_SIZE];
        b[0..4].copy_from_slice(&self.index.to_le_bytes());
        b[4..8].copy_from_slice(&self.tag_size.to_le_bytes());
        b[8..12].copy_from_slice(&self.data_size.to_le_bytes());
        b
    }
    fn from_bytes(b: &[u8; BLOCK_HEADER_SIZE]) -> Self {
        BlockHeader { index: i32::from_le_bytes([b[0], b[1], b[2], b[3]]), tag_size: i32::from_le_bytes([b[4], b[5], b[6], b[7]]), data_size: i32::from_le_bytes([b[8], b[9], b[10], b[11]]) }
    }
}

/// `class TagBlockIndex` (defined in tagblock.cpp): crc of the tag name (case-insensitive) and the block's offsets.
#[derive(Clone, Copy, Debug)]
struct TagBlockIndex {
    crc: u32,
    block_offset: i32,
    data_offset: i32,
}

impl TagBlockIndex {
    /// `TagBlockIndex(tagname, blockoffset)`: `CRC_Stringi(tagname)`, `DataOffset = Calc_Data_Offset`.
    fn new(tagname: &str, block_offset: i32) -> Self {
        TagBlockIndex { crc: crc_stringi(tagname.as_bytes(), 0), block_offset, data_offset: TagBlockFile::calc_data_offset(block_offset, tagname) }
    }
    fn get_tag_offset(&self) -> i32 {
        TagBlockFile::calc_tag_offset(self.block_offset)
    }
    fn get_tag_size(&self) -> i32 {
        self.data_offset - self.get_tag_offset()
    }
}

/// `class TagBlockHandle`: all external access to the file goes through handles.
#[derive(Clone, Copy, Debug)]
pub struct TagBlockHandle {
    id: u32,
    /// Arena slot of the block's index in the file's `IndexList` (by block offset, stable across inserts).
    block_offset: i32,
    /// "Keep header information in memory so that it can be updated."
    block_header: BlockHeader,
    /// "Current position we are in the file" (within the block's data).
    position: i32,
}

/// `class TagBlockFile`.
pub struct TagBlockFile {
    file: File,
    header: FileHeader,
    /// Id of the handle that has creation privileges (`CreateHandle`).
    create_handle: Option<u32>,
    num_open_handles: i32,
    next_handle_id: u32,
    file_time: u32,
    /// `IndexList`: ordered by DESCENDING crc.
    index_list: Vec<TagBlockIndex>,
}

impl TagBlockFile {
    /// `Calc_Tag_Offset(blockoffset)`: `blockoffset + sizeof(BlockHeader)`.
    pub fn calc_tag_offset(blockoffset: i32) -> i32 {
        blockoffset + BLOCK_HEADER_SIZE as i32
    }

    /// `Calc_Data_Offset(blockoffset, tagname)`: after the tag name and its NUL.
    pub fn calc_data_offset(blockoffset: i32, tagname: &str) -> i32 {
        Self::calc_tag_offset(blockoffset) + tagname.len() as i32 + 1
    }

    fn seek_set(&mut self, pos: i32) {
        let _ = self.file.seek(SeekFrom::Start(pos as u64));
    }

    fn save_header(&mut self) {
        self.seek_set(0);
        let b = self.header.to_bytes();
        let _ = self.file.write_all(&b);
    }

    /// `TagBlockFile(fname)` (tagblock.cpp:98): opens (creating it if need be), reads the header and rebuilds the index
    /// from the blocks; a bad version resets the file; an inconsistent header is repaired.
    pub fn new(fname: &str) -> std::io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).create(true).open(fname)?;
        let mut f = TagBlockFile { file, header: FileHeader::default(), create_handle: None, num_open_handles: 0, next_handle_id: 1, file_time: 0, index_list: Vec::new() };
        f.file_time = f.file.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as u32);

        let mut hb = [0u8; FILE_HEADER_SIZE];
        let _ = f.file.read(&mut hb);
        f.header = FileHeader::from_bytes(&hb);

        if f.header.version == FILE_VERSION {
            // Validate the file by reading in all the blocks
            let mut curpos = FILE_HEADER_SIZE as i32;
            let mut block = 0;
            while block < f.header.num_blocks {
                f.seek_set(curpos);
                let mut bb = [0u8; BLOCK_HEADER_SIZE];
                let _ = f.file.read(&mut bb);
                let blockheader = BlockHeader::from_bytes(&bb);

                if blockheader.index == block {
                    let mut tagname = vec![0u8; blockheader.tag_size.max(0) as usize];
                    let _ = f.file.read(&mut tagname);
                    let name = String::from_utf8_lossy(&tagname[..tagname.iter().position(|&c| c == 0).unwrap_or(tagname.len())]).into_owned();
                    f.create_index(&name, curpos);
                    curpos = Self::calc_tag_offset(curpos) + blockheader.tag_size + blockheader.data_size;
                } else {
                    break;
                }
                block += 1;
            }

            // If the header does not agree with what was found, trust the blocks and repair it
            if curpos != f.header.file_size || block != f.header.num_blocks {
                f.header.num_blocks = block;
                f.header.file_size = curpos;
                f.seek_set(0);
                let b = f.header.to_bytes();
                let _ = f.file.write_all(&b);
            }
        } else {
            f.reset_file();
        }
        Ok(f)
    }

    /// `Reset_File()` ("DANGEROUS!! Resets entire file and makes small"): a fresh header and an empty index; the bytes after the header are
    /// left in place, as upstream's `Close()`/`Open()` leaves them.
    pub fn reset_file(&mut self) {
        self.empty_index_list();
        self.header.version = FILE_VERSION;
        self.header.num_blocks = 0;
        self.header.file_size = FILE_HEADER_SIZE as i32;
        self.save_header();
        // (upstream then does `Close(); Open(READ|WRITE);`, which does not shorten the file: it keeps whatever follows)
    }

    /// `Get_Date_Time()`: the file's time when it was opened.
    pub fn get_date_time(&self) -> u32 {
        self.file_time
    }

    /// `Does_Tag_Exist(tagname)`.
    pub fn does_tag_exist(&mut self, tagname: &str) -> bool {
        self.find_block(tagname).is_some()
    }

    /// `Open_Tag(tagname)`: a handle to read an existing block; `None` if the tag is not found.
    pub fn open_tag(&mut self, tagname: &str) -> Option<TagBlockHandle> {
        let index = self.find_block(tagname)?;
        self.seek_set(index.block_offset);
        let mut bb = [0u8; BLOCK_HEADER_SIZE];
        let _ = self.file.read(&mut bb);
        let blockheader = BlockHeader::from_bytes(&bb);

        let id = self.next_handle_id;
        self.next_handle_id += 1;
        self.num_open_handles += 1;
        Some(TagBlockHandle { id, block_offset: index.block_offset, block_header: blockheader, position: 0 })
    }

    /// `Create_Tag(tagname)`: the (single) handle with write access to a new block at the end of the file; `None` if
    /// another handle already has it or the tag exists.
    pub fn create_tag(&mut self, tagname: &str) -> Option<TagBlockHandle> {
        // Only one handle may have write access
        if self.create_handle.is_some() {
            return None;
        }

        // Create an index for the new block (fails if the tag already exists)
        let block_offset = self.header.file_size;
        let index = self.create_index(tagname, block_offset)?;

        // Write the block header (Index = -1 until the write access ends) and the tag name
        let blockheader = BlockHeader { index: -1, tag_size: index.get_tag_size(), data_size: 0 };
        self.seek_set(index.block_offset);
        let _ = self.file.write_all(&blockheader.to_bytes());
        let mut name = tagname.as_bytes().to_vec();
        name.push(0);
        let _ = self.file.write_all(&name);

        let id = self.next_handle_id;
        self.next_handle_id += 1;
        self.create_handle = Some(id);
        self.num_open_handles += 1;
        Some(TagBlockHandle { id, block_offset: index.block_offset, block_header: blockheader, position: 0 })
    }

    /// `Close_Tag(handle)`: `delete handle`, i.e. `~TagBlockHandle` -> `Destroy_Handle`.
    pub fn close_tag(&mut self, mut handle: TagBlockHandle) {
        self.destroy_handle(&mut handle);
    }

    /// `Destroy_Handle(handle)`: ends write access and drops the open-handle count.
    fn destroy_handle(&mut self, handle: &mut TagBlockHandle) {
        self.end_write_access(handle);
        self.num_open_handles -= 1;
    }

    /// `Handle_Can_Write(handle)`.
    fn handle_can_write(&self, handle: &TagBlockHandle) -> bool {
        self.create_handle == Some(handle.id)
    }

    /// `End_Write_Access(handle)`: finalises the block (index, header, file size) and gives up write privilege.
    pub fn end_write_access(&mut self, handle: &mut TagBlockHandle) -> bool {
        if self.create_handle == Some(handle.id) {
            // Fix up the block header and the file header
            handle.block_header.index = self.header.num_blocks;
            self.header.num_blocks += 1;

            let data_offset = self.index_list.iter().find(|i| i.block_offset == handle.block_offset).map_or(0, |i| i.data_offset);
            self.header.file_size = data_offset;
            self.header.file_size += handle.block_header.data_size;

            // Rewrite the block header and the file header
            self.seek_set(handle.block_offset);
            let _ = self.file.write_all(&handle.block_header.to_bytes());
            self.save_header();

            self.create_handle = None;
            return true;
        }
        false
    }

    /// `Create_Index(tagname, blockoffset)`: adds an index in descending crc order; `None` if the tag exists.
    fn create_index(&mut self, tagname: &str, blockoffset: i32) -> Option<TagBlockIndex> {
        if self.find_block(tagname).is_some() {
            return None;
        }

        let index = TagBlockIndex::new(tagname, blockoffset);

        // Insert before the first node with a smaller crc, else at the tail
        match self.index_list.iter().position(|cur| index.crc > cur.crc) {
            Some(p) => self.index_list.insert(p, index),
            None => self.index_list.push(index),
        }
        Some(index)
    }

    /// `Find_Block(tagname)`: walks the crc-ordered list (stopping once past the crc) and confirms the name by reading it
    /// from the file (`strcmpi`).
    fn find_block(&mut self, tagname: &str) -> Option<TagBlockIndex> {
        if self.index_list.is_empty() {
            return None;
        }

        let crc = crc_stringi(tagname.as_bytes(), 0);
        for k in 0..self.index_list.len() {
            let cur = self.index_list[k];
            if cur.crc == crc {
                // Same crc: check the name in the file to rule out a collision
                self.seek_set(cur.get_tag_offset());
                let mut name = vec![0u8; cur.get_tag_size().max(0) as usize];
                let _ = self.file.read(&mut name);
                let n = name.iter().position(|&c| c == 0).unwrap_or(name.len());
                if String::from_utf8_lossy(&name[..n]).eq_ignore_ascii_case(tagname) {
                    return Some(cur);
                }
            }
            // The list is in descending order: nothing further can match
            if cur.crc < crc {
                break;
            }
        }
        None
    }

    /// `Empty_Index_List()`: asserts no handle is open.
    fn empty_index_list(&mut self) {
        assert!(self.num_open_handles == 0);
        self.index_list.clear();
    }
}

impl TagBlockHandle {
    /// `Write(buf, nbytes)`: only the handle with creation privilege may write; -1 otherwise.
    pub fn write(&mut self, file: &mut TagBlockFile, buf: &[u8]) -> i32 {
        if !file.handle_can_write(self) {
            return -1;
        }

        let data_offset = file.index_list.iter().find(|i| i.block_offset == self.block_offset).map_or(0, |i| i.data_offset);
        file.seek_set(data_offset + self.position);
        let nbytes = if file.file.write_all(buf).is_ok() { buf.len() as i32 } else { 0 };
        self.position += nbytes;
        if self.position > self.block_header.data_size {
            self.block_header.data_size = self.position;
        }
        nbytes
    }

    /// `Read(buf, nbytes)`: clipped to the end of the block's data.
    pub fn read(&mut self, file: &mut TagBlockFile, buf: &mut [u8]) -> i32 {
        let mut nbytes = buf.len() as i32;
        if self.position + nbytes > self.block_header.data_size {
            nbytes = self.block_header.data_size - self.position;
        }

        let data_offset = file.index_list.iter().find(|i| i.block_offset == self.block_offset).map_or(0, |i| i.data_offset);
        file.seek_set(data_offset + self.position);
        let n = if nbytes > 0 { file.file.read(&mut buf[..nbytes as usize]).unwrap_or(0) as i32 } else { 0 };
        self.position += n;
        n
    }

    /// `Seek(pos, dir)`: `SEEK_CUR` (default), `SEEK_SET` or `SEEK_END`; returns the new position.
    pub fn seek(&mut self, pos: i32, dir: SeekDir) -> i32 {
        match dir {
            SeekDir::Cur => self.position += pos,
            SeekDir::Set => self.position = pos,
            SeekDir::End => self.position = self.block_header.data_size + pos,
        }
        self.position
    }

    /// `End_Write_Access()`.
    pub fn end_write_access(&mut self, file: &mut TagBlockFile) -> bool {
        file.end_write_access(self)
    }

    /// `Tell()`.
    pub fn tell(&self) -> i32 {
        self.position
    }

    /// `Get_Data_Size()`.
    pub fn get_data_size(&self) -> i32 {
        self.block_header.data_size
    }
}

/// `SEEK_CUR` / `SEEK_SET` / `SEEK_END` of `TagBlockHandle::Seek`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekDir {
    Cur,
    Set,
    End,
}
