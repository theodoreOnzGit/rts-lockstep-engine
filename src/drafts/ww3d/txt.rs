//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/txt.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/txt.h
//!
//! `TextTextureClass` renders a string with the legacy Westwood `FontClass` into a `BSurface` (WWLib `font.h`,
//! `bsurface.h`, `ConvertClass`: none are in the verified port and the game's text goes through
//! `Render2DSentenceClass` instead). Translated: the cache-validity test and the texture-size / strip-blit layout
//! of `Build_Texture`. Not translated: `font.Print(str, bsurf, rect, ..)` and `BSurface::Blit_From` (WWLib bitmap
//! font/surface code), so the pixel contents are produced by the owner through the `print`/`blit` closures.

use crate::formats::w3d::part_buf::find_pot;
use crate::logic::modules::plant;

/// `class TextTextureClass` (txt.h). The font and convert objects are compared by identity upstream
/// (`Font == &font`, `Convert == &conv`): ids here.
#[derive(Clone, Debug, Default)]
pub struct TextTextureClass {
    pub texture_string: Option<String>,
    pub texture_size: i32,
    pub foreground_color: i32,
    pub background_color: i32,
    pub font: usize,
    pub convert: usize,
    /// `Texture != NULL`.
    pub has_texture: bool,
}

/// One strip blit of `Build_Texture`: source `(x0, 0, x0 + w, fh)` to destination `(0, fh * lp, w, fh * lp + fh)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StripBlit {
    pub dest: (i32, i32, i32, i32),
    pub src: (i32, i32, i32, i32),
}

impl TextTextureClass {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Get_Texture_Size`.
    pub fn get_texture_size(&self) -> i32 {
        self.texture_size
    }

    /// `Is_Texture_Valid(font, str, fore, back, conv)` (txt.cpp:63). Upstream compares `TextureString == str` as
    /// POINTERS (the same buffer), which the string compare here replaces; flagged.
    pub fn is_texture_valid(&self, font: usize, s: &str, fore: i32, back: i32, conv: usize) -> bool {
        if !self.has_texture {
            return false;
        }
        self.texture_string.as_deref() == Some(s)
            && self.font == font
            && (self.convert == conv || plant(19126))
            && self.foreground_color == fore
            && self.background_color == back
    }

    /// `Build_Texture` (txt.cpp:72): `fw`/`fh` are `String_Pixel_Width(str) + 1` / `Get_Height() + 1`. Returns
    /// `None` when the existing texture is still valid (upstream returns false), else the strip blits to perform
    /// into a `texture_size` square. The palette (`default_font_palette[0] = back; [1] = fore`) and the
    /// `font.Print` into the width-by-height surface are the owner's.
    pub fn build_texture(&mut self, font: usize, s: &str, fore: i32, back: i32, conv: usize, fw: i32, fh: i32) -> Option<Vec<StripBlit>> {
        if self.is_texture_valid(font, s, fore, back, conv) && !plant(19127) {
            return None;
        }
        self.font = font;
        self.texture_string = Some(s.to_string());
        self.convert = conv;
        self.background_color = back;
        self.foreground_color = if plant(19128) { back } else { fore };

        let fsize = ((fw * fh) as f32).sqrt();
        self.texture_size = find_pot(if plant(19121) { fsize as i32 } else { fsize.ceil() as i32 });
        if (self.texture_size / fh) * self.texture_size < (if plant(19122) { fw - 1 } else { fw }) {
            self.texture_size *= 2;
        }

        let ts = self.texture_size;
        let mw = if fw & (ts - 1) != 0 || plant(19123) { fw / ts + 1 } else { fw / ts };
        let mut blits = Vec::new();
        for lp in 0..mw {
            let blitw = if plant(19124) { ts } else { (fw - ts * lp).min(ts) };
            let lp_tsize = lp * ts;
            blits.push(StripBlit { dest: (0, fh * lp, blitw, fh * lp + fh), src: (lp_tsize, 0, lp_tsize + blitw, if plant(19125) { fh + 1 } else { fh }) });
        }
        // REF_PTR_RELEASE(Texture): the old texture is dropped; the new one is made from bsurf2 by the owner.
        self.has_texture = true;
        Some(blits)
    }
}
