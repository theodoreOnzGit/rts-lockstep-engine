// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h (ZoneBlock, PathfindZoneManager)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (lines 1768-3197: the zone helpers, ZoneBlock, PathfindZoneManager)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `ZoneBlock` and `PathfindZoneManager`: the pathfinder's zones. `p1w13-sonnet`, layer 10, batch P3.
//!
//! A zone is a connected area of cells of one type; two cells in different zones of every equivalence a locomotor set accepts cannot
//! reach each other, which the pathfinder checks before it searches. The manager numbers the zones of the ground map block by block
//! (10 by 10 cells), collapses the numbers, gives each layer a zone of its own, and keeps the equivalence tables: cells of the same
//! type (hierarchical), ground and water, ground and rubble, ground and cliff, terrain (an obstacle counted as clear), and a crusher's
//! (a fence counted as clear). Each block keeps the same tables for its own zones, for the hierarchical search.
//!
//! # Upstream facts kept
//!
//! - `waterGround` tests a type's water BIT (`type & CELL_WATER`): rubble (3) and a bridge's impassable cell (5) count as water.
//! - The tables of the first pass are a local array of 24,000 zones, and the zone numbers of the map are not checked against it.
//! - The left neighbour skips the water, rubble and cliff tables when the terrain or crusher table took the pair; the top neighbour
//!   does not (upstream's two ladders differ).
//! - A layer, the invalid and ground ones included, gets a fresh zone each calculation; a used, undestroyed layer marks the blocks of
//!   its two ends as interacting with a bridge.
//! - `ZoneBlock::allocateZones` keeps the tables while they are large enough, and `freeZones` does not forget their size;
//!   `PathfindZoneManager::freeZones` does.
//! - `setPassable` and its kin divide a negative cell index towards zero (cells -9..-1 are block 0's).
//!
//! # The shape
//!
//! The map is the store's first cells, column by column (`map[i][j]` is cell `i * height + j`, [`GroundMap`]); the layers are
//! [`PathfindLayerState`]s. Upstream's arrays are `Vec`s, absent while upstream's are `NULL`.

use crate::common::base_type::{ICoord2D, IRegion2D};
use crate::common::game_type::LAYER_GROUND;
use crate::logic::ai::cell::{CellId, CellStore, CELL_CLEAR, CELL_CLIFF, CELL_OBSTACLE, CELL_RUBBLE, CELL_WATER};
use crate::logic::ai::layer::PathfindLayerState;
use crate::logic::locomotor::{
    LocomotorSurfaceTypeMask, LOCOMOTORSURFACE_AIR, LOCOMOTORSURFACE_CLIFF, LOCOMOTORSURFACE_GROUND, LOCOMOTORSURFACE_RUBBLE,
    LOCOMOTORSURFACE_WATER,
};
use crate::logic::modules::plant;

/// `zoneStorageType`.
pub type ZoneStorage = u16;

/// `PathfindZoneManager::INITIAL_ZONES`.
pub const INITIAL_ZONES: u16 = 256;
/// `PathfindZoneManager::ZONE_BLOCK_SIZE` (the comment says 20; it is 10).
pub const ZONE_BLOCK_SIZE: i32 = 10;
/// `PathfindZoneManager::UNINITIALIZED_ZONE`.
pub const UNINITIALIZED_ZONE: ZoneStorage = 0;
/// `ZONE_UPDATE_FREQUENCY` (a file static).
pub const ZONE_UPDATE_FREQUENCY: u32 = 300;
/// `maxZones`, the first pass's local tables.
const MAX_ZONES: usize = 24000;

/// The ground map: `Pathfinder::m_map` over `m_extent` (cell `(i, j)` is the store's cell `i * height + j`, `height = hi.y + 1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundMap {
    pub extent: IRegion2D,
}

impl GroundMap {
    /// `&map[i][j]` (unchecked, as upstream indexes).
    pub fn cell(&self, i: i32, j: i32) -> CellId {
        (i * (self.extent.hi.y + 1) + j) as CellId
    }
    /// `Pathfinder::getCell(LAYER_GROUND, x, y)`: the cell, or none outside the extent.
    pub fn get(&self, x: i32, y: i32) -> Option<CellId> {
        let e = &self.extent;
        (x >= e.lo.x && x <= e.hi.x && y >= e.lo.y && y <= e.hi.y).then(|| self.cell(x, y))
    }
}

// ---- the helpers --------------------------------------------------------------------------------------------------------------

fn ty(s: &CellStore, c: CellId) -> u8 {
    s.cells[c as usize].get_type()
}

/// `typesMatch(target, source)`.
pub fn types_match(s: &CellStore, target: CellId, source: CellId) -> bool {
    ty(s, target) == ty(s, source)
}

/// `waterGround(target, source)`: one clear and the other's water bit set.
pub fn water_ground(s: &CellStore, target: CellId, source: CellId) -> bool {
    let (t, src) = (ty(s, target), ty(s, source));
    (t == CELL_CLEAR && (src & CELL_WATER) != 0) || (src == CELL_CLEAR && (t & CELL_WATER) != 0 && !plant(3030))
}

/// `groundRubble(target, source)`.
pub fn ground_rubble(s: &CellStore, target: CellId, source: CellId) -> bool {
    let (t, src) = (ty(s, target), ty(s, source));
    (t == CELL_CLEAR && src == CELL_RUBBLE) || (src == CELL_CLEAR && t == CELL_RUBBLE)
}

/// `terrain(target, source)`: the same type, an obstacle counted as clear.
pub fn terrain(s: &CellStore, target: CellId, source: CellId) -> bool {
    let clear = |t: u8| if t == CELL_OBSTACLE { CELL_CLEAR } else { t };
    clear(ty(s, target)) == clear(ty(s, source))
}

/// `crusherGround(target, source)`: a fence and a clear cell.
pub fn crusher_ground(s: &CellStore, target: CellId, source: CellId) -> bool {
    let (t, src) = (ty(s, target), ty(s, source));
    if t == CELL_OBSTACLE && s.is_obstacle_fence(target) && src == CELL_CLEAR {
        return true;
    }
    src == CELL_OBSTACLE && s.is_obstacle_fence(source) && t == CELL_CLEAR
}

/// `groundCliff(target, source)`.
pub fn ground_cliff(s: &CellStore, target: CellId, source: CellId) -> bool {
    let (t, src) = (ty(s, target), ty(s, source));
    (t == CELL_CLIFF && src == CELL_CLEAR) || (t == CELL_CLEAR && src == CELL_CLIFF)
}

/// `resolveBlockZones(srcZone, targetZone, zoneEquivalency, sizeOfZE)`: "keep the lower zone" (the entries equal to the higher one
/// become the lower one).
fn resolve_block_zones(src_zone: i32, target_zone: i32, eq: &mut [ZoneStorage], size: i32) {
    let (from, to) = if target_zone < src_zone { (src_zone, target_zone) } else { (target_zone, src_zone) };
    for e in eq.iter_mut().take(size.max(0) as usize) {
        if i32::from(*e) == from {
            *e = to as ZoneStorage;
        }
    }
}

/// `resolveZones(srcZone, targetZone, zoneEquivalency, sizeOfZE)`: both through the table, then every entry equal to either becomes the
/// table's entry of the lower.
fn resolve_zones(src_zone: i32, target_zone: i32, eq: &mut [ZoneStorage], size: i32) {
    let src = i32::from(eq[src_zone as usize]);
    let target = i32::from(eq[target_zone as usize]);
    let final_zone = if (target < src) != plant(3042) { eq[target as usize] } else { eq[src as usize] };
    for e in eq.iter_mut().take(size.max(0) as usize) {
        let ze = i32::from(*e);
        if ze == target || ze == src {
            *e = final_zone;
        }
    }
}

/// `flattenZones(zoneArray, zoneHierarchical, sizeOfZones)`.
fn flatten_zones(zones: &mut [ZoneStorage], hier: &[ZoneStorage], size: i32) {
    for i in 0..size.max(0) as usize {
        let zone1 = zones[i] as usize;
        let zone2 = hier[zone1] as usize;
        let zone1 = zones[zone2] as usize;
        zones[i] = hier[zone1];
    }
    for i in 0..size.max(0) as usize {
        let (zone1, zone2) = (i32::from(zones[i]), i32::from(hier[i]));
        if zone1 != zone2 {
            resolve_zones(zone1, zone2, zones, size);
        }
    }
}

/// `applyZone(targetCell, sourceCell, zoneEquivalency, sizeOfZE)`: an unzoned target takes the source's zone; different zones merge.
fn apply_zone(s: &mut CellStore, target: CellId, source: CellId, eq: &mut [ZoneStorage], size: i32) {
    let src_zone = i32::from(eq[s.cells[source as usize].get_zone() as usize]);
    let target_zone = i32::from(eq[s.cells[target as usize].get_zone() as usize]);
    if target_zone == 0 {
        s.cells[target as usize].set_zone(src_zone as ZoneStorage);
        return;
    }
    if target_zone == src_zone {
        return; // "already match."
    }
    resolve_zones(src_zone, target_zone, eq, size);
}

/// `applyBlockZone(targetCell, sourceCell, zoneEquivalency, firstZone, sizeOfZE)`.
fn apply_block_zone(s: &CellStore, target: CellId, source: CellId, eq: &mut [ZoneStorage], first_zone: i32, size: i32) {
    let src_zone = i32::from(eq[(i32::from(s.cells[source as usize].get_zone()) - first_zone) as usize]);
    let target_zone = i32::from(eq[(i32::from(s.cells[target as usize].get_zone()) - first_zone) as usize]);
    if target_zone == src_zone {
        return; // "already match."
    }
    resolve_block_zones(src_zone, target_zone, eq, size);
}

/// The four equivalence tables of a block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockTables {
    pub ground_cliff: Vec<ZoneStorage>,
    pub ground_water: Vec<ZoneStorage>,
    pub ground_rubble: Vec<ZoneStorage>,
    pub crusher: Vec<ZoneStorage>,
}

// ---- ZoneBlock ----------------------------------------------------------------------------------------------------------------

/// `ZoneBlock`: a block's zones and their equivalences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneBlock {
    pub cell_origin: ICoord2D,
    pub first_zone: ZoneStorage,
    pub num_zones: u16,
    pub zones_allocated: u16,
    /// The four tables (upstream's four arrays, allocated and freed together).
    pub tables: Option<BlockTables>,
    pub interacts_with_bridge: bool,
    pub marked_passable: bool,
}

impl Default for ZoneBlock {
    /// `ZoneBlock()`: no zones, no tables, passable.
    fn default() -> Self {
        ZoneBlock {
            cell_origin: ICoord2D { x: 0, y: 0 },
            first_zone: 0,
            num_zones: 0,
            zones_allocated: 0,
            tables: None,
            interacts_with_bridge: false,
            marked_passable: true,
        }
    }
}

impl ZoneBlock {
    /// `blockCalculateZones(map, layers, bounds)`: the block's zone range, its tables (none for one zone), and the equivalences of
    /// neighbouring cells within it.
    pub fn block_calculate_zones(&mut self, s: &CellStore, map: &GroundMap, bounds: &IRegion2D) {
        self.cell_origin = bounds.lo;
        let mut min_zone = u32::from(s.cells[map.cell(bounds.lo.x, bounds.lo.y) as usize].get_zone());
        let mut max_zone = min_zone;
        for j in bounds.lo.y..=bounds.hi.y {
            for i in bounds.lo.x..=bounds.hi.x {
                let zone = u32::from(s.cells[map.cell(i, j) as usize].get_zone());
                min_zone = min_zone.min(zone);
                max_zone = max_zone.max(zone);
            }
        }
        self.first_zone = min_zone as ZoneStorage;
        self.num_zones = (1 + max_zone - min_zone) as u16;
        self.allocate_zones();
        if self.num_zones == 1 {
            return; // "all zones are equivalent."
        }
        let first = i32::from(self.first_zone);
        let n = i32::from(self.num_zones);
        let t = self.tables.as_mut().expect("allocated for more than one zone");
        for i in 0..self.zones_allocated as usize {
            let z = (i as i32 + first) as ZoneStorage;
            t.ground_cliff[i] = z;
            t.ground_water[i] = z;
            t.ground_rubble[i] = z;
            t.crusher[i] = z;
        }
        for j in bounds.lo.y..=bounds.hi.y {
            for i in bounds.lo.x..=bounds.hi.x {
                let me = map.cell(i, j);
                for (cond, other) in [(i > bounds.lo.x, (i - 1, j)), (j > bounds.lo.y, (i, j - 1))] {
                    if !cond {
                        continue;
                    }
                    let o = map.cell(other.0, other.1);
                    if s.cells[me as usize].get_zone() == s.cells[o as usize].get_zone() {
                        continue;
                    }
                    if water_ground(s, me, o) {
                        apply_block_zone(s, me, o, &mut t.ground_water, first, n);
                    }
                    if ground_rubble(s, me, o) {
                        apply_block_zone(s, me, o, &mut t.ground_rubble, first, n);
                    }
                    if ground_cliff(s, me, o) && !plant(3031) {
                        apply_block_zone(s, me, o, &mut t.ground_cliff, first, n);
                    }
                    if crusher_ground(s, me, o) {
                        apply_block_zone(s, me, o, &mut t.crusher, first, n);
                    }
                }
            }
        }
    }

    /// `getEffectiveZone(acceptableSurfaces, crusher, zone)`: the zone through the tables the surfaces call for (1 for the air, and for
    /// ground, water and cliff together; the first zone for a block of one zone or a zone out of its range).
    pub fn get_effective_zone(&self, surfaces: LocomotorSurfaceTypeMask, crusher: bool, zone: ZoneStorage) -> ZoneStorage {
        if zone == UNINITIALIZED_ZONE {
            return zone;
        }
        if surfaces & LOCOMOTORSURFACE_AIR != 0 {
            return 1; // "air is all zone 1."
        }
        let has = |m: LocomotorSurfaceTypeMask| surfaces & m != 0;
        if has(LOCOMOTORSURFACE_GROUND) && has(LOCOMOTORSURFACE_WATER) && has(LOCOMOTORSURFACE_CLIFF) {
            return 1;
        }
        if self.num_zones < 2 {
            return self.first_zone; // "if we only got 1 zone, it's all the same zone."
        }
        if zone < self.first_zone || u32::from(zone) >= u32::from(self.first_zone) + u32::from(self.num_zones) {
            return self.first_zone;
        }
        let t = self.tables.as_ref().expect("a block of more than one zone has its tables");
        let mut zone = zone - self.first_zone;
        if crusher && !plant(3039) {
            zone = t.crusher[zone as usize].wrapping_sub(self.first_zone);
        }
        if has(LOCOMOTORSURFACE_GROUND) && has(LOCOMOTORSURFACE_CLIFF) {
            return t.ground_cliff[zone as usize];
        }
        if has(LOCOMOTORSURFACE_GROUND) && has(LOCOMOTORSURFACE_WATER) {
            return t.ground_water[zone as usize];
        }
        if has(LOCOMOTORSURFACE_GROUND) && has(LOCOMOTORSURFACE_RUBBLE) {
            return t.ground_rubble[zone as usize];
        }
        zone.wrapping_add(self.first_zone)
    }

    /// `allocateZones()`: keep tables large enough, else free them and, for more than one zone, allocate a power of two above the count
    /// (from 4, or the size last allocated).
    fn allocate_zones(&mut self) {
        if self.zones_allocated > self.num_zones && self.tables.is_some() {
            return;
        }
        self.tables = None; // freeZones (m_zonesAllocated kept)
        if self.num_zones == 1 {
            return; // "we don't need any zone equivalency tables."
        }
        if self.zones_allocated == 0 {
            self.zones_allocated = 4;
        }
        while self.zones_allocated <= self.num_zones {
            self.zones_allocated = self.zones_allocated.wrapping_mul(2);
        }
        let n = self.zones_allocated as usize;
        self.tables = Some(BlockTables { ground_cliff: vec![0; n], ground_water: vec![0; n], ground_rubble: vec![0; n], crusher: vec![0; n] });
    }

    /// `clearMarkedPassable()`, `isPassable()`, `setPassable(pass)`, `getInteractsWithBridge()`, `setInteractsWithBridge(interacts)`.
    pub fn clear_marked_passable(&mut self) {
        self.marked_passable = false;
    }
    pub fn is_passable(&self) -> bool {
        self.marked_passable
    }
    pub fn set_passable(&mut self, pass: bool) {
        self.marked_passable = pass;
    }
    pub fn get_interacts_with_bridge(&self) -> bool {
        self.interacts_with_bridge
    }
    pub fn set_interacts_with_bridge(&mut self, interacts: bool) {
        self.interacts_with_bridge = interacts;
    }
}

// ---- PathfindZoneManager ------------------------------------------------------------------------------------------------------

/// The manager's six tables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZoneTables {
    pub ground_cliff: Vec<ZoneStorage>,
    pub ground_water: Vec<ZoneStorage>,
    pub ground_rubble: Vec<ZoneStorage>,
    pub terrain: Vec<ZoneStorage>,
    pub crusher: Vec<ZoneStorage>,
    pub hierarchical: Vec<ZoneStorage>,
}

/// `PathfindZoneManager`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathfindZoneManager {
    /// `m_blockOfZoneBlocks` as a matrix (`m_zoneBlocks[x][y]` is `blocks[x * extent.y + y]`); none while upstream's is `NULL`.
    pub blocks: Option<Vec<ZoneBlock>>,
    pub zone_block_extent: ICoord2D,
    pub max_zone: u16,
    pub next_frame_to_calculate_zones: u32,
    pub zones_allocated: u16,
    pub tables: Option<ZoneTables>,
}

impl PathfindZoneManager {
    /// `PathfindZoneManager()`.
    pub fn new() -> Self {
        Self::default()
    }

    fn block_mut(&mut self, x: i32, y: i32) -> &mut ZoneBlock {
        let ey = self.zone_block_extent.y;
        &mut self.blocks.as_mut().expect("the blocks are allocated")[(x * ey + y) as usize]
    }
    fn block(&self, x: i32, y: i32) -> &ZoneBlock {
        &self.blocks.as_ref().expect("the blocks are allocated")[(x * self.zone_block_extent.y + y) as usize]
    }
    /// The block of cell `(x, y)`, or none outside the blocks (`DEBUG_CRASH("Invalid block.")` where upstream asserts).
    fn block_of(&self, cell_x: i32, cell_y: i32) -> Option<(i32, i32)> {
        let (bx, by) = (cell_x / ZONE_BLOCK_SIZE, cell_y / ZONE_BLOCK_SIZE);
        (bx >= 0 && bx < self.zone_block_extent.x && by >= 0 && by < self.zone_block_extent.y).then_some((bx, by))
    }

    /// `freeZones()`: no tables, and nothing allocated.
    fn free_zones(&mut self) {
        self.tables = None;
        self.zones_allocated = 0;
    }
    /// `freeBlocks()`.
    fn free_blocks(&mut self) {
        self.blocks = None;
        self.zone_block_extent = ICoord2D { x: 0, y: 0 };
    }
    /// `allocateZones()`: keep tables larger than the zone count, else allocate a power of two times `INITIAL_ZONES` above it.
    fn allocate_zones(&mut self) {
        if self.zones_allocated > self.max_zone && self.tables.is_some() {
            return;
        }
        self.free_zones();
        if self.zones_allocated == 0 {
            self.zones_allocated = INITIAL_ZONES;
        }
        while self.zones_allocated <= self.max_zone {
            self.zones_allocated = self.zones_allocated.wrapping_mul(2);
        }
        let n = self.zones_allocated as usize;
        self.tables = Some(ZoneTables {
            ground_cliff: vec![0; n],
            ground_water: vec![0; n],
            ground_rubble: vec![0; n],
            terrain: vec![0; n],
            crusher: vec![0; n],
            hierarchical: vec![0; n],
        });
    }

    /// `allocateBlocks(globalBounds)`: a block per 10 by 10 cells (rounded up), each new.
    pub fn allocate_blocks(&mut self, global_bounds: &IRegion2D) {
        self.free_blocks();
        let ex = (global_bounds.hi.x - global_bounds.lo.x + 1 + ZONE_BLOCK_SIZE - 1) / ZONE_BLOCK_SIZE;
        let ey = (global_bounds.hi.y - global_bounds.lo.y + 1 + ZONE_BLOCK_SIZE - 1) / ZONE_BLOCK_SIZE;
        self.zone_block_extent = ICoord2D { x: ex, y: ey };
        self.blocks = Some(vec![ZoneBlock::default(); (ex * ey).max(0) as usize]);
    }

    /// `reset()`: "Called when the map is reset."
    pub fn reset(&mut self) {
        self.free_zones();
        self.free_blocks();
    }

    /// `needToCalculateZones()` at `frame`.
    pub fn need_to_calculate_zones(&self, frame: u32) -> bool {
        self.next_frame_to_calculate_zones <= frame
    }

    /// `markZonesDirty(insert)` at `frame`: frame 2 before frame 2, else no later than `ZONE_UPDATE_FREQUENCY` frames on (insert is
    /// ignored; upstream's use of it is commented out).
    pub fn mark_zones_dirty(&mut self, _insert: bool, frame: u32) {
        if frame < if plant(3041) { 1 } else { 2 } {
            self.next_frame_to_calculate_zones = 2;
            return;
        }
        self.next_frame_to_calculate_zones = self.next_frame_to_calculate_zones.min(frame.wrapping_add(ZONE_UPDATE_FREQUENCY));
    }

    /// `getExtent(extent)`.
    pub fn get_extent(&self) -> ICoord2D {
        self.zone_block_extent
    }

    /// The bounds of block `(x_block, y_block)` within `global` (inclusive).
    fn block_bounds(global: &IRegion2D, x_block: i32, y_block: i32) -> IRegion2D {
        let lo = ICoord2D { x: global.lo.x + x_block * ZONE_BLOCK_SIZE, y: global.lo.y + y_block * ZONE_BLOCK_SIZE };
        let hi = ICoord2D { x: (lo.x + ZONE_BLOCK_SIZE - 1).min(global.hi.x), y: (lo.y + ZONE_BLOCK_SIZE - 1).min(global.hi.y) };
        IRegion2D { lo, hi }
    }

    /// `calculateZones(map, layers, globalBounds)`: number the map's zones block by block, collapse the numbers, give each layer its
    /// zone, then the blocks' tables and the manager's (the blocks must be allocated for the bounds).
    pub fn calculate_zones(&mut self, s: &mut CellStore, map: &GroundMap, layers: &mut [PathfindLayerState], global: &IRegion2D) {
        self.max_zone = 1; // "we start using zone 0 as a flag."
        let mut eq: Vec<ZoneStorage> = (0..MAX_ZONES).map(|i| i as ZoneStorage).collect();
        for layer in layers.iter_mut() {
            layer.set_zone(0);
        }
        let x_count = (global.hi.x - global.lo.x + 1 + ZONE_BLOCK_SIZE - 1) / ZONE_BLOCK_SIZE;
        let y_count = (global.hi.y - global.lo.y + 1 + ZONE_BLOCK_SIZE - 1) / ZONE_BLOCK_SIZE;
        for x_block in 0..x_count {
            for y_block in 0..y_count {
                let bounds = Self::block_bounds(global, x_block, y_block);
                self.block_mut(x_block, y_block).set_interacts_with_bridge(false);
                for j in bounds.lo.y..=bounds.hi.y {
                    for i in bounds.lo.x..=bounds.hi.x {
                        let cell = map.cell(i, j);
                        s.cells[cell as usize].set_zone(0);
                        let size = i32::from(self.max_zone);
                        if i > bounds.lo.x {
                            let left = map.cell(i - 1, j);
                            if ty(s, cell) == ty(s, left) {
                                apply_zone(s, cell, left, &mut eq, size);
                            }
                        }
                        if j > bounds.lo.y {
                            let top = map.cell(i, j - 1);
                            if ty(s, cell) == ty(s, top) {
                                apply_zone(s, cell, top, &mut eq, size);
                            }
                        }
                        if s.cells[cell as usize].get_zone() == 0 {
                            s.cells[cell as usize].set_zone(self.max_zone);
                            self.max_zone = self.max_zone.wrapping_add(1);
                        }
                        if s.cells[cell as usize].get_connect_layer() > LAYER_GROUND {
                            self.block_mut(x_block, y_block).set_interacts_with_bridge(true);
                        }
                    }
                }
            }
        }
        let total_zones = i32::from(self.max_zone);
        // "Collapse the zones into a 1,2,3... sequence, removing collapsed zones." (upstream's other entries are uninitialised and
        // never read: a zone's representative is no greater than itself)
        self.max_zone = 1;
        let mut collapsed = vec![0i32; MAX_ZONES];
        for i in 1..total_zones.max(1) as usize {
            let zone = eq[i] as usize;
            if zone == i {
                collapsed[i] = i32::from(self.max_zone);
                self.max_zone = self.max_zone.wrapping_add(1);
            } else {
                collapsed[i] = collapsed[zone];
            }
        }
        // "Now map the zones in the map back into the collapsed zones."
        for j in global.lo.y..=global.hi.y {
            for i in global.lo.x..=global.hi.x {
                let c = &mut s.cells[map.cell(i, j) as usize];
                c.set_zone(collapsed[c.get_zone() as usize] as ZoneStorage);
            }
        }
        for layer in layers.iter_mut() {
            let mut zone = collapsed[layer.get_zone() as usize];
            if zone == 0 {
                zone = i32::from(self.max_zone);
                self.max_zone = self.max_zone.wrapping_add(1);
            }
            layer.set_zone(zone);
            layer.apply_zone(s);
            if !layer.is_unused() && !layer.is_destroyed() {
                let start = layer.get_start_cell_index();
                self.set_bridge(start.x, start.y, true);
                let end = layer.get_end_cell_index();
                self.set_bridge(end.x, end.y, true);
            }
        }
        self.allocate_zones();
        for x_block in 0..x_count {
            for y_block in 0..y_count {
                let bounds = Self::block_bounds(global, x_block, y_block);
                self.block_mut(x_block, y_block).block_calculate_zones(s, map, &bounds);
            }
        }
        let max_zone = i32::from(self.max_zone);
        let t = self.tables.as_mut().expect("just allocated");
        for i in 0..self.zones_allocated as usize {
            let z = i as ZoneStorage;
            t.ground_cliff[i] = z;
            t.ground_water[i] = z;
            t.ground_rubble[i] = z;
            t.terrain[i] = z;
            t.crusher[i] = z;
            t.hierarchical[i] = z;
        }
        for j in global.lo.y..=global.hi.y {
            for i in global.lo.x..=global.hi.x {
                let me = map.cell(i, j);
                let connect = s.cells[me as usize].get_connect_layer();
                if connect > LAYER_GROUND && ty(s, me) == CELL_CLEAR && !plant(3032) {
                    let layer_zone = layers[connect as usize].get_zone();
                    resolve_zones(i32::from(s.cells[me as usize].get_zone()), layer_zone, &mut t.hierarchical, max_zone);
                }
                if i > global.lo.x && s.cells[me as usize].get_zone() != s.cells[map.cell(i - 1, j) as usize].get_zone() {
                    let left = map.cell(i - 1, j);
                    if ty(s, me) == ty(s, left) {
                        apply_zone(s, me, left, &mut t.hierarchical, max_zone); // "if this is true, skip all the ones below"
                    } else {
                        let mut not_terrain_or_crusher = true;
                        if terrain(s, me, left) {
                            apply_zone(s, me, left, &mut t.terrain, max_zone);
                            not_terrain_or_crusher = false;
                        }
                        if crusher_ground(s, me, left) {
                            apply_zone(s, me, left, &mut t.crusher, max_zone);
                            not_terrain_or_crusher = false;
                        }
                        if not_terrain_or_crusher {
                            if water_ground(s, me, left) {
                                apply_zone(s, me, left, &mut t.ground_water, max_zone);
                            } else if ground_rubble(s, me, left) {
                                apply_zone(s, me, left, &mut t.ground_rubble, max_zone);
                            } else if ground_cliff(s, me, left) {
                                apply_zone(s, me, left, &mut t.ground_cliff, max_zone);
                            }
                        }
                    }
                }
                if j > global.lo.y && s.cells[me as usize].get_zone() != s.cells[map.cell(i, j - 1) as usize].get_zone() {
                    let top = map.cell(i, j - 1);
                    if ty(s, me) == ty(s, top) {
                        apply_zone(s, me, top, &mut t.hierarchical, max_zone);
                    } else {
                        // (the top neighbour's ladder runs whatever the terrain and crusher tables took)
                        if terrain(s, me, top) {
                            apply_zone(s, me, top, &mut t.terrain, max_zone);
                        }
                        if crusher_ground(s, me, top) {
                            apply_zone(s, me, top, &mut t.crusher, max_zone);
                        }
                        if water_ground(s, me, top) {
                            apply_zone(s, me, top, &mut t.ground_water, max_zone);
                        } else if ground_rubble(s, me, top) {
                            apply_zone(s, me, top, &mut t.ground_rubble, max_zone);
                        } else if ground_cliff(s, me, top) {
                            apply_zone(s, me, top, &mut t.ground_cliff, max_zone);
                        }
                    }
                }
            }
        }
        // "FLATTEN HIERARCHICAL ZONES"
        for i in 1..max_zone.max(1) as usize {
            let zone = t.hierarchical[i] as usize;
            t.hierarchical[i] = t.hierarchical[zone];
        }
        let size = i32::from(self.max_zone);
        flatten_zones(&mut t.ground_cliff, &t.hierarchical, size);
        flatten_zones(&mut t.ground_water, &t.hierarchical, size);
        flatten_zones(&mut t.ground_rubble, &t.hierarchical, size);
        if !plant(3033) {
            flatten_zones(&mut t.terrain, &t.hierarchical, size);
        }
        flatten_zones(&mut t.crusher, &t.hierarchical, size);
        self.next_frame_to_calculate_zones = 0xffff_ffff;
    }

    /// `updateZonesForModify(map, layers, structureBounds, globalBounds)`: within the structure's bounds (one more cell up and right,
    /// clipped to the map), each block's unzoned cells take a zone from a neighbour of their type, in a forward then a backward sweep.
    pub fn update_zones_for_modify(&mut self, s: &mut CellStore, map: &GroundMap, structure: &IRegion2D, global: &IRegion2D) {
        let mut bounds = *structure;
        bounds.hi.x = (bounds.hi.x + 1).min(global.hi.x);
        bounds.hi.y = (bounds.hi.y + 1).min(global.hi.y);
        let zone = |s: &CellStore, i: i32, j: i32| s.cells[map.cell(i, j) as usize].get_zone();
        for x_block in 0..self.zone_block_extent.x {
            for y_block in 0..self.zone_block_extent.y {
                // the block's bounds, clipped to the structure's
                let (lo_x, lo_y) = (global.lo.x + x_block * ZONE_BLOCK_SIZE, global.lo.y + y_block * ZONE_BLOCK_SIZE);
                let bb = IRegion2D {
                    lo: ICoord2D { x: lo_x.max(bounds.lo.x), y: lo_y.max(bounds.lo.y) },
                    hi: ICoord2D { x: (lo_x + ZONE_BLOCK_SIZE - 1).min(bounds.hi.x), y: (lo_y + ZONE_BLOCK_SIZE - 1).min(bounds.hi.y) },
                };
                if bb.lo.x > bb.hi.x || bb.lo.y > bb.hi.y {
                    continue;
                }
                self.block_mut(x_block, y_block).set_interacts_with_bridge(false);
                for j in bb.lo.y..=bb.hi.y {
                    for i in bb.lo.x..=bb.hi.x {
                        let cell = map.cell(i, j);
                        if zone(s, i, j) != UNINITIALIZED_ZONE {
                            continue;
                        }
                        if i > bb.lo.x && ty(s, cell) == ty(s, map.cell(i - 1, j)) {
                            let z = zone(s, i - 1, j);
                            s.cells[cell as usize].set_zone(z);
                            if zone(s, i, j) != UNINITIALIZED_ZONE {
                                continue;
                            }
                        }
                        if j > bb.lo.y {
                            if ty(s, cell) == ty(s, map.cell(i, j - 1)) {
                                let z = zone(s, i, j - 1);
                                s.cells[cell as usize].set_zone(z);
                                if zone(s, i, j) != UNINITIALIZED_ZONE {
                                    continue;
                                }
                            }
                            if i < bb.hi.x && types_match(s, cell, map.cell(i + 1, j - 1)) && types_match(s, cell, map.cell(i + 1, j)) {
                                let z = zone(s, i + 1, j - 1);
                                s.cells[cell as usize].set_zone(z);
                            }
                        }
                    }
                }
                for j in (bb.lo.y..=bb.hi.y).rev() {
                    for i in (bb.lo.x..=bb.hi.x).rev() {
                        let cell = map.cell(i, j);
                        if zone(s, i, j) != UNINITIALIZED_ZONE {
                            continue;
                        }
                        if i < bb.hi.x && ty(s, cell) == ty(s, map.cell(i + 1, j)) {
                            let z = zone(s, i + 1, j);
                            s.cells[cell as usize].set_zone(z);
                            if zone(s, i, j) != UNINITIALIZED_ZONE {
                                continue;
                            }
                        }
                        if j < bb.hi.y {
                            if ty(s, cell) == ty(s, map.cell(i, j + 1)) {
                                let z = zone(s, i, j + 1);
                                s.cells[cell as usize].set_zone(z);
                                if zone(s, i, j) != UNINITIALIZED_ZONE {
                                    continue;
                                }
                            }
                            if i < bb.hi.x && types_match(s, cell, map.cell(i + 1, j + 1)) && types_match(s, cell, map.cell(i + 1, j)) {
                                let z = zone(s, if plant(3034) { i } else { i + 1 }, j + 1);
                                s.cells[cell as usize].set_zone(z);
                            }
                        }
                    }
                }
            }
        }
    }

    /// `clearPassableFlags()`, `setAllPassable()`.
    pub fn clear_passable_flags(&mut self) {
        if let Some(b) = self.blocks.as_mut() {
            b.iter_mut().for_each(|b| b.set_passable(false));
        }
    }
    pub fn set_all_passable(&mut self) {
        if let Some(b) = self.blocks.as_mut() {
            b.iter_mut().for_each(|b| b.set_passable(true));
        }
    }
    /// `setPassable(cellX, cellY, passable)`: nothing outside the blocks.
    pub fn set_passable(&mut self, cell_x: i32, cell_y: i32, passable: bool) {
        if let Some((x, y)) = self.block_of(cell_x, cell_y) {
            self.block_mut(x, y).set_passable(passable);
        }
    }
    /// `isPassable(cellX, cellY)`, `clipIsPassable(cellX, cellY)`: false outside the blocks.
    pub fn is_passable(&self, cell_x: i32, cell_y: i32) -> bool {
        self.block_of(cell_x, cell_y).is_some_and(|(x, y)| self.block(x, y).is_passable())
    }
    pub fn clip_is_passable(&self, cell_x: i32, cell_y: i32) -> bool {
        self.is_passable(cell_x, cell_y)
    }
    /// `setBridge(cellX, cellY, bridge)`: nothing outside the blocks.
    pub fn set_bridge(&mut self, cell_x: i32, cell_y: i32, bridge: bool) {
        if let Some((x, y)) = self.block_of(cell_x, cell_y) {
            self.block_mut(x, y).set_interacts_with_bridge(bridge);
        }
    }
    /// `interactsWithBridge(cellX, cellY)`: false outside the blocks.
    pub fn interacts_with_bridge(&self, cell_x: i32, cell_y: i32) -> bool {
        self.block_of(cell_x, cell_y).is_some_and(|(x, y)| self.block(x, y).get_interacts_with_bridge())
    }

    /// `getBlockZone(acceptableSurfaces, crusher, cellX, cellY, map)`: the cell's zone in its block's tables; 0 outside the blocks and
    /// for a zone not below the manager's count.
    pub fn get_block_zone(&self, s: &CellStore, map: &GroundMap, surfaces: LocomotorSurfaceTypeMask, crusher: bool, cell_x: i32, cell_y: i32) -> ZoneStorage {
        let cell = map.cell(cell_x, cell_y);
        let Some((x, y)) = self.block_of(cell_x, cell_y) else { return 0 };
        let zone = self.block(x, y).get_effective_zone(surfaces, crusher, s.cells[cell as usize].get_zone());
        if zone >= self.max_zone {
            return UNINITIALIZED_ZONE; // "Invalid zone."
        }
        zone
    }

    /// `getEffectiveTerrainZone(zone)`.
    pub fn get_effective_terrain_zone(&self, zone: ZoneStorage) -> ZoneStorage {
        let t = self.tables.as_ref().expect("the zones are calculated");
        t.hierarchical[t.terrain[zone as usize] as usize]
    }

    /// `getEffectiveZone(acceptableSurfaces, crusher, zone)`: 0 above the zone count; 1 for the air, and for ground, water and cliff
    /// together; otherwise through the crusher table (for a crusher), then the table the surfaces call for, else the hierarchical one.
    pub fn get_effective_zone(&self, surfaces: LocomotorSurfaceTypeMask, crusher: bool, zone: ZoneStorage) -> ZoneStorage {
        if zone > self.max_zone {
            return 0; // "Invalid zone"
        }
        if surfaces & LOCOMOTORSURFACE_AIR != 0 {
            return 1; // "air is all zone 1."
        }
        let has = |m: LocomotorSurfaceTypeMask| surfaces & m != 0;
        if has(LOCOMOTORSURFACE_GROUND) && has(LOCOMOTORSURFACE_WATER) && has(LOCOMOTORSURFACE_CLIFF) {
            return 1;
        }
        let t = self.tables.as_ref().expect("the zones are calculated");
        let mut zone = zone;
        if crusher && !plant(3038) {
            zone = t.crusher[zone as usize];
        }
        if has(LOCOMOTORSURFACE_GROUND) && has(LOCOMOTORSURFACE_CLIFF) {
            return t.ground_cliff[zone as usize];
        }
        if has(LOCOMOTORSURFACE_GROUND) && has(LOCOMOTORSURFACE_WATER) {
            return t.ground_water[zone as usize];
        }
        if has(LOCOMOTORSURFACE_GROUND) && has(LOCOMOTORSURFACE_RUBBLE) {
            return t.ground_rubble[zone as usize];
        }
        t.hierarchical[zone as usize]
    }
}
