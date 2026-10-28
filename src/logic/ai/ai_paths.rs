// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (doPathfind, the request*Path members,
//     setPathFromWaypoint, canComputeQuickPath, computeQuickPath, computePath, computeAttackPath, friend_setGoalObject,
//     isQuickPathAvailable)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's paths (`p1w13-sonnet`, layer 10, batch A6): the requests a state makes (`requestPath`, `requestAttackPath`,
//! `requestApproachPath`, `requestSafePath`), the pathfinder's call back when the AI's turn in its queue comes (`doPathfind`),
//! and the path computations (`computeQuickPath` for what flies, `computePath`, `computeAttackPath`), with `setPathFromWaypoint`.
//!
//! As the commands, these run on the lent AI ([`crate::logic::ai::ai_states::ai`]) and its machine. The pathfinder's searches
//! (`PathfindServicesInterface`: `findPath`, `findClosestPath`, `patchPath`, `findSafePath`, `findAttackPath`, and the
//! pathfinder's `getAircraftPath`, `adjustDestination`, `adjustToPossibleDestination`, `moveAllies...`) are the host's: they are
//! translated (`logic::ai::{search, attack, adjust, queue}`) and the World wires them; the weapon's range tests are the host's.
//!
//! # Upstream facts kept
//!
//! - A request within three frames of the last path ("can cause a spin") waits a second (two for the attack, approach and safe
//!   paths) instead of queueing; `m_pathTimestamp > frame - 3` is unsigned, so in the first three frames every request waits.
//! - `doPathfind`'s approach test is `m_isApproachPath & !isDoingGroundMovement()` (a bitwise and: both sides evaluated).
//! - `computePath` keeps a blocked-and-stuck AI's path while it patches it; with no new path it stops a second and pops to its
//!   cell; it always resets the blocked state and stamps the frame.
//! - `computeAttackPath` returns at once (keeping the path) when re-asked within two frames while blocked and stuck.

use crate::common::base_type::Coord3D;
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::{ObjectId, LAYER_GROUND, INVALID_ID};
use crate::common::kind_of::{KINDOF_AIRCRAFT, KINDOF_BRIDGE, KINDOF_NO_COLLIDE, KINDOF_PROJECTILE};
use crate::common::state_machine::{StateContext, StateMachine};
use crate::logic::ai::ai_commands::set_goal_position_clipped;
use crate::logic::ai::ai_states::{ai, with_ai, AiState, AiStateHost};
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::path::{Path, PATHFIND_CELL_SIZE_F};
use crate::logic::locomotor::LOCOMOTORSURFACE_AIR;
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

fn frame<H: AiStateHost>(h: &H) -> u32 {
    StateContext::frame(h)
}

/// `m_pathTimestamp > TheGameLogic->getFrame() - 3` (unsigned: true in the first three frames).
fn repathing_too_soon<H: AiStateHost>(h: &mut H) -> bool {
    let now = frame(h);
    ai(h).path_timestamp > now.wrapping_sub(3)
}

/// `requestPath(destination, isFinalGoal)`: a quick path at once if the AI flies, else a place in the pathfinder's queue.
pub fn request_path<H: AiStateHost>(h: &mut H, destination: &Coord3D, is_final_goal: bool) {
    // (an immobile unit: DEBUG_CRASH("Attempting to path immobile unit."))
    let a = ai(h);
    a.requested_destination = *destination;
    a.is_final_goal = is_final_goal;
    a.is_attack_path = false;
    a.requested_victim_id = INVALID_ID;
    a.is_approach_path = false;
    a.is_safe_path = false;
    if can_compute_quick_path(h) {
        compute_quick_path(h, destination);
        return;
    }
    ai(h).waiting_for_path = true;
    if repathing_too_soon(h) && !plant(3260) {
        // "Requesting path very quickly. Can cause a spin."
        with_ai(h, |a, h| a.set_queue_for_path_time(h, LOGICFRAMES_PER_SECOND));
        if ai(h).path.is_some() && ai(h).is_blocked_and_stuck {
            let now = frame(h);
            let a = ai(h);
            a.set_ignore_collision_time(now, 2 * LOGICFRAMES_PER_SECOND);
            a.blocked_frames = 0;
            a.is_blocked = false;
            a.is_blocked_and_stuck = false;
        }
        return;
    }
    h.pf_queue_for_path();
}

/// `requestAttackPath(victimID, victimPos)`.
pub fn request_attack_path<H: AiStateHost>(h: &mut H, victim_id: ObjectId, victim_pos: &Coord3D) {
    let a = ai(h);
    a.requested_destination = *victim_pos;
    a.requested_victim_id = victim_id;
    a.is_attack_path = true;
    a.is_approach_path = false;
    a.is_safe_path = false;
    a.waiting_for_path = true;
    if repathing_too_soon(h) {
        with_ai(h, |a, h| a.set_queue_for_path_time(h, 2 * LOGICFRAMES_PER_SECOND));
        ai(h).set_locomotor_goal_none();
        return;
    }
    h.pf_queue_for_path();
}

/// `requestApproachPath(destination)`.
pub fn request_approach_path<H: AiStateHost>(h: &mut H, destination: &Coord3D) {
    let a = ai(h);
    a.requested_destination = *destination;
    a.is_final_goal = true;
    a.is_attack_path = false;
    a.requested_victim_id = INVALID_ID;
    a.is_approach_path = true;
    a.is_safe_path = false;
    a.waiting_for_path = true;
    if repathing_too_soon(h) {
        with_ai(h, |a, h| a.set_queue_for_path_time(h, 2 * LOGICFRAMES_PER_SECOND));
        return;
    }
    h.pf_queue_for_path();
}

/// `requestSafePath(repulsor)`: away from it (and the previous one).
pub fn request_safe_path<H: AiStateHost>(h: &mut H, repulsor: ObjectId) {
    let a = ai(h);
    if repulsor != a.repulsor1 && !plant(3261) {
        // "save the prior repulsor."
        a.repulsor2 = a.repulsor1;
    }
    a.repulsor1 = repulsor;
    a.is_final_goal = false;
    a.is_attack_path = false;
    a.requested_victim_id = INVALID_ID;
    a.is_approach_path = false;
    a.is_safe_path = true;
    a.waiting_for_path = true;
    if repathing_too_soon(h) {
        with_ai(h, |a, h| a.set_queue_for_path_time(h, 2 * LOGICFRAMES_PER_SECOND));
        return;
    }
    h.pf_queue_for_path();
}

/// `WAYPOINT_PATH_LIMIT`.
pub const WAYPOINT_PATH_LIMIT: i32 = 1024;

/// `setPathFromWaypoint(way, offset)`: from the object along the waypoints' first links (offset; the last snapped), already
/// optimized.
pub fn set_path_from_waypoint<H: AiStateHost>(h: &mut H, way: Option<u32>, offset_x: f32, offset_y: f32) {
    ai(h).destroy_path();
    let mut path = Path::new();
    let pos = h.position();
    path.prepend_node(&pos, LAYER_GROUND);
    path.mark_optimized();
    let mut count = 0;
    let mut way = way;
    while let Some(w) = way {
        let mut way_pos = h.waypoint_location(w);
        way_pos.x += offset_x;
        way_pos.y += offset_y;
        let next = h.waypoint_link0(w);
        if next.is_none() {
            h.pf_snap_position(&mut way_pos);
        }
        path.append_node(&way_pos, LAYER_GROUND);
        way = next;
        count += 1;
        if count > WAYPOINT_PATH_LIMIT {
            break;
        }
    }
    let a = ai(h);
    a.path = Some(path);
    a.waiting_for_path = false;
    // (SLEEPY_AI) "if we're no longer waiting for a path, make sure we wake up right away!"
    with_ai(h, |a, h| a.wake_up_now(h));
}

/// `canComputeQuickPath()`: "if a unit is moving through the air, we can quick path".
pub fn can_compute_quick_path<H: AiStateHost>(h: &mut H) -> bool {
    let land_bound = ai(h).locomotor_set.get_valid_surfaces() & LOCOMOTORSURFACE_AIR == 0;
    if land_bound {
        return false;
    }
    !with_ai(h, |a, h| a.is_doing_ground_movement(h))
}

/// `computeQuickPath(destination)`: the start and the destination (an aircraft's from the pathfinder), unless the path already
/// ends there.
pub fn compute_quick_path<H: AiStateHost>(h: &mut H, destination: &Coord3D) -> bool {
    // "First, see if our path already goes to the destination."
    if let Some(path) = &ai(h).path {
        if let Some(close) = path.last_node() {
            if path.node(close).next_optimized().0.is_none() {
                let p = path.node(close).position();
                let mut dx = destination.x - p.x;
                dx *= dx;
                let mut dy = destination.y - p.y;
                dy *= dy;
                let mut dz = destination.z - p.z;
                dz *= dz;
                if dx + dy + dz < 0.25 {
                    return true;
                }
            }
        }
    }
    ai(h).destroy_path();
    let new_path = if h.is_kind_of(KINDOF_AIRCRAFT) && !h.is_kind_of(KINDOF_PROJECTILE) {
        h.pf_get_aircraft_path(destination)
    } else {
        let mut path = Path::new();
        path.prepend_node(destination, LAYER_GROUND);
        let mut pos = h.position();
        pos.z = destination.z;
        let layer = h.layer();
        path.prepend_node(&pos, layer);
        let first = path.first_node().expect("the start");
        let next = path.node(first).next();
        path.set_next_optimized(first, next);
        Some(path)
    };
    let now = frame(h);
    let a = ai(h);
    a.path = new_path;
    // "timestamp when the path was created"
    a.path_timestamp = now;
    a.blocked_frames = 0;
    a.is_blocked_and_stuck = false;
    true
}

/// `computePath(pathServices, destination)`: a quick path where it will do, else the pathfinder's (patched when stuck, else
/// found, else the closest), with the allies moved out of the way.
pub fn compute_path<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, destination: &Coord3D) -> bool {
    if !ai(h).is_blocked_and_stuck {
        ai(h).destroy_path();
    }
    if can_compute_quick_path(h) {
        return compute_quick_path(h, destination);
    }
    ai(h).retry_path = false;
    let (lo, hi) = h.terrain_max_pathfind_extent();
    let in_region = |p: &Coord3D| lo.x < p.x && p.x < hi.x && lo.y < p.y && p.y < hi.y;
    if !in_region(destination) {
        // "We're going off the map."
        let pos = h.position();
        if !in_region(&pos) {
            // "We're starting off the map. Since we're off the map, we can't pathfind so just build a path."
            return compute_quick_path(h, destination);
        }
    }
    // "Special case of exit factory."
    if sm.get_current_state_id() == AI_FOLLOW_EXITPRODUCTION_PATH && ai(h).can_path_through_units {
        let ok = compute_quick_path(h, destination);
        if ok {
            h.pf_move_allies_away_from_destination(destination);
            ai(h).set_can_path_through_units(false);
            set_goal_position_clipped(sm, h, Some(destination), CMD_FROM_AI);
            return ok;
        }
    }
    let ignored = ai(h).get_ignored_obstacle_id();
    h.pf_set_ignore_obstacle_id(ignored);
    let original_destination = *destination;
    let surfaces = ai(h).locomotor_set.get_valid_surfaces();
    let id = h.object_id();
    let layer = h.layer();
    let pos = h.position();
    if !ai(h).is_final_goal && h.is_line_passable(id, surfaces, layer, &pos, &original_destination, false, true) {
        return compute_quick_path(h, destination);
    }
    let destination_layer = h.layer_for_destination(destination);
    let crusher = h.crusher_level() > 0;
    let mut new_path = if !h.pf_valid_movement_position(crusher, destination_layer, surfaces, destination) {
        None
    } else if ai(h).is_blocked_and_stuck {
        // "compute a ground-based path"
        let stuck = ai(h).is_blocked_and_stuck;
        let current = ai(h).path.take();
        let patched = h.pf_patch_path(current.as_ref(), stuck);
        ai(h).path = current;
        patched
    } else {
        h.pf_find_path(destination)
    };
    if new_path.is_none() && ai(h).path.is_none() {
        let stuck = ai(h).is_blocked_and_stuck;
        new_path = h.pf_find_closest_path(destination, stuck, 0.0, false);
        ai(h).retry_path = true;
    }
    h.pf_set_ignore_obstacle_id(INVALID_ID);
    if let Some(mut path) = new_path {
        ai(h).destroy_path();
        if ai(h).get_cur_locomotor().is_some_and(|l| l.is_ultra_accurate()) && !plant(3262) {
            // "Move exactly to the destination. Normal ground pathfinding moves to a gridded location."
            path.update_last_node(h, &original_destination);
        }
        let a = ai(h);
        a.path = Some(path);
        a.set_locomotor_goal_position_on_path();
        if !h.is_kind_of(KINDOF_NO_COLLIDE) {
            let path = ai(h).path.take().expect("the path");
            h.pf_move_allies(&path);
            ai(h).path = Some(path);
        }
    } else if ai(h).path.is_some() && ai(h).is_blocked_and_stuck {
        // "Keep using the old path." ... "Stop and wait one second."
        ai(h).destroy_path();
        with_ai(h, |a, h| a.set_queue_for_path_time(h, LOGICFRAMES_PER_SECOND));
        let mut goal_pos = h.position();
        h.pf_snap_position(&mut goal_pos);
        let a = ai(h);
        a.set_final_position(&goal_pos);
        a.set_locomotor_goal_none();
        a.blocked_frames = 0;
        a.is_blocked = false;
        a.is_blocked_and_stuck = false;
    }
    let now = frame(h);
    let a = ai(h);
    a.path_timestamp = now;
    a.blocked_frames = 0;
    a.is_blocked_and_stuck = false;
    a.path.is_some()
}

/// `computeAttackPath(pathServices, victim, victimPos)`: none needed in range with a clear view; a contact weapon approaches
/// the position; a flyer flies straight at the approach point; else the pathfinder's attack path.
pub fn compute_attack_path<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, victim: ObjectId, victim_pos: Option<&Coord3D>) -> bool {
    let now = frame(h);
    // "See if it has been too soon."
    if ai(h).path_timestamp >= now.wrapping_sub(2) && ai(h).path.is_some() && ai(h).is_blocked_and_stuck {
        let a = ai(h);
        a.set_ignore_collision_time(now, 2 * LOGICFRAMES_PER_SECOND);
        a.blocked_frames = 0;
        a.is_blocked = false;
        a.is_blocked_and_stuck = false;
        return true;
    }
    let land_bound = ai(h).locomotor_set.get_valid_surfaces() & LOCOMOTORSURFACE_AIR == 0;
    let victim = if h.object_exists(victim) { victim } else { INVALID_ID };
    if victim == INVALID_ID && victim_pos.is_none() {
        return false;
    }
    let victim_layer = if victim != INVALID_ID { h.object_layer(victim) } else { LAYER_GROUND };
    if h.current_weapon_anti_mask().is_none() {
        // DEBUG_CRASH(("no weapon in AIUpdateInterface::computeAttackPath"))
        return false;
    }
    // "is our weapon within attack range? if so, just return TRUE with no path."
    if victim != INVALID_ID {
        if h.weapon_is_within_attack_range_of_object(victim) {
            let mut view_blocked = false;
            if with_ai(h, |a, h| a.is_doing_ground_movement(h)) && !h.object_is_significantly_above_terrain(victim) {
                let vpos = h.find_object(victim).expect("the victim").position;
                view_blocked = h.pf_is_attack_view_blocked(victim, &vpos);
            }
            if !view_blocked {
                ai(h).destroy_path();
                return true;
            }
        }
    } else if let Some(vp) = victim_pos {
        if h.weapon_is_within_attack_range_of_position(vp) {
            let mut view_blocked = false;
            if with_ai(h, |a, h| a.is_doing_ground_movement(h)) {
                view_blocked = h.pf_is_attack_view_blocked(INVALID_ID, vp);
            }
            if !view_blocked {
                ai(h).destroy_path();
                return true;
            }
        }
    }
    // "Contact weapon"
    if h.weapon_is_contact() {
        let vp = *victim_pos.expect("the victim's position (upstream dereferences it)");
        let tmp = vp;
        ai(h).destroy_path();
        let a = ai(h);
        if let Some(cur) = a.cur_locomotor {
            a.locomotor_set.get_mut(cur).expect("the locomotor").set_no_slow_down_as_approaching_dest(true);
        }
        let ok = compute_path(sm, h, &tmp);
        let Some(path) = &ai(h).path else { return false };
        let last = *path.node(path.last_node().expect("a node")).position();
        let (dx, dy) = (vp.x - last.x, vp.y - last.y);
        if dx * dx + dy * dy < (PATHFIND_CELL_SIZE_F * 3.0) * (PATHFIND_CELL_SIZE_F * 3.0) {
            // "jam in the coordinates of the target."
            let mut path = ai(h).path.take().expect("the path");
            path.update_last_node(h, &vp);
            ai(h).path = Some(path);
        }
        let path = ai(h).path.as_ref().expect("the path");
        let last = *path.node(path.last_node().expect("a node")).position();
        let pos = h.position();
        let (dx, dy) = (pos.x - last.x, pos.y - last.y);
        if dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
            // "Very short path - we can't get to the goal."
            ai(h).destroy_path();
            return false;
        }
        return ok;
    }

    let mut local_victim_pos = if victim != INVALID_ID {
        if h.object_is_kind_of(victim, KINDOF_BRIDGE) {
            let (p1, p2) = h.bridge_attack_points(victim);
            let d1 = h.distance_squared_bounding_sphere_3d(&p1);
            let d2 = h.distance_squared_bounding_sphere_3d(&p2);
            if d2 < d1 { p2 } else { p1 }
        } else {
            h.find_object(victim).expect("the victim").position
        }
    } else {
        *victim_pos.expect("the victim's position")
    };
    local_victim_pos.z = h.layer_height(local_victim_pos.x, local_victim_pos.y, victim_layer);

    if h.is_above_terrain() && !land_bound {
        // "for now, airborne objects don't pathfind"
        local_victim_pos = h.weapon_compute_approach_target(victim, &local_victim_pos);
        // "First, see if our path already goes to the destination."
        if let Some(path) = &ai(h).path {
            let start = path.first_node().expect("a node");
            if let Some(close) = path.node(start).next_optimized().0 {
                if path.node(close).next_optimized().0.is_none() {
                    let p = path.node(close).position();
                    let mut dx = local_victim_pos.x - p.x;
                    dx *= dx;
                    let mut dy = local_victim_pos.y - p.y;
                    dy *= dy;
                    if dx + dy < 0.25 {
                        return true;
                    }
                }
            }
        }
        ai(h).destroy_path();
        let mut path = Path::new();
        path.prepend_node(&local_victim_pos, LAYER_GROUND);
        let mut pos = h.position();
        pos.z = local_victim_pos.z;
        path.prepend_node(&pos, LAYER_GROUND);
        let first = path.first_node().expect("the start");
        let next = path.node(first).next();
        path.set_next_optimized(first, next);
        ai(h).path = Some(path);
    } else {
        ai(h).destroy_path();
        let ignored = ai(h).get_ignored_obstacle_id();
        h.pf_set_ignore_obstacle_id(ignored);
        // "compute a ground-based path"
        ai(h).path = h.pf_find_attack_path(victim, &local_victim_pos);
        if ai(h).path.is_some() {
            let path = ai(h).path.as_ref().expect("the path");
            let mut goal = *path.node(path.last_node().expect("a node")).position();
            if !h.weapon_is_goal_pos_within_attack_range(&goal, victim, &local_victim_pos) {
                // "We didn't actually find a path we can attack from."
                let mut obj_pos = h.position();
                goal.x -= obj_pos.x;
                goal.y -= obj_pos.y;
                goal.z -= obj_pos.z;
                if (goal.x * goal.x + goal.y * goal.y + goal.z * goal.z).sqrt() < 3.0 * PATHFIND_CELL_SIZE_F && !plant(3263) {
                    ai(h).destroy_path();
                    h.pf_adjust_destination(&mut obj_pos);
                    ai(h).path = h.pf_find_closest_path(&obj_pos, false, 0.2, true);
                }
                if ai(h).path.is_none() {
                    return false;
                }
            }
            let path = ai(h).path.as_ref().expect("the path");
            let goal = *path.node(path.last_node().expect("a node")).position();
            let layer = h.layer_for_destination(&goal);
            h.pf_update_goal(&goal, layer);
            if ai(h).path.as_ref().expect("the path").blocked_by_ally() && !h.is_kind_of(KINDOF_NO_COLLIDE) {
                let path = ai(h).path.take().expect("the path");
                h.pf_move_allies(&path);
                ai(h).path = Some(path);
            }
        }
        h.pf_set_ignore_obstacle_id(INVALID_ID);
    }
    let a = ai(h);
    a.path_timestamp = now;
    a.blocked_frames = 0;
    a.is_blocked_and_stuck = false;
    a.path.is_some()
}

/// `doPathfind(pathfinder)`: the AI's turn in the pathfinder's queue: the safe path, the approach path, the attack path (else
/// a path near the victim), or a path to the requested destination.
pub fn do_pathfind<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) {
    if !ai(h).waiting_for_path {
        return;
    }
    ai(h).waiting_for_path = false;
    if ai(h).is_safe_path {
        ai(h).destroy_path();
        let mut pos1 = Coord3D { x: -1000.0, y: -1000.0, z: 0.0 };
        let r1 = ai(h).repulsor1;
        if let Some(o) = h.find_object(r1) {
            pos1 = o.position;
        }
        let mut pos2 = pos1;
        let r2 = ai(h).repulsor2;
        if let Some(o) = h.find_object(r2) {
            pos2 = o.position;
        }
        let range = h.object_vision_range() + h.ai_data_repulsed_distance();
        ai(h).path = h.pf_find_safe_path(&pos1, &pos2, range);
        return;
    }
    // (a bitwise and: isDoingGroundMovement is asked either way)
    let ground = with_ai(h, |a, h| a.is_doing_ground_movement(h));
    if ai(h).is_approach_path & !ground {
        ai(h).is_approach_path = false;
    }
    if ai(h).is_approach_path {
        ai(h).destroy_path();
        let (dest, stuck) = (ai(h).requested_destination, ai(h).is_blocked_and_stuck);
        ai(h).path = h.pf_find_closest_path(&dest, stuck, 0.2, false);
        if with_ai(h, |a, h| a.is_doing_ground_movement(h)) {
            update_goal_to_path_end(h);
        }
        return;
    }
    if ai(h).is_attack_path {
        let req = ai(h).requested_victim_id;
        let victim = if req != INVALID_ID && h.object_exists(req) { req } else { INVALID_ID };
        let dest = ai(h).requested_destination;
        if compute_attack_path(sm, h, victim, Some(&dest)) {
            update_goal_to_path_end(h);
            ai(h).is_attack_path = true;
            return;
        }
        ai(h).is_attack_path = false;
        if victim != INVALID_ID {
            let mut dest = h.find_object(victim).expect("the victim").position;
            // "find a pathable destination near the victim."
            h.pf_adjust_to_possible_destination(&mut dest);
            let a = ai(h);
            a.requested_destination = dest;
            a.ignore_obstacle(victim);
        }
    }
    let dest = ai(h).requested_destination;
    compute_path(sm, h, &dest);
    if ai(h).is_final_goal && with_ai(h, |a, h| a.is_doing_ground_movement(h)) {
        update_goal_to_path_end(h);
    }
    if ai(h).queue_for_path_frame > frame(h) && !plant(3264) {
        ai(h).waiting_for_path = true;
    }
    // (SLEEPY_AI) "if we're no longer waiting for a path, make sure we wake up right away!"
    if !ai(h).waiting_for_path {
        with_ai(h, |a, h| a.wake_up_now(h));
    }
}

/// `TheAI->pathfinder()->updateGoal(obj, last node's position, its layer)`, if there is a path.
fn update_goal_to_path_end<H: AiStateHost>(h: &mut H) {
    let Some(path) = &ai(h).path else { return };
    let last = path.node(path.last_node().expect("a node"));
    let (pos, layer) = (*last.position(), last.layer());
    h.pf_update_goal(&pos, layer);
}

/// `friend_setGoalObject(obj)`: the goal object even through a lock.
pub fn friend_set_goal_object<H: AiStateHost>(sm: &mut Sm<H>, h: &H, obj: ObjectId) {
    let locked = sm.is_locked();
    sm.unlock();
    let o = h.find_object(obj);
    sm.set_goal_object(o.as_ref());
    if locked {
        sm.lock();
    }
}

/// `isQuickPathAvailable(destination)`.
pub fn is_quick_path_available<H: AiStateHost>(h: &mut H, destination: &Coord3D) -> bool {
    h.pf_quick_path_exists_for_ui(destination)
}
