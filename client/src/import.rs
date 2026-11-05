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
use rts_lockstep_engine::logic::world::install_world::{InstallError, QuickSkirmish, DEFAULT_LANGUAGE};

/// An install's failure in words a player can read and report: the INI error's kind and the files the load lacks (the
/// first eight, and how many), the map cache's error, or the map not found.
pub fn describe(e: &InstallError) -> String {
    match e {
        InstallError::Ini { error, missing } => {
            let names: Vec<String> = missing.iter().take(8).map(|m| String::from_utf8_lossy(m).into_owned()).collect();
            let more = if missing.len() > 8 { format!(" and {} more", missing.len() - 8) } else { String::new() };
            if missing.is_empty() {
                format!("an INI file did not parse ({:?})", error.kind)
            } else {
                format!("{:?}: {} game files are missing: {}{more}", error.kind, missing.len(), names.join(", "))
            }
        }
        InstallError::MapCache(m) => format!("the map cache did not load ({m:?})"),
        InstallError::MapNotInCache(m) => format!("no multiplayer map {:?} in the map cache", String::from_utf8_lossy(m)),
    }
}

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
    /// What became of the last import, for the page: empty while none has finished, `ok: <report>` or `error: <why>`.
    static OUTCOME: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Records what became of the import (the application, once it has tried to start the game).
pub fn set_outcome(outcome: Result<&str, &str>) {
    let text = match outcome {
        Ok(r) => format!("ok: {r}"),
        Err(e) => format!("error: {e}"),
    };
    OUTCOME.with(|o| *o.borrow_mut() = text);
}

/// The last import's outcome (see [`set_outcome`]); empty while none has finished.
pub fn outcome() -> String {
    OUTCOME.with(|o| o.borrow().clone())
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

/// The page's call: what became of the import (`""` while it is not done, `ok: ...` or `error: ...`).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn import_outcome() -> String {
    outcome()
}

/// A skirmish from the files of a Zero Hour folder, held in memory.
pub fn start_from_files(files: Vec<(Vec<u8>, Vec<u8>)>, map: Option<&[u8]>) -> Result<Started, String> {
    let mut d = MemoryDevice::new();
    let n = files.len();
    let (files, generals) = arrange(files);
    for (path, bytes) in files {
        d.insert(&path, bytes);
    }
    let mut fs = FileSystem::new(d);
    fs.local.game_dir = GAME_DIR.to_vec();
    fs.init(generals.then_some(GENERALS_DIR));
    start_from_fs(&fs, map, n)
}

/// The Generals install's folder on the import's device (`Win32BIGFileSystem::init`'s second directory, which upstream
/// reads from the registry).
const GENERALS_DIR: &[u8] = b"\\Generals\\";

/// The picked files on the import's device: the Zero Hour folder is the one holding `INIZH.big`, wherever it is in what
/// was picked (a parent folder holding both games, as Steam's and the Ultimate Collection's layouts do), and a Generals
/// install beside it, the folder holding `INI.big`, becomes the Generals install path. Files in neither are left out:
/// upstream reads no other folder. Without an `INIZH.big` the picked folder is taken as the game's (the page warns).
/// Returns the device's files and whether a Generals install was found.
fn arrange(files: Vec<(Vec<u8>, Vec<u8>)>) -> (Vec<(Vec<u8>, Vec<u8>)>, bool) {
    // the folder (with its trailing `\`) of the shallowest file named `name`
    let folder_of = |name: &[u8], outside: Option<&[u8]>| -> Option<Vec<u8>> {
        files
            .iter()
            .map(|(p, _)| p)
            .filter(|p| outside.is_none_or(|o| o.is_empty() || !p.starts_with(o)))
            .filter_map(|p| {
                let cut = p.iter().rposition(|&c| c == b'\\').map_or(0, |i| i + 1);
                p[cut..].eq_ignore_ascii_case(name).then(|| p[..cut].to_vec())
            })
            .min_by_key(|d| (d.iter().filter(|&&c| c == b'\\').count(), d.clone()))
    };
    let Some(zh) = folder_of(b"INIZH.big", None) else {
        return (files.into_iter().map(|(p, b)| ([GAME_DIR, &p].concat(), b)).collect(), false);
    };
    let generals = if zh.is_empty() { None } else { folder_of(b"INI.big", Some(&zh)) };
    let mut out = Vec::with_capacity(files.len());
    for (p, b) in files {
        if p.starts_with(&zh) {
            out.push(([GAME_DIR, &p[zh.len()..]].concat(), b));
        } else if let Some(g) = generals.as_ref().filter(|g| p.starts_with(g)) {
            out.push(([&GENERALS_DIR[1..], &p[g.len()..]].concat(), b));
        }
    }
    (out, generals.is_some())
}

/// A skirmish from a Zero Hour folder on disk (the native client's `--zh <folder>`).
#[cfg(not(target_arch = "wasm32"))]
pub fn start_from_folder(folder: &std::path::Path, map: Option<&[u8]>) -> Result<Started, String> {
    let mut fs = FileSystem::new(rts_lockstep_engine::common::file_system::NativeDevice::new(folder));
    fs.init(None);
    start_from_fs(&fs, map, 0)
}

fn start_from_fs<D: LocalDevice>(fs: &FileSystem<D>, map: Option<&[u8]>, files: usize) -> Result<Started, String> {
    let (mut world, report) = World::from_install(fs, DEFAULT_LANGUAGE).map_err(|e| format!("the game's data did not load: {}", describe(&e)))?;
    let game = world.quick_skirmish(fs, map, 1).map_err(|e| format!("no skirmish could start: {}", describe(&e)))?;
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
    /// not load, which the page reads back as `error: ...` (the starting of a real install is `install_world`'s test, `a_quick_skirmish_starts_on_the_installs_first_map`).
    #[test]
    fn the_import_waits_for_its_end_and_reports_what_failed() {
        assert_eq!(outcome(), "");
        add_file("Data/Scripts/x.scb", vec![1, 2]);
        assert!(take_import().is_none());
        finish_import();
        let files = take_import().expect("the files");
        assert_eq!(files, vec![(b"Data\\Scripts\\x.scb".to_vec(), vec![1, 2])]);
        assert!(take_import().is_none(), "taken once");
        let err = start_from_files(files, None).err().expect("no game from a folder without its archives");
        assert!(err.starts_with("the game's data did not load: CantOpenFile: 31 game files are missing: Data\\INI\\Default\\GameData.ini"), "{err}");
        assert!(err.ends_with("and 23 more"), "{err}");
        set_outcome(Err(&err));
        assert!(outcome().starts_with("error: the game's data did not load"));
    }

    /// C3d: a picked parent folder. Predicted: with Zero Hour and Generals side by side under `Games\`, the folder holding
    /// `INIZH.big` becomes the game's folder (its subfolders kept), the one holding `INI.big` the Generals install path,
    /// and a file in neither is left out; picking the Zero Hour folder itself keeps every file and finds no Generals
    /// install; a pick without `INIZH.big` is taken as the game's folder unchanged.
    #[test]
    fn the_zero_hour_folder_is_found_in_what_was_picked() {
        let f = |p: &str| (p.as_bytes().to_vec(), vec![]);
        let names = |v: Vec<(Vec<u8>, Vec<u8>)>| v.into_iter().map(|(p, _)| String::from_utf8(p).unwrap()).collect::<Vec<_>>();
        let (out, generals) = arrange(vec![
            f("Games\\ZH\\inizh.big"),
            f("Games\\ZH\\Data\\Maps\\M.big"),
            f("Games\\Gen\\INI.big"),
            f("Games\\Gen\\W3D.big"),
            f("Games\\Other\\x.big"),
        ]);
        assert!(generals);
        assert_eq!(names(out), ["ZH\\inizh.big", "ZH\\Data\\Maps\\M.big", "Generals\\INI.big", "Generals\\W3D.big"]);
        let (out, generals) = arrange(vec![f("INIZH.big"), f("Data\\INI\\a.ini")]);
        assert!(!generals);
        assert_eq!(names(out), ["ZH\\INIZH.big", "ZH\\Data\\INI\\a.ini"]);
        let (out, generals) = arrange(vec![f("Sub\\EnglishZH.big")]);
        assert!(!generals);
        assert_eq!(names(out), ["ZH\\Sub\\EnglishZH.big"]);
    }
}
