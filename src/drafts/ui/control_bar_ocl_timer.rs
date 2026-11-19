//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 sequences (test: tests/differential_draft_ctl_timers.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarOCLTimer.cpp
//!     (`ControlBar::updateOCLTimerTextDisplay`, `populateOCLTimer`, `updateContextOCLTimer`)

use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::client::window_script::win_status;
use crate::logic::modules::plant;

/// `LOGICFRAMES_PER_SECOND` (GameCommon.h).
pub const LOGICFRAMES_PER_SECOND: u32 = 30;

/// What the OCL timer context reads from the game.
pub trait OclTimerHost: GuiHost {
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `m_contextParent[CP_OCL_TIMER]`.
    fn context_parent_ocl_timer(&self) -> Option<usize>;
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `UnicodeString::format(fmt, minutes, seconds)` with the two `%d`s.
    fn format_minutes_seconds(&self, fmt: &[u16], minutes: i32, seconds: i32) -> Vec<u16>;
    /// `GadgetStaticTextSetText(w, text)`.
    fn gadget_static_text_set_text(&mut self, wm: &mut GameWindowManager, w: usize, text: &[u16]);
    /// `GadgetProgressBarSetProgress(w, progress)`.
    fn gadget_progress_bar_set_progress(&mut self, wm: &mut GameWindowManager, w: usize, progress: i32);
    /// `findCommandButton(name)` (an index the host knows).
    fn find_command_button(&self, name: &[u8]) -> Option<usize>;
    /// `ControlBar::setControlCommand(win, commandButton)`.
    fn set_control_command(&mut self, wm: &mut GameWindowManager, w: usize, command: Option<usize>);
    /// `creatorObject->isKindOf(KINDOF_TECH_BUILDING)`, `KINDOF_AUTO_RALLYPOINT`.
    fn is_tech_building(&self, obj: u32) -> bool;
    fn is_auto_rallypoint(&self, obj: u32) -> bool;
    /// `creatorObject->getObjectExitInterface()` then `showRallyPoint(exit->getRallyPoint())`: false when the object
    /// has no exit interface.
    fn show_rally_point_of_exit(&mut self, obj: u32) -> bool;
    fn set_portrait_by_object(&mut self, obj: u32);
    /// The selected drawable's object, `m_currentSelectedDrawable->getObject()`.
    fn current_selected_object(&self) -> Option<u32>;
    /// `OCLUpdate::getRemainingFrames()` and `getCountdownPercent()` of the object's `OCLUpdate` module.
    fn ocl_update_remaining_frames(&self, obj: u32) -> u32;
    fn ocl_update_countdown_percent(&self, obj: u32) -> f32;
    /// `m_displayedOCLTimerSeconds`.
    fn displayed_ocl_timer_seconds(&self) -> u32;
    fn set_displayed_ocl_timer_seconds(&mut self, s: u32);
}

/// `ControlBar::updateOCLTimerTextDisplay(totalSeconds, percent)` (ControlBarOCLTimer.cpp:50).
pub fn update_ocl_timer_text_display<H: OclTimerHost>(wm: &mut GameWindowManager, h: &mut H, total_seconds: u32, percent: f32) {
    let desc_id = h.name_to_key(b"ControlBar.wnd:OCLTimerStaticText");
    let desc_window = wm.win_get_window_from_id(None, desc_id);
    let bar_id = h.name_to_key(b"ControlBar.wnd:OCLTimerProgressBar");
    let bar_window = wm.win_get_window_from_id(None, bar_id);

    let minutes = (total_seconds / 60) as i32;
    let seconds = (total_seconds as i32).wrapping_sub(minutes.wrapping_mul(60));

    // format the message
    let text = if (seconds < 10 && !plant(12661)) || (seconds <= 10 && plant(12661)) {
        let fmt = h.fetch_text(b"CONTROLBAR:OCLTimerDescWithPadding");
        h.format_minutes_seconds(&fmt, minutes, seconds)
    } else {
        let fmt = h.fetch_text(b"CONTROLBAR:OCLTimerDesc");
        h.format_minutes_seconds(&fmt, minutes, seconds)
    };

    if let Some(d) = desc_window {
        h.gadget_static_text_set_text(wm, d, &text);
    }
    if let Some(b) = bar_window {
        // `(percent * 100)` converted to Int by the call
        let progress = if plant(12662) {
            (percent * 100.0 + 0.5) as i32
        } else if plant(12663) {
            (f64::from(percent) * 100.0) as i32
        } else {
            (percent * 100.0) as i32
        };
        h.gadget_progress_bar_set_progress(wm, b, progress);
    }

    // record this as the last time displayed
    if !plant(12668) {
        h.set_displayed_ocl_timer_seconds(total_seconds);
    }
}

/// `ControlBar::populateOCLTimer(creatorObject)` (ControlBarOCLTimer.cpp:84).
pub fn populate_ocl_timer<H: OclTimerHost>(wm: &mut GameWindowManager, h: &mut H, creator_object: Option<u32>) {
    // sanity
    let Some(creator) = creator_object else { return };

    // get our parent window
    let parent = h.context_parent_ocl_timer();

    // set the sell button
    let id = h.name_to_key(b"ControlBar.wnd:OCLTimerSellButton");
    let win = wm.win_get_window_from_id(parent, id);

    if !h.is_tech_building(creator) && !(plant(12666) && h.is_auto_rallypoint(creator)) {
        // Surgical bug fix.
        let command_button = h.find_command_button(if plant(12667) { b"Command_SetRallyPoint" } else { b"Command_Sell" });
        if let Some(w) = win {
            h.set_control_command(wm, w, command_button);
            wm.win_set_status(w, win_status::USE_OVERLAY_STATES);
        }
    }
    // Another last minute hack due to time constraint and feature creep
    else if h.is_tech_building(creator) && (h.is_auto_rallypoint(creator) != plant(12669)) {
        // This time we want a rally point button to show up instead of a sell button
        let command_button = h.find_command_button(b"Command_SetRallyPoint");
        if let Some(w) = win {
            h.set_control_command(wm, w, command_button);
            wm.win_set_status(w, win_status::USE_OVERLAY_STATES);
        }
        // for objects that have a production exit interface, we may have a rally point set; show it in the world
        if !plant(12670) {
            let _had_exit = h.show_rally_point_of_exit(creator);
        }
    } else if let Some(w) = win {
        wm.win_hide(w, true);
    }

    // set the text percent and bar of our timer we are displaying
    if !plant(12677) {
        update_context_ocl_timer(wm, h);
    }

    // set the portrait for the thing being constructed
    h.set_portrait_by_object(creator);
    if plant(12677) {
        update_context_ocl_timer(wm, h);
    }
}

/// `ControlBar::updateContextOCLTimer()` (ControlBarOCLTimer.cpp:140).
pub fn update_context_ocl_timer<H: OclTimerHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(obj) = h.current_selected_object() else { return };
    let frames = h.ocl_update_remaining_frames(obj);
    let seconds = if plant(12664) { frames.div_ceil(LOGICFRAMES_PER_SECOND) } else { frames / LOGICFRAMES_PER_SECOND };
    let percent = h.ocl_update_countdown_percent(obj);

    // if the time has changed since what was last shown to the user update the text
    if (h.displayed_ocl_timer_seconds() != seconds && !plant(12665)) || (plant(12665) && h.displayed_ocl_timer_seconds() < seconds) {
        update_ocl_timer_text_display(wm, h, seconds, percent);
    }
}
