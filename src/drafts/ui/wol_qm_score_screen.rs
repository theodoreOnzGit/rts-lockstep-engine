//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLQMScoreScreen.cpp
//!     (the file's statics, `WOLQMScoreScreenInit/Shutdown/Update/Input/System`)
//!
//! The GameSpy/WOL service is gone: the *Disconnect* button's body is commented out upstream (it reset the WOL API),
//! so the buttons do nothing here either (the commented-out `getState() == WOLAPI_FATAL_ERROR` check in `Init` hid *QuickMatch*).

use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WolQmScoreScreen {
    pub parent_wol_qm_score_id: i32,
    pub button_disconnect_id: i32,
    pub parent_wol_qm_score: Option<WinId>,
    pub button_disconnect: Option<WinId>,
    pub button_quickmatch_id: i32,
    pub button_quickmatch: Option<WinId>,
}

/// What the status menu needs of the host.
pub trait WolQmScoreScreenHost: ShellHost {
    fn wol_qm_score_screen(&mut self) -> &mut WolQmScoreScreen;
}

/// `WOLQMScoreScreenInit(layout, userData)`.
pub fn wol_qm_score_screen_init<H: WolQmScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let parent_id = ShellHost::name_to_key(h, b"WOLQMScoreScreen.wnd:WOLQMScoreScreenParent") as i32;
    let button_id = ShellHost::name_to_key(h, b"WOLQMScoreScreen.wnd:ButtonDisconnect") as i32;
    let lobby_id = ShellHost::name_to_key(h, b"WOLQMScoreScreen.wnd:ButtonQuickMatch") as i32;
    let parent = wm.win_get_window_from_id(None, parent_id);
    let button = wm.win_get_window_from_id(None, button_id);
    let lobby = wm.win_get_window_from_id(None, lobby_id);
    {
        let s = h.wol_qm_score_screen();
        s.parent_wol_qm_score_id = parent_id;
        s.button_disconnect_id = button_id;
        s.parent_wol_qm_score = parent;
        s.button_disconnect = button;
        s.button_quickmatch_id = lobby_id;
        s.button_quickmatch = lobby;
    }
    wm.layout_hide(layout, false);
    wm.win_set_focus(h, parent);
}

/// `WOLQMScoreScreenShutdown(layout, userData)`.
pub fn wol_qm_score_screen_shutdown<H: WolQmScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `WOLQMScoreScreenUpdate(layout, userData)`: empty.
pub fn wol_qm_score_screen_update<H: WolQmScoreScreenHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {} // `WOL::TheWOL->update()` is commented out

/// `WOLQMScoreScreenInput(window, msg, mData1, mData2)`: Escape's release presses *Disconnect*.
pub fn wol_qm_score_screen_input<H: WolQmScoreScreenHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            let s = h.wol_qm_score_screen().clone();
            wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(s.button_disconnect), s.button_disconnect_id as u32);
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `WOLQMScoreScreenSystem(window, msg, mData1, mData2)`.
pub fn wol_qm_score_screen_system<H: WolQmScoreScreenHost>(wm: &mut GameWindowManager, _h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
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
