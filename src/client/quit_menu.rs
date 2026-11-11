// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/QuitMenu.cpp (the file's statics,
//     initGadgetsFullQuit, initGadgetsNoSaveQuit, destroyQuitMenu, exitQuitMenu, noExitQuitMenu, surrenderQuitMenu,
//     restartMissionMenu, HideQuitMenu, ToggleQuitMenu, QuitMenuSystem)
//   GeneralsMD/Code/GameEngine/Include/GameClient/InGameUI.h (setQuitMenuVisible, isQuitMenuVisible, setClientQuiet)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   QuitMenu.cpp: Colin Day, October 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The in-game Escape menu (GitHub #87): `Menus/QuitMenu.wnd` (with *Save/Load*), or `Menus/QuitNoSave.wnd` in a
//! multiplayer game or a replay, made the first time it is shown and kept; it opens the in-game save/load popup
//! (`crate::client::save_load_menu`) and the options screen (`crate::client::options_menu`).
//!
//! # As upstream reads it
//!
//! **`ToggleQuitMenu`** (the meta map's `MSG_META_OPTIONS`, Escape): nothing while the intro movie plays, a map loads,
//! the game is ending, or the disconnect screen is up. If the options screen is up (`getOptionsLayout(FALSE)`), its
//! *Back* is pressed (`GBM_SELECTED` to its first window) and that is all; if the save/load popup this menu opened is
//! shown, its *Back* is pressed, the menu forgets the popup, and that is all. Shown: hidden (`isVisible` cleared, the
//! confirmation box destroyed, the game unpaused outside a multiplayer game, the group `QuitNoSaveBack` or
//! `QuitFullBack` reversed). Hidden: the cursor an arrow, the science purchase hidden; the layout made once
//! (`QuitNoSave.wnd` in a multiplayer game or a replay, else `QuitMenu.wnd`) and its buttons found
//! (`initGadgetsNoSaveQuit`, `initGadgetsFullQuit`); its group removed then set (`QuitNoSave`, `QuitFull`); with the
//! UI's input disabled (a cinematic), *Save/Load* and *Options* disabled, else enabled; in a multiplayer game or a
//! skirmish, *Restart* enabled, reading *Surrender* outside a skirmish, the game paused in a skirmish, *Restart*
//! disabled for a dead or victorious player outside a skirmish; else *Restart* enabled, reading *Restart Mission* and
//! *Exit* reading *Exit Mission* outside a replay, the game paused; the confirmation box destroyed, the diplomacy and
//! chat windows and the science purchase hidden; `isVisible` set. Then `setQuitMenuVisible(isVisible)`.
//!
//! **`HideQuitMenu`** (the logic's `closeWindows`, the control bar's): if shown, the back group reversed,
//! `setQuitMenuVisible(FALSE)`, the confirmation box destroyed, the game unpaused outside a multiplayer game.
//!
//! **`QuitMenuSystem`** (`GBM_SELECTED`): *Save/Load*: the shell's popup (`getSaveLoadMenuLayout`) kept as this menu's,
//! its init run (no user data), shown and brought forward. *Exit*: the quit box (`QuitMessageBoxYesNo`,
//! `GUI:QuitPopupTitle`, `GUI:QuitPopupMessage`; yes `exitQuitMenu`, no `noExitQuitMenu`). *Return*: `ToggleQuitMenu`.
//! *Options*: the shell's options layout made (`getOptionsLayout(TRUE)`), its init run, shown, brought forward.
//! *Restart*: in a multiplayer game the surrender box (yes `surrenderQuitMenu`), else the restart box (yes
//! `restartMissionMenu`); either's no `noExitQuitMenu`.
//!
//! **The boxes' answers.** `exitQuitMenu`: the menu destroyed (`destroyQuitMenu`); in a multiplayer game that is not a
//! skirmish or a sandbox, `MSG_SELF_DESTRUCT` (TRUE); `MSG_CLEAR_GAME_DATA`; outside a multiplayer game, unpaused; the
//! client quiet. `surrenderQuitMenu`: destroyed; unless the local side already won, `MSG_SELF_DESTRUCT` (TRUE) and the
//! client quiet. `restartMissionMenu`: destroyed; the map name (a save's: the pristine one); the replay's name; the
//! recording stopped; the rank points, the difficulty and the frame limit read; `clearGameData(FALSE)`,
//! `setQuitting(FALSE)`; then the replay played again, or `MSG_NEW_GAME` (mode, difficulty, rank points, frame limit)
//! on the map with `InitRandom(0)`; the client quiet. `noExitQuitMenu`: the confirmation window forgotten.
//!
//! # Upstream quirks that are kept
//!
//! - `destroyQuitMenu` clears `isVisible` but does **not** call `setQuitMenuVisible(FALSE)`, does not unpause, and
//!   does not forget the save/load popup (`saveLoadMenuLayout`).
//! - Hiding by Escape (`ToggleQuitMenu`) keeps the layouts: the back group hides the windows; showing again makes no
//!   layout anew, only finds its buttons again.
//! - In the no-save layout with the UI's input enabled, *Options* is **not** enabled again (the `else if
//!   (buttonSaveLoadWin)` guards both): a replay's quit menu opened in a cinematic keeps *Options* disabled.
//! - A skirmish is not a multiplayer game (`isInMultiplayerGame` is LAN or Internet): it gets the full layout with
//!   *Save/Load*, and its *Restart* restarts.
//! - The options and save/load checks of `ToggleQuitMenu` come before `isVisible`: Escape with the popup up presses its
//!   *Back* and leaves the menu shown.
//! - The windows used without a check (`buttonRestartWin`, `buttonOptionsWin`: `winEnable` on `NULL`) are recorded as
//!   undefined when missing; `GadgetButtonSetText` checks for `NULL` itself, so a missing button's text is nothing ([`GameWindowManager::set_undefined`]); upstream's
//!   `DEBUG_ASSERTCRASH` lines are compiled out of the release. A layout that does not load (`quitMenuLayout == NULL`)
//!   is upstream's `initGadgets*` first (which find nothing), then the `DEBUG_ASSERTCRASH` branch: not visible.
//! - `quitToDesktopQuitMenu` is referenced only by a commented-out button: not translated (no caller).
//!
//! # Where this differs from upstream
//!
//! - **What the menu reads of the game** (`TheGameLogic`, `TheScriptEngine`, `TheGameState`, `TheRecorder`,
//!   `TheVictoryConditions`, `ThePlayerList`, `TheDisconnectMenu`, `TheInGameUI->getInputEnabled`) is the host's copy,
//!   [`QuitMenu::game`] ([`QuitMenuGame`]), which the owner sets before calling in.
//! - **What it asks of the logic and the engine** is a [`ShellRequest`]: [`ShellRequest::ClearGameData`]
//!   (`MSG_CLEAR_GAME_DATA`), [`ShellRequest::SelfDestruct`] (`MSG_SELF_DESTRUCT`), [`ShellRequest::SetGamePaused`]
//!   (`setGamePaused`), [`ShellRequest::RestartMission`] (`restartMissionMenu`'s engine half). The owner sends the
//!   messages through the logic's own message path.
//! - `TheInGameUI`'s `m_isQuitMenuVisible` and `m_clientQuiet` are [`QuitMenu::quit_menu_visible`] and
//!   [`QuitMenu::client_quiet`]. (#108) The client feeds the first to its translators' `isQuitMenuVisible`, and
//!   clears both as `InGameUI::reset` does when it sees the World's game cleared ([`clear_game_data_windows`]).
//! - (#108) `GameLogic::closeWindows` is the client's [`close_windows`], called when it sees the World's game cleared
//!   (`clearGameData`). ~~Its call from `ScriptActions::doQuickVictory` (a debug script action) is not reached.~~
//!   **Corrected 2026-11-10 (#117):** its call from `ScriptActions::doQuickVictory` reaches the client as
//!   `ScriptInputCall::CloseWindows` (`crate::client::script_input`), on which the client calls it.
//! - ~~`TheMouse->setCursor(ARROW)`,~~ `HideDiplomacy`, `HideInGameChat`, `TheControlBar->hidePurchaseScience` (and,
//!   from `closeWindows`, `ResetDiplomacy`, `ResetInGameChat` and `hideSpecialPowerShortcut`) are the
//!   host's ([`QuitMenuHost::hide_in_game_windows`]): the diplomacy and chat windows are not translated, and the
//!   control bar is not in the shell's window manager. **Corrected 2026-11-10 (#117):** `TheMouse->setCursor(ARROW)`
//!   is queued in [`QuitMenu::mouse_set_cursor`] for the client's translated mouse (`crate::client::mouse`). The
//!   control bar's two calls stay the host's: the windows they hide (`GeneralsExpPoints.wnd:GenExpParent` and the
//!   special power shortcut bar's parent) are layouts of their own that the client's control bar (`ControlBar.wnd`
//!   alone) does not make.
//!
//! # Representation
//!
//! [`QuitMenu`]: the file's statics, with `TheInGameUI`'s two flags and the game's state the menu reads. The layouts
//! are the window manager's indices; the window ids the host's name keys ([`crate::client::shell::ShellHost::name_to_key`]).

use crate::client::gadget;
use crate::client::message_box;
use crate::client::options_menu::OptionsHost;
use crate::client::save_load_menu::SaveLoadHost;
use crate::client::shell::{self, ShellRequest};
use crate::client::window_manager::{data_win, ggm, gwm, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;
use crate::common::recorder::game_mode;

/// What the quit menu reads of the game (the owner's copy).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuitMenuGame {
    /// `TheGameLogic->getGameMode()`.
    pub game_mode: i32,
    /// `TheGameInfo->isSandbox()`.
    pub is_sandbox: bool,
    /// `TheGameLogic->isIntroMoviePlaying()`.
    pub intro_movie_playing: bool,
    /// `TheGameLogic->isLoadingMap()`.
    pub loading_map: bool,
    /// `TheScriptEngine->isGameEnding()`.
    pub game_ending: bool,
    /// `TheDisconnectMenu->isScreenVisible()` (`None`: `TheDisconnectMenu` is `NULL`).
    pub disconnect_screen_visible: Option<bool>,
    /// `TheInGameUI->getInputEnabled()`.
    pub input_enabled: bool,
    /// `ThePlayerList->getLocalPlayer()->isPlayerActive()`.
    pub local_player_active: bool,
    /// `TheVictoryConditions->isLocalAlliedVictory()`.
    pub local_allied_victory: bool,
    /// `TheGlobalData->m_mapName`.
    pub map_name: Vec<u8>,
    /// `TheGameState->getPristineMapName()` when `isInSaveDirectory(m_mapName)` (`None`: the map is not a save's).
    pub pristine_map_name: Option<Vec<u8>>,
    /// `TheRecorder->getCurrentReplayFilename()`.
    pub replay_file: Vec<u8>,
    /// `TheRecorder->getMode() == RECORDERMODETYPE_RECORD`.
    pub recording: bool,
    /// `TheGameLogic->getRankPointsToAddAtGameStart()`.
    pub rank_points: i32,
    /// `TheScriptEngine->getGlobalDifficulty()`.
    pub difficulty: i32,
    /// `TheGameEngine->getFramesPerSecondLimit()`.
    pub fps_limit: i32,
}

impl Default for QuitMenuGame {
    /// No game (`GAME_NONE`), nothing playing or loading, no disconnect menu, the input enabled, the local player
    /// active, the frame limit 30.
    fn default() -> Self {
        QuitMenuGame {
            game_mode: game_mode::NONE,
            is_sandbox: false,
            intro_movie_playing: false,
            loading_map: false,
            game_ending: false,
            disconnect_screen_visible: None,
            input_enabled: true,
            local_player_active: true,
            local_allied_victory: false,
            map_name: Vec::new(),
            pristine_map_name: None,
            replay_file: Vec::new(),
            recording: false,
            rank_points: 0,
            difficulty: crate::common::recorder::DIFFICULTY_NORMAL,
            fps_limit: 30,
        }
    }
}

impl QuitMenuGame {
    /// `TheGameLogic->isInMultiplayerGame()`: LAN or Internet.
    pub fn is_in_multiplayer_game(&self) -> bool {
        self.game_mode == game_mode::LAN || self.game_mode == game_mode::INTERNET
    }
    /// `TheGameLogic->isInSkirmishGame()`.
    pub fn is_in_skirmish_game(&self) -> bool {
        self.game_mode == game_mode::SKIRMISH
    }
    /// `TheGameLogic->isInReplayGame()`.
    pub fn is_in_replay_game(&self) -> bool {
        self.game_mode == game_mode::REPLAY
    }
    /// `TheGameLogic->isInGame()`.
    pub fn is_in_game(&self) -> bool {
        self.game_mode != game_mode::NONE
    }
}

/// QuitMenu.cpp's statics (`NAMEKEY_INVALID` is 0), `TheInGameUI`'s two flags, and the game's state it reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuitMenu {
    pub quit_menu_layout: Option<usize>,
    pub full_quit_menu_layout: Option<usize>,
    pub no_save_load_quit_menu_layout: Option<usize>,
    pub is_visible: bool,
    pub quit_confirmation_window: Option<WinId>,
    /// `saveLoadMenuLayout`: the shell's popup, once *Save/Load* opened it.
    pub save_load_menu_layout: Option<usize>,
    pub button_restart_win: Option<WinId>,
    pub button_save_load_win: Option<WinId>,
    pub button_options_win: Option<WinId>,
    pub button_exit_win: Option<WinId>,
    pub button_exit: i32,
    pub button_restart: i32,
    pub button_return: i32,
    pub button_options: i32,
    pub button_save_load: i32,
    /// `TheInGameUI->m_isQuitMenuVisible`.
    pub quit_menu_visible: bool,
    /// `TheInGameUI->m_clientQuiet`.
    pub client_quiet: bool,
    /// (#117) The menu's `TheMouse->setCursor(c)` calls, in order, for the owner to make on its mouse
    /// (`crate::client::mouse::Mouse::set_cursor`) and clear.
    pub mouse_set_cursor: Vec<i32>,
    /// What the menu reads of the game.
    pub game: QuitMenuGame,
}

/// What the quit menu needs: the save/load popup's host and the options screen's.
pub trait QuitMenuHost: SaveLoadHost + OptionsHost {
    /// QuitMenu.cpp's statics.
    fn quit_menu(&mut self) -> &mut QuitMenu;
    /// ~~`TheMouse->setCursor(Mouse::ARROW)`,~~ (#117: [`QuitMenu::mouse_set_cursor`]) `HideDiplomacy()`, `HideInGameChat()`,
    /// `TheControlBar->hidePurchaseScience()`
    /// (each named as `what`; #108: also `closeWindows`' `ResetDiplomacy()`, `ResetInGameChat()` and
    /// `TheControlBar->hideSpecialPowerShortcut()`): outside the shell's windows.
    fn hide_in_game_windows(&mut self, _what: &'static str) {}
}

/// A name key of the file (`TheNameKeyGenerator->nameToKey("<file>:<name>")`).
fn key<H: QuitMenuHost>(h: &mut H, file: &str, name: &str) -> i32 {
    h.name_to_key(format!("{file}:{name}").as_bytes()) as i32
}

/// `initGadgetsFullQuit()`.
fn init_gadgets_full_quit<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let f = "QuitMenu.wnd";
    let ids = [key(h, f, "ButtonExit"), key(h, f, "ButtonRestart"), key(h, f, "ButtonReturn"), key(h, f, "ButtonOptions"), key(h, f, "ButtonSaveLoad")];
    let q = h.quit_menu();
    [q.button_exit, q.button_restart, q.button_return, q.button_options, q.button_save_load] = ids;
    q.button_restart_win = wm.win_get_window_from_id(None, ids[1]);
    q.button_save_load_win = wm.win_get_window_from_id(None, ids[4]);
    q.button_options_win = wm.win_get_window_from_id(None, ids[3]);
    q.button_exit_win = wm.win_get_window_from_id(None, ids[0]);
}

/// `initGadgetsNoSaveQuit()`.
fn init_gadgets_no_save_quit<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let f = "QuitNoSave.wnd";
    let ids = [key(h, f, "ButtonExit"), key(h, f, "ButtonRestart"), key(h, f, "ButtonReturn"), key(h, f, "ButtonOptions")];
    let q = h.quit_menu();
    [q.button_exit, q.button_restart, q.button_return, q.button_options] = ids;
    q.button_save_load = 0;
    q.button_restart_win = wm.win_get_window_from_id(None, ids[1]);
    q.button_options_win = wm.win_get_window_from_id(None, ids[3]);
    q.button_save_load_win = None;
    q.button_exit_win = wm.win_get_window_from_id(None, ids[0]);
}

/// `destroyQuitMenu()`: both layouts destroyed (`destroyWindows`, `deleteInstance`), the menu not visible.
pub fn destroy_quit_menu<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let q = h.quit_menu();
    q.quit_confirmation_window = None;
    if let Some(l) = q.full_quit_menu_layout.take() {
        shell::destroy_layout(wm, l);
    }
    let q = h.quit_menu();
    if let Some(l) = q.no_save_load_quit_menu_layout.take() {
        shell::destroy_layout(wm, l);
    }
    let q = h.quit_menu();
    q.quit_menu_layout = None;
    q.is_visible = false;
}

/// `exitQuitMenu()` (the quit box's *Yes*).
pub fn exit_quit_menu<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    destroy_quit_menu(wm, h);
    let g = h.quit_menu().game.clone();
    if g.is_in_multiplayer_game() && !g.is_in_skirmish_game() && !g.is_sandbox {
        h.request(ShellRequest::SelfDestruct);
    }
    h.request(ShellRequest::ClearGameData);
    if !g.is_in_multiplayer_game() {
        h.request(ShellRequest::SetGamePaused(false));
    }
    h.quit_menu().client_quiet = true;
}

/// `noExitQuitMenu()` (the boxes' *No*).
pub fn no_exit_quit_menu<H: QuitMenuHost>(h: &mut H) {
    h.quit_menu().quit_confirmation_window = None;
}

/// `surrenderQuitMenu()` (the surrender box's *Yes*).
pub fn surrender_quit_menu<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    destroy_quit_menu(wm, h);
    if h.quit_menu().game.local_allied_victory {
        return;
    }
    h.request(ShellRequest::SelfDestruct);
    h.quit_menu().client_quiet = true;
}

/// `restartMissionMenu()` (the restart box's *Yes*): the menu destroyed, then what the engine does
/// ([`ShellRequest::RestartMission`]).
pub fn restart_mission_menu<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    destroy_quit_menu(wm, h);
    let g = h.quit_menu().game.clone();
    // (a save's map: "Save/..."; the pristine map name instead)
    let map_name = g.pristine_map_name.clone().unwrap_or(g.map_name.clone());
    // (the recording stopped, the rank points, difficulty and frame limit read before `clearGameData(FALSE)` and
    // `setQuitting(FALSE)`; then `playbackFile(replayFile)`, or `m_pendingFile`, `MSG_NEW_GAME` and `InitRandom(0)`)
    h.request(ShellRequest::RestartMission {
        game_mode: g.game_mode,
        difficulty: g.difficulty,
        rank_points: g.rank_points,
        fps: g.fps_limit,
        map_name,
        replay_file: g.replay_file.clone(),
        stop_recording: g.recording,
    });
    h.quit_menu().client_quiet = true;
}

/// The back group reversed: `QuitNoSaveBack` for the no-save layout, `QuitFullBack` for the full one.
fn reverse_back_group<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let q = h.quit_menu();
    let (layout, no_save, full) = (q.quit_menu_layout, q.no_save_load_quit_menu_layout, q.full_quit_menu_layout);
    if layout.is_some() && layout == no_save {
        wt::reverse(wm, h, b"QuitNoSaveBack");
    } else if layout.is_some() && layout == full {
        wt::reverse(wm, h, b"QuitFullBack");
    }
}

/// The confirmation box destroyed and forgotten.
fn destroy_confirmation<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if let Some(w) = h.quit_menu().quit_confirmation_window.take() {
        wm.win_destroy(Some(w));
    }
}

/// `HideQuitMenu()`: safe to call without the menu.
pub fn hide_quit_menu<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.quit_menu().is_visible {
        return;
    }
    reverse_back_group(wm, h);
    let q = h.quit_menu();
    q.quit_menu_visible = false;
    q.is_visible = false;
    destroy_confirmation(wm, h);
    if !h.quit_menu().game.is_in_multiplayer_game() {
        h.request(ShellRequest::SetGamePaused(false));
    }
}

/// `GameLogic::closeWindows()` (GameLogicDispatch.cpp, #108), the client's windows: `HideDiplomacy`,
/// `ResetDiplomacy`, `HideInGameChat`, `ResetInGameChat`, `TheControlBar->hidePurchaseScience` and
/// `hideSpecialPowerShortcut` (the host's: recorded), `HideQuitMenu`, then the options screen's *Back* pressed when
/// `OptionsMenu.wnd:OptionsMenuParent` exists (`GBM_SELECTED` with the *Back* button and its id).
pub fn close_windows<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    for w in ["HideDiplomacy", "ResetDiplomacy", "HideInGameChat", "ResetInGameChat", "TheControlBar->hidePurchaseScience", "TheControlBar->hideSpecialPowerShortcut"] {
        h.hide_in_game_windows(w);
    }
    hide_quit_menu(wm, h);
    let button_id = h.name_to_key(b"OptionsMenu.wnd:ButtonBack") as i32;
    let button = wm.win_get_window_from_id(None, button_id);
    let parent_id = h.name_to_key(b"OptionsMenu.wnd:OptionsMenuParent") as i32;
    if let Some(window) = wm.win_get_window_from_id(None, parent_id) {
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), button_id as WindowMsgData);
    }
}

/// #108: what `GameLogic::clearGameData` does to the quit menu after the logic's half: `TheGameEngine->reset()`'s
/// `InGameUI::reset` (`m_isQuitMenuVisible` and `m_clientQuiet` cleared, [`QuitMenu::quit_menu_visible`] and
/// [`QuitMenu::client_quiet`]), then [`close_windows`]. The client calls it when it sees the World's game cleared.
pub fn clear_game_data_windows<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let q = h.quit_menu();
    q.quit_menu_visible = false;
    q.client_quiet = false;
    close_windows(wm, h);
}

/// `layout->getFirstWindow()`.
fn first_window(wm: &GameWindowManager, l: usize) -> Option<WinId> {
    wm.layouts[l].windows.first().copied()
}

/// `winEnable` of a window upstream calls without a check.
fn enable_unchecked(wm: &mut GameWindowManager, w: Option<WinId>, enable: bool, what: &'static str) {
    match w {
        Some(w) => wm.win_enable(w, enable),
        None => wm.set_undefined(what),
    }
}

/// `GadgetButtonSetText(button, TheGameText->fetch(label))` (nothing for a missing button: its sanity check).
fn set_text<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H, w: Option<WinId>, label: &[u8]) {
    let text = h.fetch_text(label);
    gadget::button_set_text(wm, h, w, &text);
}

/// `ToggleQuitMenu()`.
pub fn toggle_quit_menu<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let g = h.quit_menu().game.clone();
    if g.intro_movie_playing || g.loading_map || g.game_ending {
        return;
    }
    if g.disconnect_screen_visible == Some(true) {
        return;
    }
    // the options screen or the save/load popup up: its Back pressed instead
    if let Some(opt) = shell::get_options_layout(wm, h, false) {
        let parent = first_window(wm, opt);
        let id = h.name_to_key(b"OptionsMenu.wnd:ButtonBack") as i32;
        let back = wm.win_get_window_from_id(parent, id);
        wm.win_send_system_msg(h, parent, ggm::GBM_SELECTED, win_data(back), 0);
        return;
    }
    if let Some(sl) = h.quit_menu().save_load_menu_layout.filter(|&l| !wm.layouts[l].hidden) {
        let parent = first_window(wm, sl);
        let id = h.name_to_key(b"PopupSaveLoad.wnd:ButtonBack") as i32;
        let back = wm.win_get_window_from_id(parent, id);
        wm.win_send_system_msg(h, parent, ggm::GBM_SELECTED, win_data(back), 0);
        h.quit_menu().save_load_menu_layout = None;
        return;
    }
    let q = h.quit_menu();
    if q.is_visible && q.quit_menu_layout.is_some() {
        q.is_visible = false;
        destroy_confirmation(wm, h);
        if !g.is_in_multiplayer_game() {
            h.request(ShellRequest::SetGamePaused(false));
        }
        reverse_back_group(wm, h);
    } else {
        // (#117: ~~the host's, recorded~~ the client's mouse: `Mouse::setCursor`, translated)
        h.quit_menu().mouse_set_cursor.push(crate::client::mouse::cursor::ARROW);
        h.hide_in_game_windows("TheControlBar->hidePurchaseScience");
        if g.is_in_multiplayer_game() || g.is_in_replay_game() {
            if h.quit_menu().no_save_load_quit_menu_layout.is_none() {
                let l = h.create_layout(wm, b"Menus/QuitNoSave.wnd");
                h.quit_menu().no_save_load_quit_menu_layout = l;
            }
            let q = h.quit_menu();
            q.quit_menu_layout = q.no_save_load_quit_menu_layout;
            init_gadgets_no_save_quit(wm, h);
            wt::remove(wm, h, b"QuitNoSave", false);
            wt::set_group(wm, h, b"QuitNoSave", false);
        } else {
            if h.quit_menu().full_quit_menu_layout.is_none() {
                let l = h.create_layout(wm, b"Menus/QuitMenu.wnd");
                h.quit_menu().full_quit_menu_layout = l;
            }
            let q = h.quit_menu();
            q.quit_menu_layout = q.full_quit_menu_layout;
            init_gadgets_full_quit(wm, h);
            wt::remove(wm, h, b"QuitFull", false);
            wt::set_group(wm, h, b"QuitFull", false);
        }
        if h.quit_menu().quit_menu_layout.is_none() {
            // (`DEBUG_ASSERTCRASH(FALSE, ...)`: compiled out)
            let q = h.quit_menu();
            q.is_visible = false;
            q.quit_menu_visible = false;
            return;
        }
        // a cinematic: Save/Load and Options disabled (the save/load window does not fit the letterbox)
        let q = h.quit_menu().clone();
        if !g.input_enabled {
            if let Some(w) = q.button_save_load_win {
                wm.win_enable(w, false);
            }
            enable_unchecked(wm, q.button_options_win, false, "ToggleQuitMenu: no ButtonOptions (buttonOptionsWin->winEnable on NULL)");
        } else if let Some(w) = q.button_save_load_win {
            wm.win_enable(w, true);
            enable_unchecked(wm, q.button_options_win, true, "ToggleQuitMenu: no ButtonOptions (buttonOptionsWin->winEnable on NULL)");
        }
        let restart_missing = "ToggleQuitMenu: no ButtonRestart (buttonRestartWin->winEnable on NULL)";
        if g.is_in_multiplayer_game() || g.is_in_skirmish_game() {
            enable_unchecked(wm, q.button_restart_win, true, restart_missing);
            if !g.is_in_skirmish_game() {
                set_text(wm, h, q.button_restart_win, b"GUI:Surrender");
            }
            if g.is_in_skirmish_game() {
                h.request(ShellRequest::SetGamePaused(true));
            }
            if (!g.local_player_active || g.local_allied_victory) && !g.is_in_skirmish_game() {
                enable_unchecked(wm, q.button_restart_win, false, restart_missing);
            }
        } else {
            enable_unchecked(wm, q.button_restart_win, true, restart_missing);
            if !g.is_in_replay_game() {
                set_text(wm, h, q.button_restart_win, b"GUI:RestartMission");
                set_text(wm, h, q.button_exit_win, b"GUI:ExitMission");
            }
            h.request(ShellRequest::SetGamePaused(true));
        }
        destroy_confirmation(wm, h);
        h.hide_in_game_windows("HideDiplomacy");
        h.hide_in_game_windows("HideInGameChat");
        h.hide_in_game_windows("TheControlBar->hidePurchaseScience");
        h.quit_menu().is_visible = true;
    }
    let q = h.quit_menu();
    q.quit_menu_visible = q.is_visible;
}

/// `QuitMenuSystem(window, msg, mData1, mData2)`.
pub fn quit_menu_system<H: QuitMenuHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else {
                wm.set_undefined("QuitMenuSystem: GBM_SELECTED without a control (control->winGetWindowId on NULL)");
                return MsgHandled::Handled;
            };
            let id = wm.tree.windows[control].id;
            let q = h.quit_menu().clone();
            if id == q.button_save_load {
                let l = shell::get_save_load_menu_layout(wm, h);
                h.quit_menu().save_load_menu_layout = l;
                let Some(l) = l else {
                    wm.set_undefined("QuitMenuSystem: no PopupSaveLoad.wnd (saveLoadMenuLayout->runInit on NULL)");
                    return MsgHandled::Handled;
                };
                shell::run_layout(wm, h, l, 0, None);
                wm.layout_hide(l, false);
                wm.layout_bring_forward(l);
            } else if id == q.button_exit {
                let t = h.fetch_text(b"GUI:QuitPopupTitle");
                let b = h.fetch_text(b"GUI:QuitPopupMessage");
                let w = message_box::quit_message_box_yes_no(wm, h, &t, &b, Some("exitQuitMenu"), Some("noExitQuitMenu"));
                h.quit_menu().quit_confirmation_window = w;
            } else if id == q.button_return {
                toggle_quit_menu(wm, h);
            } else if id == q.button_options {
                let Some(l) = shell::get_options_layout(wm, h, true) else {
                    wm.set_undefined("QuitMenuSystem: no OptionsMenu.wnd (optLayout->runInit on NULL)");
                    return MsgHandled::Handled;
                };
                shell::run_layout(wm, h, l, 0, None);
                wm.layout_hide(l, false);
                wm.layout_bring_forward(l);
            } else if id == q.button_restart {
                let (title, body, yes) = if q.game.is_in_multiplayer_game() {
                    (&b"GUI:SurrenderConfirmationTitle"[..], &b"GUI:SurrenderConfirmation"[..], "surrenderQuitMenu")
                } else {
                    (&b"GUI:RestartConfirmationTitle"[..], &b"GUI:RestartConfirmation"[..], "restartMissionMenu")
                };
                let t = h.fetch_text(title);
                let b = h.fetch_text(body);
                let w = message_box::message_box_yes_no(wm, h, &t, &b, Some(yes), Some("noExitQuitMenu"));
                h.quit_menu().quit_confirmation_window = w;
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// Whether the in-game menus are on screen (new: upstream's one window manager draws whatever is shown): the quit
/// menu, the save/load popup it opened, the options screen, or a box over them (the window manager's modal stack).
pub fn in_game_menus_shown(wm: &GameWindowManager, q: &QuitMenu, options_layout: Option<usize>) -> bool {
    let shown = |l: Option<usize>| l.is_some_and(|l| !wm.layouts[l].deleted && wm.layouts[l].windows.iter().any(|&w| !wm.tree.windows[w].is_hidden()));
    q.is_visible || shown(q.quit_menu_layout) || shown(q.save_load_menu_layout) || shown(options_layout) || !wm.modal.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::save_load_menu::tests::{booted, click, hidden, settle, H};
    use crate::client::window_manager::key;

    /// The last window named `<file>:<name>` (a layout made again makes new windows).
    fn last(wm: &GameWindowManager, file: &str, name: &str) -> WinId {
        let full = format!("{file}:{name}");
        wm.tree.windows.iter().rposition(|x| x.name() == full.as_bytes()).unwrap_or_else(|| panic!("{full}"))
    }

    /// A click on the last window `<file>:<name>`.
    fn press(wm: &mut GameWindowManager, h: &mut H, file: &str, name: &str) {
        let w = last(wm, file, name);
        click(wm, h, w);
    }

    fn text(wm: &GameWindowManager, w: WinId) -> String {
        String::from_utf16_lossy(wm.win_get_text(w))
    }

    /// The shell booted, then hidden as a game's start hides it (`hideShell`), with the game `mode` running.
    fn in_game(mode: i32) -> (GameWindowManager, H) {
        let (mut wm, mut h) = booted();
        shell::hide_shell(&mut wm, &mut h);
        settle(&mut wm, &mut h);
        h.quit_menu.game = QuitMenuGame { game_mode: mode, map_name: b"Maps\\USA01\\USA01.map".to_vec(), rank_points: 7, difficulty: 2, fps_limit: 30, ..Default::default() };
        h.requests.clear();
        (wm, h)
    }

    /// Escape: `MSG_META_OPTIONS`'s `ToggleQuitMenu`, then the transitions run to their end.
    fn escape(wm: &mut GameWindowManager, h: &mut H) {
        toggle_quit_menu(wm, h);
        settle(wm, h);
    }

    /// #87, a single-player mission. Predicted from QuitMenu.cpp before the run:
    ///
    /// - Escape: `Menus/QuitMenu.wnd` made; the requests `[SetGamePaused(true)]`; visible (both flags); after the
    ///   transitions `QuitFull` the parent is shown; *Restart* reads `T:GUI:RestartMission`, *Exit* `T:GUI:ExitMission`
    ///   (the tests' text is `T:` and the label); the cursor, science purchase, diplomacy and chat recorded as the host's;
    /// - *Options*: the options screen made and shown; Escape presses its *Back* (the layout destroyed: `options_layout`
    ///   `None`), the quit menu stays;
    /// - *Save/Load*: the popup shown, kept as the menu's; Escape presses its *Back* (the popup hidden) and the menu
    ///   forgets it; the quit menu stays;
    /// - *Return*: hidden (`[SetGamePaused(false)]`, `QuitFullBack` reversed: the parent hidden after the transitions,
    ///   the layout kept); Escape again shows the same layout (no new one);
    /// - *Exit*: the quit box; its *No* forgets it and the menu stays; *Exit*, *Yes*: the layout deleted, the requests
    ///   `[ClearGameData, SetGamePaused(false)]`, the client quiet; `quit_menu_visible` stays true (`destroyQuitMenu`
    ///   does not clear it);
    /// - nothing undefined.
    ///
    /// **Result (2026-11-10):** as predicted (first run).
    #[test]
    fn a_missions_quit_menu_pauses_opens_its_screens_and_exits() {
        let (mut wm, mut h) = in_game(game_mode::SINGLE_PLAYER);
        let f = "QuitMenu.wnd";
        escape(&mut wm, &mut h);
        let l = h.quit_menu.full_quit_menu_layout.expect("the layout");
        assert_eq!(wm.layouts[l].filename, b"Menus/QuitMenu.wnd");
        assert_eq!(h.requests, [ShellRequest::SetGamePaused(true)]);
        assert!(h.quit_menu.is_visible && h.quit_menu.quit_menu_visible);
        assert!(!hidden(&wm, last(&wm, f, "QuitMenuParent")), "QuitFull shows the parent");
        assert_eq!(text(&wm, last(&wm, f, "ButtonRestart")), "T:GUI:RestartMission");
        assert_eq!(text(&wm, last(&wm, f, "ButtonExit")), "T:GUI:ExitMission");
        for w in ["HideDiplomacy", "HideInGameChat", "TheControlBar->hidePurchaseScience"] {
            assert!(h.untranslated.iter().any(|f| f == w.as_bytes()), "{w}");
        }
        // #117: the arrow is the client's mouse's
        assert_eq!(h.quit_menu.mouse_set_cursor, [crate::client::mouse::cursor::ARROW]);
        assert!(!h.untranslated.iter().any(|f| f.starts_with(b"TheMouse")));
        assert!(in_game_menus_shown(&wm, &h.quit_menu, h.shell.options_layout));
        // Options, then Escape: its Back
        press(&mut wm, &mut h, f, "ButtonOptions");
        let o = h.shell.options_layout.expect("the options screen");
        assert!(!wm.layouts[o].hidden);
        escape(&mut wm, &mut h);
        assert!(h.shell.options_layout.is_none(), "its Back destroys it");
        assert!(h.quit_menu.is_visible);
        // Save/Load, then Escape: its Back
        press(&mut wm, &mut h, f, "ButtonSaveLoad");
        let p = h.quit_menu.save_load_menu_layout.expect("the popup");
        assert!(!wm.layouts[p].hidden);
        escape(&mut wm, &mut h);
        assert!(wm.layouts[p].hidden && h.quit_menu.save_load_menu_layout.is_none());
        assert!(h.quit_menu.is_visible);
        // Return
        h.requests.clear();
        press(&mut wm, &mut h, f, "ButtonReturn");
        settle(&mut wm, &mut h);
        assert_eq!(h.requests, [ShellRequest::SetGamePaused(false)]);
        assert!(!h.quit_menu.is_visible && !h.quit_menu.quit_menu_visible);
        assert!(hidden(&wm, last(&wm, f, "QuitMenuParent")), "QuitFullBack hides it");
        assert!(!in_game_menus_shown(&wm, &h.quit_menu, h.shell.options_layout));
        // again: the same layout
        escape(&mut wm, &mut h);
        assert_eq!(h.quit_menu.full_quit_menu_layout, Some(l));
        assert!(!hidden(&wm, last(&wm, f, "QuitMenuParent")));
        // Exit, No
        press(&mut wm, &mut h, f, "ButtonExit");
        assert!(h.quit_menu.quit_confirmation_window.is_some());
        press(&mut wm, &mut h, "QuitMessageBox.wnd", "ButtonNo");
        assert!(h.quit_menu.quit_confirmation_window.is_none() && h.quit_menu.is_visible);
        wm.update(&mut h);
        // Exit, Yes
        h.requests.clear();
        press(&mut wm, &mut h, f, "ButtonExit");
        press(&mut wm, &mut h, "QuitMessageBox.wnd", "ButtonYes");
        assert!(wm.layouts[l].deleted && h.quit_menu.full_quit_menu_layout.is_none());
        assert_eq!(h.requests, [ShellRequest::ClearGameData, ShellRequest::SetGamePaused(false)]);
        assert!(h.quit_menu.client_quiet && !h.quit_menu.is_visible);
        assert!(h.quit_menu.quit_menu_visible, "destroyQuitMenu leaves TheInGameUI's flag");
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    }

    /// #87, the other games. Predicted from QuitMenu.cpp before the run:
    ///
    /// - a skirmish: the full layout (a skirmish is not a multiplayer game), *Restart* keeps its own text
    ///   (`T:GUI:ButtonRestart`), `[SetGamePaused(true)]`; *Restart*, *Yes*: the layout deleted and
    ///   `RestartMission { SKIRMISH, difficulty 2, rank points 7, fps 30, the map, no replay, not recording }`;
    /// - a replay: `Menus/QuitNoSave.wnd` (no *Save/Load*), paused; in a cinematic (input disabled) *Options* disabled;
    ///   hidden, then shown again with the input enabled: *Options* **still disabled** (the `else if
    ///   (buttonSaveLoadWin)` quirk);
    /// - a LAN game: the no-save layout, *Restart* reads `T:GUI:Surrender`, no pause request; a dead local player:
    ///   *Restart* disabled; alive: *Restart* gives the surrender box, its *Yes* `[SelfDestruct]`; *Exit*, *Yes*:
    ///   `[SelfDestruct, ClearGameData]` (no unpause in a multiplayer game);
    /// - the game ending, or the disconnect screen up: Escape does nothing;
    /// - `HideQuitMenu` without the menu does nothing; with it, the menu hidden and unpaused.
    ///
    /// **Result (2026-11-10):** as predicted (first run).
    #[test]
    fn skirmish_replay_and_network_quit_menus() {
        // skirmish
        let (mut wm, mut h) = in_game(game_mode::SKIRMISH);
        let f = "QuitMenu.wnd";
        escape(&mut wm, &mut h);
        assert!(h.quit_menu.full_quit_menu_layout.is_some());
        assert_eq!(h.requests, [ShellRequest::SetGamePaused(true)]);
        assert_eq!(text(&wm, last(&wm, f, "ButtonRestart")), "T:GUI:ButtonRestart");
        h.requests.clear();
        press(&mut wm, &mut h, f, "ButtonRestart");
        press(&mut wm, &mut h, "MessageBox.wnd", "ButtonYes");
        assert_eq!(
            h.requests,
            [ShellRequest::RestartMission { game_mode: game_mode::SKIRMISH, difficulty: 2, rank_points: 7, fps: 30, map_name: b"Maps\\USA01\\USA01.map".to_vec(), replay_file: Vec::new(), stop_recording: false }]
        );
        assert!(h.quit_menu.full_quit_menu_layout.is_none() && h.quit_menu.client_quiet);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        // replay, in a cinematic
        let (mut wm, mut h) = in_game(game_mode::REPLAY);
        let f = "QuitNoSave.wnd";
        h.quit_menu.game.input_enabled = false;
        escape(&mut wm, &mut h);
        assert!(h.quit_menu.no_save_load_quit_menu_layout.is_some() && h.quit_menu.full_quit_menu_layout.is_none());
        assert!(h.quit_menu.button_save_load_win.is_none() && h.quit_menu.button_save_load == 0);
        assert_eq!(h.requests, [ShellRequest::SetGamePaused(true)]);
        let options = last(&wm, f, "ButtonOptions");
        assert!(!wm.is_enabled(Some(options)));
        escape(&mut wm, &mut h);
        assert!(!h.quit_menu.is_visible);
        h.quit_menu.game.input_enabled = true;
        escape(&mut wm, &mut h);
        assert!(h.quit_menu.is_visible && !wm.is_enabled(Some(options)), "Options stays disabled");
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        // LAN
        let (mut wm, mut h) = in_game(game_mode::LAN);
        h.quit_menu.game.local_player_active = false;
        escape(&mut wm, &mut h);
        assert!(h.requests.is_empty(), "{:?}", h.requests);
        let restart = last(&wm, f, "ButtonRestart");
        assert_eq!(text(&wm, restart), "T:GUI:Surrender");
        assert!(!wm.is_enabled(Some(restart)));
        escape(&mut wm, &mut h);
        h.quit_menu.game.local_player_active = true;
        escape(&mut wm, &mut h);
        assert!(wm.is_enabled(Some(restart)));
        click(&mut wm, &mut h, restart);
        press(&mut wm, &mut h, "MessageBox.wnd", "ButtonYes");
        assert_eq!(h.requests, [ShellRequest::SelfDestruct]);
        h.requests.clear();
        escape(&mut wm, &mut h);
        press(&mut wm, &mut h, f, "ButtonExit");
        press(&mut wm, &mut h, "QuitMessageBox.wnd", "ButtonYes");
        assert_eq!(h.requests, [ShellRequest::SelfDestruct, ShellRequest::ClearGameData]);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        // blocked
        let (mut wm, mut h) = in_game(game_mode::SINGLE_PLAYER);
        h.quit_menu.game.game_ending = true;
        escape(&mut wm, &mut h);
        h.quit_menu.game.game_ending = false;
        h.quit_menu.game.disconnect_screen_visible = Some(true);
        escape(&mut wm, &mut h);
        assert!(!h.quit_menu.is_visible && h.quit_menu.full_quit_menu_layout.is_none() && h.requests.is_empty());
        h.quit_menu.game.disconnect_screen_visible = None;
        // HideQuitMenu
        hide_quit_menu(&mut wm, &mut h);
        assert!(h.requests.is_empty());
        escape(&mut wm, &mut h);
        h.requests.clear();
        hide_quit_menu(&mut wm, &mut h);
        settle(&mut wm, &mut h);
        assert_eq!(h.requests, [ShellRequest::SetGamePaused(false)]);
        assert!(!h.quit_menu.quit_menu_visible && hidden(&wm, last(&wm, "QuitMenu.wnd", "QuitMenuParent")));
        let _ = key::ESC;
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    }

    /// #108: `closeWindows` and `clearGameData`'s reset of InGameUI's flags. Predicted from GameLogicDispatch.cpp and
    /// InGameUI.cpp before the run:
    /// - the menu shown, then the game cleared: hidden, `[SetGamePaused(false)]`, both flags cleared, the six windows
    ///   outside the shell recorded;
    /// - the options screen up (opened from the menu), then the game cleared: its *Back* pressed (the layout destroyed);
    /// - nothing shown: nothing requested, the flags cleared (after *Exit*, `quit_menu_visible` was left true);
    /// - nothing undefined.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn the_games_clear_closes_the_windows() {
        let (mut wm, mut h) = in_game(game_mode::SINGLE_PLAYER);
        let f = "QuitMenu.wnd";
        escape(&mut wm, &mut h);
        h.requests.clear();
        clear_game_data_windows(&mut wm, &mut h);
        settle(&mut wm, &mut h);
        assert_eq!(h.requests, [ShellRequest::SetGamePaused(false)]);
        assert!(!h.quit_menu.is_visible && !h.quit_menu.quit_menu_visible && !h.quit_menu.client_quiet);
        assert!(hidden(&wm, last(&wm, f, "QuitMenuParent")));
        for w in ["HideDiplomacy", "ResetDiplomacy", "HideInGameChat", "ResetInGameChat", "TheControlBar->hidePurchaseScience", "TheControlBar->hideSpecialPowerShortcut"] {
            assert!(h.untranslated.iter().any(|f| f == w.as_bytes()), "{w}");
        }
        // the options screen up
        escape(&mut wm, &mut h);
        press(&mut wm, &mut h, f, "ButtonOptions");
        assert!(h.shell.options_layout.is_some());
        h.requests.clear();
        close_windows(&mut wm, &mut h);
        assert!(h.shell.options_layout.is_none(), "its Back destroys it");
        assert_eq!(h.requests, [ShellRequest::SetGamePaused(false)]);
        // after Exit: the flag destroyQuitMenu leaves, cleared by the reset
        escape(&mut wm, &mut h);
        press(&mut wm, &mut h, f, "ButtonExit");
        press(&mut wm, &mut h, "QuitMessageBox.wnd", "ButtonYes");
        assert!(h.quit_menu.quit_menu_visible && h.quit_menu.client_quiet);
        h.requests.clear();
        clear_game_data_windows(&mut wm, &mut h);
        assert!(h.requests.is_empty(), "{:?}", h.requests);
        assert!(!h.quit_menu.quit_menu_visible && !h.quit_menu.client_quiet);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    }
}
