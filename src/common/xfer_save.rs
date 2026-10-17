// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/XferSave.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/XferSave.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, February 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `XferSave`: writes a save file. Upstream writes to a `FILE*` opened with
//! `fopen(identifier, "w+b")`; wasm has no file system, so here the file is
//! an in-memory byte buffer with a file position, and `ftell`/`fseek`/
//! `fwrite` are reproduced on it (writing past the end zero-fills the gap, a
//! seek past the end that is never written to does not lengthen the file).
//! The bytes are [`XferSave::bytes`] (or [`XferSave::take_bytes`]).
//!
//! Blocks: `beginBlock` writes a 4-byte placeholder and pushes its position;
//! `endBlock` pops it and writes there the number of bytes written since,
//! excluding the placeholder itself.

use super::xfer::{Snapshot, Xfer, XferBlockSize, XferMode, XferResult, XferState, XferStatus};

/// An open in-memory "file".
#[derive(Clone, Debug, Default)]
struct MemFile {
    data: Vec<u8>,
    pos: usize,
}

impl MemFile {
    /// `fwrite` at the current position.
    fn write(&mut self, bytes: &[u8]) {
        let end = self.pos + bytes.len();
        if self.data.len() < end {
            self.data.resize(end, 0);
        }
        self.data[self.pos..end].copy_from_slice(bytes);
        self.pos = end;
    }
}

/// `class XferSave`.
#[derive(Clone, Debug)]
pub struct XferSave {
    state: XferState,
    file: Option<MemFile>,
    last_file: Vec<u8>,
    /// `m_blockStack`: the file positions of the open blocks, innermost last.
    block_stack: Vec<usize>,
}

impl Default for XferSave {
    fn default() -> Self {
        Self::new()
    }
}

impl XferSave {
    /// `XferSave()`: mode `XFER_SAVE`, no file, empty block stack.
    pub fn new() -> Self {
        XferSave { state: XferState::new(XferMode::Save), file: None, last_file: Vec::new(), block_stack: Vec::new() }
    }

    /// The file contents: the open file, or the last one closed.
    pub fn bytes(&self) -> &[u8] {
        match &self.file {
            Some(f) => &f.data,
            None => &self.last_file,
        }
    }

    /// Take the contents of the last closed file.
    pub fn take_bytes(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.last_file)
    }

    /// Number of blocks begun but not ended (upstream warns in its
    /// destructor when this is not zero).
    pub fn open_blocks(&self) -> usize {
        self.block_stack.len()
    }
}

impl Xfer for XferSave {
    fn state(&self) -> &XferState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut XferState {
        &mut self.state
    }

    /// `XferSave::open`: refuse a second open, base open, new empty file.
    fn open(&mut self, identifier: &[u8]) -> XferResult<()> {
        if self.file.is_some() {
            return Err(XferStatus::FileAlreadyOpen);
        }
        self.state.open(identifier);
        self.file = Some(MemFile::default());
        Ok(())
    }

    /// `XferSave::close`: `XFER_FILE_NOT_OPEN` without a file; else close it
    /// and clear the identifier.
    fn close(&mut self) -> XferResult<()> {
        match self.file.take() {
            None => Err(XferStatus::FileNotOpen),
            Some(f) => {
                self.last_file = f.data;
                self.state.identifier.clear();
                Ok(())
            }
        }
    }

    /// `XferSave::beginBlock`: remember the position, write a 0 placeholder,
    /// return `XFER_OK` (as an `Int`, as upstream). Without an open file
    /// upstream calls `ftell(NULL)` (undefined behaviour); here it returns
    /// `XFER_WRITE_ERROR`, the status of its failed-write branch.
    fn begin_block(&mut self) -> XferResult<i32> {
        let Some(f) = self.file.as_mut() else {
            return Ok(XferStatus::WriteError as i32);
        };
        let file_pos = f.pos;
        let block_size: XferBlockSize = 0;
        f.write(&block_size.to_le_bytes());
        self.block_stack.push(file_pos);
        Ok(XferStatus::Ok as i32)
    }

    /// `XferSave::endBlock`: `XFER_BEGIN_END_MISMATCH` without an open block;
    /// else back-patch the block size and return to the current position.
    /// (Without an open file upstream calls `ftell(NULL)`; here
    /// `XFER_WRITE_ERROR`.)
    fn end_block(&mut self) -> XferResult<()> {
        let Some(top) = self.block_stack.pop() else {
            return Err(XferStatus::BeginEndMismatch);
        };
        let Some(f) = self.file.as_mut() else {
            return Err(XferStatus::WriteError);
        };
        let current_file_pos = f.pos;
        f.pos = top;
        let block_size = (current_file_pos as i64 - top as i64 - 4) as XferBlockSize;
        f.write(&block_size.to_le_bytes());
        f.pos = current_file_pos;
        Ok(())
    }

    /// `XferSave::skip`: `fseek(fp, dataSize, SEEK_CUR)`, result ignored (so a
    /// seek before the start of the file leaves the position unchanged).
    fn skip(&mut self, data_size: i32) -> XferResult<()> {
        if let Some(f) = self.file.as_mut() {
            let target = f.pos as i64 + i64::from(data_size);
            if target >= 0 {
                f.pos = target as usize;
            }
        }
        Ok(())
    }

    /// `XferSave::xferSnapshot`: runs the snapshot's `xfer`.
    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshot) -> XferResult<()> {
        snapshot.xfer(self)
    }

    /// `XferSave::xferImplementation`: `fwrite(data, dataSize, 1, fp)`, which
    /// returns 0, so `XFER_WRITE_ERROR`, when `dataSize` is 0. (Without an
    /// open file upstream passes `NULL` to `fwrite`; here `XFER_WRITE_ERROR`.)
    fn xfer_implementation(&mut self, data: &mut [u8]) -> XferResult<()> {
        match self.file.as_mut() {
            Some(f) if !data.is_empty() => {
                f.write(data);
                Ok(())
            }
            _ => Err(XferStatus::WriteError),
        }
    }

    /// `XferSave::xferAsciiString`: at most 255 bytes, an `UnsignedByte`
    /// length, then the bytes.
    fn xfer_ascii_string(&mut self, s: &mut Vec<u8>) -> XferResult<()> {
        if s.len() > 255 {
            return Err(XferStatus::StringError);
        }
        let mut len = s.len() as u8;
        self.xfer_unsigned_byte(&mut len)?;
        if len > 0 {
            self.xfer_user(&mut s[..usize::from(len)])?;
        }
        Ok(())
    }

    /// `XferSave::xferUnicodeString`: at most 255 characters, an
    /// `UnsignedByte` length, then the 16-bit characters.
    fn xfer_unicode_string(&mut self, s: &mut Vec<u16>) -> XferResult<()> {
        if s.len() > 255 {
            return Err(XferStatus::StringError);
        }
        let mut len = s.len() as u8;
        self.xfer_unsigned_byte(&mut len)?;
        if len > 0 {
            let mut b: Vec<u8> = s.iter().flat_map(|c| c.to_le_bytes()).collect();
            self.xfer_user(&mut b)?;
        }
        Ok(())
    }
}
