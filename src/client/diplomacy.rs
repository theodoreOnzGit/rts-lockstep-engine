// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Diplomacy.cpp (grabWindowPointers,
//     releaseWindowPointers, updateFunc, theBriefingList, GetBriefingTextList, UpdateDiplomacyBriefingText,
//     ShowDiplomacy, ResetDiplomacy, HideDiplomacy, ToggleDiplomacy, DiplomacyInput, DiplomacySystem; lines 68-460)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Diplomacy.h, GUICallbacks.h (the declarations and default arguments)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (the callers: militarySubtitle, popupMessage, reset)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarCallback.cpp (the communicator button's
//     ToggleDiplomacy(FALSE), line 399)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GameClient.cpp (#151: GameClient::xfer's briefing list, lines 1555-1587)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #148: the briefing history, `NonCommand_BriefingHistory`'s window (`Diplomacy.wnd`).
//!
//! # As upstream does it
//!
//! - **The button.** Outside a multiplayer game the control bar's communicator (`ControlBar.wnd:PopupCommunicator`) holds
//!   `NonCommand_BriefingHistory` (`setControlBarSchemeByPlayer`, #146); whichever command it holds, a click on it is
//!   `ControlBarSystem`'s `ToggleDiplomacy(FALSE)` (ControlBarCallback.cpp:399).
//! - **The text.** `theBriefingList`, the labels of the mission's briefings in order: `InGameUI::militarySubtitle`
//!   (the script action `MILITARY_CAPTION`, "Scripting_/Briefing/Show military briefing caption.") and
//!   `InGameUI::popupMessage` (`POPUP_MESSAGE`) each give their label to `UpdateDiplomacyBriefingText(label, FALSE)`
//!   ([`update_diplomacy_briefing_text`]): an empty or repeated label is not added; a new one is, and goes into the
//!   open window's `ListboxSolo` translated (`TheGameText->fetch`), in the in-game UI's message colours by turn
//!   (`getMessageColor(numEntries % 2)`). `InGameUI::reset` clears the list (`UpdateDiplomacyBriefingText("", TRUE)`),
//!   and a saved game carries it (`GameClient::xfer`, version 2: [`xfer_briefing_list`], #151).
//! - **The window.** `ShowDiplomacy` (Diplomacy.cpp:199, [`show_diplomacy`]) makes `Diplomacy.wnd` the first time (and, in
//!   a single player game, fills its `ListboxSolo` from the list), shows it, shows the solo parent (the multiplayer
//!   game's in-game parent instead), and slides it in from the top unless `immediate`. `HideDiplomacy`
//!   ([`hide_diplomacy`]) hides and disables it, or slides it back out; `ToggleDiplomacy` ([`toggle_diplomacy`]) does one or
//!   the other by whether it is hidden; `ResetDiplomacy` ([`reset_diplomacy`], `GameLogic::closeWindows`) destroys it.
//!   Escape (`DiplomacyInput`) and its `ButtonHide` (`DiplomacySystem`) hide it.
//!
//! # Representation, and where this differs from upstream
//!
//! [`Diplomacy`] holds the file's statics; what they are called on is the host's ([`DiplomacyHost`]): the window
//! manager, the layout, the list box, the text, the animation manager and the game's state, each named as upstream's
//! call. The client's host is the shell's window manager ([`WindowManagerDiplomacy`]).
//!
//! - **Not translated:** the multiplayer rows (`PopulateInGameDiplomacyPopup`, `TheGameInfo`'s slots and mutes: the
//!   host's call, recorded), and the GameSpy buddy controls (`TheGameSpyInfo` is never made here: `BuddyControlSystem`
//!   handles nothing, and the buddy branch of `ShowDiplomacy` is not taken).
//! - Upstream calls `radioButtonInGame->winHide(TRUE)` and the parents' `winHide` without checking for `NULL`: a
//!   `Diplomacy.wnd` without them would crash. Here the call is skipped.

use crate::client::window_manager::MsgHandled;
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// `MAX_SLOTS` (GameInfo.h).
pub const MAX_SLOTS: usize = 8;

/// What the window reads of the game (the owner's copy).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiplomacyGame {
    /// `TheInGameUI->getInputEnabled()`.
    pub input_enabled: bool,
    /// `TheGameLogic->isIntroMoviePlaying()`.
    pub intro_movie_playing: bool,
    /// `TheGameLogic->isLoadingMap()`.
    pub loading_map: bool,
    /// `TheInGameUI->isQuitMenuVisible()`.
    pub quit_menu_visible: bool,
    /// `TheDisconnectMenu && TheDisconnectMenu->isScreenVisible()`.
    pub disconnect_screen_visible: bool,
    /// `TheRecorder->isMultiplayer()`.
    pub multiplayer: bool,
    /// `TheGlobalData->m_animateWindows`.
    pub animate_windows: bool,
    /// `TheInGameUI->getMessageColor(FALSE)` and `getMessageColor(TRUE)` (`m_messageColor1`, `m_messageColor2`).
    pub message_colors: [i32; 2],
}

impl Default for DiplomacyGame {
    /// A single player game with the input on, `GlobalData`'s `m_animateWindows` (`TRUE`) and `InGameUI::InGameUI`'s
    /// message colours (white, and 180 grey).
    fn default() -> Self {
        DiplomacyGame {
            input_enabled: true,
            intro_movie_playing: false,
            loading_map: false,
            quit_menu_visible: false,
            disconnect_screen_visible: false,
            multiplayer: false,
            animate_windows: true,
            message_colors: [crate::common::ini::game_make_color(255, 255, 255, 255), crate::common::ini::game_make_color(180, 180, 180, 255)],
        }
    }
}

/// Diplomacy.cpp's statics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diplomacy {
    /// `theBriefingList` (`GetBriefingTextList()`): the labels, in order.
    pub briefing_list: Vec<Vec<u8>>,
    /// `theLayout`, `theWindow`.
    pub the_layout: Option<usize>,
    pub the_window: Option<WinId>,
    /// `theAnimateWindowManager != NULL` (the manager is the host's).
    pub the_animate_window_manager: bool,
    /// `radioButtonInGame`, `radioButtonBuddies`, `winInGame`, `winBuddies`, `winSolo`.
    pub radio_button_in_game: Option<WinId>,
    pub radio_button_buddies: Option<WinId>,
    pub win_in_game: Option<WinId>,
    pub win_buddies: Option<WinId>,
    pub win_solo: Option<WinId>,
    /// `staticTextPlayer`, `staticTextSide`, `staticTextTeam`, `staticTextStatus`, `buttonMute`, `buttonUnMute` (by
    /// `grabWindowPointers`), and `slotNumInRow`.
    pub slot_windows: [[Option<WinId>; 6]; MAX_SLOTS],
    pub slot_num_in_row: [i32; MAX_SLOTS],
    /// What the window reads of the game.
    pub game: DiplomacyGame,
}

/// What the window's code calls (Diplomacy.cpp's globals), each named as upstream's call. `name` is the window's
/// `NAMEKEY` name (`"Diplomacy.wnd:ListboxSolo"`).
pub trait DiplomacyHost {
    /// Diplomacy.cpp's statics.
    fn diplomacy(&mut self) -> &mut Diplomacy;
    /// `NAMEKEY(name)` (`TheNameKeyGenerator->nameToKey(name)`): a window's id.
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `TheWindowManager->winGetWindowFromId(parent, NAMEKEY(name))`.
    fn win_get_window_from_id(&mut self, parent: Option<WinId>, name: &[u8]) -> Option<WinId>;
    /// `TheWindowManager->winCreateLayout(file)`.
    fn win_create_layout(&mut self, file: &[u8]) -> Option<usize>;
    /// `layout->getFirstWindow()`.
    fn layout_first_window(&mut self, layout: usize) -> Option<WinId>;
    /// `layout->hide(hide)`.
    fn layout_hide(&mut self, layout: usize, hide: bool);
    /// `layout->destroyWindows()` and `deleteInstance()`.
    fn layout_destroy(&mut self, layout: usize);
    /// `window->winHide(hide)`, `winEnable(enable)`, `winIsHidden()`.
    fn win_hide(&mut self, w: WinId, hide: bool);
    fn win_enable(&mut self, w: WinId, enable: bool);
    fn win_is_hidden(&mut self, w: WinId) -> bool;
    /// `GadgetListBoxReset`, `GadgetListBoxGetNumEntries`, `GadgetListBoxAddEntryText(w, text, color, -1)`.
    fn list_box_reset(&mut self, w: WinId);
    fn list_box_get_num_entries(&mut self, w: WinId) -> i32;
    fn list_box_add_entry_text(&mut self, w: WinId, text: &[u16], color: i32);
    /// `GadgetRadioSetSelection(w, FALSE)`.
    fn radio_set_selection(&mut self, w: WinId);
    /// `TheGameText->fetch(label)`.
    fn fetch(&mut self, label: &[u8]) -> Vec<u16>;
    /// `NEW AnimateWindowManager`, `delete theAnimateWindowManager`, and its `reset()`, `update()`, `isFinished()`,
    /// `isReversed()`, `reverseAnimateWindow()` and `registerGameWindow(w, WIN_ANIMATION_SLIDE_TOP, TRUE, 200)`.
    fn animate_new(&mut self);
    fn animate_delete(&mut self);
    fn animate_reset(&mut self);
    fn animate_update(&mut self);
    fn animate_is_finished(&mut self) -> bool;
    fn animate_is_reversed(&mut self) -> bool;
    fn animate_reverse(&mut self);
    fn animate_register_slide_top(&mut self, w: WinId);
    /// `TheInGameUI->registerWindowLayout(layout)`, `unregisterWindowLayout(layout)`.
    fn register_window_layout(&mut self, layout: usize, register: bool);
    /// `HideQuitMenu()`.
    fn hide_quit_menu(&mut self);
    /// `PopulateInGameDiplomacyPopup()` (not translated: the multiplayer rows).
    fn populate_in_game_diplomacy_popup(&mut self);
}

/// `grabWindowPointers()`: the eight slots' rows (`Diplomacy.wnd:StaticTextPlayer%d`, ...), `slotNumInRow` -1.
fn grab_window_pointers<H: DiplomacyHost>(h: &mut H) {
    const NAMES: [&str; 6] = ["StaticTextPlayer", "StaticTextSide", "StaticTextTeam", "StaticTextStatus", "ButtonMute", "ButtonUnMute"];
    let the_window = h.diplomacy().the_window;
    for i in 0..MAX_SLOTS {
        for (k, n) in NAMES.iter().enumerate() {
            let w = h.win_get_window_from_id(the_window, format!("Diplomacy.wnd:{n}{i}").as_bytes());
            h.diplomacy().slot_windows[i][k] = w;
        }
        h.diplomacy().slot_num_in_row[i] = -1;
    }
}

/// `releaseWindowPointers()`.
fn release_window_pointers<H: DiplomacyHost>(h: &mut H) {
    let d = h.diplomacy();
    for i in 0..MAX_SLOTS {
        d.slot_windows[i] = [None; 6];
        d.slot_num_in_row[i] = -1;
    }
}

/// `updateFunc(layout, param)`, the layout's update: the slide, and the window hidden when the slide out ends.
pub fn update_func<H: DiplomacyHost>(h: &mut H) {
    let d = h.diplomacy();
    if d.the_animate_window_manager && d.game.animate_windows {
        let was_finished = h.animate_is_finished();
        h.animate_update();
        if h.animate_is_finished() && !was_finished && h.animate_is_reversed() {
            if let Some(w) = h.diplomacy().the_window {
                h.win_hide(w, true);
            }
        }
    }
}

/// `GetBriefingTextList()`.
pub fn get_briefing_text_list<H: DiplomacyHost>(h: &mut H) -> &[Vec<u8>] {
    &h.diplomacy().briefing_list
}

/// `UpdateDiplomacyBriefingText(newText, clear)` (Diplomacy.cpp:170).
pub fn update_diplomacy_briefing_text<H: DiplomacyHost>(h: &mut H, new_text: &[u8], clear: bool) {
    let the_window = h.diplomacy().the_window;
    let listbox_solo = h.win_get_window_from_id(the_window, b"Diplomacy.wnd:ListboxSolo");
    if clear {
        h.diplomacy().briefing_list.clear();
        if let Some(l) = listbox_solo {
            if !plant(9412) {
                h.list_box_reset(l);
            }
        }
    }
    if new_text.is_empty() && !plant(9418) {
        return;
    }
    if h.diplomacy().briefing_list.iter().any(|t| t == new_text) && !plant(9411) {
        return;
    }
    h.diplomacy().briefing_list.push(new_text.to_vec());
    let Some(listbox_solo) = listbox_solo else { return };
    let translated = h.fetch(new_text);
    add_entry(h, listbox_solo, &translated);
}

/// `GadgetListBoxAddEntryText(listboxSolo, translated, TheInGameUI->getMessageColor(numEntries%2), -1)`.
fn add_entry<H: DiplomacyHost>(h: &mut H, listbox_solo: WinId, translated: &[u16]) {
    let num_entries = h.list_box_get_num_entries(listbox_solo);
    let alt = if plant(9413) { 0 } else { num_entries % 2 };
    let color = h.diplomacy().game.message_colors[usize::from(alt != 0)];
    h.list_box_add_entry_text(listbox_solo, translated, color);
}

/// `ShowDiplomacy(immediate)` (Diplomacy.cpp:199).
pub fn show_diplomacy<H: DiplomacyHost>(h: &mut H, immediate: bool) {
    let g = h.diplomacy().game;
    if (!g.input_enabled && !plant(9415)) || g.intro_movie_playing || g.loading_map {
        return;
    }
    if g.quit_menu_visible {
        return;
    }
    if g.disconnect_screen_visible {
        return;
    }
    if let Some(w) = h.diplomacy().the_window {
        h.win_hide(w, false);
        h.win_enable(w, true);
    } else {
        let layout = h.win_create_layout(b"Diplomacy.wnd");
        let the_window = layout.and_then(|l| h.layout_first_window(l));
        let d = h.diplomacy();
        d.the_layout = layout;
        d.the_window = the_window;
        // (`theLayout->setUpdate(updateFunc)`: the host runs [`update_func`] each frame while the layout is up)
        h.animate_new();
        h.diplomacy().the_animate_window_manager = true;
        let radio_in_game = h.win_get_window_from_id(None, b"Diplomacy.wnd:RadioButtonInGame");
        let radio_buddies = h.win_get_window_from_id(None, b"Diplomacy.wnd:RadioButtonBuddies");
        let win_in_game = h.win_get_window_from_id(None, b"Diplomacy.wnd:InGameParent");
        let win_buddies = h.win_get_window_from_id(None, b"Diplomacy.wnd:BuddiesParent");
        let win_solo = h.win_get_window_from_id(None, b"Diplomacy.wnd:SoloParent");
        let d = h.diplomacy();
        (d.radio_button_in_game, d.radio_button_buddies, d.win_in_game, d.win_buddies, d.win_solo) = (radio_in_game, radio_buddies, win_in_game, win_buddies, win_solo);
        if !g.multiplayer {
            let listbox_solo = h.win_get_window_from_id(the_window, b"Diplomacy.wnd:ListboxSolo");
            if let Some(listbox_solo) = listbox_solo {
                for label in h.diplomacy().briefing_list.clone() {
                    let translated = h.fetch(&label);
                    add_entry(h, listbox_solo, &translated);
                }
            }
        }
    }
    let Some(layout) = h.diplomacy().the_layout else { return };
    h.layout_hide(layout, false);
    let d = h.diplomacy().clone();
    if let Some(w) = d.radio_button_in_game {
        h.win_hide(w, true);
    }
    if let Some(w) = d.radio_button_buddies {
        h.win_hide(w, true);
    }
    if let Some(w) = d.radio_button_in_game {
        h.radio_set_selection(w);
    }
    let solo = g.multiplayer == plant(9417);
    let hide = |h: &mut H, w: Option<WinId>, hide: bool| {
        if let Some(w) = w {
            h.win_hide(w, hide);
        }
    };
    if !solo {
        hide(h, d.win_in_game, false);
        hide(h, d.win_buddies, true);
        hide(h, d.win_solo, true);
    } else {
        hide(h, d.win_in_game, true);
        hide(h, d.win_buddies, true);
        hide(h, d.win_solo, false);
    }
    h.animate_reset();
    if !immediate && g.animate_windows {
        if let Some(w) = d.the_window {
            h.animate_register_slide_top(w);
        }
    }
    h.register_window_layout(layout, true);
    grab_window_pointers(h);
    h.populate_in_game_diplomacy_popup();
    // (`TheGameSpyInfo && TheGameSpyInfo->getLocalProfileID() != 0`: never made here, see the module docs)
}

/// `ResetDiplomacy()` (Diplomacy.cpp:287).
pub fn reset_diplomacy<H: DiplomacyHost>(h: &mut H) {
    if let Some(l) = h.diplomacy().the_layout {
        h.register_window_layout(l, false);
        h.layout_destroy(l);
        // (`InitBuddyControls(-1)`: the buddy controls, not made here)
    }
    let d = h.diplomacy();
    d.the_layout = None;
    d.the_window = None;
    if d.the_animate_window_manager {
        h.animate_delete();
    }
    h.diplomacy().the_animate_window_manager = false;
}

/// `HideDiplomacy(immediate)` (Diplomacy.cpp:305; `immediate` defaults to `TRUE`).
pub fn hide_diplomacy<H: DiplomacyHost>(h: &mut H, immediate: bool) {
    release_window_pointers(h);
    let d = h.diplomacy();
    let (the_window, animate_windows) = (d.the_window, d.game.animate_windows);
    if let Some(w) = the_window {
        if immediate || !animate_windows {
            h.win_hide(w, true);
            if !plant(9416) {
                h.win_enable(w, false);
            }
        } else if h.animate_is_finished() {
            h.animate_reverse();
        }
    }
}

/// `ToggleDiplomacy(immediate)` (Diplomacy.cpp:325; `immediate` defaults to `TRUE`, the communicator button passes
/// `FALSE`).
pub fn toggle_diplomacy<H: DiplomacyHost>(h: &mut H, immediate: bool) {
    // "If we bring this up, let's hide the quit menu"
    h.hide_quit_menu();
    if let Some(w) = h.diplomacy().the_window {
        let show = h.win_is_hidden(w) != plant(9414);
        if show {
            show_diplomacy(h, immediate);
        } else {
            hide_diplomacy(h, immediate);
        }
    } else {
        show_diplomacy(h, immediate);
    }
}

/// `DiplomacyInput(window, msg, mData1, mData2)` (Diplomacy.cpp:347): a key (`GWM_CHAR`, `key` its `mData1`) is
/// handled; Escape hides the window (`HideDiplomacy()`, immediate).
pub fn diplomacy_input<H: DiplomacyHost>(h: &mut H, msg: u32, key: u8) -> MsgHandled {
    use crate::client::window_manager::gwm;
    if msg == gwm::CHAR {
        // `KEY_ESC`
        if key == 0x01 {
            hide_diplomacy(h, true);
            return MsgHandled::Handled;
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `DiplomacySystem(window, msg, mData1, mData2)` (Diplomacy.cpp:384) for the messages it takes: `GGM_FOCUS_CHANGE`,
/// `GWM_INPUT_FOCUS` (`input_focus`: `mData1 == TRUE` and the `Bool` `mData2` points to; when offered the focus it is
/// refused, `*(Bool *)mData2 = FALSE`), and `GBM_SELECTED` of the control whose
/// window id (`winGetWindowId()`) is `control_id`: `ButtonHide` hides the window (`HideDiplomacy(FALSE)`), the two radio
/// buttons show their parent.
/// `BuddyControlSystem` (GameSpy) handles nothing here; the mute buttons need a slot (`slotNumInRow`), which only the
/// multiplayer rows give.
pub fn diplomacy_system<H: DiplomacyHost>(h: &mut H, msg: u32, control_id: i32, input_focus: Option<(bool, &mut bool)>) -> MsgHandled {
    use crate::client::window_manager::{ggm, gwm};
    match msg {
        ggm::FOCUS_CHANGE => MsgHandled::Handled,
        gwm::INPUT_FOCUS => {
            // "if we're given the opportunity to take the keyboard focus we must say we don't want it"
            if let Some((true, f)) = input_focus {
                *f = false;
            }
            MsgHandled::Handled
        }
        ggm::GBM_SELECTED => {
            let d = h.diplomacy().clone();
            let hide = |h: &mut H, w: Option<WinId>, hide: bool| {
                if let Some(w) = w {
                    h.win_hide(w, hide);
                }
            };
            let button_hide_id = h.name_to_key(b"Diplomacy.wnd:ButtonHide");
            let (radio_in_game_id, radio_buddies_id) = (h.name_to_key(b"Diplomacy.wnd:RadioButtonInGame"), h.name_to_key(b"Diplomacy.wnd:RadioButtonBuddies"));
            if control_id == button_hide_id {
                hide_diplomacy(h, false);
            } else if control_id == radio_in_game_id {
                hide(h, d.win_in_game, false);
                hide(h, d.win_buddies, true);
            } else if control_id == radio_buddies_id {
                hide(h, d.win_in_game, true);
                hide(h, d.win_buddies, false);
            }
            MsgHandled::Handled
        }
        _ => MsgHandled::Ignored,
    }
}

/// (#151) A [`DiplomacyHost`] with no window manager: only `theBriefingList` (Diplomacy.cpp's static), for a save's
/// `GameClient::xfer` when the briefing history's window is not the caller's to give (`crate::client::save_game`). Every
/// `winGetWindowFromId` finds nothing, so `UpdateDiplomacyBriefingText` keeps the list as it does before
/// `Diplomacy.wnd` is made.
pub struct BriefingListOnly<'a>(pub &'a mut Diplomacy);

impl DiplomacyHost for BriefingListOnly<'_> {
    fn diplomacy(&mut self) -> &mut Diplomacy {
        self.0
    }
    fn name_to_key(&mut self, _name: &[u8]) -> i32 {
        0
    }
    fn win_get_window_from_id(&mut self, _parent: Option<WinId>, _name: &[u8]) -> Option<WinId> {
        None
    }
    fn win_create_layout(&mut self, _file: &[u8]) -> Option<usize> {
        None
    }
    fn layout_first_window(&mut self, _layout: usize) -> Option<WinId> {
        None
    }
    fn layout_hide(&mut self, _layout: usize, _hide: bool) {}
    fn layout_destroy(&mut self, _layout: usize) {}
    fn win_hide(&mut self, _w: WinId, _hide: bool) {}
    fn win_enable(&mut self, _w: WinId, _enable: bool) {}
    fn win_is_hidden(&mut self, _w: WinId) -> bool {
        true
    }
    fn list_box_reset(&mut self, _w: WinId) {}
    fn list_box_get_num_entries(&mut self, _w: WinId) -> i32 {
        0
    }
    fn list_box_add_entry_text(&mut self, _w: WinId, _text: &[u16], _color: i32) {}
    fn radio_set_selection(&mut self, _w: WinId) {}
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        label.iter().map(|&b| u16::from(b)).collect()
    }
    fn animate_new(&mut self) {}
    fn animate_delete(&mut self) {}
    fn animate_reset(&mut self) {}
    fn animate_update(&mut self) {}
    fn animate_is_finished(&mut self) -> bool {
        true
    }
    fn animate_is_reversed(&mut self) -> bool {
        false
    }
    fn animate_reverse(&mut self) {}
    fn animate_register_slide_top(&mut self, _w: WinId) {}
    fn register_window_layout(&mut self, _layout: usize, _register: bool) {}
    fn hide_quit_menu(&mut self) {}
    fn populate_in_game_diplomacy_popup(&mut self) {}
}

/// (#151) `GameClient::xfer`'s mission briefing history (GameClient.cpp:1555-1587, save version 2 and later): a save
/// writes `GetBriefingTextList()`'s length (`xferInt`) and each label (`xferAsciiString`); a load reads the length,
/// clears the list (`UpdateDiplomacyBriefingText(AsciiString::TheEmptyString, TRUE)`), then gives each label read to
/// `UpdateDiplomacyBriefingText(label, FALSE)`, so an empty or repeated one is dropped and the open window's list box gets
/// each new one. A count of 0 or less reads no label. Before version 2 nothing is read or written.
pub fn xfer_briefing_list<H: DiplomacyHost>(xfer: &mut dyn crate::common::xfer::Xfer, version: crate::common::xfer::XferVersion, h: &mut H) -> crate::common::xfer::XferResult<()> {
    if version < 2 {
        return Ok(());
    }
    if xfer.get_xfer_mode() == crate::common::xfer::XferMode::Save {
        let list = get_briefing_text_list(h).to_vec();
        let mut num_entries = list.len() as i32;
        if plant(9423) {
            num_entries = num_entries.min(1);
        }
        xfer.xfer_int(&mut num_entries)?;
        for label in list {
            let mut temp_str = label;
            xfer.xfer_ascii_string(&mut temp_str)?;
        }
    } else {
        let mut num_entries = 0i32;
        xfer.xfer_int(&mut num_entries)?;
        if !plant(9424) {
            update_diplomacy_briefing_text(h, b"", true);
        }
        while num_entries > 0 {
            num_entries -= 1;
            let mut temp_str = Vec::new();
            xfer.xfer_ascii_string(&mut temp_str)?;
            if plant(9425) {
                h.diplomacy().briefing_list.push(temp_str);
            } else {
                update_diplomacy_briefing_text(h, &temp_str, false);
            }
        }
    }
    Ok(())
}

/// The client's [`DiplomacyHost`] on a window manager (the shell's, which draws `Diplomacy.wnd` from the player's files):
/// the layout made by the shell's host, the name keys its, the animation manager the window's own.
pub struct WindowManagerDiplomacy<'a, H: crate::client::shell::ShellHost, F: Fn(&H, &[u8]) -> Vec<u16>> {
    pub wm: &'a mut crate::client::window_manager::GameWindowManager,
    pub host: &'a mut H,
    /// `TheGameText->fetch(label)`, on the host.
    pub fetch: F,
    pub diplomacy: &'a mut Diplomacy,
    pub animate: &'a mut Option<crate::client::animate_window::AnimateWindowManager>,
    /// The quit menu is to be hidden (`HideQuitMenu()`), for the owner to do on its quit menu.
    pub hide_quit_menu: bool,
}

impl<H: crate::client::shell::ShellHost, F: Fn(&H, &[u8]) -> Vec<u16>> DiplomacyHost for WindowManagerDiplomacy<'_, H, F> {
    fn diplomacy(&mut self) -> &mut Diplomacy {
        self.diplomacy
    }
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        self.host.name_to_key(name) as i32
    }
    fn win_get_window_from_id(&mut self, parent: Option<WinId>, name: &[u8]) -> Option<WinId> {
        let id = self.host.name_to_key(name) as i32;
        self.wm.win_get_window_from_id(parent, id)
    }
    fn win_create_layout(&mut self, file: &[u8]) -> Option<usize> {
        self.host.create_layout(self.wm, file)
    }
    fn layout_first_window(&mut self, layout: usize) -> Option<WinId> {
        // (`m_windowList`'s head)
        self.wm.layouts[layout].windows.first().copied()
    }
    fn layout_hide(&mut self, layout: usize, hide: bool) {
        self.wm.layout_hide(layout, hide);
    }
    fn layout_destroy(&mut self, layout: usize) {
        crate::client::shell::destroy_layout(self.wm, layout);
    }
    fn win_hide(&mut self, w: WinId, hide: bool) {
        self.wm.win_hide(w, hide);
    }
    fn win_enable(&mut self, w: WinId, enable: bool) {
        self.wm.win_enable(w, enable);
    }
    fn win_is_hidden(&mut self, w: WinId) -> bool {
        self.wm.tree.windows[w].status & crate::client::window_script::win_status::HIDDEN != 0
    }
    fn list_box_reset(&mut self, w: WinId) {
        crate::client::gadget_list_box::reset(self.wm, self.host, w);
    }
    fn list_box_get_num_entries(&mut self, w: WinId) -> i32 {
        crate::client::gadget_list_box::get_num_entries(self.wm, w)
    }
    fn list_box_add_entry_text(&mut self, w: WinId, text: &[u16], color: i32) {
        crate::client::gadget_list_box::add_entry_text(self.wm, self.host, w, text, color, -1, 0, false);
    }
    fn radio_set_selection(&mut self, w: WinId) {
        crate::client::gadget::radio_set_selection(self.wm, self.host, w, false);
    }
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        (self.fetch)(self.host, label)
    }
    fn animate_new(&mut self) {
        *self.animate = Some(crate::client::animate_window::AnimateWindowManager::new(self.wm.display.0));
    }
    fn animate_delete(&mut self) {
        *self.animate = None;
    }
    fn animate_reset(&mut self) {
        if let Some(a) = self.animate.as_mut() {
            a.reset(self.wm);
        }
    }
    fn animate_update(&mut self) {
        let now = self.host.time_ms();
        if let Some(a) = self.animate.as_mut() {
            a.update(self.wm, now);
        }
    }
    fn animate_is_finished(&mut self) -> bool {
        self.animate.as_ref().is_none_or(|a| a.is_finished())
    }
    fn animate_is_reversed(&mut self) -> bool {
        self.animate.as_ref().is_some_and(|a| a.is_reversed())
    }
    fn animate_reverse(&mut self) {
        let now = self.host.time_ms();
        if let Some(a) = self.animate.as_mut() {
            a.reverse_animate_window(self.wm, now);
        }
    }
    fn animate_register_slide_top(&mut self, w: WinId) {
        let now = self.host.time_ms();
        if let Some(a) = self.animate.as_mut() {
            a.register_game_window(self.wm, Some(w), crate::client::animate_window::anim::WIN_ANIMATION_SLIDE_TOP, true, 200, 0, now);
        }
    }
    fn register_window_layout(&mut self, _layout: usize, _register: bool) {
        // (`TheInGameUI`'s layout list: the shell's window manager draws and updates every layout it holds)
    }
    fn hide_quit_menu(&mut self) {
        self.hide_quit_menu = true;
    }
    fn populate_in_game_diplomacy_popup(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host on a table of windows (as the differential test's), recording nothing.
    #[derive(Default)]
    struct T {
        d: Diplomacy,
        made: bool,
        hidden: Vec<bool>,
        entries: Vec<(Vec<u16>, i32)>,
        quit_hidden: u32,
        finished: bool,
    }
    const NAMES: [&str; 3] = ["Diplomacy.wnd:DiplomacyParent", "Diplomacy.wnd:ListboxSolo", "Diplomacy.wnd:SoloParent"];
    impl DiplomacyHost for T {
        fn diplomacy(&mut self) -> &mut Diplomacy {
            &mut self.d
        }
        fn name_to_key(&mut self, name: &[u8]) -> i32 {
            NAMES.iter().position(|n| n.as_bytes() == name).map_or(-1, |i| i as i32)
        }
        fn win_get_window_from_id(&mut self, _p: Option<WinId>, name: &[u8]) -> Option<WinId> {
            if !self.made {
                return None;
            }
            NAMES.iter().position(|n| n.as_bytes() == name)
        }
        fn win_create_layout(&mut self, _f: &[u8]) -> Option<usize> {
            self.made = true;
            self.hidden = vec![false; 3];
            Some(0)
        }
        fn layout_first_window(&mut self, _l: usize) -> Option<WinId> {
            Some(0)
        }
        fn layout_hide(&mut self, _l: usize, hide: bool) {
            self.hidden[0] = hide;
        }
        fn layout_destroy(&mut self, _l: usize) {
            self.made = false;
            self.entries.clear();
        }
        fn win_hide(&mut self, w: WinId, hide: bool) {
            self.hidden[w] = hide;
        }
        fn win_enable(&mut self, _w: WinId, _e: bool) {}
        fn win_is_hidden(&mut self, w: WinId) -> bool {
            self.hidden[w]
        }
        fn list_box_reset(&mut self, _w: WinId) {
            self.entries.clear();
        }
        fn list_box_get_num_entries(&mut self, _w: WinId) -> i32 {
            self.entries.len() as i32
        }
        fn list_box_add_entry_text(&mut self, _w: WinId, text: &[u16], color: i32) {
            self.entries.push((text.to_vec(), color));
        }
        fn radio_set_selection(&mut self, _w: WinId) {}
        fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
            label.iter().map(|&b| u16::from(b)).collect()
        }
        fn animate_new(&mut self) {}
        fn animate_delete(&mut self) {}
        fn animate_reset(&mut self) {}
        fn animate_update(&mut self) {}
        fn animate_is_finished(&mut self) -> bool {
            self.finished
        }
        fn animate_is_reversed(&mut self) -> bool {
            false
        }
        fn animate_reverse(&mut self) {}
        fn animate_register_slide_top(&mut self, _w: WinId) {}
        fn register_window_layout(&mut self, _l: usize, _r: bool) {}
        fn hide_quit_menu(&mut self) {
            self.quit_hidden += 1;
        }
        fn populate_in_game_diplomacy_popup(&mut self) {}
    }

    /// #148, predicted before the run: two briefings before the window exists are listed but not shown; a repeat and an
    /// empty label are not added; the communicator's toggle makes the window with both in the solo list box, white then
    /// grey, the solo parent shown, the quit menu hidden; a third briefing goes straight into the open box (white again,
    /// the third entry); the toggle again hides it (immediate: the animation not finished, so the slide back is not
    /// started; `immediate` hides it at once); clearing empties the list and the box.
    #[test]
    fn the_briefing_history_lists_each_briefing_once() {
        let mut h = T::default();
        update_diplomacy_briefing_text(&mut h, b"SCRIPT:Brief1", false);
        update_diplomacy_briefing_text(&mut h, b"SCRIPT:Brief2", false);
        update_diplomacy_briefing_text(&mut h, b"SCRIPT:Brief1", false);
        update_diplomacy_briefing_text(&mut h, b"", false);
        assert_eq!(h.d.briefing_list, vec![b"SCRIPT:Brief1".to_vec(), b"SCRIPT:Brief2".to_vec()]);
        toggle_diplomacy(&mut h, false);
        let [white, grey] = DiplomacyGame::default().message_colors;
        assert_eq!(h.entries.iter().map(|e| e.1).collect::<Vec<_>>(), vec![white, grey]);
        assert_eq!((h.hidden.clone(), h.quit_hidden), (vec![false, false, false], 1));
        update_diplomacy_briefing_text(&mut h, b"SCRIPT:Brief3", false);
        assert_eq!(h.entries.len(), 3);
        assert_eq!(h.entries[2].1, white);
        toggle_diplomacy(&mut h, true);
        assert!(h.hidden[0]);
        toggle_diplomacy(&mut h, true);
        assert!(!h.hidden[0]);
        update_diplomacy_briefing_text(&mut h, b"", true);
        assert!(h.d.briefing_list.is_empty() && h.entries.is_empty());
        assert_eq!(diplomacy_input(&mut h, crate::client::window_manager::gwm::CHAR, 0x01), MsgHandled::Handled);
        assert!(h.hidden[0]);
    }
}
