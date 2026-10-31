// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogicDispatch.cpp
//   (`logicMessageDispatcher` and the file's statics and helpers: `theBuildPlan`, `thePlanSubject`, `doSetRallyPoint`,
//   `getSingleObjectFromSelection`)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameLogic::logicMessageDispatcher`: each command of the command list, executed (layer 12 batch D1).
//!
//! # Shape
//!
//! The dispatcher reads a message's arguments and decides which objects, groups and players to call, in what order.
//! What it calls is the [`DispatchHost`] trait: the AI groups ([`GroupCall`], one enum for every group command, each
//! "from the player"), objects, players, the factories, the build assistant, the terrain, the victory conditions, the
//! recorder, the network, and the client (UI messages, sounds, the radar, EVA, the control bar, drawables, the camera
//! and mouse: [`DispatchHost::d_client`], events with no answer). Objects are named by id, players by index, groups by
//! the AI's group id.
//!
//! # Release build
//!
//! The debug-only messages (`MSG_DEBUG_*`, compiled only with `_DEBUG`, `_INTERNAL` or `_ALLOW_DEBUG_CHEATS_IN_RELEASE`)
//! and the surrender messages (`ALLOW_SURRENDER`, not defined) are not compiled upstream: they fall to the `switch`'s
//! end, as any other unlisted type.
//!
//! # Upstream facts kept
//!
//! - `MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION` has no `break`: it falls into `MSG_DO_ATTACK_OBJECT`, which reads
//!   argument 0 (the location's `x` bits) as an object id.
//! - `MSG_CANCEL_UNIT_CREATE` with a producer that has no production interface `return`s: the selection's group is
//!   never destroyed.
//! - `MSG_REMOVE_BEACON` destroys its group only when it is empty; a non-empty one is left to the AI.
//! - `MSG_SELF_DESTRUCT`'s search for an ally reads its loop variable after the loop (MSVC's for-scope): the player
//!   dies alone only when the loop ran to the end; a `break` after a transfer stops it, a `continue` past a defeated
//!   ally does not.
//! - The network's `m_cachedCRCs[playerIndex]` is a `std::map`: any player index is a key. ~~(written unchecked,
//!   [`Undefined::ArrayIndexOutOfRange`] past `MAX_SLOTS`)~~ **CORRECTED 2026-10-31** (batch L1): batch D1 took it for
//!   an array; upstream's member is `std::map<Int, UnsignedInt>`, whose size and order `processCommandList` reads.
//! - `doMoveTo` is never called, so the build plan never gains a subject; `MSG_META_END_PATH_BUILD`'s loop runs over
//!   none.
//! - The issuing player (`ThePlayerList->getNthPlayer(msg->getPlayerIndex())`) is dereferenced unchecked by most cases
//!   ([`Undefined::NullObject`] when there is none).

use std::collections::BTreeMap;
use crate::common::base_type::{Coord3D, ICoord2D};
use crate::common::ini::{IniError, Undefined};
use crate::common::message_stream::*;
use crate::logic::modules::plant;

type R<T> = Result<T, IniError>;

fn null() -> IniError {
    IniError::undefined(Undefined::NullObject)
}

/// `MAX_PATH_SUBJECTS`.
pub const MAX_PATH_SUBJECTS: usize = 64;
/// `MAX_SLOTS`.
pub const MAX_SLOTS: usize = 8;
/// `NO_MAX_SHOTS_LIMIT`.
pub const NO_MAX_SHOTS_LIMIT: i32 = 0x7fff_ffff;
/// `LOCKED_TEMPORARILY`.
pub const LOCKED_TEMPORARILY: i32 = 1;
/// `LOCKED_PERMANENTLY`.
pub const LOCKED_PERMANENTLY: i32 = 2;
/// `PRIMARY_WEAPON`.
pub const PRIMARY_WEAPON: i32 = 0;
/// `SCIENCE_INVALID`.
pub const SCIENCE_INVALID: i32 = -1;
/// `ALLIES` (`enum Relationship`).
pub const ALLIES: i32 = 2;
/// `OBJECT_STATUS_UNDER_CONSTRUCTION` and `OBJECT_STATUS_RECONSTRUCTING`, the two `testStatus` questions asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusAsked {
    UnderConstruction,
    Reconstructing,
}

/// The file's statics and the two `GameLogic` members the dispatcher writes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DispatchState {
    /// `theBuildPlan`.
    pub build_plan: bool,
    /// `thePlanSubject[0..thePlanSubjectCount]` (only `doMoveTo` adds, and nothing calls it).
    pub plan_subjects: Vec<u32>,
    /// `GameLogic::m_shouldValidateCRCs`.
    pub should_validate_crcs: bool,
    /// `GameLogic::m_cachedCRCs` (a `std::map<Int, UnsignedInt>`: by player index, in order).
    pub cached_crcs: BTreeMap<i32, u32>,
}


/// A group command (`AIGroup::...`, every one `CMD_FROM_PLAYER` where it takes a source).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GroupCall {
    /// `setWeaponLockForGroup(slot, lock)`: answers whether a lock was set.
    SetWeaponLock { slot: i32, lock: i32 },
    /// `releaseWeaponLockForGroup(lock)`.
    ReleaseWeaponLock { lock: i32 },
    /// `groupAttackPosition(pos or NULL, maxShots)`.
    AttackPosition { pos: Option<Coord3D>, max_shots: i32 },
    /// `groupAttackObject(target, maxShots)`.
    AttackObject { target: u32, max_shots: i32 },
    /// `groupForceAttackObject(target, maxShots)`.
    ForceAttackObject { target: u32, max_shots: i32 },
    /// `groupAttackMoveToPosition(pos, maxShots)`.
    AttackMoveTo { pos: Coord3D, max_shots: i32 },
    /// `groupMoveToPosition(pos, addWaypoint)`.
    MoveTo { pos: Coord3D, add_waypoint: bool },
    /// `groupCombatDrop(target or NULL, pos)`.
    CombatDrop { target: Option<u32>, pos: Coord3D },
    /// `setMineClearingDetail(true)`.
    MineClearingDetail,
    /// `groupDoSpecialPower(id, options)`.
    SpecialPower { id: u32, options: u32 },
    /// `groupDoSpecialPowerAtLocation(id, pos, angle, objectInWay or NULL, options)`.
    SpecialPowerAtLocation { id: u32, pos: Coord3D, angle: f32, object_in_way: Option<u32>, options: u32 },
    /// `groupDoSpecialPowerAtObject(id, target, options)`.
    SpecialPowerAtObject { id: u32, target: u32, options: u32 },
    /// `groupOverrideSpecialPowerDestination(type, pos)`.
    OverrideSpecialPowerDestination { sp_type: i32, pos: Coord3D },
    /// `groupGuardPosition(pos, mode)`.
    GuardPosition { pos: Coord3D, mode: i32 },
    /// `groupGuardObject(obj, mode)`.
    GuardObject { obj: u32, mode: i32 },
    /// `groupIdle`.
    Idle,
    /// `groupScatter`.
    Scatter,
    /// `groupCreateFormation`.
    CreateFormation,
    /// `groupCheer`.
    Cheer,
    /// `groupEnter(obj)`.
    Enter { obj: u32 },
    /// `groupEvacuate`.
    Evacuate,
    /// `groupExecuteRailedTransport`.
    ExecuteRailedTransport,
    /// `groupHackInternet`.
    HackInternet,
    /// `groupGetRepaired(depot)`.
    GetRepaired { obj: u32 },
    /// `groupDock(obj)`.
    Dock { obj: u32 },
    /// `groupGetHealed(obj)`.
    GetHealed { obj: u32 },
    /// `groupRepair(obj)`.
    Repair { obj: u32 },
    /// `groupResumeConstruction(obj)`.
    ResumeConstruction { obj: u32 },
    /// `isIdle()`: answers.
    IsIdle,
    /// `queueUpgrade(upgrade)` (the upgrade's name key).
    QueueUpgrade { key: i32 },
    /// `groupSell`.
    Sell,
    /// `groupToggleOvercharge`.
    ToggleOvercharge,
}

/// What the dispatcher tells the client (no answer; a headless run drops them).
#[derive(Clone, Debug, PartialEq)]
pub enum ClientCall {
    /// `TheStatsCollector->collectMsgStats(msg)` (when there is a stats collector).
    CollectMsgStats { ty: MessageType },
    /// `TheInGameUI->message(TheGameText->fetch(label))` (and the formatted ones, by their label).
    UiMessage { label: &'static str },
    /// `TheAudio->addAudioEvent(&AudioEventRTS(name))` with its position, player and object where set.
    Audio { name: &'static str, pos: Option<Coord3D>, player: Option<i32>, object: Option<u32> },
    /// `TheControlBar->markUIDirty()`.
    MarkUiDirty,
    /// `TheRadar->createEvent(pos, RADAR_EVENT_INFORMATION)`.
    RadarInformation { pos: Coord3D },
    /// `TheEva->setShouldPlay(EVA_BeaconDetected)`.
    EvaBeaconDetected,
    /// The `BeaconClientUpdate` modules of the object's drawable: `hideBeacon()`.
    HideBeacon { obj: u32 },
    /// The drawable's `setCaptionText(text)` (`clearCaptionText()` for an empty text).
    BeaconCaption { obj: u32, text: Vec<u16> },
    /// `TheInGameUI->clearPopupMessageData()`.
    ClearPopupMessage,
    /// `TheGameEngine->setFramesPerSecondLimit(fps)` and `m_useFpsLimit = true`.
    FramesPerSecondLimit { fps: i32 },
    /// `TheTacticalView->setLocation(pos, angle, pitch, zoom)`.
    ReplayCameraLocation { pos: Coord3D, angle: f32, pitch: f32, zoom: f32 },
    /// `TheMouse->setCursor(cursor)`, `setPosition`, `TheLookAtTranslator->setCurrentPos`.
    ReplayMouse { cursor: i32, pos: ICoord2D },
    /// `TheInGameUI->deselectAllDrawables()`.
    DeselectAllDrawables,
    /// `TheInGameUI->selectDrawable(obj's drawable)`.
    SelectDrawable { obj: u32 },
}

/// The world as `logicMessageDispatcher` reaches it.
pub trait DispatchHost {
    // --- the game, players, objects ---
    /// `TheGameLogic->isInGame()`.
    fn d_is_in_game(&self) -> bool;
    /// `ThePlayerList->getNthPlayer(i) != NULL`.
    fn d_player_exists(&self, i: i32) -> bool;
    /// `ThePlayerList->getPlayerCount()`.
    fn d_player_count(&self) -> i32;
    /// `ThePlayerList->getLocalPlayer()`'s index (`None`: no local player).
    fn d_local_player(&self) -> Option<i32>;
    /// `TheGameLogic->findObjectByID(id) != NULL`.
    fn d_find_object(&self, id: u32) -> bool;
    /// `*obj->getPosition()`.
    fn d_object_position(&self, obj: u32) -> Coord3D;
    /// `obj->getControllingPlayer()`'s index (`None`: no controlling player).
    fn d_controlling_player(&self, obj: u32) -> Option<i32>;
    // --- groups ---
    /// `TheAI->createGroup()`.
    fn d_create_group(&mut self) -> u32;
    /// `TheAI->destroyGroup(g)`.
    fn d_destroy_group(&mut self, g: u32);
    /// `player->getCurrentSelectionAsAIGroup(g)`.
    fn d_selection_into_group(&mut self, player: i32, g: u32);
    /// `g->isEmpty()`.
    fn d_group_is_empty(&self, g: u32) -> bool;
    /// `g->removeAnyObjectsNotOwnedByPlayer(player)`.
    fn d_group_remove_not_owned(&mut self, g: u32, player: i32) -> bool;
    /// `g->getAllIDs()`.
    fn d_group_ids(&self, g: u32) -> Vec<u32>;
    /// `g->add(obj)`.
    fn d_group_add(&mut self, g: u32, obj: u32);
    /// A group command; `SetWeaponLock` and `IsIdle` answer, the others answer false.
    fn d_group(&mut self, g: u32, call: GroupCall) -> R<bool>;
    // --- objects ---
    /// The object's AI's `executeWaypointQueue()` (when it has an AI).
    fn d_ai_execute_waypoint_queue(&mut self, obj: u32);
    /// `obj->releaseWeaponLock(lock)`.
    fn d_release_weapon_lock(&mut self, obj: u32, lock: i32);
    /// The object's AI's `aiExit(container, CMD_FROM_PLAYER)` (when it has an AI).
    fn d_ai_exit(&mut self, obj: u32, container: u32);
    /// `obj->isLocallyControlled()`.
    fn d_is_locally_controlled(&self, obj: u32) -> bool;
    /// `TheAI->pathfinder()->clientSafeQuickDoesPathExist(BasicHumanLocomotor's set, obj's position, pos)`.
    fn d_rally_path_exists(&mut self, obj: u32, pos: &Coord3D) -> bool;
    /// `obj->getDrawable() && drawable->isSelected()`.
    fn d_drawable_selected(&self, obj: u32) -> bool;
    /// `obj->getObjectExitInterface()->setRallyPoint(pos)` (when it has one).
    fn d_set_rally_point(&mut self, obj: u32, pos: &Coord3D);
    /// `obj->getProductionUpdateInterface() != NULL`.
    fn d_has_production(&self, obj: u32) -> bool;
    /// The production interface's `cancelUpgrade(upgrade)`.
    fn d_cancel_upgrade(&mut self, obj: u32, key: i32);
    /// The production interface's `queueCreateUnit(template, productionID)`.
    fn d_queue_create_unit(&mut self, obj: u32, template_id: i32, production_id: i32);
    /// The production interface's `cancelUnitCreate(productionID)`.
    fn d_cancel_unit_create(&mut self, obj: u32, production_id: i32);
    /// `obj->testStatus(status)`.
    fn d_test_status(&self, obj: u32, status: StatusAsked) -> bool;
    /// `player->getMoney()->deposit(obj->getTemplate()->calcCostToBuild(player))`.
    fn d_refund_build_cost(&mut self, player: i32, obj: u32);
    /// `obj->kill()`.
    fn d_kill(&mut self, obj: u32);
    /// `TheGameLogic->destroyObject(obj)`.
    fn d_destroy_object(&mut self, obj: u32);
    /// `TheGameLogic->selectObject(obj, createNewGroup, player->getPlayerMask())`.
    fn d_select_object(&mut self, obj: u32, create_new_group: bool, player: i32);
    /// `TheGameLogic->deselectObject(obj, player->getPlayerMask())`.
    fn d_deselect_object(&mut self, obj: u32, player: i32);
    /// `player->setCurrentlySelectedAIGroup(NULL)`.
    fn d_clear_selected_group(&mut self, player: i32);
    // --- templates, upgrades, building ---
    /// `TheUpgradeCenter->findUpgradeByKey(key) != NULL`.
    fn d_upgrade_exists(&self, key: i32) -> bool;
    /// `TheThingFactory->findByTemplateID(id) != NULL`.
    fn d_template_exists(&self, id: i32) -> bool;
    /// `TheBuildAssistant->buildObjectNow(constructor, template, loc, angle, controlling player)`.
    fn d_build_object_now(&mut self, constructor: u32, template_id: i32, loc: &Coord3D, angle: f32);
    /// `TheBuildAssistant->buildObjectLineNow(constructor, template, start, end, angle, controlling player)`.
    fn d_build_object_line_now(&mut self, constructor: u32, template_id: i32, start: &Coord3D, end: &Coord3D, angle: f32);
    // --- beacons ---
    /// The position clamped into the terrain: `getExtent`, `isInRegionNoZ`, else `findClosestEdgePoint`.
    fn d_clamp_to_terrain(&self, pos: &Coord3D) -> Coord3D;
    /// `TheThingFactory->findTemplate(player->getPlayerTemplate()->getBeaconTemplate())`'s id, if any (the player's
    /// template dereferenced unchecked).
    fn d_beacon_template(&self, player: i32) -> R<Option<i32>>;
    /// `TheVictoryConditions->hasSinglePlayerBeenDefeated(player)`.
    fn d_player_defeated(&self, player: i32) -> bool;
    /// `player->countObjectsByThingTemplate(1, &template, false, &count)`.
    fn d_count_objects_of_template(&self, player: i32, template_id: i32) -> i32;
    /// `TheMultiplayerSettings->getMaxBeaconsPerPlayer()`.
    fn d_max_beacons(&self) -> i32;
    /// `TheThingFactory->newObject(template, player->getDefaultTeam())`, then `setPosition(pos)` and
    /// `setProducer(NULL)`: the new object's id.
    fn d_new_beacon(&mut self, template_id: i32, player: i32, pos: &Coord3D) -> R<u32>;
    /// `a->getRelationship(b->getDefaultTeam())`.
    fn d_relationship(&self, a: i32, b: i32) -> i32;
    /// `ThePlayerList->getLocalPlayer()->isPlayerObserver()`.
    fn d_local_is_observer(&self) -> R<bool>;
    /// `TheThingFactory->findTemplate(obj's controlling player's template's beacon)->isEquivalentTo(obj's template)`
    /// (the controlling player and the template found dereferenced unchecked).
    fn d_is_controllers_beacon(&self, obj: u32) -> R<bool>;
    /// `obj->getDrawable() != NULL`.
    fn d_has_drawable(&self, obj: u32) -> bool;
    // --- players ---
    /// `player->setLogicalRetaliationModeEnabled(enable)`.
    fn d_set_retaliation(&mut self, player: i32, enable: bool);
    /// `to->transferAssetsFromThat(from)`.
    fn d_transfer_assets(&mut self, to: i32, from: i32);
    /// `player->killPlayer()`.
    fn d_kill_player(&mut self, player: i32);
    /// `player->processCreateTeamGameMessage`, `processSelectTeamGameMessage` or `processAddTeamGameMessage` (`kind`
    /// 0, 1, 2) of hotkey team `n`.
    fn d_team_message(&mut self, kind: i32, player: i32, n: i32, msg: &GameMessage);
    /// `player->attemptToPurchaseScience(science)`.
    fn d_purchase_science(&mut self, player: i32, science: i32);
    // --- the game loop, the recorder, the network ---
    /// `prepareNewGame(gameMode, diff, rankPoints)`.
    fn d_prepare_new_game(&mut self, mode: i32, diff: i32, rank_points: i32);
    /// `startNewGame(FALSE)`.
    fn d_start_new_game(&mut self);
    /// `TheGameLogic->clearGameData()`.
    fn d_clear_game_data(&mut self);
    /// `TheGlobalData->m_framesPerSecondLimit`.
    fn d_global_fps_limit(&self) -> i32;
    /// `TheStatsCollector != NULL`.
    fn d_has_stats_collector(&self) -> bool;
    /// `TheInGameUI != NULL`.
    fn d_has_in_game_ui(&self) -> bool;
    /// `TheRecorder->getMode() == RECORDERMODETYPE_PLAYBACK` (with `TheRecorder` there).
    fn d_recorder_playback(&self) -> bool;
    /// `TheRecorder != NULL`.
    fn d_has_recorder(&self) -> bool;
    /// `TheRecorder->handleCRCMessage(crc, playerIndex, fromPlayback)`.
    fn d_recorder_handle_crc(&mut self, crc: u32, player: i32, from_playback: bool) -> R<()>;
    /// `TheGlobalData->m_useCameraInReplay && TheControlBar->getObserverLookAtPlayer() == player`.
    fn d_replay_camera_follows(&self, player: i32) -> bool;
    /// `TheTacticalView->isCameraMovementFinished()`.
    fn d_camera_movement_finished(&self) -> bool;
    /// `TheLookAtTranslator->hasMouseMovedRecently()`.
    fn d_mouse_moved_recently(&self) -> bool;
    /// `TheNetwork != NULL`.
    fn d_has_network(&self) -> bool;
    /// `player->getPlayerType() == PLAYER_HUMAN && TheNetwork->getPlayerName(slot) == player->getPlayerDisplayName()`.
    fn d_network_slot_is(&self, slot: i32, player: i32) -> bool;
    /// `TheNetwork->isPlayerConnected(slot)`.
    fn d_network_connected(&self, slot: i32) -> bool;
    /// `player->isLocalPlayer()`.
    fn d_is_local_player(&self, player: i32) -> bool;
    /// The client.
    fn d_client(&mut self, call: ClientCall);
}

/// `getSingleObjectFromSelection(group)`: the group's first object, if it still exists (`findObjectByID` of the first
/// id; an empty group's `begin()` is dereferenced unchecked).
fn single_object_from_selection(group: Option<u32>, h: &impl DispatchHost) -> R<Option<u32>> {
    let Some(g) = group else { return Ok(None) };
    let ids = h.d_group_ids(g);
    let Some(&first) = ids.first() else {
        return Err(IniError::undefined(Undefined::EndIteratorDereferenced));
    };
    Ok(h.d_find_object(first).then_some(first))
}

/// `msg->getArgument(i)->boolean`: a byte other than 0 or 1 read as a `Bool` is undefined.
fn bool_arg(msg: &GameMessage, i: i32) -> R<bool> {
    let b = msg.get_argument(i).0[0] & 0xff;
    if b > 1 {
        return Err(IniError::undefined(Undefined::InvalidBoolLoaded));
    }
    Ok(b == 1)
}

fn obj_arg(msg: &GameMessage, i: i32, h: &impl DispatchHost) -> Option<u32> {
    let id = msg.get_argument(i).object_id();
    h.d_find_object(id).then_some(id)
}

/// `doSetRallyPoint(obj, pos)`.
fn do_set_rally_point(obj: u32, pos: Coord3D, h: &mut impl DispatchHost) -> R<()> {
    let local = h.d_is_locally_controlled(obj);
    if !h.d_rally_path_exists(obj, &pos) {
        if local {
            h.d_client(ClientCall::UiMessage { label: "GUI:RallyPointNoPath" });
            h.d_client(ClientCall::Audio { name: "UnableToSetRallyPoint", pos: Some(pos), player: None, object: None });
        }
        return Ok(());
    }
    if local {
        h.d_client(ClientCall::UiMessage { label: "GUI:RallyPointSet" });
        let player = h.d_controlling_player(obj).ok_or_else(null)?;
        h.d_client(ClientCall::Audio { name: "RallyPointSet", pos: Some(pos), player: Some(player), object: None });
        if h.d_drawable_selected(obj) {
            h.d_client(ClientCall::MarkUiDirty);
        }
    }
    h.d_set_rally_point(obj, &pos);
    Ok(())
}

/// `GameLogic::logicMessageDispatcher(msg, userData)`.
pub fn logic_message_dispatcher(state: &mut DispatchState, msg: &GameMessage, h: &mut impl DispatchHost) -> R<()> {
    let player_index = msg.get_player_index();
    let this_player = h.d_player_exists(player_index).then_some(player_index);
    let need = |p: Option<i32>| p.ok_or_else(null);
    let ty = msg.get_type();
    let arg = |i: i32| msg.get_argument(i);
    let mut group: Option<u32> = None;
    if h.d_is_in_game() && ty >= MSG_BEGIN_NETWORK_MESSAGES && ty <= MSG_END_NETWORK_MESSAGES && ty != MSG_LOGIC_CRC && ty != MSG_SET_REPLAY_CAMERA {
        let g = h.d_create_group();
        h.d_selection_into_group(need(this_player)?, g);
        group = Some(g);
        if h.d_group_is_empty(g) {
            h.d_destroy_group(g);
            group = None;
        }
        if let Some(g) = group {
            if h.d_group_remove_not_owned(g, need(this_player)?) && !plant(3971) {
                group = None;
            }
        }
        if h.d_has_stats_collector() {
            h.d_client(ClientCall::CollectMsgStats { ty });
        }
    }

    // the cases that fall through or return early come first
    let mut fall_into_attack = false;
    if ty == MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION {
        let loc = arg(0).location();
        let sp_type = arg(1).integer();
        match obj_arg(msg, 2, h) {
            Some(source) => {
                let g = h.d_create_group();
                h.d_group_add(g, source);
                h.d_group(g, GroupCall::OverrideSpecialPowerDestination { sp_type, pos: loc })?;
                h.d_destroy_group(g);
            }
            None => {
                if let Some(g) = group {
                    h.d_group(g, GroupCall::OverrideSpecialPowerDestination { sp_type, pos: loc })?;
                }
            }
        }
        // no `break`: on into MSG_DO_ATTACK_OBJECT
        fall_into_attack = !plant(3972);
    }

    let t = ty;
    if fall_into_attack || t == MSG_DO_ATTACK_OBJECT {
        if let Some(enemy) = obj_arg(msg, 0, h) {
            if let Some(g) = group {
                h.d_group(g, GroupCall::ReleaseWeaponLock { lock: LOCKED_TEMPORARILY })?;
                h.d_group(g, GroupCall::AttackObject { target: enemy, max_shots: NO_MAX_SHOTS_LIMIT })?;
            }
        }
    } else if t == MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION {
        // handled above (only with the planted bug: no fall-through)
    } else if t == MSG_NEW_GAME {
        let mode = arg(0).integer();
        let count = msg.get_argument_count();
        let mut rank = 0;
        let mut diff = 1; // DIFFICULTY_NORMAL
        if count >= 2 {
            diff = arg(1).integer();
        }
        if count >= 3 {
            rank = arg(2).integer();
        }
        if count >= 4 {
            let mut fps = arg(3).integer();
            if !(1..=1000).contains(&fps) {
                fps = h.d_global_fps_limit();
            }
            h.d_client(ClientCall::FramesPerSecondLimit { fps });
        }
        h.d_prepare_new_game(mode, diff, rank);
        h.d_start_new_game();
    } else if t == MSG_CLEAR_GAME_DATA {
        if let Some(g) = group.take() {
            h.d_destroy_group(g);
        }
        h.d_clear_game_data();
    } else if t == MSG_META_BEGIN_PATH_BUILD {
        if !state.build_plan {
            state.build_plan = true;
            state.plan_subjects.clear();
        }
    } else if t == MSG_META_END_PATH_BUILD {
        for &s in &state.plan_subjects.clone() {
            h.d_ai_execute_waypoint_queue(s);
        }
        state.build_plan = false;
        state.plan_subjects.clear();
    } else if t == MSG_SET_RALLY_POINT {
        let dest = arg(1).location();
        if let Some(obj) = obj_arg(msg, 0, h) {
            do_set_rally_point(obj, dest, h)?;
        }
    } else if t == MSG_DO_WEAPON {
        let slot = arg(0).integer();
        let max = arg(1).integer();
        if let Some(g) = group {
            if h.d_group(g, GroupCall::SetWeaponLock { slot, lock: LOCKED_TEMPORARILY })? {
                h.d_group(g, GroupCall::AttackPosition { pos: None, max_shots: max })?;
            }
        }
    } else if t == MSG_COMBATDROP_AT_OBJECT {
        let target = obj_arg(msg, 0, h);
        if let Some(g) = group {
            // `*targetObject->getPosition()` of a target that is gone
            let target = target.ok_or_else(null)?;
            let pos = h.d_object_position(target);
            h.d_group(g, GroupCall::CombatDrop { target: Some(target), pos })?;
        }
    } else if t == MSG_COMBATDROP_AT_LOCATION {
        let loc = arg(0).location();
        if let Some(g) = group {
            h.d_group(g, GroupCall::CombatDrop { target: None, pos: loc })?;
        }
    } else if t == MSG_DO_WEAPON_AT_OBJECT {
        let slot = arg(0).integer();
        let target = obj_arg(msg, 1, h);
        let max = arg(2).integer();
        if let Some(target) = target {
            if let Some(g) = group {
                if h.d_group(g, GroupCall::SetWeaponLock { slot, lock: LOCKED_TEMPORARILY })? {
                    h.d_group(g, GroupCall::AttackObject { target, max_shots: max })?;
                }
            }
        }
    } else if t == MSG_SWITCH_WEAPONS {
        let slot = arg(0).integer();
        if let Some(g) = group {
            h.d_group(g, GroupCall::SetWeaponLock { slot, lock: LOCKED_PERMANENTLY })?;
        }
    } else if t == MSG_SET_MINE_CLEARING_DETAIL {
        if let Some(g) = group {
            h.d_group(g, GroupCall::MineClearingDetail)?;
        }
    } else if t == MSG_ENABLE_RETALIATION_MODE {
        let p = arg(0).integer();
        let enable = bool_arg(msg, 1)?;
        if h.d_player_exists(p) {
            h.d_set_retaliation(p, enable);
        }
    } else if t == MSG_DO_WEAPON_AT_LOCATION {
        let slot = arg(0).integer();
        let loc = arg(1).location();
        let max = arg(2).integer();
        if let Some(g) = group {
            if h.d_group(g, GroupCall::SetWeaponLock { slot, lock: LOCKED_TEMPORARILY })? {
                h.d_group(g, GroupCall::AttackPosition { pos: Some(loc), max_shots: max })?;
            }
        }
    } else if t == MSG_DO_SPECIAL_POWER || t == MSG_DO_SPECIAL_POWER_AT_LOCATION || t == MSG_DO_SPECIAL_POWER_AT_OBJECT {
        let id = arg(0).integer() as u32;
        let (call, source_arg) = if t == MSG_DO_SPECIAL_POWER {
            (GroupCall::SpecialPower { id, options: arg(1).integer() as u32 }, 2)
        } else if t == MSG_DO_SPECIAL_POWER_AT_LOCATION {
            let pos = arg(1).location();
            let angle = arg(2).real();
            let object_in_way = obj_arg(msg, 3, h);
            (GroupCall::SpecialPowerAtLocation { id, pos, angle, object_in_way, options: arg(4).integer() as u32 }, 5)
        } else {
            let Some(target) = obj_arg(msg, 1, h) else {
                return finish(group, this_player, h);
            };
            (GroupCall::SpecialPowerAtObject { id, target, options: arg(2).integer() as u32 }, 3)
        };
        match obj_arg(msg, source_arg, h) {
            Some(source) => {
                let g = h.d_create_group();
                h.d_group_add(g, source);
                h.d_group(g, call)?;
                h.d_destroy_group(g);
            }
            None => {
                if let Some(g) = group {
                    h.d_group(g, call)?;
                }
            }
        }
    } else if t == MSG_DO_ATTACKMOVETO {
        let dest = arg(0).location();
        if let Some(g) = group {
            h.d_group(g, GroupCall::ReleaseWeaponLock { lock: LOCKED_TEMPORARILY })?;
            h.d_group(g, GroupCall::AttackMoveTo { pos: dest, max_shots: NO_MAX_SHOTS_LIMIT })?;
        }
    } else if t == MSG_DO_FORCEMOVETO || t == MSG_DO_SALVAGE || t == MSG_DO_MOVETO || t == MSG_ADD_WAYPOINT {
        let dest = arg(0).location();
        if let Some(g) = group {
            h.d_group(g, GroupCall::ReleaseWeaponLock { lock: LOCKED_TEMPORARILY })?;
            h.d_group(g, GroupCall::MoveTo { pos: dest, add_waypoint: t == MSG_ADD_WAYPOINT })?;
        }
    } else if t == MSG_DO_GUARD_POSITION {
        let pos = arg(0).location();
        let mode = arg(1).integer();
        if let Some(g) = group {
            h.d_group(g, GroupCall::GuardPosition { pos, mode })?;
        }
    } else if t == MSG_DO_GUARD_OBJECT {
        if let Some(obj) = obj_arg(msg, 0, h) {
            let mode = arg(1).integer();
            if let Some(g) = group {
                h.d_group(g, GroupCall::GuardObject { obj, mode })?;
            }
        }
    } else if let Some(call) = simple_group_call(t) {
        if let Some(g) = group {
            if call == GroupCall::Evacuate || call == GroupCall::HackInternet {
                h.d_group(g, GroupCall::ReleaseWeaponLock { lock: LOCKED_TEMPORARILY })?;
            }
            h.d_group(g, call)?;
        }
    } else if t == MSG_CLEAR_INGAME_POPUP_MESSAGE {
        if h.d_has_in_game_ui() {
            h.d_client(ClientCall::ClearPopupMessage);
        }
    } else if t == MSG_ENTER {
        if let Some(obj) = obj_arg(msg, 1, h) {
            if let Some(g) = group {
                h.d_group(g, GroupCall::ReleaseWeaponLock { lock: LOCKED_TEMPORARILY })?;
                h.d_group(g, GroupCall::Enter { obj })?;
            }
        }
    } else if t == MSG_EXIT {
        let exiter = obj_arg(msg, 0, h);
        let container = single_object_from_selection(group, h)?;
        if let (Some(exiter), Some(container)) = (exiter, container) {
            if h.d_controlling_player(exiter) == this_player || plant(3978) {
                h.d_release_weapon_lock(exiter, LOCKED_TEMPORARILY);
                h.d_ai_exit(exiter, container);
            }
        }
    } else if let Some(make) = object_group_call(t) {
        if let Some(obj) = obj_arg(msg, 0, h) {
            if let Some(g) = group {
                h.d_group(g, make(obj))?;
            }
        }
    } else if t == MSG_DO_FORCE_ATTACK_OBJECT {
        if let Some(enemy) = obj_arg(msg, 0, h) {
            if let Some(g) = group {
                h.d_group(g, GroupCall::ReleaseWeaponLock { lock: LOCKED_TEMPORARILY })?;
                h.d_group(g, GroupCall::ForceAttackObject { target: enemy, max_shots: NO_MAX_SHOTS_LIMIT })?;
            }
        }
    } else if t == MSG_DO_FORCE_ATTACK_GROUND {
        let pos = arg(0).location();
        if let Some(g) = group {
            let requires_primary = !h.d_group(g, GroupCall::IsIdle)?;
            if requires_primary && !plant(3973) {
                h.d_group(g, GroupCall::SetWeaponLock { slot: PRIMARY_WEAPON, lock: LOCKED_TEMPORARILY })?;
                h.d_group(g, GroupCall::AttackPosition { pos: Some(pos), max_shots: NO_MAX_SHOTS_LIMIT })?;
                h.d_group(g, GroupCall::ReleaseWeaponLock { lock: LOCKED_TEMPORARILY })?;
            } else {
                h.d_group(g, GroupCall::ReleaseWeaponLock { lock: LOCKED_TEMPORARILY })?;
                h.d_group(g, GroupCall::AttackPosition { pos: Some(pos), max_shots: NO_MAX_SHOTS_LIMIT })?;
            }
        }
    } else if t == MSG_QUEUE_UPGRADE {
        let key = arg(1).integer();
        if h.d_upgrade_exists(key) {
            if let Some(g) = group {
                h.d_group(g, GroupCall::QueueUpgrade { key })?;
            }
        }
    } else if t == MSG_CANCEL_UPGRADE {
        let producer = single_object_from_selection(group, h)?;
        let key = arg(0).integer();
        if let Some(p) = producer {
            if h.d_upgrade_exists(key) && h.d_controlling_player(p) == this_player && h.d_has_production(p) {
                h.d_cancel_upgrade(p, key);
            }
        }
    } else if t == MSG_QUEUE_UNIT_CREATE {
        let producer = single_object_from_selection(group, h)?;
        let template = arg(0).integer();
        let production_id = arg(1).integer();
        if let Some(p) = producer {
            if h.d_template_exists(template) && h.d_has_production(p) {
                h.d_queue_create_unit(p, template, production_id);
            }
        }
    } else if t == MSG_CANCEL_UNIT_CREATE {
        let producer = single_object_from_selection(group, h)?;
        let production_id = arg(0).integer();
        if let Some(p) = producer {
            if h.d_controlling_player(p) == this_player {
                if !h.d_has_production(p) && !plant(3974) {
                    // `return`: the selection's group is not destroyed
                    return Ok(());
                }
                if h.d_has_production(p) {
                    h.d_cancel_unit_create(p, production_id);
                }
            }
        }
    } else if t == MSG_DOZER_CONSTRUCT || t == MSG_DOZER_CONSTRUCT_LINE {
        let constructor = single_object_from_selection(group, h)?;
        let template = arg(0).integer();
        let loc = arg(1).location();
        let angle = arg(2).real();
        if let Some(c) = constructor {
            if h.d_template_exists(template) {
                if t == MSG_DOZER_CONSTRUCT {
                    h.d_build_object_now(c, template, &loc, angle);
                } else {
                    let end = arg(3).location();
                    h.d_build_object_line_now(c, template, &loc, &end, angle);
                }
                h.d_client(ClientCall::Audio { name: "PlaceBuilding", pos: None, player: None, object: Some(c) });
            }
        }
    } else if t == MSG_DOZER_CANCEL_CONSTRUCT {
        if let Some(b) = single_object_from_selection(group, h)? {
            if h.d_controlling_player(b) == this_player && h.d_test_status(b, StatusAsked::UnderConstruction) {
                if !h.d_test_status(b, StatusAsked::Reconstructing) {
                    h.d_refund_build_cost(need(this_player)?, b);
                }
                h.d_kill(b);
            }
        }
    } else if t == MSG_CREATE_SELECTED_GROUP_NO_SOUND || t == MSG_CREATE_SELECTED_GROUP {
        let create_new = bool_arg(msg, 0)?;
        if h.d_player_exists(player_index) {
            let mut first = true;
            for i in 1..i32::from(msg.get_argument_count()) {
                let Some(obj) = obj_arg(msg, i, h) else { continue };
                h.d_select_object(obj, create_new && first, player_index);
                first = plant(3976);
            }
        }
    } else if t == MSG_REMOVE_FROM_SELECTED_GROUP {
        if h.d_player_exists(player_index) {
            for i in 0..i32::from(msg.get_argument_count()) {
                let Some(obj) = obj_arg(msg, i, h) else { continue };
                h.d_deselect_object(obj, player_index);
            }
        }
    } else if t == MSG_DESTROY_SELECTED_GROUP {
        if h.d_player_exists(player_index) {
            h.d_clear_selected_group(player_index);
        }
    } else if t == MSG_PLACE_BEACON {
        place_beacon(msg, need(this_player)?, h)?;
    } else if t == MSG_REMOVE_BEACON {
        let me = need(this_player)?;
        let all = h.d_create_group();
        h.d_selection_into_group(me, all);
        for obj in h.d_group_ids(all) {
            if !h.d_find_object(obj) {
                continue;
            }
            if h.d_is_controllers_beacon(obj)? {
                if h.d_controlling_player(obj) == Some(me) {
                    h.d_destroy_object(obj);
                    h.d_client(ClientCall::MarkUiDirty);
                } else if h.d_local_player() == Some(me) && h.d_has_drawable(obj) {
                    h.d_client(ClientCall::HideBeacon { obj });
                }
            }
        }
        if h.d_group_is_empty(all) {
            h.d_destroy_group(all);
        }
    } else if t == MSG_SET_BEACON_TEXT {
        if let Some(g) = group {
            for obj in h.d_group_ids(g) {
                if h.d_find_object(obj) && h.d_has_drawable(obj) {
                    // `UnicodeString::concat(WideChar)` of each argument's wide character (a 0 ends nothing: concat of
                    // a 0 character appends nothing)
                    let text: Vec<u16> = (0..i32::from(msg.get_argument_count())).map(|i| arg(i).wide_char()).filter(|&c| c != 0).collect();
                    h.d_client(ClientCall::BeaconCaption { obj, text });
                }
            }
        }
    } else if t == MSG_SELF_DESTRUCT {
        let me = need(this_player)?;
        if bool_arg(msg, 0)? {
            let count = h.d_player_count();
            let mut i = 0;
            while i < count {
                if i != player_index {
                    if !h.d_player_exists(i) {
                        return Err(null());
                    }
                    if h.d_relationship(me, i) == ALLIES && h.d_relationship(i, me) == ALLIES {
                        if h.d_player_defeated(i) {
                            i += 1;
                            continue;
                        }
                        h.d_transfer_assets(i, me);
                        h.d_kill_player(me);
                        break;
                    }
                }
                i += 1;
            }
            if i == count && !plant(3975) {
                h.d_kill_player(me);
            }
        } else {
            h.d_kill_player(me);
        }
    } else if t == MSG_SET_REPLAY_CAMERA {
        if h.d_recorder_playback() && h.d_replay_camera_follows(player_index_ptr(this_player)) && h.d_camera_movement_finished() {
            let pos = arg(0).location();
            let (angle, pitch, zoom) = (arg(1).real(), arg(2).real(), arg(3).real());
            h.d_client(ClientCall::ReplayCameraLocation { pos, angle, pitch, zoom });
            if !h.d_mouse_moved_recently() {
                h.d_client(ClientCall::ReplayMouse { cursor: arg(4).integer(), pos: arg(5).pixel() });
            }
        }
    } else if (MSG_CREATE_TEAM0.0..=MSG_CREATE_TEAM9.0).contains(&t.0) {
        if h.d_player_exists(player_index) {
            h.d_team_message(0, player_index, t.0 - MSG_CREATE_TEAM0.0, msg);
        }
    } else if (MSG_SELECT_TEAM0.0..=MSG_SELECT_TEAM9.0).contains(&t.0) {
        if h.d_player_exists(player_index) {
            h.d_team_message(1, player_index, t.0 - MSG_SELECT_TEAM0.0, msg);
        }
    } else if (MSG_ADD_TEAM0.0..=MSG_ADD_TEAM9.0).contains(&t.0) {
        if h.d_player_exists(player_index) {
            h.d_team_message(2, player_index, t.0 - MSG_ADD_TEAM0.0, msg);
        }
    } else if t == MSG_LOGIC_CRC {
        if h.d_has_network() {
            let me = need(this_player)?;
            let mut slot = -1;
            for i in 0..MAX_SLOTS as i32 {
                if h.d_network_slot_is(i, me) {
                    slot = i;
                    break;
                }
            }
            if slot >= 0 && h.d_network_connected(slot) {
                if h.d_is_local_player(me) {
                    state.should_validate_crcs = true;
                }
                let crc = arg(0).integer() as u32;
                state.cached_crcs.insert(player_index, crc);
            }
        } else if h.d_has_recorder() && h.d_recorder_playback() {
            let crc = arg(0).integer() as u32;
            let me = need(this_player)?;
            h.d_recorder_handle_crc(crc, me, bool_arg(msg, 1)?)?;
        }
    } else if t == MSG_PURCHASE_SCIENCE {
        let science = arg(0).integer();
        if science != SCIENCE_INVALID {
            if let Some(me) = this_player {
                h.d_purchase_science(me, science);
            }
        }
    }
    finish(group, this_player, h)
}

/// `getControllingPlayer() == thisPlayer` style comparisons use the index; a missing player compares as none (-1).
fn player_index_ptr(p: Option<i32>) -> i32 {
    p.unwrap_or(-1)
}

/// The group commands that take nothing but the group.
fn simple_group_call(t: MessageType) -> Option<GroupCall> {
    Some(match t {
        x if x == MSG_DO_STOP => GroupCall::Idle,
        x if x == MSG_DO_SCATTER => GroupCall::Scatter,
        x if x == MSG_CREATE_FORMATION => GroupCall::CreateFormation,
        x if x == MSG_DO_CHEER => GroupCall::Cheer,
        x if x == MSG_EVACUATE => GroupCall::Evacuate,
        x if x == MSG_EXECUTE_RAILED_TRANSPORT => GroupCall::ExecuteRailedTransport,
        x if x == MSG_INTERNET_HACK => GroupCall::HackInternet,
        x if x == MSG_SELL => GroupCall::Sell,
        x if x == MSG_TOGGLE_OVERCHARGE => GroupCall::ToggleOvercharge,
        _ => return None,
    })
}

/// The group commands on argument 0's object, which must exist.
fn object_group_call(t: MessageType) -> Option<fn(u32) -> GroupCall> {
    Some(match t {
        x if x == MSG_GET_REPAIRED => |obj| GroupCall::GetRepaired { obj },
        x if x == MSG_DOCK => |obj| GroupCall::Dock { obj },
        x if x == MSG_GET_HEALED => |obj| GroupCall::GetHealed { obj },
        x if x == MSG_DO_REPAIR => |obj| GroupCall::Repair { obj },
        x if x == MSG_RESUME_CONSTRUCTION => |obj| GroupCall::ResumeConstruction { obj },
        _ => return None,
    })
}

/// `MSG_PLACE_BEACON`.
fn place_beacon(msg: &GameMessage, me: i32, h: &mut impl DispatchHost) -> R<()> {
    let pos = h.d_clamp_to_terrain(&msg.get_argument(0).location());
    let thing = h.d_beacon_template(me)?;
    match thing {
        Some(thing) if !h.d_player_defeated(me) => {
            let count = h.d_count_objects_of_template(me, thing);
            if count >= h.d_max_beacons() + i32::from(plant(3977)) {
                if h.d_local_player() == Some(me) {
                    h.d_client(ClientCall::UiMessage { label: "GUI:TooManyBeacons" });
                    h.d_client(ClientCall::Audio { name: "BeaconPlacementFailed", pos: Some(pos), player: Some(me), object: None });
                }
                return Ok(());
            }
            let object = h.d_new_beacon(thing, me, &pos)?;
            let local = h.d_local_player().ok_or_else(null)?;
            if h.d_relationship(me, local) == ALLIES || h.d_local_is_observer()? {
                h.d_client(ClientCall::UiMessage { label: "GUI:BeaconPlaced" });
                h.d_client(ClientCall::Audio { name: "BeaconPlaced", pos: Some(pos), player: Some(me), object: None });
                h.d_client(ClientCall::RadarInformation { pos });
                if h.d_relationship(local, me) == ALLIES {
                    h.d_client(ClientCall::EvaBeaconDetected);
                }
                h.d_client(ClientCall::MarkUiDirty);
            } else {
                h.d_client(ClientCall::HideBeacon { obj: object });
            }
        }
        _ => {
            h.d_client(ClientCall::UiMessage { label: "GUI:BeaconPlacementFailed" });
            h.d_client(ClientCall::Audio { name: "BeaconPlacementFailed", pos: Some(pos), player: Some(me), object: None });
        }
    }
    Ok(())
}

/// The end of the dispatcher: in a replay whose camera follows this player, the client selects the group's drawables;
/// then the group is destroyed.
fn finish(group: Option<u32>, this_player: Option<i32>, h: &mut impl DispatchHost) -> R<()> {
    if let Some(g) = group {
        if h.d_recorder_playback() && h.d_replay_camera_follows(player_index_ptr(this_player)) {
            h.d_client(ClientCall::DeselectAllDrawables);
            for obj in h.d_group_ids(g) {
                if h.d_find_object(obj) && h.d_has_drawable(obj) {
                    h.d_client(ClientCall::SelectDrawable { obj });
                }
            }
        }
        h.d_destroy_group(g);
    }
    Ok(())
}
