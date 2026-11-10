// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/FunctionLexicon.cpp (the entries of gameWinSystemTable,
//     gameWinInputTable, winLayoutInitTable, winLayoutUpdateTable and winLayoutShutdownTable for the callbacks
//     translated here: MainMenuSystem, MessageBoxSystem, QuitMessageBoxSystem, MainMenuInput, MainMenuInit,
//     MainMenuUpdate, MainMenuShutdown; #78: SaveLoadMenuSystem, SinglePlayerMenuSystem, DifficultySelectSystem,
//     SaveLoadMenuInput, SinglePlayerMenuInput, DifficultySelectInput, SaveLoadMenuInit, SaveLoadMenuFullScreenInit,
//     SinglePlayerMenuInit, DifficultySelectInit, SaveLoadMenuUpdate, SinglePlayerMenuUpdate, SaveLoadMenuShutdown,
//     SinglePlayerMenuShutdown; #89: SkirmishGameOptionsMenuSystem, SkirmishMapSelectMenuSystem,
//     SkirmishGameOptionsMenuInput, SkirmishMapSelectMenuInput, SkirmishGameOptionsMenuInit, SkirmishMapSelectMenuInit,
//     SkirmishGameOptionsMenuUpdate, SkirmishMapSelectMenuUpdate, SkirmishGameOptionsMenuShutdown,
//     SkirmishMapSelectMenuShutdown; the tooltip callbacks MapSelectorTooltip, mapListTooltipFunc; #89 part 2:
//     ScoreScreenSystem, ScoreScreenInput, ScoreScreenInit, ScoreScreenUpdate, ScoreScreenShutdown)
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
//! - A name this translation does not have yet (~~`SaveLoadMenuInit`,~~ `OptionsMenuInit`,
//!   ~~`SkirmishGameOptionsMenuSystem`,~~ ... every screen but the main menu and the message boxes) is recorded in
//!   [`ShellGui::untranslated`] and does nothing: the screen loads and shows its windows, but its buttons do nothing
//!   and its shutdown never completes (`Shell::popImmediate` still takes it off). Upstream's lexicon would have the
//!   function. **Corrected 2026-11-10 (#78):** the save/load screen (`crate::client::save_load_menu`: the full screen
//!   and the popup), the single player menu (`crate::client::single_player_menu`) and the difficulty popup
//!   (`crate::client::difficulty_select`) are translated and dispatched here; the runInit user data the lexicon passes
//!   them is `NULL`, as every caller in the release passes. The save directory is the owner's
//!   ([`ShellGui::save_files`]). **Corrected 2026-11-10 (#89):** the skirmish game-options screen
//!   (`crate::client::skirmish_game_options`) and its map picker (`crate::client::skirmish_map_select`) are translated
//!   and dispatched here, with their two tooltips (`MapSelectorTooltip`, `mapListTooltipFunc`); a tooltip callback
//!   that is not translated is recorded in [`ShellGui::untranslated`]. The World's data they read is the owner's copy
//!   ([`ShellGui::skirmish_data`]); the preference files are kept in memory ([`ShellGui::prefs`]). **Corrected
//!   2026-11-10 (#89 part 2):** the score screen (`crate::client::score_screen`) is translated and dispatched here; the
//!   game it scores is the owner's copy ([`ShellGui::score_screen`]'s `game`).
//! - (#78) `TheNameKeyGenerator` is [`ShellGui::keys`] ([`NameKeys`]), which the reader shares while the host lends it
//!   its files: ~~[`ShellHost::name_to_key`] asked the files' generator, and answered `u32::MAX` while they were lent
//!   to the reader~~ **Corrected 2026-11-10 (#78):** a callback the reader runs (a window's `GWM_CREATE`) gets the
//!   reader's keys, as upstream's one global generator gives (the files' own `name_to_key` is no longer asked).
//! - The engine's side: the requests ([`ShellRequest`])~~, the transitions (`Transition`)~~ and the shell hooks are
//!   kept in lists for the host's owner (the client) to act on; the clock and the global data are set by it.
//!   **Corrected 2026-11-10 (#77):** the transitions are no longer recorded here: they are the window manager's
//!   handler ([`crate::client::window_transitions`]), loaded from `WindowTransitions.ini` by the owner
//!   ([`synthetic::window_transitions_ini`] in the tests).
//!
//! [`SystemFunc::Host`]: crate::client::window_manager::SystemFunc::Host

use std::collections::BTreeMap;

use crate::client::difficulty_select::{self as ds, DifficultyHost, DifficultySelect};
use crate::client::gui_util::{GuiUtilHost, SkirmishData};
use crate::client::map_util::MapUtilHost;
use crate::client::skirmish_game_options::{self as sgo, SkirmishGameOptions, SkirmishHost};
use crate::client::skirmish_map_select::{self as smsm, SkirmishMapSelect};
use crate::client::score_screen::{self as ss, ScoreGridHost, ScoreScreen, ScoreScreenGlobals, ScoreScreenHost};
use crate::common::game_info::GameInfo;
use crate::common::map_cache::MapCache;
use crate::client::main_menu::{self, MainMenu, MenuHost};
use crate::client::message_box;
use crate::client::save_load_menu::{self as slm, SaveLoadHost, SaveLoadMenu};
use crate::client::single_player_menu::{self as spm, SinglePlayerHost, SinglePlayerMenu};
use crate::common::game_state::SaveGameInfo;
use crate::client::shell::{self, win_create_from_script_with, win_create_layout_with, KeyedFiles, LayoutFiles, NameKeys, Shell, ShellEnv, ShellHost, ShellRequest, WndFiles};
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
    /// (#78) PopupSaveLoad.cpp's statics and `TheGameState->m_availableGames`.
    pub save_load: SaveLoadMenu,
    /// (#78) The save directory as the owner gives it: each file's name and its information (`None`: the read throws).
    pub save_files: Vec<(Vec<u8>, Option<SaveGameInfo>)>,
    /// (#78) SinglePlayerMenu.cpp's statics.
    pub single_player: SinglePlayerMenu,
    /// (#78) DifficultySelect.cpp's statics.
    pub difficulty: DifficultySelect,
    /// (#78) `Options.ini`'s `CampaignDifficulty` (`None`: no entry; the options are not read in this client).
    pub campaign_difficulty_pref: Option<i32>,
    /// (#78) `TheNameKeyGenerator`: the table the reader gives the windows their keys from, and the callbacks ask
    /// (a clone of the host shares it, as the one global generator).
    pub keys: NameKeys,
    /// (#89) SkirmishGameOptionsMenu.cpp's statics, `TheSkirmishGameInfo` among them.
    pub skirmish: SkirmishGameOptions,
    /// (#89) The World's data the skirmish screens read, as the owner copied it (empty until it does: no maps).
    pub skirmish_data: SkirmishData,
    /// (#89) SkirmishMapSelectMenu.cpp's statics.
    pub map_select: SkirmishMapSelect,
    /// (#89) The preference files (`Skirmish.ini`, `Network.ini`) by leaf name, in memory: `UserPreferences::load`
    /// reads them, `write` replaces them (the files themselves are not read or written).
    pub prefs: BTreeMap<Vec<u8>, BTreeMap<Vec<u8>, Vec<u8>>>,
    /// (#89) The map names a map list's item data stands for (item data `n` is entry `n - 1`).
    pub map_list_names: Vec<Vec<u8>>,
    /// (#89) `SkirmishBattleHonors::getWins()` (the stats file is not read: 0 unless the owner sets it).
    pub skirmish_wins: i32,
    /// (#89 part 2) ScoreScreen.cpp's statics and globals, with the game it scores (the owner's copy).
    pub score_screen: ScoreScreen,
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
            save_load: SaveLoadMenu::default(),
            save_files: Vec::new(),
            single_player: SinglePlayerMenu::default(),
            difficulty: DifficultySelect::default(),
            campaign_difficulty_pref: None,
            keys: NameKeys::default(),
            skirmish: SkirmishGameOptions::default(),
            skirmish_data: SkirmishData::default(),
            map_select: SkirmishMapSelect::default(),
            prefs: BTreeMap::new(),
            map_list_names: Vec::new(),
            skirmish_wins: 0,
            score_screen: ScoreScreen::default(),
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
            b"SaveLoadMenuSystem" => slm::save_load_menu_system(wm, self, win, msg, d1, d2),
            b"SinglePlayerMenuSystem" => spm::single_player_menu_system(wm, self, win, msg, d1, d2),
            b"DifficultySelectSystem" => ds::difficulty_select_system(wm, self, win, msg, d1, d2),
            b"SkirmishGameOptionsMenuSystem" => sgo::skirmish_game_options_menu_system(wm, self, win, msg, d1, d2),
            b"SkirmishMapSelectMenuSystem" => smsm::skirmish_map_select_menu_system(wm, self, win, msg, d1, d2),
            // (#89 part 2)
            b"ScoreScreenSystem" => ss::score_screen_system(wm, self, win, msg, d1, d2),
            _ => {
                self.untranslated.push(func.to_vec());
                MsgHandled::Ignored
            }
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        match func {
            b"MainMenuInput" => main_menu::main_menu_input(wm, self, win, msg, d1, d2),
            b"SaveLoadMenuInput" => slm::save_load_menu_input(wm, self, win, msg, d1, d2),
            b"SinglePlayerMenuInput" => spm::single_player_menu_input(wm, self, win, msg, d1, d2),
            b"DifficultySelectInput" => ds::difficulty_select_input(wm, self, win, msg, d1, d2),
            b"SkirmishGameOptionsMenuInput" => sgo::skirmish_game_options_menu_input(wm, self, win, msg, d1, d2),
            b"SkirmishMapSelectMenuInput" => smsm::skirmish_map_select_menu_input(wm, self, win, msg, d1, d2),
            b"ScoreScreenInput" => ss::score_screen_input(wm, self, win, msg, d1, d2),
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
    fn tooltip(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, mouse: u32) {
        match func {
            // (#89)
            b"MapSelectorTooltip" => sgo::map_selector_tooltip(wm, self, win, mouse),
            b"mapListTooltipFunc" => smsm::map_list_tooltip_func(wm, self, win, mouse),
            _ => {
                if !self.untranslated.iter().any(|f| f == func) {
                    self.untranslated.push(func.to_vec());
                }
            }
        }
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
        // (#78: the files lent with the shared key table, so that a `GWM_CREATE` asking for a key gets the reader's)
        let mut k = KeyedFiles { files: &mut f.files, keys: self.keys.clone() };
        let r = win_create_layout_with(&mut f.script, &mut k, wm, self, filename);
        self.files = Some(f);
        r
    }
    fn create_from_script(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<WinId> {
        let mut f = self.files.take()?;
        let mut k = KeyedFiles { files: &mut f.files, keys: self.keys.clone() };
        let r = win_create_from_script_with(&mut f.script, &mut k, wm, self, filename);
        self.files = Some(f);
        r
    }
    fn layout_callback(&mut self, wm: &mut GameWindowManager, func: &[u8], layout: usize, user: Option<bool>) {
        match func {
            b"MainMenuInit" => main_menu::main_menu_init(wm, self, layout),
            b"MainMenuUpdate" => main_menu::main_menu_update(wm, self, layout),
            b"MainMenuShutdown" => main_menu::main_menu_shutdown(wm, self, layout, user),
            // (#78; the runInit user data, `SaveLoadLayoutType *`, is `NULL` from every caller in the release)
            b"SaveLoadMenuInit" => slm::save_load_menu_init(wm, self, layout, None),
            b"SaveLoadMenuFullScreenInit" => slm::save_load_menu_full_screen_init(wm, self, layout, None),
            b"SaveLoadMenuUpdate" => slm::save_load_menu_update(wm, self, layout),
            b"SaveLoadMenuShutdown" => slm::save_load_menu_shutdown(wm, self, layout, user),
            b"SinglePlayerMenuInit" => spm::single_player_menu_init(wm, self, layout),
            b"SinglePlayerMenuUpdate" => spm::single_player_menu_update(wm, self, layout),
            b"SinglePlayerMenuShutdown" => spm::single_player_menu_shutdown(wm, self, layout, user),
            b"DifficultySelectInit" => ds::difficulty_select_init(wm, self, layout),
            // (#89)
            b"SkirmishGameOptionsMenuInit" => sgo::skirmish_game_options_menu_init(wm, self, layout),
            b"SkirmishGameOptionsMenuUpdate" => sgo::skirmish_game_options_menu_update(wm, self, layout),
            b"SkirmishGameOptionsMenuShutdown" => sgo::skirmish_game_options_menu_shutdown(wm, self, layout, user),
            b"SkirmishMapSelectMenuInit" => smsm::skirmish_map_select_menu_init(wm, self, layout),
            b"SkirmishMapSelectMenuUpdate" => smsm::skirmish_map_select_menu_update(wm, self, layout),
            b"SkirmishMapSelectMenuShutdown" => smsm::skirmish_map_select_menu_shutdown(wm, self, layout),
            // (#89 part 2)
            b"ScoreScreenInit" => ss::score_screen_init(wm, self, layout),
            b"ScoreScreenUpdate" => ss::score_screen_update(wm, self, layout),
            b"ScoreScreenShutdown" => ss::score_screen_shutdown(wm, self, layout),
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
        // ~~the files' generator (`u32::MAX` while they are lent to the reader)~~ **Corrected 2026-11-10 (#78):** the
        // shared table, also while the files are lent
        self.keys.name_to_key(name)
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
    fn score_screen_globals(&mut self) -> Option<&mut ScoreScreenGlobals> {
        Some(&mut self.score_screen.globals)
    }
}

impl<W: WndFiles> ScoreGridHost for ShellGui<W> {
    fn score_screen(&mut self) -> &mut ScoreScreen {
        &mut self.score_screen
    }
    fn grid_name_to_key(&mut self, name: &[u8]) -> u32 {
        self.keys.name_to_key(name)
    }
    fn grid_fetch_text(&self, label: &[u8]) -> Vec<u16> {
        self.text(label)
    }
    fn grid_find_image(&self, name: &[u8]) -> Option<Vec<u8>> {
        MapUtilHost::find_mapped_image(self, name)
    }
    fn score_untranslated(&mut self, what: &[u8]) {
        self.untranslated.push(what.to_vec());
    }
}

impl<W: WndFiles> ScoreScreenHost for ShellGui<W> {}

impl<W: WndFiles> SaveLoadHost for ShellGui<W> {
    fn save_load_menu(&mut self) -> &mut SaveLoadMenu {
        &mut self.save_load
    }
    fn save_files(&mut self) -> Vec<(Vec<u8>, Option<SaveGameInfo>)> {
        self.save_files.clone()
    }
    fn fetch_text_exists(&self, label: &[u8]) -> (Vec<u16>, bool) {
        self.files.as_ref().map_or_else(|| (Vec::new(), false), |f| f.files.fetch_text_exists(label))
    }
    fn delete_save_file(&mut self, filename: &[u8]) {
        self.save_files.retain(|(n, _)| !n.eq_ignore_ascii_case(filename));
        self.requests.push(ShellRequest::DeleteSave { filename: filename.to_vec() });
    }
    fn destroy_quit_menu(&mut self) {
        self.untranslated.push(b"destroyQuitMenu".to_vec());
    }
}

impl<W: WndFiles> SinglePlayerHost for ShellGui<W> {
    fn single_player_menu(&mut self) -> &mut SinglePlayerMenu {
        &mut self.single_player
    }
}

impl<W: WndFiles> DifficultyHost for ShellGui<W> {
    fn difficulty_select(&mut self) -> &mut DifficultySelect {
        &mut self.difficulty
    }
    fn campaign_difficulty_pref(&self) -> Option<i32> {
        self.campaign_difficulty_pref
    }
}

impl<W: WndFiles> MapUtilHost for ShellGui<W> {
    fn map_cache(&self) -> Option<&MapCache> {
        Some(&self.skirmish_data.map_cache)
    }
    fn update_map_cache(&mut self) {
        // (#89) the cache as the owner copied it, which the World built at its start (`GameEngine::init`); the user's
        // map directory is not scanned again
    }
    fn user_data_path(&self) -> Vec<u8> {
        self.skirmish_data.user_data_path.clone()
    }
    fn find_mapped_image(&self, name: &[u8]) -> Option<Vec<u8>> {
        self.files.as_ref().and_then(|f| f.files.find_image(name))
    }
    fn map_list_item(&mut self, name: &[u8]) -> u32 {
        let i = match self.map_list_names.iter().position(|n| n == name) {
            Some(i) => i,
            None => {
                self.map_list_names.push(name.to_vec());
                self.map_list_names.len() - 1
            }
        };
        i as u32 + 1
    }
    fn map_list_name(&self, data: u32) -> Option<Vec<u8>> {
        (data as usize).checked_sub(1).and_then(|i| self.map_list_names.get(i)).cloned()
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        self.skirmish_data.real_map_path_to_portable(path)
    }
    fn map_preview_from_file(&mut self, _tga: &[u8], _name: &[u8], _temp_name: &[u8]) -> Option<Vec<u8>> {
        // (#89) `copyFromBigToDir` and the new `Image`: not translated
        self.untranslated.push(b"getMapPreviewImage: copyFromBigToDir".to_vec());
        None
    }
}

impl<W: WndFiles> GuiUtilHost for ShellGui<W> {
    fn win_initialized(&mut self) -> &mut bool {
        &mut self.skirmish.win_initialized
    }
    fn util_game(&self) -> Option<&GameInfo> {
        self.skirmish.game.as_ref()
    }
    fn util_data(&self) -> &SkirmishData {
        &self.skirmish_data
    }
}

impl<W: WndFiles> SkirmishHost for ShellGui<W> {
    fn skirmish(&mut self) -> &mut SkirmishGameOptions {
        &mut self.skirmish
    }
    fn skirmish_split(&mut self) -> (&mut SkirmishGameOptions, &mut SkirmishData) {
        (&mut self.skirmish, &mut self.skirmish_data)
    }
    fn map_select(&mut self) -> &mut SkirmishMapSelect {
        &mut self.map_select
    }
    fn prefs_load(&mut self, leaf: &[u8]) -> BTreeMap<Vec<u8>, Vec<u8>> {
        self.prefs.get(leaf).cloned().unwrap_or_default()
    }
    fn prefs_write(&mut self, leaf: &[u8], prefs: &BTreeMap<Vec<u8>, Vec<u8>>) -> bool {
        self.prefs.insert(leaf.to_vec(), prefs.clone());
        true
    }
    fn skirmish_wins(&self) -> i32 {
        self.skirmish_wins
    }
    fn clear_battle_honors(&mut self) {
        self.skirmish_wins = 0;
    }
    fn untranslated(&mut self, what: &[u8]) {
        self.untranslated.push(what.to_vec());
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

    /// (#78) One window `<file>:<name>` of `kind` (`PUSHBUTTON`, `USER`, ...) over `(x0, y0)-(x1, y1)`, hidden or
    /// shown, with `extra` lines (its data, callbacks) and its `children` (already made).
    fn win(file: &str, kind: &str, name: &str, r: (i32, i32, i32, i32), hidden: bool, extra: &str, children: &str) -> String {
        let st = if hidden { "ENABLED+HIDDEN" } else { "ENABLED" };
        let style = match kind {
            "PUSHBUTTON" => "PUSHBUTTON+MOUSETRACK",
            "RADIOBUTTON" => "RADIOBUTTON+MOUSETRACK",
            "SCROLLLISTBOX" => "SCROLLLISTBOX+MOUSETRACK",
            // (#89: a check box takes a click only once the mouse has entered it, which needs MOUSETRACK)
            "CHECKBOX" => "CHECKBOX+MOUSETRACK",
            k => k,
        };
        let text = if matches!(kind, "PUSHBUTTON" | "RADIOBUTTON") { format!("    TEXT = \"GUI:{name}\";\n") } else { String::new() };
        let mut out = format!(
            "  WINDOW\n    WINDOWTYPE = {kind};\n    SCREENRECT = UPPERLEFT: {} {}, BOTTOMRIGHT: {} {}, CREATIONRESOLUTION: 800 600;\n    NAME = \"{file}:{name}\";\n    STATUS = {st};\n    STYLE = {style};\n{text}{extra}",
            r.0, r.1, r.2, r.3
        );
        if !children.is_empty() {
            out += &format!("    CHILD\n{children}    ENDALLCHILDREN\n");
        }
        out + "  END\n"
    }

    /// (#78) A row of push buttons `<file>:<name>`, 90x20, from (x, y) down, 25 apart.
    fn buttons(file: &str, x: i32, y: i32, names: &[&str]) -> String {
        names.iter().enumerate().map(|(i, n)| win(file, "PUSHBUTTON", n, (x, y + 25 * i as i32, x + 90, y + 20 + 25 * i as i32), false, "", "")).collect()
    }

    /// (#78) A frame `<file>:<name>` that passes its buttons' presses up (`PassSelectedButtonsToParentSystem`).
    fn frame(file: &str, name: &str, r: (i32, i32, i32, i32), hidden: bool, children: &str) -> String {
        win(file, "USER", name, r, hidden, "    SYSTEMCALLBACK = \"PassSelectedButtonsToParentSystem\";\n", children)
    }

    /// (#78) A layout file: the layout block's three callbacks (`[None]` for none) and its windows.
    fn layout_file(init: &str, update: &str, shutdown: &str, windows: &str) -> Vec<u8> {
        format!("FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = {init};\n  LAYOUTUPDATE = {update};\n  LAYOUTSHUTDOWN = {shutdown};\nENDLAYOUTBLOCK\n{}", windows.replace("\n  ", "\n").trim_start_matches(' ')).into_bytes()
    }

    /// (#78) A synthetic save/load screen (no EA file): `Menus/SaveLoad.wnd` (full screen, `SaveLoadMenuFullScreenInit`)
    /// or, with `popup`, `Menus/PopupSaveLoad.wnd` (`SaveLoadMenuInit`), with the windows PopupSaveLoad.cpp finds by name:
    /// the parent `SaveLoadMenu` (the system and input callbacks), the games list box (three columns, 60/20/20 %) at
    /// (20, 100)-(560, 400), the button frame (Save, Load, Delete, Back at x 610 from y 110), and the four confirmations
    /// at (200, 420)-(600, 520), each with its two buttons at x 210 from y 430 (the delete one hidden in the file, the
    /// others shown; the description one holds `EntryDesc` at (310, 430)-(590, 450)).
    pub fn save_load_wnd(popup: bool) -> Vec<u8> {
        let f = if popup { "PopupSaveLoad.wnd" } else { "SaveLoad.wnd" };
        let conf = (200, 420, 600, 520);
        let mut c = String::new();
        c += &win(
            f,
            "SCROLLLISTBOX",
            "ListboxGames",
            (20, 100, 560, 400),
            false,
            "    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 3, COLUMNSWIDTH%: 60, COLUMNSWIDTH%: 20, COLUMNSWIDTH%: 20, FORCESELECT: 0;\n",
            "",
        );
        c += &frame(f, "MenuButtonFrame", (600, 100, 790, 300), false, &buttons(f, 610, 110, &["ButtonSave", "ButtonLoad", "ButtonDelete", "ButtonBack"]));
        c += &frame(f, "OverwriteConfirmParent", conf, false, &buttons(f, 210, 430, &["ButtonOverwriteConfirm", "ButtonOverwriteCancel"]));
        c += &frame(f, "LoadConfirmParent", conf, false, &buttons(f, 210, 430, &["ButtonLoadConfirm", "ButtonLoadCancel"]));
        let entry = win(f, "ENTRYFIELD", "EntryDesc", (310, 430, 590, 450), false, "    TEXTENTRYDATA = MAXLEN: 64, SECRETTEXT: 0, NUMERICALONLY: 0, ALPHANUMERICALONLY: 0, ASCIIONLY: 0;\n", "");
        c += &frame(f, "SaveDescParent", conf, false, &(buttons(f, 210, 430, &["ButtonSaveDescConfirm", "ButtonSaveDescCancel"]) + &entry));
        c += &frame(f, "DeleteConfirmParent", conf, true, &buttons(f, 210, 430, &["ButtonDeleteConfirm", "ButtonDeleteCancel"]));
        let parent = win(f, "USER", "SaveLoadMenu", (0, 0, 800, 600), false, "    SYSTEMCALLBACK = \"SaveLoadMenuSystem\";\n    INPUTCALLBACK = \"SaveLoadMenuInput\";\n", &c);
        let init = if popup { "SaveLoadMenuInit" } else { "SaveLoadMenuFullScreenInit" };
        layout_file(init, "SaveLoadMenuUpdate", "SaveLoadMenuShutdown", &parent)
    }

    /// (#78) A synthetic `Menus/SinglePlayerMenu.wnd` (no EA file): `SinglePlayerMenuParent` with the system and input
    /// callbacks, and its New, Load and Back buttons at x 350 from y 250.
    pub fn single_player_menu_wnd() -> Vec<u8> {
        let f = "SinglePlayerMenu.wnd";
        let parent = win(
            f,
            "USER",
            "SinglePlayerMenuParent",
            (0, 0, 800, 600),
            false,
            "    SYSTEMCALLBACK = \"SinglePlayerMenuSystem\";\n    INPUTCALLBACK = \"SinglePlayerMenuInput\";\n",
            &buttons(f, 350, 250, &["ButtonNew", "ButtonLoad", "ButtonBack"]),
        );
        layout_file("SinglePlayerMenuInit", "SinglePlayerMenuUpdate", "SinglePlayerMenuShutdown", &parent)
    }

    /// (#78) A synthetic `Menus/DifficultySelect.wnd` (no EA file): `DifficultySelectParent` at (250, 200)-(550, 400) with
    /// the system and input callbacks, the three radio buttons (group 1) at x 260 from y 210, OK and Cancel at x 400 from
    /// y 210.
    pub fn difficulty_select_wnd() -> Vec<u8> {
        let f = "DifficultySelect.wnd";
        let mut c = String::new();
        for (i, n) in ["RadioButtonEasy", "RadioButtonMedium", "RadioButtonHard"].iter().enumerate() {
            let y = 210 + 25 * i as i32;
            c += &win(f, "RADIOBUTTON", n, (260, y, 350, y + 20), false, "    RADIOBUTTONDATA = GROUP: 1;\n", "");
        }
        c += &buttons(f, 400, 210, &["ButtonOk", "ButtonCancel"]);
        let parent = win(
            f,
            "USER",
            "DifficultySelectParent",
            (250, 200, 550, 400),
            false,
            "    SYSTEMCALLBACK = \"DifficultySelectSystem\";\n    INPUTCALLBACK = \"DifficultySelectInput\";\n",
            &c,
        );
        layout_file("DifficultySelectInit", "[None]", "[None]", &parent)
    }

    /// (#89) A combo box `<file>:<name>` over `r`, not editable, 16 characters, 5 rows shown.
    fn combo(file: &str, name: &str, r: (i32, i32, i32, i32)) -> String {
        win(file, "COMBOBOX", name, r, false, "    COMBOBOXDATA = ISEDITABLE: 0, MAXCHARS: 16, MAXDISPLAY: 5, ASCIIONLY: 0, LETTERSANDNUMBERS: 0;\n", "")
    }

    /// (#89) A static text `<file>:<name>` over `r`.
    fn static_text(file: &str, name: &str, r: (i32, i32, i32, i32)) -> String {
        win(file, "STATICTEXT", name, r, false, "    STATICTEXTDATA = CENTERED: 0;\n", "")
    }

    /// (#89) The eight start-spot buttons `<file>:ButtonMapStartPosition<i>`, 16x16, in a row from (x, y), 20 apart.
    fn start_spots(file: &str, x: i32, y: i32) -> String {
        (0..8).map(|i| win(file, "PUSHBUTTON", &format!("ButtonMapStartPosition{i}"), (x + 20 * i, y, x + 20 * i + 16, y + 16), false, "", "")).collect()
    }

    /// (#89) A synthetic `Menus/SkirmishGameOptionsMenu.wnd` (no EA file), with the windows SkirmishGameOptionsMenu.cpp
    /// and SkirmishMapSelectMenu.cpp find by name, under `SkirmishGameOptionsMenuParent` (the system and input
    /// callbacks): the player's name entry at (20, 40); the seven computer slots' player combo boxes (x 20, from y 95,
    /// 25 apart; slot `i` at y 70 + 25 i); each slot's colour (x 160), faction (x 270) and team (x 380) combo boxes from
    /// y 40; their four labels at y 20; `MapWindow` at (500, 40)-(700, 240) holding the eight start-spot buttons (a row
    /// at its top left, passing their presses up); the map's name `TextEntryMapDisplay` and `StaticTextMapPreview`; *Select Map* at (500, 280),
    /// *Back* (20, 560), *Reset* (300, 560), *Start* (600, 560); the speed slider (15 to 61) at (20, 300) with its
    /// text; the superweapon box at (20, 330); the starting money's combo box at (20, 360); `SubParent`.
    pub fn skirmish_game_options_wnd() -> Vec<u8> {
        let f = "SkirmishGameOptionsMenu.wnd";
        let mut c = String::new();
        c += &win(f, "ENTRYFIELD", "TextEntryPlayerName", (20, 40, 150, 60), false, "    TEXTENTRYDATA = MAXLEN: 64, SECRETTEXT: 0, NUMERICALONLY: 0, ALPHANUMERICALONLY: 0, ASCIIONLY: 0;\n", "");
        for i in 0..8 {
            let y = 40 + 25 * i;
            if i != 0 {
                c += &combo(f, &format!("ComboBoxPlayer{i}"), (20, 30 + y, 150, 50 + y));
            }
            c += &combo(f, &format!("ComboBoxColor{i}"), (160, y, 260, y + 20));
            c += &combo(f, &format!("ComboBoxPlayerTemplate{i}"), (270, y, 370, y + 20));
            c += &combo(f, &format!("ComboBoxTeam{i}"), (380, y, 460, y + 20));
        }
        for (n, x) in [("StaticTextPlayers", 20), ("StaticTextColor", 160), ("StaticTextFaction", 270), ("StaticTextTeam", 380)] {
            c += &static_text(f, n, (x, 20, x + 90, 38));
        }
        c += &frame(f, "MapWindow", (500, 40, 700, 240), false, &start_spots(f, 500, 40));
        c += &static_text(f, "TextEntryMapDisplay", (500, 250, 700, 270));
        c += &static_text(f, "StaticTextMapPreview", (500, 20, 700, 38));
        c += &buttons(f, 500, 280, &["ButtonSelectMap"]);
        c += &buttons(f, 20, 560, &["ButtonBack"]);
        c += &buttons(f, 300, 560, &["ButtonReset"]);
        c += &buttons(f, 600, 560, &["ButtonStart"]);
        c += &win(f, "HORZSLIDER", "SliderGameSpeed", (20, 300, 220, 320), false, "    SLIDERDATA = MINVALUE: 15, MAXVALUE: 61;\n", "");
        c += &static_text(f, "StaticTextGameSpeed", (230, 300, 270, 320));
        c += &win(f, "CHECKBOX", "CheckboxLimitSuperweapons", (20, 330, 200, 350), false, "", "");
        c += &combo(f, "ComboBoxStartingCash", (20, 360, 200, 380));
        c += &win(f, "USER", "SubParent", (0, 400, 10, 410), false, "", "");
        let parent = win(
            f,
            "USER",
            "SkirmishGameOptionsMenuParent",
            (0, 0, 800, 600),
            false,
            "    SYSTEMCALLBACK = \"SkirmishGameOptionsMenuSystem\";\n    INPUTCALLBACK = \"SkirmishGameOptionsMenuInput\";\n",
            &c,
        );
        layout_file("SkirmishGameOptionsMenuInit", "SkirmishGameOptionsMenuUpdate", "SkirmishGameOptionsMenuShutdown", &parent)
    }

    /// (#89) A synthetic `Menus/SkirmishMapSelectMenu.wnd` (no EA file): the parent under the name the code looks for
    /// (`SkrimishMapSelectMenuParent`, misspelt) at (100, 50)-(700, 550) with the system and input callbacks; the map
    /// list (two columns, 10/90 %) at (110, 60)-(400, 400); the two radio buttons (group 1) at x 410 from y 60;
    /// `WinMapPreview` at (410, 120)-(610, 320) holding the eight start-spot buttons; *OK* at (410, 500) and *Back*
    /// at (520, 500).
    pub fn skirmish_map_select_wnd() -> Vec<u8> {
        let f = "SkirmishMapSelectMenu.wnd";
        let mut c = String::new();
        c += &win(
            f,
            "SCROLLLISTBOX",
            "ListboxMap",
            (110, 60, 400, 400),
            false,
            "    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 2, COLUMNSWIDTH%: 10, COLUMNSWIDTH%: 90, FORCESELECT: 0;\n",
            "",
        );
        for (n, y) in [("RadioButtonSystemMaps", 60), ("RadioButtonUserMaps", 85)] {
            c += &win(f, "RADIOBUTTON", n, (410, y, 500, y + 20), false, "    RADIOBUTTONDATA = GROUP: 1;\n", "");
        }
        c += &win(f, "USER", "WinMapPreview", (410, 120, 610, 320), false, "", &start_spots(f, 410, 120));
        c += &buttons(f, 410, 500, &["ButtonOK"]);
        c += &buttons(f, 520, 500, &["ButtonBack"]);
        let parent = win(
            f,
            "USER",
            "SkrimishMapSelectMenuParent",
            (100, 50, 700, 550),
            false,
            "    SYSTEMCALLBACK = \"SkirmishMapSelectMenuSystem\";\n    INPUTCALLBACK = \"SkirmishMapSelectMenuInput\";\n",
            &c,
        );
        layout_file("SkirmishMapSelectMenuInit", "SkirmishMapSelectMenuUpdate", "SkirmishMapSelectMenuShutdown", &parent)
    }

    /// (#89 part 2) A synthetic `Menus/ScoreScreen.wnd` (no EA file), with the windows ScoreScreen.cpp finds by name,
    /// under `ParentScoreScreen` (the system and input callbacks): *OK* (600, 560), *Continue* (700, 560), *Buddies*
    /// (20, 560), *Save Replay* (120, 560), *Emote* (220, 560); the chat entry at (20, 520)-(300, 540) with its list box
    /// and border; the academy's list box (one column) at (320, 420)-(780, 540) and title; *game saved* at (320, 400);
    /// `MainBackdrop`, `BigPortrait`, `ChallengeWinLossText`, `GeneralRemarks`; `GadgetParent` (20, 40)-(780, 400)
    /// holding the grid: per row `i` (0 .. 7, at y 60 + 40 i) `StaticTextPlayer<i>` (x 20), `StaticTextObserver<i>`
    /// (x 150), the seven numbers from x 220 (70 apart), `GameWindowWinner<i>` (x 720); `ButtonAdd0`, hidden.
    pub fn score_screen_wnd() -> Vec<u8> {
        let f = "ScoreScreen.wnd";
        let mut grid = String::new();
        for i in 0..8 {
            let y = 60 + 40 * i;
            grid += &static_text(f, &format!("StaticTextPlayer{i}"), (20, y, 140, y + 20));
            grid += &static_text(f, &format!("StaticTextObserver{i}"), (150, y, 210, y + 20));
            for (j, n) in ["UnitsBuilt", "UnitsLost", "UnitsDestroyed", "BuildingsBuilt", "BuildingsLost", "BuildingsDestroyed", "Resources"].iter().enumerate() {
                let x = 220 + 70 * j as i32;
                grid += &static_text(f, &format!("StaticText{n}{i}"), (x, y, x + 60, y + 20));
            }
            grid += &win(f, "USER", &format!("GameWindowWinner{i}"), (720, y, 760, y + 20), false, "", "");
        }
        let mut c = String::new();
        c += &frame(f, "GadgetParent", (20, 40, 780, 400), false, &grid);
        c += &buttons(f, 600, 560, &["ButtonOk"]);
        c += &buttons(f, 700, 560, &["ButtonContinue"]);
        c += &buttons(f, 20, 560, &["ButtonBuddy"]);
        c += &buttons(f, 120, 560, &["ButtonSaveReplay"]);
        c += &buttons(f, 220, 560, &["ButtonEmote"]);
        c += &win(f, "PUSHBUTTON", "ButtonAdd0", (700, 10, 760, 30), true, "", "");
        c += &win(f, "ENTRYFIELD", "TextEntryChat", (20, 520, 300, 540), false, "    TEXTENTRYDATA = MAXLEN: 64, SECRETTEXT: 0, NUMERICALONLY: 0, ALPHANUMERICALONLY: 0, ASCIIONLY: 0;\n", "");
        let list = "    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 0;\n";
        c += &win(f, "SCROLLLISTBOX", "ListboxChatWindowScoreScreen", (20, 420, 300, 510), false, list, "");
        c += &win(f, "USER", "ChatBoxBorder", (18, 418, 302, 542), false, "", "");
        c += &win(f, "SCROLLLISTBOX", "ListboxWarschoolAdvice", (320, 420, 780, 540), false, list, "");
        c += &static_text(f, "StaticTextWarSchool", (320, 405, 500, 418));
        c += &static_text(f, "StaticTextGameSaveComplete", (520, 405, 700, 418));
        c += &win(f, "USER", "MainBackdrop", (0, 0, 10, 10), false, "", "");
        c += &win(f, "USER", "BigPortrait", (20, 40, 220, 300), true, "", "");
        c += &static_text(f, "ChallengeWinLossText", (240, 40, 700, 60));
        c += &static_text(f, "GeneralRemarks", (240, 80, 700, 300));
        let parent = win(f, "USER", "ParentScoreScreen", (0, 0, 800, 600), false, "    SYSTEMCALLBACK = \"ScoreScreenSystem\";\n    INPUTCALLBACK = \"ScoreScreenInput\";\n", &c);
        layout_file("ScoreScreenInit", "ScoreScreenUpdate", "ScoreScreenShutdown", &parent)
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
        // (#78) the save/load screen's fade (`SaveLoadMenuFade`), on its parent, not fire-once (its shutdown reverses it)
        s += "WindowTransition SaveLoadMenuFade\n  Window\n    WinName = SaveLoad.wnd:SaveLoadMenu\n    Style = FLASH\n    FrameDelay = 0\n  End\nEnd\n";
        // (#89 part 2) the score screen's show, on its grid
        s += "WindowTransition ScoreScreenShow\n  Window\n    WinName = ScoreScreen.wnd:GadgetParent\n    Style = FLASH\n    FrameDelay = 0\n  End\nEnd\n";
        // (#89) the skirmish screen's fade, on its parent, not fire-once (its shutdown reverses it)
        s += "WindowTransition SkirmishGameOptionsMenuFade\n  Window\n    WinName = SkirmishGameOptionsMenu.wnd:SkirmishGameOptionsMenuParent\n    Style = FLASH\n    FrameDelay = 0\n  End\nEnd\n";
        for (g, w) in [("US", "US"), ("GLA", "GLA"), ("China", "China"), ("Training", "Training"), ("Skirmish", "Skirmish")] {
            s += &format!("WindowTransition MainMenuFaction{g}\n  Window\n    WinName = MainMenu.wnd:WinFaction{w}\n    Style = WINSCALEUP\n  End\nEnd\n");
        }
        s.into_bytes()
    }

    /// The shell's synthetic files by their paths (`Window\Menus/<name>.wnd`): the main menu, the two message boxes,
    /// and plain screens ([`screen`]) for the background and the screens the main menu pushes.
    pub fn shell_wnd_files() -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut out = vec![
            ("MainMenu", main_menu_wnd()),
            ("MessageBox", message_box_wnd(false)),
            ("QuitMessageBox", message_box_wnd(true)),
            // (#78)
            ("SaveLoad", save_load_wnd(false)),
            ("PopupSaveLoad", save_load_wnd(true)),
            ("SinglePlayerMenu", single_player_menu_wnd()),
            ("DifficultySelect", difficulty_select_wnd()),
            // (#89)
            ("SkirmishGameOptionsMenu", skirmish_game_options_wnd()),
            ("SkirmishMapSelectMenu", skirmish_map_select_wnd()),
            // (#89 part 2)
            ("ScoreScreen", score_screen_wnd()),
        ];
        for n in ["BlankWindow", "ReplayMenu", "CreditsMenu", "LanLobbyMenu", "OptionsMenu", "ChallengeMenu", "PopupReplay"] {
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
    ///   animate manager are done completes it: two screens, the save/load screen on top (~~its callbacks are not translated:
///   recorded~~ **Corrected 2026-11-10 (#78):** its full-screen init runs, `crate::client::save_load_menu`);
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
        // (#78: the save/load screen's own init, not an untranslated name)
        assert!(h.save_load.parent.is_some() && !h.save_load.is_popup && h.save_load.just_entered);
        assert!(!h.untranslated.iter().any(|f| f.starts_with(b"SaveLoad")), "{:?}", h.untranslated);
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
