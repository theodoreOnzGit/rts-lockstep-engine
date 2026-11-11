#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Mouse.cpp
//   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/GameClient/Win32Mouse.cpp
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DMouse.cpp
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (setMouseCursor, setScrolling)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::mouse` (GitHub #117) against the C++.
//!
//! # Method
//!
//! The C++ side is the mouse's cursor functions cut verbatim by build.rs (`differential/wrap_mouse.cpp` lists the
//! cuts: `CursorInfo`, `Mouse::setMouseText`, `getCursorIndex`, `Mouse::setCursor`, `Win32Mouse::setVisibility`,
//! `initCursorResources`, `Win32Mouse::setCursor`, `W3DMouse::setCursor`, `draw` and `setCursorDirection` with every
//! branch, `InGameUI::setMouseCursor` and `setScrolling`), compiled on stand-in classes holding the members they use,
//! in `RM_WINDOWS` as `m_winCursors` sets it, with `LoadCursorFromFile` failing on paths that contain "missing" or end
//! in "3.ANI" (so the directions' file names are checked too) and `SetCursor` recorded.
//!
//! Each case is a random `Mouse.ini` (each of the 40 cursors with or without a `CursorText`, random text colours, no
//! texture, a loadable one or a missing one, and -1 to 8 `Directions`, 7 at most for the last cursor, whose eighth
//! slot is past the C array), then 1 to 60 steps: `setCursor` of any cursor, `setVisibility`, `draw`, the focus lost
//! or back, the InGameUI's scrolling and scroll amount (zero, axis-aligned, the exact diagonals, the frame boundaries
//! k·π/8 of an eight-way cursor and their neighbours one ULP away, and random ones, negative included),
//! `WM_SETCURSOR`, `InGameUI::setMouseCursor` and `setScrolling` in each mouse mode, and `getCursorIndex` of names in
//! their own case, in other cases, unknown and empty. Both sides write the loaded resources after the setup, then the
//! cursor's state after the setup and after each step (the current cursor, visibility, focus, direction frame, the
//! Windows cursor and `SetCursor`'s last argument, the D3D, W3D and polygon cursors, the cursor text and its colours,
//! the InGameUI's scrolling and mode cursor) as words; the words are compared whole.
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - 0 mismatches over 20,000 sequences: the frame is an integer from `atan2` in double (bound to petir on both
//!   sides), an exact `fmod` and a division in double, so even the boundary angles agree.
//! - Planted bugs 9061-9071 all caught but **9068**, which is predicted *not* caught: after `init`,
//!   `Mouse::setCursor` only ever sees the current cursor in `RM_WINDOWS` (the module's quirk), so whether it would
//!   clear the text is never asked.
//!
//! # Results (2026-11-11, `MOUSE_CASES=20000`, the default)
//!
//! 20,000 sequences, 608,946 steps: **0 mismatches** on the first full run. `SetCursor` showed a resource after 86,436
//! steps (direction frames 0..7: 67,398, 4,711, 3,826, 0, 2,144, 3,496, 2,013, 2,848: frame 3 never shows because
//! every `...3.ANI` is a missing file here, so `SetCursor(NULL)`, as the C++ does) and hid it after 468,069; a cursor
//! text was set after **0** steps, the quirk the module records. (An earlier run crashed in the C++ on the test's own
//! generator: a "one ULP below +0.0" scroll amount is a NaN, whose `(Int)` frame indexed past `cursorResources`; the
//! InGameUI's scroll is never a NaN, so the generator now steps away from zero.)
//!
//! Planted bugs (same 20,000 sequences): **10 of 11 caught, 9068 not caught, as predicted**: 9061 by 10,291
//! sequences, 9062 11,272, 9063 5,282, 9064 20,000, 9065 15,331, 9066 9,856, 9067 8,818, 9069 10,633, 9070 20,000,
//! 9071 18,581; 9068 by 0 (the text is never asked to change after `init` in `RM_WINDOWS`, see above).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9061 | `setCursorDirection` divides by the new cursor's directions |
//! | 9062 | `Win32Mouse::setCursor` ignores the visibility |
//! | 9063 | `Win32Mouse::setCursor` goes on with the focus lost |
//! | 9064 | `W3DMouse::setCursor` reaches the Windows cursor outside `draw` |
//! | 9065 | `setVisibility` does not set the cursor again |
//! | 9066 | `getCursorIndex` compares with case |
//! | 9067 | the direction frame is truncated without the `+ 0.5` |
//! | 9068 | `Mouse::setCursor` keeps the text when the definition has none |
//! | 9069 | `setMouseCursor` remembers the arrow and the scroll cursor as the mode's |
//! | 9070 | `initCursorResources` loads one direction too many |
//! | 9071 | `draw` leaves `m_drawing` set |

mod support;

use rts_lockstep_engine::client::mouse::{cursor, get_cursor_index, in_game_ui_set_mouse_cursor, in_game_ui_set_scrolling, Mouse, OsCursor, CURSOR_INI_NAMES};
use rts_lockstep_engine::common::base_type::{Coord2D, RGBAColorInt};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const NUM: usize = cursor::NUM_MOUSE_CURSORS as usize;

fn unpack(w: i32) -> RGBAColorInt {
    let w = w as u32;
    RGBAColorInt { red: w & 0xFF, green: (w >> 8) & 0xFF, blue: (w >> 16) & 0xFF, alpha: w >> 24 }
}

fn pack(c: RGBAColorInt) -> i32 {
    (c.red | (c.green << 8) | (c.blue << 16) | (c.alpha << 24)) as i32
}

/// `LoadCursorFromFile`'s stand-in (as the wrapper's).
fn exists(path: &str) -> bool {
    !(path.contains("missing") || path.ends_with("3.ANI"))
}

/// FNV-1a over the text's 16-bit characters (the game's text stand-in returns the label, a byte a character).
fn hash(label: &[u8]) -> i32 {
    let mut h: u32 = 2166136261;
    for &c in label {
        h ^= u32::from(c);
        h = h.wrapping_mul(16777619);
    }
    h as i32
}

/// The InGameUI's members the steps use.
#[derive(Default)]
struct Ui {
    mouse_mode: i32,
    mouse_mode_cursor: i32,
    is_scrolling: bool,
    scroll_amt: Coord2D,
}

fn state(m: &Mouse, ui: &Ui, out: &mut Vec<i32>) {
    let set = match m.os_cursor {
        None => -1,
        Some(OsCursor::Hidden) => -2,
        Some(OsCursor::Resource { cursor, direction }) => cursor * 8 + direction,
    };
    out.extend([100, m.current_cursor, i32::from(m.visible), i32::from(m.lost_focus), m.direction_frame, m.current_win32_cursor, i32::from(m.drawing), set]);
    let (h, c, d, e) = match &m.cursor_text {
        Some(t) => (hash(&t.label), t.color, t.drop_color, 1),
        None => (0, RGBAColorInt { red: 255, green: 255, blue: 255, alpha: 255 }, RGBAColorInt { red: 255, green: 255, blue: 255, alpha: 255 }, 0),
    };
    out.extend([101, m.current_d3d_cursor, m.current_w3d_cursor, m.current_polygon_cursor, h, pack(c), pack(d), e]);
    out.extend([102, i32::from(ui.is_scrolling), ui.mouse_mode_cursor, 0, 0, 0, 0, 0]);
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    let mut m = Mouse::new();
    for c in 0..NUM {
        let w = &input[c * 8..c * 8 + 8];
        let info = &mut m.cursor_info[c];
        info.cursor_text = match w[0] {
            1 => b"CURSOR:A".to_vec(),
            2 => b"CURSOR:B".to_vec(),
            _ => Vec::new(),
        };
        info.cursor_text_color = unpack(w[1]);
        info.cursor_text_drop_color = unpack(w[2]);
        info.texture_name = match w[3] {
            0 => Vec::new(),
            1 => format!("SCC{c}").into_bytes(),
            _ => format!("missing{c}").into_bytes(),
        };
        info.num_directions = w[4];
    }
    m.init_cursor_resources(exists);
    m.init();
    for c in 0..NUM {
        let mask = (0..8).filter(|d| m.resources[c * 8 + d]).fold(0, |a, d| a | (1 << d));
        out.extend([104, c as i32, mask, 0, 0, 0, 0, 0]);
    }
    let mut ui = Ui::default();
    state(&m, &ui, &mut out);
    for w in input[NUM * 8..].chunks(8) {
        // `TheInGameUI->isScrolling()` and `getScrollAmount()` as the mouse reads them, live
        m.ui_scrolling = ui.is_scrolling;
        m.ui_scroll_amount = ui.scroll_amt;
        match w[0] {
            0 => m.set_cursor(w[1]),
            1 => m.set_visibility(w[1] != 0),
            2 => m.draw(),
            3 => m.lost_focus(w[1] != 0),
            4 => {
                ui.is_scrolling = w[1] != 0;
                ui.scroll_amt = Coord2D { x: f32::from_bits(w[2] as u32), y: f32::from_bits(w[3] as u32) };
            }
            5 => m.wm_set_cursor(),
            6 => {
                ui.mouse_mode = w[2];
                in_game_ui_set_mouse_cursor(&mut m, ui.mouse_mode == 2, &mut ui.mouse_mode_cursor, w[1]);
            }
            7 => {
                ui.mouse_mode = w[2];
                in_game_ui_set_scrolling(&mut m, ui.mouse_mode == 2, &mut ui.mouse_mode_cursor, &mut ui.is_scrolling, w[1] != 0);
            }
            _ => {
                let bytes: Vec<u8> = w[2..8].iter().flat_map(|x| x.to_le_bytes()).take_while(|&b| b != 0).collect();
                out.extend([103, get_cursor_index(&bytes), 0, 0, 0, 0, 0, 0]);
            }
        }
        state(&m, &ui, &mut out);
    }
    out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let cap = 8 * (NUM + 4 * (input.len() / 8 + 2));
    let mut out = vec![0i32; cap];
    let n = unsafe { cpp::ref_mouse_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), cap as i32) };
    out.truncate(n as usize);
    out
}

/// A scroll amount: zero, axis-aligned, diagonal, at an eight-way frame's boundary (k·π/8) or one ULP from it, or random.
fn scroll(g: &mut Gen) -> (f32, f32) {
    match g.below(6) {
        0 => (0.0, 0.0),
        1 => [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.0, 0.0), (-0.0, 0.0)][g.below(6) as usize],
        2 => {
            let s = [1.0f32, -1.0][g.below(2) as usize];
            let t = [1.0f32, -1.0][g.below(2) as usize];
            let k = g.uniform(0.01, 50.0);
            (s * k, t * k)
        }
        3 | 4 => {
            let a = (g.below(16) as f64) * core::f64::consts::PI / 8.0;
            let r = g.uniform(0.1, 100.0) as f64;
            let (mut x, mut y) = ((petir::Real::cos(a) * r) as f32, (petir::Real::sin(a) * r) as f32);
            // one ULP away (away from zero: a step below +0.0 would be a NaN, which the InGameUI's scroll never is)
            match g.below(3) {
                0 => x = f32::from_bits(x.to_bits() + 1),
                1 => y = f32::from_bits(y.to_bits() + 1),
                _ => {}
            }
            (x, y)
        }
        _ => (g.uniform(-500.0, 500.0), g.uniform(-500.0, 500.0)),
    }
}

/// A cursor name for `getCursorIndex`: one of the 40 as written, in other case, unknown, or empty; packed 4 bytes a word.
fn name_words(g: &mut Gen) -> [i32; 6] {
    let n = CURSOR_INI_NAMES[g.below(NUM as u32) as usize];
    let s: Vec<u8> = match g.below(5) {
        0 | 1 => n.as_bytes().to_vec(),
        2 => n.to_ascii_uppercase().into_bytes(),
        3 => n.bytes().enumerate().map(|(i, b)| if i % 2 == 0 { b.to_ascii_lowercase() } else { b }).collect(),
        _ => [b"".as_slice(), b"Nope", b"Arrows", b"Scrol"][g.below(4) as usize].to_vec(),
    };
    let mut bytes = [0u8; 24];
    bytes[..s.len().min(23)].copy_from_slice(&s[..s.len().min(23)]);
    let mut w = [0i32; 6];
    for (i, c) in bytes.chunks(4).enumerate() {
        w[i] = i32::from_le_bytes([c[0], c[1], c[2], c[3]]);
    }
    w
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let mut v = Vec::new();
    for c in 0..NUM {
        let max_dirs = if c == NUM - 1 { 7 } else { 8 };
        let dirs = match g.below(4) {
            0 => 1,
            1 => max_dirs as i32,
            2 => g.below(max_dirs + 2) as i32 - 1,
            _ => [0, 1, 2, 4, max_dirs as i32][g.below(5) as usize],
        };
        v.extend([g.below(3) as i32, g.i32(), g.i32(), g.below(3) as i32, dirs, 0, 0, 0]);
    }
    // the scroll cursor is the one with directions in the game; make it likelier to be one
    if g.below(2) == 0 {
        v[cursor::SCROLL as usize * 8 + 3] = 1;
        v[cursor::SCROLL as usize * 8 + 4] = 8;
    }
    let n = 1 + g.below(60) as usize;
    let pick = |g: &mut Gen| if g.below(3) == 0 { cursor::SCROLL } else { g.below(NUM as u32) as i32 };
    for _ in 0..n {
        let s = match g.below(16) {
            0..=2 => [0, pick(g), 0, 0, 0, 0, 0, 0],
            3 | 4 => [1, g.below(2) as i32, 0, 0, 0, 0, 0, 0],
            5..=7 => [2, 0, 0, 0, 0, 0, 0, 0],
            8 => [3, i32::from(g.below(4) == 0), 0, 0, 0, 0, 0, 0],
            9 | 10 => {
                let (x, y) = scroll(g);
                [4, i32::from(g.below(4) != 0), x.to_bits() as i32, y.to_bits() as i32, 0, 0, 0, 0]
            }
            11 => [5, 0, 0, 0, 0, 0, 0, 0],
            12 => [6, pick(g), g.below(3) as i32, 0, 0, 0, 0, 0],
            13 | 14 => [7, g.below(2) as i32, g.below(3) as i32, 0, 0, 0, 0, 0],
            _ => {
                let w = name_words(g);
                [8, 0, w[0], w[1], w[2], w[3], w[4], w[5]]
            }
        };
        v.extend(s);
    }
    v
}

#[test]
fn the_mouse_cursor_matches_upstream() {
    let cases: u64 = std::env::var("MOUSE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x117_1170);
    let mut whole = Check::new("TheMouse cursor sequences");
    let (mut steps, mut shown, mut hidden, mut frames, mut texts) = (0u64, 0u64, 0u64, [0u64; 8], 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        steps += (input.len() / 8 - NUM) as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        for rec in c.chunks(8) {
            if rec[0] == 100 {
                match rec[7] {
                    -2 => hidden += 1,
                    x if x >= 0 => {
                        shown += 1;
                        frames[(x % 8) as usize] += 1;
                    }
                    _ => {}
                }
            }
            if rec[0] == 101 && rec[7] == 1 && rec[4] != hash(b"") {
                texts += 1;
            }
        }
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len())) / 8 * 8;
            println!("sequence {i}: rust {:?} c++ {:?}", &r[k.min(r.len())..(k + 24).min(r.len())], &c[k.min(c.len())..(k + 24).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps} steps: SetCursor showed a resource after {shown} steps (direction frames {frames:?}), hid it after {hidden}; a cursor text after {texts}");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
