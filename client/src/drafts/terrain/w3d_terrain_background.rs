//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DTerrainBackground.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DTerrainBackground.cpp
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp (`getFlipState`, `setFlipState`,
//!     `clearFlipStates`, which the port's `TerrainHeightMap` stores the bits for but has no accessors for)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aabox.h (`MinMaxAABoxClass::Init_Empty`/`Add_Point`,
//!     `AABoxClass::Init(MinMaxAABoxClass)`)
//!
//! The map (`WorldHeightMap *m_map`, shared and written through `setFlipState`) is an `Arc<RwLock<TerrainHeightMap>>`.
//! The vertex and index buffers are `Vec`s; `drawVisiblePolys` returns the textures and `Draw_Triangles` range.
//! `WorldHeightMap::getFlatTexture(x, y, width, pixelsPerCell)` (which builds the flat colour texture) is not in the port:
//! it is a closure returning the texture handle. `TheTerrainRenderObject->getStaticDiffuse(i, j)` is a closure.
//! `TerrainTextureClass::setLOD` is recorded in `terrain_texture_lod`.
//!
//! Not translated: the code after the `return;` in `doPartialUpdate` (the old non-tessellated update upstream left in
//! place; it can never run).

use super::w3d_common::{AABoxClass, DrawTriangles, TextureHandle, VertexFormatXYZDUV2};
use rts_lockstep_engine::client::terrain_height_map::TerrainHeightMap;
use rts_lockstep_engine::common::base_type::{ICoord2D, IRegion2D};
use rts_lockstep_engine::logic::terrain_visual::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::wwmath::WWMATH_FLOAT_MAX;
use std::sync::{Arc, RwLock};

/// `PIXELS_PER_GRID`: default tex resolution allocated for each tile. jba. [3/24/2003]
pub const PIXELS_PER_GRID: i32 = 8;

/// `W3DTerrainBackground::m_cullStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CullStatus {
    /// `CULL_STATUS_UNKNOWN`.
    Unknown,
    /// `CULL_STATUS_VISIBLE`.
    Visible,
    /// `CULL_STATUS_INVISIBLE`.
    Invisible,
}

/// `W3DTerrainBackground::m_texMultiplier`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TexMultiplier {
    /// `TEX4X`.
    Tex4X = 4,
    /// `TEX2X`.
    Tex2X = 2,
    /// `TEX1X`.
    Tex1X = 1,
}

/// `MinMaxAABoxClass` (aabox.h): only `Init_Empty` and `Add_Point`.
#[derive(Clone, Copy, Debug)]
pub struct MinMaxAABoxClass {
    /// `MinCorner`.
    pub min_corner: Vector3,
    /// `MaxCorner`.
    pub max_corner: Vector3,
}

impl MinMaxAABoxClass {
    /// `MinMaxAABoxClass::Init_Empty`.
    pub fn init_empty() -> Self {
        MinMaxAABoxClass {
            min_corner: Vector3::new(WWMATH_FLOAT_MAX, WWMATH_FLOAT_MAX, WWMATH_FLOAT_MAX),
            max_corner: Vector3::new(-WWMATH_FLOAT_MAX, -WWMATH_FLOAT_MAX, -WWMATH_FLOAT_MAX),
        }
    }

    /// `MinMaxAABoxClass::Add_Point`.
    pub fn add_point(&mut self, point: &Vector3) {
        self.min_corner.update_min(point);
        self.max_corner.update_max(point);
    }
}

/// `AABoxClass::Init(const MinMaxAABoxClass &)` (aabox.h:291).
pub fn aabox_from_min_max(mmbox: &MinMaxAABoxClass) -> AABoxClass {
    AABoxClass {
        center: (mmbox.max_corner + mmbox.min_corner) * 0.5,
        extent: (mmbox.max_corner - mmbox.min_corner) * 0.5,
    }
}

/// `WorldHeightMap::getFlipState` (WorldHeightMap.cpp:565).
pub fn get_flip_state(map: &TerrainHeightMap, x_index: i32, y_index: i32) -> bool {
    if x_index < 0 || y_index < 0 {
        return false;
    }
    if y_index >= map.height {
        return false;
    }
    if x_index >= map.width {
        return false;
    }
    if map.cell_flip_state.is_empty() {
        return false;
    }
    (map.cell_flip_state[(y_index * map.flip_state_width + (x_index >> 3)) as usize] & (1 << (x_index & 0x7))) != 0
}

/// `WorldHeightMap::setFlipState` (WorldHeightMap.cpp:576): sets the value of the flip state bit.
pub fn set_flip_state(map: &mut TerrainHeightMap, x_index: i32, y_index: i32, value: bool) {
    if x_index < 0 || y_index < 0 {
        return;
    }
    if y_index >= map.height {
        return;
    }
    if x_index >= map.width {
        return;
    }
    if map.cell_flip_state.is_empty() {
        return;
    }
    let i = (y_index * map.flip_state_width + (x_index >> 3)) as usize;
    if value {
        map.cell_flip_state[i] |= 1 << (x_index & 0x7);
    } else {
        map.cell_flip_state[i] &= !(1 << (x_index & 0x7));
    }
}

/// `WorldHeightMap::clearFlipStates` (WorldHeightMap.cpp:592): clears all flip state bits.
pub fn clear_flip_states(map: &mut TerrainHeightMap) {
    if !map.cell_flip_state.is_empty() {
        let n = (map.flip_state_width * map.height) as usize;
        for b in map.cell_flip_state.iter_mut().take(n) {
            *b = 0; // clear all flags
        }
    }
}

/// The map of a tile (`WorldHeightMap *`, reference counted upstream).
pub type SharedHeightMap = Arc<RwLock<TerrainHeightMap>>;

/// What `drawVisiblePolys` sets and draws: `Set_Index_Buffer`/`Set_Vertex_Buffer` of the tile's own buffers,
/// `Set_Texture(1, ...)` when textures are enabled, and the triangle list.
#[derive(Clone, Copy, Debug)]
pub struct TerrainBackgroundDraw {
    /// `Set_Texture(1, tex)`: the 4x, else the 2x, else the 1x texture; `None` when `disableTextures` (no call) or the
    /// chosen texture is NULL.
    pub texture_stage1: Option<TextureHandle>,
    /// `Draw_Triangles(0, m_curNumTerrainIndices/3, 0, m_curNumTerrainVertices)`.
    pub draw: DrawTriangles,
}

/// `class W3DTerrainBackground`: draw buffer to handle backup terrain at lower res.
pub struct W3DTerrainBackground {
    /// `m_cullStatus`.
    pub cull_status: CullStatus,
    /// `m_bounds`.
    pub bounds: AABoxClass,
    /// `m_vertexTerrain`: terrain vertex buffer (`m_vertexTerrainSize + 4` vertices).
    pub vertex_terrain: Option<Vec<VertexFormatXYZDUV2>>,
    /// `m_vertexTerrainSize`: num vertices in the buffer.
    pub vertex_terrain_size: i32,
    /// `m_indexTerrain`: indices defining the triangles (`m_indexTerrainSize + 4`).
    pub index_terrain: Option<Vec<u16>>,
    /// `m_indexTerrainSize`: indices available in `m_indexTerrain`.
    pub index_terrain_size: i32,
    /// `m_terrainTexture`.
    pub terrain_texture: Option<TextureHandle>,
    /// `m_terrainTexture2X`.
    pub terrain_texture_2x: Option<TextureHandle>,
    /// `m_terrainTexture4X`.
    pub terrain_texture_4x: Option<TextureHandle>,
    /// `m_terrainTexture->setLOD(LOD)`: the last LOD set (`None` before any).
    pub terrain_texture_lod: Option<i32>,
    /// `m_texMultiplier`.
    pub tex_multiplier: TexMultiplier,
    /// `m_curNumTerrainVertices`: number of vertices used in `m_vertexTerrain`.
    pub cur_num_terrain_vertices: i32,
    /// `m_curNumTerrainIndices`: number of indices used in `m_indexTerrain`.
    pub cur_num_terrain_indices: i32,
    /// `m_xOrigin`.
    pub x_origin: i32,
    /// `m_yOrigin`.
    pub y_origin: i32,
    /// `m_width`.
    pub width: i32,
    /// `m_map`.
    pub map: Option<SharedHeightMap>,
    /// `m_anythingChanged`.
    pub anything_changed: bool,
    /// `m_initialized`.
    pub initialized: bool,
}

impl Default for W3DTerrainBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DTerrainBackground {
    /// `W3DTerrainBackground::W3DTerrainBackground` (W3DTerrainBackground.cpp:218). The members the initialiser list does
    /// not set are uninitialised upstream and zero here.
    pub fn new() -> Self {
        W3DTerrainBackground {
            cull_status: CullStatus::Unknown,
            bounds: AABoxClass::default(),
            vertex_terrain: None,
            vertex_terrain_size: 0,
            index_terrain: None,
            index_terrain_size: 0,
            terrain_texture: None,
            terrain_texture_2x: None,
            terrain_texture_4x: None,
            terrain_texture_lod: None,
            tex_multiplier: TexMultiplier::Tex1X,
            cur_num_terrain_vertices: 0,
            cur_num_terrain_indices: 0,
            x_origin: 0,
            y_origin: 0,
            width: 0,
            map: None,
            anything_changed: false,
            initialized: false,
        }
    }

    /// `W3DTerrainBackground::~W3DTerrainBackground` (W3DTerrainBackground.cpp:204).
    pub fn destroy(&mut self) {
        self.free_terrain_buffers();
        self.terrain_texture = None;
        self.terrain_texture_2x = None;
        self.terrain_texture_4x = None;
    }

    /// `W3DTerrainBackground::isCulled` (W3DTerrainBackground.h).
    pub fn is_culled(&self) -> bool {
        self.cull_status == CullStatus::Invisible
    }

    /// `W3DTerrainBackground::getTexMultiplier` (W3DTerrainBackground.h).
    pub fn get_tex_multiplier(&self) -> i32 {
        self.tex_multiplier as i32
    }

    /// `W3DTerrainBackground::freeTerrainBuffers` (W3DTerrainBackground.cpp:240): frees the index and vertex buffers.
    pub fn free_terrain_buffers(&mut self) {
        self.vertex_terrain = None;
        self.index_terrain = None;
        self.cur_num_terrain_vertices = 0;
        self.cur_num_terrain_indices = 0;
        self.initialized = false;
        self.map = None;
    }

    /// `W3DTerrainBackground::allocateTerrainBuffers` (W3DTerrainBackground.cpp:255): allocates the index and vertex
    /// buffers (they are made by the first update).
    pub fn allocate_terrain_buffers(&mut self, ht_map: Option<&SharedHeightMap>, x_origin: i32, y_origin: i32, width: i32) {
        let ht_map = match ht_map {
            Some(m) => m,
            None => return,
        };
        self.free_terrain_buffers(); // in case already allocated. jba [3/24/2003]
        self.cur_num_terrain_vertices = 0;
        self.cur_num_terrain_indices = 0;
        self.x_origin = x_origin;
        self.y_origin = y_origin;
        self.width = width;
        self.initialized = true;
        self.map = Some(Arc::clone(ht_map));
    }

    /// `W3DTerrainBackground::setFlip` (W3DTerrainBackground.cpp:77): sets the flip bit for required vertices.
    pub fn set_flip(&mut self, ht_map: Option<&SharedHeightMap>) {
        if self.map.is_none() {
            return;
        }
        if let Some(m) = ht_map {
            self.map = Some(Arc::clone(m));
        }
        if !self.initialized {
            return;
        }

        let map = Arc::clone(self.map.as_ref().unwrap());
        let mut guard = map.write().unwrap();
        self.set_flip_recursive(&mut guard, 0, 0, self.width);
    }

    /// `W3DTerrainBackground::doPartialUpdate` (W3DTerrainBackground.cpp:97): updates a partial block of vertices from
    /// [x0,y0 to x1,y1]. The coordinates in partialRange are map cell coordinates, relative to the entire map. The vertex
    /// coordinates and texture coordinates, as well as static lighting are updated. (The code after upstream's `return;`
    /// is dead and not translated.)
    pub fn do_partial_update(
        &mut self,
        partial_range: &IRegion2D,
        ht_map: Option<&SharedHeightMap>,
        do_textures: bool,
        static_diffuse: &impl Fn(i32, i32) -> u32,
        get_flat_texture: &mut impl FnMut(i32, i32, i32, i32) -> Option<TextureHandle>,
    ) {
        if self.map.is_none() {
            return;
        }
        if let Some(m) = ht_map {
            self.map = Some(Arc::clone(m));
        }

        if !self.initialized {
            return;
        }
        self.do_tesselated_update(partial_range, ht_map, do_textures, static_diffuse, get_flat_texture);
    }

    /// `W3DTerrainBackground::advanceLeft` (W3DTerrainBackground.cpp:195).
    fn advance_left(&self, map: &TerrainHeightMap, left: &mut ICoord2D, x_offset: i32, y_offset: i32, width: i32) -> bool {
        while left.y < y_offset + width {
            left.y += 1;
            if get_flip_state(map, left.x + self.x_origin, left.y + self.y_origin) {
                return true;
            }
        }
        while left.x < x_offset + width - 1 {
            left.x += 1;
            if get_flip_state(map, left.x + self.x_origin, left.y + self.y_origin) {
                return true;
            }
        }
        false
    }

    /// `W3DTerrainBackground::advanceRight` (W3DTerrainBackground.cpp:218).
    fn advance_right(&self, map: &TerrainHeightMap, right: &mut ICoord2D, x_offset: i32, y_offset: i32, width: i32) -> bool {
        while right.x < x_offset + width {
            right.x += 1;
            if get_flip_state(map, right.x + self.x_origin, right.y + self.y_origin) {
                return true;
            }
        }
        while right.y < y_offset + width - 1 {
            right.y += 1;
            if get_flip_state(map, right.x + self.x_origin, right.y + self.y_origin) {
                return true;
            }
        }
        false
    }

    /// The test both recursive functions share: whether every corner-clamped height in the `width`-square block at
    /// (`x_offset`, `y_offset`) equals the corner height (a `width` of 1 always matches). Note upstream's `break` leaves
    /// only the inner loop.
    fn block_is_flat(&self, map: &TerrainHeightMap, x_offset: i32, y_offset: i32, width: i32) -> bool {
        let limit_x = map.width - 1;
        let limit_y = map.height - 1;
        let mut matches = true;
        let min_x = self.x_origin + x_offset;
        let min_y = self.y_origin + y_offset;
        let corner_height = map.get_height(min_x, min_y) as i32;

        for i in 0..=width {
            for j in 0..=width {
                let mut k = min_x + i;
                k = if k < limit_x { k } else { limit_x };
                let mut l = min_y + j;
                l = if l < limit_y { l } else { limit_y };
                if corner_height != map.get_height(k, l) as i32 {
                    matches = false;
                    break;
                }
            }
        }
        if width == 1 {
            matches = true;
        }
        matches
    }

    /// `W3DTerrainBackground::fillVBRecursive` (W3DTerrainBackground.cpp:245): fills in the index buffer (`ib` `None`
    /// only counts, in `cur_index`).
    #[allow(clippy::too_many_arguments)]
    fn fill_vb_recursive(&self, map: &TerrainHeightMap, ib: &mut Option<Vec<u16>>, x_offset: i32, y_offset: i32, width: i32, ndx: &[u16], cur_index: &mut i32) {
        let bottom_left_ndx = ndx[(x_offset + y_offset * (self.width + 1)) as usize] as i32;
        let top_right_ndx = ndx[(x_offset + width + (y_offset + width) * (self.width + 1)) as usize] as i32;

        let matches = self.block_is_flat(map, x_offset, y_offset, width);

        let mut put = |ib: &mut Option<Vec<u16>>, cur_index: &mut i32, v: i32| {
            if let Some(b) = ib.as_mut() {
                b[*cur_index as usize] = v as u16;
            }
            *cur_index += 1;
        };

        if matches {
            let mut left = ICoord2D { x: x_offset, y: y_offset };
            let mut right = ICoord2D { x: x_offset, y: y_offset };
            self.advance_left(map, &mut left, x_offset, y_offset, width);
            self.advance_right(map, &mut right, x_offset, y_offset, width);

            put(ib, cur_index, bottom_left_ndx);

            let mut prev_ndx_right: i32 = ndx[(right.x + right.y * (self.width + 1)) as usize] as i32;
            put(ib, cur_index, prev_ndx_right);

            let mut prev_ndx_left: i32 = ndx[(left.x + left.y * (self.width + 1)) as usize] as i32;
            put(ib, cur_index, prev_ndx_left);
            let mut did_left = true;
            let mut did_right = true;
            while did_left || did_right {
                did_left = self.advance_left(map, &mut left, x_offset, y_offset, width);
                if did_left {
                    put(ib, cur_index, prev_ndx_left);

                    put(ib, cur_index, prev_ndx_right);

                    prev_ndx_left = ndx[(left.x + left.y * (self.width + 1)) as usize] as i32;
                    put(ib, cur_index, prev_ndx_left);
                }
                did_right = self.advance_right(map, &mut right, x_offset, y_offset, width);
                if did_right {
                    put(ib, cur_index, prev_ndx_left);

                    put(ib, cur_index, prev_ndx_right);

                    prev_ndx_right = ndx[(right.x + right.y * (self.width + 1)) as usize] as i32;
                    put(ib, cur_index, prev_ndx_right);
                }
            }

            put(ib, cur_index, prev_ndx_left);

            put(ib, cur_index, prev_ndx_right);

            put(ib, cur_index, top_right_ndx);

            return;
        }
        let half_width = width / 2;

        self.fill_vb_recursive(map, ib, x_offset, y_offset, half_width, ndx, cur_index);
        self.fill_vb_recursive(map, ib, x_offset, y_offset + half_width, half_width, ndx, cur_index);
        self.fill_vb_recursive(map, ib, x_offset + half_width, y_offset, half_width, ndx, cur_index);
        self.fill_vb_recursive(map, ib, x_offset + half_width, y_offset + half_width, half_width, ndx, cur_index);
    }

    /// `W3DTerrainBackground::setFlipRecursive` (W3DTerrainBackground.cpp:416).
    fn set_flip_recursive(&self, map: &mut TerrainHeightMap, x_offset: i32, y_offset: i32, width: i32) {
        let matches = self.block_is_flat(map, x_offset, y_offset, width);
        let min_x = self.x_origin + x_offset;
        let min_y = self.y_origin + y_offset;

        if matches {
            set_flip_state(map, min_x, min_y, true);
            set_flip_state(map, min_x + width, min_y, true);
            set_flip_state(map, min_x + width, min_y + width, true);
            set_flip_state(map, min_x, min_y + width, true);
            return;
        }
        let half_width = width / 2;

        self.set_flip_recursive(map, x_offset, y_offset, half_width);
        self.set_flip_recursive(map, x_offset, y_offset + half_width, half_width);
        self.set_flip_recursive(map, x_offset + half_width, y_offset, half_width);
        self.set_flip_recursive(map, x_offset + half_width, y_offset + half_width, half_width);
    }

    /// `W3DTerrainBackground::doTesselatedUpdate` (W3DTerrainBackground.cpp:455): updates a partial block of vertices from
    /// [x0,y0 to x1,y1]. The coordinates in partialRange are map cell coordinates, relative to the entire map. The vertex
    /// coordinates and texture coordinates, as well as static lighting are updated.
    pub fn do_tesselated_update(
        &mut self,
        partial_range: &IRegion2D,
        ht_map: Option<&SharedHeightMap>,
        do_textures: bool,
        static_diffuse: &impl Fn(i32, i32) -> u32,
        get_flat_texture: &mut impl FnMut(i32, i32, i32, i32) -> Option<TextureHandle>,
    ) {
        if self.map.is_none() {
            return;
        }
        if let Some(m) = ht_map {
            self.map = Some(Arc::clone(m));
        }
        if !self.initialized {
            return;
        }
        let min_x = self.x_origin;
        let min_y = self.y_origin;
        let max_x = self.x_origin + self.width;
        let max_y = self.y_origin + self.width;

        if partial_range.lo.x > max_x {
            return;
        }
        if partial_range.lo.y > max_y {
            return;
        }
        if partial_range.hi.x < min_x {
            return;
        }
        if partial_range.hi.y < min_y {
            return;
        }

        self.set_flip(ht_map);

        let map_arc = Arc::clone(self.map.as_ref().unwrap());
        let map = map_arc.read().unwrap();
        let limit_x = map.width - 1;
        let limit_y = map.height - 1;

        let count = (self.width + 1) * (self.width + 1);

        let mut ndx = vec![0u16; count as usize];

        let mut required_vertex: i32 = 0;
        for j in min_y..=max_y {
            for i in min_x..=max_x {
                let ndx_ndx = i - min_x + (self.width + 1) * (j - min_y);
                // DEBUG_ASSERTCRASH(ndxNdx<count, ("Bad ndxNdx"));
                ndx[ndx_ndx as usize] = 0;
                if get_flip_state(&map, i, j) {
                    required_vertex += 1;
                }
            }
        }

        if self.vertex_terrain_size < required_vertex || self.vertex_terrain.is_none() {
            self.vertex_terrain_size = required_vertex;
            // DX8VertexBufferClass(DX8_FVF_XYZDUV2, m_vertexTerrainSize+4, USAGE_DEFAULT)
            self.vertex_terrain = Some(vec![VertexFormatXYZDUV2::default(); (self.vertex_terrain_size + 4) as usize]);
        }

        self.cur_num_terrain_vertices = 0;
        // Lock the buffer.
        let mut vb = self.vertex_terrain.take().unwrap();
        let mut cur_vb: usize = 0;
        // Add to the vertex buffer.
        for j in min_y..=max_y {
            for i in min_x..=max_x {
                if get_flip_state(&map, i, j) {
                    vb[cur_vb].diffuse = (0u32 << 24) | static_diffuse(i, j);
                    let k = if i < limit_x { i } else { limit_x };
                    let l = if j < limit_y { j } else { limit_y };
                    let mut pos = Vector3::default();
                    pos.z = map.get_height(k, l) as f32 * MAP_HEIGHT_SCALE;
                    pos.x = i as f32 * MAP_XY_FACTOR - map.border_size as f32 * MAP_XY_FACTOR;
                    pos.y = j as f32 * MAP_XY_FACTOR - map.border_size as f32 * MAP_XY_FACTOR;
                    vb[cur_vb].u1 = (i - min_x) as f32 / self.width as f32;
                    vb[cur_vb].v1 = 1.0 - (j - min_y) as f32 / self.width as f32;
                    vb[cur_vb].x = pos.x;
                    vb[cur_vb].y = pos.y;
                    vb[cur_vb].z = pos.z;
                    cur_vb += 1;
                    let ndx_ndx = i - min_x + (self.width + 1) * (j - min_y);
                    // DEBUG_ASSERTCRASH(ndxNdx<count, ("Bad ndxNdx"));
                    ndx[ndx_ndx as usize] = self.cur_num_terrain_vertices as u16;
                    self.cur_num_terrain_vertices += 1;
                }
            }
        }
        self.vertex_terrain = Some(vb);

        let mut required_index: i32 = 0;

        self.fill_vb_recursive(&map, &mut None, 0, 0, self.width, &ndx, &mut required_index);

        if self.index_terrain_size < required_index || self.index_terrain.is_none() {
            self.index_terrain_size = required_index;
            // DX8IndexBufferClass(m_indexTerrainSize+4, USAGE_DEFAULT)
            self.index_terrain = Some(vec![0u16; (self.index_terrain_size + 4) as usize]);
        }

        self.cur_num_terrain_indices = 0;

        let mut ib = self.index_terrain.take();
        let mut cur_num_indices = self.cur_num_terrain_indices;
        self.fill_vb_recursive(&map, &mut ib, 0, 0, self.width, &ndx, &mut cur_num_indices);
        self.cur_num_terrain_indices = cur_num_indices;
        self.index_terrain = ib;

        let mut bounds = MinMaxAABoxClass::init_empty();

        for j in min_y..=max_y {
            for i in min_x..=max_x {
                let k = if i < limit_x { i } else { limit_x };
                let l = if j < limit_y { j } else { limit_y };
                let mut pos = Vector3::default();
                pos.z = map.get_height(k, l) as f32 * MAP_HEIGHT_SCALE;
                pos.x = i as f32 * MAP_XY_FACTOR - map.border_size as f32 * MAP_XY_FACTOR;
                pos.y = j as f32 * MAP_XY_FACTOR - map.border_size as f32 * MAP_XY_FACTOR;
                bounds.add_point(&pos);
            }
        }
        self.bounds = aabox_from_min_max(&bounds);

        if self.terrain_texture.is_none() || do_textures {
            self.terrain_texture = None;
            self.terrain_texture_2x = None;
            self.terrain_texture_4x = None;
            self.terrain_texture = get_flat_texture(self.x_origin, self.y_origin, self.width, PIXELS_PER_GRID);
            // m_terrainTexture->Get_Filter().Set_U_Addr_Mode / Set_V_Addr_Mode(CLAMP) — the renderer's
        }
    }

    /// `W3DTerrainBackground::updateCenter` (W3DTerrainBackground.cpp:577): updates the culling status. `cull_box` is
    /// `camera->Cull_Box(m_bounds)`, `camera_pos` is `camera->Get_Position()`.
    pub fn update_center(&mut self, cull_box: &impl Fn(&AABoxClass) -> bool, camera_pos: &Vector3) {
        if cull_box(&self.bounds) {
            self.cull_status = CullStatus::Invisible;
        } else {
            self.cull_status = CullStatus::Visible;
        }

        if self.cull_status == CullStatus::Invisible {
            self.terrain_texture_2x = None;
            self.terrain_texture_4x = None;
            self.tex_multiplier = TexMultiplier::Tex1X;
            return;
        }
        let camera_pos = *camera_pos;
        const MIP_DISTANCE: f32 = 310.0;
        const MIP_SLOP: f32 = 40.0;
        let sqr = |x: f32| x * x;
        let mip4x_distance_sqr: f32 = sqr(MIP_DISTANCE + MIP_SLOP);
        let mip2x_distance_sqr: f32 = sqr(2.0 * MIP_DISTANCE + MIP_SLOP);
        let mip_lod_distance_sqr: f32 = sqr(4.0 * MIP_DISTANCE + MIP_SLOP);
        let mut min_dist_sqr: f32 = 2.0 * mip2x_distance_sqr;
        for i in -1..2 {
            for j in -1..2 {
                for k in -1..2 {
                    let mut corner = self.bounds.center;
                    corner.x += self.bounds.extent.x * i as f32;
                    corner.y += self.bounds.extent.y * j as f32;
                    corner.z += self.bounds.extent.z * k as f32;
                    let dist_sqr = (camera_pos - corner).length2();
                    if dist_sqr < min_dist_sqr {
                        min_dist_sqr = dist_sqr;
                    }
                }
            }
        }
        self.tex_multiplier = TexMultiplier::Tex1X;
        if min_dist_sqr < mip4x_distance_sqr {
            self.tex_multiplier = TexMultiplier::Tex4X;
        } else if min_dist_sqr < mip2x_distance_sqr {
            self.tex_multiplier = TexMultiplier::Tex2X;
        } else {
            self.terrain_texture_4x = None;
            self.terrain_texture_2x = None;
            let mut lod = 0;
            if min_dist_sqr > mip_lod_distance_sqr {
                lod = 1;
            }
            self.terrain_texture_lod = Some(lod); // m_terrainTexture->setLOD(LOD)
        }
    }

    /// `W3DTerrainBackground::updateTexture` (W3DTerrainBackground.cpp:640): notify camera moved. Makes the 2x or 4x
    /// texture the multiplier asks for.
    pub fn update_texture(&mut self, get_flat_texture: &mut impl FnMut(i32, i32, i32, i32) -> Option<TextureHandle>) {
        if self.cull_status == CullStatus::Invisible {
            self.terrain_texture_2x = None;
            self.terrain_texture_4x = None;
            return;
        }

        if self.tex_multiplier == TexMultiplier::Tex4X {
            self.terrain_texture_2x = None;
            if self.terrain_texture_4x.is_none() {
                self.terrain_texture_4x = get_flat_texture(self.x_origin, self.y_origin, self.width, 4 * PIXELS_PER_GRID);
                // Get_Filter().Set_U_Addr_Mode / Set_V_Addr_Mode(CLAMP) — the renderer's
            }
        } else if self.tex_multiplier == TexMultiplier::Tex2X {
            self.terrain_texture_4x = None;
            if self.terrain_texture_2x.is_none() {
                self.terrain_texture_2x = get_flat_texture(self.x_origin, self.y_origin, self.width, 2 * PIXELS_PER_GRID);
                // Get_Filter().Set_U_Addr_Mode / Set_V_Addr_Mode(CLAMP) — the renderer's
            }
        } else {
            self.terrain_texture_4x = None;
            self.terrain_texture_2x = None;
        }
    }

    /// `W3DTerrainBackground::drawVisiblePolys` (W3DTerrainBackground.cpp:683): draws the terrain (the `#if 1` branch,
    /// texture stage 1). `None` when upstream returns without drawing.
    pub fn draw_visible_polys(&self, disable_textures: bool) -> Option<TerrainBackgroundDraw> {
        if self.cur_num_terrain_indices == 0 {
            return None;
        }
        if self.cull_status == CullStatus::Invisible {
            return None;
        }
        // Setup the vertex buffer, shader & texture.
        // upstream: Set_Index_Buffer(m_indexTerrain, 0), Set_Vertex_Buffer(m_vertexTerrain) — device-only
        let mut texture_stage1 = None;
        if !disable_textures {
            if self.terrain_texture_4x.is_some() {
                texture_stage1 = self.terrain_texture_4x;
            } else if self.terrain_texture_2x.is_some() {
                texture_stage1 = self.terrain_texture_2x;
            } else {
                texture_stage1 = self.terrain_texture;
            }
        }
        Some(TerrainBackgroundDraw {
            texture_stage1,
            draw: DrawTriangles {
                start_index: 0,
                polygon_count: self.cur_num_terrain_indices / 3,
                min_vertex_index: 0,
                vertex_count: self.cur_num_terrain_vertices,
            },
        })
    }
}
