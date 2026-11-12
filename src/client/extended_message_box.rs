// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ExtendedMessageBox.cpp (the whole file:
//     gogoExMessageBox, ExMessageBoxYesNo, ExMessageBoxYesNoCancel, ExMessageBoxOkCancel, ExMessageBoxOk,
//     ExMessageBoxCancel, ExtendedMessageBoxSystem)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ExtendedMessageBox.h (MessageBoxReturnType, WindowExMessageBoxData)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The extended message box (GitHub #94, #157): a message box whose callbacks take a user datum and answer whether the
//! box closes. The insert-disc prompt (`CheckForCDAtGameStart`, `checkCDBeforeCampaign`) is its only user in the
//! shell.
//!
//! # As upstream reads it
//!
//! `gogoExMessageBox(x, y, width, height, buttonFlags, title, body, userData, yes, no, ok, cancel)`: nothing without
//! a button. It reads `Menus/MessageBox.wnd`, makes the file's **first window** modal and gives it the focus; scales
//! and places it as `gogoMessageBox` does, but the first window and its children, not `MessageBoxParent`; places and
//! shows the buttons (OK or Yes in the first slot, No and Cancel in the second and third, No or Cancel alone in the
//! third); sets the title and the message; and makes the callbacks and `userData` the first window's user data and
//! `ExtendedMessageBoxSystem` its system callback (replacing the file's). **`ExtendedMessageBoxSystem`**: it wants the
//! focus; a button's `GBM_SELECTED` calls that button's callback with the user datum, and the window is destroyed when
//! the callback answers `MB_RETURN_CLOSE` (or there is none); `GWM_DESTROY` frees the data.
//!
//! # Upstream quirks that are kept
//!
//! - A window the file lacks is used without a check: recorded as undefined ([`GameWindowManager::set_undefined`]),
//!   no box made. `ButtonYes` is only needed when Yes is asked for.
//! - `ratioX` is uninitialised when no size is given; it is only read when one is.
//! - A press on any other window that passes `GBM_SELECTED` up closes the box (`ret` starts as `MB_RETURN_CLOSE`).
//!
//! # Where this differs from upstream
//!
//! The callbacks (`MessageBoxFunc`) and the user datum (a callback or a difficulty cast to `void *`) are names and a
//! value ([`ExMessageBoxData`]); the shell's host runs them ([`ShellHost::ex_message_box_callback`]).

use crate::client::gadget::static_text_set_text;
use crate::client::message_box::{MSG_BOX_CANCEL, MSG_BOX_NO, MSG_BOX_OK, MSG_BOX_YES};
use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, GameWindowManager, MsgHandled, SystemFunc, UserData, WindowMsgData};
use crate::client::window_script::WinId;

/// `MessageBoxReturnType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageBoxReturn {
    /// `MB_RETURN_CLOSE`.
    Close,
    /// `MB_RETURN_KEEPOPEN`.
    KeepOpen,
}

/// The `void *userData` a caller passes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ExUserData {
    #[default]
    Null,
    /// A `gameStartCallback` (`reallyDoStart`, `startNextCampaignGame`), by name.
    Callback(Vec<u8>),
    /// A number (`(void *)diff`).
    Int(i32),
}

/// `WindowExMessageBoxData`: each button's callback by name (`None`: `NULL`), and the user datum.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExMessageBoxData {
    pub yes: Option<Vec<u8>>,
    pub no: Option<Vec<u8>>,
    pub ok: Option<Vec<u8>>,
    pub cancel: Option<Vec<u8>>,
    pub user_data: ExUserData,
}

fn find<H: ShellHost>(wm: &GameWindowManager, h: &mut H, under: WinId, name: &str) -> Option<WinId> {
    let id = h.name_to_key(name.as_bytes()) as i32;
    wm.win_get_window_from_id(Some(under), id)
}

/// `gogoExMessageBox(...)`: the box's first window.
#[allow(clippy::too_many_arguments)]
pub fn gogo_ex_message_box<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, x: i32, y: i32, width: i32, height: i32, button_flags: u16, title: &[u16], body: &[u16], data: ExMessageBoxData) -> Option<WinId> {
    if button_flags == 0 {
        return None;
    }
    let parent = h.create_from_script(wm, b"Menus/MessageBox.wnd");
    wm.win_set_modal(parent);
    wm.win_set_focus(h, None);
    wm.win_set_focus(h, parent);
    let Some(parent) = parent else {
        wm.set_undefined("gogoExMessageBox: Menus/MessageBox.wnd is missing (used through NULL)");
        return None;
    };
    if width > 0 && height > 0 {
        let (pw, ph) = wm.tree.windows[parent].size;
        let ratio_x = width as f32 / pw as f32;
        let ratio_y = height as f32 / ph as f32;
        wm.win_set_size(h, parent, width, height);
        for child in wm.tree.windows[parent].children.clone() {
            let (cw, ch) = wm.tree.windows[child].size;
            wm.win_set_size(h, child, (cw as f32 * ratio_x) as i32, (ch as f32 * ratio_y) as i32);
            let (cx, cy) = (wm.tree.windows[child].region.0, wm.tree.windows[child].region.1);
            wm.win_set_position(child, (cx as f32 * ratio_x) as i32, (cy as f32 * ratio_y) as i32);
        }
    }
    if x >= 0 && y >= 0 {
        wm.win_set_position(parent, x, y);
    }
    let ok = find(wm, h, parent, "MessageBox.wnd:ButtonOk");
    let yes = find(wm, h, parent, "MessageBox.wnd:ButtonYes");
    let no = find(wm, h, parent, "MessageBox.wnd:ButtonNo");
    let cancel = find(wm, h, parent, "MessageBox.wnd:ButtonCancel");
    let (Some(ok), Some(no), Some(cancel)) = (ok, no, cancel) else {
        wm.set_undefined("gogoExMessageBox: ButtonOk, ButtonNo or ButtonCancel is missing (winGetPosition on NULL)");
        return None;
    };
    let pos = |wm: &GameWindowManager, w: WinId| (wm.tree.windows[w].region.0, wm.tree.windows[w].region.1);
    let slots = [pos(wm, ok), pos(wm, no), pos(wm, cancel)];
    if button_flags & MSG_BOX_OK == MSG_BOX_OK {
        wm.win_set_position(ok, slots[0].0, slots[0].1);
        wm.win_hide(ok, false);
    } else if button_flags & MSG_BOX_YES == MSG_BOX_YES {
        let Some(yes) = yes else {
            wm.set_undefined("gogoExMessageBox: ButtonYes is missing (winSetPosition on NULL)");
            return None;
        };
        wm.win_set_position(yes, slots[0].0, slots[0].1);
        wm.win_hide(yes, false);
    }
    if button_flags & (MSG_BOX_NO | MSG_BOX_CANCEL) == (MSG_BOX_NO | MSG_BOX_CANCEL) {
        wm.win_set_position(no, slots[1].0, slots[1].1);
        wm.win_set_position(cancel, slots[2].0, slots[2].1);
        wm.win_hide(no, false);
        wm.win_hide(cancel, false);
    } else if button_flags & MSG_BOX_NO == MSG_BOX_NO {
        wm.win_set_position(no, slots[2].0, slots[2].1);
        wm.win_hide(no, false);
    } else if button_flags & MSG_BOX_CANCEL == MSG_BOX_CANCEL {
        wm.win_set_position(cancel, slots[2].0, slots[2].1);
        wm.win_hide(cancel, false);
    }
    let t = find(wm, h, parent, "MessageBox.wnd:StaticTextTitle");
    static_text_set_text(wm, h, t, title);
    let m = find(wm, h, parent, "MessageBox.wnd:StaticTextMessage");
    static_text_set_text(wm, h, m, body);
    wm.ext[parent].user = UserData::ExMessageBox(data);
    wm.ext[parent].system = SystemFunc::Host(b"ExtendedMessageBoxSystem".to_vec());
    wm.win_hide(parent, false);
    wm.win_bring_to_top(parent);
    Some(parent)
}

fn name(s: Option<&str>) -> Option<Vec<u8>> {
    s.map(|s| s.as_bytes().to_vec())
}

/// `ExMessageBoxYesNo(title, body, userData, yes, no)`.
pub fn ex_message_box_yes_no<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], user_data: ExUserData, yes: Option<&str>, no: Option<&str>) -> Option<WinId> {
    gogo_ex_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_NO | MSG_BOX_YES, title, body, ExMessageBoxData { yes: name(yes), no: name(no), user_data, ..Default::default() })
}

/// `ExMessageBoxYesNoCancel(title, body, userData, yes, no, cancel)`.
#[allow(clippy::too_many_arguments)]
pub fn ex_message_box_yes_no_cancel<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], user_data: ExUserData, yes: Option<&str>, no: Option<&str>, cancel: Option<&str>) -> Option<WinId> {
    gogo_ex_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_NO | MSG_BOX_YES | MSG_BOX_CANCEL, title, body, ExMessageBoxData { yes: name(yes), no: name(no), cancel: name(cancel), user_data, ..Default::default() })
}

/// `ExMessageBoxOkCancel(title, body, userData, ok, cancel)`.
pub fn ex_message_box_ok_cancel<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], user_data: ExUserData, ok: Option<&str>, cancel: Option<&str>) -> Option<WinId> {
    gogo_ex_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_OK | MSG_BOX_CANCEL, title, body, ExMessageBoxData { ok: name(ok), cancel: name(cancel), user_data, ..Default::default() })
}

/// `ExMessageBoxOk(title, body, userData, ok)`.
pub fn ex_message_box_ok<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], user_data: ExUserData, ok: Option<&str>) -> Option<WinId> {
    gogo_ex_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_OK, title, body, ExMessageBoxData { ok: name(ok), user_data, ..Default::default() })
}

/// `ExMessageBoxCancel(title, body, userData, cancel)`.
pub fn ex_message_box_cancel<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], user_data: ExUserData, cancel: Option<&str>) -> Option<WinId> {
    gogo_ex_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_CANCEL, title, body, ExMessageBoxData { cancel: name(cancel), user_data, ..Default::default() })
}

/// `ExtendedMessageBoxSystem(window, msg, mData1, mData2)`.
pub fn extended_message_box_system<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::DESTROY => wm.ext[window].user = UserData::None,
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("ExtendedMessageBoxSystem: GBM_SELECTED with no control");
                return MsgHandled::Handled;
            };
            let control_id = wm.tree.windows[control].id;
            let ids: Vec<i32> = ["ButtonOk", "ButtonYes", "ButtonNo", "ButtonCancel"].iter().map(|b| h.name_to_key(format!("MessageBox.wnd:{b}").as_bytes()) as i32).collect();
            let UserData::ExMessageBox(cb) = wm.ext[window].user.clone() else {
                wm.set_undefined("ExtendedMessageBoxSystem: the window has no WindowExMessageBoxData (read through NULL)");
                return MsgHandled::Handled;
            };
            let mut ret = MessageBoxReturn::Close;
            let f = match ids.iter().position(|&i| i == control_id) {
                Some(0) => cb.ok.clone(),
                Some(1) => cb.yes.clone(),
                Some(2) => cb.no.clone(),
                Some(3) => cb.cancel.clone(),
                _ => None,
            };
            if let Some(f) = f {
                ret = h.ex_message_box_callback(wm, &f, &cb.user_data);
            }
            if ret == MessageBoxReturn::Close {
                wm.win_destroy(Some(window));
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
