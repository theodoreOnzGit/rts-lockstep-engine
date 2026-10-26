// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h (PathfindCellInfo, PathfindCell)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (lines 1101-1773: PathfindCellInfo, PathfindCell, the costs)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PathfindCellInfo` and `PathfindCell`: the cells of the pathfinding grid, and the pool of the records a cell borrows while a search,
//! a unit, an aircraft or an obstacle needs more than its few bits. `p1w13-sonnet`, layer 10, batch P2.
//!
//! # The shape
//!
//! Upstream's cells and infos point at each other; here they are the arenas of a [`CellStore`], linked by index ([`CellId`],
//! [`InfoId`]): the cells (the grid's and, later, the bridge layers'), and the pool of `CELL_INFOS_TO_ALLOCATE` infos (upstream's
//! `s_infoArray`, a static, is the store's: there is one pathfinder). The free infos are a list through their `pathParent`, as
//! upstream's; a released info keeps its cell and fields, so a stale link reads what upstream reads.
//!
//! # Upstream facts kept
//!
//! - The bit fields: a zone keeps 14 bits; a type, flags and layers 4; the info's costs are `UnsignedShort`s.
//! - An info is taken from the head of the free list and returned to it (last in, first out); none left is `NULL` (and a goal or a
//!   position is then not recorded: "Ran out of pathfind cells").
//! - `releaseInfo` keeps the info of an obstacle, of a cell with units or an aircraft goal, and of one still on a list ("better leak
//!   than crash"). `reset` releases it whatever it is linked to.
//! - The open list is sorted by total cost, a new cell after the cells of equal cost; the closed list is a stack. Both are linked
//!   through the infos' `nextOpen` and `prevOpen`.
//! - `costToGoal` is the octile estimate in integers (10 per orthogonal cell, half of 10 per diagonal one); `costToHierGoal` rounds
//!   10 times the `double` distance, and is 100000 for a cell without an info ("patch hack 1.01"); `costSoFar` adds 10 or 14 per step,
//!   14 more into a pinched cell, and 4, 8 or 16 for a turn of 45, 90 or 135 degrees.
//!
//! # Where upstream is undefined
//!
//! Most members dereference the cell's info unchecked (`startPathfind`, the parents, the lists, `costToGoal`, `costSoFar`, the getters
//! of the search); here they expect one, as callers that hold the invariant do (the panic names it).

use crate::logic::modules::plant;
use crate::common::base_type::{real_to_int_floor, ICoord2D};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND, LAYER_INVALID};

/// An index into the store's cells.
pub type CellId = u32;
/// An index into the store's info pool.
pub type InfoId = u32;

/// `CELL_INFOS_TO_ALLOCATE`.
pub const CELL_INFOS_TO_ALLOCATE: usize = 30000;
/// `PATHFIND_CELLS_PER_FRAME`: "Number of cells we will search pathfinding per frame."
pub const PATHFIND_CELLS_PER_FRAME: i32 = 5000;

/// `PathfindCell::CellType`.
pub const CELL_CLEAR: u8 = 0x00;
pub const CELL_WATER: u8 = 0x01;
pub const CELL_CLIFF: u8 = 0x02;
pub const CELL_RUBBLE: u8 = 0x03;
pub const CELL_OBSTACLE: u8 = 0x04;
pub const CELL_BRIDGE_IMPASSABLE: u8 = 0x05;
pub const CELL_IMPASSABLE: u8 = 0x06;
/// `PathfindCell::CellFlags`.
pub const NO_UNITS: u8 = 0x00;
pub const UNIT_GOAL: u8 = 0x01;
pub const UNIT_PRESENT_MOVING: u8 = 0x02;
pub const UNIT_PRESENT_FIXED: u8 = 0x03;
pub const UNIT_GOAL_OTHER_MOVING: u8 = 0x05;

/// `COST_ORTHOGONAL`, `COST_DIAGONAL`.
pub const COST_ORTHOGONAL: i32 = 10;
pub const COST_DIAGONAL: i32 = 14;
/// `COST_TO_DISTANCE_FACTOR`, `COST_TO_DISTANCE_FACTOR_SQR`.
pub const COST_TO_DISTANCE_FACTOR: f32 = 1.0 / 10.0;
pub const COST_TO_DISTANCE_FACTOR_SQR: f32 = COST_TO_DISTANCE_FACTOR * COST_TO_DISTANCE_FACTOR;

/// `class PathfindCellInfo`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathfindCellInfo {
    /// `m_nextOpen`, `m_prevOpen`: "for A* 'open' list, shared by closed list".
    pub next_open: Option<InfoId>,
    pub prev_open: Option<InfoId>,
    /// `m_pathParent`: the "parent" cell's info (the next free info while free).
    pub path_parent: Option<InfoId>,
    /// `m_cell`: "Cell this info belongs to currently".
    pub cell: Option<CellId>,
    /// `m_totalCost`, `m_costSoFar`: "cost estimates for A* search".
    pub total_cost: u16,
    pub cost_so_far: u16,
    /// `m_pos`: the cell's coordinates.
    pub pos: ICoord2D,
    pub goal_unit_id: ObjectId,
    pub pos_unit_id: ObjectId,
    pub goal_aircraft_id: ObjectId,
    pub obstacle_id: ObjectId,
    pub is_free: bool,
    pub blocked_by_ally: bool,
    pub obstacle_is_fence: bool,
    pub obstacle_is_transparent: bool,
    pub open: bool,
    pub closed: bool,
}

impl Default for PathfindCellInfo {
    /// `new PathfindCellInfo[]`: the zeros of a fresh allocation (the debug heap's fill aside; every field is set before it is read).
    fn default() -> Self {
        PathfindCellInfo {
            next_open: None,
            prev_open: None,
            path_parent: None,
            cell: None,
            total_cost: 0,
            cost_so_far: 0,
            pos: ICoord2D { x: 0, y: 0 },
            goal_unit_id: INVALID_ID,
            pos_unit_id: INVALID_ID,
            goal_aircraft_id: INVALID_ID,
            obstacle_id: INVALID_ID,
            is_free: false,
            blocked_by_ally: false,
            obstacle_is_fence: false,
            obstacle_is_transparent: false,
            open: false,
            closed: false,
        }
    }
}

/// `class PathfindCell`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathfindCell {
    /// `m_info`.
    pub info: Option<InfoId>,
    /// `m_zone:14`.
    zone: u16,
    /// `m_aircraftGoal:1`, `m_pinched:1`.
    aircraft_goal: bool,
    pinched: bool,
    /// `m_type:4`, `m_flags:4`, `m_connectsToLayer:4`, `m_layer:4`.
    cell_type: u8,
    flags: u8,
    connects_to_layer: u8,
    layer: u8,
}

impl Default for PathfindCell {
    /// `PathfindCell()`: no info, then `reset()`.
    fn default() -> Self {
        PathfindCell {
            info: None,
            zone: 0,
            aircraft_goal: false,
            pinched: false,
            cell_type: CELL_CLEAR,
            flags: NO_UNITS,
            connects_to_layer: LAYER_INVALID as u8,
            layer: LAYER_GROUND as u8,
        }
    }
}

impl PathfindCell {
    /// `getType()`.
    pub fn get_type(&self) -> u8 {
        self.cell_type
    }
    /// `getFlags()`.
    pub fn get_flags(&self) -> u8 {
        self.flags
    }
    /// `isAircraftGoal()`.
    pub fn is_aircraft_goal(&self) -> bool {
        self.aircraft_goal
    }
    /// `getPinched()`, `setPinched()`.
    pub fn get_pinched(&self) -> bool {
        self.pinched
    }
    pub fn set_pinched(&mut self, pinch: bool) {
        self.pinched = pinch;
    }
    /// `hasInfo()`.
    pub fn has_info(&self) -> bool {
        self.info.is_some()
    }
    /// `getZone()`, `setZone(zone)` (14 bits).
    pub fn get_zone(&self) -> u16 {
        self.zone
    }
    pub fn set_zone(&mut self, zone: u16) {
        self.zone = zone & if plant(3019) { 0x7fff } else { 0x3fff };
    }
    /// `setLayer(layer)`, `getLayer()` (4 bits).
    pub fn set_layer(&mut self, layer: PathfindLayer) {
        self.layer = (layer as u8) & 15;
    }
    pub fn get_layer(&self) -> PathfindLayer {
        PathfindLayer::from(self.layer)
    }
    /// `setConnectLayer(layer)`, `getConnectLayer()` (4 bits).
    pub fn set_connect_layer(&mut self, layer: PathfindLayer) {
        self.connects_to_layer = (layer as u8) & 15;
    }
    pub fn get_connect_layer(&self) -> PathfindLayer {
        PathfindLayer::from(self.connects_to_layer)
    }
}

/// What `setTypeAsObstacle` reads of the obstacle: its id, whether its body is rubble (`getBodyModule() && getDamageState() ==
/// BODY_RUBBLE`), and whether it is `KINDOF_CAN_SEE_THROUGH_STRUCTURE`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObstacleFacts {
    pub id: ObjectId,
    pub rubble: bool,
    pub see_through: bool,
}

/// The cells and the pool of their infos.
#[derive(Clone, Debug, PartialEq)]
pub struct CellStore {
    pub cells: Vec<PathfindCell>,
    /// `s_infoArray` (empty before `allocateCellInfos`).
    infos: Vec<PathfindCellInfo>,
    /// `s_firstFree`.
    first_free: Option<InfoId>,
}

impl Default for CellStore {
    fn default() -> Self {
        CellStore::new()
    }
}

impl CellStore {
    pub fn new() -> Self {
        CellStore { cells: Vec::new(), infos: Vec::new(), first_free: None }
    }

    /// The info `i`.
    pub fn info(&self, i: InfoId) -> &PathfindCellInfo {
        &self.infos[i as usize]
    }
    /// `s_firstFree`.
    pub fn first_free(&self) -> Option<InfoId> {
        self.first_free
    }
    /// The cell's info, which the caller's member dereferences unchecked.
    fn info_of(&self, c: CellId) -> InfoId {
        self.cells[c as usize].info.expect("upstream dereferences the cell's info unchecked")
    }
    fn inf(&mut self, i: InfoId) -> &mut PathfindCellInfo {
        &mut self.infos[i as usize]
    }

    // ---- PathfindCellInfo's pool --------------------------------------------------------------------------------------
    /// `allocateCellInfos()`: release the pool, then a new one of `CELL_INFOS_TO_ALLOCATE`, every info free, linked in order.
    pub fn allocate_cell_infos(&mut self) {
        self.release_cell_infos();
        self.infos = vec![PathfindCellInfo::default(); CELL_INFOS_TO_ALLOCATE];
        for i in 0..CELL_INFOS_TO_ALLOCATE {
            self.infos[i].path_parent = (i + 1 < CELL_INFOS_TO_ALLOCATE).then_some((i + 1) as InfoId);
            self.infos[i].is_free = true;
        }
        self.first_free = Some(0);
    }

    /// `releaseCellInfos()`: the pool is gone (nothing if there is none).
    pub fn release_cell_infos(&mut self) {
        if self.infos.is_empty() {
            return; // "haven't allocated any yet."
        }
        self.infos = Vec::new();
        self.first_free = None;
    }

    /// `getACellInfo(cell, pos)`: the first free info, made the cell's and cleared; `None` when none is left.
    pub fn get_a_cell_info(&mut self, cell: CellId, pos: ICoord2D) -> Option<InfoId> {
        let info = self.first_free?;
        let next = self.infos[info as usize].path_parent;
        self.first_free = next;
        *self.inf(info) = PathfindCellInfo { is_free: false, cell: Some(cell), pos, ..PathfindCellInfo::default() };
        Some(info)
    }

    /// `releaseACellInfo(info)`: back to the head of the free list (its other fields kept).
    pub fn release_a_cell_info(&mut self, info: InfoId) {
        let first = self.first_free;
        let i = self.inf(info);
        i.path_parent = first;
        i.is_free = true;
        self.first_free = Some(info);
    }

    // ---- PathfindCell -------------------------------------------------------------------------------------------------
    /// `reset()`: clear ground, no units, zone 0, no aircraft goal, not pinched, the info released (its obstacle forgotten) whatever it
    /// is linked to, no connecting layer, the ground layer.
    pub fn reset(&mut self, c: CellId) {
        let cell = &mut self.cells[c as usize];
        cell.cell_type = CELL_CLEAR;
        cell.flags = NO_UNITS;
        cell.zone = 0;
        cell.aircraft_goal = false;
        cell.pinched = false;
        if let Some(info) = cell.info.take() {
            self.inf(info).obstacle_id = INVALID_ID;
            self.release_a_cell_info(info);
        }
        let cell = &mut self.cells[c as usize];
        cell.connects_to_layer = LAYER_INVALID as u8;
        cell.layer = LAYER_GROUND as u8;
    }

    /// `getXIndex()`, `getYIndex()`.
    pub fn x_index(&self, c: CellId) -> u16 {
        self.infos[self.info_of(c) as usize].pos.x as u16
    }
    pub fn y_index(&self, c: CellId) -> u16 {
        self.infos[self.info_of(c) as usize].pos.y as u16
    }
    /// `getNextOpen()`: the next cell of the list the cell is on.
    pub fn get_next_open(&self, c: CellId) -> Option<CellId> {
        let n = self.infos[self.info_of(c) as usize].next_open?;
        self.infos[n as usize].cell
    }
    /// `getParentCell()`: the cell of the info's parent (none without an info).
    pub fn get_parent_cell(&self, c: CellId) -> Option<CellId> {
        let info = self.cells[c as usize].info?;
        let parent = self.infos[info as usize].path_parent?;
        self.infos[parent as usize].cell
    }
    /// `getCostSoFar()`, `getTotalCost()`, `getOpen()`, `getClosed()`, `isBlockedByAlly()`.
    pub fn get_cost_so_far(&self, c: CellId) -> u32 {
        u32::from(self.infos[self.info_of(c) as usize].cost_so_far)
    }
    pub fn get_total_cost(&self, c: CellId) -> u32 {
        u32::from(self.infos[self.info_of(c) as usize].total_cost)
    }
    pub fn get_open(&self, c: CellId) -> bool {
        self.infos[self.info_of(c) as usize].open
    }
    pub fn get_closed(&self, c: CellId) -> bool {
        self.infos[self.info_of(c) as usize].closed
    }
    pub fn is_blocked_by_ally(&self, c: CellId) -> bool {
        self.infos[self.info_of(c) as usize].blocked_by_ally
    }
    /// `setBlockedByAlly(blocked)`.
    pub fn set_blocked_by_ally(&mut self, c: CellId, blocked: bool) {
        let i = self.info_of(c);
        self.inf(i).blocked_by_ally = blocked;
    }
    /// `setCostSoFar(cost)`, `setTotalCost(cost)`: nothing without an info; the costs keep 16 bits.
    pub fn set_cost_so_far(&mut self, c: CellId, cost: u32) {
        if let Some(i) = self.cells[c as usize].info {
            self.inf(i).cost_so_far = cost as u16;
        }
    }
    pub fn set_total_cost(&mut self, c: CellId, cost: u32) {
        if let Some(i) = self.cells[c as usize].info {
            self.inf(i).total_cost = cost as u16;
        }
    }
    /// `getGoalUnit()`, `getGoalAircraft()`, `getPosUnit()`, `getObstacleID()`: `INVALID_ID` without an info.
    pub fn get_goal_unit(&self, c: CellId) -> ObjectId {
        self.cells[c as usize].info.map_or(INVALID_ID, |i| self.infos[i as usize].goal_unit_id)
    }
    pub fn get_goal_aircraft(&self, c: CellId) -> ObjectId {
        self.cells[c as usize].info.map_or(INVALID_ID, |i| self.infos[i as usize].goal_aircraft_id)
    }
    pub fn get_pos_unit(&self, c: CellId) -> ObjectId {
        self.cells[c as usize].info.map_or(INVALID_ID, |i| self.infos[i as usize].pos_unit_id)
    }
    pub fn get_obstacle_id(&self, c: CellId) -> ObjectId {
        self.cells[c as usize].info.map_or(INVALID_ID, |i| self.infos[i as usize].obstacle_id)
    }
    /// `isObstacleTransparent()`, `isObstacleFence()`: false without an info.
    pub fn is_obstacle_transparent(&self, c: CellId) -> bool {
        self.cells[c as usize].info.is_some_and(|i| self.infos[i as usize].obstacle_is_transparent)
    }
    pub fn is_obstacle_fence(&self, c: CellId) -> bool {
        self.cells[c as usize].info.is_some_and(|i| self.infos[i as usize].obstacle_is_fence)
    }
    /// `isObstaclePresent(objID)`: a valid id, an obstacle cell, and its info registering that id.
    pub fn is_obstacle_present(&self, c: CellId, obj_id: ObjectId) -> bool {
        obj_id != INVALID_ID
            && self.cells[c as usize].cell_type == CELL_OBSTACLE
            && self.cells[c as usize].info.is_some_and(|i| self.infos[i as usize].obstacle_id == obj_id)
    }

    /// `startPathfind(goalCell)`: no links, no parent, no cost so far, the estimate to the goal (0 without one), open and not closed.
    pub fn start_pathfind(&mut self, c: CellId, goal: Option<CellId>) -> bool {
        let i = self.info_of(c);
        {
            let info = self.inf(i);
            info.next_open = None;
            info.prev_open = None;
            info.path_parent = None;
            info.cost_so_far = 0; // "start node, no cost to get here"
            info.total_cost = 0;
        }
        if let Some(g) = goal {
            let cost = self.cost_to_goal(c, g);
            self.inf(i).total_cost = cost as u16;
        }
        let info = self.inf(i);
        info.open = true;
        info.closed = false;
        true
    }

    /// `setParentCell(parent)` and `setParentCellHierarchical(parent)`: the parent's info (the first only `DEBUG_CRASH`es on a parent
    /// that is not a neighbour).
    pub fn set_parent_cell(&mut self, c: CellId, parent: CellId) {
        let i = self.info_of(c);
        let p = self.info_of(parent);
        self.inf(i).path_parent = Some(p);
    }
    pub fn set_parent_cell_hierarchical(&mut self, c: CellId, parent: CellId) {
        self.set_parent_cell(c, parent);
    }
    /// `clearParentCell()`.
    pub fn clear_parent_cell(&mut self, c: CellId) {
        let i = self.info_of(c);
        self.inf(i).path_parent = None;
    }

    /// `allocateInfo(pos)`: an info if the cell has none; whether it has one.
    pub fn allocate_info(&mut self, c: CellId, pos: ICoord2D) -> bool {
        if self.cells[c as usize].info.is_none() {
            let info = self.get_a_cell_info(c, pos);
            self.cells[c as usize].info = info;
            return info.is_some();
        }
        true
    }

    /// `releaseInfo()`: the info back to the pool, unless the cell is an obstacle, has units or an aircraft goal, or the info is still
    /// on a list ("Bad release. Skip for now, better leak than crash. jba.").
    pub fn release_info(&mut self, c: CellId) {
        let cell = self.cells[c as usize];
        if cell.cell_type == CELL_OBSTACLE || cell.flags != NO_UNITS || (cell.aircraft_goal && !plant(3011)) {
            return;
        }
        if let Some(i) = cell.info {
            let info = self.infos[i as usize];
            if info.prev_open.is_some() || info.next_open.is_some() || info.open || info.closed {
                return;
            }
            self.release_a_cell_info(i);
            self.cells[c as usize].info = None;
        }
    }

    /// `setGoalUnit(unitID, pos)`.
    pub fn set_goal_unit(&mut self, c: CellId, unit_id: ObjectId, pos: ICoord2D) {
        if unit_id == INVALID_ID {
            // "removing goal."
            if let Some(i) = self.cells[c as usize].info {
                self.inf(i).goal_unit_id = INVALID_ID;
                if self.infos[i as usize].pos_unit_id == INVALID_ID {
                    // "No units here."
                    self.cells[c as usize].flags = NO_UNITS;
                    self.release_info(c);
                } else {
                    self.cells[c as usize].flags = UNIT_PRESENT_MOVING;
                }
            }
        } else {
            // "adding goal."
            if self.cells[c as usize].info.is_none() {
                self.allocate_info(c, pos);
            }
            let Some(i) = self.cells[c as usize].info else { return }; // "Ran out of pathfind cells - fatal error!!!!! jba."
            self.inf(i).goal_unit_id = unit_id;
            let pos_unit = self.infos[i as usize].pos_unit_id;
            self.cells[c as usize].flags = if unit_id == pos_unit && !plant(3012) {
                UNIT_PRESENT_FIXED
            } else if pos_unit == INVALID_ID {
                UNIT_GOAL
            } else {
                UNIT_GOAL_OTHER_MOVING
            };
        }
    }

    /// `setGoalAircraft(unitID, pos)`.
    pub fn set_goal_aircraft(&mut self, c: CellId, unit_id: ObjectId, pos: ICoord2D) {
        if unit_id == INVALID_ID {
            if let Some(i) = self.cells[c as usize].info {
                self.inf(i).goal_aircraft_id = INVALID_ID;
                self.cells[c as usize].aircraft_goal = false;
                self.release_info(c);
            }
        } else {
            if self.cells[c as usize].info.is_none() {
                self.allocate_info(c, pos);
            }
            let Some(i) = self.cells[c as usize].info else { return };
            self.inf(i).goal_aircraft_id = unit_id;
            self.cells[c as usize].aircraft_goal = true;
        }
    }

    /// `setPosUnit(unitID, pos)`.
    pub fn set_pos_unit(&mut self, c: CellId, unit_id: ObjectId, pos: ICoord2D) {
        if unit_id == INVALID_ID {
            // "removing position."
            if let Some(i) = self.cells[c as usize].info {
                self.inf(i).pos_unit_id = INVALID_ID;
                if self.infos[i as usize].goal_unit_id == INVALID_ID {
                    self.cells[c as usize].flags = NO_UNITS;
                    self.release_info(c);
                } else {
                    self.cells[c as usize].flags = UNIT_GOAL;
                }
            }
        } else {
            if self.cells[c as usize].info.is_none() {
                self.allocate_info(c, pos);
            }
            let Some(i) = self.cells[c as usize].info else { return };
            let info = self.infos[i as usize];
            if info.goal_unit_id != INVALID_ID && info.goal_unit_id == info.pos_unit_id {
                // "A unit is already occupying this cell."
                return;
            }
            self.inf(i).pos_unit_id = unit_id;
            self.cells[c as usize].flags = if unit_id == info.goal_unit_id {
                UNIT_PRESENT_FIXED
            } else if info.goal_unit_id == INVALID_ID {
                UNIT_PRESENT_MOVING
            } else {
                UNIT_GOAL_OTHER_MOVING
            };
        }
    }

    /// `setTypeAsObstacle(obstacle, isFence, pos)`: "flag this cell as an obstacle"; only a clear or impassable cell. A rubble obstacle
    /// makes it rubble (and forgets an obstacle the info held); otherwise an obstacle, with an info for its id, fence and transparency
    /// (false, nothing else changed but the type, when the pool is empty).
    pub fn set_type_as_obstacle(&mut self, c: CellId, obstacle: &ObstacleFacts, is_fence: bool, pos: ICoord2D) -> bool {
        let ty = self.cells[c as usize].cell_type;
        if ty != CELL_CLEAR && ty != CELL_IMPASSABLE {
            return false;
        }
        if obstacle.rubble {
            self.cells[c as usize].cell_type = CELL_RUBBLE;
            if let Some(i) = self.cells[c as usize].info {
                self.inf(i).obstacle_id = INVALID_ID;
                if !plant(3013) {
                    self.release_info(c);
                }
            }
            return true;
        }
        self.cells[c as usize].cell_type = CELL_OBSTACLE;
        if self.cells[c as usize].info.is_none() {
            let info = self.get_a_cell_info(c, pos);
            self.cells[c as usize].info = info;
            if info.is_none() {
                return false; // "Not enough PathFindCellInfos in pool."
            }
        }
        let i = self.cells[c as usize].info.expect("just made");
        let info = self.inf(i);
        info.obstacle_id = obstacle.id;
        info.obstacle_is_fence = is_fence;
        info.obstacle_is_transparent = obstacle.see_through;
        true
    }

    /// `setType(type)`: an obstacle stays one while its info holds an obstacle; the type keeps 4 bits.
    pub fn set_type(&mut self, c: CellId, ty: u8) {
        if let Some(i) = self.cells[c as usize].info {
            if self.infos[i as usize].obstacle_id != INVALID_ID {
                self.cells[c as usize].cell_type = CELL_OBSTACLE;
                return;
            }
        }
        self.cells[c as usize].cell_type = ty & if plant(3014) { 31 } else { 15 };
    }

    /// `removeObstacle(obstacle)`: rubble becomes clear; then, if the info holds this obstacle, clear and released. Whether it held it.
    pub fn remove_obstacle(&mut self, c: CellId, obstacle_id: ObjectId) -> bool {
        if self.cells[c as usize].cell_type == CELL_RUBBLE && !plant(3015) {
            self.cells[c as usize].cell_type = CELL_CLEAR;
        }
        let Some(i) = self.cells[c as usize].info else { return false };
        if self.infos[i as usize].obstacle_id != obstacle_id {
            return false;
        }
        self.cells[c as usize].cell_type = CELL_CLEAR;
        self.inf(i).obstacle_id = INVALID_ID;
        self.release_info(c);
        true
    }

    /// `putOnSortedOpenList(list)`: inserted before the first cell of a greater total cost; returns the new list.
    pub fn put_on_sorted_open_list(&mut self, c: CellId, list: Option<CellId>) -> Option<CellId> {
        let me = self.info_of(c);
        let mut list = list;
        match list {
            None => {
                list = Some(c);
                self.inf(me).prev_open = None;
                self.inf(me).next_open = None;
            }
            Some(head) => {
                // "insertion sort"
                let my_cost = self.infos[me as usize].total_cost;
                let mut cur = Some(head);
                let mut last_cell = None;
                while let Some(x) = cur {
                    if self.infos[self.info_of(x) as usize].total_cost > my_cost || (plant(3010) && self.infos[self.info_of(x) as usize].total_cost == my_cost) {
                        break;
                    }
                    last_cell = Some(x);
                    cur = self.get_next_open(x);
                }
                if let Some(x) = cur {
                    // "insert just before 'c'"
                    let xi = self.info_of(x);
                    match self.infos[xi as usize].prev_open {
                        Some(p) => self.inf(p).next_open = Some(me),
                        None => list = Some(c),
                    }
                    let prev = self.infos[xi as usize].prev_open;
                    self.inf(me).prev_open = prev;
                    self.inf(xi).prev_open = Some(me);
                    self.inf(me).next_open = Some(xi);
                } else {
                    // "append after 'lastCell' - end of list"
                    let li = self.info_of(last_cell.expect("a list has a last cell"));
                    self.inf(li).next_open = Some(me);
                    self.inf(me).prev_open = Some(li);
                    self.inf(me).next_open = None;
                }
            }
        }
        self.inf(me).open = true;
        self.inf(me).closed = false;
        list
    }

    /// `removeFromOpenList(list)`; returns the new list.
    pub fn remove_from_open_list(&mut self, c: CellId, list: Option<CellId>) -> Option<CellId> {
        let list = self.unlink(c, list);
        let me = self.info_of(c);
        let info = self.inf(me);
        info.open = false;
        info.next_open = None;
        info.prev_open = None;
        list
    }

    /// The unlinking both removals share: the neighbours joined; the head moves on when the cell had no previous.
    fn unlink(&mut self, c: CellId, list: Option<CellId>) -> Option<CellId> {
        let me = self.info_of(c);
        let (next, prev) = (self.infos[me as usize].next_open, self.infos[me as usize].prev_open);
        if let Some(n) = next {
            self.inf(n).prev_open = prev;
        }
        match prev {
            Some(p) => {
                self.inf(p).next_open = next;
                list
            }
            None => self.get_next_open(c),
        }
    }

    /// `releaseOpenList(list)`: every cell off the list (its info released if it may be); the count.
    pub fn release_open_list(&mut self, list: Option<CellId>) -> i32 {
        self.release_list(list, true)
    }

    /// `releaseClosedList(list)`.
    pub fn release_closed_list(&mut self, list: Option<CellId>) -> i32 {
        self.release_list(list, false)
    }

    fn release_list(&mut self, mut list: Option<CellId>, open: bool) -> i32 {
        let mut count = 0;
        while let Some(cur) = list {
            count += 1;
            let ci = self.info_of(cur);
            list = self.infos[ci as usize].next_open.and_then(|n| self.infos[n as usize].cell);
            let info = self.inf(ci);
            info.next_open = None;
            info.prev_open = None;
            if open {
                info.open = false;
            } else {
                info.closed = false;
            }
            self.release_info(cur);
        }
        count
    }

    /// `putOnClosedList(list)`: pushed on the head unless already closed; returns the new list.
    pub fn put_on_closed_list(&mut self, c: CellId, list: Option<CellId>) -> Option<CellId> {
        let me = self.info_of(c);
        let mut list = list;
        if !self.infos[me as usize].closed {
            let head_info = list.map(|l| self.info_of(l));
            let info = self.inf(me);
            info.closed = true;
            info.prev_open = None;
            info.next_open = head_info;
            if let Some(h) = head_info {
                self.inf(h).prev_open = Some(me);
            }
            list = Some(c);
        }
        list
    }

    /// `removeFromClosedList(list)`; returns the new list.
    pub fn remove_from_closed_list(&mut self, c: CellId, list: Option<CellId>) -> Option<CellId> {
        let list = self.unlink(c, list);
        let me = self.info_of(c);
        let info = self.inf(me);
        info.closed = false;
        info.next_open = None;
        if !plant(3020) {
            info.prev_open = None;
        }
        list
    }

    /// `costToGoal(goal)`: the octile estimate, 10 per cell along the longer axis and 5 per cell along the shorter.
    pub fn cost_to_goal(&self, c: CellId, goal: CellId) -> u32 {
        let me = &self.infos[self.info_of(c) as usize];
        let mut dx = me.pos.x - i32::from(self.x_index(goal));
        let mut dy = me.pos.y - i32::from(self.y_index(goal));
        if dx < 0 {
            dx = -dx;
        }
        if dy < 0 {
            dy = -dy;
        }
        let cost = if dx > dy { COST_ORTHOGONAL * dx + (COST_ORTHOGONAL * dy) / if plant(3016) { 3 } else { 2 } } else { COST_ORTHOGONAL * dy + (COST_ORTHOGONAL * dx) / 2 };
        cost as u32
    }

    /// `costToHierGoal(goal)`: 10 times the straight distance, rounded (`REAL_TO_INT_FLOOR` of the `double` sum plus 0.5, as a
    /// `Real`); 100000 for a cell without an info.
    pub fn cost_to_hier_goal(&self, c: CellId, goal: CellId) -> u32 {
        let Some(me) = self.cells[c as usize].info else { return 100_000 };
        let me = &self.infos[me as usize];
        let dx = me.pos.x - i32::from(self.x_index(goal));
        let dy = me.pos.y - i32::from(self.y_index(goal));
        let d = f64::from(COST_ORTHOGONAL) * f64::from(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))).sqrt() + if plant(3018) { 0.0 } else { 0.5 };
        real_to_int_floor(d as f32) as u32
    }

    /// `costSoFar(parent)`: the parent's cost so far plus 10 (orthogonal) or 14 (diagonal), 14 more into a pinched cell, plus 4, 8 or 16
    /// for a turn of 45, 90 or 135 degrees from the parent's own step; 0 for the first cell.
    pub fn cost_so_far(&self, c: CellId, parent: Option<CellId>) -> u32 {
        let me = self.info_of(c);
        let Some(parent) = parent else { return 0 };
        let my_pos = self.infos[me as usize].pos;
        let prev_dir = ICoord2D { x: i32::from(self.x_index(parent)) - my_pos.x, y: i32::from(self.y_index(parent)) - my_pos.y };
        let mut cost = if prev_dir.x == 0 || prev_dir.y == 0 {
            self.get_cost_so_far(parent).wrapping_add(COST_ORTHOGONAL as u32) as i32
        } else {
            self.get_cost_so_far(parent).wrapping_add(COST_DIAGONAL as u32) as i32
        };
        if self.cells[c as usize].pinched {
            cost += COST_DIAGONAL;
        }
        // "Increase cost of turns."
        let mut num_turns = 0;
        if let Some(prev_cell) = self.get_parent_cell(parent) {
            let dir = ICoord2D {
                x: i32::from(self.x_index(prev_cell)) - i32::from(self.x_index(parent)),
                y: i32::from(self.y_index(prev_cell)) - i32::from(self.y_index(parent)),
            };
            if dir.x != prev_dir.x || dir.y != prev_dir.y {
                let dot = dir.x * prev_dir.x + dir.y * prev_dir.y;
                num_turns = if dot > 0 {
                    4 // "45 degree turn"
                } else if dot == 0 && !plant(3017) {
                    8 // "90 degree turn"
                } else {
                    16 // "135 degree turn"
                };
            }
        }
        (cost + num_turns) as u32
    }
}
