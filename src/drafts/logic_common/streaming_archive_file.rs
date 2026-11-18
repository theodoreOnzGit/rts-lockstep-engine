//! UNTESTED DRAFT (#202, #208): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/Common/StreamingArchiveFile.h
//!   GeneralsMD/Code/GameEngine/Source/Common/System/StreamingArchiveFile.cpp
//!
//! A file inside a `.big` archive that is read straight from the archive's own open file (a window `[offset, offset+size)`),
//! not copied into memory. The archive's `File` is the host's: [`ArchiveHandle`].

use super::local_file::SeekMode;

/// The calls `StreamingArchiveFile` makes on the archive's `File*`.
pub trait ArchiveHandle {
    /// `File::seek(pos, mode)`: the new position, -1 on error.
    fn seek(&mut self, pos: i32, mode: SeekMode) -> i32;
    /// `File::read(buffer, bytes)` (`None` advances the position): bytes read.
    fn read(&mut self, buffer: Option<&mut [u8]>, bytes: i32) -> i32;
}

/// `class StreamingArchiveFile : public RAMFile`'s own members (the `File` base is the host's).
#[derive(Clone, Debug, PartialEq)]
pub struct StreamingArchiveFile {
    /// `File::m_open`.
    pub open: bool,
    /// `File::m_name`: set by `File::open`, then replaced by `m_nameStr`.
    pub name: Vec<u8>,
    /// `m_file` is non-NULL (the handle itself is passed to each call).
    pub has_file: bool,
    /// `m_startingPos`: "where the file begins in the archive".
    pub starting_pos: i32,
    /// `m_size`.
    pub size: i32,
    /// `m_curPos`.
    pub cur_pos: i32,
}

impl StreamingArchiveFile {
    /// `StreamingArchiveFile::StreamingArchiveFile()`: `m_file(NULL), m_startingPos(0), m_size(0), m_curPos(0)`.
    pub fn new() -> StreamingArchiveFile {
        StreamingArchiveFile { open: false, name: Vec::new(), has_file: false, starting_pos: 0, size: 0, cur_pos: 0 }
    }

    /// `StreamingArchiveFile::open(File *file)`: always TRUE (the body is `return TRUE`).
    pub fn open_from_file(&mut self) -> bool {
        true
    }

    /// `StreamingArchiveFile::openFromArchive(archiveFile, filename, offset, size)`: FALSE for a NULL archive file; `File::open(filename,
    /// READ | BINARY | STREAMING)`; then verifies the window by seeking to `offset` (must return `offset`) and by `size` (the relative
    /// `seek(size)` with the default `CURRENT` mode must return `m_startingPos + size`), then returns to `offset`.
    pub fn open_from_archive<A: ArchiveHandle>(&mut self, archive: Option<&mut A>, filename: &[u8], offset: i32, size: i32) -> bool {
        let archive = match archive {
            Some(a) => a,
            None => return false,
        };
        // File::open(filename.str(), File::READ | File::BINARY | File::STREAMING): base-class bookkeeping.
        self.open = true;
        self.name = filename.to_vec();
        self.has_file = true;
        self.starting_pos = offset;
        self.size = size;
        self.cur_pos = 0;
        if archive.seek(offset, SeekMode::Start) != offset {
            return false;
        }
        if archive.seek(size, SeekMode::Current) != self.starting_pos + size {
            return false;
        }
        // We know this will succeed.
        archive.seek(offset, SeekMode::Start);
        true
    }

    /// `StreamingArchiveFile::close()`: `File::close()`.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// `StreamingArchiveFile::read(buffer, bytes)`: "if buffer is null, just advance the current position by 'bytes'". 0 without an
    /// archive file; positions the archive at `m_startingPos + m_curPos`, clips `bytes` to the window's end, reads, advances `m_curPos`.
    pub fn read<A: ArchiveHandle>(&mut self, archive: &mut A, buffer: Option<&mut [u8]>, mut bytes: i32) -> i32 {
        if !self.has_file {
            return 0;
        }
        // There shouldn't be a way that this can fail, because we've already verified that the file contains at least this many bits.
        archive.seek(self.starting_pos + self.cur_pos, SeekMode::Start);
        if bytes + self.cur_pos > self.size {
            bytes = self.size - self.cur_pos;
        }
        let bytes_read = archive.read(buffer, bytes);
        self.cur_pos += bytes_read;
        bytes_read
    }

    /// `StreamingArchiveFile::write(buffer, bytes)`: `DEBUG_CRASH("Cannot write to streaming files.")` (release: nothing); -1.
    pub fn write(&mut self, _buffer: Option<&[u8]>, _bytes: i32) -> i32 {
        -1
    }

    /// `StreamingArchiveFile::seek(pos, mode)`: within the window only; the result is clamped to `[0, m_size]`.
    pub fn seek(&mut self, pos: i32, mode: SeekMode) -> i32 {
        let mut new_pos = match mode {
            SeekMode::Start => pos,
            SeekMode::Current => self.cur_pos + pos,
            SeekMode::End => self.size + pos, // DEBUG_ASSERTCRASH(pos <= 0, ...)
        };
        if new_pos < 0 {
            new_pos = 0;
        } else if new_pos > self.size {
            new_pos = self.size;
        }
        self.cur_pos = new_pos;
        self.cur_pos
    }
}

impl Default for StreamingArchiveFile {
    fn default() -> Self {
        StreamingArchiveFile::new()
    }
}
