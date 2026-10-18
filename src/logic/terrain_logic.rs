// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/TerrainLogic.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Map/TerrainLogic.cpp
//   (the device subclass, for the ground source `GroundSource::LogicHeightMap`:)
//   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameLogic/W3DTerrainLogic.h
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameLogic/W3DTerrainLogic.cpp
//   (and the polygon trigger members it uses:)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Map/PolygonTrigger.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Authors: Colin Day, April 2001 (TerrainLogic, W3DTerrainLogic);
//   John Ahlquist, November 2001 (PolygonTrigger)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class TerrainLogic` (with `Waypoint`, `Bridge`, `BridgeInfo`): the
//! logical terrain of a map. Phase 1, layer 6.
//!
//! # What it holds
//!
//! - **Waypoints** and the directed paths between them (`WaypointsList`
//!   chunk), found by name, id or path label.
//! - **Bridges**: footprint geometry, pathfind layer, damage state, height
//!   on the deck; `layerForDestination` and friends.
//! - **Water**: the water areas (polygon triggers flagged as water), the
//!   water grid, `isUnderwater`, `setWaterHeight`, and the table of water
//!   heights that change over time (`update`).
//! - **Ground height**: see [`GroundSource`].
//! - **Map boundaries** and the active one.
//!
//! # The two ground sources (the virtual methods)
//!
//! `TerrainLogic::getGroundHeight`, `getLayerHeight`, `getExtent`, ... are
//! virtual. The base class answers 0 (and `getExtent` is a `DEBUG_CRASH`
//! that writes nothing); the game uses `W3DTerrainLogic`, which reads the
//! logic height map and the stored boundaries. [`GroundSource`] selects:
//! [`GroundSource::Base`] is the base class, [`GroundSource::LogicHeightMap`]
//! is `W3DTerrainLogic` over the logic-owned
//! [`LogicHeightMap`](super::terrain_visual::LogicHeightMap). (No trait
//! object: one enum, dispatched in the methods.)
//!
//! # Cut points
//!
//! Every call that leaves `TerrainLogic.cpp` for a later layer or for the
//! client is one of these. The totals are from a read of the file
//! (`grep`, 2026-10-18).
//!
//! 1. **`TheTerrainVisual`** (32 calls). Writes (27) become
//!    [`TerrainVisualEvent`]s ([`TerrainLogic::take_events`]): `load` 1,
//!    `enableWaterGrid` 1, `setWaterGridHeightClamps` 1, `setWaterTransform`
//!    2 (angle form, matrix form), `setWaterGridResolution` 1,
//!    `setWaterAttenuationFactors` 1, `setRawMapHeight` 19 (18 in
//!    `flattenTerrain`, 1 in `createCraterInTerrain`; the porting graph
//!    says 20). Reads (5 sites) are answered from logic-owned state; see
//!    `logic::terrain_visual` for what is decided and what is not.
//! 2. **Objects** (`Object`, `ThingTemplate`, `ThingFactory`,
//!    `TerrainRoads`, `BridgeBehavior`, `BridgeTowerBehavior`, `Drawable`,
//!    `GameLogic`): [`TerrainWorld`]. Bridges spawn objects:
//!    `Bridge::Bridge(info, props, name)` creates the `GenericBridge` object
//!    (`TheThingFactory->findTemplate("GenericBridge")`, `newObject`,
//!    `setPosition`, `updateObjValuesFromMapProperties`, `setOrientation`)
//!    and looks the bridge template up (`TheTerrainRoads->findBridge`);
//!    `Bridge::Bridge(Object*)` looks up the template and the four tower
//!    templates and `createTower`s each (`newObject`, `setPosition`,
//!    `setOrientation`, `BridgeBehaviorInterface::setTower`,
//!    `BridgeTowerBehaviorInterface::setBridge/setTowerType`,
//!    `BodyModuleInterface::isIndestructible/setIndestructible`). Also:
//!    `findObjectByID`, `getFirstObject/getNextObject`, `Object::getLayer`,
//!    `getPosition`, `getGeometryInfo`, `attemptDamage`, `getBodyModule()->
//!    getDamageState`, `BridgeBehaviorInterface::isScaffoldPresent`,
//!    `getDrawable`, `TheGameLogic->getFrame/destroyObject`,
//!    `ThePartitionManager->iterateObjectsInRange`.
//! 3. **The pathfinder** (`TheAI->pathfinder()`): `addBridge`,
//!    `changeBridgeState`, `forceMapRecalculation`, `getWallHeight`,
//!    `isPointOnWall`. [`TerrainWorld`].
//! 4. **Map boundary change** (`setActiveBoundary`): `ThePartitionManager`
//!    (`processEntirePendingUndoShroudRevealQueue`, `storeFoggedCells`,
//!    `reset`, `init`, `restoreFoggedCells`), `TheGhostObjectManager`
//!    (`releasePartitionData`, `lockGhostObjects`, `restorePartitionData`),
//!    `Object::friend_prepareForMapBoundaryAdjust/friend_notifyOfNewMapBoundary`,
//!    `TheRadar->newMap`, `TheTacticalView->forceCameraConstraintRecalc`:
//!    [`TerrainWorld::before_map_boundary_change`] and
//!    [`TerrainWorld::after_map_boundary_change`].
//! 5. **`MapObject`** (the waypoint objects of the map's object list) and
//!    **`PolygonTrigger`** (the water areas): inputs. A [`WaypointMapObject`]
//!    stands for a `MapObject` with `isWaypoint()`, and a [`TriggerArea`] for
//!    the part of a `PolygonTrigger` this file touches. `PolygonTrigger` and
//!    `MapObject` are translated separately; when they land, these two are
//!    to be replaced by them.
//! 6. **The file system** (`CachedFileInputStream::open`): `loadMap` takes
//!    the file's bytes.
//! 7. **`TheGlobalData`**: `enableWaterGrid` takes the [`GlobalData`].
//! 8. **Not translated** (no logic reaches them here): `Bridge::createTower`'s
//!    object work beyond the call, the `W3DTerrainLogic::isClearLineOfSight`
//!    and `isCliffCell` (they read the render object: [`TerrainLogicError::
//!    NotTranslated`]), the time-of-day line of `W3DTerrainLogic::loadMap`,
//!    and `TheTerrainRenderObject->loadRoadsAndBridges` (the client reads
//!    the road/bridge INI and calls `addBridgeToLogic`; here the embedder
//!    does).
//!
//! # Translation choices
//!
//! - **Lists are `Vec`s in list order**: upstream links `Waypoint`s and
//!   `Bridge`s through `m_pNext`, adding at the head. Here the
//!   waypoints are stored in insertion order and walked backwards, and the
//!   bridges are stored head-first. A [`WaypointHandle`] is the position in
//!   insertion order; a bridge is named by its position in the list
//!   (`bridge_index`), valid until the list changes.
//! - **A water handle** is [`WaterHandle`]: the grid, or the index of a
//!   polygon trigger in [`TerrainLogic::triggers`].
//! - **Floats** keep upstream's operation order; the functions upstream
//!   calls on `float` arguments through the C `double` overloads (`sqrt`)
//!   are evaluated in `double` and rounded once.
//! - `m_mapData`, `m_mapDX`, `m_mapDY` are declared by `TerrainLogic.h` and
//!   never used by `TerrainLogic.cpp`; only the device subclass sets the
//!   latter two (from the height map). `m_mapData` is not held.
//!
//! # Upstream quirks kept
//!
//! - `update` removes a finished water change with a loop that copies entry
//!   `j` over entry `i` for every `j >= i` (it does not shift): the entry
//!   that ends at `i` is the last one, and the ones between stay.
//! - `Waypoint::getLink(ndx)` accepts `ndx == MAX_LINKS` (8), reading past
//!   the array. That is undefined; here it is an error.
//! - `Bridge::isCellOnSide` is documented "on the end" and `isCellOnEnd`
//!   "on the sides" (swapped comments); the code is translated, not the
//!   comments.
//! - `findFarthestEdgePoint` compares `x` with `width() / 2`, not with the
//!   centre of the extent.
//! - `flattenTerrain` and `createCraterInTerrain` loop `j` from 0, not from
//!   `iMin.y`.
//! - `objectInteractsWithBridgeEnd` returns `NULL` (false) for the ground
//!   layer.
//! - `getWaterHandle` and `isUnderwater` of a grid handle ignore whether the
//!   point is on the grid when it is asked for the height (`wZ` stays 0).
//! - `Bridge::Bridge(Object*)` keeps the tower `offset` of the previous
//!   tower when a tower template is missing.
//!
//! # Where upstream is undefined
//!
//! [`TerrainUndefined`] and [`TerrainLogic::undefined_inputs`], which counts
//! every one refused: the base-class `getExtent` (never written, then read);
//! a bridge segment parallel to the bridge plane (`t` unwritten);
//! `getWaterTransform` with no water object; a grid height read from a mesh
//! never allocated; NaN water-grid coordinates; a polygon trigger with no
//! points; an out-of-range `m_numWaterToUpdate`, active boundary or
//! waypoint link index; a `Real` that does not fit its `Int`. The
//! chunk reader's own undefined inputs pass through as
//! [`TerrainLogicError::Chunk`].
//!
//! # Verification
//!
//! `tests/differential_terrain_logic.rs` compiles the original
//! `TerrainLogic.cpp` (see the test's module docs for the method, the
//! stand-ins and the results).

use std::cmp::Ordering;

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::{
    real_to_int_floor, Coord2D, Coord3D, ICoord2D, ICoord3D, PI, Region2D, Region3D,
};
use crate::common::dict::Dict;
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::{
    DrawableId, ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND, LAYER_WALL,
};
use crate::common::geometry::{GeometryInfo, GeometryType};
use crate::common::global_data::GlobalData;
use crate::common::name_key_generator::NameKeyType;
use crate::common::trig;
use crate::common::xfer::{Xfer, XferMode, XferStatus};
use crate::formats::data_chunk::{CachedFileInputStream, ChunkError, ChunkResult, ChunkUndefined, DataChunkInfo, DataChunkInput};
use crate::logic::damage::{DamageInfo, DEATH_NORMAL, DAMAGE_FALLING, DAMAGE_WATER, HUGE_DAMAGE_AMOUNT};
use crate::logic::module::body_module::{BodyDamageType, BODY_PRISTINE, BODY_RUBBLE};
use crate::logic::terrain_visual::{
    LogicHeightMap, TerrainUndefined, TerrainVisualEvent, TerrainVisualState, WaterGridSample, K_MAX_HEIGHT,
    MAP_HEIGHT_SCALE, MAP_XY_FACTOR,
};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::plane::PlaneClass;
use crate::ww::tri::point_in_triangle_2d;
use crate::ww::vector3::Vector3;

/// `INVALID_WAYPOINT_ID`.
pub const INVALID_WAYPOINT_ID: i32 = 0x7FFF_FFFF;
/// `Waypoint::MAX_LINKS`.
pub const MAX_LINKS: usize = 8;
/// `TerrainLogic::MAX_DYNAMIC_WATER`.
pub const MAX_DYNAMIC_WATER: usize = 64;
/// `PATHFIND_CELL_SIZE_F` (`AIPathfind.h`).
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// `LAYER_Z_CLOSE_ENOUGH_F` (`AIPathfind.h`).
pub const LAYER_Z_CLOSE_ENOUGH_F: f32 = 10.0;
/// `WATER_GRID` (`Scripts.h`): the name the scripts give the grid water.
pub const WATER_GRID: &[u8] = b"Water Grid";
/// `DEATH_SPLATTED` (`Damage.h`): index 11 of `TheDeathNames`.
pub const DEATH_SPLATTED: i32 = 11;
/// `BRIDGE_MAX_TOWERS` (`TerrainRoads.h`).
pub const BRIDGE_MAX_TOWERS: usize = 4;
/// The special waypoint whose presence switches the water grid on
/// (`newMap`).
pub const WAVE_GUIDE_WAYPOINT: &[u8] = b"WaveGuide1";

/// `enum BridgeTowerType` (`TerrainRoads.h`), without `BRIDGE_MAX_TOWERS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BridgeTowerType {
    /// `BRIDGE_TOWER_FROM_LEFT`.
    FromLeft = 0,
    /// `BRIDGE_TOWER_FROM_RIGHT`.
    FromRight = 1,
    /// `BRIDGE_TOWER_TO_LEFT`.
    ToLeft = 2,
    /// `BRIDGE_TOWER_TO_RIGHT`.
    ToRight = 3,
}

impl BridgeTowerType {
    /// The four, in enum order.
    pub const ALL: [BridgeTowerType; BRIDGE_MAX_TOWERS] =
        [BridgeTowerType::FromLeft, BridgeTowerType::FromRight, BridgeTowerType::ToLeft, BridgeTowerType::ToRight];
}

/// Why a [`TerrainLogic`] call failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainLogicError {
    /// An input upstream does not define (see the module docs); counted by
    /// [`TerrainLogic::undefined_inputs`].
    Undefined(TerrainUndefined),
    /// The chunk reader met an input it does not define.
    Chunk(ChunkUndefined),
    /// `throw SC_INVALID_DATA` (`xfer`).
    InvalidData,
    /// A `throw` of an `XferStatus` from the xfer.
    Xfer(XferStatus),
    /// Behaviour of a later layer (the render object) that is not
    /// translated; not an upstream-undefined input.
    NotTranslated(&'static str),
}

impl TerrainLogicError {
    /// Whether this is one of the inputs the C++ does not define (and the
    /// differential test therefore does not compare).
    pub fn is_undefined_upstream(&self) -> bool {
        matches!(self, TerrainLogicError::Undefined(_) | TerrainLogicError::Chunk(_))
    }
}

impl std::fmt::Display for TerrainLogicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TerrainLogicError {}

impl From<XferStatus> for TerrainLogicError {
    fn from(s: XferStatus) -> Self {
        TerrainLogicError::Xfer(s)
    }
}

/// Result of a [`TerrainLogic`] call.
pub type TlResult<T> = Result<T, TerrainLogicError>;

/// Which implementation of the virtual ground queries answers (see the
/// module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroundSource {
    /// `TerrainLogic`'s own: height 0, normal zero, `getExtent` unimplemented.
    Base,
    /// `W3DTerrainLogic`: the logic height map and the loaded boundaries.
    LogicHeightMap,
}

/// A `WaterHandle`: an instance of water, whatever its implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaterHandle {
    /// `&TerrainLogic::m_gridWaterHandle` (`m_polygon == NULL`).
    Grid,
    /// A polygon trigger's handle: the index in [`TerrainLogic::triggers`].
    Polygon(usize),
}

/// The part of a `PolygonTrigger` `TerrainLogic.cpp` uses: its name, id,
/// water flag and points. A trigger always has at least two points
/// (`ParsePolygonTriggersDataChunk` deletes the others).
#[derive(Clone, Debug, PartialEq)]
pub struct TriggerArea {
    name: AsciiString,
    id: i32,
    water_area: bool,
    points: Vec<ICoord3D>,
}

impl TriggerArea {
    /// A trigger area. Fewer than one point is undefined upstream
    /// (`getPoint(0)` indexes before the array).
    pub fn new(name: AsciiString, id: i32, water_area: bool, points: Vec<ICoord3D>) -> Result<Self, TerrainUndefined> {
        if points.is_empty() {
            return Err(TerrainUndefined::TriggerWithoutPoints);
        }
        Ok(TriggerArea { name, id, water_area, points })
    }

    /// `getTriggerName()`.
    pub fn name(&self) -> &AsciiString {
        &self.name
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `isWaterArea()`.
    pub fn is_water_area(&self) -> bool {
        self.water_area
    }

    /// `getNumPoints()`.
    pub fn num_points(&self) -> i32 {
        self.points.len() as i32
    }

    /// `getPoint(ndx)`: the index is clamped into the array.
    pub fn point(&self, mut ndx: i32) -> &ICoord3D {
        if ndx < 0 {
            ndx = 0;
        }
        if ndx >= self.num_points() {
            ndx = self.num_points() - 1;
        }
        &self.points[ndx as usize]
    }

    /// `setPoint(point, ndx)` for an index inside the polygon.
    fn set_point(&mut self, point: ICoord3D, ndx: usize) {
        self.points[ndx] = point;
    }

    /// `pointInTrigger(point)`: the bounding box, then the even-odd ray
    /// test. (`updateBounds` caches the box; the cache is invalidated by
    /// every change of a point, so computing it each time gives the same
    /// answers.)
    pub fn point_in_trigger(&self, point: &ICoord3D) -> bool {
        const BIG_INT: i32 = 0x7ffff0;
        let (mut lo_x, mut lo_y, mut hi_x, mut hi_y) = (BIG_INT, BIG_INT, -BIG_INT, -BIG_INT);
        for p in &self.points {
            if p.x < lo_x {
                lo_x = p.x;
            }
            if p.y < lo_y {
                lo_y = p.y;
            }
            if p.x > hi_x {
                hi_x = p.x;
            }
            if p.y > hi_y {
                hi_y = p.y;
            }
        }
        if point.x < lo_x || point.y < lo_y || point.x > hi_x || point.y > hi_y {
            return false;
        }
        let mut inside = false;
        let n = self.points.len();
        for i in 0..n {
            let pt1 = self.points[i];
            let pt2 = if i == n - 1 { self.points[0] } else { self.points[i + 1] };
            if pt1.y == pt2.y {
                continue; // ignore horizontal lines.
            }
            if pt1.y < point.y && pt2.y < point.y {
                continue;
            }
            if pt1.y >= point.y && pt2.y >= point.y {
                continue;
            }
            if pt1.x < point.x && pt2.x < point.x {
                continue;
            }
            // Line segment crosses ray from point x->infinity.
            let dy = pt2.y.wrapping_sub(pt1.y);
            let dx = pt2.x.wrapping_sub(pt1.x);
            let intersection_x = pt1.x as f32 + (dx.wrapping_mul(point.y.wrapping_sub(pt1.y)) as f32) / (dy as f32);
            if intersection_x >= point.x as f32 {
                inside = !inside;
            }
        }
        inside
    }
}

/// A `MapObject` with `isWaypoint()`, as `TerrainLogic::loadMap` reads it.
#[derive(Clone, Debug)]
pub struct WaypointMapObject {
    /// `isWaypoint()`: only these become waypoints.
    pub is_waypoint: bool,
    /// `getLocation()`.
    pub location: Coord3D,
    /// `getProperties()`.
    pub properties: Dict,
}

/// The name keys `addWaypoint` reads from a map object's properties
/// (`TheKey_waypointID`, `TheKey_waypointName`, `TheKey_waypointPathLabel1..3`,
/// `TheKey_waypointPathBiDirectional`).
#[derive(Clone, Copy, Debug)]
pub struct WaypointKeys {
    /// `TheKey_waypointID`.
    pub id: NameKeyType,
    /// `TheKey_waypointName`.
    pub name: NameKeyType,
    /// `TheKey_waypointPathLabel1`.
    pub label1: NameKeyType,
    /// `TheKey_waypointPathLabel2`.
    pub label2: NameKeyType,
    /// `TheKey_waypointPathLabel3`.
    pub label3: NameKeyType,
    /// `TheKey_waypointPathBiDirectional`.
    pub bi_directional: NameKeyType,
}

/// A position in [`TerrainLogic`]'s waypoint storage (insertion order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WaypointHandle(pub usize);

/// `class Waypoint`.
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    id: i32,
    name: AsciiString,
    location: Coord3D,
    links: [Option<WaypointHandle>; MAX_LINKS],
    num_links: i32,
    path_label1: AsciiString,
    path_label2: AsciiString,
    path_label3: AsciiString,
    bi_directional: bool,
}

impl Waypoint {
    /// `Waypoint(id, name, pLoc, label1, label2, label3, biDirectional)`.
    pub fn new(id: i32, name: AsciiString, loc: Coord3D, label1: AsciiString, label2: AsciiString, label3: AsciiString, bi_directional: bool) -> Self {
        Waypoint {
            id,
            name,
            location: loc,
            links: [None; MAX_LINKS],
            num_links: 0,
            path_label1: label1,
            path_label2: label2,
            path_label3: label3,
            bi_directional,
        }
    }

    /// `addLink(pLink)`: ignored when the waypoint has `MAX_LINKS` already.
    fn add_link(&mut self, link: WaypointHandle) {
        if (self.num_links as usize) < MAX_LINKS {
            self.links[self.num_links as usize] = Some(link);
            self.num_links += 1;
        }
    }

    /// `getNumLinks()`.
    pub fn num_links(&self) -> i32 {
        self.num_links
    }

    /// `getLink(ndx)`: `None` for a negative index or an unused link.
    /// `ndx == MAX_LINKS` reads past the array upstream (undefined).
    pub fn link(&self, ndx: i32) -> Result<Option<WaypointHandle>, TerrainUndefined> {
        if ndx < 0 {
            return Ok(None);
        }
        if ndx as usize == MAX_LINKS {
            return Err(TerrainUndefined::WaypointLinkIndex);
        }
        if ndx as usize > MAX_LINKS {
            return Ok(None);
        }
        Ok(self.links[ndx as usize])
    }

    /// `getName()`.
    pub fn name(&self) -> &AsciiString {
        &self.name
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getLocation()`.
    pub fn location(&self) -> &Coord3D {
        &self.location
    }

    /// `getPathLabel1()`.
    pub fn path_label1(&self) -> &AsciiString {
        &self.path_label1
    }

    /// `getPathLabel2()`.
    pub fn path_label2(&self) -> &AsciiString {
        &self.path_label2
    }

    /// `getPathLabel3()`.
    pub fn path_label3(&self) -> &AsciiString {
        &self.path_label3
    }

    /// `getBiDirectional()`.
    pub fn bi_directional(&self) -> bool {
        self.bi_directional
    }
}

/// `class BridgeInfo`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BridgeInfo {
    /// `from`: the points the bridge was drawn using.
    pub from: Coord3D,
    /// `to`.
    pub to: Coord3D,
    /// `bridgeWidth`.
    pub bridge_width: f32,
    /// `fromLeft`: the four corners of the rectangle the bridge covers.
    pub from_left: Coord3D,
    /// `fromRight`.
    pub from_right: Coord3D,
    /// `toLeft`.
    pub to_left: Coord3D,
    /// `toRight`.
    pub to_right: Coord3D,
    /// `bridgeIndex`: the index to the drawable bridges.
    pub bridge_index: i32,
    /// `curDamageState`.
    pub cur_damage_state: BodyDamageType,
    /// `bridgeObjectID`.
    pub bridge_object_id: ObjectId,
    /// `towerObjectID[BRIDGE_MAX_TOWERS]`.
    pub tower_object_id: [ObjectId; BRIDGE_MAX_TOWERS],
    /// `damageStateChanged`.
    pub damage_state_changed: bool,
}

impl Default for BridgeInfo {
    /// `BridgeInfo()`.
    fn default() -> Self {
        BridgeInfo {
            from: Coord3D::default(),
            to: Coord3D::default(),
            bridge_width: 0.0,
            from_left: Coord3D::default(),
            from_right: Coord3D::default(),
            to_left: Coord3D::default(),
            to_right: Coord3D::default(),
            bridge_index: 0,
            cur_damage_state: BODY_PRISTINE,
            bridge_object_id: INVALID_ID,
            tower_object_id: [INVALID_ID; BRIDGE_MAX_TOWERS],
            damage_state_changed: false,
        }
    }
}

/// `struct TBridgeAttackInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BridgeAttackInfo {
    /// `attackPoint1`.
    pub attack_point1: Coord3D,
    /// `attackPoint2`.
    pub attack_point2: Coord3D,
}

fn vec3(c: &Coord3D) -> Vector3 {
    Vector3::new(c.x, c.y, c.z)
}

/// `LineInRegion(p1, p2, clipRegion)`: Cohen-Sutherland style clipping of a
/// line against a region; whether the (clipped) line is inside it.
pub fn line_in_region(p1: &Coord2D, p2: &Coord2D, clip_region: &Region2D) -> bool {
    const CLIP_LEFT: i32 = 0x01;
    const CLIP_RIGHT: i32 = 0x02;
    const CLIP_BOTTOM: i32 = 0x04;
    const CLIP_TOP: i32 = 0x08;

    // Use clip window that includes bottom right pixel
    let clip_left = clip_region.lo.x;
    let clip_right = clip_region.hi.x;
    let clip_top = clip_region.lo.y;
    let clip_bottom = clip_region.hi.y;

    let (mut x1, mut y1, mut x2, mut y2) = (p1.x, p1.y, p2.x, p2.y);
    let mut diff;

    // Test first point
    let mut clip_code1 = 0;
    if x1 < clip_left {
        clip_code1 = CLIP_LEFT;
    } else if x1 > clip_right {
        clip_code1 = CLIP_RIGHT;
    }
    if y1 < clip_top {
        clip_code1 |= CLIP_TOP;
    } else if y1 > clip_bottom {
        clip_code1 |= CLIP_BOTTOM;
    }

    // Test second point
    let mut clip_code2 = 0;
    if x2 < clip_left {
        clip_code2 = CLIP_LEFT;
    } else if x2 > clip_right {
        clip_code2 = CLIP_RIGHT;
    }
    if y2 < clip_top {
        clip_code2 |= CLIP_TOP;
    } else if y2 > clip_bottom {
        clip_code2 |= CLIP_BOTTOM;
    }

    // Both points inside window?
    if (clip_code1 | clip_code2) == 0 {
        return true;
    }

    // Both points outside window?
    if clip_code1 & clip_code2 != 0 {
        return false;
    }

    // First point outside window?
    if clip_code1 != 0 {
        if clip_code1 & CLIP_TOP != 0 {
            diff = y2 - y1;
            if diff == 0.0 {
                return false;
            }
            x1 += (x2 - x1) * (clip_top - y1) / diff;
            y1 = clip_top;
        } else if clip_code1 & CLIP_BOTTOM != 0 {
            diff = y2 - y1;
            if diff == 0.0 {
                return false;
            }
            x1 += (x2 - x1) * (clip_bottom - y1) / diff;
            y1 = clip_bottom;
        }

        if x1 > clip_right {
            diff = x2 - x1;
            if diff == 0.0 {
                return false;
            }
            y1 += (y2 - y1) * (clip_right - x1) / diff;
            x1 = clip_right;
        } else if x1 < clip_left {
            diff = x2 - x1;
            if diff == 0.0 {
                return false;
            }
            y1 += (y2 - y1) * (clip_left - x1) / diff;
            x1 = clip_left;
        }
    }

    // Second point outside window?
    if clip_code2 != 0 {
        if clip_code2 & CLIP_TOP != 0 {
            diff = y2 - y1;
            if diff == 0.0 {
                return false;
            }
            x2 += (x2 - x1) * (clip_top - y2) / diff;
            y2 = clip_top;
        } else if clip_code2 & CLIP_BOTTOM != 0 {
            diff = y2 - y1;
            if diff == 0.0 {
                return false;
            }
            x2 += (x2 - x1) * (clip_bottom - y2) / diff;
            y2 = clip_bottom;
        }

        if x2 > clip_right {
            diff = x2 - x1;
            if diff == 0.0 {
                return false;
            }
            y2 += (y2 - y1) * (clip_right - x2) / diff;
            x2 = clip_right;
        } else if x2 < clip_left {
            diff = x2 - x1;
            if diff == 0.0 {
                return false;
            }
            y2 += (y2 - y1) * (clip_left - x2) / diff;
            x2 = clip_left;
        }
    }

    // Line is visible
    x1 >= clip_left && x1 <= clip_right && y1 >= clip_top && y1 <= clip_bottom && x2 >= clip_left && x2 <= clip_right && y2 >= clip_top && y2 <= clip_bottom
}

/// `class Bridge`.
#[derive(Clone, Debug, PartialEq)]
pub struct Bridge {
    template_name: AsciiString,
    info: BridgeInfo,
    bounds: Region2D,
    layer: PathfindLayer,
}

impl Bridge {
    /// The bounds loop shared by both constructors: the 2D box of the four
    /// corners.
    fn compute_bounds(info: &BridgeInfo) -> Region2D {
        let mut b = Region2D::default();
        b.lo.x = info.from_left.x;
        b.lo.y = info.from_left.y;
        b.hi = b.lo;
        if b.lo.x > info.from_right.x {
            b.lo.x = info.from_right.x;
        }
        if b.lo.y > info.from_right.y {
            b.lo.y = info.from_right.y;
        }
        if b.hi.x < info.from_right.x {
            b.hi.x = info.from_right.x;
        }
        if b.hi.y < info.from_right.y {
            b.hi.y = info.from_right.y;
        }
        if b.lo.x > info.to_left.x {
            b.lo.x = info.to_left.x;
        }
        if b.lo.y > info.to_left.y {
            b.lo.y = info.to_left.y;
        }
        if b.hi.x < info.to_left.x {
            b.hi.x = info.to_left.x;
        }
        if b.hi.y < info.to_left.y {
            b.hi.y = info.to_left.y;
        }
        if b.lo.x > info.to_right.x {
            b.lo.x = info.to_right.x;
        }
        if b.lo.y > info.to_right.y {
            b.lo.y = info.to_right.y;
        }
        if b.hi.x < info.to_right.x {
            b.hi.x = info.to_right.x;
        }
        if b.hi.y < info.to_right.y {
            b.hi.y = info.to_right.y;
        }
        b
    }

    /// `Bridge(BridgeInfo &theInfo, Dict *props, bridgeTemplateName)`: a
    /// bridge drawn in the map. Creates the `GenericBridge` object through
    /// the world (cut point 2); when that template does not exist the
    /// bridge keeps the object id of `info`.
    pub fn from_map<W: TerrainWorld>(world: &mut W, info: &BridgeInfo, props: &Dict, bridge_template_name: &AsciiString) -> Bridge {
        let mut b = Bridge { template_name: bridge_template_name.clone(), info: *info, bounds: Region2D::default(), layer: 0 };
        b.bounds = Bridge::compute_bounds(&b.info);
        b.info.cur_damage_state = BODY_PRISTINE;

        let i = &b.info;
        let center = Coord3D {
            x: (i.from_left.x + i.to_right.x) / 2.0f32,
            y: (i.from_left.y + i.to_right.y) / 2.0f32,
            z: (i.from_left.z + i.to_right.z) / 2.0f32,
        };
        // we'll say the angle of this object representing the bridge is from
        // the 'from' side to the 'to' side.
        let v = Coord2D { x: i.to_left.x - i.from_left.x, y: i.to_left.y - i.from_left.y };
        let orientation = v.to_angle();
        match world.create_generic_bridge_object(&center, orientation, props) {
            None => return b, // "GenericBridge template not found"
            Some(id) => b.info.bridge_object_id = id,
        }
        // get the template of the bridge; upstream returns here whether or
        // not it is found (the tower code is compiled out: no_BRIDGE_TOWERS)
        let _ = world.find_bridge_road_template(bridge_template_name);
        b
    }

    /// `Bridge(Object *bridgeObj)`: a landmark bridge. Looks the bridge
    /// template and the four tower templates up and creates the towers.
    pub fn from_landmark<W: TerrainWorld>(world: &mut W, obj: &LandmarkBridgeObject) -> Bridge {
        let mut b = Bridge { template_name: obj.template_name.clone(), info: BridgeInfo::default(), bounds: Region2D::default(), layer: 0 };
        let pos = obj.position;
        let angle = obj.orientation;
        let halfsize_x = obj.geometry.get_major_radius();
        let halfsize_y = obj.geometry.get_minor_radius();
        b.info.bridge_width = 2.0f32 * halfsize_y;

        let c = trig::cos(angle);
        let s = trig::sin(angle);

        b.info.from_left.set(pos.x - halfsize_x * c - halfsize_y * s, pos.y + halfsize_y * c - halfsize_x * s, pos.z);
        b.info.to_left.set(pos.x + halfsize_x * c - halfsize_y * s, pos.y + halfsize_y * c + halfsize_x * s, pos.z);
        b.info.from_right.set(pos.x - halfsize_x * c + halfsize_y * s, pos.y - halfsize_y * c - halfsize_x * s, pos.z);
        b.info.to_right.set(pos.x + halfsize_x * c + halfsize_y * s, pos.y - halfsize_y * c + halfsize_x * s, pos.z);

        let i = &mut b.info;
        i.from.x = (i.from_left.x + i.from_right.x) / 2.0f32;
        i.from.y = (i.from_left.y + i.from_right.y) / 2.0f32;
        i.from.z = (i.from_left.z + i.from_right.z) / 2.0f32;

        i.to.x = (i.to_left.x + i.to_right.x) / 2.0f32;
        i.to.y = (i.to_left.y + i.to_right.y) / 2.0f32;
        i.to.z = (i.to_left.z + i.to_right.z) / 2.0f32;

        b.bounds = Bridge::compute_bounds(&b.info);
        b.info.cur_damage_state = BODY_PRISTINE;
        b.info.bridge_object_id = obj.id;

        // get the template of the bridge
        if !world.find_bridge_road_template(&obj.template_name) {
            return b;
        }

        let mut v = Coord2D { x: b.info.to_left.x - b.info.to_right.x, y: b.info.to_left.y - b.info.to_right.y };
        v.normalize();

        // initialize each of the tower positions to that of the bridge info bounding rect
        let tower_pos = [b.info.from_left, b.info.from_right, b.info.to_left, b.info.to_right];

        let mut offset = PATHFIND_CELL_SIZE_F / 2.0f32;
        // create objects targetable objects for the 4 tower pieces
        for (i, tower_type) in BridgeTowerType::ALL.iter().copied().enumerate() {
            let tower_template = world.find_bridge_tower_template(&obj.template_name, tower_type);
            if let Some(major_radius) = tower_template {
                offset = major_radius;
            }
            let mut pos = tower_pos[tower_type as usize];
            match tower_type {
                BridgeTowerType::FromLeft | BridgeTowerType::ToLeft => {
                    pos.x += v.x * offset;
                    pos.y += v.y * offset;
                }
                BridgeTowerType::FromRight | BridgeTowerType::ToRight => {
                    pos.x -= v.x * offset;
                    pos.y -= v.y * offset;
                }
            }
            // createTower: no template, no tower
            if tower_template.is_some() {
                let tower_angle = match tower_type {
                    BridgeTowerType::FromLeft | BridgeTowerType::FromRight => angle + PI,
                    BridgeTowerType::ToLeft | BridgeTowerType::ToRight => angle,
                };
                let id = world.create_bridge_tower(obj.id, tower_type, &pos, tower_angle);
                // store the tower object ID
                b.info.tower_object_id[i] = id;
            }
        }
        b
    }

    /// `getBridgeTemplateName()`.
    pub fn template_name(&self) -> &AsciiString {
        &self.template_name
    }

    /// `getBridgeInfo(pInfo)` / `peekBridgeInfo()`.
    pub fn info(&self) -> &BridgeInfo {
        &self.info
    }

    /// `getBounds()`.
    pub fn bounds(&self) -> &Region2D {
        &self.bounds
    }

    /// `getLayer()`.
    pub fn layer(&self) -> PathfindLayer {
        self.layer
    }

    /// `setBridgeObjectID(id)`.
    pub fn set_bridge_object_id(&mut self, id: ObjectId) {
        self.info.bridge_object_id = id;
    }

    /// `setTowerObjectID(id, which)`.
    pub fn set_tower_object_id(&mut self, id: ObjectId, which: BridgeTowerType) {
        self.info.tower_object_id[which as usize] = id;
    }

    /// `isPointOnBridge(pLoc)`.
    pub fn is_point_on_bridge(&self, p_loc: &Coord3D) -> bool {
        if p_loc.x < self.bounds.lo.x {
            return false;
        }
        if p_loc.x > self.bounds.hi.x {
            return false;
        }
        if p_loc.y < self.bounds.lo.y {
            return false;
        }
        if p_loc.y > self.bounds.hi.y {
            return false;
        }

        let test_pt = vec3(p_loc);
        let left1 = vec3(&self.info.from_left);
        let right1 = vec3(&self.info.from_right);
        let left2 = vec3(&self.info.to_left);
        let right2 = vec3(&self.info.to_right);

        let mut flags = 0u8;
        if point_in_triangle_2d(&left1, &right1, &left2, &test_pt, 0, 1, &mut flags) {
            return true;
        }
        if point_in_triangle_2d(&right1, &left2, &right2, &test_pt, 0, 1, &mut flags) {
            return true;
        }
        false
    }

    /// The vector from `fromLeft` to `fromRight`, normalised (shared by the
    /// three cell tests).
    fn end_vector(&self) -> Coord3D {
        let mut end_vector = Coord3D {
            x: self.info.from_right.x - self.info.from_left.x,
            y: self.info.from_right.y - self.info.from_left.y,
            z: self.info.from_right.z - self.info.from_left.z,
        };
        end_vector.normalize();
        end_vector
    }

    /// `isCellOnEnd(cell)` (upstream's comment calls it "on the sides").
    pub fn is_cell_on_end(&self, cell: &Region2D) -> bool {
        let mut end_vector = self.end_vector();
        // Offset by 1 pathfind cell.
        end_vector.x *= PATHFIND_CELL_SIZE_F;
        end_vector.y *= PATHFIND_CELL_SIZE_F;

        let mut from_left = self.info.from_left;
        from_left.x += end_vector.x;
        from_left.y += end_vector.y;

        let mut from_right = self.info.from_right;
        from_right.x -= end_vector.x;
        from_right.y -= end_vector.y;

        let mut to_left = self.info.to_left;
        to_left.x += end_vector.x;
        to_left.y += end_vector.y;

        let mut to_right = self.info.to_right;
        to_right.x -= end_vector.x;
        to_right.y -= end_vector.y;

        let line1 = Coord2D { x: from_left.x, y: from_left.y };
        let line2 = Coord2D { x: from_right.x, y: from_right.y };
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        let line1 = Coord2D { x: to_left.x, y: to_left.y };
        let line2 = Coord2D { x: to_right.x, y: to_right.y };
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        false
    }

    /// `isCellOnSide(cell)` (upstream's comment calls it "on the end").
    pub fn is_cell_on_side(&self, cell: &Region2D) -> bool {
        let mut end_vector = self.end_vector();
        // Offset by 1 pathfind cell.
        end_vector.x *= PATHFIND_CELL_SIZE_F * 0.51f32;
        end_vector.y *= PATHFIND_CELL_SIZE_F * 0.51f32;

        let mut from_left = self.info.from_left;
        from_left.x -= end_vector.x;
        from_left.y -= end_vector.y;

        let mut from_right = self.info.from_right;
        from_right.x += end_vector.x;
        from_right.y += end_vector.y;

        let mut to_left = self.info.to_left;
        to_left.x -= end_vector.x;
        to_left.y -= end_vector.y;

        let mut to_right = self.info.to_right;
        to_right.x += end_vector.x;
        to_right.y += end_vector.y;

        let line1 = Coord2D { x: from_left.x, y: from_left.y };
        let line2 = Coord2D { x: to_left.x, y: to_left.y };
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        let line1 = Coord2D { x: from_right.x, y: from_right.y };
        let line2 = Coord2D { x: to_right.x, y: to_right.y };
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        from_left.x -= end_vector.x;
        from_left.y -= end_vector.y;

        from_right.x += end_vector.x;
        from_right.y += end_vector.y;

        to_left.x -= end_vector.x;
        to_left.y -= end_vector.y;

        to_right.x += end_vector.x;
        to_right.y += end_vector.y;

        let line1 = Coord2D { x: from_left.x, y: from_left.y };
        let line2 = Coord2D { x: to_left.x, y: to_left.y };
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        let line1 = Coord2D { x: from_right.x, y: from_right.y };
        let line2 = Coord2D { x: to_right.x, y: to_right.y };
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        false
    }

    /// `isCellEntryPoint(cell)`.
    pub fn is_cell_entry_point(&self, cell: &Region2D) -> bool {
        let mut end_vector = self.end_vector();
        // Offset by 1 pathfind cell.
        end_vector.x *= PATHFIND_CELL_SIZE_F;
        end_vector.y *= PATHFIND_CELL_SIZE_F;
        let mut bridge_vector = Coord3D {
            x: self.info.to.x - self.info.from.x,
            y: self.info.to.y - self.info.from.y,
            z: self.info.to.z - self.info.from.z,
        };
        bridge_vector.normalize();
        // Offset by 1/2 pathfind cell (`PATHFIND_CELL_SIZE/2` is an integer
        // division: 5).
        bridge_vector.x *= 5.0f32;
        bridge_vector.y *= 5.0f32;

        let mut from_left = self.info.from_left;
        from_left.x -= bridge_vector.x;
        from_left.y -= bridge_vector.y;
        from_left.x += end_vector.x;
        from_left.y += end_vector.y;

        let mut from_right = self.info.from_right;
        from_right.x -= bridge_vector.x;
        from_right.y -= bridge_vector.y;
        from_right.x -= end_vector.x;
        from_right.y -= end_vector.y;

        let mut to_left = self.info.to_left;
        to_left.x += bridge_vector.x;
        to_left.y += bridge_vector.y;
        to_left.x += end_vector.x;
        to_left.y += end_vector.y;

        let mut to_right = self.info.to_right;
        to_right.x += bridge_vector.x;
        to_right.y += bridge_vector.y;
        to_right.x -= end_vector.x;
        to_right.y -= end_vector.y;

        let line1 = Coord2D { x: from_left.x, y: from_left.y };
        let line2 = Coord2D { x: from_right.x, y: from_right.y };
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        let line1 = Coord2D { x: to_left.x, y: to_left.y };
        let line2 = Coord2D { x: to_right.x, y: to_right.y };
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        false
    }

    /// The plane of the deck: through `fromLeft`, `fromRight`, `toLeft`.
    fn plane(&self) -> PlaneClass {
        PlaneClass::from_points(&vec3(&self.info.from_left), &vec3(&self.info.from_right), &vec3(&self.info.to_left))
    }

    /// `getBridgeHeight(pLoc, normal)`: the deck's height at the point's x
    /// and y, and the plane's normal. Assumes the point is on the bridge
    /// (see [`Bridge::is_point_on_bridge`]). A vertical deck is undefined.
    pub fn bridge_height(&self, p_loc: &Coord3D) -> Result<(f32, Coord3D), TerrainUndefined> {
        let plane = self.plane();
        const FACTOR: f32 = 1000.0;
        let bottom = Vector3::new(p_loc.x, p_loc.y, 0.0);
        let top = Vector3::new(p_loc.x, p_loc.y, FACTOR);
        let (_, t) = plane.compute_intersection(&bottom, &top);
        let t = t.ok_or(TerrainUndefined::BridgePlaneParallel)?;
        Ok((t * FACTOR, Coord3D { x: plane.n.x, y: plane.n.y, z: plane.n.z }))
    }

    /// `Bridge::pickBridge(from, to, &pos)`: where the segment meets the
    /// deck, if that is on the bridge; and the bridge object's drawable
    /// (cut point 2).
    fn pick<W: TerrainWorld>(&self, world: &W, from: &Vector3, to: &Vector3) -> Result<BridgePick, TerrainUndefined> {
        let plane = self.plane();
        let (_, t) = plane.compute_intersection(from, to);
        let t = t.ok_or(TerrainUndefined::BridgePlaneParallel)?;
        let intersect_pos = *from + (*to - *from) * t;

        let loc = Coord3D { x: intersect_pos.x, y: intersect_pos.y, z: intersect_pos.z };
        if self.is_point_on_bridge(&loc) {
            return Ok(BridgePick { pos: Some(intersect_pos), drawable: world.object_drawable(self.info.bridge_object_id) });
        }
        Ok(BridgePick { pos: None, drawable: None })
    }
}

/// What `Bridge::pickBridge` returns: `pos` is `Some` when it wrote the
/// position (the segment met the deck on the bridge).
#[derive(Clone, Copy, Debug, PartialEq)]
struct BridgePick {
    pos: Option<Vector3>,
    drawable: Option<DrawableId>,
}

/// What `TerrainLogic::pickBridge` returns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickedBridge {
    /// The drawable of the first bridge that was hit and has one.
    pub drawable: Option<DrawableId>,
    /// `*pos`. `None` is a value upstream leaves **uninitialised** (the
    /// last bridge the segment missed); `Some((0, 0, 0))` is the answer for a
    /// map with no bridges.
    pub pos: Option<Vector3>,
}

/// The facts about a landmark bridge object that `Bridge::Bridge(Object*)`
/// reads from it.
#[derive(Clone, Debug)]
pub struct LandmarkBridgeObject {
    /// `getID()`.
    pub id: ObjectId,
    /// `getTemplate()->getName()`.
    pub template_name: AsciiString,
    /// `getPosition()`.
    pub position: Coord3D,
    /// `getOrientation()`.
    pub orientation: f32,
    /// `getGeometryInfo()` (a box; upstream only logs when it is not).
    pub geometry: GeometryInfo,
}

/// The facts about an object that `TerrainLogic` reads.
#[derive(Clone, Debug)]
pub struct ObjectView {
    /// `getID()`.
    pub id: ObjectId,
    /// `getPosition()`.
    pub position: Coord3D,
    /// `getOrientation()`.
    pub orientation: f32,
    /// `getLayer()`.
    pub layer: PathfindLayer,
    /// `getGeometryInfo()`.
    pub geometry: GeometryInfo,
}

/// Cut points 2, 3 and 4: everything `TerrainLogic.cpp` asks of objects, the
/// game-logic clock, the partition manager and the pathfinder, in the order
/// and with the arguments upstream uses. The owner of the world implements
/// it (layer 8); the differential test implements it with a recorder.
pub trait TerrainWorld {
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `TheGameLogic->getFirstObject()` and `getNextObject()`, in order.
    fn objects(&self) -> Vec<ObjectView>;
    /// `ThePartitionManager->iterateObjectsInRange(center, range,
    /// FROM_CENTER_2D, NULL)`, in iteration order.
    fn objects_in_range_2d(&self, center: &Coord3D, range: f32) -> Vec<ObjectView>;
    /// `TheGameLogic->findObjectByID(id) != NULL`.
    fn object_exists(&self, id: ObjectId) -> bool;
    /// `findObjectByID(id)->getBodyModule()->getDamageState()`; asked only
    /// of an object that exists.
    fn object_damage_state(&self, id: ObjectId) -> BodyDamageType;
    /// `BridgeBehavior::getBridgeBehaviorInterfaceFromObject(obj)` and, if
    /// there is one, `isScaffoldPresent()`; `None` when there is no
    /// interface.
    fn bridge_scaffold_present(&self, id: ObjectId) -> Option<bool>;
    /// `findObjectByID(id)->getDrawable()`; `None` when there is no such
    /// object or it has no drawable.
    fn object_drawable(&self, id: ObjectId) -> Option<DrawableId>;
    /// `obj->attemptDamage(&damageInfo)`.
    fn attempt_damage(&mut self, id: ObjectId, info: &DamageInfo);
    /// `TheGameLogic->destroyObject(obj)`.
    fn destroy_object(&mut self, id: ObjectId);
    /// `TheAI->pathfinder()->addBridge(pBridge)`: the layer it assigns. The
    /// bridge is already first in the list.
    fn pathfinder_add_bridge(&mut self, bridge: &Bridge) -> PathfindLayer;
    /// `TheAI->pathfinder()->changeBridgeState(layer, enable)`.
    fn pathfinder_change_bridge_state(&mut self, layer: PathfindLayer, enable: bool);
    /// `TheAI->pathfinder()->forceMapRecalculation()`.
    fn pathfinder_force_map_recalculation(&mut self);
    /// `TheAI->pathfinder()->getWallHeight()`.
    fn pathfinder_wall_height(&self) -> f32;
    /// `TheAI->pathfinder()->isPointOnWall(pos)`.
    fn pathfinder_is_point_on_wall(&self, pos: &Coord3D) -> bool;
    /// The first constructor of `Bridge`: `TheThingFactory->findTemplate(
    /// "GenericBridge")` (`None` if there is none), then `newObject`,
    /// `setPosition(center)`, `updateObjValuesFromMapProperties(props)` and
    /// `setOrientation(orientation)`; the new object's id.
    fn create_generic_bridge_object(&mut self, center: &Coord3D, orientation: f32, props: &Dict) -> Option<ObjectId>;
    /// `TheTerrainRoads->findBridge(name) != NULL`.
    fn find_bridge_road_template(&self, name: &AsciiString) -> bool;
    /// `TheThingFactory->findTemplate(findBridge(bridge)->getTowerObjectName(
    /// tower))`: the tower template's `getTemplateGeometryInfo().
    /// getMajorRadius()`, or `None` if the template does not exist.
    fn find_bridge_tower_template(&self, bridge_template: &AsciiString, tower: BridgeTowerType) -> Option<f32>;
    /// `Bridge::createTower`'s object work for a tower whose template
    /// exists: `newObject(template, bridge->getTeam())`,
    /// `setPosition(pos)`, `setOrientation(angle)`, the behaviour
    /// interfaces, and the indestructible flag; the new object's id.
    fn create_bridge_tower(&mut self, bridge: ObjectId, tower: BridgeTowerType, pos: &Coord3D, angle: f32) -> ObjectId;
    /// `setActiveBoundary`, before the boundary index changes:
    /// `processEntirePendingUndoShroudRevealQueue()` and
    /// `storeFoggedCells(store, TRUE)`.
    fn before_map_boundary_change(&mut self);
    /// `setActiveBoundary`, after the boundary index changed: ghost object
    /// release, `friend_prepareForMapBoundaryAdjust` on every object,
    /// `storeFoggedCells(store, FALSE)`, the partition manager's `reset` and
    /// `init`, `TheRadar->newMap(TheTerrainLogic)` (which reads `terrain`),
    /// `restoreFoggedCells`, `friend_notifyOfNewMapBoundary` on every object,
    /// the ghost objects back, and
    /// `TheTacticalView->forceCameraConstraintRecalc()`.
    fn after_map_boundary_change(&mut self, terrain: &TerrainLogic);
}

/// `TerrainLogic::DynamicWaterEntry`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicWaterEntry {
    /// `waterTable`.
    pub water_table: Option<WaterHandle>,
    /// `changePerFrame`.
    pub change_per_frame: f32,
    /// `targetHeight`.
    pub target_height: f32,
    /// `damageAmount`.
    pub damage_amount: f32,
    /// `currentHeight`.
    pub current_height: f32,
}

impl Default for DynamicWaterEntry {
    fn default() -> Self {
        DynamicWaterEntry { water_table: None, change_per_frame: 0.0, target_height: 0.0, damage_amount: 0.0, current_height: 0.0 }
    }
}

/// What `isUnderwater` answers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Underwater {
    /// The return value.
    pub underwater: bool,
    /// `*waterZ`, written only when there is water at the point.
    pub water_z: Option<f32>,
    /// `*terrainZ`, written in both cases.
    pub terrain_z: f32,
}

/// `class TerrainLogic` (and, with [`GroundSource::LogicHeightMap`], the
/// `W3DTerrainLogic` it is used through).
#[derive(Clone, Debug)]
pub struct TerrainLogic {
    ground: GroundSource,
    /// `m_mapDX`, `m_mapDY` (set by the device subclass).
    map_dx: i32,
    map_dy: i32,
    /// `m_mapMinZ`, `m_mapMaxZ` (device subclass).
    map_min_z: f32,
    map_max_z: f32,
    /// `m_boundaries`.
    boundaries: Vec<ICoord2D>,
    /// `m_activeBoundary`.
    active_boundary: i32,
    /// `m_waypointListHead` and the `m_pNext` chain, in insertion order (the
    /// list is walked from the back).
    waypoints: Vec<Waypoint>,
    /// `m_bridgeListHead` and the `m_next` chain, head first.
    bridges: Vec<Bridge>,
    /// `m_bridgeDamageStatesChanged`.
    bridge_damage_states_changed: bool,
    /// `m_filenameString`.
    filename: AsciiString,
    /// `m_waterGridEnabled`.
    water_grid_enabled: bool,
    /// `m_waterToUpdate`.
    water_to_update: [DynamicWaterEntry; MAX_DYNAMIC_WATER],
    /// `m_numWaterToUpdate`.
    num_water_to_update: i32,
    /// `PolygonTrigger::ThePolygonTriggerListPtr`, head first.
    triggers: Vec<TriggerArea>,
    /// The logic-owned state of `TheTerrainVisual` (cut point 1).
    visual: TerrainVisualState,
    /// The events for the client, oldest first.
    events: Vec<TerrainVisualEvent>,
    /// How many undefined-upstream inputs were refused.
    undefined_inputs: u32,
}

impl Default for TerrainLogic {
    fn default() -> Self {
        TerrainLogic::new(GroundSource::LogicHeightMap)
    }
}

impl TerrainLogic {
    /// `TerrainLogic()` (and `W3DTerrainLogic()` for
    /// [`GroundSource::LogicHeightMap`]).
    pub fn new(ground: GroundSource) -> Self {
        TerrainLogic {
            ground,
            map_dx: 0,
            map_dy: 0,
            map_min_z: 0.0,
            map_max_z: 1.0,
            boundaries: Vec::new(),
            active_boundary: 0,
            waypoints: Vec::new(),
            bridges: Vec::new(),
            bridge_damage_states_changed: false,
            filename: AsciiString::new(),
            water_grid_enabled: false,
            water_to_update: [DynamicWaterEntry::default(); MAX_DYNAMIC_WATER],
            num_water_to_update: 0,
            triggers: Vec::new(),
            visual: TerrainVisualState::default(),
            events: Vec::new(),
            undefined_inputs: 0,
        }
    }

    /// The same, with the logic-owned terrain-visual state given (for a
    /// headless client: [`WaterGrid::without_render_object`](
    /// super::terrain_visual::WaterGrid::without_render_object)).
    pub fn with_visual(ground: GroundSource, visual: TerrainVisualState) -> Self {
        TerrainLogic { visual, ..TerrainLogic::new(ground) }
    }

    /// How many undefined-upstream inputs this object has refused.
    pub fn undefined_inputs(&self) -> u32 {
        self.undefined_inputs
    }

    fn undef(&mut self, e: TerrainUndefined) -> TerrainLogicError {
        self.undefined_inputs += 1;
        TerrainLogicError::Undefined(e)
    }

    fn count<T>(&mut self, r: Result<T, TerrainUndefined>) -> TlResult<T> {
        r.map_err(|e| self.undef(e))
    }

    /// The events for the client since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<TerrainVisualEvent> {
        std::mem::take(&mut self.events)
    }

    /// The events not yet taken.
    pub fn events(&self) -> &[TerrainVisualEvent] {
        &self.events
    }

    /// The logic-owned terrain-visual state (read-only).
    pub fn visual(&self) -> &TerrainVisualState {
        &self.visual
    }

    /// Mutable access, for the embedder that owns the water-mesh
    /// simulation or loads the height map.
    pub fn visual_mut(&mut self) -> &mut TerrainVisualState {
        &mut self.visual
    }

    /// Give the logic its height map (what `W3DTerrainVisual::load` makes
    /// of the map file).
    pub fn set_logic_height_map(&mut self, map: Option<LogicHeightMap>) {
        self.visual.height_map = map;
    }

    /// A write to the visual: apply it to the logic-owned state and queue
    /// the notification for the client.
    fn visual_write(&mut self, event: TerrainVisualEvent) -> TlResult<()> {
        let r = self.visual.apply(&event);
        self.count(r)?;
        self.events.push(event);
        Ok(())
    }

    /// `init()` (the device subclass also zeroes the map extent).
    pub fn init(&mut self) {
        if self.ground == GroundSource::LogicHeightMap {
            self.map_dx = 0;
            self.map_dy = 0;
            self.map_min_z = 0.0;
            self.map_max_z = 1.0;
        }
    }

    /// `reset()`: waypoints, bridges and polygon triggers are deleted
    /// (bridge objects are not), and the water change list emptied. The
    /// device subclass also zeroes the extent.
    pub fn reset(&mut self) {
        self.delete_waypoints();
        self.delete_bridges();
        self.triggers.clear(); // PolygonTrigger::deleteTriggers()
        self.num_water_to_update = 0;
        if self.ground == GroundSource::LogicHeightMap {
            self.map_dx = 0;
            self.map_dy = 0;
            self.map_min_z = 0.0;
            self.map_max_z = 1.0;
        }
    }

    // ---------------------------------------------------------------------
    // Polygon triggers (the list `PolygonTrigger` keeps globally upstream)
    // ---------------------------------------------------------------------

    /// The polygon triggers, in list order (`getFirstPolygonTrigger()`...).
    pub fn triggers(&self) -> &[TriggerArea] {
        &self.triggers
    }

    /// Replace the trigger list (what `ParsePolygonTriggersDataChunk`
    /// leaves).
    pub fn set_triggers(&mut self, triggers: Vec<TriggerArea>) {
        self.triggers = triggers;
    }

    /// `PolygonTrigger::addPolygonTrigger`: adds at the head.
    pub fn add_polygon_trigger(&mut self, trigger: TriggerArea) {
        self.triggers.insert(0, trigger);
    }

    /// `PolygonTrigger::getPolygonTriggerByID`: the first with that id.
    fn trigger_by_id(&self, id: i32) -> Option<usize> {
        self.triggers.iter().position(|t| t.id == id)
    }

    // ---------------------------------------------------------------------
    // Loading
    // ---------------------------------------------------------------------

    /// `TerrainLogic::loadMap(filename, query)`. `map_objects` is
    /// `MapObject::getFirstMapObject()`'s list in order, `keys` the name
    /// keys of the waypoint properties, and `file` the opened file
    /// (`None`: `open` failed, which upstream ignores). Returns what
    /// upstream returns: false for an empty file name or when the
    /// waypoint chunk parser fails; a chunk file that cannot be read at
    /// all (a thrown error) is eaten, as legacy files "don't have waypoint
    /// info".
    pub fn load_map(
        &mut self,
        filename: &AsciiString,
        query: bool,
        map_objects: &[WaypointMapObject],
        keys: &WaypointKeys,
        file: Option<CachedFileInputStream>,
    ) -> TlResult<bool> {
        // sanity
        if filename.is_empty() {
            return Ok(false);
        }

        // copy filename
        self.filename = filename.clone();

        // Add waypoint objects.
        for obj in map_objects {
            if obj.is_waypoint {
                self.add_waypoint(obj, keys);
            }
        }

        if let Some(stream) = file {
            // `try { ... } catch (...)`: only what upstream throws is eaten
            match DataChunkInput::<TerrainLogic>::new(stream) {
                Ok(mut input) => {
                    if input.is_valid_file_type() {
                        // Backwards compatible files aren't valid data chunk files.
                        input.register_parser(b"WaypointsList", b"", parse_waypoint_data_chunk);
                        match input.parse(self) {
                            Ok(true) => {}
                            Ok(false) => return Ok(false), // "Unable to read waypoint info."
                            Err(ChunkError::Thrown(_)) => {}
                            Err(ChunkError::Undefined(u)) => {
                                self.undefined_inputs += 1;
                                return Err(TerrainLogicError::Chunk(u));
                            }
                        }
                    }
                }
                Err(ChunkError::Thrown(_)) => {}
                Err(ChunkError::Undefined(u)) => {
                    self.undefined_inputs += 1;
                    return Err(TerrainLogicError::Chunk(u));
                }
            }
        }

        if !query {
            // tell the game interface a new terrain file has been loaded up
            self.visual_write(TerrainVisualEvent::Load { filename: self.filename.clone() })?;
        }

        Ok(true)
    }

    /// `W3DTerrainLogic::loadMap(filename, query)`: copies the map's size,
    /// boundaries and height range from the file's `HeightMapData` chunk
    /// (`WorldHeightMap(stream, true)`), then does what
    /// [`TerrainLogic::load_map`] does. `bytes` is the file; `None` is a
    /// file that does not open. (The time-of-day update at its end is
    /// client state and is not translated.)
    pub fn load_map_w3d(
        &mut self,
        filename: &AsciiString,
        query: bool,
        bytes: Option<&[u8]>,
        map_objects: &[WaypointMapObject],
        keys: &WaypointKeys,
    ) -> TlResult<bool> {
        // (an empty name does not open either)
        let Some(bytes) = bytes.filter(|_| !filename.is_empty()) else {
            return Ok(false);
        };
        let Some(stream) = CachedFileInputStream::new(bytes.to_vec()) else {
            return Ok(false);
        };
        let map = match LogicHeightMap::from_map_file(stream, true) {
            Ok(m) => m,
            Err(ChunkError::Thrown(_)) => return Ok(false),
            Err(ChunkError::Undefined(u)) => {
                self.undefined_inputs += 1;
                return Err(TerrainLogicError::Chunk(u));
            }
        };
        let Some(map) = map else {
            return Ok(false); // could not create heightmap object
        };
        // Get the whole map, because we don't know which boundary is active yet
        self.map_dx = map.x_extent();
        self.map_dy = map.y_extent();
        // now, get all the boudnaries, and set the current active boundary to boundary 0.
        self.boundaries = map.all_boundaries().to_vec();
        self.active_boundary = 0;

        let mut min_ht = K_MAX_HEIGHT;
        let mut max_ht = 0;
        for j in 0..self.map_dy {
            for i in 0..self.map_dx {
                let cur = i32::from(map.height_at(i, j));
                if cur < min_ht {
                    min_ht = cur;
                }
                if max_ht < cur {
                    max_ht = cur;
                }
            }
        }
        self.map_min_z = min_ht as f32 * MAP_HEIGHT_SCALE;
        self.map_max_z = max_ht as f32 * MAP_HEIGHT_SCALE;

        // Note - It is very important that this get called AFTER the map is read in.  jba.
        let stream = CachedFileInputStream::new(bytes.to_vec());
        self.load_map(filename, query, map_objects, keys, stream)
    }

    /// `addWaypoint(pMapObj)`: snaps the waypoint to the ground and puts it
    /// at the head of the list.
    fn add_waypoint(&mut self, obj: &WaypointMapObject, keys: &WaypointKeys) {
        let mut loc = obj.location;
        // Snap the waypoint down to the terrain. (A ground height that is
        // undefined, i.e. the base class's unwritten extent, cannot occur:
        // getGroundHeight never fails.)
        loc.z = self.ground_height(loc.x, loc.y, false).0;
        let (label1, _) = obj.properties.get_ascii_string(keys.label1);
        let (label2, _) = obj.properties.get_ascii_string(keys.label2);
        let (label3, _) = obj.properties.get_ascii_string(keys.label3);
        let (bi_directional, _) = obj.properties.get_bool(keys.bi_directional);
        // MapObject::getWaypointID / getWaypointName
        let (id, _) = obj.properties.get_int(keys.id);
        let (name, _) = obj.properties.get_ascii_string(keys.name);
        let ascii = |b: Vec<u8>| AsciiString::try_from_bytes(&b).unwrap_or_default();
        self.waypoints.push(Waypoint::new(id, ascii(name), loc, ascii(label1), ascii(label2), ascii(label3), bi_directional));
    }

    /// `addWaypointLink(id1, id2)`: the last waypoint in list order with
    /// each id is linked (duplicate ids are possible); links one way, and
    /// back as well when the first is bidirectional.
    fn add_waypoint_link(&mut self, id1: i32, id2: i32) {
        let mut way1: Option<usize> = None;
        let mut way2: Option<usize> = None;
        // Traverse all waypoints (list order: the back of the Vec first).
        for idx in (0..self.waypoints.len()).rev() {
            // ID's should be UnsignedInts (MSB)
            if self.waypoints[idx].id as u32 == id1 as u32 {
                way1 = Some(idx);
            }
            if self.waypoints[idx].id as u32 == id2 as u32 {
                way2 = Some(idx);
            }
        }
        if let (Some(w1), Some(w2)) = (way1, way2) {
            if w1 != w2 {
                for i in 0..self.waypoints[w1].num_links {
                    if self.waypoints[w1].links[i as usize] == Some(WaypointHandle(w2)) {
                        return; // already linked;
                    }
                }
                self.waypoints[w1].add_link(WaypointHandle(w2));

                if self.waypoints[w1].bi_directional {
                    // Link the other way.
                    for i in 0..self.waypoints[w2].num_links {
                        if self.waypoints[w2].links[i as usize] == Some(WaypointHandle(w1)) {
                            return; // already linked;
                        }
                    }
                    self.waypoints[w2].add_link(WaypointHandle(w1));
                }
            }
        }
    }

    /// `deleteWaypoints()`.
    fn delete_waypoints(&mut self) {
        self.waypoints.clear();
    }

    /// `deleteBridges()`: the bridges go, their objects stay.
    fn delete_bridges(&mut self) {
        self.bridges.clear();
    }

    /// `getSourceFilename()`.
    pub fn source_filename(&self) -> &AsciiString {
        &self.filename
    }

    // ---------------------------------------------------------------------
    // Waypoints
    // ---------------------------------------------------------------------

    /// `getFirstWaypoint()`.
    pub fn first_waypoint(&self) -> Option<WaypointHandle> {
        self.waypoints.len().checked_sub(1).map(WaypointHandle)
    }

    /// `Waypoint::getNext()`: the next waypoint in list order.
    pub fn next_waypoint(&self, w: WaypointHandle) -> Option<WaypointHandle> {
        w.0.checked_sub(1).map(WaypointHandle)
    }

    /// All waypoints in list order (`getFirstWaypoint()` then `getNext()`).
    pub fn waypoint_handles(&self) -> Vec<WaypointHandle> {
        (0..self.waypoints.len()).rev().map(WaypointHandle).collect()
    }

    /// A waypoint by handle.
    pub fn waypoint(&self, w: WaypointHandle) -> Option<&Waypoint> {
        self.waypoints.get(w.0)
    }

    /// `getWaypointByName(name)`.
    pub fn waypoint_by_name(&self, name: &AsciiString) -> Option<WaypointHandle> {
        self.waypoint_handles().into_iter().find(|&h| self.waypoints[h.0].name == *name)
    }

    /// `getWaypointByID(id)`.
    pub fn waypoint_by_id(&self, id: u32) -> Option<WaypointHandle> {
        self.waypoint_handles().into_iter().find(|&h| self.waypoints[h.0].id as u32 == id)
    }

    /// `getClosestWaypointOnPath(pos, label)`: the nearest (by squared
    /// distance in x and y) waypoint with a path label equal, ignoring
    /// case, to `label`; the first in list order wins a tie. An empty label
    /// finds nothing.
    pub fn closest_waypoint_on_path(&self, pos: &Coord3D, label: &AsciiString) -> Option<WaypointHandle> {
        let mut dist_sqr = 0.0f32;
        let mut p_closest_way: Option<WaypointHandle> = None;
        if label.is_empty() {
            return None;
        }

        for h in self.waypoint_handles() {
            let way = &self.waypoints[h.0];
            let mut matched = false;
            if label.compare_no_case(way.path_label1.str()) == Ordering::Equal {
                matched = true;
            }
            if label.compare_no_case(way.path_label2.str()) == Ordering::Equal {
                matched = true;
            }
            if label.compare_no_case(way.path_label3.str()) == Ordering::Equal {
                matched = true;
            }
            if matched {
                let cur_pos = way.location;
                let new_dist_sqr = (cur_pos.x - pos.x) * (cur_pos.x - pos.x) + (cur_pos.y - pos.y) * (cur_pos.y - pos.y);
                if p_closest_way.is_none() || new_dist_sqr < dist_sqr {
                    p_closest_way = Some(h);
                    dist_sqr = new_dist_sqr;
                }
            }
        }
        p_closest_way
    }

    /// `isPurposeOfPath(pWay, label)`: whether one of the waypoint's three
    /// path labels equals `label` (case-sensitively). A `NULL` waypoint or
    /// an empty label is false.
    pub fn is_purpose_of_path(&self, way: Option<WaypointHandle>, label: &AsciiString) -> bool {
        let Some(h) = way else {
            return false;
        };
        if label.is_empty() {
            return false;
        }
        let w = &self.waypoints[h.0];
        *label == w.path_label1 || *label == w.path_label2 || *label == w.path_label3
    }

    /// `getTriggerAreaByName(name)`: the index of the first trigger with the
    /// name.
    pub fn trigger_area_by_name(&self, name: &AsciiString) -> Option<usize> {
        self.triggers.iter().position(|t| t.name == *name)
    }

    // ---------------------------------------------------------------------
    // Ground
    // ---------------------------------------------------------------------

    /// `getGroundHeight(x, y, normal)`: the height, and the normal when
    /// asked for.
    pub fn ground_height(&self, x: f32, y: f32, want_normal: bool) -> (f32, Option<Coord3D>) {
        match self.ground {
            // default get height for terrain logic
            GroundSource::Base => (0.0, want_normal.then(Coord3D::default)),
            GroundSource::LogicHeightMap => match &self.visual.height_map {
                Some(map) => map.height_map_height(x, y, want_normal),
                None => (0.0, want_normal.then_some(Coord3D { x: 0.0, y: 0.0, z: 1.0 })),
            },
        }
    }

    /// `getLayerHeight(x, y, layer, normal, clip)`.
    pub fn layer_height<W: TerrainWorld>(&mut self, world: &W, x: f32, y: f32, layer: PathfindLayer, want_normal: bool, clip: bool) -> TlResult<(f32, Option<Coord3D>)> {
        match self.ground {
            GroundSource::Base => Ok((0.0, want_normal.then(Coord3D::default))),
            GroundSource::LogicHeightMap => {
                let Some(map) = &self.visual.height_map else {
                    return Ok((0.0, want_normal.then_some(Coord3D { x: 0.0, y: 0.0, z: 1.0 })));
                };
                let (height, mut normal) = map.height_map_height(x, y, want_normal);

                if layer != LAYER_GROUND {
                    let loc = Coord3D { x, y, z: height };
                    if layer == LAYER_WALL {
                        if !clip || world.pathfinder_is_point_on_wall(&loc) {
                            return Ok((world.pathfinder_wall_height(), normal));
                        } else {
                            return Ok((height, normal));
                        }
                    }
                    if let Some(bi) = self.find_bridge_layer_at_index(&loc, layer, clip) {
                        let r = self.bridges[bi].bridge_height(&loc);
                        let (bridge_height, bridge_normal) = self.count(r)?;
                        if want_normal {
                            normal = Some(bridge_normal);
                        }
                        if bridge_height > height {
                            return Ok((bridge_height, normal)); // Don't return bridge height if it's in the ground.
                        }
                    }
                }
                Ok((height, normal))
            }
        }
    }

    /// `getExtent(extent)`.
    pub fn extent(&self) -> Result<Region3D, TerrainUndefined> {
        match self.ground {
            GroundSource::Base => Err(TerrainUndefined::ExtentNotImplemented),
            GroundSource::LogicHeightMap => {
                let mut extent = Region3D::default();
                extent.lo.x = 0.0;
                extent.lo.y = 0.0;
                // Note - m_boundaries are stored in height map grids wide, so
                // we have to multiply by the grid width.
                if !self.boundaries.is_empty() {
                    let b = usize::try_from(self.active_boundary)
                        .ok()
                        .and_then(|i| self.boundaries.get(i))
                        .ok_or(TerrainUndefined::ActiveBoundary)?;
                    extent.hi.x = b.x as f32 * MAP_XY_FACTOR;
                    extent.hi.y = b.y as f32 * MAP_XY_FACTOR;
                } else {
                    extent.hi.x = 0.0;
                    extent.hi.y = 0.0;
                }
                // Note - m_mapMin and Max Z are already in real coord space.
                extent.lo.z = self.map_min_z;
                extent.hi.z = self.map_max_z;
                Ok(extent)
            }
        }
    }

    /// `getMaximumPathfindExtent(extent)` (the base class's is a
    /// `DEBUG_CRASH` stub, like `getExtent`).
    pub fn maximum_pathfind_extent(&self) -> Result<Region3D, TerrainUndefined> {
        match self.ground {
            GroundSource::Base => Err(TerrainUndefined::ExtentNotImplemented),
            GroundSource::LogicHeightMap => {
                let mut extent = Region3D::default();
                extent.lo.x = 0.0;
                extent.lo.y = 0.0;
                extent.hi.x = 0.0;
                extent.hi.y = 0.0;
                for b in &self.boundaries {
                    if extent.hi.x < b.x as f32 * MAP_XY_FACTOR {
                        extent.hi.x = b.x as f32 * MAP_XY_FACTOR;
                    }
                    if extent.hi.y < b.y as f32 * MAP_XY_FACTOR {
                        extent.hi.y = b.y as f32 * MAP_XY_FACTOR;
                    }
                }
                extent.lo.z = self.map_min_z;
                extent.hi.z = self.map_max_z;
                Ok(extent)
            }
        }
    }

    /// `getExtentIncludingBorder(extent)` (a base-class stub too). Needs the
    /// render object's map, which is the logic height map here. Upstream
    /// writes only `lo.x`, `lo.y`, `hi.x` and `hi.y`: the z of the caller's
    /// `Region3D` is left as it was, so the result is a [`Region2D`].
    pub fn extent_including_border(&self) -> Result<Region2D, TerrainUndefined> {
        match self.ground {
            GroundSource::Base => Err(TerrainUndefined::ExtentNotImplemented),
            GroundSource::LogicHeightMap => {
                let map = self.visual.height_map.as_ref().ok_or(TerrainUndefined::HeightMapShape)?;
                let mut extent = Region2D::default();
                extent.lo.x = 0.0;
                extent.lo.y = 0.0;
                let border = map.border_size() as f32 * MAP_XY_FACTOR;
                extent.lo.x -= border;
                extent.lo.y -= border;
                extent.hi.x = (self.map_dx as f32 * MAP_XY_FACTOR) - border;
                extent.hi.y = (self.map_dy as f32 * MAP_XY_FACTOR) - border;
                Ok(extent)
            }
        }
    }

    /// `isClearLineOfSight(pos, posOther)`: the base class has no
    /// implementation (`DEBUG_CRASH`, false); the device one asks the render
    /// object, which is not translated.
    pub fn is_clear_line_of_sight(&self, _pos: &Coord3D, _pos_other: &Coord3D) -> TlResult<bool> {
        match self.ground {
            GroundSource::Base => Ok(false),
            GroundSource::LogicHeightMap => Err(TerrainLogicError::NotTranslated("TheTerrainRenderObject->isClearLineOfSight")),
        }
    }

    /// `isCliffCell(x, y)`: false for the base class; the device one asks
    /// the render object, which is not translated.
    pub fn is_cliff_cell(&self, _x: f32, _y: f32) -> TlResult<bool> {
        match self.ground {
            GroundSource::Base => Ok(false),
            GroundSource::LogicHeightMap => Err(TerrainLogicError::NotTranslated("BaseHeightMapRenderObjClass::isCliffCell")),
        }
    }

    /// `findClosestEdgePoint(closestTo)`: the point on the map's edge
    /// nearest to the given one, at ground height.
    pub fn find_closest_edge_point(&mut self, closest_to: &Coord3D) -> TlResult<Coord3D> {
        let map_extent = self.extent();
        let map_extent = self.count(map_extent)?;

        let distances = [
            (closest_to.y - map_extent.lo.y).abs(), // top
            (closest_to.x - map_extent.hi.x).abs(), // right
            (closest_to.y - map_extent.hi.y).abs(), // bottom
            (closest_to.x - map_extent.lo.x).abs(), // left
        ];
        let mut best_distance = distances[0];
        let mut best_distance_index = 0;
        for (lame_index, &d) in distances.iter().enumerate().skip(1) {
            if d < best_distance {
                best_distance = d;
                best_distance_index = lame_index;
            }
        }

        let mut ret_val = *closest_to;
        match best_distance_index {
            0 => ret_val.y = map_extent.lo.y,
            1 => ret_val.x = map_extent.hi.x,
            2 => ret_val.y = map_extent.hi.y,
            _ => ret_val.x = map_extent.lo.x,
        }
        ret_val.z = self.ground_height(ret_val.x, ret_val.y, false).0;
        Ok(ret_val)
    }

    /// `findFarthestEdgePoint(farthestFrom)`.
    pub fn find_farthest_edge_point(&mut self, farthest_from: &Coord3D) -> TlResult<Coord3D> {
        let map_extent = self.extent();
        let map_extent = self.count(map_extent)?;

        let mut ret_val = *farthest_from;
        if farthest_from.x < (map_extent.width() / 2.0f32) {
            ret_val.x = map_extent.hi.x;
        } else {
            ret_val.x = map_extent.lo.x;
        }
        if farthest_from.y < (map_extent.height() / 2.0f32) {
            ret_val.y = map_extent.hi.y;
        } else {
            ret_val.y = map_extent.lo.y;
        }
        ret_val.z = self.ground_height(ret_val.x, ret_val.y, false).0;
        Ok(ret_val)
    }

    /// `alignOnTerrain(angle, pos, stickToGround, mtx)`: the layer and the
    /// matrix that stands an object on the terrain (or bridge) at `pos`.
    pub fn align_on_terrain<W: TerrainWorld>(&mut self, world: &W, angle: f32, pos: &Coord3D, stick_to_ground: bool) -> TlResult<(PathfindLayer, Matrix3D)> {
        let layer = self.layer_for_destination(world, pos)?;

        // get the normal of the terrain at our position
        let (mut terrain_at_pos, normal) = self.layer_height(world, pos.x, pos.y, layer, true, true)?;
        let terrain_normal = normal.unwrap_or_default();
        if layer != LAYER_GROUND {
            // brutal hack for bridges that are too high. jba
            terrain_at_pos += 2.5f32;
        }
        let mut mtx = make_align_to_normal_matrix(angle, pos, &terrain_normal);
        if stick_to_ground {
            mtx.set_z_translation(terrain_at_pos);
        }
        Ok((layer, mtx))
    }

    // ---------------------------------------------------------------------
    // Bridges
    // ---------------------------------------------------------------------

    /// The bridges in list order (`getFirstBridge()`, `getNext()`).
    pub fn bridges(&self) -> &[Bridge] {
        &self.bridges
    }

    /// `addBridgeToLogic(pInfo, props, bridgeTemplateName)`.
    pub fn add_bridge_to_logic<W: TerrainWorld>(&mut self, world: &mut W, info: &BridgeInfo, props: &Dict, bridge_template_name: &AsciiString) {
        let bridge = Bridge::from_map(world, info, props, bridge_template_name);
        self.bridges.insert(0, bridge);
        let layer = world.pathfinder_add_bridge(&self.bridges[0]);
        self.bridges[0].layer = layer;
    }

    /// `addLandmarkBridgeToLogic(bridgeObj)`.
    pub fn add_landmark_bridge_to_logic<W: TerrainWorld>(&mut self, world: &mut W, bridge_obj: &LandmarkBridgeObject) {
        let bridge = Bridge::from_landmark(world, bridge_obj);
        self.bridges.insert(0, bridge);
        let layer = world.pathfinder_add_bridge(&self.bridges[0]);
        self.bridges[0].layer = layer;
    }

    /// `Bridge::getBridgeHeight(pLoc, normal)` of the bridge at position
    /// `bridge_index`, counting an undefined input (a vertical deck).
    pub fn bridge_height(&mut self, bridge_index: usize, p_loc: &Coord3D) -> TlResult<(f32, Coord3D)> {
        let r = self.bridges[bridge_index].bridge_height(p_loc);
        self.count(r)
    }

    /// `Waypoint::getLink(ndx)` of a waypoint, counting an undefined input
    /// (`ndx == MAX_LINKS`).
    pub fn waypoint_link(&mut self, w: WaypointHandle, ndx: i32) -> TlResult<Option<WaypointHandle>> {
        let r = self.waypoints[w.0].link(ndx);
        self.count(r)
    }

    /// `findBridgeAt(pLoc)`: the position in the list of the first bridge
    /// the point is on.
    pub fn find_bridge_at(&self, p_loc: &Coord3D) -> Option<usize> {
        self.bridges.iter().position(|b| b.is_point_on_bridge(p_loc))
    }

    fn find_bridge_layer_at_index(&self, p_loc: &Coord3D, layer: PathfindLayer, clip: bool) -> Option<usize> {
        if layer == LAYER_GROUND {
            return None;
        }
        self.bridges.iter().position(|b| b.layer == layer && (!clip || b.is_point_on_bridge(p_loc)))
    }

    /// `findBridgeLayerAt(pLoc, layer, clip)`: the first bridge on that
    /// layer (and, when `clip`, under the point).
    pub fn find_bridge_layer_at(&self, p_loc: &Coord3D, layer: PathfindLayer, clip: bool) -> Option<usize> {
        self.find_bridge_layer_at_index(p_loc, layer, clip)
    }

    /// `getLayerForDestination(pos)`: the layer whose surface is closest to
    /// the position's height.
    pub fn layer_for_destination<W: TerrainWorld>(&mut self, world: &W, pos: &Coord3D) -> TlResult<PathfindLayer> {
        let mut best_layer = LAYER_GROUND;
        let mut best_distance = (pos.z - self.ground_height(pos.x, pos.y, false).0).abs();

        if best_distance > world.pathfinder_wall_height() / 2.0f32 {
            // check wall.
            if world.pathfinder_is_point_on_wall(pos) {
                let delta = (pos.z - world.pathfinder_wall_height()).abs();
                if delta < best_distance {
                    best_layer = LAYER_WALL;
                    best_distance = delta;
                }
            }
        }

        for i in 0..self.bridges.len() {
            if self.bridges[i].is_point_on_bridge(pos) {
                let r = self.bridges[i].bridge_height(pos);
                let (h, _) = self.count(r)?;
                let delta = (pos.z - h).abs();
                if delta < best_distance {
                    best_layer = self.bridges[i].layer;
                    best_distance = delta;
                }
            }
        }
        Ok(best_layer)
    }

    /// `getHighestLayerForDestination(pos, onlyHealthyBridges)`: like
    /// [`TerrainLogic::layer_for_destination`], but always the highest layer
    /// that is at or below the position.
    pub fn highest_layer_for_destination<W: TerrainWorld>(&mut self, world: &W, pos: &Coord3D, only_healthy_bridges: bool) -> TlResult<PathfindLayer> {
        let mut best_layer = LAYER_GROUND;
        let mut best_distance = pos.z - self.ground_height(pos.x, pos.y, false).0; // NOT fabs in this case.

        if best_distance > world.pathfinder_wall_height() / 2.0f32 {
            // check wall.
            if world.pathfinder_is_point_on_wall(pos) {
                let delta = pos.z - world.pathfinder_wall_height();
                // must be ABOVE (or on) the wall for this call. (srj)
                if delta >= 0.0 && delta.abs() < best_distance.abs() {
                    best_layer = LAYER_WALL;
                    best_distance = delta;
                }
            }
        }

        for i in 0..self.bridges.len() {
            if only_healthy_bridges && self.bridges[i].info.cur_damage_state == BODY_RUBBLE {
                continue;
            }
            if self.bridges[i].is_point_on_bridge(pos) {
                let r = self.bridges[i].bridge_height(pos);
                let (h, _) = self.count(r)?;
                let delta = pos.z - h;
                // must be ABOVE (or on) the bridge for this call. (srj)
                if delta >= 0.0 && delta.abs() < best_distance.abs() {
                    best_layer = self.bridges[i].layer;
                    best_distance = delta;
                }
            }
        }
        Ok(best_layer)
    }

    /// The 2D box `objectInteractsWithBridge*` builds around an object.
    fn interaction_bounds(obj: &ObjectView) -> Region2D {
        let mut radius = obj.geometry.get_minor_radius();
        radius += PATHFIND_CELL_SIZE_F / 2.0f32;
        let mut bounds = Region2D::default();
        bounds.lo.x = obj.position.x;
        bounds.lo.y = obj.position.y;
        bounds.hi = bounds.lo;
        bounds.lo.x -= radius;
        bounds.lo.y -= radius;
        bounds.hi.x += radius;
        bounds.hi.y += radius;
        bounds
    }

    /// `objectInteractsWithBridgeLayer(obj, layer, considerBridgeHealth)`:
    /// whether the object is close enough to the bridge on that layer to
    /// interact with it for pathfinding.
    pub fn object_interacts_with_bridge_layer<W: TerrainWorld>(&mut self, world: &W, obj: &ObjectView, layer: i32, consider_bridge_health: bool) -> TlResult<bool> {
        if layer == LAYER_GROUND {
            return Ok(false);
        }
        if layer == LAYER_WALL {
            if obj.layer == LAYER_WALL {
                return Ok(true); // objects on the wall can't fall off :)
            }
            if world.pathfinder_is_point_on_wall(&obj.position) {
                return Ok(true);
            }
            return Ok(false);
        }
        for i in 0..self.bridges.len() {
            if self.bridges[i].layer == layer {
                let mut matched = false;
                if self.bridges[i].is_point_on_bridge(&obj.position) {
                    matched = true;
                }

                let bounds = TerrainLogic::interaction_bounds(obj);
                if self.bridges[i].is_cell_on_end(&bounds) {
                    matched = true;
                }

                if matched {
                    let r = self.bridges[i].bridge_height(&obj.position);
                    let (bridge_height, _) = self.count(r)?;
                    let delta = (obj.position.z - bridge_height).abs();
                    if delta > LAYER_Z_CLOSE_ENOUGH_F {
                        return Ok(false);
                    }

                    // make sure it's not destroyed. can't interact with dead bridges.
                    if consider_bridge_health && self.bridges[i].info.cur_damage_state == BODY_RUBBLE {
                        return Ok(false);
                    }
                    return Ok(true);
                }
                return Ok(false);
            }
        }
        Ok(false)
    }

    /// `objectInteractsWithBridgeEnd(obj, layer)`: whether the object is
    /// close to one end of the bridge on that layer. (Returns `NULL` for the
    /// ground layer, upstream's way of writing false.)
    pub fn object_interacts_with_bridge_end(&mut self, obj: &ObjectView, layer: i32) -> TlResult<bool> {
        if layer == LAYER_GROUND {
            return Ok(false);
        }
        for i in 0..self.bridges.len() {
            if self.bridges[i].layer == layer {
                let mut matched = false;

                let bounds = TerrainLogic::interaction_bounds(obj);
                if self.bridges[i].is_cell_on_end(&bounds) {
                    matched = true;
                }

                if matched {
                    let r = self.bridges[i].bridge_height(&obj.position);
                    let (bridge_height, _) = self.count(r)?;
                    let delta = (obj.position.z - bridge_height).abs();
                    if delta > LAYER_Z_CLOSE_ENOUGH_F {
                        return Ok(false);
                    }
                    return Ok(true);
                }
                return Ok(false);
            }
        }
        Ok(false)
    }

    /// `updateBridgeDamageStates()`: every bridge reads its object's damage
    /// state; a bridge that falls to rubble kills what stood on it.
    pub fn update_bridge_damage_states<W: TerrainWorld>(&mut self, world: &mut W) -> TlResult<()> {
        for i in 0..self.bridges.len() {
            self.update_bridge_damage_state(world, i)?;
        }
        self.bridge_damage_states_changed = true;
        Ok(())
    }

    /// `Bridge::updateDamageState()` of the bridge at position `i`.
    fn update_bridge_damage_state<W: TerrainWorld>(&mut self, world: &mut W, i: usize) -> TlResult<()> {
        self.bridges[i].info.damage_state_changed = false;
        let id = self.bridges[i].info.bridge_object_id;
        if id == 0 {
            return Ok(());
        }
        if world.object_exists(id) {
            // get object damage state
            let damage_state = world.object_damage_state(id);
            let cur_state = self.bridges[i].info.cur_damage_state;
            if damage_state != cur_state {
                self.bridges[i].info.cur_damage_state = damage_state;
                if damage_state == BODY_RUBBLE {
                    let layer = self.bridges[i].layer;
                    world.pathfinder_change_bridge_state(layer, false);
                    self.bridges[i].info.damage_state_changed = true;
                    for obj in world.objects() {
                        if obj.layer == layer {
                            // don't consider the bridge health, 'cuz it's already dead. (srj)
                            if self.object_interacts_with_bridge_layer(&*world, &obj, obj.layer, false)? {
                                // The object fell off the bridge. Destroy it.
                                let mut extra = DamageInfo::default();
                                extra.input.damage_type = DAMAGE_FALLING;
                                extra.input.death_type = DEATH_SPLATTED;
                                extra.input.source_id = obj.id;
                                extra.input.amount = HUGE_DAMAGE_AMOUNT;
                                world.attempt_damage(obj.id, &extra);
                            }
                        }
                    }
                }
                if cur_state == BODY_RUBBLE {
                    // we do not set the bridge as usable if scaffolding is up ... the
                    // scaffolding code will take care of that
                    let bbi = world.bridge_scaffold_present(id);
                    if bbi != Some(true) {
                        let layer = self.bridges[i].layer;
                        world.pathfinder_change_bridge_state(layer, true);
                    }
                    self.bridges[i].info.damage_state_changed = true;
                }
            }
        } else {
            self.bridges[i].info.bridge_object_id = INVALID_ID;
        }
        Ok(())
    }

    /// `anyBridgesDamageStatesChanged()`.
    pub fn any_bridges_damage_states_changed(&self) -> bool {
        self.bridge_damage_states_changed
    }

    /// `isBridgeRepaired(bridge)`: the bridge object's damage state just
    /// changed to something other than rubble.
    pub fn is_bridge_repaired(&self, bridge: Option<ObjectId>) -> bool {
        let Some(id) = bridge else {
            return false;
        };
        for b in &self.bridges {
            let info = &b.info;
            if info.bridge_object_id == id {
                // found the right bridge.
                if info.damage_state_changed && info.cur_damage_state != BODY_RUBBLE {
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// `isBridgeBroken(bridge)`: the bridge object's damage state just
    /// changed to rubble.
    pub fn is_bridge_broken(&self, bridge: Option<ObjectId>) -> bool {
        let Some(id) = bridge else {
            return false;
        };
        for b in &self.bridges {
            let info = &b.info;
            if info.bridge_object_id == id {
                if info.damage_state_changed && info.cur_damage_state == BODY_RUBBLE {
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// `getBridgeAttackPoints(bridge, info)`: the two points of a bridge
    /// that can be attacked; for a bridge object not in the list, both are
    /// the object's position.
    pub fn bridge_attack_points(&self, bridge_id: ObjectId, bridge_position: &Coord3D) -> BridgeAttackInfo {
        for b in &self.bridges {
            let info = &b.info;
            if info.bridge_object_id == bridge_id {
                // found the right bridge.
                let mut delta = Coord3D { x: info.to.x - info.from.x, y: info.to.y - info.from.y, z: info.to.z - info.from.z };
                delta.normalize();
                let width = Coord3D {
                    x: info.from_right.x - info.from_left.x,
                    y: info.from_right.y - info.from_left.y,
                    z: info.from_right.z - info.from_left.z,
                };
                let mut len = width.length();
                len /= 2.0f32;
                return BridgeAttackInfo {
                    attack_point1: Coord3D { x: info.from.x + delta.x * len, y: info.from.y + delta.y * len, z: info.from.z + delta.z * len },
                    attack_point2: Coord3D { x: info.to.x - delta.x * len, y: info.to.y - delta.y * len, z: info.to.z - delta.z * len },
                };
            }
        }
        BridgeAttackInfo { attack_point1: *bridge_position, attack_point2: *bridge_position }
    }

    /// `pickBridge(from, to, pos)`.
    pub fn pick_bridge<W: TerrainWorld>(&mut self, world: &W, from: &Vector3, to: &Vector3) -> TlResult<PickedBridge> {
        let mut cur_draw: Option<DrawableId> = None;
        // `Vector3 curPos(0,0,0)`; None once it holds an unwritten value
        let mut cur_pos: Option<Vector3> = Some(Vector3::new(0.0, 0.0, 0.0));

        for i in 0..self.bridges.len() {
            let r = self.bridges[i].pick(world, from, to);
            let this = self.count(r)?;
            if cur_draw.is_none() {
                cur_draw = this.drawable;
                cur_pos = this.pos;
            }
        }
        Ok(PickedBridge { drawable: cur_draw, pos: cur_pos })
    }

    /// `deleteBridge(bridge)`: unlink the bridge at position `i`, turn its
    /// pathfind layer off, and destroy its object if there is one.
    pub fn delete_bridge<W: TerrainWorld>(&mut self, world: &mut W, i: usize) {
        if i >= self.bridges.len() {
            return; // bridge == NULL
        }
        let bridge = self.bridges.remove(i);
        // delete object associated with bridge if present
        world.pathfinder_change_bridge_state(bridge.layer, false);

        if world.object_exists(bridge.info.bridge_object_id) {
            world.destroy_object(bridge.info.bridge_object_id);
        }
    }

    // ---------------------------------------------------------------------
    // Update and newMap
    // ---------------------------------------------------------------------

    /// `update()`: runs once per logic frame. Moves the water tables that
    /// are changing over time.
    pub fn update<W: TerrainWorld>(&mut self, world: &mut W) -> TlResult<()> {
        // bridge damage states have not changed this frame now
        self.bridge_damage_states_changed = false;

        // update any water tables that we need to
        if self.num_water_to_update != 0 {
            let do_damage_this_frame = world.frame().is_multiple_of(LOGICFRAMES_PER_SECOND as u32);

            let mut i = self.num_water_to_update - 1;
            while i >= 0 {
                // get the water info
                let entry = self.water_to_update[i as usize];
                let water = entry.water_table;
                let change_per_frame = entry.change_per_frame;
                let target_height = entry.target_height;
                let mut damage_amount = entry.damage_amount;
                let mut current_height = entry.current_height;

                // check to see if this change per frame will get us to our target height, if so
                // we adjust the changePerFrame to make us be exactly at our target height, and after
                // the change we will remove our entry from this update phase
                let mut final_transition = false;
                if change_per_frame > 0.0 {
                    if current_height + change_per_frame >= target_height {
                        final_transition = true;
                    }
                } else if current_height + change_per_frame <= target_height {
                    final_transition = true;
                }

                if final_transition {
                    // make the final water height change, note we do damage on the final transition
                    // in all situations by passing a valid damage amount
                    self.set_water_height(world, water, target_height, damage_amount, true)?;

                    // remove our water entry from the per frame water list: upstream copies
                    // entry j over entry i for every j from i up (it does not shift)
                    for j in i..self.num_water_to_update {
                        self.water_to_update[i as usize] = self.water_to_update[j as usize];
                    }
                    self.num_water_to_update -= 1;
                } else {
                    // we're not doing damage every frame (0 damage) from the water
                    // because it's an expensive process
                    if !do_damage_this_frame {
                        damage_amount = 0.0;
                    }

                    // because some water implementation store the height as integers, some changes
                    // are too small to keep track of in the actual water data structures so we have to
                    // keep track of it outselves
                    current_height += change_per_frame;
                    self.water_to_update[i as usize].current_height = current_height;

                    // update actual water
                    self.set_water_height(world, water, current_height, damage_amount, false)?;
                }
                i -= 1;
            }
        }
        Ok(())
    }

    /// `newMap(saveGame)`: puts the waypoints on the ground now that the
    /// height map is loaded, and switches the water grid on for a map with a
    /// `WaveGuide1` waypoint.
    pub fn new_map(&mut self, global_data: &GlobalData) -> TlResult<()> {
        // Set waypoint's z value, now that the height map is loaded.
        for h in self.waypoint_handles() {
            let loc = self.waypoints[h.0].location;
            let z = self.ground_height(loc.x, loc.y, false).0;
            self.waypoints[h.0].location.z = z;
        }
        // until we have a real way to specify different water planes in the map, we will check
        // for a special waypoint name that we will put in maps that we want to have a
        // water grid
        let waypoint = self.waypoint_by_name(&AsciiString::try_from_bytes(WAVE_GUIDE_WAYPOINT).unwrap_or_default());
        let enable = waypoint.is_some();
        self.enable_water_grid(enable, global_data)
    }

    // ---------------------------------------------------------------------
    // Water
    // ---------------------------------------------------------------------

    /// `enableWaterGrid(enable)`: switch the grid, and (when enabling) set
    /// its clamps, transform, resolution and attenuation from the
    /// `GlobalData` entry for the current map. A map not listed there is
    /// left with the flag set and no notification.
    pub fn enable_water_grid(&mut self, enable: bool, global_data: &GlobalData) -> TlResult<()> {
        // set our internal variable we can query
        self.water_grid_enabled = enable;

        // set the vertex animated water properties, that is, the clamps, the water position,
        // the grid resolution etc ...
        if enable {
            let mut water_setting_index: Option<usize> = None;
            for i in 0..global_data.vertex_water_available_maps.len() {
                let map_name = &global_data.map_name;
                let available = &global_data.vertex_water_available_maps[i];
                if map_name.eq_ignore_ascii_case(available) {
                    water_setting_index = Some(i);
                    break;
                }

                // no exact map name (including path) was found, try to look for a match in just the
                // mapname.map without any path information. (strrchr(.., '\\'); the stripped name
                // keeps the backslash)
                let stripped = |s: &[u8]| -> Vec<u8> {
                    match s.iter().rposition(|&c| c == b'\\') {
                        Some(p) => s[p..].to_vec(),
                        None => s.to_vec(),
                    }
                };
                let stripped_map_name_only = stripped(map_name);
                let stripped_compare_map_name_only = stripped(available);
                if stripped_map_name_only.eq_ignore_ascii_case(&stripped_compare_map_name_only) {
                    water_setting_index = Some(i);
                    break;
                }
            }

            // check for no match found
            let Some(i) = water_setting_index else {
                // "Deformable water won't work because there was no group of vertex water data
                // defined in GameData.INI for this map name"
                return Ok(());
            };

            self.visual_write(TerrainVisualEvent::SetWaterGridHeightClamps {
                min_z: global_data.vertex_water_height_clamp_low[i],
                max_z: global_data.vertex_water_height_clamp_hi[i],
            })?;
            self.visual_write(TerrainVisualEvent::SetWaterTransform {
                angle: global_data.vertex_water_angle[i],
                x: global_data.vertex_water_x_position[i],
                y: global_data.vertex_water_y_position[i],
                z: global_data.vertex_water_z_position[i],
            })?;
            self.visual_write(TerrainVisualEvent::SetWaterGridResolution {
                cells_x: global_data.vertex_water_x_grid_cells[i] as f32,
                cells_y: global_data.vertex_water_y_grid_cells[i] as f32,
                cell_size: global_data.vertex_water_grid_size[i],
            })?;
            self.visual_write(TerrainVisualEvent::SetWaterAttenuationFactors {
                a: global_data.vertex_water_attenuation_a[i],
                b: global_data.vertex_water_attenuation_b[i],
                c: global_data.vertex_water_attenuation_c[i],
                range: global_data.vertex_water_attenuation_range[i],
            })?;
        }

        // notify the terrain visual of the change
        self.visual_write(TerrainVisualEvent::EnableWaterGrid { enable })
    }

    /// `m_waterGridEnabled`.
    pub fn water_grid_enabled(&self) -> bool {
        self.water_grid_enabled
    }

    /// `getWaterGridHeight(x, y)` as the terrain asks it (the logic-owned
    /// grid).
    fn water_grid_sample(&mut self, x: f32, y: f32) -> TlResult<WaterGridSample> {
        let r = self.visual.water_grid.water_grid_height(x, y);
        self.count(r)
    }

    /// `isUnderwater(x, y, &waterZ, &terrainZ)`.
    pub fn is_underwater(&mut self, x: f32, y: f32) -> TlResult<Underwater> {
        // get the water handle at this location
        let Some(water_handle) = self.water_handle(x, y)? else {
            // if no water here, no height, no nuttin; but we have to return the terrain Z
            return Ok(Underwater { underwater: false, water_z: None, terrain_z: self.ground_height(x, y, false).0 });
        };

        // if this water handle is a grid water use the grid height function, otherwise look into
        // the polygon trigger
        let mut w_z = 0.0f32;
        if water_handle == WaterHandle::Grid {
            if let WaterGridSample::OnGrid(z) = self.water_grid_sample(x, y)? {
                w_z = z;
            }
        } else {
            w_z = self.water_height(Some(water_handle))?;
        }

        // see if the terrain height here is below the water
        let terrain_height = self.ground_height(x, y, false).0;
        Ok(Underwater { underwater: terrain_height < w_z, water_z: Some(w_z), terrain_z: terrain_height })
    }

    /// `getWaterHandle(x, y)`: the water table with the highest water Z value
    /// at the location (a polygon water area, or the grid).
    pub fn water_handle(&mut self, x: f32, y: f32) -> TlResult<Option<WaterHandle>> {
        let mut water_handle: Option<WaterHandle> = None;
        let mut water_z = 0.0f32;

        let i_loc = ICoord3D { x: real_to_int_floor(x + 0.5f32), y: real_to_int_floor(y + 0.5f32), z: 0 };

        // Look for water areas in the polygon triggers
        for (ti, trig) in self.triggers.iter().enumerate() {
            if !trig.is_water_area() {
                continue;
            }

            // See if point is in a water area
            if trig.point_in_trigger(&i_loc) && trig.point(0).z as f32 >= water_z {
                water_z = trig.point(0).z as f32;
                water_handle = Some(WaterHandle::Polygon(ti));
            }
        }

        // For now do special check for water grid mesh.
        match self.water_grid_sample(x, y)? {
            WaterGridSample::NotOnGrid => {}
            WaterGridSample::OnGrid(mesh_z) => {
                // point falls on water grid, return the special handle for the grid water, since we
                // only have one of them and don't yet support multiple gridded water sections
                if mesh_z >= water_z {
                    water_handle = Some(WaterHandle::Grid);
                }
            }
            // `Real meshZ;` is read without having been written
            WaterGridSample::OnGridUnset => return Err(self.undef(TerrainUndefined::WaterGridHeightUnset)),
        }

        Ok(water_handle)
    }

    /// `getWaterHandleByName(name)`: `"Water Grid"` is the grid; otherwise
    /// the first polygon trigger with that name that is a water area.
    pub fn water_handle_by_name(&self, name: &AsciiString) -> Option<WaterHandle> {
        if name.compare(WATER_GRID) == Ordering::Equal {
            return Some(WaterHandle::Grid);
        }
        for (ti, trig) in self.triggers.iter().enumerate() {
            if trig.name.compare(name.str()) == Ordering::Equal && trig.is_water_area() {
                return Some(WaterHandle::Polygon(ti));
            }
        }
        None
    }

    /// `getWaterHeight(water)`: the height of a polygon water table; 0 for
    /// `NULL` and for the grid (which has no single height).
    pub fn water_height(&mut self, water: Option<WaterHandle>) -> TlResult<f32> {
        // sanity
        let Some(water) = water else {
            return Ok(0.0);
        };

        // when querying the water height given a handle, we cannot query gridded water in this
        // way because it's variable across the whole surface of the water
        match water {
            WaterHandle::Grid => Ok(0.0),
            WaterHandle::Polygon(ti) => match self.triggers.get(ti) {
                // return the height of the water using the polygon trigger
                Some(t) if t.is_water_area() => Ok(t.point(0).z as f32),
                _ => Err(self.undef(TerrainUndefined::StaleWaterHandle)),
            },
        }
    }

    /// `setWaterHeight(water, height, damageAmount, forcePathfindUpdate)`.
    /// If the water rises, objects that find themselves underwater are
    /// damaged by `damage_amount`.
    pub fn set_water_height<W: TerrainWorld>(&mut self, world: &mut W, water: Option<WaterHandle>, mut height: f32, damage_amount: f32, force_pathfind_update: bool) -> TlResult<()> {
        // sanity
        let Some(water) = water else {
            return Ok(());
        };

        // if this is a handle to gridded water simple change the transform to raise/lower the whole
        // water table.
        let previous_height;
        if water == WaterHandle::Grid {
            // get transform information
            let transform = self.visual.water_grid.transform();
            let Some(mut transform) = transform else {
                return Err(self.undef(TerrainUndefined::WaterTransformUnavailable));
            };

            // save the old height
            previous_height = transform.get_z_translation();

            // set the new height
            transform.set_z_translation(height);
            self.visual_write(TerrainVisualEvent::SetWaterTransformMatrix { transform })?;
        } else {
            // save the previous height
            previous_height = self.water_height(Some(water))?;

            // set the new height at all the points in the polygon trigger
            let WaterHandle::Polygon(ti) = water else {
                return Err(self.undef(TerrainUndefined::StaleWaterHandle));
            };
            let num_points = self.triggers[ti].num_points();
            // `newPoint.z = height`: Real to Int
            let z = {
                let r = real_to_int_cast_height(height);
                self.count(r)?
            };
            for i in 0..num_points {
                let p = *self.triggers[ti].point(i);
                self.triggers[ti].set_point(ICoord3D { x: p.x, y: p.y, z }, i as usize);
            }
            height = self.water_height(Some(water))?;
        }

        // find the bounding rectangle of this water area
        let mut affected_region = Region3D::default();
        affected_region.zero();
        self.find_axis_aligned_bounding_rect(Some(water), Some(&mut affected_region))?;

        // changes in the water level force us to recalculate the pathfinding map
        if force_pathfind_update || previous_height != height {
            // do the pathfind remapping
            world.pathfinder_force_map_recalculation();
        }

        // if the water height has risen, we need apply water damage to things that are now
        // under the water
        if damage_amount > 0.0 && height > previous_height {
            // find the center of the water "area" given the bounding region
            let center = Coord3D {
                x: affected_region.lo.x + affected_region.width() / 2.0f32,
                y: affected_region.lo.y + affected_region.height() / 2.0f32,
                z: 0.0, // irrelavant
            };

            // the max radius to scan around us is the diagonal of the bounding region
            let sum = affected_region.width() * affected_region.width() + affected_region.height() * affected_region.height();
            let max_dist = petir::Real::sqrt(f64::from(sum)) as f32;

            // scan the objects in the area of the water affected
            for obj in world.objects_in_range_2d(&center, max_dist) {
                // if this object is underwater, do some damage
                if self.is_underwater(obj.position.x, obj.position.y)?.underwater {
                    // do a lot of water damage
                    let mut damage_info = DamageInfo::default();
                    damage_info.input.damage_type = DAMAGE_WATER;
                    damage_info.input.death_type = DEATH_NORMAL;
                    damage_info.input.source_id = INVALID_ID;
                    damage_info.input.amount = damage_amount;
                    world.attempt_damage(obj.id, &damage_info);
                }
            }
        }
        Ok(())
    }

    /// `changeWaterHeightOverTime(water, finalHeight, transitionTimeInSeconds,
    /// damageAmount)`: queue a gradual change. At most `MAX_DYNAMIC_WATER`
    /// run at once (more is dropped). A zero transition time makes
    /// `changePerFrame` infinite or NaN, which upstream allows.
    pub fn change_water_height_over_time(&mut self, water: Option<WaterHandle>, final_height: f32, transition_time_in_seconds: f32, damage_amount: f32) -> TlResult<()> {
        // if we don't have room, oops!
        if self.num_water_to_update >= MAX_DYNAMIC_WATER as i32 {
            return Ok(());
        }

        // sanity
        let Some(water_h) = water else {
            return Ok(());
        };

        // if this water table already has an entry in the array to update, remove it
        let mut i = 0;
        while i < self.num_water_to_update {
            if self.water_to_update[i as usize].water_table == Some(water_h) {
                // put the entry at the end of the list here
                self.water_to_update[i as usize] = self.water_to_update[(self.num_water_to_update - 1) as usize];

                // we now have one less entry
                self.num_water_to_update -= 1;

                // process this index over again just to be complete
                i -= 1;
            }
            i += 1;
        }

        // get the current height of the water
        let current_height = self.water_height(water)?;

        // add the entry into the array of water to update
        let n = self.num_water_to_update as usize;
        self.water_to_update[n] = DynamicWaterEntry {
            water_table: Some(water_h),
            change_per_frame: (final_height - current_height) / (LOGICFRAMES_PER_SECOND as f32 * transition_time_in_seconds),
            target_height: final_height,
            damage_amount,
            current_height,
        };

        // we now have one more entry to update
        self.num_water_to_update += 1;
        Ok(())
    }

    /// The water changes in progress (`m_waterToUpdate[0..m_numWaterToUpdate]`).
    pub fn water_changes(&self) -> &[DynamicWaterEntry] {
        &self.water_to_update[..self.num_water_to_update as usize]
    }

    /// `findAxisAlignedBoundingRect(waterHandle, region)`: the box around a
    /// water table. The grid's corners are computed in `Int`s, as upstream.
    pub fn find_axis_aligned_bounding_rect(&mut self, water: Option<WaterHandle>, region: Option<&mut Region3D>) -> TlResult<()> {
        // sanity
        let (Some(water), Some(region)) = (water, region) else {
            return Ok(());
        };

        // setup the lo and high of the region to the *opposite* side of the map plus some big number
        const BUFFER: f32 = 99999.9;
        let map_extent = self.extent();
        let map_extent = self.count(map_extent)?;
        region.lo.x = map_extent.hi.x + BUFFER;
        region.lo.y = map_extent.hi.y + BUFFER;
        region.hi.x = map_extent.lo.x - BUFFER;
        region.hi.y = map_extent.lo.y - BUFFER;
        // for water grid we must access the transform
        if water == WaterHandle::Grid {
            // compute the 4 corners of the table according to the grids and grid spacing
            let Some((grid_x, grid_y, cell_size)) = self.visual.water_grid.resolution() else {
                return Err(self.undef(TerrainUndefined::WaterTransformUnavailable));
            };
            let to_int = |v: f32| real_to_int_cast_height(v);
            let r = (|| -> Result<[ICoord3D; 4], TerrainUndefined> {
                Ok([
                    ICoord3D { x: 0, y: 0, z: 0 },
                    ICoord3D { x: to_int(grid_x * cell_size)?, y: 0, z: 0 },
                    ICoord3D { x: to_int(grid_x * cell_size)?, y: to_int(grid_y * cell_size)?, z: 0 },
                    ICoord3D { x: 0, y: to_int(grid_y * cell_size)?, z: 0 },
                ])
            })();
            let p = self.count(r)?;

            // transform the 4 points using the transform matrix of the water
            let Some(transform) = self.visual.water_grid.transform() else {
                return Err(self.undef(TerrainUndefined::WaterTransformUnavailable));
            };
            // `ICoord3D p[4]` is never given a z upstream, and the transform is
            // applied to it: only a transform that ignores z is defined
            if transform.row[0].z != 0.0 || transform.row[1].z != 0.0 {
                return Err(self.undef(TerrainUndefined::WaterGridCornerZ));
            }
            for pi in &p {
                let v = Vector3::new(pi.x as f32, pi.y as f32, pi.z as f32);
                let v = Matrix3D::transform_vector(&transform, &v);

                // do the region compares
                if v.x < region.lo.x {
                    region.lo.x = v.x;
                }
                if v.x > region.hi.x {
                    region.hi.x = v.x;
                }
                if v.y < region.lo.y {
                    region.lo.y = v.y;
                }
                if v.y > region.hi.y {
                    region.hi.y = v.y;
                }
            }
        } else {
            // go through each polygon point and find the extents
            let WaterHandle::Polygon(ti) = water else {
                return Err(self.undef(TerrainUndefined::StaleWaterHandle));
            };
            let Some(trig) = self.triggers.get(ti) else {
                return Err(self.undef(TerrainUndefined::StaleWaterHandle));
            };
            for i in 0..trig.num_points() {
                // get this point
                let p = trig.point(i);

                // compare to our region
                if (p.x as f32) < region.lo.x {
                    region.lo.x = p.x as f32;
                }
                if (p.x as f32) > region.hi.x {
                    region.hi.x = p.x as f32;
                }

                if (p.y as f32) < region.lo.y {
                    region.lo.y = p.y as f32;
                }
                if (p.y as f32) > region.hi.y {
                    region.hi.y = p.y as f32;
                }

                if (p.z as f32) < region.lo.z {
                    region.lo.z = p.z as f32;
                }
                if (p.z as f32) > region.hi.z {
                    region.hi.z = p.z as f32;
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Boundaries
    // ---------------------------------------------------------------------

    /// `getActiveBoundary()`.
    pub fn active_boundary(&self) -> i32 {
        self.active_boundary
    }

    /// `m_boundaries`.
    pub fn boundaries(&self) -> &[ICoord2D] {
        &self.boundaries
    }

    /// Set the boundaries (what the device subclass copies from the height
    /// map).
    pub fn set_boundaries(&mut self, boundaries: Vec<ICoord2D>, active: i32) {
        self.boundaries = boundaries;
        self.active_boundary = active;
    }

    /// `m_mapDX`, `m_mapDY` and the device subclass' `m_mapMinZ`, `m_mapMaxZ`:
    /// what `W3DTerrainLogic::loadMap` computes from the height map
    /// ([`TerrainLogic::load_map_w3d`] sets them; this sets them directly).
    pub fn set_map_fields(&mut self, dx: i32, dy: i32, min_z: f32, max_z: f32) {
        self.map_dx = dx;
        self.map_dy = dy;
        self.map_min_z = min_z;
        self.map_max_z = max_z;
    }

    /// `setActiveBoundary(newActiveBoundary)`.
    pub fn set_active_boundary<W: TerrainWorld>(&mut self, world: &mut W, new_active_boundary: i32) {
        if new_active_boundary < 0 || new_active_boundary as usize >= self.boundaries.len() {
            // probably should DEBUG_ASSERT here
            return;
        }

        if new_active_boundary == self.active_boundary {
            // since this is fairly expensive, we should probably return
            return;
        }

        let b = self.boundaries[new_active_boundary as usize];
        if b.x == 0 || b.y == 0 {
            return;
        }

        world.before_map_boundary_change();
        self.active_boundary = new_active_boundary;
        world.after_map_boundary_change(self);
    }

    // ---------------------------------------------------------------------
    // Terrain shaping
    // ---------------------------------------------------------------------

    /// `flattenTerrain(obj)`: lower the height map under a structure to the
    /// average height of its footprint (never raising it).
    pub fn flatten_terrain(&mut self, obj: &ObjectView) -> TlResult<()> {
        if obj.geometry.get_is_small() {
            return Ok(());
        }

        let pos = obj.position;
        match obj.geometry.get_geom_type() {
            GeometryType::Box => {
                let angle = obj.orientation;

                let halfsize_x = obj.geometry.get_major_radius();
                let halfsize_y = obj.geometry.get_minor_radius();

                let c = trig::cos(angle);
                let s = trig::sin(angle);

                let top_left = Vector3::new(pos.x - halfsize_x * c - halfsize_y * s, pos.y + halfsize_y * c - halfsize_x * s, 0.0);
                let top_right = Vector3::new(pos.x + halfsize_x * c - halfsize_y * s, pos.y + halfsize_y * c + halfsize_x * s, 0.0);
                let bottom_right = Vector3::new(pos.x + halfsize_x * c + halfsize_y * s, pos.y - halfsize_y * c + halfsize_x * s, 0.0);
                let bottom_left = Vector3::new(pos.x - halfsize_x * c + halfsize_y * s, pos.y - halfsize_y * c - halfsize_x * s, 0.0);

                let mut min_x = top_left.x;
                if min_x > top_right.x {
                    min_x = top_right.x;
                }
                if min_x > bottom_right.x {
                    min_x = bottom_right.x;
                }
                if min_x > bottom_left.x {
                    min_x = bottom_left.x;
                }
                let mut max_x = top_left.x;
                if max_x < top_right.x {
                    max_x = top_right.x;
                }
                if max_x < bottom_right.x {
                    max_x = bottom_right.x;
                }
                if max_x < bottom_left.x {
                    max_x = bottom_left.x;
                }

                let mut min_y = top_left.y;
                if min_y > top_right.y {
                    min_y = top_right.y;
                }
                if min_y > bottom_right.y {
                    min_y = bottom_right.y;
                }
                if min_y > bottom_left.y {
                    min_y = bottom_left.y;
                }
                let mut max_y = top_left.y;
                if max_y < top_right.y {
                    max_y = top_right.y;
                }
                if max_y < bottom_right.y {
                    max_y = bottom_right.y;
                }
                if max_y < bottom_left.y {
                    max_y = bottom_left.y;
                }

                let i_min = ICoord2D { x: real_to_int_floor(min_x / MAP_XY_FACTOR), y: real_to_int_floor(min_y / MAP_XY_FACTOR) };
                let i_max = ICoord2D { x: real_to_int_floor(max_x / MAP_XY_FACTOR), y: real_to_int_floor(max_y / MAP_XY_FACTOR) };

                let inside = |test_pt: &Vector3| -> bool {
                    let mut flags = 0u8;
                    let mut matched = false;
                    if point_in_triangle_2d(&top_left, &top_right, &bottom_left, test_pt, 0, 1, &mut flags) {
                        matched = true;
                    }
                    if point_in_triangle_2d(&top_right, &bottom_right, &bottom_left, test_pt, 0, 1, &mut flags) {
                        matched = true;
                    }
                    matched
                };

                let mut total_height = 0.0f32;
                let mut num_samples = 0;
                for i in i_min.x..=i_max.x {
                    for j in 0..=i_max.y {
                        let test_pt = Vector3::new(i as f32 * MAP_XY_FACTOR, j as f32 * MAP_XY_FACTOR, 0.0);
                        if inside(&test_pt) {
                            total_height += self.ground_height(test_pt.x, test_pt.y, false).0;
                            num_samples += 1;
                        }
                    }
                }
                if num_samples == 0 {
                    return Ok(());
                }
                let avg_height = total_height / num_samples as f32;
                let mut raw_data_height = real_to_int_floor(0.5f32 + avg_height / MAP_HEIGHT_SCALE);

                // Compare to the height at the building's origin, because setRawMapHeight will only lower,
                // not raise.  jba
                let center_height = real_to_int_floor(self.ground_height(pos.x, pos.y, false).0 / MAP_HEIGHT_SCALE);
                if raw_data_height > center_height {
                    raw_data_height = center_height;
                }

                for i in i_min.x..=i_max.x {
                    for j in 0..=i_max.y {
                        let test_pt = Vector3::new(i as f32 * MAP_XY_FACTOR, j as f32 * MAP_XY_FACTOR, 0.0);
                        if inside(&test_pt) {
                            self.set_raw_map_height_3x3(i, j, raw_data_height)?;
                        }
                    }
                }
            }
            GeometryType::Sphere | GeometryType::Cylinder => {
                // not quite right, but close enough
                // fill in all cells that overlap as obstacle cells
                let radius = obj.geometry.get_major_radius();
                let radius_sqr = radius * radius;
                let i_min = ICoord2D { x: real_to_int_floor((pos.x - radius) / MAP_XY_FACTOR), y: real_to_int_floor((pos.y - radius) / MAP_XY_FACTOR) };
                let i_max = ICoord2D { x: real_to_int_floor((pos.x + radius) / MAP_XY_FACTOR), y: real_to_int_floor((pos.y + radius) / MAP_XY_FACTOR) };

                let inside = |test_pt: &Vector3| -> bool {
                    let dx = test_pt.x - pos.x;
                    let dy = test_pt.y - pos.y;
                    dx * dx + dy * dy < radius_sqr
                };

                let mut total_height = 0.0f32;
                let mut num_samples = 0;
                for i in i_min.x..=i_max.x {
                    for j in 0..=i_max.y {
                        let test_pt = Vector3::new(i as f32 * MAP_XY_FACTOR, j as f32 * MAP_XY_FACTOR, 0.0);
                        if inside(&test_pt) {
                            total_height += self.ground_height(test_pt.x, test_pt.y, false).0;
                            num_samples += 1;
                        }
                    }
                }
                if num_samples == 0 {
                    return Ok(());
                }
                let avg_height = total_height / num_samples as f32;
                let raw_data_height = real_to_int_floor(0.5f32 + avg_height / MAP_HEIGHT_SCALE);
                for i in i_min.x..=i_max.x {
                    for j in 0..=i_max.y {
                        let test_pt = Vector3::new(i as f32 * MAP_XY_FACTOR, j as f32 * MAP_XY_FACTOR, 0.0);
                        if inside(&test_pt) {
                            self.set_raw_map_height_3x3(i, j, raw_data_height)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// The nine `setRawMapHeight` calls `flattenTerrain` makes for one cell:
    /// the cell, its four neighbours, then the four corners.
    fn set_raw_map_height_3x3(&mut self, i: i32, j: i32, raw_data_height: i32) -> TlResult<()> {
        let offsets: [(i32, i32); 9] = [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (1, -1), (-1, 1)];
        for (dx, dy) in offsets {
            self.visual_write(TerrainVisualEvent::SetRawMapHeight { x: i.wrapping_add(dx), y: j.wrapping_add(dy), height: raw_data_height })?;
        }
        Ok(())
    }

    /// `createCraterInTerrain(obj)`: dig a circular dent under a structure.
    pub fn create_crater_in_terrain(&mut self, obj: &ObjectView) -> TlResult<()> {
        if obj.geometry.get_is_small() {
            return Ok(());
        }

        let pos = obj.position;
        let radius = obj.geometry.get_major_radius();

        if radius <= 0.0 {
            return Ok(()); // sanity
        }

        let i_min = ICoord2D { x: real_to_int_floor((pos.x - radius) / MAP_XY_FACTOR), y: real_to_int_floor((pos.y - radius) / MAP_XY_FACTOR) };
        let i_max = ICoord2D { x: real_to_int_floor((pos.x + radius) / MAP_XY_FACTOR), y: real_to_int_floor((pos.y + radius) / MAP_XY_FACTOR) };

        for i in i_min.x..=i_max.x {
            for j in 0..=i_max.y {
                let delta_x = (i as f32 * MAP_XY_FACTOR) - pos.x;
                let delta_y = (j as f32 * MAP_XY_FACTOR) - pos.y;

                let sum = delta_x * delta_x + delta_y * delta_y;
                let distance = petir::Real::sqrt(f64::from(sum)) as f32;

                if distance < radius {
                    // inside circle
                    let grid_pos = ICoord2D { x: i, y: j };

                    let displacement_amount = radius * (1.0f32 - distance / radius);

                    // `MAX( 1, getRawMapHeight(..) - displacementAmount )`: Int - Real is a
                    // Real, MAX picks between an Int and that Real, and the result is
                    // converted back to an Int (the template-less macro compares as Real)
                    let raw = self.visual.raw_map_height(&grid_pos);
                    let diff = raw as f32 - displacement_amount;
                    let target_height = {
                        let m = if 1.0f32 < diff { diff } else { 1.0f32 };
                        let r = real_to_int_cast_height(m);
                        self.count(r)?
                    };

                    self.visual_write(TerrainVisualEvent::SetRawMapHeight { x: grid_pos.x, y: grid_pos.y, height: target_height })?;
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Snapshot
    // ---------------------------------------------------------------------

    /// `crc(xfer)`: nothing.
    pub fn crc<X: Xfer + ?Sized>(&mut self, _xfer: &mut X) -> TlResult<()> {
        Ok(())
    }

    /// `xfer(xfer)`. Version 1: the active boundary. Version 2: the water
    /// changes in progress (each saved as the id of its polygon trigger).
    pub fn xfer<X: Xfer + ?Sized, W: TerrainWorld>(&mut self, xfer: &mut X, world: &mut W) -> TlResult<()> {
        // version
        const CURRENT_VERSION: u8 = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)?;

        // active boundrary
        let mut active_boundary = self.active_boundary;
        xfer.xfer_int(&mut active_boundary)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.set_active_boundary(world, active_boundary);
        }

        // updatable water tables
        if version >= 2 {
            // number of water entries in our update array
            let mut n = self.num_water_to_update;
            xfer.xfer_int(&mut n)?;
            if xfer.get_xfer_mode() == XferMode::Load {
                if !(0..=MAX_DYNAMIC_WATER as i32).contains(&n) {
                    return Err(self.undef(TerrainUndefined::WaterUpdateCount));
                }
                self.num_water_to_update = n;
            }

            // water update entry data
            for i in 0..self.num_water_to_update as usize {
                // water handle
                if xfer.get_xfer_mode() == XferMode::Save {
                    // write ID of polygon trigger that this water handle is representing
                    let mut trigger_id = match self.water_to_update[i].water_table {
                        Some(WaterHandle::Polygon(ti)) => match self.triggers.get(ti) {
                            Some(t) => t.id,
                            None => return Err(self.undef(TerrainUndefined::StaleWaterHandle)),
                        },
                        // `waterTable->m_polygon->getID()` with `m_polygon == NULL`
                        _ => return Err(self.undef(TerrainUndefined::StaleWaterHandle)),
                    };
                    xfer.xfer_int(&mut trigger_id)?;
                } else if xfer.get_xfer_mode() == XferMode::Load {
                    // read trigger id
                    let mut trigger_id = 0;
                    xfer.xfer_int(&mut trigger_id)?;

                    // find polygon trigger
                    let Some(ti) = self.trigger_by_id(trigger_id) else {
                        return Err(TerrainLogicError::InvalidData);
                    };

                    // set water handle (NULL for a trigger that is not a water area)
                    if !self.triggers[ti].is_water_area() {
                        return Err(TerrainLogicError::InvalidData);
                    }
                    self.water_to_update[i].water_table = Some(WaterHandle::Polygon(ti));
                }

                // change per frame
                xfer.xfer_real(&mut self.water_to_update[i].change_per_frame)?;

                // target height
                xfer.xfer_real(&mut self.water_to_update[i].target_height)?;

                // damage amount
                xfer.xfer_real(&mut self.water_to_update[i].damage_amount)?;

                // current height
                xfer.xfer_real(&mut self.water_to_update[i].current_height)?;
            }
        }
        Ok(())
    }

    /// `loadPostProcess()`: bridges whose object is gone are deleted.
    pub fn load_post_process<W: TerrainWorld>(&mut self, world: &mut W) {
        let mut i = 0;
        while i < self.bridges.len() {
            if !world.object_exists(self.bridges[i].info.bridge_object_id) {
                self.delete_bridge(world, i);
            } else {
                i += 1;
            }
        }
    }
}

/// `Real` to `Int` where upstream converts implicitly (truncation toward
/// zero); NaN and out-of-range are undefined there.
fn real_to_int_cast_height(x: f32) -> Result<i32, TerrainUndefined> {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        return Err(TerrainUndefined::ConversionOutOfRange);
    }
    Ok(x as i32)
}

/// `makeAlignToNormalMatrix(angle, pos, normal, mtx)`: the matrix whose x
/// axis points along `angle` (projected onto the plane) and whose z axis is
/// the normal.
pub fn make_align_to_normal_matrix(angle: f32, pos: &Coord3D, normal: &Coord3D) -> Matrix3D {
    let z = *normal;

    // It is extremely important that the resulting matrix is such that the xvector points in
    // the angle we specified; specifically, that atan2(xvec.y, xvec.x) == angle.
    let mut x = Coord3D { x: trig::cos(angle), y: trig::sin(angle), z: 0.0 };
    // x.normalize() -- redundant; is normalized by definition

    // dot of two unit vectors is cos of angle between them; we want there to be a 90-deg angle
    // between the x and z vectors, so calc x.z to satisfy this (ie, cos==0)
    if z.z != 0.0 {
        x.z = -(x.x * z.x + x.y * z.y) / z.z;
        x.normalize();
    }

    // now computing the y vector is trivial.
    let mut y = Coord3D::cross_product(&z, &x);
    y.normalize();

    Matrix3D::new(x.x, y.x, z.x, pos.x, x.y, y.y, z.y, pos.y, x.z, y.z, z.z, pos.z)
}

/// `TerrainLogic::parseWaypointDataChunk`/`parseWaypointData`: the links
/// between waypoints, as pairs of ids.
fn parse_waypoint_data_chunk(file: &mut DataChunkInput<TerrainLogic>, _info: &DataChunkInfo, terrain: &mut TerrainLogic) -> ChunkResult<bool> {
    let num_waypoint_links = file.read_int()?;
    for _ in 0..num_waypoint_links {
        let waypoint1 = file.read_int()?;
        let waypoint2 = file.read_int()?;
        terrain.add_waypoint_link(waypoint1, waypoint2);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world with nothing in it.
    #[derive(Default)]
    struct EmptyWorld {
        frame: u32,
        wall_height: f32,
    }

    impl TerrainWorld for EmptyWorld {
        fn frame(&self) -> u32 {
            self.frame
        }
        fn objects(&self) -> Vec<ObjectView> {
            Vec::new()
        }
        fn objects_in_range_2d(&self, _c: &Coord3D, _r: f32) -> Vec<ObjectView> {
            Vec::new()
        }
        fn object_exists(&self, _id: ObjectId) -> bool {
            false
        }
        fn object_damage_state(&self, _id: ObjectId) -> BodyDamageType {
            BODY_PRISTINE
        }
        fn bridge_scaffold_present(&self, _id: ObjectId) -> Option<bool> {
            None
        }
        fn object_drawable(&self, _id: ObjectId) -> Option<DrawableId> {
            None
        }
        fn attempt_damage(&mut self, _id: ObjectId, _info: &DamageInfo) {}
        fn destroy_object(&mut self, _id: ObjectId) {}
        fn pathfinder_add_bridge(&mut self, _b: &Bridge) -> PathfindLayer {
            2
        }
        fn pathfinder_change_bridge_state(&mut self, _l: PathfindLayer, _e: bool) {}
        fn pathfinder_force_map_recalculation(&mut self) {}
        fn pathfinder_wall_height(&self) -> f32 {
            self.wall_height
        }
        fn pathfinder_is_point_on_wall(&self, _p: &Coord3D) -> bool {
            false
        }
        fn create_generic_bridge_object(&mut self, _c: &Coord3D, _o: f32, _p: &Dict) -> Option<ObjectId> {
            None
        }
        fn find_bridge_road_template(&self, _n: &AsciiString) -> bool {
            false
        }
        fn find_bridge_tower_template(&self, _b: &AsciiString, _t: BridgeTowerType) -> Option<f32> {
            None
        }
        fn create_bridge_tower(&mut self, _b: ObjectId, _t: BridgeTowerType, _p: &Coord3D, _a: f32) -> ObjectId {
            0
        }
        fn before_map_boundary_change(&mut self) {}
        fn after_map_boundary_change(&mut self, _t: &TerrainLogic) {}
    }

    fn water_area(name: &str, id: i32, z: i32) -> TriggerArea {
        let pts = vec![
            ICoord3D { x: 0, y: 0, z },
            ICoord3D { x: 100, y: 0, z },
            ICoord3D { x: 100, y: 100, z },
            ICoord3D { x: 0, y: 100, z },
        ];
        TriggerArea::new(AsciiString::try_from_bytes(name.as_bytes()).unwrap(), id, true, pts).unwrap()
    }

    #[test]
    fn death_splatted_is_the_name_at_its_index() {
        assert_eq!(crate::logic::damage::DEATH_NAMES[DEATH_SPLATTED as usize], "SPLATTED");
    }

    /// A terrain over a flat 16 x 16 height map at raw height `raw`.
    fn flat_terrain(raw: u8) -> TerrainLogic {
        let mut t = TerrainLogic::new(GroundSource::LogicHeightMap);
        t.set_logic_height_map(Some(LogicHeightMap::new(16, 16, 0, vec![ICoord2D { x: 16, y: 16 }], vec![raw; 256]).unwrap()));
        t.set_boundaries(vec![ICoord2D { x: 16, y: 16 }], 0);
        t
    }

    #[test]
    fn water_height_polygon_round_trip_and_underwater() {
        let mut world = EmptyWorld::default();
        let mut t = flat_terrain(0);
        t.set_triggers(vec![water_area("Lake", 7, 5)]);
        let h = t.water_handle(50.0, 50.0).unwrap();
        assert_eq!(h, Some(WaterHandle::Polygon(0)));
        assert_eq!(t.water_height(h).unwrap(), 5.0);
        let u = t.is_underwater(50.0, 50.0).unwrap();
        assert!(u.underwater);
        assert_eq!((u.water_z, u.terrain_z), (Some(5.0), 0.0));
        // outside the lake
        assert_eq!(t.water_handle(500.0, 50.0).unwrap(), None);
        t.set_water_height(&mut world, h, -2.0, 0.0, false).unwrap();
        assert_eq!(t.water_height(h).unwrap(), -2.0);
        assert!(!t.is_underwater(50.0, 50.0).unwrap().underwater);
        assert_eq!(t.undefined_inputs(), 0);
        // the base class cannot give the extent the bounding box needs
        let mut b = TerrainLogic::new(GroundSource::Base);
        b.set_triggers(vec![water_area("Lake", 7, 5)]);
        assert_eq!(
            b.set_water_height(&mut world, Some(WaterHandle::Polygon(0)), 1.0, 0.0, false),
            Err(TerrainLogicError::Undefined(TerrainUndefined::ExtentNotImplemented))
        );
        assert_eq!(b.undefined_inputs(), 1);
    }

    #[test]
    fn undefined_extent_is_counted() {
        let mut t = TerrainLogic::new(GroundSource::Base);
        assert_eq!(t.find_closest_edge_point(&Coord3D::default()), Err(TerrainLogicError::Undefined(TerrainUndefined::ExtentNotImplemented)));
        assert_eq!(t.undefined_inputs(), 1);
    }

    #[test]
    fn water_change_over_time_finishes_and_is_removed() {
        let mut world = EmptyWorld::default();
        let mut t = flat_terrain(0);
        t.set_triggers(vec![water_area("Lake", 1, 0)]);
        t.change_water_height_over_time(Some(WaterHandle::Polygon(0)), 3.0, 0.1, 0.0).unwrap();
        assert_eq!(t.water_changes().len(), 1);
        // 3 frames (0.1 s at 30 frames/s) at 1.0 per frame
        for expect_left in [1, 1, 0] {
            t.update(&mut world).unwrap();
            assert_eq!(t.water_changes().len(), expect_left);
        }
        assert_eq!(t.water_height(Some(WaterHandle::Polygon(0))).unwrap(), 3.0);
        assert_eq!(t.undefined_inputs(), 0);
    }

    #[test]
    fn waypoint_links_and_lookup() {
        let mut t = TerrainLogic::new(GroundSource::Base);
        let ascii = |s: &str| AsciiString::try_from_bytes(s.as_bytes()).unwrap();
        for (id, name, bi) in [(1, "A", true), (2, "B", false), (3, "C", false)] {
            t.waypoints.push(Waypoint::new(id, ascii(name), Coord3D { x: id as f32 * 10.0, y: 0.0, z: 0.0 }, ascii("Path"), ascii(""), ascii(""), bi));
        }
        t.add_waypoint_link(1, 2);
        t.add_waypoint_link(1, 2); // already linked
        t.add_waypoint_link(2, 3);
        t.add_waypoint_link(1, 1); // to itself: ignored
        assert_eq!(t.waypoint(WaypointHandle(0)).unwrap().num_links(), 1);
        // A is bidirectional: B links back to A, and to C
        assert_eq!(t.waypoint(WaypointHandle(1)).unwrap().num_links(), 2);
        assert_eq!(t.waypoint_by_name(&ascii("B")), Some(WaypointHandle(1)));
        assert_eq!(t.waypoint_by_id(3), Some(WaypointHandle(2)));
        assert_eq!(t.first_waypoint(), Some(WaypointHandle(2)));
        // closest on path "path" (case-insensitive) to x = 14 is B (x = 20)? A is 4 away
        assert_eq!(t.closest_waypoint_on_path(&Coord3D { x: 14.0, y: 0.0, z: 0.0 }, &ascii("path")), Some(WaypointHandle(0)));
        assert!(t.is_purpose_of_path(Some(WaypointHandle(0)), &ascii("Path")));
        assert!(!t.is_purpose_of_path(Some(WaypointHandle(0)), &ascii("path"))); // case-sensitive
        assert_eq!(t.waypoint(WaypointHandle(0)).unwrap().link(8), Err(TerrainUndefined::WaypointLinkIndex));
    }

    #[test]
    fn bridge_geometry_from_a_landmark_object() {
        let mut world = EmptyWorld::default();
        let mut t = TerrainLogic::new(GroundSource::Base);
        let obj = LandmarkBridgeObject {
            id: 12,
            template_name: AsciiString::try_from_bytes(b"Bridge").unwrap(),
            position: Coord3D { x: 100.0, y: 100.0, z: 20.0 },
            orientation: 0.0,
            geometry: GeometryInfo::new(GeometryType::Box, false, 5.0, 50.0, 10.0),
        };
        t.add_landmark_bridge_to_logic(&mut world, &obj);
        assert_eq!(t.bridges()[0].layer(), 2);
        let b = &t.bridges()[0];
        assert!(b.is_point_on_bridge(&Coord3D { x: 100.0, y: 100.0, z: 20.0 }));
        assert!(!b.is_point_on_bridge(&Coord3D { x: 100.0, y: 130.0, z: 20.0 }));
        let (h, n) = b.bridge_height(&Coord3D { x: 100.0, y: 100.0, z: 0.0 }).unwrap();
        assert_eq!(h, 20.0);
        assert_eq!(n, Coord3D { x: 0.0, y: 0.0, z: 1.0 });
        assert_eq!(t.find_bridge_at(&Coord3D { x: 90.0, y: 100.0, z: 20.0 }), Some(0));
    }

    #[test]
    fn height_map_ground_and_extent() {
        let mut t = TerrainLogic::new(GroundSource::LogicHeightMap);
        t.set_logic_height_map(Some(LogicHeightMap::new(16, 16, 0, vec![ICoord2D { x: 16, y: 16 }], vec![80; 256]).unwrap()));
        t.set_boundaries(vec![ICoord2D { x: 16, y: 16 }], 0);
        assert_eq!(t.ground_height(55.0, 62.0, false).0, 50.0);
        let e = t.extent().unwrap();
        assert_eq!((e.hi.x, e.hi.y), (160.0, 160.0));
        let edge = t.find_closest_edge_point(&Coord3D { x: 10.0, y: 70.0, z: 0.0 }).unwrap();
        assert_eq!((edge.x, edge.y, edge.z), (0.0, 70.0, 50.0));
    }
}
