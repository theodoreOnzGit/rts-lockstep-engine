//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/PopupCommunicator.cpp
//!     (the file's statics, `PopupCommunicatorInit`, `PopupCommunicatorShutdown`, `PopupcommunicatorUpdate`,
//!      `PopupCommunicatorInput`, `PopupCommunicatorSystem`)

use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// The file's statics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PopupCommunicator {
    pub button_ok_id: i32,
    pub button_ok: Option<WinId>,
    pub parent: Option<WinId>,
}

pub trait PopupCommunicatorHost: ShellHost {
    fn popup_communicator(&mut self) -> &mut PopupCommunicator;
}

/// `PopupCommunicatorInit(layout, userData)`.
pub fn popup_communicator_init<H: PopupCommunicatorHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    // set keyboard focus to main parent and set modal
    let parent_id = h.name_to_key(b"PopupCommunicator.wnd:PopupCommunicator") as i32;
    let parent = wm.win_get_window_from_id(None, parent_id);
    h.popup_communicator().parent = parent;
    wm.win_set_focus(h, parent);
    wm.win_set_modal(parent);

    // get ids for our children controls
    let button_ok_id = h.name_to_key(b"PopupCommunicator.wnd:ButtonOk") as i32;
    let button_ok = wm.win_get_window_from_id(parent, button_ok_id);
    h.popup_communicator().button_ok_id = button_ok_id;
    h.popup_communicator().button_ok = button_ok;
}

/// `PopupCommunicatorShutdown(layout, userData)`: empty upstream.
pub fn popup_communicator_shutdown() {}

/// `PopupcommunicatorUpdate(layout, userData)`: empty upstream.
pub fn popup_communicator_update() {}

/// `PopupCommunicatorInput(window, msg, mData1, mData2)`.
pub fn popup_communicator_input<H: PopupCommunicatorHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR {
        let k = d1 as u8;
        let state = d2 as u8;
        if k == key::ESC {
            // send a simulated selected event to the parent window of the back/exit button
            if u32::from(state) & key::STATE_UP != 0 {
                let p = h.popup_communicator().clone();
                wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(p.button_ok), p.button_ok_id as u32);
            }
            // don't let key fall through anywhere else
            return MsgHandled::Handled;
        }
    }
    MsgHandled::Ignored
}

/// `PopupCommunicatorSystem(window, msg, mData1, mData2)`.
pub fn popup_communicator_system<H: PopupCommunicatorHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {}
        gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.win(control).id;
            if control_id == h.popup_communicator().button_ok_id {
                if let Some(layout) = wm.ext[window].layout {
                    wm.layout_destroy_windows(layout);
                    // `deleteInstance()`: the layout record is released by the host
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
