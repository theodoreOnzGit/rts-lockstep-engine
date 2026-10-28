// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIAttackMoveStateMachine, AIPickUpCrateState,
//     AIAttackMoveToState, AIAttackFollowWaypointPathState)
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
//! The AI's attack-move and crate states (`p1w13-sonnet`, layer 10, batch A11).
//!
//! `AIAttackMoveToState` (`AI_ATTACK_MOVE_TO`) moves as `AIMoveToState` does (without its mood's attack-move), and on the way
//! attacks the mood's targets and picks up crates through its own `AIAttackMoveStateMachine` (an idle state that does not look for
//! targets, the crate state, an attack state); a move that ends far from its goal is retried after three seconds, five times.
//! `AIPickUpCrateState` (`AI_PICK_UP_CRATE`, in both machines) waits three updates, then moves to the crate.
//! `AIAttackFollowWaypointPathState` (both ids, batch A12b) follows a waypoint path as `AIFollowWaypointPathState` does (without its
//! mood's attack-follow) with the same machine, attacking the mood's targets and picking up crates on the way.
//!
//! The attack-move machine is a [`StateMachine`] of [`AiState`] held by the state's variant; it is made, and its default state
//! entered, when the AI's machine is made (upstream: by the state's constructor). While it runs, the AI's machine's view is lent
//! as the bottom [`AttackFrame`], which the attack machine of its attack state reads (the AI's machine's mood and state).
//!
//! # Upstream facts kept
//!
//! - `m_commandSrc` is not saved; `AIPickUpCrateState::xfer` saves the goal position a second time (after the base's).
//! - `forceRepath` stores `-MIN_REPATH_TIME` in the unsigned path timestamp.

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::INVALID_ID;
use crate::common::model_state::MODELCONDITION_MOVING;
use crate::common::state_machine::{SmResult, StateContext, StateId, StateMachine, StateReturn};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_attack_states::{top_view, AttackFrame, AttackOwnerKinds};
use crate::logic::ai::ai_move_states::{self as move_states, mv, MoveToData, MIN_REPATH_TIME};
use crate::logic::ai::ai_waypoint_states as waypoint_states;
use crate::logic::ai::ai_states::{ai, with_ai, AiState, AiStateHost};
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `ATTACK_RETRY_COUNT`.
const ATTACK_RETRY_COUNT: i32 = 5;
/// `ATTACK_CLOSE_ENOUGH_CELLS`.
const ATTACK_CLOSE_ENOUGH_CELLS: i32 = 8;

/// `AIAttackMoveStateMachine::AIAttackMoveStateMachine(owner, name)`: "order matters: first state is the default state."
pub fn new_attack_move_machine<H: AiStateHost>(kinds: AttackOwnerKinds) -> Sm<H> {
    let mut m = StateMachine::new();
    let idle = AiState::Idle { initial_sleep_offset: u16::MAX, should_look_for_targets: false, inited: false };
    m.define_state(AI_IDLE, idle, AI_IDLE, AI_IDLE, &[]).expect("the attack-move machine");
    m.define_state(AI_PICK_UP_CRATE, pick_up_crate(), AI_IDLE, AI_IDLE, &[]).expect("the attack-move machine");
    let attack = crate::logic::ai::ai_attack_states::attack_state(false, true, false, kinds);
    m.define_state(AI_ATTACK_OBJECT, attack, AI_IDLE, AI_IDLE, &[]).expect("the attack-move machine");
    m
}

/// A new `AIPickUpCrateState` (its delay the pool's zero).
pub fn pick_up_crate<H: AiStateHost>() -> AiState<H> {
    AiState::PickUpCrate { mv: MoveToData::default(), delay_counter: 0 }
}

/// A new `AIAttackMoveToState`: not a move-to, no sleep, five retries, its machine made.
pub fn attack_move<H: AiStateHost>(kinds: AttackOwnerKinds) -> AiState<H> {
    AiState::AttackMove {
        mv: MoveToData::default(),
        frame_to_sleep_until: 0,
        retry_count: ATTACK_RETRY_COUNT,
        command_src: 0,
        machine: Some(new_attack_move_machine(kinds)),
    }
}

/// The sub-machine of a state that runs one (the attack-move, hunt and attack-area states).
fn sub_machine<H: AiStateHost>(state: Option<&mut AiState<H>>) -> &mut Option<Sm<H>> {
    match state {
        Some(AiState::AttackMove { machine, .. }) | Some(AiState::Hunt { machine, .. }) => machine,
        Some(AiState::AttackSquad { machine, .. }) => machine,
        Some(AiState::Dock { machine, .. }) => machine,
        Some(AiState::Guard { machine, .. }) => machine,
        Some(AiState::FollowWaypointPath { machine, .. }) => machine,
        _ => unreachable!("a state with a sub-machine"),
    }
}

/// Runs `f` on the sub-machine of the state `id` (taken out of it), with the AI's machine's view lent as an attack frame; a goal
/// object set on the AI's machine from inside is applied after.
pub fn run_sub_machine<H: AiStateHost, R>(sm: &mut Sm<H>, h: &mut H, id: StateId, f: impl FnOnce(&mut Sm<H>, &mut H) -> SmResult<R>) -> SmResult<R> {
    // (the view before the machine is taken: the state's isAttack is its machine's)
    let outer = top_view(sm, h);
    let mut inner = sub_machine(sm.state_mut(id)).take().expect("the sub-machine");
    let bottom = h.attack_stack().is_empty();
    // (batch A17: the docking machine's approach position, lent)
    let dock_approach = match sm.state(id) {
        Some(AiState::Dock { approach_position, .. }) => *approach_position,
        _ => -1,
    };
    // (batch A18: the guard machine's members, lent)
    let guard = match sm.state(id) {
        Some(AiState::Guard { data, .. }) => *data,
        _ => Default::default(),
    };
    h.attack_stack().push(AttackFrame {
        outer,
        original_victim_pos: Default::default(),
        attacking_object: false,
        new_victim: None,
        ai_goal_object: Vec::new(),
        dock_approach,
        guard,
    });
    let r = f(&mut inner, h);
    let lent = h.attack_stack().pop().expect("the view frame");
    *sub_machine(sm.state_mut(id)) = Some(inner);
    if let Some(AiState::Dock { approach_position, .. }) = sm.state_mut(id) {
        *approach_position = lent.dock_approach;
    }
    if let Some(AiState::Guard { data, .. }) = sm.state_mut(id) {
        *data = lent.guard;
    }
    // the AI's machine's goal, set from inside
    for obj in lent.ai_goal_object {
        if bottom {
            crate::logic::ai::ai_paths::friend_set_goal_object(sm, h, obj);
        } else {
            crate::logic::ai::ai_attack_states::ai_friend_set_goal_object(sm, h, obj);
        }
    }
    r
}

/// The AI's machine made: every attack-move machine enters its default state (upstream: in the state's constructor, during the
/// AI's machine's).
pub fn init_attack_move_machines<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    // (in the order the states are defined; batch A12b: the attack-follow-waypoint states' too)
    let ids = if plant(3346) {
        [AI_ATTACK_MOVE_TO, AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS, AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM]
    } else {
        [AI_ATTACK_MOVE_TO, AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM, AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS]
    };
    for id in ids {
        let has = match sm.state(id) {
            Some(AiState::AttackMove { .. }) => true,
            Some(AiState::FollowWaypointPath { machine, .. }) => machine.is_some(),
            _ => false,
        };
        if has {
            run_sub_machine(sm, h, id, |m, h| m.init_default_state(h).map(|_| ()))?;
        }
    }
    Ok(())
}

/// Whether the attack-move state's machine is in its attack state (`isAttack()`).
pub fn is_attack<H: AiStateHost>(machine: &Option<Sm<H>>) -> bool {
    machine.as_ref().is_some_and(|m| m.is_in_attack_state())
}

// ---- AIPickUpCrateState

/// `AIPickUpCrateState::onEnter()`: to the goal object's position, after three updates.
pub fn crate_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let Some(goal) = sm.goal_object(h) else { return Ok(StateReturn::FAILURE) };
    let m = mv(sm, id);
    m.adjust_destinations = true;
    m.goal_position = goal.position;
    if let Some(AiState::PickUpCrate { delay_counter, .. } | AiState::GuardCrate { delay_counter, .. }) = sm.state_mut(id) {
        *delay_counter = if plant(3320) { 2 } else { 3 };
    }
    Ok(StateReturn::CONTINUE)
}

/// `AIPickUpCrateState::update()`: the delay ("find a way to sleep for a number of frames here"), then the move.
pub fn crate_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let delay = match sm.state_mut(id) {
        Some(AiState::PickUpCrate { delay_counter, .. } | AiState::GuardCrate { delay_counter, .. }) => delay_counter,
        _ => unreachable!("the crate state"),
    };
    if *delay != 0 {
        *delay -= 1;
        if *delay == 0 {
            return move_states::internal_on_enter(sm, h, id);
        }
        return Ok(StateReturn::CONTINUE);
    }
    // "do movement"
    move_states::internal_update(sm, h, id)
}

/// `AIPickUpCrateState::xfer` (version 1): the base's, the delay, the goal position again.
pub fn crate_xfer(mv: &mut MoveToData, delay_counter: &mut i32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer.xfer_int(delay_counter)?;
    xfer.xfer_coord3d(&mut mv.goal_position)?;
    Ok(())
}

// ---- AIAttackMoveToState

/// `AIAttackMoveToState::onEnter()`: the machine cleared to idle, the command source kept, the retries renewed; then the move.
pub fn attack_move_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    run_sub_machine(sm, h, id, |m, h| {
        if !plant(3325) {
            m.clear(h)?;
        }
        m.set_state(h, AI_IDLE).map(|_| ())
    })?;
    let src = ai(h).last_command_source;
    if let Some(AiState::AttackMove { command_src, retry_count, frame_to_sleep_until, .. }) = sm.state_mut(id) {
        *command_src = src;
        *retry_count = ATTACK_RETRY_COUNT;
        *frame_to_sleep_until = 0;
    }
    move_states::move_to_on_enter(sm, h, id)
}

/// `AIAttackMoveToState::onExit(status)`.
pub fn attack_move_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<()> {
    run_sub_machine(sm, h, id, |m, h| m.set_state(h, AI_IDLE).map(|_| ()))?;
    move_states::internal_on_exit(sm, h, id);
    Ok(())
}

fn inner_idle<H: AiStateHost>(sm: &Sm<H>, id: StateId) -> bool {
    match sm.state(id) {
        Some(AiState::AttackMove { machine: Some(m), .. }) | Some(AiState::FollowWaypointPath { machine: Some(m), .. }) => m.is_in_idle_state(),
        _ => unreachable!("the attack-move machine"),
    }
}

/// `AIAttackMoveToState::update()`: the machine's attack or crate while it is busy; else the next mood target or crate; else the
/// move (after the sleep), retried while it ends far from its goal.
pub fn attack_move_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let mut force_retarget = false;
    let mut should_repath = false;
    // (getJetAIUpdate(): none for AIUpdateInterface)
    if !inner_idle(sm, id) {
        ai(h).set_locomotor_goal_none();
        h.clear_model_condition_state(MODELCONDITION_MOVING);
        run_sub_machine(sm, h, id, |m, h| m.update_state_machine(h).map(|_| ()))?;
        // "if the machine is now idling, then we need to attempt to get a new target"
        if inner_idle(sm, id) {
            force_retarget = true;
            should_repath = true;
            let src = match sm.state(id) {
                Some(AiState::AttackMove { command_src, .. }) => *command_src,
                _ => unreachable!("the attack-move state"),
            };
            ai(h).set_last_command_source(src);
        } else {
            return Ok(StateReturn::CONTINUE);
        }
    }
    if inner_idle(sm, id) {
        // "Check to see if we have created a crate we need to pick up."
        let crate_id = with_ai(h, |a, h| a.check_for_crate_to_pickup(h));
        if crate_id != INVALID_ID {
            let o = h.find_object(crate_id);
            run_sub_machine(sm, h, id, |m, h| {
                m.set_goal_object(o.as_ref());
                m.set_state(h, AI_PICK_UP_CRATE).map(|_| ())
            })?;
            return Ok(StateReturn::CONTINUE);
        }
        let view = top_view(sm, h);
        let next = with_ai(h, |a, h| a.get_next_mood_target(h, &view, !force_retarget, false));
        if next != INVALID_ID {
            ai(h).friend_ending_move();
            let o = h.find_object(next);
            run_sub_machine(sm, h, id, |m, h| {
                m.set_goal_object(o.as_ref());
                m.set_state(h, AI_ATTACK_OBJECT).map(|_| ())
            })?;
            // "Note that we picked up this command from the ai."
            ai(h).set_last_command_source(if plant(3321) { CMD_FROM_PLAYER } else { CMD_FROM_AI });
            // "we don't want an update to take place 'till next frame."
            return Ok(StateReturn::CONTINUE);
        }
    }
    let now = StateContext::frame(h);
    let sleep_until = match sm.state(id) {
        Some(AiState::AttackMove { frame_to_sleep_until, .. }) => *frame_to_sleep_until,
        _ => unreachable!("the attack-move state"),
    };
    if sleep_until > now {
        return Ok(StateReturn::CONTINUE);
    } else if sleep_until == now {
        should_repath = true;
    }
    if should_repath && !plant(3322) {
        move_states::move_to_on_enter(sm, h, id)?;
        // forceRepath()
        let m = mv(sm, id);
        m.path_goal_position = crate::common::base_type::Coord3D { x: -100.0, y: -100.0, z: -100.0 };
        m.path_timestamp = MIN_REPATH_TIME.wrapping_neg();
    }
    let mut ret = move_states::move_to_update(sm, h, id, false)?;
    if ret != StateReturn::CONTINUE {
        let retry = match sm.state(id) {
            Some(AiState::AttackMove { retry_count, .. }) => *retry_count,
            _ => unreachable!("the attack-move state"),
        };
        if retry < 1 {
            return Ok(ret);
        }
        // "check for close enough."
        let (p, goal) = (h.position(), if plant(3326) { mv(sm, id).goal_position } else { mv(sm, id).path_goal_position });
        let dist_sqr = (p.x - goal.x) * (p.x - goal.x) + (p.y - goal.y) * (p.y - goal.y);
        let close = ATTACK_CLOSE_ENOUGH_CELLS as f32 * PATHFIND_CELL_SIZE_F;
        if dist_sqr < close * close {
            return Ok(ret);
        }
        ret = StateReturn::CONTINUE;
        if let Some(AiState::AttackMove { retry_count, frame_to_sleep_until, .. }) = sm.state_mut(id) {
            if !plant(3324) {
                *retry_count -= 1;
            }
            // "Sleep 3 seconds. We can attack during these frames, just not move."
            let seconds: u32 = if plant(3323) { 2 } else { 3 };
            *frame_to_sleep_until = now.wrapping_add(seconds * LOGICFRAMES_PER_SECOND as u32);
        }
    }
    Ok(ret)
}

/// `AIAttackMoveToState::xfer` (version 2): `AIMoveToState`'s (the base's), the sleep, the retries, the machine.
pub fn attack_move_xfer<H: AiStateHost>(
    mv: &mut MoveToData,
    frame_to_sleep_until: &mut u32,
    retry_count: &mut i32,
    machine: &mut Option<Sm<H>>,
    xfer: &mut dyn Xfer,
) -> XferResult<()> {
    let current_version: XferVersion = 2;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    if version >= 2 {
        xfer.xfer_unsigned_int(frame_to_sleep_until)?;
        xfer.xfer_int(retry_count)?;
    }
    machine_xfer(machine, xfer)
}

/// `AIAttackMoveStateMachine::xfer`: its version, then the machine's.
fn machine_xfer<H: AiStateHost>(machine: &mut Option<Sm<H>>, xfer: &mut dyn Xfer) -> XferResult<()> {
    let m = machine.as_mut().expect("the attack-move machine");
    let cv: XferVersion = 1;
    let mut v = cv;
    xfer.xfer_version(&mut v, cv)?;
    Snapshot::xfer(m, xfer)
}

// ---- AIAttackFollowWaypointPathState

/// A new `AIAttackFollowWaypointPathState(machine, asGroup)`: `AIFollowWaypointPathState(machine, asGroup, false)` with its
/// attack-move machine made (its default state entered by [`init_attack_move_machines`]).
pub fn attack_follow<H: AiStateHost>(as_group: bool, kinds: AttackOwnerKinds) -> AiState<H> {
    AiState::FollowWaypointPath {
        mv: MoveToData::default(),
        wp: crate::logic::ai::ai_waypoint_states::WaypointPathData::default(),
        as_group,
        is_follow: false,
        machine: Some(new_attack_move_machine(kinds)),
    }
}

/// `AIAttackFollowWaypointPathState::onEnter()`: the machine cleared to idle, then the waypoint path's.
pub fn attack_follow_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    run_sub_machine(sm, h, id, |m, h| {
        if !plant(3340) {
            m.clear(h)?;
        }
        m.set_state(h, AI_IDLE).map(|_| ())
    })?;
    waypoint_states::on_enter(sm, h, id)
}

/// `AIAttackFollowWaypointPathState::update()`: the machine's attack or crate while it is busy; else the next mood target or crate;
/// else the waypoint path's (the goal and path recomputed after a retargeting: "Update the goal waypoint if we've moved along the
/// path.").
pub fn attack_follow_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let mut force_retarget = false;
    let mut should_repath = false;
    if !inner_idle(sm, id) {
        ai(h).set_locomotor_goal_none();
        h.clear_model_condition_state(MODELCONDITION_MOVING);
        run_sub_machine(sm, h, id, |m, h| m.update_state_machine(h).map(|_| ()))?;
        // "if the machine is now idling, then we need to attempt to get a new target"
        if inner_idle(sm, id) {
            force_retarget = !plant(3341);
            should_repath = true;
        } else {
            return Ok(StateReturn::CONTINUE);
        }
    }
    if inner_idle(sm, id) {
        // "Check to see if we have created a crate we need to pick up."
        let crate_id = with_ai(h, |a, h| a.check_for_crate_to_pickup(h));
        if crate_id != INVALID_ID {
            let o = h.find_object(crate_id);
            run_sub_machine(sm, h, id, |m, h| {
                m.set_goal_object(o.as_ref());
                m.set_state(h, AI_PICK_UP_CRATE).map(|_| ())
            })?;
            return Ok(StateReturn::CONTINUE);
        }
        let view = top_view(sm, h);
        let next = with_ai(h, |a, h| a.get_next_mood_target(h, &view, !force_retarget, false));
        if next != INVALID_ID {
            let o = h.find_object(next);
            run_sub_machine(sm, h, id, |m, h| {
                m.set_goal_object(o.as_ref());
                m.set_state(h, AI_ATTACK_OBJECT).map(|_| ())
            })?;
            // "we don't want an update to take place 'till next frame."
            if !plant(3344) {
                return Ok(StateReturn::CONTINUE);
            }
        }
    }
    if should_repath && !plant(3342) {
        // "Update the goal waypoint if we've moved along the path."
        waypoint_states::compute_goal(sm, h, id);
        move_states::compute_path(sm, h, id);
    }
    waypoint_states::update(sm, h, id)
}

/// `AIAttackFollowWaypointPathState::onExit(status)`: the machine to idle, then the waypoint path's.
pub fn attack_follow_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<()> {
    if !plant(3343) {
        run_sub_machine(sm, h, id, |m, h| m.set_state(h, AI_IDLE).map(|_| ()))?;
    }
    waypoint_states::on_exit(sm, h, id);
    Ok(())
}

/// `AIAttackFollowWaypointPathState::xfer` (version 1): the waypoint path's, then the machine.
pub fn attack_follow_xfer<H: AiStateHost>(
    mv: &mut MoveToData,
    wp: &mut crate::logic::ai::ai_waypoint_states::WaypointPathData,
    machine: &mut Option<Sm<H>>,
    xfer: &mut dyn Xfer,
) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    waypoint_states::waypoint_path_xfer(mv, wp, xfer)?;
    machine_xfer(machine, xfer)
}
