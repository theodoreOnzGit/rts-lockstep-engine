// SPDX-License-Identifier: GPL-3.0-only
//! The player's Zero Hour files, and the game made from them (issue #3; milestone C3).
//!
//! **The files stay the player's.** In the browser, the page (`web/index.html`) lets the player pick their Zero Hour
//! folder; it keeps the archives the logic reads in that browser's own storage (OPFS) and hands them here with
//! [`import_file`] and [`import_done`]. Nothing is uploaded: the page has no server side. Natively, `rts-client --zh
//! <folder>` reads them from disk. No EA file is in this repository or its build (`AGENTS.md`).
//!
//! **What is read.** `rts_lockstep_engine`'s `World::from_install` loads the game text, every INI the logic reads and the
//! map cache from the archives, as `GameEngine::init` does; `World::quick_skirmish` starts a skirmish on the first
//! multiplayer map, the player against a medium computer player. The art (models, textures), the audio and the GUI's
//! layouts are not read yet (issues #10 to #13): units are drawn as their collision shapes.

use std::cell::RefCell;

use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice, MemoryDevice};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::install_world::{QuickSkirmish, DEFAULT_LANGUAGE};

/// The game's folder on the import's device.
const GAME_DIR: &[u8] = b"ZH\\";

/// A game made from the player's files.
pub struct Started {
    pub world: World,
    /// The local player's index.
    pub player: i32,
    pub game: QuickSkirmish,
    /// What the load found, for the status line.
    pub report: String,
}

/// The imports waiting for the application, set by the page.
#[derive(Default)]
struct Pending {
    files: Vec<(Vec<u8>, Vec<u8>)>,
    ready: bool,
}

thread_local! {
    static PENDING: RefCell<Pending> = RefCell::new(Pending::default());
}

/// A file of the player's folder: `path` relative to the Zero Hour folder (`INIZH.big`, `Data/Scripts/x.scb`), with
/// either slash.
pub fn add_file(path: &str, bytes: Vec<u8>) {
    let p: Vec<u8> = path.bytes().map(|c| if c == b'/' { b'\\' } else { c }).collect();
    PENDING.with(|q| q.borrow_mut().files.push((p, bytes)));
}

/// The import is complete: the application starts the game on its next frame.
pub fn finish_import() {
    PENDING.with(|q| q.borrow_mut().ready = true);
}

/// The files of a completed import, once (the application's poll).
pub fn take_import() -> Option<Vec<(Vec<u8>, Vec<u8>)>> {
    PENDING.with(|q| {
        let mut q = q.borrow_mut();
        if !q.ready {
            return None;
        }
        q.ready = false;
        Some(std::mem::take(&mut q.files))
    })
}

/// The page's call: one file of the player's folder.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn import_file(path: String, bytes: Vec<u8>) {
    add_file(&path, bytes);
}

/// The page's call: every file is in.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn import_done() {
    finish_import();
}

/// A skirmish from the files of a Zero Hour folder, held in memory.
pub fn start_from_files(files: Vec<(Vec<u8>, Vec<u8>)>, map: Option<&[u8]>) -> Result<Started, String> {
    let mut d = MemoryDevice::new();
    let n = files.len();
    for (path, bytes) in files {
        d.insert(&[GAME_DIR, &path].concat(), bytes);
    }
    let mut fs = FileSystem::new(d);
    fs.local.game_dir = GAME_DIR.to_vec();
    fs.init(None);
    start_from_fs(&fs, map, n)
}

/// A skirmish from a Zero Hour folder on disk (the native client's `--zh <folder>`).
#[cfg(not(target_arch = "wasm32"))]
pub fn start_from_folder(folder: &std::path::Path, map: Option<&[u8]>) -> Result<Started, String> {
    let mut fs = FileSystem::new(rts_lockstep_engine::common::file_system::NativeDevice::new(folder));
    fs.init(None);
    start_from_fs(&fs, map, 0)
}

fn start_from_fs<D: LocalDevice>(fs: &FileSystem<D>, map: Option<&[u8]>, files: usize) -> Result<Started, String> {
    let (mut world, report) = World::from_install(fs, DEFAULT_LANGUAGE).map_err(|e| format!("the game's data did not load: {e:?}"))?;
    let game = world.quick_skirmish(fs, map, 1).map_err(|e| format!("no skirmish could start: {e:?}"))?;
    // the first frame dispatches `MSG_NEW_GAME`, the next starts the game and makes the players: the local player is the
    // slot-0 human (the second frame in `install_world`'s test; up to 30 allowed)
    let mut player = None;
    for _ in 0..30 {
        world.engine_update().map_err(|e| format!("a starting frame failed: {e:?}"))?;
        player = world.players.local_index();
        if player.is_some() {
            break;
        }
    }
    let player = player.ok_or_else(|| "the game made no local player in 30 frames".to_string())?;
    let report = format!(
        "{} on {} | {} templates, {} maps, {} archive errors{}",
        String::from_utf8_lossy(&game.sides.0),
        game.map_name,
        report.templates,
        report.maps,
        report.archive_errors.len(),
        if files > 0 { format!(", {files} files imported") } else { String::new() }
    );
    Ok(Started { world, player, game, report })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C3: the import queue and its failure report. Predicted: files added are not taken until the import is finished,
    /// then taken once, with `/` turned to `\`; a folder without the game's archives does not start, and says the data did
    /// not load (the starting of a real install is `install_world`'s test, `a_quick_skirmish_starts_on_the_installs_first_map`).
    #[test]
    fn the_import_waits_for_its_end_and_reports_what_failed() {
        add_file("Data/Scripts/x.scb", vec![1, 2]);
        assert!(take_import().is_none());
        finish_import();
        let files = take_import().expect("the files");
        assert_eq!(files, vec![(b"Data\\Scripts\\x.scb".to_vec(), vec![1, 2])]);
        assert!(take_import().is_none(), "taken once");
        let err = start_from_files(files, None).err().expect("no game from a folder without its archives");
        assert!(err.starts_with("the game's data did not load"), "{err}");
    }
}
