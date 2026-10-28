// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIStateMachine; AIIdleState, AIDeadState, AIWaitState)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIStateMachine.h (the state classes; AIBusyState)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (makeStateMachine, onObjectCreated, update)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's state machine (`AIStateMachine`) and its states, and the AI's update (`p1w13-sonnet`, layer 10, batch A4).
//!
//! # The shape
//!
//! The machine is a [`StateMachine`] of [`AiState`] (one variant per state class), as the turret's is. **The AI's state is lent to
//! the host while the machine runs** ([`AiStateHost::ai_stack`], the innermost last; [`AiUpdateModule::run`]), so that the states,
//! which upstream reach the AI through `getMachineOwner()->getAI()`, reach it through the context; an AI member that the states
//! call runs on the lent AI ([`with_ai`]). `AIStateMachine`'s own members (the goal path, the goal waypoint, the temporary state)
//! live with the AI ([`AiMachineExtras`]), and its overrides of `setState`, `clear` and `resetToDefaultState` (which tell the AI
//! that its machine changed) are the `aism_*` functions, which the states call for `getMachine()->setState(..)`.
//!
//! **The states not yet translated** are [`AiState::Untranslated`]: their callbacks are the host's (the cut point), and their
//! `isIdle`/`isAttack`/`isBusy` the static answers of their classes ([`untranslated_flags`]: an attack state's `TRUE`; a state
//! whose answer is its inner machine's answers `false` until it is translated).
//!
//! # Upstream facts kept
//!
//! - The machine is made by `onObjectCreated`, not by the constructor ("makeStateMachine is a protected virtual func"), and it is
//!   the AI's before its default state is entered (the idle state's `onEnter` asks the AI).
//! - `AIIdleState`'s constructor sets its sleep offset to `-1` (65535 in its `UnsignedShort`); its `onEnter` draws a random offset;
//!   its `doInitIdleState` runs in the first update ("onEnter for idle is called during object creation"); `NO_STOP_AND_SLIDE` is
//!   defined, so the slide to the goal is not; a mood target is attacked through `aiAttackObject` (a command).
//! - `setTemporaryState` of an id the machine does not have gets the default state (`internalGetState`'s recovery).
//! - The temporary state's frame limit is clamped to one minute.
//! - `AIStateMachine::xfer`, on load, APPENDS the saved goal path to the current one, and keeps the current temporary state when
//!   the stream has none (then reads that state's snapshot from the stream).
//! - The machine's update is the temporary state's alone while it continues within its frames; the AI's `update` sleeps for the
//!   shortest of the machine's, the path queue's, the turrets' and `doLocomotor`'s sleeps, and not at all once dead.
//!
//! The goal squad is `logic::world::player`'s `Squad` (batch A5: `setGoalTeam`, `ai_commands::aism_set_goal_team`); the goal
//! waypoint is by id (`xfer`'s name lookups are the caller's).
//!
//! # Verification
//!
//! `tests/differential_ai_update.rs` (batch A4's operations beside A2's and A3's), against the REAL `AIStateMachine` (AIStates.cpp's
//! machine members, idle, dead and wait states cut verbatim, the untranslated states a logged placeholder) on `StateMachine.cpp`
//! unchanged, and AIUpdate.cpp's `update`, `onObjectCreated` and `makeStateMachine` verbatim: 10,000 programs, 600,000
//! operations, 0 mismatches (2026-10-27; plants 3240-3247 caught). See the test's results.

use std::convert::Infallible;
use std::marker::PhantomData;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{DISABLED_EMP, DISABLED_HACKED, DISABLED_PARALYZED, DISABLED_SUBDUED, DISABLED_UNMANNED};
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID};
use crate::common::kind_of::{KindOfType, KINDOF_CAN_BE_REPULSED, KINDOF_INFANTRY};
use crate::common::model_state::*;
use crate::common::state_machine::{
    SmResult, StateBehavior, StateContext, StateExitType, StateId, StateMachine, StateReturn, INVALID_STATE_ID,
};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferVersion};
use crate::logic::ai::ai_commands::{self as commands, AiCommandParms};
use crate::logic::ai::ai_move_states::{self as move_states, MoveToData};
use crate::logic::ai::ai_waypoint_states::{self as waypoint_states, WaypointPathData};
use crate::logic::ai::ai_attack_states::{self as attack_states, AttackFrame, AttackOwnerKinds, ChaseData};
use crate::logic::ai::ai_attack_move as attack_move;
use crate::logic::ai::ai_hunt_states as hunt_states;
use crate::logic::ai::ai_face_exit_states as face_exit;
use crate::logic::ai::ai_enter_state as enter_state;
use crate::logic::ai::ai_rappel_state::{self as rappel, RappelData};
use crate::logic::ai::ai_update::{AiHost, AiMachineExtras, AiUpdate};
use crate::logic::ai::path::Path;
use crate::common::disabled_types::DisabledType;
use crate::logic::ai::ai_update_data::AiUpdateModuleData;
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::module::update_module::{update_sleep, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::plant;
use crate::logic::weapon::NO_MAX_SHOTS_LIMIT;
use crate::logic::world::player::Squad;
use crate::logic::world::player_squads::squad_xfer;
use crate::common::game_common::{AbleToAttackType, Relationship};
use crate::common::game_type::WeaponSlotType;
use crate::common::kind_of::{KINDOF_CAN_ATTACK, KINDOF_IMMOBILE, KINDOF_PORTABLE_STRUCTURE};
use crate::common::object_status_types::ObjectStatusTypes;
use crate::common::state_machine::UtilityState;
use crate::logic::weapon_set_state::WeaponChoiceCriteria;
use crate::logic::weapon_types::WeaponStatus;
use std::sync::Arc;

/// `IDLE_COUNTDOWN_DELAY`: "reset the idle countdown so that we don't do expensive checks too often".
pub const IDLE_COUNTDOWN_DELAY: u32 = LOGICFRAMES_PER_SECOND as u32 * 2;

/// What the machine's states, the AI's update and its commands reach beyond the AI's members: the AI's lent state, the
/// pathfinder's goal bookkeeping, the turrets' updates (until the turrets' AIs are held by the AI), the object's repulsors, model
/// condition and the script engine, the states not yet translated, and (batch A5) what the commands hand over to.
pub trait AiStateHost: AiHost + StateContext {
    /// The AIs whose machines are running, the innermost last.
    fn ai_stack(&mut self) -> &mut Vec<AiUpdate>;
    /// `TheAI->pathfinder()->goalPosition(obj, &pos)`: the position, `None` for false.
    fn pf_goal_position(&mut self) -> Option<Coord3D>;
    /// `TheAI->pathfinder()->snapPosition(obj, &pos)`.
    fn pf_snap_position(&mut self, pos: &mut Coord3D);
    /// `TheAI->pathfinder()->updateGoal(obj, &pos, layer)`.
    fn pf_update_goal(&mut self, pos: &Coord3D, layer: PathfindLayer);
    /// `TheAI->pathfinder()->queueForPath(obj->getID())`.
    fn pf_queue_for_path(&mut self);
    /// `m_turretAI[which]->updateTurretAI()`.
    fn turret_update(&mut self, which: usize) -> UpdateSleepTime;
    /// `TheAI->findClosestRepulsor(obj, range)`'s id (`INVALID_ID`: none).
    fn find_closest_repulsor(&mut self, range: f32) -> ObjectId;
    /// `obj->getVisionRange()`.
    fn object_vision_range(&self) -> f32;
    /// `obj->clearAndSetModelConditionFlags(clr, set)`.
    fn clear_and_set_model_condition_flags(&mut self, clear: &[ModelConditionFlagType], set: ModelConditionFlagType);
    /// `TheScriptEngine->notifyOfObjectCreationOrDestruction()`.
    fn notify_of_object_creation_or_destruction(&mut self);
    // ---- batch A5: the commands
    /// `getObject()->isMobile()`.
    fn is_mobile(&self) -> bool;
    /// `TheGlobalData->m_partitionCellSize`.
    fn partition_cell_size(&self) -> f32;
    /// `getObject()->isSignificantlyAboveTerrain()` (Thing's).
    fn is_significantly_above_terrain(&mut self) -> bool;
    /// `TheTerrainLogic->getExtent(&region)`: its low and high corners.
    fn terrain_extent(&self) -> (Coord3D, Coord3D);
    /// `TheTerrainLogic->findClosestEdgePoint(pos)`.
    fn terrain_find_closest_edge_point(&mut self, pos: &Coord3D) -> Coord3D;
    /// The team's members (`iterate_TeamMemberList`), in its list's order.
    fn team_member_ids(&mut self, team: u32) -> Vec<ObjectId>;
    /// `TheActionManager->canGetHealedAt(obj, depot, cmdSource)`.
    fn action_can_get_healed_at(&mut self, depot: ObjectId, src: CommandSourceType) -> bool;
    /// `TheActionManager->canGetRepairedAt(obj, depot, cmdSource)`.
    fn action_can_get_repaired_at(&mut self, depot: ObjectId, src: CommandSourceType) -> bool;
    /// `TheActionManager->canEnterObject(obj, target, cmdSource, DONT_CHECK_CAPACITY)`.
    fn action_can_enter_object(&mut self, target: ObjectId, src: CommandSourceType) -> bool;
    /// `findObjectByID(id)->isDisabledByType(type)` of an object of the scene.
    fn object_is_disabled_by_type(&self, id: ObjectId, t: DisabledType) -> bool;
    /// `findObjectByID(id)->getAI()->getPath() != NULL` (false without an AI).
    fn object_ai_has_path(&self, id: ObjectId) -> bool;
    /// `TheAI->pathfinder()->getMoveAwayFromPath(obj, unit, unitPath, obj2, path2)`.
    fn pf_move_away_from_path(&mut self, unit: ObjectId, other: ObjectId, other_has_path: bool) -> Option<Path>;
    /// `TheAI->pathfinder()->moveAllies(obj, path)`.
    fn pf_move_allies(&mut self, path: &Path);
    /// `TheAI->pathfinder()->clientSafeQuickDoesPathExist(m_locomotorSet, myPos, destination)` (`isPathAvailable`).
    fn is_path_available(&mut self, destination: &Coord3D) -> bool;
    /// `getObject()->getCurrentWeapon()->setMaxShotCount(n)`, if it has one.
    fn weapon_set_max_shot_count(&mut self, n: i32);
    /// The current weapon's `getContinueAttackRange()` (0 without one).
    fn weapon_continue_attack_range(&self) -> f32;
    /// The current weapon's `isContactWeapon()` (false without one).
    fn weapon_is_contact(&self) -> bool;
    /// `ThePartitionManager->getClosestObject(pos, range, FROM_CENTER_2D, {possible to attack (ATTACK_NEW_TARGET, src), same map
    /// status})`'s id.
    fn partition_closest_attack_target(&mut self, pos: &Coord3D, range: f32, src: CommandSourceType) -> ObjectId;
    /// `ThePartitionManager->findPositionAround(pos, {minRadius, maxRadius, sourceToPathToDest: the object}, &result)`.
    fn find_position_around(&mut self, pos: &Coord3D, min_radius: f32, max_radius: f32) -> Option<Coord3D>;
    /// `getObject()->getContain() != NULL`.
    fn has_contain(&self) -> bool;
    /// `privateIdle`'s loop: `aiIdle(cmdSource)` of every contained object with an AI.
    fn contain_idle_passengers(&mut self, src: CommandSourceType);
    /// `getContain()->removeAllContained(FALSE)`.
    fn contain_remove_all_contained(&mut self);
    /// `getContain()->markAllPassengersDetected()`.
    fn contain_mark_all_passengers_detected(&mut self);
    /// `getContain()->orderAllPassengersToExit(cmdSource, instantly)`.
    fn contain_order_all_passengers_to_exit(&mut self, src: CommandSourceType, instantly: bool);
    /// A `DeliverPayloadAIUpdate`'s `deliverPayloadViaModuleData(pos)`: false if the object has none.
    fn deliver_payload_via_module_data(&mut self, pos: &Coord3D) -> bool;
    /// A `ProneUpdate`'s `goProne(damageInfo)`, if the object has one.
    fn go_prone(&mut self);
    /// A `HackInternetAIUpdate`'s `hackInternet()`, if the object has one (else a `DEBUG_CRASH`).
    fn hack_internet(&mut self);
    /// The polygon trigger's `getCenterPoint`.
    fn polygon_center(&mut self, polygon: i32) -> Coord3D;
    /// `TheControlBar->findCommandSet(getCommandSetString())`: how many of its slots hold the button (0 without a set), and the
    /// button's `getCommandType()`.
    fn command_set_button_matches(&mut self, button: u32) -> (u32, i32);
    // ---- batch A6: the paths
    /// The waypoint's `getLocation()`.
    fn waypoint_location(&mut self, way: u32) -> Coord3D;
    /// The waypoint's `getLink(0)`.
    fn waypoint_link0(&mut self, way: u32) -> Option<u32>;
    /// `TheAI->pathfinder()->getAircraftPath(obj, destination)`.
    fn pf_get_aircraft_path(&mut self, destination: &Coord3D) -> Option<Path>;
    /// `TheTerrainLogic->getMaximumPathfindExtent(&extent)`: its low and high corners.
    fn terrain_max_pathfind_extent(&mut self) -> (Coord3D, Coord3D);
    /// `TheAI->pathfinder()->moveAlliesAwayFromDestination(obj, destination)`.
    fn pf_move_allies_away_from_destination(&mut self, destination: &Coord3D);
    /// `TheAI->pathfinder()->setIgnoreObstacleID(id)`.
    fn pf_set_ignore_obstacle_id(&mut self, id: ObjectId);
    /// `pathServices->patchPath(obj, locomotorSet, path, blocked)`.
    fn pf_patch_path(&mut self, path: Option<&Path>, blocked: bool) -> Option<Path>;
    /// `pathServices->findPath(obj, locomotorSet, obj's position, destination)`.
    fn pf_find_path(&mut self, destination: &Coord3D) -> Option<Path>;
    /// `pathServices->findClosestPath(obj, locomotorSet, obj's position, destination, blocked, pathCostMultiplier, moveAllies)`.
    fn pf_find_closest_path(&mut self, destination: &Coord3D, blocked: bool, path_cost_multiplier: f32, move_allies: bool) -> Option<Path>;
    /// `pathServices->findSafePath(obj, locomotorSet, obj's position, repulsorPos1, repulsorPos2, repulsorRadius)`.
    fn pf_find_safe_path(&mut self, repulsor_pos1: &Coord3D, repulsor_pos2: &Coord3D, radius: f32) -> Option<Path>;
    /// `pathServices->findAttackPath(obj, locomotorSet, obj's position, victim, victimPos, weapon)`.
    fn pf_find_attack_path(&mut self, victim: ObjectId, victim_pos: &Coord3D) -> Option<Path>;
    /// `TheAI->pathfinder()->adjustToPossibleDestination(obj, locomotorSet, pos)`.
    fn pf_adjust_to_possible_destination(&mut self, pos: &mut Coord3D);
    /// `TheAI->pathfinder()->adjustDestination(obj, locomotorSet, pos)`.
    fn pf_adjust_destination(&mut self, pos: &mut Coord3D) -> bool;
    /// `TheAI->pathfinder()->clientSafeQuickDoesPathExistForUI(locomotorSet, obj's position, destination)`.
    fn pf_quick_path_exists_for_ui(&mut self, destination: &Coord3D) -> bool;
    /// `TheAI->pathfinder()->isAttackViewBlockedByObstacle(obj, obj's position, victim (none: `INVALID_ID`), victimPos)`.
    fn pf_is_attack_view_blocked(&mut self, victim: ObjectId, victim_pos: &Coord3D) -> bool;
    /// `TheAI->getAiData()->m_repulsedDistance`.
    fn ai_data_repulsed_distance(&self) -> f32;
    /// The current weapon's `isWithinAttackRange(obj, victim)`.
    fn weapon_is_within_attack_range_of_object(&mut self, victim: ObjectId) -> bool;
    /// The current weapon's `isWithinAttackRange(obj, pos)`.
    fn weapon_is_within_attack_range_of_position(&mut self, pos: &Coord3D) -> bool;
    /// The current weapon's `computeApproachTarget(obj, victim, pos, 0, result)`'s result.
    fn weapon_compute_approach_target(&mut self, victim: ObjectId, pos: &Coord3D) -> Coord3D;
    /// The current weapon's `isGoalPosWithinAttackRange(obj, goal, victim, victimPos)`.
    fn weapon_is_goal_pos_within_attack_range(&mut self, goal: &Coord3D, victim: ObjectId, victim_pos: &Coord3D) -> bool;
    /// Another object's `isSignificantlyAboveTerrain()`.
    fn object_is_significantly_above_terrain(&mut self, id: ObjectId) -> bool;
    /// Another object's `isKindOf(kind)`.
    fn object_is_kind_of(&self, id: ObjectId, kind: KindOfType) -> bool;
    /// `TheTerrainLogic->getBridgeAttackPoints(bridge, &info)`: its two attack points.
    fn bridge_attack_points(&mut self, bridge: ObjectId) -> (Coord3D, Coord3D);
    /// `ThePartitionManager->getDistanceSquared(obj, pos, FROM_BOUNDINGSPHERE_3D)`.
    fn distance_squared_bounding_sphere_3d(&mut self, pos: &Coord3D) -> f32;
    // ---- batch A7: the move states
    /// `TheAI->pathfinder()->getCell(layer, x, y)`'s type and pinch (`None`: no cell).
    fn pf_cell_at(&mut self, layer: PathfindLayer, x: i32, y: i32) -> Option<(u8, bool)>;
    /// `TheAI->pathfinder()->snapClosestGoalPosition(obj, pos)`.
    fn pf_snap_closest_goal_position(&mut self, pos: &mut Coord3D);
    /// `TheAI->pathfinder()->removeGoal(obj)`.
    fn pf_remove_goal(&mut self);
    /// `startMoveSound()`'s sounds (the template's move start sound, damaged or not, else its move loop): the loop's handle, if
    /// one was started.
    fn start_move_sound(&mut self, damaged: bool) -> Option<u32>;
    /// `TheAudio->removeAudioEvent(handle)`.
    fn remove_audio_event(&mut self, handle: u32);
    /// `getObject()->getFormationID()` (`NO_FORMATION_ID` 0).
    fn formation_id(&self) -> i32;
    /// `getAI()->getGroup()->getSpeed()`, if the AI has a group.
    fn group_speed(&mut self) -> Option<f32>;
    /// Another object's `getGeometryInfo().getMaxHeightAbovePosition()`.
    fn object_max_height_above_position(&self, id: ObjectId) -> f32;
    /// Another object's `getPhysics() != NULL`.
    fn object_has_physics(&self, id: ObjectId) -> bool;
    /// Another object's `getPhysics()->getVelocityMagnitude()`.
    fn object_velocity_magnitude(&self, id: ObjectId) -> f32;
    /// Another object's `getUnitDirectionVector3D(dir)`.
    fn object_unit_direction_vector_3d(&mut self, id: ObjectId) -> Coord3D;
    // ---- batch A8
    /// `getObject()->getTeam()->setActive()`.
    fn team_set_active(&mut self);
    /// `TheGameLogic->destroyObject(getObject())`.
    fn destroy_object(&mut self);
    // ---- batch A9
    /// `way->getNumLinks()`.
    fn waypoint_num_links(&mut self, way: u32) -> i32;
    /// `way->getLink(which)` (none out of range).
    fn waypoint_link(&mut self, way: u32, which: i32) -> Option<u32>;
    /// `TheAI->pathfinder()->isPointOnWall(pos)`.
    fn pf_is_point_on_wall(&mut self, pos: &Coord3D) -> bool;
    /// `TheAI->pathfinder()->getWallHeight()`.
    fn pf_wall_height(&self) -> f32;
    /// The AI's group's `getCenter` (asked only when the AI has a group).
    fn group_center(&mut self) -> Coord3D;
    /// `getTeam()->getCurrentWaypoint()`.
    fn team_current_waypoint(&mut self) -> Option<u32>;
    /// `getTeam()->setCurrentWaypoint(way)`.
    fn team_set_current_waypoint(&mut self, way: Option<u32>);
    /// `getControllingPlayer()->isSkirmishAIPlayer()` (upstream reads the player unchecked).
    fn is_skirmish_ai_player(&mut self) -> bool;
    /// `TheAI->createGroup()` filled by `getTeam()->getTeamAsAIGroup(group)`: its `getCenter` and `getCount`.
    fn team_group_center_and_count(&mut self) -> (Coord3D, i32);
    /// `TheAI->getAiData()->m_skirmishGroupFudgeValue`.
    fn ai_data_skirmish_group_fudge_value(&self) -> f32;
    // ---- batch A10a: the attack machine (see `ai_attack_states`; the fire state's `FireHost` is answered from these)
    /// The attack frames lent while attack machines run, the innermost last.
    fn attack_stack(&mut self) -> &mut Vec<AttackFrame>;
    /// `obj->getContainedBy()` of another object.
    fn object_contained_by(&self, id: ObjectId) -> ObjectId;
    /// `TheAI->getAiData()->m_aiCrushesInfantry`.
    fn ai_data_crushes_infantry(&self) -> bool;
    /// `canCrushOrSquish(victim)`.
    fn can_crush_or_squish(&mut self, victim: ObjectId) -> bool;
    /// `getCurrentWeapon(&slot)`: the slot, none without a current weapon.
    fn current_weapon_slot(&self) -> Option<WeaponSlotType>;
    /// `getWeaponInWeaponSlot(slot) != NULL`.
    fn weapon_slot_present(&self, slot: WeaponSlotType) -> bool;
    /// Another object's `isAirborneTarget()`.
    fn object_is_airborne_target(&mut self, id: ObjectId) -> bool;
    /// `isAbleToAttack()`.
    fn is_able_to_attack(&self) -> bool;
    /// `isOutOfAmmo()`.
    fn is_out_of_ammo(&self) -> bool;
    /// `isCurWeaponLocked()`.
    fn is_cur_weapon_locked(&self) -> bool;
    /// `hasAnyWeapon()`.
    fn has_any_weapon(&self) -> bool;
    /// `m_turretAI[which]->isOwnersCurWeaponOnTurret()`.
    fn turret_holds_current_weapon(&self, which: usize) -> bool;
    /// `m_turretAI[which]->setTurretTargetObject(o, forceAttacking)`.
    fn turret_set_target_object(&mut self, which: usize, o: Option<ObjectId>, force_attacking: bool);
    /// `m_turretAI[which]->setTurretTargetPosition(pos)`.
    fn turret_set_target_position(&mut self, which: usize, pos: Option<&Coord3D>);
    /// `m_turretAI[which]->getTurnRate()`.
    fn turret_turn_rate(&self, which: usize) -> f32;
    /// `chooseBestWeaponForTarget(target, criteria, cmdSource)`.
    fn choose_best_weapon_for_target(&mut self, target: Option<ObjectId>, criteria: WeaponChoiceCriteria, src: CommandSourceType) -> bool;
    /// `adjustModelConditionForWeaponStatus()`.
    fn adjust_model_condition_for_weapon_status(&mut self);
    /// `clearLeechRangeModeForAllWeapons()`.
    fn clear_leech_range_mode_for_all_weapons(&mut self);
    /// Another object's `isEffectivelyDead()`.
    fn object_is_effectively_dead(&self, id: ObjectId) -> bool;
    /// Another object's `isDestroyed()`.
    fn object_is_destroyed(&mut self, id: ObjectId) -> bool;
    /// Another object's team (`getTeam()`), none without one.
    fn object_team(&self, id: ObjectId) -> Option<u32>;
    /// Another object's `testStatus(bit)`.
    fn object_test_status(&self, id: ObjectId, bit: ObjectStatusTypes) -> bool;
    /// Another object's `getContain() != NULL`.
    fn object_has_contain(&self, id: ObjectId) -> bool;
    /// `getRelationship(that)`: the owner's to another object.
    fn object_relationship(&mut self, that: ObjectId) -> Relationship;
    /// `setStatus(MAKE_OBJECT_STATUS_MASK..(bits), set)`: one call, its bits.
    fn set_status_bits(&mut self, bits: &[ObjectStatusTypes], set: bool);
    /// `getTeam()->setTeamTargetObject(target)`.
    fn set_team_target_object(&mut self, target: ObjectId);
    /// The contain's (another object's: the stand-ins' one contain) `isGarrisonable()`, `getContainCount()`.
    fn contain_is_garrisonable(&self) -> bool;
    fn contain_count(&self) -> u32;
    /// The owner's container's `isEnclosingContainerFor(owner)`.
    fn contain_is_enclosing_container_for_me(&mut self) -> bool;
    /// The owner's container's `attemptBestFirePointPosition(owner, weapon, victim)` / `(.., targetPos)`.
    fn contain_attempt_best_fire_point_at_object(&mut self, victim: ObjectId) -> bool;
    fn contain_attempt_best_fire_point_at_position(&mut self, pos: &Coord3D) -> bool;
    /// `ThePartitionManager->getRelativeAngle2D(owner, other)` / `(owner, pos)`.
    fn relative_angle_2d_to_object(&mut self, other: ObjectId) -> f32;
    fn relative_angle_2d_to_position(&mut self, pos: &Coord3D) -> f32;
    /// The current weapon's `getMaxShotCount()`, `getAimDelta()`, `getStatus()`, `hasLeechRange()`.
    fn weapon_max_shot_count(&self) -> i32;
    fn weapon_aim_delta(&self) -> f32;
    fn weapon_status_now(&self, slot: WeaponSlotType) -> WeaponStatus;
    fn weapon_has_leech_range_now(&self, slot: WeaponSlotType) -> bool;
    /// The slot's weapon's `fireWeapon(owner, pos)`: whether it reloaded.
    fn weapon_fire_at(&mut self, slot: WeaponSlotType, pos: &Coord3D) -> bool;
    /// The slot's weapon's `setPreAttackFinishedFrame(frame)`.
    fn weapon_set_pre_attack_finished(&mut self, slot: WeaponSlotType, frame: u32);
    /// The owner's weapon calls (`setFiringConditionForCurrentWeapon`, `preFireCurrentWeapon`, `fireCurrentWeapon`,
    /// `releaseWeaponLock`, `notifyFiringTrackerShotFired`).
    fn set_firing_condition_for_current_weapon(&mut self);
    fn pre_fire_current_weapon(&mut self, victim: Option<ObjectId>);
    fn fire_current_weapon_at_object(&mut self, victim: Option<ObjectId>);
    fn fire_current_weapon_at_position(&mut self, pos: &Coord3D);
    fn release_weapon_lock(&mut self, lock: i32);
    // ---- batch A12: the hunt and attack-area states
    /// The controlling player's `getUnitsShouldHunt()` (`None`: no controlling player).
    fn units_should_hunt(&self) -> Option<bool>;
    /// The attack priority info's `getPriority(obj->getTemplate())` (the info by index).
    fn attack_priority(&mut self, info: usize, obj: ObjectId) -> i32;
    // ---- batch A13: the exit states (`container`: the object whose contain and exit are asked)
    /// The container's contain's `onObjectWantsToEnterOrExit(owner, wants)` (`ObjectEnterExitType`).
    fn contain_wants_to_enter_or_exit(&mut self, container: ObjectId, wants: i32);
    /// The container's contain's `getContainExitInterface() != NULL`.
    fn contain_has_exit_interface(&mut self, container: ObjectId) -> bool;
    /// That exit's `isExitBusy()`, `reserveDoorForExit(owner's template, owner)` (`ExitDoorType`), `exitObjectViaDoor(owner, door)`.
    fn exit_is_busy(&mut self, container: ObjectId) -> bool;
    fn exit_reserve_door(&mut self, container: ObjectId) -> i32;
    fn exit_object_via_door(&mut self, container: ObjectId, door: i32);
    /// Another object's AI's `getAiFreeToExit(owner)` (`AIFreeToExitType`; `None`: it has no AI).
    fn object_ai_free_to_exit(&mut self, id: ObjectId) -> Option<i32>;
    // ---- batch A14: the enter state
    /// Another object's `isAboveTerrain()`.
    fn object_is_above_terrain(&mut self, id: ObjectId) -> bool;
    /// `TheActionManager->getCanAttackObject(owner, target, src, ATTACK_NEW_TARGET)` (`CanAttackResult`).
    fn action_can_attack_object(&mut self, target: ObjectId, src: CommandSourceType) -> CanAttackResultValue;
    /// Another object's geometry: (minor radius, major radius, whether a box).
    fn object_geometry_radii(&mut self, id: ObjectId) -> (f32, f32, bool);
    /// The container's contain's `addToContain(owner)`.
    fn contain_add_to_contain(&mut self, container: ObjectId);
    // ---- batch A15: the rappel state
    /// The container's contain's `getContainedItemsList()`, in order (empty: none).
    fn contain_items(&mut self, container: ObjectId) -> Vec<ObjectId>;
    /// The container's contain's `removeFromContain(obj)`, `isValidContainerFor(owner, TRUE)`.
    fn contain_remove_from_contain(&mut self, container: ObjectId, obj: ObjectId);
    fn contain_is_valid_container_for_me(&mut self, container: ObjectId) -> bool;
    /// The owner's `scoreTheKill(victim)`; another object's `kill()`; the owner's `kill()`.
    fn score_the_kill(&mut self, victim: ObjectId);
    fn object_kill(&mut self, id: ObjectId);
    fn kill_self(&mut self);
    /// `FXList::doFXObj(owner's template's getPerUnitFX("CombatDropKillFX"), bldg)`.
    fn do_fx_combat_drop_kill(&mut self, bldg: ObjectId);
    /// The physics' `resetDynamicPhysics()`, `scrubVelocityZ(rate)` (the physics used unchecked).
    fn physics_reset_dynamic(&mut self);
    fn physics_scrub_velocity_z(&mut self, rate: f32);
    /// The owner's `setLayer(layer)`.
    fn object_set_layer(&mut self, layer: PathfindLayer);
    /// Another object's `getOrientation()`, `getPosition()`, its geometry's `getBoundingCircleRadius()`.
    fn object_orientation(&mut self, id: ObjectId) -> f32;
    fn object_position(&mut self, id: ObjectId) -> Coord3D;
    fn object_bounding_circle_radius(&mut self, id: ObjectId) -> f32;
    /// `GameLogicRandomValueReal(lo, hi)`.
    fn logic_random_value_real(&mut self, lo: f32, hi: f32) -> f32;
    /// `ThePartitionManager->findPositionAround(pos, options)` with no source object (`find_position_around`'s is the owner).
    fn find_position_around_no_source(&mut self, pos: &Coord3D, min_radius: f32, max_radius: f32) -> Option<Coord3D>;
    // ---- batch A16: the attack-squad state
    /// The controlling player's `getPlayerDifficulty()` (`GameDifficulty`; the player read unchecked).
    fn player_difficulty(&mut self) -> i32;
    /// `TheScriptEngine->getChooseVictimAlwaysUsesNormal()`.
    fn choose_victim_always_uses_normal(&mut self) -> bool;
    /// Another object's `isSelectable()`.
    fn object_is_selectable(&mut self, id: ObjectId) -> bool;
    /// `ThePartitionManager->getClosestObject(owner, HUGE_DIST, FROM_CENTER_2D, {on the squad, the same map status})`.
    fn closest_object_on_squad(&mut self, squad: &[ObjectId]) -> ObjectId;
    fn notify_firing_tracker_shot_fired(&mut self, slot: WeaponSlotType, victim: ObjectId);
    // ---- batch A10b: the chase states
    /// The current weapon's `isTooClose(owner, victim)`.
    fn weapon_is_too_close(&mut self, victim: ObjectId) -> bool;
    /// Another object's physics' `getForwardSpeed2D()`.
    fn object_forward_speed_2d(&self, id: ObjectId) -> f32;
    /// Another object's `getUnitDirectionVector2D()`.
    fn object_unit_direction_vector_2d(&mut self, id: ObjectId) -> Coord3D;
    /// Another object's `getGeometryInfo().getCenterPosition(pos, center)`.
    fn object_center_position(&mut self, id: ObjectId, pos: &Coord3D) -> Coord3D;
    /// Another object's `getTemplate()->isKindOf(kind)`.
    fn object_template_is_kind_of(&self, id: ObjectId, kind: KindOfType) -> bool;
    /// An untranslated state's `onEnter`.
    fn untranslated_on_enter(&mut self, id: StateId) -> StateReturn;
    /// An untranslated state's `update`.
    fn untranslated_update(&mut self, id: StateId) -> StateReturn;
    /// An untranslated state's `onExit`.
    fn untranslated_on_exit(&mut self, id: StateId, status: StateExitType);
}

/// The AI's states (`AIStateMachine`'s).
pub enum AiState<H: AiStateHost> {
    /// `AIIdleState`.
    Idle { initial_sleep_offset: u16, should_look_for_targets: bool, inited: bool },
    /// `AIDeadState`.
    Dead,
    /// `AIWaitState`.
    Wait,
    /// `AIBusyState`.
    Busy,
    /// `AIMoveToState` (batch A7): `m_isMoveTo` ("ACTUALLY A MOVE TO"; subclasses not yet translated set it false).
    MoveTo { mv: MoveToData, is_move_to: bool },
    /// `AIMoveOutOfTheWayState`.
    MoveOutOfTheWay { mv: MoveToData },
    /// `AIMoveAndTightenState`.
    MoveAndTighten { mv: MoveToData, ok_to_repath_times: i32, check_for_path: bool },
    /// `AIMoveAwayFromRepulsorsState` (an `AIMoveAndTightenState`).
    MoveAwayFromRepulsors { mv: MoveToData, ok_to_repath_times: i32, check_for_path: bool },
    /// `AIFollowPathState` (batch A8; `AI_FOLLOW_PATH` and `AI_FOLLOW_EXITPRODUCTION_PATH`).
    FollowPath { mv: MoveToData, index: i32, adjust_final: bool, adjust_final_override: bool, retry_count: i32 },
    /// `AIMoveAndEvacuateState` (`AI_MOVE_AND_EVACUATE` and `AI_MOVE_AND_EVACUATE_AND_EXIT`).
    MoveAndEvacuate { mv: MoveToData, origin: Coord3D },
    /// `AIMoveAndDeleteState`.
    MoveAndDelete { mv: MoveToData, append_goal_position: bool },
    /// `AIFollowWaypointPathState` (batch A9; `AI_FOLLOW_WAYPOINT_PATH_AS_TEAM` and `_AS_INDIVIDUALS`): `m_moveAsGroup`,
    /// `m_isFollowWaypointPathState`.
    /// (batch A12b: `AIAttackFollowWaypointPathState` too, `is_follow` false, with its `m_attackFollowMachine`)
    FollowWaypointPath { mv: MoveToData, wp: WaypointPathData, as_group: bool, is_follow: bool, machine: Option<StateMachine<AiState<H>>> },
    /// `AIFollowWaypointPathExactState` (`AI_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT` and `_AS_INDIVIDUALS_EXACT`).
    FollowWaypointPathExact { mv: MoveToData, last: Option<u32>, as_group: bool },
    /// `AIWanderState`.
    Wander { mv: MoveToData, wp: WaypointPathData, wait_frames: i32, timer: i32 },
    /// `AIWanderInPlaceState`.
    WanderInPlace { mv: MoveToData, origin: Coord3D, wait_frames: i32, timer: i32 },
    /// `AIPanicState`.
    Panic { mv: MoveToData, wp: WaypointPathData, wait_frames: i32, timer: i32 },
    /// `AIAttackState` (batch A10a; the four attack ids): `m_follow`, `m_isAttackingObject`, `m_isForceAttacking`, the owner's kinds
    /// that make its machine, `m_attackMachine`, `m_originalVictimPos`, `m_victimTeam`, `m_lockedWeaponOnEnter` (its slot).
    Attack {
        follow: bool,
        attacking_object: bool,
        force_attacking: bool,
        kinds: AttackOwnerKinds,
        machine: Option<StateMachine<AiState<H>>>,
        original_victim_pos: Coord3D,
        victim_team: Option<u32>,
        locked_weapon_on_enter: Option<WeaponSlotType>,
    },
    /// The attack machine's `AIAttackAimAtTargetState`.
    AttackAim { attacking_object: bool, force_attacking: bool, can_turn_in_place: bool, set_locomotor: bool },
    /// The attack machine's `AIAttackFireWeaponState`.
    AttackFire,
    /// The attack machine's `AIAttackApproachTargetState` (batch A10b).
    AttackApproach { mv: MoveToData, chase: ChaseData },
    /// The attack machine's `AIAttackPursueTargetState`.
    AttackPursue { mv: MoveToData, chase: ChaseData },
    /// `AIAttackMoveToState` (batch A11): `AIMoveToState`'s base, `m_frameToSleepUntil`, `m_retryCount`, `m_commandSrc`,
    /// `m_attackMoveMachine`.
    AttackMove { mv: MoveToData, frame_to_sleep_until: u32, retry_count: i32, command_src: CommandSourceType, machine: Option<StateMachine<AiState<H>>> },
    /// `AIHuntState` (`area` false) and `AIAttackAreaState` (batch A12): `m_huntMachine` / `m_attackMachine`, `m_nextEnemyScanTime`
    /// (the owner's kinds, read at the AI's machine's creation, make the machine).
    Hunt { area: bool, kinds: AttackOwnerKinds, machine: Option<StateMachine<AiState<H>>>, next_enemy_scan_time: u32 },
    /// `AIAttackSquadState` (batch A16): `m_attackSquadMachine` (the owner's kinds make it).
    AttackSquad { kinds: AttackOwnerKinds, machine: Option<StateMachine<AiState<H>>> },
    /// `AIRappelState` (batch A15).
    Rappel(RappelData),
    /// `AIEnterState` (batch A14): `AIInternalMoveToState`'s base, `m_entryToClear`.
    Enter { mv: MoveToData, entry_to_clear: ObjectId },
    /// `AIExitState` (`instantly` false) and `AIExitInstantlyState` (batch A13): `m_entryToClear`.
    Exit { instantly: bool, entry_to_clear: ObjectId },
    /// `AIFaceState` (batch A13): `m_obj` (facing the goal object, not the position), `m_canTurnInPlace`.
    Face { obj: bool, can_turn_in_place: bool },
    /// `AIPickUpCrateState`: `m_delayCounter`.
    PickUpCrate { mv: MoveToData, delay_counter: i32 },
    /// The attack machine's `ContinueState` and `FailureState`.
    Utility(UtilityState),
    /// A state class not yet translated: the host's (see the module docs).
    Untranslated { id: StateId },
    #[doc(hidden)]
    _Host(Infallible, PhantomData<fn() -> H>),
}

/// The machine's transition conditions (none of the batch's states has one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiCondition {
    /// `outOfWeaponRangeObject` (batch A10a: the attack machine's).
    OutOfWeaponRangeObject,
    /// `inWeaponRangeObject`.
    InWeaponRangeObject,
    /// `wantToSquishTarget`.
    WantToSquishTarget,
    /// `outOfWeaponRangePosition`.
    OutOfWeaponRangePosition,
    /// `cannotPossiblyAttackObject` with its attack type.
    CannotPossiblyAttackObject(AbleToAttackType),
}

/// The static `isIdle`, `isAttack`, `isBusy` of an untranslated state's class: `AIAttackState`'s `TRUE` for the attack states;
/// `false` for the others (whose answers are their inner machines', or the base class's).
pub fn untranslated_flags(id: StateId) -> (bool, bool, bool) {
    let attack = matches!(id, AI_ATTACK_POSITION | AI_ATTACK_OBJECT | AI_FORCE_ATTACK_OBJECT | AI_ATTACK_AND_FOLLOW_OBJECT);
    (false, attack, false)
}

/// The lent AI (the innermost running machine's).
pub fn ai<H: AiStateHost>(h: &mut H) -> &mut AiUpdate {
    h.ai_stack().last_mut().expect("a running AI")
}

/// Runs an AI member on the lent AI (taken from the host for the call, then given back).
pub fn with_ai<H: AiStateHost, R>(h: &mut H, f: impl FnOnce(&mut AiUpdate, &mut H) -> R) -> R {
    let mut a = h.ai_stack().pop().expect("a running AI");
    let r = f(&mut a, h);
    h.ai_stack().push(a);
    r
}

fn sm_frame<H: StateContext>(h: &H) -> u32 {
    StateContext::frame(h)
}

/// `AIStateMachine::setState(newStateID)`: the base's, and the AI told if the state changed.
pub fn aism_set_state<H: AiStateHost>(sm: &mut StateMachine<AiState<H>>, h: &mut H, new_state_id: StateId) -> SmResult<StateReturn> {
    let old_id = sm.get_current_state_id();
    let tmp = sm.set_state(h, new_state_id)?;
    if old_id != new_state_id && !plant(3240) {
        with_ai(h, |a, h| a.friend_notify_state_machine_changed(h));
    }
    Ok(tmp)
}

/// `AIStateMachine::clear()`: the base's, the goal path, waypoint and squad, and the AI told.
pub fn aism_clear<H: AiStateHost>(sm: &mut StateMachine<AiState<H>>, h: &mut H) -> SmResult<()> {
    sm.clear(h)?;
    let extras = &mut ai(h).machine;
    extras.goal_path.clear();
    extras.goal_waypoint = None;
    // (`m_goalSquad = NULL`: the squad is dropped, upstream leaks it)
    extras.goal_squad = None;
    with_ai(h, |a, h| a.friend_notify_state_machine_changed(h));
    Ok(())
}

/// `AIStateMachine::resetToDefaultState()`: the base's, and the AI told.
pub fn aism_reset_to_default_state<H: AiStateHost>(sm: &mut StateMachine<AiState<H>>, h: &mut H) -> SmResult<StateReturn> {
    let tmp = sm.reset_to_default_state(h)?;
    with_ai(h, |a, h| a.friend_notify_state_machine_changed(h));
    Ok(tmp)
}

/// `AIStateMachine::updateStateMachine()`: the temporary state first (while it continues and has time, it is all that runs), then
/// the base's.
pub fn aism_update_state_machine<H: AiStateHost>(sm: &mut StateMachine<AiState<H>>, h: &mut H) -> SmResult<StateReturn> {
    let temp = ai(h).machine.temporary_state;
    if temp != INVALID_STATE_ID {
        // "execute this state"
        let mut status = AiState::update(sm, h, temp)?;
        if ai(h).machine.temporary_state_frame_end < sm_frame(h) && status == StateReturn::CONTINUE {
            // "ran out of time."
            status = StateReturn::SUCCESS;
        }
        if status == StateReturn::CONTINUE {
            return Ok(status);
        }
        AiState::on_exit(sm, h, temp, StateExitType::Normal)?;
        ai(h).machine.temporary_state = INVALID_STATE_ID;
    }
    sm.update_state_machine(h)
}

/// `AIStateMachine::setTemporaryState(newStateID, frameLimitCount)`.
pub fn aism_set_temporary_state<H: AiStateHost>(
    sm: &mut StateMachine<AiState<H>>,
    h: &mut H,
    new_state_id: StateId,
    mut frame_limit_count: i32,
) -> SmResult<StateReturn> {
    // "extract the state associated with the given ID"
    let new_state = sm.resolve_state_id(new_state_id)?;
    let temp = ai(h).machine.temporary_state;
    if temp != INVALID_STATE_ID {
        AiState::on_exit(sm, h, temp, StateExitType::Reset)?;
        ai(h).machine.temporary_state = INVALID_STATE_ID;
    }
    ai(h).machine.temporary_state = new_state;
    let ret = AiState::on_enter(sm, h, new_state)?;
    if ret != StateReturn::CONTINUE {
        AiState::on_exit(sm, h, new_state, StateExitType::Normal)?;
        ai(h).machine.temporary_state = INVALID_STATE_ID;
        return Ok(ret);
    }
    const FRAME_COUNT_MAX: i32 = 60 * LOGICFRAMES_PER_SECOND;
    // "1 minute seems overly long for a temporary state override."
    if frame_limit_count > FRAME_COUNT_MAX && !plant(3241) {
        frame_limit_count = FRAME_COUNT_MAX;
    }
    ai(h).machine.temporary_state_frame_end = sm_frame(h).wrapping_add(frame_limit_count as u32);
    Ok(ret)
}

/// `AIStateMachine::addToGoalPath(pathPoint)`: appended unless it is the path's last point.
pub fn aism_add_to_goal_path(extras: &mut AiMachineExtras, point: &Coord3D) {
    match extras.goal_path.last() {
        Some(last) if *last == *point && !plant(3247) => {}
        _ => extras.goal_path.push(*point),
    }
}

/// `AIStateMachine::getGoalPathPosition(i)`: `None` out of range.
pub fn aism_goal_path_position(extras: &AiMachineExtras, i: i32) -> Option<Coord3D> {
    if i < 0 || i as usize >= extras.goal_path.len() {
        return None;
    }
    Some(extras.goal_path[i as usize])
}

/// `AIStateMachine::xfer` (version 1): the base's, the goal path, the goal waypoint's name, the squad flag (no squad: batch AIGroup),
/// the temporary state's id and snapshot, its frame end. `waypoint_name` and `waypoint_by_name` are `TheTerrainLogic`'s.
pub fn aism_xfer<H: AiStateHost>(
    sm: &mut StateMachine<AiState<H>>,
    extras: &mut AiMachineExtras,
    xfer: &mut dyn Xfer,
    waypoint_name: impl Fn(u32) -> Vec<u8>,
    waypoint_by_name: impl Fn(&[u8]) -> Option<u32>,
) -> SmResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    sm.xfer_machine(xfer)?;

    let mut count = extras.goal_path.len() as i32;
    xfer.xfer_int(&mut count)?;
    for i in 0..count.max(0) as usize {
        let mut pos = if xfer.get_xfer_mode() != XferMode::Load { extras.goal_path[i] } else { Coord3D::default() };
        xfer.xfer_coord3d(&mut pos)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            extras.goal_path.push(pos);
        }
    }

    let mut name = extras.goal_waypoint.map(&waypoint_name).unwrap_or_default();
    xfer.xfer_ascii_string(&mut name)?;
    if xfer.get_xfer_mode() == XferMode::Load && !name.is_empty() {
        extras.goal_waypoint = waypoint_by_name(&name);
    }
    let mut has_squad = extras.goal_squad.is_some();
    xfer.xfer_bool(&mut has_squad)?;
    if xfer.get_xfer_mode() == XferMode::Load && has_squad && extras.goal_squad.is_none() {
        extras.goal_squad = Some(Squad::default());
    }
    if has_squad {
        let squad = extras.goal_squad.as_mut().expect("the goal squad");
        squad_xfer(squad, xfer).map_err(|_| crate::common::state_machine::StateMachineError::InvalidData)?;
    }

    let mut id = extras.temporary_state;
    xfer.xfer_unsigned_int(&mut id)?;
    if xfer.get_xfer_mode() == XferMode::Load && id != INVALID_STATE_ID {
        extras.temporary_state = sm.resolve_state_id(id)?;
    }
    if extras.temporary_state != INVALID_STATE_ID {
        sm.xfer_state(xfer, extras.temporary_state)?;
    }
    xfer.xfer_unsigned_int(&mut extras.temporary_state_frame_end)?;
    Ok(())
}

/// `AIStateMachine::AIStateMachine(obj, name)`'s state definitions, in upstream's order ("order matters: first state is the default
/// state"), with each state's success and failure successors.
pub fn define_states<H: AiStateHost>(sm: &mut StateMachine<AiState<H>>, kinds: AttackOwnerKinds) -> SmResult<()> {
    let u = |id| AiState::Untranslated { id };
    // AIFollowPathState's constructor: adjust final, no override, index 0, ten retries
    let follow_path =
        || AiState::FollowPath { mv: MoveToData::default(), index: 0, adjust_final: true, adjust_final_override: false, retry_count: 10 };
    // batch A9: the members no constructor sets are the pool's zeros (AIFollowWaypointPathState's constructor sets m_angle 0; the
    // wander state's its timer and wait 0; the exact state's its last waypoint none)
    let follow_waypoint_path = |as_group| AiState::FollowWaypointPath {
        mv: MoveToData::default(),
        wp: WaypointPathData::default(),
        as_group,
        is_follow: true,
        machine: None,
    };
    let exact = |as_group| AiState::FollowWaypointPathExact { mv: MoveToData::default(), last: None, as_group };
    let wander = |panic: bool| {
        let (mv, wp) = (MoveToData::default(), WaypointPathData::default());
        if panic {
            AiState::Panic { mv, wp, wait_frames: 0, timer: 0 }
        } else {
            AiState::Wander { mv, wp, wait_frames: 0, timer: 0 }
        }
    };
    let wander_in_place = || AiState::WanderInPlace { mv: MoveToData::default(), origin: Coord3D::default(), wait_frames: 0, timer: 0 };
    // batch A10a: AIAttackState( machine, follow, attackingObject, forceAttacking, NULL )
    let attack = |follow, attacking_object, force_attacking| attack_states::attack_state(follow, attacking_object, force_attacking, kinds);
    let defs: [(StateId, AiState<H>, StateId, StateId); 41] = [
        (AI_IDLE, AiState::Idle { initial_sleep_offset: u16::MAX, should_look_for_targets: true, inited: false }, AI_IDLE, AI_IDLE),
        (AI_MOVE_TO, AiState::MoveTo { mv: MoveToData::default(), is_move_to: true }, AI_IDLE, AI_IDLE),
        (AI_MOVE_OUT_OF_THE_WAY, AiState::MoveOutOfTheWay { mv: MoveToData::default() }, AI_IDLE, AI_IDLE),
        (AI_MOVE_AND_TIGHTEN, AiState::MoveAndTighten { mv: MoveToData::default(), ok_to_repath_times: 0, check_for_path: false }, AI_IDLE, AI_IDLE),
        (
            AI_MOVE_AWAY_FROM_REPULSORS,
            AiState::MoveAwayFromRepulsors { mv: MoveToData::default(), ok_to_repath_times: 0, check_for_path: false },
            AI_WANDER_IN_PLACE,
            AI_WANDER_IN_PLACE,
        ),
        (AI_WANDER_IN_PLACE, wander_in_place(), AI_MOVE_AWAY_FROM_REPULSORS, AI_MOVE_AWAY_FROM_REPULSORS),
        (AI_ATTACK_MOVE_TO, attack_move::attack_move(kinds), AI_IDLE, AI_IDLE),
        (AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM, attack_move::attack_follow(true, kinds), AI_IDLE, AI_IDLE),
        (AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS, attack_move::attack_follow(false, kinds), AI_IDLE, AI_IDLE),
        (AI_FOLLOW_WAYPOINT_PATH_AS_TEAM, follow_waypoint_path(true), AI_IDLE, AI_IDLE),
        (AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS, follow_waypoint_path(false), AI_IDLE, AI_IDLE),
        (AI_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT, exact(true), AI_IDLE, AI_IDLE),
        (AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS_EXACT, exact(false), AI_IDLE, AI_IDLE),
        (AI_FOLLOW_PATH, follow_path(), AI_IDLE, AI_IDLE),
        (AI_FOLLOW_EXITPRODUCTION_PATH, follow_path(), AI_IDLE, AI_IDLE),
        (AI_MOVE_AND_EVACUATE, AiState::MoveAndEvacuate { mv: MoveToData::default(), origin: Coord3D::default() }, AI_IDLE, AI_IDLE),
        (
            AI_MOVE_AND_EVACUATE_AND_EXIT,
            AiState::MoveAndEvacuate { mv: MoveToData::default(), origin: Coord3D::default() },
            AI_MOVE_AND_DELETE,
            AI_MOVE_AND_DELETE,
        ),
        (AI_MOVE_AND_DELETE, AiState::MoveAndDelete { mv: MoveToData::default(), append_goal_position: false }, AI_IDLE, AI_IDLE),
        (AI_WAIT, AiState::Wait, AI_IDLE, AI_IDLE),
        (AI_ATTACK_POSITION, attack(false, false, false), AI_IDLE, AI_IDLE),
        (AI_ATTACK_OBJECT, attack(false, true, false), AI_IDLE, AI_IDLE),
        (AI_FORCE_ATTACK_OBJECT, attack(false, true, true), AI_IDLE, AI_IDLE),
        (AI_ATTACK_AND_FOLLOW_OBJECT, attack(true, true, false), AI_IDLE, AI_IDLE),
        (AI_ATTACK_SQUAD, hunt_states::attack_squad(kinds), AI_IDLE, AI_IDLE),
        (AI_WANDER, wander(false), AI_IDLE, AI_MOVE_AWAY_FROM_REPULSORS),
        (AI_PANIC, wander(true), AI_IDLE, AI_MOVE_AWAY_FROM_REPULSORS),
        (AI_DEAD, AiState::Dead, AI_IDLE, AI_IDLE),
        (AI_DOCK, u(AI_DOCK), AI_IDLE, AI_IDLE),
        (AI_ENTER, AiState::Enter { mv: MoveToData::default(), entry_to_clear: INVALID_ID }, AI_IDLE, AI_IDLE),
        (AI_EXIT, AiState::Exit { instantly: false, entry_to_clear: INVALID_ID }, AI_IDLE, AI_IDLE),
        (AI_EXIT_INSTANTLY, AiState::Exit { instantly: true, entry_to_clear: INVALID_ID }, AI_IDLE, AI_IDLE),
        (AI_GUARD, u(AI_GUARD), AI_IDLE, AI_IDLE),
        (AI_GUARD_TUNNEL_NETWORK, u(AI_GUARD_TUNNEL_NETWORK), AI_IDLE, AI_IDLE),
        (AI_GUARD_RETALIATE, u(AI_GUARD_RETALIATE), AI_IDLE, AI_IDLE),
        (AI_HUNT, hunt_states::hunt(false, kinds), AI_IDLE, AI_IDLE),
        (AI_ATTACK_AREA, hunt_states::hunt(true, kinds), AI_IDLE, AI_IDLE),
        (AI_FACE_OBJECT, AiState::Face { obj: true, can_turn_in_place: false }, AI_IDLE, AI_IDLE),
        (AI_FACE_POSITION, AiState::Face { obj: false, can_turn_in_place: false }, AI_IDLE, AI_IDLE),
        (AI_PICK_UP_CRATE, attack_move::pick_up_crate(), AI_IDLE, AI_IDLE),
        // (the constructor sets nothing: the pool's zeros)
        (AI_RAPPEL_INTO, AiState::Rappel(RappelData::default()), AI_IDLE, AI_IDLE),
        (AI_BUSY, AiState::Busy, AI_IDLE, AI_IDLE),
    ];
    for (id, state, success, failure) in defs {
        sm.define_state(id, state, success, failure, &[])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The states
// ---------------------------------------------------------------------------

/// `AIIdleState::doInitIdleState()`: once, in the first update after `onEnter` ("onEnter for idle is called during object creation").
fn idle_do_init<H: AiStateHost>(sm: &mut StateMachine<AiState<H>>, h: &mut H, id: StateId) {
    let Some(AiState::Idle { inited, .. }) = sm.state_mut(id) else { unreachable!("the idle state") };
    if !*inited {
        return;
    }
    *inited = false;

    let ultra_accurate = ai(h).get_cur_locomotor().is_some_and(|l| l.is_ultra_accurate());
    // (the AI's machine: when this idle state is a sub-machine's, the lent view of it; batch A12)
    let view = attack_states::top_view(sm, h);
    let idle_and_ground = with_ai(h, |a, h| a.is_idle(&view) && a.is_doing_ground_movement(h));
    if idle_and_ground {
        // "Update the goal."
        let mut goal_pos = h.position();
        // "but only if we have a valid position."
        if goal_pos.x != 0.0 || goal_pos.y != 0.0 || goal_pos.z != 0.0 {
            let layer = h.layer();
            h.pf_update_goal(&goal_pos, layer);
            if !ultra_accurate {
                if let Some(g) = h.pf_goal_position() {
                    goal_pos = g;
                    if sm_frame(h) <= 1 {
                        h.set_position(&goal_pos);
                    }
                    // (STOP_AND_SLIDE is not defined: no setFinalPosition)
                    let layer = h.layer();
                    h.pf_update_goal(&goal_pos, layer);
                }
            }
        }
    }

    ai(h).set_locomotor_goal_none();
    with_ai(h, |a, h| a.set_current_victim(h, INVALID_ID));
}

fn idle_on_enter<H: AiStateHost>(sm: &mut StateMachine<AiState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    // "We could possibly not have ai here if we were constructed this frame." (the AI is always lent here)
    with_ai(h, |a, h| a.reset_next_mood_check_time(h));
    // "reset the idle countdown so that we don't do expensive checks too often, randomized so we avoid spikes"
    let offset = h.logic_random_value(0, IDLE_COUNTDOWN_DELAY as i32) as u16;
    let Some(AiState::Idle { inited, initial_sleep_offset, .. }) = sm.state_mut(id) else { unreachable!("the idle state") };
    *inited = !plant(3242);
    *initial_sleep_offset = offset;
    // "never sleep at the start, since we have to do checkGoalPos first time thru"
    Ok(StateReturn::CONTINUE)
}

fn idle_update<H: AiStateHost>(sm: &mut StateMachine<AiState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    idle_do_init(sm, h, id);

    let Some(AiState::Idle { initial_sleep_offset, should_look_for_targets, .. }) = sm.state_mut(id) else { unreachable!("the idle state") };
    let mut time_to_sleep = IDLE_COUNTDOWN_DELAY + u32::from(*initial_sleep_offset);
    let old_sleep_offset = *initial_sleep_offset;
    *initial_sleep_offset = 0;
    let should_look = *should_look_for_targets;

    // "This state is used internally some places, so we don't necessarily want to be looking for targets"
    if should_look && !sm.is_locked() {
        // "do repulsor logic"
        if h.is_kind_of(KINDOF_CAN_BE_REPULSED) && ai(h).is_idle(&*sm) {
            let range = h.object_vision_range();
            let enemy = h.find_closest_repulsor(range);
            if enemy != INVALID_ID {
                aism_set_state(sm, h, AI_MOVE_AWAY_FROM_REPULSORS)?;
                // "since we just changed the state, it doesn't really matter what we return here."
                return Ok(StateReturn::CONTINUE);
            }
        }

        // "Check to see if we have created a crate we need to pick up."
        let crate_id = with_ai(h, |a, h| a.check_for_crate_to_pickup(h));
        if crate_id != INVALID_ID {
            commands::ai_move_to_object(sm, h, crate_id, CMD_FROM_AI)?;
            return Ok(StateReturn::CONTINUE);
        }

        if !h.is_disabled_by_type(DISABLED_PARALYZED)
            && !h.is_disabled_by_type(DISABLED_UNMANNED)
            && !h.is_disabled_by_type(DISABLED_EMP)
            && !h.is_disabled_by_type(DISABLED_SUBDUED)
            && !h.is_disabled_by_type(DISABLED_HACKED)
        {
            // "mood targeting"
            let mood_adjust = with_ai(h, |a, h| a.get_mood_matrix_action_adjustment(h, &*sm, MM_ACTION_IDLE));
            if mood_adjust & MAA_AFFECT_RANGE_IGNORE_ALL == 0 {
                // "If we're supposed to attack based on mood, etc, then we will do so."
                let enemy = with_ai(h, |a, h| a.get_next_mood_target(h, &*sm, true, true));
                if enemy != INVALID_ID {
                    commands::ai_attack_object(sm, h, enemy, NO_MAX_SHOTS_LIMIT, CMD_FROM_AI)?;
                    // "weird but true. return state_continue, because if we're here, we're actually an attack state"
                    return Ok(StateReturn::CONTINUE);
                }
            }
        }

        let now = sm_frame(h);
        let next_mood_check_time = ai(h).get_next_mood_check_time();
        if next_mood_check_time > now {
            // "if we need to look for targets, might need to sleep less."
            let mood_sleep = next_mood_check_time - now;
            if mood_sleep < time_to_sleep {
                time_to_sleep = mood_sleep;
                // "if we do this, save the random sleep offset for next time."
                if let Some(AiState::Idle { initial_sleep_offset, .. }) = sm.state_mut(id) {
                    *initial_sleep_offset = if plant(3243) { 0 } else { old_sleep_offset };
                }
            }
        }
    }

    Ok(StateReturn::sleep(time_to_sleep))
}

/// `AIDeadState::onEnter()`: "dying objects are NEVER firing, surrendered, etc, so clear 'em all here."
fn dead_on_enter<H: AiStateHost>(h: &mut H) -> SmResult<StateReturn> {
    let non_dying_stuff = [
        MODELCONDITION_USING_WEAPON_A,
        MODELCONDITION_USING_WEAPON_B,
        MODELCONDITION_USING_WEAPON_C,
        MODELCONDITION_FIRING_A,
        MODELCONDITION_FIRING_B,
        MODELCONDITION_FIRING_C,
        MODELCONDITION_BETWEEN_FIRING_SHOTS_A,
        MODELCONDITION_BETWEEN_FIRING_SHOTS_B,
        MODELCONDITION_BETWEEN_FIRING_SHOTS_C,
        MODELCONDITION_RELOADING_A,
        MODELCONDITION_RELOADING_B,
        MODELCONDITION_RELOADING_C,
        MODELCONDITION_PREATTACK_A,
        MODELCONDITION_PREATTACK_B,
        MODELCONDITION_PREATTACK_C,
        MODELCONDITION_MOVING,
    ];
    h.clear_and_set_model_condition_flags(&non_dying_stuff[..non_dying_stuff.len() - usize::from(plant(3244))], MODELCONDITION_DYING);
    h.notify_of_object_creation_or_destruction();
    Ok(StateReturn::CONTINUE)
}

/// `AIDeadState::update()`.
fn dead_update<H: AiStateHost>(h: &mut H) -> SmResult<StateReturn> {
    ai(h).set_locomotor_goal_none();
    // "re-mark this every time, just in case our health miraculously heals from 0 to nonzero..."
    h.set_effectively_dead(true);
    if h.is_kind_of(KINDOF_INFANTRY) && h.has_physics() {
        // "we want 'em to stop, but looks wonky if they stop dead in their tracks in onEnter."
        const FACTOR: f32 = 0.8; // "0.8 ^ 30 == 0.012, so they slow to 4% of speed over 1 sec"
        let vel = h.velocity_magnitude();
        h.scrub_velocity_2d(vel * if plant(3245) { 0.9 } else { FACTOR });
    }
    Ok(StateReturn::CONTINUE)
}

impl<H: AiStateHost> StateBehavior for AiState<H> {
    type Ctx = H;
    type Condition = AiCondition;

    fn on_enter(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(AiState::Idle { .. }) => idle_on_enter(sm, h, id),
            Some(AiState::Dead) => dead_on_enter(h),
            Some(AiState::Wait) | Some(AiState::Busy) | None => Ok(StateReturn::CONTINUE),
            Some(AiState::MoveTo { .. })
            | Some(AiState::MoveOutOfTheWay { .. })
            | Some(AiState::MoveAndTighten { .. })
            | Some(AiState::MoveAwayFromRepulsors { .. })
            | Some(AiState::FollowPath { .. })
            | Some(AiState::MoveAndEvacuate { .. })
            | Some(AiState::MoveAndDelete { .. }) => move_states::on_enter(sm, h, id),
            Some(AiState::FollowWaypointPath { machine: Some(_), .. }) => attack_move::attack_follow_on_enter(sm, h, id),
            Some(AiState::FollowWaypointPath { .. })
            | Some(AiState::FollowWaypointPathExact { .. })
            | Some(AiState::Wander { .. })
            | Some(AiState::WanderInPlace { .. })
            | Some(AiState::Panic { .. }) => waypoint_states::on_enter(sm, h, id),
            Some(AiState::Attack { .. }) => attack_states::attack_on_enter(sm, h, id),
            Some(AiState::AttackMove { .. }) => attack_move::attack_move_on_enter(sm, h, id),
            Some(AiState::PickUpCrate { .. }) => attack_move::crate_on_enter(sm, h, id),
            Some(AiState::Hunt { .. }) => hunt_states::hunt_on_enter(sm, h, id),
            Some(AiState::Exit { instantly, .. }) => {
                let instantly = *instantly;
                face_exit::exit_on_enter(sm, h, id, instantly)
            }
            Some(AiState::Face { .. }) => face_exit::face_on_enter(sm, h, id),
            Some(AiState::Enter { .. }) => enter_state::enter_on_enter(sm, h, id),
            Some(AiState::Rappel(_)) => rappel::rappel_on_enter(sm, h, id),
            Some(AiState::AttackSquad { .. }) => hunt_states::squad_on_enter(sm, h, id),
            Some(AiState::AttackAim { .. }) => attack_states::aim_on_enter(sm, h, id),
            Some(AiState::AttackApproach { .. }) => attack_states::approach_on_enter(sm, h, id),
            Some(AiState::AttackPursue { .. }) => attack_states::pursue_on_enter(sm, h, id),
            Some(AiState::AttackFire) => crate::logic::ai::fire_state::fire_on_enter(sm, h),
            Some(AiState::Utility(u)) => Ok(u.on_enter()),
            Some(AiState::Untranslated { id }) => {
                let id = *id;
                Ok(h.untranslated_on_enter(id))
            }
            Some(AiState::_Host(never, _)) => match *never {},
        }
    }

    fn on_exit(sm: &mut StateMachine<Self>, h: &mut H, id: StateId, status: StateExitType) -> SmResult<()> {
        match sm.state(id) {
            Some(AiState::Dead) => h.clear_model_condition_state(MODELCONDITION_DYING),
            Some(AiState::MoveTo { .. })
            | Some(AiState::MoveOutOfTheWay { .. })
            | Some(AiState::MoveAndTighten { .. })
            | Some(AiState::MoveAwayFromRepulsors { .. })
            | Some(AiState::FollowPath { .. })
            | Some(AiState::MoveAndEvacuate { .. })
            | Some(AiState::MoveAndDelete { .. }) => move_states::on_exit(sm, h, id, status),
            Some(AiState::FollowWaypointPath { machine: Some(_), .. }) => attack_move::attack_follow_on_exit(sm, h, id)?,
            Some(AiState::FollowWaypointPath { .. })
            | Some(AiState::FollowWaypointPathExact { .. })
            | Some(AiState::Wander { .. })
            | Some(AiState::WanderInPlace { .. })
            | Some(AiState::Panic { .. }) => waypoint_states::on_exit(sm, h, id),
            Some(AiState::Attack { .. }) => attack_states::attack_on_exit(sm, h, id)?,
            Some(AiState::AttackMove { .. }) => attack_move::attack_move_on_exit(sm, h, id)?,
            Some(AiState::PickUpCrate { .. }) => move_states::internal_on_exit(sm, h, id),
            Some(AiState::Hunt { .. }) => hunt_states::hunt_on_exit(sm, h, id)?,
            Some(AiState::Exit { .. }) => face_exit::exit_on_exit(sm, h, id),
            Some(AiState::Enter { .. }) => enter_state::enter_on_exit(sm, h, id),
            Some(AiState::Rappel(_)) => rappel::rappel_on_exit(h),
            Some(AiState::AttackSquad { .. }) => hunt_states::squad_on_exit(sm, h, id)?,
            Some(AiState::AttackAim { .. }) => attack_states::aim_on_exit(sm, h, id),
            Some(AiState::AttackApproach { .. }) => attack_states::approach_on_exit(sm, h, id),
            Some(AiState::AttackPursue { .. }) => attack_states::pursue_on_exit(sm, h, id),
            Some(AiState::AttackFire) => attack_states::fire_on_exit(sm, h, status)?,
            Some(AiState::Untranslated { id }) => {
                let id = *id;
                h.untranslated_on_exit(id, status);
            }
            _ => {}
        }
        Ok(())
    }

    fn update(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(AiState::Idle { .. }) => idle_update(sm, h, id),
            Some(AiState::Dead) => dead_update(h),
            Some(AiState::MoveTo { .. })
            | Some(AiState::MoveOutOfTheWay { .. })
            | Some(AiState::MoveAndTighten { .. })
            | Some(AiState::MoveAwayFromRepulsors { .. })
            | Some(AiState::FollowPath { .. })
            | Some(AiState::MoveAndEvacuate { .. })
            | Some(AiState::MoveAndDelete { .. }) => move_states::update(sm, h, id),
            Some(AiState::FollowWaypointPath { machine: Some(_), .. }) => attack_move::attack_follow_update(sm, h, id),
            Some(AiState::FollowWaypointPath { .. })
            | Some(AiState::FollowWaypointPathExact { .. })
            | Some(AiState::Wander { .. })
            | Some(AiState::WanderInPlace { .. })
            | Some(AiState::Panic { .. }) => waypoint_states::update(sm, h, id),
            Some(AiState::Attack { .. }) => attack_states::attack_update(sm, h, id),
            Some(AiState::AttackMove { .. }) => attack_move::attack_move_update(sm, h, id),
            Some(AiState::PickUpCrate { .. }) => attack_move::crate_update(sm, h, id),
            Some(AiState::Hunt { .. }) => hunt_states::hunt_update(sm, h, id),
            Some(AiState::Exit { instantly, .. }) => {
                let instantly = *instantly;
                face_exit::exit_state_update(sm, h, id, instantly)
            }
            Some(AiState::Face { .. }) => face_exit::face_update(sm, h, id),
            Some(AiState::Enter { .. }) => enter_state::enter_update(sm, h, id),
            Some(AiState::Rappel(_)) => rappel::rappel_update(sm, h, id),
            Some(AiState::AttackSquad { .. }) => hunt_states::squad_update(sm, h, id),
            Some(AiState::AttackAim { .. }) => attack_states::aim_update(sm, h, id),
            Some(AiState::AttackApproach { .. }) => attack_states::approach_update(sm, h, id),
            Some(AiState::AttackPursue { .. }) => attack_states::pursue_update(sm, h, id),
            Some(AiState::AttackFire) => crate::logic::ai::fire_state::fire_update(sm, h),
            Some(AiState::Utility(u)) => Ok(u.update()),
            // "@todo srj -- find a way to sleep for a number of frames here, if possible"
            Some(AiState::Wait) | Some(AiState::Busy) | None => Ok(StateReturn::CONTINUE),
            Some(AiState::Untranslated { id }) => {
                let id = *id;
                Ok(h.untranslated_update(id))
            }
            Some(AiState::_Host(never, _)) => match *never {},
        }
    }

    fn test(sm: &mut StateMachine<Self>, h: &mut H, _id: StateId, condition: AiCondition) -> SmResult<bool> {
        Ok(attack_states::test(sm, h, condition))
    }

    fn is_idle(&self) -> bool {
        match self {
            AiState::Idle { .. } => true,
            AiState::Untranslated { id } => untranslated_flags(*id).0,
            _ => false,
        }
    }
    fn is_attack(&self) -> bool {
        match self {
            // (AIAttackState's TRUE; the chase states' TRUE)
            AiState::Attack { .. } => !plant(3246),
            // (AIAttackMoveToState's: its machine's)
            AiState::AttackMove { machine, .. } => attack_move::is_attack(machine),
            // (batch A12: the hunt and attack-area states': their machines')
            AiState::Hunt { machine, .. } | AiState::AttackSquad { machine, .. } => hunt_states::is_attack(machine),
            // (batch A12b: the attack-follow-waypoint states': their machines'; the plain waypoint state has none)
            AiState::FollowWaypointPath { machine, .. } => attack_move::is_attack(machine) && !plant(3345),
            AiState::AttackApproach { .. } | AiState::AttackPursue { .. } => true,
            AiState::Untranslated { id } => untranslated_flags(*id).1 && !plant(3246),
            _ => false,
        }
    }
    fn is_busy(&self) -> bool {
        match self {
            AiState::Busy => true,
            AiState::Untranslated { id } => untranslated_flags(*id).2,
            _ => false,
        }
    }

    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            AiState::Idle { initial_sleep_offset, should_look_for_targets, inited } => {
                let current_version: XferVersion = 1;
                let mut version = current_version;
                xfer.xfer_version(&mut version, current_version)?;
                xfer.xfer_unsigned_short(initial_sleep_offset)?;
                xfer.xfer_bool(should_look_for_targets)?;
                xfer.xfer_bool(inited)?;
            }
            AiState::MoveTo { mv, .. } | AiState::MoveOutOfTheWay { mv } => mv.xfer(xfer)?,
            AiState::FollowPath { mv, index, adjust_final, adjust_final_override, .. } => {
                move_states::follow_path_xfer(mv, index, adjust_final, adjust_final_override, xfer)?
            }
            AiState::MoveAndEvacuate { mv, origin } => move_states::evacuate_xfer(mv, origin, xfer)?,
            AiState::MoveAndDelete { mv, append_goal_position } => move_states::delete_xfer(mv, append_goal_position, xfer)?,
            AiState::FollowWaypointPath { mv, wp, machine, .. } if machine.is_some() => attack_move::attack_follow_xfer(mv, wp, machine, xfer)?,
            AiState::FollowWaypointPath { mv, wp, .. } => waypoint_states::waypoint_path_xfer(mv, wp, xfer)?,
            AiState::FollowWaypointPathExact { mv, last, .. } => waypoint_states::exact_xfer(mv, last, xfer)?,
            AiState::Attack { machine, original_victim_pos, follow, attacking_object, force_attacking, kinds, .. } => {
                attack_states::attack_xfer(machine, original_victim_pos, *follow, *attacking_object, *force_attacking, *kinds, xfer)?
            }
            AiState::AttackApproach { mv, chase } | AiState::AttackPursue { mv, chase } => attack_states::chase_xfer(mv, chase, xfer)?,
            AiState::AttackAim { can_turn_in_place, set_locomotor, .. } => attack_states::aim_xfer(can_turn_in_place, set_locomotor, xfer)?,
            AiState::AttackMove { mv, frame_to_sleep_until, retry_count, machine, .. } => {
                attack_move::attack_move_xfer(mv, frame_to_sleep_until, retry_count, machine, xfer)?
            }
            AiState::PickUpCrate { mv, delay_counter } => attack_move::crate_xfer(mv, delay_counter, xfer)?,
            AiState::Hunt { kinds, machine, next_enemy_scan_time, .. } => hunt_states::hunt_xfer(*kinds, machine, next_enemy_scan_time, xfer)?,
            AiState::Exit { entry_to_clear, .. } => face_exit::exit_xfer(entry_to_clear, xfer)?,
            AiState::Enter { mv, entry_to_clear } => enter_state::enter_xfer(mv, entry_to_clear, xfer)?,
            AiState::Rappel(d) => rappel::rappel_xfer(d, xfer)?,
            AiState::AttackSquad { kinds, machine } => hunt_states::squad_xfer(*kinds, machine, xfer)?,
            AiState::Face { can_turn_in_place, .. } => face_exit::face_xfer(can_turn_in_place, xfer)?,
            AiState::Wander { mv, wp, wait_frames, timer } | AiState::Panic { mv, wp, wait_frames, timer } => {
                waypoint_states::wander_xfer(mv, wp, wait_frames, timer, xfer)?
            }
            AiState::WanderInPlace { mv, origin, wait_frames, timer } => {
                waypoint_states::wander_in_place_xfer(mv, origin, wait_frames, timer, xfer)?
            }
            AiState::MoveAndTighten { mv, ok_to_repath_times, check_for_path }
            | AiState::MoveAwayFromRepulsors { mv, ok_to_repath_times, check_for_path } => {
                move_states::tighten_xfer(mv, ok_to_repath_times, check_for_path, xfer)?
            }
            _ => {
                // ("snapshot interface STUBBED": the version only)
                let current_version: XferVersion = 1;
                let mut version = current_version;
                xfer.xfer_version(&mut version, current_version)?;
            }
        }
        Ok(())
    }

    fn load_post_process(&mut self) {}
}

// ---------------------------------------------------------------------------
// The AI with its machine
// ---------------------------------------------------------------------------

/// `AIUpdateInterface` with its state machine: the AI's state ([`AiUpdate`], lent to the host while the machine runs) and the
/// machine (none until `onObjectCreated`).
pub struct AiUpdateModule<H: AiStateHost> {
    /// The AI's state; `None` only while lent.
    pub ai: Option<AiUpdate>,
    /// `m_stateMachine`.
    pub machine: Option<StateMachine<AiState<H>>>,
}

impl<H: AiStateHost> AiUpdateModule<H> {
    /// `AIUpdateInterface(thing, moduleData)`: the AI's state (the machine is made by `onObjectCreated`).
    pub fn new(h: &mut H, data: Arc<AiUpdateModuleData>) -> Self {
        AiUpdateModule { ai: Some(AiUpdate::new(h, data)), machine: None }
    }

    /// The AI's state.
    pub fn ai(&self) -> &AiUpdate {
        self.ai.as_ref().expect("the AI (not lent)")
    }

    /// The AI's state, mutably.
    pub fn ai_mut(&mut self) -> &mut AiUpdate {
        self.ai.as_mut().expect("the AI (not lent)")
    }

    /// Runs `f` on the machine with the AI lent to the host.
    pub fn run<R>(&mut self, h: &mut H, f: impl FnOnce(&mut StateMachine<AiState<H>>, &mut H) -> R) -> R {
        h.ai_stack().push(self.ai.take().expect("the AI (not lent)"));
        let r = f(self.machine.as_mut().expect("the AI's machine"), h);
        self.ai = Some(h.ai_stack().pop().expect("the lent AI"));
        r
    }

    /// `onObjectCreated()`: "create the behavior state machine" (`makeStateMachine()`), then its default state.
    pub fn on_object_created(&mut self, h: &mut H) -> SmResult<()> {
        if self.machine.is_none() {
            let mut sm = StateMachine::new();
            // (batch A10a: the owner's kinds that make the attack machines)
            let kinds = AttackOwnerKinds {
                immobile: h.is_kind_of(KINDOF_IMMOBILE),
                portable_that_attacks: h.is_kind_of(KINDOF_PORTABLE_STRUCTURE) && h.is_kind_of(KINDOF_CAN_ATTACK),
            };
            define_states(&mut sm, kinds)?;
            // (batch A11: the attack-move machine enters its default state, as its state's constructor does upstream)
            let ai_lent = self.ai.take().expect("the AI (not lent)");
            h.ai_stack().push(ai_lent);
            let r = attack_move::init_attack_move_machines(&mut sm, h);
            self.ai = Some(h.ai_stack().pop().expect("the lent AI"));
            r?;
            self.machine = Some(sm);
            self.run(h, |sm, h| sm.init_default_state(h).map(|_| ()))?;
        }
        Ok(())
    }

    /// `aiDoCommand(parms)` (batch A5, [`commands::ai_do_command`]) on the AI and its machine.
    pub fn ai_do_command(&mut self, h: &mut H, parms: &AiCommandParms) -> SmResult<()> {
        self.run(h, |sm, h| commands::ai_do_command(sm, h, parms))
    }

    /// `doPathfind(pathfinder)` (batch A6, [`crate::logic::ai::ai_paths::do_pathfind`]): the AI's turn in the pathfinder's queue.
    pub fn do_pathfind(&mut self, h: &mut H) {
        self.run(h, crate::logic::ai::ai_paths::do_pathfind);
    }

    /// `update()`: "The "main loop" of the AI subsystem".
    pub fn update(&mut self, h: &mut H) -> SmResult<UpdateSleepTime> {
        self.ai_mut().is_in_update = true;
        // "Reset so state machine update can set it if we just completed the path."
        self.ai_mut().completed_waypoint = None;

        // "assume we can sleep forever, unless the state machine (or turret, etc) demand otherwise"
        let mut sub_machine_sleep = UPDATE_SLEEP_FOREVER;

        let st_ret = self.run(h, aism_update_state_machine)?;
        if st_ret.is_sleep() {
            let frames = st_ret.sleep_frames();
            if frames < sub_machine_sleep.0 {
                sub_machine_sleep = update_sleep(frames);
            }
        } else {
            // "it's STATE_CONTINUE, STATE_SUCCESS, or STATE_FAILURE, any of which will probably require next frame"
            sub_machine_sleep = UPDATE_SLEEP_NONE;
        }

        if self.ai().movement_complete {
            let a = self.ai_mut();
            a.set_queue_for_path_time(h, 0);
            // "destroy path"
            a.destroy_path();
            a.set_locomotor_goal_none();
            h.clear_model_condition_state(MODELCONDITION_MOVING);

            if let Some(mut goal_pos) = h.pf_goal_position() {
                // "Pop to goal - This shouldn't happen (often), but make sure we got to where we're going."
                let pos = h.position();
                let dx = goal_pos.x - pos.x;
                let dy = goal_pos.y - pos.y;
                if dx * dx + dy * dy >= PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
                    // "Too far, so just grid current pos."
                    goal_pos = pos;
                    h.pf_snap_position(&mut goal_pos);
                }
                self.ai_mut().set_final_position(&goal_pos);
                let layer = h.layer();
                h.pf_update_goal(&goal_pos, layer);
            }
            let a = self.ai_mut();
            a.movement_complete = false;
            a.ignore_obstacle(INVALID_ID);
        }

        let now = StateContext::frame(h);
        let queue_for_path_frame = self.ai().queue_for_path_frame;
        if queue_for_path_frame != 0 {
            if now >= queue_for_path_frame {
                h.pf_queue_for_path();
                self.ai_mut().set_queue_for_path_time(h, 0);
            } else {
                let sleep_for_path_delta = queue_for_path_frame - now;
                if sleep_for_path_delta < sub_machine_sleep.0 {
                    sub_machine_sleep = update_sleep(sleep_for_path_delta);
                }
            }
        }

        if !h.is_effectively_dead()
            && !h.is_disabled_by_type(DISABLED_PARALYZED)
            && !h.is_disabled_by_type(DISABLED_UNMANNED)
            && !h.is_disabled_by_type(DISABLED_EMP)
            && !h.is_disabled_by_type(DISABLED_SUBDUED)
            && !h.is_disabled_by_type(DISABLED_HACKED)
        {
            // "If we are dead, don't let the turrets do anything anymore, or else they will keep attacking"
            for i in 0..2 {
                if self.ai().has_turret_ai[i] {
                    let tmp = h.turret_update(i);
                    if tmp < sub_machine_sleep {
                        sub_machine_sleep = tmp;
                    }
                }
            }
        }

        // "must do death check outside of the state machine update, to avoid corruption"
        let current = self.machine.as_ref().expect("the AI's machine").get_current_state_id();
        if self.ai().is_ai_in_dead_state() && current != AI_DEAD {
            self.run(h, |sm, h| -> SmResult<()> {
                aism_clear(sm, h)?;
                aism_set_state(sm, h, AI_DEAD)?;
                sm.lock();
                Ok(())
            })?;
            // "strangely, dead things need to NOT sleep at all."
            sub_machine_sleep = UPDATE_SLEEP_NONE;
        }

        // "do this objects movement"
        let tmp = self.ai_mut().do_locomotor(h);
        if tmp < sub_machine_sleep {
            sub_machine_sleep = tmp;
        }

        self.ai_mut().is_in_update = false;

        if self.ai().completed_waypoint.is_some() {
            // "sleep NONE here so that it will get reset next frame."
            Ok(UPDATE_SLEEP_NONE)
        } else {
            Ok(sub_machine_sleep)
        }
    }
}

impl<H: AiStateHost> AiUpdateModule<H> {
    /// `m_stateMachine` (made by `onObjectCreated`).
    pub fn machine(&self) -> &StateMachine<AiState<H>> {
        self.machine.as_ref().expect("the AI's machine")
    }
}
