// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (ControlBar::getStarImage,
//     ControlBar::onPlayerRankChanged, ControlBar::onPlayerSciencePurchasePointsChanged, the general's star lines of
//     ControlBar::init and the end of ControlBar::reset)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (m_genStarFlash, m_lastFlashedAtPointValue, m_genArrow)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The general's button's flash and the `ControlBarArrow` transition (GitHub #119): what `TheControlBar` does when the
//! local player's rank or science purchase points change, and the star image its update asks for every frame.
//!
//! - `ControlBar::onPlayerRankChanged` and `onPlayerSciencePurchasePointsChanged` (ControlBar.cpp:1674, :1692, the same
//!   body): for the local player only, unless the points have fallen below the value the button last flashed at, the
//!   transition handler's `"ControlBarArrow"` group is set, **only with the InGameUI's input on** (ControlBar.cpp:1682,
//!   :1699, #119's input gate); then the flash is on and the bar marked dirty ([`GenStar::on_player_rank_changed`],
//!   [`GenStar::on_player_science_purchase_points_changed`]).
//! - `ControlBar::getStarImage` (ControlBar.cpp:1645, called first thing by `ControlBar::update`): the flash ends when
//!   the points are below the last flashed value or not positive, else the value is remembered; the general's button
//!   (`ControlBar.wnd:ButtonGeneral`) then shows its highlight image in the second half of each logic second while
//!   flashing, its enabled image otherwise ([`GenStar::get_star_image`]). It always returns `NULL` upstream: nothing is
//!   returned here.
//! - `ControlBar::init`'s two lines (`m_genStarFlash = TRUE`, `m_lastFlashedAtPointValue = -1`; its image lookups of
//!   `BarButtonGenStarON` / `OFF` are kept by the host) and the end of `ControlBar::reset` (the group removed, the arrow
//!   image cleared, the same two values) are [`GenStar::init`] and [`GenStar::reset`].
//!
//! `tests/differential_gen_star.rs` compares them with the C++ cut verbatim by build.rs.
//!
//! **Not translated here:** `showPurchaseScience`'s `m_genStarFlash = FALSE` (the general's purchase screen is not
//! translated), and the transition's drawing, which is `client::window_transitions`' (`ControlBarArrowTransition`).

use crate::logic::modules::plant;

/// The group name `onPlayerRankChanged` and `onPlayerSciencePurchasePointsChanged` set, and `reset` removes.
pub const CONTROL_BAR_ARROW: &[u8] = b"ControlBarArrow";

/// `LOGICFRAMES_PER_SECOND` (GameCommon.h:72).
pub const LOGICFRAMES_PER_SECOND: u32 = 30;

/// Which of the scheme's general's button images `GadgetButtonSetEnabledImage` is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralButtonImage {
    /// `m_generalButtonEnable` (the scheme's `GeneralButtonEnable`).
    Enable,
    /// `m_generalButtonHighlight` (the scheme's `GeneralButtonHightlited`).
    Highlight,
}

/// What the general's star asks of the game and the bar.
pub trait GenStarHost {
    /// `p->isLocalPlayer()` for the player of index `player_index`.
    fn is_local_player(&self, player_index: i32) -> bool;
    /// `ThePlayerList->getLocalPlayer()->getSciencePurchasePoints()`.
    fn local_science_purchase_points(&self) -> i32;
    /// `TheTransitionHandler != NULL`.
    fn transition_handler(&self) -> bool;
    /// `TheInGameUI->getInputEnabled()`.
    fn ui_input_enabled(&self) -> bool;
    /// `TheTransitionHandler->setGroup(name)` (`immidiate` `FALSE`, the default).
    fn transition_set_group(&mut self, name: &[u8]);
    /// `TheTransitionHandler->remove(name)` (`skipPending` `FALSE`, the default).
    fn transition_remove(&mut self, name: &[u8]);
    /// `markUIDirty()`.
    fn mark_ui_dirty(&mut self);
    /// Whether `TheWindowManager->winGetWindowFromId(NULL, NAMEKEY("ControlBar.wnd:ButtonGeneral"))` finds the window.
    fn general_button_exists(&self) -> bool;
    /// `GadgetButtonSetEnabledImage(ButtonGeneral, image)`.
    fn general_button_set_enabled_image(&mut self, image: GeneralButtonImage);
    /// `TheGameLogic->getFrame()`.
    fn logic_frame(&self) -> u32;
}

/// `TheControlBar`'s general's star members.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenStar {
    /// `m_genStarFlash`.
    pub gen_star_flash: bool,
    /// `m_lastFlashedAtPointValue`.
    pub last_flashed_at_point_value: i32,
    /// `m_genArrow`'s being set (`setArrowImage`); `reset` clears it.
    pub gen_arrow_set: bool,
}

impl Default for GenStar {
    /// `ControlBar::ControlBar`: `m_genStarFlash = FALSE` (ControlBar.cpp:893). The constructor leaves
    /// `m_lastFlashedAtPointValue` unset (`init` sets it); 0 here.
    fn default() -> Self {
        GenStar { gen_star_flash: false, last_flashed_at_point_value: 0, gen_arrow_set: false }
    }
}

impl GenStar {
    /// `ControlBar::init`'s two lines (ControlBar.cpp:1268-1269): `m_genStarFlash = TRUE`,
    /// `m_lastFlashedAtPointValue = -1`.
    pub fn init(&mut self) {
        self.gen_star_flash = true;
        self.last_flashed_at_point_value = -1;
    }

    /// The end of `ControlBar::reset` (ControlBar.cpp:1371-1376): `TheTransitionHandler->remove("ControlBarArrow")` if
    /// there is a handler, `m_genArrow = NULL`, `m_lastFlashedAtPointValue = -1`, `m_genStarFlash = TRUE`.
    pub fn reset(&mut self, h: &mut impl GenStarHost) {
        if h.transition_handler() {
            h.transition_remove(CONTROL_BAR_ARROW);
        }
        self.gen_arrow_set = false;
        self.last_flashed_at_point_value = -1;
        self.gen_star_flash = !plant(10338);
    }

    /// `ControlBar::getStarImage` (ControlBar.cpp:1645-1670), from `ControlBar::update`.
    pub fn get_star_image(&mut self, h: &mut impl GenStarHost) {
        let points = h.local_science_purchase_points();
        if self.last_flashed_at_point_value > points || (points <= 0 && !plant(10339)) {
            self.gen_star_flash = false;
        } else {
            self.last_flashed_at_point_value = points;
        }
        if !h.general_button_exists() {
            return;
        }
        if !self.gen_star_flash {
            h.general_button_set_enabled_image(GeneralButtonImage::Enable);
            return;
        }
        let half = if plant(10340) { LOGICFRAMES_PER_SECOND / 2 - 1 } else { LOGICFRAMES_PER_SECOND / 2 };
        if h.logic_frame() % LOGICFRAMES_PER_SECOND > half {
            h.general_button_set_enabled_image(GeneralButtonImage::Highlight);
            return;
        }
        h.general_button_set_enabled_image(GeneralButtonImage::Enable);
    }

    /// `ControlBar::onPlayerRankChanged(p)` (ControlBar.cpp:1674-1688), for the logic's
    /// `PlayerClientEvent::RankChanged`.
    pub fn on_player_rank_changed(&mut self, player_index: i32, h: &mut impl GenStarHost) {
        self.on_player_changed(player_index, h);
    }

    /// `ControlBar::onPlayerSciencePurchasePointsChanged(p)` (ControlBar.cpp:1692-1705, the same body as
    /// `onPlayerRankChanged`), for the logic's `PlayerClientEvent::SciencePurchasePointsChanged`.
    pub fn on_player_science_purchase_points_changed(&mut self, player_index: i32, h: &mut impl GenStarHost) {
        self.on_player_changed(player_index, h);
    }

    /// The body the two share.
    fn on_player_changed(&mut self, player_index: i32, h: &mut impl GenStarHost) {
        if !h.is_local_player(player_index) && !plant(10341) {
            return;
        }
        let below = if plant(10342) { self.last_flashed_at_point_value >= h.local_science_purchase_points() } else { self.last_flashed_at_point_value > h.local_science_purchase_points() };
        if !below && h.transition_handler() && (h.ui_input_enabled() || plant(10343)) {
            h.transition_set_group(CONTROL_BAR_ARROW);
        }
        if !plant(10344) {
            self.gen_star_flash = true;
        }
        h.mark_ui_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct H {
        local: i32,
        points: i32,
        handler: bool,
        input: bool,
        button: bool,
        frame: u32,
        log: Vec<String>,
    }

    impl GenStarHost for H {
        fn is_local_player(&self, p: i32) -> bool {
            p == self.local
        }
        fn local_science_purchase_points(&self) -> i32 {
            self.points
        }
        fn transition_handler(&self) -> bool {
            self.handler
        }
        fn ui_input_enabled(&self) -> bool {
            self.input
        }
        fn transition_set_group(&mut self, name: &[u8]) {
            self.log.push(format!("set {}", String::from_utf8_lossy(name)));
        }
        fn transition_remove(&mut self, name: &[u8]) {
            self.log.push(format!("remove {}", String::from_utf8_lossy(name)));
        }
        fn mark_ui_dirty(&mut self) {
            self.log.push("dirty".into());
        }
        fn general_button_exists(&self) -> bool {
            self.button
        }
        fn general_button_set_enabled_image(&mut self, image: GeneralButtonImage) {
            self.log.push(format!("image {image:?}"));
        }
        fn logic_frame(&self) -> u32 {
            self.frame
        }
    }

    /// *Predictions, written before the first run (issue119_predictions.md):* a rank change of the local player with
    /// points to spend sets the arrow group and the flash; the same with the input off sets the flash but not the group
    /// (ControlBar.cpp:1682); another player's change does nothing; the flash shows the highlight on frames 16-29 of each
    /// second and the enabled image on 0-15; spending the points (fewer than flashed at) ends the flash; `reset` removes
    /// the group and turns the flash back on.
    #[test]
    fn the_general_s_star_flashes_with_the_input_gate() {
        let mut h = H { local: 2, points: 1, handler: true, input: true, button: true, ..Default::default() };
        let mut s = GenStar::default();
        s.init();
        s.on_player_rank_changed(2, &mut h);
        assert_eq!(h.log, ["set ControlBarArrow", "dirty"]);
        assert!(s.gen_star_flash);
        h.log.clear();
        h.input = false;
        s.on_player_science_purchase_points_changed(2, &mut h);
        assert_eq!(h.log, ["dirty"], "the input gate (ControlBar.cpp:1699)");
        h.log.clear();
        s.on_player_rank_changed(3, &mut h);
        assert!(h.log.is_empty());
        h.frame = 15;
        s.get_star_image(&mut h);
        h.frame = 16;
        s.get_star_image(&mut h);
        assert_eq!(h.log, ["image Enable", "image Highlight"]);
        assert_eq!(s.last_flashed_at_point_value, 1);
        h.log.clear();
        h.points = 0;
        s.get_star_image(&mut h);
        assert!(!s.gen_star_flash);
        assert_eq!(h.log, ["image Enable"]);
        h.log.clear();
        s.reset(&mut h);
        assert_eq!(h.log, ["remove ControlBarArrow"]);
        assert!(s.gen_star_flash && s.last_flashed_at_point_value == -1);
    }
}
