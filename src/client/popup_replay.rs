// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/PopupReplay.cpp (the file's statics,
//     ShowReplaySavedPopup, closeSaveMenu, PopupReplayInit, PopupReplayShutdown, PopupReplayUpdate, PopupReplayInput,
//     saveReplay, reallySaveReplay, PopupReplaySystem)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   PopupReplay.cpp: Matthew D. Campbell, November 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The score screen's *Save Replay* popup (GitHub #98): `Menus/PopupReplay.wnd`, which the score screen makes once
//! (`getPopupReplayLayout`), runs the init of and shows when *Save Replay* is pressed, and whose update it calls while
//! the popup is shown.
//!
//! # As upstream reads it
//!
//! **Init** (`PopupReplayInit`): the four ids made; the parent `PopupReplayMenu` found anywhere and given the focus;
//! the *replay saved* window (`PopupReplaySaved`) found and hidden; the button frame enabled; the games list filled
//! with the replay folder's files (ReplayMenu.cpp's `PopulateReplayFileListbox`); the name entry emptied and given
//! the focus; *Save* disabled (the name is empty).
//!
//! **System**: a row selected (`GLM_SELECTED`, row 0 or more) puts its text in the name entry. *Save* (or the entry's
//! `GEM_EDIT_DONE`) with a name: `saveReplay(name)`: `GUI:LastReplay`'s text means the last replay's name, else the
//! name as typed (`AsciiString::translate`); the path is the replay folder, the name and `.rep`; an existing file asks
//! first (`GUI:OverwriteReplay`, its OK `reallySaveReplay`), else `reallySaveReplay` at once: the last replay's own
//! path is left alone (nothing happens); an existing file is deleted (a failure's text in a box, the list filled
//! again); the last replay (`LastReplayFileName`, the score screen's global) is copied to the path (a failure's text
//! in a box); the list filled again; *replay saved* shown and its timer started. *Back* closes the popup
//! (`closeSaveMenu`: its layout hidden) and enables the score screen's buttons again (`ScoreScreenEnableControls`).
//! The entry's text changing (`GEM_UPDATE_TEXT`) enables *Save* only with a name. **Update** (the score screen's,
//! while the popup is shown): once *replay saved* has been up 1000 ms, it is hidden, the popup closed, the score
//! screen's buttons enabled again, the timer cleared. **Input**: Escape's release presses *Back*. **Shutdown**:
//! `parent` cleared.
//!
//! # Upstream quirks that are kept
//!
//! - `GEM_UPDATE_TEXT` has no `break`: it falls into `default` and answers `MSG_IGNORED`.
//! - Saving under the last replay's own name (`GUI:LastReplay`'s text, or `00000000`) does nothing: no popup, the
//!   popup stays up; the comparison of the two paths is with case (`AsciiString ==`).
//! - The *replay saved* timer is `timeGetTime()` itself: a save at time 0 never takes the popup down.
//! - `GLM_SELECTED`'s list box and name entry are found under the window the message reached; the save's list box
//!   under `parent`; `textEntryReplayName` in the init under `parent`.
//! - The windows used without a check (`buttonFrame`, `parent` in the update's `closeSaveMenu` once the shutdown has
//!   cleared it, the controls of the messages) are recorded as undefined when missing
//!   ([`GameWindowManager::set_undefined`]); `DEBUG_ASSERTCRASH` and `DEBUG_CRASH` are compiled out of the release.
//! - The delete's and the copy's error text (`FormatMessageW`) is trimmed; `messageBoxWin`, if the overwrite question
//!   made one, is unset as modal first.
//!
//! # Where this differs from upstream
//!
//! - The replay folder, `doesFileExist`, `DeleteFile` and `CopyFile` are the host's
//!   ([`crate::client::replay_menu::ReplayListHost`]): the client's replay folder (natively the user data folder's
//!   `Replays\`, in the browser OPFS `Replays/`), into which it writes the last game's replay (`00000000.rep`, from the
//!   logic's recorder) when the score screen opens.
//!
//! # Representation
//!
//! [`PopupReplay`]: the file's statics (with `replayPath` and `messageBoxWin`).

use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::message_box;
use crate::client::replay_menu::{narrow, populate_replay_file_listbox, trim, ReplayListHost};
use crate::client::score_screen::{score_screen_enable_controls, ScoreScreenHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::common::recorder::REPLAY_EXTENSION;

/// `s_fileSavePopupDuration`, milliseconds.
pub const FILE_SAVE_POPUP_DURATION: u32 = 1000;

/// What the replay popup needs: the score screen's host and the replay folder's.
pub trait PopupReplayHost: ScoreScreenHost + ReplayListHost {
    /// PopupReplay.cpp's statics.
    fn popup_replay(&mut self) -> &mut PopupReplay;
}

/// PopupReplay.cpp's statics (`NAMEKEY_INVALID` is 0).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PopupReplay {
    pub button_back_key: i32,
    pub button_save_key: i32,
    pub listbox_games_key: i32,
    pub text_entry_replay_name_key: i32,
    pub parent: Option<WinId>,
    pub replay_saved_parent: Option<WinId>,
    /// `s_fileSavePopupStartTime` (0: no popup).
    pub file_save_popup_start_time: u32,
    /// `replayPath`.
    pub replay_path: Vec<u8>,
    /// `messageBoxWin`.
    pub message_box_win: Option<WinId>,
}

/// `ShowReplaySavedPopup(show)`.
pub fn show_replay_saved_popup<H: PopupReplayHost>(wm: &mut GameWindowManager, h: &mut H, show: bool) {
    if let Some(w) = h.popup_replay().replay_saved_parent {
        wm.win_hide(w, !show);
    }
}

/// `closeSaveMenu(window)`: the window's layout hidden.
fn close_save_menu(wm: &mut GameWindowManager, window: Option<WinId>) {
    let Some(w) = window else {
        wm.set_undefined("PopupReplay closeSaveMenu: no window (window->winGetLayout on NULL)");
        return;
    };
    if let Some(l) = wm.ext[w].layout {
        wm.layout_hide(l, true);
    }
}

/// `PopupReplayInit(layout, userData)`.
pub fn popup_replay_init<H: PopupReplayHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let mut k = |n: &str| h.name_to_key(format!("PopupReplay.wnd:{n}").as_bytes()) as i32;
    let keys = [k("ButtonBack"), k("ButtonSave"), k("ListboxGames"), k("TextEntryReplayName")];
    let (parent_id, saved_id, frame_id) = (k("PopupReplayMenu"), k("PopupReplaySaved"), k("MenuButtonFrame"));
    let p = h.popup_replay();
    [p.button_back_key, p.button_save_key, p.listbox_games_key, p.text_entry_replay_name_key] = keys;
    let parent = wm.win_get_window_from_id(None, parent_id);
    h.popup_replay().parent = parent;
    wm.win_set_focus(h, parent);
    let saved = wm.win_get_window_from_id(None, saved_id);
    h.popup_replay().replay_saved_parent = saved;
    // (`if (replaySavedParent == NULL) DEBUG_CRASH(...)`: compiled out)
    show_replay_saved_popup(wm, h, false);
    let Some(frame) = wm.win_get_window_from_id(parent, frame_id) else {
        wm.set_undefined("PopupReplayInit: no MenuButtonFrame (buttonFrame->winEnable on NULL)");
        return;
    };
    wm.win_enable(frame, true);
    let list = wm.win_get_window_from_id(None, keys[2]);
    populate_replay_file_listbox(wm, h, list);
    let entry = wm.win_get_window_from_id(parent, keys[3]);
    te::set_text(wm, h, entry, &[]);
    wm.win_set_focus(h, entry);
    if let Some(c) = wm.win_get_window_from_id(parent, keys[1]) {
        wm.win_enable(c, false);
    }
}

/// `PopupReplayShutdown(layout, userData)`.
pub fn popup_replay_shutdown<H: PopupReplayHost>(_wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    h.popup_replay().parent = None;
}

/// `PopupReplayUpdate(layout, userData)`.
pub fn popup_replay_update<H: PopupReplayHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let start = h.popup_replay().file_save_popup_start_time;
    if start != 0 && h.time_ms().wrapping_sub(start) >= FILE_SAVE_POPUP_DURATION {
        show_replay_saved_popup(wm, h, false);
        let parent = h.popup_replay().parent;
        close_save_menu(wm, parent);
        score_screen_enable_controls(wm, h, true);
        h.popup_replay().file_save_popup_start_time = 0;
    }
}

/// `PopupReplayInput(window, msg, mData1, mData2)`.
pub fn popup_replay_input<H: PopupReplayHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 {
        let p = h.popup_replay();
        let (parent, id) = (p.parent, p.button_back_key);
        let button = wm.win_get_window_from_id(parent, id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), id as u32);
    }
    MsgHandled::Handled
}

/// `saveReplay(filename)`.
pub fn save_replay<H: PopupReplayHost>(wm: &mut GameWindowManager, h: &mut H, filename: &[u16]) {
    let translated = if filename == h.fetch_text(b"GUI:LastReplay") { h.last_replay_file_name() } else { narrow(filename) };
    let mut full_path = h.replay_dir();
    full_path.extend_from_slice(&translated);
    full_path.extend_from_slice(REPLAY_EXTENSION);
    h.popup_replay().replay_path = full_path.clone();
    h.popup_replay().message_box_win = None;
    if h.replay_file_exists(&full_path) {
        let t = h.fetch_text(b"GUI:OverwriteReplayTitle");
        let b = h.fetch_text(b"GUI:OverwriteReplay");
        let w = message_box::message_box_ok_cancel(wm, h, &t, &b, Some("reallySaveReplay"), None);
        h.popup_replay().message_box_win = w;
    } else {
        really_save_replay(wm, h);
    }
}

/// An error box after a failed delete or copy: `messageBoxWin` unset as modal and cleared, then
/// `MessageBoxOk(GUI:Error, trimmed text, NULL)`.
fn error_box<H: PopupReplayHost>(wm: &mut GameWindowManager, h: &mut H, text: &[u16]) {
    if let Some(m) = h.popup_replay().message_box_win.take() {
        wm.win_unset_modal(Some(m));
    }
    let t = h.fetch_text(b"GUI:Error");
    message_box::message_box_ok(wm, h, &t, &trim(text), None);
}

/// The popup's games list (`winGetWindowFromId(parent, listboxGamesKey)`) filled again.
fn repopulate<H: PopupReplayHost>(wm: &mut GameWindowManager, h: &mut H) {
    let p = h.popup_replay();
    let (parent, id) = (p.parent, p.listbox_games_key);
    let list = wm.win_get_window_from_id(parent, id);
    populate_replay_file_listbox(wm, h, list);
}

/// `reallySaveReplay()` (also the overwrite question's OK).
pub fn really_save_replay<H: PopupReplayHost>(wm: &mut GameWindowManager, h: &mut H) {
    let filename = h.popup_replay().replay_path.clone();
    let last = h.score_screen().globals.last_replay_file_name.clone();
    let mut old_filename = h.replay_dir();
    old_filename.extend_from_slice(&last);
    old_filename.extend_from_slice(REPLAY_EXTENSION);
    if old_filename == filename {
        return;
    }
    if h.replay_file_exists(&filename) {
        if let Err(text) = h.delete_replay_file(&filename) {
            error_box(wm, h, &text);
            repopulate(wm, h);
            return;
        }
    }
    // copy the replay to the right place
    if let Err(text) = h.copy_replay_file(&old_filename, &filename) {
        error_box(wm, h, &text);
        return;
    }
    repopulate(wm, h);
    show_replay_saved_popup(wm, h, true);
    let now = h.time_ms();
    h.popup_replay().file_save_popup_start_time = now;
}

/// `PopupReplaySystem(window, msg, mData1, mData2)`.
pub fn popup_replay_system<H: PopupReplayHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        lb::glm::SELECTED => {
            let control = data_win(d1);
            let list_id = h.popup_replay().listbox_games_key;
            let list = wm.win_get_window_from_id(Some(window), list_id);
            if control.is_some() && control == list {
                let row = d2 as i32;
                if row >= 0 {
                    let l = list.expect("checked");
                    let filename = lb::get_text(wm, h, l, row, 0);
                    let entry_id = h.popup_replay().text_entry_replay_name_key;
                    let entry = wm.win_get_window_from_id(Some(window), entry_id);
                    te::set_text(wm, h, entry, &filename);
                }
            }
        }
        te::gem::EDIT_DONE => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("PopupReplaySystem: GEM_EDIT_DONE without a control (control->winGetWindowId on NULL)");
                return MsgHandled::Handled;
            };
            if wm.tree.windows[control].id == h.popup_replay().text_entry_replay_name_key {
                let filename = te::get_text(wm, h, Some(control));
                if !filename.is_empty() {
                    save_replay(wm, h, &filename);
                }
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("PopupReplaySystem: GBM_SELECTED without a control (control->winGetWindowId on NULL)");
                return MsgHandled::Handled;
            };
            let id = wm.tree.windows[control].id;
            let p = h.popup_replay().clone();
            if id == p.button_save_key {
                let entry = wm.win_get_window_from_id(Some(window), p.text_entry_replay_name_key);
                let filename = te::get_text(wm, h, entry);
                if !filename.is_empty() {
                    save_replay(wm, h, &filename);
                }
            } else if id == p.button_back_key {
                close_save_menu(wm, Some(window));
                score_screen_enable_controls(wm, h, true);
            }
        }
        te::gem::UPDATE_TEXT => {
            let p = h.popup_replay().clone();
            if let Some(entry) = wm.win_get_window_from_id(p.parent, p.text_entry_replay_name_key) {
                let filename = te::get_text(wm, h, Some(entry));
                if let Some(save) = wm.win_get_window_from_id(p.parent, p.button_save_key) {
                    wm.win_enable(save, !filename.is_empty());
                }
            }
            // (no `break`: on into `default`)
            return MsgHandled::Ignored;
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::replay_menu::tests::folder;
    use crate::client::replay_menu::FILE_NOT_FOUND_TEXT;
    use crate::client::save_load_menu::tests::{click, hidden, named, tick, H};
    use crate::client::score_screen::tests::{game, keeper, player, scored, skirmish_info};
    use crate::client::shell::ShellRequest;
    use crate::common::recorder::game_mode;

    const F: &str = "PopupReplay.wnd";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// The score screen of a recorded skirmish, with the replay folder of the replay menu's tests.
    fn screen() -> (GameWindowManager, H) {
        let mut g = game(game_mode::SKIRMISH, vec![player("player0", "Me", 11, keeper(1, 5, 1, 3, 2, 0, 1, 1000), true, "USA", Some(0), true)], 1);
        g.game_info = Some(skirmish_info());
        let (wm, mut h) = scored(g);
        h.replays = folder();
        (wm, h)
    }

    fn score(wm: &GameWindowManager, name: &str) -> WinId {
        let full = format!("ScoreScreen.wnd:{name}");
        wm.tree.windows.iter().rposition(|x| x.name() == full.as_bytes()).unwrap_or_else(|| panic!("{name}"))
    }

    fn box_text(wm: &GameWindowManager) -> String {
        let m = wm.tree.windows.iter().rposition(|x| x.name() == b"MessageBox.wnd:StaticTextMessage").expect("a box");
        String::from_utf16_lossy(&crate::client::gadget::static_text_get_text(wm, m))
    }

    fn box_ok(wm: &GameWindowManager) -> WinId {
        wm.tree.windows.iter().rposition(|x| x.name() == b"MessageBox.wnd:ButtonOk").expect("a box")
    }

    /// The name typed into the entry: its text set and `GEM_UPDATE_TEXT` sent to its owner, as a key typed does.
    fn type_name(wm: &mut GameWindowManager, h: &mut H, name: &str) {
        let e = named(wm, F, "TextEntryReplayName");
        te::set_text(wm, h, Some(e), &wide(name));
        let owner = wm.ext[e].owner;
        wm.win_send_system_msg(h, owner, te::gem::UPDATE_TEXT, win_data(Some(e)), 0);
    }

    /// #98 (the issue): *Save Replay* on the score screen, predicted from PopupReplay.cpp and ScoreScreen.cpp before the
    /// run:
    ///
    /// - *Save Replay* disables the score screen's buttons and runs the popup's init: the popup shown with the list of
    ///   the replay menu (two rows), the name empty, *Save* disabled, *replay saved* hidden, the focus on the entry;
    /// - a name typed (`GEM_UPDATE_TEXT`) enables *Save*; *Save* with `Best` copies the last replay to
    ///   `Replays\Best.rep` (`WriteReplay` with its bytes), fills the list again (three rows), shows *replay saved*
    ///   and starts its timer; the score screen's buttons stay disabled;
    /// - the score screen's updates: under 1000 ms nothing; at 1000 ms *replay saved* hidden, the popup's layout
    ///   hidden, the score screen's *OK* and *Save Replay* enabled again, the timer cleared;
    /// - again, `Best` exists: the overwrite question; its OK deletes the old file then copies (`DeleteReplay`,
    ///   `WriteReplay`);
    /// - the last replay's row picked (`GLM_SELECTED` puts `T:GUI:LastReplay` in the entry) and saved: nothing at all
    ///   (**missed**: the overwrite question comes first; its OK then does nothing);
    /// - a missing last replay: the copy's error box, its text trimmed;
    /// - *Back* (Escape's release) closes the popup and enables the score screen's buttons; `GEM_UPDATE_TEXT` answers
    ///   `MSG_IGNORED` (it falls into `default`).
    #[test]
    fn save_replay_copies_the_last_replay_and_comes_back() {
        let (mut wm, mut h) = screen();
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        let save_replay = score(&wm, "ButtonSaveReplay");
        let ok = score(&wm, "ButtonOk");
        assert!(wm.is_enabled(Some(save_replay)));
        click(&mut wm, &mut h, save_replay);
        assert!(!wm.is_enabled(Some(save_replay)) && !wm.is_enabled(Some(ok)));
        let layout = h.shell.popup_replay_layout.expect("the popup");
        assert!(!wm.layouts[layout].hidden);
        assert!(!h.untranslated.iter().any(|f| f.starts_with(b"PopupReplay")), "{:?}", h.untranslated);
        let list = named(&wm, F, "ListboxGames");
        assert_eq!(lb::get_num_entries(&wm, list), 2);
        let save = named(&wm, F, "ButtonSave");
        assert!(!wm.is_enabled(Some(save)));
        assert!(hidden(&wm, named(&wm, F, "PopupReplaySaved")));
        let entry = named(&wm, F, "TextEntryReplayName");
        assert_eq!(wm.keyboard_focus, Some(entry));
        // a name, Save
        type_name(&mut wm, &mut h, "Best");
        assert!(wm.is_enabled(Some(save)));
        h.requests.clear();
        click(&mut wm, &mut h, save);
        assert_eq!(h.requests, [ShellRequest::WriteReplay { path: b"Replays\\Best.rep".to_vec(), bytes: b"LAST".to_vec() }]);
        assert_eq!(lb::get_num_entries(&wm, list), 3);
        assert!(!hidden(&wm, named(&wm, F, "PopupReplaySaved")));
        let started = h.popup_replay.file_save_popup_start_time;
        assert_eq!(started, h.time_ms);
        assert!(!wm.is_enabled(Some(ok)));
        // the score screen's updates take it down after 1000 ms
        while h.time_ms - started < FILE_SAVE_POPUP_DURATION - 40 {
            tick(&mut wm, &mut h);
        }
        assert!(!wm.layouts[layout].hidden, "under 1000 ms");
        tick(&mut wm, &mut h);
        assert!(wm.layouts[layout].hidden && hidden(&wm, named(&wm, F, "PopupReplaySaved")));
        assert!(wm.is_enabled(Some(ok)) && wm.is_enabled(Some(save_replay)));
        assert_eq!(h.popup_replay.file_save_popup_start_time, 0);
        // again: the overwrite question
        click(&mut wm, &mut h, save_replay);
        type_name(&mut wm, &mut h, "best");
        h.requests.clear();
        click(&mut wm, &mut h, save);
        assert!(h.requests.is_empty());
        assert_eq!(box_text(&wm), "T:GUI:OverwriteReplay");
        let b = box_ok(&wm);
        click(&mut wm, &mut h, b);
        assert_eq!(
            h.requests,
            [ShellRequest::DeleteReplay { path: b"Replays\\Best.rep".to_vec() }, ShellRequest::WriteReplay { path: b"Replays\\best.rep".to_vec(), bytes: b"LAST".to_vec() }]
        );
        wm.update(&mut h);
        // the last replay's row: nothing
        let p = h.popup_replay.parent.expect("parent");
        popup_replay_system(&mut wm, &mut h, p, lb::glm::SELECTED, win_data(Some(list)), 0);
        assert_eq!(String::from_utf16_lossy(&te::get_text(&mut wm, &mut h, Some(entry))), "T:GUI:LastReplay");
        h.requests.clear();
        let before = h.popup_replay.file_save_popup_start_time;
        click(&mut wm, &mut h, save);
        // (prediction missed: `saveReplay` asks to overwrite first, the last replay's file existing; only the
        // question's OK, `reallySaveReplay`, finds the two paths the same and does nothing)
        assert_eq!(box_text(&wm), "T:GUI:OverwriteReplay");
        let b = box_ok(&wm);
        click(&mut wm, &mut h, b);
        wm.update(&mut h);
        assert!(h.requests.is_empty() && wm.modal.is_empty());
        assert_eq!(h.popup_replay.file_save_popup_start_time, before);
        // the last replay gone: the copy's error, trimmed
        h.replays.files.retain(|f| f.0 != b"Replays\\00000000.rep");
        type_name(&mut wm, &mut h, "Other");
        click(&mut wm, &mut h, save);
        assert_eq!(box_text(&wm), FILE_NOT_FOUND_TEXT.trim_end());
        let b = box_ok(&wm);
        click(&mut wm, &mut h, b);
        // GEM_UPDATE_TEXT falls into default
        assert_eq!(popup_replay_system(&mut wm, &mut h, p, te::gem::UPDATE_TEXT, win_data(Some(entry)), 0), MsgHandled::Ignored);
        // Escape: Back
        h.time_ms += 2000;
        tick(&mut wm, &mut h);
        wm.win_set_focus(&mut h, Some(p));
        wm.win_process_key(&mut h, key::ESC, key::STATE_UP);
        assert!(wm.layouts[layout].hidden);
        assert!(wm.is_enabled(Some(ok)) && wm.is_enabled(Some(save_replay)));
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        // the shutdown clears `parent`; an update that then closes the menu is upstream's `NULL->winGetLayout()`
        popup_replay_shutdown(&mut wm, &mut h, layout);
        h.popup_replay.file_save_popup_start_time = 1;
        popup_replay_update(&mut wm, &mut h, layout);
        assert!(wm.undefined.is_some());
    }
}
