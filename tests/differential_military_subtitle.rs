#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (militarySubtitle, removeMilitarySubtitle, update's and
//     postDraw's military subtitle, disableTooltipsUntil, clearTooltipsDisabled, areTooltipsDisabled)
//   GeneralsMD/Code/GameEngine/Include/GameClient/InGameUI.h (MilitarySubtitleData)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::military_subtitle` (GitHub #151) against InGameUI.cpp.
//!
//! # Method
//!
//! The C++ side is InGameUI.cpp's `militarySubtitle`, `removeMilitarySubtitle`, the tooltip switch, and the blocks of
//! `update` and `postDraw` that work the subtitle, cut verbatim by build.rs, as members of a stand-in `InGameUI`
//! (`differential/wrap_military_subtitle.cpp` documents the model and the words); the Rust side is
//! [`MilitarySubtitleUi`] with the same model as its [`MilitarySubtitleHost`]. Each sequence has a configuration: the
//! display (800x600, a phone's 390x844, 1024x768, 1920x1080, or any size from 1 to 2000), the language's
//! `MilitaryCaptionDelayMS` (0 to 2,000, sometimes negative) and `MilitaryCaptionSpeed` (0 to 4), InGameUI.ini's speed,
//! the caption's position, colour and fonts' sizes and bolds. Then steps: a caption made (eight labels: one line, two,
//! four, five (undefined: see below), the empty one, leading new lines, a long line, a trailing new line; durations
//! from -100 to 20,000 ms, 0 among them, sometimes over 70 million), the logic frame moved (mostly one frame, sometimes
//! none or far, sometimes frozen by a script; ~~sometimes back~~, corrected 2026-11-12 before the results were taken:
//! see the results), the update, the draw, the removal. Compared after each step: every call
//! (the typing sound, the briefing history, each string drawn with its place, text, font and colours, the block's
//! rectangle) and the subtitle's whole state (index, line, lifetime, block, timer, colour, position, each line's text and
//! font), the tooltips' frame and whether they are off.
//!
//! **Undefined upstream:** a fifth line (`currentDisplayString` 4) is read past `displayStrings` by the draw and the
//! removal, and a character typed past the text's end reads past the string; the Rust reports either
//! ([`MilitarySubtitleUi::undefined`]) and the sequence stops before the update that met it, on both sides.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches over 20,000 sequences. If any, most likely the float conversions: the position's `Int * Real`
//!   truncation, `(Int)(30.0f * ms / 1000.0f)` for large durations (float rounding before the division), and the fade's
//!   `REAL_TO_INT` of an unsigned difference.
//! - Plants 9431-9438 all caught: 9431 (the removal keeps the tooltips off) by any removal of a live caption; 9432 (a 0 or
//!   negative duration makes a caption) by those durations, one in eight; 9433 (no first delay) by any caption with a
//!   delay of a frame or more; 9434 (frozen time ignored) by a frozen update; 9435 (the fade twice as fast) by any fade;
//!   9436 (the block toggles after 9 frames, not 10) by any caption updated 10 frames; 9437 (the language's speed
//!   ignored) when the two speeds differ; 9438 (the block 0.5 wide) by any draw with the block on.
//!
//! # Results (2026-11-12, `SUBTITLE_CASES=20000`, the default)
//!
//! - **First run: 12 sequences differ. Missed: the prediction named the float conversions; neither cause was one.**
//!   (1) A translation defect: a character typed at the text's end (`getCharAt(length)`, the terminator, 0) was pushed as
//!   a 0 unit; upstream's `appendChar(0)` is `UnicodeString::concat` of `{ 0, 0 }`, which appends nothing. Fixed (the
//!   0 is not appended). (2) The harness's: frames went back and below 0, so the unsigned frame passed the lifetime while
//!   the signed one did not (`(Int)lifetime < (Int)currLogicFrame` is the fade, `incrementOnFrame < currLogicFrame` the
//!   typing), and the typing read past the text, undefined upstream. A game's frame starts at 0 and only goes up: the
//!   frames now never go back, and the Rust records a read past the text as undefined (none met since).
//! - Third run: 20,000 sequences, 4,394,155 steps, 175,616 captions made, 46,640 characters typed, 236,437 blocks and
//!   309,351 strings drawn; 144 stopped at a fifth line (undefined); **0 mismatches**.
//! - Plants, sequences that differ of 20,000: 9431 17,911; 9432 15,340; 9433 16,631; 9434 15,386; 9435 3,604; 9436 9,738;
//!   9437 6,961; 9438 18,231. **8 of 8 caught**, as predicted. (A planted Rust can let the C++ reach a fifth line the
//!   Rust did not stop at: the wrapper ends a sequence there rather than read past the array.)
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 9431 | `clearTooltipsDisabled` does nothing |
//! | 9432 | a duration of 0 or less makes a caption |
//! | 9433 | the first character is not delayed |
//! | 9434 | a script's frozen time is ignored |
//! | 9435 | the fade takes twice the amount |
//! | 9436 | the block toggles one frame early |
//! | 9437 | InGameUI.ini's speed replaces the language's |
//! | 9438 | the block is half its height wide, not 0.8 |

mod support;

use rts_lockstep_engine::client::military_subtitle::*;
use rts_lockstep_engine::client::radius_cursor::InGameUiIniSettings;
use rts_lockstep_engine::client::window_script::GameFont;
use rts_lockstep_engine::common::base_type::{ICoord2D, RGBAColorInt};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const LABELS: [&[u8]; 8] = [b"Hi", b"Two\nlines", b"A\nB\nC\nD", b"1\n2\n3\n4\n5", b"", b"\n\nX", b"Long caption text typed out", b"Ends\n"];

fn fnv_units(t: &[u16]) -> i32 {
    let mut h: u32 = 2166136261;
    for &u in t {
        h ^= u32::from(u);
        h = h.wrapping_mul(16777619);
    }
    h as i32
}

fn fnv_font(f: &GameFont) -> i32 {
    let mut h: u32 = 2166136261;
    for &b in &f.name {
        h ^= u32::from(b);
        h = h.wrapping_mul(16777619);
    }
    h ^= f.point_size as u32;
    h = h.wrapping_mul(16777619);
    h ^= u32::from(f.bold);
    h = h.wrapping_mul(16777619);
    h as i32
}

/// The wrapper's model.
struct Model {
    frame: u32,
    frozen: bool,
    size: (i32, i32),
    delay: i32,
    speed: i32,
    out: Vec<i32>,
    /// What was exercised: typing sounds, blocks drawn, strings drawn.
    stats: [u64; 3],
}

impl MilitarySubtitleHost for Model {
    fn logic_frame(&self) -> u32 {
        self.frame
    }
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        label.iter().map(|&b| (b as i8) as i16 as u16).collect()
    }
    fn display_size(&self) -> (i32, i32) {
        self.size
    }
    fn military_caption_delay_ms(&self) -> i32 {
        self.delay
    }
    fn language_military_caption_speed(&self) -> Option<i32> {
        Some(self.speed)
    }
    fn adjust_font_size(&self, size: i32) -> i32 {
        size + 2
    }
    fn string_size(&self, text: &[u16], font: &GameFont) -> (i32, i32) {
        (text.len() as i32 * (font.point_size / 2 + 1), font.point_size + 1)
    }
    fn font_height(&self, font: &GameFont) -> i32 {
        font.point_size
    }
    fn add_audio_event(&mut self, _name: &[u8]) {
        self.stats[0] += 1;
        self.out.push(1);
    }
    fn is_time_frozen_script(&self) -> bool {
        self.frozen
    }
    fn update_diplomacy_briefing_text(&mut self, label: &[u8]) {
        let li = LABELS.iter().rposition(|l| *l == label).map_or(-1, |i| i as i32);
        self.out.extend([2, li]);
    }
}

/// The Rust run of `input`; the steps it ran (it stops before one that meets the undefined fifth line).
fn rust_seq(input: &[i32]) -> (Vec<i32>, usize, [u64; 3], Option<&'static str>) {
    let c = &input[..16];
    let mut s = InGameUiIniSettings::default();
    s.military_caption_speed = c[4];
    s.military_caption_position = ICoord2D { x: c[5], y: c[6] };
    s.military_caption_color = RGBAColorInt { red: c[7] as u32, green: c[8] as u32, blue: c[9] as u32, alpha: c[10] as u32 };
    s.military_caption_title_font = b"Courier".to_vec();
    s.military_caption_title_point_size = c[11];
    s.military_caption_title_bold = c[13] != 0;
    s.military_caption_font = b"Arial".to_vec();
    s.military_caption_point_size = c[12];
    s.military_caption_bold = c[14] != 0;
    let mut m = Model { frame: 0, frozen: false, size: (c[0], c[1]), delay: c[2], speed: c[3], out: Vec::new(), stats: [0; 3] };
    let mut u = MilitarySubtitleUi::default();
    let mut ran = 0;
    let mut reason = None;
    for st in input[16..].chunks(4) {
        if st.len() < 4 {
            break;
        }
        let before = (0, m.out.len());
        match st[0] {
            0 => u.military_subtitle(&mut m, &s, LABELS[st[1] as usize % 8], st[2]),
            1 => {
                m.frame = st[1] as u32;
                m.frozen = st[2] != 0;
            }
            2 => u.update(&mut m, &s),
            3 => {
                for d in u.draw(&m) {
                    match d {
                        SubtitleDraw::Text { x, y, text, font, color, drop_color } => {
                            m.stats[2] += 1;
                            m.out.extend([3, x, y, fnv_units(&text), fnv_font(&font), color, drop_color]);
                        }
                        SubtitleDraw::FillRect { x, y, width, height, color } => {
                            m.stats[1] += 1;
                            m.out.extend([4, x, y, width, height, color]);
                        }
                    }
                }
            }
            _ => u.remove_military_subtitle(),
        }
        if u.undefined.is_some() {
            // (stop before this step: undefined upstream)
            reason = u.undefined;
            m.out.truncate(before.1);
            break;
        }
        m.out.extend([100, i32::from(u.military_subtitle.is_some())]);
        if let Some(d) = &u.military_subtitle {
            m.out.extend([d.index as i32, d.current_display_string as i32, d.lifetime as i32, i32::from(d.block_drawn), d.block_begin_frame as i32, d.block_pos.x, d.block_pos.y, d.increment_on_frame as i32, d.color, d.position.x, d.position.y]);
            for l in d.display_strings.iter().take(d.current_display_string as usize + 1).flatten() {
                m.out.extend([fnv_units(&l.text), fnv_font(&l.font)]);
            }
        }
        m.out.extend([u.tooltips_disabled_until as i32, i32::from(u.are_tooltips_disabled(m.frame))]);
        ran += 1;
    }
    (m.out, ran, m.stats, reason)
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 400_000];
    let n = unsafe { cpp::ref_military_subtitle_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let (w, h) = match g.below(6) {
        0 | 1 => (800, 600),
        2 => (390, 844),
        3 => (1024, 768),
        4 => (1920, 1080),
        _ => (1 + g.below(2000) as i32, 1 + g.below(2000) as i32),
    };
    let delay = match g.below(10) {
        0 => 0,
        1 => -(g.below(500) as i32),
        _ => g.below(2001) as i32,
    };
    let mut v = vec![w, h, delay, g.below(5) as i32, g.below(4) as i32, g.below(801) as i32, g.below(601) as i32];
    for _ in 0..4 {
        v.push(g.below(256) as i32);
    }
    v.extend([6 + g.below(15) as i32, 6 + g.below(15) as i32, g.below(2) as i32, g.below(2) as i32, 0]);
    let mut frame: i64 = g.below(1000) as i64;
    let mut frozen = 0;
    v.extend([1, frame as i32, 0, 0]);
    let n = 20 + g.below(400);
    for _ in 0..n {
        let s = match g.below(100) {
            0..=3 => {
                let dur = match g.below(8) {
                    0 => 0,
                    1 => -(g.below(100) as i32),
                    2 => 70_000_000 + g.below(1_000_000_000) as i32,
                    _ => g.below(20_001) as i32,
                };
                [0, g.below(8) as i32, dur, 0]
            }
            4 => [4, 0, 0, 0],
            5..=59 => {
                // a frame (mostly the next); the update and the draw follow
                // (a game's logic frame only goes up, from 0: see the results)
                frame += match g.below(40) {
                    0 => 0,
                    1 => g.below(5000) as i64,
                    _ => i64::from(frozen == 0),
                };
                if g.below(30) == 0 {
                    frozen = 1 - frozen;
                }
                [1, frame as i32, frozen, 0]
            }
            60..=84 => [2, 0, 0, 0],
            _ => [3, 0, 0, 0],
        };
        v.extend(s);
    }
    v
}

#[test]
fn the_military_subtitle_matches_upstream() {
    let cases: u64 = std::env::var("SUBTITLE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x151_0003);
    let mut whole = Check::new("the military subtitle");
    let (mut steps, mut stopped, mut captions) = (0u64, 0u64, 0u64);
    let mut stats = [0u64; 3];
    let mut reasons = std::collections::BTreeMap::new();
    for i in 0..cases {
        let mut input = gen_case(&mut g);
        let all = (input.len() - 16) / 4;
        let (r, ran, st, reason) = rust_seq(&input);
        if let Some(why) = reason {
            *reasons.entry(why).or_insert(0u64) += 1;
        }
        for k in 0..3 {
            stats[k] += st[k];
        }
        if ran < all {
            stopped += 1;
            input.truncate(16 + 4 * ran);
        }
        steps += ran as u64;
        captions += input[16..].chunks(4).filter(|s| s[0] == 0).count() as u64;
        let c = cpp_seq(&input);
        if r != c && !planted {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(12);
            println!("sequence {i} first difference at word {k}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 8).min(r.len())], &c[k0.min(c.len())..(k + 8).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}: {input:?}"));
    }
    println!("{cases} sequences, {steps} steps, {captions} captions made, {stopped} stopped where upstream is undefined (a fifth line, or a character past the text); {} characters typed, {} blocks and {} strings drawn", stats[0], stats[1], stats[2]);
    println!("  stopped: {reasons:?}");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
