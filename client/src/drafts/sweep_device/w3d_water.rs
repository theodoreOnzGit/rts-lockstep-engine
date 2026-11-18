//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Water/W3DWater.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DWater.h
//!   (`setupFlatWaterShader` is called from `renderWaterMesh` (line 2391) and `drawTrapezoidWater`; those callers are in the first-pass code above.)
//!
//! Sweep 2 (#213). The water settings (`WaterSettings[]`, `TheWaterTransparency`) are the verified
//! `rts_lockstep_engine::client::water` types. Drafted here: `loadSetting`, `updateMapOverrides`,
//! `replaceSkyboxTexture`, the water grid (`setGridHeightClamps`, `addVelocity`, `changeGridHeight`,
//! `setGridChangeAttenuationFactors`, `getGridTransform`, `worldToGridSpace`), `getClippedWaterPlane`, the geometry
//! builders (`generateVertexBuffer`, `generateIndexBuffer`, `renderWaterMesh`'s vertex fill, `renderSky`,
//! `renderSkyBody`, `drawTrapezoidWater`, `renderWater`'s polygon walk) as functions that return the vertices, indices
//! and draw counts instead of calling `DX8Wrapper`, and `initBumpMap`'s pixel transform.
//!
//! Second pass (#213, remaining 5): `setupJbaWaterShader`, `setupFlatWaterShader`, `drawSea`, `renderMirror` and
//! `drawRiverWater` are drafted at the bottom of this file. They are tied to D3D, so each takes the device state it reads
//! (view/projection matrices, whether a shroud or a pixel shader exists) as arguments and returns a plain record of what
//! it would have set and drew; the device calls themselves are comments (`// upstream: <call> - device-only, not
//! translated`). The second `renderSkyBody` (line 3368, inside `#if 0`) stays excluded. `DO_WATER_SIMULATION` and
//! `USE_MESH_NORMALS` are not defined; `SCROLL_UV` is not either
//! (checked at W3DWater.cpp:2352..2365: the `#else` arm, plain UVs, is the one compiled - but the scrolled `v1Offset` is
//! still computed upstream and unused; it is omitted here).

use crate::drafts::device::w3d_poly::ClipPolyClass;
use crate::drafts::terrain::w3d_common::AABoxClass;
use rts_lockstep_engine::client::water::WaterSetting;
use rts_lockstep_engine::common::base_type::{real_to_int, ICoord3D, RGBColor};
use rts_lockstep_engine::logic::terrain_visual::MAP_XY_FACTOR;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::plane::PlaneClass;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::wwmath::fast_sin;

pub const SKYPLANE_SIZE: f32 = 384.0 * MAP_XY_FACTOR;
pub const SKYPLANE_HEIGHT: f32 = 30.0;
pub const SKYBODY_SIZE: f32 = 45.0;
/// The active `SKYBODY_X` / `SKYBODY_Y` (W3DWater.cpp:88-89; the 150/550 pair at 84-85 is the `#if 0`-ed alternative).
pub const SKYBODY_X: f32 = 120.0;
pub const SKYBODY_Y: f32 = 75.0;
pub const SKYBODY_HEIGHT: f32 = SKYPLANE_HEIGHT;
pub const PATCH_SIZE: i32 = 15;
pub const PATCH_WIDTH: i32 = PATCH_SIZE - 1;
pub const PATCH_UV_TILES: i32 = 42;
pub const PATCH_UV_SCALE: f32 = PATCH_UV_TILES as f32 / PATCH_WIDTH as f32;
pub const BUMP_SIZE: f32 = 50.0;
pub const FEATHER_THICKNESS: f32 = 4.0;
/// `INVALID_WATER_HEIGHT`: water height guaranteed to be below all terrain.
pub const INVALID_WATER_HEIGHT: f32 = 0.0;
/// `WaterRenderObjClass::IN_MOTION`.
pub const IN_MOTION: u8 = 0x01;
/// The number of times of day with a water setting (`TIME_OF_DAY_COUNT`).
pub const TIME_OF_DAY_COUNT: usize = 5;

/// `struct WaterMeshData`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterMeshData {
    pub height: f32,
    pub velocity: f32,
    pub status: u8,
    pub preferred_height: u8,
}

/// `WaterRenderObjClass::Setting` as `loadSetting` fills it (the texture handles are names the caller loads).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Setting {
    pub sky_texture_name: Vec<u8>,
    pub water_texture_name: Vec<u8>,
    pub sky_texels_per_unit: f32,
    pub water_repeat_count: i32,
    pub u_scroll_per_ms: f32,
    pub v_scroll_per_ms: f32,
    pub vertex00_diffuse: u32,
    pub vertex01_diffuse: u32,
    pub vertex10_diffuse: u32,
    pub vertex11_diffuse: u32,
    pub water_diffuse: u32,
    pub transparent_water_diffuse: u32,
}

/// `VertexFormatXYZNDUV2` (dx8fvf.h).
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

/// `SEA_PATCH_VERTEX`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SeaPatchVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub c: u32,
    pub tu: f32,
    pub tv: f32,
}

/// `MaterMeshVertexFormat` (`DX8_FVF_XYZDUV2`, `USE_MESH_NORMALS` undefined).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MaterMeshVertexFormat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub diffuse: u32,
    pub u1: f32,
    pub v1: f32,
    pub u2: f32,
    pub v2: f32,
}

/// What `drawTrapezoidWater` builds for one quad: the vertices and indices for `Draw_Triangles(0, rectangleCount*2, 0,
/// (rectangleCount+1)*2)`. (Upstream sizes the vertex buffer `(rectangleCount+1)*2` but fills `uCount*vCount`; kept.)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrapezoidWaterDraw {
    pub vertices: Vec<VertexFormatXYZNDUV2>,
    pub indices: Vec<u16>,
    pub rectangle_count: i32,
}

/// One water area trigger (`PolygonTrigger` with `isWaterArea()`), as `renderWater` reads it.
#[derive(Clone, Debug, Default)]
pub struct WaterAreaTrigger {
    pub points: Vec<ICoord3D>,
    pub is_river: bool,
}

/// What `renderWater` asks `drawRiverWater` / `drawTrapezoidWater` to draw, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum WaterDrawCall {
    /// `drawRiverWater(pTrig)` for the trigger at this index.
    River(usize),
    /// `drawTrapezoidWater(points)`.
    Trapezoid([Vector3; 4]),
}

/// The members of `WaterRenderObjClass` these functions use.
#[derive(Clone, Debug)]
pub struct WaterRenderObjClass {
    pub level: f32,
    pub dx: f32,
    pub dy: f32,
    pub tod: usize,
    pub settings: [Setting; TIME_OF_DAY_COUNT],
    pub mesh_data: Vec<WaterMeshData>,
    pub grid_cells_x: i32,
    pub grid_cells_y: i32,
    pub grid_cell_size: f32,
    pub grid_origin: (f32, f32),
    pub grid_direction_x: (f32, f32),
    pub grid_direction_y: (f32, f32),
    pub grid_change_att0: f32,
    pub grid_change_att1: f32,
    pub grid_change_att2: f32,
    pub grid_change_max_range: f32,
    pub min_grid_height: f32,
    pub max_grid_height: f32,
    pub mesh_in_motion: bool,
    pub do_water_grid: bool,
    pub disable_river: bool,
    /// `m_drawingRiver` (set by `drawRiverWater`, read by `Render`).
    pub drawing_river: bool,
    pub river_v_origin: f32,
    pub river_texture_name: Option<Vec<u8>>,
    pub u_offset: f32,
    pub v_offset: f32,
    pub last_update_time: i32,
    /// `m_transform` (translation and rotation as `Set_Transform` leaves them): row-major 3x4.
    pub transform: [[f32; 4]; 3],
    pub num_vertices: i32,
    pub num_indices: i32,
    pub vertex_buffer_d3d_offset: u32,
    /// `m_skyBox` texture names after `replaceSkyboxTexture`: (old, new) pairs applied to the prototype.
    pub skybox_replacements: Vec<(Vec<u8>, Vec<u8>)>,
}

impl WaterRenderObjClass {
    /// `Matrix3D(1)` identity as the 3x4 rows used here.
    pub fn identity_transform() -> [[f32; 4]; 3] {
        [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]
    }

    /// `WaterRenderObjClass::worldToGridSpace` (W3DWater.h:272): grid coordinates of a world point; false when outside
    /// `[0, cells - 1]` on either axis (the out values are still set).
    pub fn world_to_grid_space(&self, world_x: f32, world_y: f32) -> (bool, f32, f32) {
        let oo_grid_cell_size = 1.0 / self.grid_cell_size;
        let dx = world_x - self.grid_origin.0;
        let dy = world_y - self.grid_origin.1;
        let grid_x = oo_grid_cell_size * (dx * self.grid_direction_x.0 + dy * self.grid_direction_x.1);
        let grid_y = oo_grid_cell_size * (dx * self.grid_direction_y.0 + dy * self.grid_direction_y.1);
        if grid_x < 0.0 {
            return (false, grid_x, grid_y);
        }
        if grid_x > (self.grid_cells_x - 1) as f32 {
            return (false, grid_x, grid_y);
        }
        if grid_y < 0.0 {
            return (false, grid_x, grid_y);
        }
        if grid_y > (self.grid_cells_y - 1) as f32 {
            return (false, grid_x, grid_y);
        }
        (true, grid_x, grid_y)
    }

    fn mesh_index(&self, x: i32, y: i32) -> usize {
        ((y + 1) * (self.grid_cells_x + 1 + 2) + x + 1) as usize
    }

    /// `WaterRenderObjClass::loadSetting` (W3DWater.cpp:1363): copies the GDF settings for one time of day into a
    /// [`Setting`]. `water_texture_width` is `waterTexture->Get_Level_Description(desc, 0).Width` (the texture is loaded by
    /// name by the caller), and `skyTexelsPerUnit` is divided by it.
    pub fn load_setting(ws: &WaterSetting, water_texture_width: u32) -> Setting {
        let rgb = |c: &rts_lockstep_engine::common::base_type::RGBAColorInt| -> u32 { ((c.red as u32) << 16) | ((c.green as u32) << 8) | (c.blue as u32) };
        let rgba = |c: &rts_lockstep_engine::common::base_type::RGBAColorInt| -> u32 {
            ((c.alpha as u32) << 24) | ((c.red as u32) << 16) | ((c.green as u32) << 8) | (c.blue as u32)
        };
        Setting {
            sky_texture_name: ws.sky_texture_file.clone(),
            water_texture_name: ws.water_texture_file.clone(),
            sky_texels_per_unit: ws.sky_texels_per_unit / water_texture_width as f32,
            water_repeat_count: ws.water_repeat_count,
            u_scroll_per_ms: ws.u_scroll_per_ms,
            v_scroll_per_ms: ws.v_scroll_per_ms,
            vertex00_diffuse: rgb(&ws.vertex00_diffuse),
            vertex01_diffuse: rgb(&ws.vertex01_diffuse),
            vertex10_diffuse: rgb(&ws.vertex10_diffuse),
            vertex11_diffuse: rgb(&ws.vertex11_diffuse),
            water_diffuse: rgba(&ws.water_diffuse_color),
            transparent_water_diffuse: rgba(&ws.transparent_water_diffuse),
        }
    }

    /// `WaterRenderObjClass::updateMapOverrides` (W3DWater.cpp:1134): returns the texture name to load when the map's
    /// `WaterTransparency` standing-water texture differs (case-insensitively) from the loaded river texture. The loaded
    /// texture is replaced by the caller (`Get_Texture`); a missing river texture does nothing.
    pub fn update_map_overrides(&mut self, standing_water_texture: &[u8]) -> Option<Vec<u8>> {
        if let Some(cur) = &self.river_texture_name {
            if !cur.eq_ignore_ascii_case(standing_water_texture) {
                self.river_texture_name = Some(standing_water_texture.to_vec());
                return self.river_texture_name.clone();
            }
        }
        None
    }

    /// `WaterRenderObjClass::replaceSkyboxTexture` (W3DWater.cpp:1325): `replacePrototypeTexture(m_skyBox, old, new)`
    /// then, for a mesh skybox, every material texture is set to clamp addressing in U and V. Both are recorded for the
    /// asset manager (`sweep_device::w3d_asset_manager::replace_prototype_texture`).
    pub fn replace_skybox_texture(&mut self, old_tex_name: &[u8], new_tex_name: &[u8]) {
        self.skybox_replacements.push((old_tex_name.to_vec(), new_tex_name.to_vec()));
    }

    /// `WaterRenderObjClass::setGridHeightClamps` (W3DWater.cpp:2470).
    pub fn set_grid_height_clamps(&mut self, minz: f32, maxz: f32) {
        self.min_grid_height = minz;
        self.max_grid_height = maxz;
    }

    /// `WaterRenderObjClass::addVelocity` (W3DWater.cpp:2476): kicks the grid points within the change range of a world
    /// position; the velocity is added, the preferred height set and the point marked in motion.
    pub fn add_velocity(&mut self, world_x: f32, world_y: f32, z_velocity: f32, preferred_height: f32) {
        if self.do_water_grid {
            self.disable_river = true;
            let (ok, gx, gy) = self.world_to_grid_space(world_x, world_y);
            if ok {
                let mut min_x = (gx - self.grid_change_max_range).floor();
                if min_x < 0.0 {
                    min_x = 0.0; // clamp extent to fall within box
                }
                let mut max_x = (gx + self.grid_change_max_range).ceil();
                if max_x > self.grid_cells_x as f32 {
                    max_x = self.grid_cells_x as f32;
                }
                let mut min_y = (gy - self.grid_change_max_range).floor();
                if min_y < 0.0 {
                    min_y = 0.0;
                }
                let mut max_y = (gy + self.grid_change_max_range).ceil();
                if max_y > self.grid_cells_y as f32 {
                    max_y = self.grid_cells_y as f32;
                }
                // for (y=minY; y<=maxY; y++): Real converted to Int on assignment (truncation)
                let mut y = min_y as i32;
                while y as f32 <= max_y {
                    let mut x = min_x as i32;
                    while x as f32 <= max_x {
                        let idx = self.mesh_index(x, y);
                        let p = &mut self.mesh_data[idx];
                        p.preferred_height = preferred_height as u8; // UnsignedByte member
                        p.velocity += z_velocity;
                        p.status |= IN_MOTION;
                        x += 1;
                    }
                    y += 1;
                }
                self.mesh_in_motion = true;
            }
        } // end if, water type is 3
    }

    /// `WaterRenderObjClass::changeGridHeight` (W3DWater.cpp:2542): raises or lowers the grid around a world position,
    /// attenuated by `1 / (att0 + att1 d + att2 d^2)` and clamped to the height clamps.
    pub fn change_grid_height(&mut self, wx: f32, wy: f32, delta: f32) {
        let (ok, gx, gy) = self.world_to_grid_space(wx, wy);
        if !ok {
            return;
        }
        // find extents of influence
        let mut min_x = (gx - self.grid_change_max_range).floor();
        if min_x < 0.0 {
            min_x = 0.0;
        }
        let mut max_x = (gx + self.grid_change_max_range).ceil();
        if max_x > self.grid_cells_x as f32 {
            max_x = self.grid_cells_x as f32;
        }
        let mut min_y = (gy - self.grid_change_max_range).floor();
        if min_y < 0.0 {
            min_y = 0.0;
        }
        let mut max_y = (gy + self.grid_change_max_range).ceil();
        if max_y > self.grid_cells_y as f32 {
            max_y = self.grid_cells_y as f32;
        }
        let mut y = min_y as i32;
        while y as f32 <= max_y {
            let mut x = min_x as i32;
            while x as f32 <= max_x {
                let idx = self.mesh_index(x, y);
                let old = self.mesh_data[idx].height;
                let mut distance = (gx - x as f32) * (gx - x as f32) + (gy - y as f32) * (gy - y as f32);
                distance = (distance as f64).sqrt() as f32; // sqrt(Real) is the double overload
                let mut new_data = old + 1.0 / (self.grid_change_att0 + self.grid_change_att1 * distance + distance * distance * self.grid_change_att2) * delta;
                if new_data < self.min_grid_height {
                    new_data = self.min_grid_height;
                }
                if new_data > self.max_grid_height {
                    new_data = self.max_grid_height;
                }
                self.mesh_data[idx].height = new_data;
                x += 1;
            }
            y += 1;
        }
    }

    /// `WaterRenderObjClass::setGridChangeAttenuationFactors` (W3DWater.cpp:2586): the range is converted to grid space.
    pub fn set_grid_change_attenuation_factors(&mut self, a: f32, b: f32, c: f32, range: f32) {
        self.grid_change_att0 = a;
        self.grid_change_att1 = b;
        self.grid_change_att2 = c;
        self.grid_change_max_range = range / self.grid_cell_size; // convert range to grid space
    }

    /// `WaterRenderObjClass::getGridTransform` (W3DWater.cpp:2622): `*transform = Get_Transform()`; None when the caller
    /// passes NULL upstream.
    pub fn get_grid_transform(&self) -> [[f32; 4]; 3] {
        self.transform
    }

    /// `WaterRenderObjClass::getClippedWaterPlane` (W3DWater.cpp:1750): clips the water rectangle `(0,0)..(dx,dy)` at the
    /// water level against the six frustum planes; the bounding box of the remaining polygon, or None when fewer than
    /// 3 vertices are left (not visible).
    pub fn get_clipped_water_plane(&self, frustum_planes: &[PlaneClass; 6]) -> Option<AABoxClass> {
        let mut clipped0 = ClipPolyClass::default();
        let mut clipped1 = ClipPolyClass::default();
        clipped0.reset();
        clipped0.add_vertex(&Vector3 { x: 0.0, y: 0.0, z: self.level });
        clipped0.add_vertex(&Vector3 { x: 0.0, y: self.dy, z: self.level });
        clipped0.add_vertex(&Vector3 { x: self.dx, y: self.dy, z: self.level });
        clipped0.add_vertex(&Vector3 { x: self.dx, y: 0.0, z: self.level });
        clipped0.clip(&frustum_planes[0], &mut clipped1);
        clipped1.clip(&frustum_planes[1], &mut clipped0);
        clipped0.clip(&frustum_planes[2], &mut clipped1);
        clipped1.clip(&frustum_planes[3], &mut clipped0);
        clipped0.clip(&frustum_planes[4], &mut clipped1);
        clipped1.clip(&frustum_planes[5], &mut clipped0);
        let final_vcount = clipped0.verts.len();
        if final_vcount >= 3 {
            // box->Init(&Verts[0], count): the AABox of the points
            let mut min = clipped0.verts[0];
            let mut max = clipped0.verts[0];
            for v in &clipped0.verts {
                min.x = min.x.min(v.x);
                min.y = min.y.min(v.y);
                min.z = min.z.min(v.z);
                max.x = max.x.max(v.x);
                max.y = max.y.max(v.y);
                max.z = max.z.max(v.z);
            }
            return Some(AABoxClass {
                center: Vector3 { x: (max.x + min.x) * 0.5, y: (max.y + min.y) * 0.5, z: (max.z + min.z) * 0.5 },
                extent: Vector3 { x: (max.x - min.x) * 0.5, y: (max.y - min.y) * 0.5, z: (max.z - min.z) * 0.5 },
            });
        }
        None // water plane is not visible
    }

    /// `WaterRenderObjClass::generateVertexBuffer` (W3DWater.cpp:625) for the static case (`doStatic`): the
    /// `sizeX * sizeY` patch vertices at the water level, UVs scaled by [`PATCH_UV_SCALE`], vertex colour the current
    /// setting's `transparentWaterDiffuse`. The dynamic case only creates the buffer (nothing to return).
    pub fn generate_vertex_buffer(&mut self, size_x: i32, size_y: i32, do_static: bool) -> Vec<SeaPatchVertex> {
        self.num_vertices = size_x * size_y;
        self.vertex_buffer_d3d_offset = 0;
        if !do_static {
            return Vec::new(); // only create the buffer, other code will fill it.
        }
        let setting = &self.settings[self.tod];
        let mut out = Vec::with_capacity(self.num_vertices as usize);
        for z in 0..size_y {
            for x in 0..size_x {
                out.push(SeaPatchVertex {
                    x: x as f32,
                    y: self.level,
                    z: z as f32,
                    tu: x as f32 * PATCH_UV_SCALE,
                    tv: z as f32 * PATCH_UV_SCALE,
                    c: setting.transparent_water_diffuse, // vertex alpha/color
                });
            }
        }
        out
    }

    /// `WaterRenderObjClass::setTimeOfDay` (W3DWater.cpp:1352) records the time of day; the static patch is regenerated
    /// by the caller for the pixel-shader water type. (Already in the port as a setter; shown for `generate_vertex_buffer`.)
    pub fn set_time_of_day(&mut self, tod: usize) {
        self.tod = tod;
    }

    /// `WaterRenderObjClass::generateIndexBuffer` (W3DWater.cpp:704): the triangle strip over a `sizeX` x `sizeY` grid,
    /// strips joined by degenerate pairs; `m_numIndices = (sizeY-1)*(sizeX*2+2) - 2`. Upstream allocates
    /// `numIndices + 2` words and fills `numIndices` of them.
    pub fn generate_index_buffer(&mut self, size_x: i32, size_y: i32) -> Vec<u16> {
        self.num_indices = (size_y - 1) * (size_x * 2 + 2) - 2;
        let mut indices = vec![0u16; (self.num_indices + 2) as usize];
        let (mut i, mut j, mut k) = (0i32, 0i32, 0i32);
        while i < self.num_indices {
            while k < size_x * (j + 1) {
                indices[i as usize] = (k + size_x) as u16;
                indices[(i + 1) as usize] = k as u16;
                k += 1;
                i += 2;
            }
            if i < self.num_indices {
                // check if there is at least 1 more strip to go
                indices[i as usize] = (k - 1) as u16;
                indices[(i + 1) as usize] = (k + size_x) as u16;
                i += 2;
            }
            j += 1;
        }
        indices
    }

    /// `WaterRenderObjClass::renderWaterMesh` (W3DWater.cpp:2242): the vertex fill of the water grid, row by row from the
    /// grid's inner samples (a one sample border is skipped: 2 per row end). The D3D lock, state set-up and strip draw
    /// are device-only; the caller draws `m_numIndices - 2` triangles from `mx * my` vertices. Empty when the grid is
    /// disabled.
    pub fn render_water_mesh_vertices(&mut self) -> Vec<MaterMeshVertexFormat> {
        if !self.do_water_grid {
            return Vec::new(); // the water grid is disabled.
        }
        self.vertex_buffer_d3d_offset = 0xffff;
        let setting = self.settings[self.tod].clone();
        let mx = self.grid_cells_x + 1;
        let my = self.grid_cells_y + 1;
        let cell_size_x = self.grid_cell_size;
        let cell_size_y = self.grid_cell_size;
        let u_scale = setting.water_repeat_count as f32 / 128.0 * cell_size_x / 10.0 * 0.2;
        let v_scale = setting.water_repeat_count as f32 / 128.0 * cell_size_y / 10.0 * 0.2;

        let mut diffuse: i32 = (setting.water_diffuse & 0x00ff_ffff) as i32;
        let mut alpha: i32 = ((setting.water_diffuse & 0xff00_0000) >> 24) as i32;
        alpha -= 0x20;
        diffuse |= alpha << 24;

        let origin_scale = self.river_v_origin / v_scale;
        let bump_size_div = cell_size_y / BUMP_SIZE;
        let bump_size_div2 = 0.3 * cell_size_y / BUMP_SIZE;
        let mut out = Vec::with_capacity((mx * my) as usize);
        let mut p = (mx + 2 + 1) as usize; // skip the first border row and its first sample
        for j in 0..my {
            let y = j as f32 * cell_size_y;
            let v2_offset = (j as f32 + origin_scale) * bump_size_div + j as f32 * bump_size_div2;
            for i in 0..mx {
                let x = i as f32 * cell_size_x;
                out.push(MaterMeshVertexFormat {
                    x,
                    y,
                    z: self.mesh_data[p].height,
                    diffuse: diffuse as u32,
                    u1: i as f32 * u_scale,
                    v1: j as f32 * v_scale,
                    u2: i as f32 * cell_size_x / BUMP_SIZE,
                    v2: v2_offset,
                });
                p += 1;
            }
            p += 2; // skip 2 horizontal border samples after each row
        }
        out
    }

    /// `WaterRenderObjClass::renderSky` (W3DWater.cpp:2050): advances the sky texture scroll by the frame time, wrapping
    /// to the fraction, and returns the four sky-plane vertices (UV from the offsets, colours from the setting's corner
    /// diffuse values). `time_now_ms` is `timeGetTime()`.
    pub fn render_sky(&mut self, time_now_ms: i32) -> [VertexFormatXYZNDUV2; 4] {
        let setting = &self.settings[self.tod];
        let time_diff = time_now_ms - self.last_update_time;
        self.last_update_time = time_now_ms;
        self.u_offset += time_diff as f32 * setting.u_scroll_per_ms * setting.sky_texels_per_unit;
        self.v_offset += time_diff as f32 * setting.v_scroll_per_ms * setting.sky_texels_per_unit;
        self.u_offset -= (self.u_offset as i32) as f32;
        self.v_offset -= (self.v_offset as i32) as f32;
        let fu = self.u_offset + (SKYPLANE_SIZE * 2.0) * setting.sky_texels_per_unit;
        let fv = self.v_offset + (SKYPLANE_SIZE * 2.0) * setting.sky_texels_per_unit;
        let mut verts = [VertexFormatXYZNDUV2::default(); 4];
        verts[0] = VertexFormatXYZNDUV2 { x: -SKYPLANE_SIZE, y: SKYPLANE_SIZE, z: SKYPLANE_HEIGHT, u1: self.u_offset, v1: fv, diffuse: setting.vertex01_diffuse, ..Default::default() };
        verts[1] = VertexFormatXYZNDUV2 { x: SKYPLANE_SIZE, y: SKYPLANE_SIZE, z: SKYPLANE_HEIGHT, u1: fu, v1: fv, diffuse: setting.vertex11_diffuse, ..Default::default() };
        verts[2] = VertexFormatXYZNDUV2 { x: SKYPLANE_SIZE, y: -SKYPLANE_SIZE, z: SKYPLANE_HEIGHT, u1: fu, v1: self.v_offset, diffuse: setting.vertex10_diffuse, ..Default::default() };
        verts[3] = VertexFormatXYZNDUV2 { x: -SKYPLANE_SIZE, y: -SKYPLANE_SIZE, z: SKYPLANE_HEIGHT, u1: self.u_offset, v1: self.v_offset, diffuse: setting.vertex00_diffuse, ..Default::default() };
        verts
    }

    /// `WaterRenderObjClass::renderSkyBody` (W3DWater.cpp:2139): the billboard rotation (axis and angle, `Matrix3D::Set(axis,
    /// angle)`) that turns the sky body toward the camera at `camera_pos`, with the body's translation, and its four
    /// vertices. `ALLOW_TEMPORARIES` is not defined (the normalised-cross-product branch).
    pub fn render_sky_body(camera_pos: &Vector3) -> (Vector3, f32, Vector3, [VertexFormatXYZNDUV2; 4]) {
        let p_pos = Vector3 { x: SKYBODY_X, y: SKYBODY_Y, z: SKYBODY_HEIGHT };
        let mut p_view = Vector3 { x: camera_pos.x - p_pos.x, y: camera_pos.y - p_pos.y, z: camera_pos.z - p_pos.z }; // billboard to camera
        p_view.normalize(); // particle view direction
        let world_up = Vector3 { x: 0.0, y: 0.0, z: -1.0 }; // hacked so only works for reflections across xy plane
        let rot_axis = Vector3::normalized_cross_product(&world_up, &p_view);
        let angle = Vector3::dot_product(&world_up, &p_view);
        let angle = (angle as f64).acos() as f32;
        let mut verts = [VertexFormatXYZNDUV2::default(); 4];
        verts[0] = VertexFormatXYZNDUV2 { x: -SKYBODY_SIZE, y: SKYBODY_SIZE, z: 0.0, u2: 0.0, v2: 1.0, diffuse: 0xffff_ffff, ..Default::default() };
        verts[1] = VertexFormatXYZNDUV2 { x: SKYBODY_SIZE, y: SKYBODY_SIZE, z: 0.0, u2: 1.0, v2: 1.0, diffuse: 0xffff_ffff, ..Default::default() };
        verts[2] = VertexFormatXYZNDUV2 { x: SKYBODY_SIZE, y: -SKYBODY_SIZE, z: 0.0, u2: 1.0, v2: 0.0, diffuse: 0xffff_ffff, ..Default::default() };
        verts[3] = VertexFormatXYZNDUV2 { x: -SKYBODY_SIZE, y: -SKYBODY_SIZE, z: 0.0, u2: 0.0, v2: 0.0, diffuse: 0xffff_ffff, ..Default::default() };
        (rot_axis, angle, p_pos, verts)
    }

    /// `WaterRenderObjClass::renderWater` (W3DWater.cpp:2002): walks the water-area triggers; a river is drawn as such,
    /// any other area is cut into quads of `(point0, point k, point k+1, point k+2)` fans, each drawn once, or
    /// `m_featherWater` times with its first point raised by `FEATHER_THICKNESS / m_featherWater` each time.
    pub fn render_water_calls(triggers: &[WaterAreaTrigger], feather_water: i32) -> Vec<WaterDrawCall> {
        let mut calls = Vec::new();
        for (t, trig) in triggers.iter().enumerate() {
            let n = trig.points.len();
            if n > 2 {
                if trig.is_river {
                    calls.push(WaterDrawCall::River(t));
                    continue;
                }
                let mut k = 1;
                while k < n - 1 {
                    let pt3 = trig.points[0];
                    let pt2 = trig.points[k];
                    let pt1 = trig.points[k + 1];
                    let mut pt0 = trig.points[k + 1];
                    if k + 2 < n {
                        pt0 = trig.points[k + 2];
                    }
                    let v = |p: ICoord3D| Vector3 { x: p.x as f32, y: p.y as f32, z: p.z as f32 };
                    let mut points = [v(pt0), v(pt1), v(pt2), v(pt3)];
                    if feather_water != 0 {
                        for _r in 0..feather_water {
                            calls.push(WaterDrawCall::Trapezoid(points));
                            points[0].z += FEATHER_THICKNESS / feather_water as f32;
                        }
                    } else {
                        calls.push(WaterDrawCall::Trapezoid(points));
                    }
                    k += 2;
                }
            }
        }
        calls
    }
}

/// The globals `drawTrapezoidWater` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrapezoidInputs {
    /// `TheWaterTransparency->m_standingWaterColor`.
    pub standing_water_color: RGBColor,
    /// `TheGlobalData->m_terrainAmbient[0]`.
    pub terrain_ambient0: RGBColor,
    /// `TheGlobalData->m_numGlobalLights`, `m_terrainLightPos[i].z`, `m_terrainDiffuse[i]`.
    pub light_pos_z: [f32; 3],
    pub terrain_diffuse: [RGBColor; 3],
    pub num_global_lights: i32,
    /// `TheGlobalData->m_featherWater`.
    pub feather_water: i32,
}

/// `WaterRenderObjClass::drawTrapezoidWater` (W3DWater.cpp:3046): the index and vertex data for one water quad (corners
/// `points[0..4]`), as a `uCount` x `vCount` grid (one cell per `8 * MAP_XY_FACTOR` of edge length, 1..=50), vertices
/// bilinearly placed, lit by the standing-water colour or (when it is white) the terrain lighting times the setting's
/// water diffuse, UVs scrolling with `m_riverVOrigin`. With `featherWater`, vertices wave and take a fixed alpha. The
/// shader set-up, the soft-shoreline passes and the shroud pass are device-only. `river_v_origin` is `m_riverVOrigin`.
pub fn draw_trapezoid_water(water: &WaterRenderObjClass, inputs: &TrapezoidInputs, points: &[Vector3; 4]) -> TrapezoidWaterDraw {
    let sub = |a: &Vector3, b: &Vector3| Vector3 { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z };
    let origin = points[0];
    let mut u_vec1 = points[1];
    let mut v_vec1 = points[3];
    let mut u_vec2 = points[2];
    let mut v_vec2 = points[2];
    u_vec2 = sub(&u_vec2, &v_vec1);
    v_vec2 = sub(&v_vec2, &u_vec1);
    u_vec1 = sub(&u_vec1, &origin);
    v_vec1 = sub(&v_vec1, &origin);

    let mut u_count = ((u_vec1.length() + u_vec2.length()) / (8.0 * MAP_XY_FACTOR)) as i32;
    if u_count < 1 {
        u_count = 1;
    }
    let mut v_count = ((v_vec1.length() + v_vec2.length()) / (8.0 * MAP_XY_FACTOR)) as i32;
    if v_count < 1 {
        v_count = 1;
    }
    if u_count > 50 {
        u_count = 50;
    }
    if v_count > 50 {
        v_count = 50;
    }
    let water_factor = 150.0f32;
    let rectangle_count = u_count * v_count;
    u_count += 1;
    v_count += 1;

    // allocate 2 triangles per side with 3 indices per triangle
    let mut indices: Vec<u16> = Vec::with_capacity(((rectangle_count + 1) * 2 * 3) as usize);
    for j in 0..(v_count - 1) {
        for i in 0..(u_count - 1) {
            // triangle 1
            indices.push((j * u_count + i) as u16);
            indices.push(((j + 1) * u_count + i + 1) as u16);
            indices.push(((j + 1) * u_count + i) as u16);
            // triangle 2
            indices.push((j * u_count + i) as u16);
            indices.push((j * u_count + i + 1) as u16);
            indices.push(((j + 1) * u_count + i + 1) as u16);
        }
    }

    let mut shade_r = inputs.standing_water_color.red;
    let mut shade_g = inputs.standing_water_color.green;
    let mut shade_b = inputs.standing_water_color.blue;
    let setting = &water.settings[water.tod];
    // If the water color is not overridden, use legacy lighting code.
    if shade_r == 1.0 && shade_g == 1.0 && shade_b == 1.0 {
        shade_r = inputs.terrain_ambient0.red;
        shade_g = inputs.terrain_ambient0.green;
        shade_b = inputs.terrain_ambient0.blue;
        for light_index in 0..inputs.num_global_lights.max(0) as usize {
            if -inputs.light_pos_z[light_index] > 0.0 {
                shade_r += -inputs.light_pos_z[light_index] * inputs.terrain_diffuse[light_index].red;
                shade_g += -inputs.light_pos_z[light_index] * inputs.terrain_diffuse[light_index].green;
                shade_b += -inputs.light_pos_z[light_index] * inputs.terrain_diffuse[light_index].blue;
            }
        }
        let water_shade_r = (setting.water_diffuse & 0xff) as f32 / 255.0;
        let water_shade_g = ((setting.water_diffuse >> 8) & 0xff) as f32 / 255.0;
        let water_shade_b = ((setting.water_diffuse >> 16) & 0xff) as f32 / 255.0;
        shade_r = shade_r * water_shade_r * 255.0;
        shade_g = shade_g * water_shade_g * 255.0;
        shade_b = shade_b * water_shade_b * 255.0;
    } else {
        shade_r *= 255.0;
        shade_g *= 255.0;
        shade_b *= 255.0;
        if shade_r == 0.0 && shade_g == 0.0 && shade_b == 0.0 {
            // special case where we disable lighting
            shade_r = 255.0;
            shade_g = 255.0;
            shade_b = 255.0;
        }
    }
    let mut diffuse: i32 = real_to_int(shade_b) | (real_to_int(shade_g) << 8) | (real_to_int(shade_r) << 16);
    diffuse |= (setting.water_diffuse & 0xff00_0000) as i32; // copy alpha/opacity from ini setting

    let combine = |du: f32, dv: f32| -> Vector3 {
        // vertex = origin + uVec1*du + vVec1*dv + (dv*du)*(vVec2 - vVec1)
        let d = sub(&v_vec2, &v_vec1);
        Vector3 {
            x: origin.x + u_vec1.x * du + v_vec1.x * dv + (dv * du) * d.x,
            y: origin.y + u_vec1.y * du + v_vec1.y * dv + (dv * du) * d.y,
            z: origin.z + u_vec1.z * du + v_vec1.z * dv + (dv * du) * d.z,
        }
    };
    let pi = std::f32::consts::PI;
    let mut vertices = Vec::with_capacity((u_count * v_count) as usize);
    if inputs.feather_water != 0 {
        let map_coeff = pi / (4.0 * MAP_XY_FACTOR);
        let amplitude = 0.5f32;
        let mut alpha = 0i32;
        if inputs.feather_water == 5 {
            alpha = 80;
        }
        if inputs.feather_water == 4 {
            alpha = 110;
        }
        if inputs.feather_water == 3 {
            alpha = 140;
        }
        if inputs.feather_water == 2 {
            alpha = 200;
        }
        if inputs.feather_water == 1 {
            alpha = 255;
        }
        let custom_diffuse = (diffuse & 0x00ff_ffff) | (alpha << 24);
        for j in 0..v_count {
            let mut dv = j as f32;
            dv /= (v_count - 1) as f32;
            for i in 0..u_count {
                let mut du = i as f32;
                du /= (u_count - 1) as f32;
                let vertex = combine(du, dv);
                let phase = 25.0 * water.river_v_origin + vertex.x * map_coeff;
                let wave = ((phase as f64).sin() as f32 - 1.0) * amplitude;
                vertices.push(VertexFormatXYZNDUV2 {
                    x: vertex.x,
                    y: vertex.y,
                    z: vertex.z + wave,
                    diffuse: custom_diffuse as u32,
                    u1: (vertex.x / water_factor) + 0.02 * ((11.0 * water.river_v_origin) as f64).cos() as f32 * wave,
                    v1: (vertex.y / water_factor) + 0.02 * ((5.0 * water.river_v_origin) as f64).cos() as f32 * wave,
                    u2: vertex.x / BUMP_SIZE,
                    v2: vertex.y / BUMP_SIZE + 0.3 * vertex.x / BUMP_SIZE,
                    nx: 0.0,
                    ny: 0.0,
                    nz: 1.0,
                });
            }
        }
    } else {
        let const_a = 0.02 * ((11.0 * water.river_v_origin) as f64).cos() as f32;
        let const_b = 0.02 * ((5.0 * water.river_v_origin) as f64).cos() as f32;
        let const_c = 25.0 * water.river_v_origin;
        let oo_water_factor = 1.0 / water_factor;
        let const_d = pi / (4.0 * MAP_XY_FACTOR);
        let const_e = 1.0 / (v_count - 1) as f32;
        let const_f = 1.0 / (u_count - 1) as f32;
        for j in 0..v_count {
            let dv = j as f32 * const_e;
            for i in 0..u_count {
                let du = i as f32 * const_f;
                let vertex = combine(du, dv);
                vertices.push(VertexFormatXYZNDUV2 {
                    x: vertex.x,
                    y: vertex.y,
                    z: vertex.z,
                    diffuse: diffuse as u32,
                    u1: vertex.x * oo_water_factor + const_a * fast_sin(const_c + vertex.x * const_d),
                    v1: vertex.y * oo_water_factor + const_b * fast_sin(const_c + vertex.y * const_d),
                    u2: vertex.x / BUMP_SIZE,
                    v2: (vertex.y + 0.3 * vertex.x) / BUMP_SIZE,
                    nx: 0.0,
                    ny: 0.0,
                    nz: 1.0,
                });
            }
        }
    }
    TrapezoidWaterDraw { vertices, indices, rectangle_count }
}

/// `WaterRenderObjClass::initBumpMap` (W3DWater.cpp:430), `MIPMAP_BUMP_TEXTURE` defined, one mip level: converts a
/// 32 bit source level (`src`, rows `src_pitch` bytes, `width` x `height`) to the `D3DFMT_V8U8` bump map (the format
/// the switch is hard-wired to): 2 bytes per pixel `(iDu, iDv)` from the red channel's differences with the left,
/// right, upper and lower neighbours (edges repeat). Returns the packed level. Upstream reads the byte left of the first
/// pixel and right of the last on a row (neighbouring memory); those reads are taken as the row's own edge value here,
/// which is a difference to be checked in the differential test.
pub fn init_bump_map_level(src: &[u8], src_pitch: usize, width: usize, height: usize) -> Vec<u8> {
    let mut dst = Vec::with_capacity(width * height * 2);
    for y in 0..height {
        let row0 = y * src_pitch;
        let row1 = if y == height - 1 { row0 } else { row0 + src_pitch }; // Don't go past the last line
        let row2 = if y == 0 { row0 } else { row0 - src_pitch }; // Don't go before first line
        for x in 0..width {
            let v00 = 256 - src[row0 + x * 4] as i32; // the current pixel
            let right = if x + 1 < width { row0 + (x + 1) * 4 } else { row0 + x * 4 };
            let left = if x > 0 { row0 + (x - 1) * 4 } else { row0 + x * 4 };
            let v01 = 256 - src[right] as i32; // and the pixel to the right
            let vm1 = 256 - src[left] as i32; // and the pixel to the left
            let v10 = 256 - src[row1 + x * 4] as i32; // and the pixel one line below.
            let v1m = 256 - src[row2 + x * 4] as i32; // and the pixel one line above.
            let mut i_du = vm1 - v01; // The delta-u bump value
            let i_dv = v1m - v10; // The delta-v bump value
            if v00 < vm1 && v00 < v01 {
                // If we are at valley
                i_du = vm1 - v00; // Choose greater of 1st order diffs
                if i_du < v00 - v01 {
                    i_du = v00 - v01;
                }
            }
            dst.push(i_du as u8);
            dst.push(i_dv as u8);
        }
    }
    dst
}

// ---------------------------------------------------------------------------------------------------------------------
// Second pass: the D3D-bound water functions (shader set-up, sea, mirror, river)
// ---------------------------------------------------------------------------------------------------------------------

/// `REFLECTION_FACTOR` (W3DWater.cpp:102).
pub const REFLECTION_FACTOR: f32 = 0.1;
/// `NOISE_REPEAT_FACTOR` (W3DWater.cpp:192).
pub const NOISE_REPEAT_FACTOR: f32 = 1.0 / 16.0;
/// `SEA_BUMP_SCALE` (W3DWater.cpp:100): `m_fBumpScale`.
pub const SEA_BUMP_SCALE: f32 = 0.06;
/// `PATCH_SCALE` (W3DWater.cpp:97).
pub const PATCH_SCALE: f32 = 4.0 * MAP_XY_FACTOR;
/// `HEIGHT_TO_USE` (W3DWater.cpp:2788, local `#define` of `drawRiverWater`).
pub const HEIGHT_TO_USE: f32 = 0.5;
/// `TIME_OF_DAY_NIGHT` (`client::drawable`).
const TIME_OF_DAY_NIGHT: usize = 4;

/// A D3D matrix as `D3DXMATRIX` lays it out: `m[row][column]`, vectors are rows (`v' = v * M`).
pub type D3dMatrix = [[f32; 4]; 4];

fn d3d_identity() -> D3dMatrix {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
}

/// `D3DXMatrixMultiply(&out, &a, &b)`: `out = a * b`.
fn d3d_multiply(a: &D3dMatrix, b: &D3dMatrix) -> D3dMatrix {
    let mut out = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            out[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
        }
    }
    out
}

/// `D3DXMatrixTranspose`.
fn d3d_transpose(a: &D3dMatrix) -> D3dMatrix {
    let mut out = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            out[i][j] = a[j][i];
        }
    }
    out
}

/// `D3DXMatrixInverse(&inv, &det, &m)`: the general 4x4 inverse by cofactors, with the determinant. When the
/// determinant is 0 D3DX returns NULL and leaves `inv` untouched; the identity stands in for that here.
fn d3d_inverse(m: &D3dMatrix) -> (D3dMatrix, f32) {
    let a = |r: usize, c: usize| m[r][c] as f64;
    let mut inv = [[0.0f64; 4]; 4];
    // 2x2 sub-determinants of rows (0,1) and (2,3)
    let s0 = a(0, 0) * a(1, 1) - a(1, 0) * a(0, 1);
    let s1 = a(0, 0) * a(1, 2) - a(1, 0) * a(0, 2);
    let s2 = a(0, 0) * a(1, 3) - a(1, 0) * a(0, 3);
    let s3 = a(0, 1) * a(1, 2) - a(1, 1) * a(0, 2);
    let s4 = a(0, 1) * a(1, 3) - a(1, 1) * a(0, 3);
    let s5 = a(0, 2) * a(1, 3) - a(1, 2) * a(0, 3);
    let c5 = a(2, 2) * a(3, 3) - a(3, 2) * a(2, 3);
    let c4 = a(2, 1) * a(3, 3) - a(3, 1) * a(2, 3);
    let c3 = a(2, 1) * a(3, 2) - a(3, 1) * a(2, 2);
    let c2 = a(2, 0) * a(3, 3) - a(3, 0) * a(2, 3);
    let c1 = a(2, 0) * a(3, 2) - a(3, 0) * a(2, 2);
    let c0 = a(2, 0) * a(3, 1) - a(3, 0) * a(2, 1);
    let det = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0;
    if det == 0.0 {
        return (d3d_identity(), 0.0);
    }
    let id = 1.0 / det;
    inv[0][0] = (a(1, 1) * c5 - a(1, 2) * c4 + a(1, 3) * c3) * id;
    inv[0][1] = (-a(0, 1) * c5 + a(0, 2) * c4 - a(0, 3) * c3) * id;
    inv[0][2] = (a(3, 1) * s5 - a(3, 2) * s4 + a(3, 3) * s3) * id;
    inv[0][3] = (-a(2, 1) * s5 + a(2, 2) * s4 - a(2, 3) * s3) * id;
    inv[1][0] = (-a(1, 0) * c5 + a(1, 2) * c2 - a(1, 3) * c1) * id;
    inv[1][1] = (a(0, 0) * c5 - a(0, 2) * c2 + a(0, 3) * c1) * id;
    inv[1][2] = (-a(3, 0) * s5 + a(3, 2) * s2 - a(3, 3) * s1) * id;
    inv[1][3] = (a(2, 0) * s5 - a(2, 2) * s2 + a(2, 3) * s1) * id;
    inv[2][0] = (a(1, 0) * c4 - a(1, 1) * c2 + a(1, 3) * c0) * id;
    inv[2][1] = (-a(0, 0) * c4 + a(0, 1) * c2 - a(0, 3) * c0) * id;
    inv[2][2] = (a(3, 0) * s4 - a(3, 1) * s2 + a(3, 3) * s0) * id;
    inv[2][3] = (-a(2, 0) * s4 + a(2, 1) * s2 - a(2, 3) * s0) * id;
    inv[3][0] = (-a(1, 0) * c3 + a(1, 1) * c1 - a(1, 2) * c0) * id;
    inv[3][1] = (a(0, 0) * c3 - a(0, 1) * c1 + a(0, 2) * c0) * id;
    inv[3][2] = (-a(3, 0) * s3 + a(3, 1) * s1 - a(3, 2) * s0) * id;
    inv[3][3] = (a(2, 0) * s3 - a(2, 1) * s1 + a(2, 2) * s0) * id;
    let mut out = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            out[i][j] = inv[i][j] as f32;
        }
    }
    (out, det as f32)
}

/// The texture-stage-2 transform both shader set-ups build for the sparkle/noise pass: `inverse(view) *
/// scale(NOISE_REPEAT_FACTOR, NOISE_REPEAT_FACTOR, 1) * translate(m_riverVOrigin, m_riverVOrigin, 0)`
/// (`D3DTS_TEXTURE2`, W3DWater.cpp:239-249 and 3029-3041).
pub fn noise_texture_transform(view: &D3dMatrix, river_v_origin: f32) -> D3dMatrix {
    let (inv, _det) = d3d_inverse(view); // D3DXMatrixInverse(&inv, &det, &curView)
    let mut scale = d3d_identity(); // D3DXMatrixScaling(&scale, NOISE_REPEAT_FACTOR, NOISE_REPEAT_FACTOR, 1)
    scale[0][0] = NOISE_REPEAT_FACTOR;
    scale[1][1] = NOISE_REPEAT_FACTOR;
    scale[2][2] = 1.0;
    let mut dest = d3d_multiply(&inv, &scale);
    let mut trans = d3d_identity(); // D3DXMatrixTranslation(&scale, m_riverVOrigin, m_riverVOrigin, 0)
    trans[3][0] = river_v_origin;
    trans[3][1] = river_v_origin;
    trans[3][2] = 0.0;
    dest = d3d_multiply(&dest, &trans);
    dest
}

/// Which preset shader the water set-ups pick: `ShaderClass::_PresetAlphaShader` or `_PresetAdditiveShader`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterPresetShader {
    Alpha,
    Additive,
}

/// What `setupJbaWaterShader` sets, as the decisions it makes (the D3D calls are device-only).
#[derive(Clone, Debug, PartialEq)]
pub struct JbaWaterShaderSetup {
    /// `Set_Shader(!m_additiveBlend ? _PresetAlphaShader : _PresetAdditiveShader)`.
    pub preset_shader: WaterPresetShader,
    /// `doSparkles && m_riverWaterPixelShader`: stages 1 (sparkles) and 2 (noise) are bound and wrapped.
    pub sparkles: bool,
    /// `D3DTS_TEXTURE2` when `sparkles`.
    pub texture2_transform: Option<D3dMatrix>,
    /// `SetPixelShaderConstant(0, (REFLECTION_FACTOR x3, 1))` and `SetPixelShader(m_riverWaterPixelShader)`, when the
    /// river pixel shader exists.
    pub pixel_shader_constant: Option<[f32; 4]>,
}

/// `WaterRenderObjClass::setupJbaWaterShader` (W3DWater.cpp:197). `additive_blend` is
/// `TheWaterTransparency->m_additiveBlend`, `has_river_water_pixel_shader` is `m_riverWaterPixelShader != 0`, `view` is
/// the current `D3DTS_VIEW` (read with `_Get_DX8_Transform`), `river_v_origin` is `m_riverVOrigin`.
pub fn setup_jba_water_shader(
    additive_blend: bool,
    has_river_water_pixel_shader: bool,
    view: &D3dMatrix,
    river_v_origin: f32,
) -> JbaWaterShaderSetup {
    let preset_shader = if !additive_blend { WaterPresetShader::Alpha } else { WaterPresetShader::Additive };
    // upstream: DX8Wrapper::Set_Shader / Set_Material(PRELIT_DIFFUSE) - device-only, not translated
    // upstream: m_riverTexture Get_Filter().Set_Mag/Min_Filter/Set_Mip_Mapping(FILTER_TYPE_BEST) - device-only, not translated
    // upstream: Apply_Render_State_Changes, stage 0 ALPHAOP = ADD - device-only, not translated
    // upstream: m_riverAlphaEdge->Init() if needed, SetTexture(3, riverAlphaEdge), stage 3 wrap, stage 0/1 TEXCOORDINDEX 0,
    //           stage 3 TEXCOORDINDEX 1 - device-only, not translated

    let do_sparkles = true;
    let mut sparkles = false;
    let mut texture2_transform = None;
    if has_river_water_pixel_shader && do_sparkles {
        sparkles = true;
        // upstream: sparkle texture on stage 1 and noise texture on stage 2 (Init if needed), stage 1/2 WRAP,
        //           stage 2 TEXCOORDINDEX = CAMERASPACEPOSITION, TEXTURETRANSFORMFLAGS = COUNT2 - device-only, not translated
        texture2_transform = Some(noise_texture_transform(view, river_v_origin));
        // upstream: DX8Wrapper::_Set_DX8_Transform(D3DTS_TEXTURE2, destMatrix) - device-only, not translated
    }
    // upstream: stage 0..3 MINFILTER/MAGFILTER = LINEAR - device-only, not translated
    let mut pixel_shader_constant = None;
    if has_river_water_pixel_shader {
        pixel_shader_constant = Some([REFLECTION_FACTOR, REFLECTION_FACTOR, REFLECTION_FACTOR, 1.0]);
        // upstream: SetPixelShader(m_riverWaterPixelShader) - device-only, not translated
    }
    JbaWaterShaderSetup { preset_shader, sparkles, texture2_transform, pixel_shader_constant }
}

/// How `setupFlatWaterShader` handles the shroud when the trapezoid pixel shader exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatWaterShroudBranch {
    /// No `m_trapezoidWaterPixelShader`: stage 3 is left alone.
    NoPixelShader,
    /// `TheTerrainRenderObject->getShroud()`: the shroud texture on stage 0 via `ST_SHROUD_TEXTURE` (stage 3), and the
    /// z function switched from EQUAL to `D3DCMP_LESSEQUAL` (water does not write z).
    Shroud,
    /// No shroud: the white texture (initialised with a 0xffffffff pixel on first use) goes on stage 3, because the
    /// GF4 pixel shader makes random colours with `SetTexture(3, NULL)`.
    WhiteTexture,
}

/// What `setupFlatWaterShader` sets, as the decisions it makes.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatWaterShaderSetup {
    pub preset_shader: WaterPresetShader,
    pub shroud_branch: FlatWaterShroudBranch,
    /// `doSparkles && m_trapezoidWaterPixelShader`.
    pub sparkles: bool,
    /// `D3DTS_TEXTURE2` when `sparkles`.
    pub texture2_transform: Option<D3dMatrix>,
    /// `SetPixelShaderConstant(0, ...)` + `SetPixelShader(m_trapezoidWaterPixelShader)`.
    pub pixel_shader_constant: Option<[f32; 4]>,
}

/// `WaterRenderObjClass::setupFlatWaterShader` (W3DWater.cpp:2948). Arguments as in [`setup_jba_water_shader`], with
/// `has_trapezoid_water_pixel_shader` for `m_trapezoidWaterPixelShader` and `has_shroud` for
/// `TheTerrainRenderObject->getShroud() != NULL`.
pub fn setup_flat_water_shader(
    additive_blend: bool,
    has_trapezoid_water_pixel_shader: bool,
    has_shroud: bool,
    view: &D3dMatrix,
    river_v_origin: f32,
) -> FlatWaterShaderSetup {
    // upstream: DX8Wrapper::Set_Texture(0, m_riverTexture) - device-only, not translated
    let preset_shader = if !additive_blend { WaterPresetShader::Alpha } else { WaterPresetShader::Additive };
    // upstream: Set_Shader, Set_Material(PRELIT_DIFFUSE), m_riverTexture filters BEST, Apply_Render_State_Changes - device-only, not translated

    // Setup shroud to render in same pass as water
    let shroud_branch = if has_trapezoid_water_pixel_shader {
        if has_shroud {
            // upstream: W3DShaderManager::setTexture(0, shroud), setShader(ST_SHROUD_TEXTURE, 3), ZFUNC = LESSEQUAL - device-only, not translated
            FlatWaterShroudBranch::Shroud
        } else {
            // upstream: m_whiteTexture Init + DrawPixel(0,0,0xffffffff), SetTexture(3, white) - device-only, not translated
            FlatWaterShroudBranch::WhiteTexture
        }
    } else {
        FlatWaterShroudBranch::NoPixelShader
    };
    // upstream: stage 0 ALPHAOP = ADD, stage 0/1 TEXCOORDINDEX 0 - device-only, not translated

    let do_sparkles = true;
    let mut sparkles = false;
    let mut texture2_transform = None;
    if has_trapezoid_water_pixel_shader && do_sparkles {
        sparkles = true;
        // upstream: sparkle texture on stage 1, noise on stage 2, wrap modes, stage 2 CAMERASPACEPOSITION/COUNT2 - device-only, not translated
        texture2_transform = Some(noise_texture_transform(view, river_v_origin));
    }
    // upstream: stage 0..2 MINFILTER/MAGFILTER = LINEAR - device-only, not translated
    let mut pixel_shader_constant = None;
    if has_trapezoid_water_pixel_shader {
        pixel_shader_constant = Some([REFLECTION_FACTOR, REFLECTION_FACTOR, REFLECTION_FACTOR, 1.0]);
        // upstream: SetPixelShader(m_trapezoidWaterPixelShader) - device-only, not translated
    }
    FlatWaterShaderSetup { preset_shader, shroud_branch, sparkles, texture2_transform, pixel_shader_constant }
}

/// One patch `drawSea` draws: the matrices it loads and the indexed draw it issues.
#[derive(Clone, Debug, PartialEq)]
pub struct SeaPatchDraw {
    pub patch_x: i32,
    pub patch_y: i32,
    /// `matWorldViewProj`, transposed, loaded into `CV_WORLDVIEWPROJ_0` (register 2, 4 rows).
    pub world_view_proj_transposed: D3dMatrix,
    /// `matTemp = patchMatrix * matWW3D`, the `D3DTS_WORLD` of the shroud pass for this patch.
    pub shroud_world: D3dMatrix,
}

/// What `drawSea` sets and draws.
#[derive(Clone, Debug, PartialEq)]
pub struct SeaDraw {
    /// Bump texture frame bound to stage 0 (`m_pBumpTexture[m_iBumpFrame]`).
    pub bump_frame: i32,
    /// `D3DTS_BUMPENVMAT00` / `11` (`m_fBumpScale`; `01` and `10` are 0), `BUMPENVLSCALE` 1, `BUMPENVLOFFSET` 0.
    pub bump_scale: f32,
    /// The `CV_TEXPROJ_0` (register 6) matrix.
    pub tex_proj: D3dMatrix,
    pub patches: Vec<SeaPatchDraw>,
    /// `DrawIndexedPrimitive(D3DPT_TRIANGLESTRIP, 0, m_numVertices, 0, m_numIndices)` per patch: `(minIndex = 0,
    /// numVertices, startIndex = 0, primCount = m_numIndices)`. Upstream passes the index count as the primitive count
    /// (D3D8 `primCount`); kept as written.
    pub num_vertices: i32,
    pub prim_count: i32,
    /// A second pass over the same patches applies the shroud (`TheTerrainRenderObject->getShroud()`).
    pub shroud_pass: bool,
}

/// `WaterRenderObjClass::drawSea` (W3DWater.cpp:1790). `sea_box` is the result of [`WaterRenderObjClass::get_clipped_water_plane`]
/// (`None` returns at once, "the sea is not visible"); `view` / `proj` are `D3DTS_VIEW` / `D3DTS_PROJECTION` after
/// `Apply_Render_State_Changes`; `bump_frame` is `m_iBumpFrame`; `has_shroud` is `TheTerrainRenderObject->getShroud()`.
/// The patch loops run from `trunc((centre - extent) / (PATCH_WIDTH * PATCH_SCALE))` while
/// `patch * PATCH_WIDTH * PATCH_SCALE < centre + extent`, as upstream, X inside Y.
pub fn draw_sea(
    water: &WaterRenderObjClass,
    sea_box: Option<&AABoxClass>,
    view: &D3dMatrix,
    proj: &D3dMatrix,
    bump_frame: i32,
    has_shroud: bool,
) -> Option<SeaDraw> {
    let sea_box = sea_box?; // the sea is not visible

    // create a transform which will flip the y and z coordinates to fit our system
    let mut mat_ww3d: D3dMatrix = [[0.0; 4]; 4];
    mat_ww3d[0][0] = 1.0; // _11
    mat_ww3d[2][1] = 1.0; // _32
    mat_ww3d[1][2] = 1.0; // _23
    mat_ww3d[3][3] = 1.0; // _44

    // upstream: Set_Transform(D3DTS_WORLD, Transform), Set_Texture(0/1, NULL), Apply_Render_State_Changes - device-only, not translated
    // upstream: stage 0..3 colour/alpha ops, TEXCOORDINDEX, wrap/clamp, SetTexture(0, bump), mip filters,
    //           BUMPENVMAT*, ZWRITEENABLE false - device-only, not translated

    let mut tex_proj: D3dMatrix = [[0.0; 4]; 4];
    tex_proj[0][0] = 0.5;
    tex_proj[0][1] = -0.5;
    tex_proj[0][2] = 0.5;
    tex_proj[0][3] = 0.5;
    tex_proj[1][0] = 0.5;
    tex_proj[1][1] = 0.5;
    tex_proj[1][2] = 0.0;
    tex_proj[1][3] = 0.0;
    tex_proj[2][0] = 0.0;
    tex_proj[2][1] = 0.0;
    tex_proj[2][2] = 0.0;
    tex_proj[2][3] = 1.0;
    tex_proj[3][0] = 0.0;
    tex_proj[3][1] = 0.0;
    tex_proj[3][2] = 0.0;
    tex_proj[3][3] = 1.0;
    // upstream: SetVertexShaderConstant(CV_TEXPROJ_0..., CV_ZERO, CV_ONE), SetVertexShader(m_dwWaveVertexShader),
    //           SetPixelShader(m_dwWavePixelShader), SRCALPHA/INVSRCALPHA blend, ALPHABLENDENABLE, SetTexture(1, reflection),
    //           SetStreamSource, SetIndices - device-only, not translated

    let mut patch_matrix: D3dMatrix = [[0.0; 4]; 4];
    patch_matrix[0][0] = PATCH_SCALE;
    patch_matrix[1][1] = 1.0;
    patch_matrix[2][2] = PATCH_SCALE;
    patch_matrix[3][3] = 1.0;

    let patch_span = PATCH_WIDTH as f32 * PATCH_SCALE; // PATCH_WIDTH*PATCH_SCALE
    let mut patches = Vec::new();
    let mut patch_y = ((sea_box.center.y - sea_box.extent.y) / patch_span) as i32;
    while ((patch_y * PATCH_WIDTH) as f32 * PATCH_SCALE) < (sea_box.center.y + sea_box.extent.y) {
        let mut patch_x = ((sea_box.center.x - sea_box.extent.x) / patch_span) as i32;
        while ((patch_x * PATCH_WIDTH) as f32 * PATCH_SCALE) < (sea_box.center.x + sea_box.extent.x) {
            patch_matrix[3][0] = ((patch_x * PATCH_WIDTH) as f32 * PATCH_SCALE) as f32; // _41
            patch_matrix[3][2] = ((patch_y * PATCH_WIDTH) as f32 * PATCH_SCALE) as f32; // _43
            // convert the default D3D coordinate system into ours
            let mat_temp_world = d3d_multiply(&patch_matrix, &mat_ww3d);
            let mat_temp = d3d_multiply(&mat_temp_world, view);
            let mat_world_view_proj = d3d_multiply(&mat_temp, proj);
            // matrices must be transposed before loading into vertex shader registers
            let world_view_proj_transposed = d3d_transpose(&mat_world_view_proj);
            // upstream: SetVertexShaderConstant(CV_WORLDVIEWPROJ_0, ...), DrawIndexedPrimitive - device-only, not translated
            patches.push(SeaPatchDraw {
                patch_x,
                patch_y,
                world_view_proj_transposed,
                // the shroud pass multiplies the same patchMatrix by matWW3D (patchMatrix is unchanged by the first pass)
                shroud_world: mat_temp_world,
            });
            patch_x += 1;
        }
        patch_y += 1;
    }
    // upstream: ALPHABLENDENABLE false, release textures 0-2, stage resets, ZWRITEENABLE true, WRAP0 0, ops DISABLE,
    //           restore VIEW/PROJECTION, SetPixelShader(0), SetVertexShader(DX8_FVF_XYZDUV1),
    //           Invalidate_Cached_Render_States - device-only, not translated

    if has_shroud {
        // do second pass to apply the shroud on water plane
        // upstream: W3DShaderManager::setTexture(0, shroud), setShader(ST_SHROUD_TEXTURE, 0), SetStreamSource,
        //           SetIndices, per patch _Set_DX8_Transform(D3DTS_WORLD, shroud_world) + DrawIndexedPrimitive,
        //           resetShader - device-only, not translated
    }

    Some(SeaDraw {
        bump_frame,
        bump_scale: SEA_BUMP_SCALE,
        tex_proj,
        patches,
        num_vertices: water.num_vertices,
        prim_count: water.num_indices,
        shroud_pass: has_shroud,
    })
}

/// What `renderMirror` does to the camera for the reflected pass.
#[derive(Clone, Debug)]
pub struct MirrorRenderPlan {
    /// The camera transform to restore afterwards (`OldCameraMatrix`).
    pub old_camera_transform: Matrix3D,
    /// `reflectedTransform`, set on the camera for the render.
    pub reflected_transform: Matrix3D,
    /// `cam->Set_Viewport((0,0),(1,1))`: the reflection fills the whole texture.
    pub viewport_min: (f32, f32),
    pub viewport_max: (f32, f32),
    /// `renderSky()` is always called; `renderSkyBody(&reflectedTransform)` only at `TIME_OF_DAY_NIGHT`.
    pub render_sky_body: bool,
}

/// `WaterRenderObjClass::renderMirror` (W3DWater.cpp:1435): the reflected camera. The camera's right, up and
/// forward vectors and its position are mirrored in the plane `z = m_level` (normal `(0,0,1)`), and the new matrix is
/// built from them. Returns the plan the caller executes (render sky, optionally the sky body, `WW3D::Render` the
/// scene with `ShaderClass::Invert_Backface_Culling(true)`, then restore the camera and viewport).
/// `EXTENDED_STATS` (`m_disableWater`) is not defined.
pub fn render_mirror(water: &WaterRenderObjClass, cam_transform: &Matrix3D) -> MirrorRenderPlan {
    let old_camera_transform = *cam_transform; // OldCameraMatrix
    let water_normal = Vector3 { x: 0.0, y: 0.0, z: 1.0 }; // normal of plane used for reflection
    let water_plane_w = water.level; // Vector4 WaterPlane(WaterNormal.X, .Y, .Z, m_level)

    // FullMatrix = transpose of the 3x4 copied into a 4x4: [0..2] are the camera axes, [3] the position.
    let r = &cam_transform.row;
    let full = [
        Vector3 { x: r[0].x, y: r[1].x, z: r[2].x },
        Vector3 { x: r[0].y, y: r[1].y, z: r[2].y },
        Vector3 { x: r[0].z, y: r[1].z, z: r[2].z },
        Vector3 { x: r[0].w, y: r[1].w, z: r[2].w },
    ];
    let reflect = |v: &Vector3, axis_distance: f32| Vector3 {
        x: v.x - 2.0 * axis_distance * water_normal.x,
        y: v.y - 2.0 * axis_distance * water_normal.y,
        z: v.z - 2.0 * axis_distance * water_normal.z,
    };

    // reflect camera right vector
    let mut axis_distance = Vector3::dot_product(&full[0], &water_normal);
    let r_right = reflect(&full[0], axis_distance);
    // reflect camera up vector
    axis_distance = Vector3::dot_product(&full[1], &water_normal);
    let r_up = reflect(&full[1], axis_distance);
    // reflect camera n vector
    axis_distance = Vector3::dot_product(&full[2], &water_normal);
    let r_n = reflect(&full[2], axis_distance);
    // reflect camera position
    axis_distance = Vector3::dot_product(&full[3], &water_normal); // distance cam to origin
    axis_distance -= water_plane_w; // subtract mirror plane distance to get distance camera to plane
    let r_pos = reflect(&full[3], axis_distance);

    // generate a new camera matrix from reflected vectors
    let reflected_transform = Matrix3D::from_axes(&r_right, &r_up, &r_n, &r_pos);

    // upstream: DX8Wrapper::Set_Render_Target_With_Z(m_pReflectionTexture), WW3D::Begin_Render(false, true, black),
    //           cam->Set_Transform / Set_Viewport / Apply, ShaderClass::Invert_Backface_Culling(true), renderSky(),
    //           renderSkyBody if night, WW3D::Render(m_parentScene, cam), restore camera, Invert_Backface_Culling(false),
    //           WW3D::End_Render, Set_Render_Target(NULL) - device-only, not translated

    MirrorRenderPlan {
        old_camera_transform,
        reflected_transform,
        viewport_min: (0.0, 0.0),
        viewport_max: (1.0, 1.0),
        render_sky_body: water.tod == TIME_OF_DAY_NIGHT,
    }
}

/// `PolygonTrigger` as `drawRiverWater` reads it: its points and `getRiverStart()`.
#[derive(Clone, Debug, Default)]
pub struct RiverTrigger {
    pub points: Vec<ICoord3D>,
    pub river_start: i32,
}

/// What `drawRiverWater` draws for one river polygon.
#[derive(Clone, Debug, PartialEq)]
pub struct RiverWaterDraw {
    pub indices: Vec<u16>,
    pub vertices: Vec<VertexFormatXYZNDUV2>,
    /// `Draw_Triangles(0, rectangleCount*2, 0, (rectangleCount+1)*2)`.
    pub rectangle_count: i32,
    pub shader: JbaWaterShaderSetup,
    /// Additive blend: SRCBLEND is `D3DBLEND_SRCALPHA` during the draw (so the river edge alpha darkens rgb) and back to
    /// `D3DBLEND_ONE` afterwards.
    pub additive_src_alpha: bool,
    /// A second `Draw_Triangles` with the shroud texture (z function LESSEQUAL, cull NONE).
    pub shroud_pass: bool,
}

/// The water diffuse colour `drawTrapezoidWater` and `drawRiverWater` compute alike (W3DWater.cpp:3109-3143 and
/// 2757-2791): the standing-water colour, or, when it is white, the terrain ambient plus the downward light terms
/// times the setting's water diffuse; `0,0,0` disables lighting. Alpha is copied from the setting.
pub fn water_diffuse(water: &WaterRenderObjClass, inputs: &TrapezoidInputs) -> i32 {
    let mut shade_r = inputs.standing_water_color.red;
    let mut shade_g = inputs.standing_water_color.green;
    let mut shade_b = inputs.standing_water_color.blue;
    let setting = &water.settings[water.tod];
    // If the water color is not overridden, use legacy lighting code.
    if shade_r == 1.0 && shade_g == 1.0 && shade_b == 1.0 {
        shade_r = inputs.terrain_ambient0.red;
        shade_g = inputs.terrain_ambient0.green;
        shade_b = inputs.terrain_ambient0.blue;
        // Add in diffuse lighting from each terrain light
        for light_index in 0..inputs.num_global_lights.max(0) as usize {
            if -inputs.light_pos_z[light_index] > 0.0 {
                shade_r += -inputs.light_pos_z[light_index] * inputs.terrain_diffuse[light_index].red;
                shade_g += -inputs.light_pos_z[light_index] * inputs.terrain_diffuse[light_index].green;
                shade_b += -inputs.light_pos_z[light_index] * inputs.terrain_diffuse[light_index].blue;
            }
        }
        // Get water material colors
        let water_shade_r = (setting.water_diffuse & 0xff) as f32 / 255.0;
        let water_shade_g = ((setting.water_diffuse >> 8) & 0xff) as f32 / 255.0;
        let water_shade_b = ((setting.water_diffuse >> 16) & 0xff) as f32 / 255.0;
        shade_r = shade_r * water_shade_r * 255.0;
        shade_g = shade_g * water_shade_g * 255.0;
        shade_b = shade_b * water_shade_b * 255.0;
    } else {
        shade_r *= 255.0;
        shade_g *= 255.0;
        shade_b *= 255.0;
        if shade_r == 0.0 && shade_g == 0.0 && shade_b == 0.0 {
            // special case where we disable lighting
            shade_r = 255.0;
            shade_g = 255.0;
            shade_b = 255.0;
        }
    }
    let mut diffuse: i32 = real_to_int(shade_b) | (real_to_int(shade_g) << 8) | (real_to_int(shade_r) << 16);
    // Keep diffuse from lighting calculations but substitute custom alpha
    diffuse |= (setting.water_diffuse & 0xff00_0000) as i32; // copy alpha/opacity from ini setting
    diffuse
}

/// `WaterRenderObjClass::drawRiverWater` (W3DWater.cpp:2714): a many-sided river polygon as a strip of quads. The two
/// edges of the river are the points walking out from `getRiverStart()` in both directions (`innerNdx` down, `outerNdx`
/// up, both wrapping); each pair gives two vertices with `u1 = HEIGHT_TO_USE / 0`, `u2 = 1 / 0` and a shared
/// `v = -m_riverVOrigin + vScale*i + sin(2 PI vScale i - 3 m_riverVOrigin)/22`, where `vScale = repeatCount /
/// rectangleCount` and `repeatCount = ((totalLen/2) - endLen) / endLen` (`endLen` is the length of the river-start
/// segment). Returns `None` when `m_disableRiver` (before `m_drawingRiver` is set) or when `getRiverStart() >=
/// numPoints-1` (after it is set, as upstream). `view` is `D3DTS_VIEW` for the shader set-up. The `bumpFactor` upstream
/// computes (`endLen / BUMP_SIZE`) is never read and is omitted.
pub fn draw_river_water(
    water: &mut WaterRenderObjClass,
    trig: &RiverTrigger,
    inputs: &TrapezoidInputs,
    additive_blend: bool,
    has_river_water_pixel_shader: bool,
    has_shroud: bool,
    view: &D3dMatrix,
) -> Option<RiverWaterDraw> {
    // upstream: DX8Wrapper::Invalidate_Cached_Render_States() - device-only, not translated
    let num_points = trig.points.len() as i32;
    let mut rectangle_count = num_points / 2;
    rectangle_count -= 1;

    if water.disable_river {
        return None;
    }
    water.drawing_river = true;

    // allocate 2 triangles per side with 3 indices per triangle
    let mut indices: Vec<u16> = Vec::with_capacity(((rectangle_count.max(0) + 1) * 2 * 3) as usize);
    for i in 0..rectangle_count {
        // triangle 1
        indices.push((i * 2) as u16);
        indices.push((i * 2 + 1) as u16);
        indices.push((i * 2 + 3) as u16);
        // triangle 2
        indices.push((i * 2) as u16);
        indices.push((i * 2 + 3) as u16);
        indices.push((i * 2 + 2) as u16);
    }

    let diffuse = water_diffuse(water, inputs);

    let mut inner_ndx = trig.river_start;
    let mut outer_ndx = inner_ndx + 1;

    let mut end_len = 0.0f32;
    let mut total_len = 0.0f32;
    for i in 0..(num_points - 1) {
        let inner_pt = trig.points[i as usize];
        let outer_pt = trig.points[(i + 1) as usize];
        let dx = (inner_pt.x - outer_pt.x) as f32;
        let dy = (inner_pt.y - outer_pt.y) as f32;
        let cur_len = (dx * dx + dy * dy).sqrt();
        total_len += cur_len;
        if i == inner_ndx {
            end_len = cur_len;
        }
    }
    // bumpFactor = endLen / BUMP_SIZE: never read upstream afterwards.

    let length_of_river = (total_len / 2.0) - end_len;
    let repeat_count = length_of_river / end_len;

    let v_scale = repeat_count / rectangle_count as f32;

    if inner_ndx >= num_points - 1 {
        return None;
    }
    // allocate 2 vertices per side
    let mut vertices: Vec<VertexFormatXYZNDUV2> = Vec::with_capacity(((rectangle_count + 1) * 2).max(0) as usize);
    let const_a = 3.0 * water.river_v_origin;
    let pi = std::f32::consts::PI;
    for i in 0..(num_points / 2) {
        let inner_pt = trig.points[outer_ndx as usize];
        let outer_pt = trig.points[inner_ndx as usize];
        outer_ndx += 1;
        inner_ndx -= 1;
        if inner_ndx < 0 {
            inner_ndx = num_points - 1;
        }
        if outer_ndx >= num_points {
            outer_ndx = 0;
        }
        let wobble_const =
            -water.river_v_origin + v_scale * i as f32 + fast_sin(2.0 * pi * (v_scale * i as f32) - const_a) / 22.0;
        vertices.push(VertexFormatXYZNDUV2 {
            x: inner_pt.x as f32,
            y: inner_pt.y as f32,
            z: inner_pt.z as f32,
            diffuse: diffuse as u32,
            v1: wobble_const,
            u1: HEIGHT_TO_USE,
            v2: wobble_const,
            u2: 1.0,
            nx: 0.0,
            ny: 0.0,
            nz: 1.0,
        });
        vertices.push(VertexFormatXYZNDUV2 {
            x: outer_pt.x as f32,
            y: outer_pt.y as f32,
            z: outer_pt.z as f32,
            diffuse: diffuse as u32,
            v1: wobble_const,
            u1: 0.0,
            v2: wobble_const,
            u2: 0.0,
            nx: 0.0,
            ny: 0.0,
            nz: 1.0,
        });
    }

    // upstream: Matrix3D tm(1); Set_Transform(D3DTS_WORLD, tm); Set_Index_Buffer; Set_Vertex_Buffer;
    //           Set_Texture(0, m_riverTexture) - device-only, not translated
    let shader = setup_jba_water_shader(additive_blend, has_river_water_pixel_shader, view, water.river_v_origin);

    // In additive blending we need to use the alpha at the edges of river to darken rgb instead.
    // upstream: if additive: Set_DX8_Render_State(D3DRS_SRCBLEND, SRCALPHA) - device-only, not translated
    // upstream: SetPixelShader(m_riverWaterPixelShader), GetRenderState(CULLMODE), CULLMODE = NONE,
    //           wireframeForDebug (static false) FILLMODE, Draw_Triangles(0, rectangleCount*2, 0, (rectangleCount+1)*2),
    //           SetPixelShader(NULL) - device-only, not translated
    // restore blend mode to what W3D expects: SRCBLEND = ONE (device-only, not translated)
    // do second pass to apply the shroud on water plane
    // upstream: if shroud: setTexture/ST_SHROUD_TEXTURE, CULLMODE NONE, ZFUNC LESSEQUAL, Draw_Triangles, ZFUNC EQUAL,
    //           resetShader; then CULLMODE = saved - device-only, not translated

    Some(RiverWaterDraw {
        indices,
        vertices,
        rectangle_count,
        shader,
        additive_src_alpha: additive_blend,
        shroud_pass: has_shroud,
    })
}
