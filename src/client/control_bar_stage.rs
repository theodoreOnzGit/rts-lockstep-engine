// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (enum ControlBarStages)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (ControlBar::init's CP_MASTER and
//     m_defaultControlBarPosition, toggleControlBarStage, switchControlBarStage, setDefaultControlBarConfig,
//     setLowControlBarConfig, setHiddenControlBar, setUpDownImages, hidePurchaseScience)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarCallback.cpp (ShowControlBar, HideControlBar,
//     ToggleControlBar; ControlBarSystem's ButtonLarge case)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (showReplayControls, hideReplayControls,
//     toggleReplayControls)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (CommandTranslator::translateGameMessage's
//     MSG_META_TOGGLE_CONTROL_BAR case)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #136 (#128's gap): hiding and showing the control bar.
//!
//! # As upstream does it
//!
//! - **The key** (`MSG_META_TOGGLE_CONTROL_BAR`, CommandXlat.cpp:3168, [`toggle_control_bar_meta`]): in the shell, the
//!   shell's top layout is hidden or shown (`win->hide(!win->isHidden())`); in a game, unless a replay is playing back
//!   (`TheRecorder->getMode() == RECORDERMODETYPE_PLAYBACK`), `ToggleControlBar()` (immediate). Destroyed either way. The
//!   case also looks `ControlBar.wnd:ControlBarParent` up and computes a `hide` it never reads; nothing of it is kept.
//! - **`ToggleControlBar(immediate)`** (ControlBarCallback.cpp:578, [`toggle_control_bar`]): `toggleReplayControls()`;
//!   then, on `ControlBarParent`: when it is hidden, `showSpecialPowerShortcut()`, the tactical view's height to 80 % of
//!   the display's, the window shown, `switchControlBarStage(CONTROL_BAR_STAGE_DEFAULT)` and (not immediate, with an
//!   animate window manager) the slide-in; when shown, `hideSpecialPowerShortcut()`, the view's height to the display's,
//!   the window hidden.
//! - **`ShowControlBar(immediate)`** / **`HideControlBar(immediate)`** (:501, :536, [`show_control_bar`],
//!   [`hide_control_bar`]): the same with the replay controls shown or hidden; Show switches to the default stage and
//!   marks the UI dirty; Hide also hides the purchase-science screen ("Always get rid of the purchase science screen!").
//! - **The stages** (ControlBar.cpp:2992-3077, [`ControlBarStage`]): `toggleControlBarStage` goes from
//!   `CONTROL_BAR_STAGE_DEFAULT` to `CONTROL_BAR_STAGE_LOW` and from any other stage to the default; `switchControlBarStage`
//!   ignores a stage out of range and any switch during a replay's playback. Default: the bar at its `ControlBar.wnd`
//!   position, shown, the view at 80 %, the popup's layout repopulated, the up/down images. Low: the bar moved down to
//!   `getHeight() - .1 * getHeight()` (its top 10 % of the display above the bottom edge), the view the display's whole
//!   height. Hidden: the bar hidden. `setUpDownImages` gives `ControlBar.wnd:ButtonLarge` the scheme's "up" images in
//!   the low stage, else the "down" ones.
//! - **The bar's own button** (ControlBarCallback.cpp:434): `ButtonLarge`'s `GBM_SELECTED` is `toggleControlBarStage()`.
//!   It is the bar's way to lower and raise itself with no key, which the client's phone uses.
//!
//! # Where this differs from upstream
//!
//! - **`m_currentControlBarStage` is not set by the constructor** upstream; `ControlBarScheme::init` switches to the
//!   default stage when the side's scheme is set (ControlBarScheme.cpp:429), before any toggle. [`ControlBarStage::new`]
//!   starts at the default stage.
//! - **`setSquishedControlBarConfig`** is not translated: `switchControlBarStage`'s case for it is commented out
//!   upstream, so nothing reaches it.
//! - **Crashes are not translated.** `HideControlBar(TRUE)` without a `ControlBarParent` calls `winHide` on `NULL`, and
//!   `hidePurchaseScience` tests the purchase-science window's hidden bit before its `NULL` check; here both do nothing.
//!   `ControlBar::init` dereferences `CP_MASTER` unchecked; here a bar without it keeps the position (0, 0) and the
//!   stage functions skip the window. The differential test generates none of these.
//! - The windows, the tactical view, the display, the special power shortcut, the animation and the UI's dirty flag are
//!   the host's ([`ControlBarStageHost`]); the client's tactical view keeps the whole window (the bar is drawn over it),
//!   so it takes no height from here.

use crate::client::control_bar_scheme::{gadget_button_set_enabled_image, gadget_button_set_hilite_image, gadget_button_set_hilite_selected_image};
use crate::client::selection_xlat::Disposition;
use crate::client::window_script::{WinId, WindowTree};
use crate::logic::modules::plant;

/// `enum ControlBarStages` (ControlBar.h:639).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum ControlBarStages {
    /// "full view for the world to see"
    #[default]
    Default = 0,
    /// "squished just for expeirenced players"
    Squished = 1,
    /// "control bar a la minimalist"
    Low = 2,
    /// "yo, where be da control bar at?"
    Hidden = 3,
}

/// `MAX_CONTROL_BAR_STAGES`.
pub const MAX_CONTROL_BAR_STAGES: i32 = 4;

impl ControlBarStages {
    /// The stage of a value in range.
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Default,
            1 => Self::Squished,
            2 => Self::Low,
            3 => Self::Hidden,
            _ => return None,
        })
    }
}

/// `ControlBar::m_toggleButtonUp*` / `m_toggleButtonDown*`, the scheme's images for `ButtonLarge`
/// (`updateUpDownImages`): which one a `GadgetButtonSet*Image` call gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleImage {
    UpIn,
    UpOn,
    UpPushed,
    DownIn,
    DownOn,
    DownPushed,
}

impl ToggleImage {
    /// The scheme's field (`ControlBarScheme`'s `ToggleButtonUpIn` ...) that holds the image.
    pub fn scheme_field(self) -> &'static str {
        match self {
            Self::UpIn => "ToggleButtonUpIn",
            Self::UpOn => "ToggleButtonUpOn",
            Self::UpPushed => "ToggleButtonUpPushed",
            Self::DownIn => "ToggleButtonDownIn",
            Self::DownOn => "ToggleButtonDownOn",
            Self::DownPushed => "ToggleButtonDownPushed",
        }
    }
}

/// What the stages and the show, hide and toggle functions reach outside `TheControlBar`.
pub trait ControlBarStageHost {
    /// `TheWindowManager->winGetWindowFromId(NULL, TheNameKeyGenerator->nameToKey(name))`.
    fn win_from_name(&self, name: &[u8]) -> Option<WinId>;
    /// `win->winIsHidden()`: the window's own hidden bit.
    fn win_is_hidden(&self, w: WinId) -> bool;
    /// `win->winHide(hide)`.
    fn win_hide(&mut self, w: WinId, hide: bool);
    /// `win->winGetPosition(&x, &y)`.
    fn win_get_position(&self, w: WinId) -> (i32, i32);
    /// `win->winSetPosition(x, y)`.
    fn win_set_position(&mut self, w: WinId, x: i32, y: i32);
    /// `GadgetButtonSetEnabledImage`, `GadgetButtonSetHiliteImage`, `GadgetButtonSetHiliteSelectedImage` (in that order).
    fn gadget_button_set_images(&mut self, w: WinId, enabled: ToggleImage, hilite: ToggleImage, hilite_selected: ToggleImage);
    /// `TheDisplay->getHeight()`.
    fn display_height(&self) -> u32;
    /// `TheTacticalView->setHeight(h)`.
    fn set_tactical_view_height(&mut self, h: i32);
    /// `TheRecorder && TheRecorder->getMode() == RECORDERMODETYPE_PLAYBACK`.
    fn recorder_playback(&self) -> bool;
    /// `TheGameLogic->isInReplayGame()`.
    fn is_in_replay_game(&self) -> bool;
    /// InGameUI.cpp's `m_replayWindow` (`ReplayControl.wnd`), if there is one.
    fn replay_window(&self) -> Option<WinId>;
    /// `TheControlBar->showSpecialPowerShortcut()`.
    fn show_special_power_shortcut(&mut self);
    /// `TheControlBar->hideSpecialPowerShortcut()`.
    fn hide_special_power_shortcut(&mut self);
    /// `TheControlBar->animateSpecialPowerShortcut(on)`.
    fn animate_special_power_shortcut(&mut self, on: bool);
    /// `repopulateBuildTooltipLayout()`.
    fn repopulate_build_tooltip_layout(&mut self);
    /// `TheControlBar->markUIDirty()`.
    fn mark_ui_dirty(&mut self);
    /// `TheControlBar->m_animateWindowManager` is not `NULL`.
    fn has_animate_window_manager(&self) -> bool;
    /// `m_animateWindowManager->reset()`.
    fn animate_reset(&mut self);
    /// `m_animateWindowManager->registerGameWindow(window, WIN_ANIMATION_SLIDE_BOTTOM, TRUE, 500, 0)`.
    fn animate_register_slide_bottom(&mut self, w: WinId);
    /// `m_animateWindowManager->reverseAnimateWindow()`.
    fn animate_reverse(&mut self);
}

/// `ControlBar.wnd:ControlBarParent`.
pub const CONTROL_BAR_PARENT: &[u8] = b"ControlBar.wnd:ControlBarParent";
/// `ControlBar.wnd:ButtonLarge`.
pub const BUTTON_LARGE: &[u8] = b"ControlBar.wnd:ButtonLarge";

/// `TheControlBar`'s part the stages keep.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControlBarStage {
    /// `m_currentControlBarStage`.
    pub current: ControlBarStages,
    /// `m_defaultControlBarPosition`: `CP_MASTER`'s position in `ControlBar.wnd`.
    pub default_position: (i32, i32),
    /// `m_contextParent[CP_MASTER]` (`ControlBar.wnd:ControlBarParent`).
    pub master: Option<WinId>,
    /// `m_contextParent[CP_PURCHASE_SCIENCE]` (`GeneralsExpPoints.wnd:GenExpParent`), if loaded.
    pub purchase_science: Option<WinId>,
}

impl ControlBarStage {
    /// `ControlBar::init`'s part (ControlBar.cpp:1078-1080): `CP_MASTER` is `ControlBarParent` and its position the
    /// default one; the stage the default (see the module docs).
    pub fn new(h: &impl ControlBarStageHost) -> Self {
        let master = h.win_from_name(CONTROL_BAR_PARENT);
        let default_position = master.map(|m| h.win_get_position(m)).unwrap_or_default();
        ControlBarStage { current: ControlBarStages::Default, default_position, master, purchase_science: None }
    }

    /// `ControlBar::toggleControlBarStage()`.
    pub fn toggle_control_bar_stage(&mut self, h: &mut impl ControlBarStageHost) {
        if self.current == ControlBarStages::Default && !plant(9241) {
            self.switch_control_bar_stage(ControlBarStages::Low as i32, h);
        } else {
            self.switch_control_bar_stage(ControlBarStages::Default as i32, h);
        }
    }

    /// `ControlBar::switchControlBarStage(stage)`: out of range or during a replay's playback, nothing.
    pub fn switch_control_bar_stage(&mut self, stage: i32, h: &mut impl ControlBarStageHost) {
        if !(ControlBarStages::Default as i32..MAX_CONTROL_BAR_STAGES).contains(&stage) {
            return;
        }
        if h.recorder_playback() && !plant(9242) {
            return;
        }
        match ControlBarStages::from_i32(stage) {
            Some(ControlBarStages::Default) => self.set_default_control_bar_config(h),
            // (the squished case is commented out upstream)
            Some(ControlBarStages::Low) => self.set_low_control_bar_config(h),
            Some(ControlBarStages::Hidden) => self.set_hidden_control_bar(h),
            // `DEBUG_ASSERTCRASH(FALSE, ...)`: a debug build's assertion; nothing else
            _ => {}
        }
    }

    /// `ControlBar::setDefaultControlBarConfig()`.
    pub fn set_default_control_bar_config(&mut self, h: &mut impl ControlBarStageHost) {
        self.current = ControlBarStages::Default;
        let height = if plant(9243) { 0.85f32 } else { 0.80f32 };
        h.set_tactical_view_height((h.display_height() as f32 * height) as i32);
        if let Some(m) = self.master {
            h.win_set_position(m, self.default_position.0, self.default_position.1);
            if !plant(9244) {
                h.win_hide(m, false);
            }
        }
        h.repopulate_build_tooltip_layout();
        self.set_up_down_images(h);
    }

    /// `ControlBar::setLowControlBarConfig()`: `pos.y = TheDisplay->getHeight() - .1 * TheDisplay->getHeight()`, a
    /// `double` truncated to `Int`.
    pub fn set_low_control_bar_config(&mut self, h: &mut impl ControlBarStageHost) {
        self.current = ControlBarStages::Low;
        let x = if plant(9245) { 0 } else { self.default_position.0 };
        let hd = h.display_height() as f64;
        let y = (hd - 0.1 * hd) as i32;
        h.set_tactical_view_height(h.display_height() as i32);
        if let Some(m) = self.master {
            h.win_set_position(m, x, y);
            h.win_hide(m, false);
        }
        if !plant(9246) {
            self.set_up_down_images(h);
        }
    }

    /// `ControlBar::setHiddenControlBar()`.
    pub fn set_hidden_control_bar(&mut self, h: &mut impl ControlBarStageHost) {
        self.current = ControlBarStages::Hidden;
        if let Some(m) = self.master {
            h.win_hide(m, true);
        }
    }

    /// `ControlBar::setUpDownImages()`: `ButtonLarge`'s images, "up" in the low stage ("we only care if it's in it's low
    /// state, else we put the default images up"), else "down".
    pub fn set_up_down_images(&self, h: &mut impl ControlBarStageHost) {
        let Some(win) = h.win_from_name(BUTTON_LARGE) else { return };
        if self.current == ControlBarStages::Low && !plant(9247) {
            h.gadget_button_set_images(win, ToggleImage::UpOn, ToggleImage::UpIn, ToggleImage::UpPushed);
            return;
        }
        h.gadget_button_set_images(win, ToggleImage::DownOn, ToggleImage::DownIn, ToggleImage::DownPushed);
    }

    /// `ControlBar::hidePurchaseScience()` (the window part; the commented-out animation is not translated).
    pub fn hide_purchase_science(&self, h: &mut impl ControlBarStageHost) {
        let Some(p) = self.purchase_science else { return };
        if h.win_is_hidden(p) {
            return;
        }
        h.win_hide(p, true);
    }
}

/// InGameUI.cpp's `showReplayControls()`: the replay window shown only in a replay game.
pub fn show_replay_controls(h: &mut impl ControlBarStageHost) {
    if let Some(w) = h.replay_window() {
        let show = h.is_in_replay_game();
        h.win_hide(w, !show);
    }
}

/// InGameUI.cpp's `hideReplayControls()`.
pub fn hide_replay_controls(h: &mut impl ControlBarStageHost) {
    if let Some(w) = h.replay_window() {
        h.win_hide(w, true);
    }
}

/// InGameUI.cpp's `toggleReplayControls()`: shown when in a replay game and hidden now.
pub fn toggle_replay_controls(h: &mut impl ControlBarStageHost) {
    if let Some(w) = h.replay_window() {
        let show = h.is_in_replay_game() && (h.win_is_hidden(w) || plant(9248));
        h.win_hide(w, !show);
    }
}

/// ControlBarCallback.cpp's `ShowControlBar(immediate)`: "Force the control bar to be shown".
pub fn show_control_bar(bar: &mut ControlBarStage, immediate: bool, h: &mut impl ControlBarStageHost) {
    show_replay_controls(h);
    h.show_special_power_shortcut();
    if let Some(window) = h.win_from_name(CONTROL_BAR_PARENT) {
        bar.switch_control_bar_stage(ControlBarStages::Default as i32, h);
        h.set_tactical_view_height((h.display_height() as f32 * 0.80f32) as i32);
        if h.has_animate_window_manager() && !immediate {
            h.animate_reset();
            h.animate_register_slide_bottom(window);
            h.animate_special_power_shortcut(true);
        }
        h.win_hide(window, false);
    }
    // "We want to get everything recalced since this is a major state change."
    if !plant(9249) {
        h.mark_ui_dirty();
    }
}

/// ControlBarCallback.cpp's `HideControlBar(immediate)`: "Force the control bar to be hidden" (`SLIDE_LETTERBOX` is not
/// defined: the view takes the display's whole height).
pub fn hide_control_bar(bar: &mut ControlBarStage, immediate: bool, h: &mut impl ControlBarStageHost) {
    hide_replay_controls(h);
    h.hide_special_power_shortcut();
    let window = h.win_from_name(CONTROL_BAR_PARENT);
    if window.is_some() {
        h.set_tactical_view_height(h.display_height() as i32);
    }
    if immediate {
        // (upstream calls `winHide` on a `NULL` window here; see the module docs)
        if let Some(w) = window {
            h.win_hide(w, true);
        }
        h.hide_special_power_shortcut();
    } else {
        h.animate_reverse();
        h.animate_special_power_shortcut(false);
    }
    // "Always get rid of the purchase science screen!"
    if !plant(9250) {
        bar.hide_purchase_science(h);
    }
}

/// ControlBarCallback.cpp's `ToggleControlBar(immediate)`: "Toggle the control bar on or off".
pub fn toggle_control_bar(bar: &mut ControlBarStage, immediate: bool, h: &mut impl ControlBarStageHost) {
    toggle_replay_controls(h);
    let Some(window) = h.win_from_name(CONTROL_BAR_PARENT) else { return };
    if h.win_is_hidden(window) {
        h.show_special_power_shortcut();
        // "now hidden, we're making it visible again so shrink viewport under the window"
        h.set_tactical_view_height((h.display_height() as f32 * 0.80f32) as i32);
        let hidden = h.win_is_hidden(window);
        h.win_hide(window, !hidden);
        if !plant(9251) {
            bar.switch_control_bar_stage(ControlBarStages::Default as i32, h);
        }
        if h.has_animate_window_manager() && (!immediate || plant(9252)) {
            h.animate_reset();
            h.animate_register_slide_bottom(window);
            h.animate_special_power_shortcut(true);
        }
    } else {
        h.hide_special_power_shortcut();
        h.set_tactical_view_height(h.display_height() as i32);
        let hidden = h.win_is_hidden(window);
        h.win_hide(window, !hidden && !plant(9253));
    }
}

/// What `MSG_META_TOGGLE_CONTROL_BAR` reaches: the shell's top layout, the recorder, and `ToggleControlBar`.
pub trait ToggleControlBarHost {
    /// `TheShell->isShellActive()`.
    fn shell_is_active(&self) -> bool;
    /// `TheShell->top()`'s `isHidden()`; `None` when the shell has no layout.
    fn shell_top_is_hidden(&self) -> Option<bool>;
    /// `TheShell->top()->hide(hide)`.
    fn shell_top_hide(&mut self, hide: bool);
    /// `TheRecorder && TheRecorder->getMode() == RECORDERMODETYPE_PLAYBACK`.
    fn toggle_bar_recorder_playback(&self) -> bool;
    /// `ToggleControlBar()` (its default `immediate = TRUE`, GUICallbacks.h:323): [`toggle_control_bar`] on the
    /// client's bar.
    fn toggle_control_bar(&mut self, immediate: bool);
}

/// CommandXlat.cpp:3168, `case GameMessage::MSG_META_TOGGLE_CONTROL_BAR` (see the module docs). Destroyed.
pub fn toggle_control_bar_meta(h: &mut impl ToggleControlBarHost) -> Disposition {
    if h.shell_is_active() {
        if let Some(hidden) = h.shell_top_is_hidden() {
            h.shell_top_hide(!hidden || plant(9254));
        }
    } else if !h.toggle_bar_recorder_playback() || plant(9255) {
        // (the case's `hide`, from `ControlBarParent`'s `winIsHidden`, is never read)
        h.toggle_control_bar(true);
    }
    Disposition::Destroy
}

/// `GadgetButtonSetEnabledImage`, `GadgetButtonSetHiliteImage` and `GadgetButtonSetHiliteSelectedImage` on a window of
/// `tree`, for a host's [`ControlBarStageHost::gadget_button_set_images`] (the images by name: the client looks
/// [`ToggleImage::scheme_field`] up in the side's scheme).
pub fn set_toggle_images(tree: &mut WindowTree, w: WinId, images: [Option<Vec<u8>>; 3]) {
    let [e, hi, hs] = images;
    gadget_button_set_enabled_image(tree, w, e);
    gadget_button_set_hilite_image(tree, w, hi);
    gadget_button_set_hilite_selected_image(tree, w, hs);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recorder of the host's calls over a few windows.
    #[derive(Default)]
    struct Rec {
        hidden: Vec<bool>,
        pos: Vec<(i32, i32)>,
        playback: bool,
        replay: bool,
        calls: Vec<String>,
    }

    impl ControlBarStageHost for Rec {
        fn win_from_name(&self, name: &[u8]) -> Option<WinId> {
            match name {
                CONTROL_BAR_PARENT => Some(0),
                BUTTON_LARGE => Some(1),
                _ => None,
            }
        }
        fn win_is_hidden(&self, w: WinId) -> bool {
            self.hidden[w]
        }
        fn win_hide(&mut self, w: WinId, hide: bool) {
            self.hidden[w] = hide;
        }
        fn win_get_position(&self, w: WinId) -> (i32, i32) {
            self.pos[w]
        }
        fn win_set_position(&mut self, w: WinId, x: i32, y: i32) {
            self.pos[w] = (x, y);
        }
        fn gadget_button_set_images(&mut self, _w: WinId, e: ToggleImage, _h: ToggleImage, _s: ToggleImage) {
            self.calls.push(format!("images {e:?}"));
        }
        fn display_height(&self) -> u32 {
            600
        }
        fn set_tactical_view_height(&mut self, h: i32) {
            self.calls.push(format!("view {h}"));
        }
        fn recorder_playback(&self) -> bool {
            self.playback
        }
        fn is_in_replay_game(&self) -> bool {
            self.replay
        }
        fn replay_window(&self) -> Option<WinId> {
            Some(2)
        }
        fn show_special_power_shortcut(&mut self) {}
        fn hide_special_power_shortcut(&mut self) {}
        fn animate_special_power_shortcut(&mut self, _on: bool) {}
        fn repopulate_build_tooltip_layout(&mut self) {}
        fn mark_ui_dirty(&mut self) {}
        fn has_animate_window_manager(&self) -> bool {
            false
        }
        fn animate_reset(&mut self) {}
        fn animate_register_slide_bottom(&mut self, _w: WinId) {}
        fn animate_reverse(&mut self) {}
    }

    /// Predicted: the bar at (0, 450); the toggle hides it (view 600) and shows it again at the default stage (view 480,
    /// the "down" images); the stage toggle lowers it to y 540 ("up" images) and raises it back; a replay's playback keeps
    /// the stage; the replay window shows only in a replay game.
    #[test]
    fn the_bar_hides_shows_lowers_and_raises() {
        let mut h = Rec { hidden: vec![false, false, true], pos: vec![(0, 450), (10, 0), (0, 0)], ..Default::default() };
        let mut bar = ControlBarStage::new(&h);
        assert_eq!(bar.default_position, (0, 450));
        toggle_control_bar(&mut bar, true, &mut h);
        assert!(h.hidden[0]);
        assert_eq!(h.calls, vec!["view 600"]);
        h.calls.clear();
        toggle_control_bar(&mut bar, true, &mut h);
        assert!(!h.hidden[0]);
        assert_eq!(h.calls, vec!["view 480", "view 480", "images DownOn"]);
        bar.toggle_control_bar_stage(&mut h);
        assert_eq!((bar.current, h.pos[0]), (ControlBarStages::Low, (0, 540)));
        assert_eq!(h.calls.last().map(String::as_str), Some("images UpOn"));
        bar.toggle_control_bar_stage(&mut h);
        assert_eq!((bar.current, h.pos[0]), (ControlBarStages::Default, (0, 450)));
        h.playback = true;
        bar.toggle_control_bar_stage(&mut h);
        assert_eq!(bar.current, ControlBarStages::Default);
        assert!(h.hidden[2]);
        h.replay = true;
        show_replay_controls(&mut h);
        assert!(!h.hidden[2]);
        hide_control_bar(&mut bar, true, &mut h);
        assert!(h.hidden[0] && h.hidden[2]);
        show_control_bar(&mut bar, true, &mut h);
        assert!(!h.hidden[0] && !h.hidden[2]);
    }
}
