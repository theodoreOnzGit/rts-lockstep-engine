// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/MainMenu.cpp (the file's statics,
//     HandleCanceledDownload, showSelectiveButtons, quitCallback, setupGameStart, prepareCampaignGame, doGameStart,
//     checkCDBeforeCampaign, shutdownComplete, initialHide, MainMenuInit, MainMenuShutdown, MainMenuUpdate,
//     MainMenuInput, MainMenuSystem, diffReverseSide) as the release build compiles it (no _DEBUG, _INTERNAL,
//     _PROFILE, TEST_COMPRESSION or _CAMPEA_DEMO)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   MainMenu.cpp: Colin Day, 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The main menu (GitHub #69, part 1): `Menus/MainMenu.wnd`'s callbacks.
//!
//! # As upstream reads it
//!
//! **Init** finds its windows by name (`MainMenu.wnd:MainMenuParent` among the top-level windows, the buttons and the
//! five drop-down panels `MapBorder` .. `MapBorder4` under it: single player, multiplayer, main, load/replay,
//! difficulty), hides the panels and the faction logos and the campaigns' save buttons, shows the layout, and gives the
//! parent the focus. The first time the game runs, the mouse is hidden and the main panel waits: **Input** shows it
//! (`MapBorder2`) at the first key, or once the mouse has moved more than 20 pixels from where it first was.
//!
//! **System** (`GBM_SELECTED`), unless a button press is pending (`buttonPushed`):
//! - *Single Player*, *Multiplayer*, *Load/Replay* show their panel; *Back* buttons show the main panel again.
//! - *USA*, *GLA*, *China* set the campaign (`TheCampaignManager->setCampaign`) and hide that faction's logo; then
//!   *Easy*, *Medium*, *Hard* (`checkCDBeforeCampaign`): with the first disc present, `prepareCampaignGame(diff)`
//!   (`setupGameStart(currentMap, diff)`: `startGame`), and the next **Update** with every animation done sends
//!   `MSG_NEW_GAME` (`doGameStart`) and starts the menu's shutdown. *Diff Back* clears the campaign.
//! - *Challenge* (Generals' Challenge) marks the challenge menu to follow the difficulty (`Menus/ChallengeMenu.wnd`).
//! - *Skirmish* pushes `Menus/SkirmishGameOptionsMenu.wnd`; *Load Game* `Menus/SaveLoad.wnd`; *Replay*
//!   `Menus/ReplayMenu.wnd`; *Credits* `Menus/CreditsMenu.wnd`; *Network* `Menus/LanLobbyMenu.wnd`; *Options* shows
//!   the shell's options layout.
//! - *Exit*: windowed, `quitCallback` at once; full screen, the quit box (`QuitMessageBoxYesNo`) asks first. Its Yes
//!   pops the menu and makes the engine quit.
//!
//! **Shutdown** (`Bool*`: immediate) completes at once when immediate; else **Update** completes it once the
//! animations are done (`TheShell->shutdownComplete(layout)`, the layout hidden).
//!
//! # Upstream quirks that are kept
//!
//! - The statics live as long as the program: `notShown` and `FirstTimeRunningTheGame` are set once; returning to the
//!   menu does not hide its main panel again. `MainMenuInput`'s first mouse position is the first one it ever saw.
//!   `MainMenuInput` ignores everything once the panel is shown, and a mouse position of exactly (0, 0).
//! - A window the file lacks is used without a check where upstream does (the drop-down panels, the campaigns' save
//!   buttons): recorded as undefined ([`GameWindowManager::undefined`]).
//! - `buttonPushed` stays set after a push to another screen: the menu takes no other button until its init runs again.
//! - `launchChallengeMenu` is cleared by any button but the three difficulties, when the transitions are done ~~(always,
//!   here)~~ (**Corrected 2026-11-10 (#77):** as upstream's handler says).
//!
//! # Where this differs from upstream
//!
//! - ~~**The transitions** (`TheTransitionHandler`: `setGroup`, `reverse`, `remove`, `isFinished`, and the groups of
//!   `WindowTransitions.ini` that fade the panels in and out) are not translated (#69): the calls are given to the
//!   host (`MenuHost::transition`) and every group is finished at once. So `dontAllowTransitions` is cleared at the
//!   next update, and a panel shown stays shown, with the previous one, until the file's own windows say otherwise:
//!   upstream's transitions are what hide the panel left behind. Recorded as a gap.~~ **Corrected 2026-11-10 (#77):**
//!   the transitions are translated ([`crate::client::window_transitions`], the window manager's handler): the menu
//!   calls `setGroup`, `reverse`, `remove` and `isFinished` as upstream does, the groups of `WindowTransitions.ini`
//!   hide the panel left behind and show the next (the difficulty panel among them), and `dontAllowTransitions` is
//!   cleared by the first update once they are done. A group the INI lacks is upstream's `NULL`: `reverse` of it with
//!   none current is recorded as undefined.
//! - `TheScriptEngine->signalUIInteract` (the shell map's scripts) is given to the host
//!   ([`MenuHost::signal_ui_interact`]), which only records it: there is no shell map game here, and the client never
//!   changes the simulation.
//! - Online and the web: `ghttpStartup` / `ghttpCleanup`, `TearDownGameSpy`, `StopAsyncDNSCheck`, `StartPatchCheck`
//!   (the Online button: its panel state changes, the patch check and the online menus do not happen),
//!   `CancelPatchCheckCallback`, `StartDownloadingPatches`, `RaiseGSMessageBox`, `HTTPThinkWrapper`,
//!   `GameSpyUpdateOverlays`, `TheDownloadManager`, `TheMapCache->updateCache`: not translated (gaps).
//! - `TheShell->loadScheme("MainMenu")` (shell menu schemes)~~, `reverseAnimatewindow`~~: not translated.
//!   **Corrected 2026-11-10 (#77):** `reverseAnimatewindow` is translated (the shutdown and `setupGameStart` call it;
//!   [`shell::reverse_animate_window`]).
//! - `TheWritableGlobalData->m_breakTheMovie`~~, `DontShowMainMenu` (false in the game)~~: not modelled.
//!   **Corrected 2026-11-10 (#89):** `DontShowMainMenu` is ScoreScreen.cpp's global, true while the score screen is
//!   up (over the main menu): [`MenuHost::score_screen_globals`]; `MainMenuUpdate` reads it.
//! - `IsFirstCDPresent` (`TheFileSystem->areMusicFilesOnCD()`) is the host's ([`MenuHost::first_cd_present`]); the
//!   insert-disc prompt (`ExMessageBoxOkCancel`, ExtendedMessageBox.cpp) is not translated: without the disc the
//!   difficulty buttons do nothing.
//! - `_spawnl` of `WorldBuilder.exe` is the host's ([`MenuHost::spawn_world_builder`]): it fails here, so the menu shows
//!   upstream's failure box.
//! - ~~`OptionPreferences::setCampaignDifficulty` / `write` (Options.ini): not written.~~ **Corrected 2026-11-10
//!   (#89):** `prepareCampaignGame` writes the campaign difficulty to `Options.ini` in the host's user data folder
//!   ([`MenuHost::user_data_write`]).
//! - Starting the campaign is a [`ShellRequest::StartCampaign`] to the host: the campaign set, the difficulty, as
//!   `MSG_NEW_GAME` carries them (the World's `start_campaign` does the rest of `prepareCampaignGame` and
//!   `doGameStart`, `InitRandom(0)` included).
//!
//! # Representation
//!
//! [`MainMenu`]: the file's statics. The window ids come from the host's name keys ([`ShellHost::name_to_key`]), as
//! the file's windows got theirs.

use crate::client::message_box::quit_message_box_yes_no;
use crate::client::shell::{self, ShellHost, ShellRequest};
use crate::client::window_manager::{data_win, ggm, gwm, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;

/// `DROPDOWN_NONE` .. `DROPDOWN_DIFFICULTY`, `DROPDOWN_COUNT`.
pub const DROPDOWN_NONE: usize = 0;
pub const DROPDOWN_SINGLE: usize = 1;
pub const DROPDOWN_MULTIPLAYER: usize = 2;
pub const DROPDOWN_MAIN: usize = 3;
pub const DROPDOWN_LOADREPLAY: usize = 4;
pub const DROPDOWN_DIFFICULTY: usize = 5;
pub const DROPDOWN_COUNT: usize = 6;

/// `SHOW_NONE` .. `SHOW_SKIRMISH`.
pub const SHOW_NONE: i32 = 0;
pub const SHOW_TRAINING: i32 = 1;
pub const SHOW_USA: i32 = 2;
pub const SHOW_GLA: i32 = 3;
pub const SHOW_CHINA: i32 = 4;
pub const SHOW_SKIRMISH: i32 = 5;

/// `GameDifficulty`: `DIFFICULTY_EASY`, `DIFFICULTY_NORMAL`, `DIFFICULTY_HARD`.
pub const DIFFICULTY_EASY: i32 = 0;
pub const DIFFICULTY_NORMAL: i32 = 1;
pub const DIFFICULTY_HARD: i32 = 2;

/// The buttons `MainMenuInit` looks up, by their names after `MainMenu.wnd:`.
pub const BUTTONS: [&str; 30] = [
    "ButtonSkirmish", "ButtonOnline", "ButtonNetwork", "ButtonOptions", "ButtonExit", "ButtonMOTD", "ButtonWorldBuilder", "ButtonGetUpdate", "ButtonChallenge", "ButtonUSA",
    "ButtonGLA", "ButtonChina", "ButtonUSARecentSave", "ButtonUSALoadGame", "ButtonGLARecentSave", "ButtonGLALoadGame", "ButtonChinaRecentSave", "ButtonChinaLoadGame", "ButtonSinglePlayer",
    "ButtonMultiplayer", "ButtonMultiBack", "ButtonSingleBack", "ButtonLoadReplayBack", "ButtonReplay", "ButtonLoadReplay", "ButtonLoadGame", "ButtonCredits", "ButtonEasy", "ButtonMedium",
    "ButtonHard",
];

/// What the main menu needs besides the shell.
pub trait MenuHost: ShellHost {
    /// The file's statics.
    fn main_menu(&mut self) -> &mut MainMenu;
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `TheScriptEngine->signalUIInteract(TheShellHookNames[hook])` (the shell map's scripts: recorded only).
    fn signal_ui_interact(&mut self, _hook: &'static str) {}
    /// `IsFirstCDPresent()`.
    fn first_cd_present(&self) -> bool {
        true
    }
    /// (#89 part 2) ScoreScreen.cpp's globals (`DontShowMainMenu`, `ReplayWasPressed`, ...): `None` for a host without
    /// the score screen (they keep their initial values, false).
    fn score_screen_globals(&mut self) -> Option<&mut crate::client::score_screen::ScoreScreenGlobals> {
        None
    }
    /// `_spawnl(_P_NOWAIT, "WorldBuilder.exe", ...) >= 0`.
    fn spawn_world_builder(&mut self) -> bool {
        false
    }
    /// (#93) `TheGlobalData->getPath_UserData()`, the folder of the preference files (`UserPreferences::load`).
    fn path_user_data(&self) -> Vec<u8> {
        Vec::new()
    }
    /// (#93) `fopen(path, "r")` in the user data folder: the file's bytes (`None`: it does not open; a host without a
    /// folder has none).
    fn user_data_read(&mut self, _path: &[u8]) -> Option<Vec<u8>> {
        None
    }
    /// (#93) `fopen(path, "w")` and what is written; whether it opened.
    fn user_data_write(&mut self, _path: &[u8], _bytes: &[u8]) -> bool {
        false
    }
}

/// MainMenu.cpp's statics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MainMenu {
    pub raise_message_boxes: bool,
    pub campaign_selected: bool,
    pub button_pushed: bool,
    pub is_shutting_down: bool,
    pub start_game: bool,
    pub initial_gadget_delay: i32,
    pub show_fade: bool,
    pub drop_down: usize,
    pub pending_drop_down: usize,
    pub not_shown: bool,
    pub first_time_running_the_game: bool,
    pub show_logo: bool,
    pub show_side: i32,
    pub logo_is_shown: bool,
    pub just_entered: bool,
    pub launch_challenge_menu: bool,
    pub dont_allow_transitions: bool,
    /// `parentMainMenu`.
    pub parent: Option<WinId>,
    /// The buttons of [`BUTTONS`], in its order.
    pub buttons: Vec<Option<WinId>>,
    pub drop_down_windows: [Option<WinId>; DROPDOWN_COUNT],
    /// `MainMenuInput`'s `static Int mousePosX, mousePosY` (set by the first position it sees).
    pub first_mouse: Option<(i32, i32)>,
    /// `TheMouse->setVisibility`.
    pub mouse_visible: bool,
    /// `TheCampaignManager->setCampaign(...)`'s name (empty: none).
    pub campaign: Vec<u8>,
    /// `TheCampaignManager->setGameDifficulty(...)`.
    pub difficulty: i32,
}

impl Default for MainMenu {
    /// The statics' initial values.
    fn default() -> Self {
        MainMenu {
            raise_message_boxes: true,
            campaign_selected: false,
            button_pushed: false,
            is_shutting_down: false,
            start_game: false,
            initial_gadget_delay: 210,
            show_fade: false,
            drop_down: DROPDOWN_NONE,
            pending_drop_down: DROPDOWN_NONE,
            not_shown: true,
            first_time_running_the_game: true,
            show_logo: false,
            show_side: SHOW_NONE,
            logo_is_shown: false,
            just_entered: false,
            launch_challenge_menu: false,
            dont_allow_transitions: false,
            parent: None,
            buttons: vec![None; BUTTONS.len()],
            drop_down_windows: [None; DROPDOWN_COUNT],
            first_mouse: None,
            mouse_visible: true,
            campaign: Vec::new(),
            difficulty: DIFFICULTY_NORMAL,
        }
    }
}

fn key<H: ShellHost>(h: &mut H, name: &str) -> i32 {
    h.name_to_key(format!("MainMenu.wnd:{name}").as_bytes()) as i32
}

/// A button's window (as `MainMenuInit` found it).
pub fn button<H: MenuHost>(h: &mut H, name: &str) -> Option<WinId> {
    let i = BUTTONS.iter().position(|&b| b == name)?;
    h.main_menu().buttons[i]
}

/// `winHide` on a window upstream uses without a check.
fn hide_checked(wm: &mut GameWindowManager, w: Option<WinId>, hide: bool, what: &'static str) -> bool {
    match w {
        Some(w) => {
            wm.win_hide(w, hide);
            true
        }
        None => {
            wm.set_undefined(what);
            false
        }
    }
}

/// `HandleCanceledDownload(resetDropDown)`.
pub fn handle_canceled_download<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, reset_drop_down: bool) {
    h.main_menu().button_pushed = false;
    if reset_drop_down {
        let d = h.main_menu().drop_down_windows[DROPDOWN_MAIN];
        hide_checked(wm, d, false, "HandleCanceledDownload: no MapBorder2");
        wt::set_group(wm, h, b"MainMenuDefaultMenuLogoFade", false);
    }
}

/// `showSelectiveButtons(show)`: the campaigns' save buttons, the shown side's only.
fn show_selective_buttons<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, show: i32) {
    for (name, side) in [("ButtonUSARecentSave", SHOW_USA), ("ButtonUSALoadGame", SHOW_USA), ("ButtonGLARecentSave", SHOW_GLA), ("ButtonGLALoadGame", SHOW_GLA), ("ButtonChinaRecentSave", SHOW_CHINA), ("ButtonChinaLoadGame", SHOW_CHINA)] {
        let w = button(h, name);
        if !hide_checked(wm, w, show != side, "showSelectiveButtons: a campaign's save button is missing") {
            return;
        }
    }
}

/// `quitCallback()`: the menu popped, the engine quitting.
pub fn quit_callback<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.main_menu().button_pushed = true;
    h.signal_ui_interact("SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED");
    shell::pop(wm, h);
    h.request(ShellRequest::Quit);
    if h.env().in_game {
        h.request(ShellRequest::ClearGameData);
    }
}

/// `setupGameStart(mapName, diff)`.
pub fn setup_game_start<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, diff: i32) {
    h.main_menu().difficulty = diff;
    if h.main_menu().launch_challenge_menu {
        // (`TheChallengeGenerals->setCurrentDifficulty(diff)`: the challenge menu's)
        h.main_menu().campaign_selected = true;
        shell::push(wm, h, b"Menus/ChallengeMenu.wnd", false);
        wt::reverse(wm, h, b"MainMenuDifficultyMenuTraining");
    } else {
        // (`m_pendingFile = mapName`: the campaign's current map, the World's)
        h.main_menu().start_game = true;
        shell::reverse_animate_window(wm, h);
        wt::set_group(wm, h, b"FadeWholeScreen", false);
    }
}

/// `prepareCampaignGame(diff)`.
fn prepare_campaign_game<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, diff: i32) {
    let m = h.main_menu();
    m.dont_allow_transitions = true;
    // (#88) OptionPreferences pref; pref.setCampaignDifficulty(diff); pref.write(); (`TheScriptEngine->
    // setGlobalDifficulty(diff)`: the World's, in `start_campaign`)
    let mut pref = crate::client::options_menu::OptionPreferences::new(wm, h);
    pref.set_campaign_difficulty(diff);
    pref.write(&mut crate::client::user_preferences::HostUserData(h));
    h.main_menu().button_pushed = false;
    wt::reverse(wm, h, b"MainMenuDifficultyMenuBack");
    setup_game_start(wm, h, diff);
}

/// `doGameStart()`: `MSG_NEW_GAME` (here the host's [`ShellRequest::StartCampaign`]); the menu shuts down.
fn do_game_start<H: MenuHost>(_wm: &mut GameWindowManager, h: &mut H) {
    h.main_menu().start_game = false;
    if h.env().in_game {
        h.request(ShellRequest::ClearGameData);
    }
    let m = h.main_menu();
    let r = ShellRequest::StartCampaign { campaign: m.campaign.clone(), difficulty: m.difficulty };
    h.request(r);
    h.main_menu().is_shutting_down = true;
}

/// `checkCDBeforeCampaign(diff)`.
fn check_cd_before_campaign<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, diff: i32) {
    if !h.first_cd_present() {
        // `ExMessageBoxOkCancel(GUI:InsertCDPrompt, ...)`: not translated (see the module docs)
        return;
    }
    prepare_campaign_game(wm, h, diff);
}

/// The file's `shutdownComplete(layout)`: the layout hidden, the shell told.
fn menu_shutdown_complete<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.main_menu().is_shutting_down = false;
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// The window named `MainMenu.wnd:<name>` under the parent (`winGetWindowFromId(parentMainMenu, ...)`).
fn find<H: MenuHost>(wm: &GameWindowManager, h: &mut H, name: &str) -> Option<WinId> {
    let id = key(h, name);
    let p = h.main_menu().parent;
    wm.win_get_window_from_id(p, id)
}

/// `initialHide()`: the faction logos hidden.
fn initial_hide<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    for n in [
        "WinFactionGLA", "WinFactionChina", "WinFactionUS", "WinGrowMarker", "WinFactionTraining", "WinFactionTrainingSmall", "WinFactionTrainingMedium", "WinFactionSkirmish",
        "WinFactionSkirmishSmall", "WinFactionSkirmishMedium", "WinFactionUS", "WinFactionUSSmall", "WinFactionUSMedium", "WinFactionGLA", "WinFactionGLASmall", "WinFactionGLAMedium",
        "WinFactionChina", "WinFactionChinaSmall", "WinFactionChinaMedium",
    ] {
        if let Some(w) = find(wm, h, n) {
            wm.win_hide(w, true);
        }
    }
}

/// `MainMenuInit(layout, userData)`.
pub fn main_menu_init<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    shell::show_shell_map(wm, h, true);
    let m = h.main_menu();
    m.mouse_visible = true;
    m.button_pushed = false;
    m.is_shutting_down = false;
    m.start_game = false;
    m.drop_down = DROPDOWN_NONE;
    m.pending_drop_down = DROPDOWN_NONE;
    m.drop_down_windows = [None; DROPDOWN_COUNT];
    let parent_id = key(h, "MainMenuParent");
    let parent = wm.win_get_window_from_id(None, parent_id);
    h.main_menu().parent = parent;
    for (i, b) in BUTTONS.iter().enumerate() {
        let w = find(wm, h, b);
        h.main_menu().buttons[i] = w;
    }
    for (i, n) in [(DROPDOWN_SINGLE, "MapBorder"), (DROPDOWN_MULTIPLAYER, "MapBorder1"), (DROPDOWN_MAIN, "MapBorder2"), (DROPDOWN_LOADREPLAY, "MapBorder3"), (DROPDOWN_DIFFICULTY, "MapBorder4")] {
        let w = find(wm, h, n);
        h.main_menu().drop_down_windows[i] = w;
    }
    for i in 1..DROPDOWN_COUNT {
        let w = h.main_menu().drop_down_windows[i];
        if !hide_checked(wm, w, true, "MainMenuInit: a drop-down panel (MapBorder..MapBorder4) is missing") {
            return;
        }
    }
    initial_hide(wm, h);
    show_selective_buttons(wm, h, SHOW_NONE);
    if wm.undefined.is_some() {
        return;
    }
    wm.layout_hide(layout, false);
    // (GameSpy's teardown, the map cache, `loadScheme("MainMenu")`: not translated)
    h.main_menu().raise_message_boxes = true;
    let rule = find(wm, h, "MainMenuRuler");
    if let Some(r) = rule {
        wm.win_hide(r, true);
    }
    h.main_menu().campaign_selected = false;
    if h.main_menu().first_time_running_the_game {
        h.main_menu().mouse_visible = false;
        wt::reverse(wm, h, b"FadeWholeScreen");
        h.main_menu().first_time_running_the_game = false;
    } else {
        let m = h.main_menu();
        m.show_fade = true;
        m.just_entered = true;
        m.initial_gadget_delay = 2;
        if let Some(r) = rule {
            wm.win_hide(r, false);
        }
    }
    wm.layout_bring_forward(layout);
    let p = h.main_menu().parent;
    wm.win_set_focus(h, p);
}

/// `MainMenuShutdown(layout, userData)`: `immediate` is `*(Bool *)userData`.
pub fn main_menu_shutdown<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, immediate: Option<bool>) {
    if !h.main_menu().start_game {
        h.main_menu().is_shutting_down = true;
    }
    let Some(pop_immediate) = immediate else {
        wm.set_undefined("MainMenuShutdown: no user data (*(Bool *)NULL)");
        return;
    };
    if pop_immediate {
        menu_shutdown_complete(wm, h, layout);
        return;
    }
    // (#77) the update completes the shutdown once the animations are done
    if !h.main_menu().start_game {
        shell::reverse_animate_window(wm, h);
    }
}

/// `MainMenuUpdate(layout, userData)`.
pub fn main_menu_update<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let env = h.env();
    if env.in_game && !env.in_shell_game {
        return;
    }
    // (#89 part 2) `if(DontShowMainMenu && justEntered) justEntered = FALSE;`
    if h.score_screen_globals().is_some_and(|g| g.dont_show_main_menu) && h.main_menu().just_entered {
        h.main_menu().just_entered = false;
    }
    if h.main_menu().just_entered {
        if h.main_menu().initial_gadget_delay == 1 {
            wt::set_group(wm, h, b"MainMenuDefaultMenuLogoFade", false);
            let p = h.main_menu().parent;
            wm.win_set_focus(h, p);
            let m = h.main_menu();
            m.initial_gadget_delay = 2;
            m.just_entered = false;
        } else {
            h.main_menu().initial_gadget_delay -= 1;
        }
    }
    if h.main_menu().dont_allow_transitions && wt::is_finished(wm) {
        h.main_menu().dont_allow_transitions = false;
    }
    if h.main_menu().show_logo && !h.main_menu().dont_allow_transitions {
        let g = match h.main_menu().show_side {
            SHOW_TRAINING => Some("MainMenuFactionTraining"),
            SHOW_CHINA => Some("MainMenuFactionChina"),
            SHOW_GLA => Some("MainMenuFactionGLA"),
            SHOW_USA => Some("MainMenuFactionUS"),
            SHOW_SKIRMISH => Some("MainMenuFactionSkirmish"),
            _ => None,
        };
        if let Some(g) = g {
            wt::set_group(wm, h, g.as_bytes(), false);
        }
        h.main_menu().show_logo = false;
    }
    if h.main_menu().raise_message_boxes {
        // (`RaiseGSMessageBox()`: online)
        h.main_menu().raise_message_boxes = false;
    }
    let finished = shell::is_anim_finished(wm, h) && wt::is_finished(wm);
    if h.main_menu().start_game && finished {
        do_game_start(wm, h);
    }
    if h.main_menu().is_shutting_down && finished {
        menu_shutdown_complete(wm, h, layout);
    }
}

/// The main panel shown, as the first key or mouse move does.
fn first_show<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H) -> MsgHandled {
    h.main_menu().initial_gadget_delay = 1;
    let d = h.main_menu().drop_down_windows[DROPDOWN_MAIN];
    if !hide_checked(wm, d, false, "MainMenuInput: no MapBorder2") {
        return MsgHandled::Handled;
    }
    wt::set_group(wm, h, b"MainMenuFade", true);
    wt::set_group(wm, h, b"MainMenuDefaultMenu", false);
    let m = h.main_menu();
    m.mouse_visible = true;
    m.not_shown = false;
    MsgHandled::Handled
}

/// `MainMenuInput(window, msg, mData1, mData2)`.
pub fn main_menu_input<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    if !h.main_menu().not_shown {
        return MsgHandled::Ignored;
    }
    match msg {
        gwm::MOUSE_POS => {
            let (x, y) = ((d1 & 0xFFFF) as i32, (d1 >> 16) as i32);
            if x == 0 && y == 0 {
                return MsgHandled::Ignored;
            }
            let (fx, fy) = *h.main_menu().first_mouse.get_or_insert((x, y));
            if (x - fx).abs() > 20 || (y - fy).abs() > 20 {
                return first_show(wm, h);
            }
            MsgHandled::Ignored
        }
        gwm::CHAR => first_show(wm, h),
        _ => MsgHandled::Ignored,
    }
}

/// `GBM_MOUSE_ENTERING` / `GBM_MOUSE_LEAVING` on a faction button: the logo's transition.
fn faction_hover<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, entering: bool, side: i32, group: &'static str) {
    let m = h.main_menu();
    if entering {
        if m.dont_allow_transitions && !m.campaign_selected {
            m.show_logo = true;
            m.show_side = side;
        }
        if m.campaign_selected || m.dont_allow_transitions {
            return;
        }
        wt::set_group(wm, h, group.as_bytes(), false);
    } else {
        if m.dont_allow_transitions && !m.campaign_selected && m.show_logo {
            m.show_logo = false;
            m.show_side = SHOW_NONE;
        }
        if m.campaign_selected || m.dont_allow_transitions {
            return;
        }
        wt::reverse(wm, h, group.as_bytes());
    }
}

/// `MainMenuSystem(window, msg, mData1, mData2)`.
pub fn main_menu_system<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        // (`ghttpStartup`; `ghttpCleanup`, `TearDownGameSpy`, `StopAsyncDNSCheck`: online)
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GBM_MOUSE_ENTERING | ggm::GBM_MOUSE_LEAVING => {
            let entering = msg == ggm::GBM_MOUSE_ENTERING;
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let id = wm.tree.windows[control].id;
            let hook = |what: &str| -> &'static str {
                match (what, entering) {
                    ("Online", true) => "SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_HIGHLIGHTED",
                    ("Online", false) => "SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_UNHIGHLIGHTED",
                    ("Network", true) => "SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_HIGHLIGHTED",
                    ("Network", false) => "SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_UNHIGHLIGHTED",
                    ("Options", true) => "SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_HIGHLIGHTED",
                    ("Options", false) => "SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_UNHIGHLIGHTED",
                    (_, true) => "SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_HIGHLIGHTED",
                    (_, false) => "SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_UNHIGHLIGHTED",
                }
            };
            if id == key(h, "ButtonOnline") {
                h.signal_ui_interact(hook("Online"));
            } else if id == key(h, "ButtonNetwork") {
                h.signal_ui_interact(hook("Network"));
            } else if id == key(h, "ButtonOptions") {
                h.signal_ui_interact(hook("Options"));
            } else if id == key(h, "ButtonExit") {
                h.signal_ui_interact(hook("Exit"));
            } else if id == key(h, "ButtonChallenge") {
                faction_hover(wm, h, entering, SHOW_TRAINING, "MainMenuFactionTraining");
            } else if id == key(h, "ButtonSkirmish") {
                faction_hover(wm, h, entering, SHOW_SKIRMISH, "MainMenuFactionSkirmish");
            } else if id == key(h, "ButtonUSA") {
                faction_hover(wm, h, entering, SHOW_USA, "MainMenuFactionUS");
            } else if id == key(h, "ButtonGLA") {
                faction_hover(wm, h, entering, SHOW_GLA, "MainMenuFactionGLA");
            } else if id == key(h, "ButtonChina") {
                faction_hover(wm, h, entering, SHOW_CHINA, "MainMenuFactionChina");
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            main_menu_selected(wm, h, control);
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// A drop-down panel shown (`dropDownWindows[i]->winHide(FALSE)`, used without a check).
fn show_panel<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, i: usize) {
    let d = h.main_menu().drop_down_windows[i];
    hide_checked(wm, d, false, "MainMenuSystem: a drop-down panel is missing");
}

/// `MainMenuSystem`'s `GBM_SELECTED`.
fn main_menu_selected<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H, control: WinId) {
    if h.main_menu().button_pushed {
        return;
    }
    let id = wm.tree.windows[control].id;
    let is = |h: &mut H, name: &str| id == key(h, name);
    // `TheTransitionHandler->isFinished()` and not a difficulty
    if wt::is_finished(wm) && !is(h, "ButtonEasy") && !is(h, "ButtonMedium") && !is(h, "ButtonHard") {
        h.main_menu().launch_challenge_menu = false;
    }
    let dont = h.main_menu().dont_allow_transitions;
    let campaign_selected = h.main_menu().campaign_selected;
    if is(h, "ButtonSinglePlayer") {
        if dont {
            return;
        }
        let m = h.main_menu();
        m.dont_allow_transitions = true;
        m.button_pushed = false;
        show_panel(wm, h, DROPDOWN_SINGLE);
        wt::remove(wm, h, b"MainMenuDefaultMenu", false);
        wt::reverse(wm, h, b"MainMenuDefaultMenuBack");
        wt::set_group(wm, h, b"MainMenuSinglePlayerMenu", false);
    } else if is(h, "ButtonSingleBack") {
        if campaign_selected || dont {
            return;
        }
        h.main_menu().button_pushed = false;
        show_panel(wm, h, DROPDOWN_MAIN);
        wt::remove(wm, h, b"MainMenuSinglePlayerMenu", false);
        wt::reverse(wm, h, b"MainMenuSinglePlayerMenuBack");
        wt::set_group(wm, h, b"MainMenuDefaultMenu", false);
        h.main_menu().dont_allow_transitions = true;
    } else if is(h, "ButtonMultiBack") || is(h, "ButtonLoadReplayBack") {
        if dont {
            return;
        }
        let m = h.main_menu();
        m.dont_allow_transitions = true;
        m.button_pushed = false;
        show_panel(wm, h, DROPDOWN_MAIN);
        if is(h, "ButtonMultiBack") {
            wt::remove(wm, h, b"MainMenuMultiPlayerMenu", false);
            wt::reverse(wm, h, b"MainMenuMultiPlayerMenuReverse");
        } else {
            wt::remove(wm, h, b"MainMenuLoadReplayMenu", false);
            wt::reverse(wm, h, b"MainMenuLoadReplayMenuBack");
        }
        wt::set_group(wm, h, b"MainMenuDefaultMenu", false);
    } else if Some(control) == button(h, "ButtonCredits") {
        if dont {
            return;
        }
        let m = h.main_menu();
        m.dont_allow_transitions = true;
        m.button_pushed = true;
        shell::push(wm, h, b"Menus/CreditsMenu.wnd", false);
        show_panel(wm, h, DROPDOWN_MAIN);
        wt::reverse(wm, h, b"MainMenuDefaultMenu");
    } else if is(h, "ButtonMultiplayer") || is(h, "ButtonLoadReplay") {
        if dont {
            return;
        }
        let m = h.main_menu();
        m.dont_allow_transitions = true;
        m.button_pushed = false;
        let multi = is(h, "ButtonMultiplayer");
        show_panel(wm, h, if multi { DROPDOWN_MULTIPLAYER } else { DROPDOWN_LOADREPLAY });
        wt::remove(wm, h, b"MainMenuDefaultMenu", false);
        wt::reverse(wm, h, b"MainMenuDefaultMenuBack");
        wt::set_group(wm, h, if multi { "MainMenuMultiPlayerMenu" } else { "MainMenuLoadReplayMenu" }.as_bytes(), false);
    } else if is(h, "ButtonLoadGame") || is(h, "ButtonReplay") {
        if dont {
            return;
        }
        let m = h.main_menu();
        m.dont_allow_transitions = true;
        m.button_pushed = true;
        show_panel(wm, h, DROPDOWN_LOADREPLAY);
        wt::reverse(wm, h, b"MainMenuLoadReplayMenuBackTransition");
        let to: &[u8] = if is(h, "ButtonLoadGame") { b"Menus/SaveLoad.wnd" } else { b"Menus/ReplayMenu.wnd" };
        shell::push(wm, h, to, false);
    } else if is(h, "ButtonSkirmish") {
        if campaign_selected || dont {
            return;
        }
        let m = h.main_menu();
        m.button_pushed = true;
        m.campaign_selected = true;
        show_panel(wm, h, DROPDOWN_SINGLE);
        wt::remove(wm, h, b"MainMenuFactionSkirmish", false);
        wt::reverse(wm, h, b"MainMenuSinglePlayerMenuBackSkirmish");
        shell::push(wm, h, b"Menus/SkirmishGameOptionsMenu.wnd", false);
        h.signal_ui_interact("SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_SELECTED");
    } else if is(h, "ButtonOnline") {
        if dont {
            return;
        }
        let m = h.main_menu();
        m.dont_allow_transitions = true;
        m.button_pushed = true;
        show_panel(wm, h, DROPDOWN_MULTIPLAYER);
        wt::reverse(wm, h, b"MainMenuMultiPlayerMenuTransitionToNext");
        // (`StartPatchCheck()`: online, not translated)
        h.main_menu().drop_down = DROPDOWN_NONE;
    } else if is(h, "ButtonNetwork") {
        if dont {
            return;
        }
        let m = h.main_menu();
        m.dont_allow_transitions = true;
        m.button_pushed = true;
        show_panel(wm, h, DROPDOWN_MULTIPLAYER);
        wt::reverse(wm, h, b"MainMenuMultiPlayerMenuTransitionToNext");
        shell::push(wm, h, b"Menus/LanLobbyMenu.wnd", false);
        h.signal_ui_interact("SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_SELECTED");
    } else if is(h, "ButtonOptions") {
        if dont {
            return;
        }
        h.main_menu().dont_allow_transitions = true;
        h.signal_ui_interact("SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_SELECTED");
        let Some(opt) = shell::get_options_layout(wm, h, true) else {
            wm.set_undefined("MainMenuSystem: the options layout did not load (runInit on NULL)");
            return;
        };
        shell::run_layout(wm, h, opt, 0, None);
        wm.layout_hide(opt, false);
        wm.layout_bring_forward(opt);
    } else if is(h, "ButtonWorldBuilder") {
        if !h.spawn_world_builder() {
            let t = h.fetch_text(b"GUI:WorldBuilder");
            let b = h.fetch_text(b"GUI:WorldBuilderLoadFailed");
            crate::client::message_box::message_box_ok(wm, h, &t, &b, None);
        }
    } else if is(h, "ButtonGetUpdate") {
        // (`StartDownloadingPatches()`: online)
    } else if is(h, "ButtonExit") {
        if h.env().windowed {
            quit_callback(wm, h);
        } else {
            let t = h.fetch_text(b"GUI:QuitPopupTitle");
            let b = h.fetch_text(b"GUI:QuitPopupMessage");
            quit_message_box_yes_no(wm, h, &t, &b, Some("quitCallback"), None);
        }
    } else if is(h, "ButtonChallenge") {
        if campaign_selected || dont {
            return;
        }
        wt::set_group(wm, h, b"MainMenuFactionTraining", false);
        if let Some(w) = find(wm, h, "WinFactionTraining") {
            wm.win_hide(w, true);
        }
        wt::reverse(wm, h, b"MainMenuSinglePlayerMenuBackTraining");
        wt::set_group(wm, h, b"MainMenuDifficultyMenuTraining", false);
        let m = h.main_menu();
        m.campaign_selected = true;
        m.show_logo = false;
        m.show_side = SHOW_TRAINING;
        m.launch_challenge_menu = true;
    } else if let Some((campaign, group, logo, back, diff_menu, side)) = [
        ("ButtonUSA", "USA", "MainMenuFactionUS", "WinFactionUS", "MainMenuSinglePlayerMenuBackUS", "MainMenuDifficultyMenuUS", SHOW_USA),
        ("ButtonGLA", "GLA", "MainMenuFactionGLA", "WinFactionGLA", "MainMenuSinglePlayerMenuBackGLA", "MainMenuDifficultyMenuGLA", SHOW_GLA),
        ("ButtonChina", "China", "MainMenuFactionChina", "WinFactionChina", "MainMenuSinglePlayerMenuBackChina", "MainMenuDifficultyMenuChina", SHOW_CHINA),
    ]
    .into_iter()
    .find(|t| is(h, t.0))
    .map(|t| (t.1, t.2, t.3, t.4, t.5, t.6))
    {
        if campaign_selected || dont {
            return;
        }
        h.main_menu().campaign = campaign.as_bytes().to_vec();
        wt::set_group(wm, h, group.as_bytes(), false);
        wt::remove(wm, h, group.as_bytes(), true);
        if let Some(w) = find(wm, h, logo) {
            wm.win_hide(w, true);
        }
        wt::reverse(wm, h, back.as_bytes());
        wt::set_group(wm, h, diff_menu.as_bytes(), false);
        let m = h.main_menu();
        m.campaign_selected = true;
        m.logo_is_shown = false;
        m.show_logo = false;
        m.show_side = side;
    } else if is(h, "ButtonEasy") || is(h, "ButtonMedium") || is(h, "ButtonHard") {
        if dont {
            return;
        }
        let diff = if is(h, "ButtonEasy") {
            DIFFICULTY_EASY
        } else if is(h, "ButtonMedium") {
            DIFFICULTY_NORMAL
        } else {
            DIFFICULTY_HARD
        };
        check_cd_before_campaign(wm, h, diff);
    } else if id == key(h, "ButtonDiffBack") {
        if dont {
            return;
        }
        let m = h.main_menu();
        m.dont_allow_transitions = true;
        m.campaign.clear();
        diff_reverse_side(wm, h);
        h.main_menu().campaign_selected = false;
    }
}

/// `diffReverseSide()`.
fn diff_reverse_side<H: MenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    let (back, to) = match h.main_menu().show_side {
        SHOW_TRAINING => ("MainMenuDifficultyMenuTrainingBack", "MainMenuSinglePlayerTrainingMenuFromDiff"),
        SHOW_USA => ("MainMenuDifficultyMenuUSBack", "MainMenuSinglePlayerUSAMenuFromDiff"),
        SHOW_GLA => ("MainMenuDifficultyMenuGLABack", "MainMenuSinglePlayerGLAMenuFromDiff"),
        SHOW_CHINA => ("MainMenuDifficultyMenuChinaBack", "MainMenuSinglePlayerChinaMenuFromDiff"),
        _ => return,
    };
    wt::reverse(wm, h, back.as_bytes());
    wt::set_group(wm, h, to.as_bytes(), false);
}
