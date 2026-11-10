// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/MessageBox.cpp (MessageBoxYesNo,
//     QuitMessageBoxYesNo, MessageBoxYesNoCancel, MessageBoxOkCancel, MessageBoxOk, MessageBoxCancel,
//     MessageBoxSystem, QuitMessageBoxSystem)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (gogoMessageBox)
//   GeneralsMD/Code/GameEngine/Include/GameClient/GameWindow.h (MSG_BOX_*, WindowMessageBoxData)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The popup message box (GitHub #69): `MessageBoxOk`, `MessageBoxYesNo`, `QuitMessageBoxYesNo`, ... and the system
//! callback of their window. (Upstream has no `PopupMessageBox.cpp`: the popup is `MessageBox.cpp` and
//! `gogoMessageBox`.)
//!
//! # As upstream reads it
//!
//! `gogoMessageBox(x, y, width, height, buttonFlags, title, body, yes, no, ok, cancel, useLogo)`: nothing without a
//! button. It reads `Menus/MessageBox.wnd` (`Menus/QuitMessageBox.wnd` with the logo, the quit box) with no layout,
//! finds `<file>:MessageBoxParent` in it, makes the file's first window **modal**, takes the focus from whoever had it
//! and gives it to the parent. With a size, the parent and each of its children are scaled by the ratio of the new
//! size to the parent's; with a position, the parent moves. The buttons: `ButtonOk` (whose position is the first
//! slot), `ButtonNo` (the second), `ButtonCancel` (the third), `ButtonYes`; OK or else Yes goes to the first slot; No
//! and Cancel together go to the second and third, No alone or Cancel alone to the third; each one placed is shown.
//! The title and the message are set into `StaticTextTitle` and `StaticTextMessage`; the four callbacks are the first
//! window's user data; the parent is shown and brought to the top.
//!
//! `MessageBoxSystem` (and `QuitMessageBoxSystem`, its copy for the quit box's names): it wants the focus; on
//! `GBM_SELECTED` from `ButtonOk`, `ButtonYes`, `ButtonNo` or `ButtonCancel` it calls that button's callback if there
//! is one, then destroys the window; `GWM_DESTROY` frees the callbacks.
//!
//! # Upstream quirks that are kept
//!
//! - A button the file lacks (or the parent) is used without a check: upstream's `NULL` call is recorded as undefined
//!   ([`GameWindowManager::undefined`]) and no box is made.
//! - OK and Yes together is only a debug assertion: OK is placed, Yes stays as the file has it.
//! - The callbacks are read from the window the `GBM_SELECTED` reached (the buttons' owner), and set on the file's
//!   **first** window: the file must make them the same window (or pass the message up). A box whose window has no
//!   callbacks is upstream's `NULL` read: recorded as undefined.
//! - The box destroys its first window only; its modal state is undone by `winDestroy` (the top of the modal stack).
//!
//! # Where this differs from upstream
//!
//! The callbacks (`GameWinMsgBoxFunc`, function pointers) are the callers' names ([`MessageBoxData`]); the
//! [`ShellHost`] runs them ([`ShellHost::message_box_callback`]).
//!
//! # Representation
//!
//! [`MessageBoxData`] in the window's [`UserData::MessageBox`].

use crate::client::gadget::static_text_set_text;
use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, GameWindowManager, MsgHandled, UserData, WindowMsgData};
use crate::client::window_script::WinId;

/// `MSG_BOX_YES`.
pub const MSG_BOX_YES: u16 = 0x01;
/// `MSG_BOX_NO`.
pub const MSG_BOX_NO: u16 = 0x02;
/// `MSG_BOX_CANCEL`.
pub const MSG_BOX_CANCEL: u16 = 0x04;
/// `MSG_BOX_OK`.
pub const MSG_BOX_OK: u16 = 0x08;

/// `WindowMessageBoxData`: each button's callback, by the caller's name (`None`: `NULL`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessageBoxData {
    pub yes: Option<Vec<u8>>,
    pub no: Option<Vec<u8>>,
    pub ok: Option<Vec<u8>>,
    pub cancel: Option<Vec<u8>>,
}

/// The callbacks of a box, by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MsgBoxCallbacks {
    pub yes: Option<&'static str>,
    pub no: Option<&'static str>,
    pub ok: Option<&'static str>,
    pub cancel: Option<&'static str>,
}

fn find<H: ShellHost>(wm: &GameWindowManager, h: &mut H, under: Option<WinId>, name: &str) -> Option<WinId> {
    let id = h.name_to_key(name.as_bytes()) as i32;
    wm.win_get_window_from_id(under, id)
}

/// `gogoMessageBox(x, y, width, height, buttonFlags, title, body, yes, no, ok, cancel, useLogo)`: the box's first
/// window.
#[allow(clippy::too_many_arguments)]
pub fn gogo_message_box<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, x: i32, y: i32, width: i32, height: i32, button_flags: u16, title: &[u16], body: &[u16], cb: MsgBoxCallbacks, use_logo: bool) -> Option<WinId> {
    if button_flags == 0 {
        return None;
    }
    let (file, menu): (&[u8], &str) = if use_logo { (b"Menus/QuitMessageBox.wnd", "QuitMessageBox.wnd:") } else { (b"Menus/MessageBox.wnd", "MessageBox.wnd:") };
    let true_parent = h.create_from_script(wm, file);
    let parent = match true_parent {
        Some(t) => find(wm, h, Some(t), &format!("{menu}MessageBoxParent")),
        None => None,
    };
    wm.win_set_modal(true_parent);
    wm.win_set_focus(h, None);
    wm.win_set_focus(h, parent);
    let (Some(true_parent), Some(parent)) = (true_parent, parent) else {
        wm.set_undefined("gogoMessageBox: the message box file or its MessageBoxParent is missing (used through NULL)");
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
    let ok = find(wm, h, Some(parent), &format!("{menu}ButtonOk"));
    let yes = find(wm, h, Some(parent), &format!("{menu}ButtonYes"));
    let no = find(wm, h, Some(parent), &format!("{menu}ButtonNo"));
    let cancel = find(wm, h, Some(parent), &format!("{menu}ButtonCancel"));
    let (Some(ok), Some(no), Some(cancel)) = (ok, no, cancel) else {
        wm.set_undefined("gogoMessageBox: ButtonOk, ButtonNo or ButtonCancel is missing (winGetPosition on NULL)");
        return None;
    };
    let pos = |wm: &GameWindowManager, w: WinId| (wm.tree.windows[w].region.0, wm.tree.windows[w].region.1);
    let slots = [pos(wm, ok), pos(wm, no), pos(wm, cancel)];
    if button_flags & MSG_BOX_OK == MSG_BOX_OK {
        wm.win_set_position(ok, slots[0].0, slots[0].1);
        wm.win_hide(ok, false);
    } else if button_flags & MSG_BOX_YES == MSG_BOX_YES {
        let Some(yes) = yes else {
            wm.set_undefined("gogoMessageBox: ButtonYes is missing (winSetPosition on NULL)");
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
    let t = find(wm, h, Some(parent), &format!("{menu}StaticTextTitle"));
    static_text_set_text(wm, h, t, title);
    let m = find(wm, h, Some(parent), &format!("{menu}StaticTextMessage"));
    static_text_set_text(wm, h, m, body);
    let name = |s: Option<&'static str>| s.map(|s| s.as_bytes().to_vec());
    wm.ext[true_parent].user = UserData::MessageBox(MessageBoxData { yes: name(cb.yes), no: name(cb.no), ok: name(cb.ok), cancel: name(cb.cancel) });
    wm.win_hide(parent, false);
    wm.win_bring_to_top(parent);
    Some(true_parent)
}

/// `MessageBoxYesNo(title, body, yes, no)`.
pub fn message_box_yes_no<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], yes: Option<&'static str>, no: Option<&'static str>) -> Option<WinId> {
    gogo_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_NO | MSG_BOX_YES, title, body, MsgBoxCallbacks { yes, no, ..Default::default() }, false)
}

/// `QuitMessageBoxYesNo(title, body, yes, no)`: the quit box (with the logo).
pub fn quit_message_box_yes_no<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], yes: Option<&'static str>, no: Option<&'static str>) -> Option<WinId> {
    gogo_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_NO | MSG_BOX_YES, title, body, MsgBoxCallbacks { yes, no, ..Default::default() }, true)
}

/// `MessageBoxYesNoCancel(title, body, yes, no, cancel)`.
pub fn message_box_yes_no_cancel<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], yes: Option<&'static str>, no: Option<&'static str>, cancel: Option<&'static str>) -> Option<WinId> {
    gogo_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_NO | MSG_BOX_YES | MSG_BOX_CANCEL, title, body, MsgBoxCallbacks { yes, no, cancel, ..Default::default() }, false)
}

/// `MessageBoxOkCancel(title, body, ok, cancel)`.
pub fn message_box_ok_cancel<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], ok: Option<&'static str>, cancel: Option<&'static str>) -> Option<WinId> {
    gogo_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_OK | MSG_BOX_CANCEL, title, body, MsgBoxCallbacks { ok, cancel, ..Default::default() }, false)
}

/// `MessageBoxOk(title, body, ok)`.
pub fn message_box_ok<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], ok: Option<&'static str>) -> Option<WinId> {
    gogo_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_OK, title, body, MsgBoxCallbacks { ok, ..Default::default() }, false)
}

/// `MessageBoxCancel(title, body, cancel)`.
pub fn message_box_cancel<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, title: &[u16], body: &[u16], cancel: Option<&'static str>) -> Option<WinId> {
    gogo_message_box(wm, h, -1, -1, -1, -1, MSG_BOX_CANCEL, title, body, MsgBoxCallbacks { cancel, ..Default::default() }, false)
}

/// `MessageBoxSystem` (`quit` false) and `QuitMessageBoxSystem` (`quit` true: the quit box's button names).
pub fn message_box_system<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData, quit: bool) -> MsgHandled {
    match msg {
        gwm::DESTROY => wm.ext[window].user = UserData::None,
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("MessageBoxSystem: GBM_SELECTED with no control");
                return MsgHandled::Handled;
            };
            let control_id = wm.tree.windows[control].id;
            let menu = if quit { "QuitMessageBox.wnd:" } else { "MessageBox.wnd:" };
            let ids: Vec<i32> = ["ButtonOk", "ButtonYes", "ButtonNo", "ButtonCancel"].iter().map(|b| h.name_to_key(format!("{menu}{b}").as_bytes()) as i32).collect();
            let Some(which) = ids.iter().position(|&i| i == control_id) else { return MsgHandled::Handled };
            let UserData::MessageBox(cb) = wm.ext[window].user.clone() else {
                wm.set_undefined("MessageBoxSystem: the window has no WindowMessageBoxData (read through NULL)");
                return MsgHandled::Handled;
            };
            let f = [cb.ok, cb.yes, cb.no, cb.cancel][which].clone();
            if let Some(f) = f {
                h.message_box_callback(wm, &f);
            }
            wm.win_destroy(Some(window));
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
