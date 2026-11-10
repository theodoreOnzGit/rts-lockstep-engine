// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/PopupSaveLoad.cpp (the file's statics,
//     updateMenuActions, SaveLoadMenuInit, SaveLoadMenuFullScreenInit, SaveLoadMenuShutdown, SaveLoadMenuUpdate,
//     SaveLoadMenuInput, getSelectedSaveFileInfo, doLoadGame, closeSaveMenu, setEditDescription,
//     processLoadButtonPress, SaveLoadMenuSystem)
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp (addGameToAvailableList,
//     populateSaveGameListbox, iterateSaveFiles' choice of files, clearAvailableGames)
//   GeneralsMD/Code/GameEngine/Include/Common/GameState.h (SaveLoadLayoutType, AvailableGameInfo)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   PopupSaveLoad.cpp: Chris Brue, June 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The save/load screen (GitHub #78, item 3): `Menus/SaveLoad.wnd` (full screen, from the main menu's *Load Game*) and
//! `Menus/PopupSaveLoad.wnd` (the in-game popup), one set of callbacks, with the list of saves
//! (`GameState::populateSaveGameListbox`).
//!
//! # As upstream reads it
//!
//! **Init** (`SaveLoadMenuInit` for the popup, `SaveLoadMenuFullScreenInit` for the full screen, which first shows the
//! shell map) takes the layout type from its user data (else save-and-load for the popup, load-only for the full screen),
//! finds its windows under `<file>.wnd:SaveLoadMenu` (the button frame, the four confirmation windows, the description
//! entry, the games list box), gives the parent the focus (the popup also makes it modal), hides three of the four
//! confirmations (`OverwriteConfirmParent`, `LoadConfirmParent`, `SaveDescParent`; not `DeleteConfirmParent`), fills the
//! list box and sets the buttons (`updateMenuActions`: *Save* off when load-only; *Load* and *Delete* on only with a
//! save selected). The full screen then shows its layout but hides its parent, and its **Update** fades it in a frame
//! later (`SaveLoadMenuFade`, after removing `MainMenuDefaultMenuLogoFade`).
//!
//! **The list** (`populateSaveGameListbox`): the list box emptied; unless load-only, a first row "new save game"
//! (`GUI:NewSaveGame`, colour 200,200,255) with no item data; then every `.sav` file of the save directory
//! (`iterateSaveFiles`), its information read (`getSaveGameInfoFromFile`), newest first (`addGameToAvailableList`
//! inserts each before the first one it is newer than, `SaveDate::isNewerThan`; a file whose read throws is left out),
//! each a row: the description (or the map label's text, or the label itself), the time, the date; colour 200,255,200
//! for a mission save, else 255,255,255 on odd rows and 170,170,235 on even ones (counting the saves, not the rows); the
//! row's item data the save. Row 0 is selected.
//!
//! **System** (`GBM_SELECTED`): *Load* (`processLoadButtonPress`) with a save selected: in the shell, the menu closed
//! (`closeSaveMenu`: the popup's layout hidden, or `hideShell`) and the game loaded (`doLoadGame`: the main menu's load
//! transitions removed, `prepareNewGame(GAME_SINGLE_PLAYER, DIFFICULTY_NORMAL, 0)`, `TheGameState->loadGame(save)`; a
//! failed load clears the game, resets the engine and shows the shell again); in a game, the load confirmation shown.
//! *Save*: on the new-game row, the description window with its default text (the campaign and mission, or the map's
//! name); on a save, the overwrite confirmation. *Delete* with a save: the delete confirmation; its *Confirm* deletes the
//! file and fills the list again. *Back*: the popup closed, or the screen popped. The confirmations' buttons hide them,
//! enable the list and the frame again, and do what they confirm (`saveGame` with the file type: normal for
//! save-and-load, mission otherwise). `GLM_SELECTED` on the list sets the buttons; `GLM_DOUBLE_CLICKED` selects the row
//! and presses *Load*. **Input**: Escape's release hides the delete confirmation, enables the list and the frame, and
//! presses *Back*.
//!
//! **Shutdown**: immediate, the layout hidden and the shell told at once; else `SaveLoadMenuFade` reversed and the
//! update completes it once every animation and transition is done.
//!
//! # Upstream quirks that are kept
//!
//! - Init does not hide `DeleteConfirmParent`: a file that shows it shows it.
//! - The full screen's parent is not modal (its `winSetModal` is commented out); the popup's is.
//! - `GLM_DOUBLE_CLICKED` and `GLM_SELECTED` look the list box up under the window the message reached (`window`), and
//!   the rest of the file uses the one init found anywhere (`winGetWindowFromId(NULL, ...)`).
//! - *Save* on the new-game row does not disable the button frame (its `winEnable(FALSE)` is commented out); its
//!   description *Confirm* closes the menu **before** reading the selection for the file name (the list is enabled
//!   again first).
//! - The overwrite confirmation's *Confirm* passes `selectedGameInfo->saveGameInfo.description` whatever the
//!   `if( selectedGameInfo )` before it says: a `NULL` selection there is upstream's crash (recorded as undefined).
//! - The windows init found are used without a check where upstream uses them so (`buttonFrame->winEnable`,
//!   `overwriteConfirm->winHide`, ...): a file without one is recorded as undefined
//!   ([`GameWindowManager::undefined`]).
//! - Equal dates keep the files' order (`isNewerThan` is strict): the later file goes after.
//! - The statics live as long as the program: the layout type, `isPopup` and the windows are the last init's.
//!
//! # Where this differs from upstream
//!
//! - **The save directory** is the host's ([`SaveLoadHost::save_files`]): each file's name in the directory and its
//!   information as `getSaveGameInfoFromFile` reads it (`None` where it throws). The client gives its save folder's list
//!   (`crate::client::save_game::read_save_summary` of each `.sav`), in its order (by name); upstream's is
//!   `FindFirstFile`'s. The `.sav` choice (`strrchr('.')` and `stricmp`) is translated ([`is_save_file`]).
//! - **The date and the time** are `GetDateFormatW(LOCALE_SYSTEM_DEFAULT, DATE_SHORTDATE)` and
//!   `GetTimeFormatW(LOCALE_SYSTEM_DEFAULT, TIME_NOSECONDS)`: the host's ([`SaveLoadHost::date_text`],
//!   [`SaveLoadHost::time_text`]), by default the en-US system locale's (`M/d/yyyy`, `h:mm tt`).
//! - **The list box's item data** is an integer here: the save's index in [`SaveLoadMenu::available_games`] (upstream's
//!   `m_availableGames`, which this module keeps) plus one; 0 is `NULL`.
//! - **What the engine does** is a [`ShellRequest`] to the host: [`ShellRequest::LoadGame`] (`prepareNewGame` in the
//!   shell, then `loadGame`), [`ShellRequest::SaveGame`] (`saveGame`), [`ShellRequest::DeleteSave`] (`DeleteFile`; the
//!   host's list is told at once, [`SaveLoadHost::delete_save_file`], so the list filled again lacks it, as upstream's).
//!   A load's failure is the client's to report: it shows the shell again (`showShell(TRUE)`), as `doLoadGame` does.
//! - ~~`destroyQuitMenu` (QuitMenu.cpp, the in-game Escape menu) is not translated: the host's
//!   ([`SaveLoadHost::destroy_quit_menu`]), which records it. **The in-game popup is not reachable** in the client (the
//!   quit menu that opens it, `getSaveLoadMenuLayout`'s caller, is not translated): its callbacks are translated and
//!   tested from a synthetic `PopupSaveLoad.wnd`, and the client answers its save request by saying so (#78).~~ **Corrected 2026-11-10 (#87):** QuitMenu.cpp is translated
//!   (`crate::client::quit_menu`): its *Save/Load* opens this popup, and the host's `destroy_quit_menu` is its
//!   `destroyQuitMenu`; the client saves the popup's request through the saves panel's save.
//! - The runInit user data (`SaveLoadLayoutType *`) is an argument ([`save_load_menu_init`]); the lexicon passes `None`
//!   (`NULL`), as every caller in the release does (QuitMenu.cpp's is commented out).
//! - ~~`DontShowMainMenu` and `ReplayWasPressed` (ScoreScreen.cpp's globals; the score screen is not translated) are
//!   [`SaveLoadMenu`]'s, false.~~ **Corrected 2026-11-10 (#89):** they are the score screen's
//!   (`crate::client::score_screen::ScoreScreenGlobals`, through [`MenuHost::score_screen_globals`]; false for a host
//!   without it).
//!
//! # Verification
//!
//! `populate_save_game_listbox` (with `addGameToAvailableList`, `isNewerThan` and `iterateSaveFiles`' choice) is
//! compared with GameState.cpp compiled (tests/differential_save_list.rs, 2026-11-10): 20,000 cases, 53,965 rows,
//! 0 mismatches; planted bugs 7801, 7802, 7803 each caught. The callbacks are tested from synthetic WND files.
//!
//! # Representation
//!
//! [`SaveLoadMenu`]: the file's statics and `TheGameState`'s `m_availableGames`. The window ids are the host's name
//! keys ([`ShellHost::name_to_key`]), as the file's windows got theirs.

use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::main_menu::MenuHost;
use crate::client::shell::{self, ShellRequest};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;
use crate::common::game_state::{SaveDate, SaveGameInfo, SAVE_FILE_TYPE_MISSION, SAVE_FILE_TYPE_NORMAL};

/// `SaveLoadLayoutType`: `SLLT_INVALID` .. `SLLT_SAVE_ONLY`.
pub const SLLT_INVALID: i32 = 0;
pub const SLLT_SAVE_AND_LOAD: i32 = 1;
pub const SLLT_LOAD_ONLY: i32 = 2;
pub const SLLT_SAVE_ONLY: i32 = 3;

/// `AvailableGameInfo` (without its list links: [`SaveLoadMenu::available_games`] is the list, in its order).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AvailableGameInfo {
    pub filename: Vec<u8>,
    pub save_game_info: SaveGameInfo,
}

/// What the save/load screen needs besides the main menu's host.
pub trait SaveLoadHost: MenuHost {
    /// PopupSaveLoad.cpp's statics.
    fn save_load_menu(&mut self) -> &mut SaveLoadMenu;
    /// The save directory's files (`iterateSaveFiles`: every file, in the directory's order; [`is_save_file`] picks the
    /// `.sav` ones) with each one's `getSaveGameInfoFromFile` (`None`: the read throws).
    fn save_files(&mut self) -> Vec<(Vec<u8>, Option<SaveGameInfo>)>;
    /// `TheGameText->fetch(label, &exists)`.
    fn fetch_text_exists(&self, label: &[u8]) -> (Vec<u16>, bool) {
        (self.fetch_text(label), true)
    }
    /// `getUnicodeDateBuffer(date)`: the system locale's short date (en-US: `M/d/yyyy`).
    fn date_text(&self, d: &SaveDate) -> Vec<u16> {
        en_us_short_date(d)
    }
    /// `getUnicodeTimeBuffer(date)`: the system locale's time without seconds (en-US: `h:mm tt`).
    fn time_text(&self, d: &SaveDate) -> Vec<u16> {
        en_us_time_no_seconds(d)
    }
    /// `DeleteFile(TheGameState->getFilePathInSaveDirectory(filename))`: gone from the directory at once (the host's
    /// list), and a [`ShellRequest::DeleteSave`] for the owner of the files.
    fn delete_save_file(&mut self, filename: &[u8]);
    /// `TheCampaignManager->getCurrentCampaign()`'s name label and `getCurrentMissionNumber()` (`None`: no campaign).
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)> {
        None
    }
    /// `TheGlobalData->m_mapName`.
    fn map_name(&self) -> Vec<u8> {
        Vec::new()
    }
    /// `destroyQuitMenu()` (QuitMenu.cpp: ~~not translated; recorded~~ **Corrected 2026-11-10 (#87):** translated,
    /// `crate::client::quit_menu::destroy_quit_menu`, which the shell's host calls; nothing for a host without the quit
    /// menu).
    fn destroy_quit_menu(&mut self, _wm: &mut GameWindowManager) {}
}

/// PopupSaveLoad.cpp's statics (the window keys, the windows, the layout type) and `TheGameState->m_availableGames`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveLoadMenu {
    pub button_back_key: i32,
    pub button_save_key: i32,
    pub button_load_key: i32,
    pub button_delete_key: i32,
    pub listbox_games_key: i32,
    pub button_overwrite_cancel: i32,
    pub button_overwrite_confirm: i32,
    pub button_load_cancel: i32,
    pub button_load_confirm: i32,
    pub button_save_desc_cancel: i32,
    pub button_save_desc_confirm: i32,
    pub button_delete_confirm: i32,
    pub button_delete_cancel: i32,
    pub button_frame: Option<WinId>,
    pub overwrite_confirm: Option<WinId>,
    pub load_confirm: Option<WinId>,
    pub save_desc: Option<WinId>,
    pub listbox_games: Option<WinId>,
    pub edit_desc: Option<WinId>,
    pub delete_confirm: Option<WinId>,
    pub parent: Option<WinId>,
    pub current_layout_type: i32,
    pub is_popup: bool,
    pub initial_gadget_delay: i32,
    pub just_entered: bool,
    pub is_shutting_down: bool,
    /// `TheGameState->m_availableGames`, newest first.
    pub available_games: Vec<AvailableGameInfo>,
    // ~~ScoreScreen.cpp's `DontShowMainMenu` and `ReplayWasPressed` (the score screen is not translated: false)~~
    // **Corrected 2026-11-10 (#89):** the score screen's globals, [`MenuHost::score_screen_globals`]
}

impl Default for SaveLoadMenu {
    /// The statics' initial values (`NAMEKEY_INVALID` is 0).
    fn default() -> Self {
        SaveLoadMenu {
            button_back_key: 0,
            button_save_key: 0,
            button_load_key: 0,
            button_delete_key: 0,
            listbox_games_key: 0,
            button_overwrite_cancel: 0,
            button_overwrite_confirm: 0,
            button_load_cancel: 0,
            button_load_confirm: 0,
            button_save_desc_cancel: 0,
            button_save_desc_confirm: 0,
            button_delete_confirm: 0,
            button_delete_cancel: 0,
            button_frame: None,
            overwrite_confirm: None,
            load_confirm: None,
            save_desc: None,
            listbox_games: None,
            edit_desc: None,
            delete_confirm: None,
            parent: None,
            current_layout_type: SLLT_INVALID,
            is_popup: false,
            initial_gadget_delay: 2,
            just_entered: false,
            is_shutting_down: false,
            available_games: Vec::new(),
        }
    }
}

/// UTF-16 of ASCII text.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `GetDateFormatW(LOCALE_SYSTEM_DEFAULT, DATE_SHORTDATE)` on an en-US system: `M/d/yyyy`.
pub fn en_us_short_date(d: &SaveDate) -> Vec<u16> {
    wide(&format!("{}/{}/{}", d.month, d.day, d.year))
}

/// `GetTimeFormatW(LOCALE_SYSTEM_DEFAULT, TIME_NOSECONDS)` on an en-US system: `h:mm tt`.
pub fn en_us_time_no_seconds(d: &SaveDate) -> Vec<u16> {
    let h12 = match d.hour % 12 {
        0 => 12,
        h => h,
    };
    wide(&format!("{h12}:{:02} {}", d.minute, if d.hour < 12 { "AM" } else { "PM" }))
}

/// `iterateSaveFiles`' test of a directory entry's name: a `.` whose last occurrence starts `.sav` (case ignored).
pub fn is_save_file(name: &[u8]) -> bool {
    match name.iter().rposition(|&c| c == b'.') {
        Some(i) => name[i..].eq_ignore_ascii_case(b".sav"),
        None => false,
    }
}

/// `addGameToAvailableList(filename, &m_availableGames)` with the file's information read: inserted before the first
/// save it is newer than, else at the end.
pub fn add_game_to_available_list(list: &mut Vec<AvailableGameInfo>, filename: &[u8], info: SaveGameInfo) {
    let new_info = AvailableGameInfo { filename: filename.to_vec(), save_game_info: info };
    // (plant 7801, differential builds only: the newest last)
    let at = list.iter().position(|c| {
        let newer = new_info.save_game_info.date.is_newer_than(&c.save_game_info.date);
        if crate::logic::modules::plant(7801) {
            !newer
        } else {
            newer
        }
    });
    match at {
        Some(i) => list.insert(i, new_info),
        None => list.push(new_info),
    }
}

/// `GameMakeColor(r, g, b, a)`.
fn color(r: u8, g: u8, b: u8, a: u8) -> i32 {
    wt::game_make_color(r, g, b, a)
}

/// `GameState::populateSaveGameListbox(listbox, layoutType)`.
pub fn populate_save_game_listbox<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, listbox: Option<WinId>, layout_type: i32) {
    let Some(listbox) = listbox else { return };
    lb::reset(wm, h, listbox);
    if layout_type != SLLT_LOAD_ONLY {
        let text = h.fetch_text(b"GUI:NewSaveGame");
        let index = lb::add_entry_text(wm, h, listbox, &text, color(200, 200, 255, 255), -1, -1, true);
        lb::set_item_data(wm, h, listbox, 0, index, 0);
    }
    // `clearAvailableGames()`, then `iterateSaveFiles(addGameToAvailableList, &m_availableGames)`
    let mut games = Vec::new();
    for (name, info) in h.save_files() {
        if !is_save_file(&name) {
            continue;
        }
        // (`getSaveGameInfoFromFile` threw: `addGameToAvailableList`'s `catch(...)` leaves the file out)
        let Some(info) = info else { continue };
        add_game_to_available_list(&mut games, &name, info);
    }
    h.save_load_menu().available_games = games.clone();
    for (count, info) in games.iter().enumerate() {
        let s = &info.save_game_info;
        let date = h.date_text(&s.date);
        let time = h.time_text(&s.date);
        let mut label = s.description.clone();
        if label.is_empty() {
            let (text, exists) = h.fetch_text_exists(&s.map_label);
            label = if exists { text } else { s.map_label.iter().map(|&c| u16::from(c)).collect() };
        }
        // (plant 7802, differential builds only: the alternation counted from 1)
        let odd = if crate::logic::modules::plant(7802) { (count + 1) & 1 != 0 } else { count & 1 != 0 };
        let c = if s.save_file_type == SAVE_FILE_TYPE_MISSION {
            color(200, 255, 200, 255)
        } else if odd {
            color(255, 255, 255, 255)
        } else {
            color(170, 170, 235, 255)
        };
        let index = lb::add_entry_text(wm, h, listbox, &label, c, -1, 0, true);
        lb::add_entry_text(wm, h, listbox, &time, c, index, 1, true);
        lb::add_entry_text(wm, h, listbox, &date, c, index, 2, true);
        lb::set_item_data(wm, h, listbox, count as u32 + 1, index, 0);
    }
    // (plant 7803, differential builds only: the first save selected instead of row 0)
    let first = if crate::logic::modules::plant(7803) { 1 } else { 0 };
    lb::set_selected(wm, h, listbox, &[first]);
}

/// A window upstream uses without a check: `None` records the `NULL` call as undefined.
fn must(wm: &mut GameWindowManager, w: Option<WinId>, what: &'static str) -> Option<WinId> {
    if w.is_none() {
        wm.set_undefined(what);
    }
    w
}

/// `winEnable` on a window used without a check.
fn enable(wm: &mut GameWindowManager, w: Option<WinId>, on: bool, what: &'static str) {
    if let Some(w) = must(wm, w, what) {
        wm.win_enable(w, on);
    }
}

/// `winHide` on a window used without a check.
fn hide(wm: &mut GameWindowManager, w: Option<WinId>, on: bool, what: &'static str) {
    if let Some(w) = must(wm, w, what) {
        wm.win_hide(w, on);
    }
}

/// `GadgetListBoxGetSelected(listboxGames, &selected)` and `GadgetListBoxGetItemData(listboxGames, selected)`: the
/// selected row's item data (0: `NULL`). A `NULL` list box leaves `selected` unset: recorded as undefined, 0.
fn selected_item<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, listbox: Option<WinId>) -> (i32, u32) {
    let Some(l) = must(wm, listbox, "PopupSaveLoad: no games list box (GadgetListBoxGetSelected leaves `selected` unset)") else {
        return (-1, 0);
    };
    let selected = lb::get_selected(wm, h, l).first().copied().unwrap_or(-1);
    (selected, lb::get_item_data(wm, h, l, selected, 0))
}

/// The save an item data names (`(AvailableGameInfo *)` of it).
fn game<H: SaveLoadHost>(h: &mut H, data: u32) -> Option<AvailableGameInfo> {
    if data == 0 {
        return None;
    }
    h.save_load_menu().available_games.get(data as usize - 1).cloned()
}

/// `updateMenuActions()`.
fn update_menu_actions<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H) {
    let m = h.save_load_menu();
    let (save_key, load_key, delete_key, layout_type, listbox) = (m.button_save_key, m.button_load_key, m.button_delete_key, m.current_layout_type, m.listbox_games);
    let save = wm.win_get_window_from_id(None, save_key);
    enable(wm, save, layout_type != SLLT_LOAD_ONLY, "updateMenuActions: no save button (saveButton->winEnable)");
    let (_, data) = selected_item(wm, h, listbox);
    let selected = game(h, data).is_some();
    let load = wm.win_get_window_from_id(None, load_key);
    enable(wm, load, selected, "updateMenuActions: no load button (buttonLoad->winEnable)");
    let delete = wm.win_get_window_from_id(None, delete_key);
    enable(wm, delete, selected, "updateMenuActions: no delete button (buttonDelete->winEnable)");
}

/// The keys of `<file>:<name>` for the file's controls, set into the statics.
fn set_keys<H: SaveLoadHost>(h: &mut H, file: &str) {
    let mut k = |n: &str| h.name_to_key(format!("{file}:{n}").as_bytes()) as i32;
    let keys = [
        k("ButtonBack"),
        k("ButtonSave"),
        k("ButtonLoad"),
        k("ButtonDelete"),
        k("ListboxGames"),
        k("ButtonOverwriteCancel"),
        k("ButtonOverwriteConfirm"),
        k("ButtonLoadCancel"),
        k("ButtonLoadConfirm"),
        k("ButtonSaveDescCancel"),
        k("ButtonSaveDescConfirm"),
        k("ButtonDeleteConfirm"),
        k("ButtonDeleteCancel"),
    ];
    let m = h.save_load_menu();
    [
        m.button_back_key,
        m.button_save_key,
        m.button_load_key,
        m.button_delete_key,
        m.listbox_games_key,
        m.button_overwrite_cancel,
        m.button_overwrite_confirm,
        m.button_load_cancel,
        m.button_load_confirm,
        m.button_save_desc_cancel,
        m.button_save_desc_confirm,
        m.button_delete_confirm,
        m.button_delete_cancel,
    ] = keys;
}

/// The window `<file>:<name>` under `parent` (`winGetWindowFromId(parent, NAMEKEY(...))`).
fn find<H: SaveLoadHost>(wm: &GameWindowManager, h: &mut H, parent: Option<WinId>, file: &str, name: &str) -> Option<WinId> {
    let id = h.name_to_key(format!("{file}:{name}").as_bytes()) as i32;
    wm.win_get_window_from_id(parent, id)
}

/// The windows both inits find, the three confirmations hidden in upstream's order, the list filled and the buttons
/// set; false where upstream would have crashed (recorded).
fn init_windows<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, file: &str, popup: bool) -> bool {
    let parent_id = h.name_to_key(format!("{file}:SaveLoadMenu").as_bytes()) as i32;
    let parent = wm.win_get_window_from_id(None, parent_id);
    h.save_load_menu().parent = parent;
    wm.win_set_focus(h, parent);
    if popup {
        wm.win_set_modal(parent);
    }
    let frame = find(wm, h, parent, file, "MenuButtonFrame");
    h.save_load_menu().button_frame = frame;
    let Some(frame) = must(wm, frame, "SaveLoadMenuInit: no MenuButtonFrame (buttonFrame->winEnable)") else { return false };
    wm.win_enable(frame, true);
    let overwrite = find(wm, h, parent, file, "OverwriteConfirmParent");
    let load = find(wm, h, parent, file, "LoadConfirmParent");
    let desc = find(wm, h, parent, file, "SaveDescParent");
    let m = h.save_load_menu();
    m.overwrite_confirm = overwrite;
    m.load_confirm = load;
    m.save_desc = desc;
    for (w, what) in [
        (overwrite, "SaveLoadMenuInit: no OverwriteConfirmParent (winHide)"),
        (load, "SaveLoadMenuInit: no LoadConfirmParent (winHide)"),
        (desc, "SaveLoadMenuInit: no SaveDescParent (winHide)"),
    ] {
        hide(wm, w, true, what);
        if wm.undefined.is_some() {
            return false;
        }
    }
    let delete = find(wm, h, parent, file, "DeleteConfirmParent");
    let edit = find(wm, h, parent, file, "EntryDesc");
    let key = h.save_load_menu().listbox_games_key;
    let listbox = wm.win_get_window_from_id(None, key);
    let m = h.save_load_menu();
    m.delete_confirm = delete;
    m.edit_desc = edit;
    m.listbox_games = listbox;
    let layout_type = m.current_layout_type;
    populate_save_game_listbox(wm, h, listbox, layout_type);
    update_menu_actions(wm, h);
    wm.undefined.is_none()
}

/// `SaveLoadMenuInit(layout, userData)` (the popup): `layout_type` is `*(SaveLoadLayoutType *)userData` (`None`:
/// `NULL`).
pub fn save_load_menu_init<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize, layout_type: Option<i32>) {
    let m = h.save_load_menu();
    m.current_layout_type = layout_type.unwrap_or(SLLT_SAVE_AND_LOAD);
    m.is_popup = true;
    set_keys(h, "PopupSaveLoad.wnd");
    init_windows(wm, h, "PopupSaveLoad.wnd", true);
}

/// `SaveLoadMenuFullScreenInit(layout, userData)`.
pub fn save_load_menu_full_screen_init<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, layout_type: Option<i32>) {
    shell::show_shell_map(wm, h, true);
    let m = h.save_load_menu();
    m.is_popup = false;
    m.current_layout_type = layout_type.unwrap_or(SLLT_LOAD_ONLY);
    set_keys(h, "SaveLoad.wnd");
    if !init_windows(wm, h, "SaveLoad.wnd", false) {
        return;
    }
    wm.layout_hide(layout, false);
    let m = h.save_load_menu();
    m.just_entered = true;
    m.initial_gadget_delay = 2;
    let parent = m.parent;
    if let Some(p) = parent {
        wm.win_hide(p, true);
    }
    h.save_load_menu().is_shutting_down = false;
}

/// `SaveLoadMenuShutdown(layout, userData)`: `immediate` is `*(Bool *)userData`.
pub fn save_load_menu_shutdown<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, immediate: Option<bool>) {
    let Some(pop_immediate) = immediate else {
        wm.set_undefined("SaveLoadMenuShutdown: no user data (*(Bool *)NULL)");
        return;
    };
    if pop_immediate {
        wm.layout_hide(layout, true);
        shell::shutdown_complete(wm, h, Some(layout), false);
        return;
    }
    wt::reverse(wm, h, b"SaveLoadMenuFade");
    h.save_load_menu().is_shutting_down = true;
}

/// `SaveLoadMenuUpdate(layout, userData)`.
pub fn save_load_menu_update<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    // (#89: ScoreScreen.cpp's globals)
    let dont_show = h.score_screen_globals().is_some_and(|g| g.dont_show_main_menu);
    if dont_show && h.save_load_menu().just_entered {
        h.save_load_menu().just_entered = false;
    }
    let replay = h.score_screen_globals().is_some_and(|g| g.replay_was_pressed);
    if replay && h.save_load_menu().just_entered {
        h.save_load_menu().just_entered = false;
        if let Some(g) = h.score_screen_globals() {
            g.replay_was_pressed = false;
        }
    }
    let m = h.save_load_menu();
    if m.just_entered {
        if m.initial_gadget_delay == 1 {
            wt::remove(wm, h, b"MainMenuDefaultMenuLogoFade", false);
            wt::set_group(wm, h, b"SaveLoadMenuFade", false);
            let m = h.save_load_menu();
            m.initial_gadget_delay = 2;
            m.just_entered = false;
        } else {
            m.initial_gadget_delay -= 1;
        }
    }
    if h.save_load_menu().is_shutting_down && shell::is_anim_finished(wm, h) && wt::is_finished(wm) {
        shell::shutdown_complete(wm, h, Some(layout), false);
    }
}

/// `SaveLoadMenuInput(window, msg, mData1, mData2)`.
pub fn save_load_menu_input<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR {
        return MsgHandled::Ignored;
    }
    if (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 {
        let m = h.save_load_menu();
        let (parent, back_key, delete, listbox, frame) = (m.parent, m.button_back_key, m.delete_confirm, m.listbox_games, m.button_frame);
        let button = wm.win_get_window_from_id(parent, back_key);
        hide(wm, delete, true, "SaveLoadMenuInput: no DeleteConfirmParent (deleteConfirm->winHide)");
        enable(wm, listbox, true, "SaveLoadMenuInput: no games list box (listboxGames->winEnable)");
        enable(wm, frame, true, "SaveLoadMenuInput: no MenuButtonFrame (buttonFrame->winEnable)");
        if wm.undefined.is_some() {
            return MsgHandled::Handled;
        }
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), back_key as u32);
    }
    MsgHandled::Handled
}

/// `doLoadGame()`.
fn do_load_game<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H) {
    let listbox = h.save_load_menu().listbox_games;
    let (_, data) = selected_item(wm, h, listbox);
    let Some(info) = game(h, data) else {
        wm.set_undefined("doLoadGame: no selected game info (TheGameState->loadGame(*NULL))");
        return;
    };
    if !h.shell().is_shell_active() {
        h.destroy_quit_menu(wm);
    } else {
        wt::remove(wm, h, b"MainMenuLoadReplayMenu", false);
        wt::remove(wm, h, b"MainMenuLoadReplayMenuBack", false);
    }
    // (`prepareNewGame(GAME_SINGLE_PLAYER, DIFFICULTY_NORMAL, 0)` and `TheGameState->loadGame(*selectedGameInfo)`; a
    // failed load's `clearGameData`, `reset` and `showShell(TRUE)` are the host's)
    h.request(ShellRequest::LoadGame { filename: info.filename });
}

/// `closeSaveMenu(window)`.
fn close_save_menu<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId) {
    if h.save_load_menu().is_popup {
        if let Some(l) = wm.ext[window].layout {
            wm.layout_hide(l, true);
        }
    } else {
        shell::hide_shell(wm, h);
    }
}

/// `setEditDescription(editControl)`.
fn set_edit_description<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, edit: Option<WinId>) {
    let desc: Vec<u16> = match h.current_campaign() {
        Some((label, mission)) => {
            let mut d = h.fetch_text(&label);
            d.extend(wide(&format!(" {}", mission + 1)));
            d
        }
        None => {
            let map = h.map_name();
            let leaf = match map.iter().rposition(|&c| c == b'\\') {
                Some(i) => &map[i + 1..],
                None => &map[..],
            };
            let mut d: Vec<u16> = leaf.iter().map(|&c| u16::from(c)).collect();
            if d.len() >= 4 && d[d.len() - 4] == u16::from(b'.') {
                d.truncate(d.len() - 4);
            }
            d
        }
    };
    te::set_text(wm, h, edit, &desc);
}

/// `processLoadButtonPress(window)`.
fn process_load_button_press<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId) {
    let listbox = h.save_load_menu().listbox_games;
    let (_, data) = selected_item(wm, h, listbox);
    if game(h, data).is_none() {
        return;
    }
    if h.shell().is_shell_active() {
        close_save_menu(wm, h, window);
        do_load_game(wm, h);
    } else {
        let m = h.save_load_menu();
        let (listbox, frame, load) = (m.listbox_games, m.button_frame, m.load_confirm);
        enable(wm, listbox, false, "processLoadButtonPress: no games list box");
        enable(wm, frame, false, "processLoadButtonPress: no MenuButtonFrame");
        hide(wm, load, false, "processLoadButtonPress: no LoadConfirmParent");
    }
}

/// The file type of a save from this menu: normal with save-and-load, a mission save otherwise.
fn file_type<H: SaveLoadHost>(h: &mut H) -> i32 {
    if h.save_load_menu().current_layout_type == SLLT_SAVE_AND_LOAD {
        SAVE_FILE_TYPE_NORMAL
    } else {
        SAVE_FILE_TYPE_MISSION
    }
}

/// The list and the frame enabled again, the buttons set.
fn reenable<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H) {
    let m = h.save_load_menu();
    let (listbox, frame) = (m.listbox_games, m.button_frame);
    enable(wm, listbox, true, "SaveLoadMenuSystem: no games list box (listboxGames->winEnable)");
    enable(wm, frame, true, "SaveLoadMenuSystem: no MenuButtonFrame (buttonFrame->winEnable)");
    update_menu_actions(wm, h);
}

/// `SaveLoadMenuSystem(window, msg, mData1, mData2)`.
pub fn save_load_menu_system<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        lb::glm::DOUBLE_CLICKED => {
            let control = data_win(d1);
            let key = h.save_load_menu().listbox_games_key;
            if let Some(l) = wm.win_get_window_from_id(Some(window), key) {
                lb::set_selected(wm, h, l, &[d2 as i32]);
                if control == Some(l) {
                    process_load_button_press(wm, h, window);
                }
            }
        }
        lb::glm::SELECTED => {
            let control = data_win(d1);
            let key = h.save_load_menu().listbox_games_key;
            let l = wm.win_get_window_from_id(Some(window), key);
            if l.is_some() && control == l {
                update_menu_actions(wm, h);
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            save_load_selected(wm, h, window, control);
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// `SaveLoadMenuSystem`'s `GBM_SELECTED`.
fn save_load_selected<H: SaveLoadHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, control: WinId) {
    let id = wm.tree.windows[control].id;
    let m = h.save_load_menu().clone();
    if id == m.button_load_key {
        process_load_button_press(wm, h, window);
    } else if id == m.button_save_key {
        let (_, data) = selected_item(wm, h, m.listbox_games);
        if game(h, data).is_none() {
            hide(wm, m.save_desc, false, "SaveLoadMenuSystem: no SaveDescParent (saveDesc->winHide)");
            set_edit_description(wm, h, m.edit_desc);
            enable(wm, m.listbox_games, false, "SaveLoadMenuSystem: no games list box (listboxGames->winEnable)");
            wm.win_set_focus(h, m.edit_desc);
        } else {
            enable(wm, m.listbox_games, false, "SaveLoadMenuSystem: no games list box (listboxGames->winEnable)");
            enable(wm, m.button_frame, false, "SaveLoadMenuSystem: no MenuButtonFrame (buttonFrame->winEnable)");
            hide(wm, m.overwrite_confirm, false, "SaveLoadMenuSystem: no OverwriteConfirmParent (overwriteConfirm->winHide)");
        }
    } else if id == m.button_delete_key {
        let (_, data) = selected_item(wm, h, m.listbox_games);
        if game(h, data).is_some() {
            enable(wm, m.listbox_games, false, "SaveLoadMenuSystem: no games list box (listboxGames->winEnable)");
            enable(wm, m.button_frame, false, "SaveLoadMenuSystem: no MenuButtonFrame (buttonFrame->winEnable)");
            hide(wm, m.delete_confirm, false, "SaveLoadMenuSystem: no DeleteConfirmParent (deleteConfirm->winHide)");
        }
    } else if id == m.button_back_key {
        if m.is_popup {
            close_save_menu(wm, h, window);
        } else {
            shell::pop(wm, h);
        }
    } else if id == m.button_delete_confirm || id == m.button_delete_cancel {
        if id == m.button_delete_confirm {
            let (_, data) = selected_item(wm, h, m.listbox_games);
            let Some(info) = game(h, data) else {
                wm.set_undefined("SaveLoadMenuSystem: delete confirmed with no save selected (selectedGameInfo->filename on NULL)");
                return;
            };
            h.delete_save_file(&info.filename);
            let layout_type = h.save_load_menu().current_layout_type;
            populate_save_game_listbox(wm, h, m.listbox_games, layout_type);
        }
        hide(wm, m.delete_confirm, true, "SaveLoadMenuSystem: no DeleteConfirmParent (deleteConfirm->winHide)");
        reenable(wm, h);
    } else if id == m.button_overwrite_cancel || id == m.button_overwrite_confirm {
        hide(wm, m.overwrite_confirm, true, "SaveLoadMenuSystem: no OverwriteConfirmParent (overwriteConfirm->winHide)");
        if id == m.button_overwrite_confirm {
            let (_, data) = selected_item(wm, h, m.listbox_games);
            let info = game(h, data);
            reenable(wm, h);
            close_save_menu(wm, h, window);
            let ft = file_type(h);
            let Some(info) = info else {
                wm.set_undefined("SaveLoadMenuSystem: overwrite confirmed with no save selected (selectedGameInfo->saveGameInfo on NULL)");
                return;
            };
            h.request(ShellRequest::SaveGame { filename: info.filename, description: info.save_game_info.description, file_type: ft });
        } else {
            reenable(wm, h);
        }
    } else if id == m.button_save_desc_confirm {
        let desc = te::get_text(wm, h, m.edit_desc);
        hide(wm, m.save_desc, true, "SaveLoadMenuSystem: no SaveDescParent (saveDesc->winHide)");
        reenable(wm, h);
        close_save_menu(wm, h, window);
        let (_, data) = selected_item(wm, h, m.listbox_games);
        let filename = game(h, data).map(|g| g.filename).unwrap_or_default();
        let ft = file_type(h);
        h.request(ShellRequest::SaveGame { filename, description: desc, file_type: ft });
    } else if id == m.button_save_desc_cancel {
        hide(wm, m.save_desc, true, "SaveLoadMenuSystem: no SaveDescParent (saveDesc->winHide)");
        reenable(wm, h);
    } else if id == m.button_load_confirm || id == m.button_load_cancel {
        hide(wm, m.load_confirm, true, "SaveLoadMenuSystem: no LoadConfirmParent (loadConfirm->winHide)");
        reenable(wm, h);
        if id == m.button_load_confirm {
            close_save_menu(wm, h, window);
            do_load_game(wm, h);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::shell_gui::synthetic::window_transitions_ini;
    use crate::client::shell_gui::tests::shell_files;
    use crate::client::shell_gui::{start_shell, ShellGui};
    use crate::client::shell::tests::Files;
    use crate::client::shell::ShellHost;
    use crate::client::window_manager::GuiHost;
    use crate::client::window_script::win_status;

    pub(crate) type H = ShellGui<Files>;

    fn date(month: u16, day: u16, hour: u16, minute: u16) -> SaveDate {
        SaveDate { year: 2026, month, day, day_of_week: 0, hour, minute, second: 0, milliseconds: 0 }
    }

    fn info(desc: &str, label: &str, d: SaveDate, file_type: i32) -> Option<SaveGameInfo> {
        Some(SaveGameInfo { description: wide(desc), map_label: label.as_bytes().to_vec(), date: d, save_file_type: file_type, ..Default::default() })
    }

    /// The save directory of the tests, in the order the host lists it.
    fn folder() -> Vec<(Vec<u8>, Option<SaveGameInfo>)> {
        vec![
            (b"00000001.sav".to_vec(), info("Alpha", "MAP:A", date(11, 9, 10, 5), SAVE_FILE_TYPE_NORMAL)),
            (b"00000002.sav".to_vec(), info("", "MAP:Tank", date(11, 10, 14, 30), SAVE_FILE_TYPE_NORMAL)),
            (b"00000003.SAV".to_vec(), info("Mission", "MAP:M", date(11, 8, 0, 7), SAVE_FILE_TYPE_MISSION)),
            (b"notes.txt".to_vec(), info("Text", "MAP:T", date(12, 1, 1, 1), SAVE_FILE_TYPE_NORMAL)),
            (b"broken.sav".to_vec(), None),
            (b"00000004.sav".to_vec(), info("", "NOTEXT:Raw", date(11, 9, 10, 5), SAVE_FILE_TYPE_NORMAL)),
        ]
    }

    /// The shell from the synthetic files and INI, started on the main menu, with [`folder`] as its save directory.
    pub(crate) fn booted() -> (GameWindowManager, H) {
        let mut wm = GameWindowManager::new((800, 600));
        wt::load_window_transitions(&mut wm, wt::WINDOW_TRANSITIONS_INI, &window_transitions_ini()).expect("the synthetic INI reads");
        let mut h = ShellGui::new(shell_files());
        h.save_files = folder();
        h.time_ms = 1000;
        start_shell(&mut wm, &mut h);
        shell::update(&mut wm, &mut h);
        (wm, h)
    }

    pub(crate) fn tick(wm: &mut GameWindowManager, h: &mut H) {
        h.time_ms += 40;
        wm.time_ms = h.time_ms;
        shell::update(wm, h);
        wm.update(h);
    }

    pub(crate) fn settle(wm: &mut GameWindowManager, h: &mut H) {
        for _ in 0..400 {
            let done = wt::is_finished(wm) && wm.transitions.pending.is_none() && shell::is_anim_finished(wm, h);
            tick(wm, h);
            if done {
                return;
            }
        }
        panic!("the transitions never finish");
    }

    pub(crate) fn named(wm: &GameWindowManager, file: &str, name: &str) -> WinId {
        wm.tree.find_by_name(format!("{file}:{name}").as_bytes()).unwrap_or_else(|| panic!("{name}"))
    }

    pub(crate) fn click(wm: &mut GameWindowManager, h: &mut H, w: WinId) {
        let (x, y) = wm.tree.screen_position(w);
        let p = (x + 5, y + 5);
        wm.win_process_mouse_event(h, gwm::MOUSE_POS, p, (0, 0));
        wm.win_process_mouse_event(h, gwm::LEFT_DOWN, p, (0, 0));
        wm.win_process_mouse_event(h, gwm::LEFT_UP, p, (0, 0));
    }

    pub(crate) fn hidden(wm: &GameWindowManager, w: WinId) -> bool {
        wm.tree.windows[w].status & win_status::HIDDEN != 0
    }

    pub(crate) fn enabled(wm: &GameWindowManager, w: WinId) -> bool {
        wm.tree.windows[w].status & win_status::ENABLED != 0
    }

    /// Each row's text in its three columns, its colour and its item data.
    fn rows(wm: &mut GameWindowManager, h: &mut H, l: WinId) -> Vec<(String, String, String, i32, u32)> {
        (0..lb::get_num_entries(wm, l))
            .map(|r| {
                let (t0, c) = lb::get_text_and_color(wm, h, l, r, 0);
                let t1 = lb::get_text(wm, h, l, r, 1);
                let t2 = lb::get_text(wm, h, l, r, 2);
                let d = lb::get_item_data(wm, h, l, r, 0);
                (String::from_utf16_lossy(&t0), String::from_utf16_lossy(&t1), String::from_utf16_lossy(&t2), c, d)
            })
            .collect()
    }

    /// The main menu's *Load/Replay*, then *Load Game*, until the save/load screen is the top and faded in.
    fn to_save_load(wm: &mut GameWindowManager, h: &mut H) {
        wm.win_process_key(h, key::ENTER, key::STATE_DOWN);
        settle(wm, h);
        let b = named(wm, "MainMenu.wnd", "ButtonLoadReplay");
        click(wm, h, b);
        settle(wm, h);
        let b = named(wm, "MainMenu.wnd", "ButtonLoadGame");
        click(wm, h, b);
        settle(wm, h);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/SaveLoad.wnd");
        // (`initialGadgetDelay` 2: the second update sets `SaveLoadMenuFade`)
        tick(wm, h);
        tick(wm, h);
        settle(wm, h);
    }

    /// #78: the list (`populateSaveGameListbox`) and the full screen's init, predicted from PopupSaveLoad.cpp and
    /// GameState.cpp before the run. The directory: `00000001.sav` "Alpha" (11/9 10:05), `00000002.sav` with no
    /// description and the map label `MAP:Tank` (11/10 14:30), `00000003.SAV` a mission save (11/8 0:07), `notes.txt`,
    /// `broken.sav` that does not read, `00000004.sav` with no description and a label the text lacks (`NOTEXT:Raw`) at
    /// the same time as `00000001.sav`.
    ///
    /// - load-only (the full screen): no new-game row; four rows, newest first: `00000002` ("T:MAP:Tank", the label's
    ///   text), `00000001` ("Alpha"), `00000004` (the raw label `NOTEXT:Raw`, after `00000001`: equal dates keep the
    ///   order), `00000003` ("Mission"); colours 170,170,235 / 255,255,255 / 170,170,235 / 200,255,200 (the mission's);
    ///   time "2:30 PM", date "11/10/2026" on the first, "12:07 AM" on the last; item data 1 .. 4; row 0 selected;
    ///   `notes.txt` and `broken.sav` left out;
    /// - *Save* disabled (load-only); *Load* and *Delete* enabled (row 0 is a save); the three confirmations hidden,
    ///   the delete one hidden as the file has it; the parent hidden until `SaveLoadMenuFade`, which the second update
    ///   sets (and the transitions show it); not modal; the focus on the parent.
    ///
    /// **Result (2026-11-10):** one miss: the focus is nobody's. Init gives it to the parent, then hides the parent,
    /// and hiding the focused window drops the focus (`windowHiding`), upstream's order; the fade shows the parent
    /// without giving it back.
    /// - the popup (save-and-load): a first row "T:GUI:NewSaveGame" (200,200,255, no item data), selected: *Load* and
    ///   *Delete* disabled, *Save* enabled; the popup's parent modal.
    ///
    /// A second miss, in the test's prediction of the list box: the new-game row's other two columns read empty (a
    /// cell never written has no text), not the one space an empty text is added as.
    #[test]
    fn the_list_of_saves_and_the_init() {
        let (mut wm, mut h) = booted();
        to_save_load(&mut wm, &mut h);
        let f = "SaveLoad.wnd";
        let l = named(&wm, f, "ListboxGames");
        let r = rows(&mut wm, &mut h, l);
        let c = |r, g, b| wt::game_make_color(r, g, b, 255);
        assert_eq!(
            r,
            [
                ("T:MAP:Tank".into(), "2:30 PM".into(), "11/10/2026".into(), c(170, 170, 235), 1),
                ("Alpha".into(), "10:05 AM".into(), "11/9/2026".into(), c(255, 255, 255), 2),
                ("NOTEXT:Raw".into(), "10:05 AM".into(), "11/9/2026".into(), c(170, 170, 235), 3),
                ("Mission".into(), "12:07 AM".into(), "11/8/2026".into(), c(200, 255, 200), 4),
            ]
        );
        let names: Vec<_> = h.save_load.available_games.iter().map(|g| String::from_utf8_lossy(&g.filename).into_owned()).collect();
        assert_eq!(names, ["00000002.sav", "00000001.sav", "00000004.sav", "00000003.SAV"]);
        assert_eq!(lb::get_selected(&mut wm, &mut h, l), [0]);
        assert!(!enabled(&wm, named(&wm, f, "ButtonSave")));
        assert!(enabled(&wm, named(&wm, f, "ButtonLoad")) && enabled(&wm, named(&wm, f, "ButtonDelete")));
        for n in ["OverwriteConfirmParent", "LoadConfirmParent", "SaveDescParent", "DeleteConfirmParent"] {
            assert!(hidden(&wm, named(&wm, f, n)), "{n}");
        }
        let p = named(&wm, f, "SaveLoadMenu");
        assert!(!hidden(&wm, p), "SaveLoadMenuFade shows the parent");
        assert!(wm.modal.is_empty());
        // (prediction missed: init's `parent->winHide(TRUE)` drops the focus it had just given, `windowHiding`)
        assert_eq!(wm.keyboard_focus, None);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);

        // the popup: save-and-load
        let pl = shell::get_save_load_menu_layout(&mut wm, &mut h).expect("the popup loads");
        shell::run_layout(&mut wm, &mut h, pl, 0, None);
        assert!(h.save_load.is_popup && h.save_load.current_layout_type == SLLT_SAVE_AND_LOAD);
        let f = "PopupSaveLoad.wnd";
        let l = named(&wm, f, "ListboxGames");
        let r = rows(&mut wm, &mut h, l);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], ("T:GUI:NewSaveGame".into(), String::new(), String::new(), c(200, 200, 255), 0));
        assert_eq!(r[1].0, "T:MAP:Tank");
        assert!(enabled(&wm, named(&wm, f, "ButtonSave")));
        assert!(!enabled(&wm, named(&wm, f, "ButtonLoad")) && !enabled(&wm, named(&wm, f, "ButtonDelete")));
        assert_eq!(wm.modal.first().copied(), Some(named(&wm, f, "SaveLoadMenu")));
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    }

    /// #78: the full screen's load, delete and back, predicted before the run:
    ///
    /// - a click on row 1 (`00000001.sav`) selects it (`GLM_SELECTED`: the buttons set); *Delete* shows the delete
    ///   confirmation and disables the list and the frame; *Cancel* hides it and enables them; *Delete* and *Confirm*:
    ///   the host's list loses `00000001.sav`, the request `DeleteSave`, the list filled again (three rows, row 0
    ///   selected);
    /// - Escape (released) with the delete confirmation shown hides it and presses *Back*: the screen's shutdown
    ///   reverses `SaveLoadMenuFade` and the update completes it once the transitions are done: the main menu again,
    ///   its init run (`buttonPushed` cleared);
    /// - back on the screen, a double click on row 1 (`00000004.sav` now) is `GLM_DOUBLE_CLICKED`: the shell is
    ///   active, so the menu closes (`hideShell`: the shell inactive, the screen's immediate shutdown hides its layout,
    ///   which stays on the stack) and the game loads: the request `LoadGame` of `00000004.sav`, and the main menu's
    ///   two load transitions removed;
    /// - a failed load's `showShell(TRUE)` runs the screen's init again: the shell active, the list filled.
    ///
    /// **Result (2026-11-10):** the translation as predicted; three misses in the test's own steps, not the code: the
    /// main menu's `justEntered` is cleared again by its own updates before the test looked (the init is checked by
    /// `buttonPushed` cleared instead); back on the main menu, its load/replay panel's buttons are left hidden by the
    /// transitions, so the test presses *Load Game* with the `GBM_SELECTED` a click sends; and the first screen's
    /// destroyed list box is still in the window tree (found first by name), so the test takes the new one from the
    /// statics.
    #[test]
    fn load_delete_and_back_from_the_full_screen() {
        let (mut wm, mut h) = booted();
        to_save_load(&mut wm, &mut h);
        let f = "SaveLoad.wnd";
        let l = named(&wm, f, "ListboxGames");
        let (lx, ly) = wm.tree.screen_position(l);
        let row_y = |r: i32| ly + 4 + 13 * r;
        let press = |wm: &mut GameWindowManager, h: &mut H, y: i32| {
            wm.win_process_mouse_event(h, gwm::MOUSE_POS, (lx + 20, y), (0, 0));
            wm.win_process_mouse_event(h, gwm::LEFT_DOWN, (lx + 20, y), (0, 0));
            wm.win_process_mouse_event(h, gwm::LEFT_UP, (lx + 20, y), (0, 0));
        };
        press(&mut wm, &mut h, row_y(1));
        assert_eq!(lb::get_selected(&mut wm, &mut h, l), [1]);
        let (del, frame, conf) = (named(&wm, f, "ButtonDelete"), named(&wm, f, "MenuButtonFrame"), named(&wm, f, "DeleteConfirmParent"));
        click(&mut wm, &mut h, del);
        assert!(!hidden(&wm, conf) && !enabled(&wm, l) && !enabled(&wm, frame));
        let cancel = named(&wm, f, "ButtonDeleteCancel");
        click(&mut wm, &mut h, cancel);
        assert!(hidden(&wm, conf) && enabled(&wm, l) && enabled(&wm, frame));
        assert!(h.requests.is_empty());
        click(&mut wm, &mut h, del);
        let confirm = named(&wm, f, "ButtonDeleteConfirm");
        click(&mut wm, &mut h, confirm);
        assert_eq!(h.requests, [ShellRequest::DeleteSave { filename: b"00000001.sav".to_vec() }]);
        h.requests.clear();
        assert!(!h.save_files.iter().any(|(n, _)| n == b"00000001.sav"));
        assert_eq!(rows(&mut wm, &mut h, l).len(), 3);
        assert_eq!(lb::get_selected(&mut wm, &mut h, l), [0]);
        assert!(hidden(&wm, conf) && enabled(&wm, l) && enabled(&wm, frame));
        // Escape, with the delete confirmation shown: hidden, and Back
        click(&mut wm, &mut h, del);
        assert!(!hidden(&wm, conf));
        wm.win_set_focus(&mut h, Some(named(&wm, f, "SaveLoadMenu")));
        wm.win_process_key(&mut h, key::ESC, key::STATE_UP);
        assert!(hidden(&wm, conf) && enabled(&wm, l) && enabled(&wm, frame));
        assert!(h.save_load.is_shutting_down && h.shell.pending_pop);
        settle(&mut wm, &mut h);
        settle(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 1);
        assert!(!h.main_menu.button_pushed && !h.main_menu.is_shutting_down, "the main menu's init ran again");
        // back, and a double click on row 1
        settle(&mut wm, &mut h);
        to_save_load_again(&mut wm, &mut h);
        // (the screen's new list box: the first one's windows are destroyed, not gone from the tree)
        let l = h.save_load.listbox_games.expect("the list box");
        press(&mut wm, &mut h, row_y(1));
        h.time_ms += 100;
        wm.time_ms = h.time_ms;
        press(&mut wm, &mut h, row_y(1));
        assert_eq!(h.requests, [ShellRequest::LoadGame { filename: b"00000004.sav".to_vec() }]);
        assert!(!h.shell.is_shell_active());
        let top = h.shell.top().expect("the screen stays on the stack");
        assert_eq!(wm.layouts[top].filename, b"Menus/SaveLoad.wnd");
        assert!(wm.layouts[top].hidden);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        // a failed load: `showShell(TRUE)`
        shell::show_shell(&mut wm, &mut h, true);
        assert!(h.shell.is_shell_active() && h.save_load.just_entered);
        assert_eq!(rows(&mut wm, &mut h, l).len(), 3);
        assert!(!h.untranslated.iter().any(|f| f.starts_with(b"SaveLoad")), "{:?}", h.untranslated);
    }

    /// The main menu's *Load Game* again (`buttonPushed` was cleared by its init), pressed as a click presses it
    /// (`GBM_SELECTED` to the menu; its panel's buttons were left hidden by the transitions).
    fn to_save_load_again(wm: &mut GameWindowManager, h: &mut H) {
        let b = named(wm, "MainMenu.wnd", "ButtonLoadGame");
        let p = named(wm, "MainMenu.wnd", "MainMenuParent");
        wm.win_send_system_msg(h, Some(p), ggm::GBM_SELECTED, win_data(Some(b)), 0);
        settle(wm, h);
        settle(wm, h);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/SaveLoad.wnd");
        tick(wm, h);
        tick(wm, h);
        settle(wm, h);
    }

    /// #78: the in-game popup's save and load (~~not reachable in the client: QuitMenu.cpp is not translated~~
    /// **Corrected 2026-11-10 (#87):** reached through the translated quit menu, `crate::client::quit_menu`), predicted
    /// before the run, with the shell not active (in a game):
    ///
    /// - *Save* on the new-game row: the description window shown with the default text (no campaign: the map's name,
    ///   which the shell's host does not have: empty; the map's cases are [`edit_description_dates_and_a_missing_window`]'s),
    ///   the list disabled, the frame not (upstream's commented-out line), the focus on the entry; *Confirm* after the
    ///   player typed "Foo": the popup's layout hidden, the request `SaveGame` with no file name, "Foo" and
    ///   `SAVE_FILE_TYPE_NORMAL`;
    /// - *Save* on a save (row 1, `00000002.sav`): the overwrite confirmation, the list and the frame disabled; its
    ///   *Confirm*: `SaveGame` of `00000002.sav` with the save's own description (empty) and the normal type;
    /// - *Load* on row 1 in a game: the load confirmation (no load yet); its *Confirm*: `destroyQuitMenu` (~~recorded~~
    ///   **Corrected 2026-11-10 (#87):** translated: nothing to destroy here) and
    ///   `LoadGame` of `00000002.sav`; *Back*: the popup's layout hidden.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn the_popup_saves_and_loads_in_a_game() {
        let (mut wm, mut h) = booted();
        h.shell.is_shell_active = false;
        let pl = shell::get_save_load_menu_layout(&mut wm, &mut h).expect("the popup loads");
        wm.layout_hide(pl, false);
        shell::run_layout(&mut wm, &mut h, pl, 0, None);
        let f = "PopupSaveLoad.wnd";
        let (l, frame, save, desc, entry) = (named(&wm, f, "ListboxGames"), named(&wm, f, "MenuButtonFrame"), named(&wm, f, "ButtonSave"), named(&wm, f, "SaveDescParent"), named(&wm, f, "EntryDesc"));
        click(&mut wm, &mut h, save);
        assert!(!hidden(&wm, desc) && !enabled(&wm, l) && enabled(&wm, frame));
        assert_eq!(wm.keyboard_focus, Some(entry));
        let text = te::get_text(&mut wm, &mut h, Some(entry));
        assert_eq!(String::from_utf16_lossy(&text), "", "the map name: none set yet");
        // (the host's map name is the default's, empty: set the description as the player would)
        te::set_text(&mut wm, &mut h, Some(entry), &wide("Foo"));
        let ok = named(&wm, f, "ButtonSaveDescConfirm");
        click(&mut wm, &mut h, ok);
        assert_eq!(h.requests, [ShellRequest::SaveGame { filename: Vec::new(), description: wide("Foo"), file_type: SAVE_FILE_TYPE_NORMAL }]);
        assert!(wm.layouts[pl].hidden && hidden(&wm, desc) && enabled(&wm, l));
        h.requests.clear();
        // overwrite
        wm.layout_hide(pl, false);
        lb::set_selected(&mut wm, &mut h, l, &[1]);
        wm.win_send_system_msg(&mut h, Some(named(&wm, f, "SaveLoadMenu")), lb::glm::SELECTED, win_data(Some(l)), 1);
        assert!(enabled(&wm, named(&wm, f, "ButtonLoad")));
        click(&mut wm, &mut h, save);
        let ow = named(&wm, f, "OverwriteConfirmParent");
        assert!(!hidden(&wm, ow) && !enabled(&wm, l) && !enabled(&wm, frame));
        let ok = named(&wm, f, "ButtonOverwriteConfirm");
        click(&mut wm, &mut h, ok);
        assert_eq!(h.requests, [ShellRequest::SaveGame { filename: b"00000002.sav".to_vec(), description: Vec::new(), file_type: SAVE_FILE_TYPE_NORMAL }]);
        h.requests.clear();
        // load in a game: confirmed first
        wm.layout_hide(pl, false);
        let load = named(&wm, f, "ButtonLoad");
        click(&mut wm, &mut h, load);
        let lc = named(&wm, f, "LoadConfirmParent");
        assert!(!hidden(&wm, lc) && h.requests.is_empty());
        let ok = named(&wm, f, "ButtonLoadConfirm");
        click(&mut wm, &mut h, ok);
        assert_eq!(h.requests, [ShellRequest::LoadGame { filename: b"00000002.sav".to_vec() }]);
        // (~~`destroyQuitMenu` recorded as untranslated~~ **Corrected 2026-11-10 (#87):** translated; with no quit menu
        // made it destroys nothing)
        assert!(!h.untranslated.contains(&b"destroyQuitMenu".to_vec()));
        assert!(h.quit_menu.full_quit_menu_layout.is_none() && !h.quit_menu.is_visible);
        wm.layout_hide(pl, false);
        let back = named(&wm, f, "ButtonBack");
        click(&mut wm, &mut h, back);
        assert!(wm.layouts[pl].hidden);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    }

    /// #78: `setEditDescription`'s map name (`Maps\Foo\Foo.map` gives "Foo"; a name without a backslash is kept, its
    /// extension cut when the fourth character from the end is a dot), `en_us_*`'s noon and midnight, and a file without
    /// its button frame (upstream's `NULL` call, recorded). Predicted before the run; **Result (2026-11-10):** one miss,
    /// in the prediction: `ab.cde` was predicted kept, but its fourth character from the end **is** the dot (a, b, `.`,
    /// c, d, e), so upstream's four `removeLastChar` give "ab"; the code was right.
    #[test]
    fn edit_description_dates_and_a_missing_window() {
        let (mut wm, mut h) = booted();
        let pl = shell::get_save_load_menu_layout(&mut wm, &mut h).expect("the popup");
        shell::run_layout(&mut wm, &mut h, pl, 0, None);
        let entry = named(&wm, "PopupSaveLoad.wnd", "EntryDesc");
        for (map, want) in [(&b"Maps\\Foo\\Foo.map"[..], "Foo"), (b"Plain", "Plain"), (b"ab.cde", "ab"), (b"x.map", "x")] {
            let mut host = MapHost { h: &mut h, map: map.to_vec() };
            set_edit_description(&mut wm, &mut host, Some(entry));
            let t = te::get_text(&mut wm, &mut h, Some(entry));
            assert_eq!(String::from_utf16_lossy(&t), want);
        }
        let noon = date(1, 2, 12, 0);
        assert_eq!(String::from_utf16_lossy(&en_us_time_no_seconds(&noon)), "12:00 PM");
        assert_eq!(String::from_utf16_lossy(&en_us_short_date(&noon)), "1/2/2026");
        assert!(is_save_file(b"a.SaV") && !is_save_file(b"a.sav.bak") && !is_save_file(b"sav"));
        // a file without its button frame
        let mut f = shell_files();
        let wnd = String::from_utf8(f.files[&b"Window\\Menus/PopupSaveLoad.wnd"[..]].clone()).expect("text").replace("PopupSaveLoad.wnd:MenuButtonFrame", "PopupSaveLoad.wnd:Elsewhere");
        f.files.insert(b"Window\\Menus/PopupSaveLoad.wnd".to_vec(), wnd.into_bytes());
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = ShellGui::new(f);
        let pl = shell::get_save_load_menu_layout(&mut wm, &mut h).expect("the popup");
        shell::run_layout(&mut wm, &mut h, pl, 0, None);
        assert!(wm.undefined.as_deref().is_some_and(|u| u.contains("MenuButtonFrame")), "{:?}", wm.undefined);
    }

    /// A host whose map name is set (`TheGlobalData->m_mapName`), over the tests' shell host.
    struct MapHost<'a> {
        h: &'a mut H,
        map: Vec<u8>,
    }

    impl GuiHost for MapHost<'_> {
        fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
            self.h.system(wm, func, win, msg, d1, d2)
        }
        fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
            self.h.input(wm, func, win, msg, d1, d2)
        }
    }

    impl ShellHost for MapHost<'_> {
        fn shell(&mut self) -> &mut crate::client::shell::Shell {
            self.h.shell()
        }
        fn create_layout(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
            self.h.create_layout(wm, filename)
        }
        fn create_from_script(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<WinId> {
            self.h.create_from_script(wm, filename)
        }
        fn layout_callback(&mut self, wm: &mut GameWindowManager, func: &[u8], layout: usize, user: Option<bool>) {
            self.h.layout_callback(wm, func, layout, user)
        }
        fn time_ms(&self) -> u32 {
            self.h.time_ms
        }
        fn env(&self) -> crate::client::shell::ShellEnv {
            self.h.env.clone()
        }
        fn request(&mut self, r: ShellRequest) {
            self.h.requests.push(r)
        }
        fn name_to_key(&mut self, name: &[u8]) -> u32 {
            self.h.name_to_key(name)
        }
    }

    impl MenuHost for MapHost<'_> {
        fn main_menu(&mut self) -> &mut crate::client::main_menu::MainMenu {
            &mut self.h.main_menu
        }
        fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
            self.h.fetch_text(label)
        }
    }

    impl SaveLoadHost for MapHost<'_> {
        fn save_load_menu(&mut self) -> &mut SaveLoadMenu {
            &mut self.h.save_load
        }
        fn save_files(&mut self) -> Vec<(Vec<u8>, Option<SaveGameInfo>)> {
            self.h.save_files.clone()
        }
        fn delete_save_file(&mut self, filename: &[u8]) {
            self.h.delete_save_file(filename)
        }
        fn map_name(&self) -> Vec<u8> {
            self.map.clone()
        }
    }
}
