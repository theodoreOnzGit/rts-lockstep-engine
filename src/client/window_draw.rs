// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameWindow.cpp (initBorders,
//     blitBorderRect, W3DGameWinDefaultDraw, winDrawBorder)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DPushButton.cpp (drawButtonText,
//     W3DGadgetPushButtonDraw, W3DGadgetPushButtonImageDraw, W3DGadgetPushButtonImageDrawOne,
//     W3DGadgetPushButtonImageDrawThree)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DCheckBox.cpp (drawCheckBoxText,
//     W3DGadgetCheckBoxDraw, W3DGadgetCheckBoxImageDraw)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DRadioButton.cpp (drawRadioButtonText,
//     W3DGadgetRadioButtonDraw, W3DGadgetRadioButtonImageDraw)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DStaticText.cpp (drawStaticTextText,
//     W3DGadgetStaticTextDraw, W3DGadgetStaticTextImageDraw)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowGlobal.cpp (winDrawImage, winFillRect, winOpenRect,
//     winDrawLine)
//   GeneralsMD/Code/GameEngine/Include/GameClient/{GadgetPushButton.h, GadgetCheckBox.h, GadgetRadioButton.h,
//     GadgetStaticText.h} (which draw data index each state reads)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! What the W3D device draws for each window (GitHub #34): the draw callbacks of `W3DGameWindow` and the
//! `W3DGadget*` files, as a list of [`DrawItem`]s (`TheDisplay`'s calls) that the client renders.
//!
//! # As upstream reads it
//!
//! Each window has nine draw data (image, colour, border colour) for each of three states: enabled, disabled and
//! hilite. A draw function picks the state (**disabled** when the window's `WIN_STATUS_ENABLED` is clear, else
//! **hilite** when `WIN_STATE_HILITED`, else **enabled**) and, for buttons, the **selected** variant (index 1 instead
//! of 0). A window with `WIN_STATUS_IMAGE` draws images, any other its colours: the border colour as a one-pixel open
//! rectangle and the colour filled one pixel inside, each only when defined.
//!
//! - `W3DGameWinDefaultDraw`: index 0 of the state; no text.
//! - Push button: colours 0/1; images by `W3DGadgetPushButtonImageDraw`, which draws three pieces (left 0, middle 5,
//!   right 6 when enabled; the selected ones 1, 3, 4 ...) when the middle image (enabled 5) is set, else one image.
//!   With `WIN_STATUS_USE_OVERLAY_STATES` it draws the enabled image always (grey when disabled and ready unless
//!   `ALWAYS_COLOR`, which darkens it to 144/255 instead) and then `Cameo_hilited` / `Cameo_push` over it; flashing
//!   adds `Cameo_push`. The text is centred (top left + 2 for a shortcut button) in the state's text colours; then the
//!   overlay image, the clock (once) and the special border.
//! - Check box: colours (background 0, box 1 unchecked or 2 checked; the box an `X` of two lines); images (only the
//!   box, inset 3 pixels); text at `x + height`.
//! - Radio button: colours (background 0; the boxes 1 / 2 at both ends); images (left 0, a repeated, clipped centre
//!   1, right 2; the selected set is hilite 3, 4, 5 and wins over disabled).
//! - Static text: colour 0 or image 0, text clipped to the window, word wrapped at `width - 10`, centred or at its
//!   margins.
//! - The border (`winDrawBorder`): the style's lowest bit chooses; push buttons, radio buttons, static texts, progress
//!   bars, user windows and tab controls blit the `Border*` images around the window.
//!
//! # Upstream quirks that are kept
//!
//! - `W3DGadgetPushButtonImageDrawOne`: a selected (pushed) button that is enabled but not hilited draws its
//!   **hilite** selected image (hilite 1), not its enabled selected one.
//! - `W3DGadgetPushButtonImageDraw` tests `WIN_STATUS_USE_OVERLAY_STATES` against the window's **state**, not its
//!   status: the bit (0x200000) is never set there, so three-piece buttons ignore overlay states.
//! - The push button's clock is drawn once per `GadgetButtonDrawClock`: the draw clears it.
//! - `W3DGameWinDefaultDraw` draws no text.
//! - The check box's colour draw draws the background border and fill without testing whether they are defined.
//! - The three-piece push button with no room for its centre draws its left image to the middle and its right image
//!   from there to the window's right edge **without** the image offset.
//!
//! # Where this differs from upstream
//!
//! Text is measured by the [`GuiHost`] (`DisplayString::getSize`), and so are image sizes; the list records the clip
//! region where upstream sets one on the display. Video buffers are not drawn (none is ever set by what is
//! translated). Drawing an image the collection does not have (`NULL`) is skipped, as `Display::drawImage` does.
//!
//! # Representation
//!
//! Free functions that append to a `Vec<DrawItem>`.

use crate::client::window_manager::{ClockKind, DrawFunc, DrawItem, GameWindowManager, GuiHost, UserData, WIN_STATE_HILITED, WIN_STATE_SELECTED, WIN_STATUS_SHORTCUT_BUTTON};
use crate::client::window_script::{gws, win_status, GameWindow, WinDrawData, WinId, WIN_COLOR_UNDEFINED};

/// The state a draw function draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawState {
    Enabled,
    Disabled,
    Hilite,
}

/// The state: disabled (`WIN_STATUS_ENABLED` clear), else hilite (`WIN_STATE_HILITED`), else enabled.
pub fn draw_state(w: &GameWindow) -> DrawState {
    if w.status & win_status::ENABLED == 0 {
        DrawState::Disabled
    } else if w.inst.state & WIN_STATE_HILITED != 0 {
        DrawState::Hilite
    } else {
        DrawState::Enabled
    }
}

fn dd(w: &GameWindow, s: DrawState, i: usize) -> &WinDrawData {
    match s {
        DrawState::Enabled => &w.inst.enabled_draw_data[i],
        DrawState::Disabled => &w.inst.disabled_draw_data[i],
        DrawState::Hilite => &w.inst.hilite_draw_data[i],
    }
}

fn selected(w: &GameWindow) -> bool {
    w.inst.state & WIN_STATE_SELECTED != 0
}

/// `winOpenRect(color, width, x0, y0, x1, y1)`.
fn open_rect(out: &mut Vec<DrawItem>, color: i32, x0: i32, y0: i32, x1: i32, y1: i32) {
    out.push(DrawItem::OpenRect { x: x0, y: y0, w: x1 - x0, h: y1 - y0, color });
}
/// `winFillRect(color, width, x0, y0, x1, y1)`.
fn fill_rect(out: &mut Vec<DrawItem>, color: i32, x0: i32, y0: i32, x1: i32, y1: i32) {
    out.push(DrawItem::FillRect { x: x0, y: y0, w: x1 - x0, h: y1 - y0, color });
}
/// `winDrawImage` / `Display::drawImage` (a `NULL` image draws nothing).
fn image(out: &mut Vec<DrawItem>, wm: &GameWindowManager, img: Option<&Vec<u8>>, x0: i32, y0: i32, x1: i32, y1: i32) {
    image_mode(out, wm, img, x0, y0, x1, y1, 0xFFFF_FFFF, false);
}
#[allow(clippy::too_many_arguments)]
fn image_mode(out: &mut Vec<DrawItem>, wm: &GameWindowManager, img: Option<&Vec<u8>>, x0: i32, y0: i32, x1: i32, y1: i32, color: u32, grayscale: bool) {
    if let Some(i) = img {
        out.push(DrawItem::Image { image: i.clone(), x0, y0, x1, y1, color, grayscale, clip: wm.clip });
    }
}

/// A window's screen origin and size.
fn place(wm: &GameWindowManager, w: WinId) -> ((i32, i32), (i32, i32)) {
    (wm.tree.screen_position(w), wm.tree.windows[w].size)
}

/// The window's draw callback.
pub fn draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    match wm.ext[w].draw.clone() {
        DrawFunc::GameWinDefault | DrawFunc::NoDraw => {}
        DrawFunc::W3DDefault | DrawFunc::Untranslated(_) => w3d_game_win_default_draw(wm, w, out),
        DrawFunc::PushButton => push_button_draw(wm, h, w, out),
        DrawFunc::PushButtonImage => push_button_image_draw(wm, h, w, out),
        DrawFunc::CheckBox => check_box_draw(wm, h, w, out),
        DrawFunc::CheckBoxImage => check_box_image_draw(wm, h, w, out),
        DrawFunc::RadioButton => radio_button_draw(wm, h, w, out),
        DrawFunc::RadioButtonImage => radio_button_image_draw(wm, h, w, out),
        DrawFunc::StaticText => static_text_draw(wm, h, w, out),
        DrawFunc::StaticTextImage => static_text_image_draw(wm, h, w, out),
        DrawFunc::Host(name) => h.draw(wm, &name, w, out),
    }
}

/// `W3DGameWinDefaultDraw`.
pub fn w3d_game_win_default_draw(wm: &GameWindowManager, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let st = draw_state(win);
    if win.status & win_status::IMAGE != 0 {
        let (x, y) = (ox + win.inst.image_offset.0, oy + win.inst.image_offset.1);
        image(out, wm, dd(win, st, 0).image.as_ref(), x, y, x + sx, y + sy);
    } else {
        let d = dd(win, st, 0);
        if d.border_color != WIN_COLOR_UNDEFINED {
            open_rect(out, d.border_color, ox, oy, ox + sx, oy + sy);
        }
        if d.color != WIN_COLOR_UNDEFINED {
            fill_rect(out, d.color, ox + 1, oy + 1, ox + sx - 1, oy + sy - 1);
        }
    }
}

/// The text colour and its drop (border) colour of the state.
fn text_colors(win: &GameWindow, s: DrawState) -> (i32, i32) {
    let t = match s {
        DrawState::Enabled => win.inst.enabled_text,
        DrawState::Disabled => win.inst.disabled_text,
        DrawState::Hilite => win.inst.hilite_text,
    };
    (t.color, t.border_color)
}

/// `drawButtonText` (push button): wrapped at the width, centred (or the shortcut button's top left).
fn draw_button_text<H: GuiHost>(wm: &GameWindowManager, h: &H, w: WinId, out: &mut Vec<DrawItem>) {
    let win = &wm.tree.windows[w];
    let Some(text) = win.text.as_ref().filter(|t| !t.is_empty()) else { return };
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let centered = win.inst.status & win_status::WRAP_CENTERED != 0;
    let (color, drop) = text_colors(win, draw_state(win));
    let (tw, th) = h.text_size(win.inst.font.as_ref(), text, Some(sx));
    let (x, y) = if win.status & WIN_STATUS_SHORTCUT_BUTTON != 0 { (ox + 2, oy) } else { (ox + sx / 2 - tw / 2, oy + sy / 2 - th / 2) };
    out.push(DrawItem::Text { text: text.clone(), x, y, color, drop_color: drop, font: win.inst.font.clone(), word_wrap: Some(sx), wrap_centered: centered, hotkey: false, clip: None });
}

/// The end of the push button draws: the overlay image, the clock (then cleared), the special border.
fn push_button_extras(wm: &mut GameWindowManager, w: WinId, (x, y): (i32, i32), (sx, sy): (i32, i32), overlay_at: (i32, i32), out: &mut Vec<DrawItem>) {
    let clip = wm.clip;
    if let UserData::PushButton(Some(p)) = &mut wm.ext[w].user {
        if let Some(o) = &p.overlay_image {
            out.push(DrawItem::Image { image: o.clone(), x0: overlay_at.0, y0: overlay_at.1, x1: overlay_at.0 + sx, y1: overlay_at.1 + sy, color: 0xFFFF_FFFF, grayscale: false, clip });
        }
        if p.draw_clock != ClockKind::None {
            out.push(DrawItem::RectClock { x, y, w: sx, h: sy, percent: p.percent_clock, color: p.color_clock, inverse: p.draw_clock == ClockKind::Inverse });
            p.draw_clock = ClockKind::None;
        }
        if p.draw_border && p.color_border != WIN_COLOR_UNDEFINED {
            out.push(DrawItem::OpenRect { x: x - 1, y: y - 1, w: sx + 2, h: sy + 2, color: p.color_border });
        }
    }
}

/// `W3DGadgetPushButtonDraw`: colours 0 (1 when selected) of the state.
pub fn push_button_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let d = dd(win, draw_state(win), usize::from(selected(win)));
    let (color, border) = (d.color, d.border_color);
    let (mut x0, mut y0, mut x1, mut y1) = (ox, oy, ox + sx, oy + sy);
    if border != WIN_COLOR_UNDEFINED {
        open_rect(out, border, x0, y0, x1, y1);
    }
    if color != WIN_COLOR_UNDEFINED {
        x0 += 1;
        y0 += 1;
        x1 -= 1;
        y1 -= 1;
        fill_rect(out, color, x0, y0, x1, y1);
    }
    draw_button_text(wm, h, w, out);
    push_button_extras(wm, w, (ox, oy), (sx, sy), (ox, oy), out);
}

/// `W3DGadgetPushButtonImageDraw`: three pieces when the middle enabled image (5) is set, else one.
pub fn push_button_image_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let win = &wm.tree.windows[w];
    if win.inst.enabled_draw_data[5].image.is_some() {
        // upstream tests the status bit against the state (never set there)
        if win.inst.state as u32 & win_status::USE_OVERLAY_STATES != 0 {
            push_button_image_draw_one(wm, h, w, out);
        } else {
            push_button_image_draw_three(wm, h, w, out);
        }
    } else {
        push_button_image_draw_one(wm, h, w, out);
    }
}

/// `W3DGadgetPushButtonImageDrawOne`.
pub fn push_button_image_draw_one<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let status = win.status;
    let sel = selected(win);
    let mut img = win.inst.enabled_draw_data[0].image.clone();
    if status & win_status::USE_OVERLAY_STATES == 0 {
        match draw_state(win) {
            DrawState::Disabled => img = win.inst.disabled_draw_data[usize::from(sel)].image.clone(),
            DrawState::Hilite => img = win.inst.hilite_draw_data[usize::from(sel)].image.clone(),
            DrawState::Enabled => {
                if sel {
                    img = win.inst.hilite_draw_data[1].image.clone();
                }
            }
        }
    }
    let (sx0, sy0) = (ox + win.inst.image_offset.0, oy + win.inst.image_offset.1);
    if img.is_some() {
        let (mut color, mut gray) = (0xFFFF_FFFFu32, false);
        if status & win_status::USE_OVERLAY_STATES != 0 && status & win_status::ENABLED == 0 && status & win_status::NOT_READY == 0 {
            if status & win_status::ALWAYS_COLOR == 0 {
                gray = true;
            } else {
                color = 0xFF90_9090;
            }
        }
        image_mode(out, wm, img.as_ref(), sx0, sy0, sx0 + sx, sy0 + sy, color, gray);
    }
    draw_button_text(wm, h, w, out);
    push_button_extras(wm, w, (ox, oy), (sx, sy), (ox, oy), out);
    let win = &wm.tree.windows[w];
    let push = b"Cameo_push".to_vec();
    let hilited = b"Cameo_hilited".to_vec();
    if status & win_status::FLASHING != 0 && h.has_image(&push) {
        image(out, wm, Some(&push), ox, oy, ox + sx, oy + sy);
    }
    if status & win_status::USE_OVERLAY_STATES != 0 && h.has_image(&push) && h.has_image(&hilited) && status & win_status::ENABLED != 0 {
        if win.inst.state & WIN_STATE_HILITED != 0 {
            let o = if selected(win) { &push } else { &hilited };
            image(out, wm, Some(o), ox, oy, ox + sx, oy + sy);
        } else if selected(win) {
            image(out, wm, Some(&push), ox, oy, ox + sx, oy + sy);
        }
    }
}

/// An image's name, if the draw data has one.
type ImageName = Option<Vec<u8>>;

/// The three images (left, right, middle) of the push button's state: enabled 0/6/5 (selected 1/4/3), and the same
/// indices of the disabled and hilite sets.
fn three_images(win: &GameWindow) -> (ImageName, ImageName, ImageName) {
    let s = draw_state(win);
    let (l, r, m) = if selected(win) { (1, 4, 3) } else { (0, 6, 5) };
    (dd(win, s, l).image.clone(), dd(win, s, r).image.clone(), dd(win, s, m).image.clone())
}

/// `W3DGadgetPushButtonImageDrawThree`.
pub fn push_button_image_draw_three<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let (xo, yo) = win.inst.image_offset;
    let (left, right, center) = three_images(win);
    let (Some(left), Some(right), Some(center)) = (left, right, center) else { return };
    let (lw, _) = h.image_size(&left).unwrap_or((0, 0));
    let (rw, _) = h.image_size(&right).unwrap_or((0, 0));
    let (cw, _) = h.image_size(&center).unwrap_or((0, 0));
    let left_end = (ox + lw + xo, oy + sy + yo);
    let right_start = (ox + sx - rw + xo, oy + yo);
    let center_width = right_start.0 - left_end.0;
    if center_width <= 0 {
        let (x0, y0) = (ox + xo, oy + yo);
        let x1 = ox + xo + sx / 2;
        image(out, wm, Some(&left), x0, y0, x1, left_end.1);
        let (x0, y0) = (x1, right_start.1);
        image(out, wm, Some(&right), x0, y0, ox + sx, y0 + sy);
    } else {
        let pieces = if cw != 0 { center_width / cw } else { 0 };
        let mut x = left_end.0;
        let y = oy + yo;
        let y1 = y + sy + yo;
        for _ in 0..pieces {
            image(out, wm, Some(&center), x, y, x + cw, y1);
            x += cw;
        }
        if right_start.0 - x > 0 {
            wm.clip = Some((x, y, right_start.0, y1));
            image(out, wm, Some(&center), x, y, x + cw, y1);
            wm.clip = None;
        }
        image(out, wm, Some(&left), ox + xo, oy + yo, left_end.0, left_end.1);
        image(out, wm, Some(&right), right_start.0, right_start.1, right_start.0 + rw, right_start.1 + sy);
    }
    draw_button_text(wm, h, w, out);
    push_button_extras(wm, w, (ox, oy), (sx, sy), (ox, oy), out);
}

/// `drawCheckBoxText`: at `x + height`, centred vertically.
fn draw_check_box_text<H: GuiHost>(wm: &GameWindowManager, h: &H, w: WinId, out: &mut Vec<DrawItem>, centered_x: bool) {
    let win = &wm.tree.windows[w];
    let Some(text) = win.text.as_ref().filter(|t| !t.is_empty()) else { return };
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let (color, drop) = text_colors(win, draw_state(win));
    let (tw, th) = h.text_size(win.inst.font.as_ref(), text, None);
    let x = if centered_x { ox + sx / 2 - tw / 2 } else { ox + sy };
    out.push(DrawItem::Text { text: text.clone(), x, y: oy + sy / 2 - th / 2, color, drop_color: drop, font: win.inst.font.clone(), word_wrap: None, wrap_centered: false, hotkey: false, clip: None });
}

/// `W3DGadgetCheckBoxDraw`.
pub fn check_box_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let s = draw_state(win);
    let back = dd(win, s, 0);
    let boxd = dd(win, s, if selected(win) { 2 } else { 1 });
    let off = sx / 16;
    open_rect(out, back.border_color, ox, oy, ox + sx, oy + sy);
    fill_rect(out, back.color, ox + 1, oy + 1, ox + sx - 1, oy + sy - 1);
    let (x0, y0) = (ox + off, oy + sy / 3);
    let (x1, y1) = (x0 + sy / 3, y0 + sy / 3);
    open_rect(out, boxd.border_color, x0, y0, x1, y1);
    if boxd.color != WIN_COLOR_UNDEFINED {
        out.push(DrawItem::Line { x0, y0, x1, y1, color: boxd.color });
        out.push(DrawItem::Line { x0, y0: y1, x1, y1: y0, color: boxd.color });
    }
    draw_check_box_text(wm, h, w, out, false);
}

/// `W3DGadgetCheckBoxImageDraw`: the box image of the state, inset 3 pixels, square.
pub fn check_box_image_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (_sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let img = dd(win, draw_state(win), if selected(win) { 2 } else { 1 }).image.clone();
    if img.is_some() {
        let x0 = ox + win.inst.image_offset.0;
        let y0 = oy + 3;
        image(out, wm, img.as_ref(), x0, y0, x0 + (sy - 6), y0 + (sy - 6));
    }
    draw_check_box_text(wm, h, w, out, false);
}

/// `W3DGadgetRadioButtonDraw`.
pub fn radio_button_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let s = draw_state(win);
    let back = dd(win, s, 0).clone();
    let boxd = dd(win, s, if selected(win) { 2 } else { 1 }).clone();
    open_rect(out, back.border_color, ox, oy, ox + sx, oy + sy);
    fill_rect(out, back.color, ox + 1, oy + 1, ox + sx - 1, oy + sy - 1);
    out.push(DrawItem::Line { x0: ox + sy, y0: oy, x1: ox + sy, y1: oy + sy, color: back.border_color });
    fill_rect(out, boxd.color, ox + 1, oy + 1, ox + sy - 1, oy + sy - 1);
    out.push(DrawItem::Line { x0: ox + sx - sy, y0: oy, x1: ox + sx - sy, y1: oy + sy, color: back.border_color });
    fill_rect(out, boxd.color, ox + sx - sy, oy + 1, ox + sx - 1, oy + sy - 1);
    draw_check_box_text(wm, h, w, out, true);
}

/// `W3DGadgetRadioButtonImageDraw`: left, a repeated centre clipped between the ends, right.
pub fn radio_button_image_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let (xo, yo) = win.inst.image_offset;
    let (l, c, r) = if selected(win) {
        let d = &win.inst.hilite_draw_data;
        (d[3].image.clone(), d[4].image.clone(), d[5].image.clone())
    } else {
        let s = draw_state(win);
        (dd(win, s, 0).image.clone(), dd(win, s, 1).image.clone(), dd(win, s, 2).image.clone())
    };
    let (Some(l), Some(c), Some(r)) = (l, c, r) else { return };
    let (lw, _) = h.image_size(&l).unwrap_or((0, 0));
    let (rw, _) = h.image_size(&r).unwrap_or((0, 0));
    let (cw, _) = h.image_size(&c).unwrap_or((0, 0));
    let left_end = (ox + lw + xo, oy + sy + yo);
    let right_start = (ox + sx - rw + xo, oy + sy + yo);
    let center_width = right_start.0 - left_end.0;
    let pieces = if cw != 0 { center_width / cw } else { 0 } + 1;
    let mut x = left_end.0;
    let (y0, y1) = (oy + yo, oy + sy + yo);
    wm.clip = Some((left_end.0, oy, right_start.0, left_end.1));
    for _ in 0..pieces {
        image(out, wm, Some(&c), x, y0, x + cw, y1);
        x += cw;
    }
    wm.clip = None;
    image(out, wm, Some(&l), ox + xo, oy + yo, left_end.0, left_end.1);
    image(out, wm, Some(&r), right_start.0, oy + yo, ox + sx, left_end.1);
    draw_check_box_text(wm, h, w, out, true);
}

/// `drawStaticTextText`.
fn draw_static_text_text<H: GuiHost>(wm: &GameWindowManager, h: &H, w: WinId, color: i32, drop: i32, out: &mut Vec<DrawItem>) {
    let UserData::StaticText(td, text) = &wm.ext[w].user else { return };
    if text.is_empty() {
        return;
    }
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let wrap = sx - 10;
    let (tw, th) = h.text_size(win.inst.font.as_ref(), text, Some(wrap));
    let x = if td.centered { ox + sx / 2 - tw / 2 } else { ox + td.left_margin };
    let y = if td.centered_vertically { oy + sy / 2 - th / 2 } else { oy + td.top_margin };
    out.push(DrawItem::Text {
        text: text.clone(),
        x,
        y,
        color,
        drop_color: drop,
        font: win.inst.font.clone(),
        word_wrap: Some(wrap),
        wrap_centered: win.status & win_status::WRAP_CENTERED != 0,
        hotkey: win.status & win_status::HOTKEY_TEXT != 0,
        clip: Some((ox, oy, ox + sx, oy + sy)),
    });
}

/// The static text's state: disabled or enabled (no hilite).
fn static_state(win: &GameWindow) -> DrawState {
    if win.status & win_status::ENABLED == 0 {
        DrawState::Disabled
    } else {
        DrawState::Enabled
    }
}

/// `W3DGadgetStaticTextDraw`.
pub fn static_text_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let s = static_state(win);
    let d = dd(win, s, 0);
    let (tc, tb) = text_colors(win, s);
    if d.border_color != WIN_COLOR_UNDEFINED {
        open_rect(out, d.border_color, ox, oy, ox + sx, oy + sy);
    }
    if d.color != WIN_COLOR_UNDEFINED {
        fill_rect(out, d.color, ox + 1, oy + 1, ox + 1 + sx - 2, oy + 1 + sy - 2);
    }
    if tc != WIN_COLOR_UNDEFINED {
        draw_static_text_text(wm, h, w, tc, tb, out);
    }
}

/// `W3DGadgetStaticTextImageDraw`.
pub fn static_text_image_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let s = static_state(win);
    let img = dd(win, s, 0).image.clone();
    let (tc, tb) = text_colors(win, s);
    if img.is_some() {
        let (x, y) = (ox + win.inst.image_offset.0, oy + win.inst.image_offset.1);
        image(out, wm, img.as_ref(), x, y, x + sx, y + sy);
    }
    if tc != WIN_COLOR_UNDEFINED {
        draw_static_text_text(wm, h, w, tc, tb, out);
    }
}

/// `BORDER_CORNER_SIZE`, `BORDER_LINE_SIZE`.
const BORDER_CORNER_SIZE: i32 = 15;
const BORDER_LINE_SIZE: i32 = 20;

/// `blitBorderRect(x, y, width, height)`: the `Border*` images around a rectangle.
pub fn blit_border_rect(wm: &GameWindowManager, out: &mut Vec<DrawItem>, ox: i32, oy: i32, width: i32, height: i32) {
    let (offset, lower, size, half) = (15, 5, 20, 10);
    let (max_x, max_y) = (ox + width, oy + height);
    let img = |n: &str| Some(n.as_bytes().to_vec());
    let put = |out: &mut Vec<DrawItem>, n: &str, x0: i32, y0: i32, x1: i32, y1: i32| image(out, wm, img(n).as_ref(), x0, y0, x1, y1);
    // horizontal lines
    let y = oy - offset;
    let y2 = max_y - lower;
    let x2 = max_x - (lower + BORDER_LINE_SIZE);
    let mut x = ox + lower;
    while x <= x2 {
        put(out, "BorderTop", x, y, x + size, y + size);
        put(out, "BorderBottom", x, y2, x + size, y2 + size);
        x += BORDER_LINE_SIZE;
    }
    let x2 = max_x - 5;
    if x2 - x >= BORDER_LINE_SIZE / 2 {
        put(out, "BorderTopShort", x, y, x + half, y + size);
        put(out, "BorderBottomShort", x, y2, x + half, y2 + size);
        x += BORDER_LINE_SIZE / 2;
    }
    if x < x2 {
        x -= (BORDER_LINE_SIZE / 2) - (((x2 - x) + 1) & !1);
        put(out, "BorderTopShort", x, y, x + half, y + size);
        put(out, "BorderBottomShort", x, y2, x + half, y2 + size);
    }
    // vertical lines
    let x = ox - offset;
    let x2 = max_x - lower;
    let y2 = max_y - (lower + BORDER_LINE_SIZE);
    let mut y = oy + lower;
    while y <= y2 {
        put(out, "BorderLeft", x, y, x + size, y + size);
        put(out, "BorderRight", x2, y, x2 + size, y + size);
        y += BORDER_LINE_SIZE;
    }
    let y2 = max_y - lower;
    if y2 - y >= BORDER_LINE_SIZE / 2 {
        put(out, "BorderLeftShort", x, y, x + size, y + half);
        put(out, "BorderRightShort", x2, y, x2 + size, y + half);
        y += BORDER_LINE_SIZE / 2;
    }
    if y < y2 {
        y -= (BORDER_LINE_SIZE / 2) - (((y2 - y) + 1) & !1);
        put(out, "BorderLeftShort", x, y, x + size, y + half);
        put(out, "BorderRightShort", x2, y, x2 + size, y + half);
    }
    // corners
    put(out, "BorderCornerUL", ox - BORDER_CORNER_SIZE, oy - BORDER_CORNER_SIZE, ox - BORDER_CORNER_SIZE + size, oy - BORDER_CORNER_SIZE + size);
    put(out, "BorderCornerUR", max_x - 5, oy - BORDER_CORNER_SIZE, max_x - 5 + size, oy - BORDER_CORNER_SIZE + size);
    put(out, "BorderCornerLL", ox - BORDER_CORNER_SIZE, max_y - 5, ox - BORDER_CORNER_SIZE + size, max_y - 5 + size);
    put(out, "BorderCornerLR", max_x - 5, max_y - 5, max_x - 5 + size, max_y - 5 + size);
}

/// `W3DGameWindow::winDrawBorder`: by the style's lowest set bit. The images are kept only where the host's collection
/// has them (`findImageByName` is `NULL` otherwise and `drawImage` draws nothing).
pub fn win_draw_border<H: GuiHost>(wm: &GameWindowManager, h: &H, w: WinId, out: &mut Vec<DrawItem>) {
    let win = &wm.tree.windows[w];
    let (ox, oy) = wm.tree.screen_position(w);
    let style = win.inst.style;
    let mut items = Vec::new();
    for i in 0..32 {
        let bit = 1u32 << i;
        if style & bit == 0 {
            continue;
        }
        match bit {
            gws::CHECK_BOX | gws::VERT_SLIDER | gws::HORZ_SLIDER => {}
            gws::ENTRY_FIELD => {
                let mut width = win.size.0;
                let mut x = ox;
                if let Some(t) = win.text.as_ref().filter(|t| !t.is_empty()) {
                    let (tw, _) = h.text_size(win.inst.font.as_ref(), t, None);
                    width -= tw + 6;
                    x += tw + 6;
                }
                blit_border_rect(wm, &mut items, x, oy, width, win.size.1);
            }
            gws::SCROLL_LISTBOX => {
                // the slider's thumb is not created yet (#34's remaining list): no slider adjustment
                let label = if win.text.as_ref().is_some_and(|t| !t.is_empty()) { 4 } else { 0 };
                blit_border_rect(wm, &mut items, ox - 3, oy - (3 + label), win.size.0 + 3, win.size.1 + 6);
            }
            gws::RADIO_BUTTON | gws::STATIC_TEXT | gws::PROGRESS_BAR | gws::PUSH_BUTTON | gws::USER_WINDOW | gws::TAB_CONTROL => {
                blit_border_rect(wm, &mut items, ox, oy, win.size.0, win.size.1);
            }
            _ => continue,
        }
        break;
    }
    out.extend(items.into_iter().filter(|d| match d {
        DrawItem::Image { image, .. } => h.has_image(image),
        _ => true,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::gadget;
    use crate::client::window_manager::tests::Rec;
    use crate::client::window_manager::SystemFunc;
    use crate::client::window_script::{GadgetData, TextData, WinInstanceData};

    fn colours(inst: &mut WinInstanceData) {
        for (set, base) in [(&mut inst.enabled_draw_data, 0x100), (&mut inst.disabled_draw_data, 0x200), (&mut inst.hilite_draw_data, 0x300)] {
            for (i, d) in set.iter_mut().enumerate() {
                d.color = base + i as i32;
                d.border_color = base + 0x10 + i as i32;
                d.image = Some(format!("Img{base:x}_{i}").into_bytes());
            }
        }
    }

    fn button(wm: &mut GameWindowManager, h: &mut Rec, status: u32) -> WinId {
        let mut inst = WinInstanceData { style: gws::PUSH_BUTTON, ..WinInstanceData::new() };
        colours(&mut inst);
        inst.enabled_text.color = 0x7001;
        inst.hilite_text.color = 0x7003;
        inst.font = Some(crate::client::window_script::GameFont { name: b"F".to_vec(), point_size: 10, bold: false });
        wm.gogo_gadget(h, None, status, 10, 20, 50, 30, inst, GadgetData::None).expect("button")
    }

    /// #34: the push button's colours and text. Predicted from W3DPushButton.cpp (window at (10,20), 50x30; colours
    /// 0x1ii enabled, 0x2ii disabled, 0x3ii hilite, borders +0x10):
    /// - enabled: `OpenRect(10,20,50,30, 0x110)`, `FillRect(11,21,48,28, 0x100)`;
    /// - enabled and selected: index 1 (0x111 / 0x101); hilite: 0x310 / 0x300; hilite and selected: 0x311 / 0x301;
    ///   disabled and selected: 0x211 / 0x201 (disabled wins over hilite);
    /// - with text `AB` (font 10: 10x10 by the host's measure) the text is centred at (30, 30) in the state's text colour
    ///   (0x7001; 0x7003 when hilited), wrapped at 50;
    /// - a clock set by `GadgetButtonDrawClock(40)` draws once, at the end; the next repaint has none.
    #[test]
    fn a_push_buttons_colours_follow_its_state() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let b = button(&mut wm, &mut h, win_status::ENABLED);
        let rects = |wm: &mut GameWindowManager, h: &mut Rec| -> Vec<DrawItem> { wm.win_repaint(h).into_iter().filter(|d| !matches!(d, DrawItem::Text { .. })).collect() };
        assert_eq!(rects(&mut wm, &mut h), vec![DrawItem::OpenRect { x: 10, y: 20, w: 50, h: 30, color: 0x110 }, DrawItem::FillRect { x: 11, y: 21, w: 48, h: 28, color: 0x100 }]);
        let cases = [(0, WIN_STATE_SELECTED, 0x111, 0x101), (0, WIN_STATE_HILITED, 0x310, 0x300), (0, WIN_STATE_HILITED | WIN_STATE_SELECTED, 0x311, 0x301), (1, WIN_STATE_HILITED | WIN_STATE_SELECTED, 0x211, 0x201)];
        for (disabled, st, border, fill) in cases {
            wm.win_mut(b).inst.state = st;
            wm.win_enable(b, disabled == 0);
            let r = rects(&mut wm, &mut h);
            assert_eq!(r[0], DrawItem::OpenRect { x: 10, y: 20, w: 50, h: 30, color: border }, "{st} {disabled}");
            assert_eq!(r[1], DrawItem::FillRect { x: 11, y: 21, w: 48, h: 28, color: fill });
        }
        wm.win_enable(b, true);
        wm.win_mut(b).inst.state = 0;
        gadget::check_like_button_set_visual_check(&mut wm, b, true);
        assert_eq!(wm.win(b).inst.state, 0, "not check-like: no visual check");
        let ab: Vec<u16> = "AB".encode_utf16().collect();
        wm.set_label(&mut h, b, &ab);
        let t = wm.win_repaint(&mut h).into_iter().find(|d| matches!(d, DrawItem::Text { .. })).expect("text");
        assert_eq!(t, DrawItem::Text { text: ab.clone(), x: 30, y: 30, color: 0x7001, drop_color: crate::client::window_script::WIN_COLOR_UNDEFINED, font: wm.win(b).inst.font.clone(), word_wrap: Some(50), wrap_centered: false, hotkey: false, clip: None });
        gadget::button_draw_clock(&mut wm, b, 40, 0x55, false);
        let d = wm.win_repaint(&mut h);
        assert_eq!(d.last(), Some(&DrawItem::RectClock { x: 10, y: 20, w: 50, h: 30, percent: 40, color: 0x55, inverse: false }));
        assert!(!wm.win_repaint(&mut h).iter().any(|d| matches!(d, DrawItem::RectClock { .. })));
    }

    /// #34: the push button's images. Predicted from `W3DGadgetPushButtonImageDrawOne` / `Three`:
    /// - with `IMAGE` and no middle image (enabled 5 cleared): one image; enabled `Img100_0`; selected (not hilited)
    ///   `Img300_1`, the **hilite** selected image (the quirk); disabled `Img200_0`; hilite `Img300_0`, all at
    ///   (10,20)-(60,50);
    /// - with `USE_OVERLAY_STATES`: always `Img100_0`; disabled and not `NOT_READY` drawn grey; with `ALWAYS_COLOR` not
    ///   grey but 0xFF909090; enabled and hilited: then `Cameo_hilited` over it; hilited and selected: `Cameo_push`;
    /// - with the middle image set (three pieces; the host's images 5 wide, `...Mid` 7): at (0,0) 30x10, the left end
    ///   at x 5 and the right start at 25: two centre pieces (5..12, 12..19), a clipped one at 19 (clip 19..25), the left
    ///   (0..5) and the right (25..30).
    #[test]
    fn a_push_buttons_images_follow_its_state() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let b = button(&mut wm, &mut h, win_status::ENABLED | win_status::IMAGE);
        assert_eq!(wm.ext[b].draw, DrawFunc::PushButtonImage);
        wm.win_mut(b).inst.enabled_draw_data[5].image = None;
        let first = |wm: &mut GameWindowManager, h: &mut Rec| match wm.win_repaint(h).into_iter().next() {
            Some(DrawItem::Image { image, x0, y0, x1, y1, color, grayscale, .. }) => (String::from_utf8(image).unwrap(), (x0, y0, x1, y1), color, grayscale),
            o => panic!("{o:?}"),
        };
        assert_eq!(first(&mut wm, &mut h), ("Img100_0".into(), (10, 20, 60, 50), 0xFFFF_FFFF, false));
        wm.win_mut(b).inst.state = WIN_STATE_SELECTED;
        assert_eq!(first(&mut wm, &mut h).0, "Img300_1");
        wm.win_mut(b).inst.state = WIN_STATE_HILITED;
        assert_eq!(first(&mut wm, &mut h).0, "Img300_0");
        wm.win_mut(b).inst.state = 0;
        wm.win_enable(b, false);
        assert_eq!(first(&mut wm, &mut h).0, "Img200_0");
        wm.win_set_status(b, win_status::USE_OVERLAY_STATES);
        assert_eq!(first(&mut wm, &mut h), ("Img100_0".into(), (10, 20, 60, 50), 0xFFFF_FFFF, true));
        wm.win_set_status(b, win_status::ALWAYS_COLOR);
        assert_eq!(first(&mut wm, &mut h), ("Img100_0".into(), (10, 20, 60, 50), 0xFF90_9090, false));
        wm.win_enable(b, true);
        wm.win_mut(b).inst.state = WIN_STATE_HILITED;
        let names = |d: Vec<DrawItem>| d.into_iter().filter_map(|d| if let DrawItem::Image { image, .. } = d { Some(String::from_utf8(image).unwrap()) } else { None }).collect::<Vec<_>>();
        assert_eq!(names(wm.win_repaint(&mut h)), vec!["Img100_0", "Cameo_hilited"]);
        wm.win_mut(b).inst.state = WIN_STATE_HILITED | WIN_STATE_SELECTED;
        assert_eq!(names(wm.win_repaint(&mut h)), vec!["Img100_0", "Cameo_push"]);
        // three pieces
        let mut inst = WinInstanceData { style: gws::PUSH_BUTTON, ..WinInstanceData::new() };
        inst.enabled_draw_data[0].image = Some(b"ImgL".to_vec());
        inst.enabled_draw_data[5].image = Some(b"ImgMid".to_vec());
        inst.enabled_draw_data[6].image = Some(b"ImgR".to_vec());
        let mut wm = GameWindowManager::new((800, 600));
        wm.gogo_gadget(&mut h, None, win_status::ENABLED | win_status::IMAGE, 0, 0, 30, 10, inst, GadgetData::None).expect("button");
        let d: Vec<_> = wm.win_repaint(&mut h).into_iter().filter_map(|d| if let DrawItem::Image { image, x0, x1, clip, .. } = d { Some((String::from_utf8(image).unwrap(), x0, x1, clip)) } else { None }).collect();
        assert_eq!(d, vec![("ImgMid".into(), 5, 12, None), ("ImgMid".into(), 12, 19, None), ("ImgMid".into(), 19, 26, Some((19, 0, 25, 10))), ("ImgL".into(), 0, 5, None), ("ImgR".into(), 25, 30, None)]);
    }

    /// #34: the other gadgets and the repaint order. Predicted:
    /// - a check box at (10,50) 100x20, unchecked, enabled: back border 0x110 (10,50,100,20), fill 0x100 (11,51,98,18),
    ///   the box border 0x111 at (16,56) 6x6 and its `X` in 0x101 (two lines); checked: 0x112 / 0x102;
    /// - its image draw: the box image only (`Img100_1`, checked `Img100_2`), at (10,53)-(24,67);
    /// - a static text (centred, top margin 3) `T:x` of 3 characters (font none: 18x12): back 0x110 / 0x100 and the text
    ///   at (10 + 50 - 9, 50 + 3) clipped to the window, wrapped at 90;
    /// - the repaint order of top-level windows created BELOW, plain, ABOVE: below first, above last, whatever the list
    ///   order; a parent's children oldest first; a hidden child not at all; a see-through parent not, its child yes;
    /// - a bordered user window: its `Border*` images after its children.
    #[test]
    fn other_gadgets_and_the_repaint_order() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let mut inst = WinInstanceData { style: gws::CHECK_BOX, ..WinInstanceData::new() };
        colours(&mut inst);
        let c = wm.gogo_gadget(&mut h, None, win_status::ENABLED, 10, 50, 100, 20, inst.clone(), GadgetData::None).expect("check");
        assert_eq!(
            wm.win_repaint(&mut h),
            vec![
                DrawItem::OpenRect { x: 10, y: 50, w: 100, h: 20, color: 0x110 },
                DrawItem::FillRect { x: 11, y: 51, w: 98, h: 18, color: 0x100 },
                DrawItem::OpenRect { x: 16, y: 56, w: 6, h: 6, color: 0x111 },
                DrawItem::Line { x0: 16, y0: 56, x1: 22, y1: 62, color: 0x101 },
                DrawItem::Line { x0: 16, y0: 62, x1: 22, y1: 56, color: 0x101 },
            ]
        );
        gadget::check_box_set_checked(&mut wm, &mut h, c, true);
        assert_eq!(wm.win_repaint(&mut h)[2], DrawItem::OpenRect { x: 16, y: 56, w: 6, h: 6, color: 0x112 });
        let ci = wm.gogo_gadget(&mut h, None, win_status::ENABLED | win_status::IMAGE, 10, 50, 100, 20, inst, GadgetData::None).expect("check");
        wm.win_destroy(Some(c));
        let d = wm.win_repaint(&mut h);
        assert!(matches!(&d[0], DrawItem::Image { image, x0: 10, y0: 53, x1: 24, y1: 67, .. } if image == b"Img100_1"), "{d:?}");
        wm.win_destroy(Some(ci));
        // static text
        let mut inst = WinInstanceData { style: gws::STATIC_TEXT, ..WinInstanceData::new() };
        colours(&mut inst);
        inst.enabled_text.color = 0x7001;
        inst.font = None;
        let t = wm.gogo_gadget(&mut h, None, win_status::ENABLED, 10, 50, 100, 20, inst, GadgetData::Text(TextData { centered: true, centered_vertically: false, left_margin: 0, top_margin: 3 })).expect("text");
        wm.win_mut(t).inst.font = None;
        let x: Vec<u16> = "T:x".encode_utf16().collect();
        wm.set_label(&mut h, t, &x);
        let d = wm.win_repaint(&mut h);
        assert_eq!(d[..2], [DrawItem::OpenRect { x: 10, y: 50, w: 100, h: 20, color: 0x110 }, DrawItem::FillRect { x: 11, y: 51, w: 98, h: 18, color: 0x100 }]);
        assert!(matches!(&d[2], DrawItem::Text { x: 51, y: 53, word_wrap: Some(90), clip: Some((10, 50, 110, 70)), color: 0x7001, .. }), "{:?}", d[2]);
        wm.win_destroy(Some(t));
        // the repaint order
        let mut wm = GameWindowManager::new((800, 600));
        let mk = |wm: &mut GameWindowManager, h: &mut Rec, parent: Option<WinId>, status: u32, color: i32| {
            let mut inst = WinInstanceData { style: gws::USER_WINDOW, ..WinInstanceData::new() };
            inst.enabled_draw_data[0].color = color;
            wm.win_create(h, parent, status | win_status::ENABLED, 0, 0, 10, 10, SystemFunc::Default, Some(inst))
        };
        let _below = mk(&mut wm, &mut h, None, win_status::BELOW, 1);
        let p = mk(&mut wm, &mut h, None, 0, 2);
        let _above = mk(&mut wm, &mut h, None, win_status::ABOVE, 3);
        let _c1 = mk(&mut wm, &mut h, Some(p), 0, 4);
        let c2 = mk(&mut wm, &mut h, Some(p), 0, 5);
        let _c3 = mk(&mut wm, &mut h, Some(p), win_status::HIDDEN, 6);
        let fills = |wm: &mut GameWindowManager, h: &mut Rec| wm.win_repaint(h).into_iter().filter_map(|d| if let DrawItem::FillRect { color, .. } = d { Some(color) } else { None }).collect::<Vec<_>>();
        assert_eq!(fills(&mut wm, &mut h), vec![1, 2, 4, 5, 3]);
        wm.win_set_status(p, win_status::SEE_THRU);
        wm.win_hide(c2, true);
        assert_eq!(fills(&mut wm, &mut h), vec![1, 4, 3]);
        wm.win_clear_status(p, win_status::SEE_THRU);
        wm.win_set_status(p, win_status::BORDER);
        let d = wm.win_repaint(&mut h);
        let i4 = d.iter().position(|d| matches!(d, DrawItem::FillRect { color: 4, .. })).unwrap();
        let ib = d.iter().position(|d| matches!(d, DrawItem::Image { image, .. } if image == b"BorderCornerUL")).expect("a border");
        assert!(ib > i4);
    }
}
