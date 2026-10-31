// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   (`update`, `processCommandList`, `getCRC`, `isIntroMoviePlaying`, `selectObject`, `deselectObject`,
//   `isGamePaused`, `setGamePaused`, `processProgress`, `processProgressComplete`, `isProgressComplete`,
//   `lastHeardFrom`, `testTimeOut`, `timeOutGameStart`, `initTimeOutValues`, `getObjectCount`, and the buildable
//   and control-bar overrides)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameLogic`'s frame: the order of a logic frame's work, the CRC it keeps, the command list's CRC check, pausing,
//! the network game's load progress, and the overrides scripts set (layer 12 batch L1).
//!
//! # Shape
//!
//! [`game_logic_update`] is `GameLogic::update`: which subsystem runs when, the time freeze, when a CRC is taken
//! and sent, the end of the frame. Each subsystem is a call on [`GameLogicHost`] (the world's own parts, already
//! translated, are calls too: the sleepy module loop is [`crate::logic::world::World::run_sleepy_updates`], the
//! destroy list `process_destroy_list`); the command list is dispatched one message at a time through
//! [`GameLogicHost::u_dispatch`], which is handed the dispatcher's state. The members of `GameLogic` the functions
//! here own are [`GameLogicState`]; the frame number, `m_startNewGame` and `m_isInUpdate` are the world's.
//!
//! # Release build
//!
//! `DO_UNIT_TIMINGS`, `DUMP_PERF_STATS`, `_PROFILE`, `ALLOW_NONSLEEPY_UPDATES` and `DO_COPY_PROTECTION` are not
//! defined in the release build, and `validateSleepyUpdate` is empty outside `_DEBUG`. `inCRCGen` (the latch
//! `getCRC` sets) is read only by the debug build's CRC log.
//!
//! # Upstream facts kept
//!
//! - A frame frozen by the camera or a script returns before the terrain, the CRC and the commands, and the frame
//!   number does not advance, unless the command list holds `MSG_CLEAR_GAME_DATA` (then time is unfrozen and the
//!   frame runs).
//! - A multiplayer game (or its replay) takes a CRC when the frame is a multiple of `TheGameInfo`'s CRC interval
//!   (`TheGameInfo` dereferenced; an interval of 0 divides by zero); a solo one when it is a multiple of
//!   `REPLAY_CRC_INTERVAL`. Both send `MSG_LOGIC_CRC` with the CRC and whether a replay is playing.
//! - `TheRecorder->update()` is called whether or not there is a recorder (a null dereference when there is none).
//! - `processCommandList` checks CRCs only when a local CRC was seen and the network has not seen a mismatch
//!   (`TheNetwork` dereferenced before its null check): fewer CRCs than connected players is a mismatch, else each
//!   is compared with the lowest player's.
//! - `initTimeOutValues` writes one time-out per network player, unchecked against the eight slots.
//! - `setControlBarOverride` builds its key as the slot's digit (`'0' + slot`, a `char`) and the command set's
//!   name in a 256-byte buffer (a name of 255 bytes or more overflows it); a slot of -48 makes a NUL first byte, so
//!   the key is empty.

use crate::common::ini::{IniError, IniErrorKind, Undefined};
use crate::common::message_stream::MessageType;
use crate::common::message_types::MSG_CLEAR_GAME_DATA;
use crate::common::recorder::game_mode;
use crate::common::xfer::{Xfer, XferMode, XferResult};
use crate::common::xfer_crc::{XferCrc, XferDeepCrc};
use crate::logic::modules::plant;
use crate::logic::world::game_logic_dispatch::DispatchState;
use std::collections::BTreeMap;

type R<T> = Result<T, IniError>;

/// `MAX_SLOTS`.
pub const MAX_SLOTS: usize = 8;
/// `GameLogic::PROGRESS_COMPLETE_TIMEOUT` (milliseconds).
pub const PROGRESS_COMPLETE_TIMEOUT: i32 = 60_000;
/// `CRC_CACHED`: `getCRC` answers the last CRC taken.
pub const CRC_CACHED: i32 = 0;
/// `CRC_RECALC`: `getCRC` takes a new one.
pub const CRC_RECALC: i32 = 1;
/// `AudioAffect_Music`.
pub const AUDIO_AFFECT_MUSIC: i32 = 0x01;
/// `AudioAffect_All`.
pub const AUDIO_AFFECT_ALL: i32 = 0x0F;

fn undefined(u: Undefined) -> IniError {
    IniError::undefined(u)
}

/// The members of `GameLogic` this file owns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameLogicState {
    /// `m_CRC`: the last CRC taken.
    pub crc: u32,
    /// `m_gamePaused`.
    pub game_paused: bool,
    /// `m_inputEnabledMemory`.
    pub input_enabled_memory: bool,
    /// `m_mouseVisibleMemory`.
    pub mouse_visible_memory: bool,
    /// `m_progressComplete`.
    pub progress_complete: [bool; MAX_SLOTS],
    /// `m_progressCompleteTimeout` (each a `timeGetTime()` reading).
    pub progress_complete_timeout: [i32; MAX_SLOTS],
    /// `m_forceGameStartByTimeOut`.
    pub force_game_start_by_timeout: bool,
    /// `m_thingTemplateBuildableOverrides`: by template name.
    pub buildable_overrides: BTreeMap<Vec<u8>, i32>,
    /// `m_controlBarOverrides`: by the slot's digit and the command set's name; `None` is a NULL button.
    pub control_bar_overrides: BTreeMap<Vec<u8>, Option<u32>>,
    // --- saved by `GameLogic::xfer` (batch L2, `game_logic_xfer`) ---
    /// `m_objectTOC`.
    pub object_toc: Vec<crate::logic::world::game_logic_xfer::TocEntry>,
    /// `m_isScoringEnabled`.
    pub scoring_enabled: bool,
    /// `m_showBehindBuildingMarkers`.
    pub show_behind_building_markers: bool,
    /// `m_drawIconUI`.
    pub draw_icon_ui: bool,
    /// `m_showDynamicLOD`.
    pub show_dynamic_lod: bool,
    /// `m_scriptHulkMaxLifetimeOverride` (frames; -1 is off).
    pub script_hulk_max_lifetime_override: i32,
    /// `m_rankPointsToAddAtGameStart`.
    pub rank_points_to_add_at_game_start: i32,
    /// `m_superweaponRestriction`.
    pub superweapon_restriction: u16,
}

/// The world as `GameLogic::update` and its neighbours reach it. Objects are named by id, players by index.
pub trait GameLogicHost {
    // --- the world's own (GameLogic's other members) ---
    /// `m_frame`.
    fn u_frame(&self) -> u32;
    /// `m_frame++`.
    fn u_advance_frame(&mut self);
    /// `m_startNewGame`.
    fn u_start_new_game_pending(&self) -> bool;
    /// `m_startNewGame = on`.
    fn u_set_start_new_game_pending(&mut self, on: bool);
    /// `m_isInUpdate`.
    fn u_is_in_update(&self) -> bool;
    /// `m_isInUpdate = on`.
    fn u_set_in_update(&mut self, on: bool);
    /// `m_gameMode`.
    fn u_game_mode(&self) -> i32;
    /// `startNewGame(loadingSaveGame)`.
    fn u_start_new_game(&mut self, loading_save_game: bool);
    /// The objects in `m_objList`'s order.
    fn u_objects(&self) -> Vec<u32>;
    /// `obj->isDisabled()`.
    fn u_object_is_disabled(&self, id: u32) -> bool;
    /// `obj->checkDisabledStatus()`.
    fn u_check_disabled_status(&mut self, id: u32);
    /// `obj->isMassSelectable()`.
    fn u_object_is_mass_selectable(&self, id: u32) -> bool;
    /// `obj->getDrawable() != NULL`.
    fn u_object_has_drawable(&self, id: u32) -> bool;
    /// The sleepy module loop of `update` (`peekSleepyUpdate` ... `rebalanceSleepyUpdate(0)`).
    fn u_run_sleepy_updates(&mut self);
    /// `processDestroyList()`.
    fn u_process_destroy_list(&mut self);

    // --- the command list ---
    /// `TheCommandList->containsMessageOfType(t)`.
    fn u_command_list_contains(&self, t: MessageType) -> bool;
    /// The command list's messages, first to last (read once, before the first is dispatched: the dispatcher does
    /// not change the command list).
    fn u_command_list(&self) -> Vec<u32>;
    /// `logicMessageDispatcher(msg, NULL)`, with the dispatcher's state.
    fn u_dispatch(&mut self, state: &mut DispatchState, msg: u32) -> R<()>;
    /// `TheCommandList->reset()`.
    fn u_command_list_reset(&mut self);
    /// `TheMessageStream->appendMessage(MSG_LOGIC_CRC)` with the CRC and the playback flag as arguments.
    fn u_send_logic_crc(&mut self, crc: u32, playback: bool);

    // --- the subsystems ---
    /// `setFPMode()`.
    fn u_set_fp_mode(&mut self);
    /// `TheDisplay->isMoviePlaying()`.
    fn u_movie_playing(&self) -> bool;
    /// `TheGameClient->setFrame(frame)`.
    fn u_client_set_frame(&mut self, frame: u32);
    /// `TheScriptEngine->update()`.
    fn u_script_engine_update(&mut self);
    /// `TheTacticalView->isTimeFrozen()`.
    fn u_view_time_frozen(&self) -> bool;
    /// `TheTacticalView->isCameraMovementFinished()`.
    fn u_view_camera_movement_finished(&self) -> bool;
    /// `TheScriptEngine->isTimeFrozenDebug()`.
    fn u_script_time_frozen_debug(&self) -> bool;
    /// `TheScriptEngine->isTimeFrozenScript()`.
    fn u_script_time_frozen_script(&self) -> bool;
    /// `TheScriptEngine->forceUnfreezeTime()`.
    fn u_script_force_unfreeze_time(&mut self);
    /// `TheTerrainLogic->update()`.
    fn u_terrain_update(&mut self);
    /// `TheRecorder != NULL`.
    fn u_has_recorder(&self) -> bool;
    /// `TheRecorder->isMultiplayer()`.
    fn u_recorder_is_multiplayer(&self) -> bool;
    /// `TheRecorder->getMode() == RECORDERMODETYPE_PLAYBACK`.
    fn u_recorder_playback(&self) -> bool;
    /// `TheRecorder->update()`.
    fn u_recorder_update(&mut self);
    /// `TheGameInfo->getCRCInterval()`, `None` when there is no `TheGameInfo`.
    fn u_game_info_crc_interval(&self) -> Option<i32>;
    /// `REPLAY_CRC_INTERVAL` (a global the recorder sets).
    fn u_replay_crc_interval(&self) -> i32;
    /// `TheStatsCollector != NULL`, then its `update()`.
    fn u_stats_collector_update(&mut self) -> bool;
    /// `TheAI->update()`.
    fn u_ai_update(&mut self);
    /// `TheBuildAssistant->update()`.
    fn u_build_assistant_update(&mut self);
    /// `ThePartitionManager->update()`.
    fn u_partition_update(&mut self);
    /// `TheWeaponStore->update()`.
    fn u_weapon_store_update(&mut self);
    /// `TheLocomotorStore->update()`.
    fn u_locomotor_store_update(&mut self);
    /// `TheVictoryConditions->update()`.
    fn u_victory_conditions_update(&mut self);

    // --- the CRC ---
    /// `xfer->xferSnapshot(obj)` (a CRC: the object's `crc`).
    fn u_crc_object(&mut self, id: u32, xfer: &mut dyn Xfer) -> XferResult<()>;
    /// `GetGameLogicRandomSeedCRC()`.
    fn u_random_seed_crc(&self) -> u32;
    /// `xfer->xferSnapshot(ThePartitionManager)`.
    fn u_crc_partition_manager(&mut self, xfer: &mut dyn Xfer) -> XferResult<()>;
    /// `xfer->xferSnapshot(ThePlayerList)`.
    fn u_crc_player_list(&mut self, xfer: &mut dyn Xfer) -> XferResult<()>;
    /// `xfer->xferSnapshot(TheAI)`.
    fn u_crc_ai(&mut self, xfer: &mut dyn Xfer) -> XferResult<()>;
    /// `TheGameState->friend_xferSaveDataForCRC(xfer, SNAPSHOT_DEEPCRC_LOGICONLY)` (a deep CRC only).
    fn u_crc_game_state(&mut self, xfer: &mut dyn Xfer) -> XferResult<()>;

    // --- the network ---
    /// `TheNetwork != NULL`.
    fn u_has_network(&self) -> bool;
    /// `TheNetwork->sawCRCMismatch()`.
    fn u_network_saw_crc_mismatch(&self) -> bool;
    /// `TheNetwork->setSawCRCMismatch()`.
    fn u_network_set_saw_crc_mismatch(&mut self);
    /// `TheNetwork->isPlayerConnected(slot)`.
    fn u_network_player_connected(&self, slot: i32) -> bool;
    /// `TheNetwork->getNumPlayers()`.
    fn u_network_num_players(&self) -> i32;
    /// `timeGetTime()` (as the `Int` it is stored in).
    fn u_time_get_time(&mut self) -> i32;
    /// `m_loadScreen != NULL`, then its `processProgress(player, percentage)`.
    fn u_load_screen_process_progress(&mut self, player: i32, percentage: i32) -> bool;

    // --- selection and the client ---
    /// `ThePlayerList->getEachPlayerFromMask(mask)`: the first player (by index) whose mask meets `mask`, with its
    /// bits taken out of `mask`; none clears `mask`.
    fn u_each_player_from_mask(&self, mask: &mut u16) -> Option<i32>;
    /// `TheAI->createGroup()`.
    fn u_create_group(&mut self) -> u32;
    /// `TheAI->destroyGroup(group)`.
    fn u_destroy_group(&mut self, group: u32);
    /// `group->add(obj)`.
    fn u_group_add(&mut self, group: u32, id: u32);
    /// `group->remove(obj)`: whether the group was deleted (it became empty).
    fn u_group_remove(&mut self, group: u32, id: u32) -> bool;
    /// `player->setCurrentlySelectedAIGroup(group)` (`None` clears).
    fn u_player_set_selected_group(&mut self, player: i32, group: Option<u32>);
    /// `player->addAIGroupToCurrentSelection(group)`.
    fn u_player_add_group_to_selection(&mut self, player: i32, group: u32);
    /// `player->getCurrentSelectionAsAIGroup(group)`.
    fn u_player_selection_as_group(&mut self, player: i32, group: u32);
    /// `TheInGameUI->selectDrawable(obj's drawable)`.
    fn u_ui_select_drawable(&mut self, id: u32);
    /// `TheInGameUI->deselectDrawable(obj's drawable)`.
    fn u_ui_deselect_drawable(&mut self, id: u32);
    /// `TheInGameUI->getInputEnabled()`.
    fn u_ui_input_enabled(&self) -> bool;
    /// `TheInGameUI->setInputEnabled(on)`.
    fn u_ui_set_input_enabled(&mut self, on: bool);
    /// `TheMouse->getVisibility()`.
    fn u_mouse_visible(&self) -> bool;
    /// `TheMouse->setVisibility(on)`.
    fn u_mouse_set_visible(&mut self, on: bool);
    /// `TheMouse->setCursor(Mouse::ARROW)`.
    fn u_mouse_set_arrow_cursor(&mut self);
    /// `TheAudio->pauseAudio(affect)`.
    fn u_audio_pause(&mut self, affect: i32);
    /// `TheAudio->resumeAudio(affect)`.
    fn u_audio_resume(&mut self, affect: i32);
}

/// `frame % interval` of an `UnsignedInt` frame and an `Int` interval (converted to unsigned; 0 divides by zero).
fn frame_is_multiple(frame: u32, interval: i32) -> R<bool> {
    let d = interval as u32;
    if d == 0 {
        return Err(undefined(Undefined::ModuloByZero));
    }
    Ok(frame.is_multiple_of(d))
}

/// `GameLogic::update()`: one logic frame. On an error the frame stops where upstream's behaviour is undefined
/// (`m_isInUpdate` restored, as upstream's latch would be on a throw).
pub fn game_logic_update(state: &mut GameLogicState, dispatch: &mut DispatchState, h: &mut impl GameLogicHost) -> R<()> {
    let was = h.u_is_in_update();
    h.u_set_in_update(true);
    let r = update_frame(state, dispatch, h);
    h.u_set_in_update(was);
    r
}

fn update_frame(state: &mut GameLogicState, dispatch: &mut DispatchState, h: &mut impl GameLogicHost) -> R<()> {
    h.u_set_fp_mode();
    if h.u_start_new_game_pending() && !h.u_movie_playing() {
        h.u_start_new_game(false);
        h.u_set_start_new_game_pending(false);
    }
    let now = h.u_frame();
    h.u_client_set_frame(now);
    h.u_script_engine_update();

    let mut freeze_time = h.u_view_time_frozen() && !h.u_view_camera_movement_finished();
    freeze_time = freeze_time || h.u_script_time_frozen_debug() || h.u_script_time_frozen_script();
    if freeze_time && !plant(4001) {
        if h.u_command_list_contains(MSG_CLEAR_GAME_DATA) {
            h.u_script_force_unfreeze_time();
        } else {
            return Ok(());
        }
    }

    h.u_terrain_update();

    let frame = h.u_frame();
    let mode = h.u_game_mode();
    let playing = mode != game_mode::SHELL && mode != game_mode::NONE;
    let has_recorder = h.u_has_recorder();
    let is_mp = has_recorder && h.u_recorder_is_multiplayer() && playing;
    let is_solo = has_recorder && !h.u_recorder_is_multiplayer() && playing;
    let generate_for_mp = is_mp && {
        let interval = h.u_game_info_crc_interval().ok_or_else(|| undefined(Undefined::NullObject))?;
        frame_is_multiple(frame, interval)?
    };
    let generate_for_solo = is_solo && frame_is_multiple(frame, if plant(4002) { 1 } else { h.u_replay_crc_interval() })?;
    if generate_for_solo || generate_for_mp {
        state.crc = get_crc(state, CRC_RECALC, None, h).map_err(|s| IniError::new(IniErrorKind::Xfer(s)))?;
        // the same message either way (upstream's two branches differ only in a debug log)
        let playback = h.u_has_recorder() && h.u_recorder_playback();
        h.u_send_logic_crc(state.crc, playback);
    }

    h.u_stats_collector_update();
    if !h.u_has_recorder() {
        return Err(undefined(Undefined::NullObject));
    }
    h.u_recorder_update();

    process_command_list(dispatch, h)?;

    h.u_run_sleepy_updates();
    // validateSleepyUpdate(): empty outside _DEBUG
    h.u_ai_update();
    h.u_build_assistant_update();
    h.u_partition_update();

    h.u_process_destroy_list();
    h.u_command_list_reset();
    h.u_weapon_store_update();
    h.u_locomotor_store_update();
    h.u_victory_conditions_update();

    for id in h.u_objects() {
        if h.u_object_is_disabled(id) && !plant(4003) {
            h.u_check_disabled_status(id);
        }
    }

    if !h.u_start_new_game_pending() {
        h.u_advance_frame();
    }
    Ok(())
}

/// `processCommandList(TheCommandList)`: the CRCs seen cleared, each message dispatched, then (when a local CRC was
/// seen and the network has not seen a mismatch) the CRCs checked: fewer than the connected players, or one that
/// differs from the lowest player's, is a mismatch the network is told of.
pub fn process_command_list(dispatch: &mut DispatchState, h: &mut impl GameLogicHost) -> R<()> {
    dispatch.cached_crcs.clear();
    dispatch.should_validate_crcs = false;
    for msg in h.u_command_list() {
        h.u_dispatch(dispatch, msg)?;
    }
    if dispatch.should_validate_crcs {
        if !h.u_has_network() {
            // TheNetwork->sawCRCMismatch() before the null check
            return Err(undefined(Undefined::NullObject));
        }
        if h.u_network_saw_crc_mismatch() {
            return Ok(());
        }
        let mut saw_mismatch = false;
        let mut num_players = 0usize;
        for i in 0..MAX_SLOTS as i32 {
            if h.u_network_player_connected(i) {
                num_players += 1;
            }
        }
        if dispatch.cached_crcs.len() < num_players && !plant(4004) {
            saw_mismatch = true;
        } else {
            let mut it = dispatch.cached_crcs.values();
            // begin()->second of an empty map dereferences end()
            let validator = *it.next().ok_or_else(|| undefined(Undefined::EndIteratorDereferenced))?;
            for &c in it {
                if c != validator {
                    saw_mismatch = true;
                }
            }
        }
        if saw_mismatch {
            h.u_network_set_saw_crc_mismatch();
        }
    }
    Ok(())
}

/// `getCRC(mode, deepCRCFileName)`: `CRC_CACHED` answers the last; `CRC_RECALC` takes one over the objects in list
/// order, the logic's random seed, the partition manager, the player list and the AI, each after a marker string;
/// a deep CRC (`deep` given: an `XferDeepCRC`, a save-mode stream) has no seed and adds the game state's
/// logic-only save data. The deep stream's bytes are returned with it (upstream writes them to the named file).
pub fn get_crc(state: &GameLogicState, mode: i32, deep: Option<&mut Vec<u8>>, h: &mut impl GameLogicHost) -> XferResult<u32> {
    if mode != CRC_RECALC {
        return Ok(state.crc);
    }
    h.u_set_fp_mode();
    match deep {
        Some(bytes) => {
            let mut x = XferDeepCrc::new();
            x.open(b"deep")?;
            crc_body(&mut x, h)?;
            x.close()?;
            bytes.clear();
            bytes.extend_from_slice(x.bytes());
            Ok(x.get_crc())
        }
        None => {
            let mut x = XferCrc::new();
            x.open(b"lightCRC")?;
            crc_body(&mut x, h)?;
            x.close()?;
            Ok(x.get_crc())
        }
    }
}

fn crc_body(x: &mut dyn Xfer, h: &mut impl GameLogicHost) -> XferResult<()> {
    let mut marker = b"MARKER:Objects".to_vec();
    x.xfer_ascii_string(&mut marker)?;
    for id in h.u_objects() {
        h.u_crc_object(id, x)?;
    }
    let mut seed = h.u_random_seed_crc();
    if x.get_xfer_mode() == XferMode::Crc && !plant(4005) {
        x.xfer_unsigned_int(&mut seed)?;
    }
    let mut marker = b"MARKER:ThePartitionManager".to_vec();
    x.xfer_ascii_string(&mut marker)?;
    h.u_crc_partition_manager(x)?;
    let mut marker = b"MARKER:ThePlayerList".to_vec();
    x.xfer_ascii_string(&mut marker)?;
    h.u_crc_player_list(x)?;
    let mut marker = b"MARKER:TheAI".to_vec();
    x.xfer_ascii_string(&mut marker)?;
    h.u_crc_ai(x)?;
    if x.get_xfer_mode() == XferMode::Save {
        let mut marker = b"MARKER:GameSave".to_vec();
        x.xfer_ascii_string(&mut marker)?;
        h.u_crc_game_state(x)?;
    }
    Ok(())
}

/// `isIntroMoviePlaying()`.
pub fn is_intro_movie_playing(h: &impl GameLogicHost) -> bool {
    h.u_start_new_game_pending() && h.u_movie_playing()
}

/// `selectObject(obj, createNewSelection, playerMask, affectClient)`: for each player of the mask, a group of the
/// object made the player's selection (or added to it), then destroyed; the object must be mass-selectable unless
/// it starts a new selection. A `PlayerMaskType` is 16 bits.
pub fn select_object(id: Option<u32>, create_new_selection: bool, mut player_mask: u16, affect_client: bool, h: &mut impl GameLogicHost) {
    let Some(id) = id else {
        return;
    };
    if !h.u_object_is_mass_selectable(id) && !create_new_selection {
        return;
    }
    while player_mask != 0 {
        let Some(player) = h.u_each_player_from_mask(&mut player_mask) else {
            return;
        };
        let group = h.u_create_group();
        h.u_group_add(group, id);
        if create_new_selection {
            h.u_player_set_selected_group(player, Some(group));
        } else {
            h.u_player_add_group_to_selection(player, group);
        }
        h.u_destroy_group(group);
        if affect_client && h.u_object_has_drawable(id) {
            h.u_ui_select_drawable(id);
        }
    }
}

/// `deselectObject(obj, playerMask, affectClient)`: for each player of the mask, the object taken out of a group
/// of its selection, which becomes the selection (destroyed after), or none when the group emptied.
pub fn deselect_object(id: Option<u32>, mut player_mask: u16, affect_client: bool, h: &mut impl GameLogicHost) {
    let Some(id) = id else {
        return;
    };
    while player_mask != 0 {
        let Some(player) = h.u_each_player_from_mask(&mut player_mask) else {
            return;
        };
        let group = h.u_create_group();
        h.u_player_selection_as_group(player, group);
        let deleted = h.u_group_remove(group, id);
        if !deleted {
            h.u_player_set_selected_group(player, Some(group));
            h.u_destroy_group(group);
        } else {
            h.u_player_set_selected_group(player, None);
        }
        if affect_client && h.u_object_has_drawable(id) {
            h.u_ui_deselect_drawable(id);
        }
    }
}

/// `setGamePaused(paused, pauseMusic)`: pausing remembers the UI's input and the mouse's visibility, shows the arrow,
/// disables input and pauses the audio (all of it, or all but the music); resuming restores them.
pub fn set_game_paused(state: &mut GameLogicState, paused: bool, pause_music: bool, h: &mut impl GameLogicHost) {
    if paused == state.game_paused {
        return;
    }
    state.game_paused = paused;
    let affect = if pause_music && !plant(4006) { AUDIO_AFFECT_ALL } else { AUDIO_AFFECT_ALL & !AUDIO_AFFECT_MUSIC };
    if paused {
        state.input_enabled_memory = h.u_ui_input_enabled();
        state.mouse_visible_memory = h.u_mouse_visible();
        h.u_mouse_set_visible(true);
        h.u_mouse_set_arrow_cursor();
        if state.input_enabled_memory {
            h.u_ui_set_input_enabled(false);
        }
        h.u_audio_pause(affect);
    } else {
        h.u_mouse_set_visible(state.mouse_visible_memory);
        if state.input_enabled_memory {
            h.u_ui_set_input_enabled(true);
        }
        h.u_audio_resume(affect);
    }
}

/// `processProgress(playerId, percentage)`.
pub fn process_progress(state: &mut GameLogicState, player: i32, percentage: i32, h: &mut impl GameLogicHost) {
    h.u_load_screen_process_progress(player, percentage);
    last_heard_from(state, player, h);
}

/// `processProgressComplete(playerId)`: a slot's load is complete (once).
pub fn process_progress_complete(state: &mut GameLogicState, player: i32, h: &mut impl GameLogicHost) {
    let Some(i) = usize::try_from(player).ok().filter(|&i| i < MAX_SLOTS) else {
        return;
    };
    if state.progress_complete[i] {
        return;
    }
    state.progress_complete[i] = true;
    last_heard_from(state, player, h);
}

/// `isProgressComplete()`: outside a multiplayer game, without a network, or forced by a time-out, true; else
/// whether every slot completed.
pub fn is_progress_complete(state: &GameLogicState, h: &impl GameLogicHost) -> bool {
    let mode = h.u_game_mode();
    let multiplayer = mode == game_mode::LAN || mode == game_mode::INTERNET;
    if !multiplayer || !h.u_has_network() || state.force_game_start_by_timeout {
        return true;
    }
    state.progress_complete.iter().all(|&c| c)
}

/// `lastHeardFrom(playerId)`.
pub fn last_heard_from(state: &mut GameLogicState, player: i32, h: &mut impl GameLogicHost) {
    if let Some(i) = usize::try_from(player).ok().filter(|&i| i < MAX_SLOTS) {
        state.progress_complete_timeout[i] = h.u_time_get_time();
    }
}

/// `testTimeOut()`: the game is forced to start when every slot not yet complete was last heard from
/// `PROGRESS_COMPLETE_TIMEOUT` or more ago.
pub fn test_time_out(state: &mut GameLogicState, h: &mut impl GameLogicHost) {
    if is_progress_complete(state, h) {
        return;
    }
    let cur = h.u_time_get_time();
    for i in 0..MAX_SLOTS {
        if state.progress_complete[i] {
            continue;
        }
        // Int arithmetic: the sum wraps as MSVC's does
        if state.progress_complete_timeout[i].wrapping_add(PROGRESS_COMPLETE_TIMEOUT) > cur && !plant(4007) {
            return;
        }
    }
    state.force_game_start_by_timeout = true;
}

/// `timeOutGameStart()`.
pub fn time_out_game_start(state: &mut GameLogicState) {
    state.force_game_start_by_timeout = true;
}

/// `initTimeOutValues()`: each network player's time-out set to now (a player index of 8 or more is past the array).
pub fn init_time_out_values(state: &mut GameLogicState, h: &mut impl GameLogicHost) -> R<()> {
    if !h.u_has_network() {
        return Ok(());
    }
    let n = h.u_network_num_players();
    for i in 0..n.max(0) as usize {
        if i >= MAX_SLOTS {
            return Err(undefined(Undefined::ArrayIndexOutOfRange));
        }
        state.progress_complete_timeout[i] = h.u_time_get_time();
    }
    Ok(())
}

/// `getObjectCount()`.
pub fn object_count(h: &impl GameLogicHost) -> u32 {
    h.u_objects().len() as u32
}

/// `setBuildableStatusOverride(tt, bs)` (a NULL template does nothing).
pub fn set_buildable_status_override(state: &mut GameLogicState, template_name: Option<&[u8]>, status: i32) {
    if let Some(name) = template_name {
        state.buildable_overrides.insert(name.to_vec(), status);
    }
}

/// `findBuildableStatusOverride(tt, bs)`.
pub fn find_buildable_status_override(state: &GameLogicState, template_name: Option<&[u8]>) -> Option<i32> {
    template_name.and_then(|n| state.buildable_overrides.get(n).copied())
}

/// The control-bar override key: `buf[0] = '0' + slot; strcpy(&buf[1], name)`, read back as a C string.
fn control_bar_key(command_set: &[u8], slot: i32) -> R<Vec<u8>> {
    if command_set.len() >= 255 {
        return Err(undefined(Undefined::MessageOverflow));
    }
    let digit = (b'0' as i32).wrapping_add(slot) as u8;
    if digit == 0 {
        return Ok(Vec::new());
    }
    let mut k = vec![if plant(4008) { b'0' } else { digit }];
    k.extend_from_slice(command_set);
    Ok(k)
}

/// `setControlBarOverride(commandSetName, slot, commandButton)`.
pub fn set_control_bar_override(state: &mut GameLogicState, command_set: &[u8], slot: i32, button: Option<u32>) -> R<()> {
    let k = control_bar_key(command_set, slot)?;
    state.control_bar_overrides.insert(k, button);
    Ok(())
}

/// `findControlBarOverride(commandSetName, slot, commandButton)`: `Some(button)` when there is an override (the
/// button perhaps NULL).
pub fn find_control_bar_override(state: &GameLogicState, command_set: &[u8], slot: i32) -> R<Option<Option<u32>>> {
    let k = control_bar_key(command_set, slot)?;
    Ok(state.control_bar_overrides.get(&k).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_interval_divides_by_zero() {
        assert!(frame_is_multiple(5, 0).is_err());
        assert_eq!(frame_is_multiple(200, 100), Ok(true));
        // a negative interval is a large unsigned one
        assert_eq!(frame_is_multiple(0, -1), Ok(true));
        assert_eq!(frame_is_multiple(5, -1), Ok(false));
    }

    #[test]
    fn control_bar_keys() {
        assert_eq!(control_bar_key(b"Set", 3).unwrap(), b"3Set".to_vec());
        assert_eq!(control_bar_key(b"Set", -48).unwrap(), Vec::<u8>::new());
        assert!(control_bar_key(&[b'a'; 255], 0).is_err());
    }
}
