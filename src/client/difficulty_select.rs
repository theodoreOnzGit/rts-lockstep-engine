// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/DifficultySelect.cpp (the file's statics,
//     SetDifficultyRadioButton, DifficultySelectInit, DifficultySelectInput, DifficultySelectSystem)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/OptionsMenu.cpp
//     (OptionPreferences::getCampaignDifficulty)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   DifficultySelect.cpp: Chris Huybregts, November 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The campaign's difficulty popup (GitHub #78, item 3): `Menus/DifficultySelect.wnd`'s callbacks.
//!
//! # As upstream reads it
//!
//! **Init** finds `DifficultySelectParent`'s *OK*, *Cancel* and three radio buttons, sets the difficulty from the
//! options (`SetDifficultyRadioButton`: `OptionPreferences::getCampaignDifficulty`, the `CampaignDifficulty` entry of
//! `Options.ini` clamped to easy .. hard, or the script engine's global difficulty without one; its radio button
//! selected without a message), brings the parent to the top and makes it modal. **System** (`GBM_SELECTED`): a radio
//! button sets the difficulty; *OK* writes it to the options, destroys the layout and calls the main menu's
//! `setupGameStart(currentMap, difficulty)` (the campaign's start, or the challenge menu); *Cancel* clears the campaign,
//! undoes the modal and destroys the layout. **Input**: nothing (its Escape handling is commented out).
//!
//! **Not reachable in Zero Hour:** the main menu's calls that made this layout are commented out (its difficulty
//! panel took its place). It is translated as the lexicon names it; the tests make the layout themselves.
//!
//! # Upstream quirks that are kept
//!
//! - *OK* does not undo the modal (only *Cancel* calls `winUnsetModal`); `destroyWindows` removes it from the modal
//!   stack only as `winDestroy` does.
//! - Init resets the difficulty to normal before it reads the options.
//! - A window the file lacks: the radio buttons are checked (`GadgetRadioSetSelection` of `NULL` does nothing); the
//!   parent is not (`parent->winBringToTop()` on `NULL` is recorded as undefined).
//!
//! # Where this differs from upstream
//!
//! - `OptionPreferences` (`Options.ini`) is the host's to read ([`DifficultyHost::campaign_difficulty_pref`]) and is
//!   not written (`setCampaignDifficulty`, `write`): the main menu's campaign difficulty is not written either.
//!
//! # Representation
//!
//! [`DifficultySelect`]: the file's statics.

use crate::client::gadget::radio_set_selection;
use crate::client::main_menu::{self, MenuHost, DIFFICULTY_EASY, DIFFICULTY_HARD, DIFFICULTY_NORMAL};
use crate::client::window_manager::{data_win, ggm, gwm, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// What the difficulty popup needs besides the main menu's host.
pub trait DifficultyHost: MenuHost {
    /// DifficultySelect.cpp's statics.
    fn difficulty_select(&mut self) -> &mut DifficultySelect;
    /// `Options.ini`'s `CampaignDifficulty` entry, as `atoi` reads it (`None`: no entry).
    fn campaign_difficulty_pref(&self) -> Option<i32> {
        None
    }
    /// `TheScriptEngine->getGlobalDifficulty()`.
    fn global_difficulty(&self) -> i32 {
        DIFFICULTY_NORMAL
    }
}

/// DifficultySelect.cpp's statics (`NAMEKEY_INVALID` is 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DifficultySelect {
    pub ai_diff: i32,
    pub button_ok_id: i32,
    pub button_ok: Option<WinId>,
    pub button_cancel_id: i32,
    pub button_cancel: Option<WinId>,
    pub radio_easy_id: i32,
    pub radio_medium_id: i32,
    pub radio_hard_id: i32,
    pub radio_easy: Option<WinId>,
    pub radio_medium: Option<WinId>,
    pub radio_hard: Option<WinId>,
}

impl Default for DifficultySelect {
    /// The statics' initial values.
    fn default() -> Self {
        DifficultySelect {
            ai_diff: DIFFICULTY_NORMAL,
            button_ok_id: 0,
            button_ok: None,
            button_cancel_id: 0,
            button_cancel: None,
            radio_easy_id: 0,
            radio_medium_id: 0,
            radio_hard_id: 0,
            radio_easy: None,
            radio_medium: None,
            radio_hard: None,
        }
    }
}

/// `OptionPreferences::getCampaignDifficulty()`: the entry clamped to easy .. hard, or the global difficulty.
pub fn get_campaign_difficulty<H: DifficultyHost>(h: &H) -> i32 {
    match h.campaign_difficulty_pref() {
        None => h.global_difficulty(),
        Some(f) => f.clamp(DIFFICULTY_EASY, DIFFICULTY_HARD),
    }
}

/// `SetDifficultyRadioButton()` (`TheScriptEngine` always exists here).
fn set_difficulty_radio_button<H: DifficultyHost>(wm: &mut GameWindowManager, h: &mut H) {
    let d = h.difficulty_select().clone();
    let (radio, diff) = match get_campaign_difficulty(h) {
        DIFFICULTY_EASY => (d.radio_easy, DIFFICULTY_EASY),
        DIFFICULTY_NORMAL => (d.radio_medium, DIFFICULTY_NORMAL),
        DIFFICULTY_HARD => (d.radio_hard, DIFFICULTY_HARD),
        // (`DEBUG_CRASH`: nothing in the release)
        _ => return,
    };
    if let Some(r) = radio {
        radio_set_selection(wm, h, r, false);
    }
    h.difficulty_select().ai_diff = diff;
}

/// `DifficultySelectInit(layout, userData)`.
pub fn difficulty_select_init<H: DifficultyHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let mut k = |n: &str| h.name_to_key(format!("DifficultySelect.wnd:{n}").as_bytes()) as i32;
    let (parent_id, ok, cancel, easy, medium, hard) = (k("DifficultySelectParent"), k("ButtonOk"), k("ButtonCancel"), k("RadioButtonEasy"), k("RadioButtonMedium"), k("RadioButtonHard"));
    let parent = wm.win_get_window_from_id(None, parent_id);
    let d = h.difficulty_select();
    d.button_ok_id = ok;
    d.button_cancel_id = cancel;
    d.radio_easy_id = easy;
    d.radio_medium_id = medium;
    d.radio_hard_id = hard;
    let found = [ok, cancel, easy, medium, hard].map(|id| wm.win_get_window_from_id(parent, id));
    let d = h.difficulty_select();
    [d.button_ok, d.button_cancel, d.radio_easy, d.radio_medium, d.radio_hard] = found;
    d.ai_diff = DIFFICULTY_NORMAL;
    set_difficulty_radio_button(wm, h);
    let Some(p) = parent else {
        wm.set_undefined("DifficultySelectInit: no DifficultySelectParent (parent->winBringToTop)");
        return;
    };
    wm.win_bring_to_top(p);
    wm.win_set_modal(Some(p));
}

/// `DifficultySelectInput(window, msg, mData1, mData2)`: everything ignored.
pub fn difficulty_select_input<H: DifficultyHost>(_wm: &mut GameWindowManager, _h: &mut H, _window: WinId, _msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    MsgHandled::Ignored
}

/// `layout->destroyWindows(); layout->deleteInstance();` of the window's layout.
fn destroy_own_layout(wm: &mut GameWindowManager, window: WinId) {
    match wm.ext[window].layout {
        Some(l) => {
            wm.layout_destroy_windows(l);
            wm.layouts[l].deleted = true;
        }
        None => wm.set_undefined("DifficultySelectSystem: the window has no layout (layout->destroyWindows on NULL)"),
    }
}

/// `DifficultySelectSystem(window, msg, mData1, mData2)`.
pub fn difficulty_select_system<H: DifficultyHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let id = wm.tree.windows[control].id;
            let d = h.difficulty_select().clone();
            if id == d.button_ok_id {
                // (`OptionPreferences::setCampaignDifficulty`, `write`: not written)
                destroy_own_layout(wm, window);
                main_menu::setup_game_start(wm, h, d.ai_diff);
            } else if id == d.button_cancel_id {
                h.main_menu().campaign.clear();
                wm.win_unset_modal(Some(window));
                destroy_own_layout(wm, window);
            } else if id == d.radio_easy_id {
                h.difficulty_select().ai_diff = DIFFICULTY_EASY;
            } else if id == d.radio_medium_id {
                h.difficulty_select().ai_diff = DIFFICULTY_NORMAL;
            } else if id == d.radio_hard_id {
                h.difficulty_select().ai_diff = DIFFICULTY_HARD;
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::save_load_menu::tests::{booted, click, named, settle};
    use crate::client::shell::{ShellHost, ShellRequest};
    use crate::client::window_manager::WIN_STATE_SELECTED;

    /// #78: the difficulty popup, made by the test (Zero Hour's main menu no longer makes it) over the main menu with
    /// the campaign `USA` set, predicted from DifficultySelect.cpp before the run:
    ///
    /// - init with `CampaignDifficulty = 7` in the options: clamped to hard; the hard radio button selected (no
    ///   message), the difficulty hard; the parent on top and modal;
    /// - a click on *Easy*: the difficulty easy; *OK*: the layout destroyed, `setupGameStart(easy)`: the main menu's
    ///   `startGame`, and its update, once the animations and transitions are done, asks the host for
    ///   `StartCampaign { USA, easy }`: the same start as the main menu's own difficulty buttons;
    /// - made again with no options entry: the global difficulty (normal), the medium button; *Cancel*: the campaign
    ///   cleared, the modal undone, the layout destroyed.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn the_difficulty_popup_starts_the_campaign() {
        let (mut wm, mut h) = booted();
        h.main_menu.campaign = b"USA".to_vec();
        h.campaign_difficulty_pref = Some(7);
        let l = h.create_layout(&mut wm, b"Menus/DifficultySelect.wnd").expect("the popup loads");
        crate::client::shell::run_layout(&mut wm, &mut h, l, 0, None);
        let f = "DifficultySelect.wnd";
        let p = named(&wm, f, "DifficultySelectParent");
        assert_eq!(h.difficulty.ai_diff, DIFFICULTY_HARD);
        assert!(wm.tree.windows[named(&wm, f, "RadioButtonHard")].inst.state & WIN_STATE_SELECTED != 0);
        assert_eq!(wm.modal.first().copied(), Some(p));
        let easy = named(&wm, f, "RadioButtonEasy");
        click(&mut wm, &mut h, easy);
        assert_eq!(h.difficulty.ai_diff, DIFFICULTY_EASY);
        let ok = named(&wm, f, "ButtonOk");
        click(&mut wm, &mut h, ok);
        assert!(wm.layouts[l].deleted && h.main_menu.start_game && h.main_menu.difficulty == DIFFICULTY_EASY);
        settle(&mut wm, &mut h);
        assert_eq!(h.requests, [ShellRequest::StartCampaign { campaign: b"USA".to_vec(), difficulty: DIFFICULTY_EASY }]);
        // again, with no options entry, and Cancel
        h.campaign_difficulty_pref = None;
        let l = h.create_layout(&mut wm, b"Menus/DifficultySelect.wnd").expect("the popup loads");
        crate::client::shell::run_layout(&mut wm, &mut h, l, 0, None);
        assert_eq!(h.difficulty.ai_diff, DIFFICULTY_NORMAL);
        let p = wm.layouts[l].windows[0];
        let medium = wm.tree.windows[p].children.iter().copied().find(|&c| wm.tree.windows[c].name() == b"DifficultySelect.wnd:RadioButtonMedium").expect("medium");
        assert!(wm.tree.windows[medium].inst.state & WIN_STATE_SELECTED != 0);
        let cancel = wm.tree.windows[p].children.iter().copied().find(|&c| wm.tree.windows[c].name() == b"DifficultySelect.wnd:ButtonCancel").expect("cancel");
        click(&mut wm, &mut h, cancel);
        assert!(h.main_menu.campaign.is_empty() && wm.layouts[l].deleted);
        assert!(!wm.modal.contains(&p));
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert_eq!(difficulty_select_input(&mut wm, &mut h, p, 0, 0, 0), MsgHandled::Ignored);
    }
}
