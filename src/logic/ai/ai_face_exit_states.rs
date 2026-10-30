// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIExitState, AIExitInstantlyState, AIFaceState)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIStateMachine.h (those classes)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's exit and face states (`p1w13-sonnet`, layer 10, batch A13).
//!
//! `AIExitState` (`AI_EXIT`) tells the goal object's contain that the owner wants to exit, waits while the goal's AI or its exit is
//! busy, then reserves a door and exits through it. `AIExitInstantlyState` (`AI_EXIT_INSTANTLY`) exits through the first door on
//! entry. Both tell the contain the owner wants neither on exit. `AIFaceState` (`AI_FACE_OBJECT`, `AI_FACE_POSITION`) turns toward
//! the goal object or position: in place when the locomotor can stop, else by moving toward it.
//!
//! # Upstream facts kept
//!
//! - The exit states succeed only while the machine is still in them after the exit (`exitObjectViaDoor` may set another state).
//! - The face state's turn-in-place flag is its constructor's false until its first entry.

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::state_machine::{SmResult, StateId, StateMachine, StateReturn};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_states::{ai, AiState, AiStateHost};
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `ObjectEnterExitType`'s `WANTS_TO_ENTER`, `WANTS_TO_EXIT`, `WANTS_NEITHER`.
pub const WANTS_TO_ENTER: i32 = 0;
pub const WANTS_TO_EXIT: i32 = 1;
pub const WANTS_NEITHER: i32 = 2;
/// `AIFreeToExitType`'s `WAIT_TO_EXIT`.
pub const WAIT_TO_EXIT: i32 = 2;
/// `ExitDoorType`'s `DOOR_1`, `DOOR_NONE_AVAILABLE`.
pub const DOOR_1: i32 = 0;
pub const DOOR_NONE_AVAILABLE: i32 = -1;

/// `REL_THRESH`: "about 2 degrees. (getRelativeAngle2D is current only accurate to about 1.25 degrees)".
const REL_THRESH: f32 = 0.035;

fn entry_to_clear<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> &mut ObjectId {
    match sm.state_mut(id) {
        Some(AiState::Exit { entry_to_clear, .. }) => entry_to_clear,
        _ => unreachable!("an exit state"),
    }
}

/// `onEnter()` (both exit states'): the goal's contain told the owner wants to exit (and remembered); the instant one exits
/// through the first door now.
pub fn exit_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, instantly: bool) -> SmResult<StateReturn> {
    *entry_to_clear(sm, id) = INVALID_ID;
    let Some(goal) = sm.goal_object(h) else { return Ok(StateReturn::FAILURE) };
    if h.object_has_contain(goal.id) {
        h.contain_wants_to_enter_or_exit(goal.id, WANTS_TO_EXIT);
        *entry_to_clear(sm, id) = goal.id;
    }
    if !instantly {
        return Ok(StateReturn::CONTINUE);
    }
    if !h.object_has_contain(goal.id) || !h.contain_has_exit_interface(goal.id) {
        return Ok(StateReturn::FAILURE);
    }
    h.exit_object_via_door(goal.id, if plant(3350) { 1 } else { DOOR_1 });
    // "Not success, because exitViaDoor has changed us to FollowPath, and if we say Success, our machine will think FollowPath
    // succeeded"
    Ok(StateReturn::CONTINUE)
}

/// `AIExitState::update()`: wait while the goal's AI or its exit is busy; then a door, and out through it.
fn exit_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let Some(goal) = sm.goal_object(h) else { return Ok(StateReturn::FAILURE) };
    if h.object_ai_free_to_exit(goal.id) == Some(WAIT_TO_EXIT) && !plant(3351) {
        return Ok(StateReturn::CONTINUE);
    }
    if !h.object_has_contain(goal.id) || !h.contain_has_exit_interface(goal.id) {
        return Ok(StateReturn::FAILURE);
    }
    if h.exit_is_busy(goal.id) && !plant(3352) {
        // "Just wait a sec."
        return Ok(StateReturn::CONTINUE);
    }
    let door = h.exit_reserve_door(goal.id);
    if door == DOOR_NONE_AVAILABLE {
        return Ok(StateReturn::FAILURE);
    }
    h.exit_object_via_door(goal.id, door);
    if sm.get_current_state_id() != id {
        // "Not sucess, because exitViaDoor has changed us to FollowPath"
        Ok(StateReturn::CONTINUE)
    } else {
        Ok(StateReturn::SUCCESS)
    }
}

/// `update()` (both exit states'). The instant one: success while still the machine's state.
pub fn exit_state_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, instantly: bool) -> SmResult<StateReturn> {
    if !instantly {
        return exit_update(sm, h, id);
    }
    if sm.get_current_state_id() != id {
        return Ok(StateReturn::CONTINUE);
    }
    Ok(if plant(3353) { StateReturn::CONTINUE } else { StateReturn::SUCCESS })
}

/// `onExit(status)` (both exit states'): the remembered entry's contain told the owner wants neither ("use this, rather than
/// getMachineGoalObject, in case the goal is killed while we were waiting...").
pub fn exit_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let entry = *entry_to_clear(sm, id);
    if entry != INVALID_ID && !plant(3354) && h.object_exists(entry) && h.object_has_contain(entry) {
        h.contain_wants_to_enter_or_exit(entry, WANTS_NEITHER);
    }
}

/// `xfer` (both exit states', version 1): the entry to clear.
pub fn exit_xfer(entry_to_clear: &mut ObjectId, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_object_id(entry_to_clear)
}

// ---- AIFaceState

/// `AIFaceState::onEnter()`: whether the locomotor turns in place (its minimum speed zero); nothing to face: failure.
pub fn face_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let in_place = ai(h).get_cur_locomotor().is_some_and(|l| l.get_min_speed() == 0.0);
    let obj = match sm.state_mut(id) {
        Some(AiState::Face { obj, can_turn_in_place }) | Some(AiState::JetPause { obj, can_turn_in_place, .. }) => {
            *can_turn_in_place = in_place != plant(3355);
            *obj
        }
        _ => unreachable!("the face state"),
    };
    if obj && sm.goal_object(h).is_none() {
        // "Nothing to face..."
        return Ok(StateReturn::FAILURE);
    }
    Ok(StateReturn::CONTINUE)
}

/// `AIFaceState::update()`: done within two degrees; else the orientation goal (in place) or the position goal.
pub fn face_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let (obj, in_place) = match sm.state(id) {
        Some(AiState::Face { obj, can_turn_in_place }) | Some(AiState::JetPause { obj, can_turn_in_place, .. }) => (*obj, *can_turn_in_place),
        _ => unreachable!("the face state"),
    };
    let mut pos = *sm.goal_position();
    if obj {
        let Some(target) = sm.goal_object(h) else { return Ok(StateReturn::FAILURE) };
        pos = target.position;
    }
    let rel_angle = h.relative_angle_2d_to_position(&pos);
    if rel_angle.abs() < if plant(3356) { 0.35 } else { REL_THRESH } {
        return Ok(StateReturn::SUCCESS);
    }
    // "turnDelta = yawRate() NO, do not get this, it is not useful. (srj)"
    if in_place {
        let desired = h.orientation() + rel_angle;
        ai(h).set_locomotor_goal_orientation(desired);
    } else {
        ai(h).set_locomotor_goal_position_explicit(&pos);
    }
    Ok(StateReturn::CONTINUE)
}

/// `AIFaceState::xfer` (version 1): the turn-in-place flag.
pub fn face_xfer(can_turn_in_place: &mut bool, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_bool(can_turn_in_place)
}
