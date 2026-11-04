// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp (`init`'s registration of the save blocks,
//     `saveGame`/`loadGame`'s xfer of them, as the World holds the subsystems)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! Integration batch I10: the whole game saved and loaded. [`GameStateHost`] for World: the save file's blocks in upstream's
//! order (`GameState::init`) that the World holds, each xferred by its subsystem.
//!
//! **The blocks:** `CHUNK_GameState` (the save information), `CHUNK_TerrainLogic`, `CHUNK_TeamFactory`, `CHUNK_Players`,
//! `CHUNK_GameLogic` (the objects, the campaign, the caves, the polygon triggers: the AI is not saved upstream; the
//! pathfinder's map is rebuilt as the objects load), `CHUNK_ScriptEngine`, `CHUNK_SidesList` (the scripts' active flags)
//! and `CHUNK_Partition`; for a deep CRC the six logic blocks of `SNAPSHOT_DEEPCRC_LOGICONLY`. **Not registered:**
//! `CHUNK_Campaign` (the campaign manager is the cut table `cuts.campaign`, saved within `CHUNK_GameLogic` as upstream also
//! does), `CHUNK_GameStateMap` (it embeds the map file; the World is made from a map in memory, so a load is into a World
//! made from the same map: [`World::load_game`]'s caller's), and the client's (`CHUNK_Radar`, `CHUNK_TacticalView`,
//! `CHUNK_GameClient`, `CHUNK_InGameUI`, `CHUNK_ParticleSystem`, `CHUNK_TerrainVisual`, `CHUNK_GhostObject`). A file the
//! World saves therefore lacks blocks upstream's has; a load skips blocks it does not know by their size, as upstream's.
//!
//! **Post-processing:** upstream's load adds every snapshot it loads to the list, nested ones first; here each block's
//! post-process runs its nested ones first (each object's modules and the object, then the game logic; each team, then the
//! factory; each player, then the list), so the order across blocks and within each is upstream's.
//!
//! **The save information's time** is the save's local time upstream; here it is zero (a World is deterministic, and the
//! time is shown, never read back into the logic).
//!
//! `p1w13-sonnet`, integration batch I10, 2026-11-01.

use crate::common::game_state::{GameState, GameStateHost, SnapshotType};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferStatus};
use crate::common::xfer_load::XferLoad;
use crate::common::xfer_save::XferSave;
use crate::logic::world::game_logic::World;

/// A save block of the World (`GameState`'s registered snapshots).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldBlock {
    GameState,
    GameStateMap,
    /// `TheSkirmishGameInfo` and its starting cash (post-processed after a load, their `loadPostProcess` empty).
    SkirmishGameInfo,
    SkirmishCash,
    TerrainLogic,
    TeamFactory,
    Players,
    GameLogic,
    ScriptEngine,
    SidesList,
    Partition,
}

/// `GameState::init`'s registration, of the blocks the World holds (see the module docs).
pub fn world_game_state() -> GameState<WorldBlock> {
    use WorldBlock as B;
    let mut gs = GameState::new();
    for (name, b) in [
        (&b"CHUNK_GameState"[..], B::GameState),
        (b"CHUNK_GameStateMap", B::GameStateMap),
        (b"CHUNK_TerrainLogic", B::TerrainLogic),
        (b"CHUNK_TeamFactory", B::TeamFactory),
        (b"CHUNK_Players", B::Players),
        (b"CHUNK_GameLogic", B::GameLogic),
        (b"CHUNK_ScriptEngine", B::ScriptEngine),
        (b"CHUNK_SidesList", B::SidesList),
        (b"CHUNK_Partition", B::Partition),
    ] {
        gs.add_snapshot_block(name, b, SnapshotType::SaveLoad);
    }
    for (name, b) in [
        (&b"CHUNK_TeamFactory"[..], B::TeamFactory),
        (b"CHUNK_Players", B::Players),
        (b"CHUNK_GameLogic", B::GameLogic),
        (b"CHUNK_ScriptEngine", B::ScriptEngine),
        (b"CHUNK_SidesList", B::SidesList),
        (b"CHUNK_Partition", B::Partition),
    ] {
        gs.add_snapshot_block(name, b, SnapshotType::DeepCrcLogicOnly);
    }
    gs
}

impl World {
    /// A block's error that is not an `XferStatus`, recorded; the xfer sees `XFER_UNKNOWN_ERROR`.
    fn block_error(&mut self, call: &str, e: impl std::fmt::Debug) -> XferStatus {
        self.cuts.team_live_errors.push(format!("{call}: {e:?}"));
        XferStatus::ErrorUnknown
    }

    /// `GameState::saveGame`'s data: every save block into a save file's bytes.
    pub fn save_game(&mut self) -> XferResult<Vec<u8>> {
        let mut gs = world_game_state();
        let mut x = XferSave::new();
        x.open(b"save game")?;
        gs.xfer_save_data(&mut x, SnapshotType::SaveLoad, self)?;
        x.close()?;
        Ok(x.take_bytes())
    }

    /// `GameState::loadGame`'s data: a save file's blocks loaded into this World (made from the same map, see the module
    /// docs), then post-processed in load order.
    pub fn load_game(&mut self, bytes: Vec<u8>) -> XferResult<()> {
        let mut gs = world_game_state();
        let mut x = XferLoad::new(bytes);
        x.open(b"save game")?;
        gs.xfer_save_data(&mut x, SnapshotType::SaveLoad, self)?;
        x.close()?;
        gs.game_state_post_process_load(self);
        Ok(())
    }
}

impl GameStateHost<WorldBlock> for World {
    fn xfer_block(&mut self, block: WorldBlock, gs: &mut GameState<WorldBlock>, xfer: &mut dyn Xfer) -> XferResult<()> {
        match block {
            WorldBlock::GameState => gs.xfer_info(xfer, self),
            WorldBlock::GameStateMap => {
                crate::common::game_state_map::xfer_game_state_map(gs, xfer, self).map_err(|e| self.block_error("GameStateMap::xfer", e))
            }
            WorldBlock::SkirmishGameInfo | WorldBlock::SkirmishCash => Ok(()),
            WorldBlock::TerrainLogic => match self.with_terrain(|t, w| t.xfer(xfer, w)) {
                Ok(()) => Ok(()),
                Err(e) => Err(self.block_error("TerrainLogic::xfer", e)),
            },
            WorldBlock::TeamFactory => crate::logic::world::team_xfer::xfer_team_factory(self, xfer).map_err(|e| self.block_error("TeamFactory::xfer", e)),
            WorldBlock::Players => crate::logic::world::player_list::player_list_xfer(self, xfer).map_err(|e| self.block_error("PlayerList::xfer", e)),
            WorldBlock::GameLogic => self.xfer_game_logic_block(xfer).map_err(|e| self.block_error("GameLogic::xfer", e)),
            WorldBlock::ScriptEngine => {
                let mut se = std::mem::take(&mut self.script_engine);
                // (integration batch I29: the notifications not yet taken, applied before the engine is saved)
                let notes = std::mem::take(&mut self.cuts.pending_script_notes);
                se.apply_notes(self, notes);
                let r = se.xfer(self, xfer);
                // (integration batch I49: a loaded `m_gameDifficulty` reaches the World's copy, which the players read)
                self.cuts.player_host.global_difficulty = crate::logic::world::player_init::difficulty_from_int(se.game_difficulty);
                self.script_engine = se;
                r
            }
            WorldBlock::SidesList => {
                let mut sides = std::mem::take(&mut self.sides);
                let r = sides.xfer(xfer);
                self.sides = sides;
                r.map_err(|e| e.unwrap_or_else(|| self.block_error("SidesList::xfer", "SC_INVALID_DATA")))
            }
            WorldBlock::Partition => {
                let mut pm = std::mem::take(&mut self.partition);
                let r = xfer.xfer_snapshot(&mut pm);
                self.partition = pm;
                if r.is_ok() && xfer.get_xfer_mode() == XferMode::Load {
                    // ("refresh the shroud for the local player": the caller's, as the partition's xfer leaves it marked)
                }
                r
            }
        }
    }

    fn load_post_process(&mut self, block: WorldBlock) {
        match block {
            WorldBlock::GameLogic => {
                // (the nested snapshots first: each object's modules, then the object; then `GameLogic::loadPostProcess`)
                for id in self.objects.ids_in_order() {
                    let n = self.find_object(id).map_or(0, |o| o.modules.len());
                    for idx in 0..n {
                        self.load_post_process_module_slot(id, idx);
                    }
                    self.object_load_post_process(id);
                }
                World::load_post_process(self);
            }
            WorldBlock::TeamFactory => {
                let teams: Vec<_> = self.teams.teams().map(|t| t.id).collect();
                for t in teams {
                    if let Err(e) = crate::logic::world::team_xfer::team_load_post_process(self, t) {
                        self.block_error("Team::loadPostProcess", e);
                    }
                }
                crate::logic::world::team_xfer::factory_load_post_process(self);
            }
            WorldBlock::Players => {
                for p in 0..self.players.player_count() {
                    if let Err(e) = crate::logic::world::player_xfer::player_load_post_process(self, p) {
                        self.block_error("Player::loadPostProcess", e);
                    }
                }
                crate::logic::world::player_list::player_list_load_post_process(self);
            }
            WorldBlock::ScriptEngine => {
                let mut se = std::mem::take(&mut self.script_engine);
                let r = se.load_post_process(self);
                self.script_engine = se;
                if let Err(e) = r {
                    self.block_error("ScriptEngine::loadPostProcess", e);
                }
            }
            WorldBlock::TerrainLogic => self.with_terrain(|t, w| t.load_post_process(w)),
            WorldBlock::GameState
            | WorldBlock::GameStateMap
            | WorldBlock::SkirmishGameInfo
            | WorldBlock::SkirmishCash
            | WorldBlock::SidesList
            | WorldBlock::Partition => {}
        }
    }

    fn partition_update(&mut self) {
        World::partition_update(self);
    }
    fn user_data_path(&self) -> Vec<u8> {
        Vec::new()
    }
    fn map_name(&self) -> Vec<u8> {
        self.save_side.map_name.clone()
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
        self.save_side.disk.contains_key(&path.to_ascii_lowercase())
    }
}

/// The save's side of the World (batch I10): what upstream keeps in the global data, the game client and the file system
/// that `GameStateMap` reads and writes.
#[derive(Clone, Debug, Default)]
pub struct SaveSide {
    /// `TheGlobalData->m_mapName`: the map the World is made from (its file on [`SaveSide::disk`]).
    pub map_name: Vec<u8>,
    /// The files, by lower-case path: the map, and the copy a load extracts to the save directory.
    pub disk: std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
    /// The file open (its lower-case path) and the position in it.
    open: Vec<u8>,
    pos: usize,
    /// `TheGameLogic->setLoadingSave(on)`.
    pub loading_save: bool,
    /// `TheGameClient`'s drawable id counter (the client's; kept so a save writes what it read).
    pub drawable_id_counter: crate::common::game_type::DrawableId,
    /// `TheSkirmishGameInfo`.
    pub skirmish: Option<crate::common::game_info::GameInfo>,
}

impl crate::common::game_info::GameInfoHost for World {
    // (the skirmish game's options are xferred only in a skirmish; the World's games are not, and these are its fixed answers)
    fn game_text(&mut self, _label: &str) -> Vec<u16> {
        Vec::new()
    }
    fn tick_count(&mut self) -> u32 {
        0
    }
    fn default_starting_cash(&self) -> crate::common::money::Money {
        crate::common::money::Money::new()
    }
    fn net_crc_interval(&self) -> i32 {
        crate::logic::world::game_logic_update::GameLogicHost::u_replay_crc_interval(self)
    }
    fn has_map_cache(&self) -> bool {
        false
    }
    fn map_cache_find(&self, _lower_name: &[u8]) -> Option<crate::common::game_info::MapMeta> {
        None
    }
    fn file_exists(&mut self, path: &[u8]) -> bool {
        self.save_side.disk.contains_key(&path.to_ascii_lowercase())
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        GameState::<WorldBlock>::new().real_map_path_to_portable(path, self).unwrap_or_default()
    }
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8> {
        GameState::<WorldBlock>::new().portable_map_path_to_real(path, self).unwrap_or_default()
    }
    fn has_multiplayer_settings(&self) -> bool {
        false
    }
    fn num_colors(&self) -> i32 {
        0
    }
    fn show_random_player_template(&self) -> bool {
        false
    }
    fn show_random_color(&self) -> bool {
        false
    }
    fn show_random_start_pos(&self) -> bool {
        false
    }
    fn observer_color(&self) -> Result<i32, crate::common::ini::IniError> {
        Ok(0)
    }
    fn player_template_count(&self) -> i32 {
        0
    }
    fn player_template_display_name(&self, _n: i32) -> Result<Vec<u16>, crate::common::ini::IniError> {
        Ok(Vec::new())
    }
    fn record_income(&mut self, _player_index: i32) {}
}

impl crate::common::game_state_map::GameStateMapHost<WorldBlock> for World {
    fn set_loading_save(&mut self, on: bool) {
        self.save_side.loading_save = on;
    }
    fn logic_game_mode(&self) -> i32 {
        self.cuts.game_loop.game_mode
    }
    fn set_logic_game_mode(&mut self, mode: i32) {
        self.cuts.game_loop.game_mode = mode;
    }
    fn set_map_name(&mut self, name: &[u8]) {
        self.save_side.map_name = name.to_vec();
    }
    fn object_id_counter(&self) -> crate::common::game_type::ObjectId {
        self.next_object_id()
    }
    fn set_object_id_counter(&mut self, id: crate::common::game_type::ObjectId) {
        self.set_next_object_id(id);
    }
    fn drawable_id_counter(&self) -> crate::common::game_type::DrawableId {
        self.save_side.drawable_id_counter
    }
    fn set_drawable_id_counter(&mut self, id: crate::common::game_type::DrawableId) {
        self.save_side.drawable_id_counter = id;
    }
    /// (a load's new game: the World is already made from the map the save names, by [`World::load_game`]'s caller)
    fn start_new_game(&mut self, _loading_save_game: bool) {}
    fn skirmish_game_info(&mut self) -> &mut Option<crate::common::game_info::GameInfo> {
        &mut self.save_side.skirmish
    }
    fn skirmish_block(&self) -> WorldBlock {
        WorldBlock::SkirmishGameInfo
    }
    fn skirmish_cash_block(&self) -> WorldBlock {
        WorldBlock::SkirmishCash
    }
    fn fs_open(&mut self, path: &[u8]) -> bool {
        self.save_open(path, false)
    }
    fn fs_seek_end(&mut self) -> i32 {
        let n = self.save_side.disk.get(&self.save_side.open).map_or(0, Vec::len);
        self.save_side.pos = n;
        n as i32
    }
    fn fs_seek_start(&mut self) {
        self.save_side.pos = 0;
    }
    fn fs_read(&mut self, n: usize) -> Vec<u8> {
        self.save_read(n)
    }
    fn fs_close(&mut self) {
        self.save_side.open.clear();
    }
    fn c_fopen(&mut self, path: &[u8], mode: &str) -> bool {
        self.save_open(path, mode.contains('w'))
    }
    fn c_size(&mut self) -> i32 {
        self.save_side.pos = 0;
        self.save_side.disk.get(&self.save_side.open).map_or(0, Vec::len) as i32
    }
    fn c_fread(&mut self, n: usize) -> Vec<u8> {
        self.save_read(n)
    }
    fn c_fwrite(&mut self, data: &[u8]) -> usize {
        let key = self.save_side.open.clone();
        self.save_side.disk.entry(key).or_default().extend_from_slice(data);
        data.len()
    }
    fn c_fclose(&mut self) {
        self.save_side.open.clear();
    }
    fn current_directory(&mut self) -> Vec<u8> {
        Vec::new()
    }
    fn set_current_directory(&mut self, _dir: &[u8]) {}
    fn find_first_file(&mut self, _pattern: &[u8]) -> Option<crate::common::game_state_map::FindItem> {
        None
    }
    fn find_next_file(&mut self) -> Option<crate::common::game_state_map::FindItem> {
        None
    }
    fn find_close(&mut self) {}
    fn delete_file(&mut self, name: &[u8]) {
        self.save_side.disk.remove(&name.to_ascii_lowercase());
    }
}

impl World {
    /// A file opened on the save side's disk: for reading, it must exist; for writing, it is made empty.
    fn save_open(&mut self, path: &[u8], write: bool) -> bool {
        let key = path.to_ascii_lowercase();
        if write {
            self.save_side.disk.insert(key.clone(), Vec::new());
        } else if !self.save_side.disk.contains_key(&key) {
            return false;
        }
        self.save_side.open = key;
        self.save_side.pos = 0;
        true
    }

    fn save_read(&mut self, n: usize) -> Vec<u8> {
        let Some(d) = self.save_side.disk.get(&self.save_side.open) else { return Vec::new() };
        let got = n.min(d.len().saturating_sub(self.save_side.pos));
        let out = d[self.save_side.pos..self.save_side.pos + got].to_vec();
        self.save_side.pos += got;
        out
    }
}
