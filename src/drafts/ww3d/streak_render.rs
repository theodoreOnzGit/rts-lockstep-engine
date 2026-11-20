//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/streakRender.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/streakRender.h
//!
//! `StreakRendererClass` is a Zero Hour copy of `SegLineRendererClass::Render` with per-point colours and widths
//! (`RenderStreak`), no UV animation and no texture-mapping modes (`u_values` is {0, 1} and every base V is 0).
//! Reused from the port: `SegLineChunk` / `SegLineVertex` (`crate::formats::w3d::seglinerenderer`) as the output
//! record, `Random3` for the noise. The DX8 buffer fill is device-only: `render_streak` returns the chunks.
//!
//! Upstream quirks kept, flagged for the differential pass:
//! - `radius = widths[segmentIndex]` (not half the width) per segment, and the merge-abort test afterwards uses the
//!   LAST value of `radius` ("TODO: does not work correctly when subdivisions are not 0" in upstream);
//! - the intersections' `TexV` for interior points is `personalities[intersectionIndex] & 1` ("LORENZEN"), indexed
//!   by the subdivided intersection index;
//! - in merging, `next_factor = oo_new_count * curr_int->PointCount` (the CURRENT intersection's count, not the
//!   next's);
//! - in merging's parallel branch with `curr_int->Parallel`, `new_direction` is read before it is written
//!   (uninitialised in C++); here it starts at zero;
//! - the vertex colour is `colors[MIN(i/2, point_cnt)]`, which can index one past the end of `colors`; clamped here;
//! - `Render` (the plain-line entry) is an empty `return;` upstream.

use super::dynamesh::convert_color_clamp;
use crate::logic::modules::plant;
use crate::formats::w3d::seglinerenderer::{SegLineChunk, SegLineVertex, MAX_SEGLINE_SUBDIV_LEVELS, SEGLINE_CHUNK_SIZE};
use crate::formats::w3d::shader::{ShaderClass, DSTBLEND_ZERO, GRADIENT_MODULATE, PRESET_ADDITIVE_SPRITE_SHADER};
use crate::formats::w3d::mesh::shader_bits::{MASK_PRIGRADIENT, MASK_TEXTURING, SHIFT_PRIGRADIENT, SHIFT_TEXTURING};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::random::Random3;
use crate::ww::vector3::{equal_within_epsilon, Vector3};
use crate::ww::vector4::Vector4;
use crate::ww::wwmath;

/// `MAX_STREAK_SUBDIV_LEVELS` (streakRender.h): same as the segmented line's.
pub const MAX_STREAK_SUBDIV_LEVELS: u32 = MAX_SEGLINE_SUBDIV_LEVELS;
/// `STREAK_CHUNK_SIZE`.
pub const STREAK_CHUNK_SIZE: u32 = SEGLINE_CHUNK_SIZE;

const MERGE_INTERSECTIONS: u32 = 0x0000_0001;
const FREEZE_RANDOM: u32 = 0x0000_0002;
const DISABLE_SORTING: u32 = 0x0000_0004;
const END_CAPS: u32 = 0x0000_0008;
const TEXTURE_MAP_MODE_OFFSET: u32 = 24;
const TEXTURE_MAP_MODE_MASK: u32 = 0xFF00_0000;
const DEFAULT_BITS: u32 = MERGE_INTERSECTIONS;
const OO_INT_MAX: f32 = 1.0 / 2_147_483_648.0;
const TOP_EDGE: usize = 0;
const BOTTOM_EDGE: usize = 1;

#[derive(Clone, Copy, Debug, Default)]
struct LineSegment {
    start_plane: Vector3,
    edge_plane: [Vector3; 2],
}

#[derive(Clone, Copy, Debug, Default)]
struct LineSegmentIntersection {
    point_count: u32,
    next_segment_id: u32,
    direction: Vector3,
    point: Vector3,
    tex_v: f32,
    fold: bool,
    parallel: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct StreakSubdivision {
    start_pos: Vector3,
    end_pos: Vector3,
    start_tex_v: f32,
    end_tex_v: f32,
    rand: f32,
    level: u32,
}

/// `class StreakRendererClass` (streakRender.h).
#[derive(Clone, Debug, PartialEq)]
pub struct StreakRendererClass {
    /// Texture name (`None` = NULL); a handle in the finished port.
    pub texture: Option<Vec<u8>>,
    pub shader: ShaderClass,
    pub width: f32,
    pub color: Vector3,
    pub opacity: f32,
    pub subdivision_level: u32,
    pub noise_amplitude: f32,
    pub merge_abort_factor: f32,
    pub bits: u32,
}

impl StreakRendererClass {
    /// `StreakRendererClass::StreakRendererClass()` (streakRender.cpp:60).
    pub fn new() -> Self {
        StreakRendererClass {
            texture: None,
            shader: PRESET_ADDITIVE_SPRITE_SHADER,
            width: 0.0,
            color: Vector3::new(1.0, 1.0, 1.0),
            opacity: 1.0,
            subdivision_level: 0,
            noise_amplitude: 0.0,
            merge_abort_factor: 1.5,
            bits: DEFAULT_BITS,
        }
    }

    // StreakRendererClass(const &) / operator= copy every field except the vertex buffer: `Clone`.

    fn set_bit(&mut self, bit: u32, onoff: bool) {
        if onoff {
            self.bits |= bit;
        } else {
            self.bits &= !bit;
        }
    }

    pub fn set_merge_intersections(&mut self, onoff: bool) {
        self.set_bit(MERGE_INTERSECTIONS, onoff);
    }
    pub fn set_freeze_random(&mut self, onoff: bool) {
        self.set_bit(FREEZE_RANDOM, onoff);
    }
    pub fn set_disable_sorting(&mut self, onoff: bool) {
        self.set_bit(DISABLE_SORTING, onoff);
    }
    pub fn set_end_caps(&mut self, onoff: bool) {
        self.set_bit(END_CAPS, onoff);
    }
    pub fn is_merge_intersections(&self) -> bool {
        self.bits & MERGE_INTERSECTIONS != 0
    }
    pub fn is_freeze_random(&self) -> bool {
        self.bits & FREEZE_RANDOM != 0
    }
    pub fn is_sorting_disabled(&self) -> bool {
        self.bits & DISABLE_SORTING != 0
    }
    pub fn are_end_caps_enabled(&self) -> bool {
        self.bits & END_CAPS != 0
    }
    pub fn get_texture_mapping_mode(&self) -> u32 {
        (self.bits & TEXTURE_MAP_MODE_MASK) >> TEXTURE_MAP_MODE_OFFSET
    }
    pub fn set_texture_mapping_mode(&mut self, mode: u32) {
        self.bits = (self.bits & !TEXTURE_MAP_MODE_MASK) | (mode << TEXTURE_MAP_MODE_OFFSET);
    }

    pub fn set_texture(&mut self, texture: Option<Vec<u8>>) {
        self.texture = texture;
    }
    pub fn get_texture(&self) -> Option<&[u8]> {
        self.texture.as_deref()
    }
    pub fn get_shader(&self) -> ShaderClass {
        self.shader
    }
    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.shader = shader;
    }
    pub fn get_width(&self) -> f32 {
        self.width
    }
    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }
    pub fn get_color(&self) -> Vector3 {
        self.color
    }
    pub fn set_color(&mut self, color: &Vector3) {
        self.color = *color;
    }
    pub fn get_opacity(&self) -> f32 {
        self.opacity
    }
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }
    pub fn get_noise_amplitude(&self) -> f32 {
        self.noise_amplitude
    }
    pub fn set_noise_amplitude(&mut self, amplitude: f32) {
        self.noise_amplitude = amplitude;
    }
    pub fn get_merge_abort_factor(&self) -> f32 {
        self.merge_abort_factor
    }
    pub fn set_merge_abort_factor(&mut self, factor: f32) {
        self.merge_abort_factor = factor;
    }
    pub fn get_current_subdivision_level(&self) -> u32 {
        self.subdivision_level
    }
    pub fn set_current_subdivision_level(&mut self, lvl: u32) {
        self.subdivision_level = lvl;
    }

    /// `StreakRendererClass::Reset_Line` (streakRender.h): the streak renderer keeps no UV animation state.
    pub fn reset_line(&mut self) {}

    /// `StreakRendererClass::Init(props)` (streakRender.cpp:108), from the `W3D_ELINE_*` flags.
    pub fn init(&mut self, flags: u32, subdivision_level: u32, noise_amplitude: f32, merge_abort_factor: f32) {
        use crate::formats::w3d::seglinerenderer::*;
        self.set_merge_intersections(flags & W3D_ELINE_MERGE_INTERSECTIONS != 0);
        self.set_freeze_random(flags & W3D_ELINE_FREEZE_RANDOM != 0);
        self.set_disable_sorting(flags & W3D_ELINE_DISABLE_SORTING != 0);
        self.set_end_caps(flags & W3D_ELINE_END_CAPS != 0);
        let texture_mode = (flags & W3D_ELINE_TEXTURE_MAP_MODE_MASK) >> W3D_ELINE_TEXTURE_MAP_MODE_OFFSET;
        match texture_mode {
            W3D_ELINE_UNIFORM_WIDTH_TEXTURE_MAP => self.set_texture_mapping_mode(UNIFORM_WIDTH_TEXTURE_MAP),
            W3D_ELINE_UNIFORM_LENGTH_TEXTURE_MAP => self.set_texture_mapping_mode(UNIFORM_LENGTH_TEXTURE_MAP),
            W3D_ELINE_TILED_TEXTURE_MAP => self.set_texture_mapping_mode(TILED_TEXTURE_MAP),
            _ => {}
        }
        self.set_current_subdivision_level(subdivision_level);
        self.set_noise_amplitude(noise_amplitude);
        self.set_merge_abort_factor(merge_abort_factor);
    }

    /// `StreakRendererClass::Render` (streakRender.cpp:136): an empty function upstream (`return;`).
    pub fn render(&self) {}

    /// `StreakRendererClass::subdivision_util` (streakRender.cpp:148). Returns the subdivided points and their V
    /// coordinates.
    fn subdivision_util(&self, xformed_pts: &[Vector3], base_tex_v: &[f32], v3: &mut Random3) -> (Vec<Vector3>, Vec<f32>) {
        let point_cnt = xformed_pts.len();
        let freeze_random = self.is_freeze_random();
        let mut randomize = Random3::default();
        let mut out_pts = Vec::new();
        let mut out_v = Vec::new();
        let mut stack = [StreakSubdivision::default(); 2 * MAX_STREAK_SUBDIV_LEVELS as usize];

        for pidx in 0..point_cnt - 1 {
            let mut tos: i32 = 0;
            stack[0].start_pos = xformed_pts[pidx];
            stack[0].end_pos = xformed_pts[pidx + 1];
            stack[0].start_tex_v = base_tex_v[pidx];
            stack[0].end_tex_v = base_tex_v[pidx + 1];
            stack[0].rand = self.noise_amplitude;
            stack[0].level = 0;
            while tos >= 0 {
                let t = tos as usize;
                if stack[t].level == self.subdivision_level {
                    out_pts.push(stack[t].start_pos);
                    out_v.push(stack[t].start_tex_v);
                    tos -= 1;
                } else {
                    let randvec = if freeze_random {
                        let x = randomize.next() as f32 * OO_INT_MAX;
                        let y = randomize.next() as f32 * OO_INT_MAX;
                        let z = randomize.next() as f32 * OO_INT_MAX;
                        Vector3::new(x, y, z)
                    } else {
                        // Vector3SolidBoxRandomizer(Vector3(1,1,1)).Get_Vector
                        let x = v3.next() as f32 * OO_INT_MAX * 1.0f32;
                        let y = v3.next() as f32 * OO_INT_MAX * 1.0f32;
                        let z = v3.next() as f32 * OO_INT_MAX * 1.0f32;
                        Vector3::new(x, y, z)
                    };
                    let s = stack[t];
                    let mut child = s;
                    child.start_pos = s.start_pos;
                    child.end_pos = (s.start_pos + s.end_pos) * 0.5f32 + randvec * s.rand;
                    child.start_tex_v = s.start_tex_v;
                    child.end_tex_v = (s.start_tex_v + s.end_tex_v) * 0.5f32;
                    child.rand = s.rand * 0.5f32;
                    child.level = s.level + 1;
                    stack[t + 1] = child;
                    stack[t].start_pos = child.end_pos;
                    stack[t].start_tex_v = child.end_tex_v;
                    stack[t].rand = child.rand;
                    stack[t].level = child.level;
                    tos += 1;
                }
            }
        }
        out_pts.push(xformed_pts[point_cnt - 1]);
        out_v.push(base_tex_v[point_cnt - 1]);
        (out_pts, out_v)
    }

    /// `StreakRendererClass::RenderStreak` (streakRender.cpp:240). `view` is the view matrix's top three rows
    /// (`view2`, from `DX8Wrapper::Get_Transform(D3DTS_VIEW)`); the world/view reset to identity and the restore are
    /// device-only. `colors` and `widths` have one entry per input point; `personalities` one per subdivided
    /// intersection. Returns one [`SegLineChunk`] per chunk (the reuse of the port's record).
    pub fn render_streak(
        &self,
        view: &Matrix3D,
        transform: &Matrix3D,
        points: &[Vector3],
        colors: &[Vector4],
        widths: &[f32],
        personalities: &[u32],
        v3: &mut Random3,
    ) -> Vec<SegLineChunk> {
        let num_points = points.len() as u32;
        let mut chunks = Vec::new();
        if num_points < 2 {
            return chunks;
        }
        // upstream: DX8Wrapper::Get_Transform/Set_Transform(D3DTS_WORLD/VIEW, identity) — device-only, not translated

        let parallel_factor = if plant(19146) { 0.99f32 } else { 0.9f32 };
        let mut chunk_size = (STREAK_CHUNK_SIZE >> self.subdivision_level) + if plant(19147) { 0 } else { 1 };
        if chunk_size > num_points {
            chunk_size = num_points;
        }

        let mut modelview = Matrix3D::default();
        modelview.mul(view, transform);

        let mut chunk_index = 0u32;
        while chunk_index < num_points - 1 {
            let mut point_cnt = (num_points - chunk_index).min(chunk_size);

            // Transform points in chunk from objectspace to eyespace
            let xformed_pts: Vec<Vector3> =
                points[chunk_index as usize..(chunk_index + point_cnt) as usize].iter().map(|p| modelview.mul_vector3(p)).collect();

            // Prepare v parameter per point: all 0; u = {0, 1}
            let base_tex_v = vec![0.0f32; point_cnt as usize];
            let u_values = [0.0f32, 1.0f32];

            let (mut pts, tex_v) = self.subdivision_util(&xformed_pts, &base_tex_v, v3);
            point_cnt = pts.len() as u32;
            let pc = point_cnt as usize;

            // Calculate line segment edge planes
            let mut switch_edges = false;
            let mut segment = vec![LineSegment::default(); pc + 2];
            let mut intersection = vec![[LineSegmentIntersection::default(); 2]; pc + 2];
            let mut radius = self.width * 0.5f32;

            for sidx in 1..pc {
                // upstream: radius = widths[segmentIndex] (TODO: wrong when subdivision != 0)
                radius = widths[if plant(19141) { sidx - 1 } else { sidx }];
                let curr_point = pts[sidx - 1];
                if equal_within_epsilon(&curr_point, &pts[sidx], 0.0001f32) {
                    if plant(19142) { pts[sidx].y += 0.001f32 } else { pts[sidx].x += 0.001f32 };
                }
                let next_point = pts[sidx];

                let mut segdir = next_point - curr_point;
                segdir.normalize();
                segment[sidx].start_plane = segdir;

                let nearest = curr_point + segdir * -Vector3::dot_product(&segdir, &curr_point);
                let mut offset = Vector3::cross_product(&segdir, &nearest);
                offset.normalize();
                let top = curr_point + offset * radius;
                let bottom = curr_point + offset * (if plant(19143) { radius } else { -radius });

                let mut top_normal = Vector3::cross_product(&top, &segdir);
                top_normal.normalize();
                segment[sidx].edge_plane[TOP_EDGE] = top_normal;
                let mut bottom_normal = Vector3::cross_product(&segdir, &bottom);
                bottom_normal.normalize();
                segment[sidx].edge_plane[BOTTOM_EDGE] = bottom_normal;

                if sidx > 1 {
                    let mut prev_plane = Vector3::cross_product(&pts[sidx - 2], &curr_point);
                    prev_plane.normalize();
                    let mut curr_plane = Vector3::cross_product(&curr_point, &next_point);
                    curr_plane.normalize();
                    if if plant(19144) { Vector3::dot_product(&prev_plane, &curr_plane) > 0.0f32 } else { Vector3::dot_product(&prev_plane, &curr_plane) < 0.0f32 } {
                        switch_edges = !switch_edges;
                        intersection[sidx][TOP_EDGE].fold = true;
                        intersection[sidx][BOTTOM_EDGE].fold = true;
                    } else {
                        intersection[sidx][TOP_EDGE].fold = false;
                        intersection[sidx][BOTTOM_EDGE].fold = false;
                    }
                }
                if switch_edges {
                    segment[sidx].edge_plane[TOP_EDGE] = -bottom_normal;
                    segment[sidx].edge_plane[BOTTOM_EDGE] = -top_normal;
                }
            }

            // Calculate segment edge intersections
            let numsegs = pc - 1;
            let mut num_intersections = [pc, pc];

            for e in 0..2 {
                intersection[0][e].point_count = 0;
                intersection[0][e].next_segment_id = 0;
                intersection[0][e].direction = Vector3::new(1.0, 0.0, 0.0);
                intersection[0][e].point = Vector3::new(0.0, 0.0, 0.0);
                intersection[0][e].tex_v = 0.0;
                intersection[0][e].fold = true;
                intersection[0][e].parallel = false;
                intersection[1][e].point_count = 1;
                intersection[1][e].next_segment_id = 1;
                intersection[1][e].point = pts[0];
                intersection[1][e].tex_v = tex_v[0];
                intersection[1][e].fold = true;
                intersection[1][e].parallel = false;
            }

            let first_point = pts[0];
            let first_plane = segment[1].edge_plane;
            let mut top = first_point - first_plane[TOP_EDGE] * Vector3::dot_product(&first_plane[TOP_EDGE], &first_point);
            top.normalize();
            intersection[1][TOP_EDGE].direction = top;
            let mut bottom = first_point - first_plane[BOTTOM_EDGE] * Vector3::dot_product(&first_plane[BOTTOM_EDGE], &first_point);
            bottom.normalize();
            intersection[1][BOTTOM_EDGE].direction = bottom;

            let mut segdir = pts[1] - pts[0];
            segdir.normalize();
            let mut start_pl = Vector3::cross_product(&top, &bottom);
            start_pl.normalize();
            let mut dp = Vector3::dot_product(&segdir, &start_pl);
            let sp0 = if dp > 0.0f32 { start_pl } else { -start_pl };
            segment[0].start_plane = sp0;
            segment[0].edge_plane = [sp0, sp0];
            segment[1].start_plane = segment[0].start_plane;

            let last_isec = num_intersections[TOP_EDGE];
            for e in 0..2 {
                intersection[last_isec][e].point_count = 1;
                intersection[last_isec][e].next_segment_id = (numsegs + 1) as u32;
                intersection[last_isec][e].point = pts[pc - 1];
                intersection[last_isec][e].tex_v = tex_v[pc - 1];
                intersection[last_isec][e].fold = true;
                intersection[last_isec][e].parallel = false;
            }
            let last_point = pts[pc - 1];
            let last_plane = segment[numsegs].edge_plane;
            top = last_point - last_plane[TOP_EDGE] * Vector3::dot_product(&last_plane[TOP_EDGE], &last_point);
            top.normalize();
            intersection[last_isec][TOP_EDGE].direction = top;
            bottom = last_point - last_plane[BOTTOM_EDGE] * Vector3::dot_product(&last_plane[BOTTOM_EDGE], &last_point);
            bottom.normalize();
            intersection[last_isec][BOTTOM_EDGE].direction = bottom;

            segdir = pts[pc - 1] - pts[pc - 2];
            segdir.normalize();
            start_pl = Vector3::cross_product(&top, &bottom);
            start_pl.normalize();
            dp = Vector3::dot_product(&segdir, &start_pl);
            let spl = if dp > 0.0f32 { start_pl } else { -start_pl };
            segment[numsegs + 1].start_plane = spl;
            segment[numsegs + 1].edge_plane = [spl, spl];

            for ii in 2..num_intersections[TOP_EDGE] {
                let midpoint = pts[ii - 1];
                for e in 0..2 {
                    intersection[ii][e].point_count = 1;
                    intersection[ii][e].next_segment_id = ii as u32;
                    intersection[ii][e].point = midpoint;
                    // upstream: `personalities[intersectionIndex]&1` // LORENZEN
                    intersection[ii][e].tex_v = (personalities[if plant(19145) { ii - 1 } else { ii }] & 1) as f32;
                }
                for e in 0..2 {
                    let a = segment[ii - 1].edge_plane[e];
                    let b = segment[ii].edge_plane[e];
                    let vdp = Vector3::dot_product(&a, &b);
                    if wwmath::fabs(vdp) < parallel_factor {
                        let mut d = Vector3::cross_product(&a, &b);
                        d.normalize();
                        if Vector3::dot_product(&d, &midpoint) < 0.0f32 {
                            d = -d;
                        }
                        intersection[ii][e].direction = d;
                        intersection[ii][e].parallel = false;
                    } else {
                        let mut pl = if vdp > 0.0f32 { a + b } else { a - b };
                        pl.normalize();
                        let mut d = midpoint - pl * Vector3::dot_product(&pl, &midpoint);
                        d.normalize();
                        intersection[ii][e].direction = d;
                        intersection[ii][e].parallel = true;
                    }
                }
                start_pl = Vector3::cross_product(&intersection[ii][TOP_EDGE].direction, &intersection[ii][BOTTOM_EDGE].direction);
                start_pl.normalize();
                dp = Vector3::dot_product(&segment[ii].start_plane, &start_pl);
                segment[ii].start_plane = if dp > 0.0f32 { start_pl } else { -start_pl };
            }

            // Intersection merging: when an intersection is inside an adjacent segment and certain other
            // conditions hold, merge intersections to avoid polys folding over on themselves.
            if self.is_merge_intersections() {
                let mut merged = true;
                while merged {
                    merged = false;
                    for edge in 0..=BOTTOM_EDGE {
                        // Capture here because num_intersections[edge] will change inside the loop
                        let num_isects = num_intersections[edge];
                        let mut r = 1usize;
                        let mut w = 1usize;
                        while r < num_isects {
                            // Upstream's curr_int / write_int / the segment pointers are fixed at the top of the
                            // iteration; r moves inside the merge loop.
                            let r_top = r;
                            let curr_seg_id = intersection[r_top][edge].next_segment_id as usize;
                            let prev_seg_id = intersection[w - 1][edge].next_segment_id as usize;
                            let mut next_idx = r + 1;
                            let mut next_seg_id = intersection[next_idx][edge].next_segment_id as usize;

                            loop {
                                let curr = intersection[r_top][edge];
                                let next = intersection[next_idx][edge];
                                let cond = (!next.fold
                                    && Vector3::dot_product(&curr.direction, &segment[next_seg_id].start_plane) > 0.0f32
                                    && Vector3::dot_product(&curr.direction, &segment[next_seg_id].edge_plane[edge]) > 0.0f32)
                                    || (!curr.fold
                                        && Vector3::dot_product(&next.direction, &-segment[curr_seg_id].start_plane) > 0.0f32
                                        && Vector3::dot_product(&next.direction, &segment[prev_seg_id].edge_plane[edge]) > 0.0f32);
                                if !cond {
                                    break;
                                }

                                let new_count = curr.point_count + next.point_count;
                                let oo_new_count = 1.0f32 / new_count as f32;
                                let curr_factor = oo_new_count * curr.point_count as f32;
                                let next_factor = oo_new_count * curr.point_count as f32; // upstream quirk: curr's count
                                let new_point = curr.point * curr_factor + next.point * next_factor;
                                let new_tex_v = curr.tex_v * curr_factor + next.tex_v * next_factor;

                                let mut new_direction = Vector3::new(0.0, 0.0, 0.0); // uninitialised upstream
                                let new_parallel;
                                let pe = segment[prev_seg_id].edge_plane[edge];
                                let ne = segment[next_seg_id].edge_plane[edge];
                                let vdp = Vector3::dot_product(&pe, &ne);
                                if wwmath::fabs(vdp) < parallel_factor {
                                    new_direction = Vector3::cross_product(&pe, &ne);
                                    new_direction.normalize();
                                    if Vector3::dot_product(&new_direction, &new_point) < 0.0f32 {
                                        new_direction = -new_direction;
                                    }
                                    new_parallel = false;
                                } else {
                                    let mut pl = if vdp > 0.0f32 { pe + ne } else { pe - ne };
                                    pl.normalize();
                                    if curr.parallel {
                                        new_direction = new_direction - pl * Vector3::dot_product(&pl, &new_direction);
                                        new_direction.normalize();
                                    } else {
                                        new_direction = Vector3::cross_product(&segment[curr_seg_id].edge_plane[edge], &pl);
                                        new_direction.normalize();
                                    }
                                    new_parallel = true;
                                }

                                if self.merge_abort_factor > 0.0f32 {
                                    let abort_dist = radius * self.merge_abort_factor;
                                    let abort_dist2 = abort_dist * abort_dist;
                                    let diff_curr = curr.point - new_direction * Vector3::dot_product(&curr.point, &new_direction);
                                    if diff_curr.length2() > abort_dist2 {
                                        break;
                                    }
                                    let next_curr = next.point - new_direction * Vector3::dot_product(&next.point, &new_direction);
                                    if next_curr.length2() > abort_dist2 {
                                        break;
                                    }
                                }

                                merged = true;
                                {
                                    let c = &mut intersection[r_top][edge];
                                    c.direction = new_direction;
                                    c.parallel = new_parallel;
                                    c.point = new_point;
                                    c.tex_v = new_tex_v;
                                    c.point_count = new_count;
                                    c.next_segment_id = next.next_segment_id;
                                    c.fold = curr.fold || next.fold;
                                }
                                num_intersections[edge] -= 1;
                                r += 1;
                                if r == num_isects {
                                    break;
                                }
                                next_idx = r + 1;
                                next_seg_id = intersection[next_idx][edge].next_segment_id as usize;
                            }

                            // Copy the (possibly merged) intersection down: write_int <- curr_int (no Parallel).
                            let c = intersection[r_top][edge];
                            let wi = &mut intersection[w][edge];
                            wi.point_count = c.point_count;
                            wi.next_segment_id = c.next_segment_id;
                            wi.point = c.point;
                            wi.tex_v = c.tex_v;
                            wi.direction = c.direction;
                            wi.fold = c.fold;

                            r += 1;
                            w += 1;
                        }
                        if r == num_isects {
                            let c = intersection[r][edge];
                            let wi = &mut intersection[w][edge];
                            wi.point_count = c.point_count;
                            wi.next_segment_id = c.next_segment_id;
                            wi.point = c.point;
                            wi.tex_v = c.tex_v;
                            wi.direction = c.direction;
                            wi.fold = c.fold;
                        }
                    }
                }
            }

            // Find vertex positions, generate vertices and triangles. With top/bottom intersections merged we skip
            // points if both are merged, generate triangle fans if one side is merged, strips otherwise.
            let vnum = num_intersections[TOP_EDGE] + num_intersections[BOTTOM_EDGE];
            // VertexFormatXYZUV1 array: (position, u1, v1)
            let mut vertex_array: Vec<(Vector3, f32, f32)> = Vec::with_capacity(vnum);
            let mut v_index_array: Vec<[u16; 3]> = Vec::new();
            let mut vertex_index = 0usize;

            let top_dir = intersection[1][TOP_EDGE].direction;
            let mut top = top_dir * Vector3::dot_product(&pts[0], &top_dir);
            let bottom_dir = intersection[1][BOTTOM_EDGE].direction;
            let mut bottom = bottom_dir * Vector3::dot_product(&pts[0], &bottom_dir);
            vertex_array.push((top, u_values[0], intersection[1][TOP_EDGE].tex_v));
            vertex_index += 1;
            vertex_array.push((bottom, u_values[1], intersection[1][BOTTOM_EDGE].tex_v));
            vertex_index += 1;

            let mut last_top_vertex_index = 0usize;
            let mut last_bottom_vertex_index = 1usize;
            let mut top_int_idx = 1usize; // Skip "pre-first-point" dummy intersection
            let mut bottom_int_idx = 1usize;
            let mut point_index = 0usize;

            let mut residual_top_points = intersection[1][TOP_EDGE].point_count;
            let mut residual_bottom_points = intersection[1][BOTTOM_EDGE].point_count;
            let mut delta = residual_top_points.min(residual_bottom_points) - 1;
            residual_top_points -= delta;
            residual_bottom_points -= delta;
            point_index += delta as usize;

            loop {
                if residual_top_points == 1 && residual_bottom_points == 1 {
                    // Both sides advance: a quad (two triangles).
                    v_index_array.push([last_top_vertex_index as u16, last_bottom_vertex_index as u16, vertex_index as u16]);
                    v_index_array.push([last_bottom_vertex_index as u16, (vertex_index + 1) as u16, vertex_index as u16]);
                    last_top_vertex_index = vertex_index;
                    last_bottom_vertex_index = vertex_index + 1;
                    top_int_idx += 1;
                    bottom_int_idx += 1;
                    residual_top_points = intersection[top_int_idx][TOP_EDGE].point_count;
                    residual_bottom_points = intersection[bottom_int_idx][BOTTOM_EDGE].point_count;
                    point_index += 1;
                    let top_dir = intersection[top_int_idx][TOP_EDGE].direction;
                    top = top_dir * Vector3::dot_product(&pts[point_index], &top_dir);
                    let bottom_dir = intersection[bottom_int_idx][BOTTOM_EDGE].direction;
                    bottom = bottom_dir * Vector3::dot_product(&pts[point_index], &bottom_dir);
                    vertex_array.push((top, u_values[0], intersection[top_int_idx][TOP_EDGE].tex_v));
                    vertex_index += 1;
                    vertex_array.push((bottom, u_values[1], intersection[bottom_int_idx][BOTTOM_EDGE].tex_v));
                    vertex_index += 1;
                } else if residual_top_points > 1 {
                    // Top is merged: a fan around the top vertex; advance the bottom.
                    v_index_array.push([last_top_vertex_index as u16, last_bottom_vertex_index as u16, vertex_index as u16]);
                    last_bottom_vertex_index = vertex_index;
                    residual_top_points -= 1;
                    bottom_int_idx += 1;
                    residual_bottom_points = intersection[bottom_int_idx][BOTTOM_EDGE].point_count;
                    point_index += 1;
                    let bottom_dir = intersection[bottom_int_idx][BOTTOM_EDGE].direction;
                    bottom = bottom_dir * Vector3::dot_product(&pts[point_index], &bottom_dir);
                    vertex_array.push((bottom, u_values[1], intersection[bottom_int_idx][BOTTOM_EDGE].tex_v));
                    vertex_index += 1;
                } else {
                    // Bottom is merged: a fan around the bottom vertex; advance the top.
                    v_index_array.push([last_top_vertex_index as u16, last_bottom_vertex_index as u16, vertex_index as u16]);
                    last_top_vertex_index = vertex_index;
                    residual_bottom_points -= 1;
                    top_int_idx += 1;
                    residual_top_points = intersection[top_int_idx][TOP_EDGE].point_count;
                    point_index += 1;
                    let top_dir = intersection[top_int_idx][TOP_EDGE].direction;
                    top = top_dir * Vector3::dot_product(&pts[point_index], &top_dir);
                    vertex_array.push((top, u_values[0], intersection[top_int_idx][TOP_EDGE].tex_v));
                    vertex_index += 1;
                }

                delta = residual_top_points.min(residual_bottom_points) - 1;
                residual_top_points -= delta;
                residual_bottom_points -= delta;
                point_index += delta as usize;

                if (top_int_idx >= num_intersections[TOP_EDGE] && residual_top_points == 1)
                    || (bottom_int_idx >= num_intersections[BOTTOM_EDGE] && residual_bottom_points == 1)
                {
                    debug_assert!(top_int_idx == num_intersections[TOP_EDGE]);
                    debug_assert!(bottom_int_idx == num_intersections[BOTTOM_EDGE]);
                    debug_assert!(point_index == pc - 1);
                    break;
                }
            }

            // Set color, opacity, vertex flags
            let sorting = !self.is_sorting_disabled()
                && self.shader.get_dst_blend_func() != DSTBLEND_ZERO
                && (self.shader.get_alpha_test() == 0) != plant(19148); // ALPHATEST_DISABLE
            let mut shader = self.shader;
            shader.set_cull_mode(0); // CULL_MODE_DISABLE
            shader = ShaderClass((shader.0 & !MASK_PRIGRADIENT) | (GRADIENT_MODULATE << SHIFT_PRIGRADIENT));
            // upstream: DX8Wrapper::Set_Material(PRELIT_DIFFUSE) — device-only; recorded as `prelit_diffuse`
            shader = ShaderClass((shader.0 & !MASK_TEXTURING) | ((if self.texture.is_some() { 1 } else { 0 }) << SHIFT_TEXTURING));

            // Copy into the (dynamic) vertex buffer: colour per vertex is colors[MIN(i/2, point_cnt)]
            let mut vertices = Vec::with_capacity(vnum);
            for (i, v) in vertex_array.iter().enumerate() {
                let ci = (i / 2 + if plant(19149) { 1 } else { 0 }).min(point_cnt as usize).min(colors.len().saturating_sub(1)); // clamp: upstream may read past the end
                vertices.push(SegLineVertex { position: v.0, diffuse: convert_color_clamp(&colors[ci]), u: v.1, v: v.2 });
            }
            chunks.push(SegLineChunk {
                vertices,
                triangles: v_index_array,
                shader,
                prelit_diffuse: true,
                sorting,
                texture: self.texture.clone(),
            });
            // upstream: Set_Index_Buffer/Set_Vertex_Buffer/Set_Texture/Set_Shader, then
            // SortingRendererClass::Insert_Triangles or Draw_Triangles — device-only, not translated

            chunk_index += chunk_size - 1;
        }
        // upstream: DX8Wrapper::Set_Transform(D3DTS_VIEW, view) — device-only, not translated
        chunks
    }
}
