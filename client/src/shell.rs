// SPDX-License-Identifier: GPL-3.0-only
//! The game's shell from its files (#69, part 1): the main menu (`Window\Menus\MainMenu.wnd`) and its message boxes,
//! run by the translated shell (`rts_lockstep_engine::client::shell`, `main_menu`, `message_box`, `shell_gui`) in a
//! window manager of their own. Plain Rust, no Bevy.
//!
//! - [`ShellUi::load`] reads the shell's WND files from the art's file system (the owner's files; never shipped) and
//!   starts the shell as the game does after its intro (`showShellMap(TRUE)`, `showShell()`): the main menu is pushed.
//! - [`ShellUi::pointer`], [`ShellUi::keyboard`] and [`ShellUi::frame`] give it the mouse (a touch is a mouse), the
//!   keyboard (`crate::ui::keyboard_into`: keys and typed characters) and the clock (`Shell::update`).
//! - [`act`] turns what the callbacks asked of the engine into the client's own calls: a campaign start goes to the
//!   same [`Flow::choose`] the campaign panel uses (the campaign's first mission, as the main menu starts it: the World's
//!   `start_campaign`), a quit is the application's.
//!
//! **Not drawn yet (#69's remaining work):** the application does not show this shell; the windows' quads, as
//! `crate::ui::GameUi::frame` makes the control bar's, and the phone layout (the menu over the 3D view, the zoom
//! column kept) are the next step. ~~The transitions (`WindowTransitions.ini`) that show the difficulty panel are not
//! translated, so the campaign's path is reachable only through its messages (the tests' path) until they are.~~
//! **Corrected 2026-11-10 (#77):** the transitions are translated (`client::window_transitions`): [`ShellUi::load`]
//! reads `Data\INI\WindowTransitions.ini` from the owner's files (the INI archive), and the panels, the difficulty
//! panel among them, show and hide as upstream's groups make them; the campaign's path is clicked through.
//!
//! The client never changes the simulation: the shell reads nothing of the `World`, and a start is the campaign
//! panel's own start (a copy of the install's World).

use std::collections::BTreeMap;

use rts_lockstep_engine::client::shell::{self as sh, ShellRequest, WndFiles};
use rts_lockstep_engine::client::shell_gui::{start_shell, ShellGui};
use rts_lockstep_engine::client::window_manager::{gwm, GameWindowManager};
use rts_lockstep_engine::client::window_script::WndHost;
use rts_lockstep_engine::client::window_transitions as wt;
use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice};
use rts_lockstep_engine::common::science::GameTextFetch;
use rts_lockstep_engine::logic::world::campaign_world::MissionChoice;
use rts_lockstep_engine::logic::world::game_data::TextTable;
use rts_lockstep_engine::logic::world::game_logic::World;

use crate::campaigns::Flow;
use crate::ui::LAYOUT;

/// The shell's WND files (`Window\Menus\<name>.wnd`) read when the shell is made: the main menu, its boxes, the
/// background, and the screens the main menu pushes (their callbacks are not translated yet; they load and show).
pub const SHELL_FILES: [&str; 11] = ["MainMenu", "MessageBox", "QuitMessageBox", "BlankWindow", "SaveLoad", "SkirmishGameOptionsMenu", "ReplayMenu", "CreditsMenu", "LanLobbyMenu", "OptionsMenu", "ChallengeMenu"];

/// The files the shell reads, read ahead (the art's file system is not kept by the shell), with the reader's name keys
/// and the game's text.
pub struct MenuFiles {
    pub files: BTreeMap<Vec<u8>, Vec<u8>>,
    pub keys: BTreeMap<Vec<u8>, u32>,
    pub text: TextTable,
}

impl WndHost for MenuFiles {
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        let n = self.keys.len() as u32 + 1;
        *self.keys.entry(name.to_vec()).or_insert(n)
    }
    fn find_image(&self, _name: &[u8]) -> Option<Vec<u8>> {
        // (the mapped images are the control bar's to read; the shell's windows are drawn by colour until it is drawn)
        None
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        self.text.fetch(label).str().to_vec()
    }
    fn display_size(&self) -> (i32, i32) {
        LAYOUT
    }
}

impl WndFiles for MenuFiles {
    fn open_wnd(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        // `Window\Menus/MainMenu.wnd`: the file system's paths use `\`
        let p: Vec<u8> = path.iter().map(|&b| if b == b'/' { b'\\' } else { b }).collect();
        self.files.iter().find(|(k, _)| k.eq_ignore_ascii_case(&p)).map(|(_, v)| v.clone())
    }
}

/// What the client does for a request of the shell's callbacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellAction {
    /// Nothing to do here (said why).
    None(&'static str),
    /// A campaign's first mission waits for its map in the flow (the campaign panel's path).
    CampaignChosen,
    /// The start failed (the campaign has no such mission, ...).
    Failed(String),
    /// The application should end (`TheGameEngine->setQuitting(TRUE)`).
    Quit,
}

/// The client's side of a request: [`ShellRequest::StartCampaign`] is the campaign panel's start of the campaign's
/// first mission ([`Flow::choose`] in a copy of `base`, `MissionChoice::Campaign` with `FirstMission`: the World's
/// `start_campaign`, `setCampaign` + `prepareCampaignGame` + `doGameStart`), at the menu's difficulty.
pub fn act(r: &ShellRequest, base: &World, flow: &mut Flow) -> ShellAction {
    match r {
        ShellRequest::StartCampaign { campaign, difficulty } => {
            let lower = campaign.to_ascii_lowercase();
            let Some(c) = base.campaign.campaign_list.iter().find(|c| c.name == lower) else {
                return ShellAction::Failed(format!("no campaign {:?}", String::from_utf8_lossy(campaign)));
            };
            let choice = MissionChoice::Campaign { campaign: c.name.clone(), mission: c.first_mission.to_ascii_lowercase() };
            match flow.choose(base, choice, *difficulty) {
                Ok(()) => ShellAction::CampaignChosen,
                Err(e) => ShellAction::Failed(e),
            }
        }
        ShellRequest::Quit => ShellAction::Quit,
        ShellRequest::ClearGameData => ShellAction::None("no game runs under the shell (the client's games are started by the flow)"),
        ShellRequest::NewShellGame => ShellAction::None("the shell map is not translated"),
    }
}

/// The shell: its window manager and its host.
pub struct ShellUi {
    pub wm: GameWindowManager,
    pub gui: ShellGui<MenuFiles>,
}

impl ShellUi {
    /// The shell from the art's files, started (the main menu pushed), or `None` without `Menus\MainMenu.wnd` or
    /// (#77) `Data\INI\WindowTransitions.ini` (or with one that does not read). The time is the client's clock in
    /// milliseconds.
    pub fn load<D: LocalDevice>(fs: &FileSystem<D>, text: &TextTable, now_ms: u32) -> Option<ShellUi> {
        let mut files = BTreeMap::new();
        for n in SHELL_FILES {
            let p = format!("Window\\Menus\\{n}.wnd").into_bytes();
            if let Some(d) = fs.open_file(&p) {
                files.insert(p, d);
            }
        }
        if !files.contains_key(&b"Window\\Menus\\MainMenu.wnd"[..]) {
            return None;
        }
        // (#77) `TheTransitionHandler->load()` at the window manager's init: without the file upstream's INI load
        // throws; here there is no shell
        let transitions = fs.open_file(wt::WINDOW_TRANSITIONS_INI)?;
        let mut wm = GameWindowManager::new(LAYOUT);
        wt::load_window_transitions(&mut wm, wt::WINDOW_TRANSITIONS_INI, &transitions).ok()?;
        let mut ui = ShellUi { wm, gui: ShellGui::new(MenuFiles { files, keys: BTreeMap::new(), text: text.clone() }) };
        ui.gui.time_ms = now_ms;
        start_shell(&mut ui.wm, &mut ui.gui);
        Some(ui)
    }

    /// A frame at `now_ms`: the shell's update (every screen's, at most 30 a second) and the window manager's
    /// (`processDestroyList`, then (#77) the transitions' update).
    pub fn frame(&mut self, now_ms: u32) {
        self.gui.time_ms = now_ms;
        self.wm.time_ms = now_ms;
        sh::update(&mut self.wm, &mut self.gui);
        self.wm.update(&mut self.gui);
    }

    /// The pointer at `pos` (the layout's pixels) through `winProcessMouseEvent`: `msg` is `GWM_MOUSE_POS`,
    /// `GWM_LEFT_DOWN` or `GWM_LEFT_UP`; a touch's press moves the pointer there first.
    pub fn pointer(&mut self, msg: u32, pos: (f32, f32), touch: bool) {
        let p = (pos.0.floor() as i32, pos.1.floor() as i32);
        if msg == gwm::LEFT_DOWN && touch {
            self.wm.win_process_mouse_event(&mut self.gui, gwm::MOUSE_POS, p, (0, 0));
        }
        self.wm.win_process_mouse_event(&mut self.gui, msg, p, (0, 0));
    }

    /// A key and its text ([`crate::ui::keyboard_into`]); whether the shell used it.
    pub fn keyboard(&mut self, scan: u8, down: bool, text: Option<&str>) -> bool {
        crate::ui::keyboard_into(&mut self.wm, &mut self.gui, scan, down, text)
    }

    /// The callbacks' requests since the last call, acted on ([`act`]).
    pub fn take_actions(&mut self, base: &World, flow: &mut Flow) -> Vec<ShellAction> {
        std::mem::take(&mut self.gui.requests).iter().map(|r| act(r, base, flow)).collect()
    }

    /// The file the top screen was read from.
    pub fn top_screen(&self) -> Option<Vec<u8>> {
        self.gui.shell.top().map(|l| self.wm.layouts[l].filename.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_lockstep_engine::client::shell_gui::synthetic::{shell_wnd_files, window_transitions_ini};
    use rts_lockstep_engine::common::file_system::MemoryDevice;

    const CAMPAIGN: &[u8] = b"Campaign USA\n FirstMission = M01\n Mission M01\n  Map = Maps\\USA01\\USA01.map\n  NextMission = M02\n End\n Mission M02\n  Map = Maps\\USA02\\USA02.map\n End\nEnd\n";

    fn shell() -> ShellUi {
        let mut d = MemoryDevice::new();
        for (p, b) in shell_wnd_files() {
            let p: Vec<u8> = p.iter().map(|&c| if c == b'/' { b'\\' } else { c }).collect();
            d.insert(&p, b);
        }
        d.insert(b"Data\\INI\\WindowTransitions.ini", window_transitions_ini());
        let mut fs = FileSystem::new(d);
        fs.init(None);
        ShellUi::load(&fs, &TextTable::default(), 1000).expect("the shell loads")
    }

    /// (#77) Frames 40 ms apart until the transitions are done, and one more (the menu's update sees it).
    fn settle(ui: &mut ShellUi, now: &mut u32) {
        for _ in 0..400 {
            *now += 40;
            let done = wt::is_finished(&ui.wm) && ui.wm.transitions.pending.is_none();
            ui.frame(*now);
            if done {
                return;
            }
        }
        panic!("the transitions never finish");
    }

    fn win(ui: &ShellUi, name: &str) -> usize {
        ui.wm.tree.find_by_name(format!("MainMenu.wnd:{name}").as_bytes()).expect(name)
    }

    fn click(ui: &mut ShellUi, name: &str, touch: bool) {
        let w = win(ui, name);
        let (x, y) = ui.wm.tree.screen_position(w);
        let p = (x as f32 + 5.0, y as f32 + 5.0);
        ui.pointer(gwm::MOUSE_POS, p, touch);
        ui.pointer(gwm::LEFT_DOWN, p, touch);
        ui.pointer(gwm::LEFT_UP, p, touch);
    }

    /// #69: the shell from synthetic WND files (no EA file) in the client. Predicted from MainMenu.cpp before the run:
    ///
    /// - no `MainMenu.wnd`: no shell; with it, the main menu is the top screen after the start;
    /// - a key (scan 0x39, space) shows the main panel (`MapBorder2`); the keyboard is used;
    /// - touches on *Single Player* then *USA* (frames between them clear `dontAllowTransitions`; the shell's first
    ///   update only starts its clock), then *Medium*
    ///   ~~(`GBM_SELECTED`, its panel being a transition's)~~ **Corrected 2026-11-10 (#77):** touched too, once the
    ///   transitions (the synthetic `WindowTransitions.ini`) have shown the difficulty panel; without the INI there is
    ///   no shell; after the frames that finish `FadeWholeScreen`: one request, `StartCampaign { USA, 1 }`, which [`act`] turns
    ///   into the flow's choice of `usa`'s first mission `m01` on `Maps\USA01\USA01.map` at difficulty 1, waiting for
    ///   its map;
    /// - a campaign the World lacks fails; a quit is the application's; the shell map's requests do nothing;
    /// - the World's CRC-relevant state is not touched: the base World is only read (`flow.choose` copies it).
    #[test]
    fn the_main_menu_starts_the_campaign_panels_flow() {
        let mut d = MemoryDevice::new();
        d.insert(b"Window\\Menus\\MessageBox.wnd", Vec::new());
        let mut fs = FileSystem::new(d);
        fs.init(None);
        assert!(ShellUi::load(&fs, &TextTable::default(), 0).is_none());
        let mut d = MemoryDevice::new();
        for (p, b) in shell_wnd_files() {
            let p: Vec<u8> = p.iter().map(|&c| if c == b'/' { b'\\' } else { c }).collect();
            d.insert(&p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        assert!(ShellUi::load(&fs, &TextTable::default(), 0).is_none(), "no WindowTransitions.ini: no shell");
        let mut ui = shell();
        // (the shell's first update only starts its clock: `static Int lastUpdate`; missed in the first run, where the
        // first frame came after the click and the USA press was refused)
        ui.frame(1000);
        assert_eq!(ui.top_screen().as_deref(), Some(&b"Menus/MainMenu.wnd"[..]));
        assert!(ui.keyboard(0x39, true, Some(" ")));
        // (#77: the main panel's group hides it at its init and shows it over its frames)
        let mut now = 1000;
        settle(&mut ui, &mut now);
        assert!(!ui.wm.tree.windows[win(&ui, "MapBorder2")].is_hidden());
        click(&mut ui, "ButtonSinglePlayer", true);
        settle(&mut ui, &mut now);
        assert!(ui.wm.tree.windows[win(&ui, "MapBorder2")].is_hidden(), "the transitions hid the main panel");
        click(&mut ui, "ButtonUSA", true);
        settle(&mut ui, &mut now);
        assert!(!ui.wm.tree.windows[win(&ui, "MapBorder4")].is_hidden(), "the difficulty panel is shown");
        click(&mut ui, "ButtonMedium", true);
        assert!(ui.gui.requests.is_empty(), "the screen fades first");
        settle(&mut ui, &mut now);
        assert_eq!(ui.gui.requests, [ShellRequest::StartCampaign { campaign: b"USA".to_vec(), difficulty: 1 }]);
        let mut base = World::new();
        base.campaign.init(CAMPAIGN).expect("Campaign.ini");
        let before = base.campaign.clone();
        let mut flow = Flow::default();
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::CampaignChosen]);
        assert_eq!(flow.waiting_for(), Some(&b"Maps\\USA01\\USA01.map"[..]));
        let p = flow.pending.as_ref().expect("pending");
        assert_eq!(p.choice, MissionChoice::Campaign { campaign: b"usa".to_vec(), mission: b"m01".to_vec() });
        assert_eq!(p.difficulty, 1);
        assert_eq!(base.campaign, before, "the base World is only read");
        assert!(matches!(act(&ShellRequest::StartCampaign { campaign: b"GLA".to_vec(), difficulty: 0 }, &base, &mut flow), ShellAction::Failed(_)));
        assert_eq!(act(&ShellRequest::Quit, &base, &mut flow), ShellAction::Quit);
        assert!(matches!(act(&ShellRequest::NewShellGame, &base, &mut flow), ShellAction::None(_)));
        assert!(matches!(act(&ShellRequest::ClearGameData, &base, &mut flow), ShellAction::None(_)));
        assert!(ui.take_actions(&base, &mut flow).is_empty());
    }
}
