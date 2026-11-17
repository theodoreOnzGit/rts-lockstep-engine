// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (ControlBar::init's three
//     AnimateWindowManagers, :1253-1258; ControlBar::reset's resets, :1314-1321; ControlBar::update's animate lines,
//     :1394-1416; ControlBar::animateSpecialPowerShortcut, :3716-3741)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (m_animateWindowManager,
//     m_animateWindowManagerForGenShortcuts, m_generalsScreenAnimate, MAX_SPECIAL_POWER_SHORTCUTS)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #137: the control bar's animate window managers, which slide the bar in (`ShowControlBar(FALSE)`) and out
//! (`HideControlBar(FALSE)`), and the special power shortcut bar with it.
//!
//! # As upstream does it
//!
//! - **`ControlBar::init`** (ControlBar.cpp:1253) makes three [`AnimateWindowManager`]s, once each: `m_animateWindowManager`
//!   (the bar), `m_generalsScreenAnimate` (the purchase-science screen; its uses are all commented out upstream) and
//!   `m_animateWindowManagerForGenShortcuts` (the special power shortcut bar). Each process reads the display's width
//!   when it is made, so the spiral's radius is the width at `init` ([`ControlBarAnimate::new`]).
//! - **`ShowControlBar(FALSE)`** and **`ToggleControlBar(FALSE)`** reset the bar's manager and register
//!   `ControlBar.wnd:ControlBarParent` with `WIN_ANIMATION_SLIDE_BOTTOM`, needing to finish, 500 ms, no delay
//!   (ControlBarCallback.cpp:515, :598): the bar jumps the display's width below its place and slides up, 40 pixels a
//!   frame and slower within 80 of it. **`HideControlBar(FALSE)`** reverses it (:563): the bar slides back down. Both
//!   then call [`ControlBarAnimate::animate_special_power_shortcut`]. Those calls are
//!   [`crate::client::control_bar_stage`]'s (`animate_reset`, `animate_register_slide_bottom`, `animate_reverse`).
//! - **`ControlBar::update`** (:1394, [`ControlBarAnimate::update`]), once a client frame: the bar's manager's
//!   `update`; when it is finished and reversed (the slide out done), `ControlBarParent` is hidden if shown. Then the
//!   shortcut bar's manager, the same way for its parent window. (`m_generalsScreenAnimate`'s update is commented out
//!   upstream, :2933.)
//! - **`ControlBar::reset`** (:1314, [`ControlBarAnimate::reset`]) resets the three.
//!
//! # Where this differs from upstream
//!
//! - `timeGetTime()` is the caller's `now`; `TheDisplay` is the window manager's [`GameWindowManager::display`].
//! - `TheWindowManager->winGetWindowFromId(NULL, nameToKey("ControlBar.wnd:ControlBarParent"))` is the tree's
//!   [`WindowTree::find_by_name`](crate::client::window_script::WindowTree::find_by_name), as the stage functions find it.
//! - The shortcut bar's windows (`m_specialPowerShortcutParent`, `m_specialPowerShortcutButtons` with their user data,
//!   the special power's command button, and `m_currentlyUsedSpecialPowersButtons`) are the caller's: the client loads
//!   no shortcut bar yet (#137), so it passes none and the shortcut's calls do nothing.
//! - The destructor's `delete`s (the managers' own destructors put every window at its rest) are not translated: the
//!   client's bar lives as long as its window manager.

use crate::client::animate_window::{anim, AnimateWindowManager};
use crate::client::control_bar_stage::CONTROL_BAR_PARENT;
use crate::client::window_manager::GameWindowManager;
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// `MAX_SPECIAL_POWER_SHORTCUTS` (ControlBar.h:415).
pub const MAX_SPECIAL_POWER_SHORTCUTS: usize = 11;

/// `TheControlBar`'s three animate window managers.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlBarAnimate {
    /// `m_animateWindowManager`: the bar's slide.
    pub animate_window_manager: AnimateWindowManager,
    /// `m_animateWindowManagerForGenShortcuts`: the special power shortcut bar's slide.
    pub animate_window_manager_for_gen_shortcuts: AnimateWindowManager,
    /// `m_generalsScreenAnimate`: made and reset, never registered nor updated (upstream's uses are commented out).
    pub generals_screen_animate: AnimateWindowManager,
}

/// The special power shortcut bar as `animateSpecialPowerShortcut` and `update` read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpecialPowerShortcut<'a> {
    /// `m_specialPowerShortcutParent`.
    pub parent: Option<WinId>,
    /// `m_specialPowerShortcutButtons[i]->winGetUserData() != NULL`, for the first `m_currentlyUsedSpecialPowersButtons`
    /// buttons (the slice's length).
    pub button_has_user_data: &'a [bool],
}

impl ControlBarAnimate {
    /// `ControlBar::init`'s `NEW AnimateWindowManager` (three times), on a display `display_width` wide.
    pub fn new(display_width: i32) -> Self {
        let width = if plant(10400) { 0 } else { display_width };
        ControlBarAnimate {
            animate_window_manager: AnimateWindowManager::new(width),
            generals_screen_animate: AnimateWindowManager::new(display_width),
            animate_window_manager_for_gen_shortcuts: AnimateWindowManager::new(display_width),
        }
    }

    /// `ControlBar::reset`'s lines: `m_animateWindowManager->reset()`, then the shortcut bar's, then
    /// `m_generalsScreenAnimate`'s (every window put at its rest and forgotten).
    pub fn reset(&mut self, wm: &mut GameWindowManager) {
        self.animate_window_manager.reset(wm);
        if !plant(10390) {
            self.animate_window_manager_for_gen_shortcuts.reset(wm);
        }
        self.generals_screen_animate.reset(wm);
    }

    /// `ControlBar::update`'s animate lines at `now` (`timeGetTime()`): the bar's manager updated, and once it is finished
    /// and reversed, `ControlBarParent` hidden (if it is not); then the shortcut bar's manager, and its parent the same
    /// way.
    pub fn update(&mut self, wm: &mut GameWindowManager, now: u32, shortcut: SpecialPowerShortcut) {
        self.animate_window_manager.update(wm, now);
        if wm.undefined.is_some() {
            return;
        }
        let m = &self.animate_window_manager;
        if m.is_finished() && (m.is_reversed() || plant(10386)) {
            if let Some(window) = wm.tree.find_by_name(CONTROL_BAR_PARENT) {
                if !wm.tree.windows[window].is_hidden() {
                    wm.win_hide(window, true);
                }
            }
        }
        if !plant(10387) {
            self.animate_window_manager_for_gen_shortcuts.update(wm, now);
            if wm.undefined.is_some() {
                return;
            }
        }
        if let Some(parent) = shortcut.parent {
            let g = &self.animate_window_manager_for_gen_shortcuts;
            if g.is_finished() && g.is_reversed() && !wm.tree.windows[parent].is_hidden() {
                wm.win_hide(parent, true);
            }
        }
    }

    /// `ControlBar::animateSpecialPowerShortcut(isOn)` at `now`: nothing without the shortcut bar, with no button in use,
    /// or when no used button holds a special power ("dontAnimate"); else on, the shortcut bar's manager reset and its
    /// parent registered sliding from the right (needing to finish, 500 ms, no delay); off, reversed.
    pub fn animate_special_power_shortcut(&mut self, wm: &mut GameWindowManager, now: u32, is_on: bool, shortcut: SpecialPowerShortcut) {
        let Some(parent) = shortcut.parent else { return };
        if shortcut.button_has_user_data.is_empty() {
            return;
        }
        let dont_animate = !shortcut.button_has_user_data.iter().any(|&u| u) && !plant(10388);
        if dont_animate {
            return;
        }
        let m = &mut self.animate_window_manager_for_gen_shortcuts;
        if is_on {
            m.reset(wm);
            let t = if plant(10389) { anim::WIN_ANIMATION_SLIDE_LEFT } else { anim::WIN_ANIMATION_SLIDE_RIGHT };
            m.register_game_window(wm, Some(parent), t, true, 500, 0, now);
        } else {
            m.reverse_animate_window(wm, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::window_manager::{NoGuiHost, SystemFunc};
    use crate::client::window_script::{win_status, WinInstanceData};

    /// Predicted from ControlBarCallback.cpp and ControlBar::update: the bar at (0, 450) on an 800-wide display,
    /// registered sliding from the bottom, jumps to y 1250 and comes up 40 a frame (1210 after the first update), slows
    /// within 80 and stops at 450 (the bottom slide puts it at its rest); the manager is finished and not reversed, and
    /// the bar stays shown. Reversed, it goes back down to 1250; the update that finds it finished and reversed hides it.
    /// The shortcut bar without a used button with a special power does nothing.
    #[test]
    fn the_bar_slides_in_and_out_and_hides() {
        let mut wm = GameWindowManager::new((800, 600));
        let bar = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, 0, 450, 800, 150, SystemFunc::Default, Some(WinInstanceData::new()));
        wm.tree.windows[bar].inst.decorated_name_string = CONTROL_BAR_PARENT.to_vec();
        let mut a = ControlBarAnimate::new(800);
        let none = SpecialPowerShortcut::default();
        a.animate_window_manager.reset(&mut wm);
        a.animate_window_manager.register_game_window(&mut wm, Some(bar), anim::WIN_ANIMATION_SLIDE_BOTTOM, true, 500, 0, 0);
        assert_eq!(wm.tree.windows[bar].region.1, 1250);
        a.update(&mut wm, 0, none);
        assert_eq!(wm.tree.windows[bar].region.1, 1210);
        let mut n = 1;
        while !a.animate_window_manager.is_finished() {
            a.update(&mut wm, 0, none);
            n += 1;
            assert!(n < 200);
        }
        assert_eq!(wm.tree.windows[bar].region.1, 450);
        assert!(!wm.tree.windows[bar].is_hidden());
        a.animate_window_manager.reverse_animate_window(&mut wm, 0);
        while !a.animate_window_manager.is_finished() {
            a.update(&mut wm, 0, none);
            n += 1;
            assert!(n < 400);
        }
        assert_eq!(wm.tree.windows[bar].region.1, 1250);
        assert!(wm.tree.windows[bar].is_hidden());
        // the shortcut bar: a parent but no special power on its used buttons, so no animation
        let parent = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, 700, 100, 100, 300, SystemFunc::Default, Some(WinInstanceData::new()));
        let sc = SpecialPowerShortcut { parent: Some(parent), button_has_user_data: &[false, false] };
        a.animate_special_power_shortcut(&mut wm, 0, true, sc);
        assert!(a.animate_window_manager_for_gen_shortcuts.is_empty());
        // with one: it slides in from the right, the display's width out
        let sc = SpecialPowerShortcut { parent: Some(parent), button_has_user_data: &[false, true] };
        a.animate_special_power_shortcut(&mut wm, 0, true, sc);
        assert_eq!(wm.tree.windows[parent].region.0, 1500);
        a.reset(&mut wm);
        assert_eq!(wm.tree.windows[parent].region.0, 700);
        assert!(a.animate_window_manager_for_gen_shortcuts.is_empty());
    }
}
