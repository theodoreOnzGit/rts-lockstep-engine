#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Mouse.cpp (the tooltip)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Color.h (GameMakeColor)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::mouse::MouseTooltip` (GitHub #124) against the C++.
//!
//! # Method
//!
//! The C++ side is Mouse.cpp's constructor, `init` (with `mouseNotifyResolutionChange`), `createStreamMessages` (whose
//! tooltip timer is under test; the messages it appends are not compared), `setCursorTooltip`, `resetTooltipDelay` and
//! `drawTooltip`, cut verbatim by build.rs (`differential/wrap_mouse_tooltip.cpp` lists the cuts and the stand-ins).
//! Both sides measure the display string the same way (6 pixels a character, 13 a line, wrapped at its word wrap).
//!
//! Each case is a random `Mouse` block (font size and boldness, `TooltipAnimateBackground`, `TooltipFillTime` 1..200 or
//! negative, `TooltipDelayTime` -5..300, the five colours, `TooltipWidth` among 15 %, 0, 100 %, random to 300 %, huge and
//! negative, the three alternate-colour switches), with or without a language tooltip font and a display width, then 1
//! to 60 steps on a clock that starts anywhere (wrapping past 2^32 too) and moves 0..150 ms a step: `setCursorTooltip`
//! of six texts (empty, short, two lines, sixty characters, one character, the short one again) with a delay of -1, 0
//! or random, no colour, `setFromInt`'s colour or reals from -1 to 2, and widths 1, 1.5, 0.5, 0 or random;
//! `resetTooltipDelay`; `createStreamMessages` with 0..4 events, any of them moving, with and without script debugging;
//! `drawTooltip` at random positions near and past the limits, with and without a fade; `mouseNotifyResolutionChange`;
//! a new display width. Both sides write the tooltip's state after the setup and after each step, and each draw.
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - 0 mismatches over 20,000 sequences: the arithmetic is integer but for `setCursorTooltip`'s pixel width and the
//!   alternate colours (`REAL_TO_INT`), which both sides compute in `f32` with the same operations.
//! - Every plant 9161-9172 caught.
//!
//! # Results (2026-11-11, `MOUSE_TOOLTIP_CASES=20000`, the default)
//!
//! 20,000 sequences, 609,559 steps: **0 mismatches** on the first full run. The tooltip box was drawn 40,915 times,
//! `m_displayTooltip` was set after 355,060 steps, and a text was drawn with its highlight under way 58,466 times.
//!
//! Planted bugs (same 20,000 sequences): **12 of 12 caught**, as predicted: 9161 by 20,000 sequences, 9162 3,689, 9163
//! 4,921, 9164 9,326, 9165 16,888, 9166 9,007, 9167 13,047, 9168 18,994, 9169 5,883, 9170 10,140, 9171 13,486, 9172 8,251.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9161 | `mouseNotifyResolutionChange` leaves no word wrap |
//! | 9162 | `setCursorTooltip` recalculates the width every time |
//! | 9163 | `setCursorTooltip` does not cap the width at the display's |
//! | 9164 | the alternate background colour is never taken |
//! | 9165 | no colour keeps the background colour |
//! | 9166 | the window's delay is ignored |
//! | 9167 | the tooltip shows at once |
//! | 9168 | the highlight does not restart when the tooltip starts to show |
//! | 9169 | the box is not moved up at the bottom edge |
//! | 9170 | the background never animates |
//! | 9171 | the highlighted strip is 16 pixels wide |
//! | 9172 | the highlight runs one millisecond ahead |

mod support;

use rts_lockstep_engine::client::mouse::{MouseIniSettings, MouseTooltip};
use rts_lockstep_engine::client::window_manager::DrawItem;
use rts_lockstep_engine::client::window_script::GameFont;
use rts_lockstep_engine::common::base_type::{ICoord2D, RGBAColorInt, RGBColor};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const TEXTS: [&str; 6] = ["", "Tank", "Supply Warehouse: $1000\nPlayer 1", "A tooltip of sixty characters, long enough to wrap twice!!!", "A", "Tank"];

fn unpack(w: i32) -> RGBAColorInt {
    let w = w as u32;
    RGBAColorInt { red: w & 0xFF, green: (w >> 8) & 0xFF, blue: (w >> 16) & 0xFF, alpha: w >> 24 }
}

fn pack(c: RGBAColorInt) -> i32 {
    (c.red | (c.green << 8) | (c.blue << 16) | (c.alpha << 24)) as i32
}

/// The wrapper's measure.
fn measure(t: &[u16], wrap: i32) -> (i32, i32) {
    let mut w = t.len() as i32 * 6;
    let mut h = 13;
    if wrap > 0 && w > wrap {
        h = 13 * ((w + wrap - 1) / wrap);
        w = wrap;
    }
    (w, h)
}

fn state(t: &MouseTooltip, out: &mut Vec<i32>) {
    out.extend([200, i32::from(t.is_empty), t.delay, t.last_tooltip_width.to_bits() as i32, pack(t.text_color), pack(t.back_color), t.still_time as i32, i32::from(t.display)]);
    let mut h: u32 = 2166136261;
    for &c in &t.display_string {
        h ^= u32::from(c);
        h = h.wrapping_mul(16777619);
    }
    let (size, bold) = t.font.as_ref().map_or((-1, -1), |f| (f.point_size, i32::from(f.bold)));
    out.extend([201, t.highlight_pos, t.highlight_update_start as i32, t.word_wrap, h as i32, t.display_string.len() as i32, size, bold]);
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    let s = MouseIniSettings {
        tooltip_font_name: b"Arial".to_vec(),
        tooltip_font_size: input[0],
        tooltip_font_is_bold: input[1] != 0,
        tooltip_animate_background: input[2] != 0,
        tooltip_fill_time: input[3],
        tooltip_delay_time: input[4],
        tooltip_color_text: unpack(input[5]),
        tooltip_color_highlight: unpack(input[6]),
        tooltip_color_shadow: unpack(input[7]),
        tooltip_color_background: unpack(input[8]),
        tooltip_color_border: unpack(input[9]),
        tooltip_width: f32::from_bits(input[10] as u32),
        use_tooltip_alt_text_color: input[11] != 0,
        use_tooltip_alt_back_color: input[12] != 0,
        adjust_tooltip_alt_color: input[13] != 0,
        ..MouseIniSettings::default()
    };
    let mut t = MouseTooltip { settings: s, display_width: input[15], ..MouseTooltip::default() };
    // `Mouse::init`
    let lang = (input[14] != 0).then(|| GameFont { name: b"Language".to_vec(), point_size: input[14], bold: true });
    t.mouse_notify_resolution_change(lang.clone());
    t.display = false;
    state(&t, &mut out);
    for w in input[16..].chunks(8) {
        let now = w[1] as u32;
        match w[0] {
            0 => {
                let text: Vec<u16> = TEXTS[(w[2] % 6) as usize].encode_utf16().collect();
                let mut rgb = RGBColor::default();
                if w[4] == 1 {
                    rgb.set_from_int(w[5]);
                } else {
                    rgb.red = ((w[5] & 0xFF) - 85) as f32 / 85.0;
                    rgb.green = (((w[5] >> 8) & 0xFF) - 85) as f32 / 85.0;
                    rgb.blue = (((w[5] >> 16) & 0xFF) - 85) as f32 / 85.0;
                }
                t.set_cursor_tooltip_with(&text, w[3], (w[4] != 0).then_some(&rgb), f32::from_bits(w[6] as u32));
            }
            1 => t.reset_tooltip_delay(now),
            2 => {
                t.script_debug = w[4] != 0;
                let moved = (0..w[2]).any(|i| (w[3] as u32 >> i) & 1 != 0);
                t.create_stream_messages(now, moved);
                t.script_debug = false;
            }
            3 => {
                for d in t.draw_tooltip(now, w[6] != 0, ICoord2D { x: w[2], y: w[3] }, ICoord2D { x: w[4], y: w[5] }, measure) {
                    match d {
                        DrawItem::FillRect { x, y, w, h, color } => out.extend([210, 0, x, y, w, h, color, 0]),
                        DrawItem::OpenRect { x, y, w, h, color } => out.extend([210, 1, x, y, w, h, color, 0]),
                        DrawItem::Text { x, y, color, drop_color, clip: Some(c), .. } => {
                            out.extend([211, x, y, color, drop_color, c.0, c.1, c.2]);
                            out.extend([212, c.3, 0, 0, 0, 0, 0, 0]);
                        }
                        other => panic!("unexpected draw {other:?}"),
                    }
                }
            }
            4 => t.mouse_notify_resolution_change(lang.clone()),
            _ => t.display_width = w[2],
        }
        state(&t, &mut out);
    }
    out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let cap = 16 * (input.len() + 16);
    let mut out = vec![0i32; cap];
    let n = unsafe { cpp::ref_mouse_tooltip_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), cap as i32) };
    out.truncate(n as usize);
    out
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let width = match g.below(6) {
        0 | 1 => 0.15f32,
        2 => 0.0,
        3 => 1.0,
        4 => g.uniform(0.0, 3.0),
        _ => [1.0e10f32, -0.5, 0.001][g.below(3) as usize],
    };
    let fill = if g.below(8) == 0 { -(g.below(50) as i32) - 1 } else { 1 + g.below(200) as i32 };
    let mut v = vec![
        g.below(30) as i32,
        g.below(2) as i32,
        i32::from(g.below(4) != 0),
        fill,
        g.below(306) as i32 - 5,
        g.i32(),
        g.i32(),
        g.i32(),
        g.i32(),
        g.i32(),
        width.to_bits() as i32,
        g.below(2) as i32,
        g.below(2) as i32,
        g.below(2) as i32,
        if g.below(3) == 0 { 1 + g.below(20) as i32 } else { 0 },
        [800, 1024, 390, 1920, 100][g.below(5) as usize],
    ];
    let mut now: u32 = if g.below(4) == 0 { u32::MAX - g.below(2000) } else { g.below(100_000) };
    let n = 1 + g.below(60) as usize;
    for _ in 0..n {
        now = now.wrapping_add(g.below(151));
        let s = match g.below(14) {
            0..=3 => {
                let delay = match g.below(4) {
                    0 | 1 => -1,
                    2 => 0,
                    _ => g.below(300) as i32,
                };
                let w = match g.below(5) {
                    0 | 1 => 1.0f32,
                    2 => 1.5,
                    3 => [0.5f32, 0.0][g.below(2) as usize],
                    _ => g.uniform(-1.0, 4.0),
                };
                [0, now as i32, g.below(6) as i32, delay, g.below(3) as i32, g.i32(), w.to_bits() as i32, 0]
            }
            4 => [1, now as i32, 0, 0, 0, 0, 0, 0],
            5..=8 => [2, now as i32, g.below(5) as i32, g.below(32) as i32, i32::from(g.below(10) == 0), 0, 0, 0],
            9..=11 => {
                let (mx, my) = match g.below(3) {
                    0 => (799, 599),
                    1 => (800, 600),
                    _ => (g.below(2000) as i32, g.below(1500) as i32),
                };
                [3, now as i32, g.below(950) as i32 - 50, g.below(750) as i32 - 50, mx, my, i32::from(g.below(8) == 0), 0]
            }
            12 => [4, now as i32, 0, 0, 0, 0, 0, 0],
            _ => [5, now as i32, [800, 1024, 390, 0, 5000][g.below(5) as usize], 0, 0, 0, 0, 0],
        };
        v.extend(s);
    }
    v
}

#[test]
fn the_mouse_tooltip_matches_upstream() {
    let cases: u64 = std::env::var("MOUSE_TOOLTIP_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x124_1240);
    let mut whole = Check::new("the mouse's tooltip sequences");
    let (mut steps, mut boxes, mut shown_steps, mut full) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        steps += ((input.len() - 16) / 8) as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        for rec in c.chunks(8) {
            match rec[0] {
                210 if rec[1] == 0 => boxes += 1,
                200 if rec[7] == 1 => shown_steps += 1,
                211 if rec[7] - rec[5] > 0 => full += 1,
                _ => {}
            }
        }
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len())) / 8 * 8;
            println!("sequence {i}: rust {:?} c++ {:?}", &r[k.min(r.len())..(k + 24).min(r.len())], &c[k.min(c.len())..(k + 24).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps} steps: a tooltip box drawn {boxes} times, m_displayTooltip set after {shown_steps} steps, text drawn with a highlight under way {full} times");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
