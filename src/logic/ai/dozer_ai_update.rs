// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/DozerAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DozerAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `DozerAIUpdate` (`p1w13-sonnet`, layer 10, batch A38): the builder. Its tasks (build, repair, fortify: a target and the frame it
//! was ordered, with dock points) are run by a second machine (`DozerPrimaryStateMachine`, [`DozerPrimaryState`]) beside the AI's:
//! idle (bored: repair something near, else attack a mine), and an action state per task, each running its own third machine
//! (`DozerActionStateMachine`, [`DozerActionState`]: pick a position around the target, move there, do the action: build the
//! construction percent up, repair the health up).
//!
//! The members are [`DozerMembers`] (the Worker's too) in [`Dozer`], held in [`AiSubclass::Dozer`]; the primary machine is
//! `AiUpdateModule::dozer_machine`, lent to the host ([`AiStateHost::dozer_machine_stack`]) while the AI runs, since a command
//! (`newTask`, `cancelTask`, a player's command) resets it. While it runs its slot is empty; a reset asked for then (a task
//! cancelled by the action machine, a repair ordered by the idle state) is [`DozerMembers::machine_reset_pending`], done by the
//! running state as soon as the call that asked returns (upstream resets it re-entrantly at that point; nothing between the two
//! points reads or logs anything, and the action machine's own failure exit touches only itself).
//!
//! # Upstream facts kept
//!
//! - `update` runs the base's update FIRST, then (unless dead) cancels a repair it may no longer do, then the machine; never asleep.
//! - The machine is made by the constructor (the object has no AI yet: the idle state's transitions all answer false).
//! - The action machine's `xfer` saves its version and task only, not the machine; the going-home state saves nothing.
//! - `getMostRecentCommand` takes the latest order frame strictly after 0: a task ordered on frame 0 is never the most recent.
//! - `internalCancelTask` ignores an invalid task; `internalTaskComplete` does not check (its callers pass their own task).
//! - `m_isRebuild` is set only by `construct`; before it, false: the memory pool zeroes every object it makes
//!   (`MemoryPool::allocateBlockImplementation`). (Corrected in batch A39: batch A38 refused a build completed before any
//!   `construct` as undefined.)
//! - Refused as undefined (`StateMachineError::Undefined`): the idle state's player index without a player, a bridge tower without
//!   its interfaces during a repair, construction without the player's money.
//!
//! Batch A39: the machines and the members are the worker's too ([`dz`] reaches a `WorkerAIUpdate`'s, its `DozerAIInterface`);
//! where the worker's virtual members differ (see `worker_ai_update`), these branch on [`is_worker`].

use std::convert::Infallible;
use std::marker::PhantomData;

use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::game_common::{PlayerType, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER, KINDOF_DOZER};
use crate::common::model_state::{
    MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED, MODELCONDITION_ACTIVELY_CONSTRUCTING, MODELCONDITION_AWAITING_CONSTRUCTION,
    MODELCONDITION_PARTIALLY_CONSTRUCTED,
};
use crate::common::object_status_types::{OBJECT_STATUS_RECONSTRUCTING, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::state_machine::{
    SmResult, StateBehavior, StateConditionInfo, StateExitType, StateId, StateMachine, StateMachineError, StateReturn,
    EXIT_MACHINE_WITH_FAILURE, EXIT_MACHINE_WITH_SUCCESS, INVALID_STATE_ID,
};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferVersion};
use crate::logic::ai::ai_commands::{self as commands, AiCommandParms, AICMD_MOVE_AWAY_FROM_UNIT, AICMD_REPAIR, AICMD_RESUME_CONSTRUCTION};
use crate::logic::ai::ai_states::{ai, AiState, AiStateHost, DistanceKind};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::{CommandSourceType, CMD_FROM_AI, CMD_FROM_DOZER, CMD_FROM_PLAYER};
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::ai::supply_truck_ai_update::with_ai_machine;
pub use crate::logic::ai::worker_ai_update::is_worker;
use crate::logic::ai::worker_ai_update as worker;
use crate::logic::locomotor::LOCOMOTORSURFACE_AIR;
use crate::logic::modules::plant;
use crate::logic::weapon_types::WEAPONSET_MINE_CLEARING_DETAIL;
use crate::ww::vector3::Vector3;

type Sm<H> = StateMachine<AiState<H>>;

/// `DOZER_TASK_INVALID`.
pub const DOZER_TASK_INVALID: i32 = -1;
/// `DOZER_TASK_BUILD`: "go build something".
pub const DOZER_TASK_BUILD: i32 = 0;
/// `DOZER_TASK_REPAIR`: "go repair something".
pub const DOZER_TASK_REPAIR: i32 = 1;
/// `DOZER_TASK_FORTIFY`: "go fortify something".
pub const DOZER_TASK_FORTIFY: i32 = 2;
/// `DOZER_NUM_TASKS`.
pub const DOZER_NUM_TASKS: usize = 3;
/// `DOZER_DOCK_POINT_START`.
pub const DOZER_DOCK_POINT_START: i32 = 0;
/// `DOZER_DOCK_POINT_ACTION`.
pub const DOZER_DOCK_POINT_ACTION: i32 = 1;
/// `DOZER_DOCK_POINT_END`.
pub const DOZER_DOCK_POINT_END: i32 = 2;
/// `DOZER_NUM_DOCK_POINTS`.
pub const DOZER_NUM_DOCK_POINTS: usize = 3;
/// `DOZER_SELECT_BUILD_DOCK_LOCATION`.
pub const DOZER_SELECT_BUILD_DOCK_LOCATION: i32 = 0;
/// `DOZER_MOVING_TO_BUILD_DOCK_LOCATION`.
pub const DOZER_MOVING_TO_BUILD_DOCK_LOCATION: i32 = 1;
/// `DOZER_DO_BUILD_AT_DOCK`.
pub const DOZER_DO_BUILD_AT_DOCK: i32 = 2;

/// `DOZER_PRIMARY_IDLE`.
pub const DOZER_PRIMARY_IDLE: StateId = 0;
/// `DOZER_PRIMARY_BUILD`.
pub const DOZER_PRIMARY_BUILD: StateId = 1;
/// `DOZER_PRIMARY_REPAIR`.
pub const DOZER_PRIMARY_REPAIR: StateId = 2;
/// `DOZER_PRIMARY_FORTIFY`.
pub const DOZER_PRIMARY_FORTIFY: StateId = 3;
/// `DOZER_PRIMARY_GO_HOME`.
pub const DOZER_PRIMARY_GO_HOME: StateId = 4;
/// `DOZER_ACTION_PICK_ACTION_POS`.
pub const DOZER_ACTION_PICK_ACTION_POS: StateId = 0;
/// `DOZER_ACTION_MOVE_TO_ACTION_POS`.
pub const DOZER_ACTION_MOVE_TO_ACTION_POS: StateId = 1;
/// `DOZER_ACTION_DO_ACTION`.
pub const DOZER_ACTION_DO_ACTION: StateId = 2;

/// `MIN_ACTION_TOLERANCE`.
const MIN_ACTION_TOLERANCE: f32 = 70.0;
/// `CONSTRUCTION_COMPLETE` (`BuildAssistant.h`).
pub const CONSTRUCTION_COMPLETE: f32 = -1.0;
/// `BRIDGE_MAX_TOWERS` (`BridgeBehavior.h`).
pub const BRIDGE_MAX_TOWERS: usize = 4;
/// `BuildAssistant::CLEAR_PATH`.
pub const CLEAR_PATH: u32 = 0x02;
/// `BuildAssistant::NO_OBJECT_OVERLAP`.
pub const NO_OBJECT_OVERLAP: u32 = 0x04;
/// A player's construction's checks: `TERRAIN_RESTRICTIONS | CLEAR_PATH | NO_OBJECT_OVERLAP | SHROUD_REVEALED`.
pub const BUILD_CHECKS: u32 = 0x01 | CLEAR_PATH | NO_OBJECT_OVERLAP | 0x10;

/// `DozerAIUpdateModuleData`'s own fields.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DozerData {
    /// `RepairHealthPercentPerSecond`.
    pub repair_health_percent_per_second: f32,
    /// `BoredTime` (frames).
    pub bored_time: f32,
    /// `BoredRange`.
    pub bored_range: f32,
}

/// `DozerTaskInfo`: a task's target and the frame it was ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DozerTaskInfo {
    /// `m_targetObjectID`.
    pub target_object_id: ObjectId,
    /// `m_taskOrderFrame`.
    pub task_order_frame: u32,
}

/// The dozer's task members (`DozerAIUpdate`'s, and `WorkerAIUpdate`'s alike).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DozerMembers {
    /// `m_task[]`.
    pub task: [DozerTaskInfo; DOZER_NUM_TASKS],
    /// `m_currentTask`.
    pub current_task: i32,
    /// `m_dockPoint[][]`: valid, location.
    pub dock_point: [[(bool, Coord3D); DOZER_NUM_DOCK_POINTS]; DOZER_NUM_TASKS],
    /// `m_buildSubTask`.
    pub build_sub_task: i32,
    /// `m_isRebuild` (false until `construct`: the pool's block is zeroed).
    pub is_rebuild: bool,
    /// `m_buildingSound`'s playing handle.
    pub building_sound_handle: u32,
    /// A reset of the primary machine asked for while it runs (see the module docs).
    pub machine_reset_pending: bool,
}

impl Default for DozerMembers {
    /// `DozerAIUpdate(thing, moduleData)`'s members.
    fn default() -> Self {
        DozerMembers {
            task: [DozerTaskInfo::default(); DOZER_NUM_TASKS],
            current_task: DOZER_TASK_INVALID,
            dock_point: [[(false, Coord3D::default()); DOZER_NUM_DOCK_POINTS]; DOZER_NUM_TASKS],
            build_sub_task: DOZER_SELECT_BUILD_DOCK_LOCATION,
            is_rebuild: false,
            building_sound_handle: 0,
            machine_reset_pending: false,
        }
    }
}

/// `DozerAIUpdate`'s members (its machine is the module's), with its module data.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dozer {
    /// The module data's own fields.
    pub data: DozerData,
    /// The task members.
    pub m: DozerMembers,
}

impl Dozer {
    /// `DozerAIUpdate(thing, moduleData)`'s members.
    pub fn new(data: DozerData) -> Self {
        Dozer { data, m: DozerMembers::default() }
    }
}

/// `DozerPrimaryStateMachine`'s states, with their members.
pub enum DozerPrimaryState<H: AiStateHost> {
    /// `DozerPrimaryIdleState`.
    Idle { idle_too_long_timestamp: u32, idle_player_number: i32, is_marked_as_idle: bool },
    /// `DozerActionState`: its task and its action machine.
    Action { task: i32, machine: StateMachine<DozerActionState<H>> },
    /// `DozerPrimaryGoingHomeState`.
    GoHome,
    /// (names the host's type; never made)
    _Host(Infallible, PhantomData<fn() -> H>),
}

/// `DozerPrimaryStateMachine`'s transition conditions (the idle state's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DozerCondition {
    /// `isBuildMostImportant`.
    BuildMostImportant,
    /// `isRepairMostImportant`.
    RepairMostImportant,
    /// `isFortifyMostImportant`.
    FortifyMostImportant,
}

/// `DozerActionStateMachine`'s states.
pub enum DozerActionState<H: AiStateHost> {
    /// `DozerActionPickActionPosState`: `m_task`, `m_failedAttempts`.
    PickActionPos { task: i32, failed_attempts: i32 },
    /// `DozerActionMoveToActionPosState`.
    MoveToActionPos { task: i32 },
    /// `DozerActionDoActionState`: `m_task`, `m_enterFrame`.
    DoAction { task: i32, enter_frame: u32 },
    /// (names the host's type; never made)
    _Host(Infallible, PhantomData<fn() -> H>),
}

/// The action machine has no conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoCondition {}

/// The running AI's dozer members (a `DozerAIUpdate`'s; batch A39: or a `WorkerAIUpdate`'s).
pub fn dz<H: AiStateHost>(h: &mut H) -> &mut DozerMembers {
    match &mut ai(h).subclass {
        AiSubclass::Dozer(d) => &mut d.m,
        AiSubclass::Worker(w) => &mut w.dozer.m,
        _ => unreachable!("a DozerAIUpdate or a WorkerAIUpdate"),
    }
}

/// The running AI's dozer data.
fn dz_data<H: AiStateHost>(h: &mut H) -> DozerData {
    match &ai(h).subclass {
        AiSubclass::Dozer(d) => d.data,
        AiSubclass::Worker(w) => w.dozer.data,
        _ => unreachable!("a DozerAIUpdate or a WorkerAIUpdate"),
    }
}

/// `sqr(x)`.
fn sqr(x: f32) -> f32 {
    x * x
}

/// `frame()` of the game logic.
fn frame<H: AiStateHost>(h: &H) -> u32 {
    crate::logic::locomotor::LocomotorHost::frame(h)
}

/// `ai->isIdle()` (the AI's machine lent).
fn ai_is_idle<H: AiStateHost>(h: &mut H) -> bool {
    with_ai_machine(h, |sm, h| ai(h).is_idle(&*sm))
}

/// Runs `f` with the AI's machine `sm` lent to the host (the dozer's code reaches it there).
pub fn lend_ai_machine<H: AiStateHost, R>(sm: &mut Sm<H>, h: &mut H, f: impl FnOnce(&mut H) -> R) -> R {
    let m = std::mem::take(sm);
    h.ai_machine_stack().push(m);
    let r = f(h);
    *sm = h.ai_machine_stack().pop().expect("the lent machine");
    r
}

/// `DozerActionStateMachine(owner, task)`: its three states (picking first: the default state), entered.
pub fn make_action_machine<H: AiStateHost>(h: &mut H, task: i32) -> SmResult<StateMachine<DozerActionState<H>>> {
    let mut m = StateMachine::new();
    m.define_state(
        DOZER_ACTION_PICK_ACTION_POS,
        DozerActionState::PickActionPos { task, failed_attempts: 0 },
        DOZER_ACTION_MOVE_TO_ACTION_POS,
        EXIT_MACHINE_WITH_FAILURE,
        &[],
    )?;
    m.define_state(DOZER_ACTION_MOVE_TO_ACTION_POS, DozerActionState::MoveToActionPos { task }, DOZER_ACTION_DO_ACTION, DOZER_ACTION_PICK_ACTION_POS, &[])?;
    m.define_state(DOZER_ACTION_DO_ACTION, DozerActionState::DoAction { task, enter_frame: 0 }, EXIT_MACHINE_WITH_SUCCESS, EXIT_MACHINE_WITH_FAILURE, &[])?;
    m.init_default_state(h)?;
    Ok(m)
}

/// `DozerPrimaryStateMachine(owner)`: idle (first: the default state, with its three conditions), the three action states (each
/// with its action machine made and entered), going home.
pub fn make_dozer_machine<H: AiStateHost>(h: &mut H) -> SmResult<StateMachine<DozerPrimaryState<H>>> {
    use DozerCondition::*;
    let c = |test, to_state_id| StateConditionInfo { test, to_state_id };
    let mut m = StateMachine::new();
    m.define_state(
        DOZER_PRIMARY_IDLE,
        DozerPrimaryState::Idle { idle_too_long_timestamp: 0, idle_player_number: 0, is_marked_as_idle: false },
        INVALID_STATE_ID,
        INVALID_STATE_ID,
        &[c(BuildMostImportant, DOZER_PRIMARY_BUILD), c(RepairMostImportant, DOZER_PRIMARY_REPAIR), c(FortifyMostImportant, DOZER_PRIMARY_FORTIFY)],
    )?;
    for (id, task) in [(DOZER_PRIMARY_BUILD, DOZER_TASK_BUILD), (DOZER_PRIMARY_REPAIR, DOZER_TASK_REPAIR), (DOZER_PRIMARY_FORTIFY, DOZER_TASK_FORTIFY)] {
        let machine = make_action_machine(h, task)?;
        m.define_state(id, DozerPrimaryState::Action { task, machine }, DOZER_PRIMARY_IDLE, DOZER_PRIMARY_IDLE, &[])?;
    }
    m.define_state(DOZER_PRIMARY_GO_HOME, DozerPrimaryState::GoHome, DOZER_PRIMARY_IDLE, DOZER_PRIMARY_IDLE, &[])?;
    Ok(m)
}

/// `m_dozerMachine->resetToDefaultState()`: now when the machine is lent, else when the running state resumes.
pub fn reset_dozer_machine<H: AiStateHost>(h: &mut H) -> SmResult<()> {
    let slot = h.dozer_machine_stack().last_mut().expect("the dozer machine's slot").take();
    match slot {
        Some(mut m) => {
            let r = m.reset_to_default_state(h);
            *h.dozer_machine_stack().last_mut().expect("the dozer machine's slot") = Some(m);
            r.map(|_| ())
        }
        None => {
            dz(h).machine_reset_pending = true;
            Ok(())
        }
    }
}

/// The reset asked for while the machine runs, done on the running machine.
fn consume_reset<H: AiStateHost>(sm: &mut StateMachine<DozerPrimaryState<H>>, h: &mut H) -> SmResult<()> {
    if std::mem::take(&mut dz(h).machine_reset_pending) {
        sm.reset_to_default_state(h)?;
    }
    Ok(())
}

/// `isTaskPending(task)`.
pub fn is_task_pending<H: AiStateHost>(h: &mut H, task: i32) -> bool {
    dz(h).task[task as usize].target_object_id != 0
}

/// `isAnyTaskPending()`.
pub fn is_any_task_pending<H: AiStateHost>(h: &mut H) -> bool {
    (0..DOZER_NUM_TASKS as i32).any(|t| is_task_pending(h, t))
}

/// `getTaskTarget(task)`.
pub fn get_task_target<H: AiStateHost>(h: &mut H, task: i32) -> ObjectId {
    dz(h).task[task as usize].target_object_id
}

/// `getMostRecentCommand()`: the pending task ordered latest (strictly after frame 0).
pub fn get_most_recent_command<H: AiStateHost>(h: &mut H) -> i32 {
    let mut most_recent_task = DOZER_TASK_INVALID;
    let mut most_recent_frame = 0u32;
    for i in 0..DOZER_NUM_TASKS {
        if is_task_pending(h, i as i32) && dz(h).task[i].task_order_frame > most_recent_frame && !plant(3700) {
            most_recent_task = i as i32;
            most_recent_frame = dz(h).task[i].task_order_frame;
        }
    }
    most_recent_task
}

/// `getDockPoint(task, point)`: the point, if set (none out of range).
pub fn get_dock_point<H: AiStateHost>(h: &mut H, task: i32, point: i32) -> Option<Coord3D> {
    if !(0..DOZER_NUM_TASKS as i32).contains(&task) || !(0..DOZER_NUM_DOCK_POINTS as i32).contains(&point) {
        return None;
    }
    let (valid, location) = dz(h).dock_point[task as usize][point as usize];
    valid.then_some(location)
}

/// `getBoredRange()`: a computer player's scaled by the AI data's modifier (batch A39: the worker's never).
pub fn get_bored_range<H: AiStateHost>(h: &mut H) -> f32 {
    let range = dz_data(h).bored_range;
    if !is_worker(h) && h.controlling_player_type() == Some(PlayerType::Computer) {
        return h.ai_data_dozer_bored_radius_modifier() * range;
    }
    range
}

/// `internalTaskCompleteOrCancelled(task)`: building or repairing, no longer actively constructing (batch A39: the worker's, a
/// repaired bridge tower's scaffolding then removed; the dozer's has that commented out).
fn internal_task_complete_or_cancelled<H: AiStateHost>(h: &mut H, task: i32) {
    if (task == DOZER_TASK_BUILD || task == DOZER_TASK_REPAIR) && !plant(3701) {
        h.clear_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
    }
    if task == DOZER_TASK_REPAIR && is_worker(h) {
        let obj = dz(h).task[task as usize].target_object_id;
        // "when we're done repairing bridges, tell the scaffolding to go away"
        if h.object_exists(obj) && h.object_is_kind_of(obj, KINDOF_BRIDGE_TOWER) && !plant(3736) {
            bridge_scaffolding(h, obj, false);
        }
    }
}

/// The task's order and dock points removed.
fn clear_task<H: AiStateHost>(h: &mut H, task: i32) {
    let m = dz(h);
    m.task[task as usize] = DozerTaskInfo::default();
    for p in 0..DOZER_NUM_DOCK_POINTS {
        m.dock_point[task as usize][p].0 = false;
    }
}

/// `internalTaskComplete(task)`.
pub fn internal_task_complete<H: AiStateHost>(h: &mut H, task: i32) {
    internal_task_complete_or_cancelled(h, task);
    clear_task(h, task);
}

/// `internalCancelTask(task)`: an invalid task ignored; else removed, and the AI told to idle (batch A39: the worker's, to move to
/// where it is).
pub fn internal_cancel_task<H: AiStateHost>(h: &mut H, task: i32) -> SmResult<()> {
    if !(0..DOZER_NUM_TASKS as i32).contains(&task) {
        return Ok(());
    }
    internal_task_complete_or_cancelled(h, task);
    clear_task(h, task);
    if is_worker(h) {
        // "@todo we really need a stop command instead of making it move to it's current location"
        let pos = h.position();
        return with_ai_machine(h, |sm, h| commands::ai_move_to_position(sm, h, &pos, CMD_FROM_AI));
    }
    // "stop the dozer from moving"
    with_ai_machine(h, |sm, h| commands::ai_idle(sm, h, CMD_FROM_AI))
}

/// `cancelTask(task)`: removed, the machine reset.
pub fn cancel_task<H: AiStateHost>(h: &mut H, task: i32) -> SmResult<()> {
    internal_cancel_task(h, task)?;
    reset_dozer_machine(h)
}

/// `isUsingAirborneLocomotor()` (Object.cpp's): the current locomotor's surfaces include the air.
fn is_using_airborne_locomotor<H: AiStateHost>(h: &mut H) -> bool {
    ai(h).get_cur_locomotor().is_some_and(|l| l.get_legal_surfaces() & LOCOMOTORSURFACE_AIR != 0)
}

/// `findGoodBuildOrRepairPosition(me, target, out)`: around the target's point on our side (half its major radius), within 100
/// (and 10 in z unless airborne; airborne, over the target): found, and the spot (else that point).
pub fn find_good_build_or_repair_position<H: AiStateHost>(h: &mut H, target: ObjectId) -> (bool, Coord3D) {
    let our_position = h.position();
    let their_position = h.object_position(target);
    let mut working_position = their_position;
    let mut offset = Vector3::new(our_position.x - their_position.x, our_position.y - their_position.y, our_position.z - their_position.z);
    offset.normalize();
    // "This scaler makes FindPositionAround bias towards our side"
    offset *= h.object_major_radius(target) / 2.0;
    working_position.x += offset.x;
    working_position.y += offset.y;
    working_position.z += offset.z;
    const MAX_Z_DELTA: f32 = 10.0;
    let airborne = is_using_airborne_locomotor(h);
    let max_z_delta = if airborne { 1e10 } else { MAX_Z_DELTA };
    let ignore = if airborne { target } else { INVALID_ID };
    match h.find_position_around_options(&working_position, 0.0, 100.0, true, None, max_z_delta, ignore) {
        Some(best) if !plant(3702) => (true, best),
        _ => (false, working_position),
    }
}

/// `findGoodBuildOrRepairPositionAndTarget(me, target, out)`: a bridge is repaired at its closest reachable tower (none: none);
/// else the target itself.
pub fn find_good_build_or_repair_position_and_target<H: AiStateHost>(h: &mut H, target: ObjectId) -> Option<(ObjectId, Coord3D)> {
    if h.object_is_kind_of(target, KINDOF_BRIDGE) {
        if let Some(towers) = h.bridge_tower_ids(target) {
            // "have to repair at a tower."
            let mut best_dist_sqr = 1e10f32;
            let mut best: Option<(ObjectId, Coord3D)> = None;
            for tower in towers {
                if h.object_exists(tower) {
                    let (found, tmp) = find_good_build_or_repair_position(h, tower);
                    if found && h.is_path_available(&tmp) {
                        let me = h.position();
                        let this_dist_sqr = sqr(me.x - tmp.x) + sqr(me.y - tmp.y);
                        if this_dist_sqr < best_dist_sqr {
                            best_dist_sqr = this_dist_sqr;
                            best = Some((tower, tmp));
                        }
                    }
                }
            }
            // (none: a DEBUG_CRASH, "should not happen, no reachable tower found")
            return best;
        }
    }
    let (_, pos) = find_good_build_or_repair_position(h, target);
    Some((target, pos))
}

/// `newTask(task, target)`: a build or repair replaces its pending one, goes to the target's good position (a bridge's tower), the
/// builder set for a build, its dock points the position and one 5 cells further from the target; the order and the frame noted;
/// the machine reset. (Batch A39: the worker's forgets its preferred dock first, its dock points all the position, and it leaves
/// the supply truck role last.)
pub fn new_task<H: AiStateHost>(h: &mut H, task: i32, target: ObjectId) -> SmResult<()> {
    if target == INVALID_ID {
        return Ok(());
    }
    let is_worker = is_worker(h);
    if is_worker {
        worker::forget_preferred_dock(h);
    }
    let mut target = target;
    if task == DOZER_TASK_BUILD || task == DOZER_TASK_REPAIR {
        // "handle getting two tasks"
        if is_task_pending(h, task) {
            cancel_task(h, task)?;
        }
        let Some((t, mut position)) = find_good_build_or_repair_position_and_target(h, target) else {
            // "could happen for some bridges"
            return Ok(());
        };
        target = t;
        if task == DOZER_TASK_BUILD {
            let me = h.object_id();
            h.object_set_builder(target, me);
        }
        if is_worker {
            let m = dz(h);
            for p in [DOZER_DOCK_POINT_START, DOZER_DOCK_POINT_ACTION, DOZER_DOCK_POINT_END] {
                m.dock_point[task as usize][p as usize] = (true, position);
            }
        } else {
            let tp = h.object_position(target);
            let m = dz(h);
            m.dock_point[task as usize][DOZER_DOCK_POINT_START as usize] = (true, position);
            m.dock_point[task as usize][DOZER_DOCK_POINT_ACTION as usize] = (true, position);
            let mut offset = Coord3D { x: position.x - tp.x, y: position.y - tp.y, z: 0.0 };
            offset.normalize();
            offset.scale(5.0 * PATHFIND_CELL_SIZE_F);
            // "move away from the dock point at the end of build."
            position.add(&offset);
            dz(h).dock_point[task as usize][DOZER_DOCK_POINT_END as usize] = (true, position);
        }
    }
    let f = frame(h);
    dz(h).task[task as usize] = DozerTaskInfo { target_object_id: target, task_order_frame: f };
    // "reset the dozer behavior so that it can re-evluate which task to continue working on"
    reset_dozer_machine(h)?;
    if is_worker {
        // "reset the workermachine, if we've been acting like a supply truck"
        worker::exiting_supply_truck_state(h)?;
    }
    Ok(())
}

/// `canAcceptNewRepair(obj)`: not the target being repaired, nor a tower of its bridge.
pub fn can_accept_new_repair<H: AiStateHost>(h: &mut H, obj: ObjectId) -> bool {
    if !h.object_exists(obj) {
        return false;
    }
    if dz(h).current_task != DOZER_TASK_REPAIR {
        return true;
    }
    let current = dz(h).task[DOZER_TASK_REPAIR as usize].target_object_id;
    if h.object_exists(current) {
        if current == obj && !plant(3703) {
            return false;
        }
        if h.object_is_kind_of(current, KINDOF_BRIDGE_TOWER) && h.object_is_kind_of(obj, KINDOF_BRIDGE_TOWER) {
            let (Some(a), Some(b)) = (h.bridge_tower_bridge_id(current), h.bridge_tower_bridge_id(obj)) else {
                // "Unable to find bridge tower interface on object" (a DEBUG_CRASH)
                return false;
            };
            if a == b {
                return false;
            }
        }
    }
    true
}

/// `privateRepair(obj, cmdSource)`: unless already repairing it, unable to, or another dozer heals it, a repair task (batch A39:
/// the worker's asks whether it is able first).
pub fn private_repair<H: AiStateHost>(h: &mut H, obj: ObjectId, src: CommandSourceType) -> SmResult<()> {
    if is_worker(h) {
        let target = if h.object_exists(obj) { obj } else { INVALID_ID };
        // "sanity, if we can't repair the object then get out of there"
        if !h.action_can_repair_object(target, src) {
            return Ok(());
        }
    }
    if !can_accept_new_repair(h, obj) {
        return Ok(());
    }
    if !is_worker(h) && !h.action_can_repair_object(obj, src) {
        return Ok(());
    }
    let current_repairer = h.object_sole_healing_benefactor(obj);
    let me = h.object_id();
    if current_repairer != INVALID_ID && current_repairer != me && !plant(3704) {
        return Ok(());
    }
    new_task(h, DOZER_TASK_REPAIR, obj)
}

/// `privateResumeConstruction(obj, cmdSource)`: a build task, if it may.
pub fn private_resume_construction<H: AiStateHost>(h: &mut H, obj: ObjectId, src: CommandSourceType) -> SmResult<()> {
    if !h.object_exists(obj) {
        return Ok(());
    }
    if !h.action_can_resume_construction_of(obj, src) {
        return Ok(());
    }
    new_task(h, DOZER_TASK_BUILD, obj)
}

/// `aiDoCommand(parms)` (`DozerAIUpdate`'s): no longer actively constructing; if allowed, a move away (not while on a task, unless
/// from a dozer), a repair or resumed construction (idle first when on no task), else the base's (the player's command cancelling
/// the current task first, resetting the machine after). (Batch A39: the worker's has no move-away case, and drops its boxes
/// after a command that leaves it clearing mines.)
pub fn dozer_ai_do_command<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, parms: &AiCommandParms) -> SmResult<()> {
    h.clear_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
    if !commands::is_allowed_to_respond_to_ai_commands(sm, h, parms) {
        return Ok(());
    }
    if is_worker(h) {
        do_command(sm, h, parms)?;
        worker::drop_boxes_if_clearing_mines(h);
        return Ok(());
    }
    if parms.cmd == AICMD_MOVE_AWAY_FROM_UNIT {
        let other_is_dozer = h.object_exists(parms.obj) && h.object_is_kind_of(parms.obj, KINDOF_DOZER);
        // "We only want to do this if we aren't busy doing dozer things. jba."
        if !other_is_dozer && dz(h).current_task != DOZER_TASK_INVALID && !plant(3705) {
            return Ok(());
        }
        return commands::base_ai_do_command(sm, h, parms);
    }
    do_command(sm, h, parms)
}

/// `aiDoCommand`'s switch but the dozer's move away: a repair or resumed construction, else the base's.
fn do_command<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, parms: &AiCommandParms) -> SmResult<()> {
    match parms.cmd {
        AICMD_REPAIR | AICMD_RESUME_CONSTRUCTION => {
            // "if we have no task right now, go idle so we can immediately respond to this"
            if dz(h).current_task == DOZER_TASK_INVALID {
                commands::ai_idle(sm, h, CMD_FROM_AI)?;
            }
            lend_ai_machine(sm, h, |h| {
                if parms.cmd == AICMD_REPAIR {
                    private_repair(h, parms.obj, parms.cmd_source)
                } else {
                    private_resume_construction(h, parms.obj, parms.cmd_source)
                }
            })
        }
        _ => {
            let current = dz(h).current_task;
            if parms.cmd_source == CMD_FROM_PLAYER && current != DOZER_TASK_INVALID {
                lend_ai_machine(sm, h, |h| cancel_task(h, current))?;
            }
            commands::base_ai_do_command(sm, h, parms)?;
            // "when a player issues commands, this will cause the dozer to re-evaluate what it's doing"
            if parms.cmd_source == CMD_FROM_PLAYER && !plant(3706) {
                lend_ai_machine(sm, h, reset_dozer_machine)?;
            }
            Ok(())
        }
    }
}

/// `update()`'s own part, after the base's (the AI and its machine lent, the dozer machine's slot lent): dead, nothing; a current
/// repair it may no longer do cancelled; on no task, mine clearing allowed.
pub fn dozer_validate_task<H: AiStateHost>(h: &mut H) -> SmResult<bool> {
    if h.is_effectively_dead() {
        return Ok(false);
    }
    let current_task = dz(h).current_task;
    if current_task != DOZER_TASK_INVALID {
        let target = get_task_target(h, current_task);
        let target = if h.object_exists(target) { target } else { INVALID_ID };
        let src = ai(h).last_command_source;
        if current_task == DOZER_TASK_REPAIR && !h.action_can_repair_object(target, src) && !plant(3707) {
            cancel_task(h, current_task)?;
        }
    } else {
        h.set_weapon_set_flag(WEAPONSET_MINE_CLEARING_DETAIL, true);
    }
    Ok(true)
}

/// `startBuildingSound(sound, site)`: the site's sound, its handle kept.
fn start_building_sound<H: AiStateHost>(h: &mut H, site: ObjectId) {
    let handle = h.play_under_construction_sound(site);
    dz(h).building_sound_handle = handle;
}

/// `finishBuildingSound()`.
fn finish_building_sound<H: AiStateHost>(h: &mut H) {
    let handle = dz(h).building_sound_handle;
    h.remove_audio_event(handle);
}

/// `createBridgeScaffolding(tower)` (and `removeBridgeScaffolding`): the tower's bridge told, if both are there.
pub fn bridge_scaffolding<H: AiStateHost>(h: &mut H, tower: ObjectId, create: bool) {
    if !h.object_exists(tower) {
        return;
    }
    let Some(bridge) = h.bridge_tower_bridge_id(tower) else { return };
    if !h.object_exists(bridge) || h.bridge_tower_ids(bridge).is_none() {
        return;
    }
    h.bridge_scaffolding(bridge, create);
}

/// `findObjectToRepair(dozer)`: the closest (2-D) structure of ours in the bored range we may repair.
fn find_object_to_repair<H: AiStateHost>(h: &mut H) -> ObjectId {
    let range = get_bored_range(h);
    let pos = h.position();
    let mut closest = INVALID_ID;
    let mut closest_dist_sqr = 0.0f32;
    for obj in h.partition_repair_candidates(&pos, range) {
        if !h.action_can_repair_object(obj, CMD_FROM_AI) {
            continue;
        }
        let dist_sqr = h.distance_squared_to_object(obj, DistanceKind::Center2d);
        if closest == INVALID_ID || dist_sqr < closest_dist_sqr || (plant(3708) && dist_sqr == closest_dist_sqr) {
            closest = obj;
            closest_dist_sqr = dist_sqr;
        }
    }
    closest
}

/// The idle state's update: marked idle in the UI while idle and alive (unmarked else); the bored timer reset while busy; bored with
/// nothing pending, something to repair near, else a mine to clear.
fn idle_update<H: AiStateHost>(sm: &mut StateMachine<DozerPrimaryState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let (mut ts, mut player_number, mut marked) = match sm.state(id) {
        Some(DozerPrimaryState::Idle { idle_too_long_timestamp, idle_player_number, is_marked_as_idle }) => {
            (*idle_too_long_timestamp, *idle_player_number, *is_marked_as_idle)
        }
        _ => unreachable!("the idle state"),
    };
    let idle = ai_is_idle(h);
    let dead = h.is_effectively_dead();
    if idle && !marked && !dead {
        let Some(index) = h.player_index() else {
            return Err(StateMachineError::Undefined("a dozer's idle state's player index without a player"));
        };
        player_number = index;
        h.in_game_ui_idle_worker(true, player_number);
        marked = true;
        h.set_weapon_set_flag(WEAPONSET_MINE_CLEARING_DETAIL, true);
    }
    let idle = ai_is_idle(h);
    if marked && (!idle || h.is_effectively_dead()) && !plant(3709) {
        h.in_game_ui_idle_worker(false, player_number);
        player_number = -1;
        marked = false;
    }
    if !ai_is_idle(h) {
        ts = frame(h);
    }
    let save = |sm: &mut StateMachine<DozerPrimaryState<H>>, ts: u32, player_number: i32, marked: bool| {
        if let Some(DozerPrimaryState::Idle { idle_too_long_timestamp, idle_player_number, is_marked_as_idle }) = sm.state_mut(id) {
            *idle_too_long_timestamp = ts;
            *idle_player_number = player_number;
            *is_marked_as_idle = marked;
        }
    };
    save(sm, ts, player_number, marked);
    let bored_time = dz_data(h).bored_time;
    if (frame(h).wrapping_sub(ts) as f32) > bored_time && !is_any_task_pending(h) && !plant(3710) {
        // "we'll only try it every once in a while"
        save(sm, frame(h), player_number, marked);
        let repair_target = find_object_to_repair(h);
        if repair_target != INVALID_ID {
            with_ai_machine(h, |sm, h| {
                let mut parms = AiCommandParms::new(AICMD_REPAIR, CMD_FROM_AI);
                parms.obj = repair_target;
                commands::ai_do_command(sm, h, &parms)
            })?;
            consume_reset(sm, h)?;
        } else {
            h.set_weapon_set_flag(WEAPONSET_MINE_CLEARING_DETAIL, true);
            let range = get_bored_range(h);
            let mine = h.partition_closest_mine(range);
            if mine != INVALID_ID {
                with_ai_machine(h, |sm, h| commands::ai_attack_object(sm, h, mine, 1, CMD_FROM_DOZER))?;
                consume_reset(sm, h)?;
            }
        }
    }
    Ok(StateReturn::CONTINUE)
}

impl<H: AiStateHost> StateBehavior for DozerPrimaryState<H> {
    type Ctx = H;
    type Condition = DozerCondition;

    fn on_enter(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(DozerPrimaryState::Idle { .. }) => {
                let f = frame(h);
                if let Some(DozerPrimaryState::Idle { idle_too_long_timestamp, is_marked_as_idle, .. }) = sm.state_mut(id) {
                    *idle_too_long_timestamp = f;
                    *is_marked_as_idle = false;
                }
                h.set_weapon_set_flag(WEAPONSET_MINE_CLEARING_DETAIL, true);
            }
            Some(DozerPrimaryState::Action { task, .. }) => {
                let task = *task;
                // (the object's AI: none yet while the AI's constructor enters the default state)
                if h.ai_stack().is_empty() {
                    return Ok(StateReturn::FAILURE);
                }
                dz(h).current_task = task;
                // "anytime we transition into doing an action, we need to reset our state machine to do that action"
                let Some(DozerPrimaryState::Action { machine, .. }) = sm.state_mut(id) else { unreachable!() };
                let mut m = std::mem::take(machine);
                let r = m.reset_to_default_state(h);
                if let Some(DozerPrimaryState::Action { machine, .. }) = sm.state_mut(id) {
                    *machine = m;
                }
                r?;
            }
            Some(DozerPrimaryState::_Host(never, _)) => match *never {},
            _ => {}
        }
        Ok(StateReturn::CONTINUE)
    }

    fn on_exit(sm: &mut StateMachine<Self>, h: &mut H, id: StateId, _status: StateExitType) -> SmResult<()> {
        match sm.state(id) {
            Some(DozerPrimaryState::Idle { is_marked_as_idle: true, idle_player_number, .. }) => {
                let n = *idle_player_number;
                h.in_game_ui_idle_worker(false, n);
                if let Some(DozerPrimaryState::Idle { idle_player_number, is_marked_as_idle, .. }) = sm.state_mut(id) {
                    *idle_player_number = -1;
                    *is_marked_as_idle = false;
                }
            }
            Some(DozerPrimaryState::Action { .. }) if !h.ai_stack().is_empty() => {
                dz(h).current_task = DOZER_TASK_INVALID;
            }
            _ => {}
        }
        Ok(())
    }

    fn update(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(DozerPrimaryState::Idle { .. }) => {
                if h.ai_stack().is_empty() {
                    return Ok(StateReturn::FAILURE);
                }
                idle_update(sm, h, id)
            }
            Some(DozerPrimaryState::Action { .. }) => {
                // "we can sleep when this machine sleeps"
                let Some(DozerPrimaryState::Action { machine, .. }) = sm.state_mut(id) else { unreachable!() };
                let mut m = std::mem::take(machine);
                let r = m.update_state_machine(h);
                if let Some(DozerPrimaryState::Action { machine, .. }) = sm.state_mut(id) {
                    *machine = m;
                }
                let r = r?;
                consume_reset(sm, h)?;
                Ok(r)
            }
            Some(DozerPrimaryState::GoHome) => Ok(StateReturn::FAILURE),
            _ => Ok(StateReturn::CONTINUE),
        }
    }

    fn test(_sm: &mut StateMachine<Self>, h: &mut H, _id: StateId, condition: DozerCondition) -> SmResult<bool> {
        // (each asks the owner's AI first: none while the AI's constructor enters the default state)
        if h.ai_stack().is_empty() {
            return Ok(false);
        }
        // "busy doing something else"
        if !ai_is_idle(h) {
            return Ok(false);
        }
        let task = get_most_recent_command(h);
        Ok(task
            == match condition {
                DozerCondition::BuildMostImportant => DOZER_TASK_BUILD,
                DozerCondition::RepairMostImportant => DOZER_TASK_REPAIR,
                DozerCondition::FortifyMostImportant => DOZER_TASK_FORTIFY,
            })
    }

    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// The idle state: its version (1), timestamp, player number, mark; an action state: its version, task, then its action
    /// machine's snapshot (whose `xfer` is its version and task only); going home: nothing.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            DozerPrimaryState::Idle { idle_too_long_timestamp, idle_player_number, is_marked_as_idle } => {
                xfer_version_1(xfer)?;
                xfer.xfer_unsigned_int(idle_too_long_timestamp)?;
                xfer.xfer_int(idle_player_number)?;
                xfer.xfer_bool(is_marked_as_idle)?;
            }
            DozerPrimaryState::Action { task, .. } => {
                xfer_version_1(xfer)?;
                xfer_task(xfer, task)?;
                // `xferSnapshot(m_actionMachine)`: `DozerActionStateMachine::xfer`, its version and task (the machine's own task,
                // the same); in CRC mode its `crc`, nothing
                if xfer.get_xfer_mode() != XferMode::Crc {
                    xfer_version_1(xfer)?;
                    let mut t = *task;
                    xfer_task(xfer, &mut t)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn load_post_process(&mut self) {}
}

/// A version 1.
fn xfer_version_1(xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)
}

/// A `DozerTask` (`xferUser`, 4 bytes).
fn xfer_task(xfer: &mut dyn Xfer, task: &mut i32) -> XferResult<()> {
    let mut b = task.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    *task = i32::from_le_bytes(b);
    Ok(())
}

/// The picking state's update: no target, the task cancelled (a failure); else the start dock point (none: around the target from
/// our side, at its bounding radius; not found, its position; the target ignored as an obstacle), the goal set, a move there.
fn pick_update<H: AiStateHost>(sm: &mut StateMachine<DozerActionState<H>>, h: &mut H, task: i32) -> SmResult<StateReturn> {
    if h.ai_stack().is_empty() {
        return Ok(StateReturn::FAILURE);
    }
    let goal = get_task_target(h, task);
    if !h.object_exists(goal) {
        // "to be clean get rid of the goal object we set"
        sm.set_goal_object(None);
        cancel_task(h, task)?;
        return Ok(StateReturn::FAILURE);
    }
    let goal_pos = match get_dock_point(h, task, DOZER_DOCK_POINT_START) {
        Some(p) if !plant(3711) => p,
        _ => {
            let me = h.position();
            let gp = h.object_position(goal);
            let v = Coord2D { x: me.x - gp.x, y: me.y - gp.y };
            let radius = h.object_bounding_sphere_radius(goal);
            let found = h.find_position_around_options(&gp, radius, radius, false, Some(v.to_angle()), 1e10, INVALID_ID);
            // "we only ignore the goal object when we did the point selection in this more "random" method"
            ai(h).ignore_obstacle(goal);
            found.unwrap_or(gp)
        }
    };
    let o = h.find_object(goal);
    sm.set_goal_object(o.as_ref());
    sm.set_goal_position(Some(&goal_pos));
    ai(h).ignore_obstacle(goal);
    with_ai_machine(h, |asm, h| commands::ai_move_to_position(asm, h, &goal_pos, CMD_FROM_AI))?;
    Ok(StateReturn::SUCCESS)
}

/// The moving state's update: another dozer repairing it, or another builder building it, the task complete (a failure); near
/// enough (70, or our bounding radius and 15), a build's target now being constructed (a success); idle, a failure.
fn move_update<H: AiStateHost>(sm: &mut StateMachine<DozerActionState<H>>, h: &mut H, task: i32) -> SmResult<StateReturn> {
    let Some(goal) = sm.goal_object(h).map(|o| o.id) else {
        return Ok(StateReturn::FAILURE);
    };
    let me = h.object_id();
    let current_repairer = h.object_sole_healing_benefactor(goal);
    if task == DOZER_TASK_REPAIR && current_repairer != INVALID_ID && current_repairer != me && !plant(3712) {
        // "oops I guess someone beat me to it!"
        internal_task_complete(h, task);
        sm.set_goal_object(None);
        return Ok(StateReturn::FAILURE);
    }
    if task == DOZER_TASK_BUILD && h.object_builder_id(goal) != me && !plant(3713) {
        // "Double oops, someone else has taken over the build responsibility for this building."
        internal_task_complete(h, task);
        sm.set_goal_object(None);
        return Ok(StateReturn::FAILURE);
    }
    let goal_pos = *sm.goal_position();
    let dist_sqr = h.distance_squared_to_position(&goal_pos, DistanceKind::BoundingSphere2d);
    const SLOP: f32 = 15.0;
    let r = h.bounding_sphere_radius() + SLOP;
    let allowable_distance_sqr = sqr(if MIN_ACTION_TOLERANCE > r { MIN_ACTION_TOLERANCE } else { r });
    if dist_sqr <= allowable_distance_sqr && !plant(3714) {
        if task == DOZER_TASK_BUILD {
            h.object_clear_and_set_model_condition_flags(
                goal,
                &[MODELCONDITION_AWAITING_CONSTRUCTION],
                &[MODELCONDITION_PARTIALLY_CONSTRUCTED, MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED],
            );
        }
        return Ok(StateReturn::SUCCESS);
    }
    if ai_is_idle(h) {
        return Ok(StateReturn::FAILURE);
    }
    Ok(StateReturn::CONTINUE)
}

/// The build at the dock: to the action dock point, once idle there the site's sound; then per frame the construction percent
/// and the health up a frame's share, the producer and builder ours; at 100 %, complete (the statuses, the sound, the conditions,
/// the player's notices, the create modules, the partition, the upgrades, the UI), and off to the end dock point.
fn do_build<H: AiStateHost>(h: &mut H, goal: ObjectId) -> SmResult<Option<bool>> {
    if !h.object_same_controlling_player(goal) {
        // "Yipes, SOmehow I have changed sides in mid build!"
        return Ok(None);
    }
    if dz(h).build_sub_task == DOZER_SELECT_BUILD_DOCK_LOCATION {
        if let Some(dock_location) = get_dock_point(h, DOZER_TASK_BUILD, DOZER_DOCK_POINT_ACTION) {
            with_ai_machine(h, |sm, h| commands::ai_move_to_position(sm, h, &dock_location, CMD_FROM_AI))?;
        }
        dz(h).build_sub_task = DOZER_MOVING_TO_BUILD_DOCK_LOCATION;
    }
    if dz(h).build_sub_task == DOZER_MOVING_TO_BUILD_DOCK_LOCATION && ai_is_idle(h) && !plant(3715) {
        dz(h).build_sub_task = DOZER_DO_BUILD_AT_DOCK;
        start_building_sound(h, goal);
    }
    if dz(h).build_sub_task != DOZER_DO_BUILD_AT_DOCK {
        return Ok(Some(false));
    }
    h.set_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
    let frames_to_build = h.object_time_to_build(goal);
    let percent_progress_this_frame = 100.0f32 / frames_to_build as f32;
    let pct = h.object_construction_percent(goal) + percent_progress_this_frame;
    h.object_set_construction_percent(goal, pct);
    let max_health = h.object_max_health(goal);
    h.object_internal_change_health(goal, max_health / frames_to_build as f32);
    let me = h.object_id();
    h.object_set_producer(goal, me);
    h.object_set_builder(goal, me);
    if h.object_construction_percent(goal) < 100.0 || plant(3716) {
        return Ok(Some(false));
    }
    h.object_clear_status(goal, OBJECT_STATUS_UNDER_CONSTRUCTION);
    h.object_clear_status(goal, OBJECT_STATUS_RECONSTRUCTING);
    finish_building_sound(h);
    h.object_clear_model_condition_flags(
        goal,
        &[MODELCONDITION_AWAITING_CONSTRUCTION, MODELCONDITION_PARTIALLY_CONSTRUCTED, MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED],
    );
    h.object_set_construction_percent(goal, CONSTRUCTION_COMPLETE);
    h.object_evaluate_visual_condition(goal);
    if h.controlling_player_type().is_some() {
        let is_rebuild = dz(h).is_rebuild;
        h.player_on_structure_construction_complete(goal, is_rebuild);
        h.academy_record_production(goal);
        // "Now at finish is when we want the Game side of OnCreate"
        h.object_on_build_complete(goal);
    }
    h.object_handle_partition_cell_maintenance(goal);
    h.object_update_upgrade_modules(goal);
    if h.is_locally_controlled(me) {
        h.dozer_construction_complete_ui(goal);
    }
    let end_pos = get_dock_point(h, DOZER_TASK_BUILD, DOZER_DOCK_POINT_END);
    let mut pos = end_pos.unwrap_or_else(|| h.position());
    // "Our goal may be inside a building, if we are packing them in tight, so try adjusting. jba."
    h.pf_adjust_to_possible_destination(&mut pos);
    with_ai_machine(h, |sm, h| commands::ai_move_to_position(sm, h, &pos, CMD_FROM_AI))?;
    Ok(Some(true))
}

/// The repair: at full health complete; else a bridge tower's scaffolding made, constructing, and (unless the scaffold is still
/// moving) a second's share of the repair rate healed: refused (another heals it), the task complete (a failure).
fn do_repair<H: AiStateHost>(h: &mut H, goal: ObjectId) -> SmResult<Option<bool>> {
    if h.object_health(goal).is_some_and(|(health, max)| health == max) && !plant(3717) {
        // "issue repair complete message"
        h.in_game_ui_message(b"DOZER:RepairComplete");
        return Ok(Some(true));
    }
    let mut can_heal = true;
    let tower = h.object_is_kind_of(goal, KINDOF_BRIDGE_TOWER);
    if tower {
        bridge_scaffolding(h, goal, true);
    }
    h.set_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
    if tower {
        let Some(bridge) = h.bridge_tower_bridge_id(goal) else {
            return Err(StateMachineError::Undefined("a dozer repairing a bridge tower without its interface"));
        };
        let in_motion = if h.object_exists(bridge) { h.bridge_scaffold_in_motion(bridge) } else { None };
        let Some(in_motion) = in_motion else {
            return Err(StateMachineError::Undefined("a dozer repairing a bridge tower whose bridge has no interface"));
        };
        if in_motion && !plant(3718) {
            can_heal = false;
        }
    }
    if can_heal {
        let max_health = h.object_max_health(goal);
        let health = max_health * dz_data(h).repair_health_percent_per_second / LOGICFRAMES_PER_SECOND as f32;
        let me = h.object_id();
        if !h.object_attempt_healing_from_sole_benefactor(goal, health, me, 2) && !plant(3719) {
            return Ok(None);
        }
    }
    Ok(Some(false))
}

/// The doing state's update: the target gone, or the dozer unmanned, a failure; the task's work; complete, the task complete, the
/// goal cleared, mine clearing allowed (a success); else mine clearing disallowed.
fn do_update<H: AiStateHost>(sm: &mut StateMachine<DozerActionState<H>>, h: &mut H, task: i32) -> SmResult<StateReturn> {
    if h.ai_stack().is_empty() {
        return Ok(StateReturn::FAILURE);
    }
    let Some(goal) = sm.goal_object(h).map(|o| o.id) else {
        return Ok(StateReturn::FAILURE);
    };
    if h.is_disabled_by_type(crate::common::disabled_types::DISABLED_UNMANNED) {
        // "Yipes, I've been sniped!"
        return Ok(StateReturn::FAILURE);
    }
    let complete = match task {
        DOZER_TASK_BUILD => match do_build(h, goal)? {
            Some(c) => c,
            None => return Ok(StateReturn::FAILURE),
        },
        DOZER_TASK_REPAIR => match do_repair(h, goal)? {
            Some(c) => c,
            None => {
                internal_task_complete(h, task);
                sm.set_goal_object(None);
                return Ok(StateReturn::FAILURE);
            }
        },
        // "@todo write me"
        DOZER_TASK_FORTIFY => false,
        // "Unknown task for the dozer action do action state" (a DEBUG_CRASH)
        _ => return Ok(StateReturn::FAILURE),
    };
    if complete {
        internal_task_complete(h, task);
        sm.set_goal_object(None);
        h.set_weapon_set_flag(WEAPONSET_MINE_CLEARING_DETAIL, true);
        return Ok(StateReturn::SUCCESS);
    }
    // "no mine clearing fun while I'm on the job"
    h.set_weapon_set_flag(WEAPONSET_MINE_CLEARING_DETAIL, false);
    Ok(StateReturn::CONTINUE)
}

impl<H: AiStateHost> StateBehavior for DozerActionState<H> {
    type Ctx = H;
    type Condition = NoCondition;

    fn on_enter(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        if let Some(DozerActionState::DoAction { task, .. }) = sm.state(id) {
            let task = *task;
            if h.ai_stack().is_empty() {
                return Ok(StateReturn::FAILURE);
            }
            let f = frame(h);
            if let Some(DozerActionState::DoAction { enter_frame, .. }) = sm.state_mut(id) {
                *enter_frame = f;
            }
            // "when building, we have additional movement that we will for docking"
            if task == DOZER_TASK_BUILD {
                dz(h).build_sub_task = DOZER_SELECT_BUILD_DOCK_LOCATION;
            }
        }
        Ok(StateReturn::CONTINUE)
    }

    fn on_exit(_sm: &mut StateMachine<Self>, _h: &mut H, _id: StateId, _status: StateExitType) -> SmResult<()> {
        Ok(())
    }

    fn update(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(DozerActionState::PickActionPos { task, .. }) => {
                let task = *task;
                pick_update(sm, h, task)
            }
            Some(DozerActionState::MoveToActionPos { task }) => {
                let task = *task;
                move_update(sm, h, task)
            }
            Some(DozerActionState::DoAction { task, .. }) => {
                let task = *task;
                do_update(sm, h, task)
            }
            Some(DozerActionState::_Host(never, _)) => match *never {},
            None => Ok(StateReturn::CONTINUE),
        }
    }

    fn test(_sm: &mut StateMachine<Self>, _h: &mut H, _id: StateId, condition: NoCondition) -> SmResult<bool> {
        match condition {}
    }

    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// Each its version (1) and task; the picking state its failed attempts, the doing state its enter frame.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer_version_1(xfer)?;
        match self {
            DozerActionState::PickActionPos { task, failed_attempts } => {
                xfer_task(xfer, task)?;
                xfer.xfer_int(failed_attempts)?;
            }
            DozerActionState::MoveToActionPos { task } => xfer_task(xfer, task)?,
            DozerActionState::DoAction { task, enter_frame } => {
                xfer_task(xfer, task)?;
                xfer.xfer_unsigned_int(enter_frame)?;
            }
            DozerActionState::_Host(never, _) => match *never {},
        }
        Ok(())
    }

    fn load_post_process(&mut self) {}
}

/// `DozerPrimaryStateMachine::xfer`: its version (1), then the machine's.
pub fn dozer_machine_xfer<H: AiStateHost>(m: &mut StateMachine<DozerPrimaryState<H>>, xfer: &mut dyn Xfer) -> SmResult<()> {
    xfer_version_1(xfer)?;
    m.xfer_machine(xfer)
}

/// `DozerAIUpdate::xfer`'s members after the base's, before and after the machine's snapshot (`machine` runs it): the task count
/// (3, else the load fails, `SC_INVALID_DATA`), the tasks, the machine, the current task, the dock point count (3), the dock
/// points, the build sub-task.
pub fn dozer_xfer_members(m: &mut DozerMembers, xfer: &mut dyn Xfer, machine: impl FnOnce(&mut dyn Xfer) -> SmResult<()>) -> SmResult<()> {
    let mut num_tasks = DOZER_NUM_TASKS as i32;
    xfer.xfer_int(&mut num_tasks)?;
    if num_tasks != DOZER_NUM_TASKS as i32 {
        return Err(StateMachineError::InvalidData);
    }
    for t in m.task.iter_mut() {
        xfer.xfer_object_id(&mut t.target_object_id)?;
        xfer.xfer_unsigned_int(&mut t.task_order_frame)?;
    }
    machine(xfer)?;
    xfer_task(xfer, &mut m.current_task)?;
    let mut dock_points = DOZER_NUM_DOCK_POINTS as i32;
    xfer.xfer_int(&mut dock_points)?;
    if dock_points != DOZER_NUM_DOCK_POINTS as i32 {
        return Err(StateMachineError::InvalidData);
    }
    for task in m.dock_point.iter_mut() {
        for (valid, location) in task.iter_mut() {
            xfer.xfer_bool(valid)?;
            xfer.xfer_coord3d(location)?;
        }
    }
    xfer_task(xfer, &mut m.build_sub_task)?;
    Ok(())
}

/// `construct(what, pos, angle, owningPlayer, isRebuild)` (`what` a template index; the owning player the dozer's): the rebuild
/// flag set; no template or player, none; a player's (not the computer's) new building checked (makeable, the location legal);
/// the new object (under construction, reconstructing for a rebuild) produced and built by us, paid for (not a rebuild), placed,
/// the terrain flattened under it and the object put on it, in the pathfinder's map, announced, at 0 % and one hit point,
/// awaiting construction; then our build task. The new object's id (none: `INVALID_ID`).
pub fn construct<H: AiStateHost>(h: &mut H, what: Option<u32>, pos: &Coord3D, angle: f32, is_rebuild: bool) -> SmResult<ObjectId> {
    dz(h).is_rebuild = is_rebuild;
    let Some(what) = what else { return Ok(INVALID_ID) };
    let Some(player_type) = h.controlling_player_type() else { return Ok(INVALID_ID) };
    let is_worker = is_worker(h);
    if !is_rebuild {
        // "AI has weaker restriction on building"
        if player_type != PlayerType::Computer {
            if !h.build_assistant_can_make_unit(what) {
                return Ok(INVALID_ID);
            }
            if !h.build_assistant_is_location_legal_to_build(what, pos, angle, BUILD_CHECKS) && !plant(3720) {
                return Ok(INVALID_ID);
            }
        } else if is_worker && !h.build_assistant_is_location_legal_to_build(what, pos, angle, CLEAR_PATH | NO_OBJECT_OVERLAP) && !plant(3737) {
            // (batch A39: the worker's checks a computer player's location, weakly)
            return Ok(INVALID_ID);
        }
    }
    let obj = h.thing_factory_new_structure(what, is_rebuild);
    let me = h.object_id();
    h.object_set_producer(obj, me);
    h.object_set_builder(obj, me);
    if is_worker {
        // "leave the supply truck state and now behave like a dozer."
        worker::exiting_supply_truck_state(h)?;
    }
    if !is_rebuild {
        let cost = h.template_cost_to_build(what);
        if !h.player_money_withdraw(cost) {
            return Err(StateMachineError::Undefined("a dozer's construction without the player's money"));
        }
    }
    if is_worker && !plant(3738) {
        // "set a bit that this object is under construction, it is important to do this early before the hooks add/subtract power
        // from a player are executed"
        h.object_set_status(obj, OBJECT_STATUS_UNDER_CONSTRUCTION);
    }
    h.new_structure_set_position(obj, pos);
    h.new_structure_set_orientation(obj, angle);
    // "Flatten the terrain underneath the object, then adjust to the flattened height. jba."
    h.terrain_flatten(obj);
    let mut adjusted_pos = *pos;
    adjusted_pos.z = h.ground_height(pos.x, pos.y);
    h.new_structure_set_position(obj, &adjusted_pos);
    h.pf_add_object_to_pathfind_map(obj);
    h.player_on_structure_created(obj);
    h.object_set_construction_percent(obj, 0.0);
    // "newly constructed objects start at one hit point"
    let health = h.object_health(obj).map_or(0.0, |(health, _)| health);
    h.object_internal_change_health(obj, -health + 1.0);
    h.object_clear_and_set_model_condition_flags(
        obj,
        &[MODELCONDITION_PARTIALLY_CONSTRUCTED, MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED],
        &[MODELCONDITION_AWAITING_CONSTRUCTION],
    );
    new_task(h, DOZER_TASK_BUILD, obj)?;
    Ok(obj)
}

/// `onDelete()`: every pending task cancelled; then the targets (by now none) no longer being constructed.
pub fn on_delete<H: AiStateHost>(h: &mut H) -> SmResult<()> {
    for t in 0..DOZER_NUM_TASKS as i32 {
        if is_task_pending(h, t) {
            cancel_task(h, t)?;
        }
    }
    for t in 0..DOZER_NUM_TASKS {
        let goal = dz(h).task[t].target_object_id;
        if h.object_exists(goal) {
            h.object_clear_model_condition_state(goal, MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED);
        }
    }
    Ok(())
}
