// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIDock.cpp (AIDockMachine and its states)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIDock.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIDockState)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's dock state and its docking machine (`p1w13-sonnet`, layer 10, batch A17).
//!
//! `AIDockState` (`AI_DOCK`) runs an `AIDockMachine` with the goal object (a dock) as its goal: approach a reserved queue
//! position, wait for clearance (advancing in line when the dock allows), move to the entry, into the dock (the machine locked),
//! invoke the dock's action until it is done, move to the exit, then to the dock's rally point if it gives one. Each step tells the
//! dock what it reached, or cancels when interrupted or the dock closes.
//!
//! The docking machine is a [`StateMachine`] of [`AiState`] held by the dock state; its `m_approachPosition` is the dock state's
//! `approach_position`, lent on the attack frame while the machine runs ([`AttackFrame::dock_approach`]).
//!
//! # Upstream facts kept
//!
//! - `AIDockMoveToDockState::update` asks the goal's dock unchecked (a goal without one is not generated).
//! - The process state's action delay and drone id are not saved (its `xfer` is the version only).
//! - The machine's `halt` cancels the dock before halting (the dock state's exit halts, then deletes it).

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::KINDOF_DRONE;
use crate::common::state_machine::{
    SmResult, StateConditionInfo, StateContext, StateExitType, StateId, StateMachine, StateReturn, EXIT_MACHINE_WITH_FAILURE,
    EXIT_MACHINE_WITH_SUCCESS,
};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_attack_move::run_sub_machine;
use crate::logic::ai::ai_move_states::{self as move_states, mv, MoveToData};
use crate::logic::ai::ai_states::{ai, AiCondition, AiState, AiStateHost};
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `AI_DOCK_APPROACH` .. `AI_DOCK_MOVE_TO_RALLY`.
pub const AI_DOCK_APPROACH: StateId = 0;
pub const AI_DOCK_WAIT_FOR_CLEARANCE: StateId = 1;
pub const AI_DOCK_ADVANCE_POSITION: StateId = 2;
pub const AI_DOCK_MOVE_TO_ENTRY: StateId = 3;
pub const AI_DOCK_MOVE_TO_DOCK: StateId = 4;
pub const AI_DOCK_PROCESS_DOCK: StateId = 5;
pub const AI_DOCK_MOVE_TO_EXIT: StateId = 6;
pub const AI_DOCK_MOVE_TO_RALLY: StateId = 7;

/// The dock's positions (`getEnterPosition`, `getDockPosition`, `getExitPosition`).
pub const DOCK_ENTER: u32 = 0;
pub const DOCK_DOCK: u32 = 1;
pub const DOCK_EXIT: u32 = 2;
/// What the docker reached (`onApproachReached`, `onEnterReached`, `onDockReached`, `onExitReached`).
pub const REACHED_APPROACH: u32 = 0;
pub const REACHED_ENTER: u32 = 1;
pub const REACHED_DOCK: u32 = 2;
pub const REACHED_EXIT: u32 = 3;

/// `AIDockMachine::AIDockMachine(obj)`: "order matters: first state is the default state."
pub fn new_dock_machine<H: AiStateHost>() -> Sm<H> {
    let mut m = StateMachine::new();
    let dm = |which| AiState::DockMove { which, mv: MoveToData::default() };
    let wait_conditions = [StateConditionInfo { test: AiCondition::DockAbleToAdvance, to_state_id: AI_DOCK_ADVANCE_POSITION }];
    let defs: [(StateId, AiState<H>, StateId, StateId, &[StateConditionInfo<AiCondition>]); 8] = [
        (AI_DOCK_APPROACH, dm(AI_DOCK_APPROACH), AI_DOCK_WAIT_FOR_CLEARANCE, EXIT_MACHINE_WITH_FAILURE, &[]),
        (AI_DOCK_WAIT_FOR_CLEARANCE, AiState::DockWait { enter_frame: 0 }, AI_DOCK_MOVE_TO_ENTRY, EXIT_MACHINE_WITH_FAILURE, &wait_conditions),
        (AI_DOCK_ADVANCE_POSITION, dm(AI_DOCK_ADVANCE_POSITION), AI_DOCK_WAIT_FOR_CLEARANCE, EXIT_MACHINE_WITH_FAILURE, &[]),
        (AI_DOCK_MOVE_TO_ENTRY, dm(AI_DOCK_MOVE_TO_ENTRY), AI_DOCK_MOVE_TO_DOCK, AI_DOCK_MOVE_TO_EXIT, &[]),
        (AI_DOCK_MOVE_TO_DOCK, dm(AI_DOCK_MOVE_TO_DOCK), AI_DOCK_PROCESS_DOCK, AI_DOCK_MOVE_TO_EXIT, &[]),
        // (AIDockProcessDockState's constructor: the action frame 0; the drone id the pool's zero)
        (AI_DOCK_PROCESS_DOCK, AiState::DockProcess { next_action_frame: 0, drone_id: INVALID_ID }, AI_DOCK_MOVE_TO_EXIT, AI_DOCK_MOVE_TO_EXIT, &[]),
        (AI_DOCK_MOVE_TO_EXIT, dm(AI_DOCK_MOVE_TO_EXIT), AI_DOCK_MOVE_TO_RALLY, EXIT_MACHINE_WITH_FAILURE, &[]),
        (AI_DOCK_MOVE_TO_RALLY, dm(AI_DOCK_MOVE_TO_RALLY), EXIT_MACHINE_WITH_SUCCESS, EXIT_MACHINE_WITH_FAILURE, &[]),
    ];
    for (id, state, success, failure, conditions) in defs {
        m.define_state(id, state, success, failure, conditions).expect("the dock machine");
    }
    m
}

/// The approach position lent with the docking machine (`m_approachPosition`).
fn approach_position<H: AiStateHost>(h: &mut H) -> &mut i32 {
    &mut h.attack_stack().last_mut().expect("the docking machine's frame").dock_approach
}

/// `AIDockMachine::ableToAdvance`: the dock lets the owner advance from its approach position.
pub fn able_to_advance<H: AiStateHost>(sm: &Sm<H>, h: &mut H) -> bool {
    let Some(goal) = sm.goal_object(h) else { return false };
    if !h.object_has_dock(goal.id) {
        return false;
    }
    let index = *approach_position(h);
    // "if the dock says we can advance, then sidetrack to the scoot forward state"
    h.dock_is_clear_to_advance(goal.id, index)
}

/// The goal's dock (`getMachineGoalObject()->getDockUpdateInterface()`): the goal's id if it has one.
fn dock_of<H: AiStateHost>(sm: &Sm<H>, h: &mut H) -> Option<ObjectId> {
    sm.goal_object(h).map(|g| g.id).filter(|&g| h.object_has_dock(g))
}

/// `AIDockMachine::halt()`: "We need to say goodbye, or we will leave our spot taken forever."
pub fn dock_machine_halt<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) {
    if let Some(dock) = dock_of(sm, h) {
        h.dock_cancel(dock);
    }
    sm.halt();
}

// ---- the docking machine's states

/// The move states' `onEnter`.
pub fn dock_move_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId) -> SmResult<StateReturn> {
    let Some(dock) = dock_of(sm, h) else { return Ok(StateReturn::FAILURE) };
    match which {
        AI_DOCK_APPROACH | AI_DOCK_ADVANCE_POSITION => {
            // "fail if the dock is closed"
            if !h.dock_is_open(dock) {
                h.dock_cancel(dock);
                return Ok(StateReturn::FAILURE);
            }
            // "get a good place to wait from the dock"
            let Some((pos, index)) = h.dock_reserve_approach(dock, which == AI_DOCK_ADVANCE_POSITION) else {
                return Ok(StateReturn::FAILURE); // "dock is full"
            };
            mv(sm, id).goal_position = pos;
            *approach_position(h) = index;
            ai(h).ignore_obstacle(INVALID_ID);
        }
        AI_DOCK_MOVE_TO_ENTRY | AI_DOCK_MOVE_TO_DOCK => {
            if !h.dock_is_open(dock) {
                h.dock_cancel(dock);
                return Ok(StateReturn::FAILURE);
            }
            if which == AI_DOCK_MOVE_TO_ENTRY {
                if h.dock_is_allow_passthrough(dock) {
                    ai(h).ignore_obstacle(dock);
                }
                mv(sm, id).goal_position = h.dock_position(dock, DOCK_ENTER);
                *approach_position(h) = if plant(3390) { 0 } else { -1 };
            } else {
                mv(sm, id).goal_position = h.dock_position(dock, DOCK_DOCK);
                if h.dock_is_allow_passthrough(dock) {
                    ai(h).ignore_obstacle(dock);
                    mv(sm, id).adjust_destinations = false;
                }
                // "since we are moving inside the dock, disallow interruptions"
                if !plant(3391) {
                    sm.lock();
                }
            }
        }
        AI_DOCK_MOVE_TO_EXIT => {
            mv(sm, id).goal_position = h.dock_position(dock, DOCK_EXIT);
            if h.dock_is_allow_passthrough(dock) {
                ai(h).ignore_obstacle(dock);
                mv(sm, id).adjust_destinations = false;
            }
        }
        _ => {
            // AI_DOCK_MOVE_TO_RALLY: "if they don't have anywhere to send us, then we are good"
            let rally = if h.dock_is_rally_point_after_dock(dock) { h.object_rally_point(dock) } else { None };
            let Some(rally) = rally else { return Ok(StateReturn::SUCCESS) };
            mv(sm, id).goal_position = rally;
        }
    }
    // "this behavior is an extention of basic MoveTo"
    move_states::internal_on_enter(sm, h, id)
}

/// The move states' `update`: without the goal, failure (but the rally state's); the move-to-dock state fails when the dock
/// closes.
pub fn dock_move_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId) -> SmResult<StateReturn> {
    if which != AI_DOCK_MOVE_TO_RALLY {
        let Some(goal) = sm.goal_object(h) else { return Ok(StateReturn::FAILURE) };
        if which == AI_DOCK_MOVE_TO_DOCK && !h.dock_is_open(goal.id) {
            return Ok(StateReturn::FAILURE);
        }
    }
    move_states::internal_update(sm, h, id)
}

/// The move states' `onExit`: the dock told what was reached, or (interrupted, or closed) cancelled; the move-to-dock and exit
/// states unlock the machine.
pub fn dock_move_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, which: StateId, status: StateExitType) {
    let dock = dock_of(sm, h);
    let reached = match which {
        AI_DOCK_APPROACH | AI_DOCK_ADVANCE_POSITION => Some(REACHED_APPROACH),
        AI_DOCK_MOVE_TO_ENTRY => Some(REACHED_ENTER),
        AI_DOCK_MOVE_TO_DOCK => Some(REACHED_DOCK),
        AI_DOCK_MOVE_TO_EXIT => Some(REACHED_EXIT),
        _ => None,
    };
    if let (Some(dock), Some(reached)) = (dock, reached) {
        // "if we were interrupted, let the dock know we're not coming" (the exit state tells it in any case)
        if reached != REACHED_EXIT && (status == StateExitType::Reset || !h.dock_is_open(dock)) && !plant(3392) {
            h.dock_cancel(dock);
        } else {
            h.dock_reached(dock, reached);
        }
    }
    if matches!(which, AI_DOCK_MOVE_TO_DOCK | AI_DOCK_MOVE_TO_EXIT) {
        sm.unlock();
    }
    move_states::internal_on_exit(sm, h, id);
}

/// `AIDockWaitForClearanceState::onEnter()`: the frame it began waiting.
pub fn wait_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let now = StateContext::frame(h);
    if let Some(AiState::DockWait { enter_frame }) = sm.state_mut(id) {
        *enter_frame = now;
    }
    Ok(StateReturn::CONTINUE)
}

/// `AIDockWaitForClearanceState::update()`: until the dock clears the owner to enter (closed: cancelled, failure); thirty seconds
/// at most.
pub fn wait_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let Some(dock) = dock_of(sm, h) else { return Ok(StateReturn::FAILURE) };
    if !h.dock_is_open(dock) {
        h.dock_cancel(dock);
        return Ok(StateReturn::FAILURE);
    }
    // "if the dock says we can enter, our wait is over"
    if h.dock_is_clear_to_enter(dock) {
        return Ok(StateReturn::SUCCESS);
    }
    let enter_frame = match sm.state(id) {
        Some(AiState::DockWait { enter_frame }) => *enter_frame,
        _ => unreachable!("the wait state"),
    };
    let limit = if plant(3393) { 3 } else { 30 } * LOGICFRAMES_PER_SECOND as u32;
    if enter_frame.wrapping_add(limit) < StateContext::frame(h) {
        return Ok(StateReturn::FAILURE);
    }
    Ok(StateReturn::CONTINUE)
}

/// `AIDockWaitForClearanceState::onExit(status)`: closed or interrupted, cancelled.
pub fn wait_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, status: StateExitType) {
    if let Some(dock) = dock_of(sm, h) {
        if !h.dock_is_open(dock) || status == StateExitType::Reset {
            h.dock_cancel(dock);
        }
    }
}

/// `AIDockWaitForClearanceState::xfer` (version 2): the frame it began waiting.
pub fn wait_xfer(enter_frame: &mut u32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 2;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_unsigned_int(enter_frame)
}

/// `AIDockProcessDockState::setNextDockActionFrame()`: a supply truck's delay for the dock, else now.
fn set_next_dock_action_frame<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let now = StateContext::frame(h);
    let goal = sm.goal_object_id();
    let next = match h.supply_truck_action_delay(goal) {
        Some(delay) => now.wrapping_add(delay),
        None => now,
    };
    if let Some(AiState::DockProcess { next_action_frame, .. }) = sm.state_mut(id) {
        *next_action_frame = next;
    }
}

/// `AIDockProcessDockState::onEnter()`.
pub fn process_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if dock_of(sm, h).is_none() {
        return Ok(StateReturn::FAILURE);
    }
    set_next_dock_action_frame(sm, h, id);
    Ok(StateReturn::CONTINUE)
}

/// `AIDockProcessDockState::findMyDrone()`: the cached drone if it still exists, else the first of the player's objects that is a
/// drone the owner produced (cached).
fn find_my_drone<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> ObjectId {
    let cached = match sm.state(id) {
        Some(AiState::DockProcess { drone_id, .. }) => *drone_id,
        _ => unreachable!("the process state"),
    };
    // "First do the fast cached check."
    if h.object_exists(cached) {
        return cached;
    }
    let me = h.object_id();
    let mut drone = INVALID_ID;
    if h.controlling_player_type().is_some() {
        for o in h.player_object_ids() {
            if h.object_is_kind_of(o, KINDOF_DRONE) && h.object_producer_id(o) == me && !plant(3394) {
                drone = o;
                break;
            }
        }
    }
    if drone != INVALID_ID {
        if let Some(AiState::DockProcess { drone_id, .. }) = sm.state_mut(id) {
            *drone_id = drone;
        }
    }
    drone
}

/// `AIDockProcessDockState::update()`: "Invoke the dock's action() method until it returns false" (or the dock closes), after
/// the docker's delay.
pub fn process_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let Some(dock) = dock_of(sm, h) else { return Ok(StateReturn::FAILURE) };
    let next = match sm.state(id) {
        Some(AiState::DockProcess { next_action_frame, .. }) => *next_action_frame,
        _ => unreachable!("the process state"),
    };
    // "Some dockers can have a delay built in"
    if StateContext::frame(h) < next {
        return Ok(StateReturn::CONTINUE);
    }
    set_next_dock_action_frame(sm, h, id);
    let drone = find_my_drone(sm, h, id);
    if !h.dock_is_open(dock) || !h.dock_action(dock, drone) {
        return Ok(StateReturn::SUCCESS);
    }
    Ok(StateReturn::CONTINUE)
}

/// `AIDockProcessDockState::onExit(status)`: the machine unlocked.
pub fn process_on_exit<H: AiStateHost>(sm: &mut Sm<H>) {
    sm.unlock();
}

// ---- AIDockState

/// `AIDockState::onEnter()`: the goal must be a dock; the pathfinder ignores it; the docking machine made, its goal the dock, its
/// default state entered (its answer the state's).
pub fn dock_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let Some(goal) = sm.goal_object(h) else {
        return Ok(StateReturn::FAILURE); // "we have nothing to dock with!"
    };
    if !h.object_has_dock(goal.id) {
        return Ok(StateReturn::FAILURE);
    }
    // "tell the pathfinder to ignore the object we are docking with, so it doesn't block us"
    ai(h).ignore_obstacle(goal.id);
    match sm.state_mut(id) {
        Some(AiState::Dock { machine, approach_position, .. }) => {
            *machine = Some(new_dock_machine());
            *approach_position = -1;
        }
        _ => unreachable!("the dock state"),
    }
    run_sub_machine(sm, h, id, |m, h| {
        m.set_goal_object(Some(&goal));
        m.init_default_state(h)
    })
}

/// `AIDockState::onExit(status)`: the docking machine halted ("you have to halt before you delete to do cleanup") and deleted;
/// the owner no longer paths through units or ignores its goal.
pub fn dock_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<()> {
    if matches!(sm.state(id), Some(AiState::Dock { machine: Some(_), .. })) {
        run_sub_machine(sm, h, id, |m, h| {
            dock_machine_halt(m, h);
            Ok(())
        })?;
        if let Some(AiState::Dock { machine, .. }) = sm.state_mut(id) {
            *machine = None;
        }
    }
    ai(h).set_can_path_through_units(false);
    ai(h).ignore_obstacle(INVALID_ID);
    Ok(())
}

/// `AIDockState::update()`: the owner paths through units; the docking machine's update, a sleep a continue.
pub fn dock_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    ai(h).set_can_path_through_units(!plant(3395));
    Ok(run_sub_machine(sm, h, id, |m, h| m.update_state_machine(h))?.convert_sleep_to_continue())
}

/// `AIDockState::xfer` (version 1): whether there is a machine (one made to load into), the machine (`AIDockMachine::xfer`: its
/// version, the machine's, the approach position), whether it uses precision movement.
pub fn dock_xfer<H: AiStateHost>(
    machine: &mut Option<Sm<H>>,
    approach_position: &mut i32,
    using_precision_movement: &mut bool,
    xfer: &mut dyn Xfer,
) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    let mut has_machine = machine.is_some();
    xfer.xfer_bool(&mut has_machine)?;
    if has_machine && machine.is_none() {
        *machine = Some(new_dock_machine());
        *approach_position = -1;
    }
    if let Some(m) = machine.as_mut().filter(|_| has_machine) {
        let cv: XferVersion = 1;
        let mut v = cv;
        xfer.xfer_version(&mut v, cv)?;
        Snapshot::xfer(m, xfer)?;
        xfer.xfer_int(approach_position)?;
    }
    xfer.xfer_bool(using_precision_movement)
}

/// The docking machine's move states' `xfer`: the approach state's (version 2: the base's), the others' the base's.
pub fn dock_move_xfer(which: StateId, mv: &mut MoveToData, xfer: &mut dyn Xfer) -> XferResult<()> {
    if which == AI_DOCK_APPROACH {
        let current_version: XferVersion = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        if version >= 2 {
            mv.xfer(xfer)?;
        }
        return Ok(());
    }
    mv.xfer(xfer)
}
