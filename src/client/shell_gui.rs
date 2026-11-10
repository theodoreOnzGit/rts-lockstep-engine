// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/FunctionLexicon.cpp (the entries of gameWinSystemTable,
//     gameWinInputTable, winLayoutInitTable, winLayoutUpdateTable and winLayoutShutdownTable for the callbacks
//     translated here: MainMenuSystem, MessageBoxSystem, QuitMessageBoxSystem, MainMenuInput, MainMenuInit,
//     MainMenuUpdate, MainMenuShutdown)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The shell's host (GitHub #69): the function lexicon's entries for the shell's translated callbacks, with the shell,
//! the main menu's statics and the WND files, as one [`GuiHost`] / [`ShellHost`] / [`MenuHost`] ([`ShellGui`]).
//!
//! # As upstream reads it
//!
//! The WND file names its callbacks; `TheFunctionLexicon` turns each name into a function pointer when the file is
//! read, and the window manager and the layouts call them. The shell's menus are the game's own callbacks
//! (`MainMenuSystem`, `MainMenuInit`, ...).
//!
//! # Where this differs from upstream
//!
//! - The window manager keeps the names (a callback of the game's own is [`SystemFunc::Host`]); [`ShellGui`] maps
//!   them to the translated functions when they are called.
//! - A name this translation does not have yet (`SaveLoadMenuInit`, `OptionsMenuInit`, `SkirmishGameOptionsMenuSystem`,
//!   ... every screen but the main menu and the message boxes) is recorded in [`ShellGui::untranslated`] and does
//!   nothing: the screen loads and shows its windows, but its buttons do nothing and its shutdown never completes
//!   (`Shell::popImmediate` still takes it off). Upstream's lexicon would have the function.
//! - The engine's side: the requests ([`ShellRequest`])~~, the transitions (`Transition`)~~ and the shell hooks are
//!   kept in lists for the host's owner (the client) to act on; the clock and the global data are set by it.
//!   **Corrected 2026-11-10 (#77):** the transitions are no longer recorded here: they are the window manager's
//!   handler ([`crate::client::window_transitions`]), loaded from `WindowTransitions.ini` by the owner
//!   ([`synthetic::window_transitions_ini`] in the tests).
//!
//! [`SystemFunc::Host`]: crate::client::window_manager::SystemFunc::Host

use crate::client::main_menu::{self, MainMenu, MenuHost};
use crate::client::message_box;
use crate::client::shell::{self, LayoutFiles, Shell, ShellEnv, ShellHost, ShellRequest, WndFiles};
use crate::client::window_manager::{GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// The shell's host: the shell, the main menu, the files, and what the callbacks asked of the engine.
#[derive(Clone, Debug)]
pub struct ShellGui<W> {
    pub shell: Shell,
    pub main_menu: MainMenu,
    /// The reader and the files (`None` while a file is being read).
    pub files: Option<LayoutFiles<W>>,
    /// `timeGetTime()`, set by the owner.
    pub time_ms: u32,
    pub env: ShellEnv,
    /// What the callbacks asked of the engine and the logic, oldest first (the owner takes them).
    pub requests: Vec<ShellRequest>,
    /// `TheScriptEngine->signalUIInteract`'s hooks (not run: no shell map).
    pub ui_interacts: Vec<&'static str>,
    /// `TheAudio->addAudioEvent` names (the buttons' clicks), for the owner to play.
    pub audio: Vec<Vec<u8>>,
    /// The callbacks a file named that are not translated yet, as they were called.
    pub untranslated: Vec<Vec<u8>>,
    /// `IsFirstCDPresent()`.
    pub first_cd_present: bool,
}

impl<W: WndFiles> ShellGui<W> {
    /// A host on these files: the shell made (`Shell::Shell`), the main menu's statics at their start, the disc
    /// present.
    pub fn new(files: W) -> Self {
        let width = files.display_size().0;
        ShellGui {
            shell: Shell::with_display_width(width),
            main_menu: MainMenu::default(),
            files: Some(LayoutFiles::new(files)),
            time_ms: 0,
            env: ShellEnv::default(),
            requests: Vec::new(),
            ui_interacts: Vec::new(),
            audio: Vec::new(),
            untranslated: Vec::new(),
            first_cd_present: true,
        }
    }

    /// The files' text (`TheGameText`), through the reader's host.
    fn text(&self, label: &[u8]) -> Vec<u16> {
        self.files.as_ref().map_or_else(Vec::new, |f| f.files.fetch_text(label))
    }
}

impl<W: WndFiles> GuiHost for ShellGui<W> {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        match func {
            b"MainMenuSystem" => main_menu::main_menu_system(wm, self, win, msg, d1, d2),
            b"MessageBoxSystem" => message_box::message_box_system(wm, self, win, msg, d1, d2, false),
            b"QuitMessageBoxSystem" => message_box::message_box_system(wm, self, win, msg, d1, d2, true),
            _ => {
                self.untranslated.push(func.to_vec());
                MsgHandled::Ignored
            }
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        match func {
            b"MainMenuInput" => main_menu::main_menu_input(wm, self, win, msg, d1, d2),
            _ => {
                if !self.untranslated.iter().any(|f| f == func) {
                    self.untranslated.push(func.to_vec());
                }
                MsgHandled::Ignored
            }
        }
    }
    fn layout(&mut self, wm: &mut GameWindowManager, func: &[u8], layout: usize) {
        self.layout_callback(wm, func, layout, None);
    }
    fn add_audio_event(&mut self, name: &[u8]) {
        self.audio.push(name.to_vec());
    }
}

impl<W: WndFiles> ShellHost for ShellGui<W> {
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
        match func {
            b"MainMenuInit" => main_menu::main_menu_init(wm, self, layout),
            b"MainMenuUpdate" => main_menu::main_menu_update(wm, self, layout),
            b"MainMenuShutdown" => main_menu::main_menu_shutdown(wm, self, layout, user),
            _ => self.untranslated.push(func.to_vec()),
        }
    }
    fn time_ms(&self) -> u32 {
        self.time_ms
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
    fn message_box_callback(&mut self, wm: &mut GameWindowManager, func: &[u8]) {
        match func {
            b"quitCallback" => main_menu::quit_callback(wm, self),
            _ => self.untranslated.push(func.to_vec()),
        }
    }
}

impl<W: WndFiles> MenuHost for ShellGui<W> {
    fn main_menu(&mut self) -> &mut MainMenu {
        &mut self.main_menu
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        self.text(label)
    }
    fn signal_ui_interact(&mut self, hook: &'static str) {
        self.ui_interacts.push(hook);
    }
    fn first_cd_present(&self) -> bool {
        self.first_cd_present
    }
}

/// The game's start of the shell (`GameClient::update` after the intro): `showShellMap(TRUE)`, then `showShell()`.
pub fn start_shell<W: WndFiles>(wm: &mut GameWindowManager, h: &mut ShellGui<W>) {
    shell::show_shell_map(wm, h, true);
    shell::show_shell(wm, h, true);
}

/// Synthetic WND text for the shell's tests (no EA file): what the game's `Menus/MainMenu.wnd`, `MessageBox.wnd` and
/// `QuitMessageBox.wnd` are made of, as far as the translated callbacks look (the window names), laid out anew.
pub mod synthetic {
    /// One push button `MainMenu.wnd:<name>` at (x, y), 100x20, hidden or shown.
    fn button(name: &str, x: i32, y: i32, hidden: bool) -> String {
        let st = if hidden { "ENABLED+HIDDEN" } else { "ENABLED" };
        format!(
            "  WINDOW\n    WINDOWTYPE = PUSHBUTTON;\n    SCREENRECT = UPPERLEFT: {x} {y}, BOTTOMRIGHT: {} {}, CREATIONRESOLUTION: 800 600;\n    NAME = \"MainMenu.wnd:{name}\";\n    STATUS = {st};\n    STYLE = PUSHBUTTON+MOUSETRACK;\n    TEXT = \"GUI:{name}\";\n  END\n",
            x + 100,
            y + 20
        )
    }

    /// A panel `MainMenu.wnd:<name>` at x (passing its buttons' messages up), holding `buttons`.
    fn panel(name: &str, x: i32, buttons: &[&str]) -> String {
        let mut s = format!(
            "  WINDOW\n    WINDOWTYPE = USER;\n    SCREENRECT = UPPERLEFT: {x} 100, BOTTOMRIGHT: {} 400, CREATIONRESOLUTION: 800 600;\n    NAME = \"MainMenu.wnd:{name}\";\n    STATUS = ENABLED;\n    STYLE = USER;\n    SYSTEMCALLBACK = \"PassSelectedButtonsToParentSystem\";\n    CHILD\n",
            x + 120
        );
        for (i, b) in buttons.iter().enumerate() {
            s += &button(b, x + 10, 110 + 30 * i as i32, false);
        }
        s + "    ENDALLCHILDREN\n  END\n"
    }

    /// A synthetic `Menus/MainMenu.wnd` (no EA file): the parent with the game's callbacks, the five panels (the main
    /// one at x 0, single player 130, multiplayer 260, load/replay 390, difficulty 520), the campaigns' save buttons,
    /// two faction logos and the ruler.
    pub fn main_menu_wnd() -> Vec<u8> {
        let mut s = String::from(
            "FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = MainMenuInit;\n  LAYOUTUPDATE = MainMenuUpdate;\n  LAYOUTSHUTDOWN = MainMenuShutdown;\nENDLAYOUTBLOCK\n\
WINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;\n\
  NAME = \"MainMenu.wnd:MainMenuParent\";\n  STATUS = ENABLED+HIDDEN;\n  STYLE = USER;\n  SYSTEMCALLBACK = \"MainMenuSystem\";\n  INPUTCALLBACK = \"MainMenuInput\";\n  CHILD\n",
        );
        s += &panel("MapBorder2", 0, &["ButtonSinglePlayer", "ButtonMultiplayer", "ButtonLoadReplay", "ButtonOptions", "ButtonCredits", "ButtonExit", "ButtonWorldBuilder"]);
        s += &panel("MapBorder", 130, &["ButtonUSA", "ButtonGLA", "ButtonChina", "ButtonChallenge", "ButtonSkirmish", "ButtonSingleBack"]);
        s += &panel("MapBorder1", 260, &["ButtonOnline", "ButtonNetwork", "ButtonMultiBack"]);
        s += &panel("MapBorder3", 390, &["ButtonLoadGame", "ButtonReplay", "ButtonLoadReplayBack"]);
        s += &panel("MapBorder4", 520, &["ButtonEasy", "ButtonMedium", "ButtonHard", "ButtonDiffBack"]);
        for (i, b) in ["ButtonUSARecentSave", "ButtonUSALoadGame", "ButtonGLARecentSave", "ButtonGLALoadGame", "ButtonChinaRecentSave", "ButtonChinaLoadGame"].iter().enumerate() {
            s += &button(b, 650, 100 + 25 * i as i32, false);
        }
        for (i, b) in ["WinFactionUS", "WinFactionGLA", "MainMenuRuler", "WinFactionChina", "WinFactionTraining", "WinFactionSkirmish"].iter().enumerate() {
            s += &format!("  WINDOW\n    WINDOWTYPE = USER;\n    SCREENRECT = UPPERLEFT: {} 450, BOTTOMRIGHT: {} 500, CREATIONRESOLUTION: 800 600;\n    NAME = \"MainMenu.wnd:{b}\";\n    STATUS = ENABLED;\n    STYLE = USER;\n  END\n", 10 + 60 * i, 60 + 60 * i);
        }
        s += "  ENDALLCHILDREN\nEND\n";
        s.into_bytes()
    }

    /// A synthetic message box (`Menus/MessageBox.wnd`, or the quit box's with `quit`): the parent with the system
    /// callback, the four buttons hidden at x 10, 110, 210 (Yes at 300), the title and the message.
    pub fn message_box_wnd(quit: bool) -> Vec<u8> {
        let m = if quit { "QuitMessageBox" } else { "MessageBox" };
        let mut s = format!(
            "FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = [None];\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\n\
WINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 200 200, BOTTOMRIGHT: 600 400, CREATIONRESOLUTION: 800 600;\n\
  NAME = \"{m}.wnd:MessageBoxParent\";\n  STATUS = ENABLED+HIDDEN;\n  STYLE = USER;\n  SYSTEMCALLBACK = \"{m}System\";\n  CHILD\n"
        );
        for (b, x) in [("ButtonOk", 10), ("ButtonNo", 110), ("ButtonCancel", 210), ("ButtonYes", 300)] {
            s += &format!("  WINDOW\n    WINDOWTYPE = PUSHBUTTON;\n    SCREENRECT = UPPERLEFT: {} 350, BOTTOMRIGHT: {} 380, CREATIONRESOLUTION: 800 600;\n    NAME = \"{m}.wnd:{b}\";\n    STATUS = ENABLED+HIDDEN;\n    STYLE = PUSHBUTTON;\n  END\n", 200 + x, 280 + x);
        }
        for (t, y) in [("StaticTextTitle", 210), ("StaticTextMessage", 250)] {
            s += &format!("  WINDOW\n    WINDOWTYPE = STATICTEXT;\n    SCREENRECT = UPPERLEFT: 210 {y}, BOTTOMRIGHT: 590 {}, CREATIONRESOLUTION: 800 600;\n    NAME = \"{m}.wnd:{t}\";\n    STATUS = ENABLED;\n    STYLE = STATICTEXT;\n  END\n", y + 30);
        }
        s += "  ENDALLCHILDREN\nEND\n";
        s.into_bytes()
    }

    /// A plain screen `Menus/<Name>.wnd`: one hidden window `<Name>.wnd:Parent` over the whole layout, with the layout
    /// callbacks `<Name>Init`, `<Name>Update`, `<Name>Shutdown`.
    pub fn screen(name: &str) -> Vec<u8> {
        format!(
            "FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = {name}Init;\n  LAYOUTUPDATE = {name}Update;\n  LAYOUTSHUTDOWN = {name}Shutdown;\nENDLAYOUTBLOCK\n\
WINDOW\n  WINDOWTYPE = USER;\n  SCREENRECT = UPPERLEFT: 0 0, BOTTOMRIGHT: 800 600, CREATIONRESOLUTION: 800 600;\n\
  NAME = \"{name}.wnd:Parent\";\n  STATUS = ENABLED+HIDDEN;\n  STYLE = USER;\nEND\n"
        )
        .into_bytes()
    }

    /// The panels of [`main_menu_wnd`] and their buttons.
    const PANELS: [(&str, &[&str]); 5] = [
        ("MapBorder2", &["ButtonSinglePlayer", "ButtonMultiplayer", "ButtonLoadReplay", "ButtonOptions", "ButtonCredits", "ButtonExit", "ButtonWorldBuilder"]),
        ("MapBorder", &["ButtonUSA", "ButtonGLA", "ButtonChina", "ButtonChallenge", "ButtonSkirmish", "ButtonSingleBack"]),
        ("MapBorder1", &["ButtonOnline", "ButtonNetwork", "ButtonMultiBack"]),
        ("MapBorder3", &["ButtonLoadGame", "ButtonReplay", "ButtonLoadReplayBack"]),
        ("MapBorder4", &["ButtonEasy", "ButtonMedium", "ButtonHard", "ButtonDiffBack"]),
    ];

    /// One `WindowTransition` block: a panel (`FLASH`) and its buttons (`BUTTONFLASH`, one frame apart).
    fn panel_group(name: &str, fire_once: bool, panel: usize) -> String {
        let (p, buttons) = PANELS[panel];
        let mut s = format!("WindowTransition {name}\n  FireOnce = {}\n  Window\n    WinName = MainMenu.wnd:{p}\n    Style = FLASH\n    FrameDelay = 0\n  End\n", if fire_once { "Yes" } else { "No" });
        for (i, b) in buttons.iter().enumerate() {
            s += &format!("  Window\n    WinName = MainMenu.wnd:{b}\n    Style = BUTTONFLASH\n    FrameDelay = {}\n  End\n", i + 1);
        }
        s + "End\n"
    }

    /// A synthetic `Data\INI\WindowTransitions.ini` (no EA file) for [`main_menu_wnd`]: every group `MainMenu.cpp`
    /// names. The groups that show a panel are fire-once (so that a hover's group does not reverse them); the `Back`
    /// groups, which the menu reverses to hide a panel, are not; the faction logos scale up (`WINSCALEUP`); the whole
    /// screen fades (`SCREENFADE`), `MainMenuFade` once.
    pub fn window_transitions_ini() -> Vec<u8> {
        let mut s = String::from("; synthetic, for the tests (not the game's file)\n");
        s += "WindowTransition FadeWholeScreen\n  Window\n    WinName = MainMenu.wnd:MainMenuParent\n    Style = SCREENFADE\n  End\nEnd\n";
        s += "WindowTransition MainMenuFade\n  FireOnce = Yes\n  Window\n    WinName = MainMenu.wnd:MainMenuParent\n    Style = SCREENFADE\n  End\nEnd\n";
        for (name, fire_once, panel) in [
            ("MainMenuDefaultMenu", true, 0),
            ("MainMenuDefaultMenuLogoFade", true, 0),
            ("MainMenuDefaultMenuBack", false, 0),
            ("MainMenuSinglePlayerMenu", true, 1),
            ("MainMenuSinglePlayerMenuBack", false, 1),
            ("MainMenuSinglePlayerMenuBackUS", false, 1),
            ("MainMenuSinglePlayerMenuBackGLA", false, 1),
            ("MainMenuSinglePlayerMenuBackChina", false, 1),
            ("MainMenuSinglePlayerMenuBackTraining", false, 1),
            ("MainMenuSinglePlayerMenuBackSkirmish", false, 1),
            ("MainMenuSinglePlayerUSAMenuFromDiff", true, 1),
            ("MainMenuSinglePlayerGLAMenuFromDiff", true, 1),
            ("MainMenuSinglePlayerChinaMenuFromDiff", true, 1),
            ("MainMenuSinglePlayerTrainingMenuFromDiff", true, 1),
            ("MainMenuMultiPlayerMenu", true, 2),
            ("MainMenuMultiPlayerMenuReverse", false, 2),
            ("MainMenuMultiPlayerMenuTransitionToNext", false, 2),
            ("MainMenuLoadReplayMenu", true, 3),
            ("MainMenuLoadReplayMenuBack", false, 3),
            ("MainMenuLoadReplayMenuBackTransition", false, 3),
            ("MainMenuDifficultyMenuUS", true, 4),
            ("MainMenuDifficultyMenuGLA", true, 4),
            ("MainMenuDifficultyMenuChina", true, 4),
            ("MainMenuDifficultyMenuTraining", true, 4),
            ("MainMenuDifficultyMenuBack", false, 4),
            ("MainMenuDifficultyMenuUSBack", false, 4),
            ("MainMenuDifficultyMenuGLABack", false, 4),
            ("MainMenuDifficultyMenuChinaBack", false, 4),
            ("MainMenuDifficultyMenuTrainingBack", false, 4),
        ] {
            s += &panel_group(name, fire_once, panel);
        }
        for (g, w) in [("US", "US"), ("GLA", "GLA"), ("China", "China"), ("Training", "Training"), ("Skirmish", "Skirmish")] {
            s += &format!("WindowTransition MainMenuFaction{g}\n  Window\n    WinName = MainMenu.wnd:WinFaction{w}\n    Style = WINSCALEUP\n  End\nEnd\n");
        }
        s.into_bytes()
    }

    /// The shell's synthetic files by their paths (`Window\Menus/<name>.wnd`): the main menu, the two message boxes,
    /// and plain screens ([`screen`]) for the background and the screens the main menu pushes.
    pub fn shell_wnd_files() -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut out = vec![("MainMenu", main_menu_wnd()), ("MessageBox", message_box_wnd(false)), ("QuitMessageBox", message_box_wnd(true))];
        for n in ["BlankWindow", "SaveLoad", "SkirmishGameOptionsMenu", "ReplayMenu", "CreditsMenu", "LanLobbyMenu", "OptionsMenu", "ChallengeMenu"] {
            out.push((n, screen(n)));
        }
        out.into_iter().map(|(n, d)| (format!("Window\\Menus/{n}.wnd").into_bytes(), d)).collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::ime_manager;
    use crate::client::shell::tests::Files;
    use crate::client::window_manager::{gwm, key, short_to_long};
    use crate::client::window_script::win_status;
    use crate::client::window_transitions as wt;

    /// The synthetic files of the shell ([`synthetic::shell_wnd_files`]).
    pub(crate) fn shell_files() -> Files {
        let mut f = Files::default();
        for (p, d) in synthetic::shell_wnd_files() {
            f.files.insert(p, d);
        }
        f
    }

    fn w(wm: &GameWindowManager, name: &str) -> WinId {
        wm.tree.find_by_name(format!("MainMenu.wnd:{name}").as_bytes()).unwrap_or_else(|| panic!("{name}"))
    }

    fn shown(wm: &GameWindowManager, name: &str) -> bool {
        wm.tree.windows[w(wm, name)].status & win_status::HIDDEN == 0
    }

    /// A click (press and release) at the centre of a window.
    fn click(wm: &mut GameWindowManager, h: &mut ShellGui<Files>, win: WinId) {
        let (x, y) = wm.tree.screen_position(win);
        let (sx, sy) = wm.tree.windows[win].size;
        let p = (x + sx / 2, y + sy / 2);
        wm.win_process_mouse_event(h, gwm::MOUSE_POS, p, (0, 0));
        wm.win_process_mouse_event(h, gwm::LEFT_DOWN, p, (0, 0));
        wm.win_process_mouse_event(h, gwm::LEFT_UP, p, (0, 0));
    }

    /// A frame: the shell's update (40 ms later: each frame runs it) and the window manager's (the transitions').
    fn tick(wm: &mut GameWindowManager, h: &mut ShellGui<Files>) {
        h.time_ms += 40;
        shell::update(wm, h);
        wm.update(h);
    }

    /// (#77) Frames until the transitions are done (no current group running, none pending) and one more, so that
    /// the menu's update sees it; the number of frames.
    fn settle(wm: &mut GameWindowManager, h: &mut ShellGui<Files>) -> usize {
        for n in 0..400 {
            if wt::is_finished(wm) && wm.transitions.pending.is_none() && shell::is_anim_finished(wm, h) {
                tick(wm, h);
                return n + 1;
            }
            tick(wm, h);
        }
        panic!("the transitions never finish: {:?}", wm.transitions.current.map(|c| String::from_utf8_lossy(&wm.transitions.groups[c].name).into_owned()));
    }

    /// The handler's current group's name.
    fn current(wm: &GameWindowManager) -> Option<String> {
        wm.transitions.current.map(|c| String::from_utf8_lossy(&wm.transitions.groups[c].name).into_owned())
    }

    /// The shell from the synthetic files, with the synthetic `WindowTransitions.ini` (#77), started.
    fn booted() -> (GameWindowManager, ShellGui<Files>) {
        let mut wm = GameWindowManager::new((800, 600));
        wt::load_window_transitions(&mut wm, wt::WINDOW_TRANSITIONS_INI, &synthetic::window_transitions_ini()).expect("the synthetic INI reads");
        let mut h = ShellGui::new(shell_files());
        h.time_ms = 1000;
        start_shell(&mut wm, &mut h);
        shell::update(&mut wm, &mut h);
        (wm, h)
    }

    /// #69, the main menu from synthetic WND text, predicted from Shell.cpp and MainMenu.cpp before the run:
    ///
    /// - `showShellMap(TRUE)` (no shell map) loads the background; `showShell()` pushes `Menus/MainMenu.wnd`; its init
    ///   (the first time) hides the five panels and the six save buttons, hides `MainMenuRuler`, shows the layout,
    ///   hides the mouse, reverses `FadeWholeScreen`, and gives `MainMenuParent` the focus;
    /// - a key (`GWM_CHAR` to the focus) shows the main panel (`MapBorder2`) and the mouse; then input is ignored;
    /// - ~~clicking *Single Player* shows `MapBorder` (the main panel stays: the transition that hides it is a gap) and
    ///   sets `dontAllowTransitions`, which the next update clears (transitions finish at once);~~
    ///   **Corrected 2026-11-10 (#77):** with the transitions translated (the synthetic `WindowTransitions.ini` of
    ///   [`synthetic::window_transitions_ini`]), clicking *Single Player* sets `dontAllowTransitions`; the main panel
    ///   (`MainMenuDefaultMenuBack` reversed) hides and the single player panel (`MainMenuSinglePlayerMenu`, pending)
    ///   shows; `dontAllowTransitions` is cleared by the first update after they are done;
    /// - *USA* sets the campaign `USA`; ~~*Medium* (`GBM_SELECTED`, its panel being a transition's)~~ **Corrected
    ///   2026-11-10 (#77):** the difficulty panel `MapBorder4` (`MainMenuDifficultyMenuUS`) shows and the single
    ///   player panel hides; a click on *Medium* with the disc present asks for nothing at once (`startGame`); the
    ///   update once `FadeWholeScreen` (forward) and the animate manager are done asks for `StartCampaign { USA, 1 }`
    ///   and starts the shutdown, and **the same update** completes it (`MainMenuUpdate` tests `isShuttingDown` right
    ///   after `doGameStart` sets it; missed in #69's first run);
    /// - every button press played `GUIClick`-less push buttons: no audio (the file names no sound).
    ///
    /// **#77 predictions (written before the run):** right after the key, `MapBorder2` is **hidden** again
    /// (`MainMenuDefaultMenu`'s init runs its frame 0 backwards, which hides it) and shown once the group has run; the
    /// main panel's buttons play `GUIButtonsFadeIn` and the panel `GUIBoarderFadeIn`. **Result:** held.
    #[test]
    fn the_main_menu_starts_a_campaign_as_upstream() {
        let (mut wm, mut h) = booted();
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert_eq!(h.shell.get_screen_count(), 1);
        assert!(h.shell.background.is_some());
        for p in ["MapBorder", "MapBorder1", "MapBorder2", "MapBorder3", "MapBorder4", "ButtonUSARecentSave", "ButtonChinaLoadGame", "MainMenuRuler", "WinFactionUS"] {
            assert!(!shown(&wm, p), "{p} hidden");
        }
        assert!(shown(&wm, "MainMenuParent"));
        assert!(!h.main_menu.mouse_visible);
        assert_eq!(current(&wm).as_deref(), Some("FadeWholeScreen"));
        assert!(wm.transitions.groups[wm.transitions.current.expect("current")].is_reversed());
        assert_eq!(wm.keyboard_focus, Some(w(&wm, "MainMenuParent")));
        wm.win_process_key(&mut h, key::SPACE, key::STATE_DOWN);
        assert!(!shown(&wm, "MapBorder2"), "MainMenuDefaultMenu's init hides it");
        assert!(h.main_menu.mouse_visible && !h.main_menu.not_shown);
        assert_eq!(current(&wm).as_deref(), Some("MainMenuFade"));
        settle(&mut wm, &mut h);
        assert!(shown(&wm, "MapBorder2") && shown(&wm, "ButtonSinglePlayer"));
        assert!(h.audio.contains(&b"GUIButtonsFadeIn".to_vec()) && h.audio.contains(&b"GUIBoarderFadeIn".to_vec()));
        let b = w(&wm, "ButtonSinglePlayer");
        click(&mut wm, &mut h, b);
        assert!(h.main_menu.dont_allow_transitions);
        assert!(!shown(&wm, "MapBorder"), "MainMenuSinglePlayerMenu's init hides it");
        settle(&mut wm, &mut h);
        assert!(shown(&wm, "MapBorder") && !shown(&wm, "MapBorder2"), "the transitions swap the panels");
        assert!(!h.main_menu.dont_allow_transitions);
        let b = w(&wm, "ButtonUSA");
        click(&mut wm, &mut h, b);
        assert_eq!(h.main_menu.campaign, b"USA");
        assert!(h.main_menu.campaign_selected);
        settle(&mut wm, &mut h);
        assert!(shown(&wm, "MapBorder4") && !shown(&wm, "MapBorder"), "the difficulty panel replaces the single player one");
        let b = w(&wm, "ButtonMedium");
        click(&mut wm, &mut h, b);
        assert!(h.main_menu.start_game);
        assert!(h.requests.is_empty());
        tick(&mut wm, &mut h);
        assert!(h.requests.is_empty(), "FadeWholeScreen is still running");
        settle(&mut wm, &mut h);
        assert_eq!(h.requests, [ShellRequest::StartCampaign { campaign: b"USA".to_vec(), difficulty: 1 }]);
        // (prediction missed in #69: `doGameStart` sets `isShuttingDown` and the same update's next test completes it)
        assert!(!h.main_menu.is_shutting_down);
        let top = h.shell.top().expect("the menu stays on the stack");
        assert!(wm.layouts[top].hidden);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert!(h.untranslated.iter().all(|f| f.starts_with(b"BlankWindow")), "{:?}", h.untranslated);
    }

    /// #69, the other buttons, predicted from MainMenu.cpp before the run:
    ///
    /// - *Load/Replay* then *Load Game*: the menu's shutdown is asked (`Menus/SaveLoad.wnd` pending), and ~~the next
    ///   update~~ **Corrected 2026-11-10 (#77):** the first update once `MainMenuLoadReplayMenuBackTransition` and the
    ///   animate manager are done completes it: two screens, the save/load screen on top (its callbacks are not translated: recorded);
    ///   `buttonPushed` stays set; `popImmediate` takes it off and the main menu's init runs again, which (not the
    ///   first time) shows the ruler and sets `justEntered`; two updates later (`initialGadgetDelay` 2, 1) the focus is
    ///   set again;
    /// - *Exit* (full screen) opens the quit box: modal, its Yes and No shown at the first and third places, the
    ///   title and message from the game text; *No* closes it (no callback); *Exit* again and *Yes*: `quitCallback`
    ///   pops the menu (its shutdown) and asks the engine to quit;
    /// - *Options* shows the options layout (made once); *World Builder* fails to spawn and shows an OK box.
    ///
    /// **#77 (predicted before the run):** the panels wait for their transitions (the test lets them settle before
    /// each click). **Missed:** the quit's pop took two updates, not one: `MainMenuShutdown` reverses the animate
    /// manager (`reverseAnimatewindow`), which is not finished until its own update, run after the screens' updates
    /// in `Shell::update` (upstream's order).
    #[test]
    fn the_main_menu_buttons_reach_the_shell_and_the_boxes() {
        let (mut wm, mut h) = booted();
        wm.win_process_key(&mut h, key::ENTER, key::STATE_DOWN);
        settle(&mut wm, &mut h);
        let b = w(&wm, "ButtonLoadReplay");
        click(&mut wm, &mut h, b);
        settle(&mut wm, &mut h);
        assert!(shown(&wm, "MapBorder3"));
        let b = w(&wm, "ButtonLoadGame");
        click(&mut wm, &mut h, b);
        assert!(h.shell.pending_push && h.main_menu.button_pushed);
        // (#77: the menu's shutdown waits for `MainMenuLoadReplayMenuBackTransition` and the animate manager)
        tick(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 1);
        settle(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 2);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/SaveLoad.wnd");
        assert!(h.untranslated.contains(&b"SaveLoadInit".to_vec()));
        let b = w(&wm, "ButtonOptions");
        click(&mut wm, &mut h, b);
        assert!(h.shell.options_layout.is_none(), "buttonPushed: ignored");
        shell::pop_immediate(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 1);
        assert!(!h.main_menu.button_pushed);
        assert!(shown(&wm, "MainMenuRuler") && h.main_menu.just_entered);
        wm.win_set_focus(&mut h, None);
        tick(&mut wm, &mut h);
        assert_eq!(wm.keyboard_focus, None);
        tick(&mut wm, &mut h);
        assert_eq!(wm.keyboard_focus, Some(w(&wm, "MainMenuParent")));
        assert!(!h.main_menu.just_entered);
        // (#77: `MainMenuDefaultMenuLogoFade` hid the main panel's buttons at its init; it shows them)
        assert_eq!(current(&wm).as_deref(), Some("MainMenuDefaultMenuLogoFade"));
        settle(&mut wm, &mut h);
        // the quit box
        let mm = h.main_menu.parent.expect("parent");
        wm.win_hide(w(&wm, "MapBorder2"), false);
        let b = w(&wm, "ButtonExit");
        click(&mut wm, &mut h, b);
        let qb = wm.modal.first().copied().expect("the quit box is modal");
        let find = |wm: &GameWindowManager, n: &str| wm.tree.find_by_name(format!("QuitMessageBox.wnd:{n}").as_bytes()).expect(n);
        let (yes, no, ok, cancel) = (find(&wm, "ButtonYes"), find(&wm, "ButtonNo"), find(&wm, "ButtonOk"), find(&wm, "ButtonCancel"));
        // (positions are the parent's: Ok's slot 10, Cancel's 210)
        assert_eq!(wm.tree.windows[yes].region.0, 10, "Yes at the first place");
        assert_eq!(wm.tree.windows[no].region.0, 210, "No alone at the third");
        assert!(wm.tree.windows[ok].status & win_status::HIDDEN != 0 && wm.tree.windows[cancel].status & win_status::HIDDEN != 0);
        let title = crate::client::gadget::static_text_get_text(&wm, find(&wm, "StaticTextTitle"));
        assert_eq!(String::from_utf16_lossy(&title), "T:GUI:QuitPopupTitle");
        click(&mut wm, &mut h, no);
        wm.update(&mut h);
        assert!(wm.ext[qb].freed && wm.modal.is_empty());
        assert!(h.requests.is_empty());
        click(&mut wm, &mut h, b);
        let yes = wm.tree.windows.len() - 1 - wm.tree.windows.iter().rev().position(|x| x.name() == b"QuitMessageBox.wnd:ButtonYes").expect("yes");
        click(&mut wm, &mut h, yes);
        assert_eq!(h.requests, [ShellRequest::Quit]);
        assert!(h.ui_interacts.contains(&"SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED"));
        assert!(h.main_menu.is_shutting_down && h.shell.pending_pop);
        // (#77: the shutdown reversed the animate manager, which is finished only after its next update, which runs
        // after the screens' updates: the second update completes the shutdown)
        tick(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 1, "the animate manager is not finished yet");
        tick(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 0, "the menu popped");
        assert!(wm.tree.windows[mm].status & win_status::DESTROYED != 0);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
    }

    /// #69, options, the world builder, the hover and the IME detach, predicted from MainMenu.cpp and Shell.cpp:
    /// *Options* loads `Menus/OptionsMenu.wnd` once and runs its (untranslated) init; *World Builder* cannot spawn, so
    /// an OK box (`GUI:WorldBuilder`) opens; *OK* closes it. The mouse entering *Exit* signals the exit hook; entering
    /// *USA* asks for its logo's transition. A text entry attached to the IME is detached by the shell's push.
    #[test]
    fn options_world_builder_and_hover_follow_upstream() {
        let (mut wm, mut h) = booted();
        wm.win_process_key(&mut h, key::ENTER, key::STATE_DOWN);
        settle(&mut wm, &mut h);
        let b = w(&wm, "ButtonOptions");
        click(&mut wm, &mut h, b);
        let o = h.shell.options_layout.expect("options");
        assert!(!wm.layouts[o].hidden);
        assert!(h.untranslated.contains(&b"OptionsMenuInit".to_vec()));
        // (the options screen covers the menu: hidden, as its own Back would)
        wm.layout_hide(o, true);
        tick(&mut wm, &mut h);
        let b = w(&wm, "ButtonWorldBuilder");
        click(&mut wm, &mut h, b);
        let ok = wm.tree.find_by_name(b"MessageBox.wnd:ButtonOk").expect("ok box");
        let t = crate::client::gadget::static_text_get_text(&wm, wm.tree.find_by_name(b"MessageBox.wnd:StaticTextMessage").expect("msg"));
        assert_eq!(String::from_utf16_lossy(&t), "T:GUI:WorldBuilderLoadFailed");
        assert!(wm.tree.windows[ok].status & win_status::HIDDEN == 0);
        click(&mut wm, &mut h, ok);
        assert!(wm.modal.is_empty());
        settle(&mut wm, &mut h);
        let e = w(&wm, "ButtonExit");
        let (x, y) = wm.tree.screen_position(e);
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (x + 5, y + 5), (0, 0));
        assert!(h.ui_interacts.contains(&"SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_HIGHLIGHTED"));
        wm.win_hide(w(&wm, "MapBorder"), false);
        let u = w(&wm, "ButtonUSA");
        let (x, y) = wm.tree.screen_position(u);
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (x + 5, y + 5), (0, 0));
        assert!(h.ui_interacts.contains(&"SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_UNHIGHLIGHTED"));
        assert_eq!(current(&wm).as_deref(), Some("MainMenuFactionUS"));
        assert!(!wm.transitions.groups[wm.transitions.current.expect("current")].is_reversed());
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (790, 590), (0, 0));
        assert_eq!(current(&wm).as_deref(), Some("MainMenuFactionUS"));
        assert!(wm.transitions.groups[wm.transitions.current.expect("current")].is_reversed());
        wm.ime.attach(Some(0));
        shell::push(&mut wm, &mut h, b"Menus/CreditsMenu.wnd", true);
        tick(&mut wm, &mut h);
        assert_eq!(wm.ime.get_window(), None);
        assert_eq!(ime_manager::type_text(&mut wm, &mut h, "x"), 0);
        // a mouse position of (0, 0) and the first position seen: as upstream
        let mut m = MainMenu::default();
        std::mem::swap(&mut m, &mut h.main_menu);
        h.main_menu.drop_down_windows = m.drop_down_windows;
        let p = h.main_menu.parent.unwrap_or(0);
        assert_eq!(main_menu::main_menu_input(&mut wm, &mut h, p, gwm::MOUSE_POS, 0, 0), MsgHandled::Ignored);
        assert_eq!(main_menu::main_menu_input(&mut wm, &mut h, p, gwm::MOUSE_POS, short_to_long(100, 100), 0), MsgHandled::Ignored);
        assert_eq!(main_menu::main_menu_input(&mut wm, &mut h, p, gwm::MOUSE_POS, short_to_long(115, 110), 0), MsgHandled::Ignored);
        assert_eq!(main_menu::main_menu_input(&mut wm, &mut h, p, gwm::MOUSE_POS, short_to_long(121, 100), 0), MsgHandled::Handled);
        assert_eq!(main_menu::main_menu_input(&mut wm, &mut h, p, gwm::CHAR, 0, 0), MsgHandled::Ignored);
    }
}
