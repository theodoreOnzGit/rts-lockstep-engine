//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/HeightMap.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/HeightMap.h
//!
//! Sweep 2 (#213): `HeightMapRenderObjClass` members absent from the port: `freeIndexVertexBuffers`, `freeMapResources`'
//! override, `doTheDynamicLight`, `updateVBForLight`, `renderTerrainPass`. Vertex buffers are plain vectors of
//! [`TerrainVertex`] (the caller uploads them); the dynamic lights are [`DynamicLightInfo`] (the `W3DDynamicLight` members the
//! lighting reads, `crate::drafts::terrain::w3d_dynamic_light`).
//!
//! Excluded (listed in the #213 comment): `updateVBForLightOptimized` (HeightMap.cpp:724) is reached only through
//! `updateVBForLight`'s `#if (OPTIMIZED_HEIGHTMAP_LIGHTING)`, and the macro is spelled `no_OPTIMIZED_HEIGHTMAP_LIGHTING`
//! (HeightMap.cpp:105), so `#if (OPTIMIZED_HEIGHTMAP_LIGHTING)` is `#if (0)`: dead code in the retail build.
//! `HALF_RES_MESH` is `const Bool false` (HeightMap.cpp:108), so its branches are kept as upstream's `if`s with the value
//! `false`; `USE_NORMALS` is not defined (the full `doTheDynamicLight` body is the compiled one); `PRE_TRANSFORM_VERTEX` is not.

use rts_lockstep_engine::logic::terrain_visual::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use rts_lockstep_engine::ww::vector3::Vector3;

/// `VERTEX_BUFFER_TILE_LENGTH`: tiles of side length 32 (grid of 33x33 vertices).
pub const VERTEX_BUFFER_TILE_LENGTH: i32 = 32;
/// `HALF_RES_MESH`.
pub const HALF_RES_MESH: bool = false;

/// `VERTEX_FORMAT` as the lighting touches it (`VertexFormatXYZDUV2`): position and packed ARGB diffuse.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerrainVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub diffuse: u32,
}

/// `LightClass::LightType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightType {
    Point,
    Directional,
    Spot,
}

/// The members of a `W3DDynamicLight` that the terrain lighting reads.
#[derive(Clone, Copy, Debug)]
pub struct DynamicLightInfo {
    pub enabled: bool,
    pub light_type: LightType,
    pub position: Vector3,
    /// `Get_Far_Attenuation_Range(midRange, range)`.
    pub mid_range: f64,
    pub range: f64,
    /// `Get_Spot_Direction`.
    pub spot_direction: Vector3,
    pub diffuse: Vector3,
    pub ambient: Vector3,
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
    pub prev_min_x: i32,
    pub prev_max_x: i32,
    pub prev_min_y: i32,
    pub prev_max_y: i32,
}

/// `HeightMapRenderObjClass::doTheDynamicLight` (HeightMap.cpp:175): the lit diffuse of one vertex. Starts from the static
/// colour of `vb_mirror` (the unlit copy), adds each enabled light's diffuse (scaled by `normal . -lightDirection`, with a
/// distance fall-off for point and spot lights) and ambient (scaled by the fall-off), clamps each channel to [0, 1], and
/// keeps the static alpha. Returns the packed value (the caller stores it in `vb->diffuse`).
pub fn do_the_dynamic_light(vb_mirror: &TerrainVertex, normal: &Vector3, lights: &[&DynamicLightInfo]) -> u32 {
    let diffuse = vb_mirror.diffuse as i32;
    let oo255 = 1.0f32 / 255.0;
    let mut shade_r = ((diffuse >> 16) & 0x00FF) as f32 * oo255;
    let mut shade_g = ((diffuse >> 8) & 0x00FF) as f32 * oo255;
    let mut shade_b = (diffuse & 0x00FF) as f32 * oo255;
    let alpha = (diffuse >> 24) & 0x00FF;

    for light in lights {
        if !light.enabled {
            continue; // he is turned off.
        }
        let mut light_direction = Vector3 { x: vb_mirror.x, y: vb_mirror.y, z: vb_mirror.z };
        let mut factor = 1.0f32;
        match light.light_type {
            LightType::Point | LightType::Spot => {
                light_direction.x -= light.position.x;
                light_direction.y -= light.position.y;
                light_direction.z -= light.position.z;
                let (mid_range, range) = (light.mid_range, light.range);
                let dist = light_direction.length();
                if dist as f64 >= range {
                    continue;
                }
                if mid_range < 0.1 {
                    continue;
                }
                factor = 1.0 - ((dist as f64 - mid_range) / (range - mid_range)) as f32;
                factor = factor.clamp(0.0, 1.0);
                light_direction.x /= dist;
                light_direction.y /= dist;
                light_direction.z /= dist;
            }
            LightType::Directional => {
                light_direction = light.spot_direction;
                factor = 1.0;
            }
        }
        let light_ray = Vector3 { x: -light_direction.x, y: -light_direction.y, z: -light_direction.z };
        let mut shade = Vector3::dot_product(&light_ray, normal);
        shade *= factor;
        if shade > 1.0 {
            shade = 1.0;
        }
        if shade < 0.0 {
            shade = 0.0;
        }
        shade_r += shade * light.diffuse.x;
        shade_g += shade * light.diffuse.y;
        shade_b += shade * light.diffuse.z;
        shade_r += factor * light.ambient.x;
        shade_g += factor * light.ambient.y;
        shade_b += factor * light.ambient.z;
    }
    if shade_r > 1.0 {
        shade_r = 1.0;
    }
    if shade_r < 0.0 {
        shade_r = 0.0;
    }
    if shade_g > 1.0 {
        shade_g = 1.0;
    }
    if shade_g < 0.0 {
        shade_g = 0.0;
    }
    if shade_b > 1.0 {
        shade_b = 1.0;
    }
    if shade_b < 0.0 {
        shade_b = 0.0;
    }
    shade_r *= 255.0;
    shade_g *= 255.0;
    shade_b *= 255.0;
    // WWMath::Float_To_Int_Chop: truncation
    ((shade_b as i32) | ((shade_g as i32) << 8) | ((shade_r as i32) << 16) | (alpha << 24)) as u32
}

/// The state `updateVBForLight` uses: the draw window and the map's display heights.
pub struct LightUpdateInput<'a> {
    pub x: i32,
    pub y: i32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub draw_org_x: i32,
    pub draw_org_y: i32,
    pub x_extent: i32,
    pub y_extent: i32,
    pub border_size: i32,
    /// `m_map->getDisplayHeight(x, y)`.
    pub display_height: &'a dyn Fn(i32, i32) -> i32,
}

impl<'a> LightUpdateInput<'a> {
    /// `HeightMapRenderObjClass::getXWithOrigin` (HeightMap.cpp:241): the data column for a mesh column, with the sliding
    /// origin applied (wraps around the `m_x - 1` columns; clamped).
    pub fn get_x_with_origin(&self, x: i32) -> i32 {
        let mut x = x - self.origin_x;
        if x < 0 {
            x += self.x - 1;
        }
        if x >= self.x - 1 {
            x -= self.x - 1;
        }
        if x < 0 {
            x = 0;
        }
        if x >= self.x - 1 {
            x = self.x - 1;
        }
        x
    }

    /// `HeightMapRenderObjClass::getYWithOrigin` (HeightMap.cpp:259).
    pub fn get_y_with_origin(&self, y: i32) -> i32 {
        let mut y = y - self.origin_y;
        if y < 0 {
            y += self.y - 1;
        }
        if y >= self.y - 1 {
            y -= self.y - 1;
        }
        if y < 0 {
            y = 0;
        }
        if y >= self.y - 1 {
            y = self.y - 1;
        }
        y
    }
}

/// `HeightMapRenderObjClass::updateVBForLight` (HeightMap.cpp:572), the non-optimized path: relights the rectangle
/// `[x0, x1) x [y0, y1)` of one vertex buffer tile for the dynamic lights that touch it (current or previous bounds).
/// `vb` is the tile's vertex array (relit in place), `data` the unlit mirror, both `VERTEX_BUFFER_TILE_LENGTH * 4` vertices per
/// row (4 vertices per cell, one per corner: upper left, upper right, lower right, lower left). Returns 0, or -1 when there
/// is no vertex buffer/map (`have_buffers_and_map` false).
pub fn update_vb_for_light(
    input: &LightUpdateInput,
    have_buffers_and_map: bool,
    vb: &mut [TerrainVertex],
    data: &[TerrainVertex],
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    origin_x: i32,
    origin_y: i32,
    lights: &[&DynamicLightInfo],
) -> i32 {
    if !have_buffers_and_map {
        return -1;
    }
    let verts_per_row = VERTEX_BUFFER_TILE_LENGTH * 4; // vertices per row of VB
    let dh = |x: i32, y: i32| (input.display_height)(x, y);
    for j in y0..y1 {
        if HALF_RES_MESH && (j & 1) != 0 {
            continue;
        }
        let y_coord = input.get_y_with_origin(j) + input.draw_org_y - input.border_size;
        let mut intersect = false;
        for l in lights {
            if l.min_y <= y_coord + 1 && l.max_y >= y_coord {
                intersect = true;
            }
            if l.prev_min_y <= y_coord + 1 && l.prev_max_y >= y_coord {
                intersect = true;
            }
        }
        if !intersect {
            continue;
        }
        let mut vn0 = input.get_y_with_origin(j) - 1;
        if vn0 < -input.draw_org_y {
            vn0 = -input.draw_org_y;
        }
        let mut vp1 = input.get_y_with_origin(j + 1) + 1;
        if vp1 >= input.y_extent - input.draw_org_y {
            vp1 = input.y_extent - input.draw_org_y - 1;
        }
        for i in x0..x1 {
            if HALF_RES_MESH && (i & 1) != 0 {
                continue;
            }
            let x_coord = input.get_x_with_origin(i) + input.draw_org_x - input.border_size;
            let mut intersect = false;
            for l in lights {
                if l.min_x <= x_coord + 1 && l.max_x >= x_coord && l.min_y <= y_coord + 1 && l.max_y >= y_coord {
                    intersect = true;
                }
                if l.prev_min_x <= x_coord + 1 && l.prev_max_x >= x_coord && l.prev_min_y <= y_coord + 1 && l.prev_max_y >= y_coord {
                    intersect = true;
                }
            }
            if !intersect {
                continue;
            }
            let mut offset = (j - origin_y) * verts_per_row + 4 * (i - origin_x);
            if HALF_RES_MESH {
                offset = (j - origin_y) * verts_per_row / 4 + 2 * (i - origin_x);
            }
            let mut un0 = input.get_x_with_origin(i) - 1;
            if un0 < -input.draw_org_x {
                un0 = -input.draw_org_x;
            }
            let mut up1 = input.get_x_with_origin(i + 1) + 1;
            if up1 >= input.x_extent - input.draw_org_x {
                up1 = input.x_extent - input.draw_org_x - 1;
            }
            let xo = input.get_x_with_origin(i);
            let yo = input.get_y_with_origin(j);
            let h = MAP_HEIGHT_SCALE;
            let two_xy = 2.0 * MAP_XY_FACTOR;
            // the four corners: (l2r z, n2f z)
            let corners: [(f32, f32); 4] = [
                (h * (dh(xo + 1, yo) - dh(un0, yo)) as f32, h * (dh(xo, yo + 1) - dh(xo, vn0)) as f32),
                (h * (dh(up1, yo) - dh(xo, yo)) as f32, h * (dh(xo + 1, yo + 1) - dh(xo + 1, vn0)) as f32),
                (h * (dh(up1, yo + 1) - dh(xo, yo + 1)) as f32, h * (dh(xo + 1, vp1) - dh(xo + 1, yo)) as f32),
                (h * (dh(xo + 1, yo + 1) - dh(un0, yo + 1)) as f32, h * (dh(xo, vp1) - dh(xo, yo)) as f32),
            ];
            for (k, (lz, nz)) in corners.iter().enumerate() {
                let l2r = Vector3 { x: two_xy, y: 0.0, z: *lz };
                let n2f = Vector3 { x: 0.0, y: two_xy, z: *nz };
                let normal_at_texel = Vector3::normalized_cross_product(&l2r, &n2f);
                let idx = offset as usize + k;
                let lit = do_the_dynamic_light(&data[idx], &normal_at_texel, lights);
                vb[idx].diffuse = lit;
            }
        }
    }
    0 // success.
}

/// What `HeightMapRenderObjClass::freeIndexVertexBuffers` leaves (HeightMap.cpp:137).
#[derive(Clone, Debug, Default)]
pub struct HeightMapBuffers {
    /// `m_indexBuffer`.
    pub index_buffer: Option<Vec<u16>>,
    /// `m_vertexBufferTiles[]` and `m_vertexBufferBackup[]`.
    pub vertex_buffer_tiles: Option<Vec<Vec<TerrainVertex>>>,
    pub vertex_buffer_backup: Option<Vec<Vec<TerrainVertex>>>,
    pub num_vertex_buffer_tiles: i32,
}

impl HeightMapBuffers {
    /// `HeightMapRenderObjClass::freeIndexVertexBuffers` (HeightMap.cpp:137): releases the index buffer, every vertex buffer
    /// tile and its backup, and resets the tile count.
    pub fn free_index_vertex_buffers(&mut self) {
        self.index_buffer = None;
        self.vertex_buffer_tiles = None;
        self.vertex_buffer_backup = None;
        self.num_vertex_buffer_tiles = 0;
    }

    /// `HeightMapRenderObjClass::freeMapResources` (HeightMap.cpp:166): the base class's (see
    /// `sweep_device::base_height_map`), then the index and vertex buffers; returns 0.
    pub fn free_map_resources(&mut self, base_free_map_resources: &mut impl FnMut()) -> i32 {
        base_free_map_resources();
        self.free_index_vertex_buffers();
        0
    }
}

/// One `Draw_Triangles(0, numPolys, 0, numVertex)` of `renderTerrainPass`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainTileDraw {
    /// The tile index `j * m_numVBTilesX + i` whose vertex buffer is set.
    pub tile: i32,
    pub num_polys: i32,
    pub num_vertex: i32,
}

/// `HeightMapRenderObjClass::renderTerrainPass` (HeightMap.cpp:2200): one draw per vertex buffer tile (row by row) with
/// the shared index buffer and an identity world transform. Each tile has `32 * 32 * 2` triangles over `64 * 64` vertices
/// (a quarter of each when `HALF_RES_MESH`). No draw when the object is hidden (`Is_Hidden()`). The
/// `PRE_TRANSFORM_VERTEX` path is not compiled.
pub fn render_terrain_pass(num_vb_tiles_x: i32, num_vb_tiles_y: i32, is_hidden: bool) -> Vec<TerrainTileDraw> {
    let mut draws = Vec::new();
    for j in 0..num_vb_tiles_y {
        for i in 0..num_vb_tiles_x {
            let mut num_polys = VERTEX_BUFFER_TILE_LENGTH * VERTEX_BUFFER_TILE_LENGTH * 2;
            let mut num_vertex = (VERTEX_BUFFER_TILE_LENGTH * 2) * (VERTEX_BUFFER_TILE_LENGTH * 2);
            if HALF_RES_MESH {
                num_polys /= 4;
                num_vertex /= 4;
            }
            if !is_hidden {
                draws.push(TerrainTileDraw { tile: j * num_vb_tiles_x + i, num_polys, num_vertex });
            }
        }
    }
    draws
}
