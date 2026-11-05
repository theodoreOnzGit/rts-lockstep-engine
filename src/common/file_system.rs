// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/FileSystem.cpp, Include/Common/FileSystem.h
//   (FileSystem::openFile, doesFileExist, getFileListInDirectory, getFileInfo)
//   GeneralsMD/Code/GameEngine/Source/Common/System/LocalFileSystem.cpp, Include/Common/LocalFileSystem.h
//   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32LocalFileSystem.cpp
//   (doesFileExist, getFileListInDirectory, getFileInfo)
//   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32BIGFileSystem.cpp
//   (init, loadBigFilesFromDirectory)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TheFileSystem`: the game's files, from the local file system first and then from the `.big` archives.
//! Integration batch F1, GitHub issue #9.
//!
//! - [`LocalDevice`] is what `Win32LocalFileSystem` asks Windows for: a file's length, a range of its bytes, and a
//!   directory's entries. It is the one part that is not translated: the browser's storage of the player's imported
//!   folder (#3), the install folder on disk ([`NativeDevice`]), or memory ([`MemoryDevice`], for tests).
//! - [`LocalFileSystem`] is `Win32LocalFileSystem`'s logic over it: existence, whole-file reads, and the recursive
//!   `getFileListInDirectory` with `FindFirstFile`'s wildcard.
//! - [`FileSystem`] is `FileSystem.cpp` over the local file system and an [`ArchiveFileSystem`]. `init` is
//!   `Win32BIGFileSystem::init`: every `*.big` under the game's folder, then every one under the *Generals*
//!   install path (Zero Hour loads the base game's archives too), each without overwriting (`loadBigFilesFromDirectory`'s
//!   default), so a path found in a Zero Hour archive keeps it. Archives are read by their directory only
//!   ([`ArchiveFile::open_archive_directory`](crate::formats::big::ArchiveFile::open_archive_directory)), and a member
//!   is read from its archive's range when it is opened, so a gigabyte archive is never held whole.
//!
//! # Where this differs from upstream
//!
//! - **`FindFirstFile`'s pattern** is matched with `SearchStringMatches` on lower-cased names (Windows matches without
//!   case); the patterns the game uses (`*.big`, `*.ini`, `*.map`) mean the same.
//! - **`FileSystem::doesFileExist`'s cache** (`m_fileExist`, keyed by `TheNameKeyGenerator->nameToLowercaseKey`) is not
//!   kept here: it is a cache of the same answer, but upstream's key lookup also *creates* name keys. Where the logic's
//!   own name keys must follow upstream's (the map loader's existence checks), the logic makes those keys itself.
//! - **No `File` objects**: a read returns the bytes (`RAMFile`'s whole-file read, which is how the INI, map and model
//!   loaders read).

use crate::formats::big::{search_string_matches, ArchiveFileSystem, BigError, FilenameList};
use crate::logic::world::game_data::GameDataFiles;

/// What the local file system asks the operating system (the browser's storage, a disk, or memory). Paths are the
/// game's: backslash-separated, relative to the game's folder, any case.
pub trait LocalDevice {
    /// The file's length in bytes, or `None` when there is no such file (a directory is not a file).
    fn file_len(&self, path: &[u8]) -> Option<usize>;
    /// The file's bytes from `offset`, `len` of them (fewer at its end; none when there is no such file).
    fn read_range(&self, path: &[u8], offset: usize, len: usize) -> Vec<u8>;
    /// The entries of the directory `dir` (empty: the game's folder): each name, as stored, and whether it is a directory.
    fn list_directory(&self, dir: &[u8]) -> Vec<(Vec<u8>, bool)>;
    /// GitHub #24: whether a read of `len` bytes at `offset` of `path` would be served now. A device whose bytes arrive
    /// later ([`RangedDevice`], the browser's) answers false for bytes it does not hold yet, and asks for them; the
    /// others hold every byte. A path the device does not have is ready (its read fails at once).
    fn ready(&self, _path: &[u8], _offset: usize, _len: usize) -> bool {
        true
    }
    /// GitHub #24: how many reads the device could not serve yet because their bytes had not arrived (they are being
    /// fetched); always 0 for a device that holds every byte. A caller compares it before and after an operation to tell
    /// "not there" from "not there yet".
    fn unserved_reads(&self) -> u64 {
        0
    }
}

/// `Win32LocalFileSystem`: the local files. Upstream's paths are relative to the process's current directory (the game's
/// folder) unless absolute (the *Generals* install path from the registry). Here the device holds both, and
/// `game_dir` is the game's folder in it: a path starting with `\` is the device's own (absolute), any other is under
/// `game_dir`.
#[derive(Clone, Debug, Default)]
pub struct LocalFileSystem<D> {
    pub device: D,
    /// The game's folder in the device (empty: its root), ending in `\` when not empty.
    pub game_dir: Vec<u8>,
}

impl<D: LocalDevice> LocalFileSystem<D> {
    pub fn new(device: D) -> Self {
        LocalFileSystem { device, game_dir: Vec::new() }
    }

    /// A game path as the device's: an absolute path (`\...`) without its first separator, a relative one under
    /// `game_dir`.
    pub fn device_path(&self, path: &[u8]) -> Vec<u8> {
        match path.first() {
            Some(b'\\') | Some(b'/') => path[1..].to_vec(),
            _ => {
                let mut p = self.game_dir.clone();
                p.extend_from_slice(path);
                p
            }
        }
    }

    /// `doesFileExist(filename)`: `_access(filename, 0) == 0` (here: a file, not a directory).
    pub fn does_file_exist(&self, filename: &[u8]) -> bool {
        self.device.file_len(&self.device_path(filename)).is_some()
    }

    /// `openFile(filename, READ)` read whole, or `None`.
    pub fn open_file(&self, filename: &[u8]) -> Option<Vec<u8>> {
        let p = self.device_path(filename);
        let len = self.device.file_len(&p)?;
        Some(self.device.read_range(&p, 0, len))
    }

    /// The file's bytes from `offset`, `len` of them (an archive member's read).
    pub fn read_range(&self, filename: &[u8], offset: usize, len: usize) -> Vec<u8> {
        self.device.read_range(&self.device_path(filename), offset, len)
    }

    /// `getFileInfo(filename)`: the file's size (`sizeLow`).
    pub fn get_file_info(&self, filename: &[u8]) -> Option<usize> {
        self.device.file_len(&self.device_path(filename))
    }

    /// `Win32LocalFileSystem::getFileListInDirectory(currentDirectory, originalDirectory, searchName, filenameList,
    /// searchSubdirectories)`: the files of `originalDirectory + currentDirectory` matching `searchName`, named
    /// `originalDirectory + currentDirectory + name`; then, when asked, each subdirectory in turn
    /// (`currentDirectory + subdirectory + "\\"`).
    pub fn get_file_list_in_directory(
        &self,
        current_directory: &[u8],
        original_directory: &[u8],
        search_name: &[u8],
        filename_list: &mut FilenameList,
        search_subdirectories: bool,
    ) {
        let mut dir = original_directory.to_vec();
        dir.extend_from_slice(current_directory);
        let entries = self.device.list_directory(&self.device_path(&dir));
        let pattern = search_name.to_ascii_lowercase();
        for (name, is_dir) in &entries {
            if !is_dir && name != b"." && name != b".." && search_string_matches(&name.to_ascii_lowercase(), &pattern) {
                let mut new_filename = dir.clone();
                new_filename.extend_from_slice(name);
                if !filename_list.contains(&new_filename) {
                    filename_list.insert(new_filename);
                }
            }
        }
        if search_subdirectories {
            for (name, is_dir) in &entries {
                if *is_dir && name != b"." && name != b".." {
                    let mut sub = current_directory.to_vec();
                    sub.extend_from_slice(name);
                    sub.push(b'\\');
                    self.get_file_list_in_directory(&sub, original_directory, search_name, filename_list, search_subdirectories);
                }
            }
        }
    }
}

/// `TheFileSystem`: the local files over the archives (`FileSystem.cpp`), with the archives
/// (`Win32BIGFileSystem`) loaded from the local files.
#[derive(Clone, Debug, Default)]
pub struct FileSystem<D> {
    pub local: LocalFileSystem<D>,
    pub archives: ArchiveFileSystem,
    /// The archives that could not be opened (`openArchiveFile`'s `DEBUG_CRASH` and `NULL`): their names and why.
    pub archive_errors: Vec<(Vec<u8>, BigError)>,
}

impl<D: LocalDevice> FileSystem<D> {
    pub fn new(device: D) -> Self {
        FileSystem { local: LocalFileSystem::new(device), archives: ArchiveFileSystem::new(), archive_errors: Vec::new() }
    }

    /// `Win32BIGFileSystem::init`: `loadBigFilesFromDirectory("", "*.big")`, then the same under the *Generals* install
    /// path (upstream reads it from the registry; here the caller gives it as an absolute device path, `\Generals\`).
    pub fn init(&mut self, generals_install_path: Option<&[u8]>) {
        self.load_big_files_from_directory(b"", b"*.big", false);
        if let Some(p) = generals_install_path.filter(|p| !p.is_empty()) {
            self.load_big_files_from_directory(p, b"*.big", false);
        }
    }

    /// `Win32BIGFileSystem::loadBigFilesFromDirectory(dir, fileMask, overwrite)`: every matching file under `dir`
    /// (subdirectories too), in the list's order, opened and merged into the directory tree. Returns whether any was
    /// added.
    pub fn load_big_files_from_directory(&mut self, dir: &[u8], file_mask: &[u8], overwrite: bool) -> bool {
        let mut list = FilenameList::new();
        self.local.get_file_list_in_directory(dir, b"", file_mask, &mut list, true);
        let mut actually_added = false;
        let names: Vec<Vec<u8>> = list.iter().map(|n| n.to_vec()).collect();
        for name in names {
            let Some(len) = self.local.get_file_info(&name) else { continue };
            let local = &self.local;
            match self.archives.load_big_file_directory(&name, len, |o, l| local.read_range(&name, o, l), overwrite) {
                Ok(()) => actually_added = true,
                Err(e) => self.archive_errors.push((name, e)),
            }
        }
        actually_added
    }

    /// `FileSystem::openFile(filename, READ)`, read whole: the local file, else the archive member.
    pub fn open_file(&self, filename: &[u8]) -> Option<Vec<u8>> {
        if let Some(d) = self.local.open_file(filename) {
            return Some(d);
        }
        let (archive, offset, size) = self.archives.member_location(filename)?;
        if size == 0 {
            return Some(Vec::new());
        }
        let data = self.local.read_range(&archive, offset, size);
        // `RAMFile::openFromArchive`: the read must return the whole member
        (data.len() == size).then_some(data)
    }

    /// GitHub #24 (new): whether [`FileSystem::open_file`] of `filename` would be served now by a device whose bytes
    /// arrive later ([`LocalDevice::ready`]): the local file's bytes, else the archive member's range. A file that is not
    /// there is ready (opening it fails at once). Asks the device for the bytes it lacks.
    pub fn file_ready(&self, filename: &[u8]) -> bool {
        let p = self.local.device_path(filename);
        if let Some(len) = self.local.device.file_len(&p) {
            return self.local.device.ready(&p, 0, len);
        }
        match self.archives.member_location(filename) {
            Some((archive, offset, size)) if size > 0 => self.local.device.ready(&self.local.device_path(&archive), offset, size),
            _ => true,
        }
    }

    /// `FileSystem::doesFileExist(filename)`: a local file or an archive member.
    pub fn does_file_exist(&self, filename: &[u8]) -> bool {
        self.local.does_file_exist(filename) || self.archives.does_file_exist(filename)
    }

    /// `FileSystem::getFileInfo(filename)`: the size, local first.
    pub fn get_file_info(&self, filename: &[u8]) -> Option<usize> {
        if let Some(s) = self.local.get_file_info(filename) {
            return Some(s);
        }
        self.archives.get_file_info(filename).map(|s| s as usize)
    }

    /// `FileSystem::getFileListInDirectory(directory, searchName, filenameList, searchSubdirectories)`: the local
    /// matches, then the archives'.
    pub fn get_file_list_in_directory(&self, directory: &[u8], search_name: &[u8], filename_list: &mut FilenameList, search_subdirectories: bool) {
        self.local.get_file_list_in_directory(b"", directory, search_name, filename_list, search_subdirectories);
        self.archives.get_file_list_in_directory(b"", directory, search_name, filename_list, search_subdirectories);
    }
}

/// The logic's INI and map loaders read through `TheFileSystem`.
impl<D: LocalDevice> GameDataFiles for FileSystem<D> {
    fn open(&self, path: &[u8]) -> Option<Vec<u8>> {
        self.open_file(path)
    }
    fn list_ini(&self, dir: &[u8]) -> FilenameList {
        let mut d = dir.to_vec();
        d.push(b'\\');
        let mut list = FilenameList::new();
        self.get_file_list_in_directory(&d, b"*.ini", &mut list, true);
        list
    }
}

/// A local device in memory: files by path (any case, `\` or `/`), for tests and for files already read.
#[derive(Clone, Debug, Default)]
pub struct MemoryDevice {
    /// lower-cased, backslash path -> (path as given, bytes)
    files: std::collections::BTreeMap<Vec<u8>, (Vec<u8>, Vec<u8>)>,
}

fn normalise(path: &[u8]) -> Vec<u8> {
    path.iter().map(|&c| if c == b'/' { b'\\' } else { c.to_ascii_lowercase() }).collect()
}

impl MemoryDevice {
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds (or replaces) the file `path`.
    pub fn insert(&mut self, path: &[u8], data: Vec<u8>) {
        let p: Vec<u8> = path.iter().map(|&c| if c == b'/' { b'\\' } else { c }).collect();
        self.files.insert(normalise(path), (p, data));
    }
}

impl LocalDevice for MemoryDevice {
    fn file_len(&self, path: &[u8]) -> Option<usize> {
        self.files.get(&normalise(path)).map(|(_, d)| d.len())
    }
    fn read_range(&self, path: &[u8], offset: usize, len: usize) -> Vec<u8> {
        let Some((_, d)) = self.files.get(&normalise(path)) else { return Vec::new() };
        let start = offset.min(d.len());
        let end = offset.saturating_add(len).min(d.len());
        d[start..end].to_vec()
    }
    fn list_directory(&self, dir: &[u8]) -> Vec<(Vec<u8>, bool)> {
        list_paths(self.files.iter().map(|(k, (orig, _))| (k.as_slice(), orig.as_slice())), dir)
    }
}

/// The entries of `dir` among files given by (normalised path, path as given): each name as given, and whether it is a
/// directory (a memory device's listing, shared by [`MemoryDevice`] and [`RangedDevice`]).
fn list_paths<'a>(files: impl Iterator<Item = (&'a [u8], &'a [u8])>, dir: &[u8]) -> Vec<(Vec<u8>, bool)> {
    let mut prefix = normalise(dir);
    if !prefix.is_empty() && prefix.last() != Some(&b'\\') {
        prefix.push(b'\\');
    }
    let mut out: std::collections::BTreeMap<Vec<u8>, (Vec<u8>, bool)> = std::collections::BTreeMap::new();
    for (k, orig) in files {
        let Some(rest) = k.strip_prefix(prefix.as_slice()) else { continue };
        let orig_rest = &orig[orig.len() - rest.len()..];
        match rest.iter().position(|&c| c == b'\\') {
            Some(i) => {
                out.entry(rest[..i].to_vec()).or_insert((orig_rest[..i].to_vec(), true));
            }
            None => {
                out.insert(rest.to_vec(), (orig_rest.to_vec(), false));
            }
        }
    }
    out.into_values().collect()
}

/// One file of a [`RangedDevice`]: its length, the page's key for it, and the byte ranges that have arrived.
#[derive(Clone, Debug, Default)]
struct RangedFile {
    /// The path as given.
    name: Vec<u8>,
    len: usize,
    /// The page's key for the file (its index in the page's list of handles).
    key: u32,
    /// The ranges held, by offset (they may overlap).
    chunks: std::collections::BTreeMap<usize, Vec<u8>>,
}

impl RangedFile {
    /// The bytes `[offset, end)` when one held range covers them.
    fn covered(&self, offset: usize, end: usize) -> Option<&[u8]> {
        self.chunks.range(..=offset).rev().find_map(|(&o, c)| (o + c.len() >= end).then(|| &c[offset - o..end - o]))
    }
}

/// What a [`RangedDevice`] has held and served (GitHub #24's measure of the ranged reads).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RangedStats {
    /// The files' total length.
    pub total_len: usize,
    /// The bytes held now (each range that arrived, counted once).
    pub held: usize,
    /// The ranges that arrived, and their bytes.
    pub ranges_arrived: usize,
    pub bytes_arrived: usize,
    /// Reads served, and reads not served because their bytes had not arrived ([`LocalDevice::unserved_reads`]).
    pub reads_served: u64,
    pub reads_unserved: u64,
}

/// GitHub #24 (new; upstream reads its archives through Windows' synchronous `ReadFile`): a local device whose bytes
/// arrive later, by range. The browser's: the game runs on the page's main thread, which cannot read a file
/// synchronously, so each file is known by its length first and its bytes are fetched when a read asks for them.
///
/// - [`RangedDevice::insert_ranged`] adds a file by length (and the page's key for it), with the bytes already read
///   from its start (an archive's header and directory, [`ArchiveFile::directory_prefix_needed`](crate::formats::big::ArchiveFile::directory_prefix_needed),
///   so `FileSystem::init` can mount it at once). [`RangedDevice::insert`] adds a file whole.
/// - A read of bytes that have not arrived returns nothing (the caller's open fails: `FileSystem::open_file` checks the
///   member's length) and is listed as wanted, once; [`RangedDevice::take_wanted`] gives the list to the page, which
///   reads those ranges from the player's file and hands them back with [`RangedDevice::provide`]. The same read then
///   succeeds. [`LocalDevice::unserved_reads`] counts the reads that waited, so a caller can tell "not yet" from "not
///   there".
/// - Only the ranges read are held: an archive's directory and the members opened.
#[derive(Debug, Default)]
pub struct RangedDevice {
    /// lower-cased, backslash path -> the file
    files: std::collections::BTreeMap<Vec<u8>, RangedFile>,
    /// The ranges asked for and not arrived: (key, offset, len); taken by the page.
    wanted: std::cell::RefCell<std::collections::BTreeSet<(u32, usize, usize)>>,
    /// Every range asked for that has not arrived (kept after `take_wanted`, so a range is asked for once).
    asked: std::cell::RefCell<std::collections::BTreeSet<(u32, usize, usize)>>,
    stats: std::cell::Cell<RangedStats>,
}

impl RangedDevice {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds (or replaces) the file `path` whole.
    pub fn insert(&mut self, path: &[u8], data: Vec<u8>) {
        let len = data.len();
        self.insert_ranged(path, len, u32::MAX, data);
    }

    /// Adds (or replaces) the file `path`, `len` bytes long, known to the page as `key`, with its first `prefix.len()`
    /// bytes (possibly none).
    pub fn insert_ranged(&mut self, path: &[u8], len: usize, key: u32, prefix: Vec<u8>) {
        let name: Vec<u8> = path.iter().map(|&c| if c == b'/' { b'\\' } else { c }).collect();
        let mut f = RangedFile { name, len, key, chunks: Default::default() };
        let mut s = self.stats.get();
        if let Some(old) = self.files.get(&normalise(path)) {
            s.total_len -= old.len;
            s.held -= old.chunks.values().map(Vec::len).sum::<usize>();
        }
        s.total_len += len;
        if !prefix.is_empty() {
            s.held += prefix.len().min(len);
            f.chunks.insert(0, prefix.into_iter().take(len).collect());
        }
        self.stats.set(s);
        self.files.insert(normalise(path), f);
    }

    /// The bytes of the file the page knows as `key`, from `offset`, have arrived (a range [`RangedDevice::take_wanted`]
    /// listed, or any other). Bytes past the file's end are dropped. Returns whether the file is known.
    pub fn provide(&mut self, key: u32, offset: usize, mut bytes: Vec<u8>) -> bool {
        let Some(f) = self.files.values_mut().find(|f| f.key == key) else { return false };
        bytes.truncate(f.len.saturating_sub(offset));
        let mut s = self.stats.get();
        s.ranges_arrived += 1;
        s.bytes_arrived += bytes.len();
        s.held += bytes.len();
        if let Some(old) = f.chunks.insert(offset, bytes) {
            s.held -= old.len();
        }
        self.stats.set(s);
        let served = |&(k, o, l): &(u32, usize, usize)| k == key && f.covered(o, o + l).is_some();
        self.asked.borrow_mut().retain(|r| !served(r));
        self.wanted.borrow_mut().retain(|r| !served(r));
        true
    }

    /// The ranges wanted since the last call: (key, offset, len), each asked for once.
    pub fn take_wanted(&self) -> Vec<(u32, usize, usize)> {
        std::mem::take(&mut *self.wanted.borrow_mut()).into_iter().collect()
    }

    /// Ranges asked for that have not arrived.
    pub fn outstanding(&self) -> usize {
        self.asked.borrow().len()
    }

    pub fn stats(&self) -> RangedStats {
        self.stats.get()
    }

    /// The bytes, or `None` after asking for them (a file the device does not have, or a read past its end, is `Some`:
    /// nothing to wait for).
    fn try_read(&self, path: &[u8], offset: usize, len: usize) -> Option<&[u8]> {
        let Some(f) = self.files.get(&normalise(path)) else { return Some(&[]) };
        let start = offset.min(f.len);
        let end = offset.saturating_add(len).min(f.len);
        if start == end {
            return Some(&[]);
        }
        if let Some(b) = f.covered(start, end) {
            return Some(b);
        }
        let r = (f.key, start, end - start);
        if f.key != u32::MAX && self.asked.borrow_mut().insert(r) {
            self.wanted.borrow_mut().insert(r);
        }
        None
    }
}

impl LocalDevice for RangedDevice {
    fn file_len(&self, path: &[u8]) -> Option<usize> {
        self.files.get(&normalise(path)).map(|f| f.len)
    }
    fn read_range(&self, path: &[u8], offset: usize, len: usize) -> Vec<u8> {
        let mut s = self.stats.get();
        let r = self.try_read(path, offset, len);
        match r {
            Some(_) => s.reads_served += 1,
            None => s.reads_unserved += 1,
        }
        self.stats.set(s);
        r.map(<[u8]>::to_vec).unwrap_or_default()
    }
    fn list_directory(&self, dir: &[u8]) -> Vec<(Vec<u8>, bool)> {
        list_paths(self.files.iter().map(|(k, f)| (k.as_slice(), f.name.as_slice())), dir)
    }
    fn ready(&self, path: &[u8], offset: usize, len: usize) -> bool {
        self.try_read(path, offset, len).is_some()
    }
    fn unserved_reads(&self) -> u64 {
        self.stats.get().reads_unserved
    }
}

/// A local device on disk: the game's folder at `root`. The game's paths ignore case, the disk's may not (a Steam
/// install on Linux keeps Windows' mixed case), so each path component is matched without case.
#[derive(Clone, Debug)]
pub struct NativeDevice {
    pub root: std::path::PathBuf,
}

impl NativeDevice {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        NativeDevice { root: root.into() }
    }

    /// The real path of a game path, each component found without case; `None` when one is missing.
    fn resolve(&self, path: &[u8]) -> Option<std::path::PathBuf> {
        let mut cur = self.root.clone();
        for comp in path.split(|&c| c == b'\\' || c == b'/').filter(|c| !c.is_empty()) {
            let want = String::from_utf8_lossy(comp).to_ascii_lowercase();
            let exact = cur.join(String::from_utf8_lossy(comp).as_ref());
            if exact.exists() {
                cur = exact;
                continue;
            }
            let found = std::fs::read_dir(&cur).ok()?.flatten().find(|e| e.file_name().to_string_lossy().to_ascii_lowercase() == want)?;
            cur = found.path();
        }
        Some(cur)
    }
}

impl LocalDevice for NativeDevice {
    fn file_len(&self, path: &[u8]) -> Option<usize> {
        let p = self.resolve(path)?;
        let m = std::fs::metadata(p).ok()?;
        m.is_file().then_some(m.len() as usize)
    }
    fn read_range(&self, path: &[u8], offset: usize, len: usize) -> Vec<u8> {
        use std::io::{Read, Seek, SeekFrom};
        let Some(p) = self.resolve(path) else { return Vec::new() };
        let Ok(mut f) = std::fs::File::open(p) else { return Vec::new() };
        if f.seek(SeekFrom::Start(offset as u64)).is_err() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(len);
        let _ = f.take(len as u64).read_to_end(&mut out);
        out
    }
    fn list_directory(&self, dir: &[u8]) -> Vec<(Vec<u8>, bool)> {
        let Some(p) = self.resolve(dir) else { return Vec::new() };
        let Ok(rd) = std::fs::read_dir(p) else { return Vec::new() };
        let mut out: Vec<(Vec<u8>, bool)> =
            rd.flatten().map(|e| (e.file_name().to_string_lossy().as_bytes().to_vec(), e.file_type().is_ok_and(|t| t.is_dir()))).collect();
        out.sort();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::big::build_big;

    /// A Zero Hour folder with a loose file and two archives, and a Generals install with one. Predicted: archives are
    /// found under subdirectories too; a loose file wins over an archived one (`FileSystem::openFile` asks the local
    /// file system first); between archives the first loaded keeps a path (no overwrite), so Zero Hour's `INIZH.big`
    /// serves `Data\INI\Weapon.ini` over the Generals `INI.big`; a path only the Generals archive has is served by it;
    /// the INI list is the local and archived `.ini` files under the directory; reads are by member range.
    ///
    /// The device holds both folders side by side, as the browser's import will (and as a Steam install lays them out):
    /// `ZH\` is the game's folder, `\Generals\` the install path. (The first run of this test put the Generals folder
    /// inside the game's folder. `loadBigFilesFromDirectory("", "*.big")` searches subdirectories, so it found that
    /// archive first, before `INIZH.big` in name order, and it kept the path, exactly as upstream would. The prediction
    /// was wrong about the layout, not the code.)
    fn game() -> FileSystem<MemoryDevice> {
        let mut d = MemoryDevice::new();
        d.insert(b"ZH\\INIZH.big", build_big(&[(b"Data\\INI\\Weapon.ini", b"zh weapons"), (b"Data\\INI\\Object\\Tank.ini", b"zh tank")]));
        d.insert(b"ZH\\Data\\Maps\\Extra.big", build_big(&[(b"Maps\\Alpine\\Alpine.map", b"map")]));
        d.insert(b"ZH\\Data\\INI\\GameData.ini", b"loose game data".to_vec());
        d.insert(b"Generals\\INI.big", build_big(&[(b"Data\\INI\\Weapon.ini", b"generals weapons"), (b"Data\\INI\\Armor.ini", b"armor")]));
        d.insert(b"ZH\\notes.txt", b"x".to_vec());
        let mut fs = FileSystem::new(d);
        fs.local.game_dir = b"ZH\\".to_vec();
        fs.init(Some(b"\\Generals\\"));
        fs
    }

    #[test]
    fn the_file_system_reads_local_then_archives_in_load_order() {
        let fs = game();
        assert!(fs.archive_errors.is_empty(), "{:?}", fs.archive_errors);
        assert_eq!(fs.open_file(b"data\\ini\\weapon.ini").as_deref(), Some(&b"zh weapons"[..]), "the first archive keeps the path");
        assert_eq!(fs.open_file(b"Data/INI/Armor.ini").as_deref(), Some(&b"armor"[..]), "only Generals has it");
        assert_eq!(fs.open_file(b"Maps\\Alpine\\Alpine.map").as_deref(), Some(&b"map"[..]), "an archive in a subdirectory");
        assert_eq!(fs.open_file(b"Data\\INI\\GameData.ini").as_deref(), Some(&b"loose game data"[..]), "a loose file");
        assert!(fs.does_file_exist(b"DATA\\INI\\OBJECT\\TANK.INI"));
        assert!(!fs.does_file_exist(b"Data\\INI\\Missing.ini"));
        assert_eq!(fs.get_file_info(b"Data\\INI\\Weapon.ini"), Some(10));
        let ini: Vec<String> = fs.list_ini(b"Data\\INI").iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect();
        assert_eq!(ini, ["Data\\INI\\armor.ini", "Data\\INI\\GameData.ini", "Data\\INI\\object\\tank.ini", "Data\\INI\\weapon.ini"]);
        // no archive is held whole
        for name in [&b"INIZH.big"[..], b"Data\\Maps\\Extra.big", b"\\Generals\\INI.big"] {
            let a = fs.archives.archive(name).unwrap_or_else(|| panic!("{}", String::from_utf8_lossy(name)));
            assert!(!a.members_in_memory());
        }
    }

    /// A loose file overrides an archived one of the same path, as upstream's local-first `openFile`.
    #[test]
    fn a_loose_file_overrides_the_archive() {
        let mut fs = game();
        fs.local.device.insert(b"ZH\\Data\\INI\\Weapon.ini", b"modded".to_vec());
        assert_eq!(fs.open_file(b"Data\\INI\\Weapon.ini").as_deref(), Some(&b"modded"[..]));
    }

    /// The directory-only open gives the same tree as the whole-archive open, also when the directory is longer than
    /// the first 64 KiB read; a bad archive is reported and skipped.
    #[test]
    fn directory_only_archives_match_whole_ones() {
        use crate::formats::big::ArchiveFile;
        let names: Vec<Vec<u8>> = (0..3000).map(|i| format!("Art\\Textures\\texture_with_a_long_name_{i:05}.dds").into_bytes()).collect();
        let members: Vec<(&[u8], &[u8])> = names.iter().map(|n| (n.as_slice(), &b"abc"[..])).collect();
        let big = build_big(&members);
        assert!(big.len() > 64 * 1024 * 2, "the directory needs more than one read");
        let whole = ArchiveFile::open_archive_file(b"T.big", big.clone()).unwrap();
        let mut reads = 0;
        let dir = ArchiveFile::open_archive_directory(b"T.big", big.len(), |o, l| {
            reads += 1;
            big[o..(o + l).min(big.len())].to_vec()
        })
        .unwrap();
        assert!(reads >= 2, "{reads}");
        assert_eq!(format!("{:?}", dir.root_directory()), format!("{:?}", whole.root_directory()));
        let r = dir.member_range(b"art\\textures\\texture_with_a_long_name_02999.dds").unwrap();
        assert_eq!(&big[r.0..r.0 + r.1], b"abc");

        let mut d = MemoryDevice::new();
        d.insert(b"Broken.big", b"NOPE".to_vec());
        let mut fs = FileSystem::new(d);
        fs.init(None);
        assert_eq!(fs.archive_errors, vec![(b"Broken.big".to_vec(), BigError::BadIdentifier)]);
    }

    /// The page's fetch of an archive's directory (`web/index.html`): the first 64 KiB, then as many as
    /// [`ArchiveFile::directory_prefix_needed`](crate::formats::big::ArchiveFile::directory_prefix_needed) says, until
    /// it says no more. Returns the prefix and how many reads it took.
    fn page_prefix(big: &[u8]) -> (Vec<u8>, usize) {
        use crate::formats::big::ArchiveFile;
        let mut p = big.len().min(64 * 1024);
        let mut reads = 0;
        loop {
            reads += 1;
            let need = ArchiveFile::directory_prefix_needed(big.len(), &big[..p]);
            if need <= p {
                return (big[..p].to_vec(), reads);
            }
            p = need;
        }
    }

    /// GitHub #24: the ranged device (the browser's) against the memory device, on one synthetic archive of 3,000 small
    /// members (a 160 KiB directory), a 1 MiB member and a 300 KiB one after them, and a loose file known by length only.
    ///
    /// *Predictions, written before the first run:*
    /// - the page's directory fetch takes two reads (64 KiB is short of the directory, 256 KiB is enough), and the prefix
    ///   is 262,144 bytes; with it the archive mounts with no error and the same directory tree as the memory device's;
    /// - a small member lying inside that prefix is served at once, with nothing wanted;
    /// - the 300 KiB member is not: its open gives `None`, one wanted range equal to its `member_location`, and one
    ///   unserved read; opened again before its bytes arrive, nothing new is wanted (asked once) and a second unserved
    ///   read is counted; `file_ready` is false;
    /// - once the page provides the range, every member and the loose file read through the ranged path equal the memory
    ///   device's byte for byte, and `file_ready` is true;
    /// - the 1 MiB member never opened is never fetched: the device holds the prefix, the 300 KiB member and the loose
    ///   file (262,144 + 307,200 + 5 bytes), under half the archive.
    ///
    /// *Results (2026-11-05):* all held on the first run (the assertions below are the predictions).
    #[test]
    fn the_ranged_device_reads_what_is_used_and_matches_the_memory_device() {
        let names: Vec<Vec<u8>> = (0..3000).map(|i| format!("Art\\Textures\\texture_with_a_long_name_{i:05}.dds").into_bytes()).collect();
        let big_member: Vec<u8> = (0..1024 * 1024).map(|i| (i % 251) as u8).collect();
        let mid_member: Vec<u8> = (0..300 * 1024).map(|i| (i % 241) as u8).collect();
        let mut members: Vec<(&[u8], &[u8])> = names.iter().map(|n| (n.as_slice(), &b"abc"[..])).collect();
        members.push((b"Art\\W3D\\huge.w3d", &big_member));
        members.push((b"Art\\W3D\\mid.w3d", &mid_member));
        let big = build_big(&members);
        let (prefix, reads) = page_prefix(&big);
        assert_eq!((prefix.len(), reads), (262_144, 2));

        let mut mem = MemoryDevice::new();
        mem.insert(b"ZH\\W3DZH.big", big.clone());
        mem.insert(b"ZH\\Art\\Textures\\loose.tga", b"loose".to_vec());
        let mut mem = FileSystem::new(mem);
        mem.local.game_dir = b"ZH\\".to_vec();
        mem.init(None);

        let mut dev = RangedDevice::new();
        dev.insert_ranged(b"ZH\\W3DZH.big", big.len(), 0, prefix);
        dev.insert_ranged(b"ZH\\Art\\Textures\\loose.tga", 5, 1, Vec::new());
        let mut fs = FileSystem::new(dev);
        fs.local.game_dir = b"ZH\\".to_vec();
        fs.init(None);
        assert!(fs.archive_errors.is_empty(), "{:?}", fs.archive_errors);
        let a = fs.archives.archive(b"W3DZH.big").expect("mounted");
        assert_eq!(format!("{:?}", a.root_directory()), format!("{:?}", mem.archives.archive(b"W3DZH.big").unwrap().root_directory()));

        // a small member inside the prefix: at once
        assert_eq!(fs.open_file(b"art\\textures\\texture_with_a_long_name_00007.dds").as_deref(), Some(&b"abc"[..]));
        assert!(fs.local.device.take_wanted().is_empty());

        // the 300 KiB member: wanted once, then served
        let (_, off, size) = fs.archives.member_location(b"Art\\W3D\\mid.w3d").unwrap();
        let unserved = fs.local.device.unserved_reads();
        assert_eq!(fs.open_file(b"Art\\W3D\\mid.w3d"), None);
        assert_eq!(fs.local.device.take_wanted(), vec![(0, off, size)]);
        assert_eq!(fs.open_file(b"Art\\W3D\\mid.w3d"), None);
        assert!(fs.local.device.take_wanted().is_empty(), "asked once");
        assert_eq!(fs.local.device.unserved_reads(), unserved + 2);
        assert!(!fs.file_ready(b"Art\\W3D\\mid.w3d"));
        assert_eq!(fs.local.device.outstanding(), 1);
        assert!(fs.local.device.provide(0, off, big[off..off + size].to_vec()));
        assert_eq!(fs.local.device.outstanding(), 0);
        assert!(fs.file_ready(b"Art\\W3D\\mid.w3d"));

        // the loose file: wanted, then served (the page provides whatever is wanted, as `web/index.html` does)
        assert!(!fs.file_ready(b"Art\\Textures\\loose.tga"));
        for (key, o, l) in fs.local.device.take_wanted() {
            assert_eq!((key, o, l), (1, 0, 5));
            fs.local.device.provide(key, o, b"loose"[o..o + l].to_vec());
        }
        for path in [&b"Art\\W3D\\mid.w3d"[..], b"Art\\Textures\\loose.tga", b"art\\textures\\texture_with_a_long_name_02999.dds"] {
            assert_eq!(fs.open_file(path), mem.open_file(path), "{}", String::from_utf8_lossy(path));
            assert!(fs.open_file(path).is_some());
        }
        let s = fs.local.device.stats();
        assert_eq!(s.total_len, big.len() + 5);
        assert_eq!(s.held, 262_144 + 307_200 + 5);
        assert_eq!((s.ranges_arrived, s.bytes_arrived), (2, 307_205));
        assert!(s.held * 2 < big.len(), "held {} of {}", s.held, big.len());
    }

    /// The native device finds files without case, as a Windows install does.
    #[test]
    fn the_native_device_ignores_case() {
        let root = std::env::temp_dir().join(format!("rts_fs_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Data").join("INI")).unwrap();
        std::fs::write(root.join("Data").join("INI").join("Weapon.ini"), b"w").unwrap();
        std::fs::write(root.join("W3DZH.big"), build_big(&[(b"Art\\W3D\\Tank.w3d", b"model")])).unwrap();
        let mut fs = FileSystem::new(NativeDevice::new(&root));
        fs.init(None);
        assert_eq!(fs.open_file(b"DATA\\ini\\weapon.INI").as_deref(), Some(&b"w"[..]));
        assert_eq!(fs.open_file(b"art\\w3d\\tank.w3d").as_deref(), Some(&b"model"[..]));
        let _ = std::fs::remove_dir_all(&root);
    }
}
