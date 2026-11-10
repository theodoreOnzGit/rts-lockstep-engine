// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetVerticalSlider.cpp (GadgetVerticalSliderInput,
//     GadgetVerticalSliderSystem)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetHorizontalSlider.cpp (GadgetHorizontalSliderInput,
//     GadgetHorizontalSliderSystem)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (gogoGadgetSlider)
//   GeneralsMD/Code/GameEngine/Include/GameClient/{Gadget.h, GadgetSlider.h} (SliderData, GADGET_SIZE, the thumb's
//     size and position, GadgetSliderSetPosition, GadgetSliderGetPosition, GadgetSliderSetMinMax)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The sliders (GitHub #70, a part of #34): the vertical and horizontal slider's input and system callbacks and
//! `gogoGadgetSlider`, which creates the slider and its **thumb**, a draggable push button child.
//!
//! # As upstream reads it
//!
//! A slider keeps `SliderData`: `minVal`, `maxVal`, `numTicks` (pixels per unit, a `Real`) and `position`. The thumb
//! is the slider's first child (`winGetChild`); its owner is the slider, so its `GBM_SELECTED` (the release of a drag)
//! reaches the slider's system, which tells its own owner `GSM_SLIDER_DONE`. Dragging the thumb: the manager moves
//! it (it is `DRAGABLE`, clipped to the slider), the thumb's input sends the slider `GGM_LEFT_DRAG` with the mouse,
//! and the slider computes its position from where the thumb now is and sends its owner `GSM_SLIDER_TRACK`.
//!
//! - **Vertical**: `maxVal` at the top. The thumb is `width` by `width + 1`; `numTicks = (height - GADGET_SIZE) /
//!   (max - min)`; a value `v` puts the thumb at `y = (int)((max - v) * numTicks)`. A click on the track jumps the
//!   thumb a fifth of the height toward the mouse. Up and down keys move by 2.
//! - **Horizontal**: `minVal` at the left. The thumb is 13 by 16 at `y = 10`; `numTicks = (width - 13) / (max -
//!   min)`; a value puts the thumb at `x = (int)((v - min) * numTicks)`. Its thumb's hilite follows the slider's.
//!
//! # Upstream quirks that are kept
//!
//! - The track click's clamp compares the **relative** click position with the slider's **screen** `y + size.y`
//!   (the horizontal one compares with `x + size.x` and then clamps to `x + size.y`).
//! - `GSM_SET_MIN_MAX` on a vertical slider puts the position at `minVal` but the thumb at the **top** (`(max - min) *
//!   numTicks` from the top is the bottom only when the thumb's travel is the track; it is `height - 16`).
//! - The horizontal slider's left/right keys are swapped: right **decreases** the position, left increases it; up and
//!   down are the tab keys.
//! - `GBM_SELECTED` is handled by the vertical slider only (`GSM_SLIDER_DONE`); the horizontal one ignores it.
//! - `numTicks` is a float divided by `maxVal - minVal`; `gogoGadgetSlider` makes `maxVal = minVal + 1` when they are
//!   equal, `GSM_SET_MIN_MAX` does not, and the list box's `adjustDisplay` divides by `maxVal` which is often 0.
//!
//! # Upstream undefined behaviour, and what the translation does
//!
//! - **A float to `Int` conversion out of range** (`(Int)(x * numTicks)` with `numTicks` infinite or NaN, after a
//!   division by zero): C++ leaves it undefined; x86 (`cvttss2si`, and the shipped build's x87 `fistp`) gives
//!   `0x80000000`. The translation gives that value ([`real_to_int_x86`]): the list box hits it whenever its list fits
//!   its display (`maxVal` 0), so it is not skipped.
//! - **No thumb** (`winGetChild()` NULL: a slider whose only child was destroyed) where the code uses it: the manager's
//!   [`GameWindowManager::undefined`] is set and the message stops.
//!
//! # Representation
//!
//! [`SliderRt`] in the window's [`UserData::Slider`]. Floats are `f32` with IEEE arithmetic only (`+ - * /`, no
//! `std` functions), as `Real`.

use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, GuiHost, MsgHandled, SystemFunc, UserData, WindowMsgData, WIN_STATE_HILITED};
use crate::client::window_script::{gws, win_status, SliderData, WinId, WinInstanceData};

use MsgHandled::{Handled, Ignored};

/// `GADGET_SIZE`.
pub const GADGET_SIZE: i32 = 16;
/// `HORIZONTAL_SLIDER_THUMB_WIDTH`.
pub const HORIZONTAL_SLIDER_THUMB_WIDTH: i32 = 13;
/// `HORIZONTAL_SLIDER_THUMB_HEIGHT`.
pub const HORIZONTAL_SLIDER_THUMB_HEIGHT: i32 = 16;
/// `HORIZONTAL_SLIDER_THUMB_POSITION` (`HEIGHT * 2 / 3`).
pub const HORIZONTAL_SLIDER_THUMB_POSITION: i32 = HORIZONTAL_SLIDER_THUMB_HEIGHT * 2 / 3;

/// `SliderData` as the slider keeps it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SliderRt {
    pub min_val: i32,
    pub max_val: i32,
    /// `numTicks` (`Real`).
    pub num_ticks: f32,
    pub position: i32,
}

/// C's `(Int)f`: truncation toward zero; NaN or out of `Int`'s range gives x86's "integer indefinite" `0x80000000`
/// (see the module docs).
pub fn real_to_int_x86(f: f32) -> i32 {
    if f.is_nan() || f64::from(f) < -2_147_483_648.0 || f64::from(f) >= 2_147_483_648.0 {
        return i32::MIN;
    }
    f as i32
}

/// The slider's data (`NULL` if the window has none).
pub fn slider_rt(wm: &GameWindowManager, w: WinId) -> Option<SliderRt> {
    match wm.ext[w].user {
        UserData::Slider(s) => Some(s),
        _ => None,
    }
}

fn set_rt(wm: &mut GameWindowManager, w: WinId, s: SliderRt) {
    wm.ext[w].user = UserData::Slider(s);
}

fn style(wm: &GameWindowManager, w: WinId) -> u32 {
    wm.tree.windows[w].inst.style
}

fn set_hilite(wm: &mut GameWindowManager, w: WinId, on: bool) {
    if on {
        wm.tree.windows[w].inst.state |= WIN_STATE_HILITED;
    } else {
        wm.tree.windows[w].inst.state &= !WIN_STATE_HILITED;
    }
}

/// `winGetChild()`: the head of the child list.
fn child(wm: &GameWindowManager, w: WinId) -> Option<WinId> {
    wm.tree.windows[w].children.first().copied()
}

/// The slider's data and thumb, or upstream's undefined behaviour (a `NULL` dereference) recorded.
fn need(wm: &mut GameWindowManager, w: WinId, thumb: bool, what: &'static str) -> Option<(SliderRt, Option<WinId>)> {
    let Some(s) = slider_rt(wm, w) else {
        wm.set_undefined(what);
        return None;
    };
    let c = child(wm, w);
    if thumb && c.is_none() {
        wm.set_undefined(what);
        return None;
    }
    Some((s, c))
}

/// `GWM_MOUSE_ENTERING` / `LEAVING` of both sliders: the hilite and the owner told, with mouse tracking.
fn track<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, on: bool) {
    if style(wm, w) & gws::MOUSE_TRACK != 0 {
        set_hilite(wm, w, on);
        let owner = wm.ext[w].owner;
        let m = if on { ggm::GBM_MOUSE_ENTERING } else { ggm::GBM_MOUSE_LEAVING };
        wm.win_send_system_msg(h, owner, m, win_data(Some(w)), 0);
    }
}

/// The `GWM_INPUT_FOCUS` of both sliders: the hilite, `GGM_FOCUS_CHANGE` to the owner, wants it.
fn focus<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, d1: WindowMsgData) {
    set_hilite(wm, w, d1 != 0);
    let owner = wm.ext[w].owner;
    let id = wm.tree.windows[w].id;
    wm.win_send_system_msg(h, owner, ggm::FOCUS_CHANGE, d1, id as u32);
    wm.focus_reply = true;
}

/// `GadgetVerticalSliderInput`.
pub fn vertical_slider_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::MOUSE_ENTERING => track(wm, h, w, true),
        gwm::MOUSE_LEAVING => track(wm, h, w, false),
        gwm::LEFT_DRAG => {
            if style(wm, w) & gws::MOUSE_TRACK != 0 {
                let owner = wm.ext[w].owner;
                wm.win_send_system_msg(h, owner, ggm::LEFT_DRAG, win_data(Some(w)), d1);
            }
        }
        gwm::LEFT_DOWN => {}
        gwm::LEFT_UP => {
            let Some((_, Some(c))) = need(wm, w, true, "GadgetVerticalSliderInput: no thumb (winGetChild NULL)") else { return Handled };
            let mousey = (d1 >> 16) as i32;
            let (_x, y) = wm.tree.screen_position(w);
            let size = wm.tree.windows[w].size;
            let csize = wm.tree.windows[c].size;
            let creg = wm.tree.windows[c].region;
            let center_y = creg.1.wrapping_add(csize.1 / 2);
            let page = size.1 / 5;
            let mut click = mousey.wrapping_sub(y);
            if click >= center_y {
                click = center_y.wrapping_add(page);
                if click > mousey.wrapping_sub(y) {
                    click = mousey.wrapping_sub(y);
                }
            } else {
                click = center_y.wrapping_sub(page);
                if click < mousey.wrapping_sub(y) {
                    click = mousey.wrapping_sub(y);
                }
            }
            // upstream compares the relative position with the screen bottom
            if click > y.wrapping_add(size.1).wrapping_sub(csize.1 / 2) {
                click = y.wrapping_add(size.1).wrapping_sub(csize.1 / 2);
            }
            if click < csize.1 / 2 {
                click = csize.1 / 2;
            }
            wm.win_set_position(c, 0, click.wrapping_sub(csize.1 / 2));
            wm.win_send_system_msg(h, Some(w), ggm::LEFT_DRAG, 0, d1);
        }
        gwm::CHAR => match d1 as u8 {
            key::UP => {
                if d2 & key::STATE_DOWN != 0 {
                    let Some((mut s, _)) = need(wm, w, false, "GadgetVerticalSliderInput: no SliderData") else { return Handled };
                    if s.position < s.max_val.wrapping_sub(1) {
                        let Some(c) = child(wm, w) else {
                            wm.set_undefined("GadgetVerticalSliderInput: no thumb (winGetChild NULL)");
                            return Handled;
                        };
                        s.position = s.position.wrapping_add(2);
                        set_rt(wm, w, s);
                        let owner = wm.ext[w].owner;
                        wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_TRACK, win_data(Some(w)), s.position as u32);
                        let s = slider_rt(wm, w).unwrap_or(s);
                        wm.win_set_position(c, 0, real_to_int_x86(s.max_val.wrapping_sub(s.position) as f32 * s.num_ticks));
                    }
                }
            }
            key::DOWN => {
                if d2 & key::STATE_DOWN != 0 {
                    let Some((mut s, _)) = need(wm, w, false, "GadgetVerticalSliderInput: no SliderData") else { return Handled };
                    if s.position > s.min_val.wrapping_add(1) {
                        let Some(c) = child(wm, w) else {
                            wm.set_undefined("GadgetVerticalSliderInput: no thumb (winGetChild NULL)");
                            return Handled;
                        };
                        s.position = s.position.wrapping_sub(2);
                        set_rt(wm, w, s);
                        let owner = wm.ext[w].owner;
                        wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_TRACK, win_data(Some(w)), s.position as u32);
                        let s = slider_rt(wm, w).unwrap_or(s);
                        wm.win_set_position(c, 0, real_to_int_x86(s.max_val.wrapping_sub(s.position) as f32 * s.num_ticks));
                    }
                }
            }
            // `window->winNextTab()` / `winPrevTab()`: empty upstream
            key::RIGHT | key::TAB | key::LEFT => {}
            _ => return Ignored,
        },
        _ => return Ignored,
    }
    Handled
}

/// `GadgetVerticalSliderSystem`.
pub fn vertical_slider_system<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::GBM_SELECTED => {
            let Some((s, _)) = need(wm, w, false, "GadgetVerticalSliderSystem: no SliderData") else { return Handled };
            let owner = wm.ext[w].owner;
            wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_DONE, win_data(Some(w)), s.position as u32);
        }
        ggm::LEFT_DRAG => {
            let Some((s, Some(c))) = need(wm, w, true, "GadgetVerticalSliderSystem: no thumb (winGetChild NULL)") else { return Handled };
            let mousey = (d2 >> 16) as i32;
            let (_x, y) = wm.tree.screen_position(w);
            let size = wm.tree.windows[w].size;
            let csize = wm.tree.windows[c].size;
            let center_y = wm.tree.screen_position(c).1.wrapping_add(csize.1 / 2);
            let owner = wm.ext[w].owner;
            if mousey > y.wrapping_add(size.1) {
                wm.win_send_system_msg(h, Some(w), ggm::GSM_SET_SLIDER, s.min_val as u32, 0);
                let p = slider_rt(wm, w).map_or(s.position, |s| s.position);
                wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_TRACK, win_data(Some(w)), p as u32);
                return Handled;
            } else if mousey < y {
                wm.win_send_system_msg(h, Some(w), ggm::GSM_SET_SLIDER, s.max_val as u32, 0);
                let p = slider_rt(wm, w).map_or(s.position, |s| s.position);
                wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_TRACK, win_data(Some(w)), p as u32);
                return Handled;
            }
            let mut s = s;
            if center_y <= y.wrapping_add(csize.1 / 2) {
                wm.win_set_position(c, 0, 0);
                s.position = s.max_val;
            } else if center_y >= y.wrapping_add(size.1).wrapping_sub(csize.1 / 2) {
                wm.win_set_position(c, 0, size.1.wrapping_sub(csize.1));
                s.position = s.min_val;
            } else {
                let delta = center_y.wrapping_sub(y).wrapping_sub(csize.1 / 2);
                s.position = real_to_int_x86(delta as f32 / s.num_ticks);
                if s.position > s.max_val {
                    s.position = s.max_val;
                }
                s.position = s.max_val.wrapping_sub(s.position);
            }
            set_rt(wm, w, s);
            wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_TRACK, win_data(Some(w)), s.position as u32);
        }
        ggm::GSM_SET_SLIDER => {
            let Some((mut s, c)) = need(wm, w, false, "GadgetVerticalSliderSystem: no SliderData") else { return Handled };
            let new_pos = d1 as i32;
            if new_pos < s.min_val || new_pos > s.max_val {
                return Handled;
            }
            let Some(c) = c else {
                wm.set_undefined("GadgetVerticalSliderSystem: GSM_SET_SLIDER without a thumb");
                return Handled;
            };
            s.position = new_pos;
            set_rt(wm, w, s);
            // (plant 4703, differential builds only: measured from the minimum)
            let from = if crate::logic::modules::plant(4703) { new_pos.wrapping_sub(s.min_val) } else { s.max_val.wrapping_sub(new_pos) };
            wm.win_set_position(c, 0, real_to_int_x86(from as f32 * s.num_ticks));
        }
        ggm::GSM_SET_MIN_MAX => {
            let Some((mut s, c)) = need(wm, w, false, "GadgetVerticalSliderSystem: no SliderData") else { return Handled };
            let size = wm.tree.windows[w].size;
            s.min_val = d1 as i32;
            s.max_val = d2 as i32;
            s.num_ticks = size.1.wrapping_sub(GADGET_SIZE) as f32 / s.max_val.wrapping_sub(s.min_val) as f32;
            s.position = s.min_val;
            set_rt(wm, w, s);
            let Some(c) = c else {
                wm.set_undefined("GadgetVerticalSliderSystem: GSM_SET_MIN_MAX without a thumb");
                return Handled;
            };
            wm.win_set_position(c, 0, real_to_int_x86(s.max_val.wrapping_sub(s.min_val) as f32 * s.num_ticks));
        }
        gwm::CREATE => {}
        gwm::DESTROY => wm.ext[w].user = UserData::None,
        gwm::INPUT_FOCUS => focus(wm, h, w, d1),
        ggm::RESIZED => {
            if let Some(t) = child(wm, w) {
                wm.win_set_size(h, t, d1 as i32, GADGET_SIZE);
            }
        }
        _ => return Ignored,
    }
    Handled
}

/// The thumb's hilite on the horizontal slider's enter and leave (when the first child is a push button).
fn hilite_thumb(wm: &mut GameWindowManager, w: WinId, on: bool) {
    if let Some(c) = child(wm, w) {
        if style(wm, c) & gws::PUSH_BUTTON != 0 {
            set_hilite(wm, c, on);
        }
    }
}

/// `GadgetHorizontalSliderInput`.
pub fn horizontal_slider_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::MOUSE_ENTERING => {
            track(wm, h, w, true);
            hilite_thumb(wm, w, true);
        }
        gwm::MOUSE_LEAVING => {
            track(wm, h, w, false);
            hilite_thumb(wm, w, false);
        }
        gwm::LEFT_DRAG => {
            if style(wm, w) & gws::MOUSE_TRACK != 0 {
                let owner = wm.ext[w].owner;
                wm.win_send_system_msg(h, owner, ggm::LEFT_DRAG, win_data(Some(w)), d1);
            }
        }
        gwm::LEFT_DOWN => {}
        gwm::LEFT_UP => {
            let Some((_, Some(c))) = need(wm, w, true, "GadgetHorizontalSliderInput: no thumb (winGetChild NULL)") else { return Handled };
            let mousex = (d1 & 0xFFFF) as i32;
            let (x, _y) = wm.tree.screen_position(w);
            let size = wm.tree.windows[w].size;
            let csize = wm.tree.windows[c].size;
            let creg = wm.tree.windows[c].region;
            let center_x = creg.0.wrapping_add(csize.0 / 2);
            let page = size.0 / 5;
            let mut click = mousex.wrapping_sub(x);
            if click >= center_x {
                click = center_x.wrapping_add(page);
                if click > mousex.wrapping_sub(x) {
                    click = mousex.wrapping_sub(x);
                }
            } else {
                click = center_x.wrapping_sub(page);
                if click < mousex.wrapping_sub(x) {
                    click = mousex.wrapping_sub(x);
                }
            }
            // upstream compares with `x + size.x` and clamps to `x + size.y`
            if click > x.wrapping_add(size.0).wrapping_sub(csize.0 / 2) {
                click = x.wrapping_add(size.1).wrapping_sub(csize.0 / 2);
            }
            if click < csize.0 / 2 {
                click = csize.0 / 2;
            }
            wm.win_set_position(c, click.wrapping_sub(csize.0 / 2), HORIZONTAL_SLIDER_THUMB_POSITION);
            wm.win_send_system_msg(h, Some(w), ggm::LEFT_DRAG, 0, d1);
        }
        gwm::CHAR => match d1 as u8 {
            k @ (key::RIGHT | key::LEFT) => {
                if d2 & key::STATE_DOWN != 0 {
                    let Some((mut s, _)) = need(wm, w, false, "GadgetHorizontalSliderInput: no SliderData") else { return Handled };
                    let go = if k == key::RIGHT { s.position > s.min_val.wrapping_add(1) } else { s.position < s.max_val.wrapping_sub(1) };
                    if go {
                        let Some(c) = child(wm, w) else {
                            wm.set_undefined("GadgetHorizontalSliderInput: no thumb (winGetChild NULL)");
                            return Handled;
                        };
                        s.position = if k == key::RIGHT { s.position.wrapping_sub(2) } else { s.position.wrapping_add(2) };
                        set_rt(wm, w, s);
                        let owner = wm.ext[w].owner;
                        wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_TRACK, win_data(Some(w)), s.position as u32);
                        let s = slider_rt(wm, w).unwrap_or(s);
                        wm.win_set_position(c, real_to_int_x86(s.position.wrapping_sub(s.min_val) as f32 * s.num_ticks), HORIZONTAL_SLIDER_THUMB_POSITION);
                    }
                }
            }
            // `window->winNextTab()` / `winPrevTab()`: empty upstream
            key::DOWN | key::TAB | key::UP => {}
            _ => return Ignored,
        },
        _ => return Ignored,
    }
    Handled
}

/// `GadgetHorizontalSliderSystem`.
pub fn horizontal_slider_system<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::LEFT_DRAG => {
            let Some((s, Some(c))) = need(wm, w, true, "GadgetHorizontalSliderSystem: no thumb (winGetChild NULL)") else { return Handled };
            let mousex = (d2 & 0xFFFF) as i32;
            let (x, _y) = wm.tree.screen_position(w);
            let size = wm.tree.windows[w].size;
            let csize = wm.tree.windows[c].size;
            let crel = wm.tree.windows[c].region;
            let center_x = wm.tree.screen_position(c).0.wrapping_add(csize.0 / 2);
            let owner = wm.ext[w].owner;
            if mousex > x.wrapping_add(size.0).wrapping_sub(HORIZONTAL_SLIDER_THUMB_WIDTH / 2) {
                wm.win_send_system_msg(h, Some(w), ggm::GSM_SET_SLIDER, s.max_val as u32, 0);
                let p = slider_rt(wm, w).map_or(s.position, |s| s.position);
                wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_TRACK, win_data(Some(w)), p as u32);
                return Handled;
            } else if mousex < x.wrapping_add(HORIZONTAL_SLIDER_THUMB_WIDTH / 2) {
                wm.win_send_system_msg(h, Some(w), ggm::GSM_SET_SLIDER, s.min_val as u32, 0);
                let p = slider_rt(wm, w).map_or(s.position, |s| s.position);
                wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_TRACK, win_data(Some(w)), p as u32);
                return Handled;
            }
            let mut s = s;
            if center_x < x.wrapping_add(csize.0 / 2) {
                wm.win_set_position(c, 0, HORIZONTAL_SLIDER_THUMB_POSITION);
                s.position = s.min_val;
            } else if center_x >= x.wrapping_add(size.0).wrapping_sub(csize.0 / 2) {
                let px = real_to_int_x86(s.max_val.wrapping_sub(s.min_val) as f32 * s.num_ticks).wrapping_sub(HORIZONTAL_SLIDER_THUMB_WIDTH / 2);
                wm.win_set_position(c, px, HORIZONTAL_SLIDER_THUMB_POSITION);
                s.position = s.max_val;
            } else {
                let delta = center_x.wrapping_sub(x).wrapping_sub(HORIZONTAL_SLIDER_THUMB_WIDTH / 2);
                s.position = real_to_int_x86(delta as f32 / s.num_ticks).wrapping_add(s.min_val);
                if s.position > s.max_val {
                    s.position = s.max_val;
                }
                if s.position < s.min_val {
                    s.position = s.min_val;
                }
                wm.win_set_position(c, crel.0, HORIZONTAL_SLIDER_THUMB_POSITION);
            }
            set_rt(wm, w, s);
            wm.win_send_system_msg(h, owner, ggm::GSM_SLIDER_TRACK, win_data(Some(w)), s.position as u32);
        }
        ggm::GSM_SET_SLIDER => {
            let Some((mut s, c)) = need(wm, w, false, "GadgetHorizontalSliderSystem: no SliderData") else { return Handled };
            let new_pos = d1 as i32;
            if new_pos < s.min_val || new_pos > s.max_val {
                return Handled;
            }
            let Some(c) = c else {
                wm.set_undefined("GadgetHorizontalSliderSystem: GSM_SET_SLIDER without a thumb");
                return Handled;
            };
            s.position = new_pos;
            set_rt(wm, w, s);
            wm.win_set_position(c, real_to_int_x86(new_pos.wrapping_sub(s.min_val) as f32 * s.num_ticks), HORIZONTAL_SLIDER_THUMB_POSITION);
        }
        ggm::GSM_SET_MIN_MAX => {
            let Some((mut s, c)) = need(wm, w, false, "GadgetHorizontalSliderSystem: no SliderData") else { return Handled };
            let size = wm.tree.windows[w].size;
            s.min_val = d1 as i32;
            s.max_val = d2 as i32;
            s.num_ticks = size.0.wrapping_sub(HORIZONTAL_SLIDER_THUMB_WIDTH) as f32 / s.max_val.wrapping_sub(s.min_val) as f32;
            s.position = s.min_val;
            set_rt(wm, w, s);
            let Some(c) = c else {
                wm.set_undefined("GadgetHorizontalSliderSystem: GSM_SET_MIN_MAX without a thumb");
                return Handled;
            };
            wm.win_set_position(c, 0, HORIZONTAL_SLIDER_THUMB_POSITION);
        }
        gwm::CREATE => {}
        gwm::DESTROY => wm.ext[w].user = UserData::None,
        gwm::INPUT_FOCUS => focus(wm, h, w, d1),
        ggm::RESIZED => {
            if let Some(t) = child(wm, w) {
                wm.win_set_size(h, t, GADGET_SIZE, d2 as i32);
            }
        }
        _ => return Ignored,
    }
    Handled
}

/// `gogoGadgetSlider(parent, status, x, y, width, height, instData, sliderData, NULL, ...)`: the slider and its thumb.
/// `data` is the caller's template, which upstream updates in place (`maxVal`, `numTicks`). `None` where upstream
/// asserts (neither slider style).
#[allow(clippy::too_many_arguments)]
pub fn gogo_gadget_slider<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, parent: Option<WinId>, status: u32, x: i32, y: i32, width: i32, height: i32, inst: WinInstanceData, data: &mut SliderRt, default_visual: bool) -> Option<WinId> {
    let status = status | win_status::TAB_STOP;
    let horz = inst.style & gws::HORZ_SLIDER != 0;
    if !horz && inst.style & gws::VERT_SLIDER == 0 {
        return None;
    }
    let style = inst.style;
    let font = inst.font.clone();
    let system = if horz { SystemFunc::HorizontalSlider } else { SystemFunc::VerticalSlider };
    let s = wm.win_create(h, parent, status, x, y, width, height, system, Some(inst));
    slider_after_create(wm, h, s, parent, status, style, font, data, default_visual);
    Some(s)
}

/// `gogoGadgetSlider` after its `winCreate` (`status` already with `TAB_STOP`): the callbacks, owner, thumb (made with
/// the default visual), data and look.
#[allow(clippy::too_many_arguments)]
pub fn slider_after_create<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, s: WinId, parent: Option<WinId>, status: u32, style: u32, font: Option<crate::client::window_script::GameFont>, data: &mut SliderRt, default_visual: bool) {
    let horz = style & gws::HORZ_SLIDER != 0;
    let (width, height) = wm.tree.windows[s].size;
    wm.install_slider_callbacks(s, parent);
    wm.win_set_owner(s, parent);
    let mut bi = WinInstanceData { style: gws::PUSH_BUTTON, ..WinInstanceData::new() };
    if style & gws::MOUSE_TRACK != 0 {
        bi.style |= gws::MOUSE_TRACK;
    }
    let flags = (status | win_status::ENABLED | win_status::DRAGABLE) & !win_status::HIDDEN;
    if horz {
        wm.gogo_push_button(h, Some(s), flags, 0, HORIZONTAL_SLIDER_THUMB_POSITION, HORIZONTAL_SLIDER_THUMB_WIDTH, HORIZONTAL_SLIDER_THUMB_HEIGHT, bi, true);
    } else {
        wm.gogo_push_button(h, Some(s), flags, 0, 0, width, width.wrapping_add(1), bi, true);
    }
    if data.max_val == data.min_val {
        data.max_val = data.min_val.wrapping_add(1);
    }
    data.num_ticks = if horz { width.wrapping_sub(HORIZONTAL_SLIDER_THUMB_WIDTH) as f32 / data.max_val.wrapping_sub(data.min_val) as f32 } else { height.wrapping_sub(GADGET_SIZE) as f32 / data.max_val.wrapping_sub(data.min_val) as f32 };
    wm.ext[s].user = UserData::Slider(*data);
    crate::client::gadget_look::assign_default_gadget_look(wm, h, s, font, default_visual);
}

/// The script's `SliderData` (min and max; the rest zero, as `getDataTemplate` clears it).
pub fn slider_template(d: &SliderData) -> SliderRt {
    SliderRt { min_val: d.min_val, max_val: d.max_val, num_ticks: 0.0, position: 0 }
}

/// `GadgetSliderSetPosition(win, pos)` (`GSM_SET_SLIDER`).
pub fn slider_set_position<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, pos: i32) {
    wm.win_send_system_msg(h, Some(w), ggm::GSM_SET_SLIDER, pos as u32, 0);
}

/// `GadgetSliderGetPosition(win)`: -1 without data.
pub fn slider_get_position(wm: &GameWindowManager, w: WinId) -> i32 {
    slider_rt(wm, w).map_or(-1, |s| s.position)
}

/// `GadgetSliderSetMinMax(win, min, max)` (`GSM_SET_MIN_MAX`).
pub fn slider_set_min_max<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, min: i32, max: i32) {
    wm.win_send_system_msg(h, Some(w), ggm::GSM_SET_MIN_MAX, min as u32, max as u32);
}

/// The slider a `GSM_*` message names in its `mData1`.
pub fn msg_slider(d1: WindowMsgData) -> Option<WinId> {
    data_win(d1)
}
