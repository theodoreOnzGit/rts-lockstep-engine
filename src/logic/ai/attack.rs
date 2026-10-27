// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: getMoveAwayFromPath, patchPath, findAttackPath and
//     findSafePath, 10195-11066)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The Pathfinder's searches for a purpose: to attack, to be safe, to patch a path, to clear another's path. `p1w13-sonnet`, layer
//! 10, batch P10.
//!
//! - `findAttackPath`: up to nine half-cell steps straight toward the victim, while the unit may stop there, for a spot in range
//!   with a clear view; else A* (2,500 cells' worth of neighbours, the hierarchical search's blocks first) for the first cell in range
//!   where the unit may stop, with a clear view from a cell not the start's nor within half a cell of the unit. A vehicle stops short
//!   of the last cell on the last 12 of the way that is blocked (by a fixed enemy, an ally that is not idle, or the cells' movement).
//! - `findSafePath`: A* outward for the first cell (where the unit may stop) farther than the repulsors' radius from both, or the
//!   farthest so far once 2,000 cells are counted, or the last when the search runs out.
//! - `patchPath`: back along the path from its end while its cells allow movement (without fixed units), for the node nearest the
//!   unit; then A* from the unit to a cell centred on one of those nodes, and the path's rest after it.
//! - `getMoveAwayFromPath`: A* for the first cell where the unit may stop whose box (both units' sizes) crosses neither path to
//!   avoid, nor is the start.
//!
//! # Upstream facts kept
//!
//! - `findAttackPath` moves the unit's position by half a cell when it is CENTRED in its cell (the others when not), and its fall
//!   back to the closest cell is compiled out (`#if 0`): no path when no cell is in range.
//! - `patchPath` and `getMoveAwayFromPath` add the half cell to x twice and never to y, for an uncentred unit's start cell.
//! - `findSafePath` ignores `from`; `getMoveAwayFromPath` ignores the second object.

use crate::common::base_type::{Coord2D, Coord3D, Region2D};
use crate::common::game_type::{ObjectId, LAYER_GROUND};
use crate::logic::ai::adjust::AdjustHost;
use crate::logic::ai::layer::{PATHFIND_CELL_SIZE, PATHFIND_CELL_SIZE_F};
use crate::logic::ai::movement::CheckMovementInfo;
use crate::logic::ai::path::Path;
use crate::logic::ai::pathfinder::{Pathfinder, PfObjectFacts};
use crate::logic::ai::search::{PathfinderView, PfLocomotorSet, NO_ATTACK};
use crate::logic::modules::plant;
use crate::logic::terrain_logic::line_in_region;

/// What the purposeful searches ask beyond the adjustments: the weapon's attack distance, the units' idleness, the AI's locomotors.
pub trait AttackHost: AdjustHost {
    /// `weapon->getAttackDistance(obj, victim, victimPos)`, of the weapon the caller passes.
    fn attack_distance(&self, obj: ObjectId, victim: Option<ObjectId>, victim_pos: &Coord3D) -> f32;
    /// `unit->getAI() && unit->getAI()->isIdle()`.
    fn is_idle(&self, id: ObjectId) -> bool;
    /// `getAIUpdateInterface()->getLocomotorSet()` (the unit has an AI).
    fn ai_locomotor_set(&self, id: ObjectId) -> PfLocomotorSet;
}

impl Pathfinder {
    /// The cell's position (`adjustCoordToCell` of its indices and layer).
    fn cell_center(&self, host: &impl AttackHost, c: crate::logic::ai::cell::CellId, center: bool) -> Coord3D {
        let mut p = Coord3D::default();
        let (x, y) = (i32::from(self.store.x_index(c)), i32::from(self.store.y_index(c)));
        Self::adjust_coord_to_cell(host, x, y, center, &mut p, self.store.cells[c as usize].get_layer());
        p
    }

    /// `findAttackPath(obj, locomotorSet, from, victim, victimPos, weapon)` (the host answers for the weapon): see the module's notes.
    pub fn find_attack_path(&mut self, host: &impl AttackHost, obj: &PfObjectFacts, loco: PfLocomotorSet, from: &Coord3D, victim: Option<&PfObjectFacts>, victim_pos: &Coord3D) -> Option<Path> {
        if !self.is_map_ready {
            return None;
        }
        let crusher = host.crusher_level(obj.id) > 0;
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let victim_id = victim.map(|v| v.id);
        {
            // "Quick check:  See if moving up a couple of cells gets us in range."
            let cur = obj.pos;
            let goal = victim.map_or(*victim_pos, |v| v.pos);
            let mut delta = Coord3D { x: goal.x - cur.x, y: goal.y - cur.y, z: 0.0 };
            delta.normalize();
            delta.x *= PATHFIND_CELL_SIZE_F;
            delta.y *= PATHFIND_CELL_SIZE_F;
            for i in 1..if plant(3140) { 5 } else { 10 } {
                let mut test = cur;
                test.x += delta.x * i as f32 * 0.5;
                test.y += delta.y * i as f32 * 0.5;
                let (ndx, _) = self.world_to_cell(&test);
                let Some(a) = self.get_cell(obj.layer, ndx.x, ndx.y) else { break };
                if !self.valid_movement_position(crusher, loco.valid_surfaces, Some(a)) {
                    break;
                }
                if !self.check_destination(host, Some(obj), ndx.x, ndx.y, obj.layer, radius, center) {
                    break;
                }
                if host.goal_pos_within_attack_range(obj.id, &test, victim_id, victim_pos) && !self.is_attack_view_blocked_by_obstacle(host, obj, &test, victim, victim_pos) {
                    let mut path = Path::new();
                    path.prepend_node(&test, obj.layer);
                    path.prepend_node(&cur, obj.layer);
                    let first = path.first_node().expect("two nodes");
                    let next = path.node(first).next();
                    path.set_next_optimized(first, next);
                    return Some(path);
                }
            }
        }
        const ATTACK_CELL_LIMIT: i32 = 2500; // "this is a rather expensive operation, so limit the search."
        let is_human = host.is_human(obj.id);
        self.zone_manager.clear_passable_flags();
        if self.find_closest_hierarchical_path(host, is_human, loco.valid_surfaces, from, victim_pos, crusher).is_none() {
            self.zone_manager.set_all_passable();
        }
        let mut cell_count = 0;
        let mut attack_distance = host.attack_distance(obj.id, victim_id, victim_pos) as i32;
        attack_distance += if plant(3148) { 2 } else { 3 } * PATHFIND_CELL_SIZE;
        let mut obj_pos = obj.pos;
        if center {
            obj_pos.x += PATHFIND_CELL_SIZE_F / 2.0;
            obj_pos.y += PATHFIND_CELL_SIZE_F / 2.0;
        }
        let (start, _) = self.world_to_cell(&obj_pos);
        let mut parent = self.get_clipped_cell(obj.layer, &obj_pos)?;
        obj.ai?; // "shouldn't happen, but can't move it without an ai."
        let start_cell = parent;
        if !self.store.allocate_info(parent, start) {
            return None;
        }
        self.store.start_pathfind(parent, None);
        let (victim_ndx, _) = self.world_to_cell(victim.map_or(victim_pos, |v| &v.pos));
        let goal = self.get_cell(LAYER_GROUND, victim_ndx.x, victim_ndx.y)?;
        if !self.store.allocate_info(goal, victim_ndx) {
            return None;
        }
        self.open_list = Some(parent);
        self.closed_list = None;
        // (upstream also keeps the closest cell where the unit may stop, for a fallback compiled out)
        let check_los = victim.is_none_or(|v| !host.is_significantly_above_terrain(v.id));
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            let (px, py) = (i32::from(self.store.x_index(parent)), i32::from(self.store.y_index(parent)));
            let player = self.store.cells[parent as usize].get_layer();
            let cell_center = self.cell_center(host, parent, center);
            if host.goal_pos_within_attack_range(obj.id, &cell_center, victim_id, victim_pos) && self.check_destination(host, Some(obj), px, py, player, radius, center) {
                let mut view_blocked = false;
                if check_los && !plant(3141) {
                    view_blocked = self.is_attack_view_blocked_by_obstacle(host, obj, &cell_center, victim, victim_pos);
                }
                if start_cell == parent {
                    view_blocked = true; // "Never return the start cell."
                } else {
                    let cell_pos = self.cell_center(host, parent, center);
                    let (dx, dy) = (cell_pos.x - obj_pos.x, cell_pos.y - obj_pos.y);
                    let half = PATHFIND_CELL_SIZE_F * 0.5;
                    if dx * dx + dy * dy < half * half {
                        view_blocked = true;
                    }
                }
                if !view_blocked {
                    self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
                    if host.is_vehicle(obj.id) && !plant(3142) {
                        // "Back up until we aren't blocked."
                        let mut last_blocked = None;
                        let mut cur = Some(parent);
                        let mut use_large_radius = false;
                        let mut cell_limit = 12; // "about 4 * size of an average vehicle width (3 cells)"
                        while let Some(c) = cur {
                            cell_limit -= 1;
                            if cell_limit < 0 {
                                break;
                            }
                            let cell = crate::common::base_type::ICoord2D { x: i32::from(self.store.x_index(c)), y: i32::from(self.store.y_index(c)) };
                            let layer = self.store.cells[c as usize].get_layer();
                            let mut info = CheckMovementInfo {
                                cell,
                                layer,
                                center_in_cell: if use_large_radius { center } else { true },
                                radius: if use_large_radius { radius } else { 0 },
                                consider_transient: false,
                                acceptable_surfaces: loco.valid_surfaces,
                                ..CheckMovementInfo::default()
                            };
                            let mut unit_idle = false;
                            if let Some(at) = self.get_cell(layer, cell.x, cell.y) {
                                let pos_unit = self.store.get_pos_unit(at);
                                if host.object(pos_unit).is_some() && host.is_idle(pos_unit) {
                                    unit_idle = true;
                                }
                            }
                            let check_movement = self.check_for_movement(host, Some(obj), &mut info);
                            let blocked_by_enemy = info.enemy_fixed;
                            let mut blocked_by_allies = info.ally_fixed_count != 0 || info.ally_goal;
                            if unit_idle {
                                blocked_by_allies = false;
                            }
                            if !check_movement || blocked_by_enemy || blocked_by_allies {
                                last_blocked = Some(c);
                                use_large_radius = true;
                            } else {
                                use_large_radius = false;
                            }
                            cur = self.store.get_parent_cell(c);
                        }
                        if let Some(lb) = last_blocked {
                            parent = lb;
                            if let Some(pp) = self.store.get_parent_cell(lb) {
                                parent = pp;
                            }
                        }
                    }
                    let path = self.build_actual_path(host, Some(obj), loco.valid_surfaces, &obj.pos, parent, center, false);
                    if self.store.cells[goal as usize].has_info() && !self.store.get_closed(goal) && !self.store.get_open(goal) {
                        self.store.release_info(goal);
                    }
                    self.clean_open_and_closed_lists();
                    return Some(path);
                }
            }
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            if cell_count < ATTACK_CELL_LIMIT {
                self.check_change_layers(parent);
                cell_count += self.examine_neighboring_cells(host, parent, Some(goal), loco, is_human, center, radius, start, Some(obj), attack_distance);
            }
        }
        self.is_tunneling = false;
        if self.store.cells[goal as usize].has_info() && !self.store.get_closed(goal) && !self.store.get_open(goal) {
            self.store.release_info(goal);
        }
        self.clean_open_and_closed_lists();
        None
    }

    /// `findSafePath(obj, locomotorSet, from, repulsorPos1, repulsorPos2, repulsorRadius)`: see the module's notes.
    #[allow(clippy::too_many_arguments)] // (upstream's parameters)
    pub fn find_safe_path(&mut self, host: &impl AttackHost, obj: &PfObjectFacts, loco: PfLocomotorSet, _from: &Coord3D, repulsor1: &Coord3D, repulsor2: &Coord3D, repulsor_radius: f32) -> Option<Path> {
        if !self.is_map_ready {
            return None;
        }
        const MAX_CELLS: i32 = 2000; // "this is a rather expensive operation, so limit the search."
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let repulsor_dist_sqr = repulsor_radius * repulsor_radius;
        let mut cell_count = 0;
        let is_human = host.is_human(obj.id);
        self.zone_manager.set_all_passable();
        let (start, _) = self.world_to_cell(&obj.pos);
        let mut parent = self.get_clipped_cell(obj.layer, &obj.pos)?;
        obj.ai?; // "shouldn't happen, but can't move it without an ai."
        if !self.store.allocate_info(parent, start) {
            return None;
        }
        self.store.start_pathfind(parent, None);
        self.open_list = Some(parent);
        self.closed_list = None;
        let mut farthest_dist_sqr = 0.0f32;
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            let cell_center = self.cell_center(host, parent, center);
            let (dx, dy) = (cell_center.x - repulsor1.x, cell_center.y - repulsor1.y);
            let mut ok = false;
            let mut dist_sqr = dx * dx + dy * dy;
            let (dx, dy) = (cell_center.x - repulsor2.x, cell_center.y - repulsor2.y);
            let dist_sqr2 = dx * dx + dy * dy;
            if dist_sqr2 < dist_sqr && !plant(3143) {
                dist_sqr = dist_sqr2;
            }
            if dist_sqr > repulsor_dist_sqr {
                ok = true;
            }
            if self.open_list.is_none() && cell_count > 0 {
                ok = true; // "exhausted the search space, just take the last cell."
            }
            if dist_sqr > farthest_dist_sqr {
                farthest_dist_sqr = dist_sqr;
                if cell_count > MAX_CELLS {
                    ok = true; // "Already a big search, just take this one."
                }
            }
            let (px, py) = (i32::from(self.store.x_index(parent)), i32::from(self.store.y_index(parent)));
            let player = self.store.cells[parent as usize].get_layer();
            if ok && self.check_destination(host, Some(obj), px, py, player, radius, center) {
                let path = self.build_actual_path(host, Some(obj), loco.valid_surfaces, &obj.pos, parent, center, false);
                self.store.release_info(parent);
                self.clean_open_and_closed_lists();
                return Some(path);
            }
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            self.check_change_layers(parent);
            cell_count += self.examine_neighboring_cells(host, parent, None, loco, is_human, center, radius, start, Some(obj), NO_ATTACK);
        }
        self.is_tunneling = false;
        self.clean_open_and_closed_lists();
        None
    }

    /// `patchPath(obj, locomotorSet, originalPath, blocked)`: see the module's notes.
    pub fn patch_path(&mut self, host: &impl AttackHost, obj: &PfObjectFacts, loco: PfLocomotorSet, original: Option<&Path>, blocked: bool) -> Option<Path> {
        let original = original?;
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let is_human = host.is_human(obj.id);
        self.zone_manager.set_all_passable();
        const CELL_LIMIT: i32 = 2000; // "max cells to examine."
        let mut cell_count = 0;
        let current = obj.pos;
        let mut start_pos = obj.pos;
        if !center {
            start_pos.x += PATHFIND_CELL_SIZE_F * 0.5;
            start_pos.x += PATHFIND_CELL_SIZE_F * 0.5; // (upstream's: x twice, y never)
        }
        let (start, _) = self.world_to_cell(&start_pos);
        let mut parent = self.get_clipped_cell(obj.layer, &current)?;
        obj.ai?; // "shouldn't happen, but can't move it without an ai."
        self.is_tunneling = false;
        if !self.store.allocate_info(parent, start) {
            return None;
        }
        self.store.start_pathfind(parent, None);
        self.open_list = Some(parent);
        self.closed_list = None;
        let last = original.last_node().expect("upstream reads the path's last node unchecked");
        let first = original.first_node();
        let mut goal_pos = *original.node(last).position();
        let sqr = |v: f32| v * v;
        let mut goal_delta_sqr = sqr(goal_pos.x - current.x) + sqr(goal_pos.y - current.y);
        let mut start_node = Some(last);
        while start_node != first {
            let n = original.node(start_node.expect("a node before the first"));
            let (cell, _) = self.world_to_cell(n.position());
            let mut info = CheckMovementInfo {
                cell,
                layer: n.layer(),
                center_in_cell: center,
                radius,
                consider_transient: blocked,
                acceptable_surfaces: loco.valid_surfaces,
                ..CheckMovementInfo::default()
            };
            let (dx, dy) = (cell.x - start.x, cell.y - start.y);
            if !(-2..=if plant(3147) { 3 } else { 2 }).contains(&dx) {
                info.consider_transient = false;
            }
            if !(-2..=2).contains(&dy) {
                info.consider_transient = false;
            }
            if !self.check_for_movement(host, Some(obj), &mut info) {
                break;
            }
            if info.ally_fixed_count != 0 || info.enemy_fixed {
                break; // "Don't patch through cells that are occupied."
            }
            let cur_sqr = sqr(n.position().x - current.x) + sqr(n.position().y - current.y);
            if cur_sqr < goal_delta_sqr && !plant(3144) {
                goal_pos = *n.position();
                goal_delta_sqr = cur_sqr;
            }
            start_node = n.previous();
        }
        if start_node == Some(last) {
            self.clean_open_and_closed_lists();
            return None; // "no open nodes."
        }
        let candidate = self.get_cell_at(LAYER_GROUND, &goal_pos).expect("upstream dereferences the candidate goal unchecked");
        let (goal_ndx, _) = self.world_to_cell(&goal_pos);
        if !self.store.allocate_info(candidate, goal_ndx) {
            return None;
        }
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            let cell_center = self.cell_center(host, parent, center);
            let mut match_node = Some(last);
            let mut found = false;
            while match_node != start_node {
                let m = original.node(match_node.expect("a node after the start"));
                if cell_center.x == m.position().x && cell_center.y == m.position().y {
                    found = true;
                    break;
                }
                match_node = m.previous();
            }
            if found {
                self.is_tunneling = false;
                let mut path = Path::new();
                let mut node = Some(last);
                while node != match_node {
                    let nd = original.node(node.expect("a node after the match"));
                    path.prepend_node(nd.position(), nd.layer());
                    node = nd.previous();
                }
                self.prepend_cells(host, &mut path, &obj.pos, parent, center);
                let mut view = PathfinderView { pf: self, host };
                path.optimize(&mut view, obj.id, loco.valid_surfaces, blocked);
                self.store.release_info(parent);
                self.clean_open_and_closed_lists();
                self.store.release_info(candidate);
                return Some(path);
            }
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            if cell_count < CELL_LIMIT {
                self.check_change_layers(parent);
                cell_count += self.examine_neighboring_cells(host, parent, None, loco, is_human, center, radius, start, Some(obj), NO_ATTACK);
            }
        }
        self.is_tunneling = false;
        if !self.store.get_open(candidate) && !self.store.get_closed(candidate) {
            self.store.release_info(candidate);
        }
        self.clean_open_and_closed_lists();
        None
    }

    /// The segments of a path along its optimized nodes, from the first.
    fn optimized_segments(path: &Path) -> Vec<(Coord2D, Coord2D)> {
        let mut out = Vec::new();
        let mut node = path.first_node();
        while let Some(n) = node {
            let Some(next) = path.node(n).next_optimized().0 else { break };
            let (a, b) = (path.node(n).position(), path.node(next).position());
            out.push((Coord2D { x: a.x, y: a.y }, Coord2D { x: b.x, y: b.y }));
            node = Some(next);
        }
        out
    }

    /// `getMoveAwayFromPath(obj, otherObj, pathToAvoid, otherObj2, pathToAvoid2)`: see the module's notes.
    pub fn get_move_away_from_path(&mut self, host: &impl AttackHost, obj: &PfObjectFacts, other: Option<&PfObjectFacts>, avoid: &Path, avoid2: Option<&Path>) -> Option<Path> {
        if !self.is_map_ready {
            return None;
        }
        let is_human = host.is_human(obj.id);
        let (other_radius, other_center) = Self::get_radius_and_center(other);
        let crusher = host.crusher_level(obj.id) > 0;
        self.zone_manager.set_all_passable();
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let mut start_pos = obj.pos;
        if !center {
            start_pos.x += PATHFIND_CELL_SIZE_F * 0.5;
            start_pos.x += PATHFIND_CELL_SIZE_F * 0.5; // (upstream's: x twice, y never)
        }
        let (start, _) = self.world_to_cell(&start_pos);
        let mut parent = self.get_clipped_cell(obj.layer, &obj.pos)?;
        obj.ai?; // "shouldn't happen, but can't move it without an ai."
        let loco = host.ai_locomotor_set(obj.id);
        self.is_tunneling = false;
        if !self.valid_movement_position(crusher, loco.valid_surfaces, Some(parent)) {
            self.is_tunneling = true; // "We can't move from our current location.  So relax the constraints."
        }
        let mut info = CheckMovementInfo {
            cell: start,
            layer: obj.layer,
            center_in_cell: center,
            radius,
            consider_transient: false,
            acceptable_surfaces: loco.valid_surfaces,
            ..CheckMovementInfo::default()
        };
        if !self.check_for_movement(host, Some(obj), &mut info) || info.enemy_fixed {
            self.is_tunneling = true;
        }
        if !self.store.allocate_info(parent, start) {
            return None;
        }
        self.store.start_pathfind(parent, None);
        self.open_list = Some(parent);
        self.closed_list = None;
        let mut box_half_width = radius as f32 * PATHFIND_CELL_SIZE_F - (PATHFIND_CELL_SIZE_F / 4.0);
        if center {
            box_half_width += PATHFIND_CELL_SIZE_F / 2.0;
        }
        box_half_width += other_radius as f32 * PATHFIND_CELL_SIZE_F;
        if other_center && !plant(3145) {
            box_half_width += PATHFIND_CELL_SIZE_F / 2.0;
        }
        let segments = Self::optimized_segments(avoid);
        let segments2 = avoid2.map(Self::optimized_segments);
        while let Some(p) = self.open_list {
            parent = p;
            self.open_list = self.store.remove_from_open_list(parent, self.open_list);
            let cell_center = self.cell_center(host, parent, center);
            let bounds = Region2D {
                lo: Coord2D { x: cell_center.x - box_half_width, y: cell_center.y - box_half_width },
                hi: Coord2D { x: cell_center.x + box_half_width, y: cell_center.y + box_half_width },
            };
            let mut overlap = segments.iter().any(|(a, b)| line_in_region(a, b, &bounds));
            if !overlap {
                if let Some(s2) = &segments2 {
                    overlap = s2.iter().any(|(a, b)| line_in_region(a, b, &bounds));
                }
            }
            let (px, py) = (i32::from(self.store.x_index(parent)), i32::from(self.store.y_index(parent)));
            if !overlap && start.x == px && start.y == py && !plant(3146) {
                overlap = true;
            }
            let player = self.store.cells[parent as usize].get_layer();
            if !overlap && self.check_destination(host, Some(obj), px, py, player, radius, center) {
                self.is_tunneling = false;
                let path = self.build_actual_path(host, Some(obj), loco.valid_surfaces, &obj.pos, parent, center, false);
                self.store.release_info(parent);
                self.clean_open_and_closed_lists();
                return Some(path);
            }
            self.closed_list = self.store.put_on_closed_list(parent, self.closed_list);
            self.check_change_layers(parent);
            self.examine_neighboring_cells(host, parent, None, loco, is_human, center, radius, start, Some(obj), NO_ATTACK);
        }
        self.is_tunneling = false;
        self.clean_open_and_closed_lists();
        None
    }
}
