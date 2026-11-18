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

/// (#195) The C++ stand-ins are globals: the two tests take turns.
static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_case(input: &[i32]) -> Vec<i32> {
    let _turn = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0i32; 1 << 16];
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

// ---------------------------------------------------------------------------------------------------------------------
// GitHub #195: the other draw callbacks of W3DControlBar.cpp.
// ---------------------------------------------------------------------------------------------------------------------
//
// # Method (#195)
//
// The same C++ wrapper, kinds 4 to 10 (`differential/wrap_command_bar_draw.cpp` lists the cuts and stand-ins), on its own
// generator (seed `0x195_0bf4`), 20,000 cases by default (`COMMAND_BAR_DRAW_195_CASES`):
//
// - **power** (30 %, `W3DPowerDraw`): a window under a parent as the grid's (width 0..400, height 0..60, a tenth
//   negative); the observer's bar on a fifth of the time, its look-at player present 3/4, the local player 9/10;
//   production and consumption each from a mix (0, 1 and 2; 0..200; 200..1,000,000; a twentieth negative), consumption
//   half the time production plus -10..10 (the yellow band's edges); `TheGlobalData` present 9/10 with base 7,
//   intervals 3.0 and yellow range 5 (`GameData.ini`'s defaults, `GlobalData::GlobalData`) half the time, else base
//   0..12, intervals 0..10 (a twentieth 0) and range 0..20; the four images sized 1..24 (the slider 0..24).
// - **experience** (25 %, `W3DCommandBarGenExpDraw`): active 4/5; level down 0..5,000, level up above it by 1..5,000 (a
//   tenth `INT_MAX`, as `Player::resetRank` gives at the top rank, a tenth below it), skill points from 500 under level
//   down to 30 % past level up; the window 0..100 by 0..300; the three images 1..30 each way.
// - **help popup** (20 %, `W3DCommandBarHelpPopupDraw`): the window 0..400 by 0..300 (an eighth exactly the two ends'
//   heights), the three images 1..40 each way.
// - **right HUD** (8 %, `W3DRightHUDDraw`): as the grid's cases.
// - **cameo movie** (5 %, `W3DCameoMovieDraw`): a video buffer half the time.
// - **top** (4 %, `W3DCommandBarTopDraw`): `ButtonGeneral` present, hidden, the player active, each at random.
// - **logN** (8 %): `logN(value, base)`'s bits: small integers, 1.5, 0, negatives, raw bit patterns.
//
// Both sides record each `drawImage` (the image, the rectangle, the colour, and the display's clip region when it is
// on), `drawVideoBuffer`, `W3DGameWinDefaultDraw` (as the grid's), and the clip flag at the end. Cases that crash upstream
// are not generated: a centre image of width (power) or height (experience, popup) 0 where it is divided by, and
// `levelUp == levelDown`. Upstream's images are function statics found once, so in the C++ every image is always
// present; their absence is checked by the unit tests in `src/client/w3d_command_bar.rs`.
//
// # Predictions (written before the first run, 2026-11-18)
//
// - 0 mismatches over 20,000 cases: integer arithmetic (both wrap: `-fwrapv`), `logN` in the same mixed float/double
//   order with petir's `log10` on both sides, and the float-to-`Int` conversions x86's (`cvttss2si`'s `0x80000000` for
//   NaN and out of range).
// - Plants 10431-10445 all caught. Roughly: 10431 (yellow at `>=`) only where consumption is exactly production less the
//   range, ~80; 10432 (no clamp) where production passes `base^intervals` (343 at the defaults), ~1,000; 10433 (the last
//   piece unclipped) in most bars drawn, ~2,500; 10434 (no 1.5) at consumption 1, ~150; 10435 (no right alignment)
//   where consumption passes the scale, ~800; 10436 (Rust's `as`) at NaN and +inf only (-inf saturates to the same
//   `i32::MIN`), ~300; 10437 (no 100 % hold) ~600; 10438 (the top piece upright) in every experience bar with room
//   between the ends, ~2,500; 10439 (width for height) ~2,500; 10440 (inactive drawn) ~500; 10441 (the popup's pieces
//   from the top) ~3,000; 10442 (`< 0`) at exactly no room, ~400; 10443 (the clip to the window's bottom) ~2,500; 10444
//   (no `IMAGE` test) ~800; 10445 (the video's far corner) ~500.
//
// # Results (2026-11-18)
//
// 20,000 cases (power 6,042, experience 4,964, popup 4,006, right HUD 1,571, cameo 966, top 823, logN 1,628; 10,413
// cases drew images, 167,876 images of which 7,653 clipped, 484 videos, 795 default draws): **0 mismatches**, every word
// equal. Plants, mismatching cases: 10431 52; 10432 1,055; 10433 3,025; 10434 98; 10435 1,076; 10436 235; 10437 601;
// 10438 2,441; 10439 2,351; 10440 660; 10441 2,760; 10442 1,246 (the replacement, below); 10443 2,499; 10444 748;
// 10445 484. **15 of 15 caught.**
//
// **Before that, the harness missed three times:** the first run crashed (SIGSEGV), the two tests in this file running
// the C++ stand-ins' globals at once (they now take turns, `CPP`); the second overran the C++ output buffer (4,096 words:
// a power bar of 1-pixel pieces records ~400 images; now 65,536); the third, on 200 cases, had 6 right-HUD mismatches
// from the harness: an `IMAGE` window with no image draws nothing, so the Rust side could not see the default draw's
// call (each state's draw data 0 now carries a probe image or colour on the Rust side only). **Predictions missed:** the
// first 10442 (the no-room test at `< 0`) was caught 0 times: at exactly no room the two branches hand over the same
// two rectangles (the top piece at `bottom - bh - th` is the window's top), so the plant was an equivalent mutant; it was
// replaced. 10431 was caught 52 times, not ~80, and 10434 98, not ~150 (fewer of those cases drew a bar than guessed).
//
// # Planted bugs (#195)
//
// | id | planted bug |
// |---|---|
// | 10431 | `W3DPowerDraw`'s yellow band at `consumption >= production - range` |
// | 10432 | `W3DPowerDraw` does not hold the bar to the window's width |
// | 10433 | `W3DPowerDraw`'s last piece not clipped |
// | 10434 | `W3DPowerDraw`'s needle at `log(1)` for a consumption of 1 |
// | 10435 | `W3DPowerDraw`'s needle not right-aligned past the window |
// | 10436 | the float-to-`Int` conversion Rust's `as`, not x86's |
// | 10437 | `W3DCommandBarGenExpDraw` does not hold the progress to 100 |
// | 10438 | `W3DCommandBarGenExpDraw`'s top piece drawn upright |
// | 10439 | `W3DCommandBarGenExpDraw`'s pieces step by the image's width |
// | 10440 | `W3DCommandBarGenExpDraw` draws for an inactive player |
// | 10441 | `W3DCommandBarHelpPopupDraw`'s pieces start at the window's top |
// | 10442 | `W3DCommandBarHelpPopupDraw`'s no-room top piece not raised by its own height (replaced the first plant, the no-room test at `< 0`: see the results) |
// | 10443 | `W3DCommandBarHelpPopupDraw`'s clip to the window's bottom |
// | 10444 | `W3DRightHUDDraw` draws a window without `IMAGE` |
// | 10445 | `W3DCameoMovieDraw`'s far corner the window's size |

use rts_lockstep_engine::client::control_bar_popup::PopupWindowHost;
use rts_lockstep_engine::client::w3d_command_bar::{
    log_n, power_draw_energy, w3d_cameo_movie_draw, w3d_command_bar_gen_exp_draw, w3d_command_bar_help_popup_draw, w3d_command_bar_top_draw, w3d_power_draw, w3d_right_hud_draw, GenExpPoints, PowerBarGlobals,
};

/// The wrapper's named images (index 100 + place).
const NAMED: [&[u8]; 10] = [b"PowerPointY", b"PowerPointR", b"PowerPointG", b"PowerBarSlider", b"GenExpBarTop1", b"GenExpBarBottom1", b"GenExpBar1", b"Helpbox-top", b"Helpbox-bottom", b"Helpbox-middle"];

fn energy_value(g: &mut Gen) -> i32 {
    match g.below(20) {
        0 => -(g.below(1000) as i32) - 1,
        1..=3 => g.below(3) as i32,
        4..=14 => g.below(201) as i32,
        _ => 200 + g.below(999_801) as i32,
    }
}

fn geometry(g: &mut Gen, wmax: u32, hmax: u32) -> [i32; 6] {
    [place(g), place(g), place(g) / 2, place(g) / 2, size(g, wmax), size(g, hmax)]
}

fn gen_case_195(g: &mut Gen) -> Vec<i32> {
    let r = g.below(100);
    let kind = match r {
        0..=29 => 4,
        30..=54 => 5,
        55..=74 => 6,
        75..=82 => 7,
        83..=87 => 8,
        88..=91 => 9,
        _ => 10,
    };
    let mut v = vec![kind];
    match kind {
        4 => {
            v.extend(geometry(g, 400, 60));
            let production = energy_value(g);
            let consumption = if g.below(2) == 0 { production.wrapping_add(g.below(21) as i32 - 10) } else { energy_value(g) };
            let local = (production, consumption);
            let look = (energy_value(g), energy_value(g));
            v.extend([(g.below(5) == 0) as i32, (g.below(4) != 0) as i32, (g.below(10) != 0) as i32, local.0, local.1, look.0, look.1]);
            let (base, intervals, yellow) = if g.below(2) == 0 {
                (7, 3.0f32, 5)
            } else {
                (g.below(13) as i32, if g.below(20) == 0 { 0.0 } else { g.uniform(0.0, 10.0) }, g.below(21) as i32)
            };
            v.extend([(g.below(10) != 0) as i32, base, intervals.to_bits() as i32, yellow]);
            for i in 0..4 {
                let w = if i == 3 { g.below(25) as i32 } else { 1 + g.below(24) as i32 };
                v.extend([w, g.below(25) as i32]);
            }
        }
        5 => {
            v.extend(geometry(g, 100, 300));
            let down = g.below(5001) as i32;
            let up = match g.below(10) {
                0 => i32::MAX,
                1 => down - 1 - g.below(500) as i32,
                _ => down + 1 + g.below(5000) as i32,
            };
            let span = (up as i64 - down as i64).clamp(-5000, 5000);
            let skill = (down as i64 - 500 + g.below((500 + span.abs() * 13 / 10 + 1) as u32) as i64) as i32;
            v.extend([(g.below(5) != 0) as i32, skill, up, down]);
            for _ in 0..3 {
                v.extend([1 + g.below(30) as i32, 1 + g.below(30) as i32]);
            }
        }
        6 => {
            let mut geo = geometry(g, 400, 300);
            let sizes: Vec<i32> = (0..6).map(|_| 1 + g.below(40) as i32).collect();
            if g.below(8) == 0 {
                geo[5] = sizes[1] + sizes[3];
            }
            v.extend(geo);
            v.extend(sizes);
        }
        7 => {
            v.extend(geometry(g, 400, 400));
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
            v.extend([status as i32, state, color(g), color(g)]);
        }
        8 => {
            v.extend(geometry(g, 400, 400));
            v.push(g.below(2) as i32);
        }
        9 => {
            v.extend(geometry(g, 400, 400));
            v.extend([g.below(2) as i32, g.below(2) as i32, g.below(2) as i32]);
        }
        _ => {
            let value = match g.below(6) {
                0 => g.below(1_000_001) as f32,
                1 => 1.5,
                2 => g.below(3) as f32,
                3 => -(g.below(100) as f32),
                4 => f32::from_bits(g.u32()),
                _ => g.uniform(0.0, 300.0),
            };
            let base = match g.below(4) {
                0 => 7.0,
                1 => g.below(13) as f32,
                2 => f32::from_bits(g.u32()),
                _ => g.uniform(0.0, 20.0),
            };
            v.extend([value.to_bits() as i32, base.to_bits() as i32]);
        }
    }
    v
}

/// `PopupWindowHost` from a power case's players.
struct Players {
    observer: bool,
    look: Option<(i32, i32)>,
    local: Option<(i32, i32)>,
}

impl PopupWindowHost for Players {
    fn fetch_text(&self, _label: &[u8]) -> Vec<u16> {
        Vec::new()
    }
    fn is_observer_control_bar_on(&self) -> bool {
        self.observer
    }
    fn observer_look_at_player_energy(&self) -> Option<(i32, i32)> {
        self.look
    }
    fn local_energy(&self) -> Option<(i32, i32)> {
        self.local
    }
}

fn image_records(out: &mut Vec<i32>, items: &[DrawItem]) {
    for d in items {
        match d {
            DrawItem::Image { image, x0, y0, x1, y1, color, clip, .. } => {
                let idx = NAMED.iter().position(|n| *n == &image[..]).map_or(-1, |i| 100 + i as i32);
                let (on, c) = clip.map_or((0, (0, 0, 0, 0)), |c| (1, c));
                out.extend([30, idx, *x0, *y0, *x1, *y1, *color as i32, on, c.0, c.1, c.2, c.3]);
            }
            _ => out.push(97),
        }
    }
}

fn rust_case_195(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    if input[0] == 10 {
        let r = log_n(f32::from_bits(input[1] as u32), f32::from_bits(input[2] as u32));
        out.extend([33, r.to_bits() as i32]);
        return out;
    }
    let mut wm = GameWindowManager::new((800, 600));
    let p = window(&mut wm, None, input[1], input[2], 10, 10);
    let w = window(&mut wm, Some(p), input[3], input[4], input[5], input[6]);
    let mut items = Vec::new();
    match input[0] {
        4 => {
            let players = Players { observer: input[7] != 0, look: (input[8] != 0).then_some((input[12], input[13])), local: (input[9] != 0).then_some((input[10], input[11])) };
            let globals = (input[14] != 0).then_some(PowerBarGlobals { base: input[15], intervals: f32::from_bits(input[16] as u32), yellow_range: input[17] });
            let sizes: Vec<(i32, i32)> = (0..4).map(|i| (input[18 + i * 2], input[19 + i * 2])).collect();
            let find = |n: &[u8]| NAMED.iter().position(|m| *m == n).map(|i| if i < 4 { sizes[i] } else { (0, 0) });
            w3d_power_draw(&mut wm, w, power_draw_energy(&players), globals, find, &mut items);
            image_records(&mut out, &items);
        }
        5 => {
            let player = GenExpPoints { active: input[7] != 0, skill_points: input[8], level_up: input[9], level_down: input[10] };
            let sizes: Vec<(i32, i32)> = (0..3).map(|i| (input[11 + i * 2], input[12 + i * 2])).collect();
            let find = |n: &[u8]| NAMED.iter().position(|m| *m == n).map(|i| if (4..7).contains(&i) { sizes[i - 4] } else { (0, 0) });
            w3d_command_bar_gen_exp_draw(&mut wm, w, player, find, &mut items);
            image_records(&mut out, &items);
        }
        6 => {
            let sizes: Vec<(i32, i32)> = (0..3).map(|i| (input[7 + i * 2], input[8 + i * 2])).collect();
            let find = |n: &[u8]| NAMED.iter().position(|m| *m == n).map(|i| if i >= 7 { sizes[i - 7] } else { (0, 0) });
            w3d_command_bar_help_popup_draw(&mut wm, w, find, &mut items);
            image_records(&mut out, &items);
        }
        7 => {
            let win = &mut wm.tree.windows[w];
            win.status = input[7] as u32;
            win.inst.state = input[8];
            win.inst.enabled_draw_data[0].border_color = input[9];
            win.inst.enabled_draw_data[0].color = input[10];
            // (the C++ stand-in records the call whatever it would draw: here every state draws something, an image
            // or a colour, so the call shows, except an enabled colour window whose two colours are undefined)
            for d in [&mut win.inst.enabled_draw_data[0], &mut win.inst.disabled_draw_data[0], &mut win.inst.hilite_draw_data[0]] {
                d.image = Some(b"Probe".to_vec());
            }
            win.inst.disabled_draw_data[0].color = 0x55;
            win.inst.hilite_draw_data[0].color = 0x55;
            w3d_right_hud_draw(&wm, w, &mut items);
            if !items.is_empty() {
                let mut default = Vec::new();
                w3d_game_win_default_draw(&wm, w, &mut default);
                if items != default {
                    out.push(99);
                }
                out.extend([1, (input[7] as u32 & win_status::IMAGE != 0) as i32, wm.tree.windows[w].inst.enabled_draw_data[0].border_color]);
            }
        }
        8 => {
            if let Some(r) = w3d_cameo_movie_draw(&wm, w, input[7] != 0) {
                out.push(31);
                out.extend(r);
            }
        }
        _ => {
            w3d_command_bar_top_draw(&mut items);
            image_records(&mut out, &items);
        }
    }
    out.extend([32, wm_clip_on(&wm)]);
    out
}

/// The display's clip flag at the end.
fn wm_clip_on(wm: &GameWindowManager) -> i32 {
    wm.display_clip().is_some() as i32
}

#[test]
fn the_other_command_bar_draws_are_upstreams() {
    let cases: u64 = std::env::var("COMMAND_BAR_DRAW_195_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x195_0bf4);
    let names = ["W3DPowerDraw", "W3DCommandBarGenExpDraw", "W3DCommandBarHelpPopupDraw", "W3DRightHUDDraw", "W3DCameoMovieDraw", "W3DCommandBarTopDraw", "logN"];
    let mut checks: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    let mut kinds = [0u64; 7];
    let (mut images, mut clipped, mut drawn, mut videos, mut defaults) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case_195(&mut g);
        let k = input[0] as usize - 4;
        kinds[k] += 1;
        let r = rust_case_195(&input);
        let c = cpp_case(&input);
        let mut j = 0;
        let mut any = false;
        while j < c.len() {
            let n = match c[j] {
                30 => 12,
                31 => 5,
                1 => 3,
                32 | 33 => 2,
                _ => 1,
            };
            match c[j] {
                30 => {
                    images += 1;
                    any = true;
                    if c[j + 7] != 0 {
                        clipped += 1;
                    }
                }
                31 => videos += 1,
                1 => defaults += 1,
                _ => {}
            }
            j += n;
        }
        drawn += any as u64;
        if r != c && !planted {
            println!("case {i} {input:?}:\n rust {r:?}\n c++  {c:?}");
        }
        checks[k].eq(r, c, || format!("case {i} {input:?}"));
    }
    println!("{cases} cases (power, experience, popup, right HUD, cameo, top, logN: {kinds:?}): {drawn} drew images, {images} images ({clipped} clipped), {videos} videos, {defaults} default draws");
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
