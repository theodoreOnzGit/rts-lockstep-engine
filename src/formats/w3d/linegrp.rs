// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/linegrp.cpp, linegrp.h
//   (LineGroupClass: the constructor, Set_Arrays, Set_Line_Size, Get_Line_Size,
//   Set_Line_Color, Get_Line_Color, Set_Tail_Diffuse, Get_Tail_Diffuse,
//   Set_Line_Alpha, Get_Line_Alpha, Set_Line_UCoord, Get_Line_UCoord, Set_Flag,
//   Get_Flag, Set_Texture, Peek_Texture, Set_Shader, Get_Shader,
//   Set_Line_Mode, Get_Line_Mode, Render, Get_Polygon_Count; FlagsType,
//   LineModeType)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix3d.cpp
//   (Matrix3D::Get_Orthogonal_Inverse, as Render calls it: in place)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `LineGroupClass` (GitHub #65): the tetrahedra or prisms a line-group
//! emitter draws, one per particle, from its position (the start) to its tail
//! position (the end). [`super::part_buf`]'s `Render_Line_Group` feeds it
//! (`W3D_EMITTER_RENDER_MODE_LINEGRP_TETRA`, `_PRISM`).
//!
//! # As upstream does it
//!
//! - **Made** (`LineGroupClass()`): no arrays, no lines, no texture, the
//!   additive sprite shader, no flags, size 0, white, alpha 1, U 0, a black
//!   transparent tail, tetrahedra. `Set_Arrays` takes the start and end points
//!   and the optional colour, tail colour, active line table (`ALT`), size and
//!   U arrays; the line count is the active count with a table, else the count
//!   given, or every point for a negative one. An absent array's value is the
//!   default (`Set_Line_*`, `Set_Tail_Diffuse`).
//! - **`Render`**: nothing for no lines. The shader is edited (and kept so):
//!   culling on; the primary gradient modulating when there is a colour array,
//!   the default colour or alpha is not over `0.9961` (254/255) or there is no
//!   texture, else disabled; texturing on with a texture. Sorted when the
//!   destination blend is not `ZERO`, alpha testing is off and sorting is
//!   enabled. Three offsets in the view plane, `(cos a, sin a, 0)` at
//!   `pi/2`, `7 pi/6` and `11 pi/6` (`WWMath::Cos`, `Sin`); with `TRANSFORM`
//!   (world-space points, what the particle buffer sets) they are turned by
//!   the camera's rotation inverted (see the quirks), the world transform the
//!   identity and the view kept; without it the view is the identity too
//!   (camera-space points). A tetrahedron is 4 vertices (the apex at the end,
//!   the tail colour, V 1; the start plus `size` times each offset, the head
//!   colour, V 0; U the line's) and 4 triangles; a prism 6 vertices (the start
//!   cap, then the end cap at the end with the tail colour and V 1) and 8
//!   triangles. Colours packed by `DX8Wrapper::Convert_Color` (chopped, not
//!   clamped). The material is `PRELIT_DIFFUSE`. One draw.
//!
//! # Upstream quirks that are kept
//!
//! - **The camera's rotation is not inverted.** `Render` calls
//!   `xform_mat.Get_Orthogonal_Inverse(xform_mat)`: the inverse written over
//!   the matrix it reads. The transpose's first writes (`[0][1]`, `[0][2]`,
//!   `[1][2]`) overwrite the entries its later reads take (`[1][0]` reads the
//!   new `[0][1]`, ...), so the result is symmetric: the camera rotation's
//!   lower triangle mirrored into the upper one. For a camera that is not
//!   symmetric itself the offsets are not in the view plane. (The translation
//!   part is `-(R' * 0)`: a signed zero.)
//! - The edited shader stays edited: the next `Get_Shader` returns it (the
//!   particle buffer's `Determine_Tail_Diffuse` reads only blend and alpha
//!   test bits, which the edits leave alone).
//! - The index buffer's entries are `unsigned short` arithmetic (`4 * j`,
//!   `6 * j` truncated): never past 16 bits in the defined range.
//!
//! # Undefined upstream ([`W3dUndefined`])
//!
//! - A negative line count (a table with a negative active count):
//!   [`W3dUndefined::Allocation`].
//! - More than 65,535 indices or vertices (5,462 tetrahedra, 2,731 prisms): the
//!   dynamic buffers' `unsigned short` counts truncate and the writes run past
//!   them; a table entry or a line past its array's end:
//!   [`W3dUndefined::IndexOutOfRange`]. (The shader edits are made first, as
//!   upstream's are.)
//!
//! # Where this differs from upstream
//!
//! - The DirectX 8 device is replaced: the draw's vertices, triangles,
//!   shader, sorting choice and texture are returned ([`LineGroupChunk`]),
//!   with whether the view was set to the identity (no `TRANSFORM`) for the
//!   caller to place them. The material (always `PRELIT_DIFFUSE`) is not kept.
//!   The texture is its name.
//! - The share buffers are owned copies (`Set_Arrays` copies them): the
//!   caller sets them again before each render, as the particle buffer does.
//! - `WW3D::Is_Sorting_Enabled()` is an argument (the game never turns sorting
//!   off: `WW3D::IsSortingEnabled` stays `true`).

use super::mesh::dx8_convert_color;
use super::seglinerenderer::{set_primary_gradient, set_texturing};
use super::shader::{ShaderClass, DSTBLEND_ZERO, GRADIENT_DISABLE, GRADIENT_MODULATE, PRESET_ADDITIVE_SPRITE_SHADER};
use super::{W3dResult, W3dUndefined};
use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;
use crate::ww::wwmath;

/// `LineGroupClass::FlagsType::TRANSFORM`: transform the offsets by the
/// camera (world-space points).
pub const TRANSFORM: u32 = 0;

/// `ShaderClass::CULL_MODE_ENABLE`, `TEXTURING_DISABLE`, `TEXTURING_ENABLE`,
/// `ALPHATEST_DISABLE`.
const CULL_MODE_ENABLE: u32 = 1;
const TEXTURING_DISABLE: u32 = 0;
const TEXTURING_ENABLE: u32 = 1;
const ALPHATEST_DISABLE: u32 = 0;

/// `LineGroupClass::LineModeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineModeType {
    Tetrahedron,
    Prism,
}

/// One vertex as `Render` writes it (`VertexFormatXYZNDUV2`'s written
/// fields: the position, the packed ARGB colour, the first texture
/// coordinates; the normal and second coordinates are not written).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineGroupVertex {
    pub position: Vector3,
    pub diffuse: u32,
    pub u: f32,
    pub v: f32,
}

/// One `Render`'s draw: its vertices and triangles (`unsigned short`
/// indices), the shader it set, whether it went to the sorting renderer, the
/// texture (its name), and whether the view transform was the identity
/// (`TRANSFORM` off: the vertices are in camera space; else in the world).
#[derive(Clone, Debug, PartialEq)]
pub struct LineGroupChunk {
    pub vertices: Vec<LineGroupVertex>,
    pub triangles: Vec<[u16; 3]>,
    pub shader: ShaderClass,
    pub sorting: bool,
    pub texture: Option<Vec<u8>>,
    pub view_identity: bool,
}

/// `class LineGroupClass` (the share buffers held as copies: `None` for
/// null).
#[derive(Clone, Debug, PartialEq)]
pub struct LineGroup {
    /// `StartLineLoc`.
    pub start_line_loc: Option<Vec<Vector3>>,
    /// `EndLineLoc`.
    pub end_line_loc: Option<Vec<Vector3>>,
    /// `LineDiffuse`.
    pub line_diffuse: Option<Vec<Vector4>>,
    /// `TailDiffuse`.
    pub tail_diffuse: Option<Vec<Vector4>>,
    /// `ALT`.
    pub alt: Option<Vec<u32>>,
    /// `LineSize`.
    pub line_size: Option<Vec<f32>>,
    /// `LineUCoord`.
    pub line_ucoord: Option<Vec<f32>>,
    /// `LineCount`.
    pub line_count: i32,
    /// `Texture`: its name, `None` for null.
    pub texture: Option<Vec<u8>>,
    pub shader: ShaderClass,
    pub flags: u32,
    pub default_line_size: f32,
    pub default_line_color: Vector3,
    pub default_line_alpha: f32,
    pub default_line_ucoord: f32,
    pub default_tail_diffuse: Vector4,
    pub line_mode: LineModeType,
}

impl Default for LineGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// `xform_mat.Get_Orthogonal_Inverse(xform_mat)` (matrix3d.cpp), as `Render`
/// calls it: `inv` is `*this`, so every read sees the writes before it (see
/// the module's quirks).
fn get_orthogonal_inverse_in_place(m: &mut Matrix3D) {
    if plant(10157) {
        *m = m.get_orthogonal_inverse();
        return;
    }
    // Transposing the rotation submatrix
    m.row[0][0] = m.row[0][0];
    m.row[0][1] = m.row[1][0];
    m.row[0][2] = m.row[2][0];

    m.row[1][0] = m.row[0][1];
    m.row[1][1] = m.row[1][1];
    m.row[1][2] = m.row[2][1];

    m.row[2][0] = m.row[0][2];
    m.row[2][1] = m.row[1][2];
    m.row[2][2] = m.row[2][2];

    // Now, calculate translation portion of matrix: T' = -R'T
    let mut trans = m.get_translation();
    trans = m.rotate_vector(&trans);
    trans = -trans;

    m.row[0][3] = trans[0];
    m.row[1][3] = trans[1];
    m.row[2][3] = trans[2];
}

impl LineGroup {
    /// `LineGroupClass()`.
    pub fn new() -> Self {
        LineGroup {
            start_line_loc: None,
            end_line_loc: None,
            line_diffuse: None,
            tail_diffuse: None,
            alt: None,
            line_size: None,
            line_ucoord: None,
            line_count: 0,
            texture: None,
            shader: PRESET_ADDITIVE_SPRITE_SHADER,
            flags: 0,
            default_line_size: 0.0,
            default_line_color: Vector3::new(1.0, 1.0, 1.0),
            default_line_alpha: 1.0,
            default_line_ucoord: 0.0,
            default_tail_diffuse: Vector4::new(0.0, 0.0, 0.0, 0.0),
            line_mode: LineModeType::Tetrahedron,
        }
    }

    /// `Set_Arrays(startlocs, endlocs, diffuse, taildiffuse, alt, sizes,
    /// ucoords, active_line_count)` (the length asserts are compiled out in
    /// the release build: not checked).
    #[allow(clippy::too_many_arguments)]
    pub fn set_arrays(
        &mut self,
        startlocs: Vec<Vector3>,
        endlocs: Vec<Vector3>,
        diffuse: Option<Vec<Vector4>>,
        taildiffuse: Option<Vec<Vector4>>,
        alt: Option<Vec<u32>>,
        sizes: Option<Vec<f32>>,
        ucoords: Option<Vec<f32>>,
        active_line_count: i32,
    ) {
        let count = startlocs.len() as i32;
        self.start_line_loc = Some(startlocs);
        self.end_line_loc = Some(endlocs);
        self.line_diffuse = diffuse;
        self.tail_diffuse = taildiffuse;
        self.alt = alt;
        self.line_size = sizes;
        self.line_ucoord = ucoords;
        if self.alt.is_some() {
            self.line_count = active_line_count;
        } else {
            self.line_count = if active_line_count >= 0 && !plant(10171) { active_line_count } else { count };
        }
    }

    /// `Set_Line_Size(size)`.
    pub fn set_line_size(&mut self, size: f32) {
        self.default_line_size = size;
    }
    /// `Get_Line_Size()`.
    pub fn get_line_size(&self) -> f32 {
        self.default_line_size
    }
    /// `Set_Line_Color(color)`.
    pub fn set_line_color(&mut self, color: Vector3) {
        self.default_line_color = color;
    }
    /// `Get_Line_Color()`.
    pub fn get_line_color(&self) -> Vector3 {
        self.default_line_color
    }
    /// `Set_Tail_Diffuse(tdiffuse)`.
    pub fn set_tail_diffuse(&mut self, tdiffuse: Vector4) {
        self.default_tail_diffuse = tdiffuse;
    }
    /// `Get_Tail_Diffuse()`.
    pub fn get_tail_diffuse(&self) -> Vector4 {
        self.default_tail_diffuse
    }
    /// `Set_Line_Alpha(alpha)`.
    pub fn set_line_alpha(&mut self, alpha: f32) {
        self.default_line_alpha = alpha;
    }
    /// `Get_Line_Alpha()`.
    pub fn get_line_alpha(&self) -> f32 {
        self.default_line_alpha
    }
    /// `Set_Line_UCoord(ucoord)`.
    pub fn set_line_ucoord(&mut self, ucoord: f32) {
        self.default_line_ucoord = ucoord;
    }
    /// `Get_Line_UCoord()`.
    pub fn get_line_ucoord(&self) -> f32 {
        self.default_line_ucoord
    }
    /// `Set_Flag(flag, on)`.
    pub fn set_flag(&mut self, flag: u32, on: bool) {
        if on {
            self.flags |= 1 << flag;
        } else {
            self.flags &= !(1 << flag);
        }
    }
    /// `Get_Flag(flag)`.
    pub fn get_flag(&self, flag: u32) -> i32 {
        ((self.flags >> flag) & 0x1) as i32
    }
    /// `Set_Texture(texture)`: its name, `None` for null.
    pub fn set_texture(&mut self, texture: Option<Vec<u8>>) {
        self.texture = texture;
    }
    /// `Peek_Texture()`.
    pub fn peek_texture(&self) -> Option<&[u8]> {
        self.texture.as_deref()
    }
    /// `Set_Shader(shader)`.
    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.shader = shader;
    }
    /// `Get_Shader()`.
    pub fn get_shader(&self) -> ShaderClass {
        self.shader
    }
    /// `Set_Line_Mode(linemode)`.
    pub fn set_line_mode(&mut self, linemode: LineModeType) {
        self.line_mode = linemode;
    }
    /// `Get_Line_Mode()`.
    pub fn get_line_mode(&self) -> LineModeType {
        self.line_mode
    }

    /// `Get_Polygon_Count()`.
    pub fn get_polygon_count(&self) -> i32 {
        match self.line_mode {
            LineModeType::Tetrahedron => self.line_count.wrapping_mul(4),
            LineModeType::Prism => self.line_count.wrapping_mul(if plant(10174) { 6 } else { 8 }),
        }
    }

    /// `Render(rinfo)`: `camera` is `rinfo.Camera.Get_Transform()`,
    /// `sorting_enabled` `WW3D::Is_Sorting_Enabled()`. The draw, `None` for no
    /// lines (nothing is edited then).
    pub fn render(&mut self, camera: &Matrix3D, sorting_enabled: bool) -> W3dResult<Option<LineGroupChunk>> {
        // If no lines, do nothing:
        if self.line_count == 0 {
            return Ok(None);
        }

        // Shader handling
        if !plant(10172) {
            self.shader.set_cull_mode(CULL_MODE_ENABLE);
        }

        // If there is a color or alpha array enable gradient in shader - otherwise disable.
        let value_255 = 0.9961f32; //254 / 255
        let default_white_opaque = if plant(10163) {
            self.default_line_color.x >= 1.0f32
                && self.default_line_color.y >= 1.0f32
                && self.default_line_color.z >= 1.0f32
                && self.default_line_alpha >= 1.0f32
        } else {
            self.default_line_color.x > value_255
                && self.default_line_color.y > value_255
                && self.default_line_color.z > value_255
                && self.default_line_alpha > value_255
        };

        if (self.line_diffuse.is_some() && !plant(10164)) || !default_white_opaque || self.texture.is_none() {
            set_primary_gradient(&mut self.shader, GRADIENT_MODULATE);
        } else {
            set_primary_gradient(&mut self.shader, GRADIENT_DISABLE);
        }

        // If Texture is non-NULL enable texturing in shader - otherwise disable.
        if self.texture.is_some() {
            set_texturing(&mut self.shader, TEXTURING_ENABLE);
        } else {
            set_texturing(&mut self.shader, TEXTURING_DISABLE);
        }

        // (Set_Material(PRELIT_DIFFUSE), Set_Shader, Set_Texture: the chunk's)

        // Enable sorting if the primitives are translucent and alpha testing is not enabled.
        let sort = self.shader.get_dst_blend_func() != DSTBLEND_ZERO
            && (self.shader.get_alpha_test() == ALPHATEST_DISABLE || plant(10165))
            && (sorting_enabled || plant(10166));

        // the 3 offsets in view space
        let pi = wwmath::WWMATH_PI;
        let offset_a = Vector3::new(wwmath::cos(pi / 2.0f32), wwmath::sin(pi / 2.0f32), 0.0);
        let offset_b = Vector3::new(wwmath::cos(7.0f32 * pi / 6.0f32), wwmath::sin(7.0f32 * pi / 6.0f32), 0.0);
        let offset_c = Vector3::new(wwmath::cos(11.0f32 * pi / 6.0f32), wwmath::sin(11.0f32 * pi / 6.0f32), 0.0);

        let mut offset = if plant(10156) { [offset_a, offset_c, offset_b] } else { [offset_a, offset_b, offset_c] };

        // (the view matrix saved; the world transform the identity)

        // if the points are in world space, transform the offsets
        let view_identity;
        if self.get_flag(TRANSFORM) != 0 || plant(10173) {
            let mut xform_mat = *camera;
            if !plant(10158) {
                xform_mat.set_translation(&Vector3::new(0.0, 0.0, 0.0));
            }
            get_orthogonal_inverse_in_place(&mut xform_mat);
            for o in offset.iter_mut() {
                *o = Matrix3D::transform_vector(&xform_mat, o);
            }
            view_identity = self.get_flag(TRANSFORM) == 0;
        } else {
            view_identity = true;
        }

        let (num_tris, num_vertices) = match self.line_mode {
            LineModeType::Tetrahedron => (4i64 * i64::from(self.line_count), 4i64 * i64::from(self.line_count)),
            LineModeType::Prism => (8i64 * i64::from(self.line_count), 6i64 * i64::from(self.line_count)),
        };
        let num_indices = 3 * num_tris;
        if self.line_count < 0 {
            return Err(W3dUndefined::Allocation.into());
        }
        // the dynamic buffers' counts are unsigned shorts
        if num_indices > 0xFFFF || num_vertices > 0xFFFF {
            return Err(W3dUndefined::IndexOutOfRange.into());
        }

        // construct the tetrahedra in the index buffers
        // assume first vertex is the apex, followed by offset[0-3]
        let mut triangles: Vec<[u16; 3]> = Vec::with_capacity(num_tris as usize);
        let line_count = self.line_count as u32;
        match self.line_mode {
            LineModeType::Tetrahedron => {
                for j in 0..line_count {
                    let idx = (4u32.wrapping_mul(j)) as u16;
                    let at = |k: u16| idx.wrapping_add(k);
                    // apex, offset[1], offset[0]
                    if plant(10160) {
                        triangles.push([at(0), at(1), at(2)]);
                    } else {
                        triangles.push([at(0), at(2), at(1)]);
                    }
                    // apex, offset[2], offset[1]
                    triangles.push([at(0), at(3), at(2)]);
                    // apex, offset[0], offset[2]
                    triangles.push([at(0), at(1), at(3)]);
                    // offset[0-3]
                    triangles.push([at(1), at(2), at(3)]);
                }
            }
            LineModeType::Prism => {
                for j in 0..line_count {
                    let idx = (6u32.wrapping_mul(j)) as u16;
                    let at = |k: u16| idx.wrapping_add(k);
                    // starting cap 0,1,2
                    triangles.push([at(0), at(1), at(2)]);
                    // left side
                    triangles.push([at(0), at(3), at(1)]);
                    if plant(10161) {
                        triangles.push([at(1), at(4), at(3)]);
                    } else {
                        triangles.push([at(1), at(3), at(4)]);
                    }
                    // bottom side
                    triangles.push([at(1), at(4), at(5)]);
                    triangles.push([at(1), at(5), at(2)]);
                    // right side
                    triangles.push([at(0), at(2), at(5)]);
                    triangles.push([at(0), at(5), at(3)]);
                    // end cap
                    triangles.push([at(3), at(5), at(4)]);
                }
            }
        }

        // make the vertex buffers
        let mut vertices: Vec<LineGroupVertex> = Vec::with_capacity(num_vertices as usize);
        let mut size = self.default_line_size;
        let mut diffuse = Vector4::new(self.default_line_color.x, self.default_line_color.y, self.default_line_color.z, self.default_line_alpha);
        let mut ucoord = self.default_line_ucoord;
        let mut taildiffuse = self.default_tail_diffuse;
        let start_locs = self.start_line_loc.as_deref().unwrap_or(&[]);
        let end_locs = self.end_line_loc.as_deref().unwrap_or(&[]);
        let get = |a: &[f32], k: usize| a.get(k).copied().ok_or(W3dUndefined::IndexOutOfRange);
        let get4 = |a: &[Vector4], k: usize| a.get(k).copied().ok_or(W3dUndefined::IndexOutOfRange);
        for i in 0..line_count {
            let point = match &self.alt {
                Some(alt) if !plant(10167) => *alt.get(i as usize).ok_or(W3dUndefined::IndexOutOfRange)? as usize,
                _ => i as usize,
            };
            if let Some(ls) = &self.line_size {
                if !plant(10168) {
                    size = get(ls, if plant(10175) { i as usize } else { point })?;
                }
            }
            if let Some(ld) = &self.line_diffuse {
                diffuse = get4(ld, point)?;
            }
            if let Some(lu) = &self.line_ucoord {
                if !plant(10169) {
                    ucoord = get(lu, point)?;
                }
            }
            if let Some(td) = &self.tail_diffuse {
                if !plant(10170) {
                    taildiffuse = get4(td, point)?;
                }
            }

            let end = *end_locs.get(point).ok_or(W3dUndefined::IndexOutOfRange)?;
            let start = *start_locs.get(point).ok_or(W3dUndefined::IndexOutOfRange)?;

            let pack = |c: Vector4| dx8_convert_color([c.x, c.y, c.z, c.w]);
            match self.line_mode {
                LineModeType::Tetrahedron => {
                    // apex
                    vertices.push(LineGroupVertex { position: end, diffuse: pack(taildiffuse), u: ucoord, v: if plant(10159) { 0.0 } else { 1.0 } });
                    for o in &offset {
                        let loc = start + size * *o;
                        vertices.push(LineGroupVertex { position: loc, diffuse: pack(diffuse), u: ucoord, v: 0.0 });
                    }
                }
                LineModeType::Prism => {
                    // start cap
                    for o in &offset {
                        let loc = start + size * *o;
                        vertices.push(LineGroupVertex { position: loc, diffuse: pack(diffuse), u: ucoord, v: 0.0 });
                    }
                    // Do not merge loops. The vb has to be written in a specific order
                    // (This is to optimize AGP memory write)

                    // end cap
                    for o in &offset {
                        let loc = end + size * *o;
                        let c = if plant(10162) { diffuse } else { taildiffuse };
                        vertices.push(LineGroupVertex { position: loc, diffuse: pack(c), u: ucoord, v: 1.0 });
                    }
                }
            }
        }

        // (Set_Index_Buffer, Set_Vertex_Buffer; Insert_Triangles when sorting, else Draw_Triangles; the view restored)
        Ok(Some(LineGroupChunk { vertices, triangles, shader: self.shader, sorting: sort, texture: self.texture.clone(), view_identity }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// *Predictions, written before the first run (issue65e P1):* the constructor's state is linegrp.cpp's; a line
    /// group with no lines draws nothing and leaves the shader alone; `Set_Arrays` without a table takes the count given
    /// (every point for -1), with one the active count; `Set_Flag` and `Get_Flag` round trip; the polygon count is 4 or
    /// 8 per line.
    ///
    /// *Results (2026-11-14), first run:* held.
    #[test]
    fn made_set_and_counted_as_upstream() {
        let mut g = LineGroup::new();
        assert_eq!(
            (g.line_count, g.shader, g.flags, g.default_line_size, g.default_line_color, g.default_line_alpha, g.default_line_ucoord),
            (0, PRESET_ADDITIVE_SPRITE_SHADER, 0, 0.0, Vector3::new(1.0, 1.0, 1.0), 1.0, 0.0)
        );
        assert_eq!((g.default_tail_diffuse, g.line_mode, g.peek_texture()), (Vector4::new(0.0, 0.0, 0.0, 0.0), LineModeType::Tetrahedron, None));
        assert_eq!(g.render(&Matrix3D::IDENTITY, true).unwrap(), None);
        assert_eq!(g.shader, PRESET_ADDITIVE_SPRITE_SHADER);
        let pts = vec![Vector3::default(); 5];
        g.set_arrays(pts.clone(), pts.clone(), None, None, None, None, None, -1);
        assert_eq!(g.line_count, 5);
        g.set_arrays(pts.clone(), pts.clone(), None, None, None, None, None, 3);
        assert_eq!(g.line_count, 3);
        g.set_arrays(pts.clone(), pts.clone(), None, None, Some(vec![4, 0, 0, 0, 0]), None, None, 1);
        assert_eq!(g.line_count, 1);
        g.set_flag(TRANSFORM, true);
        assert_eq!((g.get_flag(TRANSFORM), g.flags), (1, 1));
        g.set_flag(TRANSFORM, false);
        assert_eq!(g.get_flag(TRANSFORM), 0);
        assert_eq!(g.get_polygon_count(), 4);
        g.set_line_mode(LineModeType::Prism);
        assert_eq!((g.get_line_mode(), g.get_polygon_count()), (LineModeType::Prism, 8));
        g.set_line_size(2.0);
        g.set_line_color(Vector3::new(0.5, 0.25, 0.0));
        g.set_line_alpha(0.5);
        g.set_line_ucoord(0.75);
        g.set_tail_diffuse(Vector4::new(1.0, 0.0, 0.0, 1.0));
        g.set_texture(Some(b"t.tga".to_vec()));
        g.set_shader(ShaderClass(7));
        assert_eq!((g.get_line_size(), g.get_line_color(), g.get_line_alpha(), g.get_line_ucoord()), (2.0, Vector3::new(0.5, 0.25, 0.0), 0.5, 0.75));
        assert_eq!((g.get_tail_diffuse(), g.peek_texture(), g.get_shader()), (Vector4::new(1.0, 0.0, 0.0, 1.0), Some(&b"t.tga"[..]), ShaderClass(7)));
    }

    /// *Predictions, written before the first run (issue65e P1):* one camera-space tetrahedron (no `TRANSFORM`, the
    /// identity camera), size 2, from (0, 0, 0) to (0, 0, 10): 4 vertices (the apex (0, 0, 10) with the tail colour and
    /// V 1; the base at 2 times (cos, sin) of 90, 210 and 330 degrees, V 0) and 4 triangles (0 2 1, 0 3 2, 0 1 3,
    /// 1 2 3); the view the identity; the shader culling, modulating (no texture), untextured; sorted (additive).
    ///
    /// *Results (2026-11-14), first run:* held.
    #[test]
    fn a_tetrahedron_in_camera_space() {
        let mut g = LineGroup::new();
        g.set_line_size(2.0);
        g.set_tail_diffuse(Vector4::new(1.0, 0.0, 0.0, 1.0));
        g.set_arrays(vec![Vector3::new(0.0, 0.0, 0.0)], vec![Vector3::new(0.0, 0.0, 10.0)], None, None, None, None, None, -1);
        let c = g.render(&Matrix3D::IDENTITY, true).unwrap().expect("a draw");
        assert!(c.view_identity && c.sorting && c.texture.is_none());
        assert_eq!(c.triangles, vec![[0, 2, 1], [0, 3, 2], [0, 1, 3], [1, 2, 3]]);
        assert_eq!(c.vertices.len(), 4);
        assert_eq!((c.vertices[0].position, c.vertices[0].diffuse, c.vertices[0].v), (Vector3::new(0.0, 0.0, 10.0), 0xFFFF_0000, 1.0));
        let p = wwmath::WWMATH_PI;
        for (k, a) in [p / 2.0, 7.0 * p / 6.0, 11.0 * p / 6.0].into_iter().enumerate() {
            let v = c.vertices[k + 1];
            assert_eq!(v.position, Vector3::new(0.0, 0.0, 0.0) + 2.0 * Vector3::new(wwmath::cos(a), wwmath::sin(a), 0.0));
            assert_eq!((v.diffuse, v.v, v.u), (0xFFFF_FFFF, 0.0, 0.0));
        }
        assert_eq!((c.shader.get_cull_mode(), c.shader.get_primary_gradient(), c.shader.get_texturing()), (1, GRADIENT_MODULATE, 0));
        assert_eq!(g.shader, c.shader);
    }

    /// *Predictions, written before the first run (issue65e P1):* with `TRANSFORM` and a camera turned 90 degrees about
    /// z (not symmetric), the offsets are turned by the in-place "inverse": the symmetric matrix of the camera's lower
    /// triangle, not its transpose (upstream's quirk); a prism gives 6 vertices (the start cap V 0, the end cap V 1 with
    /// the tail colour) and 8 triangles; the camera's translation does not move them.
    ///
    /// *Results (2026-11-14):* the first run failed on the test's own check: it compared the first offset (90 degrees,
    /// x about 0), which the symmetric matrix and the true inverse turn alike; it now checks the second (210 degrees).
    /// Then held.
    #[test]
    fn world_space_offsets_turn_by_the_in_place_inverse() {
        let mut cam = Matrix3D::new(0.0, -1.0, 0.0, 50.0, 1.0, 0.0, 0.0, 60.0, 0.0, 0.0, 1.0, 70.0);
        let mut sym = cam;
        sym.set_translation(&Vector3::new(0.0, 0.0, 0.0));
        // the lower triangle mirrored: [0][1] = [1][0] = 1
        sym.row[0][1] = sym.row[1][0];
        let mut g = LineGroup::new();
        g.set_flag(TRANSFORM, true);
        g.set_line_mode(LineModeType::Prism);
        g.set_line_size(1.0);
        g.set_arrays(vec![Vector3::new(1.0, 2.0, 3.0)], vec![Vector3::new(4.0, 5.0, 6.0)], None, Some(vec![Vector4::new(0.0, 1.0, 0.0, 1.0)]), None, None, None, 1);
        let c = g.render(&cam, true).unwrap().expect("a draw");
        assert!(!c.view_identity);
        assert_eq!((c.vertices.len(), c.triangles.len()), (6, 8));
        let p = wwmath::WWMATH_PI;
        // (the second offset, at 210 degrees: the first's x is about 0, so the entry the quirk changes, [1][0], barely
        // touches it; checking the first was this test's own mistake on its first run)
        let o = Vector3::new(wwmath::cos(7.0 * p / 6.0), wwmath::sin(7.0 * p / 6.0), 0.0);
        let turned = Matrix3D::rotate_vector_by(&sym, &o);
        let v1 = c.vertices[1].position - Vector3::new(1.0, 2.0, 3.0);
        assert!((v1 - turned).length() < 1e-6, "{v1:?} {turned:?}");
        assert!((v1 - Matrix3D::rotate_vector_by(&cam.get_orthogonal_inverse(), &o)).length() > 0.5, "not the true inverse");
        assert_eq!((c.vertices[3].v, c.vertices[3].diffuse, c.vertices[0].v), (1.0, 0xFF00_FF00, 0.0));
        assert_eq!(c.triangles[7], [3, 5, 4]);
        cam.set_translation(&Vector3::new(0.0, 0.0, 0.0));
        assert_eq!(g.render(&cam, true).unwrap().unwrap().vertices, c.vertices);
    }

    /// *Predictions, written before the first run (issue65e P1):* undefined upstream: a table with a negative active
    /// count (`Allocation`), 5,462 tetrahedra (65,544 indices: `IndexOutOfRange`) while 5,461 draw, a table entry past
    /// the arrays (`IndexOutOfRange`); a textured opaque white group without a colour array disables the gradient, and
    /// an alpha-tested one is not sorted, nor one drawn with sorting disabled.
    ///
    /// *Results (2026-11-14), first run:* held.
    #[test]
    fn undefined_cases_and_the_shader_choices() {
        let pts = |n: usize| vec![Vector3::default(); n];
        let mut g = LineGroup::new();
        g.set_arrays(pts(2), pts(2), None, None, Some(vec![0, 1]), None, None, -1);
        assert_eq!(g.render(&Matrix3D::IDENTITY, true), Err(W3dUndefined::Allocation.into()));
        g.set_arrays(pts(5462), pts(5462), None, None, None, None, None, -1);
        assert_eq!(g.render(&Matrix3D::IDENTITY, true), Err(W3dUndefined::IndexOutOfRange.into()));
        g.set_arrays(pts(5461), pts(5461), None, None, None, None, None, -1);
        assert_eq!(g.render(&Matrix3D::IDENTITY, true).unwrap().unwrap().vertices.len(), 4 * 5461);
        g.set_arrays(pts(2), pts(2), None, None, Some(vec![0, 2]), None, None, 2);
        assert_eq!(g.render(&Matrix3D::IDENTITY, true), Err(W3dUndefined::IndexOutOfRange.into()));
        g.set_arrays(pts(1), pts(1), None, None, None, None, None, -1);
        g.set_texture(Some(b"t".to_vec()));
        let c = g.render(&Matrix3D::IDENTITY, true).unwrap().unwrap();
        assert_eq!((c.shader.get_primary_gradient(), c.shader.get_texturing()), (GRADIENT_DISABLE, 1));
        g.set_line_alpha(0.99);
        assert_eq!(g.render(&Matrix3D::IDENTITY, true).unwrap().unwrap().shader.get_primary_gradient(), GRADIENT_MODULATE);
        assert!(!g.render(&Matrix3D::IDENTITY, false).unwrap().unwrap().sorting);
        g.set_shader(super::super::shader::PRESET_ALPHA_SPRITE_SHADER);
        assert!(g.render(&Matrix3D::IDENTITY, true).unwrap().unwrap().sorting);
        g.shader.0 |= 1 << super::super::mesh::shader_bits::SHIFT_ALPHATEST;
        assert!(!g.render(&Matrix3D::IDENTITY, true).unwrap().unwrap().sorting);
    }
}
