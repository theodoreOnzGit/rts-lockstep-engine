//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarUnderConstruction.cpp
//!     (`ControlBar::updateConstructionTextDisplay`, `populateUnderConstruction`, `updateContextUnderConstruction`)

use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::client::window_script::win_status;

/// What the under-construction context reads from the game.
pub trait UnderConstructionHost: GuiHost {
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `m_contextParent[CP_UNDER_CONSTRUCTION]`.
    fn context_parent_under_construction(&self) -> Option<usize>;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `UnicodeString::format(fmt, percent)` with the one `%.2f`/`%d` upstream's label holds (the percent is a `Real`).
    fn format_percent(&self, fmt: &[u16], percent: f32) -> Vec<u16>;
    fn gadget_static_text_set_text(&mut self, wm: &mut GameWindowManager, w: usize, text: &[u16]);
    fn find_command_button(&self, name: &[u8]) -> Option<usize>;
    fn set_control_command(&mut self, wm: &mut GameWindowManager, w: usize, command: Option<usize>);
    fn set_portrait_by_object(&mut self, obj: u32);
    /// `obj->getConstructionPercent()`.
    fn construction_percent(&self, obj: u32) -> f32;
    /// `exit = obj->getObjectExitInterface()`: `Some` when there is one (then `showRallyPoint(exit->getRallyPoint())`).
    fn show_rally_point_of_exit(&mut self, obj: u32) -> bool;
    fn current_selected_object(&self) -> Option<u32>;
    /// `obj->getStatusBits().test(OBJECT_STATUS_UNDER_CONSTRUCTION)`.
    fn is_under_construction(&self, obj: u32) -> bool;
    /// `m_displayedConstructPercent`.
    fn displayed_construct_percent(&self) -> f32;
    fn set_displayed_construct_percent(&mut self, p: f32);
    /// `ControlBar::evaluateContextUI()`.
    fn evaluate_context_ui(&mut self, wm: &mut GameWindowManager);
}

/// `ControlBar::updateConstructionTextDisplay(obj)` (ControlBarUnderConstruction.cpp:51).
pub fn update_construction_text_display<H: UnderConstructionHost>(wm: &mut GameWindowManager, h: &mut H, obj: u32) {
    let desc_id = h.name_to_key(b"ControlBar.wnd:UnderConstructionDesc");
    let desc_window = wm.win_get_window_from_id(None, desc_id);

    // format the message
    let fmt = h.fetch_text(b"CONTROLBAR:UnderConstructionDesc");
    let percent = h.construction_percent(obj);
    let text = h.format_percent(&fmt, percent);
    if let Some(d) = desc_window {
        h.gadget_static_text_set_text(wm, d, &text);
    }

    // record this as the last percentage displayed
    h.set_displayed_construct_percent(h.construction_percent(obj));
}

/// `ControlBar::populateUnderConstruction(objectUnderConstruction)` (ControlBarUnderConstruction.cpp:73).
pub fn populate_under_construction<H: UnderConstructionHost>(wm: &mut GameWindowManager, h: &mut H, object_under_construction: Option<u32>) {
    // sanity
    let Some(obj) = object_under_construction else { return };

    // get our parent window
    let parent = h.context_parent_under_construction();

    // set the cancel construction button
    let command_button = h.find_command_button(b"Command_CancelConstruction");
    let id = h.name_to_key(b"ControlBar.wnd:ButtonCancelConstruction");
    let win = wm.win_get_window_from_id(parent, id);

    if let Some(w) = win {
        h.set_control_command(wm, w, command_button);
        wm.win_set_status(w, win_status::USE_OVERLAY_STATES);
    }

    // set the text description of what is building
    update_construction_text_display(wm, h, obj);

    // set the portrait for the thing being constructed
    h.set_portrait_by_object(obj);

    // and show the rally point, if it should have one
    let _had_exit = h.show_rally_point_of_exit(obj);
}

/// `ControlBar::updateContextUnderConstruction()` (ControlBarUnderConstruction.cpp:104).
pub fn update_context_under_construction<H: UnderConstructionHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(obj) = h.current_selected_object() else { return };

    // if the object is no longer under construction switch to a new appropriate context
    if !h.is_under_construction(obj) {
        h.evaluate_context_ui(wm);
        return;
    }

    // if the construction percent has changed since what was last shown to the user update the text
    if h.displayed_construct_percent() != h.construction_percent(obj) {
        update_construction_text_display(wm, h, obj);
    }
}
