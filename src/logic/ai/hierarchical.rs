// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: findPath, 6388-6460; buildHierachicalPath, 6837-6899;
//     the hierarchical search, 7346-7974; findBrokenBridge and the quick checks, 7976-8155)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The Pathfinder's hierarchical search and `findPath`. `p1w13-sonnet`, layer 10, batch P7.
//!
//! `findPath` asks the quick zone check (`clientSafeQuickDoesPathExist`), then marks the zone blocks a hierarchical search passes
//! through as passable (all of them when it fails) so the cell search (`internalFindPath`) stays in them. The hierarchical search
//! walks zone blocks: from a cell, the cells along each side of its block, out from the middle, step into the neighbouring block where
//! the two blocks' zones join (`processHierarchicalCell`), at most once per neighbouring zone; a bridge's two ends join the blocks it
//! spans. A block of the goal's zone (the goal's own block on the ground) ends it; `findClosestHierarchicalPath` settles for the cell
//! nearest the goal.
//!
//! # Upstream facts kept
//!
//! - `clientSafeQuickDoesPathExist` treats a start in an obstacle with no zone as a path; with an obstacle at either end it compares
//!   the terrain zones, through the effective zone and back to the terrain zone.
//! - The hierarchical search's side scans stop at the first cell already searched in the parent's zone; the zones a side examined are
//!   kept for the left side across the other sides only for the right, top and bottom ones (each resets them; the left one does not
//!   need to: it is first).
//! - `processHierarchicalCell` needs the scanned cell to hold an info already (a unit's or an obstacle's cell, or one an earlier step
//!   of the search gave one) before it allocates one; elsewhere the scan does not cross. Kept.
//! - A bridge start block's search skips the bridge's start cell when it is the parent itself.
//! - `buildHierachicalPath` marks passable the blocks of the cells within one block of the path's first node.
//!
//! # Upstream defects kept
//!
//! - A search from a bridge's cell to that same cell (two ends within a cell, or an end `clip` moves onto the other's cell) moves the
//!   cell to the closed list and opens the bridge's two ground ends with it
//!   as their parent. If an end's BLOCK zone number happens to equal the bridge LAYER's zone number (two kinds of zone number,
//!   compared), the search takes it for the goal, makes it the goal's parent, and the parents loop: `prependCells` never returns,
//!   upstream as here (found by the differential tests, which keep clear of it).
//! - A short bridge (both ends in one zone block) can put the cell the search popped back on the open list as the bridge's end while
//!   it also goes on the closed list; the lists then cycle or crash, upstream as here.

use crate::common::base_type::{real_to_int, Coord3D, ICoord2D};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND, LAYER_LAST};
use crate::logic::ai::cell::{CellId, CELL_CLIFF, CELL_OBSTACLE, COST_ORTHOGONAL};
use crate::logic::ai::layer::{PATHFIND_CELL_SIZE, PATHFIND_CELL_SIZE_F};
use crate::logic::ai::movement::MovementHost;
use crate::logic::ai::path::Path;
use crate::logic::ai::pathfinder::{Pathfinder, PfObjectFacts};
use crate::logic::ai::search::{PfLocomotorSet, SearchHost};
use crate::logic::ai::zone::{ZoneStorage, UNINITIALIZED_ZONE, ZONE_BLOCK_SIZE};
use crate::logic::locomotor::{LocomotorSurfaceTypeMask, LOCOMOTORSURFACE_GROUND};
use crate::logic::modules::plant;

/// `HUGE_DIST` (PartitionManager.h).
const HUGE_DIST: f32 = 1_000_000.0;

impl Pathfinder {
    fn valid_at_pos(&self, crusher: bool, layer: PathfindLayer, surfaces: LocomotorSurfaceTypeMask, p: &Coord3D) -> bool {
        // (the header's position overload: `REAL_TO_INT`, truncating)
        self.valid_movement_position_at(crusher, layer, surfaces, real_to_int(p.x / PATHFIND_CELL_SIZE as f32), real_to_int(p.y / PATHFIND_CELL_SIZE as f32))
    }

    /// The terrain zone through the effective zone and back: `getEffectiveTerrainZone(getEffectiveZone(getEffectiveTerrainZone(z)))`.
    fn terrain_round_trip(&self, surfaces: LocomotorSurfaceTypeMask, z: ZoneStorage) -> ZoneStorage {
        let zm = &self.zone_manager;
        zm.get_effective_terrain_zone(zm.get_effective_zone(surfaces, false, zm.get_effective_terrain_zone(z)))
    }

    /// `findBrokenBridge(locoSet, from, to, bridgeID)`: the first destroyed bridge whose repair would join the two ends' terrain zones.
    pub fn find_broken_bridge(&self, host: &impl MovementHost, surfaces: LocomotorSurfaceTypeMask, from: &Coord3D, to: &Coord3D) -> (bool, ObjectId) {
        let destination_layer = host.layer_for_destination(to);
        let from_layer = host.layer_for_destination(from);
        let parent = self.get_clipped_cell(from_layer, from).expect("a clipped cell");
        let goal = self.get_clipped_cell(destination_layer, to).expect("a clipped cell");
        let zm = &self.zone_manager;
        let z1 = zm.get_effective_zone(surfaces, false, self.store.cells[parent as usize].get_zone());
        let z2 = zm.get_effective_zone(surfaces, false, self.store.cells[goal as usize].get_zone());
        let z1 = if plant(3116) { z1 } else { zm.get_effective_zone(surfaces, false, zm.get_effective_terrain_zone(z1)) };
        let z2 = zm.get_effective_zone(surfaces, false, zm.get_effective_terrain_zone(z2));
        if z1 == z2 {
            return (false, INVALID_ID);
        }
        let ground = self.ground();
        for l in &self.layers {
            if l.is_destroyed() && l.connects_zones(&self.store, &ground, zm, surfaces, i32::from(z1), i32::from(z2)) {
                return (true, l.get_bridge_id(host));
            }
        }
        (false, INVALID_ID)
    }

    /// `clientSafeQuickDoesPathExist(locomotorSet, from, to)`: the terrain allows a path (see the module's notes).
    pub fn client_safe_quick_does_path_exist(&self, host: &impl MovementHost, surfaces: LocomotorSurfaceTypeMask, from: &Coord3D, to: &Coord3D) -> bool {
        let destination_layer = host.layer_for_destination(to);
        if !self.valid_at_pos(false, destination_layer, surfaces, to) {
            return false;
        }
        let from_layer = host.layer_for_destination(from);
        let parent = self.get_clipped_cell(from_layer, from).expect("a clipped cell");
        let goal = self.get_clipped_cell(destination_layer, to).expect("a clipped cell");
        if self.store.cells[goal as usize].get_type() == CELL_CLIFF && !plant(3112) {
            return false; // "No goals on cliffs."
        }
        let zm = &self.zone_manager;
        let mut terrain = false;
        let mut z1 = zm.get_effective_zone(surfaces, false, self.store.cells[parent as usize].get_zone());
        if self.store.cells[parent as usize].get_type() == CELL_OBSTACLE {
            terrain = true;
            if z1 == UNINITIALIZED_ZONE {
                return true;
            }
        }
        let mut z2 = zm.get_effective_zone(surfaces, false, self.store.cells[goal as usize].get_zone());
        if self.store.cells[goal as usize].get_type() == CELL_OBSTACLE {
            terrain = true;
        }
        if terrain {
            z1 = self.terrain_round_trip(surfaces, self.store.cells[parent as usize].get_zone());
            z2 = self.terrain_round_trip(surfaces, self.store.cells[goal as usize].get_zone());
        }
        z1 == z2
    }

    /// `clientSafeQuickDoesPathExistForUI(locomotorSet, from, to)`: the terrain zones only (an unzoned end allows it).
    pub fn client_safe_quick_does_path_exist_for_ui(&self, host: &impl MovementHost, surfaces: LocomotorSurfaceTypeMask, from: &Coord3D, to: &Coord3D) -> bool {
        let destination_layer = host.layer_for_destination(to);
        let from_layer = host.layer_for_destination(from);
        let parent = self.get_clipped_cell(from_layer, from).expect("a clipped cell");
        let goal = self.get_clipped_cell(destination_layer, to).expect("a clipped cell");
        if self.store.cells[goal as usize].get_type() == CELL_CLIFF {
            return false;
        }
        let zm = &self.zone_manager;
        let z1 = zm.get_effective_zone(surfaces, false, self.store.cells[parent as usize].get_zone());
        let z2 = zm.get_effective_zone(surfaces, false, self.store.cells[goal as usize].get_zone());
        if (z1 == UNINITIALIZED_ZONE || z2 == UNINITIALIZED_ZONE) && !plant(3118) {
            return true;
        }
        let z1 = self.terrain_round_trip(surfaces, self.store.cells[parent as usize].get_zone());
        let z2 = self.terrain_round_trip(surfaces, self.store.cells[goal as usize].get_zone());
        z1 == UNINITIALIZED_ZONE || z1 == z2
    }

    /// `findPath(obj, locomotorSet, from, rawTo)`: see the module's notes.
    pub fn find_path(&mut self, host: &impl SearchHost, obj: Option<&PfObjectFacts>, loco: PfLocomotorSet, from: &Coord3D, raw_to: &Coord3D) -> Option<Path> {
        if !self.client_safe_quick_does_path_exist(host, loco.valid_surfaces, from, raw_to) {
            return None;
        }
        let is_human = obj.is_none_or(|o| host.is_human(o.id));
        self.zone_manager.clear_passable_flags();
        let h = self.find_hierarchical_path(host, is_human, loco.valid_surfaces, from, raw_to, false);
        if h.is_none() || plant(3110) {
            self.zone_manager.set_all_passable();
        }
        self.internal_find_path(host, obj, loco, from, raw_to)
    }

    /// `findHierarchicalPath(isHuman, locomotorSet, from, to, crusher)`.
    pub fn find_hierarchical_path(&mut self, host: &impl MovementHost, is_human: bool, surfaces: LocomotorSurfaceTypeMask, from: &Coord3D, to: &Coord3D, crusher: bool) -> Option<Path> {
        self.internal_find_hierarchical_path(host, is_human, surfaces, from, to, crusher, false)
    }

    /// `findClosestHierarchicalPath(isHuman, locomotorSet, from, to, crusher)`.
    pub fn find_closest_hierarchical_path(&mut self, host: &impl MovementHost, is_human: bool, surfaces: LocomotorSurfaceTypeMask, from: &Coord3D, to: &Coord3D, crusher: bool) -> Option<Path> {
        self.internal_find_hierarchical_path(host, is_human, surfaces, from, to, crusher, true)
    }

    fn block_zone(&self, surfaces: LocomotorSurfaceTypeMask, crusher: bool, x: i32, y: i32) -> ZoneStorage {
        self.zone_manager.get_block_zone(&self.store, &self.ground(), surfaces, crusher, x, y)
    }

    fn in_extent(&self, c: ICoord2D) -> bool {
        let e = &self.extent;
        !(c.x < e.lo.x || c.x > e.hi.x || c.y < e.lo.y || c.y > e.hi.y)
    }

    fn xy(&self, c: CellId) -> ICoord2D {
        ICoord2D { x: i32::from(self.store.x_index(c)), y: i32::from(self.store.y_index(c)) }
    }

    /// `processHierarchicalCell(scanCell, delta, parentCell, goalCell, parentZone, examinedZones, numExZones, crusher, cellCount)`.
    #[allow(clippy::too_many_arguments)]
    fn process_hierarchical_cell(&mut self, scan: ICoord2D, delta: ICoord2D, parent: CellId, goal: CellId, parent_zone: ZoneStorage, examined: &mut Vec<ZoneStorage>, crusher: bool, cell_count: &mut i32) {
        if !self.in_extent(scan) || parent_zone == UNINITIALIZED_ZONE {
            return;
        }
        if parent_zone != self.block_zone(LOCOMOTORSURFACE_GROUND, crusher, scan.x, scan.y) {
            return;
        }
        let new_cell = self.get_cell(LAYER_GROUND, scan.x, scan.y).expect("within the extent");
        if !self.store.cells[new_cell as usize].has_info() {
            return;
        }
        if self.store.get_open(new_cell) || self.store.get_closed(new_cell) {
            return; // "already looked at this one."
        }
        if parent_zone != self.store.cells[new_cell as usize].get_zone() {
            return;
        }
        let adjacent = ICoord2D { x: scan.x + delta.x, y: scan.y + delta.y };
        if !self.in_extent(adjacent) {
            return;
        }
        let adj = self.get_cell(LAYER_GROUND, adjacent.x, adjacent.y).expect("within the extent");
        if self.store.cells[adj as usize].has_info() && (self.store.get_open(adj) || self.store.get_closed(adj)) {
            return;
        }
        let zm = &self.zone_manager;
        let parent_global = zm.get_effective_zone(LOCOMOTORSURFACE_GROUND, crusher, parent_zone);
        let new_zone = self.block_zone(LOCOMOTORSURFACE_GROUND, crusher, adjacent.x, adjacent.y);
        let new_global = self.zone_manager.get_effective_zone(LOCOMOTORSURFACE_GROUND, crusher, new_zone);
        if new_global != parent_global {
            return; // "can't step over."
        }
        if examined.contains(&new_zone) {
            return;
        }
        self.store.allocate_info(new_cell, scan);
        if !self.store.get_closed(new_cell) && !self.store.get_open(new_cell) {
            self.closed_list = self.store.put_on_closed_list(new_cell, self.closed_list);
        }
        self.store.allocate_info(adj, adjacent);
        if self.store.cells[adj as usize].has_info() {
            *cell_count += 1;
            let mut cur = self.store.cost_to_hier_goal(adj, parent) as i32;
            let rem = self.store.cost_to_hier_goal(adj, goal) as i32;
            if self.store.cells[adj as usize].get_pinched() || self.store.cells[new_cell as usize].get_pinched() {
                cur += if plant(3113) { 3 } else { 2 } * COST_ORTHOGONAL;
            } else {
                if !plant(3114) {
                    examined.push(new_zone);
                }
            }
            let so_far = self.store.get_cost_so_far(parent).wrapping_add(cur as u32);
            self.store.set_cost_so_far(adj, so_far);
            let total = self.store.get_cost_so_far(adj).wrapping_add(rem as u32);
            self.store.set_total_cost(adj, total);
            self.store.set_parent_cell_hierarchical(adj, parent);
            self.open_list = self.store.put_on_sorted_open_list(adj, self.open_list);
        }
    }

    /// `buildHierachicalPath(fromPos, goalCell)`: the cells prepended (centred), and the blocks within a block of the first node
    /// marked passable.
    fn build_hierachical_path(&mut self, host: &impl MovementHost, from: &Coord3D, goal: CellId) -> Path {
        let mut path = Path::new();
        self.prepend_cells(host, &mut path, from, goal, true);
        let pos = *path.node(path.first_node().expect("a node")).position();
        let span = if plant(3115) { 0.0 } else { ZONE_BLOCK_SIZE as f32 * PATHFIND_CELL_SIZE_F };
        let min = Coord3D { x: pos.x - span, y: pos.y - span, z: pos.z };
        let max = Coord3D { x: pos.x + span, y: pos.y + span, z: pos.z };
        let (lo, _) = self.world_to_cell(&min);
        let (hi, _) = self.world_to_cell(&max);
        for i in lo.x..=hi.x {
            for j in lo.y..=hi.y {
                self.zone_manager.set_passable(i, j, true);
            }
        }
        path
    }

    /// `internal_findHierarchicalPath(isHuman, locomotorSurface, from, rawTo, crusher, closestOK)`.
    #[allow(clippy::too_many_arguments)]
    pub fn internal_find_hierarchical_path(&mut self, host: &impl MovementHost, is_human: bool, surfaces: LocomotorSurfaceTypeMask, from: &Coord3D, raw_to: &Coord3D, crusher: bool, closest_ok: bool) -> Option<Path> {
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
        let (cell, _) = self.world_to_cell(&to);
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
        let z1 = zm.get_effective_zone(surfaces, false, self.store.cells[parent as usize].get_zone());
        let z2 = zm.get_effective_zone(surfaces, false, self.store.cells[goal as usize].get_zone());
        if z1 != z2 {
            self.store.release_info(goal);
            self.store.release_info(parent);
            return None;
        }
        self.store.start_pathfind(parent, Some(goal));
        self.closed_list = None;
        let mut cell_count = 0;
        let (goal_block_zone, goal_block) = if self.store.cells[goal as usize].get_layer() == LAYER_GROUND {
            let g = self.xy(goal);
            (self.block_zone(surfaces, crusher, g.x, g.y), ICoord2D { x: g.x / ZONE_BLOCK_SIZE, y: g.y / ZONE_BLOCK_SIZE })
        } else {
            (self.store.cells[goal as usize].get_zone(), ICoord2D { x: -1, y: -1 })
        };
        self.open_list = Some(parent);
        let parent_layer = self.store.cells[parent as usize].get_layer();
        if parent_layer != LAYER_GROUND {
            let l = &self.layers[parent_layer as usize];
            let (ndx, to_ndx) = (l.get_start_cell_index(), l.get_end_cell_index());
            let end_cell = self.get_cell(LAYER_GROUND, to_ndx.x, to_ndx.y);
            let start_cell = self.get_cell(LAYER_GROUND, ndx.x, ndx.y);
            if let (Some(c), Some(sc)) = (end_cell, start_cell) {
                self.open_list = self.store.remove_from_open_list(parent, self.open_list);
                self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
                for (cell, at) in [(sc, ndx), (c, to_ndx)] {
                    self.store.allocate_info(cell, at);
                    self.store.set_parent_cell_hierarchical(cell, parent);
                    cell_count += 1;
                    let cur = self.store.cost_to_hier_goal(cell, parent);
                    let rem = self.store.cost_to_hier_goal(cell, goal);
                    self.store.set_cost_so_far(cell, cur);
                    self.store.set_total_cost(cell, rem);
                    self.store.set_parent_cell_hierarchical(cell, parent);
                    self.open_list = self.store.put_on_sorted_open_list(cell, self.open_list);
                }
            }
        }
        let mut closest: Option<CellId> = None;
        let mut closest_dist_sqr = HUGE_DIST * HUGE_DIST;
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            let pxy = self.xy(parent);
            let parent_zone = if self.store.cells[parent as usize].get_layer() == LAYER_GROUND {
                self.block_zone(surfaces, crusher, pxy.x, pxy.y)
            } else {
                self.store.cells[parent as usize].get_zone()
            };
            let mut reached = false;
            let (block_x, block_y) = (pxy.x / ZONE_BLOCK_SIZE, pxy.y / ZONE_BLOCK_SIZE);
            if parent_zone == goal_block_zone && (goal_block.x == -1 || (block_x == goal_block.x && block_y == goal_block.y)) {
                reached = true;
            }
            let block_extent = self.zone_manager.get_extent();
            if !reached && self.zone_manager.interacts_with_bridge(pxy.x, pxy.y) {
                for i in 0..=LAYER_LAST as usize {
                    if self.layers[i].is_unused() || self.layers[i].is_destroyed() {
                        continue;
                    }
                    let (mut ndx, mut to_ndx) = (self.layers[i].get_start_cell_index(), self.layers[i].get_end_cell_index());
                    if ndx.x / ZONE_BLOCK_SIZE != block_x || ndx.y / ZONE_BLOCK_SIZE != block_y {
                        std::mem::swap(&mut ndx, &mut to_ndx);
                    }
                    if ndx.x < 0 || ndx.y < 0 || to_ndx.x < 0 || to_ndx.y < 0 {
                        continue;
                    }
                    if ndx.x / ZONE_BLOCK_SIZE != block_x || ndx.y / ZONE_BLOCK_SIZE != block_y {
                        continue;
                    }
                    if self.block_zone(surfaces, crusher, ndx.x, ndx.y) != parent_zone {
                        continue;
                    }
                    if self.layers[i].get_zone() == i32::from(goal_block_zone) {
                        reached = true;
                        break;
                    }
                    let Some(cell) = self.get_cell(LAYER_GROUND, to_ndx.x, to_ndx.y) else { continue };
                    if self.store.cells[cell as usize].has_info() && (self.store.get_closed(cell) || self.store.get_open(cell)) {
                        continue;
                    }
                    let Some(start_cell) = self.get_cell(LAYER_GROUND, ndx.x, ndx.y) else { continue };
                    if start_cell != parent {
                        self.store.allocate_info(start_cell, ndx);
                        self.store.set_parent_cell_hierarchical(start_cell, parent);
                        if !self.store.get_closed(start_cell) && !self.store.get_open(start_cell) {
                            self.closed_list = self.store.put_on_closed_list(start_cell, self.closed_list);
                        }
                    }
                    self.store.allocate_info(cell, to_ndx);
                    self.store.set_parent_cell_hierarchical(cell, start_cell);
                    cell_count += 1;
                    let cur = self.store.cost_to_hier_goal(cell, start_cell);
                    let rem = self.store.cost_to_hier_goal(cell, goal);
                    let so_far = self.store.get_cost_so_far(start_cell).wrapping_add(cur);
                    self.store.set_cost_so_far(cell, so_far);
                    let total = self.store.get_cost_so_far(cell).wrapping_add(rem);
                    self.store.set_total_cost(cell, total);
                    self.store.set_parent_cell_hierarchical(cell, start_cell);
                    self.open_list = self.store.put_on_sorted_open_list(cell, self.open_list);
                }
            }
            if reached {
                if parent != goal {
                    self.store.set_parent_cell_hierarchical(goal, parent);
                }
                self.is_tunneling = false;
                let path = self.build_hierachical_path(host, from, goal);
                if self.store.cells[goal as usize].has_info() && !self.store.get_closed(goal) && !self.store.get_open(goal) {
                    self.store.release_info(goal);
                }
                self.store.release_info(parent);
                self.clean_open_and_closed_lists();
                return Some(path);
            }
            let gxy = self.xy(goal);
            let dx = (gxy.x - pxy.x).abs() as f32;
            let dy = (gxy.y - pxy.y).abs() as f32;
            let dist_sqr = dx * dx + dy * dy;
            if dist_sqr < closest_dist_sqr || (plant(3117) && dist_sqr == closest_dist_sqr) {
                closest = Some(parent);
                closest_dist_sqr = dist_sqr;
            }
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            let mut examined: Vec<ZoneStorage> = Vec::with_capacity(ZONE_BLOCK_SIZE as usize);
            // the four sides: (has the side, the scan's fixed axis is x, the fixed coordinate, the step out)
            let half = ZONE_BLOCK_SIZE / 2;
            let sides = [
                (block_x > 0, true, block_x * ZONE_BLOCK_SIZE, ICoord2D { x: -1, y: 0 }),
                (block_x < block_extent.x - 1, true, block_x * ZONE_BLOCK_SIZE + ZONE_BLOCK_SIZE - 1, ICoord2D { x: 1, y: 0 }),
                (block_y > 0, false, block_y * ZONE_BLOCK_SIZE, ICoord2D { x: 0, y: -1 }),
                (block_y < block_extent.y - 1, false, block_y * ZONE_BLOCK_SIZE + ZONE_BLOCK_SIZE - 1, ICoord2D { x: 0, y: 1 }),
            ];
            for (has, fixed_x, fixed, delta) in sides {
                if !has {
                    continue;
                }
                examined.clear();
                for i in 1..=ZONE_BLOCK_SIZE {
                    let mut offset = i >> 1;
                    if i & 1 != 0 {
                        offset = -offset;
                    }
                    let scan = if fixed_x {
                        ICoord2D { x: fixed, y: block_y * ZONE_BLOCK_SIZE + half + offset }
                    } else {
                        ICoord2D { x: block_x * ZONE_BLOCK_SIZE + half + offset, y: fixed }
                    };
                    let Some(c) = self.get_cell(LAYER_GROUND, scan.x, scan.y) else { continue };
                    if self.store.cells[c as usize].has_info() && (self.store.get_closed(c) || self.store.get_open(c)) && parent_zone == self.block_zone(surfaces, crusher, scan.x, scan.y) && !plant(3111) {
                        break;
                    }
                    if is_human {
                        let e = &self.logical_extent;
                        if scan.x < e.lo.x || scan.x > e.hi.x || scan.y < e.lo.y || scan.y > e.hi.y {
                            continue;
                        }
                    }
                    self.process_hierarchical_cell(scan, delta, parent, goal, parent_zone, &mut examined, crusher, &mut cell_count);
                }
            }
        }
        if closest_ok {
            if let Some(c) = closest {
                self.is_tunneling = false;
                let path = self.build_hierachical_path(host, from, c);
                if self.store.cells[goal as usize].has_info() && !self.store.get_closed(goal) && !self.store.get_open(goal) {
                    self.store.release_info(goal);
                }
                self.clean_open_and_closed_lists();
                return Some(path);
            }
        }
        self.is_tunneling = false;
        self.store.release_info(goal);
        self.clean_open_and_closed_lists();
        None
    }
}
