// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIFollowWaypointPathState, AIFollowWaypointPathExactState,
//     AIWanderState, AIWanderInPlaceState, AIPanicState)
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
//! The AI's waypoint states (`p1w13-sonnet`, layer 10, batch A9): `AIFollowWaypointPathState` (a waypoint path link by link,
//! alone or as a team, with `ALLOW_BACKTRACK`'s random link), `AIFollowWaypointPathExactState` (the path from the waypoints as
//! given), `AIWanderState` and `AIPanicState` (a waypoint path with a random sideways offset, giving up near a repulsor) and
//! `AIWanderInPlaceState` (random points about the start).
//!
//! Each is a move state ([`MoveToData`] in its [`AiState`] variant); the follow, wander and panic states share
//! [`WaypointPathData`] (`AIFollowWaypointPathState`'s members). A waypoint is named by its id (`Waypoint::getID`).
//!
//! Upstream facts kept:
//! - `AIFollowWaypointPathState::onEnter` dereferences the goal waypoint unchecked when moving as a team (its `NULL` check covers
//!   the individual's case only); `computeGoal`, off the map, and the exact state dereference the current locomotor unchecked.
//!   This port panics there (callers must not reach them).
//! - The machine's state and its temporary state are one instance per id: a waypoint state in both is left without a current
//!   waypoint when one of them ends, and the other's next update dereferences it.
//! - The members no constructor sets (the offset, the sleep, the waypoints, the append flag; the wander and panic states' timer
//!   and wait) are the pool's zeros; the panic state keeps its offset from one entry to the next when the locomotor does not
//!   wander, and the wander state keeps the append flag.
//! - `getNextWaypoint` with no links asks for a random value in `[0, -1]`, which is `-1` without a draw, and `getLink(-1)` is none.
//! - The panic state moves on only after a success (a failed move continues); the wander state after either.
//! - Loading names a waypoint by the saved id (upstream's `getWaypointByID`, none for an id the terrain does not know: never
//!   saved).

use crate::common::base_type::{real_to_int_floor, Coord2D, Coord3D, Region3D};
use crate::common::game_type::{INVALID_ID, LAYER_GROUND, LAYER_WALL};
use crate::common::kind_of::{KINDOF_CAN_BE_REPULSED, KINDOF_PROJECTILE};
use crate::common::model_state::MODELCONDITION_PANICKING;
use crate::common::state_machine::{SmResult, StateId, StateMachine, StateReturn};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferVersion};
use crate::logic::ai::ai_commands;
use crate::logic::ai::ai_move_states::{self as move_states, mv, MoveToData};
use crate::logic::ai::ai_paths;
use crate::logic::ai::ai_states::{ai, with_ai, AiState, AiStateHost};
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::modules::plant;
use crate::logic::weapon::NO_MAX_SHOTS_LIMIT;

type Sm<H> = StateMachine<AiState<H>>;

/// `INVALID_WAYPOINT_ID`.
pub const INVALID_WAYPOINT_ID: u32 = 0x7FFF_FFFF;

/// `AIFollowWaypointPathState`'s members (the wander and panic states' too).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaypointPathData {
    /// `m_groupOffset`.
    pub group_offset: Coord2D,
    /// `m_angle` (used only under `ROTATE_OFFSETS`, which is not defined; saved).
    pub angle: f32,
    /// `m_framesSleeping`.
    pub frames_sleeping: i32,
    /// `m_currentWaypoint`.
    pub current: Option<u32>,
    /// `m_priorWaypoint`.
    pub prior: Option<u32>,
    /// `m_appendGoalPosition`: "moving off the map".
    pub append_goal_position: bool,
}

/// A waypoint's id, or `INVALID_WAYPOINT_ID`.
fn waypoint_id(w: Option<u32>) -> u32 {
    w.unwrap_or(INVALID_WAYPOINT_ID)
}

/// Loads a waypoint saved by its id.
fn waypoint_from_id(id: u32) -> Option<u32> {
    (id != INVALID_WAYPOINT_ID).then_some(id)
}

/// `xfer` of a waypoint: its id, the waypoint on load.
fn xfer_waypoint(w: &mut Option<u32>, xfer: &mut dyn Xfer) -> XferResult<()> {
    let mut id = waypoint_id(*w);
    xfer.xfer_unsigned_int(&mut id)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        *w = waypoint_from_id(id);
    }
    Ok(())
}

/// `AIFollowWaypointPathState::xfer` (version 1): the base's, the offset, the angle, the sleep, the two waypoints' ids, the append
/// flag.
pub fn waypoint_path_xfer(mv: &mut MoveToData, wp: &mut WaypointPathData, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer.xfer_coord2d(&mut wp.group_offset)?;
    xfer.xfer_real(&mut wp.angle)?;
    xfer.xfer_int(&mut wp.frames_sleeping)?;
    xfer_waypoint(&mut wp.current, xfer)?;
    xfer_waypoint(&mut wp.prior, xfer)?;
    xfer.xfer_bool(&mut wp.append_goal_position)?;
    Ok(())
}

/// `AIWanderState::xfer` and `AIPanicState::xfer` (version 1): the follow state's, the wait and the timer.
pub fn wander_xfer(mv: &mut MoveToData, wp: &mut WaypointPathData, wait_frames: &mut i32, timer: &mut i32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    waypoint_path_xfer(mv, wp, xfer)?;
    xfer.xfer_int(wait_frames)?;
    xfer.xfer_int(timer)?;
    Ok(())
}

/// `AIWanderInPlaceState::xfer` (version 1): the base's, the origin, the wait and the timer.
pub fn wander_in_place_xfer(mv: &mut MoveToData, origin: &mut Coord3D, wait_frames: &mut i32, timer: &mut i32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer.xfer_coord3d(origin)?;
    xfer.xfer_int(wait_frames)?;
    xfer.xfer_int(timer)?;
    Ok(())
}

/// `AIFollowWaypointPathExactState::xfer` (version 1): the base's and the last waypoint's id.
pub fn exact_xfer(mv: &mut MoveToData, last: &mut Option<u32>, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    mv.xfer(xfer)?;
    xfer_waypoint(last, xfer)
}

/// The waypoint-path data of the state `id`.
fn wp<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> &mut WaypointPathData {
    match sm.state_mut(id) {
        Some(AiState::FollowWaypointPath { wp, .. }) | Some(AiState::Wander { wp, .. }) | Some(AiState::Panic { wp, .. }) => wp,
        _ => unreachable!("a waypoint-path state"),
    }
}

/// The wander and panic states' (wait, timer), and the wander-in-place state's.
fn wait_timer<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> (&mut i32, &mut i32) {
    match sm.state_mut(id) {
        Some(AiState::Wander { wait_frames, timer, .. })
        | Some(AiState::Panic { wait_frames, timer, .. })
        | Some(AiState::WanderInPlace { wait_frames, timer, .. }) => (wait_frames, timer),
        _ => unreachable!("a wandering state"),
    }
}

fn current<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> u32 {
    wp(sm, id).current.expect("upstream dereferences the current waypoint")
}

fn cur_locomotor_mut<H: AiStateHost>(h: &mut H) -> &mut crate::logic::locomotor::Locomotor {
    let a = ai(h);
    let cur = a.cur_locomotor.expect("upstream dereferences the current locomotor unchecked");
    a.locomotor_set.get_mut(cur).expect("the locomotor")
}

fn set_precise_z_if_locomotor<H: AiStateHost>(h: &mut H, on: bool) {
    let a = ai(h);
    if let Some(cur) = a.cur_locomotor {
        a.locomotor_set.get_mut(cur).expect("the locomotor").set_use_precise_z_pos(on);
    }
}

/// `getNextWaypoint()` (`ALLOW_BACKTRACK`): a random link of the current waypoint (none without links), which becomes the
/// prior; the machine's goal position is the current waypoint's.
fn get_next_waypoint<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> Option<u32> {
    let cur = current(sm, id);
    let link_count = h.waypoint_num_links(cur);
    let which = if plant(3293) { 0 } else { h.logic_random_value(0, link_count - 1) };
    let next = h.waypoint_link(cur, which);
    wp(sm, id).prior = Some(cur);
    // "THANKS, JOHN"
    let loc = h.waypoint_location(cur);
    sm.set_goal_position(Some(&loc));
    next
}

/// `hasNextWaypoint()` (`ALLOW_BACKTRACK`): the current waypoint has links.
fn has_next_waypoint<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> bool {
    let cur = current(sm, id);
    h.waypoint_num_links(cur) > 0
}

/// `calcExtraPathDistance()`: a tenth of a cell and the length of the first links, five at most ("in case of circular paths").
fn calc_extra_path_distance<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> f32 {
    let mut extra = PATHFIND_CELL_SIZE_F / 10.0;
    let mut cur_way = wp(sm, id).current;
    let mut limit = 5;
    while let (Some(cur), true) = (cur_way, limit > 0) {
        limit -= 1;
        if h.waypoint_num_links(cur) == 0 {
            return extra;
        }
        let next = h.waypoint_link(cur, 0).expect("the first link");
        let (n, c) = (h.waypoint_location(next), h.waypoint_location(cur));
        let delta = Coord2D { x: n.x - c.x, y: n.y - c.y };
        extra += delta.length();
        cur_way = Some(next);
    }
    extra
}

/// `computeGoal(useGroupOffsets)`: the current waypoint (on the ground, or the wall) moved by the offset, clamped into the map
/// when the waypoint is in it; the destination adjusted at the path's end only; off the map, not adjusted, the locomotor allowed
/// off it and the goal to be appended.
fn compute_goal<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let Some(cur) = wp(sm, id).current else { return };
    let mut dest = h.waypoint_location(cur);
    // "waypoints are always on the ground."
    mv(sm, id).goal_layer = LAYER_GROUND;
    if h.pf_is_point_on_wall(&dest) {
        // "except when they're on the wall."
        if !plant(3297) {
            dest.z = h.pf_wall_height();
        }
        mv(sm, id).goal_layer = LAYER_WALL;
    }
    let extra = calc_extra_path_distance(sm, h, id);
    ai(h).set_path_extra_distance(extra);
    if has_next_waypoint(sm, h, id) {
        // "We are in the middle of a path, so don't set the final goal location yet."
        mv(sm, id).adjust_destinations = false;
    } else {
        mv(sm, id).adjust_destinations = true;
        // "urg. hacky. if we are a projectile on the last segment, turn on precise z-pos."
        if h.is_kind_of(KINDOF_PROJECTILE) {
            set_precise_z_if_locomotor(h, true);
        }
    }
    let offset = wp(sm, id).group_offset;
    let mut goal = dest;
    goal.x += offset.x;
    goal.y += offset.y;
    if mv(sm, id).goal_layer == LAYER_WALL {
        if !h.pf_is_point_on_wall(&goal) {
            goal = dest;
        }
    } else {
        goal.z = h.ground_height(goal.x, goal.y);
    }
    let (lo, hi) = h.terrain_max_pathfind_extent();
    let extent = Region3D { lo, hi };
    if extent.is_in_region_no_z(&dest) && !extent.is_in_region_no_z(&goal) {
        // "The waypoint is on the map. Check & see if the adjusted position is off map": "clamp to in region."
        if goal.x < extent.lo.x + PATHFIND_CELL_SIZE_F {
            goal.x = extent.lo.x + PATHFIND_CELL_SIZE_F;
        }
        if goal.y < extent.lo.y + PATHFIND_CELL_SIZE_F {
            goal.y = extent.lo.y + PATHFIND_CELL_SIZE_F;
        }
        if goal.x > extent.hi.x - PATHFIND_CELL_SIZE_F && !plant(3290) {
            goal.x = extent.hi.x - PATHFIND_CELL_SIZE_F;
        }
        if goal.y > extent.hi.y - PATHFIND_CELL_SIZE_F {
            goal.y = extent.hi.y - PATHFIND_CELL_SIZE_F;
        }
    }
    mv(sm, id).goal_position = goal;
    if !extent.is_in_region_no_z(&goal) {
        // "moving off the map."
        mv(sm, id).adjust_destinations = false;
        cur_locomotor_mut(h).set_allow_invalid_position(true);
        wp(sm, id).append_goal_position = true;
    }
}

/// The wander and panic states' random offset: a whole number of cells within the locomotor's wander width, if it wanders.
fn wander_offset<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let a = ai(h);
    let Some(cur) = a.cur_locomotor else { return };
    let width = a.locomotor_set.get(cur).expect("the locomotor").get_wander_width_factor();
    if width > 0.0 {
        let delta = real_to_int_floor(width + 0.5).max(if plant(3294) { 0 } else { 1 });
        let x = h.logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        let y = h.logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        wp(sm, id).group_offset = Coord2D { x, y };
    }
}

/// The wander-in-place state's goal: the origin and a random whole number of cells within the locomotor's wander radius (3
/// without a locomotor).
fn wander_in_place_goal<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let a = ai(h);
    let delta = match a.cur_locomotor {
        Some(cur) => {
            let radius = a.locomotor_set.get(cur).expect("the locomotor").get_wander_about_point_radius();
            real_to_int_floor((radius / PATHFIND_CELL_SIZE_F) + 0.5)
        }
        None => 3,
    };
    let x = h.logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
    let y = h.logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
    let origin = match sm.state(id) {
        Some(AiState::WanderInPlace { origin, .. }) => *origin,
        _ => unreachable!("the wander-in-place state"),
    };
    let mut goal = origin;
    goal.x += x;
    goal.y += y;
    mv(sm, id).goal_position = goal;
}

/// `10 + (getMachineOwner()->getID() & 0x7)`.
fn wait_frames_of<H: AiStateHost>(h: &H) -> i32 {
    10 + (h.object_id() & 0x7)
}

/// The wander, wander-in-place and panic states' repulsor check: every `m_waitFrames` updates, a repulsor in vision range fails.
fn repulsed<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> bool {
    if !h.is_kind_of(KINDOF_CAN_BE_REPULSED) {
        return false;
    }
    let (wait, timer) = wait_timer(sm, id);
    *timer -= 1;
    if *timer < 0 {
        *timer = *wait;
        let range = h.object_vision_range();
        if h.find_closest_repulsor(range) != INVALID_ID {
            return true;
        }
    }
    false
}

/// The follow state's move to a new waypoint: the destination adjusted for ground movement (failure if it cannot be), the path
/// recomputed, the pathfinder's goal updated.
fn move_to_new_goal<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> Option<StateReturn> {
    if move_states::get_adjusts_destination(sm, h, id) && with_ai(h, |a, h| a.is_doing_ground_movement(h)) {
        let mut goal = mv(sm, id).goal_position;
        let ok = h.pf_adjust_destination(&mut goal);
        mv(sm, id).goal_position = goal;
        if !ok {
            return Some(StateReturn::FAILURE);
        }
    }
    ai(h).friend_starting_move();
    move_states::compute_path(sm, h, id);
    if move_states::get_adjusts_destination(sm, h, id) {
        let (goal, layer) = (mv(sm, id).goal_position, mv(sm, id).goal_layer);
        h.pf_update_goal(&goal, layer);
    }
    None
}

/// The states' `onEnter`.
pub fn on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    match sm.state(id) {
        Some(AiState::FollowWaypointPath { as_group, .. }) => {
            let as_group = *as_group;
            let goal_waypoint = ai(h).machine.goal_waypoint;
            {
                let w = wp(sm, id);
                // "not moving off the map at this point."
                w.append_goal_position = false;
                w.prior = None;
                w.current = goal_waypoint;
            }
            if goal_waypoint.is_none() && !as_group {
                return Ok(StateReturn::FAILURE);
            }
            let cur = goal_waypoint.expect("upstream dereferences the goal waypoint unchecked when moving as a team");
            let loc = h.waypoint_location(cur);
            sm.set_goal_position(Some(&loc));
            {
                let w = wp(sm, id);
                w.frames_sleeping = 0;
                w.group_offset = Coord2D { x: 0.0, y: 0.0 };
            }
            let mut speed = FAST_AS_POSSIBLE;
            if as_group {
                h.team_set_current_waypoint(Some(cur));
                if let Some(s) = h.group_speed() {
                    speed = s;
                    let center = h.group_center();
                    let pos = h.position();
                    wp(sm, id).group_offset = Coord2D { x: pos.x - center.x, y: pos.y - center.y };
                }
            }
            // (`m_currentWaypoint == NULL && m_moveAsGroup`: the team's current waypoint; not reachable, the goal waypoint was
            // dereferenced above)
            // "set initial movement goal"
            compute_goal(sm, h, id);
            let ret = move_states::internal_on_enter(sm, h, id)?;
            ai(h).desired_speed = speed;
            // "Update the extra path distance. AIInternalMoveToState::onEnter resets it."
            let extra = calc_extra_path_distance(sm, h, id);
            ai(h).set_path_extra_distance(extra);
            if has_next_waypoint(sm, h, id) {
                // "We are in the middle of a path, so don't set the final goal location yet."
                mv(sm, id).adjust_destinations = false;
            } else {
                let ground = with_ai(h, |a, h| a.is_doing_ground_movement(h));
                mv(sm, id).adjust_destinations = ground;
                if move_states::get_adjusts_destination(sm, h, id) {
                    let mut goal = mv(sm, id).goal_position;
                    let ok = h.pf_adjust_destination(&mut goal);
                    mv(sm, id).goal_position = goal;
                    if !ok {
                        return Ok(StateReturn::FAILURE);
                    }
                    let layer = mv(sm, id).goal_layer;
                    h.pf_update_goal(&goal, layer);
                }
                if h.is_kind_of(KINDOF_PROJECTILE) {
                    set_precise_z_if_locomotor(h, true);
                }
            }
            Ok(ret)
        }
        Some(AiState::FollowWaypointPathExact { as_group, .. }) => {
            let as_group = *as_group;
            let Some(cur) = ai(h).machine.goal_waypoint else { return Ok(StateReturn::FAILURE) };
            let loc = h.waypoint_location(cur);
            sm.set_goal_position(Some(&loc));
            let mut offset = Coord2D { x: 0.0, y: 0.0 };
            let mut speed = FAST_AS_POSSIBLE;
            if as_group {
                if let Some(s) = h.group_speed() {
                    speed = s;
                    let center = h.group_center();
                    let pos = h.position();
                    offset = Coord2D { x: pos.x - center.x, y: pos.y - center.y };
                }
            }
            ai(h).set_can_path_through_units(true);
            mv(sm, id).adjust_destinations = false;
            // "set initial movement goal"
            let ret = move_states::internal_on_enter(sm, h, id)?;
            ai_paths::set_path_from_waypoint(h, Some(cur), offset.x, offset.y);
            if let Some(AiState::FollowWaypointPathExact { last, .. }) = sm.state_mut(id) {
                *last = Some(cur);
            }
            // "allow it to move off the map."
            cur_locomotor_mut(h).set_allow_invalid_position(true);
            ai(h).desired_speed = speed;
            Ok(ret)
        }
        Some(AiState::Wander { .. }) => {
            let goal_waypoint = ai(h).machine.goal_waypoint;
            {
                let w = wp(sm, id);
                w.current = goal_waypoint;
                w.prior = None;
            }
            if goal_waypoint.is_none() {
                return Ok(StateReturn::FAILURE);
            }
            wp(sm, id).group_offset = Coord2D { x: 0.0, y: 0.0 };
            wander_offset(sm, h, id);
            let wait = wait_frames_of(h);
            let (w, t) = wait_timer(sm, id);
            *t = 0;
            *w = wait;
            // "set initial movement goal"
            compute_goal(sm, h, id);
            let ret = move_states::internal_on_enter(sm, h, id)?;
            let extra = calc_extra_path_distance(sm, h, id);
            ai(h).set_path_extra_distance(extra);
            Ok(ret)
        }
        Some(AiState::WanderInPlace { .. }) => {
            let pos = h.position();
            if let Some(AiState::WanderInPlace { origin, .. }) = sm.state_mut(id) {
                *origin = pos;
            }
            with_ai(h, |a, h| a.choose_locomotor_set(h, LOCOMOTORSET_WANDER));
            wander_in_place_goal(sm, h, id);
            let wait = wait_frames_of(h);
            let (w, t) = wait_timer(sm, id);
            *t = 0;
            *w = wait;
            move_states::internal_on_enter(sm, h, id)
        }
        Some(AiState::Panic { .. }) => {
            let goal_waypoint = ai(h).machine.goal_waypoint;
            wp(sm, id).current = goal_waypoint;
            if goal_waypoint.is_none() {
                return Ok(StateReturn::FAILURE);
            }
            // "set initial movement goal"
            wander_offset(sm, h, id);
            compute_goal(sm, h, id);
            let ret = move_states::internal_on_enter(sm, h, id)?;
            let wait = wait_frames_of(h);
            let (w, t) = wait_timer(sm, id);
            *t = 0;
            *w = wait;
            // "Update the extra path distance. AIInternalMoveToState::onEnter resets it."
            let extra = calc_extra_path_distance(sm, h, id);
            ai(h).set_path_extra_distance(extra);
            h.set_model_condition_state(MODELCONDITION_PANICKING);
            Ok(ret)
        }
        _ => unreachable!("a waypoint state"),
    }
}

/// The states' `onExit`.
pub fn on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    match sm.state(id) {
        Some(AiState::FollowWaypointPath { .. }) | Some(AiState::Wander { .. }) => {
            move_states::internal_on_exit(sm, h, id);
            // "turn off precision-z-pos when we exit, just in case."
            let a = ai(h);
            if let Some(cur) = a.cur_locomotor {
                let l = a.locomotor_set.get_mut(cur).expect("the locomotor");
                l.set_use_precise_z_pos(false);
                l.set_ultra_accurate(false);
            }
        }
        Some(AiState::FollowWaypointPathExact { last, .. }) => {
            let last = *last;
            move_states::internal_on_exit(sm, h, id);
            ai(h).completed_waypoint = if plant(3295) { None } else { last };
            ai(h).set_can_path_through_units(false);
            // "turn off allow it to move off the map."
            cur_locomotor_mut(h).set_allow_invalid_position(false);
        }
        Some(AiState::WanderInPlace { .. }) => move_states::internal_on_exit(sm, h, id),
        Some(AiState::Panic { .. }) => {
            h.clear_model_condition_state(MODELCONDITION_PANICKING);
            move_states::internal_on_exit(sm, h, id);
        }
        _ => unreachable!("a waypoint state"),
    }
}

/// `AIFollowWaypointPathState::update()`.
fn follow_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, as_group: bool, is_follow: bool) -> SmResult<StateReturn> {
    {
        let w = wp(sm, id);
        if w.frames_sleeping > 0 {
            w.frames_sleeping -= 1;
            return Ok(StateReturn::CONTINUE);
        }
    }
    let cur = current(sm, id);
    let loc = h.waypoint_location(cur);
    sm.set_goal_position(Some(&loc));
    let adjustment = with_ai(h, |a, h| a.get_mood_matrix_action_adjustment(h, &*sm, MM_ACTION_MOVE));
    if is_follow && adjustment & MAA_ACTION_TO_ATTACK_MOVE != 0 {
        ai_commands::ai_attack_follow_waypoint_path(sm, h, Some(cur), NO_MAX_SHOTS_LIMIT, as_group, CMD_FROM_AI)?;
    }
    if wp(sm, id).append_goal_position {
        let a = ai(h);
        if !a.waiting_for_path && a.path.is_some() {
            let goal = mv(sm, id).goal_position;
            // "waypoints are always on the ground."
            ai(h).path.as_mut().expect("the path").append_node(&goal, LAYER_GROUND);
            // "just did it."
            wp(sm, id).append_goal_position = plant(3291);
        }
    }
    if as_group && wp(sm, id).current != h.team_current_waypoint() {
        let team_way = h.team_current_waypoint();
        {
            let w = wp(sm, id);
            w.prior = w.current;
            w.current = team_way;
        }
        if team_way.is_none() {
            return Ok(StateReturn::SUCCESS);
        }
        compute_goal(sm, h, id);
        if let Some(r) = move_to_new_goal(sm, h, id) {
            return Ok(r);
        }
    }
    // "do movement"
    let mut status = move_states::internal_update(sm, h, id)?;
    // "We may want to allow ourselves to bail out of this one early."
    if as_group && h.is_skirmish_ai_player() {
        let (center, num_in_group) = h.team_group_center_and_count();
        let goal = mv(sm, id).goal_position;
        let pos = Coord3D { x: center.x - goal.x, y: center.y - goal.y, z: 0.0 };
        let z = if plant(3296) { center.z - goal.z } else { 0.0 };
        let pos = Coord3D { z, ..pos };
        if pos.length() <= num_in_group as f32 * h.ai_data_skirmish_group_fudge_value() {
            // "Consider ourselves close enough."
            status = StateReturn::SUCCESS;
        }
    }
    // "if move to has finished, move to next point on waypoint path"
    if status != StateReturn::CONTINUE {
        let next = get_next_waypoint(sm, h, id);
        wp(sm, id).current = next;
        let prior = wp(sm, id).prior;
        if let Some(p) = prior {
            ai(h).prior_waypoint_id = p;
        }
        if let Some(c) = next {
            ai(h).current_waypoint_id = c;
        }
        // "if there are no links from this waypoint, we're done"
        let Some(next) = next else {
            // "Trigger "end of waypoint path" scripts"
            ai(h).completed_waypoint = prior;
            return Ok(StateReturn::SUCCESS);
        };
        if as_group {
            h.team_set_current_waypoint(Some(next));
        }
        compute_goal(sm, h, id);
        if let Some(r) = move_to_new_goal(sm, h, id) {
            return Ok(r);
        }
        return Ok(StateReturn::CONTINUE);
    }
    Ok(status)
}

/// The wander and panic states' `update()` (the panic state moves on only after a success).
fn wander_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, panic: bool) -> SmResult<StateReturn> {
    // "do movement"
    let status = move_states::internal_update(sm, h, id)?;
    if repulsed(sm, h, id) {
        return Ok(StateReturn::FAILURE);
    }
    // "if move to has finished, move to next point on waypoint path"
    let finished = if panic { status == StateReturn::SUCCESS } else { status != StateReturn::CONTINUE };
    if finished {
        let next = get_next_waypoint(sm, h, id);
        wp(sm, id).current = next;
        // "if there are no links from this waypoint, we're done"
        if next.is_none() {
            ai(h).completed_waypoint = wp(sm, id).prior;
            return Ok(StateReturn::SUCCESS);
        }
        wander_offset(sm, h, id);
        compute_goal(sm, h, id);
        move_states::compute_path(sm, h, id);
        return Ok(StateReturn::CONTINUE);
    }
    // "Never leave this state until told to."
    Ok(StateReturn::CONTINUE)
}

/// The states' `update`.
pub fn update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    match sm.state(id) {
        Some(AiState::FollowWaypointPath { as_group, is_follow, .. }) => {
            let (as_group, is_follow) = (*as_group, *is_follow);
            follow_update(sm, h, id, as_group, is_follow)
        }
        Some(AiState::FollowWaypointPathExact { .. }) => {
            ai(h).set_can_path_through_units(true);
            // "do movement"
            move_states::internal_update(sm, h, id)
        }
        Some(AiState::Wander { .. }) => wander_update(sm, h, id, false),
        Some(AiState::Panic { .. }) => wander_update(sm, h, id, !plant(3292)),
        Some(AiState::WanderInPlace { .. }) => {
            // "do movement"
            let status = move_states::internal_update(sm, h, id)?;
            if repulsed(sm, h, id) {
                return Ok(StateReturn::FAILURE);
            }
            if status != StateReturn::CONTINUE {
                wander_in_place_goal(sm, h, id);
                move_states::internal_on_enter(sm, h, id)?;
                return Ok(StateReturn::CONTINUE);
            }
            // "Never leave this state until told to."
            Ok(StateReturn::CONTINUE)
        }
        _ => unreachable!("a waypoint state"),
    }
}
