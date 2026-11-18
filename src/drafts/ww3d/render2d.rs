//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/render2d.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/render2d.h
//!
//! `Render2DClass` accumulates screen-space quads/triangles/lines; `Render` submits them. The geometry
//! accumulation is translated exactly; `Render` returns a [`Render2DDraw`] (positions with `ZValue`, colours, UVs,
//! indices, the shader and texture) instead of filling DX8 buffers, and the grayscale special case
//! (`D3DRS_TEXTUREFACTOR`, `D3DTSS_*` states) is device-only and recorded as a flag. The `SimpleDynVecClass`
//! pre-allocated arrays are `Vec`s. `Render2DTextClass` uses the draft [`Font3DInstanceClass`]; strings are
//! `&[u16]` (`WCHAR`), and the `char *` overload widens each byte (`WideStringClass::Convert_From`).
//! `Set_Texture(const char *filename)` goes through the asset manager and is the owner's business.

use super::font3d::Font3DInstanceClass;
use super::handles::TextureHandle;
use super::rect::RectClass;
use crate::formats::w3d::mesh::shader_bits::*;
use crate::formats::w3d::shader::{
    ShaderClass, DSTBLEND_ONE, DSTBLEND_ONE_MINUS_SRC_ALPHA, DSTBLEND_ZERO, FOG_DISABLE, GRADIENT_MODULATE, PASS_LEQUAL,
    SRCBLEND_ONE, SRCBLEND_SRC_ALPHA,
};
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

const PASS_ALWAYS: u32 = 7;

/// `ShaderClass::Set_*` over the bit fields (the port's `ShaderClass` has only a few setters).
fn set_field(shader: &mut ShaderClass, mask: u32, shift: u32, value: u32) {
    shader.0 = (shader.0 & !mask) | (value << shift);
}

/// The data one `Render2DClass::Render` call submits.
#[derive(Clone, Debug, PartialEq)]
pub struct Render2DDraw {
    /// Vertex positions as written: `(x, y, ZValue)`.
    pub positions: Vec<Vector3>,
    pub colors: Vec<u32>,
    pub uvs: Vec<Vector2>,
    pub indices: Vec<u16>,
    pub shader: ShaderClass,
    pub texture: Option<TextureHandle>,
    /// `IsGrayScale`: the opaque shader plus the `TEXTUREFACTOR` stage setup (device-only) was used instead of
    /// `shader`.
    pub grayscale: bool,
    /// `Draw_Triangles(0, Indices.Count()/3, 0, Vertices.Count())`.
    pub triangle_count: u32,
}

/// `class Render2DClass` (render2d.h).
#[derive(Clone, Debug)]
pub struct Render2DClass {
    pub coordinate_scale: Vector2,
    pub coordinate_offset: Vector2,
    pub biased_coordinate_offset: Vector2,
    pub texture: Option<TextureHandle>,
    pub shader: ShaderClass,
    pub z_value: f32,
    pub is_hidden: bool,
    pub is_gray_scale: bool,
    pub indices: Vec<u16>,
    pub vertices: Vec<Vector2>,
    pub uv_coordinates: Vec<Vector2>,
    pub colors: Vec<u32>,
    /// `WW3D::Is_Screen_UV_Biased()` as seen by `Update_Bias`, and `Render2DClass::ScreenResolution` (a class
    /// static upstream): supplied by the owner on each `reset`/`set_coordinate_range`.
    pub screen_uv_biased: bool,
    pub screen_resolution: RectClass,
}

impl Render2DClass {
    /// `Render2DClass::Get_Default_Shader` (render2d.cpp:96): the default `ShaderClass()` (bits 0) with the listed
    /// setters.
    pub fn get_default_shader() -> ShaderClass {
        let mut shader = ShaderClass(0);
        set_field(&mut shader, MASK_DEPTHMASK, SHIFT_DEPTHMASK, 0); // DEPTH_WRITE_DISABLE
        set_field(&mut shader, MASK_DEPTHCOMPARE, SHIFT_DEPTHCOMPARE, PASS_ALWAYS);
        set_field(&mut shader, MASK_DSTBLEND, SHIFT_DSTBLEND, DSTBLEND_ONE_MINUS_SRC_ALPHA);
        set_field(&mut shader, MASK_SRCBLEND, SHIFT_SRCBLEND, SRCBLEND_SRC_ALPHA);
        set_field(&mut shader, MASK_FOG, SHIFT_FOG, FOG_DISABLE);
        set_field(&mut shader, MASK_PRIGRADIENT, SHIFT_PRIGRADIENT, GRADIENT_MODULATE);
        set_field(&mut shader, MASK_TEXTURING, SHIFT_TEXTURING, 1); // TEXTURING_ENABLE
        shader
    }

    /// `Render2DClass::Render2DClass(tex)` (render2d.cpp:55).
    pub fn new(tex: Option<TextureHandle>, screen_resolution: RectClass, screen_uv_biased: bool) -> Self {
        Render2DClass {
            coordinate_scale: Vector2::new(1.0, 1.0),
            coordinate_offset: Vector2::new(0.0, 0.0),
            biased_coordinate_offset: Vector2::new(0.0, 0.0),
            texture: tex,
            shader: Self::get_default_shader(),
            z_value: 0.0,
            is_hidden: false,
            is_gray_scale: false,
            indices: Vec::new(),
            vertices: Vec::new(),
            uv_coordinates: Vec::new(),
            colors: Vec::new(),
            screen_uv_biased,
            screen_resolution,
        }
    }

    /// `Render2DClass::Set_Screen_Resolution` (static upstream).
    pub fn set_screen_resolution(&mut self, screen: RectClass) {
        self.screen_resolution = screen;
    }

    pub fn get_screen_resolution(&self) -> RectClass {
        self.screen_resolution
    }

    /// `Render2DClass::Reset` (render2d.cpp:123): drops the geometry and refreshes the bias.
    pub fn reset(&mut self) {
        self.vertices.clear();
        self.uv_coordinates.clear();
        self.colors.clear();
        self.indices.clear();
        self.update_bias(); // Keep the bias updated
    }

    pub fn set_texture(&mut self, tex: Option<TextureHandle>) {
        self.texture = tex;
    }

    /// `Enable_Grayscale` (added for Generals to draw disabled button states).
    pub fn enable_grayscale(&mut self, b: bool) {
        self.is_gray_scale = b;
    }

    /// `Enable_Alpha`.
    pub fn enable_alpha(&mut self, b: bool) {
        self.is_gray_scale = false;
        if b {
            set_field(&mut self.shader, MASK_DSTBLEND, SHIFT_DSTBLEND, DSTBLEND_ONE_MINUS_SRC_ALPHA);
            set_field(&mut self.shader, MASK_SRCBLEND, SHIFT_SRCBLEND, SRCBLEND_SRC_ALPHA);
        } else {
            set_field(&mut self.shader, MASK_SRCBLEND, SHIFT_SRCBLEND, SRCBLEND_ONE);
            set_field(&mut self.shader, MASK_DSTBLEND, SHIFT_DSTBLEND, DSTBLEND_ZERO);
        }
    }

    /// `Enable_Additive`.
    pub fn enable_additive(&mut self, b: bool) {
        self.is_gray_scale = false;
        if b {
            set_field(&mut self.shader, MASK_DSTBLEND, SHIFT_DSTBLEND, DSTBLEND_ONE);
            set_field(&mut self.shader, MASK_SRCBLEND, SHIFT_SRCBLEND, SRCBLEND_ONE);
        } else {
            set_field(&mut self.shader, MASK_SRCBLEND, SHIFT_SRCBLEND, SRCBLEND_ONE);
            set_field(&mut self.shader, MASK_DSTBLEND, SHIFT_DSTBLEND, DSTBLEND_ZERO);
        }
    }

    /// `Enable_Texturing`.
    pub fn enable_texturing(&mut self, b: bool) {
        set_field(&mut self.shader, MASK_TEXTURING, SHIFT_TEXTURING, if b { 1 } else { 0 });
    }

    /// `Set_Coordinate_Range` (render2d.cpp:176).
    pub fn set_coordinate_range(&mut self, range: &RectClass) {
        self.coordinate_scale.x = 2.0 / range.width();
        self.coordinate_scale.y = -2.0 / range.height();
        self.coordinate_offset.x = -(self.coordinate_scale.x * range.left) - 1.0;
        self.coordinate_offset.y = -(self.coordinate_scale.y * range.top) + 1.0;
        self.update_bias();
    }

    /// `Update_Bias` (render2d.cpp:185): the global half-pixel bias, in screen units.
    pub fn update_bias(&mut self) {
        self.biased_coordinate_offset = self.coordinate_offset;
        if self.screen_uv_biased {
            // offset by -0.5,-0.5 in pixels
            let mut bais_add = Vector2::new(-0.5, -0.5);
            bais_add.x /= self.get_screen_resolution().width() * 0.5f32;
            bais_add.y /= self.get_screen_resolution().height() * -0.5f32;
            self.biased_coordinate_offset.x += bais_add.x;
            self.biased_coordinate_offset.y += bais_add.y;
        }
    }

    /// `Convert_Vert(vert_out, vert_in)` (render2d.cpp:230, the `#else` branch).
    fn convert_vert(&self, vert_in: &Vector2) -> Vector2 {
        Vector2::new(
            vert_in.x * self.coordinate_scale.x + self.biased_coordinate_offset.x,
            vert_in.y * self.coordinate_scale.y + self.biased_coordinate_offset.y,
        )
    }

    fn convert_vert_xy(&self, x_in: f32, y_in: f32) -> Vector2 {
        Vector2::new(
            x_in * self.coordinate_scale.x + self.biased_coordinate_offset.x,
            y_in * self.coordinate_scale.y + self.biased_coordinate_offset.y,
        )
    }

    /// `Move(move)`: all vertices by `move` scaled by the coordinate scale.
    pub fn move_all(&mut self, mv: &Vector2) {
        let scaled_move = Vector2::new(mv.x * self.coordinate_scale.x, mv.y * self.coordinate_scale.y);
        for v in self.vertices.iter_mut() {
            v.x += scaled_move.x;
            v.y += scaled_move.y;
        }
    }

    /// `Force_Alpha(alpha)`: `(unsigned)(Clamp(alpha, 0, 1) * 255.0f)` into the top byte of every colour.
    pub fn force_alpha(&mut self, alpha: f32) {
        let mut a = (wwmath::clamp(alpha, 0.0, 1.0) * 255.0f32) as u32;
        a <<= 24;
        for c in self.colors.iter_mut() {
            *c = (*c & 0x00FF_FFFF) | a;
        }
    }

    /// `Force_Color(color)`.
    pub fn force_color(&mut self, color: u32) {
        for c in self.colors.iter_mut() {
            *c = color;
        }
    }

    fn internal_add_quad_vertices(&mut self, v0: &Vector2, v1: &Vector2, v2: &Vector2, v3: &Vector2) {
        let a = self.convert_vert(v0);
        let b = self.convert_vert(v1);
        let c = self.convert_vert(v2);
        let d = self.convert_vert(v3);
        self.vertices.extend_from_slice(&[a, b, c, d]);
    }

    fn internal_add_quad_vertices_rect(&mut self, screen: &RectClass) {
        let a = self.convert_vert_xy(screen.left, screen.top);
        let b = self.convert_vert_xy(screen.left, screen.bottom);
        let c = self.convert_vert_xy(screen.right, screen.top);
        let d = self.convert_vert_xy(screen.right, screen.bottom);
        self.vertices.extend_from_slice(&[a, b, c, d]);
    }

    fn internal_add_quad_uvs(&mut self, uv: &RectClass) {
        self.uv_coordinates.push(Vector2::new(uv.left, uv.top));
        self.uv_coordinates.push(Vector2::new(uv.left, uv.bottom));
        self.uv_coordinates.push(Vector2::new(uv.right, uv.top));
        self.uv_coordinates.push(Vector2::new(uv.right, uv.bottom));
    }

    fn internal_add_quad_colors(&mut self, color: u32) {
        for _ in 0..4 {
            self.colors.push(color);
        }
    }

    fn internal_add_quad_vcolors(&mut self, color1: u32, color2: u32) {
        self.colors.extend_from_slice(&[color1, color2, color1, color2]);
    }

    fn internal_add_quad_hcolors(&mut self, color1: u32, color2: u32) {
        self.colors.extend_from_slice(&[color1, color1, color2, color2]);
    }

    /// `Internal_Add_Quad_Indicies(start_vert_index, backfaced)` (render2d.cpp:333): winding depends on the sign
    /// of `CoordinateScale.X * CoordinateScale.Y`.
    fn internal_add_quad_indicies(&mut self, start_vert_index: u16, backfaced: bool) {
        let s = start_vert_index;
        if backfaced ^ (self.coordinate_scale.x * self.coordinate_scale.y > 0.0) {
            self.indices.extend_from_slice(&[s + 1, s, s + 2, s + 1, s + 2, s + 3]);
        } else {
            self.indices.extend_from_slice(&[s, s + 1, s + 2, s + 2, s + 1, s + 3]);
        }
    }

    /// `Add_Quad(v0, v1, v2, v3, uv, color)`.
    pub fn add_quad_v(&mut self, v0: &Vector2, v1: &Vector2, v2: &Vector2, v3: &Vector2, uv: &RectClass, color: u32) {
        self.internal_add_quad_indicies(self.vertices.len() as u16, false);
        self.internal_add_quad_vertices(v0, v1, v2, v3);
        self.internal_add_quad_uvs(uv);
        self.internal_add_quad_colors(color);
    }

    /// `Add_Quad_Backfaced`.
    pub fn add_quad_backfaced(&mut self, v0: &Vector2, v1: &Vector2, v2: &Vector2, v3: &Vector2, uv: &RectClass, color: u32) {
        self.internal_add_quad_indicies(self.vertices.len() as u16, true);
        self.internal_add_quad_vertices(v0, v1, v2, v3);
        self.internal_add_quad_uvs(uv);
        self.internal_add_quad_colors(color);
    }

    /// `Add_Quad_VGradient(v0..v3, uv, top_color, bottom_color)`.
    pub fn add_quad_vgradient_v(&mut self, v0: &Vector2, v1: &Vector2, v2: &Vector2, v3: &Vector2, uv: &RectClass, top_color: u32, bottom_color: u32) {
        self.internal_add_quad_indicies(self.vertices.len() as u16, false);
        self.internal_add_quad_vertices(v0, v1, v2, v3);
        self.internal_add_quad_uvs(uv);
        self.internal_add_quad_vcolors(top_color, bottom_color);
    }

    /// `Add_Quad_HGradient(v0..v3, uv, left_color, right_color)`.
    pub fn add_quad_hgradient_v(&mut self, v0: &Vector2, v1: &Vector2, v2: &Vector2, v3: &Vector2, uv: &RectClass, left_color: u32, right_color: u32) {
        self.internal_add_quad_indicies(self.vertices.len() as u16, false);
        self.internal_add_quad_vertices(v0, v1, v2, v3);
        self.internal_add_quad_uvs(uv);
        self.internal_add_quad_hcolors(left_color, right_color);
    }

    /// `Add_Quad_VGradient(screen, top_color, bottom_color)`: UV (0,0)-(1,1).
    pub fn add_quad_vgradient(&mut self, screen: &RectClass, top_color: u32, bottom_color: u32) {
        self.internal_add_quad_indicies(self.vertices.len() as u16, false);
        self.internal_add_quad_vertices_rect(screen);
        self.internal_add_quad_uvs(&RectClass::new(0.0, 0.0, 1.0, 1.0));
        self.internal_add_quad_vcolors(top_color, bottom_color);
    }

    /// `Add_Quad_HGradient(screen, left_color, right_color)`.
    pub fn add_quad_hgradient(&mut self, screen: &RectClass, left_color: u32, right_color: u32) {
        self.internal_add_quad_indicies(self.vertices.len() as u16, false);
        self.internal_add_quad_vertices_rect(screen);
        self.internal_add_quad_uvs(&RectClass::new(0.0, 0.0, 1.0, 1.0));
        self.internal_add_quad_hcolors(left_color, right_color);
    }

    /// `Add_Quad(screen, uv, color)`.
    pub fn add_quad_uv(&mut self, screen: &RectClass, uv: &RectClass, color: u32) {
        self.internal_add_quad_indicies(self.vertices.len() as u16, false);
        self.internal_add_quad_vertices_rect(screen);
        self.internal_add_quad_uvs(uv);
        self.internal_add_quad_colors(color);
    }

    /// `Add_Quad(v0..v3, color)`: UV (0,0)-(1,1).
    pub fn add_quad_v_color(&mut self, v0: &Vector2, v1: &Vector2, v2: &Vector2, v3: &Vector2, color: u32) {
        self.internal_add_quad_indicies(self.vertices.len() as u16, false);
        self.internal_add_quad_vertices(v0, v1, v2, v3);
        self.internal_add_quad_uvs(&RectClass::new(0.0, 0.0, 1.0, 1.0));
        self.internal_add_quad_colors(color);
    }

    /// `Add_Quad(screen, color)`.
    pub fn add_quad(&mut self, screen: &RectClass, color: u32) {
        self.internal_add_quad_indicies(self.vertices.len() as u16, false);
        self.internal_add_quad_vertices_rect(screen);
        self.internal_add_quad_uvs(&RectClass::new(0.0, 0.0, 1.0, 1.0));
        self.internal_add_quad_colors(color);
    }

    /// `Add_Tri(v0, v1, v2, uv0, uv1, uv2, color)`.
    pub fn add_tri(&mut self, v0: &Vector2, v1: &Vector2, v2: &Vector2, uv0: &Vector2, uv1: &Vector2, uv2: &Vector2, color: u32) {
        let old_vert_count = self.vertices.len() as u16;
        let a = self.convert_vert(v0);
        let b = self.convert_vert(v1);
        let c = self.convert_vert(v2);
        self.vertices.extend_from_slice(&[a, b, c]);
        self.uv_coordinates.extend_from_slice(&[*uv0, *uv1, *uv2]);
        self.colors.extend_from_slice(&[color, color, color]);
        self.indices.extend_from_slice(&[old_vert_count, old_vert_count + 1, old_vert_count + 2]);
    }

    /// The perpendicular half-width offset `Add_Line` computes: `a - b` rotated 90 degrees, normalised, scaled to
    /// `width / 2`.
    fn line_corner_offset(a: &Vector2, b: &Vector2, width: f32) -> Vector2 {
        let mut corner_offset = Vector2::new(a.x - b.x, a.y - b.y); // line relative to b
        let temp = corner_offset.x; // Rotate 90
        corner_offset.x = corner_offset.y;
        corner_offset.y = -temp;
        corner_offset.normalize(); // scale to length width/2
        corner_offset.x *= width / 2.0;
        corner_offset.y *= width / 2.0;
        corner_offset
    }

    /// `Add_Line(a, b, width, uv, color)` (`uv` defaults to (0,0)-(1,1) in the 4-argument overload).
    pub fn add_line(&mut self, a: &Vector2, b: &Vector2, width: f32, uv: &RectClass, color: u32) {
        let c = Self::line_corner_offset(a, b, width);
        self.add_quad_v(
            &Vector2::new(a.x - c.x, a.y - c.y),
            &Vector2::new(a.x + c.x, a.y + c.y),
            &Vector2::new(b.x - c.x, b.y - c.y),
            &Vector2::new(b.x + c.x, b.y + c.y),
            uv,
            color,
        );
    }

    /// `Add_Line(a, b, width, uv, color, color2)`: the quad has a horizontal gradient.
    pub fn add_line_gradient(&mut self, a: &Vector2, b: &Vector2, width: f32, uv: &RectClass, color: u32, color2: u32) {
        let c = Self::line_corner_offset(a, b, width);
        self.add_quad_hgradient_v(
            &Vector2::new(a.x - c.x, a.y - c.y),
            &Vector2::new(a.x + c.x, a.y + c.y),
            &Vector2::new(b.x - c.x, b.y - c.y),
            &Vector2::new(b.x + c.x, b.y + c.y),
            uv,
            color,
            color2,
        );
    }

    /// `Add_Rect(rect, border_width, border_color, fill_color)` (render2d.cpp:515): the fill is inset by
    /// `border_width + 1` on the top/left and `border_width - 1` on the bottom/right, as upstream.
    pub fn add_rect(&mut self, rect: &RectClass, border_width: f32, border_color: u32, fill_color: u32) {
        if border_width > 0.0 {
            self.add_outline(rect, border_width, border_color);
        }
        let mut fill_rect = *rect;
        if border_width > 0.0 {
            fill_rect.left += border_width + 1.0;
            fill_rect.top += border_width + 1.0;
            fill_rect.right -= border_width - 1.0;
            fill_rect.bottom -= border_width - 1.0;
        }
        self.add_quad(&fill_rect, fill_color);
    }

    /// `Add_Outline(rect, width, color)` / `(rect, width, uv, color)` (the `uv` is unused by the 4-line body, which
    /// calls the 4-argument `Add_Line`): the +1/-1 tweaks are upstream's "to get precise line drawing".
    pub fn add_outline(&mut self, rect: &RectClass, width: f32, color: u32) {
        let uv = RectClass::new(0.0, 0.0, 1.0, 1.0);
        self.add_line(&Vector2::new(rect.left + 1.0, rect.bottom), &Vector2::new(rect.left + 1.0, rect.top + 1.0), width, &uv, color);
        self.add_line(&Vector2::new(rect.left, rect.top + 1.0), &Vector2::new(rect.right - 1.0, rect.top + 1.0), width, &uv, color);
        self.add_line(&Vector2::new(rect.right, rect.top), &Vector2::new(rect.right, rect.bottom - 1.0), width, &uv, color);
        self.add_line(&Vector2::new(rect.right, rect.bottom), &Vector2::new(rect.left + 1.0, rect.bottom), width, &uv, color);
    }

    /// `Render2DClass::Render` (render2d.cpp:540), the data half: `None` when empty or hidden. The viewport setup,
    /// identity world/view/projection and the vertex/index buffer writes are device-only.
    pub fn render(&self) -> Option<Render2DDraw> {
        if self.indices.is_empty() || self.is_hidden {
            return None;
        }
        let mut positions = Vec::with_capacity(self.vertices.len());
        for v in &self.vertices {
            positions.push(Vector3::new(v.x, v.y, self.z_value));
        }
        Some(Render2DDraw {
            positions,
            colors: self.colors.clone(),
            uvs: self.uv_coordinates.clone(),
            indices: self.indices.clone(),
            // grayscale: Set_Shader(_PresetOpaqueShader) + texture-factor stage states (device-only)
            shader: self.shader,
            texture: self.texture,
            grayscale: self.is_gray_scale,
            triangle_count: (self.indices.len() / 3) as u32,
        })
    }
}

/// `class Render2DTextClass` (render2d.h).
#[derive(Clone, Debug)]
pub struct Render2DTextClass {
    pub base: Render2DClass,
    pub location: Vector2,
    pub cursor: Vector2,
    pub font: Option<Font3DInstanceClass>,
    pub wrap_width: f32,
    pub draw_extents: RectClass,
    pub total_extents: RectClass,
    pub block_uv: RectClass,
    pub clip_rect: RectClass,
    pub is_clipped_enabled: bool,
}

impl Render2DTextClass {
    /// `Render2DTextClass::Render2DTextClass(font)` (render2d.cpp:588).
    pub fn new(font: Option<Font3DInstanceClass>, screen_resolution: RectClass, screen_uv_biased: bool) -> Self {
        let mut t = Render2DTextClass {
            base: Render2DClass::new(None, screen_resolution, screen_uv_biased),
            location: Vector2::new(0.0, 0.0),
            cursor: Vector2::new(0.0, 0.0),
            font: None,
            wrap_width: 0.0,
            draw_extents: RectClass::default(),
            total_extents: RectClass::default(),
            block_uv: RectClass::default(),
            clip_rect: RectClass::new(0.0, 0.0, 0.0, 0.0),
            is_clipped_enabled: false,
        };
        t.base.set_coordinate_range(&RectClass::new(-320.0, -240.0, 320.0, 240.0));
        t.set_font(font);
        t.reset();
        t
    }

    /// `Render2DTextClass::Reset` (render2d.cpp:608).
    pub fn reset(&mut self) {
        self.base.reset();
        self.cursor = self.location;
        self.wrap_width = 0.0;
        self.draw_extents = RectClass::new(0.0, 0.0, 0.0, 0.0);
        self.total_extents = RectClass::new(0.0, 0.0, 0.0, 0.0);
        self.clip_rect.set(0.0, 0.0, 0.0, 0.0);
        self.is_clipped_enabled = false;
    }

    /// `Render2DTextClass::Set_Font` (render2d.cpp:622): the texture handle is the font texture's (supplied by the
    /// caller through `base.set_texture`); `BlockUV` is `Char_UV(BLOCK_CHAR = 0)` inflated by `-w/4, -h/4`.
    pub fn set_font(&mut self, font: Option<Font3DInstanceClass>) {
        if let Some(f) = &font {
            let mut block_uv = f.char_uv(0);
            let infl = Vector2::new(-block_uv.width() / 4.0, -block_uv.height() / 4.0);
            block_uv.inflate(&infl);
            self.block_uv = block_uv;
        }
        self.font = font;
    }

    /// `Render2DTextClass::Draw_Char` (render2d.cpp:640).
    pub fn draw_char(&mut self, ch: u16, color: u32) {
        let font = self.font.as_ref().expect("font");
        let char_spacing = font.char_spacing(ch);
        let char_height = font.char_height();
        let char_width = font.char_width(ch);
        let uv = font.char_uv(ch);

        let mut is_clipped = false;
        if self.is_clipped_enabled
            && (self.cursor.x < self.clip_rect.left
                || self.cursor.x + char_spacing > self.clip_rect.right
                || self.cursor.y < self.clip_rect.top
                || self.cursor.y + char_height > self.clip_rect.bottom)
        {
            is_clipped = true;
        }

        if ch != ' ' as u16 && !is_clipped {
            let screen = RectClass::new(self.cursor.x, self.cursor.y, self.cursor.x + char_width, self.cursor.y + char_height);
            self.base.internal_add_quad_indicies(self.base.vertices.len() as u16, false);
            self.base.internal_add_quad_vertices_rect(&screen);
            self.base.internal_add_quad_uvs(&uv);
            self.base.internal_add_quad_colors(color);
            self.draw_extents.union_with(&screen);
            self.total_extents.union_with(&screen);
        }
        self.cursor.x += char_spacing;
    }

    /// `Draw_Text(const char *, color)`: widens each byte.
    pub fn draw_text_ascii(&mut self, text: &[u8], color: u32) {
        let wide: Vec<u16> = text.iter().take_while(|&&b| b != 0).map(|&b| b as u16).collect();
        self.draw_text(&wide, color);
    }

    /// `Draw_Text(const WCHAR *, color)` (render2d.cpp:688): word wrap at spaces when `WrapWidth > 0`, newline wraps.
    pub fn draw_text(&mut self, text: &[u16], color: u32) {
        self.draw_extents = RectClass::from_corners(&self.location, &self.location);
        if self.total_extents.width() == 0.0 {
            self.total_extents = RectClass::from_corners(&self.location, &self.location);
        }
        let mut i = 0usize;
        while i < text.len() && text[i] != 0 {
            let ch = text[i];
            i += 1;
            let mut wrap = ch == '\n' as u16;
            if ch == ' ' as u16 && self.wrap_width > 0.0 {
                // look ahead to the end of the next word
                let font = self.font.as_ref().expect("font");
                let mut word = i;
                let mut word_width = font.char_spacing(ch);
                while word < text.len() && text[word] != 0 && text[word] > ' ' as u16 {
                    word_width += font.char_spacing(text[word]);
                    word += 1;
                }
                wrap = (self.cursor.x + word_width) >= (self.location.x + self.wrap_width);
            }
            if wrap {
                self.cursor.y += self.font.as_ref().expect("font").char_height();
                self.cursor.x = self.location.x;
            } else {
                self.draw_char(ch, color);
            }
        }
    }

    /// `Draw_Block(screen, color)` (render2d.cpp:725): a solid block using `BlockUV`.
    pub fn draw_block(&mut self, screen: &RectClass, color: u32) {
        self.base.internal_add_quad_indicies(self.base.vertices.len() as u16, false);
        self.base.internal_add_quad_vertices_rect(screen);
        let uv = self.block_uv;
        self.base.internal_add_quad_uvs(&uv);
        self.base.internal_add_quad_colors(color);
        self.total_extents.union_with(screen);
    }

    /// `Get_Text_Extents(text)`: width of the non-newline characters, one line high.
    pub fn get_text_extents(&self, text: Option<&[u16]>) -> Vector2 {
        let font = self.font.as_ref().expect("font");
        let mut extent = Vector2::new(0.0, font.char_height());
        if let Some(t) = text {
            for &ch in t {
                if ch == 0 {
                    break;
                }
                if ch != '\n' as u16 {
                    extent.x += font.char_spacing(ch);
                }
            }
        }
        extent
    }
}
