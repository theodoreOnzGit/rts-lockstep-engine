// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetPushButton.cpp (buttonTriggersOnMouseDown,
//     GadgetPushButtonInput, GadgetPushButtonSystem, GadgetCheckLikeButtonSetVisualCheck,
//     GadgetCheckLikeButtonIsChecked, GadgetButtonEnableCheckLike, GadgetButtonSetText, getNewPushButtonData,
//     GadgetButtonSetBorder, GadgetButtonDrawClock, GadgetButtonDrawInverseClock, GadgetButtonDrawOverlayImage,
//     GadgetButtonSetAltSound)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetCheckBox.cpp (GadgetCheckBoxInput,
//     GadgetCheckBoxSystem, GadgetCheckBoxSetText, GadgetCheckBoxSetChecked, GadgetCheckBoxToggle,
//     GadgetCheckBoxIsChecked)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetRadioButton.cpp (doRadioUnselect,
//     unselectOtherRadioOfGroup, GadgetRadioButtonInput, GadgetRadioButtonSystem, GadgetRadioSetText,
//     GadgetRadioSetGroup, GadgetRadioSetSelection)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetStaticText.cpp (GadgetStaticTextInput,
//     GadgetStaticTextSystem, GadgetStaticTextSetText, GadgetStaticTextGetText)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Gadget/GadgetProgressBar.cpp (GadgetProgressBarSystem,
//     GadgetProgressBarSetProgress)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The gadgets' behaviour (GitHub #34): the input and system callbacks of the push button, check box, radio button,
//! static text and progress bar, run by [`crate::client::window_manager`].
//!
//! # As upstream reads it
//!
//! A gadget tells its **owner** (the parent it was created in) what happened, through the owner's system callback:
//! `GBM_SELECTED` with the gadget as `mData1`. The states are bits of `WinInstanceData::m_state`: `WIN_STATE_HILITED`
//! (the mouse is over it, with `GWS_MOUSE_TRACK`, or it has the focus) and `WIN_STATE_SELECTED` (pushed, checked).
//!
//! - **Push button.** A left press selects it (a check-like button toggles) and plays `GUIClick` (or its alternative
//!   sound); a release while selected sends `GBM_SELECTED` and clears it, unless the button fires on the press
//!   (`WIN_STATUS_ON_MOUSE_DOWN`, and every check-like button), which sent it at the press. Leaving the button clears a
//!   plain button's selection, so a press dragged off does nothing. Right clicks only with `WIN_STATUS_RIGHT_CLICK`
//!   (`GBM_SELECTED_RIGHT`). Enter or space: down selects, up sends `GBM_SELECTED`. The focus hilites it.
//! - **Check box.** The release toggles `SELECTED` and sends `GBM_SELECTED`, but only while hilited; enter or space
//!   on the press.
//! - **Radio button.** The release of an unselected one sends `GBM_SELECTED`, unselects every other radio button of
//!   its group and screen anywhere in the window system, and selects it.
//! - **Static text** answers only tab and arrows, and keeps its text in its own `TextData`.
//! - **Progress bar** keeps its position (0..100) in its user data.
//!
//! # Upstream quirks that are kept
//!
//! - The check box's `GWM_LEFT_UP` toggles only when it is hilited, which needs `GWS_MOUSE_TRACK` (or the focus):
//!   a check box without mouse tracking ignores the mouse.
//! - The check box's `GWM_RIGHT_UP` sends `GBM_SELECTED_RIGHT` and **unchecks** a checked box.
//! - `GadgetRadioSetSelection(g, sendMsg)` passes `&sendMsg`, a pointer, and the system tests `(Bool)mData1 == TRUE`:
//!   never true, so it never sends `GBM_SELECTED` (modelled as `mData1 = 2`).
//! - The radio button and static text call `GameWindow::winNextTab`, which is empty.
//! - The radio button's `GWM_INPUT_FOCUS` only clears the hilite and always says it wants the focus.
//! - The push button's `GWM_MOUSE_ENTERING` / `LEAVING` hilites a horizontal slider parent (the slider's thumb).
//!
//! # Where this differs from upstream
//!
//! Sounds go to [`GuiHost::add_audio_event`]. `GBM_SET_SELECTION`'s and `GGM_SET_LABEL`'s pointed-to values are the
//! manager's [`GameWindowManager::msg_text`]; the focus answer is [`GameWindowManager::focus_reply`].
//!
//! # Representation
//!
//! Free functions on the manager; the user data is [`UserData`].

use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, ClockKind, GameWindowManager, GuiHost, MsgHandled, PushButtonData, UserData, WindowMsgData, WIN_STATE_HILITED, WIN_STATE_SELECTED};
use crate::client::window_script::{gws, win_status, RadioButtonData, WinId};

use MsgHandled::{Handled, Ignored};

fn state(wm: &GameWindowManager, w: WinId) -> i32 {
    wm.tree.windows[w].inst.state
}
fn set_state(wm: &mut GameWindowManager, w: WinId, bits: i32, on: bool) {
    if on {
        wm.tree.windows[w].inst.state |= bits;
    } else {
        wm.tree.windows[w].inst.state &= !bits;
    }
}
fn status(wm: &GameWindowManager, w: WinId) -> u32 {
    wm.tree.windows[w].status
}
fn style(wm: &GameWindowManager, w: WinId) -> u32 {
    wm.tree.windows[w].inst.style
}

/// `buttonTriggersOnMouseDown`: `WIN_STATUS_ON_MOUSE_DOWN`, or check-like.
fn button_triggers_on_mouse_down(wm: &GameWindowManager, w: WinId) -> bool {
    status(wm, w) & (win_status::ON_MOUSE_DOWN | win_status::CHECK_LIKE) != 0
}

/// The push button's click sound (`altSound`, else `GUIClick`).
fn click_sound(wm: &GameWindowManager, w: WinId) -> Vec<u8> {
    match &wm.ext[w].user {
        UserData::PushButton(Some(p)) if !p.alt_sound.is_empty() => p.alt_sound.clone(),
        _ => b"GUIClick".to_vec(),
    }
}

/// The hilite of a horizontal slider parent (the push button is its thumb).
fn hilite_slider_parent(wm: &mut GameWindowManager, w: WinId, on: bool) {
    if let Some(p) = wm.tree.windows[w].parent {
        if style(wm, p) & gws::HORZ_SLIDER != 0 {
            set_state(wm, p, WIN_STATE_HILITED, on);
        }
    }
}

/// `GadgetPushButtonInput`.
pub fn push_button_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    let me = win_data(Some(w));
    let owner = wm.ext[w].owner;
    let check_like = status(wm, w) & win_status::CHECK_LIKE != 0;
    match msg {
        gwm::MOUSE_ENTERING => {
            if style(wm, w) & gws::MOUSE_TRACK != 0 {
                set_state(wm, w, WIN_STATE_HILITED, true);
                wm.win_send_system_msg(h, owner, ggm::GBM_MOUSE_ENTERING, me, d1);
            }
            hilite_slider_parent(wm, w, true);
        }
        gwm::MOUSE_LEAVING => {
            if style(wm, w) & gws::MOUSE_TRACK != 0 {
                set_state(wm, w, WIN_STATE_HILITED, false);
                wm.win_send_system_msg(h, owner, ggm::GBM_MOUSE_LEAVING, me, d1);
            }
            if !check_like && state(wm, w) & WIN_STATE_SELECTED != 0 {
                set_state(wm, w, WIN_STATE_SELECTED, false);
            }
            hilite_slider_parent(wm, w, false);
        }
        gwm::LEFT_DRAG => {
            wm.win_send_system_msg(h, owner, ggm::LEFT_DRAG, me, d1);
        }
        gwm::LEFT_DOWN => {
            let s = click_sound(wm, w);
            h.add_audio_event(&s);
            if check_like {
                let sel = state(wm, w) & WIN_STATE_SELECTED != 0;
                set_state(wm, w, WIN_STATE_SELECTED, !sel);
            } else {
                set_state(wm, w, WIN_STATE_SELECTED, true);
            }
            if button_triggers_on_mouse_down(wm, w) {
                wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED, me, d1);
            }
        }
        gwm::LEFT_UP => {
            if state(wm, w) & WIN_STATE_SELECTED != 0 && !check_like {
                if !button_triggers_on_mouse_down(wm, w) {
                    wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED, me, d1);
                }
                set_state(wm, w, WIN_STATE_SELECTED, false);
            } else {
                return Ignored;
            }
        }
        gwm::RIGHT_DOWN => {
            if wm.tree.windows[w].inst.status & win_status::RIGHT_CLICK != 0 {
                let s = click_sound(wm, w);
                h.add_audio_event(&s);
                if check_like {
                    let sel = state(wm, w) & WIN_STATE_SELECTED != 0;
                    set_state(wm, w, WIN_STATE_SELECTED, !sel);
                    wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED_RIGHT, me, d1);
                } else {
                    set_state(wm, w, WIN_STATE_SELECTED, true);
                }
            } else {
                return Ignored;
            }
        }
        gwm::RIGHT_UP => {
            if wm.tree.windows[w].inst.status & win_status::RIGHT_CLICK != 0 {
                if state(wm, w) & WIN_STATE_SELECTED != 0 && !check_like {
                    wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED_RIGHT, me, d1);
                    set_state(wm, w, WIN_STATE_SELECTED, false);
                } else {
                    return Ignored;
                }
            } else {
                return Ignored;
            }
        }
        gwm::CHAR => match d1 as u8 {
            key::ENTER | key::SPACE => {
                if d2 & key::STATE_UP != 0 {
                    if state(wm, w) & WIN_STATE_SELECTED != 0 && !check_like {
                        wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED, me, 0);
                        set_state(wm, w, WIN_STATE_SELECTED, false);
                    }
                } else if check_like {
                    let sel = state(wm, w) & WIN_STATE_SELECTED != 0;
                    set_state(wm, w, WIN_STATE_SELECTED, !sel);
                    wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED, me, d1);
                } else {
                    set_state(wm, w, WIN_STATE_SELECTED, true);
                }
            }
            key::DOWN | key::RIGHT | key::TAB => {
                if d2 & key::STATE_DOWN != 0 {
                    wm.win_next_tab(h, w);
                }
            }
            key::UP | key::LEFT => {
                if d2 & key::STATE_DOWN != 0 {
                    wm.win_prev_tab(h, w);
                }
            }
            _ => return Ignored,
        },
        _ => return Ignored,
    }
    Handled
}

/// The `GWM_INPUT_FOCUS` of the push button and check box: hilite, `GGM_FOCUS_CHANGE` to the owner, the answer.
fn focus_hilite<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, d1: WindowMsgData) {
    set_state(wm, w, WIN_STATE_HILITED, d1 != 0);
    let owner = wm.ext[w].owner;
    let id = wm.tree.windows[w].id;
    wm.win_send_system_msg(h, owner, ggm::FOCUS_CHANGE, d1, id as u32);
    wm.focus_reply = d1 != 0;
}

/// `GadgetPushButtonSystem`.
pub fn push_button_system<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::SET_LABEL => {
            let t = wm.msg_text.clone();
            wm.win_set_text(w, &t);
        }
        gwm::CREATE => {}
        gwm::DESTROY => wm.ext[w].user = UserData::PushButton(None),
        gwm::INPUT_FOCUS => focus_hilite(wm, h, w, d1),
        _ => return Ignored,
    }
    Handled
}

/// (#89) `GadgetButtonSetText(g, text)`: `GGM_SET_LABEL` with the text (nothing for no window).
pub fn button_set_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: Option<WinId>, text: &[u16]) {
    let Some(w) = w else { return };
    wm.msg_text = text.to_vec();
    wm.win_send_system_msg(h, Some(w), ggm::SET_LABEL, 1, 0);
}

/// `GadgetCheckLikeButtonSetVisualCheck`: only the look, no message; only for a check-like button.
pub fn check_like_button_set_visual_check(wm: &mut GameWindowManager, w: WinId, checked: bool) {
    if status(wm, w) & win_status::CHECK_LIKE == 0 {
        return;
    }
    set_state(wm, w, WIN_STATE_SELECTED, checked);
}

/// `GadgetCheckLikeButtonIsChecked`.
pub fn check_like_button_is_checked(wm: &GameWindowManager, w: WinId) -> bool {
    state(wm, w) & WIN_STATE_SELECTED != 0
}

/// `GadgetButtonEnableCheckLike`.
pub fn button_enable_check_like(wm: &mut GameWindowManager, w: WinId, check_like: bool, checked: bool) {
    if check_like {
        wm.win_set_status(w, win_status::CHECK_LIKE);
    } else {
        wm.win_clear_status(w, win_status::CHECK_LIKE);
    }
    set_state(wm, w, WIN_STATE_SELECTED, checked);
}

/// The push button data, made by the setters when there is none (`getNewPushButtonData`).
fn push_data(wm: &mut GameWindowManager, w: WinId) -> &mut PushButtonData {
    let u = &mut wm.ext[w].user;
    if !matches!(u, UserData::PushButton(Some(_))) {
        *u = UserData::PushButton(Some(PushButtonData::default()));
    }
    match u {
        UserData::PushButton(Some(p)) => p,
        _ => unreachable!("just set"),
    }
}

/// `GadgetButtonSetBorder(g, color, drawBorder)`.
pub fn button_set_border(wm: &mut GameWindowManager, w: WinId, color: i32, draw: bool) {
    let p = push_data(wm, w);
    p.draw_border = draw;
    p.color_border = color;
}

/// `GadgetButtonDrawClock` / `GadgetButtonDrawInverseClock`: drawn once, at the next draw.
pub fn button_draw_clock(wm: &mut GameWindowManager, w: WinId, percent: i32, color: i32, inverse: bool) {
    let p = push_data(wm, w);
    p.draw_clock = if inverse { ClockKind::Inverse } else { ClockKind::Normal };
    p.percent_clock = percent;
    p.color_clock = color;
}

/// `GadgetButtonDrawOverlayImage`.
pub fn button_draw_overlay_image(wm: &mut GameWindowManager, w: WinId, image: Option<Vec<u8>>) {
    push_data(wm, w).overlay_image = image;
}

/// `GadgetButtonSetAltSound`: only on a button that already has its data.
pub fn button_set_alt_sound(wm: &mut GameWindowManager, w: WinId, sound: &[u8]) {
    if let UserData::PushButton(Some(p)) = &mut wm.ext[w].user {
        p.alt_sound = sound.to_vec();
    }
}

/// `GadgetCheckBoxInput`.
pub fn check_box_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    let me = win_data(Some(w));
    let owner = wm.ext[w].owner;
    match msg {
        gwm::MOUSE_ENTERING => {
            if style(wm, w) & gws::MOUSE_TRACK != 0 {
                set_state(wm, w, WIN_STATE_HILITED, true);
                wm.win_send_system_msg(h, owner, ggm::GBM_MOUSE_ENTERING, me, d1);
            }
        }
        gwm::MOUSE_LEAVING => {
            if style(wm, w) & gws::MOUSE_TRACK != 0 {
                set_state(wm, w, WIN_STATE_HILITED, false);
                wm.win_send_system_msg(h, owner, ggm::GBM_MOUSE_LEAVING, me, d1);
            }
        }
        gwm::LEFT_DRAG => {
            wm.win_send_system_msg(h, owner, ggm::LEFT_DRAG, me, d1);
        }
        gwm::LEFT_DOWN | gwm::RIGHT_DOWN => {}
        gwm::LEFT_UP => {
            if state(wm, w) & WIN_STATE_HILITED == 0 {
                return Ignored;
            }
            wm.tree.windows[w].inst.state ^= WIN_STATE_SELECTED;
            wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED, me, d1);
        }
        gwm::RIGHT_UP => {
            if state(wm, w) & WIN_STATE_SELECTED != 0 {
                wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED_RIGHT, me, d1);
                set_state(wm, w, WIN_STATE_SELECTED, false);
            } else {
                return Ignored;
            }
        }
        gwm::CHAR => match d1 as u8 {
            key::ENTER | key::SPACE => {
                if d2 & key::STATE_DOWN != 0 {
                    wm.tree.windows[w].inst.state ^= WIN_STATE_SELECTED;
                    wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED, me, 0);
                }
            }
            key::DOWN | key::RIGHT | key::TAB => {
                if d2 & key::STATE_DOWN != 0 {
                    wm.win_next_tab(h, w);
                }
            }
            key::UP | key::LEFT => {
                if d2 & key::STATE_DOWN != 0 {
                    wm.win_prev_tab(h, w);
                }
            }
            _ => return Ignored,
        },
        _ => return Ignored,
    }
    Handled
}

/// `GadgetCheckBoxSystem`.
pub fn check_box_system<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::SET_LABEL => {
            let t = wm.msg_text.clone();
            wm.win_set_text(w, &t);
        }
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => focus_hilite(wm, h, w, d1),
        _ => return Ignored,
    }
    Handled
}

/// `GadgetCheckBoxSetChecked` (sends `GBM_SELECTED` to the owner).
pub fn check_box_set_checked<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, checked: bool) {
    set_state(wm, w, WIN_STATE_SELECTED, checked);
    let owner = wm.ext[w].owner;
    wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED, win_data(Some(w)), 0);
}

/// `GadgetCheckBoxToggle`.
pub fn check_box_toggle<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId) {
    let c = state(wm, w) & WIN_STATE_SELECTED != 0;
    check_box_set_checked(wm, h, w, !c);
}

/// `GadgetCheckBoxIsChecked`.
pub fn check_box_is_checked(wm: &GameWindowManager, w: WinId) -> bool {
    state(wm, w) & WIN_STATE_SELECTED != 0
}

/// `doRadioUnselect`.
fn do_radio_unselect(wm: &mut GameWindowManager, w: WinId, group: i32, screen: i32, except: WinId) {
    if w != except && style(wm, w) & gws::RADIO_BUTTON != 0 {
        if let UserData::Radio(r) = wm.ext[w].user {
            if r.group == group && r.screen == screen {
                set_state(wm, w, WIN_STATE_SELECTED, false);
            }
        }
    }
    for c in wm.tree.windows[w].children.clone() {
        do_radio_unselect(wm, c, group, screen, except);
    }
}

/// `unselectOtherRadioOfGroup`: over the whole window list.
fn unselect_other_radio_of_group(wm: &mut GameWindowManager, group: i32, screen: i32, except: WinId) {
    for w in wm.window_list.clone() {
        do_radio_unselect(wm, w, group, screen, except);
    }
}

fn radio_data(wm: &GameWindowManager, w: WinId) -> RadioButtonData {
    match wm.ext[w].user {
        UserData::Radio(r) => r,
        _ => RadioButtonData::default(),
    }
}

/// The radio button's selection: `GBM_SELECTED` (when `send`), the group unselected, this one selected.
fn radio_select<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, send: Option<WindowMsgData>) {
    if let Some(d) = send {
        let owner = wm.ext[w].owner;
        wm.win_send_system_msg(h, owner, ggm::GBM_SELECTED, win_data(Some(w)), d);
    }
    let r = radio_data(wm, w);
    if r.group != 0 {
        unselect_other_radio_of_group(wm, r.group, r.screen, w);
    }
    set_state(wm, w, WIN_STATE_SELECTED, true);
}

/// `GadgetRadioButtonInput`.
pub fn radio_button_input<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    let me = win_data(Some(w));
    let owner = wm.ext[w].owner;
    match msg {
        gwm::MOUSE_ENTERING => {
            if style(wm, w) & gws::MOUSE_TRACK != 0 {
                set_state(wm, w, WIN_STATE_HILITED, true);
                wm.win_send_system_msg(h, owner, ggm::GBM_MOUSE_ENTERING, me, d1);
            }
        }
        gwm::MOUSE_LEAVING => {
            if style(wm, w) & gws::MOUSE_TRACK != 0 {
                set_state(wm, w, WIN_STATE_HILITED, false);
                wm.win_send_system_msg(h, owner, ggm::GBM_MOUSE_LEAVING, me, d1);
            }
        }
        gwm::LEFT_DRAG => {
            wm.win_send_system_msg(h, owner, ggm::LEFT_DRAG, me, d1);
        }
        gwm::LEFT_DOWN => {}
        gwm::LEFT_UP => {
            if state(wm, w) & WIN_STATE_SELECTED == 0 {
                radio_select(wm, h, w, Some(d1));
            } else if state(wm, w) & WIN_STATE_HILITED == 0 {
                return Ignored;
            }
        }
        gwm::CHAR => match d1 as u8 {
            key::ENTER | key::SPACE => {
                if d2 & key::STATE_DOWN != 0 && state(wm, w) & WIN_STATE_SELECTED == 0 {
                    radio_select(wm, h, w, Some(d1));
                }
            }
            // `window->winNextTab()` / `winPrevTab()`: empty upstream
            key::DOWN | key::RIGHT | key::TAB | key::UP | key::LEFT => {}
            _ => return Ignored,
        },
        _ => return Ignored,
    }
    Handled
}

/// `GadgetRadioButtonSystem`. `GBM_SET_SELECTION`'s `mData1` is the `Bool` itself (as `GadgetRadioSetSelection`
/// should have passed it); see [`radio_set_selection`] for what upstream passes.
pub fn radio_button_system<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::GBM_SET_SELECTION => {
            if state(wm, w) & WIN_STATE_SELECTED == 0 {
                // `(Bool)mData1 == TRUE`
                radio_select(wm, h, w, (d1 == 1).then_some(0));
            }
        }
        ggm::SET_LABEL => {
            let t = wm.msg_text.clone();
            wm.win_set_text(w, &t);
        }
        gwm::CREATE => {}
        gwm::DESTROY => wm.ext[w].user = UserData::None,
        gwm::INPUT_FOCUS => {
            if d1 == 0 {
                set_state(wm, w, WIN_STATE_HILITED, false);
            }
            let owner = wm.ext[w].owner;
            let id = wm.tree.windows[w].id;
            wm.win_send_system_msg(h, owner, ggm::FOCUS_CHANGE, d1, id as u32);
            wm.focus_reply = true;
        }
        _ => return Ignored,
    }
    Handled
}

/// `GadgetRadioSetGroup(g, group, screen)`.
pub fn radio_set_group(wm: &mut GameWindowManager, w: WinId, group: i32, screen: i32) {
    wm.ext[w].user = UserData::Radio(RadioButtonData { screen, group });
}

/// `GadgetRadioSetSelection(g, sendMsg)`: upstream passes `&sendMsg`, a pointer, as `mData1`, which is never `TRUE`:
/// the selection never sends `GBM_SELECTED`, whatever `send` is.
pub fn radio_set_selection<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, send: bool) {
    let _ = send;
    wm.win_send_system_msg(h, Some(w), ggm::GBM_SET_SELECTION, 2, 0);
}

/// `GadgetStaticTextInput`: tab and arrows (into the empty `winNextTab`), nothing else.
pub fn static_text_input<H: GuiHost>(_wm: &mut GameWindowManager, _h: &mut H, _w: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CHAR => match d1 as u8 {
            key::DOWN | key::RIGHT | key::TAB | key::UP | key::LEFT => Handled,
            _ => Ignored,
        },
        _ => Ignored,
    }
}

/// `GadgetStaticTextSystem`.
pub fn static_text_system<H: GuiHost>(wm: &mut GameWindowManager, _h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::GET_LABEL => {
            if let UserData::StaticText(_, t) = &wm.ext[w].user {
                wm.msg_text = t.clone();
            }
        }
        ggm::SET_LABEL => {
            if d1 != 0 {
                let t = wm.msg_text.clone();
                if let UserData::StaticText(_, s) = &mut wm.ext[w].user {
                    *s = t;
                }
            }
        }
        gwm::CREATE => {}
        gwm::DESTROY => wm.ext[w].user = UserData::None,
        _ => return Ignored,
    }
    Handled
}

/// (#69) `GadgetStaticTextSetText(window, text)`: `GGM_SET_LABEL` with the text (nothing for no window).
pub fn static_text_set_text<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: Option<WinId>, text: &[u16]) {
    let Some(w) = w else { return };
    wm.msg_text = text.to_vec();
    wm.win_send_system_msg(h, Some(w), ggm::SET_LABEL, 1, 0);
}

/// `GadgetStaticTextGetText`.
pub fn static_text_get_text(wm: &GameWindowManager, w: WinId) -> Vec<u16> {
    match &wm.ext[w].user {
        UserData::StaticText(_, t) => t.clone(),
        _ => Vec::new(),
    }
}

/// `GadgetProgressBarSystem`: `GPM_SET_PROGRESS` keeps a position of 0..100.
pub fn progress_bar_system<H: GuiHost>(wm: &mut GameWindowManager, _h: &mut H, w: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::GPM_SET_PROGRESS => {
            let p = d1 as i32;
            if (0..=100).contains(&p) {
                wm.ext[w].user = UserData::Progress(p);
            }
        }
        _ => return Ignored,
    }
    Handled
}

/// `GadgetProgressBarSetProgress`.
pub fn progress_bar_set_progress<H: GuiHost>(wm: &mut GameWindowManager, h: &mut H, w: WinId, progress: i32) {
    wm.win_send_system_msg(h, Some(w), ggm::GPM_SET_PROGRESS, progress as u32, 0);
}

/// The gadget a `GBM_*` message names in its `mData1`.
pub fn msg_gadget(d1: WindowMsgData) -> Option<WinId> {
    data_win(d1)
}
