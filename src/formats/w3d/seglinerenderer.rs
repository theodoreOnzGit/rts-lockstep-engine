// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/seglinerenderer.cpp, seglinerenderer.h
//   (SegLineRendererClass: the constructor, Init, Set_Texture_Tile_Factor,
//   Reset_Line, Render, subdivision_util, Scale, the inline setters and
//   getters; MAX_SEGLINE_SUBDIV_LEVELS, SEGLINE_CHUNK_SIZE)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vp.cpp
//   (VectorProcessorClass::Transform(Vector3 *, const Vector3 *, const Matrix3D &, int), the non-SSE path)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, seglinerenderer by Naty Hoffman and Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SegLineRendererClass` (GitHub #65): the camera-space triangle strip a
//! segmented line becomes. It draws the line-mode particles of a W3D emitter
//! ([`super::part_buf`]'s `Render_Line` hands it one run per group ID) and,
//! upstream, `SegmentedLineClass`.
//!
//! # As upstream does it
//!
//! - **Made** (`SegLineRendererClass()`): no texture, the additive sprite
//!   shader, width 0, white, opacity 1, subdivision level 0, no noise, a
//!   merge abort factor of 1.5, a tile factor of 1, the UV offset 0 at the
//!   current sync time, no UV scroll, intersections merged, the uniform-width
//!   mapping. `Init(props)` takes the line properties of a W3D emitter: the
//!   four flags (`W3D_ELINE_*`), the mapping mode (an unknown one leaves the
//!   mode as it was), the subdivision level, noise amplitude, merge abort
//!   factor, tile factor (`Set_Texture_Tile_Factor`: at most 50, `MAX(f, 0)`)
//!   and the UV scroll (per second, kept per millisecond: `rate * 0.001`).
//! - **`Render`** (once per line): the UV offset advances by the milliseconds
//!   since the last render and keeps its fraction (`x - floorf(x)`). The
//!   points are cut into chunks of `(128 >> SubdivisionLevel) + 1` (the last
//!   point of a chunk is the first of the next); each chunk is one draw:
//!   1. the points to eye space (`view * transform`, the view's top three
//!      rows); a texture V per point (0, or `index * TileFactor` along the
//!      whole line for the length and tiled mappings) and the two U values (0
//!      and 1, both 0 for the length mapping);
//!   2. `subdivision_util`: each segment halved `SubdivisionLevel` times, each
//!      midpoint moved by a random vector in the unit box times the noise
//!      amplitude (halved per level); V interpolated, the colour the
//!      segment's first point's (see the quirks; the line's colour and
//!      opacity when no colours are given);
//!   3. per segment, the two silhouette planes through the eye (top and
//!      bottom), a point moved by 0.001 in X when it equals the one before
//!      within 0.0001; where the view plane of two segments turns by more than
//!      90 degrees the edges swap (a fold);
//!   4. per point, the top and bottom intersection lines of the adjoining
//!      segments' planes (the closest line on their average plane when they
//!      are within `0.9` of parallel), the first and last from the end
//!      segments' planes, and each segment's start plane;
//!   5. with intersection merging, repeated until a pass merges nothing: per
//!      edge, an intersection inside the next segment's (or the previous
//!      inside this one's) start and edge planes is merged with the next,
//!      never across a fold, unless the merged line passes farther than
//!      `radius * MergeAbortFactor` from either point;
//!   6. the vertices: the point nearest each intersection line to the point
//!      it stands for, two per point where neither edge merged, one on the
//!      side that did not merge otherwise (a fan); two triangles per strip
//!      step, one per fan step; colours packed by `DX8Wrapper::Convert_Color`
//!      (chopped, not clamped), U and V plus the offset;
//!   7. the shader: culling off, the primary gradient modulating, texturing
//!      on with a texture; sorted unless sorting is disabled, the destination
//!      blend is `ZERO` or alpha testing is on.
//!
//! # Upstream quirks that are kept
//!
//! - A merge weighs both points by the current intersection's count
//!   (`next_factor` is `oo_new_count * curr_int->PointCount`).
//! - The copy of a merged intersection to its place leaves out `Parallel`:
//!   the slot keeps the flag of whatever stood there before.
//! - The segment planes a merge is tested against (`curr_seg`, `prev_seg`)
//!   are found once per intersection, not again after a merge moves its next
//!   segment.
//! - The vertex material is the diffuse one whatever the colour
//!   (`!rgba_all || rgba != 0` is always true).
//! - The frozen randomizer is a fresh `Random3Class` per chunk, and its three
//!   draws are the arguments of one `Set` call: their order is unspecified
//!   in C++; taken left to right, as the compiler of the differential tests
//!   does (as [`super::part_buf`]'s colour table).
//! - `subdivision_util` moves the start of the second half of a split
//!   segment to the midpoint (position and V) but not its colour: every
//!   point a segment is subdivided into takes the colour of the segment's
//!   first point (the interpolated end colours are never emitted).
//! - The UV offset is advanced once per `Render`: a second line drawn at the
//!   same sync time does not move it.
//!
//! # Undefined upstream ([`W3dUndefined`])
//!
//! - **A parallel merge of a parallel intersection** reads the merged
//!   direction before setting it (`new_direction` is a `Vector3` with no
//!   constructor): ~~[`W3dUndefined::UninitialisedRead`].~~
//!   Not rare: 8,520 of the 14,527 merging sequences of the differential test's
//!   20,000 random sequences reach it (2026-11-14). Upstream most likely
//!   meant the current intersection's direction; ~~what to draw instead is
//!   the owner's decision (none is drawn here).~~
//!   **Corrected 2026-11-14 (#185):** the owner's decision on GitHub #185
//!   (2026-11-14, option (b)): `new_direction` starts as the current
//!   intersection's own direction (`curr_int->Direction`) at that read, and
//!   the merge goes on (`new_direction - pl * Dot(pl, new_direction)`,
//!   normalized). **A deliberate deviation from upstream**, whose value there
//!   is undefined; the differential test's C++ makes the same substitution
//!   (one asserted edit to the verbatim cut in `build.rs`).
//! - **A subdivision level over 7** (`MAX_SEGLINE_SUBDIV_LEVELS`): the chunks
//!   are 1 point (the loop never advances) and the buffers overflow;
//!   **no points** (`num_points - 1` wraps): [`W3dUndefined::IndexOutOfRange`].
//!
//! # Where this differs from upstream
//!
//! - The DirectX 8 device is replaced: each chunk's vertices, triangles,
//!   shader, material and sorting are returned ([`SegLineChunk`]) for the
//!   client to draw, in eye space as upstream hands them to the device with
//!   identity world and view transforms. The texture is its name.
//! - A parallel merge of a parallel intersection starts its direction from
//!   the current intersection's (`curr_int->Direction`), where upstream reads
//!   an uninitialised `new_direction`: the owner's decision on GitHub #185
//!   (2026-11-14, option (b)); see the undefined cases above.

use super::mesh::dx8_convert_color;
use super::mesh::shader_bits::{MASK_PRIGRADIENT, MASK_TEXTURING, SHIFT_PRIGRADIENT, SHIFT_TEXTURING};
use super::shader::{ShaderClass, DSTBLEND_ZERO, GRADIENT_MODULATE, PRESET_ADDITIVE_SPRITE_SHADER};
use super::w3d_file::W3dEmitterLinePropertiesStruct;
use super::{W3dResult, W3dUndefined};
use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::random::Random3;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::{equal_within_epsilon, Vector3};
use crate::ww::vector4::Vector4;
use crate::ww::wwmath;

/// `MAX_SEGLINE_SUBDIV_LEVELS`.
pub const MAX_SEGLINE_SUBDIV_LEVELS: u32 = 7;
/// `SEGLINE_CHUNK_SIZE` (128: `MAX_SEGLINE_SUBDIV_LEVELS` is not over 7).
pub const SEGLINE_CHUNK_SIZE: u32 = 128;
/// `Set_Texture_Tile_Factor`'s `MAX_LINE_TILING_FACTOR`.
pub const MAX_LINE_TILING_FACTOR: f32 = 50.0;

/// `W3D_ELINE_*` (w3d_file.h).
pub const W3D_ELINE_MERGE_INTERSECTIONS: u32 = 0x0000_0001;
pub const W3D_ELINE_FREEZE_RANDOM: u32 = 0x0000_0002;
pub const W3D_ELINE_DISABLE_SORTING: u32 = 0x0000_0004;
pub const W3D_ELINE_END_CAPS: u32 = 0x0000_0008;
pub const W3D_ELINE_TEXTURE_MAP_MODE_MASK: u32 = 0xFF00_0000;
pub const W3D_ELINE_TEXTURE_MAP_MODE_OFFSET: u32 = 24;
pub const W3D_ELINE_UNIFORM_WIDTH_TEXTURE_MAP: u32 = 0;
pub const W3D_ELINE_UNIFORM_LENGTH_TEXTURE_MAP: u32 = 1;
pub const W3D_ELINE_TILED_TEXTURE_MAP: u32 = 2;

/// `SegLineRendererClass::TextureMapMode`.
pub const UNIFORM_WIDTH_TEXTURE_MAP: u32 = 0;
pub const UNIFORM_LENGTH_TEXTURE_MAP: u32 = 1;
pub const TILED_TEXTURE_MAP: u32 = 2;

/// The `Bits` flags (seglinerenderer.h).
const MERGE_INTERSECTIONS: u32 = 0x0000_0001;
const FREEZE_RANDOM: u32 = 0x0000_0002;
const DISABLE_SORTING: u32 = 0x0000_0004;
const END_CAPS: u32 = 0x0000_0008;
const TEXTURE_MAP_MODE_MASK: u32 = 0xFF00_0000;
const TEXTURE_MAP_MODE_OFFSET: u32 = 24;
const DEFAULT_BITS: u32 = MERGE_INTERSECTIONS | (UNIFORM_WIDTH_TEXTURE_MAP << TEXTURE_MAP_MODE_OFFSET);

/// `ShaderClass::CULL_MODE_DISABLE`, `TEXTURING_DISABLE`, `TEXTURING_ENABLE`,
/// `ALPHATEST_DISABLE`.
const CULL_MODE_DISABLE: u32 = 0;
const TEXTURING_DISABLE: u32 = 0;
const TEXTURING_ENABLE: u32 = 1;
const ALPHATEST_DISABLE: u32 = 0;

/// `1.0f / (float)INT_MAX` (2^-31), `subdivision_util`'s `oo_int_max`.
const OO_INT_MAX: f32 = 1.0 / 2_147_483_648.0;

/// The edges (`SegmentEdge`): `TOP_EDGE`, `BOTTOM_EDGE`.
const TOP_EDGE: usize = 0;
const BOTTOM_EDGE: usize = 1;

/// One vertex as `Render` writes it (`VertexFormatXYZDUV1`): eye space, the
/// packed ARGB colour, the texture coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SegLineVertex {
    pub position: Vector3,
    pub diffuse: u32,
    pub u: f32,
    pub v: f32,
}

/// One chunk's draw: its vertices and triangles (`TriIndex`, unsigned
/// shorts), the shader it set, the vertex material (`PRELIT_DIFFUSE`, else
/// `PRELIT_NODIFFUSE`), whether it went to the sorting renderer, the texture.
#[derive(Clone, Debug, PartialEq)]
pub struct SegLineChunk {
    pub vertices: Vec<SegLineVertex>,
    pub triangles: Vec<[u16; 3]>,
    pub shader: ShaderClass,
    pub prelit_diffuse: bool,
    pub sorting: bool,
    pub texture: Option<Vec<u8>>,
}

/// `class SegLineRendererClass` (no vertex buffer: the chunks are returned).
#[derive(Clone, Debug, PartialEq)]
pub struct SegLineRenderer {
    /// `Texture`: its name, `None` for null.
    pub texture: Option<Vec<u8>>,
    pub shader: ShaderClass,
    pub width: f32,
    pub color: Vector3,
    pub opacity: f32,
    pub subdivision_level: u32,
    pub noise_amplitude: f32,
    pub merge_abort_factor: f32,
    pub texture_tile_factor: f32,
    pub last_used_sync_time: u32,
    pub current_uv_offset: Vector2,
    pub uv_offset_delta_per_ms: Vector2,
    pub bits: u32,
}

/// `LineSegment`: the start plane (the segment's direction until the start
/// planes are found) and the two edge planes, each a normal through the eye.
#[derive(Clone, Copy, Debug, Default)]
struct LineSegment {
    start_plane: Vector3,
    edge_plane: [Vector3; 2],
}

/// `LineSegmentIntersection`.
#[derive(Clone, Copy, Debug, Default)]
struct LineSegmentIntersection {
    point_count: u32,
    next_segment_id: u32,
    direction: Vector3,
    point: Vector3,
    tex_v: f32,
    rgba: Vector4,
    fold: bool,
    parallel: bool,
}

/// `SegLineSubdivision`.
#[derive(Clone, Copy, Debug, Default)]
struct SegLineSubdivision {
    start_pos: Vector3,
    end_pos: Vector3,
    start_tex_v: f32,
    end_tex_v: f32,
    start_diffuse: Vector4,
    end_diffuse: Vector4,
    rand: f32,
    level: u32,
}

/// The two planes' intersection line, or, when they are within
/// `parallel_factor` of parallel, the line on their average plane nearest
/// `midpoint` (`Render`'s top and bottom intersection blocks, which are the
/// same text for each edge). Returns the direction and whether it was parallel.
fn intersect_planes(a: &Vector3, b: &Vector3, midpoint: &Vector3, parallel_factor: f32) -> (Vector3, bool) {
    let vdp = Vector3::dot_product(a, b);
    if wwmath::fabs(vdp) < parallel_factor {
        // Not parallel - intersect planes to get line (get vector, normalize it, ensure it is
        // pointing towards the midpoint)
        let mut d = Vector3::cross_product(a, b);
        d.normalize();
        if Vector3::dot_product(&d, midpoint) < 0.0f32 {
            d = -d;
        }
        (d, false)
    } else {
        // Parallel (or almost): find point on av. plane closest to midpoint, convert to line
        // Ensure average calculation is numerically stable:
        let mut pl = if vdp > 0.0f32 || plant(10104) { *a + *b } else { *a - *b };
        pl.normalize();
        let mut d = *midpoint - pl * Vector3::dot_product(&pl, midpoint);
        d.normalize();
        (d, true)
    }
}

/// The plane through two direction lines, pointing the way `toward` does
/// (`Cross_Product`, `Normalize`, then the sign of the dot product).
fn oriented_plane(top: &Vector3, bottom: &Vector3, toward: &Vector3) -> Vector3 {
    let mut start_pl = Vector3::cross_product(top, bottom);
    start_pl.normalize();
    let dp = Vector3::dot_product(toward, &start_pl);
    if dp > 0.0f32 {
        start_pl
    } else {
        -start_pl
    }
}

impl SegLineRenderer {
    /// `SegLineRendererClass()` at sync time `sync_time`
    /// (`WW3D::Get_Sync_Time()`).
    pub fn new(sync_time: u32) -> Self {
        SegLineRenderer {
            texture: None,
            shader: PRESET_ADDITIVE_SPRITE_SHADER,
            width: 0.0,
            color: Vector3::new(1.0, 1.0, 1.0),
            opacity: 1.0,
            subdivision_level: 0,
            noise_amplitude: 0.0,
            merge_abort_factor: 1.5,
            texture_tile_factor: 1.0,
            last_used_sync_time: sync_time,
            current_uv_offset: Vector2::new(0.0, 0.0),
            uv_offset_delta_per_ms: Vector2::new(0.0, 0.0),
            bits: DEFAULT_BITS,
        }
    }

    fn set_bit(&mut self, bit: u32, onoff: bool) {
        if onoff {
            self.bits |= bit;
        } else {
            self.bits &= !bit;
        }
    }

    /// `Set_Merge_Intersections(onoff)`.
    pub fn set_merge_intersections(&mut self, onoff: bool) {
        self.set_bit(MERGE_INTERSECTIONS, onoff);
    }
    /// `Set_Freeze_Random(onoff)`.
    pub fn set_freeze_random(&mut self, onoff: bool) {
        self.set_bit(FREEZE_RANDOM, onoff);
    }
    /// `Set_Disable_Sorting(onoff)`.
    pub fn set_disable_sorting(&mut self, onoff: bool) {
        self.set_bit(DISABLE_SORTING, onoff);
    }
    /// `Set_End_Caps(onoff)`.
    pub fn set_end_caps(&mut self, onoff: bool) {
        self.set_bit(END_CAPS, onoff);
    }
    /// `Is_Merge_Intersections()`.
    pub fn is_merge_intersections(&self) -> bool {
        self.bits & MERGE_INTERSECTIONS != 0
    }
    /// `Is_Freeze_Random()`.
    pub fn is_freeze_random(&self) -> bool {
        self.bits & FREEZE_RANDOM != 0
    }
    /// `Is_Sorting_Disabled()`.
    pub fn is_sorting_disabled(&self) -> bool {
        self.bits & DISABLE_SORTING != 0
    }
    /// `Are_End_Caps_Enabled()` (stored; `Render` never reads it).
    pub fn are_end_caps_enabled(&self) -> bool {
        self.bits & END_CAPS != 0
    }
    /// `Get_Texture_Mapping_Mode()`.
    pub fn get_texture_mapping_mode(&self) -> u32 {
        (self.bits & TEXTURE_MAP_MODE_MASK) >> TEXTURE_MAP_MODE_OFFSET
    }
    /// `Set_Texture_Mapping_Mode(mode)`.
    pub fn set_texture_mapping_mode(&mut self, mode: u32) {
        self.bits &= !TEXTURE_MAP_MODE_MASK;
        self.bits |= (mode << TEXTURE_MAP_MODE_OFFSET) & TEXTURE_MAP_MODE_MASK;
    }
    /// `Set_UV_Offset_Rate(rate)`: kept per millisecond.
    pub fn set_uv_offset_rate(&mut self, rate: Vector2) {
        self.uv_offset_delta_per_ms = rate * 0.001f32;
    }
    /// `Get_UV_Offset_Rate()`.
    pub fn get_uv_offset_rate(&self) -> Vector2 {
        self.uv_offset_delta_per_ms * 1000.0f32
    }

    /// `Set_Texture_Tile_Factor(factor)`: at most `MAX_LINE_TILING_FACTOR`,
    /// else `MAX(factor, 0)` (a NaN is 0).
    pub fn set_texture_tile_factor(&mut self, mut factor: f32) {
        if factor > MAX_LINE_TILING_FACTOR {
            factor = if plant(10115) { 40.0 } else { MAX_LINE_TILING_FACTOR };
        } else {
            factor = if factor > 0.0f32 { factor } else { 0.0f32 };
        }
        self.texture_tile_factor = factor;
    }

    /// `Init(props)`.
    pub fn init(&mut self, props: &W3dEmitterLinePropertiesStruct) {
        // translate the flags
        self.set_merge_intersections(props.flags & W3D_ELINE_MERGE_INTERSECTIONS != 0);
        self.set_freeze_random(props.flags & W3D_ELINE_FREEZE_RANDOM != 0);
        self.set_disable_sorting(props.flags & W3D_ELINE_DISABLE_SORTING != 0);
        self.set_end_caps(props.flags & W3D_ELINE_END_CAPS != 0);
        let texture_mode = (props.flags & W3D_ELINE_TEXTURE_MAP_MODE_MASK) >> W3D_ELINE_TEXTURE_MAP_MODE_OFFSET;
        match texture_mode {
            W3D_ELINE_UNIFORM_WIDTH_TEXTURE_MAP => self.set_texture_mapping_mode(UNIFORM_WIDTH_TEXTURE_MAP),
            W3D_ELINE_UNIFORM_LENGTH_TEXTURE_MAP => self.set_texture_mapping_mode(UNIFORM_LENGTH_TEXTURE_MAP),
            W3D_ELINE_TILED_TEXTURE_MAP => self.set_texture_mapping_mode(TILED_TEXTURE_MAP),
            _ => {}
        }
        // install all other settings
        self.subdivision_level = props.subdivision_level;
        self.noise_amplitude = props.noise_amplitude;
        self.merge_abort_factor = props.merge_abort_factor;
        self.set_texture_tile_factor(props.texture_tile_factor);
        self.set_uv_offset_rate(Vector2::new(props.u_per_sec, props.v_per_sec));
    }

    /// `Reset_Line()`: the UV offset back to 0 at sync time `sync_time`.
    pub fn reset_line(&mut self, sync_time: u32) {
        self.last_used_sync_time = sync_time;
        self.current_uv_offset.set(0.0, 0.0);
    }

    /// `Scale(scale)`: the width and the noise amplitude.
    pub fn scale(&mut self, scale: f32) {
        self.width *= scale;
        self.noise_amplitude *= scale;
    }

    /// `Render(rinfo, transform, num_points, points, obj_sphere, rgbas)` at
    /// sync time `sync_time` with the camera's view transform `view`
    /// (`DX8Wrapper::Get_Transform(D3DTS_VIEW)`, Westwood's convention; only
    /// its top three rows are used). `v3` is the randomizers' shared
    /// generator (`Vector3Randomizer::Randomizer`), which the noise draws
    /// from unless the random offsets are frozen. Returns one draw per chunk.
    pub fn render(
        &mut self,
        sync_time: u32,
        view: &Matrix3D,
        transform: &Matrix3D,
        points: &[Vector3],
        rgbas: Option<&[Vector4]>,
        v3: &mut Random3,
    ) -> W3dResult<Vec<SegLineChunk>> {
        // Handle texture UV offset animation (done once for entire line).
        let delta = sync_time.wrapping_sub(self.last_used_sync_time);
        let del = delta as f32;
        let mut uv_offset = self.current_uv_offset + self.uv_offset_delta_per_ms * del;
        // ensure offsets are in [0, 1] range:
        uv_offset.x -= wwmath::floor(uv_offset.x);
        if !plant(10096) {
            uv_offset.y -= wwmath::floor(uv_offset.y);
        }
        // Update state
        self.current_uv_offset = uv_offset;
        self.last_used_sync_time = sync_time;

        let map_mode = self.get_texture_mapping_mode();
        let num_points = points.len() as u32;
        if num_points == 0 {
            // num_points - 1 wraps: the chunk loop reads past the points
            return Err(W3dUndefined::IndexOutOfRange.into());
        }
        if num_points == 1 {
            // (chidx < num_points - 1 never holds: nothing is drawn)
            return Ok(Vec::new());
        }
        if self.subdivision_level > MAX_SEGLINE_SUBDIV_LEVELS {
            // chunks of 1 point (the loop never advances), the subdivision buffers overflow
            return Err(W3dUndefined::UninitialisedRead.into());
        }
        let mut chunks = Vec::new();

        // We reduce the chunk size to take account of subdivision levels.
        let mut chunk_size = (SEGLINE_CHUNK_SIZE >> self.subdivision_level) + 1;
        if plant(10097) {
            chunk_size = (SEGLINE_CHUNK_SIZE >> self.subdivision_level) + 2;
        }
        if chunk_size > num_points {
            chunk_size = num_points;
        }

        // View matrix: the top three rows of the 4x4 (`view2`).
        let mut modelview = Matrix3D::default();
        modelview.mul(view, transform);

        let mut chidx = 0u32;
        while chidx < num_points - 1 {
            let mut point_cnt = (num_points - chidx).min(chunk_size);

            // Transform points in chunk from objectspace to eyespace
            // (VectorProcessorClass::Transform: mulVector3Array)
            let xformed_pts: Vec<Vector3> =
                points[chidx as usize..(chidx + point_cnt) as usize].iter().map(|p| modelview.mul_vector3(p)).collect();

            // Prepare v parameter per point - used for texture mapping
            let mut base_tex_v = vec![0.0f32; point_cnt as usize];
            let u_values: [f32; 2];
            match map_mode {
                UNIFORM_WIDTH_TEXTURE_MAP => {
                    // All 0
                    u_values = [0.0, 1.0];
                }
                UNIFORM_LENGTH_TEXTURE_MAP => {
                    for (pidx, v) in base_tex_v.iter_mut().enumerate() {
                        // Increasing V
                        *v = (pidx as u32 + chidx) as f32 * self.texture_tile_factor;
                    }
                    u_values = [0.0, 0.0];
                }
                _ => {
                    // TILED_TEXTURE_MAP (Set_Texture_Mapping_Mode only ever stores the three modes)
                    for (pidx, v) in base_tex_v.iter_mut().enumerate() {
                        let k = if plant(10098) { pidx as u32 } else { pidx as u32 + chidx };
                        *v = k as f32 * self.texture_tile_factor;
                    }
                    u_values = [0.0, 1.0];
                }
            }

            // Fractal noise recursive subdivision
            let rgbas_pointer = rgbas.map(|c| &c[chidx as usize..(chidx + point_cnt) as usize]);
            let (mut pts, tex_v, diffuse) = self.subdivision_util(&xformed_pts, &base_tex_v, rgbas_pointer, v3);
            point_cnt = pts.len() as u32;
            let pc = point_cnt as usize;

            // Calculate line segment edge planes
            let mut switch_edges = false;
            let mut segment = vec![LineSegment::default(); pc + 1];
            let mut intersection = vec![[LineSegmentIntersection::default(); 2]; pc + 1];
            let radius = self.width * 0.5f32;

            for sidx in 1..pc {
                // #segments = #points - 1 (+ 2 dummy segments)
                let curr_point = pts[sidx - 1];
                if equal_within_epsilon(&curr_point, &pts[sidx], 0.0001f32) {
                    pts[sidx].x += 0.001f32;
                }
                let next_point = pts[sidx];

                // We temporarily store the segment direction in the segment's StartPlane
                let mut segdir = next_point - curr_point;
                segdir.normalize();
                segment[sidx].start_plane = segdir;

                // Find nearest point on infinite line to eye (origin)
                let nearest = curr_point + segdir * -Vector3::dot_product(&segdir, &curr_point);

                // Find top and bottom points on cylinder
                let mut offset = Vector3::cross_product(&segdir, &nearest);
                offset.normalize();
                let top = curr_point + offset * radius;
                let bottom = curr_point + offset * -radius;

                // Find planes through top/bottom points and eyepoint.
                let mut top_normal = Vector3::cross_product(&top, &segdir);
                top_normal.normalize();
                segment[sidx].edge_plane[TOP_EDGE] = top_normal;

                let mut bottom_normal = Vector3::cross_product(&segdir, &bottom);
                bottom_normal.normalize();
                segment[sidx].edge_plane[BOTTOM_EDGE] = bottom_normal;

                // A fold where the planes of the eye and the two segments turn by over 90 degrees
                if sidx > 1 {
                    let mut prev_plane = Vector3::cross_product(&pts[sidx - 2], &curr_point);
                    prev_plane.normalize();
                    let mut curr_plane = Vector3::cross_product(&curr_point, &next_point);
                    curr_plane.normalize();
                    let fold = Vector3::dot_product(&prev_plane, &curr_plane) < 0.0f32;
                    if fold && !plant(10099) {
                        switch_edges = !switch_edges;
                    }
                    intersection[sidx][TOP_EDGE].fold = fold;
                    intersection[sidx][BOTTOM_EDGE].fold = fold;
                }

                if switch_edges {
                    // We switch signs so the normals will always point inwards
                    segment[sidx].edge_plane[TOP_EDGE] = -bottom_normal;
                    segment[sidx].edge_plane[BOTTOM_EDGE] = -top_normal;
                }
            }

            // Calculate segment edge intersections
            let numsegs = pc - 1; // Doesn't include the two dummy segments
            let mut num_intersections = [point_cnt, point_cnt];

            // Initialize pre-first point dummy intersection record (only NextSegmentID will be used).
            for e in [TOP_EDGE, BOTTOM_EDGE] {
                intersection[0][e] = LineSegmentIntersection {
                    point_count: 0,
                    next_segment_id: 0,
                    direction: Vector3::new(1.0, 0.0, 0.0),
                    point: Vector3::new(0.0, 0.0, 0.0),
                    tex_v: 0.0,
                    rgba: Vector4::new(0.0, 0.0, 0.0, 0.0),
                    fold: true,
                    parallel: false,
                };
            }

            // Initialize first point "intersection" record.
            for e in [TOP_EDGE, BOTTOM_EDGE] {
                let i = &mut intersection[1][e];
                i.point_count = 1;
                i.next_segment_id = 1;
                i.point = pts[0];
                i.tex_v = tex_v[0];
                i.rgba = diffuse[0];
                i.fold = true;
                i.parallel = false;
            }

            // Find closest point to 1st top/bottom segment edge plane, and convert to direction vector
            // and dummy segment edge plane.
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
            let pl = oriented_plane(&top, &bottom, &segdir);
            segment[0] = LineSegment { start_plane: pl, edge_plane: [pl, pl] };

            // Initialize StartPlane for the first "real" segment
            segment[1].start_plane = segment[0].start_plane;

            // Initialize last point "intersection" record.
            let last_isec = num_intersections[TOP_EDGE] as usize;
            for e in [TOP_EDGE, BOTTOM_EDGE] {
                let i = &mut intersection[last_isec][e];
                i.point_count = 1;
                i.next_segment_id = numsegs as u32 + 1; // Last dummy segment
                i.point = pts[pc - 1];
                i.tex_v = tex_v[pc - 1];
                i.rgba = diffuse[pc - 1];
                i.fold = true;
                i.parallel = false;
            }

            // Find closest point to last top/bottom segment edge plane, and convert to direction vector
            // and dummy segment edge vector
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
            let pl = oriented_plane(&top, &bottom, &segdir);
            segment[numsegs + 1] = LineSegment { start_plane: pl, edge_plane: [pl, pl] };

            // Calculate midpoint segment intersections.
            let parallel_factor = if plant(10100) { 0.99f32 } else { 0.9f32 };
            for iidx in 2..num_intersections[TOP_EDGE] as usize {
                // Relevant midpoint:
                let midpoint = pts[iidx - 1];
                let mid_tex_v = tex_v[iidx - 1];
                let mid_diffuse = diffuse[iidx - 1];
                for e in [TOP_EDGE, BOTTOM_EDGE] {
                    let (direction, parallel) =
                        intersect_planes(&segment[iidx - 1].edge_plane[e], &segment[iidx].edge_plane[e], &midpoint, parallel_factor);
                    let i = &mut intersection[iidx][e];
                    i.point_count = 1;
                    i.next_segment_id = iidx as u32;
                    i.point = midpoint;
                    i.tex_v = mid_tex_v;
                    i.rgba = mid_diffuse;
                    i.direction = direction;
                    i.parallel = parallel;
                }
                // Find StartPlane:
                let toward = segment[iidx].start_plane;
                segment[iidx].start_plane =
                    oriented_plane(&intersection[iidx][TOP_EDGE].direction, &intersection[iidx][BOTTOM_EDGE].direction, &toward);
            }

            // Intersection merging
            if self.is_merge_intersections() {
                self.merge_intersections(&mut intersection, &segment, &mut num_intersections, radius, parallel_factor)?;
            }

            // Find vertex positions, generate vertices and triangles
            let (vertices, triangles) = generate_strip(&intersection, &num_intersections, &pts, u_values, uv_offset)?;

            // Set color, opacity, vertex flags
            let rgba = dx8_convert_color([self.color.x, self.color.y, self.color.z, self.opacity]);
            let rgba_all = rgba == 0xFFFF_FFFF;
            // Enable sorting if sorting has not been disabled and line is translucent and alpha testing is not enabled.
            let sorting = !self.is_sorting_disabled()
                && (self.shader.get_dst_blend_func() != DSTBLEND_ZERO && self.shader.get_alpha_test() == ALPHATEST_DISABLE);
            let mut shader = self.shader;
            shader.set_cull_mode(CULL_MODE_DISABLE);
            // if there's a default color or an rgba array modulate (always: see the module docs)
            let prelit_diffuse = !rgba_all || rgba != 0;
            if prelit_diffuse {
                set_primary_gradient(&mut shader, GRADIENT_MODULATE);
            } else {
                set_primary_gradient(&mut shader, 0);
            }
            // If Texture is non-NULL enable texturing in shader - otherwise disable.
            set_texturing(&mut shader, if self.texture.is_some() { TEXTURING_ENABLE } else { TEXTURING_DISABLE });

            chunks.push(SegLineChunk { vertices, triangles, shader, prelit_diffuse, sorting, texture: self.texture.clone() });
            chidx += chunk_size - 1;
        }
        Ok(chunks)
    }

    /// `Render`'s intersection merging (`if (Is_Merge_Intersections())`),
    /// in place over the intersections of both edges.
    ///
    /// A parallel merge of a parallel intersection starts `new_direction` from
    /// the current intersection's direction, a deliberate deviation from
    /// upstream (whose value is undefined there): the owner's decision on
    /// GitHub #185 (2026-11-14, option (b)). Plant 10147 restores the skip
    /// before it (`W3dUndefined::UninitialisedRead`).
    fn merge_intersections(
        &self,
        intersection: &mut [[LineSegmentIntersection; 2]],
        segment: &[LineSegment],
        num_intersections: &mut [u32; 2],
        radius: f32,
        parallel_factor: f32,
    ) -> W3dResult<()> {
        // The merges will be repeated in multiple passes until none are performed.
        let mut merged = true;
        while merged {
            merged = false;
            for edge in [TOP_EDGE, BOTTOM_EDGE] {
                // Capture here because will change inside loop
                let num_isects = num_intersections[edge] as usize;
                let mut iidx_r = 1usize;
                let mut iidx_w = 1usize;
                while iidx_r < num_isects {
                    let cur = iidx_r;
                    let mut next = iidx_r + 1;
                    let prev = iidx_w - 1;
                    let mut next_seg = segment[intersection[next][edge].next_segment_id as usize];
                    let curr_seg = segment[intersection[cur][edge].next_segment_id as usize];
                    let prev_seg = segment[intersection[prev][edge].next_segment_id as usize];

                    // If this intersection is inside both the start plane and the segment edge plane of the
                    // segment after the next intersection, merge. NOTE - we do not merge across a fold.
                    loop {
                        let c = intersection[cur][edge];
                        let n = intersection[next][edge];
                        let forward = !n.fold
                            && Vector3::dot_product(&c.direction, &next_seg.start_plane) > 0.0f32
                            && Vector3::dot_product(&c.direction, &next_seg.edge_plane[edge]) > 0.0f32;
                        let backward = !c.fold
                            && Vector3::dot_product(&n.direction, &-curr_seg.start_plane) > 0.0f32
                            && Vector3::dot_product(&n.direction, &prev_seg.edge_plane[edge]) > 0.0f32;
                        if !(forward || (backward && !plant(10101))) {
                            break;
                        }

                        // Find mean point (weighted so all points have same weighting)
                        let new_count = c.point_count + n.point_count;
                        let oo_new_count = 1.0f32 / new_count as f32;
                        let curr_factor = oo_new_count * c.point_count as f32;
                        let next_factor = if plant(10102) { oo_new_count * n.point_count as f32 } else { oo_new_count * c.point_count as f32 };
                        let new_point = c.point * curr_factor + n.point * next_factor;
                        let new_tex_v = c.tex_v * curr_factor + n.tex_v * next_factor;
                        let new_diffuse = c.rgba * curr_factor + n.rgba * next_factor;

                        // Calculate new intersection direction by intersecting prev_seg with next_seg
                        let new_parallel;
                        let mut new_direction;
                        let vdp = Vector3::dot_product(&prev_seg.edge_plane[edge], &next_seg.edge_plane[edge]);
                        if wwmath::fabs(vdp) < parallel_factor {
                            new_direction = Vector3::cross_product(&prev_seg.edge_plane[edge], &next_seg.edge_plane[edge]);
                            new_direction.normalize();
                            if Vector3::dot_product(&new_direction, &new_point) < 0.0f32 {
                                new_direction = -new_direction;
                            }
                            new_parallel = false;
                        } else {
                            let mut pl = if vdp > 0.0f32 {
                                prev_seg.edge_plane[edge] + next_seg.edge_plane[edge]
                            } else {
                                prev_seg.edge_plane[edge] - next_seg.edge_plane[edge]
                            };
                            pl.normalize();
                            if c.parallel {
                                if plant(10147) {
                                    // the skip before #185: upstream reads new_direction before it is set
                                    return Err(W3dUndefined::UninitialisedRead.into());
                                }
                                // #185, owner's decision 2026-11-14 (option b), a deliberate deviation from upstream
                                // (whose new_direction is undefined here): new_direction starts as curr_int->Direction.
                                new_direction = c.direction;
                                new_direction = new_direction - pl * Vector3::dot_product(&pl, &new_direction);
                                new_direction.normalize();
                            } else {
                                new_direction = Vector3::cross_product(&curr_seg.edge_plane[edge], &pl);
                                new_direction.normalize();
                            }
                            new_parallel = true;
                        }

                        // Abort the merge if the merged line passes too far from the points.
                        if self.merge_abort_factor > 0.0f32 && !plant(10103) {
                            let abort_dist = radius * self.merge_abort_factor;
                            let abort_dist2 = abort_dist * abort_dist;
                            let diff_curr = c.point - new_direction * Vector3::dot_product(&c.point, &new_direction);
                            if diff_curr.length2() > abort_dist2 {
                                break;
                            }
                            let next_curr = n.point - new_direction * Vector3::dot_product(&n.point, &new_direction);
                            if next_curr.length2() > abort_dist2 {
                                break;
                            }
                        }

                        // Merge edge intersections (curr_int and next_int) into curr_int
                        merged = true;
                        let ci = &mut intersection[cur][edge];
                        ci.direction = new_direction;
                        ci.parallel = new_parallel;
                        ci.point = new_point;
                        ci.tex_v = new_tex_v;
                        ci.rgba = new_diffuse;
                        ci.point_count = new_count;
                        ci.next_segment_id = n.next_segment_id;
                        ci.fold = if plant(10105) { c.fold } else { c.fold || n.fold };

                        // Decrement number of edge intersections
                        num_intersections[edge] -= 1;

                        // Advance iidx_r to shift subsequent entries backwards in result.
                        iidx_r += 1;

                        // If we are at the end then break:
                        if iidx_r == num_isects {
                            break;
                        }

                        // Advance next_int and next_seg.
                        next = iidx_r + 1;
                        next_seg = segment[intersection[next][edge].next_segment_id as usize];
                    }

                    // Copy from "read index" to "write index" (Parallel is not copied)
                    copy_intersection(intersection, cur, iidx_w, edge);
                    iidx_r += 1;
                    iidx_w += 1;
                }

                // If iidx_r is exactly equal to num_isects, the last intersection was not merged with the
                // previous one: one last copy.
                if iidx_r == num_isects && !plant(10106) {
                    copy_intersection(intersection, iidx_r, iidx_w, edge);
                }
            }
        }
        Ok(())
    }

    /// `subdivision_util(point_cnt, xformed_pts, base_tex_v, ...,
    /// base_diffuse, subdiv_diffuse)`: the subdivided points, their V and
    /// their colours.
    fn subdivision_util(
        &self,
        xformed_pts: &[Vector3],
        base_tex_v: &[f32],
        base_diffuse: Option<&[Vector4]>,
        v3: &mut Random3,
    ) -> (Vec<Vector3>, Vec<f32>, Vec<Vector4>) {
        let point_cnt = xformed_pts.len();
        // CAUTION: freezing the random offsets will make it more readily apparent that the offsets
        // are in camera space rather than worldspace.
        let freeze_random = self.is_freeze_random();
        let mut randomize = Random3::default();
        let mut out_pts = Vec::new();
        let mut out_v = Vec::new();
        let mut out_d = Vec::new();
        let line_diffuse = Vector4::new(self.color.x, self.color.y, self.color.z, self.opacity);
        let mut stack = [SegLineSubdivision::default(); 2 * MAX_SEGLINE_SUBDIV_LEVELS as usize];

        for pidx in 0..point_cnt - 1 {
            // Subdivide the (pidx, pidx + 1) segment. Produce pidx and all subdivided points up to
            // (not including) pidx + 1.
            let mut tos: i32 = 0;
            stack[0].start_pos = xformed_pts[pidx];
            stack[0].end_pos = xformed_pts[pidx + 1];
            stack[0].start_tex_v = base_tex_v[pidx];
            stack[0].end_tex_v = base_tex_v[pidx + 1];
            if let Some(d) = base_diffuse {
                stack[0].start_diffuse = d[pidx];
                stack[0].end_diffuse = d[pidx + 1];
            } else {
                stack[0].start_diffuse = line_diffuse;
                stack[0].end_diffuse = stack[0].start_diffuse;
            }
            stack[0].rand = self.noise_amplitude;
            stack[0].level = 0;

            while tos >= 0 {
                let t = tos as usize;
                if stack[t].level == self.subdivision_level {
                    // Generate point location and texture V coordinate
                    out_pts.push(stack[t].start_pos);
                    out_v.push(stack[t].start_tex_v);
                    out_d.push(stack[t].start_diffuse);
                    // Pop
                    tos -= 1;
                } else {
                    // Recurse down: pop existing entry and push two subdivided ones.
                    let randvec = if freeze_random {
                        // the three draws are Set's arguments: taken left to right
                        let x = randomize.next() as f32 * OO_INT_MAX;
                        let y = randomize.next() as f32 * OO_INT_MAX;
                        let z = randomize.next() as f32 * OO_INT_MAX;
                        Vector3::new(x, y, z)
                    } else {
                        // Vector3SolidBoxRandomizer(Vector3(1,1,1)).Get_Vector
                        let x = v3.next() as f32 * OO_INT_MAX * 1.0f32;
                        let y = v3.next() as f32 * OO_INT_MAX * 1.0f32;
                        let z = v3.next() as f32 * OO_INT_MAX * 1.0f32;
                        if plant(10107) {
                            Vector3::new(y, x, z)
                        } else {
                            Vector3::new(x, y, z)
                        }
                    };
                    let s = stack[t];
                    let mut child = s;
                    child.start_pos = s.start_pos;
                    child.end_pos = (s.start_pos + s.end_pos) * 0.5f32 + randvec * s.rand;
                    child.start_tex_v = s.start_tex_v;
                    child.end_tex_v = (s.start_tex_v + s.end_tex_v) * 0.5f32;
                    child.start_diffuse = s.start_diffuse;
                    child.end_diffuse = (s.start_diffuse + s.end_diffuse) * 0.5f32;
                    child.rand = if plant(10108) { s.rand } else { s.rand * 0.5f32 };
                    child.level = s.level + 1;
                    stack[t + 1] = child;
                    stack[t].start_pos = child.end_pos;
                    // stack[tos].EndPos already has the right value
                    stack[t].start_tex_v = child.end_tex_v;
                    // stack[tos].EndTexV already has the right value
                    // (StartDiffuse is not moved to the midpoint: the second half keeps the first's start colour)
                    if plant(10109) {
                        stack[t].start_diffuse = child.end_diffuse;
                    }
                    stack[t].rand = child.rand;
                    stack[t].level = child.level;
                    tos += 1;
                }
            }
        }
        // Last point
        out_pts.push(xformed_pts[point_cnt - 1]);
        out_v.push(base_tex_v[point_cnt - 1]);
        match base_diffuse {
            Some(d) => out_d.push(d[point_cnt - 1]),
            None => out_d.push(line_diffuse),
        }
        (out_pts, out_v, out_d)
    }
}

/// `ShaderClass::Set_Primary_Gradient(x)`.
pub(super) fn set_primary_gradient(shader: &mut ShaderClass, x: u32) {
    shader.0 &= !MASK_PRIGRADIENT;
    shader.0 |= x << SHIFT_PRIGRADIENT;
}

/// `ShaderClass::Set_Texturing(x)`.
pub(super) fn set_texturing(shader: &mut ShaderClass, x: u32) {
    shader.0 &= !MASK_TEXTURING;
    shader.0 |= x << SHIFT_TEXTURING;
}

/// The merge's copy from the read index to the write index: every field
/// but `Parallel`.
fn copy_intersection(intersection: &mut [[LineSegmentIntersection; 2]], from: usize, to: usize, edge: usize) {
    let c = intersection[from][edge];
    let w = &mut intersection[to][edge];
    w.point_count = c.point_count;
    w.next_segment_id = c.next_segment_id;
    w.point = c.point;
    w.tex_v = c.tex_v;
    w.rgba = c.rgba;
    w.direction = c.direction;
    w.fold = c.fold;
    if plant(10110) {
        w.parallel = c.parallel;
    }
}

/// `Render`'s vertex and triangle generation: a strip step where neither
/// edge's intersection merged points, a fan step on the side that did not.
fn generate_strip(
    intersection: &[[LineSegmentIntersection; 2]],
    num_intersections: &[u32; 2],
    points: &[Vector3],
    u_values: [f32; 2],
    uv_offset: Vector2,
) -> W3dResult<(Vec<SegLineVertex>, Vec<[u16; 3]>)> {
    let point_cnt = points.len();
    let mut v_array: Vec<SegLineVertex> = Vec::with_capacity((num_intersections[0] + num_intersections[1]) as usize);
    let mut tris: Vec<[u16; 3]> = Vec::new();
    let get = |i: usize, e: usize| -> W3dResult<&LineSegmentIntersection> {
        intersection.get(i).map(|x| &x[e]).ok_or(W3dUndefined::IndexOutOfRange.into())
    };
    let point = |p: usize| -> W3dResult<Vector3> { points.get(p).copied().ok_or(W3dUndefined::IndexOutOfRange.into()) };
    // the nearest point on an intersection's direction line to a point
    let vertex = |isec: &LineSegmentIntersection, p: &Vector3, u: f32| -> SegLineVertex {
        let pos = isec.direction * Vector3::dot_product(p, &isec.direction);
        SegLineVertex {
            position: pos,
            diffuse: dx8_convert_color([isec.rgba.x, isec.rgba.y, isec.rgba.z, isec.rgba.w]),
            u: u + uv_offset.x,
            v: isec.tex_v + uv_offset.y,
        }
    };

    // "Prime the pump" with two vertices (pick nearest point on each direction line):
    v_array.push(vertex(get(1, TOP_EDGE)?, &points[0], u_values[0]));
    v_array.push(vertex(get(1, BOTTOM_EDGE)?, &points[0], u_values[1]));
    let mut vidx = 2usize;
    let mut last_top_vidx = 0usize;
    let mut last_bottom_vidx = 1usize;

    // Loop over intersections, create new vertices and triangles.
    let mut top_int_idx = 1usize;
    let mut bottom_int_idx = 1usize;
    let mut pidx = 0usize;
    let mut residual_top_points = get(1, TOP_EDGE)?.point_count;
    let mut residual_bottom_points = get(1, BOTTOM_EDGE)?.point_count;

    // Reduce both pointcounts by the same amount so the smaller one is 1 (skip points)
    let mut delta = residual_top_points.min(residual_bottom_points).wrapping_sub(1);
    residual_top_points = residual_top_points.wrapping_sub(delta);
    residual_bottom_points = residual_bottom_points.wrapping_sub(delta);
    pidx += delta as usize;

    let tri = |i: usize, j: usize, k: usize| -> [u16; 3] { [i as u16, j as u16, k as u16] };
    loop {
        if tris.len() > 2 * point_cnt + 2 {
            // past the index array upstream (MAX_SEGLINE_POLY_BUFFER_SIZE)
            return Err(W3dUndefined::IndexOutOfRange.into());
        }
        if residual_top_points == 1 && residual_bottom_points == 1 {
            // Advance both intersections, creating a tristrip segment
            tris.push(tri(last_top_vidx, last_bottom_vidx, vidx));
            if plant(10111) {
                tris.push(tri(last_bottom_vidx, vidx, vidx + 1));
            } else {
                tris.push(tri(last_bottom_vidx, vidx + 1, vidx));
            }
            last_top_vidx = vidx;
            last_bottom_vidx = vidx + 1;

            // Advance both intersections.
            top_int_idx += 1;
            bottom_int_idx += 1;
            residual_top_points = get(top_int_idx, TOP_EDGE)?.point_count;
            residual_bottom_points = get(bottom_int_idx, BOTTOM_EDGE)?.point_count;

            // Advance point index
            pidx += 1;
            let p = point(pidx)?;
            v_array.push(vertex(get(top_int_idx, TOP_EDGE)?, &p, u_values[0]));
            v_array.push(vertex(get(bottom_int_idx, BOTTOM_EDGE)?, &p, if plant(10112) { u_values[0] } else { u_values[1] }));
            vidx += 2;
        } else if residual_top_points > 1 {
            // Draw one triangle (fan segment)
            tris.push(tri(last_top_vidx, last_bottom_vidx, vidx));
            last_bottom_vidx = vidx;

            // Advance bottom intersection only
            residual_top_points -= 1;
            bottom_int_idx += 1;
            residual_bottom_points = get(bottom_int_idx, BOTTOM_EDGE)?.point_count;

            pidx += 1;
            let p = point(pidx)?;
            v_array.push(vertex(get(bottom_int_idx, BOTTOM_EDGE)?, &p, u_values[1]));
            vidx += 1;
        } else {
            // residual_bottom_points > 1: draw one triangle (fan segment)
            tris.push(tri(last_top_vidx, last_bottom_vidx, vidx));
            last_top_vidx = vidx;

            // Advance top intersection only
            residual_bottom_points = residual_bottom_points.wrapping_sub(1);
            top_int_idx += 1;
            residual_top_points = get(top_int_idx, TOP_EDGE)?.point_count;

            pidx += 1;
            let p = point(pidx)?;
            let isec = get(top_int_idx, TOP_EDGE)?;
            let mut v = vertex(isec, &p, u_values[0]);
            if plant(10113) {
                v.v = isec.tex_v;
            }
            v_array.push(v);
            vidx += 1;
        }

        // Reduce both pointcounts by the same amount so the smaller one is 1 (skip points)
        delta = residual_top_points.min(residual_bottom_points).wrapping_sub(1);
        residual_top_points = residual_top_points.wrapping_sub(delta);
        residual_bottom_points = residual_bottom_points.wrapping_sub(delta);
        pidx = pidx.wrapping_add(delta as usize);

        // Exit conditions
        if (top_int_idx >= num_intersections[TOP_EDGE] as usize && residual_top_points == 1)
            || (bottom_int_idx >= num_intersections[BOTTOM_EDGE] as usize && residual_bottom_points == 1)
        {
            break;
        }
    }
    if plant(10114) {
        v_array.reverse();
    }
    Ok((v_array, tris))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(flags: u32, level: u32, noise: f32) -> W3dEmitterLinePropertiesStruct {
        W3dEmitterLinePropertiesStruct {
            flags,
            subdivision_level: level,
            noise_amplitude: noise,
            merge_abort_factor: 1.5,
            texture_tile_factor: 1.0,
            u_per_sec: 0.0,
            v_per_sec: 0.0,
            reserved: [0; 9],
        }
    }

    /// *Predictions, written before the first run:* a straight two-point line of width 2 along X, 10 in front of an
    /// identity camera (eye space -Z ahead), gives one chunk of 4 vertices (two per point, 1 either side of the line
    /// in Y) and 2 triangles `[0,1,2]`, `[1,3,2]`; white packs to `0xFFFFFFFF`; U 0 and 1; V 0; culling off, the
    /// gradient modulating, no texturing; the additive shader sorts.
    ///
    /// *Results (2026-11-14):* first run, one miss: the vertices are 0.990099 (100 / 101) either side, not 1:
    /// each is the point nearest the line's own point on an edge line through the eye, which is pulled towards the
    /// eye (z -1000 / 101). The rest held.
    #[test]
    fn a_straight_line_is_one_quad() {
        let mut r = SegLineRenderer::new(0);
        r.width = 2.0;
        let pts = [Vector3::new(-5.0, 0.0, -10.0), Vector3::new(5.0, 0.0, -10.0)];
        let id = Matrix3D::identity();
        let chunks = r.render(0, &id, &id, &pts, None, &mut Random3::default()).unwrap();
        assert_eq!(chunks.len(), 1);
        let c = &chunks[0];
        assert_eq!(c.vertices.len(), 4);
        assert_eq!(c.triangles, vec![[0, 1, 2], [1, 3, 2]]);
        for v in &c.vertices {
            assert!((v.position.y.abs() - 100.0 / 101.0).abs() < 1e-5 && (v.position.z + 1000.0 / 101.0).abs() < 1e-4, "{:?}", c.vertices);
            assert_eq!(v.diffuse, 0xFFFF_FFFF);
            assert_eq!(v.v, 0.0);
        }
        assert_eq!((c.vertices[0].u, c.vertices[1].u), (0.0, 1.0));
        assert!(c.prelit_diffuse && c.sorting && c.texture.is_none());
        assert_eq!(c.shader.get_cull_mode(), CULL_MODE_DISABLE);
        assert_eq!(c.shader.get_primary_gradient(), GRADIENT_MODULATE);
        assert_eq!(c.shader.get_texturing(), TEXTURING_DISABLE);
    }

    /// *Predictions:* `Init` takes the flags and settings (a tile factor of 80 is kept at 50, -1 at 0; an unknown
    /// mapping mode keeps the old one); the UV scroll advances by the milliseconds and keeps its fraction; level 1 with
    /// no noise doubles the points (3 points from 2, 6 vertices); 300 points at level 0 are 3 chunks (129, 129, 44
    /// points: the last point of each chunk starts the next); a level over 7 and an empty line are undefined; one
    /// point draws nothing; `Scale` scales the width and the noise.
    ///
    /// *Results (2026-11-14):* first run, one miss of the test's: `Get_UV_Offset_Rate` of 1500 is 1500.0001
    /// (`rate * 0.001f * 1000.0f` does not round-trip); compared within 1e-3 now. The rest held.
    #[test]
    fn init_scroll_subdivision_and_chunks() {
        let mut r = SegLineRenderer::new(100);
        let mut p = props(W3D_ELINE_FREEZE_RANDOM | W3D_ELINE_END_CAPS | (W3D_ELINE_TILED_TEXTURE_MAP << 24), 1, 0.0);
        p.texture_tile_factor = 80.0;
        p.u_per_sec = 250.0;
        p.v_per_sec = 1500.0;
        r.init(&p);
        assert!(!r.is_merge_intersections() && r.is_freeze_random() && r.are_end_caps_enabled() && !r.is_sorting_disabled());
        assert_eq!(r.get_texture_mapping_mode(), TILED_TEXTURE_MAP);
        assert_eq!(r.texture_tile_factor, 50.0);
        p.flags = 7 << 24;
        p.texture_tile_factor = -1.0;
        r.init(&p);
        assert_eq!(r.get_texture_mapping_mode(), TILED_TEXTURE_MAP);
        assert_eq!(r.texture_tile_factor, 0.0);
        let rate = r.get_uv_offset_rate();
        assert!((rate.x - 250.0).abs() < 1e-3 && (rate.y - 1500.0).abs() < 1e-3, "{rate:?}");
        r.width = 1.0;
        let id = Matrix3D::identity();
        let pts = [Vector3::new(-5.0, 0.0, -10.0), Vector3::new(5.0, 1.0, -10.0)];
        let c = r.render(102, &id, &id, &pts, None, &mut Random3::default()).unwrap();
        assert!((r.current_uv_offset.x - 0.5).abs() < 1e-6, "{:?}", r.current_uv_offset);
        assert_eq!(c[0].vertices.len(), 6);
        assert_eq!(c[0].triangles.len(), 4);
        r.reset_line(200);
        assert_eq!((r.last_used_sync_time, r.current_uv_offset), (200, Vector2::new(0.0, 0.0)));

        let mut r = SegLineRenderer::new(0);
        r.width = 1.0;
        let long: Vec<Vector3> = (0..300).map(|i| Vector3::new(i as f32, (i % 3) as f32, -50.0)).collect();
        let c = r.render(0, &id, &id, &long, None, &mut Random3::default()).unwrap();
        assert_eq!(c.len(), 3);
        r.subdivision_level = 8;
        assert!(r.render(0, &id, &id, &long, None, &mut Random3::default()).is_err());
        r.subdivision_level = 0;
        assert!(r.render(0, &id, &id, &[], None, &mut Random3::default()).is_err());
        assert!(r.render(0, &id, &id, &long[..1], None, &mut Random3::default()).unwrap().is_empty());
        r.noise_amplitude = 2.0;
        r.scale(3.0);
        assert_eq!((r.width, r.noise_amplitude), (3.0, 6.0));
    }
}
