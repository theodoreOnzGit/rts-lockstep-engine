// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Shell/Shell.cpp (Shell::Shell, ~Shell's pops, reset, update,
//     findScreenByFilename, hide, push, pop, popImmediate, showShell, showShellMap, hideShell, top, linkScreen,
//     unlinkScreen, doPush, doPop, shutdownComplete, isAnimFinished, getSaveLoadMenuLayout, getPopupReplayLayout,
//     getOptionsLayout, destroyOptionsLayout)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Shell.h (MAX_SHELL_STACK, getScreenCount, isShellActive)
//   GeneralsMD/Code/GameEngine/Include/GameClient/WindowLayout.h (runInit, runUpdate, runShutdown with their user data,
//     getFirstWindow, isHidden)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManagerScript.cpp (winCreateLayout; winCreateFromScript
//     without a layout, as gogoMessageBox calls it)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Shell.cpp: Colin Day, 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The shell (GitHub #69, part 1): `TheShell`, the stack of menu screens before (and after) a game.
//!
//! # As upstream reads it
//!
//! A **screen** is a [`WindowLayout`] loaded from a WND file (`winCreateLayout`); the shell keeps at most 16
//! (`MAX_SHELL_STACK`), the top last. Each screen's callbacks: **init** (run when it becomes the top: after its push,
//! or when the screen above it is popped), **update** (every screen's, top first, at most 30 times a second), and
//! **shutdown** (run before it is popped, or before a screen is pushed on it), which is given a `Bool*`: whether the
//! pop is immediate. A shutdown is **required** to call `shutdownComplete(layout)` when it is done, at once or later
//! (from its update, once its animation has ended).
//!
//! - `push(name)`: nothing for an empty name or a full stack; a push becomes **pending**; if the top is shown, its
//!   shutdown runs (with the caller's `shutdownImmediate`), else `shutdownComplete(NULL)` runs at once.
//! - `pop()`: a pop becomes pending and the top's shutdown runs (not immediate); the IME is detached.
//! - `popImmediate()`: the top's shutdown runs (immediate), then the top is popped at once; the pending pop is
//!   cleared, not used.
//! - `shutdownComplete(layout, impendingPush)`: a pending push is done (`doPush`: the layout is loaded, put on the
//!   top, the IME detached, its init run, its windows brought forward), else a pending pop (`doPop`: the top is taken
//!   off, its windows destroyed, and the new top's init runs unless a push is impending). The background is cleared
//!   if `hideShell` asked.
//! - `showShell(runInit)`: unless the game was started on a file, the top's init (if asked); with no shell map and an
//!   empty stack, `Menus/MainMenu.wnd` is pushed; the shell is active.
//! - `hideShell()`: the top's shutdown runs (immediate) but the top stays; the IME is detached; the shell is not
//!   active.
//! - `showShellMap(use)`: with the shell map, a shell game is started (`MSG_NEW_GAME` with `GAME_SHELL`); without,
//!   the background `Menus/BlankWindow.wnd` is loaded (once) and shown, under the top.
//!
//! # Upstream quirks that are kept
//!
//! - `update`'s clock is a function `static` set at its **first** call: the first call never runs the updates; then they
//!   run when `now - lastUpdate >= 1000 / 30 - 1` (32.33 ms: every 33 ms on a millisecond clock).
//! - `pop` and `popImmediate` on an empty stack do nothing; `doPop` with nothing on the stack (a pending pop after
//!   `popImmediate` emptied it) is upstream's `DEBUG_ASSERTCRASH` then a `NULL` call: recorded as undefined.
//! - `doPush` of a file that does not load (`winCreateLayout` returns `NULL`): `linkScreen(NULL)` does nothing, then
//!   `newScreen->runInit` calls through `NULL`: recorded as undefined, the push not done.
//! - `push` does **not** detach the IME (its call is commented out); `doPush`, `doPop`, `pop`, `popImmediate`, `hide`,
//!   `hideShell` and `reset` do.
//! - `findScreenByFilename` compares without case, over the whole stack array (empty slots skipped).
//!
//! # Where this differs from upstream
//!
//! - The window layouts are the manager's ([`GameWindowManager::layouts`], by index); a layout upstream deletes
//!   (`deleteInstance`) is marked [`WindowLayout::deleted`] and stays in the vector, so indices stay stable.
//! - The layout callbacks are the game's own (the main menu's, ...), named in the file and run by the
//!   [`ShellHost`] ([`ShellHost::layout_callback`]), with the user data upstream passes (`None` for `NULL`, the
//!   shutdown's `Bool`).
//! - **Gaps (#69):** ~~the animate window manager (`AnimateWindowManager`: `registerWithAnimateManager`,
//!   `reverseAnimatewindow`, its `update` and `reset`) and the window transitions (`TheTransitionHandler`) are not
//!   translated: `Shell::is_anim_finished` is always true, as upstream's is with nothing registered and every
//!   transition group done.~~ **Corrected 2026-11-10 (#77):** both are translated
//!   ([`crate::client::animate_window`], the shell's [`Shell::animate`]; [`crate::client::window_transitions`], the
//!   window manager's): [`is_anim_finished`] asks the transitions, then (with [`ShellEnv::animate_windows`]) the
//!   animate manager, which `update` runs after the screens' updates and `shutdownComplete` and `reset` reset;
//!   [`register_with_animate_manager`], [`reverse_animate_window`] and [`is_anim_reversed`] are upstream's. The
//!   shell menu schemes (`ShellMenuScheme.ini`, `loadScheme`, the scheme manager's draws)
//!   are not translated. The shell **map** (a game of `GAME_SHELL` behind the menus) is not: the host's
//!   [`ShellEnv::shell_map_on`] is false in this client, so `showShellMap(TRUE)` takes its background branch.
//!   `GameSpyCloseAllOverlays` (online) is not called.
//!
//! # Representation
//!
//! [`Shell`] holds the stack (layout indices, bottom first), the pending operations, the background and the three
//! layouts the shell keeps hidden rather than destroys. Its operations are free functions over a [`ShellHost`] (the
//! host owns the shell, so that a callback the shell runs can call the shell again, as upstream's do through the
//! global `TheShell`).

use crate::client::animate_window::{AnimType, AnimateWindowManager};
use crate::client::window_manager::{GameWindowManager, GuiHost, WindowLayout};
use crate::client::window_script::{window_script_path, win_status, WinId, WindowScript, WndHost};

/// `MAX_SHELL_STACK`.
pub const MAX_SHELL_STACK: usize = 16;
/// `shellUpdateDelay`: updates a second.
pub const SHELL_UPDATE_DELAY: i32 = 30;

/// The global data and engine state the shell reads (`TheGlobalData`, `TheGameLogic`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellEnv {
    /// `m_initialFile` (a map given on the command line: no shell).
    pub initial_file: Vec<u8>,
    /// `m_shellMapOn`.
    pub shell_map_on: bool,
    /// `TheGameLogic->isInGame()`.
    pub in_game: bool,
    /// `TheGameLogic->isInShellGame()` (`getGameMode() == GAME_SHELL` and in game).
    pub in_shell_game: bool,
    /// `m_windowed`.
    pub windowed: bool,
    /// (#77) `m_animateWindows` (`TRUE` by default: GlobalData.cpp).
    pub animate_windows: bool,
}

impl Default for ShellEnv {
    /// The global data's defaults: no initial file, no shell map, not in a game, full screen, windows animated.
    fn default() -> Self {
        ShellEnv { initial_file: Vec::new(), shell_map_on: false, in_game: false, in_shell_game: false, windowed: false, animate_windows: true }
    }
}

/// What a callback asks of the engine and the logic (upstream's `TheMessageStream` messages and engine calls). The
/// client acts on them: they never reach the simulation except as the game's own start messages.
#[derive(Clone, Debug, PartialEq)]
pub enum ShellRequest {
    /// `MSG_CLEAR_GAME_DATA`.
    ClearGameData,
    /// `MSG_NEW_GAME` with `GAME_SHELL` (the shell map, `m_pendingFile` the shell map's name, `InitGameLogicRandom(0)`).
    NewShellGame,
    /// The main menu's campaign start: `TheCampaignManager->setCampaign(campaign)`, `prepareCampaignGame(difficulty)`,
    /// `setupGameStart(getCurrentMap(), difficulty)` and `doGameStart()` (`MSG_NEW_GAME` with `GAME_SINGLE_PLAYER`,
    /// the difficulty and the rank points, `InitRandom(0)`): `World::start_campaign`'s path.
    StartCampaign { campaign: Vec<u8>, difficulty: i32 },
    /// `TheGameEngine->setQuitting(TRUE)`.
    Quit,
    /// (#78) The save/load screen's load (`doLoadGame`): in the shell, `prepareNewGame(GAME_SINGLE_PLAYER,
    /// DIFFICULTY_NORMAL, 0)`; then `TheGameState->loadGame(info)` of the save `filename` (its name in the save
    /// directory). A failed load is the host's to answer as `doLoadGame` does (`clearGameData`, `reset`,
    /// `showShell(TRUE)`).
    LoadGame { filename: Vec<u8> },
    /// (#78) `TheGameState->saveGame(filename, description, fileType)` (the save/load screen; an empty `filename`: the
    /// next free name, `findNextSaveFilename`).
    SaveGame { filename: Vec<u8>, description: Vec<u16>, file_type: i32 },
    /// (#78) `DeleteFile(getFilePathInSaveDirectory(filename))` (the save/load screen's delete).
    DeleteSave { filename: Vec<u8> },
    /// (#89) The skirmish screen's `reallyDoStart`: `TheWritableGlobalData->m_mapName = game.map_name`,
    /// `TheSkirmishGameInfo` the `game` given (after its `startGame(0)`), `InitGameLogicRandom(game.seed)`, and
    /// `MSG_NEW_GAME` with `mode` (`GAME_SKIRMISH`, or `GAME_SINGLE_PLAYER` on a solo map), `difficulty`
    /// (`DIFFICULTY_NORMAL`, "not really used"), rank 0 and the FPS limit `max_fps`: the World's
    /// `start_skirmish_game`.
    StartSkirmish { game: Box<crate::common::game_info::GameInfo>, mode: i32, difficulty: i32, max_fps: i32 },
    /// (#89 part 2) The score screen's *Continue*: `startNextCampaignGame`'s logic half (the pending file the
    /// campaign's current map, a challenge's `TheChallengeGameInfo`, `MSG_NEW_GAME`, `InitRandom(0)`: the World's
    /// `start_next_campaign_game`), with the screen's `TheCampaignManager` (`campaign`: its rank points, difficulty and
    /// next mission set by the screen), in the World that played.
    StartNextCampaignGame { campaign: Box<crate::client::campaign_manager::CampaignManager> },
    /// (#89 part 2) `TheGameState->missionSave()` (the score screen's `finishSinglePlayerInit` after a victory with a
    /// mission to follow), with the screen's `TheCampaignManager`.
    MissionSave { campaign: Box<crate::client::campaign_manager::CampaignManager> },
    /// (#88) The options screen's `TheAudio->setVolume(volume, affect)` (`AudioAffect` bits): the client's audio.
    SetVolume { volume: f32, affect: u32 },
    /// (#98) `TheRecorder->playbackFile(filename)` (the replay menu's): the replay folder's file `filename` played
    /// back through the logic's recorder (`World::play_replay`), with its bytes as the host's folder holds them.
    PlaybackReplay { filename: Vec<u8> },
    /// (#98) A file of the replay folder written (`CopyFile` to `path`, the replay popup's save): its path (the replay
    /// folder's, `Replays\<name>.rep`) and bytes, for the owner's folder.
    WriteReplay { path: Vec<u8>, bytes: Vec<u8> },
    /// (#98) `DeleteFile(path)` of a file of the replay folder (the replay popup's overwrite, the replay menu's
    /// delete).
    DeleteReplay { path: Vec<u8> },
    /// (#98) `CopyFile` of a replay to the desktop folder (`copyReplay`): the file's name and bytes, for the owner to
    /// put where the player keeps files (natively the desktop folder, in the browser a download).
    ExportReplay { name: Vec<u8>, bytes: Vec<u8> },
    /// (#98) The challenge menu's *Play*: `setGeneralCampaign(general)`, the pending file and `TheChallengeGameInfo`'s
    /// map, `clearGameData`, the challenge's `difficulty`, `MSG_NEW_GAME`, `InitRandom(0)`: the World's
    /// `start_challenge`.
    StartChallenge { general: i32, difficulty: i32 },
    /// (#87) The quit menu's `MSG_SELF_DESTRUCT` with `TRUE` (a multiplayer game's exit or surrender).
    SelfDestruct,
    /// (#87) `TheGameLogic->setGamePaused(paused)` (the quit menu's): while paused, `GameEngine::update` does not run
    /// the logic's frames (outside a network game).
    SetGamePaused(bool),
    /// (#87) The quit menu's `restartMissionMenu`, its engine half: the recording stopped (`stop_recording`),
    /// `clearGameData(FALSE)`, `setQuitting(FALSE)`; then `TheRecorder->playbackFile(replay_file)` when it is not empty,
    /// else `m_pendingFile = map_name` and `MSG_NEW_GAME` (`game_mode`, `difficulty`, `rank_points`, `fps`) with
    /// `InitRandom(0)`. (#110) The World's half is `World::restart_mission` (`MissionChoice::Restart`).
    RestartMission { game_mode: i32, difficulty: i32, rank_points: i32, fps: i32, map_name: Vec<u8>, replay_file: Vec<u8>, stop_recording: bool },
}

/// `TheShell`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shell {
    /// `m_screenStack[0 .. m_screenCount]`: layout indices, bottom first.
    pub screen_stack: Vec<usize>,
    /// `m_background`.
    pub background: Option<usize>,
    pub clear_background: bool,
    pub pending_push: bool,
    pub pending_pop: bool,
    pub pending_push_name: Vec<u8>,
    pub is_shell_active: bool,
    /// `m_shellMapOn`.
    pub shell_map_on: bool,
    pub save_load_menu_layout: Option<usize>,
    pub popup_replay_layout: Option<usize>,
    pub options_layout: Option<usize>,
    /// `update`'s `static Int lastUpdate` (set at the first call).
    pub last_update: Option<u32>,
    /// (#77) `m_animateWindowManager` ([`crate::client::animate_window`]).
    pub animate: AnimateWindowManager,
}

impl Shell {
    /// `Shell::Shell()`: an empty stack, the shell active.
    pub fn new() -> Self {
        Shell { is_shell_active: true, ..Default::default() }
    }

    /// `top()`.
    pub fn top(&self) -> Option<usize> {
        self.screen_stack.last().copied()
    }

    /// `getScreenCount()`.
    pub fn get_screen_count(&self) -> usize {
        self.screen_stack.len()
    }

    /// `isShellActive()`.
    pub fn is_shell_active(&self) -> bool {
        self.is_shell_active
    }

    /// `Shell::Shell()` with the animate window manager made on a display `display_width` wide (its spiral's radius).
    pub fn with_display_width(display_width: i32) -> Self {
        Shell { animate: AnimateWindowManager::new(display_width), ..Shell::new() }
    }

    /// `linkScreen(screen)`: on the top, unless the stack is full.
    fn link_screen(&mut self, screen: usize) {
        if self.screen_stack.len() == MAX_SHELL_STACK {
            return;
        }
        self.screen_stack.push(screen);
    }

    /// `unlinkScreen(screen)`: only the top is taken off.
    fn unlink_screen(&mut self, screen: usize) {
        if self.top() == Some(screen) {
            self.screen_stack.pop();
        }
    }
}

/// What the shell needs of the game: itself (the host owns it), the WND files, the layouts' callbacks, the clock and
/// the global data, and somewhere to send its requests.
pub trait ShellHost: GuiHost {
    /// `TheShell`.
    fn shell(&mut self) -> &mut Shell;
    /// `TheWindowManager->winCreateLayout(filename)`: the layout's index, or `None` when the file does not load (see
    /// [`LayoutFiles::win_create_layout`]).
    fn create_layout(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize>;
    /// `TheWindowManager->winCreateFromScript(filename)` with no layout info: the file's first window.
    fn create_from_script(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<WinId>;
    /// A layout's callback `func` (`m_init(layout, userData)`, ...): `user` is the shutdown's `Bool*` (`None`: `NULL`).
    fn layout_callback(&mut self, wm: &mut GameWindowManager, func: &[u8], layout: usize, user: Option<bool>);
    /// `timeGetTime()`, milliseconds.
    fn time_ms(&self) -> u32;
    /// The global data and the logic's state.
    fn env(&self) -> ShellEnv;
    /// A message to the logic or a call on the engine.
    fn request(&mut self, r: ShellRequest);
    /// `TheNameKeyGenerator->nameToKey(name)`, the keys the WND reader gave the windows.
    fn name_to_key(&mut self, name: &[u8]) -> u32;
    /// A message box's callback (`GameWinMsgBoxFunc`), by the name its caller gave
    /// ([`crate::client::message_box::MessageBoxData`]).
    fn message_box_callback(&mut self, _wm: &mut GameWindowManager, _func: &[u8]) {}
    /// (#157) An extended message box's callback `func(userData)` (`MessageBoxFunc`): whether the box closes.
    fn ex_message_box_callback(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _user_data: &crate::client::extended_message_box::ExUserData) -> crate::client::extended_message_box::MessageBoxReturn {
        crate::client::extended_message_box::MessageBoxReturn::Close
    }
}

/// The WND files and what the reader needs, for a [`ShellHost`]'s [`ShellHost::create_layout`] and
/// [`ShellHost::create_from_script`] (new: upstream's reader opens the file itself).
pub trait WndFiles: WndHost {
    /// The bytes of `path` (`Window\...`), if the file is there.
    fn open_wnd(&mut self, path: &[u8]) -> Option<Vec<u8>>;
    /// (#78) `TheGameText->fetch(label, &exists)`: the text and whether the game's text has the label (by default
    /// [`WndHost::fetch_text`], found).
    fn fetch_text_exists(&self, label: &[u8]) -> (Vec<u16>, bool) {
        (self.fetch_text(label), true)
    }
    /// (#98) `image->getImageWidth()`, `getImageHeight()` of the mapped image `image` (by default unknown).
    fn image_size(&self, _image: &[u8]) -> Option<(i32, i32)> {
        None
    }
    /// (#98) `TheMappedImageCollection->findImageByName(image) != NULL` (by default [`WndHost::find_image`]'s answer).
    fn has_image(&self, image: &[u8]) -> bool {
        self.find_image(image).is_some()
    }
}

/// The reader's state with its files: what a host keeps to load layouts. A host takes it out of itself while it loads
/// (the reader runs the host's callbacks: `GWM_CREATE`, ...), and puts it back.
#[derive(Clone, Debug, Default)]
pub struct LayoutFiles<W> {
    pub script: WindowScript,
    pub files: W,
}

impl<W: WndFiles> LayoutFiles<W> {
    /// Files read with a new reader.
    pub fn new(files: W) -> Self {
        LayoutFiles { script: WindowScript::new(), files }
    }

    /// `winCreateLayout(filename)`: a new layout loaded from `Window\<filename>` (`WindowLayout::load`); `None` (the
    /// layout deleted) when the file is missing, does not read, or makes no window.
    pub fn win_create_layout<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H, filename: &[u8]) -> Option<usize> {
        win_create_layout_with(&mut self.script, &mut self.files, wm, h, filename)
    }

    /// `winCreateFromScript(filename)` with no layout info (`gogoMessageBox`): the windows are made but belong to no
    /// layout. The first window.
    pub fn win_create_from_script<H: GuiHost>(&mut self, wm: &mut GameWindowManager, h: &mut H, filename: &[u8]) -> Option<WinId> {
        win_create_from_script_with(&mut self.script, &mut self.files, wm, h, filename)
    }
}

/// [`LayoutFiles::win_create_layout`] with the reader and the files apart (#78: a host lends its files through
/// [`KeyedFiles`]).
pub fn win_create_layout_with<F: WndFiles, H: GuiHost>(script: &mut WindowScript, files: &mut F, wm: &mut GameWindowManager, h: &mut H, filename: &[u8]) -> Option<usize> {
    if filename.is_empty() {
        return None;
    }
    let data = files.open_wnd(&window_script_path(filename))?;
    match wm.win_create_from_script(script, filename, &data, files, h) {
        Ok((Some(l), _)) => Some(l),
        _ => None,
    }
}

/// [`LayoutFiles::win_create_from_script`] with the reader and the files apart.
pub fn win_create_from_script_with<F: WndFiles, H: GuiHost>(script: &mut WindowScript, files: &mut F, wm: &mut GameWindowManager, h: &mut H, filename: &[u8]) -> Option<WinId> {
    let data = files.open_wnd(&window_script_path(filename))?;
    let (l, r) = wm.win_create_from_script(script, filename, &data, files, h).ok()?;
    if let Some(l) = l {
        // (the manager made a layout of the file's windows; upstream's call makes none)
        for w in wm.layouts[l].windows.clone() {
            wm.layout_remove_window(l, w);
        }
        wm.layouts[l].deleted = true;
    }
    r.first_window
}

/// (#78) `TheNameKeyGenerator` as one table that a host and the reader it lends its files to share: a callback the
/// reader runs while the files are lent (a window's `GWM_CREATE`: `SinglePlayerMenuSystem` takes its buttons' keys
/// there) gets the keys the reader gives the windows, as upstream's one global generator does. The keys are numbered
/// from 1 in the order the names are first asked for.
#[derive(Clone, Debug, Default)]
pub struct NameKeys(std::rc::Rc<std::cell::RefCell<std::collections::BTreeMap<Vec<u8>, u32>>>);

impl NameKeys {
    /// `nameToKey(name)`.
    pub fn name_to_key(&self, name: &[u8]) -> u32 {
        let mut m = self.0.borrow_mut();
        let n = m.len() as u32 + 1;
        *m.entry(name.to_vec()).or_insert(n)
    }
}

/// (#78) Files lent to the reader with the shared [`NameKeys`]: every [`WndHost`] call but `name_to_key` is the
/// files' own.
pub struct KeyedFiles<'a, W> {
    pub files: &'a mut W,
    pub keys: NameKeys,
}

impl<W: WndFiles> WndHost for KeyedFiles<'_, W> {
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        self.keys.name_to_key(name)
    }
    fn find_image(&self, name: &[u8]) -> Option<Vec<u8>> {
        self.files.find_image(name)
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        self.files.fetch_text(label)
    }
    fn get_font(&self, name: &[u8], size: i32, bold: bool) -> Option<crate::client::window_script::GameFont> {
        self.files.get_font(name, size, bold)
    }
    fn font_from_template(&self, name: &[u8]) -> Option<crate::client::window_script::GameFont> {
        self.files.font_from_template(name)
    }
    fn lexicon_has(&self, kind: crate::client::window_script::Callback, name: &[u8]) -> bool {
        self.files.lexicon_has(kind, name)
    }
    fn display_size(&self) -> (i32, i32) {
        self.files.display_size()
    }
}

impl<W: WndFiles> WndFiles for KeyedFiles<'_, W> {
    fn open_wnd(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.files.open_wnd(path)
    }
    fn fetch_text_exists(&self, label: &[u8]) -> (Vec<u16>, bool) {
        self.files.fetch_text_exists(label)
    }
}

/// `runInit(user)`, `runUpdate(user)`, `runShutdown(user)` (`which` 0, 1, 2): the layout's callback, if it has one.
pub fn run_layout<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, l: usize, which: usize, user: Option<bool>) {
    let f = match which {
        0 => wm.layouts[l].init.clone(),
        1 => wm.layouts[l].update.clone(),
        _ => wm.layouts[l].shutdown.clone(),
    };
    if let Some(f) = f {
        h.layout_callback(wm, &f, l, user);
    }
}

/// `layout->deleteInstance()` after `destroyWindows()`.
pub(crate) fn destroy_layout(wm: &mut GameWindowManager, l: usize) {
    wm.layout_destroy_windows(l);
    wm.layouts[l].deleted = true;
}

/// `WindowLayout::isHidden`.
fn layout_hidden(wm: &GameWindowManager, l: usize) -> bool {
    wm.layouts[l].hidden
}

/// `Shell::reset()`: the IME detached, every screen popped (`pop`, so each waits for its shutdown).
pub fn reset<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) {
    wm.ime.detatch();
    // `while (m_screenCount) pop();`: a shutdown that does not complete at once would loop for ever upstream
    let mut guard = 0;
    while h.shell().get_screen_count() > 0 {
        let before = h.shell().get_screen_count();
        pop(wm, h);
        if h.shell().get_screen_count() == before {
            guard += 1;
            if guard > 1 {
                wm.set_undefined("Shell::reset: a screen's shutdown did not complete (upstream loops for ever)");
                return;
            }
        }
    }
    // `m_animateWindowManager->reset()` (#77)
    reset_animate(wm, h);
}

/// `Shell::update()` at the host's clock: every screen's update, top first, at most 30 times a second; the
/// background goes once the shell map is on; (#77) then the animate window manager's update.
pub fn update<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) {
    let now = h.time_ms();
    let last = *h.shell().last_update.get_or_insert(now);
    let elapsed = now.wrapping_sub(last) as i32;
    // `now - lastUpdate >= ((1000.0f / shellUpdateDelay) - 1)`
    if elapsed as f32 >= (1000.0f32 / SHELL_UPDATE_DELAY as f32) - 1.0 {
        let stack = h.shell().screen_stack.clone();
        for &l in stack.iter().rev() {
            run_layout(wm, h, l, 1, None);
        }
        let env = h.env();
        let s = h.shell();
        if env.shell_map_on && s.shell_map_on {
            if let Some(b) = s.background.take() {
                destroy_layout(wm, b);
            }
        }
        // `m_animateWindowManager->update()` (#77); (the scheme manager's update: not translated)
        let mut a = std::mem::take(&mut h.shell().animate);
        a.update(wm, now);
        h.shell().animate = a;
        h.shell().last_update = Some(now);
    }
}

/// `findScreenByFilename(filename)`: the screen of the stack loaded from it (case ignored).
pub fn find_screen_by_filename(wm: &GameWindowManager, shell: &Shell, filename: &[u8]) -> Option<usize> {
    if filename.is_empty() {
        return None;
    }
    shell.screen_stack.iter().copied().find(|&l| wm.layouts[l].filename.eq_ignore_ascii_case(filename))
}

/// `Shell::hide(hide)`: every screen hidden or shown; the IME detached.
pub fn hide<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, hide: bool) {
    for l in h.shell().screen_stack.clone() {
        wm.layout_hide(l, hide);
    }
    wm.ime.detatch();
}

/// `Shell::push(filename, shutdownImmediate)`.
pub fn push<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, filename: &[u8], shutdown_immediate: bool) {
    if filename.is_empty() {
        return;
    }
    // (`GameSpyCloseAllOverlays`: online, not translated)
    let s = h.shell();
    if s.get_screen_count() >= MAX_SHELL_STACK {
        return;
    }
    s.pending_push = true;
    s.pending_push_name = filename.to_vec();
    let current_top = s.top();
    match current_top {
        Some(t) if !layout_hidden(wm, t) => run_layout(wm, h, t, 2, Some(shutdown_immediate)),
        _ => shutdown_complete(wm, h, None, false),
    }
}

/// `Shell::pop()`: the top's shutdown runs; the pop is done when it completes.
pub fn pop<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(screen) = h.shell().top() else { return };
    h.shell().pending_pop = true;
    run_layout(wm, h, screen, 2, Some(false));
    wm.ime.detatch();
}

/// `Shell::popImmediate()`: the top's shutdown runs (told the pop is immediate), then the top is popped.
pub fn pop_immediate<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(screen) = h.shell().top() else { return };
    h.shell().pending_pop = false;
    run_layout(wm, h, screen, 2, Some(true));
    do_pop(wm, h, false);
    wm.ime.detatch();
}

/// `Shell::showShell(runInit)`.
pub fn show_shell<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, run_init: bool) {
    let env = h.env();
    if !env.initial_file.is_empty() {
        return;
    }
    if run_init {
        if let Some(l) = h.shell().top() {
            run_layout(wm, h, l, 0, None);
        }
    }
    if !env.shell_map_on && h.shell().get_screen_count() == 0 {
        push(wm, h, b"Menus/MainMenu.wnd", false);
    }
    h.shell().is_shell_active = true;
}

/// `Shell::showShellMap(useShellMap)`.
pub fn show_shell_map<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, use_shell_map: bool) {
    let env = h.env();
    // (`|| !TheGameLogic`: the logic always exists here)
    if !env.initial_file.is_empty() {
        return;
    }
    if use_shell_map && env.shell_map_on {
        if env.in_game && env.in_shell_game {
            return;
        }
        if env.in_game {
            h.request(ShellRequest::ClearGameData);
        }
        h.request(ShellRequest::NewShellGame);
        h.shell().shell_map_on = true;
    } else {
        if env.in_game && env.in_shell_game {
            h.request(ShellRequest::ClearGameData);
        }
        if !h.shell().is_shell_active {
            return;
        }
        if h.shell().background.is_none() {
            let b = h.create_layout(wm, b"Menus/BlankWindow.wnd");
            h.shell().background = b;
        }
        let Some(b) = h.shell().background else {
            wm.set_undefined("Shell::showShellMap: Menus/BlankWindow.wnd did not load (m_background->getFirstWindow() on NULL)");
            return;
        };
        let Some(&first) = wm.layouts[b].windows.first() else {
            wm.set_undefined("Shell::showShellMap: the background layout has no window (winSetStatus on NULL)");
            return;
        };
        wm.win_set_status(first, win_status::IMAGE);
        wm.layout_hide(b, false);
        if let Some(t) = h.shell().top() {
            wm.layout_bring_forward(t);
        }
        let s = h.shell();
        s.shell_map_on = false;
        s.clear_background = false;
    }
}

/// `Shell::hideShell()`: the top's shutdown (immediate), but it stays on the stack.
pub fn hide_shell<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.shell().clear_background = true;
    if let Some(l) = h.shell().top() {
        run_layout(wm, h, l, 2, Some(true));
    }
    wm.ime.detatch();
    h.shell().is_shell_active = false;
}

/// `doPush(layoutFile)`.
fn do_push<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, layout_file: &[u8]) {
    let Some(new_screen) = h.create_layout(wm, layout_file) else {
        // `linkScreen(NULL)` returns; `newScreen->runInit(NULL)` calls through NULL
        wm.set_undefined("Shell::doPush: the layout did not load (runInit on NULL)");
        return;
    };
    h.shell().link_screen(new_screen);
    wm.ime.detatch();
    run_layout(wm, h, new_screen, 0, None);
    wm.layout_bring_forward(new_screen);
}

/// `doPop(impendingPush)`.
fn do_pop<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, impending_push: bool) {
    let Some(current_top) = h.shell().top() else {
        wm.set_undefined("Shell::doPop: no top of the stack (unlinkScreen reads m_screenStack[-1])");
        return;
    };
    h.shell().unlink_screen(current_top);
    destroy_layout(wm, current_top);
    if let Some(new_top) = h.shell().top() {
        if !impending_push {
            run_layout(wm, h, new_top, 0, None);
        }
    }
    wm.ime.detatch();
}

/// `shutdownComplete(layout, impendingPush)`: a layout's shutdown is done; the pending push or pop is done now.
pub fn shutdown_complete<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, _layout: Option<usize>, impending_push: bool) {
    // `m_animateWindowManager->reset()` (#77)
    reset_animate(wm, h);
    let s = h.shell();
    if s.pending_push {
        let name = s.pending_push_name.clone();
        do_push(wm, h, &name);
        let s = h.shell();
        s.pending_push = false;
        s.pending_push_name.clear();
    } else if s.pending_pop {
        do_pop(wm, h, impending_push);
        h.shell().pending_pop = false;
    }
    if h.shell().clear_background {
        if let Some(b) = h.shell().background.take() {
            destroy_layout(wm, b);
            h.shell().clear_background = false;
        }
    }
}

/// `getSaveLoadMenuLayout()`: `Menus/PopupSaveLoad.wnd`, made once.
pub fn get_save_load_menu_layout<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) -> Option<usize> {
    if h.shell().save_load_menu_layout.is_none() {
        let l = h.create_layout(wm, b"Menus/PopupSaveLoad.wnd");
        h.shell().save_load_menu_layout = l;
    }
    h.shell().save_load_menu_layout
}

/// `getPopupReplayLayout()`: `Menus/PopupReplay.wnd`, made once.
pub fn get_popup_replay_layout<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) -> Option<usize> {
    if h.shell().popup_replay_layout.is_none() {
        let l = h.create_layout(wm, b"Menus/PopupReplay.wnd");
        h.shell().popup_replay_layout = l;
    }
    h.shell().popup_replay_layout
}

/// `getOptionsLayout(create)`: `Menus/OptionsMenu.wnd`, made when asked.
pub fn get_options_layout<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, create: bool) -> Option<usize> {
    if h.shell().options_layout.is_none() && create {
        let l = h.create_layout(wm, b"Menus/OptionsMenu.wnd");
        h.shell().options_layout = l;
    }
    h.shell().options_layout
}

/// `destroyOptionsLayout()`.
pub fn destroy_options_layout<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) {
    if let Some(l) = h.shell().options_layout.take() {
        destroy_layout(wm, l);
    }
}

/// The layout as the manager keeps it (for a host's checks).
pub fn layout(wm: &GameWindowManager, l: usize) -> &WindowLayout {
    &wm.layouts[l]
}

/// (#77) `m_animateWindowManager->reset()`.
fn reset_animate<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) {
    let mut a = std::mem::take(&mut h.shell().animate);
    a.reset(wm);
    h.shell().animate = a;
}

/// (#77) `registerWithAnimateManager(win, animType, needsToFinish, delayMS)`: with `m_animateWindows`, registered for
/// 500 ms at the host's clock.
pub fn register_with_animate_manager<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H, win: Option<WinId>, anim_type: AnimType, needs_to_finish: bool, delay_ms: u32) {
    if h.env().animate_windows {
        let now = h.time_ms();
        let mut a = std::mem::take(&mut h.shell().animate);
        a.register_game_window(wm, win, anim_type, needs_to_finish, 500, delay_ms, now);
        h.shell().animate = a;
    }
}

/// (#77) `isAnimFinished()`: the transitions' (`TheTransitionHandler->isFinished()`), then, with `m_animateWindows`,
/// the animate manager's.
pub fn is_anim_finished<H: ShellHost>(wm: &GameWindowManager, h: &mut H) -> bool {
    if !crate::client::window_transitions::is_finished(wm) {
        return false;
    }
    if h.env().animate_windows {
        h.shell().animate.is_finished()
    } else {
        true
    }
}

/// (#77) `reverseAnimatewindow()`: with `m_animateWindows`, every animation turned around at the host's clock.
pub fn reverse_animate_window<H: ShellHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.env().animate_windows {
        let now = h.time_ms();
        let mut a = std::mem::take(&mut h.shell().animate);
        a.reverse_animate_window(wm, now);
        h.shell().animate = a;
    }
}

/// (#77) `isAnimReversed()`: the animate manager's, or true without `m_animateWindows`.
pub fn is_anim_reversed<H: ShellHost>(h: &mut H) -> bool {
    !h.env().animate_windows || h.shell().animate.is_reversed()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::window_manager::{MsgHandled, WindowMsgData};
    use crate::client::shell_gui::synthetic::screen;
    use crate::client::window_script::tests::Host as WndTestHost;
    use std::collections::BTreeMap;

    /// Synthetic WND files (no EA file; a screen is [`crate::client::shell_gui::synthetic::screen`]).
    #[derive(Default)]
    pub(crate) struct Files {
        pub wnd: WndTestHost,
        pub files: BTreeMap<Vec<u8>, Vec<u8>>,
    }

    impl WndHost for Files {
        fn name_to_key(&mut self, name: &[u8]) -> u32 {
            self.wnd.name_to_key(name)
        }
        fn find_image(&self, name: &[u8]) -> Option<Vec<u8>> {
            self.wnd.find_image(name)
        }
        fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
            self.wnd.fetch_text(label)
        }
        fn display_size(&self) -> (i32, i32) {
            (800, 600)
        }
    }

    impl WndFiles for Files {
        fn open_wnd(&mut self, path: &[u8]) -> Option<Vec<u8>> {
            self.files.get(path).cloned()
        }
        /// (#78) A label starting `NOTEXT` is not in the game's text.
        fn fetch_text_exists(&self, label: &[u8]) -> (Vec<u16>, bool) {
            (self.fetch_text(label), !label.starts_with(b"NOTEXT"))
        }
        /// (#98) Every `Img...` image is 32x20.
        fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
            image.starts_with(b"Img").then_some((32, 20))
        }
    }

    /// A host whose layout callbacks record `(func, layout, user)`; a shutdown completes at once when `immediate` says
    /// so (or the user data does), else it waits (as the main menu's does, until its update).
    #[derive(Default)]
    struct Rec {
        shell: Shell,
        files: Option<LayoutFiles<Files>>,
        log: Vec<(String, usize, Option<bool>)>,
        complete_at_once: bool,
        time: u32,
        env: ShellEnv,
        requests: Vec<ShellRequest>,
    }

    impl GuiHost for Rec {
        fn system(&mut self, _wm: &mut GameWindowManager, _f: &[u8], _w: WinId, _m: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
            MsgHandled::Ignored
        }
    }

    impl ShellHost for Rec {
        fn shell(&mut self) -> &mut Shell {
            &mut self.shell
        }
        fn create_layout(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
            let mut f = self.files.take()?;
            let r = f.win_create_layout(wm, self, filename);
            self.files = Some(f);
            r
        }
        fn create_from_script(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<WinId> {
            let mut f = self.files.take()?;
            let r = f.win_create_from_script(wm, self, filename);
            self.files = Some(f);
            r
        }
        fn layout_callback(&mut self, wm: &mut GameWindowManager, func: &[u8], layout: usize, user: Option<bool>) {
            let f = String::from_utf8_lossy(func).into_owned();
            self.log.push((f.clone(), layout, user));
            if f.ends_with("Init") {
                wm.layout_hide(layout, false);
            }
            if f.ends_with("Shutdown") && (self.complete_at_once || user == Some(true)) {
                wm.layout_hide(layout, true);
                shutdown_complete(wm, self, Some(layout), false);
            }
        }
        fn time_ms(&self) -> u32 {
            self.time
        }
        fn env(&self) -> ShellEnv {
            self.env.clone()
        }
        fn request(&mut self, r: ShellRequest) {
            self.requests.push(r);
        }
        fn name_to_key(&mut self, name: &[u8]) -> u32 {
            self.files.as_mut().map_or(u32::MAX, |f| f.files.name_to_key(name))
        }
    }

    fn rec() -> Rec {
        let mut files = Files::default();
        for n in ["MainMenu", "SaveLoad", "Credits", "BlankWindow", "PopupSaveLoad", "OptionsMenu", "PopupReplay"] {
            files.files.insert(format!("Window\\Menus/{n}.wnd").into_bytes(), screen(n));
        }
        Rec { shell: Shell::new(), files: Some(LayoutFiles::new(files)), ..Default::default() }
    }

    fn names(wm: &GameWindowManager, s: &Shell) -> Vec<String> {
        s.screen_stack.iter().map(|&l| String::from_utf8_lossy(&wm.layouts[l].filename).into_owned()).collect()
    }

    fn calls(h: &mut Rec) -> Vec<String> {
        std::mem::take(&mut h.log).into_iter().map(|(f, _, u)| format!("{f}{}", u.map_or(String::new(), |b| format!("({b})")))).collect()
    }

    /// #69, the stack, predicted from Shell.cpp before the run:
    ///
    /// - `showShell(TRUE)` on an empty stack, no shell map: no init (no top), then `push("Menus/MainMenu.wnd")`, which
    ///   with nothing on the stack completes at once: the main menu is loaded and its init runs; it is the only screen;
    /// - `push("Menus/SaveLoad.wnd")` with a shutdown that waits: `MainMenuShutdown(false)` runs and nothing else; the
    ///   push is pending. `shutdownComplete` (as the main menu's update calls it) does the push: `SaveLoadInit`; two
    ///   screens; the IME detached;
    /// - `pop()`: `SaveLoadShutdown(false)` and the pop is pending; its `shutdownComplete`: the save/load screen's
    ///   windows destroyed, its layout deleted, `MainMenuInit` again;
    /// - `popImmediate()`: `MainMenuShutdown(true)` (which completes at once: no pending operation, so nothing) and the
    ///   main menu is popped: the stack is empty, and no init runs;
    /// - `findScreenByFilename` finds a screen by its name in any case; a push of an empty name or a 17th screen does
    ///   nothing.
    #[test]
    fn the_shell_stack_pushes_and_pops_as_upstream() {
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = rec();
        show_shell(&mut wm, &mut h, true);
        assert_eq!(names(&wm, &h.shell), ["Menus/MainMenu.wnd"]);
        assert_eq!(calls(&mut h), ["MainMenuInit"]);
        assert!(h.shell.is_shell_active());
        wm.ime.attach(Some(0));
        push(&mut wm, &mut h, b"Menus/SaveLoad.wnd", false);
        assert_eq!(calls(&mut h), ["MainMenuShutdown(false)"]);
        assert!(h.shell.pending_push);
        assert_eq!(h.shell.get_screen_count(), 1);
        assert_eq!(wm.ime.get_window(), Some(0), "push does not detach");
        let mm = h.shell.top();
        shutdown_complete(&mut wm, &mut h, mm, false);
        assert_eq!(names(&wm, &h.shell), ["Menus/MainMenu.wnd", "Menus/SaveLoad.wnd"]);
        assert_eq!(calls(&mut h), ["SaveLoadInit"]);
        assert!(!h.shell.pending_push && h.shell.pending_push_name.is_empty());
        assert_eq!(wm.ime.get_window(), None);
        assert_eq!(find_screen_by_filename(&wm, &h.shell, b"menus/saveload.WND"), h.shell.top());
        assert_eq!(find_screen_by_filename(&wm, &h.shell, b""), None);
        let sl = h.shell.top().expect("top");
        let sl_win = wm.layouts[sl].windows[0];
        pop(&mut wm, &mut h);
        assert_eq!(calls(&mut h), ["SaveLoadShutdown(false)"]);
        assert!(h.shell.pending_pop);
        shutdown_complete(&mut wm, &mut h, Some(sl), false);
        assert_eq!(names(&wm, &h.shell), ["Menus/MainMenu.wnd"]);
        assert_eq!(calls(&mut h), ["MainMenuInit"]);
        assert!(wm.layouts[sl].deleted && wm.layouts[sl].windows.is_empty());
        assert!(wm.tree.windows[sl_win].status & win_status::DESTROYED != 0);
        pop_immediate(&mut wm, &mut h);
        assert_eq!(calls(&mut h), ["MainMenuShutdown(true)"]);
        assert_eq!(h.shell.get_screen_count(), 0);
        pop(&mut wm, &mut h);
        pop_immediate(&mut wm, &mut h);
        assert!(calls(&mut h).is_empty());
        push(&mut wm, &mut h, b"", false);
        assert!(!h.shell.pending_push);
        h.complete_at_once = true;
        for _ in 0..MAX_SHELL_STACK + 1 {
            push(&mut wm, &mut h, b"Menus/Credits.wnd", false);
        }
        assert_eq!(h.shell.get_screen_count(), MAX_SHELL_STACK);
        assert!(wm.undefined.is_none());
        reset(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 0);
    }

    /// #69, the clock, the background and the hidden layouts, predicted from Shell.cpp before the run:
    ///
    /// - `update` at 1000 ms (its first call) runs nothing; at 1032 nothing (32 < 32.33); at 1033 every screen's update,
    ///   top first; at 1040 nothing again;
    /// - `showShellMap(TRUE)` without the shell map loads `Menus/BlankWindow.wnd` once, sets its first window's
    ///   `IMAGE` status, shows it; `hideShell` runs the top's shutdown (immediate), detaches the IME and makes the shell
    ///   inactive; the background goes at the next `shutdownComplete` (the shutdown's own);
    /// - with the shell map on, `showShellMap(TRUE)` asks for a shell game (`MSG_NEW_GAME`), after clearing a game in
    ///   progress; once on, `update` removes a background;
    /// - `getSaveLoadMenuLayout`, `getOptionsLayout(TRUE)` make their layouts once; `getOptionsLayout(FALSE)` makes
    ///   none; `destroyOptionsLayout` deletes it;
    /// - a push of a file that does not load is upstream's undefined behaviour (recorded); so is a pending pop with
    ///   the stack emptied.
    #[test]
    fn the_shell_updates_and_keeps_its_layouts_as_upstream() {
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = rec();
        h.complete_at_once = true;
        push(&mut wm, &mut h, b"Menus/MainMenu.wnd", false);
        push(&mut wm, &mut h, b"Menus/Credits.wnd", false);
        calls(&mut h);
        for (t, ran) in [(1000, false), (1032, false), (1033, true), (1040, false), (1066, true)] {
            h.time = t;
            update(&mut wm, &mut h);
            let c = calls(&mut h);
            if ran {
                assert_eq!(c, ["CreditsUpdate", "MainMenuUpdate"], "at {t}");
            } else {
                assert!(c.is_empty(), "at {t}: {c:?}");
            }
        }
        show_shell_map(&mut wm, &mut h, true);
        let b = h.shell.background.expect("the background");
        let bw = wm.layouts[b].windows[0];
        assert!(wm.tree.windows[bw].status & win_status::IMAGE != 0);
        assert!(!wm.tree.windows[bw].is_hidden());
        show_shell_map(&mut wm, &mut h, true);
        assert_eq!(h.shell.background, Some(b), "made once");
        wm.ime.attach(Some(1));
        hide_shell(&mut wm, &mut h);
        assert_eq!(calls(&mut h), ["CreditsShutdown(true)"]);
        assert!(!h.shell.is_shell_active());
        assert_eq!(wm.ime.get_window(), None);
        assert_eq!(h.shell.background, None, "cleared by the shutdown's shutdownComplete");
        assert!(wm.layouts[b].deleted);
        assert_eq!(h.shell.get_screen_count(), 2, "hideShell keeps the stack");
        // not active: no background
        show_shell_map(&mut wm, &mut h, false);
        assert_eq!(h.shell.background, None);
        show_shell(&mut wm, &mut h, true);
        assert_eq!(calls(&mut h), ["CreditsInit"]);
        assert_eq!(h.shell.get_screen_count(), 2);
        h.env.shell_map_on = true;
        h.env.in_game = true;
        show_shell_map(&mut wm, &mut h, true);
        assert_eq!(h.requests, [ShellRequest::ClearGameData, ShellRequest::NewShellGame]);
        assert!(h.shell.shell_map_on);
        h.shell.background = h.create_layout(&mut wm, b"Menus/BlankWindow.wnd");
        h.time = 2000;
        update(&mut wm, &mut h);
        assert_eq!(h.shell.background, None);
        h.env.initial_file = b"x.map".to_vec();
        show_shell(&mut wm, &mut h, true);
        assert!(calls(&mut h).iter().all(|c| c.ends_with("Update")));
        let sl = get_save_load_menu_layout(&mut wm, &mut h);
        assert!(sl.is_some());
        assert_eq!(get_save_load_menu_layout(&mut wm, &mut h), sl);
        assert!(get_popup_replay_layout(&mut wm, &mut h).is_some());
        assert_eq!(get_options_layout(&mut wm, &mut h, false), None);
        let o = get_options_layout(&mut wm, &mut h, true).expect("options");
        destroy_options_layout(&mut wm, &mut h);
        assert!(wm.layouts[o].deleted && h.shell.options_layout.is_none());
        assert_eq!(layout(&wm, o).filename, b"Menus/OptionsMenu.wnd");
        hide(&mut wm, &mut h, true);
        assert!(h.shell.screen_stack.iter().all(|&l| wm.layouts[l].hidden));
        push(&mut wm, &mut h, b"Menus/Missing.wnd", false);
        assert!(wm.undefined.is_some());
        wm.undefined = None;
        let mut h2 = rec();
        h2.shell.pending_pop = true;
        shutdown_complete(&mut wm, &mut h2, None, false);
        assert!(wm.undefined.is_some());
        // a file with no layout: its windows belong to none
        let first = h.create_from_script(&mut wm, b"Menus/Credits.wnd").expect("made");
        assert_eq!(wm.ext[first].layout, None);
    }

    /// (#77) The animate window manager in the shell, predicted from Shell.cpp: `registerWithAnimateManager` registers
    /// (500 ms) only with `m_animateWindows`; `isAnimFinished` is false while a window must finish (and while a
    /// transition group runs), `isAnimReversed` after `reverseAnimatewindow`; the shell's update runs the manager after
    /// the screens; `reset` puts the window at rest and empties it. Without `m_animateWindows` nothing registers and
    /// the shell is always finished and reversed.
    #[test]
    fn the_animate_manager_follows_the_shell() {
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = rec();
        h.shell = Shell::with_display_width(800);
        h.complete_at_once = true;
        push(&mut wm, &mut h, b"Menus/MainMenu.wnd", false);
        let w = wm.layouts[h.shell.top().expect("top")].windows[0];
        let rest = (wm.tree.windows[w].region.0, wm.tree.windows[w].region.1);
        register_with_animate_manager(&mut wm, &mut h, Some(w), crate::client::animate_window::anim::WIN_ANIMATION_SLIDE_TOP, true, 0);
        assert!(!is_anim_finished(&wm, &mut h) && !is_anim_reversed(&mut h));
        assert_ne!((wm.tree.windows[w].region.0, wm.tree.windows[w].region.1), rest);
        for t in 0..60 {
            h.time = 1000 + 40 * t;
            update(&mut wm, &mut h);
        }
        assert!(is_anim_finished(&wm, &mut h));
        assert_eq!((wm.tree.windows[w].region.0, wm.tree.windows[w].region.1), rest);
        reverse_animate_window(&mut wm, &mut h);
        assert!(is_anim_reversed(&mut h) && !is_anim_finished(&wm, &mut h));
        reset(&mut wm, &mut h);
        assert!(h.shell.animate.is_empty());
        let mut h2 = rec();
        h2.env.animate_windows = false;
        register_with_animate_manager(&mut wm, &mut h2, Some(w), crate::client::animate_window::anim::WIN_ANIMATION_SLIDE_TOP, true, 0);
        assert!(h2.shell.animate.is_empty() && is_anim_finished(&wm, &mut h2) && is_anim_reversed(&mut h2));
    }
}

