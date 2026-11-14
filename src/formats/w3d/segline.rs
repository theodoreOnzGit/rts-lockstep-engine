// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/segline.cpp, segline.h
//   (SegmentedLineClass: the constructors, operator=, Reset_Line, Set_Points,
//   Get_Num_Points, Set_Point_Location, Get_Point_Location, Add_Point,
//   Delete_Point, the Get_* and Set_* line properties, Get_Num_Polys, Render,
//   Get_Obj_Space_Bounding_Sphere, Get_Obj_Space_Bounding_Box, Prepare_LOD,
//   Increment_LOD, Decrement_LOD, Get_Cost, Get_Value,
//   Get_Post_Increment_Value, Set_LOD_Level, Get_LOD_Level, Get_LOD_Count,
//   Render_Seg_Line)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.h, rendobj.cpp
//   (what SegmentedLineClass uses of RenderObjClass: Bits and its
//   DEFAULT_BITS, Transform, the constructor, operator=, Set_Visible,
//   Is_Visible, Set_Hidden, Is_Hidden, Set_Animation_Hidden,
//   Is_Animation_Hidden, Set_Force_Visible, Is_Force_Visible,
//   Get_Collision_Type, Set_Collision_Type, Is_Not_Hidden_At_All,
//   Invalidate_Cached_Bounding_Volumes, AT_MIN_LOD, AT_MAX_LOD)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aabox.h
//   (AABoxClass::Init_Min_Max)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, segline by Naty Hoffman and Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SegmentedLineClass` (GitHub #169, #65): the render object of a thick
//! segmented line, which `W3DLaserDraw` draws its beams with
//! ([`crate::client::w3d_laser_draw`]). It keeps the line's points and hands
//! them to its [`SegLineRenderer`] (`LineRenderer`), which holds every other
//! setting and builds the strip.
//!
//! # As upstream does it
//!
//! - **Made** (`SegmentedLineClass()`): no subdivision levels, a normalized
//!   screen area of 0, no points, a renderer as its constructor makes it, and
//!   the render object's defaults (`RenderObjClass()`: `DEFAULT_BITS`, which
//!   is every collision type, not hidden, not animation-hidden and **not
//!   visible** (`IS_VISIBLE` is the scene's visibility pass's to set), and the
//!   identity transform).
//! - **The copy** (`SegmentedLineClass(src)`): the line's state copied, the
//!   render object made afresh (the copy constructor's list does not name
//!   `RenderObjClass`, so its default constructor runs). **The assignment**
//!   (`operator=`): `RenderObjClass::operator=` (the hidden, animation-hidden,
//!   force-visible and collision bits copied) then the line's state.
//! - **The points**: `Set_Points` needs at least two (else a debug assertion
//!   and nothing) and replaces them; `Set_Point_Location` and
//!   `Get_Point_Location` act within the list (the getter answers 0, 0, 0
//!   past it); `Add_Point` appends; `Delete_Point` removes one within the
//!   list, the rest moved down. The ones that change the extent invalidate
//!   the cached bounding volumes.
//! - **The settings** go to the renderer as given but: `Set_Width` keeps
//!   `MAX(width, 0)`, `Set_Noise_Amplitude` keeps `Fabs(amplitude)`;
//!   `Set_Subdivision_Levels` keeps its own maximum, at most
//!   `MAX_SEGLINE_SUBDIV_LEVELS`, and the renderer's current level is the LOD
//!   code's: `Prepare_LOD` clamps it to the maximum, `Increment_LOD` and
//!   `Decrement_LOD` step it within 0 and the maximum, `Set_LOD_Level` sets
//!   it clamped.
//! - **`Render`**: nothing for a hidden line (`Is_Not_Hidden_At_All`; the
//!   visibility bit is not read). With sorting on (always, in the game) the
//!   sort level is `SORT_LEVEL_NONE` and `Render_Seg_Line` draws at once:
//!   nothing for fewer than two points, else the renderer's `Render` with the
//!   render object's transform, the points, the object-space bounding sphere
//!   and no per-point colours.
//! - **The bounding box**: for two points or more, the points' box grown by
//!   half the width; with subdivision levels, also the box of the segments'
//!   midpoints grown by half the width plus twice the noise amplitude, and
//!   the two combined. Fewer than two points: centre 0, extent 1. **The
//!   sphere**: the box's centre, the length of its extent.
//! - **The LOD**: `Get_Num_Polys` is `2 * (points - 1) * 2^level` (an `int`);
//!   the cost is that; the value is `AT_MIN_LOD` at level 0, else `(1 -
//!   0.5 / polys^2) * area / cost`; the value after an increment is
//!   `AT_MAX_LOD` at the maximum level, else the same with the polygons
//!   doubled.
//!
//! # Upstream quirks that are kept
//!
//! - `Add_Point` and `Delete_Point` do not invalidate the cached bounding
//!   volumes (`Set_Points`, `Set_Point_Location`, `Set_Width`,
//!   `Set_Noise_Amplitude` and `Set_Subdivision_Levels` do).
//! - The bounding box's noise term ignores the number of levels (twice the
//!   amplitude, the limit as the levels grow).
//! - `Get_Num_Polys` of a line without points is `-2` (`Count() - 1` is an
//!   `int`).
//!
//! # Undefined upstream
//!
//! - **The copy and the assignment share the points** (#169, found by the
//!   differential test's first run, which aborted on a double delete):
//!   `SimpleDynVecClass` has no copy constructor nor assignment, so the
//!   memberwise copies of `PointLocations` share the source's array; the two
//!   lines alias it until one grows it (freeing it under the other), and the
//!   second destructor deletes it again. Nothing in the game copies a line.
//!   [`SegmentedLine::copy`] and [`SegmentedLine::assign`] give an
//!   independent copy (the evident intent); the differential test detaches
//!   the source's array before releasing it so that the C++ copy owns it
//!   alone.
//! - **A texture mapping mode other than the three**
//!   ([`SegmentedLine::set_texture_mapping_mode`] stores any): the renderer's
//!   `Render` reads `u_values` and `base_tex_v` uninitialised
//!   ([`super::W3dUndefined::UninitialisedRead`] from [`SegmentedLine::render`]).
//!
//! # Where this differs from upstream
//!
//! - **The device** is replaced: `Render` returns the renderer's chunks
//!   ([`SegLineChunk`]) for the client to draw. The bounding sphere upstream
//!   hands the renderer is only the sorting renderer's key for the triangles
//!   (`SortingRendererClass::Insert_Triangles`): the client sorts its own way
//!   (`client/src/particles.rs`), so it is not passed;
//!   [`SegmentedLine::get_obj_space_bounding_sphere`] gives it.
//! - **The view** is the caller's (`DX8Wrapper::Get_Transform(D3DTS_VIEW)`,
//!   upstream the camera's, set by the scene's render), as is the WW3D sync
//!   time and the randomizers' shared generator.
//! - **`Prepare_LOD`** takes the screen size its camera gives
//!   (`RenderObjClass::Get_Screen_Size(camera)`) and returns what it asks of
//!   the predictive LOD optimizer ([`LodRequest`]).
//! - **Not translated** (gaps of #169): `Cast_Ray` (the port has no
//!   `LineSegClass` nor ray test), `Clone`'s reference counting (here
//!   [`SegmentedLine::copy`]), `Class_ID` (`CLASSID_SEGLINE`), the scene's
//!   static sort list (never reached in the game: sorting is never turned
//!   off).

use super::part_buf::Sphere;
use super::seglinerenderer::{SegLineChunk, SegLineRenderer, MAX_SEGLINE_SUBDIV_LEVELS};
use super::shader::ShaderClass;
use super::W3dResult;
use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::random::Random3;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `RenderObjClass`'s `Bits` (rendobj.h).
pub const COLL_TYPE_MASK: u32 = 0x0000_00FF;
pub const IS_VISIBLE: u32 = 0x0000_0100;
pub const IS_NOT_HIDDEN: u32 = 0x0000_0200;
pub const IS_NOT_ANIMATION_HIDDEN: u32 = 0x0000_0400;
pub const IS_FORCE_VISIBLE: u32 = 0x0000_0800;
pub const BOUNDING_VOLUMES_VALID: u32 = 0x0000_2000;
/// `COLL_TYPE_ALL` (coltype.h).
pub const COLL_TYPE_ALL: u32 = 0x01;
/// `IS_NOT_HIDDEN_AT_ALL`.
pub const IS_NOT_HIDDEN_AT_ALL: u32 = IS_NOT_HIDDEN | IS_NOT_ANIMATION_HIDDEN;
/// `DEFAULT_BITS`: every collision type, not hidden; not visible.
pub const DEFAULT_BITS: u32 = COLL_TYPE_ALL | IS_NOT_HIDDEN | IS_NOT_ANIMATION_HIDDEN;

/// `RenderObjClass::AT_MIN_LOD` (rendobj.cpp: `FLT_MAX`).
pub const AT_MIN_LOD: f32 = f32::MAX;
/// `RenderObjClass::AT_MAX_LOD` (rendobj.cpp: `-1.0f`).
pub const AT_MAX_LOD: f32 = -1.0;

/// `SORT_LEVEL_NONE` (w3d_file.h).
const SORT_LEVEL_NONE: i32 = 0;
/// `WW3D::Is_Sorting_Enabled()`: `IsSortingEnabled` starts true (ww3d.cpp)
/// and nothing in the game calls `WW3D::Enable_Sorting`.
const WW3D_IS_SORTING_ENABLED: bool = true;

/// `AABoxClass`: centre and extent (half the sides).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AABox {
    pub center: Vector3,
    pub extent: Vector3,
}

impl AABox {
    /// `Init_Min_Max(min, max)` (aabox.h): the centre `(max + min) * 0.5`,
    /// the extent `(max - min) * 0.5`.
    pub fn init_min_max(min: &Vector3, max: &Vector3) -> AABox {
        AABox { center: (*max + *min) * 0.5f32, extent: (*max - *min) * 0.5f32 }
    }
}

/// What `Prepare_LOD` asks of `PredictiveLODOptimizerClass`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LodRequest {
    /// `Add_Object(this)`: the line has subdivision levels to choose from.
    AddObject,
    /// `Add_Cost(Get_Cost())`: it has none.
    AddCost(f32),
}

/// `class SegmentedLineClass` and the parts of its `RenderObjClass` it uses.
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentedLine {
    /// `RenderObjClass::Bits`.
    pub bits: u32,
    /// `RenderObjClass::Transform` (object space to the world).
    pub transform: Matrix3D,
    /// `MaxSubdivisionLevels`.
    pub max_subdivision_levels: u32,
    /// `NormalizedScreenArea` (for LOD).
    pub normalized_screen_area: f32,
    /// `PointLocations` (object space).
    pub point_locations: Vec<Vector3>,
    /// `LineRenderer`: every other setting.
    pub line_renderer: SegLineRenderer,
}

impl SegmentedLine {
    /// `SegmentedLineClass()` at the WW3D sync time `sync_time` (the
    /// renderer's `LastUsedSyncTime`), with `RenderObjClass()`'s bits and
    /// transform.
    pub fn new(sync_time: u32) -> Self {
        SegmentedLine {
            bits: DEFAULT_BITS,
            transform: Matrix3D::identity(),
            max_subdivision_levels: 0,
            normalized_screen_area: 0.0,
            point_locations: Vec::new(),
            line_renderer: SegLineRenderer::new(sync_time),
        }
    }

    /// `SegmentedLineClass(const SegmentedLineClass & src)` (what `Clone`
    /// makes): the line's state copied; the render object default-made (the
    /// copy constructor does not name its base).
    pub fn copy(src: &SegmentedLine) -> Self {
        SegmentedLine {
            bits: if plant(10195) { src.bits } else { DEFAULT_BITS },
            transform: Matrix3D::identity(),
            max_subdivision_levels: src.max_subdivision_levels,
            normalized_screen_area: src.normalized_screen_area,
            point_locations: src.point_locations.clone(),
            line_renderer: src.line_renderer.clone(),
        }
    }

    /// `operator=(that)`: `RenderObjClass::operator=` (rendobj.cpp: the hidden,
    /// animation-hidden and force-visible bits and the collision type), then
    /// the line's state.
    pub fn assign(&mut self, that: &SegmentedLine) {
        self.set_hidden(that.is_hidden());
        self.set_animation_hidden(that.is_animation_hidden());
        self.set_force_visible(that.is_force_visible());
        self.set_collision_type(that.get_collision_type());
        self.max_subdivision_levels = that.max_subdivision_levels;
        self.normalized_screen_area = that.normalized_screen_area;
        self.point_locations = that.point_locations.clone();
        self.line_renderer = that.line_renderer.clone();
    }

    // ---- RenderObjClass (rendobj.h's inline accessors) ----

    /// `Is_Visible()`.
    pub fn is_visible(&self) -> bool {
        self.bits & IS_VISIBLE != 0
    }
    /// `Set_Visible(onoff)`.
    pub fn set_visible(&mut self, onoff: bool) {
        if onoff {
            self.bits |= IS_VISIBLE;
        } else {
            self.bits &= !IS_VISIBLE;
        }
    }
    /// `Is_Hidden()`.
    pub fn is_hidden(&self) -> bool {
        self.bits & IS_NOT_HIDDEN == 0
    }
    /// `Set_Hidden(onoff)`.
    pub fn set_hidden(&mut self, onoff: bool) {
        if onoff {
            self.bits &= !IS_NOT_HIDDEN;
        } else {
            self.bits |= IS_NOT_HIDDEN;
        }
    }
    /// `Is_Animation_Hidden()`.
    pub fn is_animation_hidden(&self) -> bool {
        self.bits & IS_NOT_ANIMATION_HIDDEN == 0
    }
    /// `Set_Animation_Hidden(onoff)`.
    pub fn set_animation_hidden(&mut self, onoff: bool) {
        if onoff {
            self.bits &= !IS_NOT_ANIMATION_HIDDEN;
        } else {
            self.bits |= IS_NOT_ANIMATION_HIDDEN;
        }
    }
    /// `Is_Force_Visible()`.
    pub fn is_force_visible(&self) -> bool {
        self.bits & IS_FORCE_VISIBLE != 0
    }
    /// `Set_Force_Visible(onoff)`.
    pub fn set_force_visible(&mut self, onoff: bool) {
        if onoff {
            self.bits |= IS_FORCE_VISIBLE;
        } else {
            self.bits &= !IS_FORCE_VISIBLE;
        }
    }
    /// `Get_Collision_Type()`.
    pub fn get_collision_type(&self) -> u32 {
        self.bits & COLL_TYPE_MASK
    }
    /// `Set_Collision_Type(type)`: `COLL_TYPE_ALL` always kept.
    pub fn set_collision_type(&mut self, kind: u32) {
        self.bits &= !COLL_TYPE_MASK;
        self.bits |= (kind & COLL_TYPE_MASK) | COLL_TYPE_ALL;
    }
    /// `Is_Not_Hidden_At_All()`: neither hidden nor animation-hidden.
    pub fn is_not_hidden_at_all(&self) -> bool {
        self.bits & IS_NOT_HIDDEN_AT_ALL == IS_NOT_HIDDEN_AT_ALL
    }
    /// `Invalidate_Cached_Bounding_Volumes()`.
    fn invalidate_cached_bounding_volumes(&mut self) {
        self.bits &= !BOUNDING_VOLUMES_VALID;
    }

    // ---- SegmentedLineClass ----

    /// `Reset_Line()`: the renderer's (the UV offset back to 0 at `sync_time`).
    pub fn reset_line(&mut self, sync_time: u32) {
        self.line_renderer.reset_line(sync_time);
    }

    /// `Set_Points(num_points, locs)`: at least two points (else a debug
    /// assertion and nothing), which replace the line's.
    pub fn set_points(&mut self, locs: &[Vector3]) {
        let least = if plant(10176) { 1 } else { 2 };
        if locs.len() < least {
            // WWASSERT(0)
            return;
        }
        self.point_locations.clear();
        for l in locs {
            self.point_locations.push(*l);
        }
        self.invalidate_cached_bounding_volumes();
    }

    /// `Get_Num_Points()`.
    pub fn get_num_points(&self) -> i32 {
        self.point_locations.len() as i32
    }

    /// `Set_Point_Location(point_idx, location)`: nothing past the list.
    pub fn set_point_location(&mut self, point_idx: u32, location: &Vector3) {
        let count = self.point_locations.len() as u32;
        let limit = if plant(10177) { count.saturating_sub(1) } else { count };
        if point_idx < limit {
            self.point_locations[point_idx as usize] = *location;
        }
        self.invalidate_cached_bounding_volumes();
    }

    /// `Get_Point_Location(point_idx, loc)`: 0, 0, 0 past the list.
    pub fn get_point_location(&self, point_idx: u32) -> Vector3 {
        if point_idx < self.point_locations.len() as u32 {
            self.point_locations[point_idx as usize]
        } else if plant(10178) && !self.point_locations.is_empty() {
            self.point_locations[self.point_locations.len() - 1]
        } else {
            Vector3::new(0.0, 0.0, 0.0)
        }
    }

    /// `Add_Point(location)`.
    pub fn add_point(&mut self, location: &Vector3) {
        self.point_locations.push(*location);
    }

    /// `Delete_Point(point_idx)`: within the list, the rest moved down
    /// (`SimpleDynVecClass::Delete`).
    pub fn delete_point(&mut self, point_idx: u32) {
        if point_idx < self.point_locations.len() as u32 {
            if plant(10179) {
                self.point_locations.swap_remove(point_idx as usize);
            } else {
                self.point_locations.remove(point_idx as usize);
            }
        }
    }

    /// `Get_Texture()`: the renderer's (its name; `None` for `NULL`).
    pub fn get_texture(&self) -> Option<&[u8]> {
        self.line_renderer.texture.as_deref()
    }
    /// `Get_Shader()`.
    pub fn get_shader(&self) -> ShaderClass {
        self.line_renderer.shader
    }
    /// `Get_Width()`.
    pub fn get_width(&self) -> f32 {
        self.line_renderer.width
    }
    /// `Get_Color(color)`.
    pub fn get_color(&self) -> Vector3 {
        self.line_renderer.color
    }
    /// `Get_Opacity()`.
    pub fn get_opacity(&self) -> f32 {
        self.line_renderer.opacity
    }
    /// `Get_Noise_Amplitude()`.
    pub fn get_noise_amplitude(&self) -> f32 {
        self.line_renderer.noise_amplitude
    }
    /// `Get_Merge_Abort_Factor()`.
    pub fn get_merge_abort_factor(&self) -> f32 {
        self.line_renderer.merge_abort_factor
    }
    /// `Get_Subdivision_Levels()`: the maximum (`MaxSubdivisionLevels`), not
    /// the renderer's current level.
    pub fn get_subdivision_levels(&self) -> u32 {
        self.max_subdivision_levels
    }
    /// `Get_Texture_Mapping_Mode()`.
    pub fn get_texture_mapping_mode(&self) -> u32 {
        self.line_renderer.get_texture_mapping_mode()
    }
    /// `Get_Texture_Tile_Factor()`.
    pub fn get_texture_tile_factor(&self) -> f32 {
        self.line_renderer.texture_tile_factor
    }
    /// `Get_UV_Offset_Rate()`: per second (`UVOffsetDeltaPerMS * 1000`).
    pub fn get_uv_offset_rate(&self) -> Vector2 {
        self.line_renderer.get_uv_offset_rate()
    }
    /// `Is_Merge_Intersections()`.
    pub fn is_merge_intersections(&self) -> bool {
        self.line_renderer.is_merge_intersections()
    }
    /// `Is_Freeze_Random()`.
    pub fn is_freeze_random(&self) -> bool {
        self.line_renderer.is_freeze_random()
    }
    /// `Is_Sorting_Disabled()`.
    pub fn is_sorting_disabled(&self) -> bool {
        self.line_renderer.is_sorting_disabled()
    }
    /// `Are_End_Caps_Enabled()`.
    pub fn are_end_caps_enabled(&self) -> bool {
        self.line_renderer.are_end_caps_enabled()
    }

    /// `Set_Texture(texture)`: its name, `None` for `NULL`.
    pub fn set_texture(&mut self, texture: Option<Vec<u8>>) {
        self.line_renderer.texture = texture;
    }
    /// `Set_Shader(shader)`.
    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.line_renderer.shader = shader;
    }
    /// `Set_Width(width)`: `MAX(width, 0.0f)` ("Widths need to be clamped
    /// because they are not automatically clamped later").
    pub fn set_width(&mut self, width: f32) {
        self.line_renderer.width = if plant(10180) || width > 0.0f32 { width } else { 0.0f32 };
        self.invalidate_cached_bounding_volumes();
    }
    /// `Set_Color(color)`.
    pub fn set_color(&mut self, color: &Vector3) {
        self.line_renderer.color = *color;
    }
    /// `Set_Opacity(opacity)`.
    pub fn set_opacity(&mut self, opacity: f32) {
        self.line_renderer.opacity = opacity;
    }
    /// `Set_Noise_Amplitude(amplitude)`: `WWMath::Fabs(amplitude)`.
    pub fn set_noise_amplitude(&mut self, amplitude: f32) {
        self.line_renderer.noise_amplitude = if plant(10181) { amplitude } else { wwmath::fabs(amplitude) };
        self.invalidate_cached_bounding_volumes();
    }
    /// `Set_Merge_Abort_Factor(factor)`.
    pub fn set_merge_abort_factor(&mut self, factor: f32) {
        self.line_renderer.merge_abort_factor = factor;
    }
    /// `Set_Subdivision_Levels(levels)`: the maximum, at most
    /// `MAX_SEGLINE_SUBDIV_LEVELS`.
    pub fn set_subdivision_levels(&mut self, levels: u32) {
        let cap = if plant(10182) { MAX_SEGLINE_SUBDIV_LEVELS - 1 } else { MAX_SEGLINE_SUBDIV_LEVELS };
        self.max_subdivision_levels = levels.min(cap);
        self.invalidate_cached_bounding_volumes();
    }
    /// `Set_Texture_Mapping_Mode(mode)`.
    pub fn set_texture_mapping_mode(&mut self, mode: u32) {
        self.line_renderer.set_texture_mapping_mode(mode);
    }
    /// `Set_Texture_Tile_Factor(factor)`: the renderer's clamp (at most 50,
    /// `MAX(factor, 0)`).
    pub fn set_texture_tile_factor(&mut self, factor: f32) {
        self.line_renderer.set_texture_tile_factor(factor);
    }
    /// `Set_UV_Offset_Rate(rate)`: kept per millisecond.
    pub fn set_uv_offset_rate(&mut self, rate: &Vector2) {
        self.line_renderer.set_uv_offset_rate(*rate);
    }
    /// `Set_Merge_Intersections(onoff)`.
    pub fn set_merge_intersections(&mut self, onoff: bool) {
        self.line_renderer.set_merge_intersections(onoff);
    }
    /// `Set_Freeze_Random(onoff)`.
    pub fn set_freeze_random(&mut self, onoff: bool) {
        self.line_renderer.set_freeze_random(onoff);
    }
    /// `Set_Disable_Sorting(onoff)`.
    pub fn set_disable_sorting(&mut self, onoff: bool) {
        self.line_renderer.set_disable_sorting(onoff);
    }
    /// `Set_End_Caps(onoff)`.
    pub fn set_end_caps(&mut self, onoff: bool) {
        self.line_renderer.set_end_caps(onoff);
    }

    /// `Get_Num_Polys()`: `2 * (points - 1) * (1 << level)`, an `int` (-2
    /// without points).
    pub fn get_num_polys(&self) -> i32 {
        let level = self.line_renderer.subdivision_level;
        let subdivision_factor: i32 = if plant(10183) { level.max(1) as i32 } else { 1i32.wrapping_shl(level) };
        2i32.wrapping_mul(self.get_num_points().wrapping_sub(1)).wrapping_mul(subdivision_factor)
    }

    /// `Render(rinfo)` at the WW3D sync time `sync_time`, with the camera's
    /// view `view` (`DX8Wrapper::Get_Transform(D3DTS_VIEW)`) and the
    /// randomizers' shared generator `v3`: nothing for a hidden line; with
    /// sorting on (always in the game) `Render_Seg_Line` at once.
    pub fn render(&mut self, sync_time: u32, view: &Matrix3D, v3: &mut Random3) -> W3dResult<Vec<SegLineChunk>> {
        let shown = if plant(10184) { self.is_not_hidden_at_all() && self.is_visible() } else { self.is_not_hidden_at_all() };
        if !shown {
            return Ok(Vec::new());
        }
        let mut sort_level = SORT_LEVEL_NONE;
        if !WW3D_IS_SORTING_ENABLED {
            sort_level = self.get_shader().guess_sort_level();
        }
        // `WW3D::Are_Static_Sort_Lists_Enabled() && sort_level != SORT_LEVEL_NONE` puts the line on the static sort
        // list, drawn later through the same `Render_Seg_Line`: never here (the sort level is always NONE)
        let _ = sort_level;
        self.render_seg_line(sync_time, view, v3)
    }

    /// `Get_Obj_Space_Bounding_Sphere(sphere)`: the bounding box's centre and
    /// the length of its extent.
    pub fn get_obj_space_bounding_sphere(&self) -> Sphere {
        let b = self.get_obj_space_bounding_box();
        let radius = if plant(10185) { b.extent.x } else { b.extent.length() };
        Sphere { center: b.center, radius }
    }

    /// `Get_Obj_Space_Bounding_Box(box)`: the points' box grown by half the
    /// width, and with subdivision levels the midpoints' box grown by half
    /// the width plus twice the noise amplitude, combined; fewer than two
    /// points, centre 0 and extent 1.
    pub fn get_obj_space_bounding_box(&self) -> AABox {
        let num_points = self.point_locations.len();
        if num_points >= 2 {
            let p = &self.point_locations;
            // First bounding box:
            let mut max_coords = p[0];
            let mut min_coords = p[0];
            for q in &p[1..num_points] {
                max_coords.update_max(q);
                min_coords.update_min(q);
            }
            // Enlarge bounding box by half the width
            let mut enlarge_factor = if plant(10186) { self.line_renderer.width } else { self.line_renderer.width * 0.5f32 };
            let mut enlarge_offset = Vector3::new(enlarge_factor, enlarge_factor, enlarge_factor);
            max_coords += enlarge_offset;
            min_coords -= enlarge_offset;
            if self.max_subdivision_levels > 0 {
                // Second bounding box:
                let mut midpoint = (p[0] + p[1]) * 0.5f32;
                let mut max_coords2 = midpoint;
                let mut min_coords2 = midpoint;
                let end = if plant(10188) { num_points - 2 } else { num_points - 1 };
                for i in 1..end {
                    midpoint = (p[i] + p[i + 1]) * 0.5f32;
                    max_coords2.update_max(&midpoint);
                    min_coords2.update_min(&midpoint);
                }
                // (the noise amplitude times 2: the limit as the number of subdivision levels goes to infinity)
                enlarge_factor += if plant(10187) { self.line_renderer.noise_amplitude } else { 2.0f32 * self.line_renderer.noise_amplitude };
                enlarge_offset = Vector3::new(enlarge_factor, enlarge_factor, enlarge_factor);
                max_coords2 += enlarge_offset;
                min_coords2 -= enlarge_offset;
                // Combine the two:
                max_coords.update_max(&max_coords2);
                min_coords.update_min(&min_coords2);
            }
            AABox::init_min_max(&min_coords, &max_coords)
        } else {
            // Invalid line - return something
            AABox { center: Vector3::new(0.0, 0.0, 0.0), extent: Vector3::new(1.0, 1.0, 1.0) }
        }
    }

    /// `Prepare_LOD(camera)`, `screen_size` its `Get_Screen_Size(camera)`: the
    /// normalized screen area kept, the renderer's level clamped to the
    /// maximum, and the line added to the LOD optimizer when it has levels
    /// (else its cost).
    pub fn prepare_lod(&mut self, screen_size: f32) -> LodRequest {
        self.normalized_screen_area = screen_size;
        let mut lvl = self.line_renderer.subdivision_level;
        if !plant(10189) {
            lvl = lvl.min(self.max_subdivision_levels);
        }
        self.line_renderer.subdivision_level = lvl;
        if self.max_subdivision_levels > 0 {
            LodRequest::AddObject
        } else {
            LodRequest::AddCost(self.get_cost())
        }
    }

    /// `Increment_LOD()`: one level more, at most the maximum.
    pub fn increment_lod(&mut self) {
        let lvl = self.line_renderer.subdivision_level;
        let lvl = if plant(10190) { lvl } else { (lvl + 1).min(self.max_subdivision_levels) };
        self.line_renderer.subdivision_level = lvl;
    }

    /// `Decrement_LOD()`: one level less, none below 0.
    pub fn decrement_lod(&mut self) {
        let lvl = self.line_renderer.subdivision_level;
        let floor = if plant(10191) { 1 } else { 0 };
        if lvl <= floor {
            return;
        }
        self.line_renderer.subdivision_level = lvl - 1;
    }

    /// `Get_Cost()`: `Get_Num_Polys()`.
    pub fn get_cost(&self) -> f32 {
        self.get_num_polys() as f32
    }

    /// `Get_Value()`: `AT_MIN_LOD` at level 0, else `(1 - 0.5 / polys^2) *
    /// area / cost`.
    pub fn get_value(&self) -> f32 {
        // If we are at the minimum LOD, we must return AT_MIN_LOD.
        if self.line_renderer.subdivision_level == 0 {
            AT_MIN_LOD
        } else {
            let polycount = self.get_num_polys() as f32;
            let benefit_factor = if plant(10192) { 1.0f32 - (0.5f32 / polycount) } else { 1.0f32 - (0.5f32 / (polycount * polycount)) };
            (benefit_factor * self.normalized_screen_area) / self.get_cost()
        }
    }

    /// `Get_Post_Increment_Value()`: `AT_MAX_LOD` at the maximum level, else
    /// the value with the polygons doubled.
    pub fn get_post_increment_value(&self) -> f32 {
        // If we are at the maximum LOD, we must return AT_MIN_LOD.
        if self.line_renderer.subdivision_level == self.max_subdivision_levels {
            AT_MAX_LOD
        } else {
            // Assumption: each subdivision level doubles polycount
            let polycount = if plant(10193) { self.get_num_polys() as f32 } else { 2.0f32 * self.get_num_polys() as f32 };
            let benefit_factor = 1.0f32 - (0.5f32 / (polycount * polycount));
            // Assumption: Cost() == polycount
            (benefit_factor * self.normalized_screen_area) / polycount
        }
    }

    /// `Set_LOD_Level(lod)`: clamped to 0 and the maximum.
    pub fn set_lod_level(&mut self, lod: i32) {
        let mut lod = if plant(10194) { lod } else { lod.max(0) };
        lod = lod.min(self.max_subdivision_levels as i32);
        self.line_renderer.subdivision_level = lod as u32;
    }

    /// `Get_LOD_Level()`.
    pub fn get_lod_level(&self) -> i32 {
        self.line_renderer.subdivision_level as i32
    }

    /// `Get_LOD_Count()`: the maximum.
    pub fn get_lod_count(&self) -> i32 {
        self.max_subdivision_levels as i32
    }

    /// `Render_Seg_Line(rinfo)`: nothing for fewer than two points, else the
    /// renderer's `Render` with the transform, the points and no colours (the
    /// bounding sphere it is also given is the sorting renderer's key: module
    /// docs).
    fn render_seg_line(&mut self, sync_time: u32, view: &Matrix3D, v3: &mut Random3) -> W3dResult<Vec<SegLineChunk>> {
        // Line must have at least two points to be valid
        if self.point_locations.len() < 2 {
            return Ok(Vec::new());
        }
        self.line_renderer.render(sync_time, view, &self.transform, &self.point_locations, None, v3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::seglinerenderer::TILED_TEXTURE_MAP;
    use crate::formats::w3d::shader::PRESET_ADDITIVE_SHADER;

    fn v(x: f32, y: f32, z: f32) -> Vector3 {
        Vector3::new(x, y, z)
    }

    /// Predicted: made, the line has no points, not visible (`DEFAULT_BITS`), the identity transform, width 0, white; the
    /// box of an invalid line is centre 0 extent 1; two points at width 2 give the points' box grown by 1 and the
    /// sphere its centre and extent length; Set_Points with one point does nothing.
    #[test]
    fn made_points_and_bounds() {
        let mut l = SegmentedLine::new(5);
        assert_eq!(l.get_num_points(), 0);
        assert!(!l.is_visible());
        assert!(l.is_not_hidden_at_all());
        assert_eq!(l.get_collision_type(), COLL_TYPE_ALL);
        assert_eq!(l.get_obj_space_bounding_box(), AABox { center: v(0.0, 0.0, 0.0), extent: v(1.0, 1.0, 1.0) });
        l.set_points(&[v(1.0, 1.0, 1.0)]);
        assert_eq!(l.get_num_points(), 0);
        l.set_width(2.0);
        l.set_points(&[v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0)]);
        let b = l.get_obj_space_bounding_box();
        assert_eq!(b, AABox { center: v(2.0, 0.0, 0.0), extent: v(3.0, 1.0, 1.0) });
        let s = l.get_obj_space_bounding_sphere();
        assert_eq!(s.center, v(2.0, 0.0, 0.0));
        assert_eq!(s.radius, v(3.0, 1.0, 1.0).length());
        // with levels the midpoints' box (grown by 1 + 2 * noise) joins: midpoint (2, 0, 0), noise 3: extent 7 about it
        l.set_subdivision_levels(2);
        l.set_noise_amplitude(-3.0);
        assert_eq!(l.get_noise_amplitude(), 3.0);
        let b = l.get_obj_space_bounding_box();
        assert_eq!(b.extent, v(7.0, 7.0, 7.0));
        assert_eq!(l.get_point_location(1), v(4.0, 0.0, 0.0));
        assert_eq!(l.get_point_location(2), v(0.0, 0.0, 0.0));
        l.set_point_location(0, &v(-1.0, 0.0, 0.0));
        l.add_point(&v(9.0, 9.0, 9.0));
        l.delete_point(1);
        assert_eq!(l.point_locations, vec![v(-1.0, 0.0, 0.0), v(9.0, 9.0, 9.0)]);
        l.set_width(-5.0);
        assert_eq!(l.get_width(), 0.0);
        l.set_subdivision_levels(99);
        assert_eq!(l.get_subdivision_levels(), MAX_SEGLINE_SUBDIV_LEVELS);
    }

    /// Predicted: the LOD steps within 0 and the maximum (3); polygons 2 * (points - 1) * 2^level; the value at level 0 is
    /// AT_MIN_LOD, at the maximum the post-increment value is AT_MAX_LOD; Prepare_LOD asks for the object with levels and
    /// adds the cost without; -2 polygons without points.
    #[test]
    fn the_lod() {
        let mut l = SegmentedLine::new(0);
        assert_eq!(l.get_num_polys(), -2);
        l.set_points(&[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(2.0, 0.0, 0.0)]);
        assert_eq!(l.prepare_lod(0.5), LodRequest::AddCost(4.0));
        l.set_subdivision_levels(3);
        assert_eq!(l.prepare_lod(0.5), LodRequest::AddObject);
        assert_eq!(l.get_value(), AT_MIN_LOD);
        l.increment_lod();
        l.increment_lod();
        assert_eq!(l.get_lod_level(), 2);
        assert_eq!(l.get_num_polys(), 16);
        assert_eq!(l.get_value(), (1.0 - 0.5 / 256.0) * 0.5 / 16.0);
        assert_eq!(l.get_post_increment_value(), (1.0 - 0.5 / 1024.0) * 0.5 / 32.0);
        l.increment_lod();
        l.increment_lod();
        assert_eq!(l.get_lod_level(), 3);
        assert_eq!(l.get_post_increment_value(), AT_MAX_LOD);
        l.set_lod_level(-4);
        assert_eq!(l.get_lod_level(), 0);
        l.decrement_lod();
        assert_eq!(l.get_lod_level(), 0);
        l.set_lod_level(9);
        assert_eq!(l.get_lod_level(), 3);
        l.decrement_lod();
        assert_eq!(l.get_lod_level(), 2);
        assert_eq!(l.get_lod_count(), 3);
        l.set_subdivision_levels(1);
        l.prepare_lod(1.0);
        assert_eq!(l.get_lod_level(), 1);
    }

    /// Predicted: a laser-like line (two points, the additive preset, width 2, tiled, a texture) renders one chunk of 4
    /// vertices and 2 triangles sent to the sorting renderer, with no draw from the generator; hidden, nothing; the copy
    /// keeps the line and resets the render object; the assignment copies the hidden bits.
    #[test]
    fn render_copy_and_assign() {
        let mut l = SegmentedLine::new(0);
        l.set_texture(Some(b"Beam.tga".to_vec()));
        l.set_shader(PRESET_ADDITIVE_SHADER);
        l.set_width(2.0);
        l.set_color(&v(1.0, 0.5, 0.25));
        l.set_opacity(1.0);
        l.set_uv_offset_rate(&Vector2::new(0.0, 0.5));
        l.set_texture_mapping_mode(TILED_TEXTURE_MAP);
        l.set_texture_tile_factor(3.0);
        l.set_merge_abort_factor(1.5);
        l.set_merge_intersections(true);
        l.set_freeze_random(false);
        l.set_disable_sorting(false);
        l.set_end_caps(false);
        l.set_points(&[v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0)]);
        l.set_visible(true);
        assert_eq!(l.get_texture(), Some(&b"Beam.tga"[..]));
        assert_eq!(l.get_shader(), PRESET_ADDITIVE_SHADER);
        assert_eq!(l.get_color(), v(1.0, 0.5, 0.25));
        assert_eq!(l.get_opacity(), 1.0);
        assert_eq!(l.get_texture_mapping_mode(), TILED_TEXTURE_MAP);
        assert_eq!(l.get_texture_tile_factor(), 3.0);
        assert_eq!(l.get_merge_abort_factor(), 1.5);
        assert!(l.is_merge_intersections() && !l.is_freeze_random() && !l.is_sorting_disabled() && !l.are_end_caps_enabled());
        assert!((l.get_uv_offset_rate().y - 0.5).abs() < 1e-6);
        let mut view = Matrix3D::identity();
        view.translate(0.0, 0.0, -50.0);
        let mut g = Random3::default();
        let before = g.clone();
        let chunks = l.render(100, &view, &mut g).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].vertices.len(), 4);
        assert_eq!(chunks[0].triangles.len(), 2);
        assert!(chunks[0].sorting);
        assert_eq!(g, before);
        let c = SegmentedLine::copy(&l);
        assert_eq!(c.bits, DEFAULT_BITS);
        assert_eq!(c.point_locations, l.point_locations);
        l.set_hidden(true);
        assert!(l.is_hidden());
        assert!(l.render(200, &view, &mut g).unwrap().is_empty());
        l.set_hidden(false);
        l.set_animation_hidden(true);
        assert!(l.is_animation_hidden());
        assert!(l.render(200, &view, &mut g).unwrap().is_empty());
        l.set_force_visible(true);
        l.set_collision_type(0x10);
        let mut d = SegmentedLine::new(0);
        d.assign(&l);
        assert!(d.is_animation_hidden() && d.is_force_visible() && !d.is_hidden());
        assert_eq!(d.get_collision_type(), 0x11);
        assert_eq!(d.line_renderer, l.line_renderer);
        d.reset_line(300);
        assert_eq!(d.line_renderer.last_used_sync_time, 300);
        let mut one = SegmentedLine::new(0);
        one.add_point(&v(1.0, 2.0, 3.0));
        assert!(one.render(0, &view, &mut g).unwrap().is_empty());
        // a fourth mapping mode (only this setter can store one) is undefined upstream at the render
        one.add_point(&v(5.0, 2.0, 3.0));
        one.set_texture_mapping_mode(3);
        assert!(matches!(one.render(10, &view, &mut g), Err(crate::formats::w3d::W3dError::Undefined(crate::formats::w3d::W3dUndefined::UninitialisedRead))));
    }
}
