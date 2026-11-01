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
//!   dispatcher runs them, `dispatch_host_world.rs`), `startNewGame` (I7c), the script engine (I8), the AI and its groups (I9), the
//!   build assistant (I6), the victory conditions (I5), the recorder, `TheGameInfo`, the network, the stats collector,
//!   the game state's deep CRC, and the client (display, tactical view, in-game UI, mouse, audio, load screen).
//!
//! # Deviation until batch I9
//!
//! `xferSnapshot(TheAI)` in a CRC: the world has no AI yet, so the AI adds nothing to the CRC. A CRC the world takes is
//! therefore not upstream's for the same game; it is reproducible, and it is what the native-versus-wasm gate compares.
//!
//! `p1w13-sonnet`, integration batch I4, 2026-10-31.

use crate::common::ini::IniError;
use crate::common::message_stream::MessageType;
use crate::common::xfer::{Snapshot, Xfer, XferResult};
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_dispatch::DispatchState;
use crate::logic::world::game_logic_update::{game_logic_update, get_crc, GameLogicHost};
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
    /// batch I7b: the messages of `TheCommandList` (and of the stream, unused here).
    pub messages: crate::common::message_stream::Messages,
    /// batch I7b: `TheCommandList`, first to last.
    pub commands: Vec<crate::common::message_stream::MessageId>,
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
    /// `TheRecorder != NULL`, `isMultiplayer()`, `getMode() == RECORDERMODETYPE_PLAYBACK`.
    pub has_recorder: bool,
    /// See `has_recorder`.
    pub recorder_multiplayer: bool,
    /// See `has_recorder`.
    pub recorder_playback: bool,
    /// `TheGameInfo->getCRCInterval()`, `None` when there is no `TheGameInfo`.
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
            commands: Vec::new(),
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
        let r = game_logic_update(&mut state, &mut dispatch, self);
        self.game_logic = state;
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
    fn u_start_new_game(&mut self, loading_save_game: bool) {
        self.loop_cut("GameLogic::startNewGame", &[i64::from(loading_save_game)]);
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
        let gl = &self.cuts.game_loop;
        gl.commands.iter().any(|id| gl.messages.get(*id).is_some_and(|m| m.get_type() == t))
    }
    fn u_command_list(&self) -> Vec<u32> {
        (0..self.cuts.game_loop.commands.len() as u32).collect()
    }
    fn u_dispatch(&mut self, state: &mut DispatchState, msg: u32) -> R<()> {
        // batch I7b: the world's own dispatcher
        let gl = &self.cuts.game_loop;
        let Some(m) = gl.commands.get(msg as usize).and_then(|id| gl.messages.get(*id)).cloned() else { return Ok(()) };
        crate::logic::world::game_logic_dispatch::logic_message_dispatcher(state, &m, self)
    }
    fn u_command_list_reset(&mut self) {
        let gl = &mut self.cuts.game_loop;
        for id in std::mem::take(&mut gl.commands) {
            let _ = gl.messages.delete_message(id);
        }
    }
    fn u_send_logic_crc(&mut self, crc: u32, playback: bool) {
        self.loop_cut("MessageStream::appendMessage(MSG_LOGIC_CRC)", &[i64::from(crc), i64::from(playback)]);
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
        self.loop_cut("ScriptEngine::update", &[]);
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
        self.cuts.game_loop.script_time_frozen_script
    }
    fn u_script_force_unfreeze_time(&mut self) {
        self.loop_cut("ScriptEngine::forceUnfreezeTime", &[]);
        self.cuts.game_loop.script_time_frozen_script = false;
    }
    fn u_terrain_update(&mut self) {
        if let Err(e) = self.terrain_update() {
            self.loop_cut("TerrainLogic::update (undefined)", &[]);
            self.cuts.team_live_errors.push(format!("TerrainLogic::update: {e:?}"));
        }
    }
    fn u_has_recorder(&self) -> bool {
        self.cuts.game_loop.has_recorder
    }
    fn u_recorder_is_multiplayer(&self) -> bool {
        self.cuts.game_loop.recorder_multiplayer
    }
    fn u_recorder_playback(&self) -> bool {
        self.cuts.game_loop.recorder_playback
    }
    fn u_recorder_update(&mut self) {
        self.loop_cut("Recorder::update", &[]);
    }
    fn u_game_info_crc_interval(&self) -> Option<i32> {
        self.cuts.game_loop.game_info_crc_interval
    }
    fn u_replay_crc_interval(&self) -> i32 {
        self.cuts.game_loop.replay_crc_interval
    }
    fn u_stats_collector_update(&mut self) -> bool {
        if self.cuts.game_loop.has_stats_collector {
            self.loop_cut("StatsCollector::update", &[]);
        }
        self.cuts.game_loop.has_stats_collector
    }
    fn u_ai_update(&mut self) {
        self.loop_cut("AI::update", &[]);
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
    fn u_crc_ai(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        // the world has no AI until batch I9 (see the module docs)
        self.loop_cut("AI::crc", &[]);
        Ok(())
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
        self.cuts.game_loop.groups_made += 1;
        let g = self.cuts.game_loop.groups_made;
        self.loop_cut("AI::createGroup", &[i64::from(g)]);
        g
    }
    fn u_destroy_group(&mut self, group: u32) {
        self.loop_cut("AI::destroyGroup", &[i64::from(group)]);
        self.cuts.player_host.groups.remove(&group);
    }
    fn u_group_add(&mut self, group: u32, id: u32) {
        self.loop_cut("AIGroup::add", &[i64::from(group), i64::from(id)]);
        self.cuts.player_host.groups.entry(group).or_default().push(id as ObjectId);
    }
    fn u_group_remove(&mut self, group: u32, id: u32) -> bool {
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

    /// A solo game's frame 0 (a multiple of the replay CRC interval): the client frame, the script engine, a CRC sent,
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
                "ScriptEngine::update",
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
        assert_eq!((w.frame(), hosts(&w)), (0, vec!["GameClient::setFrame", "ScriptEngine::update"]));
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
