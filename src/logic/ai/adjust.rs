// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: checkForAdjust, 5201-5250; adjustDestination and the
//     target and possible families, 5355-5646; pathDestination, tightenPath and checkPathCost, 8178-8659; findClosestPath, 8661-8958)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The Pathfinder's destination adjustments and `findClosestPath`. `p1w13-sonnet`, layer 10, batch P9.
//!
//! - `adjustDestination` finds the first cell, on a square spiral of 400 cells from the destination, where the unit may stop
//!   (`checkForAdjust`): not a cliff, inside the logical extent for a human player, free for the unit (`checkDestination`), reachable
//!   (an aircraft always; otherwise the quick zone check from the unit or from the destination), and, for a group, not much costlier
//!   from the group's destination than straight (`tightenPath`, `checkPathCost`: 1.4 times the cells' Manhattan distance); then once
//!   more without the group.
//! - `adjustTargetDestination`: the spiral's first cell from which the weapon reaches the target.
//! - `adjustToPossibleDestination`: the spiral's first cell of the unit's zone (the terrain zone from an obstacle) it may stop on.
//! - `pathDestination`: A* from the destination over the cells the unit may stop on, nearer the group's goal each time; nothing
//!   comes of it in the shipped (release) build, see below.
//! - `checkPathCost`: an A* from `from` to `to` of at most about 500 cells; the goal's total cost, or `MAX_COST`.
//! - `findClosestPath`: A* toward `to` that settles for the reachable cell where the unit may stop that is closest to it, its
//!   distance weighted by the cost so far times `pathCostMultiplier`.
//!
//! # Upstream facts kept
//!
//! - `pathDestination` returns false whatever it finds, and writes `dest` only in a debug build (`_DEBUG` or `_INTERNAL`; the
//!   shipped game is neither), so `dest` is read-only here. It returns the cell count (true when nonzero) when an info cannot be
//!   allocated, leaving its lists in place, as `checkPathCost` does.
//! - `checkForAdjust`'s callers in `pathDestination` and `tightenPathCallback` pass an uninitialised position as `dest`, which its
//!   reachability test reads; the C++ build zeroes locals (`-ftrivial-auto-var-init=zero`), and the port passes the origin.
//! - `checkPathCost` returns the cell count when an info cannot be allocated, and `NULL` (0) before the map is ready.
//! - `adjustToPossibleDestination` writes `dest` for each cell of the zone it tries, so a failure leaves the last one there.
//! - `findClosestPath`'s goal on the ignored obstacle: the obstacle under the goal becomes the ignored one when it is an airfield.
//!
//! # Upstream defects kept
//!
//! - `checkForAdjust` with a group destination runs `checkPathCost`, a search of its own on the pathfinder's one pair of lists: from
//!   `pathDestination`, it replaces that search's lists and cleans them, releasing the infos of its goal and current cells.
//!   `pathDestination` then reads the goal's indices through the released info, a null pointer: upstream crashes (the C++ build
//!   does, on the same programs), and the port panics there (`CellStore::x_index`). Nothing upstream calls `pathDestination`.

use crate::common::base_type::{Coord3D, ICoord2D};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID};
use crate::common::game_type::{LAYER_GROUND, LAYER_WALL};
use crate::logic::ai::cell::{CellId, CELL_CLIFF, CELL_OBSTACLE, COST_TO_DISTANCE_FACTOR_SQR};
use crate::logic::ai::layer::PATHFIND_CELL_SIZE_F;
use crate::logic::ai::movement::{is_impassable, CheckMovementInfo, LineProc, MovementHost};
use crate::logic::locomotor::LocomotorSurfaceTypeMask;
use crate::logic::ai::path::Path;
use crate::logic::ai::pathfinder::{Pathfinder, PfObjectFacts};
use crate::logic::ai::search::{PfLocomotorSet, SearchHost, NO_ATTACK};
use crate::logic::modules::plant;

/// `checkPathCost`'s `MAX_COST`.
pub const MAX_COST: i32 = 0x7fff_0000;
/// `MAX_CELL_COUNT`.
const MAX_CELL_COUNT: i32 = 500;

/// What the adjustments ask beyond the search: the weapon's reach.
pub trait AdjustHost: SearchHost {
    /// `weapon->isGoalPosWithinAttackRange(obj, goalPos, victim, victimPos)`, of the weapon the caller passes.
    fn goal_pos_within_attack_range(&self, obj: ObjectId, goal: &Coord3D, victim: Option<ObjectId>, victim_pos: &Coord3D) -> bool;
}

impl Pathfinder {
    fn cell_xy(&self, c: CellId) -> ICoord2D {
        ICoord2D { x: i32::from(self.store.x_index(c)), y: i32::from(self.store.y_index(c)) }
    }

    /// `checkForAdjust` up to its group test: the cell's position when the unit may stop there and can reach it (from where it is,
    /// or from `dest` when `dest` cannot be reached).
    #[allow(clippy::too_many_arguments)] // (upstream's parameters)
    pub fn adjust_candidate(&self, host: &impl MovementHost, obj: &PfObjectFacts, surfaces: LocomotorSurfaceTypeMask, is_human: bool, x: i32, y: i32, layer: PathfindLayer, radius: i32, center: bool, dest: &Coord3D) -> Option<Coord3D> {
        let c = self.get_cell(layer, x, y)?;
        if self.store.cells[c as usize].get_type() == CELL_CLIFF {
            return None; // "no final destinations on cliffs."
        }
        if is_human {
            let e = &self.logical_extent;
            if x < e.lo.x || y < e.lo.y || x > e.hi.x - i32::from(plant(3130)) || y > e.hi.y {
                return None;
            }
        }
        if !self.check_destination(host, Some(obj), x, y, layer, radius, center) {
            return None;
        }
        let mut adjust = Coord3D::default();
        Self::adjust_coord_to_cell(host, x, y, center, &mut adjust, self.store.cells[c as usize].get_layer());
        let mut adjusted_exists = true;
        if !obj.aircraft {
            let exists = self.client_safe_quick_does_path_exist(host, surfaces, &obj.pos, dest);
            adjusted_exists = self.client_safe_quick_does_path_exist(host, surfaces, &obj.pos, &adjust);
            if !exists && self.client_safe_quick_does_path_exist(host, surfaces, dest, &adjust) {
                adjusted_exists = true;
            }
        }
        adjusted_exists.then_some(adjust)
    }

    /// `checkForAdjust(obj, locomotorSet, isHuman, cellX, cellY, layer, iRadius, center, dest, groupDest)`: see the module's notes.
    #[allow(clippy::too_many_arguments)] // (upstream's parameters)
    pub fn check_for_adjust(&mut self, host: &impl AdjustHost, obj: &PfObjectFacts, loco: PfLocomotorSet, is_human: bool, x: i32, y: i32, layer: PathfindLayer, radius: i32, center: bool, dest: &mut Coord3D, group_dest: Option<&Coord3D>) -> bool {
        let Some(mut adjust) = self.adjust_candidate(host, obj, loco.valid_surfaces, is_human, x, y, layer, radius, center, dest) else { return false };
        if let Some(gd) = group_dest {
            self.tighten_path(host, obj, loco, &mut adjust, gd);
            // "Check to see if it is a long way to get to the adjusted destination."
            let cost = self.check_path_cost(host, Some(obj), loco, gd, &adjust);
            let dx = (gd.x - adjust.x).abs() as i32;
            let dy = (gd.y - adjust.y).abs() as i32;
            if 1.4 * ((dx + dy) as f32) < cost as f32 && !plant(3131) {
                return false;
            }
        }
        *dest = adjust;
        true
    }

    /// The adjust family's square spiral from `cell` (see `movement`'s `spiral`), mutating.
    fn spiral_mut(&mut self, cell: ICoord2D, mut try_cell: impl FnMut(&mut Self, i32, i32) -> bool) -> bool {
        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let (mut i, mut j) = (cell.x, cell.y);
        if try_cell(self, i, j) {
            return true;
        }
        let mut delta = 1;
        while limit > 0 {
            for (di, dj, grow) in [(1, 0, plant(3134)), (0, 1, true), (-1, 0, false), (0, -1, true)] {
                for _ in 0..delta {
                    i += di;
                    j += dj;
                    limit -= 1;
                    if try_cell(self, i, j) {
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

    /// The cell an adjustment starts from: the destination (half a cell on for an uncentred unit), clamped; and whether it was outside.
    fn adjust_start(&self, dest: &Coord3D, center: bool) -> (ICoord2D, bool) {
        let mut a = *dest;
        if !center {
            a.x += PATHFIND_CELL_SIZE_F / 2.0;
            a.y += PATHFIND_CELL_SIZE_F / 2.0;
        }
        self.world_to_cell(&a)
    }

    /// `adjustDestination(obj, locomotorSet, dest, groupDest)`: see the module's notes (a projectile goes anywhere).
    pub fn adjust_destination(&mut self, host: &impl AdjustHost, obj: &PfObjectFacts, loco: PfLocomotorSet, dest: &mut Coord3D, group_dest: Option<&Coord3D>) -> bool {
        if obj.projectile {
            return true; // "missiles can go wherever they want to."
        }
        let is_human = host.is_human(obj.id);
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let (cell, _) = self.adjust_start(dest, center);
        let mut layer = host.layer_for_destination(dest);
        if let Some(gd) = group_dest {
            layer = host.layer_for_destination(gd);
        }
        let found = self.spiral_mut(cell, |pf, i, j| pf.check_for_adjust(host, obj, loco, is_human, i, j, layer, radius, center, dest, group_dest));
        if found {
            return true;
        }
        if group_dest.is_some() {
            return self.adjust_destination(host, obj, loco, dest, None);
        }
        false
    }

    /// `checkForTarget(obj, cellX, cellY, weapon, victim, victimPos, iRadius, center, dest)`.
    #[allow(clippy::too_many_arguments)]
    fn check_for_target(&self, host: &impl AdjustHost, obj: &PfObjectFacts, x: i32, y: i32, victim: Option<ObjectId>, victim_pos: &Coord3D, radius: i32, center: bool, dest: &mut Coord3D) -> bool {
        if !self.check_destination(host, Some(obj), x, y, LAYER_GROUND, radius, center) {
            return false;
        }
        let mut adjust = Coord3D::default();
        Self::adjust_coord_to_cell(host, x, y, center, &mut adjust, LAYER_GROUND);
        if host.goal_pos_within_attack_range(obj.id, &adjust, victim, victim_pos) {
            *dest = adjust;
            return true;
        }
        false
    }

    /// `adjustTargetDestination(obj, target, targetPos, weapon, dest)` (the host answers for the weapon): false when the destination is
    /// off the map.
    #[allow(clippy::too_many_arguments)]
    pub fn adjust_target_destination(&mut self, host: &impl AdjustHost, obj: &PfObjectFacts, target: Option<ObjectId>, target_pos: &Coord3D, dest: &mut Coord3D) -> bool {
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let (cell, outside) = self.adjust_start(dest, center);
        if outside && !plant(3137) {
            return false; // "outside of bounds."
        }
        self.spiral_mut(cell, |pf, i, j| pf.check_for_target(host, obj, i, j, target, target_pos, radius, center, dest))
    }

    /// `checkForPossible(isCrusher, fromZone, center, locomotorSet, cellX, cellY, layer, dest, startingInObstacle)`.
    #[allow(clippy::too_many_arguments)]
    fn check_for_possible(&self, host: &impl AdjustHost, crusher: bool, from_zone: i32, center: bool, loco: PfLocomotorSet, x: i32, y: i32, layer: PathfindLayer, dest: &mut Coord3D, in_obstacle: bool) -> bool {
        let Some(goal) = self.get_cell(layer, x, y) else { return false };
        if is_impassable(self.store.cells[goal as usize].get_type()) {
            return false;
        }
        let mut zone2 = self.zone_manager.get_effective_zone(loco.valid_surfaces, crusher, self.store.cells[goal as usize].get_zone());
        if in_obstacle && !plant(3136) {
            zone2 = self.zone_manager.get_effective_terrain_zone(zone2);
        }
        if from_zone == i32::from(zone2) {
            Self::adjust_coord_to_cell(host, x, y, center, dest, layer);
            return true;
        }
        false
    }

    /// `adjustToPossibleDestination(obj, locomotorSet, dest)`: see the module's notes.
    pub fn adjust_to_possible_destination(&mut self, host: &impl AdjustHost, obj: &PfObjectFacts, loco: PfLocomotorSet, dest: &mut Coord3D) -> bool {
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let (goal_ndx, outside) = self.adjust_start(dest, center);
        if outside {
            return false; // "outside of bounds."
        }
        let destination_layer = host.layer_for_destination(dest);
        let goal = self.get_cell(destination_layer, goal_ndx.x, goal_ndx.y).expect("within the extent");
        let parent = match self.get_clipped_cell(obj.layer, &obj.pos) {
            Some(p) => p,
            None => return false,
        };
        let crusher = host.crusher_level(obj.id) > 0;
        let zm = &self.zone_manager;
        let mut zone1 = zm.get_effective_zone(loco.valid_surfaces, crusher, self.store.cells[parent as usize].get_zone());
        let is_obstacle = self.store.cells[parent as usize].get_type() == CELL_OBSTACLE;
        if is_obstacle {
            zone1 = zm.get_effective_zone(loco.valid_surfaces, crusher, zm.get_effective_terrain_zone(zone1));
        }
        let zone2 = zm.get_effective_zone(loco.valid_surfaces, crusher, self.store.cells[goal as usize].get_zone());
        if zone1 == zone2 && self.check_destination(host, Some(obj), goal_ndx.x, goal_ndx.y, destination_layer, radius, center) {
            return true;
        }
        // (the spiral without its first cell: upstream starts at the first step)
        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let (mut i, mut j) = (goal_ndx.x, goal_ndx.y);
        let mut delta = 1;
        while limit > 0 {
            for (di, dj, grow) in [(1, 0, false), (0, 1, true), (-1, 0, false), (0, -1, true)] {
                for _ in 0..delta {
                    i += di;
                    j += dj;
                    limit -= 1;
                    if self.check_for_possible(host, crusher, i32::from(zone1), center, loco, i, j, destination_layer, dest, is_obstacle)
                        && self.check_destination(host, Some(obj), i, j, destination_layer, radius, center)
                    {
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

    /// The eight-neighbour expansion of `pathDestination` and `checkPathCost`: a valid cell (a diagonal past a valid orthogonal one),
    /// priced by `costSoFar` and the octile estimate; false when an info cannot be allocated (upstream returns there).
    fn expand_plain(&mut self, crusher: bool, loco: PfLocomotorSet, parent: CellId, goal: CellId, cell_count: &mut i32) -> bool {
        const DELTA: [(i32, i32); 8] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)];
        const ADJACENT: [usize; 5] = [0, 1, 2, 3, 0];
        let mut flags = [false; 8];
        let pxy = self.cell_xy(parent);
        let layer = self.store.cells[parent as usize].get_layer();
        for i in 0..8 {
            flags[i] = false;
            let coord = ICoord2D { x: pxy.x + DELTA[i].0, y: pxy.y + DELTA[i].1 };
            let Some(new_cell) = self.get_cell(layer, coord.x, coord.y) else { continue };
            let on_list = self.store.cells[new_cell as usize].has_info() && (self.store.get_open(new_cell) || self.store.get_closed(new_cell));
            if i >= 4 && !flags[ADJACENT[i - 4]] && !flags[ADJACENT[i - 3]] {
                continue;
            }
            if !self.valid_movement_position(crusher, loco.valid_surfaces, Some(new_cell)) {
                continue;
            }
            flags[i] = true;
            if !self.store.allocate_info(new_cell, coord) {
                return false;
            }
            *cell_count += 1;
            let new_cost = self.store.cost_so_far(new_cell, Some(parent));
            self.store.set_blocked_by_ally(new_cell, false);
            if on_list && self.store.get_cost_so_far(new_cell) <= new_cost {
                continue;
            }
            self.store.set_parent_cell(new_cell, parent);
            self.store.set_cost_so_far(new_cell, new_cost);
            let remaining = self.store.cost_to_goal(new_cell, goal);
            let total = self.store.get_cost_so_far(new_cell).wrapping_add(remaining);
            self.store.set_total_cost(new_cell, total);
            if self.store.get_closed(new_cell) {
                self.closed_list = self.store.remove_from_closed_list(new_cell, self.closed_list);
            }
            if self.store.get_open(new_cell) {
                self.open_list = self.store.remove_from_open_list(new_cell, self.open_list);
            }
            self.open_list = self.store.put_on_sorted_open_list(new_cell, self.open_list);
        }
        true
    }

    /// `pathDestination(obj, locomotorSet, dest, layer, groupDest)`: see the module's notes; the return as upstream's (`Bool` from a
    /// cell count where upstream returns one).
    pub fn path_destination(&mut self, host: &impl AdjustHost, obj: Option<&PfObjectFacts>, loco: PfLocomotorSet, dest: &Coord3D, layer: PathfindLayer, group_dest: &Coord3D) -> bool {
        if !self.is_map_ready {
            return false;
        }
        let Some(obj) = obj else { return false };
        let mut cell_count = 0;
        let to = *group_dest;
        let crusher = host.crusher_level(obj.id) > 0;
        let desired_layer = host.layer_for_destination(dest);
        let Some(desired) = self.get_clipped_cell(desired_layer, dest) else { return false };
        let goal_layer = host.layer_for_destination(&to);
        let Some(goal) = self.get_clipped_cell(goal_layer, &to) else { return false };
        let is_human = host.is_human(obj.id);
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let (start, _) = self.world_to_cell(dest);
        let Some(mut parent) = self.get_cell(layer, start.x, start.y) else { return false };
        let (pos2d, _) = self.world_to_cell(&to);
        if !self.store.allocate_info(goal, pos2d) {
            return false;
        }
        if parent != goal && !self.store.allocate_info(parent, start) {
            self.store.release_info(desired);
            self.store.release_info(goal);
            return false;
        }
        if !self.valid_movement_position(crusher, loco.valid_surfaces, Some(parent)) {
            self.store.release_info(parent);
            self.store.release_info(goal);
            return false;
        }
        let mut closest_dist_sqr = f32::MAX;
        self.store.start_pathfind(parent, Some(goal));
        self.open_list = Some(parent);
        self.closed_list = None;
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            let pxy = self.cell_xy(parent);
            let mut pos = Coord3D::default();
            let player = self.store.cells[parent as usize].get_layer();
            if !self.check_for_adjust(host, obj, loco, is_human, pxy.x, pxy.y, player, radius, center, &mut pos, Some(group_dest)) {
                continue;
            }
            let g = self.cell_xy(goal);
            let (dx, dy) = ((g.x - pxy.x).abs(), (g.y - pxy.y).abs());
            let dist_sqr = (dx * dx + dy * dy) as f32;
            if dist_sqr < closest_dist_sqr {
                closest_dist_sqr = dist_sqr; // (a debug build also keeps the cell and `pos` for `dest`)
            } else {
                continue;
            }
            if cell_count > MAX_CELL_COUNT {
                continue;
            }
            self.check_change_layers(parent);
            if !self.expand_plain(crusher, loco, parent, goal, &mut cell_count) {
                return cell_count != 0;
            }
        }
        self.is_tunneling = false;
        self.clean_open_and_closed_lists();
        self.store.release_info(goal);
        false
    }

    /// `tightenPath(obj, locomotorSet, from, to)`: `from` moved to the last cell on the line to `to` (on `from`'s destination layer)
    /// where the unit may stop (`tightenPathCallback`, `LineProc::TightenPath`).
    pub fn tighten_path(&self, host: &impl MovementHost, obj: &PfObjectFacts, loco: PfLocomotorSet, from: &mut Coord3D, to: &Coord3D) {
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let layer = host.layer_for_destination(from);
        let mut p = LineProc::TightenPath { obj: obj.id, surfaces: loco.valid_surfaces, layer, radius, center, found: None };
        self.iterate_cells_along_line_world(host, from, to, layer, &mut p);
        if let LineProc::TightenPath { found: Some(pos), .. } = p {
            *from = pos;
        }
    }

    /// `checkPathCost(obj, locomotorSet, from, rawTo)`: see the module's notes.
    pub fn check_path_cost(&mut self, host: &impl AdjustHost, obj: Option<&PfObjectFacts>, loco: PfLocomotorSet, from: &Coord3D, raw_to: &Coord3D) -> i32 {
        if !self.is_map_ready {
            return 0;
        }
        let Some(obj) = obj else { return MAX_COST };
        let mut cell_count = 0;
        let to = *raw_to;
        let crusher = host.crusher_level(obj.id) > 0;
        let goal_layer = host.layer_for_destination(&to);
        let Some(goal) = self.get_clipped_cell(goal_layer, &to) else { return MAX_COST };
        let (start, _) = self.world_to_cell(from);
        let from_layer = host.layer_for_destination(from);
        let Some(mut parent) = self.get_cell_at(from_layer, from) else { return MAX_COST };
        let (pos2d, _) = self.world_to_cell(&to);
        if !self.store.allocate_info(goal, pos2d) {
            return MAX_COST;
        }
        if parent != goal && !self.store.allocate_info(parent, start) {
            self.store.release_info(goal);
            return MAX_COST;
        }
        if !self.valid_movement_position(crusher, loco.valid_surfaces, Some(parent)) {
            self.store.release_info(parent);
            self.store.release_info(goal);
            return MAX_COST;
        }
        self.store.start_pathfind(parent, Some(goal));
        self.open_list = Some(parent);
        self.closed_list = None;
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            if parent == goal {
                let cost = self.store.get_total_cost(parent) as i32;
                self.is_tunneling = false;
                self.clean_open_and_closed_lists();
                return cost;
            }
            if cell_count > if plant(3132) { MAX_CELL_COUNT * 2 } else { MAX_CELL_COUNT } {
                continue;
            }
            self.check_change_layers(parent);
            if !self.expand_plain(crusher, loco, parent, goal, &mut cell_count) {
                return cell_count;
            }
        }
        self.is_tunneling = false;
        if self.store.cells[goal as usize].has_info() && !self.store.get_closed(goal) && !self.store.get_open(goal) {
            self.store.release_info(goal);
        }
        self.clean_open_and_closed_lists();
        MAX_COST
    }

    /// `findClosestPath(obj, locomotorSet, from, rawTo, blocked, pathCostMultiplier, moveAllies)`: see the module's notes; `raw_to`
    /// becomes the closest cell's centre when the goal is not reached.
    #[allow(clippy::too_many_arguments)]
    pub fn find_closest_path(&mut self, host: &impl AdjustHost, obj: Option<&PfObjectFacts>, loco: PfLocomotorSet, from: &Coord3D, raw_to: &mut Coord3D, blocked: bool, path_cost_multiplier: f32) -> Option<Path> {
        let is_human = obj.is_none_or(|o| host.is_human(o.id));
        if loco.valid_surfaces == 0 {
            return None; // "Attempting to path immobile unit."
        }
        if !self.is_map_ready {
            return None;
        }
        self.is_tunneling = false;
        let obj = obj?;
        let can_path_through_units = obj.ai.is_some() && host.can_path_through_units(obj.id);
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let mut to = *raw_to;
        if !center {
            to.x += PATHFIND_CELL_SIZE_F / 2.0;
            to.y += PATHFIND_CELL_SIZE_F / 2.0;
        }
        let crusher = host.crusher_level(obj.id) > 0;
        let mut clip_from = *from;
        self.clip(&mut clip_from, &mut to);
        let destination_layer = host.layer_for_destination(&to);
        let goal = self.get_clipped_cell(destination_layer, &to)?;
        if self.store.cells[goal as usize].get_zone() == 0 && destination_layer == LAYER_WALL {
            return None;
        }
        let mut goal_on_obstacle = false;
        if self.ignore_obstacle_id != INVALID_ID {
            if let Some(goal_obj) = host.object(self.ignore_obstacle_id) {
                let ignore_cell = self.get_clipped_cell(goal_obj.layer, &goal_obj.pos).expect("a clipped cell");
                let goal_obstacle = self.store.get_obstacle_id(goal);
                if goal_obstacle == self.store.get_obstacle_id(ignore_cell) && goal_obstacle != INVALID_ID {
                    if host.object(goal_obstacle).is_some_and(|o| o.fs_airfield) {
                        self.ignore_obstacle_id = goal_obstacle;
                        goal_on_obstacle = true;
                    } else if self.ignore_obstacle_id == goal_obstacle {
                        goal_on_obstacle = true;
                    }
                }
            }
        }
        let (start, _) = self.world_to_cell(from);
        let mut parent = self.get_clipped_cell(obj.layer, &clip_from)?;
        if !self.valid_movement_position(crusher, loco.valid_surfaces, Some(parent)) {
            self.is_tunneling = true; // "We can't move from our current location.  So relax the constraints."
        }
        let mut info = CheckMovementInfo {
            cell: start,
            layer: obj.layer,
            center_in_cell: center,
            radius,
            consider_transient: blocked,
            acceptable_surfaces: loco.valid_surfaces,
            ..CheckMovementInfo::default()
        };
        if !self.check_for_movement(host, Some(obj), &mut info) || info.enemy_fixed {
            self.is_tunneling = true;
        }
        let mut got_hierarchical = false;
        if self.is_tunneling {
            self.zone_manager.set_all_passable(); // "can't optimize."
        } else {
            self.zone_manager.clear_passable_flags();
            if self.find_closest_hierarchical_path(host, is_human, loco.valid_surfaces, from, raw_to, false).is_some() {
                got_hierarchical = true;
            } else {
                self.zone_manager.set_all_passable();
            }
        }
        let started_stuck = self.is_tunneling;
        let (pos2d, _) = self.world_to_cell(&to);
        if !self.store.allocate_info(goal, pos2d) {
            return None;
        }
        if parent != goal {
            let (pos2d, _) = self.world_to_cell(&clip_from);
            if !self.store.allocate_info(parent, pos2d) {
                return None;
            }
        }
        self.store.start_pathfind(parent, Some(goal));
        let mut closest: Option<CellId> = None;
        let mut closest_dist_sqr = f32::MAX;
        let mut closest_screen_sqr = f32::MAX;
        self.open_list = Some(parent);
        self.closed_list = None;
        let mut count = 0;
        let mut found_goal = false;
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            let pxy = self.cell_xy(parent);
            let player = self.store.cells[parent as usize].get_layer();
            if parent == goal {
                if !goal_on_obstacle && closest.is_some() && !can_path_through_units && !self.check_destination(host, Some(obj), pxy.x, pxy.y, player, radius, center) {
                    found_goal = true;
                    continue;
                }
                self.is_tunneling = false;
                let path = self.build_actual_path(host, Some(obj), loco.valid_surfaces, from, goal, center, blocked);
                self.store.release_info(parent);
                self.store.release_info(goal);
                self.clean_open_and_closed_lists();
                return Some(path);
            }
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            let g = self.cell_xy(goal);
            if !self.is_tunneling && self.check_destination(host, Some(obj), pxy.x, pxy.y, player, radius, center) && (!started_stuck || self.valid_movement_position(crusher, loco.valid_surfaces, Some(parent))) {
                let (dx, dy) = ((g.x - pxy.x).abs() as f32, (g.y - pxy.y).abs() as f32);
                let mut dist_sqr = dx * dx + dy * dy;
                if dist_sqr < closest_screen_sqr {
                    closest_screen_sqr = dist_sqr;
                }
                let so_far = self.store.get_cost_so_far(parent) as f32;
                dist_sqr += (so_far * (so_far * COST_TO_DISTANCE_FACTOR_SQR)) * if plant(3133) { 1.0 } else { path_cost_multiplier };
                if dist_sqr < closest_dist_sqr {
                    closest = Some(parent);
                    closest_dist_sqr = dist_sqr;
                }
            }
            let (dx, dy) = ((g.x - pxy.x).abs() as f32, (g.y - pxy.y).abs() as f32);
            let dist_sqr = dx * dx + dy * dy;
            if dist_sqr > closest_screen_sqr * 4.0 {
                let skip = !got_hierarchical || count > 2000 || closest_screen_sqr < if plant(3135) { 10.0 * 10.0 } else { 10.0 * 10.0 * PATHFIND_CELL_SIZE_F };
                if skip {
                    continue;
                }
            }
            if !found_goal {
                self.check_change_layers(parent);
                count += self.examine_neighboring_cells(host, parent, Some(goal), loco, is_human, center, radius, start, Some(obj), NO_ATTACK);
            }
        }
        if let Some(c) = closest {
            self.is_tunneling = false;
            let cxy = self.cell_xy(c);
            raw_to.x = cxy.x as f32 * PATHFIND_CELL_SIZE_F + if plant(3138) { 0.0 } else { PATHFIND_CELL_SIZE_F / 2.0 };
            raw_to.y = cxy.y as f32 * PATHFIND_CELL_SIZE_F + PATHFIND_CELL_SIZE_F / 2.0;
            let path = self.build_actual_path(host, Some(obj), loco.valid_surfaces, from, c, center, blocked);
            self.store.release_info(goal);
            self.clean_open_and_closed_lists();
            return Some(path);
        }
        self.is_tunneling = false;
        self.store.release_info(goal);
        self.clean_open_and_closed_lists();
        None
    }
}
