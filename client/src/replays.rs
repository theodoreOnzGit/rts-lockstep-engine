// SPDX-License-Identifier: GPL-3.0-only
//! The replay folder (GitHub #98): upstream's `TheRecorder->getReplayDir()`, the user data folder's `Replays\`, which
//! the translated replay menu lists, plays back, deletes and copies from, and into which the score screen's *Save
//! Replay* copies the last replay (`rts_lockstep_engine::client::replay_menu`, `popup_replay`). Plain Rust, no Bevy.
//!
//! **Where it lives.** Natively the user data folder's `Replays` ([`native_replay_dir`]), so the player's replays from
//! the original game are listed as they are (their version and CRCs differ from this client's: the list shows them in
//! grey, as upstream shows another build's). In the browser OPFS `Replays/`, which the page (`web/index.html`) reads at
//! start and writes and deletes as the game asks ([`page`]'s queues), as it does the saves. Nothing is uploaded.
//!
//! **The last replay.** Upstream's recorder writes `00000000.rep` into the folder while a game is played. The client's
//! recorder is the logic's, which writes to the World's own disk; when a game ends with the score screen, the client
//! copies that file into the folder ([`ReplayFolder::write`] of the World's `last_replay`), so the folder holds it when
//! *Save Replay* and the replay menu read it. A game left without its score screen leaves no `00000000.rep` (upstream's
//! file would hold the game so far): a difference.
//!
//! **Copy to the desktop** (`copyReplay`, `CSIDL_DESKTOPDIRECTORY`): natively the home folder's `Desktop`, in the
//! browser a download.
//!
//! **Sub-folders** (upstream lists them, `getFileListInDirectory(..., TRUE)`) are not read: only the folder's own
//! `.rep` files.

use std::collections::BTreeMap;

/// The browser page's side of the folder: the files it read from OPFS `Replays/`, and those the game writes, deletes
/// and exports.
pub mod page {
    use std::cell::RefCell;

    #[derive(Default)]
    pub(crate) struct Queues {
        pub incoming: Vec<(String, Vec<u8>)>,
        pub writes: Vec<(String, Vec<u8>)>,
        pub deletes: Vec<String>,
        pub downloads: Vec<(String, Vec<u8>)>,
    }

    thread_local! {
        pub(crate) static Q: RefCell<Queues> = RefCell::new(Queues::default());
    }

    /// A replay of OPFS `Replays/`, read at start.
    pub fn add(name: &str, bytes: Vec<u8>) {
        Q.with(|q| q.borrow_mut().incoming.push((name.to_string(), bytes)));
    }

    /// The next file to write, once.
    pub fn take_write() -> Option<(String, Vec<u8>)> {
        Q.with(|q| {
            let mut q = q.borrow_mut();
            (!q.writes.is_empty()).then(|| q.writes.remove(0))
        })
    }

    /// The next file to delete, once.
    pub fn take_delete() -> Option<String> {
        Q.with(|q| {
            let mut q = q.borrow_mut();
            (!q.deletes.is_empty()).then(|| q.deletes.remove(0))
        })
    }

    /// The next download, once.
    pub fn take_download() -> Option<(String, Vec<u8>)> {
        Q.with(|q| {
            let mut q = q.borrow_mut();
            (!q.downloads.is_empty()).then(|| q.downloads.remove(0))
        })
    }

    /// The page's calls (the browser build).
    #[cfg(target_arch = "wasm32")]
    mod js {
        use wasm_bindgen::prelude::wasm_bindgen;

        /// A replay read from OPFS `Replays/` at start.
        #[wasm_bindgen]
        pub fn replays_add(name: String, bytes: Vec<u8>) {
            super::add(&name, bytes);
        }

        /// The next replay to write to OPFS `Replays/`: its name (`""`: none); then [`replays_write_bytes`].
        #[wasm_bindgen]
        pub fn replays_write_name() -> String {
            super::Q.with(|q| q.borrow().writes.first().map(|(n, _)| n.clone()).unwrap_or_default())
        }

        /// The bytes of the file [`replays_write_name`] named, taken.
        #[wasm_bindgen]
        pub fn replays_write_bytes() -> Vec<u8> {
            super::take_write().map(|(_, b)| b).unwrap_or_default()
        }

        /// The next replay to delete from OPFS `Replays/` (`""`: none), taken.
        #[wasm_bindgen]
        pub fn replays_delete_name() -> String {
            super::take_delete().unwrap_or_default()
        }

        /// The next replay to offer as a download (the replay menu's copy to the desktop): its name (`""`: none);
        /// then [`replays_download_bytes`].
        #[wasm_bindgen]
        pub fn replays_download_name() -> String {
            super::Q.with(|q| q.borrow().downloads.first().map(|(n, _)| n.clone()).unwrap_or_default())
        }

        /// The bytes of the download [`replays_download_name`] named, taken.
        #[wasm_bindgen]
        pub fn replays_download_bytes() -> Vec<u8> {
            super::take_download().map(|(_, b)| b).unwrap_or_default()
        }
    }
}

/// The replay folder natively: the user data folder's `Replays` (`getReplayDir()`).
#[cfg(not(target_arch = "wasm32"))]
pub fn native_replay_dir() -> Option<std::path::PathBuf> {
    crate::user_data::native_user_data_dir().map(|d| d.join("Replays"))
}

/// The desktop folder natively (`CSIDL_DESKTOPDIRECTORY`): the home folder's `Desktop`.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_desktop_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(std::path::PathBuf::from(home).join("Desktop"))
}

/// A replay's name: a plain name ending in `.rep` (any case), no folder.
pub fn is_replay_name(name: &str) -> bool {
    name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(".rep") && !name.contains(['/', '\\'])
}

/// The `FormatMessage`-like text of an error (the text and CR LF, as `FormatMessage` ends it).
#[cfg(not(target_arch = "wasm32"))]
fn error_text(e: impl std::fmt::Display) -> String {
    format!("{e}\r\n")
}

/// The replay folder's files by name, natively the folder they are written to, and how many times it has changed
/// (for the shell's copy).
#[derive(Clone, Debug, Default)]
pub struct ReplayFolder {
    /// Natively, the folder on disk; `None` in the browser (the page writes OPFS) and in tests.
    pub dir: Option<std::path::PathBuf>,
    /// Natively, the desktop folder (`copyReplay`'s); `None`: a download (the browser) or nowhere (tests).
    pub desktop: Option<std::path::PathBuf>,
    pub files: BTreeMap<String, Vec<u8>>,
    pub generation: u64,
    /// The exports made in tests (natively without a desktop folder, or in the browser: the download queue).
    pub exported: Vec<String>,
}

impl ReplayFolder {
    /// The folder on disk, with the `.rep` files it holds (a missing folder is empty: it is made on the first write).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native(dir: std::path::PathBuf) -> Self {
        let mut files = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if is_replay_name(&name) {
                    if let Ok(b) = std::fs::read(e.path()) {
                        files.insert(name, b);
                    }
                }
            }
        }
        ReplayFolder { dir: Some(dir), desktop: native_desktop_dir(), files, generation: 0, exported: Vec::new() }
    }

    /// The folder the client starts with: natively [`native_replay_dir`], in the browser the page's (empty until it
    /// hands the files over: [`ReplayFolder::take_page_files`]).
    pub fn start() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            native_replay_dir().map_or_else(ReplayFolder::default, ReplayFolder::native)
        }
        #[cfg(target_arch = "wasm32")]
        {
            ReplayFolder::default()
        }
    }

    /// The files the page handed over since the last call; how many.
    pub fn take_page_files(&mut self) -> usize {
        let incoming = page::Q.with(|q| std::mem::take(&mut q.borrow_mut().incoming));
        let n = incoming.len();
        for (name, bytes) in incoming {
            if is_replay_name(&name) {
                self.files.insert(name, bytes);
                self.generation += 1;
            }
        }
        n
    }

    /// The file `name` of the folder, by its name without case (as Windows finds it).
    fn key(&self, name: &str) -> Option<String> {
        self.files.keys().find(|k| k.eq_ignore_ascii_case(name)).cloned()
    }

    /// A replay written (`CopyFile` into the folder, or the recorder's last replay): kept under the spelling a file of
    /// that name already has, and written to the folder natively, or to OPFS by the page.
    pub fn write(&mut self, name: &str, bytes: Vec<u8>) -> Result<(), String> {
        if !is_replay_name(name) {
            return Err(format!("not a replay of the folder: {name}"));
        }
        let name = self.key(name).unwrap_or_else(|| name.to_string());
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(dir) = &self.dir {
            std::fs::create_dir_all(dir).map_err(|e| format!("the replay folder {}: {}", dir.display(), error_text(e)))?;
            std::fs::write(dir.join(&name), &bytes).map_err(error_text)?;
        }
        #[cfg(target_arch = "wasm32")]
        page::Q.with(|q| q.borrow_mut().writes.push((name.clone(), bytes.clone())));
        self.files.insert(name, bytes);
        self.generation += 1;
        Ok(())
    }

    /// A replay deleted (`DeleteFile`).
    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        let Some(name) = self.key(name) else { return Err(format!("no replay {name}")) };
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(dir) = &self.dir {
            std::fs::remove_file(dir.join(&name)).map_err(error_text)?;
        }
        #[cfg(target_arch = "wasm32")]
        page::Q.with(|q| q.borrow_mut().deletes.push(name.clone()));
        self.files.remove(&name);
        self.generation += 1;
        Ok(())
    }

    /// A replay copied to the desktop (`copyReplay`): natively into the desktop folder, in the browser a download.
    pub fn export(&mut self, name: &str, bytes: Vec<u8>) -> Result<String, String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(dir) = &self.desktop {
            let to = dir.join(name);
            std::fs::write(&to, &bytes).map_err(error_text)?;
            return Ok(to.display().to_string());
        }
        #[cfg(target_arch = "wasm32")]
        page::Q.with(|q| q.borrow_mut().downloads.push((name.to_string(), bytes.clone())));
        let _ = bytes;
        self.exported.push(name.to_string());
        Ok(format!("{name} (download)"))
    }
}

/// The name of a replay folder path of the shell (`Replays\<name>`).
pub fn name_of_path(path: &[u8]) -> String {
    let leaf = match path.iter().rposition(|&c| c == b'\\') {
        Some(i) => &path[i + 1..],
        None => path,
    };
    String::from_utf8_lossy(leaf).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #98: the folder on disk, predicted before the run: `.rep` files read (any case), others left; a write kept on
    /// disk under the spelling the folder has (`MINE.rep` writes `Mine.REP`); a delete removes the file; an export
    /// goes to the desktop folder given; a path with a folder is refused; the generation counts the changes (3).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_native_folder_reads_writes_deletes_and_exports() {
        let root = std::env::temp_dir().join(format!("rts_replays_{}", std::process::id()));
        let dir = root.join("Replays");
        let desk = root.join("Desktop");
        std::fs::create_dir_all(&dir).expect("folder");
        std::fs::create_dir_all(&desk).expect("desktop");
        std::fs::write(dir.join("Mine.REP"), b"A").expect("write");
        std::fs::write(dir.join("notes.txt"), b"x").expect("write");
        let mut f = ReplayFolder::native(dir.clone());
        f.desktop = Some(desk.clone());
        assert_eq!(f.files.keys().cloned().collect::<Vec<_>>(), vec!["Mine.REP".to_string()]);
        f.write("MINE.rep", b"B".to_vec()).expect("written");
        assert_eq!(std::fs::read(dir.join("Mine.REP")).expect("on disk"), b"B");
        f.write("00000000.rep", b"L".to_vec()).expect("written");
        f.delete("mine.rep").expect("deleted");
        assert!(!dir.join("Mine.REP").exists());
        assert!(f.delete("mine.rep").is_err());
        f.export("00000000.rep", b"L".to_vec()).expect("exported");
        assert_eq!(std::fs::read(desk.join("00000000.rep")).expect("on the desktop"), b"L");
        assert!(f.write("a\\b.rep", Vec::new()).is_err());
        assert_eq!(f.generation, 3);
        assert_eq!(name_of_path(b"Replays\\x.rep"), "x.rep");
        let _ = std::fs::remove_dir_all(&root);
    }
}
