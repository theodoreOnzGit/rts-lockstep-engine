// SPDX-License-Identifier: GPL-3.0-only
//
// Integration batch I57 (GitHub #3, #15), after (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/GameEngine.cpp (GameEngine::init: the file system, the game text, the
//   logic's subsystems with their INI, TheMapCache)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameText.cpp (GameTextManager::init: g_csfFile)
//   GeneralsMD/Code/Main/WinMain.cpp (g_csfFile = "data\\%s\\Generals.csf")
//   GeneralsMD/Code/GameEngine/Source/Common/GlobalData.cpp (the user data directory's leaf name)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! A [`World`] made from a game install, as `GameEngine::init` makes the engine: the file system over the install's
//! folder and archives (batch F1), the game text from `Data\<language>\Generals.csf`, the logic's INI in
//! `initSubsystem`'s order (batch I12), and `TheMapCache` (batch I56).
//!
//! **The World's disk is its `TheFileSystem`.** The World reads its files (maps, `map.ini`, replays, saves) from
//! `save_side.disk`. An install's files reach it there: `Maps\MapCache.ini` at start, and a map's folder when the map
//! is chosen ([`World::mount_map`]), as upstream's file system would find them in `MapsZH.big`. Only what is mounted is
//! held in memory, not the install's archives.
//!
//! **Not done here:** the client's subsystems (art, audio, the GUI's INI), and every INI the logic does not read.
//! (GitHub #61: `Campaign.ini` and `ChallengeMode.ini`, the client's upstream, are read, as the logic reads their
//! managers.)

use std::sync::Arc;

use crate::common::file_system::{FileSystem, LocalDevice};
use crate::common::ini::IniError;
use crate::common::map_cache::MapCacheError;
use crate::formats::big::FilenameList;
use crate::formats::csf::{CsfError, GameText};
use crate::logic::world::game_data::{load_game_data, GameDataFiles, MissingFiles, TextTable};
use crate::logic::world::game_logic::World;

/// `GetRegistryLanguage()`'s answer for an English install (the folder name under `Data\`).
pub const DEFAULT_LANGUAGE: &[u8] = b"English";

/// The user data directory under the user's documents (`GlobalData`'s default `UserDataLeafName`, with its `\`). The
/// World's disk stands for the documents folder.
pub const USER_DATA_DIR: &[u8] = b"Command and Conquer Generals Zero Hour Data\\";

/// What stopped an install's World from being made.
#[derive(Clone, Debug, PartialEq)]
pub enum InstallError {
    /// The logic's INI load threw (a missing file, an unknown block, a bad value), with the files the load opens that
    /// the install does not have (upstream throws at the first; all of them are listed for the report).
    Ini { error: IniError, missing: Vec<Vec<u8>> },
    /// `updateCache` threw (`Maps\MapCache.ini` missing or bad, or a user map that does not parse).
    MapCache(MapCacheError),
    /// [`World::start_skirmish`]: the map is not in the map cache.
    MapNotInCache(Vec<u8>),
}

/// What the install gave, for the loader's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstallReport {
    /// The archives that did not open (`FileSystem::archive_errors`).
    pub archive_errors: Vec<String>,
    /// The string file's labels, or why it did not load (`None`: not found).
    pub csf: Option<Result<usize, CsfError>>,
    /// The thing templates read.
    pub templates: usize,
    /// The maps in the map cache.
    pub maps: usize,
    /// Batch I60: the script files (`Data\Scripts\*.scb`) mounted.
    pub scripts: usize,
    /// GitHub #52: the audio events with a length (`World::statics.audio_lengths_ms`).
    pub audio_lengths: usize,
    /// GitHub #61: the campaigns of `Data\INI\Campaign.ini` (0 without the file).
    pub campaigns: usize,
    /// GitHub #61: whether `Data\INI\ChallengeMode.ini` was read.
    pub challenge_mode: bool,
}

/// `"%s\\%s"`-style join of the parts.
fn join(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

impl World {
    /// `GameEngine::init` over an install's file system: the game text, the logic's INI (every file must open, as
    /// `INI::load` throws on one that does not), a World with those statics, the user data directory, the shipped map
    /// cache file mounted, and `TheMapCache->updateCache()`.
    pub fn from_install<D: LocalDevice>(fs: &FileSystem<D>, language: &[u8]) -> Result<(World, InstallReport), InstallError> {
        Self::from_install_with_audio(fs, fs, language)
    }

    /// [`World::from_install`] with the audio files' headers read from `audio` (GitHub #52): the same install, where its
    /// audio archives are on another device than its INI (the browser's: the logic's files are handed over whole, the
    /// audio archives by range, `client/src/import.rs`). Every byte `audio` is asked for must be there now: a read that
    /// is not served gives a length of 0, so a caller whose bytes arrive later fetches them first.
    pub fn from_install_with_audio<D: LocalDevice>(fs: &FileSystem<D>, audio: &impl GameDataFiles, language: &[u8]) -> Result<(World, InstallReport), InstallError> {
        let mut report = InstallReport { archive_errors: fs.archive_errors.iter().map(|e| format!("{e:?}")).collect(), ..Default::default() };
        // TheGameText->init(): `g_csfFile` with the registry's language
        let csf_path = join(&[b"data\\", language, b"\\Generals.csf"]);
        let text = match fs.open_file(&csf_path) {
            Some(bytes) => match GameText::load_csf(&bytes) {
                Ok(t) => {
                    report.csf = Some(Ok(t.text_count()));
                    TextTable::from_csf(t)
                }
                Err(e) => {
                    report.csf = Some(Err(e));
                    TextTable::default()
                }
            },
            None => TextTable::default(),
        };
        // the logic's subsystems, each `init()` then its INI, then `postProcessLoadAll()`
        let load = load_game_data(fs, MissingFiles::Error, text).map_err(|error| {
            let missing = crate::logic::world::game_data::LOGIC_SUBSYSTEMS
                .iter()
                .flat_map(|sys| [sys.path1, sys.path2])
                .flatten()
                .filter(|p| !fs.does_file_exist(p))
                .map(<[u8]>::to_vec)
                .collect();
            InstallError::Ini { error, missing }
        })?;
        // (GitHub #52: `getAudioLengthMS` of every event, from its files' headers, for the script engine's waits)
        let mut load = load;
        load.load_audio_lengths(audio, language);
        report.audio_lengths = load.audio_lengths_ms.len();
        report.templates = load.factory.templates().count();
        let (mut statics, names) = load.into_statics();
        if statics.global_data.the_global_data().is_none() {
            statics.global_data.create_original();
        }
        if let Some(g) = statics.global_data.the_writable_global_data() {
            g.set_user_data_dir(USER_DATA_DIR);
        }
        let mut w = World::new();
        w.names = names;
        w.statics = Arc::new(statics);
        w.install_command_buttons();
        // a game: every integration switch on (the tables are the module tests' ablations)
        w.cuts.set_all_live();
        // (GitHub #64: `GameLogic::init`'s `TheScriptEngine->init()`, which ends in `reset()`, and `ThePlayerList->init()`
        // (the neutral player, local), as `GameEngine::init` makes them; they were left out, so the install's World had no
        // player and the script engine's counters at 0, where a World cleared by `clearGameData` has upstream's)
        w.init_engine_subsystems().map_err(|_| InstallError::Ini { error: IniError::new(crate::common::ini::IniErrorKind::Bug), missing: Vec::new() })?;
        // (GitHub #61: `GameClient::init`'s `TheChallengeGenerals->init()` and `TheCampaignManager->init()`, which the
        // logic reads; each file when the install has it)
        let (challenge, _) = w.load_campaigns(fs).map_err(|error| InstallError::Ini { error, missing: Vec::new() })?;
        report.challenge_mode = challenge;
        report.campaigns = w.campaign.campaign_list.len();
        // TheMapCache = NEW MapCache; TheMapCache->updateCache(): the shipped cache file from the archives
        let cache = join(&[b"Maps\\", crate::common::map_cache::MAP_CACHE_NAME]);
        if let Some(bytes) = fs.open_file(&cache) {
            w.save_side.disk.insert(cache.to_ascii_lowercase(), bytes);
        }
        w.update_map_cache().map_err(InstallError::MapCache)?;
        report.maps = w.statics.map_cache.maps.len();
        // (batch I60: the scripts a new game opens, `Data\Scripts\SkirmishScripts.scb` (`prepareForMP_or_Skirmish`) and
        // `MultiplayerScripts.scb`, which an install keeps in its archives; without them every skirmish computer player whose
        // side the map lacks was made human, as upstream does when the file is missing)
        let mut scripts = FilenameList::new();
        fs.get_file_list_in_directory(b"Data\\Scripts\\", b"*.scb", &mut scripts, false);
        for name in scripts.iter() {
            if let Some(bytes) = fs.open_file(name) {
                w.save_side.disk.insert(name.to_ascii_lowercase(), bytes);
                report.scripts += 1;
            }
        }
        Ok((w, report))
    }

    /// The skirmish menu's launch (SkirmishGameOptionsMenu.cpp): the options entered (`enterGame`), slot 0 the local
    /// human with player template `template` at the map's first start, `startGame(0)`; then the recorder, the map's
    /// name, `InitGameLogicRandom(seed)` and `MSG_NEW_GAME` (`GAME_SKIRMISH`). The game starts on the next frame. The map
    /// must be in the map cache (the menu lists only those).
    pub fn start_skirmish(&mut self, map: &[u8], template: i32, seed: i32) -> Result<(), InstallError> {
        self.start_skirmish_against(map, template, seed, None)
    }

    /// [`World::start_skirmish`] with a computer opponent in slot 1: `(slot state, player template)`, the state one of
    /// `SLOT_EASY_AI`, `SLOT_MED_AI`, `SLOT_BRUTAL_AI`, at the map's second start, with the second colour.
    pub fn start_skirmish_against(&mut self, map: &[u8], template: i32, seed: i32, opponent: Option<(i32, i32)>) -> Result<(), InstallError> {
        use crate::common::game_info::slot_state;
        let mut slots = vec![SkirmishSlot { state: slot_state::PLAYER, template, color: 0, start_pos: 0, team: -1 }];
        if let Some((state, t)) = opponent {
            slots.push(SkirmishSlot { state, template: t, color: 1, start_pos: 1, team: -1 });
        }
        self.start_skirmish_slots(map, seed, &slots)
    }

    /// The skirmish menu's launch with the slots given, from slot 0 (GitHub #58; [`World::start_skirmish_against`] is its
    /// two-slot case): `enterGame`, each slot's state, player template, colour, start and team as the menu's controls set
    /// them (a human slot is named "Me"; the menu has one), `startGame(0)`; then the recorder, the map's name,
    /// `InitGameLogicRandom(seed)` and `MSG_NEW_GAME` (`GAME_SKIRMISH`). The game starts on the next frame. The map must be
    /// in the map cache (the menu lists only those).
    pub fn start_skirmish_slots(&mut self, map: &[u8], seed: i32, slots: &[SkirmishSlot]) -> Result<(), InstallError> {
        use crate::common::game_info::{slot_state, GameInfo};
        use crate::common::recorder::game_mode;
        use crate::logic::world::game_logic_new_game::GameSource;
        if self.statics.map_cache.find_map(map).is_none() {
            return Err(InstallError::MapNotInCache(map.to_vec()));
        }
        // (GitHub #64: `reallyDoStart`'s `if (TheGameLogic->isInGame()) TheGameLogic->clearGameData(FALSE)`)
        if crate::common::recorder::RecorderHost::is_in_game(self) {
            self.clear_game_data(false);
        }
        self.cuts.game_loop.game_mode = game_mode::SHELL;
        let mut g = GameInfo::new(self);
        g.enter_game(self);
        let name: Vec<u16> = "Me".encode_utf16().collect();
        for (i, s) in slots.iter().enumerate().take(g.slots.len()) {
            let n: &[u16] = if s.state == slot_state::PLAYER { &name } else { &[] };
            g.slots[i].set_state(s.state, n, 0, self);
            g.slots[i].set_player_template(s.template);
            g.slots[i].color = s.color;
            g.slots[i].start_pos = s.start_pos;
            g.slots[i].team_number = s.team;
        }
        g.map_name = map.to_vec();
        g.seed = seed;
        g.start_game(0, self);
        self.cuts.new_game.games.push((GameSource::Skirmish, g));
        self.recorder_init();
        self.cuts.new_game.map_name = map.to_vec();
        self.random.init_game_logic_random(seed as u32);
        self.push_command(crate::common::message_types::MSG_NEW_GAME, 0, |m| {
            m.append_integer_argument(game_mode::SKIRMISH);
            m.append_integer_argument(1);
            m.append_integer_argument(0);
        });
        Ok(())
    }

    /// The files of a map's folder (`Maps\<name>\`: the map, its `map.ini`, `map.str`, `solo.ini`) put on the World's
    /// disk, as upstream's file system finds them when the map is loaded. The number of files mounted.
    pub fn mount_map<D: LocalDevice>(&mut self, fs: &FileSystem<D>, map_path: &[u8]) -> usize {
        let Some(slash) = map_path.iter().rposition(|&c| c == b'\\') else { return 0 };
        let dir = &map_path[..=slash];
        let mut list = FilenameList::new();
        fs.get_file_list_in_directory(dir, b"*", &mut list, false);
        let mut n = 0;
        for name in list.iter() {
            if let Some(bytes) = fs.open_file(name) {
                self.save_side.disk.insert(name.to_ascii_lowercase(), bytes);
                n += 1;
            }
        }
        n
    }

    /// GitHub #62 (new; upstream's file system holds every byte): [`World::mount_map`] from a file system whose bytes may
    /// not have arrived yet (the browser's ranged device, `RangedDevice`, whose archive members the page reads by range).
    /// The folder's files are read; if any read had to wait (`LocalDevice::unserved_reads` moved: the device has asked for
    /// the bytes), nothing is mounted and `None` is returned, to be called again once they have arrived. Otherwise every
    /// file is mounted whole, as `mount_map` mounts it: a map is never started from part of its folder, so every machine
    /// that mounts it gets the same bytes. The number of files mounted.
    pub fn mount_map_when_ready<D: LocalDevice>(&mut self, fs: &FileSystem<D>, map_path: &[u8]) -> Option<usize> {
        let Some(slash) = map_path.iter().rposition(|&c| c == b'\\') else { return Some(0) };
        let dir = &map_path[..=slash];
        let mut list = FilenameList::new();
        fs.get_file_list_in_directory(dir, b"*", &mut list, false);
        let before = fs.local.device.unserved_reads();
        let files: Vec<(Vec<u8>, Vec<u8>)> = list.iter().filter_map(|name| fs.open_file(name).map(|b| (name.to_ascii_lowercase(), b))).collect();
        if fs.local.device.unserved_reads() != before {
            return None;
        }
        let n = files.len();
        self.save_side.disk.extend(files);
        Some(n)
    }

    /// GitHub #64 (new, as [`World::mount_map_when_ready`]): a save's map folder put on the World's disk before the save is
    /// loaded into it, as a mission's start puts its folder there: the save's pristine map (`GameState`'s
    /// `portableMapPathToRealMapPath`) and the files beside it, read from `fs` once all its bytes have arrived. A save of a
    /// campaign mission started from the list then reads its folder's `map.ini`, `solo.ini` and `map.str` on load, as the
    /// mission's start read them (`loadMapINI` looks beside the pristine map). `Ok(None)` while waiting; the number of files
    /// mounted (0 for a map the files do not hold: the load then uses the map the save embeds). An error when the bytes
    /// are not a save.
    pub fn mount_save_map_when_ready<D: LocalDevice>(&mut self, fs: &FileSystem<D>, save: &[u8]) -> Result<Option<usize>, String> {
        use crate::common::game_state::GameState;
        use crate::logic::world::game_state_world::WorldBlock;
        let summary = crate::client::save_game::read_save_summary(save.to_vec()).map_err(|e| format!("the save: {e:?}"))?;
        let real = GameState::<WorldBlock>::new().portable_map_path_to_real(&summary.pristine_map, self).map_err(|e| format!("the save's map: {e:?}"))?;
        Ok(self.mount_map_when_ready(fs, &real))
    }
}

/// A slot of [`World::start_skirmish_slots`]: its state (`SLOT_PLAYER`, `SLOT_EASY_AI`, ...), player template index, colour
/// index, start position index and team number (-1: no team).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkirmishSlot {
    pub state: i32,
    pub template: i32,
    pub color: i32,
    pub start_pos: i32,
    pub team: i32,
}

/// The game [`World::quick_skirmish`] started: the map, the local player's side and the opponent's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickSkirmish {
    /// The map's path in the map cache (`maps\<name>\<name>.map`, lower case).
    pub map: Vec<u8>,
    /// The map's display name.
    pub map_name: String,
    /// The player templates' sides (`FactionAmerica`, ...), the local player's then the computer's.
    pub sides: (Vec<u8>, Vec<u8>),
    /// The files of the map's folder mounted on the World's disk.
    pub mounted: usize,
}

impl World {
    /// The skirmish menu's map list: the map cache's multiplayer maps for two or more players, by path.
    pub fn skirmish_maps(&self) -> Vec<Vec<u8>> {
        self.statics.map_cache.maps.iter().filter(|(_, m)| m.is_multiplayer && m.num_players >= 2).map(|(k, _)| k.clone()).collect()
    }

    /// A skirmish on `map` (else the first of [`World::skirmish_maps`]) as the skirmish menu would start it with its
    /// defaults: the local player with the first playable side (`PlayableSide`, not an observer) in slot 0, a medium
    /// computer player with the next playable side (the same one when there is only one) in slot 1, the map's folder
    /// mounted, and [`World::start_skirmish_against`]. The game starts on the next frame.
    pub fn quick_skirmish<D: LocalDevice>(&mut self, fs: &FileSystem<D>, map: Option<&[u8]>, seed: i32) -> Result<QuickSkirmish, InstallError> {
        let chosen = self.choose_skirmish_map(map)?;
        let mounted = self.mount_map(fs, &chosen);
        let mut q = self.quick_skirmish_mounted(&chosen, seed)?;
        q.mounted = mounted;
        Ok(q)
    }

    /// [`World::quick_skirmish`]'s map: `map` (lower case), else the first of [`World::skirmish_maps`].
    pub fn choose_skirmish_map(&self, map: Option<&[u8]>) -> Result<Vec<u8>, InstallError> {
        match map {
            Some(m) => Ok(m.to_ascii_lowercase()),
            None => self.skirmish_maps().first().cloned().ok_or_else(|| InstallError::MapNotInCache(Vec::new())),
        }
    }

    /// [`World::quick_skirmish`] on a map whose folder is already on the World's disk (GitHub #4: a World cloned from the
    /// one an install made, for a save's load; `mounted` is 0).
    pub fn quick_skirmish_mounted(&mut self, chosen: &[u8], seed: i32) -> Result<QuickSkirmish, InstallError> {
        let chosen = chosen.to_vec();
        let playable: Vec<(i32, Vec<u8>)> = self
            .statics
            .player_template_store
            .templates()
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_playable_side() && !t.is_observer())
            .map(|(i, t)| (i as i32, t.side().to_vec()))
            .collect();
        let me = playable.first().cloned().unwrap_or((0, Vec::new()));
        let them = playable.get(1).cloned().unwrap_or_else(|| me.clone());
        let map_name = self.statics.map_cache.find_map(&chosen).map(|m| String::from_utf16_lossy(m.display_name.str())).unwrap_or_default();
        self.start_skirmish_against(&chosen, me.0, seed, Some((crate::common::game_info::slot_state::MED_AI, them.0)))?;
        Ok(QuickSkirmish { map: chosen, map_name, sides: (me.1, them.1), mounted: 0 })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::common::file_system::MemoryDevice;
    use crate::formats::big::build_big;
    use crate::logic::world::game_data::LOGIC_SUBSYSTEMS;

    pub(crate) const MAP: &[u8] = b"Maps\\Duel\\Duel.map";

    /// A string file of `(label, text)` pairs (the format of `formats::csf`'s module docs).
    fn csf(labels: &[(&str, &str)]) -> Vec<u8> {
        let tag = |s: &[u8; 4]| i32::from_be_bytes(*s);
        let mut v = Vec::new();
        let w = |v: &mut Vec<u8>, x: i32| v.extend_from_slice(&x.to_le_bytes());
        for x in [tag(b"CSF "), 3, labels.len() as i32, labels.len() as i32, 0, 0] {
            w(&mut v, x);
        }
        for (l, t) in labels {
            w(&mut v, tag(b"LBL "));
            w(&mut v, 1);
            w(&mut v, l.len() as i32);
            v.extend_from_slice(l.as_bytes());
            w(&mut v, tag(b"STR "));
            let units: Vec<u16> = t.encode_utf16().collect();
            w(&mut v, units.len() as i32);
            for u in units {
                v.extend_from_slice(&(!u).to_le_bytes());
            }
        }
        v
    }

    /// A map with a flat 30 x 30 height map, the neutral side, and two start waypoints.
    pub(crate) fn duel_map(w: &mut World) -> Vec<u8> {
        map_with_starts(w, &[(b"Player_1_Start", 100.0, 100.0), (b"Player_2_Start", 200.0, 200.0)])
    }

    /// GitHub #58: [`duel_map`] with four start waypoints, one near each corner.
    pub(crate) fn four_map(w: &mut World) -> Vec<u8> {
        map_with_starts(w, &[(b"Player_1_Start", 60.0, 60.0), (b"Player_2_Start", 240.0, 240.0), (b"Player_3_Start", 60.0, 240.0), (b"Player_4_Start", 240.0, 60.0)])
    }

    /// [`duel_map`]'s map with the start waypoints given: (name, x, y).
    fn map_with_starts(w: &mut World, starts: &[(&[u8], f32, f32)]) -> Vec<u8> {
        use crate::common::dict::Dict;
        use crate::formats::data_chunk_output::DataChunkOutput;
        use crate::logic::map_object::K_OBJECTS_VERSION_3;
        let mut out = DataChunkOutput::new();
        out.open_data_chunk(b"HeightMapData", crate::logic::terrain_visual::K_HEIGHT_MAP_VERSION_4);
        for v in [30, 30, 0, 1, 30, 30, 900] {
            out.write_int(v);
        }
        out.write_array_of_bytes(&[4u8; 900]);
        out.close_data_chunk();
        let templates = crate::logic::script_templates::script_templates(&mut w.names);
        let mut sc = crate::logic::sides_list::SidesCtx::new(w.names.clone(), templates);
        let mut d = Dict::new();
        d.set_ascii_string(sc.names.generator.name_to_key(b"playerName"), b"").unwrap();
        d.set_bool(sc.names.generator.name_to_key(b"playerIsHuman"), false).unwrap();
        d.set_ascii_string(sc.names.generator.name_to_key(b"playerFaction"), b"").unwrap();
        sc.sides.add_side(&d);
        // (batch I60: the sides of a multiplayer map: the neutral one, the civilians (`FactionCivilian`, which
        // `prepareForMP_or_Skirmish` keeps), and a skirmish side for America with its team, which it moves to the skirmish
        // sides: a computer slot of `FactionAmerica` is a skirmish AI only when one exists, `Player::initFromDict`'s check)
        for (name, faction) in [(&b"PlyrCivilian"[..], &b"FactionCivilian"[..]), (b"SkirmishAmerica", b"FactionAmerica")] {
            let mut d = Dict::new();
            d.set_ascii_string(sc.names.generator.name_to_key(b"playerName"), name).unwrap();
            d.set_bool(sc.names.generator.name_to_key(b"playerIsHuman"), false).unwrap();
            d.set_ascii_string(sc.names.generator.name_to_key(b"playerFaction"), faction).unwrap();
            sc.sides.add_side(&d);
            let mut t = Dict::new();
            let mut team_name = b"team".to_vec();
            team_name.extend_from_slice(name);
            t.set_ascii_string(sc.names.generator.name_to_key(b"teamName"), &team_name).unwrap();
            t.set_ascii_string(sc.names.generator.name_to_key(b"teamOwner"), name).unwrap();
            t.set_bool(sc.names.generator.name_to_key(b"teamIsSingleton"), true).unwrap();
            sc.sides.add_team(&t);
        }
        // (and its script list, empty, as every side of a real map has one: `initFromDict` dereferences the skirmish side's list)
        for i in 0..sc.sides.get_num_sides() {
            sc.sides.get_side_info_mut(i).expect("the side").scripts = Some(crate::logic::scripts::ScriptList { groups: Vec::new(), scripts: Vec::new() });
        }
        crate::logic::sides_list::SidesList::write_sides_data_chunk(&mut out, &mut sc).expect("the sides written");
        // (the objects after the sides, in WorldBuilder's order: a map's last chunk, if empty, is never parsed, as upstream's
        // `DataChunkInput::parse` stops at the end of the file after opening it; the sides list's last script list is empty)
        out.open_data_chunk(b"ObjectsList", K_OBJECTS_VERSION_3);
        for (i, &(name, x, y)) in starts.iter().enumerate() {
            out.open_data_chunk(b"Object", K_OBJECTS_VERSION_3);
            for r in [x, y, 0.0, 0.0] {
                out.write_real(r);
            }
            out.write_int(0);
            out.write_ascii_string(b"*Waypoints/Waypoint");
            let mut props = Dict::new();
            props.set_int(w.names.name_to_key(b"waypointID"), i as i32 + 1).unwrap();
            props.set_ascii_string(w.names.name_to_key(b"waypointName"), name).unwrap();
            out.write_dict(&props, &w.names);
            out.close_data_chunk();
        }
        out.close_data_chunk();
        out.finish()
    }

    /// An install in memory, laid out as Zero Hour's: `INIZH.big` (every file the logic's load opens: the tests'
    /// templates in `Object\`, `Multiplayer.ini` with one colour, the others empty), `EnglishZH.big` (the string file),
    /// and `MapsZH.big` (the shipped map cache, listing the map with its name tag, and the map's folder).
    fn install(map: &[u8]) -> FileSystem<MemoryDevice> {
        install_with(map, None)
    }

    /// [`install`] with a `Data\\Scripts\\SkirmishScripts.scb` in `INIZH.big`.
    fn install_with(map: &[u8], skirmish_scripts: Option<&[u8]>) -> FileSystem<MemoryDevice> {
        let mut d = MemoryDevice::new();
        let mut archives = install_archives(map);
        if let Some(scb) = skirmish_scripts {
            archives.push((b"ScriptsZH.big".to_vec(), build_big(&[(b"Data\\Scripts\\SkirmishScripts.scb", scb)])));
        }
        for (name, bytes) in archives {
            d.insert(&[&b"ZH\\"[..], &name].concat(), bytes);
        }
        let mut fs = FileSystem::new(d);
        fs.local.game_dir = b"ZH\\".to_vec();
        fs.init(None);
        fs
    }

    /// GitHub #58: an install for four players: [`install_with`]'s archives with two more playable sides (`FactionChina`,
    /// `FactionGLA`, each with the starting Barracks and Tank), four colours, and the map cache's entry for a four-player
    /// map (`map` should be [`four_map`]'s).
    pub(crate) fn install_four(map: &[u8], skirmish_scripts: Option<&[u8]>) -> FileSystem<MemoryDevice> {
        let mut d = MemoryDevice::new();
        let mut archives = install_archives_for(map, true);
        if let Some(scb) = skirmish_scripts {
            archives.push((b"ScriptsZH.big".to_vec(), build_big(&[(b"Data\\Scripts\\SkirmishScripts.scb", scb)])));
        }
        for (name, bytes) in archives {
            d.insert(&[&b"ZH\\"[..], &name].concat(), bytes);
        }
        let mut fs = FileSystem::new(d);
        fs.local.game_dir = b"ZH\\".to_vec();
        fs.init(None);
        fs
    }

    /// The three archives of [`install`], by file name.
    fn install_archives(map: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        install_archives_for(map, false)
    }

    /// [`install_archives`], or [`install_four`]'s (`four`).
    pub(crate) fn install_archives_for(map: &[u8], four: bool) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut ini: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for sys in LOGIC_SUBSYSTEMS {
            for p in [sys.path1, sys.path2].into_iter().flatten() {
                ini.push((p.to_vec(), Vec::new()));
            }
        }
        let set = |ini: &mut Vec<(Vec<u8>, Vec<u8>)>, path: &[u8], text: &[u8]| {
            ini.retain(|(p, _)| p != path);
            ini.push((path.to_vec(), text.to_vec()));
        };
        let mut units = crate::logic::world::player_host_world::tests::INI.to_string();
        let mut colors = "MultiplayerColor Gold\n TooltipName = Color:Gold\n RGBColor = R:255 G:200 B:0\n RGBNightColor = R:128 G:100 B:0\nEnd\n\
              MultiplayerColor Red\n TooltipName = Color:Red\n RGBColor = R:255 G:0 B:0\n RGBNightColor = R:128 G:0 B:0\nEnd\n"
            .to_string();
        if four {
            for (faction, side) in [("FactionChina", "China"), ("FactionGLA", "GLA")] {
                units.push_str(&format!("PlayerTemplate {faction}\n Side = {side}\n PlayableSide = Yes\n StartingBuilding = Barracks\n StartingUnit0 = Tank\nEnd\n"));
            }
            colors.push_str(
                "MultiplayerColor Blue\n TooltipName = Color:Blue\n RGBColor = R:0 G:0 B:255\n RGBNightColor = R:0 G:0 B:128\nEnd\n\
                 MultiplayerColor Green\n TooltipName = Color:Green\n RGBColor = R:0 G:255 B:0\n RGBNightColor = R:0 G:128 B:0\nEnd\n",
            );
        }
        set(&mut ini, b"Data\\INI\\Object\\Units.ini", units.as_bytes());
        set(&mut ini, b"Data\\INI\\Multiplayer.ini", colors.as_bytes());
        let ini_refs: Vec<(&[u8], &[u8])> = ini.iter().map(|(p, t)| (p.as_slice(), t.as_slice())).collect();
        let text = csf(&[("MAP:Duel", "Desert Duel"), ("GUI:MediumAI", "Medium Army")]);
        let cache: &[u8] = if four {
            b"MapCache maps_5Cduel_5Cduel_2Emap\n fileSize = 1\n fileCRC = 7\n isOfficial = yes\n isMultiplayer = yes\n\
            numPlayers = 4\n extentMin = X:0.00 Y:0.00 Z:0.00\n extentMax = X:300.00 Y:300.00 Z:0.00\n nameLookupTag = MAP_3ADuel\n\
            Player_1_Start = X:60.00 Y:60.00 Z:0.00\n Player_2_Start = X:240.00 Y:240.00 Z:0.00\n\
            Player_3_Start = X:60.00 Y:240.00 Z:0.00\n Player_4_Start = X:240.00 Y:60.00 Z:0.00\nEND\n"
        } else {
            b"MapCache maps_5Cduel_5Cduel_2Emap\n fileSize = 1\n fileCRC = 7\n isOfficial = yes\n isMultiplayer = yes\n\
            numPlayers = 2\n extentMin = X:0.00 Y:0.00 Z:0.00\n extentMax = X:300.00 Y:300.00 Z:0.00\n nameLookupTag = MAP_3ADuel\n\
            Player_1_Start = X:100.00 Y:100.00 Z:0.00\n Player_2_Start = X:200.00 Y:200.00 Z:0.00\nEND\n"
        };
        vec![
            (b"INIZH.big".to_vec(), build_big(&ini_refs)),
            (b"EnglishZH.big".to_vec(), build_big(&[(b"Data\\English\\Generals.csf", &text)])),
            (b"MapsZH.big".to_vec(), build_big(&[(b"Maps\\MapCache.ini", cache), (MAP, map), (b"Maps\\Duel\\map.ini", b"")])),
        ]
    }

    /// Batch I57: the same install written to a folder on disk, with the archives' names in other cases (a Steam folder
    /// on Linux keeps Windows' mixed case), read through [`crate::common::file_system::NativeDevice`] as the loader
    /// (`examples/load_install.rs`) reads one. Predicted: the same report, and the map mounts and starts.
    #[test]
    fn an_install_on_disk_loads_through_the_native_device() {
        use crate::common::file_system::NativeDevice;
        let mut scratch = World::new();
        let map = duel_map(&mut scratch);
        let dir = std::env::temp_dir().join(format!("rts_install_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the folder");
        for (name, bytes) in install_archives(&map) {
            std::fs::write(dir.join(String::from_utf8_lossy(&name).to_uppercase()), bytes).expect("an archive");
        }
        let mut fs = FileSystem::new(NativeDevice::new(&dir));
        fs.init(None);
        let (mut w, report) = World::from_install(&fs, DEFAULT_LANGUAGE).expect("the install");
        assert!(report.archive_errors.is_empty(), "{:?}", report.archive_errors);
        assert_eq!((report.csf, report.templates, report.maps), (Some(Ok(2)), 11, 1));
        assert_eq!(w.mount_map(&fs, MAP), 2);
        w.start_skirmish(MAP, 0, 5).expect("the skirmish");
        for _ in 0..10 {
            w.engine_update().expect("a frame");
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        assert!(w.objects.ids_in_order().len() >= 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Integration batch I57: a World made from an install, as `GameEngine::init` makes the engine, starts a skirmish on a
    /// map of the install's archives. Predicted before the run:
    /// - the install loads with no error: the string file's one label, the tests' ~~twelve~~ eleven templates (a miscount in the prediction: the tests' INI holds eleven objects), one map in the cache
    ///   (batch I62: the string file has a second label, `GUI:MediumAI`, so two)
    ///   (from the shipped `MapCache.ini`, not parsed: its CRC is the file's 7, and its display name is the string file's
    ///   "Desert Duel (2)");
    /// - the user data directory is `Command and Conquer Generals Zero Hour Data\`, so the user's map directory is not the
    ///   shipped one and no cache file is written;
    /// - mounting the map puts its folder's two files on the disk;
    /// - a skirmish of one human `FactionAmerica` slot at start 0 starts on the next frame: the player's starting
    ///   Barracks and Tank are made at `Player_1_Start`, and 100 frames run with no live error;
    /// - with only `GameData.ini` in the archive, the load stops with `CantOpenFile`, and the report lists the other files
    ///   (`Multiplayer.ini` among them) and not `GameData.ini`.
    #[test]
    fn a_world_from_an_install_starts_a_skirmish_on_its_map() {
        // the map is written with a World's name keys (the dictionary's keys are names in the file)
        let mut scratch = World::new();
        let map = duel_map(&mut scratch);
        let fs = install(&map);
        assert!(fs.archive_errors.is_empty());
        let (mut w, report) = World::from_install(&fs, DEFAULT_LANGUAGE).expect("the install");
        assert_eq!(report.csf, Some(Ok(2)));
        assert_eq!(report.templates, 11);
        assert_eq!(report.maps, 1);
        let md = w.statics.map_cache.find_map(MAP).expect("the shipped entry").clone();
        assert_eq!((md.num_players, md.crc), (2, 7));
        assert_eq!(String::from_utf16_lossy(md.display_name.str()), "Desert Duel (2)");
        assert!(!w.save_side.disk.keys().any(|k| k.starts_with(b"command and conquer") && k.ends_with(b".ini")), "no cache file written");
        assert_eq!(w.mount_map(&fs, MAP), 2);

        assert!(w.cuts.all_live());
        w.start_skirmish(MAP, 0, 99).expect("the skirmish");
        let nowhere = b"Maps\\Nowhere\\Nowhere.map";
        assert_eq!(w.start_skirmish(nowhere, 0, 99), Err(InstallError::MapNotInCache(nowhere.to_vec())));
        for _ in 0..100 {
            w.engine_update().expect("a frame");
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        let made: Vec<(Vec<u8>, crate::common::base_type::Coord3D)> = w
            .objects
            .ids_in_order()
            .into_iter()
            .filter_map(|o| w.find_object(o).map(|x| (x.template().get_name().to_vec(), x.get_position())))
            .collect();
        let names: Vec<&[u8]> = made.iter().map(|(n, _)| n.as_slice()).collect();
        assert!(names.contains(&&b"Barracks"[..]) && names.contains(&&b"Tank"[..]), "{names:?}");
        let barracks = made.iter().find(|(n, _)| n == b"Barracks").map(|(_, p)| *p).expect("the barracks");
        assert!((barracks.x - 100.0).abs() < 1.0 && (barracks.y - 100.0).abs() < 1.0, "{barracks:?}");

        // the same install without `Multiplayer.ini`: `INI::load` throws on the file it cannot open
        let mut d = MemoryDevice::new();
        let empty: [(&[u8], &[u8]); 1] = [(b"Data\\INI\\GameData.ini", b"")];
        d.insert(b"ZH\\INIZH.big", build_big(&empty));
        let mut bare = FileSystem::new(d);
        bare.local.game_dir = b"ZH\\".to_vec();
        bare.init(None);
        let r = World::from_install(&bare, DEFAULT_LANGUAGE);
        let Err(InstallError::Ini { error, missing }) = r else { panic!("{r:?}") };
        assert_eq!(error.kind, crate::common::ini::IniErrorKind::CantOpenFile);
        assert!(missing.iter().any(|m| m == b"Data\\INI\\Multiplayer.ini") && !missing.iter().any(|m| m == b"Data\\INI\\GameData.ini"), "{missing:?}");
    }

    /// A skirmish scripts file (`SkirmishScripts.scb`, the shipped one's form): one script list, the player it belongs to
    /// (`ScriptsPlayers`: `SkirmishAmerica`) and its team (`ScriptTeams`: `teamSkirmishAmerica`, a singleton), the teams last (a
    /// file's last chunk, if empty, is not parsed).
    pub(crate) fn skirmish_scripts(names: &mut crate::common::name_key_generator::NameKeyGenerator) -> Vec<u8> {
        use crate::common::dict::Dict;
        use crate::formats::data_chunk_output::DataChunkOutput;
        use crate::logic::scripts::ScriptList;
        let templates = crate::logic::script_templates::script_templates(names);
        let mut out = DataChunkOutput::new();
        let mut list = ScriptList::default();
        ScriptList::write_scripts_data_chunk(&mut out, &mut [Some(&mut list)], &templates, names).expect("the scripts written");
        out.open_data_chunk(b"ScriptsPlayers", 2);
        out.write_int(1);
        out.write_int(1);
        out.write_ascii_string(b"SkirmishAmerica");
        let mut d = Dict::new();
        d.set_ascii_string(names.name_to_key(b"playerName"), b"SkirmishAmerica").unwrap();
        d.set_ascii_string(names.name_to_key(b"playerFaction"), b"FactionAmerica").unwrap();
        out.write_dict(&d, names);
        out.close_data_chunk();
        out.open_data_chunk(b"ScriptTeams", 1);
        let mut t = Dict::new();
        t.set_ascii_string(names.name_to_key(b"teamName"), b"teamSkirmishAmerica").unwrap();
        t.set_ascii_string(names.name_to_key(b"teamOwner"), b"SkirmishAmerica").unwrap();
        t.set_bool(names.name_to_key(b"teamIsSingleton"), true).unwrap();
        out.write_dict(&t, names);
        out.close_data_chunk();
        out.finish()
    }

    /// Integration batch I60: a skirmish against a computer player (slot 1, `SLOT_MED_AI`, at `Player_2_Start`) on the install's
    /// map, run for 600 frames (20 seconds of game). Predicted before the run: both players are made from the slots, the
    /// computer one with a skirmish AI (`AISkirmishPlayer`, `isSkirmishAIPlayer`); each gets its starting Barracks and Tank at
    /// its start; the 600 frames run with no live error.
    ///
    /// The first run failed: the computer slot became a human player. Not a defect of the port: `Player::initFromDict` makes a
    /// computer slot a skirmish AI only when the sides list has a skirmish side of its faction's side ("quietly making into
    /// nonskirmish" otherwise), and the test map had only the neutral side. The map now carries `SkirmishAmerica`, as a
    /// multiplayer map does. It did expose a gap: an install's `Data\Scripts\*.scb` were never mounted (now they are).
    ///
    /// The second run failed in `initFromDict`'s skirmish branch: the skirmish side had no script list, which upstream
    /// dereferences. Two causes:
    /// - **a port defect:** the World's player host answered the sides' script lists from a table and `duplicateAndQualify`
    ///   with a bare handle, so in a game no player got its qualified scripts (now the sides' own lists and a real copy);
    /// - the fixture's map ended with the sides chunk, whose last script list, empty, upstream's chunk reader never parses (it
    ///   stops at the end of the file after opening the chunk); a real map has chunks after the sides, and the fixture now does.
    ///
    /// The third run made the skirmish AI with no team, so its units went to the neutral player. Upstream again:
    /// `prepareForMP_or_Skirmish` clears the map's skirmish teams when no side has scripts and reads scripts and teams from
    /// `SkirmishScripts.scb`, which a real install has. The fixture now lays out the map's sides as a multiplayer map does
    /// (neutral, `PlyrCivilian`, `SkirmishAmerica`) and has a `SkirmishScripts.scb`, which reaches the World through the new
    /// mount. Then all held: `SkirmishAmerica1` is the skirmish AI with its own team, both players' units are at their starts,
    /// and the 600 frames run with no live error.
    #[test]
    fn a_skirmish_against_a_computer_player_runs() {
        let mut scratch = World::new();
        let map = duel_map(&mut scratch);
        let scb = skirmish_scripts(&mut scratch.names);
        let fs = install_with(&map, Some(&scb));
        let (mut w, report) = World::from_install(&fs, DEFAULT_LANGUAGE).expect("the install");
        assert_eq!(report.scripts, 1, "the skirmish scripts mounted");
        assert_eq!(w.mount_map(&fs, MAP), 2);
        w.start_skirmish_against(MAP, 0, 7, Some((crate::common::game_info::slot_state::MED_AI, 0))).expect("the skirmish");
        for _ in 0..600 {
            w.engine_update().expect("a frame");
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        let players = w.players.player_count();
        let skirmish_ai: Vec<i32> =
            (0..players).filter(|&p| crate::logic::world::player_ops::player_is_skirmish_ai_player(&w, p).unwrap_or(false)).collect();
        let made: Vec<(Vec<u8>, i32, crate::common::base_type::Coord3D)> = w
            .objects
            .ids_in_order()
            .into_iter()
            .filter_map(|o| w.find_object(o).map(|x| (x.template().get_name().to_vec(), w.get_controlling_player(o).unwrap_or(-1), x.get_position())))
            .collect();
        eprintln!("players {players}; skirmish AI {skirmish_ai:?}; objects {:?}", made.iter().map(|(n, p, c)| (String::from_utf8_lossy(n).into_owned(), *p, c.x, c.y)).collect::<Vec<_>>());
        for p in 0..players {
            let pl = w.players.get_nth(p).expect("a player");
            eprintln!("player {p}: {:?} default team {:?}", String::from_utf8_lossy(&pl.player_name), pl.default_team);
        }
        assert_eq!(skirmish_ai.len(), 1, "one computer player");
        let ai = skirmish_ai[0];
        let near = |c: &crate::common::base_type::Coord3D, x: f32, y: f32| (c.x - x).abs() < 30.0 && (c.y - y).abs() < 30.0;
        assert!(made.iter().any(|(n, p, c)| n == b"Barracks" && *p == ai && near(c, 200.0, 200.0)), "the computer's barracks at its start");
        assert!(made.iter().any(|(n, p, c)| n == b"Barracks" && *p != ai && near(c, 100.0, 100.0)), "the human's barracks at theirs");
        assert_eq!(made.iter().filter(|(n, _, _)| n == b"Tank").count(), 2);

        // batch I62: three answers that were fixed literals. The computer slot's name is its label's text (`setState`'s
        // `TheGameText->fetch("GUI:MediumAI")`; it was empty); the save side's user data path is the install's (it was empty);
        // `getTeam() != NULL` is the object's (it was always true).
        let game = w.cuts.new_game.games.iter().find(|(s, _)| *s == crate::logic::world::game_logic_new_game::GameSource::Skirmish).map(|(_, g)| g.clone());
        let slot_name = game.map(|g| String::from_utf16_lossy(&g.slots[1].name)).unwrap_or_default();
        assert_eq!(slot_name, "Medium Army");
        assert_eq!(crate::common::game_state::GameStateHost::user_data_path(&w), USER_DATA_DIR.to_vec());
        let tank = w.objects.ids_in_order().into_iter().find(|o| w.find_object(*o).is_some_and(|x| x.template().get_name() == b"Tank")).expect("a tank");
        let t = World::thing_factory_find_template(&w, b"Tank").expect("the template");
        // (first written predicting a tank made with no team would have none: wrong, as upstream's `newObject` with a NULL team
        // puts it on the neutral player's default team (batch I47); so the team is cleared by hand to exercise the answer)
        let loner = w.thing_factory_new_object(t, None, Default::default()).expect("a tank");
        w.ai_self = loner;
        assert!(crate::logic::ai::ai_states::AiStateHost::has_team(&mut w), "the neutral player's team");
        w.objects.find_mut(loner).expect("the tank").team = 0;
        assert!(!crate::logic::ai::ai_states::AiStateHost::has_team(&mut w));
        w.ai_self = tank;
        assert!(crate::logic::ai::ai_states::AiStateHost::has_team(&mut w));
    }

    /// Batch C3 (GitHub #3): the browser's quick start over an install. Predicted: the install's map cache lists the duel map
    /// as the one skirmish map; `quick_skirmish` with no map chooses it, mounts its two files, takes the tests' playable
    /// side for both slots (the fixture has one player template), and after 300 frames both players have their starting
    /// Barracks and Tank, with no live error; an unknown map is refused.
    ///
    /// Results (2026-11-05): all held. Found by the browser's check of the page: the local player exists from the second
    /// frame only (the first dispatches `MSG_NEW_GAME`), which the client had read after one; now pinned here.
    #[test]
    fn a_quick_skirmish_starts_on_the_installs_first_map() {
        let mut scratch = World::new();
        let map = duel_map(&mut scratch);
        let scb = skirmish_scripts(&mut scratch.names);
        let fs = install_with(&map, Some(&scb));
        let (mut w, _) = World::from_install(&fs, DEFAULT_LANGUAGE).expect("the install");
        assert_eq!(w.skirmish_maps(), vec![MAP.to_ascii_lowercase()]);
        assert!(matches!(w.quick_skirmish(&fs, Some(b"Maps\\Nowhere\\Nowhere.map"), 1), Err(InstallError::MapNotInCache(_))));
        let q = w.quick_skirmish(&fs, None, 7).expect("the skirmish");
        eprintln!("C3: {q:?}");
        assert_eq!((q.map.as_slice(), q.mounted), (MAP.to_ascii_lowercase().as_slice(), 2));
        let mut local = Vec::new();
        for _ in 0..300 {
            w.engine_update().expect("a frame");
            local.push((w.game_started(), w.players.local_index()));
        }
        eprintln!("C3: started and local player by frame: {:?}", &local[..6]);
        // (the first frame dispatches `MSG_NEW_GAME`, the second starts the game and makes its players: the client waits
        // for the game's start, `client::import`. ~~`None` before~~ **Corrected 2026-11-06 (#64):** before the start the
        // local player is `ThePlayerList->init()`'s neutral one, 0, as upstream's engine has it)
        assert_eq!((local[0], local[1].0), ((false, Some(0)), true));
        assert!(local[1].1.is_some_and(|p| p > 0));
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        let tanks = w.objects.ids_in_order().into_iter().filter(|&o| w.find_object(o).is_some_and(|x| x.template().get_name() == b"Tank")).count();
        assert_eq!(tanks, 2);
    }

    /// Not a check: with `RTS_WRITE_TEST_INSTALL=<dir>` set, writes the test install's archives (`INIZH.big`, `EnglishZH.big`,
    /// `MapsZH.big` and `ScriptsZH.big`, the repository's own test data) into `<dir>`, for the web page's import to be tried
    /// in a browser without the game's files.
    #[test]
    fn write_the_test_install_when_asked() {
        let Ok(dir) = std::env::var("RTS_WRITE_TEST_INSTALL") else { return };
        let mut scratch = World::new();
        let map = duel_map(&mut scratch);
        let scb = skirmish_scripts(&mut scratch.names);
        std::fs::create_dir_all(&dir).expect("the folder");
        let mut archives = install_archives(&map);
        archives.push((b"ScriptsZH.big".to_vec(), build_big(&[(b"Data\\Scripts\\SkirmishScripts.scb", &scb)])));
        for (name, bytes) in archives {
            std::fs::write(std::path::Path::new(&dir).join(String::from_utf8_lossy(&name).as_ref()), bytes).expect("written");
        }
    }


    /// GitHub #58: the four-player game of [`a_save_starts_its_own_game_on_load`], started on an install's World: the
    /// World as the install made it (no game, the map mounted: the load's base) and the started game after `frames` frames.
    pub(crate) fn four_player_game(frames: usize) -> (World, World) {
        use crate::common::game_info::slot_state;
        let mut scratch = World::new();
        let map = four_map(&mut scratch);
        let scb = skirmish_scripts(&mut scratch.names);
        let fs = install_four(&map, Some(&scb));
        let (mut base, _) = World::from_install(&fs, DEFAULT_LANGUAGE).expect("the install");
        assert_eq!(base.mount_map(&fs, MAP), 2);
        let side = |w: &World, s: &[u8]| w.statics.player_template_store.templates().iter().position(|t| t.side() == s).expect("the side") as i32;
        let (america, china, gla) = (side(&base, b"America"), side(&base, b"China"), side(&base, b"GLA"));
        // (the human in slot 0 at the third start, three computer players on three sides, two teams: a layout the quick
        // skirmish, one human and one medium computer of the first playable side, never makes)
        let slots = [
            SkirmishSlot { state: slot_state::PLAYER, template: china, color: 2, start_pos: 2, team: 0 },
            SkirmishSlot { state: slot_state::MED_AI, template: america, color: 0, start_pos: 0, team: 0 },
            SkirmishSlot { state: slot_state::EASY_AI, template: gla, color: 1, start_pos: 1, team: 1 },
            SkirmishSlot { state: slot_state::BRUTAL_AI, template: china, color: 3, start_pos: 3, team: 1 },
        ];
        let mut w = base.clone();
        w.start_skirmish_slots(MAP, 11, &slots).expect("the skirmish");
        for _ in 0..frames {
            w.engine_update().expect("a frame");
        }
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        (base, w)
    }

    /// The players of a World: (name, side), in the list's order.
    pub(crate) fn player_sides(w: &World) -> Vec<(String, String)> {
        (0..w.players.player_count())
            .filter_map(|p| w.players.get_nth(p))
            .map(|p| (String::from_utf8_lossy(&p.player_name).into_owned(), String::from_utf8_lossy(&p.side).into_owned()))
            .collect()
    }

    /// GitHub #58: a save of a game other than the quick skirmish loads by starting its own game, as upstream's
    /// `GameStateMap::xfer` does (`startNewGame(TRUE)` with `TheSkirmishGameInfo` read from the save). A four-player game
    /// (the human with China at the third start, an America medium, a GLA easy and a China brutal computer player, two teams)
    /// runs 60 frames and is saved by the World (`World::save_game`); the save is loaded into two copies of the install's
    /// World with no game ([`SaveSide::new_game_on_load`](crate::logic::world::game_state_world::SaveSide)), and, as the
    /// control, into a World with the quick skirmish started (the client's way before #58).
    ///
    /// **Predicted before the run:** the save's `TheSkirmishGameInfo` carries the four slots (the loaded Worlds' skirmish
    /// game has the original's states, templates, colours, starts and teams; before #58 every save wrote an empty slot
    /// list); each load starts the game and gives the original's player list (names and sides) and local player; the two
    /// loads give the same logic CRC at each of the next 150 frames; no load or frame records an error; the control load
    /// fails (the quick skirmish has two slot players, the save four: `PlayerList::xfer`'s count check).
    ///
    /// **Results (2026-11-06, native, release), first run:** the four slots came back as saved
    /// (`(5,2,2,2,0) (3,0,0,0,0) (2,3,1,1,1) (4,2,3,3,1)`: state, template, colour, start, team); the players are the
    /// neutral one, `PlyrCivilian`, `player0` (China), `SkirmishAmerica0` (America, the skirmish AI), `player2` (GLA),
    /// `player3` (China) and `ReplayObserver`, the same after the load, with the same local player; the two loads agreed at all
    /// 150 frames; no error. **Miss (the prediction's):** the control failed one block earlier than predicted, in
    /// `CHUNK_TeamFactory` (`TeamFactory::xfer: InvalidData`): the team factory's block comes before the players' and its
    /// teams already differ. The object count check was added after the first run (it held).
    #[test]
    fn a_save_starts_its_own_game_on_load() {
        use crate::logic::world::game_logic_new_game::GameSource;
        use crate::logic::world::game_logic_update::CRC_RECALC;
        let (base, mut a) = four_player_game(60);
        let bytes = a.save_game().unwrap_or_else(|e| panic!("the save: {e:?} {:?}", a.cuts.team_live_errors));
        let load = |bytes: Vec<u8>| {
            let mut w = base.clone();
            w.save_side.new_game_on_load = true;
            w.load_game(bytes).unwrap_or_else(|e| panic!("the load: {e:?} {:?}", w.cuts.team_live_errors));
            assert!(!w.save_side.new_game_on_load, "the load started the game");
            w
        };
        let (mut b, mut c) = (load(bytes.clone()), load(bytes.clone()));
        let slots = |w: &World| {
            let g = w.cuts.new_game.games.iter().find(|(s, _)| *s == GameSource::Skirmish).map(|(_, g)| g.clone()).expect("TheSkirmishGameInfo");
            g.slots.iter().map(|s| (s.state, s.player_template, s.color, s.start_pos, s.team_number)).collect::<Vec<_>>()
        };
        eprintln!("slots {:?}", slots(&a));
        eprintln!("players {:?}", player_sides(&a));
        assert_eq!(slots(&b), slots(&a));
        assert_eq!(player_sides(&b), player_sides(&a));
        assert_eq!(b.players.local_index(), a.players.local_index());
        assert!(a.players.local_index().is_some());
        assert_eq!(b.objects.ids_in_order().len(), a.objects.ids_in_order().len());
        let crc = |w: &mut World| {
            w.engine_update().expect("a frame");
            w.logic_crc(CRC_RECALC, None).expect("the CRC")
        };
        for f in 0..150 {
            assert_eq!(crc(&mut b), crc(&mut c), "two loads of one save part at frame {f} after the load");
        }
        for w in [&a, &b, &c] {
            assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
        }

        // the control: the save into the quick skirmish's game
        let mut q = base.clone();
        q.quick_skirmish_mounted(MAP, 1).expect("the quick skirmish");
        for _ in 0..30 {
            q.engine_update().expect("a frame");
            if q.game_started() {
                break;
            }
        }
        let r = q.load_game(bytes);
        eprintln!("the control: {r:?} {:?}", q.cuts.team_live_errors);
        assert!(r.is_err(), "a four-player save does not load into the quick skirmish's game");
    }
}
