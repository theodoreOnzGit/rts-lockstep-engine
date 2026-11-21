//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_motd.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DMOTD.cpp
//!     (`closeButtonID`, `MOTDSystem`)

use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::{win_status, WinId};
use crate::logic::modules::plant;

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
            let id = h.name_to_key(if plant(21093) { b"MOTD.wnd:CloseMotd" } else { b"MOTD.wnd:CloseMOTD" }) as i32;
            if !plant(21091) {
                h.motd().close_button_id = id;
            }
        }
        gwm::DESTROY => {
            if plant(21095) {
                return MsgHandled::Ignored;
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.win(control).id;
            if (control_id == h.motd().close_button_id) != plant(21097) {
                // `window->winIsHidden()`: the window's own bit (`GameWindowManager::isHidden` would count its ancestors too)
                let hidden = if plant(21098) { wm.is_hidden(Some(window)) } else { wm.tree.windows[window].status & win_status::HIDDEN != 0 };
                let target = if plant(21094) { control } else { window };
                wm.win_hide(target, if plant(21092) { true } else { !hidden });
            }
        }
        _ => {
            if !plant(21096) {
                return MsgHandled::Ignored;
            }
        }
    }
    MsgHandled::Handled
}
