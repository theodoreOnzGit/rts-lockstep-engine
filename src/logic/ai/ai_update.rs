// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (AIUpdateInterface: the members listed below)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AIUpdate.h (AIUpdateInterface's data members and inlines)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `AIUpdateInterface`, the AI of a unit: its state and the members that do not need its state machine (`p1w13-sonnet`, layer 10,
//! batch A2). The machine (`AIStateMachine`), its first states and the update are `ai_states` (batch A4); the turrets' `TurretAI`s
//! and the commands come in later batches.
//!
//! # What is here
//!
//! The constructor's state; the locomotors (`chooseLocomotorSet`, `chooseLocomotorSetExplicit`,
//! `chooseGoodLocomotorFromCurrentSet`, `setLocomotorUpgrade`, `getCurLocomotorSpeed`, `hasLocomotorForSurface`); the locomotor's
//! goal (`setLocomotorGoal*`, `getLocomotorDistanceToGoal`, `doLocomotor`); the movement queries (`isDoingGroundMovement`,
//! `isAircraftThatAdjustsDestination`, `getTreatAsAircraftForLocoDistToGoal`, `isValidLocomotorPosition`); the path's bookkeeping
//! (`destroyPath`, `friend_startingMove`, `friend_endingMove`, `friend_setPath`); the sleep (`setQueueForPathTime`, `wakeUpNow`); the
//! waypoint queue; the plain setters and getters of the header.
//!
//! # The host
//!
//! [`AiHost`] is the AI's object and the world as these members reach them: the object (it is the [`LocomotorHost`] the locomotors
//! drive and the [`PathHost`] the path asks), its sleepy update's wake frame, its group, the pathfinder's three answers the AI asks
//! for itself, the partition's distance, and the store a new locomotor is made from. The World implements it once the AI is a module
//! of it. The members that ask the state machine take it as a parameter ([`AiMachineQuery`]: the real machine since batch A4).
//!
//! # Upstream facts kept
//!
//! - `setQueueForPathTime` compares `frames >= UPDATE_SLEEP_NONE` (1) and the RELATIVE wake frame with `UPDATE_SLEEP(frames)`.
//! - `chooseGoodLocomotorFromCurrentSet` keeps the previous locomotor when the cell has none, and takes the set's ground locomotor
//!   when there was none before; a change resets the new locomotor's precise-z, no-slow-down and ultra-accurate flags.
//! - `doLocomotor`'s `POSITION_ON_PATH` without a path and not waiting for one only `DEBUG_CRASH`es and moves nothing; with a
//!   waiting path it returns `UPDATE_SLEEP_FOREVER` before the airborne-target flag is updated.
//! - `getLocomotorDistanceToGoal`'s `POSITION_EXPLICIT` is "not yet implemented": 0.
//!
//! Batch A3 adds the victim (`setCurrentVictim`, `getCurrentVictim`, `getCurrentVictimPos`), the mood (`getMoodMatrixValue`,
//! `getMoodMatrixActionAdjustment`, `wakeUpAndAttemptToTarget`, the mood check times, `canAutoAcquireWhileStealthed`,
//! `getNextMoodTarget`), the morale (`evaluateMoraleBonus`), the state queries (`isIdle`, `isAttacking`, `isBusy`,
//! `isClearingMines`, `isMovingAwayFrom`, `isMoving`, `getAIStateType`, `getEnterTarget`), `checkForCrateToPickup` and the plain
//! getters. Upstream facts kept: `checkForCrateToPickup` clears the
//! crate's id BEFORE looking it up (it never finds the crate); `getNextMoodTarget` reads the controlling player unchecked when called
//! by the AI.
//!
//! Verified against upstream's members (cut verbatim, with `Locomotor.cpp` unchanged and batch P1's paths) by
//! `tests/differential_ai_update.rs`: 10,000 programs, 600,000 operations, 0 mismatches in the results, the call logs and the
//! dumps (2026-10-27; plants 3200-3214 and 3220-3229 caught). Since batch A4 the same test runs them on the real machine
//! (`AIStateMachine` and `StateMachine.cpp` on the C++ side, `ai_states` here): see its results.

use crate::common::base_type::{sqr, Coord3D, ICoord2D};
use crate::common::disabled_types::{DisabledType, DISABLED_HELD, DISABLED_UNMANNED};
use crate::common::game_common::{WhichTurretType, LOGICFRAMES_PER_SECOND, TURRET_INVALID};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND};
use crate::common::game_common::{AbleToAttackType, PlayerType, ATTACK_NEW_TARGET};
use crate::common::kind_of::{
    KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT, KINDOF_IGNORED_IN_GUI, KINDOF_IMMOBILE, KINDOF_INFANTRY, KINDOF_PRODUCED_AT_HELIPAD, KINDOF_PROJECTILE,
};
use crate::common::object_status_types::{OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_IS_USING_ABILITY, OBJECT_STATUS_STEALTHED};
use crate::logic::damage::{DamageType, DAMAGE_HEALING};
use crate::logic::ai::deploy_style_ai_update::DeployStyle;
use crate::logic::weapon::WEAPON_ANTI_MINE;
use crate::logic::weapon_bonus::condition;
use crate::common::name_key_generator::NameKeyType;
use crate::common::object_status_types::OBJECT_STATUS_AIRBORNE_TARGET;
use crate::logic::ai::ai_update_data::AiUpdateModuleData;
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::path::{Path, PathHost, PATHFIND_CELL_SIZE_F, PATHFIND_CLOSE_ENOUGH};
use crate::logic::locomotor::{
    LocomotorAppearance, LocomotorHost, LocomotorSet, LocomotorSurfaceTypeMask, LOCOMOTORSURFACE_AIR, LOCOMOTORSURFACE_GROUND,
};
use crate::logic::module::update_module::{update_sleep, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::common::state_machine::{StateBehavior, StateId, StateMachine, INVALID_STATE_ID};
use crate::logic::modules::plant;
use std::sync::Arc;

/// `MAX_WAYPOINTS`.
pub const MAX_WAYPOINTS: usize = 16;

/// `enum LocoGoalType` ("written out in save/load xfer, don't change these numbers").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum LocoGoalType {
    None = 0,
    PositionOnPath = 1,
    PositionExplicit = 2,
    Angle = 3,
}

/// What the AI's members ask of its state machine (`getStateMachine()->...`): the machine itself (batch A4,
/// [`StateMachine`]), or a stand-in in a test.
pub trait AiMachineQuery {
    /// `getCurrentStateID()`.
    fn current_state_id(&self) -> StateId;
    /// `isInIdleState()`.
    fn is_in_idle_state(&self) -> bool;
    /// `isInAttackState()`.
    fn is_in_attack_state(&self) -> bool;
    /// `isInBusyState()`.
    fn is_in_busy_state(&self) -> bool;
    /// The goal object's id (`getGoalObject()` is `findObjectByID` of it).
    fn goal_object_id(&self) -> ObjectId;
    /// `*getGoalPosition()`.
    fn goal_position(&self) -> Coord3D;
}

impl<S: StateBehavior> AiMachineQuery for StateMachine<S> {
    fn current_state_id(&self) -> StateId {
        self.get_current_state_id()
    }
    fn is_in_idle_state(&self) -> bool {
        StateMachine::is_in_idle_state(self)
    }
    fn is_in_attack_state(&self) -> bool {
        StateMachine::is_in_attack_state(self)
    }
    fn is_in_busy_state(&self) -> bool {
        StateMachine::is_in_busy_state(self)
    }
    fn goal_object_id(&self) -> ObjectId {
        StateMachine::goal_object_id(self)
    }
    fn goal_position(&self) -> Coord3D {
        *StateMachine::goal_position(self)
    }
}

/// `AIStateMachine`'s own members (beyond `StateMachine`'s): kept with the AI's state, so that the machine's states reach them
/// through the AI as upstream's reach them through the machine (`AIStateMachine::getGoalPath`, ...).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AiMachineExtras {
    /// `m_goalPath`.
    pub goal_path: Vec<Coord3D>,
    /// `m_goalWaypoint`, by id.
    pub goal_waypoint: Option<u32>,
    /// `m_temporaryState`'s id (`INVALID_STATE_ID`: none).
    pub temporary_state: StateId,
    /// `m_temporaryStateFramEnd`.
    pub temporary_state_frame_end: u32,
    /// `m_goalSquad` (batch A5).
    pub goal_squad: Option<crate::logic::world::player::Squad>,
}

/// What the AI's members reach outside themselves: the object, its world and its subsystems (see the module docs).
pub trait AiHost: LocomotorHost + PathHost {
    /// `getObject()->getID()`.
    fn object_id(&self) -> ObjectId;
    /// `getObject()->isDisabledByType(t)`.
    fn is_disabled_by_type(&self, t: DisabledType) -> bool;
    /// `getObject()->getCrusherLevel()`.
    fn crusher_level(&self) -> i32;
    /// `getObject()->getPhysics()->getAllowToFall()`, `None` without a physics behaviour.
    fn physics_allow_to_fall(&self) -> Option<bool>;
    /// `getObject()->getHeightAboveTerrain()`.
    fn height_above_terrain(&mut self) -> f32;
    /// `getObject()->setEffectivelyDead(dead)`.
    fn set_effectively_dead(&mut self, dead: bool);
    /// `getWakeFrame()` of the AI's update module: frames until its next call.
    fn wake_frame(&self) -> UpdateSleepTime;
    /// `setWakeFrame(getObject(), wakeDelay)`.
    fn set_wake_frame(&mut self, wake_delay: UpdateSleepTime);
    /// `if (getGroup()) getGroup()->recomputeGroupSpeed()`.
    fn group_recompute_speed(&mut self);
    /// `TheAI->pathfinder()->chooseBestLocomotorForPosition(layer, set, pos)`'s cell: the surfaces it accepts there (the set's
    /// `findLocomotor` of them is the answer).
    fn pf_surfaces_for_position(&mut self, layer: PathfindLayer, pos: &Coord3D) -> LocomotorSurfaceTypeMask;
    /// `TheAI->pathfinder()->updateLayer(getObject(), layer)`.
    fn pf_update_layer(&mut self, layer: PathfindLayer);
    /// `TheAI->pathfinder()->validMovementPosition(isCrusher, layer, locomotorSet, pos)`.
    fn pf_valid_movement_position(&mut self, is_crusher: bool, layer: PathfindLayer, surfaces: LocomotorSurfaceTypeMask, pos: &Coord3D) -> bool;
    /// `ThePartitionManager->getDistanceSquared(getObject(), dest, FROM_CENTER_3D)`.
    fn distance_squared_3d(&mut self, dest: &Coord3D) -> f32;
    /// `set.addLocomotor(lt)`: `TheLocomotorStore->newLocomotor(lt)` appended (none for a template the store does not have).
    fn add_locomotor(&mut self, set: &mut LocomotorSet, template: NameKeyType);

    // ---- batch A3 ----
    /// `TheGameLogic->findObjectByID(id) != NULL`.
    fn object_exists(&self, id: ObjectId) -> bool;
    /// `findObjectByID(target)->getAI()`, if it has one, `->addTargeter(targeter, add)`.
    fn ai_add_targeter(&mut self, target: ObjectId, targeter: ObjectId, add: bool);
    /// `getObject()->isEffectivelyDead()`.
    fn is_effectively_dead(&self) -> bool;
    /// `getObject()->getControllingPlayer()->getPlayerType()`, `None` without a controlling player.
    fn controlling_player_type(&self) -> Option<PlayerType>;
    /// `getObject()->getTeam()->getPrototype()->getTemplateInfo()->m_attackCommonTarget`.
    fn team_attack_common_target(&self) -> bool;
    /// `getObject()->getTeam()->getTeamTargetObject()`'s id (`INVALID_ID`: none).
    fn team_target_object(&self) -> ObjectId;
    /// `getObject()->getAbleToAttackSpecificObject(type, target, commandSource)`.
    fn able_to_attack_specific_object(&mut self, t: AbleToAttackType, target: ObjectId, command_source: CommandSourceType) -> CanAttackResultValue;
    /// `getObject()->getStealth()->isGrantedBySpecialPower()`, `None` without a stealth module.
    fn stealth_granted_by_special_power(&self) -> Option<bool>;
    /// `getObject()->getContainedBy()`'s id (`INVALID_ID`: none).
    fn contained_by(&self) -> ObjectId;
    /// `getContainedBy()->getContain()->isPassengerAllowedToFire()`.
    fn container_passenger_allowed_to_fire(&self) -> bool;
    /// `getContainedBy()->getGeometryInfo().getBoundingCircleRadius()`.
    fn container_bounding_circle_radius(&self) -> f32;
    /// `getObject()->getBodyModule()->getLastDamageInfo()->in`'s damage type and source.
    fn last_damage_type_and_source(&self) -> (DamageType, ObjectId);
    /// `TheAI->getAiData()->m_forceIdleFramesCount`.
    fn ai_data_force_idle_frames(&self) -> u32;
    /// `TheAI->getAiData()->m_attackUsesLineOfSight`.
    fn ai_data_attack_uses_line_of_sight(&self) -> bool;
    /// `TheAI->getAiData()->m_attackIgnoreInsignificantBuildings`.
    fn ai_data_attack_ignore_insignificant_buildings(&self) -> bool;
    /// `TheAI->getAdjustedVisionRangeForObject(getObject(), factors)`.
    fn adjusted_vision_range_for_object(&mut self, factors: i32) -> f32;
    /// `TheAI->findClosestEnemy(getObject(), range, qualifiers, info, filter)`'s id (`INVALID_ID`: none); `area`: a
    /// `PartitionFilterPolygonTrigger` of that polygon trigger (batch A12).
    fn find_closest_enemy(&mut self, range: f32, qualifiers: u32, info: Option<usize>, area: Option<i32>) -> ObjectId;
    /// `getObject()->getCurrentWeapon()->getAntiMask()`, `None` without a current weapon.
    fn current_weapon_anti_mask(&self) -> Option<i32>;
    /// `player->hasUpgradeComplete(TheUpgradeCenter->findUpgrade(name))` for the controlling player.
    fn player_has_upgrade_complete(&self, name: &str) -> bool;
    /// The object's behavior modules' horde interfaces, in order: `(isInHorde(), isAllowedNationalism())` of each that has one.
    fn horde_update_interfaces(&self) -> Vec<(bool, bool)>;
    /// `getObject()->setWeaponBonusCondition(c)`.
    fn set_weapon_bonus_condition(&mut self, c: i32);
    /// `getObject()->clearWeaponBonusCondition(c)`.
    fn clear_weapon_bonus_condition(&mut self, c: i32);
    /// `GameLogicRandomValue(lo, hi)`.
    fn logic_random_value(&mut self, lo: i32, hi: i32) -> i32;
}

/// `class AIUpdateInterface`'s state (its data members, in the header's order), and `AIStateMachine`'s own members
/// ([`AiMachineExtras`], batch A4; the machine itself is `ai_states::AiUpdateModule`'s). The turrets' AIs come with their batch.
#[derive(Clone, Debug)]
pub struct AiUpdate {
    /// The module's data (`getAIUpdateModuleData()`).
    pub data: Arc<AiUpdateModuleData>,
    pub prior_waypoint_id: u32,
    pub current_waypoint_id: u32,
    pub next_enemy_scan_time: u32,
    pub current_victim_id: ObjectId,
    pub desired_speed: f32,
    pub last_command_source: CommandSourceType,
    pub guard_mode: GuardMode,
    pub guard_target_type: [GuardTargetType; 2],
    pub location_to_guard: Coord3D,
    pub object_to_guard: ObjectId,
    /// `m_areaToGuard` (a polygon trigger), by id.
    pub area_to_guard: Option<i32>,
    /// `m_attackInfo` (an attack priority info), by index.
    pub attack_info: Option<usize>,
    pub waypoint_queue: [Coord3D; MAX_WAYPOINTS],
    pub waypoint_count: i32,
    pub waypoint_index: i32,
    /// `m_completedWaypoint`, by id.
    pub completed_waypoint: Option<u32>,
    pub path: Option<Path>,
    pub requested_victim_id: ObjectId,
    pub requested_destination: Coord3D,
    pub requested_destination2: Coord3D,
    pub path_timestamp: u32,
    pub ignore_obstacle_id: ObjectId,
    pub path_extra_distance: f32,
    pub pathfind_goal_cell: ICoord2D,
    pub pathfind_cur_cell: ICoord2D,
    pub blocked_frames: i32,
    pub cur_max_blocked_speed: f32,
    pub bump_speed_limit: f32,
    pub ignore_collisions_until: u32,
    /// "NEVER EVER EVER set this directly. you must call setQueueForPathTime() in ALL CASES."
    pub queue_for_path_frame: u32,
    pub final_position: Coord3D,
    pub repulsor1: ObjectId,
    pub repulsor2: ObjectId,
    pub next_goal_path_index: i32,
    pub move_out_of_way1: ObjectId,
    pub move_out_of_way2: ObjectId,
    pub locomotor_set: LocomotorSet,
    /// `m_curLocomotor`: an index into `locomotor_set`.
    pub cur_locomotor: Option<usize>,
    pub cur_locomotor_set: LocomotorSetType,
    pub locomotor_goal_type: LocoGoalType,
    pub locomotor_goal_data: Coord3D,
    pub turret_sync_flag: WhichTurretType,
    /// `m_turretAI[i] != NULL`: the constructor makes one per turret of the data (batch A3; the turrets' AIs themselves join with
    /// their batch).
    pub has_turret_ai: [bool; 2],
    pub attitude: AttitudeType,
    pub next_mood_check_time: u32,
    pub crate_created: ObjectId,
    pub tmp_int: i32,
    pub do_final_position: bool,
    pub waiting_for_path: bool,
    pub is_attack_path: bool,
    pub is_final_goal: bool,
    pub is_approach_path: bool,
    pub is_safe_path: bool,
    pub movement_complete: bool,
    pub is_moving: bool,
    pub is_blocked: bool,
    pub is_blocked_and_stuck: bool,
    pub upgraded_locomotors: bool,
    pub can_path_through_units: bool,
    pub randomly_offset_mood_check: bool,
    pub is_ai_dead: bool,
    pub is_recruitable: bool,
    pub executing_waypoint_queue: bool,
    pub retry_path: bool,
    /// `m_allowedToChase`: the constructor leaves it unset (an indeterminate `Bool` upstream); false here.
    pub allowed_to_chase: bool,
    pub is_in_update: bool,
    pub fix_loco_in_post_process: bool,
    /// `AIStateMachine`'s own members (batch A4).
    pub machine: AiMachineExtras,
    /// The module's class (batch A30): its overrides are taken where it is not `AIUpdateInterface` itself.
    pub subclass: AiSubclass,
}

/// Which `AIUpdateInterface` the module is (batch A30): the base, or a subclass whose overrides `ai_states` and `ai_commands`
/// take where upstream's virtual members differ.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AiSubclass {
    /// `AIUpdateInterface`.
    #[default]
    Plain,
    /// `WanderAIUpdate`: when idle, a random move of 5 to 50 in x and y; never asleep.
    Wander,
    /// `TransportAIUpdate`: the passengers allowed to fire attack what the transport is told to attack.
    Transport,
    /// `DeployStyleAIUpdate` (batch A31): unpacks before it attacks, packs up before it moves (its members and data).
    DeployStyle(DeployStyle),
}

impl AiUpdate {
    /// `AIUpdateInterface(thing, moduleData)`: the state, then `chooseLocomotorSet(LOCOMOTORSET_NORMAL)` and
    /// `setWakeFrame(UPDATE_SLEEP_NONE)` (the turrets' AIs are made by their batch).
    pub fn new<H: AiHost>(h: &mut H, data: Arc<AiUpdateModuleData>) -> Self {
        let zero = Coord3D::default();
        let mut ai = AiUpdate {
            data,
            prior_waypoint_id: 0xfacade,
            current_waypoint_id: 0xfacade,
            next_enemy_scan_time: 0,
            current_victim_id: INVALID_ID,
            desired_speed: FAST_AS_POSSIBLE,
            last_command_source: CMD_FROM_AI,
            guard_mode: GUARDMODE_NORMAL,
            guard_target_type: [GUARDTARGET_NONE, GUARDTARGET_NONE],
            location_to_guard: zero,
            object_to_guard: INVALID_ID,
            area_to_guard: None,
            attack_info: None,
            waypoint_queue: [zero; MAX_WAYPOINTS],
            waypoint_count: 0,
            waypoint_index: 0,
            completed_waypoint: None,
            path: None,
            requested_victim_id: INVALID_ID,
            requested_destination: zero,
            requested_destination2: zero,
            path_timestamp: 0,
            ignore_obstacle_id: INVALID_ID,
            path_extra_distance: 0.0,
            pathfind_goal_cell: ICoord2D { x: -1, y: -1 },
            pathfind_cur_cell: ICoord2D { x: -1, y: -1 },
            blocked_frames: 0,
            cur_max_blocked_speed: 0.0,
            bump_speed_limit: FAST_AS_POSSIBLE,
            ignore_collisions_until: 0,
            queue_for_path_frame: 0,
            final_position: zero,
            repulsor1: INVALID_ID,
            repulsor2: INVALID_ID,
            next_goal_path_index: -1,
            move_out_of_way1: INVALID_ID,
            move_out_of_way2: INVALID_ID,
            locomotor_set: LocomotorSet::new(),
            cur_locomotor: None,
            cur_locomotor_set: LOCOMOTORSET_INVALID,
            locomotor_goal_type: LocoGoalType::None,
            locomotor_goal_data: zero,
            turret_sync_flag: TURRET_INVALID,
            has_turret_ai: [false; 2],
            attitude: AI_NORMAL,
            next_mood_check_time: 0,
            crate_created: INVALID_ID,
            tmp_int: 0,
            do_final_position: false,
            waiting_for_path: false,
            is_attack_path: false,
            is_final_goal: false,
            is_approach_path: false,
            is_safe_path: false,
            movement_complete: false,
            is_moving: false,
            is_blocked: false,
            is_blocked_and_stuck: false,
            upgraded_locomotors: false,
            can_path_through_units: false,
            randomly_offset_mood_check: false,
            is_ai_dead: false,
            is_recruitable: true,
            executing_waypoint_queue: false,
            retry_path: false,
            allowed_to_chase: false,
            is_in_update: false,
            fix_loco_in_post_process: false,
            machine: AiMachineExtras { temporary_state: INVALID_STATE_ID, ..AiMachineExtras::default() },
            subclass: AiSubclass::Plain,
        };
        for i in 0..2 {
            ai.has_turret_ai[i] = ai.data.turret_data[i].is_some();
        }
        ai.choose_locomotor_set(h, LOCOMOTORSET_NORMAL);
        h.set_wake_frame(UPDATE_SLEEP_NONE);
        ai
    }

    // ---- the header's inlines ----

    /// `isAiInDeadState()`.
    pub fn is_ai_in_dead_state(&self) -> bool {
        self.is_ai_dead
    }
    /// `isWaitingForPath()`.
    pub fn is_waiting_for_path(&self) -> bool {
        self.waiting_for_path
    }
    /// `getPathExtraDistance()`.
    pub fn get_path_extra_distance(&self) -> f32 {
        self.path_extra_distance
    }
    /// `setPathExtraDistance(dist)`.
    pub fn set_path_extra_distance(&mut self, dist: f32) {
        self.path_extra_distance = dist;
    }
    /// `getCurLocomotor()`.
    pub fn get_cur_locomotor(&self) -> Option<&crate::logic::locomotor::Locomotor> {
        self.cur_locomotor.and_then(|i| self.locomotor_set.get(i))
    }
    /// `setFinalPosition(pos)`: the position, and no move towards it.
    pub fn set_final_position(&mut self, pos: &Coord3D) {
        self.final_position = *pos;
        self.do_final_position = false;
    }
    /// `setCanPathThroughUnits(canPath)`.
    pub fn set_can_path_through_units(&mut self, can_path: bool) {
        self.can_path_through_units = can_path;
        if can_path {
            self.is_blocked_and_stuck = false;
        }
    }
    /// `setIgnoreCollisionTime(frames)`.
    pub fn set_ignore_collision_time(&mut self, now: u32, frames: i32) {
        self.ignore_collisions_until = now.wrapping_add(frames as u32);
    }

    // ---- AIUpdate.cpp ----

    /// `setLocomotorUpgrade(set)`.
    pub fn set_locomotor_upgrade<H: AiHost>(&mut self, h: &mut H, set: bool) {
        self.upgraded_locomotors = set;
        if self.cur_locomotor_set == LOCOMOTORSET_NORMAL || self.cur_locomotor_set == LOCOMOTORSET_NORMAL_UPGRADED {
            self.choose_locomotor_set(h, LOCOMOTORSET_NORMAL);
        }
    }

    /// `chooseLocomotorSet(wst)`: the normal set is the upgraded one once upgraded; a new set takes a good locomotor of it.
    pub fn choose_locomotor_set<H: AiHost>(&mut self, h: &mut H, mut wst: LocomotorSetType) -> bool {
        if wst == LOCOMOTORSET_NORMAL && self.upgraded_locomotors && !plant(3200) {
            wst = LOCOMOTORSET_NORMAL_UPGRADED;
        }

        if wst == self.cur_locomotor_set {
            return true;
        }

        if self.choose_locomotor_set_explicit(h, wst) {
            self.choose_good_locomotor_from_current_set(h);
            return true;
        }

        false
    }

    /// `chooseLocomotorSetExplicit(wst)`: "it does no sanity checking; it just jams it in" (false if the data has no such set).
    pub fn choose_locomotor_set_explicit<H: AiHost>(&mut self, h: &mut H, wst: LocomotorSetType) -> bool {
        let Some(set) = self.data.find_locomotor_template_vector(wst) else { return false };
        self.locomotor_set.clear();
        if !plant(3211) {
            self.cur_locomotor = None;
        }
        for lt in set {
            h.add_locomotor(&mut self.locomotor_set, lt);
        }
        self.cur_locomotor_set = wst;
        true
    }

    /// `chooseGoodLocomotorFromCurrentSet()`.
    pub fn choose_good_locomotor_from_current_set<H: AiHost>(&mut self, h: &mut H) {
        let prev_loco = self.cur_locomotor;

        let layer = h.layer();
        let pos = h.position();
        let surfaces = h.pf_surfaces_for_position(layer, &pos);
        let mut new_loco = self.locomotor_set.find_locomotor(surfaces);

        if new_loco.is_none() {
            if prev_loco.is_some() && !plant(3201) {
                // "due to physics, we might slight into a cell for which we have no loco (eg, cliff) and get stuck."
                new_loco = prev_loco;
            } else {
                // "this can happen for a newly-created object, which might come into being in the middle of an obstacle."
                new_loco = self.locomotor_set.find_locomotor(LOCOMOTORSURFACE_GROUND);
            }
        }

        self.cur_locomotor = new_loco;

        if prev_loco != self.cur_locomotor {
            // "make sure the group's speed will be recalculated"
            h.group_recompute_speed();

            let loco = self.locomotor_set.get_mut(self.cur_locomotor.expect("a locomotor changed to")).expect("in the set");
            loco.set_use_precise_z_pos(false);
            loco.set_no_slow_down_as_approaching_dest(false);
            loco.set_ultra_accurate(plant(3202));
        }
    }

    /// `getCurLocomotorSpeed()`: the current locomotor's top speed in the body's damage state (0 without one).
    pub fn get_cur_locomotor_speed<H: AiHost>(&self, h: &H) -> f32 {
        match self.get_cur_locomotor() {
            Some(l) => l.get_max_speed_for_condition(h, h.damage_state()),
            None => 0.0, // "no current locomotor!"
        }
    }

    /// `hasLocomotorForSurface(surfaceType)`.
    pub fn has_locomotor_for_surface(&self, surface_type: LocomotorSurfaceTypeMask) -> bool {
        self.locomotor_set.find_locomotor(surface_type).is_some()
    }

    /// `setQueueForPathTime(frames)`: wake up by then (unless in the update, which computes its own sleep), and the frame.
    pub fn set_queue_for_path_time<H: AiHost>(&mut self, h: &mut H, frames: i32) {
        if frames >= UPDATE_SLEEP_NONE.0 as i32 + i32::from(plant(3209)) && h.wake_frame() > update_sleep(frames as u32) && !self.is_in_update {
            // (in our own update: "just do nothing, since update will calculate the correct sleep behavior at the end")
            h.set_wake_frame(update_sleep(frames as u32));
        }
        self.queue_for_path_frame = if frames != 0 { h.frame().wrapping_add(frames as u32) } else { 0 };
    }

    /// `wakeUpNow()`.
    pub fn wake_up_now<H: AiHost>(&mut self, h: &mut H) {
        if h.wake_frame() > UPDATE_SLEEP_NONE && !self.is_in_update && !plant(3203) {
            h.set_wake_frame(UPDATE_SLEEP_NONE);
        }
    }

    /// `friend_notifyStateMachineChanged()`.
    pub fn friend_notify_state_machine_changed<H: AiHost>(&mut self, h: &mut H) {
        self.wake_up_now(h);
    }

    /// `queueWaypoint(pos)`: appended while the queue has room.
    pub fn queue_waypoint(&mut self, pos: &Coord3D) -> bool {
        if (self.waypoint_count as usize) < MAX_WAYPOINTS - usize::from(plant(3212)) {
            self.waypoint_queue[self.waypoint_count as usize] = *pos;
            self.waypoint_count += 1;
            return true;
        }
        false
    }

    /// `executeWaypointQueue()`: "the dead don't listen very well".
    pub fn execute_waypoint_queue(&mut self) {
        if self.is_ai_in_dead_state() && !plant(3213) {
            return;
        }
        if self.waypoint_count > 0 {
            self.waypoint_index = 0;
            self.executing_waypoint_queue = true;
        }
    }

    /// `clearWaypointQueue()`.
    pub fn clear_waypoint_queue(&mut self) {
        self.waypoint_count = 0;
        self.executing_waypoint_queue = false;
    }

    /// `destroyPath()`.
    pub fn destroy_path(&mut self) {
        self.path = None;
        self.waiting_for_path = false;
        self.is_attack_path = false;
        self.set_locomotor_goal_none();
    }

    /// `friend_startingMove()`.
    pub fn friend_starting_move(&mut self) {
        self.movement_complete = false;
        self.is_moving = true;
        self.blocked_frames = 0;
        self.is_blocked_and_stuck = false;
    }

    /// `friend_endingMove()`.
    pub fn friend_ending_move(&mut self) {
        self.movement_complete = true;
        self.is_moving = false;
    }

    /// `friend_setPath(path)`.
    pub fn friend_set_path(&mut self, path: Option<Path>) {
        self.destroy_path();
        self.path = path;
    }

    /// `isValidLocomotorPosition(pos)`.
    pub fn is_valid_locomotor_position<H: AiHost>(&self, h: &mut H, pos: &Coord3D) -> bool {
        let crusher = h.crusher_level() > 0;
        let layer = h.layer();
        h.pf_valid_movement_position(crusher, layer, self.locomotor_set.get_valid_surfaces(), pos)
    }

    /// `doLocomotor()`: the drive forces of the frame; how long the locomotor may sleep.
    pub fn do_locomotor<H: AiHost>(&mut self, h: &mut H) -> UpdateSleepTime {
        if h.is_kind_of(KINDOF_IMMOBILE) {
            return UPDATE_SLEEP_FOREVER;
        }

        self.choose_good_locomotor_from_current_set(h);

        if self.is_blocked {
            self.blocked_frames += 1;
        } else {
            self.blocked_frames = 0;
        }

        self.is_blocked = false;

        let mut blocked = self.blocked_frames > 0;
        let mut requires_constant_calling = true; // "assume the worst."

        if let Some(cur) = self.cur_locomotor {
            self.locomotor_set.get(cur).expect("the current locomotor").set_physics_options(h);

            let works_when_dead = self.locomotor_set.get(cur).expect("the current locomotor").get_locomotor_works_when_dead();
            if self.is_ai_in_dead_state() && !works_when_dead {
                // "now it's over, I'm dead, and I haven't done anything that I want"
            } else {
                match self.locomotor_goal_type {
                    LocoGoalType::PositionExplicit => {
                        let mut speed = self.desired_speed;
                        let my_max_speed = self.locomotor_set.get(cur).expect("loco").get_max_speed_for_condition(h, h.damage_state());
                        if speed == FAST_AS_POSSIBLE || speed > my_max_speed {
                            speed = my_max_speed;
                        }
                        let goal = self.locomotor_goal_data;
                        self.locomotor_set.get_mut(cur).expect("loco").loco_update_move_towards_position(h, &goal, 0.0, speed, &mut blocked);
                        self.do_final_position = false;
                    }

                    LocoGoalType::PositionOnPath => 'on_path: {
                        if self.path.is_none() {
                            if self.waiting_for_path {
                                return UPDATE_SLEEP_FOREVER; // "Can't move till we get our path."
                            }
                            // DEBUG_CRASH(("must have a path here (doLocomotor)"))
                            break 'on_path;
                        }
                        let goal_pos;
                        let on_path_dist_to_goal;
                        if !self.is_doing_ground_movement(h) {
                            // "airborne locomotor. Get the goal and distance direct to the goal, don't consider obstacles."
                            let pos = h.position();
                            let (d, g) = self.path.as_mut().expect("a path").compute_flight_dist_to_goal(&pos);
                            on_path_dist_to_goal = d;
                            goal_pos = g;
                        } else {
                            // "Compute the actual goal position along the path to move towards."
                            let id = h.object_id();
                            let pos = h.position();
                            let surfaces = self.locomotor_set.get_valid_surfaces();
                            let info = self.path.as_mut().expect("a path").compute_point_on_path(h, id, surfaces, &pos);
                            on_path_dist_to_goal = info.dist_along_path;
                            goal_pos = info.pos_on_path;
                            // "layer is a possible bridge in the path. Check & set the layer if applicable."
                            h.pf_update_layer(info.layer);
                        }

                        let mut speed = self.desired_speed;
                        let my_max_speed = self.locomotor_set.get(cur).expect("loco").get_max_speed_for_condition(h, h.damage_state());
                        if speed == FAST_AS_POSSIBLE || speed > my_max_speed {
                            speed = my_max_speed;
                        }

                        if blocked && speed > self.cur_max_blocked_speed {
                            speed = self.cur_max_blocked_speed;
                            if self.bump_speed_limit > speed {
                                self.bump_speed_limit = speed;
                            }
                            self.bump_speed_limit *= if plant(3205) { 0.96 } else { 0.95 };
                            speed = self.bump_speed_limit;
                        } else {
                            blocked = false;
                            if self.bump_speed_limit < FAST_AS_POSSIBLE {
                                if self.bump_speed_limit < speed * 0.2 {
                                    self.bump_speed_limit = speed * 0.2;
                                }
                                self.bump_speed_limit *= 1.05;
                            }
                            if speed > self.bump_speed_limit {
                                speed = self.bump_speed_limit;
                            }
                        }

                        let extra = self.get_path_extra_distance();
                        self.locomotor_set.get_mut(cur).expect("loco").loco_update_move_towards_position(
                            h,
                            &goal_pos,
                            on_path_dist_to_goal + extra,
                            speed,
                            &mut blocked,
                        );

                        self.do_final_position = false;
                    }

                    LocoGoalType::Angle => {
                        let angle = self.locomotor_goal_data.x;
                        self.locomotor_set.get_mut(cur).expect("loco").loco_update_move_towards_angle(h, angle);
                        self.do_final_position = false;
                    }

                    LocoGoalType::None => {
                        if self.do_final_position {
                            let mut pos = h.position();
                            let on_ground = !h.is_above_terrain() && h.layer() == LAYER_GROUND;
                            let dx = self.final_position.x - pos.x;
                            let dy = self.final_position.y - pos.y;
                            let d_sqr = dx * dx + dy * dy;
                            const DARN_CLOSE: f32 = 0.25;
                            if d_sqr < DARN_CLOSE + if plant(3206) { 0.1 } else { 0.0 } {
                                self.do_final_position = false;
                                if on_ground {
                                    self.final_position.z = h.ground_height(self.final_position.x, self.final_position.y);
                                } else {
                                    self.final_position.z = pos.z;
                                }
                                let fp = self.final_position;
                                h.set_position(&fp);
                            } else {
                                let mut dist = d_sqr.sqrt();
                                if dist < 1.0 {
                                    dist = 1.0;
                                }
                                pos.x += 2.0 * PATHFIND_CELL_SIZE_F * dx / (dist * LOGICFRAMES_PER_SECOND as f32);
                                pos.y += 2.0 * PATHFIND_CELL_SIZE_F * dy / (dist * LOGICFRAMES_PER_SECOND as f32);
                                if on_ground {
                                    pos.z = h.ground_height(pos.x, pos.y);
                                }
                                h.set_position(&pos);
                            }
                        }
                        requires_constant_calling = self.locomotor_set.get_mut(cur).expect("loco").loco_update_maintain_current_position(h);
                    }
                }
            }

            if !blocked && self.blocked_frames > 1 && !plant(3210) {
                self.blocked_frames = 1;
            }

            // "After our movement for the frame, update our AirborneTarget flag."
            let targeting_height = self.locomotor_set.get(cur).expect("loco").get_airborne_targeting_height();
            if h.height_above_terrain() > targeting_height as f32 - if plant(3214) { 0.5 } else { 0.0 } {
                h.set_status(OBJECT_STATUS_AIRBORNE_TARGET, true);
            } else {
                h.set_status(OBJECT_STATUS_AIRBORNE_TARGET, false);
            }

            self.cur_max_blocked_speed = FAST_AS_POSSIBLE;
        }

        if self.cur_locomotor.is_some()
            && self.locomotor_goal_type == LocoGoalType::None
            && !self.do_final_position
            && !self.is_blocked
            && !requires_constant_calling
        {
            UPDATE_SLEEP_FOREVER
        } else {
            UPDATE_SLEEP_NONE
        }
    }

    /// `setLocomotorGoalPositionOnPath()`.
    pub fn set_locomotor_goal_position_on_path(&mut self) {
        self.locomotor_goal_type = LocoGoalType::PositionOnPath;
        self.locomotor_goal_data = Coord3D::default();
    }

    /// `setLocomotorGoalPositionExplicit(newPos)`.
    pub fn set_locomotor_goal_position_explicit(&mut self, new_pos: &Coord3D) {
        self.locomotor_goal_type = LocoGoalType::PositionExplicit;
        self.locomotor_goal_data = *new_pos;
    }

    /// `setLocomotorGoalOrientation(angle)`: the angle in the data's `x`.
    pub fn set_locomotor_goal_orientation(&mut self, angle: f32) {
        self.locomotor_goal_type = LocoGoalType::Angle;
        self.locomotor_goal_data.x = angle;
    }

    /// `setLocomotorGoalNone()`.
    pub fn set_locomotor_goal_none(&mut self) {
        self.locomotor_goal_type = LocoGoalType::None;
    }

    /// `isDoingGroundMovement()`.
    pub fn is_doing_ground_movement<H: AiHost>(&self, h: &mut H) -> bool {
        if h.is_disabled_by_type(DISABLED_UNMANNED) && h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) {
            return true; // "an unmanned helicopter gets grounded, eventually."
        }

        if self.locomotor_set.get_valid_surfaces() == LOCOMOTORSURFACE_AIR {
            return false; // "air only loco."
        }

        let Some(cur) = self.get_cur_locomotor() else {
            return false; // "No loco, so we aren't moving."
        };

        if cur.get_legal_surfaces() & LOCOMOTORSURFACE_AIR != 0 {
            return false; // "Cur loco is air, so not ground."
        }

        if h.is_disabled_by_type(DISABLED_HELD) && !plant(3207) {
            return false; // "We are held, so not moving on ground."
        }

        // "if we're airborne and "allowed to fall", we are probably deliberately in midair due to rappel or accident..."
        if h.is_above_terrain() && h.physics_allow_to_fall() == Some(true) {
            return false;
        }

        true
    }

    /// `isAircraftThatAdjustsDestination()`: a hovering or winged current locomotor.
    pub fn is_aircraft_that_adjusts_destination(&self) -> bool {
        let Some(cur) = self.get_cur_locomotor() else { return false };
        match cur.get_appearance() {
            LocomotorAppearance::LocoHover => true,
            LocomotorAppearance::LocoWings => !plant(3204),
            _ => false,
        }
    }

    /// `getTreatAsAircraftForLocoDistToGoal()`.
    pub fn get_treat_as_aircraft_for_loco_dist_to_goal<H: AiHost>(&self, h: &mut H) -> bool {
        let mut treat_as_aircraft = !self.is_doing_ground_movement(h);
        if self.get_path_extra_distance() > PATHFIND_CLOSE_ENOUGH {
            // "We are following a waypoint or other multiple point path, so use the "easy" success criteria."
            treat_as_aircraft = true;
        }
        if self.get_cur_locomotor().is_some_and(|l| l.get_appearance() == LocomotorAppearance::LocoHover) && !plant(3208) {
            treat_as_aircraft = true; // "Hovercrafts are very sloppy."
        }
        treat_as_aircraft
    }

    /// `getLocomotorDistanceToGoal()`.
    pub fn get_locomotor_distance_to_goal<H: AiHost, M: AiMachineQuery>(&mut self, h: &mut H, m: &M) -> f32 {
        match self.locomotor_goal_type {
            LocoGoalType::PositionExplicit => 0.0, // DEBUG_CRASH(("not yet implemented"))
            LocoGoalType::PositionOnPath => {
                if self.path.is_none() {
                    return 0.0; // DEBUG_CRASH(("must have a path here (getLocomotorDistanceToGoal)"))
                }
                let Some(cur) = self.get_cur_locomotor() else { return 0.0 };
                if cur.is_close_enough_dist_3d() || h.is_kind_of(KINDOF_PROJECTILE) {
                    let mut dest = m.goal_position();
                    let path = self.path.as_ref().expect("a path");
                    if let Some(last) = path.last_node() {
                        dest = *path.node(last).position();
                    }
                    let distance = h.distance_squared_3d(&dest);
                    return (f64::from(distance).sqrt()) as f32;
                }
                let treat_as_aircraft = self.get_treat_as_aircraft_for_loco_dist_to_goal(h);
                let mut goal_pos;
                let dist;
                if treat_as_aircraft {
                    // "airborne locomotor. Get the goal and distance direct to the goal, don't consider obstacles."
                    let pos = h.position();
                    let (d, g) = self.path.as_mut().expect("a path").compute_flight_dist_to_goal(&pos);
                    dist = d;
                    goal_pos = g;
                } else {
                    let id = h.object_id();
                    let pos = h.position();
                    let surfaces = self.locomotor_set.get_valid_surfaces();
                    let info = self.path.as_mut().expect("a path").compute_point_on_path(h, id, surfaces, &pos);
                    goal_pos = info.pos_on_path;
                    dist = info.dist_along_path;
                }
                let path = self.path.as_ref().expect("a path");
                if let Some(last) = path.last_node() {
                    goal_pos = *path.node(last).position();
                }
                // "If the actual distance is farther, then use the actual distance so we get there."
                let pos = h.position();
                let dx = goal_pos.x - pos.x;
                let dy = goal_pos.y - pos.y;
                let dist_sqr = dx * dx + dy * dy;

                if treat_as_aircraft {
                    return if sqr(dist) > dist_sqr { (f64::from(dist_sqr).sqrt()) as f32 } else { dist };
                }

                if dist < PATHFIND_CELL_SIZE_F || sqr(dist) < dist_sqr {
                    dist_sqr.sqrt()
                } else {
                    dist
                }
            }
            // "If it isn't a positional goal, we are there already."
            LocoGoalType::Angle | LocoGoalType::None => 0.0,
        }
    }

    /// `markAsDead()`: dead, the object effectively dead, and awake at once ("so that our anim plays promptly!").
    pub fn mark_as_dead<H: AiHost>(&mut self, h: &mut H) {
        self.is_ai_dead = true;
        h.set_effectively_dead(true);
        self.wake_up_now(h);
    }

    /// `setAttitude(tude)`.
    pub fn set_attitude(&mut self, tude: AttitudeType) {
        self.attitude = tude;
    }

    /// `ignoreObstacleID(id)`.
    pub fn ignore_obstacle_id(&mut self, id: ObjectId) {
        self.ignore_obstacle_id = id;
    }

    /// `setLastCommandSource(source)`.
    pub fn set_last_command_source(&mut self, source: CommandSourceType) {
        self.last_command_source = source;
    }

    // ---- batch A3: the victim, the mood, the morale, the state queries ----

    /// `getNextMoodCheckTime()`.
    pub fn get_next_mood_check_time(&self) -> u32 {
        self.next_mood_check_time
    }
    /// `notifyCrate(id)`.
    pub fn notify_crate(&mut self, id: ObjectId) {
        self.crate_created = id;
    }
    /// `canAutoAcquire()`.
    pub fn can_auto_acquire(&self) -> bool {
        self.data.auto_acquire_enemies_when_idle != 0
    }

    /// `checkForCrateToPickup()`: the crate made by a kill, if it would like to be collided with by us. Upstream clears the id
    /// BEFORE it looks the crate up, so it looks up `INVALID_ID` and never finds one: the answer is always none (kept).
    pub fn check_for_crate_to_pickup<H: AiHost>(&mut self, h: &mut H) -> ObjectId {
        if self.crate_created != INVALID_ID {
            self.crate_created = INVALID_ID; // "we have processed it, so clear it."
            let crate_id = if plant(3220) { 1 } else { self.crate_created };
            if h.object_exists(crate_id) {
                // (the crate's collide modules' wouldLikeToCollideWith: unreachable, see above)
                return crate_id;
            }
        }
        INVALID_ID
    }

    /// `setCurrentVictim(victim)`: none tells the previous victim's AI we no longer target it.
    pub fn set_current_victim<H: AiHost>(&mut self, h: &mut H, victim: ObjectId) {
        if victim == INVALID_ID {
            // "be paranoid, in case we are called from dtors, etc."
            if self.current_victim_id != INVALID_ID && h.object_exists(self.current_victim_id) && !plant(3221) {
                let me = h.object_id();
                h.ai_add_targeter(self.current_victim_id, me, false);
            }
            self.current_victim_id = INVALID_ID;
        } else {
            // "we don't add a targeter here, since we usually want to defer that until we are actually aiming"
            self.current_victim_id = victim;
        }
    }

    /// `getCurrentVictim()`: the victim if it still exists.
    pub fn get_current_victim<H: AiHost>(&self, h: &H) -> ObjectId {
        if self.current_victim_id != INVALID_ID && h.object_exists(self.current_victim_id) {
            return self.current_victim_id;
        }
        INVALID_ID
    }

    /// `getCurrentVictimPos()`: "if we are attacking a position (and NOT an object), return it".
    pub fn get_current_victim_pos<H: AiHost, M: AiMachineQuery>(&self, h: &mut H, m: &M) -> Option<Coord3D> {
        if h.status_test(OBJECT_STATUS_IS_ATTACKING) && self.current_victim_id == INVALID_ID {
            return Some(m.goal_position());
        }
        None
    }

    /// `getAttitude()`.
    pub fn get_attitude(&self) -> AttitudeType {
        self.attitude
    }

    /// `getAIStateType()`.
    pub fn get_ai_state_type<M: AiMachineQuery>(&self, m: &M) -> AiStateType {
        m.current_state_id()
    }

    /// `ignoreObstacle(obj)`.
    pub fn ignore_obstacle(&mut self, obj: ObjectId) {
        self.ignore_obstacle_id = obj;
    }

    /// `getIgnoredObstacleID()`.
    pub fn get_ignored_obstacle_id(&self) -> ObjectId {
        self.ignore_obstacle_id
    }

    /// `getEnterTarget()`: the goal object while entering, guarding a tunnel network or getting repaired.
    pub fn get_enter_target<H: AiHost, M: AiMachineQuery>(&self, h: &H, m: &M) -> ObjectId {
        let state_type = self.get_ai_state_type(m);
        if state_type != AI_ENTER && state_type != AI_GUARD_TUNNEL_NETWORK && (state_type != AI_GET_REPAIRED || plant(3222)) {
            return INVALID_ID;
        }
        let goal = m.goal_object_id();
        if h.object_exists(goal) { goal } else { INVALID_ID }
    }

    /// `getMoodMatrixValue()`: the controller, the mood (computer players only) and the unit type.
    /// (`m` is `getStateMachine()`, none before `onObjectCreated` makes it.)
    pub fn get_mood_matrix_value<H: AiHost, M: AiMachineQuery>(&self, h: &H, m: Option<&M>) -> u32 {
        let mut return_val = 0;
        if m.is_none() {
            return return_val;
        }
        let Some(player_type) = h.controlling_player_type() else { return return_val };
        if player_type == PlayerType::Human {
            return_val |= MM_CONTROLLER_PLAYER; // "Human units don't have a mood."
        } else {
            return_val |= MM_CONTROLLER_AI;
            return_val |= match self.get_attitude() {
                AI_SLEEP => MM_MOOD_SLEEP,
                AI_PASSIVE => MM_MOOD_PASSIVE,
                AI_NORMAL => MM_MOOD_NORMAL,
                AI_ALERT => MM_MOOD_ALERT,
                AI_AGGRESSIVE => MM_MOOD_AGGRESSIVE,
                _ => MM_MOOD_NORMAL, // DEBUG_CRASH(("Unknown mood ...")), "Using normal."
            };
        }

        if self.locomotor_set.get_valid_surfaces() & LOCOMOTORSURFACE_AIR != 0 {
            return_val |= MM_UNITTYPE_AIR;
        } else if self.has_turret_ai[0] && !plant(3223) {
            return_val |= MM_UNITTYPE_TURRETED;
        } else {
            return_val |= MM_UNITTYPE_NONTURRETED;
        }

        return_val
    }

    /// `getMoodMatrixActionAdjustment(action)`.
    pub fn get_mood_matrix_action_adjustment<H: AiHost, M: AiMachineQuery>(&self, h: &H, m: &M, action: MoodMatrixAction) -> u32 {
        // "Angry Mob Members (but not Nexi) are never subject to moods."
        if h.is_kind_of(KINDOF_INFANTRY) && h.is_kind_of(KINDOF_IGNORED_IN_GUI) {
            return MAA_ACTION_OK;
        }

        let mood_matrix = self.get_mood_matrix_value(h, Some(m));
        if mood_matrix & MM_CONTROLLER_PLAYER != 0 {
            // "Player-controlled units can always do actions (from a mood perspective, at any rate)"
            return MAA_ACTION_OK;
        }

        let mood = mood_matrix & MM_MOOD_BITMASK;
        let pick = |sleep: u32, passive: u32, normal: u32, alert: u32, aggressive: u32| -> u32 {
            match mood {
                MM_MOOD_SLEEP => sleep,
                MM_MOOD_PASSIVE => passive,
                MM_MOOD_NORMAL => normal,
                MM_MOOD_ALERT => alert,
                MM_MOOD_AGGRESSIVE => aggressive,
                _ => MAA_ACTION_OK,
            }
        };
        match action {
            MM_ACTION_IDLE => pick(
                MAA_ACTION_OK | MAA_AFFECT_RANGE_IGNORE_ALL,
                MAA_ACTION_OK | MAA_AFFECT_RANGE_WAIT_FOR_ATTACK,
                MAA_ACTION_OK,
                MAA_ACTION_OK | MAA_AFFECT_RANGE_ALERT,
                MAA_ACTION_OK | MAA_AFFECT_RANGE_AGGRESSIVE,
            ),
            MM_ACTION_MOVE => pick(
                MAA_ACTION_TO_IDLE | MAA_AFFECT_RANGE_IGNORE_ALL,
                MAA_ACTION_OK | MAA_AFFECT_RANGE_WAIT_FOR_ATTACK,
                MAA_ACTION_OK,
                MAA_ACTION_TO_ATTACK_MOVE | MAA_AFFECT_RANGE_ALERT,
                MAA_ACTION_TO_ATTACK_MOVE | if plant(3224) { MAA_AFFECT_RANGE_ALERT } else { MAA_AFFECT_RANGE_AGGRESSIVE },
            ),
            MM_ACTION_ATTACK => pick(MAA_ACTION_TO_IDLE | MAA_AFFECT_RANGE_IGNORE_ALL, MAA_ACTION_OK, MAA_ACTION_OK, MAA_ACTION_OK, MAA_ACTION_OK),
            MM_ACTION_ATTACK_MOVE => pick(
                MAA_ACTION_TO_IDLE | MAA_AFFECT_RANGE_IGNORE_ALL,
                MAA_ACTION_OK,
                MAA_ACTION_OK,
                MAA_ACTION_OK | MAA_AFFECT_RANGE_ALERT,
                MAA_ACTION_OK | MAA_AFFECT_RANGE_AGGRESSIVE,
            ),
            _ => MAA_ACTION_OK,
        }
    }

    /// `wakeUpAndAttemptToTarget()`: an idle AI checks its mood now, randomly offset.
    pub fn wake_up_and_attempt_to_target<H: AiHost, M: AiMachineQuery>(&mut self, h: &mut H, m: &M) {
        if !self.is_idle(m) {
            return;
        }
        self.next_mood_check_time = h.frame();
        self.randomly_offset_mood_check = true;
    }

    /// `resetNextMoodCheckTime()`: "Usually called by *Idle::onEnter".
    pub fn reset_next_mood_check_time<H: AiHost>(&mut self, h: &mut H) {
        let now = h.frame();
        self.next_mood_check_time = now.wrapping_add(h.ai_data_force_idle_frames());
        self.randomly_offset_mood_check = !plant(3225);
    }

    /// `setNextMoodCheckTime(frame)`.
    pub fn set_next_mood_check_time(&mut self, frame: u32) {
        self.next_mood_check_time = frame;
        self.randomly_offset_mood_check = false;
    }

    /// `canAutoAcquireWhileStealthed()`: a stealth granted by a special power, or the data's `STEALTHED` bit.
    pub fn can_auto_acquire_while_stealthed<H: AiHost>(&self, h: &H) -> bool {
        if h.stealth_granted_by_special_power() == Some(true) {
            return true;
        }
        self.data.auto_acquire_enemies_when_idle & AAS_IDLE_STEALTHED != 0
    }

    /// `getNextMoodTarget(calledByAI, calledDuringIdle)`: "the next object that our mood suggests we should attack".
    pub fn get_next_mood_target<H: AiHost, M: AiMachineQuery>(&mut self, h: &mut H, m: &M, called_by_ai: bool, called_during_idle: bool) -> ObjectId {
        // "if we're dead, we can't attack"
        if h.is_effectively_dead() {
            return INVALID_ID;
        }
        if h.status_test(OBJECT_STATUS_IS_USING_ABILITY) {
            return INVALID_ID; // "we are doing a special ability. Shouldn't auto-acquire a target at this time."
        }
        let auto = self.data.auto_acquire_enemies_when_idle;
        if called_during_idle && auto & AAS_IDLE == 0 {
            return INVALID_ID;
        }
        // "this should ignore calledDuringIdle, despite what the name of the bit implies."
        if self.is_attacking(m) && auto & AAS_IDLE_NOT_WHILE_ATTACKING != 0 {
            return INVALID_ID;
        }
        if called_during_idle && h.status_test(OBJECT_STATUS_STEALTHED) && !self.can_auto_acquire_while_stealthed(h) {
            let container = h.contained_by();
            if !(container != INVALID_ID && h.container_passenger_allowed_to_fire()) {
                return INVALID_ID; // "Sorry, stealthed and not allowed to idle fire when stealthed."
            }
        }

        let now = h.frame();

        // "Check if team auto targets same victim."
        if called_by_ai && h.team_attack_common_target() {
            let mut team_victim = h.team_target_object();
            if team_victim != INVALID_ID {
                let result = h.able_to_attack_specific_object(ATTACK_NEW_TARGET, team_victim, CMD_FROM_AI);
                if result != ATTACKRESULT_POSSIBLE && result != ATTACKRESULT_POSSIBLE_AFTER_MOVING {
                    team_victim = INVALID_ID; // "Can't attack him."
                }
            }
            if team_victim != INVALID_ID && self.get_attitude() >= AI_NORMAL {
                return team_victim;
            }
        }

        if called_by_ai {
            // "make sure it's time to check again."
            if now < self.next_mood_check_time {
                return INVALID_ID;
            }
            let check_rate = self.data.mood_attack_check_rate as i32;
            self.next_mood_check_time = now.wrapping_add(check_rate as u32);
            if self.randomly_offset_mood_check {
                let half_rate = check_rate >> 1;
                let offset = h.logic_random_value(-half_rate, if plant(3226) { half_rate + 1 } else { half_rate });
                self.next_mood_check_time = (self.next_mood_check_time as i32).wrapping_add(offset) as u32;
                self.randomly_offset_mood_check = false;
            }
        }

        // "Use Guard Outer, which typically corresponds to the total range"
        let mut range_to_find_within = h.adjusted_vision_range_for_object(AI_VISIONFACTOR_OWNERTYPE | AI_VISIONFACTOR_MOOD);
        if range_to_find_within <= 0.0 {
            return INVALID_ID;
        }

        // "If we are contained by an object, add it's bounding radius"
        if h.contained_by() != INVALID_ID {
            range_to_find_within += h.container_bounding_circle_radius();
        }

        let mood_matrix_val = self.get_mood_matrix_value(h, Some(m));
        if mood_matrix_val & MM_CONTROLLER_AI != 0 && mood_matrix_val & MM_MOOD_PASSIVE != 0 {
            // "Do not allow units that healed me to get acquired! They are our friends!!!"
            let (damage_type, source) = h.last_damage_type_and_source();
            if damage_type != DAMAGE_HEALING {
                return if h.object_exists(source) { source } else { INVALID_ID };
            }
        }
        let mut flags = AI_CAN_ATTACK;
        if h.ai_data_attack_uses_line_of_sight() && h.is_kind_of(KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT) {
            flags |= AI_CAN_SEE;
        }
        if h.ai_data_attack_ignore_insignificant_buildings() {
            flags |= AI_IGNORE_INSIGNIFICANT_BUILDINGS;
        }
        if auto & AAS_IDLE_ATTACK_BUILDINGS != 0 {
            flags |= AI_ATTACK_BUILDINGS;
        }
        // "if we're called by AI, and are human controlled, then our AI will not allow us to pursue the target." (the controlling
        // player is read unchecked here upstream)
        let human = h.controlling_player_type() == Some(PlayerType::Human);
        if called_by_ai && human {
            flags |= AI_WITHIN_ATTACK_RANGE;
        }
        if called_by_ai && human && !plant(3227) {
            flags |= AI_UNFOGGED;
        }

        h.find_closest_enemy(range_to_find_within, flags, self.attack_info, None)
    }

    /// `evaluateMoraleBonus()`: the horde, nationalism and fanaticism weapon bonuses.
    pub fn evaluate_morale_bonus<H: AiHost>(&mut self, h: &mut H) {
        let mut horde = false;
        let mut nationalism = h.player_has_upgrade_complete("Upgrade_Nationalism");
        let mut fanaticism = h.player_has_upgrade_complete("Upgrade_Fanaticism");

        // "are we in a horde"
        for (in_horde, allowed_nationalism) in h.horde_update_interfaces() {
            if in_horde {
                horde = true;
                if !allowed_nationalism && !plant(3228) {
                    nationalism = false;
                    fanaticism = false;
                }
            }
        }

        if horde {
            h.set_weapon_bonus_condition(condition::HORDE);
        } else {
            h.clear_weapon_bonus_condition(condition::HORDE);
        }

        if nationalism {
            h.set_weapon_bonus_condition(condition::NATIONALISM);
            if fanaticism {
                h.set_weapon_bonus_condition(condition::FANATICISM); // "FOR THE NEW GC INFANTRY GENERAL"
            } else {
                h.clear_weapon_bonus_condition(condition::FANATICISM);
            }
        } else {
            h.clear_weapon_bonus_condition(condition::NATIONALISM);
        }
    }

    /// `isIdle()`: the idle state, or a current state that is idle.
    pub fn is_idle<M: AiMachineQuery>(&self, m: &M) -> bool {
        if m.current_state_id() == AI_IDLE {
            return true;
        }
        m.is_in_idle_state()
    }

    /// `isAttacking()`.
    pub fn is_attacking<M: AiMachineQuery>(&self, m: &M) -> bool {
        m.is_in_attack_state()
    }

    /// `isBusy()`: "when explicitly in the busy state. Moving or attacking is not considered busy!"
    pub fn is_busy<M: AiMachineQuery>(&self, m: &M) -> bool {
        m.is_in_busy_state()
    }

    /// `isClearingMines()`: attacking with an anti-mine weapon.
    pub fn is_clearing_mines<H: AiHost>(&self, h: &H) -> bool {
        if !h.status_test(OBJECT_STATUS_IS_ATTACKING) {
            return false;
        }
        let Some(anti) = h.current_weapon_anti_mask() else { return false };
        anti & WEAPON_ANTI_MINE as i32 != 0
    }

    /// `isMovingAwayFrom(obj)`: moving out of its way (the temporary state).
    pub fn is_moving_away_from(&self, obj: ObjectId) -> bool {
        if self.machine.temporary_state == AI_MOVE_OUT_OF_THE_WAY {
            if self.move_out_of_way1 == obj {
                return true;
            }
            if self.move_out_of_way2 == obj && !plant(3229) {
                return true;
            }
        }
        false
    }

    /// `isMoving()`.
    pub fn is_moving<M: AiMachineQuery>(&self, m: &M) -> bool {
        if self.is_idle(m) {
            return false;
        }
        self.locomotor_goal_type != LocoGoalType::None || self.is_moving
    }
}
