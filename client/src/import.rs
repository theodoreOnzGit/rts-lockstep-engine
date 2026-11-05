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
//! multiplayer map, the player against a medium computer player. ~~The art (models, textures), the audio and the GUI's
//! layouts are not read yet (issues #10 to #13): units are drawn as their collision shapes.~~ **Corrected 2026-11-05
//! (#24):** the art is read: the drawables' models and textures come from the same files (`drawables`), and an object
//! whose model is not found is drawn as its shape. The audio and the GUI's layouts are not read yet.
//!
//! **Ranged reads (#24).** The art archives run to hundreds of megabytes, so the page does not hand them over whole: it
//! gives each one's length and the bytes of its header and directory ([`import_ranged`]; the page finds how many with
//! [`big_directory_prefix`]), and the art's device ([`RangedDevice`]) asks for each member's range when it is read. The
//! application lists the ranges wanted ([`pump_ranges`]), the page reads them from the player's file and hands them back
//! ([`import_range`]). Every file the page hands over carries a key, its index in the page's list of handles, so the
//! art's device can also read members of the archives the logic was given whole, after those bytes are dropped.

use std::cell::RefCell;

use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice, MemoryDevice, RangedDevice};
use rts_lockstep_engine::formats::big::ArchiveFile;
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

/// A file the page hands over: whole, or by its length and the bytes from its start (#24).
#[derive(Clone, Debug, PartialEq)]
pub enum Imported {
    /// The bytes, and the page's key for the file (`u32::MAX`: none; its bytes cannot be asked for again).
    Whole { key: u32, bytes: Vec<u8> },
    /// The file's length, the page's key, and its first bytes (an archive's header and directory, or none).
    Ranged { key: u32, len: usize, prefix: Vec<u8> },
}

/// The imports waiting for the application, set by the page.
#[derive(Default)]
struct Pending {
    files: Vec<(Vec<u8>, Imported)>,
    ready: bool,
}

thread_local! {
    static PENDING: RefCell<Pending> = RefCell::new(Pending::default());
    /// What became of the last import, for the page: empty while none has finished, `ok: <report>` or `error: <why>`.
    static OUTCOME: RefCell<String> = const { RefCell::new(String::new()) };
    /// The ranges the page has read and not yet given to the art's device: (key, offset, bytes).
    static ARRIVED: RefCell<Vec<(u32, usize, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
    /// The ranges the art's device wants and the page has not taken yet: (key, offset, len).
    static WANTED: RefCell<Vec<(u32, usize, usize)>> = const { RefCell::new(Vec::new()) };
    /// The art's state for the page ([`art_status`]).
    static ART_STATUS: RefCell<String> = const { RefCell::new(String::new()) };
}

/// One application frame's exchange with the page (#24): the ranges that arrived go to the device, and the ranges it
/// wants since the last frame are listed for the page. Returns how many ranges arrived.
pub fn pump_ranges(device: &mut RangedDevice) -> usize {
    let arrived = ARRIVED.with(|a| std::mem::take(&mut *a.borrow_mut()));
    let n = arrived.len();
    for (key, offset, bytes) in arrived {
        device.provide(key, offset, bytes);
    }
    let wanted = device.take_wanted();
    if !wanted.is_empty() {
        WANTED.with(|w| w.borrow_mut().extend(wanted));
    }
    n
}

/// A range the page has read (see [`pump_ranges`]).
pub fn add_range(key: u32, offset: usize, bytes: Vec<u8>) {
    ARRIVED.with(|a| a.borrow_mut().push((key, offset, bytes)));
}

/// The ranges wanted, once each: (key, offset, len).
pub fn take_wanted() -> Vec<(u32, usize, usize)> {
    WANTED.with(|w| std::mem::take(&mut *w.borrow_mut()))
}

/// Sets the art's status for the page.
pub fn set_art_status(s: String) {
    ART_STATUS.with(|a| *a.borrow_mut() = s);
}

/// The art's status (`""` before a game with art; else `models=<objects drawn as models> ...`, see the app).
pub fn art_status_text() -> String {
    ART_STATUS.with(|a| a.borrow().clone())
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
    add_imported(path, Imported::Whole { key: u32::MAX, bytes });
}

/// A file of the player's folder, whole or by range (see [`Imported`]).
pub fn add_imported(path: &str, file: Imported) {
    let p: Vec<u8> = path.bytes().map(|c| if c == b'/' { b'\\' } else { c }).collect();
    PENDING.with(|q| q.borrow_mut().files.push((p, file)));
}

/// The import is complete: the application starts the game on its next frame.
pub fn finish_import() {
    PENDING.with(|q| q.borrow_mut().ready = true);
}

/// The files of a completed import, once (the application's poll).
pub fn take_import() -> Option<Vec<(Vec<u8>, Imported)>> {
    PENDING.with(|q| {
        let mut q = q.borrow_mut();
        if !q.ready {
            return None;
        }
        q.ready = false;
        Some(std::mem::take(&mut q.files))
    })
}

/// The page's call: one file of the player's folder, whole; `key` is the page's index of its handle.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn import_file(path: String, key: u32, bytes: Vec<u8>) {
    add_imported(&path, Imported::Whole { key, bytes });
}

/// The page's call: one file of the player's folder by its length and first bytes, read by range later (#24).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn import_ranged(path: String, key: u32, len: f64, prefix: Vec<u8>) {
    add_imported(&path, Imported::Ranged { key, len: len as usize, prefix });
}

/// The page's call: how many leading bytes of an archive `len` bytes long its directory read needs, given the first
/// `have.len()` ([`ArchiveFile::directory_prefix_needed`]): read that many and ask again while it is more than `have`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn big_directory_prefix(len: f64, have: Vec<u8>) -> f64 {
    ArchiveFile::directory_prefix_needed(len as usize, &have) as f64
}

/// The page's call: the ranges the game wants read, as `[key, offset, len, ...]` (each listed once).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn import_wanted() -> Vec<f64> {
    take_wanted().into_iter().flat_map(|(k, o, l)| [k as f64, o as f64, l as f64]).collect()
}

/// The page's call: a range it has read.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn import_range(key: u32, offset: f64, bytes: Vec<u8>) {
    add_range(key, offset as usize, bytes);
}

/// The page's call: the art's state (see [`art_status_text`]).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn art_status() -> String {
    art_status_text()
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

/// A skirmish from the files of a Zero Hour folder, held in memory: the files given whole on a memory device for the
/// logic's load (dropped after it), and every file on the art's ranged device (#24): an archive by its directory, its
/// members read when they are opened. Returns the game and the art's file system, mounted as the logic's was.
pub fn start_from_files(files: Vec<(Vec<u8>, Imported)>, map: Option<&[u8]>) -> Result<(Started, FileSystem<RangedDevice>), String> {
    let mut d = MemoryDevice::new();
    let mut art = RangedDevice::new();
    let n = files.len();
    let (files, generals) = arrange(files);
    for (path, file) in files {
        match file {
            Imported::Whole { key, bytes } => {
                if key == u32::MAX {
                    art.insert(&path, bytes.clone());
                } else {
                    // the art keeps the archive's directory only, and reads its members back from the page
                    let keep = if is_big(&path) { ArchiveFile::directory_prefix_needed(bytes.len(), &bytes) } else { 0 };
                    art.insert_ranged(&path, bytes.len(), key, bytes[..keep.min(bytes.len())].to_vec());
                }
                d.insert(&path, bytes);
            }
            Imported::Ranged { key, len, prefix } => art.insert_ranged(&path, len, key, prefix),
        }
    }
    let generals = generals.then_some(GENERALS_DIR);
    let mut fs = FileSystem::new(d);
    fs.local.game_dir = GAME_DIR.to_vec();
    fs.init(generals);
    let started = start_from_fs(&fs, map, n)?;
    drop(fs);
    let mut art = FileSystem::new(art);
    art.local.game_dir = GAME_DIR.to_vec();
    art.init(generals);
    Ok((started, art))
}

/// A `.big` file's name.
fn is_big(path: &[u8]) -> bool {
    path.len() >= 4 && path[path.len() - 4..].eq_ignore_ascii_case(b".big")
}

/// The Generals install's folder on the import's device (`Win32BIGFileSystem::init`'s second directory, which upstream
/// reads from the registry).
const GENERALS_DIR: &[u8] = b"\\Generals\\";

/// The picked files on the import's device: the Zero Hour folder is the one holding `INIZH.big`, wherever it is in what
/// was picked (a parent folder holding both games, as Steam's and the Ultimate Collection's layouts do), and a Generals
/// install beside it, the folder holding `INI.big`, becomes the Generals install path. Files in neither are left out:
/// upstream reads no other folder. Without an `INIZH.big` the picked folder is taken as the game's (the page warns).
/// Returns the device's files and whether a Generals install was found.
fn arrange<T>(files: Vec<(Vec<u8>, T)>) -> (Vec<(Vec<u8>, T)>, bool) {
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
        assert_eq!(files, vec![(b"Data\\Scripts\\x.scb".to_vec(), Imported::Whole { key: u32::MAX, bytes: vec![1, 2] })]);
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

    /// #24: the exchange with the page. Predicted: a read of bytes not held lists its range once for the page after a
    /// pump; the range the page hands back reaches the device on the next pump (one arrived), and the read succeeds.
    #[test]
    fn ranges_go_to_the_page_and_back() {
        let mut d = RangedDevice::new();
        d.insert_ranged(b"A.big", 10, 4, Vec::new());
        assert!(d.read_range(b"A.big", 2, 3).is_empty());
        assert_eq!(pump_ranges(&mut d), 0);
        assert_eq!(take_wanted(), vec![(4, 2, 3)]);
        assert!(take_wanted().is_empty());
        add_range(4, 2, vec![7, 8, 9]);
        assert_eq!(pump_ranges(&mut d), 1);
        assert_eq!(d.read_range(b"A.big", 2, 3), vec![7, 8, 9]);
        assert_eq!(take_wanted(), vec![]);
    }

    /// The page's directory fetch (`web/index.html`'s `prefixOf`): 64 KiB, then what `directory_prefix_needed` asks.
    #[cfg(feature = "app")]
    fn page_prefix(big: &[u8]) -> Vec<u8> {
        let mut p = big.len().min(64 * 1024);
        loop {
            let need = ArchiveFile::directory_prefix_needed(big.len(), &big[..p]);
            if need <= p {
                return big[..p].to_vec();
            }
            p = need;
        }
    }

    /// #24, not a check unless asked: with `RTS_ART_INSTALL_FROM=<the test install>` (`install_world`'s
    /// `write_the_test_install_when_asked`) and `RTS_WRITE_ART_INSTALL=<dir>`, writes that install with art into `<dir>`:
    /// its `Units.ini` gives the Tank and the Barracks `Draw = W3DModelDraw` with the generated robot (`Robot`, animated
    /// by `Robot.Wave`), and `W3DZH.big` (a 32 MiB member nobody opens, then `robot.w3d`) and `TexturesZH.big` (the
    /// robot's checker) are added. Then plays it as the browser does: the art archives by length and directory only,
    /// the others whole, every wanted range read back from the files written, for 60 frames.
    ///
    /// *Predictions:* the game starts on Desert Duel; within a few frames every Tank and Barracks (two each) is posed as
    /// `robot`, and the device holds the two archives' first 64 KiB (`TexturesZH.big` is shorter: whole) and
    /// `robot.w3d`'s range, about 1 % of `W3DZH.big`.
    ///
    /// *Results (2026-11-05):* the game started on Desert Duel; 0 models at frame 0, 4 from frame 1 on; one range of
    /// 4,068 bytes read from the 33,558,571-byte `W3DZH.big` (0.012 %, well under the 1 % predicted), 78,071 bytes held
    /// in all (both art archives' prefixes, the other archives' directories, which are whole at these sizes, and the
    /// model).
    #[cfg(feature = "app")]
    #[test]
    fn write_and_play_the_test_install_with_art_when_asked() {
        use crate::drawables::{Art, WorldDrawables};
        use rts_lockstep_engine::formats::big::build_big;
        use rts_lockstep_engine::formats::big::FilenameList;
        let (Ok(from), Ok(to)) = (std::env::var("RTS_ART_INSTALL_FROM"), std::env::var("RTS_WRITE_ART_INSTALL")) else { return };
        let from = std::path::Path::new(&from);
        let to = std::path::Path::new(&to);
        std::fs::create_dir_all(to).expect("the folder");
        let ini_big = std::fs::read(from.join("INIZH.big")).expect("the test install's INIZH.big");
        let a = ArchiveFile::open_archive_file(b"INIZH.big", ini_big).expect("an archive");
        let mut list = FilenameList::new();
        a.get_file_list_in_directory(b"", b"", b"*", &mut list, true);
        let draw = "Object {name}\n Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = Robot\n   Animation = Robot.Wave\n   AnimationMode = LOOP\n  End\n End\n";
        let mut members: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for name in list.iter() {
            let mut bytes = a.open_file(name).expect("a member").to_vec();
            if name.eq_ignore_ascii_case(b"data\\ini\\object\\units.ini") {
                let mut text = String::from_utf8(bytes).expect("text");
                for o in ["Tank", "Barracks"] {
                    let head = format!("Object {o}\n");
                    assert!(text.contains(&head), "{o}");
                    text = text.replacen(&head, &draw.replace("{name}", o), 1);
                }
                bytes = text.into_bytes();
            }
            members.push((name.to_vec(), bytes));
        }
        let refs: Vec<(&[u8], &[u8])> = members.iter().map(|(p, b)| (p.as_slice(), b.as_slice())).collect();
        let mut archives: Vec<(String, Vec<u8>)> = vec![("INIZH.big".into(), build_big(&refs))];
        for n in ["EnglishZH.big", "MapsZH.big", "ScriptsZH.big"] {
            archives.push((n.into(), std::fs::read(from.join(n)).expect(n)));
        }
        let art = crate::model::demo_art_files();
        let filler = vec![0x5au8; 32 << 20];
        archives.push(("W3DZH.big".into(), build_big(&[(b"Art\\W3D\\zzfiller.w3d", &filler), (b"Art\\W3D\\robot.w3d", &art[0].1)])));
        archives.push(("TexturesZH.big".into(), build_big(&[(b"Art\\Textures\\robotchecker.tga", &art[1].1)])));
        for (n, b) in &archives {
            std::fs::write(to.join(n), b).expect("written");
        }

        // the browser's import: the art archives by range, the others whole
        let files: Vec<(Vec<u8>, Imported)> = archives
            .iter()
            .enumerate()
            .map(|(key, (n, b))| {
                let f = if n.starts_with("W3D") || n.starts_with("Textures") {
                    Imported::Ranged { key: key as u32, len: b.len(), prefix: page_prefix(b) }
                } else {
                    Imported::Whole { key: key as u32, bytes: b.clone() }
                };
                (n.as_bytes().to_vec(), f)
            })
            .collect();
        let (mut s, fs) = start_from_files(files, None).expect("the game starts");
        assert!(s.report.contains("Desert Duel"), "{}", s.report);
        let mut d = WorldDrawables::new(Some(Art::new(fs)));
        let mut posed = Vec::new();
        for f in 0..60u32 {
            s.world.logic_update().expect("a frame");
            let ev = s.world.take_client_events();
            d.update(&mut s.world, &ev, f * 33);
            let device = &mut d.art.as_mut().unwrap().fs.local.device;
            for (key, o, l) in device.take_wanted() {
                let b = &archives[key as usize].1;
                device.provide(key, o, b[o..o + l].to_vec());
            }
            posed.push(d.instance_count());
        }
        let stats = d.art.as_ref().unwrap().fs.local.device.stats();
        let w3d = archives.iter().find(|(n, _)| n == "W3DZH.big").unwrap().1.len();
        println!("models posed by frame: {posed:?}\n{stats:?}\nW3DZH.big {w3d} bytes; missing {:?}", d.missing);
        assert_eq!(*posed.last().unwrap(), 4, "two Tanks and two Barracks posed");
        assert!(stats.bytes_arrived * 100 < w3d, "{stats:?}");
    }
}
