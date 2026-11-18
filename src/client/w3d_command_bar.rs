// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DControlBar.cpp
//     (W3DCommandBarGridDraw; #195: W3DCameoMovieDraw, W3DRightHUDDraw, logN, W3DPowerDraw, W3DCommandBarGenExpDraw,
//     W3DCommandBarTopDraw, W3DCommandBarHelpPopupDraw)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowGlobal.cpp (GameWindowManager::winDrawImage, #195)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (drawRectClock,
//     drawRemainingRectClock: the rectangles and triangles they hand the 2D renderer)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindow.cpp (GameWindow::winSetEnabledBorderColor)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (updateBuildQueueDisabledImages,
//     updateRightHUDImage)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp (ControlBarScheme::init's
//     updateBuildQueueDisabledImages and updateRightHUDImage lines)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The command bar's own drawing in the scheme's colours (GitHub #194): the grid over the command bar in
//! `getBorderColor` (`W3DCommandBarGridDraw`), and the clocks a push button draws over itself (`drawRectClock`,
//! `drawRemainingRectClock`), which is how `GadgetButtonDrawInverseClock`'s build-up clock colour (`m_buildUpClockColor`)
//! reaches the screen. With them, the two lines of `ControlBarScheme::init` that give the bar images:
//! `updateBuildQueueDisabledImages` and `updateRightHUDImage`.
//!
//! # As upstream reads it
//!
//! - `W3DCommandBarGridDraw` (W3DControlBar.cpp:466), a window's draw callback: a window with `WIN_STATUS_IMAGE` draws as
//!   `W3DGameWinDefaultDraw` alone. Any other first takes the bar's border colour as its **enabled border colour 0**
//!   (`winSetEnabledBorderColor(0, color)`, kept on the window), draws as `W3DGameWinDefaultDraw`, then four one-pixel
//!   lines in that colour: across at a third (`.33`) and two thirds (`.66`) of its height, down at a third and two thirds
//!   of its width. Each fraction is a `double` product added to the `Int` position and truncated to `Int` by
//!   `drawLine`'s parameters.
//! - `W3DDisplay::drawRectClock(x, y, w, h, percent, color)` (W3DDisplay.cpp:2287): nothing outside 1..=100; 100 is the
//!   whole rectangle; otherwise the window's four quarters (1 top right, 2 bottom right, 3 bottom left, 4 top left) fill
//!   clockwise from the top centre: the whole quarters done as rectangles, the quarter in progress as one or two
//!   triangles from the centre (each quarter is two eighths of 12 and 13 percent).
//! - `W3DDisplay::drawRemainingRectClock` (W3DDisplay.cpp:2451, "Variation added by Kris"): nothing outside 0..=99; it
//!   covers what is **left** of the clock, from `percent` to 100, so an icon is "revealed" as it completes: 0 is the
//!   whole rectangle.
//! - `ControlBar::updateBuildQueueDisabledImages(image)` (ControlBar.cpp:2856): nothing without an image; else each of
//!   the nine `ButtonQueue%02d` under `CP_BUILD_QUEUE` gets it as `GadgetButtonSetDisabledImage` (disabled image 0, and
//!   enabled images 5 and 6 cleared). `ControlBar::updateRightHUDImage(image)` (:2896): nothing without the right HUD
//!   window or an image; else its enabled image 0.
//!
//! # Upstream quirks that are kept
//!
//! - The grid's colour stays on the window as its enabled border colour 0, and the default draw uses it only when the
//!   window draws its enabled state (a disabled or hilited window draws its own border colour).
//! - `drawRectClock`'s eighths are 12 and 13 percent wide but the thresholds are `> 12`, so 13 % of a quarter draws a
//!   full first eighth and a 1/13 second one; `drawRemainingRectClock` splits at 13 / 38 / 63 and **87** (`< 87`), but
//!   its last eighth subtracts 88 (`percent - 88`), so at 87 the last triangle starts at `-1/12` of the half width.
//! - `drawRemainingRectClock`'s 62..74 % triangle subtracts 62 though its range starts at 63.
//! - A missing queue button (the layout without `ButtonQueue%02d`) is dereferenced upstream: here it is skipped.
//!
//! # Where this differs from upstream
//!
//! The clocks return the pieces the 2D renderer is given (`Add_Rect(RectClass(..), 0, 0, color)` and
//! `Add_Tri(v0, v1, v2, 0, 0, 0, color)`, in order) instead of rendering them; the client fills them
//! (`client/src/ui.rs`'s `draw_list_quads`). `Reset`, `Enable_Texturing(FALSE)` and `Render` are implied by a
//! `Some`.
//!
//! # Verification
//!
//! `tests/differential_command_bar_draw.rs` (2026-11-17): against the C++ cut verbatim (the grid with
//! `winSetEnabledBorderColor`, both clocks, and `init`'s two image lines with `GadgetButtonSetDisabledImage`), 20,000
//! cases (6,007 grids, 5,962 clocks, 6,034 remaining clocks, 1,997 image updates; 15,999 triangles): **0 mismatches**,
//! every float bit-identical; planted bugs 10416 .. 10430 all caught.
//!
//! # The other draw callbacks (GitHub #195)
//!
//! - `W3DCameoMovieDraw` (W3DControlBar.cpp:49): the cameo video buffer over the window, if there is one (never in the
//!   port: Bink was never released).
//! - `W3DRightHUDDraw` (:98): `W3DGameWinDefaultDraw` for an `IMAGE` window, nothing for any other.
//! - `logN` (:107) and `W3DPowerDraw` (:118): the power bar, a log scale of `m_powerBarIntervals` decades of
//!   `m_powerBarBase` across the window: `PowerPointG`/`Y`/`R` repeated to `logN(production)`, the last piece clipped at the
//!   window's right, then `PowerBarSlider` at `logN(consumption)`. `W3DPowerDrawA` (:285) is not translated: no
//!   lexicon names it (W3DFunctionLexicon.cpp:72-94).
//! - `W3DCommandBarGenExpDraw` (:492): the general's experience as a bar rising from the window's bottom.
//! - `W3DCommandBarTopDraw` (:616): draws nothing (its drawing is commented out upstream).
//! - `W3DCommandBarHelpPopupDraw` (:917): the popup's frame, top, repeated middle (the last piece clipped), bottom.
//!
//! Upstream quirks kept: `logN`'s numerator is rounded to `Real` and divided by the double `log10(base)`; a float
//! converted to `Int` is x86's (`0x80000000` for NaN, infinities and out of range: [`x86_float_to_int`]), so a
//! consumption of 0 puts the needle at `0x80000000` and it ends left-aligned; the experience bar's top piece is handed
//! over with its rectangle upward (y0 below y1), which the 2D renderer draws mirrored. Where it differs: upstream finds
//! its images once (function statics), here each call; a zero-sized centre image or `levelUp == levelDown`, which upstream
//! divides by (a crash), draw nothing here.
//!
//! `tests/differential_command_bar_draw.rs`'s `the_other_command_bar_draws_are_upstreams` (2026-11-18): 20,000 cases
//! against the C++ cut verbatim, **0 mismatches**; planted bugs 10431 .. 10445 all caught.
//!
//! # Representation
//!
//! Free functions; [`ClockPiece`] for the renderer's calls.

use crate::client::window_draw::w3d_game_win_default_draw;
use crate::client::window_manager::{DrawItem, GameWindowManager, WIN_ERR_INVALID_PARAMETER, WIN_ERR_OK};
use crate::client::window_script::{win_status, WinId, WindowTree, MAX_DRAW_DATA};
use crate::logic::modules::plant;

/// One call the clocks make on the 2D renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClockPiece {
    /// `Add_Rect(RectClass(left, top, right, bottom), 0, 0, color)`.
    Rect([f32; 4]),
    /// `Add_Tri(Vector2(x0, y0), Vector2(x1, y1), Vector2(x2, y2), Vector2(0,0), Vector2(0,0), Vector2(0,0), color)`.
    Tri([f32; 6]),
}

fn rect(out: &mut Vec<ClockPiece>, l: i32, t: i32, r: i32, b: i32) {
    out.push(ClockPiece::Rect([l as f32, t as f32, r as f32, b as f32]));
}

/// `GameWindow::winSetEnabledBorderColor(index, color)` (GameWindow.cpp:1606): the enabled border colour at `index`;
/// `WIN_ERR_INVALID_PARAMETER` outside `0..MAX_DRAW_DATA`.
pub fn win_set_enabled_border_color(tree: &mut WindowTree, w: WinId, index: i32, color: i32) -> i32 {
    if index < 0 || index >= MAX_DRAW_DATA as i32 {
        return WIN_ERR_INVALID_PARAMETER;
    }
    tree.windows[w].inst.enabled_draw_data[index as usize].border_color = color;
    WIN_ERR_OK
}

/// `W3DCommandBarGridDraw(window, instData)` (W3DControlBar.cpp:466): `border_color` is `TheControlBar->getBorderColor()`
/// (`client::control_bar_scheme::ControlBarColors::get_border_color`).
pub fn w3d_command_bar_grid_draw(wm: &mut GameWindowManager, w: WinId, border_color: i32, out: &mut Vec<DrawItem>) {
    if wm.tree.windows[w].status & win_status::IMAGE != 0 && !plant(10418) {
        w3d_game_win_default_draw(wm, w, out);
        return;
    }
    let pos = wm.tree.screen_position(w);
    let size = wm.tree.windows[w].size;
    let color = border_color;
    if !plant(10417) {
        win_set_enabled_border_color(&mut wm.tree, w, 0, color);
    }
    w3d_game_win_default_draw(wm, w, out);
    // (`Int + Int * double`, truncated to `drawLine`'s `Int`)
    let at = |p: i32, s: i32, f: f64| -> i32 {
        let v = p as f64 + s as f64 * f;
        if plant(10416) {
            (v + 0.5) as i32
        } else {
            v as i32
        }
    };
    let across = if plant(10419) { size.1 } else { size.0 };
    out.push(DrawItem::Line { x0: pos.0, y0: at(pos.1, size.1, 0.33), x1: pos.0 + size.0, y1: at(pos.1, size.1, 0.33), color });
    out.push(DrawItem::Line { x0: pos.0, y0: at(pos.1, size.1, 0.66), x1: pos.0 + size.0, y1: at(pos.1, size.1, 0.66), color });
    out.push(DrawItem::Line { x0: at(pos.0, across, 0.33), y0: pos.1, x1: at(pos.0, across, 0.33), y1: pos.1 + size.1, color });
    out.push(DrawItem::Line { x0: at(pos.0, across, 0.66), y0: pos.1, x1: at(pos.0, across, 0.66), y1: pos.1 + size.1, color });
}

/// `W3DDisplay::drawRectClock(startX, startY, width, height, percent, color)` (W3DDisplay.cpp:2287): the pieces, in
/// order, all in `color`; `None` when it returns before `Reset` (`percent` outside 1..=100).
pub fn draw_rect_clock(start_x: i32, start_y: i32, width: i32, height: i32, percent: i32) -> Option<Vec<ClockPiece>> {
    // sanity
    if (percent < 1 && !plant(10421)) || percent > 100 {
        return None;
    }
    let mut out = Vec::new();
    let (sx, sy, w, h) = (start_x, start_y, width, height);
    let (fx, fy) = (sx as f32, sy as f32);
    let tri = |out: &mut Vec<ClockPiece>, v: [f32; 6]| out.push(ClockPiece::Tri(v));
    if percent == 100 && !plant(10420) {
        // we're done, lets just draw one rectangle for it all.
        rect(&mut out, sx, sy, sx + w, sy + h);
    } else if percent > 75 {
        //rectangle #1 & 2
        rect(&mut out, sx + w / 2, sy, sx + w, sy + h);
        // rectangle #3
        rect(&mut out, sx, sy + h / 2, sx + w / 2, sy + h);
        // draw the part of rectangle 4
        let remain = (percent - 75) as f32;
        if remain > 12.0 {
            //draw the full triangle
            tri(&mut out, [fx, fy, fx, (sy + h / 2) as f32, (sx + w / 2) as f32, (sy + h / 2) as f32]);
            // draw the part of triangle
            let percent_draw = (remain - 12.0) / if plant(10422) { 12.0 } else { 13.0 };
            tri(&mut out, [fx, fy, (sx + w / 2) as f32, (sy + h / 2) as f32, fx + (w / 2) as f32 * percent_draw, fy]);
        } else {
            // draw the part of triangle
            let percent_draw = remain / 12.0;
            tri(&mut out, [fx, (sy + h / 2) as f32 - (h / 2) as f32 * percent_draw, fx, (sy + h / 2) as f32, (sx + w / 2) as f32, (sy + h / 2) as f32]);
        }
    } else if percent > 50 {
        //rectangle #1 & 2
        rect(&mut out, sx + w / 2, sy, sx + w, sy + h);
        // draw the part of rectangle 3
        let remain = (percent - 50) as f32;
        if remain > 12.0 {
            //draw the full triangle
            tri(&mut out, [(sx + w / 2) as f32, (sy + h / 2) as f32, fx, (sy + h) as f32, (sx + w / 2) as f32, (sy + h) as f32]);
            // draw the part of triangle
            let percent_draw = (remain - 12.0) / 13.0;
            tri(&mut out, [fx, (sy + h) as f32 - (h / 2) as f32 * percent_draw, fx, (sy + h) as f32, (sx + w / 2) as f32, (sy + h / 2) as f32]);
        } else {
            // draw the part of triangle
            let percent_draw = remain / 12.0;
            tri(&mut out, [(sx + w / 2) as f32, (sy + h) as f32, (sx + w / 2) as f32, (sy + h / 2) as f32, (sx + w / 2) as f32 - (w / 2) as f32 * percent_draw, (sy + h) as f32]);
        }
    } else if percent > 25 {
        // rectangel #1
        rect(&mut out, sx + w / 2, sy, sx + w, sy + h / 2);
        // draw the part of rectangle 2
        let remain = (percent - 25) as f32;
        if remain > 12.0 {
            //draw the full triangle
            tri(&mut out, [(sx + w / 2) as f32, (sy + h / 2) as f32, (sx + w) as f32, (sy + h) as f32, (sx + w) as f32, (sy + h / 2) as f32]);
            // draw the part of triangle
            let percent_draw = (remain - 12.0) / 13.0;
            tri(&mut out, [(sx + w / 2) as f32, (sy + h / 2) as f32, (sx + w) as f32 - (w / 2) as f32 * percent_draw, (sy + h) as f32, (sx + w) as f32, (sy + h) as f32]);
        } else {
            // draw the part of triangle
            let percent_draw = remain / 12.0;
            tri(&mut out, [(sx + w) as f32, (sy + h / 2) as f32, (sx + w / 2) as f32, (sy + h / 2) as f32, (sx + w) as f32, (sy + h / 2) as f32 + (h / 2) as f32 * percent_draw]);
        }
    } else {
        // draw the part of rectangle 1
        if percent > 12 {
            //draw the full triangle
            tri(&mut out, [(sx + w / 2) as f32, fy, (sx + w / 2) as f32, (sy + h / 2) as f32, (sx + w) as f32, fy]);
            // draw the part of triangle
            let percent_draw = (percent - 12) as f32 / 13.0;
            let half = if plant(10423) { h } else { h / 2 };
            tri(&mut out, [(sx + w) as f32, fy, (sx + w / 2) as f32, (sy + h / 2) as f32, (sx + w) as f32, fy + half as f32 * percent_draw]);
        } else {
            // draw the part of triangle
            let percent_draw = percent as f32 / 12.0;
            tri(&mut out, [(sx + w / 2) as f32, fy, (sx + w / 2) as f32, (sy + h / 2) as f32, (sx + w / 2) as f32 + (w / 2) as f32 * percent_draw, fy]);
        }
    }
    Some(out)
}

/// `W3DDisplay::drawRemainingRectClock(startX, startY, width, height, percent, color)` (W3DDisplay.cpp:2451): the
/// pieces covering the clock from `percent` to 100, in order, all in `color`; `None` when it returns before `Reset`
/// (`percent` outside 0..=99).
pub fn draw_remaining_rect_clock(start_x: i32, start_y: i32, width: i32, height: i32, percent: i32) -> Option<Vec<ClockPiece>> {
    // sanity
    if percent < 0 || (percent > 99 && !plant(10424)) || percent > 100 {
        return None;
    }
    let mut out = Vec::new();
    let half_w = if plant(10428) { (width + 1) / 2 } else { width / 2 };
    let mid_x = start_x + half_w;
    let mid_y = start_y + height / 2;
    let end_x = start_x + width;
    let end_y = start_y + height;
    let half_width = width / 2;
    let half_height = height / 2;
    let (sx, sy, mx, my, ex, ey) = (start_x as f32, start_y as f32, mid_x as f32, mid_y as f32, end_x as f32, end_y as f32);
    let (hw, hh) = (half_width as f32, half_height as f32);
    let tri = |out: &mut Vec<ClockPiece>, v: [f32; 6]| out.push(ClockPiece::Tri(v));
    if percent == 0 {
        // We just started, so draw the entire remaining rectangle.
        rect(&mut out, start_x, start_y, end_x, end_y);
    } else if percent < 25 {
        //Rectangle #3 & 4
        rect(&mut out, start_x, start_y, mid_x, end_y);
        //Rectangle #2
        rect(&mut out, mid_x, mid_y, end_x, end_y);
        //Handle rectangle #1 than needs partial rendering.
        let first = if plant(10425) { percent <= 13 } else { percent < 13 };
        if first {
            //Draw the 2nd half of rectangle #1
            tri(&mut out, [mx, my, ex, my, ex, sy]);
            //Draw the last part of the 1st portion of rectangle #1
            let percent_draw = (13 - percent) as f32 / 13.0;
            tri(&mut out, [mx, my, ex, sy, ex - hw * percent_draw, sy]);
        } else {
            //Draw the last part of the 2nd half of rectangle #1
            let percent_draw = (percent - 13) as f32 / 12.0;
            tri(&mut out, [mx, my, ex, my, ex, sy + hh * percent_draw]);
        }
    } else if percent < 50 {
        //rectangle #3 & 4
        rect(&mut out, start_x, start_y, mid_x, end_y);
        //Handle rectangle #2 that needs partial rendering.
        if percent < 38 {
            //Draw the 2nd half of rectangle #2
            tri(&mut out, [mx, my, mx, ey, ex, ey]);
            //Draw the last part of the 1st portion of rectangle #2
            let percent_draw = (percent - 25) as f32 / 13.0;
            tri(&mut out, [mx, my, ex, ey, ex, my + hh * percent_draw]);
        } else {
            //Draw the last part of the 2nd half of rectangle #1
            let percent_draw = (percent - 38) as f32 / if plant(10426) { 13.0 } else { 12.0 };
            tri(&mut out, [mx, my, mx, ey, ex - hw * percent_draw, ey]);
        }
    } else if percent < 75 {
        //Rectangle #4
        rect(&mut out, start_x, start_y, mid_x, mid_y);
        //Handle rectangle #3 that needs partial rendering.
        if percent < 63 {
            //Draw the 2nd half of rectangle #3
            tri(&mut out, [mx, my, sx, my, sx, ey]);
            //Draw the last part of the 1st portion of rectangle #3
            let percent_draw = (percent - 50) as f32 / 13.0;
            tri(&mut out, [mx, my, sx, ey, mx - hw * percent_draw, ey]);
        } else {
            //Draw the last part of the 2nd half of rectangle #3
            let percent_draw = (percent - 62) as f32 / 12.0;
            tri(&mut out, [mx, my, sx, my, sx, ey - hh * percent_draw]);
        }
    } else {
        //Handle rectangle #4 that needs partial rendering.
        let first = if plant(10427) { percent < 88 } else { percent < 87 };
        if first {
            //Draw the 2nd half of rectangle #4
            tri(&mut out, [mx, my, mx, sy, sx, sy]);
            //Draw the last part of the 1st portion of rectangle #4
            let percent_draw = (percent - 75) as f32 / 13.0;
            tri(&mut out, [mx, my, sx, sy, sx, my - hh * percent_draw]);
        } else {
            //Draw the last part of the 2nd half of rectangle #4
            let percent_draw = (percent - 88) as f32 / 12.0;
            tri(&mut out, [mx, my, mx, sy, sx + hw * percent_draw, sy]);
        }
    }
    Some(out)
}

/// The clock a push button draws (`W3DGadgetPushButtonDraw`'s `pData->drawClock`, W3DPushButton.cpp:259-268):
/// `drawRemainingRectClock` for an inverse clock (`GadgetButtonDrawInverseClock`), else `drawRectClock`.
pub fn push_button_clock(x: i32, y: i32, w: i32, h: i32, percent: i32, inverse: bool) -> Option<Vec<ClockPiece>> {
    if inverse {
        draw_remaining_rect_clock(x, y, w, h, percent)
    } else {
        draw_rect_clock(x, y, w, h, percent)
    }
}

/// `ControlBar::updateBuildQueueDisabledImages(image)` (ControlBar.cpp:2856), called by `ControlBarScheme::init`
/// (ControlBarScheme.cpp:430) with the scheme's `QueueButtonImage`: `queue` is the nine `ButtonQueue%02d` under
/// `CP_BUILD_QUEUE` (`None` where the layout has none: skipped, where upstream dereferences it).
pub fn update_build_queue_disabled_images(tree: &mut WindowTree, queue: &[Option<WinId>], image: Option<Vec<u8>>) {
    let Some(image) = image else { return };
    for w in queue.iter().take(crate::client::control_bar_command::MAX_BUILD_QUEUE_BUTTONS).flatten() {
        if plant(10429) {
            tree.windows[*w].inst.disabled_draw_data[0].image = Some(image.clone());
        } else {
            crate::client::control_bar_scheme::gadget_button_set_disabled_image(tree, *w, Some(image.clone()));
        }
    }
}

/// `ControlBar::updateRightHUDImage(image)` (ControlBar.cpp:2896), called by `ControlBarScheme::init`
/// (ControlBarScheme.cpp:431) with the scheme's `RightHUDImage`: `m_rightHUDWindow` (`ControlBar.wnd:RightHUD`) gets it as
/// its enabled image 0 (`winSetEnabledImage(0, image)`).
pub fn update_right_hud_image(tree: &mut WindowTree, right_hud: Option<WinId>, image: Option<Vec<u8>>) {
    let Some(w) = right_hud else { return };
    if image.is_none() && !plant(10430) {
        return;
    }
    tree.windows[w].inst.enabled_draw_data[0].image = image;
}

// ---------------------------------------------------------------------------------------------------------------------
// GitHub #195: the other draw callbacks of W3DControlBar.cpp.
// ---------------------------------------------------------------------------------------------------------------------

/// The power bar's three `GameData.ini` values `W3DPowerDraw` reads from `TheGlobalData` (`m_powerBarBase`,
/// `m_powerBarIntervals`, `m_powerBarYellowRange`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowerBarGlobals {
    pub base: i32,
    pub intervals: f32,
    pub yellow_range: i32,
}

impl PowerBarGlobals {
    /// The three from a `GlobalData`.
    pub fn of(g: &crate::common::global_data::GlobalData) -> Self {
        PowerBarGlobals { base: g.power_bar_base, intervals: g.power_bar_intervals, yellow_range: g.power_bar_yellow_range }
    }
}

/// What `W3DCommandBarGenExpDraw` reads of the local player: `isPlayerActive()`, `getSkillPoints()`,
/// `getSkillPointsLevelUp()` (`m_levelUp`) and `getSkillPointsLevelDown()` (`m_levelDown`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenExpPoints {
    pub active: bool,
    pub skill_points: i32,
    pub level_up: i32,
    pub level_down: i32,
}

/// A float converted to `Int` as upstream's x86 build does (`cvttss2si`, and MSVC's `_ftol`): truncated toward zero,
/// and the "integer indefinite" `0x80000000` for a NaN or a value outside `Int`'s range (where Rust's `as` saturates, and
/// gives 0 for a NaN).
pub fn x86_float_to_int(f: f32) -> i32 {
    if plant(10436) {
        return f as i32;
    }
    #[allow(clippy::eq_op)]
    let nan = f != f;
    if nan || f >= 2147483648.0 || f < -2147483648.0 {
        return i32::MIN;
    }
    f as i32
}

/// `logN(value, logBase)` (W3DControlBar.cpp:107): `(Real)log10(value)/ log10(logBase)`. Upstream's `log10` is C's double
/// one (MSVC's `math.h`; the differential build's shim `math.h` keeps it so): the numerator is rounded to `Real`, widened
/// again for the division by the double denominator, and the quotient rounded to the `Real` it returns. Through petir.
pub fn log_n(value: f32, log_base: f32) -> f32 {
    use petir::Real;
    let num = Real::log10(value as f64) as f32;
    (num as f64 / Real::log10(log_base as f64)) as f32
}

/// `GameWindowManager::winDrawImage(image, startX, startY, endX, endY, color = 0xFFFFFFFF)` (GameWindowGlobal.cpp:77):
/// `TheDisplay->drawImage` (`DRAW_IMAGE_ALPHA`), under the display's clip region (`wm.clip`) when it is enabled.
fn win_draw_image(wm: &GameWindowManager, image: &[u8], x0: i32, y0: i32, x1: i32, y1: i32, out: &mut Vec<DrawItem>) {
    out.push(DrawItem::Image { image: image.to_vec(), x0, y0, x1, y1, color: 0xFFFF_FFFF, grayscale: false, clip: wm.clip });
}

/// `W3DCameoMovieDraw(window, instData)` (W3DControlBar.cpp:49): with the in-game UI's cameo video buffer
/// (`TheInGameUI->cameoVideoBuffer()`, `video`), `TheDisplay->drawVideoBuffer(video, x, y, x + w, y + h)` over the
/// window: the rectangle returned. The port has no video buffer (Bink was never released), so the client never has one.
pub fn w3d_cameo_movie_draw(wm: &GameWindowManager, w: WinId, video: bool) -> Option<[i32; 4]> {
    if !video {
        return None;
    }
    let pos = wm.tree.screen_position(w);
    let size = wm.tree.windows[w].size;
    if plant(10445) {
        return Some([pos.0, pos.1, size.0, size.1]);
    }
    Some([pos.0, pos.1, pos.0 + size.0, pos.1 + size.1])
}

/// `W3DRightHUDDraw(window, instData)` (W3DControlBar.cpp:98): `W3DGameWinDefaultDraw` for a window with
/// `WIN_STATUS_IMAGE`, nothing for any other.
pub fn w3d_right_hud_draw(wm: &GameWindowManager, w: WinId, out: &mut Vec<DrawItem>) {
    if wm.tree.windows[w].status & win_status::IMAGE != 0 || plant(10444) {
        w3d_game_win_default_draw(wm, w, out);
    }
}

/// `W3DPowerDraw`'s player (W3DControlBar.cpp:136-147): `TheControlBar->getObserverLookAtPlayer()` with the observer's
/// bar on, else `ThePlayerList->getLocalPlayer()`; its energy's `(getProduction(), getConsumption())`, `None` without
/// the player (`getEnergy()` is never `NULL`: a member).
pub fn power_draw_energy(h: &impl crate::client::control_bar_popup::PopupWindowHost) -> Option<(i32, i32)> {
    if h.is_observer_control_bar_on() {
        h.observer_look_at_player_energy()
    } else {
        h.local_energy()
    }
}

/// `W3DPowerDraw(window, instData)` (W3DControlBar.cpp:118), "Temporary power meter drawing": `energy` is the player's
/// `(getProduction(), getConsumption())` (`TheControlBar->getObserverLookAtPlayer()`'s with the observer's bar on, else
/// the local player's; `None` without a player), `globals` `TheGlobalData`'s (`None` without it), `find`
/// `TheMappedImageCollection->findImageByName`'s image size (`None` for `NULL`).
///
/// The bar is `PowerPointG`, `PowerPointY` (consumption within `m_powerBarYellowRange` below production) or `PowerPointR`
/// (over it) repeated from the window's left over `logN(production, base) x width / intervals` pixels, the last piece
/// clipped at the window's right; then the `PowerBarSlider` at `logN(consumption)` on the same scale (1 drawn as 1.5),
/// centred there, right-aligned at the window's right, or left-aligned at its left.
pub fn w3d_power_draw(wm: &mut GameWindowManager, w: WinId, energy: Option<(i32, i32)>, globals: Option<PowerBarGlobals>, find: impl Fn(&[u8]) -> Option<(i32, i32)>, out: &mut Vec<DrawItem>) {
    // (the `static const Image *`s)
    let center_bar_yellow = find(b"PowerPointY").map(|s| (&b"PowerPointY"[..], s));
    let center_bar_red = find(b"PowerPointR").map(|s| (&b"PowerPointR"[..], s));
    let center_bar_green = find(b"PowerPointG").map(|s| (&b"PowerPointG"[..], s));
    let slider = find(b"PowerBarSlider").map(|s| (&b"PowerBarSlider"[..], s));
    let (Some((production, consumption)), Some(g)) = (energy, globals) else { return };
    let pos = wm.tree.screen_position(w);
    let size = wm.tree.windows[w].size;
    let delta = g.yellow_range;
    let yellow = if plant(10431) { consumption >= production.wrapping_sub(delta) } else { consumption > production.wrapping_sub(delta) };
    let center_bar = if yellow && consumption <= production {
        // 6 and 1 is Green, 6 and 2 is yellow, 6 and 6 is yellow
        center_bar_yellow
    } else if consumption > production {
        center_bar_red
    } else {
        center_bar_green
    };
    let (Some(slider), Some(center_bar)) = (slider, center_bar) else { return };
    // (`Int / Real`: the window's width over the intervals, a `Real`)
    let per = size.0 as f32 / g.intervals;
    let mut range = x86_float_to_int(log_n(production as f32, g.base as f32) * per);
    if range >= size.0 && !plant(10432) {
        range = size.0;
    }
    // draw the center repeating bar; get width we have to draw our repeating center in
    let mut center_width = range;
    if center_width > 0 {
        let (cw, _) = center_bar.1;
        // (upstream divides by a zero-width image: a crash; here nothing more is drawn)
        if cw == 0 {
            return;
        }
        // how many whole repeating pieces will fit in that width
        let pieces = center_width / cw;
        let mut start = pos;
        let end_y = start.1.wrapping_add(size.1);
        for _ in 0..pieces {
            let end_x = start.0.wrapping_add(cw);
            win_draw_image(wm, center_bar.0, start.0, start.1, end_x, end_y, out);
            start.0 = start.0.wrapping_add(cw);
        }
        // we will draw the image but clip the parts we don't want to show
        let reg = (start.0, start.1, pos.0.wrapping_add(size.0), pos.1.wrapping_add(size.1));
        center_width = pos.0.wrapping_add(size.0).wrapping_sub(start.0);
        if center_width > 0 {
            if !plant(10433) {
                wm.clip = Some(reg);
            }
            let end_x = start.0.wrapping_add(cw);
            win_draw_image(wm, center_bar.0, start.0, start.1, end_x, end_y, out);
            wm.clip = None;
        }
    }
    // Log(1) == 0, but we need to show something for 1 power used.
    let consumption_for_needle = if consumption == 1 && !plant(10434) { 1.5f32 } else { consumption as f32 };
    range = x86_float_to_int(log_n(consumption_for_needle, g.base as f32) * per);
    if center_width <= 0 && range <= 0 {
        return;
    }
    let (sw, sh) = slider.1;
    let (mut pos_x_start, mut pos_x_end) = if range >= size.0 && !plant(10435) {
        (pos.0.wrapping_add(size.0).wrapping_sub(sw), pos.0.wrapping_add(size.0))
    } else {
        (pos.0.wrapping_add(range).wrapping_sub(sw / 2), pos.0.wrapping_add(range).wrapping_add(sw / 2))
    };
    if pos_x_start <= pos.0 {
        pos_x_start = pos.0;
        pos_x_end = pos.0.wrapping_add(sw);
    }
    let bottom = pos.1.wrapping_add(size.1);
    win_draw_image(wm, slider.0, pos_x_start, bottom.wrapping_sub(sh), pos_x_end, bottom, out);
}

/// `W3DCommandBarGenExpDraw(window, instData)` (W3DControlBar.cpp:492): the general's experience toward the next rank
/// as a vertical bar rising from the window's bottom: `GenExpBarBottom1` at the bottom, `GenExpBar1` repeated above it
/// (the last piece clipped at the bottom image's top), `GenExpBarTop1` on top. Nothing for an inactive player or no
/// progress (`(skill - levelDown) x 100 / (levelUp - levelDown)`, held to 100). `find` as [`w3d_power_draw`]'s.
pub fn w3d_command_bar_gen_exp_draw(wm: &mut GameWindowManager, w: WinId, player: GenExpPoints, find: impl Fn(&[u8]) -> Option<(i32, i32)>, out: &mut Vec<DrawItem>) {
    if !player.active && !plant(10440) {
        return;
    }
    let end_bar = find(b"GenExpBarTop1").map(|s| (&b"GenExpBarTop1"[..], s));
    let begin_bar = find(b"GenExpBarBottom1").map(|s| (&b"GenExpBarBottom1"[..], s));
    let center_bar = find(b"GenExpBar1").map(|s| (&b"GenExpBar1"[..], s));
    // (upstream divides by `levelUp - levelDown` unguarded: zero, or `INT_MIN / -1`, is a crash; here nothing is drawn)
    let Some(mut progress) = player.skill_points.wrapping_sub(player.level_down).wrapping_mul(100).checked_div(player.level_up.wrapping_sub(player.level_down)) else { return };
    if progress <= 0 {
        return;
    }
    // GS This should never be necessary, but scripts can change the points required or even disable a level.
    if progress > 100 && !plant(10437) {
        progress = 100;
    }
    let pos = wm.tree.screen_position(w);
    let size = wm.tree.windows[w].size;
    let (Some(end_bar), Some(begin_bar), Some(center_bar)) = (end_bar, begin_bar, center_bar) else { return };
    let range = size.1.wrapping_mul(progress) / 100;
    // get image sizes for the ends
    let bottom_size = begin_bar.1;
    let top_size = end_bar.1;
    // get two key points used in the end drawing
    let bottom_end = (pos.0.wrapping_add(size.0), pos.1.wrapping_add(size.1).wrapping_sub(bottom_size.1));
    let top_start_y = pos.1.wrapping_add(size.1).wrapping_sub(range).wrapping_sub(top_size.1);
    // get width we have to draw our repeating center in
    let mut center_width = bottom_end.1.wrapping_sub(top_start_y);
    let right = pos.0.wrapping_add(size.0);
    let bottom = pos.1.wrapping_add(size.1);
    if center_width <= 0 {
        // draw left end
        win_draw_image(wm, begin_bar.0, pos.0, bottom.wrapping_sub(bottom_size.1), right, bottom, out);
        // draw right end
        let sy = bottom.wrapping_sub(bottom_size.1).wrapping_sub(top_size.1);
        win_draw_image(wm, end_bar.0, pos.0, sy, right, sy.wrapping_add(top_size.1), out);
    } else {
        let ch = if plant(10439) { center_bar.1 .0 } else { center_bar.1 .1 };
        // (a zero-height image: upstream divides by zero, a crash; here nothing is drawn)
        if ch == 0 {
            return;
        }
        // how many whole repeating pieces will fit in that width
        let pieces = center_width / ch;
        let mut start = (pos.0, top_start_y);
        let end_x = start.0.wrapping_add(size.0);
        for _ in 0..pieces {
            win_draw_image(wm, center_bar.0, start.0, start.1, end_x, start.1.wrapping_add(ch), out);
            start.1 = start.1.wrapping_add(ch);
        }
        // we will draw the image but clip the parts we don't want to show
        let reg = (start.0, start.1, bottom_end.0, bottom_end.1);
        center_width = bottom_end.1.wrapping_sub(start.1);
        if center_width > 0 {
            wm.clip = Some(reg);
            win_draw_image(wm, center_bar.0, start.0, start.1, end_x, start.1.wrapping_add(ch), out);
            wm.clip = None;
        }
        // draw left end
        win_draw_image(wm, begin_bar.0, pos.0, bottom_end.1, right, bottom, out);
        // draw right end (upstream's rectangle runs upward: its top edge below its bottom one)
        let y0 = bottom.wrapping_sub(range);
        let y1 = y0.wrapping_sub(top_size.1);
        if plant(10438) {
            win_draw_image(wm, end_bar.0, pos.0, y1, right, y0, out);
        } else {
            win_draw_image(wm, end_bar.0, pos.0, y0, right, y1, out);
        }
    }
}

/// `W3DCommandBarTopDraw(window, instData)` (W3DControlBar.cpp:616): it returns early without `ButtonGeneral`, with it
/// hidden or with the local player inactive, and otherwise reaches only commented-out code: it draws nothing either way.
pub fn w3d_command_bar_top_draw(_out: &mut Vec<DrawItem>) {}

/// `W3DCommandBarHelpPopupDraw(window, instData)` (W3DControlBar.cpp:917): the popup's frame, `Helpbox-top` at the
/// window's top, `Helpbox-middle` repeated below it (the last piece clipped at the bottom image's top), `Helpbox-bottom`
/// at its bottom; when there is no room between the ends, the bottom then the top piece stacked at the window's bottom.
/// `find` as [`w3d_power_draw`]'s.
pub fn w3d_command_bar_help_popup_draw(wm: &mut GameWindowManager, w: WinId, find: impl Fn(&[u8]) -> Option<(i32, i32)>, out: &mut Vec<DrawItem>) {
    let end_bar = find(b"Helpbox-top").map(|s| (&b"Helpbox-top"[..], s));
    let begin_bar = find(b"Helpbox-bottom").map(|s| (&b"Helpbox-bottom"[..], s));
    let center_bar = find(b"Helpbox-middle").map(|s| (&b"Helpbox-middle"[..], s));
    let pos = wm.tree.screen_position(w);
    let size = wm.tree.windows[w].size;
    let (Some(end_bar), Some(begin_bar), Some(center_bar)) = (end_bar, begin_bar, center_bar) else { return };
    let bottom_size = begin_bar.1;
    let top_size = end_bar.1;
    let right = pos.0.wrapping_add(size.0);
    let bottom = pos.1.wrapping_add(size.1);
    // get width we have to draw our repeating center in
    let mut center_width = size.1.wrapping_sub(top_size.1).wrapping_sub(bottom_size.1);
    if center_width <= 0 {
        // draw left end
        win_draw_image(wm, begin_bar.0, pos.0, bottom.wrapping_sub(bottom_size.1), right, bottom, out);
        // draw right end
        let sy = bottom.wrapping_sub(bottom_size.1).wrapping_sub(if plant(10442) { 0 } else { top_size.1 });
        win_draw_image(wm, end_bar.0, pos.0, sy, right, sy.wrapping_add(top_size.1), out);
    } else {
        let ch = center_bar.1 .1;
        // (a zero-height image: upstream divides by zero, a crash; here nothing is drawn)
        if ch == 0 {
            return;
        }
        // how many whole repeating pieces will fit in that width
        let pieces = center_width / ch;
        let mut start = (pos.0, if plant(10441) { pos.1 } else { pos.1.wrapping_add(top_size.1) });
        let end_x = start.0.wrapping_add(size.0);
        for _ in 0..pieces {
            win_draw_image(wm, center_bar.0, start.0, start.1, end_x, start.1.wrapping_add(ch), out);
            start.1 = start.1.wrapping_add(ch);
        }
        // we will draw the image but clip the parts we don't want to show
        let hi_y = if plant(10443) { bottom } else { bottom.wrapping_sub(bottom_size.1) };
        let reg = (start.0, start.1, right, hi_y);
        center_width = bottom.wrapping_sub(bottom_size.1).wrapping_sub(start.1);
        if center_width > 0 {
            wm.clip = Some(reg);
            win_draw_image(wm, center_bar.0, start.0, start.1, end_x, start.1.wrapping_add(ch), out);
            wm.clip = None;
        }
        // draw left end
        win_draw_image(wm, begin_bar.0, pos.0, bottom.wrapping_sub(bottom_size.1), right, bottom, out);
        // draw right end
        win_draw_image(wm, end_bar.0, pos.0, pos.1, right, pos.1.wrapping_add(top_size.1), out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #194: predicted from W3DDisplay.cpp before the run, on (10, 20) 40x30 (mid (30, 35), end (50, 50)):
    /// - `drawRectClock` 0 and 101: nothing; 100: one rectangle (10, 20, 50, 50); 6: one triangle from the top centre,
    ///   (30, 20), (30, 35), (30 + 20 * 0.5, 20) = (40, 20);
    /// - `drawRemainingRectClock` 100: nothing; 0: the whole rectangle; 50: the left half's top quarter (10, 20, 30,
    ///   35), the 2nd half of #3 (30,35)(10,35)(10,50), and the 1st portion's last part with `percentDraw` 0: (30,35)
    ///   (10,50)(30,50).
    #[test]
    fn the_clocks_pieces() {
        assert_eq!(draw_rect_clock(10, 20, 40, 30, 0), None);
        assert_eq!(draw_rect_clock(10, 20, 40, 30, 101), None);
        assert_eq!(draw_rect_clock(10, 20, 40, 30, 100), Some(vec![ClockPiece::Rect([10.0, 20.0, 50.0, 50.0])]));
        assert_eq!(draw_rect_clock(10, 20, 40, 30, 6), Some(vec![ClockPiece::Tri([30.0, 20.0, 30.0, 35.0, 40.0, 20.0])]));
        assert_eq!(draw_remaining_rect_clock(10, 20, 40, 30, 100), None);
        assert_eq!(draw_remaining_rect_clock(10, 20, 40, 30, 0), Some(vec![ClockPiece::Rect([10.0, 20.0, 50.0, 50.0])]));
        assert_eq!(
            draw_remaining_rect_clock(10, 20, 40, 30, 50),
            Some(vec![ClockPiece::Rect([10.0, 20.0, 30.0, 35.0]), ClockPiece::Tri([30.0, 35.0, 10.0, 35.0, 10.0, 50.0]), ClockPiece::Tri([30.0, 35.0, 10.0, 50.0, 30.0, 50.0])])
        );
    }

    /// #194: predicted from W3DControlBar.cpp before the run: a colour window at (10, 20) 100x60 with the border colour
    /// 0x11 draws the default draw with enabled border 0 = 0x11 (`OpenRect`, then its fill), then lines at y = 20 +
    /// 19.8 -> 39 and 20 + 39.6 -> 59, x = 10 + 33 = 43 and 10 + 66 = 76, all 0x11; with `IMAGE` only the default draw and
    /// the border untouched.
    #[test]
    fn the_grid_is_drawn_in_the_border_colour() {
        use crate::client::window_manager::{NoGuiHost, SystemFunc};
        use crate::client::window_script::WinInstanceData;
        let mut wm = GameWindowManager::new((800, 600));
        let mut inst = WinInstanceData::new();
        inst.enabled_draw_data[0].color = 0x22;
        let w = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, 10, 20, 100, 60, SystemFunc::Default, Some(inst));
        let mut out = Vec::new();
        w3d_command_bar_grid_draw(&mut wm, w, 0x11, &mut out);
        assert_eq!(
            out,
            vec![
                DrawItem::OpenRect { x: 10, y: 20, w: 100, h: 60, color: 0x11 },
                DrawItem::FillRect { x: 11, y: 21, w: 98, h: 58, color: 0x22 },
                DrawItem::Line { x0: 10, y0: 39, x1: 110, y1: 39, color: 0x11 },
                DrawItem::Line { x0: 10, y0: 59, x1: 110, y1: 59, color: 0x11 },
                DrawItem::Line { x0: 43, y0: 20, x1: 43, y1: 80, color: 0x11 },
                DrawItem::Line { x0: 76, y0: 20, x1: 76, y1: 80, color: 0x11 },
            ]
        );
        wm.tree.windows[w].status |= win_status::IMAGE;
        wm.tree.windows[w].inst.enabled_draw_data[0].border_color = 0x33;
        let mut out = Vec::new();
        w3d_command_bar_grid_draw(&mut wm, w, 0x11, &mut out);
        assert!(out.iter().all(|d| !matches!(d, DrawItem::Line { .. })));
        assert_eq!(wm.tree.windows[w].inst.enabled_draw_data[0].border_color, 0x33);
    }

    /// #194: the scheme's queue and right HUD images: none given changes nothing; given, each queue button's disabled
    /// image 0 with its enabled 5 and 6 cleared, and the right HUD's enabled image 0.
    #[test]
    fn the_schemes_bar_images() {
        use crate::client::window_manager::{NoGuiHost, SystemFunc};
        let mut wm = GameWindowManager::new((800, 600));
        let q = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, 0, 0, 10, 10, SystemFunc::Default, None);
        let r = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, 0, 0, 10, 10, SystemFunc::Default, None);
        wm.tree.windows[q].inst.enabled_draw_data[5].image = Some(b"Mid".to_vec());
        wm.tree.windows[r].inst.enabled_draw_data[0].image = Some(b"Old".to_vec());
        update_build_queue_disabled_images(&mut wm.tree, &[Some(q), None], None);
        update_right_hud_image(&mut wm.tree, Some(r), None);
        assert_eq!((wm.tree.windows[q].inst.enabled_draw_data[5].image.as_deref(), wm.tree.windows[r].inst.enabled_draw_data[0].image.as_deref()), (Some(&b"Mid"[..]), Some(&b"Old"[..])));
        update_build_queue_disabled_images(&mut wm.tree, &[Some(q), None], Some(b"Queue".to_vec()));
        update_right_hud_image(&mut wm.tree, Some(r), Some(b"Hud".to_vec()));
        let (qi, ri) = (&wm.tree.windows[q].inst, &wm.tree.windows[r].inst);
        assert_eq!((qi.disabled_draw_data[0].image.as_deref(), qi.enabled_draw_data[5].image.as_deref(), ri.enabled_draw_data[0].image.as_deref()), (Some(&b"Queue"[..]), None, Some(&b"Hud"[..])));
    }

    fn img(name: &[u8], x0: i32, y0: i32, x1: i32, y1: i32, clip: Option<(i32, i32, i32, i32)>) -> DrawItem {
        DrawItem::Image { image: name.to_vec(), x0, y0, x1, y1, color: 0xFFFF_FFFF, grayscale: false, clip }
    }

    /// #195: predicted from W3DControlBar.cpp before the run: a power window at (10, 20) 90x10, base 10, intervals 3, yellow
    /// range 5, production 100 and consumption 10 (green: 10 is not above 95), 10x10 bar images and a 6x10 slider. The
    /// scale is 90 / 3 = 30 pixels per decade: the bar runs log10(100) x 30 = 60 pixels, six whole pieces from x 10 to 70,
    /// then one more at 70..80 clipped to (70, 20, 100, 30), the clip off after; the needle at log10(10) x 30 = 30,
    /// centred: x 37..43, y 20..30. Without the slider image, or without `TheGlobalData`, nothing. And the conversions:
    /// NaN and +inf to `0x80000000`, -1.5 to -1; `logN(100, 10)` exactly 2.
    #[test]
    fn the_power_bar() {
        use crate::client::window_manager::{NoGuiHost, SystemFunc};
        let mut wm = GameWindowManager::new((800, 600));
        let w = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, 10, 20, 90, 10, SystemFunc::Default, None);
        let g = PowerBarGlobals { base: 10, intervals: 3.0, yellow_range: 5 };
        let find = |n: &[u8]| Some(if n == b"PowerBarSlider" { (6, 10) } else { (10, 10) });
        let mut out = Vec::new();
        w3d_power_draw(&mut wm, w, Some((100, 10)), Some(g), find, &mut out);
        let mut want: Vec<DrawItem> = (0..6).map(|i| img(b"PowerPointG", 10 + i * 10, 20, 20 + i * 10, 30, None)).collect();
        want.push(img(b"PowerPointG", 70, 20, 80, 30, Some((70, 20, 100, 30))));
        want.push(img(b"PowerBarSlider", 37, 20, 43, 30, None));
        assert_eq!(out, want);
        assert_eq!(wm.display_clip(), None);
        let mut out = Vec::new();
        w3d_power_draw(&mut wm, w, Some((100, 10)), Some(g), |n: &[u8]| (n != b"PowerBarSlider").then_some((10, 10)), &mut out);
        w3d_power_draw(&mut wm, w, Some((100, 10)), None, find, &mut out);
        w3d_power_draw(&mut wm, w, None, Some(g), find, &mut out);
        assert!(out.is_empty());
        assert_eq!((x86_float_to_int(f32::NAN), x86_float_to_int(f32::INFINITY), x86_float_to_int(-1.5)), (i32::MIN, i32::MIN, -1));
        assert_eq!(log_n(100.0, 10.0), 2.0);
    }

    /// #195: predicted from W3DControlBar.cpp before the run: an experience window at (0, 0) 10x100, skill points 50 of 0
    /// to 100 (progress 50, range 50), the top and bottom images 10x5, the bar 10x10. The bottom image's top is 95 and the
    /// top's start 100 - 50 - 5 = 45: five pieces at y 45, 55, .., 85 (no room left, so no clipped piece), the bottom
    /// image at (0, 95, 10, 100), and the top one at (0, 50)-(10, 45), upside down as upstream hands it over. Nothing
    /// for an inactive player, and nothing (not a crash) for level up equal to level down.
    #[test]
    fn the_generals_experience_bar() {
        use crate::client::window_manager::{NoGuiHost, SystemFunc};
        let mut wm = GameWindowManager::new((800, 600));
        let w = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, 0, 0, 10, 100, SystemFunc::Default, None);
        let find = |n: &[u8]| Some(if n == b"GenExpBar1" { (10, 10) } else { (10, 5) });
        let p = GenExpPoints { active: true, skill_points: 50, level_up: 100, level_down: 0 };
        let mut out = Vec::new();
        w3d_command_bar_gen_exp_draw(&mut wm, w, p, find, &mut out);
        let mut want: Vec<DrawItem> = (0..5).map(|i| img(b"GenExpBar1", 0, 45 + i * 10, 10, 55 + i * 10, None)).collect();
        want.push(img(b"GenExpBarBottom1", 0, 95, 10, 100, None));
        want.push(img(b"GenExpBarTop1", 0, 50, 10, 45, None));
        assert_eq!(out, want);
        let mut out = Vec::new();
        w3d_command_bar_gen_exp_draw(&mut wm, w, GenExpPoints { active: false, ..p }, find, &mut out);
        w3d_command_bar_gen_exp_draw(&mut wm, w, GenExpPoints { level_up: 0, ..p }, find, &mut out);
        assert!(out.is_empty());
    }

    /// #195: predicted from W3DControlBar.cpp before the run: a popup window at (0, 0) 100x40 with 100x8 ends and a
    /// 100x10 middle: 24 pixels between the ends, two pieces at y 8 and 18, then one at 28..38 clipped to (0, 28, 100,
    /// 32); the bottom at (0, 32, 100, 40), the top at (0, 0, 100, 8). Without the middle image nothing.
    #[test]
    fn the_help_popups_frame() {
        use crate::client::window_manager::{NoGuiHost, SystemFunc};
        let mut wm = GameWindowManager::new((800, 600));
        let w = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, 0, 0, 100, 40, SystemFunc::Default, None);
        let find = |n: &[u8]| Some(if n == b"Helpbox-middle" { (100, 10) } else { (100, 8) });
        let mut out = Vec::new();
        w3d_command_bar_help_popup_draw(&mut wm, w, find, &mut out);
        let want = vec![
            img(b"Helpbox-middle", 0, 8, 100, 18, None),
            img(b"Helpbox-middle", 0, 18, 100, 28, None),
            img(b"Helpbox-middle", 0, 28, 100, 38, Some((0, 28, 100, 32))),
            img(b"Helpbox-bottom", 0, 32, 100, 40, None),
            img(b"Helpbox-top", 0, 0, 100, 8, None),
        ];
        assert_eq!(out, want);
        let mut out = Vec::new();
        w3d_command_bar_help_popup_draw(&mut wm, w, |n: &[u8]| (n != b"Helpbox-middle").then_some((100, 8)), &mut out);
        assert!(out.is_empty());
    }
}
