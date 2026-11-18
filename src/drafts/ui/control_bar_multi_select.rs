//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarMultiSelect.cpp
//!     (`ControlBar::resetCommonCommandData`, `addCommonCommands`, `populateMultiSelect`, `updateContextMultiSelect`)

use crate::client::control_bar_command::{opt, CommandAvailability};
use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::client::window_script::win_status;

pub use super::control_bar_structure_inventory::MAX_COMMANDS_PER_SET;

/// `OK_FOR_MULTI_SELECT` (ControlBar.h CommandOption).
pub const OK_FOR_MULTI_SELECT: u32 = 0x0000_0100;

/// What multi select reads from the game. A command button is an index the host knows; a drawable is an id.
pub trait MultiSelectHost: GuiHost {
    fn command_window(&self, i: usize) -> Option<usize>;
    /// `m_commonCommands[i]`.
    fn common_command(&self, i: usize) -> Option<usize>;
    fn set_common_command(&mut self, i: usize, c: Option<usize>);
    /// `GadgetButtonDrawOverlayImage(w, NULL)`.
    fn gadget_button_clear_overlay(&mut self, wm: &mut GameWindowManager, w: usize);
    fn set_control_command(&mut self, wm: &mut GameWindowManager, w: usize, command: Option<usize>);
    /// `draw->getObject()` (`None`: no object).
    fn drawable_object(&self, draw: u32) -> Option<u32>;
    /// `obj->isKindOf(KINDOF_IGNORED_IN_GUI)`.
    fn is_ignored_in_gui(&self, obj: u32) -> bool;
    /// `findCommandSet(obj->getCommandSetString())` then `commandSet->getCommandButton(i)`: `None` outer for no
    /// command set, else the 18 slots.
    fn command_set_buttons(&self, obj: u32) -> Option<Vec<Option<usize>>>;
    /// `command->getOptions()`, `command->getCommandType() == GUI_COMMAND_ATTACK_MOVE`.
    fn command_options(&self, c: usize) -> u32;
    fn command_is_attack_move(&self, c: usize) -> bool;
    /// `TheInGameUI->getAllSelectedDrawables()`, in list order.
    fn selected_drawables(&self) -> Vec<u32>;
    /// `obj->getStatusBits().test(OBJECT_STATUS_SOLD)`.
    fn is_sold(&self, obj: u32) -> bool;
    /// `draw->getTemplate()->getSelectedPortraitImage()`'s identity (the image pointer; 0 for `NULL`).
    fn drawable_portrait_image(&self, draw: u32) -> usize;
    fn set_portrait_by_object(&mut self, obj: Option<u32>);
    /// `GadgetButtonGetData(win)` (the command button).
    fn gadget_button_command(&self, wm: &GameWindowManager, w: usize) -> Option<usize>;
    /// `getCommandAvailability(command, obj, win)`.
    fn get_command_availability(&mut self, wm: &mut GameWindowManager, command: usize, obj: u32, w: usize) -> CommandAvailability;
    /// `GadgetCheckLikeButtonSetVisualCheck(win, checked)`.
    fn gadget_check_like_button_set_visual_check(&mut self, wm: &mut GameWindowManager, w: usize, checked: bool);
}

/// `ControlBar::resetCommonCommandData()` (ControlBarMultiSelect.cpp:55).
pub fn reset_common_command_data<H: MultiSelectHost>(wm: &mut GameWindowManager, h: &mut H) {
    for i in 0..MAX_COMMANDS_PER_SET {
        h.set_common_command(i, None);
        // Clear out any remnant overlays.
        if let Some(w) = h.command_window(i) {
            h.gadget_button_clear_overlay(wm, w);
        }
    }
}

/// `ControlBar::addCommonCommands(draw, firstDrawable)` (ControlBarMultiSelect.cpp:70).
pub fn add_common_commands<H: MultiSelectHost>(wm: &mut GameWindowManager, h: &mut H, draw: u32, first_drawable: bool) {
    let Some(obj) = h.drawable_object(draw) else { return };

    if h.is_ignored_in_gui(obj) {
        // ignore these guys
        return;
    }

    // get the command set of this drawable
    let Some(command_set) = h.command_set_buttons(obj) else {
        // if there is no command set for this drawable, none of the selected drawables can possibly have matching
        // commands so we'll get rid of them all
        for i in 0..MAX_COMMANDS_PER_SET {
            h.set_common_command(i, None);
            if let Some(w) = h.command_window(i) {
                wm.win_hide(w, true);
            }
        }
        return;
    };

    // easy case, if we're adding the first drawable we simply just add any of the commands in its set that can be
    // multi-select commands to the common command set
    if first_drawable {
        for i in 0..MAX_COMMANDS_PER_SET {
            // our implementation doesn't necessarily make use of the max possible command buttons
            let Some(w) = h.command_window(i) else { continue };

            let command = command_set.get(i).copied().flatten();

            // add if present and can be used in a multi select
            if let Some(c) = command {
                if h.command_options(c) & OK_FOR_MULTI_SELECT != 0 {
                    // put it in the common command set
                    h.set_common_command(i, Some(c));

                    // show and enable this control
                    wm.win_hide(w, false);
                    wm.win_enable(w, true);

                    // set the command into the control
                    h.set_control_command(wm, w, Some(c));
                }
            }
        }
    } else {
        // go through each command one by one
        for i in 0..MAX_COMMANDS_PER_SET {
            let Some(w) = h.command_window(i) else { continue };

            let command = command_set.get(i).copied().flatten();
            let common = h.common_command(i);

            let attack_move = command.map_or(false, |c| h.command_is_attack_move(c)) || common.map_or(false, |c| h.command_is_attack_move(c));

            // Kris: When any units have attack move, they all get it.
            if attack_move && common.is_none() {
                // put it in the common command set
                h.set_common_command(i, command);

                // show and enable this control
                wm.win_hide(w, false);
                wm.win_enable(w, true);

                // set the command into the control
                h.set_control_command(wm, w, command);
            } else if command != common && !attack_move {
                // if this command does not match the command that is in the common command set then *neither* this
                // command OR the command in the common command set are really common commands: remove the one
                // that has been stored in the common set
                h.set_common_command(i, None);

                // hide the window control cause it should have been made visible from a command that was placed in
                // this common 'slot' earlier
                wm.win_hide(w, true);
            }
        }
    }
}

/// `ControlBar::populateMultiSelect()` (ControlBarMultiSelect.cpp:200).
pub fn populate_multi_select<H: MultiSelectHost>(wm: &mut GameWindowManager, h: &mut H) {
    let mut first_drawable = true;
    let mut portrait_set = false;
    let mut portrait: usize = 0;
    let mut portrait_obj: Option<u32> = None;

    // first reset the common command data
    reset_common_command_data(wm, h);

    // by default, hide all the controls in the command section
    for i in 0..MAX_COMMANDS_PER_SET {
        if let Some(w) = h.command_window(i) {
            wm.win_hide(w, true);
        }
    }

    // get the list of drawables from the in game UI
    for draw in h.selected_drawables() {
        let Some(obj0) = h.drawable_object(draw) else { continue };
        if h.is_ignored_in_gui(obj0) {
            // ignore these guys
            continue;
        }

        // add command for this drawable; we're not considering objects that are currently in the process of being
        // sold as those objects can't be issued anymore commands
        if !h.is_sold(obj0) {
            // add the common commands of this drawable to the common command set
            add_common_commands(wm, h, draw, first_drawable);

            // not adding the first drawble anymore
            first_drawable = false;

            // keep track of the portrait images, if all units selected have the same portrait we will display it in
            // the right HUD, otherwise we won't
            if !portrait_set {
                portrait = h.drawable_portrait_image(draw);
                portrait_obj = Some(obj0);
                portrait_set = true;
            } else if h.drawable_portrait_image(draw) != portrait {
                portrait = 0;
            }
        }
    }
    let _ = portrait; // upstream keeps `portrait` but only `portraitObj` reaches setPortraitByObject

    // set the portrait image
    h.set_portrait_by_object(portrait_obj);
}

/// `ControlBar::updateContextMultiSelect()` (ControlBarMultiSelect.cpp:316).
pub fn update_context_multi_select<H: MultiSelectHost>(wm: &mut GameWindowManager, h: &mut H) {
    // zero the array that counts how many objects can do each command
    let mut objects_that_can_do_command = [0i32; MAX_COMMANDS_PER_SET];

    // loop through all the selected drawables
    for draw in h.selected_drawables() {
        let Some(obj) = h.drawable_object(draw) else { continue };
        if h.is_ignored_in_gui(obj) {
            // ignore these guys
            continue;
        }

        // for each of the visible command windows make sure the object can execute the command
        for i in 0..MAX_COMMANDS_PER_SET {
            // get the control window
            let Some(win) = h.command_window(i) else { continue };

            // don't consider hidden windows
            if wm.is_hidden(Some(win)) {
                continue;
            }

            // get the command
            let Some(command) = h.gadget_button_command(wm, win) else { continue };

            // can we do the command
            let availability = h.get_command_availability(wm, command, obj, win);

            wm.win_clear_status(win, win_status::NOT_READY);
            wm.win_clear_status(win, win_status::ALWAYS_COLOR);

            // enable/disable the window control
            match availability {
                CommandAvailability::Hidden => wm.win_hide(win, true),
                CommandAvailability::Restricted => wm.win_enable(win, false),
                CommandAvailability::NotReady => {
                    wm.win_enable(win, false);
                    wm.win_set_status(win, win_status::NOT_READY);
                }
                CommandAvailability::CantAfford => {
                    wm.win_enable(win, false);
                    wm.win_set_status(win, win_status::ALWAYS_COLOR);
                }
                _ => wm.win_enable(win, true),
            }

            // If button is a CHECK_LIKE, then update it's status now.
            if h.command_options(command) & opt::CHECK_LIKE != 0 {
                h.gadget_check_like_button_set_visual_check(wm, win, availability == CommandAvailability::Active);
            }

            if availability == CommandAvailability::Available || availability == CommandAvailability::Active {
                objects_that_can_do_command[i] += 1;
            }
        }
    }

    // for each command, if any objects can do the command we enable the window, otherwise we disable it
    for i in 0..MAX_COMMANDS_PER_SET {
        let Some(w) = h.command_window(i) else { continue };

        // don't consider hidden commands
        if wm.is_hidden(Some(w)) {
            continue;
        }

        // don't consider slots that don't have commands
        if h.common_command(i).is_none() {
            continue;
        }

        // check the count of objects that can do the command and enable/disable the control
        wm.win_enable(w, objects_that_can_do_command[i] > 0);
    }
}
