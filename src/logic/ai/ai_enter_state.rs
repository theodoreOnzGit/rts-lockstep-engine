// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIEnterState)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIStateMachine.h (that class)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's enter state (`p1w13-sonnet`, layer 10, batch A14).
//!
//! `AIEnterState` (`AI_ENTER`) moves into the goal object (the pathfinder ignoring it, the locomotor allowed off the map) as
//! `AIInternalMoveToState` does, while the action manager lets the owner enter it; an enemy it may no longer enter it attacks
//! instead. Held (by the container) is entered; arrived close enough without being taken in, it adds itself to the contain ("the
//! partition manager doesn't generate collisions in the border area").
//!
//! # Upstream facts kept
//!
//! - The arrival test uses the goal's minor radius only for a box geometry (its major radius otherwise), against the 2-D distance.
//! - An airborne goal is waited for: a success is a continue while the goal is above the terrain and the owner is not.

use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_common::Relationship;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::state_machine::{SmResult, StateId, StateMachine, StateReturn};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_commands;
use crate::logic::ai::ai_face_exit_states::{WANTS_NEITHER, WANTS_TO_ENTER};
use crate::logic::ai::ai_move_states::{self as move_states, mv, MoveToData};
use crate::logic::ai::ai_states::{ai, AiState, AiStateHost};
use crate::logic::ai::ai_update_types::*;
use crate::logic::modules::plant;
use crate::logic::weapon::NO_MAX_SHOTS_LIMIT;

type Sm<H> = StateMachine<AiState<H>>;

fn entry_to_clear<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> &mut ObjectId {
    match sm.state_mut(id) {
        Some(AiState::Enter { entry_to_clear, .. }) => entry_to_clear,
        _ => unreachable!("the enter state"),
    }
}

/// `onEnter()`: the goal must be enterable; the move's goal its position, its contain told the owner wants to enter; the
/// pathfinder ignores it and the locomotor may leave the map; no destination adjustment.
pub fn enter_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    *entry_to_clear(sm, id) = INVALID_ID;
    let Some(goal) = sm.goal_object(h) else { return Ok(StateReturn::FAILURE) };
    let src = ai(h).last_command_source;
    if !h.action_can_enter_object(goal.id, src) {
        return Ok(StateReturn::FAILURE);
    }
    mv(sm, id).goal_position = goal.position;
    if h.object_has_contain(goal.id) {
        h.contain_wants_to_enter_or_exit(goal.id, WANTS_TO_ENTER);
        *entry_to_clear(sm, id) = goal.id;
    }
    // "tell the pathfinder to ignore the enterable object"
    let g = sm.goal_object_id();
    ai(h).ignore_obstacle(g);
    if let Some(cur) = ai(h).cur_locomotor {
        ai(h).locomotor_set.get_mut(cur).expect("the locomotor").set_allow_invalid_position(!plant(3360));
    }
    mv(sm, id).adjust_destinations = false;
    move_states::internal_on_enter(sm, h, id)
}

/// `onExit(status)`: the move's; the pathfinder no longer ignores the goal, the locomotor kept on the map; the remembered entry's
/// contain told the owner wants neither.
pub fn enter_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    move_states::internal_on_exit(sm, h, id);
    // "tell the pathfinder to stop ignoring the object"
    ai(h).ignore_obstacle(INVALID_ID);
    if let Some(cur) = ai(h).cur_locomotor {
        ai(h).locomotor_set.get_mut(cur).expect("the locomotor").set_allow_invalid_position(false);
    }
    let entry = *entry_to_clear(sm, id);
    if entry != INVALID_ID && !plant(3361) && h.object_exists(entry) && h.object_has_contain(entry) {
        h.contain_wants_to_enter_or_exit(entry, WANTS_NEITHER);
    }
}

/// `update()`: give up on a goal carried off in the air; follow the goal; an enemy no longer enterable is attacked if it may be;
/// held is entered; then the move, a success waiting for an airborne goal, and the arrival close to it added to its contain.
pub fn enter_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let Some(goal) = sm.goal_object(h) else { return Ok(StateReturn::FAILURE) };
    // "if our goal is contained by something else, give up."
    if h.object_contained_by(goal.id) != INVALID_ID && h.object_is_above_terrain(goal.id) && !h.is_above_terrain() {
        return Ok(StateReturn::FAILURE);
    }
    mv(sm, id).goal_position = goal.position;
    // (`ai->friend_setGoalObject`: the AI's machine's goal, also when this state runs inside a guard machine; batch A18)
    crate::logic::ai::ai_attack_states::ai_friend_set_goal_object(sm, h, goal.id);
    let src = ai(h).last_command_source;
    if !h.action_can_enter_object(goal.id, src) {
        // "special-case: if it's an enemy, try attacking it instead."
        if h.object_relationship(goal.id) == Relationship::Enemies {
            let result = h.action_can_attack_object(goal.id, src);
            if result == ATTACKRESULT_POSSIBLE || (result == ATTACKRESULT_POSSIBLE_AFTER_MOVING && !plant(3362)) {
                if h.attack_stack().is_empty() {
                    ai_commands::ai_attack_object(sm, h, goal.id, NO_MAX_SHOTS_LIMIT, src)?;
                } else {
                    // (inside a guard machine: the AI's machine is locked by the guard state's update; batch A18)
                    let view = h.attack_stack()[0].outer.clone();
                    ai_commands::ai_attack_object_on_locked_machine(h, &view, goal.id, NO_MAX_SHOTS_LIMIT, src)?;
                }
                // "weird but true. return state_continue, because if we're here, we're actually an attack state"
                return Ok(StateReturn::CONTINUE);
            }
            return Ok(StateReturn::FAILURE);
        }
        return Ok(StateReturn::FAILURE);
    }
    // "If we are held, then we must have entered the goal."
    if h.is_disabled_by_type(DISABLED_HELD) && !plant(3363) {
        return Ok(StateReturn::SUCCESS);
    }
    let mut code = move_states::internal_update(sm, h, id)?;
    // "if it's airborne, wait for it to land"
    if code == StateReturn::SUCCESS && h.object_is_above_terrain(goal.id) && !h.is_above_terrain() {
        code = StateReturn::CONTINUE;
    }
    if code == StateReturn::SUCCESS {
        // "we didn't enter. See if we're close."
        let p = h.position();
        let (dx, dy) = (p.x - goal.position.x, p.y - goal.position.y);
        let (minor, major, is_box) = h.object_geometry_radii(goal.id);
        let radius = if is_box != plant(3364) { minor } else { major };
        if dx * dx + dy * dy < radius * radius && h.object_has_contain(goal.id) {
            // "Grab the container and force ourselves into it."
            h.contain_add_to_contain(goal.id);
        }
    }
    Ok(code)
}

/// `AIEnterState::xfer` (version 1): the move's, the entry to clear.
pub fn enter_xfer(mv: &mut MoveToData, entry_to_clear: &mut ObjectId, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer.xfer_object_id(entry_to_clear)
}
