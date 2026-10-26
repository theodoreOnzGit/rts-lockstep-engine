// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h (PathfindLayer)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (lines 3198-3827: PathfindLayer)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `PathfindLayer`: the cells of a bridge, or of the wall layer that lets units run along a wall's top. `p1w13-sonnet`, layer 10,
//! batch P3.
//!
//! A bridge's layer covers its bounds, one cell more each way, clipped to the map; a cell is clear where all four corners are on the
//! bridge, impassable where none is, and a bridge's impassable cell otherwise (or on the bridge's side); a cell at the bridge's end is
//! clear, and an entry point connects the layer to the ground cell below it, both ways. Where the bridge is less than 10 above the
//! ground, the ground cell under it becomes a bridge's impassable cell (an obstacle stays one). A destroyed bridge's cells are all a
//! bridge's impassable cells, and its ground cells no longer connect. The wall layer is the same over the wall's pieces, with the
//! cells next to a non-clear cell made cliffs.
//!
//! # The shape
//!
//! The struct is [`PathfindLayerState`] (`PathfindLayer` is the layer NUMBER here, as in `common::game_type`). Its cells are a block of
//! the [`CellStore`] (cell `(i, j)` of the layer is `block + i * height + j`), appended when allocated and dropped when reset (truncated
//! when it is the store's last block; otherwise left, reset, until the map's store is rebuilt). What it asks of the bridge, the terrain
//! and the wall's pieces is a [`LayerHost`]; the bridge is a [`BridgeHandle`] the host understands.
//!
//! # Upstream facts kept
//!
//! - `classifyCells` computes the bridge's ends inside its column loop (the same each time), so a layer without cells keeps (-1, -1).
//! - `allocateCells` subtracts `PATHFIND_CELL_SIZE/100`, integer 0, from the bounds.
//! - `getCell` hides a layer's impassable cells (the pathfinder then uses the ground's).
//! - `allocateCellsForWallLayer` with no piece found reads uninitialised bounds; here (as in the differential build, which zeroes
//!   locals) they are zero.

use crate::common::base_type::{real_to_int_ceil, real_to_int_floor, Coord2D, Coord3D, ICoord2D, IRegion2D, Region2D};
use crate::common::game_type::{ObjectId, PathfindLayer, LAYER_GROUND, LAYER_INVALID, LAYER_WALL};
use crate::common::trig;
use crate::logic::ai::cell::{CellId, CellStore, PathfindCell, CELL_BRIDGE_IMPASSABLE, CELL_CLEAR, CELL_CLIFF, CELL_IMPASSABLE, CELL_OBSTACLE};
use crate::logic::ai::zone::{GroundMap, PathfindZoneManager};
use crate::logic::locomotor::LocomotorSurfaceTypeMask;
use crate::logic::modules::plant;

/// `PATHFIND_CELL_SIZE`, `PATHFIND_CELL_SIZE_F`.
pub const PATHFIND_CELL_SIZE: i32 = 10;
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// `LAYER_Z_CLOSE_ENOUGH_F`.
pub const LAYER_Z_CLOSE_ENOUGH_F: f32 = 10.0;

/// A bridge, as the host knows it (upstream's `Bridge *`).
pub type BridgeHandle = u32;

/// What the wall layer reads of a wall piece: its position and orientation, its geometry's 2-D bounds there (`get2DBounds`), its radii
/// and whether it is a sphere.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WallPieceFacts {
    pub pos: Coord3D,
    pub orientation: f32,
    pub bounds_2d: Region2D,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub is_sphere: bool,
}

/// What a layer asks of the world: its bridge (`Bridge`'s bounds, `BridgeInfo`'s ends and object, which points and cells are on it,
/// its height), the ground's height (`TerrainLogic::getLayerHeight(x, y, LAYER_GROUND)`), and the wall's pieces
/// (`GameLogic::findObjectByID`).
pub trait LayerHost {
    fn bridge_bounds(&self, bridge: BridgeHandle) -> Region2D;
    /// `BridgeInfo::from`, `to`.
    fn bridge_ends(&self, bridge: BridgeHandle) -> (Coord3D, Coord3D);
    /// `BridgeInfo::bridgeObjectID`.
    fn bridge_object_id(&self, bridge: BridgeHandle) -> ObjectId;
    fn is_point_on_bridge(&self, bridge: BridgeHandle, p: &Coord3D) -> bool;
    fn is_cell_on_end(&self, bridge: BridgeHandle, cell: &Region2D) -> bool;
    fn is_cell_on_side(&self, bridge: BridgeHandle, cell: &Region2D) -> bool;
    fn is_cell_entry_point(&self, bridge: BridgeHandle, cell: &Region2D) -> bool;
    /// `getBridgeHeight(pLoc, NULL)`.
    fn bridge_height(&self, bridge: BridgeHandle, p: &Coord3D) -> f32;
    fn ground_height(&self, x: f32, y: f32) -> f32;
    fn wall_piece(&self, id: ObjectId) -> Option<WallPieceFacts>;
}

/// `PathfindLayer` (see the module's note on the name).
#[derive(Clone, Debug, PartialEq)]
pub struct PathfindLayerState {
    /// `m_blockOfMapCells`: the first of the layer's cells in the store.
    pub block: Option<CellId>,
    pub width: i32,
    pub height: i32,
    pub x_origin: i32,
    pub y_origin: i32,
    pub start_cell: ICoord2D,
    pub end_cell: ICoord2D,
    pub layer: PathfindLayer,
    pub zone: i32,
    pub bridge: Option<BridgeHandle>,
    pub destroyed: bool,
}

impl Default for PathfindLayerState {
    /// `PathfindLayer()` (`m_layer` is not initialised upstream; 0 here, as the differential build's zeroed memory has it).
    fn default() -> Self {
        PathfindLayerState {
            block: None,
            width: 0,
            height: 0,
            x_origin: 0,
            y_origin: 0,
            start_cell: ICoord2D { x: -1, y: -1 },
            end_cell: ICoord2D { x: -1, y: -1 },
            layer: LAYER_INVALID,
            zone: 0,
            bridge: None,
            destroyed: false,
        }
    }
}

/// The corners and bounds of cell `(i, j)`: top left, bottom right.
fn cell_corners(i: i32, j: i32) -> (Coord3D, Coord3D) {
    // (upstream leaves z uninitialised; the differential build zeroes it, and the bridge's tests read x and y)
    let tl = Coord3D { x: i as f32 * PATHFIND_CELL_SIZE_F, y: j as f32 * PATHFIND_CELL_SIZE_F, z: 0.0 };
    let br = Coord3D { x: tl.x + PATHFIND_CELL_SIZE_F, y: tl.y + PATHFIND_CELL_SIZE_F, z: 0.0 };
    (tl, br)
}

/// How many of the cell's corners `on` holds (top left, bottom left, bottom right, top right, as upstream asks).
fn corners_on(tl: &Coord3D, br: &Coord3D, mut on: impl FnMut(&Coord3D) -> bool) -> i32 {
    let mut count = 0;
    for p in [*tl, Coord3D { y: br.y, ..*tl }, *br, Coord3D { x: br.x, ..*tl }] {
        if on(&p) {
            count += 1;
        }
    }
    count
}

impl PathfindLayerState {
    /// The store's cell `(i, j)` of the layer (unchecked, as upstream indexes `m_layerCells[i][j]`).
    pub fn layer_cell(&self, i: i32, j: i32) -> CellId {
        self.block.expect("the layer has cells") + (i * self.height + j) as CellId
    }

    /// `reset()`: the cells reset and dropped, no bridge, no extent, the ends (-1, -1), the ground layer.
    pub fn reset(&mut self, s: &mut CellStore) {
        self.bridge = None;
        if let Some(block) = self.block {
            for i in 0..self.width {
                for j in 0..self.height {
                    s.reset(self.layer_cell(i, j));
                }
            }
            let end = block as usize + (self.width * self.height) as usize;
            if end == s.cells.len() {
                s.cells.truncate(block as usize);
            }
            self.block = None;
        }
        self.width = 0;
        self.height = 0;
        self.x_origin = 0;
        self.y_origin = 0;
        self.start_cell = ICoord2D { x: -1, y: -1 };
        self.end_cell = ICoord2D { x: -1, y: -1 };
        self.layer = LAYER_GROUND;
    }

    /// `isUnused()`: "True if it doesn't contain a bridge" (a wall layer with cells is used).
    pub fn is_unused(&self) -> bool {
        if self.layer == LAYER_WALL && self.width > 0 {
            return false;
        }
        self.bridge.is_none()
    }
    /// `isDestroyed()`.
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }
    /// `getZone()`, `setZone(zone)`.
    pub fn get_zone(&self) -> i32 {
        self.zone
    }
    pub fn set_zone(&mut self, zone: i32) {
        self.zone = zone;
    }
    /// `getStartCellIndex(start)`, `getEndCellIndex(end)`.
    pub fn get_start_cell_index(&self) -> ICoord2D {
        self.start_cell
    }
    pub fn get_end_cell_index(&self) -> ICoord2D {
        self.end_cell
    }

    /// `init(theBridge, layer)`: false when the layer has a bridge already.
    pub fn init(&mut self, bridge: Option<BridgeHandle>, layer: PathfindLayer) -> bool {
        if self.bridge.is_some() {
            return false;
        }
        self.bridge = bridge;
        self.layer = layer;
        self.destroyed = false;
        true
    }

    /// The extent of `bounds`, one cell more each way, clipped to `extent`; then the cells, when there is room for any.
    fn allocate_over(&mut self, s: &mut CellStore, bounds: &Region2D, extent: &IRegion2D) {
        // (`PATHFIND_CELL_SIZE/100` is integer 0)
        self.x_origin = real_to_int_floor(bounds.lo.x / PATHFIND_CELL_SIZE as f32);
        self.y_origin = real_to_int_floor(bounds.lo.y / PATHFIND_CELL_SIZE as f32);
        self.width = 0;
        self.height = 0;
        let mut max_x = real_to_int_ceil(bounds.hi.x / PATHFIND_CELL_SIZE as f32);
        let mut max_y = real_to_int_ceil(bounds.hi.y / PATHFIND_CELL_SIZE as f32);
        self.x_origin -= 1;
        self.y_origin -= 1;
        max_x += 1;
        max_y += 1;
        self.x_origin = self.x_origin.max(extent.lo.x);
        self.y_origin = self.y_origin.max(extent.lo.y);
        max_x = max_x.min(extent.hi.x);
        max_y = max_y.min(if plant(3035) { extent.hi.y + 1 } else { extent.hi.y });
        if max_x <= self.x_origin || max_y <= self.y_origin {
            return;
        }
        self.width = max_x - self.x_origin;
        self.height = max_y - self.y_origin;
        let start = s.cells.len();
        self.block = Some(start as CellId);
        s.cells.resize(start + (self.width * self.height) as usize, PathfindCell::default());
    }

    /// `allocateCells(extent)`: the cells over the bridge's bounds (nothing without a bridge).
    pub fn allocate_cells(&mut self, s: &mut CellStore, extent: &IRegion2D, host: &impl LayerHost) {
        let Some(bridge) = self.bridge else { return };
        let bounds = host.bridge_bounds(bridge);
        self.allocate_over(s, &bounds, extent);
    }

    /// `allocateCellsForWallLayer(extent, wallPieces, numPieces)`: the cells over the union of the pieces' bounds (the wall layer only).
    pub fn allocate_cells_for_wall_layer(&mut self, s: &mut CellStore, extent: &IRegion2D, wall_pieces: &[ObjectId], host: &impl LayerHost) {
        if self.layer != LAYER_WALL {
            return;
        }
        let mut bounds = Region2D::default();
        let mut first = true;
        for &id in wall_pieces {
            let Some(piece) = host.wall_piece(id) else { continue };
            let b = piece.bounds_2d;
            if first {
                bounds = b;
                first = false;
            } else {
                bounds.lo.x = bounds.lo.x.min(b.lo.x);
                bounds.lo.y = bounds.lo.y.min(b.lo.y);
                bounds.hi.x = bounds.hi.x.max(b.hi.x);
                bounds.hi.y = bounds.hi.y.max(b.hi.y);
            }
        }
        self.allocate_over(s, &bounds, extent);
    }

    /// `connectsZones(zm, locoSet, zone1, zone2)`: a destroyed bridge whose ground connections lie in both zones (each ground cell's
    /// zone for the surfaces, as a crusher, then its terrain zone).
    pub fn connects_zones(&self, s: &CellStore, ground: &GroundMap, zm: &PathfindZoneManager, surfaces: LocomotorSurfaceTypeMask, zone1: i32, zone2: i32) -> bool {
        if !self.destroyed {
            return false;
        }
        let (mut found1, mut found2) = (false, false);
        for i in 0..self.width {
            for j in 0..self.height {
                let cell = self.layer_cell(i, j);
                if s.cells[cell as usize].get_connect_layer() == LAYER_GROUND {
                    if let Some(g) = ground.get(i + self.x_origin, j + self.y_origin) {
                        let zone = zm.get_effective_zone(surfaces, true, s.cells[g as usize].get_zone());
                        let zone = i32::from(zm.get_effective_terrain_zone(zone));
                        found1 |= zone == zone1;
                        found2 |= zone == zone2;
                    }
                }
            }
        }
        found1 && found2
    }

    /// Disconnect a destroyed layer's ground cells and make its cells `ty`.
    fn destroy_cells(&self, s: &mut CellStore, ground: &GroundMap, ty: u8) {
        for i in 0..self.width {
            for j in 0..self.height {
                let cell = self.layer_cell(i, j);
                if s.cells[cell as usize].get_connect_layer() == LAYER_GROUND {
                    if let Some(g) = ground.get(i + self.x_origin, j + self.y_origin) {
                        s.cells[g as usize].set_connect_layer(LAYER_INVALID); // "disconnect it."
                    }
                }
                s.set_type(cell, ty);
            }
        }
    }

    /// `classifyCells()`: each cell from the bridge, then the bridge's ends (0.7 cells beyond its two points); a destroyed bridge's
    /// cells impassable and disconnected.
    pub fn classify_cells(&mut self, s: &mut CellStore, ground: &GroundMap, host: &impl LayerHost) {
        self.start_cell = ICoord2D { x: -1, y: -1 };
        self.end_cell = ICoord2D { x: -1, y: -1 };
        for i in 0..self.width {
            for j in 0..self.height {
                let cell = self.layer_cell(i, j);
                s.cells[cell as usize].set_connect_layer(LAYER_INVALID);
                s.cells[cell as usize].set_layer(self.layer);
                self.classify_layer_map_cell(s, ground, host, i + self.x_origin, j + self.y_origin, cell);
            }
            let bridge = self.bridge.expect("a bridge's layer has its bridge");
            let (from, to) = host.bridge_ends(bridge);
            let mut dir = Coord3D { x: to.x - from.x, y: to.y - from.y, z: to.z - from.z };
            dir.normalize();
            dir.x *= PATHFIND_CELL_SIZE_F * 0.7;
            dir.y *= PATHFIND_CELL_SIZE_F * 0.7;
            self.start_cell = ICoord2D {
                x: real_to_int_floor((from.x - dir.x) / PATHFIND_CELL_SIZE_F),
                y: real_to_int_floor((from.y - dir.y) / PATHFIND_CELL_SIZE_F),
            };
            self.end_cell = ICoord2D {
                x: real_to_int_floor((to.x + dir.x) / PATHFIND_CELL_SIZE_F),
                y: real_to_int_floor((to.y + if plant(3036) { -dir.y } else { dir.y }) / PATHFIND_CELL_SIZE_F),
            };
        }
        if self.destroyed {
            self.destroy_cells(s, ground, CELL_BRIDGE_IMPASSABLE);
        }
    }

    /// `classifyWallCells(wallPieces, numPieces)`: each cell from the pieces (the wall layer, with cells, only); a destroyed wall's
    /// cells impassable and disconnected; then a clear cell next to (or at) a non-clear one becomes a cliff (the edge cells are not
    /// looked at).
    pub fn classify_wall_cells(&mut self, s: &mut CellStore, ground: &GroundMap, wall_pieces: &[ObjectId], host: &impl LayerHost) {
        if self.layer != LAYER_WALL || self.block.is_none() {
            return;
        }
        for i in 0..self.width {
            for j in 0..self.height {
                let cell = self.layer_cell(i, j);
                s.cells[cell as usize].set_connect_layer(LAYER_INVALID);
                s.cells[cell as usize].set_layer(self.layer);
                self.classify_wall_map_cell(s, host, i + self.x_origin, j + self.y_origin, cell, wall_pieces);
                s.cells[cell as usize].set_pinched(false);
            }
        }
        if self.destroyed {
            self.destroy_cells(s, ground, CELL_IMPASSABLE);
        }
        for i in 1..self.width - 1 {
            for j in 1..self.height - 1 {
                let cell = self.layer_cell(i, j);
                for k in i - 1..i + 2 {
                    for l in j - 1..j + 2 {
                        let t = s.cells[self.layer_cell(k, l) as usize].get_type();
                        if if plant(3040) { t == CELL_IMPASSABLE } else { t != CELL_CLEAR } {
                            s.cells[cell as usize].set_pinched(true);
                        }
                    }
                }
            }
        }
        for i in 0..self.width {
            for j in 0..self.height {
                let cell = self.layer_cell(i, j);
                if s.cells[cell as usize].get_pinched() && s.cells[cell as usize].get_type() == CELL_CLEAR {
                    s.set_type(cell, CELL_CLIFF);
                }
                s.cells[cell as usize].set_pinched(false);
            }
        }
    }

    /// `setDestroyed(destroyed)`: a change reclassifies the cells; false when it is no change.
    pub fn set_destroyed(&mut self, destroyed: bool, s: &mut CellStore, ground: &GroundMap, host: &impl LayerHost) -> bool {
        if destroyed == self.destroyed {
            return false;
        }
        self.destroyed = destroyed;
        self.classify_cells(s, ground, host);
        true
    }

    /// `applyZone()`: "Copies m_zone into the zone for all the member cells."
    pub fn apply_zone(&self, s: &mut CellStore) {
        for i in 0..self.width {
            for j in 0..self.height {
                s.cells[self.layer_cell(i, j) as usize].set_zone(self.zone as u16);
            }
        }
    }

    /// `getBridgeID()`.
    pub fn get_bridge_id(&self, host: &impl LayerHost) -> ObjectId {
        host.bridge_object_id(self.bridge.expect("a bridge's layer has its bridge"))
    }

    /// `getCell(x, y)`: the layer's cell at map cell `(x, y)`; none without cells, outside the layer, or impassable ("Impassable cells
    /// are ignored.").
    pub fn get_cell(&self, s: &CellStore, x: i32, y: i32) -> Option<CellId> {
        self.block?;
        let (x, y) = (x - self.x_origin, y - self.y_origin);
        if x < 0 || x >= self.width || y < 0 || y >= self.height {
            return None;
        }
        let cell = self.layer_cell(x, y);
        (s.cells[cell as usize].get_type() != CELL_IMPASSABLE).then_some(cell)
    }

    /// `classifyLayerMapCell(i, j, cell, theBridge)`.
    fn classify_layer_map_cell(&self, s: &mut CellStore, ground: &GroundMap, host: &impl LayerHost, i: i32, j: i32, cell: CellId) {
        let bridge = self.bridge.expect("a bridge's layer has its bridge");
        let (tl, br) = cell_corners(i, j);
        let bridge_count = corners_on(&tl, &br, |p| host.is_point_on_bridge(bridge, p));
        s.reset(cell);
        s.cells[cell as usize].set_layer(self.layer);
        s.set_type(cell, CELL_IMPASSABLE);
        if bridge_count == 4 {
            s.set_type(cell, CELL_CLEAR);
        } else {
            if bridge_count != 0 {
                s.set_type(cell, CELL_BRIDGE_IMPASSABLE); // "it's off the bridge."
            }
            let bounds = Region2D { lo: Coord2D { x: tl.x, y: tl.y }, hi: Coord2D { x: br.x, y: br.y } };
            if host.is_cell_on_side(bridge, &bounds) {
                s.set_type(cell, CELL_BRIDGE_IMPASSABLE);
            } else {
                if host.is_cell_on_end(bridge, &bounds) {
                    s.set_type(cell, CELL_CLEAR);
                }
                if host.is_cell_entry_point(bridge, &bounds) {
                    s.set_type(cell, CELL_CLEAR);
                    s.cells[cell as usize].set_connect_layer(LAYER_GROUND);
                    let g = ground.get(i, j).expect("a layer lies within the map");
                    let layer = s.cells[cell as usize].get_layer();
                    s.cells[g as usize].set_connect_layer(layer);
                }
            }
        }
        // (`PATHFIND_CELL_SIZE/2` is integer 5)
        let center = Coord3D { x: tl.x + 5.0, y: tl.y + 5.0, z: tl.z };
        if s.cells[cell as usize].get_type() != CELL_IMPASSABLE && s.cells[cell as usize].get_connect_layer() != LAYER_GROUND {
            let ground_height = host.ground_height(center.x, center.y);
            let bridge_height = host.bridge_height(bridge, &center);
            if ground_height + LAYER_Z_CLOSE_ENOUGH_F > bridge_height {
                let g = ground.get(i, j).expect("a layer lies within the map");
                if s.cells[g as usize].get_type() != CELL_OBSTACLE {
                    s.set_type(g, CELL_BRIDGE_IMPASSABLE);
                }
            }
        }
    }

    /// `isPointOnWall(wallPieces, numPieces, pt)`: within a piece's rectangle (its major radius along its orientation, its minor, or
    /// for a sphere its major, across).
    pub fn is_point_on_wall(&self, wall_pieces: &[ObjectId], pt: &Coord3D, host: &impl LayerHost) -> bool {
        for &id in wall_pieces {
            let Some(piece) = host.wall_piece(id) else { continue };
            let major = piece.major_radius;
            let minor = if piece.is_sphere { piece.major_radius } else { piece.minor_radius };
            let c = trig::cos(-piece.orientation);
            let sn = trig::sin(-piece.orientation);
            let ptx = pt.x - piece.pos.x;
            let pty = pt.y - piece.pos.y;
            let ptx_new = (ptx * c - pty * sn).abs();
            let pty_new = (ptx * sn + pty * c).abs();
            if ptx_new <= major && (pty_new <= minor || plant(3037)) {
                return true;
            }
        }
        false
    }

    /// `classifyWallMapCell(i, j, cell, wallPieces, numPieces)`: clear with all four corners on the wall, impassable with none, a
    /// bridge's impassable cell otherwise.
    fn classify_wall_map_cell(&self, s: &mut CellStore, host: &impl LayerHost, i: i32, j: i32, cell: CellId, wall_pieces: &[ObjectId]) {
        let (tl, br) = cell_corners(i, j);
        let count = corners_on(&tl, &br, |p| self.is_point_on_wall(wall_pieces, p, host));
        s.reset(cell);
        s.cells[cell as usize].set_layer(self.layer);
        s.set_type(cell, CELL_IMPASSABLE);
        if count == 4 {
            s.set_type(cell, CELL_CLEAR);
        } else if count != 0 {
            s.set_type(cell, CELL_BRIDGE_IMPASSABLE); // "it's off the bridge."
        }
    }
}
