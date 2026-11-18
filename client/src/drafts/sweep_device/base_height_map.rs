//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/BaseHeightMap.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/BaseHeightMap.h
//!
//! `BaseHeightMapRenderObjClass` for the [`BaseHeightMapRenderObj`] contract that
//! `crate::drafts::terrain::flat_height_map` is generic over. Sweep 2 (#213) drafts the members whose names were absent
//! from the port (the shoreline tiles, scorches, visible-cliff table, bib/tree/prop forwarders, macro texture) and the
//! contract's own members (`freeMapResources`, `adjustTerrainLOD`, `ReleaseResources`, `ReAcquireResources`, `reset`,
//! `On_Frame_Update`, `initHeightData`, `staticLightingChanged`, `updateCenter`). Not drafted here: `Cast_Ray`,
//! `getHeightMapHeight`, `isClearLineOfSight`, `getMaxCellHeight`, `isCliffCell`, `Get_Obj_Space_Bounding_*`,
//! `getMaximumVisibleBox`, `loadRoadsAndBridges`, `xfer`/`crc`/`loadPostProcess`, `renderShoreLines` (the unsorted
//! world-builder version) and `drawScorches`/`doTheLight`/`getStaticDiffuse`: they are named in the port or in
//! `terrain/` drafts already, or are D3D-only; they stay out of this sweep's list.
//!
//! Design: the water plane (`TheWaterRenderObj->getWaterHeight`) is a type parameter ([`WaterHeightQuery`]), the globals
//! the functions read (`TheGlobalData`, `TheWaterTransparency`) are a [`BaseHeightMapGlobals`] snapshot, and what upstream
//! does through the D3D device or other singletons is recorded in `deferred` ([`BaseHeightMapDeferred`]) for the caller.
//!
//! Notes on upstream behaviour kept as it is:
//! - `recordShoreLineSortInfos` tests `positions[j].m_xy` before `j < m_numShoreLineTiles` (a read one past the live
//!   tiles, inside the allocated array): the positions vector here is kept at its allocated size, zero filled.
//! - `renderShoreLinesSorted` resumes a batch at the same column/row with `lastRenderedTile`.
//! - `DO_SCORCH` is defined (BaseHeightMap.h:73); `DO_ROADS` is defined; `TEST_CUSTOM_EDGING` and `EXTENDED_STATS` are not.

use crate::drafts::terrain::flat_height_map::BaseHeightMapRenderObj;
use crate::drafts::terrain::w3d_bib_buffer::W3DBibBuffer;
use crate::drafts::terrain::w3d_bridge_buffer::W3DBridgeBuffer;
use crate::drafts::terrain::w3d_common::VertexFormatXYZDUV1;
use crate::drafts::terrain::w3d_prop_buffer::W3DPropBuffer;
use crate::drafts::terrain::w3d_terrain_background::SharedHeightMap;
use crate::drafts::terrain::w3d_tree_buffer::W3DTreeBuffer;
use crate::drafts::terrain::w3d_waypoint_buffer::W3DWaypointBuffer;
use rts_lockstep_engine::common::base_type::{real_to_int, real_to_int_ceil, real_to_int_floor, Coord3D, RGBColor};
use rts_lockstep_engine::logic::terrain_visual::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use rts_lockstep_engine::ww::vector3::Vector3;

/// `DEFAULT_MAX_MAP_SHORELINE_TILES`.
pub const DEFAULT_MAX_MAP_SHORELINE_TILES: usize = 4096;
/// `DEFAULT_MAX_BATCH_SHORELINE_TILES`: most shoreline tiles per draw call.
pub const DEFAULT_MAX_BATCH_SHORELINE_TILES: usize = 512;
/// `MAX_SCORCH_VERTEX`.
pub const MAX_SCORCH_VERTEX: usize = 8194;
/// `MAX_SCORCH_INDEX`.
pub const MAX_SCORCH_INDEX: usize = 6 * 8194;
/// `MAX_SCORCH_MARKS`.
pub const MAX_SCORCH_MARKS: usize = 500;
/// `SCORCH_MARKS_IN_TEXTURE`.
pub const SCORCH_MARKS_IN_TEXTURE: i32 = 9;
/// `SCORCH_PER_ROW`.
pub const SCORCH_PER_ROW: i32 = 3;
/// `TERRAIN_LOD_MIN` = 1, `TERRAIN_LOD_MAX` = 7, `TERRAIN_LOD_AUTOMATIC` = 8 (TerrainVisual.h `TerrainLOD`; the port's
/// `common::global_data` has them too). ~~An earlier draft had MAX 8 and AUTOMATIC 9~~ **Corrected 2026-11-18 (batch
/// sweep_device):** checked against TerrainVisual.h:176-185.
pub const TERRAIN_LOD_MIN: i32 = 1;
pub const TERRAIN_LOD_MAX: i32 = 7;
pub const TERRAIN_LOD_AUTOMATIC: i32 = 8;
/// `WorldHeightMap::getMaxHeightValue`.
pub const MAX_HEIGHT_VALUE: i32 = 255;

/// `TheWaterRenderObj->getWaterHeight(x, y)` (see `sweep_device::w3d_water`).
pub trait WaterHeightQuery {
    fn get_water_height(&self, x: f32, y: f32) -> f32;
}

/// The globals the functions read.
#[derive(Clone, Copy, Debug, Default)]
pub struct BaseHeightMapGlobals {
    /// `TheGlobalData->m_terrainLOD`.
    pub terrain_lod: i32,
    /// `TheGlobalData->m_showSoftWaterEdge`.
    pub show_soft_water_edge: bool,
    /// `TheGlobalData->m_isWorldBuilder`.
    pub is_world_builder: bool,
    /// `TheGlobalData->m_shroudAlpha`.
    pub shroud_alpha: u8,
    /// `TheGlobalData->m_terrainAmbient[0]` / `m_terrainDiffuse[0]`.
    pub terrain_ambient0: RGBColor,
    pub terrain_diffuse0: RGBColor,
    /// `TheWaterTransparency->m_transparentWaterDepth` / `m_minWaterOpacity`.
    pub transparent_water_depth: f32,
    pub min_water_opacity: f32,
    /// `DX8Wrapper::getBackBufferFormat() == WW3D_FORMAT_A8R8G8B8`.
    pub back_buffer_a8r8g8b8: bool,
}

/// `struct shoreLineTileInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShoreLineTileInfo {
    /// `m_xy`: x in the low 16 bits, y in the high.
    pub m_xy: i32,
    /// `verts[4*3]`.
    pub verts: [f32; 12],
    /// `t0..t3`: index into the water depth alpha LUT.
    pub t: [f32; 4],
}

/// `struct shoreLineTileSortInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShoreLineTileSortInfo {
    pub tile_start_index: i32,
    pub num_tiles: i32,
    pub min_tile_coordinate: u16,
    pub max_tile_coordinate: u16,
}

/// `TScorch`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TScorch {
    pub location: Vector3,
    pub radius: f32,
    pub scorch_type: i32,
}

/// `VertexFormatXYZNDUV2` (dx8fvf.h), as `renderShoreLinesSorted` fills it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexFormatXYZNDUV2 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub diffuse: u32,
    pub u1: f32,
    pub v1: f32,
    pub u2: f32,
    pub v2: f32,
}

/// One `Draw_Triangles` batch of `renderShoreLinesSorted`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShoreLineBatch {
    pub vertices: Vec<VertexFormatXYZNDUV2>,
    pub indices: Vec<u16>,
}

/// The `WorldHeightMap` draw window `renderShoreLinesSorted` reads (`getDrawOrgX/Y`, `getDrawWidth/Height`), kept in
/// `WorldHeightMapExtras`/the verified map's draw origin.
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawWindow {
    pub org_x: i32,
    pub org_y: i32,
    pub width: i32,
    pub height: i32,
}

/// Work upstream does through other singletons or the device, recorded for the caller.
#[derive(Clone, Debug, PartialEq)]
pub enum BaseHeightMapDeferred {
    /// `m_shroud->reset()`.
    ShroudReset,
    /// `m_shroud->init(m_map, partitionCellSize, partitionCellSize)`.
    ShroudInit { partition_cell_size: i32 },
    /// `m_shroud->setBorderShroudLevel(TheGlobalData->m_shroudAlpha)`.
    ShroudBorderLevel(u8),
    /// `m_shroud->ReleaseResources()` / `ReAcquireResources()`.
    ShroudReleaseResources,
    ShroudReAcquireResources,
    /// `TheWaterRenderObj->ReleaseResources()` / `ReAcquireResources()`.
    WaterReleaseResources,
    WaterReAcquireResources,
    /// `TheTerrainTracksRenderObjClassSystem`, `TheW3DShadowManager`, `TheSmudgeManager`, `TheSnowManager`.
    TerrainTracksReleaseResources,
    TerrainTracksReAcquireResources,
    ShadowManagerReleaseResources,
    ShadowManagerReAcquireResources,
    SmudgeReleaseResources,
    SmudgeReAcquireResources,
    SnowReleaseResources,
    SnowReAcquireResources,
    /// `W3DShaderManager::shutdown()` / `init()`.
    ShaderManagerShutdown,
    ShaderManagerInit,
    /// `m_roadBuffer->freeRoadBuffers()` / `allocateRoadBuffers()` + `loadRoads()` / `setMap` / `clearAllRoads` /
    /// `updateLighting` / `updateCenter`.
    RoadFreeBuffers,
    RoadAllocateAndLoad,
    RoadSetMap,
    RoadClearAll,
    RoadUpdateLighting,
    RoadUpdateCenter,
    /// `m_treeBuffer->setBounds(bounds)`.
    TreeSetBounds { hi_x: f32, hi_y: f32 },
    /// `m_treeBuffer->doFullUpdate()` / `m_propBuffer->doFullUpdate()`.
    TreeDoFullUpdate,
    PropDoFullUpdate,
    /// `m_bridgeBuffer->doFullUpdate()` and `updateCenter(camera, lights)`.
    BridgeDoFullUpdate,
    BridgeUpdateCenter,
    /// `TheTacticalView->forceRedraw()`.
    TacticalViewForceRedraw,
    /// `adjustTerrainLOD`: swap the scene's terrain object to this kind (`None` = no terrain object, LOD 5).
    SwitchTerrainObject(Option<TerrainObjectKind>),
    /// `TheTacticalView->setAngle(angle + 1); setAngle(angle - 1)` after a LOD switch.
    TacticalViewJiggleAngle,
    /// `initDestAlphaLUT` filled the LUT (see `dest_alpha_lut`).
    DestAlphaLutChanged,
    /// `freeMapResources` released the stage textures and the vertex material.
    FreeStageTextures,
    /// `m_stageThreeTexture = NEW LightMapTerrainTextureClass(m_macroTextureName)`.
    NewMacroTexture(Vec<u8>),
    /// `initHeightData` allocated the cloud/light/dest-alpha textures, vertex material and shader.
    AllocateStageTextures,
}

/// Which `BaseHeightMapRenderObjClass` descendant `adjustTerrainLOD` makes the terrain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainObjectKind {
    /// `HeightMapRenderObjClass` (LOD 7).
    HeightMap,
    /// `FlatHeightMapRenderObjClass`.
    FlatHeightMap,
}

/// `class BaseHeightMapRenderObjClass` (the members of this sweep).
#[derive(Clone, Debug)]
pub struct BaseHeightMapRenderObjClass<W: WaterHeightQuery> {
    /// `m_map`.
    pub map: Option<SharedHeightMap>,
    /// `m_x` / `m_y`.
    pub x: i32,
    pub y: i32,
    pub need_full_update: bool,
    pub updating: bool,
    /// `m_minHeight` / `m_maxHeight`.
    pub min_height: f32,
    pub max_height: f32,
    /// `m_macroTextureName`.
    pub macro_texture_name: Vec<u8>,
    /// `m_showAsVisibleCliff`, `xSize * ySize`.
    pub show_as_visible_cliff: Vec<bool>,
    /// `m_curImpassableSlope`, degrees.
    pub cur_impassable_slope: f32,
    /// The shoreline arrays. `shore_line_tile_positions.len()` is `m_shoreLineTilePositionsSize`.
    pub shore_line_tile_positions: Vec<ShoreLineTileInfo>,
    pub num_shore_line_tiles: i32,
    pub num_visible_shore_line_tiles: i32,
    pub current_min_water_opacity: f32,
    pub shore_line_sort_infos: Vec<ShoreLineTileSortInfo>,
    pub shore_line_sort_infos_x_major: bool,
    pub shore_line_tile_sort_max_coordinate: i32,
    pub shore_line_tile_sort_min_coordinate: i32,
    /// The LUT `initDestAlphaLUT` writes into `m_destAlphaTexture` (256 ARGB words); empty until allocated.
    pub dest_alpha_lut: Vec<u32>,
    /// Whether `m_destAlphaTexture` / `m_stageTwoTexture` exist (`needToAllocate` tests the stage-two texture).
    pub has_dest_alpha_texture: bool,
    pub has_stage_two_texture: bool,
    /// Scorches.
    pub scorches: Vec<TScorch>,
    pub scorches_in_buffer: i32,
    pub cur_num_scorch_vertices: i32,
    pub cur_num_scorch_indices: i32,
    pub has_scorch_buffers: bool,
    pub scorch_vertices: Vec<VertexFormatXYZDUV1>,
    pub scorch_indices: Vec<u16>,
    /// Buffers.
    pub tree_buffer: Option<W3DTreeBuffer>,
    pub prop_buffer: Option<W3DPropBuffer>,
    pub bib_buffer: Option<W3DBibBuffer>,
    pub bridge_buffer: Option<W3DBridgeBuffer>,
    pub waypoint_buffer: Option<W3DWaypointBuffer>,
    /// `m_shroud != NULL`.
    pub has_shroud: bool,
    pub globals: BaseHeightMapGlobals,
    pub water: W,
    /// Follow-up work for the caller, in order.
    pub deferred: Vec<BaseHeightMapDeferred>,
}

impl<W: WaterHeightQuery> BaseHeightMapRenderObjClass<W> {
    /// A fresh object (the constructor's zero state; `m_curImpassableSlope` defaults as `BaseHeightMapRenderObjClass()`
    /// sets it, not part of this sweep: 45 degrees is the retail default of `TheGlobalData->m_impassableSlope`).
    pub fn new(water: W, globals: BaseHeightMapGlobals) -> Self {
        BaseHeightMapRenderObjClass {
            map: None,
            x: 0,
            y: 0,
            need_full_update: true,
            updating: false,
            min_height: 0.0,
            max_height: 0.0,
            macro_texture_name: Vec::new(),
            show_as_visible_cliff: Vec::new(),
            cur_impassable_slope: 45.0,
            shore_line_tile_positions: Vec::new(),
            num_shore_line_tiles: 0,
            num_visible_shore_line_tiles: 0,
            current_min_water_opacity: -1.0,
            shore_line_sort_infos: Vec::new(),
            shore_line_sort_infos_x_major: false,
            shore_line_tile_sort_max_coordinate: 0,
            shore_line_tile_sort_min_coordinate: 0,
            dest_alpha_lut: Vec::new(),
            has_dest_alpha_texture: false,
            has_stage_two_texture: false,
            scorches: Vec::new(),
            scorches_in_buffer: 0,
            cur_num_scorch_vertices: 0,
            cur_num_scorch_indices: 0,
            has_scorch_buffers: false,
            scorch_vertices: Vec::new(),
            scorch_indices: Vec::new(),
            tree_buffer: None,
            prop_buffer: None,
            bib_buffer: None,
            bridge_buffer: None,
            waypoint_buffer: None,
            has_shroud: false,
            globals,
            water,
            deferred: Vec::new(),
        }
    }

    fn map_extent(&self) -> (i32, i32, i32) {
        match &self.map {
            Some(m) => {
                let m = m.read().unwrap();
                (m.width, m.height, m.border_size)
            }
            None => (0, 0, 0),
        }
    }

    /// `BaseHeightMapRenderObjClass::updateMacroTexture` (BaseHeightMap.cpp:628).
    pub fn update_macro_texture(&mut self, texture_name: &[u8]) {
        self.macro_texture_name = texture_name.to_vec();
        // REF_PTR_RELEASE(m_stageThreeTexture); m_stageThreeTexture = NEW LightMapTerrainTextureClass(name): device-only
        self.deferred.push(BaseHeightMapDeferred::NewMacroTexture(self.macro_texture_name.clone()));
    }

    // --- visible cliffs -------------------------------------------------------------------------------------------

    /// `BaseHeightMapRenderObjClass::showAsVisibleCliff` (BaseHeightMap.cpp:1283).
    pub fn show_as_visible_cliff(&self, x_index: i32, y_index: i32) -> bool {
        if self.map.is_none() {
            return false;
        }
        let (x_size, _, _) = self.map_extent();
        self.show_as_visible_cliff[(x_index + y_index * x_size) as usize]
    }

    /// `BaseHeightMapRenderObjClass::evaluateAsVisibleCliff` (BaseHeightMap.cpp:1297): is any of the cell's other three
    /// corners more than `values_greater_than_rad` (a tangent) steeper than corner 0?
    pub fn evaluate_as_visible_cliff(&self, x_index: i32, y_index: i32, values_greater_than_rad: f32) -> bool {
        let map = self.map.as_ref().expect("m_map");
        let map = map.read().unwrap();
        // sqrt(2.0f) is a double in C++ (`sqrt(2.0f)` picks the double overload on MSVC6): kept in f64.
        let distance: [f32; 4] = [0.0, 1.0 * MAP_XY_FACTOR, ((2.0f32 as f64).sqrt() as f32) * MAP_XY_FACTOR, 1.0 * MAP_XY_FACTOR];
        let bytes: [u8; 4] = [
            map.get_height(x_index, y_index),
            map.get_height(x_index + 1, y_index),
            map.get_height(x_index + 1, y_index + 1),
            map.get_height(x_index, y_index + 1),
        ];
        let heights: [f32; 4] = [
            bytes[0] as f32 * MAP_HEIGHT_SCALE,
            bytes[1] as f32 * MAP_HEIGHT_SCALE,
            bytes[2] as f32 * MAP_HEIGHT_SCALE,
            bytes[3] as f32 * MAP_HEIGHT_SCALE,
        ];
        let mut any_impassable = false;
        let mut i = 1;
        while i < 4 && !any_impassable {
            if ((heights[i] - heights[0]) / distance[i]).abs() > values_greater_than_rad {
                any_impassable = true;
            }
            i += 1;
        }
        any_impassable
    }

    /// `BaseHeightMapRenderObjClass::updateViewImpassableAreas` (BaseHeightMap.cpp:1715): rebuilds the table for the
    /// region (everything unless `partial`). `tan` is `petir`-style: `tan_fn` is the port's deterministic tangent.
    pub fn update_view_impassable_areas(&mut self, partial: bool, min_x: i32, max_x: i32, min_y: i32, max_y: i32, tan_fn: &impl Fn(f32) -> f32) {
        let (x_size, y_size, _) = self.map_extent();
        if self.show_as_visible_cliff.len() as i32 != x_size * y_size {
            self.show_as_visible_cliff.resize((x_size * y_size) as usize, false);
        }
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (min_x, max_x, min_y, max_y);
        if !partial {
            min_x = 0;
            min_y = 0;
            max_x = x_size;
            max_y = y_size;
        }
        let tan_impassable_rad = tan_fn(self.cur_impassable_slope / 360.0 * 2.0 * std::f32::consts::PI);
        for j in min_y..max_y {
            for i in min_x..max_x {
                let v = self.evaluate_as_visible_cliff(i, j, tan_impassable_rad);
                self.show_as_visible_cliff[(i + j * x_size) as usize] = v;
            }
        }
    }

    // --- shorelines -----------------------------------------------------------------------------------------------

    /// `BaseHeightMapRenderObjClass::setShoreLineDetail` (BaseHeightMap.cpp:1482).
    pub fn set_shore_line_detail(&mut self) {
        if self.map.is_none() {
            return;
        }
        let (dx, dy, _) = self.map_extent();
        self.update_shoreline_tiles(0, 0, dx - 1, dy - 1);
    }

    /// `BaseHeightMapRenderObjClass::updateShorelineTile` (BaseHeightMap.cpp:1597): records the tile if it needs
    /// shoreline blending (some corner under water but shallow, or some corners above and some below).
    pub fn update_shoreline_tile(&mut self, i: i32, j: i32, border: i32) {
        let transparent_depth = self.globals.transparent_water_depth * self.globals.min_water_opacity;
        let depth_scale_factor = 1.0 / transparent_depth;
        let x0 = (i - border) as f32 * MAP_XY_FACTOR;
        let y0 = (j - border) as f32 * MAP_XY_FACTOR;
        let (h00, h10, h11, h01) = {
            let m = self.map.as_ref().expect("m_map").read().unwrap();
            (m.get_height(i, j), m.get_height(i + 1, j), m.get_height(i + 1, j + 1), m.get_height(i, j + 1))
        };
        let terrain_z0 = MAP_HEIGHT_SCALE * h00 as f32;
        let water_z0 = self.water.get_water_height(x0, y0);
        let mut water_side: i32 = (water_z0 > terrain_z0) as i32;
        let x1 = (i - border + 1) as f32 * MAP_XY_FACTOR;
        let y1 = (j - border + 1) as f32 * MAP_XY_FACTOR;
        let terrain_z1 = MAP_HEIGHT_SCALE * h10 as f32;
        let water_z1 = self.water.get_water_height(x1, y0);
        water_side |= ((water_z1 > terrain_z1) as i32) << 1;
        let terrain_z2 = MAP_HEIGHT_SCALE * h11 as f32;
        let water_z2 = self.water.get_water_height(x1, y1);
        water_side |= ((water_z2 > terrain_z2) as i32) << 2;
        let terrain_z3 = MAP_HEIGHT_SCALE * h01 as f32;
        let water_z3 = self.water.get_water_height(x0, y1);
        water_side |= ((water_z3 > terrain_z3) as i32) << 3;

        if water_side == 0 || (water_z0 * water_z1 * water_z2 * water_z3) <= 0.0 {
            return; // all verts on the surface side of water, or one is outside the water plane bounds (waterHeight <= 0!)
        }
        if water_side < 0xf
            || (water_z0 - terrain_z0) < transparent_depth
            || (water_z1 - terrain_z1) < transparent_depth
            || (water_z2 - terrain_z2) < transparent_depth
            || (water_z3 - terrain_z3) < transparent_depth
        {
            // add tile to set that needs shoreline blending.
            if self.num_shore_line_tiles as usize >= self.shore_line_tile_positions.len() {
                // no more room to store extra blend tiles so enlarge the buffer.
                let new_len = self.shore_line_tile_positions.len() + 512;
                self.shore_line_tile_positions.resize(new_len, ShoreLineTileInfo::default());
            }
            let info = &mut self.shore_line_tile_positions[self.num_shore_line_tiles as usize];
            info.m_xy = i | (j << 16);
            info.verts[0] = x0;
            info.verts[1] = y0;
            info.verts[2] = terrain_z0;
            info.t[0] = (water_z0 - terrain_z0) * depth_scale_factor;
            info.verts[3] = x1;
            info.verts[4] = y0;
            info.verts[5] = terrain_z1;
            info.t[1] = (water_z1 - terrain_z1) * depth_scale_factor;
            info.verts[6] = x1;
            info.verts[7] = y1;
            info.verts[8] = terrain_z2;
            info.t[2] = (water_z2 - terrain_z2) * depth_scale_factor;
            info.verts[9] = x0;
            info.verts[10] = y1;
            info.verts[11] = terrain_z3;
            info.t[3] = (water_z3 - terrain_z3) * depth_scale_factor;
            self.num_shore_line_tiles += 1;
        }
    }

    /// `BaseHeightMapRenderObjClass::updateShorelineTiles` (BaseHeightMap.cpp:1653): drops the recorded tiles inside the
    /// region (`x < maxX`, `y < maxY`: the last row and column are not touched), then re-scans it.
    pub fn update_shoreline_tiles(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        let (xe, ye, border) = self.map_extent();
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (min_x, min_y, max_x, max_y);
        if min_x < 0 {
            min_x = 0;
        }
        if min_y < 0 {
            min_y = 0;
        }
        if max_x > xe - 1 {
            max_x = xe - 1;
        }
        if max_y > ye - 1 {
            max_y = ye - 1;
        }
        if self.shore_line_tile_positions.is_empty() {
            // Need to allocate memory
            self.shore_line_tile_positions = vec![ShoreLineTileInfo::default(); DEFAULT_MAX_MAP_SHORELINE_TILES];
        }
        let mut j = 0i32;
        while j < self.num_shore_line_tiles {
            let xy = self.shore_line_tile_positions[j as usize].m_xy;
            let x = xy & 0xffff;
            let y = xy >> 16;
            if x >= min_x && x < max_x && y >= min_y && y < max_y {
                // this tile is inside region being updated so remove it by shifting tile array
                let n = (self.num_shore_line_tiles - 1 - j) as usize;
                let from = (j + 1) as usize;
                self.shore_line_tile_positions.copy_within(from..from + n, j as usize);
                self.num_shore_line_tiles -= 1;
                j -= 1; // look at current tile again since it was removed.
            }
            j += 1;
        }
        if self.globals.transparent_water_depth == 0.0 || !self.globals.show_soft_water_edge {
            return;
        }
        let mut sort_x_major = false;
        if !self.globals.is_world_builder {
            // we're in the game, not the builder
            if (xe - 1) > (ye - 1) {
                sort_x_major = true;
            }
        }
        if sort_x_major {
            for i in min_x..max_x {
                for j in min_y..max_y {
                    self.update_shoreline_tile(i, j, border);
                }
            }
        } else {
            for j in min_y..max_y {
                for i in min_x..max_x {
                    self.update_shoreline_tile(i, j, border);
                }
            }
        }
        self.record_shore_line_sort_infos();
    }

    /// `BaseHeightMapRenderObjClass::recordShoreLineSortInfos` (BaseHeightMap.cpp:1498): per column (or row, whichever
    /// is the longer axis) start index, count and min/max of the other coordinate. Assumes the whole map was scanned.
    pub fn record_shore_line_sort_infos(&mut self) {
        if self.globals.is_world_builder || self.shore_line_tile_positions.is_empty() || self.map.is_none() {
            return; // we must be in the builder so don't sort.
        }
        let (xe, ye, _) = self.map_extent();
        let mut size = xe - 1;
        let mut x_major = true;
        if size <= (ye - 1) {
            size = ye - 1;
            x_major = false;
        }
        self.shore_line_sort_infos_x_major = x_major;
        if self.shore_line_sort_infos.is_empty() || size as usize > self.shore_line_sort_infos.len() {
            // old buffer was too small.
            self.shore_line_sort_infos = vec![ShoreLineTileSortInfo::default(); size as usize];
        } else {
            for s in self.shore_line_sort_infos.iter_mut() {
                *s = ShoreLineTileSortInfo::default(); // memset over m_shoreLineSortInfosSize entries
            }
        }
        let n = self.num_shore_line_tiles;
        let pos = |p: &Vec<ShoreLineTileInfo>, k: i32| -> i32 { p.get(k as usize).map(|t| t.m_xy).unwrap_or(0) };
        if x_major {
            // map is wider than taller, so tiles are already sorted by x
            let first = pos(&self.shore_line_tile_positions, 0) & 0xffff;
            self.shore_line_tile_sort_max_coordinate = first;
            self.shore_line_tile_sort_min_coordinate = first;
            let mut i = 0;
            while i < n {
                let x = pos(&self.shore_line_tile_positions, i) & 0xffff;
                if x > self.shore_line_tile_sort_max_coordinate {
                    self.shore_line_tile_sort_max_coordinate = x;
                }
                let mut j = i + 1;
                let min_y = pos(&self.shore_line_tile_positions, i) >> 16;
                let mut max_y = min_y;
                while (pos(&self.shore_line_tile_positions, j) & 0xffff) == x && j < n {
                    // keep track of highest y coordinate.
                    let y = pos(&self.shore_line_tile_positions, j) >> 16;
                    if y > max_y {
                        max_y = y;
                    }
                    j += 1;
                }
                let info = &mut self.shore_line_sort_infos[x as usize];
                info.tile_start_index = i;
                info.num_tiles = j - i;
                info.min_tile_coordinate = min_y as u16;
                info.max_tile_coordinate = max_y as u16;
                i += info.num_tiles - 1; // skip tiles we just scanned.
                i += 1;
            }
        } else {
            // map is taller than wider so tiles are already sorted by y
            let first = pos(&self.shore_line_tile_positions, 0) >> 16;
            self.shore_line_tile_sort_max_coordinate = first;
            self.shore_line_tile_sort_min_coordinate = first;
            let mut i = 0;
            while i < n {
                let y = pos(&self.shore_line_tile_positions, i) >> 16;
                if y > self.shore_line_tile_sort_max_coordinate {
                    self.shore_line_tile_sort_max_coordinate = y;
                }
                let mut j = i + 1;
                let min_x = pos(&self.shore_line_tile_positions, i) & 0xffff;
                let mut max_x = min_x;
                while (pos(&self.shore_line_tile_positions, j) >> 16) == y && j < n {
                    // keep track of highest x coordinate.
                    let x = pos(&self.shore_line_tile_positions, j) & 0xffff;
                    if x > max_x {
                        max_x = x;
                    }
                    j += 1;
                }
                let info = &mut self.shore_line_sort_infos[y as usize];
                info.tile_start_index = i;
                info.num_tiles = j - i;
                info.min_tile_coordinate = min_x as u16;
                info.max_tile_coordinate = max_x as u16;
                i += info.num_tiles - 1; // skip tiles we just scanned.
                i += 1;
            }
        }
    }

    /// `BaseHeightMapRenderObjClass::initDestAlphaLUT` (BaseHeightMap.cpp:1742): the 256 entry water-depth alpha ramp
    /// `(alpha << 24) | 0x00ffffff`, alpha capped at `minWaterOpacity * 255`.
    pub fn init_dest_alpha_lut(&mut self) {
        if !self.has_dest_alpha_texture {
            return;
        }
        let max_opacity = (self.globals.min_water_opacity * 255.0) as i32;
        let mut lut = Vec::with_capacity(256);
        for x in 0..256i32 {
            let mut alpha = x;
            if alpha > max_opacity {
                alpha = max_opacity;
            }
            lut.push(((alpha as u32) << 24) | 0x00ff_ffff);
        }
        self.dest_alpha_lut = lut;
        // Get_Filter().Set_U/V_Addr_Mode(TEXTURE_ADDRESS_CLAMP): device-only, not translated
        self.current_min_water_opacity = self.globals.min_water_opacity;
        self.deferred.push(BaseHeightMapDeferred::DestAlphaLutChanged);
    }

    /// `BaseHeightMapRenderObjClass::renderShoreLinesSorted` (BaseHeightMap.cpp:2624): the vertex/index batches for the
    /// visible shoreline tiles, at most [`DEFAULT_MAX_BATCH_SHORELINE_TILES`] tiles per batch, each to be drawn with the
    /// destination-alpha-only state (`D3DRS_COLORWRITEENABLE = ALPHA`, the LUT as texture 0; device-only, not translated).
    /// `quick_flip_state(x, y)` is `WorldHeightMap::getQuickFlipState`.
    pub fn render_shore_lines_sorted(&mut self, window: DrawWindow, quick_flip_state: &impl Fn(i32, i32) -> bool) -> Vec<ShoreLineBatch> {
        self.num_visible_shore_line_tiles = 0;
        let mut out = Vec::new();
        if !self.globals.show_soft_water_edge || self.globals.transparent_water_depth == 0.0 || self.num_shore_line_tiles == 0 {
            return out;
        }
        if !self.globals.back_buffer_a8r8g8b8 {
            return out; // can't apply effect on cards without destination alpha
        }
        let (xe, ye, _) = self.map_extent();
        let batch_verts = DEFAULT_MAX_BATCH_SHORELINE_TILES * 4;
        let mut draw_edge_y = window.org_y + window.height - 1;
        let mut draw_edge_x = window.org_x + window.width - 1;
        if draw_edge_x > xe - 1 {
            draw_edge_x = xe - 1;
        }
        if draw_edge_y > ye - 1 {
            draw_edge_y = ye - 1;
        }
        let mut draw_start_x = window.org_x;
        let mut draw_start_y = window.org_y;
        if self.shore_line_sort_infos_x_major {
            // map is wider than taller.
            if self.shore_line_tile_sort_min_coordinate > draw_start_x {
                draw_start_x = self.shore_line_tile_sort_min_coordinate;
            }
            if (self.shore_line_tile_sort_max_coordinate + 1) < draw_edge_x {
                draw_edge_x = self.shore_line_tile_sort_max_coordinate + 1;
            }
            if (draw_edge_x - draw_start_x) <= 0 {
                return out; // nothing to draw
            }
        } else {
            if self.shore_line_tile_sort_min_coordinate > draw_start_y {
                draw_start_y = self.shore_line_tile_sort_min_coordinate;
            }
            if (self.shore_line_tile_sort_max_coordinate + 1) < draw_edge_y {
                draw_edge_y = self.shore_line_tile_sort_max_coordinate + 1;
            }
            if (draw_edge_y - draw_start_y) <= 0 {
                return out; // nothing to draw
            }
        }

        let mut last_rendered_tile: i32 = 0;
        let mut is_done = false;
        while !is_done {
            let mut batch = ShoreLineBatch::default();
            let x_major = self.shore_line_sort_infos_x_major;
            // The axis being swept (x for x-major) and the other coordinate.
            let (start_a, edge_a, start_b, edge_b) =
                if x_major { (draw_start_x, draw_edge_x, draw_start_y, draw_edge_y) } else { (draw_start_y, draw_edge_y, draw_start_x, draw_edge_x) };
            let mut a = start_a;
            let mut flushed = false;
            'sweep: while a < edge_a {
                let sort_info = self.shore_line_sort_infos[a as usize];
                if sort_info.num_tiles == 0 {
                    a += 1;
                    continue; // no tiles in this column/row.
                }
                let mut start_b2 = start_b;
                if sort_info.min_tile_coordinate as i32 > start_b2 {
                    start_b2 = sort_info.min_tile_coordinate as i32;
                }
                let mut edge_b2 = edge_b;
                if (sort_info.max_tile_coordinate as i32 + 1) < edge_b2 {
                    edge_b2 = sort_info.max_tile_coordinate as i32 + 1;
                }
                if (edge_b2 - start_b2) <= 0 {
                    a += 1;
                    continue; // no tiles visible in this column/row.
                }
                let mut shore_idx = (sort_info.tile_start_index + last_rendered_tile) as usize;
                let mut k = last_rendered_tile;
                while k < sort_info.num_tiles {
                    let info = self.shore_line_tile_positions[shore_idx];
                    let tile_b = if x_major { info.m_xy >> 16 } else { info.m_xy & 0xffff };
                    if tile_b < start_b2 {
                        shore_idx += 1; // advance to next tile.
                        k += 1;
                        continue; // this tile is not visible
                    }
                    if tile_b >= edge_b2 {
                        break; // since tiles are sorted, there will not be any visible ones after this one.
                    }
                    if batch.vertices.len() >= batch_verts {
                        last_rendered_tile = k;
                        flushed = true;
                        break 'sweep; // goto flushVertexBuffer
                    }
                    let vertex_count = batch.vertices.len() as u16;
                    for c in 0..4 {
                        batch.vertices.push(VertexFormatXYZNDUV2 {
                            x: info.verts[c * 3],
                            y: info.verts[c * 3 + 1],
                            z: info.verts[c * 3 + 2],
                            nx: 0.0, // filling these to keep AGP write buffer happy.
                            ny: 0.0,
                            nz: 0.0,
                            diffuse: 0,
                            u1: info.t[c],
                            v1: 0.0,
                            u2: 0.0,
                            v2: 0.0,
                        });
                    }
                    let flip = if x_major { quick_flip_state(a, tile_b) } else { quick_flip_state(tile_b, a) };
                    if flip {
                        batch.indices.extend_from_slice(&[1 + vertex_count, 3 + vertex_count, vertex_count, 1 + vertex_count, 2 + vertex_count, 3 + vertex_count]);
                    } else {
                        batch.indices.extend_from_slice(&[vertex_count, 2 + vertex_count, 3 + vertex_count, vertex_count, 1 + vertex_count, 2 + vertex_count]);
                    }
                    shore_idx += 1; // advance to next tile.
                    k += 1;
                } // looping over tiles in column
                last_rendered_tile = 0;
                a += 1;
            } // looping over all visible columns.
            // flushVertexBuffer: record how far we've moved so far
            if x_major {
                draw_start_x = a;
                is_done = a >= draw_edge_x;
            } else {
                draw_start_y = a;
                is_done = a >= draw_edge_y;
            }
            let _ = flushed;
            if !batch.indices.is_empty() && !batch.vertices.is_empty() {
                // DX8Wrapper::Draw_Triangles(0, indexCount/3, 0, vertexCount): the caller draws this batch.
                self.num_visible_shore_line_tiles += (batch.indices.len() / 6) as i32;
                out.push(batch);
            }
        } // for all shore tiles
        // Set_DX8_Render_State(D3DRS_COLORWRITEENABLE, B|G|R); ShaderClass::Invalidate(): device-only, not translated
        out
    }

    // --- scorches -------------------------------------------------------------------------------------------------

    /// `BaseHeightMapRenderObjClass::freeScorchBuffers` (BaseHeightMap.cpp:1878).
    pub fn free_scorch_buffers(&mut self) {
        self.has_scorch_buffers = false;
        self.scorch_vertices.clear();
        self.scorch_indices.clear();
    }

    /// `BaseHeightMapRenderObjClass::allocateScorchBuffers` (BaseHeightMap.cpp:1890). The `_DEBUG` test scorches are not
    /// added (release build).
    pub fn allocate_scorch_buffers(&mut self) {
        self.has_scorch_buffers = true;
        self.scorch_vertices = Vec::with_capacity(MAX_SCORCH_VERTEX);
        self.scorch_indices = Vec::with_capacity(MAX_SCORCH_INDEX);
        self.scorches_in_buffer = 0; // If we just allocated the buffers, we got no scorches in the buffer.
        self.cur_num_scorch_vertices = 0;
        self.cur_num_scorch_indices = 0;
    }

    /// `BaseHeightMapRenderObjClass::updateScorches` (BaseHeightMap.cpp:1913): rebuilds the scorch vertex and index
    /// data, newest scorch first, each as a patch of terrain cells textured with its mark. `get_clip_height(x, y)` is
    /// `getClipHeight` (index with the border added), `get_flip_state(x, y)` is `WorldHeightMap::getFlipState`.
    pub fn update_scorches(&mut self, get_clip_height: &impl Fn(i32, i32) -> i32, get_flip_state: &impl Fn(i32, i32) -> bool) {
        if self.scorches_in_buffer > 1 {
            return;
        }
        if self.scorches.is_empty() {
            return;
        }
        if !self.has_scorch_buffers {
            return;
        }
        self.cur_num_scorch_vertices = 0;
        self.cur_num_scorch_indices = 0;
        self.scorch_vertices.clear();
        self.scorch_indices.clear();
        let (x_extent, y_extent, border) = self.map_extent();

        let mut shade_r = self.globals.terrain_ambient0.red;
        let mut shade_g = self.globals.terrain_ambient0.green;
        let mut shade_b = self.globals.terrain_ambient0.blue;
        shade_r += self.globals.terrain_diffuse0.red / 2.0;
        shade_g += self.globals.terrain_diffuse0.green / 2.0;
        shade_b += self.globals.terrain_diffuse0.blue / 2.0;
        shade_r *= 255.0;
        shade_g *= 255.0;
        shade_b *= 255.0;
        let diffuse = real_to_int(shade_b) | (real_to_int(shade_g) << 8) | (real_to_int(shade_r) << 16) | (255i32 << 24);

        self.scorches_in_buffer = 0;
        let mut cur_scorch = self.scorches.len() as i32 - 1;
        while cur_scorch >= 0 {
            self.scorches_in_buffer += 1;
            let s = self.scorches[cur_scorch as usize];
            let radius = s.radius;
            let loc = s.location;
            let mut ty = s.scorch_type;
            if ty < 0 {
                ty = 0;
            }
            if ty >= SCORCH_MARKS_IN_TEXTURE {
                ty = 0;
            }
            let amt_to_float = MAP_HEIGHT_SCALE / 10.0;
            let mut min_x = real_to_int_floor((loc.x - radius) / MAP_XY_FACTOR);
            let mut min_y = real_to_int_floor((loc.y - radius) / MAP_XY_FACTOR);
            if min_x < -border {
                min_x = -border;
            }
            if min_y < -border {
                min_y = -border;
            }
            let mut max_x = real_to_int_ceil((loc.x + radius) / MAP_XY_FACTOR);
            let mut max_y = real_to_int_ceil((loc.y + radius) / MAP_XY_FACTOR);
            max_x += 1;
            max_y += 1;
            if max_x > x_extent - border {
                max_x = x_extent - border;
            }
            if max_y > y_extent - border {
                max_y = y_extent - border;
            }
            let start_vertex = self.cur_num_scorch_vertices;
            for j in min_y..max_y {
                for i in min_x..max_x {
                    if self.cur_num_scorch_vertices as usize >= MAX_SCORCH_VERTEX {
                        return;
                    }
                    let the_z = amt_to_float + (get_clip_height(i + border, j + border) as f32 * MAP_HEIGHT_SCALE);
                    let u_offset = (ty % SCORCH_PER_ROW) as f32 * 1.5;
                    let v_offset = (ty / SCORCH_PER_ROW) as f32 * 1.5;
                    let x = i as f32 * MAP_XY_FACTOR;
                    let y = j as f32 * MAP_XY_FACTOR;
                    self.scorch_vertices.push(VertexFormatXYZDUV1 {
                        x,
                        y,
                        z: the_z,
                        diffuse: diffuse as u32,
                        u1: (u_offset + 0.5 + (x - loc.x) / (2.0 * radius)) / (SCORCH_PER_ROW + 1) as f32,
                        v1: (v_offset + 0.5 + (y - loc.y) / (2.0 * radius)) / (SCORCH_PER_ROW + 1) as f32,
                    });
                    self.cur_num_scorch_vertices += 1;
                }
            }
            let y_offset = max_x - min_x;
            for j in 0..(max_y - min_y - 1) {
                for i in 0..(max_x - min_x - 1) {
                    if self.cur_num_scorch_indices as usize + 6 > MAX_SCORCH_INDEX {
                        return;
                    }
                    let x_ndx = i + min_x + border;
                    let y_ndx = j + min_y + border;
                    let flip_for_blend = get_flip_state(x_ndx, y_ndx);
                    let b = start_vertex + j * y_offset + i;
                    let idx: [i32; 6] = if flip_for_blend {
                        [b + 1, b + y_offset, b, b + 1, b + 1 + y_offset, b + y_offset]
                    } else {
                        [b, b + 1 + y_offset, b + y_offset, b, b + 1, b + 1 + y_offset]
                    };
                    for v in idx {
                        self.scorch_indices.push(v as u16);
                    }
                    self.cur_num_scorch_indices += 6;
                }
            }
            cur_scorch -= 1;
        }
    }

    /// `BaseHeightMapRenderObjClass::clearAllScorches` (BaseHeightMap.cpp:2039).
    pub fn clear_all_scorches(&mut self) {
        self.scorches.clear();
        self.scorches_in_buffer = 0;
    }

    /// `BaseHeightMapRenderObjClass::addScorch` (BaseHeightMap.cpp:2052), included because `updateScorches` depends on its
    /// invariants: oldest dropped at `MAX_SCORCH_MARKS`; a near-duplicate is ignored. Upstream uses the integer `abs`
    /// on reals (`abs(location.X - ...)`), which truncates the difference to an `int` before comparing: kept.
    pub fn add_scorch(&mut self, location: Vector3, radius: f32, scorch_type: i32) {
        if self.scorches.len() >= MAX_SCORCH_MARKS {
            self.scorches.remove(0);
        }
        let limit = radius / 4.0;
        for s in &self.scorches {
            if (((location.x - s.location.x) as i32).abs() as f32) < limit
                && (((location.y - s.location.y) as i32).abs() as f32) < limit
                && (((radius - s.radius) as i32).abs() as f32) < limit
                && s.scorch_type == scorch_type
            {
                return; // basically a duplicate.
            }
        }
        self.scorches.push(TScorch { location, radius, scorch_type });
        self.scorches_in_buffer = 0; // force buffer regenerations.
    }

    // --- forwarders to the buffers --------------------------------------------------------------------------------

    /// `BaseHeightMapRenderObjClass::removeAllTrees` (BaseHeightMap.cpp:2224).
    pub fn remove_all_trees(&mut self) {
        if let Some(t) = self.tree_buffer.as_mut() {
            t.clear_all_trees();
        }
    }

    /// `BaseHeightMapRenderObjClass::removeAllProps` (BaseHeightMap.cpp:2275).
    pub fn remove_all_props(&mut self) {
        if let Some(p) = self.prop_buffer.as_mut() {
            p.clear_all_props();
        }
    }

    /// `BaseHeightMapRenderObjClass::addTerrainBib` (BaseHeightMap.cpp:2299). Upstream does not test `m_bibBuffer`.
    pub fn add_terrain_bib(&mut self, corners: &[Vector3; 4], id: rts_lockstep_engine::common::game_type::ObjectId, highlight: bool) {
        self.bib_buffer.as_mut().expect("m_bibBuffer").add_bib(corners, id, highlight);
    }

    /// `BaseHeightMapRenderObjClass::addTerrainBibDrawable` (BaseHeightMap.cpp:2310).
    pub fn add_terrain_bib_drawable(&mut self, corners: &[Vector3; 4], id: rts_lockstep_engine::common::game_type::DrawableId, highlight: bool) {
        self.bib_buffer.as_mut().expect("m_bibBuffer").add_bib_drawable(corners, id, highlight);
    }

    /// `BaseHeightMapRenderObjClass::removeTerrainBibHighlighting` (BaseHeightMap.cpp:2321).
    pub fn remove_terrain_bib_highlighting(&mut self) {
        self.bib_buffer.as_mut().expect("m_bibBuffer").remove_highlighting();
    }

    /// `BaseHeightMapRenderObjClass::removeAllTerrainBibs` (BaseHeightMap.cpp:2331).
    pub fn remove_all_terrain_bibs(&mut self) {
        self.bib_buffer.as_mut().expect("m_bibBuffer").clear_all_bibs();
    }

    /// `BaseHeightMapRenderObjClass::removeTerrainBib` (BaseHeightMap.cpp:2341).
    pub fn remove_terrain_bib(&mut self, id: rts_lockstep_engine::common::game_type::ObjectId) {
        self.bib_buffer.as_mut().expect("m_bibBuffer").remove_bib(id);
    }

    /// `BaseHeightMapRenderObjClass::removeTerrainBibDrawable` (BaseHeightMap.cpp:2351).
    pub fn remove_terrain_bib_drawable(&mut self, id: rts_lockstep_engine::common::game_type::DrawableId) {
        self.bib_buffer.as_mut().expect("m_bibBuffer").remove_bib_drawable(id);
    }

    /// `BaseHeightMapRenderObjClass::renderTrees` (BaseHeightMap.cpp:2969): true when the tree buffer should draw
    /// (`m_map` and the scene exist and there is a tree buffer); the draw itself (`m_treeBuffer->drawTrees(camera,
    /// &dynamicLights)` under the object's transform and vertex material) is the caller's, device-only. Upstream
    /// `EXTENDED_STATS` is not defined.
    pub fn render_trees(&self, scene_exists: bool) -> bool {
        if self.map.is_none() {
            return false;
        }
        if !scene_exists {
            return false;
        }
        self.tree_buffer.is_some()
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The contract FlatHeightMapRenderObjClass is generic over
// -------------------------------------------------------------------------------------------------------------------

impl<W: WaterHeightQuery> BaseHeightMapRenderObj for BaseHeightMapRenderObjClass<W> {
    /// `BaseHeightMapRenderObjClass::freeMapResources` (BaseHeightMap.cpp:156): scorch buffers, vertex material, the four
    /// stage textures, the destination-alpha texture and `m_map` are released.
    fn free_map_resources(&mut self) {
        self.free_scorch_buffers();
        self.has_stage_two_texture = false;
        self.has_dest_alpha_texture = false;
        self.deferred.push(BaseHeightMapDeferred::FreeStageTextures);
        self.map = None;
    }

    /// `BaseHeightMapRenderObjClass::adjustTerrainLOD` (BaseHeightMap.cpp:341): steps `m_terrainLOD` (automatic becomes the
    /// maximum), then asks the caller to swap the terrain object (`HeightMapRenderObjClass` at LOD 7, none at 5,
    /// else `FlatHeightMapRenderObjClass`) and re-init it with the draw window of `m_map`, which is a scene operation.
    fn adjust_terrain_lod(&mut self, adj: i32) {
        if adj > 0 && self.globals.terrain_lod < TERRAIN_LOD_MAX {
            self.globals.terrain_lod += 1;
        }
        if adj < 0 && self.globals.terrain_lod > TERRAIN_LOD_MIN {
            self.globals.terrain_lod -= 1;
        }
        if self.globals.terrain_lod == TERRAIN_LOD_AUTOMATIC {
            self.globals.terrain_lod = TERRAIN_LOD_MAX;
        }
        if self.map.is_none() {
            return;
        }
        if self.has_shroud {
            self.deferred.push(BaseHeightMapDeferred::ShroudReset); // need reset here since initHeightData will load new shroud.
        }
        let mut kind = if self.globals.terrain_lod == 7 { Some(TerrainObjectKind::HeightMap) } else { Some(TerrainObjectKind::FlatHeightMap) };
        if self.globals.terrain_lod == 5 {
            kind = None;
        }
        self.deferred.push(BaseHeightMapDeferred::SwitchTerrainObject(kind));
        if kind.is_some() {
            // newROBJ->initHeightData(drawWidth, drawHeight, m_map, NULL); TheTerrainRenderObject = newROBJ;
            // newROBJ->staticLightingChanged(); newROBJ->m_roadBuffer->loadRoads(): done by the caller on the new object.
            self.deferred.push(BaseHeightMapDeferred::RoadAllocateAndLoad);
        }
        self.deferred.push(BaseHeightMapDeferred::TacticalViewJiggleAngle);
    }

    /// `BaseHeightMapRenderObjClass::ReleaseResources` (BaseHeightMap.cpp:400).
    fn release_resources(&mut self) {
        if let Some(t) = self.tree_buffer.as_mut() {
            t.free_tree_buffers();
        }
        if let Some(b) = self.bib_buffer.as_mut() {
            b.free_bib_buffers();
        }
        if let Some(b) = self.bridge_buffer.as_mut() {
            b.free_bridge_buffers();
        }
        if let Some(w) = self.waypoint_buffer.as_mut() {
            w.free_waypoint_buffers();
        }
        let p_map = self.map.clone(); // REF_PTR_SET(pMap, m_map)
        self.free_map_resources();
        self.map = p_map; // ref_ptr_set has already incremented the ref count.
        self.deferred.push(BaseHeightMapDeferred::WaterReleaseResources);
        self.deferred.push(BaseHeightMapDeferred::TerrainTracksReleaseResources);
        self.deferred.push(BaseHeightMapDeferred::ShadowManagerReleaseResources);
        if self.has_shroud {
            self.deferred.push(BaseHeightMapDeferred::ShroudReset);
            self.deferred.push(BaseHeightMapDeferred::ShroudReleaseResources);
        }
        self.deferred.push(BaseHeightMapDeferred::SmudgeReleaseResources);
        self.deferred.push(BaseHeightMapDeferred::SnowReleaseResources);
        self.deferred.push(BaseHeightMapDeferred::ShaderManagerShutdown);
        self.deferred.push(BaseHeightMapDeferred::RoadFreeBuffers); // DO_ROADS
    }

    /// `BaseHeightMapRenderObjClass::ReAcquireResources` (BaseHeightMap.cpp:452).
    fn re_acquire_resources(&mut self) {
        self.deferred.push(BaseHeightMapDeferred::ShaderManagerInit); // reacquire resources needed by custom shaders
        self.deferred.push(BaseHeightMapDeferred::WaterReAcquireResources);
        self.deferred.push(BaseHeightMapDeferred::TerrainTracksReAcquireResources);
        self.deferred.push(BaseHeightMapDeferred::ShadowManagerReAcquireResources);
        if self.has_shroud {
            self.deferred.push(BaseHeightMapDeferred::ShroudReAcquireResources);
        }
        if let Some(map) = self.map.clone() {
            let (x, y) = (self.x, self.y);
            self.init_height_data(x, y, &map);
            self.need_full_update = true;
        }
        if let Some(t) = self.tree_buffer.as_mut() {
            t.allocate_tree_buffers();
        }
        if let Some(b) = self.bib_buffer.as_mut() {
            b.allocate_bib_buffers();
        }
        if let Some(b) = self.bridge_buffer.as_mut() {
            b.allocate_bridge_buffers();
        }
        self.deferred.push(BaseHeightMapDeferred::SmudgeReAcquireResources);
        self.deferred.push(BaseHeightMapDeferred::SnowReAcquireResources);
        self.deferred.push(BaseHeightMapDeferred::RoadAllocateAndLoad); // DO_ROADS: allocateRoadBuffers(); loadRoads()
        self.deferred.push(BaseHeightMapDeferred::TacticalViewForceRedraw); // force map to update itself for the current camera
        self.deferred.push(BaseHeightMapDeferred::TacticalViewForceRedraw);
    }

    /// `BaseHeightMapRenderObjClass::reset` (BaseHeightMap.cpp:642).
    fn reset(&mut self) {
        self.remove_all_trees();
        self.remove_all_props();
        self.clear_all_scorches();
        self.deferred.push(BaseHeightMapDeferred::RoadClearAll); // DO_ROADS
        if let Some(b) = self.bridge_buffer.as_mut() {
            b.clear_all_bridges();
        }
        if let Some(b) = self.bib_buffer.as_mut() {
            b.clear_all_bibs();
        }
        self.show_as_visible_cliff.clear();
        if self.has_shroud {
            self.deferred.push(BaseHeightMapDeferred::ShroudReset);
            // assume border is always black at start.
            self.deferred.push(BaseHeightMapDeferred::ShroudBorderLevel(self.globals.shroud_alpha));
        }
    }

    /// `BaseHeightMapRenderObjClass::On_Frame_Update` (BaseHeightMap.cpp:2162): empty upstream.
    fn on_frame_update(&mut self) {}

    /// `BaseHeightMapRenderObjClass::initHeightData` (BaseHeightMap.cpp:1785) with `updateExtraPassTiles` true (the
    /// default), returning 0.
    fn init_height_data(&mut self, _x: i32, _y: i32, map: &SharedHeightMap) -> i32 {
        self.map = Some(map.clone()); // REF_PTR_SET(m_map, pMap)
        if self.has_shroud {
            self.deferred.push(BaseHeightMapDeferred::ShroudInit { partition_cell_size: 0 }); // caller fills TheGlobalData->m_partitionCellSize
        }
        self.deferred.push(BaseHeightMapDeferred::RoadSetMap); // DO_ROADS: m_roadBuffer->setMap(m_map)
        let (xe, ye, border) = self.map_extent();
        if self.tree_buffer.is_some() {
            self.deferred.push(BaseHeightMapDeferred::TreeSetBounds {
                hi_x: (xe - 2 * border) as f32 * MAP_XY_FACTOR,
                hi_y: (ye - 2 * border) as f32 * MAP_XY_FACTOR,
            });
        }
        // updateExtraPassTiles
        self.num_shore_line_tiles = 0;
        {
            let mut min_ht = MAX_HEIGHT_VALUE;
            let mut max_ht = 0;
            {
                let m = map.read().unwrap();
                for j in 0..ye {
                    for i in 0..xe {
                        let cur = m.get_height(i, j) as i32;
                        if cur < min_ht {
                            min_ht = cur;
                        }
                        if max_ht < cur {
                            max_ht = cur;
                        }
                    }
                }
            }
            self.min_height = min_ht as f32 * MAP_HEIGHT_SCALE;
            self.max_height = max_ht as f32 * MAP_HEIGHT_SCALE;
            self.update_shoreline_tiles(0, 0, xe - 1, ye - 1);
            if self.globals.min_water_opacity != self.current_min_water_opacity {
                self.init_dest_alpha_lut();
            }
        }
        // Set_Force_Visible(TRUE): terrain is always visible (render object flag; device-only)
        self.need_full_update = true;
        self.scorches_in_buffer = 0;
        self.cur_num_scorch_vertices = 0;
        self.cur_num_scorch_indices = 0;
        let need_to_allocate = !self.has_stage_two_texture;
        let has_data = !map.read().unwrap().data.is_empty();
        if has_data && need_to_allocate {
            // requested heightmap different from old one.
            self.free_map_resources(); // free old data and ib/vb
            self.map = Some(map.clone()); // update our heightmap pointer in case it changed since last call.
            self.has_stage_two_texture = true; // CloudMapTerrainTextureClass
            self.deferred.push(BaseHeightMapDeferred::NewMacroTexture(self.macro_texture_name.clone()));
            self.has_dest_alpha_texture = true; // TextureClass(256, 1, A8R8G8B8, MIP_LEVELS_1)
            self.init_dest_alpha_lut();
            self.allocate_scorch_buffers();
            self.deferred.push(BaseHeightMapDeferred::AllocateStageTextures);
        }
        0
    }

    /// `BaseHeightMapRenderObjClass::staticLightingChanged` (BaseHeightMap.cpp:2361).
    fn static_lighting_changed(&mut self) {
        self.need_full_update = true;
        self.scorches_in_buffer = 0; // If we just allocated the buffers, we got no scorches in the buffer.
        self.cur_num_scorch_vertices = 0;
        self.cur_num_scorch_indices = 0;
        self.deferred.push(BaseHeightMapDeferred::RoadUpdateLighting);
    }

    /// `BaseHeightMapRenderObjClass::updateCenter` (BaseHeightMap.cpp:2405). The bridge buffer's `updateCenter` takes the
    /// camera and lights iterator, so it is recorded for the caller.
    fn update_center(&mut self) {
        if self.map.is_none() {
            return;
        }
        if self.updating {
            return;
        }
        if self.tree_buffer.is_some() {
            self.deferred.push(BaseHeightMapDeferred::TreeDoFullUpdate); // Tell the trees to update for view change.
        }
        if self.prop_buffer.is_some() {
            self.deferred.push(BaseHeightMapDeferred::PropDoFullUpdate);
        }
        self.updating = true;
        self.deferred.push(BaseHeightMapDeferred::RoadUpdateCenter); // DO_ROADS
        if self.need_full_update {
            self.deferred.push(BaseHeightMapDeferred::BridgeDoFullUpdate);
            self.deferred.push(BaseHeightMapDeferred::BridgeUpdateCenter);
            self.updating = false;
            return;
        }
        self.deferred.push(BaseHeightMapDeferred::BridgeUpdateCenter);
        self.updating = false;
    }

    fn map(&self) -> Option<SharedHeightMap> {
        self.map.clone()
    }

    fn set_map(&mut self, map: Option<SharedHeightMap>) {
        self.map = map;
    }

    fn set_need_full_update(&mut self, value: bool) {
        self.need_full_update = value;
    }
}

/// `Coord3D` is the position type of tree/prop calls; re-exported for the callers of the forwarders.
pub type TreePosition = Coord3D;
