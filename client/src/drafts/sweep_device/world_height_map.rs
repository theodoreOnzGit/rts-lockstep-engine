//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/TileData.cpp
//!
//! Sweep 2 (#213): only the members whose names appear nowhere in the verified port
//! (`src/client/terrain_height_map.rs`, `TerrainHeightMap`) are drafted. The verified `TerrainHeightMap` has no seismic
//! arrays, no cell-cliff bitmap, no per-class `globalTextureClass`, no draw-window sizes and a `TileData` with one mip
//! level only, so those live here in [`WorldHeightMapExtras`] and [`TileDataMips`], to be merged into the verified type
//! when it is compiled. Functions that need the verified map's tile lists take it as `&TerrainHeightMap`.
//!
//! Translated already (not drafted): `WorldHeightMap::getCliffState`, `getUVData`, `clearFlipStates`,
//! `ParseWorldDictDataChunk` (not present by that name; world dict reading is in `src/logic/world/map_load_world.rs`),
//! `TileData::updateMips`/`doMip` (`TileData::do_mip`). Cut points: `getTerrainTexture`, `getFlatTexture`
//! (device textures, `TerrainTextureClass`) and `MapObject::setRenderObj`/`setBridgeRenderObject`/`getBridgeRenderObject`
//! are kept as handles (see `MapObjectRenderHandles`).

use crate::drafts::terrain::w3d_common::RenderObjHandle;
use rts_lockstep_engine::client::terrain_height_map::{
    BlendTileInfo, TerrainHeightMap, TileData, DATA_LEN_BYTES, NUM_BLEND_TILES, NUM_SOURCE_TILES, TILE_BYTES_PER_PIXEL,
    TILE_PIXEL_EXTENT,
};
use rts_lockstep_engine::common::base_type::RGBColor;
use rts_lockstep_engine::common::dict::{DataType, Dict};
use rts_lockstep_engine::common::global_data::{GlobalData, MAX_GLOBAL_LIGHTS};
use rts_lockstep_engine::common::game_type::TIME_OF_DAY_FIRST;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::formats::data_chunk::{ChunkResult, DataChunkInfo, DataChunkInput};
use rts_lockstep_engine::logic::map_object::MapObject;
use rts_lockstep_engine::logic::terrain_visual::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR, PATHFIND_CLIFF_SLOPE_LIMIT_F};

/// `K_OBJECTS_VERSION_2` (MapReaderWriterInfo.h).
pub const K_OBJECTS_VERSION_2: u16 = 2;
/// `K_LIGHTING_VERSION_2`: added 2 additional global lights for objects.
pub const K_LIGHTING_VERSION_2: u16 = 2;
/// `K_LIGHTING_VERSION_3`: added 2 additional global lights for terrain.
pub const K_LIGHTING_VERSION_3: u16 = 3;

/// `NUM_ALPHA_TILES` (WorldHeightMap.h).
pub const NUM_ALPHA_TILES: usize = 12;
const K_HORIZ: usize = 0;
const K_VERT: usize = 1;
const K_LDIAG: usize = 2;
const K_RDIAG: usize = 3;
const K_LLDIAG: usize = 4;
const K_LRDIAG: usize = 5;
const K_INV: usize = 6;

// ---------------------------------------------------------------------------------------------------------------------
// MapObject render-object handles
// ---------------------------------------------------------------------------------------------------------------------

/// `BRIDGE_MAX_TOWERS` (BridgeTowerType: FROM_LEFT, FROM_RIGHT, TO_LEFT, TO_RIGHT).
pub const BRIDGE_MAX_TOWERS: usize = 4;

/// The render-object members `MapObject` carries in the client build (`m_renderObj`, `m_bridgeTowers[]`), which the
/// verified logic `MapObject` (`src/logic/map_object.rs`) leaves out.
#[derive(Clone, Debug, Default)]
pub struct MapObjectRenderHandles {
    /// `m_renderObj`.
    pub render_obj: Option<RenderObjHandle>,
    /// `m_bridgeTowers[BRIDGE_MAX_TOWERS]`.
    pub bridge_towers: [Option<RenderObjHandle>; BRIDGE_MAX_TOWERS],
}

impl MapObjectRenderHandles {
    /// `MapObject::setRenderObj` (WorldHeightMap.cpp:154): `REF_PTR_SET(m_renderObj, pObj)`.
    pub fn set_render_obj(&mut self, obj: Option<RenderObjHandle>) {
        self.render_obj = obj;
    }

    /// `MapObject::setBridgeRenderObject` (WorldHeightMap.cpp:159): ignores an out-of-range tower type.
    pub fn set_bridge_render_object(&mut self, tower_type: i32, render_obj: Option<RenderObjHandle>) {
        if tower_type >= 0 && (tower_type as usize) < BRIDGE_MAX_TOWERS {
            self.bridge_towers[tower_type as usize] = render_obj;
        }
    }

    /// `MapObject::getBridgeRenderObject` (WorldHeightMap.cpp:167): NULL for an out-of-range tower type.
    pub fn get_bridge_render_object(&self, tower_type: i32) -> Option<&RenderObjHandle> {
        if tower_type >= 0 && (tower_type as usize) < BRIDGE_MAX_TOWERS {
            return self.bridge_towers[tower_type as usize].as_ref();
        }
        None
    }
}

/// `MapObject::setThingTemplate` (WorldHeightMap.cpp:335): `m_thingTemplate = thing; m_objectName = thing->getName();`.
/// The verified `MapObject` keeps the template as its name, so the name is both. Returns the new
/// `(object_name, thing_template)` pair for the caller to store (the verified type has no setter; upstream is a
/// two-line body, so extending `MapObject` is a one-line change once it is open to edit).
pub fn map_object_set_thing_template(template_name: &[u8]) -> (Vec<u8>, Option<Vec<u8>>) {
    (template_name.to_vec(), Some(template_name.to_vec()))
}

/// `WorldHeightMap::freeListOfMapObjects` (WorldHeightMap.cpp:431): deletes the whole object list
/// (`MapObject::TheMapObjectListPtr->deleteInstance()`, set NULL) and clears the world dictionary
/// (`MapObject::getWorldDict()->clear()`).
pub fn free_list_of_map_objects(objects: &mut Vec<MapObject>, world_dict: &mut Dict) {
    objects.clear();
    *world_dict = Dict::new();
}

// ---------------------------------------------------------------------------------------------------------------------
// TileData mips
// ---------------------------------------------------------------------------------------------------------------------

/// `TileData` with the whole mip chain (`m_tileData`, `m_tileDataMip32` .. `m_tileDataMip1`), BGRA.
#[derive(Clone, Debug, PartialEq)]
pub struct TileDataMips {
    pub tile_data: Vec<u8>,
    pub mip32: Vec<u8>,
    pub mip16: Vec<u8>,
    pub mip8: Vec<u8>,
    pub mip4: Vec<u8>,
    pub mip2: Vec<u8>,
    pub mip1: Vec<u8>,
}

impl Default for TileDataMips {
    fn default() -> Self {
        TileDataMips {
            tile_data: vec![0; DATA_LEN_BYTES],
            mip32: vec![0; 32 * 32 * TILE_BYTES_PER_PIXEL],
            mip16: vec![0; 16 * 16 * TILE_BYTES_PER_PIXEL],
            mip8: vec![0; 8 * 8 * TILE_BYTES_PER_PIXEL],
            mip4: vec![0; 4 * 4 * TILE_BYTES_PER_PIXEL],
            mip2: vec![0; 2 * 2 * TILE_BYTES_PER_PIXEL],
            mip1: vec![0; TILE_BYTES_PER_PIXEL],
        }
    }
}

impl TileDataMips {
    /// `TileData::hasRGBDataForWidth` (TileData.cpp:53): true for the 64, 32, 16, 8, 4, 2 and 1 pixel widths.
    pub fn has_rgb_data_for_width(width: i32) -> bool {
        width == TILE_PIXEL_EXTENT || width == 32 || width == 16 || width == 8 || width == 4 || width == 2 || width == 1
    }

    /// `TileData::getRGBDataForWidth`: the 64 pixel data for any width that is not a mip width.
    pub fn get_rgb_data_for_width(&self, width: i32) -> &[u8] {
        match width {
            32 => &self.mip32,
            16 => &self.mip16,
            8 => &self.mip8,
            4 => &self.mip4,
            2 => &self.mip2,
            1 => &self.mip1,
            _ => &self.tile_data,
        }
    }

    /// `TileData::updateMips`, using the verified `TileData::do_mip`.
    pub fn update_mips(&mut self) {
        self.mip32 = TileData::do_mip(&self.tile_data, TILE_PIXEL_EXTENT);
        self.mip16 = TileData::do_mip(&self.mip32, 32);
        self.mip8 = TileData::do_mip(&self.mip16, 16);
        self.mip4 = TileData::do_mip(&self.mip8, 8);
        self.mip2 = TileData::do_mip(&self.mip4, 4);
        self.mip1 = TileData::do_mip(&self.mip2, 2);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// WorldHeightMap extras
// ---------------------------------------------------------------------------------------------------------------------

/// The `WorldHeightMap` members the verified `TerrainHeightMap` lacks.
#[derive(Clone, Debug, Default)]
pub struct WorldHeightMapExtras {
    pub width: i32,
    pub height: i32,
    /// `m_seismicUpdateFlag` (1 bit per cell, rows `seismic_update_width` bytes).
    pub seismic_update_flag: Vec<u8>,
    pub seismic_update_width: i32,
    /// `m_seismicZVelocities`, `width * height` reals.
    pub seismic_z_velocities: Vec<f32>,
    /// `m_cellCliffState` (rows `flip_state_width` bytes).
    pub cell_cliff_state: Vec<u8>,
    pub flip_state_width: i32,
    /// `m_drawWidthX` / `m_drawHeightY`.
    pub draw_width_x: i32,
    pub draw_height_y: i32,
    /// `m_textureClasses[i].globalTextureClass`, one per texture class.
    pub global_texture_class: Vec<i32>,
    /// `m_alphaTiles[NUM_ALPHA_TILES]` (None until `setupAlphaTiles`).
    pub alpha_tiles: Vec<Option<TileDataMips>>,
}

/// `STRETCH_DRAW_WIDTH` / `STRETCH_DRAW_HEIGHT` (WorldHeightMap.h).
pub const STRETCH_DRAW_WIDTH: i32 = 129;
pub const STRETCH_DRAW_HEIGHT: i32 = 129;

impl WorldHeightMapExtras {
    /// `WorldHeightMap::getSeismicUpdateFlag` (WorldHeightMap.cpp:602).
    pub fn get_seismic_update_flag(&self, x_index: i32, y_index: i32) -> bool {
        if x_index < 0 || y_index < 0 {
            return false;
        }
        if y_index >= self.height {
            return false;
        }
        if x_index >= self.width {
            return false;
        }
        if self.seismic_update_flag.is_empty() {
            return false;
        }
        self.seismic_update_flag[(y_index * self.seismic_update_width + (x_index >> 3)) as usize] & (1 << (x_index & 0x7)) != 0
    }

    /// `WorldHeightMap::setSeismicUpdateFlag` (WorldHeightMap.cpp:610).
    pub fn set_seismic_update_flag(&mut self, x_index: i32, y_index: i32, value: bool) {
        if x_index < 0 || y_index < 0 {
            return;
        }
        if y_index >= self.height {
            return;
        }
        if x_index >= self.width {
            return;
        }
        if self.seismic_update_flag.is_empty() {
            return;
        }
        let cur = &mut self.seismic_update_flag[(y_index * self.seismic_update_width + (x_index >> 3)) as usize];
        if value {
            *cur |= 1 << (x_index & 0x7);
        } else {
            *cur &= !(1 << (x_index & 0x7));
        }
    }

    /// `WorldHeightMap::clearSeismicUpdateFlags` (WorldHeightMap.cpp:623).
    pub fn clear_seismic_update_flags(&mut self) {
        for b in self.seismic_update_flag.iter_mut() {
            *b = 0;
        }
    }

    /// `WorldHeightMap::getSeismicZVelocity` (WorldHeightMap.cpp:631). Upstream returns `false` (0.0) out of range.
    pub fn get_seismic_z_velocity(&self, x_index: i32, y_index: i32) -> f32 {
        if x_index < 0 || y_index < 0 {
            return 0.0;
        }
        if y_index >= self.height {
            return 0.0;
        }
        if x_index >= self.width {
            return 0.0;
        }
        if self.seismic_z_velocities.is_empty() {
            return 0.0;
        }
        self.seismic_z_velocities[(y_index * self.width + x_index) as usize]
    }

    /// `WorldHeightMap::setSeismicZVelocity` (WorldHeightMap.cpp:639).
    pub fn set_seismic_z_velocity(&mut self, x_index: i32, y_index: i32, value: f32) {
        if x_index < 0 || y_index < 0 {
            return;
        }
        if y_index >= self.height {
            return;
        }
        if x_index >= self.width {
            return;
        }
        if self.seismic_z_velocities.is_empty() {
            return;
        }
        self.seismic_z_velocities[(y_index * self.width + x_index) as usize] = value;
    }

    /// `WorldHeightMap::fillSeismicZVelocities` (WorldHeightMap.cpp:647).
    pub fn fill_seismic_z_velocities(&mut self, value: f32) {
        for v in self.seismic_z_velocities.iter_mut() {
            *v = value;
        }
    }

    /// `WorldHeightMap::getBilinearSampleSeismicZVelocity` (WorldHeightMap.cpp:654): the mean of the cell and its
    /// existing 8 neighbours, in upstream's accumulation order (bottom row, top row, left, right).
    pub fn get_bilinear_sample_seismic_z_velocity(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 {
            return 0.0;
        }
        if y >= self.height {
            return 0.0;
        }
        if x >= self.width {
            return 0.0;
        }
        if self.seismic_z_velocities.is_empty() {
            return 0.0;
        }
        let v = &self.seismic_z_velocities;
        let w = self.width;
        let at = |xx: i32, yy: i32| v[(yy * w + xx) as usize];

        let mut collector = 0.0f32;
        let mut divisor = 0.0f32;

        collector += at(x, y);
        divisor += 1.0;

        if y > 0 {
            collector += at(x, y - 1); // bottom
            divisor += 1.0;
            if x > 0 {
                collector += at(x - 1, y - 1); // lower left
                divisor += 1.0;
            }
            if x < self.width - 1 {
                collector += at(x + 1, y - 1); // lower right
                divisor += 1.0;
            }
        }
        if y < self.height - 1 {
            collector += at(x, y + 1); // top
            divisor += 1.0;
            if x > 0 {
                collector += at(x - 1, y + 1); // upper left
                divisor += 1.0;
            }
            if x < self.width - 1 {
                collector += at(x + 1, y + 1); // upper right
                divisor += 1.0;
            }
        }
        if x > 0 {
            collector += at(x - 1, y); // left
            divisor += 1.0;
        }
        if x < self.width - 1 {
            collector += at(x + 1, y); // right
            divisor += 1.0;
        }
        collector /= divisor;
        collector
    }

    /// `WorldHeightMap::getCliffState` (WorldHeightMap.cpp, translated already in spirit by the cliff bitmap; kept
    /// here because the bitmap lives in this struct).
    pub fn get_cliff_state(&self, x_index: i32, y_index: i32) -> bool {
        if x_index < 0 || y_index < 0 {
            return false;
        }
        if y_index >= self.height {
            return false;
        }
        if x_index >= self.width {
            return false;
        }
        if self.cell_cliff_state.is_empty() {
            return false;
        }
        self.cell_cliff_state[(y_index * self.flip_state_width + (x_index >> 3)) as usize] & (1 << (x_index & 0x7)) != 0
    }

    /// `WorldHeightMap::setCliffState` (WorldHeightMap.cpp:747).
    pub fn set_cliff_state(&mut self, x_index: i32, y_index: i32, state: bool) {
        if x_index < 0 || y_index < 0 {
            return;
        }
        if y_index >= self.height {
            return;
        }
        if x_index >= self.width {
            return;
        }
        if self.cell_cliff_state.is_empty() {
            return;
        }
        let ndx = (y_index * self.flip_state_width + (x_index >> 3)) as usize;
        let mut flag_byte = self.cell_cliff_state[ndx];
        let flag_mask: u8 = 1 << (x_index & 0x7);
        if state {
            flag_byte |= flag_mask;
        } else {
            flag_byte &= !flag_mask;
        }
        self.cell_cliff_state[ndx] = flag_byte;
    }

    /// `WorldHeightMap::setCellCliffFlagFromHeights` (WorldHeightMap.cpp:2312). `get_height(x, y)` is the map's
    /// `getHeight` (raw byte heights, clamped at the edge by the caller as upstream's accessor does).
    pub fn set_cell_cliff_flag_from_heights(&mut self, x_index: i32, y_index: i32, get_height: &impl Fn(i32, i32) -> u8) {
        let height1 = get_height(x_index, y_index) as f32 * MAP_HEIGHT_SCALE;
        let height2 = get_height(x_index + 1, y_index) as f32 * MAP_HEIGHT_SCALE;
        let height3 = get_height(x_index, y_index + 1) as f32 * MAP_HEIGHT_SCALE;
        let height4 = get_height(x_index + 1, y_index + 1) as f32 * MAP_HEIGHT_SCALE;
        let mut min_z = height1;
        if min_z > height2 {
            min_z = height2;
        }
        if min_z > height3 {
            min_z = height3;
        }
        if min_z > height4 {
            min_z = height4;
        }
        let mut max_z = height1;
        if max_z < height2 {
            max_z = height2;
        }
        if max_z < height3 {
            max_z = height3;
        }
        if max_z < height4 {
            max_z = height4;
        }
        let cliff_range = PATHFIND_CLIFF_SLOPE_LIMIT_F;
        let is_cliff = max_z - min_z > cliff_range;
        self.set_cliff_state(x_index, y_index, is_cliff);
    }

    /// `WorldHeightMap::initCliffFlagsFromHeights` (WorldHeightMap.cpp:2297): every cell but the last row and column.
    pub fn init_cliff_flags_from_heights(&mut self, get_height: &impl Fn(i32, i32) -> u8) {
        for x_index in 0..self.width - 1 {
            for y_index in 0..self.height - 1 {
                self.set_cell_cliff_flag_from_heights(x_index, y_index, get_height);
            }
        }
    }

    /// `WorldHeightMap::setDrawOrg` (WorldHeightMap.cpp:2248): clamps the draw window to the map and returns whether
    /// anything changed. `draw_origin` is the verified map's `(draw_origin_x, draw_origin_y)`, updated in place.
    /// `stretch_terrain` / `draw_entire_terrain` are `TheGlobalData->m_stretchTerrain` / `m_drawEntireTerrain`.
    pub fn set_draw_org(
        &mut self,
        draw_origin: &mut (i32, i32),
        x_org: i32,
        y_org: i32,
        stretch_terrain: bool,
        draw_entire_terrain: bool,
    ) -> bool {
        let mut new_x = x_org;
        let mut new_y = y_org;
        let mut new_width = self.draw_width_x;
        let mut new_height = self.draw_height_y;
        if stretch_terrain {
            new_width = STRETCH_DRAW_WIDTH;
            new_height = STRETCH_DRAW_HEIGHT;
        }
        if draw_entire_terrain {
            new_width = self.width;
            new_height = self.height;
        }
        if new_width > self.width {
            new_width = self.width;
        }
        if new_height > self.height {
            new_height = self.height;
        }
        if new_x > self.width - new_width {
            new_x = self.width - new_width;
        }
        if new_x < 0 {
            new_x = 0;
        }
        if new_y > self.height - new_height {
            new_y = self.height - new_height;
        }
        if new_y < 0 {
            new_y = 0;
        }
        let anything_different = draw_origin.0 != new_x
            || draw_origin.1 != new_y
            || self.draw_width_x != new_width
            || self.draw_height_y != new_height;
        if anything_different {
            draw_origin.0 = new_x;
            draw_origin.1 = new_y;
            self.draw_width_x = new_width;
            self.draw_height_y = new_height;
            return true;
        }
        false
    }

    /// `WorldHeightMap::getTextureClassFromNdx` (WorldHeightMap.cpp:2333): the global class of the texture class whose
    /// tile range holds `tile_ndx >> 2`; classes with a negative `firstTile` are skipped; -1 when none.
    pub fn get_texture_class_from_ndx(&self, map: &TerrainHeightMap, tile_ndx: i32) -> i32 {
        let tile_ndx = tile_ndx >> 2;
        for (i, class) in map.texture_classes.iter().enumerate() {
            if class.first_tile < 0 {
                continue;
            }
            // see if the blend tile is in a texture class, and get the right tile for xIndex, yIndex.
            if tile_ndx >= class.first_tile && tile_ndx < class.first_tile + class.num_tiles {
                return self.global_texture_class.get(i).copied().unwrap_or(-1);
            }
        }
        -1
    }

    /// `WorldHeightMap::getTextureClass` (WorldHeightMap.cpp:2286): the global texture class of a cell; -1 when out of
    /// range, or (unless `base_class`) when the cell is blended.
    pub fn get_texture_class(&self, map: &TerrainHeightMap, x_index: i32, y_index: i32, base_class: bool) -> i32 {
        let ndx = y_index * map.width + x_index;
        if ndx < 0 || ndx >= map.data_size {
            return -1;
        }
        let texture_ndx = map.tile_ndxes[ndx as usize] as i32;
        if !base_class && (map.blend_tile_ndxes[ndx as usize] != 0 || map.extra_blend_tile_ndxes[ndx as usize] != 0) {
            return -1; // blended, so not of the original class.
        }
        self.get_texture_class_from_ndx(map, texture_ndx)
    }

    /// `WorldHeightMap::getTextureFromIndex` (WorldHeightMap.cpp:2350): `m_textureClasses[textureIndex]`.
    pub fn get_texture_from_index<'a>(
        map: &'a TerrainHeightMap,
        texture_index: usize,
    ) -> &'a rts_lockstep_engine::client::terrain_height_map::TxTextureClass {
        &map.texture_classes[texture_index]
    }

    /// `WorldHeightMap::setupAlphaTiles` (WorldHeightMap.cpp:2481): builds the 12 alpha ramp tiles (6 directions and
    /// their inversions) once, alpha in the 4th byte of each BGRA pixel, mips updated.
    pub fn setup_alpha_tiles(&mut self) {
        if self.alpha_tiles.len() == NUM_ALPHA_TILES && self.alpha_tiles[0].is_some() {
            return;
        }
        self.alpha_tiles = vec![None; NUM_ALPHA_TILES];
        let extent = TILE_PIXEL_EXTENT;
        for k in 0..NUM_ALPHA_TILES {
            let mut blend_info = BlendTileInfo::default();
            let mut base_k = k;
            if k >= K_INV {
                blend_info.inverted = 1;
                base_k -= K_INV;
            }
            match base_k {
                K_HORIZ => blend_info.horiz = 1,
                K_VERT => blend_info.vert = 1,
                K_LDIAG => blend_info.left_diagonal = 1,
                K_RDIAG => blend_info.right_diagonal = 1,
                K_LLDIAG => {
                    blend_info.left_diagonal = 1;
                    blend_info.long_diagonal = 1;
                }
                K_LRDIAG => {
                    blend_info.right_diagonal = 1;
                    blend_info.long_diagonal = 1;
                }
                _ => {}
            }
            let mut tile = TileDataMips::default();
            let mut dest = 0usize;
            for j in 0..extent {
                for i in 0..extent {
                    let mut h = i;
                    let mut v = j;
                    let mut alpha: i32 = 255; // 0 - 255.
                    if blend_info.horiz != 0 {
                        if blend_info.inverted == 0 {
                            h = extent - h - 1;
                        }
                        alpha = (alpha * h) / (extent - 1);
                    } else if blend_info.vert != 0 {
                        if blend_info.inverted == 0 {
                            v = extent - v - 1;
                        }
                        alpha = (alpha * v) / (extent - 1);
                    } else if blend_info.right_diagonal != 0 {
                        h = extent - h - 1;
                        if blend_info.inverted == 0 {
                            v = extent - v - 1;
                        }
                        v += h; // angled
                        if blend_info.long_diagonal != 0 {
                            v -= extent;
                        }
                        alpha = (alpha * v) / (extent - 1);
                    } else if blend_info.left_diagonal != 0 {
                        if blend_info.inverted == 0 {
                            v = extent - v - 1;
                        }
                        v += h; // angled
                        if blend_info.long_diagonal != 0 {
                            v -= extent;
                        }
                        alpha = (alpha * v) / (extent - 1);
                    }
                    if alpha > 255 {
                        alpha = 255;
                    }
                    if alpha < 0 {
                        alpha = 0;
                    }
                    alpha = 255 - alpha;
                    dest += 3; // skip blue, green & red bytes.
                    tile.tile_data[dest] = alpha as u8; // alpha.
                    dest += 1;
                }
            }
            tile.update_mips();
            self.alpha_tiles[k] = Some(tile);
        }
    }

    /// `WorldHeightMap::getRGBAlphaDataForWidth` (WorldHeightMap.cpp:2461): the alpha tile for a blend's direction.
    /// `setup_alpha_tiles` must have run (upstream would dereference NULL otherwise).
    pub fn get_rgb_alpha_data_for_width(&self, width: i32, blend: &BlendTileInfo) -> &[u8] {
        let mut alpha_tile_ndx = 0usize;
        if blend.horiz != 0 {
            alpha_tile_ndx = K_HORIZ;
        } else if blend.vert != 0 {
            alpha_tile_ndx = K_VERT;
        } else if blend.right_diagonal != 0 {
            alpha_tile_ndx = K_RDIAG;
            if blend.long_diagonal != 0 {
                alpha_tile_ndx = K_LRDIAG;
            }
        } else if blend.left_diagonal != 0 {
            alpha_tile_ndx = K_LDIAG;
            if blend.long_diagonal != 0 {
                alpha_tile_ndx = K_LLDIAG;
            }
        }
        if blend.inverted != 0 {
            alpha_tile_ndx += K_INV;
        }
        self.alpha_tiles[alpha_tile_ndx].as_ref().expect("setupAlphaTiles not run").get_rgb_data_for_width(width)
    }
}

/// `WorldHeightMap::isCliffMappedTexture` (WorldHeightMap.cpp:1673): is the cell cliff-indexed, with the draw-origin
/// offset that `HeightMapRenderObjClass` builds in.
pub fn is_cliff_mapped_texture(map: &TerrainHeightMap, x: i32, y: i32) -> bool {
    let ndx = x + map.draw_origin_x + map.width * (y + map.draw_origin_y);
    if ndx >= 0 && ndx < map.data_size {
        return map.cliff_info_ndxes[ndx as usize] != 0;
    }
    false
}

/// `WorldHeightMap::getRawTileData` (WorldHeightMap.cpp:2549): copies the `width` x `width` quarter of the source
/// tile `tile_ndx / 4` (quadrant from bits 0 and 1 of `tile_ndx`) from its `2 * width` mip into `buffer`. `src_tiles`
/// is `m_sourceTiles`. Returns false when the buffer is too small or the tile has no data for that width.
pub fn get_raw_tile_data(src_tiles: &[Option<TileDataMips>], tile_ndx: i16, width: i32, buffer: &mut [u8]) -> bool {
    let mut src: Option<&TileDataMips> = None;
    if (tile_ndx / 4) < NUM_SOURCE_TILES as i16 {
        src = src_tiles.get((tile_ndx / 4) as usize).and_then(|t| t.as_ref());
    }
    if (buffer.len() as i32) < width * width * TILE_BYTES_PER_PIXEL as i32 {
        return false;
    }
    if let Some(src) = src {
        if TileDataMips::has_rgb_data_for_width(2 * width) {
            let src_data = src.get_rgb_data_for_width(2 * width);
            let mut x_offset = 0;
            let mut y_offset = 0;
            if tile_ndx & 1 != 0 {
                x_offset = width;
            }
            if tile_ndx & 2 != 0 {
                y_offset = width;
            }
            let bpp = TILE_BYTES_PER_PIXEL as i32;
            for j in 0..width {
                let dest = (j * width * bpp) as usize;
                let s = ((j + y_offset) * width * bpp * 2 + x_offset * bpp) as usize;
                let n = (width * bpp) as usize;
                buffer[dest..dest + n].copy_from_slice(&src_data[s..s + n]);
            }
            return true;
        }
    }
    false
}

/// `WorldHeightMap::getPointerToTileData` (WorldHeightMap.cpp:2411): the BGRA pixels of one cell's tile at `width`,
/// with the blend tile (if any) alpha-blended over it. Upstream returns a pointer into a static buffer; this returns
/// the buffer (`None` for NULL). Upstream's in-place `*pDestData++ = ... (*pDestData)` reads the destination byte
/// before the post-increment store, i.e. blends the existing byte: kept.
pub fn get_pointer_to_tile_data(
    map: &TerrainHeightMap,
    extras: &WorldHeightMapExtras,
    src_tiles: &[Option<TileDataMips>],
    x_index: i32,
    y_index: i32,
    width: i32,
) -> Option<Vec<u8>> {
    let ndx = y_index * map.width + x_index;
    if y_index < 0 || x_index < 0 || x_index >= map.width || y_index >= map.height {
        return None;
    }
    if ndx < 0 || ndx >= map.data_size {
        return None;
    }
    let mut s_buffer = vec![0u8; DATA_LEN_BYTES];
    let mut s_blend_buffer = vec![0u8; DATA_LEN_BYTES];
    let tile_ndx = map.tile_ndxes[ndx as usize];
    if get_raw_tile_data(src_tiles, tile_ndx, width, &mut s_buffer) {
        let blend_tile_ndx = map.blend_tile_ndxes[ndx as usize];
        if blend_tile_ndx > 0 && (blend_tile_ndx as i32) < NUM_BLEND_TILES {
            let blend = map.blended_tiles[blend_tile_ndx as usize].clone();
            if get_raw_tile_data(src_tiles, blend.blend_ndx as i16, width, &mut s_blend_buffer) {
                let alpha_data = extras.get_rgb_alpha_data_for_width(width, &blend);
                let limit = (width * width) as usize;
                let mut blend_i = 0usize;
                let mut dest_i = 0usize;
                let mut alpha_i = 3usize; // skip over the rgb to the a.
                for _ in 0..limit {
                    let b = s_blend_buffer[blend_i] as i32;
                    let g = s_blend_buffer[blend_i + 1] as i32;
                    let r = s_blend_buffer[blend_i + 2] as i32;
                    blend_i += 4;
                    let a = alpha_data[alpha_i] as i32;
                    alpha_i += 4;
                    s_buffer[dest_i] = (((b * a) / 255) + ((s_buffer[dest_i] as i32 * (255 - a)) / 255)) as u8;
                    s_buffer[dest_i + 1] = (((g * a) / 255) + ((s_buffer[dest_i + 1] as i32 * (255 - a)) / 255)) as u8;
                    s_buffer[dest_i + 2] = (((r * a) / 255) + ((s_buffer[dest_i + 2] as i32 * (255 - a)) / 255)) as u8;
                    s_buffer[dest_i + 3] = 255; // just skip alpha. not really used. jba.
                    dest_i += 4;
                }
            }
        }
        return Some(s_buffer);
    }
    None
}

// ---------------------------------------------------------------------------------------------------------------------
// Chunk parsers
// ---------------------------------------------------------------------------------------------------------------------

/// What `ParseLightingDataChunk` reaches through `TheW3DShadowManager`: the shadow colour set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LightingChunkResult {
    /// `setShadowColor(shadowColor)` argument, if the chunk carried one (and `TheW3DShadowManager` exists).
    pub shadow_color: Option<u32>,
}

/// `WorldHeightMap::ParseLightingDataChunk` (WorldHeightMap.cpp:782): the global lights chunk. Fills
/// `TheWritableGlobalData` (`global`): the time of day, then for each of the 4 times of day the first terrain light
/// and first object light, then (version >= 2) object lights 1..3 and (version >= 3) terrain lights 1..3; an optional
/// trailing shadow colour. All lights are first initialised to `{ambient 0, diffuse 0, lightPos (0,0,-1)}` so old maps
/// have valid directions.
pub fn parse_lighting_data_chunk<C>(
    file: &mut DataChunkInput<C>,
    info: &DataChunkInfo,
    global: &mut GlobalData,
) -> ChunkResult<LightingChunkResult> {
    global.time_of_day = file.read_int()?;
    let init_light_values = rts_lockstep_engine::common::global_data::TerrainLighting {
        ambient: RGBColor { red: 0.0, green: 0.0, blue: 0.0 },
        diffuse: RGBColor { red: 0.0, green: 0.0, blue: 0.0 },
        light_pos: Coord3D { x: 0.0, y: 0.0, z: -1.0 },
    };
    // initialize the directions of the lights to not be totally invalid, in case old maps are read
    for i in 0..4usize {
        for j in 0..MAX_GLOBAL_LIGHTS {
            global.terrain_objects_lighting[i + TIME_OF_DAY_FIRST as usize][j] = init_light_values;
            global.terrain_lighting[i + TIME_OF_DAY_FIRST as usize][j] = init_light_values;
        }
    }
    fn read_light<C>(
        file: &mut DataChunkInput<C>,
        light: &mut rts_lockstep_engine::common::global_data::TerrainLighting,
    ) -> ChunkResult<()> {
        light.ambient.red = file.read_real()?;
        light.ambient.green = file.read_real()?;
        light.ambient.blue = file.read_real()?;
        light.diffuse.red = file.read_real()?;
        light.diffuse.green = file.read_real()?;
        light.diffuse.blue = file.read_real()?;
        light.light_pos.x = file.read_real()?;
        light.light_pos.y = file.read_real()?;
        light.light_pos.z = file.read_real()?;
        Ok(())
    }
    for i in 0..4usize {
        let t = i + TIME_OF_DAY_FIRST as usize;
        read_light(file, &mut global.terrain_lighting[t][0])?;
        read_light(file, &mut global.terrain_objects_lighting[t][0])?;
        if info.version >= K_LIGHTING_VERSION_2 {
            for j in 1..3usize {
                // added support for 2 extra object lights
                read_light(file, &mut global.terrain_objects_lighting[t][j])?;
            }
        }
        if info.version >= K_LIGHTING_VERSION_3 {
            for j in 1..3usize {
                // added support for 2 extra terrain lights
                read_light(file, &mut global.terrain_lighting[t][j])?;
            }
        }
    }
    let mut result = LightingChunkResult::default();
    if !file.at_end_of_chunk() {
        // `if (TheW3DShadowManager) setShadowColor(shadowColor)`: the caller applies it.
        result.shadow_color = Some(file.read_int()? as u32);
    }
    // DEBUG_ASSERTCRASH(file.atEndOfChunk(), ("Unexpected data left over."));
    Ok(result)
}

/// The two flags `ParseObjectData` sets that the verified `MapObject` has no setter for (`MO_LIGHT`, `MO_SCORCH`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectDataFlags {
    pub is_waypoint: bool,
    pub is_light: bool,
    pub is_scorch: bool,
}

/// `WorldHeightMap::ParseObjectData` (WorldHeightMap.cpp:1251): reads one object. Differences from the verified
/// `parse_object_data_chunk` (which is `ParseObjectDataChunk`): this one drops an object whose height is outside
/// `[-100 * MAP_XY_FACTOR, 255 * 10 * MAP_HEIGHT_SCALE]` ("Removing object at z height"), and also classifies lights
/// (`lightHeightAboveTerrain` real) and scorches (`scorchType` int). Returns `Ok(None)` for a removed object (upstream
/// returns true without linking it), else the object and its flags; the caller links it after the previous one.
/// `key_*` are the well-known name keys (`TheKey_waypointID`, `TheKey_lightHeightAboveTerrain`, `TheKey_scorchType`).
pub fn parse_object_data<C>(
    file: &mut DataChunkInput<C>,
    info: &DataChunkInfo,
    read_dict: bool,
    names: &mut rts_lockstep_engine::common::name_key_generator::NameKeyGenerator,
    key_waypoint_id: rts_lockstep_engine::common::name_key_generator::NameKeyType,
    key_light_height_above_terrain: rts_lockstep_engine::common::name_key_generator::NameKeyType,
    key_scorch_type: rts_lockstep_engine::common::name_key_generator::NameKeyType,
) -> ChunkResult<Option<(MapObject, ObjectDataFlags)>> {
    let x = file.read_real()?;
    let y = file.read_real()?;
    let mut z = file.read_real()?;
    let min_z = -100.0 * MAP_XY_FACTOR;
    let max_z = (255.0 * 10.0) * MAP_HEIGHT_SCALE;
    if info.version <= K_OBJECTS_VERSION_2 {
        z = 0.0;
    }
    let loc = Coord3D { x, y, z };
    let angle = file.read_real()?;
    let flags = file.read_int()?;
    let name = file.read_ascii_string()?;
    let d = if read_dict { file.read_dict(names)? } else { Dict::new() };

    if loc.z < min_z || loc.z > max_z {
        // DEBUG_LOG(("Removing object at z height %f\n", loc.z));
        return Ok(None);
    }

    // newInstance(MapObject)(loc, name, angle, flags, &d, TheThingFactory->findTemplate(name, FALSE))
    let obj = match MapObject::new(loc, &name, angle, flags, &d, Some(&name)) {
        Ok(o) => o,
        Err(_) => return Ok(None), // normalizeAngle never returns upstream; the verified parser stops the parse here
    };
    let mut out_flags = ObjectDataFlags::default();
    if obj.properties().get_type(key_waypoint_id) == DataType::Int {
        out_flags.is_waypoint = true;
    }
    if obj.properties().get_type(key_light_height_above_terrain) == DataType::Real {
        out_flags.is_light = true;
    }
    if obj.properties().get_type(key_scorch_type) == DataType::Int {
        out_flags.is_scorch = true;
    }
    Ok(Some((obj, out_flags)))
}
