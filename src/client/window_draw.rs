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
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DListBox.cpp (drawHiliteBar,
//     drawListBoxText, W3DGadgetListBoxDraw, W3DGadgetListBoxImageDraw)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DVerticalSlider.cpp
//     (W3DGadgetVerticalSliderDraw, W3DGadgetVerticalSliderImageDraw)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DHorizontalSlider.cpp
//     (W3DGadgetHorizontalSliderDraw, W3DGadgetHorizontalSliderImageDraw)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DTextEntry.cpp (drawTextEntryText,
//     W3DGadgetTextEntryDraw, W3DGadgetTextEntryImageDraw)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DComboBox.cpp (W3DGadgetComboBoxDraw,
//     W3DGadgetComboBoxImageDraw)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DTabControl.cpp
//     (W3DGadgetTabControlDraw, W3DGadgetTabControlImageDraw)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/Gadget/W3DProgressBar.cpp
//     (W3DGadgetProgressBarDraw, W3DGadgetProgressBarImageDraw)
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
//! - List box (#70): the title, then the border and background (index 0), the rows in view from `displayPos`, each
//!   cell clipped to its column (text at `x + 5`, images centred), and the selected rows' bar from images 1 to 4 (left,
//!   right, centre, small centre). The image draw draws image 0 less the slider's width instead of the colours.
//! - Sliders (#70): the colour draw is the border and colour 0 of the state; the vertical image draw stacks the top
//!   (0), the repeated centre (2), the small centre (3) and the bottom (1); the horizontal image draw is a row of
//!   boxes, filled (the **disabled** image 0) up to the position and blank (disabled image 1) after, with the hilite
//!   image 0 behind them when hilited.
//! - Text entry (#72): the back (colour 0) or the images (left 0, right 1, a repeated centre 2 and small centre 3),
//!   then the text (or `*`s) 5 pixels in, clipped to the field and scrolled by half its width once it is wider; drawn
//!   from its start (a non-editable combo box's edit box), clipped to the window. While the entry, or its combo box,
//!   has the focus, a caret 2 pixels wide blinks every 8 focused draws.
//! - Combo box (#72): its title, then the border and colour 0 below it; the image draw draws image 0 over it all.
//! - Tab control (#72): `W3DGameWinDefaultDraw`, the border (with `BORDER`), then each tab: draw data `1 + tab` of
//!   the disabled state (a disabled tab), the hilite state (the active one) or the enabled state.
//! - Progress bar (#72): the back (colour 0), the bar (colour 4) over `progress` % of the width with a white top line
//!   and a light grey left line; the image draw repeats the back's centre between its ends and the bar's centre (6)
//!   for the progress, the bar's right piece (5) after it.
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
//! - The list box's colour draw asks `drawListBoxText` for **images** (`useImages` TRUE): its selection bar is the
//!   image bar, never the selection colours. A list box's own draw sets each cell's font to the window's (and, with
//!   `ONE_LINE`, its word wrap to 0), which later row heights see.
//! - `winDrawBorder` takes the list box's slider adjustment from the thumb's **height**, off the width.
//! - The text entry with `ONE_LINE` puts its text at `size.y / 2 - fontHeight / 2`, without the window's `y`.
//! - The caret's counter (`static Byte drawCnt`) is shared by every text entry and counts only focused draws.
//! - A tab control with `BORDER` draws its border twice (its draw, then `drawWindow`).
//! - The progress bar's image draw does not test the bar's centre image (a `NULL` one is dereferenced: recorded as
//!   undefined), and draws the back's last, clipped centre piece only when there is a gap.
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
        DrawFunc::W3DDefault => w3d_game_win_default_draw(wm, w, out),
        DrawFunc::PushButton => push_button_draw(wm, h, w, out),
        DrawFunc::PushButtonImage => push_button_image_draw(wm, h, w, out),
        DrawFunc::CheckBox => check_box_draw(wm, h, w, out),
        DrawFunc::CheckBoxImage => check_box_image_draw(wm, h, w, out),
        DrawFunc::RadioButton => radio_button_draw(wm, h, w, out),
        DrawFunc::RadioButtonImage => radio_button_image_draw(wm, h, w, out),
        DrawFunc::StaticText => static_text_draw(wm, h, w, out),
        DrawFunc::StaticTextImage => static_text_image_draw(wm, h, w, out),
        DrawFunc::ListBox => list_box_draw(wm, h, w, out),
        DrawFunc::ListBoxImage => list_box_image_draw(wm, h, w, out),
        DrawFunc::VerticalSlider | DrawFunc::HorizontalSlider => slider_draw(wm, w, out),
        DrawFunc::VerticalSliderImage => vertical_slider_image_draw(wm, h, w, out),
        DrawFunc::HorizontalSliderImage => horizontal_slider_image_draw(wm, h, w, out),
        DrawFunc::TextEntry => text_entry_draw(wm, h, w, out),
        DrawFunc::TextEntryImage => text_entry_image_draw(wm, h, w, out),
        DrawFunc::ComboBox => combo_box_draw(wm, h, w, out),
        DrawFunc::ComboBoxImage => combo_box_image_draw(wm, h, w, out),
        DrawFunc::TabControl => tab_control_draw(wm, h, w, out, false),
        DrawFunc::TabControlImage => tab_control_draw(wm, h, w, out, true),
        DrawFunc::ProgressBar => progress_bar_draw(wm, w, out),
        DrawFunc::ProgressBarImage => progress_bar_image_draw(wm, h, w, out),
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

/// `drawHiliteBar(left, right, center, smallCenter, startX, startY, endX, endY)`.
#[allow(clippy::too_many_arguments)]
fn draw_hilite_bar<H: GuiHost>(wm: &mut GameWindowManager, h: &H, imgs: [&Vec<u8>; 4], sx: i32, sy: i32, ex: i32, ey: i32, out: &mut Vec<DrawItem>) {
    let [left, right, center, small] = imgs;
    let size = |i: &Vec<u8>| h.image_size(i).unwrap_or((0, 0));
    let (ls, rs, cs, ss) = (size(left), size(right), size(center), size(small));
    let mut bar = (ex - sx, ey - sy);
    if bar.0 < ls.0 + rs.0 {
        bar.0 = ls.0 + rs.0;
    }
    let left_end = (sx + ls.0, sy + bar.1);
    let right_start = (sx + bar.0 - rs.0, sy);
    let mut center_width = right_start.0 - left_end.0;
    // a zero-width image divides by zero upstream: nothing is drawn
    if cs.0 == 0 || ss.0 == 0 {
        return;
    }
    let pieces = center_width / cs.0;
    let mut x = left_end.0;
    let (y0, y1) = (sy, sy + bar.1);
    for _ in 0..pieces {
        image(out, wm, Some(center), x, y0, x + cs.0, y1);
        x += cs.0;
    }
    wm.clip = Some((left_end.0, sy, left_end.0 + center_width, y0 + bar.1));
    center_width = right_start.0 - x;
    if center_width != 0 {
        let pieces = center_width / ss.0 + 1;
        for _ in 0..pieces {
            image(out, wm, Some(small), x, y0, x + ss.0, y1);
            x += ss.0;
        }
    }
    wm.clip = None;
    image(out, wm, Some(left), sx, sy, left_end.0, left_end.1);
    image(out, wm, Some(right), right_start.0, right_start.1, right_start.0 + rs.0, right_start.1 + bar.1);
}

/// `drawListBoxText(window, instData, x, y, width, height, useImages)`: the rows in view, the selection bar, each
/// cell clipped to its column. The cells' display strings take the window's font (and, with `ONE_LINE`, no word
/// wrap) as upstream's draw sets them.
#[allow(clippy::too_many_arguments)]
fn draw_list_box_text<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId, x: i32, y: i32, width: i32, height: i32, use_images: bool, out: &mut Vec<DrawItem>) {
    use crate::client::gadget_list_box::{CellData, LISTBOX_IMAGE, LISTBOX_TEXT, TEXT_X_OFFSET};
    let UserData::Listbox(l) = &wm.ext[w].user else { return };
    let l = l.clone();
    let Some(data) = l.list_data.clone() else { return };
    let clip = (x + 1, y - 3, x + width - 1, y + height - 1);
    let mut draw_y = y - i32::from(l.display_pos);
    let win = wm.tree.windows[w].clone();
    let st = draw_state(&win);
    let window_font = win.inst.font.clone();
    let one_line = win.status & win_status::ONE_LINE != 0;
    let slider_hidden = l.slider.is_some_and(|s| wm.tree.windows[s].status & win_status::HIDDEN != 0);
    let mut i = 0usize;
    loop {
        if i > 0 && data.get(i - 1).is_none_or(|r| r.list_height > i32::from(l.display_pos) + i32::from(l.display_height)) {
            break;
        }
        if i as i32 == i32::from(l.end_pos) {
            break;
        }
        let Some(r) = data.get(i).copied() else { break };
        if r.list_height < i32::from(l.display_pos) {
            draw_y += i32::from(r.height) + 1;
            i += 1;
            continue;
        }
        let line = i32::from(r.height) + 1;
        let selected = if l.multi_select { l.selections.as_ref().is_some_and(|s| s.iter().take_while(|&&v| v >= 0).any(|&v| v == i as i32)) } else { i as i32 == l.select_pos };
        if selected {
            let (mut s0, mut e0) = ((x, draw_y), (x + width, draw_y + line));
            if e0.1 > clip.3 {
                e0.1 = clip.3;
            }
            if s0.1 < clip.1 {
                s0.1 = clip.1;
            }
            if use_images {
                let im = |k: usize| dd(&win, st, k).image.clone();
                if let (Some(a), Some(b), Some(c), Some(d)) = (im(1), im(2), im(3), im(4)) {
                    draw_hilite_bar(wm, h, [&a, &b, &c, &d], s0.0 + 1, s0.1, e0.0, e0.1, out);
                }
            } else {
                let d = dd(&win, st, 1);
                if d.border_color != WIN_COLOR_UNDEFINED {
                    open_rect(out, d.border_color, s0.0, s0.1, e0.0, e0.1);
                }
                let (mut s1, mut e1) = ((x + 1, draw_y + 1), (x + 1 + width - 2, draw_y + 1 + line - 2));
                if e1.1 > clip.3 {
                    e1.1 = clip.3;
                }
                if s1.1 < clip.1 {
                    s1.1 = clip.1;
                }
                if d.color != WIN_COLOR_UNDEFINED {
                    fill_rect(out, d.color, s1.0, s1.1, e1.0, e1.1);
                }
            }
        }
        // `winMakeColor(0, 0, 0, 255)`
        let drop = 0xFF00_0000u32 as i32;
        if let Some(a) = r.cell {
            let mut column_x = x;
            for j in 0..usize::try_from(l.columns).unwrap_or(0) {
                let cw = l.column_width.as_ref().and_then(|c| c.get(j)).copied().unwrap_or(0);
                let hi_x = if l.columns == 1 && slider_hidden { column_x + width - 3 } else { column_x + cw };
                let mut region = (column_x, draw_y, hi_x, draw_y + i32::from(r.height));
                if region.1 < clip.1 {
                    region.1 = clip.1;
                }
                if region.3 > clip.3 {
                    region.3 = clip.3;
                }
                let Some(Some(cells)) = (match &wm.ext[w].user { UserData::Listbox(l) => l.arrays.get(a).cloned(), _ => None }) else { break };
                let Some(c) = cells.get(j).cloned() else { break };
                if c.cell_type == LISTBOX_TEXT {
                    if let CellData::Text(mut d) = c.data {
                        if one_line {
                            d.word_wrap = 0;
                        }
                        if d.font != window_font {
                            d.font = window_font.clone();
                        }
                        out.push(DrawItem::Text { text: d.text.clone(), x: column_x + TEXT_X_OFFSET, y: draw_y, color: c.color, drop_color: drop, font: d.font.clone(), word_wrap: (d.word_wrap > 0).then_some(d.word_wrap), wrap_centered: false, hotkey: false, clip: Some(region) });
                        if let UserData::Listbox(l) = &mut wm.ext[w].user {
                            if let Some(cc) = l.arrays.get_mut(a).and_then(|x| x.as_mut()).and_then(|x| x.get_mut(j)) {
                                cc.data = CellData::Text(d);
                            }
                        }
                    }
                } else if c.cell_type == LISTBOX_IMAGE {
                    if let CellData::Image(img) = &c.data {
                        let mut iw = if c.width > 0 { c.width } else { cw };
                        let ih = if c.height > 0 { c.height } else { i32::from(r.height) };
                        if j == 0 {
                            iw -= 1;
                        }
                        let mut ox = if iw < cw { column_x + (cw - iw) / 2 } else { column_x };
                        let mut oy = if ih < i32::from(r.height) { draw_y + (i32::from(r.height) - ih) / 2 } else { draw_y };
                        oy += 1;
                        if ox < x + 1 {
                            ox = x + 1;
                        }
                        wm.clip = Some(region);
                        image_mode(out, wm, Some(img), ox, oy, ox + iw, oy + ih, c.color as u32, false);
                    }
                }
                column_x += cw;
            }
        }
        draw_y += line;
        wm.clip = None;
        i += 1;
    }
}

/// `W3DGadgetListBoxDraw`: the title, the border and background colours (index 0), the rows (with images for the
/// selection, as upstream asks even here).
pub fn list_box_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((mut x, mut y), (width0, mut height)) = place(wm, w);
    let _ = &mut x;
    let win = wm.tree.windows[w].clone();
    let fh = h.font_height(win.inst.font.as_ref());
    let st = draw_state(&win);
    let d = dd(&win, st, 0);
    let (bg, border) = (d.color, d.border_color);
    let (tc, tb) = text_colors(&win, st);
    let mut width = width0;
    if let Some(t) = win.text.as_ref().filter(|t| !t.is_empty()) {
        out.push(DrawItem::Text { text: t.clone(), x: x + 1, y, color: tc, drop_color: tb, font: win.inst.font.clone(), word_wrap: None, wrap_centered: false, hotkey: false, clip: None });
        y += fh + 1;
        height -= fh + 1;
    }
    if border != WIN_COLOR_UNDEFINED {
        open_rect(out, border, x, y, x + width, y + height);
    }
    if bg != WIN_COLOR_UNDEFINED {
        fill_rect(out, bg, x + 1, y + 1, x + width - 1, y + height - 1);
    }
    if let UserData::Listbox(l) = &wm.ext[w].user {
        if let Some(s) = l.slider {
            if wm.tree.windows[s].status & win_status::HIDDEN == 0 {
                width -= wm.tree.windows[s].size.0 + 3;
            }
        }
    }
    draw_list_box_text(wm, h, w, x, y + 4, width, height - 4, true, out);
}

/// `W3DGadgetListBoxImageDraw`: the state's image 0 (less the slider's width), the title, the rows.
pub fn list_box_image_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((x, mut y), (mut width, mut height)) = place(wm, w);
    let win = wm.tree.windows[w].clone();
    if let UserData::Listbox(l) = &wm.ext[w].user {
        if let Some(s) = l.slider {
            width -= wm.tree.windows[s].size.0;
        }
    }
    let st = draw_state(&win);
    let (tc, tb) = text_colors(&win, st);
    if let Some(img) = dd(&win, st, 0).image.clone() {
        let (sx, sy) = (x + win.inst.image_offset.0, y + win.inst.image_offset.1);
        image(out, wm, Some(&img), sx, sy, sx + width, sy + height);
    }
    if let Some(t) = win.text.as_ref().filter(|t| !t.is_empty()) {
        out.push(DrawItem::Text { text: t.clone(), x: x + 1, y, color: tc, drop_color: tb, font: win.inst.font.clone(), word_wrap: None, wrap_centered: false, hotkey: false, clip: None });
        let fh = h.font_height(win.inst.font.as_ref());
        y += fh;
        height -= fh + 1;
    }
    draw_list_box_text(wm, h, w, x, y + 4, width, height - 4, true, out);
}

/// `W3DGadgetVerticalSliderDraw` / `W3DGadgetHorizontalSliderDraw` (the same): the state's border and colour 0.
pub fn slider_draw(wm: &GameWindowManager, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let d = dd(win, draw_state(win), 0);
    if d.border_color != WIN_COLOR_UNDEFINED {
        open_rect(out, d.border_color, ox, oy, ox + sx, oy + sy);
    }
    if d.color != WIN_COLOR_UNDEFINED {
        fill_rect(out, d.color, ox + 1, oy + 1, ox + 1 + sx - 2, oy + 1 + sy - 2);
    }
}

/// `W3DGadgetVerticalSliderImageDraw`: top (0) and bottom (1) ends, the centre (2) repeated and the small centre (3)
/// up to the bottom; only when all four images are set.
pub fn vertical_slider_image_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (_sx, sy)) = place(wm, w);
    let win = wm.tree.windows[w].clone();
    let (xo, yo) = win.inst.image_offset;
    let st = draw_state(&win);
    let im = |k: usize| dd(&win, st, k).image.clone();
    let (Some(top), Some(bot), Some(cen), Some(small)) = (im(0), im(1), im(2), im(3)) else { return };
    let size = |i: &Vec<u8>| h.image_size(i).unwrap_or((0, 0));
    let (ts, bs, cs, ss) = (size(&top), size(&bot), size(&cen), size(&small));
    if ts.1 + bs.1 >= sy {
        image(out, wm, Some(&top), ox + xo, oy + yo, ox + xo + ts.0, oy + sy / 2);
        image(out, wm, Some(&bot), ox + xo, oy + sy / 2, ox + xo + bs.0, oy + yo + sy);
        return;
    }
    let top_end = (ox + ts.0 + xo, oy + ts.1 + yo);
    let bottom_start = (ox + xo, oy + sy - bs.1 + yo);
    if cs.1 == 0 || ss.1 == 0 {
        return;
    }
    let pieces = (bottom_start.1 - top_end.1) / cs.1;
    let (x0, mut y0) = (ox + xo, top_end.1);
    let x1 = x0 + cs.0;
    let mut y1 = y0 + cs.1;
    for _ in 0..pieces {
        image(out, wm, Some(&cen), x0, y0, x1, y1);
        y0 += cs.1;
        y1 += cs.1;
    }
    let pieces = (bottom_start.1 - y0) / ss.1 + 1;
    y1 = y0 + ss.1;
    for _ in 0..pieces {
        image(out, wm, Some(&small), x0, y0, x1, y1);
        y0 += ss.1;
        y1 += ss.1;
    }
    image(out, wm, Some(&top), ox + xo, oy + yo, top_end.0, top_end.1);
    image(out, wm, Some(&bot), bottom_start.0, bottom_start.1, bottom_start.0 + bs.0, bottom_start.1 + bs.1);
}

/// `W3DGadgetHorizontalSliderImageDraw`: a row of boxes, the filled ones (disabled image 0) up to the position, the
/// blank ones (disabled image 1) after, and with the hilite the hilite image 0 behind them. Upstream dereferences the
/// images without a test: without them nothing is drawn.
pub fn horizontal_slider_image_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    use crate::client::gadget_slider::real_to_int_x86;
    let ((mut ox, oy), (sx, _sy)) = place(wm, w);
    let win = wm.tree.windows[w].clone();
    let (Some(hi), Some(blank), Some(fill)) = (win.inst.hilite_draw_data[0].image.clone(), win.inst.disabled_draw_data[1].image.clone(), win.inst.disabled_draw_data[0].image.clone()) else { return };
    let UserData::Slider(s) = wm.ext[w].user else { return };
    let x_multi = wm.display.0 as f32 / 800.0;
    let fw = h.image_size(&fill).unwrap_or((0, 0)).0;
    let box_w = real_to_int_x86(fw as f32 * x_multi);
    let pad = 2;
    let (mut start_x, mut end_x) = (ox, ox + box_w);
    let pct = s.position.wrapping_sub(s.min_val) as f32 / s.max_val.wrapping_sub(s.min_val) as f32;
    let max_sel = ox + crate::common::base_type::real_to_int(pct * sx as f32);
    let (mut boxes, mut sel) = (0, 0);
    while end_x < ox + sx {
        if start_x <= max_sel && end_x < ox + sx && s.position != s.min_val {
            sel += 1;
        }
        start_x = end_x + pad;
        end_x = start_x + box_w;
        boxes += 1;
    }
    let hi_boxes = boxes + 1;
    let covered = end_x - ox - box_w;
    let hoff = (-(box_w + pad) / 2, box_w / 3);
    ox += (sx - covered) / 2;
    if win.inst.state & WIN_STATE_HILITED != 0 {
        let by = oy + hoff.1;
        for i in 0..hi_boxes {
            let bx = ox + hoff.0 + i * (box_w + pad);
            image(out, wm, Some(&hi), bx, by, bx + box_w + pad, by + box_w + pad);
        }
    }
    for i in 0..boxes {
        let x0 = ox + i * (box_w + pad);
        let img = if i < sel { &fill } else { &blank };
        image(out, wm, Some(img), x0, oy, x0 + box_w, oy + box_w);
    }
}

/// `drawTextEntryText`: the entry's text (or its secret text) in the window's font, clipped to the field (or, drawn
/// from its start, to the window), scrolled by half widths when it is wider than the field, and the caret, blinking
/// every 8 draws while the entry (or its combo box) has the focus. The IME's construction text is always empty.
#[allow(clippy::too_many_arguments)]
fn draw_text_entry_text<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId, text_color: i32, text_drop: i32, x: i32, y: i32, width: i32, font_height: i32, out: &mut Vec<DrawItem>) {
    let font = wm.tree.windows[w].inst.font.clone();
    let UserData::Entry(e) = &mut wm.ext[w].user else { return };
    e.construct_text.text.clear();
    if text_color == WIN_COLOR_UNDEFINED {
        return;
    }
    // the strings take the window's font
    let secret = e.secret_text;
    let ds = if secret { &mut e.s_text } else { &mut e.text };
    ds.font = font.clone();
    let text = ds.text.clone();
    e.construct_text.font = font.clone();
    let from_start = e.draw_text_from_start;
    let text_width = h.text_size(font.as_ref(), &text, None).0;
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let mut x = x;
    let cursor;
    let draw = |out: &mut Vec<DrawItem>, at: i32, clip: (i32, i32, i32, i32)| out.push(DrawItem::Text { text: text.clone(), x: at, y, color: text_color, drop_color: text_drop, font: font.clone(), word_wrap: None, wrap_centered: false, hotkey: false, clip: Some(clip) });
    if !from_start {
        let clip = (x, y, x + width, y + font_height);
        x += 2;
        if text_width < width {
            draw(out, x, clip);
            cursor = text_width + x;
        } else {
            if width / 2 == 0 {
                wm.set_undefined("drawTextEntryText: textWidth / (width / 2) with width / 2 == 0 (division by zero)");
                return;
            }
            let div = text_width / (width / 2) - 1;
            draw(out, x - div * (width / 2), clip);
            cursor = text_width - div * (width / 2) + x;
        }
    } else {
        let clip = (ox, oy, ox + sx, oy + sy);
        x += 5;
        draw(out, x, clip);
        cursor = text_width + x;
    }
    // the caret: `drawCnt` counts only the draws where the entry (or its combo box) has the focus
    let focus = wm.keyboard_focus;
    let parent = crate::client::gadget_text_entry::combo_parent(wm, w);
    if focus == Some(w) || (parent.is_some() && parent == focus) {
        let cnt = wm.entry_draw_cnt;
        wm.entry_draw_cnt = cnt.wrapping_add(1);
        if (cnt >> 3) & 1 != 0 {
            fill_rect(out, text_color, cursor, oy + 3, cursor + 2, oy + sy - 3);
        }
    }
}

/// The text colours and the `Gadget*Get{state}...` draw data index 0 of the entry's state.
fn entry_state(win: &GameWindow) -> (DrawState, (i32, i32)) {
    let st = draw_state(win);
    (st, text_colors(win, st))
}

/// `W3DGadgetTextEntryDraw`: the back border and fill (colour 0 of the state), then the text 5 pixels in (with
/// `ONE_LINE`, at `size.y / 2 - fontHeight / 2`, **without** the window's `y`).
pub fn text_entry_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    if let UserData::Entry(e) = &mut wm.ext[w].user {
        e.received_unichar = false;
    }
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let (st, (tc, tb)) = entry_state(win);
    let d = dd(win, st, 0).clone();
    if d.border_color != WIN_COLOR_UNDEFINED {
        open_rect(out, d.border_color, ox, oy, ox + sx, oy + sy);
    }
    if d.color != WIN_COLOR_UNDEFINED {
        fill_rect(out, d.color, ox + 1, oy + 1, ox + 1 + sx - 2, oy + 1 + sy - 2);
    }
    entry_text_at(wm, h, w, tc, tb, out);
}

/// The end of both text entry draws: where the text goes.
fn entry_text_at<H: GuiHost>(wm: &mut GameWindowManager, h: &H, w: WinId, tc: i32, tb: i32, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let fh = h.font_height(win.inst.font.as_ref());
    let y = if win.status & win_status::ONE_LINE != 0 { sy / 2 - fh / 2 } else { oy + 5 };
    draw_text_entry_text(wm, h, w, tc, tb, ox + 5, y, sx - 10, fh, out);
}

/// `W3DGadgetTextEntryImageDraw`: the repeated centre (2), the small centre (3) up to the right end, the left (0) and
/// right (1) ends, then the text. Upstream dereferences all four images: without one, or with a zero-width centre,
/// it is undefined (recorded, nothing drawn).
pub fn text_entry_image_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    if let UserData::Entry(e) = &mut wm.ext[w].user {
        e.received_unichar = false;
    }
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = wm.tree.windows[w].clone();
    let (xo, yo) = win.inst.image_offset;
    let (st, (tc, tb)) = entry_state(&win);
    let im = |k: usize| dd(&win, st, k).image.clone();
    let (Some(left), Some(right), Some(center), Some(small)) = (im(0), im(1), im(2), im(3)) else {
        wm.set_undefined("W3DGadgetTextEntryImageDraw: a NULL image dereferenced");
        return;
    };
    let size = |i: &Vec<u8>| h.image_size(i).unwrap_or((0, 0));
    let (ls, rs, cs, ss) = (size(&left), size(&right), size(&center), size(&small));
    if cs.0 == 0 || ss.0 == 0 {
        wm.set_undefined("W3DGadgetTextEntryImageDraw: a centre image of width 0 (division by zero)");
        return;
    }
    let left_end = (ox + ls.0 + xo, oy + sy + yo);
    let right_start = (ox + sx - rs.0 + xo, oy + yo);
    let pieces = (right_start.0 - left_end.0) / cs.0;
    let (mut x, y0) = (left_end.0, oy + yo);
    let y1 = y0 + sy;
    for _ in 0..pieces {
        image(out, wm, Some(&center), x, y0, x + cs.0, y1);
        x += cs.0;
    }
    let pieces = (right_start.0 - x) / ss.0 + 1;
    for _ in 0..pieces {
        image(out, wm, Some(&small), x, y0, x + ss.0, y1);
        x += ss.0;
    }
    image(out, wm, Some(&left), ox + xo, oy + yo, left_end.0, left_end.1);
    image(out, wm, Some(&right), right_start.0, right_start.1, right_start.0 + rs.0, right_start.1 + sy);
    entry_text_at(wm, h, w, tc, tb, out);
}

/// The combo box's title, if it has one: drawn at `(x + 1, y)` in the state's text colours.
fn combo_title(wm: &GameWindowManager, w: WinId, x: i32, y: i32, color: i32, drop: i32, out: &mut Vec<DrawItem>) -> bool {
    let win = &wm.tree.windows[w];
    let Some(t) = win.text.as_ref().filter(|t| !t.is_empty()) else { return false };
    out.push(DrawItem::Text { text: t.clone(), x: x + 1, y, color, drop_color: drop, font: win.inst.font.clone(), word_wrap: None, wrap_centered: false, hotkey: false, clip: None });
    true
}

/// `W3DGadgetComboBoxDraw`: the title, then the border and the background (colour 0 of the state) below it.
pub fn combo_box_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((x, mut y), (width, mut height)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let fh = h.font_height(win.inst.font.as_ref());
    let st = draw_state(win);
    let d = dd(win, st, 0).clone();
    let (tc, tb) = text_colors(win, st);
    if combo_title(wm, w, x, y, tc, tb, out) {
        y += fh + 1;
        height -= fh + 1;
    }
    if d.border_color != WIN_COLOR_UNDEFINED {
        open_rect(out, d.border_color, x, y, x + width, y + height);
    }
    if d.color != WIN_COLOR_UNDEFINED {
        fill_rect(out, d.color, x + 1, y + 1, x + width - 1, y + height - 1);
    }
}

/// `W3DGadgetComboBoxImageDraw`: image 0 of the state over the whole window (title included), then the title.
pub fn combo_box_image_draw<H: GuiHost>(wm: &mut GameWindowManager, _h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((x, y), (width, height)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let st = draw_state(win);
    let (xo, yo) = win.inst.image_offset;
    let img = dd(win, st, 0).image.clone();
    let (tc, tb) = text_colors(win, st);
    image(out, wm, img.as_ref(), x + xo, y + yo, x + xo + width, y + yo + height);
    combo_title(wm, w, x, y, tc, tb, out);
}

/// `W3DGadgetTabControlDraw` / `W3DGadgetTabControlImageDraw`: the background (`W3DGameWinDefaultDraw`), the border
/// with `BORDER` (and not `SEE_THRU`), then each tab that exists (`tabCount`), along the edge from the tab rectangle's
/// corner: draw data `1 + tab` of the disabled state if the tab is disabled, else hilite if it is the active tab, else
/// enabled (border and colour, or the image).
pub fn tab_control_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>, images: bool) {
    let (ox, oy) = wm.tree.screen_position(w);
    w3d_game_win_default_draw(wm, w, out);
    let s = wm.tree.windows[w].status;
    if s & win_status::BORDER != 0 && s & win_status::SEE_THRU == 0 {
        win_draw_border(wm, h, w, out);
    }
    let Some(t) = crate::client::gadget_tab_control::tab_rt(wm, w).cloned() else {
        wm.set_undefined("W3DGadgetTabControlDraw: no TabControlData");
        return;
    };
    let win = &wm.tree.windows[w];
    let (mut tx, mut ty) = (ox + t.tabs_left_limit, oy + t.tabs_top_limit);
    let (tw, th) = (t.tab_width, t.tab_height);
    let along_x = t.tab_edge == crate::client::gadget_tab_control::tp::TOP_SIDE || t.tab_edge == crate::client::gadget_tab_control::tp::BOTTOM_SIDE;
    let (dx, dy) = if along_x { (tw, 0) } else { (0, th) };
    for k in 0..crate::client::window_script::NUM_TAB_PANES {
        if t.tab_count > k as i32 {
            let st = if t.sub_pane_disabled[k] {
                DrawState::Disabled
            } else if t.active_tab == k as i32 {
                DrawState::Hilite
            } else {
                DrawState::Enabled
            };
            let d = dd(win, st, 1 + k);
            if images {
                image(out, wm, d.image.as_ref(), tx, ty, tx + tw, ty + th);
            } else {
                if d.border_color != WIN_COLOR_UNDEFINED {
                    open_rect(out, d.border_color, tx, ty, tx + tw, ty + th);
                }
                if d.color != WIN_COLOR_UNDEFINED {
                    fill_rect(out, d.color, tx + 1, ty + 1, tx + tw - 1, ty + th - 1);
                }
            }
        }
        tx += dx;
        ty += dy;
    }
}

/// The progress bar's progress (its user data, an `Int`).
fn progress(wm: &GameWindowManager, w: WinId) -> i32 {
    match wm.ext[w].user {
        UserData::Progress(p) => p,
        _ => 0,
    }
}

/// `W3DGadgetProgressBarDraw`: the back (colour 0 of the state), then, with progress, the bar (colour 4) over
/// `size.x * progress / 100` (only when more than one pixel wide), with a white line on its top and a light grey one
/// on its left.
pub fn progress_bar_draw(wm: &GameWindowManager, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = &wm.tree.windows[w];
    let st = draw_state(win);
    let (back, bar) = (dd(win, st, 0), dd(win, st, 4));
    let p = progress(wm, w);
    if back.border_color != WIN_COLOR_UNDEFINED {
        open_rect(out, back.border_color, ox, oy, ox + sx, oy + sy);
    }
    if back.color != WIN_COLOR_UNDEFINED {
        fill_rect(out, back.color, ox + 1, oy + 1, ox + 1 + sx - 2, oy + 1 + sy - 2);
    }
    if p != 0 {
        if bar.border_color != WIN_COLOR_UNDEFINED {
            let ex = ox + sx.wrapping_mul(p) / 100;
            if ex - ox > 1 {
                open_rect(out, bar.border_color, ox, oy, ex, oy + sy);
            }
        }
        if bar.color != WIN_COLOR_UNDEFINED {
            let (x0, y0) = (ox + 1, oy + 1);
            let (x1, y1) = (x0 + sx.wrapping_mul(p) / 100 - 2, y0 + sy - 2);
            if x1 - x0 > 1 {
                fill_rect(out, bar.color, x0, y0, x1, y1);
                out.push(DrawItem::Line { x0, y0, x1, y1: y0, color: 0xFFFF_FFFF_u32 as i32 });
                out.push(DrawItem::Line { x0, y0, x1: x0, y1, color: 0xFFC8_C8C8_u32 as i32 });
            }
        }
    }
}

/// `W3DGadgetProgressBarImageDraw`: the back's repeated centre (2) between the ends, one more clipped to the gap, the
/// left (0) and right (1) ends; then the bar: `(size.x - 20) * progress / 100` of bar centres (6) from 10 pixels in,
/// and bar right pieces (5) for the rest of the `(size.x - 20) / width` slots. Nothing without the back's three
/// images or the bar's right one; the bar's centre is dereferenced unchecked (recorded as undefined, as is a zero
/// image width).
pub fn progress_bar_image_draw<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, out: &mut Vec<DrawItem>) {
    let ((ox, oy), (sx, sy)) = place(wm, w);
    let win = wm.tree.windows[w].clone();
    let (xo, yo) = win.inst.image_offset;
    let st = draw_state(&win);
    let im = |k: usize| dd(&win, st, k).image.clone();
    let p = progress(wm, w);
    let (Some(back_left), Some(back_right), Some(back_center), Some(bar_right)) = (im(0), im(1), im(2), im(5)) else { return };
    let size = |i: &Vec<u8>| h.image_size(i).unwrap_or((0, 0));
    let (ls, rs, cs) = (size(&back_left), size(&back_right), size(&back_center));
    if cs.0 == 0 {
        wm.set_undefined("W3DGadgetProgressBarImageDraw: a centre image of width 0 (division by zero)");
        return;
    }
    let left_end = (ox + ls.0 + xo, oy + sy + yo);
    let right_start = (ox + sx - rs.0 + xo, oy + yo);
    let pieces = (right_start.0 - left_end.0) / cs.0;
    let (mut x, y0) = (left_end.0, oy + yo);
    let y1 = y0 + sy;
    for _ in 0..pieces {
        image(out, wm, Some(&back_center), x, y0, x + cs.0, y1);
        x += cs.0;
    }
    if right_start.0 - x > 0 {
        wm.clip = Some((x, y0, right_start.0, y1));
        image(out, wm, Some(&back_center), x, y0, x + cs.0, y1);
        wm.clip = None;
    }
    image(out, wm, Some(&back_left), ox + xo, oy + yo, left_end.0, left_end.1);
    image(out, wm, Some(&back_right), right_start.0, right_start.1, right_start.0 + rs.0, right_start.1 + sy);
    let Some(bar_center) = im(6) else {
        wm.set_undefined("W3DGadgetProgressBarImageDraw: the bar's centre image NULL dereferenced");
        return;
    };
    let (bcw, brw) = (size(&bar_center).0, size(&bar_right).0);
    if bcw == 0 {
        wm.set_undefined("W3DGadgetProgressBarImageDraw: the bar's centre image of width 0 (division by zero)");
        return;
    }
    let bar_w = (sx - 20).wrapping_mul(p) / 100;
    let pieces = bar_w / bcw;
    let mut x = ox + 10;
    let y0 = oy + yo + 5;
    let y1 = y0 + sy - 10;
    for _ in 0..pieces {
        image(out, wm, Some(&bar_center), x, y0, x + bcw, y1);
        x += bcw;
    }
    let mut x = ox + 10 + bcw * pieces;
    let rest = (sx - 20) / bcw - pieces;
    for _ in 0..rest {
        image(out, wm, Some(&bar_right), x, y0, x + brw, y1);
        x += brw;
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
                // the slider adjustment is the thumb's **height** (`size.y`), taken off the width
                let mut slider = 0;
                if let UserData::Listbox(l) = &wm.ext[w].user {
                    if l.scroll_bar {
                        let thumb = l.slider.and_then(|s| wm.tree.windows[s].children.first().copied());
                        match thumb {
                            Some(t) => slider = wm.tree.windows[t].size.1,
                            // `list->slider->winGetChild()` NULL: upstream dereferences it
                            None => return,
                        }
                    }
                }
                let label = if win.text.as_ref().is_some_and(|t| !t.is_empty()) { 4 } else { 0 };
                blit_border_rect(wm, &mut items, ox - 3, oy - (3 + label), win.size.0 + 3 - slider, win.size.1 + 6);
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

    fn images_of(d: &[DrawItem]) -> Vec<(Vec<u8>, i32, i32, i32, i32)> {
        d.iter().filter_map(|x| if let DrawItem::Image { image, x0, y0, x1, y1, .. } = x { Some((image.clone(), *x0, *y0, *x1, *y1)) } else { None }).collect()
    }

    fn entry(wm: &mut GameWindowManager, h: &mut Rec, status: u32) -> WinId {
        let mut inst = WinInstanceData { style: gws::ENTRY_FIELD, text_label_string: b"abc".to_vec(), ..WinInstanceData::new() };
        colours(&mut inst);
        inst.enabled_text.color = 0x7001;
        inst.hilite_text.color = 0x7003;
        inst.font = Some(crate::client::window_script::GameFont { name: b"F".to_vec(), point_size: 10, bold: false });
        wm.gogo_gadget(h, None, status, 10, 20, 100, 20, inst, GadgetData::Entry(Default::default())).expect("an entry")
    }

    /// #72: the text entry's colour draw. Predicted from W3DTextEntry.cpp (entry at (10,20), 100x20, font 10: text 5
    /// pixels a unit, 10 high; label `abc`):
    /// - enabled: `OpenRect(10,20,100,20, 0x110)`, `FillRect(11,21,98,18, 0x100)`, the text at (17, 25) in 0x7001,
    ///   clipped to (15, 25, 105, 35);
    /// - with the focus (hilited and selected): 0x310 / 0x300, text 0x7003; no caret on the first 8 focused draws, a
    ///   caret `FillRect(32, 23, 2, 14)` (text end 17 + 15) on the 9th;
    /// - 40 characters (200 wide, over the 90 of the field): scrolled by `(200 / 45 - 1) * 45` = 135: at x -118;
    /// - secret: `***` drawn; `ONE_LINE`: y = 20 / 2 - 10 / 2 = 5 (without the window's y); drawn from its start: at
    ///   x 20, clipped to the window.
    #[test]
    fn a_text_entry_draws_as_upstream_draws_it() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let e = entry(&mut wm, &mut h, win_status::ENABLED);
        let draw = |wm: &mut GameWindowManager, h: &mut Rec| {
            let mut out = Vec::new();
            text_entry_draw(wm, h, e, &mut out);
            out
        };
        let d = draw(&mut wm, &mut h);
        assert_eq!(d[..2], [DrawItem::OpenRect { x: 10, y: 20, w: 100, h: 20, color: 0x110 }, DrawItem::FillRect { x: 11, y: 21, w: 98, h: 18, color: 0x100 }]);
        assert!(matches!(&d[2], DrawItem::Text { x: 17, y: 25, color: 0x7001, clip: Some((15, 25, 105, 35)), .. }), "{:?}", d[2]);
        assert_eq!(d.len(), 3);
        wm.win_set_focus(&mut h, Some(e));
        for _ in 0..8 {
            let d = draw(&mut wm, &mut h);
            assert_eq!(d.len(), 3);
            assert!(matches!(d[0], DrawItem::OpenRect { color: 0x310, .. }) && matches!(&d[2], DrawItem::Text { color: 0x7003, .. }));
        }
        let d = draw(&mut wm, &mut h);
        assert_eq!(d[3], DrawItem::FillRect { x: 32, y: 23, w: 2, h: 14, color: 0x7003 });
        wm.win_set_focus(&mut h, None);
        let long: Vec<u16> = vec![u16::from(b'x'); 40];
        crate::client::gadget_text_entry::set_text(&mut wm, &mut h, Some(e), &long);
        let d = draw(&mut wm, &mut h);
        assert!(matches!(&d[2], DrawItem::Text { x: -118, .. }), "{:?}", d[2]);
        if let UserData::Entry(x) = &mut wm.ext[e].user {
            x.secret_text = true;
            x.draw_text_from_start = true;
        }
        wm.win_set_status(e, win_status::ONE_LINE);
        let d = draw(&mut wm, &mut h);
        assert!(matches!(&d[2], DrawItem::Text { x: 20, y: 5, clip: Some((10, 20, 110, 40)), text, .. } if text.len() == 40 && text.iter().all(|&c| c == u16::from(b'*'))), "{:?}", d[2]);
        // no text colour: no text
        wm.win_mut(e).inst.enabled_text.color = WIN_COLOR_UNDEFINED;
        assert_eq!(draw(&mut wm, &mut h).len(), 2);
    }

    /// #72: the text entry's image draw. Predicted (images 5x10; entry 100x20 at (10,20)): 18 centres from x 15 to 105,
    /// one small centre at 105, the left end (10,20)-(15,40), the right end (105,20)-(110,40), then the text; without
    /// an image upstream dereferences `NULL` (recorded).
    #[test]
    fn a_text_entry_draws_its_images_as_upstream_draws_them() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let e = entry(&mut wm, &mut h, win_status::ENABLED | win_status::IMAGE);
        assert_eq!(wm.ext[e].draw, DrawFunc::TextEntryImage);
        let d = wm.win_repaint(&mut h);
        let im = images_of(&d);
        assert_eq!(im.len(), 21);
        assert_eq!(im[0], (b"Img100_2".to_vec(), 15, 20, 20, 40));
        assert_eq!(im[17], (b"Img100_2".to_vec(), 100, 20, 105, 40));
        assert_eq!(im[18], (b"Img100_3".to_vec(), 105, 20, 110, 40));
        assert_eq!(im[19..], [(b"Img100_0".to_vec(), 10, 20, 15, 40), (b"Img100_1".to_vec(), 105, 20, 110, 40)]);
        assert!(matches!(d.last(), Some(DrawItem::Text { x: 17, .. })));
        wm.win_mut(e).inst.enabled_draw_data[3].image = None;
        wm.win_repaint(&mut h);
        assert!(wm.undefined.is_some());
    }

    /// #72: the combo box's draws. Predicted from W3DComboBox.cpp (120x20 at (10,20), font 10, title `T`): the title at
    /// (11, 20), then the border at y 31 (20 + 10 + 1), 9 high, and the fill inside; the image draw puts image 0 over
    /// the whole window and the title on it.
    #[test]
    fn a_combo_box_draws_as_upstream_draws_it() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let mut inst = WinInstanceData { style: gws::COMBO_BOX, ..WinInstanceData::new() };
        colours(&mut inst);
        inst.font = Some(crate::client::window_script::GameFont { name: b"F".to_vec(), point_size: 10, bold: false });
        let c = wm.gogo_gadget(&mut h, None, win_status::ENABLED, 10, 20, 120, 20, inst, GadgetData::ComboBox(Default::default())).expect("a combo box");
        wm.win_mut(c).inst.enabled_text.color = 0x7001;
        let t: Vec<u16> = "T".encode_utf16().collect();
        wm.win_set_text(c, &t);
        let mut out = Vec::new();
        combo_box_draw(&mut wm, &mut h, c, &mut out);
        assert!(matches!(&out[0], DrawItem::Text { x: 11, y: 20, color: 0x7001, .. }));
        assert_eq!(out[1..], [DrawItem::OpenRect { x: 10, y: 31, w: 120, h: 9, color: 0x110 }, DrawItem::FillRect { x: 11, y: 32, w: 118, h: 7, color: 0x100 }]);
        let mut out = Vec::new();
        combo_box_image_draw(&mut wm, &mut h, c, &mut out);
        assert_eq!(images_of(&out), vec![(b"Img100_0".to_vec(), 10, 20, 130, 40)]);
        assert!(matches!(&out[1], DrawItem::Text { x: 11, y: 20, .. }));
        assert_eq!(wm.ext[c].draw, DrawFunc::ComboBox);
    }

    /// #72: the tab control's draws. Predicted from W3DTabControl.cpp (200x100 at (10,10), border 4, tabs 30x20, 3
    /// tabs on the top, centred: the tabs from (65, 14)): the background (enabled 0), then tab 0, active: hilite 1
    /// (`OpenRect(65,14,30,20, 0x311)`, fill 0x301 inside); tab 1: enabled 2 (0x112 / 0x102) at x 95; tab 2,
    /// disabled: disabled 3 (0x213 / 0x203) at x 125. The image draw: `Img300_1`, `Img100_2`, `Img200_3` at the same
    /// places, after the background image.
    #[test]
    fn a_tab_control_draws_as_upstream_draws_it() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let mut inst = WinInstanceData { style: gws::TAB_CONTROL, ..WinInstanceData::new() };
        colours(&mut inst);
        let mut td = crate::client::window_script::TabControlData { tab_orientation: 0, tab_edge: 3, tab_width: 30, tab_height: 20, tab_count: 3, pane_border: 4, ..Default::default() };
        td.sub_pane_disabled[2] = true;
        let t = wm.gogo_gadget(&mut h, None, win_status::ENABLED, 10, 10, 200, 100, inst, GadgetData::TabControl(td)).expect("a tab control");
        let mut out = Vec::new();
        tab_control_draw(&mut wm, &mut h, t, &mut out, false);
        assert_eq!(
            out,
            vec![
                DrawItem::OpenRect { x: 10, y: 10, w: 200, h: 100, color: 0x110 },
                DrawItem::FillRect { x: 11, y: 11, w: 198, h: 98, color: 0x100 },
                DrawItem::OpenRect { x: 65, y: 14, w: 30, h: 20, color: 0x311 },
                DrawItem::FillRect { x: 66, y: 15, w: 28, h: 18, color: 0x301 },
                DrawItem::OpenRect { x: 95, y: 14, w: 30, h: 20, color: 0x112 },
                DrawItem::FillRect { x: 96, y: 15, w: 28, h: 18, color: 0x102 },
                DrawItem::OpenRect { x: 125, y: 14, w: 30, h: 20, color: 0x213 },
                DrawItem::FillRect { x: 126, y: 15, w: 28, h: 18, color: 0x203 },
            ]
        );
        wm.win_set_status(t, win_status::IMAGE);
        let mut out = Vec::new();
        tab_control_draw(&mut wm, &mut h, t, &mut out, true);
        assert_eq!(images_of(&out), vec![(b"Img100_0".to_vec(), 10, 10, 210, 110), (b"Img300_1".to_vec(), 65, 14, 95, 34), (b"Img100_2".to_vec(), 95, 14, 125, 34), (b"Img200_3".to_vec(), 125, 14, 155, 34)]);
        // with a border, the border's images come after the background
        wm.win_set_status(t, win_status::BORDER);
        let mut out = Vec::new();
        tab_control_draw(&mut wm, &mut h, t, &mut out, true);
        assert!(images_of(&out).iter().any(|i| i.0 == b"BorderCornerUL"));
        assert_eq!(wm.ext[t].draw, DrawFunc::TabControl);
    }

    /// #72: the progress bar's draws. Predicted from W3DProgressBar.cpp (100x20 at (10,20)):
    /// - 50 %: the back (0x110 / 0x100), the bar's border `OpenRect(10,20,50,20, 0x114)`, its fill `FillRect(11,21,48,18,
    ///   0x104)`, a white line on its top (11,21)-(59,21) and a light grey one (0xFFC8C8C8) on its left; 1 %: no bar
    ///   (less than two pixels); 0: the back only;
    /// - the image draw at 50 %: 18 back centres (images 5 wide) from 15, both ends, then (100 - 20) * 50 / 100 = 40
    ///   pixels of bar centres (8, `Img100_6`) from x 20 (y 25 to 35) and 80 / 5 - 8 = 8 bar right pieces; 103 wide, a
    ///   back centre clipped to (105, 20, 108, 40).
    #[test]
    fn a_progress_bar_draws_as_upstream_draws_it() {
        let mut h = Rec::default();
        let mut wm = GameWindowManager::new((800, 600));
        let mut inst = WinInstanceData { style: gws::PROGRESS_BAR, ..WinInstanceData::new() };
        colours(&mut inst);
        let p = wm.gogo_gadget(&mut h, None, win_status::ENABLED, 10, 20, 100, 20, inst, GadgetData::None).expect("a progress bar");
        assert_eq!(wm.ext[p].draw, DrawFunc::ProgressBar);
        gadget::progress_bar_set_progress(&mut wm, &mut h, p, 50);
        let d = wm.win_repaint(&mut h);
        assert_eq!(
            d,
            vec![
                DrawItem::OpenRect { x: 10, y: 20, w: 100, h: 20, color: 0x110 },
                DrawItem::FillRect { x: 11, y: 21, w: 98, h: 18, color: 0x100 },
                DrawItem::OpenRect { x: 10, y: 20, w: 50, h: 20, color: 0x114 },
                DrawItem::FillRect { x: 11, y: 21, w: 48, h: 18, color: 0x104 },
                DrawItem::Line { x0: 11, y0: 21, x1: 59, y1: 21, color: -1 },
                DrawItem::Line { x0: 11, y0: 21, x1: 11, y1: 39, color: 0xFFC8_C8C8_u32 as i32 },
            ]
        );
        gadget::progress_bar_set_progress(&mut wm, &mut h, p, 1);
        assert_eq!(wm.win_repaint(&mut h).len(), 2);
        gadget::progress_bar_set_progress(&mut wm, &mut h, p, 50);
        let mut out = Vec::new();
        progress_bar_image_draw(&mut wm, &mut h, p, &mut out);
        let im = images_of(&out);
        assert_eq!(im.len(), 18 + 2 + 8 + 8);
        assert_eq!(im[20], (b"Img100_6".to_vec(), 20, 25, 25, 35));
        assert_eq!(im[28], (b"Img100_5".to_vec(), 60, 25, 65, 35));
        wm.win_set_size(&mut h, p, 103, 20);
        let mut out = Vec::new();
        progress_bar_image_draw(&mut wm, &mut h, p, &mut out);
        assert!(out.iter().any(|d| matches!(d, DrawItem::Image { x0: 105, clip: Some((105, 20, 108, 40)), .. })));
        wm.win_mut(p).inst.enabled_draw_data[6].image = None;
        progress_bar_image_draw(&mut wm, &mut h, p, &mut Vec::new());
        assert!(wm.undefined.is_some());
    }
}
