// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/XferLoad.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/XferLoad.cpp
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

//! `XferLoad`: reads a save file. Upstream reads a `FILE*` opened with
//! `fopen(identifier, "rb")`; here the file is a byte buffer handed to
//! [`XferLoad::new`] (wasm has no file system), read with the same `fread`
//! and `fseek` semantics: a short read consumes what is there and fails, a
//! seek past the end succeeds (later reads then fail), a seek before the
//! start fails.
//!
//! `xferSnapshot` hands each loaded snapshot to
//! `TheGameState->addPostProcessSnapshot` unless `XO_NO_POST_PROCESSING` is
//! set. `GameState` is not translated yet, so this records how many such
//! hand-offs upstream would have made ([`XferLoad::post_process_requests`]);
//! the caller owns the snapshots and runs their `load_post_process`.

use super::xfer::{Snapshot, Xfer, XferBlockSize, XferMode, XferResult, XferState, XferStatus, XO_NO_POST_PROCESSING};

/// `class XferLoad`.
#[derive(Clone, Debug)]
pub struct XferLoad {
    state: XferState,
    data: Vec<u8>,
    /// `Some(position)` while the "file" is open.
    pos: Option<usize>,
    post_process_requests: usize,
}

impl XferLoad {
    /// `XferLoad()`, with the contents of the file that `open` will read.
    pub fn new(data: Vec<u8>) -> Self {
        XferLoad { state: XferState::new(XferMode::Load), data, pos: None, post_process_requests: 0 }
    }

    /// The current read position (`ftell`), if open.
    pub fn position(&self) -> Option<usize> {
        self.pos
    }

    /// How many snapshots upstream would have passed to
    /// `TheGameState->addPostProcessSnapshot`.
    pub fn post_process_requests(&self) -> usize {
        self.post_process_requests
    }

    /// `fread(buf, len, 1, fp) == 1`, with glibc's partial-read behaviour
    /// (the bytes that exist are consumed and copied).
    fn fread_one(&mut self, buf: &mut [u8]) -> bool {
        let Some(pos) = self.pos else {
            return false;
        };
        let avail = self.data.len().saturating_sub(pos);
        let n = avail.min(buf.len());
        if n > 0 {
            buf[..n].copy_from_slice(&self.data[pos..pos + n]);
        }
        self.pos = Some(pos + n);
        !buf.is_empty() && n == buf.len()
    }
}

impl Xfer for XferLoad {
    fn state(&self) -> &XferState {
        &self.state
    }
    fn state_mut(&mut self) -> &mut XferState {
        &mut self.state
    }

    /// `XferLoad::open`: refuse a second open, base open, open at position 0.
    fn open(&mut self, identifier: &[u8]) -> XferResult<()> {
        if self.pos.is_some() {
            return Err(XferStatus::FileAlreadyOpen);
        }
        self.state.open(identifier);
        self.pos = Some(0);
        Ok(())
    }

    /// `XferLoad::close`: `XFER_FILE_NOT_OPEN` if not open; else close and
    /// clear the identifier.
    fn close(&mut self) -> XferResult<()> {
        if self.pos.take().is_none() {
            return Err(XferStatus::FileNotOpen);
        }
        self.state.identifier.clear();
        Ok(())
    }

    /// `XferLoad::beginBlock`: read the 4-byte block size; a failed read
    /// returns 0 (not an error), as upstream.
    fn begin_block(&mut self) -> XferResult<i32> {
        let mut b = [0u8; 4];
        if !self.fread_one(&mut b) {
            return Ok(0);
        }
        Ok(XferBlockSize::from_le_bytes(b))
    }

    /// `XferLoad::endBlock`: nothing.
    fn end_block(&mut self) -> XferResult<()> {
        Ok(())
    }

    /// `XferLoad::skip`: `fseek(fp, dataSize, SEEK_CUR)`, `XFER_SKIP_ERROR`
    /// if it fails (a target before the start of the file, or no file).
    fn skip(&mut self, data_size: i32) -> XferResult<()> {
        let Some(pos) = self.pos else {
            return Err(XferStatus::SkipError);
        };
        let target = pos as i64 + i64::from(data_size);
        if target < 0 {
            return Err(XferStatus::SkipError);
        }
        self.pos = Some(target as usize);
        Ok(())
    }

    /// `XferLoad::xferSnapshot`: run the snapshot's `xfer`, then (unless
    /// `XO_NO_POST_PROCESSING`) count a post-process hand-off.
    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshot) -> XferResult<()> {
        snapshot.xfer(self)?;
        if self.get_options() & XO_NO_POST_PROCESSING == 0 {
            self.post_process_requests += 1;
        }
        Ok(())
    }

    /// `XferLoad::xferImplementation`: `fread(data, dataSize, 1, fp)`;
    /// `XFER_READ_ERROR` on a short read, on `dataSize == 0`, or (where
    /// upstream would pass `NULL` to `fread`) without an open file.
    fn xfer_implementation(&mut self, data: &mut [u8]) -> XferResult<()> {
        if !self.fread_one(data) {
            return Err(XferStatus::ReadError);
        }
        Ok(())
    }

    /// `XferLoad::xferAsciiString`: an `UnsignedByte` length, the bytes, and
    /// the string is then set from the NUL-terminated buffer, so it ends at
    /// the first NUL byte read.
    fn xfer_ascii_string(&mut self, s: &mut Vec<u8>) -> XferResult<()> {
        let mut len = 0u8;
        self.xfer_unsigned_byte(&mut len)?;
        let mut buffer = vec![0u8; usize::from(len)];
        if len > 0 {
            self.xfer_user(&mut buffer)?;
        }
        if let Some(nul) = buffer.iter().position(|&c| c == 0) {
            buffer.truncate(nul);
        }
        *s = buffer;
        Ok(())
    }

    /// `XferLoad::xferUnicodeString`: an `UnsignedByte` length, the 16-bit
    /// characters, truncated at the first NUL character.
    fn xfer_unicode_string(&mut self, s: &mut Vec<u16>) -> XferResult<()> {
        let mut len = 0u8;
        self.xfer_unsigned_byte(&mut len)?;
        let mut bytes = vec![0u8; 2 * usize::from(len)];
        if len > 0 {
            self.xfer_user(&mut bytes)?;
        }
        let mut chars: Vec<u16> = bytes.chunks_exact(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
        if let Some(nul) = chars.iter().position(|&c| c == 0) {
            chars.truncate(nul);
        }
        *s = chars;
        Ok(())
    }
}
