// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: buildGroundPath, 6789-6835; groundCellsCallback and
//     findGroundPath, 6950-7344)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The Pathfinder's ground search: `findGroundPath`, a path of a given diameter over clear ground (the paths of wide vehicles and
//! formations). `p1w13-sonnet`, layer 10, batch P8.
//!
//! A hierarchical search over the ground opens the zone blocks (all of them when it fails). The goal moves, if it is not clear for the
//! diameter, to the first cell of a square ring around it that is (rings of 1 to 7 cells: right, down, left 2, up 2, right 2); the
//! start and the goal must share a ground zone. Then A*: the straight line of clear cells toward the goal first
//! (`groundCellsCallback`), then the eight neighbours, only clear unpinched cells (a diagonal past an open orthogonal one), each
//! costing `costSoFar` plus 0.6 times 10 per cell of diameter missing; a closed zone block is entered only when a block 3 cells away
//! diagonally is open. The path is the parents back to the start, optimized for the ground (`Path::optimizeGroundPath`).
//!
//! # Upstream facts kept
//!
//! - The goal itself is taken as a neighbour without the clear and pinch tests, at the cost the loop's previous neighbour computed (a
//!   variable left from the last iteration; zero when the goal is the parent's first neighbour, as in the differential build, which
//!   zeroes locals: upstream's uninitialised read).
//! - A neighbour already on a list is re-priced only when cheaper; a neighbour's info is allocated before that test.
//! - `centerInCell` is false: the path's cells are their corners plus 0.05.

use crate::common::base_type::{Coord3D, ICoord2D};
use crate::logic::ai::cell::{CellId, CELL_CLEAR, COST_ORTHOGONAL};
use crate::logic::ai::movement::MovementHost;
use crate::logic::ai::path::Path;
use crate::logic::ai::pathfinder::Pathfinder;
use crate::logic::locomotor::LOCOMOTORSURFACE_GROUND;
use crate::logic::modules::plant;
use crate::common::game_type::PathfindLayer;

impl Pathfinder {
    /// `buildGroundPath(isCrusher, fromPos, goalCell, center, pathDiameter)`: the cells prepended, then `optimizeGroundPath`.
    pub fn build_ground_path(&mut self, host: &impl MovementHost, crusher: bool, from: &Coord3D, goal: CellId, center: bool, diameter: i32) -> Path {
        let mut path = Path::new();
        self.prepend_cells(host, &mut path, from, goal, center);
        let mut view = crate::logic::ai::search::PathfinderView { pf: self, host };
        if !plant(3126) {
            path.optimize_ground_path(&mut view, crusher, diameter);
        }
        path
    }

    /// `groundCellsCallback`: open a clear cell of the straight line toward the goal.
    #[allow(clippy::too_many_arguments)] // (upstream's callback arguments and its userData)
    fn ground_cells_callback(&mut self, host: &impl MovementHost, crusher: bool, diameter: i32, goal: CellId, from: Option<CellId>, to: CellId, to_x: i32, to_y: i32) -> i32 {
        let Some(from) = from else { return 0 };
        if self.store.cells[to as usize].has_info() && (self.store.get_open(to) || self.store.get_closed(to)) {
            return 1; // "abort."
        }
        let layer = self.store.cells[to as usize].get_layer();
        if self.clear_cell_for_diameter(host, crusher, to_x, to_y, layer, diameter) != diameter {
            return 1;
        }
        if !self.store.allocate_info(to, ICoord2D { x: to_x, y: to_y }) {
            return 1;
        }
        // (a float, truncated)
        let new_cost = (self.store.get_cost_so_far(from) as f32 + if plant(3124) { 0.6 } else { 0.5 } * COST_ORTHOGONAL as f32) as u32;
        self.store.set_blocked_by_ally(to, false);
        let remaining = self.store.cost_to_goal(to, goal);
        self.store.set_cost_so_far(to, new_cost);
        self.store.set_parent_cell(to, from);
        let total = self.store.get_cost_so_far(to).wrapping_add(remaining);
        self.store.set_total_cost(to, total);
        self.open_list = self.store.put_on_sorted_open_list(to, self.open_list);
        0
    }

    /// `iterateCellsAlongLine` with `groundCellsCallback` (the walk of [`Pathfinder::iterate_cells_along_line`], mutating).
    #[allow(clippy::too_many_arguments)]
    fn ground_line(&mut self, host: &impl MovementHost, start: ICoord2D, end: ICoord2D, layer: PathfindLayer, crusher: bool, diameter: i32, goal: CellId) {
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
            if self.ground_cells_callback(host, crusher, diameter, goal, from, to, x, y) != 0 {
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
                if self.ground_cells_callback(host, crusher, diameter, goal, from, to, x, y) != 0 {
                    return;
                }
            }
            x += xinc2;
            y += yinc2;
            from = Some(to);
        }
    }

    /// `findGroundPath(from, rawTo, pathDiameter, crusher)`: see the module's notes.
    pub fn find_ground_path(&mut self, host: &impl MovementHost, from: &Coord3D, raw_to: &Coord3D, diameter: i32, crusher: bool) -> Option<Path> {
        let center = false;
        self.zone_manager.clear_passable_flags();
        let h = self.internal_find_hierarchical_path(host, true, LOCOMOTORSURFACE_GROUND, from, raw_to, false, false);
        if h.is_none() {
            self.zone_manager.set_all_passable();
        }
        if raw_to.x == 0.0 && raw_to.y == 0.0 {
            return None; // "Attempting pathfind to 0,0, generally a bug."
        }
        if !self.is_map_ready {
            return None;
        }
        let mut to = *raw_to;
        let mut clip_from = *from;
        self.clip(&mut clip_from, &mut to);
        self.is_tunneling = false;
        let destination_layer = host.layer_for_destination(&to);
        let (mut cell, _) = self.world_to_cell(&to);
        let clear_at = |pf: &Self, c: ICoord2D| pf.clear_cell_for_diameter(host, crusher, c.x, c.y, destination_layer, diameter) == diameter;
        if !clear_at(self, cell) {
            const MAX_OFFSET: i32 = 8;
            let mut offset = 1;
            'ring: while offset < MAX_OFFSET {
                let saved = cell;
                let ring = if plant(3122) { [(0, 1), (1, 0), (-1, 0), (-1, 0), (0, -1), (0, -1), (1, 0), (1, 0)] } else { [(1, 0), (0, 1), (-1, 0), (-1, 0), (0, -1), (0, -1), (1, 0), (1, 0)] };
                for (dx, dy) in ring {
                    cell.x += dx * offset;
                    cell.y += dy * offset;
                    if clear_at(self, cell) {
                        break 'ring;
                    }
                }
                offset += 1;
                cell = saved;
            }
            if offset >= MAX_OFFSET {
                return None;
            }
        }
        let goal = self.get_cell(destination_layer, cell.x, cell.y)?;
        if !self.store.allocate_info(goal, cell) {
            return None;
        }
        let layer = host.layer_for_destination(from);
        let mut parent = self.get_clipped_cell(layer, &clip_from)?;
        if parent != goal {
            let (start, _) = self.world_to_cell(&clip_from);
            if !self.store.allocate_info(parent, start) {
                self.store.release_info(goal);
                return None;
            }
        }
        let zm = &self.zone_manager;
        let z1 = zm.get_effective_zone(LOCOMOTORSURFACE_GROUND, false, self.store.cells[parent as usize].get_zone());
        let z2 = zm.get_effective_zone(LOCOMOTORSURFACE_GROUND, false, self.store.cells[goal as usize].get_zone());
        if z1 != z2 {
            self.store.release_info(goal);
            self.store.release_info(parent);
            return None;
        }
        self.store.start_pathfind(parent, Some(goal));
        self.open_list = Some(parent);
        self.closed_list = None;
        const DELTA: [(i32, i32); 8] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)];
        const FIRST_DIAGONAL: usize = 4;
        const ADJACENT: [usize; 5] = [0, 1, 2, 3, 0];
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            if parent == goal {
                self.is_tunneling = false;
                let path = self.build_ground_path(host, crusher, from, goal, center, diameter);
                self.store.release_info(parent);
                self.clean_open_and_closed_lists();
                return Some(path);
            }
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            self.check_change_layers(parent);
            let pxy = ICoord2D { x: i32::from(self.store.x_index(parent)), y: i32::from(self.store.y_index(parent)) };
            let gxy = ICoord2D { x: i32::from(self.store.x_index(goal)), y: i32::from(self.store.y_index(goal)) };
            let parent_layer = self.store.cells[parent as usize].get_layer();
            self.ground_line(host, pxy, gxy, parent_layer, crusher, diameter, goal);
            let mut neighbor_flags = [false; 8];
            // (upstream's `newCostSoFar`, declared per parent and read for the goal without being set: see the module's notes)
            let mut new_cost: u32 = 0;
            for i in 0..DELTA.len() {
                if plant(3123) {
                    new_cost = 0;
                }
                neighbor_flags[i] = false;
                let coord = ICoord2D { x: pxy.x + DELTA[i].0, y: pxy.y + DELTA[i].1 };
                let Some(new_cell) = self.get_cell(parent_layer, coord.x, coord.y) else { continue };
                if self.store.cells[new_cell as usize].get_layer() == crate::common::game_type::LAYER_GROUND && !self.zone_manager.is_passable(coord.x, coord.y) {
                    let zm = &self.zone_manager;
                    let passable = [(3, 3), (-3, 3), (3, -3), (-3, -3)].iter().any(|&(dx, dy)| zm.clip_is_passable(coord.x + dx, coord.y + dy));
                    if !passable && !plant(3120) {
                        continue;
                    }
                }
                let on_list = self.store.cells[new_cell as usize].has_info() && (self.store.get_open(new_cell) || self.store.get_closed(new_cell));
                if new_cell != goal {
                    if i >= FIRST_DIAGONAL && !neighbor_flags[ADJACENT[i - 4]] && !neighbor_flags[ADJACENT[i - 3]] {
                        continue;
                    }
                    let new_layer = self.store.cells[new_cell as usize].get_layer();
                    let clear_diameter = self.clear_cell_for_diameter(host, crusher, coord.x, coord.y, new_layer, diameter);
                    if self.store.cells[new_cell as usize].get_type() != CELL_CLEAR || (self.store.cells[new_cell as usize].get_pinched() && !plant(3125)) {
                        continue;
                    }
                    neighbor_flags[i] = true;
                    if !self.store.allocate_info(new_cell, coord) {
                        continue;
                    }
                    new_cost = self.store.cost_so_far(new_cell, Some(parent));
                    if clear_diameter < diameter {
                        let delta = diameter - clear_diameter;
                        // (a float, truncated)
                        new_cost = (new_cost as f32 + if plant(3121) { 0.5 } else { 0.6 } * (delta * COST_ORTHOGONAL) as f32) as u32;
                    }
                    self.store.set_blocked_by_ally(new_cell, false);
                }
                let remaining = self.store.cost_to_goal(new_cell, goal);
                if on_list && self.store.get_cost_so_far(new_cell) <= new_cost {
                    continue;
                }
                self.store.set_cost_so_far(new_cell, new_cost);
                self.store.set_parent_cell(new_cell, parent);
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
        }
        self.is_tunneling = false;
        self.store.release_info(goal);
        self.clean_open_and_closed_lists();
        None
    }
}
