// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarPopupDescription.cpp (its file statics,
//     ControlBarPopupDescriptionUpdateFunc, ControlBar::showBuildTooltipLayout, repopulateBuildTooltipLayout,
//     hideBuildTooltipLayout, deleteBuildTooltipLayout)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (commandButtonTooltip; ControlBar::update's
//     popup part; ControlBar::init's and setControlCommand's winSetTooltipFunc calls; #144: ControlBar::init's tooltip part
//     (lines 1190-1240), setControlCommand and setCommandBarBorder)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #138: when the control bar's popup description (`ControlBarPopupDescription.wnd`, `m_buildToolTipLayout`)
//! shows and hides: ControlBar's tooltip path.
//!
//! # As upstream does it
//!
//! - **Through the mouse's events.** `ControlBar::init` and `setControlCommand` give the bar's buttons (a command
//!   button whose command has a text label or sciences; the options, idle worker, beacon, general's and up/down buttons;
//!   the power, money and general's experience windows) the tooltip callback `commandButtonTooltip`
//!   ([`COMMAND_BUTTON_TOOLTIP`]). `GameWindowManager::winProcessMouseEvent` clears `TheMouse`'s tooltip at every mouse
//!   event (:851) and, for the window under the mouse, calls its callback **instead of** setting `TheMouse`'s tooltip
//!   (:1222; `crate::client::window_manager`). So a bar button's description never goes into `TheMouse`'s tooltip: the
//!   one mouse's tooltip stays clear over it, and the callback drives the popup ([`command_button_tooltip`]).
//! - **`showBuildTooltipLayout(cmdButton)`** ([`BuildTooltip::show_build_tooltip_layout`]): nothing while the tooltips
//!   are disabled or the game is ending. The first call over a window remembers it (`prevWindow`) and the time
//!   (`timeGetTime()`). Each later call over the same window marks the popup wanted (`m_showBuildToolTipLayout`) and,
//!   once the window's tooltip delay has passed (`beginWaitTime + getTooltipDelay() < timeGetTime()`, unsigned) and the
//!   popup was not already filled for it (`isInitialized`), fills it and shows it: a push button from its command (none:
//!   nothing; not in a replay, the quit menu or the disconnect screen), a user window or static text from the window. A
//!   call over another window while the popup shows hides it and forgets the window.
//! - **Every frame** (`ControlBar::update`, [`BuildTooltip::update`]): while the popup shows, its layout's update
//!   (`ControlBarPopupDescriptionUpdateFunc`, [`BuildTooltip::popup_update`]) hides it when the game is ending or when no
//!   call marked it wanted since the last frame; then the mark is cleared. So the popup stays while the mouse's events
//!   keep reaching the button and goes the frame after they stop.
//! - **Which windows** (GitHub #144, [`control_bar_init_tooltips`]): `ControlBar::init` gives the non-command push
//!   buttons (`PopupCommunicator`, `ButtonOptions`, `ButtonIdleWorker`, `ButtonPlaceBeacon`, `ButtonGeneral`,
//!   `ButtonLarge`) their `NonCommand_*` command buttons (`setControlCommand(win, findCommandButton(...))`,
//!   [`set_control_command`]) and `commandButtonTooltip`; the power, money and general's experience windows get
//!   `commandButtonTooltip` alone, and their popups are `populateBuildTooltipLayout(NULL, window)`'s
//!   (`crate::client::control_bar_popup::populate_window_tooltip`).
//! - `hideBuildTooltipLayout` and `deleteBuildTooltipLayout` ([`BuildTooltip::delete_build_tooltip_layout`]: the mark
//!   cleared, the window forgotten, the layout hidden); `repopulateBuildTooltipLayout` (the stages' default config) refills
//!   it from the remembered push button.
//!
//! # Where this differs from upstream
//!
//! - **The animation is dead code upstream and not translated:** `useAnimation` is a file static set to `FALSE`
//!   (ControlBarPopupDescription.cpp:106) and never assigned, so `theAnimateWindowManager` is never made; every branch
//!   that needs one is unreachable. `TheGlobalData->m_animateWindows` is read only in a condition with `!useAnimation`,
//!   which is always true.
//! - `showBuildTooltipLayout`'s two function statics (`isInitialized`, `beginWaitTime`) and the file static `prevWindow`
//!   are members of [`BuildTooltip`]; upstream has one of each per process, the client one bar.
//! - The layout, the windows, the command buttons, the clock and the menus are the host's ([`BuildTooltipHost`]);
//!   `populateBuildTooltipLayout` is `crate::client::control_bar_popup::populate_build_tooltip_layout`, which the host
//!   calls ([`BuildTooltipHost::populate_build_tooltip_layout`]).

use crate::client::window_script::{gws, WinId};
use crate::logic::modules::plant;

/// The name the bar's windows give their tooltip callback: ControlBar.cpp's `commandButtonTooltip` (`winSetTooltipFunc`).
pub const COMMAND_BUTTON_TOOLTIP: &[u8] = b"commandButtonTooltip";

/// What the popup's timing reaches outside `TheControlBar`.
pub trait BuildTooltipHost {
    /// `TheInGameUI->areTooltipsDisabled()`.
    fn tooltips_disabled(&self) -> bool;
    /// `TheScriptEngine->isGameEnding()`.
    fn is_game_ending(&self) -> bool;
    /// `timeGetTime()`, milliseconds.
    fn time_get_time(&self) -> u32;
    /// `window->winGetStyle()`.
    fn win_style(&self, w: WinId) -> u32;
    /// `window->getTooltipDelay()` (`m_instData.m_tooltipDelay`).
    fn tooltip_delay(&self, w: WinId) -> i32;
    /// `(const CommandButton *)GadgetButtonGetData(window)`, as an index the host knows; `None` for `NULL`.
    fn gadget_button_command(&self, w: WinId) -> Option<usize>;
    /// `TheGameLogic->isInReplayGame()`.
    fn is_in_replay_game(&self) -> bool;
    /// `TheInGameUI->isQuitMenuVisible()`.
    fn is_quit_menu_visible(&self) -> bool;
    /// `TheDisconnectMenu && TheDisconnectMenu->isScreenVisible()`.
    fn disconnect_menu_visible(&self) -> bool;
    /// `populateBuildTooltipLayout(commandButton, tooltipWin)`.
    fn populate_build_tooltip_layout(&mut self, command: Option<usize>, tooltip_win: Option<WinId>);
    /// `m_buildToolTipLayout->isHidden()`.
    fn layout_is_hidden(&self) -> bool;
    /// `m_buildToolTipLayout->hide(hide)`.
    fn layout_hide(&mut self, hide: bool);
}

/// `TheControlBar`'s popup state and ControlBarPopupDescription.cpp's statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuildTooltip {
    /// `m_showBuildToolTipLayout` (`getShowBuildTooltipLayout()`).
    pub show_build_tooltip_layout: bool,
    /// The file static `prevWindow`: the window the mouse's calls are about.
    pub prev_window: Option<WinId>,
    /// `showBuildTooltipLayout`'s static `isInitialized`: the popup was filled for `prev_window`.
    pub is_initialized: bool,
    /// `showBuildTooltipLayout`'s static `beginWaitTime`.
    pub begin_wait_time: u32,
}

impl BuildTooltip {
    /// `ControlBar::showBuildTooltipLayout(cmdButton)` (ControlBarPopupDescription.cpp:133). See the module docs.
    pub fn show_build_tooltip_layout(&mut self, cmd_button: Option<WinId>, h: &mut impl BuildTooltipHost) {
        if (h.tooltips_disabled() && !plant(9286)) || (h.is_game_ending() && !plant(9287)) {
            return;
        }
        let mut passed_wait_time = false;
        if self.prev_window == cmd_button {
            if !plant(9288) {
                self.show_build_tooltip_layout = true;
            }
            if let Some(b) = cmd_button {
                let due = self.begin_wait_time.wrapping_add(h.tooltip_delay(b) as u32);
                let now = h.time_get_time();
                let elapsed = if plant(9289) { due <= now } else { due < now };
                if (!self.is_initialized || plant(9290)) && elapsed {
                    passed_wait_time = true;
                }
            }
            if !passed_wait_time {
                return;
            }
        } else if !h.layout_is_hidden() {
            // (the animation's two branches: `useAnimation` is FALSE)
            if !plant(9291) {
                h.layout_hide(true);
            }
            if !plant(9292) {
                self.prev_window = None;
            }
            return;
        }
        // "will only get here the firsttime through the function through this window"
        if !passed_wait_time {
            self.prev_window = cmd_button;
            self.begin_wait_time = h.time_get_time();
            self.is_initialized = false;
            return;
        }
        self.is_initialized = true;
        let Some(cmd_button) = cmd_button else { return };
        let style = h.win_style(cmd_button);
        if style & gws::PUSH_BUTTON != 0 {
            let command = h.gadget_button_command(cmd_button);
            if command.is_none() && !plant(9293) {
                return;
            }
            // "note that, in this branch, ENABLE_SOLO_PLAY is ***NEVER*** defined..."
            if h.is_in_replay_game() && !plant(9294) {
                return;
            }
            if h.is_quit_menu_visible() && !plant(9295) {
                return;
            }
            if h.disconnect_menu_visible() {
                return;
            }
            self.show_build_tooltip_layout = true;
            h.populate_build_tooltip_layout(command, None);
        } else {
            // "we're a generic window"
            if style & gws::USER_WINDOW == 0 && style & gws::STATIC_TEXT == 0 && !plant(9296) {
                return;
            }
            h.populate_build_tooltip_layout(None, Some(cmd_button));
        }
        h.layout_hide(false);
    }

    /// `ControlBar::repopulateBuildTooltipLayout()` (ControlBarPopupDescription.cpp:239): the remembered push button's
    /// command into the popup again (the layout always exists here: `m_buildToolTipLayout` is made by `ControlBar::init`).
    pub fn repopulate_build_tooltip_layout(&self, h: &mut impl BuildTooltipHost) {
        let Some(prev) = self.prev_window else { return };
        if h.win_style(prev) & gws::PUSH_BUTTON == 0 && !plant(9300) {
            return;
        }
        let command = h.gadget_button_command(prev);
        h.populate_build_tooltip_layout(command, None);
    }

    /// `ControlBarPopupDescriptionUpdateFunc(layout, param)` (ControlBarPopupDescription.cpp:107), the popup layout's
    /// update: hidden when the game is ending, and when nothing asked for it since the last frame (the animation's
    /// branches are dead: see the module docs).
    pub fn popup_update(&mut self, h: &mut impl BuildTooltipHost) {
        if h.is_game_ending() && !plant(9297) {
            self.hide_build_tooltip_layout(h);
        }
        // (`!TheGlobalData->m_animateWindows || !useAnimation`: always true)
        if !self.show_build_tooltip_layout {
            self.delete_build_tooltip_layout(h);
        }
    }

    /// `ControlBar::hideBuildTooltipLayout()` (ControlBarPopupDescription.cpp:690): with no animation,
    /// `deleteBuildTooltipLayout()`.
    pub fn hide_build_tooltip_layout(&mut self, h: &mut impl BuildTooltipHost) {
        self.delete_build_tooltip_layout(h);
    }

    /// `ControlBar::deleteBuildTooltipLayout()` (ControlBarPopupDescription.cpp:701): the mark cleared, the window
    /// forgotten, the layout hidden.
    pub fn delete_build_tooltip_layout(&mut self, h: &mut impl BuildTooltipHost) {
        self.show_build_tooltip_layout = false;
        if !plant(9298) {
            self.prev_window = None;
        }
        h.layout_hide(true);
    }

    /// `ControlBar::update`'s popup part (ControlBar.cpp:1420): while the popup shows, its layout's update
    /// (`runUpdate`: [`Self::popup_update`]), then the mark cleared for the next frame's calls.
    pub fn update(&mut self, h: &mut impl BuildTooltipHost) {
        if !h.layout_is_hidden() {
            self.popup_update(h);
            if !plant(9299) {
                self.show_build_tooltip_layout = false;
            }
        }
    }
}

/// ControlBar.cpp:130, `commandButtonTooltip(window, instData, mouse)`: the window manager's tooltip callback for the
/// bar's buttons (see [`COMMAND_BUTTON_TOOLTIP`]): `TheControlBar->showBuildTooltipLayout(window)`.
pub fn command_button_tooltip(bar: &mut BuildTooltip, window: WinId, h: &mut impl BuildTooltipHost) {
    bar.show_build_tooltip_layout(Some(window), h);
}

/// `ControlBar::setControlCommand`'s tooltip (ControlBar.cpp:2483): a command whose text label is not empty, or which
/// has sciences, gives its button `commandButtonTooltip`; otherwise the button's text is cleared and its tooltip
/// callback left as it was. Whether the button gets the callback.
pub fn command_gets_tooltip(text_label_empty: bool, has_sciences: bool) -> bool {
    !text_label_empty || has_sciences
}

/// ControlBar.cpp:1190-1224, `ControlBar::init`: the non-command push buttons and the command button each is given
/// (`findCommandButton`), in order. The communicator's window is `m_communicatorButton`.
pub const NON_COMMAND_BUTTONS: [(&[u8], &[u8]); 6] = [
    (b"ControlBar.wnd:PopupCommunicator", b"NonCommand_Communicator"),
    (b"ControlBar.wnd:ButtonOptions", b"NonCommand_Options"),
    (b"ControlBar.wnd:ButtonIdleWorker", b"NonCommand_IdleWorker"),
    (b"ControlBar.wnd:ButtonPlaceBeacon", b"NonCommand_Beacon"),
    (b"ControlBar.wnd:ButtonGeneral", b"NonCommand_GeneralsExperience"),
    (b"ControlBar.wnd:ButtonLarge", b"NonCommand_UpDown"),
];

/// ControlBar.cpp:1226-1240, `ControlBar::init`: the windows given `commandButtonTooltip` alone, in order (their popups are
/// `populateBuildTooltipLayout(NULL, window)`'s: `crate::client::control_bar_popup::populate_window_tooltip`).
pub const TOOLTIP_ONLY_WINDOWS: [&[u8]; 3] = [b"ControlBar.wnd:PowerWindow", b"ControlBar.wnd:MoneyDisplay", b"ControlBar.wnd:GeneralsExp"];

/// `CommandButtonMappedBorderType` (ControlBar.h): `COMMAND_BUTTON_BORDER_NONE` .. `_SYSTEM`.
pub mod border {
    pub const NONE: i32 = 0;
    pub const BUILD: i32 = 1;
    pub const UPGRADE: i32 = 2;
    pub const ACTION: i32 = 3;
    pub const SYSTEM: i32 = 4;
}

/// `GAME_COLOR_UNDEFINED` (Color.h): what `ControlBar::ControlBar` gives the four border colours
/// (`updateCommanBarBorderColors`), until a scheme's `init` sets them.
pub const GAME_COLOR_UNDEFINED: i32 = 0x00FF_FFFF;

/// What `ControlBar::init`'s tooltip part and `setControlCommand` reach on the window manager and the command buttons
/// (GitHub #144). A command button is an index the host knows (as [`BuildTooltipHost::gadget_button_command`]'s).
pub trait ControlBarInitHost {
    /// `TheWindowManager->winGetWindowFromId(NULL, TheNameKeyGenerator->nameToKey(name))`.
    fn win_get_window_from_id(&self, name: &[u8]) -> Option<WinId>;
    /// `findCommandButton(name)` (`None`: `NULL`).
    fn find_command_button(&self, name: &[u8]) -> Option<usize>;
    /// `window->winSetTooltipFunc(commandButtonTooltip)` ([`COMMAND_BUTTON_TOOLTIP`]).
    fn win_set_tooltip_func(&mut self, w: WinId);
    /// `button->winGetInputFunc() == GadgetPushButtonInput`.
    fn is_push_button_input(&self, w: WinId) -> bool;
    /// The command's `getOptions()`.
    fn command_options(&self, c: usize) -> u32;
    /// The command's `getButtonImage()`: the mapped image's name (`None`: `NULL`, no name or no such image).
    fn command_button_image(&self, c: usize) -> Option<Vec<u8>>;
    /// The command's `getTextLabel()`.
    fn command_text_label(&self, c: usize) -> Vec<u8>;
    /// `!getScienceVec().empty()`.
    fn command_has_sciences(&self, c: usize) -> bool;
    /// `getCommandButtonMappedBorderType()` ([`border`]).
    fn command_border(&self, c: usize) -> i32;
    /// `GadgetButtonEnableCheckLike(button, checkLike, checked)`.
    fn gadget_button_enable_check_like(&mut self, w: WinId, check_like: bool, checked: bool);
    /// `GadgetButtonSetEnabledImage(button, image)`.
    fn gadget_button_set_enabled_image(&mut self, w: WinId, image: Vec<u8>);
    /// `GadgetButtonSetText(button, UnicodeString(L""))`.
    fn gadget_button_clear_text(&mut self, w: WinId);
    /// `GadgetButtonSetData(button, (void*)commandButton)`.
    fn gadget_button_set_data(&mut self, w: WinId, c: usize);
    /// `GadgetButtonSetBorder(button, color, drawBorder)`.
    fn gadget_button_set_border(&mut self, w: WinId, color: i32, draw: bool);
    /// `m_commandButtonBorderBuildColor`, `..UpgradeColor`, `..ActionColor`, `..SystemColor`.
    fn border_colors(&self) -> [i32; 4];
    /// `TheHotKeyManager->searchHotKey(label)`; `None` without `TheHotKeyManager`.
    fn search_hot_key(&mut self, label: &[u8]) -> Option<Vec<u8>>;
    /// `TheHotKeyManager->addHotKey(button, hotKey)`.
    fn add_hot_key(&mut self, w: WinId, key: Vec<u8>);
    /// `GadgetButtonSetAltSound(button, sound)`.
    fn gadget_button_set_alt_sound(&mut self, w: WinId, sound: &[u8]);
}

/// `ControlBar::setCommandBarBorder(button, type)` (ControlBar.cpp:2385): the border's colour by the command's type;
/// none for `COMMAND_BUTTON_BORDER_NONE` and any other value.
pub fn set_command_bar_border(button: WinId, kind: i32, h: &mut impl ControlBarInitHost) {
    let [build, upgrade, action, system] = h.border_colors();
    let (build, upgrade) = if plant(9350) { (upgrade, build) } else { (build, upgrade) };
    match kind {
        border::BUILD => h.gadget_button_set_border(button, build, true),
        border::UPGRADE => h.gadget_button_set_border(button, upgrade, true),
        border::ACTION => h.gadget_button_set_border(button, action, true),
        border::SYSTEM => h.gadget_button_set_border(button, system, true),
        _ => h.gadget_button_set_border(button, GAME_COLOR_UNDEFINED, plant(9351)),
    }
}

/// `ControlBar::setControlCommand(button, commandButton)` (ControlBar.cpp:2427): nothing for a window that is not a
/// push button, or for no command (both `DEBUG_ASSERTCRASH`, compiled out); else the check-like state from the
/// command's `CHECK_LIKE` option, the button image as the enabled image (when there is one), `commandButtonTooltip` for a
/// command with a text label or sciences (else the button's text cleared), the command as the button's data, the
/// border, the text label's hot key, and the `GUICommandBarClick` alternate sound.
pub fn set_control_command(button: WinId, command: Option<usize>, h: &mut impl ControlBarInitHost) {
    if !h.is_push_button_input(button) && !plant(9345) {
        return;
    }
    let Some(c) = command else {
        if plant(9346) {
            h.win_set_tooltip_func(button);
        }
        return;
    };
    let options = h.command_options(c);
    if options & crate::client::control_bar_command::opt::CHECK_LIKE != 0 && !plant(9347) {
        h.gadget_button_enable_check_like(button, true, false);
    } else {
        h.gadget_button_enable_check_like(button, false, false);
    }
    if let Some(image) = h.command_button_image(c) {
        h.gadget_button_set_enabled_image(button, image);
    }
    let label = h.command_text_label(c);
    let has_sciences = h.command_has_sciences(c) && !plant(9348);
    if command_gets_tooltip(label.is_empty(), has_sciences) {
        h.win_set_tooltip_func(button);
    } else {
        h.gadget_button_clear_text(button);
    }
    if !plant(9349) {
        h.gadget_button_set_data(button, c);
    }
    let kind = h.command_border(c);
    set_command_bar_border(button, kind, h);
    if let Some(key) = h.search_hot_key(&label) {
        if !key.is_empty() || plant(9352) {
            h.add_hot_key(button, key);
        }
    }
    if !plant(9353) {
        h.gadget_button_set_alt_sound(button, b"GUICommandBarClick");
    }
}

/// `ControlBar::init`'s tooltip callbacks (ControlBar.cpp:1190-1240), GitHub #144: each non-command push button
/// ([`NON_COMMAND_BUTTONS`]) gets its `NonCommand_*` command ([`set_control_command`] with `findCommandButton`'s) and
/// `commandButtonTooltip`; the power, money and general's experience windows ([`TOOLTIP_ONLY_WINDOWS`]) get
/// `commandButtonTooltip` alone. A window the layout does not have is skipped (`if(win)`); upstream reads
/// `m_communicatorButton` without that check (a layout without it would crash there), so the communicator is skipped
/// here too. The window manager then calls `commandButtonTooltip` over these windows instead of giving their text to
/// `TheMouse`'s tooltip.
pub fn control_bar_init_tooltips(h: &mut impl ControlBarInitHost) {
    for (i, (window, command)) in NON_COMMAND_BUTTONS.iter().enumerate() {
        let Some(win) = h.win_get_window_from_id(window) else { continue };
        let command = if plant(9342) && i == 5 { b"NonCommand_Options".as_slice() } else { command };
        let c = h.find_command_button(command);
        set_control_command(win, c, h);
        if !(plant(9341) && i == 0) {
            h.win_set_tooltip_func(win);
        }
    }
    for (i, window) in TOOLTIP_ONLY_WINDOWS.iter().enumerate() {
        if (plant(9343) && i == 0) || (plant(9344) && i == 2) {
            continue;
        }
        if let Some(win) = h.win_get_window_from_id(window) {
            h.win_set_tooltip_func(win);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct H {
        now: u32,
        hidden: bool,
        populated: Vec<(Option<usize>, Option<WinId>)>,
        ending: bool,
    }

    impl BuildTooltipHost for H {
        fn tooltips_disabled(&self) -> bool {
            false
        }
        fn is_game_ending(&self) -> bool {
            self.ending
        }
        fn time_get_time(&self) -> u32 {
            self.now
        }
        fn win_style(&self, w: WinId) -> u32 {
            if w == 1 { gws::PUSH_BUTTON } else { gws::USER_WINDOW }
        }
        fn tooltip_delay(&self, _w: WinId) -> i32 {
            100
        }
        fn gadget_button_command(&self, w: WinId) -> Option<usize> {
            (w == 1).then_some(7)
        }
        fn is_in_replay_game(&self) -> bool {
            false
        }
        fn is_quit_menu_visible(&self) -> bool {
            false
        }
        fn disconnect_menu_visible(&self) -> bool {
            false
        }
        fn populate_build_tooltip_layout(&mut self, command: Option<usize>, tooltip_win: Option<WinId>) {
            self.populated.push((command, tooltip_win));
        }
        fn layout_is_hidden(&self) -> bool {
            self.hidden
        }
        fn layout_hide(&mut self, hide: bool) {
            self.hidden = hide;
        }
    }

    /// *Predictions:* the popup shows on the first frame past the button's 100 ms delay (not at exactly 100), filled once
    /// from its command; it stays while the mouse's events keep coming; it goes the frame after they stop; the game's end
    /// hides it.
    ///
    /// *Results (2026-11-11):* held.
    #[test]
    fn the_popup_waits_for_the_delay_and_goes_when_the_mouse_leaves() {
        let mut h = H { hidden: true, ..Default::default() };
        let mut b = BuildTooltip::default();
        let frame = |b: &mut BuildTooltip, h: &mut H, over: Option<WinId>| {
            b.update(h);
            if let Some(w) = over {
                command_button_tooltip(b, w, h);
            }
            h.now += 50;
        };
        for _ in 0..3 {
            frame(&mut b, &mut h, Some(1));
        }
        // first call at 0, then 50, 100: not yet (100 < 100 is false)
        assert!(h.hidden && h.populated.is_empty());
        frame(&mut b, &mut h, Some(1));
        assert!(!h.hidden && h.populated == [(Some(7), None)]);
        for _ in 0..5 {
            frame(&mut b, &mut h, Some(1));
        }
        assert!(!h.hidden && h.populated.len() == 1);
        frame(&mut b, &mut h, None);
        frame(&mut b, &mut h, None);
        assert!(h.hidden && b.prev_window.is_none());
        for _ in 0..4 {
            frame(&mut b, &mut h, Some(2));
        }
        assert!(!h.hidden && h.populated.last() == Some(&(None, Some(2))));
        h.ending = true;
        frame(&mut b, &mut h, None);
        assert!(h.hidden);
        assert!(command_gets_tooltip(false, false) && command_gets_tooltip(true, true) && !command_gets_tooltip(true, false));
    }
}
