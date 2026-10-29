// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (isSamePosition; AIInternalMoveToState, AIMoveToState,
//     AIMoveOutOfTheWayState, AIMoveAndTightenState, AIMoveAwayFromRepulsorsState; batch A8: AIFollowPathState,
//     AIMoveAndEvacuateState, AIMoveAndDeleteState)
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
//! The AI's move states (`p1w13-sonnet`, layer 10, batch A7): `AIInternalMoveToState` (the base of every moving state: the path
//! request, the model conditions on the way, the repath when the goal moves or the unit is stuck, the arrival) and the four
//! states that are it with a goal: `AIMoveToState` (the goal object or position, a missile's lead), `AIMoveOutOfTheWayState`
//! (the existing path), `AIMoveAndTightenState` (an approach path, one repath when stuck) and `AIMoveAwayFromRepulsorsState` (a
//! safe path, panicking). Batch A8 adds three more: `AIFollowPathState` (the machine's goal path point by point, also the
//! exit-production path), `AIMoveAndEvacuateState` (locked; evacuates at the goal) and `AIMoveAndDeleteState` (locked; may
//! leave the map; destroys the object at the goal).
//!
//! Each state's data is [`MoveToData`] (the base class's members) in its [`AiState`] variant; the base's virtual `computePath`
//! is [`compute_path`], which dispatches on the variant as upstream's overrides do.
//!
//! # Upstream facts kept
//!
//! - `m_waitingForPath`, `m_tryOneMoreRepath` and `m_ambientPlayingHandle` are not set by the constructor (0 from the pool).
//! - `TheGameLogic->getFrame() - m_pathTimestamp > MIN_REPATH_TIME` is unsigned.
//! - `AIMoveToState::update`'s mood adjustment gives an attack-move command and then still runs this state's update.
//! - `loadPostProcess` restarts the move sound, but nothing calls a state's `loadPostProcess` (the machine's does nothing).

use crate::common::base_type::{real_to_int, Coord3D};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_INVALID};
use crate::common::kind_of::{KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_IMMOBILE, KINDOF_PROJECTILE};
use crate::common::model_state::*;
use crate::common::object_status_types::{OBJECT_STATUS_IMMOBILE, OBJECT_STATUS_PARACHUTING, OBJECT_STATUS_RIDER8};
use crate::common::state_machine::{SmResult, StateContext, StateExitType, StateId, StateMachine, StateReturn};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_commands;
use crate::logic::ai::ai_paths;
use crate::logic::ai::ai_states::{ai, with_ai, AiState, AiStateHost};
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::modules::plant;
use crate::logic::module::body_module::BODY_DAMAGED;
use crate::logic::weapon::NO_MAX_SHOTS_LIMIT;

type Sm<H> = StateMachine<AiState<H>>;

/// `AHSV_Error` (AudioHandleSpecialValues.h).
pub const AHSV_ERROR: u32 = 0;
/// `MIN_REPATH_TIME`: "minimum # of frames must elapse before re-pathing".
pub const MIN_REPATH_TIME: u32 = 10;
/// `PathfindCell::CELL_CLIFF`.
const CELL_CLIFF: u8 = 2;

/// `AIInternalMoveToState`'s data members.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveToData {
    /// `m_goalPosition`.
    pub goal_position: Coord3D,
    /// `m_goalLayer`.
    pub goal_layer: PathfindLayer,
    /// `m_pathGoalPosition`: "the position our current path leads to".
    pub path_goal_position: Coord3D,
    /// `m_ambientPlayingHandle`.
    pub ambient_playing_handle: u32,
    /// `m_pathTimestamp`.
    pub path_timestamp: u32,
    /// `m_blockedRepathTimestamp`.
    pub blocked_repath_timestamp: u32,
    /// `m_adjustDestinations`.
    pub adjust_destinations: bool,
    /// `m_waitingForPath`.
    pub waiting_for_path: bool,
    /// `m_tryOneMoreRepath`.
    pub try_one_more_repath: bool,
}

impl Default for MoveToData {
    /// The constructor's (and the pool's zeros for what it does not set).
    fn default() -> Self {
        MoveToData {
            goal_position: Coord3D::default(),
            goal_layer: LAYER_INVALID,
            path_goal_position: Coord3D::default(),
            ambient_playing_handle: 0,
            path_timestamp: 0,
            blocked_repath_timestamp: 0,
            adjust_destinations: true,
            waiting_for_path: false,
            try_one_more_repath: false,
        }
    }
}

impl MoveToData {
    /// `AIInternalMoveToState::xfer` (version 1).
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        xfer.xfer_coord3d(&mut self.goal_position)?;
        // xferUser(&m_goalLayer, sizeof(m_goalLayer)): the enum's 4 bytes
        xfer.xfer_int(&mut self.goal_layer)?;
        xfer.xfer_bool(&mut self.waiting_for_path)?;
        xfer.xfer_coord3d(&mut self.path_goal_position)?;
        xfer.xfer_unsigned_int(&mut self.path_timestamp)?;
        xfer.xfer_unsigned_int(&mut self.blocked_repath_timestamp)?;
        xfer.xfer_bool(&mut self.adjust_destinations)?;
        Ok(())
    }
}

/// `AIMoveAndTightenState::xfer` (version 1; also `AIMoveAwayFromRepulsorsState`'s): the base's, then its own.
pub fn tighten_xfer(mv: &mut MoveToData, ok_to_repath_times: &mut i32, check_for_path: &mut bool, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer.xfer_int(ok_to_repath_times)?;
    xfer.xfer_bool(check_for_path)?;
    Ok(())
}

/// `AIFollowPathState::xfer` (version 1): the base's, the index and the two adjust flags (not the retry count).
pub fn follow_path_xfer(mv: &mut MoveToData, index: &mut i32, adjust_final: &mut bool, adjust_final_override: &mut bool, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer.xfer_int(index)?;
    xfer.xfer_bool(adjust_final)?;
    xfer.xfer_bool(adjust_final_override)?;
    Ok(())
}

/// `AIMoveAndEvacuateState::xfer` (version 1): the base's and the origin.
pub fn evacuate_xfer(mv: &mut MoveToData, origin: &mut Coord3D, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer.xfer_coord3d(origin)?;
    Ok(())
}

/// `AIMoveAndDeleteState::xfer` (version 1): the base's and the append flag.
pub fn delete_xfer(mv: &mut MoveToData, append_goal_position: &mut bool, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer.xfer_bool(append_goal_position)?;
    Ok(())
}

/// The move data of the state `id`.
pub fn mv<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> &mut MoveToData {
    match sm.state_mut(id) {
        Some(AiState::MoveTo { mv, .. })
        | Some(AiState::MoveOutOfTheWay { mv })
        | Some(AiState::MoveAndTighten { mv, .. })
        | Some(AiState::MoveAwayFromRepulsors { mv, .. })
        | Some(AiState::FollowPath { mv, .. })
        | Some(AiState::MoveAndEvacuate { mv, .. })
        | Some(AiState::MoveAndDelete { mv, .. })
        | Some(AiState::FollowWaypointPath { mv, .. })
        | Some(AiState::FollowWaypointPathExact { mv, .. })
        | Some(AiState::Wander { mv, .. })
        | Some(AiState::WanderInPlace { mv, .. })
        | Some(AiState::Panic { mv, .. })
        | Some(AiState::AttackApproach { mv, .. })
        | Some(AiState::AttackPursue { mv, .. })
        | Some(AiState::AttackMove { mv, .. })
        | Some(AiState::PickUpCrate { mv, .. })
        | Some(AiState::Enter { mv, .. })
        | Some(AiState::DockMove { mv, .. })
        | Some(AiState::GuardReturn { mv, .. })
        | Some(AiState::TnGuardReturn { mv, .. })
        | Some(AiState::GuardCrate { mv, .. }) => mv,
        _ => unreachable!("a move state"),
    }
}

fn tighten<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> (&mut MoveToData, &mut i32, &mut bool) {
    match sm.state_mut(id) {
        Some(AiState::MoveAndTighten { mv, ok_to_repath_times, check_for_path })
        | Some(AiState::MoveAwayFromRepulsors { mv, ok_to_repath_times, check_for_path }) => (mv, ok_to_repath_times, check_for_path),
        _ => unreachable!("a tighten state"),
    }
}

fn frame<H: AiStateHost>(h: &H) -> u32 {
    StateContext::frame(h)
}

/// `isSamePosition(ourPos, prevTargetPos, curTargetPos)`: the target moved less than a tenth of its 2-D distance.
pub fn is_same_position(our_pos: &Coord3D, prev_target_pos: &Coord3D, cur_target_pos: &Coord3D) -> bool {
    // "for pathfinding purposes, only care about 2d pos."
    let dx = cur_target_pos.x - prev_target_pos.x;
    let dy = cur_target_pos.y - prev_target_pos.y;
    let tx = cur_target_pos.x - our_pos.x;
    let ty = cur_target_pos.y - our_pos.y;
    // "Tolerance is (dist/10)squared."
    const TOLERANCE_FACTOR: f32 = 1.0 / (10.0 * 10.0);
    let tolerance_sqr = (tx * tx + ty * ty) * TOLERANCE_FACTOR;
    // (upstream returns false only when greater: a NaN distance is the same position)
    !matches!((dx * dx + dy * dy).partial_cmp(&tolerance_sqr), Some(std::cmp::Ordering::Greater))
}

/// `getAdjustsDestination()`: not while parachuting, not if the AI forbids it (the base class allows it).
pub(crate) fn get_adjusts_destination<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> bool {
    if h.status_test(OBJECT_STATUS_PARACHUTING) {
        return false;
    }
    // (isAllowedToAdjustDestination: AIUpdateInterface's TRUE)
    mv(sm, id).adjust_destinations
}

/// The virtual `computePath()`: the base's (request a path to the goal), or the subclass's.
pub(crate) fn compute_path<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> bool {
    match sm.state(id) {
        Some(AiState::AttackApproach { .. }) => crate::logic::ai::ai_attack_states::approach_compute_path(sm, h, id),
        Some(AiState::AttackPursue { .. }) => crate::logic::ai::ai_attack_states::pursue_compute_path(sm, h, id),
        Some(AiState::MoveOutOfTheWay { .. }) => {
            mv(sm, id).waiting_for_path = true;
            if ai(h).is_blocked_and_stuck {
                ai(h).set_can_path_through_units(true);
                // "don't repath, just stop."
                return true;
            }
            // "just use the existing path."
            true
        }
        Some(AiState::MoveAndTighten { .. }) => {
            if ai(h).is_blocked_and_stuck {
                let (mv, ok, _) = tighten(sm, id);
                if *ok > 0 {
                    *ok -= 1;
                    mv.waiting_for_path = true;
                    let goal = mv.goal_position;
                    ai_paths::request_path(h, &goal, true);
                    return true;
                }
                // "don't repath for now."
                return false;
            }
            true
        }
        Some(AiState::MoveAwayFromRepulsors { .. }) => {
            let (_, ok, _) = tighten(sm, id);
            if *ok > 0 && !plant(3270) {
                *ok -= 1;
                return true;
            }
            // "don't recompute path, just stop moving."
            false
        }
        _ => {
            let adjusts = get_adjusts_destination(sm, h, id);
            let m = mv(sm, id);
            m.waiting_for_path = true;
            let goal = m.goal_position;
            ai_paths::request_path(h, &goal, adjusts);
            ai(h).friend_starting_move();
            true
        }
    }
}

/// `startMoveSound()`: the move start sound (damaged or not), else the loop, whose handle is kept.
fn start_move_sound<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let damaged = h.damage_state() > BODY_DAMAGED;
    if let Some(handle) = h.start_move_sound(damaged) {
        mv(sm, id).ambient_playing_handle = handle;
    }
}

/// The cell under the object (`TheAI->pathfinder()->getCell(layer, REAL_TO_INT(x / PATHFIND_CELL_SIZE), ...)`).
fn cell_under<H: AiStateHost>(h: &mut H) -> Option<(u8, bool)> {
    let pos = h.position();
    let cx = real_to_int(pos.x / PATHFIND_CELL_SIZE_F);
    let cy = real_to_int(pos.y / PATHFIND_CELL_SIZE_F);
    let layer = h.layer();
    h.pf_cell_at(layer, cx, cy)
}

/// `AIInternalMoveToState::onEnter()`: "We are initiating a moveTo action. Pathfind from m_goalPosition to goal."
pub(crate) fn internal_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    mv(sm, id).ambient_playing_handle = AHSV_ERROR;
    let waiting = ai(h).waiting_for_path;
    mv(sm, id).waiting_for_path = waiting;
    if h.status_test(OBJECT_STATUS_IMMOBILE) {
        return Ok(StateReturn::FAILURE);
    }
    let now = frame(h);
    let a = ai(h);
    if let Some(cur) = a.cur_locomotor {
        let loco = a.locomotor_set.get_mut(cur).expect("the locomotor");
        loco.start_move(now);
        if loco.is_ultra_accurate() {
            // "if we're being ultra accurate, we can't adjust the destination."
            mv(sm, id).adjust_destinations = false;
        }
    }
    // "We may try one more repath after the first one is finished."
    mv(sm, id).try_one_more_repath = true;
    ai(h).friend_starting_move();
    if h.has_physics() && h.is_motive() {
        // "If we were already moving, go immediately to the animation."
        let cell = cell_under(h);
        let flag = if cell.is_some_and(|(t, _)| t != CELL_CLIFF) { MODELCONDITION_MOVING } else { MODELCONDITION_CLIMBING };
        h.set_model_condition_state(flag);
    } else if h.is_kind_of(KINDOF_DOZER) && h.is_kind_of(KINDOF_HARVESTER) {
        // "This is a classic January 2 type of hack"
        h.set_model_condition_state(MODELCONDITION_MOVING);
    }
    if get_adjusts_destination(sm, h, id) && !h.status_test(OBJECT_STATUS_RIDER8) {
        let mut goal = mv(sm, id).goal_position;
        if !h.pf_adjust_destination(&mut goal) {
            h.pf_snap_closest_goal_position(&mut goal);
        }
        mv(sm, id).goal_position = goal;
        let layer = h.layer_for_destination(&goal);
        h.pf_update_goal(&goal, layer);
    }
    // "request a path to the destination"
    if !compute_path(sm, h, id) {
        ai(h).friend_ending_move();
        return Ok(StateReturn::FAILURE);
    }
    // "Target to stop at the end of this path. This value will be overriden by the FollowWaypoint ai state."
    let a = ai(h);
    a.set_path_extra_distance(0.0);
    a.desired_speed = FAST_AS_POSSIBLE;
    start_move_sound(sm, h, id);
    Ok(StateReturn::CONTINUE)
}

/// `AIInternalMoveToState::onExit(status)`: the move sound stopped, the move ended; accurate ground movement ends exactly at
/// the goal when within a cell.
pub(crate) fn internal_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    // "stop ambient sound associated with movement"
    let handle = mv(sm, id).ambient_playing_handle;
    h.remove_audio_event(handle);
    ai(h).friend_ending_move();
    let ground = with_ai(h, |a, h| a.is_doing_ground_movement(h));
    if ground && ai(h).get_cur_locomotor().is_some_and(|l| l.is_ultra_accurate()) {
        let goal = mv(sm, id).goal_position;
        let pos = h.position();
        let (dx, dy) = (goal.x - pos.x, goal.y - pos.y);
        if dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
            // "We are doing accurate ground movement, so make sure we end exactly at the goal."
            ai(h).set_final_position(&goal);
        }
    }
}

/// `AIInternalMoveToState::update()`: "Execute the moveTo behavior towards GoalPosition."
pub(crate) fn internal_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let have_path = ai(h).path.is_some();
    if mv(sm, id).waiting_for_path {
        // "bump the timer."
        let now = frame(h);
        mv(sm, id).path_timestamp = now;
        if ai(h).waiting_for_path {
            return Ok(StateReturn::CONTINUE);
        }
        if !have_path {
            return Ok(StateReturn::FAILURE);
        }
        let m = mv(sm, id);
        m.waiting_for_path = false;
        m.path_goal_position = m.goal_position;
        if get_adjusts_destination(sm, h, id) {
            let path = ai(h).path.as_ref().expect("the path");
            let last = path.node(path.last_node().expect("a node"));
            let (pos, layer) = (*last.position(), last.layer());
            h.pf_update_goal(&pos, layer);
        } else {
            h.pf_remove_goal();
        }
        if !ai(h).retry_path {
            mv(sm, id).try_one_more_repath = false;
        }
    }

    let mut force_recompute = !have_path;
    if ai(h).is_blocked_and_stuck || ai(h).blocked_frames > 2 * LOGICFRAMES_PER_SECOND_I {
        force_recompute = true;
        let now = frame(h);
        mv(sm, id).blocked_repath_timestamp = now;
    }

    // "Determine if we are on a cliff cell... if so, use the climbing model condition instead of moving."
    let cell = cell_under(h);
    let mut set_condition_flag = MODELCONDITION_MOVING;
    if let Some((t, pinched)) = cell {
        if t == CELL_CLIFF && !pinched {
            if ai(h).get_cur_locomotor().is_some_and(|l| l.is_moving_backwards()) {
                set_condition_flag = MODELCONDITION_RAPPELLING;
                h.clear_model_condition_state(MODELCONDITION_CLIMBING);
            } else {
                set_condition_flag = MODELCONDITION_CLIMBING;
                h.clear_model_condition_state(MODELCONDITION_RAPPELLING);
            }
        }
    }
    if ai(h).blocked_frames > LOGICFRAMES_PER_SECOND_I / 4 {
        h.clear_model_condition_state(MODELCONDITION_MOVING);
    } else {
        // "Clear climbing if modelConditionFlag is not climbing"
        if set_condition_flag == MODELCONDITION_MOVING {
            h.clear_model_condition_state(MODELCONDITION_CLIMBING);
            h.clear_model_condition_state(MODELCONDITION_RAPPELLING);
        }
        h.set_model_condition_state(MODELCONDITION_MOVING);
        if set_condition_flag != MODELCONDITION_MOVING {
            h.set_model_condition_state(set_condition_flag);
        }
    }

    if ai_paths::can_compute_quick_path(h) && h.is_kind_of(KINDOF_PROJECTILE) {
        // "We can compute paths cheap, so don't delay to recompute."
        mv(sm, id).path_timestamp = 0;
        force_recompute = true;
    }

    if ai(h).path.is_some() {
        ai(h).set_locomotor_goal_position_on_path();
    }

    // "if our goal has moved, recompute our path"
    let now = frame(h);
    if force_recompute || now.wrapping_sub(mv(sm, id).path_timestamp) > MIN_REPATH_TIME {
        let pos = h.position();
        let (pg, g) = (mv(sm, id).path_goal_position, mv(sm, id).goal_position);
        if force_recompute || !is_same_position(&pos, &pg, &g) {
            // "goal moved - repath"
            if !compute_path(sm, h, id) {
                return Ok(StateReturn::FAILURE);
            }
            // "must re-set setLocoGoal after computePath, since computePath can set the loco goal to NONE..."
            if ai(h).path.is_some() {
                ai(h).set_locomotor_goal_position_on_path();
            } else {
                return Ok(StateReturn::CONTINUE);
            }
        }
    }

    // "Check if we have reached our destination"
    let on_path_dist_to_goal = with_ai(h, |a, h| a.get_locomotor_distance_to_goal(h, &*sm));
    let close_enough = ai(h).get_cur_locomotor().map(|l| l.get_close_enough_dist());
    if let Some(close_enough) = close_enough {
        if on_path_dist_to_goal < close_enough {
            if with_ai(h, |a, h| a.is_doing_ground_movement(h)) {
                // "sanity check"
                let mut goal_pos = mv(sm, id).goal_position;
                if let Some(path) = &ai(h).path {
                    if let Some(last) = path.last_node() {
                        goal_pos = *path.node(last).position();
                    }
                }
                let pos = h.position();
                let (dx, dy) = (pos.x - goal_pos.x, pos.y - goal_pos.y);
                if (dx * dx + dy * dy).sqrt() > 4.0 * PATHFIND_CELL_SIZE_F && !plant(3271) {
                    // "Trying to finish early. Continuing..." (the distance is asked again)
                    with_ai(h, |a, h| a.get_locomotor_distance_to_goal(h, &*sm));
                    return Ok(StateReturn::CONTINUE);
                }
            }
            // "we have reached the end of the path"
            if get_adjusts_destination(sm, h, id) {
                ai(h).set_locomotor_goal_none();
            }
            return Ok(StateReturn::SUCCESS);
        }
    }
    Ok(StateReturn::CONTINUE)
}

/// `LOGICFRAMES_PER_SECOND` as the `Int`s the blocked frames are compared with.
const LOGICFRAMES_PER_SECOND_I: i32 = crate::common::game_common::LOGICFRAMES_PER_SECOND;

/// A missile's goal height: half the goal object's height above its position, twice if the goal is above it.
fn half_height<H: AiStateHost>(h: &mut H, goal: ObjectId) -> f32 {
    h.object_max_height_above_position(goal) / 2.0
}

/// `AIMoveToState::onEnter()` (the attack-move state's base, batch A11).
pub(crate) fn move_to_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    mv(sm, id).adjust_destinations = true;
    // "If we have a goal object and are trying to ignore it as an obstacle..."
    let goal_obj = sm.goal_object(h).map(|o| o.id).unwrap_or(INVALID_ID);
    if goal_obj != INVALID_ID && goal_obj == ai(h).get_ignored_obstacle_id() && !plant(3272) {
        mv(sm, id).adjust_destinations = false;
    }
    // "if we have a goal object, move to it, otherwise move to goal position"
    if let Some(o) = sm.goal_object(h) {
        let mut goal = o.position;
        if h.is_kind_of(KINDOF_PROJECTILE) {
            let half = half_height(h, o.id);
            goal.z += half;
            if o.position.z < goal.z {
                goal.z += half;
            }
        }
        mv(sm, id).goal_position = goal;
    } else {
        mv(sm, id).goal_position = *sm.goal_position();
    }
    let ret = internal_on_enter(sm, h, id)?;
    if h.formation_id() != 0 {
        if let Some(speed) = h.group_speed() {
            ai(h).desired_speed = speed;
        }
    }
    Ok(ret)

}

/// `AIMoveToState::update()` (`m_isMoveTo`: false for the attack-move state, batch A11).
pub(crate) fn move_to_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, is_move_to: bool) -> SmResult<StateReturn> {
    let adjustment = with_ai(h, |a, h| a.get_mood_matrix_action_adjustment(h, &*sm, MM_ACTION_MOVE));
    if is_move_to && adjustment & MAA_ACTION_TO_ATTACK_MOVE != 0 {
        let goal = mv(sm, id).goal_position;
        ai_commands::ai_attack_move_to_position(sm, h, &goal, NO_MAX_SHOTS_LIMIT, CMD_FROM_AI)?;
    }
    // "if we have a goal object, move to it, as it may have moved"
    let is_missile = h.is_kind_of(KINDOF_PROJECTILE);
    if let Some(o) = sm.goal_object(h) {
        let mut goal = o.position;
        let got_physics = h.has_physics() && h.object_has_physics(o.id);
        if is_missile {
            let half = half_height(h, o.id);
            goal.z += half;
            let z_delta = goal.z - h.position().z;
            if z_delta > 0.0 {
                goal.z += z_delta;
            }
        }
        if got_physics && is_missile && !h.object_is_kind_of(o.id, KINDOF_IMMOBILE) {
            let our_pos = h.position();
            let (dx, dy, dz) = (goal.x - our_pos.x, goal.y - our_pos.y, goal.z - our_pos.z);
            let mut my_speed = h.velocity_magnitude();
            let goal_speed = h.object_velocity_magnitude(o.id);
            if my_speed < 5.0 {
                // "avoid divide by 0."
                my_speed = 5.0;
            }
            // (0.5*delta.length()) is a double upstream
            let lead_distance = ((0.5 * f64::from((dx * dx + dy * dy + dz * dz).sqrt())) * f64::from(goal_speed) / f64::from(my_speed)) as f32;
            let dir = h.object_unit_direction_vector_3d(o.id);
            goal.x += dir.x * lead_distance;
            goal.y += dir.y * lead_distance;
            goal.z += dir.z * lead_distance;
        }
        mv(sm, id).goal_position = goal;
    } else if is_missile {
        // "When missiles are moving uphill, they need to start up quickly to clear hills."
        let mut goal = *sm.goal_position();
        let z_delta = goal.z - h.position().z;
        if z_delta > 0.0 {
            goal.z += z_delta;
        }
        mv(sm, id).goal_position = goal;
    }
    internal_update(sm, h, id)

}

/// The states' `onEnter`.
pub fn on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    match sm.state(id) {
        Some(AiState::MoveTo { .. }) => move_to_on_enter(sm, h, id),
        Some(AiState::MoveOutOfTheWay { .. }) => {
            mv(sm, id).adjust_destinations = true;
            let Some(path) = &ai(h).path else {
                // "Must have existing path."
                return Ok(StateReturn::FAILURE);
            };
            let goal = *path.node(path.last_node().expect("a node")).position();
            mv(sm, id).goal_position = goal;
            internal_on_enter(sm, h, id)
        }
        Some(AiState::MoveAndTighten { .. }) => {
            {
                let (mv, ok, check) = tighten(sm, id);
                mv.adjust_destinations = false;
                *ok = 1;
                *check = true;
            }
            h.pf_remove_goal();
            let goal = *sm.goal_position();
            mv(sm, id).goal_position = goal;
            ai_paths::request_approach_path(h, &goal);
            internal_on_enter(sm, h, id)
        }
        Some(AiState::MoveAwayFromRepulsors { .. }) => {
            mv(sm, id).adjust_destinations = false;
            let range = h.object_vision_range();
            let enemy = h.find_closest_repulsor(range);
            if enemy == INVALID_ID {
                return Ok(StateReturn::FAILURE);
            }
            with_ai(h, |a, h| a.choose_locomotor_set(h, LOCOMOTORSET_PANIC));
            h.set_model_condition_state(MODELCONDITION_PANICKING);
            {
                let (_, ok, check) = tighten(sm, id);
                *ok = 1;
                *check = true;
            }
            h.pf_remove_goal();
            ai_paths::request_safe_path(h, enemy);
            internal_on_enter(sm, h, id)
        }
        Some(AiState::FollowPath { .. }) => follow_path_on_enter(sm, h, id),
        Some(AiState::MoveAndEvacuate { .. }) => {
            // "This state is not user interruptable."
            sm.lock();
            let origin = h.position();
            if let Some(AiState::MoveAndEvacuate { origin: o, .. }) = sm.state_mut(id) {
                *o = origin;
            }
            mv(sm, id).adjust_destinations = true;
            // "if we have a goal object, move to it, otherwise move to goal position"
            let goal = match sm.goal_object(h) {
                Some(o) => o.position,
                None => *sm.goal_position(),
            };
            mv(sm, id).goal_position = goal;
            internal_on_enter(sm, h, id)
        }
        Some(AiState::MoveAndDelete { .. }) => {
            mv(sm, id).adjust_destinations = false;
            sm.lock();
            let goal = match sm.goal_object(h) {
                Some(o) => o.position,
                None => *sm.goal_position(),
            };
            mv(sm, id).goal_position = goal;
            // "We may be moving off the map."
            if let Some(AiState::MoveAndDelete { append_goal_position, .. }) = sm.state_mut(id) {
                *append_goal_position = true;
            }
            internal_on_enter(sm, h, id)
        }
        _ => unreachable!("a move state"),
    }
}

/// `AIFollowPathState::onEnter()`: the first point of the machine's goal path (none: failure), the path's extra distance to the
/// next ones.
fn follow_path_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let index = 0;
    if let Some(AiState::FollowPath { index: i, .. }) = sm.state_mut(id) {
        *i = index;
    }
    let Some(pos) = goal_path_position(h, index) else { return Ok(StateReturn::FAILURE) };
    // "set initial movement goal"
    mv(sm, id).goal_position = pos;
    let next_pos = goal_path_position(h, index + 1);
    set_adjust_final(sm, id, true);
    // friend_setCurrentGoalPathIndex
    ai(h).next_goal_path_index = index;
    if id == AI_FOLLOW_EXITPRODUCTION_PATH {
        ai(h).set_can_path_through_units(true);
        mv(sm, id).adjust_destinations = false;
        set_adjust_final(sm, id, true);
    }
    let ret = internal_on_enter(sm, h, id)?;
    if h.formation_id() != 0 {
        if let Some(speed) = h.group_speed() {
            ai(h).desired_speed = speed;
        }
    }
    follow_path_extra_distance(sm, h, id, &pos, next_pos.as_ref(), index);
    Ok(ret)
}

/// The path's extra distance after the point `pos` at `index` (the next segment, and four cells more if one follows it); at the
/// last point, the final adjustment and a projectile's precise z.
fn follow_path_extra_distance<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, pos: &Coord3D, next_pos: Option<&Coord3D>, index: i32) {
    if let Some(next) = next_pos {
        let (dx, dy) = (next.x - pos.x, next.y - pos.y);
        let mut offset = (dx * dx + dy * dy).sqrt();
        if goal_path_position(h, index + 2).is_some() {
            offset += 4.0 * PATHFIND_CELL_SIZE_F;
        }
        ai(h).set_path_extra_distance(offset);
        // "We are in the middle of a path, so don't set the final goal location yet."
        mv(sm, id).adjust_destinations = false;
    } else {
        let adjust_final = follow_path_flags(sm, id).0;
        mv(sm, id).adjust_destinations = adjust_final;
        ai(h).set_path_extra_distance(0.0);
        // "urg. hacky. if we are a projectile on the last segment, turn on precise z-pos."
        if h.is_kind_of(KINDOF_PROJECTILE) {
            set_precise_z(h, true);
        }
    }
}

pub(crate) fn set_precise_z<H: AiStateHost>(h: &mut H, on: bool) {
    let a = ai(h);
    if let Some(cur) = a.cur_locomotor {
        a.locomotor_set.get_mut(cur).expect("the locomotor").set_use_precise_z_pos(on);
    }
}

/// `friend_getGoalPathPosition(i)`.
fn goal_path_position<H: AiStateHost>(h: &mut H, i: i32) -> Option<Coord3D> {
    crate::logic::ai::ai_states::aism_goal_path_position(&ai(h).machine, i)
}

fn set_adjust_final<H: AiStateHost>(sm: &mut Sm<H>, id: StateId, b: bool) {
    if let Some(AiState::FollowPath { adjust_final, .. }) = sm.state_mut(id) {
        *adjust_final = b;
    }
}

/// (`m_adjustFinal`, `m_adjustFinalOverride`).
fn follow_path_flags<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> (bool, bool) {
    match sm.state(id) {
        Some(AiState::FollowPath { adjust_final, adjust_final_override, .. }) => (*adjust_final, *adjust_final_override),
        _ => unreachable!("a follow-path state"),
    }
}

/// `AIFollowPathState::update()`: the move to the current point; at its end (or failure, once the retries are spent) the next
/// point not within a cell, with the path recomputed; the end of the path is success.
fn follow_path_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let goal = mv(sm, id).goal_position;
    sm.set_goal_position(Some(&goal));
    // "do movement"
    let status = internal_update(sm, h, id)?;
    // "if move to has finished, move to next point on path"
    if status != StateReturn::SUCCESS && status != StateReturn::FAILURE {
        return Ok(status);
    }
    let mut index = match sm.state_mut(id) {
        Some(AiState::FollowPath { index, retry_count, .. }) => {
            if status == StateReturn::FAILURE && *retry_count > 0 && !plant(3280) {
                // "If we failed, & haven't reached retry limit, try again."
                *retry_count -= 1;
            } else {
                *index += 1;
            }
            *index
        }
        _ => unreachable!("a follow-path state"),
    };
    let mut pos = goal_path_position(h, index);
    let mut too_close = true;
    while let (Some(p), true) = (pos, too_close) {
        let here = h.position();
        let (dx, dy) = (p.x - here.x, p.y - here.y);
        too_close = dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F;
        if too_close {
            index += 1;
            pos = goal_path_position(h, index);
        }
    }
    if let Some(AiState::FollowPath { index: i, .. }) = sm.state_mut(id) {
        *i = index;
    }
    ai(h).next_goal_path_index = index;
    // "we have exited whatever object we are leaving, if any."
    ai(h).ignore_obstacle_id(INVALID_ID);
    let Some(pos) = pos else {
        // "reached the end of the path"
        return Ok(StateReturn::SUCCESS);
    };
    ai(h).friend_starting_move();
    // "set next movement goal"
    mv(sm, id).goal_position = pos;
    let next_pos = goal_path_position(h, index + 1);
    if next_pos.is_some() {
        follow_path_extra_distance(sm, h, id, &pos, next_pos.as_ref(), index);
    } else {
        let (adjust_final, adjust_override) = follow_path_flags(sm, id);
        let adjusts = adjust_final && (adjust_override || with_ai(h, |a, h| a.is_doing_ground_movement(h)));
        mv(sm, id).adjust_destinations = adjusts;
        if get_adjusts_destination(sm, h, id) {
            let mut goal = mv(sm, id).goal_position;
            let ok = h.pf_adjust_destination(&mut goal);
            mv(sm, id).goal_position = goal;
            if !ok && !plant(3281) {
                return Ok(StateReturn::FAILURE);
            }
            let layer = h.layer_for_destination(&goal);
            h.pf_update_goal(&goal, layer);
        }
        if h.is_kind_of(KINDOF_PROJECTILE) {
            set_precise_z(h, true);
        }
    }
    compute_path(sm, h, id);
    Ok(StateReturn::CONTINUE)
}

/// The states' `onExit`.
pub fn on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, _status: StateExitType) {
    match sm.state(id) {
        Some(AiState::MoveAndEvacuate { origin, .. }) => {
            let origin = *origin;
            sm.unlock();
            // "In case we follow with a AIMoveAndDeleteState."
            sm.set_goal_position(Some(&origin));
        }
        Some(AiState::MoveAndDelete { .. }) => sm.unlock(),
        _ => {}
    }
    internal_on_exit(sm, h, id);
    match sm.state(id) {
        Some(AiState::FollowPath { .. }) => {
            // "turn off precision-z-pos when we exit, just in case."
            ai(h).set_can_path_through_units(false);
            set_precise_z(h, false);
            ai(h).next_goal_path_index = -1;
        }
        Some(AiState::MoveOutOfTheWay { .. }) => {
            let a = ai(h);
            a.destroy_path();
            a.set_can_path_through_units(false);
            // clearMoveOutOfWay()
            a.move_out_of_way1 = INVALID_ID;
            a.move_out_of_way2 = INVALID_ID;
        }
        Some(AiState::MoveAwayFromRepulsors { .. }) => h.clear_model_condition_state(MODELCONDITION_PANICKING),
        _ => {}
    }
}

/// The states' `update`.
pub fn update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    match sm.state(id) {
        Some(AiState::MoveTo { is_move_to, .. }) => {
            let is_move_to = *is_move_to;
            move_to_update(sm, h, id, is_move_to)
        }
        Some(AiState::MoveOutOfTheWay { .. }) => {
            if h.is_effectively_dead() {
                return Ok(StateReturn::SUCCESS);
            }
            internal_update(sm, h, id)
        }
        Some(AiState::MoveAndTighten { .. }) => {
            if tighten(sm, id).2.to_owned() && ai(h).path.is_some() && !ai(h).waiting_for_path {
                let (mv, _, check) = tighten(sm, id);
                mv.adjust_destinations = true;
                *check = false;
            }
            internal_update(sm, h, id)
        }
        Some(AiState::FollowPath { .. }) => follow_path_update(sm, h, id),
        Some(AiState::MoveAndEvacuate { .. }) => {
            if h.is_effectively_dead() {
                return Ok(StateReturn::FAILURE);
            }
            // "do movement"
            let status = internal_update(sm, h, id)?;
            if status != StateReturn::CONTINUE {
                if h.is_effectively_dead() {
                    return Ok(StateReturn::FAILURE);
                }
                ai_commands::ai_evacuate(sm, h, 0, CMD_FROM_AI)?;
                h.team_set_active();
            }
            Ok(status)
        }
        Some(AiState::MoveAndDelete { .. }) => {
            if h.is_effectively_dead() {
                return Ok(StateReturn::FAILURE);
            }
            // "do movement"
            let a = ai(h);
            if let Some(cur) = a.cur_locomotor {
                a.locomotor_set.get_mut(cur).expect("the locomotor").set_allow_invalid_position(true);
            }
            let append = matches!(sm.state(id), Some(AiState::MoveAndDelete { append_goal_position: true, .. }));
            if append && !ai(h).waiting_for_path && ai(h).path.is_some() {
                let mut goal = mv(sm, id).goal_position;
                goal.z = h.ground_height(goal.x, goal.y);
                mv(sm, id).goal_position = goal;
                ai(h).path.as_mut().expect("the path").append_node(&goal, crate::common::game_type::LAYER_GROUND);
                // "just did it."
                if let Some(AiState::MoveAndDelete { append_goal_position, .. }) = sm.state_mut(id) {
                    *append_goal_position = plant(3282);
                }
            }
            let status = internal_update(sm, h, id)?;
            if status != StateReturn::CONTINUE {
                h.destroy_object();
            }
            Ok(status)
        }
        Some(AiState::MoveAwayFromRepulsors { .. }) => {
            if tighten(sm, id).2.to_owned() {
                let a = ai(h);
                let goal = match &a.path {
                    Some(path) if !a.waiting_for_path => Some(*path.node(path.last_node().expect("a node")).position()),
                    _ => None,
                };
                if let Some(goal) = goal {
                    let (mv, _, check) = tighten(sm, id);
                    mv.goal_position = goal;
                    mv.adjust_destinations = false;
                    *check = false;
                }
            }
            internal_update(sm, h, id)
        }
        _ => unreachable!("a move state"),
    }
}
