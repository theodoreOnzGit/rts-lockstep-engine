//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,001 cases after one draft fix (`new_drive` inserts at the head, as `LList::add` does); the Windows drive calls are the host's (test: tests/differential_draft_cd_manager.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/Common/CDManager.h
//!   GeneralsMD/Code/GameEngine/Source/Common/System/CDManager.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/Win32Device/Common/Win32CDManager.h
//!   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32CDManager.cpp
//!
//! The CD-drive tracker: the list of CD-ROM drives, refreshed every 300 frames. Vestigial in the released game (it only matters for
//! music that came from a disc; `CDDrive::refreshInfo` maps every disc to `UNKNOWN_DISK`). The Windows calls
//! (`GetDriveType`, `GetVolumeInformation`, `TheFileSystem->unloadMusicFilesFromCD`) are the host's: [`CdHost`].

/// `CD::Disk` (the values the .cpp uses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CdDisk {
    /// `CD::UNKNOWN_DISK`.
    UnknownDisk,
    /// `CD::NO_DISK`.
    NoDisk,
}

/// `class CDDrive` (the base's data): path, disk name, disk.
#[derive(Clone, Debug, PartialEq)]
pub struct CdDrive {
    /// `m_drivePath`.
    pub drive_path: Vec<u8>,
    /// `m_diskName`.
    pub disk_name: Vec<u8>,
    /// `m_disk`.
    pub disk: CdDisk,
}

use crate::logic::modules::plant;

/// The OS calls.
pub trait CdHost {
    /// `GetDriveType(path) == DRIVE_CDROM`.
    fn is_cdrom(&mut self, drive_path: &[u8]) -> bool;
    /// `GetVolumeInformation(path, volName, ...)`: the volume name, or `None` when it fails (no disc).
    fn volume_name(&mut self, drive_path: &[u8]) -> Option<Vec<u8>>;
    /// `TheFileSystem->unloadMusicFilesFromCD()`.
    fn unload_music_files_from_cd(&mut self);
}

impl CdDrive {
    /// `CDDrive::CDDrive()`: `m_disk(CD::UNKNOWN_DISK)`, empty name and path.
    pub fn new() -> CdDrive {
        CdDrive { drive_path: Vec::new(), disk_name: Vec::new(), disk: CdDisk::UnknownDisk }
    }

    /// `CDDrive::getPath()`.
    pub fn get_path(&self) -> &[u8] {
        &self.drive_path
    }

    /// `CDDrive::getDiskName()`.
    pub fn get_disk_name(&self) -> &[u8] {
        &self.disk_name
    }

    /// `CDDrive::getDisk()`.
    pub fn get_disk(&self) -> CdDisk {
        self.disk
    }

    /// `CDDrive::setPath(path)`.
    pub fn set_path(&mut self, path: &[u8]) {
        self.drive_path = path.to_vec();
    }

    /// `CDDrive::refreshInfo()` (the base's; the Win32 drive overrides it, see [`CdDrive::win32_refresh_info`]): `m_disk = UNKNOWN_DISK`.
    pub fn refresh_info(&mut self) {
        // map disk names to disk ID
        self.disk = if plant(16931) { CdDisk::NoDisk } else { CdDisk::UnknownDisk };
    }

    /// `Win32CDDrive::refreshInfo()` ("an override, not an extension of CDDrive"): a readable volume sets the name and `UNKNOWN_DISK`;
    /// otherwise the name clears, the disk is `NO_DISK`, and a drive that had a disc unloads the music files from the CD.
    pub fn win32_refresh_info<H: CdHost>(&mut self, host: &mut H) {
        let may_require_update = self.disk != CdDisk::NoDisk || plant(16924);
        // read the volume info
        match host.volume_name(&self.drive_path) {
            Some(name) => {
                self.disk_name = name;
                self.disk = if plant(16926) { CdDisk::NoDisk } else { CdDisk::UnknownDisk };
            }
            None => {
                if !plant(16925) {
                    self.disk_name.clear();
                }
                self.disk = CdDisk::NoDisk;
                if may_require_update {
                    host.unload_music_files_from_cd();
                }
            }
        }
    }
}

impl Default for CdDrive {
    fn default() -> Self {
        CdDrive::new()
    }
}

/// `class CDManager` / `Win32CDManager`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CdManager {
    /// `m_drives`.
    pub drives: Vec<CdDrive>,
}

impl CdManager {
    /// `CDManager::CDManager()`.
    pub fn new() -> CdManager {
        CdManager { drives: Vec::new() }
    }

    /// `Win32CDManager::init()`: `CDManager::init()` (empty), `destroyAllDrives()`, then a drive for every letter `a:\` to `z:\` that
    /// `GetDriveType` calls a CD-ROM, then `refreshDrives()`.
    pub fn init<H: CdHost>(&mut self, host: &mut H) {
        if !plant(16928) {
            self.destroy_all_drives();
        }
        // detect CD Drives
        for drive_letter in b'a'..=(if plant(16927) { b'y' } else { b'z' }) {
            let drive_path = vec![drive_letter, b':', b'\\'];
            if host.is_cdrom(&drive_path) {
                self.new_drive(&drive_path);
            }
        }
        if !plant(16929) {
            self.refresh_drives(host);
        }
    }

    /// `CDManager::update()`: "Every so often, check to make sure the CD is still in the drive": every 300th frame (`% 300 == 299`).
    pub fn update<H: CdHost>(&mut self, host: &mut H, frame: u32) {
        if (if plant(16922) { frame % 300 == 0 } else if plant(16923) { frame % 100 == 99 } else { (frame % 300) == 299 }) {
            self.refresh_drives(host);
        }
    }

    /// `CDManager::reset()`: empty.
    pub fn reset(&mut self) {}

    /// `CDManager::driveCount()`.
    pub fn drive_count(&self) -> i32 {
        self.drives.len() as i32
    }

    /// `CDManager::getDrive(index)`: NULL (`None`) out of range.
    pub fn get_drive(&self, index: i32) -> Option<&CdDrive> {
        if index < 0 && !plant(16921) {
            return None;
        }
        self.drives.get(index.unsigned_abs() as usize)
    }

    /// `CDManager::newDrive(path)`: creates a drive (`createDrive()`), sets its path and adds it with `LList::add`.
    ///
    /// ~~appends it to the list~~ **Corrected 2026-11-20 (#215, group lc2):** `LList::add` (List.cpp) with `m_sortMode == DESCENDING` and
    /// `m_addToEndOfGroup` FALSE inserts the new node BEFORE the first node whose priority is `>=` its own (all drives are priority 0), i.e.
    /// at the HEAD, so the list is in the reverse of detection order (`z:` first). `LList::LList` never initialises `m_addToEndOfGroup`;
    /// the game's `NEW Win32CDManager` is zero-filled by `DynamicMemoryAllocator::allocateBytes`, so it is FALSE in the shipped game.
    pub fn new_drive(&mut self, path: &[u8]) -> &mut CdDrive {
        let mut drive = CdDrive::new();
        drive.set_path(path);
        self.drives.insert(0, drive);
        self.drives.first_mut().unwrap()
    }

    /// `CDManager::refreshDrives()` as `Win32CDManager` runs it: every drive's `Win32CDDrive::refreshInfo()`.
    pub fn refresh_drives<H: CdHost>(&mut self, host: &mut H) {
        for d in self.drives.iter_mut() {
            d.win32_refresh_info(host);
        }
    }

    /// `CDManager::destroyAllDrives()`: unlinks and deletes every drive.
    pub fn destroy_all_drives(&mut self) {
        if !plant(16930) {
            self.drives.clear();
        }
    }
}
