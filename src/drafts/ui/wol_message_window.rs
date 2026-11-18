//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLMessageWindow.cpp
//!     (the file's statics, `WOLMessageWindowInit/Shutdown/Update/Input/System`)
//!
//! The GameSpy/WOL service is gone. `WOLMessageWindowUpdate`'s body (`WOL::TheWOL->update()`) is commented out
//! upstream; *Cancel* has no handler upstream.

use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WolMessageWindow {
    pub parent_wol_message_window_id: i32,
    pub button_cancel_id: i32,
    pub parent_wol_message_window: Option<WinId>,
    pub button_cancel: Option<WinId>,
}

/// What the message window needs of the host.
pub trait WolMessageWindowHost: ShellHost {
    fn wol_message_window(&mut self) -> &mut WolMessageWindow;
}

/// `WOLMessageWindowInit(layout, userData)`.
pub fn wol_message_window_init<H: WolMessageWindowHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let parent_id = ShellHost::name_to_key(h, b"WOLMessageWindow.wnd:WOLMessageWindowParent") as i32;
    let cancel_id = ShellHost::name_to_key(h, b"WOLMessageWindow.wnd:ButtonCancel") as i32;
    let parent = wm.win_get_window_from_id(None, parent_id);
    let cancel = wm.win_get_window_from_id(None, cancel_id);
    {
        let s = h.wol_message_window();
        s.parent_wol_message_window_id = parent_id;
        s.button_cancel_id = cancel_id;
        s.parent_wol_message_window = parent;
        s.button_cancel = cancel;
    }
    wm.layout_hide(layout, false);
    wm.win_set_focus(h, parent);
}

/// `WOLMessageWindowShutdown(layout, userData)`.
pub fn wol_message_window_shutdown<H: WolMessageWindowHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `WOLMessageWindowUpdate(layout, userData)`: empty (`WOL::TheWOL->update()` is commented out).
pub fn wol_message_window_update<H: WolMessageWindowHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `WOLMessageWindowInput(window, msg, mData1, mData2)`: Escape's release presses *Cancel*.
pub fn wol_message_window_input<H: WolMessageWindowHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            let s = h.wol_message_window().clone();
            wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(s.button_cancel), s.button_cancel_id as u32);
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `WOLMessageWindowSystem(window, msg, mData1, mData2)`.
pub fn wol_message_window_system<H: WolMessageWindowHost>(wm: &mut GameWindowManager, _h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GBM_SELECTED | ggm::GEM_EDIT_DONE => {}
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
