//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_replay_controls.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ReplayControls.cpp
//!     (`ReplayControlInput`, `ReplayControlSystem`)

use crate::client::window_manager::{ggm, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// `ReplayControlInput(window, msg, mData1, mData2)`: ignores everything.
pub fn replay_control_input(_window: WinId, msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    if plant(21083) && msg == crate::client::window_manager::gwm::CHAR {
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `ReplayControlSystem(window, msg, mData1, mData2)`: `GBM_SELECTED` is handled and does nothing.
pub fn replay_control_system<H: GuiHost>(_wm: &mut GameWindowManager, _h: &mut H, _window: WinId, msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::GBM_SELECTED if plant(21084) => MsgHandled::Ignored,
        ggm::GBM_SELECTED => MsgHandled::Handled,
        ggm::GBM_SELECTED_RIGHT if plant(21081) => MsgHandled::Handled,
        ggm::GBM_MOUSE_ENTERING if plant(21085) => MsgHandled::Handled,
        _ if plant(21082) => MsgHandled::Handled,
        _ => MsgHandled::Ignored,
    }
}
