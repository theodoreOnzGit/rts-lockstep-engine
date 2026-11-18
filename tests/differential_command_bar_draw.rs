#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DControlBar.cpp (W3DCommandBarGridDraw)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (drawRectClock, drawRemainingRectClock)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindow.cpp (GameWindow::winSetEnabledBorderColor)
//   GeneralsMD/Code/GameEngine/Include/GameClient/GadgetPushButton.h (GadgetButtonSetDisabledImage)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (updateBuildQueueDisabledImages,
//     updateRightHUDImage)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp (init's two image lines)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! GitHub #194: the command bar's grid in the scheme's border colour, the push button clocks in the build-up clock
//! colour, and the scheme's queue and right HUD images, against the C++.
//!
//! # Method
//!
//! The C++ side is cut verbatim by build.rs (`differential/wrap_command_bar_draw.cpp` lists the cuts and the stand-ins).
//! Four kinds of case:
//!
//! - **grid** (30 %): `W3DCommandBarGridDraw` on a window under a parent (both placed in -100..900), its size 0..400 (a
//!   tenth of them negative down to -50), its status random among `ENABLED` and `IMAGE` (and other bits), its state
//!   hilited or not, its enabled border colour and colour 0 random or `GAME_COLOR_UNDEFINED`, the bar's border colour
//!   random. Both sides record `W3DGameWinDefaultDraw`'s call as `[1, IMAGE bit, enabled border colour 0 at the call]`
//!   (the Rust side checks that its draw list starts with exactly what `w3d_game_win_default_draw` draws for the window
//!   after the call, else records a marker), each line as `[2, x0, y0, x1, y1, width bits, colour]` (the Rust's
//!   `DrawItem::Line` has no width: #82's `WIN_DRAW_LINE_WIDTH` 1.0 is written for it), then the window's enabled border
//!   colour 0 after the call.
//! - **clock** and **remaining clock** (30 % each): `drawRectClock` / `drawRemainingRectClock` on a rectangle placed in
//!   -100..900, sized 0..200 (a tenth negative down to -50, so `width/2` truncates toward zero both ways), the percent
//!   half the time from the thresholds and their neighbours (-1, 0, 1, 12, 13, 14, 24, 25, 26, 37, 38, 39, 49, 50, 51, 62,
//!   63, 64, 74, 75, 76, 86, 87, 88, 99, 100, 101) and half uniform in -5..=105, and a random colour. The renderer's calls
//!   in order: `Reset`, `Enable_Texturing`, `Add_Rect` (the rectangle's four floats' bits, border width and colour,
//!   fill), `Add_Tri` (six vertex and six uv floats' bits, colour), `Render`. The Rust's `ClockPiece`s are written in
//!   that form with the constants upstream passes (`FALSE`, `0, 0`, the zero uvs).
//! - **images** (10 %): `ControlBarScheme::init`'s two lines on a bar whose nine queue buttons are each present or not
//!   and whose right HUD window is present or not, with random images (or none) everywhere. A queue button missing while
//!   the queue image is set is dereferenced upstream (a crash): such cases are not generated (with an image all nine
//!   buttons are present). Both record each present queue button's enabled 5, 6 and disabled 0 images and the right
//!   HUD's enabled 0 after.
//!
//! # Predictions (written before the first run, 2026-11-17)
//!
//! - 0 mismatches over 20,000 cases: integer arithmetic, `double` products truncated, and `float` products and
//!   differences of exactly representable values in the same order on both sides (SSE, no contraction).
//! - Plants 10416-10430 all caught. The fewest: 10427 (the 87/88 split), only at percent 87 of a remaining clock: about
//!   half of 30 % of 20,000 cases draw from the 27 thresholds, so ~110 cases at 87; 10424 (100 accepted) about the same
//!   from 100 plus a few uniform; 10421 (0 accepted in `drawRectClock`) only at 0: ~140; 10423 (the `h` slip in 13..24 %) in every such clock with a height of 2 or more, ~700; 10418 (an `IMAGE`
//!   window drawing lines) about half the grid cases; 10428 (`(width+1)/2`) only on odd widths, about half the remaining
//!   clocks that draw; 10429 and 10430 only in the image cases with an image: ~1,400 and ~700.
//!
//! # Results (2026-11-17, `COMMAND_BAR_DRAW_CASES=20000`, the default)
//!
//! 20,000 cases (grid 6,007, clock 5,962, remaining clock 6,034, images 1,997; 11,856 lines, 10,746 clocks drawn, 10,511
//! rectangles, 15,999 triangles): **0 mismatches** on the first run, as predicted, every float bit-identical. Plants,
//! mismatching cases: 10416 2,731; 10417 2,764; 10418 3,043; 10419 2,956; 10420 130; 10421 374; 10422 545; 10423 755;
//! 10424 147; 10425 140; 10426 525; 10427 133; 10428 2,625; 10429 1,410; 10430 228. **15 of 15 caught.**
//! **Predictions missed:** 10421 was caught 374 times, not ~140: the plant accepts every percent below 1, not only 0,
//! so -1 from the thresholds and -5..-1 from the uniform draw count too (the table below said "accepts 0"). 10430 was
//! caught 228 times, not ~700: it matters only **without** a right HUD image (30 %), with the window (50 %) holding an
//! image to lose (70 %) of ~2,000 image cases, ~210; the prediction counted the cases with an image.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 10416 | the grid's fractions rounded instead of truncated |
//! | 10417 | the grid does not set the window's enabled border colour |
//! | 10418 | an `IMAGE` window's grid draws its lines too |
//! | 10419 | the grid's vertical lines use the height for the width |
//! | 10420 | `drawRectClock` 100 % not one rectangle |
//! | 10421 | `drawRectClock` accepts percents below 1 |
//! | 10422 | `drawRectClock`'s 88..99 % triangle divides by 12 |
//! | 10423 | `drawRectClock`'s 13..24 % triangle uses the height, not half |
//! | 10424 | `drawRemainingRectClock` accepts 100 |
//! | 10425 | `drawRemainingRectClock` splits the first quarter at `<= 13` |
//! | 10426 | `drawRemainingRectClock`'s 38..49 % triangle divides by 13 |
//! | 10427 | `drawRemainingRectClock` splits the last quarter at 88 |
//! | 10428 | `drawRemainingRectClock`'s centre `(width + 1) / 2` across |
//! | 10429 | `updateBuildQueueDisabledImages` does not clear enabled 5 and 6 |
//! | 10430 | `updateRightHUDImage` takes a `NULL` image |

mod support;

use rts_lockstep_engine::client::w3d_command_bar::{draw_rect_clock, draw_remaining_rect_clock, update_build_queue_disabled_images, update_right_hud_image, w3d_command_bar_grid_draw, ClockPiece};
use rts_lockstep_engine::client::window_draw::w3d_game_win_default_draw;
use rts_lockstep_engine::client::window_manager::{DrawItem, GameWindowManager, NoGuiHost, SystemFunc};
use rts_lockstep_engine::client::window_script::{win_status, WinId, WinInstanceData};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

/// `GAME_COLOR_UNDEFINED`.
const UNDEFINED: i32 = 0x00FF_FFFF;
/// The percents the clocks branch on, and their neighbours.
const THRESHOLDS: [i32; 27] = [-1, 0, 1, 12, 13, 14, 24, 25, 26, 37, 38, 39, 49, 50, 51, 62, 63, 64, 74, 75, 76, 86, 87, 88, 99, 100, 101];

fn color(g: &mut Gen) -> i32 {
    match g.below(4) {
        0 => UNDEFINED,
        _ => g.i32(),
    }
}

fn place(g: &mut Gen) -> i32 {
    g.below(1001) as i32 - 100
}

fn size(g: &mut Gen, max: u32) -> i32 {
    if g.below(10) == 0 {
        -(g.below(51) as i32)
    } else {
        g.below(max + 1) as i32
    }
}

fn image(g: &mut Gen) -> i32 {
    if g.below(10) < 3 {
        -1
    } else {
        g.below(8) as i32
    }
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let r = g.below(10);
    let kind = if r < 3 { 0 } else if r < 6 { 1 } else if r < 9 { 2 } else { 3 };
    let mut v = vec![kind];
    match kind {
        0 => {
            let (px, py, x, y) = (place(g), place(g), place(g) / 2, place(g) / 2);
            let (w, h) = (size(g, 400), size(g, 400));
            let mut status = 0u32;
            if g.below(4) != 0 {
                status |= win_status::ENABLED;
            }
            if g.below(2) == 0 {
                status |= win_status::IMAGE;
            }
            if g.below(4) == 0 {
                status |= 1 << g.below(32);
            }
            let state = if g.below(3) == 0 { 2 } else { 0 };
            v.extend([px, py, x, y, w, h, status as i32, state, color(g), color(g), color(g)]);
        }
        1 | 2 => {
            let (x, y, w, h) = (place(g), place(g), size(g, 200), size(g, 200));
            let percent = if g.below(2) == 0 { THRESHOLDS[g.below(27) as usize] } else { g.below(111) as i32 - 5 };
            v.extend([x, y, w, h, percent, g.i32()]);
        }
        _ => {
            let (q, r) = (image(g), image(g));
            let hud = g.below(2) as i32;
            let mask = if q >= 0 { 0x1FF } else { g.below(512) as i32 };
            v.extend([q, r, hud, mask]);
            for _ in 0..9 * 3 + 1 {
                v.push(image(g));
            }
        }
    }
    v
}

fn cpp_case(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 4096];
    let n = unsafe { cpp::ref_command_bar_draw_case(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn name(i: i32) -> Option<Vec<u8>> {
    (i >= 0).then(|| format!("Img{i}").into_bytes())
}

fn index(n: &Option<Vec<u8>>) -> i32 {
    n.as_ref().map_or(-1, |n| String::from_utf8_lossy(&n[3..]).parse().expect("an image name"))
}

/// A window at (x, y) of size (w, h) under `parent` (its region and size set as the C++ stand-in's are).
fn window(wm: &mut GameWindowManager, parent: Option<WinId>, x: i32, y: i32, w: i32, h: i32) -> WinId {
    let id = wm.win_create(&mut NoGuiHost, parent, 0, x, y, w.max(0), h.max(0), SystemFunc::Default, Some(WinInstanceData::new()));
    let win = &mut wm.tree.windows[id];
    win.region = (x, y, x.wrapping_add(w), y.wrapping_add(h));
    win.size = (w, h);
    id
}

fn pieces(out: &mut Vec<i32>, p: Option<Vec<ClockPiece>>, color: i32) {
    let Some(p) = p else { return };
    out.extend([10, 11, 0]);
    let b = |f: f32| f.to_bits() as i32;
    for piece in p {
        match piece {
            ClockPiece::Rect(r) => out.extend([12, b(r[0]), b(r[1]), b(r[2]), b(r[3]), b(0.0), 0, color]),
            ClockPiece::Tri(t) => {
                out.push(13);
                out.extend(t.iter().map(|&f| b(f)));
                out.extend([b(0.0); 6]);
                out.push(color);
            }
        }
    }
    out.push(14);
}

fn rust_case(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    match input[0] {
        0 => {
            let mut wm = GameWindowManager::new((800, 600));
            let p = window(&mut wm, None, input[1], input[2], 10, 10);
            let w = window(&mut wm, Some(p), input[3], input[4], input[5], input[6]);
            let win = &mut wm.tree.windows[w];
            win.status = input[7] as u32;
            win.inst.state = input[8];
            win.inst.enabled_draw_data[0].border_color = input[9];
            win.inst.enabled_draw_data[0].color = input[10];
            let mut items = Vec::new();
            w3d_command_bar_grid_draw(&mut wm, w, input[11], &mut items);
            let mut default = Vec::new();
            w3d_game_win_default_draw(&wm, w, &mut default);
            let border = wm.tree.windows[w].inst.enabled_draw_data[0].border_color;
            if items.len() < default.len() || items[..default.len()] != default[..] {
                out.push(99);
            }
            out.extend([1, (input[7] as u32 & win_status::IMAGE != 0) as i32, border]);
            for d in &items[default.len().min(items.len())..] {
                match d {
                    DrawItem::Line { x0, y0, x1, y1, color } => out.extend([2, *x0, *y0, *x1, *y1, 1.0f32.to_bits() as i32, *color]),
                    _ => out.push(98),
                }
            }
            out.extend([3, border]);
        }
        1 => pieces(&mut out, draw_rect_clock(input[1], input[2], input[3], input[4], input[5]), input[6]),
        2 => pieces(&mut out, draw_remaining_rect_clock(input[1], input[2], input[3], input[4], input[5]), input[6]),
        _ => {
            let mut wm = GameWindowManager::new((800, 600));
            let queue: Vec<Option<WinId>> = (0..9).map(|i| ((input[4] >> i) & 1 != 0).then(|| window(&mut wm, None, 0, 0, 10, 10))).collect();
            for (i, q) in queue.iter().enumerate() {
                if let Some(q) = q {
                    let d = &mut wm.tree.windows[*q].inst;
                    d.enabled_draw_data[5].image = name(input[5 + i * 3]);
                    d.enabled_draw_data[6].image = name(input[6 + i * 3]);
                    d.disabled_draw_data[0].image = name(input[7 + i * 3]);
                }
            }
            let hud = (input[3] != 0).then(|| window(&mut wm, None, 0, 0, 10, 10));
            if let Some(h) = hud {
                wm.tree.windows[h].inst.enabled_draw_data[0].image = name(input[5 + 27]);
            }
            update_build_queue_disabled_images(&mut wm.tree, &queue, name(input[1]));
            update_right_hud_image(&mut wm.tree, hud, name(input[2]));
            for (i, q) in queue.iter().enumerate() {
                if let Some(q) = q {
                    let d = &wm.tree.windows[*q].inst;
                    out.extend([20, i as i32, index(&d.enabled_draw_data[5].image), index(&d.enabled_draw_data[6].image), index(&d.disabled_draw_data[0].image)]);
                }
            }
            if let Some(h) = hud {
                out.extend([21, index(&wm.tree.windows[h].inst.enabled_draw_data[0].image)]);
            }
        }
    }
    out
}

#[test]
fn the_command_bar_draws_are_upstreams() {
    let cases: u64 = std::env::var("COMMAND_BAR_DRAW_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x194_0bf4);
    let mut checks = [Check::new("W3DCommandBarGridDraw"), Check::new("drawRectClock"), Check::new("drawRemainingRectClock"), Check::new("init's queue and right HUD images")];
    let mut kinds = [0u64; 4];
    let (mut lines, mut rects, mut tris, mut drawn) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        let k = input[0] as usize;
        kinds[k] += 1;
        let r = rust_case(&input);
        let c = cpp_case(&input);
        let mut j = 0;
        while j < c.len() {
            let n = match c[j] {
                1 => 3,
                2 => 7,
                3 => 2,
                10 | 14 => 1,
                11 => 2,
                12 => 8,
                13 => 14,
                20 => 5,
                21 => 2,
                _ => 1,
            };
            match c[j] {
                2 => lines += 1,
                12 => rects += 1,
                13 => tris += 1,
                10 => drawn += 1,
                _ => {}
            }
            j += n;
        }
        if r != c && !planted {
            println!("case {i} {input:?}:\n rust {r:?}\n c++  {c:?}");
        }
        checks[k].eq(r, c, || format!("case {i} {input:?}"));
    }
    println!("{cases} cases (grid, clock, remaining clock, images: {kinds:?}): {lines} lines, {drawn} clocks drawn, {rects} rectangles, {tris} triangles");
    let mut caught = 0;
    for c in checks {
        if planted {
            caught += c.report();
        } else {
            c.finish();
        }
    }
    if planted {
        println!("MEASURED planted total mismatching cases {caught}");
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}
