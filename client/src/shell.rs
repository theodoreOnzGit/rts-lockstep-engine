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
//! ~~**Not drawn yet (#69's remaining work):** the application does not show this shell; the windows' quads, as
//! `crate::ui::GameUi::frame` makes the control bar's, and the phone layout (the menu over the 3D view, the zoom
//! column kept) are the next step.~~ **Corrected 2026-11-10 (#78):** the application shows it (`app::shell_ui`):
//! [`ShellUi::draw`] is the window manager's `winRepaint` draw list (the transitions' draws included) as the quads the
//! control bar is drawn with ([`crate::ui::draw_list_quads`]), placed by [`shell_placement`] (the menu over the 3D view,
//! the zoom column kept clear); the windows' mapped images are the control bar's collection, shared
//! ([`MenuFiles::images`]).
//! ~~The transitions (`WindowTransitions.ini`) that show the difficulty
//! panel are not translated, so the campaign's path is reachable only through its messages (the tests' path) until they
//! are.~~ **Corrected 2026-11-10 (#77):** the transitions are translated (`client::window_transitions`):
//! [`ShellUi::load`] reads `Data\INI\WindowTransitions.ini` from the owner's files (the INI archive), and the panels,
//! the difficulty panel among them, show and hide as upstream's groups make them; the campaign's path is clicked
//! through.
//!
//! # As upstream reads it
//!
//! `GameClient::update` shows the shell after the intro (`showShellMap(TRUE)`, `showShell()`); the window manager draws
//! every window each frame (`winRepaint`, then `TheTransitionHandler->draw()`), at the display's resolution, the WND
//! layouts scaled from their creation resolution; the mapped images are one collection (`TheMappedImageCollection`) for
//! every window. A game's start hides the shell (`GameLogic::startNewGame` calls `TheShell->hideShell()`); the control
//! bar is shown in the game, not in the shell. `TheGameEngine->setQuitting(TRUE)` ends the program.
//!
//! # Upstream quirks that are kept
//!
//! - `hideShell` runs the top screen's shutdown (immediate) and leaves it on the stack.
//! - A window's text is drawn where its draw placed it (from the top left of the measured string), not re-centred.
//!
//! # Where this differs from upstream
//!
//! - The shell has its own window manager (the control bar has another): it is drawn and takes the pointer and keyboard
//!   only while it is active ([`ShellUi::is_active`]); upstream's one manager keeps drawing what the shutdowns left
//!   shown.
//! - The layout stays 800x600 (`LAYOUT`) and is scaled whole onto the screen ([`shell_placement`]), not re-made at the
//!   display's resolution: on a phone the menu is fitted above the zoom column (mobile first). The synthetic menu's
//!   20-pixel buttons are 9.75 CSS pixels high at 390 wide (reachable by a tap, small; the real WND's sizes are the
//!   owner's files' and not checked).
//! - On the web, a quit cannot close the page: it says so in the status line.
//! - The shell's sounds (`TheAudio->addAudioEvent`) and the script hooks (`signalUIInteract`) are not played or run; the
//!   app clears them each frame.
//!
//! # Representation
//!
//! [`ShellUi`] is the window manager and the shell's host ([`ShellGui`] over [`MenuFiles`], the WND files read ahead
//! with the game's text and the shared images). The app keeps it across games, shows it over the install's game
//! (paused), and hides it when another game starts (a mission, a loaded save).
//!
//! The client never changes the simulation: the shell reads nothing of the `World`, and a start is the campaign
//! panel's own start (a copy of the install's World).

use std::collections::BTreeMap;
use std::sync::Arc;

use rts_lockstep_engine::client::shell::{self as sh, ShellRequest, WndFiles};
use rts_lockstep_engine::client::image::ImageCollection;
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
use crate::ui::{LayoutPlacement, UiQuad, LAYOUT};

/// The shell's WND files (`Window\Menus\<name>.wnd`) read when the shell is made: the main menu, its boxes, the
/// background, and the screens the main menu pushes (their callbacks are not translated yet; they load and show).
pub const SHELL_FILES: [&str; 11] = ["MainMenu", "MessageBox", "QuitMessageBox", "BlankWindow", "SaveLoad", "SkirmishGameOptionsMenu", "ReplayMenu", "CreditsMenu", "LanLobbyMenu", "OptionsMenu", "ChallengeMenu"];

/// The files the shell reads, read ahead (the art's file system is not kept by the shell), with the reader's name keys
/// and the game's text.
pub struct MenuFiles {
    pub files: BTreeMap<Vec<u8>, Vec<u8>>,
    pub keys: BTreeMap<Vec<u8>, u32>,
    pub text: TextTable,
    /// (#78) The mapped images, shared with the control bar (`crate::ui::load_mapped_images`): upstream's one
    /// `TheMappedImageCollection`.
    pub images: Arc<ImageCollection>,
}

impl WndHost for MenuFiles {
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        let n = self.keys.len() as u32 + 1;
        *self.keys.entry(name.to_vec()).or_insert(n)
    }
    fn find_image(&self, name: &[u8]) -> Option<Vec<u8>> {
        // (#78) `TheMappedImageCollection->findImageByName(name)`, the collection the control bar reads too
        self.images.find_image_by_name(name).map(|i| i.name.clone())
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
    ///
    /// (#78) `images` is the mapped image collection the control bar shares (`crate::ui::load_mapped_images`).
    pub fn load<D: LocalDevice>(fs: &FileSystem<D>, text: &TextTable, images: Arc<ImageCollection>, now_ms: u32) -> Option<ShellUi> {
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
        let mut ui = ShellUi { wm, gui: ShellGui::new(MenuFiles { files, keys: BTreeMap::new(), text: text.clone(), images }) };
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

    /// (#78) `TheShell->isShellActive()`: the shell is shown (it is drawn and takes the input).
    pub fn is_active(&self) -> bool {
        self.gui.shell.is_shell_active()
    }

    /// (#78) `TheShell->hideShell()`, as `GameLogic::startNewGame` calls it when a game starts: the top screen's shutdown
    /// (immediate), the shell no longer active. The screens stay on the stack.
    pub fn hide(&mut self) {
        sh::hide_shell(&mut self.wm, &mut self.gui);
    }

    /// (#78) What to draw this frame: `winRepaint`'s draw list (the windows, then the transitions' draws) as the quads
    /// the app draws the control bar with ([`crate::ui::draw_list_quads`]); nothing while the shell is not active.
    pub fn draw(&mut self) -> Vec<UiQuad> {
        if !self.is_active() {
            return Vec::new();
        }
        let items = self.wm.win_repaint(&mut self.gui);
        crate::ui::draw_list_quads(&items, &self.gui)
    }

    /// (#78) A pointer event at a screen point (logical pixels), into the layout through `placement`
    /// ([`LayoutPlacement::to_layout`]) and on to [`ShellUi::pointer`]; nothing while the shell is not active.
    pub fn pointer_at(&mut self, msg: u32, screen: (f32, f32), placement: &LayoutPlacement, touch: bool) {
        if self.is_active() {
            self.pointer(msg, placement.to_layout(screen), touch);
        }
    }
}

/// The top of the area the shell may cover: below the status line's first line (as the radar's box keeps it).
const STATUS_TOP: f32 = 36.0;
/// The zoom column's width and its margins (`app::setup_ui`: 48-pixel buttons, 8 from the right).
const ZOOM_COLUMN_WIDTH: f32 = 48.0 + 8.0 + 8.0;

/// (#78) Where the shell's layout (800x600, the menus' creation resolution) sits on a screen `screen` logical pixels in
/// size, over the 3D view, scaled to fit whole (mobile first): the larger of two places, centred in it, both below the
/// status line's first line and clear of the zoom column (`app::setup_ui`) so that every button the menu shows can be
/// tapped: the full width above the column (a phone held upright), or the full height left of it (a wide screen).
/// New: upstream's menus are made at the display's resolution.
pub fn shell_placement(screen: (f32, f32)) -> LayoutPlacement {
    let (lw, lh) = (LAYOUT.0 as f32, LAYOUT.1 as f32);
    let above = (screen.0, screen.1 - crate::ui::ZOOM_COLUMN_HEIGHT - 8.0 - STATUS_TOP);
    let beside = (screen.0 - ZOOM_COLUMN_WIDTH, screen.1 - 8.0 - STATUS_TOP);
    let fit = |a: (f32, f32)| (a.0 / lw).min(a.1 / lh).max(0.05);
    let (area, scale) = if fit(above) >= fit(beside) { (above, fit(above)) } else { (beside, fit(beside)) };
    LayoutPlacement { left: ((area.0 - lw * scale) / 2.0).max(0.0), top: STATUS_TOP + ((area.1 - lh * scale) / 2.0).max(0.0), y0: 0.0, scale }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_lockstep_engine::client::shell_gui::synthetic::{shell_wnd_files, window_transitions_ini};
    use rts_lockstep_engine::common::file_system::MemoryDevice;

    const CAMPAIGN: &[u8] = b"Campaign USA\n FirstMission = M01\n Mission M01\n  Map = Maps\\USA01\\USA01.map\n  NextMission = M02\n End\n Mission M02\n  Map = Maps\\USA02\\USA02.map\n End\nEnd\n";

    fn shell() -> ShellUi {
        shell_with(&TextTable::default()).0
    }

    /// The shell from the synthetic WND and INI, with (#78) the synthetic control bar's files (its mapped images) on the
    /// same file system, and the mapped images read as the app reads them ([`crate::ui::load_mapped_images`]).
    fn shell_with(text: &TextTable) -> (ShellUi, Arc<ImageCollection>, FileSystem<MemoryDevice>) {
        let mut d = MemoryDevice::new();
        for (p, b) in shell_wnd_files() {
            let p: Vec<u8> = p.iter().map(|&c| if c == b'/' { b'\\' } else { c }).collect();
            d.insert(&p, b);
        }
        d.insert(b"Data\\INI\\WindowTransitions.ini", window_transitions_ini());
        for (p, b) in crate::ui::synthetic_ui_files() {
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let crate::ui::ImagesBuild::Ready(images) = crate::ui::load_mapped_images(&fs) else { panic!("the mapped images load") };
        let ui = ShellUi::load(&fs, text, images.clone(), 1000).expect("the shell loads");
        (ui, images, fs)
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
        assert!(ShellUi::load(&fs, &TextTable::default(), Arc::new(ImageCollection::new()), 0).is_none());
        let mut d = MemoryDevice::new();
        for (p, b) in shell_wnd_files() {
            let p: Vec<u8> = p.iter().map(|&c| if c == b'/' { b'\\' } else { c }).collect();
            d.insert(&p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        assert!(ShellUi::load(&fs, &TextTable::default(), Arc::new(ImageCollection::new()), 0).is_none(), "no WindowTransitions.ini: no shell");
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

    /// #78: the shell drawn from the window manager's draw list, and hidden when a game starts. Predicted from
    /// `winRepaint` and `W3DGadgetPushButtonDraw` before the run:
    ///
    /// - once the main panel is shown, the quads hold each of its 7 buttons' text (`GUI:ButtonSinglePlayer` given the
    ///   text "Single Player" here), placed from the button's top left inward, inside its rectangle (10..110, 110..130);
    /// - no quad of the hidden panels' buttons (`ButtonUSA`'s text);
    /// - `hideShell`: not active, the main menu's shutdown run, nothing drawn, the pointer ignored.
    #[test]
    fn the_shell_draws_from_the_window_managers_draw_list() {
        let mut text = TextTable::default();
        for (l, t) in [("GUI:ButtonSinglePlayer", "Single Player"), ("GUI:ButtonUSA", "USA")] {
            text.strings.insert(l.as_bytes().to_vec(), rts_lockstep_engine::common::unicode_string::UnicodeString::try_from_chars(&t.encode_utf16().collect::<Vec<_>>()).expect("text"));
        }
        let (mut ui, _, _) = shell_with(&text);
        ui.frame(1000);
        assert!(ui.keyboard(0x39, true, Some(" ")));
        let mut now = 1000;
        settle(&mut ui, &mut now);
        let quads = ui.draw();
        assert!(!quads.is_empty());
        let sp: Vec<&UiQuad> = quads.iter().filter(|q| q.text.as_deref() == Some("Single Player")).collect();
        assert_eq!(sp.len(), 1, "{quads:?}");
        let r = sp[0].rect;
        println!("Single Player's text quad: {r:?}, font {:?}, colour {:?}", sp[0].font_px, sp[0].text_color);
        assert!(sp[0].text_top_left);
        assert!(r[0] >= 10.0 && r[2] <= 110.0 && r[1] >= 110.0 && r[3] <= 130.0, "{r:?}");
        assert!(!quads.iter().any(|q| q.text.as_deref() == Some("USA")), "the single player panel is hidden");
        assert!(ui.is_active());
        ui.hide();
        assert!(!ui.is_active());
        assert!(ui.draw().is_empty());
        let p = shell_placement((800.0, 600.0));
        let at = p.to_screen([20.0, 115.0, 20.0, 115.0]);
        ui.pointer_at(gwm::LEFT_DOWN, (at[0], at[1]), &p, true);
        assert!(ui.wm.grab_window.is_none(), "a hidden shell takes no pointer");
    }

    /// #78 (mobile first): the menu on a phone 390 CSS pixels wide (844 high), and on a wide screen. Predicted from
    /// [`shell_placement`]'s arithmetic before the run:
    ///
    /// - 390x844: the full width above the zoom column wins (scale 390 / 800 = 0.4875 against (390 - 64) / 800 =
    ///   0.4075 beside it); the menu 390 x 292.5 at left 0, top 36 + (474 - 292.5) / 2 = 126.75;
    /// - 1280x720: beside the column wins (scale min(1216 / 800, 676 / 600) = 1.1267); left (1216 - 901.3) / 2 = 157.3,
    ///   top 36;
    /// - at 390x844 every button of the main panel is on the screen, below the status line's first line (y 36) and
    ///   above the zoom column (y 518), and a tap at its centre on the screen is that button's (`getWindowUnderCursor`);
    ///   a tap on *Single Player* through the screen shows the single player panel, as a click does. Each button is
    ///   48.75 x 9.75 CSS pixels (the synthetic layout's 100 x 20).
    #[test]
    fn the_menu_fits_a_phone_and_every_button_is_tapped() {
        let p = shell_placement((390.0, 844.0));
        assert_eq!((p.left, p.top, p.y0, p.scale), (0.0, 126.75, 0.0, 0.4875));
        let d = shell_placement((1280.0, 720.0));
        assert!((d.scale - 676.0 / 600.0).abs() < 1e-5 && (d.left - (1216.0 - 800.0 * 676.0 / 600.0) / 2.0).abs() < 1e-3 && d.top == 36.0, "{d:?}");
        let mut ui = shell();
        ui.frame(1000);
        assert!(ui.keyboard(0x39, true, Some(" ")));
        let mut now = 1000;
        settle(&mut ui, &mut now);
        let column_top = 844.0 - crate::ui::ZOOM_COLUMN_HEIGHT;
        for name in ["ButtonSinglePlayer", "ButtonMultiplayer", "ButtonLoadReplay", "ButtonOptions", "ButtonCredits", "ButtonExit", "ButtonWorldBuilder"] {
            let w = win(&ui, name);
            let (x, y) = ui.wm.tree.screen_position(w);
            let reg = ui.wm.tree.windows[w].region;
            let r = p.to_screen([x as f32, y as f32, (x + reg.2 - reg.0) as f32, (y + reg.3 - reg.1) as f32]);
            assert!(r[0] >= 0.0 && r[2] <= 390.0 && r[1] >= 36.0 && r[3] <= column_top, "{name}: {r:?}");
            let centre = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
            let (lx, ly) = p.to_layout(centre);
            assert_eq!(ui.wm.get_window_under_cursor(&mut ui.gui, lx as i32, ly as i32, false), Some(w), "{name}");
            assert_eq!(((r[2] - r[0]), (r[3] - r[1])), (48.75, 9.75));
        }
        let w = win(&ui, "ButtonSinglePlayer");
        let (x, y) = ui.wm.tree.screen_position(w);
        let r = p.to_screen([x as f32, y as f32, x as f32 + 100.0, y as f32 + 20.0]);
        let centre = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
        for msg in [gwm::LEFT_DOWN, gwm::LEFT_UP] {
            ui.pointer_at(msg, centre, &p, true);
        }
        settle(&mut ui, &mut now);
        assert!(ui.wm.tree.windows[win(&ui, "MapBorder2")].is_hidden(), "the tap left the main panel");
        assert!(!ui.wm.tree.windows[win(&ui, "MapBorder")].is_hidden(), "the single player panel is shown");
    }

    /// #78 item 5: the shell's windows find the mapped images the control bar reads, one collection shared. Predicted:
    /// `find_image` gives `ImgBack` (the synthetic bar's image) and nothing for a name the INI lacks; the control bar
    /// loaded with the shell's collection holds the same one (`Arc::ptr_eq`), with its 4 images.
    #[test]
    fn the_shell_and_the_bar_share_the_mapped_images() {
        let (ui, images, fs) = shell_with(&TextTable::default());
        let files = &ui.gui.files.as_ref().expect("the files").files;
        assert_eq!(files.find_image(b"ImgBack").as_deref(), Some(&b"ImgBack"[..]));
        assert_eq!(files.find_image(b"NoSuchImage"), None);
        let crate::ui::UiBuild::Ready(bar) = crate::ui::load_game_ui_with(&fs, &TextTable::default(), b"America", Some(images.clone())) else { panic!("the bar loads") };
        assert!(Arc::ptr_eq(&bar.images, &images));
        assert_eq!(bar.images.len(), 4);
    }

    /// #78, the page's and the native client's check install (not a test of its own): with `RTS_SHELL_INSTALL_FROM` (a
    /// synthetic install with the control bar, `crate::ui`'s `write_the_ui_install_when_asked`) and
    /// `RTS_WRITE_SHELL_INSTALL` (a folder) set, writes that install with the synthetic shell added (no EA file): the
    /// shell's WND files ([`shell_wnd_files`]) in `WindowZH.big`, [`window_transitions_ini`] in `INIZH.big`. Then checks
    /// the shell loads from it, natively. *Prediction:* it loads, with the main menu on top.
    #[test]
    fn write_the_shell_install_when_asked() {
        use rts_lockstep_engine::formats::big::{build_big, ArchiveFile, FilenameList};
        let (Ok(from), Ok(to)) = (std::env::var("RTS_SHELL_INSTALL_FROM"), std::env::var("RTS_WRITE_SHELL_INSTALL")) else { return };
        let (from, to) = (std::path::Path::new(&from), std::path::Path::new(&to));
        std::fs::create_dir_all(to).expect("the folder");
        let add = |name: &str, bytes: Vec<u8>, extra: Vec<(Vec<u8>, Vec<u8>)>| {
            let a = ArchiveFile::open_archive_file(name.as_bytes(), bytes).expect("an archive");
            let mut list = FilenameList::new();
            a.get_file_list_in_directory(b"", b"", b"*", &mut list, true);
            let mut members: Vec<(Vec<u8>, Vec<u8>)> = list.iter().map(|m| (m.to_vec(), a.open_file(m).expect("a member").to_vec())).collect();
            members.extend(extra);
            let refs: Vec<(&[u8], &[u8])> = members.iter().map(|(p, b)| (p.as_slice(), b.as_slice())).collect();
            build_big(&refs)
        };
        // (the synthetic buttons given colours, so that the screenshots show them: the tests' WND has none, and a window
        // without colours draws nothing, as upstream's does)
        let dd = |c: &str| {
            let mut d = format!("IMAGE: NoImage, COLOR: {c}, BORDERCOLOR: 200 200 220 255");
            for _ in 0..8 {
                d.push_str(", IMAGE: NoImage, COLOR: 0 0 0 0, BORDERCOLOR: 0 0 0 0");
            }
            d
        };
        let looks = format!(
            "    TEXTCOLOR = ENABLED: 255 255 255 255, ENABLEDBORDER: 0 0 0 255, DISABLED: 128 128 128 255, DISABLEDBORDER: 0 0 0 255, HILITE: 255 230 120 255, HILITEBORDER: 0 0 0 255;\n    ENABLEDDRAWDATA = {};\n    HILITEDRAWDATA = {};\n",
            dd("30 50 90 230"),
            dd("60 90 150 240")
        );
        let wnds: Vec<(Vec<u8>, Vec<u8>)> = shell_wnd_files()
            .into_iter()
            .map(|(p, b)| {
                let t = String::from_utf8(b).expect("text").replace("    STYLE = PUSHBUTTON", &format!("{looks}    STYLE = PUSHBUTTON"));
                (p.iter().map(|&c| if c == b'/' { b'\\' } else { c }).collect(), t.into_bytes())
            })
            .collect();
        for e in std::fs::read_dir(from).expect("the install") {
            let n = e.expect("an entry").file_name().into_string().expect("a name");
            if !n.ends_with(".big") {
                continue;
            }
            let bytes = std::fs::read(from.join(&n)).expect("read");
            let bytes = match n.as_str() {
                "INIZH.big" => add(&n, bytes, vec![(wt::WINDOW_TRANSITIONS_INI.to_vec(), window_transitions_ini())]),
                "WindowZH.big" => add(&n, bytes, wnds.clone()),
                _ => bytes,
            };
            std::fs::write(to.join(&n), bytes).expect("written");
        }
        let mut fs = FileSystem::new(rts_lockstep_engine::common::file_system::NativeDevice::new(to));
        fs.init(None);
        let crate::ui::ImagesBuild::Ready(images) = crate::ui::load_mapped_images(&fs) else { panic!("the images") };
        let mut ui = ShellUi::load(&fs, &TextTable::default(), images, 0).expect("the shell loads from the install");
        ui.frame(0);
        assert_eq!(ui.top_screen().as_deref(), Some(&b"Menus/MainMenu.wnd"[..]));
    }
}
