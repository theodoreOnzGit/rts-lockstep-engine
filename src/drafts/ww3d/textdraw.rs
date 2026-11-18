//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/textdraw.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/textdraw.h
//!
//! `TextDrawClass : DynamicMeshClass` becomes a struct that owns a draft [`DynamicMeshClass`] (`mesh`). The default
//! vertex material (`Diffuse 0, Opacity 1, Emissive 1`) is made by the owner and passed in as a handle; the font's
//! texture handle for a character (`Font3DInstanceClass::Peek_Texture(ch)`) is passed in at the print call.

use super::dynamesh::DynamicMeshClass;
use super::font3d::Font3DInstanceClass;
use super::handles::{TextureHandle, VertexMaterialHandle};
use super::rect::RectClass;
use crate::formats::w3d::mesh::shader_bits::*;
use crate::formats::w3d::shader::{ShaderClass, DSTBLEND_ONE_MINUS_SRC_ALPHA, SRCBLEND_SRC_ALPHA};
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

const PASS_ALWAYS: u32 = 7;

/// `class TextDrawClass` (textdraw.h).
#[derive(Clone, Debug)]
pub struct TextDrawClass {
    pub mesh: DynamicMeshClass,
    pub text_color: Vector3,
    default_vertex_material: VertexMaterialHandle,
    default_shader: ShaderClass,
    translate_scale: Vector2,
    translate_offset: Vector2,
    pixel_size: Vector2,
    /// `WW3D::Is_Screen_UV_Biased()`, read at each `Print(char)`.
    pub screen_uv_biased: bool,
}

impl TextDrawClass {
    /// `TextDrawClass::TextDrawClass(max_chars)` (textdraw.cpp:30).
    pub fn new(max_chars: usize, default_vertex_material: VertexMaterialHandle) -> Self {
        let mut shader = ShaderClass(0);
        let mut set = |mask: u32, shift: u32, v: u32| shader.0 = (shader.0 & !mask) | (v << shift);
        set(MASK_DEPTHMASK, SHIFT_DEPTHMASK, 0); // DEPTH_WRITE_DISABLE
        set(MASK_DEPTHCOMPARE, SHIFT_DEPTHCOMPARE, PASS_ALWAYS);
        set(MASK_DSTBLEND, SHIFT_DSTBLEND, DSTBLEND_ONE_MINUS_SRC_ALPHA);
        set(MASK_SRCBLEND, SHIFT_SRCBLEND, SRCBLEND_SRC_ALPHA);
        set(MASK_TEXTURING, SHIFT_TEXTURING, 1); // TEXTURING_ENABLE
        set(MASK_CULLMODE, SHIFT_CULLMODE, 0); // CULL_MODE_DISABLE

        let mut mesh = DynamicMeshClass::new(max_chars * 2, max_chars * 4, None);
        mesh.set_vertex_material(default_vertex_material, false, 0);
        mesh.set_shader(shader, 0);
        mesh.enable_sort();
        let mut t = TextDrawClass {
            mesh,
            text_color: Vector3::new(1.0, 1.0, 1.0),
            default_vertex_material,
            default_shader: shader,
            translate_scale: Vector2::new(1.0, 1.0),
            translate_offset: Vector2::new(0.0, 0.0),
            pixel_size: Vector2::new(0.0, 0.0),
            screen_uv_biased: false,
        };
        t.set_coordinate_ranges(
            &Vector2::new(0.0, 0.0),
            &Vector2::new(1.0, 1.0),
            &Vector2::new(-1.0, 0.75),
            &Vector2::new(1.0, -0.75),
        );
        t
    }

    /// `TextDrawClass::Reset` (textdraw.cpp:56).
    pub fn reset(&mut self) {
        self.mesh.reset_flags();
        self.mesh.reset_mesh_counters();
        self.mesh.enable_sort();
        self.mesh.set_vertex_material(self.default_vertex_material, false, 0);
        self.mesh.set_shader(self.default_shader, 0);
    }

    /// `Set_Coordinate_Ranges(src_ul, src_lr, dest_ul, dest_lr)` (textdraw.cpp:70).
    pub fn set_coordinate_ranges(&mut self, src_ul: &Vector2, src_lr: &Vector2, dest_ul: &Vector2, dest_lr: &Vector2) {
        self.translate_scale.x = (dest_lr.x - dest_ul.x) / (src_lr.x - src_ul.x);
        self.translate_scale.y = (dest_lr.y - dest_ul.y) / (src_lr.y - src_ul.y);
        self.translate_offset.x = dest_ul.x - self.translate_scale.x * src_ul.x;
        self.translate_offset.y = dest_ul.y - self.translate_scale.y * src_ul.y;
        // fabs of a double expression narrowed on assignment
        self.pixel_size.x = ((src_lr.x - src_ul.x) / 640.0f32).abs();
        self.pixel_size.y = ((src_lr.y - src_ul.y) / 480.0f32).abs();
    }

    /// `Quad(x0, y0, x1, y1, u0, v0, u1, v1)` (textdraw.cpp:86): a tri strip whose second/third vertex order
    /// depends on the facing after the coordinate remap.
    pub fn quad(&mut self, mut x0: f32, mut y0: f32, mut x1: f32, mut y1: f32, u0: f32, v0: f32, u1: f32, v1: f32) {
        x0 = x0 * self.translate_scale.x + self.translate_offset.x;
        x1 = x1 * self.translate_scale.x + self.translate_offset.x;
        y0 = y0 * self.translate_scale.y + self.translate_offset.y;
        y1 = y1 * self.translate_scale.y + self.translate_offset.y;
        let flip_faces = ((x1 - x0) * (y1 - y0)) > 0.0;
        self.mesh.begin_tri_strip();
        self.mesh.vertex(x0, y0, 0.0, u0, v0);
        if flip_faces {
            self.mesh.vertex(x1, y0, 0.0, u1, v0);
            self.mesh.vertex(x0, y1, 0.0, u0, v1);
        } else {
            self.mesh.vertex(x0, y1, 0.0, u0, v1);
            self.mesh.vertex(x1, y0, 0.0, u1, v0);
        }
        self.mesh.vertex(x1, y1, 0.0, u1, v1);
        self.mesh.end_tri_strip();
    }

    /// `Quad(rect, uv)`.
    pub fn quad_rect(&mut self, rect: &RectClass, uv: &RectClass) {
        self.quad(rect.left, rect.top, rect.right, rect.bottom, uv.left, uv.top, uv.right, uv.bottom);
    }

    /// `Line(a, b, width)` (textdraw.cpp:113): a perpendicular-offset tri strip.
    pub fn line(&mut self, a_in: &Vector2, b_in: &Vector2, mut width: f32) {
        let a = Vector2::new(a_in.x * self.translate_scale.x + self.translate_offset.x, a_in.y * self.translate_scale.y + self.translate_offset.y);
        let b = Vector2::new(b_in.x * self.translate_scale.x + self.translate_offset.x, b_in.y * self.translate_scale.y + self.translate_offset.y);
        width *= wwmath::fabs(self.translate_scale.x);

        let mut corner_offset = Vector2::new(a.x - b.x, a.y - b.y); // line relative to b
        let temp = corner_offset.x; // Rotate 90
        corner_offset.x = corner_offset.y;
        corner_offset.y = -temp;
        corner_offset.normalize(); // scale to length width/2
        corner_offset.x *= width / 2.0;
        corner_offset.y *= width / 2.0;

        self.mesh.begin_tri_strip();
        self.mesh.vertex_v2(Vector2::new(a.x + corner_offset.x, a.y + corner_offset.y));
        self.mesh.vertex_v2(Vector2::new(a.x - corner_offset.x, a.y - corner_offset.y));
        self.mesh.vertex_v2(Vector2::new(b.x + corner_offset.x, b.y + corner_offset.y));
        self.mesh.vertex_v2(Vector2::new(b.x - corner_offset.x, b.y - corner_offset.y));
        self.mesh.end_tri_strip();
    }

    /// `Line_Ends(a, b, width, end_percent)` (textdraw.cpp:141): a short line at each end.
    pub fn line_ends(&mut self, a: &Vector2, b: &Vector2, width: f32, end_percent: f32) {
        let mut a_ = Vector2::new(b.x - a.x, b.y - a.y);
        a_.x *= end_percent;
        a_.y *= end_percent;
        a_.x += a.x;
        a_.y += a.y;
        self.line(a, &a_, width);

        let mut b_ = Vector2::new(a.x - b.x, a.y - b.y);
        b_.x *= end_percent;
        b_.y *= end_percent;
        b_.x += b.x;
        b_.y += b.y;
        self.line(b, &b_, width);
    }

    /// `Get_Width(font, message)`: sum of `Char_Spacing`.
    pub fn get_width(font: &Font3DInstanceClass, message: &[u8]) -> f32 {
        let mut total_width = 0.0f32;
        for &c in message {
            if c == 0 {
                break;
            }
            total_width += font.char_spacing(c as u16);
        }
        total_width
    }

    /// `Get_Inter_Char_Width(font)`: 1.
    pub fn get_inter_char_width(_font: &Font3DInstanceClass) -> f32 {
        1.0
    }

    /// `Get_Height(font, message)`: `Char_Height`.
    pub fn get_height(font: &Font3DInstanceClass, _message: &[u8]) -> f32 {
        font.char_height()
    }

    /// `Print(font, ch, screen_x, screen_y)` (textdraw.cpp:187): pixel-snapped glyph quad; returns the spacing.
    /// `font_texture` is `font->Peek_Texture(ch)`.
    pub fn print_char(&mut self, font: &Font3DInstanceClass, ch: u8, screen_x: f32, screen_y: f32, font_texture: TextureHandle) -> f32 {
        let width = font.char_width(ch as u16);
        let spacing = font.char_spacing(ch as u16);
        // If asked to draw the space char (' '), don't add any triangles, just return the scaled spacing
        if ch == b' ' {
            return spacing;
        }
        let mut screen_x0 = screen_x + spacing / 2.0 - width / 2.0;
        screen_x0 = (screen_x0 / self.pixel_size.x + 0.5f32).floor() * self.pixel_size.x;
        let mut screen_x1 = screen_x0 + width;
        screen_x1 = (screen_x1 / self.pixel_size.x + 0.5f32).floor() * self.pixel_size.x;
        let mut screen_y0 = screen_y;
        screen_y0 = (screen_y0 / self.pixel_size.y + 0.5f32).floor() * self.pixel_size.y;
        let mut screen_y1 = screen_y0 + font.char_height() * wwmath::sign(-self.translate_scale.y);
        screen_y1 = (screen_y1 / self.pixel_size.y + 0.5f32).floor() * self.pixel_size.y;

        if self.screen_uv_biased {
            // Global bias setting
            screen_x0 += self.pixel_size.x / 2.0;
            screen_x1 += self.pixel_size.x / 2.0;
            screen_y0 += self.pixel_size.y / 2.0;
            screen_y1 += self.pixel_size.y / 2.0;
        }

        let font_uv = font.char_uv(ch as u16);
        self.mesh.set_texture(font_texture, false, 0);
        self.quad(screen_x0, screen_y0, screen_x1, screen_y1, font_uv.left, font_uv.top, font_uv.right, font_uv.bottom);
        spacing
    }

    /// `Print(font, message, screen_x, screen_y)`: returns the total drawn width. `texture_of(ch)` is
    /// `font->Peek_Texture(ch)`.
    pub fn print<F: Fn(u8) -> TextureHandle>(&mut self, font: &Font3DInstanceClass, message: &[u8], mut screen_x: f32, screen_y: f32, texture_of: F) -> f32 {
        let mut total_width = 0.0f32;
        for &c in message {
            if c == 0 {
                break;
            }
            let width = self.print_char(font, c, screen_x, screen_y, texture_of(c));
            screen_x += width;
            total_width += width;
        }
        total_width
    }

    /// `Show_Font(font, screen_x, screen_y)`: draws the whole 256x256 font page.
    pub fn show_font(&mut self, screen_x: f32, screen_y: f32, font_texture_a: TextureHandle) {
        let size_x = self.pixel_size.x * 256.0;
        let size_y = self.pixel_size.y * 256.0;
        self.mesh.set_texture(font_texture_a, false, 0);
        self.quad(screen_x, screen_y, screen_x + size_x, screen_y + size_y * wwmath::sign(-self.translate_scale.y), 0.0, 0.0, 1.0, 1.0);
    }
}
