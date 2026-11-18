//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DWaterTracks.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Water/W3DWaterTracks.cpp
//!
//! As in `w3d_terrain_tracks.rs`, the linked lists of `WaterTracksObj` are indices into one `Vec`. `render` writes the
//! four vertices of a wave into the vertex buffer `Vec` and returns the `Draw_Strip` as data. `TheTerrainLogic->isUnderwater`,
//! the time of day colours, the shroud and the water settings are parameters.
//!
//! Not translated: `TestWaterUpdate`, the in-game wave editor (`GetAsyncKeyState`, `GetCursorPos`, `HWND`, the in-game
//! UI messages and `TheDisplay->drawLine`; it runs only when `m_usingWaterTrackEditor` is set, a development setting), and
//! the `#if 0` obsolete part of `loadTracks`. `saveTracks` returns the bytes `fwrite` would write instead of opening a
//! file, and `loadTracks` takes the bytes of the `.wak` file.
//!
//! Notes on upstream behaviour kept as it is:
//! - `m_totalMs` and `m_timeToCompress` conversions: `m_totalMs` is an `Int` assigned a `Real` (truncation, the C cast).
//! - `m_flipU` is not set by the constructor (uninitialised upstream, zero here); `loadTracks` sets it.
//! - `WaterTracksRenderSystem::update` locks the frame rate to 30 fps: `timeDiff = 1.0f/30.0f*1000.0f`, truncated to 33.
//! - `flush` never updates `LastTextureType`, so the texture is set for every wave.
//! - `sqrt(fabs(x))`: `fabs` and `sqrt` are the double overloads of <math.h> (no `abs` on a float here).
//! - `TheTerrainLogic->isUnderwater(x, y, &waterHeight)` only writes `waterHeight` when the point is in water, so the
//!   closure returns `Option<f32>` and a `None` keeps the previous value (`render`'s local, zero when never written;
//!   uninitialised upstream).

use super::w3d_common::{AABoxClass, SphereClass, TextureHandle, VertexFormatXYZDUV1};
use rts_lockstep_engine::common::base_type::{real_to_int, PI};
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::logic::terrain_visual::MAP_XY_FACTOR;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::wwmath;

/// `WATER_VB_PAGES`: number of vertex pages allocated - allows double buffering of vertex updates.
pub const WATER_VB_PAGES: i32 = 1000;
/// `WATER_STRIP_X`: vertex resolution of each strip.
pub const WATER_STRIP_X: i32 = 2;
/// `WATER_STRIP_Y`.
pub const WATER_STRIP_Y: i32 = 2;

/// `enum waveType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum WaveType {
    /// `WaveTypePond` (`WaveTypeFirst`).
    Pond = 0,
    /// `WaveTypeOcean`.
    Ocean = 1,
    /// `WaveTypeCloseOcean`: same as above but appears much closer to beach.
    CloseOcean = 2,
    /// `WaveTypeCloseOceanDouble`: same as above but waves much closer together.
    CloseOceanDouble = 3,
    /// `WaveTypeRadial` (`WaveTypeLast`).
    Radial = 4,
    /// `WaveTypeStationary`.
    Stationary = 5,
    /// `WaveTypeMax`.
    Max = 6,
}

impl WaveType {
    /// The enum from the `int` `loadTracks` reads from the file; out of range is undefined upstream (it indexes
    /// `waveTypeInfo`), here it reads as `Max`.
    pub fn from_i32(v: i32) -> WaveType {
        match v {
            0 => WaveType::Pond,
            1 => WaveType::Ocean,
            2 => WaveType::CloseOcean,
            3 => WaveType::CloseOceanDouble,
            4 => WaveType::Radial,
            5 => WaveType::Stationary,
            _ => WaveType::Max,
        }
    }
}

/// `struct waveInfo`.
#[derive(Clone, Copy, Debug)]
pub struct WaveInfo {
    /// `m_finalWidth`: final width of wave when it reaches beach.
    pub final_width: f32,
    /// `m_finalHeight`: final height of wave after it stretched out on beach.
    pub final_height: f32,
    /// `m_waveDistance`: distance away from beach where wave starts.
    pub wave_distance: f32,
    /// `m_initialVelocity`.
    pub initial_velocity: f32,
    /// `m_fadeMs`: time to fade out wave after it stops on shore.
    pub fade_ms: i32,
    /// `m_initialWidthFraction`: fraction of `m_finalWidth` when wave first appears.
    pub initial_width_fraction: f32,
    /// `m_initialHeightWidthFraction`: fraction of initial width to use as the initial height.
    pub initial_height_width_fraction: f32,
    /// `m_timeToCompress`: time for back of wave to continue moving forward after front starts retreating.
    pub time_to_compress: i32,
    /// `m_secondWaveTimeOffset`: time for second wave to start. Should always be half of first wave's TotalMs.
    pub second_wave_time_offset: i32,
    /// `m_textureName`: name of texture to use on wave.
    pub texture_name: &'static [u8],
    /// `m_waveTypeName`: name of this wave type.
    pub wave_type_name: &'static [u8],
}

/// `waveTypeInfo[WaveTypeMax]`. (`WaveTypeStationary` has no initialiser upstream: zeros.)
pub const WAVE_TYPE_INFO: [WaveInfo; 6] = [
    WaveInfo { final_width: 28.0, final_height: 18.0, wave_distance: 25.0, initial_velocity: 0.018, fade_ms: 900, initial_width_fraction: 0.01, initial_height_width_fraction: 0.18, time_to_compress: 1500, second_wave_time_offset: 0, texture_name: b"wave256.tga", wave_type_name: b"Pond" }, // pond
    WaveInfo { final_width: 55.0, final_height: 36.0, wave_distance: 80.0, initial_velocity: 0.015, fade_ms: 2000, initial_width_fraction: 0.5, initial_height_width_fraction: 0.18, time_to_compress: 1000, second_wave_time_offset: 6267, texture_name: b"wave256.tga", wave_type_name: b"Ocean" }, // ocean
    WaveInfo { final_width: 55.0, final_height: 36.0, wave_distance: 80.0, initial_velocity: 0.015, fade_ms: 2000, initial_width_fraction: 0.05, initial_height_width_fraction: 0.18, time_to_compress: 1000, second_wave_time_offset: 6267, texture_name: b"wave256.tga", wave_type_name: b"Close Ocean" },
    WaveInfo { final_width: 55.0, final_height: 36.0, wave_distance: 80.0, initial_velocity: 0.015, fade_ms: 4000, initial_width_fraction: 0.01, initial_height_width_fraction: 0.18, time_to_compress: 2000, second_wave_time_offset: 6267, texture_name: b"wave256.tga", wave_type_name: b"Close Ocean Double" },
    WaveInfo { final_width: 55.0, final_height: 27.0, wave_distance: 80.0, initial_velocity: 0.015, fade_ms: 2000, initial_width_fraction: 0.01, initial_height_width_fraction: 8.0, time_to_compress: 2000, second_wave_time_offset: 5367, texture_name: b"wave256.tga", wave_type_name: b"Radial" },
    WaveInfo { final_width: 0.0, final_height: 0.0, wave_distance: 0.0, initial_velocity: 0.0, fade_ms: 0, initial_width_fraction: 0.0, initial_height_width_fraction: 0.0, time_to_compress: 0, second_wave_time_offset: 0, texture_name: b"", wave_type_name: b"" },
];

/// `waveTypeInfo[type]` (an index past the table is undefined upstream; `Max` reads the zero entry here).
fn wave_info(t: WaveType) -> &'static WaveInfo {
    let i = t as usize;
    if i < WAVE_TYPE_INFO.len() {
        &WAVE_TYPE_INFO[i]
    } else {
        &WAVE_TYPE_INFO[5]
    }
}

/// One `DX8Wrapper::Set_Index_Buffer(m_indexBuffer, batchStart)` + `Draw_Strip(0, idxCount-2, 0, m_x*m_y)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaterStripDraw {
    /// `Set_Index_Buffer(..., index_base_vertex)`: the batch's first vertex.
    pub index_base_vertex: i32,
    /// `Draw_Strip`'s `start_index`.
    pub start_index: i32,
    /// `Draw_Strip`'s `polygon_count` (`idxCount-2`).
    pub polygon_count: i32,
    /// `Draw_Strip`'s `min_vertex_index`.
    pub min_vertex_index: i32,
    /// `Draw_Strip`'s `vertex_count` (`m_x*m_y`).
    pub vertex_count: i32,
}

/// `class WaterTracksObj`: draws a small breaking wave or splash animation. Managed by `WaterTracksRenderSystem` only.
#[derive(Clone, Debug)]
pub struct WaterTracksObj {
    /// `m_stageZeroTexture`: primary texture.
    pub stage_zero_texture: Option<TextureHandle>,
    /// `m_boundingSphere`.
    pub bounding_sphere: SphereClass,
    /// `m_boundingBox`.
    pub bounding_box: AABoxClass,
    /// `m_type`: used for render state sorting.
    pub wave_type: WaveType,
    /// `m_x`: vertex count.
    pub x: i32,
    /// `m_y`: vertex count.
    pub y: i32,
    /// `m_bound`: object is bound to owner and accepts new edges.
    pub bound: bool,
    /// `m_startPos`: starting position of wave.
    pub start_pos: Vector2,
    /// `m_waveDir`: direction of wave travel.
    pub wave_dir: Vector2,
    /// `m_perpDir`: direction perpendicular to wave travel.
    pub perp_dir: Vector2,
    /// `m_initStartPos`: original settings used to create wave.
    pub init_start_pos: Vector2,
    /// `m_initEndPos`: original settings used to create wave.
    pub init_end_pos: Vector2,
    /// `m_initTimeOffset`: time offset when wave is added into the system.
    pub init_time_offset: i32,
    /// `m_fadeMs`: time for wave to fade out after it stops moving.
    pub fade_ms: i32,
    /// `m_totalMs`: amount of time to complete full motion.
    pub total_ms: i32,
    /// `m_elapsedMs`: amount of time since start of motion.
    pub elapsed_ms: i32,
    /// `m_waveInitialWidth`: width of wave segment when it first appears.
    pub wave_initial_width: f32,
    /// `m_waveInitialHeight`: height of wave segment when it first appears.
    pub wave_initial_height: f32,
    /// `m_waveFinalWidth`: width of wave segment at full size.
    pub wave_final_width: f32,
    /// `m_waveFinalWidthPeakFrac`: fraction along path when wave reaches full width.
    pub wave_final_width_peak_frac: f32,
    /// `m_waveFinalHeight`: final height of unstretched wave.
    pub wave_final_height: f32,
    /// `m_initialVelocity`: initial velocity in world units per ms.
    pub initial_velocity: f32,
    /// `m_waveDistance`: total distance traveled by wave front.
    pub wave_distance: f32,
    /// `m_timeToReachBeach`.
    pub time_to_reach_beach: f32,
    /// `m_frontSlowDownAcc`.
    pub front_slow_down_acc: f32,
    /// `m_timeToStop`.
    pub time_to_stop: f32,
    /// `m_timeToRetreat`.
    pub time_to_retreat: f32,
    /// `m_backSlowDownAcc`.
    pub back_slow_down_acc: f32,
    /// `m_timeToCompress`.
    pub time_to_compress: f32,
    /// `m_flipU`: force uv coordinates to flip.
    pub flip_u: f32,
    /// `m_nextSystem`.
    pub next_system: Option<usize>,
    /// `m_prevSystem`.
    pub prev_system: Option<usize>,
}

impl Default for WaterTracksObj {
    fn default() -> Self {
        Self::new()
    }
}

impl WaterTracksObj {
    /// `WaterTracksObj::WaterTracksObj` (W3DWaterTracks.cpp:124): just nulls out some variables (the rest is
    /// uninitialised upstream and zero here).
    pub fn new() -> Self {
        WaterTracksObj {
            stage_zero_texture: None,
            bounding_sphere: SphereClass::default(),
            bounding_box: AABoxClass::default(),
            wave_type: WaveType::Pond,
            x: 0,
            y: 0,
            bound: false,
            start_pos: Vector2::default(),
            wave_dir: Vector2::default(),
            perp_dir: Vector2::default(),
            init_start_pos: Vector2::default(),
            init_end_pos: Vector2::default(),
            init_time_offset: 0,
            fade_ms: 0,
            total_ms: 0,
            elapsed_ms: 0,
            wave_initial_width: 0.0,
            wave_initial_height: 0.0,
            wave_final_width: 0.0,
            wave_final_width_peak_frac: 0.0,
            wave_final_height: 0.0,
            initial_velocity: 0.0,
            wave_distance: 0.0,
            time_to_reach_beach: 0.0,
            front_slow_down_acc: 0.0,
            time_to_stop: 0.0,
            time_to_retreat: 0.0,
            back_slow_down_acc: 0.0,
            time_to_compress: 0.0,
            flip_u: 0.0,
            next_system: None,
            prev_system: None,
        }
    }

    /// `Get_Obj_Space_Bounding_Sphere` (W3DWaterTracks.cpp:136).
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        self.bounding_sphere
    }

    /// `Get_Obj_Space_Bounding_Box` (W3DWaterTracks.cpp:146).
    pub fn get_obj_space_bounding_box(&self) -> AABoxClass {
        self.bounding_box
    }

    /// `WaterTracksObj::freeWaterTracksResources` (W3DWaterTracks.cpp:157): free any W3D resources associated with this
    /// object.
    pub fn free_water_tracks_resources(&mut self) -> i32 {
        self.stage_zero_texture = None;
        0
    }

    /// `WaterTracksObj::init(width, length, start, end, texturename, waveTimeOffset)` (W3DWaterTracks.cpp:169): setup
    /// size settings and allocate the W3D texture (`texture` is `Get_Texture(texturename)`). The width/length define the
    /// size of the polygon quad which will contain the specified texture.
    pub fn init(&mut self, width: f32, length: f32, start: Vector2, end: Vector2, texture: Option<TextureHandle>, wave_time_offset: i32) {
        self.free_water_tracks_resources(); // free old resources used by this track

        // save original settings used to create this wave
        self.init_start_pos = start;
        self.init_end_pos = end;
        self.init_time_offset = wave_time_offset;

        self.bounding_sphere = SphereClass::new(Vector3::new(0.0, 0.0, 0.0), 400.0);
        self.bounding_box.center.set(0.0, 0.0, 0.0);
        self.bounding_box.extent.set(400.0, 400.0, 1.0);
        self.x = WATER_STRIP_X;
        self.y = WATER_STRIP_Y;
        self.elapsed_ms = self.init_time_offset;
        self.start_pos = start;
        self.wave_dir = end - start;
        self.perp_dir = self.wave_dir;
        self.perp_dir.rotate(-1.57079632679); // get vector perpendicular to wave motion.
        self.perp_dir.normalize();

        self.wave_dir = self.perp_dir;
        self.wave_dir.rotate(PI / 2.0); // get vector along wave travel direction.
        // move back by width of wave so start point turns into maximum reach of wave.
        // move back initial tip off of wave a couple units off the final position to give it some room to travel.
        // Travel vector is stored in m_waveDir.
        let info = wave_info(self.wave_type);
        self.wave_distance = info.wave_distance; // total distance traveled by wave front

        self.wave_dir *= self.wave_distance;
        self.start_pos -= self.wave_dir; // move start point down away from shoreline

        self.initial_velocity = info.initial_velocity; // velocity per ms
        self.total_ms = (self.wave_distance / self.initial_velocity) as i32; // amount of time for wave to travel complete distance

        self.fade_ms = info.fade_ms; // time for wave to fade out after it stops on beach

        self.wave_initial_width = length * info.initial_width_fraction; // width of wave segment when it first appears
        self.wave_initial_height = self.wave_initial_width * info.initial_height_width_fraction; // height of wave segment when it first appears
        self.wave_final_width = length; // width of wave segment at full size
        self.wave_final_height = width; // final height of unstretched wave

        // get total time for front to complete its cycle
        self.time_to_reach_beach = (self.wave_distance - self.wave_final_height) / self.initial_velocity;
        self.front_slow_down_acc = -(self.initial_velocity * self.initial_velocity) / (2.0 * self.wave_final_height); // deceleration of wave after it hits land
        self.time_to_stop = -self.initial_velocity / self.front_slow_down_acc;
        self.time_to_retreat = wwmath::sqrt(wwmath::fabs(2.0 * self.wave_final_height / self.front_slow_down_acc));
        self.total_ms = (self.time_to_reach_beach + self.time_to_stop + self.time_to_retreat) as i32; // total time that wave front is on screen
        self.back_slow_down_acc = 2.0 * self.wave_initial_height / (self.time_to_stop * self.time_to_stop);
        self.time_to_compress = info.time_to_compress as f32; // time for back of wave to continue moving forward after front starts retreating.

        if self.wave_type == WaveType::Stationary {
            // this is a stationary wave slightly behind starting point
            self.time_to_retreat = 1000.0; // time to fade out.
            self.total_ms = (self.time_to_reach_beach + self.time_to_stop + self.fade_ms as f32 + self.time_to_retreat) as i32; // trigger when other wave stops.
            self.start_pos = start;
            self.fade_ms = 1000; // time for wave to fade out after it stops on beach
        }

        self.stage_zero_texture = texture;
    }

    /// `WaterTracksObj::init(width, start, end, texturename)` (W3DWaterTracks.cpp:243): alternate version of init where
    /// (start, end) define a vector perpendicular to wave travel. The length of this vector is used as the length of the
    /// wave segment. The line between start-end defines the maximum distance the wave will reach.
    pub fn init_alt(&mut self, width: f32, start: Vector2, end: Vector2, texture: Option<TextureHandle>) {
        self.free_water_tracks_resources(); // free old resources used by this track
        self.bounding_sphere = SphereClass::new(Vector3::new(0.0, 0.0, 0.0), 400.0);
        self.bounding_box.center.set(0.0, 0.0, 0.0);
        self.bounding_box.extent.set(400.0, 400.0, 1.0);
        self.perp_dir = end - start;
        self.start_pos = start + self.perp_dir * 0.5; // move start point to middle
        let length = self.perp_dir.length();
        self.perp_dir *= 1.0 / length; // normalize it.
        self.wave_dir = self.perp_dir;
        self.wave_dir.rotate(PI / 2.0); // get vector along wave travel direction.
        self.start_pos -= self.wave_dir * width; // move back by width of wave
        self.wave_dir *= 1.3 * MAP_XY_FACTOR; // travel 4 units
        self.start_pos -= self.wave_dir; // move start point down away from shoreline
        self.x = WATER_STRIP_X;
        self.y = WATER_STRIP_Y;
        self.elapsed_ms = 0;
        self.initial_velocity = 0.001 * MAP_XY_FACTOR; // velocity per ms
        self.total_ms = (self.wave_dir.length() / self.initial_velocity) as i32;
        self.fade_ms = 3000; // time for wave to fade out after it stops on beach

        self.stage_zero_texture = texture;
    }

    /// `WaterTracksObj::update` (W3DWaterTracks.cpp:277): update state of object - advance animations and other states.
    /// (The `ALLOW_WATER_TRACK_EDIT` pause is not compiled.)
    pub fn update(&mut self, ms_elapsed: i32) -> i32 {
        // nothing to do here...most of the work is done on render.
        self.elapsed_ms += ms_elapsed; // advance time for this update

        // @todo: Should check in here if we are done with this object are return FALSE to free it.

        1 // assume we had an update
    }

    /// `WaterTracksObj::render` (W3DWaterTracks.cpp:303): draws the object in its current state. Writes its four
    /// vertices into `vertex_buffer` (`WATER_VB_PAGES*WATER_STRIP_X*WATER_STRIP_Y` vertices) at `batch_start` (or at 0
    /// when the page is full: the DISCARD lock) and returns the new offset into the unused area of the buffer and the
    /// strip draw. `is_underwater(x, y)` is `TheTerrainLogic->isUnderwater(x, y, &waterHeight)`: `Some(waterHeight)`
    /// when underwater.
    pub fn render(
        &mut self,
        vertex_buffer: &mut [VertexFormatXYZDUV1],
        batch_start: i32,
        is_underwater: &impl Fn(f32, f32) -> Option<f32>,
    ) -> (i32, WaterStripDraw) {
        let mut batch_start = batch_start;
        let mut water_height: f32 = 0.0;
        let wave_alpha: f32;
        let mut width_frac: f32;
        let wave_tail_origin: Vector2;
        let wave_front_origin: Vector2;
        let oo_wave_dir_len: f32 = 1.0 / self.wave_dir.length(); // one over length

        if batch_start < (WATER_VB_PAGES * WATER_STRIP_X * WATER_STRIP_Y - self.x * self.y) {
            // we have room in current VB, append new verts (Lock(..., D3DLOCK_NOOVERWRITE))
        } else {
            // ran out of room in last VB, request a substitute VB (Lock(0, ..., D3DLOCK_DISCARD)).
            batch_start = 0; // reset start of page to first vertex
        }
        let mut vb = batch_start as usize;

        width_frac = 1.0;

        if self.wave_type == WaveType::Stationary {
            // stationary wave
            let mut front = self.start_pos;
            front -= self.perp_dir * self.wave_final_width * 0.5; // offset to left edge of wave
            wave_front_origin = front;
            wave_tail_origin = wave_front_origin - self.wave_final_height * oo_wave_dir_len * self.wave_dir;
            let mut alpha: f32 = 0.0;

            if self.elapsed_ms >= self.total_ms {
                self.elapsed_ms = 0; // done with effect*/
            }
            if self.elapsed_ms as f32 > (self.time_to_reach_beach + self.time_to_stop - 1000.0 + self.fade_ms as f32) {
                // fading out
                alpha = self.elapsed_ms as f32 - (self.time_to_reach_beach + self.time_to_stop - 1000.0 + self.fade_ms as f32);
                alpha = alpha / self.time_to_retreat;
                alpha = 1.0 - alpha;
                if alpha < 0.0 {
                    alpha = 0.0;
                }
            } else if self.elapsed_ms as f32 > (self.time_to_reach_beach + self.time_to_stop - 1000.0) {
                // start fading up
                alpha = self.elapsed_ms as f32 - (self.time_to_reach_beach + self.time_to_stop - 1000.0);
                alpha = alpha / self.fade_ms as f32;
                if alpha > 1.0 {
                    alpha = 1.0;
                }
            }
            wave_alpha = alpha;
        } else {
            // moving wave
            let mut alpha: f32;
            let front: Vector2;
            let tail: Vector2;

            // get coordinate of top left of wave strip
            if (self.elapsed_ms as f32) < self.time_to_reach_beach {
                // wave has not reached beach yet so position only depends on velocity
                alpha = self.elapsed_ms as f32 / self.time_to_reach_beach;
                width_frac = alpha;
                width_frac = (self.wave_initial_width + width_frac * (self.wave_final_width - self.wave_initial_width)) / self.wave_final_width;

                let mut f = self.start_pos + self.initial_velocity * self.elapsed_ms as f32 * oo_wave_dir_len * self.wave_dir;
                f -= self.perp_dir * self.wave_final_width * 0.5 * width_frac; // offset to left edge of wave
                front = f;
                // Tail of wave will be behind the front by fixed amount.
                tail = front - self.wave_initial_height * oo_wave_dir_len * self.wave_dir;
            } else if self.elapsed_ms < self.total_ms {
                // wave has reached beach and is decelerating
                width_frac = 1.0;
                // Get position of wave when it hit the beach
                let mut f = self.start_pos + self.initial_velocity * self.time_to_reach_beach * oo_wave_dir_len * self.wave_dir;
                let mut t = f; // store position for calculating tail position
                // Add movement after it hit the beach
                let mut elapsed_ms: f32 = self.elapsed_ms as f32 - self.time_to_reach_beach;
                f += (self.initial_velocity * elapsed_ms + 0.5 * self.front_slow_down_acc * elapsed_ms * elapsed_ms) * oo_wave_dir_len * self.wave_dir;
                f -= self.perp_dir * self.wave_final_width * 0.5 * width_frac; // offset to left edge of wave

                let mut time_since_backtrack: f32 = self.elapsed_ms as f32 - self.time_to_reach_beach - self.time_to_stop;
                if time_since_backtrack < 0.0 {
                    time_since_backtrack = 0.0;
                }
                alpha = time_since_backtrack / self.fade_ms as f32;
                if alpha > 1.0 {
                    alpha = 1.0;
                }

                alpha = 1.0 - alpha;

                // Get position of tail when front hits the beach.
                t -= self.wave_initial_height * oo_wave_dir_len * self.wave_dir;

                if self.elapsed_ms as f32 > (self.time_to_reach_beach + self.time_to_stop + self.time_to_compress) {
                    elapsed_ms = elapsed_ms; // @todo: bug? (upstream's self-assignment)
                    let _ = elapsed_ms;
                    t += (0.5 * self.back_slow_down_acc * (self.time_to_stop + self.time_to_compress) * (self.time_to_stop + self.time_to_compress)) * oo_wave_dir_len * self.wave_dir;
                    // get time since wave should have stopped moving forward
                    let new_elapsed: f32 = self.elapsed_ms as f32 - (self.time_to_reach_beach + self.time_to_stop + self.time_to_compress);
                    t += (0.5 * self.front_slow_down_acc * new_elapsed * new_elapsed) * oo_wave_dir_len * self.wave_dir;
                } else {
                    // find position of tail including slowdown after it hit the beach
                    t += (0.5 * self.back_slow_down_acc * elapsed_ms * elapsed_ms) * oo_wave_dir_len * self.wave_dir;
                }

                t -= self.perp_dir * self.wave_final_width * 0.5 * width_frac; // offset to left edge of wave
                front = f;
                tail = t;
            } else {
                self.elapsed_ms = 0;
                alpha = self.elapsed_ms as f32 / self.time_to_reach_beach;
                width_frac = alpha;
                width_frac = (self.wave_initial_width + width_frac * (self.wave_final_width - self.wave_initial_width)) / self.wave_final_width;

                let mut f = self.start_pos + self.initial_velocity * self.elapsed_ms as f32 * oo_wave_dir_len * self.wave_dir;
                f -= self.perp_dir * self.wave_final_width * 0.5 * width_frac; // offset to left edge of wave
                front = f;
                // Tail of wave will be behind the front by fixed amount.
                tail = front - self.wave_initial_height * oo_wave_dir_len * self.wave_dir;
            }
            wave_alpha = alpha;
            wave_front_origin = front;
            wave_tail_origin = tail;
        }

        let diffuse: u32 = ((real_to_int(wave_alpha * 255.0) << 24) | 0xffffff) as u32;
        let flip = self.flip_u != 0.0;

        // First insert tail of wave:
        let mut test_point = wave_tail_origin;
        if let Some(h) = is_underwater(test_point.x, test_point.y) {
            water_height = h;
        }
        vertex_buffer[vb].x = test_point.x;
        vertex_buffer[vb].y = test_point.y;
        vertex_buffer[vb].z = water_height + 1.5;
        vertex_buffer[vb].diffuse = diffuse;
        vertex_buffer[vb].u1 = if flip { 1.0 } else { 0.0 };
        vertex_buffer[vb].v1 = 0.0;
        vb += 1;
        test_point = wave_tail_origin + self.perp_dir * self.wave_final_width * width_frac;
        vertex_buffer[vb].x = test_point.x;
        vertex_buffer[vb].y = test_point.y;
        vertex_buffer[vb].z = water_height + 1.5;
        vertex_buffer[vb].diffuse = diffuse;
        vertex_buffer[vb].u1 = if flip { 0.0 } else { 1.0 };
        vertex_buffer[vb].v1 = 0.0;
        vb += 1;
        // insert front of wave
        test_point = wave_front_origin;
        vertex_buffer[vb].x = test_point.x;
        vertex_buffer[vb].y = test_point.y;
        vertex_buffer[vb].z = water_height + 1.5;
        vertex_buffer[vb].diffuse = diffuse;
        vertex_buffer[vb].u1 = if flip { 1.0 } else { 0.0 };
        vertex_buffer[vb].v1 = 1.0;
        vb += 1;
        test_point = wave_front_origin + self.perp_dir * self.wave_final_width * width_frac;
        vertex_buffer[vb].x = test_point.x;
        vertex_buffer[vb].y = test_point.y;
        vertex_buffer[vb].z = water_height + 1.5;
        vertex_buffer[vb].diffuse = diffuse;
        vertex_buffer[vb].u1 = if flip { 0.0 } else { 1.0 };
        vertex_buffer[vb].v1 = 1.0;

        let idx_count = (self.y - 1) * (self.x * 2 + 2) - 2; // index count

        // Set_Index_Buffer(m_indexBuffer, batchStart); Draw_Strip(0, idxCount-2, 0, m_x*m_y): there are always n-2
        // primitives for n index strip.
        let draw = WaterStripDraw {
            index_base_vertex: batch_start,
            start_index: 0,
            polygon_count: idx_count - 2,
            min_vertex_index: 0,
            vertex_count: self.x * self.y,
        };

        (batch_start + self.x * self.y, draw) // return new offset into unused area of vertex buffer
    }
}

/// One wave of `flush`: `Set_Texture(0, mod->m_stageZeroTexture)` and the strip `render` drew.
#[derive(Clone, Copy, Debug)]
pub struct WaterTrackFlushDraw {
    /// `mod->m_stageZeroTexture`.
    pub texture: Option<TextureHandle>,
    /// The strip.
    pub draw: WaterStripDraw,
}

/// What `flush` reads besides the render system's own state.
#[derive(Clone, Copy, Debug, Default)]
pub struct WaterFlushInfo {
    /// `TheGlobalData->m_showSoftWaterEdge`.
    pub show_soft_water_edge: bool,
    /// `TheWaterTransparency->m_transparentWaterDepth`.
    pub transparent_water_depth: f32,
    /// `ShaderClass::Is_Backface_Culling_Inverted()`.
    pub backface_culling_inverted: bool,
    /// `TheTerrainRenderObject->getShroud() != NULL` (then the shroud texture shader, `ZFUNC` LESSEQUAL and the stage 1
    /// modulate are set and reset around the draws: device-only).
    pub shroud_present: bool,
}

/// `class WaterTracksRenderSystem`: system for drawing, updating, and re-using water mark render objects.
pub struct WaterTracksRenderSystem {
    /// `m_vertexBuffer`: `m_stripSizeX*m_stripSizeY*WATER_VB_PAGES` vertices.
    pub vertex_buffer: Option<Vec<VertexFormatXYZDUV1>>,
    /// `m_indexBuffer`: `idxCount` indices.
    pub index_buffer: Option<Vec<u16>>,
    /// Whether `init` made the material and shader (`PRELIT_DIFFUSE`, `_PresetAlphaShader` with cull mode disabled:
    /// water should be visible from both sides).
    pub material_and_shader_set: bool,
    /// All the wave objects; the lists index it.
    pub modules: Vec<WaterTracksObj>,
    /// `m_usedModules`: active objects being rendered in the scene.
    pub used_modules: Option<usize>,
    /// `m_freeModules`: unused modules that are free to use again.
    pub free_modules: Option<usize>,
    /// `m_stripSizeX`: resolution (vertex count) of wave strip.
    pub strip_size_x: i32,
    /// `m_stripSizeY`.
    pub strip_size_y: i32,
    /// `m_batchStart`: start of unused vertices in vertex buffer.
    pub batch_start: i32,
    /// `m_level`: water level.
    pub level: f32,
}

impl Default for WaterTracksRenderSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl WaterTracksRenderSystem {
    /// `WaterTracksRenderSystem::WaterTracksRenderSystem` (W3DWaterTracks.cpp:517): just nulls out some variables
    /// (`TheWaterTracksRenderSystem = this`: the owner keeps the single instance).
    pub fn new() -> Self {
        WaterTracksRenderSystem {
            vertex_buffer: None,
            index_buffer: None,
            material_and_shader_set: false,
            modules: Vec::new(),
            used_modules: None,
            free_modules: None,
            strip_size_x: WATER_STRIP_X,
            strip_size_y: WATER_STRIP_Y,
            batch_start: 0,
            level: 0.0,
        }
    }

    /// `WaterTracksRenderSystem::~WaterTracksRenderSystem` (W3DWaterTracks.cpp:534).
    pub fn destroy(&mut self) {
        // free all data
        self.shutdown();
        self.material_and_shader_set = false;
    }

    /// `WaterTracksRenderSystem::bindTrack` (W3DWaterTracks.cpp:426): grab a track from the free store. If no free
    /// tracks exist, return `None`. The new track goes into the used list next to tracks of the same type. (With
    /// `SYNC_WAVES` all the waves are in sync: movement resets at the same time.)
    pub fn bind_track(&mut self, wave_type: WaveType) -> Option<usize> {
        let m = self.free_modules;
        if let Some(idx) = m {
            // take module off the free list
            if let Some(n) = self.modules[idx].next_system {
                self.modules[n].prev_system = self.modules[idx].prev_system;
            }
            if let Some(p) = self.modules[idx].prev_system {
                self.modules[p].next_system = self.modules[idx].next_system;
            } else {
                self.free_modules = self.modules[idx].next_system;
            }

            self.modules[idx].wave_type = wave_type;

            // put module on the used list (sorted next to similar types)
            let mut prevmod: Option<usize> = None;
            let mut nextmod = self.used_modules;
            while let Some(nm) = nextmod {
                if self.modules[nm].wave_type == wave_type {
                    // found start of other shadows using same texture, insert new shadow here.
                    self.modules[idx].next_system = Some(nm);
                    self.modules[idx].prev_system = prevmod;
                    self.modules[nm].prev_system = Some(idx);
                    if let Some(pm) = prevmod {
                        self.modules[pm].next_system = Some(idx);
                    } else {
                        self.used_modules = Some(idx);
                    }
                    break;
                }
                prevmod = Some(nm);
                nextmod = self.modules[nm].next_system;
            }

            if nextmod.is_none() {
                // shadow with new texture. Add to top of list.
                self.modules[idx].next_system = self.used_modules;
                if let Some(u) = self.used_modules {
                    self.modules[u].prev_system = Some(idx);
                }
                self.used_modules = Some(idx);
            }

            self.modules[idx].bound = true;
        }

        // SYNC_WAVES
        let mut nextmod = self.used_modules;
        while let Some(nm) = nextmod {
            self.modules[nm].elapsed_ms = self.modules[nm].init_time_offset;
            nextmod = self.modules[nm].next_system;
        }

        m
    }

    /// `WaterTracksRenderSystem::unbindTrack` (W3DWaterTracks.cpp:487): this object should return to free store as soon
    /// as there is nothing left to render.
    pub fn unbind_track(&mut self, m: usize) {
        self.modules[m].bound = false;
        self.release_track(Some(m));
    }

    /// `WaterTracksRenderSystem::releaseTrack` (W3DWaterTracks.cpp:500): returns a track object to free store to be used
    /// again later.
    pub fn release_track(&mut self, m: Option<usize>) {
        let idx = match m {
            Some(i) => i,
            None => return,
        };

        // assert(mod->m_bound == false);

        // remove module from used list
        if let Some(n) = self.modules[idx].next_system {
            self.modules[n].prev_system = self.modules[idx].prev_system;
        }
        if let Some(p) = self.modules[idx].prev_system {
            self.modules[p].next_system = self.modules[idx].next_system;
        } else {
            self.used_modules = self.modules[idx].next_system;
        }

        // add module to free list
        self.modules[idx].prev_system = None;
        self.modules[idx].next_system = self.free_modules;
        if let Some(f) = self.free_modules {
            self.modules[f].prev_system = Some(idx);
        }
        self.free_modules = Some(idx);
        self.modules[idx].free_water_tracks_resources();
    }

    /// `WaterTracksRenderSystem::ReAcquireResources` (W3DWaterTracks.cpp:550): (re)allocates all W3D assets after a
    /// reset.
    pub fn re_acquire_resources(&mut self) {
        // just for paranoia's sake.
        self.index_buffer = None;
        self.vertex_buffer = None;

        // Will need m_y-1 strips, each of length m_x*2. Will also need 2 extra indices to connect each strip to next one
        // (except last strip). Total index buffer size = (m_y-1)*(m_x*2+2) - 2 (drop the extra 2 indices from last strip)

        let idx_count = (self.strip_size_y - 1) * (self.strip_size_x * 2 + 2) - 2;

        let mut ib = vec![0u16; idx_count as usize];

        // Fill up the IB
        {
            let mut i: i32 = 0;
            let mut j: i32 = 0;
            let mut k: i32 = 0;
            while i < idx_count {
                while k < (self.strip_size_x * (j + 1)) {
                    ib[i as usize] = (k + self.strip_size_x) as u16;
                    ib[(i + 1) as usize] = k as u16;
                    k += 1;
                    i += 2;
                }
                // Generate 4 degenerate triangle to connect current strip to next strip/row of map. To do this, we just
                // repeat the last index of first strip and first index of new strip. Any triangles with repeated
                // vertices will be skipped during rendering.
                if i < idx_count {
                    // check if there is at least 1 more strip to go
                    ib[i as usize] = (k - 1) as u16;
                    ib[(i + 1) as usize] = (k + self.strip_size_x) as u16;
                    i += 2;
                }
                j += 1;
            }
        }
        self.index_buffer = Some(ib);

        // DX8VertexBufferClass(DX8_FVF_XYZDUV1, m_stripSizeX*m_stripSizeY*WATER_VB_PAGES, USAGE_DYNAMIC)
        self.vertex_buffer = Some(vec![VertexFormatXYZDUV1::default(); (self.strip_size_x * self.strip_size_y * WATER_VB_PAGES) as usize]);
        self.batch_start = 0;
    }

    /// `WaterTracksRenderSystem::ReleaseResources` (W3DWaterTracks.cpp:604): releases the dx8 buffers.
    pub fn release_resources(&mut self) {
        self.index_buffer = None;
        self.vertex_buffer = None;
        // Note - it is ok to not release the material, as it is a w3d object that has no dx8 resources. jba.
    }

    /// `WaterTracksRenderSystem::init` (W3DWaterTracks.cpp:616): initialize the system, allocate all the render objects
    /// we will need.
    pub fn init(&mut self, global_data: &GlobalData) {
        const NUM_MODULES: i32 = 2000; // @todo: Get a value out of gdf

        self.strip_size_x = WATER_STRIP_X; // @todo: grab these out of gdf or define
        self.strip_size_y = WATER_STRIP_Y;
        self.level = global_data.water_position_z;

        self.re_acquire_resources();
        // go with a preset material for now: VertexMaterialClass::PRELIT_DIFFUSE
        // use a multi-texture shader: _PresetAlphaShader, Set_Cull_Mode(CULL_MODE_DISABLE)
        self.material_and_shader_set = true;

        // we cannot initialize a system that is already initialized
        if self.free_modules.is_some() || self.used_modules.is_some() {
            // system already online!
            return;
        }

        // allocate our modules for this system
        for _i in 0..NUM_MODULES {
            let mut m = WaterTracksObj::new();
            let idx = self.modules.len();
            m.prev_system = None;
            m.next_system = self.free_modules;
            self.modules.push(m);
            if let Some(f) = self.free_modules {
                self.modules[f].prev_system = Some(idx);
            }
            self.free_modules = Some(idx);
        }
    }

    /// `WaterTracksRenderSystem::reset` (W3DWaterTracks.cpp:675): free all map dependent items.
    pub fn reset(&mut self) {
        // release unbound tracks that may still be fading out
        let mut m = self.used_modules;

        while let Some(idx) = m {
            let next_mod = self.modules[idx].next_system;
            self.modules[idx].bound = false;
            self.release_track(Some(idx));

            m = next_mod;
        }

        // free all attached things and used modules
        // assert( m_usedModules == NULL );
    }

    /// `WaterTracksRenderSystem::shutdown` (W3DWaterTracks.cpp:698): shutdown and free all memory for this system.
    pub fn shutdown(&mut self) {
        // release unbound tracks that may still be fading out
        let mut m = self.used_modules;

        while let Some(idx) = m {
            let next_mod = self.modules[idx].next_system;

            if !self.modules[idx].bound {
                self.release_track(Some(idx));
            }

            m = next_mod;
        }

        // free all attached things and used modules
        // assert( m_usedModules == NULL );

        // free all module storage
        while let Some(f) = self.free_modules {
            let next_mod = self.modules[f].next_system;
            self.free_modules = next_mod;
        }
        self.modules.clear();

        self.index_buffer = None;
        self.material_and_shader_set = false;
        self.vertex_buffer = None;
    }

    /// `WaterTracksRenderSystem::update` (W3DWaterTracks.cpp:737): update the state of all active track marks - fade,
    /// expire, etc. The frame rate is locked to 30 fps (`timeGetTime()` is read upstream but its difference is
    /// overwritten).
    pub fn update(&mut self) {
        let mut m = self.used_modules;

        // Lock framerate to 30 fps
        let time_diff: i32 = (1.0f32 / 30.0f32 * 1000.0f32) as i32;

        // first update all the tracks
        while let Some(idx) = m {
            let next_mod = self.modules[idx].next_system;

            if !self.modules[idx].bound || (self.modules[idx].update(time_diff) == 0 && !self.modules[idx].bound) {
                // object is not longer updating and is unbound so ok to release it.
                self.release_track(Some(idx));
            }

            m = next_mod;
        }
    }

    /// `WaterTracksRenderSystem::flush` (W3DWaterTracks.cpp:775): draw all active track marks for this frame. The
    /// `m_usingWaterTrackEditor` call to `TestWaterUpdate` is not translated (see the module notes). Returns the waves'
    /// draws; the render states, the shroud texture shader and `D3DRS_ZBIAS` set-up around them are device-only.
    pub fn flush(
        &mut self,
        info: &WaterFlushInfo,
        is_underwater: &impl Fn(f32, f32) -> Option<f32>,
    ) -> Vec<WaterTrackFlushDraw> {
        // @todo: Optimize system by drawing tracks as triangle strips and use dynamic vertex buffer access.
        let mut draws = Vec::new();

        if !info.show_soft_water_edge || info.transparent_water_depth == 0.0 {
            return draws;
        }

        self.update(); // update positions of all the tracks

        // rinfo.Camera.Apply() — device-only, not translated

        if self.used_modules.is_none() || info.backface_culling_inverted {
            return draws; // don't render track marks in reflections.
        }

        // According to Nvidia there's a D3D bug that happens if you don't start with a new dynamic VB each frame - so
        // we force a DISCARD by overflowing the counter.
        self.batch_start = 0xffff;

        // The time of day shading (shadeR/G/B, diffuseLight) is computed here upstream but never used by the waves.

        // upstream: Set_Transform(WORLD, identity), Set_Material, Set_Shader, Set_Vertex_Buffer, Set_DX8_Render_State
        // (ZBIAS, 8), Apply_Render_State_Changes, and with a shroud: setTexture/setShader(ST_SHROUD_TEXTURE, 1), stage 1
        // modulate, ZFUNC LESSEQUAL — device-only, not translated

        // LastTextureType is never updated upstream, so the texture is set for every wave.
        let mut vb = self.vertex_buffer.take().unwrap();
        let mut m = self.used_modules;
        while let Some(idx) = m {
            let (verts_rendered, draw) = self.modules[idx].render(&mut vb, self.batch_start, is_underwater);
            draws.push(WaterTrackFlushDraw { texture: self.modules[idx].stage_zero_texture, draw });

            self.batch_start = verts_rendered; // advance past vertices already in buffer

            m = self.modules[idx].next_system;
        } // while (mod)
        self.vertex_buffer = Some(vb);

        // upstream: Set_DX8_Render_State(ZBIAS, 0); with a shroud: ZFUNC EQUAL, resetShader(ST_SHROUD_TEXTURE) — device-only

        draws
    }

    /// `WaterTracksRenderSystem::findTrack` (W3DWaterTracks.cpp:918).
    pub fn find_track(&self, start: &Vector2, end: &Vector2, wave_type: WaveType) -> Option<usize> {
        let mut m = self.used_modules;

        while let Some(idx) = m {
            if self.modules[idx].init_end_pos == *end && self.modules[idx].init_start_pos == *start && self.modules[idx].wave_type == wave_type {
                return Some(idx);
            }
            m = self.modules[idx].next_system;
        } // while (mod)
        None
    }

    /// `WaterTracksRenderSystem::saveTracks` (W3DWaterTracks.cpp:932): save all used tracks. Returns the bytes upstream
    /// `fwrite`s to the `.wak` file next to the map (`TheTerrainLogic->getSourceFilename()` with its last 4 characters
    /// replaced by `.wak`; the caller names and writes the file): per primary wave front the start and end
    /// `Vector2` (8 bytes each) and the `waveType` (4 bytes), then the track count (4 bytes).
    pub fn save_tracks(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut track_count: i32 = 0;

        let mut umod = self.used_modules;
        while let Some(idx) = umod {
            let m = &self.modules[idx];
            if m.init_time_offset == 0 {
                // only save the primary wave front, second layer is added automatically.
                out.extend_from_slice(&m.init_start_pos.x.to_le_bytes());
                out.extend_from_slice(&m.init_start_pos.y.to_le_bytes());
                out.extend_from_slice(&m.init_end_pos.x.to_le_bytes());
                out.extend_from_slice(&m.init_end_pos.y.to_le_bytes());
                out.extend_from_slice(&(m.wave_type as i32).to_le_bytes());
                track_count += 1;
            }
            umod = m.next_system;
        } // end while
        out.extend_from_slice(&track_count.to_le_bytes());
        out
    }

    /// `WaterTracksRenderSystem::loadTracks` (W3DWaterTracks.cpp:978): load tracks from the `.wak` file's bytes (`None`
    /// when `TheFileSystem->openFile` fails). `get_texture(name)` is `WW3DAssetManager::Get_Texture`. A short read
    /// leaves the variable as it was, as `File::read` does.
    pub fn load_tracks(&mut self, file: Option<&[u8]>, get_texture: &mut impl FnMut(&[u8]) -> Option<TextureHandle>) {
        let file = match file {
            Some(f) => f,
            None => return,
        };
        let mut flip_u: i32 = 0;
        let mut start_pos = Vector2::default();
        let mut end_pos = Vector2::default();
        let mut wtype: WaveType = WaveType::Pond;
        let mut track_count: i32 = 0;

        // file->seek(-4, END); read the track count
        if file.len() >= 4 {
            let t = &file[file.len() - 4..];
            track_count = i32::from_le_bytes([t[0], t[1], t[2], t[3]]);
        }
        let mut pos: usize = 0; // file->seek(0, START)
        let mut read_f32 = |pos: &mut usize, v: &mut f32| {
            if *pos + 4 <= file.len() {
                *v = f32::from_le_bytes([file[*pos], file[*pos + 1], file[*pos + 2], file[*pos + 3]]);
                *pos += 4;
            } else {
                *pos = file.len();
            }
        };
        let mut i: i32 = 0;
        while i < track_count {
            loop {
                // tryagain:
                read_f32(&mut pos, &mut start_pos.x);
                read_f32(&mut pos, &mut start_pos.y);
                read_f32(&mut pos, &mut end_pos.x);
                read_f32(&mut pos, &mut end_pos.y);
                let mut w: f32 = f32::from_bits(wtype as i32 as u32);
                read_f32(&mut pos, &mut w);
                wtype = WaveType::from_i32(w.to_bits() as i32);
                // Check if this track already exists.
                if self.find_track(&start_pos, &end_pos, wtype).is_some() {
                    i += 1;
                    continue; // goto tryagain
                }
                break;
            }

            let umod = self.bind_track(wtype);
            if let Some(u) = umod {
                // umod->init(1.5f*MAP_XY_FACTOR,Vector2(0,0),Vector2(1,1),"wave256.tga");
                flip_u ^= 1; // toggle flip state
                let info = wave_info(wtype);
                let tex = get_texture(info.texture_name);
                self.modules[u].init(info.final_height, info.final_width, start_pos, end_pos, tex, 0);
                self.modules[u].flip_u = flip_u as f32;

                if info.second_wave_time_offset != 0 {
                    // check if we need a second wave to follow
                    let umod2 = self.bind_track(wtype);
                    if let Some(u2) = umod2 {
                        let tex = get_texture(info.texture_name);
                        self.modules[u2].init(info.final_height, info.final_width, start_pos, end_pos, tex, info.second_wave_time_offset);
                        self.modules[u2].flip_u = if flip_u == 0 { 1.0 } else { 0.0 };
                    }
                }
            }
            i += 1;
        }
        // file->close()
        // (the `#if 0` obsolete waypoint code is not compiled)
    }
}
