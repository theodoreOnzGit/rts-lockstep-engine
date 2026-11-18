//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarStructureInventory.cpp
//!     (`PopulateButtonInfo`, `ControlBar::populateButtonProc`, `populateStructureInventory`,
//!      `updateContextStructureInventory`)

use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::client::window_script::{win_status, WinId};

/// `STOP_ID` (ControlBarStructureInventory.cpp:57).
pub const STOP_ID: usize = 10;
/// `EVACUATE_ID`.
pub const EVACUATE_ID: usize = 11;
/// `MAX_COMMANDS_PER_SET` (CommandSet.h).
pub const MAX_COMMANDS_PER_SET: usize = 18;
/// `MAX_STRUCTURE_INVENTORY_BUTTONS` (ControlBar.h).
pub const MAX_STRUCTURE_INVENTORY_BUTTONS: usize = 10;

/// One entry of `ControlBar::m_containData`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContainEntry {
    /// `control`.
    pub control: Option<WinId>,
    /// `objectID` (`INVALID_ID` is 0).
    pub object_id: u32,
}

/// `struct PopulateButtonInfo` (ControlBarStructureInventory.cpp:62): the `source` is only used by the sanity message.
#[derive(Clone, Debug)]
pub struct PopulateButtonInfo {
    pub source: u32,
    pub button_index: usize,
    pub inventory_buttons: Vec<Option<WinId>>,
}

/// What the structure inventory reads from the game.
pub trait StructureInventoryHost: GuiHost {
    /// `m_commandWindows[i]`.
    fn command_window(&self, i: usize) -> Option<WinId>;
    /// `resetContainData()`.
    fn reset_contain_data(&mut self);
    /// `m_containData[i] = entry`.
    fn set_contain_data(&mut self, i: usize, entry: ContainEntry);
    /// `TheHotKeyManager->reset()` (nothing without `TheHotKeyManager`).
    fn hot_key_manager_reset(&mut self);
    fn find_command_button(&self, name: &[u8]) -> Option<usize>;
    fn set_control_command(&mut self, wm: &mut GameWindowManager, w: WinId, command: Option<usize>);
    /// `GadgetButtonSetEnabledImage(w, obj->getTemplate()->getButtonImage())`.
    fn gadget_button_set_enabled_image_of_template(&mut self, wm: &mut GameWindowManager, w: WinId, obj: u32);
    /// `GadgetButtonDrawOverlayImage(w, calculateVeterancyOverlayForObject(obj))`; `None` clears it.
    fn gadget_button_draw_overlay_for_object(&mut self, wm: &mut GameWindowManager, w: WinId, obj: Option<u32>);
    /// `building->getContain()`: false when the object has no contain module.
    fn has_contain(&self, building: u32) -> bool;
    /// `contain->getContainMax()`, `getContainCount()`.
    fn contain_max(&self, building: u32) -> i32;
    fn contain_count(&self, building: u32) -> i32;
    /// `contain->iterateContained(proc, &info, FALSE)`: the contained objects in the order upstream iterates them
    /// (reverse order, "we're iterating in reverse order here").
    fn iterate_contained(&self, building: u32) -> Vec<u32>;
    /// `obj->getID()`.
    fn object_id(&self, obj: u32) -> u32;
    /// `m_lastRecordedInventoryCount`.
    fn last_recorded_inventory_count(&self) -> i32;
    fn set_last_recorded_inventory_count(&mut self, n: i32);
    /// `m_currentSelectedDrawable->getObject()`.
    fn current_selected_object(&self) -> Option<u32>;
    /// `source->isLocallyControlled()`.
    fn is_locally_controlled(&self, obj: u32) -> bool;
    /// `localPlayer->getRelationship(source->getTeam()) != NEUTRAL`.
    fn local_relationship_to_object_not_neutral(&self, obj: u32) -> bool;
    /// `TheInGameUI->deselectDrawable(source->getDrawable())` (nothing without a drawable).
    fn deselect_drawable_of_object(&mut self, obj: u32);
}

/// `ControlBar::populateButtonProc(obj, userData)` (ControlBarStructureInventory.cpp:73).
pub fn populate_button_proc<H: StructureInventoryHost>(wm: &mut GameWindowManager, h: &mut H, obj: u32, info: &mut PopulateButtonInfo) {
    // (upstream's DEBUG_ASSERTCRASH is compiled out: an index past the buttons is out of bounds upstream)
    let Some(Some(button)) = info.inventory_buttons.get(info.button_index).copied() else {
        info.button_index += 1;
        return;
    };

    // put object in inventory data
    let object_id = h.object_id(obj);
    h.set_contain_data(info.button_index, ContainEntry { control: Some(button), object_id });

    // set the UI button that will allow us to press it and cause the object to exit the container
    h.gadget_button_set_enabled_image_of_template(wm, button, obj);

    // Show the auto-contained object's veterancy symbol!
    h.gadget_button_draw_overlay_for_object(wm, button, Some(obj));

    // Enable the button
    wm.win_enable(button, true);

    // move to the next button index
    info.button_index += 1;
}

/// `ControlBar::populateStructureInventory(building)` (ControlBarStructureInventory.cpp:102).
pub fn populate_structure_inventory<H: StructureInventoryHost>(wm: &mut GameWindowManager, h: &mut H, building: u32) {
    // reset the inventory data
    h.reset_contain_data();

    // reset hotkeys -- the hotkeys are built on the fly as guys enter and leave
    h.hot_key_manager_reset();

    // Start by hiding all the buttons.
    for command_index in 0..MAX_COMMANDS_PER_SET {
        if let Some(w) = h.command_window(command_index) {
            wm.win_hide(w, true);
        }
    }

    // get the contain module of the object
    if !h.has_contain(building) {
        return;
    }

    let evacuate_command = h.find_command_button(b"Command_Evacuate");
    if let Some(w) = h.command_window(EVACUATE_ID) {
        h.set_control_command(wm, w, evacuate_command);
        wm.win_enable(w, false);
    }

    let stop_command = h.find_command_button(b"Command_Stop");
    if let Some(w) = h.command_window(STOP_ID) {
        h.set_control_command(wm, w, stop_command);
        wm.win_enable(w, false);
    }

    // get the inventory exit command to assign into the button
    let exit_command = h.find_command_button(b"Command_StructureExit");

    // get window handles for each of the inventory buttons
    for i in 0..MAX_STRUCTURE_INVENTORY_BUTTONS {
        let Some(w) = h.command_window(i) else { continue };
        // show the window
        wm.win_hide(w, false);

        // disable the button for now, it will be enabled if there is something there for its contents
        wm.win_enable(w, false);
        wm.win_set_status(w, win_status::ALWAYS_COLOR);
        wm.win_clear_status(w, win_status::NOT_READY);

        // set an inventory command into the game window UI element
        h.set_control_command(wm, w, exit_command);

        // Clear any veterancy icon incase the unit leaves!
        h.gadget_button_draw_overlay_for_object(wm, w, None);

        // if the structure can hold a lesser amount inside it than what the GUI displays we will completely hide the
        // buttons that can't contain anything
        if (i as i32) + 1 > h.contain_max(building) {
            wm.win_hide(w, true);
        }
    }

    // show the window
    if let Some(w) = h.command_window(EVACUATE_ID) {
        wm.win_hide(w, false);
    }
    if let Some(w) = h.command_window(STOP_ID) {
        wm.win_hide(w, false);
    }

    // if there is at least one item in there enable the evacuate and stop buttons
    if h.contain_count(building) != 0 {
        if let Some(w) = h.command_window(EVACUATE_ID) {
            wm.win_enable(w, true);
        }
        if let Some(w) = h.command_window(STOP_ID) {
            wm.win_enable(w, true);
        }
    }

    // iterate each of the objects inside the container and put them in a button
    let mut info = PopulateButtonInfo {
        source: building,
        button_index: 0,
        inventory_buttons: (0..MAX_COMMANDS_PER_SET).map(|i| h.command_window(i)).collect(),
    };
    for obj in h.iterate_contained(building) {
        populate_button_proc(wm, h, obj, &mut info);
    }

    // save how many items were contained by the object at this time
    let count = h.contain_count(building);
    h.set_last_recorded_inventory_count(count);
}

/// `ControlBar::updateContextStructureInventory()` (ControlBarStructureInventory.cpp:207).
pub fn update_context_structure_inventory<H: StructureInventoryHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(source) = h.current_selected_object() else { return };

    // it is possible that we had a building selected that can be garrisoned, and while it was selected the enemy
    // occupied it: unselect the building so that we can't see the contents
    if !h.is_locally_controlled(source) && h.local_relationship_to_object_not_neutral(source) {
        h.deselect_drawable_of_object(source);
        return;
    }

    // if the object being displayed has a different count than we last knew about, repopulate the buttons
    if !h.has_contain(source) {
        return;
    }
    if h.last_recorded_inventory_count() != h.contain_count(source) {
        populate_structure_inventory(wm, h, source);
    }
}
