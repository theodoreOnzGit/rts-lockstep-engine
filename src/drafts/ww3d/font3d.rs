//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/font3d.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/font3d.h
//!
//! `Font3DDataClass` loads a targa font image through `SurfaceClass(filename)` and the asset manager caches it
//! (`Get_Font3DData`); here the decoded [`SurfaceClass`] is passed in and the resulting font texture is returned as
//! a surface (`TextureClass(_surface, MIP_LEVELS_1)` is the texture layer's). `Font3DInstanceClass` holds the font
//! data by value (`FontData` is a ref-counted shared pointer upstream; callers that share one wrap it in `Arc`).
//! Upstream's file-static `_surface` is the local return value.

use super::rect::RectClass;
use super::surfaceclass::{SurfaceClass, SurfaceDescription};
use crate::formats::texture::format::Ww3dFormat;
use crate::ww::vector4::Vector4;

/// `class Font3DDataClass` (font3d.h).
#[derive(Clone, Debug)]
pub struct Font3DDataClass {
    pub name: String,
    /// The finished font image (A4R4G4B4, 128 or 256 square): what `Texture` is made from.
    pub texture_surface: SurfaceClass,
    u_offset_table: [f32; 256],
    v_offset_table: [f32; 256],
    u_width_table: [f32; 256],
    v_height: f32,
    char_width_table: [u8; 256],
    char_height: u8,
}

impl Font3DDataClass {
    /// `Font3DDataClass::Font3DDataClass(filename)` (font3d.cpp:36), with the decoded image supplied. The name is
    /// upper-cased (`strupr`).
    pub fn new(filename: &str, image: SurfaceClass) -> Self {
        let mut f = Font3DDataClass {
            name: filename.to_ascii_uppercase(),
            texture_surface: image.clone(),
            u_offset_table: [0.0; 256],
            v_offset_table: [0.0; 256],
            u_width_table: [0.0; 256],
            v_height: 0.0,
            char_width_table: [0; 256],
            char_height: 0,
        };
        f.load_font_image(image);
        f
    }

    /// `Char_Width(ch)` (`ch & 0xFF`).
    pub fn char_width(&self, ch: u16) -> u8 {
        self.char_width_table[(ch & 0xFF) as usize]
    }
    /// `Char_Height(ch)`.
    pub fn char_height(&self, _ch: u16) -> u8 {
        self.char_height
    }
    pub fn char_u_offset(&self, ch: u16) -> f32 {
        self.u_offset_table[(ch & 0xFF) as usize]
    }
    pub fn char_v_offset(&self, ch: u16) -> f32 {
        self.v_offset_table[(ch & 0xFF) as usize]
    }
    pub fn char_u_width(&self, ch: u16) -> f32 {
        self.u_width_table[(ch & 0xFF) as usize]
    }
    pub fn char_v_height(&self, _ch: u16) -> f32 {
        self.v_height
    }
    /// `Char_UV_Corners(ch)`: note upstream indexes the tables with `ch` unmasked.
    pub fn char_uv_corners(&self, ch: u16) -> Vector4 {
        let c = ch as usize;
        Vector4 {
            x: self.u_offset_table[c],
            y: self.v_offset_table[c],
            z: self.u_offset_table[c] + self.u_width_table[c],
            w: self.v_offset_table[c] + self.v_height,
        }
    }

    /// `Font3DDataClass::Minimize_Font_Image` (font3d.cpp:53): re-stacks the characters into a 128 or 256 square
    /// A4R4G4B4 surface and rewrites the tables.
    fn minimize_font_image(&mut self, surface: &SurfaceClass) -> SurfaceClass {
        let sd: SurfaceDescription = surface.get_description();
        let current_width = sd.width as f32;
        let current_height = sd.height as f32;
        let new_width: i32 = if current_width < 256.0 { 128 } else { 256 };
        let new_height = new_width;
        let mut new_surface = SurfaceClass::new(new_width as u32, new_height as u32, Ww3dFormat::A4R4G4B4);
        new_surface.clear();

        let mut new_x: i32 = 0;
        let mut new_y: i32 = 0;
        for char_index in 0..256usize {
            let src_x = (self.u_offset_table[char_index] * current_width + 0.5) as i32;
            let src_y = (self.v_offset_table[char_index] * current_height + 0.5) as i32;
            let width = (self.u_width_table[char_index] * current_width + 0.5) as i32;
            let height = (self.v_height * current_height + 0.5) as i32;
            if width != 0 {
                if new_x + width > new_width {
                    new_x = 0;
                    new_y += height;
                    if new_y + height > new_height {
                        new_y -= height;
                        // WWDEBUG_SAY(( "Font doesn't fit texture 2 on char %c\n", char_index ))
                    }
                }
                new_surface.copy_from_surface(new_x as u32, new_y as u32, src_x as u32, src_y as u32, width as u32, height as u32, surface);
            }
            self.u_offset_table[char_index] = new_x as f32 / new_width as f32;
            self.v_offset_table[char_index] = new_y as f32 / new_width as f32; // upstream divides by new_width
            self.u_width_table[char_index] *= current_width / new_width as f32;
            new_x += width;
        }
        self.v_height *= current_height / new_height as f32;
        new_surface
    }

    /// `Font3DDataClass::Make_Proportional` (font3d.cpp:107): tightens each character cell to its non-transparent
    /// columns, then minimises. Returns the minimised surface (upstream returns NULL and leaves it in `_surface`).
    fn make_proportional(&mut self, surface: &SurfaceClass) -> SurfaceClass {
        let sd = surface.get_description();
        let width = sd.width as f32;
        let height = sd.height as f32;
        for char_index in 0..256usize {
            let mut x0 = (self.u_offset_table[char_index] * width + 0.5) as i32;
            let y0 = (self.v_offset_table[char_index] * height + 0.5) as i32;
            let mut x1 = x0 + (self.u_width_table[char_index] * width + 0.5) as i32;
            let y1 = y0 + (self.v_height * height + 0.5) as i32;

            let mut minb = (x0, y0);
            let mut maxb = (x1, y1);
            surface.find_bb(&mut minb, &mut maxb);
            x0 = minb.0;
            x1 = maxb.0 + 1;
            if x1 < x0 {
                x1 = x0;
            }
            if char_index > 0x80 {
                x1 = x0;
            }
            self.u_offset_table[char_index] = x0 as f32 / width;
            self.u_width_table[char_index] = (x1 - x0) as f32 / width;
            self.char_width_table[char_index] = (x1 - x0) as u8;
        }
        self.minimize_font_image(surface)
    }

    /// `Font3DDataClass::Load_Font_Image` (font3d.cpp:150).
    fn load_font_image(&mut self, surface: SurfaceClass) {
        let sd = surface.get_description();
        let final_surface;
        if sd.width > 8 * sd.height {
            // A strip: one row of characters of varying width.
            self.v_height = 1.0;
            self.char_height = sd.height as u8;
            let mut column: u32 = 0;
            let width = sd.width;
            for char_index in 0..256usize {
                if char_index >= 0x7F {
                    self.u_offset_table[char_index] = 0.0;
                    self.v_offset_table[char_index] = 0.0;
                    self.u_width_table[char_index] = 0.0;
                    self.char_width_table[char_index] = 0;
                } else {
                    while column < width && surface.is_transparent_column(column) {
                        column += 1;
                    }
                    let start = column;
                    while column < width && !surface.is_transparent_column(column) {
                        column += 1;
                    }
                    let end = column;
                    // if end <= start: WWDEBUG_SAY(( "Error Char %d start %d end %d width %d\n", ... ))
                    self.u_offset_table[char_index] = start as f32 / width as f32;
                    self.v_offset_table[char_index] = 0.0;
                    self.u_width_table[char_index] = (end as f32 - start as f32) / width as f32;
                    self.char_width_table[char_index] = (end as i32 - start as i32) as u8;
                }
            }
            final_surface = self.minimize_font_image(&surface);
        } else {
            // A 16x16 grid of equal cells.
            let font_width = sd.width as f32;
            let font_height = sd.height as f32;
            let mono_pixel_width = font_width / 16.0;
            let mono_pixel_height = font_height / 16.0;
            for char_index in 0..256usize {
                self.u_offset_table[char_index] = ((char_index % 16) as f32 * mono_pixel_width) / font_width;
                self.v_offset_table[char_index] = ((char_index / 16) as f32 * mono_pixel_height) / font_height;
                self.u_width_table[char_index] = mono_pixel_width / font_width;
                self.char_width_table[char_index] = mono_pixel_width as u8;
            }
            self.v_height = mono_pixel_height / font_height;
            self.char_height = mono_pixel_height as u8;
            final_surface = self.make_proportional(&surface);
        }
        self.texture_surface = final_surface;
    }
}

/// `class Font3DInstanceClass` (font3d.h): a scaled view of a [`Font3DDataClass`].
#[derive(Clone, Debug)]
pub struct Font3DInstanceClass {
    font_data: Font3DDataClass,
    scale: f32,
    space_spacing: f32,
    inter_char_spacing: f32,
    mono_spacing: f32,
    scaled_width_table: [f32; 256],
    scaled_spacing_table: [f32; 256],
    scaled_height: f32,
}

impl Font3DInstanceClass {
    /// `Font3DInstanceClass::Font3DInstanceClass(filename)` (font3d.cpp:233), with the data already fetched.
    pub fn new(font_data: Font3DDataClass) -> Self {
        let space_spacing = (font_data.char_width('H' as u16) as f32 / 2.0f32) as i32 as f32;
        let mut f = Font3DInstanceClass {
            font_data,
            scale: 1.0,
            space_spacing,
            inter_char_spacing: 1.0,
            mono_spacing: 0.0,
            scaled_width_table: [0.0; 256],
            scaled_spacing_table: [0.0; 256],
            scaled_height: 0.0,
        };
        f.build_cached_tables();
        f
    }

    pub fn peek_font_data(&self) -> &Font3DDataClass {
        &self.font_data
    }

    /// `Set_Mono_Spaced`: `MonoSpacing = Char_Width('W') + 1`.
    pub fn set_mono_spaced(&mut self) {
        self.mono_spacing = self.font_data.char_width('W' as u16) as f32 + 1.0;
        self.build_cached_tables();
    }

    pub fn set_proportional(&mut self) {
        self.mono_spacing = 0.0;
        self.build_cached_tables();
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.build_cached_tables();
    }

    pub fn char_width(&self, ch: u16) -> f32 {
        self.scaled_width_table[(ch & 0xFF) as usize]
    }
    pub fn char_spacing(&self, ch: u16) -> f32 {
        self.scaled_spacing_table[(ch & 0xFF) as usize]
    }
    pub fn char_height(&self) -> f32 {
        self.scaled_height
    }

    /// `Font3DInstanceClass::Build_Cached_Tables` (font3d.cpp:258).
    fn build_cached_tables(&mut self) {
        for a in 0..256usize {
            let mut width = self.font_data.char_width(a as u16) as f32;
            if a == ' ' as usize {
                width = self.space_spacing;
            }
            self.scaled_width_table[a] = self.scale * width;
            if self.mono_spacing != 0.0 {
                self.scaled_spacing_table[a] = self.scale * self.mono_spacing;
            } else {
                self.scaled_spacing_table[a] = self.scale * (width + self.inter_char_spacing);
            }
        }
        self.scaled_height = (self.scale * self.font_data.char_height('A' as u16) as f32).floor();
    }

    /// `String_Width(const WCHAR *)`: sum of the spacing of each character.
    pub fn string_width(&self, test_str: &[u16]) -> f32 {
        let mut width = 0.0f32;
        for &c in test_str {
            if c == 0 {
                break;
            }
            width += self.char_spacing(c);
        }
        width
    }

    /// `String_Width(const char *)`.
    pub fn string_width_ascii(&self, test_str: &[u8]) -> f32 {
        let mut width = 0.0f32;
        for &c in test_str {
            if c == 0 {
                break;
            }
            width += self.char_spacing(c as u16);
        }
        width
    }

    /// `Char_UV(ch)`: the character's rectangle in the font texture.
    pub fn char_uv(&self, ch: u16) -> RectClass {
        RectClass::new(
            self.font_data.char_u_offset(ch),
            self.font_data.char_v_offset(ch),
            self.font_data.char_u_offset(ch) + self.font_data.char_u_width(ch),
            self.font_data.char_v_offset(ch) + self.font_data.char_v_height(ch),
        )
    }
}
