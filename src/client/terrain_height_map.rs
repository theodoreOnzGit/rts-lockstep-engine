// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/WorldHeightMap.h
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp
//   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/TileData.h
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/TileData.cpp (GitHub #35: TileData::doMip, updateMips,
//     getRGBDataForWidth)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/TerrainTex.cpp
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/HeightMap.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: John Ahlquist (WorldHeightMap, TerrainTex, HeightMap)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The terrain as the game draws it (GitHub #11): `WorldHeightMap`'s client side (the tiles, the blend tiles, the cliff UVs,
//! the tile atlas and the UV lookups) and `HeightMapRenderObjClass::updateVB`'s vertices (positions, normals, the two UV
//! sets, the blend alpha and the triangle diagonal), as plain data. The Bevy client (`client/src/terrain.rs`) turns them
//! into meshes and an image; nothing here draws.
//!
//! # As upstream reads it
//!
//! - **The map.** `WorldHeightMap(pStrm, logicalDataOnly = false)` registers `ParseBlendTileData`, which reads the tile,
//!   blend, extra blend and cliff info indices of every sample, the texture classes (each one's tiles read from
//!   `Art/Terrain/<Terrain.ini texture>` by `readTexClass`), the edge classes, the blend tiles (each ending in
//!   `FLAG_VAL`) and the cliff infos. The constructor then "patches bad maps": a blend, extra blend or cliff info index
//!   outside its table becomes 0. The logic's copy of that chunk (`logic::terrain_visual::BlendTiles`) already holds the
//!   indices and keeps the rest of the chunk's bytes; [`TerrainHeightMap::from_logic`] reads the rest and patches.
//! - **The tiles.** `countTiles` and `readTiles` cut a 24- or 32-bit Targa into 64-pixel tiles, `numRows * numRows` of
//!   them, row 0 of the file first (`readTexClass`: the class's tile count, rounded down to a square).
//! - **The atlas.** `getTerrainTexture`: `updateTileTexturePositions` places each texture class in a 2048-pixel-wide
//!   texture on a grid of 72-pixel cells (64 and a 4-pixel border each side); the height is rounded up to a power of
//!   two; `TerrainTextureClass::update` writes the tiles as A1R5G5B5, upside down within each tile, and copies 4 pixels of
//!   wrap-around border round each class; then each cell's flip bit is set from `getAlphaUVData`.
//! - **The UVs.** `getUVData`/`getUVForTileIndex`/`getUVForNdx` give the base texture's UVs of a cell (a quarter tile:
//!   each tile covers 2x2 cells, `tileNdx & 3` picking the quarter), with the cliff adjustments when
//!   `TheGlobalData->m_adjustCliffTextures`; `getAlphaUVData` gives the blend layer's UVs and the four corner alphas;
//!   `getExtraAlphaUVData` the third layer's.
//! - **The vertices.** `HeightMapRenderObjClass::updateVB` (`HALF_RES_MESH` is false: one cell per sample): four
//!   vertices per cell, `(x, y)`, `(x+1, y)`, `(x+1, y+1)`, `(x, y+1)`, at `(index - border) * MAP_XY_FACTOR` and
//!   `height * MAP_HEIGHT_SCALE`, each with a normal from the central differences of its neighbours; the index buffer
//!   draws `(0, 2, 3)` and `(0, 1, 2)`, and a cell whose blend needs it has its four vertices rotated by one, which moves
//!   the diagonal from 0-2 to 1-3 (`FLIP_TRIANGLES`).
//!
//! # Upstream quirks that are kept
//!
//! - `updateTileTexturePositions` looks, in each row, only at the first free grid cell: if the class does not fit
//!   there, it goes on to the next row (the `break` after the open check), so a class can be left out (position 0, no
//!   texture) while room remains.
//! - `getUVForTileIndex`'s cliff UVs scale V by `TEXTURE_WIDTH / m_terrainTexHeight`, an integer division.
//! - `getUVForTileIndex` (with `m_adjustCliffTextures`) returns "stretched" for a cell in the map's last row without
//!   adjusting it (the `return true` after the `ndx < m_dataSize - m_width - 1` test), so `getAlphaUVData` flips such a
//!   cell by its heights.
//! - `readTiles` reads the pixels right after the 18-byte header: the Targa's ID field and its origin bit are not
//!   looked at. A class whose file holds fewer tiles than the map's class has is not read at all.
//! - `updateVB` over the whole map (the configuration `m_drawEntireTerrain` gives: the draw window is the map, origin
//!   0): `getXWithOrigin(i + 1)` wraps the last column to 0, so the right-hand neighbour of the last column's normals is
//!   column 1 (and likewise row 1 for the last row's).
//! - The logic's ground height (`getHeightMapHeight`) always splits a cell on the 0-2 diagonal, so on a flipped cell the
//!   drawn surface and the height objects stand at differ; both are kept as they are.
//! - A1R5G5B5: the atlas keeps 5 bits a channel.
//!
//! # Where this differs from upstream
//!
//! - The D3D textures and vertex buffers are plain vectors. `doTheLight` (the lighting baked into each vertex's colour) is
//!   not translated: the vertex keeps its normal and the renderer lights it. The atlas's mip levels
//!   (`D3DXFilterTexture`) are the renderer's.
//! - Indices that are undefined upstream are not followed: a tile index outside `m_sourceTiles` is a missing tile, a
//!   count beyond upstream's fixed arrays is an error ([`TerrainMapError::Undefined`]).
//! - Below version 6 (extra blend) and 5 (cliff infos) upstream leaves those index arrays uninitialised; here they are 0.
//!   A version 1 chunk (halved on read) is not supported.
//! - A map with no `BlendTileData` crashes upstream (the patch loop reads a null array); here it is an error, and the
//!   client draws an untextured height mesh.
//! - The custom blend edges (`m_edgeTiles`, `AlphaEdgeTextureClass`) are read but not drawn.
//!
//! # Representation
//!
//! [`TerrainHeightMap`] holds `WorldHeightMap`'s members under their upstream names in snake case. Tiles are
//! [`TileData`] (BGRA, 64x64, as upstream's `m_tileData`). [`TerrainHeightMap::build_terrain_texture`] returns the atlas as
//! [`TerrainAtlas`] (A1R5G5B5 words). [`TerrainHeightMap::update_vb`] returns [`TerrainVertex`]es, four per cell in the
//! cell's draw order, row by row, and [`TERRAIN_CELL_INDICES`] is the per-cell index pattern.

use crate::common::base_type::ICoord2D;
use crate::common::terrain_types::TerrainTypeCollection;
use crate::logic::terrain_visual::{LogicHeightMap, TextureClass, K_BLEND_TILE_VERSION_5, K_BLEND_TILE_VERSION_6, MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use crate::ww::vector3::Vector3;

/// `NUM_SOURCE_TILES`.
pub const NUM_SOURCE_TILES: usize = 1024;
/// `NUM_BLEND_TILES`.
pub const NUM_BLEND_TILES: i32 = 16192;
/// `NUM_CLIFF_INFO`.
pub const NUM_CLIFF_INFO: i32 = 32384;
/// `NUM_TEXTURE_CLASSES`.
pub const NUM_TEXTURE_CLASSES: i32 = 256;
/// `FLAG_VAL`: the word ending each blend tile.
pub const FLAG_VAL: i32 = 0x7ADA_0000;
/// `TILE_PIXEL_EXTENT`.
pub const TILE_PIXEL_EXTENT: i32 = 64;
/// `TILE_BYTES_PER_PIXEL`.
pub const TILE_BYTES_PER_PIXEL: usize = 4;
/// `DATA_LEN_BYTES`.
pub const DATA_LEN_BYTES: usize = (TILE_PIXEL_EXTENT * TILE_PIXEL_EXTENT) as usize * TILE_BYTES_PER_PIXEL;
/// `TEXTURE_WIDTH` ("was 1024 jba").
pub const TEXTURE_WIDTH: i32 = 2048;
/// `TILE_OFFSET`: the border round each class in the atlas, half on each side.
pub const TILE_OFFSET: i32 = 8;
/// `INVERTED_MASK`.
pub const INVERTED_MASK: u8 = 0x1;
/// `FLIPPED_MASK`.
pub const FLIPPED_MASK: u8 = 0x2;
/// `K_BLEND_TILE_VERSION_1`, `_3`, `_4`.
pub const K_BLEND_TILE_VERSION_1: u16 = 1;
pub const K_BLEND_TILE_VERSION_3: u16 = 3;
pub const K_BLEND_TILE_VERSION_4: u16 = 4;
/// `TERRAIN_TGA_DIR_PATH`.
pub const TERRAIN_TGA_DIR_PATH: &[u8] = b"Art/Terrain/";
/// `HeightMapRenderObjClass::initHeightData`'s index buffer, per cell of four vertices: `(0, 2, 3)` and `(0, 1, 2)`.
pub const TERRAIN_CELL_INDICES: [u32; 6] = [0, 2, 3, 0, 1, 2];

/// Why the client's height map could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerrainMapError {
    /// The map has no `BlendTileData` (upstream crashes).
    NoBlendTileData,
    /// A version 1 `BlendTileData` (halved on read), not supported.
    Version1,
    /// `throw ERROR_CORRUPT_FILE_FORMAT` (a blend tile not ending in `FLAG_VAL`), or the chunk ends early.
    Corrupt,
    /// Undefined upstream (a count past a fixed array).
    Undefined(&'static str),
}

/// `TBlendTileInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlendTileInfo {
    pub blend_ndx: i32,
    pub horiz: u8,
    pub vert: u8,
    pub right_diagonal: u8,
    pub left_diagonal: u8,
    pub inverted: u8,
    pub long_diagonal: u8,
    /// `customBlendEdgeClass`: -1 means use alpha.
    pub custom_blend_edge_class: i32,
}

/// `TCliffInfo`: the UVs of a cliff cell's four corners in its class's tiles.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CliffInfo {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub u2: f32,
    pub v2: f32,
    pub u3: f32,
    pub v3: f32,
    pub flip: bool,
    pub mutant: bool,
    pub tile_index: i16,
}

/// `TXTextureClass`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TxTextureClass {
    pub first_tile: i32,
    pub num_tiles: i32,
    pub width: i32,
    pub name: Vec<u8>,
    /// `positionInTexture`: set by `updateTileTexturePositions`.
    pub position_in_texture: ICoord2D,
}

/// `TileData`: one 64x64 tile, BGRA (`m_tileData`), and where it is in the atlas (`m_tileLocationInTexture`).
#[derive(Clone, Debug, PartialEq)]
pub struct TileData {
    pub tile_data: Vec<u8>,
    pub tile_location_in_texture: ICoord2D,
}

impl Default for TileData {
    fn default() -> Self {
        TileData { tile_data: vec![0; DATA_LEN_BYTES], tile_location_in_texture: ICoord2D { x: 0, y: 0 } }
    }
}

/// The terrain atlas (`TerrainTextureClass`, A1R5G5B5): `width * height` words, row 0 at the top (D3D's order).
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainAtlas {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u16>,
}

impl TerrainAtlas {
    /// The atlas as sRGB RGBA8, each 5-bit channel widened as the hardware reads it (`c * 255 / 31`, rounded), alpha from
    /// the A1 bit.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let w = |c: u16| ((u32::from(c) * 255 + 15) / 31) as u8;
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &p in &self.pixels {
            out.extend_from_slice(&[w((p >> 10) & 31), w((p >> 5) & 31), w(p & 31), if p & 0x8000 != 0 { 255 } else { 0 }]);
        }
        out
    }
}

/// One vertex of `updateVB`'s `VERTEX_FORMAT`, with the normal `doTheLight` was given and the alpha it was given.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerrainVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// `normalAtTexel`.
    pub normal: [f32; 3],
    /// The base texture's UVs (`u1`, `v1`).
    pub u1: f32,
    pub v1: f32,
    /// The blend layer's UVs (`u2`, `v2`).
    pub u2: f32,
    pub v2: f32,
    /// The blend layer's alpha at this corner (the diffuse alpha `doTheLight` sets).
    pub alpha: u8,
}

/// One cell of `renderExtraBlendTiles` (the third texture layer): four corners in `(x, y)`, `(x+1, y)`, `(x+1, y+1)`,
/// `(x, y+1)` order, the UVs (`u1`, `v1`), the alphas, and whether the diagonal is flipped.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExtraBlendCell {
    pub x: i32,
    pub y: i32,
    pub positions: [[f32; 3]; 4],
    pub u: [f32; 4],
    pub v: [f32; 4],
    pub alpha: [u8; 4],
    pub flip: bool,
}

impl ExtraBlendCell {
    /// `renderExtraBlendTiles`' indices for the cell: `(1, 3, 0), (1, 2, 3)` when flipped, else `(0, 2, 3), (0, 1, 2)`.
    pub fn indices(&self) -> [u32; 6] {
        if self.flip {
            [1, 3, 0, 1, 2, 3]
        } else {
            [0, 2, 3, 0, 1, 2]
        }
    }
}

/// `getExtraAlphaUVData`'s outputs: `U`, `V`, `alpha`, `needFlip`, `cliff`.
pub type ExtraAlphaUv = ([f32; 4], [f32; 4], [u8; 4], bool, bool);

/// A cursor over the bytes of `BlendTileData` the logic did not read (`DataChunkInput`'s reads).
struct Rest<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Rest<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], TerrainMapError> {
        let s = self.b.get(self.pos..self.pos + N).ok_or(TerrainMapError::Corrupt)?;
        self.pos += N;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }
    fn read_int(&mut self) -> Result<i32, TerrainMapError> {
        Ok(i32::from_le_bytes(self.take::<4>()?))
    }
    fn read_real(&mut self) -> Result<f32, TerrainMapError> {
        Ok(f32::from_le_bytes(self.take::<4>()?))
    }
    fn read_byte(&mut self) -> Result<u8, TerrainMapError> {
        Ok(self.take::<1>()?[0])
    }
    fn read_ascii_string(&mut self) -> Result<Vec<u8>, TerrainMapError> {
        let len = usize::from(u16::from_le_bytes(self.take::<2>()?));
        let s = self.b.get(self.pos..self.pos + len).ok_or(TerrainMapError::Corrupt)?;
        self.pos += len;
        Ok(s.iter().copied().take_while(|&c| c != 0).collect())
    }
}

/// `WorldHeightMap`, the client's: the samples and everything `BlendTileData` holds, the tiles, and the atlas layout.
#[derive(Clone, Debug)]
pub struct TerrainHeightMap {
    pub width: i32,
    pub height: i32,
    pub border_size: i32,
    pub data_size: i32,
    pub data: Vec<u8>,
    pub tile_ndxes: Vec<i16>,
    pub blend_tile_ndxes: Vec<i16>,
    pub cliff_info_ndxes: Vec<i16>,
    pub extra_blend_tile_ndxes: Vec<i16>,
    pub num_bitmap_tiles: i32,
    pub num_edge_tiles: i32,
    pub num_blended_tiles: i32,
    /// `m_sourceTiles`, `NUM_SOURCE_TILES` slots.
    pub source_tiles: Vec<Option<TileData>>,
    /// `m_edgeTiles`, `NUM_SOURCE_TILES` slots.
    pub edge_tiles: Vec<Option<TileData>>,
    /// `m_blendedTiles[0 .. m_numBlendedTiles]` (entry 0 is all zeros, as upstream's static array).
    pub blended_tiles: Vec<BlendTileInfo>,
    /// `m_cliffInfo[0 .. m_numCliffInfo]` (entry 0 is the default).
    pub cliff_info: Vec<CliffInfo>,
    pub num_cliff_info: i32,
    pub texture_classes: Vec<TxTextureClass>,
    pub edge_texture_classes: Vec<TxTextureClass>,
    /// `m_terrainTexHeight` (1 until the atlas is built).
    pub terrain_tex_height: i32,
    /// `m_alphaEdgeHeight` (1 until the atlas is built).
    pub alpha_edge_height: i32,
    pub cell_flip_state: Vec<u8>,
    pub flip_state_width: i32,
    pub draw_origin_x: i32,
    pub draw_origin_y: i32,
    /// `TheGlobalData->m_adjustCliffTextures`.
    pub adjust_cliff_textures: bool,
}

fn tex_class(c: &TextureClass) -> TxTextureClass {
    TxTextureClass { first_tile: c.first_tile, num_tiles: c.num_tiles, width: c.width, name: c.name.clone(), position_in_texture: ICoord2D { x: 0, y: 0 } }
}

impl TerrainHeightMap {
    /// `WorldHeightMap(pStrm, false)` from the logic's copy of the map: the rest of `ParseBlendTileData` (edge classes,
    /// blend tiles, cliff infos), with `TheGlobalData->m_use3WayTerrainBlends` (`use_3way_terrain_blends`) applied as the
    /// parse applies it, then the constructor's "patch bad maps". No tile is read yet ([`Self::read_tex_classes`]).
    pub fn from_logic(map: &LogicHeightMap, use_3way_terrain_blends: i32, adjust_cliff_textures: bool) -> Result<Self, TerrainMapError> {
        let b = map.blend_tiles().ok_or(TerrainMapError::NoBlendTileData)?;
        if b.version == K_BLEND_TILE_VERSION_1 {
            return Err(TerrainMapError::Version1);
        }
        let (width, height) = (map.x_extent(), map.y_extent());
        let data_size = width * height;
        let mut extra = b.extra_blend_tile_ndxes.clone();
        if b.version >= K_BLEND_TILE_VERSION_6 && use_3way_terrain_blends == 0 {
            // "Allow clearing of extra blend tiles via ini and resaving of map."
            extra.iter_mut().for_each(|e| *e = 0);
        }
        if b.num_blended_tiles > NUM_BLEND_TILES || b.num_cliff_info > NUM_CLIFF_INFO {
            return Err(TerrainMapError::Undefined("blend tile or cliff info count past upstream's array"));
        }
        if b.texture_classes.len() > NUM_TEXTURE_CLASSES as usize {
            return Err(TerrainMapError::Undefined("texture class count past upstream's array"));
        }
        let mut r = Rest { b: &b.rest, pos: 0 };
        let mut num_edge_tiles = 0;
        let mut edge_texture_classes = Vec::new();
        if b.version >= K_BLEND_TILE_VERSION_4 {
            num_edge_tiles = r.read_int()?;
            let n = r.read_int()?;
            if n > NUM_TEXTURE_CLASSES {
                return Err(TerrainMapError::Undefined("edge class count past upstream's array"));
            }
            for _ in 0..n.max(0) {
                let first_tile = r.read_int()?;
                let num_tiles = r.read_int()?;
                let width = r.read_int()?;
                let name = r.read_ascii_string()?;
                edge_texture_classes.push(TxTextureClass { first_tile, num_tiles, width, name, position_in_texture: ICoord2D { x: 0, y: 0 } });
            }
        }
        let mut blended_tiles = vec![BlendTileInfo::default(); b.num_blended_tiles.max(1) as usize];
        for t in blended_tiles.iter_mut().skip(1) {
            t.blend_ndx = r.read_int()?;
            t.horiz = r.read_byte()?;
            t.vert = r.read_byte()?;
            t.right_diagonal = r.read_byte()?;
            t.left_diagonal = r.read_byte()?;
            t.inverted = r.read_byte()?;
            if use_3way_terrain_blends == 0 {
                t.inverted &= !FLIPPED_MASK; // filter out extra flips from 3-way
            }
            t.long_diagonal = if b.version >= K_BLEND_TILE_VERSION_3 { r.read_byte()? } else { 0 };
            t.custom_blend_edge_class = if b.version >= K_BLEND_TILE_VERSION_4 { r.read_int()? } else { -1 };
            if r.read_int()? != FLAG_VAL {
                return Err(TerrainMapError::Corrupt);
            }
        }
        let mut cliff_info = vec![CliffInfo::default(); b.num_cliff_info.max(1) as usize];
        if b.version >= K_BLEND_TILE_VERSION_5 {
            for c in cliff_info.iter_mut().skip(1) {
                c.tile_index = r.read_int()? as i16;
                c.u0 = r.read_real()?;
                c.v0 = r.read_real()?;
                c.u1 = r.read_real()?;
                c.v1 = r.read_real()?;
                c.u2 = r.read_real()?;
                c.v2 = r.read_real()?;
                c.u3 = r.read_real()?;
                c.v3 = r.read_real()?;
                c.flip = r.read_byte()? != 0;
                c.mutant = r.read_byte()? != 0;
            }
        }
        let mut cliff_info_ndxes = b.cliff_info_ndxes.clone();
        let mut blend_tile_ndxes = b.blend_tile_ndxes.clone();
        // patch bad maps.
        for i in 0..data_size.max(0) as usize {
            if cliff_info_ndxes[i] < 0 || i32::from(cliff_info_ndxes[i]) >= b.num_cliff_info {
                cliff_info_ndxes[i] = 0;
            }
            if blend_tile_ndxes[i] < 0 || i32::from(blend_tile_ndxes[i]) >= b.num_blended_tiles {
                blend_tile_ndxes[i] = 0;
            }
            if extra[i] < 0 || i32::from(extra[i]) >= b.num_blended_tiles {
                extra[i] = 0;
            }
        }
        let flip_state_width = (width + 7) / 8;
        Ok(TerrainHeightMap {
            width,
            height,
            border_size: map.border_size(),
            data_size,
            data: map.data()[..data_size as usize].to_vec(),
            tile_ndxes: b.tile_ndxes.clone(),
            blend_tile_ndxes,
            cliff_info_ndxes,
            extra_blend_tile_ndxes: extra,
            num_bitmap_tiles: b.num_bitmap_tiles,
            num_edge_tiles,
            num_blended_tiles: b.num_blended_tiles,
            source_tiles: vec![None; NUM_SOURCE_TILES],
            edge_tiles: vec![None; NUM_SOURCE_TILES],
            blended_tiles,
            cliff_info,
            num_cliff_info: b.num_cliff_info,
            texture_classes: b.texture_classes.iter().map(tex_class).collect(),
            edge_texture_classes,
            terrain_tex_height: 1,
            alpha_edge_height: 1,
            cell_flip_state: vec![0; (flip_state_width * height).max(0) as usize],
            flip_state_width,
            draw_origin_x: 0,
            draw_origin_y: 0,
            adjust_cliff_textures,
        })
    }

    /// New (upstream crashes on a map without `BlendTileData`): the samples alone, every tile index 0 and no tile, so
    /// [`Self::update_vb`] gives the positions and normals with zero UVs, for an untextured mesh.
    pub fn heights_only(map: &LogicHeightMap) -> Self {
        let (width, height) = (map.x_extent(), map.y_extent());
        let n = (width * height).max(0) as usize;
        let flip_state_width = (width + 7) / 8;
        TerrainHeightMap {
            width,
            height,
            border_size: map.border_size(),
            data_size: width * height,
            data: map.data()[..n].to_vec(),
            tile_ndxes: vec![0; n],
            blend_tile_ndxes: vec![0; n],
            cliff_info_ndxes: vec![0; n],
            extra_blend_tile_ndxes: vec![0; n],
            num_bitmap_tiles: 0,
            num_edge_tiles: 0,
            num_blended_tiles: 1,
            source_tiles: vec![None; NUM_SOURCE_TILES],
            edge_tiles: vec![None; NUM_SOURCE_TILES],
            blended_tiles: vec![BlendTileInfo::default()],
            cliff_info: vec![CliffInfo::default()],
            num_cliff_info: 1,
            texture_classes: Vec::new(),
            edge_texture_classes: Vec::new(),
            terrain_tex_height: 1,
            alpha_edge_height: 1,
            cell_flip_state: vec![0; (flip_state_width * height).max(0) as usize],
            flip_state_width,
            draw_origin_x: 0,
            draw_origin_y: 0,
            adjust_cliff_textures: false,
        }
    }

    /// `readTexClass` for every texture class and edge class, as `ParseBlendTileData` calls it: the class's terrain in
    /// `TheTerrainTypes`, its texture opened from `Art/Terrain/` (`open` gives a file's bytes, `None` when it cannot be
    /// opened); a class whose terrain is unknown reads nothing (`LOAD_TEST_ASSETS` is not defined). Returns how many
    /// classes found their file.
    pub fn read_tex_classes(&mut self, terrain_types: &TerrainTypeCollection, open: &mut impl FnMut(&[u8]) -> Option<Vec<u8>>) -> usize {
        self.read_tex_classes_with(&|name| terrain_types.find_terrain(name).map(|t| t.texture().to_vec()), open)
    }

    /// [`Self::read_tex_classes`] with the lookup of a terrain's texture by name given (`None`: no such terrain).
    pub fn read_tex_classes_with(&mut self, texture_of: &impl Fn(&[u8]) -> Option<Vec<u8>>, open: &mut impl FnMut(&[u8]) -> Option<Vec<u8>>) -> usize {
        let mut found = 0;
        for edge in [false, true] {
            let n = if edge { self.edge_texture_classes.len() } else { self.texture_classes.len() };
            for i in 0..n {
                let class = if edge { self.edge_texture_classes[i].clone() } else { self.texture_classes[i].clone() };
                let Some(texture) = texture_of(&class.name) else { continue };
                let mut path = TERRAIN_TGA_DIR_PATH.to_vec();
                path.extend_from_slice(&texture);
                let Some(bytes) = open(&path) else { continue };
                found += 1;
                let tiles = if edge { &mut self.edge_tiles } else { &mut self.source_tiles };
                read_tex_class(&class, &bytes, tiles);
            }
        }
        found
    }

    /// Whether any tile was read (else there is nothing to texture with).
    pub fn has_tiles(&self) -> bool {
        self.source_tiles.iter().any(Option::is_some)
    }

    fn source_tile(&self, ndx: i32) -> Option<&TileData> {
        usize::try_from(ndx).ok().and_then(|n| self.source_tiles.get(n)).and_then(Option::as_ref)
    }

    /// `getDisplayHeight(x, y)`.
    pub fn get_display_height(&self, x: i32, y: i32) -> u8 {
        self.data[(x + self.draw_origin_x + self.width * (y + self.draw_origin_y)) as usize]
    }

    /// `getHeight(xIndex, yIndex)`: 0 outside the data.
    pub fn get_height(&self, x: i32, y: i32) -> u8 {
        let ndx = y * self.width + x;
        if ndx >= 0 && ndx < self.data_size {
            self.data[ndx as usize]
        } else {
            0
        }
    }

    /// `updateTileTexturePositions(&edgeHeight)`: places each class (widest first, then in class order) at the first
    /// grid position whose row's first free cell has room, and each of its tiles within it; then the edge classes in a
    /// second grid. Returns `(maxHeight, edgeHeight)`.
    pub fn update_tile_texture_positions(&mut self) -> (i32, i32) {
        const TILES_PER_ROW: i32 = TEXTURE_WIDTH / (TILE_PIXEL_EXTENT + TILE_OFFSET);
        let mut max_height = 0;
        let mut grid = [[true; TILES_PER_ROW as usize]; TILES_PER_ROW as usize];
        for i in 0..self.num_bitmap_tiles.clamp(0, NUM_SOURCE_TILES as i32) as usize {
            if let Some(t) = &mut self.source_tiles[i] {
                t.tile_location_in_texture = ICoord2D { x: 0, y: 0 };
            }
        }
        let place = |grid: &mut [[bool; TILES_PER_ROW as usize]; TILES_PER_ROW as usize], width: i32| -> Option<(i32, i32)> {
            let mut found = false;
            let (mut row, mut column) = (0, 0);
            'rows: while row < TILES_PER_ROW - width + 1 && !found {
                column = 0;
                while column < TILES_PER_ROW - width + 1 && !found {
                    if grid[row as usize][column as usize] {
                        let mut open = true;
                        for i in 0..width {
                            for j in 0..width {
                                if !grid[(row + j) as usize][(column + i) as usize] {
                                    open = false;
                                }
                            }
                        }
                        if open {
                            found = true;
                        }
                        break;
                    }
                    column += 1;
                }
                if found {
                    break 'rows;
                }
                row += 1;
            }
            found.then_some((row, column))
        };
        let mut tile_width = TILES_PER_ROW;
        while tile_width > 0 {
            for tc in 0..self.texture_classes.len() {
                let width = self.texture_classes[tc].width;
                if width != tile_width {
                    continue;
                }
                let Some((row, column)) = place(&mut grid, width) else {
                    self.texture_classes[tc].position_in_texture = ICoord2D { x: 0, y: 0 };
                    continue;
                };
                let x_origin = TILE_OFFSET / 2 + column * (TILE_PIXEL_EXTENT + TILE_OFFSET);
                let y_origin = TILE_OFFSET / 2 + row * (TILE_PIXEL_EXTENT + TILE_OFFSET);
                self.texture_classes[tc].position_in_texture = ICoord2D { x: x_origin, y: y_origin };
                let class_height = y_origin + width * TILE_PIXEL_EXTENT + TILE_OFFSET / 2;
                max_height = max_height.max(class_height);
                for i in 0..width {
                    for j in 0..width {
                        grid[(row + j) as usize][(column + i) as usize] = false;
                        let base_ndx = self.texture_classes[tc].first_tile + i + j * width;
                        if let Some(Some(t)) = usize::try_from(base_ndx).ok().and_then(|n| self.source_tiles.get_mut(n)) {
                            t.tile_location_in_texture = ICoord2D { x: x_origin + i * TILE_PIXEL_EXTENT, y: y_origin + (width - j - 1) * TILE_PIXEL_EXTENT };
                        }
                    }
                }
            }
            tile_width -= 1;
        }
        for i in 0..self.num_bitmap_tiles.clamp(0, NUM_SOURCE_TILES as i32) as usize {
            if let Some(t) = &mut self.edge_tiles[i] {
                t.tile_location_in_texture = ICoord2D { x: 0, y: 0 };
            }
        }
        let mut max_edge_height = 0;
        let mut grid = [[true; TILES_PER_ROW as usize]; TILES_PER_ROW as usize];
        for tc in 0..self.edge_texture_classes.len() {
            let width = self.edge_texture_classes[tc].width;
            // (a width past the grid finds no room, and a width below 1 "fits" at the first free cell, as upstream)
            let Some((row, column)) = place(&mut grid, width) else {
                self.edge_texture_classes[tc].position_in_texture = ICoord2D { x: 0, y: 0 };
                continue;
            };
            let x_origin = TILE_OFFSET / 2 + column * (TILE_PIXEL_EXTENT + TILE_OFFSET);
            let y_origin = TILE_OFFSET / 2 + row * (TILE_PIXEL_EXTENT + TILE_OFFSET);
            self.edge_texture_classes[tc].position_in_texture = ICoord2D { x: x_origin, y: y_origin };
            let class_height = y_origin + width * TILE_PIXEL_EXTENT + TILE_OFFSET / 2;
            max_edge_height = max_edge_height.max(class_height);
            for i in 0..width {
                for j in 0..width {
                    grid[(row + j) as usize][(column + i) as usize] = false;
                    let base_ndx = self.edge_texture_classes[tc].first_tile + i + j * width;
                    if let Some(Some(t)) = usize::try_from(base_ndx).ok().and_then(|n| self.edge_tiles.get_mut(n)) {
                        t.tile_location_in_texture = ICoord2D { x: x_origin + i * TILE_PIXEL_EXTENT, y: y_origin + (width - j - 1) * TILE_PIXEL_EXTENT };
                    }
                }
            }
        }
        (max_height, max_edge_height)
    }

    /// `getTerrainTexture()`'s first build: the positions, the atlas at the next power of two in height
    /// (`TerrainTextureClass(pow2Height)` and its `update`), `m_terrainTexHeight` and `m_alphaEdgeHeight`, and each cell's
    /// flip bit from `getAlphaUVData`.
    pub fn build_terrain_texture(&mut self) -> TerrainAtlas {
        let (height, edge_height) = self.update_tile_texture_positions();
        let pow2 = |h: i32| {
            let mut p = 1;
            while p < h {
                p *= 2;
            }
            p
        };
        let atlas = self.terrain_texture_update(pow2(height));
        self.terrain_tex_height = atlas.height;
        self.alpha_edge_height = pow2(edge_height);
        for y in 0..self.height - 1 {
            for x in 0..self.width - 1 {
                let (_, _, _, flip) = self.get_alpha_uv_data(x, y, false);
                self.cell_flip_state[(y * self.flip_state_width + (x >> 3)) as usize] |= u8::from(flip) << (x & 0x7);
            }
        }
        atlas
    }

    /// `TerrainTextureClass::update(htMap)` (the version compiled, "Sets the tile bitmap data into the texture"): each
    /// placed tile (`x > 0`) written upside down as A1R5G5B5 at its location, then round each placed class 4 columns of
    /// its far side before it and of its near side after it, then 4 rows likewise (wrapping, so the tiles filter
    /// seamlessly).
    pub fn terrain_texture_update(&self, height: i32) -> TerrainAtlas {
        let w = TEXTURE_WIDTH as usize;
        let mut px = vec![0u16; w * height.max(0) as usize];
        let ext = TILE_PIXEL_EXTENT as usize;
        for tile_ndx in 0..self.num_bitmap_tiles.max(0) {
            let Some(tile) = self.source_tile(tile_ndx) else { continue };
            let position = tile.tile_location_in_texture;
            if position.x <= 0 {
                continue; // all real tile offsets start at 2.  jba.
            }
            for j in 0..ext {
                let src = &tile.tile_data[(ext - 1 - j) * TILE_BYTES_PER_PIXEL * ext..];
                let row = position.y as usize + j;
                for i in 0..ext {
                    let p = &src[i * TILE_BYTES_PER_PIXEL..];
                    px[row * w + position.x as usize + i] = 0x8000 + ((u16::from(p[2]) >> 3) << 10) + ((u16::from(p[1]) >> 3) << 5) + (u16::from(p[0]) >> 3);
                }
            }
        }
        for class in &self.texture_classes {
            let origin = class.position_in_texture;
            if origin.x <= 0 {
                continue;
            }
            let width = (class.width * TILE_PIXEL_EXTENT) as usize;
            let (ox, oy) = (origin.x as usize, origin.y as usize);
            for j in 0..width {
                let r = (oy + j) * w + ox;
                // copy before
                px.copy_within(r + width - 4..r + width, r - 4);
                // copy after
                px.copy_within(r..r + 4, r + width);
            }
            for j in 0..4 {
                // copy before.
                let target = (oy - j - 1) * w + ox - 4;
                px.copy_within(target + width * w..target + width * w + width + 8, target);
                // copy after.
                let target = (oy + j) * w + ox - 4;
                px.copy_within(target..target + width + 8, target + width * w);
            }
        }
        TerrainAtlas { width: TEXTURE_WIDTH, height, pixels: px }
    }

    /// `getUVForNdx(tileNdx, &minU, &minV, &maxU, &maxV, fullTile)`: the tile's square in the atlas, or (not `fullTile`)
    /// the quarter `tileNdx & 3` picks ("y's are flipped"). A missing tile gives all zeros.
    pub fn get_uv_for_ndx(&self, tile_ndx: i32, full_tile: bool) -> (f32, f32, f32, f32) {
        let base_ndx = i32::from((tile_ndx >> 2) as i16);
        let Some(tile) = self.source_tile(base_ndx) else {
            // Missing texture.
            return (0.0, 0.0, 0.0, 0.0);
        };
        let pos = tile.tile_location_in_texture;
        let mut min_u = pos.x as f32;
        let mut min_v = pos.y as f32;
        let mut max_u = min_u + TILE_PIXEL_EXTENT as f32;
        let mut max_v = min_v + TILE_PIXEL_EXTENT as f32;
        min_u /= TEXTURE_WIDTH as f32;
        min_v /= self.terrain_tex_height as f32;
        max_u /= TEXTURE_WIDTH as f32;
        max_v /= self.terrain_tex_height as f32;
        if !full_tile {
            let mid_x = (min_u + max_u) / 2.0;
            let mid_y = (min_v + max_v) / 2.0;
            if tile_ndx & 2 != 0 {
                max_v = mid_y;
            } else {
                min_v = mid_y;
            }
            if tile_ndx & 1 != 0 {
                min_u = mid_x;
            } else {
                max_u = mid_x;
            }
        }
        (min_u, min_v, max_u, max_v)
    }

    /// `getUVData(xIndex, yIndex, U, V, fullTile)`: the base layer's UVs of a cell, and whether they were stretched for
    /// a cliff. `None` past the data (upstream returns false and leaves `U`, `V` unwritten).
    pub fn get_uv_data(&self, x: i32, y: i32, full_tile: bool) -> Option<([f32; 4], [f32; 4], bool)> {
        let x = x + self.draw_origin_x;
        let y = y + self.draw_origin_y;
        let ndx = y * self.width + x;
        if ndx < self.data_size {
            let tile_ndx = self.tile_ndxes[ndx as usize];
            return Some(self.get_uv_for_tile_index(ndx, tile_ndx, full_tile));
        }
        None
    }

    /// `getUVForTileIndex(ndx, tileNdx, U, V, fullTile)`: the quarter tile's UVs for corners 0..3 (`(x, y)`, `(x+1, y)`,
    /// `(x+1, y+1)`, `(x, y+1)`); with `m_adjustCliffTextures`, a cell with cliff info in the same class takes the cliff
    /// info's UVs (and returns its `flip`), else a steep cell has its UVs stretched (`DO_OLD_UV`). Returns whether the
    /// UVs were stretched for a cliff.
    pub fn get_uv_for_tile_index(&self, ndx: i32, tile_ndx: i16, full_tile: bool) -> ([f32; 4], [f32; 4], bool) {
        let mut u = [0.0f32; 4];
        let mut v = [0.0f32; 4];
        if ndx >= self.data_size {
            return (u, v, false);
        }
        let (n_u, n_v, x_u, x_v) = self.get_uv_for_ndx(i32::from(tile_ndx), full_tile);
        u = [n_u, x_u, x_u, n_u];
        v = [x_v, x_v, n_v, n_v];
        if !self.adjust_cliff_textures {
            return (u, v, false);
        }
        if n_u == 0.0 {
            return (u, v, false); // missing texture.
        }
        if full_tile {
            return (u, v, false);
        }
        let cliff_ndx = self.cliff_info_ndxes[ndx as usize];
        if cliff_ndx != 0 {
            let info = self.cliff_info[cliff_ndx as usize];
            let mut tiles_match = false;
            let ndx1 = i32::from(tile_ndx) >> 2;
            let ndx2 = i32::from(info.tile_index) >> 2;
            let mut i = 0;
            while i < self.texture_classes.len() {
                let c = &self.texture_classes[i];
                if ndx1 >= c.first_tile && ndx1 < c.first_tile + c.num_tiles {
                    tiles_match = ndx2 >= c.first_tile && ndx2 < c.first_tile + c.num_tiles;
                    break;
                }
                i += 1;
            }
            if tiles_match {
                let c = &self.texture_classes[i];
                let mut min_u = c.position_in_texture.x as f32;
                let mut max_v = (c.position_in_texture.y + c.width * TILE_PIXEL_EXTENT) as f32;
                min_u /= TEXTURE_WIDTH as f32;
                max_v /= self.terrain_tex_height as f32;
                let v_factor = (TEXTURE_WIDTH / self.terrain_tex_height) as f32;
                u = [info.u0 + min_u, info.u1 + min_u, info.u2 + min_u, info.u3 + min_u];
                v = [info.v0 * v_factor + max_v, info.v1 * v_factor + max_v, info.v2 * v_factor + max_v, info.v3 * v_factor + max_v];
                return (u, v, info.flip);
            }
        }
        // old uv adjustment for cliffs (DO_OLD_UV): the same UVs again, then the stretch.
        const STRETCH_LIMIT: f32 = 1.5;
        const TILE_LIMIT: f32 = 4.0;
        const TALL_STRETCH_LIMIT: f32 = 2.0;
        const DIAMOND_STRETCH_LIMIT: f32 = 2.4;
        const HEIGHT_SCALE: f32 = MAP_HEIGHT_SCALE / MAP_XY_FACTOR;
        let (n_u, n_v, x_u, x_v) = self.get_uv_for_ndx(i32::from(tile_ndx), full_tile);
        u = [n_u, x_u, x_u, n_u];
        v = [x_v, x_v, n_v, n_v];
        if n_u == 0.0 {
            return (u, v, false);
        }
        // check for excessive heights.
        if ndx < self.data_size - self.width - 1 {
            let d = |k: i32| i32::from(self.data[k as usize]);
            let h0 = d(ndx);
            let h1 = d(ndx + 1);
            let h2 = d(ndx + self.width + 1);
            let h3 = d(ndx + self.width);
            let min_h = h0.min(h1).min(h2).min(h3);
            let max_h = h0.max(h1).max(h2).max(h3);
            let delta_h = max_h - min_h;
            let below_limit = min_h + (2 * delta_h + 1) / 3;
            let above_limit = min_h + (delta_h + 1) / 3;
            let mut below = [h0, h1, h2, h3].iter().filter(|&&h| h < below_limit).count() as i32;
            let above = [h0, h1, h2, h3].iter().filter(|&&h| h > above_limit).count() as i32;
            if (delta_h as f32) * HEIGHT_SCALE < STRETCH_LIMIT {
                return (u, v, false);
            }
            let base_ndx = i32::from((i32::from(tile_ndx) >> 2) as i16);
            let mut tex_class = 0usize;
            while tex_class < self.texture_classes.len() {
                let c = &self.texture_classes[tex_class];
                if c.first_tile >= 0 && base_ndx >= c.first_tile && base_ndx < c.first_tile + c.num_tiles {
                    break;
                }
                tex_class += 1;
            }
            if tex_class >= self.texture_classes.len() {
                return (u, v, false);
            }
            let c = &self.texture_classes[tex_class];
            let mut n_ub = c.position_in_texture.x as f32;
            let mut n_vb = c.position_in_texture.y as f32;
            let mut x_ub = n_ub + (c.width * TILE_PIXEL_EXTENT) as f32;
            let mut x_vb = n_vb + (c.width * TILE_PIXEL_EXTENT) as f32;
            n_ub /= TEXTURE_WIDTH as f32;
            n_vb /= self.terrain_tex_height as f32;
            x_ub /= TEXTURE_WIDTH as f32;
            x_vb /= self.terrain_tex_height as f32;
            let _ = n_ub;
            // too much stretch.
            // (upstream caps at TILE_LIMIT, then at 1.0 below: delta_h > 0 here, so the value is finite and that is a clamp)
            let divisor = (TILE_LIMIT / ((delta_h as f32) * HEIGHT_SCALE)).clamp(1.0, TILE_LIMIT);
            let delta_v = x_vb - n_vb;
            if above != 1 && below != 1 && (above != 2 || below != 2) {
                // diamond shaped.
                if (delta_h as f32) * HEIGHT_SCALE < DIAMOND_STRETCH_LIMIT {
                    return (u, v, false);
                }
            }
            if below == 1 || above > below {
                // we got one low guy.
                if h0 == min_h {
                    v[0] = n_v + delta_v / divisor;
                } else if h1 == min_h {
                    v[1] = n_v + delta_v / divisor;
                } else if h2 == min_h {
                    v[2] = x_v - delta_v / divisor;
                } else if h3 == min_h {
                    v[3] = x_v - delta_v / divisor;
                }
            } else if above == 1 || below > above {
                // we got one high guy
                if h0 == max_h {
                    v[0] = n_v + delta_v / divisor;
                } else if h1 == max_h {
                    v[1] = n_v + delta_v / divisor;
                } else if h2 == max_h {
                    v[2] = x_v - delta_v / divisor;
                } else if h3 == max_h {
                    v[3] = x_v - delta_v / divisor;
                }
            } else {
                // we got two up and two down.
                if (delta_h as f32) * HEIGHT_SCALE < TALL_STRETCH_LIMIT {
                    return (u, v, false);
                }
                let side = |a: i32, b: i32| {
                    let mut s = ((a - b) as f32) * HEIGHT_SCALE;
                    s = petir::Real::sqrt(1.0 + s * s);
                    if s < STRETCH_LIMIT {
                        s = 1.0; // don't make a seam unless there is great stretch.
                    }
                    if s > TILE_LIMIT {
                        s = TILE_LIMIT; // don't tile past the texture's edge.
                    }
                    s
                };
                let dx = side(h3, h2) * (x_u - n_u);
                let dy = side(h3, h0) * (x_v - n_v);
                u = [n_u, n_u + dx, n_u + dx, n_u];
                v = [n_v + dy, n_v + dy, n_v, n_v];
                if below == 1 {
                    below = 1;
                }
                let _ = below;
                // recalc for point 1.
                let dx = side(h1, h0) * (x_u - n_u);
                let dy = side(h2, h1) * (x_v - n_v);
                u[1] = u[0] + dx;
                v[1] = v[3] + dy;
            }
            // Make sure we are within the texture;
            let mut adj_u = 0.0f32;
            let mut adj_v = 0.0f32;
            for vi in v {
                if n_vb - vi > adj_v {
                    adj_v = n_vb - vi;
                }
            }
            for vi in &mut v {
                *vi += adj_v;
            }
            adj_v = 0.0;
            for i in 0..4 {
                if u[i] - x_ub > adj_u {
                    adj_u = u[i] - x_ub;
                }
                if v[i] - x_vb > adj_v {
                    adj_v = v[i] - x_vb;
                }
            }
            for i in 0..4 {
                u[i] -= adj_u;
                v[i] -= adj_v;
            }
        }
        (u, v, true)
    }

    /// The blend flags of a blend tile into the corner alphas and the flip (`getAlphaUVData`'s and
    /// `getExtraAlphaUVData`'s shared body).
    fn blend_alpha(t: &BlendTileInfo, need_flip: &mut bool) -> [u8; 4] {
        let mut alpha = [0u8; 4];
        if t.horiz != 0 {
            // Horizontals don't need flipping unless forced because of 3way blend.
            *need_flip = t.inverted & FLIPPED_MASK != 0;
            if t.inverted & INVERTED_MASK != 0 {
                alpha[0] = 255;
                alpha[3] = 255;
            } else {
                alpha[1] = 255;
                alpha[2] = 255;
            }
        }
        if t.vert != 0 {
            *need_flip = t.inverted & FLIPPED_MASK != 0;
            if t.inverted & INVERTED_MASK != 0 {
                alpha[0] = 255;
                alpha[1] = 255;
            } else {
                alpha[2] = 255;
                alpha[3] = 255;
            }
        }
        if t.right_diagonal != 0 {
            if t.inverted & INVERTED_MASK != 0 {
                alpha[1] = 255;
                if t.long_diagonal != 0 {
                    alpha[0] = 255;
                    alpha[2] = 255;
                }
            } else {
                // Uninverted right diagonals need flipping.
                *need_flip = true;
                alpha[2] = 255;
                if t.long_diagonal != 0 {
                    alpha[1] = 255;
                    alpha[3] = 255;
                }
            }
        }
        if t.left_diagonal != 0 {
            if t.inverted & INVERTED_MASK != 0 {
                // Inverted left diagonals need flipping.
                *need_flip = true;
                alpha[0] = 255;
                if t.long_diagonal != 0 {
                    alpha[1] = 255;
                    alpha[3] = 255;
                }
            } else {
                alpha[3] = 255;
                if t.long_diagonal != 0 {
                    alpha[0] = 255;
                    alpha[2] = 255;
                }
            }
        }
        if t.custom_blend_edge_class >= 0 {
            alpha = [0; 4];
            // No alpha blend, so never need to flip.
            *need_flip = false;
        }
        alpha
    }

    /// `getAlphaUVData(xIndex, yIndex, U, V, alpha, &flip, fullTile)`: the blend layer's UVs (the blend tile's source
    /// tile, or the base tile when there is no blend), the corner alphas, and whether the cell's diagonal is flipped
    /// (`FLIP_TRIANGLES`). `U`, `V` stay zero past the data (upstream leaves them unwritten).
    pub fn get_alpha_uv_data(&self, x: i32, y: i32, full_tile: bool) -> ([f32; 4], [f32; 4], [u8; 4], bool) {
        let x = x + self.draw_origin_x;
        let y = y + self.draw_origin_y;
        let ndx = y * self.width + x;
        let mut stretched_for_cliff = false;
        let mut need_flip = false;
        let mut u = [0.0f32; 4];
        let mut v = [0.0f32; 4];
        let mut alpha = [0u8; 4];
        if ndx < self.data_size {
            let mut blend_ndx = self.blend_tile_ndxes[ndx as usize];
            if full_tile {
                blend_ndx = 0;
            }
            if blend_ndx == 0 {
                (u, v, stretched_for_cliff) = self.get_uv_for_tile_index(ndx, self.tile_ndxes[ndx as usize], full_tile);
                // No alpha blend, so never need to flip.
                need_flip = false;
            } else {
                let t = &self.blended_tiles[blend_ndx as usize];
                (u, v, stretched_for_cliff) = self.get_uv_for_tile_index(ndx, t.blend_ndx as i16, full_tile);
                alpha = Self::blend_alpha(t, &mut need_flip);
            }
        }
        if stretched_for_cliff {
            // If we had to stretch for clif, check heights.
            let p0 = i32::from(self.get_height(x, y));
            let p1 = i32::from(self.get_height(x + 1, y));
            let p2 = i32::from(self.get_height(x + 1, y + 1));
            let p3 = i32::from(self.get_height(x, y + 1));
            need_flip = (p0 - p2).abs() > (p1 - p3).abs();
        }
        (u, v, alpha, need_flip)
    }

    /// `getExtraAlphaUVData(xIndex, yIndex, U, V, alpha, &needFlip, &cliff)`: the third layer of a cell. `None` when the
    /// cell has none (upstream's FALSE); inside the data, the UVs, alphas, flip and cliff stretch of its blend tile.
    pub fn get_extra_alpha_uv_data(&self, x: i32, y: i32) -> Option<ExtraAlphaUv> {
        let ndx = y * self.width + x;
        let mut need_flip = false;
        if ndx >= 0 && ndx < self.data_size {
            let blend_ndx = self.extra_blend_tile_ndxes[ndx as usize];
            if blend_ndx == 0 {
                return None;
            }
            let t = &self.blended_tiles[blend_ndx as usize];
            let (u, v, cliff) = self.get_uv_for_tile_index(ndx, t.blend_ndx as i16, false);
            let alpha = Self::blend_alpha(t, &mut need_flip);
            return Some((u, v, alpha, need_flip, cliff));
        }
        Some(([0.0; 4], [0.0; 4], [0; 4], false, false))
    }

    /// `HeightMapRenderObjClass::getXWithOrigin(x)` with `m_originX = 0` and `m_x` the map's width (the whole map drawn).
    fn x_with_origin(&self, mut x: i32) -> i32 {
        let m_x = self.width;
        if x < 0 {
            x += m_x - 1;
        }
        if x >= m_x - 1 {
            x -= m_x - 1;
        }
        x.clamp(0, m_x - 1)
    }

    /// `getYWithOrigin(y)`, likewise.
    fn y_with_origin(&self, mut y: i32) -> i32 {
        let m_y = self.height;
        if y < 0 {
            y += m_y - 1;
        }
        if y >= m_y - 1 {
            y -= m_y - 1;
        }
        y.clamp(0, m_y - 1)
    }

    /// `HeightMapRenderObjClass::updateVB` over every cell of the map (`updateBlock(0, 0, x-1, y-1)` after
    /// `initHeightData(width, height)`): four vertices per cell, cells row by row (`(height-1) * (width-1)` of them), each
    /// cell's vertices rotated by one when `getAlphaUVData` flips it. Draw each cell with [`TERRAIN_CELL_INDICES`].
    pub fn update_vb(&self) -> Vec<TerrainVertex> {
        let cell_offset = 1;
        let mut out = Vec::with_capacity(((self.width - 1).max(0) * (self.height - 1).max(0) * 4) as usize);
        let dh = |x: i32, y: i32| i32::from(self.get_display_height(x, y));
        let adjust = |k: i32| (k as f32 - self.border_size as f32) * MAP_XY_FACTOR;
        let normal = |l2r_z: i32, n2f_z: i32| {
            let l2r = Vector3::new(2.0 * MAP_XY_FACTOR, 0.0, MAP_HEIGHT_SCALE * l2r_z as f32);
            let n2f = Vector3::new(0.0, 2.0 * MAP_XY_FACTOR, MAP_HEIGHT_SCALE * n2f_z as f32);
            let n = Vector3::normalized_cross_product(&l2r, &n2f);
            [n.x, n.y, n.z]
        };
        for j in 0..self.height - 1 {
            let yo = self.y_with_origin(j);
            let mut vn0 = yo - cell_offset;
            if vn0 < -self.draw_origin_y {
                vn0 = -self.draw_origin_y;
            }
            let mut vp1 = self.y_with_origin(j + cell_offset) + cell_offset;
            if vp1 >= self.height - self.draw_origin_y {
                vp1 = self.height - self.draw_origin_y - 1;
            }
            let y_coord = yo + self.draw_origin_y;
            for i in 0..self.width - 1 {
                let xo = self.x_with_origin(i);
                let mut un0 = xo - cell_offset;
                if un0 < -self.draw_origin_x {
                    un0 = -self.draw_origin_x;
                }
                let mut up1 = self.x_with_origin(i + cell_offset) + cell_offset;
                if up1 >= self.width - self.draw_origin_x {
                    up1 = self.width - self.draw_origin_x - 1;
                }
                let x_coord = xo + self.draw_origin_x;
                let (u, v, _) = self.get_uv_data(xo, yo, false).unwrap_or(([0.0; 4], [0.0; 4], false));
                let (ua, va, alpha, flip_for_blend) = self.get_alpha_uv_data(xo, yo, false);
                // the last row: `yCoord + 1 == drawOrgY + m_y - 1` gives yCoord+1, the same as yCoord+cellOffset here
                let corners = [
                    (x_coord, y_coord, dh(xo, yo), normal(dh(xo + cell_offset, yo) - dh(un0, yo), dh(xo, yo + cell_offset) - dh(xo, vn0))),
                    (
                        x_coord + cell_offset,
                        y_coord,
                        dh(xo + cell_offset, yo),
                        normal(dh(up1, yo) - dh(xo, yo), dh(xo + cell_offset, yo + cell_offset) - dh(xo + cell_offset, vn0)),
                    ),
                    (
                        x_coord + cell_offset,
                        y_coord + cell_offset,
                        dh(xo + cell_offset, yo + cell_offset),
                        normal(dh(up1, yo + cell_offset) - dh(xo, yo + cell_offset), dh(xo + cell_offset, vp1) - dh(xo + cell_offset, yo)),
                    ),
                    (
                        x_coord,
                        y_coord + cell_offset,
                        dh(xo, yo + cell_offset),
                        normal(dh(xo + cell_offset, yo + cell_offset) - dh(un0, yo + cell_offset), dh(xo, vp1) - dh(xo, yo)),
                    ),
                ];
                let mut cell = [TerrainVertex::default(); 4];
                for (k, (x, y, z, n)) in corners.into_iter().enumerate() {
                    cell[k] = TerrainVertex {
                        x: adjust(x),
                        y: adjust(y),
                        z: z as f32 * MAP_HEIGHT_SCALE,
                        normal: n,
                        u1: u[k],
                        v1: v[k],
                        u2: ua[k],
                        v2: va[k],
                        alpha: alpha[k],
                    };
                }
                if flip_for_blend {
                    cell.rotate_left(1);
                }
                out.extend_from_slice(&cell);
            }
        }
        out
    }

    /// `initHeightData`'s list of extra blend tiles and `renderExtraBlendTiles`' vertices for them: each cell with a
    /// third layer, its corners at `(x - border) * MAP_XY_FACTOR` and the raw heights, flipped as the layer asks or, for
    /// a cliff-stretched cell, when `|p0 - p2| > |p1 - p3|`.
    pub fn extra_blend_cells(&self) -> Vec<ExtraBlendCell> {
        let mut out = Vec::new();
        let border = self.border_size;
        for y in 0..self.height - 1 {
            for x in 0..self.width - 1 {
                let Some((u, v, alpha, mut flip, cliff)) = self.get_extra_alpha_uv_data(x, y) else { continue };
                let idx = (x + y * self.width) as usize;
                let w = self.width as usize;
                let p0 = f32::from(self.data[idx]) * MAP_HEIGHT_SCALE;
                let p1 = f32::from(self.data[idx + 1]) * MAP_HEIGHT_SCALE;
                let p2 = f32::from(self.data[idx + 1 + w]) * MAP_HEIGHT_SCALE;
                let p3 = f32::from(self.data[idx + w]) * MAP_HEIGHT_SCALE;
                if cliff && (p0 - p2).abs() > (p1 - p3).abs() {
                    flip = true; // cliffs sometimes force a flip
                }
                let px = |k: i32| ((k - border) as f32) * MAP_XY_FACTOR;
                out.push(ExtraBlendCell {
                    x,
                    y,
                    positions: [[px(x), px(y), p0], [px(x + 1), px(y), p1], [px(x + 1), px(y + 1), p2], [px(x), px(y + 1), p3]],
                    u,
                    v,
                    alpha,
                    flip,
                });
            }
        }
        out
    }
}

/// A Targa read as `GDIFileStream` reads it: a short read copies what there is and leaves the rest of the buffer.
struct TileStream<'a> {
    b: &'a [u8],
    pos: usize,
}

impl TileStream<'_> {
    fn read(&mut self, dst: &mut [u8]) -> usize {
        let n = dst.len().min(self.b.len().saturating_sub(self.pos));
        dst[..n].copy_from_slice(&self.b[self.pos..self.pos + n]);
        self.pos += n;
        n
    }
}

/// `TTargaHeader` (18 bytes): what `countTiles` and `readTiles` read of it.
struct TargaHeader {
    color_map_type: u8,
    image_type: u8,
    image_width: i16,
    image_height: i16,
    pixel_depth: u8,
}

fn read_header(s: &mut TileStream) -> (usize, TargaHeader) {
    let mut h = [0u8; 18];
    let n = s.read(&mut h);
    let sh = |i: usize| i16::from_le_bytes([h[i], h[i + 1]]);
    (n, TargaHeader { color_map_type: h[1], image_type: h[2], image_width: sh(12), image_height: sh(14), pixel_depth: h[16] })
}

/// `WorldHeightMap::countTiles(pStr, halfTile)`: how many tiles a Targa holds (a square number up to 100, or 0), and
/// whether it is a half tile (32x32: one tile, only when the caller asks, `want_half`, as `halfTile` is not NULL;
/// `readTexClass` does not ask, so a half tile counts 0 there).
pub fn count_tiles(tga: &[u8], want_half: bool) -> (i32, bool) {
    let mut s = TileStream { b: tga, pos: 0 };
    let (len, hdr) = read_header(&mut s);
    if len != 18 {
        return (0, false);
    }
    let tile_width = i32::from(hdr.image_width) / TILE_PIXEL_EXTENT;
    let tile_height = i32::from(hdr.image_height) / TILE_PIXEL_EXTENT;
    if hdr.color_map_type != 0 {
        return (0, false); // we don't do indexed at this time. jba.
    }
    if hdr.image_type != 0x2 && hdr.image_type != 0xA {
        return (0, false);
    }
    if hdr.pixel_depth < 24 || hdr.pixel_depth > 32 {
        return (0, false);
    }
    if tile_width > 10 || tile_height > 10 {
        return (0, false); // don't do huge images, or bad files.
    }
    for n in (1..=10).rev() {
        if tile_width >= n && tile_height >= n {
            return (n * n, false);
        }
    }
    if want_half && i32::from(hdr.image_height) == TILE_PIXEL_EXTENT / 2 && i32::from(hdr.image_width) == TILE_PIXEL_EXTENT / 2 {
        return (1, true);
    }
    (0, false)
}

/// `WorldHeightMap::readTiles(pStr, tiles, numRows)`: cuts the Targa's pixels (raw or run-length, 24 or 32 bits) into
/// `numRows * numRows` tiles, BGRA; rows past the image are zeros. False (nothing read) when the image is narrower and
/// shorter than `numRows` tiles or not 3 or 4 bytes a pixel. `tiles` is the slots from the class's first tile.
pub fn read_tiles(tga: &[u8], tiles: &mut [Option<TileData>], num_rows: i32) -> bool {
    let mut s = TileStream { b: tga, pos: 0 };
    let (_, hdr) = read_header(&mut s);
    let mut tile_width = i32::from(hdr.image_width) / TILE_PIXEL_EXTENT;
    let mut tile_height = i32::from(hdr.image_height) / TILE_PIXEL_EXTENT;
    if i32::from(hdr.image_height) == TILE_PIXEL_EXTENT / 2 {
        tile_height = 1;
    }
    if i32::from(hdr.image_width) == TILE_PIXEL_EXTENT / 2 {
        tile_width = 1;
    }
    if tile_width < num_rows && tile_height < num_rows {
        return false;
    }
    let compressed = hdr.image_type & 0x08 != 0;
    let bytes_per_pixel = (i32::from(hdr.pixel_depth) + 7) / 8;
    if !(3..=4).contains(&bytes_per_pixel) {
        return false;
    }
    let n = (num_rows * num_rows) as usize;
    if tiles.len() < n {
        return false; // past m_sourceTiles: undefined upstream
    }
    for t in tiles.iter_mut().take(n) {
        if t.is_none() {
            *t = Some(TileData::default());
        }
    }
    let mut buf = [0u8; 4];
    let mut repeat_count = 0i32;
    let mut running = false;
    let bpp = bytes_per_pixel as usize;
    let ext = TILE_PIXEL_EXTENT;
    for row in 0..num_rows * ext {
        for column in 0..i32::from(hdr.image_width) {
            let (r, g, b, a);
            if row < i32::from(hdr.image_height) {
                if compressed && repeat_count == 0 {
                    let mut flag = [0u8; 1];
                    s.read(&mut flag);
                    repeat_count = i32::from(flag[0] & 0x7f) + 1;
                    if flag[0] & 0x80 != 0 {
                        running = true;
                        s.read(&mut buf[..bpp]);
                    } else {
                        running = false;
                    }
                }
                if compressed {
                    repeat_count -= 1;
                }
                if !running {
                    s.read(&mut buf[..bpp]);
                }
                r = buf[2];
                g = buf[1];
                b = buf[0];
                a = if bpp == 4 { buf[3] } else { 255 };
            } else {
                (r, g, b, a) = (0, 0, 0, 0);
            }
            if column >= num_rows * ext {
                continue;
            }
            let tile_ndx = (column / ext + num_rows * (row / ext)) as usize;
            let pixel_ndx = ((column % ext) + ext * (row % ext)) as usize * TILE_BYTES_PER_PIXEL;
            if let Some(t) = &mut tiles[tile_ndx] {
                t.tile_data[pixel_ndx..pixel_ndx + 4].copy_from_slice(&[b, g, r, a]);
            }
        }
    }
    true
}

/// `WorldHeightMap::readTexClass(texClass, tileData)` after the file is open: the class's tiles, the file's tile count
/// clipped to the class's and rounded down to a square, read from the class's first tile on. Nothing is read when the
/// file holds fewer tiles than the class.
pub fn read_tex_class(class: &TxTextureClass, tga: &[u8], tiles: &mut [Option<TileData>]) {
    let (mut num_tiles, _) = count_tiles(tga, false);
    if num_tiles >= class.num_tiles {
        num_tiles = class.num_tiles;
        let mut width = 10;
        while width >= 1 {
            if num_tiles >= width * width {
                break;
            }
            width -= 1;
        }
        let Ok(first) = usize::try_from(class.first_tile) else { return };
        if let Some(slots) = tiles.get_mut(first..) {
            read_tiles(tga, slots, width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::terrain_visual::BlendTiles;

    /// A synthetic map: `w * h` samples from `heights`, one texture class of `class_width`² tiles named "Grass", the
    /// given blend tiles (from index 1) and cliff infos, as version 7 data.
    fn map(w: i32, h: i32, heights: Vec<u8>, tiles: Vec<i16>, blends: &[(usize, BlendTileInfo)], class_width: i32) -> LogicHeightMap {
        let n = (w * h) as usize;
        let mut m = LogicHeightMap::new(w, h, 0, vec![ICoord2D { x: w, y: h }], heights).unwrap();
        let mut blend_ndx = vec![0i16; n];
        let mut rest = Vec::new();
        rest.extend_from_slice(&0i32.to_le_bytes()); // numEdgeTiles
        rest.extend_from_slice(&0i32.to_le_bytes()); // numEdgeTextureClasses
        for (k, (cell, t)) in blends.iter().enumerate() {
            blend_ndx[*cell] = (k + 1) as i16;
            rest.extend_from_slice(&t.blend_ndx.to_le_bytes());
            rest.extend_from_slice(&[t.horiz, t.vert, t.right_diagonal, t.left_diagonal, t.inverted, t.long_diagonal]);
            rest.extend_from_slice(&t.custom_blend_edge_class.to_le_bytes());
            rest.extend_from_slice(&FLAG_VAL.to_le_bytes());
        }
        m.set_blend_tiles(Some(BlendTiles {
            version: 7,
            blend_tile_ndxes: blend_ndx,
            extra_blend_tile_ndxes: vec![0; n],
            cliff_info_ndxes: vec![0; n],
            num_bitmap_tiles: class_width * class_width,
            num_blended_tiles: blends.len() as i32 + 1,
            num_cliff_info: 1,
            rest,
            tile_ndxes: tiles,
            cell_cliff_state: vec![0; ((w + 7) / 8 * h) as usize],
            flip_state_width: (w + 7) / 8,
            texture_classes: vec![TextureClass { first_tile: 0, num_tiles: class_width * class_width, width: class_width, name: b"Grass".to_vec() }],
        }));
        m
    }

    /// A 24-bit raw Targa of `n` x `n` tiles, pixel (column, row) = (column, row, 7).
    fn tga(n: i32) -> Vec<u8> {
        let side = n * TILE_PIXEL_EXTENT;
        let mut b = vec![0u8; 18];
        b[2] = 2;
        b[12..14].copy_from_slice(&(side as i16).to_le_bytes());
        b[14..16].copy_from_slice(&(side as i16).to_le_bytes());
        b[16] = 24;
        for row in 0..side {
            for col in 0..side {
                b.extend_from_slice(&[7, row as u8, col as u8]);
            }
        }
        b
    }

    /// Prediction: a flat 3x3 map at height 16 gives 4 cells, 16 vertices, the first cell at (0,0), (10,0), (10,10),
    /// (0,10), all at z = 16 * 0.625 = 10 with normals straight up; no blend, so no flip and alpha 0.
    #[test]
    fn a_flat_map_gives_upward_vertices() {
        let m = map(3, 3, vec![16; 9], vec![0; 9], &[], 1);
        let t = TerrainHeightMap::from_logic(&m, 1, false).unwrap();
        let v = t.update_vb();
        assert_eq!(v.len(), 16);
        let xy: Vec<(f32, f32)> = v[..4].iter().map(|v| (v.x, v.y)).collect();
        assert_eq!(xy, vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        for p in &v {
            assert_eq!(p.z, 10.0);
            assert_eq!(p.normal, [0.0, 0.0, 1.0]);
            assert_eq!(p.alpha, 0);
        }
        // the border moves the map by -border * 10
        let mut m2 = LogicHeightMap::new(3, 3, 1, vec![], vec![16; 9]).unwrap();
        m2.set_blend_tiles(m.blend_tiles().cloned());
        let t2 = TerrainHeightMap::from_logic(&m2, 1, false).unwrap();
        assert_eq!((t2.update_vb()[0].x, t2.update_vb()[0].y), (-10.0, -10.0));
    }

    /// Prediction: a slope rising 8 a sample in x: an interior vertex's central difference is 16 samples (10 units) over
    /// 20, l2r = (20, 0, 10), so its normal is (-10, 0, 20) normalised, (-0.447, 0, 0.894); the last column's right neighbour wraps to column 1 (upstream's getXWithOrigin), so its top-right
    /// normal's x difference is h(1) - h(2) = -8, normal (+0.243, 0, 0.970).
    #[test]
    fn slope_normals_and_the_wrapped_last_column() {
        let w = 4;
        let heights: Vec<u8> = (0..16).map(|k| (k % w) as u8 * 8).collect();
        let m = map(w, 4, heights, vec![0; 16], &[], 1);
        let t = TerrainHeightMap::from_logic(&m, 1, false).unwrap();
        let v = t.update_vb();
        // cell (1, 1), corner 0 at sample (1, 1): neighbours 0 and 2 -> 16
        let c = &v[(w as usize - 1 + 1) * 4];
        assert!((c.normal[0] + 0.4472136).abs() < 1e-6 && c.normal[1].abs() < 1e-6 && (c.normal[2] - 0.8944272).abs() < 1e-6, "{:?}", c.normal);
        // cell (2, 1), corner 1 at sample (3, 1): up1 = getXWithOrigin(3) + 1 = 0 + 1
        let c = &v[(w as usize - 1 + 2) * 4 + 1];
        assert!(c.normal[0] > 0.0, "wrapped neighbour gives a normal leaning +x: {:?}", c.normal);
        assert!((c.normal[0] - 0.24253563).abs() < 1e-6, "{:?}", c.normal);
    }

    /// Prediction: tile placement of one 2x2 class: at grid (0, 0), origin (4, 4); tiles 0 and 1 in the lower row of the
    /// class (y = 4 + 64), tiles 2 and 3 in the upper row (y = 4); height 4 + 128 + 4 = 136, so the atlas is 256 high.
    /// The base UVs of a cell on tile 0's quarter 0 (`tileNdx` 0): u 4/2048 .. 36/2048, v (68+32)/256 .. (68+64)/256.
    #[test]
    fn the_atlas_places_a_class_and_the_uvs_follow() {
        let m = map(3, 3, vec![0; 9], vec![0, 1, 0, 2, 3, 2, 0, 1, 0], &[], 2);
        let mut t = TerrainHeightMap::from_logic(&m, 1, false).unwrap();
        let mut types = TerrainTypeCollection::new();
        types.new_terrain(b"Grass");
        // the terrain's texture is empty, so the path is "Art/Terrain/"; the stand-in opens anything
        assert_eq!(t.read_tex_classes(&types, &mut |_p| Some(tga(2))), 1);
        assert!(t.has_tiles());
        let atlas = t.build_terrain_texture();
        assert_eq!((atlas.width, atlas.height), (2048, 256));
        let loc: Vec<(i32, i32)> = (0..4).map(|i| t.source_tiles[i].as_ref().unwrap().tile_location_in_texture).map(|p| (p.x, p.y)).collect();
        assert_eq!(loc, vec![(4, 68), (68, 68), (4, 4), (68, 4)]);
        let (u, v, s) = t.get_uv_data(0, 0, false).unwrap();
        assert!(!s);
        assert_eq!(u, [4.0 / 2048.0, 36.0 / 2048.0, 36.0 / 2048.0, 4.0 / 2048.0]);
        assert_eq!(v, [132.0 / 256.0, 132.0 / 256.0, 100.0 / 256.0, 100.0 / 256.0]);
        // the atlas pixel at the class's top-left is the file's row 127 (tile row 63 of tile 2) column 0, 5-bit
        let p = atlas.pixels[4 * 2048 + 4];
        assert_eq!(p, 0x8000 | ((0u16 >> 3) << 10) | ((127u16 >> 3) << 5) | (7 >> 3));
        // the wrapped border: column 0 of the border (x = 0) is the class's column 124
        assert_eq!(atlas.pixels[10 * 2048], atlas.pixels[10 * 2048 + 4 + 124]);
    }

    /// Prediction: without `BlendTileData`, `heights_only` gives the same positions and normals as the full map, and
    /// zero UVs.
    #[test]
    fn heights_only_matches_the_positions() {
        let heights: Vec<u8> = (0..16).map(|k| (k * 13 % 50) as u8).collect();
        let m = map(4, 4, heights.clone(), vec![0; 16], &[], 1);
        let full = TerrainHeightMap::from_logic(&m, 1, false).unwrap().update_vb();
        let bare = TerrainHeightMap::heights_only(&LogicHeightMap::new(4, 4, 0, vec![], heights).unwrap()).update_vb();
        assert_eq!(full.len(), bare.len());
        for (a, b) in full.iter().zip(&bare) {
            assert_eq!((a.x, a.y, a.z, a.normal), (b.x, b.y, b.z, b.normal));
            assert_eq!((b.u1, b.v1, b.u2, b.v2), (0.0, 0.0, 0.0, 0.0));
        }
    }

    /// Prediction: a horizontal, uninverted blend gives alphas 0, 255, 255, 0 and no flip; an uninverted right diagonal
    /// gives alpha 255 at corner 2 and flips the cell, so its vertices start at corner 1.
    #[test]
    fn blends_set_alphas_and_flip_the_diagonal() {
        let horiz = BlendTileInfo { blend_ndx: 4, horiz: 1, custom_blend_edge_class: -1, ..Default::default() };
        let diag = BlendTileInfo { blend_ndx: 4, right_diagonal: 1, custom_blend_edge_class: -1, ..Default::default() };
        let m = map(3, 3, vec![0; 9], vec![0; 9], &[(0, horiz), (1, diag)], 1);
        let t = TerrainHeightMap::from_logic(&m, 1, false).unwrap();
        let (_, _, a, f) = t.get_alpha_uv_data(0, 0, false);
        assert_eq!((a, f), ([0, 255, 255, 0], false));
        let (_, _, a, f) = t.get_alpha_uv_data(1, 0, false);
        assert_eq!((a, f), ([0, 0, 255, 0], true));
        let v = t.update_vb();
        assert_eq!((v[4].x, v[4].y, v[4].alpha), (20.0, 0.0, 0)); // corner 1 of cell (1, 0) first
        assert_eq!((v[5].x, v[5].y, v[5].alpha), (20.0, 10.0, 255));
    }

    /// Prediction: a blend tile whose flag word is not FLAG_VAL is a corrupt file; an index past the table is patched to 0.
    #[test]
    fn a_bad_flag_is_corrupt_and_bad_indices_are_patched() {
        let t = BlendTileInfo { blend_ndx: 4, horiz: 1, custom_blend_edge_class: -1, ..Default::default() };
        let mut m = map(3, 3, vec![0; 9], vec![0; 9], &[(0, t)], 1);
        let mut b = m.blend_tiles().cloned().unwrap();
        b.blend_tile_ndxes[2] = 9;
        b.cliff_info_ndxes[3] = -1;
        m.set_blend_tiles(Some(b.clone()));
        let tm = TerrainHeightMap::from_logic(&m, 1, false).unwrap();
        assert_eq!((tm.blend_tile_ndxes[0], tm.blend_tile_ndxes[2], tm.cliff_info_ndxes[3]), (1, 0, 0));
        let k = b.rest.len() - 4;
        b.rest[k] ^= 1;
        m.set_blend_tiles(Some(b));
        assert_eq!(TerrainHeightMap::from_logic(&m, 1, false).unwrap_err(), TerrainMapError::Corrupt);
    }

    /// Prediction: countTiles gives 4 for a 128x128 24-bit file, 1 and half for 32x32 when asked (0 when not), 0 for
    /// 16-bit; readTiles' RLE path
    /// gives the same tiles as the raw path.
    #[test]
    fn count_and_read_tiles() {
        assert_eq!(count_tiles(&tga(2), false), (4, false));
        let mut half = tga(1)[..18].to_vec();
        half[12] = 32;
        half[14] = 32;
        assert_eq!(count_tiles(&half, true), (1, true));
        assert_eq!(count_tiles(&half, false), (0, false));
        let mut b16 = tga(1);
        b16[16] = 16;
        assert_eq!(count_tiles(&b16, true), (0, false));
        // run-length: each pixel as its own raw packet (flag 0)
        let raw = tga(1);
        let mut rle = raw[..18].to_vec();
        rle[2] = 0xA;
        for px in raw[18..].chunks(3) {
            rle.push(0);
            rle.extend_from_slice(px);
        }
        let mut a = vec![None; 1];
        let mut b = vec![None; 1];
        assert!(read_tiles(&raw, &mut a, 1));
        assert!(read_tiles(&rle, &mut b, 1));
        assert_eq!(a, b);
        let t = a[0].as_ref().unwrap();
        assert_eq!(&t.tile_data[(5 + 64 * 3) * 4..(5 + 64 * 3) * 4 + 4], &[7, 3, 5, 255]);
    }
}

// --- GitHub #35: the radar's terrain colour (`W3DRadar::buildTerrainTexture`'s `getTerrainColorAt`) ---

impl TileData {
    /// `TileData::doMip(pHiRes, hiRow, pLoRes)`: each 2x2 block of the `hi_row`-wide BGRA image averaged, `(a + b + c + d
    /// + 2) / 4` per byte, into a `hi_row / 2`-wide one.
    pub fn do_mip(hi: &[u8], hi_row: i32) -> Vec<u8> {
        let bpp = TILE_BYTES_PER_PIXEL as i32;
        let lo_row = hi_row / 2;
        let mut lo = vec![0u8; (lo_row * lo_row * bpp) as usize];
        let mut i = 0;
        while i < hi_row {
            let mut j = 0;
            while j < hi_row {
                let ndx0 = (j * hi_row + i) * bpp;
                let lo_ndx0 = ((j / 2) * (hi_row / 2) + (i / 2)) * bpp;
                for p in 0..bpp {
                    let (ndx, lo_ndx) = (ndx0 + p, lo_ndx0 + p);
                    let at = |k: i32| i32::from(hi[k as usize]);
                    let pxl = (at(ndx) + at(ndx + bpp) + at(ndx + bpp * hi_row) + at(ndx + bpp * hi_row + bpp) + 2) / 4;
                    lo[lo_ndx as usize] = pxl as u8;
                }
                j += 2;
            }
            i += 2;
        }
        lo
    }

    /// `updateMips()` then `getRGBDataForWidth(1)`: the tile mipped down to one pixel (`m_tileDataMip1`), BGRA. Upstream
    /// keeps every level; only the last is read by the radar, so the chain is made when asked.
    pub fn rgb_data_for_width_1(&self) -> [u8; 4] {
        let mut level = self.tile_data.clone();
        let mut row = TILE_PIXEL_EXTENT;
        while row > 1 {
            level = TileData::do_mip(&level, row);
            row /= 2;
        }
        [level[0], level[1], level[2], level[3]]
    }
}

/// What `WorldHeightMap::getTerrainColorAt` reads (GitHub #35): the samples' tile indices and each source tile mipped to
/// one pixel ([`TileData::rgb_data_for_width_1`], made once here; upstream made them when the tile was read).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerrainColors {
    pub width: i32,
    pub height: i32,
    pub border_size: i32,
    pub data_size: i32,
    pub tile_ndxes: Vec<i16>,
    /// `getSourceTile(i)->getRGBDataForWidth(1)` (BGRA), `None` for a missing tile.
    pub mips: Vec<Option<[u8; 4]>>,
}

impl TerrainColors {
    /// `WorldHeightMap::getTerrainColorAt(x, y, &color)`: the one-pixel colour of the tile under the world point (the
    /// sample clamped into the map, border included); black with no tile there.
    pub fn get_terrain_color_at(&self, x: f32, y: f32) -> crate::common::base_type::RGBColor {
        use crate::common::base_type::{real_to_int_floor, RGBColor};
        let mut x_index = real_to_int_floor(x / MAP_XY_FACTOR);
        let mut y_index = real_to_int_floor(y / MAP_XY_FACTOR);
        x_index += self.border_size;
        y_index += self.border_size;
        let mut c = RGBColor { red: 0.0, green: 0.0, blue: 0.0 };
        if x_index < 0 {
            x_index = 0;
        }
        if y_index < 0 {
            y_index = 0;
        }
        if x_index >= self.width {
            x_index = self.width - 1;
        }
        if y_index >= self.height {
            y_index = self.height - 1;
        }
        let ndx = (y_index * self.width) + x_index;
        if ndx < 0 || ndx >= self.data_size {
            return c;
        }
        let Some(&tile_ndx) = self.tile_ndxes.get(ndx as usize) else { return c };
        // "We pack 4 grids into a tile."
        let tile_ndx = i32::from(tile_ndx) >> 2;
        if let Some(Some(p)) = usize::try_from(tile_ndx).ok().and_then(|n| self.mips.get(n)) {
            // "Data is in microsoft bgra format." (`pData[2]/255.0`: a double, stored as a float)
            c.red = (f64::from(p[2]) / 255.0) as f32;
            c.green = (f64::from(p[1]) / 255.0) as f32;
            c.blue = (f64::from(p[0]) / 255.0) as f32;
        }
        c
    }
}

impl TerrainHeightMap {
    /// The colours `getTerrainColorAt` reads ([`TerrainColors`]).
    pub fn terrain_colors(&self) -> TerrainColors {
        TerrainColors {
            width: self.width,
            height: self.height,
            border_size: self.border_size,
            data_size: self.data_size,
            tile_ndxes: self.tile_ndxes.clone(),
            mips: self.source_tiles.iter().map(|t| t.as_ref().map(TileData::rgb_data_for_width_1)).collect(),
        }
    }
}

#[cfg(test)]
mod radar_color_tests {
    use super::*;

    /// #35: the tiles' one-pixel colours and `getTerrainColorAt`. Predicted before the run: a tile of one BGRA colour mips
    /// to that colour; a tile whose left half is 0 and right half 255 (each byte) mips to 128 (the halves stay apart down to
    /// 2 x 1, then `(0 + 255 + 0 + 255 + 2) / 4`); on a 4 x 4 map with a border of 1,
    /// the world point (5, 5) is sample (1, 1), whose tile index 4 is tile 1 (`>> 2`), and its colour reads as red 0.8
    /// (204/255), green 0.4, blue 0.2; a point off the map clamps to the corner sample; a missing tile is black.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn tiles_mip_to_one_pixel_and_colour_the_terrain() {
        let solid = TileData { tile_data: [51u8, 102, 204, 255].repeat((TILE_PIXEL_EXTENT * TILE_PIXEL_EXTENT) as usize), ..Default::default() };
        assert_eq!(solid.rgb_data_for_width_1(), [51, 102, 204, 255]);
        let mut half = TileData::default();
        for y in 0..TILE_PIXEL_EXTENT {
            for x in TILE_PIXEL_EXTENT / 2..TILE_PIXEL_EXTENT {
                let i = ((y * TILE_PIXEL_EXTENT + x) * 4) as usize;
                half.tile_data[i..i + 4].copy_from_slice(&[255; 4]);
            }
        }
        assert_eq!(half.rgb_data_for_width_1(), [128; 4]);
        let c = TerrainColors { width: 4, height: 4, border_size: 1, data_size: 16, tile_ndxes: vec![4; 16], mips: vec![None, Some(solid.rgb_data_for_width_1())] };
        let got = c.get_terrain_color_at(5.0, 5.0);
        assert_eq!((got.red, got.green, got.blue), ((204.0f64 / 255.0) as f32, (102.0f64 / 255.0) as f32, (51.0f64 / 255.0) as f32));
        assert_eq!(c.get_terrain_color_at(-500.0, 9999.0), got);
        let none = TerrainColors { mips: vec![None; 2], ..c };
        assert_eq!(none.get_terrain_color_at(5.0, 5.0), crate::common::base_type::RGBColor::default());
    }
}
