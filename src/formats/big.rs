// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/ArchiveFile.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/ArchiveFile.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/ArchiveFileSystem.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/ArchiveFileSystem.cpp
//   GeneralsMD/Code/GameEngineDevice/Include/Win32Device/Common/Win32BIGFile.h
//   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32BIGFile.cpp
//   GeneralsMD/Code/GameEngineDevice/Include/Win32Device/Common/Win32BIGFileSystem.h
//   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32BIGFileSystem.cpp
//   (and the RAMFile::openFromArchive read of GameEngine/Source/Common/System/RAMFile.cpp)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   ArchiveFileSystem: created 11/26/01 TR
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `.big` archives: the parser of `Win32BIGFileSystem::openArchiveFile`, the
//! per-archive directory tree of `ArchiveFile`, and the merged tree of
//! `ArchiveFileSystem`, reading from in-memory byte buffers (wasm has no file
//! system; where the bytes come from is the caller's business).
//!
//! # The format, as upstream reads it
//!
//! ```text
//! offset  size  field
//! 0       4     "BIGF"
//! 4       4     archive size, little-endian (read, then only logged)
//! 8       4     number of files N, big-endian
//! 12      4     (not read: the reader seeks to 0x10)
//! 16      ...   N entries: offset (u32 BE), size (u32 BE), NUL-terminated path
//! ```
//!
//! Each path is split at its last `\` or `/` into a directory path and a file
//! name; the file name is lower-cased, the directory path is lower-cased when
//! it is added to the tree, and the archive's own name is lower-cased.
//! Directory and file maps are ordered by `strcmp` (byte order), file lists
//! by `_stricmp` (byte order of the lower-cased names), as upstream's
//! `std::map<AsciiString>` and `FilenameList`.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! - A truncated header or entry table, or a path longer than 259 bytes:
//!   upstream then reads uninitialised stack bytes, loops on a failed read, or
//!   overruns its `char buffer[_MAX_PATH]`. Here: [`BigError`].
//! - Header fields that are only partly present are still read as upstream
//!   reads them (missing bytes are zero), because that is defined there: a
//!   count of zero or less means an archive with no files.
//! - A member opened normally must lie inside the archive (as upstream's
//!   `RAMFile::openFromArchive` requires; a streaming open does not check
//!   this, see [`ArchiveFile::open_file_streaming`]), with offset and size below 2^31
//!   (upstream converts both to `Int`). The one exception upstream has, offset
//!   `0xFFFFFFFF`, where a failed `lseek(-1)` returns -1 and so "succeeds" and
//!   the member is read from wherever the archive's file position was, is not
//!   reproduced: such a member does not open here.
//! - No local file system: `getFileInfo` gives only the size (upstream also
//!   fills timestamps from the archive file on disk), write access to a
//!   member (upstream copies it to the local disk) is not offered, and
//!   loading archives from a directory is the caller's loop over
//!   [`ArchiveFileSystem::load_big_file`].
//!
//! # Upstream quirks that are kept
//!
//! - A member whose name has no `.` lands in the merged tree as
//!   `name/name`, and is then not found in its own archive's tree, so
//!   [`ArchiveFileSystem::open_file`] cannot open it.
//! - [`ArchiveFile::get_file_list_in_directory`] finds a directory only when
//!   it is given with a trailing separator (`"data\\"`): for `"data"`, the
//!   tokenizer leaves its last token in place and the search descends into
//!   `data/data`.
//! - `searchSubdirectories` is ignored: listings always recurse.

use std::collections::BTreeMap;

/// `BIGFileIdentifier`.
pub const BIG_FILE_IDENTIFIER: &[u8; 4] = b"BIGF";
/// `#define MUSIC_BIG "Music.big"`.
pub const MUSIC_BIG: &str = "Music.big";
/// `_MAX_PATH` on Win32: the size of the parser's path buffer.
pub const MAX_PATH: usize = 260;

/// Why an archive could not be parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BigError {
    /// The first four bytes are not `BIGF` (upstream returns `NULL`).
    BadIdentifier,
    /// Fewer than four bytes (upstream compares uninitialised bytes).
    TooShortForIdentifier,
    /// Entry `index` is cut off by the end of the data (upstream then loops
    /// on failed one-byte reads over uninitialised stack bytes).
    TruncatedEntry { index: u32 },
    /// Entry `index` has no NUL within the 260-byte path buffer (upstream
    /// overruns the buffer).
    PathTooLong { index: u32 },
}

impl std::fmt::Display for BigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BigError::BadIdentifier => write!(f, "not a BIG archive (no BIGF identifier)"),
            BigError::TooShortForIdentifier => {
                write!(f, "BIG archive shorter than its 4-byte identifier")
            }
            BigError::TruncatedEntry { index } => {
                write!(f, "BIG archive directory entry {index} is truncated")
            }
            BigError::PathTooLong { index } => write!(
                f,
                "BIG archive entry {index} has a path longer than 259 bytes"
            ),
        }
    }
}

impl std::error::Error for BigError {}

/// `class ArchivedFileInfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArchivedFileInfo {
    pub filename: Vec<u8>,
    pub archive_filename: Vec<u8>,
    pub offset: u32,
    pub size: u32,
}

/// `class DetailedArchivedDirectoryInfo`: one archive's directory tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DetailedArchivedDirectoryInfo {
    pub directory_name: Vec<u8>,
    pub directories: BTreeMap<Vec<u8>, DetailedArchivedDirectoryInfo>,
    pub files: BTreeMap<Vec<u8>, ArchivedFileInfo>,
}

/// `class ArchivedDirectoryInfo`: the merged tree; each file maps to the
/// name of the archive that holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArchivedDirectoryInfo {
    pub directory_name: Vec<u8>,
    pub directories: BTreeMap<Vec<u8>, ArchivedDirectoryInfo>,
    pub files: BTreeMap<Vec<u8>, Vec<u8>>,
}

/// `typedef std::set<AsciiString, rts::less_than_nocase<AsciiString>>
/// FilenameList`: names compare by `_stricmp`, so two names that differ only
/// in case are one entry (the first inserted is kept).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilenameList {
    by_folded: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl FilenameList {
    pub fn new() -> Self {
        Self::default()
    }
    /// `find(name) == end()`.
    pub fn contains(&self, name: &[u8]) -> bool {
        self.by_folded.contains_key(&name.to_ascii_lowercase())
    }
    /// `insert(name)`: keeps an existing entry that compares equal.
    pub fn insert(&mut self, name: Vec<u8>) {
        self.by_folded
            .entry(name.to_ascii_lowercase())
            .or_insert(name);
    }
    /// The names in `_stricmp` order.
    pub fn iter(&self) -> impl Iterator<Item = &[u8]> {
        self.by_folded.values().map(|v| v.as_slice())
    }
    pub fn len(&self) -> usize {
        self.by_folded.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_folded.is_empty()
    }
}

// ---------------------------------------------------------------------------
// AsciiString helpers, as upstream defines them
// ---------------------------------------------------------------------------

const SEPS: &[u8] = b"\\/";

/// `AsciiString::nextToken(tok, "\\/")` (AsciiString.cpp): skip separators,
/// take the following non-separators into `tok` and keep the rest. On an
/// empty string it returns `false` and leaves `tok` **unchanged**; when only
/// separators remain it clears both and returns `false`.
fn next_token(s: &mut Vec<u8>, tok: &mut Vec<u8>) -> bool {
    if s.is_empty() {
        return false;
    }
    let start = s.iter().position(|c| !SEPS.contains(c)).unwrap_or(s.len());
    let end = s[start..]
        .iter()
        .position(|c| SEPS.contains(c))
        .map_or(s.len(), |p| start + p);
    if end > start {
        *tok = s[start..end].to_vec();
        s.drain(..end);
        true
    } else {
        s.clear();
        tok.clear();
        false
    }
}

/// `AsciiString::find('.') != NULL`.
fn has_dot(s: &[u8]) -> bool {
    s.contains(&b'.')
}

/// `AsciiString::endsWith("\\")`.
fn ends_with_backslash(s: &[u8]) -> bool {
    s.last() == Some(&b'\\')
}

/// The byte at `i` of a NUL-terminated string (`0` past the end).
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// `static Bool SearchStringMatches(AsciiString str, AsciiString
/// searchString)` (ArchiveFile.cpp): `*` matches any run, `?` one character.
/// Translated statement for statement, on NUL-terminated semantics.
pub fn search_string_matches(s: &[u8], search: &[u8]) -> bool {
    if s.is_empty() {
        return search.is_empty();
    }
    if search.is_empty() {
        return false;
    }
    let (mut c1, mut c2) = (0usize, 0usize);
    while at(s, c1) == at(search, c2) || at(search, c2) == b'?' || at(search, c2) == b'*' {
        if at(s, c1) == at(search, c2) || at(search, c2) == b'?' {
            c1 += 1;
            c2 += 1;
        } else if at(search, c2) == b'*' {
            c2 += 1;
            if at(search, c2) == 0 {
                return true;
            }
            while at(s, c1) != 0 {
                if search_string_matches(&s[c1..], &search[c2..]) {
                    return true;
                }
                c1 += 1;
            }
        }
        if at(s, c1) == 0 {
            return at(search, c2) == 0;
        }
        if at(search, c2) == 0 {
            return false;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// ArchiveFile / Win32BIGFile
// ---------------------------------------------------------------------------

/// `class ArchiveFile` with the `Win32BIGFile` implementation: one parsed
/// `.big` archive and its bytes.
#[derive(Clone, Debug)]
pub struct ArchiveFile {
    /// The archive's name as given to [`ArchiveFile::open_archive_file`].
    name: Vec<u8>,
    data: Vec<u8>,
    root_directory: DetailedArchivedDirectoryInfo,
    /// The size field of the header (little-endian; upstream only logs it).
    archive_file_size: i32,
}

/// `fp->read(&x, 4)` into an `Int` initialised to 0: the bytes that exist,
/// in memory (little-endian) order.
fn read_int_partial(data: &[u8], pos: &mut usize) -> [u8; 4] {
    let mut b = [0u8; 4];
    for (k, slot) in b.iter_mut().enumerate() {
        if let Some(&v) = data.get(*pos + k) {
            *slot = v;
        }
    }
    *pos = (*pos + 4).min(data.len().max(*pos));
    b
}

impl ArchiveFile {
    /// `Win32BIGFileSystem::openArchiveFile(filename)`, on the archive's
    /// bytes: check the identifier, read the count, walk the directory
    /// entries and build the tree (`ArchiveFile::addFile`).
    pub fn open_archive_file(filename: &[u8], data: Vec<u8>) -> Result<ArchiveFile, BigError> {
        let archive_file_name = filename.to_ascii_lowercase();
        if data.len() < 4 {
            return Err(BigError::TooShortForIdentifier);
        }
        if &data[0..4] != BIG_FILE_IDENTIFIER {
            return Err(BigError::BadIdentifier);
        }
        let mut pos = 4usize;
        let archive_file_size = i32::from_le_bytes(read_int_partial(&data, &mut pos));
        // "change the order of the bytes cause the file size is in reverse
        // byte order for some reason": ntohl of the in-memory Int.
        let num_little_files = i32::from_be_bytes(read_int_partial(&data, &mut pos));

        let mut archive = ArchiveFile {
            name: filename.to_vec(),
            data,
            root_directory: DetailedArchivedDirectoryInfo::default(),
            archive_file_size,
        };

        // seek to the beginning of the directory listing.
        let mut pos = 0x10usize;
        let mut index: u32 = 0;
        let mut i: i32 = 0;
        while i < num_little_files {
            let d = &archive.data;
            if pos + 8 > d.len() {
                return Err(BigError::TruncatedEntry { index });
            }
            let file_offset = u32::from_be_bytes([d[pos], d[pos + 1], d[pos + 2], d[pos + 3]]);
            let filesize = u32::from_be_bytes([d[pos + 4], d[pos + 5], d[pos + 6], d[pos + 7]]);
            pos += 8;

            // read in the path name of the file (into char buffer[_MAX_PATH]).
            let rest = &d[pos..];
            let nul = match rest.iter().take(MAX_PATH).position(|&c| c == 0) {
                Some(n) => n,
                None if rest.len() < MAX_PATH => return Err(BigError::TruncatedEntry { index }),
                None => return Err(BigError::PathTooLong { index }),
            };
            let buffer = rest[..nul].to_vec();
            pos += nul + 1;

            // split at the last separator (none: the whole name is the file).
            let split = buffer.iter().rposition(|&c| c == b'\\' || c == b'/');
            let (path, name) = match split {
                Some(k) => (buffer[..=k].to_vec(), buffer[k + 1..].to_vec()),
                None => (Vec::new(), buffer.clone()),
            };
            let info = ArchivedFileInfo {
                filename: name.to_ascii_lowercase(),
                archive_filename: archive_file_name.clone(),
                offset: file_offset,
                size: filesize,
            };
            archive.add_file(&path, &info);
            i += 1;
            index += 1;
        }
        Ok(archive)
    }

    /// The name the archive was opened with.
    pub fn name(&self) -> &[u8] {
        &self.name
    }

    /// The archive's own header size field (upstream only logs it).
    pub fn archive_file_size(&self) -> i32 {
        self.archive_file_size
    }

    /// The directory tree.
    pub fn root_directory(&self) -> &DetailedArchivedDirectoryInfo {
        &self.root_directory
    }

    /// `ArchiveFile::addFile`: walk (creating) the lower-cased directory
    /// path, then store the info under its file name (a later entry with the
    /// same name replaces an earlier one).
    fn add_file(&mut self, path: &[u8], file_info: &ArchivedFileInfo) {
        let mut temp = path.to_ascii_lowercase();
        let mut token = Vec::new();
        let mut dir_info = &mut self.root_directory;
        next_token(&mut temp, &mut token);
        while !token.is_empty() {
            dir_info = dir_info
                .directories
                .entry(token.clone())
                .or_insert_with(|| DetailedArchivedDirectoryInfo {
                    directory_name: token.clone(),
                    ..Default::default()
                });
            next_token(&mut temp, &mut token);
        }
        dir_info
            .files
            .insert(file_info.filename.clone(), file_info.clone());
    }

    /// `ArchiveFile::getArchivedFileInfo`: look a path up in this archive.
    pub fn get_archived_file_info(&self, filename: &[u8]) -> Option<&ArchivedFileInfo> {
        let mut path = filename.to_ascii_lowercase();
        let mut token = Vec::new();
        let mut dir_info = &self.root_directory;
        next_token(&mut path, &mut token);
        while !has_dot(&token) || has_dot(&path) {
            dir_info = dir_info.directories.get(&token)?;
            next_token(&mut path, &mut token);
        }
        dir_info.files.get(&token)
    }

    /// `Win32BIGFile::openFile` for reading: the member's bytes
    /// (`RAMFile::openFromArchive`: seek to the offset, read the size; both
    /// must succeed). `None` when the name is not in this archive or the data
    /// is not inside it.
    pub fn open_file(&self, filename: &[u8]) -> Option<&[u8]> {
        let info = self.get_archived_file_info(filename)?;
        let offset = i32::try_from(info.offset).ok()? as usize;
        let size = i32::try_from(info.size).ok()? as usize;
        if size == 0 {
            return Some(&[]);
        }
        self.data.get(offset..offset.checked_add(size)?)
    }

    /// `Win32BIGFile::openFile` with `File::STREAMING`: a
    /// `StreamingArchiveFile`. Its open does not read: it checks that the
    /// archive file can *seek* to the member's offset (`seek(offset, START)
    /// == offset`) and then `size` bytes further (`seek(size, CURRENT) ==
    /// offset + size`, in `Int` arithmetic). A seek past the end of a file
    /// succeeds and a failed seek returns -1, so:
    ///
    /// - a member that runs past the end of the archive still opens, and
    ///   reading it returns only the bytes that exist;
    /// - a size of 2^31 or more (a negative `Int`) opens when `offset + size`
    ///   is still a valid position, or when it wraps to exactly -1 (the
    ///   failed seek's -1 then "matches"); `size()` is then 0.
    ///
    /// An offset of 2^31 or more fails the first seek, except `0xFFFFFFFF`
    /// (-1 again), where upstream's result depends on the archive's previous
    /// file position; that one does not open here.
    pub fn open_file_streaming(&self, filename: &[u8]) -> Option<StreamingMember<'_>> {
        let info = self.get_archived_file_info(filename)?;
        let offset = info.offset as i32;
        let size = info.size as i32;
        // seek(offset, START): fails (-1) before the start of the file.
        if offset < 0 {
            return None;
        }
        // seek(size, CURRENT) must return m_startingPos + size.
        let target = i64::from(offset) + i64::from(size);
        let returned: i32 = if (0..=i64::from(i32::MAX)).contains(&target) { target as i32 } else { -1 };
        if returned != offset.wrapping_add(size) {
            return None;
        }
        // File::size(): StreamingArchiveFile::seek(0, END) clamps to [0, m_size].
        let reported = size.max(0);
        let start = (offset as usize).min(self.data.len());
        let stop = (offset as usize).saturating_add(reported as usize).min(self.data.len());
        Some(StreamingMember { size: reported, data: &self.data[start..stop] })
    }

    /// `Win32BIGFile::getFileInfo`: the member's size (`sizeLow`; `sizeHigh`
    /// is always 0). Upstream also copies the archive file's timestamps from
    /// the local file system, which does not exist here.
    pub fn get_file_info(&self, filename: &[u8]) -> Option<u32> {
        self.get_archived_file_info(filename).map(|i| i.size)
    }

    /// `ArchiveFile::getFileListInDirectory(currentDirectory,
    /// originalDirectory, searchName, filenameList, searchSubdirectories)`.
    /// (`currentDirectory` and `searchSubdirectories` are unused upstream.)
    pub fn get_file_list_in_directory(
        &self,
        _current_directory: &[u8],
        original_directory: &[u8],
        search_name: &[u8],
        filename_list: &mut FilenameList,
        _search_subdirectories: bool,
    ) {
        let mut search_dir = original_directory.to_ascii_lowercase();
        let mut dir_info = &self.root_directory;
        let mut token = Vec::new();
        next_token(&mut search_dir, &mut token);
        while !token.is_empty() {
            match dir_info.directories.get(&token) {
                Some(d) => dir_info = d,
                // if the directory doesn't exist, then there aren't any files to be had.
                None => return,
            }
            next_token(&mut search_dir, &mut token);
        }
        Self::list_dir(dir_info, original_directory, search_name, filename_list);
    }

    /// The recursive `getFileListInDirectory(dirInfo, currentDirectory, ...)`:
    /// subdirectories first (byte order), then this directory's matches.
    fn list_dir(
        dir_info: &DetailedArchivedDirectoryInfo,
        current_directory: &[u8],
        search_name: &[u8],
        filename_list: &mut FilenameList,
    ) {
        for temp_dir_info in dir_info.directories.values() {
            let mut tempdirname = current_directory.to_vec();
            if !tempdirname.is_empty() && !ends_with_backslash(&tempdirname) {
                tempdirname.push(b'\\');
            }
            tempdirname.extend_from_slice(&temp_dir_info.directory_name);
            Self::list_dir(temp_dir_info, &tempdirname, search_name, filename_list);
        }
        for info in dir_info.files.values() {
            if search_string_matches(&info.filename, search_name) {
                let mut tempfilename = current_directory.to_vec();
                if !tempfilename.is_empty() && !ends_with_backslash(&tempfilename) {
                    tempfilename.push(b'\\');
                }
                tempfilename.extend_from_slice(&info.filename);
                if !filename_list.contains(&tempfilename) {
                    filename_list.insert(tempfilename);
                }
            }
        }
    }
}

/// A member opened for streaming (see [`ArchiveFile::open_file_streaming`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamingMember<'a> {
    /// `size()`: the size declared in the directory entry (0 if it is a
    /// negative `Int`).
    pub size: i32,
    /// What reading `size()` bytes returns (shorter when the member runs
    /// past the end of the archive).
    pub data: &'a [u8],
}

// ---------------------------------------------------------------------------
// ArchiveFileSystem / Win32BIGFileSystem
// ---------------------------------------------------------------------------

/// `class ArchiveFileSystem` with the `Win32BIGFileSystem` loading: the
/// open archives by name, and the merged directory tree that says which
/// archive serves each path.
#[derive(Clone, Debug, Default)]
pub struct ArchiveFileSystem {
    archive_file_map: BTreeMap<Vec<u8>, ArchiveFile>,
    root_directory: ArchivedDirectoryInfo,
}

impl ArchiveFileSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// One iteration of `Win32BIGFileSystem::loadBigFilesFromDirectory`:
    /// open the archive, merge it into the tree (`loadIntoDirectoryTree`;
    /// with `overwrite`, its files replace earlier archives' files of the
    /// same path) and file it under `filename`. Returns whether it was added
    /// (`false` where upstream's `openArchiveFile` returns `NULL`), with the
    /// parse error.
    pub fn load_big_file(
        &mut self,
        filename: &[u8],
        data: Vec<u8>,
        overwrite: bool,
    ) -> Result<(), BigError> {
        let archive = ArchiveFile::open_archive_file(filename, data)?;
        self.load_into_directory_tree(&archive, filename, overwrite);
        self.archive_file_map.insert(filename.to_vec(), archive);
        Ok(())
    }

    /// `ArchiveFileSystem::loadIntoDirectoryTree`.
    fn load_into_directory_tree(
        &mut self,
        archive_file: &ArchiveFile,
        archive_filename: &[u8],
        overwrite: bool,
    ) {
        let mut filename_list = FilenameList::new();
        archive_file.get_file_list_in_directory(b"", b"", b"*", &mut filename_list, true);
        for name in filename_list.iter() {
            let mut path = name.to_ascii_lowercase();
            let mut token = Vec::new();
            let mut dir_info = &mut self.root_directory;
            let mut info_in_path = next_token(&mut path, &mut token);
            while info_in_path && (!has_dot(&token) || has_dot(&path)) {
                dir_info = dir_info
                    .directories
                    .entry(token.clone())
                    .or_insert_with(|| ArchivedDirectoryInfo {
                        directory_name: token.clone(),
                        ..Default::default()
                    });
                info_in_path = next_token(&mut path, &mut token);
            }
            // token is the filename, and dirInfo is the directory that this file is in.
            if overwrite || !dir_info.files.contains_key(&token) {
                dir_info.files.insert(token, archive_filename.to_vec());
            }
        }
    }

    /// The merged tree.
    pub fn root_directory(&self) -> &ArchivedDirectoryInfo {
        &self.root_directory
    }

    /// The archive filed under `name`.
    pub fn archive(&self, name: &[u8]) -> Option<&ArchiveFile> {
        self.archive_file_map.get(name)
    }

    /// `Win32BIGFileSystem::closeArchiveFile`: drop the archive from the map.
    /// Upstream also stops the music when it is `Music.big` (no audio here),
    /// and, as here, leaves the merged tree pointing at it (an `openFile` of
    /// such a path then dereferences a null `ArchiveFile*` upstream; here it
    /// returns `None`).
    pub fn close_archive_file(&mut self, filename: &[u8]) {
        self.archive_file_map.remove(filename);
    }

    /// `ArchiveFileSystem::doesFileExist`.
    pub fn does_file_exist(&self, filename: &[u8]) -> bool {
        let mut path = filename.to_ascii_lowercase();
        let mut token = Vec::new();
        let mut dir_info = &self.root_directory;
        next_token(&mut path, &mut token);
        while !has_dot(&token) || has_dot(&path) {
            match dir_info.directories.get(&token) {
                Some(d) => {
                    dir_info = d;
                    next_token(&mut path, &mut token);
                }
                None => return false,
            }
        }
        dir_info.files.contains_key(&token)
    }

    /// `ArchiveFileSystem::getArchiveFilenameForFile`: the archive that
    /// serves `filename`, or empty.
    pub fn get_archive_filename_for_file(&self, filename: &[u8]) -> Vec<u8> {
        let mut path = filename.to_ascii_lowercase();
        let mut token = Vec::new();
        let mut dir_info = &self.root_directory;
        next_token(&mut path, &mut token);
        while !has_dot(&token) || has_dot(&path) {
            match dir_info.directories.get(&token) {
                Some(d) => dir_info = d,
                None => return Vec::new(),
            }
            next_token(&mut path, &mut token);
        }
        dir_info.files.get(&token).cloned().unwrap_or_default()
    }

    /// `ArchiveFileSystem::openFile` (read access): the member's bytes from
    /// the archive that serves it.
    pub fn open_file(&self, filename: &[u8]) -> Option<&[u8]> {
        let archive_filename = self.get_archive_filename_for_file(filename);
        if archive_filename.is_empty() {
            return None;
        }
        self.archive_file_map
            .get(&archive_filename)?
            .open_file(filename)
    }

    /// `ArchiveFileSystem::openFile` with `File::STREAMING` (see
    /// [`ArchiveFile::open_file_streaming`]).
    pub fn open_file_streaming(&self, filename: &[u8]) -> Option<StreamingMember<'_>> {
        let archive_filename = self.get_archive_filename_for_file(filename);
        if archive_filename.is_empty() {
            return None;
        }
        self.archive_file_map
            .get(&archive_filename)?
            .open_file_streaming(filename)
    }

    /// `ArchiveFileSystem::getFileInfo`: the member's size (see
    /// [`ArchiveFile::get_file_info`]).
    pub fn get_file_info(&self, filename: &[u8]) -> Option<u32> {
        if filename.is_empty() {
            return None;
        }
        let archive_filename = self.get_archive_filename_for_file(filename);
        self.archive_file_map
            .get(&archive_filename)?
            .get_file_info(filename)
    }

    /// `ArchiveFileSystem::getFileListInDirectory`: every archive's matches,
    /// archives in name order.
    pub fn get_file_list_in_directory(
        &self,
        current_directory: &[u8],
        original_directory: &[u8],
        search_name: &[u8],
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
    ) {
        for archive in self.archive_file_map.values() {
            archive.get_file_list_in_directory(
                current_directory,
                original_directory,
                search_name,
                filename_list,
                search_subdirectories,
            );
        }
    }
}

/// Builds a `.big` archive in the layout upstream reads (for tests and
/// tools; upstream has no writer in the engine). `members` are (path, data);
/// the header's size field is the total length and the unread word at 12 is
/// zero.
pub fn build_big(members: &[(&[u8], &[u8])]) -> Vec<u8> {
    let table: usize = members.iter().map(|(p, _)| 8 + p.len() + 1).sum();
    let mut offset = 0x10 + table;
    let mut out = Vec::new();
    out.extend_from_slice(BIG_FILE_IDENTIFIER);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(members.len() as u32).to_be_bytes());
    out.extend_from_slice(&[0; 4]);
    for (p, d) in members {
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(d.len() as u32).to_be_bytes());
        out.extend_from_slice(p);
        out.push(0);
        offset += d.len();
    }
    for (_, d) in members {
        out.extend_from_slice(d);
    }
    let total = out.len() as u32;
    out[4..8].copy_from_slice(&total.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_open() {
        let big = build_big(&[
            (b"Data\\INI\\Object.ini", b"hello"),
            (b"Art/Textures/a.TGA", b"xy"),
            (b"readme", b"r"),
        ]);
        let a = ArchiveFile::open_archive_file(b"Test.big", big.clone()).unwrap();
        assert_eq!(a.archive_file_size(), big.len() as i32);
        assert_eq!(a.open_file(b"data/ini/object.ini"), Some(&b"hello"[..]));
        assert_eq!(a.open_file(b"ART\\TEXTURES\\A.tga"), Some(&b"xy"[..]));
        // a dot-less member is stored at the root but not found by name
        assert!(a.root_directory().files.contains_key(&b"readme"[..]));
        assert_eq!(a.open_file(b"readme"), None);

        let mut fs = ArchiveFileSystem::new();
        fs.load_big_file(b"Test.big", big, false).unwrap();
        assert!(fs.does_file_exist(b"Data\\INI\\Object.ini"));
        assert_eq!(
            fs.get_archive_filename_for_file(b"art/textures/a.tga"),
            b"Test.big"
        );
        assert_eq!(fs.open_file(b"art/textures/a.tga"), Some(&b"xy"[..]));
        assert!(fs.root_directory().directories[&b"readme"[..]]
            .files
            .contains_key(&b"readme"[..]));
        let mut list = FilenameList::new();
        fs.get_file_list_in_directory(b"", b"data\\", b"*.ini", &mut list, true);
        assert_eq!(
            list.iter().collect::<Vec<_>>(),
            vec![&b"data\\ini\\object.ini"[..]]
        );
        let mut none = FilenameList::new();
        fs.get_file_list_in_directory(b"", b"data", b"*.ini", &mut none, true);
        assert!(none.is_empty(), "upstream needs the trailing separator");
    }

    #[test]
    fn errors() {
        assert_eq!(
            ArchiveFile::open_archive_file(b"x", b"BIG".to_vec()).unwrap_err(),
            BigError::TooShortForIdentifier
        );
        assert_eq!(
            ArchiveFile::open_archive_file(b"x", b"BIGX0000".to_vec()).unwrap_err(),
            BigError::BadIdentifier
        );
        let mut big = build_big(&[(b"a.txt", b"1")]);
        big.truncate(20);
        assert_eq!(
            ArchiveFile::open_archive_file(b"x", big).unwrap_err(),
            BigError::TruncatedEntry { index: 0 }
        );
        let long = vec![b'a'; 300];
        let big = build_big(&[(&long, b"1")]);
        assert_eq!(
            ArchiveFile::open_archive_file(b"x", big).unwrap_err(),
            BigError::PathTooLong { index: 0 }
        );
        // "BIGF" alone: count reads as 0, a valid empty archive.
        assert!(ArchiveFile::open_archive_file(b"x", b"BIGF".to_vec()).is_ok());
    }

    #[test]
    fn wildcard_matching() {
        assert!(search_string_matches(b"object.ini", b"*.ini"));
        assert!(search_string_matches(b"a.ini", b"?.ini"));
        assert!(!search_string_matches(b"a.ini", b"*.tga"));
        assert!(search_string_matches(b"abc", b"*"));
        assert!(!search_string_matches(b"", b"*"));
        assert!(search_string_matches(b"", b""));
    }
}
