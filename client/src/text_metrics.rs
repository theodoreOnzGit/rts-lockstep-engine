// SPDX-License-Identifier: GPL-3.0-only
//! (#153) The client's text measure: upstream's `DisplayString::getSize` and `winFontHeight`
//! (`rts_lockstep_engine::client::render2d_sentence`, translated and checked against the C++) over an estimate of what
//! Windows' GDI answers for the game's fonts ([`ArialEstimate`], new). Plain Rust, no Bevy.
//!
//! Upstream asks GDI for each character's width in the font at its pixel size (`CreateFont` at `MulDiv(points, 96, 72)`,
//! `GetTextExtentPoint32W`) and for the font's height (`GetTextMetrics`). The client has no Windows fonts, so it answers
//! from Arial's published metrics instead: each printable ASCII character's advance width as Helvetica's AFM files give
//! it (Arial and Liberation Sans are made to the same widths), regular and bold, scaled to the pixel size and rounded;
//! the height Arial's ascent plus descent (1854 + 434 of 2048 units) at that size. Every face is measured as Arial
//! (upstream's `Generals` face is Arial narrowed, which the translated `Create_GDI_Font` asks for; Times New Roman and
//! the others are not told apart). A character outside printable ASCII is 556 units wide, or 1000 for a CJK or
//! full-width one.
//!
//! **How close (2026-11-12, #153).** Not checked against GDI (no Windows here): GDI's hinted widths at a small pixel
//! size can differ from the scaled AFM width by a pixel a character, and the face the owner's `Language.ini` names may
//! not be Arial. The measure's structure (the overlap, the `W`, the wrap, the alternate Unicode font, the extra overlap)
//! is upstream's, differential-tested (`tests/differential_render2d.rs`). Against the estimate it replaces: "Single
//! Player" in a 12-point font measured 93 pixels by 12 with 0.6 of the point size a character (the subtitle's), 78 by
//! 12 with half the point size (the shell's and the bar's), and measures 98 by 18 here (`the_measure_is_arials`).
//! ~~What the client draws is Bevy's own font unless it has the named face ([`draw_face`]), so the drawn text can still be
//! wider or narrower than its measured box.~~ **Corrected 2026-11-12, #155:** the client draws every face in Liberation
//! Sans (`crate::fonts`, shipped with it: Arial's metrics), regular or bold, line by line where this measure breaks the
//! lines ([`text_lines`]), each line the font's height ([`font_height`]) below the last, as render2dsentence.cpp lays
//! them out. Arial's widths are Liberation Sans' widths, so the drawn text fits its measured box but for the measure's
//! rounding of each character to a whole pixel (the box's extra overlap, 2 pixels at 12 points, takes it up). A face
//! that is not Arial (Times New Roman, the narrowed `Generals`) is drawn in Liberation Sans all the same, an
//! approximation: the measure is Arial's too, so box and text agree with each other but not with upstream's face.

use rts_lockstep_engine::client::render2d_sentence::{display_string_size, mul_div, Alternate, FontChars, FontGdi, TextMetric, FW_BOLD};
use rts_lockstep_engine::client::window_manager::TextMetrics;
use rts_lockstep_engine::client::window_script::GameFont;

/// Helvetica's advance widths (thousandths of the em) of the characters 32 to 126, regular.
const WIDTHS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // space to /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 0 to ?
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // @ to O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // P to _
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // ` to o
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // p to ~
];

/// The same, bold.
const BOLD_WIDTHS: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, // space to /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, // 0 to ?
    975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, // @ to O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556, // P to _
    333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, // ` to o
    611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584, // p to ~
];

/// Arial's ascent, descent and average character width (`xAvgCharWidth`), in its 2048 units to the em.
const ASCENT: i64 = 1854;
const DESCENT: i64 = 434;
const AVERAGE_WIDTH: i64 = 904;

/// A character's advance in thousandths of the em.
fn advance(ch: u16, bold: bool) -> i64 {
    match ch {
        32..=126 => i64::from(if bold { BOLD_WIDTHS } else { WIDTHS }[usize::from(ch - 32)]),
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFF00..=0xFF60 => 1000,
        0..=31 => 0,
        _ => 556,
    }
}

/// `n / d` rounded half up (both positive).
fn round_div(n: i64, d: i64) -> i64 {
    (n + d / 2) / d
}

/// New: GDI's answers for the game's fonts, estimated from Arial's metrics (see the module's doc). A font is its pixel
/// height, its width (0: the face's own), and bold.
#[derive(Clone, Debug, Default)]
pub struct ArialEstimate {
    fonts: Vec<(i64, i64, bool)>,
}

impl FontGdi for ArialEstimate {
    fn create_font(&mut self, height: i32, width: i32, weight: i32, _face: &[u8]) -> (usize, TextMetric) {
        let em = i64::from(height).abs();
        self.fonts.push((em, i64::from(width).max(0), weight >= FW_BOLD));
        let tm = TextMetric { height: round_div((ASCENT + DESCENT) * em, 2048) as i32, ascent: round_div(ASCENT * em, 2048) as i32, overhang: 0 };
        (self.fonts.len() - 1, tm)
    }

    fn text_extent(&mut self, font: usize, ch: u16) -> (i32, i32) {
        let (em, width, bold) = self.fonts[font];
        let a = advance(ch, bold);
        // (a width given narrows every glyph by it over the face's average width)
        let cx = if width > 0 { round_div(a * width * 2048, 1000 * AVERAGE_WIDTH) } else { round_div(a * em, 1000) };
        (cx as i32, round_div((ASCENT + DESCENT) * em, 2048) as i32)
    }
}

/// The font a window's text is measured and drawn in: its own, else (no language data) Arial at 12 points.
fn font_or_default(font: Option<&GameFont>) -> (Vec<u8>, i32, bool) {
    match font {
        Some(f) if !f.name.is_empty() => (f.name.clone(), f.point_size, f.bold),
        Some(f) => (b"Arial".to_vec(), f.point_size, f.bold),
        None => (b"Arial".to_vec(), 12, false),
    }
}

/// `TheFontLibrary->getFont(name, size, bold)` as `W3DFontLibrary::loadFontData` makes it: the font, with the
/// alternate Unicode font of the same size (`Arial Unicode MS`, the language's default `UnicodeFontName`; itself when it
/// is that font).
fn load_font(gdi: &mut ArialEstimate, font: Option<&GameFont>) -> FontChars {
    let (name, size, bold) = font_or_default(font);
    let mut f = FontChars::new(gdi, &name, size, bold);
    let unicode = b"Arial Unicode MS";
    f.alternate = if name.eq_ignore_ascii_case(unicode) { Alternate::SelfFont } else { Alternate::Other(Box::new(FontChars::new(gdi, unicode, size, bold))) };
    f
}

/// `DisplayString::getSize` of `text` in `font`, word wrapped at `wrap` pixels when given (`setWordWrap`), the hard
/// wrap off.
pub fn text_size(font: Option<&GameFont>, text: &[u16], wrap: Option<i32>) -> (i32, i32) {
    let mut gdi = ArialEstimate::default();
    let mut f = load_font(&mut gdi, font);
    display_string_size(Some(&mut f), &mut gdi, text, wrap.unwrap_or(0).max(0), false, 0)
}

/// `winFontHeight(font)`: `GameFont::height`, the text metrics' height (`Get_Char_Height`).
pub fn font_height(font: Option<&GameFont>) -> i32 {
    let mut gdi = ArialEstimate::default();
    load_font(&mut gdi, font).get_char_height()
}

/// The font's em in pixels as GDI makes it (`Create_GDI_Font`'s `MulDiv(PointSize, 96, 72)`): the size the client
/// draws the text at (Bevy's font size is the em).
pub fn em_px(font: Option<&GameFont>) -> f32 {
    mul_div(font_or_default(font).1, 96, 72) as f32
}

/// ~~The face the client asks Bevy for: the font's name (`Generals` is Arial, as `Create_GDI_Font` draws it). Bevy falls
/// back to its own font when it does not have the face.~~ **Corrected 2026-11-12, #155:** the face the client draws a
/// font in: Liberation Sans, bold for a bold font, whatever the font's name (see the module's doc).
pub fn draw_face(font: Option<&GameFont>) -> crate::fonts::Face {
    let (_, _, bold) = font_or_default(font);
    if bold {
        crate::fonts::Face::Bold
    } else {
        crate::fonts::Face::Regular
    }
}

/// (#155) `text` as the lines `DisplayString::getSize`'s pass breaks it into ([`text_size`]'s measure, word wrapped at
/// `wrap` pixels when given): each line from where the pass starts it (`formatted_text_line_starts`), a newline that ends
/// it left out, so the client draws exactly the measured lines (a line a space wrap starts begins with that space, as
/// upstream's does). An empty text is no line.
pub fn text_lines(font: Option<&GameFont>, text: &[u16], wrap: Option<i32>) -> Vec<Vec<u16>> {
    use rts_lockstep_engine::client::render2d_sentence::formatted_text_line_starts;
    if text.is_empty() {
        return Vec::new();
    }
    let mut gdi = ArialEstimate::default();
    let mut f = load_font(&mut gdi, font);
    // (`computeExtents` hands the pass the text as the display string holds it: it ends at a 0)
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    let text = &text[..end];
    let starts = formatted_text_line_starts(&mut f, &mut gdi, text, wrap.unwrap_or(0).max(0) as f32, false, 0);
    let mut lines = Vec::with_capacity(starts.len() + 1);
    let mut from = 0;
    for s in starts.into_iter().chain([text.len()]) {
        let mut line = &text[from.min(s)..s];
        if line.last() == Some(&u16::from(b'\n')) {
            line = &line[..line.len() - 1];
        }
        lines.push(line.to_vec());
        from = s;
    }
    lines
}

/// (#155) [`text_lines`] as one string, the lines joined by newlines (for a text node that does not wrap).
pub fn text_lines_string(font: Option<&GameFont>, text: &[u16], wrap: Option<i32>) -> String {
    let lines: Vec<String> = text_lines(font, text, wrap).iter().map(|l| String::from_utf16_lossy(l)).collect();
    lines.join("\n")
}

/// (#155) The line height the client draws a font's lines at, in pixels: [`font_height`] (render2dsentence.cpp moves
/// each line down by `Get_Char_Height`, the text metrics' height), not Bevy's 1.2 em.
pub fn line_px(font: Option<&GameFont>) -> f32 {
    font_height(font) as f32
}

/// The measure given to the shell's window manager ([`rts_lockstep_engine::client::shell_gui::ShellGui::text_metrics`]).
pub const METRICS: TextMetrics = TextMetrics { text_size, font_height };

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    fn font(name: &str, size: i32, bold: bool) -> GameFont {
        GameFont { name: name.as_bytes().to_vec(), point_size: size, bold }
    }

    /// #153: the measure, predicted by hand from the widths and render2dsentence.cpp before the run:
    ///
    /// - Arial 12: the em `MulDiv(12, 96, 72)` = 16 pixels, the height round(2288 x 16 / 2048) = 18 (`winFontHeight`),
    ///   the overlap 16 / 8 = 2;
    /// - "Single Player": S 11, i 4, n 9, g 9, l 4, e 9, space 4, P 11, l 4, a 9, y 8, e 9, r 5 pixels (each AFM width x
    ///   16 / 1000, rounded) = 96, plus the extra overlap 2: 98 by 18;
    /// - `W` is one pixel wider than its 15 (the `xOrigin`); "Wi" is 15 + 1 + 4 = 20, plus 2: 22;
    /// - wrapped at 50: "Single Player" breaks at the space ("Single" 46 + the space and "Player", 4 + 46, >= 50): two lines, 18 x
    ///   2 = 36 high; the widest line is the second, the space's 4 with "Player"'s 46 = 50, plus 2: 52;
    /// - `Generals` 12 is narrowed: `fontWidth` 6 (0.40 x 16, truncated), each width scaled by 6 x 2048 / 904 / 1000:
    ///   "Single Player" 83 + 2 = 85 wide (its overhang 0);
    /// - an empty string and a missing font name measure 0 by 0 and Arial 12;
    /// - the em drawn is 16 pixels; `Generals` is drawn as Arial. (~~as Arial~~ **#155:** in Liberation Sans, bold for a
    ///   bold font, as every face; "Single Player" wrapped at 50 is drawn as "Single" and " Player"; "a\nb\n" as "a", "b"
    ///   and an empty third line, as the measure's three rows; an empty text as no line; the lines 18 apart.)
    #[test]
    fn the_measure_is_arials() {
        let a12 = font("Arial", 12, false);
        assert_eq!(font_height(Some(&a12)), 18);
        assert_eq!(text_size(Some(&a12), &u("Single Player"), None), (98, 18));
        assert_eq!(text_size(Some(&a12), &u("Wi"), None), (22, 18));
        assert_eq!(text_size(Some(&a12), &u("Single Player"), Some(50)), (52, 36));
        assert_eq!(text_size(Some(&font("Generals", 12, false)), &u("Single Player"), None), (85, 18));
        assert_eq!(text_size(Some(&a12), &[], None), (0, 0));
        assert_eq!(text_size(None, &u("Wi"), None), (22, 18));
        assert_eq!(em_px(Some(&a12)), 16.0);
        // (#155: every face in Liberation Sans, bold for a bold font; the lines as measured)
        assert_eq!(draw_face(Some(&font("Generals", 10, true))), crate::fonts::Face::Bold);
        assert_eq!(draw_face(Some(&font("Times New Roman", 10, false))), crate::fonts::Face::Regular);
        assert_eq!(draw_face(None), crate::fonts::Face::Regular);
        assert_eq!(text_lines(Some(&a12), &u("Single Player"), Some(50)), vec![u("Single"), u(" Player")]);
        assert_eq!(text_lines_string(Some(&a12), &u("Single Player"), None), "Single Player");
        assert_eq!(text_lines_string(Some(&a12), &u("a\nb\n"), None), "a\nb\n");
        assert!(text_lines(Some(&a12), &[], None).is_empty());
        assert_eq!(line_px(Some(&a12)), 18.0);
    }
}
