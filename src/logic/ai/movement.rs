// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: IS_IMPASSABLE and TCheckMovementInfo, lines 78-108;
//     the movement checks, 4746-5276; clearCellForDiameter, 6724-6788; adjustCoordToCell, 8960-8976; the lines and the views,
//     9109-9684)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The Pathfinder's movement checks and lines. `p1w13-sonnet`, layer 10, batch P5.
//!
//! What a cell allows (`validLocomotorSurfacesForCellType`, `validMovementPosition`, `validMovementTerrain`), whether a unit may stop
//! or pass over an area of cells (`checkDestination`, `checkForMovement`, `clearCellForDiameter`), where a unit stands
//! (`snapPosition`, `snapClosestGoalPosition`, `goalPosition`, `adjustCoordToCell`), where an aircraft lands
//! (`adjustToLandingDestination`), and the Bresenham lines (`iterateCellsAlongLine` with each of its callbacks: `isLinePassable`,
//! `isGroundPathPassable`, the view checks, the tall buildings an aircraft flies around).
//!
//! # Upstream facts kept
//!
//! - `validMovementTerrain` compares a cell's LAYER with `CELL_CLEAR` (0): a cell on a layer other than the ground is never "clear"
//!   by that test (upstream's typo, kept). No reachable cell has layer 0, so the branch is dead code.
//! - `adjustCoordToCell` adds the `double` 0.05 for an uncentred cell: the product is computed in `double` and narrowed.
//! - `clearCellForDiameter` returns `false` (0) off the map, else the diameter it found clear (1 for radius 0), retrying 2 smaller.
//! - The lines visit a cell twice when the minor axis steps (once as `to`, once as `from` and the next `to`); a callback sees each.
//! - `isAttackViewBlockedByObstacle` skips the first 3 cells of an attacker on a bridge ("someone wanna tell me what this magic number
//!   means?"); `isViewBlockedByObstacle` is that with the objects' own positions.

use crate::common::base_type::{real_to_int_floor, Coord2D, Coord3D, ICoord2D, Region3D};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND};
use crate::logic::ai::cell::{
    CellId, CELL_BRIDGE_IMPASSABLE, CELL_CLEAR, CELL_CLIFF, CELL_IMPASSABLE, CELL_OBSTACLE, CELL_RUBBLE, CELL_WATER, NO_UNITS,
    UNIT_GOAL, UNIT_GOAL_OTHER_MOVING, UNIT_PRESENT_FIXED, UNIT_PRESENT_MOVING,
};
use crate::logic::ai::layer::PATHFIND_CELL_SIZE_F;
use crate::logic::ai::path::Path;
use crate::logic::ai::pathfinder::{Pathfinder, PathfinderHost, PfObjectFacts};
use crate::logic::locomotor::{
    LocomotorSurfaceTypeMask, LOCOMOTORSURFACE_AIR, LOCOMOTORSURFACE_CLIFF, LOCOMOTORSURFACE_GROUND, LOCOMOTORSURFACE_RUBBLE,
    LOCOMOTORSURFACE_WATER, NO_SURFACES,
};
use crate::logic::modules::plant;

/// What the movement checks and lines ask beyond the map's host: the units' relations and crush levels, the AI's ignored obstacle,
/// the terrain's heights and destination layers, the weapons' terrain line of sight, the slavers and containers, and the closest tall
/// building an aircraft flies around.
pub trait MovementHost: PathfinderHost {
    /// `getAIUpdateInterface()->getIgnoredObstacleID()` (`INVALID_ID` without an AI).
    fn ignored_obstacle_id(&self, id: ObjectId) -> ObjectId;
    /// `obj->getRelationship(unit) == ALLIES`.
    fn are_allies(&self, obj: ObjectId, unit: ObjectId) -> bool;
    /// `obj->canCrushOrSquish(unit, TEST_CRUSH_OR_SQUISH)`.
    fn can_crush_or_squish(&self, obj: ObjectId, unit: ObjectId) -> bool;
    fn crusher_level(&self, id: ObjectId) -> i32;
    fn crushable_level(&self, id: ObjectId) -> i32;
    /// `TheTerrainLogic->getLayerHeight(x, y, layer)`.
    fn layer_height(&self, x: f32, y: f32, layer: PathfindLayer) -> f32;
    /// `TheTerrainLogic->getLayerForDestination(pos)`.
    fn layer_for_destination(&self, pos: &Coord3D) -> PathfindLayer;
    fn is_significantly_above_terrain(&self, id: ObjectId) -> bool;
    /// `TheAI->getAiData()->m_attackUsesLineOfSight`.
    fn attack_uses_line_of_sight(&self) -> bool;
    /// The attacker's current weapon's `isClearGoalFiringLineOfSightTerrain` to the victim (or its position); none without a weapon.
    fn weapon_terrain_line_clear(&self, attacker: ObjectId, attacker_pos: &Coord3D, victim: Option<ObjectId>, victim_pos: &Coord3D) -> Option<bool>;
    /// `getSlaverID(o)` (the first slaved update's slaver), `getContainerID(o)`.
    fn slaver_id(&self, id: ObjectId) -> ObjectId;
    fn container_id(&self, id: ObjectId) -> ObjectId;
    /// `obj->isKindOf(KINDOF_INFANTRY)` (batch P11).
    fn is_infantry(&self, id: ObjectId) -> bool;
    /// `ThePartitionManager->getClosestObject(pos, radius, FROM_BOUNDINGSPHERE_2D, {KINDOF_AIRCRAFT_PATH_AROUND})`.
    fn closest_tall_building(&self, pos: &Coord3D, radius: f32) -> Option<ObjectId>;
}

/// `IS_IMPASSABLE(type)`: "impassable to ground units".
pub fn is_impassable(ty: u8) -> bool {
    ty == CELL_IMPASSABLE || ty == CELL_OBSTACLE || ty == CELL_BRIDGE_IMPASSABLE
}

/// `TCheckMovementInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckMovementInfo {
    pub cell: ICoord2D,
    pub layer: PathfindLayer,
    pub radius: i32,
    pub center_in_cell: bool,
    pub consider_transient: bool,
    pub acceptable_surfaces: LocomotorSurfaceTypeMask,
    pub ally_fixed_count: i32,
    pub enemy_fixed: bool,
    pub ally_moving: bool,
    pub ally_goal: bool,
}

/// `validLocomotorSurfacesForCellType(t)`.
pub fn valid_locomotor_surfaces_for_cell_type(t: u8) -> LocomotorSurfaceTypeMask {
    match t {
        CELL_OBSTACLE | CELL_IMPASSABLE | CELL_BRIDGE_IMPASSABLE => LOCOMOTORSURFACE_AIR,
        CELL_CLEAR => LOCOMOTORSURFACE_GROUND | LOCOMOTORSURFACE_AIR,
        CELL_WATER => LOCOMOTORSURFACE_WATER | LOCOMOTORSURFACE_AIR,
        CELL_RUBBLE => LOCOMOTORSURFACE_RUBBLE | LOCOMOTORSURFACE_AIR,
        CELL_CLIFF => LOCOMOTORSURFACE_CLIFF | LOCOMOTORSURFACE_AIR,
        _ => NO_SURFACES,
    }
}

/// The callbacks of `iterateCellsAlongLine` (upstream's function pointers and their `userData`).
#[derive(Clone, Copy, Debug)]
pub enum LineProc {
    /// `linePassableCallback`.
    LinePassable { obj: Option<ObjectId>, surfaces: LocomotorSurfaceTypeMask, radius: i32, center: bool, blocked: bool, allow_pinched: bool },
    /// `groundPathPassableCallback`.
    GroundPathPassable { diameter: i32, crusher: bool },
    /// `lineBlockedByObstacleCallback` (no caller upstream; kept for the later batches' use).
    LineBlocked { obj: ObjectId, other: ObjectId },
    /// `attackBlockedByObstacleCallback`.
    AttackBlocked { obj: ObjectId, victim: Option<ObjectId>, victim_cell: Option<CellId>, skip_count: i32 },
    /// `segmentIntersectsBuildingCallback`: the building found.
    SegmentIntersectsBuilding { ignore: ObjectId, found: Option<ObjectId> },
    /// `tightenPathCallback` (batch P9): the last position on the line where the unit may stop (`adjust_candidate`, as a human's).
    TightenPath { obj: ObjectId, surfaces: LocomotorSurfaceTypeMask, layer: PathfindLayer, radius: i32, center: bool, found: Option<Coord3D> },
}

/// `computeNormalRadialOffset(from, insert, to, obj, radius)`: the object's position moved `radius` along the normal to from-to on
/// the object's side.
fn compute_normal_radial_offset(from: &Coord3D, to: &Coord3D, obj_pos: &Coord3D, radius: f32) -> Coord3D {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let obj_dx = obj_pos.x - from.x;
    let obj_dy = obj_pos.y - from.y;
    let cross = dx * obj_dy - dy * obj_dx;
    let mut normal = if cross > 0.0 { Coord3D { x: dy, y: -dx, z: 0.0 } } else { Coord3D { x: -dy, y: dx, z: 0.0 } };
    normal.normalize();
    let mut insert = *obj_pos;
    insert.x += normal.x * radius;
    insert.y += normal.y * radius;
    insert
}

impl Pathfinder {
    /// `chooseBestLocomotorForPosition(layer, locomotorSet, pos)`: the surfaces of the cell there (clear without one), for the set's
    /// `findLocomotor`.
    pub fn surfaces_for_position(&self, layer: PathfindLayer, pos: &Coord3D) -> LocomotorSurfaceTypeMask {
        let x = real_to_int_floor(pos.x / 10.0);
        let y = real_to_int_floor(pos.y / 10.0);
        let t = self.get_cell(layer, x, y).map_or(CELL_CLEAR, |c| self.store.cells[c as usize].get_type());
        valid_locomotor_surfaces_for_cell_type(t)
    }

    /// `validMovementTerrain(layer, locomotor, pos)`: the cell there accepts one of the locomotor's legal surfaces (an obstacle and an
    /// impassable cell do; no cell does not).
    pub fn valid_movement_terrain(&self, layer: PathfindLayer, legal_surfaces: LocomotorSurfaceTypeMask, pos: &Coord3D) -> bool {
        let x = real_to_int_floor(pos.x / 10.0);
        let y = real_to_int_floor(pos.y / 10.0);
        let Some(c) = self.get_cell(layer, x, y) else { return false };
        let cell = &self.store.cells[c as usize];
        if (cell.get_type() == CELL_OBSTACLE && !plant(3072)) || cell.get_type() == CELL_IMPASSABLE {
            return true;
        }
        // (upstream compares the layer with CELL_CLEAR)
        if cell.get_layer() != LAYER_GROUND && cell.get_layer() == i32::from(CELL_CLEAR) {
            return true;
        }
        legal_surfaces & valid_locomotor_surfaces_for_cell_type(cell.get_type()) != 0
    }

    /// `cleanOpenAndClosedLists()`: both lists released, their counts added to the cumulative count.
    pub fn clean_open_and_closed_lists(&mut self) {
        let mut count = 0;
        if self.open_list.is_some() {
            count += self.store.release_open_list(self.open_list);
            self.open_list = None;
        }
        if self.closed_list.is_some() {
            count += self.store.release_closed_list(self.closed_list);
            self.closed_list = None;
        }
        self.cumulative_cells_allocated = self.cumulative_cells_allocated.wrapping_add(count);
    }

    /// `validMovementPosition(isCrusher, acceptableSurfaces, toCell, fromCell)`: the ignored obstacle, a fence for a crusher, or a
    /// cell of an accepted surface.
    pub fn valid_movement_position(&self, is_crusher: bool, surfaces: LocomotorSurfaceTypeMask, to: Option<CellId>) -> bool {
        let Some(to) = to else { return false };
        if self.store.is_obstacle_present(to, self.ignore_obstacle_id) {
            return true;
        }
        if is_crusher && self.store.is_obstacle_fence(to) && !plant(3073) {
            return true;
        }
        valid_locomotor_surfaces_for_cell_type(self.store.cells[to as usize].get_type()) & surfaces != 0
    }

    /// `validMovementPosition(isCrusher, layer, locomotorSet, x, y)` (the header's inline).
    pub fn valid_movement_position_at(&self, is_crusher: bool, layer: PathfindLayer, surfaces: LocomotorSurfaceTypeMask, x: i32, y: i32) -> bool {
        self.valid_movement_position(is_crusher, surfaces, self.get_cell(layer, x, y))
    }

    /// `checkDestination(obj, cellX, cellY, layer, iRadius, centerInCell)`: whether the unit may stop on the cells around: none off
    /// the map, impassable or an obstacle (but its ignored one); an aircraft that adjusts its destination only avoids other aircraft's
    /// goals; a ground unit avoids an ally's goal and a fixed unit it cannot crush.
    #[allow(clippy::too_many_arguments)] // (upstream's parameters)
    pub fn check_destination(&self, host: &impl MovementHost, obj: Option<&PfObjectFacts>, cell_x: i32, cell_y: i32, layer: PathfindLayer, radius: i32, center: bool) -> bool {
        let above = radius + i32::from(center);
        let (mut ignore, mut check_aircraft, mut obj_id) = (INVALID_ID, false, INVALID_ID);
        if let Some(o) = obj {
            if let Some(ai) = o.ai {
                ignore = host.ignored_obstacle_id(o.id);
                check_aircraft = ai.aircraft_adjusts_destination;
                obj_id = o.id;
            }
        }
        for i in cell_x - radius..cell_x + above {
            for j in cell_y - radius..cell_y + above {
                let Some(c) = self.get_cell(layer, i, j) else { return false }; // "off the map, so can't place here."
                let cell = &self.store.cells[c as usize];
                if check_aircraft {
                    if !cell.is_aircraft_goal() || self.store.get_goal_aircraft(c) == obj_id {
                        continue;
                    }
                    return false;
                }
                if cell.get_type() == CELL_OBSTACLE {
                    if self.store.is_obstacle_present(c, ignore) {
                        continue;
                    }
                    return false;
                }
                if is_impassable(cell.get_type()) {
                    return false;
                }
                if cell.get_flags() == NO_UNITS {
                    continue; // "Nobody is here, so it's ok."
                }
                let goal = self.store.get_goal_unit(c);
                if goal == obj_id || goal == ignore {
                    continue; // "we got it." / "we are ignoring it."
                }
                if goal != INVALID_ID {
                    let Some(o) = obj else { return false };
                    if host.object(goal).is_some() {
                        if host.are_allies(o.id, goal) && !plant(3074) {
                            return false; // "Don't usurp your allies goals."
                        }
                        if cell.get_flags() == UNIT_PRESENT_FIXED && !host.can_crush_or_squish(o.id, goal) {
                            return false; // "Don't move to an occupied cell."
                        }
                    }
                }
            }
        }
        true
    }

    /// `checkForMovement(obj, info)`: whether the unit may pass over the cells around (false off the map), counting the fixed allies
    /// (each once) and noting fixed enemies it cannot crush, moving allies and goals; infantry passes infantry.
    pub fn check_for_movement(&self, host: &impl MovementHost, obj: Option<&PfObjectFacts>, info: &mut CheckMovementInfo) -> bool {
        const MAX_ALLY: usize = 5;
        info.ally_fixed_count = 0;
        info.ally_moving = false;
        info.ally_goal = false;
        info.enemy_fixed = false;
        let mut allies: Vec<ObjectId> = Vec::with_capacity(MAX_ALLY);
        let Some(o) = obj else { return true }; // "not object can move there."
        let ignore = if o.ai.is_some() { host.ignored_obstacle_id(o.id) } else { INVALID_ID };
        let above = info.radius + i32::from(info.center_in_cell);
        for i in info.cell.x - info.radius..info.cell.x + above {
            for j in info.cell.y - info.radius..info.cell.y + above {
                let Some(c) = self.get_cell(info.layer, i, j) else { return false };
                let flags = self.store.cells[c as usize].get_flags();
                let pos_unit = self.store.get_pos_unit(c);
                if flags == UNIT_GOAL || flags == UNIT_GOAL_OTHER_MOVING {
                    info.ally_goal = true;
                }
                if flags == NO_UNITS || pos_unit == o.id || pos_unit == ignore {
                    continue;
                }
                let mut check = false;
                let mut unit = None;
                if flags == UNIT_PRESENT_MOVING || flags == UNIT_GOAL_OTHER_MOVING {
                    unit = host.object(pos_unit).map(|u| u.id);
                    if unit.is_some_and(|u| host.are_allies(o.id, u)) {
                        info.ally_moving = true;
                    }
                    if info.consider_transient {
                        check = true;
                    }
                }
                if flags == UNIT_PRESENT_FIXED {
                    check = true;
                    unit = host.object(pos_unit).map(|u| u.id);
                }
                if let (true, Some(u)) = (check, unit) {
                    if o.ai.is_some() && host.ignored_obstacle_id(o.id) == u {
                        check = false;
                    }
                }
                let (true, Some(u)) = (check, unit) else { continue };
                // (`INFANTRY_MOVES_THROUGH_INFANTRY`, which AIPathfind.h defines; batch P11)
                if host.is_infantry(o.id) && host.is_infantry(u) && !plant(3157) {
                    continue; // "Infantry can run through infantry."
                }
                if host.are_allies(o.id, u) {
                    if host.object(u).and_then(|f| f.ai).is_none() {
                        return false; // "can't path through not-idle units."
                    }
                    if !allies.contains(&u) || plant(3075) {
                        info.ally_fixed_count += 1;
                        if allies.len() < MAX_ALLY {
                            allies.push(u);
                        }
                    }
                } else if !host.can_crush_or_squish(o.id, u) {
                    info.enemy_fixed = true;
                }
            }
        }
        true
    }

    /// `adjustCoordToCell(cellX, cellY, centerInCell, pos, layer)`: the cell's centre, or 0.05 cells into it (in `double`), at the
    /// layer's height there.
    pub fn adjust_coord_to_cell(host: &impl MovementHost, cell_x: i32, cell_y: i32, center: bool, pos: &mut Coord3D, layer: PathfindLayer) {
        if center {
            pos.x = (cell_x as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            pos.y = (cell_y as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
        } else {
            let off = if plant(3070) { 0.06 } else { 0.05 };
            pos.x = ((f64::from(cell_x as f32) + off) * f64::from(PATHFIND_CELL_SIZE_F)) as f32;
            pos.y = ((f64::from(cell_y as f32) + off) * f64::from(PATHFIND_CELL_SIZE_F)) as f32;
        }
        pos.z = host.layer_height(pos.x, pos.y, layer);
    }

    /// The cell a unit of that centring snaps to from `pos` (half a cell on for an uncentred one), clamped to the map.
    fn snap_cell(&self, pos: &Coord3D, center: bool) -> ICoord2D {
        let mut adjust = *pos;
        if !center {
            adjust.x += PATHFIND_CELL_SIZE_F / 2.0;
            adjust.y += PATHFIND_CELL_SIZE_F / 2.0;
        }
        self.world_to_cell(&adjust).0
    }

    /// `snapPosition(obj, pos)`: the position moved to its cell (on the ground's height).
    pub fn snap_position(&self, host: &impl MovementHost, obj: Option<&PfObjectFacts>, pos: &mut Coord3D) {
        let (_, center) = Self::get_radius_and_center(obj);
        let cell = self.snap_cell(pos, center);
        Self::adjust_coord_to_cell(host, cell.x, cell.y, center, pos, LAYER_GROUND);
    }

    /// `snapClosestGoalPosition(obj, pos)`: the position snapped, then moved to a neighbouring cell the unit may stop on if its own is
    /// taken (for a unit of radius 0, a neighbour without another unit's goal, then one without a fixed unit).
    pub fn snap_closest_goal_position(&self, host: &impl MovementHost, obj: &PfObjectFacts, pos: &mut Coord3D) {
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let layer = host.layer_for_destination(pos);
        let cell = self.snap_cell(pos, center);
        Self::adjust_coord_to_cell(host, cell.x, cell.y, center, pos, LAYER_GROUND);
        if self.check_destination(host, Some(obj), cell.x, cell.y, layer, radius, center) {
            return;
        }
        let around = || (cell.x - 1..cell.x + 2).flat_map(move |i| (cell.y - 1..cell.y + 2).map(move |j| (i, j)));
        for (i, j) in around() {
            if self.check_destination(host, Some(obj), i, j, layer, radius, center) {
                Self::adjust_coord_to_cell(host, i, j, center, pos, layer);
                return;
            }
        }
        if radius == 0 {
            for (i, j) in around() {
                if let Some(c) = self.get_cell(layer, i, j) {
                    let goal = self.store.get_goal_unit(c);
                    if goal == INVALID_ID || goal == obj.id {
                        Self::adjust_coord_to_cell(host, i, j, center, pos, layer);
                        return;
                    }
                }
            }
            for (i, j) in around() {
                if let Some(c) = self.get_cell(layer, i, j) {
                    if self.store.cells[c as usize].get_flags() != UNIT_PRESENT_FIXED {
                        Self::adjust_coord_to_cell(host, i, j, center, pos, layer);
                        return;
                    }
                }
            }
        }
    }

    /// `goalPosition(obj, pos)`: the unit's goal cell's position (zero and false without an AI's goal).
    pub fn goal_position(&self, host: &impl MovementHost, obj: &PfObjectFacts, pos: &mut Coord3D) -> bool {
        let Some(ai) = obj.ai else { return false };
        let (_, center) = Self::get_radius_and_center(Some(obj));
        *pos = Coord3D::default();
        if ai.goal_cell.x < 0 || ai.goal_cell.y < 0 {
            return false;
        }
        Self::adjust_coord_to_cell(host, ai.goal_cell.x, ai.goal_cell.y, center, pos, LAYER_GROUND);
        true
    }

    /// `checkForLanding(cellX, cellY, layer, iRadius, center, dest)`: a cell that is not a cliff, water or impassable, free of others'
    /// goals around.
    #[allow(clippy::too_many_arguments)] // (upstream's parameters)
    fn check_for_landing(&self, host: &impl MovementHost, cell_x: i32, cell_y: i32, layer: PathfindLayer, radius: i32, center: bool, dest: &mut Coord3D) -> bool {
        let Some(c) = self.get_cell(layer, cell_x, cell_y) else { return false };
        if matches!(self.store.cells[c as usize].get_type(), CELL_CLIFF | CELL_WATER | CELL_IMPASSABLE) {
            return false; // "no final destinations on cliffs, water, etc."
        }
        if self.check_destination(host, None, cell_x, cell_y, layer, radius, center) {
            let mut adjust = Coord3D::default();
            Self::adjust_coord_to_cell(host, cell_x, cell_y, center, &mut adjust, self.store.cells[c as usize].get_layer());
            *dest = adjust;
            return true;
        }
        false
    }

    /// `adjustToLandingDestination(obj, dest)`: the first landing cell on a square spiral of at most 400 cells from the destination
    /// (true at once when both it and the object are off the map).
    pub fn adjust_to_landing_destination(&self, host: &impl MovementHost, obj: &PfObjectFacts, dest: &mut Coord3D) -> bool {
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let extent: Region3D = host.maximum_pathfind_extent();
        // (`isInRegionNoZ`: strictly inside)
        let in_region = |p: &Coord3D| extent.lo.x < p.x && p.x < extent.hi.x && extent.lo.y < p.y && (p.y < extent.hi.y || plant(3079));
        if !in_region(dest) && !in_region(&obj.pos) {
            return true;
        }
        let cell = self.snap_cell(dest, center);
        let layer = host.layer_for_destination(dest);
        self.spiral(cell, |i, j| self.check_for_landing(host, i, j, layer, radius, center, dest))
    }

    /// The square spiral of the adjust family: the cell, then right, down, left 2, up 2, ... while `limit` (400) cells remain; the first
    /// cell `try_cell` accepts.
    fn spiral(&self, cell: ICoord2D, mut try_cell: impl FnMut(i32, i32) -> bool) -> bool {
        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let (mut i, mut j) = (cell.x, cell.y);
        if try_cell(i, j) {
            return true;
        }
        let mut delta = 1;
        while limit > 0 {
            for (di, dj, grow) in [(1, 0, false), (0, 1, true), (-1, 0, false), (0, -1, true)] {
                for _ in 0..delta {
                    i += di;
                    j += dj;
                    limit -= 1;
                    if try_cell(i, j) {
                        return true;
                    }
                }
                if grow {
                    delta += 1;
                }
            }
        }
        false
    }

    /// `clearCellForDiameter(crusher, cellX, cellY, layer, pathDiameter)`: see the module's notes.
    pub fn clear_cell_for_diameter(&self, host: &impl MovementHost, crusher: bool, cell_x: i32, cell_y: i32, layer: PathfindLayer, diameter: i32) -> i32 {
        let radius = diameter / 2;
        let above = if radius == 0 { radius + 1 } else { radius };
        let mut clear = true;
        let cut_corners = radius > if plant(3076) { 0 } else { 1 };
        for i in cell_x - radius..cell_x + above {
            let x_edge = i == cell_x - radius || i == cell_x + above - 1;
            for j in cell_y - radius..cell_y + above {
                let y_edge = j == cell_y - radius || j == cell_y + above - 1;
                if x_edge && y_edge && cut_corners {
                    continue; // "this is an outside corner cell, and we are cutting corners."
                }
                let Some(c) = self.get_cell(layer, i, j) else { return 0 }; // "off the map."
                let ty = self.store.cells[c as usize].get_type();
                if ty != CELL_CLEAR && !(ty == CELL_OBSTACLE && self.store.is_obstacle_fence(c) && crusher) {
                    clear = false;
                }
                if self.store.cells[c as usize].get_flags() == UNIT_PRESENT_FIXED && diameter >= 2 {
                    let unit = self.store.get_pos_unit(c);
                    if host.object(unit).is_some() && host.crushable_level(unit) > i32::from(crusher) {
                        clear = false;
                    }
                }
                if !clear {
                    break; // (the inner loop only: the next columns still check their first cell is on the map)
                }
            }
        }
        if clear {
            return if radius == 0 { 1 } else { 2 * radius };
        }
        if diameter < 2 {
            return 0;
        }
        self.clear_cell_for_diameter(host, crusher, cell_x, cell_y, layer, diameter - 2)
    }

    /// One call of a line's callback; nonzero ends the line.
    fn line_proc(&self, host: &impl MovementHost, proc_: &mut LineProc, from: Option<CellId>, to: CellId, to_x: i32, to_y: i32) -> i32 {
        let cells = &self.store.cells;
        match proc_ {
            LineProc::LinePassable { obj, surfaces, radius, center, blocked, allow_pinched } => {
                let facts = obj.and_then(|id| host.object(id));
                let is_crusher = obj.is_some_and(|id| host.crusher_level(id) > 0);
                let mut info = CheckMovementInfo {
                    cell: ICoord2D { x: to_x, y: to_y },
                    layer: cells[to as usize].get_layer(),
                    center_in_cell: *center,
                    radius: *radius,
                    consider_transient: *blocked,
                    acceptable_surfaces: *surfaces,
                    ..CheckMovementInfo::default()
                };
                if !self.check_for_movement(host, facts.as_ref(), &mut info) || info.ally_fixed_count != 0 || info.enemy_fixed {
                    return 1; // "bail out"
                }
                if !*allow_pinched && cells[to as usize].get_pinched() && !plant(3078) {
                    return 1;
                }
                if let Some(f) = from {
                    if cells[to as usize].get_layer() != LAYER_GROUND && cells[f as usize].get_layer() == cells[to as usize].get_layer() && cells[to as usize].get_type() == CELL_CLEAR {
                        return 0;
                    }
                }
                i32::from(!self.valid_movement_position(is_crusher, *surfaces, Some(to)))
            }
            LineProc::GroundPathPassable { diameter, crusher } => {
                let cur = self.clear_cell_for_diameter(host, *crusher, to_x, to_y, cells[to as usize].get_layer(), *diameter);
                if cur == *diameter {
                    return 0; // "good to go."
                }
                if let Some(f) = from {
                    if cells[to as usize].get_layer() != LAYER_GROUND && cells[f as usize].get_layer() == cells[to as usize].get_layer() {
                        return 0;
                    }
                }
                1 // "failed."
            }
            LineProc::LineBlocked { obj, other } => {
                if cells[to as usize].get_type() != CELL_OBSTACLE {
                    return 0;
                }
                let present = |id: ObjectId| self.store.is_obstacle_present(to, id);
                if present(*obj) || present(*other) || present(host.container_id(*obj)) || present(host.slaver_id(*obj)) || present(host.slaver_id(*other)) {
                    return 0;
                }
                i32::from(!self.store.is_obstacle_transparent(to))
            }
            LineProc::AttackBlocked { obj, victim, victim_cell, skip_count } => {
                if *skip_count > 0 {
                    *skip_count -= 1;
                    return 0;
                }
                if cells[to as usize].get_type() != CELL_OBSTACLE {
                    return 0;
                }
                let present = |id: ObjectId| self.store.is_obstacle_present(to, id);
                if present(*obj) {
                    return 0;
                }
                if let Some(v) = victim {
                    if present(*v) || present(host.slaver_id(*v)) {
                        return 0;
                    }
                }
                if (present(host.container_id(*obj)) && !plant(3080)) || present(host.slaver_id(*obj)) || self.store.is_obstacle_transparent(to) {
                    return 0;
                }
                if let Some(vc) = victim_cell {
                    if present(self.store.get_obstacle_id(*vc)) {
                        return 0;
                    }
                }
                1 // "bail early"
            }
            LineProc::TightenPath { obj, surfaces, layer, radius, center, found } => {
                if from.is_none() || cells[to as usize].get_layer() != *layer {
                    return 0; // ("abort", but the walk goes on)
                }
                let Some(facts) = host.object(*obj) else { return 0 };
                // (upstream's `pos` is uninitialised: the origin, see `adjust`)
                if let Some(pos) = self.adjust_candidate(host, &facts, *surfaces, true, to_x, to_y, *layer, *radius, *center, &Coord3D::default()) {
                    *found = Some(pos);
                }
                0
            }
            LineProc::SegmentIntersectsBuilding { ignore, found } => {
                if cells[to as usize].get_type() != CELL_OBSTACLE {
                    return 0;
                }
                let id = self.store.get_obstacle_id(to);
                match host.object(id) {
                    Some(o) if o.aircraft_path_around => {
                        if o.id == *ignore {
                            return 0;
                        }
                        *found = Some(o.id);
                        1
                    }
                    _ => 0,
                }
            }
        }
    }

    /// `iterateCellsAlongLine(start, end, layer, proc, userData)` from world positions (each clamped to the map).
    pub fn iterate_cells_along_line_world(&self, host: &impl MovementHost, start: &Coord3D, end: &Coord3D, layer: PathfindLayer, proc_: &mut LineProc) -> i32 {
        let (s, _) = self.world_to_cell(start);
        let (e, _) = self.world_to_cell(end);
        self.iterate_cells_along_line(host, s, e, layer, proc_)
    }

    /// `iterateCellsAlongLine(start, end, layer, proc, userData)`: Bresenham's line, the callback on each cell (see the module's
    /// notes); 0 when it completes or leaves the map.
    pub fn iterate_cells_along_line(&self, host: &impl MovementHost, start: ICoord2D, end: ICoord2D, layer: PathfindLayer, proc_: &mut LineProc) -> i32 {
        self.walk_line(start, end, layer, |pf, from, to, x, y| pf.line_proc(host, proc_, from, to, x, y))
    }

    /// `iterateCellsAlongLine`'s walk, `f` its callback (`(pathfinder, from, to, to_x, to_y)`; nonzero ends the line).
    pub fn walk_line(&self, start: ICoord2D, end: ICoord2D, layer: PathfindLayer, mut f: impl FnMut(&Self, Option<CellId>, CellId, i32, i32) -> i32) -> i32 {
        let delta_x = (end.x - start.x).abs();
        let delta_y = (end.y - start.y).abs();
        let (mut x, mut y) = (start.x, start.y);
        let (mut xinc1, mut xinc2) = if end.x >= start.x { (1, 1) } else { (-1, -1) };
        let (mut yinc1, mut yinc2) = if end.y >= start.y { (1, 1) } else { (-1, -1) };
        let (den, mut num, numadd, numpixels);
        if delta_x >= delta_y {
            xinc1 = 0;
            yinc2 = 0;
            den = delta_x;
            num = delta_x / 2;
            numadd = delta_y;
            numpixels = delta_x;
        } else {
            xinc2 = 0;
            yinc1 = 0;
            den = delta_y;
            num = delta_y / 2;
            numadd = delta_x;
            numpixels = delta_y;
        }
        let mut from: Option<CellId> = None;
        for _ in 0..=numpixels {
            let Some(mut to) = self.get_cell(layer, x, y) else { return 0 };
            let ret = f(self, from, to, x, y);
            if ret != 0 {
                return ret;
            }
            num += numadd;
            if num >= den && !plant(3071) {
                num -= den;
                x += xinc1;
                y += yinc1;
                from = Some(to);
                let Some(t) = self.get_cell(layer, x, y) else { return 0 };
                to = t;
                let ret = f(self, from, to, x, y);
                if ret != 0 {
                    return ret;
                }
            }
            x += xinc2;
            y += yinc2;
            from = Some(to);
        }
        0
    }

    /// `isLinePassable(obj, acceptableSurfaces, layer, startWorld, endWorld, blocked, allowPinched)`.
    #[allow(clippy::too_many_arguments)]
    pub fn is_line_passable(&self, host: &impl MovementHost, obj: Option<&PfObjectFacts>, surfaces: LocomotorSurfaceTypeMask, layer: PathfindLayer, start: &Coord3D, end: &Coord3D, blocked: bool, allow_pinched: bool) -> bool {
        let (radius, center) = Self::get_radius_and_center(obj);
        let mut p = LineProc::LinePassable { obj: obj.map(|o| o.id), surfaces, radius, center, blocked, allow_pinched };
        self.iterate_cells_along_line_world(host, start, end, layer, &mut p) == 0
    }

    /// `isGroundPathPassable(isCrusher, startWorld, startLayer, endWorld, pathDiameter)`.
    pub fn is_ground_path_passable(&self, host: &impl MovementHost, is_crusher: bool, start: &Coord3D, start_layer: PathfindLayer, end: &Coord3D, diameter: i32) -> bool {
        let mut p = LineProc::GroundPathPassable { diameter, crusher: is_crusher };
        self.iterate_cells_along_line_world(host, start, end, start_layer, &mut p) == 0
    }

    /// `isViewBlockedByObstacle(obj, objOther)`: no for a flying other; else the attack view between their positions.
    pub fn is_view_blocked_by_obstacle(&self, host: &impl MovementHost, obj: &PfObjectFacts, other: &PfObjectFacts) -> bool {
        if host.is_significantly_above_terrain(other.id) {
            return false; // "We don't check los to flying objects."
        }
        self.is_attack_view_blocked_by_obstacle(host, obj, &obj.pos, Some(other), &other.pos)
    }

    /// `isAttackViewBlockedByObstacle(attacker, attackerPos, victim, victimPos)`: when attacks use the line of sight and the attacker
    /// needs it, its weapon's terrain line (a mobile attacker's), then the obstacles along the line (see the module's notes).
    pub fn is_attack_view_blocked_by_obstacle(&self, host: &impl MovementHost, attacker: &PfObjectFacts, attacker_pos: &Coord3D, victim: Option<&PfObjectFacts>, victim_pos: &Coord3D) -> bool {
        if !host.attack_uses_line_of_sight() || !attacker.attack_needs_line_of_sight {
            return false;
        }
        if !attacker.immobile {
            if let Some(clear) = host.weapon_terrain_line_clear(attacker.id, attacker_pos, victim.map(|v| v.id), victim_pos) {
                if !clear {
                    return true;
                }
            }
        }
        let mut layer = victim.map_or(LAYER_GROUND, |v| v.layer);
        let victim_cell = self.get_cell_at(layer, victim_pos);
        let mut skip_count = 0;
        if attacker.layer != LAYER_GROUND {
            skip_count = if plant(3077) { 2 } else { 3 }; // "someone wanna tell me what this magic number means?"
            if layer == LAYER_GROUND {
                layer = attacker.layer;
            }
        }
        let mut p = LineProc::AttackBlocked { obj: attacker.id, victim: victim.map(|v| v.id), victim_cell, skip_count };
        self.iterate_cells_along_line_world(host, attacker_pos, victim_pos, layer, &mut p) != 0
    }

    /// `segmentIntersectsTallBuilding(curNode, nextNode, ignoreBuilding, insertPos1..3)`: a tall building an aircraft flies around on
    /// the segment (twice: the segment's end moved out of the building's circle first): the three points around it; the next node
    /// moved when its end was inside.
    pub fn segment_intersects_tall_building(&self, host: &impl MovementHost, path: &mut Path, cur: usize, next: usize, ignore: ObjectId) -> Option<[Coord3D; 3]> {
        let mut from_pos = *path.node(cur).position();
        let mut to_pos = *path.node(next).position();
        for _ in 0..2 {
            let mut p = LineProc::SegmentIntersectsBuilding { ignore, found: None };
            let ret = self.iterate_cells_along_line_world(host, &from_pos, &to_pos, LAYER_GROUND, &mut p);
            let LineProc::SegmentIntersectsBuilding { found: Some(b), .. } = p else { continue };
            if ret == 0 {
                continue;
            }
            let bldg = host.object(b).expect("the building was found");
            let radius = bldg.bounding_circle_radius + 2.0 * PATHFIND_CELL_SIZE_F;
            let mut delta = Coord2D { x: to_pos.x - bldg.pos.x, y: to_pos.y - bldg.pos.y };
            if f64::from(delta.length()) <= f64::from(radius) * if plant(3081) { 0.9 } else { 0.98 } {
                if f64::from(delta.length()) < 0.1 {
                    delta.x = 1.0;
                }
                delta.normalize();
                delta.x *= radius;
                delta.y *= radius;
                to_pos.x = bldg.pos.x + delta.x;
                to_pos.y = bldg.pos.y + delta.y;
                path.node_mut(next).set_position(&to_pos);
                continue;
            }
            delta = Coord2D { x: from_pos.x - bldg.pos.x, y: from_pos.y - bldg.pos.y };
            if f64::from(delta.length()) <= f64::from(radius) * 0.98 {
                if f64::from(delta.length()) < 0.1 {
                    delta.x = 1.0;
                }
                delta.normalize();
                delta.x *= radius;
                delta.y *= radius;
                from_pos.x = bldg.pos.x + delta.x;
                from_pos.y = bldg.pos.y + delta.y;
            }
            let insert2 = compute_normal_radial_offset(&from_pos, &to_pos, &bldg.pos, radius);
            let insert1 = compute_normal_radial_offset(&from_pos, &insert2, &bldg.pos, radius);
            let insert3 = compute_normal_radial_offset(&insert2, &to_pos, &bldg.pos, radius);
            return Some([insert1, insert2, insert3]);
        }
        None
    }

    /// `circleClipsTallBuilding(from, to, circleRadius, ignoreBuilding, adjustTo)`: the closest tall building to `to` within the
    /// circle: the point beside it (and beside a second one there); none without one.
    pub fn circle_clips_tall_building(&self, host: &impl MovementHost, from: &Coord3D, to: &Coord3D, circle_radius: f32) -> Option<Coord3D> {
        let tall = host.closest_tall_building(to, circle_radius)?;
        let t = host.object(tall).expect("the building was found");
        let radius = t.bounding_circle_radius + 2.0 * PATHFIND_CELL_SIZE_F;
        let mut adjust = compute_normal_radial_offset(from, to, &t.pos, circle_radius + radius);
        if let Some(other) = host.closest_tall_building(&adjust, circle_radius) {
            if other != tall {
                let o = host.object(other).expect("the building was found");
                let radius = o.bounding_circle_radius + 2.0 * PATHFIND_CELL_SIZE_F;
                let tmp_to = adjust;
                adjust = compute_normal_radial_offset(from, &tmp_to, &o.pos, circle_radius + radius);
            }
        }
        Some(adjust)
    }
}
