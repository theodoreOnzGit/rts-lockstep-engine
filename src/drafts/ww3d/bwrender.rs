//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/bwrender.cpp   (`BWRenderClass`)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/bwrender.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/bw_render.cpp  (`BW_Render`: the same code under the old name;
//!   a `diff` of the two files with comments removed shows only the class name and the include)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/bw_render.h
//!
//! A black-and-white software triangle rasteriser for shadow textures. `Float_To_Long` is the game's `fistp`
//! version (`wwmath::float_to_long_fistp`, round to nearest even, see that function's note). `VectorProcessorClass::
//! MulAdd` is `dest[i] = dest[i] * multiplier + add` (vp.cpp:516). `Swap` swaps two `Vector2`s.

use crate::ww::vector2::Vector2;
use crate::ww::wwmath::float_to_long_fistp as float_to_long;

/// `BW_Render` is the old name of [`BWRenderClass`] (bw_render.h).
pub type BW_Render = BWRenderClass;

/// `BWRenderClass::Buffer`: a square 8-bit pixel buffer with a 3-pixel border that is never drawn into.
#[derive(Clone, Debug)]
pub struct BWBuffer {
    pub buffer: Vec<u8>,
    scale: i32,
    minv: i32,
    maxv: i32,
}

impl BWBuffer {
    /// `Buffer::Buffer(buffer, scale)`: `minv = 3`, `maxv = scale - 3`.
    pub fn new(buffer: Vec<u8>, scale: i32) -> Self {
        BWBuffer { buffer, scale, minv: 3, maxv: scale - 3 }
    }

    pub fn scale(&self) -> i32 {
        self.scale
    }

    /// `Buffer::Set_H_Line(start_x, end_x, y)` (bwrender.cpp:46): clips to `[minv, maxv)` and clears
    /// `end_x - start_x` pixels (so a line of width 0 draws nothing and the last pixel is not covered).
    pub fn set_h_line(&mut self, mut start_x: i32, mut end_x: i32, y: i32) {
        if y < self.minv || y >= self.maxv || end_x < self.minv || start_x >= self.maxv {
            return;
        }
        if start_x < self.minv {
            start_x = self.minv;
        }
        if end_x >= self.maxv {
            end_x = self.maxv - 1;
        }
        let start = (self.scale * y + start_x) as usize;
        let w = end_x - start_x;
        if w != 0 {
            // ::memset(ptr, 0x00, w); a negative w would be a huge memset upstream: treat it as the same wild write
            // by clamping to nothing (not reachable for start_x <= end_x).
            if w > 0 {
                for b in &mut self.buffer[start..start + w as usize] {
                    *b = 0x00;
                }
            }
        }
    }

    /// `Buffer::Fill(c)`.
    pub fn fill(&mut self, c: u8) {
        let n = (self.scale * self.scale) as usize;
        for b in &mut self.buffer[..n] {
            *b = c;
        }
    }
}

/// `Cull(c1, c2, c3)` (bwrender.cpp:132): true for a non-negative signed area.
fn cull(c1: &Vector2, c2: &Vector2, c3: &Vector2) -> bool {
    let x1 = c2.x - c1.x;
    let y1 = c2.y - c1.y;
    let x2 = c3.x - c1.x;
    let y2 = c3.y - c1.y;
    let r = x1 * y2 - x2 * y1;
    if r < 0.0 {
        return false;
    }
    true
}

/// `class BWRenderClass`.
#[derive(Clone, Debug)]
pub struct BWRenderClass {
    pub pixel_buffer: BWBuffer,
    /// `Vector2 *vertices` (scaled in place by `Set_Vertex_Locations`).
    pub vertices: Vec<Vector2>,
}

impl BWRenderClass {
    /// `BWRenderClass::BWRenderClass(buffer, buffer_scale)`.
    pub fn new(buffer: Vec<u8>, buffer_scale: i32) -> Self {
        BWRenderClass { pixel_buffer: BWBuffer::new(buffer, buffer_scale), vertices: Vec::new() }
    }

    pub fn fill(&mut self, c: u8) {
        self.pixel_buffer.fill(c);
    }

    /// `Set_Vertex_Locations(vertices, count)`: takes the vertices (-1..1 space) and maps each coordinate to
    /// `x * half_scale + half_scale` (the caller's array is modified upstream).
    pub fn set_vertex_locations(&mut self, vertices: Vec<Vector2>) {
        let half_scale = self.pixel_buffer.scale() as f32 * 0.5f32;
        self.vertices = vertices;
        for v in self.vertices.iter_mut() {
            v.x = v.x * half_scale + half_scale;
            v.y = v.y * half_scale + half_scale;
        }
    }

    /// The shared corner sort and rasterisation call of both `Render_*` functions.
    fn render_corners(&mut self, idx_1: usize, idx_2: usize, idx_3: usize) {
        let mut corner_1 = self.vertices[idx_1];
        let mut corner_2 = self.vertices[idx_2];
        let mut corner_3 = self.vertices[idx_3];
        if corner_2.y < corner_1.y {
            std::mem::swap(&mut corner_1, &mut corner_2);
        }
        if corner_3.y < corner_1.y {
            std::mem::swap(&mut corner_1, &mut corner_3);
        }
        if corner_3.y < corner_2.y {
            std::mem::swap(&mut corner_2, &mut corner_3);
        }
        let yci = [float_to_long(corner_1.y), float_to_long(corner_2.y), float_to_long(corner_3.y)];
        let xcf = [corner_1.x, corner_2.x, corner_3.x];
        self.render_preprocessed_triangle(&xcf, &yci);
    }

    /// `Render_Triangle_Strip(indices, index_count)` (bwrender.cpp:147): odd triangles are culled with the
    /// opposite sense.
    pub fn render_triangle_strip(&mut self, indices: &[u32]) {
        let index_count = indices.len() as i32 - 2;
        let mut b = false;
        for n in 0..index_count.max(0) as usize {
            b = !b;
            let idx_1 = indices[n] as usize;
            let idx_2 = indices[n + 1] as usize;
            let idx_3 = indices[n + 2] as usize;
            if cull(&self.vertices[idx_1], &self.vertices[idx_2], &self.vertices[idx_3]) == b {
                continue;
            }
            self.render_corners(idx_1, idx_2, idx_3);
        }
    }

    /// `Render_Triangles(indices, index_count)` (bwrender.cpp:174).
    pub fn render_triangles(&mut self, indices: &[u32]) {
        let index_count = indices.len() / 3;
        for n in 0..index_count {
            let idx_1 = indices[n * 3] as usize;
            let idx_2 = indices[n * 3 + 1] as usize;
            let idx_3 = indices[n * 3 + 2] as usize;
            if cull(&self.vertices[idx_1], &self.vertices[idx_2], &self.vertices[idx_3]) {
                continue;
            }
            self.render_corners(idx_1, idx_2, idx_3);
        }
    }

    /// `Render_Preprocessed_Triangle(xcf, yci)` (bwrender.cpp:199): corners sorted by y; scan converts the upper
    /// then the lower half.
    fn render_preprocessed_triangle(&mut self, xcf: &[f32; 3], yci: &[i32; 3]) {
        let mut x_left = xcf[0];
        let mut x_right = x_left;
        let mut ycnt = yci[1] - yci[0];
        let mut y = yci[0];

        if ycnt != 0 {
            let mut x_step_1 = (xcf[1] - xcf[0]) / ycnt as f32;
            let mut x_step_2 = (xcf[2] - xcf[0]) / (yci[2] - y) as f32;
            if x_step_1 > x_step_2 {
                std::mem::swap(&mut x_step_1, &mut x_step_2);
            }
            while ycnt > 0 {
                self.pixel_buffer.set_h_line(float_to_long(x_left), float_to_long(x_right), y);
                x_left += x_step_1;
                x_right += x_step_2;
                ycnt -= 1;
                y += 1;
            }
        } else if xcf[0] < xcf[1] {
            x_left = xcf[0];
            x_right = xcf[1];
        } else {
            x_right = xcf[0];
            x_left = xcf[1];
        }

        ycnt = yci[2] - yci[1];
        y = yci[1];
        if ycnt != 0 {
            let one_per_ycnt = 1.0f32 / ycnt as f32;
            let x_step_1 = (xcf[2] - x_left) * one_per_ycnt;
            let x_step_2 = (xcf[2] - x_right) * one_per_ycnt;
            while ycnt > 0 {
                self.pixel_buffer.set_h_line(float_to_long(x_left), float_to_long(x_right), y);
                x_left += x_step_1;
                x_right += x_step_2;
                ycnt -= 1;
                y += 1;
            }
        }
    }
}
