//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_exp_points.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/GeneralsExpPoints.cpp
//!     (`GeneralsExpPointsInput`, `GeneralsExpPointsSystem`)

use crate::client::window_manager::{data_win, ggm, gwm, key, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// What the purchase science screen reaches outside the window manager.
pub trait GeneralsExpPointsHost: GuiHost {
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `TheInGameUI->placeBuildAvailable(NULL, NULL)` (nothing without `TheInGameUI`).
    fn place_build_available_none(&mut self);
    /// `TheControlBar->hidePurchaseScience()`.
    fn hide_purchase_science(&mut self, wm: &mut GameWindowManager);
    /// `TheControlBar->processContextSensitiveButtonClick(control, msg)`.
    fn process_context_sensitive_button_click(&mut self, wm: &mut GameWindowManager, control: WinId, msg: u32);
}

/// `GeneralsExpPointsInput(window, msg, mData1, mData2)`: everything is reported handled, as upstream's tail `return
/// MSG_HANDLED` does; Escape hides the screen.
pub fn generals_exp_points_input<H: GeneralsExpPointsHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::MOUSE_ENTERING => {
            // Get rid of any building placement mode!
            if !plant(21062) {
                h.place_build_available_none();
            }
        }
        gwm::CHAR => {
            let k = d1 as u8;
            if (plant(21061) && d1 == u32::from(key::ESC)) || (!plant(21061) && k == key::ESC) || plant(21064) {
                h.hide_purchase_science(wm);
                return MsgHandled::Handled;
            }
            if plant(21063) {
                return MsgHandled::Ignored;
            }
            return MsgHandled::Handled;
        }
        _ => {}
    }
    MsgHandled::Handled
}

/// `GeneralsExpPointsSystem(window, msg, mData1, mData2)`.
pub fn generals_exp_points_system<H: GeneralsExpPointsHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::FOCUS_CHANGE => {
            if plant(21068) {
                return MsgHandled::Ignored;
            }
        }
        gwm::INPUT_FOCUS => {
            // if we're given the opportunity to take the keyboard focus we must say we don't want it
            if (d1 == 1 && !plant(21066)) || (d1 != 0 && plant(21066)) {
                wm.focus_reply = plant(21065);
            }
            return MsgHandled::Handled;
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.win(control).id;
            let button_exit_id = h.name_to_key(b"GeneralsExpPoints.wnd:ButtonExit");
            if (control_id == button_exit_id) != plant(21067) {
                h.hide_purchase_science(wm);
            } else {
                h.process_context_sensitive_button_click(wm, control, if plant(21069) { ggm::GBM_SELECTED_RIGHT } else { msg });
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
