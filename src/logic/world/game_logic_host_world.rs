// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp (the calls `GameLogic::update`, `getCRC` and
//   `processCommandList` make out of `GameLogic`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I4: the world as `GameLogicHost`, so `GameLogic::update` ([`game_logic_update`]) and
//! `GameLogic::getCRC` ([`get_crc`]) run on the world. The world now holds `GameLogic`'s own members
//! ([`World::game_logic`], a [`GameLogicState`]) and the dispatcher's ([`World::dispatch`], a [`DispatchState`]);
//! [`World::logic_update`] and [`World::logic_crc`] are the entry points.
//!
//! # What answers what
//!
//! - **The world:** the frame, `m_startNewGame`, `m_isInUpdate`, the object list, the objects' disabled, selectable and
//!   drawable state, `checkDisabledStatus`, the sleepy module loop, the destroy list, the terrain's update (batch I2),
//!   the partition manager's update (batch I1), the weapon store's update (its delayed damage), the selection of the
//!   player list (`getEachPlayerFromMask`, `setCurrentlySelectedAIGroup`, ...), and the CRC: every object's `crc`, the
//!   random seeds' CRC, the partition manager's and the player list's snapshots.
//! - **Cut points** ([`CutCall::Host`] with upstream's name; answers in [`GameLoopCuts`]): the units the world does not
//!   hold yet. ~~The command list and its dispatcher (batch I7: the commands are stand-ins, each a type, a player and a
//!   CRC, and dispatching one is logged)~~ (**corrected in I7b**: the command list is real messages and the world's
//!   dispatcher runs them, `dispatch_host_world.rs`), ~~`startNewGame` (I7c), the script engine (I8)~~ (the world's
//!   since batches I7c and I8a), the AI and its groups (I9), the
//!   build assistant (I6), the victory conditions (I5), ~~the recorder, `TheGameInfo`~~ (**corrected 2026-11-04,
//!   batch I55**: the recorder is live once `World::recorder_init` makes it, `recorder_world.rs`, and `TheGameInfo`'s CRC
//!   interval is the game `startNewGame` took), the network, the stats collector,
//!   the game state's deep CRC, and the client (display, tactical view, in-game UI, mouse, audio, load screen).
//!
//! # Deviation until batch I9
//!
//! `xferSnapshot(TheAI)` in a CRC: ~~the world has no AI yet, so the AI adds nothing to the CRC~~ **since batch I9c-1, with
//! `cuts.live_ai`**, the pathfinder's CRC and the AI data's; the AI groups are not live yet, so they add nothing (without
//! `live_ai` the AI adds nothing, as before). A CRC the world takes is therefore not upstream's for the same game; it is
//! reproducible, and it is what the native-versus-wasm gate compares.
//!
//! `p1w13-sonnet`, integration batch I4, 2026-10-31.

use crate::common::ini::IniError;
use crate::common::message_stream::MessageType;
use crate::common::xfer::{Snapshot, Xfer, XferResult};
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_dispatch::DispatchState;
use crate::logic::world::game_logic_update::{game_logic_update, get_crc, GameLogicHost, GamePauseHost};
use crate::logic::world::ids::ObjectId;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::player_list::{player_list_crc, player_list_get_each_player_from_mask};
use crate::logic::world::player_squads::{
    player_add_ai_group_to_current_selection, player_get_current_selection_as_ai_group, player_set_currently_selected_ai_group,
};

type R<T> = Result<T, IniError>;

/// The answers of the units `GameLogic` reaches that the world does not hold yet. See the module docs.
#[derive(Clone, Debug)]
pub struct GameLoopCuts {
    /// `m_gameMode` (`GAME_NONE` until a game starts, as `GameLogic::GameLogic()` sets it).
    pub game_mode: i32,
    /// batch I7b: the messages of `TheMessageStream` and `TheCommandList`. ~~`TheCommandList` was a list of ids of its
    /// own (`commands`), and the stream was unused.~~ **Corrected 2026-11-04 (batch I55):** `TheCommandList` is the
    /// arena's [`Messages::COMMAND_LIST`](crate::common::message_stream::Messages::COMMAND_LIST), which the recorder reads
    /// and writes, and the logic CRC goes to its [`Messages::STREAM`](crate::common::message_stream::Messages::STREAM)
    /// ([`World::engine_update`] moves the stream to the command list).
    pub messages: crate::common::message_stream::Messages,
    /// batch I55 (#14): `TheRecorder`, once [`World::recorder_init`] makes it; `None` keeps the stand-ins below.
    pub recorder: Option<crate::common::recorder::Recorder>,
    /// `TheDisplay->isMoviePlaying()`.
    pub movie_playing: bool,
    /// `TheTacticalView->isTimeFrozen()`, `isCameraMovementFinished()`.
    pub view_time_frozen: bool,
    /// See `view_time_frozen`.
    pub view_camera_movement_finished: bool,
    /// `TheScriptEngine->isTimeFrozenDebug()`, `isTimeFrozenScript()`.
    pub script_time_frozen_debug: bool,
    /// See `script_time_frozen_debug`.
    pub script_time_frozen_script: bool,
    /// `TheRecorder != NULL`, `isMultiplayer()`, `getMode() == RECORDERMODETYPE_PLAYBACK`: the stand-in's answers,
    /// read only while there is no live `recorder` (batch I55).
    pub has_recorder: bool,
    /// See `has_recorder`.
    pub recorder_multiplayer: bool,
    /// See `has_recorder`.
    pub recorder_playback: bool,
    /// `TheGameInfo->getCRCInterval()`, `None` when there is no `TheGameInfo`: read only when `startNewGame` took no
    /// game (batch I55; the taken game's interval otherwise).
    pub game_info_crc_interval: Option<i32>,
    /// batch I5: `TheGameInfo`'s slots, whether each is an AI's (`None`: no `TheGameInfo`).
    pub game_info_slots_ai: Option<Vec<bool>>,
    /// `REPLAY_CRC_INTERVAL` (100 until the recorder or the command line sets it, `Recorder.cpp`).
    pub replay_crc_interval: i32,
    /// `TheStatsCollector != NULL`.
    pub has_stats_collector: bool,
    /// `TheNetwork != NULL`, `sawCRCMismatch()`, the connected slots, `getNumPlayers()`.
    pub has_network: bool,
    /// See `has_network`.
    pub network_saw_crc_mismatch: bool,
    /// See `has_network`.
    pub network_connected: [bool; 8],
    /// See `has_network`.
    pub network_num_players: i32,
    /// `timeGetTime()`.
    pub time: i32,
    /// `m_loadScreen != NULL`.
    pub has_load_screen: bool,
    /// `TheInGameUI->getInputEnabled()`, `TheMouse->getVisibility()`.
    pub ui_input_enabled: bool,
    /// See `ui_input_enabled`.
    pub mouse_visible: bool,
    /// The AI groups made (`TheAI->createGroup()` gives out the next number).
    pub groups_made: u32,
}

impl Default for GameLoopCuts {
    fn default() -> Self {
        GameLoopCuts {
            game_mode: crate::common::recorder::game_mode::NONE,
            messages: crate::common::message_stream::Messages::new(),
            recorder: None,
            movie_playing: false,
            view_time_frozen: false,
            view_camera_movement_finished: true,
            script_time_frozen_debug: false,
            script_time_frozen_script: false,
            has_recorder: true,
            recorder_multiplayer: false,
            recorder_playback: false,
            game_info_crc_interval: None,
            game_info_slots_ai: None,
            replay_crc_interval: 100,
            has_stats_collector: false,
            has_network: false,
            network_saw_crc_mismatch: false,
            network_connected: [false; 8],
            network_num_players: 0,
            time: 0,
            has_load_screen: false,
            ui_input_enabled: true,
            mouse_visible: true,
            groups_made: 0,
        }
    }
}

impl World {
    /// `GameLogic::update()`: one logic frame (see [`game_logic_update`]).
    pub fn logic_update(&mut self) -> R<()> {
        let mut state = std::mem::take(&mut self.game_logic);
        let mut dispatch = std::mem::take(&mut self.dispatch);
        self.in_logic_frame = true;
        let r = game_logic_update(&mut state, &mut dispatch, self);
        // batch I7c: the rank points a dispatched `prepareNewGame` set while the frame held `GameLogic`'s members
        if let Some(points) = self.cuts.pending_rank_points.take() {
            state.rank_points_to_add_at_game_start = points;
        }
        self.game_logic = state;
        self.in_logic_frame = false;
        // (batch I20: the members a script set during the frame)
        for wr in std::mem::take(&mut self.cuts.pending_game_logic_writes) {
            self.apply_game_logic_write(wr);
        }
        self.dispatch = dispatch;
        r
    }

    /// `GameLogic::getCRC(mode, deepCRCFileName)`: the CRC of the game (`CRC_CACHED` answers the last one taken,
    /// `CRC_RECALC` takes it); `deep` is the deep CRC's bytes when wanted.
    pub fn logic_crc(&mut self, mode: i32, deep: Option<&mut Vec<u8>>) -> XferResult<u32> {
        let state = std::mem::take(&mut self.game_logic);
        let r = get_crc(&state, mode, deep, self);
        self.game_logic = state;
        r
    }

    /// Logs a call of `GameLogic` into a unit the world does not hold.
    fn loop_cut(&mut self, call: &'static str, args: &[i64]) {
        self.cut(CutCall::Host { call, args: args.to_vec(), text: Vec::new() });
    }
}

impl GameLogicHost for World {
    fn u_frame(&self) -> u32 {
        self.frame()
    }
    fn u_advance_frame(&mut self) {
        let f = self.frame().wrapping_add(1);
        self.set_frame(f);
    }
    fn u_start_new_game_pending(&self) -> bool {
        self.start_new_game_pending()
    }
    fn u_set_start_new_game_pending(&mut self, on: bool) {
        self.set_start_new_game(on);
    }
    fn u_is_in_update(&self) -> bool {
        self.is_in_game_logic_update()
    }
    fn u_set_in_update(&mut self, on: bool) {
        self.set_in_game_logic_update(on);
    }
    fn u_game_mode(&self) -> i32 {
        self.cuts.game_loop.game_mode
    }
    fn u_start_new_game(&mut self, state: &mut crate::logic::world::game_logic_update::GameLogicState, loading_save_game: bool) {
        // batch I7c: the world's own new game, on the frame's `GameLogic` members
        if let Err(e) = crate::logic::world::game_logic_new_game::start_new_game(state, loading_save_game, self) {
            self.cuts.team_live_errors.push(format!("GameLogic::startNewGame: {e:?}"));
        }
    }
    fn u_objects(&self) -> Vec<u32> {
        self.objects.ids_in_order().into_iter().map(|o| o as u32).collect()
    }
    fn u_object_is_disabled(&self, id: u32) -> bool {
        self.find_object(id as ObjectId).is_some_and(|o| o.is_disabled())
    }
    fn u_check_disabled_status(&mut self, id: u32) {
        self.check_disabled_status(id as ObjectId);
    }
    fn u_object_is_mass_selectable(&self, id: u32) -> bool {
        self.is_mass_selectable(id as ObjectId)
    }
    fn u_object_has_drawable(&self, id: u32) -> bool {
        self.has_drawable(id as ObjectId)
    }
    fn u_run_sleepy_updates(&mut self) {
        self.run_sleepy_updates();
    }
    fn u_process_destroy_list(&mut self) {
        self.process_destroy_list();
    }
    fn u_command_list_contains(&self, t: MessageType) -> bool {
        use crate::common::message_stream::Messages;
        self.cuts.game_loop.messages.contains_message_of_type(Messages::COMMAND_LIST, t).unwrap_or(false)
    }
    fn u_command_list(&self) -> Vec<u32> {
        use crate::common::message_stream::Messages;
        let n = self.cuts.game_loop.messages.list_messages(Messages::COMMAND_LIST).map_or(0, |l| l.len());
        (0..n as u32).collect()
    }
    fn u_dispatch(&mut self, state: &mut DispatchState, msg: u32) -> R<()> {
        // batch I7b: the world's own dispatcher (batch I55: the `msg`th message of `TheCommandList`)
        use crate::common::message_stream::Messages;
        let gl = &self.cuts.game_loop;
        let id = gl.messages.list_messages(Messages::COMMAND_LIST)?.get(msg as usize).copied();
        let Some(m) = id.and_then(|id| gl.messages.get(id)).cloned() else { return Ok(()) };
        crate::logic::world::game_logic_dispatch::logic_message_dispatcher(state, &m, self)
    }
    fn u_command_list_reset(&mut self) {
        use crate::common::message_stream::Messages;
        let _ = self.cuts.game_loop.messages.destroy_all_messages(Messages::COMMAND_LIST);
    }
    fn u_send_logic_crc(&mut self, crc: u32, playback: bool) {
        // batch I55: appended to `TheMessageStream` (GameLogic.cpp:3666), as upstream does; the call is still logged
        self.loop_cut("MessageStream::appendMessage(MSG_LOGIC_CRC)", &[i64::from(crc), i64::from(playback)]);
        use crate::common::message_stream::Messages;
        let local = self.players.local_index().map_or(-1, |p| p);
        let gl = &mut self.cuts.game_loop;
        if let Ok(id) = gl.messages.append_new_message(Messages::STREAM, crate::common::message_types::MSG_LOGIC_CRC, local) {
            if let Some(m) = gl.messages.get_mut(id) {
                m.append_integer_argument(crc as i32);
                m.append_boolean_argument(playback);
            }
        }
    }
    fn u_set_fp_mode(&mut self) {
        // `setFPMode()`: the x87 control word; Rust's floating point is already IEEE single/double as written
    }
    fn u_movie_playing(&self) -> bool {
        self.cuts.game_loop.movie_playing
    }
    fn u_client_set_frame(&mut self, frame: u32) {
        self.loop_cut("GameClient::setFrame", &[i64::from(frame)]);
    }
    fn u_script_engine_update(&mut self) {
        // batch I8a: the world's own script engine
        if let Err(e) = self.script_engine_update() {
            self.cuts.team_live_errors.push(format!("ScriptEngine::update: {e:?}"));
        }
    }
    fn u_view_time_frozen(&self) -> bool {
        self.cuts.game_loop.view_time_frozen
    }
    fn u_view_camera_movement_finished(&self) -> bool {
        self.cuts.game_loop.view_camera_movement_finished
    }
    fn u_script_time_frozen_debug(&self) -> bool {
        self.cuts.game_loop.script_time_frozen_debug
    }
    fn u_script_time_frozen_script(&self) -> bool {
        // batch I8a: the engine's (the table still freezes time for the tests that set it)
        self.script_engine.is_time_frozen_script() || self.cuts.game_loop.script_time_frozen_script
    }
    fn u_script_force_unfreeze_time(&mut self) {
        // `forceUnfreezeTime()`: `m_freezeByScript = FALSE`
        self.script_engine.do_unfreeze_time();
        self.cuts.game_loop.script_time_frozen_script = false;
    }
    fn u_terrain_update(&mut self) {
        if let Err(e) = self.terrain_update() {
            self.loop_cut("TerrainLogic::update (undefined)", &[]);
            self.cuts.team_live_errors.push(format!("TerrainLogic::update: {e:?}"));
        }
    }
    fn u_has_recorder(&self) -> bool {
        // (batch I55: the live recorder, once made; the stand-in's flag otherwise)
        self.cuts.game_loop.recorder.is_some() || self.cuts.game_loop.has_recorder
    }
    fn u_recorder_is_multiplayer(&self) -> bool {
        self.recorder_is_multiplayer().unwrap_or(self.cuts.game_loop.recorder_multiplayer)
    }
    fn u_recorder_playback(&self) -> bool {
        match &self.cuts.game_loop.recorder {
            Some(r) => r.mode == crate::common::recorder::RecorderMode::Playback,
            None => self.cuts.game_loop.recorder_playback,
        }
    }
    fn u_recorder_update(&mut self) {
        // (batch I55: the live recorder's `update()`; a logged call without one)
        match self.with_recorder(|r, w| r.update(w)) {
            Some(Ok(())) => {}
            Some(Err(e)) => self.cuts.team_live_errors.push(format!("Recorder::update: {e:?}")),
            None => self.loop_cut("Recorder::update", &[]),
        }
    }
    fn u_game_info_crc_interval(&self) -> Option<i32> {
        // (batch I55: `TheGameInfo->getCRCInterval()`, of the game `startNewGame` took: the replay's, the recorder's
        // `getGameInfo()`, or the skirmish's; the table when none was taken)
        use crate::logic::world::game_logic_new_game::GameSource;
        match self.cuts.new_game.selected {
            Some(GameSource::Recorder) => {
                if let Some(r) = &self.cuts.game_loop.recorder {
                    return Some(r.game_info.crc_interval);
                }
            }
            Some(s) => {
                if let Some((_, g)) = self.cuts.new_game.games.iter().find(|(x, _)| *x == s) {
                    return Some(g.crc_interval);
                }
            }
            None => {}
        }
        self.cuts.game_loop.game_info_crc_interval
    }
    fn u_replay_crc_interval(&self) -> i32 {
        // (batch I55: `REPLAY_CRC_INTERVAL`, which the live recorder's `playbackFile` sets)
        self.cuts.game_loop.recorder.as_ref().map_or(self.cuts.game_loop.replay_crc_interval, |r| r.replay_crc_interval)
    }
    fn u_stats_collector_update(&mut self) -> bool {
        if self.cuts.game_loop.has_stats_collector {
            self.loop_cut("StatsCollector::update", &[]);
        }
        self.cuts.game_loop.has_stats_collector
    }
    fn u_ai_update(&mut self) {
        // batch I9c-1: the world's own AI update with `cuts.live_ai`
        if self.cuts.live_ai {
            self.ai_update();
        } else {
            self.loop_cut("AI::update", &[]);
        }
    }
    fn u_build_assistant_update(&mut self) {
        // batch I6: the world's own build assistant
        self.build_assistant_update();
    }
    fn u_partition_update(&mut self) {
        if self.cuts.live_partition {
            self.partition_update();
        } else {
            self.loop_cut("PartitionManager::update", &[]);
        }
    }
    fn u_weapon_store_update(&mut self) {
        if let Err(e) = crate::logic::weapon_fire::weapon_store_update(self) {
            self.cuts.team_live_errors.push(format!("WeaponStore::update: {e:?}"));
        }
    }
    fn u_locomotor_store_update(&mut self) {
        // `LocomotorStore::update()` is empty upstream
    }
    fn u_victory_conditions_update(&mut self) {
        // batch I5: the world's own conditions
        if let Err(e) = self.victory_update() {
            self.cuts.team_live_errors.push(format!("VictoryConditions::update: {e:?}"));
        }
    }
    fn u_crc_object(&mut self, id: u32, xfer: &mut dyn Xfer) -> XferResult<()> {
        let statics = self.statics.clone();
        self.object_crc(id as ObjectId, xfer, &statics.weapon_store)
    }
    fn u_random_seed_crc(&self) -> u32 {
        self.random.get_game_logic_random_seed_crc()
    }
    fn u_crc_partition_manager(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut pm = std::mem::take(&mut self.partition);
        let r = pm.crc(xfer);
        self.partition = pm;
        r
    }
    fn u_crc_player_list(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        // (a player error in a CRC is upstream's undefined behaviour: recorded; the xfer's own status is passed on)
        match player_list_crc(self, xfer) {
            Ok(()) => Ok(()),
            Err(crate::logic::world::player_ops::PlayerError::Xfer(s)) => Err(s),
            Err(e) => {
                self.cuts.team_live_errors.push(format!("PlayerList::crc: {e:?}"));
                Ok(())
            }
        }
    }
    fn u_crc_ai(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        // batch I9c-1: with `live_ai`, `xferSnapshot(TheAI)`: in a CRC `AI::crc` (the pathfinder's, then each AI data entry's
        // after its marker, then each group's; the World has one AI data entry and no live groups yet), in a save `AI::xfer`
        // (its version). Without it, the cut point (see the module docs).
        if !self.cuts.live_ai {
            self.loop_cut("AI::crc", &[]);
            return Ok(());
        }
        if xfer.get_xfer_mode() != crate::common::xfer::XferMode::Crc {
            let current: crate::common::xfer::XferVersion = 1;
            let mut version = current;
            return xfer.xfer_version(&mut version, current);
        }
        if let Some(pf) = self.pathfinder.as_mut() {
            xfer.xfer_snapshot(pf)?;
        }
        let mut marker = b"MARKER:TAiData".to_vec();
        xfer.xfer_ascii_string(&mut marker)?;
        let mut data = self.statics.ai_data.data.clone();
        data.crc(xfer)?;
        // (batch I9c-2: the groups, in the list's order)
        self.ai_groups_crc(xfer)
    }
    fn u_crc_game_state(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        self.loop_cut("GameState::friend_xferSaveDataForCRC", &[]);
        Ok(())
    }
    fn u_has_network(&self) -> bool {
        self.cuts.game_loop.has_network
    }
    fn u_network_saw_crc_mismatch(&self) -> bool {
        self.cuts.game_loop.network_saw_crc_mismatch
    }
    fn u_network_set_saw_crc_mismatch(&mut self) {
        self.loop_cut("Network::setSawCRCMismatch", &[]);
        self.cuts.game_loop.network_saw_crc_mismatch = true;
    }
    fn u_network_player_connected(&self, slot: i32) -> bool {
        usize::try_from(slot).ok().and_then(|s| self.cuts.game_loop.network_connected.get(s)).copied().unwrap_or(false)
    }
    fn u_network_num_players(&self) -> i32 {
        self.cuts.game_loop.network_num_players
    }
    fn u_time_get_time(&mut self) -> i32 {
        self.cuts.game_loop.time
    }
    fn u_load_screen_process_progress(&mut self, player: i32, percentage: i32) -> bool {
        if self.cuts.game_loop.has_load_screen {
            self.loop_cut("LoadScreen::processProgress", &[i64::from(player), i64::from(percentage)]);
        }
        self.cuts.game_loop.has_load_screen
    }
    fn u_each_player_from_mask(&self, mask: &mut u16) -> Option<i32> {
        player_list_get_each_player_from_mask(self, mask)
    }
    fn u_create_group(&mut self) -> u32 {
        // batch I9c-2: with `live_ai`, `TheAI`'s own groups
        if self.cuts.live_ai {
            return self.ai_create_group();
        }
        self.cuts.game_loop.groups_made += 1;
        let g = self.cuts.game_loop.groups_made;
        self.loop_cut("AI::createGroup", &[i64::from(g)]);
        g
    }
    fn u_destroy_group(&mut self, group: u32) {
        if self.cuts.live_ai {
            return self.ai_destroy_group_live(group);
        }
        self.loop_cut("AI::destroyGroup", &[i64::from(group)]);
        self.cuts.player_host.groups.remove(&group);
    }
    fn u_group_add(&mut self, group: u32, id: u32) {
        if self.cuts.live_ai {
            self.with_group(group, |g, w| g.add(w, id as ObjectId));
            return;
        }
        self.loop_cut("AIGroup::add", &[i64::from(group), i64::from(id)]);
        self.cuts.player_host.groups.entry(group).or_default().push(id as ObjectId);
    }
    fn u_group_remove(&mut self, group: u32, id: u32) -> bool {
        if self.cuts.live_ai {
            return self.with_group(group, |g, w| g.remove(w, id as ObjectId)).unwrap_or(false);
        }
        self.loop_cut("AIGroup::remove", &[i64::from(group), i64::from(id)]);
        let members = self.cuts.player_host.groups.entry(group).or_default();
        members.retain(|o| *o != id as ObjectId);
        // `AIGroup::remove`: a group left empty deletes itself
        let gone = members.is_empty();
        if gone {
            self.cuts.player_host.groups.remove(&group);
        }
        gone
    }
    fn u_player_set_selected_group(&mut self, player: i32, group: Option<u32>) {
        let _ = player_set_currently_selected_ai_group(self, player, group);
    }
    fn u_player_add_group_to_selection(&mut self, player: i32, group: u32) {
        let _ = player_add_ai_group_to_current_selection(self, player, Some(group));
    }
    fn u_player_selection_as_group(&mut self, player: i32, group: u32) {
        let _ = player_get_current_selection_as_ai_group(self, player, Some(group));
    }
    fn u_ui_select_drawable(&mut self, id: u32) {
        self.loop_cut("InGameUI::selectDrawable", &[i64::from(id)]);
    }
    fn u_ui_deselect_drawable(&mut self, id: u32) {
        self.loop_cut("InGameUI::deselectDrawable", &[i64::from(id)]);
    }
}

impl GamePauseHost for World {
    fn u_ui_input_enabled(&self) -> bool {
        self.cuts.game_loop.ui_input_enabled
    }
    fn u_ui_set_input_enabled(&mut self, on: bool) {
        self.loop_cut("InGameUI::setInputEnabled", &[i64::from(on)]);
        self.cuts.game_loop.ui_input_enabled = on;
    }
    fn u_mouse_visible(&self) -> bool {
        self.cuts.game_loop.mouse_visible
    }
    fn u_mouse_set_visible(&mut self, on: bool) {
        self.loop_cut("Mouse::setVisibility", &[i64::from(on)]);
        self.cuts.game_loop.mouse_visible = on;
    }
    fn u_mouse_set_arrow_cursor(&mut self) {
        self.loop_cut("Mouse::setCursor(ARROW)", &[]);
    }
    fn u_audio_pause(&mut self, affect: i32) {
        self.loop_cut("AudioManager::pauseAudio", &[i64::from(affect)]);
    }
    fn u_audio_resume(&mut self, affect: i32) {
        self.loop_cut("AudioManager::resumeAudio", &[i64::from(affect)]);
    }
}

#[cfg(test)]
mod tests {
    //! `GameLogic::update`'s order and the CRC's coverage are checked against the compiled GameLogic.cpp by
    //! `tests/differential_game_logic_frame.rs` with a recording host; these check that on the world the frame runs the
    //! world's own parts, stops when time is frozen, and takes a reproducible CRC of the world's objects.
    use super::*;
    use crate::common::base_type::Coord3D;
    use crate::common::recorder::game_mode;
    use crate::logic::world::game_logic_update::{CRC_CACHED, CRC_RECALC};
    use crate::logic::world::player_host_world::tests::{make, world};

    fn hosts(w: &World) -> Vec<&'static str> {
        w.cuts.log.as_ref().unwrap().iter().filter_map(|c| if let CutCall::Host { call, .. } = c { Some(*call) } else { None }).collect()
    }

    /// A solo game's frame 0 (a multiple of the replay CRC interval): the client frame, the script engine (with no map
    /// started it does nothing, batch I8a), a CRC sent,
    /// the recorder, then the AI, the build assistant and the victory conditions, and the frame advances; the CRC is
    /// kept as `m_CRC`.
    #[test]
    fn a_frame_runs_in_upstreams_order_and_advances() {
        let (mut w, _) = world();
        w.cuts.game_loop.game_mode = game_mode::SINGLE_PLAYER;
        w.logic_update().unwrap();
        assert_eq!(w.frame(), 1);
        assert_eq!(
            hosts(&w),
            [
                "GameClient::setFrame",
                "AI::crc",
                "MessageStream::appendMessage(MSG_LOGIC_CRC)",
                "Recorder::update",
                "AI::update",
            ]
        );
        assert_ne!(w.game_logic.crc, 0);
        assert_eq!(w.logic_crc(CRC_CACHED, None).unwrap(), w.game_logic.crc);
        // frame 1 is not a multiple of 100: no CRC
        w.cuts.log = Some(Vec::new());
        w.logic_update().unwrap();
        assert!(!hosts(&w).contains(&"AI::crc") && w.frame() == 2);
    }

    /// Time frozen by a script: the frame stops after the script engine, and the frame number stays.
    #[test]
    fn a_frozen_frame_stops_after_the_script_engine() {
        let (mut w, _) = world();
        w.cuts.game_loop.script_time_frozen_script = true;
        w.logic_update().unwrap();
        let h = hosts(&w);
        assert_eq!(w.frame(), 0);
        assert_eq!(h.first(), Some(&"GameClient::setFrame"));
        // the script engine's update runs (its own calls are logged), and nothing after it
        assert!(!h.contains(&"TerrainLogic::update (undefined)") && !h.contains(&"Recorder::update"), "{h:?}");
        assert!(!w.is_in_game_logic_update());
    }

    /// The CRC is reproducible (two worlds built alike agree, and so does a second take) and covers the objects.
    #[test]
    fn the_crc_is_reproducible_and_covers_the_objects() {
        let (mut a, team) = world();
        let (mut b, _) = world();
        let crc = a.logic_crc(CRC_RECALC, None).unwrap();
        assert_eq!(b.logic_crc(CRC_RECALC, None).unwrap(), crc);
        assert_eq!(a.logic_crc(CRC_RECALC, None).unwrap(), crc);
        make(&mut a, b"Tank", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        assert_ne!(a.logic_crc(CRC_RECALC, None).unwrap(), crc);
        make(&mut b, b"Tank", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        assert_eq!(a.logic_crc(CRC_RECALC, None).unwrap(), b.logic_crc(CRC_RECALC, None).unwrap());
    }
}
