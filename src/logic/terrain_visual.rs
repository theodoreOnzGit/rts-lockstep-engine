// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/TerrainVisual.h
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DTerrainVisual.cpp
//   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DWater.h
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Water/W3DWater.cpp
//   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/WorldHeightMap.h
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/BaseHeightMap.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Authors: Colin Day (TerrainVisual, W3DTerrainVisual), John Ahlquist
//   (WorldHeightMap, BaseHeightMap), Mark Wilczynski (W3DWater)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! What `TerrainLogic` needs from `TheTerrainVisual`, as logic-owned state
//! and client events (phase 1, layer 6; see `logic::terrain_logic` for the
//! cut points).
//!
//! `TerrainLogic.cpp` calls `TheTerrainVisual` 32 times. The **writes** are
//! notifications for the renderer and become [`TerrainVisualEvent`]s. The
//! **reads** feed game logic and cannot come from a renderer in a lockstep
//! simulation, so the state they read is held here, by the logic:
//!
//! | upstream read | `TerrainLogic.cpp` | held as |
//! |---|---|---|
//! | `getWaterGridHeight(x, y, &z)` | 2164 (`isUnderwater`), 2222 (`getWaterHandle`) | [`WaterGrid::water_grid_height`] |
//! | `getWaterTransform(water, &m)` | 2316 (`setWaterHeight`), 2515 (`findAxisAlignedBoundingRect`) | [`WaterGrid::transform`] |
//! | `getWaterGridResolution(water, &x, &y, &size)` | 2502 (`findAxisAlignedBoundingRect`) | [`WaterGrid::resolution`] |
//! | `getRawMapHeight(&gridPos)` | 2899 (`createCraterInTerrain`) | [`LogicHeightMap::raw_map_height`] |
//!
//! # Where upstream reads each of them
//!
//! - **`W3DTerrainVisual::getWaterGridHeight`** is true only when
//!   `m_isWaterGridRenderingEnabled && m_waterRenderObject` and the point is
//!   inside the grid (`WaterRenderObjClass::worldToGridSpace`); then it is
//!   `m_meshData[(y+1)*(cellsX+3)+x+1].height + Get_Position().Z` with
//!   `x = REAL_TO_INT(gridX)`.
//! - **`getWaterTransform`** is `WaterRenderObjClass::Get_Transform()`;
//!   **`getWaterGridResolution`** is `m_gridCellsX`, `m_gridCellsY`,
//!   `m_gridCellSize`. Both are written by the logic itself through
//!   `enableWaterGrid` and `setWaterHeight`, so a write-then-read round trip
//!   needs the state to be where logic can see it.
//! - **`getRawMapHeight`** is `m_logicHeightMap->getHeight(x + border,
//!   y + border)`. `m_logicHeightMap` is, by upstream's own comment, "the
//!   true, golden standard" height map: `setRawMapHeight` writes it (only
//!   ever *lowering* a sample), and the client's copy follows.
//!
//! # What is decided, and what is NOT
//!
//! The state itself, and every function that reads or writes it, is
//! translated exactly as upstream has them and is differential-tested
//! against the original `TerrainLogic.cpp` (with a stand-in that mirrors the
//! W3D members).
//!
//! **Undecided (recorded, not settled here).** `docs/phase1-porting-graph.md`
//! says the water grid "must move into logic (or be proved irrelevant on
//! shipped maps)". This module makes it *possible* for the grid to be logic
//! state; it does not decide that it is. The facts found while reading
//! upstream, for whoever decides:
//!
//! 1. **The mesh heights are simulated on the client, per client frame.**
//!    `WaterRenderObjClass::update()` damps and advances every in-motion
//!    vertex, but only on a client frame in which `TheGameLogic->getFrame()`
//!    differs from the one it last saw (a function-local `static`). A client
//!    that renders slower than logic runs sees fewer steps per logic frame,
//!    so the heights, and therefore `isUnderwater`, depend on the client's
//!    frame rate. That simulation is NOT translated; the heights here are
//!    set by the embedder ([`WaterGrid::set_mesh_height`]) or stay 0.
//! 2. **Logic already pushes velocity into it.** `WaveGuideUpdate.cpp:436`
//!    calls `TheTerrainVisual->addWaterVelocity` for the wave guides; those
//!    are logic modules. The mesh is in `WaterRenderObjClass::xfer`, so
//!    upstream saves it in a save game.
//! 3. **Which maps use it.** `TerrainLogic::newMap` switches the grid on
//!    only for a map with a waypoint named `WaveGuide1`, and
//!    `enableWaterGrid` needs the map to be listed in
//!    `GlobalData::m_vertexWaterAvailableMaps` (four slots). Whether any
//!    shipped map has both was NOT checked: the map files are not in this
//!    repository. Until that is checked (and, if some do, until the mesh
//!    simulation is decided), a grid-water map is not lockstep-safe.
//! 4. **A fourth read, outside `TerrainLogic.cpp`.** `W3DTerrainLogic`
//!    (the device subclass) gets the ground height from
//!    `TheTerrainRenderObject->getHeightMapHeight`, which reads
//!    `TheTerrainVisual->getLogicHeightMap()`. That is the same logic height
//!    map as `getRawMapHeight`, and it is translated here
//!    ([`LogicHeightMap::height_map_height`]) so that `TerrainLogic` can
//!    answer ground-height queries without a renderer. `isCliffCell`
//!    (`BaseHeightMap.cpp:1256`) and `isClearLineOfSight` read the render
//!    object too and are NOT translated.
//! 5. `DO_SEISMIC_SIMULATIONS` is not defined by any file of the upstream
//!    tree (checked 2026-10-18: no `#define` of it), so `m_clientHeightMap`
//!    and `m_logicHeightMap` are the same data on shipped builds' logic side;
//!    this module has one height map.
//!
//! # Upstream quirks kept
//!
//! - `setGridResolution` compares `m_gridCellsY != m_gridCellsY` (always
//!   false), so the resolution is only taken (and the mesh only reallocated)
//!   when the **X** cell count differs. A change of Y alone is ignored.
//! - `m_gridDirectionY` starts as `(1, 0)`, the same as X, and only
//!   `setGridTransform(angle, x, y, z)` (not the matrix overload) updates the
//!   origin and the two directions.
//! - `setRawMapHeight` writes only when the stored sample is **greater**
//!   than the new height, truncates the `Int` height to a byte, and indexes
//!   with `y * width + x` and no row check, so an `x` outside the row wraps
//!   into the next one (`WorldHeightMap::getHeight`/`setRawHeight`).
//! - `getHeightMapHeight` computes the smoothed normal's two interpolations
//!   with `1.0 - fy` in `double`.
//!
//! # Where upstream is undefined
//!
//! [`TerrainUndefined`]: a `Matrix3D` read from a render object that does not
//! exist (uninitialised); a grid vertex height read when the mesh was never
//! allocated (uninitialised); a grid coordinate that is NaN (`REAL_TO_INT` of
//! NaN); a grid resolution that does not fit an `Int`, or a mesh no allocator
//! could serve; a height map with a zero extent (`getClipHeight` indexes
//! `data[-1]`). Each is an `Err`.

use crate::common::base_type::{fast_float2long_round, fast_float_floor, real_to_int, Coord3D, ICoord2D};
use crate::common::ascii_string::AsciiString;
use crate::formats::data_chunk::{ChunkError, ChunkResult, ChunkUndefined, DataChunkInfo, DataChunkInput, ERROR_CORRUPT_FILE_FORMAT};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `MAP_XY_FACTOR` (`Common/MapObject.h`): how wide and tall each height map
/// square is in world space.
pub const MAP_XY_FACTOR: f32 = 10.0;
/// `MAP_HEIGHT_SCALE` (`Common/MapObject.h`): `MAP_XY_FACTOR / 16.0f`.
pub const MAP_HEIGHT_SCALE: f32 = MAP_XY_FACTOR / 16.0;
/// `K_HEIGHT_MAP_VERSION_1` (`Common/MapReaderWriterInfo.h`).
pub const K_HEIGHT_MAP_VERSION_1: u16 = 1;
/// `K_HEIGHT_MAP_VERSION_3`: added `m_borderSize`.
pub const K_HEIGHT_MAP_VERSION_3: u16 = 3;
/// `K_HEIGHT_MAP_VERSION_4`: the boundaries list.
pub const K_HEIGHT_MAP_VERSION_4: u16 = 4;
/// `K_MAX_HEIGHT` (`WorldHeightMap.h`): `getMaxHeightValue()`.
pub const K_MAX_HEIGHT: i32 = 255;
/// `WATER_MESH_X_VERTICES` and `WATER_MESH_Y_VERTICES` (`W3DWater.cpp`).
pub const WATER_MESH_VERTICES: i32 = 128;
/// The largest water mesh (in vertices, borders included) this translation
/// will allocate; larger is [`TerrainUndefined::WaterGridSize`]. Upstream has
/// no limit (it asks the allocator); the shipped value is 131 x 131.
pub const MAX_WATER_MESH_VERTICES: i64 = 1 << 24;
/// The most height samples a map may have here (upstream: whatever the
/// allocator serves).
pub const MAX_HEIGHT_SAMPLES: i32 = 1 << 26;

/// Where upstream is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TerrainUndefined {
    /// `getWaterTransform` with no water render object: the `Matrix3D` the
    /// caller declared is read uninitialised.
    WaterTransformUnavailable,
    /// `getWaterGridHeight` returned true with no mesh allocated: the
    /// caller's `Real` is read uninitialised.
    WaterGridHeightUnset,
    /// A water-grid coordinate that is NaN reached `REAL_TO_INT`.
    NonFiniteWaterGridCoordinate,
    /// A grid resolution that does not fit an `Int`, a negative mesh size, or
    /// a mesh larger than [`MAX_WATER_MESH_VERTICES`].
    WaterGridSize,
    /// A height map whose width or height is below 1, or whose data is
    /// shorter than `width * height` (upstream indexes `data[-1]` for the
    /// extents, and reads past the data for the size).
    HeightMapShape,
    /// The base-class `TerrainLogic::getExtent` (and friends) is a
    /// `DEBUG_CRASH("not implemented")` that writes nothing; the caller reads
    /// the `Region3D` it declared.
    ExtentNotImplemented,
    /// `Compute_Intersection` on a segment parallel to the bridge's plane
    /// returns false **without writing `t`**, which `getBridgeHeight` then
    /// uses.
    BridgePlaneParallel,
    /// A `WaterHandle` that names no water: a polygon trigger that is gone or
    /// is not a water area.
    StaleWaterHandle,
    /// `m_numWaterToUpdate` (loaded from a save) outside `0..=64`.
    WaterUpdateCount,
    /// A polygon trigger with no points (`getPoint(0)` indexes before the
    /// array).
    TriggerWithoutPoints,
    /// `m_boundaries[m_activeBoundary]` with the index out of range.
    ActiveBoundary,
    /// A `Real` that does not fit the `Int` it is converted to.
    ConversionOutOfRange,
    /// `Waypoint::getLink(MAX_LINKS)`, which reads past the link array.
    WaypointLinkIndex,
    /// `findAxisAlignedBoundingRect` for the grid water applies the water
    /// transform to corner points whose z was never written; defined only
    /// when the transform ignores z (the third column's x and y are 0).
    WaterGridCornerZ,
}

/// The client events that replace the writes to `TheTerrainVisual`
/// (`TerrainLogic.cpp`). Each names the call it replaces. The queue is
/// drained by the client after the logic frame, as `ClientEvent`'s is
/// (`logic::client_event`); this is a separate enum only because those
/// carry no floats and are `Eq + Hash`.
#[derive(Clone, Debug, PartialEq)]
pub enum TerrainVisualEvent {
    /// `TheTerrainVisual->load(getSourceFilename())` (`loadMap`, when not a
    /// query).
    Load { filename: AsciiString },
    /// `TheTerrainVisual->enableWaterGrid(enable)`.
    EnableWaterGrid { enable: bool },
    /// `setWaterGridHeightClamps(NULL, minZ, maxZ)`.
    SetWaterGridHeightClamps { min_z: f32, max_z: f32 },
    /// `setWaterTransform(NULL, angle, x, y, z)`.
    SetWaterTransform { angle: f32, x: f32, y: f32, z: f32 },
    /// `setWaterTransform(&transform)` (the matrix overload).
    SetWaterTransformMatrix { transform: Matrix3D },
    /// `setWaterGridResolution(NULL, cellsX, cellsY, cellSize)`.
    SetWaterGridResolution { cells_x: f32, cells_y: f32, cell_size: f32 },
    /// `setWaterAttenuationFactors(NULL, a, b, c, range)`.
    SetWaterAttenuationFactors { a: f32, b: f32, c: f32, range: f32 },
    /// `setRawMapHeight(&gridPos, height)`.
    SetRawMapHeight { x: i32, y: i32, height: i32 },
}

/// `REAL -> Int` the way the C++ implicit conversion does for in-range
/// values (truncate toward zero); out-of-range and NaN are undefined there.
fn real_to_int_cast(x: f32) -> Result<i32, TerrainUndefined> {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        return Err(TerrainUndefined::ConversionOutOfRange);
    }
    Ok(x as i32)
}

/// What `TheTerrainVisual->getWaterGridHeight(x, y, &z)` answers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaterGridSample {
    /// It returns false: the point is not on the grid (or the grid is off).
    NotOnGrid,
    /// It returns true and writes `z`.
    OnGrid(f32),
    /// It returns true and writes nothing (no mesh): the caller's variable
    /// keeps its previous value.
    OnGridUnset,
}

/// The water grid: `W3DTerrainVisual`'s `m_isWaterGridRenderingEnabled` and
/// `m_waterRenderObject` (a `WaterRenderObjClass`), reduced to the members
/// the logic reads or that decide what it reads.
#[derive(Clone, Debug)]
pub struct WaterGrid {
    /// `m_waterRenderObject != NULL`.
    render_object_present: bool,
    /// `W3DTerrainVisual::m_isWaterGridRenderingEnabled`.
    rendering_enabled: bool,
    /// `WaterRenderObjClass::m_doWaterGrid`.
    do_water_grid: bool,
    /// `Get_Transform()`/`Set_Transform()` (identity at construction).
    transform: Matrix3D,
    /// `m_gridOrigin`.
    grid_origin: Vector2,
    /// `m_gridDirectionX`.
    grid_direction_x: Vector2,
    /// `m_gridDirectionY` (starts as `(1, 0)`, see the module docs).
    grid_direction_y: Vector2,
    /// `m_gridCellSize`.
    cell_size: f32,
    /// `m_gridCellsX`.
    cells_x: i32,
    /// `m_gridCellsY`.
    cells_y: i32,
    /// `m_minGridHeight`, `m_maxGridHeight` (stored, not read by logic).
    height_clamps: (f32, f32),
    /// `m_gridChangeAtt0..2` and `m_gridChangeMaxRange` (stored, not read by
    /// logic).
    attenuation: (f32, f32, f32, f32),
    /// `m_meshData[i].height`, `(cellsX+3) * (cellsY+3)` entries, or `None`
    /// before the first `enableWaterGrid(true)`. Velocity, status and the
    /// preferred height (the simulation) are not held.
    mesh: Option<Vec<f32>>,
}

impl Default for WaterGrid {
    /// `WaterRenderObjClass`'s constructor: a 128 x 128 grid of
    /// `MAP_XY_FACTOR` cells, no mesh, grid off.
    fn default() -> Self {
        WaterGrid {
            render_object_present: true,
            rendering_enabled: false,
            do_water_grid: false,
            transform: Matrix3D::IDENTITY,
            grid_origin: Vector2::new(0.0, 0.0),
            grid_direction_x: Vector2::new(1.0, 0.0),
            grid_direction_y: Vector2::new(1.0, 0.0),
            cell_size: MAP_XY_FACTOR,
            cells_x: WATER_MESH_VERTICES,
            cells_y: WATER_MESH_VERTICES,
            height_clamps: (0.0, 0.0),
            attenuation: (0.0, 0.0, 0.0, 0.0),
            mesh: None,
        }
    }
}

impl WaterGrid {
    /// A terrain visual with no water render object (`m_waterRenderObject ==
    /// NULL`, as a headless client has): every write is ignored and
    /// `getWaterGridHeight` is false.
    pub fn without_render_object() -> Self {
        WaterGrid { render_object_present: false, ..WaterGrid::default() }
    }

    /// `m_isWaterGridRenderingEnabled`.
    pub fn rendering_enabled(&self) -> bool {
        self.rendering_enabled
    }

    /// `WaterRenderObjClass::m_doWaterGrid`.
    pub fn do_water_grid(&self) -> bool {
        self.do_water_grid
    }

    /// `Get_Transform()`; `None` when there is no render object.
    pub fn transform(&self) -> Option<Matrix3D> {
        self.render_object_present.then_some(self.transform)
    }

    /// `getGridResolution`: `(cellsX, cellsY, cellSize)`, as `Real`s; `None`
    /// when there is no render object (the callers' variables are then
    /// unwritten).
    pub fn resolution(&self) -> Option<(f32, f32, f32)> {
        self.render_object_present.then_some((self.cells_x as f32, self.cells_y as f32, self.cell_size))
    }

    /// The number of mesh entries, `(cellsX+3) * (cellsY+3)`, or `None` when
    /// the mesh is not allocated.
    pub fn mesh_len(&self) -> Option<usize> {
        self.mesh.as_ref().map(Vec::len)
    }

    /// `W3DTerrainVisual::enableWaterGrid(enable)`.
    pub fn enable_water_grid(&mut self, enable: bool) -> Result<(), TerrainUndefined> {
        self.rendering_enabled = enable;
        if self.render_object_present {
            self.render_enable_water_grid(enable)?;
        }
        Ok(())
    }

    /// `WaterRenderObjClass::enableWaterGrid(state)`: allocates (and zeroes)
    /// the mesh the first time it is switched on.
    fn render_enable_water_grid(&mut self, state: bool) -> Result<(), TerrainUndefined> {
        self.do_water_grid = state;
        if state && self.mesh.is_none() {
            let (mx, my) = (i64::from(self.cells_x) + 3, i64::from(self.cells_y) + 3);
            if mx < 0 || my < 0 || mx * my > MAX_WATER_MESH_VERTICES {
                return Err(TerrainUndefined::WaterGridSize);
            }
            self.mesh = Some(vec![0.0; (mx * my) as usize]);
        }
        Ok(())
    }

    /// `setWaterGridHeightClamps(waterTable, minZ, maxZ)`.
    pub fn set_height_clamps(&mut self, min_z: f32, max_z: f32) {
        if self.render_object_present {
            self.height_clamps = (min_z, max_z);
        }
    }

    /// `setWaterAttenuationFactors(waterTable, a, b, c, range)`: the range is
    /// converted to grid space (`range / m_gridCellSize`).
    pub fn set_attenuation_factors(&mut self, a: f32, b: f32, c: f32, range: f32) {
        if self.render_object_present {
            self.attenuation = (a, b, c, range / self.cell_size);
        }
    }

    /// `setWaterTransform(waterTable, angle, x, y, z)`: sets the grid origin
    /// and axis directions from the rotation, and the transform.
    pub fn set_transform_angle(&mut self, angle: f32, x: f32, y: f32, z: f32) {
        if !self.render_object_present {
            return;
        }
        self.grid_direction_x = Vector2::new(1.0, 0.0);
        self.grid_origin.x = x;
        self.grid_origin.y = y;
        let mut xform = Matrix3D::IDENTITY;
        xform.rotate_z(angle);
        self.grid_direction_x.x = xform.get_x_vector().x;
        self.grid_direction_x.y = xform.get_x_vector().y;
        self.grid_direction_y.x = xform.get_y_vector().x;
        self.grid_direction_y.y = xform.get_y_vector().y;
        xform.set_translation(&Vector3::new(x, y, z));
        self.transform = xform;
    }

    /// `setWaterTransform(&transform)`: the transform only (not the origin
    /// and directions).
    pub fn set_transform_matrix(&mut self, transform: &Matrix3D) {
        if self.render_object_present {
            self.transform = *transform;
        }
    }

    /// `setWaterGridResolution(waterTable, cellsX, cellsY, cellSize)`, with
    /// upstream's always-false `m_gridCellsY != m_gridCellsY`: the cell
    /// counts are taken, and the mesh reallocated, only when the X count
    /// (compared as a `Real`) differs.
    pub fn set_resolution(&mut self, cells_x: f32, cells_y: f32, cell_size: f32) -> Result<(), TerrainUndefined> {
        if !self.render_object_present {
            return Ok(());
        }
        self.cell_size = cell_size;
        if (self.cells_x as f32) != cells_x {
            // the implicit Real -> Int conversions
            let (nx, ny) = (real_to_int_cast(cells_x)?, real_to_int_cast(cells_y)?);
            self.cells_x = nx;
            self.cells_y = ny;
            if self.mesh.is_some() {
                // free the mesh and allocate a new one at the new size
                self.mesh = None;
                let enable = self.do_water_grid;
                self.render_enable_water_grid(true)?;
                self.do_water_grid = enable;
            }
        }
        Ok(())
    }

    /// `WaterRenderObjClass::worldToGridSpace`: the grid coordinates of a
    /// world point, or `None` outside the grid. NaN is undefined upstream
    /// (it passes every comparison and reaches `REAL_TO_INT`).
    fn world_to_grid_space(&self, world_x: f32, world_y: f32) -> Result<Option<(f32, f32)>, TerrainUndefined> {
        let oo_grid_cell_size = 1.0f32 / self.cell_size;
        let dx = world_x - self.grid_origin.x;
        let dy = world_y - self.grid_origin.y;
        let grid_x = oo_grid_cell_size * (dx * self.grid_direction_x.x + dy * self.grid_direction_x.y);
        let grid_y = oo_grid_cell_size * (dx * self.grid_direction_y.x + dy * self.grid_direction_y.y);
        if grid_x < 0.0 {
            return Ok(None);
        }
        if grid_x > (self.cells_x.wrapping_sub(1)) as f32 {
            return Ok(None);
        }
        if grid_y < 0.0 {
            return Ok(None);
        }
        if grid_y > (self.cells_y.wrapping_sub(1)) as f32 {
            return Ok(None);
        }
        if grid_x.is_nan() || grid_y.is_nan() {
            return Err(TerrainUndefined::NonFiniteWaterGridCoordinate);
        }
        Ok(Some((grid_x, grid_y)))
    }

    /// `W3DTerrainVisual::getWaterGridHeight(worldX, worldY, &height)`.
    pub fn water_grid_height(&self, world_x: f32, world_y: f32) -> Result<WaterGridSample, TerrainUndefined> {
        if self.rendering_enabled && self.render_object_present {
            if let Some((gx, gy)) = self.world_to_grid_space(world_x, world_y)? {
                // point falls within grid, return correct height
                let (x, y) = (real_to_int(gx), real_to_int(gy));
                return Ok(match &self.mesh {
                    Some(mesh) => {
                        let idx = (i64::from(y) + 1) * (i64::from(self.cells_x) + 1 + 2) + i64::from(x) + 1;
                        let h = usize::try_from(idx).ok().and_then(|i| mesh.get(i)).copied().ok_or(TerrainUndefined::WaterGridSize)?;
                        WaterGridSample::OnGrid(h + self.transform.get_z_translation())
                    }
                    None => WaterGridSample::OnGridUnset,
                });
            }
        }
        Ok(WaterGridSample::NotOnGrid)
    }

    /// `setGridVertexHeight(x, y, value)`: the mesh height at a grid vertex
    /// (where the client simulation, or the embedder, writes). Ignored when
    /// the mesh is not allocated or the vertex is outside it.
    pub fn set_mesh_height(&mut self, x: i32, y: i32, value: f32) {
        if let Some(mesh) = &mut self.mesh {
            let idx = (i64::from(y) + 1) * (i64::from(self.cells_x) + 1 + 2) + i64::from(x) + 1;
            if let Some(h) = usize::try_from(idx).ok().and_then(|i| mesh.get_mut(i)) {
                *h = value;
            }
        }
    }
}

/// `WorldHeightMap`, the logic copy of it (`W3DTerrainVisual::
/// m_logicHeightMap`): the raw height samples of the map, one byte each,
/// with a border of `border_size` samples around the playable area.
#[derive(Clone, Debug, PartialEq)]
pub struct LogicHeightMap {
    /// `m_width`.
    width: i32,
    /// `m_height`.
    height: i32,
    /// `m_borderSize`.
    border_size: i32,
    /// `m_boundaries`.
    boundaries: Vec<ICoord2D>,
    /// `m_data`, `m_dataSize` entries.
    data: Vec<u8>,
    /// Batch I59 (#16): what `ParseBlendTileData` read of the map (`None`: no `BlendTileData` chunk, upstream's `NULL` arrays).
    blend: Option<BlendTiles>,
    /// Batch I63: `BaseHeightMapRenderObjClass::m_maxHeight` as `initHeightData` sets it when the map is loaded: the highest
    /// sample times `MAP_HEIGHT_SCALE`. Later height changes do not move it (upstream recomputes it only on a full
    /// `initHeightData`).
    max_height_at_load: f32,
}

/// `PATHFIND_CLIFF_SLOPE_LIMIT_F` (WorldHeightMap.cpp:65).
pub const PATHFIND_CLIFF_SLOPE_LIMIT_F: f32 = 9.8;
/// `K_BLEND_TILE_VERSION_5`: the cliff info indices.
pub const K_BLEND_TILE_VERSION_5: u16 = 5;
/// `K_BLEND_TILE_VERSION_6`: the extra blend layer.
pub const K_BLEND_TILE_VERSION_6: u16 = 6;
/// `K_BLEND_TILE_VERSION_7`: the painted cliff (impassable) flags, saved one byte short per row.
pub const K_BLEND_TILE_VERSION_7: u16 = 7;

/// One `TXTextureClass` of the map: the tiles `firstTile .. firstTile + numTiles` are the terrain type `name`.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureClass {
    /// `firstTile`.
    pub first_tile: i32,
    /// `numTiles`.
    pub num_tiles: i32,
    /// `width`.
    pub width: i32,
    /// `name`: a `TerrainType`'s name.
    pub name: Vec<u8>,
}

/// The parts of `BlendTileData` the logic reads (batch I59, #16): the tile of each sample, the cliff flag of each cell, and the
/// texture classes. The blend tiles, cliff UVs, edge classes and the tiles' art are the client's.
#[derive(Clone, Debug, PartialEq)]
pub struct BlendTiles {
    /// `m_tileNdxes`, `m_dataSize` entries.
    pub tile_ndxes: Vec<i16>,
    /// `m_cellCliffState`: one bit per cell, `flip_state_width` bytes a row, `m_height` rows.
    pub cell_cliff_state: Vec<u8>,
    /// `m_flipStateWidth`: `(m_width + 7) / 8`.
    pub flip_state_width: i32,
    /// `m_textureClasses`, `m_numTextureClasses` of them.
    pub texture_classes: Vec<TextureClass>,
}

/// What a `BlendTileData` parse reads into: the height map it belongs to, and the result.
pub struct BlendParse {
    /// The map's samples (`m_width`, `m_height`, `m_data`) as `ParseHeightMapData` left them.
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
    /// The chunk read.
    pub blend: Option<BlendTiles>,
}

/// `WorldHeightMap::ParseBlendTileData`, the logic's part: `m_dataSize` must match (`ERROR_CORRUPT_FILE_FORMAT`); the tile, blend,
/// extra blend (version 6) and cliff info (version 5) indices; the cliff flags (version 7, with its one-byte-short rows, or
/// later), else computed from the heights (`initCliffFlagsFromHeights`); the tile counts; and the texture classes. What follows
/// (the edge classes, the blend tiles, the cliff infos) is the client's and is left unread; the chunk's end is skipped.
pub fn parse_blend_tile_data_chunk(file: &mut DataChunkInput<BlendParse>, info: &DataChunkInfo, user: &mut BlendParse) -> ChunkResult<bool> {
    let data_size = user.width * user.height;
    let len = file.read_int()?;
    if len != data_size {
        return Err(ChunkError::Thrown(ERROR_CORRUPT_FILE_FORMAT));
    }
    let n = usize::try_from(data_size).unwrap_or(0);
    let read_shorts = |file: &mut DataChunkInput<BlendParse>| -> ChunkResult<Vec<i16>> {
        let mut b = vec![0u8; n * 2];
        file.read_array_of_bytes(&mut b, (n * 2) as i32)?;
        Ok(b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect())
    };
    let tile_ndxes = read_shorts(file)?;
    let _blend_tile_ndxes = read_shorts(file)?;
    if info.version >= K_BLEND_TILE_VERSION_6 {
        let _extra_blend_tile_ndxes = read_shorts(file)?;
    }
    if info.version >= K_BLEND_TILE_VERSION_5 {
        let _cliff_info_ndxes = read_shorts(file)?;
    }
    let flip_state_width = (user.width + 7) / 8;
    let rows = usize::try_from(user.height).unwrap_or(0);
    let fsw = usize::try_from(flip_state_width).unwrap_or(0);
    let mut cell_cliff_state = vec![0u8; fsw * rows];
    if info.version >= K_BLEND_TILE_VERSION_7 {
        if info.version == K_BLEND_TILE_VERSION_7 {
            // "previous incorrect length that got used to save the file": (m_width + 1) / 8 bytes a row
            let byte_width = usize::try_from((user.width + 1) / 8).unwrap_or(0);
            let mut d = vec![0u8; rows * byte_width];
            file.read_array_of_bytes(&mut d, (rows * byte_width) as i32)?;
            for j in 0..rows {
                for i in 0..byte_width {
                    if let Some(c) = cell_cliff_state.get_mut(j * fsw + i) {
                        *c = d[j * byte_width + i];
                    }
                }
            }
        } else {
            file.read_array_of_bytes(&mut cell_cliff_state, (rows * fsw) as i32)?;
        }
    } else {
        // initCliffFlagsFromHeights: each cell whose four corners span more than the slope limit
        let h = |x: i32, y: i32| f32::from(user.data[(y * user.width + x) as usize]) * MAP_HEIGHT_SCALE;
        for x in 0..user.width - 1 {
            for y in 0..user.height - 1 {
                let c = [h(x, y), h(x + 1, y), h(x, y + 1), h(x + 1, y + 1)];
                let min = c.iter().copied().fold(c[0], f32::min);
                let max = c.iter().copied().fold(c[0], f32::max);
                if max - min > PATHFIND_CLIFF_SLOPE_LIMIT_F {
                    cell_cliff_state[y as usize * fsw + (x as usize >> 3)] |= 1 << (x & 7);
                }
            }
        }
    }
    let _num_bitmap_tiles = file.read_int()?;
    let _num_blended_tiles = file.read_int()?;
    if info.version >= K_BLEND_TILE_VERSION_5 {
        let _num_cliff_info = file.read_int()?;
    }
    let num_texture_classes = file.read_int()?;
    let mut texture_classes = Vec::new();
    for _ in 0..num_texture_classes.max(0) {
        let first_tile = file.read_int()?;
        let num_tiles = file.read_int()?;
        let width = file.read_int()?;
        let _legacy = file.read_int()?;
        let name = file.read_ascii_string()?;
        texture_classes.push(TextureClass { first_tile, num_tiles, width, name });
    }
    user.blend = Some(BlendTiles { tile_ndxes, cell_cliff_state, flip_state_width, texture_classes });
    Ok(true)
}

impl LogicHeightMap {
    /// A height map of `width * height` samples (the data may be longer: a
    /// version 1 map through `ParseSizeOnly` halves its extents and keeps
    /// the full data). The shape is undefined upstream when the extents are
    /// below 1 (`getClipHeight` indexes `data[-1]`) or the data is shorter
    /// than `width * height`.
    pub fn new(width: i32, height: i32, border_size: i32, boundaries: Vec<ICoord2D>, data: Vec<u8>) -> Result<Self, TerrainUndefined> {
        if width < 1 || height < 1 || i64::from(width) * i64::from(height) > data.len() as i64 {
            return Err(TerrainUndefined::HeightMapShape);
        }
        let n = (width as usize) * (height as usize);
        let max_height_at_load = f32::from(data[..n].iter().copied().max().unwrap_or(0)) * MAP_HEIGHT_SCALE;
        Ok(LogicHeightMap { width, height, border_size, boundaries, data, blend: None, max_height_at_load })
    }

    /// Batch I59 (#16): the map's `BlendTileData` (see [`BlendTiles`]).
    pub fn set_blend_tiles(&mut self, blend: Option<BlendTiles>) {
        self.blend = blend;
    }

    /// The map's `BlendTileData`, if it had one.
    pub fn blend_tiles(&self) -> Option<&BlendTiles> {
        self.blend.as_ref()
    }

    /// `WorldHeightMap::getCliffState(xIndex, yIndex)`: false outside the map or without the chunk.
    pub fn get_cliff_state(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || y >= self.height || x >= self.width {
            return false;
        }
        let Some(b) = &self.blend else { return false };
        b.cell_cliff_state.get((y * b.flip_state_width + (x >> 3)) as usize).is_some_and(|v| v & (1 << (x & 7)) != 0)
    }

    /// `BaseHeightMapRenderObjClass::isCliffCell(x, y)`, on the logic height map: the cell of the point (`Int iX = x /
    /// MAP_XY_FACTOR`, truncated), moved by the border and clamped to `[0, extent - 2]`, and its cliff flag.
    pub fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        let mut ix = (x / MAP_XY_FACTOR) as i32 + self.border_size;
        let mut iy = (y / MAP_XY_FACTOR) as i32 + self.border_size;
        ix = ix.max(0);
        iy = iy.max(0);
        if ix >= self.width - 1 {
            ix = self.width - 2;
        }
        if iy >= self.height - 1 {
            iy = self.height - 2;
        }
        self.get_cliff_state(ix, iy)
    }

    /// `BaseHeightMapRenderObjClass::isClearLineOfSight(pos, posOther)` (the `DO_BRESENHAM` path, BaseHeightMap.cpp:1011): a
    /// Bresenham walk over the samples from `pos` to `posOther`, the line's height moving from `pos.z` to `posOther.z`; blocked
    /// where the highest of a cell's four samples is above the line by more than 0.5; clear once off the map, or once the line
    /// is above the terrain's maximum height and still rising.
    pub fn is_clear_line_of_sight(&self, pos: &Coord3D, pos_other: &Coord3D) -> bool {
        const MAP_XY_FACTOR_INV: f32 = 1.0 / MAP_XY_FACTOR;
        const LOS_FUDGE: f32 = 0.5;
        let floor = |v: f32| v.floor() as i32;
        let start_x = floor(pos.x * MAP_XY_FACTOR_INV) + self.border_size;
        let start_y = floor(pos.y * MAP_XY_FACTOR_INV) + self.border_size;
        let end_x = floor(pos_other.x * MAP_XY_FACTOR_INV) + self.border_size;
        let end_y = floor(pos_other.y * MAP_XY_FACTOR_INV) + self.border_size;
        let delta_x = (end_x - start_x).abs();
        let delta_y = (end_y - start_y).abs();
        let (mut x, mut y) = (start_x, start_y);
        let (mut xinc1, mut xinc2) = if end_x >= start_x { (1, 1) } else { (-1, -1) };
        let (mut yinc1, mut yinc2) = if end_y >= start_y { (1, 1) } else { (-1, -1) };
        let (den, mut num, numadd, numpixels) = if delta_x >= delta_y {
            xinc1 = 0;
            yinc2 = 0;
            (delta_x, delta_x / 2, delta_y, delta_x)
        } else {
            xinc2 = 0;
            yinc1 = 0;
            (delta_y, delta_y / 2, delta_x, delta_y)
        };
        let ns_inv = 1.0f32 / numpixels as f32;
        let mut z = pos.z;
        let dz = pos_other.z - z;
        let zinc = dz * ns_inv;
        let (xe, ye) = (self.width, self.height);
        for _ in 0..numpixels {
            if x < 0 || y < 0 || x >= xe - 1 || y >= ye - 1 {
                // once we go off the map, we're done
                break;
            }
            let idx = (x + y * xe) as usize;
            let d = |i: usize| f32::from(self.data[i]);
            let h = d(idx).max(d(idx + 1)).max(d(idx + xe as usize)).max(d(idx + xe as usize + 1)) * MAP_HEIGHT_SCALE;
            if h > z + LOS_FUDGE {
                return false;
            }
            if z >= self.max_height_at_load && zinc > 0.0 {
                break;
            }
            z += zinc;
            num += numadd;
            if num >= den {
                num -= den;
                x += xinc1;
                y += yinc1;
            }
            x += xinc2;
            y += yinc2;
        }
        true
    }

    /// `WorldHeightMap::getTerrainNameAt(x, y)`: the texture class of the sample under the point (floored, moved by the border,
    /// clamped to the map), whose tile (`m_tileNdxes[ndx] >> 2`) lies in the class; `None` (the empty string) when no class
    /// holds it or the map has no `BlendTileData`.
    pub fn terrain_name_at(&self, x: f32, y: f32) -> Option<&[u8]> {
        let b = self.blend.as_ref()?;
        let mut xi = (x / MAP_XY_FACTOR).floor() as i32 + self.border_size;
        let mut yi = (y / MAP_XY_FACTOR).floor() as i32 + self.border_size;
        xi = xi.max(0);
        yi = yi.max(0);
        if xi >= self.width {
            xi = self.width - 1;
        }
        if yi >= self.height {
            yi = self.height - 1;
        }
        let ndx = yi * self.width + xi;
        let tile = i32::from(*b.tile_ndxes.get(usize::try_from(ndx).ok()?)?) >> 2;
        b.texture_classes.iter().find(|c| tile >= c.first_tile && tile < c.first_tile + c.num_tiles).map(|c| c.name.as_slice())
    }

    /// `getXExtent()`.
    pub fn x_extent(&self) -> i32 {
        self.width
    }

    /// `getYExtent()`.
    pub fn y_extent(&self) -> i32 {
        self.height
    }

    /// `getBorderSizeInline()`.
    pub fn border_size(&self) -> i32 {
        self.border_size
    }

    /// `getAllBoundaries()`.
    pub fn all_boundaries(&self) -> &[ICoord2D] {
        &self.boundaries
    }

    /// `getDataPtr()`.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// `getHeight(xIndex, yIndex)`: 0 for an index outside the data. The
    /// index is `yIndex * width + xIndex`, so an `xIndex` outside the row
    /// reads the neighbouring row.
    pub fn height_at(&self, x: i32, y: i32) -> u8 {
        let ndx = y.wrapping_mul(self.width).wrapping_add(x);
        if ndx >= 0 && (ndx as usize) < self.data.len() {
            self.data[ndx as usize]
        } else {
            0
        }
    }

    /// `setRawHeight(xIndex, yIndex, height)`: ignored outside the data.
    fn set_raw_height(&mut self, x: i32, y: i32, height: u8) {
        let ndx = y.wrapping_mul(self.width).wrapping_add(x);
        if ndx >= 0 && (ndx as usize) < self.data.len() {
            self.data[ndx as usize] = height;
        }
    }

    /// `getClipHeight(x, y)` (`BaseHeightMap.h`): the sample with the index
    /// clamped to the map.
    fn clip_height(&self, mut x: i32, mut y: i32) -> u8 {
        let xextent = self.width - 1;
        let yextent = self.height - 1;
        if x < 0 {
            x = 0;
        } else if x > xextent {
            x = xextent;
        }
        if y < 0 {
            y = 0;
        } else if y > yextent {
            y = yextent;
        }
        self.data[(x + y * self.width) as usize]
    }

    /// `W3DTerrainVisual::getRawMapHeight(gridPos)`.
    pub fn raw_map_height(&self, grid_pos: &ICoord2D) -> i32 {
        let x = grid_pos.x.wrapping_add(self.border_size);
        let y = grid_pos.y.wrapping_add(self.border_size);
        i32::from(self.height_at(x, y))
    }

    /// `W3DTerrainVisual::setRawMapHeight(gridPos, height)`: lowers a sample
    /// to `height` (truncated to a byte); a sample that is not higher is left
    /// alone.
    pub fn set_raw_map_height(&mut self, grid_pos: &ICoord2D, height: i32) {
        let x = grid_pos.x.wrapping_add(self.border_size);
        let y = grid_pos.y.wrapping_add(self.border_size);
        if i32::from(self.height_at(x, y)) > height {
            self.set_raw_height(x, y, height as u8);
        }
    }

    /// `BaseHeightMapRenderObjClass::getHeightMapHeight(x, y, normal)`: the
    /// height of the terrain triangle plane that contains the point, and
    /// (when `want_normal`) its smoothed normal. Off the map (the sample
    /// would read outside the interior) the normal points up and the height
    /// is the clamped border sample's.
    pub fn height_map_height(&self, x: f32, y: f32, want_normal: bool) -> (f32, Option<Coord3D>) {
        //	3-----2
        //  |    /|
        //  |  /  |
        //	|/    |
        //  0-----1
        // Find surrounding grid points
        let map_xy_factor_inv: f32 = 1.0f32 / MAP_XY_FACTOR;
        let xdiv = x * map_xy_factor_inv;
        let ydiv = y * map_xy_factor_inv;

        let ixf = fast_float_floor(xdiv);
        let iyf = fast_float_floor(ydiv);

        let fx = xdiv - ixf; // get fraction
        let fy = ydiv - iyf; // get fraction

        // since ixf & iyf are already floor'ed, the fastest f->i conversion applies
        let ix = fast_float2long_round(ixf).wrapping_add(self.border_size);
        let iy = fast_float2long_round(iyf).wrapping_add(self.border_size);
        let x_extent = self.width;

        // Check for extent-3, not extent-1: we go into the next row/column of
        // data for smoothed triangle points.
        if ix > (x_extent - 3) || iy > (self.height - 3) || iy < 1 || ix < 1 {
            // sample point is not on the heightmap
            let normal = want_normal.then_some(Coord3D { x: 0.0, y: 0.0, z: 1.0 });
            return (f32::from(self.clip_height(ix, iy)) * MAP_HEIGHT_SCALE, normal);
        }

        let data = &self.data;
        let idx = (ix + iy * x_extent) as usize;
        let xe = x_extent as usize;
        let p0 = f32::from(data[idx]);
        let p2 = f32::from(data[idx + xe + 1]);
        let height = if fy > fx {
            // test if we are in the upper triangle
            let p3 = f32::from(data[idx + xe]);
            (p3 + (1.0f32 - fy) * (p0 - p3) + fx * (p2 - p3)) * MAP_HEIGHT_SCALE
        } else {
            // we are in the lower triangle
            let p1 = f32::from(data[idx + 1]);
            (p1 + fy * (p2 - p1) + (1.0f32 - fx) * (p0 - p1)) * MAP_HEIGHT_SCALE
        };

        if !want_normal {
            return (height, None);
        }
        //		9		  8
        //
        //10	3-----2		7
        //	  |    /|
        //	  |  /  |
        //		|/    |
        //11	0-----1		6
        //		4			5
        // Find surrounding grid points for smoothed normals.
        let idx4 = (ix + (iy - 1) * x_extent) as usize;
        let idx0 = (ix + iy * x_extent) as usize;
        let idx3 = (ix + iy * x_extent + x_extent) as usize;
        let idx9 = (ix + (iy + 2) * x_extent) as usize;
        let d0 = i32::from(data[idx0]);
        let d1 = i32::from(data[idx0 + 1]);
        let d2 = i32::from(data[idx3 + 1]);
        let d3 = i32::from(data[idx3]);
        let d4 = i32::from(data[idx4]);
        let d5 = i32::from(data[idx4 + 1]);
        let d6 = i32::from(data[idx0 + 2]);
        let d7 = i32::from(data[idx3 + 2]);
        let d8 = i32::from(data[idx9 + 1]);
        let d9 = i32::from(data[idx9]);
        let d11 = i32::from(data[idx0 - 1]);

        let delta_z_x0 = (d1 - d11) as f32;
        let delta_z_x1 = (d6 - d0) as f32;
        let delta_z_x2 = (d7 - d3) as f32;
        let delta_z_x3 = (d6 - d0) as f32;

        let delta_z_y0 = (d3 - d4) as f32;
        let delta_z_y1 = (d2 - d5) as f32;
        let delta_z_y2 = (d8 - d1) as f32;
        let delta_z_y3 = (d9 - d0) as f32;

        // Interpolate to get the smoothed value. `1.0-fy` is a double
        // upstream, so the last step of each is in double.
        let delta_z_x_left = delta_z_x0 * (1.0f32 - fx) + fx * delta_z_x3;
        let delta_z_x_right = delta_z_x1 * (1.0f32 - fx) + fx * delta_z_x2;
        let delta_z_x = (f64::from(delta_z_x_left) * (1.0f64 - f64::from(fy)) + f64::from(fy * delta_z_x_right)) as f32;

        let delta_z_y_left = delta_z_y0 * (1.0f32 - fx) + fx * delta_z_y3;
        let delta_z_y_right = delta_z_y1 * (1.0f32 - fx) + fx * delta_z_y2;
        let delta_z_y = (f64::from(delta_z_y_left) * (1.0f64 - f64::from(fy)) + f64::from(fy * delta_z_y_right)) as f32;

        let l2r = Vector3::new(2.0f32 * MAP_XY_FACTOR / MAP_HEIGHT_SCALE, 0.0, delta_z_x);
        let n2f = Vector3::new(0.0, 2.0f32 * MAP_XY_FACTOR / MAP_HEIGHT_SCALE, delta_z_y);
        let normal_at_texel = Vector3::normalized_cross_product(&l2r, &n2f);
        (height, Some(Coord3D { x: normal_at_texel.x, y: normal_at_texel.y, z: normal_at_texel.z }))
    }

    /// `WorldHeightMap::ParseHeightMapData` (`size_only == false`, the
    /// visual's logic height map) and `ParseSizeOnly` (`size_only == true`,
    /// the copy `W3DTerrainLogic::loadMap` makes). Both read the same
    /// chunk; they differ only for version 1, where `ParseSizeOnly` also
    /// halves the stored width and height and `ParseHeightMapData` leaves
    /// them (and so reads the compacted data with the old width: an
    /// upstream bug, kept).
    ///
    /// The data size is allocated *before* it is checked upstream; a size
    /// that is not positive or not `width * height` throws
    /// `ERROR_CORRUPT_FILE_FORMAT`.
    pub fn parse_chunk(file: &mut DataChunkInput<HeightMapParse>, info: &DataChunkInfo, size_only: bool) -> ChunkResult<HeightMapParse> {
        let mut m_width = file.read_int()?;
        let mut m_height = file.read_int()?;
        let border_size = if info.version >= K_HEIGHT_MAP_VERSION_3 { file.read_int()? } else { 0 };
        let boundaries = if info.version >= K_HEIGHT_MAP_VERSION_4 {
            let num_borders = file.read_int()?;
            // `m_boundaries.resize(numBorders)` of a negative count is a
            // huge resize (std::length_error)
            if num_borders < 0 {
                return Err(ChunkError::Thrown(ERROR_CORRUPT_FILE_FORMAT));
            }
            let mut b = Vec::new();
            for _ in 0..num_borders {
                let x = file.read_int()?;
                let y = file.read_int()?;
                b.push(ICoord2D { x, y });
            }
            b
        } else {
            vec![ICoord2D { x: m_width.wrapping_sub(border_size.wrapping_mul(2)), y: m_height.wrapping_sub(border_size.wrapping_mul(2)) }]
        };
        let data_size = file.read_int()?;
        if data_size <= 0 || data_size != m_width.wrapping_mul(m_height) {
            return Err(ChunkError::Thrown(ERROR_CORRUPT_FILE_FORMAT));
        }
        if data_size > MAX_HEIGHT_SAMPLES {
            return Err(ChunkError::Undefined(ChunkUndefined::BufferOverrun));
        }
        let mut data = vec![0u8; data_size as usize];
        file.read_array_of_bytes(&mut data, data_size)?;
        // Resize me.
        if info.version == K_HEIGHT_MAP_VERSION_1 {
            let new_width = (m_width + 1) / 2;
            let new_height = (m_height + 1) / 2;
            for i in 0..new_height {
                for j in 0..new_width {
                    data[(i * new_width + j) as usize] = data[(2 * i * m_width + 2 * j) as usize];
                }
            }
            if size_only {
                m_width = new_width;
                m_height = new_height;
            }
        }
        Ok(HeightMapParse { width: m_width, height: m_height, border_size, boundaries, data })
    }
}

/// What a height map chunk parse leaves (`WorldHeightMap`'s members after
/// `ParseHeightMapData`/`ParseSizeOnly`); [`HeightMapParse::into_map`] makes
/// the map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeightMapParse {
    width: i32,
    height: i32,
    border_size: i32,
    boundaries: Vec<ICoord2D>,
    data: Vec<u8>,
}

impl HeightMapParse {
    /// Batch I56: `m_width`, `m_height` and `m_borderSize` as the parse left them (`MapUtil.cpp`'s `ParseSizeOnly`
    /// statics).
    pub fn size(&self) -> (i32, i32, i32) {
        (self.width, self.height, self.border_size)
    }

    /// The map, if the parse found a `HeightMapData` chunk. A map whose
    /// stored extents no longer match its data (version 1 through
    /// `ParseHeightMapData`) keeps them as stored: [`LogicHeightMap::new`]
    /// is then an error, as upstream indexes the data by those extents.
    pub fn into_map(self) -> Result<LogicHeightMap, TerrainUndefined> {
        if self.width == 0 && self.height == 0 && self.data.is_empty() {
            return Err(TerrainUndefined::HeightMapShape);
        }
        // version 1 through ParseHeightMapData keeps the old extents over
        // the compacted data: the data is still width*height long
        LogicHeightMap::new(self.width, self.height, self.border_size, self.boundaries, self.data)
    }
}

/// `registerParser("HeightMapData", "", ParseHeightMapDataChunk)`.
fn parse_height_map_data_chunk(file: &mut DataChunkInput<HeightMapParse>, info: &DataChunkInfo, user: &mut HeightMapParse) -> ChunkResult<bool> {
    *user = LogicHeightMap::parse_chunk(file, info, false)?;
    Ok(true)
}

/// `registerParser("HeightMapData", "", ParseSizeOnlyInChunk)`.
fn parse_size_only_chunk(file: &mut DataChunkInput<HeightMapParse>, info: &DataChunkInfo, user: &mut HeightMapParse) -> ChunkResult<bool> {
    *user = LogicHeightMap::parse_chunk(file, info, true)?;
    Ok(true)
}

impl LogicHeightMap {
    /// The `WorldHeightMap(stream)` constructor's height part: parse the
    /// `HeightMapData` chunk of a map file (`size_only` selects
    /// `ParseSizeOnly`, as the logic's own copy does). The other chunks of
    /// the file are skipped. `Ok(None)` is a file with no such chunk or that
    /// is not a chunk file (upstream: `parse` false, which the constructor
    /// turns into a thrown `ERROR_CORRUPT_FILE_FORMAT`).
    pub fn from_map_file(file: crate::formats::data_chunk::CachedFileInputStream, size_only: bool) -> ChunkResult<Option<Self>> {
        let mut input = DataChunkInput::<HeightMapParse>::new(file)?;
        input.register_parser(
            b"HeightMapData",
            b"",
            if size_only { parse_size_only_chunk } else { parse_height_map_data_chunk },
        );
        let mut parsed = HeightMapParse::default();
        if !input.parse(&mut parsed)? {
            return Err(ChunkError::Thrown(ERROR_CORRUPT_FILE_FORMAT));
        }
        parsed.into_map().map(Some).or(Ok(None))
    }
}

/// The logic-owned state `TerrainLogic` reads where upstream reads
/// `TheTerrainVisual`, and the events for the writes.
#[derive(Clone, Debug, Default)]
pub struct TerrainVisualState {
    /// The water grid (`m_waterRenderObject` and its flag).
    pub water_grid: WaterGrid,
    /// `m_logicHeightMap`; `None` before a map is loaded (`NULL`).
    pub height_map: Option<LogicHeightMap>,
}

impl TerrainVisualState {
    /// Apply a write to the logic-owned state, as `W3DTerrainVisual` would
    /// have applied it to its members. (The client does its own rendering
    /// work for the same event.)
    pub fn apply(&mut self, event: &TerrainVisualEvent) -> Result<(), TerrainUndefined> {
        match event {
            TerrainVisualEvent::Load { .. } => {}
            TerrainVisualEvent::EnableWaterGrid { enable } => self.water_grid.enable_water_grid(*enable)?,
            TerrainVisualEvent::SetWaterGridHeightClamps { min_z, max_z } => self.water_grid.set_height_clamps(*min_z, *max_z),
            TerrainVisualEvent::SetWaterTransform { angle, x, y, z } => self.water_grid.set_transform_angle(*angle, *x, *y, *z),
            TerrainVisualEvent::SetWaterTransformMatrix { transform } => self.water_grid.set_transform_matrix(transform),
            TerrainVisualEvent::SetWaterGridResolution { cells_x, cells_y, cell_size } => self.water_grid.set_resolution(*cells_x, *cells_y, *cell_size)?,
            TerrainVisualEvent::SetWaterAttenuationFactors { a, b, c, range } => self.water_grid.set_attenuation_factors(*a, *b, *c, *range),
            TerrainVisualEvent::SetRawMapHeight { x, y, height } => {
                if let Some(map) = &mut self.height_map {
                    map.set_raw_map_height(&ICoord2D { x: *x, y: *y }, *height);
                }
            }
        }
        Ok(())
    }

    /// `getRawMapHeight(gridPos)`: 0 with no height map.
    pub fn raw_map_height(&self, grid_pos: &ICoord2D) -> i32 {
        self.height_map.as_ref().map_or(0, |m| m.raw_map_height(grid_pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_map(w: i32, h: i32, v: u8) -> LogicHeightMap {
        LogicHeightMap::new(w, h, 0, vec![ICoord2D { x: w, y: h }], vec![v; (w * h) as usize]).unwrap()
    }

    #[test]
    fn raw_height_set_only_lowers_and_truncates() {
        let mut m = flat_map(8, 8, 100);
        m.set_raw_map_height(&ICoord2D { x: 2, y: 3 }, 120);
        assert_eq!(m.raw_map_height(&ICoord2D { x: 2, y: 3 }), 100);
        m.set_raw_map_height(&ICoord2D { x: 2, y: 3 }, 40);
        assert_eq!(m.raw_map_height(&ICoord2D { x: 2, y: 3 }), 40);
        // 300 is not below 40; 99 + 256 = 355 is not below either; a value
        // that is lower as an Int is stored as a byte
        m.set_raw_map_height(&ICoord2D { x: 4, y: 4 }, -1);
        assert_eq!(m.raw_map_height(&ICoord2D { x: 4, y: 4 }), 255);
        // outside the data: reads 0, writes are ignored
        assert_eq!(m.raw_map_height(&ICoord2D { x: 100, y: 100 }), 0);
    }

    #[test]
    fn flat_terrain_height_and_normal() {
        let m = flat_map(16, 16, 80);
        let (h, n) = m.height_map_height(55.0, 62.0, true);
        assert_eq!(h, 80.0 * MAP_HEIGHT_SCALE);
        let n = n.unwrap();
        assert_eq!((n.x, n.y), (0.0, 0.0));
        assert!((n.z - 1.0).abs() < 1e-6);
        // off the map: the border sample, normal straight up
        let (h, n) = m.height_map_height(-500.0, -500.0, true);
        assert_eq!(h, 80.0 * MAP_HEIGHT_SCALE);
        assert_eq!(n, Some(Coord3D { x: 0.0, y: 0.0, z: 1.0 }));
    }

    #[test]
    fn water_grid_off_by_default_and_on_after_enable() {
        let mut g = WaterGrid::default();
        assert_eq!(g.water_grid_height(5.0, 5.0), Ok(WaterGridSample::NotOnGrid));
        g.enable_water_grid(true).unwrap();
        assert_eq!(g.mesh_len(), Some(131 * 131));
        assert_eq!(g.water_grid_height(5.0, 5.0), Ok(WaterGridSample::OnGrid(0.0)));
        g.set_transform_angle(0.0, 0.0, 0.0, 7.0);
        g.set_mesh_height(0, 0, 2.5);
        assert_eq!(g.water_grid_height(5.0, 5.0), Ok(WaterGridSample::OnGrid(9.5)));
        assert_eq!(g.water_grid_height(-5.0, 5.0), Ok(WaterGridSample::NotOnGrid));
        assert_eq!(g.water_grid_height(f32::NAN, 5.0), Err(TerrainUndefined::NonFiniteWaterGridCoordinate));
    }

    #[test]
    fn set_resolution_ignores_a_y_only_change() {
        let mut g = WaterGrid::default();
        g.enable_water_grid(true).unwrap();
        g.set_resolution(128.0, 64.0, 10.0).unwrap();
        assert_eq!(g.resolution(), Some((128.0, 128.0, 10.0)));
        g.set_resolution(64.0, 32.0, 10.0).unwrap();
        assert_eq!(g.resolution(), Some((64.0, 32.0, 10.0)));
        assert_eq!(g.mesh_len(), Some(67 * 35));
        assert!(WaterGrid::without_render_object().transform().is_none());
    }
}
