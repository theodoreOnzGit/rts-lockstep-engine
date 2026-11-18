//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/Common/System/FileSystem.cpp   (`FileSystem::loadMusicFilesFromCD`)
//!   GeneralsMD/Code/GameEngine/Source/Common/System/RAMFile.cpp      (`RAMFile::copyDataToFile`)
//!
//! The verified file system has no `File` objects (reads return bytes), so `copyDataToFile` is a function on the
//! bytes and a writer; the CD drives are the host's.
//!
//! Excluded with reasons (#213): `ArchiveFile::attachFile` (ArchiveFile.cpp:190) swaps the archive's open `File`, which
//! the port does not hold (archives are read by range from the device); `Directory::getFiles` / `getSubdirs` return the
//! Windows `Directory` iterator's sets (the port lists a directory through `LocalDevice`); `WSYS_String::makeUpperCase`
//! / `makeLowerCase` are the narrow string class of the Windows file layer (the port uses `AsciiString` / `String`).
//! Also excluded: `AsciiString::freeBytes`, `UnicodeString::releaseBuffer` and `Dict::ensureUnique` / `releaseData` are
//! memory-pool and reference-count plumbing that Rust ownership replaces (see the module notes of `crate::common::dict`).

/// What `loadMusicFilesFromCD` reaches.
pub trait CdHost {
    /// `TheCDManager != NULL`.
    fn has_cd_manager(&self) -> bool;
    /// `TheCDManager->driveCount()`.
    fn drive_count(&self) -> i32;
    /// `TheCDManager->getDrive(i)`'s `getPath()`; `None` for a NULL drive.
    fn drive_path(&self, i: i32) -> Option<Vec<u8>>;
    /// `TheArchiveFileSystem->loadBigFilesFromDirectory(cdRoot, MUSIC_BIG)`.
    fn load_big_files_from_directory(&mut self, root: &[u8], music_big: &str) -> bool;
}

/// `MUSIC_BIG` (ArchiveFileSystem.h:49).
pub const MUSIC_BIG: &str = "Music.big";

/// `FileSystem::loadMusicFilesFromCD()` (FileSystem.cpp:301): the first drive whose root has the music archive.
pub fn load_music_files_from_cd<H: CdHost + ?Sized>(h: &mut H) {
    if !h.has_cd_manager() {
        return;
    }
    let dc = h.drive_count();
    for i in 0..dc {
        let cd_root = match h.drive_path(i) {
            Some(p) => p,
            None => continue,
        };
        if h.load_big_files_from_directory(&cd_root, MUSIC_BIG) {
            break;
        }
    }
}

/// `RAMFile::copyDataToFile(localFile)` (RAMFile.cpp:462): FALSE without a destination; TRUE when the destination
/// accepted all `m_size` bytes (`write` returns the count written).
pub fn copy_data_to_file(data: &[u8], local_file: Option<&mut dyn FnMut(&[u8]) -> usize>) -> bool {
    let f = match local_file {
        Some(f) => f,
        None => return false,
    };
    f(data) == data.len()
}
