#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/render2dsentence.cpp (FontCharsClass::Create_GDI_Font,
//     Get_Char_Data, Get_Char_Spacing, Store_GDI_Char, Render2DSentenceClass::Build_Sentence_Not_Centered)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::render2d_sentence` (GitHub #153; the line starts #155) against render2dsentence.cpp.
//!
//! # Method
//!
//! The C++ side is render2dsentence.cpp's `FontCharsClass` measure (`Get_Char_Data` to `Create_GDI_Font`,
//! `Grow_Unicode_Array`) and `Build_Sentence_Not_Centered`, cut verbatim by build.rs, on a GDI of the sequence's answers
//! (`differential/wrap_render2d_sentence.cpp` documents the model and the words); the Rust side is the same GDI as a
//! [`FontGdi`]. Each sequence: a point size (1 to 48), bold or not, a face (Arial, `Generals`, Arial Unicode MS, Times New
//! Roman), the alternate Unicode font (none, itself, another), `TextureSizeHint` (0, 64, 128, 256, 512), the hard wrap on or off, each font's text metrics (height 1 to 40, ascent, overhang 0 to 3,
//! mostly 0) and its widths (0 to 40 by character mod 16), then 1 to 12 strings of 0 to 40 characters (letters, `W`,
//! spaces, newlines, Latin-1 and CJK characters) with a wrap width (0, or 1 to 300). Compared: each `CreateFont` (height,
//! width, weight, face), each font's overlap, height, ascent and overhang, and for each string the extents' float bits,
//! `computeExtents`' size and the first character's spacing.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches over 20,000 sequences: integer arithmetic held in floats, the same breaks.
//! - ~~The texture size does not change any result (the port does not model it).~~ **First run, a miss:** see the
//!   results.
//! - Plants 9451-9465 all caught: 9451 (the `Generals` width) by any `Generals` font; 9452 (overlap not capped) by any
//!   point size of 25 or more; 9453 (overlap a quarter) by any point size of 6 or more; 9454 (`Generals`' overhang kept)
//!   by a `Generals` font with an overhang; 9455 (no `W` origin) by any `W`; 9456 (no alternate font) by a CJK or Latin
//!   Extended character with another alternate; 9457 (overhang not subtracted) by any overhang; 9458 (a newline measured)
//!   by a newline; 9459 (`>` for `>=`) by a word that ends exactly at the wrap; 9460 (the space left out of the next
//!   word) by a wrap that it decides; 9461 (no extra overlap) by every font with an overlap; 9462 (no hard wrap) by a hard
//!   wrapped long word; 9463 (`MulDiv` truncated) by a point size whose 4/3 has a fraction of a half or more; 9464 (the
//!   empty string measured) by any empty string; 9465 (bold not asked) by any bold font.
//! - (Written after the first run, before the second.) 0 mismatches; 9466 (a chunk's end not resetting the offsets)
//!   caught, though by few sequences: only where the texture fills mid-line and a later negative spacing makes the
//!   reset matter.
//!
//! - (#155, written before the run.) The line starts (`formatted_text_line_starts`, new output of the same pass) hold
//!   for every string: as many lines as the C++'s height has rows, each start at a space, after a newline or (hard wrap)
//!   anywhere. Plants 9481-9483 caught by those checks, the measure itself unchanged.
//!
//! # Results (2026-11-12, `RENDER2D_CASES=20000`, the default)
//!
//! - **First run (port without the texture): 2 mismatches of 20,000**, sequences 5938 and 18738, the widest line 1
//!   pixel narrower in the port. The prediction that the texture cannot matter was wrong: with a negative spacing (an
//!   overhang wider than the glyph's extent, possible with the random GDI) the cursor goes back within a line, and
//!   upstream reads the widest line also where a texture chunk ends. The port now follows `Allocate_New_Surface`'s size
//!   and the chunk ends (plant 9466 added for it), and the C++ side cuts `Allocate_New_Surface` too (no stand-in size).
//! - Second run: 20,000 sequences, 130,005 strings (90,920 of more than one line): **0 mismatches**, as predicted after
//!   the first.
//! - Plants, mismatching sequences of 20,000: 9451 4,879; 9452 7,844; 9453 8,846; 9454 885; 9455 18,009; 9456 6,316;
//!   9457 2,811; 9458 15,557; 9459 728; 9460 8,179; 9461 17,897; 9462 5,148; 9463 6,757; 9464 10,489; 9465 9,997; 9466
//!   474. **16 of 16 caught**, as predicted. (`MulDiv` is Windows', not upstream's: the C++ side's is the stand-in's, so
//!   9463 checks the port against that model.)
//! - (#155) The line starts. **First run, a miss in the check:** 3,788 sequences failed, every one on two equal starts
//!   (a newline, then a space that wraps at once: an empty line, which upstream makes too, two moves down); the check
//!   asked for rising starts and now asks that they never go back. Second run: 130,005 strings, **0 failures**, the
//!   measure 0 mismatches. Plants: 9481 5,149 sequences; 9482 11,227; 9483 18,981. **3 of 3 caught.**
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 9451 | the `Generals` font is not narrowed (`fontWidth` 0) |
//! | 9452 | `PixelOverlap` is not capped at 4 |
//! | 9453 | `PixelOverlap` is a quarter of the height, not an eighth |
//! | 9454 | the `Generals` font keeps its overhang |
//! | 9455 | `W` is not drawn one pixel right (no `xOrigin`) |
//! | 9456 | a character above 255 is not measured in the alternate font |
//! | 9457 | `Get_Char_Spacing` does not subtract the overhang |
//! | 9458 | a newline advances the texture offset |
//! | 9459 | the space wrap uses `>` for `>=` |
//! | 9460 | the space's width is left out of the next word's |
//! | 9461 | the extents leave out `Get_Extra_Overlap` |
//! | 9462 | the hard wrap is ignored |
//! | 9463 | `MulDiv` truncates |
//! | 9464 | an empty string is measured |
//! | 9465 | a bold font is asked for at `FW_NORMAL` |
//! | 9466 | a texture chunk's end does not reset the texture's offsets |
//! | 9481 | (#155) a hard wrap's new line is not reported |
//! | 9482 | (#155) a space wrap's new line is reported one character late (after the space) |
//! | 9483 | (#155) a newline's new line is not reported |

mod support;

use rts_lockstep_engine::client::render2d_sentence::*;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const FACES: [&[u8]; 4] = [b"Arial", b"Generals", b"Arial Unicode MS", b"Times New Roman"];
const CHARS: [u16; 14] = [b'a' as u16, b'W' as u16, b'i' as u16, b'M' as u16, b' ' as u16, b' ' as u16, b'\n' as u16, b'x' as u16, b'0' as u16, 0xE9, 0x100, 0x4E00, 0x3042, b'&' as u16];

fn fnv(s: &[u8]) -> i32 {
    let mut h: u32 = 2_166_136_261;
    for &b in s {
        h ^= u32::from(b);
        h = h.wrapping_mul(16_777_619);
    }
    h as i32
}

/// The wrapper's GDI: fonts numbered in creation order, each call recorded.
struct Gdi<'a> {
    metrics: [[i32; 3]; 2],
    widths: &'a [i32],
    fonts: usize,
    out: Vec<i32>,
}

impl FontGdi for Gdi<'_> {
    fn create_font(&mut self, height: i32, width: i32, weight: i32, face: &[u8]) -> (usize, TextMetric) {
        self.out.extend([1, height, width, weight, fnv(face)]);
        let id = self.fonts.min(1);
        self.fonts += 1;
        let m = self.metrics[id];
        (id, TextMetric { height: m[0], ascent: m[1], overhang: m[2] })
    }
    fn text_extent(&mut self, font: usize, ch: u16) -> (i32, i32) {
        (self.widths[font * 16 + usize::from(ch % 16)], self.metrics[font][0])
    }
}

/// (#155) Each string's line starts ([`formatted_text_line_starts`]) checked against the C++'s extents: as many lines as
/// the height's rows (`(starts + 1) x char_height`, the C++'s float bits), the starts never going back, each at a space, after a newline, or (hard wrap on) anywhere. The number of strings checked, or the first failure.
fn line_starts_hold(input: &[i32], c: &[i32]) -> Result<u64, String> {
    let (point, bold, face, alt, hard) = (input[0], input[1] != 0, input[2] as usize, input[3], input[5] != 0);
    let m = &input[8..14];
    let mut g = Gdi { metrics: [[m[0], m[1], m[2]], [m[3], m[4], m[5]]], widths: &input[14..46], fonts: 0, out: Vec::new() };
    let mut font = FontChars::new(&mut g, FACES[face], point, bold);
    if alt == 1 {
        font.alternate = Alternate::SelfFont;
    }
    if alt == 2 {
        font.alternate = Alternate::Other(Box::new(FontChars::new(&mut g, b"Arial Unicode MS", point, bold)));
    }
    let fonts = if alt == 2 { 2 } else { 1 };
    let records = &c[10 * fonts..];
    let (mut k, mut n) = (46, 0usize);
    while k + 2 <= input.len() {
        let (wrap, len) = (input[k], input[k + 1] as usize);
        k += 2;
        let text: Vec<u16> = input[k..k + len].iter().map(|&c| c as u16).collect();
        k += len;
        let starts = formatted_text_line_starts(&mut font, &mut g, &text, wrap as f32, hard, input[4]);
        let rows = (starts.len() + 1) as f32 * font.get_char_height() as f32;
        let y = records.get(n * 6 + 2).map(|&b| f32::from_bits(b as u32));
        if y != Some(rows) {
            return Err(format!("string {n}: {} lines ({rows} high), the C++'s height {y:?}", starts.len() + 1));
        }
        // (two starts at one character are an empty line: a newline, then a space wrap at the next line's first
        // character, moves down twice)
        if !starts.windows(2).all(|w| w[0] <= w[1]) {
            return Err(format!("string {n}: the starts {starts:?} go back"));
        }
        for &s in &starts {
            let at_space = text.get(s) == Some(&u16::from(b' '));
            let after_newline = s > 0 && text.get(s - 1) == Some(&u16::from(b'\n'));
            if !(at_space || after_newline || hard) {
                return Err(format!("string {n}: a line starts at {s} in {text:?}"));
            }
        }
        n += 1;
    }
    Ok(n as u64)
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let (point, bold, face, alt, hard) = (input[0], input[1] != 0, input[2] as usize, input[3], input[5] != 0);
    let m = &input[8..14];
    let mut g = Gdi { metrics: [[m[0], m[1], m[2]], [m[3], m[4], m[5]]], widths: &input[14..46], fonts: 0, out: Vec::new() };
    let mut font = FontChars::new(&mut g, FACES[face], point, bold);
    if alt == 1 {
        font.alternate = Alternate::SelfFont;
    }
    if alt == 2 {
        font.alternate = Alternate::Other(Box::new(FontChars::new(&mut g, b"Arial Unicode MS", point, bold)));
    }
    g.out.extend([2, font.pixel_overlap, font.char_height, font.char_ascent, font.char_overhang]);
    if let Alternate::Other(o) = &font.alternate {
        let o = o.clone();
        g.out.extend([2, o.pixel_overlap, o.char_height, o.char_ascent, o.char_overhang]);
    }
    let mut k = 46;
    while k + 2 <= input.len() {
        let (wrap, len) = (input[k], input[k + 1] as usize);
        k += 2;
        let text: Vec<u16> = input[k..k + len].iter().map(|&c| c as u16).collect();
        k += len;
        let (x, y) = formatted_text_extents(&mut font, &mut g, &text, wrap as f32, hard, input[4]);
        let (sx, sy) = display_string_size(Some(&mut font), &mut g, &text, wrap, hard, input[4]);
        let first = if len > 0 { font.get_char_spacing(&mut g, text[0]) } else { 0 };
        g.out.extend([3, x.to_bits() as i32, y.to_bits() as i32, sx, sy, first]);
    }
    g.out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 100_000];
    let n = unsafe { cpp::ref_render2d_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let point = 1 + g.below(48) as i32;
    let mut v = vec![point, g.below(2) as i32, g.below(4) as i32, g.below(3) as i32, [0, 0, 64, 128, 256, 512][g.below(6) as usize], i32::from(g.below(3) == 0), 0, 0];
    for _ in 0..2 {
        let h = 1 + g.below(40) as i32;
        v.extend([h, g.below(h as u32 + 1) as i32, if g.below(4) == 0 { g.below(4) as i32 } else { 0 }]);
    }
    for _ in 0..32 {
        v.push(if g.below(16) == 0 { 0 } else { g.below(41) as i32 });
    }
    for _ in 0..1 + g.below(12) {
        let wrap = if g.below(3) == 0 { 0 } else { 1 + g.below(300) as i32 };
        let len = if g.below(10) == 0 { 0 } else { g.below(41) };
        v.extend([wrap, len as i32]);
        for _ in 0..len {
            v.push(i32::from(CHARS[g.below(CHARS.len() as u32) as usize]));
        }
    }
    v
}

#[test]
fn the_text_measure_matches_upstream() {
    let cases: u64 = std::env::var("RENDER2D_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x153_0001);
    let mut whole = Check::new("the text measure");
    let mut strings = 0u64;
    let mut wrapped = 0u64;
    let (mut lines_checked, mut lines_failed) = (0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        // (the records: 5 words per font made, 5 per font state, then 6 per string)
        let fonts = if input[3] == 2 { 2 } else { 1 };
        let height = r[5 * fonts + 2];
        for s in r[10 * fonts..].chunks(6) {
            strings += 1;
            wrapped += u64::from(s.len() == 6 && s[4] > height);
        }
        if r != c && !planted {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(8);
            println!("sequence {i} first difference at word {k}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 8).min(r.len())], &c[k0.min(c.len())..(k + 8).min(c.len())]);
        }
        // (#155) the line starts, against the C++'s heights (a plant changing the measure itself shows above)
        match line_starts_hold(&input, &c) {
            Ok(n) => lines_checked += n,
            Err(e) => {
                lines_failed += 1;
                if lines_failed <= 3 {
                    println!("sequence {i}: line starts: {e}");
                }
            }
        }
        whole.eq(r, c, || format!("sequence {i}: {input:?}"));
    }
    println!("{cases} sequences, about {strings} strings measured ({wrapped} of more than one line)");
    println!("line starts: {lines_checked} strings held, {lines_failed} sequences failed");
    assert!(planted || lines_failed == 0, "{lines_failed} sequences' line starts disagree with the C++'s heights");
    if planted && lines_failed > 0 {
        // (a plant in the line starts: caught by them, the measure itself unchanged)
        println!("PLANTED line starts caught in {lines_failed} sequences");
        whole.report();
    } else if planted {
        whole.finish_planted();
    } else {
        whole.finish();
    }
}
