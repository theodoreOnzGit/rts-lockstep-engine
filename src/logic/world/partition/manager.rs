// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/PartitionManager.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp
//   (`PartitionManager`'s grid, registration, update and collision pass;
//   `PartitionData`; `CellAndObjectIntersection`; `PartitionContactList`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! [`PartitionManager`]: the spatial grid. This file is the grid and the
//! objects' place in it: building the cells, registering and unregistering
//! objects, the dirty list, the footprint rasterisers, the collision pass of
//! `update()` and its contact list. The shroud, the value maps, the queries
//! and the filters are in the sibling files (`shroud`, `query`, `filters`)
//! as further `impl PartitionManager` blocks.
//!
//! # What upstream does that this keeps exactly
//!
//! - **Every list order.** A cell's list of intersections is prepended to; the
//!   module list is prepended to (newest first, so `iterateAllObjects` is
//!   newest first); the dirty list is prepended to and drained from its head
//!   (so objects are processed last-dirtied first); the contact list is
//!   prepended to and processed newest first. Query results (ties included)
//!   depend on these, so the intrusive lists are reproduced with index links.
//! - **The footprint rasterisers** `doSmallFill`, `doCircleFill`,
//!   `doRectFill` and the COI-array size `calcMaxCoiForShape`, including its
//!   missing `break` (a sphere or cylinder falls into the box case and is
//!   sized as the box with the same radii; since a sphere or cylinder has
//!   `minor == major` the size is the box formula of the diagonal).
//! - **`m_lastCell`** and the `onPartitionCellChange` call: made only when the
//!   object's centre cell changed (`None`, off the map, compares equal to `None`).
//! - **`DISABLE_INVALID_PREVENTION`**, which the file defines: a shroudedness
//!   is invalidated unconditionally (the `#ifndef` branches are dead text and
//!   are not translated; `OBJECTSHROUD_INVALID_BUT_PREVIOUS_VALID` is therefore
//!   never produced).
//! - **`FASTER_GCO`**, which the header defines: the closest-object search
//!   uses the precomputed ring table `m_radiusVec`; `CellOutwardIterator` is
//!   `#ifdef`ed out upstream and not translated.
//!
//! # Cut points
//!
//! - **Ghost objects** (`TheGhostObjectManager`, fog-of-war memory of
//!   buildings; client). Whether an immobile object gets a ghost is the
//!   host's answer ([`PartitionHost::add_ghost_object`]); the manager keeps
//!   the ghost's geometry (a ghost-only module still occupies cells and
//!   shows up in cell lists, exactly as upstream) and emits
//!   [`PartitionClientEvent`]s for the ghost calls (`snapShot`,
//!   `freeSnapShot`, `removeGhostObject`, `updateParentObject`).
//! - **`TheDisplay` and `TheRadar`** shroud writes: [`PartitionClientEvent::SetShroudLevel`]
//!   and `ClearShroud`, emitted for the local player's cells only, as upstream.
//! - **Debug drawing** (`addIcon`, the threat-map overlay of `update()`, under
//!   `_DEBUG`/`_INTERNAL`) and the perf counters (`DUMP_PERF_STATS`): not translated.
//!
//! # Where upstream is undefined (the Rust is bounded, never a crash)
//!
//! - `doSmallFill` stores intersections without checking the array size (a
//!   "small" geometry is assumed to touch at most 4 cells; a radius of exactly
//!   half a cell can touch 9 after float rounding). Here a COI beyond the
//!   array is dropped. Not compared by the differential tests.
//! - `getClosestObjects` with an empty ring table (no terrain): upstream
//!   indexes `m_radiusVec[0]` of an empty vector; here nothing is found.

use super::cell::{PartitionCell, SightingInfo};
use super::collide::{self, CollideInfo, CollideLocAndNormal};
use super::data::{Coi, CoiRef, DirtyStatus, PartitionData};
use super::host::{GhostGeometry, PartitionHost};
use crate::common::base_type::{real_to_int_ceil, real_to_int_floor, ICoord2D, Region3D};
use crate::common::game_common::{CellShroudStatus, MAX_PLAYER_COUNT};
use crate::common::geometry::GeometryType;
use crate::common::kind_of::{KINDOF_IMMOBILE, KINDOF_NO_COLLIDE};
use crate::common::object_status_types::OBJECT_STATUS_NO_COLLISIONS;
use crate::logic::terrain_visual::MAP_XY_FACTOR;
use crate::logic::world::ids::ObjectId;
use petir::Real;
use std::collections::{BTreeMap, VecDeque};

/// `PartitionContactList_SOCKET_COUNT`.
const CONTACT_SOCKET_COUNT: usize = 5381;

/// What the partition manager says to the client (the calls it makes on
/// `TheDisplay`, `TheRadar` and `TheGhostObjectManager`). The client drains
/// them with [`PartitionManager::take_client_events`] (the world forwards them
/// as `ClientEvent::Partition`).
#[derive(Clone, Debug, PartialEq)]
pub enum PartitionClientEvent {
    /// `TheDisplay->setShroudLevel(x, y, status)` and `TheRadar->setShroudLevel(x, y, status)`
    /// for a cell of the local player.
    SetShroudLevel {
        x: i32,
        y: i32,
        status: CellShroudStatus,
    },
    /// `TheDisplay->clearShroud()` and `TheRadar->clearShroud()`.
    ClearShroud,
    /// `ghost->snapShot(playerIndex)`: remember the object's look while fogged.
    GhostSnapShot { ghost: u32, player: i32 },
    /// `ghost->freeSnapShot(playerIndex)`.
    GhostFreeSnapShot { ghost: u32, player: i32 },
    /// `TheGhostObjectManager->removeGhostObject(ghost)`.
    GhostRemoved { ghost: u32 },
    /// `ghost->updateParentObject(NULL, module)`: the ghost's object is gone.
    GhostParentGone { ghost: u32 },
}

/// `PartitionContactListNode`.
#[derive(Clone, Copy, Debug)]
struct ContactNode {
    /// `m_obj` (module slot), `None` once removed or collided.
    obj: Option<usize>,
    /// `m_other`.
    other: Option<usize>,
    /// `m_nextHash`: the next node of the same socket (an index into `nodes`).
    next_hash: Option<usize>,
}

/// `class PartitionContactList`: the pairs of modules whose cells overlap this
/// update, deduplicated through a hash of the object ids, processed newest
/// first.
#[derive(Clone, Debug, Default)]
struct ContactList {
    /// Oldest first; upstream's `m_contactList` runs from the last pushed.
    nodes: Vec<ContactNode>,
    /// `m_contactHash`: the newest node of each socket. Allocated on first use.
    heads: Vec<Option<usize>>,
}

/// `hash2ints(a, b)`: `[a,b]` hashes like `[b,a]`; `Int` shifts wrap.
fn hash2ints(a: i32, b: i32) -> u32 {
    if a < b {
        (a.wrapping_shl(16)).wrapping_add(b) as u32
    } else {
        (b.wrapping_shl(16)).wrapping_add(a) as u32
    }
}

/// `class PartitionManager`. See the module docs.
#[derive(Clone, Debug)]
pub struct PartitionManager {
    /// `m_moduleList`: head of the master module list (newest).
    pub(super) module_list: Option<usize>,
    /// `m_worldExtents`.
    pub(super) world_extents: Region3D,
    /// `m_cellSize`.
    pub(super) cell_size: f32,
    /// `m_cellSizeInv`.
    pub(super) cell_size_inv: f32,
    /// `m_cellCountX`.
    pub(super) cell_count_x: i32,
    /// `m_cellCountY`.
    pub(super) cell_count_y: i32,
    /// `m_totalCellCount`.
    pub(super) total_cell_count: i32,
    /// `m_cells`.
    pub(super) cells: Vec<PartitionCell>,
    /// `m_dirtyModules`: head of the dirty list.
    pub(super) dirty_modules: Option<usize>,
    /// `m_updatedSinceLastReset`.
    pub(super) updated_since_last_reset: bool,
    /// `m_pendingUndoShroudReveals`.
    pub(super) pending_undo_shroud_reveals: VecDeque<SightingInfo>,
    /// `m_maxGcoRadius`.
    pub(super) max_gco_radius: i32,
    /// `m_radiusVec`.
    pub(super) radius_vec: Vec<Vec<ICoord2D>>,

    /// The modules, by slot (upstream: heap objects).
    pub(super) modules: Vec<Option<PartitionData>>,
    /// Free slots, the most recently freed last.
    free_slots: Vec<usize>,
    /// `object->friend_getPartitionData()`: the module of each registered object.
    pub(super) by_object: BTreeMap<ObjectId, usize>,
    /// `TheContactList` (non-null only inside `update()`).
    contact_list: Option<ContactList>,
    /// `theIterFlag` of `getClosestObjects` (a function-local static upstream).
    pub(super) iter_flag: i32,
    /// Calls for the client, oldest first.
    pub(super) events: Vec<PartitionClientEvent>,
    /// Set by an `xfer` load: the caller must run `refresh_shroud_for_local_player`.
    pub(super) needs_shroud_refresh: bool,
}

impl Default for PartitionManager {
    /// `PartitionManager::PartitionManager()`: no cells, nothing registered.
    fn default() -> Self {
        PartitionManager {
            module_list: None,
            world_extents: Region3D::default(),
            cell_size: 0.0,
            cell_size_inv: 0.0,
            cell_count_x: 0,
            cell_count_y: 0,
            total_cell_count: 0,
            cells: Vec::new(),
            dirty_modules: None,
            updated_since_last_reset: false,
            pending_undo_shroud_reveals: VecDeque::new(),
            max_gco_radius: 0,
            radius_vec: Vec::new(),
            modules: Vec::new(),
            free_slots: Vec::new(),
            by_object: BTreeMap::new(),
            contact_list: None,
            iter_flag: 1, // "nonzero, thanks"
            events: Vec::new(),
            needs_shroud_refresh: false,
        }
    }
}

impl PartitionManager {
    /// `PartitionManager::PartitionManager()`.
    pub fn new() -> PartitionManager {
        PartitionManager::default()
    }

    // ---- small accessors -----------------------------------------------------

    /// The module in `slot`. Slots are only handed out for live modules.
    pub(super) fn md(&self, slot: usize) -> &PartitionData {
        self.modules[slot].as_ref().expect("live partition module")
    }

    pub(super) fn md_mut(&mut self, slot: usize) -> &mut PartitionData {
        self.modules[slot].as_mut().expect("live partition module")
    }

    /// Whether `slot` still holds a module (a callback may have deleted it).
    pub(super) fn is_live(&self, slot: usize) -> bool {
        matches!(self.modules.get(slot), Some(Some(_)))
    }

    pub(super) fn coi(&self, r: CoiRef) -> &Coi {
        &self.md(r.module as usize).coi_array[r.index as usize]
    }

    pub(super) fn coi_mut(&mut self, r: CoiRef) -> &mut Coi {
        &mut self.md_mut(r.module as usize).coi_array[r.index as usize]
    }

    /// `getUpdatedSinceLastReset()`.
    pub fn get_updated_since_last_reset(&self) -> bool {
        self.updated_since_last_reset
    }

    /// `getCellCountX()`.
    pub fn get_cell_count_x(&self) -> i32 {
        self.cell_count_x
    }

    /// `getCellCountY()`.
    pub fn get_cell_count_y(&self) -> i32 {
        self.cell_count_y
    }

    /// `getCellSize()`.
    pub fn get_cell_size(&self) -> f32 {
        self.cell_size
    }

    /// `getCellSizeInv()`.
    pub fn get_cell_size_inv(&self) -> f32 {
        self.cell_size_inv
    }

    /// `m_worldExtents`.
    pub fn get_world_extents(&self) -> Region3D {
        self.world_extents
    }

    /// The calls waiting for the client, oldest first; leaves the queue empty.
    pub fn take_client_events(&mut self) -> Vec<PartitionClientEvent> {
        std::mem::take(&mut self.events)
    }

    /// `worldToCell(wx, wy, &cx, &cy)`: `REAL_TO_INT_FLOOR((w - lo) * cellSizeInv)`.
    pub fn world_to_cell(&self, wx: f32, wy: f32) -> (i32, i32) {
        (
            real_to_int_floor((wx - self.world_extents.lo.x) * self.cell_size_inv),
            real_to_int_floor((wy - self.world_extents.lo.y) * self.cell_size_inv),
        )
    }

    /// `worldToCellDist(w)`: `REAL_TO_INT_CEIL(w * cellSizeInv)`.
    pub fn world_to_cell_dist(&self, w: f32) -> i32 {
        real_to_int_ceil(w * self.cell_size_inv)
    }

    /// The index of the cell at `(x, y)`, `None` off the grid (`getCellAt`).
    pub(super) fn cell_index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.cell_count_x || y >= self.cell_count_y {
            None
        } else {
            Some((i64::from(y) * i64::from(self.cell_count_x) + i64::from(x)) as usize)
        }
    }

    /// `getCellAt(x, y)`: `None` (upstream's `NULL`) off the grid.
    pub fn get_cell_at(&self, x: i32, y: i32) -> Option<&PartitionCell> {
        self.cell_index(x, y).map(|i| &self.cells[i])
    }

    /// `getCellCenterPos(x, y, xx, yy)`.
    pub fn get_cell_center_pos(&self, x: i32, y: i32) -> (f32, f32) {
        let half = self.cell_size * 0.5;
        (
            self.world_extents.lo.x + (x as f32 * self.cell_size) + half,
            self.world_extents.lo.y + (y as f32 * self.cell_size) + half,
        )
    }

    // ---- building and tearing down the grid ----------------------------------

    /// `PartitionManager::init()`: sizes the grid to the terrain, records each
    /// cell's terrain height range (`PM_CACHE_TERRAIN_HEIGHT`) and builds the
    /// ring table `m_radiusVec` (`FASTER_GCO`). Without terrain the manager
    /// stays empty.
    pub fn init<H: PartitionHost>(&mut self, host: &H) {
        self.cell_size = host.partition_cell_size();
        if f64::from(self.cell_size) < 1.0 {
            self.cell_size = 1.0;
        }

        self.cell_size_inv = (1.0f64 / f64::from(self.cell_size)) as f32;

        if let Some(extent) = host.terrain_extent() {
            self.world_extents = extent;
            // you'd think it wouldn't be legal for terrainlogic to have zero area, but apparently
            // that's what it resets itself to. let's just make the world a simple place and pretend
            // it's nonzero in area, so that we can proceed without crashing.
            if self.world_extents.width() < 1.0 {
                self.world_extents.hi.x = self.world_extents.lo.x + 1.0;
            }
            if self.world_extents.height() < 1.0 {
                self.world_extents.hi.y = self.world_extents.lo.y + 1.0;
            }
            self.cell_count_x = real_to_int_ceil(self.world_extents.width() * self.cell_size_inv);
            self.cell_count_y = real_to_int_ceil(self.world_extents.height() * self.cell_size_inv);
            self.total_cell_count = self.cell_count_x.wrapping_mul(self.cell_count_y);
            self.cells = vec![PartitionCell::default(); self.total_cell_count.max(0) as usize];
            for x in 0..self.cell_count_x {
                for y in 0..self.cell_count_y {
                    let (lo_z, hi_z) = self.calc_heights(host, x, y);
                    let idx = self.cell_index(x, y).expect("in range");
                    self.cells[idx].init(x, y, lo_z, hi_z);
                }
            }

            self.calc_radius_vec();
        } else {
            self.cell_size = 0.0;
            self.cell_size_inv = 0.0;
            self.cell_count_x = 0;
            self.cell_count_y = 0;
            self.total_cell_count = 0;
            self.cells = Vec::new();
            self.world_extents = Region3D::default();
        }
    }

    /// `calcHeights` (file-local): the lowest and highest ground height
    /// sampled over a cell, at `MAP_XY_FACTOR` steps (a `float` accumulation).
    fn calc_heights<H: PartitionHost>(&self, host: &H, x: i32, y: i32) -> (f32, f32) {
        let cell_size = self.cell_size;
        let xbase = self.world_extents.lo.x + (x as f32 * cell_size);
        let ybase = self.world_extents.lo.y + (y as f32 * cell_size);
        let rough_step_size = MAP_XY_FACTOR; // no point in stepping smaller than grid scale
        let num_steps = Real::ceil(cell_size / rough_step_size);
        let step = cell_size / num_steps;
        let mut lo_z = collide::HUGE_DIST; // huge positive
        let mut hi_z = -collide::HUGE_DIST; // huge negative
        let mut yy = 0.0f32;
        while yy <= cell_size {
            let mut xx = 0.0f32;
            while xx <= cell_size {
                let h = host.ground_height(xbase + xx, ybase + yy);
                if h < lo_z {
                    lo_z = h;
                }
                if h > hi_z {
                    hi_z = h;
                }
                xx += step;
            }
            yy += step;
        }
        (lo_z, hi_z)
    }

    /// `calcMinRadius(cur)`: the smallest distance, in whole cells, two
    /// objects could be apart if one is in cell (0,0) and the other in `cur`
    /// (each can be anywhere in its cell).
    fn calc_min_radius(&self, cur: ICoord2D) -> i32 {
        let half_cell = (f64::from(self.cell_size) * 0.5) as f32;
        let center_pos = [
            (-half_cell, -half_cell),
            (half_cell, -half_cell),
            (-half_cell, half_cell),
            (half_cell, half_cell),
        ];

        let x = cur.x as f32 * self.cell_size;
        let y = cur.y as f32 * self.cell_size;
        let other_pos = [
            (x - half_cell, y - half_cell),
            (x + half_cell, y - half_cell),
            (x - half_cell, y + half_cell),
            (x + half_cell, y + half_cell),
        ];

        // double, not real
        let mut min_dist_sqr: f64 = 1e12;
        for c in &center_pos {
            for o in &other_pos {
                // the differences are taken in float, then widened
                let dx = f64::from(c.0 - o.0);
                let dy = f64::from(c.1 - o.1);
                let cur_dist_sqr = dx * dx + dy * dy;
                if min_dist_sqr > cur_dist_sqr {
                    min_dist_sqr = cur_dist_sqr;
                }
            }
        }

        // `sqrtf(double)`: the argument is narrowed to float
        let dist = f64::from(Real::sqrt(min_dist_sqr as f32));
        real_to_int_ceil((dist / f64::from(self.cell_size)) as f32)
    }

    /// `calcRadiusVec()`: `m_radiusVec[r]` lists the cell offsets that could
    /// hold objects at most `r` cells away from cell (0,0), in the order of
    /// the double loop (rows outermost).
    fn calc_radius_vec(&mut self) {
        let cell_size = self.cell_size;
        let cx = self.cell_count_x;
        let cy = self.cell_count_y;

        // double, not real
        let dx = f64::from(cx) * f64::from(cell_size);
        let dy = f64::from(cy) * f64::from(cell_size);
        let max_possible_dist = Real::sqrt(dx * dx + dy * dy);

        self.max_gco_radius = real_to_int_ceil((max_possible_dist / f64::from(cell_size)) as f32);

        self.radius_vec.clear();
        self.radius_vec
            .resize((self.max_gco_radius + 1).max(0) as usize, Vec::new());

        let mut cur = ICoord2D { x: 0, y: 0 };
        cur.y = -cy + 1;
        while cur.y < cy {
            cur.x = -cx + 1;
            while cur.x < cx {
                let cur_radius = self.calc_min_radius(cur);
                if cur_radius <= self.max_gco_radius && cur_radius >= 0 {
                    self.radius_vec[cur_radius as usize].push(cur);
                }
                cur.x += 1;
            }
            cur.y += 1;
        }
    }

    /// `PartitionManager::reset()`: forgets the pending unlooks and drops the
    /// grid. Every object must already be unregistered (as `GameLogic::reset`
    /// ensures); any module left over has its intersections forgotten, since
    /// the cells they pointed into are gone.
    pub fn reset(&mut self) {
        self.reset_pending_undo_shroud_reveal_queue();
        self.shutdown();
    }

    /// `shutdown()`.
    fn shutdown(&mut self) {
        self.updated_since_last_reset = false;
        self.remove_all_dirty_modules();

        self.radius_vec.clear();

        self.reset_pending_undo_shroud_reveal_queue();

        // `delete [] m_cells`: leftover intersections would dangle, so forget them
        for m in self.modules.iter_mut().flatten() {
            for c in &mut m.coi_array {
                *c = Coi::default();
            }
            m.coi_in_use_count = 0;
        }
        self.cells = Vec::new();

        self.cell_size = 0.0;
        self.cell_size_inv = 0.0;
        self.cell_count_x = 0;
        self.cell_count_y = 0;
        self.total_cell_count = 0;
        self.world_extents = Region3D::default();
        self.max_gco_radius = 0;
    }

    // ---- the dirty list ------------------------------------------------------

    /// `isInListDirtyModules`: `*pListHead == this || m_prevDirty || m_nextDirty`.
    pub(super) fn is_in_list_dirty_modules(&self, slot: usize) -> bool {
        let m = self.md(slot);
        self.dirty_modules == Some(slot) || m.prev_dirty.is_some() || m.next_dirty.is_some()
    }

    /// `prependToDirtyModules`.
    fn prepend_to_dirty_modules(&mut self, slot: usize) {
        let head = self.dirty_modules;
        self.md_mut(slot).next_dirty = head;
        if let Some(h) = head {
            self.md_mut(h).prev_dirty = Some(slot);
        }
        self.dirty_modules = Some(slot);
    }

    /// `removeFromDirtyModules`.
    pub(super) fn remove_from_dirty_modules(&mut self, slot: usize) {
        let (next, prev) = {
            let m = self.md_mut(slot);
            m.dirty_status = DirtyStatus::NotDirty;
            (m.next_dirty, m.prev_dirty)
        };
        if let Some(n) = next {
            self.md_mut(n).prev_dirty = prev;
        }
        if let Some(p) = prev {
            self.md_mut(p).next_dirty = next;
        } else {
            self.dirty_modules = next;
        }
        let m = self.md_mut(slot);
        m.prev_dirty = None;
        m.next_dirty = None;
    }

    /// `removeAllDirtyModules()`.
    fn remove_all_dirty_modules(&mut self) {
        while let Some(tmp) = self.dirty_modules {
            self.remove_from_dirty_modules(tmp);
        }
    }

    /// `PartitionData::makeDirty(needToUpdateCells)`: mark the module as needing
    /// recalculation during the next update phase. With `need_to_update_cells`
    /// the cells are recalculated and collisions checked; without, only
    /// collisions.
    pub(super) fn make_dirty(&mut self, slot: usize, need_to_update_cells: bool) {
        if !self.is_in_list_dirty_modules(slot) {
            let m = self.md_mut(slot);
            if need_to_update_cells {
                m.dirty_status = DirtyStatus::NeedCellUpdateAndCollisionCheck;
            } else if m.dirty_status == DirtyStatus::NotDirty {
                m.dirty_status = DirtyStatus::NeedCollisionCheck;
            }
            self.prepend_to_dirty_modules(slot);
        } else if need_to_update_cells {
            // If I am in the list already, I may be in there for a half update, so if my
            // needs have increased I'll upgrade what I want.
            self.md_mut(slot).dirty_status = DirtyStatus::NeedCellUpdateAndCollisionCheck;
        }
    }

    // ---- intersections -------------------------------------------------------

    /// `PartitionCell::friend_addToCellList` + `CellAndObjectIntersection::friend_addToCellList`.
    fn cell_add_to_list(&mut self, cell: usize, r: CoiRef) {
        let head = self.cells[cell].first_coi;
        self.coi_mut(r).next = head;
        if let Some(h) = head {
            self.coi_mut(h).prev = Some(r);
        }
        self.cells[cell].first_coi = Some(r);
        self.cells[cell].coi_count = self.cells[cell].coi_count.wrapping_add(1);
    }

    /// `PartitionCell::friend_removeFromCellList` + the COI's.
    fn cell_remove_from_list(&mut self, cell: usize, r: CoiRef) {
        let (prev, next) = {
            let c = self.coi(r);
            (c.prev, c.next)
        };
        if let Some(p) = prev {
            self.coi_mut(p).next = next;
        } else {
            self.cells[cell].first_coi = next;
        }
        if let Some(n) = next {
            self.coi_mut(n).prev = prev;
        }
        let c = self.coi_mut(r);
        c.prev = None;
        c.next = None;
        self.cells[cell].coi_count = self.cells[cell].coi_count.wrapping_sub(1);
    }

    /// `CellAndObjectIntersection::addCoverage(cell, module)`.
    fn coi_add_coverage(&mut self, r: CoiRef, cell: usize, module: usize) {
        let c = *self.coi(r);
        if c.module.is_some() && c.module != Some(module as u32) {
            return; // "COI already in use by another module!"
        }
        if c.cell.is_none() {
            self.cell_add_to_list(cell, r);
        }
        let c = self.coi_mut(r);
        c.cell = Some(cell as u32);
        c.module = Some(module as u32);
    }

    /// `CellAndObjectIntersection::removeAllCoverage()`.
    fn coi_remove_all_coverage(&mut self, r: CoiRef) {
        let c = *self.coi(r);
        if c.module.is_none() {
            return; // "COI not in use"
        }
        if let Some(cell) = c.cell {
            self.cell_remove_from_list(cell as usize, r);
        }
        let c = self.coi_mut(r);
        c.cell = None;
        c.module = None;
    }

    /// `PartitionData::removeAllTouchedCells()`.
    pub(super) fn remove_all_touched_cells(&mut self, slot: usize) {
        let n = self.md(slot).coi_array.len();
        for i in 0..n {
            let r = CoiRef {
                module: slot as u32,
                index: i as u32,
            };
            if self.coi(r).module.is_some() {
                self.coi_remove_all_coverage(r);
                self.md_mut(slot).coi_in_use_count -= 1;
            }
        }
    }

    /// `PartitionData::freeCoiArray()`: drops the array and (as upstream does)
    /// marks the module dirty.
    fn free_coi_array(&mut self, slot: usize) {
        let m = self.md_mut(slot);
        m.coi_array = Vec::new();
        m.coi_in_use_count = 0;
        self.make_dirty(slot, true);
    }

    /// `PartitionData::calcMaxCoiForShape`. Note upstream's missing `break`
    /// after the sphere/cylinder case: it falls into the box case, so every
    /// shape is sized by the box formula (see the module docs).
    pub fn calc_max_coi_for_shape(
        &self,
        _geom: GeometryType,
        major_radius: f32,
        minor_radius: f32,
        _is_small: bool,
    ) -> i32 {
        // (SPHERE, CYLINDER) cells = worldToCellDist(majorRadius*2) + 1; result = cells*cells; then
        // falls through to:
        let diagonal = Real::sqrt(major_radius * major_radius + minor_radius * minor_radius);
        let cells = self.world_to_cell_dist(diagonal * 2.0).wrapping_add(1);
        let mut result = cells.wrapping_mul(cells);
        if result < 4 {
            result = 4;
        }
        result
    }

    /// `PartitionData::allocCoiArray()` for the module's object.
    fn alloc_coi_array<H: PartitionHost>(&mut self, host: &H, slot: usize) {
        let count = match self.md(slot).object.and_then(|id| host.object_geometry(id)) {
            Some(og) => self.calc_max_coi_for_shape(
                og.geometry.get_geom_type(),
                og.geometry.get_major_radius(),
                og.geometry.get_minor_radius(),
                og.geometry.get_is_small(),
            ),
            None => 4,
        };
        let m = self.md_mut(slot);
        m.coi_array = vec![Coi::default(); count.max(0) as usize];
        m.coi_in_use_count = 0;
        self.make_dirty(slot, true);
    }

    // ---- registration --------------------------------------------------------

    /// Takes a free slot (or grows the slab) and stores `data` there.
    fn alloc_module(&mut self, data: PartitionData) -> usize {
        if let Some(s) = self.free_slots.pop() {
            self.modules[s] = Some(data);
            s
        } else {
            self.modules.push(Some(data));
            self.modules.len() - 1
        }
    }

    /// Links a new module at the head of the master list (`registerObject`).
    fn link_module(&mut self, slot: usize) {
        let head = self.module_list;
        let m = self.md_mut(slot);
        m.prev = None;
        m.next = head;
        if let Some(h) = head {
            self.md_mut(h).prev = Some(slot);
        }
        self.module_list = Some(slot);
    }

    /// Unlinks a module from the master list and deletes it (`deleteInstance`;
    /// its destructor drops the intersections and leaves the dirty list).
    fn unlink_and_delete_module(&mut self, slot: usize) {
        let (next, prev) = {
            let m = self.md(slot);
            (m.next, m.prev)
        };
        if let Some(n) = next {
            self.md_mut(n).prev = prev;
        }
        if let Some(p) = prev {
            self.md_mut(p).next = next;
        } else {
            self.module_list = next;
        }

        // ~PartitionData: removeAllTouchedCells(); freeCoiArray() (which marks it
        // dirty, prepending it if it was not in the list); then it leaves the
        // dirty list. The net effect is that it is out of the list.
        self.remove_all_touched_cells(slot);
        if self.is_in_list_dirty_modules(slot) {
            self.remove_from_dirty_modules(slot);
        }
        self.modules[slot] = None;
        self.free_slots.push(slot);
    }

    /// `registerObject(object)`: add a thing to the system. An object that is
    /// already registered is left alone.
    pub fn register_object<H: PartitionHost>(&mut self, host: &mut H, object: ObjectId) {
        // if object is already part of this system get out of here
        if self.by_object.contains_key(&object) {
            return;
        }

        // allocate a new module of partition data, link it to the list in the manager
        let slot = self.alloc_module(PartitionData::new());
        self.link_module(slot);

        // add module to object
        self.attach_to_object(host, slot, object);
    }

    /// `PartitionData::attachToObject(object)`.
    fn attach_to_object<H: PartitionHost>(&mut self, host: &mut H, slot: usize, object: ObjectId) {
        // remember who contains us
        self.md_mut(slot).object = Some(object);

        // we only snapshot things that are immobile and have something to draw
        // (the draw-module test and the ghost manager's answer are the host's)
        if host.is_kind_of(object, KINDOF_IMMOBILE) {
            let ghost = host.add_ghost_object(object);
            self.md_mut(slot).ghost = ghost;
        }

        // (re)calc maxCoi and (re)alloc cois
        self.free_coi_array(slot);
        self.alloc_coi_array(&*host, slot);

        self.by_object.insert(object, slot);
        self.make_dirty(slot, true);
    }

    /// `PartitionData::detachFromObject()`.
    fn detach_from_object(&mut self, slot: usize) {
        // if we are in the midst of processing the contact list, ensure that any
        // potential collisions get removed from that list (we are about to go bye-bye)
        self.contact_remove_specific(slot);

        if let Some(o) = self.md(slot).object {
            self.by_object.remove(&o);
        }
        self.remove_all_touched_cells(slot);
        self.free_coi_array(slot);

        // no longer attached to object
        let ghost = {
            let m = self.md_mut(slot);
            m.object = None;
            m.ghost.take()
        };
        if let Some(g) = ghost {
            self.events
                .push(PartitionClientEvent::GhostRemoved { ghost: g.handle });
        }
    }

    /// `unRegisterObject(object)`: remove a thing from the system. A fogged
    /// building some player remembers keeps its module (for its ghost).
    pub fn unregister_object(&mut self, object: ObjectId) {
        // get the partition module
        let Some(&slot) = self.by_object.get(&object) else {
            return;
        };

        // need to figure out if any players have a fogged memory of this object.
        // if so, we can't remove it from the shroud system just yet.
        if let Some(ghost) = self.md(slot).ghost.as_ref().map(|g| g.handle) {
            if self.md(slot).was_seen_by_any_players() < MAX_PLAYER_COUNT {
                self.contact_remove_specific(slot);
                self.by_object.remove(&object);
                self.md_mut(slot).object = None;
                // Tell the ghost object that its parent is dead.
                self.events
                    .push(PartitionClientEvent::GhostParentGone { ghost });
                return;
            }
        }

        // detach the module from the object, remove it from the master list and delete it
        self.detach_from_object(slot);
        self.unlink_and_delete_module(slot);
    }

    /// `registerGhostObject(ghost)`: recreate the partition data needed to hold
    /// a ghost (only used to restore after a reset). A ghost that is already
    /// registered is left alone.
    pub fn register_ghost_object(&mut self, ghost: GhostGeometry) {
        if self
            .modules
            .iter()
            .flatten()
            .any(|m| m.ghost.as_ref().map(|g| g.handle) == Some(ghost.handle))
        {
            return;
        }
        let slot = self.alloc_module(PartitionData::new());
        self.link_module(slot);

        // attachToGhostObject
        {
            let m = self.md_mut(slot);
            m.object = None; // it's only attached to a ghost object, no parent object.
            m.ghost = Some(ghost.clone());
        }
        self.free_coi_array(slot);
        let count = self.calc_max_coi_for_shape(
            ghost.geometry.get_geom_type(),
            ghost.geometry.get_major_radius(),
            ghost.geometry.get_minor_radius(),
            ghost.geometry.get_is_small(),
        );
        {
            let m = self.md_mut(slot);
            m.coi_array = vec![Coi::default(); count.max(0) as usize];
            m.coi_in_use_count = 0;
        }
        self.make_dirty(slot, true);
        self.events.push(PartitionClientEvent::GhostParentGone {
            ghost: ghost.handle,
        });
    }

    /// `unRegisterGhostObject(ghost)`: release the partition data held for a ghost.
    pub fn unregister_ghost_object(&mut self, handle: u32) {
        let Some(slot) = (0..self.modules.len()).find(|&s| {
            self.modules[s]
                .as_ref()
                .is_some_and(|m| m.ghost.as_ref().map(|g| g.handle) == Some(handle))
        }) else {
            return;
        };
        // detachFromGhostObject
        self.contact_remove_specific(slot);
        if let Some(o) = self.md(slot).object {
            self.by_object.remove(&o);
        }
        self.remove_all_touched_cells(slot);
        self.free_coi_array(slot);
        {
            let m = self.md_mut(slot);
            m.object = None;
            m.ghost = None;
        }
        self.unlink_and_delete_module(slot);
    }

    /// The cells the object's module touches, as `(x, y)`, in the order of its
    /// intersection array (the order the rasteriser found them).
    pub fn cells_touched_by(&self, object: ObjectId) -> Vec<(i32, i32)> {
        let Some(&slot) = self.by_object.get(&object) else {
            return Vec::new();
        };
        let m = self.md(slot);
        m.coi_array
            .iter()
            .take(m.coi_in_use_count.max(0) as usize)
            .filter_map(|c| c.cell)
            .map(|c| {
                (
                    self.cells[c as usize].get_cell_x(),
                    self.cells[c as usize].get_cell_y(),
                )
            })
            .collect()
    }

    /// The objects with an intersection in the cell, in the order of the cell's
    /// list (newest intersection first); `None` for a module with no object (a
    /// ghost that outlived its object).
    pub fn objects_in_cell(&self, x: i32, y: i32) -> Vec<Option<ObjectId>> {
        let mut out = Vec::new();
        let Some(c) = self.cell_index(x, y) else {
            return out;
        };
        let mut coi = self.cells[c].first_coi;
        while let Some(r) = coi {
            out.push(self.coi(r).module.and_then(|m| self.md(m as usize).object));
            coi = self.coi(r).next;
        }
        out
    }

    /// The registered objects in master-list order (newest registration first).
    pub fn registered_objects(&self) -> Vec<Option<ObjectId>> {
        let mut out = Vec::new();
        let mut m = self.module_list;
        while let Some(s) = m {
            out.push(self.md(s).object);
            m = self.md(s).next;
        }
        out
    }

    /// The dirty list's objects, head first (the order `update` processes them).
    pub fn dirty_objects(&self) -> Vec<Option<ObjectId>> {
        let mut out = Vec::new();
        let mut m = self.dirty_modules;
        while let Some(s) = m {
            out.push(self.md(s).object);
            m = self.md(s).next_dirty;
        }
        out
    }

    /// The whole observable state of the manager as words, for the differential
    /// test (`differential/wrap_partition.cpp` `dumpState` writes the same
    /// words from the C++ records): whether it has updated, the module list and
    /// the dirty list as object ids (0 for a module with no object), per module
    /// its intersections, dirty status, shroudedness (current, previous, ever
    /// seen) and last cell, per cell its intersection count, its list of object
    /// ids and a hash each of its shroud, threat and cash arrays, and the queue of
    /// pending unlooks.
    #[doc(hidden)]
    pub fn state_words(&self) -> Vec<u32> {
        fn fold(h: u32, w: u32) -> u32 {
            (h ^ w).wrapping_mul(16_777_619)
        }
        let id_of = |o: Option<ObjectId>| o.map_or(0u32, |i| i as u32);
        let mut out = vec![u32::from(self.updated_since_last_reset)];
        let mut mods = Vec::new();
        let mut m = self.module_list;
        while let Some(s) = m {
            mods.push(s);
            m = self.md(s).next;
        }
        out.push(mods.len() as u32);
        out.extend(mods.iter().map(|&s| id_of(self.md(s).object)));
        let mut dirty = Vec::new();
        let mut m = self.dirty_modules;
        while let Some(s) = m {
            dirty.push(s);
            m = self.md(s).next_dirty;
        }
        out.push(dirty.len() as u32);
        out.extend(dirty.iter().map(|&s| id_of(self.md(s).object)));
        for &s in &mods {
            let m = self.md(s);
            out.push(id_of(m.object));
            out.push(m.coi_in_use_count as u32);
            out.push(m.coi_array.len() as u32);
            for k in 0..m.coi_in_use_count.max(0) as usize {
                out.push(m.coi_array.get(k).and_then(|c| c.cell).unwrap_or(u32::MAX));
            }
            out.push(match m.dirty_status {
                DirtyStatus::NotDirty => 0,
                DirtyStatus::NeedCollisionCheck => 1,
                DirtyStatus::NeedCellUpdateAndCollisionCheck => 2,
            });
            out.extend(m.shroudedness.iter().map(|&x| x as u32));
            out.extend(m.shroudedness_previous.iter().map(|&x| x as u32));
            let mut seen = 0u32;
            for (p, &e) in m.ever_seen_by_player.iter().enumerate() {
                if e {
                    seen |= 1 << p;
                }
            }
            out.push(seen);
            out.push(m.last_cell.map_or(u32::MAX, |c| c as u32));
        }
        for cell in self.cells.iter() {
            out.push(cell.coi_count as u32);
            let mut ids = Vec::new();
            let mut coi = cell.first_coi;
            while let Some(r) = coi {
                ids.push(id_of(
                    self.coi(r).module.and_then(|m| self.md(m as usize).object),
                ));
                coi = self.coi(r).next;
            }
            out.push(ids.len() as u32);
            out.extend(ids);
            let (mut hs, mut ht, mut hc) = (2_166_136_261u32, 2_166_136_261u32, 2_166_136_261u32);
            for p in 0..MAX_PLAYER_COUNT as usize {
                hs = fold(hs, u32::from(cell.shroud_level[p].current_shroud as u16));
                hs = fold(
                    hs,
                    u32::from(cell.shroud_level[p].active_shroud_level as u16),
                );
                ht = fold(ht, cell.threat_value[p] as u32);
                hc = fold(hc, cell.cash_value[p] as u32);
            }
            out.extend([hs, ht, hc]);
        }
        out.push(self.pending_undo_shroud_reveals.len() as u32);
        for s in &self.pending_undo_shroud_reveals {
            out.extend([
                s.where_.x.to_bits(),
                s.where_.y.to_bits(),
                s.how_far.to_bits(),
                u32::from(s.for_whom),
                s.data,
            ]);
        }
        out
    }

    /// `m_radiusVec`: the cell offsets of each ring, in table order.
    pub fn radius_vec(&self) -> &[Vec<ICoord2D>] {
        &self.radius_vec
    }

    /// `m_maxGcoRadius`.
    pub fn max_gco_radius(&self) -> i32 {
        self.max_gco_radius
    }

    /// Whether a module holds the ghost with this handle and no object (the
    /// state in which the ghost manager may call `unRegisterGhostObject`).
    pub fn has_objectless_ghost_module(&self, handle: u32) -> bool {
        self.modules
            .iter()
            .flatten()
            .any(|m| m.object.is_none() && m.ghost.as_ref().map(|g| g.handle) == Some(handle))
    }

    /// `object->friend_getPartitionData() != NULL`.
    pub fn is_registered(&self, object: ObjectId) -> bool {
        self.by_object.contains_key(&object)
    }

    /// `object->friend_getPartitionData()->makeDirty(needToUpdateCells)`: the
    /// object moved (cells too) or must be re-checked for collisions.
    pub fn make_object_dirty(&mut self, object: ObjectId, need_to_update_cells: bool) {
        if let Some(&slot) = self.by_object.get(&object) {
            self.make_dirty(slot, need_to_update_cells);
        }
    }

    // ---- the footprint rasterisers -------------------------------------------

    /// `addSubPixToCoverage(cell)`: set the "pixel" at a cell, reusing the
    /// module's intersection for it if there is one.
    fn add_sub_pix_to_coverage(&mut self, slot: usize, cell: usize) {
        let (in_use, count) = {
            let m = self.md(slot);
            (m.coi_in_use_count, m.coi_array.len() as i32)
        };
        // see if we already have a coi for this cell.
        let mut coi_to_use: Option<usize> = None;
        let n = in_use.min(count).max(0) as usize;
        for i in 0..n {
            if self.md(slot).coi_array[i].cell == Some(cell as u32) {
                coi_to_use = Some(i);
                break;
            }
        }
        if coi_to_use.is_none() && in_use < count {
            // nope, no coi for this cell, allocate a new one
            coi_to_use = Some(in_use as usize);
            self.md_mut(slot).coi_in_use_count += 1;
        }
        if let Some(i) = coi_to_use {
            self.coi_add_coverage(
                CoiRef {
                    module: slot as u32,
                    index: i as u32,
                },
                cell,
                slot,
            );
        }
    }

    /// `hLineCircle(x1, x2, y)`.
    fn h_line_circle(&mut self, slot: usize, x1: i32, x2: i32, y: i32) {
        let mut x = x1;
        while x <= x2 {
            if let Some(cell) = self.cell_index(x, y) {
                self.add_sub_pix_to_coverage(slot, cell);
            }
            if x == i32::MAX {
                break;
            }
            x += 1;
        }
    }

    /// `doCircleFill`: the cells covered by a circle (used for spheres and cylinders).
    fn do_circle_fill(&mut self, slot: usize, center_x: f32, center_y: f32, radius: f32) {
        let (cell_center_x, cell_center_y) = self.world_to_cell(center_x, center_y);

        let mut cell_radius = self.world_to_cell_dist(radius);
        if cell_radius < 1 {
            cell_radius = 1;
        }

        let mut y = cell_radius.wrapping_sub(1);
        let mut dec = 3i32.wrapping_sub(cell_radius.wrapping_mul(2));
        for x in 0..cell_radius {
            self.h_line_circle(
                slot,
                cell_center_x.wrapping_sub(x),
                cell_center_x.wrapping_add(x),
                cell_center_y.wrapping_add(y),
            );
            self.h_line_circle(
                slot,
                cell_center_x.wrapping_sub(x),
                cell_center_x.wrapping_add(x),
                cell_center_y.wrapping_sub(y),
            );
            self.h_line_circle(
                slot,
                cell_center_x.wrapping_sub(y),
                cell_center_x.wrapping_add(y),
                cell_center_y.wrapping_add(x),
            );
            self.h_line_circle(
                slot,
                cell_center_x.wrapping_sub(y),
                cell_center_x.wrapping_add(y),
                cell_center_y.wrapping_sub(x),
            );

            if dec >= 0 {
                dec = dec.wrapping_add((1i32.wrapping_sub(y)).wrapping_shl(2));
                y = y.wrapping_sub(1);
            }
            dec = dec.wrapping_add((x.wrapping_shl(2)).wrapping_add(6));
        }
    }

    /// `doSmallFill`: a "small" shape covers at most 4 cells, so a special-purpose
    /// filler replaces the general rasteriser.
    fn do_small_fill(&mut self, slot: usize, center_x: f32, center_y: f32, mut radius: f32) {
        let half_cell_size = self.cell_size * 0.5;
        if radius > half_cell_size {
            radius = half_cell_size; // "object is too large to use a 'small' geometry, truncating size to cellsize"
        }

        let (cx1, cy1) = self.world_to_cell(center_x - radius, center_y - radius);
        let (cx2, cy2) = self.world_to_cell(center_x + radius, center_y + radius);

        let mut x = cx1;
        while x <= cx2 {
            let mut y = cy1;
            while y <= cy2 {
                if let Some(cell) = self.cell_index(x, y) {
                    let idx = self.md(slot).coi_in_use_count;
                    // upstream does not check the array size (see the module docs)
                    if (idx as usize) < self.md(slot).coi_array.len() {
                        self.md_mut(slot).coi_in_use_count += 1;
                        self.coi_add_coverage(
                            CoiRef {
                                module: slot as u32,
                                index: idx as u32,
                            },
                            cell,
                            slot,
                        );
                    }
                }
                y += 1;
            }
            x += 1;
        }
    }

    /// `doRectFill`: the cells covered by a rotated rectangle, sampled at half
    /// a cell steps ("needs to be smaller to avoid aliasing problems").
    fn do_rect_fill(
        &mut self,
        slot: usize,
        center_x: f32,
        center_y: f32,
        halfsize_x: f32,
        halfsize_y: f32,
        angle: f32,
    ) {
        let c = crate::common::trig::cos(angle);
        let s = crate::common::trig::sin(angle);

        let actual_cell_size = self.cell_size;
        let step_size = actual_cell_size * 0.5; // in theory, should be getCellSize() exactly, but needs to be smaller
        let ydx = s * step_size;
        let ydy = -c * step_size;
        let xdx = c * step_size;
        let xdy = s * step_size;

        // the step size has been fudged down, so the number of steps uses the fudged size
        let step_size_inv_times2 = 2.0 * (1.0 / step_size);

        let num_steps_x = real_to_int_ceil(halfsize_x * step_size_inv_times2);
        let num_steps_y = real_to_int_ceil(halfsize_y * step_size_inv_times2);

        let mut tl_x = center_x - halfsize_x * c - halfsize_y * s;
        let mut tl_y = center_y + halfsize_y * c - halfsize_x * s;

        let mut iy = 0;
        while iy < num_steps_y {
            let mut x = tl_x;
            let mut y = tl_y;
            let mut ix = 0;
            while ix < num_steps_x {
                let (cellx, celly) = self.world_to_cell(x, y);
                if let Some(cell) = self.cell_index(cellx, celly) {
                    // might be null if off the edge
                    self.add_sub_pix_to_coverage(slot, cell);
                }
                ix += 1;
                x += xdx;
                y += xdy;
            }
            iy += 1;
            tl_x += ydx;
            tl_y += ydy;
        }
    }

    /// `PartitionData::updateCellsTouched()`: discard the touch information and
    /// recalculate the cells the module's geometry touches.
    pub(super) fn update_cells_touched<H: PartitionHost>(&mut self, host: &mut H, slot: usize) {
        let object = self.md(slot).object;
        let (geom_type, is_small, pos, angle, major_radius, minor_radius) = if let Some(id) = object
        {
            match host.object_geometry(id) {
                Some(og) => (
                    og.geometry.get_geom_type(),
                    og.geometry.get_is_small(),
                    og.position,
                    og.orientation,
                    og.geometry.get_major_radius(),
                    og.geometry.get_minor_radius(),
                ),
                None => return,
            }
        } else if let Some(g) = &self.md(slot).ghost {
            // we have no object using this PartitionData but we still have a GhostObject so copy its data.
            (
                g.geometry.get_geom_type(),
                g.geometry.get_is_small(),
                g.position,
                g.angle,
                g.geometry.get_major_radius(),
                g.geometry.get_minor_radius(),
            )
        } else {
            return;
        };

        self.remove_all_touched_cells(slot);
        if is_small {
            self.do_small_fill(slot, pos.x, pos.y, major_radius);
        } else {
            match geom_type {
                GeometryType::Sphere | GeometryType::Cylinder => {
                    self.do_circle_fill(slot, pos.x, pos.y, major_radius)
                }
                GeometryType::Box => {
                    self.do_rect_fill(slot, pos.x, pos.y, major_radius, minor_radius, angle)
                }
            }
        }

        let (current_cell_index_x, current_cell_index_y) = self.world_to_cell(pos.x, pos.y);
        let current_cell = self.cell_index(current_cell_index_x, current_cell_index_y);
        let last_cell = self.md(slot).last_cell;
        if let Some(id) = object {
            if current_cell != last_cell {
                // To not expose PartitionCells, the object thinks in terms of points: it
                // unlooks at a point and looks at the new point.
                host.on_partition_cell_change(self, id);
                if self.is_live(slot) {
                    self.md_mut(slot).last_cell = current_cell;
                }
            }
        }

        // if we have moved, our shroudedness status might be different for all players,
        // so it must all be invalidated.
        if self.is_live(slot) {
            self.invalidate_shrouded_status_for_all_players(slot);
        }
    }

    // ---- the contact list and the update pass --------------------------------

    /// `PartitionContactList::removeSpecificPartitionData(data)`.
    fn contact_remove_specific(&mut self, slot: usize) {
        if let Some(cl) = &mut self.contact_list {
            for cd in &mut cl.nodes {
                if cd.obj == Some(slot) || cd.other == Some(slot) {
                    cd.obj = None;
                    cd.other = None;
                }
            }
        }
    }

    /// `PartitionContactList::addToContactList(obj, other)`: a pair already
    /// present (in either order) is not added again.
    fn contact_add(&mut self, obj: usize, other: usize) {
        if obj == other {
            return;
        }
        let (Some(obj_id), Some(other_id)) = (self.md(obj).object, self.md(other).object) else {
            return;
        };

        // compute hash index based on object's ids.
        let hash_value = (hash2ints(obj_id, other_id) % CONTACT_SOCKET_COUNT as u32) as usize;

        let Some(cl) = &mut self.contact_list else {
            return;
        };
        if cl.heads.is_empty() {
            cl.heads = vec![None; CONTACT_SOCKET_COUNT];
        }

        // make sure given hit has not already been recorded
        let mut cd = cl.heads[hash_value];
        while let Some(i) = cd {
            let n = cl.nodes[i];
            if (n.obj == Some(obj) && n.other == Some(other))
                || (n.obj == Some(other) && n.other == Some(obj))
            {
                return; // already noted
            }
            cd = n.next_hash;
        }

        // new hit: add to hash table and to the list of contacts for this frame
        let head = cl.heads[hash_value];
        cl.nodes.push(ContactNode {
            obj: Some(obj),
            other: Some(other),
            next_hash: head,
        });
        cl.heads[hash_value] = Some(cl.nodes.len() - 1);
    }

    /// `PartitionData::addPossibleCollisions(ctList)`: the objects that share a
    /// cell with this module are possible collisions.
    fn add_possible_collisions(&mut self, slot: usize) {
        let in_use = self.md(slot).coi_in_use_count;
        for i in 0..in_use.max(0) as usize {
            let Some(cell) = self.md(slot).coi_array.get(i).and_then(|c| c.cell) else {
                continue;
            };
            let cell = cell as usize;
            if self.cells[cell].coi_count < 2 {
                continue;
            }

            let mut coi = self.cells[cell].first_coi;
            while let Some(r) = coi {
                if let Some(that) = self.coi(r).module {
                    if that as usize != slot {
                        self.contact_add(slot, that as usize);
                    }
                }
                coi = self.coi(r).next;
            }
        }
    }

    /// `PartitionData::collidesWith(that, cinfo)`: a careful test of the
    /// geometries of the two modules' objects.
    fn collides_with<H: PartitionHost>(
        &self,
        host: &H,
        this: usize,
        that: usize,
        cinfo: Option<&mut CollideLocAndNormal>,
    ) -> bool {
        let (Some(this_obj), Some(that_obj)) = (self.md(this).object, self.md(that).object) else {
            return false;
        };
        if host.is_kind_of(this_obj, KINDOF_NO_COLLIDE)
            || host.is_kind_of(that_obj, KINDOF_NO_COLLIDE)
        {
            return false; // A collision extent of zero size is still a point and can collide, but we don't always want to.
        }
        let (Some(a), Some(b)) = (
            host.object_geometry(this_obj),
            host.object_geometry(that_obj),
        ) else {
            return false;
        };
        let this_info = CollideInfo::new(&a.position, &a.geometry, a.orientation);
        let that_info = CollideInfo::new(&b.position, &b.geometry, b.orientation);
        collide::collides_with(&this_info, &that_info, cinfo)
    }

    /// `isColliding(a, b)`: whether two registered objects' geometries touch.
    pub fn is_colliding<H: PartitionHost>(&self, host: &H, a: ObjectId, b: ObjectId) -> bool {
        let (Some(&ad), Some(&bd)) = (self.by_object.get(&a), self.by_object.get(&b)) else {
            return false;
        };
        self.collides_with(host, ad, bd, None)
    }

    /// `PartitionContactList::processContactList()`.
    fn process_contact_list<H: PartitionHost>(&mut self, host: &mut H) {
        // m_contactList runs from the newest node
        let mut i = self.contact_list.as_ref().map_or(0, |c| c.nodes.len());
        while i > 0 {
            i -= 1;
            let Some(cl) = &self.contact_list else {
                return;
            };
            let (Some(obj_slot), Some(other_slot)) = (cl.nodes[i].obj, cl.nodes[i].other) else {
                continue;
            };

            // we know that their partitions overlap; determine if they REALLY collide before proceeding...
            let mut cinfo = CollideLocAndNormal::default();
            if !self.collides_with(&*host, obj_slot, other_slot, Some(&mut cinfo)) {
                continue;
            }

            let (Some(obj), Some(other)) = (self.md(obj_slot).object, self.md(other_slot).object)
            else {
                continue;
            };

            if host.status_bits(obj).test(OBJECT_STATUS_NO_COLLISIONS)
                || host.status_bits(other).test(OBJECT_STATUS_NO_COLLISIONS)
            {
                continue;
            }

            // the onCollide() calls can remove the object(s) from the partition mgr,
            // thus destroying the partitiondata for 'em. go ahead and null these out here
            // so we won't be tempted to use 'em (since they might be bogus).
            if let Some(cl) = &mut self.contact_list {
                cl.nodes[i].obj = None;
                cl.nodes[i].other = None;
            }

            host.on_collide(self, obj, other, &cinfo.loc, &cinfo.normal);
            cinfo.normal.x = -cinfo.normal.x;
            cinfo.normal.y = -cinfo.normal.y;
            cinfo.normal.z = -cinfo.normal.z;

            // Before checking the "other" case, make sure that the previous collision didn't absorb him.
            if !host.is_destroyed(obj) && !host.is_destroyed(other) {
                host.on_collide(self, other, obj, &cinfo.loc, &cinfo.normal);
            }

            // NOTE: it is VERY IMPORTANT (for performance reasons) to not re-dirty immobile things.
            // NOTE also that we re-get partitiondata from the object, since it might have been
            // removed from the partition system by the onCollide call...
            if !host.is_destroyed(obj) && !host.is_kind_of(obj, KINDOF_IMMOBILE) {
                if let Some(&s) = self.by_object.get(&obj) {
                    self.make_dirty(s, false);
                }
            }
            if !host.is_destroyed(other) && !host.is_kind_of(other, KINDOF_IMMOBILE) {
                if let Some(&s) = self.by_object.get(&other) {
                    self.make_dirty(s, false);
                }
            }
        }
    }

    /// `PartitionManager::update()`: bring every dirty module up to date (cells,
    /// then possible collisions), process the collisions, then the pending
    /// unlooks that are due.
    pub fn update<H: PartitionHost>(&mut self, host: &mut H) {
        if !self.updated_since_last_reset {
            self.updated_since_last_reset = true;
        }

        self.contact_list = Some(ContactList::default());
        while let Some(dirty) = self.dirty_modules {
            // get this BEFORE removing from dirty list, since that clears the flag in question.
            let (update_em, collide_em) = {
                let m = self.md(dirty);
                (
                    m.is_in_need_of_updating_cells(),
                    m.is_in_need_of_collision_check() && m.object.is_some(),
                ) // only update collisions if we have object
            };

            // detach it from the dirty list.
            self.remove_from_dirty_modules(dirty);

            if update_em {
                self.update_cells_touched(host, dirty);
            }

            if collide_em && self.is_live(dirty) {
                if let Some(obj) = self.md(dirty).object {
                    if !host.is_kind_of(obj, KINDOF_IMMOBILE) {
                        self.add_possible_collisions(dirty);
                    }
                }
            }
        }

        self.process_contact_list(host);
        self.contact_list = None;

        self.process_pending_undo_shroud_reveal_queue(&*host, true);
    }
}
