// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (RADAR_ATTACK_GLOW_FRAMES,
//     RADAR_ATTACK_GLOW_NUM_TIMES, triggerRadarAttackGlow, updateRadarAttackGlow; the glow's lines of the constructor,
//     init, reset, update and the destructor)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (m_radarAttackGlowOn, m_remainingRadarAttackGlowFrames,
//     m_radarAttackGlowWindow)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The radar's "under attack" glow (GitHub #57, #162): `ControlBar::triggerRadarAttackGlow` and
//! `updateRadarAttackGlow`.
//!
//! # As upstream does it
//!
//! - **The window** is `ControlBar.wnd:WinUAttack` (`ControlBar::init`). Its enabled image is the scheme's
//!   `UAttackButtonEnable`, its disabled image `UAttackButtonHightlited` (`ControlBarScheme`, translated in
//!   [`crate::client::control_bar_scheme`]): the glow is the window drawn disabled. Nothing else draws it.
//! - **`triggerRadarAttackGlow`** (from `Radar::tryUnderAttackEvent` when an event was made): with the window, the glow
//!   is on for `RADAR_ATTACK_GLOW_FRAMES` (150) frames, and the window is disabled if it was enabled.
//! - **`updateRadarAttackGlow`** (first in `ControlBar::update`, once a client frame): while on, one frame less; at 0 or
//!   below the glow is off and the window enabled; else, every `RADAR_ATTACK_GLOW_NUM_TIMES` (15) frames, the window's
//!   enabled state flips. So a trigger disables the window at once and it flips at 135, 120, ..., 15 frames left: lit
//!   for frames 150-136, 120-106, 90-76, 60-46 and 30-16 left (five lit spans of 15), unlit from 15 left on.
//! - **`reset`** enables the window and turns the glow off; the destructor and `init` forget the window.
//! - `winEnable` sets or clears `WIN_STATUS_ENABLED` on the window and every child.
//!
//! # Upstream quirks that are kept
//!
//! - A trigger while the glow is on restarts the count (150) but leaves the window as it is when it is disabled
//!   (mid-flash), so the flashes continue from the window's current state.
//!
//! # Where this differs from upstream
//!
//! Nothing: the window manager is the translated one ([`GameWindowManager::win_enable`]).

use crate::client::window_manager::GameWindowManager;
use crate::client::window_script::{win_status, WinId};

/// `RADAR_ATTACK_GLOW_FRAMES`.
pub const RADAR_ATTACK_GLOW_FRAMES: i32 = 150;
/// `RADAR_ATTACK_GLOW_NUM_TIMES` ("number of times we'll flash").
pub const RADAR_ATTACK_GLOW_NUM_TIMES: i32 = 15;
/// The window's name (`ControlBar::init`).
pub const RADAR_ATTACK_GLOW_WINDOW: &[u8] = b"ControlBar.wnd:WinUAttack";

/// `ControlBar`'s glow members.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadarAttackGlow {
    /// `m_radarAttackGlowOn`.
    pub on: bool,
    /// `m_remainingRadarAttackGlowFrames`.
    pub remaining_frames: i32,
    /// `m_radarAttackGlowWindow`.
    pub window: Option<WinId>,
}

impl RadarAttackGlow {
    /// `ControlBar::ControlBar`: off, no frames, no window.
    pub fn new() -> Self {
        Self::default()
    }

    /// `ControlBar::init`'s `m_radarAttackGlowWindow = winGetWindowFromId(NULL, nameToKey("ControlBar.wnd:WinUAttack"))`.
    pub fn init(&mut self, wm: &GameWindowManager) {
        self.window = wm.tree.find_by_name(RADAR_ATTACK_GLOW_WINDOW);
    }

    /// `triggerRadarAttackGlow()`.
    pub fn trigger(&mut self, wm: &mut GameWindowManager) {
        let Some(w) = self.window else { return };
        self.on = true;
        self.remaining_frames = RADAR_ATTACK_GLOW_FRAMES;
        if (wm.tree.windows[w].status & win_status::ENABLED != 0) == (!crate::logic::modules::plant(9581)) {
            wm.win_enable(w, false);
        }
    }

    /// `updateRadarAttackGlow()`.
    pub fn update(&mut self, wm: &mut GameWindowManager) {
        let Some(w) = self.window else { return };
        if !self.on {
            return;
        }
        self.remaining_frames -= 1;
        if self.remaining_frames <= i32::from(crate::logic::modules::plant(9582)) {
            self.on = false;
            wm.win_enable(w, true);
            return;
        }
        let period = RADAR_ATTACK_GLOW_NUM_TIMES + i32::from(crate::logic::modules::plant(9583));
        if self.remaining_frames % period == 0 {
            let enabled = wm.tree.windows[w].status & win_status::ENABLED != 0;
            wm.win_enable(w, !enabled);
        }
    }

    /// `ControlBar::reset()`'s glow lines: the window enabled, the glow off, no frames left.
    pub fn reset(&mut self, wm: &mut GameWindowManager) {
        if let Some(w) = self.window {
            wm.win_enable(w, true);
        }
        self.on = false;
        if !crate::logic::modules::plant(9584) {
            self.remaining_frames = 0;
        }
    }

    /// Whether the glow's window is drawn lit (disabled, `UAttackButtonHightlited`) now.
    pub fn is_lit(&self, wm: &GameWindowManager) -> bool {
        self.window.is_some_and(|w| wm.tree.windows[w].status & win_status::ENABLED == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::window_manager::{NoGuiHost, SystemFunc};
    use crate::client::window_script::WinInstanceData;

    /// Predicted: a trigger disables the window; it flips at 135, 120, ..., 15 frames left and is enabled again at 0,
    /// 150 updates after the trigger. *Miss (2026-11-12, first run):* I first counted ten changes of the window; the ninth
    /// flip already leaves it enabled, so the end changes nothing: nine.
    #[test]
    fn the_glow_flashes_and_ends_enabled() {
        let mut wm = GameWindowManager::new((800, 600));
        let w = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, 0, 0, 10, 10, SystemFunc::Default, Some(WinInstanceData::new()));
        wm.tree.windows[w].inst.decorated_name_string = RADAR_ATTACK_GLOW_WINDOW.to_vec();
        let mut g = RadarAttackGlow::new();
        g.trigger(&mut wm);
        assert!(g.window.is_none() && !g.on, "no window before init: nothing");
        g.init(&wm);
        assert_eq!(g.window, Some(w));
        g.trigger(&mut wm);
        assert!(g.on && g.is_lit(&wm));
        let mut flips = 0;
        let mut lit = true;
        for k in 1..=150 {
            g.update(&mut wm);
            if g.is_lit(&wm) != lit {
                flips += 1;
                lit = !lit;
            }
            if k < 150 {
                assert!(g.on);
            }
        }
        assert!(!g.on && !g.is_lit(&wm));
        // nine flips at 135..15 left; the ninth leaves it enabled, so the end's `winEnable(TRUE)` changes nothing
        assert_eq!(flips, 9);
        g.trigger(&mut wm);
        g.reset(&mut wm);
        assert!(!g.on && !g.is_lit(&wm) && g.remaining_frames == 0);
    }
}
