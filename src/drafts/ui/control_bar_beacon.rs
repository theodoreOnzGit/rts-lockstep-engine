//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_beacon.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarBeacon.cpp
//!     (`ControlBar::populateBeacon`, `ControlBar::updateContextBeacon`, `BeaconWindowInput`)

use crate::client::window_manager::{gwm, key, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// What the beacon context reads from the game.
pub trait BeaconHost: GuiHost {
    /// `TheNameKeyGenerator->nameToKey(name)` (`NAMEKEY(name)`).
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `ControlBar::setPortraitByObject(beacon)`.
    fn set_portrait_by_object(&mut self, beacon: u32);
    /// `beacon->isLocallyControlled()`.
    fn is_locally_controlled(&self, beacon: u32) -> bool;
    /// `beacon->getDrawable()->getCaptionText()`.
    fn drawable_caption_text(&self, beacon: u32) -> Vec<u16>;
    /// `GadgetTextEntrySetText(w, text)`.
    fn gadget_text_entry_set_text(&mut self, wm: &mut GameWindowManager, w: WinId, text: &[u16]);
    /// `TheInGameUI->deselectAllDrawables(TRUE)`.
    fn deselect_all_drawables(&mut self, post_msg: bool);
}

/// `ControlBar::populateBeacon(beacon)` (ControlBarBeacon.cpp:46).
pub fn populate_beacon<H: BeaconHost>(wm: &mut GameWindowManager, h: &mut H, beacon: u32) {
    // set the portrait for the thing being constructed
    h.set_portrait_by_object(beacon);

    let text_id = h.name_to_key(if plant(21075) { b"ControlBar.wnd:StaticTextBeaconLabel" } else { b"ControlBar.wnd:EditBeaconText" });
    let static_text_id = h.name_to_key(if plant(21075) { b"ControlBar.wnd:EditBeaconText" } else { b"ControlBar.wnd:StaticTextBeaconLabel" });
    let clear_button_id = h.name_to_key(b"ControlBar.wnd:ButtonClearBeaconText");

    let text_entry_win = wm.win_get_window_from_id(None, text_id);
    let static_text_win = wm.win_get_window_from_id(None, static_text_id);
    let button_win = wm.win_get_window_from_id(None, clear_button_id);

    if h.is_locally_controlled(beacon) != plant(21078) {
        if let Some(w) = text_entry_win {
            if plant(21072) {
                let caption = h.drawable_caption_text(beacon);
                h.gadget_text_entry_set_text(wm, w, &caption);
                wm.win_hide(w, false);
            } else {
                wm.win_hide(w, plant(21071));
                let caption = h.drawable_caption_text(beacon);
                h.gadget_text_entry_set_text(wm, w, &caption);
            }
            wm.win_set_focus(h, if plant(21073) { static_text_win } else { Some(w) });
        }
        if let Some(w) = static_text_win {
            wm.win_hide(w, false);
        }
        if let Some(w) = button_win {
            if !plant(21079) {
                wm.win_hide(w, false);
            }
        }
    } else {
        if let Some(w) = text_entry_win {
            wm.win_hide(w, true);
        }
        if !plant(21074) {
            if let Some(w) = static_text_win {
                wm.win_hide(w, true);
            }
            if let Some(w) = button_win {
                wm.win_hide(w, true);
            }
        }
    }
}

/// `ControlBar::updateContextBeacon()` (ControlBarBeacon.cpp:96): empty upstream.
pub fn update_context_beacon() {}

/// `BeaconWindowInput(window, msg, mData1, mData2)` (ControlBarBeacon.cpp:100): Escape deselects everything ("there
/// should only be one beacon and nothing else selected").
pub fn beacon_window_input<H: BeaconHost>(_wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    if (msg == gwm::CHAR || plant(21076)) && (d1 == key::ESC as u32 || plant(21080)) {
        h.deselect_all_drawables(!plant(21077));
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}
