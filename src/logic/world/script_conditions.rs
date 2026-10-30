// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ScriptConditions.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptConditions.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: John Ahlquist, Nov. 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ScriptConditions` (`TheScriptConditions`): the evaluation of a map script's conditions. `evaluateCondition` dispatches on the
//! condition's type to one of the `evaluate*` questions about players, teams, named units, trigger areas, the client's selection,
//! special powers, sciences and the skirmish AI's interests. Some questions cache their answer in the condition (`m_customData` 1 true,
//! -1 false, 0 nothing yet; `m_customFrame` the frame the answer is good for) or in a parameter (`playerFromParam` keeps the player's
//! mask in the side parameter's `Int`; the skirmish special power keeps the frame it is next worth asking at). The transports that
//! `UNIT_EMPTIED` has seen are a list (`s_transportStatuses`), here the conditions' own.
//!
//! The world is reached through [`ScriptConditionsHost`]: the script engine's lookups (`getTeamNamed`, `getUnitNamed`,
//! `getQualifiedTriggerAreaByName`, `getObjectTypes`, the completion queries), the player list, players, teams, objects, triggers,
//! the partition's searches, the thing factory, the special power and science stores, the control bar's buttons, the client's
//! selection, the camera, the victory conditions and the audio. Each host method is one upstream call (or one read of an upstream
//! member); the order and the conditions of the calls are this file's.
//!
//! # Upstream facts kept
//!
//! - `playerFromParam` caches a mask in the parameter: the player's, or `0xFFFF0000` for no player; for `<This Player's Enemy>` it stores
//!   0, so the name is looked up every time. A cached mask that no player has any more answers no player.
//! - `evaluateNamedAttackedByPlayer` and `evaluateTeamAttackedByPlayer` compare players as pointers: an attack with no source player is
//!   by the side parameter when that names no player (NULL == NULL).
//! - `evaluatePlayerHasUnitKindInArea` stores the object-count frame in `m_customData` (not `m_customFrame`), so its cached answer is
//!   used only while that frame is 1 or -1 (`u32::MAX`) and the condition's frame is not behind.
//! - `evaluateSkirmishPlayerTechBuildingWithinDistancePerimeter` keeps its first answer for ever (no frame).
//! - `evaluateTeamIsContained`: an object that is not contained stays not contained (`isContained && ...` with `isContained` false), so
//!   an exiting object never counts.
//! - `evaluateTeamHasObjectStatus` returns false at a NULL member (never, the list holds objects).
//! - `evaluateBuiltByPlayer` with no type that names a template returns false without caching.
//! - `evaluatePlayerHasCredits` compares the parameter's `Int` with the money's `UnsignedInt` (a negative count is huge).
//! - `evaluateSkirmishSpecialPowerIsReady` compares the parameter's `Int` with the frame (`UnsignedInt`), and a module's ready frame with
//!   `nextFrame` as unsigned values.
//! - `evaluateUnitHasEmptied` compares the frame it last saw with `frame - 1` in `UnsignedInt` (at frame 0 that is `u32::MAX`).
//! - `evaluateEnemySighted` with a relation that is none of `REL_NEUTRAL`, `REL_FRIEND`, `REL_ENEMY` filters with an uninitialised
//!   `Int` ([`Undefined::UninitialisedLocal`]).
//! - `evaluateUnitHealth` converts `(health*100 + initial/2)/initial` (a `Real`) to `Int`: not representable (an initial health of 0)
//!   is [`Undefined::FloatToIntOutOfRange`].
//! - `evaluateTypeSighted` reads the template's name of a seen object of the player's unchecked ([`Undefined::NullObject`] when it has
//!   none); `evaluateNamedSelected` reads the object of each selected drawable unchecked (the same).
//! - A condition whose type reads a parameter it does not have dereferences NULL ([`Undefined::NullParameter`]); `ENEMY_SIGHTED` and the
//!   two in-area comparisons check the count first and answer false.
//! - The types `evaluateCondition` does not handle (`CONDITION_FALSE`, `COUNTER`, `FLAG`, `CONDITION_TRUE`, `TIMER_EXPIRED`, the
//!   obsolete ones and the sequential-execution ones, which the script engine answers itself) and the defunct general's are false.
//!
//! Differential-verified in `tests/differential_script_conditions_c1.rs` (the file with one mechanical edit, `typename`, on stand-ins):
//! 20,000 programs, 567,137 operations (379,813 evaluations covering all 109 types, 4,252 direct calls), 0 mismatches; plants
//! 3841-3854 caught.
//!
//! `p1w13-sonnet`, layer 11, batch C1, 2026-10-30.

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::{Coord3D, ICoord3D};
use crate::common::disabled_types::{DisabledType, DISABLED_HELD, DISABLED_UNMANNED};
use crate::common::game_common::{ObjectShroudStatus, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::ObjectId;
use crate::common::ini::{IniError, Undefined};
use crate::common::kind_of::{
    KindOfMaskType, KindOfType, KINDOF_CRATE, KINDOF_INERT, KINDOF_MP_COUNT_FOR_VICTORY, KINDOF_PROJECTILE, KINDOF_STRUCTURE,
};
use crate::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::logic::modules::plant;
use crate::logic::object_types::ObjectTypes;
use crate::logic::scripts::{condition_type as ct, Condition, Parameter, THIS_TEAM};
use crate::logic::world::partition::filters::{ALLOW_ALLIES, ALLOW_ENEMIES, ALLOW_NEUTRAL};

/// `THIS_PLAYER_ENEMY` (Scripts.h).
pub const THIS_PLAYER_ENEMY: &[u8] = b"<This Player's Enemy>";

/// `Parameter::LESS_THAN` .. `NOT_EQUAL`.
pub mod comparison {
    /// `LESS_THAN`.
    pub const LESS_THAN: i32 = 0;
    /// `LESS_EQUAL`.
    pub const LESS_EQUAL: i32 = 1;
    /// `EQUAL`.
    pub const EQUAL: i32 = 2;
    /// `GREATER_EQUAL`.
    pub const GREATER_EQUAL: i32 = 3;
    /// `GREATER`.
    pub const GREATER: i32 = 4;
    /// `NOT_EQUAL`.
    pub const NOT_EQUAL: i32 = 5;
}

/// `Parameter::REL_ENEMY`, `REL_NEUTRAL`, `REL_FRIEND` (`ENEMIES`, `NEUTRAL`, `ALLIES`).
pub mod relation {
    /// `REL_ENEMY`.
    pub const REL_ENEMY: i32 = 0;
    /// `REL_NEUTRAL`.
    pub const REL_NEUTRAL: i32 = 1;
    /// `REL_FRIEND`.
    pub const REL_FRIEND: i32 = 2;
}

/// A player: its index in the player list.
pub type PlayerH = i32;
/// A team instance (`Team*`), as the host names it.
pub type TeamH = u32;
/// An object: its id.
pub type ObjH = ObjectId;
/// A trigger area (`PolygonTrigger*`), as the host names it.
pub type TriggerH = u32;
/// A thing template, as the host names it.
pub type TemplateH = u32;
/// A special power template, as the host names it.
pub type SpecialPowerH = u32;
/// A command button, as the host names it.
pub type ButtonH = u32;

/// `BodyModuleInterface::getLastDamageInfo()`'s `in` fields that the conditions read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastDamage {
    /// `m_sourceTemplate`.
    pub source_template: Option<TemplateH>,
    /// `m_sourceID`.
    pub source_id: ObjectId,
    /// `m_sourcePlayerMask`.
    pub source_player_mask: u32,
}

/// `ContainModuleInterface`'s answers the conditions read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainInfo {
    /// `getPlayerWhoEntered()`.
    pub player_who_entered: u32,
    /// `getContainCount()`.
    pub count: u32,
    /// `getContainMax()`.
    pub max: u32,
    /// `isGarrisonable()`.
    pub garrisonable: bool,
}

/// A special power module's answers (`isReady`, `getReadyFrame`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialPowerModuleInfo {
    /// `isReady()`.
    pub ready: bool,
    /// `getReadyFrame()`.
    pub ready_frame: u32,
}

/// The world, as `ScriptConditions.cpp` reaches it. See the module docs.
pub trait ScriptConditionsHost {
    // --- the player list and players ---
    /// `ThePlayerList->getPlayerFromMask(mask)`.
    fn sc_player_from_mask(&self, mask: u32) -> Option<PlayerH>;
    /// `ThePlayerList->getNeutralPlayer()`.
    fn sc_neutral_player(&self) -> Option<PlayerH>;
    /// `getPlayerMask()`.
    fn sc_player_mask(&self, p: PlayerH) -> u32;
    /// `hasAnyObjects()`.
    fn sc_player_has_any_objects(&self, p: PlayerH) -> bool;
    /// `hasAnyBuildFacility()`.
    fn sc_player_has_any_build_facility(&self, p: PlayerH) -> bool;
    /// `getMoney()->countMoney()`.
    fn sc_player_money(&self, p: PlayerH) -> u32;
    /// `countBuildings()`.
    fn sc_player_count_buildings(&self, p: PlayerH) -> i32;
    /// `countObjects(must, mustNot)`.
    fn sc_player_count_objects(&self, p: PlayerH, must: &KindOfMaskType, must_not: &KindOfMaskType) -> i32;
    /// `getEnergy()->hasSufficientPower()`.
    fn sc_player_has_sufficient_power(&self, p: PlayerH) -> bool;
    /// `getEnergy()->getEnergySupplyRatio()`.
    fn sc_player_energy_supply_ratio(&self, p: PlayerH) -> f32;
    /// `getEnergy()->getProduction()`.
    fn sc_player_energy_production(&self, p: PlayerH) -> i32;
    /// `getEnergy()->getConsumption()`.
    fn sc_player_energy_consumption(&self, p: PlayerH) -> i32;
    /// `getPlayerTeams()`: each prototype's instance list, in order.
    fn sc_player_teams(&self, p: PlayerH) -> Vec<Vec<TeamH>>;
    /// `countObjectsByThingTemplate(n, templates, ignoreDead, counts)`: the count of each template, in order.
    fn sc_player_count_objects_by_thing_template(&self, p: PlayerH, templates: &[TemplateH], ignore_dead: bool) -> Vec<i32>;
    /// `getSciencePurchasePoints()`.
    fn sc_player_science_purchase_points(&self, p: PlayerH) -> i32;
    /// `isCapableOfPurchasingScience(science)`.
    fn sc_player_is_capable_of_purchasing_science(&self, p: PlayerH, science: ScienceType) -> bool;
    /// `getSide()`.
    fn sc_player_side(&self, p: PlayerH) -> AsciiString;
    /// `getSupplyBoxValue()`.
    fn sc_player_supply_box_value(&self, p: PlayerH) -> i32;
    /// `isSupplySourceSafe(minSupplies)`.
    fn sc_player_is_supply_source_safe(&mut self, p: PlayerH, min_supplies: i32) -> bool;
    /// `isSupplySourceAttacked()`.
    fn sc_player_is_supply_source_attacked(&mut self, p: PlayerH) -> bool;
    /// `getMpStartIndex()`.
    fn sc_player_mp_start_index(&self, p: PlayerH) -> i32;
    /// `getAttackedBy(index)`.
    fn sc_player_attacked_by(&self, p: PlayerH, index: i32) -> bool;

    // --- the script engine ---
    /// `TheScriptEngine->getPlayerFromAsciiString(name)`.
    fn sc_player_from_name(&self, name: &[u8]) -> Option<PlayerH>;
    /// `TheScriptEngine->getTeamNamed(name)`.
    fn sc_team_named(&self, name: &[u8]) -> Option<TeamH>;
    /// `TheScriptEngine->getUnitNamed(name)`.
    fn sc_unit_named(&self, name: &[u8]) -> Option<ObjH>;
    /// `TheScriptEngine->didUnitExist(name)`.
    fn sc_did_unit_exist(&self, name: &[u8]) -> bool;
    /// `TheScriptEngine->getQualifiedTriggerAreaByName(name)`.
    fn sc_trigger_named(&self, name: &[u8]) -> Option<TriggerH>;
    /// `TheScriptEngine->getFrameObjectCountChanged()`.
    fn sc_frame_object_count_changed(&self) -> u32;
    /// `TheScriptEngine->getObjectTypes(name)` (a copy of the list).
    fn sc_object_types(&self, name: &AsciiString) -> Option<ObjectTypes>;
    /// `TheScriptEngine->isVideoComplete(name, removeIfSo)`.
    fn sc_is_video_complete(&mut self, name: &[u8], remove: bool) -> bool;
    /// `TheScriptEngine->isSpeechComplete(name, removeIfSo)`.
    fn sc_is_speech_complete(&mut self, name: &[u8], remove: bool) -> bool;
    /// `TheScriptEngine->isAudioComplete(name, removeIfSo)`.
    fn sc_is_audio_complete(&mut self, name: &[u8], remove: bool) -> bool;
    /// `TheScriptEngine->isSpecialPowerTriggered(playerIndex, name, removeIfSo, sourceID)`.
    fn sc_is_special_power_triggered(&mut self, index: i32, name: &[u8], remove: bool, source: ObjectId) -> bool;
    /// `TheScriptEngine->isSpecialPowerMidway(..)`.
    fn sc_is_special_power_midway(&mut self, index: i32, name: &[u8], remove: bool, source: ObjectId) -> bool;
    /// `TheScriptEngine->isSpecialPowerComplete(..)`.
    fn sc_is_special_power_complete(&mut self, index: i32, name: &[u8], remove: bool, source: ObjectId) -> bool;
    /// `TheScriptEngine->isUpgradeComplete(..)`.
    fn sc_is_upgrade_complete(&mut self, index: i32, name: &[u8], remove: bool, source: ObjectId) -> bool;
    /// `TheScriptEngine->isScienceAcquired(playerIndex, science, removeIfSo)`.
    fn sc_is_science_acquired(&mut self, index: i32, science: ScienceType, remove: bool) -> bool;
    /// `TheScriptEngine->getObjectCount(playerIndex, name)`.
    fn sc_object_count(&self, index: i32, name: &[u8]) -> i32;
    /// `TheScriptEngine->setObjectCount(playerIndex, name, count)`.
    fn sc_set_object_count(&mut self, index: i32, name: &[u8], count: i32);
    /// `TheTeamFactory->findTeamPrototype(name)`: its instance list.
    fn sc_team_prototype_instances(&self, name: &[u8]) -> Option<Vec<TeamH>>;

    // --- teams ---
    /// `hasAnyObjects()`.
    fn sc_team_has_any_objects(&self, t: TeamH) -> bool;
    /// `hasAnyUnits()`.
    fn sc_team_has_any_units(&self, t: TeamH) -> bool;
    /// `getName()`.
    fn sc_team_name(&self, t: TeamH) -> AsciiString;
    /// `someInsideSomeOutside(trigger, whichToConsider)`.
    fn sc_team_some_inside_some_outside(&self, t: TeamH, trig: TriggerH, which: u32) -> bool;
    /// `allInside(trigger, whichToConsider)`.
    fn sc_team_all_inside(&self, t: TeamH, trig: TriggerH, which: u32) -> bool;
    /// `didAllEnter(trigger, whichToConsider)`.
    fn sc_team_did_all_enter(&self, t: TeamH, trig: TriggerH, which: u32) -> bool;
    /// `didPartialEnter(trigger, whichToConsider)`.
    fn sc_team_did_partial_enter(&self, t: TeamH, trig: TriggerH, which: u32) -> bool;
    /// `didAllExit(trigger, whichToConsider)`.
    fn sc_team_did_all_exit(&self, t: TeamH, trig: TriggerH, which: u32) -> bool;
    /// `didPartialExit(trigger, whichToConsider)`.
    fn sc_team_did_partial_exit(&self, t: TeamH, trig: TriggerH, which: u32) -> bool;
    /// `didEnterOrExit()`.
    fn sc_team_did_enter_or_exit(&self, t: TeamH) -> bool;
    /// `getState()`.
    fn sc_team_state(&self, t: TeamH) -> AsciiString;
    /// `isCreated()`.
    fn sc_team_is_created(&self, t: TeamH) -> bool;
    /// `getControllingPlayer()`.
    fn sc_team_controlling_player(&self, t: TeamH) -> Option<PlayerH>;
    /// `iterate_TeamMemberList()`, in order.
    fn sc_team_members(&self, t: TeamH) -> Vec<ObjH>;

    // --- objects ---
    /// `TheGameLogic->findObjectByID(id)`.
    fn sc_find_object(&self, id: ObjectId) -> Option<ObjH>;
    /// `isEffectivelyDead()`.
    fn sc_obj_is_effectively_dead(&self, o: ObjH) -> bool;
    /// `getPosition()`.
    fn sc_obj_position(&self, o: ObjH) -> Coord3D;
    /// `getTemplate()`.
    fn sc_obj_template(&self, o: ObjH) -> Option<TemplateH>;
    /// `isKindOf(kind)`.
    fn sc_obj_is_kind_of(&self, o: ObjH, kind: KindOfType) -> bool;
    /// `isInside(trigger)`.
    fn sc_obj_is_inside(&self, o: ObjH, trig: TriggerH) -> bool;
    /// `didEnter(trigger)`.
    fn sc_obj_did_enter(&self, o: ObjH, trig: TriggerH) -> bool;
    /// `didExit(trigger)`.
    fn sc_obj_did_exit(&self, o: ObjH, trig: TriggerH) -> bool;
    /// `getBodyModule()` is not NULL.
    fn sc_obj_has_body(&self, o: ObjH) -> bool;
    /// `getBodyModule()->getHealth()` and `getInitialHealth()` (with a body).
    fn sc_obj_health(&self, o: ObjH) -> (f32, f32);
    /// `getBodyModule()->getLastDamageInfo()` (with a body).
    fn sc_obj_last_damage(&self, o: ObjH) -> Option<LastDamage>;
    /// `getControllingPlayer()`.
    fn sc_obj_controlling_player(&self, o: ObjH) -> Option<PlayerH>;
    /// `getContain()`.
    fn sc_obj_contain(&self, o: ObjH) -> Option<ContainInfo>;
    /// `getStatusBits()`.
    fn sc_obj_status(&self, o: ObjH) -> ObjectStatusMaskType;
    /// `isDisabledByType(type)`.
    fn sc_obj_is_disabled_by_type(&self, o: ObjH, t: DisabledType) -> bool;
    /// `isDisabled()`.
    fn sc_obj_is_disabled(&self, o: ObjH) -> bool;
    /// `getShroudedStatus(playerIndex)`.
    fn sc_obj_shrouded_status(&self, o: ObjH, index: i32) -> ObjectShroudStatus;
    /// `getAIUpdateInterface()->getCompletedWaypoint()`: `None` without an AI, `Some(None)` with no waypoint, else the waypoint's
    /// three path labels.
    fn sc_obj_completed_waypoint(&self, o: ObjH) -> Option<Option<[AsciiString; 3]>>;
    /// `getContainedBy()` is not NULL.
    fn sc_obj_is_contained(&self, o: ObjH) -> bool;
    /// `isCaptured()`.
    fn sc_obj_is_captured(&self, o: ObjH) -> bool;
    /// `getSpecialPowerModule(power)`.
    fn sc_obj_special_power_module(&self, o: ObjH, power: SpecialPowerH) -> Option<SpecialPowerModuleInfo>;
    /// `hasSpecialPower(type)`.
    fn sc_obj_has_special_power(&self, o: ObjH, power_type: i32) -> bool;
    /// `findUpdateModule("SupplyWarehouseDockUpdate")`'s `getBoxesStored()`, or `None` without one.
    fn sc_obj_warehouse_boxes(&self, o: ObjH) -> Option<i32>;

    // --- templates and stores ---
    /// `TheThingFactory->findTemplate(name)`.
    fn sc_find_template(&self, name: &AsciiString) -> Option<TemplateH>;
    /// `getName()`.
    fn sc_template_name(&self, t: TemplateH) -> AsciiString;
    /// `friend_getBuildCost()`.
    fn sc_template_build_cost(&self, t: TemplateH) -> i32;
    /// `player->canBuild(template)` (for `ObjectTypes::canBuildAny`).
    fn sc_player_can_build(&self, p: PlayerH, t: TemplateH) -> bool;
    /// `TheSpecialPowerStore->findSpecialPowerTemplate(name)`.
    fn sc_find_special_power(&self, name: &[u8]) -> Option<SpecialPowerH>;
    /// `TheSpecialPowerStore->canUseSpecialPower(obj, power)`.
    fn sc_can_use_special_power(&self, o: ObjH, power: SpecialPowerH) -> bool;
    /// `getSpecialPowerType()`.
    fn sc_special_power_type(&self, power: SpecialPowerH) -> i32;
    /// `TheScienceStore->getScienceFromInternalName(name)`.
    fn sc_science_from_name(&self, name: &[u8]) -> ScienceType;
    /// `TheControlBar->findCommandButton(name)`.
    fn sc_find_command_button(&self, name: &[u8]) -> Option<ButtonH>;
    /// `getSpecialPowerTemplate()`.
    fn sc_button_special_power(&self, b: ButtonH) -> Option<SpecialPowerH>;
    /// `getUpgradeTemplate()` is not NULL.
    fn sc_button_has_upgrade(&self, b: ButtonH) -> bool;
    /// `isReady(obj)`.
    fn sc_button_is_ready(&self, b: ButtonH, o: ObjH) -> bool;

    // --- trigger areas and the partition ---
    /// `pointInTrigger(point)`.
    fn sc_trigger_point_in(&self, trig: TriggerH, p: ICoord3D) -> bool;
    /// `getCenterPoint()`.
    fn sc_trigger_center(&self, trig: TriggerH) -> Coord3D;
    /// `getRadius()`.
    fn sc_trigger_radius(&self, trig: TriggerH) -> f32;
    /// `iterateObjectsInRange(obj, visionRange, FROM_CENTER_2D, filters)`: with `relation` (an `ALLOW_*` set) the relationship filter
    /// first, then alive, not stealthed (unless detected or disguised) and on the same map side as `obj`.
    fn sc_partition_seen_by(&self, o: ObjH, relation: Option<i32>) -> Vec<ObjH>;
    /// `iterateObjectsInRange(&center, distance, FROM_CENTER_2D, {structure, player affiliation (neutral, with self), on map})`.
    fn sc_partition_supply_structures(&self, center: Coord3D, distance: f32, p: PlayerH) -> Vec<ObjH>;
    /// `getClosestObject(&center, radius, FROM_CENTER_2D, {tech building, allies, not own, on map})` is not NULL.
    fn sc_partition_tech_building_near(&self, center: Coord3D, radius: f32, p: PlayerH) -> bool;

    // --- bridges (the terrain logic) ---
    /// `TheTerrainLogic->anyBridgesDamageStatesChanged()`.
    fn sc_any_bridges_damage_states_changed(&self) -> bool;
    /// `TheTerrainLogic->isBridgeBroken(bridge)`.
    fn sc_is_bridge_broken(&self, o: ObjH) -> bool;
    /// `TheTerrainLogic->isBridgeRepaired(bridge)`.
    fn sc_is_bridge_repaired(&self, o: ObjH) -> bool;

    // --- the rest ---
    /// `TheGameLogic->getFrame()`.
    fn sc_frame(&self) -> u32;
    /// `TheGameEngine->isMultiplayerSession()`.
    fn sc_is_multiplayer_session(&self) -> bool;
    /// `TheInGameUI->getFrameSelectionChanged()`.
    fn sc_frame_selection_changed(&self) -> u32;
    /// `TheInGameUI->getAllSelectedDrawables()`: each drawable's object's name (`None`: a drawable with no object).
    fn sc_selected_object_names(&self) -> Vec<Option<AsciiString>>;
    /// `TheTacticalView->isCameraMovementFinished()`.
    fn sc_camera_movement_finished(&self) -> bool;
    /// `TheVictoryConditions->isLocalAlliedVictory()`.
    fn sc_is_local_allied_victory(&self) -> Result<bool, IniError>;
    /// `TheVictoryConditions->isLocalAlliedDefeat()`.
    fn sc_is_local_allied_defeat(&self) -> Result<bool, IniError>;
    /// `TheVictoryConditions->isLocalDefeat()`.
    fn sc_is_local_defeat(&self) -> bool;
    /// `TheAudio->hasMusicTrackCompleted(name, numberOfTimes)`.
    fn sc_music_track_completed(&self, name: &[u8], times: i32) -> bool;
}

/// `TransportStatus`: what `UNIT_EMPTIED` last saw of a transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportStatus {
    /// `m_objID`.
    pub obj_id: ObjectId,
    /// `m_frameNumber`.
    pub frame_number: u32,
    /// `m_unitCount`.
    pub unit_count: i32,
}

/// `class ScriptConditions`, with `s_transportStatuses`.
#[derive(Clone, Debug, Default)]
pub struct ScriptConditions {
    /// `s_transportStatuses`, head first (a new status is put at the head).
    pub transport_statuses: Vec<TransportStatus>,
}

type R<T> = Result<T, IniError>;

/// `pCondition->getParameter(i)`, dereferenced.
fn parm(c: &Condition, i: usize) -> R<&Parameter> {
    c.get_parameter(i as i32).ok_or(IniError::undefined(Undefined::NullParameter))
}

fn parm_mut(c: &mut Condition, i: usize) -> R<&mut Parameter> {
    if (i as i32) < c.num_parms {
        if let Some(p) = c.parms[i].as_mut() {
            return Ok(p);
        }
    }
    Err(IniError::undefined(Undefined::NullParameter))
}

fn s(c: &Condition, i: usize) -> R<Vec<u8>> {
    Ok(parm(c, i)?.string.clone())
}

fn int(c: &Condition, i: usize) -> R<i32> {
    Ok(parm(c, i)?.int)
}

/// The `switch (comparison)` of the counting conditions: `None` for a value that is no comparison.
fn compare<T: PartialOrd>(cmp: i32, a: T, b: T) -> Option<bool> {
    use comparison::*;
    Some(match cmp {
        LESS_THAN => a < b,
        LESS_EQUAL => a <= b,
        EQUAL => a == b,
        GREATER_EQUAL => a >= b,
        GREATER => a > b,
        NOT_EQUAL => a != b,
        _ => return None,
    })
}

fn ascii(b: &[u8]) -> AsciiString {
    AsciiString::try_from_bytes(b).unwrap_or_default()
}

impl ScriptConditions {
    /// `ScriptConditions()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `init()`: `reset()`.
    pub fn init(&mut self) {
        self.reset();
    }

    /// `reset()`: the transport statuses deleted.
    pub fn reset(&mut self) {
        self.transport_statuses.clear();
    }

    /// `update()`: nothing.
    pub fn update(&mut self) {}

    /// `playerFromParam(pSideParm)`. See the module docs for the cached mask.
    pub fn player_from_param<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, i: usize) -> R<Option<PlayerH>> {
        let parm = parm_mut(c, i)?;
        let mut mask = parm.int as u32;
        let player;
        if mask != 0 {
            player = h.sc_player_from_mask(mask);
        } else {
            player = h.sc_player_from_name(&parm.string);
            if let Some(p) = player {
                // Enemy player can change dynamically, so don't cache the player mask.
                if parm.string != THIS_PLAYER_ENEMY || plant(3841) {
                    mask = h.sc_player_mask(p);
                }
            } else {
                mask = 0xFFFF_0000;
            }
            parm.int = mask as i32;
        }
        Ok(player)
    }

    /// `objectTypesFromParam(pTypeParm, outObjectTypes)`: the list the script engine has by the parameter's name, else a list of that
    /// one name; an empty name leaves the list empty.
    pub fn object_types_from_param<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, i: usize) -> R<ObjectTypes> {
        let mut out = ObjectTypes::new();
        let st = ascii(&s(c, i)?);
        if st.is_empty() {
            return Ok(out);
        }
        match h.sc_object_types(&st) {
            None => out.add_object_type(&st),
            Some(types) => out = types,
        }
        Ok(out)
    }

    fn types_contain_template<H: ScriptConditionsHost>(h: &H, types: &ObjectTypes, t: Option<TemplateH>) -> bool {
        let name = t.map(|t| h.sc_template_name(t));
        types.is_in_set_of_template(name.as_ref())
    }

    /// `prepForPlayerCounting` then `countObjectsByThingTemplate`: `None` when no name is a template.
    fn count_types<H: ScriptConditionsHost>(h: &H, p: PlayerH, types: &ObjectTypes, ignore_dead: bool) -> Option<i32> {
        let mut templates = Vec::new();
        let mut counts = Vec::new();
        let n = types.prep_for_player_counting(|name| h.sc_find_template(name), &mut templates, &mut counts);
        if n > 0 {
            let got = h.sc_player_count_objects_by_thing_template(p, &templates, ignore_dead);
            Some(got.iter().fold(0i32, |a, b| a.wrapping_add(*b)))
        } else {
            None
        }
    }

    /// Whether any team of the player entered or exited an area since the last check (the area-counting conditions' "any changes").
    fn any_team_entered_or_exited<H: ScriptConditionsHost>(h: &H, p: PlayerH) -> bool {
        h.sc_player_teams(p).iter().any(|proto| proto.iter().any(|t| h.sc_team_did_enter_or_exit(*t)))
    }

    /// Every member of every team of the player, in order.
    fn player_members<H: ScriptConditionsHost>(h: &H, p: PlayerH) -> Vec<ObjH> {
        let mut out = Vec::new();
        for proto in h.sc_player_teams(p) {
            for t in proto {
                out.extend(h.sc_team_members(t));
            }
        }
        out
    }

    fn all_destroyed<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize) -> R<bool> {
        Ok(match self.player_from_param(h, c, sp)? {
            Some(p) => !h.sc_player_has_any_objects(p),
            None => true, // Non existent player is all destroyed.
        })
    }

    fn all_build_facilities_destroyed<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize) -> R<bool> {
        Ok(match self.player_from_param(h, c, sp)? {
            Some(p) => !h.sc_player_has_any_build_facility(p),
            None => true,
        })
    }

    fn is_destroyed<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize) -> R<bool> {
        Ok(match h.sc_team_named(&s(c, tp)?) {
            Some(t) => !h.sc_team_has_any_objects(t),
            None => false, // Non existent team is not destroyed.
        })
    }

    fn named_unit_destroyed<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize) -> R<bool> {
        let name = s(c, up)?;
        if let Some(o) = h.sc_unit_named(&name) {
            return Ok(h.sc_obj_is_effectively_dead(o));
        }
        Ok(h.sc_did_unit_exist(&name))
    }

    fn named_unit_exists<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize) -> R<bool> {
        Ok(match h.sc_unit_named(&s(c, up)?) {
            Some(o) => !h.sc_obj_is_effectively_dead(o),
            None => false,
        })
    }

    fn named_unit_dying<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize) -> R<bool> {
        let name = s(c, up)?;
        if let Some(o) = h.sc_unit_named(&name) {
            return Ok(h.sc_obj_is_effectively_dead(o));
        }
        // (upstream asks didUnitExist and answers false either way)
        let _ = h.sc_did_unit_exist(&name);
        Ok(false)
    }

    fn named_unit_totally_dead<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize) -> R<bool> {
        let name = s(c, up)?;
        if h.sc_unit_named(&name).is_some() {
            return Ok(false); // if the unit still exists, it isn't totally dead.
        }
        Ok(h.sc_did_unit_exist(&name) != plant(3842))
    }

    fn has_units<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize) -> R<bool> {
        let desired = s(c, tp)?;
        if desired == THIS_TEAM {
            return Ok(match h.sc_team_named(&desired) {
                Some(t) => h.sc_team_has_any_units(t),
                None => false,
            });
        }
        if let Some(this_team) = h.sc_team_named(THIS_TEAM) {
            if h.sc_team_name(this_team).str() == desired.as_slice() && !plant(3852) {
                return Ok(h.sc_team_has_any_units(this_team));
            }
        }
        if let Some(instances) = h.sc_team_prototype_instances(&desired) {
            for t in instances {
                if h.sc_team_has_any_units(t) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn team_inside_area_partially<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize, ap: usize, kp: usize) -> R<bool> {
        let team = h.sc_team_named(&s(c, tp)?);
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        if let Some(t) = team {
            let which = int(c, kp)? as u32;
            return Ok(h.sc_team_some_inside_some_outside(t, trig, which) || h.sc_team_all_inside(t, trig, which));
        }
        Ok(false)
    }

    fn named_inside_area<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize, ap: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        let p = h.sc_obj_position(o);
        let ic = ICoord3D { x: real_to_int(p.x)?, y: real_to_int(p.y)?, z: real_to_int(p.z)? };
        Ok(h.sc_trigger_point_in(trig, ic))
    }

    #[allow(clippy::too_many_arguments)]
    fn player_has_unit_type_in_area<H: ScriptConditionsHost>(
        &self,
        h: &H,
        c: &mut Condition,
        pp: usize,
        cp: usize,
        np: usize,
        typ: usize,
        ap: usize,
    ) -> R<bool> {
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let mut any_changes = c.custom_data == 0;
        if !any_changes && Self::any_team_entered_or_exited(h, p) {
            any_changes = true;
        }
        if h.sc_frame_object_count_changed() > c.custom_frame {
            any_changes = true; // Objects were added/deleted last frame, so count could have changed.
        }
        if !any_changes {
            if c.custom_data == -1 {
                return Ok(false);
            }
            if c.custom_data == 1 {
                return Ok(true);
            }
        }
        let types = self.object_types_from_param(h, c, typ)?;
        let mut count = 0i32;
        for o in Self::player_members(h, p) {
            if Self::types_contain_template(h, &types, h.sc_obj_template(o)) && h.sc_obj_is_inside(o, trig) {
                // dead objects will not be considered, except crates
                if !(h.sc_obj_is_effectively_dead(o) || h.sc_obj_is_kind_of(o, KINDOF_INERT)) || h.sc_obj_is_kind_of(o, KINDOF_CRATE) {
                    count += 1;
                }
            }
        }
        let comparison = compare(int(c, cp)?, count, int(c, np)?).unwrap_or(false);
        c.custom_data = if comparison { 1 } else { -1 };
        c.custom_frame = h.sc_frame_object_count_changed();
        Ok(comparison)
    }

    #[allow(clippy::too_many_arguments)]
    fn player_has_unit_kind_in_area<H: ScriptConditionsHost>(
        &self,
        h: &H,
        c: &mut Condition,
        pp: usize,
        cp: usize,
        np: usize,
        kp: usize,
        ap: usize,
    ) -> R<bool> {
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        let kind = int(c, kp)?;
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let mut any_changes = c.custom_data == 0;
        if !any_changes && Self::any_team_entered_or_exited(h, p) {
            any_changes = true;
        }
        if h.sc_frame_object_count_changed() > c.custom_frame {
            any_changes = true;
        }
        if !any_changes {
            if c.custom_data == -1 {
                return Ok(false);
            }
            if c.custom_data == 1 {
                return Ok(true);
            }
        }
        let mut count = 0i32;
        for o in Self::player_members(h, p) {
            if h.sc_obj_is_kind_of(o, kind)
                && h.sc_obj_is_inside(o, trig)
                && !(h.sc_obj_is_effectively_dead(o) || h.sc_obj_is_kind_of(o, KINDOF_INERT))
            {
                count += 1;
            }
        }
        let comparison = compare(int(c, cp)?, count, int(c, np)?).unwrap_or(false);
        // (upstream stores the object-count frame in the custom data, not the custom frame)
        if plant(3843) {
            c.custom_frame = h.sc_frame_object_count_changed();
        } else {
            c.custom_data = h.sc_frame_object_count_changed() as i32;
        }
        Ok(comparison)
    }

    fn team_state_is<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize, stp: usize, want: bool) -> R<bool> {
        let team = h.sc_team_named(&s(c, tp)?);
        let state = s(c, stp)?;
        Ok(match team {
            Some(t) => (h.sc_team_state(t).str() == state.as_slice()) == want,
            None => false,
        })
    }

    fn team_inside_area_entirely<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize, ap: usize, kp: usize) -> R<bool> {
        let team = h.sc_team_named(&s(c, tp)?);
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        Ok(match team {
            Some(t) => h.sc_team_all_inside(t, trig, int(c, kp)? as u32),
            None => false,
        })
    }

    fn named_attacked_by_type<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize, typ: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        if !h.sc_obj_has_body(o) {
            return Ok(false);
        }
        let Some(info) = h.sc_obj_last_damage(o) else { return Ok(false) };
        if let Some(attacker) = info.source_template {
            let types = self.object_types_from_param(h, c, typ)?;
            if Self::types_contain_template(h, &types, Some(attacker)) {
                return Ok(true);
            }
        } else {
            let attacker = h.sc_find_object(info.source_id);
            let Some(t) = attacker.and_then(|a| h.sc_obj_template(a)) else { return Ok(false) };
            let types = self.object_types_from_param(h, c, typ)?;
            if types.is_in_set(&h.sc_template_name(t)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn team_attacked_by_type<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize, typ: usize) -> R<bool> {
        let Some(team) = h.sc_team_named(&s(c, tp)?) else { return Ok(false) };
        let _outer = self.object_types_from_param(h, c, typ)?;
        for o in h.sc_team_members(team) {
            if !h.sc_obj_has_body(o) {
                continue;
            }
            let Some(info) = h.sc_obj_last_damage(o) else { continue };
            let types = self.object_types_from_param(h, c, typ)?;
            if let Some(attacker) = info.source_template {
                if Self::types_contain_template(h, &types, Some(attacker)) {
                    return Ok(true);
                }
            } else {
                let attacker = h.sc_find_object(info.source_id);
                let Some(t) = attacker.and_then(|a| h.sc_obj_template(a)) else { continue };
                if types.is_in_set(&h.sc_template_name(t)) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn named_attacked_by_player<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, up: usize, pp: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        if !h.sc_obj_has_body(o) {
            return Ok(false);
        }
        let Some(info) = h.sc_obj_last_damage(o) else { return Ok(false) };
        let attacker = h.sc_find_object(info.source_id);
        let mut player = None;
        if info.source_player_mask != 0 && !plant(3854) {
            player = h.sc_player_from_mask(info.source_player_mask);
        }
        if player.is_some() || attacker.is_some() {
            let victim = self.player_from_param(h, c, pp)?;
            if player == victim && !plant(3844) {
                return Ok(true);
            }
            let Some(a) = attacker else { return Ok(false) };
            return Ok(h.sc_obj_controlling_player(a) == victim);
        }
        Ok(false)
    }

    fn team_attacked_by_player<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, tp: usize, pp: usize) -> R<bool> {
        let Some(team) = h.sc_team_named(&s(c, tp)?) else { return Ok(false) };
        for o in h.sc_team_members(team) {
            if !h.sc_obj_has_body(o) {
                continue;
            }
            let Some(info) = h.sc_obj_last_damage(o) else { continue };
            let Some(a) = h.sc_find_object(info.source_id) else { continue };
            if h.sc_obj_controlling_player(a) == self.player_from_param(h, c, pp)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn built_by_player<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, typ: usize, pp: usize) -> R<bool> {
        if c.custom_data != 0 && h.sc_frame_object_count_changed() == c.custom_frame {
            // object count hasn't changed.  Use cached value.
            if c.custom_data == 1 {
                return Ok(true);
            }
            if c.custom_data == -1 {
                return Ok(false);
            }
        }
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        if h.sc_find_template(&ascii(&s(c, typ)?)).is_none() {
            return Ok(false);
        }
        let types = self.object_types_from_param(h, c, typ)?;
        let Some(sum) = Self::count_types(h, p, &types, false) else { return Ok(false) };
        c.custom_data = if sum != 0 { 1 } else { -1 };
        c.custom_frame = h.sc_frame_object_count_changed();
        Ok(sum != 0)
    }

    fn team_created<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize) -> R<bool> {
        Ok(match h.sc_team_named(&s(c, tp)?) {
            Some(t) => h.sc_team_is_created(t),
            None => false,
        })
    }

    fn unit_health<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize, cp: usize, hp: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        if !h.sc_obj_has_body(o) {
            return Ok(false);
        }
        let (cur, initial) = h.sc_obj_health(o);
        let cur_percent = real_to_int((cur * 100.0 + if plant(3851) { 0.0 } else { initial / 2.0 }) / initial)?;
        Ok(compare(int(c, cp)?, cur_percent, int(c, hp)?).unwrap_or(false))
    }

    fn player_has_credits<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, crp: usize, cp: usize, pp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        // (the parameter's Int against the money's UnsignedInt: compared unsigned)
        if plant(3850) {
            return Ok(compare(int(c, cp)?, int(c, crp)? as i64, i64::from(h.sc_player_money(p))).unwrap_or(false));
        }
        let credits = int(c, crp)? as u32;
        Ok(compare(int(c, cp)?, credits, h.sc_player_money(p)).unwrap_or(false))
    }

    fn building_entered<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, pp: usize, ip: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, ip)?) else { return Ok(false) };
        let Some(contain) = h.sc_obj_contain(o) else { return Ok(false) };
        let mask = contain.player_who_entered;
        if mask == 0 {
            return Ok(false);
        }
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        Ok(mask == h.sc_player_mask(p))
    }

    fn is_building_empty<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, ip: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, ip)?) else { return Ok(false) };
        let Some(contain) = h.sc_obj_contain(o) else { return Ok(false) };
        Ok(contain.count == 0)
    }

    fn enemy_sighted<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, ip: usize, ap: usize, pp: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, ip)?) else { return Ok(false) };
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let relation = match int(c, ap)? {
            relation::REL_NEUTRAL => ALLOW_NEUTRAL,
            relation::REL_FRIEND => ALLOW_ALLIES,
            relation::REL_ENEMY => {
                if plant(3845) {
                    ALLOW_ALLIES
                } else {
                    ALLOW_ENEMIES
                }
            }
            _ => return Err(IniError::undefined(Undefined::UninitialisedLocal)),
        };
        for them in h.sc_partition_seen_by(o, Some(relation)) {
            if h.sc_obj_controlling_player(them) == Some(p) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn type_sighted<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, ip: usize, typ: usize, pp: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, ip)?) else { return Ok(false) };
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let types = self.object_types_from_param(h, c, typ)?;
        for them in h.sc_partition_seen_by(o, None) {
            if h.sc_obj_controlling_player(them) == Some(p) {
                let t = h.sc_obj_template(them).ok_or(IniError::undefined(Undefined::NullObject))?;
                if types.is_in_set(&h.sc_template_name(t)) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Visible to the player: not held, not stealthed unless detected or disguised, and its shroud clear or partly clear.
    fn discovered_by<H: ScriptConditionsHost>(h: &H, o: ObjH, index: i32) -> Option<bool> {
        if h.sc_obj_is_disabled_by_type(o, DISABLED_HELD) {
            return None;
        }
        let st = h.sc_obj_status(o);
        if st.test(OBJECT_STATUS_STEALTHED) && !st.test(OBJECT_STATUS_DETECTED) && !st.test(OBJECT_STATUS_DISGUISED) {
            return None;
        }
        let shroud = h.sc_obj_shrouded_status(o, index);
        Some(shroud == ObjectShroudStatus::Clear || shroud == ObjectShroudStatus::PartialClear)
    }

    fn named_discovered<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, ip: usize, pp: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, ip)?) else { return Ok(false) };
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        Ok(Self::discovered_by(h, o, p).unwrap_or(false))
    }

    fn team_discovered<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, tp: usize, pp: usize) -> R<bool> {
        let Some(team) = h.sc_team_named(&s(c, tp)?) else { return Ok(false) };
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        for o in h.sc_team_members(team) {
            if Self::discovered_by(h, o, p) == Some(true) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn named_owned_by_player<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, up: usize, pp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        Ok(h.sc_obj_controlling_player(o) == Some(p))
    }

    fn team_owned_by_player<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, tp: usize, pp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let Some(t) = h.sc_team_named(&s(c, tp)?) else { return Ok(false) };
        Ok(h.sc_team_controlling_player(t) == Some(p))
    }

    fn player_has_n_or_fewer_buildings<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, np: usize, pp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        Ok(int(c, np)? >= h.sc_player_count_buildings(p))
    }

    fn player_has_n_or_fewer_faction_buildings<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, np: usize, pp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let mut mask = KindOfMaskType::new();
        mask.set(KINDOF_MP_COUNT_FOR_VICTORY, 1);
        mask.set(KINDOF_STRUCTURE, 1);
        Ok(int(c, np)? >= h.sc_player_count_objects(p, &mask, &KindOfMaskType::new()))
    }

    fn player_has_power<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, pp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        Ok(h.sc_player_has_sufficient_power(p))
    }

    fn on_path(labels: &[AsciiString; 3], path: &[u8]) -> bool {
        labels.iter().any(|l| l.str() == path)
    }

    fn named_reached_waypoints_end<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize, wp: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        let Some(Some(labels)) = h.sc_obj_completed_waypoint(o) else { return Ok(false) };
        Ok(Self::on_path(&labels, &s(c, wp)?))
    }

    fn team_reached_waypoints_end<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize, wp: usize) -> R<bool> {
        let Some(team) = h.sc_team_named(&s(c, tp)?) else { return Ok(false) };
        let path = s(c, wp)?;
        let mut any_at_end = false;
        for o in h.sc_team_members(team) {
            let Some(way) = h.sc_obj_completed_waypoint(o) else { continue };
            if let Some(labels) = way {
                if Self::on_path(&labels, &path) {
                    any_at_end = true;
                }
            }
        }
        Ok(any_at_end)
    }

    fn named_selected<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, up: usize) -> R<bool> {
        if h.sc_is_multiplayer_session() {
            return Ok(false);
        }
        let mut any_changes = c.custom_data == 0;
        if h.sc_frame_selection_changed() != c.custom_frame {
            any_changes = true; // Selection changed since we cached the value.
        }
        if !any_changes {
            if c.custom_data == -1 {
                return Ok(false);
            }
            if c.custom_data == 1 {
                return Ok(true);
            }
        }
        let name = s(c, up)?;
        let mut is_selected = false;
        for d in h.sc_selected_object_names() {
            let n = d.ok_or(IniError::undefined(Undefined::NullObject))?;
            if n.str() == name.as_slice() {
                is_selected = true;
                break;
            }
        }
        c.custom_data = if is_selected { 1 } else { -1 };
        c.custom_frame = h.sc_frame_selection_changed();
        Ok(is_selected)
    }

    /// The four "from unit" queries: the unit's id when one is named (a named unit that is gone answers false).
    fn source_id<H: ScriptConditionsHost>(h: &H, c: &Condition, up: Option<usize>) -> R<Option<ObjectId>> {
        match up {
            None => Ok(Some(0)),
            Some(i) => Ok(h.sc_unit_named(&s(c, i)?)),
        }
    }

    fn special_power_query<H: ScriptConditionsHost>(&self, h: &mut H, c: &mut Condition, pp: usize, sp: usize, up: Option<usize>, which: u8) -> R<bool> {
        let player = self.player_from_param(h, c, pp)?;
        let Some(source) = Self::source_id(h, c, up)? else { return Ok(false) };
        if let Some(p) = player {
            let name = s(c, sp)?;
            return Ok(match which {
                0 => h.sc_is_special_power_triggered(p, &name, true, source),
                1 => h.sc_is_special_power_midway(p, &name, true, source),
                2 => h.sc_is_special_power_complete(p, &name, true, source),
                _ => h.sc_is_upgrade_complete(p, &name, true, source),
            });
        }
        Ok(false)
    }

    fn science_acquired<H: ScriptConditionsHost>(&self, h: &mut H, c: &mut Condition, pp: usize, scp: usize) -> R<bool> {
        if let Some(p) = self.player_from_param(h, c, pp)? {
            let science = h.sc_science_from_name(&s(c, scp)?);
            if science == SCIENCE_INVALID {
                return Ok(false);
            }
            return Ok(h.sc_is_science_acquired(p, science, true));
        }
        Ok(false)
    }

    fn can_purchase_science<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, pp: usize, scp: usize) -> R<bool> {
        if let Some(p) = self.player_from_param(h, c, pp)? {
            let science = h.sc_science_from_name(&s(c, scp)?);
            if science == SCIENCE_INVALID {
                return Ok(false);
            }
            return Ok(h.sc_player_is_capable_of_purchasing_science(p, science));
        }
        Ok(false)
    }

    fn science_purchase_points<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, pp: usize, np: usize) -> R<bool> {
        if let Some(p) = self.player_from_param(h, c, pp)? {
            return Ok(h.sc_player_science_purchase_points(p) >= int(c, np)?);
        }
        Ok(false)
    }

    fn named_has_free_container_slots<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        Ok(match h.sc_obj_contain(o) {
            Some(contain) => contain.count < contain.max,
            None => false,
        })
    }

    fn named_entered_area<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize, ap: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        if h.sc_obj_is_kind_of(o, KINDOF_INERT) {
            return Ok(false);
        }
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        Ok(h.sc_obj_did_enter(o, trig))
    }

    fn named_exited_area<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize, ap: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        Ok(h.sc_obj_did_exit(o, trig))
    }

    /// The four team enter/exit questions (`which`: 0 all enter, 1 partial enter, 2 all exit, 3 partial exit).
    fn team_entered_exited<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize, ap: usize, kp: usize, which: u8) -> R<bool> {
        let Some(team) = h.sc_team_named(&s(c, tp)?) else { return Ok(false) };
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        let k = int(c, kp)? as u32;
        Ok(match which {
            0 => h.sc_team_did_all_enter(team, trig, k),
            1 => h.sc_team_did_partial_enter(team, trig, k),
            2 => h.sc_team_did_all_exit(team, trig, k),
            _ => h.sc_team_did_partial_exit(team, trig, k),
        })
    }

    fn player_unit_condition<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, pp: usize, cp: usize, np: usize, typ: usize) -> R<bool> {
        if c.custom_data != 0 && h.sc_frame_object_count_changed() == c.custom_frame {
            if c.custom_data == 1 {
                return Ok(true);
            }
            if c.custom_data == -1 {
                return Ok(false);
            }
        }
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let types = self.object_types_from_param(h, c, typ)?;
        let count = Self::count_types(h, p, &types, false).unwrap_or(0);
        let comparison = compare(int(c, cp)?, count, int(c, np)?).unwrap_or(false);
        c.custom_data = if comparison { 1 } else { -1 };
        c.custom_frame = h.sc_frame_object_count_changed();
        Ok(comparison)
    }

    fn percent_power<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, pp: usize, cp: usize, pcp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let ratio = h.sc_player_energy_supply_ratio(p);
        let test = int(c, pcp)? as f32 / 100.0f32;
        Ok(compare(int(c, cp)?, ratio, test).unwrap_or(false))
    }

    fn excess_power<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, pp: usize, cp: usize, kp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let desired = int(c, kp)?;
        let actual = h
            .sc_player_energy_production(p)
            .checked_sub(h.sc_player_energy_consumption(p))
            .ok_or(IniError::undefined(Undefined::SignedIntOverflow))?;
        Ok(compare(int(c, cp)?, actual, desired).unwrap_or(false))
    }

    fn skirmish_special_power_is_ready<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, pwp: usize) -> R<bool> {
        let v = int(c, pwp)?;
        if v == -1 {
            return Ok(false);
        }
        // (Int against UnsignedInt: compared unsigned)
        if v > 0 && (v as u32) > h.sc_frame() {
            return Ok(false);
        }
        let mut next_frame = h.sc_frame().wrapping_add((10 * LOGICFRAMES_PER_SECOND) as u32) as i32;
        let Some(power) = h.sc_find_special_power(&s(c, pwp)?) else {
            parm_mut(c, pwp)?.int = -1; // flag as never true.
            return Ok(false);
        };
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        for o in Self::player_members(h, p) {
            if h.sc_obj_status(o).test(OBJECT_STATUS_UNDER_CONSTRUCTION) || h.sc_obj_is_disabled(o) {
                continue; // can't fire if under construction or disabled.
            }
            if let Some(module) = h.sc_obj_special_power_module(o, power) {
                if !h.sc_can_use_special_power(o, power) {
                    continue;
                }
                if module.ready {
                    return Ok(true);
                }
                if module.ready_frame < next_frame as u32 && !plant(3846) {
                    next_frame = module.ready_frame as i32;
                }
            }
        }
        parm_mut(c, pwp)?.int = next_frame;
        Ok(false)
    }

    fn player_destroyed_n_or_more_buildings<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, pp: usize, op: usize) -> R<bool> {
        let _player = self.player_from_param(h, c, pp)?;
        let _opponent = self.player_from_param(h, c, op)?;
        Ok(false)
    }

    fn unit_has_emptied<H: ScriptConditionsHost>(&mut self, h: &H, c: &Condition, up: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        let idx = self.transport_statuses.iter().position(|st| st.obj_id == o);
        let num_peeps = h.sc_obj_contain(o).map(|c| c.count as i32).unwrap_or(0);
        let frame = h.sc_frame();
        let Some(i) = idx else {
            self.transport_statuses.insert(0, TransportStatus { obj_id: o, frame_number: frame, unit_count: num_peeps });
            return Ok(false);
        };
        let st = &mut self.transport_statuses[i];
        if (st.frame_number == frame.wrapping_sub(1) || plant(3849)) && st.unit_count > 0 && num_peeps == 0 {
            // don't update on this round, so that several calls in the same frame all see it.
            return Ok(true);
        }
        st.frame_number = frame;
        st.unit_count = num_peeps;
        Ok(false)
    }

    /// `evaluateTeamIsContained(pTeamParm, allContained)`.
    pub fn team_is_contained<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize, all_contained: bool) -> R<bool> {
        let Some(team) = h.sc_team_named(&s(c, tp)?) else { return Ok(false) };
        let mut any_considered = false;
        for o in h.sc_team_members(team) {
            // (an exiting object would count, but `isContained && ...` keeps it false)
            let is_contained = h.sc_obj_is_contained(o);
            if is_contained {
                if !all_contained {
                    return Ok(true);
                }
            } else if all_contained {
                return Ok(false);
            }
            any_considered = true;
        }
        Ok((any_considered || plant(3848)) && all_contained)
    }

    fn unit_has_object_status<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, up: usize, stp: usize) -> R<bool> {
        let Some(o) = h.sc_unit_named(&s(c, up)?) else { return Ok(false) };
        Ok(h.sc_obj_status(o).test_for_any(&parm(c, stp)?.object_status))
    }

    fn team_has_object_status<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize, stp: usize, entire: bool) -> R<bool> {
        let Some(team) = h.sc_team_named(&s(c, tp)?) else { return Ok(false) };
        for o in h.sc_team_members(team) {
            let has = h.sc_obj_status(o).test_for_any(&parm(c, stp)?.object_status);
            if entire && !has {
                return Ok(false);
            } else if !entire && has {
                return Ok(true);
            }
        }
        Ok(entire)
    }

    fn skirmish_value_in_area<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, cp: usize, mp: usize, ap: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        let mut any_changes = c.custom_data == 0;
        if !any_changes && Self::any_team_entered_or_exited(h, p) {
            any_changes = true;
        }
        if if plant(3853) { h.sc_frame_object_count_changed() > c.custom_frame } else { h.sc_frame_object_count_changed() != c.custom_frame } {
            any_changes = true;
        }
        if !any_changes {
            if c.custom_data == -1 {
                return Ok(false);
            }
            if c.custom_data == 1 {
                return Ok(true);
            }
        }
        let mut total_cost = 0i32;
        for o in Self::player_members(h, p) {
            if !h.sc_obj_is_kind_of(o, KINDOF_INERT) && h.sc_obj_is_inside(o, trig) && !h.sc_obj_is_effectively_dead(o) {
                let Some(t) = h.sc_obj_template(o) else { continue };
                total_cost = total_cost.wrapping_add(h.sc_template_build_cost(t));
            }
        }
        let comparison = compare(int(c, cp)?, total_cost, int(c, mp)?).unwrap_or(false);
        c.custom_data = if comparison { 1 } else { -1 };
        c.custom_frame = h.sc_frame_object_count_changed();
        Ok(comparison)
    }

    fn skirmish_player_is_faction<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, fp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        Ok(h.sc_player_side(p).str() == s(c, fp)?.as_slice())
    }

    fn skirmish_supplies_within<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, dp: usize, lp: usize, vp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let Some(trig) = h.sc_trigger_named(&s(c, lp)?) else { return Ok(false) };
        let center = h.sc_trigger_center(trig);
        let distance = h.sc_trigger_radius(trig) + parm(c, dp)?.real;
        let compare_to = parm(c, vp)?.real;
        let mut max_value = 0.0f32;
        for them in h.sc_partition_supply_structures(center, distance, p) {
            let Some(boxes) = h.sc_obj_warehouse_boxes(them) else { continue };
            let value = h.sc_player_supply_box_value(p).wrapping_mul(boxes) as f32;
            if value > max_value {
                max_value = value;
            }
        }
        Ok(max_value > compare_to)
    }

    fn skirmish_tech_building_within<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, dp: usize, lp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        if c.custom_data == 1 {
            return Ok(true);
        }
        if c.custom_data == -1 && !plant(3847) {
            return Ok(false);
        }
        let Some(trig) = h.sc_trigger_named(&s(c, lp)?) else { return Ok(false) };
        let center = h.sc_trigger_center(trig);
        let radius = h.sc_trigger_radius(trig) + parm(c, dp)?.real;
        let comparison = h.sc_partition_tech_building_near(center, radius, p);
        c.custom_data = if comparison { 1 } else { -1 };
        Ok(comparison)
    }

    /// `evaluateSkirmishCommandButtonIsReady(pSkirmishPlayerParm, pTeamParm, pCommandButtonParm, allReady)` (the player is not read).
    pub fn skirmish_command_button_is_ready<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, tp: usize, bp: usize, all_ready: bool) -> R<bool> {
        let Some(team) = h.sc_team_named(&s(c, tp)?) else { return Ok(false) };
        let Some(button) = h.sc_find_command_button(&s(c, bp)?) else { return Ok(false) };
        for o in h.sc_team_members(team) {
            if let Some(power) = h.sc_button_special_power(button) {
                if !h.sc_obj_has_special_power(o, h.sc_special_power_type(power)) {
                    continue;
                }
            } else if !h.sc_button_has_upgrade(button) {
                continue;
            }
            if h.sc_button_is_ready(button, o) {
                if !all_ready {
                    return Ok(true);
                }
            } else if all_ready {
                return Ok(false);
            }
        }
        Ok(all_ready)
    }

    fn skirmish_unowned_faction_unit_comparison<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, cp: usize, np: usize) -> R<bool> {
        let Some(p) = h.sc_neutral_player() else { return Ok(false) };
        let n = Self::player_members(h, p).into_iter().filter(|o| h.sc_obj_is_disabled_by_type(*o, DISABLED_UNMANNED)).count() as i32;
        Ok(compare(int(c, cp)?, n, int(c, np)?).unwrap_or(false))
    }

    fn skirmish_has_prereqs_to_build<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, typ: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let types = self.object_types_from_param(h, c, typ)?;
        Ok(types.can_build_any(|name| h.sc_find_template(name), |t| h.sc_player_can_build(p, t)))
    }

    fn skirmish_comparison_garrisoned<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, cp: usize, np: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let n = Self::player_members(h, p)
            .into_iter()
            .filter(|o| h.sc_obj_contain(*o).is_some_and(|ci| ci.garrisonable && ci.count > 0))
            .count() as i32;
        Ok(compare(int(c, cp)?, n, int(c, np)?).unwrap_or(false))
    }

    fn skirmish_comparison_captured<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, cp: usize, np: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let n = Self::player_members(h, p).into_iter().filter(|o| h.sc_obj_is_captured(*o)).count() as i32;
        Ok(compare(int(c, cp)?, n, int(c, np)?).unwrap_or(false))
    }

    fn skirmish_player_has_units_in_area<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, ap: usize) -> R<bool> {
        let Some(trig) = h.sc_trigger_named(&s(c, ap)?) else { return Ok(false) };
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let mut any_changes = c.custom_data == 0;
        if !any_changes && Self::any_team_entered_or_exited(h, p) {
            any_changes = true;
        }
        if h.sc_frame_object_count_changed() > c.custom_frame {
            any_changes = true;
        }
        if !any_changes {
            if c.custom_data == -1 {
                return Ok(false);
            }
            if c.custom_data == 1 {
                return Ok(true);
            }
        }
        let mut count = 0i32;
        for o in Self::player_members(h, p) {
            if h.sc_obj_is_inside(o, trig)
                && !(h.sc_obj_is_effectively_dead(o) || h.sc_obj_is_kind_of(o, KINDOF_INERT) || h.sc_obj_is_kind_of(o, KINDOF_PROJECTILE))
            {
                count += 1;
            }
        }
        let comparison = count > 0;
        c.custom_data = if comparison { 1 } else { -1 };
        c.custom_frame = h.sc_frame_object_count_changed();
        Ok(comparison)
    }

    fn skirmish_supply_source_safe<H: ScriptConditionsHost>(&self, h: &mut H, c: &mut Condition, sp: usize, mp: usize) -> R<bool> {
        // Trigger every 2*LOGICFRAMES_PER_SECOND.
        let any_changes = h.sc_frame() > c.custom_frame;
        if !any_changes {
            if c.custom_data == -1 {
                return Ok(false);
            }
            if c.custom_data == 1 {
                return Ok(true);
            }
        }
        c.custom_frame = h.sc_frame().wrapping_add((2 * LOGICFRAMES_PER_SECOND) as u32);
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let safe = h.sc_player_is_supply_source_safe(p, int(c, mp)?);
        c.custom_data = if safe { 1 } else { -1 };
        Ok(safe)
    }

    fn skirmish_supply_source_attacked<H: ScriptConditionsHost>(&self, h: &mut H, c: &mut Condition, sp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        Ok(h.sc_player_is_supply_source_attacked(p))
    }

    fn skirmish_start_position<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, np: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let ndx = int(c, np)?.wrapping_sub(1); // externally 1, 2, 3, internally 0, 1, 2.
        Ok(ndx == h.sc_player_mp_start_index(p))
    }

    fn skirmish_attacked_by_player<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, ap: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let Some(src) = self.player_from_param(h, c, ap)? else { return Ok(false) };
        Ok(h.sc_player_attacked_by(p, src))
    }

    fn skirmish_player_is_outside_area<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, ap: usize) -> R<bool> {
        if self.player_from_param(h, c, sp)?.is_none() {
            return Ok(false);
        }
        if h.sc_trigger_named(&s(c, ap)?).is_none() {
            return Ok(false);
        }
        Ok(!self.skirmish_player_has_units_in_area(h, c, sp, ap)?)
    }

    fn skirmish_has_discovered_player<H: ScriptConditionsHost>(&self, h: &H, c: &mut Condition, sp: usize, dp: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, sp)? else { return Ok(false) };
        let Some(by) = self.player_from_param(h, c, dp)? else { return Ok(false) };
        for o in Self::player_members(h, p) {
            let st = h.sc_obj_shrouded_status(o, by);
            if st == ObjectShroudStatus::Clear || st == ObjectShroudStatus::PartialClear {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn player_lost_object_type<H: ScriptConditionsHost>(&self, h: &mut H, c: &mut Condition, pp: usize, typ: usize) -> R<bool> {
        let Some(p) = self.player_from_param(h, c, pp)? else { return Ok(false) };
        let types = self.object_types_from_param(h, c, typ)?;
        let Some(sum) = Self::count_types(h, p, &types, true) else { return Ok(false) };
        let name = s(c, typ)?;
        let current = h.sc_object_count(p, &name);
        if sum != current {
            h.sc_set_object_count(p, &name, sum);
        }
        Ok(sum < current)
    }

    /// `evaluateCondition(pCondition)`.
    pub fn evaluate_condition<H: ScriptConditionsHost>(&mut self, h: &mut H, c: &mut Condition) -> R<bool> {
        use ct::*;
        match c.condition_type {
            PLAYER_ALL_DESTROYED => self.all_destroyed(h, c, 0),
            PLAYER_ALL_BUILDFACILITIES_DESTROYED => self.all_build_facilities_destroyed(h, c, 0),
            TEAM_INSIDE_AREA_PARTIALLY => self.team_inside_area_partially(h, c, 0, 1, 2),
            NAMED_INSIDE_AREA => self.named_inside_area(h, c, 0, 1),
            TEAM_DESTROYED => self.is_destroyed(h, c, 0),
            NAMED_DESTROYED => self.named_unit_destroyed(h, c, 0),
            NAMED_DYING => self.named_unit_dying(h, c, 0),
            NAMED_TOTALLY_DEAD => self.named_unit_totally_dead(h, c, 0),
            NAMED_NOT_DESTROYED => self.named_unit_exists(h, c, 0),
            TEAM_HAS_UNITS => self.has_units(h, c, 0),
            CAMERA_MOVEMENT_FINISHED => Ok(h.sc_camera_movement_finished()),
            TEAM_STATE_IS => self.team_state_is(h, c, 0, 1, true),
            TEAM_STATE_IS_NOT => self.team_state_is(h, c, 0, 1, false),
            NAMED_OUTSIDE_AREA => Ok(!self.named_inside_area(h, c, 0, 1)?),
            TEAM_INSIDE_AREA_ENTIRELY => self.team_inside_area_entirely(h, c, 0, 1, 2),
            TEAM_OUTSIDE_AREA_ENTIRELY => {
                Ok(!(self.team_inside_area_entirely(h, c, 0, 1, 2)? || self.team_inside_area_partially(h, c, 0, 1, 2)?))
            }
            NAMED_ATTACKED_BY_OBJECTTYPE => self.named_attacked_by_type(h, c, 0, 1),
            TEAM_ATTACKED_BY_OBJECTTYPE => self.team_attacked_by_type(h, c, 0, 1),
            NAMED_ATTACKED_BY_PLAYER => self.named_attacked_by_player(h, c, 0, 1),
            TEAM_ATTACKED_BY_PLAYER => self.team_attacked_by_player(h, c, 0, 1),
            BUILT_BY_PLAYER => self.built_by_player(h, c, 0, 1),
            NAMED_CREATED => Ok(h.sc_unit_named(&s(c, 0)?).is_some()),
            TEAM_CREATED => self.team_created(h, c, 0),
            PLAYER_HAS_CREDITS => self.player_has_credits(h, c, 0, 1, 2),
            BRIDGE_REPAIRED | BRIDGE_BROKEN => self.bridge(h, c, 0, c.condition_type == BRIDGE_BROKEN),
            NAMED_DISCOVERED => self.named_discovered(h, c, 0, 1),
            TEAM_DISCOVERED => self.team_discovered(h, c, 0, 1),
            BUILDING_ENTERED_BY_PLAYER => self.building_entered(h, c, 0, 1),
            ENEMY_SIGHTED => {
                if c.num_parms < 3 {
                    return Ok(false);
                }
                self.enemy_sighted(h, c, 0, 1, 2)
            }
            TYPE_SIGHTED => self.type_sighted(h, c, 0, 1, 2),
            NAMED_BUILDING_IS_EMPTY => self.is_building_empty(h, c, 0),
            MISSION_ATTEMPTS => Ok(false), // (a stub: the parameters are passed, never read)
            NAMED_OWNED_BY_PLAYER => self.named_owned_by_player(h, c, 0, 1),
            TEAM_OWNED_BY_PLAYER => self.team_owned_by_player(h, c, 0, 1),
            PLAYER_HAS_N_OR_FEWER_BUILDINGS => self.player_has_n_or_fewer_buildings(h, c, 1, 0),
            PLAYER_HAS_N_OR_FEWER_FACTION_BUILDINGS => self.player_has_n_or_fewer_faction_buildings(h, c, 1, 0),
            PLAYER_HAS_POWER => self.player_has_power(h, c, 0),
            PLAYER_HAS_NO_POWER => Ok(!self.player_has_power(h, c, 0)?),
            NAMED_REACHED_WAYPOINTS_END => self.named_reached_waypoints_end(h, c, 0, 1),
            TEAM_REACHED_WAYPOINTS_END => self.team_reached_waypoints_end(h, c, 0, 1),
            NAMED_SELECTED => self.named_selected(h, c, 0),
            NAMED_ENTERED_AREA => self.named_entered_area(h, c, 0, 1),
            NAMED_EXITED_AREA => self.named_exited_area(h, c, 0, 1),
            TEAM_ENTERED_AREA_ENTIRELY => self.team_entered_exited(h, c, 0, 1, 2, 0),
            TEAM_ENTERED_AREA_PARTIALLY => self.team_entered_exited(h, c, 0, 1, 2, 1),
            TEAM_EXITED_AREA_ENTIRELY => self.team_entered_exited(h, c, 0, 1, 2, 2),
            TEAM_EXITED_AREA_PARTIALLY => self.team_entered_exited(h, c, 0, 1, 2, 3),
            MULTIPLAYER_ALLIED_VICTORY => h.sc_is_local_allied_victory(),
            MULTIPLAYER_ALLIED_DEFEAT => h.sc_is_local_allied_defeat(),
            MULTIPLAYER_PLAYER_DEFEAT => Ok(h.sc_is_local_defeat() && !h.sc_is_local_allied_defeat()?),
            HAS_FINISHED_VIDEO => {
                let n = s(c, 0)?;
                Ok(h.sc_is_video_complete(&n, true))
            }
            HAS_FINISHED_SPEECH => {
                let n = s(c, 0)?;
                Ok(h.sc_is_speech_complete(&n, true))
            }
            HAS_FINISHED_AUDIO => {
                let n = s(c, 0)?;
                Ok(h.sc_is_audio_complete(&n, true))
            }
            UNIT_HEALTH => self.unit_health(h, c, 0, 1, 2),
            PLAYER_TRIGGERED_SPECIAL_POWER => self.special_power_query(h, c, 0, 1, None, 0),
            PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED => self.special_power_query(h, c, 0, 1, Some(2), 0),
            PLAYER_MIDWAY_SPECIAL_POWER => self.special_power_query(h, c, 0, 1, None, 1),
            PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED => self.special_power_query(h, c, 0, 1, Some(2), 1),
            PLAYER_COMPLETED_SPECIAL_POWER => self.special_power_query(h, c, 0, 1, None, 2),
            PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED => self.special_power_query(h, c, 0, 1, Some(2), 2),
            PLAYER_ACQUIRED_SCIENCE => self.science_acquired(h, c, 0, 1),
            PLAYER_CAN_PURCHASE_SCIENCE => self.can_purchase_science(h, c, 0, 1),
            PLAYER_HAS_SCIENCEPURCHASEPOINTS => self.science_purchase_points(h, c, 0, 1),
            NAMED_HAS_FREE_CONTAINER_SLOTS => self.named_has_free_container_slots(h, c, 0),
            DEFUNCT_PLAYER_SELECTED_GENERAL | DEFUNCT_PLAYER_SELECTED_GENERAL_FROM_NAMED => Ok(false),
            PLAYER_BUILT_UPGRADE => self.special_power_query(h, c, 0, 1, None, 3),
            PLAYER_BUILT_UPGRADE_FROM_NAMED => self.special_power_query(h, c, 0, 1, Some(2), 3),
            PLAYER_HAS_OBJECT_COMPARISON => self.player_unit_condition(h, c, 0, 1, 2, 3),
            PLAYER_DESTROYED_N_BUILDINGS_PLAYER => self.player_destroyed_n_or_more_buildings(h, c, 0, 2),
            PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA => {
                if c.num_parms < 5 {
                    return Ok(false);
                }
                self.player_has_unit_type_in_area(h, c, 0, 1, 2, 3, 4)
            }
            PLAYER_HAS_COMPARISON_UNIT_KIND_IN_TRIGGER_AREA => {
                if c.num_parms < 5 {
                    return Ok(false);
                }
                self.player_has_unit_kind_in_area(h, c, 0, 1, 2, 3, 4)
            }
            UNIT_EMPTIED => self.unit_has_emptied(h, c, 0),
            PLAYER_POWER_COMPARE_PERCENT => self.percent_power(h, c, 0, 1, 2),
            PLAYER_EXCESS_POWER_COMPARE_VALUE => self.excess_power(h, c, 0, 1, 2),
            SKIRMISH_SPECIAL_POWER_READY => self.skirmish_special_power_is_ready(h, c, 0, 1),
            UNIT_HAS_OBJECT_STATUS => self.unit_has_object_status(h, c, 0, 1),
            TEAM_ALL_HAS_OBJECT_STATUS => self.team_has_object_status(h, c, 0, 1, true),
            TEAM_SOME_HAVE_OBJECT_STATUS => self.team_has_object_status(h, c, 0, 1, false),
            SKIRMISH_VALUE_IN_AREA => self.skirmish_value_in_area(h, c, 0, 1, 2, 3),
            SKIRMISH_PLAYER_FACTION => self.skirmish_player_is_faction(h, c, 0, 1),
            SKIRMISH_SUPPLIES_VALUE_WITHIN_DISTANCE => self.skirmish_supplies_within(h, c, 0, 1, 2, 3),
            SKIRMISH_TECH_BUILDING_WITHIN_DISTANCE => self.skirmish_tech_building_within(h, c, 0, 1, 2),
            SKIRMISH_COMMAND_BUTTON_READY_ALL => self.skirmish_command_button_is_ready(h, c, 1, 2, true),
            SKIRMISH_COMMAND_BUTTON_READY_PARTIAL => self.skirmish_command_button_is_ready(h, c, 1, 2, false),
            SKIRMISH_UNOWNED_FACTION_UNIT_EXISTS => self.skirmish_unowned_faction_unit_comparison(h, c, 1, 2),
            SKIRMISH_PLAYER_HAS_PREREQUISITE_TO_BUILD => self.skirmish_has_prereqs_to_build(h, c, 0, 1),
            SKIRMISH_PLAYER_HAS_COMPARISON_GARRISONED => self.skirmish_comparison_garrisoned(h, c, 0, 1, 2),
            SKIRMISH_PLAYER_HAS_COMPARISON_CAPTURED_UNITS => self.skirmish_comparison_captured(h, c, 0, 1, 2),
            SKIRMISH_NAMED_AREA_EXIST => Ok(h.sc_trigger_named(&s(c, 1)?).is_some()),
            SKIRMISH_PLAYER_HAS_UNITS_IN_AREA => self.skirmish_player_has_units_in_area(h, c, 0, 1),
            SKIRMISH_PLAYER_HAS_BEEN_ATTACKED_BY_PLAYER => self.skirmish_attacked_by_player(h, c, 0, 1),
            SKIRMISH_PLAYER_IS_OUTSIDE_AREA => self.skirmish_player_is_outside_area(h, c, 0, 1),
            SKIRMISH_PLAYER_HAS_DISCOVERED_PLAYER => self.skirmish_has_discovered_player(h, c, 0, 1),
            MUSIC_TRACK_HAS_COMPLETED => {
                let n = s(c, 0)?;
                Ok(h.sc_music_track_completed(&n, int(c, 1)?))
            }
            SUPPLY_SOURCE_SAFE => self.skirmish_supply_source_safe(h, c, 0, 1),
            SUPPLY_SOURCE_ATTACKED => self.skirmish_supply_source_attacked(h, c, 0),
            START_POSITION_IS => self.skirmish_start_position(h, c, 0, 1),
            PLAYER_LOST_OBJECT_TYPE => self.player_lost_object_type(h, c, 0, 1),
            _ => Ok(false),
        }
    }

    /// `evaluateBridgeBroken` / `evaluateBridgeRepaired`: nothing to check while no bridge changed its damage state.
    fn bridge<H: ScriptConditionsHost>(&self, h: &H, c: &Condition, bp: usize, broken: bool) -> R<bool> {
        if !h.sc_any_bridges_damage_states_changed() {
            return Ok(false);
        }
        let Some(o) = h.sc_unit_named(&s(c, bp)?) else { return Ok(false) };
        Ok(if broken { h.sc_is_bridge_broken(o) } else { h.sc_is_bridge_repaired(o) })
    }
}

/// `Real` to `Int` (truncation); a value an `Int` cannot hold is undefined.
fn real_to_int(v: f32) -> R<i32> {
    // (NaN is in no range)
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&v) {
        return Err(IniError::undefined(Undefined::FloatToIntOutOfRange));
    }
    Ok(v as i32)
}
