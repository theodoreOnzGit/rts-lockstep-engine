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
//! ~~`CHUNK_Campaign` (the campaign manager is the cut table `cuts.campaign`, saved within `CHUNK_GameLogic` as upstream also
//! does),~~ (registered since GitHub #58, below) `CHUNK_GameStateMap` (it embeds the map file; the World is made from a map in memory, so a load is into a World
//! made from the same map: [`World::load_game`]'s caller's), and the client's (`CHUNK_Radar`, `CHUNK_TacticalView`,
//! `CHUNK_GameClient`, `CHUNK_InGameUI`, `CHUNK_ParticleSystem`, ~~`CHUNK_TerrainVisual`,~~ `CHUNK_GhostObject`). A file the
//! World saves therefore lacks blocks upstream's has; a load skips blocks it does not know by their size, as upstream's.
//! **Corrected 2026-11-06 (GitHub #58):** `CHUNK_Campaign` is registered, second as upstream's, and xferred by
//! `CampaignManager::xfer` over the World's campaign stand-in (`cuts.campaign`, which `CHUNK_GameLogic` also xfers, as
//! upstream's `GameLogic::xfer` does); ~~a campaign's save fails there (`SC_INVALID_DATA`: the port has no campaigns)~~
//! **corrected 2026-11-06 (GitHub #61):** over the World's `TheCampaignManager` (`World::campaign`, translated), so a
//! campaign's or a challenge's save loads (its campaign and mission set before `CHUNK_GameStateMap` starts the game), and
//! the block's `loadPostProcess` gives `TheChallengeGenerals` its player template number. And
//! `CHUNK_GameStateMap`'s `startNewGame(TRUE)` starts the save's game when the World was reset for the load
//! ([`SaveSide::new_game_on_load`]); otherwise the World is still taken to be made from the save's map by
//! [`World::load_game`]'s caller.
//!
//! **Corrected 2026-11-06 (GitHub #4):** `CHUNK_GameStateMap` is registered (since batch I10, as the list above the
//! correction already says); `CHUNK_TerrainVisual` is now the World's own block (`WorldBlock::TerrainVisual`,
//! [`World::xfer_terrain_visual_part`]): its data is the logic height map the ground heights are read from and the water
//! grid, both held by the World, so a save restores a height a script or a crater lowered. The client's other blocks are
//! written and read by `crate::client::save_game`, which saves the whole file in upstream's order; a World saving alone
//! still lacks them.
//!
//! **Post-processing:** upstream's load adds every snapshot it loads to the list, nested ones first; here each block's
//! post-process runs its nested ones first (each object's modules and the object, then the game logic; each team, then the
//! factory; each player, then the list), so the order across blocks and within each is upstream's.
//!
//! **The save information's time** is the save's local time upstream; here it is zero (a World is deterministic, and the
//! time is shown, never read back into the logic).
//!
//! `p1w13-sonnet`, integration batch I10, 2026-11-01.

use crate::common::game_state::{GameState, GameStateHost, SnapshotType, SAVE_FILE_TYPE_MISSION};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::common::xfer_load::XferLoad;
use crate::common::xfer_save::XferSave;
use crate::logic::world::game_logic::World;

/// A save block of the World (`GameState`'s registered snapshots).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldBlock {
    GameState,
    /// GitHub #58: `CHUNK_Campaign` (`CampaignManager::xfer`, ~~the World's campaign stand-in `cuts.campaign`~~ the World's
    /// `TheCampaignManager` since GitHub #61).
    Campaign,
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
    /// GitHub #4: `CHUNK_TerrainVisual` (`W3DTerrainVisual::xfer`): the logic height map's heights and the water grid,
    /// which the World holds ([`World::xfer_terrain_visual`]).
    TerrainVisual,
}

/// `GameState::init`'s registration, of the blocks the World holds (see the module docs).
pub fn world_game_state() -> GameState<WorldBlock> {
    use WorldBlock as B;
    let mut gs = GameState::new();
    for (name, b) in [
        (&b"CHUNK_GameState"[..], B::GameState),
        (b"CHUNK_Campaign", B::Campaign),
        (b"CHUNK_GameStateMap", B::GameStateMap),
        (b"CHUNK_TerrainLogic", B::TerrainLogic),
        (b"CHUNK_TeamFactory", B::TeamFactory),
        (b"CHUNK_Players", B::Players),
        (b"CHUNK_GameLogic", B::GameLogic),
        (b"CHUNK_ScriptEngine", B::ScriptEngine),
        (b"CHUNK_SidesList", B::SidesList),
        (b"CHUNK_Partition", B::Partition),
        (b"CHUNK_TerrainVisual", B::TerrainVisual),
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

    /// GitHub #62: `GameState::missionSave()`'s `saveGame(AsciiString(""), desc, SAVE_FILE_TYPE_MISSION)`: the save file
    /// type a mission's, its mission map `TheCampaignManager->getCurrentMap()` (after the score screen's
    /// `gotoNextMission`: the next mission's), and only `CHUNK_GameState` and `CHUNK_Campaign` written (`xferSaveData`).
    /// The description is the caller's ([`World::mission_save_description`]).
    pub fn mission_save(&mut self, description: &[u16]) -> XferResult<Vec<u8>> {
        let mut gs = world_game_state();
        gs.info.description = description.to_vec();
        gs.info.save_file_type = SAVE_FILE_TYPE_MISSION;
        gs.info.mission_map_name = self.campaign.get_current_map();
        let mut x = XferSave::new();
        x.open(b"save game")?;
        gs.xfer_save_data(&mut x, SnapshotType::SaveLoad, self)?;
        x.close()?;
        Ok(x.take_bytes())
    }

    /// `GameState::loadGame`'s data: a save file's blocks loaded into this World (made from the same map, see the module
    /// docs), then post-processed in load order. (#62) A mission save's (`SAVE_FILE_TYPE_MISSION`: its campaign alone)
    /// then starts its mission, as `loadGame`'s end does ([`World::start_loaded_mission`]); its map must be on the
    /// World's disk before the next frame.
    ///
    /// GitHub #64: a World that starts the save's own game ([`SaveSide::new_game_on_load`]) is cleared first, as upstream's
    /// load is: a game in progress by `clearGameData(FALSE)` (the save menu's `doLoadGame`, and `loadGame`'s own for a
    /// mission save), then `TheGameEngine->reset()` ([`World::engine_reset`]); a load that fails clears it again, as
    /// `loadGame`'s error branch does. So a load can be into the World that was playing, in place.
    ///
    /// **Deviation (#64):** the random generators then start from their state before any `InitRandom`, the install
    /// World's ([`World::reset_random_for_load`]). Upstream's load seeds nothing (neither the save nor `loadGame` holds a
    /// seed), so its loaded game draws from whatever the engine's generators hold after the games before it, and two
    /// machines would not agree; every load into a fresh copy of the install's World drew from that state.
    pub fn load_game(&mut self, bytes: Vec<u8>) -> XferResult<()> {
        let in_place = self.save_side.new_game_on_load;
        if in_place {
            if crate::common::recorder::RecorderHost::is_in_game(self) {
                self.clear_game_data(false);
            }
            self.engine_reset();
            self.reset_random_for_load();
        }
        let r = self.load_game_data(bytes);
        if r.is_err() && in_place {
            if crate::common::recorder::RecorderHost::is_in_game(self) {
                self.clear_game_data(false);
            }
            self.engine_reset();
        }
        r
    }

    /// [`World::load_game`]'s reading and post-process.
    fn load_game_data(&mut self, bytes: Vec<u8>) -> XferResult<()> {
        let mut gs = world_game_state();
        let mut x = XferLoad::new(bytes);
        x.open(b"save game")?;
        gs.xfer_save_data(&mut x, SnapshotType::SaveLoad, self)?;
        x.close()?;
        gs.game_state_post_process_load(self);
        if gs.info.save_file_type == SAVE_FILE_TYPE_MISSION {
            self.start_loaded_mission(&gs.info.mission_map_name);
        }
        Ok(())
    }
}

impl GameStateHost<WorldBlock> for World {
    fn xfer_block(&mut self, block: WorldBlock, gs: &mut GameState<WorldBlock>, xfer: &mut dyn Xfer) -> XferResult<()> {
        match block {
            WorldBlock::GameState => gs.xfer_info(xfer, self),
            WorldBlock::Campaign => <World as crate::logic::world::game_logic_xfer::GameLogicSaveHost>::x_xfer_campaign_manager(self, xfer)
                .map_err(|e| self.block_error("CampaignManager::xfer", e)),
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
            WorldBlock::TerrainVisual => self.xfer_terrain_visual(xfer).map_err(|e| self.block_error("W3DTerrainVisual::xfer", e)),
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
            // (GitHub #61: `CampaignManager::loadPostProcess` gives `TheChallengeGenerals` its template number)
            WorldBlock::Campaign => {
                let c = self.campaign.clone();
                c.load_post_process(self);
            }
            WorldBlock::GameState
            | WorldBlock::GameStateMap
            | WorldBlock::SkirmishGameInfo
            | WorldBlock::SkirmishCash
            | WorldBlock::SidesList
            | WorldBlock::Partition
            // (`W3DTerrainVisual::loadPostProcess` is `TerrainVisual::loadPostProcess`, empty)
            | WorldBlock::TerrainVisual => {}
        }
    }

    fn partition_update(&mut self) {
        World::partition_update(self);
    }
    fn user_data_path(&self) -> Vec<u8> {
        // (~~empty~~ batch I62: `TheGlobalData->getPath_UserData()`, as the recorder's and the map cache's (batch I57): an install's
        // saves go under its user data directory)
        crate::common::recorder::RecorderHost::user_data_path(self)
    }
    fn map_name(&self) -> Vec<u8> {
        // (GitHub #4: both are `TheGlobalData->m_mapName`; a game started from an install sets the new game's, the save side's
        // stays empty until a load sets it, so a skirmish's first save named no map and could not embed it)
        if self.save_side.map_name.is_empty() {
            return self.cuts.new_game.map_name.clone();
        }
        self.save_side.map_name.clone()
    }
    fn local_time(&mut self) -> [u16; 8] {
        [0; 8]
    }
    fn world_dict_map_name(&self) -> Option<Vec<u8>> {
        None
    }
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)> {
        // (GitHub #61: `TheCampaignManager`'s; was `None`, the port having no campaigns)
        let c = self.campaign.get_current_campaign()?;
        Some((c.name.clone(), self.campaign.get_current_mission_number()))
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
    /// ~~`TheSkirmishGameInfo`.~~ **Corrected 2026-11-06 (GitHub #58):** `TheSkirmishGameInfo` is the new game's skirmish
    /// entry (`cuts.new_game.games`, [`GameSource::Skirmish`](crate::logic::world::game_logic_new_game::GameSource)), the one
    /// the skirmish menu fills and `startNewGame` reads. This field was a second copy that no game ever filled, so every save
    /// of a skirmish wrote an empty slot list (`clearSlotList`'s) where upstream writes the game's slots.
    ///
    /// GitHub #58: the World has been reset for a load (upstream's `TheGameEngine->reset()` in `GameState::loadGame`), so the
    /// save's `CHUNK_GameStateMap` starts the save's own game in it (`startNewGame(TRUE)`, with the save's map and
    /// `TheSkirmishGameInfo`). Off (the default), the World is taken to be made from the save's map already, as
    /// [`World::load_game`]'s callers have always made it (the gate world's tests), and the block starts nothing. Cleared
    /// once the game is started.
    pub new_game_on_load: bool,
}

impl crate::common::game_info::GameInfoHost for World {
    // (the skirmish game's options are xferred only in a skirmish; the World's games are not, and these are its fixed answers)
    fn game_text(&mut self, label: &str) -> Vec<u16> {
        // (~~empty~~ batch I62: `TheGameText->fetch(label)`, the statics' game text (batch I57): an AI slot's name is its
        // `GUI:EasyAI`-style label's text)
        crate::common::science::GameTextFetch::fetch(&self.statics.game_text, label.as_bytes()).str().to_vec()
    }
    fn tick_count(&mut self) -> u32 {
        0
    }
    fn default_starting_cash(&self) -> crate::common::money::Money {
        // ~~`Money::new()`~~ **Corrected 2026-11-04 (batch I56):** `TheGlobalData->m_defaultStartingCash`, the statics' global data
        // (it was an empty purse, so every slot of a game read from a save or a replay header started with no money)
        self.statics.global_data.the_global_data().map(|g| g.default_starting_cash).unwrap_or_default()
    }
    fn net_crc_interval(&self) -> i32 {
        crate::logic::world::game_logic_update::GameLogicHost::u_replay_crc_interval(self)
    }
    fn has_map_cache(&self) -> bool {
        // ~~false~~ **Corrected 2026-11-04 (batch I55):** `TheMapCache` exists in every game (the client makes it at
        // start), and a replay's header (`ParseAsciiStringToGameInfo`) dereferences it; ~~it is the World's map cache
        // table (`cuts.map_cache`) until `MapUtil.cpp` is translated~~ **corrected 2026-11-04 (batch I56):** the
        // statics' `MapCache`, translated, which `World::update_map_cache` builds from the disk
        true
    }
    fn map_cache_find(&self, lower_name: &[u8]) -> Option<crate::common::game_info::MapMeta> {
        // (~~batch I55: the table's entry; its CRC is not kept, 0~~ batch I56: the cache's entry, with its CRC)
        self.statics.map_cache.maps.get(lower_name).map(|m| crate::common::game_info::MapMeta { num_players: m.num_players, crc: m.crc })
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
        // ~~false~~ **Corrected 2026-11-04 (batch I55):** `TheMultiplayerSettings` exists in every game, and a replay's
        // slot list asks it for its colour count. ~~its colours are the World's colour table (`cuts.new_game.colors`)~~
        // **Corrected 2026-11-04 (batch I56):** the statics' settings, read from `Multiplayer.ini` (#15)
        true
    }
    fn num_colors(&self) -> i32 {
        self.statics.multiplayer_settings.get_num_colors()
    }
    // ~~false, false, false and an observer colour of 0~~ **Corrected 2026-11-04 (batch I56):** the statics' settings (the
    // constructor's are TRUE, TRUE, TRUE and white)
    fn show_random_player_template(&self) -> bool {
        self.statics.multiplayer_settings.show_random_player_template()
    }
    fn show_random_color(&self) -> bool {
        self.statics.multiplayer_settings.show_random_color()
    }
    fn show_random_start_pos(&self) -> bool {
        self.statics.multiplayer_settings.show_random_start_pos()
    }
    fn observer_color(&self) -> Result<i32, crate::common::ini::IniError> {
        let c = self.statics.multiplayer_settings.color_at(crate::common::multiplayer_settings::PLAYERTEMPLATE_OBSERVER);
        Ok(c.map_or(-1, |c| c.color()))
    }
    fn player_template_count(&self) -> i32 {
        // ~~0~~ **Corrected 2026-11-04 (batch I55):** `ThePlayerTemplateStore->getPlayerTemplateCount()`, the statics'
        // store (it was 0, so no game's slot list parsed: every slot's template was out of range)
        self.statics.player_template_store.player_template_count()
    }
    fn player_template_display_name(&self, n: i32) -> Result<Vec<u16>, crate::common::ini::IniError> {
        // (batch I55: `getNthPlayerTemplate(n)->getDisplayName()`, dereferenced unchecked upstream)
        let t = self.statics.player_template_store.nth_player_template(n).ok_or_else(|| crate::common::ini::IniError::undefined(crate::common::ini::Undefined::NullObject))?;
        Ok(t.display_name().str().to_vec())
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
    /// `TheGameLogic->startNewGame(TRUE)` (GitHub #58) when the World was reset for the load
    /// ([`SaveSide::new_game_on_load`]): the new game on `TheGlobalData->m_mapName`, which the block has just set to the map
    /// it extracted to the save directory (`map.ini` is then read beside the pristine map, `TheGameState`'s, as upstream's
    /// `startNewGame` does for a map in the save directory), with the game mode and `TheSkirmishGameInfo` the block read.
    /// `TheRecorder` is an engine subsystem upstream (`GameEngine::init`), always there; a World without one is given one
    /// here, as [`World::start_skirmish_against`] does. Otherwise nothing: the World is already made from the save's map,
    /// by [`World::load_game`]'s caller.
    fn start_new_game(&mut self, gs: &GameState<WorldBlock>, loading_save_game: bool) -> Result<(), crate::common::game_state::SaveError> {
        if !self.save_side.new_game_on_load {
            return Ok(());
        }
        self.save_side.new_game_on_load = false;
        let map = self.save_side.map_name.clone();
        let ng = &mut self.cuts.new_game;
        ng.map_name = map.clone();
        ng.pristine_map_name = gs.info.pristine_map_name.clone();
        if gs.is_in_save_directory(&map, self) && !self.cuts.new_game.save_directory.contains(&map) {
            self.cuts.new_game.save_directory.push(map);
        }
        if self.cuts.game_loop.recorder.is_none() {
            self.recorder_init();
        }
        World::start_new_game(self, loading_save_game)?;
        Ok(())
    }
    fn take_skirmish_game_info(&mut self) -> Option<crate::common::game_info::GameInfo> {
        use crate::logic::world::game_logic_new_game::GameSource;
        let games = &mut self.cuts.new_game.games;
        let at = games.iter().position(|(s, _)| *s == GameSource::Skirmish)?;
        Some(games.remove(at).1)
    }
    fn set_skirmish_game_info(&mut self, game: Option<crate::common::game_info::GameInfo>) {
        use crate::logic::world::game_logic_new_game::GameSource;
        let games = &mut self.cuts.new_game.games;
        games.retain(|(s, _)| *s != GameSource::Skirmish);
        if let Some(g) = game {
            games.push((GameSource::Skirmish, g));
        }
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

/// GitHub #4: the version this port writes of `W3DTerrainVisual::xfer` (upstream's current is 3). Version 3 adds the
/// terrain render object's tree and prop buffers (`BaseHeightMapRenderObjClass::xfer`); this port draws neither, and a
/// version 3 block with no trees would make upstream's load delete the map's trees (`W3DTreeBuffer::xfer` empties the
/// buffer before reading), so it writes version 2, which upstream loads without touching them.
pub const TERRAIN_VISUAL_SAVE_VERSION: XferVersion = 2;

impl World {
    /// GitHub #4: `W3DTerrainVisual::xfer` (current version 3, written as [`TERRAIN_VISUAL_SAVE_VERSION`]) and the
    /// `TerrainVisual::xfer` it extends (version 1, no data), over the World's terrain visual (the logic height map the
    /// ground heights are read from, and the water grid; `TerrainLogic::visual`):
    ///
    /// - the water grid's rendering flag, which must equal the World's (`SC_INVALID_DATA` otherwise, as upstream), and, if
    ///   it is on, `WaterRenderObjClass::xfer` (version 1): the cell counts, which must equal the grid's, then each mesh
    ///   entry's height, velocity, status and preferred height. The port's grid holds the heights only (the velocity, status
    ///   and preferred height are the client's water simulation, not translated): a load keeps the heights and reads past the
    ///   rest; a save writes them as 0.
    /// - (version 2) the logic height map's samples (`xExtent * yExtent` of them, as an `Int` and the bytes). A save whose
    ///   count differs is read for the smaller of the two, as upstream (`DEBUG_CRASH` and `len = xferLen` when the save's is
    ///   shorter). Upstream then tells the render object its lighting changed (the client's).
    /// - (version 3) `BaseHeightMapRenderObjClass::xfer` (the trees and props): not translated, read past to the block's
    ///   end by the caller ([`crate::client::save_game`] knows where the block ends; a World loading alone has the block
    ///   skipped by `GameState` only when it is not registered).
    ///
    /// Returns `Ok(true)` when the block's remainder is a version 3 tree and prop part this function did not read.
    pub fn xfer_terrain_visual_part(&mut self, xfer: &mut dyn Xfer) -> Result<bool, crate::common::game_state::SaveError> {
        use crate::common::game_state::SaveError;
        let current: XferVersion = 3;
        let mut version = if xfer.get_xfer_mode() == XferMode::Save { TERRAIN_VISUAL_SAVE_VERSION } else { current };
        xfer.xfer_version(&mut version, current)?;
        // TerrainVisual::xfer
        let mut base: XferVersion = 1;
        xfer.xfer_version(&mut base, 1)?;
        let enabled = self.terrain.visual().water_grid.rendering_enabled();
        let mut grid_enabled = enabled;
        xfer.xfer_bool(&mut grid_enabled)?;
        if grid_enabled != enabled {
            return Err(SaveError::InvalidData);
        }
        if grid_enabled {
            // WaterRenderObjClass::xfer
            let mut wv: XferVersion = 1;
            xfer.xfer_version(&mut wv, 1)?;
            let (cx, cy) = self.terrain.visual().water_grid.grid_cells();
            let (mut x, mut y) = (cx, cy);
            xfer.xfer_int(&mut x)?;
            if x != cx {
                return Err(SaveError::InvalidData);
            }
            xfer.xfer_int(&mut y)?;
            if y != cy {
                return Err(SaveError::InvalidData);
            }
            let mut heights = self.terrain.visual_mut().water_grid.mesh_heights_mut().map(std::mem::take).unwrap_or_default();
            let mut r = Ok(());
            for h in heights.iter_mut() {
                let (mut velocity, mut status, mut preferred) = (0.0f32, 0u8, 0u8);
                r = xfer.xfer_real(h).and_then(|_| xfer.xfer_real(&mut velocity)).and_then(|_| xfer.xfer_unsigned_byte(&mut status)).and_then(|_| xfer.xfer_unsigned_byte(&mut preferred));
                if r.is_err() {
                    break;
                }
            }
            if let Some(m) = self.terrain.visual_mut().water_grid.mesh_heights_mut() {
                *m = heights;
            }
            r?;
        }
        if version >= 2 {
            let visual = self.terrain.visual_mut();
            let Some(map) = visual.height_map.as_mut() else {
                // (m_logicHeightMap->getDataPtr() of a NULL map)
                return Err(SaveError::Undefined(crate::common::ini::IniError::undefined(crate::common::ini::Undefined::NullObject)));
            };
            let mut len = (map.x_extent() * map.y_extent()) as usize;
            let mut xfer_len = len as i32;
            xfer.xfer_int(&mut xfer_len)?;
            if len as i32 != xfer_len && len as i64 > i64::from(xfer_len) {
                len = xfer_len.max(0) as usize;
            }
            let data = map.data_mut();
            let n = len.min(data.len());
            xfer.xfer_user(&mut data[..n])?;
        }
        Ok(version >= 3)
    }

    /// [`World::xfer_terrain_visual_part`] as the World's own block. A version 3 block (an original game's save) has its
    /// trees and props left, which the World cannot read past (it does not know where the block ends), so its load stops
    /// there with an error: such a save is loaded through [`crate::client::save_game`], which reads past them.
    fn xfer_terrain_visual(&mut self, xfer: &mut dyn Xfer) -> Result<(), crate::common::game_state::SaveError> {
        if self.xfer_terrain_visual_part(xfer)? {
            return Err(crate::common::game_state::SaveError::Undefined(crate::common::ini::IniError::undefined(crate::common::ini::Undefined::SaveBlockPartNotTranslated)));
        }
        Ok(())
    }

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
