// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h (Pathfinder)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (Pathfinder: lines 3828-4691, the map and the footprints;
//     9686-10111, the bridges' state and the units' cells)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `Pathfinder`: the pathfinding map and its services. `p1w13-sonnet`, layer 10, batch P4 (the map, the objects' footprints, the
//! bridges' layers, the wall, the units' goal and position cells); the searches are later batches.
//!
//! # The map
//!
//! `newMap` sizes the map from the terrain's pathfinding extent (one cell less each way than its last whole cell), allocates the
//! zone blocks, the cells and the used layers' cells (and the wall layer's, with any wall piece), then `classifyMap`: each cell clear,
//! a cliff (the terrain's cliff cell at its top left corner) or water (any corner underwater), an obstacle kept; the clear cells next
//! to a cliff BIT (cliff, rubble, impassable) pinched, made cliffs, and the clear cells next to those pinched in turn; the layers
//! classified; the zones calculated. Then every object's footprint.
//!
//! # Footprints
//!
//! A structure that is not mobile, not small, and no more than a cell above the terrain (a blast crater whatever its height) marks the
//! cells under it obstacles: a box stepped in half cells along its axes, a sphere or cylinder as the cells whose centres are within
//! its radius plus 0.4 cells; then the zones of those cells are updated, and in two cells around them a clear cell with fewer than two
//! clear orthogonal neighbours or four clear neighbours is made impassable, and a clear cell orthogonally next to an obstacle pinched
//! (removal first turns that area's impassable cells clear). A fence (a template with a fence width, not a defensive wall) marks the
//! cells along it as fence obstacles. Mines, projectiles and bridge towers have no footprint; a blast crater's is never removed.
//! Removing an object also removes it from the units' cells and, for a wall piece, from the wall (splatting what stands on it).
//!
//! # Upstream facts kept
//!
//! - Footprints and fences skip the map's last row and column (`cx < m_extent.hi.x`).
//! - `reset` frees the map's cells in reverse order (each destructor returns its info to the pool); the layers' cells are reset first
//!   in their own order.
//! - `addWallPiece` keeps at most 127 pieces (`< MAX_WALL_PIECES - 1`).
//! - `classifyFence`'s bounds start with an uninitialised high corner (zero here, as in the differential build).
//! - The cliff test of `classifyMap` is a BIT test (`type & CELL_CLIFF`): rubble and impassable cells pinch their neighbours too.
//!
//! # Where this port differs
//!
//! - `newMap` with a different extent while a map exists: upstream asserts and leaks the old map's cells; here it panics (the map
//!   must be the store's first cells). Upstream calls `reset` first.
//! - A unit's goal is removed over a radius of at least 1; its cells are set over its radius (`getRadiusAndCenter`: the bounding
//!   circle's diameter in cells, rounded with 0.3, 2 cells for a diameter between 1 and 2 cells, halved, at most 2).
//!
//! # The shape
//!
//! The cells are the [`CellStore`]'s (the map's first, [`GroundMap`]); the layers are [`PathfindLayerState`]s; what the pathfinder
//! asks of the objects, the terrain and the AI is a [`PathfinderHost`], with each object's facts a [`PfObjectFacts`].

use crate::common::base_type::{real_to_int_ceil, real_to_int_floor, Coord2D, Coord3D, ICoord2D, IRegion2D, Region3D};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND, LAYER_LAST, LAYER_WALL};
use crate::common::trig;
use crate::logic::ai::cell::{CellId, CellStore, ObstacleFacts, CELL_CLEAR, CELL_CLIFF, CELL_IMPASSABLE, CELL_OBSTACLE, CELL_WATER};
use crate::logic::ai::layer::{BridgeHandle, LayerHost, PathfindLayerState, PATHFIND_CELL_SIZE, PATHFIND_CELL_SIZE_F};
use crate::logic::ai::path::Path;
use crate::logic::ai::zone::{GroundMap, PathfindZoneManager, UNINITIALIZED_ZONE};
use crate::logic::modules::plant;

/// `MAX_WALL_PIECES`.
pub const MAX_WALL_PIECES: usize = 128;
/// `PATHFIND_QUEUE_LEN`.
pub const PATHFIND_QUEUE_LEN: usize = 512;

/// `GeometryType` (Geometry.h): the shapes a footprint knows.
pub const GEOMETRY_SPHERE: u8 = 0;
pub const GEOMETRY_CYLINDER: u8 = 1;
pub const GEOMETRY_BOX: u8 = 2;

/// What the pathfinder reads of a unit's AI (`AIUpdateInterface`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PfAiFacts {
    pub doing_ground_movement: bool,
    pub aircraft_adjusts_destination: bool,
    /// `getPathfindGoalCell()`, `getCurPathfindCell()`.
    pub goal_cell: ICoord2D,
    pub cur_cell: ICoord2D,
}

/// What the pathfinder reads of an object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PfObjectFacts {
    pub id: ObjectId,
    pub pos: Coord3D,
    pub orientation: f32,
    pub layer: PathfindLayer,
    pub destination_layer: PathfindLayer,
    /// The kinds the pathfinder asks for.
    pub mine: bool,
    pub projectile: bool,
    pub bridge_tower: bool,
    pub defensive_wall: bool,
    pub blast_crater: bool,
    pub walk_on_top_of_wall: bool,
    pub structure: bool,
    pub immobile: bool,
    pub produced_at_helipad: bool,
    pub can_see_through_structure: bool,
    /// (batch P5) `KINDOF_AIRCRAFT_PATH_AROUND`, `KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT`.
    pub aircraft_path_around: bool,
    pub attack_needs_line_of_sight: bool,
    /// `isDisabledByType(DISABLED_UNMANNED)`.
    pub unmanned: bool,
    /// `getBodyModule()->getDamageState() == BODY_RUBBLE`.
    pub rubble: bool,
    /// The template's `getFenceWidth()`, `getFenceXOffset()`.
    pub fence_width: f32,
    pub fence_x_offset: f32,
    pub mobile: bool,
    /// The geometry: its type, radii, `getIsSmall()`, `getBoundingCircleRadius()`.
    pub geom_type: u8,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub small: bool,
    pub bounding_circle_radius: f32,
    pub height_above_terrain: f32,
    pub ai: Option<PfAiFacts>,
}

impl PfObjectFacts {
    fn obstacle(&self) -> ObstacleFacts {
        ObstacleFacts { id: self.id, rubble: self.rubble, see_through: self.can_see_through_structure }
    }
}

/// What the pathfinder asks of the world: the objects (by id and in the game logic's order), the terrain, the AI's data, and the
/// changes it makes to objects.
pub trait PathfinderHost: LayerHost {
    /// `TheGameLogic->findObjectByID(id)`.
    fn object(&self, id: ObjectId) -> Option<PfObjectFacts>;
    /// `getFirstObject()` .. `getNextObject()`.
    fn objects(&self) -> Vec<ObjectId>;
    fn set_layer(&mut self, id: ObjectId, layer: PathfindLayer);
    fn set_destination_layer(&mut self, id: ObjectId, layer: PathfindLayer);
    fn set_pathfind_goal_cell(&mut self, id: ObjectId, cell: ICoord2D);
    fn set_cur_pathfind_cell(&mut self, id: ObjectId, cell: ICoord2D);
    /// `attemptDamage` of `HUGE_DAMAGE_AMOUNT`, `DAMAGE_FALLING`, `DEATH_SPLATTED`, from the object itself.
    fn splat(&mut self, id: ObjectId);
    fn is_cliff_cell(&self, x: f32, y: f32) -> bool;
    fn is_underwater(&self, x: f32, y: f32) -> bool;
    fn maximum_pathfind_extent(&self) -> Region3D;
    fn object_interacts_with_bridge_layer(&self, id: ObjectId, layer: PathfindLayer) -> bool;
    fn object_interacts_with_bridge_end(&self, id: ObjectId, layer: PathfindLayer) -> bool;
    /// `TheAI->getAiData()->m_wallHeight`.
    fn wall_height(&self) -> f32;
}

/// `Pathfinder`.
#[derive(Clone, Debug)]
pub struct Pathfinder {
    pub store: CellStore,
    /// `m_blockOfMapCells` and `m_map` allocated.
    pub map_allocated: bool,
    pub extent: IRegion2D,
    pub logical_extent: IRegion2D,
    pub open_list: Option<CellId>,
    pub closed_list: Option<CellId>,
    pub is_map_ready: bool,
    pub is_tunneling: bool,
    pub frame_to_show_obstacles: i32,
    pub debug_path_pos: Coord3D,
    pub debug_path: Option<Path>,
    pub ignore_obstacle_id: ObjectId,
    pub zone_manager: PathfindZoneManager,
    pub layers: Vec<PathfindLayerState>,
    pub wall_pieces: Vec<ObjectId>,
    pub num_wall_pieces: i32,
    pub wall_height: f32,
    pub move_allies_depth: i32,
    pub queued_pathfind_requests: Vec<ObjectId>,
    pub queue_pr_head: i32,
    pub queue_pr_tail: i32,
    pub cumulative_cells_allocated: i32,
}

impl Pathfinder {
    /// `Pathfinder()`: the info pool, then `reset` (`wall_height`: the AI's data, none before the AI has it).
    pub fn new(wall_height: Option<f32>) -> Self {
        let mut store = CellStore::new();
        store.allocate_cell_infos();
        let mut pf = Pathfinder {
            store,
            map_allocated: false,
            extent: IRegion2D::default(),
            logical_extent: IRegion2D::default(),
            open_list: None,
            closed_list: None,
            is_map_ready: false,
            is_tunneling: false,
            frame_to_show_obstacles: 0,
            debug_path_pos: Coord3D::default(),
            debug_path: None,
            ignore_obstacle_id: INVALID_ID,
            zone_manager: PathfindZoneManager::new(),
            layers: vec![PathfindLayerState::default(); (LAYER_LAST + 1) as usize],
            wall_pieces: vec![INVALID_ID; MAX_WALL_PIECES],
            num_wall_pieces: 0,
            wall_height: 0.0,
            move_allies_depth: 0,
            queued_pathfind_requests: vec![INVALID_ID; PATHFIND_QUEUE_LEN],
            queue_pr_head: 0,
            queue_pr_tail: 0,
            cumulative_cells_allocated: 0,
        };
        pf.reset(wall_height);
        pf
    }

    /// The map as the zone and layer code index it.
    pub fn ground(&self) -> GroundMap {
        GroundMap { extent: self.extent }
    }

    /// `reset()`: "Reset system in preparation for new map" (`wall_height`: the AI's data, or none: 0).
    pub fn reset(&mut self, wall_height: Option<f32>) {
        self.frame_to_show_obstacles = 0;
        let map_cells = if self.map_allocated { ((self.extent.hi.x + 1) * (self.extent.hi.y + 1)) as usize } else { 0 };
        // delete[] m_blockOfMapCells: the destructors, last first, return the infos
        for k in (0..map_cells).rev() {
            if let Some(i) = self.store.cells[k].info.take() {
                self.store.release_a_cell_info(i);
            }
        }
        for layer in &mut self.layers {
            layer.reset(&mut self.store);
        }
        self.store.cells.clear();
        self.map_allocated = false;
        self.extent = IRegion2D::default();
        self.logical_extent = IRegion2D::default();
        self.open_list = None;
        self.closed_list = None;
        self.ignore_obstacle_id = INVALID_ID;
        self.is_tunneling = false;
        self.move_allies_depth = 0;
        self.is_map_ready = false;
        self.cumulative_cells_allocated = 0;
        self.debug_path_pos = Coord3D::default();
        self.debug_path = None;
        self.queued_pathfind_requests.iter_mut().for_each(|q| *q = INVALID_ID);
        self.queue_pr_head = 0;
        self.queue_pr_tail = 0;
        self.num_wall_pieces = 0;
        self.wall_pieces.iter_mut().for_each(|w| *w = INVALID_ID);
        self.wall_height = wall_height.unwrap_or(0.0);
        self.zone_manager.reset();
    }

    /// `getCell(layer, x, y)`: within the extent, the layer's cell there (a bridge or the wall) if it has one, else the map's.
    pub fn get_cell(&self, layer: PathfindLayer, x: i32, y: i32) -> Option<CellId> {
        let e = &self.extent;
        if !(x >= e.lo.x && x <= e.hi.x && y >= e.lo.y && y <= e.hi.y) {
            return None;
        }
        if layer > LAYER_GROUND && layer <= LAYER_LAST {
            if let Some(c) = self.layers[layer as usize].get_cell(&self.store, x, y) {
                return Some(c);
            }
        }
        Some(self.ground().cell(x, y))
    }

    /// `worldToCell(pos, cell)`: the cell, clamped to the extent; true when it was outside.
    pub fn world_to_cell(&self, pos: &Coord3D) -> (ICoord2D, bool) {
        let mut cell = ICoord2D {
            x: real_to_int_floor(pos.x / PATHFIND_CELL_SIZE as f32),
            y: real_to_int_floor(pos.y / PATHFIND_CELL_SIZE as f32),
        };
        let e = &self.extent;
        let mut overflow = false;
        if cell.x < e.lo.x {
            overflow = true;
            cell.x = e.lo.x;
        }
        if cell.y < e.lo.y {
            overflow = true;
            cell.y = e.lo.y;
        }
        if cell.x > e.hi.x {
            overflow = !plant(3062);
            cell.x = e.hi.x;
        }
        if cell.y > e.hi.y {
            overflow = true;
            cell.y = e.hi.y;
        }
        (cell, overflow)
    }

    /// `getCell(layer, pos)`: none outside the extent.
    pub fn get_cell_at(&self, layer: PathfindLayer, pos: &Coord3D) -> Option<CellId> {
        let (cell, overflow) = self.world_to_cell(pos);
        if overflow {
            return None;
        }
        self.get_cell(layer, cell.x, cell.y)
    }

    /// `getClippedCell(layer, pos)`: the cell, the position clamped to the extent.
    pub fn get_clipped_cell(&self, layer: PathfindLayer, pos: &Coord3D) -> Option<CellId> {
        let (cell, _) = self.world_to_cell(pos);
        self.get_cell(layer, cell.x, cell.y)
    }

    /// `addWallPiece(wallPiece)`: at most `MAX_WALL_PIECES - 1`.
    pub fn add_wall_piece(&mut self, id: ObjectId) {
        if (self.num_wall_pieces as usize) < MAX_WALL_PIECES - 1 {
            self.wall_pieces[self.num_wall_pieces as usize] = id;
            self.num_wall_pieces += 1;
        }
    }

    /// `removeWallPiece(wallPiece)`: the last piece takes its place.
    pub fn remove_wall_piece(&mut self, id: ObjectId) {
        for i in 0..self.num_wall_pieces as usize {
            if self.wall_pieces[i] == id {
                self.wall_pieces[i] = self.wall_pieces[self.num_wall_pieces as usize - 1];
                self.num_wall_pieces -= 1;
                return;
            }
        }
    }

    fn pieces(&self) -> Vec<ObjectId> {
        self.wall_pieces[..self.num_wall_pieces.max(0) as usize].to_vec()
    }

    /// `isPointOnWall(pos)`: a clear cell of the wall layer there.
    pub fn is_point_on_wall(&self, pos: &Coord3D) -> bool {
        if self.num_wall_pieces == 0 || self.layers[LAYER_WALL as usize].is_unused() {
            return false;
        }
        match self.get_cell_at(LAYER_WALL, pos) {
            Some(c) => self.store.cells[c as usize].get_layer() == LAYER_WALL && self.store.cells[c as usize].get_type() == CELL_CLEAR,
            None => false,
        }
    }

    /// `addBridge(theBridge)`: the first unused layer above the ground (the wall's included); the ground when the layer will not take
    /// it or none is left.
    pub fn add_bridge(&mut self, bridge: BridgeHandle) -> PathfindLayer {
        for layer in LAYER_GROUND + 1..=LAYER_WALL {
            if self.layers[layer as usize].is_unused() {
                if self.layers[layer as usize].init(Some(bridge), layer) {
                    return layer;
                }
                return LAYER_GROUND; // "failed to init, usually cause off of the map."
            }
        }
        LAYER_GROUND // "Ran out of bridge layers."
    }

    /// `updateLayer(obj, layer)`: the object's layer, the ground unless it is on that bridge.
    pub fn update_layer(&self, host: &mut impl PathfinderHost, id: ObjectId, layer: PathfindLayer) {
        let mut layer = layer;
        if layer != LAYER_GROUND && !host.object_interacts_with_bridge_layer(id, layer) {
            layer = LAYER_GROUND;
        }
        host.set_layer(id, layer);
    }

    /// `changeBridgeState(layer, repaired)`: a used layer destroyed or repaired; the zones dirty when it changed.
    pub fn change_bridge_state(&mut self, host: &impl PathfinderHost, layer: PathfindLayer, repaired: bool, frame: u32) {
        if self.layers[layer as usize].is_unused() {
            return;
        }
        let ground = self.ground();
        if self.layers[layer as usize].set_destroyed(!repaired, &mut self.store, &ground, host) {
            self.zone_manager.mark_zones_dirty(repaired, frame);
        }
    }

    /// One cell of a footprint: an obstacle set or removed, its zone unset when that changed it.
    fn footprint_cell(&mut self, obj: &PfObjectFacts, fence: bool, insert: bool, cx: i32, cy: i32) -> bool {
        let c = self.ground().cell(cx, cy);
        let changed = if insert {
            self.store.set_type_as_obstacle(c, &obj.obstacle(), fence, ICoord2D { x: cx, y: cy })
        } else {
            self.store.remove_obstacle(c, obj.id)
        };
        if changed {
            self.store.cells[c as usize].set_zone(UNINITIALIZED_ZONE);
        }
        changed
    }

    /// The cells along a box (or a fence) stepped in half cells: `half_x` along the orientation (from `offset`), `half_y` across.
    #[allow(clippy::too_many_arguments)] // (upstream's two loops in one: the box's and the fence's parameters)
    fn stepped_box(&mut self, obj: &PfObjectFacts, offset: f32, half_x: f32, half_y: f32, fence: bool, insert: bool, bounds: &mut IRegion2D) -> bool {
        let c = trig::cos(obj.orientation);
        let s = trig::sin(obj.orientation);
        // "in theory, should be PATHFIND_CELL_SIZE_F exactly, but needs to be smaller to avoid aliasing problems"
        let step = PATHFIND_CELL_SIZE_F * if plant(3059) { 0.6 } else { 0.5 };
        let (ydx, ydy, xdx, xdy) = (s * step, -c * step, c * step, s * step);
        let num_x = real_to_int_ceil(2.0 * half_x / step);
        let num_y = real_to_int_ceil(2.0 * half_y / step);
        let mut tl_x = obj.pos.x - offset * c - half_y * s;
        let mut tl_y = obj.pos.y + half_y * c - offset * s;
        let mut did = false;
        for _ in 0..num_y {
            let (mut x, mut y) = (tl_x, tl_y);
            for _ in 0..num_x {
                let cx = real_to_int_floor((x + 0.5) / PATHFIND_CELL_SIZE_F);
                let cy = real_to_int_floor((y + 0.5) / PATHFIND_CELL_SIZE_F);
                if cx >= 0 && cy >= 0 && cx < self.extent.hi.x && cy < self.extent.hi.y {
                    did |= self.footprint_cell(obj, fence, insert, cx, cy);
                    bounds.lo.x = bounds.lo.x.min(cx);
                    bounds.lo.y = bounds.lo.y.min(cy);
                    bounds.hi.x = bounds.hi.x.max(cx);
                    bounds.hi.y = bounds.hi.y.max(cy);
                }
                x += xdx;
                y += xdy;
            }
            tl_x += ydx;
            tl_y += ydy;
        }
        did
    }

    fn cell_bounds_at(pos: &Coord3D) -> IRegion2D {
        let lo = ICoord2D { x: real_to_int_floor((pos.x + 0.5) / PATHFIND_CELL_SIZE_F), y: real_to_int_floor((pos.y + 0.5) / PATHFIND_CELL_SIZE_F) };
        IRegion2D { lo, hi: lo }
    }

    /// `classifyFence(obj, insert)`: the cells along the fence (its width, a tenth of a cell across, from its offset) fence obstacles
    /// or not; the zones dirty and updated when that changed any. (The bounds' high corner starts uninitialised upstream; the
    /// differential build zeroes it, as here.)
    pub fn classify_fence(&mut self, obj: &PfObjectFacts, insert: bool, frame: u32) {
        let half_x = obj.fence_width / 2.0;
        let half_y = PATHFIND_CELL_SIZE_F / 10.0;
        let mut bounds = Self::cell_bounds_at(&obj.pos);
        bounds.hi = ICoord2D { x: 0, y: 0 };
        let did = self.stepped_box(obj, obj.fence_x_offset, half_x, half_y, true, insert, &mut bounds);
        if did && !plant(3060) {
            self.zone_manager.mark_zones_dirty(insert, frame);
            let ground = self.ground();
            self.zone_manager.update_zones_for_modify(&mut self.store, &ground, &bounds, &self.extent.clone());
        }
    }

    /// `classifyObjectFootprint(obj, insert)` (see the module's notes).
    pub fn classify_object_footprint(&mut self, host: &mut impl PathfinderHost, id: ObjectId, insert: bool, frame: u32) {
        let Some(obj) = host.object(id) else { return };
        if obj.mine || obj.projectile || obj.bridge_tower {
            return;
        }
        if obj.fence_width > 0.0 && !obj.defensive_wall {
            self.classify_fence(&obj, insert, frame);
            return;
        }
        if !insert {
            if obj.blast_crater {
                return; // "since these footprints are permanent, never remove them"
            }
            self.remove_unit_from_pathfind_map(host, id);
            if obj.walk_on_top_of_wall && !self.layers[LAYER_WALL as usize].is_unused() {
                let n = self.num_wall_pieces.max(0) as usize;
                for w in &mut self.wall_pieces[..n] {
                    if *w == id {
                        *w = INVALID_ID;
                    }
                }
                for other in host.objects() {
                    let Some(o) = host.object(other) else { continue };
                    if o.layer == LAYER_WALL && self.layers[LAYER_WALL as usize].is_point_on_wall(&[id], &o.pos, host) && !plant(3061) {
                        host.splat(other);
                    }
                }
                let pieces = self.pieces();
                let ground = self.ground();
                self.layers[LAYER_WALL as usize].classify_wall_cells(&mut self.store, &ground, &pieces, host);
            }
        }
        if !obj.structure || obj.mobile || obj.small {
            return; // "Only path around structures." "mobile aren't obstacles."
        }
        if obj.height_above_terrain > PATHFIND_CELL_SIZE_F && !obj.blast_crater {
            return; // "Don't add bounds that are up in the air.... unless a blast crater wants to do just that"
        }
        self.internal_classify_object_footprint(&obj, insert, frame);
    }

    /// `internal_classifyObjectFootprint(obj, insert)`.
    pub fn internal_classify_object_footprint(&mut self, obj: &PfObjectFacts, insert: bool, frame: u32) {
        let mut bounds = Self::cell_bounds_at(&obj.pos);
        match obj.geom_type {
            GEOMETRY_BOX => {
                self.zone_manager.mark_zones_dirty(insert, frame);
                self.stepped_box(obj, obj.major_radius, obj.major_radius, obj.minor_radius, false, insert, &mut bounds);
            }
            GEOMETRY_SPHERE | GEOMETRY_CYLINDER => {
                // "not quite right, but close enough"
                self.zone_manager.mark_zones_dirty(insert, frame);
                let radius = obj.major_radius;
                let top_left = ICoord2D {
                    x: real_to_int_floor(0.5 + (obj.pos.x - radius) / PATHFIND_CELL_SIZE_F) - 1,
                    y: real_to_int_floor(0.5 + (obj.pos.y - radius) / PATHFIND_CELL_SIZE_F) - 1,
                };
                let mut size = radius / PATHFIND_CELL_SIZE_F;
                let center = Coord2D { x: obj.pos.x / PATHFIND_CELL_SIZE_F, y: obj.pos.y / PATHFIND_CELL_SIZE_F };
                size += if plant(3050) { 0.5 } else { 0.4 };
                let r2 = size * size;
                // (an Int plus Reals, truncated)
                let bottom_right = ICoord2D { x: (top_left.x as f32 + 2.0 * size + 2.0) as i32, y: (top_left.y as f32 + 2.0 * size + 2.0) as i32 };
                for j in top_left.y..bottom_right.y {
                    for i in top_left.x..bottom_right.x {
                        let dx = i as f32 + 0.5 - center.x;
                        let dy = j as f32 + 0.5 - center.y;
                        if dx * dx + dy * dy <= r2 && i >= 0 && j >= 0 && i < self.extent.hi.x && j < self.extent.hi.y {
                            self.footprint_cell(obj, false, insert, i, j);
                            bounds.lo.x = bounds.lo.x.min(i);
                            bounds.lo.y = bounds.lo.y.min(j);
                            bounds.hi.x = bounds.hi.x.max(i);
                            bounds.hi.y = bounds.hi.y.max(j);
                        }
                    }
                }
            }
            _ => {}
        }
        let ground = self.ground();
        let extent = self.extent;
        self.zone_manager.update_zones_for_modify(&mut self.store, &ground, &bounds, &extent);
        bounds.lo.x = (bounds.lo.x - 2).max(extent.lo.x);
        bounds.lo.y = (bounds.lo.y - 2).max(extent.lo.y);
        bounds.hi.x = (bounds.hi.x + 2).min(extent.hi.x);
        bounds.hi.y = (bounds.hi.y + 2).min(extent.hi.y);
        let cell = |i: i32, j: i32| ground.cell(i, j);
        if !insert && !plant(3054) {
            for j in bounds.lo.y..=bounds.hi.y {
                for i in bounds.lo.x..=bounds.hi.x {
                    if self.store.cells[cell(i, j) as usize].get_type() == CELL_IMPASSABLE {
                        self.store.set_type(cell(i, j), CELL_CLEAR);
                    }
                }
            }
        }
        let in_extent = |k: i32, l: i32| k >= extent.lo.x && k <= extent.hi.x && l >= extent.lo.y && l <= extent.hi.y;
        for j in bounds.lo.y..=bounds.hi.y {
            for i in bounds.lo.x..=bounds.hi.x {
                self.store.cells[cell(i, j) as usize].set_pinched(false);
                if self.store.cells[cell(i, j) as usize].get_type() != CELL_CLEAR {
                    continue;
                }
                let (mut total, mut orthogonal) = (0, 0);
                for k in i - 1..i + 2 {
                    for l in j - 1..j + 2 {
                        if !in_extent(k, l) || (k == i && l == j) {
                            continue;
                        }
                        if self.store.cells[cell(k, l) as usize].get_type() == CELL_CLEAR {
                            total += 1;
                            if k == i || l == j {
                                orthogonal += 1;
                            }
                        }
                    }
                }
                if orthogonal < 2 || total < if plant(3051) { 3 } else { 4 } {
                    self.store.cells[cell(i, j) as usize].set_pinched(true);
                }
            }
        }
        for j in bounds.lo.y..=bounds.hi.y {
            for i in bounds.lo.x..=bounds.hi.x {
                let c = cell(i, j);
                if self.store.cells[c as usize].get_pinched() && self.store.cells[c as usize].get_type() == CELL_CLEAR {
                    self.store.set_type(c, CELL_IMPASSABLE);
                    self.store.cells[c as usize].set_pinched(false);
                }
            }
        }
        // MARK_BORDER_PINCHED
        for j in bounds.lo.y..=bounds.hi.y {
            for i in bounds.lo.x..=bounds.hi.x {
                if self.store.cells[cell(i, j) as usize].get_type() != CELL_CLEAR {
                    continue;
                }
                let mut adjacent = false;
                'k: for k in i - 1..i + 2 {
                    for l in j - 1..j + 2 {
                        if !in_extent(k, l) || (k == i && l == j) || (k != i && l != j) {
                            continue;
                        }
                        if self.store.cells[cell(k, l) as usize].get_type() == CELL_OBSTACLE {
                            adjacent = true;
                            break 'k;
                        }
                    }
                }
                if adjacent {
                    self.store.cells[cell(i, j) as usize].set_pinched(true);
                }
            }
        }
    }

    /// `classifyMapCell(i, j, cell)`: clear, a cliff, or water (an obstacle stays one), unpinched, its info released.
    pub fn classify_map_cell(&mut self, host: &impl PathfinderHost, i: i32, j: i32, cell: CellId) {
        let has_obstacle = self.store.cells[cell as usize].get_type() == CELL_OBSTACLE;
        let (tl_x, tl_y) = (i as f32 * PATHFIND_CELL_SIZE_F, j as f32 * PATHFIND_CELL_SIZE_F);
        let (br_x, br_y) = (tl_x + PATHFIND_CELL_SIZE_F, tl_y + PATHFIND_CELL_SIZE_F);
        self.store.cells[cell as usize].set_pinched(false);
        let mut ty = CELL_CLEAR;
        if host.is_cliff_cell(tl_x, tl_y) {
            ty = CELL_CLIFF;
        }
        for (x, y) in [(tl_x, tl_y), (tl_x, br_y), (br_x, br_y), (br_x, tl_y)].into_iter().take(if plant(3053) { 1 } else { 4 }) {
            if host.is_underwater(x, y) {
                ty = CELL_WATER;
            }
        }
        if has_obstacle {
            ty = CELL_OBSTACLE;
        }
        self.store.set_type(cell, ty);
        self.store.release_info(cell);
    }

    /// `newMap()`: see the module's notes.
    pub fn new_map(&mut self, host: &mut impl PathfinderHost, frame: u32) {
        self.wall_height = host.wall_height(); // "may be updated by map.ini."
        let te = host.maximum_pathfind_extent();
        let mut bounds = IRegion2D {
            lo: ICoord2D { x: real_to_int_floor(te.lo.x / PATHFIND_CELL_SIZE_F), y: real_to_int_floor(te.lo.y / PATHFIND_CELL_SIZE_F) },
            hi: ICoord2D { x: real_to_int_floor(te.hi.x / PATHFIND_CELL_SIZE_F), y: real_to_int_floor(te.hi.y / PATHFIND_CELL_SIZE_F) },
        };
        bounds.hi.x -= 1;
        bounds.hi.y -= 1;
        let data_allocated = self.extent.hi.x == bounds.hi.x && self.extent.hi.y == bounds.hi.y && self.map_allocated;
        // "For map load from file, we have to call newMap twice to do sequencing issues."
        if !data_allocated {
            self.extent = bounds;
            self.zone_manager.allocate_blocks(&bounds);
            // the map's cells first in the store (a store with cells already is upstream's leak of the old map; reset first)
            assert!(self.store.cells.is_empty(), "Can't reallocate pathfind cells.");
            self.store.cells = vec![Default::default(); ((bounds.hi.x + 1) * (bounds.hi.y + 1)).max(0) as usize];
            self.map_allocated = true;
            let extent = self.extent;
            for i in 0..LAYER_LAST as usize {
                if !self.layers[i].is_unused() {
                    self.layers[i].allocate_cells(&mut self.store, &extent, host);
                }
            }
            if self.num_wall_pieces > 0 {
                self.layers[LAYER_WALL as usize].init(None, LAYER_WALL);
                let pieces = self.pieces();
                self.layers[LAYER_WALL as usize].allocate_cells_for_wall_layer(&mut self.store, &extent, &pieces, host);
            }
        }
        self.classify_map(host);
        // "Add existing objects."
        for id in host.objects() {
            self.classify_object_footprint(host, id, true, frame);
        }
        self.is_map_ready = true;
    }

    /// The clear cells next to a cell with the cliff BIT pinched.
    fn pinch_next_to_cliffs(&mut self) {
        let (e, ground) = (self.extent, self.ground());
        for j in e.lo.y..=e.hi.y {
            for i in e.lo.x..=e.hi.x {
                if self.store.cells[ground.cell(i, j) as usize].get_type() & CELL_CLIFF == 0 {
                    continue;
                }
                for k in i - 1..i + 2 {
                    if k < e.lo.x || k > e.hi.x {
                        continue;
                    }
                    for l in j - 1..j + 2 {
                        if l < e.lo.y || l > e.hi.y {
                            continue;
                        }
                        let c = &mut self.store.cells[ground.cell(k, l) as usize];
                        if c.get_type() == CELL_CLEAR {
                            c.set_pinched(true);
                        }
                    }
                }
            }
        }
    }

    /// `classifyMap()`: see the module's notes.
    pub fn classify_map(&mut self, host: &impl PathfinderHost) {
        let (e, ground) = (self.extent, self.ground());
        for j in e.lo.y..=e.hi.y {
            for i in e.lo.x..=e.hi.x {
                self.classify_map_cell(host, i, j, ground.cell(i, j));
            }
        }
        self.pinch_next_to_cliffs();
        for j in e.lo.y..=e.hi.y {
            for i in e.lo.x..=e.hi.x {
                let c = ground.cell(i, j);
                if self.store.cells[c as usize].get_pinched() && self.store.cells[c as usize].get_type() == CELL_CLEAR && !plant(3052) {
                    self.store.set_type(c, CELL_CLIFF);
                }
            }
        }
        self.pinch_next_to_cliffs();
        for i in 0..LAYER_LAST as usize {
            if !self.layers[i].is_unused() {
                self.layers[i].classify_cells(&mut self.store, &ground, host);
            }
        }
        if !self.layers[LAYER_WALL as usize].is_unused() {
            let pieces = self.pieces();
            self.layers[LAYER_WALL as usize].classify_wall_cells(&mut self.store, &ground, &pieces, host);
        }
        self.zone_manager.calculate_zones(&mut self.store, &ground, &mut self.layers, &e);
    }

    /// `forceMapRecalculation()`.
    pub fn force_map_recalculation(&mut self, host: &impl PathfinderHost) {
        self.classify_map(host);
    }

    /// `getRadiusAndCenter(obj, iRadius, center)`: see the module's notes; no object is radius 0, centred.
    pub fn get_radius_and_center(obj: Option<&PfObjectFacts>) -> (i32, bool) {
        const MAX_RADIUS: i32 = 2;
        let Some(obj) = obj else { return (0, true) };
        let mut diameter = 2.0 * obj.bounding_circle_radius;
        if diameter > PATHFIND_CELL_SIZE_F && diameter < 2.0 * PATHFIND_CELL_SIZE_F {
            diameter = 2.0 * PATHFIND_CELL_SIZE_F;
        }
        let mut radius = real_to_int_floor(diameter / PATHFIND_CELL_SIZE_F + if plant(3057) { 0.5 } else { 0.3 });
        let mut center = false;
        if radius == 0 {
            radius += 1;
        }
        if radius & 1 != 0 {
            center = true;
        }
        radius /= 2;
        if radius > MAX_RADIUS {
            radius = MAX_RADIUS;
            center = true;
        }
        (radius, center)
    }

    /// The cell a unit of that centring stands on at `pos`.
    fn unit_cell(pos: &Coord3D, center: bool) -> ICoord2D {
        if center {
            ICoord2D { x: real_to_int_floor(pos.x / PATHFIND_CELL_SIZE_F), y: real_to_int_floor(pos.y / PATHFIND_CELL_SIZE_F) }
        } else {
            ICoord2D { x: real_to_int_floor(0.5 + pos.x / PATHFIND_CELL_SIZE_F), y: real_to_int_floor(0.5 + pos.y / PATHFIND_CELL_SIZE_F) }
        }
    }

    /// `updateGoal(obj, newGoalPos, layer)`: a ground unit's goal cells (a flying one's are an aircraft's, unless it is an unmanned
    /// helicopter on the ground).
    pub fn update_goal(&mut self, host: &mut impl PathfinderHost, id: ObjectId, new_goal: &Coord3D, layer: PathfindLayer) {
        let Some(obj) = host.object(id) else { return };
        if obj.immobile {
            return;
        }
        let Some(ai) = obj.ai else { return }; // "only consider ai objects."
        if !ai.doing_ground_movement && !(obj.produced_at_helipad && obj.unmanned) {
            // ("exception:sniped choppers are on ground")
            self.update_aircraft_goal(host, id, new_goal);
            return;
        }
        let layer_changed = obj.destination_layer != layer;
        let (mut do_ground, mut do_layer) = (false, false);
        if layer == LAYER_GROUND {
            do_ground = true;
        } else {
            do_layer = true;
            if host.object_interacts_with_bridge_end(id, layer) {
                do_ground = true;
            }
        }
        let (radius, center) = Self::get_radius_and_center(Some(&obj));
        let above = radius + i32::from(center);
        let above = if plant(3055) { radius } else { above };
        let new_cell = Self::unit_cell(new_goal, center);
        if !layer_changed && new_cell == ai.goal_cell {
            return;
        }
        self.remove_goal(host, id);
        host.set_destination_layer(id, layer);
        host.set_pathfind_goal_cell(id, new_cell);
        for i in new_cell.x - radius..new_cell.x + above {
            for j in new_cell.y - radius..new_cell.y + above {
                let ndx = ICoord2D { x: i, y: j };
                if do_layer {
                    if let Some(c) = self.get_cell(layer, i, j) {
                        self.store.set_goal_unit(c, id, ndx);
                    }
                }
                if do_ground {
                    if let Some(c) = self.get_cell(LAYER_GROUND, i, j) {
                        self.store.set_goal_unit(c, id, ndx);
                    }
                }
            }
        }
    }

    /// `updateAircraftGoal(obj, newGoalPos)`: an aircraft that adjusts its destination marks its goal cells.
    pub fn update_aircraft_goal(&mut self, host: &mut impl PathfinderHost, id: ObjectId, new_goal: &Coord3D) {
        let Some(obj) = host.object(id) else { return };
        if obj.immobile {
            return;
        }
        self.remove_goal(host, id);
        let Some(obj) = host.object(id) else { return };
        let Some(ai) = obj.ai else { return };
        if ai.doing_ground_movement || !ai.aircraft_adjusts_destination {
            return; // "shouldn't really happen, but just in case."
        }
        let (radius, center) = Self::get_radius_and_center(Some(&obj));
        let above = radius + i32::from(center);
        let new_cell = Self::unit_cell(new_goal, center);
        if new_cell == ai.goal_cell {
            return;
        }
        host.set_pathfind_goal_cell(id, new_cell);
        for i in new_cell.x - radius..new_cell.x + above {
            for j in new_cell.y - radius..new_cell.y + above {
                if let Some(c) = self.get_cell(LAYER_GROUND, i, j) {
                    self.store.set_goal_aircraft(c, id, ICoord2D { x: i, y: j });
                }
            }
        }
    }

    /// `removeGoal(obj)`: the unit's goal cells (radius at least 1) cleared of it, on the ground and on its destination layer.
    pub fn remove_goal(&mut self, host: &mut impl PathfinderHost, id: ObjectId) {
        let Some(obj) = host.object(id) else { return };
        if obj.immobile {
            return;
        }
        let Some(ai) = obj.ai else { return };
        let goal = ai.goal_cell;
        let (mut radius, center) = Self::get_radius_and_center(Some(&obj));
        if radius == 0 {
            radius += 1;
        }
        let above = radius + i32::from(center);
        let none = ICoord2D { x: -1, y: -1 };
        if goal == none {
            return;
        }
        host.set_pathfind_goal_cell(id, none);
        if goal.x >= 0 && goal.y >= 0 {
            for i in goal.x - radius..goal.x + above {
                for j in goal.y - radius..goal.y + above {
                    let ndx = ICoord2D { x: i, y: j };
                    if let Some(c) = self.get_cell(LAYER_GROUND, i, j) {
                        if self.store.get_goal_unit(c) == id {
                            self.store.set_goal_unit(c, INVALID_ID, ndx);
                        }
                        if self.store.get_goal_aircraft(c) == id {
                            self.store.set_goal_aircraft(c, INVALID_ID, ndx);
                        }
                    }
                    if obj.destination_layer != LAYER_GROUND && !plant(3056) {
                        if let Some(c) = self.get_cell(obj.destination_layer, i, j) {
                            if self.store.get_goal_unit(c) == id {
                                self.store.set_goal_unit(c, INVALID_ID, ndx);
                            }
                        }
                    }
                }
            }
        }
    }

    /// The unit's position cells around `cur` cleared of it, on `layer` and the ground.
    fn clear_pos_cells(&mut self, id: ObjectId, cur: ICoord2D, radius: i32, above: i32, layer: PathfindLayer) {
        if cur.x < 0 || cur.y < 0 {
            return;
        }
        for i in cur.x - radius..cur.x + above {
            for j in cur.y - radius..cur.y + above {
                let ndx = ICoord2D { x: i, y: j };
                if let Some(c) = self.get_cell(layer, i, j) {
                    if self.store.get_pos_unit(c) == id {
                        self.store.set_pos_unit(c, INVALID_ID, ndx);
                    }
                }
                if layer != LAYER_GROUND && !plant(3058) {
                    if let Some(c) = self.get_cell(LAYER_GROUND, i, j) {
                        if self.store.get_pos_unit(c) == id {
                            self.store.set_pos_unit(c, INVALID_ID, ndx);
                        }
                    }
                }
            }
        }
    }

    /// `updatePos(obj, newPos)`: a ground unit's position cells moved (a flying one's removed); nothing before the map is ready.
    pub fn update_pos(&mut self, host: &mut impl PathfinderHost, id: ObjectId, new_pos: &Coord3D) {
        let Some(obj) = host.object(id) else { return };
        if obj.immobile || !self.is_map_ready {
            return;
        }
        let Some(ai) = obj.ai else { return };
        let cur = ai.cur_cell;
        if !ai.doing_ground_movement {
            if cur.x >= 0 && cur.y >= 0 {
                self.remove_pos(host, id);
            }
            return;
        }
        let (radius, center) = Self::get_radius_and_center(Some(&obj));
        let above = radius + i32::from(center);
        let new_cell = Self::unit_cell(new_pos, center);
        if new_cell == cur {
            return;
        }
        let layer = obj.layer;
        let (mut do_ground, mut do_layer) = (false, false);
        if layer == LAYER_GROUND {
            do_ground = true; // "just have to do ground"
        } else {
            do_layer = true; // "have to do the layer"
            if host.object_interacts_with_bridge_end(id, layer) {
                do_ground = true; // "In this case, have to both layer & ground, as they overlap here."
            }
        }
        host.set_cur_pathfind_cell(id, new_cell);
        self.clear_pos_cells(id, cur, radius, above, layer);
        for i in new_cell.x - radius..new_cell.x + above {
            for j in new_cell.y - radius..new_cell.y + above {
                let ndx = ICoord2D { x: i, y: j };
                if do_layer {
                    if let Some(c) = self.get_cell(layer, i, j) {
                        self.store.set_pos_unit(c, id, ndx);
                    }
                }
                if do_ground {
                    if let Some(c) = self.get_cell(LAYER_GROUND, i, j) {
                        self.store.set_pos_unit(c, id, ndx);
                    }
                }
            }
        }
    }

    /// `removePos(obj)`: the unit's position cells cleared of it; nothing before the map is ready.
    pub fn remove_pos(&mut self, host: &mut impl PathfinderHost, id: ObjectId) {
        let Some(obj) = host.object(id) else { return };
        if obj.immobile || !self.is_map_ready {
            return;
        }
        let Some(ai) = obj.ai else { return };
        let (radius, center) = Self::get_radius_and_center(Some(&obj));
        let above = radius + i32::from(center);
        host.set_cur_pathfind_cell(id, ICoord2D { x: -1, y: -1 });
        self.clear_pos_cells(id, ai.cur_cell, radius, above, obj.layer);
    }

    /// `removeUnitFromPathfindMap(obj)`.
    pub fn remove_unit_from_pathfind_map(&mut self, host: &mut impl PathfinderHost, id: ObjectId) {
        self.remove_pos(host, id);
        self.remove_goal(host, id);
    }
}
