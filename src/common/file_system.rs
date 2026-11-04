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
        let mut prefix = normalise(dir);
        if !prefix.is_empty() && prefix.last() != Some(&b'\\') {
            prefix.push(b'\\');
        }
        let mut out: std::collections::BTreeMap<Vec<u8>, (Vec<u8>, bool)> = std::collections::BTreeMap::new();
        for (k, (orig, _)) in &self.files {
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
