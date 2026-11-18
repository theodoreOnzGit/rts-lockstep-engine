//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLStatusMenu.cpp
//!     (the file's statics, `WOLStatusMenuInit/Shutdown/Update/Input/System`)
//!
//! The GameSpy/WOL service is gone: the *Disconnect* button's body is commented out upstream (it reset the WOL API),
//! so the button does nothing here either.

use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`); `progressTextWindow` is a global.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WolStatusMenu {
    pub parent_wol_status_id: i32,
    pub button_disconnect_id: i32,
    pub parent_wol_status: Option<WinId>,
    pub button_disconnect: Option<WinId>,
    pub progress_text_window: Option<WinId>,
}

/// What the status menu needs of the host.
pub trait WolStatusMenuHost: ShellHost {
    fn wol_status_menu(&mut self) -> &mut WolStatusMenu;
}

/// `WOLStatusMenuInit(layout, userData)`.
pub fn wol_status_menu_init<H: WolStatusMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let parent_id = ShellHost::name_to_key(h, b"WOLStatusMenu.wnd:WOLStatusMenuParent") as i32;
    let button_id = ShellHost::name_to_key(h, b"WOLStatusMenu.wnd:ButtonDisconnect") as i32;
    let list_id = ShellHost::name_to_key(h, b"WOLStatusMenu.wnd:ListboxStatus") as i32;
    let parent = wm.win_get_window_from_id(None, parent_id);
    let button = wm.win_get_window_from_id(None, button_id);
    let progress = wm.win_get_window_from_id(None, list_id);
    {
        let s = h.wol_status_menu();
        s.parent_wol_status_id = parent_id;
        s.button_disconnect_id = button_id;
        s.parent_wol_status = parent;
        s.button_disconnect = button;
        s.progress_text_window = progress;
    }
    wm.layout_hide(layout, false);
    wm.win_set_focus(h, parent);
}

/// `WOLStatusMenuShutdown(layout, userData)`.
pub fn wol_status_menu_shutdown<H: WolStatusMenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `WOLStatusMenuUpdate(layout, userData)`: empty.
pub fn wol_status_menu_update<H: WolStatusMenuHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `WOLStatusMenuInput(window, msg, mData1, mData2)`: Escape's release presses *Disconnect*.
pub fn wol_status_menu_input<H: WolStatusMenuHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            let s = h.wol_status_menu().clone();
            wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(s.button_disconnect), s.button_disconnect_id as u32);
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `WOLStatusMenuSystem(window, msg, mData1, mData2)`.
pub fn wol_status_menu_system<H: WolStatusMenuHost>(wm: &mut GameWindowManager, _h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        // GBM_SELECTED: the body (reset the WOL API on *Disconnect*) is commented out upstream
        ggm::GBM_SELECTED | ggm::GEM_EDIT_DONE => {}
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
