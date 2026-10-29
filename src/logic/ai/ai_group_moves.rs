// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGroup.cpp (lines 476-2036: the group's moves)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Line2D.cpp (`Coord3DInsideRect2D`, `ScaleRect2D`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `AIGroup`'s moves (`p1w13-sonnet`, layer 10, batch A25): to a position (in columns along a ground path for infantry and
//! vehicles, in formation, or each to its own offset from the group's), scatter, tighten, and the waypoint and evacuate moves.
//!
//! The ground path the columns follow is [`AiGroup::ground_path`]: the positions of its nodes in their optimised order (upstream's
//! `Path`, walked by `getFirstNode`, `getNextOptimized` and `getLastNode`; the pathfinder's search is the host's,
//! [`AiGroupMoveHost::pf_find_ground_path`]). `recompute` clears it.
//!
//! # Upstream facts kept
//!
//! - `friend_computeGroundPath` may return early and leave the former ground path in place; it replaces one without freeing it.
//! - `groupMoveToPosition`'s tighten test measures its cells as `dx * dy` with `dy` taken from the x extent too.
//! - In the vehicles' columns the move-priority filter is compiled out, so the evening-out pass runs three times over every unit.
//! - The formation move gives every member not held a command through its AI: a member without one is undefined upstream (a
//!   null AI), which the callers here must not ask for.

use crate::common::base_type::{Coord2D, Coord3D, Region3D};
use crate::common::game_common::PlayerType;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{
    KINDOF_AIRCRAFT, KINDOF_CLIFF_JUMPER, KINDOF_IMMOBILE, KINDOF_INFANTRY, KINDOF_MOB_NEXUS, KINDOF_PRODUCED_AT_HELIPAD, KINDOF_VEHICLE,
};
use crate::common::object_status_types::{OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_DETECTED, OBJECT_STATUS_STEALTHED};
use crate::logic::ai::ai_group::{AiGroup, AiGroupHost, GroupAiCommand, NO_FORMATION_ID};
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::modules::plant;
use crate::logic::object_iter::{IterOrderType, SimpleObjectIterator};

/// `PATH_DIAMETER_IN_CELLS`.
pub const PATH_DIAMETER_IN_CELLS: i32 = 6;
/// `STD_WAYPOINT_CLAMP_MARGIN`, `STD_AIRCRAFT_EXTRA_MARGIN` (`const Int`s of `Real` products).
const STD_WAYPOINT_CLAMP_MARGIN: i32 = (PATHFIND_CELL_SIZE_F * 4.0) as i32;
const STD_AIRCRAFT_EXTRA_MARGIN: i32 = (PATHFIND_CELL_SIZE_F * 10.0) as i32;
/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: i32 = 30;
/// `LocomotorPriority`'s values (`LOCO_MOVES_BACK` .. `LOCO_MOVES_FRONT`).
const LOCO_MOVES_BACK: i32 = 0;
const LOCO_MOVES_MIDDLE: i32 = 1;
const LOCO_MOVES_FRONT: i32 = 2;
/// `CIRCLE` (`2 * PI`, in `Real`).
const CIRCLE: f32 = 2.0 * std::f32::consts::PI;

/// What the moves ask of the members' world beyond [`AiGroupHost`].
pub trait AiGroupMoveHost: AiGroupHost {
    /// `TheGlobalData->m_debugAI == AI_DEBUG_TERRAIN`; `m_groupMoveClickToGatherFactor`.
    fn debug_ai_terrain(&mut self) -> bool;
    fn group_move_click_to_gather_factor(&mut self) -> f32;
    /// `TheAI->getAiData()`'s `m_distanceRequiresGroup`, `m_minDistanceForGroup`, `m_minInfantryForGroup`, `m_minVehiclesForGroup`.
    fn ai_distance_requires_group(&mut self) -> f32;
    fn ai_min_distance_for_group(&mut self) -> f32;
    fn ai_min_infantry_for_group(&mut self) -> i32;
    fn ai_min_vehicles_for_group(&mut self) -> i32;
    // ---- the member
    fn formation_offset(&mut self, o: ObjectId) -> Coord2D;
    /// `getGeometryInfo().getBoundingCircleRadius()`, `getMajorRadius()`.
    fn bounding_circle_radius(&mut self, o: ObjectId) -> f32;
    fn major_radius(&mut self, o: ObjectId) -> f32;
    /// `getControllingPlayer()->getPlayerType()` (`None`: no player).
    fn player_type(&mut self, o: ObjectId) -> Option<PlayerType>;
    // ---- the member's AI
    fn ai_is_doing_ground_movement(&mut self, o: ObjectId) -> bool;
    fn ai_tmp_value(&mut self, o: ObjectId) -> i32;
    fn ai_set_tmp_value(&mut self, o: ObjectId, v: i32);
    /// `getCurLocomotor()->getMovePriority()` (`None`: no current locomotor).
    fn ai_cur_locomotor_priority(&mut self, o: ObjectId) -> Option<i32>;
    /// `aiFollowPath(&path, NULL, cmdSource)`.
    fn ai_follow_path(&mut self, o: ObjectId, path: &[Coord3D], src: CommandSourceType);
    // ---- the pathfinder
    fn pf_remove_goal(&mut self, o: ObjectId);
    /// `adjustDestination(obj, ai->getLocomotorSet(), dest, groupDest)` (its answer unused here).
    fn pf_adjust_destination(&mut self, o: ObjectId, dest: &mut Coord3D, group_dest: Option<&Coord3D>);
    /// `updateGoal(obj, dest, LAYER_GROUND)`; `updatePos(obj, pos)`.
    fn pf_update_goal(&mut self, o: ObjectId, dest: &Coord3D);
    fn pf_update_pos(&mut self, o: ObjectId, pos: &Coord3D);
    /// `isLinePassable(obj, its locomotor set's surfaces, obj->getLayer(), from, to, false, true)`.
    fn pf_is_line_passable(&mut self, o: ObjectId, from: &Coord3D, to: &Coord3D) -> bool;
    /// `findGroundPath(from, to, PATH_DIAMETER_IN_CELLS, false)`: its nodes' positions in their optimised order (`None`: no path).
    fn pf_find_ground_path(&mut self, from: &Coord3D, to: &Coord3D) -> Option<Vec<Coord3D>>;
    // ---- the terrain
    fn layer_for_destination(&mut self, pos: &Coord3D) -> i32;
    fn ground_height(&mut self, x: f32, y: f32) -> f32;
    /// `getExtent(&extent)`; `getMaximumPathfindExtent(&extent)`.
    fn map_extent(&mut self) -> Region3D;
    fn max_pathfind_extent(&mut self) -> Region3D;
}

/// `MAX(a, b)` (WWLib always.h): `a > b ? a : b`.
fn max_of(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

fn sqr(x: f32) -> f32 {
    x * x
}

/// `Coord3DInsideRect2D(point, tl, br)` (Line2D.cpp).
pub fn coord3d_inside_rect_2d(p: &Coord3D, tl: &Coord2D, br: &Coord2D) -> bool {
    p.x >= tl.x && p.x <= br.x && p.y >= tl.y && p.y <= br.y
}

/// `ScaleRect2D(tl, br, scaleFactor)` (Line2D.cpp).
pub fn scale_rect_2d(tl: &mut Coord2D, br: &mut Coord2D, scale_factor: f32) {
    let scale_factor = scale_factor - 1.0;
    let delta_width = (br.x - tl.x) * scale_factor * 0.5;
    let delta_height = (br.y - tl.y) * scale_factor * 0.5;
    tl.x -= delta_width;
    tl.y -= delta_height;
    br.x += delta_width;
    br.y += delta_height;
}

/// `clampToMap(dest, pt)`: inside the computer's pathable extent or the human's visible map, less a cell each side.
fn clamp_to_map<H: AiGroupMoveHost>(h: &mut H, dest: &mut Coord3D, pt: PlayerType) {
    let mut extent = if pt == PlayerType::Computer { h.max_pathfind_extent() } else { h.map_extent() };
    extent.hi.x -= PATHFIND_CELL_SIZE_F;
    extent.hi.y -= PATHFIND_CELL_SIZE_F;
    extent.lo.x += PATHFIND_CELL_SIZE_F;
    extent.lo.y += PATHFIND_CELL_SIZE_F;
    if !extent.is_in_region_no_z(dest) {
        if dest.x < extent.lo.x {
            dest.x = extent.lo.x;
        }
        if dest.y < extent.lo.y {
            dest.y = extent.lo.y;
        }
        if dest.x > extent.hi.x {
            dest.x = extent.hi.x;
        }
        if dest.y > extent.hi.y {
            dest.y = extent.hi.y;
        }
    }
}

/// `clampWaypointPosition(position, margin)`: inside the map less the margin (then on the ground).
pub fn clamp_waypoint_position<H: AiGroupMoveHost>(h: &mut H, position: &mut Coord3D, margin: i32) {
    let mut map_extent = h.map_extent();
    let m = margin as f32;
    map_extent.hi.x -= m;
    map_extent.hi.y -= m;
    map_extent.lo.x += m;
    map_extent.lo.y += m;
    if !map_extent.is_in_region_no_z(position) {
        if position.x > map_extent.hi.x {
            position.x = map_extent.hi.x;
        } else if position.x < map_extent.lo.x {
            position.x = map_extent.lo.x;
        }
        if position.y > map_extent.hi.y {
            position.y = map_extent.hi.y;
        } else if position.y < map_extent.lo.y {
            position.y = map_extent.lo.y;
        }
        position.z = h.ground_height(position.x, position.y);
    }
}

/// `getHelicopterOffset(posOut, idx)`: the idx-th helicopter's spot on rings of 70 around the position (`sin`, `cos` in `double`).
pub fn get_helicopter_offset(pos_out: &mut Coord3D, idx: i32) {
    if idx == 0 {
        return;
    }
    let assumed_heli_diameter = 70.0f32;
    let mut radius = assumed_heli_diameter;
    let mut circumference = radius * CIRCLE;
    let mut angle = 0.0f32;
    let mut angle_between_each_chopper = assumed_heli_diameter / circumference * CIRCLE;
    for _ in 1..idx {
        angle += angle_between_each_chopper;
        if angle > CIRCLE {
            radius += assumed_heli_diameter;
            circumference = radius * CIRCLE;
            angle_between_each_chopper = assumed_heli_diameter / circumference * CIRCLE;
            angle -= CIRCLE;
        }
    }
    let ctr = *pos_out;
    use petir::Real;
    pos_out.x = (f64::from(ctr.x) + Real::sin(f64::from(angle)) * f64::from(radius)) as f32;
    pos_out.y = (f64::from(ctr.y) + Real::cos(f64::from(angle)) * f64::from(radius)) as f32;
}

/// The squared distance in x and y.
fn d2(a: &Coord3D, b: &Coord3D) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

/// `(fiveColumnDelta << 16) | (columnDelta & 0xffff)` and back.
fn pack(hi: i32, lo: i32) -> i32 {
    (hi << 16) | (lo & 0xffff)
}
fn unpack(tmp: i32) -> (i32, i32) {
    (tmp >> 16, (tmp & 0xffff) as i16 as i32)
}

/// The start node (the first one farther than `far` from the first), the end node (the last one farther than `far` from the
/// last) of a ground path.
fn path_start_end(gp: &[Coord3D], far: f32) -> (Option<usize>, Option<usize>) {
    let start_point = gp[0];
    let start = (0..gp.len()).find(|&i| d2(&gp[i], &start_point) > far);
    let end_point = gp[gp.len() - 1];
    let end = (0..gp.len()).rev().find(|&i| d2(&gp[i], &end_point) > far);
    (start, end)
}

/// A unit's column path along the ground path (the infantry's and the vehicles' shared walk): from the start node, each node
/// with a farther one after it, moved `offset * columnDelta` and half a cell along the corner's normal, clamped, kept when it
/// goes the corner's way.
#[allow(clippy::too_many_arguments)]
fn column_path<H: AiGroupMoveHost>(
    h: &mut H,
    gp: &[Coord3D],
    start_node: usize,
    far: f32,
    unit_pos: Coord3D,
    offset: f32,
    column_delta: i32,
    factor: i32,
    pt: PlayerType,
) -> Vec<Coord3D> {
    let mut path = Vec::new();
    let mut node = Some(start_node);
    let mut previous = 0usize;
    let mut prev_pos = unit_pos;
    while let Some(nd) = node {
        let mut dest = gp[nd];
        let Some(next) = (nd + 1..gp.len()).find(|&t| d2(&gp[t], &dest) > far) else { break };
        let mut cvn = Coord2D { x: -(gp[next].y - gp[previous].y), y: gp[next].x - gp[previous].x };
        cvn.normalize();
        let cv = Coord2D { x: gp[next].x - gp[previous].x, y: gp[next].y - gp[previous].y };
        dest.x += offset * column_delta as f32 * cvn.x;
        dest.y += offset * column_delta as f32 * cvn.y;
        if factor & 1 != 0 {
            dest.x += 0.5 * PATHFIND_CELL_SIZE_F * cvn.x;
            dest.y += 0.5 * PATHFIND_CELL_SIZE_F * cvn.y;
        } else {
            dest.x -= 0.5 * PATHFIND_CELL_SIZE_F * cvn.x;
            dest.y -= 0.5 * PATHFIND_CELL_SIZE_F * cvn.y;
        }
        let cur = Coord2D { x: dest.x - prev_pos.x, y: dest.y - prev_pos.y };
        clamp_to_map(h, &mut dest, pt);
        if cv.x * cur.x + cv.y * cur.y > 0.0 {
            path.push(dest);
            prev_pos = dest;
        }
        // (a node with a farther one after it is not the last)
        let n2 = nd + 1;
        node = Some(n2);
        let mut t = previous + 1;
        while t < gp.len() && t != n2 {
            if d2(&gp[t], &gp[n2]) > far {
                previous = t;
            }
            t += 1;
        }
    }
    path
}

/// The final destination's tail: the path's last points that do not lead the end vector's way dropped, the destination clamped,
/// adjusted and the goal set, then the path given.
fn finish_column<H: AiGroupMoveHost>(h: &mut H, unit: ObjectId, mut path: Vec<Coord3D>, mut dest: Coord3D, end_vector: Coord2D, pt: PlayerType, src: CommandSourceType) {
    while let Some(prev) = path.last() {
        let cur = Coord2D { x: dest.x - prev.x, y: dest.y - prev.y };
        if end_vector.x * cur.x + end_vector.y * cur.y <= 0.0 {
            path.pop();
        } else {
            break;
        }
    }
    clamp_to_map(h, &mut dest, pt);
    h.pf_adjust_destination(unit, &mut dest, None);
    h.pf_update_goal(unit, &dest);
    path.push(dest);
    h.ai_follow_path(unit, &path, src);
}

/// The start and end vectors and their normals.
fn path_vectors(gp: &[Coord3D], start_node: usize, end_node: usize) -> (Coord2D, Coord2D, Coord2D, Coord2D) {
    let start_point = gp[0];
    let end_point = gp[gp.len() - 1];
    let mut sv = Coord2D { x: gp[start_node].x - start_point.x, y: gp[start_node].y - start_point.y };
    sv.normalize();
    let mut ev = Coord2D { x: end_point.x - gp[end_node].x, y: end_point.y - gp[end_node].y };
    ev.normalize();
    let mut svn = Coord2D { x: -sv.y, y: sv.x };
    svn.normalize();
    let mut evn = Coord2D { x: -ev.y, y: ev.x };
    evn.normalize();
    (sv, ev, svn, evn)
}

/// The units sorted again along the end vector's normal (`useEndVector`): each moved through a second iterator, keyed again.
fn resort_on_end_normal<H: AiGroupMoveHost>(h: &mut H, iter: &mut SimpleObjectIterator<ObjectId>, center: &Coord3D, svn: Coord2D) {
    let mut iter2 = SimpleObjectIterator::new();
    let mut u = iter.first();
    while let Some(unit) = u {
        let _ = iter2.insert(Some(unit), 0.0);
        u = iter.next();
    }
    iter.make_empty();
    let mut u = iter2.first();
    while let Some(unit) = u {
        let p = h.position(unit);
        let (dx, dy) = (p.x - center.x, p.y - center.y);
        let _ = iter.insert(Some(unit), dx * svn.x + dy * svn.y);
        u = iter2.next();
    }
}

/// The units of an iterator, in its order.
fn units(iter: &mut SimpleObjectIterator<ObjectId>) -> Vec<ObjectId> {
    let mut v = Vec::new();
    let mut u = iter.first();
    while let Some(unit) = u {
        v.push(unit);
        u = iter.next();
    }
    v
}

/// The column with the fewest units nearest `want` (its index; ties to the lower), among `cols` (the indices allowed).
fn even_out(column: &mut [i32], cols: &[usize], want: i32) -> Option<usize> {
    let min = cols.iter().map(|&i| column[i]).fold(10000, i32::min);
    let mut delta = 10000;
    let mut best = None;
    for &i in cols {
        if column[i] == min {
            let dx = (want - i as i32).abs();
            if dx < delta {
                delta = dx;
                best = Some(i);
            }
        }
    }
    if let Some(b) = best {
        column[b] += 1;
    }
    best
}

impl AiGroup {
    /// `computeIndividualDestination(dest, groupDest, obj, center, isFormation)`: the member's offset from the centre (or its
    /// formation offset), at most six bounding radii, at the group's destination; adjusted when it moves on the ground.
    pub fn compute_individual_destination<H: AiGroupMoveHost>(h: &mut H, group_dest: &Coord3D, o: ObjectId, center: &Coord3D, is_formation: bool) -> Coord3D {
        let pos = h.position(o);
        let mut v = if is_formation { h.formation_offset(o) } else { Coord2D { x: pos.x - center.x, y: pos.y - center.y } };
        let mut length = v.length();
        let r = h.bounding_circle_radius(o);
        if length > 6.0 * r {
            length = 6.0 * r;
        }
        v.normalize();
        v.x *= length;
        v.y *= length;
        let layer = h.layer_for_destination(group_dest);
        let mut dest = Coord3D { x: group_dest.x + v.x, y: group_dest.y + v.y, z: 0.0 };
        dest.z = h.layer_height(dest.x, dest.y, layer);
        if h.has_ai(o) && h.ai_is_doing_ground_movement(o) {
            h.pf_adjust_destination(o, &mut dest, if is_formation { None } else { Some(group_dest) });
            h.pf_update_goal(o, &dest);
        }
        dest
    }

    /// `friend_computeGroundPath(pos, cmdSource)`: a ground path from the member nearest the centre to the position, when the
    /// group is spread or large enough (or its infantry all see the centre); whether there is one.
    pub fn friend_compute_ground_path<H: AiGroupMoveHost>(&mut self, h: &mut H, pos: &Coord3D) -> bool {
        if self.dirty {
            self.recompute(h);
        }
        if h.debug_ai_terrain() {
            return false;
        }
        let mut close_enough = false;
        let (min, max, center, _) = self.get_min_max_and_center(h);
        let requires = h.ai_distance_requires_group();
        let mut dist_sqr = 4.0 * sqr(requires);
        let mut num_infantry = 0;
        let mut num_vehicles = 0;
        let mut center_vehicle: Option<ObjectId> = None;
        let mut dist_sqr_center_veh = dist_sqr * 10.0;
        for &o in &self.members.clone() {
            h.pf_remove_goal(o);
            if h.is_held(o) || !h.has_ai(o) {
                continue;
            }
            if h.is_kind_of(o, KINDOF_INFANTRY) {
                num_infantry += 1;
            } else if h.is_kind_of(o, KINDOF_VEHICLE) {
                if h.is_kind_of(o, KINDOF_AIRCRAFT) {
                    continue;
                }
                num_vehicles += 1;
            } else {
                continue;
            }
            let unit_pos = h.position(o);
            let d = d2(&unit_pos, pos);
            if d < dist_sqr {
                dist_sqr = d;
            }
            let d = d2(&unit_pos, &center);
            if center_vehicle.is_none() || d < dist_sqr_center_veh {
                center_vehicle = Some(o);
                dist_sqr_center_veh = d;
            }
        }
        let Some(cv) = center_vehicle else { return false };
        let center = h.position(cv);
        let (dx, dy) = (max.x - min.x, max.y - min.y);
        if dx * dx + dy * dy > sqr(requires) {
            dist_sqr = dx * dx + dy * dy;
        }
        if dist_sqr < sqr(h.ai_min_distance_for_group()) {
            return false;
        }
        if dist_sqr > sqr(requires) {
            close_enough = true;
        }
        if num_infantry > 6 || num_vehicles > 4 {
            close_enough = true;
        }
        if !close_enough {
            // "see if all units have an unobstructed path to the center"
            let mut is_passable = true;
            for &o in &self.members.clone() {
                if !h.is_kind_of(o, KINDOF_INFANTRY) {
                    continue;
                }
                if h.has_ai(o) {
                    let p = h.position(o);
                    if !h.pf_is_line_passable(o, &p, &center) {
                        is_passable = false;
                    }
                }
            }
            if is_passable && !plant(3550) {
                close_enough = true;
            }
        }
        if !close_enough {
            return false;
        }
        self.ground_path = h.pf_find_ground_path(&center, pos);
        self.ground_path.is_some()
    }

    /// `friend_moveInfantryToPos(pos, cmdSource)`: the infantry in three (or five) columns along the ground path; whether they went.
    pub fn friend_move_infantry_to_pos<H: AiGroupMoveHost>(&mut self, h: &mut H, pos: &Coord3D, src: CommandSourceType) -> bool {
        let Some(gp) = self.ground_path.clone() else { return false };
        let num_columns = 3;
        let half_num_columns = num_columns / 2;
        let (center, ok) = self.get_center(h);
        if !ok {
            return false;
        }
        let far = sqr(PATH_DIAMETER_IN_CELLS as f32 * PATHFIND_CELL_SIZE_F);
        let (start_node, end_node) = path_start_end(&gp, far);
        let (Some(start_node), Some(end_node)) = (start_node, end_node) else {
            self.ground_path = None;
            return false;
        };
        let start_point = gp[0];
        let end_point = gp[gp.len() - 1];
        let (mut sv, ev, mut svn, evn) = path_vectors(&gp, start_node, end_node);
        let mut use_end_vector = false;
        let mut units_to_path = 0;
        let mut iter = SimpleObjectIterator::new();
        let mut pt = PlayerType::Computer;
        for &o in &self.members.clone() {
            if h.is_held(o) || !h.is_kind_of(o, KINDOF_INFANTRY) || !h.has_ai(o) {
                continue;
            }
            if h.is_kind_of(o, KINDOF_MOB_NEXUS) {
                return false; // "means I did NOT do a column group pathfind"
            }
            if let Some(t) = h.player_type(o) {
                pt = t;
            }
            let unit_pos = h.position(o);
            h.pf_remove_goal(o);
            let (dx, dy) = (unit_pos.x - center.x, unit_pos.y - center.y);
            let _ = iter.insert(Some(o), dx * svn.x + dy * svn.y);
            units_to_path += 1;
            if d2(&unit_pos, &start_point) > d2(&unit_pos, &end_point) {
                use_end_vector = true;
            }
        }
        if units_to_path < h.ai_min_infantry_for_group() {
            return false;
        }
        if use_end_vector && !plant(3551) {
            sv = ev;
            svn = evn;
            resort_on_end_normal(h, &mut iter, &center, svn);
        }
        iter.sort(IterOrderType::SortedFarToNear, |_| 0);
        let mut iter2 = SimpleObjectIterator::new();
        for (cur_index, unit) in units(&mut iter).into_iter().enumerate() {
            let cur_index = cur_index as i32;
            let divisor = ((units_to_path + 1) / num_columns).max(1);
            let mut column_delta = 1 - cur_index / divisor;
            if column_delta < -half_num_columns {
                column_delta = -half_num_columns;
            }
            let divisor = ((units_to_path + 3) / 5).max(1);
            let mut five_column_delta = 2 - cur_index / divisor;
            if five_column_delta < -2 {
                five_column_delta = -2;
            }
            if units_to_path < 16 {
                five_column_delta = column_delta;
            }
            h.ai_set_tmp_value(unit, pack(five_column_delta, column_delta));
            let p = h.position(unit);
            let (dx, dy) = (p.x - center.x, p.y - center.y);
            let mut adjust = 0i32;
            match h.ai_cur_locomotor_priority(unit) {
                Some(LOCO_MOVES_MIDDLE) => adjust = (-100.0 * PATHFIND_CELL_SIZE_F) as i32,
                Some(LOCO_MOVES_BACK) => adjust = (-200.0 * PATHFIND_CELL_SIZE_F) as i32,
                _ => {}
            }
            let _ = iter2.insert(Some(unit), adjust as f32 + dx * sv.x + dy * sv.y);
        }
        iter2.sort(IterOrderType::SortedFarToNear, |_| 0);
        let order = units(&mut iter2);
        // "Even out columns by priority."
        let mut column3 = [0i32; 3];
        let mut column5 = [0i32; 5];
        for group in (LOCO_MOVES_BACK..=LOCO_MOVES_FRONT).rev() {
            for &unit in &order {
                let tmp = h.ai_tmp_value(unit);
                let move_priority = h.ai_cur_locomotor_priority(unit).unwrap_or(LOCO_MOVES_MIDDLE);
                if group != move_priority {
                    continue;
                }
                let (mut five_column_delta, mut column_delta) = unpack(tmp);
                if let Some(b) = even_out(&mut column3, &[0, 1, 2], 1 + column_delta) {
                    column_delta = b as i32 - 1;
                }
                if let Some(b) = even_out(&mut column5, &[0, 1, 2, 3, 4], 2 + five_column_delta) {
                    five_column_delta = b as i32 - 2;
                }
                if units_to_path < 16 {
                    five_column_delta = column_delta;
                }
                h.ai_set_tmp_value(unit, pack(five_column_delta, column_delta));
            }
        }
        let mut column_factor = [0i32; 5];
        let layer = h.layer_for_destination(pos);
        for &unit in &order {
            let (mut five_column_delta, column_delta) = unpack(h.ai_tmp_value(unit));
            let factor = column_factor[(five_column_delta + 2) as usize];
            column_factor[(five_column_delta + 2) as usize] = factor + 1;
            let unit_pos = h.position(unit);
            let offset = PATHFIND_CELL_SIZE_F * 2.1 / half_num_columns as f32;
            let path = column_path(h, &gp, start_node, far, unit_pos, offset, column_delta, factor, pt);
            let mut dest = *pos;
            five_column_delta = five_column_delta.clamp(-2, 2);
            let offset = PATHFIND_CELL_SIZE_F * 2.2;
            dest.x += offset * five_column_delta as f32 * evn.x;
            dest.y += offset * five_column_delta as f32 * evn.y;
            if factor & 1 != 0 {
                dest.x += PATHFIND_CELL_SIZE_F * evn.x;
                dest.y += PATHFIND_CELL_SIZE_F * evn.y;
            }
            let move_priority = h.ai_cur_locomotor_priority(unit).unwrap_or(LOCO_MOVES_MIDDLE);
            let delta = move_priority - LOCO_MOVES_FRONT;
            dest.x += delta as f32 * PATHFIND_CELL_SIZE_F * ev.x;
            dest.y += delta as f32 * PATHFIND_CELL_SIZE_F * ev.y;
            dest.x -= factor as f32 * offset * ev.x;
            dest.y -= factor as f32 * offset * ev.y;
            dest.z = h.layer_height(dest.x, dest.y, layer);
            finish_column(h, unit, path, dest, ev, pt, src);
        }
        true
    }

    /// `friend_moveFormationToPos(pos, cmdSource)`: each member not held to its formation offset from the end (along the ground
    /// path when it still has a start and an end node, the end after the start).
    pub fn friend_move_formation_to_pos<H: AiGroupMoveHost>(&mut self, h: &mut H, pos: &Coord3D, src: CommandSourceType) {
        let (_, ok) = self.get_center(h);
        if !ok {
            return;
        }
        let mut start_node = None;
        let mut end_node = None;
        let mut end_point = *pos;
        if let Some(gp) = self.ground_path.clone() {
            let far = sqr(PATH_DIAMETER_IN_CELLS as f32 * PATHFIND_CELL_SIZE_F);
            let (s, e) = path_start_end(&gp, far);
            start_node = s;
            end_node = e;
            end_point = gp[gp.len() - 1];
            // (the end node dropped when the start node comes at or after it)
            if let (Some(s), Some(e)) = (s, e) {
                if s >= e && !plant(3552) {
                    end_node = None;
                }
            }
            if start_node.is_none() || end_node.is_none() {
                self.ground_path = None;
                start_node = None;
                end_node = None;
            }
        }
        let gp = self.ground_path.clone();
        for &unit in &self.members.clone() {
            if h.is_held(unit) {
                continue;
            }
            let offset = h.formation_offset(unit);
            if let (Some(s), Some(e), Some(gp)) = (start_node, end_node, gp.as_ref()) {
                let mut path: Vec<Coord3D> = gp[s..=e].iter().map(|p| Coord3D { x: p.x + offset.x, y: p.y + offset.y, z: p.z }).collect();
                let mut dest = Coord3D { x: end_point.x + offset.x, y: end_point.y + offset.y, z: end_point.z };
                h.pf_adjust_destination(unit, &mut dest, None);
                h.pf_update_goal(unit, &dest);
                path.push(dest);
                h.ai_follow_path(unit, &path, src);
            } else {
                let dest = Coord3D { x: end_point.x + offset.x, y: end_point.y + offset.y, z: end_point.z };
                h.ai_command(unit, GroupAiCommand::MoveToPosition { pos: dest }, src);
            }
        }
    }

    /// `friend_moveVehicleToPos(pos, cmdSource)`: the ground-moving vehicles in two (or three) columns along the ground path.
    pub fn friend_move_vehicle_to_pos<H: AiGroupMoveHost>(&mut self, h: &mut H, pos: &Coord3D, src: CommandSourceType) -> bool {
        let Some(gp) = self.ground_path.clone() else { return false };
        let (center, ok) = self.get_center(h);
        if !ok {
            return false;
        }
        let num_columns = 2;
        let far = sqr(PATH_DIAMETER_IN_CELLS as f32 * PATHFIND_CELL_SIZE_F);
        let (start_node, mut end_node) = path_start_end(&gp, far);
        if end_node == Some(0) {
            end_node = None;
        }
        let (Some(start_node), Some(end_node)) = (start_node, end_node) else {
            self.ground_path = None;
            return false;
        };
        let start_point = gp[0];
        let end_point = gp[gp.len() - 1];
        let (mut sv, ev, mut svn, evn) = path_vectors(&gp, start_node, end_node);
        let mut units_to_path = 0;
        let mut use_end_vector = false;
        let mut iter = SimpleObjectIterator::new();
        let mut pt = PlayerType::Computer;
        for &o in &self.members.clone() {
            if h.is_held(o) || !h.is_kind_of(o, KINDOF_VEHICLE) || !h.has_ai(o) || !h.ai_is_doing_ground_movement(o) {
                continue;
            }
            if let Some(t) = h.player_type(o) {
                pt = t;
            }
            let unit_pos = h.position(o);
            h.pf_remove_goal(o);
            let (dx, dy) = (unit_pos.x - center.x, unit_pos.y - center.y);
            let _ = iter.insert(Some(o), dx * svn.x + dy * svn.y);
            units_to_path += 1;
            if d2(&unit_pos, &start_point) > d2(&unit_pos, &end_point) {
                use_end_vector = true;
            }
        }
        if units_to_path < h.ai_min_vehicles_for_group() {
            return false;
        }
        if use_end_vector {
            sv = ev;
            svn = evn;
            resort_on_end_normal(h, &mut iter, &center, svn);
        }
        iter.sort(IterOrderType::SortedFarToNear, |_| 0);
        let mut iter2 = SimpleObjectIterator::new();
        for (cur_index, unit) in units(&mut iter).into_iter().enumerate() {
            let cur_index = cur_index as i32;
            let divisor = ((units_to_path + 1) / num_columns).max(1);
            let mut column_delta = 1 - cur_index / divisor;
            if column_delta == 0 {
                column_delta = -1;
            }
            let divisor = ((units_to_path + 1) / 3).max(1);
            let mut three_column_delta = 1 - cur_index / divisor;
            if three_column_delta < -1 {
                three_column_delta = -1;
            }
            if units_to_path < 5 {
                three_column_delta = column_delta;
            }
            h.ai_set_tmp_value(unit, pack(three_column_delta, column_delta));
            let p = h.position(unit);
            let (dx, dy) = (p.x - center.x, p.y - center.y);
            let adjust = 0i32;
            let _ = iter2.insert(Some(unit), adjust as f32 + dx * sv.x + dy * sv.y);
        }
        iter2.sort(IterOrderType::SortedFarToNear, |_| 0);
        let order = units(&mut iter2);
        // "Even out columns by priority." (the priority filter compiled out: three passes over every unit)
        let mut column2 = [0i32; 3];
        let mut column3 = [0i32; 3];
        for _group in (LOCO_MOVES_BACK..=LOCO_MOVES_FRONT).rev() {
            for &unit in &order {
                let (mut three_column_delta, mut column_delta) = unpack(h.ai_tmp_value(unit));
                if let Some(b) = even_out(&mut column2, &[0, 2], 1 + column_delta) {
                    column_delta = b as i32 - 1;
                }
                if let Some(b) = even_out(&mut column3, &[0, 1, 2], 1 + three_column_delta) {
                    three_column_delta = b as i32 - 1;
                }
                if units_to_path < 5 {
                    three_column_delta = column_delta;
                }
                h.ai_set_tmp_value(unit, pack(three_column_delta, column_delta));
            }
        }
        let mut column_factor = [0i32; 5];
        let layer = h.layer_for_destination(pos);
        for &unit in &order {
            let (mut three_column_delta, column_delta) = unpack(h.ai_tmp_value(unit));
            let factor = column_factor[(three_column_delta + 2) as usize];
            column_factor[(three_column_delta + 2) as usize] = factor + 1;
            let unit_pos = h.position(unit);
            let path = column_path(h, &gp, start_node, far, unit_pos, PATHFIND_CELL_SIZE_F * 1.5, column_delta, factor, pt);
            let mut dest = *pos;
            three_column_delta = three_column_delta.clamp(-3, 3);
            let mut offset = PATHFIND_CELL_SIZE_F * 3.2;
            if units_to_path < 5 && !plant(3553) {
                offset = PATHFIND_CELL_SIZE_F * 1.5;
            }
            dest.x += offset * three_column_delta as f32 * evn.x;
            dest.y += offset * three_column_delta as f32 * evn.y;
            if factor & 1 != 0 {
                dest.x += PATHFIND_CELL_SIZE_F * evn.x;
                dest.y += PATHFIND_CELL_SIZE_F * evn.y;
            }
            dest.x -= factor as f32 * offset * ev.x;
            dest.y -= factor as f32 * offset * ev.y;
            dest.z = h.layer_height(dest.x, dest.y, layer);
            finish_column(h, unit, path, dest, ev, pt, src);
        }
        true
    }

    /// `groupMoveToPosition(pos, addWaypoint, cmdSource)`: infantry and vehicles in columns when they can, a formation in
    /// formation, a group clicked into gathered, the rest (nearest the goal first) each to its own offset from the goal.
    pub fn group_move_to_position<H: AiGroupMoveHost>(&mut self, h: &mut H, pos_in: &Coord3D, add_waypoint: bool, src: CommandSourceType) {
        let mut position = *pos_in;
        let mut did_infantry = false;
        let mut did_vehicles = false;
        let mut tighten_group = false;
        let (mut min, mut max, mut center, mut is_formation) = self.get_min_max_and_center(h);
        if add_waypoint {
            is_formation = false;
        }
        if !add_waypoint && !is_formation {
            self.friend_compute_ground_path(h, &position);
            did_infantry = self.friend_move_infantry_to_pos(h, &position, src);
            did_vehicles = self.friend_move_vehicle_to_pos(h, &position, src);
        }
        if self.dirty {
            self.recompute(h);
        }
        let gather = h.group_move_click_to_gather_factor();
        if !is_formation && src == CMD_FROM_PLAYER && gather > 0.0 {
            scale_rect_2d(&mut min, &mut max, gather);
            if coord3d_inside_rect_2d(&position, &min, &max) {
                tighten_group = true;
            }
        }
        let mut extra_margin = 0.0f32;
        for &m in &self.members.clone() {
            if h.is_kind_of(m, KINDOF_PRODUCED_AT_HELIPAD) {
                is_formation = false;
                extra_margin = max_of(extra_margin, h.major_radius(m));
            } else if h.is_kind_of(m, KINDOF_AIRCRAFT) {
                if h.has_ai(m) && !h.ai_is_doing_ground_movement(m) {
                    tighten_group = false; // "Don't tighten aircraft."
                    is_formation = false;
                }
                extra_margin = max_of(extra_margin, STD_AIRCRAFT_EXTRA_MARGIN as f32);
            }
        }
        let margin = (STD_WAYPOINT_CLAMP_MARGIN as f32 + extra_margin) as i32;
        clamp_waypoint_position(h, &mut position, margin);
        if tighten_group {
            is_formation = false;
            if !add_waypoint {
                let dx = ((max.x - min.x) / PATHFIND_CELL_SIZE_F) as i32;
                let dy = ((max.x - min.x) / PATHFIND_CELL_SIZE_F) as i32; // (upstream's: the x extent again)
                let cells = dx.wrapping_mul(dy);
                if cells < 2000 && !plant(3554) {
                    self.group_tighten_to_position(h, &position, false, src);
                    return;
                }
            }
        }
        if is_formation {
            self.friend_compute_ground_path(h, &position);
            self.friend_move_formation_to_pos(h, &position, src);
            return;
        }
        let mut iter = SimpleObjectIterator::new();
        for &o in &self.members.clone() {
            if h.is_held(o) || h.is_kind_of(o, KINDOF_IMMOBILE) || !h.has_ai(o) {
                continue;
            }
            if h.is_kind_of(o, KINDOF_INFANTRY) && did_infantry {
                continue;
            }
            if h.is_kind_of(o, KINDOF_VEHICLE) && did_vehicles && h.ai_is_doing_ground_movement(o) && !h.is_kind_of(o, KINDOF_CLIFF_JUMPER) {
                continue;
            }
            let unit_pos = h.position(o);
            h.pf_remove_goal(o);
            let adjust = 0.0f32;
            let _ = iter.insert(Some(o), adjust + d2(&unit_pos, &position));
        }
        let mut goal_pos = position;
        iter.sort(IterOrderType::SortedNearToFar, |_| 0);
        let mut first_unit = true;
        for unit in units(&mut iter) {
            h.set_formation_id(unit, NO_FORMATION_ID);
            if first_unit {
                if is_formation {
                    let v = h.formation_offset(unit);
                    goal_pos.x -= v.x;
                    goal_pos.y -= v.y;
                } else {
                    center = h.position(unit);
                }
                first_unit = false;
            }
            let dest = Self::compute_individual_destination(h, &goal_pos, unit, &center, is_formation);
            if src == CMD_FROM_PLAYER
                && h.status_test(unit, OBJECT_STATUS_CAN_STEALTH)
                && h.ai_can_auto_acquire(unit)
                && !h.status_test(unit, OBJECT_STATUS_STEALTHED)
                && !h.status_test(unit, OBJECT_STATUS_DETECTED)
                && !h.ai_can_auto_acquire_while_stealthed(unit)
            {
                if let Some(stealth_frames) = h.stealth_delay(unit) {
                    let random_frames = h.logic_random_value(0, LOGICFRAMES_PER_SECOND) as u32;
                    let t = h.frame().wrapping_add(stealth_frames).wrapping_add(random_frames);
                    h.ai_set_next_mood_check_time(unit, t);
                }
            }
            if !add_waypoint {
                h.ai_command(unit, GroupAiCommand::MoveToPosition { pos: dest }, src);
            } else {
                h.ai_command(unit, GroupAiCommand::FollowPathAppend { pos: dest }, src);
            }
        }
    }

    /// `groupScatter(cmdSource)`: each mobile member with an AI, farthest from the centre first, four bounding radii away from it
    /// (the centre nudged 0.01 down x for each).
    pub fn group_scatter<H: AiGroupMoveHost>(&mut self, h: &mut H, src: CommandSourceType) {
        if self.dirty {
            self.recompute(h);
        }
        let (_, _, mut center, _) = self.get_min_max_and_center(h);
        let mut iter = SimpleObjectIterator::new();
        for &o in &self.members.clone() {
            if h.is_held(o) || h.is_kind_of(o, KINDOF_IMMOBILE) || !h.has_ai(o) {
                continue;
            }
            let unit_pos = h.position(o);
            h.pf_remove_goal(o);
            let _ = iter.insert(Some(o), d2(&unit_pos, &center));
        }
        iter.sort(IterOrderType::SortedFarToNear, |_| 0);
        for unit in units(&mut iter) {
            center.x -= 0.01;
            let unit_pos = h.position(unit);
            let mut dest = unit_pos;
            let mut delta = Coord2D { x: unit_pos.x - center.x, y: unit_pos.y - center.y };
            delta.normalize();
            let r = h.bounding_circle_radius(unit);
            dest.x += delta.x * 4.0 * r;
            dest.y += delta.y * 4.0 * r;
            h.ai_command(unit, GroupAiCommand::MoveToPosition { pos: dest }, src);
        }
    }

    /// `groupTightenToPosition(pos, addWaypoint, cmdSource)`: each mobile member with an AI, nearest first, tightens to the
    /// position (a helicopter to its own spot around it, as the AI's command) or appends it; then the pathfinder told where each is.
    pub fn group_tighten_to_position<H: AiGroupMoveHost>(&mut self, h: &mut H, pos: &Coord3D, add_waypoint: bool, src: CommandSourceType) {
        if src == CMD_FROM_PLAYER && h.group_move_click_to_gather_factor() > 0.0 {
            // (its answers unused: "Disabled (because its not used to make a logical difference)")
            let _ = self.get_min_max_and_center(h);
        }
        let mut iter = SimpleObjectIterator::new();
        for &o in &self.members.clone() {
            let unit_pos = h.position(o);
            if h.is_held(o) || h.is_kind_of(o, KINDOF_IMMOBILE) || !h.has_ai(o) {
                continue;
            }
            let _ = iter.insert(Some(o), d2(&unit_pos, pos));
        }
        iter.sort(IterOrderType::SortedNearToFar, |_| 0);
        let order = units(&mut iter);
        let mut heli_idx = 0;
        for &unit in &order {
            if !add_waypoint {
                if h.is_kind_of(unit, KINDOF_PRODUCED_AT_HELIPAD) && !plant(3555) {
                    let mut heli_offs = *pos;
                    get_helicopter_offset(&mut heli_offs, heli_idx);
                    heli_idx += 1;
                    h.ai_command(unit, GroupAiCommand::TightenToPosition { pos: heli_offs }, CMD_FROM_AI);
                } else {
                    h.ai_command(unit, GroupAiCommand::TightenToPosition { pos: *pos }, src);
                }
            } else {
                h.ai_command(unit, GroupAiCommand::FollowPathAppend { pos: *pos }, src);
            }
        }
        for &unit in &order {
            let p = h.position(unit);
            h.pf_update_pos(unit, &p);
        }
    }

    /// `groupFollowWaypointPath`, `groupFollowWaypointPathExact`, `groupFollowWaypointPathAsTeam`,
    /// `groupFollowWaypointPathAsTeamExact`, `groupMoveToAndEvacuate`, `groupMoveToAndEvacuateAndExit`: each AI given the command
    /// ([`AiGroup::group_command`]).
    pub fn group_follow_waypoint_path<H: AiGroupHost>(&self, h: &mut H, way: u32, exact: bool, as_team: bool, src: CommandSourceType) {
        self.group_command(h, GroupAiCommand::FollowWaypointPath { way, exact, as_team }, src);
    }
}
