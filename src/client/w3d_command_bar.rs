// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DControlBar.cpp
//     (W3DCommandBarGridDraw)
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
}
