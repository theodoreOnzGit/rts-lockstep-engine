//! UNTESTED DRAFT (#202, #204, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DSnow.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DSnow.cpp
//!   GeneralsMD/Code/GameEngine/Source/GameClient/Snow.cpp, Include/GameClient/Snow.h (`SnowManager`, the base class:
//!     `init`, `updateIniSettings`, `setVisible`, `reset`, and `WeatherSetting`'s members, which are not in this area and
//!     not in the port; only what `W3DSnowManager` needs is copied here)
//!
//! The particle vertices are `Vec`s the renderer uploads: the point-sprite path fills `point_vertices`
//! (`SNOW_BUFFER_SIZE` positions) and records each `DrawPrimitive(D3DPT_POINTLIST, base, count)` as a [`SnowPointDraw`];
//! the quad path returns the view-space quad vertices of each batch. The D3D vertex buffer lock/unlock and the render
//! states are device-only. `goto flush_particles` is a labelled break.

use super::w3d_common::{AABoxClass, TextureHandle};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::wwmath;

/// `SNOW_BUFFER_SIZE`: size of the vertex buffer holding particles.
pub const SNOW_BUFFER_SIZE: i32 = 4096;
/// `SNOW_BATCH_SIZE`: we render at most this many particles per drawprimitive call.
pub const SNOW_BATCH_SIZE: i32 = 2048;
/// `SnowManager::SNOW_NOISE_X`: dimensions of the table holding the noise function used for initial snow positions.
pub const SNOW_NOISE_X: i32 = 64;
/// `SnowManager::SNOW_NOISE_Y`.
pub const SNOW_NOISE_Y: i32 = 64;
/// `MAXIMUM_CAMERA_DISTANCE`: maximum distance of camera position from world origin.
pub const MAXIMUM_CAMERA_DISTANCE: i32 = 100000;

/// `WeatherSetting` (Snow.h): the INI values snow reads, with `WeatherSetting::WeatherSetting`'s defaults.
#[derive(Clone, Debug)]
pub struct WeatherSetting {
    /// `m_snowTexture`.
    pub snow_texture: Vec<u8>,
    /// `m_snowFrequencyScaleX`: used to adjust snow position.
    pub snow_frequency_scale_x: f32,
    /// `m_snowFrequencyScaleY`.
    pub snow_frequency_scale_y: f32,
    /// `m_snowAmplitude`: used to adjust amount of snow movement (in world units).
    pub snow_amplitude: f32,
    /// `m_snowPointSize`: hardware point-sprite size.
    pub snow_point_size: f32,
    /// `m_snowMaxPointSize`: maximum size (in pixels) of point sprite.
    pub snow_max_point_size: f32,
    /// `m_snowMinPointSize`: minimum size (in pixels) of point sprite.
    pub snow_min_point_size: f32,
    /// `m_snowQuadSize`: quad size when no hardware point sprites (world width/height of quad).
    pub snow_quad_size: f32,
    /// `m_snowBoxDimensions`: dimensions of the box surrounding the camera (world units).
    pub snow_box_dimensions: f32,
    /// `m_snowBoxDensity`: how many emitters are present per world unit.
    pub snow_box_density: f32,
    /// `m_snowVelocity`: speed at which snow falls (world units/sec).
    pub snow_velocity: f32,
    /// `m_usePointSprites`: used to disable hardware point-sprite support.
    pub use_point_sprites: bool,
    /// `m_snowEnabled`: enable/disable snow on the map.
    pub snow_enabled: bool,
}

impl Default for WeatherSetting {
    fn default() -> Self {
        WeatherSetting {
            snow_texture: b"EXSnowFlake.tga".to_vec(),
            snow_frequency_scale_x: 0.0533,
            snow_frequency_scale_y: 0.0275,
            snow_amplitude: 5.0,
            snow_point_size: 1.0,
            snow_quad_size: 0.5,
            snow_box_dimensions: 200.0,
            snow_box_density: 1.0,
            snow_velocity: 4.0,
            use_point_sprites: true,
            snow_enabled: false,
            snow_max_point_size: 64.0,
            snow_min_point_size: 0.0,
        }
    }
}

/// One `DrawPrimitive(D3DPT_POINTLIST, m_dwBase, numberInBatch)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnowPointDraw {
    /// `m_dwBase`: index of the first vertex of the batch in `point_vertices`.
    pub base: i32,
    /// `numberInBatch`.
    pub count: i32,
}

/// `VertexFormatXYZNDUV2` for the quad fallback (dx8fvf.h).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SnowQuadVertex {
    /// Position (view space).
    pub pos: Vector3,
    /// `nx, ny, nz`.
    pub normal: Vector3,
    /// `diffuse`.
    pub diffuse: u32,
    /// `u1`.
    pub u1: f32,
    /// `v1`.
    pub v1: f32,
    /// `u2`.
    pub u2: f32,
    /// `v2`.
    pub v2: f32,
}

/// `MinMaxAABoxClass` (aabox.h): the box `renderSubBox` tests against the frustum.
#[derive(Clone, Copy, Debug, Default)]
pub struct MinMaxAABoxClass {
    /// `MinCorner`.
    pub min_corner: Vector3,
    /// `MaxCorner`.
    pub max_corner: Vector3,
}

/// What a `render` produced.
#[derive(Clone, Debug, Default)]
pub struct SnowRender {
    /// True when the point-sprite path ran (`point_draws` and the manager's `point_vertices`).
    pub point_sprites: bool,
    /// The point list draws, in order.
    pub point_draws: Vec<SnowPointDraw>,
    /// The quad path: one entry per `Draw_Triangles(0, n*2, 0, n*4)`, the batch's view-space vertices (4 per flake).
    pub quad_batches: Vec<Vec<SnowQuadVertex>>,
}

/// `class W3DSnowManager : public SnowManager` (W3DSnow.h, Snow.h).
pub struct W3DSnowManager {
    /// `m_time` (`SnowManager`).
    pub time: f32,
    /// `m_velocity`.
    pub velocity: f32,
    /// `m_isVisible`: default to showing if it's enabled via INI.
    pub is_visible: bool,
    /// `m_startingHeights`: `SNOW_NOISE_X * SNOW_NOISE_Y` starting heights of each particle.
    pub starting_heights: Vec<f32>,
    /// `m_frequencyScaleX`.
    pub frequency_scale_x: f32,
    /// `m_frequencyScaleY`.
    pub frequency_scale_y: f32,
    /// `m_amplitude`.
    pub amplitude: f32,
    /// `m_pointSize`.
    pub point_size: f32,
    /// `m_quadSize`.
    pub quad_size: f32,
    /// `m_boxDimensions`.
    pub box_dimensions: f32,
    /// `m_emitterSpacing`.
    pub emitter_spacing: f32,
    /// `m_maxPointSize`.
    pub max_point_size: f32,
    /// `m_minPointSize`.
    pub min_point_size: f32,
    /// `m_fullTimePeriod`: time for a snow flake to make it from top to bottom of the rendered cube around the camera.
    pub full_time_period: f32,
    /// `m_indexBuffer` exists (the quad fallback's static indices, `SNOW_BATCH_SIZE * 6` of them).
    pub index_buffer: Option<Vec<u16>>,
    /// `m_snowTexture`: the renderer's texture, and its name.
    pub snow_texture: Option<(TextureHandle, Vec<u8>)>,
    /// `m_VertexBufferD3D`: the point sprite vertex positions (`SNOW_BUFFER_SIZE`); `None` where upstream's is NULL.
    pub point_vertices: Option<Vec<Vector3>>,
    /// `m_dwBase`: index to beginning of unused vertex buffer space.
    pub dw_base: i32,
    /// `m_dwFlush`: maximum amount of vertices to submit before rendering.
    pub dw_flush: i32,
    /// `m_dwDiscard`: maximum index allowed before needing to discard the buffer.
    pub dw_discard: i32,
    /// `m_leafDim`: horizontal dimensions of leaf nodes that are always rendered without visibility checks.
    pub leaf_dim: i32,
    /// `m_snowCeiling`: height at the top of the cube with camera at center.
    pub snow_ceiling: f32,
    /// `m_heightTraveled`: height that snow flake traveled this frame.
    pub height_traveled: f32,
    /// `m_totalRendered`: total number of snow particles rendered this frame - only for profiling.
    pub total_rendered: i32,
    /// `m_cullOverscan`: how much extra padding to put on the sides of AABoxes when view culling.
    pub cull_overscan: f32,
}

impl Default for W3DSnowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DSnowManager {
    /// `W3DSnowManager::W3DSnowManager` (W3DSnow.cpp:60) with `SnowManager::SnowManager` (Snow.cpp).
    pub fn new() -> Self {
        W3DSnowManager {
            time: 0.0,
            velocity: 1.0,
            is_visible: true, // default to showing if it's enabled via INI.
            starting_heights: Vec::new(),
            frequency_scale_x: 0.0,
            frequency_scale_y: 0.0,
            amplitude: 0.0,
            point_size: 0.0,
            quad_size: 0.0,
            box_dimensions: 0.0,
            emitter_spacing: 0.0,
            max_point_size: 0.0,
            min_point_size: 0.0,
            full_time_period: 0.0,
            index_buffer: None,
            snow_texture: None,
            point_vertices: None,
            dw_base: 0,
            dw_flush: 0,
            dw_discard: 0,
            leaf_dim: 0,
            snow_ceiling: 0.0,
            height_traveled: 0.0,
            total_rendered: 0,
            cull_overscan: 0.0,
        }
    }

    /// `W3DSnowManager::~W3DSnowManager` (W3DSnow.cpp:68) and `SnowManager::~SnowManager`.
    pub fn destroy(&mut self) {
        self.release_resources();
        self.starting_heights = Vec::new();
    }

    /// `W3DSnowManager::init` (W3DSnow.cpp:73): `SnowManager::init()` then `ReAcquireResources()`.
    /// `rand` is C's `rand()`, `get_texture` is `WW3DAssetManager::Get_Texture`, `point_sprites_supported` is
    /// `DX8Wrapper::Get_Current_Caps()->Support_PointSprites()`.
    pub fn init(&mut self, weather: &WeatherSetting, rand: &mut impl FnMut() -> i32, point_sprites_supported: bool, get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>) {
        // SnowManager::init: starting heights of each particle.
        self.starting_heights = vec![0.0; (SNOW_NOISE_X * SNOW_NOISE_Y) as usize];
        self.time = 0.0;
        self.update_ini_settings_base(weather, rand);
        self.re_acquire_resources(weather, point_sprites_supported, get_texture);
    }

    /// `SnowManager::updateIniSettings` (Snow.cpp).
    fn update_ini_settings_base(&mut self, weather: &WeatherSetting, rand: &mut impl FnMut() -> i32) {
        // initialize a table of random starting positions for each particle.
        let box_dimensions = weather.snow_box_dimensions as i32;
        let mut dst = 0usize;
        for _y in 0..SNOW_NOISE_Y {
            for _x in 0..SNOW_NOISE_X {
                self.starting_heights[dst] = (rand() % box_dimensions) as f32;
                dst += 1;
            }
        }

        self.velocity = weather.snow_velocity;
        self.frequency_scale_x = weather.snow_frequency_scale_x;
        self.frequency_scale_y = weather.snow_frequency_scale_y;
        self.amplitude = weather.snow_amplitude;
        self.point_size = weather.snow_point_size;
        self.quad_size = weather.snow_quad_size;
        self.box_dimensions = weather.snow_box_dimensions;
        self.emitter_spacing = 1.0 / weather.snow_box_density;
        self.max_point_size = weather.snow_max_point_size;
        self.min_point_size = weather.snow_min_point_size;

        // Time for snow flake to make it from top to bottom of rendered cube around camera.
        self.full_time_period = self.box_dimensions / self.velocity;
    }

    /// `W3DSnowManager::ReleaseResources` (W3DSnow.cpp:81): releases all W3D/D3D assets before a reset.
    pub fn release_resources(&mut self) {
        self.snow_texture = None;
        self.point_vertices = None;
        self.index_buffer = None;
    }

    /// `W3DSnowManager::ReAcquireResources` (W3DSnow.cpp:93): (re)allocates all W3D/D3D assets after a reset.
    pub fn re_acquire_resources(&mut self, weather: &WeatherSetting, point_sprites_supported: bool, get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>) -> bool {
        self.release_resources();

        if !weather.snow_enabled {
            return true; // no need for resources if snow is disabled.
        }

        if weather.use_point_sprites && point_sprites_supported {
            // CreateVertexBuffer(SNOW_BUFFER_SIZE*sizeof(POINTVERTEX), WRITEONLY|DYNAMIC|POINTS, D3DFVF_XYZ, DEFAULT)
            if self.point_vertices.is_none() {
                self.point_vertices = Some(vec![Vector3::default(); SNOW_BUFFER_SIZE as usize]);
            }
        } else {
            // allocate 2 triangles per flake, each with 3 indices.
            let mut ib = vec![0u16; (SNOW_BATCH_SIZE * 6) as usize];
            // Fill up the IB with static vertex indices that will be used for all smudges.
            //   0-----3
            //   |\   /|
            //   |  X  |
            //   |/   \|
            //   1-----2
            let mut vb_count: i32 = 0;
            let mut p = 0usize;
            for _i in 0..SNOW_BATCH_SIZE {
                // Top
                ib[p] = (vb_count + 3) as u16;
                ib[p + 1] = vb_count as u16;
                ib[p + 2] = (vb_count + 2) as u16;
                // Bottom
                ib[p + 3] = (vb_count + 2) as u16;
                ib[p + 4] = vb_count as u16;
                ib[p + 5] = (vb_count + 1) as u16;
                vb_count += 4;
                p += 6;
            }
            self.index_buffer = Some(ib);
        }
        self.snow_texture = get_texture(&weather.snow_texture).map(|t| (t, weather.snow_texture.clone()));

        self.dw_base = SNOW_BUFFER_SIZE;
        self.dw_discard = SNOW_BUFFER_SIZE;
        self.dw_flush = SNOW_BATCH_SIZE;
        true
    }

    /// `W3DSnowManager::updateIniSettings` (W3DSnow.cpp:152): calls the base class, then replaces the texture if its name
    /// changed.
    pub fn update_ini_settings(&mut self, weather: &WeatherSetting, rand: &mut impl FnMut() -> i32, get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>) {
        // Call base class
        self.update_ini_settings_base(weather, rand);

        if let Some((_, name)) = &self.snow_texture {
            if !name.eq_ignore_ascii_case(&weather.snow_texture) {
                self.snow_texture = get_texture(&weather.snow_texture).map(|t| (t, weather.snow_texture.clone()));
            }
        }
    }

    /// `SnowManager::setVisible` (Snow.cpp): enable/disable rendering of weather.
    pub fn set_visible(&mut self, show_weather: bool) {
        self.is_visible = show_weather;
    }

    /// `W3DSnowManager::reset` (W3DSnow.cpp:166): `SnowManager::reset()`.
    pub fn reset(&mut self) {
        self.is_visible = true; // default to showing if it's enabled via INI.
    }

    /// `W3DSnowManager::update` (W3DSnow.cpp:172). `frame_time_ms` is `WW3D::Get_Frame_Time()`.
    pub fn update(&mut self, frame_time_ms: u32) {
        self.time += frame_time_ms as f32 / 1000.0;
        // find current time offset, adjusting for overflow
        self.time = (self.time as f64 % self.full_time_period as f64) as f32;
    }

    /// The noise lookup both render paths do: `noiseOffset`, `h0`, then the world position of the flake before the sine
    /// adjustment.
    fn flake_center(&self, x: i32, y: i32) -> (Vector3, f32) {
        // Get initial height from noise table.  We add a large value to make sure it's positive.  Then modulate by table
        // dimensions to find a value. MODPOW2(x, y) is ((x) & (y-1)).
        let mut noise_offset = ((x + MAXIMUM_CAMERA_DISTANCE) & (SNOW_NOISE_X - 1)) + ((y + MAXIMUM_CAMERA_DISTANCE) & (SNOW_NOISE_Y - 1)) * SNOW_NOISE_X;
        if noise_offset > (SNOW_NOISE_X * SNOW_NOISE_Y) {
            noise_offset = 0; // this should never happen but check to prevent buffer over/under flow.
        }

        // find current height
        let h0 = (self.snow_ceiling as f64 - ((self.height_traveled + self.starting_heights[noise_offset as usize]) as f64 % self.box_dimensions as f64)) as f32;

        // find world-space position of snow flake
        let mut snow_center = Vector3::default();
        snow_center.set(x as f32 * self.emitter_spacing, y as f32 * self.emitter_spacing, h0);
        (snow_center, h0)
    }

    /// `W3DSnowManager::renderSubBox` (W3DSnow.cpp:196): recursively subdivides the large snow box enclosing the camera
    /// until a predefined leaf size, so very few off-screen particles end up rendered. `frustum_outside` is
    /// `CollisionMath::Overlap_Test(camera.Get_Frustum(), mmbox) == OUTSIDE`.
    pub fn render_sub_box(
        &mut self,
        origin_x: i32,
        origin_y: i32,
        cube_dim_x: i32,
        cube_dim_y: i32,
        frustum_outside: &impl Fn(&MinMaxAABoxClass) -> bool,
        out: &mut SnowRender,
    ) {
        // check if this box is too large and needs subdivision
        let box_dim_x = cube_dim_x - origin_x;
        let box_dim_y = cube_dim_y - origin_y;
        let half_x = ((box_dim_x as f32 * 0.5).ceil()) as i32; // REAL_TO_INT_CEIL
        let half_y = ((box_dim_y as f32 * 0.5).ceil()) as i32;
        let es = self.emitter_spacing;
        let co = self.cull_overscan;
        let (zmin, zmax) = (self.snow_ceiling - self.box_dimensions, self.snow_ceiling);
        let mk = |x0: i32, y0: i32, x1: i32, y1: i32| -> MinMaxAABoxClass {
            MinMaxAABoxClass {
                min_corner: Vector3::new(x0 as f32 * es - co, y0 as f32 * es - co, zmin),
                max_corner: Vector3::new(x1 as f32 * es + co, y1 as f32 * es + co, zmax),
            }
        };

        if box_dim_x > self.leaf_dim {
            // subdivide the box
            if box_dim_y > self.leaf_dim {
                // subdivide in both directions
                // Upper left
                if !frustum_outside(&mk(origin_x, origin_y + half_y, origin_x + half_x, cube_dim_y)) {
                    self.render_sub_box(origin_x, origin_y + half_y, origin_x + half_x, cube_dim_y, frustum_outside, out);
                }
                // Upper right
                if !frustum_outside(&mk(origin_x + half_x, origin_y + half_y, cube_dim_x, cube_dim_y)) {
                    self.render_sub_box(origin_x + half_x, origin_y + half_y, cube_dim_x, cube_dim_y, frustum_outside, out);
                }
                // Lower left
                if !frustum_outside(&mk(origin_x, origin_y, origin_x + half_x, origin_y + half_y)) {
                    self.render_sub_box(origin_x, origin_y, origin_x + half_x, origin_y + half_y, frustum_outside, out);
                }
                // Lower right
                if !frustum_outside(&mk(origin_x + half_x, origin_y, cube_dim_x, origin_y + half_y)) {
                    self.render_sub_box(origin_x + half_x, origin_y, cube_dim_x, origin_y + half_y, frustum_outside, out);
                }
                return;
            } else {
                // only subdivide in x direction.
                // Left
                if !frustum_outside(&mk(origin_x, origin_y, origin_x + half_x, cube_dim_y)) {
                    self.render_sub_box(origin_x, origin_y, origin_x + half_x, cube_dim_y, frustum_outside, out);
                }
                // Right
                if !frustum_outside(&mk(origin_x + half_x, origin_y, cube_dim_x, cube_dim_y)) {
                    self.render_sub_box(origin_x + half_x, origin_y, cube_dim_x, cube_dim_y, frustum_outside, out);
                }
                return;
            }
        } else if box_dim_y > self.leaf_dim {
            // only subdivide in y direction
            // Top
            if !frustum_outside(&mk(origin_x, origin_y + half_y, cube_dim_x, cube_dim_y)) {
                self.render_sub_box(origin_x, origin_y + half_y, cube_dim_x, cube_dim_y, frustum_outside, out);
            }
            // Bottom
            if !frustum_outside(&mk(origin_x, origin_y, cube_dim_x, origin_y + half_y)) {
                self.render_sub_box(origin_x, origin_y, cube_dim_x, origin_y + half_y, frustum_outside, out);
            }
            return;
        }

        // Box too small to subdivide so render it.

        // Find total number of particles that need rendering.
        let mut total_part = (cube_dim_y - origin_y) * (cube_dim_x - origin_x);
        if total_part == 0 {
            return; // nothing to render.
        }
        let mut y = origin_y; // loop counter.
        let mut cube_origin_x_remainder = origin_x; // loop counter - adjusted when not all particles fit into render buffer.

        self.total_rendered += total_part;

        let mut vertices = match self.point_vertices.take() {
            Some(v) => v,
            None => return,
        };
        while total_part != 0 {
            let mut batch_size = total_part;
            if batch_size > self.dw_flush {
                batch_size = self.dw_flush;
            }
            if (self.dw_base + batch_size) > self.dw_discard {
                self.dw_base = 0;
            }

            // (Lock(m_dwBase ..., m_dwBase ? D3DLOCK_NOOVERWRITE : D3DLOCK_DISCARD))
            let mut verts = self.dw_base as usize;
            let mut number_in_batch: i32 = 0;
            'fill: while y < cube_dim_y {
                let mut x = cube_origin_x_remainder;
                while x < cube_dim_x {
                    if number_in_batch >= batch_size {
                        cube_origin_x_remainder = x;
                        break 'fill; // goto flush_particles
                    }
                    let (mut snow_center, h0) = self.flake_center(x, y);

                    // Adjust position so snow flakes don't fall straight down.
                    snow_center.x += self.amplitude * wwmath::fast_sin(h0 * self.frequency_scale_x + x as f32);
                    snow_center.y += self.amplitude * wwmath::fast_sin(h0 * self.frequency_scale_y + y as f32);

                    vertices[verts] = snow_center;
                    verts += 1;
                    number_in_batch += 1;
                    x += 1;
                }
                // getting here means we did not overflow the render buffer, so reset x origin to normal.
                cube_origin_x_remainder = origin_x; // reset to normal amount
                y += 1;
            }
            // flush_particles: Unlock

            // Render any particles that may be queued up.
            if number_in_batch != 0 {
                // upstream: Debug_Statistics::Record_DX8_Polys_And_Vertices(n*2, n*4, ...) — stats only
                out.point_draws.push(SnowPointDraw { base: self.dw_base, count: number_in_batch });
                total_part -= number_in_batch;
                self.dw_base += number_in_batch;
            }
        }
        self.point_vertices = Some(vertices);
    }

    /// `W3DSnowManager::render` (W3DSnow.cpp:326). Parameters stand for: `point_sprites_supported`
    /// (`Support_PointSprites()`), `camera_pos` (`TheTacticalView->get3DCameraPosition()`), `max_visible_box`
    /// (`TheTerrainRenderObject->getMaximumVisibleBox(frustum, &bbox, TRUE)`), `view` (`camera.Get_View_Matrix`) and
    /// `frustum_outside` (the frustum test). The render states (point sprite size and scale) are
    /// `m_pointSize`, `m_minPointSize`, `m_maxPointSize`, scale (0, 0, 1): device-only.
    pub fn render(
        &mut self,
        weather: &WeatherSetting,
        point_sprites_supported: bool,
        camera_pos: Vector3,
        max_visible_box: AABoxClass,
        view: &Matrix3D,
        frustum_outside: &impl Fn(&MinMaxAABoxClass) -> bool,
        get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>,
    ) -> SnowRender {
        let mut out = SnowRender::default();
        if !weather.snow_enabled || !self.is_visible {
            return out;
        }

        let use_point_sprites = point_sprites_supported && weather.use_point_sprites;

        // make sure the noise table is powers of 2 in dimensions: ISPOW2(SNOW_NOISE_X) && ISPOW2(SNOW_NOISE_Y)

        let cam_pos = camera_pos;

        // Number of emitters from cube center to edge of visible extent.
        let mum_emitters_in_half = ((self.box_dimensions / self.emitter_spacing * 0.5) as f64).floor() as i32;

        // Find origin of visible cube surrounding camera.
        let cube_center_x = ((cam_pos.x / self.emitter_spacing) as f64).floor() as i32;
        let cube_center_y = ((cam_pos.y / self.emitter_spacing) as f64).floor() as i32;

        // Find extents of visible cube surrounding camera.
        let mut cube_origin_x = cube_center_x - mum_emitters_in_half; // top/left extents.
        let mut cube_origin_y = cube_center_y - mum_emitters_in_half;
        let mut cube_dim_x = cube_center_x + mum_emitters_in_half; // bottom/right extents.
        let mut cube_dim_y = cube_center_y + mum_emitters_in_half;

        // Get a bounding box around our visible universe.  Bounded by terrain and the sky so much tighter fitting volume
        // than what's actually visible.  This will cull particles falling under the ground.
        let mut bbox = max_visible_box;

        // Particles move outside the visible box as a result of local sine movement so adjust bounding box to include them.
        bbox.extent.x += self.amplitude + self.quad_size;
        bbox.extent.y += self.amplitude + self.quad_size;

        // Clip our visible snow rendering box
        if (cube_origin_x as f32 * self.emitter_spacing) < (bbox.center.x - bbox.extent.x) {
            cube_origin_x = (((bbox.center.x - bbox.extent.x) / self.emitter_spacing) as f64).floor() as i32;
        }
        if (cube_origin_y as f32 * self.emitter_spacing) < (bbox.center.y - bbox.extent.y) {
            cube_origin_y = (((bbox.center.y - bbox.extent.y) / self.emitter_spacing) as f64).floor() as i32;
        }
        if (cube_dim_x as f32 * self.emitter_spacing) > (bbox.center.x + bbox.extent.x) {
            cube_dim_x = (((bbox.center.x + bbox.extent.x) / self.emitter_spacing) as f64).floor() as i32;
        }
        if (cube_dim_y as f32 * self.emitter_spacing) > (bbox.center.y + bbox.extent.y) {
            cube_dim_y = (((bbox.center.y + bbox.extent.y) / self.emitter_spacing) as f64).floor() as i32;
        }

        if (cube_dim_y - cube_origin_y) < 0 || (cube_dim_x - cube_origin_x) < 0 {
            return out; // entire snow box is culled by either x or y screen boundary.
        }

        // Find total number of particles that need rendering.
        let total_part = (cube_dim_y - cube_origin_y) * (cube_dim_x - cube_origin_x);
        if total_part <= 0 {
            return out; // nothing to render.
        }

        // Height at the top of the cube with camera at center.
        self.snow_ceiling = cam_pos.z + self.box_dimensions / 2.0;

        // Offset to allow cube extents to move with camera.
        let camera_offset = (cam_pos.z as f64 % self.box_dimensions as f64) as f32;
        self.height_traveled = self.time * self.velocity + camera_offset; // height that snow flake traveled this frame.

        // upstream: Set_Transform(WORLD, identity); Set_Shader(_PresetAlphaShader); Set_Material(PRELIT_DIFFUSE)

        // make sure we have all the resources we need
        if use_point_sprites && self.point_vertices.is_none() {
            self.re_acquire_resources(weather, point_sprites_supported, get_texture);
        }
        if !use_point_sprites && self.index_buffer.is_none() {
            self.re_acquire_resources(weather, point_sprites_supported, get_texture);
        }

        // upstream: Set_Texture(0, m_snowTexture)

        if !use_point_sprites {
            self.render_as_quads(view, cube_origin_x, cube_origin_y, cube_dim_x, cube_dim_y, &mut out);
            return out;
        }

        out.point_sprites = true;

        // Set the render states for using point sprites (device-only) ...
        self.dw_base = SNOW_BUFFER_SIZE; // start with a new vertex buffer each frame.

        self.leaf_dim = 45; // cull boxes that are 20x20 emitters in size. Making them much smaller will result in too many draw calls.
        self.total_rendered = 0; // keep track of how many particles were rendered.

        // Particle centers can deviate from center by by amplitude of sine offset.  They also have radius m_quadSize.
        // Enlarge culling bounds to compensate.
        self.cull_overscan = self.amplitude + self.quad_size;

        self.render_sub_box(cube_origin_x, cube_origin_y, cube_dim_x, cube_dim_y, frustum_outside, &mut out);

        // Reset render states (device-only)
        out
    }

    /// `W3DSnowManager::renderAsQuads` (W3DSnow.cpp:487): for hardware that doesn't support point sprites.
    pub fn render_as_quads(&mut self, view: &Matrix3D, cube_origin_x: i32, cube_origin_y: i32, cube_dim_x: i32, cube_dim_y: i32, out: &mut SnowRender) {
        let mut vertex_offsets = [Vector3::new(-0.5, 0.5, 0.0), Vector3::new(-0.5, -0.5, 0.0), Vector3::new(0.5, -0.5, 0.0), Vector3::new(0.5, 0.5, 0.0)];
        let quad_uvs = [Vector2::new(0.0, 0.0), Vector2::new(0.0, 1.0), Vector2::new(1.0, 1.0), Vector2::new(1.0, 0.0)];

        // pre-multiple the offsets by particle size
        for i in 0..4 {
            vertex_offsets[i] *= self.quad_size;
        }

        // upstream: Set_Transform(D3DTS_VIEW, identity); Set_Index_Buffer(m_indexBuffer, 0)

        let mut y = cube_origin_y; // loop counter.
        let mut cube_origin_x_remainder = cube_origin_x; // loop counter - adjusted when not all particles fit into render buffer.

        // Find total number of particles that need rendering.
        let mut total_part = (cube_dim_y - cube_origin_y) * (cube_dim_x - cube_origin_x);

        self.total_rendered += total_part;
        while total_part != 0 {
            let mut batch_size = total_part;
            if batch_size > SNOW_BATCH_SIZE {
                batch_size = SNOW_BATCH_SIZE;
            }

            let mut number_in_batch: i32 = 0;
            // DynamicVBAccessClass(BUFFER_TYPE_DYNAMIC_DX8, dynamic_fvf_type, batchSize*4): 4 verts per flake
            let mut verts: Vec<SnowQuadVertex> = Vec::with_capacity((batch_size * 4) as usize);
            'fill: while y < cube_dim_y {
                let mut x = cube_origin_x_remainder;
                while x < cube_dim_x {
                    if number_in_batch >= batch_size {
                        cube_origin_x_remainder = x;
                        break 'fill; // goto flush_particles
                    }
                    let (snow_center, h0) = self.flake_center(x, y);

                    // Get view-space position
                    let mut snow_center_vs = Matrix3D::transform_vector(view, &snow_center);

                    // Adjust position so snow flakes don't fall straight down.
                    snow_center_vs.x += self.amplitude * wwmath::fast_sin(h0 * self.frequency_scale_x + x as f32);
                    snow_center_vs.y += self.amplitude * wwmath::fast_sin(h0 * self.frequency_scale_y + y as f32);

                    for i in 0..4 {
                        verts.push(SnowQuadVertex {
                            pos: snow_center_vs + vertex_offsets[i],
                            normal: Vector3::new(0.0, 0.0, 0.0), // keep AGP write-combining active
                            diffuse: 0xffff_ffff,                // set to opaque
                            u1: quad_uvs[i].x,
                            v1: quad_uvs[i].y,
                            u2: 0.0, // keep AGP write-combining active
                            v2: 0.0,
                        });
                    }
                    number_in_batch += 1;
                    x += 1;
                }
                // getting here means we did not overflow the render buffer, so reset x origin to normal.
                cube_origin_x_remainder = cube_origin_x; // reset to normal amount
                y += 1;
            }
            // flush_particles:

            // Render any particles that may be queued up.
            if number_in_batch != 0 {
                // upstream: Set_Vertex_Buffer(vb_access); Draw_Triangles(0, numberInBatch*2, 0, numberInBatch*4)
                out.quad_batches.push(verts);
                total_part -= number_in_batch;
            }
        }
    }
}
