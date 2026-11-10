// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/ReplayMenu.cpp (the file's statics,
//     deleteReplayFlag, copyReplayFlag, GetReplayFilenameFromListbox, PopulateReplayFileListbox, ReplayMenuInit,
//     ReplayMenuShutdown, ReplayMenuUpdate, ReplayMenuInput, reallyLoadReplay, ReplayMenuSystem, deleteReplay,
//     copyReplay)
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp (getUnicodeTimeBuffer: the host's)
//   GeneralsMD/Code/GameEngine/Source/Common/Recorder.cpp (testVersionPlayback's comparison, over the header the
//     host read)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The replay menu (GitHub #98): `Menus/ReplayMenu.wnd`, the main menu's *Load/Replay*, *Replay*: the list of the
//! replay files, their playback, delete and copy, with the list the score screen's replay popup shares
//! (`PopulateReplayFileListbox`, `GetReplayFilenameFromListbox`: `crate::client::popup_replay`).
//!
//! # As upstream reads it
//!
//! **The list** (`PopulateReplayFileListbox`): nothing without `TheMapCache`; else the list box emptied, every file
//! `*.rep` of the replay folder (`TheRecorder->getReplayDir()`, the user data folder's `Replays\`) and its sub-folders
//! listed (`getFileListInDirectory(..., TRUE)`: a set, `_stricmp` order), `TheMapCache->updateCache()`. For each file,
//! its name without its folder (`reverseFind('\\') + 1`); its header read (`readReplayHeader`, `forPlayback` false,
//! which opens the replay folder's file **of that name**) and its game options parsed again into a new
//! `ReplayGameInfo`. A file whose header or options do not read is left out. A row is: the name without its last four
//! characters (`removeLastChar` once per character of `.rep`), or `GUI:LastReplay` for the last replay's file
//! (`00000000.rep`, compared without case); the header's time (`getUnicodeTimeBuffer`); the header's version string;
//! the map's display name (`TheMapCache->findMap`), or the map's path when the cache does not have it. The colour is
//! white for a file of this build's version, version number, exe CRC and INI CRC, else grey (the multiplayer and
//! single player colours are the same). Row 0 is selected.
//!
//! **GetReplayFilenameFromListbox(listbox, index)**: the row's text (column 0); `GUI:LastReplay`'s text means the last
//! replay's name; `.rep` appended.
//!
//! **Init**: the shell map shown, the six window ids made and their windows found under `ParentReplayMenu`, the list
//! emptied and filled, the layout shown, the focus on the parent, `justEntered`, `GadgetParent` hidden. **Update**: a
//! frame later the main menu's `MainMenuDefaultMenuLogoFade` removed and `ReplayMenuFade` set; a delete or copy the
//! message box asked for runs (`callCopy`, `callDelete`); a shutdown completes once the animations and transitions
//! are done. **Shutdown**: immediate, done at once; else `ReplayMenuFade` reversed. **Input**: Escape's release presses
//! *Back*.
//!
//! **System**: a double click on a row plays it back (`TheRecorder->playbackFile(name)`) and hides the parent; *Load*
//! with a row selected: a file of another version asks first (`GUI:OlderReplayVersion`, its OK `reallyLoadReplay`),
//! else plays it back and hides the parent; no row: `GUI:NoFileSelected`. *Back* pops the menu. *Delete* and *Copy*
//! (with a row) ask yes or no; the yes sets the flag the next update acts on: `deleteReplay` deletes the replay
//! folder's file (an error's text in a box) and fills the list again; `copyReplay` copies it to the desktop folder
//! (`CSIDL_DESKTOPDIRECTORY`).
//!
//! # Upstream quirks that are kept
//!
//! - A file in a sub-folder is listed by its name alone, and its header is read from the replay folder's file of that
//!   name (if any): a sub-folder's file shows the top folder's header, or is left out.
//! - The last replay's row is matched without case (`compareNoCase`), but the name is cut by the length of `.rep`
//!   whatever the file's own extension's case.
//! - `GetReplayFilenameFromListbox` reads the row's text: a replay that was named `GUI:LastReplay`'s text by its player
//!   is the last replay.
//! - `deleteReplay`'s error text is `FormatMessage`'s, not trimmed (its CR LF kept); `copyReplay` trims it.
//! - *Delete* and *Copy* read the file name before their question and throw it away; the flag's function reads the
//!   selection again when the next update runs.
//! - `reallyLoadReplay` and the double click call `playbackFile` whatever it answers, and hide the parent: a replay
//!   that does not read leaves the menu hidden.
//! - The *Load* branch checks `listboxReplayFiles`; *Delete* and *Copy* do not: a file without the list box is
//!   `GadgetListBoxGetSelected` of `NULL`, which leaves `selected` unset (recorded as undefined).
//! - Every system message returns `MSG_HANDLED` but the unknown ones; `GWM_INPUT_FOCUS` asks for the focus.
//!
//! # Where this differs from upstream
//!
//! - **The replay folder** is the host's ([`ReplayListHost`]): its files' paths (`replay_file_list`), each header as
//!   `readReplayHeader` reads it with the game options parsed again (`read_replay_header`, a [`ReplayListing`]: the
//!   client reads it with the logic's `RecorderClass::readReplayHeader` and `ParseAsciiStringToGameInfo` on a copy of
//!   the install's World, [`replay_listing`]), whether a file exists, the delete and the copy (their failure is the
//!   error text `FormatMessage` would give), the desktop folder.
//! - **`TheRecorder->playbackFile`** is a [`ShellRequest::PlaybackReplay`]: the client plays the file back through the
//!   logic's recorder (`World::play_replay`, batch I55), in a copy of the install's World, once the replay's map has
//!   arrived. `testVersionPlayback` is translated over the host's header ([`test_version_playback`]).
//! - **The version** (`TheVersion`, `TheGlobalData->m_exeCRC` / `m_iniCRC`) is the host's ([`ReplayVersion`]): the
//!   client's World records an empty version string, version 0 and its global data's CRCs, and answers the same.
//! - **The time** (`getUnicodeTimeBuffer`: `GetTimeFormatW(LOCALE_SYSTEM_DEFAULT, TIME_NOSECONDS)` on Windows NT) is
//!   the host's, by default the en-US system locale's `h:mm tt`; a header time that `GetTimeFormatW` rejects (an hour,
//!   minute, second or millisecond out of range) leaves the buffer unset: recorded as undefined. Not re-checked against
//!   Windows: which fields `GetTimeFormatW` validates.
//! - The debug build's *Analyze Replay* button (`#if defined _DEBUG || defined _INTERNAL`) is not in the release, and
//!   not here.
//!
//! # Verification
//!
//! `PopulateReplayFileListbox` and `GetReplayFilenameFromListbox` are compared with ReplayMenu.cpp compiled
//! (tests/differential_replay_list.rs, 2026-11-10, with the host's `MemoryReplayFolder` listing): 20,000 cases, 98,837
//! files, 32,718 rows, 0 mismatches; planted bugs 8961 .. 8965 each caught. The callbacks are tested from a synthetic
//! `ReplayMenu.wnd`.
//!
//! # Representation
//!
//! [`ReplayMenu`]: the file's statics. The window ids are the host's name keys
//! ([`crate::client::shell::ShellHost::name_to_key`]).

use crate::client::gadget_list_box as lb;
use crate::client::main_menu::MenuHost;
use crate::client::map_util::MapUtilHost;
use crate::client::message_box;
use crate::client::save_load_menu::en_us_time_no_seconds;
use crate::client::shell::{self, ShellRequest};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;
use crate::common::game_state::SaveDate;
use crate::common::recorder::{ReplayHeader, LAST_REPLAY_FILE_NAME, REPLAY_EXTENSION};
use crate::formats::big::FilenameList;
use crate::logic::modules::plant;

/// A replay file's header as `readReplayHeader` reads it (`None` when it answers false), and whether
/// `ParseAsciiStringToGameInfo` of its game options into a new `ReplayGameInfo` succeeds, with that game's map
/// (`info.getMap()`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplayListing {
    pub header: ReplayHeader,
    pub parsed: bool,
    pub map: Vec<u8>,
}

/// This build's version as the list compares it: `TheVersion->getUnicodeVersion()`, `getUnicodeBuildTime()` (for
/// `testVersionPlayback`), `getVersionNumber()`, `TheGlobalData->m_exeCRC`, `m_iniCRC`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplayVersion {
    pub version_string: Vec<u16>,
    pub version_time_string: Vec<u16>,
    pub version_number: u32,
    pub exe_crc: u32,
    pub ini_crc: u32,
}

/// What the replay list and the replay menu reach besides the main menu's host and the map cache.
pub trait ReplayListHost: MenuHost + MapUtilHost {
    /// `TheRecorder->getReplayDir()`: the user data path and `Replays\`.
    fn replay_dir(&self) -> Vec<u8>;
    /// `TheRecorder->getLastReplayFileName()`: `00000000` in the release.
    fn last_replay_file_name(&self) -> Vec<u8> {
        LAST_REPLAY_FILE_NAME.to_vec()
    }
    /// `TheFileSystem->getFileListInDirectory(dir, search, list, TRUE)`: the matching files of `dir` and its
    /// sub-folders, by their paths.
    fn replay_file_list(&mut self, dir: &[u8], search: &[u8]) -> FilenameList;
    /// `readReplayHeader` of the replay folder's file `filename` (`None`: it answers false), with the second parse of
    /// its game options.
    fn read_replay_header(&mut self, filename: &[u8]) -> Option<ReplayListing>;
    /// This build's version and CRCs.
    fn replay_version(&self) -> ReplayVersion;
    /// `getUnicodeTimeBuffer(timeVal)` (`None`: `GetTimeFormatW` fails and leaves its buffer unset); by default the
    /// en-US system locale's `h:mm tt`, failing for a time out of range.
    fn replay_time_text(&self, t: &SaveDate) -> Option<Vec<u16>> {
        if t.hour > 23 || t.minute > 59 || t.second > 59 || t.milliseconds > 999 {
            return None;
        }
        Some(en_us_time_no_seconds(t))
    }
    /// `TheLocalFileSystem->doesFileExist(path)`.
    fn replay_file_exists(&mut self, path: &[u8]) -> bool;
    /// `DeleteFile(path)`: `Err` with `FormatMessage`'s text of the error when it fails.
    fn delete_replay_file(&mut self, path: &[u8]) -> Result<(), Vec<u16>>;
    /// `CopyFile(from, to, FALSE)` (an existing `to` replaced): `Err` with `FormatMessage`'s text when it fails.
    fn copy_replay_file(&mut self, from: &[u8], to: &[u8]) -> Result<(), Vec<u16>>;
    /// `SHGetPathFromIDList` of `CSIDL_DESKTOPDIRECTORY`: the desktop folder.
    fn desktop_path(&self) -> Vec<u8>;
    /// ReplayMenu.cpp's statics.
    fn replay_menu(&mut self) -> &mut ReplayMenu;
}

/// ReplayMenu.cpp's statics (`NAMEKEY_INVALID` is 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayMenu {
    pub parent_replay_menu_id: i32,
    pub button_load_id: i32,
    pub button_back_id: i32,
    pub listbox_replay_files_id: i32,
    pub button_delete_id: i32,
    pub button_copy_id: i32,
    pub is_shutting_down: bool,
    pub parent_replay_menu: Option<WinId>,
    pub button_load: Option<WinId>,
    pub button_back: Option<WinId>,
    pub listbox_replay_files: Option<WinId>,
    pub button_delete: Option<WinId>,
    pub button_copy: Option<WinId>,
    pub initial_gadget_delay: i32,
    pub just_entered: bool,
    pub call_copy: bool,
    pub call_delete: bool,
}

impl Default for ReplayMenu {
    fn default() -> Self {
        ReplayMenu {
            parent_replay_menu_id: 0,
            button_load_id: 0,
            button_back_id: 0,
            listbox_replay_files_id: 0,
            button_delete_id: 0,
            button_copy_id: 0,
            is_shutting_down: false,
            parent_replay_menu: None,
            button_load: None,
            button_back: None,
            listbox_replay_files: None,
            button_delete: None,
            button_copy: None,
            initial_gadget_delay: 2,
            just_entered: false,
            call_copy: false,
            call_delete: false,
        }
    }
}

/// `UnicodeString::translate(AsciiString)`: each `char`, signed, widened.
pub fn widen(a: &[u8]) -> Vec<u16> {
    a.iter().map(|&c| c as i8 as i16 as u16).collect()
}

/// `AsciiString::translate(UnicodeString)`: each unit cut to a `char` (`concat(char)`: a 0 appends nothing).
pub fn narrow(u: &[u16]) -> Vec<u8> {
    u.iter().map(|&c| c as u8).filter(|&c| c != 0).collect()
}

/// A header's `SYSTEMTIME` (eight little-endian `WORD`s: year, month, day of the week, day, hour, minute, second,
/// milliseconds).
pub fn system_time(t: &[u8; 16]) -> SaveDate {
    let w = |i: usize| u16::from_le_bytes([t[2 * i], t[2 * i + 1]]);
    SaveDate { year: w(0), month: w(1), day_of_week: w(2), day: w(3), hour: w(4), minute: w(5), second: w(6), milliseconds: w(7) }
}

/// `GameMakeColor(r, g, b, a)`.
fn color(r: u8, g: u8, b: u8, a: u8) -> i32 {
    wt::game_make_color(r, g, b, a)
}

/// `deleteReplayFlag()`: the message box's yes.
pub fn delete_replay_flag<H: ReplayListHost>(h: &mut H) {
    h.replay_menu().call_delete = true;
}

/// `copyReplayFlag()`: the message box's yes.
pub fn copy_replay_flag<H: ReplayListHost>(h: &mut H) {
    h.replay_menu().call_copy = true;
}

/// `GetReplayFilenameFromListbox(listbox, index)`.
pub fn get_replay_filename_from_listbox<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H, listbox: Option<WinId>, index: i32) -> Vec<u16> {
    let mut fname = match listbox {
        Some(l) => lb::get_text(wm, h, l, index, 0),
        None => Vec::new(),
    };
    // (plant 8965, differential builds only: the last replay's text not recognised)
    if fname == h.fetch_text(b"GUI:LastReplay") && !plant(8965) {
        fname = widen(&h.last_replay_file_name());
    }
    fname.extend(widen(REPLAY_EXTENSION));
    fname
}

/// `PopulateReplayFileListbox(listbox)`.
pub fn populate_replay_file_listbox<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H, listbox: Option<WinId>) {
    if h.map_cache().is_none() {
        return;
    }
    if let Some(l) = listbox {
        lb::reset(wm, h, l);
    }
    // COLOR_SP, COLOR_SP_CRC_MISMATCH, COLOR_MP, COLOR_MP_CRC_MISMATCH
    let colors = [color(255, 255, 255, 255), color(128, 128, 128, 255), color(255, 255, 255, 255), color(128, 128, 128, 255)];
    let mut search = b"*".to_vec();
    search.extend_from_slice(REPLAY_EXTENSION);
    let dir = h.replay_dir();
    let names = h.replay_file_list(&dir, &search);
    h.update_map_cache();
    for path in names.iter() {
        // just want the filename
        let asciistr = match path.iter().rposition(|&c| c == b'\\') {
            Some(i) => path[i + 1..].to_vec(),
            None => path.to_vec(),
        };
        let Some(listing) = h.read_replay_header(&asciistr) else { continue };
        if !listing.parsed {
            continue;
        }
        let header = &listing.header;
        let mut replay_name = widen(&asciistr);
        // (plant 8962, differential builds only: one character fewer cut)
        let cut = if plant(8962) { REPLAY_EXTENSION.len() - 1 } else { REPLAY_EXTENSION.len() };
        for _ in 0..cut {
            replay_name.pop();
        }
        let mut replay_name_to_show = replay_name;
        let mut last = h.last_replay_file_name();
        last.extend_from_slice(REPLAY_EXTENSION);
        // (plant 8961, differential builds only: compared with case)
        let same = if plant(8961) { last == asciistr } else { last.eq_ignore_ascii_case(&asciistr) };
        if same {
            replay_name_to_show = h.fetch_text(b"GUI:LastReplay");
        }
        let Some(t) = header.time_val else {
            wm.set_undefined("PopulateReplayFileListbox: the header's time was not read (getUnicodeTimeBuffer of an unset SYSTEMTIME)");
            return;
        };
        let Some(display_time) = h.replay_time_text(&system_time(&t)) else {
            wm.set_undefined("PopulateReplayFileListbox: GetTimeFormatW failed (its buffer unset)");
            return;
        };
        let map_str = match h.map_cache().and_then(|c| c.find_map(&listing.map)) {
            Some(md) => md.display_name.str().to_vec(),
            None => widen(&listing.map),
        };
        let v = h.replay_version();
        // (`&&` stops at the first difference: a field the read never reached is compared only when the ones before it
        // were equal)
        let same = |a: Option<u32>, b: u32| a.map(|a| a == b);
        let good = if header.version_string != v.version_string {
            Some(false)
        } else {
            match same(header.version_number, v.version_number) {
                Some(true) => match same(header.exe_crc, v.exe_crc) {
                    Some(true) => same(header.ini_crc, v.ini_crc),
                    x => x,
                },
                x => x,
            }
        };
        let Some(good) = good else {
            wm.set_undefined("PopulateReplayFileListbox: the header's version or CRCs were not read (compared unset)");
            return;
        };
        let mp = header.local_player_index >= 0;
        // (plant 8963, differential builds only: the mismatch colour for a good version)
        let good = if plant(8963) { !good } else { good };
        let c = match (good, mp) {
            (true, true) => colors[2],
            (true, false) => colors[0],
            (false, true) => colors[3],
            (false, false) => colors[1],
        };
        let Some(l) = listbox else { continue };
        let insertion_index = lb::add_entry_text(wm, h, l, &replay_name_to_show, c, -1, 0, true);
        lb::add_entry_text(wm, h, l, &display_time, c, insertion_index, 1, true);
        let vs = header.version_string.clone();
        lb::add_entry_text(wm, h, l, &vs, c, insertion_index, 2, true);
        lb::add_entry_text(wm, h, l, &map_str, c, insertion_index, 3, true);
    }
    if let Some(l) = listbox {
        // (plant 8964, differential builds only: the second row selected)
        let first = if plant(8964) { 1 } else { 0 };
        lb::set_selected(wm, h, l, &[first]);
    }
}

/// `TheRecorder->testVersionPlayback(filename)` over the host's header (`readReplayHeader` with `forPlayback`):
/// whether the file's version strings, version number, exe CRC or INI CRC differ from this build's (false when the
/// header does not read).
pub fn test_version_playback<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H, filename: &[u8]) -> bool {
    let Some(listing) = h.read_replay_header(filename) else { return false };
    let header = listing.header;
    let v = h.replay_version();
    // (`||` stops at the first difference: a field the read never reached is compared only when it is reached; the
    // INI CRC always is)
    let differs = |a: Option<u32>, b: u32| a.map(|a| a != b);
    let exe_different = if header.version_string != v.version_string || header.version_time_string != v.version_time_string {
        Some(true)
    } else {
        match differs(header.version_number, v.version_number) {
            Some(false) => differs(header.exe_crc, v.exe_crc),
            x => x,
        }
    };
    let (Some(exe_different), Some(ini_different)) = (exe_different, differs(header.ini_crc, v.ini_crc)) else {
        wm.set_undefined("testVersionPlayback: the header's version or CRCs were not read (compared unset)");
        return false;
    };
    exe_different || ini_different
}

/// `TheRecorder->playbackFile(filename)`: the client's playback ([`ShellRequest::PlaybackReplay`]).
fn playback_file<H: ReplayListHost>(h: &mut H, filename: &[u8]) {
    h.request(ShellRequest::PlaybackReplay { filename: filename.to_vec() });
}

/// `GadgetListBoxGetSelected(listboxReplayFiles, &selected)`: `None` for a `NULL` list box (`selected` left unset:
/// recorded as undefined).
fn selected<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H) -> Option<i32> {
    let Some(l) = h.replay_menu().listbox_replay_files else {
        wm.set_undefined("ReplayMenu: no replay list box (GadgetListBoxGetSelected leaves `selected` unset)");
        return None;
    };
    Some(lb::get_selected(wm, h, l).first().copied().unwrap_or(-1))
}

/// `MessageBoxOk(GUI:NoFileSelected, GUI:PleaseSelectAFile, NULL)`.
fn no_file_selected<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H) {
    let t = h.fetch_text(b"GUI:NoFileSelected");
    let b = h.fetch_text(b"GUI:PleaseSelectAFile");
    message_box::message_box_ok(wm, h, &t, &b, None);
}

/// The selected row's file name (`GetReplayFilenameFromListbox`, as `AsciiString::translate` makes it).
fn selected_filename<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H, selected: i32) -> Vec<u8> {
    let l = h.replay_menu().listbox_replay_files;
    let f = get_replay_filename_from_listbox(wm, h, l, selected);
    narrow(&f)
}

/// The parent hidden, after a playback (`if(parentReplayMenu != NULL) parentReplayMenu->winHide(TRUE)`).
fn hide_parent<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H) {
    if let Some(p) = h.replay_menu().parent_replay_menu {
        wm.win_hide(p, true);
    }
}

/// `ReplayMenuInit(layout, userData)`.
pub fn replay_menu_init<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    shell::show_shell_map(wm, h, true);
    let mut k = |n: &str| h.name_to_key(format!("ReplayMenu.wnd:{n}").as_bytes()) as i32;
    let keys = [k("ParentReplayMenu"), k("ButtonLoadReplay"), k("ButtonBack"), k("ListboxReplayFiles"), k("ButtonDeleteReplay"), k("ButtonCopyReplay")];
    let parent = wm.win_get_window_from_id(None, keys[0]);
    let w = |id: i32| wm.win_get_window_from_id(parent, id);
    let (load, back, list, delete, copy) = (w(keys[1]), w(keys[2]), w(keys[3]), w(keys[4]), w(keys[5]));
    let m = h.replay_menu();
    [m.parent_replay_menu_id, m.button_load_id, m.button_back_id, m.listbox_replay_files_id, m.button_delete_id, m.button_copy_id] = keys;
    m.parent_replay_menu = parent;
    m.button_load = load;
    m.button_back = back;
    m.listbox_replay_files = list;
    m.button_delete = delete;
    m.button_copy = copy;
    // Load the listbox shiznit
    if let Some(l) = list {
        lb::reset(wm, h, l);
    }
    populate_replay_file_listbox(wm, h, list);
    // (`#if defined _DEBUG || defined _INTERNAL`: the *Analyze Replay* button; not in the release)
    wm.layout_hide(layout, false);
    wm.win_set_focus(h, parent);
    let m = h.replay_menu();
    m.just_entered = true;
    m.initial_gadget_delay = 2;
    let gp = h.name_to_key(b"ReplayMenu.wnd:GadgetParent") as i32;
    if let Some(win) = wm.win_get_window_from_id(None, gp) {
        wm.win_hide(win, true);
    }
    h.replay_menu().is_shutting_down = false;
}

/// `ReplayMenuShutdown(layout, userData)`: `immediate` is `*(Bool *)userData`.
pub fn replay_menu_shutdown<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, immediate: Option<bool>) {
    let Some(pop_immediate) = immediate else {
        wm.set_undefined("ReplayMenuShutdown: no user data (*(Bool *)NULL)");
        return;
    };
    if pop_immediate {
        wm.layout_hide(layout, true);
        shell::shutdown_complete(wm, h, Some(layout), false);
        return;
    }
    wt::reverse(wm, h, b"ReplayMenuFade");
    h.replay_menu().is_shutting_down = true;
}

/// `ReplayMenuUpdate(layout, userData)`.
pub fn replay_menu_update<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let m = h.replay_menu();
    if m.just_entered {
        if m.initial_gadget_delay == 1 {
            wt::remove(wm, h, b"MainMenuDefaultMenuLogoFade", false);
            wt::set_group(wm, h, b"ReplayMenuFade", false);
            let m = h.replay_menu();
            m.initial_gadget_delay = 2;
            m.just_entered = false;
        } else {
            m.initial_gadget_delay -= 1;
        }
    }
    if h.replay_menu().call_copy {
        copy_replay(wm, h);
    }
    if h.replay_menu().call_delete {
        delete_replay(wm, h);
    }
    if h.replay_menu().is_shutting_down && shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shell::shutdown_complete(wm, h, Some(layout), false);
    }
}

/// `ReplayMenuInput(window, msg, mData1, mData2)`.
pub fn replay_menu_input<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 {
        let m = h.replay_menu();
        let (back, id) = (m.button_back, m.button_back_id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(back), id as u32);
    }
    MsgHandled::Handled
}

/// `reallyLoadReplay()`: the older version's OK.
pub fn really_load_replay<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(sel) = selected(wm, h) else { return };
    if sel < 0 {
        no_file_selected(wm, h);
        return;
    }
    let f = selected_filename(wm, h, sel);
    playback_file(h, &f);
    hide_parent(wm, h);
}

/// `ReplayMenuSystem(window, msg, mData1, mData2)`.
pub fn replay_menu_system<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        lb::glm::DOUBLE_CLICKED => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("ReplayMenuSystem: GLM_DOUBLE_CLICKED without a control (control->winGetWindowId on NULL)");
                return MsgHandled::Handled;
            };
            if wm.tree.windows[control].id == h.replay_menu().listbox_replay_files_id {
                let row = d2 as i32;
                if row >= 0 {
                    let f = selected_filename(wm, h, row);
                    playback_file(h, &f);
                    hide_parent(wm, h);
                }
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("ReplayMenuSystem: GBM_SELECTED without a control (control->winGetWindowId on NULL)");
                return MsgHandled::Handled;
            };
            let id = wm.tree.windows[control].id;
            let m = h.replay_menu().clone();
            if id == m.button_load_id {
                if m.listbox_replay_files.is_some() {
                    let Some(sel) = selected(wm, h) else { return MsgHandled::Handled };
                    if sel < 0 {
                        no_file_selected(wm, h);
                        return MsgHandled::Handled;
                    }
                    let f = selected_filename(wm, h, sel);
                    if test_version_playback(wm, h, &f) {
                        let t = h.fetch_text(b"GUI:OlderReplayVersionTitle");
                        let b = h.fetch_text(b"GUI:OlderReplayVersion");
                        message_box::message_box_ok_cancel(wm, h, &t, &b, Some("reallyLoadReplay"), None);
                    } else {
                        playback_file(h, &f);
                        hide_parent(wm, h);
                    }
                }
            } else if id == m.button_back_id {
                // thou art directed to return to thy known solar system immediately!
                shell::pop(wm, h);
            } else if id == m.button_delete_id || id == m.button_copy_id {
                let Some(sel) = selected(wm, h) else { return MsgHandled::Handled };
                if sel < 0 {
                    no_file_selected(wm, h);
                    return MsgHandled::Handled;
                }
                // (the name read and dropped, as upstream)
                let _ = selected_filename(wm, h, sel);
                let (title, body, cb): (&[u8], &[u8], &'static str) = if id == m.button_delete_id {
                    (b"GUI:DeleteFile", b"GUI:AreYouSureDelete", "deleteReplayFlag")
                } else {
                    (b"GUI:CopyReplay", b"GUI:AreYouSureCopy", "copyReplayFlag")
                };
                let t = h.fetch_text(title);
                let b = h.fetch_text(body);
                message_box::message_box_yes_no(wm, h, &t, &b, Some(cb), None);
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// `deleteReplay()`.
pub fn delete_replay<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.replay_menu().call_delete = false;
    let Some(sel) = selected(wm, h) else { return };
    if sel < 0 {
        no_file_selected(wm, h);
        return;
    }
    let mut filename = h.replay_dir();
    filename.extend(selected_filename(wm, h, sel));
    if let Err(text) = h.delete_replay_file(&filename) {
        // (`FormatMessage` narrow, then `errorStr.translate`: not trimmed)
        let t = h.fetch_text(b"GUI:Error");
        message_box::message_box_ok(wm, h, &t, &widen(&narrow(&text)), None);
    }
    let l = h.replay_menu().listbox_replay_files;
    if let Some(l) = l {
        lb::reset(wm, h, l);
    }
    populate_replay_file_listbox(wm, h, l);
}

/// `UnicodeString::trim()`: leading and trailing white space removed.
pub fn trim(t: &[u16]) -> Vec<u16> {
    let sp = |c: &u16| matches!(*c, 0x20 | 0x09..=0x0D);
    let a = t.iter().position(|c| !sp(c)).unwrap_or(t.len());
    let b = t.iter().rposition(|c| !sp(c)).map_or(a, |i| i + 1);
    t[a..b.max(a)].to_vec()
}

/// `copyReplay()`: the selected replay copied to the desktop folder.
pub fn copy_replay<H: ReplayListHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.replay_menu().call_copy = false;
    let Some(sel) = selected(wm, h) else { return };
    if sel < 0 {
        no_file_selected(wm, h);
        return;
    }
    let translate = selected_filename(wm, h, sel);
    let mut filename = h.replay_dir();
    filename.extend_from_slice(&translate);
    let mut new_filename = h.desktop_path();
    new_filename.push(b'\\');
    new_filename.extend_from_slice(&translate);
    if let Err(text) = h.copy_replay_file(&filename, &new_filename) {
        let t = h.fetch_text(b"GUI:Error");
        message_box::message_box_ok(wm, h, &t, &trim(&text), None);
    }
}

/// The `FormatMessage` text of `ERROR_FILE_NOT_FOUND` (an en-US system), as `DeleteFile` and `CopyFile` report a
/// missing file: `FormatMessage` ends its text with CR LF.
pub const FILE_NOT_FOUND_TEXT: &str = "The system cannot find the file specified.\r\n";

/// (#98, new) The replay folder as a host keeps it in memory (the client gives it the files of its folder, with each
/// one's [`ReplayListing`] read by [`replay_listings`]): what `getFileListInDirectory`, `readReplayHeader`,
/// `doesFileExist`, `DeleteFile` and `CopyFile` reach. Paths compare without case (a Windows file system); the
/// writes and deletes are queued for the owner as [`ShellRequest`]s.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryReplayFolder {
    /// The folder's files: their paths (`Replays\<name>`, the replay folder's), bytes and listing.
    pub files: Vec<(Vec<u8>, Vec<u8>, Option<ReplayListing>)>,
    /// The desktop folder's path (`CSIDL_DESKTOPDIRECTORY`): a copy there is an export.
    pub desktop: Vec<u8>,
}

impl MemoryReplayFolder {
    /// The index of the file at `path` (without case).
    pub fn find(&self, path: &[u8]) -> Option<usize> {
        self.files.iter().position(|(p, _, _)| p.eq_ignore_ascii_case(path))
    }

    /// `getFileListInDirectory(dir, "*<ext>", list, TRUE)`: the files under `dir` (and its sub-folders) whose name ends
    /// with the search's extension (without case).
    pub fn list(&self, dir: &[u8], search: &[u8]) -> FilenameList {
        let ext = search.strip_prefix(b"*").unwrap_or(search).to_ascii_lowercase();
        let mut out = FilenameList::new();
        for (p, _, _) in &self.files {
            let lower = p.to_ascii_lowercase();
            if lower.starts_with(&dir.to_ascii_lowercase()) && lower.ends_with(&ext) {
                out.insert(p.clone());
            }
        }
        out
    }

    /// `readReplayHeader` of `dir` + `filename`: the file's listing.
    pub fn header(&self, dir: &[u8], filename: &[u8]) -> Option<ReplayListing> {
        let mut p = dir.to_vec();
        p.extend_from_slice(filename);
        self.find(&p).and_then(|i| self.files[i].2.clone())
    }

    /// `DeleteFile(path)`: the request for the owner, or `FormatMessage`'s text.
    pub fn delete(&mut self, path: &[u8]) -> Result<ShellRequest, Vec<u16>> {
        let Some(i) = self.find(path) else { return Err(FILE_NOT_FOUND_TEXT.encode_utf16().collect()) };
        let (p, _, _) = self.files.remove(i);
        Ok(ShellRequest::DeleteReplay { path: p })
    }

    /// `CopyFile(from, to, FALSE)`: an existing `to` replaced (its path's spelling kept, as a Windows file system keeps
    /// it); a copy to the desktop folder is an export. The request for the owner, or `FormatMessage`'s text.
    pub fn copy(&mut self, from: &[u8], to: &[u8]) -> Result<ShellRequest, Vec<u16>> {
        let Some(i) = self.find(from) else { return Err(FILE_NOT_FOUND_TEXT.encode_utf16().collect()) };
        let (bytes, listing) = (self.files[i].1.clone(), self.files[i].2.clone());
        let mut desk = self.desktop.clone();
        desk.push(b'\\');
        if to.len() > desk.len() && to[..desk.len()].eq_ignore_ascii_case(&desk) {
            return Ok(ShellRequest::ExportReplay { name: to[desk.len()..].to_vec(), bytes });
        }
        let path = match self.find(to) {
            Some(j) => {
                self.files[j].1 = bytes.clone();
                self.files[j].2 = listing;
                self.files[j].0.clone()
            }
            None => {
                self.files.push((to.to_vec(), bytes.clone(), listing));
                to.to_vec()
            }
        };
        Ok(ShellRequest::WriteReplay { path, bytes })
    }
}

/// (#98, new) Each replay file's [`ReplayListing`], as the list reads it: the logic's `RecorderClass::readReplayHeader`
/// (`forPlayback` false) on a copy of `base` (the install's World; not changed) with the file on its disk, in its
/// replay folder, then `ParseAsciiStringToGameInfo` of the game options into a new game. `None` where the header does
/// not read, and where the read is undefined upstream (`IniError`).
pub fn replay_listings(base: &crate::logic::world::game_logic::World, files: &[(Vec<u8>, Vec<u8>)]) -> Vec<Option<ReplayListing>> {
    use crate::common::game_info::{parse_ascii_string_to_game_info, GameInfo};
    use crate::common::recorder::Recorder;
    let mut w = base.clone();
    let mut r = Recorder::new(&mut w);
    let dir = r.replay_dir(&w);
    files
        .iter()
        .map(|(name, bytes)| {
            let mut path = dir.clone();
            path.extend_from_slice(name);
            let key = path.to_ascii_lowercase();
            let before = w.save_side.disk.insert(key.clone(), bytes.clone());
            let mut header = ReplayHeader { filename: name.clone(), for_playback: false, ..Default::default() };
            let read = r.read_replay_header(&mut header, &mut w);
            let out = match read {
                Ok(true) => {
                    let mut info = GameInfo::new(&mut w);
                    let parsed = parse_ascii_string_to_game_info(Some(&mut info), &header.game_options, &mut w).unwrap_or(false);
                    Some(ReplayListing { header, parsed, map: info.map_name.clone() })
                }
                _ => None,
            };
            match before {
                Some(b) => w.save_side.disk.insert(key, b),
                None => w.save_side.disk.remove(&key),
            };
            out
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::save_load_menu::tests::{booted, click, hidden, named, settle, tick, H};
    use crate::common::map_cache::MapMetaData;

    const F: &str = "ReplayMenu.wnd";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// A listing: version number `version`, local player `local`, the map `map`, the time 14:05.
    pub(crate) fn listing(version: u32, local: i32, map: &str) -> Option<ReplayListing> {
        let mut t = [0u8; 16];
        t[8] = 14;
        t[10] = 5;
        let header = ReplayHeader {
            replay_name: wide("T:GUI:LastReplay"),
            time_val: Some(t),
            version_string: Vec::new(),
            version_number: Some(version),
            exe_crc: Some(0),
            ini_crc: Some(0),
            local_player_index: local,
            ..Default::default()
        };
        Some(ReplayListing { header, parsed: true, map: map.as_bytes().to_vec() })
    }

    /// The replay folder of the tests, by path: the last replay (this version, single player, a map the cache has),
    /// `Mine.REP` (another version, multiplayer, a map it lacks), a file whose header does not read, one whose options
    /// do not parse, a sub-folder's file with no file of its name at the top, and a text file.
    pub(crate) fn folder() -> MemoryReplayFolder {
        let mut bad_options = listing(0, -1, "x");
        bad_options.as_mut().expect("listing").parsed = false;
        MemoryReplayFolder {
            files: vec![
                (b"Replays\\00000000.rep".to_vec(), b"LAST".to_vec(), listing(0, -1, "Maps\\Alpine\\Alpine.map")),
                (b"Replays\\Mine.REP".to_vec(), b"MINE".to_vec(), listing(5, 0, "Maps\\Gone\\Gone.map")),
                (b"Replays\\bad.rep".to_vec(), b"BAD".to_vec(), None),
                (b"Replays\\opts.rep".to_vec(), b"OPTS".to_vec(), bad_options),
                (b"Replays\\sub\\Other.rep".to_vec(), b"OTHER".to_vec(), listing(0, -1, "y")),
                (b"Replays\\notes.txt".to_vec(), b"TXT".to_vec(), listing(0, -1, "z")),
            ],
            desktop: b"Desktop".to_vec(),
        }
    }

    /// The shell with the folder, and the map cache holding `Alpine` (display name `Alpine Assault`).
    pub(crate) fn with_replays() -> (GameWindowManager, H) {
        let (wm, mut h) = booted();
        h.replays = folder();
        let md = MapMetaData { display_name: crate::common::unicode_string::UnicodeString::try_from_chars(&wide("Alpine Assault")).expect("the name"), ..Default::default() };
        h.skirmish_data.map_cache.maps.insert(b"maps\\alpine\\alpine.map".to_vec(), md);
        (wm, h)
    }

    /// A row's four texts and its colour.
    pub(crate) fn row(wm: &mut GameWindowManager, h: &mut H, l: WinId, r: i32) -> (Vec<String>, i32) {
        let t: Vec<String> = (0..4).map(|c| String::from_utf16_lossy(&lb::get_text(wm, h, l, r, c))).collect();
        let (_, color) = lb::get_text_and_color(wm, h, l, r, 0);
        (t, color)
    }

    fn requests(h: &mut H) -> Vec<ShellRequest> {
        std::mem::take(&mut h.requests)
    }

    /// The message box's button `name` (the last made).
    fn box_button(wm: &GameWindowManager, name: &str) -> WinId {
        let full = format!("MessageBox.wnd:{name}");
        wm.tree.windows.iter().rposition(|x| x.name() == full.as_bytes()).unwrap_or_else(|| panic!("{name}"))
    }

    fn box_text(wm: &GameWindowManager) -> String {
        let m = box_button(wm, "StaticTextMessage");
        String::from_utf16_lossy(&crate::client::gadget::static_text_get_text(wm, m))
    }

    /// #98: the replay menu from a synthetic `ReplayMenu.wnd`, predicted from ReplayMenu.cpp before the run:
    ///
    /// - the push: the list holds two rows, in `_stricmp` order of the paths: the last replay (`T:GUI:LastReplay`,
    ///   `2:05 PM`, an empty version, `Alpine Assault`, white) and `Mine` (the name cut by four, the map's path for
    ///   the map the cache lacks, grey: another version); `bad.rep` (no header), `opts.rep` (options that do not
    ///   parse), `sub\Other.rep` (its header looked up as `Replays\Other.rep`: none) and `notes.txt` are left out; row
    ///   0 selected; `GadgetParent` hidden; a frame later `ReplayMenuFade` is set;
    /// - *Load* on row 0 (this version): `PlaybackReplay("00000000.rep")`, the parent hidden;
    /// - row 1 and *Load*: an older version asks (OK and Cancel); its OK (`reallyLoadReplay`) plays back `Mine.rep`
    ///   (the row's text and `.rep`, not the file's `.REP`) and hides the parent;
    /// - a double click on row 0 plays it back;
    /// - *Copy* on row 0, *Yes*: nothing until the next update, which exports `00000000.rep` with its bytes;
    /// - *Delete* on row 1, *Yes*: the next update deletes `Replays\Mine.REP` and fills the list again (one row);
    /// - a delete whose file is gone: the error box's text untrimmed (its CR LF kept);
    /// - an empty folder: *Load* with no row asks `GUI:NoFileSelected`;
    /// - Escape's release presses *Back*: the menu pops (once `ReplayMenuFade` is reversed).
    #[test]
    fn the_replay_menu_lists_plays_copies_and_deletes() {
        let (mut wm, mut h) = with_replays();
        shell::push(&mut wm, &mut h, b"Menus/ReplayMenu.wnd", false);
        settle(&mut wm, &mut h);
        settle(&mut wm, &mut h);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/ReplayMenu.wnd");
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        let l = h.replay_menu.listbox_replay_files.expect("the list");
        assert_eq!(lb::get_num_entries(&wm, l), 2);
        let white = color(255, 255, 255, 255);
        let grey = color(128, 128, 128, 255);
        assert_eq!(row(&mut wm, &mut h, l, 0), (vec!["T:GUI:LastReplay".into(), "2:05 PM".into(), " ".into(), "Alpine Assault".into()], white));
        assert_eq!(row(&mut wm, &mut h, l, 1), (vec!["Mine".into(), "2:05 PM".into(), " ".into(), "Maps\\Gone\\Gone.map".into()], grey));
        assert_eq!(lb::get_selected(&mut wm, &mut h, l), [0]);
        assert!(hidden(&wm, named(&wm, F, "GadgetParent")));
        // (prediction missed: the two settles ran the menu's update once, not twice: one more frame sets the fade)
        assert_eq!((h.replay_menu.just_entered, h.replay_menu.initial_gadget_delay), (true, 1));
        tick(&mut wm, &mut h);
        assert!(!h.replay_menu.just_entered, "the fade was set a frame later");
        settle(&mut wm, &mut h);
        // Load, this version
        let load = named(&wm, F, "ButtonLoadReplay");
        click(&mut wm, &mut h, load);
        assert_eq!(requests(&mut h), [ShellRequest::PlaybackReplay { filename: b"00000000.rep".to_vec() }]);
        let parent = h.replay_menu.parent_replay_menu.expect("parent");
        assert!(hidden(&wm, parent));
        wm.win_hide(parent, false);
        // Load, another version: asked, then played
        lb::set_selected(&mut wm, &mut h, l, &[1]);
        click(&mut wm, &mut h, load);
        assert!(requests(&mut h).is_empty());
        assert_eq!(box_text(&wm), "T:GUI:OlderReplayVersion");
        let ok = box_button(&wm, "ButtonOk");
        click(&mut wm, &mut h, ok);
        assert_eq!(requests(&mut h), [ShellRequest::PlaybackReplay { filename: b"Mine.rep".to_vec() }]);
        assert!(hidden(&wm, parent));
        wm.win_hide(parent, false);
        wm.update(&mut h);
        // a double click on row 0
        replay_menu_system(&mut wm, &mut h, parent, lb::glm::DOUBLE_CLICKED, win_data(Some(l)), 0);
        assert_eq!(requests(&mut h), [ShellRequest::PlaybackReplay { filename: b"00000000.rep".to_vec() }]);
        wm.win_hide(parent, false);
        // Copy row 0
        lb::set_selected(&mut wm, &mut h, l, &[0]);
        let copy = named(&wm, F, "ButtonCopyReplay");
        click(&mut wm, &mut h, copy);
        assert_eq!(box_text(&wm), "T:GUI:AreYouSureCopy");
        let yes = box_button(&wm, "ButtonYes");
        click(&mut wm, &mut h, yes);
        assert!(h.replay_menu.call_copy && requests(&mut h).is_empty());
        tick(&mut wm, &mut h);
        assert!(!h.replay_menu.call_copy);
        assert_eq!(requests(&mut h), [ShellRequest::ExportReplay { name: b"00000000.rep".to_vec(), bytes: b"LAST".to_vec() }]);
        // Delete row 1
        lb::set_selected(&mut wm, &mut h, l, &[1]);
        let delete = named(&wm, F, "ButtonDeleteReplay");
        click(&mut wm, &mut h, delete);
        assert_eq!(box_text(&wm), "T:GUI:AreYouSureDelete");
        let yes = box_button(&wm, "ButtonYes");
        click(&mut wm, &mut h, yes);
        tick(&mut wm, &mut h);
        assert_eq!(requests(&mut h), [ShellRequest::DeleteReplay { path: b"Replays\\Mine.REP".to_vec() }]);
        assert_eq!(lb::get_num_entries(&wm, l), 1);
        // a delete of a file that is gone: the error's text untrimmed
        h.replays.files.retain(|f| f.0 != b"Replays\\00000000.rep");
        h.replay_menu.call_delete = true;
        tick(&mut wm, &mut h);
        assert_eq!(box_text(&wm), FILE_NOT_FOUND_TEXT);
        assert_eq!(lb::get_num_entries(&wm, l), 0);
        let ok = box_button(&wm, "ButtonOk");
        click(&mut wm, &mut h, ok);
        // nothing selected
        click(&mut wm, &mut h, load);
        assert_eq!(box_text(&wm), "T:GUI:PleaseSelectAFile");
        let ok = box_button(&wm, "ButtonOk");
        click(&mut wm, &mut h, ok);
        // Escape: Back
        wm.win_set_focus(&mut h, Some(parent));
        wm.win_process_key(&mut h, key::ESC, key::STATE_UP);
        assert!(h.replay_menu.is_shutting_down && h.shell.pending_pop);
        settle(&mut wm, &mut h);
        tick(&mut wm, &mut h);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/MainMenu.wnd");
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert!(!h.untranslated.iter().any(|f| f.starts_with(b"ReplayMenu")), "{:?}", h.untranslated);
        // a shutdown without its user data: upstream's `*(Bool *)NULL`
        replay_menu_shutdown(&mut wm, &mut h, 0, None);
        assert!(wm.undefined.is_some());
    }

    /// #98: the pieces on their own, predicted from ReplayMenu.cpp and Recorder.cpp:
    /// - `GetReplayFilenameFromListbox` of `GUI:LastReplay`'s text is `00000000.rep`; of any other text, it with `.rep`;
    ///   of no list box, `.rep`;
    /// - `testVersionPlayback`: false for this version, true for another, false for a file that does not read, and an
    ///   unread version is undefined;
    /// - a header without its time, or a time `GetTimeFormatW` rejects: undefined; no map cache: the list is left
    ///   as it was;
    /// - `widen` sign-extends (`0xE9` is `0xFFE9`), `narrow` cuts and drops 0; `system_time` reads the eight words;
    /// - the folder copies over an existing file in place, exports to the desktop, and reports a missing source.
    #[test]
    fn the_replay_pieces_follow_upstream() {
        let (mut wm, mut h) = with_replays();
        assert_eq!(get_replay_filename_from_listbox(&mut wm, &mut h, None, 0), wide(".rep"));
        shell::push(&mut wm, &mut h, b"Menus/ReplayMenu.wnd", true);
        tick(&mut wm, &mut h);
        let l = h.replay_menu.listbox_replay_files;
        assert_eq!(get_replay_filename_from_listbox(&mut wm, &mut h, l, 0), wide("00000000.rep"));
        assert_eq!(get_replay_filename_from_listbox(&mut wm, &mut h, l, 1), wide("Mine.rep"));
        assert!(!test_version_playback(&mut wm, &mut h, b"00000000.rep"));
        assert!(test_version_playback(&mut wm, &mut h, b"Mine.rep"));
        assert!(!test_version_playback(&mut wm, &mut h, b"bad.rep"));
        assert!(wm.undefined.is_none());
        h.replays.files[0].2.as_mut().expect("listing").header.ini_crc = None;
        test_version_playback(&mut wm, &mut h, b"00000000.rep");
        assert!(wm.undefined.take().is_some());
        h.replays.files[0].2.as_mut().expect("listing").header.ini_crc = Some(0);
        h.replays.files[0].2.as_mut().expect("listing").header.time_val = None;
        populate_replay_file_listbox(&mut wm, &mut h, l);
        assert!(wm.undefined.take().is_some());
        let mut t = [0u8; 16];
        t[8] = 24;
        h.replays.files[0].2.as_mut().expect("listing").header.time_val = Some(t);
        populate_replay_file_listbox(&mut wm, &mut h, l);
        assert!(wm.undefined.take().is_some());
        assert_eq!(widen(&[0xE9, b'a']), [0xFFE9, 0x61]);
        assert_eq!(narrow(&[0x141, 0, 0x62]), b"Ab");
        let d = system_time(&[0xEA, 0x07, 11, 0, 2, 0, 10, 0, 14, 0, 5, 0, 6, 0, 7, 0]);
        assert_eq!((d.year, d.month, d.day_of_week, d.day, d.hour, d.minute, d.second, d.milliseconds), (2026, 11, 2, 10, 14, 5, 6, 7));
        // the folder's copy and delete
        let mut f = folder();
        assert_eq!(f.copy(b"Replays\\00000000.rep", b"replays\\MINE.rep"), Ok(ShellRequest::WriteReplay { path: b"Replays\\Mine.REP".to_vec(), bytes: b"LAST".to_vec() }));
        assert_eq!(f.files[1].1, b"LAST");
        assert_eq!(f.copy(b"Replays\\00000000.rep", b"desktop\\x.rep"), Ok(ShellRequest::ExportReplay { name: b"x.rep".to_vec(), bytes: b"LAST".to_vec() }));
        assert_eq!(f.copy(b"Replays\\none.rep", b"Replays\\y.rep"), Err(wide(FILE_NOT_FOUND_TEXT)));
        assert_eq!(f.delete(b"Replays\\none.rep"), Err(wide(FILE_NOT_FOUND_TEXT)));
        assert_eq!(f.list(b"Replays\\", b"*.rep").iter().collect::<Vec<_>>(), [&b"Replays\\00000000.rep"[..], b"Replays\\bad.rep", b"Replays\\Mine.REP", b"Replays\\opts.rep", b"Replays\\sub\\Other.rep"]);
    }
}
