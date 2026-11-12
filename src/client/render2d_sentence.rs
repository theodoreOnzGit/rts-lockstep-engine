// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit
// 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/render2dsentence.cpp (FontCharsClass::Create_GDI_Font, Get_Char_Data,
//     Get_Char_Spacing, Store_GDI_Char's measure, Render2DSentenceClass::Build_Sentence_Not_Centered's extents and
//     Allocate_New_Surface's texture size)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplayString.cpp (computeExtents, getSize)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameFont.cpp (loadFontData's height and
//     alternate Unicode font)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! (#153) How upstream measures a string: `DisplayString::getSize`, `winFontHeight`. Client only: the logic never
//! measures text.
//!
//! - [`FontChars`] is `FontCharsClass` as far as it measures: [`FontChars::new`] is `Initialize_GDI_Font` and
//!   `Create_GDI_Font` (the point size made pixels at 96 dots per inch, `MulDiv(PointSize, 96, 72)`; the
//!   `Generals` font drawn as Arial at 0.40 of its height wide; `PixelOverlap`, an eighth of the height at most 4; the
//!   text metrics' height, ascent and overhang), [`FontChars::get_char_spacing`] is `Get_Char_Spacing` over
//!   `Get_Char_Data` and `Store_GDI_Char`'s width (`GetTextExtentPoint32W`'s, plus the overlap, plus one for `W`), with
//!   a character above 255 measured in the alternate Unicode font when there is one.
//! - [`formatted_text_extents`] is `Build_Sentence_Not_Centered`'s pass with `justCalcExtents` (what
//!   `Get_Formatted_Text_Extents` returns): the widest line at its breaks, the space wrap (the next word with the
//!   space before it, `>=` the wrap width) and the hard wrap, plus `Get_Extra_Overlap`.
//! - [`display_string_size`] is `W3DDisplayString::computeExtents` (an empty string or no font measures 0 by 0).
//! - (#155) [`formatted_text_line_starts`] is the same pass's line breaks, where the client starts each line it draws
//!   (new output: upstream draws each glyph where the pass puts it).
//!
//! What GDI answers (`CreateFont`'s text metrics, `GetTextExtentPoint32W`) is the host's ([`FontGdi`]): upstream's are
//! Windows' fonts, which a client elsewhere can only estimate. Checked against the C++ by `tests/differential_render2d.rs`
//! with a GDI of random answers.
//!
//! # Upstream quirks that are kept
//!
//! - A line wrapped at a space starts with that space's width.
//! - A character measured in the alternate font is its width there (with that font's overlap) less this font's overlap
//!   and overhang.
//!
//! # Where this differs from upstream
//!
//! - The texture's chunks are followed (`Allocate_New_Surface`'s size for the text left, and a chunk ended where the
//!   texture is full): with a character of negative spacing (an overhang wider than the glyph) the widest line is read
//!   at those ends too. A renderer that has drawn keeps its last surface (and its size); this one measures as a string
//!   never drawn (no surface, so each measure sizes one). `TextureSizeHint` is the caller's (0 for a display string).
//! - Hot keys (`ParseHotKey`, `&`) are not measured: the window manager's measure (`GuiHost::text_size`) has no hot key.
//! - The cache of measured characters is a map (upstream: the ASCII array, the grown Unicode array and the glyph
//!   buffers); a character is measured once either way.

use std::collections::BTreeMap;

use crate::logic::modules::plant;

/// `TEXTMETRIC`'s members `Create_GDI_Font` reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextMetric {
    pub height: i32,
    pub ascent: i32,
    pub overhang: i32,
}

/// What `FontCharsClass` asks of GDI. New: upstream calls Windows.
pub trait FontGdi {
    /// `CreateFont(height, width, 0, 0, weight, 0, FALSE, FALSE, DEFAULT_CHARSET, ..., VARIABLE_PITCH, face)`, selected
    /// into the font's DC: a font id, and `GetTextMetrics` of it.
    fn create_font(&mut self, height: i32, width: i32, weight: i32, face: &[u8]) -> (usize, TextMetric);
    /// `GetTextExtentPoint32W(dc, &ch, 1, &size)` in the font `font`: `(cx, cy)`.
    fn text_extent(&mut self, font: usize, ch: u16) -> (i32, i32);
}

/// `FW_NORMAL`, `FW_BOLD`.
pub const FW_NORMAL: i32 = 400;
pub const FW_BOLD: i32 = 700;

/// `MulDiv(number, numerator, denominator)` (Win32): the product over the denominator, rounded half away from zero; -1
/// for a zero denominator. New (Windows').
pub fn mul_div(number: i32, numerator: i32, denominator: i32) -> i32 {
    if denominator == 0 {
        return -1;
    }
    let n = i64::from(number) * i64::from(numerator);
    let d = i64::from(denominator);
    if plant(9463) {
        return (n / d) as i32;
    }
    let q = (n.abs() + d.abs() / 2) / d.abs();
    (if (n < 0) != (d < 0) { -q } else { q }) as i32
}

/// The alternate Unicode font (`AlternateUnicodeFont`): none, this font itself, or another.
#[derive(Clone, Debug, Default)]
pub enum Alternate {
    #[default]
    None,
    SelfFont,
    Other(Box<FontChars>),
}

/// `FontCharsClass`, as far as it measures.
#[derive(Clone, Debug, Default)]
pub struct FontChars {
    pub point_size: i32,
    pub is_bold: bool,
    /// The GDI font's id ([`FontGdi::create_font`]).
    pub gdi_font: usize,
    pub char_height: i32,
    pub char_ascent: i32,
    pub char_overhang: i32,
    pub pixel_overlap: i32,
    /// `Get_Char_Data`'s widths (`FontCharsClassCharDataStruct::Width`, a `short`), by character.
    pub widths: BTreeMap<u16, i16>,
    pub alternate: Alternate,
}

impl FontChars {
    /// `Initialize_GDI_Font(font_name, point_size, is_bold)`: the settings, then `Create_GDI_Font(font_name)`.
    pub fn new<G: FontGdi>(gdi: &mut G, font_name: &[u8], point_size: i32, is_bold: bool) -> FontChars {
        let mut f = FontChars { point_size, is_bold, ..Default::default() };
        f.create_gdi_font(gdi, font_name);
        f
    }

    /// `Create_GDI_Font` (render2dsentence.cpp:1513).
    fn create_gdi_font<G: FontGdi>(&mut self, gdi: &mut G, font_name: &[u8]) {
        let mut font_name = font_name;
        let mut doing_generals = false;
        if font_name == b"Generals" {
            font_name = b"Arial";
            doing_generals = true;
        }
        // "always use 96. jba."
        let dots_per_inch = 96;
        let font_height = -mul_div(self.point_size, dots_per_inch, 72);
        let mut font_width = 0;
        if doing_generals && !plant(9451) {
            // (`fontWidth = -font_height*0.40f;`: an int times a float, truncated)
            font_width = ((-font_height) as f32 * 0.40f32) as i32;
        }
        self.pixel_overlap = (-font_height) / if plant(9453) { 4 } else { 8 };
        if self.pixel_overlap < 0 {
            self.pixel_overlap = 0;
        }
        if self.pixel_overlap > 4 && !plant(9452) {
            self.pixel_overlap = 4;
        }
        let bold = if self.is_bold && !plant(9465) { FW_BOLD } else { FW_NORMAL };
        let (id, tm) = gdi.create_font(font_height, font_width, bold, font_name);
        self.gdi_font = id;
        self.char_height = tm.height;
        self.char_ascent = tm.ascent;
        self.char_overhang = tm.overhang;
        if doing_generals && !plant(9454) {
            self.char_overhang = 0;
        }
    }

    /// `Get_Char_Height()`: `CharHeight`, the text metrics' height (`GameFont::height`, `winFontHeight`).
    pub fn get_char_height(&self) -> i32 {
        self.char_height
    }

    /// `Get_Extra_Overlap()`: `PixelOverlap`.
    pub fn get_extra_overlap(&self) -> i32 {
        self.pixel_overlap
    }

    /// `Get_Char_Data(ch)`'s width (render2dsentence.cpp:1239): from the cache, else (a character above 255, with an
    /// alternate font that is not this one) the alternate's, else measured and kept (`Store_GDI_Char`).
    fn get_char_width_data<G: FontGdi>(&mut self, gdi: &mut G, ch: u16) -> i16 {
        if ch >= 256 && !plant(9456) {
            if let Alternate::Other(alt) = &mut self.alternate {
                return alt.get_char_width_data(gdi, ch);
            }
        }
        if let Some(&w) = self.widths.get(&ch) {
            return w;
        }
        self.store_gdi_char(gdi, ch)
    }

    /// `Store_GDI_Char(ch)`'s width (render2dsentence.cpp:1349): `GetTextExtentPoint32W`'s `cx`, plus `PixelOverlap`,
    /// plus `xOrigin` (1 for `W`, drawn one pixel right), kept as the character's `Width` (a `short`). The glyph's bits
    /// are not kept (nothing here draws them).
    fn store_gdi_char<G: FontGdi>(&mut self, gdi: &mut G, ch: u16) -> i16 {
        let x_origin = i32::from(ch == u16::from(b'W') && !plant(9455));
        let (cx, _cy) = gdi.text_extent(self.gdi_font, ch);
        let w = (cx + self.pixel_overlap + x_origin) as i16;
        self.widths.insert(ch, w);
        w
    }

    /// `Get_Char_Spacing(ch)` (render2dsentence.cpp:1292): the character's width less `PixelOverlap` and `CharOverhang`
    /// (this font's), 0 for a character of no width.
    pub fn get_char_spacing<G: FontGdi>(&mut self, gdi: &mut G, ch: u16) -> i32 {
        let w = i32::from(self.get_char_width_data(gdi, ch));
        if w != 0 {
            return w - self.pixel_overlap - if plant(9457) { 0 } else { self.char_overhang };
        }
        0
    }
}

/// `Render2DSentenceClass::Allocate_New_Surface(text, true)` (render2dsentence.cpp:632): the texture's size for the
/// text left (each of 64, 128 and 256 tried for the fewest texture pixels), at least `TextureSizeHint`. No surface is made.
fn allocate_new_surface<G: FontGdi>(font: &mut FontChars, gdi: &mut G, text: &[u16], texture_size_hint: i32) -> i32 {
    let mut text_width = 0i32;
    for &ch in text.iter().take_while(|&&c| c != 0) {
        text_width = text_width.wrapping_add(font.get_char_spacing(gdi, ch));
    }
    let char_height = font.get_char_height();
    let mut curr_texture_size = 256;
    let mut best_tex_mem_usage = 999_999_999;
    for pow2 in 6..=8 {
        let size = 1 << pow2;
        let row_count = (text_width / size) + 1;
        let rows_per_texture = size / (char_height + 1);
        if rows_per_texture > 0 {
            let texture_count = (row_count / rows_per_texture).max(1);
            let texture_mem_usage = texture_count.wrapping_mul(size).wrapping_mul(size);
            if texture_mem_usage < best_tex_mem_usage {
                curr_texture_size = size;
                best_tex_mem_usage = texture_mem_usage;
            }
        }
    }
    texture_size_hint.max(curr_texture_size)
}

/// `Render2DSentenceClass::Build_Sentence_Not_Centered(text, NULL, NULL, true)` (render2dsentence.cpp:1004), the
/// extents it returns (`Get_Formatted_Text_Extents`): `(maxX + Get_Extra_Overlap(), Cursor.Y + char_height)`.
/// `wrap_width` is `WrapWidth` (0: none), `hard_wrap` `useHardWordWrap`, `texture_size_hint` `TextureSizeHint`. The
/// text ends at its end or at a 0. The renderer has no surface (a string never drawn).
pub fn formatted_text_extents<G: FontGdi>(font: &mut FontChars, gdi: &mut G, text: &[u16], wrap_width: f32, hard_wrap: bool, texture_size_hint: i32) -> (f32, f32) {
    build_sentence_not_centered(font, gdi, text, wrap_width, hard_wrap, texture_size_hint, |_| {})
}

/// (#155) Where [`formatted_text_extents`]'s pass starts each line after the first: the index of the character the new
/// line starts with (the space a space wrap breaks before, which the new line starts with; the character after a
/// newline; the character a hard wrap moves down). New output of the same pass (upstream draws each character where the
/// pass puts it; the client draws the lines in its own font): the line count times the font's height is the extents'
/// height (`tests/differential_render2d.rs` checks it against the C++'s height).
pub fn formatted_text_line_starts<G: FontGdi>(font: &mut FontChars, gdi: &mut G, text: &[u16], wrap_width: f32, hard_wrap: bool, texture_size_hint: i32) -> Vec<usize> {
    let mut starts = Vec::new();
    build_sentence_not_centered(font, gdi, text, wrap_width, hard_wrap, texture_size_hint, |i| starts.push(i));
    starts
}

/// `Build_Sentence_Not_Centered`'s pass with `justCalcExtents` (see [`formatted_text_extents`]); `new_line(i)` is told
/// each line it starts after the first, at character `i` (#155).
fn build_sentence_not_centered<G: FontGdi>(font: &mut FontChars, gdi: &mut G, text: &[u16], wrap_width: f32, hard_wrap: bool, texture_size_hint: i32, mut new_line: impl FnMut(usize)) -> (f32, f32) {
    const TEXTURE_OFFSET: i32 = 2;
    let at = |i: usize| text.get(i).copied().unwrap_or(0);
    let (space, newline) = (u16::from(b' '), u16::from(b'\n'));
    let (mut cursor_x, mut cursor_y) = (0.0f32, 0.0f32);
    let mut max_x = 0.0f32;
    // (no surface: `Allocate_New_Surface(text, TRUE)` sizes one; then the offsets start at TEXTURE_OFFSET)
    let mut curr_texture_size = allocate_new_surface(font, gdi, text, texture_size_hint);
    let (mut texture_i, mut texture_j) = (TEXTURE_OFFSET, 0i32);
    let mut texture_start_x = TEXTURE_OFFSET;
    let char_height = font.get_char_height() as f32;
    let mut i = 0usize;
    loop {
        let ch = at(i);
        i += 1;
        let char_spacing = font.get_char_spacing(gdi, ch) as f32;
        let exceeded_texture_width = (texture_i as f32 + char_spacing) >= curr_texture_size as f32;
        let encountered_break_char = ch == space || ch == newline || ch == 0;
        let word_bigger_then_line = hard_wrap && !plant(9462) && wrap_width != 0.0 && (cursor_x + (texture_i - texture_start_x) as f32 + char_spacing) >= wrap_width;
        if exceeded_texture_width || encountered_break_char || word_bigger_then_line {
            cursor_x += (texture_i - texture_start_x) as f32;
            max_x = if max_x > cursor_x { max_x } else { cursor_x };
            texture_start_x = texture_i;
            if ch == space {
                if wrap_width > 0.0 {
                    // the length of the next word, with this space
                    let mut word_width = if plant(9460) { 0.0 } else { char_spacing };
                    let mut j = i;
                    while at(j) != 0 && at(j) > space {
                        word_width += font.get_char_spacing(gdi, at(j)) as f32;
                        j += 1;
                    }
                    let wraps = if plant(9459) { cursor_x + word_width > wrap_width } else { cursor_x + word_width >= wrap_width };
                    if wraps {
                        cursor_x = 0.0;
                        cursor_y += char_height;
                        new_line(if plant(9482) { i } else { i - 1 });
                    }
                }
            } else if ch == newline {
                cursor_x = 0.0;
                cursor_y += char_height;
                if !plant(9483) {
                    new_line(i);
                }
            } else if ch == 0 {
                break;
            } else if word_bigger_then_line {
                cursor_x = 0.0;
                cursor_y += char_height;
                if !plant(9481) {
                    new_line(i - 1);
                }
            }
            if exceeded_texture_width && !plant(9466) {
                texture_start_x = TEXTURE_OFFSET;
                texture_i = texture_start_x;
                texture_j = (texture_j as f32 + char_height) as i32;
                if (texture_j as f32 + char_height) >= curr_texture_size as f32 {
                    // (the text left: after this character)
                    curr_texture_size = allocate_new_surface(font, gdi, text.get(i..).unwrap_or(&[]), texture_size_hint);
                    texture_i = 0;
                    texture_j = 0;
                    texture_start_x = 0;
                }
            }
        }
        if ch != newline || plant(9458) {
            // (`TextureOffset.I += char_spacing`: an int plus a float, truncated)
            texture_i = (texture_i as f32 + char_spacing) as i32;
        }
    }
    let extra = if plant(9461) { 0 } else { font.get_extra_overlap() };
    (max_x + extra as f32, cursor_y + char_height)
}

/// `W3DDisplayString::computeExtents` (W3DDisplayString.cpp:352), as `getSize` reads it: 0 by 0 for an empty string or
/// no font; else the formatted extents, each truncated to an `Int`.
pub fn display_string_size<G: FontGdi>(font: Option<&mut FontChars>, gdi: &mut G, text: &[u16], wrap_width: i32, hard_wrap: bool, texture_size_hint: i32) -> (i32, i32) {
    let Some(font) = font else { return (0, 0) };
    if text.is_empty() && !plant(9464) {
        return (0, 0);
    }
    let (x, y) = formatted_text_extents(font, gdi, text, wrap_width as f32, hard_wrap, texture_size_hint);
    (x as i32, y as i32)
}
