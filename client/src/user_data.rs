// SPDX-License-Identifier: GPL-3.0-only
//! The user data folder's preference files (GitHub #93): `Options.ini`, `Skirmish.ini`, `Network.ini`, ... that the
//! translated `UserPreferences::load` and `write` (`rts_lockstep_engine::client::user_preferences`) read and write.
//! Plain Rust, no Bevy.
//!
//! **Where they live.** Upstream's `TheGlobalData->getPath_UserData()`: the folder whose `Save\` holds the saves
//! ([`crate::saves`]). Natively that folder ([`native_user_data_dir`]), so the player's own `Options.ini` from the
//! original game is read as it is (it is written back in the game's text mode, CR LF); in the browser the OPFS root,
//! whose `Save/` holds the saves: the page (`web/index.html`) reads the root's `.ini` files at start and writes the
//! ones the game writes ([`page`]'s queues), the same way as the saves. Nothing is uploaded.
//!
//! **What the shell sees.** The shell's host holds the folder's files by their names (`getPath_UserData()` is empty
//! there, so `load("Options.ini")` opens `Options.ini`); the files it writes come back through
//! [`crate::shell::ShellUi::take_user_data_writes`] and are kept here ([`UserDataFolder::write`]).

use std::collections::BTreeMap;

/// The browser page's side of the folder: the files it read from the OPFS root, and those the game asks it to write.
pub mod page {
    use std::cell::RefCell;

    #[derive(Default)]
    pub(crate) struct Queues {
        /// Files the page read from the OPFS root: (name, bytes).
        pub incoming: Vec<(String, Vec<u8>)>,
        /// Files to write to the OPFS root.
        pub writes: Vec<(String, Vec<u8>)>,
    }

    thread_local! {
        pub(crate) static Q: RefCell<Queues> = RefCell::new(Queues::default());
    }

    /// A file of the OPFS root, read at start.
    pub fn add(name: &str, bytes: Vec<u8>) {
        Q.with(|q| q.borrow_mut().incoming.push((name.to_string(), bytes)));
    }

    /// The next file to write to OPFS, once.
    pub fn take_write() -> Option<(String, Vec<u8>)> {
        Q.with(|q| {
            let mut q = q.borrow_mut();
            (!q.writes.is_empty()).then(|| q.writes.remove(0))
        })
    }

    /// The page's calls (the browser build).
    #[cfg(target_arch = "wasm32")]
    mod js {
        use wasm_bindgen::prelude::wasm_bindgen;

        /// A preference file read from the OPFS root at start.
        #[wasm_bindgen]
        pub fn prefs_add(name: String, bytes: Vec<u8>) {
            super::add(&name, bytes);
        }

        /// The next preference file to write to the OPFS root: its name (`""`: none); then [`prefs_write_bytes`] gives
        /// its bytes.
        #[wasm_bindgen]
        pub fn prefs_write_name() -> String {
            super::Q.with(|q| q.borrow().writes.first().map(|(n, _)| n.clone()).unwrap_or_default())
        }

        /// The bytes of the file [`prefs_write_name`] named, taken.
        #[wasm_bindgen]
        pub fn prefs_write_bytes() -> Vec<u8> {
            super::take_write().map(|(_, b)| b).unwrap_or_default()
        }
    }
}

/// The game's user data folder natively (`GlobalData`'s `m_userDataDir`): `RTS_ZH_USER_DATA` when set (a test's or a
/// player's other folder), else the documents folder (`%USERPROFILE%\Documents` on Windows, `$HOME/Documents`
/// elsewhere) and `Command and Conquer Generals Zero Hour Data`. Upstream reads the leaf name from the registry
/// (`UserDataLeafName`) and falls back to that default.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_user_data_dir() -> Option<std::path::PathBuf> {
    if let Some(d) = std::env::var_os("RTS_ZH_USER_DATA") {
        return Some(std::path::PathBuf::from(d));
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(std::path::PathBuf::from(home).join("Documents").join("Command and Conquer Generals Zero Hour Data"))
}

/// A preference file's name: a plain name ending in `.ini` (any case), no folder.
fn is_pref_name(name: &str) -> bool {
    name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(".ini") && !name.contains(['/', '\\'])
}

/// The folder's preference files by name, and natively the folder they are written to.
#[derive(Clone, Debug, Default)]
pub struct UserDataFolder {
    /// Natively, the folder on disk; `None` in the browser (the page writes OPFS) and in tests.
    pub dir: Option<std::path::PathBuf>,
    pub files: BTreeMap<String, Vec<u8>>,
}

impl UserDataFolder {
    /// The folder on disk, with the `.ini` files it holds (a missing folder is empty: it is made on the first write).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native(dir: std::path::PathBuf) -> Self {
        let mut files = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if is_pref_name(&name) {
                    if let Ok(b) = std::fs::read(e.path()) {
                        files.insert(name, b);
                    }
                }
            }
        }
        UserDataFolder { dir: Some(dir), files }
    }

    /// The folder the client starts with: natively [`native_user_data_dir`], in the browser the page's (empty until
    /// it hands the files over: [`UserDataFolder::take_page_files`]).
    pub fn start() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            native_user_data_dir().map_or_else(UserDataFolder::default, UserDataFolder::native)
        }
        #[cfg(target_arch = "wasm32")]
        {
            UserDataFolder::default()
        }
    }

    /// The files the page handed over since the last call; how many.
    pub fn take_page_files(&mut self) -> usize {
        let incoming = page::Q.with(|q| std::mem::take(&mut q.borrow_mut().incoming));
        let n = incoming.len();
        for (name, bytes) in incoming {
            if is_pref_name(&name) {
                self.files.insert(name, bytes);
            }
        }
        n
    }

    /// A file the game wrote (`fopen(path, "w")`): kept, and written to the folder natively, or to OPFS by the page.
    /// A path with a folder in it (the online service's `GeneralsOnline\`) is refused, as no such file is written here.
    pub fn write(&mut self, name: &str, bytes: Vec<u8>) -> Result<(), String> {
        if !is_pref_name(name) {
            return Err(format!("not a preference file of the folder: {name}"));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(dir) = &self.dir {
            std::fs::create_dir_all(dir).map_err(|e| format!("the user data folder {}: {e}", dir.display()))?;
            std::fs::write(dir.join(name), &bytes).map_err(|e| format!("{name}: {e}"))?;
        }
        #[cfg(target_arch = "wasm32")]
        page::Q.with(|q| q.borrow_mut().writes.push((name.to_string(), bytes.clone())));
        self.files.insert(name.to_string(), bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #93: the folder on disk: the `.ini` files read, others left; a write kept and on disk; a folder path refused.
    /// Predicted: `Options.ini` read, `notes.txt` not; `Skirmish.ini` written to the disk; `a\b.ini` refused.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_native_folder_reads_and_writes_preference_files() {
        let dir = std::env::temp_dir().join(format!("rts_user_data_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("folder");
        std::fs::write(dir.join("Options.ini"), b"MusicVolume = 33\r\n").expect("write");
        std::fs::write(dir.join("notes.txt"), b"x").expect("write");
        let mut f = UserDataFolder::native(dir.clone());
        assert_eq!(f.files.keys().cloned().collect::<Vec<_>>(), vec!["Options.ini".to_string()]);
        f.write("Skirmish.ini", b"FPS = 30\r\n".to_vec()).expect("written");
        assert_eq!(std::fs::read(dir.join("Skirmish.ini")).expect("on disk"), b"FPS = 30\r\n");
        assert!(f.write("a\\b.ini", Vec::new()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
