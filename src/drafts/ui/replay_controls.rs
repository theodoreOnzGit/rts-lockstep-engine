//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ReplayControls.cpp
//!     (`ReplayControlInput`, `ReplayControlSystem`)

use crate::client::window_manager::{ggm, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// `ReplayControlInput(window, msg, mData1, mData2)`: ignores everything.
pub fn replay_control_input(_window: WinId, _msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    MsgHandled::Ignored
}

/// `ReplayControlSystem(window, msg, mData1, mData2)`: `GBM_SELECTED` is handled and does nothing.
pub fn replay_control_system<H: GuiHost>(_wm: &mut GameWindowManager, _h: &mut H, _window: WinId, msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::GBM_SELECTED => MsgHandled::Handled,
        _ => MsgHandled::Ignored,
    }
}
