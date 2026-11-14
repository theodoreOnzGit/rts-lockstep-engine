// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/part_buf.cpp, part_buf.h
//   (NewParticleStruct; ParticleBufferClass: the constructor, Reset_Colors,
//   Reset_Opacity, Reset_Size, Reset_Rotations, Reset_Frames,
//   Reset_Blur_Times, Scale, Emitter_Is_Dead, Set_Emitter, Is_Complete,
//   Add_Uninitialized_New_Particle, Get_Particle_Count, Render,
//   Generate_APT, Combine_Color_And_Alpha, Render_Particles, Render_Line,
//   Render_Line_Group, Determine_Tail_Diffuse, Update_Kinematic_Particle_State,
//   Update_Visual_Particle_State, Update_Bounding_Box, Get_New_Particles,
//   Kill_Old_Particles, Update_Non_New_Particles, Set_Acceleration, the
//   inline accessors; PermutationArray, MAX_RANDOM_ENTRIES; (#65) Prepare_LOD,
//   Increment_LOD, Decrement_LOD, Get_Cost, Get_Value,
//   Get_Post_Increment_Value, Set_LOD_Level, Get_LOD_Level, Get_LOD_Count,
//   Set_LOD_Bias, Calculate_Cost_Value_Arrays, Set_LOD_Max_Screen_Size,
//   Get_LOD_Max_Screen_Size, LODMaxScreenSizes, On_Frame_Update,
//   Get_Obj_Space_Bounding_Sphere, Get_Obj_Space_Bounding_Box)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.h, rendobj.cpp
//   (#65: RenderObjClass::Get_Bounding_Sphere, Update_Cached_Bounding_Volumes,
//   Set_Hidden, Set_Visible, Set_Animation_Hidden, Is_Not_Hidden_At_All,
//   AT_MIN_LOD, AT_MAX_LOD)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/predlod.cpp
//   (#65: PredictiveLODOptimizerClass::Add_Object's cost, as Prepare_LOD
//   hands it over)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vp.cpp
//   (VectorProcessorClass::Copy (the three Vector4 overloads) and Clamp)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/pot.cpp (Find_POT)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aabox.h
//   (AABoxClass::Init(const MinMaxAABoxClass &))
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, part_buf by Naty Hoffman and others)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ParticleBufferClass` (GitHub #49): the particles one W3D emitter
//! ([`super::part_emt`]) has emitted, their motion over W3D sync time, their
//! keyframed colour, opacity, size, orientation, frame and blur time, and
//! what each render mode hands to its renderer.
//!
//! # As upstream does it
//!
//! - **Keyframes** (`Reset_*`): each property's keyframes become millisecond
//!   times (`(unsigned int)(time * 1000)`), stopping at the first time at or
//!   past `MaxAge`, with the start value as keyframe 0 at time 0 and a delta
//!   (value per millisecond) for each segment; the last delta runs to the
//!   next keyframe if one was cut, else 0. A property with no keyframes and
//!   a random range under its epsilon is a constant: no per-particle array.
//!   Rotations are per millisecond and integrated into orientations at the
//!   keyframes. A non-zero random range fills a table of
//!   `min(Find_POT(MaxNum), 32)` entries from the buffer's `Random4Class`
//!   (`rand_gen * range / INT_MAX`), indexed by the particle's slot.
//! - **Motion** (`Update_Kinematic_Particle_State`, once per sync time): the
//!   new-particle queue's particles are placed at their state at the current
//!   time (`p + (v + a t / 2) t`), dropping those already past `MaxAge`;
//!   particles past `MaxAge` are killed from the start of the ring; the
//!   others advance by the elapsed time.
//! - **Visual state** (`Update_Visual_Particle_State`, at render): each
//!   property at the particle's age, walking keyframes backwards from the
//!   oldest particle to the youngest, plus the slot's random entry; size
//!   clamped at 0; orientation `(int)(o * 256) & 0xFF`; frame `(int)f & 0xFF`.
//! - **Rendering**: triangle and quad particles go to a point group
//!   ([`PointGroupDraw`]); line mode to the segment-line renderer as runs of
//!   the same group ID, the emitter's position closing the newest run
//!   ([`LineRun`]); the line-group modes to a line group with tail
//!   positions ([`LineGroupDraw`]). Colour and alpha are combined and
//!   clamped to 0..1 first (`Combine_Color_And_Alpha`).
//!
//! # Upstream quirks that are kept
//!
//! - `FutureStartTime` is stored and never used (`Get_Future_Start_Time`
//!   only): an emitter's future start time does not delay or pre-age it.
//! - `Get_New_Particles` writes a skipped (too old) particle's time stamp
//!   into the slot it would have taken before skipping it.
//! - `VectorProcessorClass::Clamp` is called in place: its second
//!   assignment of each component reads the first's result, so it clamps
//!   both ends (with distinct arrays it would only clamp the top).
//! - The keyframe walk only goes backwards: it assumes ages fall along the
//!   ring.
//! - The line-group defaults test `Frame`, not `UCoord`, before setting the
//!   default u coordinate (always set: line groups have no `Frame`).
//! - The colour randomizer table's three draws are the arguments of one
//!   `Vector3` constructor: their order is unspecified in C++. They are
//!   taken left to right, as the compiler of the differential tests does
//!   (MSVC may take them right to left).
//!
//! # Where this differs from upstream
//!
//! - ~~**Not modelled**: the predictive LOD (`Prepare_LOD`, the cost and
//!   value arrays): the buffer keeps the decimation threshold the
//!   constructor leaves with the default screen-size clamps, 0 (every
//!   particle drawn);~~ **Corrected 2026-11-14 (#65, batch b):** the
//!   predictive LOD is translated ([`ParticleBuffer::prepare_lod`],
//!   [`ParticleBuffer::calculate_cost_value_arrays`], the cost and value
//!   arrays, the LOD level and bias); the static `LODMaxScreenSizes` is
//!   [`LodMaxScreenSizes`], which the caller holds; the predictive LOD
//!   optimizer's object list is the caller's (`prepare_lod` returns the cost
//!   it adds). Zero Hour never drives it (see [`LodMaxScreenSizes`]), so the
//!   threshold stays 0 in the game. **Not modelled:** the static sort lists (`Render` draws directly); the
//!   scene (the caller drives the frame: [`super::part_emt`]).
//! - **Undefined upstream** ([`W3dUndefined`]): a float converted to an
//!   integer that cannot hold it (key times, `MaxAge`, orientations,
//!   frames: [`W3dUndefined::FloatToInt`]); a buffer the 32-bit original
//!   cannot allocate ([`W3dUndefined::Allocation`]); (#65) a LOD array read
//!   or a screen size written past its end (`Set_LOD_Level(17)` then
//!   `Get_Cost`, `Decrement_LOD` to 17, `Set_LOD_Max_Screen_Size(17, ..)`:
//!   [`W3dUndefined::IndexOutOfRange`]).
//! - The draws are returned as data ([`ParticleDraw`]); ~~the point group,
//!   segment-line renderer and line group that draw them are the client's.~~
//!   **Corrected 2026-11-14 (#65):** the point group and line group that
//!   draw them are the client's; the segment-line renderer is translated
//!   ([`super::seglinerenderer`], kept in the buffer), and the line runs go
//!   through it in [`ParticleBuffer::render_line_runs`], which the caller
//!   calls after `render` with the camera (`Render` has none here).

use super::shader::{ShaderClass, DSTBLEND_ONE, DSTBLEND_ONE_MINUS_SRC_ALPHA, DSTBLEND_ONE_MINUS_SRC_COLOR, DSTBLEND_SRC_COLOR};
use super::shader::{SRCBLEND_ONE, SRCBLEND_SRC_ALPHA};
use super::seglinerenderer::{SegLineChunk, SegLineRenderer};
use super::w3d_file::W3dEmitterLinePropertiesStruct;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::random::Random3;
use super::{W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::random::Random4;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;
use crate::ww::wwmath;

/// `W3D_EMITTER_RENDER_MODE_*` (w3d_file.h).
pub const W3D_EMITTER_RENDER_MODE_TRI_PARTICLES: i32 = 0;
pub const W3D_EMITTER_RENDER_MODE_QUAD_PARTICLES: i32 = 1;
pub const W3D_EMITTER_RENDER_MODE_LINE: i32 = 2;
pub const W3D_EMITTER_RENDER_MODE_LINEGRP_TETRA: i32 = 3;
pub const W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM: i32 = 4;

/// `RenderObjClass::AT_MIN_LOD` (rendobj.cpp: `FLT_MAX`).
pub const AT_MIN_LOD: f32 = f32::MAX;
/// `RenderObjClass::AT_MAX_LOD` (rendobj.cpp: `-1.0f`).
pub const AT_MAX_LOD: f32 = -1.0;
/// The number of LOD levels a buffer has (`LodCount = 17` in the
/// constructor; the sizes of `Cost[17]`, `Value[18]` and
/// `LODMaxScreenSizes[17]`).
pub const PARTICLE_LOD_COUNT: u32 = 17;

/// `ParticleBufferClass::LODMaxScreenSizes[17]`, a static upstream (the
/// screen-size clamps of the 17 LOD levels, all `NO_MAX_SCREEN_SIZE` at
/// start), held by the caller here. **Zero Hour never sets them** (no caller
/// of `Set_LOD_Max_Screen_Size` outside WW3D2), so with them
/// `Calculate_Cost_Value_Arrays` gives a minimum LOD of 0 (16 only for an
/// infinite screen area) and never raises a buffer's LOD.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodMaxScreenSizes(pub [f32; 17]);

impl Default for LodMaxScreenSizes {
    fn default() -> Self {
        LodMaxScreenSizes([super::w3d_file::NO_MAX_SCREEN_SIZE; 17])
    }
}

impl LodMaxScreenSizes {
    /// `ParticleBufferClass::Set_LOD_Max_Screen_Size(lod_level, max_screen_size)`:
    /// levels outside 0..17 are ignored; level 17 passes the test and writes
    /// past the array's end (undefined).
    pub fn set(&mut self, lod_level: i32, max_screen_size: f32) -> W3dResult<()> {
        if !(0..=17).contains(&lod_level) {
            return Ok(());
        }
        if lod_level == 17 {
            return Err(W3dUndefined::IndexOutOfRange.into());
        }
        self.0[lod_level as usize] = max_screen_size;
        Ok(())
    }

    /// `ParticleBufferClass::Get_LOD_Max_Screen_Size(lod_level)`: outside
    /// 0..17 `NO_MAX_SCREEN_SIZE`; level 17 reads past the end (undefined).
    pub fn get(&self, lod_level: i32) -> W3dResult<f32> {
        if !(0..=17).contains(&lod_level) {
            return Ok(super::w3d_file::NO_MAX_SCREEN_SIZE);
        }
        self.0.get(lod_level as usize).copied().ok_or(W3dUndefined::IndexOutOfRange.into())
    }
}

/// What `Prepare_LOD` reads of its `CameraClass`: `Get_Position()`, the
/// viewport (`Get_Viewport()`, whose `Width()` and `Height()` are `Max -
/// Min`) and the view plane (`Get_View_Plane(min, max)`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LodCamera {
    pub position: Vector3,
    pub viewport_min: Vector2,
    pub viewport_max: Vector2,
    pub view_plane_min: Vector2,
    pub view_plane_max: Vector2,
}

/// `SphereClass` (centre and radius).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sphere {
    pub center: Vector3,
    pub radius: f32,
}

/// `ParticleBufferClass::PermutationArray` (LOD decimation).
pub const PERMUTATION_ARRAY: [u32; 16] = [11, 3, 7, 14, 0, 13, 1, 2, 5, 12, 15, 6, 9, 8, 4, 10];
/// `MAX_RANDOM_ENTRIES`.
const MAX_RANDOM_ENTRIES: u32 = 32;
/// `oo_intmax`: `1.0f / (float)INT_MAX` (2^-31).
const OO_INTMAX: f32 = 1.0 / 2_147_483_648.0;
/// `sizeof(NewParticleStruct)` on Win32 (two `Vector3`s, a time stamp, a
/// group ID, padded).
const NEW_PARTICLE_SIZE: u64 = 32;

/// The WW3D clock the particle code reads: `WW3D::Get_Sync_Time()`,
/// `Get_Frame_Time()` and `Get_Frame_Count()` (process globals upstream).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ww3dClock {
    /// `SyncTime`, milliseconds.
    pub sync_time: u32,
    /// `SyncTime - PreviousSyncTime`.
    pub frame_time: u32,
    /// `FrameCount`.
    pub frame_count: u32,
}

/// `(unsigned int)f`: defined when the truncation fits.
pub(crate) fn f2u(f: f32) -> W3dResult<u32> {
    if f > -1.0f32 && f < 4_294_967_296.0f32 {
        Ok(f as u32)
    } else {
        Err(W3dUndefined::FloatToInt.into())
    }
}

/// `(int)f`: defined when the truncation fits.
pub(crate) fn f2i(f: f32) -> W3dResult<i32> {
    if f > -2_147_483_904.0f32 && f < 2_147_483_648.0f32 {
        Ok(f as i32)
    } else {
        Err(W3dUndefined::FloatToInt.into())
    }
}

/// `Find_POT(val)` (pot.cpp): the power of two at or above `val`.
pub fn find_pot(mut val: i32) -> i32 {
    let mut recpos = 0;
    let mut reccnt = 0;
    let mut lp = 0;
    while val != 0 {
        if val & 1 != 0 {
            recpos = lp;
            reccnt += 1;
        }
        val >>= 1;
        lp += 1;
        if lp > 32 {
            // a negative value never reaches 0 under the arithmetic shift
            break;
        }
    }
    if reccnt < 2 {
        return 1i32.wrapping_shl(recpos);
    }
    1i32.wrapping_shl(recpos + 1)
}

/// `struct NewParticleStruct`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NewParticle {
    /// `Position`, world space.
    pub position: Vector3,
    /// `Velocity`, world units per millisecond.
    pub velocity: Vector3,
    /// `TimeStamp`, milliseconds.
    pub time_stamp: u32,
    /// `GroupID`.
    pub group_id: u8,
}

/// A `ParticlePropertyStruct<T>` as `Copy_Emitter_Property_Struct` hands it
/// to the buffer: the start value, random range and the keyframes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Props<T> {
    pub start: T,
    pub rand: T,
    pub key_times: Vec<f32>,
    pub values: Vec<T>,
}

impl<T> Props<T> {
    /// `NumKeyFrames`.
    pub fn num_key_frames(&self) -> u32 {
        self.key_times.len() as u32
    }
}

/// The arithmetic the keyframe code does on a property's value type.
pub trait KeyValue: Copy + Default {
    fn sub(self, o: Self) -> Self;
    fn add(self, o: Self) -> Self;
    fn mul(self, k: f32) -> Self;
    /// `operator /` (`Vector3`'s multiplies by `1.0f / k`).
    fn div(self, k: f32) -> Self;
}

impl KeyValue for f32 {
    fn sub(self, o: f32) -> f32 {
        self - o
    }
    fn add(self, o: f32) -> f32 {
        self + o
    }
    fn mul(self, k: f32) -> f32 {
        self * k
    }
    fn div(self, k: f32) -> f32 {
        self / k
    }
}

impl KeyValue for Vector3 {
    fn sub(self, o: Vector3) -> Vector3 {
        self - o
    }
    fn add(self, o: Vector3) -> Vector3 {
        self + o
    }
    fn mul(self, k: f32) -> Vector3 {
        self * k
    }
    fn div(self, k: f32) -> Vector3 {
        self / k
    }
}

/// One property's keyframes (`Num*KeyFrames`, `*KeyFrameTimes`,
/// `*KeyFrameValues`, `*KeyFrameDeltas`). `None` for a null pointer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyTrack<T> {
    pub num: u32,
    pub times: Option<Vec<u32>>,
    pub values: Option<Vec<T>>,
    pub deltas: Option<Vec<T>>,
}

/// A randomizer table (`NumRandom*EntriesMinus1`, `Random*Entries`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RandomTable<T> {
    pub minus1: u32,
    pub entries: Option<Vec<T>>,
}

/// The rotation keyframes (`NumRotationKeyFrames`, `RotationKeyFrameTimes`,
/// `RotationKeyFrameValues` (per millisecond), `HalfRotationKeyFrameDeltas`,
/// `OrientationKeyFrameValues`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RotationTrack {
    pub num: u32,
    pub times: Option<Vec<u32>>,
    pub values: Option<Vec<f32>>,
    pub half_deltas: Option<Vec<f32>>,
    pub orientations: Option<Vec<f32>>,
}

/// `AABoxClass` (center and extent).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AaBox {
    pub center: Vector3,
    pub extent: Vector3,
}

/// What a triangle or quad buffer hands its point group in
/// `Render_Particles` (`Set_Arrays` and the defaults), resolved per point.
#[derive(Clone, Debug, PartialEq)]
pub struct PointGroupDraw {
    /// `PointGroupClass::QUADS` (else `TRIS`).
    pub quads: bool,
    /// `Set_Frame_Row_Column_Count_Log2(frame_mode)` (`MIN(frccl2, 4)`).
    pub frame_row_column_count_log2: u8,
    pub points: Vec<PointSprite>,
}

/// One active point: its slot, and each array's entry or the default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointSprite {
    pub index: u32,
    pub position: Vector3,
    /// The combined colour and alpha (`Diffuse`), or the defaults
    /// (`Set_Point_Color`, `Set_Point_Alpha`) when there is no array.
    pub diffuse: Vector4,
    pub size: f32,
    pub orientation: u8,
    pub frame: u8,
}

/// One run `Render_Line` hands `SegLineRendererClass::Render`: points of one
/// group ID, in queue order, with their colours.
#[derive(Clone, Debug, PartialEq)]
pub struct LineRun {
    pub points: Vec<Vector3>,
    pub colors: Vec<Vector4>,
}

/// What a line-group buffer hands its line group in `Render_Line_Group`,
/// resolved per line.
#[derive(Clone, Debug, PartialEq)]
pub struct LineGroupDraw {
    /// `LineGroupClass::PRISM` (else `TETRAHEDRON`).
    pub prism: bool,
    pub lines: Vec<GroupLine>,
}

/// One line of a line group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupLine {
    pub index: u32,
    pub head: Vector3,
    pub tail: Vector3,
    pub head_diffuse: Vector4,
    pub tail_diffuse: Vector4,
    pub size: f32,
    pub ucoord: f32,
}

/// What one `Render` draws.
#[derive(Clone, Debug, PartialEq)]
pub enum ParticleDraw {
    /// Triangle or quad particles.
    Points(PointGroupDraw),
    /// Line mode: the runs, and the line's width (`Set_Width`).
    Lines { width: f32, runs: Vec<LineRun> },
    /// Tetrahedron or prism line groups.
    LineGroup(LineGroupDraw),
    /// A render mode the constructor did not know (no renderer).
    Nothing,
}

/// `TailDiffuseTypeEnum`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TailDiffuseType {
    Black,
    White,
    SameAsHead,
    SameAsHeadAlphaZero,
}

/// `class ParticleBufferClass`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleBuffer {
    // the new-particle queue
    pub new_particle_queue: Vec<NewParticle>,
    pub new_particle_queue_start: u32,
    pub new_particle_queue_end: u32,
    pub new_particle_queue_count: i32,
    // state global to the buffer
    pub render_mode: i32,
    pub frame_mode: i32,
    /// `Accel`: world units per ms^2.
    pub accel: Vector3,
    pub has_accel: bool,
    /// `MaxAge`, ms.
    pub max_age: u32,
    /// `FutureStartTime`, ms (never used).
    pub future_start_time: u32,
    pub last_update_time: u32,
    pub is_emitter_dead: bool,
    pub max_size: f32,
    // the ring
    pub max_num: u32,
    pub start: u32,
    pub end: u32,
    pub new_end: u32,
    pub non_new_num: i32,
    pub new_num: i32,
    pub bounding_box: AaBox,
    pub bounding_box_dirty: bool,
    // keyframes
    pub color_keys: KeyTrack<Vector3>,
    pub alpha_keys: KeyTrack<f32>,
    pub size_keys: KeyTrack<f32>,
    pub rotation_keys: RotationTrack,
    pub frame_keys: KeyTrack<f32>,
    pub blur_time_keys: KeyTrack<f32>,
    pub default_tail_diffuse: Vector4,
    // randomizer tables
    pub random_color: RandomTable<Vector3>,
    pub random_alpha: RandomTable<f32>,
    pub random_size: RandomTable<f32>,
    pub random_rotation: RandomTable<f32>,
    pub random_orientation: RandomTable<f32>,
    pub random_frame: RandomTable<f32>,
    pub random_blur_time: RandomTable<f32>,
    pub color_random: Vector3,
    pub opacity_random: f32,
    pub size_random: f32,
    pub rotation_random: f32,
    pub frame_random: f32,
    pub blur_time_random: f32,
    pub initial_orientation_random: f32,
    /// The renderer made for the render mode: `PointGroup` (triangle or
    /// quad), `LineRenderer` or `LineGroup`; none for an unknown mode.
    pub has_point_group: bool,
    pub has_line_renderer: bool,
    pub has_line_group: bool,
    /// ~~`LineRenderer->Init(*line_props)`; its width (`Set_Width`).~~
    /// **Corrected 2026-11-14 (#65):** the `SegLineRendererClass` itself
    /// ([`SegLineRenderer`]: `Init(*line_props)`, `Set_Texture`, `Set_Shader`,
    /// `Set_Width(Get_Particle_Size())`), whose width, noise and UV scroll
    /// state the buffer keeps; [`ParticleBuffer::render_line_runs`] draws
    /// [`ParticleDraw::Lines`]' runs with it.
    pub line_renderer: Option<SegLineRenderer>,
    /// The renderer's texture (`Set_Texture`): its name, `None` for null.
    pub texture: Option<Vec<u8>>,
    /// The renderer's shader (`Set_Shader`, after `Enable_Fog`).
    pub shader: ShaderClass,
    // per-particle arrays (ShareBufferClass; `None` for null)
    pub position: [Option<Vec<Vector3>>; 2],
    pub diffuse: Option<Vec<Vector4>>,
    pub color: Option<Vec<Vector3>>,
    pub alpha: Option<Vec<f32>>,
    pub size: Option<Vec<f32>>,
    pub frame: Option<Vec<u8>>,
    pub ucoord: Option<Vec<f32>>,
    pub tail_position: Option<Vec<Vector3>>,
    pub tail_diffuse: Option<Vec<Vector4>>,
    pub orientation: Option<Vec<u8>>,
    pub apt: Vec<u32>,
    pub group_id: Vec<u8>,
    pub ping_pong_position: bool,
    pub velocity: Vec<Vector3>,
    pub time_stamp: Vec<u32>,
    /// `Emitter != NULL`.
    pub has_emitter: bool,
    pub decimation_threshold: u32,
    pub lod_count: u32,
    pub current_group_id: u8,
    /// `Cost[17]`: the cost of each LOD level (#65).
    pub cost: [f32; 17],
    /// `Value[18]`: the value of each LOD level, and the post-increment
    /// sentinel (#65).
    pub value: [f32; 18],
    /// `LodBias`.
    pub lod_bias: f32,
    /// `ProjectedArea`, filtered over the `Prepare_LOD` calls.
    pub projected_area: f32,
    /// `RenderObjClass`'s cached bounding sphere (`CachedBoundingSphere`) and
    /// `BOUNDING_VOLUMES_VALID` (#65; `Get_Bounding_Sphere` reads it,
    /// `On_Frame_Update` invalidates it). The cached box is not kept: nothing
    /// translated reads it.
    pub cached_bounding_sphere: Sphere,
    pub bounding_volumes_valid: bool,
    /// The buffer's own `RenderObjClass` visibility bits (`IS_VISIBLE`,
    /// `IS_NOT_HIDDEN`, `IS_NOT_ANIMATION_HIDDEN`; `DEFAULT_BITS` leaves it
    /// not visible and not hidden). `Prepare_LOD` tests them.
    pub visible: bool,
    pub hidden: bool,
    pub animation_hidden: bool,
}

/// The constructor's arguments (beyond the emitter and the randomizer).
#[derive(Clone, Debug, PartialEq)]
pub struct BufferParams {
    pub buffer_size: u32,
    pub color: Props<Vector3>,
    pub opacity: Props<f32>,
    pub size: Props<f32>,
    pub rotation: Props<f32>,
    pub orient_rnd: f32,
    pub frame: Props<f32>,
    pub blurtime: Props<f32>,
    pub accel: Vector3,
    pub max_age: f32,
    pub future_start: f32,
    pub texture: Option<Vec<u8>>,
    pub shader: ShaderClass,
    pub pingpong: bool,
    pub render_mode: i32,
    pub frame_mode: i32,
    pub line_props: Option<W3dEmitterLinePropertiesStruct>,
}

/// The ring's two subranges (`sub1_end`, `sub2_start`), as every loop finds
/// them.
fn subranges(start: u32, end: u32, non_new_num: i32, max_num: u32) -> (u32, u32) {
    if start < end || (start == end && non_new_num == 0) {
        (end, end)
    } else {
        (max_num, 0)
    }
}

/// `shader.Enable_Fog("ParticleBufferClass")` on a copy.
fn enable_fog(mut shader: ShaderClass) -> ShaderClass {
    shader.enable_fog();
    shader
}

impl ParticleBuffer {
    /// `ParticleBufferClass(emitter, buffer_size, ...)` at sync time
    /// `clock.sync_time`, drawing its randomizer tables from `rand_gen`.
    /// `lod_sizes` is the static `LODMaxScreenSizes` (#65).
    pub fn new(p: &BufferParams, rand_gen: &mut Random4, clock: &Ww3dClock, lod_sizes: &LodMaxScreenSizes) -> W3dResult<ParticleBuffer> {
        let max_num = p.buffer_size;
        if u64::from(max_num) * NEW_PARTICLE_SIZE > WIN32_MAX_ALLOCATION {
            return Err(W3dUndefined::Allocation.into());
        }
        let n = max_num as usize;
        let mut b = ParticleBuffer {
            new_particle_queue: Vec::new(),
            new_particle_queue_start: 0,
            new_particle_queue_end: 0,
            new_particle_queue_count: 0,
            render_mode: p.render_mode,
            frame_mode: p.frame_mode,
            accel: Vector3::default(),
            has_accel: false,
            max_age: f2u(1000.0f32 * p.max_age)?,
            future_start_time: f2u(1000.0f32 * p.future_start)?,
            last_update_time: clock.sync_time,
            is_emitter_dead: false,
            max_size: 0.0,
            max_num,
            start: 0,
            end: 0,
            new_end: 0,
            non_new_num: 0,
            new_num: 0,
            bounding_box: AaBox::default(),
            bounding_box_dirty: true,
            color_keys: KeyTrack::default(),
            alpha_keys: KeyTrack::default(),
            size_keys: KeyTrack::default(),
            rotation_keys: RotationTrack::default(),
            frame_keys: KeyTrack::default(),
            blur_time_keys: KeyTrack::default(),
            default_tail_diffuse: Vector4::new(0.0, 0.0, 0.0, 0.0),
            random_color: RandomTable::default(),
            random_alpha: RandomTable::default(),
            random_size: RandomTable::default(),
            random_rotation: RandomTable::default(),
            random_orientation: RandomTable::default(),
            random_frame: RandomTable::default(),
            random_blur_time: RandomTable::default(),
            color_random: Vector3::default(),
            opacity_random: 0.0,
            size_random: 0.0,
            rotation_random: 0.0,
            frame_random: 0.0,
            blur_time_random: 0.0,
            initial_orientation_random: 0.0,
            has_point_group: false,
            has_line_renderer: false,
            has_line_group: false,
            line_renderer: None,
            texture: None,
            shader: p.shader,
            position: [None, None],
            diffuse: None,
            color: None,
            alpha: None,
            size: None,
            frame: None,
            ucoord: None,
            tail_position: None,
            tail_diffuse: None,
            orientation: None,
            apt: Vec::new(),
            group_id: Vec::new(),
            ping_pong_position: p.pingpong,
            velocity: Vec::new(),
            time_stamp: Vec::new(),
            has_emitter: true,
            decimation_threshold: 0,
            lod_count: PARTICLE_LOD_COUNT,
            current_group_id: 0,
            cost: [0.0; 17],
            value: [0.0; 18],
            lod_bias: 1.0,
            projected_area: 0.0,
            cached_bounding_sphere: Sphere::default(),
            bounding_volumes_valid: false,
            visible: false,
            hidden: false,
            animation_hidden: false,
        };
        b.reset_colors(&p.color, rand_gen)?;
        b.reset_opacity(&p.opacity, rand_gen)?;
        b.reset_size(&p.size, rand_gen)?;
        b.reset_rotations(&p.rotation, p.orient_rnd, rand_gen)?;
        b.reset_frames(&p.frame, rand_gen)?;
        b.reset_blur_times(&p.blurtime, rand_gen)?;
        // Set up new particle queue:
        b.new_particle_queue = vec![NewParticle::default(); n];
        b.accel = p.accel;
        b.has_accel = p.accel.x != 0.0f32 || p.accel.y != 0.0f32 || p.accel.z != 0.0f32;
        let shader = enable_fog(p.shader);
        b.shader = shader;
        match b.render_mode {
            W3D_EMITTER_RENDER_MODE_TRI_PARTICLES | W3D_EMITTER_RENDER_MODE_QUAD_PARTICLES => {
                b.has_point_group = true;
                b.texture = p.texture.clone();
            }
            W3D_EMITTER_RENDER_MODE_LINE => {
                // (Init(*line_props) before the null test; the second renderer below replaces this one)
                b.has_line_renderer = true;
                b.texture = p.texture.clone();
                b.line_renderer = Some(b.new_line_renderer(p.line_props.as_ref(), &shader, clock));
            }
            W3D_EMITTER_RENDER_MODE_LINEGRP_TETRA | W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM => {
                b.has_line_group = true;
                b.texture = p.texture.clone();
                b.tail_position = Some(vec![Vector3::default(); n]);
            }
            _ => {}
        }
        b.position[0] = Some(vec![Vector3::default(); n]);
        if b.ping_pong_position {
            b.position[1] = Some(vec![Vector3::default(); n]);
        }
        b.apt = vec![0; n];
        b.group_id = vec![0; n];
        b.velocity = vec![Vector3::default(); n];
        b.time_stamp = vec![0; n];
        // ~~(Calculate_Cost_Value_Arrays(1.0f): with the default screen-size clamps the minimum LOD is 0 and the
        // decimation threshold stays 0)~~ Corrected 2026-11-14 (#65, batch b): translated. So that the object is ready
        // for use after construction, its cost and value arrays for a screen area of 1:
        let (mut value, mut cost) = ([0.0f32; 18], [0.0f32; 17]);
        let minlod = b.calculate_cost_value_arrays(1.0f32, lod_sizes, &mut value, &mut cost);
        (b.value, b.cost) = (value, cost);
        // Ensure lod is no less than minimum allowed
        if b.get_lod_level() < minlod {
            b.set_lod_level(minlod);
        }
        if b.render_mode == W3D_EMITTER_RENDER_MODE_LINE {
            if let Some(lp) = p.line_props.as_ref() {
                b.has_line_renderer = true;
                b.line_renderer = Some(b.new_line_renderer(Some(lp), &shader, clock));
            } else {
                // in line mode without line properties, drop back to triangles
                b.render_mode = W3D_EMITTER_RENDER_MODE_TRI_PARTICLES;
            }
        }
        Ok(b)
    }

    /// `Get_Particle_Size()`: `SizeKeyFrameValues[0]`.
    pub fn get_particle_size(&self) -> f32 {
        self.size_keys.values.as_ref().map_or(0.0, |v| v[0])
    }

    /// `Get_Lifetime()`.
    pub fn get_lifetime(&self) -> f32 {
        self.max_age as f32 / 1000.0f32
    }

    /// `Get_Future_Start_Time()`.
    pub fn get_future_start_time(&self) -> f32 {
        self.future_start_time as f32 / 1000.0f32
    }

    /// `Get_Buffer_Size()`.
    pub fn get_buffer_size(&self) -> u32 {
        self.max_num
    }

    /// `Get_Particle_Count()`.
    pub fn get_particle_count(&self) -> i32 {
        self.non_new_num + self.new_num
    }

    /// `Is_Complete()`.
    pub fn is_complete(&self) -> bool {
        self.is_emitter_dead && self.non_new_num == 0 && self.new_num == 0
    }

    /// `Emitter_Is_Dead()`.
    pub fn emitter_is_dead(&mut self) {
        self.is_emitter_dead = true;
        self.has_emitter = false;
    }

    /// `Set_Acceleration(acceleration)`.
    pub fn set_acceleration(&mut self, acceleration: Vector3) {
        self.accel = acceleration;
        self.has_accel = self.accel.x != 0.0 || self.accel.y != 0.0 || self.accel.z != 0.0;
    }

    /// `Set_Current_GroupID(grp)`.
    pub fn set_current_group_id(&mut self, grp: u8) {
        self.current_group_id = grp;
    }

    /// `RenderObjClass::Set_Hidden(onoff)` (the buffer's own bit).
    pub fn set_hidden(&mut self, onoff: bool) {
        self.hidden = onoff;
    }

    /// `RenderObjClass::Set_Visible(onoff)` (the buffer's own bit).
    pub fn set_visible(&mut self, onoff: bool) {
        self.visible = onoff;
    }

    /// `RenderObjClass::Set_Animation_Hidden(onoff)` (the buffer's own bit).
    pub fn set_animation_hidden(&mut self, onoff: bool) {
        self.animation_hidden = onoff;
    }

    /// `RenderObjClass::Is_Not_Hidden_At_All()`: `IS_NOT_HIDDEN |
    /// IS_NOT_ANIMATION_HIDDEN` (not `IS_VISIBLE`).
    pub fn is_not_hidden_at_all(&self) -> bool {
        !self.hidden && !self.animation_hidden
    }

    /// `Calculate_Cost_Value_Arrays(screen_area, values, costs)` with the
    /// static screen sizes `sizes`: each level's cost (the particles it
    /// draws, `lod / 16` of the buffer, times the polygons per particle),
    /// each level's value (`AT_MIN_LOD` up to the minimum level, then the
    /// benefit per cost), `values[LodCount]` the `AT_MAX_LOD` sentinel; the
    /// minimum level.
    pub fn calculate_cost_value_arrays(&self, screen_area: f32, sizes: &LodMaxScreenSizes, values: &mut [f32; 18], costs: &mut [f32; 17]) -> i32 {
        let lod_count = self.lod_count;
        // Calculate Cost heuristic for each LOD (we currently ignore pixel costs for particle systems). The cost
        // factor is based on a 1/16 (0.0625) of the total.
        let mut cost_factor = 0.0f32;
        match self.render_mode {
            W3D_EMITTER_RENDER_MODE_TRI_PARTICLES => cost_factor = self.max_num as f32 * 0.0625f32,
            W3D_EMITTER_RENDER_MODE_QUAD_PARTICLES => cost_factor = self.max_num as f32 * 2.0f32 * 0.0625f32,
            W3D_EMITTER_RENDER_MODE_LINE => {
                cost_factor = if crate::logic::modules::plant(10116) {
                    (2u32.wrapping_mul(self.max_num)) as f32 * 0.0625f32
                } else {
                    (2u32.wrapping_mul(self.max_num).wrapping_sub(1)) as f32 * 0.0625f32
                }
            }
            W3D_EMITTER_RENDER_MODE_LINEGRP_TETRA => cost_factor = self.max_num as f32 * 4.0f32 * 0.0625f32,
            W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM => {
                cost_factor = self.max_num as f32 * if crate::logic::modules::plant(10117) { 4.0f32 } else { 8.0f32 } * 0.0625f32
            }
            _ => {}
        }
        for lod in 0..lod_count as usize {
            costs[lod] = cost_factor * lod as f32;
            // If cost is zero set it to a small nonzero amount to avoid divisions by zero.
            costs[lod] = if costs[lod] != 0.0f32 && !crate::logic::modules::plant(10118) { costs[lod] } else { 0.000001f32 };
        }
        // Calculate Value heuristic. First, all LOD levels for which MaxScreenSize is smaller than screen_area have
        // their Value set to AT_MIN_LOD, as well as the first LOD after that (unless there are no other LODs):
        let mut lod = 0u32;
        while lod < lod_count && (sizes.0[lod as usize] < screen_area || (crate::logic::modules::plant(10119) && sizes.0[lod as usize] == screen_area)) {
            values[lod as usize] = AT_MIN_LOD;
            lod += 1;
        }
        if lod >= lod_count {
            lod = if crate::logic::modules::plant(10120) { 0 } else { lod_count - 1 };
        } else {
            values[lod as usize] = AT_MIN_LOD;
        }
        // Now lod is the lowest allowed - return this value.
        let minlod = lod as i32;
        // Calculate Value heuristic for any remaining LODs based on normalized screen area:
        lod += 1;
        while lod < lod_count {
            let polycount = costs[lod as usize];
            let benefit_factor = if polycount > wwmath::WWMATH_EPSILON {
                if crate::logic::modules::plant(10121) {
                    1.0f32 - (0.5f32 / polycount)
                } else {
                    1.0f32 - (0.5f32 / (polycount * polycount))
                }
            } else {
                0.0f32
            };
            values[lod as usize] = if crate::logic::modules::plant(10122) {
                (benefit_factor * screen_area) / costs[lod as usize]
            } else {
                (benefit_factor * screen_area * self.lod_bias) / costs[lod as usize]
            };
            lod += 1;
        }
        values[lod_count as usize] = AT_MAX_LOD; // Post-inc value will flag max LOD.
        minlod
    }

    /// `Increment_LOD()`: one level up (a lower threshold).
    pub fn increment_lod(&mut self) {
        if self.decimation_threshold > 0 && !crate::logic::modules::plant(10123) {
            self.decimation_threshold -= 1;
        }
    }

    /// `Decrement_LOD()`: one level down, to a threshold of `LodCount` (one
    /// past the lowest level: level -1).
    pub fn decrement_lod(&mut self) {
        let top = if crate::logic::modules::plant(10124) { self.lod_count - 1 } else { self.lod_count };
        if self.decimation_threshold < top {
            self.decimation_threshold += 1;
        }
    }

    /// `Cost[(LodCount - 1) - DecimationThreshold]` and its index as the
    /// `unsigned` it is computed in.
    fn lod_index(&self, extra: u32) -> u32 {
        (self.lod_count.wrapping_sub(1).wrapping_add(extra)).wrapping_sub(self.decimation_threshold)
    }

    /// `Get_Cost()`: `Cost[(LodCount - 1) - DecimationThreshold]`; undefined
    /// past the array (a threshold of `LodCount` or more).
    pub fn get_cost(&self) -> W3dResult<f32> {
        self.cost.get(self.lod_index(0) as usize).copied().ok_or(W3dUndefined::IndexOutOfRange.into())
    }

    /// `Get_Value()`: `Value[(LodCount - 1) - DecimationThreshold]`.
    pub fn get_value(&self) -> W3dResult<f32> {
        let i = if crate::logic::modules::plant(10125) { self.lod_index(1) } else { self.lod_index(0) };
        self.value.get(i as usize).copied().ok_or(W3dUndefined::IndexOutOfRange.into())
    }

    /// `Get_Post_Increment_Value()`: `Value[LodCount - DecimationThreshold]`.
    pub fn get_post_increment_value(&self) -> W3dResult<f32> {
        self.value.get(self.lod_index(1) as usize).copied().ok_or(W3dUndefined::IndexOutOfRange.into())
    }

    /// `Set_LOD_Level(lod)`: `Bound(lod, 0, LodCount)` (so 17 makes the
    /// threshold wrap to `0xFFFFFFFF`), then the threshold.
    pub fn set_lod_level(&mut self, lod: i32) {
        let hi = if crate::logic::modules::plant(10126) { self.lod_count as i32 - 1 } else { self.lod_count as i32 };
        let lod = if lod < 0 {
            0
        } else if lod > hi {
            hi
        } else {
            lod
        };
        self.decimation_threshold = self.lod_count.wrapping_sub(1).wrapping_sub(lod as u32);
    }

    /// `Get_LOD_Level()`: `(LodCount - 1) - DecimationThreshold`, an
    /// `unsigned` returned as an `int` (a threshold of 17 is level -1).
    pub fn get_lod_level(&self) -> i32 {
        self.lod_count.wrapping_sub(1).wrapping_sub(self.decimation_threshold) as i32
    }

    /// `Get_LOD_Count()`.
    pub fn get_lod_count(&self) -> i32 {
        self.lod_count as i32
    }

    /// `Set_LOD_Bias(bias)`: `LodBias = MAX(bias, 0.0f)` (a NaN bias is 0).
    pub fn set_lod_bias(&mut self, bias: f32) {
        self.lod_bias = if bias > 0.0f32 || crate::logic::modules::plant(10127) { bias } else { 0.0f32 };
    }

    /// `Invalidate_Cached_Bounding_Volumes()`.
    pub fn invalidate_cached_bounding_volumes(&mut self) {
        self.bounding_volumes_valid = false;
    }

    /// `Get_Obj_Space_Bounding_Sphere(sphere)`: `Update_Bounding_Box`, then
    /// the sphere around the box (centre, `Extent.Length()`).
    pub fn get_obj_space_bounding_sphere(&mut self, clock: &Ww3dClock) -> Sphere {
        self.update_bounding_box(clock);
        Sphere { center: self.bounding_box.center, radius: self.bounding_box.extent.length() }
    }

    /// `RenderObjClass::Get_Bounding_Sphere()`: the cached sphere, made again
    /// when the cache is invalid (`Update_Cached_Bounding_Volumes`: the box
    /// and the sphere in object space, the centre through the buffer's
    /// transform, the identity, and the radius times `ObjectScale`, 1).
    pub fn get_bounding_sphere(&mut self, clock: &Ww3dClock) -> Sphere {
        if !self.bounding_volumes_valid || crate::logic::modules::plant(10128) {
            // Get_Obj_Space_Bounding_Box(CachedBoundingBox): Update_Bounding_Box
            self.update_bounding_box(clock);
            let mut s = self.get_obj_space_bounding_sphere(clock);
            s.center = Matrix3D::identity().mul_vector3(&s.center);
            s.radius *= 1.0f32;
            self.cached_bounding_sphere = s;
            self.bounding_volumes_valid = true;
        }
        self.cached_bounding_sphere
    }

    /// `Prepare_LOD(camera)` at the clock, with the static screen sizes: the
    /// screen area (the lesser of the bounding sphere's projection and the
    /// particles' at their largest, times the viewport's pixels per view-plane
    /// unit), filtered (`0.9 * old + 0.1 * new`), the cost and value arrays,
    /// the level raised to the minimum. Returns the cost
    /// `PredictiveLODOptimizerClass::Add_Object(this)` adds (`Get_Cost()`), or
    /// `None` for a hidden buffer, which returns at once.
    pub fn prepare_lod(&mut self, camera: &LodCamera, sizes: &LodMaxScreenSizes, clock: &Ww3dClock) -> W3dResult<Option<f32>> {
        if !self.is_not_hidden_at_all() && !crate::logic::modules::plant(10129) {
            return Ok(None);
        }
        let cam = camera.position;
        // ViewportClass::Width() and Height()
        let vp_width = camera.viewport_max.x - camera.viewport_min.x;
        let vp_height = camera.viewport_max.y - camera.viewport_min.y;
        let (vpr_min, vpr_max) = (camera.view_plane_min, camera.view_plane_max);
        let width_factor = vp_width / (vpr_max.x - vpr_min.x);
        let height_factor = if crate::logic::modules::plant(10130) { vp_height / (vpr_max.x - vpr_min.x) } else { vp_height / (vpr_max.y - vpr_min.y) };
        let sphere = self.get_bounding_sphere(clock);
        let dist = (sphere.center - cam).length();
        let mut bounding_sphere_projected_radius = 0.0f32;
        let mut particle_projected_radius = 0.0f32;
        if dist != 0.0f32 {
            let oo_dist = 1.0f32 / dist;
            bounding_sphere_projected_radius = sphere.radius * oo_dist;
            particle_projected_radius = self.max_size * oo_dist;
        }
        let bs_rad_sq = bounding_sphere_projected_radius * bounding_sphere_projected_radius;
        let p_rad_sq = particle_projected_radius * particle_projected_radius * self.max_num as f32;
        // MIN(bs_rad_sq, p_rad_sq)
        let m = if bs_rad_sq < p_rad_sq || crate::logic::modules::plant(10131) { bs_rad_sq } else { p_rad_sq };
        let proj_area = wwmath::WWMATH_PI * m * width_factor * height_factor;
        // Filter the area over time so we don't get as many pops in the LOD algorithm
        self.projected_area = if crate::logic::modules::plant(10132) {
            0.1f32 * self.projected_area + 0.9f32 * proj_area
        } else {
            0.9f32 * self.projected_area + 0.1f32 * proj_area
        };
        let (mut value, mut cost) = (self.value, self.cost);
        let minlod = self.calculate_cost_value_arrays(self.projected_area, sizes, &mut value, &mut cost);
        (self.value, self.cost) = (value, cost);
        // Ensure lod is no less than minimum allowed
        if self.get_lod_level() < minlod {
            self.set_lod_level(minlod);
        }
        // PredictiveLODOptimizerClass::Add_Object(this): the object's cost
        Ok(Some(self.get_cost()?))
    }

    /// `On_Frame_Update()` without an emitter (`Emitter` null, after
    /// `Emitter_Is_Dead`): the cached bounding volumes invalidated; whether
    /// the buffer is complete and registers itself for release
    /// (`Scene->Register(this, SceneClass::RELEASE)`), which the caller does.
    /// With an emitter, [`super::part_emt::ParticleEmitter::buffer_on_frame_update`].
    pub fn on_frame_update(&mut self) -> bool {
        self.invalidate_cached_bounding_volumes();
        self.is_complete()
    }

    /// The table size for a non-zero randomizer:
    /// `MIN(Find_POT(MaxNum), MAX_RANDOM_ENTRIES)`.
    fn default_randomizer_entries(&self) -> u32 {
        let pot_num = find_pot(self.max_num as i32) as u32;
        pot_num.min(MAX_RANDOM_ENTRIES)
    }

    /// The keyframe half of `Reset_Colors`, `Reset_Opacity`, `Reset_Size`,
    /// `Reset_Frames` and `Reset_Blur_Times` (their text is the same): the
    /// times checked against `MaxAge`, the start keyframe and the deltas.
    /// Returns whether the property is constant past its last keyframe
    /// (`*_constant_at_end`).
    fn reset_keys<T: KeyValue>(max_age: u32, keys: &mut KeyTrack<T>, p: &Props<T>) -> W3dResult<()> {
        let nk = p.num_key_frames();
        let mut key = 0u32;
        while key < nk {
            let t = f2u(p.key_times[key as usize] * 1000.0f32)?;
            if t >= max_age {
                break;
            }
            key += 1;
        }
        let constant_at_end = key == nk;
        let n = key + 1; // includes start keyframe (keytime == 0)
        let mut times = vec![0u32; n as usize];
        let mut values = vec![T::default(); n as usize];
        let mut deltas = vec![T::default(); n as usize];
        times[0] = 0;
        values[0] = p.start;
        for i in 1..n as usize {
            times[i] = f2u(p.key_times[i - 1] * 1000.0f32)?;
            values[i] = p.values[i - 1];
        }
        // deltas for all keyframes except last
        for i in 0..(n - 1) as usize {
            deltas[i] = values[i + 1].sub(values[i]).div(times[i + 1].wrapping_sub(times[i]) as f32);
        }
        let i = (n - 1) as usize;
        if constant_at_end {
            deltas[i] = T::default();
        } else {
            deltas[i] = p.values[i].sub(values[i]).div(p.key_times[i] * 1000.0f32 - times[i] as f32);
        }
        keys.num = n;
        keys.times = Some(times);
        keys.values = Some(values);
        keys.deltas = Some(deltas);
        Ok(())
    }

    /// A property's constant case: no keyframe arrays, one value.
    fn constant_keys<T: KeyValue>(keys: &mut KeyTrack<T>, start: T) {
        keys.times = None;
        keys.deltas = None;
        keys.values = Some(vec![start]);
        keys.num = 0;
    }

    /// `Reset_Colors(new_props)`.
    pub fn reset_colors(&mut self, p: &Props<Vector3>, rand_gen: &mut Random4) -> W3dResult<()> {
        self.color_random = p.rand;
        // Epsilon value - less than 1/255
        const EPS_BYTE: f32 = 0.0038;
        let r = p.rand;
        let color_rand_zero = wwmath::fabs(r.x) < EPS_BYTE && wwmath::fabs(r.y) < EPS_BYTE && wwmath::fabs(r.z) < EPS_BYTE;
        if color_rand_zero && p.num_key_frames() == 0 {
            self.color = None;
            Self::constant_keys(&mut self.color_keys, p.start);
            self.random_color.minus1 = 0;
        } else {
            if self.color.is_none() {
                self.color = Some(vec![Vector3::default(); self.max_num as usize]);
            }
            Self::reset_keys(self.max_age, &mut self.color_keys, p)?;
            if color_rand_zero {
                self.random_color = RandomTable { minus1: 0, entries: Some(vec![Vector3::new(0.0, 0.0, 0.0)]) };
            } else {
                let n = self.default_randomizer_entries();
                let rscale = r.x * OO_INTMAX;
                let gscale = r.y * OO_INTMAX;
                let bscale = r.z * OO_INTMAX;
                let mut e = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    // (the constructor's arguments, taken left to right)
                    let x = rand_gen.next() as f32 * rscale;
                    let y = rand_gen.next() as f32 * gscale;
                    let z = rand_gen.next() as f32 * bscale;
                    e.push(Vector3::new(x, y, z));
                }
                self.random_color = RandomTable { minus1: n - 1, entries: Some(e) };
            }
        }
        Ok(())
    }

    /// A float table: one 0 entry when the randomizer is zero, else
    /// `rand_gen * scale` per entry.
    fn float_table(&self, zero: bool, scale: f32, rand_gen: &mut Random4) -> RandomTable<f32> {
        if zero {
            RandomTable { minus1: 0, entries: Some(vec![0.0]) }
        } else {
            let n = self.default_randomizer_entries();
            let e = (0..n).map(|_| rand_gen.next() as f32 * scale).collect();
            RandomTable { minus1: n - 1, entries: Some(e) }
        }
    }

    /// `Reset_Opacity(new_props)`.
    pub fn reset_opacity(&mut self, p: &Props<f32>, rand_gen: &mut Random4) -> W3dResult<()> {
        self.opacity_random = p.rand;
        const EPS_BYTE: f32 = 0.0038;
        let zero = wwmath::fabs(p.rand) < EPS_BYTE;
        if zero && p.num_key_frames() == 0 {
            self.alpha = None;
            Self::constant_keys(&mut self.alpha_keys, p.start);
            self.random_alpha.minus1 = 0;
        } else {
            if self.alpha.is_none() {
                self.alpha = Some(vec![0.0; self.max_num as usize]);
            }
            Self::reset_keys(self.max_age, &mut self.alpha_keys, p)?;
            self.random_alpha = self.float_table(zero, p.rand * OO_INTMAX, rand_gen);
        }
        Ok(())
    }

    /// `Reset_Size(new_props)`, with `MaxSize` for the bounding box.
    pub fn reset_size(&mut self, p: &Props<f32>, rand_gen: &mut Random4) -> W3dResult<()> {
        self.size_random = p.rand;
        // Size scale unknown so must use very small epsilon
        const EPS_SIZE: f32 = 1.0e-12;
        let zero = wwmath::fabs(p.rand) < EPS_SIZE;
        if zero && p.num_key_frames() == 0 {
            self.size = None;
            Self::constant_keys(&mut self.size_keys, p.start);
            self.random_size.minus1 = 0;
            self.max_size = p.start;
        } else {
            if self.size.is_none() {
                self.size = Some(vec![0.0; self.max_num as usize]);
            }
            Self::reset_keys(self.max_age, &mut self.size_keys, p)?;
            let k = &self.size_keys;
            let (values, deltas, times) = (k.values.as_ref().unwrap(), k.deltas.as_ref().unwrap(), k.times.as_ref().unwrap());
            let n = k.num as usize;
            // Find maximum size (for BBox updates)
            let mut max_size = values[0];
            for v in values.iter().take(n).skip(1) {
                max_size = if max_size > *v { max_size } else { *v };
            }
            // If last delta is positive, there may be a larger size keyframe:
            let last_size = values[n - 1] + deltas[n - 1] * self.max_age.wrapping_sub(times[n - 1]) as f32;
            max_size = if max_size > last_size { max_size } else { last_size };
            max_size += wwmath::fabs(p.rand);
            self.max_size = max_size;
            self.random_size = self.float_table(zero, p.rand * OO_INTMAX, rand_gen);
        }
        Ok(())
    }

    /// `Reset_Rotations(new_props, orient_rnd)`: rotations per second made
    /// per millisecond, integrated into orientations at the keyframes.
    pub fn reset_rotations(&mut self, p: &Props<f32>, orient_rnd: f32, rand_gen: &mut Random4) -> W3dResult<()> {
        self.rotation_random = p.rand * 0.001f32;
        self.initial_orientation_random = orient_rnd;
        // 0.1 degree; one rotation per hour
        const EPS_ORIENTATION: f32 = 2.777_777_8e-4;
        const EPS_ROTATION: f32 = 2.777_777_8e-4;
        let orientation_rand_zero = wwmath::fabs(orient_rnd) < EPS_ORIENTATION;
        let rotation_rand_zero = wwmath::fabs(p.rand) < EPS_ROTATION;
        if orientation_rand_zero && rotation_rand_zero && p.num_key_frames() == 0 && wwmath::fabs(p.start) < EPS_ROTATION {
            self.orientation = None;
            self.rotation_keys = RotationTrack::default();
            self.random_rotation.minus1 = 0;
            self.random_orientation.minus1 = 0;
            return Ok(());
        }
        if self.orientation.is_none() {
            self.orientation = Some(vec![0; self.max_num as usize]);
        }
        let nk = p.num_key_frames();
        let mut key = 0u32;
        while key < nk {
            if f2u(p.key_times[key as usize] * 1000.0f32)? >= self.max_age {
                break;
            }
            key += 1;
        }
        let constant_at_end = key == nk;
        let n = (key + 1) as usize;
        let mut times = vec![0u32; n];
        let mut values = vec![0.0f32; n];
        let mut half = vec![0.0f32; n];
        let mut orient = vec![0.0f32; n];
        values[0] = p.start * 0.001f32;
        for i in 1..n {
            times[i] = f2u(p.key_times[i - 1] * 1000.0f32)?;
            values[i] = p.values[i - 1] * 0.001f32;
        }
        for i in 0..n - 1 {
            half[i] = 0.5f32 * ((values[i + 1] - values[i]) / times[i + 1].wrapping_sub(times[i]) as f32);
        }
        let i = n - 1;
        if constant_at_end {
            half[i] = 0.0;
        } else {
            half[i] = 0.5f32 * (p.values[i] * 0.001f32 - values[i]) / (p.key_times[i] * 1000.0f32 - times[i] as f32);
        }
        // orientation keyframes: the rotation integrated at each keyframe
        orient[0] = 0.0;
        for i in 1..n {
            let delta_t = times[i].wrapping_sub(times[i - 1]) as f32;
            orient[i] = orient[i - 1] + delta_t * (values[i - 1] + half[i - 1] * delta_t);
        }
        self.rotation_keys = RotationTrack { num: n as u32, times: Some(times), values: Some(values), half_deltas: Some(half), orientations: Some(orient) };
        self.random_rotation = self.float_table(rotation_rand_zero, p.rand * 0.001f32 * OO_INTMAX, rand_gen);
        self.random_orientation = self.float_table(orientation_rand_zero, orient_rnd * OO_INTMAX, rand_gen);
        Ok(())
    }

    /// `Reset_Frames(new_props)` (the line groups keep the frame as a `float`
    /// u coordinate, `UCoord`).
    pub fn reset_frames(&mut self, p: &Props<f32>, rand_gen: &mut Random4) -> W3dResult<()> {
        self.frame_random = p.rand;
        // Epsilon is equivalent to 0.1 frame
        const EPS_FRAME: f32 = 0.1;
        let zero = wwmath::fabs(p.rand) < EPS_FRAME;
        if zero && p.num_key_frames() == 0 {
            self.frame = None;
            self.ucoord = None;
            Self::constant_keys(&mut self.frame_keys, p.start);
            self.random_frame.minus1 = 0;
        } else {
            if self.render_mode == W3D_EMITTER_RENDER_MODE_LINEGRP_TETRA || self.render_mode == W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM {
                if self.ucoord.is_none() {
                    self.ucoord = Some(vec![0.0; self.max_num as usize]);
                }
            } else if self.frame.is_none() {
                self.frame = Some(vec![0; self.max_num as usize]);
            }
            Self::reset_keys(self.max_age, &mut self.frame_keys, p)?;
            self.random_frame = self.float_table(zero, p.rand * OO_INTMAX, rand_gen);
        }
        Ok(())
    }

    /// `Reset_Blur_Times(new_blur_times)` (no per-particle array: the tail
    /// positions use them).
    pub fn reset_blur_times(&mut self, p: &Props<f32>, rand_gen: &mut Random4) -> W3dResult<()> {
        self.blur_time_random = p.rand;
        // Epsilon is equivalent to 1e-5 units per second
        const EPS_BLUR: f32 = 1e-5;
        let zero = wwmath::fabs(p.rand) < EPS_BLUR;
        if zero && p.num_key_frames() == 0 {
            Self::constant_keys(&mut self.blur_time_keys, p.start);
            self.random_blur_time.minus1 = 0;
        } else {
            Self::reset_keys(self.max_age, &mut self.blur_time_keys, p)?;
            self.random_blur_time = self.float_table(zero, p.rand * OO_INTMAX, rand_gen);
        }
        Ok(())
    }

    /// `Scale(scale)`: the sizes (keyframes, deltas, random entries,
    /// `MaxSize`, `SizeRandom`), the line's width and the acceleration.
    pub fn scale(&mut self, scale: f32) {
        let k = &mut self.size_keys;
        if k.num != 0 {
            if let (Some(v), Some(d)) = (k.values.as_mut(), k.deltas.as_mut()) {
                for i in 0..k.num as usize {
                    v[i] *= scale;
                    d[i] *= scale;
                }
            }
        } else if let Some(v) = k.values.as_mut() {
            v[0] *= scale;
        }
        if let Some(e) = self.random_size.entries.as_mut() {
            for x in e.iter_mut().take(self.random_size.minus1 as usize + 1) {
                *x *= scale;
            }
        }
        if let Some(lr) = self.line_renderer.as_mut() {
            // SegLineRendererClass::Scale: the width and the noise amplitude
            lr.scale(scale);
        }
        self.max_size *= scale;
        self.size_random *= scale;
        self.accel *= scale;
    }

    /// `Add_Uninitialized_New_Particle()`: the queue slot to fill (the
    /// oldest is dropped on overflow).
    pub fn add_uninitialized_new_particle(&mut self) -> &mut NewParticle {
        let at = self.new_particle_queue_end as usize;
        self.new_particle_queue_end += 1;
        if self.new_particle_queue_end == self.max_num {
            self.new_particle_queue_end = 0;
        }
        self.new_particle_queue_count += 1;
        if self.new_particle_queue_count == self.max_num.wrapping_add(1) as i32 {
            // Overflow - advance queue start:
            self.new_particle_queue_start += 1;
            if self.new_particle_queue_start == self.max_num {
                self.new_particle_queue_start = 0;
            }
            self.new_particle_queue_count -= 1;
        }
        &mut self.new_particle_queue[at]
    }

    /// The position array this frame (`Position[pingpong]`).
    fn pingpong(&self, clock: &Ww3dClock) -> usize {
        if self.ping_pong_position {
            (clock.frame_count & 1) as usize
        } else {
            0
        }
    }

    /// `Update_Kinematic_Particle_State()`.
    pub fn update_kinematic_particle_state(&mut self, clock: &Ww3dClock) {
        let elapsed = clock.sync_time.wrapping_sub(self.last_update_time);
        if elapsed == 0 {
            return;
        }
        self.get_new_particles(clock);
        self.kill_old_particles(clock);
        if self.non_new_num > 0 {
            self.update_non_new_particles(elapsed, clock);
        }
        // Mark all new particles as non-new.
        self.end = self.new_end;
        self.non_new_num += self.new_num;
        self.new_num = 0;
        self.last_update_time = clock.sync_time;
        self.bounding_box_dirty = true;
    }

    /// `Get_New_Particles()`.
    fn get_new_particles(&mut self, clock: &Ww3dClock) {
        let current_time = clock.sync_time;
        let pp = self.pingpong(clock);
        while self.new_particle_queue_count != 0 {
            let np = self.new_particle_queue[self.new_particle_queue_start as usize];
            self.new_particle_queue_start += 1;
            if self.new_particle_queue_start == self.max_num {
                self.new_particle_queue_start = 0;
            }
            self.new_particle_queue_count -= 1;
            let ne = self.new_end as usize;
            // (written before the age test)
            self.time_stamp[ne] = np.time_stamp;
            let age = current_time.wrapping_sub(self.time_stamp[ne]);
            if age >= self.max_age {
                continue;
            }
            let fp_age = age as f32;
            let position = self.position[pp].as_mut().expect("Position[pingpong]");
            if self.has_accel {
                position[ne] = np.position + (np.velocity + 0.5f32 * self.accel * fp_age) * fp_age;
                self.velocity[ne] = np.velocity + (self.accel * fp_age);
            } else {
                position[ne] = np.position + (np.velocity * fp_age);
                self.velocity[ne] = np.velocity;
            }
            if self.ping_pong_position {
                self.position[pp ^ 1].as_mut().expect("Position[!pingpong]")[ne] = np.position;
            }
            self.group_id[ne] = np.group_id;
            self.new_end += 1;
            if self.new_end == self.max_num {
                self.new_end = 0;
            }
            self.new_num += 1;
            // If we have just overflowed the total buffer, advance Start.
            if self.new_num + self.non_new_num == self.max_num.wrapping_add(1) as i32 {
                self.start += 1;
                if self.start == self.max_num {
                    self.start = 0;
                }
                self.non_new_num -= 1;
                // If this underflows the 'non-new' buffer, advance End.
                if self.non_new_num == -1 {
                    self.end += 1;
                    if self.end == self.max_num {
                        self.end = 0;
                    }
                    self.non_new_num = 0;
                    self.new_num -= 1;
                }
            }
        }
    }

    /// `Kill_Old_Particles()`.
    fn kill_old_particles(&mut self, clock: &Ww3dClock) {
        let (sub1_end, sub2_start) = subranges(self.start, self.end, self.non_new_num, self.max_num);
        let current_time = clock.sync_time;
        let mut broke = false;
        let mut i = self.start;
        while i < sub1_end {
            if current_time.wrapping_sub(self.time_stamp[i as usize]) < self.max_age {
                broke = true;
                break;
            }
            self.non_new_num -= 1;
            i += 1;
        }
        if !broke {
            i = sub2_start;
            while i < self.end {
                if current_time.wrapping_sub(self.time_stamp[i as usize]) < self.max_age {
                    break;
                }
                self.non_new_num -= 1;
                i += 1;
            }
        }
        self.start = i;
    }

    /// `Update_Non_New_Particles(elapsed)`.
    fn update_non_new_particles(&mut self, elapsed: u32, clock: &Ww3dClock) {
        let (sub1_end, sub2_start) = subranges(self.start, self.end, self.non_new_num, self.max_num);
        let fp_elapsed_time = elapsed as f32;
        let ranges = [self.start..sub1_end, sub2_start..self.end];
        if self.ping_pong_position {
            let pp = self.pingpong(clock);
            let prev = self.position[pp ^ 1].clone().expect("Position[!pingpong]");
            let position = self.position[pp].as_mut().expect("Position[pingpong]");
            if self.has_accel {
                let delta_v = self.accel * fp_elapsed_time;
                let accel_p = self.accel * (0.5f32 * fp_elapsed_time * fp_elapsed_time);
                for r in ranges {
                    for i in r {
                        let i = i as usize;
                        position[i] = prev[i] + self.velocity[i] * fp_elapsed_time + accel_p;
                        self.velocity[i] += delta_v;
                    }
                }
            } else {
                for r in ranges {
                    for i in r {
                        let i = i as usize;
                        position[i] += self.velocity[i] * fp_elapsed_time;
                    }
                }
            }
        } else {
            let position = self.position[0].as_mut().expect("Position[0]");
            if self.has_accel {
                let delta_v = self.accel * fp_elapsed_time;
                let accel_p = self.accel * (0.5f32 * fp_elapsed_time * fp_elapsed_time);
                for r in ranges {
                    for i in r {
                        let i = i as usize;
                        position[i] += self.velocity[i] * fp_elapsed_time + accel_p;
                        self.velocity[i] += delta_v;
                    }
                }
            } else {
                for r in ranges {
                    for i in r {
                        let i = i as usize;
                        position[i] += self.velocity[i] * fp_elapsed_time;
                    }
                }
            }
        }
    }

    /// `Update_Visual_Particle_State()`: colour, alpha, size, orientation,
    /// frame (or u coordinate) and tail position of every particle at its
    /// age, from the oldest to the youngest.
    pub fn update_visual_particle_state(&mut self, clock: &Ww3dClock) -> W3dResult<()> {
        let is_linegroup = self.render_mode == W3D_EMITTER_RENDER_MODE_LINEGRP_TETRA || self.render_mode == W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM;
        if self.color.is_none()
            && self.alpha.is_none()
            && self.size.is_none()
            && self.orientation.is_none()
            && self.frame.is_none()
            && self.ucoord.is_none()
            && !is_linegroup
        {
            return Ok(());
        }
        let (sub1_end, sub2_start) = subranges(self.start, self.end, self.non_new_num, self.max_num);
        let current_time = clock.sync_time;
        let mut ckey = self.color_keys.num.wrapping_sub(1);
        let mut akey = self.alpha_keys.num.wrapping_sub(1);
        let mut skey = self.size_keys.num.wrapping_sub(1);
        let mut rkey = self.rotation_keys.num.wrapping_sub(1);
        let mut fkey = self.frame_keys.num.wrapping_sub(1);
        let mut bkey = self.blur_time_keys.num.wrapping_sub(1);
        let pp = self.pingpong(clock);
        // (back-to-back loops over the ring's subranges, in order)
        let parts: Vec<u32> = (self.start..sub1_end).chain(sub2_start..self.end).collect();
        for part in parts {
            let p = part as usize;
            let part_age = current_time.wrapping_sub(self.time_stamp[p]);
            if let Some(color) = self.color.as_mut() {
                let k = &self.color_keys;
                let times = k.times.as_ref().expect("ColorKeyFrameTimes");
                while part_age < times[ckey as usize] {
                    ckey = ckey.wrapping_sub(1);
                }
                let c = ckey as usize;
                let rt = &self.random_color;
                color[p] = k.values.as_ref().unwrap()[c]
                    + k.deltas.as_ref().unwrap()[c] * part_age.wrapping_sub(times[c]) as f32
                    + rt.entries.as_ref().unwrap()[(part & rt.minus1) as usize];
            }
            if let Some(alpha) = self.alpha.as_mut() {
                let k = &self.alpha_keys;
                let times = k.times.as_ref().expect("AlphaKeyFrameTimes");
                while part_age < times[akey as usize] {
                    akey = akey.wrapping_sub(1);
                }
                let a = akey as usize;
                let rt = &self.random_alpha;
                alpha[p] = k.values.as_ref().unwrap()[a]
                    + k.deltas.as_ref().unwrap()[a] * part_age.wrapping_sub(times[a]) as f32
                    + rt.entries.as_ref().unwrap()[(part & rt.minus1) as usize];
            }
            if let Some(size) = self.size.as_mut() {
                let k = &self.size_keys;
                let times = k.times.as_ref().expect("SizeKeyFrameTimes");
                while part_age < times[skey as usize] {
                    skey = skey.wrapping_sub(1);
                }
                let s = skey as usize;
                let rt = &self.random_size;
                size[p] = k.values.as_ref().unwrap()[s]
                    + k.deltas.as_ref().unwrap()[s] * part_age.wrapping_sub(times[s]) as f32
                    + rt.entries.as_ref().unwrap()[(part & rt.minus1) as usize];
                // Size isn't clamped in the engine: clamp negative values to zero
                size[p] = if size[p] >= 0.0f32 { size[p] } else { 0.0 };
            }
            if let Some(orientation) = self.orientation.as_mut() {
                let k = &self.rotation_keys;
                let times = k.times.as_ref().expect("RotationKeyFrameTimes");
                while part_age < times[rkey as usize] {
                    rkey = rkey.wrapping_sub(1);
                }
                let r = rkey as usize;
                let f_delta_t = part_age.wrapping_sub(times[r]) as f32;
                let rr = &self.random_rotation;
                let ro = &self.random_orientation;
                let tmp_orient = k.orientations.as_ref().unwrap()[r]
                    + (k.values.as_ref().unwrap()[r] + k.half_deltas.as_ref().unwrap()[r] * f_delta_t) * f_delta_t
                    + rr.entries.as_ref().unwrap()[(part & rr.minus1) as usize] * part_age as f32
                    + ro.entries.as_ref().unwrap()[(part & ro.minus1) as usize];
                orientation[p] = (f2i(tmp_orient * 256.0f32)? & 0xFF) as u8;
            }
            if self.frame.is_some() || self.ucoord.is_some() {
                let k = &self.frame_keys;
                let times = k.times.as_ref().expect("FrameKeyFrameTimes");
                while part_age < times[fkey as usize] {
                    fkey = fkey.wrapping_sub(1);
                }
                let f = fkey as usize;
                let rt = &self.random_frame;
                let v = k.values.as_ref().unwrap()[f]
                    + k.deltas.as_ref().unwrap()[f] * part_age.wrapping_sub(times[f]) as f32
                    + rt.entries.as_ref().unwrap()[(part & rt.minus1) as usize];
                if let Some(frame) = self.frame.as_mut() {
                    frame[p] = (f2i(v)? & 0xFF) as u8;
                }
                if let Some(ucoord) = self.ucoord.as_mut() {
                    ucoord[p] = v;
                }
            }
            if let Some(tail) = self.tail_position.as_mut() {
                let k = &self.blur_time_keys;
                let mut blur_time = k.values.as_ref().unwrap()[0];
                if let Some(times) = k.times.as_ref() {
                    while part_age < times[bkey as usize] {
                        bkey = bkey.wrapping_sub(1);
                    }
                    let b = bkey as usize;
                    let rt = &self.random_blur_time;
                    blur_time = k.values.as_ref().unwrap()[b]
                        + k.deltas.as_ref().unwrap()[b] * part_age.wrapping_sub(times[b]) as f32
                        + rt.entries.as_ref().unwrap()[(part & rt.minus1) as usize];
                }
                let position = self.position[pp].as_ref().expect("Position");
                tail[p] = position[p] - self.velocity[p] * blur_time * 1000.0f32;
            }
        }
        Ok(())
    }

    /// `Update_Bounding_Box()`: the kinematic update, then the box around
    /// every particle grown by `MaxSize`.
    pub fn update_bounding_box(&mut self, clock: &Ww3dClock) {
        self.update_kinematic_particle_state(clock);
        if !self.bounding_box_dirty {
            return;
        }
        if self.non_new_num == 0 {
            self.bounding_box = AaBox::default();
            self.bounding_box_dirty = false;
            return;
        }
        let pp = self.pingpong(clock);
        let position = self.position[pp].as_ref().expect("Position");
        let mut max_c = position[self.start as usize];
        let mut min_c = position[self.start as usize];
        let (sub1_end, sub2_start) = subranges(self.start, self.end, self.non_new_num, self.max_num);
        for i in (self.start..sub1_end).chain(sub2_start..self.end) {
            let p = position[i as usize];
            max_c.x = if max_c.x >= p.x { max_c.x } else { p.x };
            max_c.y = if max_c.y >= p.y { max_c.y } else { p.y };
            max_c.z = if max_c.z >= p.z { max_c.z } else { p.z };
            min_c.x = if min_c.x <= p.x { min_c.x } else { p.x };
            min_c.y = if min_c.y <= p.y { min_c.y } else { p.y };
            min_c.z = if min_c.z <= p.z { min_c.z } else { p.z };
        }
        // Extend by maximum possible particle size:
        let size = Vector3::new(self.max_size, self.max_size, self.max_size);
        max_c += size;
        min_c -= size;
        // AABoxClass::Init(MinMaxAABoxClass(min, max))
        self.bounding_box = AaBox { center: (max_c + min_c) * 0.5f32, extent: (max_c - min_c) * 0.5f32 };
        self.bounding_box_dirty = false;
    }

    /// `Generate_APT(&apt, active_point_count)`: the active points (`None`
    /// when every slot is, and no table is built).
    fn generate_apt(&mut self) -> (Option<Vec<u32>>, u32) {
        if self.non_new_num < self.max_num as i32 || self.decimation_threshold > 0 {
            let (sub1_start, sub1_end, sub2_start, sub2_end) = if self.start < self.end || (self.start == self.end && self.non_new_num == 0) {
                (self.start, self.end, self.end, self.end)
            } else {
                (0, self.end, self.start, self.max_num)
            };
            let mut count = 0u32;
            for i in (sub1_start..sub1_end).chain(sub2_start..sub2_end) {
                let keep = if crate::logic::modules::plant(10133) {
                    PERMUTATION_ARRAY[(i & 0xF) as usize] > self.decimation_threshold
                } else {
                    PERMUTATION_ARRAY[(i & 0xF) as usize] >= self.decimation_threshold
                };
                if keep {
                    self.apt[count as usize] = i;
                    count += 1;
                }
            }
            (Some(self.apt[..count as usize].to_vec()), count)
        } else {
            (None, self.non_new_num as u32)
        }
    }

    /// `Combine_Color_And_Alpha()`: `Diffuse` from `Color` and `Alpha`
    /// (`VectorProcessorClass::Copy`), clamped in place to 0..1
    /// (`VectorProcessorClass::Clamp`).
    fn combine_color_and_alpha(&mut self) {
        if self.color.is_some() || self.alpha.is_some() {
            let cnt = self.max_num as usize;
            let diffuse = self.diffuse.get_or_insert_with(|| vec![Vector4::new(0.0, 0.0, 0.0, 0.0); cnt]);
            match (&self.color, &self.alpha) {
                (Some(c), Some(a)) => {
                    for i in 0..cnt {
                        diffuse[i] = Vector4::new(c[i].x, c[i].y, c[i].z, a[i]);
                    }
                }
                (Some(c), None) => {
                    for i in 0..cnt {
                        diffuse[i] = Vector4::new(c[i].x, c[i].y, c[i].z, 1.0);
                    }
                }
                (None, Some(a)) => {
                    for i in 0..cnt {
                        diffuse[i] = Vector4::new(1.0, 1.0, 1.0, a[i]);
                    }
                }
                (None, None) => unreachable!(),
            }
            // Clamp(Diffuse, Diffuse, 0, 1): dst and src the same array
            for d in diffuse.iter_mut() {
                for k in 0..4 {
                    d[k] = if d[k] < 0.0f32 { 0.0 } else { d[k] };
                    d[k] = if d[k] > 1.0f32 { 1.0 } else { d[k] };
                }
            }
        } else {
            self.diffuse = None;
        }
    }

    /// `Render(rinfo)`: the kinematic update, the visual state (unless the
    /// whole buffer is decimated), then the mode's renderer. `emitter` is
    /// the emitter's position and whether it is stopped (line mode closes
    /// the newest run at it), `None` once it is dead.
    pub fn render(&mut self, clock: &Ww3dClock, emitter: Option<(Vector3, bool)>) -> W3dResult<ParticleDraw> {
        self.update_kinematic_particle_state(clock);
        if self.decimation_threshold < self.lod_count - 1 && !(crate::logic::modules::plant(10134) && self.decimation_threshold == self.lod_count - 2) {
            self.update_visual_particle_state(clock)?;
        }
        Ok(match self.render_mode {
            W3D_EMITTER_RENDER_MODE_TRI_PARTICLES | W3D_EMITTER_RENDER_MODE_QUAD_PARTICLES if self.has_point_group => {
                ParticleDraw::Points(self.render_particles(clock)?)
            }
            W3D_EMITTER_RENDER_MODE_LINE if self.has_line_renderer => self.render_line(clock, emitter),
            W3D_EMITTER_RENDER_MODE_LINEGRP_TETRA | W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM if self.has_line_group => {
                ParticleDraw::LineGroup(self.render_line_group(clock))
            }
            _ => ParticleDraw::Nothing,
        })
    }

    /// The slots a point or line group draws: the table's, or the first
    /// `count`.
    fn active(apt: Option<Vec<u32>>, count: u32) -> Vec<u32> {
        apt.unwrap_or_else(|| (0..count).collect())
    }

    /// `Render_Particles(rinfo)`: what the point group is given.
    fn render_particles(&mut self, clock: &Ww3dClock) -> W3dResult<PointGroupDraw> {
        let (apt, count) = self.generate_apt();
        // defaults if an array is not present
        let default_color = self.color_keys.values.as_ref().map_or(Vector3::default(), |v| v[0]);
        let default_alpha = self.alpha_keys.values.as_ref().map_or(0.0, |v| v[0]);
        let default_size = self.get_particle_size();
        let default_frame = if self.frame.is_none() { (f2i(self.frame_keys.values.as_ref().map_or(0.0, |v| v[0]))? & 0xFF) as u8 } else { 0 };
        let pp = self.pingpong(clock);
        self.combine_color_and_alpha();
        let mut points = Vec::with_capacity(count as usize);
        {
            let position = self.position[pp].as_ref().expect("Position");
            for i in Self::active(apt, count) {
                let p = i as usize;
                points.push(PointSprite {
                    index: i,
                    position: position[p],
                    diffuse: match &self.diffuse {
                        Some(d) => d[p],
                        None => Vector4::new(default_color.x, default_color.y, default_color.z, default_alpha),
                    },
                    size: self.size.as_ref().map_or(default_size, |s| s[p]),
                    orientation: self.orientation.as_ref().map_or(0, |o| o[p]),
                    frame: self.frame.as_ref().map_or(default_frame, |f| f[p]),
                });
            }
        }
        self.update_bounding_box(clock);
        Ok(PointGroupDraw {
            quads: self.render_mode == W3D_EMITTER_RENDER_MODE_QUAD_PARTICLES,
            frame_row_column_count_log2: (self.frame_mode as u8).min(4),
            points,
        })
    }

    /// `Render_Line(rinfo)`: the ring unrolled (decimated slots skipped)
    /// into runs of one group ID; the emitter's position closes the last
    /// run when the emitter runs and the last point is of its current group.
    /// Runs of one point are not drawn.
    fn render_line(&mut self, clock: &Ww3dClock, emitter: Option<(Vector3, bool)>) -> ParticleDraw {
        let pp = self.pingpong(clock);
        self.combine_color_and_alpha();
        let mut default_diffuse = Vector4::new(0.0, 0.0, 0.0, 0.0);
        if self.diffuse.is_none() {
            let c = self.color_keys.values.as_ref().map_or(Vector3::default(), |v| v[0]);
            let a = self.alpha_keys.values.as_ref().map_or(0.0, |v| v[0]);
            default_diffuse = Vector4::new(c.x, c.y, c.z, a);
        }
        let (sub1_end, sub2_start) = if self.start < self.end || (self.start == self.end && self.non_new_num == 0) {
            (self.end, self.end)
        } else {
            (self.max_num, 0)
        };
        let positions = self.position[pp].as_ref().expect("Position");
        let mut pts = Vec::new();
        let mut cols = Vec::new();
        let mut ids = Vec::new();
        let mut last_color = default_diffuse;
        let mut last_id = 0u8;
        for i in (self.start..sub1_end).chain(sub2_start..self.end) {
            let k = if crate::logic::modules::plant(10135) { i & 0x7 } else { i & 0xF };
            if PERMUTATION_ARRAY[k as usize] >= self.decimation_threshold {
                pts.push(positions[i as usize]);
                last_color = self.diffuse.as_ref().map_or(default_diffuse, |d| d[i as usize]);
                cols.push(last_color);
                last_id = self.group_id[i as usize];
                ids.push(last_id);
            }
        }
        // add in the emitter's position too for the source
        if let Some((pos, stopped)) = emitter {
            if !stopped && last_id == self.current_group_id {
                pts.push(pos);
                cols.push(last_color);
                ids.push(last_id);
            }
        }
        let mut runs = Vec::new();
        let count = pts.len();
        if count > 0 {
            // Get_Obj_Space_Bounding_Sphere: Update_Bounding_Box
            self.update_bounding_box(clock);
        }
        let (mut start, mut end) = (0usize, 0usize);
        while end < count {
            while end < count && ids[start] == ids[end] {
                end += 1;
            }
            if end - start > 1 {
                runs.push(LineRun { points: pts[start..end].to_vec(), colors: cols[start..end].to_vec() });
            }
            start = end;
        }
        ParticleDraw::Lines { width: self.line_renderer.as_ref().map_or(0.0, |lr| lr.width), runs }
    }

    /// `new SegLineRendererClass` at sync time `clock.sync_time`, then
    /// `Init(*line_props)`, `Set_Texture(tex)`, `Set_Shader(shader)` and
    /// `Set_Width(Get_Particle_Size())`, as the constructor makes it (a null
    /// `line_props` is dereferenced upstream: the defaults are kept here, and
    /// the constructor then drops back to triangles).
    fn new_line_renderer(&self, line_props: Option<&W3dEmitterLinePropertiesStruct>, shader: &ShaderClass, clock: &Ww3dClock) -> SegLineRenderer {
        let mut lr = SegLineRenderer::new(clock.sync_time);
        if let Some(lp) = line_props {
            lr.init(lp);
        }
        lr.texture = self.texture.clone();
        lr.shader = *shader;
        lr.width = self.get_particle_size();
        lr
    }

    /// The rest of `Render_Line`: each run of a [`ParticleDraw::Lines`] (in
    /// order) through `LineRenderer->Render(rinfo, Transform, ...)` at sync
    /// time `clock.sync_time`, with the buffer's transform (the identity:
    /// particles are kept in world space) and the camera's `view`; `v3` is
    /// the randomizers' shared generator. **Differs from upstream:** the
    /// caller draws the runs after `Render` (which has no camera here),
    /// before anything else draws from `v3`. `Set_Freeze_Random(
    /// Is_Freeze_Random())` sets the flag to itself and is left out. Each
    /// run's chunks, `None` for a buffer with no line renderer.
    pub fn render_line_runs(
        &mut self,
        runs: &[LineRun],
        clock: &Ww3dClock,
        view: &Matrix3D,
        v3: &mut Random3,
    ) -> W3dResult<Option<Vec<Vec<SegLineChunk>>>> {
        let Some(lr) = self.line_renderer.as_mut() else { return Ok(None) };
        let transform = Matrix3D::identity();
        let mut out = Vec::with_capacity(runs.len());
        for r in runs {
            out.push(lr.render(clock.sync_time, view, &transform, &r.points, Some(&r.colors), v3)?);
        }
        Ok(Some(out))
    }

    /// `Determine_Tail_Diffuse()`: from the texture and the shader.
    pub fn determine_tail_diffuse(&self) -> TailDiffuseType {
        // a texture controls the fade-off: the tail is the head's colour
        if self.texture.is_some() {
            return TailDiffuseType::SameAsHead;
        }
        // (called with a line group, whose shader Get_Shader returns)
        let shader = self.shader;
        let (src, dst) = (shader.get_src_blend_func(), shader.get_dst_blend_func());
        if dst == DSTBLEND_SRC_COLOR {
            TailDiffuseType::White
        } else if (src == SRCBLEND_ONE && dst == DSTBLEND_ONE) || (src == SRCBLEND_ONE && dst == DSTBLEND_ONE_MINUS_SRC_COLOR) {
            TailDiffuseType::Black
        } else if (src == SRCBLEND_SRC_ALPHA && dst == DSTBLEND_ONE_MINUS_SRC_ALPHA) || shader.get_alpha_test() == 1 {
            TailDiffuseType::SameAsHeadAlphaZero
        } else {
            TailDiffuseType::SameAsHead
        }
    }

    /// `Render_Line_Group(rinfo)`: what the line group is given.
    fn render_line_group(&mut self, clock: &Ww3dClock) -> LineGroupDraw {
        let (apt, count) = self.generate_apt();
        let default_color = self.color_keys.values.as_ref().map_or(Vector3::default(), |v| v[0]);
        let default_alpha = self.alpha_keys.values.as_ref().map_or(0.0, |v| v[0]);
        let default_size = self.get_particle_size();
        // (tests Frame, which line groups never have: the default u coordinate is always set)
        let default_ucoord = self.frame_keys.values.as_ref().map_or(0.0, |v| v[0]);
        let pp = self.pingpong(clock);
        self.combine_color_and_alpha();
        match self.determine_tail_diffuse() {
            TailDiffuseType::Black => {
                self.tail_diffuse = None;
                self.default_tail_diffuse = Vector4::new(0.0, 0.0, 0.0, 0.0);
            }
            TailDiffuseType::White => {
                self.tail_diffuse = None;
                self.default_tail_diffuse = Vector4::new(1.0, 1.0, 1.0, 1.0);
            }
            TailDiffuseType::SameAsHeadAlphaZero => match &self.diffuse {
                None => {
                    self.tail_diffuse = None;
                    self.default_tail_diffuse = Vector4::new(default_color.x, default_color.y, default_color.z, 0.0);
                }
                Some(d) => {
                    let t = self.tail_diffuse.get_or_insert_with(Vec::new);
                    t.clear();
                    t.extend(d.iter().map(|e| Vector4::new(e.x, e.y, e.z, 0.0)));
                }
            },
            TailDiffuseType::SameAsHead => match &self.diffuse {
                None => {
                    self.tail_diffuse = None;
                    self.default_tail_diffuse = Vector4::new(default_color.x, default_color.y, default_color.z, default_alpha);
                }
                Some(d) => self.tail_diffuse = Some(d.clone()),
            },
        }
        let position = self.position[pp].as_ref().expect("Position");
        let tails = self.tail_position.as_ref().expect("TailPosition");
        let mut lines = Vec::with_capacity(count as usize);
        for i in Self::active(apt, count) {
            let p = i as usize;
            lines.push(GroupLine {
                index: i,
                head: position[p],
                tail: tails[p],
                head_diffuse: self.diffuse.as_ref().map_or(Vector4::new(default_color.x, default_color.y, default_color.z, default_alpha), |d| d[p]),
                tail_diffuse: self.tail_diffuse.as_ref().map_or(self.default_tail_diffuse, |d| d[p]),
                size: self.size.as_ref().map_or(default_size, |s| s[p]),
                ucoord: self.ucoord.as_ref().map_or(default_ucoord, |u| u[p]),
            });
        }
        self.update_bounding_box(clock);
        LineGroupDraw { prism: self.render_mode == W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM, lines }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props_f(start: f32, rand: f32, keys: &[(f32, f32)]) -> Props<f32> {
        Props { start, rand, key_times: keys.iter().map(|k| k.0).collect(), values: keys.iter().map(|k| k.1).collect() }
    }

    fn params(size: Props<f32>) -> BufferParams {
        BufferParams {
            buffer_size: 8,
            color: Props { start: Vector3::new(1.0, 0.5, 0.0), ..Props::default() },
            opacity: props_f(1.0, 0.0, &[]),
            size,
            rotation: props_f(0.0, 0.0, &[]),
            orient_rnd: 0.0,
            frame: props_f(0.0, 0.0, &[]),
            blurtime: props_f(0.0, 0.0, &[]),
            accel: Vector3::default(),
            max_age: 2.0,
            future_start: 0.0,
            texture: Some(b"a.tga".to_vec()),
            shader: super::super::shader::PRESET_ADDITIVE_SPRITE_SHADER,
            pingpong: false,
            render_mode: W3D_EMITTER_RENDER_MODE_QUAD_PARTICLES,
            frame_mode: 0,
            line_props: None,
        }
    }

    /// Predicted before the run: a size going 1 -> 3 at 1 s with lifetime
    /// 2 s gives keyframe times 0 and 1000, values 1 and 3, deltas 0.002 and
    /// 0 (constant at the end), `MaxSize` 3; a keyframe at 2.5 s (past
    /// `MaxAge`) is cut and the last delta runs towards it: (5 - 3) / (2500 -
    /// 1000); `Find_POT` of 8 is 8, of 9 is 16, of 1 is 1; with no keyframes
    /// and no randomness the colour is a constant (no array).
    ///
    /// *Result (2026-11-07), first run:* held.
    #[test]
    fn keyframes_are_made_as_upstream() {
        let mut r = Random4::default();
        let clock = Ww3dClock::default();
        let b = ParticleBuffer::new(&params(props_f(1.0, 0.0, &[(1.0, 3.0)])), &mut r, &clock, &LodMaxScreenSizes::default()).unwrap();
        let k = &b.size_keys;
        assert_eq!((k.num, k.times.clone().unwrap(), k.values.clone().unwrap()), (2, vec![0, 1000], vec![1.0, 3.0]));
        assert_eq!(k.deltas.clone().unwrap(), vec![2.0 / 1000.0, 0.0]);
        assert_eq!(b.max_size, 3.0);
        assert!(b.color.is_none() && b.color_keys.times.is_none());
        let b = ParticleBuffer::new(&params(props_f(1.0, 0.0, &[(1.0, 3.0), (2.5, 5.0)])), &mut r, &clock, &LodMaxScreenSizes::default()).unwrap();
        assert_eq!(b.size_keys.num, 2);
        assert_eq!(b.size_keys.deltas.clone().unwrap()[1], 2.0f32 / 1500.0);
        assert_eq!((find_pot(8), find_pot(9), find_pot(1)), (8, 16, 1));
    }

    /// Predicted before the run: two particles queued at times 0 and 100
    /// with velocity (1, 0, 0) per ms, read at 500: positions 500 and 400
    /// along x, both alive; at 2100 the first (age 2100 >= 2000) is killed,
    /// the second is at 2000; the point group gets one point of the constant
    /// colour (1, 0.5, 0) and alpha 1.
    ///
    /// **Prediction missed (2026-11-07), first run:** at 2100 the second
    /// particle is 2000 ms old too, and a particle lives while its age is
    /// under `MaxAge` (`age < MaxAge`): both died. The read is now at 2050,
    /// where the second (age 1950) is at x 1950.
    #[test]
    fn particles_move_and_die() {
        let mut r = Random4::default();
        let mut b = ParticleBuffer::new(&params(props_f(1.0, 0.0, &[])), &mut r, &Ww3dClock::default(), &LodMaxScreenSizes::default()).unwrap();
        for t in [0, 100] {
            *b.add_uninitialized_new_particle() = NewParticle { velocity: Vector3::new(1.0, 0.0, 0.0), time_stamp: t, ..NewParticle::default() };
        }
        let c = Ww3dClock { sync_time: 500, frame_time: 500, frame_count: 1 };
        match b.render(&c, None).unwrap() {
            ParticleDraw::Points(p) => {
                assert_eq!(p.points.iter().map(|p| p.position.x).collect::<Vec<_>>(), vec![500.0, 400.0]);
            }
            d => panic!("{d:?}"),
        }
        let c = Ww3dClock { sync_time: 2050, frame_time: 1550, frame_count: 2 };
        match b.render(&c, None).unwrap() {
            ParticleDraw::Points(p) => {
                assert_eq!(p.points.len(), 1);
                assert_eq!(p.points[0].position.x, 1950.0);
                assert_eq!(p.points[0].diffuse, Vector4::new(1.0, 0.5, 0.0, 1.0));
            }
            d => panic!("{d:?}"),
        }
    }

    /// (#65, batch b) *Predictions, written before the first run (`issue65b_predictions.md`, P1):* a 40-particle quad
    /// buffer: cost factor 40 * 2 / 16 = 5, `Cost[k] = 5k` (`Cost[0]` the small nonzero cost); `Value[0]` `AT_MIN_LOD`,
    /// `Value[17]` `AT_MAX_LOD`, `Value[1] = (1 - 0.5 / 25) / 5`; level 16, threshold 0. `Set_LOD_Level(17)`: `Get_Cost`
    /// undefined, `Get_Value` is `Value[17]`. 17 `Decrement_LOD`s: threshold 17, level -1, `Get_Cost` undefined. Screen
    /// size 17 undefined; level 0's size under the area makes the minimum level 1. A hidden buffer prepares nothing; an
    /// empty one's area is 0 and it hands the optimizer `Cost[16]` = 80.
    ///
    /// *Results (2026-11-14), first run:* all held.
    #[test]
    fn lod_levels_costs_and_values_are_upstreams() {
        let mut r = Random4::default();
        let clock = Ww3dClock::default();
        let mut p = params(props_f(1.0, 0.0, &[]));
        p.buffer_size = 40;
        let mut sizes = LodMaxScreenSizes::default();
        let mut b = ParticleBuffer::new(&p, &mut r, &clock, &sizes).unwrap();
        assert_eq!((b.cost[0], b.cost[1], b.cost[16]), (0.000001, 5.0, 80.0));
        assert_eq!((b.value[0], b.value[17], b.value[1]), (AT_MIN_LOD, AT_MAX_LOD, (1.0f32 - 0.5 / 25.0) * 1.0 * 1.0 / 5.0));
        assert_eq!((b.get_lod_level(), b.decimation_threshold, b.get_lod_count()), (16, 0, 17));
        assert_eq!((b.get_cost().unwrap(), b.get_value().unwrap(), b.get_post_increment_value().unwrap()), (80.0, b.value[16], AT_MAX_LOD));
        b.set_lod_level(17);
        assert_eq!(b.decimation_threshold, u32::MAX);
        assert!(b.get_cost().is_err() && b.get_post_increment_value().is_err());
        assert_eq!(b.get_value().unwrap(), AT_MAX_LOD);
        b.set_lod_level(-4);
        assert_eq!(b.decimation_threshold, 16);
        b.increment_lod();
        assert_eq!(b.decimation_threshold, 15);
        for _ in 0..20 {
            b.decrement_lod();
        }
        assert_eq!((b.decimation_threshold, b.get_lod_level()), (17, -1));
        assert!(b.get_cost().is_err() && b.get_value().is_err());
        assert!(sizes.set(17, 1.0).is_err() && sizes.get(17).is_err());
        assert_eq!((sizes.set(18, 1.0), sizes.get(-1).unwrap()), (Ok(()), f32::MAX));
        sizes.set(0, 0.5).unwrap();
        let (mut v, mut c) = ([0.0f32; 18], [0.0f32; 17]);
        assert_eq!(b.calculate_cost_value_arrays(1.0, &sizes, &mut v, &mut c), 1);
        assert_eq!((v[0], v[1]), (AT_MIN_LOD, AT_MIN_LOD));
        b.set_lod_bias(-2.0);
        assert_eq!(b.lod_bias, 0.0);
        b.set_lod_level(16);
        let cam = LodCamera {
            position: Vector3::new(10.0, 0.0, 0.0),
            viewport_min: Vector2::new(0.0, 0.0),
            viewport_max: Vector2::new(800.0, 600.0),
            view_plane_min: Vector2::new(-1.0, -1.0),
            view_plane_max: Vector2::new(1.0, 1.0),
        };
        b.set_hidden(true);
        assert!(!b.is_not_hidden_at_all());
        assert_eq!(b.prepare_lod(&cam, &LodMaxScreenSizes::default(), &clock).unwrap(), None);
        b.set_hidden(false);
        b.set_visible(true);
        b.set_animation_hidden(false);
        assert_eq!(b.prepare_lod(&cam, &LodMaxScreenSizes::default(), &clock).unwrap(), Some(80.0));
        assert_eq!((b.projected_area, b.bounding_volumes_valid), (0.0, true));
        assert!(!b.on_frame_update() && !b.bounding_volumes_valid);
    }

    /// (#65) *Predictions, written before the first run:* a line-mode buffer of particle size 1 made at sync 300 has a
    /// line renderer of width 1 with the line properties' flags, the buffer's texture and fogged shader, its UV clock at
    /// 300; `Scale(2)` doubles the width and the noise; two particles at x 0 and 400 (after 500 ms) draw one run, which
    /// `render_line_runs` turns into one chunk of 4 vertices and 2 triangles with the texture; a buffer of points has
    /// none (`None`).
    ///
    /// *Results (2026-11-14), first run:* all held.
    #[test]
    fn line_mode_draws_through_the_segment_renderer() {
        use super::super::seglinerenderer::W3D_ELINE_MERGE_INTERSECTIONS;
        let mut r = Random4::default();
        let mut p = params(props_f(1.0, 0.0, &[]));
        p.render_mode = W3D_EMITTER_RENDER_MODE_LINE;
        let lp = W3dEmitterLinePropertiesStruct {
            flags: W3D_ELINE_MERGE_INTERSECTIONS,
            subdivision_level: 0,
            noise_amplitude: 0.5,
            merge_abort_factor: 1.5,
            texture_tile_factor: 1.0,
            u_per_sec: 0.0,
            v_per_sec: 0.0,
            reserved: [0; 9],
        };
        p.line_props = Some(lp);
        let clock = Ww3dClock { sync_time: 300, frame_time: 0, frame_count: 0 };
        let mut b = ParticleBuffer::new(&p, &mut r, &clock, &LodMaxScreenSizes::default()).unwrap();
        let lr = b.line_renderer.clone().unwrap();
        assert_eq!((lr.width, lr.last_used_sync_time, lr.bits & 0xFF), (1.0, 300, 1));
        assert_eq!((lr.texture.clone(), lr.shader), (b.texture.clone(), b.shader));
        b.scale(2.0);
        let lr = b.line_renderer.clone().unwrap();
        assert_eq!((lr.width, lr.noise_amplitude), (2.0, 1.0));
        for t in [300, 400] {
            *b.add_uninitialized_new_particle() = NewParticle { velocity: Vector3::new(1.0, 0.0, 0.0), time_stamp: t, ..NewParticle::default() };
        }
        let c = Ww3dClock { sync_time: 800, frame_time: 500, frame_count: 1 };
        let ParticleDraw::Lines { width, runs } = b.render(&c, None).unwrap() else { panic!("not lines") };
        assert_eq!((width, runs.len()), (2.0, 1));
        let mut view = Matrix3D::identity();
        view.row[2].w = -100.0;
        let chunks = b.render_line_runs(&runs, &c, &view, &mut Random3::default()).unwrap().unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!((chunks[0].len(), chunks[0][0].vertices.len(), chunks[0][0].triangles.len()), (1, 4, 2));
        assert_eq!(chunks[0][0].texture, Some(b"a.tga".to_vec()));
        let mut q = ParticleBuffer::new(&params(props_f(1.0, 0.0, &[])), &mut r, &clock, &LodMaxScreenSizes::default()).unwrap();
        assert_eq!(q.render_line_runs(&runs, &c, &view, &mut Random3::default()).unwrap(), None);
    }
}
