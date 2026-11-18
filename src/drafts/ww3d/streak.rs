//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/streak.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/streak.h
//!
//! `StreakLineClass` is `SegmentedLineClass` with per-point colours/widths. Its `LineRenderer` is the port's
//! `SegLineRenderer` (reused, `crate::formats::w3d::seglinerenderer`) and its `StreakRenderer` is the draft
//! [`StreakRendererClass`]. The `RenderObjClass` base (hidden flags, cached bounding volumes, container, static
//! sort lists) and `PredictiveLODOptimizerClass` are not in the verified port: the LOD hooks that talk to the
//! optimiser are comments, and the ones that only touch the renderer's subdivision level are translated.
//! `Cast_Ray` needs `LineSegClass::Find_Intersection` (WWMath), taken as a closure.

use super::streak_render::StreakRendererClass;
use crate::formats::w3d::seglinerenderer::{SegLineChunk, SegLineRenderer, MAX_SEGLINE_SUBDIV_LEVELS};
use crate::formats::w3d::segline::AABox;
use crate::formats::w3d::shader::{ShaderClass, SORT_LEVEL_NONE};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::random::Random3;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;
use crate::ww::wwmath;

/// `AT_MIN_LOD` / `AT_MAX_LOD` (predlod.h): -FLT_MAX and +FLT_MAX style sentinels of the LOD optimiser.
pub const AT_MIN_LOD: f32 = -f32::MAX;
pub const AT_MAX_LOD: f32 = f32::MAX;

/// `SURFACE_TYPE_DEFAULT` (w3d_file.h).
pub const SURFACE_TYPE_DEFAULT: u32 = 13;

/// `class StreakLineClass` (streak.h).
#[derive(Clone, Debug)]
pub struct StreakLineClass {
    max_subdivision_levels: u32,
    normalized_screen_area: f32,
    point_locations: Vec<Vector3>,
    point_colors: Vec<Vector4>,
    point_widths: Vec<f32>,
    pub line_renderer: SegLineRenderer,
    pub streak_renderer: StreakRendererClass,
    /// `unsigned int *Personalities` (borrowed from the caller upstream; owned copy here).
    personalities: Vec<u32>,
    /// `RenderObjClass::Transform`.
    pub transform: Matrix3D,
    /// `RenderObjClass::CollisionType` bits (`Get_Collision_Type`).
    pub collision_type: u32,
}

impl StreakLineClass {
    /// `StreakLineClass::StreakLineClass()` (streak.cpp:57). `sync_time` is `WW3D::Get_Sync_Time()` for the line
    /// renderer's constructor.
    pub fn new(sync_time: u32) -> Self {
        StreakLineClass {
            max_subdivision_levels: 0,
            normalized_screen_area: 0.0,
            point_locations: Vec::new(),
            point_colors: Vec::new(),
            point_widths: Vec::new(),
            line_renderer: SegLineRenderer::new(sync_time),
            streak_renderer: StreakRendererClass::new(),
            personalities: Vec::new(),
            transform: Matrix3D::from_translation(&Vector3::new(0.0, 0.0, 0.0)),
            collision_type: 0,
        }
    }

    // StreakLineClass(const &) and operator= copy every member: `Clone`.

    /// `StreakLineClass::Reset_Line` (streak.cpp:95): `LineRenderer.Reset_Line()` takes the sync time.
    pub fn reset_line(&mut self, sync_time: u32) {
        self.line_renderer.reset_line(sync_time);
        self.streak_renderer.reset_line();
    }

    /// `StreakLineClass::Set_Locs` (streak.cpp:107): at least two points; otherwise `WWASSERT(0)` and return.
    pub fn set_locs(&mut self, locs: &[Vector3]) {
        if locs.len() < 2 {
            debug_assert!(false);
            return;
        }
        self.point_locations.clear();
        self.point_locations.extend_from_slice(locs);
        // Invalidate_Cached_Bounding_Volumes: render-object base
    }

    /// `StreakLineClass::Set_Widths` (streak.cpp:131).
    pub fn set_widths(&mut self, widths: &[f32]) {
        if widths.len() < 2 {
            debug_assert!(false);
            return;
        }
        self.point_widths.clear();
        self.point_widths.extend_from_slice(widths);
    }

    /// `StreakLineClass::Set_Colors` (streak.cpp:152).
    pub fn set_colors(&mut self, colors: &[Vector4]) {
        if colors.len() < 2 {
            debug_assert!(false);
            return;
        }
        self.point_colors.clear();
        self.point_colors.extend_from_slice(colors);
    }

    /// `StreakLineClass::Set_LocsWidthsColors` (streak.cpp:173).
    pub fn set_locs_widths_colors(
        &mut self,
        locs: &[Vector3],
        widths: Option<&[f32]>,
        colors: Option<&[Vector4]>,
        personalities: Option<&[u32]>,
    ) {
        self.personalities = personalities.map(|p| p.to_vec()).unwrap_or_default();
        self.set_locs(locs);
        if let Some(w) = widths {
            self.set_widths(w);
            debug_assert!(self.point_locations.len() == self.point_widths.len());
        }
        if let Some(c) = colors {
            self.set_colors(c);
            debug_assert!(self.point_locations.len() == self.point_colors.len());
        }
    }

    /// `StreakLineClass::Get_Num_Points` (streak.cpp:216).
    pub fn get_num_points(&self) -> i32 {
        self.point_locations.len() as i32
    }

    /// `StreakLineClass::Set_Point_Location`: out of range does nothing.
    pub fn set_point_location(&mut self, point_idx: u32, location: &Vector3) {
        if (point_idx as usize) < self.point_locations.len() {
            self.point_locations[point_idx as usize] = *location;
        }
    }

    /// `StreakLineClass::Get_Point_Location`: out of range gives (0, 0, 0).
    pub fn get_point_location(&self, point_idx: u32) -> Vector3 {
        if (point_idx as usize) < self.point_locations.len() {
            self.point_locations[point_idx as usize]
        } else {
            Vector3::new(0.0, 0.0, 0.0)
        }
    }

    /// `StreakLineClass::Add_Point`.
    pub fn add_point(&mut self, location: &Vector3) {
        self.point_locations.push(*location);
    }

    /// `StreakLineClass::Delete_Point`.
    pub fn delete_point(&mut self, point_idx: u32) {
        if (point_idx as usize) < self.point_locations.len() {
            self.point_locations.remove(point_idx as usize);
        }
    }

    pub fn get_texture(&self) -> Option<&[u8]> {
        self.line_renderer.texture.as_deref()
    }
    pub fn get_shader(&self) -> ShaderClass {
        self.line_renderer.shader
    }
    pub fn get_color(&self) -> Vector3 {
        self.line_renderer.color
    }
    pub fn get_opacity(&self) -> f32 {
        self.line_renderer.opacity
    }
    pub fn get_noise_amplitude(&self) -> f32 {
        self.line_renderer.noise_amplitude
    }
    pub fn get_merge_abort_factor(&self) -> f32 {
        self.line_renderer.merge_abort_factor
    }
    pub fn get_subdivision_levels(&self) -> u32 {
        self.max_subdivision_levels
    }
    pub fn get_texture_mapping_mode(&self) -> u32 {
        self.line_renderer.get_texture_mapping_mode()
    }
    pub fn get_texture_tile_factor(&self) -> f32 {
        self.line_renderer.texture_tile_factor
    }
    pub fn get_uv_offset_rate(&self) -> Vector2 {
        self.line_renderer.get_uv_offset_rate()
    }
    pub fn is_merge_intersections(&self) -> bool {
        self.line_renderer.is_merge_intersections()
    }
    pub fn is_freeze_random(&self) -> bool {
        self.line_renderer.is_freeze_random()
    }
    pub fn is_sorting_disabled(&self) -> bool {
        self.line_renderer.is_sorting_disabled()
    }
    pub fn are_end_caps_enabled(&self) -> bool {
        self.line_renderer.are_end_caps_enabled()
    }

    /// `StreakLineClass::Set_Texture`: both renderers.
    pub fn set_texture(&mut self, texture: Option<Vec<u8>>) {
        self.line_renderer.texture = texture.clone();
        self.streak_renderer.set_texture(texture);
    }

    /// `StreakLineClass::Set_Shader`: both renderers.
    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.line_renderer.shader = shader;
        self.streak_renderer.set_shader(shader);
    }

    /// `StreakLineClass::Get_Width`.
    pub fn get_width(&self) -> f32 {
        self.line_renderer.width
    }

    /// `StreakLineClass::Set_Width`: clamped at 0 ("Widths need to be clamped because they are not automatically
    /// clamped later").
    pub fn set_width(&mut self, width: f32) {
        let w = wwmath::max(width, 0.0f32);
        self.line_renderer.width = w;
        self.streak_renderer.set_width(w);
    }

    pub fn set_color(&mut self, color: &Vector3) {
        self.line_renderer.color = *color;
        self.streak_renderer.set_color(color);
    }

    pub fn set_opacity(&mut self, opacity: f32) {
        self.line_renderer.opacity = opacity;
        self.streak_renderer.set_opacity(opacity);
    }

    /// `StreakLineClass::Set_Noise_Amplitude`: only the line renderer, `Fabs` of the value.
    pub fn set_noise_amplitude(&mut self, amplitude: f32) {
        self.line_renderer.noise_amplitude = wwmath::fabs(amplitude);
    }

    /// `StreakLineClass::Set_Merge_Abort_Factor`: only the line renderer.
    pub fn set_merge_abort_factor(&mut self, factor: f32) {
        self.line_renderer.merge_abort_factor = factor;
    }

    /// `StreakLineClass::Set_Subdivision_Levels`: `MIN(levels, MAX_SEGLINE_SUBDIV_LEVELS)`.
    pub fn set_subdivision_levels(&mut self, levels: u32) {
        self.max_subdivision_levels = levels.min(MAX_SEGLINE_SUBDIV_LEVELS);
    }

    pub fn set_texture_mapping_mode(&mut self, mode: u32) {
        self.line_renderer.set_texture_mapping_mode(mode);
    }
    pub fn set_texture_tile_factor(&mut self, factor: f32) {
        self.line_renderer.set_texture_tile_factor(factor);
    }
    pub fn set_uv_offset_rate(&mut self, rate: Vector2) {
        self.line_renderer.set_uv_offset_rate(rate);
    }
    pub fn set_merge_intersections(&mut self, onoff: bool) {
        self.line_renderer.set_merge_intersections(onoff);
    }
    pub fn set_freeze_random(&mut self, onoff: bool) {
        self.line_renderer.set_freeze_random(onoff);
    }
    pub fn set_disable_sorting(&mut self, onoff: bool) {
        self.line_renderer.set_disable_sorting(onoff);
    }
    pub fn set_end_caps(&mut self, onoff: bool) {
        self.line_renderer.set_end_caps(onoff);
    }

    /// `StreakLineClass::Get_Num_Polys` (streak.cpp:385).
    pub fn get_num_polys(&self) -> i32 {
        let subdivision_factor = 1i32 << self.line_renderer.subdivision_level;
        2 * (self.point_locations.len() as i32 - 1) * subdivision_factor
    }

    /// `StreakLineClass::Render` (streak.cpp:395), the data half. The hidden test and the static-sort-list
    /// redirect (taken when `sorting_enabled` is off and the shader guesses a sort level, with static sort lists on)
    /// belong to the render-object base / `WW3D`; `static_sort_list_redirect` tells this function it applies.
    /// Renders the plain segmented line when colours or widths are absent, else the streak.
    pub fn render(
        &mut self,
        static_sort_list_redirect: bool,
        sync_time: u32,
        view: &Matrix3D,
        v3: &mut Random3,
    ) -> Vec<SegLineChunk> {
        if static_sort_list_redirect {
            return Vec::new(); // WW3D::Add_To_Static_Sort_List(this, sort_level)
        }
        if self.point_colors.is_empty() || self.point_widths.is_empty() {
            self.render_seg_line(sync_time, view, v3)
        } else {
            self.render_streak_line(view, v3)
        }
    }

    /// `StreakLineClass::Get_Obj_Space_Bounding_Box` (streak.cpp:429).
    pub fn get_obj_space_bounding_box(&self) -> AABox {
        let num_points = self.point_locations.len();
        if num_points >= 2 {
            let mut max_coords = self.point_locations[0];
            let mut min_coords = self.point_locations[0];
            for i in 1..num_points {
                max_coords.update_max(&self.point_locations[i]);
                min_coords.update_min(&self.point_locations[i]);
            }
            // Enlarge bounding box by half the width
            let mut enlarge_factor = self.line_renderer.width * 0.5f32;
            let mut enlarge_offset = Vector3::new(enlarge_factor, enlarge_factor, enlarge_factor);
            max_coords += enlarge_offset;
            min_coords -= enlarge_offset;

            if self.max_subdivision_levels > 0 {
                // Second bounding box from the midpoints, with the noise amplitude factored in
                let mut midpoint = (self.point_locations[0] + self.point_locations[1]) * 0.5f32;
                let mut max_coords2 = midpoint;
                let mut min_coords2 = midpoint;
                for i in 1..(num_points - 1) {
                    midpoint = (self.point_locations[i] + self.point_locations[i + 1]) * 0.5f32;
                    max_coords2.update_max(&midpoint);
                    min_coords2.update_min(&midpoint);
                }
                enlarge_factor += 2.0f32 * self.line_renderer.noise_amplitude;
                enlarge_offset = Vector3::new(enlarge_factor, enlarge_factor, enlarge_factor);
                max_coords2 += enlarge_offset;
                min_coords2 -= enlarge_offset;
                max_coords.update_max(&max_coords2);
                min_coords.update_min(&min_coords2);
            }
            AABox::init_min_max(&min_coords, &max_coords)
        } else {
            // Invalid line - return something: box.Init(Vector3(0,0,0), Vector3(1,1,1)) (centre, extent)
            AABox { center: Vector3::new(0.0, 0.0, 0.0), extent: Vector3::new(1.0, 1.0, 1.0) }
        }
    }

    /// `StreakLineClass::Get_Obj_Space_Bounding_Sphere` (streak.cpp:419): (center, radius) from the box.
    pub fn get_obj_space_bounding_sphere(&self) -> (Vector3, f32) {
        let b = self.get_obj_space_bounding_box();
        (b.center, b.extent.length())
    }

    /// `StreakLineClass::Prepare_LOD` (streak.cpp:489), the renderer half: clamps the current subdivision level to
    /// the maximum. `NormalizedScreenArea = Get_Screen_Size(camera)` is given as `screen_size`. The optimiser
    /// registration (`PredictiveLODOptimizerClass::Add_Object` / `Add_Cost`) is the owner's: returns whether the
    /// line should be added to the optimiser (`MaxSubdivisionLevels > 0`), else its cost to add.
    pub fn prepare_lod(&mut self, screen_size: f32) -> bool {
        self.normalized_screen_area = screen_size;
        let mut lvl = self.line_renderer.subdivision_level;
        lvl = lvl.min(self.max_subdivision_levels);
        self.line_renderer.subdivision_level = lvl;
        self.max_subdivision_levels > 0
    }

    /// `StreakLineClass::Increment_LOD`.
    pub fn increment_lod(&mut self) {
        let mut lvl = self.line_renderer.subdivision_level;
        lvl = (lvl + 1).min(self.max_subdivision_levels);
        self.line_renderer.subdivision_level = lvl;
    }

    /// `StreakLineClass::Decrement_LOD`.
    pub fn decrement_lod(&mut self) {
        let lvl = self.line_renderer.subdivision_level;
        if lvl == 0 {
            return;
        }
        self.line_renderer.subdivision_level = lvl - 1;
    }

    /// `StreakLineClass::Get_Cost`.
    pub fn get_cost(&self) -> f32 {
        self.get_num_polys() as f32
    }

    /// `StreakLineClass::Get_Value`.
    pub fn get_value(&self) -> f32 {
        if self.line_renderer.subdivision_level == 0 {
            AT_MIN_LOD
        } else {
            let polycount = self.get_num_polys() as f32;
            let benefit_factor = 1.0f32 - (0.5f32 / (polycount * polycount));
            (benefit_factor * self.normalized_screen_area) / self.get_cost()
        }
    }

    /// `StreakLineClass::Get_Post_Increment_Value`.
    pub fn get_post_increment_value(&self) -> f32 {
        if self.line_renderer.subdivision_level == self.max_subdivision_levels {
            AT_MAX_LOD
        } else {
            // Assumption: each subdivision level doubles polycount
            let polycount = 2.0f32 * self.get_num_polys() as f32;
            let benefit_factor = 1.0f32 - (0.5f32 / (polycount * polycount));
            // Assumption: Cost() == polycount
            (benefit_factor * self.normalized_screen_area) / polycount
        }
    }

    /// `StreakLineClass::Set_LOD_Level`.
    pub fn set_lod_level(&mut self, lod: i32) {
        let mut lod = lod.max(0);
        lod = lod.min(self.max_subdivision_levels as i32);
        self.line_renderer.subdivision_level = lod as u32;
    }

    pub fn get_lod_level(&self) -> i32 {
        self.line_renderer.subdivision_level as i32
    }

    pub fn get_lod_count(&self) -> i32 {
        self.max_subdivision_levels as i32
    }

    /// `StreakLineClass::Render_Seg_Line` (streak.cpp:610): `LineRenderer.Render` over the locations (no per-point
    /// colours).
    fn render_seg_line(&mut self, sync_time: u32, view: &Matrix3D, v3: &mut Random3) -> Vec<SegLineChunk> {
        // Line must have at least two points to be valid
        if self.point_locations.len() < 2 {
            return Vec::new();
        }
        // The sphere is only handed to the sorting renderer upstream.
        self.line_renderer
            .render(sync_time, view, &self.transform, &self.point_locations, None, v3)
            .unwrap_or_default()
    }

    /// `StreakLineClass::Render_Streak_Line` (streak.cpp:630).
    fn render_streak_line(&mut self, view: &Matrix3D, v3: &mut Random3) -> Vec<SegLineChunk> {
        debug_assert!(self.point_locations.len() == self.point_colors.len());
        debug_assert!(self.point_locations.len() == self.point_widths.len());
        if self.point_locations.len() < 2 {
            return Vec::new();
        }
        if self.point_colors.len() < 2 {
            return Vec::new();
        }
        if self.point_widths.len() < 2 {
            return Vec::new();
        }
        if self.point_locations.len() != self.point_colors.len() {
            return Vec::new();
        }
        if self.point_locations.len() != self.point_widths.len() {
            return Vec::new();
        }
        self.streak_renderer.render_streak(
            view,
            &self.transform,
            &self.point_locations,
            &self.point_colors,
            &self.point_widths,
            &self.personalities,
            v3,
        )
    }

    /// `StreakLineClass::Cast_Ray` (streak.cpp:670). `find_intersection(start, end)` is
    /// `raytest.Ray.Find_Intersection(LineSegClass(start,end), &p0, &fraction, &p1, NULL)`: `Some((p0, fraction, p1))`
    /// on a hit. `result_fraction` is `raytest.Result->Fraction` and is updated, with the surface type, on a hit;
    /// the caller sets `CollidedRenderObj`. The collision-type test is `(Get_Collision_Type() & raytest.CollisionType)`.
    pub fn cast_ray<F: Fn(&Vector3, &Vector3) -> Option<(Vector3, f32, Vector3)>>(
        &self,
        ray_collision_type: u32,
        result_fraction: &mut f32,
        result_surface_type: &mut u32,
        find_intersection: F,
    ) -> bool {
        if (self.collision_type & ray_collision_type) == 0 {
            return false;
        }
        let mut retval = false;
        let mut fraction = 1.0f32;
        for index in 1..self.point_locations.len() {
            // Transform.mulVector3Array(&PointLocations[index-1], curr, 2)
            let curr0 = self.transform.mul_vector3(&self.point_locations[index - 1]);
            let curr1 = self.transform.mul_vector3(&self.point_locations[index]);
            if let Some((p0, f, p1)) = find_intersection(&curr0, &curr1) {
                fraction = f;
                // Determine if the ray was close enough to this line to be considered intersecting
                let dist = (p0 - p1).length();
                if dist <= self.line_renderer.width && fraction >= 0.0 && fraction < *result_fraction {
                    retval = true;
                    break;
                }
            }
        }
        if retval {
            *result_fraction = fraction;
            *result_surface_type = SURFACE_TYPE_DEFAULT;
        }
        retval
    }

    /// `StreakLineClass::Class_ID` / sort level: the streak has none of its own (`SORT_LEVEL_NONE`).
    pub fn sort_level(&self) -> i32 {
        SORT_LEVEL_NONE
    }
}
