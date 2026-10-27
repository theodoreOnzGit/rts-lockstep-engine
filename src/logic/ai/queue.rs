// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: queueForPath, 5648-5688; getAircraftPath and
//     processPathfindQueue, 5805-5964; moveAlliesDestinationCallback and moveAlliesAwayFromDestination, 6894-6948;
//     slowDoesPathExist, 8136-8155; moveAllies, 10112-10193; crc, xfer and loadPostProcess, 11067-11124)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The Pathfinder's allies, aircraft paths, request queue and snapshot. `p1w13-sonnet`, layer 10, batch P11.
//!
//! - `moveAllies`: for a dozer, a harvester or a path blocked by an ally, the allies on the cells around the path's nodes (but its
//!   first) that stand still, not attacking, busy or using an ability, are asked to move away (infantry through infantry is let be,
//!   and infantry moves others only when blocked); at most two deep.
//! - `moveAlliesAwayFromDestination`: the same request of the allies on the line to the destination.
//! - `getAircraftPath`: straight to the destination (beside a tall building there, for a winged locomotor), with three nodes around
//!   each tall building a segment crosses, for 20 segments.
//! - `queueForPath`, `processPathfindQueue`: a ring of 512 requests; each frame, after the zones (recalculated instead, when due) and
//!   the logical extent from the terrain's, the queued units' `doPathfind` until 5,000 cells are counted.
//! - `slowDoesPathExist`: `findPath` with the AI's locomotors and an obstacle ignored.
//! - The snapshot: a light CRC of the extent, flags, ignored obstacle, queue, wall pieces, wall height and cell count; the save is a
//!   version alone.
//!
//! # Upstream facts kept
//!
//! - `crc` reads `m_wallPieces[MAX_WALL_PIECES]`, one past the array, 128 times: in the class's layout that is `m_numWallPieces`,
//!   which the port reads instead.
//! - `moveAllies` asks the relationship of the cell's unit before checking it exists.

use crate::common::base_type::{real_to_int_floor, Coord3D, IRegion2D, Region3D};
use crate::common::game_type::{ObjectId, INVALID_ID, LAYER_GROUND};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::ai::attack::AttackHost;
use crate::logic::ai::cell::{CellId, PATHFIND_CELLS_PER_FRAME};
use crate::logic::ai::layer::PATHFIND_CELL_SIZE_F;
use crate::logic::ai::path::Path;
use crate::logic::ai::pathfinder::{Pathfinder, PfObjectFacts, MAX_WALL_PIECES, PATHFIND_QUEUE_LEN};
use crate::logic::modules::plant;

/// An AI's activity, as `moveAllies` asks it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiActivity {
    /// `isMoving()`, `isAttacking()`, `isBusy()`.
    pub moving: bool,
    pub attacking: bool,
    pub busy: bool,
    /// `testStatus(OBJECT_STATUS_IS_USING_ABILITY)`.
    pub using_ability: bool,
}

/// What the allies, aircraft paths and queue ask beyond the purposeful searches.
pub trait AlliesHost: AttackHost {
    fn is_harvester(&self, id: ObjectId) -> bool;
    /// The unit's AI's activity; none without an AI.
    fn ai_activity(&self, id: ObjectId) -> Option<AiActivity>;
    /// `getAI()->aiMoveAwayFromUnit(from, CMD_FROM_AI)`.
    fn ai_move_away_from_unit(&mut self, id: ObjectId, from: ObjectId);
    /// `getAI()->getBuildingToNotPathAround()`.
    fn building_to_not_path_around(&self, id: ObjectId) -> ObjectId;
    /// `getAI()->getCurLocomotor()`'s appearance is `LOCO_WINGS` (false without one).
    fn has_wings_locomotor(&self, id: ObjectId) -> bool;
    /// `TheTerrainLogic->getExtent()`.
    fn terrain_extent(&self) -> Region3D;
}

impl Pathfinder {
    /// The ally at a cell asked to move away, as `moveAllies` and `moveAlliesDestinationCallback` share it (`attacking`: whether an
    /// attacking unit is let be, `moveAllies`'s test).
    fn ask_ally_to_move(host: &mut impl AlliesHost, obj: ObjectId, other: ObjectId, attacking: bool) {
        if let Some(act) = host.ai_activity(other) {
            if !act.moving {
                if attacking && act.attacking {
                    return; // "Don't move units that are attacking."
                }
                if act.using_ability || act.busy {
                    return; // "Packing or unpacking objects for example"
                }
                host.ai_move_away_from_unit(other, obj);
            }
        }
    }

    /// `moveAllies(obj, path)`: see the module's notes; whether the allies were looked for.
    pub fn move_allies(&mut self, host: &mut impl AlliesHost, obj: &PfObjectFacts, path: &Path) -> bool {
        if !host.is_dozer(obj.id) && !host.is_harvester(obj.id) && !path.blocked_by_ally() {
            return false; // "Only move units if it is required."
        }
        let depth = self.move_allies_depth;
        self.move_allies_depth += 1;
        let r = self.move_allies_along(host, obj, path);
        self.move_allies_depth = depth;
        r
    }

    fn move_allies_along(&mut self, host: &mut impl AlliesHost, obj: &PfObjectFacts, path: &Path) -> bool {
        if self.move_allies_depth > 2 {
            return false;
        }
        let (radius, center) = Self::get_radius_and_center(Some(obj));
        let above = radius + i32::from(center && !plant(3150));
        // (the AI's, when there is one)
        let ignore = if obj.ai.is_some() { host.ignored_obstacle_id(obj.id) } else { INVALID_ID };
        let first = path.first_node();
        let mut node = path.last_node();
        while let Some(n) = node {
            if Some(n) == first {
                break;
            }
            let nd = path.node(n);
            let (cur, _) = self.world_to_cell(nd.position());
            for i in cur.x - radius..cur.x + above {
                for j in cur.y - radius..cur.y + above {
                    let Some(cell) = self.get_cell(nd.layer(), i, j) else { continue };
                    let pos_unit = self.store.get_pos_unit(cell);
                    if pos_unit == INVALID_ID || pos_unit == obj.id || pos_unit == ignore {
                        continue; // (none, "It's us.", "It's the one we are ignoring.")
                    }
                    if !host.are_allies(obj.id, pos_unit) {
                        continue; // "Only move allies."
                    }
                    if host.object(pos_unit).is_none() {
                        continue;
                    }
                    if host.is_infantry(obj.id) && host.is_infantry(pos_unit) && !plant(3151) {
                        continue; // "infantry can walk through other infantry, so just let them."
                    }
                    if host.is_infantry(obj.id) && !host.is_infantry(pos_unit) && !path.blocked_by_ally() {
                        continue;
                    }
                    Self::ask_ally_to_move(host, obj.id, pos_unit, true);
                }
            }
            node = nd.previous();
        }
        true
    }

    /// `moveAlliesAwayFromDestination(obj, destination)`: see the module's notes (`moveAlliesDestinationCallback` on each cell of the
    /// line, which never ends it).
    pub fn move_allies_away_from_destination(&self, host: &mut impl AlliesHost, obj: &PfObjectFacts, destination: &Coord3D) {
        obj.ai.expect("upstream dereferences the AI unchecked");
        let ignore = host.ignored_obstacle_id(obj.id);
        let mut layer = obj.layer;
        if layer == LAYER_GROUND && !plant(3152) {
            layer = host.layer_for_destination(destination);
        }
        // (the line's cells first: which they are depends on the map alone, and each is then asked in order)
        let mut cells: Vec<CellId> = Vec::new();
        let (s, _) = self.world_to_cell(&obj.pos);
        let (e, _) = self.world_to_cell(destination);
        self.walk_line(s, e, layer, |_, _, to, _, _| {
            cells.push(to);
            0
        });
        for to in cells {
            let pos_unit = self.store.get_pos_unit(to);
            if pos_unit == INVALID_ID || pos_unit == obj.id || pos_unit == ignore {
                continue;
            }
            if host.object(pos_unit).is_none() {
                continue;
            }
            if !host.are_allies(obj.id, pos_unit) {
                continue; // "Only move allies."
            }
            Self::ask_ally_to_move(host, obj.id, pos_unit, false);
        }
    }

    /// `getAircraftPath(obj, to)`: see the module's notes.
    pub fn get_aircraft_path(&self, host: &impl AlliesHost, obj: &PfObjectFacts, to: &Coord3D) -> Path {
        let mut path = Path::new();
        let has_ai = obj.ai.is_some();
        let avoid = if has_ai { host.building_to_not_path_around(obj.id) } else { INVALID_ID };
        let check_clips = has_ai && host.has_wings_locomotor(obj.id);
        let radius = 100.0;
        let mut adjust = *to;
        if check_clips {
            if let Some(a) = self.circle_clips_tall_building(host, &obj.pos, to, radius) {
                adjust = a;
            }
        }
        path.prepend_node(&adjust, LAYER_GROUND);
        let mut pos = obj.pos;
        pos.z = to.z;
        path.prepend_node(&pos, LAYER_GROUND);
        let mut limit = 20;
        let mut cur = path.first_node();
        while let Some(c) = cur {
            let Some(next) = path.node(c).next() else { break };
            if let Some([p1, p2, p3]) = self.segment_intersects_tall_building(host, &mut path, c, next, avoid) {
                path.insert_after(c, &p3, LAYER_GROUND);
                let n2 = path.insert_after(c, &p2, LAYER_GROUND);
                path.insert_after(c, if plant(3153) { &p3 } else { &p1 }, LAYER_GROUND);
                cur = Some(n2);
            }
            cur = cur.and_then(|c| path.node(c).next());
            limit -= 1;
            if limit < 0 {
                break;
            }
        }
        let mut cur = path.first_node();
        while let Some(c) = cur {
            let Some(next) = path.node(c).next() else { break };
            path.set_next_optimized(c, Some(next));
            cur = Some(next);
        }
        path.mark_optimized();
        path
    }

    /// `queueForPath(id)`: true when queued (or already), false when the ring is full.
    pub fn queue_for_path(&mut self, id: ObjectId) -> bool {
        // "Check & see if we are already queued."
        let mut slot = self.queue_pr_head;
        while slot != self.queue_pr_tail {
            if self.queued_pathfind_requests[slot as usize] == id && !plant(3158) {
                return true;
            }
            slot += 1;
            if slot >= PATHFIND_QUEUE_LEN as i32 {
                slot = 0;
            }
        }
        // "Tail is the first available slot."
        let mut next_slot = self.queue_pr_tail + 1;
        if next_slot >= PATHFIND_QUEUE_LEN as i32 {
            next_slot = 0;
        }
        if next_slot == self.queue_pr_head {
            return false; // "Ran out of pathfind queue slots."
        }
        self.queued_pathfind_requests[self.queue_pr_tail as usize] = id;
        self.queue_pr_tail = next_slot;
        true
    }

    /// `processPathfindQueue()` at `frame`: see the module's notes; `do_pathfind` is the queued unit's AI's `doPathfind(this)` (for a
    /// unit that exists and has an AI).
    pub fn process_pathfind_queue<H: AlliesHost>(&mut self, host: &mut H, frame: u32, mut do_pathfind: impl FnMut(&mut Pathfinder, &mut H, ObjectId)) {
        if !self.is_map_ready {
            return;
        }
        if self.zone_manager.need_to_calculate_zones(frame) {
            let ground = self.ground();
            let e = self.extent;
            self.zone_manager.calculate_zones(&mut self.store, &ground, &mut self.layers, &e);
            return;
        }
        // "Get the current logical extent."
        let t = host.terrain_extent();
        let floor = |v: f32| real_to_int_floor(v / PATHFIND_CELL_SIZE_F);
        let mut bounds = IRegion2D::default();
        bounds.lo.x = floor(t.lo.x);
        bounds.hi.x = floor(t.hi.x);
        bounds.lo.y = floor(t.lo.y);
        bounds.hi.y = floor(t.hi.y);
        bounds.hi.x -= i32::from(!plant(3154));
        bounds.hi.y -= 1;
        self.logical_extent = bounds;
        self.cumulative_cells_allocated = 0; // "Number of pathfind cells examined."
        while self.cumulative_cells_allocated < PATHFIND_CELLS_PER_FRAME && self.queue_pr_tail != self.queue_pr_head {
            let head = self.queue_pr_head as usize;
            let id = self.queued_pathfind_requests[head];
            self.queued_pathfind_requests[head] = INVALID_ID;
            if host.object(id).is_some_and(|o| o.ai.is_some()) {
                do_pathfind(self, host, id);
            }
            self.queue_pr_head += 1;
            if self.queue_pr_head >= PATHFIND_QUEUE_LEN as i32 {
                self.queue_pr_head = 0;
            }
        }
    }

    /// `slowDoesPathExist(obj, from, to, ignoreObject)`: false without an AI.
    pub fn slow_does_path_exist(&mut self, host: &impl AlliesHost, obj: &PfObjectFacts, from: &Coord3D, to: &Coord3D, ignore_object: ObjectId) -> bool {
        if obj.ai.is_none() {
            return false;
        }
        let loco = host.ai_locomotor_set(obj.id);
        self.ignore_obstacle_id = if plant(3155) { INVALID_ID } else { ignore_object };
        let path = self.find_path(host, Some(obj), loco, from, to);
        self.ignore_obstacle_id = INVALID_ID;
        path.is_some()
    }
}

impl Snapshot for Pathfinder {
    /// `crc`: the extent (as its bytes), the map-ready and tunneling flags, an obsolete 0, the ignored obstacle and the queue (as
    /// their bytes), its head and tail, the wall pieces' count, the count again 128 times (upstream's `m_wallPieces[MAX_WALL_PIECES]`,
    /// see the module's notes), the wall height and the cell count.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let e = self.extent;
        let mut extent = [e.lo.x, e.lo.y, e.hi.x, e.hi.y].map(i32::to_le_bytes).concat();
        xfer.xfer_user(&mut extent)?;
        let mut ready = self.is_map_ready;
        xfer.xfer_bool(&mut ready)?;
        let mut tunneling = self.is_tunneling;
        xfer.xfer_bool(&mut tunneling)?;
        let mut obsolete1 = 0;
        xfer.xfer_int(&mut obsolete1)?;
        let mut ignore = self.ignore_obstacle_id.to_le_bytes();
        xfer.xfer_user(&mut ignore)?;
        let mut queue: Vec<u8> = self.queued_pathfind_requests.iter().flat_map(|id| id.to_le_bytes()).collect();
        xfer.xfer_user(&mut queue)?;
        let (mut head, mut tail) = (self.queue_pr_head, self.queue_pr_tail);
        xfer.xfer_int(&mut head)?;
        xfer.xfer_int(&mut tail)?;
        let mut num = self.num_wall_pieces;
        xfer.xfer_int(&mut num)?;
        for _ in 0..MAX_WALL_PIECES {
            let mut past_the_end = if plant(3156) { INVALID_ID } else { self.num_wall_pieces as ObjectId };
            xfer.xfer_object_id(&mut past_the_end)?;
        }
        let mut height = self.wall_height;
        xfer.xfer_real(&mut height)?;
        let mut cells = self.cumulative_cells_allocated;
        xfer.xfer_int(&mut cells)?;
        Ok(())
    }
    /// `xfer` (version 1): the version alone.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)
    }
    /// `loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}
