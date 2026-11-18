//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DMOTD.cpp
//!     (`closeButtonID`, `MOTDSystem`)

use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// `static NameKeyType closeButtonID`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Motd {
    pub close_button_id: i32,
}

pub trait MotdHost: ShellHost {
    fn motd(&mut self) -> &mut Motd;
}

/// `MOTDSystem(window, msg, mData1, mData2)`: the message of the day.
pub fn motd_system<H: MotdHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {
            // load id's needed
            let id = h.name_to_key(b"MOTD.wnd:CloseMOTD") as i32;
            h.motd().close_button_id = id;
        }
        gwm::DESTROY => {}
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.win(control).id;
            if control_id == h.motd().close_button_id {
                let hidden = wm.is_hidden(Some(window));
                wm.win_hide(window, !hidden);
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
