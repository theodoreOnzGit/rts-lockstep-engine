// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: checkChangeLayers and the neighbours, 5966-6386;
//     internalFindPath, 6462-6722; clip, 8157-8176; buildActualPath and prependCells, 8978-9087)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The Pathfinder's A* search: `internalFindPath` and what it calls. `p1w13-sonnet`, layer 10, batch P6.
//!
//! The search clips the segment to the map, finds the goal cell on the destination's layer, refuses a goal the unit may not stop
//! on, a start or goal off the map, zones that differ (through the terrain zones when the goal is the ignored obstacle or the start
//! is inside one, "tunneling") and a goal of the wrong surface; then pops the open list until the goal comes off it. Each cell
//! popped goes on the closed list, offers its bridge connection (`checkChangeLayers`), and opens its neighbours
//! (`examineNeighboringCells`): first, toward the goal, the straight line of cells a unit could take (`examineCellsCallback`), then
//! the eight neighbours, a diagonal only past an open orthogonal one, each priced by `costSoFar` plus the surcharges for moving
//! allies, cliffs, pinched cells, fixed allies, closed zone blocks and obstacles. The path is the goal's parents back to the start
//! (`prependCells`), optimized.
//!
//! # Upstream facts kept
//!
//! - `examineCellsCallback` prices a cell at its parent's cost plus 0.5 times 10 (a `float`, truncated), and returns early on the
//!   same tests twice; it runs only without an attack distance, outside a tunnel and for a locomotor set that is not downhill only.
//! - In `examineNeighboringCells`, `onList` is always false where it is read after the `continue`; a neighbour already on a list is
//!   never re-priced (upstream's dead branch kept).
//! - A dozer may path into a friendly or neutral obstacle (the "dozer hack"), which does not count as an open orthogonal neighbour.
//! - `considerTransient` is false whatever the distance (upstream's two tests set it false).
//! - `internalFindPath` returns `false` (`NULL`) when the unit may not stop at the goal, and a path to (0, 0) is refused.
//! - The cliff surcharge compares the ground heights at the cells' corners (`getGroundHeight`), 7 diagonals when they differ by less
//!   than a cell.

use crate::common::base_type::{real_to_int, real_to_int_floor, Coord3D, ICoord2D};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND, LAYER_INVALID, LAYER_WALL};
use crate::logic::ai::cell::{CellId, CELL_CLIFF, CELL_OBSTACLE, COST_DIAGONAL, COST_ORTHOGONAL};
use crate::logic::ai::layer::{PATHFIND_CELL_SIZE, PATHFIND_CELL_SIZE_F};
use crate::logic::ai::movement::{CheckMovementInfo, MovementHost};
use crate::logic::ai::path::{Path, PathHost};
use crate::logic::ai::pathfinder::{Pathfinder, PfObjectFacts};
use crate::logic::locomotor::LocomotorSurfaceTypeMask;
use crate::logic::modules::plant;
use crate::logic::world::partition::filters::clip_line_2d;

/// `NO_ATTACK`.
pub const NO_ATTACK: i32 = 0;

/// What the search reads of a `LocomotorSet`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PfLocomotorSet {
    pub valid_surfaces: LocomotorSurfaceTypeMask,
    pub downhill_only: bool,
}

/// What the search asks beyond the movement checks: the controlling player's type, the AI's `canPathThroughUnits`, the kinds a
/// neighbour's price reads, the relation that lets a dozer into an obstacle, and the terrain's ground height.
pub trait SearchHost: MovementHost {
    /// Not `getControllingPlayer()->getPlayerType() == PLAYER_COMPUTER` (true without a player).
    fn is_human(&self, id: ObjectId) -> bool;
    fn can_path_through_units(&self, id: ObjectId) -> bool;
    fn is_dozer(&self, id: ObjectId) -> bool;
    fn is_vehicle(&self, id: ObjectId) -> bool;
    /// `obj->getRelationship(other) == ENEMIES`.
    fn are_enemies(&self, obj: ObjectId, other: ObjectId) -> bool;
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn ground_height_at(&self, x: f32, y: f32) -> f32;
}

/// The pathfinder as the paths see it (`PathHost`): the real line tests, cells and layers.
pub struct PathfinderView<'a, H: MovementHost> {
    pub pf: &'a Pathfinder,
    pub host: &'a H,
}

impl<H: MovementHost> PathHost for PathfinderView<'_, H> {
    fn is_line_passable(&mut self, obj: ObjectId, surfaces: LocomotorSurfaceTypeMask, layer: PathfindLayer, start: &Coord3D, end: &Coord3D, blocked: bool, allow_pinched: bool) -> bool {
        let facts = if obj == INVALID_ID { None } else { self.host.object(obj) };
        self.pf.is_line_passable(self.host, facts.as_ref(), surfaces, layer, start, end, blocked, allow_pinched)
    }
    fn is_ground_path_passable(&mut self, crusher: bool, start: &Coord3D, layer: PathfindLayer, end: &Coord3D, diameter: i32) -> bool {
        self.pf.is_ground_path_passable(self.host, crusher, start, layer, end, diameter)
    }
    fn cell_type_and_pinched(&mut self, layer: PathfindLayer, pos: &Coord3D) -> Option<(i32, bool)> {
        self.pf.get_cell_at(layer, pos).map(|c| (i32::from(self.pf.store.cells[c as usize].get_type()), self.pf.store.cells[c as usize].get_pinched()))
    }
    fn layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        self.host.layer_for_destination(pos)
    }
    fn object_layer(&mut self, obj: ObjectId) -> PathfindLayer {
        self.host.object(obj).map_or(LAYER_GROUND, |o| o.layer)
    }
}

/// `ExamineCellsStruct`.
struct ExamineCells {
    loco: PfLocomotorSet,
    center: bool,
    is_human: bool,
    radius: i32,
    obj: Option<PfObjectFacts>,
    goal: CellId,
}

impl Pathfinder {
    fn on_list(&self, c: CellId) -> bool {
        self.store.cells[c as usize].has_info() && (self.store.get_open(c) || self.store.get_closed(c))
    }

    fn in_logical_extent(&self, x: i32, y: i32) -> bool {
        let e = &self.logical_extent;
        !(x < e.lo.x || y < e.lo.y || x > e.hi.x || y > e.hi.y)
    }

    /// `clip(from, to)`: the segment's cells clipped to the map; an end moved to its clipped cell's corner (plus 0.05).
    pub fn clip(&self, from: &mut Coord3D, to: &mut Coord3D) {
        let cell = |p: &Coord3D| ICoord2D { x: real_to_int_floor(p.x / PATHFIND_CELL_SIZE as f32), y: real_to_int_floor(p.y / PATHFIND_CELL_SIZE as f32) };
        let (from_cell, to_cell) = (cell(from), cell(to));
        if let Some((true, cf, ct)) = clip_line_2d(from_cell, to_cell, &self.extent) {
            if from_cell != cf {
                from.x = cf.x as f32 * PATHFIND_CELL_SIZE_F + 0.05;
                from.y = cf.y as f32 * PATHFIND_CELL_SIZE_F + 0.05;
            }
            if to_cell != ct {
                to.x = ct.x as f32 * PATHFIND_CELL_SIZE_F + 0.05;
                to.y = ct.y as f32 * PATHFIND_CELL_SIZE_F + if plant(3090) { 0.5 } else { 0.05 };
            }
        }
    }

    /// `checkChangeLayers(parentCell)`: a cell connecting to another layer opens that layer's cell there at the same costs.
    pub fn check_change_layers(&mut self, parent: CellId) {
        let connect = self.store.cells[parent as usize].get_connect_layer();
        if connect == LAYER_INVALID || plant(3094) {
            return;
        }
        let coord = ICoord2D { x: i32::from(self.store.x_index(parent)), y: i32::from(self.store.y_index(parent)) };
        let Some(new_cell) = self.get_cell(connect, coord.x, coord.y) else { return };
        if self.on_list(new_cell) {
            return;
        }
        if !self.store.allocate_info(new_cell, coord) {
            return;
        }
        self.store.set_parent_cell(new_cell, parent);
        let (so_far, total) = (self.store.get_cost_so_far(parent), self.store.get_total_cost(parent));
        self.store.set_cost_so_far(new_cell, so_far); // "same as parent cost"
        self.store.set_total_cost(new_cell, total);
        self.open_list = self.store.put_on_sorted_open_list(new_cell, self.open_list);
    }

    /// `examineCellsCallback`: open a cell of the straight line toward the goal.
    fn examine_cells_callback(&mut self, host: &impl SearchHost, d: &ExamineCells, from: Option<CellId>, to: CellId, to_x: i32, to_y: i32) -> i32 {
        let is_crusher = d.obj.is_some_and(|o| host.crusher_level(o.id) > 0);
        if self.is_tunneling {
            return 1; // "abort."
        }
        let Some(from) = from else { return 0 };
        if !self.valid_movement_position(is_crusher, d.loco.valid_surfaces, Some(to)) {
            return 1;
        }
        if self.store.cells[to as usize].get_layer() == LAYER_GROUND && !self.zone_manager.is_passable(to_x, to_y) {
            return 1;
        }
        let on_list = self.on_list(to);
        if self.store.cells[to as usize].get_pinched() {
            return 1; // "abort."
        }
        if d.is_human && !self.in_logical_extent(to_x, to_y) {
            return 1;
        }
        let mut info = CheckMovementInfo {
            cell: ICoord2D { x: to_x, y: to_y },
            layer: self.store.cells[from as usize].get_layer(),
            center_in_cell: d.center,
            radius: d.radius,
            consider_transient: false,
            acceptable_surfaces: d.loco.valid_surfaces,
            ..CheckMovementInfo::default()
        };
        if !self.check_for_movement(host, d.obj.as_ref(), &mut info) || info.enemy_fixed {
            return 1;
        }
        // (a float, truncated)
        let new_cost = (self.store.get_cost_so_far(from) as f32 + if plant(3095) { 0.6 } else { 0.5 } * COST_ORTHOGONAL as f32) as u32;
        if self.store.cells[to as usize].get_type() == CELL_CLIFF || info.ally_fixed_count != 0 {
            return 1;
        }
        if !self.store.allocate_info(to, ICoord2D { x: to_x, y: to_y }) {
            return 1;
        }
        self.store.set_blocked_by_ally(to, false);
        let cost_remaining = self.store.cost_to_goal(to, d.goal);
        if on_list && self.store.get_cost_so_far(to) <= new_cost {
            return 0; // "keep going."
        }
        self.store.set_cost_so_far(to, new_cost);
        self.store.set_parent_cell(to, from);
        let total = self.store.get_cost_so_far(to).wrapping_add(cost_remaining);
        self.store.set_total_cost(to, total);
        self.relist(to);
        0
    }

    /// Off the closed and open lists, then onto the open list in cost order.
    fn relist(&mut self, c: CellId) {
        if self.store.get_closed(c) {
            self.closed_list = self.store.remove_from_closed_list(c, self.closed_list);
        }
        if self.store.get_open(c) {
            self.open_list = self.store.remove_from_open_list(c, self.open_list);
        }
        self.open_list = self.store.put_on_sorted_open_list(c, self.open_list);
    }

    /// `iterateCellsAlongLine` with `examineCellsCallback` (the same walk as [`Pathfinder::iterate_cells_along_line`], mutating).
    fn examine_line(&mut self, host: &impl SearchHost, start: ICoord2D, end: ICoord2D, layer: PathfindLayer, d: &ExamineCells) {
        let delta_x = (end.x - start.x).abs();
        let delta_y = (end.y - start.y).abs();
        let (mut x, mut y) = (start.x, start.y);
        let (mut xinc1, mut xinc2) = if end.x >= start.x { (1, 1) } else { (-1, -1) };
        let (mut yinc1, mut yinc2) = if end.y >= start.y { (1, 1) } else { (-1, -1) };
        let (den, mut num, numadd, numpixels);
        if delta_x >= delta_y {
            xinc1 = 0;
            yinc2 = 0;
            (den, num, numadd, numpixels) = (delta_x, delta_x / 2, delta_y, delta_x);
        } else {
            xinc2 = 0;
            yinc1 = 0;
            (den, num, numadd, numpixels) = (delta_y, delta_y / 2, delta_x, delta_y);
        }
        let mut from: Option<CellId> = None;
        for _ in 0..=numpixels {
            let Some(mut to) = self.get_cell(layer, x, y) else { return };
            if self.examine_cells_callback(host, d, from, to, x, y) != 0 {
                return;
            }
            num += numadd;
            if num >= den {
                num -= den;
                x += xinc1;
                y += yinc1;
                from = Some(to);
                let Some(t) = self.get_cell(layer, x, y) else { return };
                to = t;
                if self.examine_cells_callback(host, d, from, to, x, y) != 0 {
                    return;
                }
            }
            x += xinc2;
            y += yinc2;
            from = Some(to);
        }
    }

    /// The ground heights at two cells' corners differ by less than a cell.
    fn cliff_is_shallow(host: &impl SearchHost, a: ICoord2D, b: ICoord2D) -> bool {
        let za = host.ground_height_at(a.x as f32 * PATHFIND_CELL_SIZE_F, a.y as f32 * PATHFIND_CELL_SIZE_F);
        let zb = host.ground_height_at(b.x as f32 * PATHFIND_CELL_SIZE_F, b.y as f32 * PATHFIND_CELL_SIZE_F);
        f64::from(za - zb).abs() < f64::from(PATHFIND_CELL_SIZE_F)
    }

    /// `examineNeighboringCells(parentCell, goalCell, locomotorSet, isHuman, centerInCell, radius, startCellNdx, obj, attackDistance)`:
    /// see the module's notes; the number of infos it allocated.
    #[allow(clippy::too_many_arguments)]
    pub fn examine_neighboring_cells(&mut self, host: &impl SearchHost, parent: CellId, goal: Option<CellId>, loco: PfLocomotorSet, is_human: bool, center: bool, radius: i32, start: ICoord2D, obj: Option<&PfObjectFacts>, attack_distance: i32) -> i32 {
        let can_path_through_units = obj.is_some_and(|o| o.ai.is_some() && host.can_path_through_units(o.id));
        let is_crusher = obj.is_some_and(|o| host.crusher_level(o.id) > 0);
        let parent_layer = self.store.cells[parent as usize].get_layer();
        let parent_xy = ICoord2D { x: i32::from(self.store.x_index(parent)), y: i32::from(self.store.y_index(parent)) };
        if attack_distance == NO_ATTACK && !self.is_tunneling && !loco.downhill_only {
            if let Some(g) = goal {
                let d = ExamineCells { loco, center, is_human, radius, obj: obj.copied(), goal: g };
                let end = ICoord2D { x: i32::from(self.store.x_index(g)), y: i32::from(self.store.y_index(g)) };
                self.examine_line(host, parent_xy, end, parent_layer, &d);
            }
        }
        const DELTA: [(i32, i32); 8] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)];
        const FIRST_DIAGONAL: usize = 4;
        const ADJACENT: [usize; 5] = [0, 1, 2, 3, 0];
        let mut neighbor_flags = [false; 8];
        let mut cell_count = 0;
        for i in 0..DELTA.len() {
            neighbor_flags[i] = false;
            let coord = ICoord2D { x: parent_xy.x + DELTA[i].0, y: parent_xy.y + DELTA[i].1 };
            let Some(new_cell) = self.get_cell(parent_layer, coord.x, coord.y) else { continue };
            let not_zone_passable = self.store.cells[new_cell as usize].get_layer() == LAYER_GROUND && !self.zone_manager.is_passable(coord.x, coord.y);
            if is_human && !self.in_logical_extent(coord.x, coord.y) {
                continue;
            }
            if self.on_list(new_cell) {
                continue;
            }
            if i >= FIRST_DIAGONAL && !neighbor_flags[ADJACENT[i - 4]] && !neighbor_flags[ADJACENT[i - 3]] && !plant(3096) {
                continue;
            }
            if loco.downhill_only {
                let from_z = host.ground_height_at(parent_xy.x as f32 * PATHFIND_CELL_SIZE_F, parent_xy.y as f32 * PATHFIND_CELL_SIZE_F);
                let to_z = host.ground_height_at(coord.x as f32 * PATHFIND_CELL_SIZE_F, coord.y as f32 * PATHFIND_CELL_SIZE_F);
                if from_z < to_z {
                    continue;
                }
            }
            let mut movement_valid = true;
            let mut dozer_hack = false;
            if !self.valid_movement_position(is_crusher, loco.valid_surfaces, Some(new_cell)) {
                movement_valid = false;
                let o = obj.expect("upstream dereferences the object here");
                if host.is_dozer(o.id) && self.store.cells[new_cell as usize].get_type() == CELL_OBSTACLE {
                    let obstacle = self.store.get_obstacle_id(new_cell);
                    if host.object(obstacle).is_some() && !host.are_enemies(o.id, obstacle) {
                        movement_valid = true;
                        dozer_hack = true;
                    }
                }
                if !movement_valid && !self.is_tunneling {
                    continue;
                }
            }
            if !dozer_hack {
                neighbor_flags[i] = true;
            }
            let mut info = CheckMovementInfo {
                cell: coord,
                layer: parent_layer,
                center_in_cell: center,
                radius,
                consider_transient: false,
                acceptable_surfaces: loco.valid_surfaces,
                ..CheckMovementInfo::default()
            };
            let mut dx = (coord.x - start.x).abs();
            let mut dy = (coord.y - start.y).abs();
            if !self.check_for_movement(host, obj, &mut info) || info.enemy_fixed {
                if !self.is_tunneling {
                    continue;
                }
                movement_valid = false;
            }
            if movement_valid && !self.store.cells[new_cell as usize].get_pinched() {
                self.is_tunneling = false;
            }
            if !self.store.cells[new_cell as usize].has_info() {
                if !self.store.allocate_info(new_cell, coord) {
                    return cell_count;
                }
                cell_count += 1;
            }
            let mut new_cost = self.store.cost_so_far(new_cell, Some(parent));
            if info.ally_moving && dx < 10 && dy < 10 && !plant(3100) {
                new_cost = new_cost.wrapping_add((3 * COST_DIAGONAL) as u32);
            }
            let ty = self.store.cells[new_cell as usize].get_type();
            if ty == CELL_CLIFF && !self.store.cells[new_cell as usize].get_pinched() {
                if Self::cliff_is_shallow(host, parent_xy, coord) {
                    new_cost = new_cost.wrapping_add((if plant(3091) { 6 } else { 7 } * COST_DIAGONAL) as u32);
                }
            } else if self.store.cells[new_cell as usize].get_pinched() {
                new_cost = new_cost.wrapping_add(COST_ORTHOGONAL as u32);
            }
            self.store.set_blocked_by_ally(new_cell, false);
            if info.ally_fixed_count > 0 {
                let cost_factor = 3 * COST_DIAGONAL;
                if !can_path_through_units && !plant(3092) {
                    self.store.set_blocked_by_ally(new_cell, true);
                }
                new_cost = new_cost.wrapping_add(cost_factor as u32);
            }
            let mut cost_remaining: i32 = 0;
            if let Some(g) = goal {
                if attack_distance == NO_ATTACK {
                    cost_remaining = self.store.cost_to_goal(new_cell, g) as i32;
                } else {
                    dx = coord.x - i32::from(self.store.x_index(g));
                    dy = coord.y - i32::from(self.store.y_index(g));
                    cost_remaining = (f64::from(COST_ORTHOGONAL) * f64::from(dx * dx + dy * dy).sqrt()) as i32;
                    cost_remaining -= attack_distance / 2;
                    if cost_remaining < 0 {
                        cost_remaining = 0;
                    }
                    if info.ally_goal {
                        let o = obj.expect("upstream dereferences the object here");
                        let add = if host.is_vehicle(o.id) { 3 * COST_ORTHOGONAL } else { COST_ORTHOGONAL };
                        new_cost = new_cost.wrapping_add(add as u32);
                    }
                }
            }
            if not_zone_passable {
                new_cost = new_cost.wrapping_add((if plant(3099) { 99 } else { 100 } * COST_ORTHOGONAL) as u32);
            }
            if ty == CELL_OBSTACLE {
                new_cost = new_cost.wrapping_add((100 * COST_ORTHOGONAL) as u32);
            }
            if self.is_tunneling && !self.valid_movement_position(is_crusher, loco.valid_surfaces, Some(new_cell)) {
                new_cost = new_cost.wrapping_add((10 * COST_ORTHOGONAL) as u32);
            }
            self.store.set_cost_so_far(new_cell, new_cost);
            self.store.set_parent_cell(new_cell, parent);
            if self.is_tunneling {
                cost_remaining = 0; // "find the closest valid cell."
            }
            let total = self.store.get_cost_so_far(new_cell).wrapping_add(cost_remaining as u32);
            self.store.set_total_cost(new_cell, total);
            self.relist(new_cell);
        }
        cell_count
    }

    /// `prependCells(path, fromPos, goalCell, center)`: the goal's parents back to the start, each node at its cell (a bridge's
    /// layer kept where two cells share a place), cliffs not optimized across, each cell's zone block marked passable; the start
    /// position first unless it is the first node's.
    pub fn prepend_cells(&mut self, host: &impl MovementHost, path: &mut Path, from: &Coord3D, goal: CellId, center: bool) {
        let goal_null = self.store.get_parent_cell(goal).is_none();
        let mut prev: Option<CellId> = None;
        let mut cell = goal;
        let xy = |s: &crate::logic::ai::cell::CellStore, c: CellId| (i32::from(s.x_index(c)), i32::from(s.y_index(c)));
        while let Some(parent) = self.store.get_parent_cell(cell) {
            let (x, y) = xy(&self.store, cell);
            if !plant(3097) {
                self.zone_manager.set_passable(x, y, true);
            }
            let mut pos = Coord3D::default();
            Self::adjust_coord_to_cell(host, x, y, center, &mut pos, self.store.cells[cell as usize].get_layer());
            if let Some(p) = prev {
                if (x, y) == xy(&self.store, p) {
                    let mut layer = self.store.cells[cell as usize].get_layer();
                    if layer == LAYER_GROUND {
                        layer = self.store.cells[p as usize].get_layer();
                    }
                    let first = path.first_node().expect("a node was prepended");
                    path.node_mut(first).set_layer(layer);
                    cell = parent;
                    continue;
                }
            }
            let mut can_optimize = true;
            let is_cliff = |c: CellId| self.store.cells[c as usize].get_type() == CELL_CLIFF;
            if is_cliff(cell) {
                if prev.is_some_and(|p| !is_cliff(p)) {
                    if let Some(first) = path.first_node() {
                        path.node_mut(first).set_can_optimize(false);
                    }
                }
            } else if prev.is_some_and(is_cliff) {
                can_optimize = false;
            }
            path.prepend_node(&pos, self.store.cells[cell as usize].get_layer());
            let first = path.first_node().expect("just prepended");
            path.node_mut(first).set_can_optimize(can_optimize);
            if self.store.is_blocked_by_ally(cell) {
                path.set_blocked_by_ally(true);
            }
            if let Some(p) = prev {
                self.store.clear_parent_cell(p);
            }
            prev = Some(cell);
            cell = parent;
        }
        let (x, y) = xy(&self.store, cell);
        self.zone_manager.set_passable(x, y, true);
        if goal_null {
            let mut pos = Coord3D::default();
            Self::adjust_coord_to_cell(host, x, y, center, &mut pos, self.store.cells[cell as usize].get_layer());
            path.prepend_node(&pos, self.store.cells[cell as usize].get_layer());
        }
        let first = path.first_node().expect("a path has a node");
        let fp = *path.node(first).position();
        if from.x != fp.x || from.y != fp.y {
            path.prepend_node(from, self.store.cells[cell as usize].get_layer());
        }
    }

    /// `buildActualPath(obj, acceptableSurfaces, fromPos, goalCell, center, blocked)`: from a pinched goal whose parent is not pinched,
    /// that parent; the cells prepended, then optimized.
    #[allow(clippy::too_many_arguments)]
    pub fn build_actual_path(&mut self, host: &impl MovementHost, obj: Option<&PfObjectFacts>, surfaces: LocomotorSurfaceTypeMask, from: &Coord3D, goal: CellId, center: bool, blocked: bool) -> Path {
        let mut path = Path::new();
        let mut goal = goal;
        if self.store.cells[goal as usize].get_pinched() && !plant(3098) {
            if let Some(p) = self.store.get_parent_cell(goal) {
                if !self.store.cells[p as usize].get_pinched() {
                    goal = p;
                }
            }
        }
        self.prepend_cells(host, &mut path, from, goal, center);
        let mut view = PathfinderView { pf: self, host };
        path.optimize(&mut view, obj.map_or(INVALID_ID, |o| o.id), surfaces, blocked);
        path
    }

    /// `internalFindPath(obj, locomotorSet, from, rawTo)`: the A* search (see the module's notes); none when there is no path.
    pub fn internal_find_path(&mut self, host: &impl SearchHost, obj: Option<&PfObjectFacts>, loco: PfLocomotorSet, from: &Coord3D, raw_to: &Coord3D) -> Option<Path> {
        let (mut radius, mut center) = (0, true);
        if obj.is_some() {
            (radius, center) = Self::get_radius_and_center(obj);
        }
        let is_human = obj.is_none_or(|o| host.is_human(o.id));
        if raw_to.x == 0.0 && raw_to.y == 0.0 {
            return None; // "Attempting pathfind to 0,0, generally a bug."
        }
        if !self.is_map_ready {
            return None;
        }
        let mut to = *raw_to;
        let mut clip_from = *from;
        self.clip(&mut clip_from, &mut to);
        if !center {
            to.x += PATHFIND_CELL_SIZE_F / 2.0;
            to.y += PATHFIND_CELL_SIZE_F / 2.0;
        }
        self.is_tunneling = false;
        let destination_layer = host.layer_for_destination(&to);
        let goal = self.get_cell_at(destination_layer, &to)?;
        let (cell, _) = self.world_to_cell(&to);
        if !self.check_destination(host, obj, cell.x, cell.y, destination_layer, radius, center) {
            return None;
        }
        let (start, _) = self.world_to_cell(&clip_from);
        let layer = obj.map_or(LAYER_GROUND, |o| o.layer);
        let mut parent = self.get_clipped_cell(layer, &clip_from)?;
        let (pos2d, _) = self.world_to_cell(&to);
        if !self.store.allocate_info(goal, pos2d) {
            return None;
        }
        if parent != goal {
            let (pos2d, _) = self.world_to_cell(&clip_from);
            if !self.store.allocate_info(parent, pos2d) {
                self.store.release_info(goal);
                return None;
            }
        }
        self.is_tunneling = self.store.cells[parent as usize].get_type() == CELL_OBSTACLE;
        let is_crusher = obj.is_some_and(|o| host.crusher_level(o.id) > 0);
        let zm = &self.zone_manager;
        let mut zone1 = zm.get_effective_zone(loco.valid_surfaces, is_crusher, self.store.cells[parent as usize].get_zone());
        let mut zone2 = zm.get_effective_zone(loco.valid_surfaces, is_crusher, self.store.cells[goal as usize].get_zone());
        if layer == LAYER_WALL && zone1 == 0 {
            return None;
        }
        if destination_layer == LAYER_WALL && zone2 == 0 {
            return None;
        }
        if self.store.is_obstacle_present(goal, self.ignore_obstacle_id) || self.is_tunneling {
            zone2 = zm.get_effective_zone(loco.valid_surfaces, is_crusher, zm.get_effective_terrain_zone(zone2));
            zone1 = zm.get_effective_zone(loco.valid_surfaces, is_crusher, zm.get_effective_terrain_zone(zone1));
        }
        if zone1 != zone2 && !plant(3093) {
            self.store.release_info(goal);
            self.store.release_info(parent);
            return None;
        }
        // (the header's `validMovementPosition(isCrusher, layer, locomotorSet, pos)`: `REAL_TO_INT`, truncating)
        let at = |pf: &Self, l: PathfindLayer, p: &Coord3D| {
            pf.valid_movement_position_at(is_crusher, l, loco.valid_surfaces, real_to_int(p.x / PATHFIND_CELL_SIZE as f32), real_to_int(p.y / PATHFIND_CELL_SIZE as f32))
        };
        if !at(self, destination_layer, &to) {
            self.is_tunneling = false;
            self.store.release_info(goal);
            self.store.release_info(parent);
            return None;
        }
        if !at(self, layer, from) {
            self.is_tunneling = true;
        }
        self.store.start_pathfind(parent, Some(goal));
        self.open_list = Some(parent);
        self.closed_list = None;
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            if parent == goal {
                self.is_tunneling = false;
                let path = self.build_actual_path(host, obj, loco.valid_surfaces, from, goal, center, false);
                self.store.release_info(parent);
                self.clean_open_and_closed_lists();
                return Some(path);
            }
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            self.check_change_layers(parent);
            self.examine_neighboring_cells(host, parent, Some(goal), loco, is_human, center, radius, start, obj, NO_ATTACK);
        }
        self.is_tunneling = false;
        self.store.release_info(goal);
        self.clean_open_and_closed_lists();
        None
    }
}
