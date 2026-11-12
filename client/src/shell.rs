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
//!   `start_campaign`), a quit is the application's. (#78) The save/load screen's load is the saves panel's own load
//!   ([`ShellAction::LoadGame`]: `SaveCmd::Load` in the app), its delete the folder's
//!   ([`crate::saves::SaveFolder::delete`]); the folder is given to the shell's host as `iterateSaveFiles` reads the
//!   directory ([`ShellUi::set_saves`]), and a load that fails shows the shell again ([`ShellUi::show`], `doLoadGame`'s
//!   `showShell(TRUE)`). (#89) The skirmish screen's start ([`ShellRequest::StartSkirmish`]) goes through the same flow
//!   as a campaign's ([`ShellAction::SkirmishChosen`]: `MissionChoice::Skirmish`, the World's `start_skirmish_game`
//!   once the map's folder has arrived); the screens read a copy of the install's data
//!   ([`ShellUi::set_skirmish_data`], given when the shell is loaded). (#89 part 2) A mission's or a skirmish's end
//!   pushes the translated score screen ([`ShellUi::score_screen`], with the copy of the game `clearGameData` keeps);
//!   its *Continue* goes to the flow ([`ShellRequest::StartNextCampaignGame`]: [`Flow::continue_from_score_screen`],
//!   in the World that played) and its mission save to the save folder ([`ShellAction::MissionSaved`]). (#98) The
//!   replay folder ([`crate::replays`]) is given to the shell's host with each file's header read by the logic's
//!   recorder ([`ShellUi::set_replays`]); the score screen's *Save Replay* and the replay menu's delete and copy come
//!   back as [`ShellAction::ReplayWritten`], [`ShellAction::ReplayDeleted`], [`ShellAction::ReplayExported`]; the
//!   replay menu's playback goes through the flow ([`ShellAction::ReplayChosen`]: `MissionChoice::Replay`, the World's
//!   `play_replay`, once the replay's map has arrived); the challenge menu's *Play* is the campaign panel's challenge
//!   start ([`ShellRequest::StartChallenge`]: `MissionChoice::Challenge`, the World's `start_challenge`).
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
//! - ~~(#78) The in-game save popup (`Menus/PopupSaveLoad.wnd`, opened by QuitMenu.cpp, not translated) is not reachable:
//!   its save request ([`ShellRequest::SaveGame`]) is answered with a [`ShellAction::None`] that says so; the saves
//!   panel's *Save game* is the client's way to save.~~ **Corrected 2026-11-10 (#87):** QuitMenu.cpp is translated
//!   (`rts_lockstep_engine::client::quit_menu`): Escape in a game opens the quit menu ([`ShellUi::toggle_quit_menu`],
//!   with what it reads of the World: [`quit_menu_game`]), drawn and given the pointer and keyboard by this window
//!   manager while it or a screen it opened is up ([`ShellUi::in_game_menus_shown`]). Its popup's save is the saves
//!   panel's save under the popup's name and description ([`ShellAction::SaveGame`]); its pause is the app's (the logic's
//!   frames not run: [`ShellAction::SetGamePaused`]; **(#108)** with `setGamePaused`'s side effects: the InGameUI's
//!   input disabled and restored, the audio paused and resumed, the mouse's state, and the drawables' time held,
//!   `rts_lockstep_engine::client::game_pause`); InGameUI's quit menu flag reaches the translators
//!   ([`ShellUi::quit_menu_visible`]), and the World's game cleared runs `closeWindows`
//!   ([`ShellUi::clear_game_data_windows`]); its *Exit* is `MSG_CLEAR_GAME_DATA` sent on the World's command
//!   list ([`ShellAction::ClearGameData`]), whose `clearGameData` opens the score screen as a mission's end does. Its
//!   *Restart* ~~is a gap ([`ShellRequest::RestartMission`] is answered with a [`ShellAction::None`])~~ **Corrected
//!   2026-11-10 (#110):** goes to the flow with the game being played ([`ShellAction::RestartMission`],
//!   [`Flow::restart`]: the World's `restart_mission` in a copy of it, once the map is held). (#109) The control bar's
//!   options button is the other way in, the phone's (ControlBarCallback.cpp's `ToggleQuitMenu`, and `HideQuitMenu`
//!   for the general's and the idle worker's buttons: `crate::ui::QuitMenuCall`, [`ShellUi::bar_quit_menu_call`]). Its
//!   surrender and a
//!   multiplayer exit's `MSG_SELF_DESTRUCT` are never asked (the client has no multiplayer game). A mission-type save
//!   from the popup (not asked by the quit menu, whose popup is save-and-load) fails with a message.
//! - The shell has its own window manager (the control bar has another): it is drawn and takes the pointer and keyboard
//!   only while it is active ([`ShellUi::is_active`]); upstream's one manager keeps drawing what the shutdowns left
//!   shown.
//! - The layout stays 800x600 (`LAYOUT`) and is scaled whole onto the screen ([`shell_placement`]), not re-made at the
//!   display's resolution: ~~on a phone the menu is fitted above the zoom column (mobile first). The synthetic menu's
//!   20-pixel buttons are 9.75 CSS pixels high at 390 wide (reachable by a tap, small; the real WND's sizes are the
//!   owner's files' and not checked).~~ **Corrected 2026-11-12, #153 (owner decision):** on a portrait screen the layout
//!   is drawn a quarter turn clockwise, beside the zoom column, so it fills the long side (scale 0.5433 at 390x844,
//!   against 0.43 upright), and the pointer is turned back with it ([`crate::ui::LayoutPlacement::turned`]); the
//!   synthetic menu's 100 x 20 buttons are 10.87 x 54.33 CSS pixels there (the real WND's sizes are the owner's files'
//!   and not checked). A landscape screen is placed as before.
//! - On the web, a quit cannot close the page: it says so in the status line.
//! - ~~The shell's sounds (`TheAudio->addAudioEvent`) and the script hooks (`signalUIInteract`) are not played or run; the
//!   app clears them each frame.~~ **Corrected 2026-11-11, #136:** the shell's sounds are played through the client's
//!   audio (`ClientAudio::play_gui_sound`, by the app's `update_audio`); ~~the script hooks (`signalUIInteract`) are still
//!   not run, and the app clears them each frame.~~ **Corrected 2026-11-11, #142:** the script hooks (upstream's
//!   `TheShellHookNames` strings, #140) go to the World's script engine when this machine is the sole player and no
//!   replay is recorded (the owner's exception, #139: `shell_hooks::apply_ui_interacts`), and are dropped otherwise.
//! - **(#136)** The shell's window manager sets the one `TheMouse`'s tooltip: the app lends it around each call
//!   ([`ShellUi::with_mouse`]), and draws it over the shell.
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

use rts_lockstep_engine::client::save_game::read_save_summary;
use rts_lockstep_engine::client::shell::{self as sh, ShellRequest, WndFiles};
use rts_lockstep_engine::client::quit_menu::QuitMenuGame;
use rts_lockstep_engine::client::image::ImageCollection;
use rts_lockstep_engine::client::gui_util::SkirmishData;
use rts_lockstep_engine::client::options_menu::{apply_option_preferences, OptionPreferences};
use rts_lockstep_engine::client::user_preferences::MemoryUserData;
use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::client::shell_gui::{start_shell, ShellGui};
use rts_lockstep_engine::client::window_manager::{gwm, GameWindowManager};
use rts_lockstep_engine::client::window_script::WndHost;
use rts_lockstep_engine::client::window_transitions as wt;
use rts_lockstep_engine::common::file_system::{FileSystem, LocalDevice};
use rts_lockstep_engine::common::science::GameTextFetch;
use rts_lockstep_engine::logic::world::campaign_world::{MissionChoice, Restart};
use rts_lockstep_engine::logic::world::game_data::TextTable;
use rts_lockstep_engine::logic::world::game_logic::World;

use crate::campaigns::Flow;
use crate::ui::{LayoutPlacement, UiQuad, LAYOUT};

/// The shell's WND files (`Window\Menus\<name>.wnd`) read when the shell is made: the main menu, its boxes, the
/// background, and the screens the main menu pushes (~~their callbacks are not translated yet; they load and show~~
/// **Corrected 2026-11-10 (#78):** the save/load screen's are translated; the others' are not yet: they load and show).
///
/// (#78) With the save/load screen (`SaveLoad`, and the in-game `PopupSaveLoad`), the single player menu and the
/// difficulty popup, whose callbacks are translated (neither of the last two is reached from Zero Hour's main menu).
///
/// (#89) With the skirmish screen's map picker (`SkirmishMapSelectMenu`); the skirmish screen's callbacks are
/// translated.
///
/// (#89 part 2) With the score screen (`ScoreScreen`, translated) and the replay popup it opens (`PopupReplay`, whose
/// callbacks are ~~not~~ **Corrected 2026-11-10 (#98):** translated, as are `ReplayMenu`'s and `ChallengeMenu`'s).
///
/// (#87) With the in-game quit menu (`QuitMenu`, and `QuitNoSave` for a replay), translated.
pub const SHELL_FILES: [&str; 20] = [
    "MainMenu", "MessageBox", "QuitMessageBox", "BlankWindow", "SaveLoad", "SkirmishGameOptionsMenu", "ReplayMenu", "CreditsMenu", "LanLobbyMenu", "OptionsMenu", "ChallengeMenu",
    "PopupSaveLoad", "SinglePlayerMenu", "DifficultySelect", "SkirmishMapSelectMenu", "ScoreScreen", "PopupReplay",
    // (#88: the options screen's *Keyboard Options*)
    "KeyboardOptionsMenu",
    // (#87)
    "QuitMenu", "QuitNoSave",
];

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
    fn fetch_text_exists(&self, label: &[u8]) -> (Vec<u16>, bool) {
        // (#78) `TheGameText->fetch(label, &exists)`: found in the test's strings or the string file
        let exists = self.text.strings.contains_key(label) || self.text.csf.as_ref().is_some_and(|c| c.fetch(label).is_some());
        (self.text.fetch(label).str().to_vec(), exists)
    }
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        // (#98) the mapped image's size (`getImageWidth`, `getImageHeight`)
        self.images.find_image_by_name(image).map(|i| i.image_size)
    }
}

/// What the client does for a request of the shell's callbacks.
#[derive(Clone, Debug, PartialEq)]
pub enum ShellAction {
    /// Nothing to do here (said why).
    None(&'static str),
    /// A campaign's first mission waits for its map in the flow (the campaign panel's path).
    CampaignChosen,
    /// The start failed (the campaign has no such mission, ...).
    Failed(String),
    /// The application should end (`TheGameEngine->setQuitting(TRUE)`).
    Quit,
    /// (#78) The save/load screen loads this save of the folder: the saves panel's own load (`SaveCmd::Load` in the app,
    /// which starts with [`crate::saves::PendingLoad::new`]).
    LoadGame(String),
    /// (#78) The save/load screen deleted this save: the folder's [`crate::saves::SaveFolder::delete`].
    DeleteSave(String),
    /// (#89) The skirmish screen's start waits for its map in the flow (the campaign panel's path: [`Flow::choose`]
    /// with `MissionChoice::Skirmish`, the World's `start_skirmish_game`).
    SkirmishChosen,
    /// (#89 part 2) The score screen's mission save (`missionSave`): its description and bytes, for the save folder.
    MissionSaved(String, Vec<u8>),
    /// (#88) The options screen's `TheAudio->setVolume(volume, affect)`: the client's audio manager's.
    SetVolume(f32, u32),
    /// (#98) A replay the shell wrote into the replay folder (the score screen's *Save Replay*): its name and bytes,
    /// for [`crate::replays::ReplayFolder::write`].
    ReplayWritten(String, Vec<u8>),
    /// (#98) A replay the shell deleted: [`crate::replays::ReplayFolder::delete`].
    ReplayDeleted(String),
    /// (#98) The replay menu's copy to the desktop: [`crate::replays::ReplayFolder::export`].
    ReplayExported(String, Vec<u8>),
    /// (#98) The replay menu's playback waits for the replay's map in the flow ([`Flow::choose`] with
    /// `MissionChoice::Replay`: the World's `play_replay`).
    ReplayChosen,
    /// (#87) The in-game save popup's `saveGame`: the saves panel's save, under `name` (`None`: the folder's next name,
    /// `findNextSaveFilename`) with the popup's `description`.
    SaveGame { name: Option<String>, description: String },
    /// (#87) `setGamePaused`: the app stops (or resumes) running the logic's frames.
    SetGamePaused(bool),
    /// (#87) The quit menu's `MSG_CLEAR_GAME_DATA`: sent on the World's command list (`World::push_command`), its next
    /// frame's `clearGameData` ends the game.
    ClearGameData,
    /// (#110) The quit menu's *Restart* (`restartMissionMenu`, what it read of the game): the app hands it to the flow
    /// with the game being played ([`Flow::restart`]), which waits for the game's map and starts it again.
    RestartMission(Restart),
}

/// The client's side of a request (#89: [`ShellRequest::StartSkirmish`] goes through the same flow as a campaign's
/// start: [`ShellAction::SkirmishChosen`]): [`ShellRequest::StartCampaign`] is the campaign panel's start of the campaign's
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
        ShellRequest::LoadGame { filename } => ShellAction::LoadGame(String::from_utf8_lossy(filename).into_owned()),
        ShellRequest::DeleteSave { filename } => ShellAction::DeleteSave(String::from_utf8_lossy(filename).into_owned()),
        // ~~the in-game save popup is not reachable~~ (#87) the saves panel's save, under the popup's name
        ShellRequest::SaveGame { filename, description, file_type } => {
            if *file_type == rts_lockstep_engine::common::game_state::SAVE_FILE_TYPE_MISSION {
                ShellAction::Failed("a mission-type save from the save popup is not wired (the quit menu's popup saves normal games)".to_string())
            } else {
                let name = (!filename.is_empty()).then(|| String::from_utf8_lossy(filename).into_owned());
                ShellAction::SaveGame { name, description: String::from_utf16_lossy(description) }
            }
        }
        // ~~no game runs under the shell~~ (#87) the quit menu's exit: the World's own message
        ShellRequest::ClearGameData => ShellAction::ClearGameData,
        ShellRequest::SetGamePaused(p) => ShellAction::SetGamePaused(*p),
        ShellRequest::SelfDestruct => ShellAction::None("MSG_SELF_DESTRUCT is a multiplayer game's (the client has none)"),
        // ~~not wired~~ (#110) the quit menu's restart: the app gives it the game being played ([`Flow::restart`])
        ShellRequest::RestartMission { game_mode, difficulty, rank_points, fps, map_name, replay_file, stop_recording } => ShellAction::RestartMission(Restart {
            game_mode: *game_mode,
            difficulty: *difficulty,
            rank_points: *rank_points,
            fps: *fps,
            map_name: map_name.clone(),
            replay_file: replay_file.clone(),
            stop_recording: *stop_recording,
        }),
        ShellRequest::NewShellGame => ShellAction::None("the shell map is not translated"),
        // (#89 part 2) the score screen's *Continue*: the next mission (or the same, after a defeat) in the World that
        // played, with the screen's campaign
        ShellRequest::StartNextCampaignGame { campaign } => match flow.continue_from_score_screen((**campaign).clone()) {
            Ok(()) => ShellAction::CampaignChosen,
            Err(e) => ShellAction::Failed(e),
        },
        ShellRequest::MissionSave { campaign } => match flow.mission_save_from_score_screen(campaign) {
            Some((d, b)) => ShellAction::MissionSaved(d, b),
            None => ShellAction::Failed("the mission save failed (no game ended, or no campaign)".to_string()),
        },
        ShellRequest::SetVolume { volume, affect } => ShellAction::SetVolume(*volume, *affect),
        // (#98) the challenge menu's *Play*: the campaign panel's challenge start (`start_challenge`)
        ShellRequest::StartChallenge { general, difficulty } => match flow.choose(base, MissionChoice::Challenge(*general), *difficulty) {
            Ok(()) => ShellAction::CampaignChosen,
            Err(e) => ShellAction::Failed(e),
        },
        // (#98) the replay folder's changes, kept by the client's folder
        ShellRequest::WriteReplay { path, bytes } => ShellAction::ReplayWritten(crate::replays::name_of_path(path), bytes.clone()),
        ShellRequest::DeleteReplay { path } => ShellAction::ReplayDeleted(crate::replays::name_of_path(path)),
        ShellRequest::ExportReplay { name, bytes } => ShellAction::ReplayExported(String::from_utf8_lossy(name).into_owned(), bytes.clone()),
        // (#98) without the folder's bytes (see `ShellUi::take_actions`, which gives them)
        ShellRequest::PlaybackReplay { .. } => ShellAction::Failed("a replay's playback needs the replay folder".to_string()),
        // (#89) the skirmish screen's start: a copy of `base` waits for the map's folder, then `start_skirmish_game`
        ShellRequest::StartSkirmish { game, mode, difficulty, max_fps } => {
            let choice = MissionChoice::Skirmish { game: game.clone(), mode: *mode, max_fps: *max_fps };
            match flow.choose(base, choice, *difficulty) {
                Ok(()) => ShellAction::SkirmishChosen,
                Err(e) => ShellAction::Failed(e),
            }
        }
    }
}

/// (#88) What the client applies of the options ([`ShellUi::client_options`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientOptions {
    /// The keyboard pan's speed, 1 at the default scroll factor.
    pub keyboard_scroll: f32,
    /// `TheGlobalData->m_useAlternateMouse` (the command translation's `use_alternate_mouse`).
    pub alternate_mouse: bool,
}

/// The shell: its window manager and its host.
pub struct ShellUi {
    pub wm: GameWindowManager,
    pub gui: ShellGui<MenuFiles>,
    /// (#157) The maps' `.tga` files the listing found and the file system has not served yet (the browser fetches
    /// them as they are asked for): read by [`ShellUi::load_map_previews`].
    pub previews_pending: Vec<Vec<u8>>,
    /// (#160) `Data\INI\GameLOD.ini` and `Data\INI\GameLODPresets.ini` as the art's file system had them when the
    /// shell was read (the LOD manager's `init` reads them: [`ShellUi::set_user_data`]).
    pub lod_files: Vec<(Vec<u8>, Vec<u8>)>,
    /// (#160) `W3DDisplay::updateAverageFPS`'s statics and `m_averageFPS` (the client's clock in microseconds).
    pub average_fps: rts_lockstep_engine::client::game_lod::AverageFps,
    /// (#160) What the LOD manager's init said (`None`: not run yet; an error: an INI file missing or refused).
    pub lod_init: Option<Result<(), String>>,
}

impl ShellUi {
    /// The shell from the art's files, started (the main menu pushed), or `None` without `Menus\MainMenu.wnd` or
    /// (#77) `Data\INI\WindowTransitions.ini` (or with one that does not read). The time is the client's clock in
    /// milliseconds.
    ///
    /// (#78) `images` is the mapped image collection the control bar shares (`crate::ui::load_mapped_images`).
    ///
    /// (#151) `language` is `TheGlobalLanguageData`: its `DefaultWindowFont` is every window's default font
    /// (`GameWindowManager::default_window_font`, GameWindowManager.cpp:1447).
    pub fn load<D: LocalDevice>(fs: &FileSystem<D>, text: &TextTable, images: Arc<ImageCollection>, now_ms: u32, language: &rts_lockstep_engine::client::global_language::GlobalLanguage) -> Option<ShellUi> {
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
        // (#148) the briefing history's window (`ToggleDiplomacy`'s `winCreateLayout("Diplomacy.wnd")`); (#151) the popup
        // message's (`popupMessage`'s `winCreateLayout("InGamePopupMessage.wnd")`)
        for f in [&b"Window\\Diplomacy.wnd"[..], b"Window\\InGamePopupMessage.wnd"] {
            if let Some(d) = fs.open_file(f) {
                files.insert(f.to_vec(), d);
            }
        }
        // (#77) `TheTransitionHandler->load()` at the window manager's init: without the file upstream's INI load
        // throws; here there is no shell
        let transitions = fs.open_file(wt::WINDOW_TRANSITIONS_INI)?;
        let mut wm = GameWindowManager::new(LAYOUT);
        wm.default_window_font = Some(language.default_window_font.clone());
        wt::load_window_transitions(&mut wm, wt::WINDOW_TRANSITIONS_INI, &transitions).ok()?;
        // (#160) the LOD manager's two files
        let lod_files = [&b"Data\\INI\\GameLOD.ini"[..], b"Data\\INI\\GameLODPresets.ini"].iter().filter_map(|p| fs.open_file(p).map(|d| (p.to_vec(), d))).collect();
        let mut ui = ShellUi {
            wm,
            gui: ShellGui::new(MenuFiles { files, keys: BTreeMap::new(), text: text.clone(), images }),
            previews_pending: Vec::new(),
            lod_files,
            average_fps: Default::default(),
            lod_init: None,
        };
        ui.gui.time_ms = now_ms;
        // (#153) the windows measured as upstream measures them (`DisplayString::getSize`, `winFontHeight`), over the
        // client's estimate of GDI's answers (`crate::text_metrics`)
        ui.gui.text_metrics = Some(crate::text_metrics::METRICS);
        start_shell(&mut ui.wm, &mut ui.gui);
        Some(ui)
    }

    /// A frame at `now_ms`: the shell's update (every screen's, at most 30 a second) and the window manager's
    /// (`processDestroyList`, then (#77) the transitions' update).
    pub fn frame(&mut self, now_ms: u32) {
        self.gui.time_ms = now_ms;
        self.wm.time_ms = now_ms;
        sh::update(&mut self.wm, &mut self.gui);
        // (#148) the briefing history's layout update (`updateFunc`: its slide), and the quit menu's `HideDiplomacy`
        self.diplomacy_calls();
        if self.gui.diplomacy.the_layout.is_some() {
            self.gui.diplomacy_with(&mut self.wm, |h| rts_lockstep_engine::client::diplomacy::update_func(h));
        }
        self.wm.update(&mut self.gui);
    }

    /// (#148) The quit menu's `HideDiplomacy()` and `closeWindows`' `ResetDiplomacy()` calls, made on the briefing
    /// history's window.
    fn diplomacy_calls(&mut self) {
        use rts_lockstep_engine::client::diplomacy::{hide_diplomacy, reset_diplomacy};
        for c in std::mem::take(&mut self.gui.diplomacy_calls) {
            self.gui.diplomacy_with(&mut self.wm, |h| if c == "ResetDiplomacy" { reset_diplomacy(h) } else { hide_diplomacy(h, true) });
        }
    }

    /// (#148) `ToggleDiplomacy(FALSE)`: the control bar's communicator button holding `NonCommand_BriefingHistory`
    /// (`ControlBarSystem`, ControlBarCallback.cpp:399), with what the window reads of the game (`game`). Its
    /// `HideQuitMenu()` hides the shell's quit menu. `rts_lockstep_engine::client::diplomacy`.
    pub fn toggle_diplomacy(&mut self, game: rts_lockstep_engine::client::diplomacy::DiplomacyGame) {
        self.gui.diplomacy.game = game;
        let ((), hide_quit_menu) = self.gui.diplomacy_with(&mut self.wm, |h| rts_lockstep_engine::client::diplomacy::toggle_diplomacy(h, false));
        if hide_quit_menu {
            self.hide_quit_menu();
        }
    }

    /// (#148) `UpdateDiplomacyBriefingText(label, FALSE)`: a briefing the scripts showed (`InGameUI::militarySubtitle`,
    /// `popupMessage`, the logic's `ClientEvent::InGameUi`), into the history (and the open window's list).
    pub fn briefing(&mut self, label: &[u8]) {
        self.gui.diplomacy_with(&mut self.wm, |h| rts_lockstep_engine::client::diplomacy::update_diplomacy_briefing_text(h, label, false));
    }

    /// (#151) `UpdateDiplomacyBriefingText(TheEmptyString, TRUE)`: the history cleared, as `GameClient::xfer`'s load does
    /// before it gives the save's list.
    pub fn clear_briefings(&mut self) {
        self.gui.diplomacy_with(&mut self.wm, |h| rts_lockstep_engine::client::diplomacy::update_diplomacy_briefing_text(h, b"", true));
    }

    /// (#151) `GetBriefingTextList()`: the history, for a save (`GameClient::xfer`).
    pub fn briefing_list(&self) -> &[Vec<u8>] {
        &self.gui.diplomacy.briefing_list
    }

    /// (#148) The briefing history's window is up (made and not hidden): it is drawn and takes the input.
    pub fn diplomacy_shown(&self) -> bool {
        self.gui.diplomacy.the_window.is_some_and(|w| !self.wm.is_hidden(Some(w)))
    }

    /// (#151) `TheInGameUI->popupMessage(message, x, y, width, textColor, pause, pauseMusic)`: the logic's
    /// `ClientEvent::InGameUi(PopupMessage)` with InGameUI.ini's `PopupMessageColor`
    /// (`rts_lockstep_engine::client::in_game_popup_message`), on the shell's window manager. Its label goes to the briefing
    /// history.
    #[allow(clippy::too_many_arguments)]
    pub fn popup_message(&mut self, message: &[u8], x: i32, y: i32, width: i32, color: i32, pause: bool, pause_music: bool) {
        let mut h = rts_lockstep_engine::client::shell_gui::ShellPopup { wm: &mut self.wm, gui: &mut self.gui };
        rts_lockstep_engine::client::in_game_popup_message::popup_message(&mut h, message, x, y, width, color, pause, pause_music);
    }

    /// (#151) `TheInGameUI->clearPopupMessageData()`: the logic's `MSG_CLEAR_INGAME_POPUP_MESSAGE`
    /// (`ClientCall::ClearPopupMessage`), and `InGameUI::reset`.
    pub fn clear_popup_message(&mut self) {
        let mut h = rts_lockstep_engine::client::shell_gui::ShellPopup { wm: &mut self.wm, gui: &mut self.gui };
        rts_lockstep_engine::client::in_game_popup_message::clear_popup_message_data(&mut h);
    }

    /// (#155) A popup message held while there was no shell (`rts_lockstep_engine::client::in_game_popup_message::
    /// WindowlessPopup`, its pause and history label already made), shown on the shell's window manager now that there is
    /// one (`show_held_popup_message`: `popupMessage`'s window, made as if when the message came).
    pub fn show_held_popup_message(&mut self, data: rts_lockstep_engine::client::in_game_popup_message::PopupMessageData) {
        let mut h = rts_lockstep_engine::client::shell_gui::ShellPopup { wm: &mut self.wm, gui: &mut self.gui };
        rts_lockstep_engine::client::in_game_popup_message::show_held_popup_message(&mut h, data);
    }

    /// (#151) The popup's calls on the game since the last call (the pause, the clear message), for the app to make.
    pub fn take_popup_calls(&mut self) -> Vec<rts_lockstep_engine::client::shell_gui::PopupCall> {
        std::mem::take(&mut self.gui.popup_calls)
    }

    /// (#151) The popup message's window is up: it is drawn and takes the input.
    pub fn popup_shown(&self) -> bool {
        self.gui.popup.popup_message_data.as_ref().is_some_and(|d| d.layout.is_some()) && self.gui.popup.parent.is_some_and(|w| !self.wm.is_hidden(Some(w)))
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

    /// (#136) `f` with the one `TheMouse`'s tooltip lent to the shell's window manager
    /// (`GameWindowManager::swap_mouse_tooltip`): its windows' tooltips (`winProcessMouseEvent`) and its menus'
    /// `setCursorTooltip` calls set the tooltip the application draws, and nothing else's.
    pub fn with_mouse<R>(&mut self, the_mouse: &mut rts_lockstep_engine::client::mouse::MouseTooltip, f: impl FnOnce(&mut Self) -> R) -> R {
        self.wm.swap_mouse_tooltip(the_mouse);
        let r = f(self);
        self.wm.swap_mouse_tooltip(the_mouse);
        r
    }

    /// A key and its text ([`crate::ui::keyboard_into`]); whether the shell used it.
    pub fn keyboard(&mut self, scan: u8, down: bool, text: Option<&str>) -> bool {
        crate::ui::keyboard_into(&mut self.wm, &mut self.gui, scan, down, text)
    }

    /// The callbacks' requests since the last call, acted on ([`act`]). (#98) A replay's playback
    /// (`TheRecorder->playbackFile`) takes the file's bytes from the shell's replay folder: the flow's
    /// `MissionChoice::Replay`, which waits for the replay's map.
    pub fn take_actions(&mut self, base: &World, flow: &mut Flow) -> Vec<ShellAction> {
        let requests = std::mem::take(&mut self.gui.requests);
        requests
            .iter()
            .map(|r| match r {
                ShellRequest::PlaybackReplay { filename } => self.playback(filename, base, flow),
                r => act(r, base, flow),
            })
            .collect()
    }

    /// (#98) `TheRecorder->playbackFile(filename)`: the replay folder's file (by its name, without case), in a copy of
    /// `base`, waiting for its map (the difficulty is the file's: `DIFFICULTY_NORMAL` is not read).
    fn playback(&self, filename: &[u8], base: &World, flow: &mut Flow) -> ShellAction {
        let mut path = b"Replays\\".to_vec();
        path.extend_from_slice(filename);
        let Some(i) = self.gui.replays.find(&path) else {
            return ShellAction::Failed(format!("no replay {}", String::from_utf8_lossy(filename)));
        };
        let bytes = self.gui.replays.files[i].1.clone();
        let choice = MissionChoice::Replay { filename: filename.to_vec(), bytes };
        match flow.choose(base, choice, rts_lockstep_engine::common::recorder::DIFFICULTY_NORMAL) {
            Ok(()) => ShellAction::ReplayChosen,
            Err(e) => ShellAction::Failed(e),
        }
    }

    /// (#98) The replay folder as the replay list and the replay popup read it: each `.rep` file under `Replays\`,
    /// with its header and options read by the logic's recorder on a copy of `base` (the install's World; not
    /// changed): `replay_listings`.
    pub fn set_replays(&mut self, folder: &crate::replays::ReplayFolder, base: &World) {
        let files: Vec<(Vec<u8>, Vec<u8>)> = folder.files.iter().map(|(n, b)| (n.as_bytes().to_vec(), b.clone())).collect();
        let listings = rts_lockstep_engine::client::replay_menu::replay_listings(base, &files);
        self.gui.replays.files = files
            .into_iter()
            .zip(listings)
            .map(|((n, b), l)| {
                let mut p = b"Replays\\".to_vec();
                p.extend(n);
                (p, b, l)
            })
            .collect();
    }

    /// (#78) The save folder as the save/load screen lists it (`iterateSaveFiles`, `getSaveGameInfoFromFile`): each
    /// file's name and its information ([`read_save_summary`]; `None` for a file that does not read, which
    /// `addGameToAvailableList` leaves out), in the folder's order.
    pub fn set_saves(&mut self, folder: &crate::saves::SaveFolder) {
        self.gui.save_files = folder.files.iter().map(|(n, b)| (n.as_bytes().to_vec(), read_save_summary(b.clone()).ok().map(|s| s.info))).collect();
    }

    /// (#93) The user data folder's preference files, for the translated `UserPreferences::load`
    /// (`getPath_UserData()` is empty here: a file is opened by its name), and (#88) the client's copy of the global
    /// data the options screen reads and writes: `base`'s (the install's World; a new one without it), with
    /// `Options.ini` applied as `GlobalData::parseGameDataDefinition`'s tail applies it (`apply_option_preferences`; no
    /// IP addresses: the client has no network). The copy is the shell's; `base` is not changed.
    pub fn set_user_data(&mut self, folder: &crate::user_data::UserDataFolder, base: Option<&World>) {
        self.gui.user_data = MemoryUserData { path: Vec::new(), files: folder.files.iter().map(|(n, b)| (n.as_bytes().to_vec(), b.clone())).collect() };
        let mut g = base.and_then(|w| w.statics.global_data.the_global_data().cloned()).unwrap_or_else(GlobalData::new);
        let mut p = OptionPreferences::load_from(&mut self.gui.user_data);
        apply_option_preferences(&mut g, &mut p, &[]);
        self.gui.global_data = g;
        // (#160) `TheGameLODManager->init()` (GameEngine::init, after the global data) and W3DDisplay::init's level, on
        // the shell's copy of the global data
        let files = self.lod_files.clone();
        let r = self.gui.init_game_lod(|p| files.iter().find(|(n, _)| n.as_slice() == p).map(|(_, d)| d.clone()));
        self.lod_init = Some(r.map_err(|e| format!("{e:?}")));
    }

    /// (#160) GUI glue's helper, each frame: `W3DDisplay::draw`'s `updateAverageFPS` at `now_us` (the client's clock,
    /// in microseconds) and its dynamic LOD (`m_enableDynamicLOD` of the shell's global data, `show_dynamic_lod` the
    /// World's `TheGameLogic->getShowDynamicLOD()`), after the particle count the particles advanced
    /// (`particle_generations`, from [`ParticleLod`]) is given back to the manager. What the particles read now.
    ///
    /// [`ParticleLod`]: rts_lockstep_engine::client::client_fx::ParticleLod
    pub fn lod_frame(&mut self, now_us: i64, show_dynamic_lod: bool, particle_generations: Option<i32>) -> rts_lockstep_engine::client::client_fx::ParticleLod {
        if let Some(n) = particle_generations {
            self.gui.lod.num_particle_generations = n;
        }
        self.average_fps.update(now_us, 1_000_000);
        let enable = self.gui.global_data.enable_dynamic_lod;
        let fps = self.average_fps.average_fps;
        self.gui.lod.display_draw(enable, show_dynamic_lod, fps);
        let l = &self.gui.lod;
        rts_lockstep_engine::client::client_fx::ParticleLod {
            max_particle_count: self.gui.global_data.max_particle_count,
            min_dynamic_particle_priority: l.min_dynamic_particle_priority,
            min_dynamic_particle_skip_priority: l.min_dynamic_particle_skip_priority,
            particle_skip_mask: l.dynamic_particle_skip_mask,
            particle_generations: l.num_particle_generations,
        }
    }

    /// (#160) `TheGameLODManager->getStaticLODLevel()` once the manager has a level (`None` before: the drawables keep
    /// their own).
    pub fn static_lod(&self) -> Option<i32> {
        let l = self.gui.lod.get_static_lod_level();
        (l != rts_lockstep_engine::client::game_lod::STATIC_GAME_LOD_UNKNOWN).then_some(l)
    }

    /// (#93) The preference files the shell wrote since the last call (`UserPreferences::write`): name and bytes.
    pub fn take_user_data_writes(&mut self) -> Vec<(String, Vec<u8>)> {
        std::mem::take(&mut self.gui.user_data_writes).into_iter().map(|(p, b)| (String::from_utf8_lossy(&p).into_owned(), b)).collect()
    }

    /// (#88) The options the client applies, from the shell's global data: the keyboard scroll factor relative to
    /// its default (the client's camera's keyboard pan is scaled by it, as `LookAtXlat` scales upstream's by the
    /// factor) and the alternate mouse mode.
    pub fn client_options(&self) -> ClientOptions {
        let g = &self.gui.global_data;
        let scroll = if g.keyboard_default_scroll_factor > 0.0 { g.keyboard_scroll_factor / g.keyboard_default_scroll_factor } else { 1.0 };
        ClientOptions { keyboard_scroll: scroll, alternate_mouse: g.use_alternate_mouse }
    }

    /// (#89) The install's data the skirmish screens read (`TheMapCache`, `TheMultiplayerSettings`, the player
    /// templates, ...), copied from `base` without changing it, with the map files `fs` holds (for `GameInfo::setMap`'s
    /// contents mask).
    pub fn set_skirmish_data<D: LocalDevice>(&mut self, base: &World, fs: &FileSystem<D>) {
        let mut list = rts_lockstep_engine::formats::big::FilenameList::new();
        fs.get_file_list_in_directory(b"Maps\\", b"*", &mut list, true);
        let files: std::collections::BTreeSet<Vec<u8>> = list.iter().map(|n| n.to_ascii_lowercase()).collect();
        // (#157) the maps' preview `.tga` files, which `getMapPreviewImage` copies out (`copyFromBigToDir`)
        self.previews_pending = files.iter().filter(|n| n.ends_with(b".tga")).cloned().collect();
        self.gui.skirmish_data = SkirmishData::from_world(base, files);
        self.load_map_previews(fs);
    }

    /// (#157) The pending map previews the file system serves now, read into the skirmish data
    /// ([`SkirmishData::map_previews`]); the others stay pending (the browser's fetch is under way). How many are left.
    pub fn load_map_previews<D: LocalDevice>(&mut self, fs: &FileSystem<D>) -> usize {
        let pending = std::mem::take(&mut self.previews_pending);
        for p in pending {
            let before = fs.local.device.unserved_reads();
            match fs.open_file(&p) {
                Some(bytes) => {
                    self.gui.skirmish_data.map_previews.insert(p, bytes);
                }
                // (not there yet: asked again later; not there at all: dropped)
                None if fs.local.device.unserved_reads() != before => self.previews_pending.push(p),
                None => {}
            }
        }
        self.previews_pending.len()
    }

    /// (#89 part 2) `GameLogic::clearGameData`'s score screen for the game that ended (`game`, what the screen reads of
    /// it: [`Flow::check_score_screen`]): the fade, the push of `Menus/ScoreScreen.wnd` (its init reads `game`),
    /// `showShell(FALSE)`.
    pub fn score_screen(&mut self, game: rts_lockstep_engine::client::score_screen::ScoreScreenGame) {
        self.gui.score_screen.game = Some(game);
        rts_lockstep_engine::client::score_screen::clear_game_data_score_screen(&mut self.wm, &mut self.gui);
    }

    /// (#78) `TheShell->showShell(TRUE)`, as a failed load's `doLoadGame` calls it: the top screen's init again, the
    /// shell active.
    pub fn show(&mut self) {
        // (#87: no game under the shell)
        self.gui.env.in_game = false;
        sh::show_shell(&mut self.wm, &mut self.gui, true);
    }

    /// The file the top screen was read from.
    pub fn top_screen(&self) -> Option<Vec<u8>> {
        self.gui.shell.top().map(|l| self.wm.layouts[l].filename.clone())
    }

    /// (#87) `ToggleQuitMenu()` (Escape in a game, `MSG_META_OPTIONS`), with what the menu reads of the game (`game`:
    /// [`quit_menu_game`]). The shell's global data says a game runs (`isInGame`, for the options screen).
    pub fn toggle_quit_menu(&mut self, game: QuitMenuGame) {
        self.gui.env.in_game = game.is_in_game();
        self.gui.env.in_shell_game = game.game_mode == rts_lockstep_engine::common::recorder::game_mode::SHELL;
        self.gui.quit_menu.game = game;
        rts_lockstep_engine::client::quit_menu::toggle_quit_menu(&mut self.wm, &mut self.gui);
        // (#148: its `HideDiplomacy()`)
        self.diplomacy_calls();
    }

    /// (#109) `HideQuitMenu()` (the control bar's general and idle worker buttons call it first): the quit menu hidden if
    /// it is shown, the game unpaused outside a multiplayer game (the action [`ShellAction::SetGamePaused`]).
    pub fn hide_quit_menu(&mut self) {
        rts_lockstep_engine::client::quit_menu::hide_quit_menu(&mut self.wm, &mut self.gui);
    }

    /// (#108) `TheInGameUI->isQuitMenuVisible()`: the flag the quit menu keeps, which the app feeds to the translators
    /// (`WorldClient`'s `ClientModes::quit_menu_visible`).
    pub fn quit_menu_visible(&self) -> bool {
        self.gui.quit_menu.quit_menu_visible
    }

    /// (#108) What `GameLogic::clearGameData` does to the in-game windows, the client's half: `InGameUI::reset`'s two
    /// quit-menu flags cleared, then `closeWindows` (`HideQuitMenu`, the options screen's *Back*):
    /// `rts_lockstep_engine::client::quit_menu::clear_game_data_windows`. The app calls it when it sees the World's game
    /// cleared.
    pub fn clear_game_data_windows(&mut self) {
        rts_lockstep_engine::client::quit_menu::clear_game_data_windows(&mut self.wm, &mut self.gui);
        self.diplomacy_calls();
        // (#148) `TheGameEngine->reset()`'s `InGameUI::reset`: `UpdateDiplomacyBriefingText("", TRUE)`, the history cleared
        self.gui.diplomacy_with(&mut self.wm, |h| rts_lockstep_engine::client::diplomacy::update_diplomacy_briefing_text(h, b"", true));
        // (#151) and its `clearPopupMessageData()`
        self.clear_popup_message();
    }

    /// (#117) `TheGameLogic->closeWindows()` from a script (`ScriptActions::doQuickVictory`, which reaches the client as
    /// `ScriptInputCall::CloseWindows`): `rts_lockstep_engine::client::quit_menu::close_windows`.
    pub fn close_windows(&mut self) {
        rts_lockstep_engine::client::quit_menu::close_windows(&mut self.wm, &mut self.gui);
        self.diplomacy_calls();
    }

    /// (#117) `TheInGameUI->isClientQuiet()` (the quit menu's flag), for the audio's radar edge sounds.
    pub fn client_quiet(&self) -> bool {
        self.gui.quit_menu.client_quiet
    }

    /// (#117) The shell's calls on `TheMouse` since the last call, made on `mouse` in order: the main menu's
    /// `setVisibility`, the quit menu's `setCursor(ARROW)`.
    pub fn take_mouse_calls(&mut self, mouse: &mut rts_lockstep_engine::client::mouse::Mouse) {
        for v in self.gui.main_menu.mouse_set_visibility.drain(..) {
            mouse.set_visibility(v);
        }
        for c in self.gui.quit_menu.mouse_set_cursor.drain(..) {
            mouse.set_cursor(c);
        }
    }

    /// (#109) The control bar's quit menu call ([`crate::ui::QuitMenuCall`], ControlBarCallback.cpp's `ToggleQuitMenu`
    /// on the options button, `HideQuitMenu` on the general's and the idle worker's), with what the menu reads of the
    /// game when it toggles.
    pub fn bar_quit_menu_call(&mut self, call: crate::ui::QuitMenuCall, game: impl FnOnce() -> QuitMenuGame) {
        match call {
            crate::ui::QuitMenuCall::Toggle => self.toggle_quit_menu(game()),
            crate::ui::QuitMenuCall::Hide => self.hide_quit_menu(),
        }
    }

    /// (#87) Whether the in-game menus are up (the quit menu, its save/load popup, the options screen, a box):
    /// `rts_lockstep_engine::client::quit_menu::in_game_menus_shown`.
    pub fn in_game_menus_shown(&self) -> bool {
        rts_lockstep_engine::client::quit_menu::in_game_menus_shown(&self.wm, &self.gui.quit_menu, self.gui.shell.options_layout)
    }

    /// (#87) Whether this window manager is drawn and takes the pointer and keyboard: the shell is active, or the
    /// in-game menus are up.
    pub fn takes_input(&self) -> bool {
        // (#148: and while the briefing history is up; new: upstream's game keeps its input around the window; #151: and
        // the popup message)
        self.is_active() || self.in_game_menus_shown() || self.diplomacy_shown() || self.popup_shown()
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
        // (#87: also while the in-game menus are up)
        if !self.takes_input() {
            return Vec::new();
        }
        let items = self.wm.win_repaint(&mut self.gui);
        crate::ui::draw_list_quads(&items, &self.gui)
    }

    /// (#78) A pointer event at a screen point (logical pixels), into the layout through `placement`
    /// ([`LayoutPlacement::to_layout`]) and on to [`ShellUi::pointer`]; nothing while the shell is not active.
    pub fn pointer_at(&mut self, msg: u32, screen: (f32, f32), placement: &LayoutPlacement, touch: bool) {
        // (#87: also while the in-game menus are up)
        if self.takes_input() {
            self.pointer(msg, placement.to_layout(screen), touch);
        }
    }
}

/// (#87) What the quit menu reads of the game being played (`QuitMenuGame`), from the World, which is only read:
/// the game mode, `isGameEnding`, the map name, the rank points, the difficulty and the frame limit; `input_enabled` is
/// the client's `InGameUI`'s. Not read (the client has no such state~~, or the quit menu's use of it is a gap~~): no intro
/// movie or map loading under a running game, no disconnect menu (no network), the local player active and not
/// victorious (read only in a multiplayer game)~~, no replay file name and no recording (read only by the restart, a
/// gap), the map not a save's~~. **Corrected 2026-11-10 (#110):** the restart's are read: the recorder's
/// `getCurrentReplayFilename()` and whether it records, and for a map in the save directory (`isInSaveDirectory`, as
/// the World's `startNewGame` found it) `getPristineMapName()`.
pub fn quit_menu_game(world: &World, input_enabled: bool) -> QuitMenuGame {
    use rts_lockstep_engine::common::recorder::RecorderMode;
    use rts_lockstep_engine::logic::world::game_logic_new_game::NewGameHost;
    let g = world.statics.global_data.the_global_data();
    // ~~`g.map_name`~~ **Corrected 2026-11-10 (#110):** `TheGlobalData->m_mapName` as the game sets it is the World's
    // `cuts.new_game.map_name` (`startNewGame`'s, a skirmish's or a load's); the INI's global data is not given the
    // map, so the quit menu read an empty name for every game
    let map_name = world.cuts.new_game.map_name.clone();
    let recorder = world.cuts.game_loop.recorder.as_ref();
    QuitMenuGame {
        game_mode: world.cuts.game_loop.game_mode,
        game_ending: world.script_engine.is_game_ending(),
        input_enabled,
        pristine_map_name: world.sn_is_in_save_directory(&map_name).then(|| world.sn_pristine_map_name()),
        replay_file: recorder.map(|r| r.current_replay_filename()).unwrap_or_default(),
        recording: recorder.is_some_and(|r| r.mode == RecorderMode::Record),
        map_name,
        rank_points: world.game_logic.rank_points_to_add_at_game_start,
        difficulty: world.script_engine.game_difficulty,
        fps_limit: g.map_or(30, |g| g.frames_per_second_limit),
        ..QuitMenuGame::default()
    }
}

/// (#87) The quit menu's exit (`exitQuitMenu`'s `MSG_CLEAR_GAME_DATA`) on the World's command list, from the local
/// player `player`: the logic's own message path (the recorder does not record it), dispatched at the next frame
/// (`clearGameData`, which opens the score screen).
pub fn send_clear_game_data(world: &mut World, player: i32) {
    world.push_command(rts_lockstep_engine::common::message_types::MSG_CLEAR_GAME_DATA, player, |_| {});
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
///
/// **(#153, owner decision 2026-11-12)** On a portrait screen (taller than wide) the layout is drawn a quarter turn
/// clockwise ([`LayoutPlacement::turned`]), so its 800-pixel side runs along the screen's long side: the same two places,
/// with the turned layout's box (600 x 800, scaled) fitted to them. At 390x844 the scale is 0.5433 (beside the column,
/// 326 x 800: 326 / 600), not the upright 0.43. The player turns the phone to read it; the pointer is turned back through
/// [`LayoutPlacement::to_layout`]. A landscape screen is placed as before.
pub fn shell_placement(screen: (f32, f32)) -> LayoutPlacement {
    let (lw, lh) = (LAYOUT.0 as f32, LAYOUT.1 as f32);
    let turned = screen.1 > screen.0;
    // (turned, the box on screen is the layout's height wide and its width high)
    let (bw, bh) = if turned { (lh, lw) } else { (lw, lh) };
    let above = (screen.0, screen.1 - crate::ui::ZOOM_COLUMN_HEIGHT - 8.0 - STATUS_TOP);
    let beside = (screen.0 - ZOOM_COLUMN_WIDTH, screen.1 - 8.0 - STATUS_TOP);
    let fit = |a: (f32, f32)| (a.0 / bw).min(a.1 / bh).max(0.05);
    let (area, scale) = if fit(above) >= fit(beside) { (above, fit(above)) } else { (beside, fit(beside)) };
    LayoutPlacement { left: ((area.0 - bw * scale) / 2.0).max(0.0), top: STATUS_TOP + ((area.1 - bh * scale) / 2.0).max(0.0), y0: 0.0, scale, turned }
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
        // (#148) the briefing history's window
        d.insert(b"Window\\Diplomacy.wnd", rts_lockstep_engine::client::shell_gui::synthetic::diplomacy_wnd());
        // (#151) the popup message's window
        d.insert(b"Window\\InGamePopupMessage.wnd", rts_lockstep_engine::client::shell_gui::synthetic::in_game_popup_message_wnd());
        for (p, b) in crate::ui::synthetic_ui_files() {
            d.insert(p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        let crate::ui::ImagesBuild::Ready(images) = crate::ui::load_mapped_images(&fs) else { panic!("the mapped images load") };
        let ui = ShellUi::load(&fs, text, images.clone(), 1000, &Default::default()).expect("the shell loads");
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

    /// (#78) A click on the window `<file>:<name>` (the last one of that name: a screen made again makes new windows).
    fn click_named(ui: &mut ShellUi, file: &str, name: &str) {
        let full = format!("{file}:{name}");
        let w = ui.wm.tree.windows.len() - 1 - ui.wm.tree.windows.iter().rev().position(|x| x.name() == full.as_bytes()).expect(name);
        let (x, y) = ui.wm.tree.screen_position(w);
        let p = (x as f32 + 5.0, y as f32 + 5.0);
        ui.pointer(gwm::MOUSE_POS, p, false);
        ui.pointer(gwm::LEFT_DOWN, p, false);
        ui.pointer(gwm::LEFT_UP, p, false);
    }

    /// #78: the save/load screen in the client, with a save the client made (the demo game saved through
    /// `crate::saves::save_game`, as the saves panel saves) and a file that does not read. Predicted from
    /// PopupSaveLoad.cpp and GameState.cpp before the run:
    ///
    /// - the folder given to the shell ([`ShellUi::set_saves`]) lists the save (its description "demo at 90") and not
    ///   the file that does not read (`addGameToAvailableList`'s `catch`);
    /// - the main menu's *Load/Replay*, *Load Game*: the save/load screen on top, its list one row, selected;
    /// - *Load*: the shell hidden (`hideShell`), one action [`ShellAction::LoadGame`] of `00000000.sav`, the saves
    ///   panel's own load (its first call, [`crate::saves::PendingLoad::new`], takes the save);
    /// - a failed load's [`ShellUi::show`]: the shell active, the screen's init again;
    /// - *Delete* and *Confirm*: [`ShellAction::DeleteSave`], and the folder's delete removes the file; the list empty.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn the_save_load_screen_loads_through_the_saves_panel() {
        let (w, _) = crate::demo::demo_world();
        let mut c = crate::input::Controls::new(w, crate::demo::DEMO_LOCAL_PLAYER);
        let mut particles = rts_lockstep_engine::client::particle_sys::ParticleSystemManager::new();
        let mut state = rts_lockstep_engine::client::save_game::ClientSaveState::default();
        let bytes = crate::saves::save_game(&mut c, &mut particles, &mut state, None, "demo at 90").expect("the save");
        let mut folder = crate::saves::SaveFolder::default();
        folder.write("00000000.sav", bytes).expect("kept");
        folder.write("00000001.sav", b"junk".to_vec()).expect("kept");
        let mut ui = shell();
        ui.set_saves(&folder);
        assert_eq!(ui.gui.save_files.len(), 2);
        assert!(ui.gui.save_files[1].1.is_none(), "the junk does not read");
        let mut now = 1000;
        ui.frame(now);
        assert!(ui.keyboard(0x39, true, Some(" ")));
        settle(&mut ui, &mut now);
        click(&mut ui, "ButtonLoadReplay", false);
        settle(&mut ui, &mut now);
        click(&mut ui, "ButtonLoadGame", false);
        settle(&mut ui, &mut now);
        settle(&mut ui, &mut now);
        assert_eq!(ui.top_screen().as_deref(), Some(&b"Menus/SaveLoad.wnd"[..]));
        for _ in 0..3 {
            now += 40;
            ui.frame(now);
        }
        settle(&mut ui, &mut now);
        let l = ui.gui.save_load.listbox_games.expect("the list box");
        assert_eq!(rts_lockstep_engine::client::gadget_list_box::get_num_entries(&ui.wm, l), 1);
        let desc = String::from_utf16_lossy(&ui.gui.save_load.available_games[0].save_game_info.description);
        assert_eq!(desc, "demo at 90");
        click_named(&mut ui, "SaveLoad.wnd", "ButtonLoad");
        assert!(!ui.is_active(), "hideShell");
        let base = World::new();
        let mut flow = Flow::default();
        let actions = ui.take_actions(&base, &mut flow);
        assert_eq!(actions, [ShellAction::LoadGame("00000000.sav".to_string())]);
        assert!(crate::saves::PendingLoad::new(folder.files["00000000.sav"].clone(), None).is_ok(), "the saves panel's load takes it");
        // a failed load: the shell again
        ui.show();
        assert!(ui.is_active());
        assert_eq!(ui.top_screen().as_deref(), Some(&b"Menus/SaveLoad.wnd"[..]));
        for _ in 0..3 {
            now += 40;
            ui.frame(now);
        }
        settle(&mut ui, &mut now);
        click_named(&mut ui, "SaveLoad.wnd", "ButtonDelete");
        click_named(&mut ui, "SaveLoad.wnd", "ButtonDeleteConfirm");
        let actions = ui.take_actions(&base, &mut flow);
        assert_eq!(actions, [ShellAction::DeleteSave("00000000.sav".to_string())]);
        assert_eq!(folder.delete("00000000.SAV").as_deref(), Some("00000000.sav"));
        assert!(!folder.files.contains_key("00000000.sav"));
        assert_eq!(crate::saves::page::take_delete().as_deref(), Some("00000000.sav"), "the page removes it from OPFS");
        let l = ui.gui.save_load.listbox_games.expect("the list box");
        assert_eq!(rts_lockstep_engine::client::gadget_list_box::get_num_entries(&ui.wm, l), 0);
        // (~~not reachable: `None`~~ **Corrected 2026-11-10 (#87):** the saves panel's save, under the next name)
        assert_eq!(act(&ShellRequest::SaveGame { filename: Vec::new(), description: Vec::new(), file_type: 0 }, &base, &mut flow), ShellAction::SaveGame { name: None, description: String::new() });
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
        assert!(ShellUi::load(&fs, &TextTable::default(), Arc::new(ImageCollection::new()), 0, &Default::default()).is_none());
        let mut d = MemoryDevice::new();
        for (p, b) in shell_wnd_files() {
            let p: Vec<u8> = p.iter().map(|&c| if c == b'/' { b'\\' } else { c }).collect();
            d.insert(&p, b);
        }
        let mut fs = FileSystem::new(d);
        fs.init(None);
        assert!(ShellUi::load(&fs, &TextTable::default(), Arc::new(ImageCollection::new()), 0, &Default::default()).is_none(), "no WindowTransitions.ini: no shell");
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
        // (~~nothing~~ **Corrected 2026-11-10 (#87):** the quit menu's exit, sent on the World's command list)
        assert_eq!(act(&ShellRequest::ClearGameData, &base, &mut flow), ShellAction::ClearGameData);
        assert!(ui.take_actions(&base, &mut flow).is_empty());
    }

    /// #157: the maps' preview `.tga` files the listing finds are read into the skirmish data (lower case), the
    /// others' bytes left; one the device does not hold is dropped, not kept pending. Predicted: `Maps\Alpha\Alpha.tga`
    /// read as `maps\alpha\alpha.tga`, the `.map` not; nothing pending.
    #[test]
    fn the_map_previews_are_read_from_the_files() {
        let (mut ui, _, _) = shell_with(&TextTable::default());
        let mut d = rts_lockstep_engine::common::file_system::MemoryDevice::new();
        d.insert(b"Maps\\Alpha\\Alpha.tga", b"TGA".to_vec());
        d.insert(b"Maps\\Alpha\\Alpha.map", b"MAP".to_vec());
        let mut fs = FileSystem::new(d);
        fs.init(None);
        ui.set_skirmish_data(&World::new(), &fs);
        assert_eq!(ui.gui.skirmish_data.map_previews.keys().cloned().collect::<Vec<_>>(), vec![b"maps\\alpha\\alpha.tga".to_vec()]);
        assert_eq!(ui.gui.skirmish_data.map_previews.values().next().map(|v| v.as_slice()), Some(&b"TGA"[..]));
        assert!(ui.previews_pending.is_empty());
        assert_eq!(ui.load_map_previews(&fs), 0);
    }

    /// #89: the skirmish screen in the client, from the synthetic files (no EA file), on a World whose map cache holds
    /// one two-player multiplayer map (Alpha) and whose settings offer one starting amount (the global default, 0).
    /// Predicted from SkirmishGameOptionsMenu.cpp before the run:
    ///
    /// - `set_skirmish_data` copies the World's cache (Alpha) and the map files the file system holds (none here);
    /// - the screen pushed (as the main menu's *Skirmish* pushes it) and settled: `TheSkirmishGameInfo` on Alpha (the
    ///   cache's first multiplayer map, no `Skirmish.ini`), slot 0 the player, slot 1 an easy computer;
    ///   ~~`populateSkirmishBattleHonors` recorded as untranslated~~ (**corrected 2026-11-12, #157:** translated: the
    ///   honours list filled), nothing undefined;
    /// - *Start*: one request, `StartSkirmish` (`GAME_SKIRMISH`, the speed 30: the global data's limit, none here, is
    ///   0, so the slider's 15 .. 61 clamp gives 15, and `reallyDoStart` keeps 15), which [`act`] turns into the flow's
    ///   skirmish waiting for `maps\\alpha\\alpha.map`; the base World only read; a map the cache lacks fails.
    ///
    /// **Result (2026-11-10):** as predicted but for the speed: **30, not 15**. A new World has no global data at all
    /// (`the_global_data()` is `None`), and `SkirmishData::from_world` then takes 30 for `m_framesPerSecondLimit`, so
    /// the slider starts at 30 (the prediction took the missing data for a limit of 0). The assertion now says 30.
    #[test]
    fn the_skirmish_screen_starts_the_flows_skirmish() {
        use rts_lockstep_engine::common::map_cache::MapMetaData;
        use rts_lockstep_engine::common::money::Money;
        use rts_lockstep_engine::common::unicode_string::UnicodeString;
        const ALPHA: &[u8] = b"maps\\alpha\\alpha.map";
        let (mut ui, _, fs) = shell_with(&TextTable::default());
        let mut base = World::new();
        {
            let st = Arc::make_mut(&mut base.statics);
            let display = UnicodeString::try_from_chars(&"Alpha (2)".encode_utf16().collect::<Vec<u16>>()).expect("name");
            st.map_cache.maps.insert(ALPHA.to_vec(), MapMetaData { display_name: display, num_players: 2, is_multiplayer: true, ..Default::default() });
            st.multiplayer_settings.add_starting_money_choice(Money::new(), true);
        }
        ui.set_skirmish_data(&base, &fs);
        assert!(ui.gui.skirmish_data.map_cache.maps.contains_key(ALPHA));
        assert!(ui.gui.skirmish_data.files.is_empty());
        let mut now = 1000;
        ui.frame(now);
        sh::push(&mut ui.wm, &mut ui.gui, b"Menus/SkirmishGameOptionsMenu.wnd", false);
        settle(&mut ui, &mut now);
        for _ in 0..3 {
            now += 40;
            ui.frame(now);
        }
        settle(&mut ui, &mut now);
        assert_eq!(ui.top_screen().as_deref(), Some(&b"Menus/SkirmishGameOptionsMenu.wnd"[..]));
        let g = ui.gui.skirmish.game.clone().expect("TheSkirmishGameInfo");
        assert_eq!(g.map_name, ALPHA);
        assert_eq!((g.slots[0].state, g.slots[1].state), (rts_lockstep_engine::common::game_info::slot_state::PLAYER, rts_lockstep_engine::common::game_info::slot_state::EASY_AI));
        // (#157: the honours translated: `ListboxInfo` filled, nothing recorded)
        assert!(!ui.gui.untranslated.iter().any(|f| f == b"populateSkirmishBattleHonors"));
        let info = ui.wm.tree.find_by_name(b"SkirmishGameOptionsMenu.wnd:ListboxInfo").expect("ListboxInfo");
        assert!(rts_lockstep_engine::client::gadget_list_box::get_num_entries(&ui.wm, info) > 1);
        assert!(ui.wm.undefined.is_none(), "{:?}", ui.wm.undefined);
        ui.gui.requests.clear();
        click_named(&mut ui, "SkirmishGameOptionsMenu.wnd", "ButtonStart");
        let [ShellRequest::StartSkirmish { mode, max_fps, .. }] = ui.gui.requests.as_slice() else { panic!("{:?}", ui.gui.requests) };
        assert_eq!((*mode, *max_fps), (rts_lockstep_engine::common::recorder::game_mode::SKIRMISH, 30));
        let before = base.statics.map_cache.maps.clone();
        let mut flow = Flow::default();
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::SkirmishChosen]);
        assert_eq!(flow.waiting_for(), Some(ALPHA));
        assert!(matches!(flow.pending.as_ref().map(|p| &p.choice), Some(MissionChoice::Skirmish { .. })));
        assert_eq!(base.statics.map_cache.maps, before, "the base World is only read");
        let mut gone = g.clone();
        gone.map_name = b"maps\\gone\\gone.map".to_vec();
        assert!(matches!(act(&ShellRequest::StartSkirmish { game: Box::new(gone), mode: 2, difficulty: 1, max_fps: 30 }, &base, &mut flow), ShellAction::Failed(_)));
    }

    /// #89 part 2: the translated score screen in the client, after a campaign mission won (a World with the campaign
    /// `usa` at its first mission, victorious; local player 1, human, 777 skill points). Predicted from ScoreScreen.cpp
    /// before the run:
    ///
    /// - [`ShellUi::score_screen`] pushes `Menus/ScoreScreen.wnd` over the (hidden) main menu and shows the shell;
    ///   `DontShowMainMenu` is set;
    /// - the first frame (`finishSinglePlayerInit`) goes to M02 and asks for the mission save: the action
    ///   [`ShellAction::MissionSaved`] (or [`ShellAction::Failed`] if the World without a game cannot save; the
    ///   prediction: saved, the description `GUI:MissionSave` formatted);
    /// - *Continue*: the shell hidden, [`ShellAction::CampaignChosen`], the flow waiting for `Maps\USA02\USA02.map` in
    ///   the World that played, its campaign the screen's (rank points 777).
    #[test]
    fn the_score_screen_continues_the_campaign_in_the_flow() {
        let (mut ui, _, _) = shell_with(&TextTable::default());
        let mut now = 1000;
        settle(&mut ui, &mut now);
        let mut w = World::new();
        w.campaign.init(CAMPAIGN).expect("Campaign.ini");
        w.campaign.set_campaign(b"usa");
        w.campaign.set_victorious(true);
        w.players.set_player_count(2);
        w.players.friend_set_local_index(Some(1));
        w.players.get_nth_mut(1).expect("player 1").skill_points = 777;
        let g = rts_lockstep_engine::client::score_screen::ScoreScreenGame::from_world(&w);
        let mut flow = Flow { played: Some(w.clone()), ..Flow::default() };
        ui.hide();
        ui.score_screen(g);
        assert!(ui.is_active());
        assert_eq!(ui.top_screen().as_deref(), Some(&b"Menus/ScoreScreen.wnd"[..]));
        assert!(ui.gui.score_screen.globals.dont_show_main_menu);
        assert!(ui.wm.undefined.is_none(), "{:?}", ui.wm.undefined);
        now += 40;
        ui.frame(now);
        let actions = ui.take_actions(&w, &mut flow);
        assert!(matches!(actions.as_slice(), [ShellAction::MissionSaved(..)]), "{actions:?}");
        settle(&mut ui, &mut now);
        click_named(&mut ui, "ScoreScreen.wnd", "ButtonContinue");
        assert!(!ui.is_active());
        assert_eq!(ui.take_actions(&w, &mut flow), [ShellAction::CampaignChosen]);
        assert_eq!(flow.waiting_for(), Some(&b"Maps\\USA02\\USA02.map"[..]));
        let p = flow.pending.as_ref().expect("pending");
        assert_eq!(p.world.campaign.current_rank_points(), 777);
        assert!(ui.wm.undefined.is_none(), "{:?}", ui.wm.undefined);
    }

    /// #98: the replay folder and the challenge menu's start in the client, predicted from ReplayMenu.cpp,
    /// PopupReplay.cpp and ChallengeMenu.cpp before the run:
    ///
    /// - [`ShellUi::set_replays`] gives the host each file of the folder under `Replays\`, with its listing read on a
    ///   copy of the base World: bytes that are not a replay have none (left out of the lists);
    /// - the playback of a file the folder lacks, and of one that does not read, fail ([`ShellAction::Failed`]); the
    ///   flow waits for nothing;
    /// - the folder's write, delete and export come back as the actions the app keeps ([`ShellAction::ReplayWritten`]
    ///   by the file's name, ...);
    /// - the challenge menu's `StartChallenge { 0, 2 }` is the campaign panel's challenge: the flow waits for the
    ///   general's campaign's first map (`Maps\GC01\GC01.map`) at difficulty 2; the base World only read.
    #[test]
    fn the_replay_folder_and_the_challenge_reach_the_flow() {
        const CHALLENGE_CAMPAIGN: &[u8] = b"Campaign Challenge_0\n IsChallengeCampaign = Yes\n FirstMission = M01\n Mission M01\n  Map = Maps\\GC01\\GC01.map\n End\nEnd\n";
        const CHALLENGE: &[u8] = b"ChallengeGenerals\n GeneralPersona0\n  StartsEnabled = Yes\n  Campaign = Challenge_0\n  PlayerTemplate = FactionAmerica\n End\nEnd\n";
        let (mut ui, _, _) = shell_with(&TextTable::default());
        let mut base = World::new();
        base.campaign.init(CHALLENGE_CAMPAIGN).expect("Campaign.ini");
        base.challenge_generals.init(CHALLENGE).expect("ChallengeMode.ini");
        let mut folder = crate::replays::ReplayFolder::default();
        folder.files.insert("bad.rep".to_string(), b"NOT A REPLAY".to_vec());
        ui.set_replays(&folder, &base);
        assert_eq!(ui.gui.replays.files.len(), 1);
        assert_eq!(ui.gui.replays.files[0].0, b"Replays\\bad.rep");
        assert_eq!(ui.gui.replays.files[0].2, None);
        let mut flow = Flow::default();
        ui.gui.requests.push(ShellRequest::PlaybackReplay { filename: b"none.rep".to_vec() });
        ui.gui.requests.push(ShellRequest::PlaybackReplay { filename: b"BAD.rep".to_vec() });
        let a = ui.take_actions(&base, &mut flow);
        assert!(matches!(a.as_slice(), [ShellAction::Failed(_), ShellAction::Failed(_)]), "{a:?}");
        assert!(flow.pending.is_none());
        assert_eq!(act(&ShellRequest::WriteReplay { path: b"Replays\\Best.rep".to_vec(), bytes: b"B".to_vec() }, &base, &mut flow), ShellAction::ReplayWritten("Best.rep".into(), b"B".to_vec()));
        assert_eq!(act(&ShellRequest::DeleteReplay { path: b"Replays\\Best.rep".to_vec() }, &base, &mut flow), ShellAction::ReplayDeleted("Best.rep".into()));
        assert_eq!(act(&ShellRequest::ExportReplay { name: b"x.rep".to_vec(), bytes: b"X".to_vec() }, &base, &mut flow), ShellAction::ReplayExported("x.rep".into(), b"X".to_vec()));
        let before = base.campaign.clone();
        assert_eq!(act(&ShellRequest::StartChallenge { general: 0, difficulty: 2 }, &base, &mut flow), ShellAction::CampaignChosen);
        assert_eq!(flow.waiting_for(), Some(&b"Maps\\GC01\\GC01.map"[..]));
        let p = flow.pending.as_ref().expect("pending");
        assert_eq!((&p.choice, p.difficulty), (&MissionChoice::Challenge(0), 2));
        assert_eq!(base.campaign, before, "the base World is only read");
        assert!(matches!(act(&ShellRequest::StartChallenge { general: 40, difficulty: 2 }, &base, &mut flow), ShellAction::Failed(_)));
    }

    /// #78: the shell drawn from the window manager's draw list, and hidden when a game starts. Predicted from
    /// `winRepaint` and `W3DGadgetPushButtonDraw` before the run:
    ///
    /// - once the main panel is shown, the quads hold each of its 7 buttons' text (`GUI:ButtonSinglePlayer` given the
    ///   text "Single Player" here), placed from the button's top left inward, inside its rectangle (10..110, 110..130);
    /// - no quad of the hidden panels' buttons (`ButtonUSA`'s text);
    /// - `hideShell`: not active, the main menu's shutdown run, nothing drawn, the pointer ignored.
    /// (#88, #93) The options screen over the user data folder, predicted from OptionsMenu.cpp and GlobalData.cpp
    /// before the run: `Options.ini`'s `ScrollFactor = 100` makes the keyboard pan twice the default's (factor 1.0
    /// over the default 0.5) and `UseAlternateMouse = yes` the alternate mouse; the screen opened by *Options* and
    /// accepted with the music slider at 25 asks the client's audio for 0.25 music ([`ShellAction::SetVolume`]) and
    /// writes `Options.ini` back (CR LF lines, `MusicVolume = 25`, the scroll factor kept), which the folder keeps.
    #[test]
    fn the_options_screen_reads_and_writes_the_user_data_folder() {
        let mut ui = shell();
        let mut folder = crate::user_data::UserDataFolder::default();
        folder.files.insert("Options.ini".into(), b"ScrollFactor = 100\r\nUseAlternateMouse = yes\r\n".to_vec());
        ui.set_user_data(&folder, None);
        let o = ui.client_options();
        assert_eq!((o.keyboard_scroll, o.alternate_mouse), (2.0, true));
        ui.frame(1000);
        assert!(ui.keyboard(0x39, true, Some(" ")));
        let mut now = 1000;
        settle(&mut ui, &mut now);
        click(&mut ui, "ButtonOptions", false);
        let music = ui.wm.tree.find_by_name(b"OptionsMenu.wnd:SliderMusicVolume").expect("the options screen");
        rts_lockstep_engine::client::gadget_slider::slider_set_position(&mut ui.wm, &mut ui.gui, music, 25);
        click_named(&mut ui, "OptionsMenu.wnd", "ButtonAccept");
        let (base, mut flow) = (World::new(), Flow::default());
        let actions = ui.take_actions(&base, &mut flow);
        assert!(actions.contains(&ShellAction::SetVolume(0.25, 0x11)), "{actions:?}");
        for (name, bytes) in ui.take_user_data_writes() {
            folder.write(&name, bytes).expect("kept");
        }
        let text = String::from_utf8(folder.files["Options.ini"].clone()).expect("text");
        assert!(text.contains("MusicVolume = 25\r\n") && text.contains("ScrollFactor = 100\r\n"), "{text}");
    }

    /// #87: the in-game quit menu in the client, over the demo game (the shell hidden, as a game's start hides it),
    /// predicted from QuitMenu.cpp and PopupSaveLoad.cpp before the run:
    ///
    /// - before Escape nothing is drawn and the window manager takes no input; [`quit_menu_game`] of the demo World
    ///   reads a game (its mode not `GAME_NONE`), and Escape ([`ShellUi::toggle_quit_menu`]) shows `QuitMenu.wnd`
    ///   (full: the demo is not a replay or a network game) with the action [`ShellAction::SetGamePaused`] (true);
    ///   after the transitions the menu's buttons are drawn (the quads hold *Save/Load*'s text) and it takes input;
    /// - *Save/Load* clicked: the popup; *Save* on the new-game row, the description "Mine", *Confirm*: the action
    ///   [`ShellAction::SaveGame`] with no name (the next free name) and "Mine";
    /// - *Exit*, *Yes*: the actions [`ShellAction::ClearGameData`] then [`ShellAction::SetGamePaused`] (false); the
    ///   menu gone; [`send_clear_game_data`] then one logic frame of the demo World clears its game (`GAME_NONE`) with the
    ///   score screen's copy made (`score_screen_game_over`);
    /// - the World is only read until the message: its CRC before and after the menu's toggles is the same.
    ///
    /// - (#108) the quit menu's flag is set while it is shown and still set after *Exit* (`destroyQuitMenu` leaves it);
    ///   [`ShellUi::clear_game_data_windows`] once the World's game is cleared clears it and asks for no action.
    ///
    /// **Result (2026-11-10):** one miss, then as predicted. The first run's Escape did nothing: the demo World's
    /// `isGameEnding` is true (its script engine was never given `newMap`: the end-game timer is the constructor's 0),
    /// and `ToggleQuitMenu` returns for an ending game. The test now asserts that and opens the menu with the game not
    /// ending, as a game started by `startNewGame` has it. **Corrected 2026-11-10 (#107):** the demo World now has had
    /// `newMap`; the test asserts its game is not ending and opens the menu on it as read.
    #[test]
    fn escape_opens_the_quit_menu_whose_save_and_exit_reach_the_game() {
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        let mut text = TextTable::default();
        text.strings.insert(b"GUI:ButtonSaveLoad".to_vec(), rts_lockstep_engine::common::unicode_string::UnicodeString::try_from_chars(&"Save/Load".encode_utf16().collect::<Vec<_>>()).expect("text"));
        let (mut ui, _, _) = shell_with(&text);
        let mut now = 1000;
        settle(&mut ui, &mut now);
        ui.hide();
        settle(&mut ui, &mut now);
        assert!(!ui.takes_input() && ui.draw().is_empty());
        let (mut w, _) = crate::demo::demo_world();
        let crc = w.logic_crc(CRC_RECALC, None).expect("the CRC");
        // (first run, a miss: the demo World is made without `ScriptEngine::newMap` or `reset`, so its end-game timer is
        // the constructor's 0 and `isGameEnding` is true: Escape did nothing, as upstream's would. ~~The game here is a
        // started game's: the timer at -1, as `newMap` leaves it; the demo's own timer is a defect, issue draft~~
        // **Corrected 2026-11-10 (#107):** the demo World is given `newMap` (`crate::demo::demo_world`), so its game is
        // not ending and the menu opens on what the World reads)
        let game = quit_menu_game(&w, true);
        assert!(game.is_in_game() && !game.game_ending, "{game:?}");
        ui.toggle_quit_menu(game.clone());
        settle(&mut ui, &mut now);
        let (base, mut flow) = (World::new(), Flow::default());
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::SetGamePaused(true)]);
        assert!(ui.takes_input() && ui.quit_menu_visible());
        assert!(ui.draw().iter().any(|q| q.text.as_deref() == Some("Save/Load")));
        click_named(&mut ui, "QuitMenu.wnd", "ButtonSaveLoad");
        settle(&mut ui, &mut now);
        click_named(&mut ui, "PopupSaveLoad.wnd", "ButtonSave");
        let entry = ui.gui.save_load.edit_desc.expect("the description entry");
        rts_lockstep_engine::client::gadget_text_entry::set_text(&mut ui.wm, &mut ui.gui, Some(entry), &"Mine".encode_utf16().collect::<Vec<_>>());
        click_named(&mut ui, "PopupSaveLoad.wnd", "ButtonSaveDescConfirm");
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::SaveGame { name: None, description: "Mine".to_string() }]);
        click_named(&mut ui, "QuitMenu.wnd", "ButtonExit");
        click_named(&mut ui, "QuitMessageBox.wnd", "ButtonYes");
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::ClearGameData, ShellAction::SetGamePaused(false)]);
        assert!(ui.wm.undefined.is_none(), "{:?}", ui.wm.undefined);
        settle(&mut ui, &mut now);
        assert!(!ui.takes_input(), "the menu is gone");
        assert_eq!(w.logic_crc(CRC_RECALC, None).expect("the CRC"), crc, "the World only read");
        send_clear_game_data(&mut w, crate::demo::DEMO_LOCAL_PLAYER);
        w.engine_update().expect("the frame");
        assert_eq!(w.cuts.game_loop.game_mode, rts_lockstep_engine::common::recorder::game_mode::NONE);
        assert!(w.score_screen_game_over().is_some(), "clearGameData's score screen");
        // (#108) the flag `destroyQuitMenu` leaves, cleared when the app sees the game cleared (`InGameUI::reset`), and
        // `closeWindows` asks nothing more (the menu already hidden)
        assert!(ui.quit_menu_visible() && w.game_cleared.is_some());
        ui.clear_game_data_windows();
        assert!(!ui.quit_menu_visible());
        assert!(ui.take_actions(&base, &mut flow).is_empty());
    }

    /// #110: the quit menu's *Restart* reaches the flow, over the demo game (not ending: #107's `newMap`). Predicted from QuitMenu.cpp before the run:
    ///
    /// - [`quit_menu_game`] of the demo World reads what the restart needs: no replay file, not recording (the demo has
    ///   no recorder), the map not a save's (`pristine_map_name` `None`), no map name (the demo World has no global data);
    /// - Escape: the full menu and [`ShellAction::SetGamePaused`] (true); *Restart* (its text `GUI:ButtonRestart`, the
    ///   demo is a skirmish), *Yes*: the menu gone and one action, [`ShellAction::RestartMission`] with `GAME_SKIRMISH`,
    ///   the script engine's difficulty, no rank points, the frame limit ~~30~~ 0 (the gate's global data has none), no
    ///   map, no replay, not recording (no pause
    ///   request: `restartMissionMenu` does not unpause);
    /// - [`Flow::restart`] of it fails (no map: the demo cannot be restarted, `NoMission`) and nothing waits; with the
    ///   map named, a copy of the running World waits for it (`MissionChoice::Restart`), the World only read (its CRC
    ///   unchanged, the copy's the same);
    /// - [`Flow::poll`] with the map's folder absent mounts nothing and starts the restart, which fails (the map is not
    ///   on the World's disk): an error, and the game's kept choice ([`Flow::playing`]) is still the one given.
    ///   **Result (2026-11-10):** three misses over three runs, each recorded where it is asserted: the restart box is
    ///   `MessageBox.wnd`; the frame limit read is 0; and the restart without its map **starts** rather than failing
    ///   (the kept choice held, labelled with the map).
    #[test]
    fn the_quit_menus_restart_reaches_the_flow() {
        use rts_lockstep_engine::common::recorder::game_mode;
        use rts_lockstep_engine::logic::world::game_logic_update::CRC_RECALC;
        let (mut ui, _, fs) = shell_with(&TextTable::default());
        let mut now = 1000;
        settle(&mut ui, &mut now);
        ui.hide();
        settle(&mut ui, &mut now);
        let (mut w, _) = crate::demo::demo_world();
        let crc = w.logic_crc(CRC_RECALC, None).expect("the CRC");
        let game = quit_menu_game(&w, true);
        assert_eq!((game.replay_file.as_slice(), game.recording, game.pristine_map_name.as_ref(), game.map_name.as_slice()), (&b""[..], false, None, &b""[..]));
        assert!(!game.game_ending, "#107: the demo has had newMap");
        ui.toggle_quit_menu(game.clone());
        settle(&mut ui, &mut now);
        let (base, mut flow) = (World::new(), Flow::default());
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::SetGamePaused(true)]);
        click_named(&mut ui, "QuitMenu.wnd", "ButtonRestart");
        // (first run, a miss of the test's: the restart box is `MessageBoxYesNo`, `Menus/MessageBox.wnd`, not the exit's
        // `QuitMessageBoxYesNo`)
        click_named(&mut ui, "MessageBox.wnd", "ButtonYes");
        // (first run, a miss: the frame limit is **0, not 30**: the demo World has the gate's global data, which sets no
        // limit; `MSG_NEW_GAME` takes the global limit for a value outside 1 .. 1000)
        let r = Restart { game_mode: game_mode::SKIRMISH, difficulty: w.script_engine.game_difficulty, rank_points: 0, fps: 0, map_name: Vec::new(), replay_file: Vec::new(), stop_recording: false };
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::RestartMission(r.clone())]);
        assert!(ui.wm.undefined.is_none(), "{:?}", ui.wm.undefined);
        settle(&mut ui, &mut now);
        assert!(!ui.takes_input(), "the menu is gone");
        assert!(flow.restart(&w, r.clone()).is_err() && flow.pending.is_none(), "no map");
        let kept = MissionChoice::Campaign { campaign: b"usa".to_vec(), mission: b"m01".to_vec() };
        flow.playing = Some(kept.clone());
        let named = Restart { map_name: b"Maps\\Gate\\Gate.map".to_vec(), ..r };
        flow.restart(&w, named.clone()).expect("the restart waits");
        assert_eq!(flow.waiting_for(), Some(&b"Maps\\Gate\\Gate.map"[..]));
        let p = flow.pending.as_ref().expect("the pending restart");
        assert_eq!(p.choice, MissionChoice::Restart(named));
        assert_eq!(p.world.clone().logic_crc(CRC_RECALC, None).expect("the CRC"), crc, "a copy of the running World");
        assert_eq!(w.logic_crc(CRC_RECALC, None).expect("the CRC"), crc, "the World only read");
        let started = flow.poll(&fs).expect("the start ran");
        // (first run, a miss: the restart **starts** (a local player within 30 frames), though the map is not on the
        // World's disk: the demo World's map was never a file (`gate_map_world` builds it in memory), and its start
        // does not fail without one. Not investigated further: issue draft)
        let started = started.expect("the restart started");
        eprintln!("the restart without its map: {:?}", started.label);
        assert_eq!(flow.playing, Some(kept), "the game's choice kept");
        assert!(started.label.contains("Maps\\Gate\\Gate.map"), "{}", started.label);
    }

    /// #151: the popup message (`popupMessage`, InGamePopupMessage.cpp) on the synthetic `InGamePopupMessage.wnd`, in a
    /// game (the shell hidden). Predicted from InGameUI.cpp and InGamePopupMessage.cpp before the run:
    ///
    /// - a pausing message at (25 %, 40 %), 300 wide, on the 800x600 layout: the box at (200, 240), 300 wide; the label in
    ///   the briefing history; the game paused (`SetGamePaused { paused: true, .. }`); the window up, modal, with the
    ///   focus, drawn and taking the input; the text in the given colour;
    /// - Enter released (its `GBM_SELECTED` on OK): a pausing message is cleared at once, the layout destroyed, the game
    ///   unpaused; nothing drawn;
    /// - a message that does not pause: its OK (Escape) sends `MSG_CLEAR_INGAME_POPUP_MESSAGE` (`PopupCall::ClearMessage`)
    ///   and the box stays until the logic's clear comes back (`clear_popup_message`);
    /// - the game's end (`InGameUI::reset`) clears one that is up.
    #[test]
    fn a_popup_message_shows_and_closes_as_upstream() {
        use rts_lockstep_engine::client::shell_gui::PopupCall;
        let mut text = TextTable::default();
        text.strings.insert(b"SCRIPT:Pop".to_vec(), rts_lockstep_engine::common::unicode_string::UnicodeString::try_from_chars(&"Reinforcements are on the way".encode_utf16().collect::<Vec<_>>()).expect("text"));
        let (mut ui, _, _) = shell_with(&text);
        let mut now = 1000;
        settle(&mut ui, &mut now);
        ui.hide();
        settle(&mut ui, &mut now);
        assert!(!ui.takes_input());
        let color = rts_lockstep_engine::common::ini::game_make_color(255, 255, 0, 255);
        ui.popup_message(b"SCRIPT:Pop", 25, 40, 300, color, true, false);
        assert_eq!(ui.take_popup_calls(), vec![PopupCall::SetGamePaused { paused: true, pause_music: false }]);
        assert_eq!(ui.briefing_list(), &[b"SCRIPT:Pop".to_vec()]);
        let parent = ui.gui.popup.parent.expect("the parent");
        let st = ui.gui.popup.static_text_message.expect("the text");
        assert_eq!((ui.wm.tree.windows[parent].region.0, ui.wm.tree.windows[parent].region.1, ui.wm.tree.windows[parent].size.0), (200, 240, 300));
        assert_eq!(ui.wm.tree.windows[st].inst.enabled_text.color, color);
        assert_eq!(ui.wm.modal.first(), Some(&parent));
        assert_eq!(ui.wm.keyboard_focus, Some(parent));
        assert!(ui.popup_shown() && ui.takes_input() && !ui.draw().is_empty());
        // Enter, pressed then released: the release is OK's click
        ui.keyboard(0x1C, true, None);
        ui.keyboard(0x1C, false, None);
        assert!(ui.gui.popup.popup_message_data.is_none(), "cleared at once");
        assert_eq!(ui.take_popup_calls(), vec![PopupCall::SetGamePaused { paused: false, pause_music: false }]);
        assert!(!ui.popup_shown() && ui.draw().is_empty());
        ui.popup_message(b"SCRIPT:Pop", 50, 50, 10, color, false, false);
        assert_eq!(ui.gui.popup.popup_message_data.as_ref().map(|d| d.width), Some(50));
        ui.keyboard(0x01, true, None);
        ui.keyboard(0x01, false, None);
        assert_eq!(ui.take_popup_calls(), vec![PopupCall::ClearMessage]);
        assert!(ui.popup_shown(), "up until the logic's clear");
        ui.clear_popup_message();
        assert!(!ui.popup_shown());
        ui.popup_message(b"SCRIPT:Pop", 50, 50, 200, color, false, false);
        ui.clear_game_data_windows();
        assert!(ui.gui.popup.popup_message_data.is_none() && !ui.popup_shown());
    }

    /// #155: a pausing popup message with no shell (`WindowlessPopup`), then a shell. Predicted from InGameUI.cpp's
    /// `popupMessage` before the run:
    ///
    /// - without a window manager a pausing message at (25 %, 40 %), 300 wide, on an 800x600 display is held (at (200,
    ///   240), 300 wide, no layout), its label goes to the history and the game is paused, in that order (`popupMessage`
    ///   calls `UpdateDiplomacyBriefingText` before `setGamePaused`); nothing is recorded undefined;
    /// - a second message replaces it: the first's clear unpauses, then the second's label and pause; one is held;
    /// - the logic's clear unpauses and holds none; `InGameUI::reset` likewise;
    /// - a held pausing message given to the shell: its box at (200, 240), up, modal, with the focus; no pause call made
    ///   again; Enter (OK) clears it and unpauses once;
    /// - the long message's text is drawn as the measured lines: as many lines (each 18 apart: Arial 12) as fit the static
    ///   text's measured height, which its window (the measure plus 7) holds: no line cut off.
    #[test]
    fn a_pausing_popup_without_a_shell_pauses_and_is_shown_when_the_shell_comes() {
        use rts_lockstep_engine::client::in_game_popup_message::{WindowlessCall, WindowlessPopup};
        use rts_lockstep_engine::client::shell_gui::PopupCall;
        let mut text = TextTable::default();
        let long = "The convoy is under attack. Hold the bridge until the reinforcements arrive from the north.";
        text.strings.insert(b"SCRIPT:Pop".to_vec(), rts_lockstep_engine::common::unicode_string::UnicodeString::try_from_chars(&long.encode_utf16().collect::<Vec<_>>()).expect("text"));
        let fetch = |l: &[u8]| {
            use rts_lockstep_engine::common::science::GameTextFetch;
            text.fetch(l).str().to_vec()
        };
        let mut w = WindowlessPopup { display: (800, 600), ..Default::default() };
        w.popup_message(fetch, b"SCRIPT:Pop", 25, 40, 300, 7, true, false);
        let d = w.popup.popup_message_data.clone().expect("held");
        assert_eq!((d.x, d.y, d.width, d.layout, d.pause), (200, 240, 300, None, true));
        assert_eq!(std::mem::take(&mut w.calls), vec![WindowlessCall::Briefing(b"SCRIPT:Pop".to_vec()), WindowlessCall::SetGamePaused { paused: true, pause_music: false }]);
        assert_eq!(w.popup.undefined, None);
        w.popup_message(fetch, b"SCRIPT:Two", 0, 0, 100, 7, true, true);
        assert_eq!(std::mem::take(&mut w.calls), vec![WindowlessCall::SetGamePaused { paused: false, pause_music: false }, WindowlessCall::Briefing(b"SCRIPT:Two".to_vec()), WindowlessCall::SetGamePaused { paused: true, pause_music: true }]);
        w.clear_popup_message_data();
        assert_eq!(std::mem::take(&mut w.calls), vec![WindowlessCall::SetGamePaused { paused: false, pause_music: true }]);
        assert!(w.popup.popup_message_data.is_none() && w.take_held().is_none());
        w.popup_message(fetch, b"SCRIPT:Pop", 25, 40, 300, 7, true, false);
        w.calls.clear();
        let held = w.take_held().expect("held");
        assert!(w.calls.is_empty() && w.popup.popup_message_data.is_none());
        let (mut ui, _, _) = shell_with(&text);
        let mut now = 1000;
        settle(&mut ui, &mut now);
        ui.hide();
        settle(&mut ui, &mut now);
        ui.show_held_popup_message(held);
        assert!(ui.take_popup_calls().is_empty(), "the pause was made when the message came");
        let parent = ui.gui.popup.parent.expect("the parent");
        let st = ui.gui.popup.static_text_message.expect("the text");
        assert_eq!((ui.wm.tree.windows[parent].region.0, ui.wm.tree.windows[parent].region.1, ui.wm.tree.windows[parent].size.0), (200, 240, 300));
        assert_eq!(ui.wm.modal.first(), Some(&parent));
        assert!(ui.popup_shown() && ui.takes_input());
        // the text drawn: the measured lines, inside the static text's window
        let quads = ui.draw();
        let t = quads.iter().find(|q| q.text.as_deref().is_some_and(|t| t.starts_with("The convoy"))).expect("the text drawn");
        let lines = t.text.as_deref().map_or(0, |t| t.split('\n').count());
        let (sw, sh) = ui.wm.tree.windows[st].size;
        eprintln!("popup text: {lines} lines {:?}, its window {sw}x{sh}, the quad {:?}", t.text, t.rect);
        assert!(lines >= 2, "{t:?}");
        assert_eq!(t.line_px, Some(18.0));
        assert_eq!(t.rect[3] - t.rect[1], 18.0 * lines as f32, "the box is the lines' height");
        assert!(18 * lines as i32 <= sh, "every line inside the window's {sh}");
        assert!(long.replace(' ', "") == t.text.as_deref().unwrap_or("").replace(['\n', ' '], ""), "no word lost");
        ui.keyboard(0x1C, true, None);
        ui.keyboard(0x1C, false, None);
        assert_eq!(ui.take_popup_calls(), vec![PopupCall::SetGamePaused { paused: false, pause_music: false }]);
        assert!(!ui.popup_shown());
    }

    /// #148: the briefing history from the communicator button (`ToggleDiplomacy(FALSE)`), on the synthetic
    /// `Diplomacy.wnd`. Predicted from Diplomacy.cpp before the run:
    ///
    /// - two briefings and a repeat before the window exists: the history holds two, nothing is drawn;
    /// - the toggle makes the window (shown, taking the input), its solo list box filled with the two, the solo parent
    ///   shown and the in-game parent hidden; a third briefing goes straight into the box (three entries);
    /// - the toggle again slides it out (`HideDiplomacy(FALSE)` with the windows animated): still shown until the slide
    ///   ends, then hidden; the toggle shows it again (`winHide(FALSE)`, the box kept: three entries);
    /// - its `ButtonHide` hides it the same way; the quit menu's Escape (`HideDiplomacy()`, immediate) hides it at once;
    /// - the game's end (`clearGameData`: `closeWindows`' `ResetDiplomacy`, `InGameUI::reset`'s clear) destroys the window
    ///   and empties the history.
    #[test]
    fn the_communicator_shows_the_briefing_history() {
        use rts_lockstep_engine::client::diplomacy::DiplomacyGame;
        let (mut ui, _, _) = shell_with(&TextTable::default());
        let mut now = 1000;
        settle(&mut ui, &mut now);
        ui.hide();
        settle(&mut ui, &mut now);
        for l in [&b"SCRIPT:Brief1"[..], b"SCRIPT:Brief2", b"SCRIPT:Brief1"] {
            ui.briefing(l);
        }
        assert_eq!(ui.gui.diplomacy.briefing_list.len(), 2);
        assert!(!ui.takes_input() && ui.draw().is_empty());
        // (60 frames: the slide from the top moves a step an update, from 690 above the screen: 21 to 40 updates)
        let frames = |ui: &mut ShellUi, now: &mut u32| {
            for _ in 0..60 {
                *now += 40;
                ui.frame(*now);
            }
        };
        ui.toggle_diplomacy(DiplomacyGame::default());
        frames(&mut ui, &mut now);
        assert!(ui.diplomacy_shown() && ui.takes_input(), "the window is up");
        let list = ui.wm.tree.find_by_name(b"Diplomacy.wnd:ListboxSolo").expect("the list box");
        assert_eq!(rts_lockstep_engine::client::gadget_list_box::get_num_entries(&ui.wm, list), 2);
        let hidden = |ui: &ShellUi, n: &str| ui.wm.is_hidden(ui.wm.tree.find_by_name(format!("Diplomacy.wnd:{n}").as_bytes()));
        assert!(!hidden(&ui, "SoloParent") && hidden(&ui, "InGameParent"));
        assert!(!ui.draw().is_empty());
        ui.briefing(b"SCRIPT:Brief3");
        assert_eq!(rts_lockstep_engine::client::gadget_list_box::get_num_entries(&ui.wm, list), 3);
        ui.toggle_diplomacy(DiplomacyGame::default());
        assert!(ui.diplomacy_shown(), "sliding out");
        frames(&mut ui, &mut now);
        assert!(!ui.diplomacy_shown(), "hidden once the slide ends");
        ui.toggle_diplomacy(DiplomacyGame::default());
        frames(&mut ui, &mut now);
        assert!(ui.diplomacy_shown());
        assert_eq!(rts_lockstep_engine::client::gadget_list_box::get_num_entries(&ui.wm, list), 3);
        click_named(&mut ui, "Diplomacy.wnd", "ButtonHide");
        frames(&mut ui, &mut now);
        assert!(!ui.diplomacy_shown(), "ButtonHide");
        ui.toggle_diplomacy(DiplomacyGame::default());
        frames(&mut ui, &mut now);
        assert!(ui.diplomacy_shown());
        let (w, _) = crate::demo::demo_world();
        ui.toggle_quit_menu(quit_menu_game(&w, true));
        assert!(!ui.diplomacy_shown(), "the quit menu's HideDiplomacy");
        ui.clear_game_data_windows();
        assert!(ui.gui.diplomacy.the_window.is_none() && ui.gui.diplomacy.briefing_list.is_empty());
    }

    /// #109: the control bar's options button's way into the quit menu (ControlBarCallback.cpp's `ToggleQuitMenu` and
    /// `HideQuitMenu`, [`ShellUi::bar_quit_menu_call`]), and the menu and its save/load popup on a phone, 390 by 844 CSS
    /// pixels. Predicted from QuitMenu.cpp and [`shell_placement`] before the run:
    ///
    /// - over the demo game (not ending: #107's `newMap`), `Toggle` shows `QuitMenu.wnd` and asks for the pause
    ///   ([`ShellAction::SetGamePaused`] true); `Hide` hides it and unpauses (false); `Hide` again does nothing (not
    ///   visible); `Toggle` shows it again (true);
    /// - at 390x844 (the placement of the #78 test: scale 0.4875, top ~~126.75, unchanged: no button is added to the zoom
    ///   column~~ **corrected 2026-11-11 (#131):** ~~72.75~~, the two turn buttons added to the zoom column; **corrected 2026-11-11, #136:**
    ///   45.75, the control bar's `B` added) each of the menu's five buttons, ~~100~~ 90 x 20 in the layout, is ~~48.75~~ 43.875 x 9.75 CSS pixels
    ///   on the screen, below
    ///   the status line (y 36) and above the zoom column, and a tap at its centre is that button's
    ///   (`getWindowUnderCursor`);
    /// - a tap on *Save/Load* opens the popup; its four buttons (*Save*, *Load*, *Delete*, *Back*) are each on the screen
    ///   and tapped the same way (~~all four~~ **first run:** *Load* and *Delete* are disabled with no save listed, so
    ///   not under the cursor; *Save* and *Back* are); a tap on *Save* shows the description box, whose *Confirm* and *Cancel* are too; a tap on
    ///   *Cancel* hides it.
    ///
    /// **Result (2026-11-10):** the toggles and the hide as predicted; two misses over two runs, recorded where asserted
    /// (the buttons are 90 wide, ~~43.875 x 9.75~~ **corrected 2026-11-11, #142:** 38.7 x 8.6 CSS pixels; *Load* and
    /// *Delete* disabled). Every enabled button of the menu,
    /// the popup and its description box is on the screen above the zoom column and taken by a tap at its centre.
    ///
    /// **Corrected 2026-11-12, #153:** the 390x844 placement is now the turned one (scale 0.5433, see
    /// [`shell_placement`]); the menu's buttons are 10.87 x 48.9 CSS pixels, beside the zoom column, each still taken by a
    /// tap at its centre.
    ///
    /// (#151: this doc and a second `#[test]` had been left above #148's test, which #148 put between them and this
    /// function: rustdoc gave this doc to #148's test, and rustc warned of the duplicated attribute. Moved back; the test
    /// itself always ran.)
    #[test]
    fn the_bars_options_button_opens_the_quit_menu_which_fits_a_phone() {
        use crate::ui::QuitMenuCall;
        let (mut ui, _, _) = shell_with(&TextTable::default());
        let mut now = 1000;
        settle(&mut ui, &mut now);
        ui.hide();
        settle(&mut ui, &mut now);
        let (w, _) = crate::demo::demo_world();
        let game = || quit_menu_game(&w, true);
        let (base, mut flow) = (World::new(), Flow::default());
        ui.bar_quit_menu_call(QuitMenuCall::Toggle, game);
        settle(&mut ui, &mut now);
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::SetGamePaused(true)]);
        assert!(ui.takes_input() && ui.gui.quit_menu.quit_menu_visible);
        ui.bar_quit_menu_call(QuitMenuCall::Hide, game);
        settle(&mut ui, &mut now);
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::SetGamePaused(false)]);
        assert!(!ui.takes_input() && !ui.gui.quit_menu.quit_menu_visible);
        ui.bar_quit_menu_call(QuitMenuCall::Hide, game);
        assert!(ui.take_actions(&base, &mut flow).is_empty());
        ui.bar_quit_menu_call(QuitMenuCall::Toggle, game);
        settle(&mut ui, &mut now);
        assert_eq!(ui.take_actions(&base, &mut flow), [ShellAction::SetGamePaused(true)]);
        let p = shell_placement((390.0, 844.0));
        // (~~126.75~~ corrected 2026-11-11, #131: the zoom column is 108 taller with the two turn buttons, so the menu
        // centres 54 higher in the room above it; the scale, set by the width, is the same) (~~72.75~~ corrected
        // 2026-11-11, #136: 54 taller again with `B`, 27 higher: 45.75) (~~(0, 45.75, 0.4875)~~ corrected 2026-11-11,
        // #142: 54 taller again with the phone's `X`, #135; the room above the column, 390 x 258, is now set by its
        // height: scale 258 / 600 = 0.43, the menu 344 wide at left 23, top 36) (~~(23, 36, 0.43)~~ corrected
        // 2026-11-12, #153: a portrait screen turns the layout a quarter, beside the column: 326 / 600 = 0.5433, the box
        // 326 x 434.67 at left 0, top 36 + (800 - 434.67) / 2 = 218.67)
        assert!(p.turned && p.left == 0.0 && (p.top - (36.0 + (800.0 - 800.0 * 326.0 / 600.0) / 2.0)).abs() < 1e-3 && (p.scale - 326.0 / 600.0).abs() < 1e-6, "{p:?}");
        let column_top = 844.0 - crate::ui::ZOOM_COLUMN_HEIGHT;
        let column_left = 390.0 - ZOOM_COLUMN_WIDTH;
        let last = |ui: &ShellUi, file: &str, name: &str| -> usize {
            let full = format!("{file}:{name}");
            ui.wm.tree.windows.len() - 1 - ui.wm.tree.windows.iter().rev().position(|x| x.name() == full.as_bytes()).expect(name)
        };
        // (each button on the screen and its centre's tap its own; the centre, for the tap)
        let tappable = |ui: &mut ShellUi, file: &str, name: &str| -> (f32, f32) {
            let w = last(ui, file, name);
            assert!(!ui.wm.tree.windows[w].is_hidden(), "{name} shown");
            let (x, y) = ui.wm.tree.screen_position(w);
            let reg = ui.wm.tree.windows[w].region;
            let r = p.to_screen([x as f32, y as f32, (x + reg.2 - reg.0) as f32, (y + reg.3 - reg.1) as f32]);
            // (#153: clear of the zoom column, beside it or above it)
            assert!(r[0] >= 0.0 && r[2] <= 390.0 && r[1] >= 36.0 && r[3] <= 844.0 && (r[2] <= column_left || r[3] <= column_top), "{name}: {r:?}");
            let centre = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
            let (lx, ly) = p.to_layout(centre);
            assert_eq!(ui.wm.get_window_under_cursor(&mut ui.gui, lx as i32, ly as i32, false), Some(w), "{name}");
            println!("{file}:{name}: {:.2} x {:.2} at ({:.2}, {:.2})", r[2] - r[0], r[3] - r[1], r[0], r[1]);
            centre
        };
        let tap = |ui: &mut ShellUi, at: (f32, f32)| {
            for msg in [gwm::LEFT_DOWN, gwm::LEFT_UP] {
                ui.pointer_at(msg, at, &p, true);
            }
        };
        for name in ["ButtonExit", "ButtonRestart", "ButtonReturn", "ButtonOptions", "ButtonSaveLoad"] {
            let at = tappable(&mut ui, "QuitMenu.wnd", name);
            let w = last(&ui, "QuitMenu.wnd", name);
            let reg = ui.wm.tree.windows[w].region;
            // (first run, a miss: the synthetic quit menu's buttons are 90 by 20, not 100: 43.875 x 9.75 CSS pixels)
            // (~~43.875 x 9.75~~ corrected 2026-11-11, #142: 38.7 x 8.6 at the scale 0.43) (~~38.7 x 8.6~~ corrected
            // 2026-11-12, #153: turned, 20 x 90 at 0.5433: 10.87 wide, 48.9 high)
            let (sw, sh) = p.screen_size(((reg.2 - reg.0) as f32, (reg.3 - reg.1) as f32));
            assert!((sw - 20.0 * 326.0 / 600.0).abs() < 1e-3 && (sh - 90.0 * 326.0 / 600.0).abs() < 1e-3, "{name}: {sw} x {sh}");
            if name == "ButtonSaveLoad" {
                tap(&mut ui, at);
            }
        }
        settle(&mut ui, &mut now);
        // (first run, a miss: *Load* and *Delete* are disabled with no save listed (the popup's init), and a disabled
        // button is not under the cursor (`getWindowUnderCursor` gives its frame): they are checked disabled, the
        // others tapped)
        for name in ["ButtonLoad", "ButtonDelete"] {
            assert!(!ui.wm.is_enabled(Some(last(&ui, "PopupSaveLoad.wnd", name))), "{name} disabled");
        }
        for name in ["ButtonSave", "ButtonBack"] {
            tappable(&mut ui, "PopupSaveLoad.wnd", name);
        }
        let save = tappable(&mut ui, "PopupSaveLoad.wnd", "ButtonSave");
        tap(&mut ui, save);
        settle(&mut ui, &mut now);
        tappable(&mut ui, "PopupSaveLoad.wnd", "ButtonSaveDescConfirm");
        let cancel = tappable(&mut ui, "PopupSaveLoad.wnd", "ButtonSaveDescCancel");
        tap(&mut ui, cancel);
        settle(&mut ui, &mut now);
        assert!(ui.wm.tree.windows[last(&ui, "PopupSaveLoad.wnd", "SaveDescParent")].is_hidden(), "the description box hidden");
        assert!(ui.wm.undefined.is_none(), "{:?}", ui.wm.undefined);
    }

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
    ///   0.4075 beside it); the menu 390 x 292.5 at left 0, top 36 + (474 - 292.5) / 2 = 126.75 (**corrected 2026-11-11
    ///   (#131):** ~~126.75~~ ~~36 + (366 - 292.5) / 2 = 72.75~~, the zoom column 108 taller with the two turn buttons;
    ///   **corrected 2026-11-11, #136:** 36 + (312 - 292.5) / 2 = 45.75, 54 taller again with `B`); **corrected
    ///   2026-11-11, #142:** with the phone's `X` the room above the column is 390 x 258, so its height sets the scale,
    ///   258 / 600 = 0.43 (still above beside's 0.4075): the menu 344 x 258 at left 23, top 36;
    /// - 1280x720: beside the column wins (scale min(1216 / 800, 676 / 600) = 1.1267); left (1216 - 901.3) / 2 = 157.3,
    ///   top 36;
    /// - at 390x844 every button of the main panel is on the screen, below the status line's first line (y 36) and
    ///   above the zoom column (~~y 518~~ **corrected 2026-11-11, #142:** y 302, 844 less the column's 542; it was 356
    ///   since #136), and a tap at its centre on the screen is that button's (`getWindowUnderCursor`);
    ///   a tap on *Single Player* through the screen shows the single player panel, as a click does. Each button is
    ///   ~~48.75 x 9.75~~ **corrected 2026-11-11, #142:** 43 x 8.6 CSS pixels (the synthetic layout's 100 x 20).
    ///
    /// ~~The 390x844 placement above (scale 0.43, upright, above the column)~~ **Corrected 2026-11-12, #153:** a
    /// portrait screen turns the layout a quarter clockwise ([`shell_placement`]): beside the column, scale 326 / 600 =
    /// 0.5433, the box 326 x 434.67 at left 0, top 218.67; each button 10.87 x 54.33 CSS pixels, beside the column
    /// (right edge at most 326), and a tap at its centre still that button's. The 1280x720 placement is unchanged.
    #[test]
    fn the_menu_fits_a_phone_and_every_button_is_tapped() {
        let p = shell_placement((390.0, 844.0));
        // (~~126.75~~ corrected 2026-11-11, #131: the taller zoom column, see the quit menu's test; ~~72.75~~
        // corrected 2026-11-11, #136: 45.75, `B` added; ~~(0, 45.75, 0, 0.4875)~~ corrected 2026-11-11, #142: `X` added,
        // the room's height sets the scale, 0.43, see the quit menu's test) (~~(23, 36, 0, 0.43)~~ corrected 2026-11-12,
        // #153: turned, see the quit menu's test)
        assert!(p.turned && p.left == 0.0 && p.y0 == 0.0 && (p.scale - 326.0 / 600.0).abs() < 1e-6, "{p:?}");
        let d = shell_placement((1280.0, 720.0));
        assert!((d.scale - 676.0 / 600.0).abs() < 1e-5 && (d.left - (1216.0 - 800.0 * 676.0 / 600.0) / 2.0).abs() < 1e-3 && d.top == 36.0, "{d:?}");
        let mut ui = shell();
        ui.frame(1000);
        assert!(ui.keyboard(0x39, true, Some(" ")));
        let mut now = 1000;
        settle(&mut ui, &mut now);
        let column_top = 844.0 - crate::ui::ZOOM_COLUMN_HEIGHT;
        let column_left = 390.0 - ZOOM_COLUMN_WIDTH;
        for name in ["ButtonSinglePlayer", "ButtonMultiplayer", "ButtonLoadReplay", "ButtonOptions", "ButtonCredits", "ButtonExit", "ButtonWorldBuilder"] {
            let w = win(&ui, name);
            let (x, y) = ui.wm.tree.screen_position(w);
            let reg = ui.wm.tree.windows[w].region;
            let r = p.to_screen([x as f32, y as f32, (x + reg.2 - reg.0) as f32, (y + reg.3 - reg.1) as f32]);
            // (#153: clear of the zoom column, beside it or above it)
            assert!(r[0] >= 0.0 && r[2] <= 390.0 && r[1] >= 36.0 && r[3] <= 844.0 && (r[2] <= column_left || r[3] <= column_top), "{name}: {r:?}");
            let centre = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
            let (lx, ly) = p.to_layout(centre);
            assert_eq!(ui.wm.get_window_under_cursor(&mut ui.gui, lx as i32, ly as i32, false), Some(w), "{name}");
            // (~~48.75 x 9.75~~ corrected 2026-11-11, #142: 43 x 8.6 at the scale 0.43, to f32's rounding) (~~43 x 8.6~~
            // corrected 2026-11-12, #153: turned, 20 x 100 at 0.5433: 10.87 x 54.33)
            assert!((r[2] - r[0] - 20.0 * 326.0 / 600.0).abs() < 1e-3 && (r[3] - r[1] - 100.0 * 326.0 / 600.0).abs() < 1e-3, "{name}: {r:?}");
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

    /// #153 (owner decision 2026-11-12): the shell's layout turned on a portrait screen. Predicted from
    /// [`shell_placement`] and [`LayoutPlacement`] before the run:
    ///
    /// - (a) 390x844 is turned: beside the zoom column (326 x 800 room) wins over above it (390 x 258): scale min(326 /
    ///   600, 800 / 800) = 0.5433 against min(390 / 600, 258 / 800) = 0.3225; the box 326 x 434.67 at left 0, top 36 +
    ///   (800 - 434.67) / 2 = 218.67. The layout's corners: (0, 0), its top left, at the box's top right (326, 218.67);
    ///   (800, 600) at its bottom left (0, 653.33); (0, 600) at the top left (0, 218.67);
    /// - (b) a tap maps back: the screen point (300, 250) is the layout's ((250 - 218.67) / 0.5433, 600 - 300 / 0.5433)
    ///   = (57.67, 47.85), and that point to the screen is (300, 250) again; a tap at the centre of the main menu's
    ///   *Single Player* (100 x 20 in the layout, drawn 10.87 wide and 54.33 high) hits it, as the shell's own
    ///   `pointer_at` gives it;
    /// - (c) 844x390 (and 1280x720, 800x600) is not turned and placed as before #153: 844x390 beside the column, scale
    ///   min(780 / 800, 346 / 600) = 0.5767, left (780 - 461.33) / 2 = 159.33, top 36; a point maps as `left + x x scale`.
    ///
    /// **Result (2026-11-12):** as predicted, but for one miss in the test itself: the landscape round trip of (10, 20)
    /// came back as 9.999993 in f32, so it is compared to within 0.001.
    #[test]
    fn a_portrait_screen_turns_the_shell_and_its_taps() {
        let p = shell_placement((390.0, 844.0));
        let s = 326.0f32 / 600.0;
        let top = 36.0 + (800.0 - 800.0 * s) / 2.0;
        assert!(p.turned && p.left == 0.0 && (p.top - top).abs() < 1e-3 && (p.scale - s).abs() < 1e-6, "{p:?}");
        let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);
        assert!(near(p.to_screen([0.0, 0.0, 0.0, 0.0]), [326.0, top, 326.0, top]), "{:?}", p.to_screen([0.0; 4]));
        assert!(near(p.to_screen([800.0, 600.0, 800.0, 600.0]), [0.0, top + 800.0 * s, 0.0, top + 800.0 * s]));
        assert!(near(p.to_screen([0.0, 600.0, 0.0, 600.0]), [0.0, top, 0.0, top]));
        assert_eq!(p.screen_size((800.0, 600.0)), (600.0 * s, 800.0 * s));
        // (b) a tap's point to the layout and back
        let (lx, ly) = p.to_layout((300.0, 250.0));
        assert!((lx - (250.0 - top) / s).abs() < 1e-3 && (ly - (600.0 - 300.0 / s)).abs() < 1e-3, "{lx}, {ly}");
        let back = p.to_screen([lx, ly, lx, ly]);
        assert!((back[0] - 300.0).abs() < 1e-3 && (back[1] - 250.0).abs() < 1e-3, "{back:?}");
        let mut ui = shell();
        ui.frame(1000);
        assert!(ui.keyboard(0x39, true, Some(" ")));
        let mut now = 1000;
        settle(&mut ui, &mut now);
        let w = win(&ui, "ButtonSinglePlayer");
        let (x, y) = ui.wm.tree.screen_position(w);
        let r = p.to_screen([x as f32, y as f32, x as f32 + 100.0, y as f32 + 20.0]);
        assert!((r[2] - r[0] - 20.0 * s).abs() < 1e-3 && (r[3] - r[1] - 100.0 * s).abs() < 1e-3, "{r:?}");
        let centre = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
        let (cx, cy) = p.to_layout(centre);
        assert_eq!(ui.wm.get_window_under_cursor(&mut ui.gui, cx as i32, cy as i32, false), Some(w));
        for msg in [gwm::LEFT_DOWN, gwm::LEFT_UP] {
            ui.pointer_at(msg, centre, &p, true);
        }
        settle(&mut ui, &mut now);
        assert!(!ui.wm.tree.windows[win(&ui, "MapBorder")].is_hidden(), "the turned tap pressed Single Player");
        // (c) landscape: not turned, as before
        let l = shell_placement((844.0, 390.0));
        let ls = (780.0f32 / 800.0).min(346.0 / 600.0);
        assert!(!l.turned && (l.scale - ls).abs() < 1e-6 && (l.left - (780.0 - 800.0 * ls) / 2.0).abs() < 1e-3 && l.top == 36.0, "{l:?}");
        assert_eq!(l.to_screen([10.0, 20.0, 30.0, 40.0]), [l.left + 10.0 * l.scale, l.top + 20.0 * l.scale, l.left + 30.0 * l.scale, l.top + 40.0 * l.scale]);
        // (first run, a miss in the test: f32's round trip gives 9.999993, not 10 exactly)
        let (bx, by) = l.to_layout((l.left + 10.0 * l.scale, l.top + 20.0 * l.scale));
        assert!((bx - 10.0).abs() < 1e-3 && (by - 20.0).abs() < 1e-3, "{bx}, {by}");
        for screen in [(1280.0, 720.0), (800.0, 600.0)] {
            assert!(!shell_placement(screen).turned, "{screen:?}");
        }
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
        let crate::ui::UiBuild::Ready(bar) = crate::ui::load_game_ui_with(&fs, &TextTable::default(), b"America", Some(images.clone()), &Default::default()) else { panic!("the bar loads") };
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
        let mut ui = ShellUi::load(&fs, &TextTable::default(), images, 0, &Default::default()).expect("the shell loads from the install");
        ui.frame(0);
        assert_eq!(ui.top_screen().as_deref(), Some(&b"Menus/MainMenu.wnd"[..]));
    }
}
