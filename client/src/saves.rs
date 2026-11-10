// SPDX-License-Identifier: GPL-3.0-only
//! Saved games in the client (GitHub #4): the save folder, the list of saves, and saving and loading a game through the
//! translated save code (`rts_lockstep_engine::client::save_game`, the whole file: the logic's blocks and the client's).
//! Plain Rust, no Bevy: the application's buttons call these, and the tests drive them the same way.
//!
//! **Where saves live.** Upstream keeps them in `GlobalData::getPath_UserData()` + `Save\` (`GameState::getSaveDirectory`):
//! "My Documents" (`SHGetSpecialFolderPath(CSIDL_PERSONAL)`) and the registry's `UserDataLeafName`, by default
//! `Command and Conquer Generals Zero Hour Data`. Natively the client uses that folder ([`native_save_dir`]), so the
//! player's saves from the original game are listed as they are. In the browser it is the OPFS folder `Save/`, which the
//! page (`web/index.html`) reads at start and writes as the game asks ([`page`]'s queues). **A save is never uploaded:**
//! the page has no server side; export is a download to the player's own device, and import reads a file they pick.
//!
//! **The list** is upstream's load menu's information, plainly: each save's description, map label, date and campaign
//! side (`SaveGameInfo`, read from the file's `CHUNK_GameState`), newest first (`SaveDate::isNewerThan`). The game's own
//! save/load menu (the WND layouts) comes with milestone C5.
//!
//! **A save's name** is upstream's: the lowest `%08d.sav` the folder does not have (`GameState::findNextSaveFilename`,
//! translated in the engine's `GameState::find_next_save_filename`).
//!
//! **A load** starts the save's map first, as upstream's load does (`GameStateMap::xfer`'s `startNewGame(TRUE)`): the demo's
//! map for a demo save ([`crate::demo::demo_map_world`]), else ~~the install's World started on the save's map~~
//! **corrected 2026-11-06 (GitHub #58):** a copy of the install's World with no game, in which the save's own
//! `CHUNK_GameStateMap` starts the save's game (its map, game mode and slots) as upstream's does
//! ([`crate::import::world_for_save`]); then every block is read into it and into a fresh client.
//!
//! **The drawables (GitHub #59).** A save writes the client's drawables (`CHUNK_GameClient`, `Drawable::xfer`) and a load
//! makes them, each from its object (or without one) and the save's data, when the caller passes them ([`Drawables`]:
//! the application's, with its art); the ambient sound's part of each is copied from the client's audio before a save
//! (`AmbientSounds::save_into`) and given back to it after a load, once the audio is up (`AmbientSounds::restore_saved`,
//! the application's).

use std::collections::BTreeMap;

use rts_lockstep_engine::client::drawable::ClientDrawables;
use rts_lockstep_engine::client::particle_sys::ParticleSystemManager;
use rts_lockstep_engine::client::w3d_model_draw::W3dAssetSource;
use rts_lockstep_engine::client::save_game::{load_client_game, read_save_summary, save_client_game, ClientSaveState, LoadOutcome, SaveHost, SaveSummary};
use rts_lockstep_engine::common::game_state::{GameState, GameStateHost, SaveDate, SAVE_GAME_EXTENSION};
use rts_lockstep_engine::common::xfer::{Xfer, XferResult};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice};

use crate::input::Controls;

/// The browser page's side of the save folder: the files it read from OPFS for the game, and what the game asks it to
/// write to OPFS or offer as a download. Natively the queues stay empty (the folder is written directly).
pub mod page {
    use std::cell::RefCell;

    #[derive(Default)]
    pub(crate) struct Queues {
        /// Files the page read from OPFS `Save/` (or the player imported), for the folder: (name, bytes, imported).
        pub incoming: Vec<(String, Vec<u8>, bool)>,
        /// Files to write to OPFS `Save/`.
        pub writes: Vec<(String, Vec<u8>)>,
        /// Files to offer as a download.
        pub downloads: Vec<(String, Vec<u8>)>,
        /// (#78) Files to remove from OPFS `Save/` (the shell's save/load screen's delete).
        pub deletes: Vec<String>,
        /// Whether the saves panel is open (the page shows its import button then).
        pub panel_open: bool,
        /// The last save action's outcome, for the page and checks.
        pub status: String,
        /// The logic's frame after the last logic step (for the page's checks: a loaded game runs on from its frame).
        pub frame: u32,
    }

    thread_local! {
        pub(crate) static Q: RefCell<Queues> = RefCell::new(Queues::default());
    }

    /// A file of OPFS `Save/` (`imported`: picked by the player now, so the folder names it and has it written).
    pub fn add(name: &str, bytes: Vec<u8>, imported: bool) {
        Q.with(|q| q.borrow_mut().incoming.push((name.to_string(), bytes, imported)));
    }

    /// The next file to write to OPFS, once.
    pub fn take_write() -> Option<(String, Vec<u8>)> {
        Q.with(|q| {
            let mut q = q.borrow_mut();
            (!q.writes.is_empty()).then(|| q.writes.remove(0))
        })
    }

    /// (#78) The next file to remove from OPFS, once.
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

    pub fn set_panel_open(open: bool) {
        Q.with(|q| q.borrow_mut().panel_open = open);
    }

    pub fn panel_open() -> bool {
        Q.with(|q| q.borrow().panel_open)
    }

    pub fn set_status(s: String) {
        Q.with(|q| q.borrow_mut().status = s);
    }

    pub fn status() -> String {
        Q.with(|q| q.borrow().status.clone())
    }

    pub fn set_frame(frame: u32) {
        Q.with(|q| q.borrow_mut().frame = frame);
    }

    pub fn frame() -> u32 {
        Q.with(|q| q.borrow().frame)
    }

    /// The page's calls (the browser build).
    #[cfg(target_arch = "wasm32")]
    mod exports {
        use wasm_bindgen::prelude::wasm_bindgen;

        /// A save file read from OPFS `Save/` at start.
        #[wasm_bindgen]
        pub fn saves_add(name: String, bytes: Vec<u8>) {
            super::add(&name, bytes, false);
        }

        /// A `.sav` file the player picked to import (it is written to OPFS under the name the folder gives it).
        #[wasm_bindgen]
        pub fn saves_import(name: String, bytes: Vec<u8>) {
            super::add(&name, bytes, true);
        }

        /// The next file to write to OPFS: its name (`""`: none); then [`saves_write_bytes`] gives its bytes.
        #[wasm_bindgen]
        pub fn saves_write_name() -> String {
            super::Q.with(|q| q.borrow().writes.first().map(|w| w.0.clone()).unwrap_or_default())
        }

        /// The bytes of the file [`saves_write_name`] named, taken.
        #[wasm_bindgen]
        pub fn saves_write_bytes() -> Vec<u8> {
            super::take_write().map(|w| w.1).unwrap_or_default()
        }

        /// The next download's name (`""`: none); then [`saves_download_bytes`].
        #[wasm_bindgen]
        pub fn saves_download_name() -> String {
            super::Q.with(|q| q.borrow().downloads.first().map(|w| w.0.clone()).unwrap_or_default())
        }

        #[wasm_bindgen]
        pub fn saves_download_bytes() -> Vec<u8> {
            super::take_download().map(|w| w.1).unwrap_or_default()
        }

        /// (#78) The next file to remove from OPFS `Save/` (`""`: none), taken.
        #[wasm_bindgen]
        pub fn saves_delete_name() -> String {
            super::take_delete().unwrap_or_default()
        }

        /// Whether the saves panel is open.
        #[wasm_bindgen]
        pub fn saves_panel_open() -> bool {
            super::panel_open()
        }

        /// The last save action's outcome (`saved <name>`, `loaded <name>`, `error: ...`).
        #[wasm_bindgen]
        pub fn saves_status() -> String {
            super::status()
        }

        /// The logic's frame after its last step (for checks).
        #[wasm_bindgen]
        pub fn saves_logic_frame() -> u32 {
            super::frame()
        }
    }
}

/// The game's save folder natively (`GlobalData`'s `m_userDataDir` and `GameState::getSaveDirectory`): `RTS_ZH_USER_DATA`
/// when set (a test's or a player's other folder), else the documents folder (`%USERPROFILE%\Documents` on Windows,
/// `$HOME/Documents` elsewhere) and `Command and Conquer Generals Zero Hour Data`, then `Save`. Upstream reads the leaf
/// name from the registry (`UserDataLeafName`) and falls back to that default.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_save_dir() -> Option<std::path::PathBuf> {
    // (#93: the user data folder is the preference files' too: `crate::user_data`)
    crate::user_data::native_user_data_dir().map(|d| d.join("Save"))
}

/// The save folder: the files by name, and natively the directory they are written to.
#[derive(Clone, Debug, Default)]
pub struct SaveFolder {
    /// Natively, the folder on disk; `None` in the browser (the page writes OPFS) and in tests.
    pub dir: Option<std::path::PathBuf>,
    pub files: BTreeMap<String, Vec<u8>>,
}

/// A name ending in `.sav` (any case).
fn is_save_name(name: &str) -> bool {
    name.len() > 4 && name[name.len() - 4..].as_bytes().eq_ignore_ascii_case(SAVE_GAME_EXTENSION)
}

/// One save of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct SaveListing {
    pub name: String,
    /// The file's information, or why it does not read.
    pub summary: Result<SaveSummary, String>,
}

impl SaveListing {
    /// The list's line: the description, the map label, the date and the campaign side (the load menu's columns).
    pub fn line(&self) -> String {
        match &self.summary {
            Err(e) => format!("{}: does not read ({e})", self.name),
            Ok(s) => {
                let i = &s.info;
                let d = &i.date;
                let desc = String::from_utf16_lossy(&i.description);
                let side = if i.campaign_side.is_empty() { String::new() } else { format!(" | {}", String::from_utf8_lossy(&i.campaign_side)) };
                format!(
                    "{} | {} | {:04}-{:02}-{:02} {:02}:{:02}{side}",
                    if desc.is_empty() { self.name.clone() } else { desc },
                    String::from_utf8_lossy(&i.map_label),
                    d.year,
                    d.month,
                    d.day,
                    d.hour,
                    d.minute
                )
            }
        }
    }

    fn date(&self) -> SaveDate {
        self.summary.as_ref().map(|s| s.info.date).unwrap_or_default()
    }
}

/// The folder as `findNextSaveFilename` asks it (`_access` of each candidate in the save directory).
struct FolderHost<'a>(&'a SaveFolder);

impl GameStateHost<()> for FolderHost<'_> {
    fn xfer_block(&mut self, _: (), _: &mut GameState<()>, _: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    fn load_post_process(&mut self, _: ()) {}
    fn partition_update(&mut self) {}
    fn user_data_path(&self) -> Vec<u8> {
        Vec::new()
    }
    fn map_name(&self) -> Vec<u8> {
        Vec::new()
    }
    fn local_time(&mut self) -> [u16; 8] {
        [0; 8]
    }
    fn world_dict_map_name(&self) -> Option<Vec<u8>> {
        None
    }
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)> {
        None
    }
    fn file_exists(&self, path: &[u8]) -> bool {
        // (the path is the save directory's, `Save\` and the leaf)
        let leaf = path.iter().rposition(|&c| c == b'\\').map_or(path, |i| &path[i + 1..]);
        let leaf = String::from_utf8_lossy(leaf);
        self.0.files.keys().any(|k| k.eq_ignore_ascii_case(&leaf))
    }
}

impl SaveFolder {
    /// The folder on disk, with the `.sav` files it holds (a missing folder is empty: it is made on the first save, as
    /// `saveGame`'s `CreateDirectory`).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native(dir: std::path::PathBuf) -> Self {
        let mut files = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if is_save_name(&name) {
                    if let Ok(b) = std::fs::read(e.path()) {
                        files.insert(name, b);
                    }
                }
            }
        }
        SaveFolder { dir: Some(dir), files }
    }

    /// The files the page handed over since the last call (read from OPFS, or imported: those are named here and written
    /// back). Returns how many arrived.
    pub fn take_page_files(&mut self) -> usize {
        let incoming = page::Q.with(|q| std::mem::take(&mut q.borrow_mut().incoming));
        let n = incoming.len();
        for (name, bytes, imported) in incoming {
            if imported {
                let _ = self.import(&name, bytes);
            } else if is_save_name(&name) {
                self.files.insert(name, bytes);
            }
        }
        n
    }

    /// `findNextSaveFilename`: the lowest `%08d.sav` the folder does not have.
    pub fn next_name(&self) -> String {
        let name = GameState::<()>::new().find_next_save_filename(&FolderHost(self));
        String::from_utf8_lossy(&name).into_owned()
    }

    /// A save written: kept, and written to the folder natively, or to OPFS by the page.
    pub fn write(&mut self, name: &str, bytes: Vec<u8>) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(dir) = &self.dir {
            std::fs::create_dir_all(dir).map_err(|e| format!("the save folder {}: {e}", dir.display()))?;
            std::fs::write(dir.join(name), &bytes).map_err(|e| format!("{name}: {e}"))?;
        }
        if self.dir.is_none() {
            page::Q.with(|q| q.borrow_mut().writes.push((name.to_string(), bytes.clone())));
        }
        self.files.insert(name.to_string(), bytes);
        Ok(())
    }

    /// A `.sav` file the player picked: kept under its own name (made `.sav`), or the next free one when the folder has a
    /// file of that name with other contents. Its name in the folder.
    pub fn import(&mut self, name: &str, bytes: Vec<u8>) -> Result<String, String> {
        let leaf = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let mut leaf = if is_save_name(leaf) { leaf.to_string() } else { format!("{leaf}.sav") };
        if self.files.get(&leaf).is_some_and(|b| *b != bytes) {
            leaf = self.next_name();
        }
        self.write(&leaf, bytes)?;
        Ok(leaf)
    }

    /// (#78) A save deleted (the shell's save/load screen: `DeleteFile` of the file in the save directory, whose names
    /// Windows compares without case): gone from the folder, from disk natively, from OPFS by the page. Its name in the
    /// folder, or `None` when there is no such save (upstream's `DeleteFile` then fails, unchecked).
    pub fn delete(&mut self, name: &str) -> Option<String> {
        let key = self.files.keys().find(|k| k.eq_ignore_ascii_case(name))?.clone();
        self.files.remove(&key);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_file(dir.join(&key));
        }
        if self.dir.is_none() {
            page::Q.with(|q| q.borrow_mut().deletes.push(key.clone()));
        }
        Some(key)
    }

    /// A save offered as a download (the browser's page); natively the file is already in the folder, and its path is
    /// returned.
    pub fn export(&self, name: &str) -> Option<String> {
        let bytes = self.files.get(name)?;
        if let Some(dir) = &self.dir {
            return Some(dir.join(name).display().to_string());
        }
        page::Q.with(|q| q.borrow_mut().downloads.push((name.to_string(), bytes.clone())));
        Some(name.to_string())
    }

    /// The list: every save with its information, newest first.
    pub fn listing(&self) -> Vec<SaveListing> {
        let mut out: Vec<SaveListing> = self
            .files
            .iter()
            .map(|(name, b)| {
                let summary = read_save_summary(b.clone()).map(|mut s| {
                    s.map_file = Vec::new();
                    s
                });
                SaveListing { name: name.clone(), summary: summary.map_err(|e| e.to_string()) }
            })
            .collect();
        out.sort_by(|a, b| {
            let (da, db) = (a.date(), b.date());
            if da.is_newer_than(&db) {
                std::cmp::Ordering::Less
            } else if db.is_newer_than(&da) {
                std::cmp::Ordering::Greater
            } else {
                a.name.cmp(&b.name)
            }
        });
        out
    }
}

/// The client's state a save holds besides the World and the radar, kept by the application across frames.
pub type ClientState = ClientSaveState;

/// `GetLocalTime` for a save's date: year, month, day of week, day, hour, minute, second, milliseconds. In the browser the
/// page's local time (`Date`); natively UTC (the standard library has no time zones; the date is shown, never read back
/// into the game).
pub fn local_time_now() -> [u16; 8] {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new_0();
        [d.get_full_year() as u16, d.get_month() as u16 + 1, d.get_day() as u16, d.get_date() as u16, d.get_hours() as u16, d.get_minutes() as u16, d.get_seconds() as u16, d.get_milliseconds() as u16]
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        civil_time(ms)
    }
}

/// A Unix time in milliseconds as `SYSTEMTIME`'s fields (UTC), by the days-to-civil algorithm (H. Hinnant's
/// `civil_from_days`).
pub fn civil_time(ms: i64) -> [u16; 8] {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    // 1970-01-01 was a Thursday (4; Sunday is 0)
    let dow = (days + 4).rem_euclid(7);
    [year as u16, month as u16, dow as u16, day as u16, (rem / 3_600_000) as u16, (rem / 60_000 % 60) as u16, (rem / 1000 % 60) as u16, (rem % 1000) as u16]
}

/// The client's drawables for a save or a load (GitHub #59): the drawables, the art their modules are made from, and the
/// client's clock (milliseconds) they are drawn by.
pub type Drawables<'a> = (&'a mut ClientDrawables, &'a mut dyn W3dAssetSource, u32);

/// The SaveHost of a client, with its drawables when given.
fn host<'a, 'b>(controls: &'a mut Controls, particles: &'a mut ParticleSystemManager, state: &'a mut ClientState, drawables: Option<&'a mut Drawables<'b>>) -> SaveHost<'a> {
    let h = SaveHost::new(&mut controls.client.world, &mut controls.radar.radar, particles, state);
    match drawables {
        Some((d, a, t)) => h.with_drawables(&mut **d, &mut **a, *t),
        None => h,
    }
}

/// The game saved through the client: the World, the radar, the particle systems' id counter, the client's state
/// (`state.view` is the camera, set by the caller) and, since GitHub #59, its drawables when given, with `description`.
/// The file's bytes.
pub fn save_game(controls: &mut Controls, particles: &mut ParticleSystemManager, state: &mut ClientState, mut drawables: Option<Drawables<'_>>, description: &str) -> Result<Vec<u8>, String> {
    state.game_client.frame = controls.client.world.frame();
    state.local_time = local_time_now();
    state.errors.clear();
    let d: Vec<u16> = description.encode_utf16().collect();
    let mut host = host(controls, particles, state, drawables.as_mut());
    let r = save_client_game(&mut host, &d);
    r.map_err(|e| format!("the save failed ({e:?}) {:?} {:?}", state.errors, controls.client.world.cuts.team_live_errors))
}

/// A save's bytes loaded into a client whose World was made for it ([`world_for_save`]) and whose radar has had its new
/// map: every block read, post-processed.
pub fn load_game(controls: &mut Controls, particles: &mut ParticleSystemManager, state: &mut ClientState, mut drawables: Option<Drawables<'_>>, bytes: Vec<u8>) -> Result<LoadOutcome, String> {
    state.errors.clear();
    state.read_past.clear();
    let mut host = host(controls, particles, state, drawables.as_mut());
    load_client_game(&mut host, bytes).map_err(|e| format!("the load failed: {e}"))
}

/// The World a save is loaded into (see the module docs), and the local player when it is known before the load: the
/// demo's map for a demo save (its player), else the install's (`base`, which a demo game has not got), whose game the
/// load starts (its local player known after the load).
pub fn world_for_save(summary: &SaveSummary, base: Option<&World>) -> Result<(World, Option<i32>), String> {
    if summary.pristine_map.eq_ignore_ascii_case(crate::demo::DEMO_MAP_PORTABLE) {
        return Ok((crate::demo::demo_map_world(), Some(crate::demo::DEMO_LOCAL_PLAYER)));
    }
    let base = base.ok_or_else(|| format!("this save is of {:?}: load your Zero Hour files first", String::from_utf8_lossy(&summary.info.map_label)))?;
    Ok((crate::import::world_for_save(base, &summary.pristine_map, &summary.map_file)?, None))
}

/// A client ready for a save: the World made for it ([`world_for_save`]) in fresh controls, a fresh particle manager and
/// state, and the save loaded (for an install's save, the load starts the save's game, and the radar's new map is made
/// then: `client::save_game`'s host); the save's drawables made into `drawables` when given (GitHub #59). Returns them
/// with the local player and the load's outcome.
pub fn client_from_save(bytes: Vec<u8>, base: Option<&World>, drawables: Option<Drawables<'_>>) -> Result<(Controls, ParticleSystemManager, ClientState, i32, LoadOutcome), String> {
    client_from_pending(PendingLoad::new(bytes, base)?, drawables)
}

/// GitHub #64, part 2: a save chosen for loading, waiting for its map's folder. The World made for it
/// ([`world_for_save`]) gets the folder of the save's pristine map (its `map.ini`, `solo.ini`, `map.str`) from the art's
/// files, read by range in the browser, as a mission's start gets it (`World::mount_save_map_when_ready`); until then the
/// load waits. Without it a save of a mission started from the list loaded with the map the save embeds and no `map.ini`.
pub struct PendingLoad {
    pub bytes: Vec<u8>,
    pub world: World,
    /// The local player when it is known before the load (the demo's).
    pub player: Option<i32>,
    /// The files of the map's folder mounted, once they have all arrived.
    pub mounted: Option<usize>,
}

impl PendingLoad {
    /// The save's World ([`world_for_save`]), its folder not yet mounted.
    pub fn new(bytes: Vec<u8>, base: Option<&World>) -> Result<PendingLoad, String> {
        let summary = read_save_summary(bytes.clone()).map_err(|e| format!("the save does not read: {e}"))?;
        let (world, player) = world_for_save(&summary, base)?;
        Ok(PendingLoad { bytes, world, player, mounted: None })
    }

    /// Once a frame: the map's folder read from `fs` and mounted once all of it has arrived (the demo's map has none to
    /// read). Whether it is mounted.
    pub fn mount<D: LocalDevice>(&mut self, fs: &FileSystem<D>) -> Result<bool, String> {
        if self.mounted.is_none() {
            self.mounted = if self.player.is_some() { Some(0) } else { self.world.mount_save_map_when_ready(fs, &self.bytes)? };
        }
        Ok(self.mounted.is_some())
    }
}

/// [`client_from_save`] of a save whose World is ready ([`PendingLoad`]).
pub fn client_from_pending(p: PendingLoad, drawables: Option<Drawables<'_>>) -> Result<(Controls, ParticleSystemManager, ClientState, i32, LoadOutcome), String> {
    let PendingLoad { bytes, world, player, .. } = p;
    let mut controls = Controls::new(world, player.unwrap_or(-1));
    let mut particles = ParticleSystemManager::new();
    let mut state = ClientState::default();
    let outcome = load_game(&mut controls, &mut particles, &mut state, drawables, bytes)?;
    let player = match player {
        Some(p) => p,
        None => controls.client.world.players.local_index().ok_or("the loaded game has no local player")?,
    };
    controls.client.player = player;
    Ok((controls, particles, state, player, outcome))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_lockstep_engine::common::base_type::{Coord3D, ICoord2D};
    use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;

    fn frame(c: &mut Controls) -> u32 {
        c.observe_sent();
        c.world().engine_update().expect("the frame");
        let crc = c.world().logic_crc(CRC_RECALC, None).expect("the CRC");
        let ev = c.world().take_client_events();
        c.take_radar_events(&ev);
        crc
    }

    /// GitHub #4, the native test of the client's saves. The demo, with the player's own order given through the client
    /// (the first Soldier selected by a click, then a move order by a click on the ground), runs
    /// 90 frames, is saved through [`save_game`] into a folder, listed, and loaded from the list into a fresh client twice
    /// ([`client_from_save`]); a third copy is loaded by the World alone (`World::load_game`).
    ///
    /// **Predicted before the run:** the folder names the save `00000000.sav` and the next `00000001.sav`; the list's line
    /// has the description, the gate map's label and the date given; the two client loads give the same logic CRC at each
    /// of the next 150 frames, and so does the World loaded alone; the radar of a loaded client lists the same objects the
    /// saving client's did; the soldier ordered to move is still moving after the load (its y grows). **#59, predicted
    /// before its run:** the client's drawables (one per object, no art) saved with the game come back by object, each
    /// with its template.
    ///
    /// **Results (2026-11-06, native, release):** all held on the second run. **Miss (the test's):** the first run's soldier
    /// stood still after the load (y 65 to 65): it had moved only 5 before the save, because the move was ordered in the same
    /// frame as the selection, before the selection reached the logic; the input's own test runs a frame between them, and
    /// so does this one now (y 60 to 120 before the save, 120 to 269 after the load). The CRCs agreed at all 150 frames on
    /// both runs.
    #[test]
    fn a_demo_game_saved_through_the_client_loads_and_runs_on() {
        let (w, o) = crate::demo::demo_world();
        let mut a = Controls::new(w, crate::demo::DEMO_LOCAL_PLAYER);
        a.radar.radar.force_on(true);
        let s = o.soldiers[0];
        let at = a.client.world.find_object(s).expect("the soldier").get_position();
        let px = ICoord2D { x: 100, y: 100 };
        assert!(a.left_release(px, px, &[s], Some(s), at), "the soldier is selected");
        // (the selection reaches the logic on the next frame, as the input's own test runs it, before the order)
        frame(&mut a);
        let target = Coord3D { x: at.x, y: at.y + 200.0, z: 0.0 };
        let gx = ICoord2D { x: 200, y: 50 };
        assert!(a.left_release(gx, gx, &[], None, target), "the move is ordered");
        for _ in 0..90 {
            frame(&mut a);
        }
        eprintln!("the soldier before the save: y {} -> {} (selected {})", at.y, a.client.world.find_object(s).expect("the soldier").get_position().y, a.ui.select_count());
        let mut particles = ParticleSystemManager::new();
        let mut state = ClientState::default();
        state.view.angle = 0.25;
        // (#59: the client's drawables, made for every object as the client's draw makes them, without art)
        let mut drawables = ClientDrawables::new();
        drawables.draw(&a.client.world, &mut crate::drawables::NoArt, 0);
        let bytes = save_game(&mut a, &mut particles, &mut state, Some((&mut drawables, &mut crate::drawables::NoArt, 0)), "demo at 90").expect("the save");
        let mut folder = SaveFolder::default();
        let name = folder.next_name();
        assert_eq!(name, "00000000.sav");
        folder.write(&name, bytes.clone()).expect("written");
        assert_eq!(folder.next_name(), "00000001.sav");
        let list = folder.listing();
        assert_eq!(list.len(), 1);
        eprintln!("the list: {}", list[0].line());
        assert!(list[0].line().starts_with("demo at 90 | Gate.map | "), "{}", list[0].line());

        let loaded = folder.files[&list[0].name].clone();
        let mut loaded_drawables = ClientDrawables::new();
        let (mut b, _, sb, player, _) = client_from_save(loaded.clone(), None, Some((&mut loaded_drawables, &mut crate::drawables::NoArt, 0))).unwrap_or_else(|e| panic!("{e}"));
        let templates = |d: &ClientDrawables| d.drawables.iter().map(|(id, x)| (*id, x.template.clone())).collect::<Vec<_>>();
        assert_eq!(templates(&loaded_drawables), templates(&drawables), "#59: the save's drawables, by object");
        assert!(!drawables.drawables.is_empty());
        let (mut c, _, _, _, _) = client_from_save(loaded.clone(), None, None).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(player, crate::demo::DEMO_LOCAL_PLAYER);
        assert_eq!(sb.view.angle, 0.25);
        let mut d = crate::demo::demo_map_world();
        d.load_game(loaded).unwrap_or_else(|e| panic!("the World's load: {e:?} {:?}", d.cuts.team_live_errors));
        let ids = |c: &Controls| c.radar.radar.local_object_list.iter().chain(c.radar.radar.object_list.iter()).map(|o| o.object).collect::<Vec<_>>();
        assert_eq!(ids(&b), ids(&a));
        assert!(!ids(&b).is_empty());
        let y0 = b.client.world.find_object(s).expect("the soldier").get_position().y;
        for f in 0..150 {
            let (cb, cc) = (frame(&mut b), frame(&mut c));
            d.engine_update().expect("the frame");
            let cd = d.logic_crc(CRC_RECALC, None).expect("the CRC");
            assert_eq!(cb, cc, "two client loads part at frame {f} after the load");
            assert_eq!(cb, cd, "the client's load and the World's part at frame {f} after the load");
        }
        let y1 = b.client.world.find_object(s).expect("the soldier").get_position().y;
        eprintln!("the soldier: y {y0} -> {y1}");
        assert!(y1 > y0, "the ordered soldier moves on after the load");
        for w in [&b.client.world, &c.client.world, &d] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }
    }

    /// GitHub #4: the folder's queues for the page. Predicted: a file the page read is listed; an import under a name the
    /// folder holds with other bytes is given the next free name and queued for writing to OPFS; an export is queued as a
    /// download; a file that is not a save is listed with why it does not read; the civil date of a known instant.
    /// **Result (2026-11-06):** as predicted, first run.
    #[test]
    fn the_page_queues_and_the_list() {
        let mut f = SaveFolder::default();
        page::add("00000000.sav", b"junk".to_vec(), false);
        page::add("00000000.sav", b"other".to_vec(), true);
        assert_eq!(f.take_page_files(), 2);
        assert_eq!(f.files.keys().cloned().collect::<Vec<_>>(), ["00000000.sav", "00000001.sav"]);
        assert_eq!(page::take_write().map(|w| w.0), Some("00000001.sav".to_string()));
        assert!(page::take_write().is_none());
        assert_eq!(f.export("00000000.sav"), Some("00000000.sav".to_string()));
        assert_eq!(page::take_download().map(|d| d.1), Some(b"junk".to_vec()));
        let l = f.listing();
        assert!(l.iter().all(|x| x.summary.is_err()));
        assert!(l[0].line().contains("does not read"), "{}", l[0].line());
        // 2026-11-06 12:30:15.250 UTC was a Friday
        assert_eq!(civil_time(1_793_968_215_250), [2026, 11, 5, 6, 12, 30, 15, 250]);
    }
}
