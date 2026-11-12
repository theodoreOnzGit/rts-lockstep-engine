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
//     ScoreScreenSystem, ScoreScreenInput, ScoreScreenInit, ScoreScreenUpdate, ScoreScreenShutdown; #88:
//     OptionsMenuSystem, OptionsMenuInput, OptionsMenuInit, OptionsMenuUpdate, OptionsMenuShutdown; #98:
//     ReplayMenuSystem, PopupReplaySystem, ChallengeMenuSystem, ReplayMenuInput, PopupReplayInput, ChallengeMenuInput,
//     ReplayMenuInit, PopupReplayInit, ChallengeMenuInit, ReplayMenuUpdate, PopupReplayUpdate, ChallengeMenuUpdate,
//     ReplayMenuShutdown, PopupReplayShutdown, ChallengeMenuShutdown; #87: QuitMenuSystem; #157: the tooltip callbacks
//     BattleHonorTooltip, playerTemplateComboBoxTooltip, playerTemplateListBoxTooltip, ExtendedMessageBoxSystem's
//     setting by gogoExMessageBox)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/System/W3DFunctionLexicon.cpp (#157: the
//     W3DDrawMapPreview entry of gameWinDrawTable)
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
//!   ([`ShellGui::skirmish_data`]); ~~the preference files are kept in memory (`ShellGui::prefs`)~~. **Corrected
//!   2026-11-10 (#89 part 2):** the score screen (`crate::client::score_screen`) is translated and dispatched here; the
//!   game it scores is the owner's copy ([`ShellGui::score_screen`]'s `game`). **Corrected 2026-11-10 (#89):** the
//!   preference files are the owner's user data folder ([`ShellGui::user_data`], read by the translated
//!   `UserPreferences::load`; the files written queued in [`ShellGui::user_data_writes`]), and the options screen
//!   (`crate::client::options_menu`) is translated and dispatched here, over the owner's copy of the global data
//!   ([`ShellGui::global_data`]); what the client lacks of it (the LOD manager, the display modes, the gamma, the
//!   registry, the IP addresses) is answered here as missing and recorded in [`ShellGui::untranslated`].
//!   **Corrected 2026-11-10 (#98):** the replay menu (`crate::client::replay_menu`), the score screen's replay popup
//!   (`crate::client::popup_replay`) and the challenge menu (`crate::client::challenge_menu`) are translated and
//!   dispatched here, with their message boxes' callbacks (`reallySaveReplay`, `reallyLoadReplay`,
//!   `deleteReplayFlag`, `copyReplayFlag`); the replay folder is the owner's ([`ShellGui::replays`]).
//!   **Corrected 2026-11-10 (#87):** the in-game quit menu (`crate::client::quit_menu`, `QuitMenuSystem`) is
//!   translated and dispatched here, with its boxes' callbacks (`exitQuitMenu`, `noExitQuitMenu`,
//!   `surrenderQuitMenu`, `restartMissionMenu`); `destroyQuitMenu` (the save/load popup's load, the options screen's
//!   accept) is its translation, no longer recorded as untranslated. What it reads of the game is the owner's copy
//!   ([`ShellGui::quit_menu`]'s `game`).
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

use crate::client::audio_info::AudioSettings;
use crate::client::options_menu::{self as om, DisplaySettings, OptionsHost, OptionsMenu};
use crate::client::user_preferences::{MemoryUserData, PreferenceMap, UserDataFiles, UserPreferences};
use crate::common::global_data::GlobalData;
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
use crate::client::challenge_menu::{self as cm, ChallengeHost, ChallengeMenu};
use crate::client::popup_replay::{self as pr, PopupReplay, PopupReplayHost};
use crate::client::quit_menu::{self as qm, QuitMenu, QuitMenuHost};
use crate::client::replay_menu::{self as rm, MemoryReplayFolder, ReplayListHost, ReplayListing, ReplayMenu, ReplayVersion};
use crate::formats::big::FilenameList;
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
    /// `TheScriptEngine->signalUIInteract`'s hooks, ~~(not run: no shell map)~~ **Corrected 2026-11-11, #142:** upstream's
    /// `TheShellHookNames` strings, recorded this frame; the app hands them to the World only under the owner's exception
    /// (#139, `crate::client::shell_hooks::apply_ui_interacts`).
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
    /// (#78) `TheNameKeyGenerator`: the table the reader gives the windows their keys from, and the callbacks ask
    /// (a clone of the host shares it, as the one global generator).
    pub keys: NameKeys,
    /// (#89) SkirmishGameOptionsMenu.cpp's statics, `TheSkirmishGameInfo` among them.
    pub skirmish: SkirmishGameOptions,
    /// (#89) The World's data the skirmish screens read, as the owner copied it (empty until it does: no maps).
    pub skirmish_data: SkirmishData,
    /// (#89) SkirmishMapSelectMenu.cpp's statics.
    pub map_select: SkirmishMapSelect,
    /// ~~(#89) The preference files (`Skirmish.ini`, `Network.ini`) by leaf name, in memory: `UserPreferences::load`
    /// reads them, `write` replaces them (the files themselves are not read or written).~~ **Corrected 2026-11-10
    /// (#89):** the user data folder as the owner gave it (`Options.ini`, `Skirmish.ini`, `Network.ini`, ... by their
    /// full paths, `path` the folder): the translated `UserPreferences::load` reads them; a `write` replaces the file
    /// here and queues it in [`ShellGui::user_data_writes`] for the owner to keep (on disk, or in OPFS).
    pub user_data: MemoryUserData,
    /// (#93) The files written since the owner last took them: (full path, bytes as written in text mode).
    pub user_data_writes: Vec<(Vec<u8>, Vec<u8>)>,
    /// (#88) OptionsMenu.cpp's statics.
    pub options: OptionsMenu,
    /// (#88) `TheWritableGlobalData` as the options screen reads and writes it: the client's copy (the owner gives it
    /// from the World's and applies what the screen changed; the logic's own is never written).
    pub global_data: GlobalData,
    /// (#88) `TheAudio->getAudioSettings()` (the owner's audio's; the defaults until it gives them).
    pub audio_settings: AudioSettings,
    /// (#88) `TheGameLODManager->getStaticLODLevel()`: kept, not applied (GameLOD.cpp is not translated).
    pub static_lod_level: i32,
    /// (#88) `TheDisplay`'s modes (width, height, bit depth): none, the client does not change its display mode.
    pub display_modes: Vec<(i32, i32, i32)>,
    /// (#88) `TheInGameUI`'s RMB scroll anchors (draw, move): kept for the owner (the client has none yet).
    pub rmb_scroll_anchor: (bool, bool),
    /// (#89) The map names a map list's item data stands for (item data `n` is entry `n - 1`).
    pub map_list_names: Vec<Vec<u8>>,
    /// ~~(#89) `SkirmishBattleHonors::getWins()` (the stats file is not read: 0 unless the owner sets it).~~
    /// **Corrected 2026-11-12, #157:** removed; `SkirmishBattleHonors` reads `SkirmishStats.ini` from
    /// [`ShellGui::user_data`]. The map previews `getMapPreviewImage` made (`newInstance(Image)` added to
    /// `TheMappedImageCollection`), each a 128x128 texture whose file the shell copied into
    /// `<user data>MapPreviews/` of [`ShellGui::user_data`].
    pub map_preview_images: Vec<crate::client::image::Image>,
    /// (#157) `GetUnsignedIntFromRegistry("", "Preorder", isPreorder)` gave a non-zero value: the owner's (false: the
    /// client reads no registry).
    pub preorder: bool,
    /// (#89 part 2) ScoreScreen.cpp's statics and globals, with the game it scores (the owner's copy).
    pub score_screen: ScoreScreen,
    /// (#98) ReplayMenu.cpp's statics.
    pub replay_menu: ReplayMenu,
    /// (#98) PopupReplay.cpp's statics.
    pub popup_replay: PopupReplay,
    /// (#98) ChallengeMenu.cpp's statics.
    pub challenge: ChallengeMenu,
    /// (#98) The replay folder as the owner gave it (each file's listing read by the owner with the logic's recorder);
    /// its writes and deletes are queued in [`ShellGui::requests`].
    pub replays: MemoryReplayFolder,
    /// (#87) QuitMenu.cpp's statics, `TheInGameUI`'s quit-menu flags and the game's state the menu reads (the owner's).
    pub quit_menu: QuitMenu,
    /// (#148) Diplomacy.cpp's statics (the briefing history and its window, `crate::client::diplomacy`), the window's
    /// slide (`theAnimateWindowManager`), and the quit menu's `HideDiplomacy` / `ResetDiplomacy` calls not yet made (the
    /// owner makes them on the window manager, [`ShellGui::diplomacy_with`]).
    pub diplomacy: crate::client::diplomacy::Diplomacy,
    pub diplomacy_animate: Option<crate::client::animate_window::AnimateWindowManager>,
    pub diplomacy_calls: Vec<&'static str>,
    /// (#151) `TheInGameUI`'s popup message and InGamePopupMessage.cpp's statics (`crate::client::in_game_popup_message`),
    /// and its calls on the game for the owner to make ([`PopupCall`]).
    pub popup: crate::client::in_game_popup_message::InGamePopupMessage,
    pub popup_calls: Vec<PopupCall>,
    /// (#153) The owner's text measure (`DisplayString::getSize`, `winFontHeight`); `None`, [`GuiHost`]'s default
    /// estimate (as the tests and the differential tests measure).
    pub text_metrics: Option<crate::client::window_manager::TextMetrics>,
}

/// (#151) A call of the popup message on the game, for the owner of the game to make.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupCall {
    /// `TheGameLogic->setGamePaused(paused, pauseMusic)`.
    SetGamePaused { paused: bool, pause_music: bool },
    /// `TheMessageStream->appendMessage(GameMessage::MSG_CLEAR_INGAME_POPUP_MESSAGE)`.
    ClearMessage,
}

/// (#151) The client's [`crate::client::in_game_popup_message::PopupMessageHost`] on the shell's window manager: the
/// layout made from the player's `InGamePopupMessage.wnd`, the text and its measure the shell's, the briefing history
/// the shell's (`ShellGui::diplomacy_with`); the pause and the clear message queued for the owner
/// ([`ShellGui::popup_calls`]).
pub struct ShellPopup<'a, W: WndFiles> {
    pub wm: &'a mut GameWindowManager,
    pub gui: &'a mut ShellGui<W>,
}

impl<W: WndFiles> crate::client::in_game_popup_message::PopupMessageHost for ShellPopup<'_, W> {
    fn popup(&mut self) -> &mut crate::client::in_game_popup_message::InGamePopupMessage {
        &mut self.gui.popup
    }
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        self.gui.text(label)
    }
    fn update_diplomacy_briefing_text(&mut self, label: &[u8]) {
        self.gui.diplomacy_with(self.wm, |h| crate::client::diplomacy::update_diplomacy_briefing_text(h, label, false));
    }
    fn display_size(&self) -> (i32, i32) {
        self.wm.display
    }
    fn set_game_paused(&mut self, paused: bool, pause_music: bool) {
        self.gui.popup_calls.push(PopupCall::SetGamePaused { paused, pause_music });
    }
    fn win_create_layout(&mut self, file: &[u8]) -> Option<usize> {
        ShellHost::create_layout(self.gui, self.wm, file)
    }
    fn layout_run_init(&mut self, layout: usize) {
        crate::client::shell::run_layout(self.wm, self.gui, layout, 0, None);
    }
    fn layout_destroy(&mut self, layout: usize) {
        crate::client::shell::destroy_layout(self.wm, layout);
    }
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        ShellHost::name_to_key(self.gui, name) as i32
    }
    fn win_get_window_from_id(&mut self, parent: Option<WinId>, id: i32) -> Option<WinId> {
        self.wm.win_get_window_from_id(parent, id)
    }
    fn win_get_font(&mut self, w: WinId) -> Option<crate::client::window_script::GameFont> {
        self.wm.tree.windows[w].inst.font.clone()
    }
    fn string_size(&mut self, text: &[u16], font: Option<&crate::client::window_script::GameFont>, wrap: i32) -> (i32, i32) {
        GuiHost::text_size(self.gui, font, text, Some(wrap))
    }
    fn static_text_set_text(&mut self, w: WinId, text: &[u16]) {
        crate::client::gadget::static_text_set_text(self.wm, self.gui, Some(w), text);
    }
    fn win_get_size(&mut self, w: WinId) -> (i32, i32) {
        self.wm.tree.windows[w].size
    }
    fn win_set_position(&mut self, w: WinId, x: i32, y: i32) {
        self.wm.win_set_position(w, x, y);
    }
    fn win_set_size(&mut self, w: WinId, width: i32, height: i32) {
        self.wm.win_set_size(self.gui, w, width, height);
    }
    fn win_set_enabled_text_colors(&mut self, w: WinId, color: i32, border_color: i32) {
        self.wm.win_set_text_colors(w, 0, color, border_color);
    }
    fn win_set_modal(&mut self, w: WinId) {
        self.wm.win_set_modal(Some(w));
    }
    fn win_set_focus(&mut self, w: WinId) {
        self.wm.win_set_focus(self.gui, Some(w));
    }
    fn win_hide(&mut self, w: WinId, hide: bool) {
        self.wm.win_hide(w, hide);
    }
    fn win_bring_to_top(&mut self, w: WinId) {
        self.wm.win_bring_to_top(w);
    }
    fn send_selected(&mut self, window: WinId, control: Option<WinId>, control_id: i32) {
        let d1 = crate::client::window_manager::win_data(control);
        self.wm.win_send_system_msg(self.gui, Some(window), crate::client::window_manager::ggm::GBM_SELECTED, d1, control_id as u32);
    }
    fn append_clear_popup_message(&mut self) {
        self.gui.popup_calls.push(PopupCall::ClearMessage);
    }
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
            keys: NameKeys::default(),
            skirmish: SkirmishGameOptions::default(),
            skirmish_data: SkirmishData::default(),
            map_select: SkirmishMapSelect::default(),
            user_data: MemoryUserData::default(),
            user_data_writes: Vec::new(),
            options: OptionsMenu::default(),
            global_data: GlobalData::new(),
            audio_settings: AudioSettings::default(),
            static_lod_level: crate::client::options_menu::STATIC_GAME_LOD_UNKNOWN,
            display_modes: Vec::new(),
            rmb_scroll_anchor: (false, false),
            map_list_names: Vec::new(),
            map_preview_images: Vec::new(),
            preorder: false,
            score_screen: ScoreScreen::default(),
            replay_menu: ReplayMenu::default(),
            popup_replay: PopupReplay::default(),
            challenge: ChallengeMenu::default(),
            replays: MemoryReplayFolder { files: Vec::new(), desktop: b"Desktop".to_vec() },
            quit_menu: QuitMenu::default(),
            diplomacy: Default::default(),
            diplomacy_animate: None,
            diplomacy_calls: Vec::new(),
            popup: Default::default(),
            popup_calls: Vec::new(),
            text_metrics: None,
        }
    }

    /// The files' text (`TheGameText`), through the reader's host.
    fn text(&self, label: &[u8]) -> Vec<u16> {
        self.files.as_ref().map_or_else(Vec::new, |f| f.files.fetch_text(label))
    }

    /// (#148) `f` on the briefing history's window (`crate::client::diplomacy`) with this host and `wm`; whether it asked
    /// for the quit menu to be hidden (`ToggleDiplomacy`'s `HideQuitMenu()`).
    pub fn diplomacy_with<R>(&mut self, wm: &mut GameWindowManager, f: impl FnOnce(&mut crate::client::diplomacy::WindowManagerDiplomacy<'_, Self, fn(&Self, &[u8]) -> Vec<u16>>) -> R) -> (R, bool) {
        let mut d = std::mem::take(&mut self.diplomacy);
        let mut a = self.diplomacy_animate.take();
        let fetch: fn(&Self, &[u8]) -> Vec<u16> = |h, l| h.text(l);
        let mut h = crate::client::diplomacy::WindowManagerDiplomacy { wm, host: self, diplomacy: &mut d, animate: &mut a, fetch, hide_quit_menu: false };
        let r = f(&mut h);
        let hide_quit_menu = h.hide_quit_menu;
        self.diplomacy = d;
        self.diplomacy_animate = a;
        (r, hide_quit_menu)
    }
}

impl<W: WndFiles> GuiHost for ShellGui<W> {
    // (#153) the owner's measure when it gave one ([`ShellGui::text_metrics`]), else the default estimate
    fn text_size(&self, font: Option<&crate::client::window_script::GameFont>, text: &[u16], wrap: Option<i32>) -> (i32, i32) {
        match self.text_metrics {
            Some(m) => (m.text_size)(font, text, wrap),
            None => crate::client::window_manager::NoGuiHost.text_size(font, text, wrap),
        }
    }
    fn font_height(&self, font: Option<&crate::client::window_script::GameFont>) -> i32 {
        match self.text_metrics {
            Some(m) => (m.font_height)(font),
            None => crate::client::window_manager::NoGuiHost.font_height(font),
        }
    }
    // (#98, defect fixed: the shell's host answered no image sizes and no images, `GuiHost`'s defaults, so every draw
    // and callback that measures a mapped image (`getImageWidth`) or asks whether one exists read 0 and false)
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        self.files.as_ref().and_then(|f| f.files.image_size(image))
    }
    fn has_image(&self, image: &[u8]) -> bool {
        self.files.as_ref().is_some_and(|f| f.files.has_image(image))
    }
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        match func {
            b"MainMenuSystem" => main_menu::main_menu_system(wm, self, win, msg, d1, d2),
            b"MessageBoxSystem" => message_box::message_box_system(wm, self, win, msg, d1, d2, false),
            b"QuitMessageBoxSystem" => message_box::message_box_system(wm, self, win, msg, d1, d2, true),
            // (#157)
            b"ExtendedMessageBoxSystem" => crate::client::extended_message_box::extended_message_box_system(wm, self, win, msg, d1, d2),
            b"SaveLoadMenuSystem" => slm::save_load_menu_system(wm, self, win, msg, d1, d2),
            b"SinglePlayerMenuSystem" => spm::single_player_menu_system(wm, self, win, msg, d1, d2),
            b"DifficultySelectSystem" => ds::difficulty_select_system(wm, self, win, msg, d1, d2),
            b"SkirmishGameOptionsMenuSystem" => sgo::skirmish_game_options_menu_system(wm, self, win, msg, d1, d2),
            b"SkirmishMapSelectMenuSystem" => smsm::skirmish_map_select_menu_system(wm, self, win, msg, d1, d2),
            // (#89 part 2)
            b"ScoreScreenSystem" => ss::score_screen_system(wm, self, win, msg, d1, d2),
            // (#88)
            b"OptionsMenuSystem" => om::options_menu_system(wm, self, win, msg, d1, d2),
            // (#98)
            b"ReplayMenuSystem" => rm::replay_menu_system(wm, self, win, msg, d1, d2),
            b"PopupReplaySystem" => pr::popup_replay_system(wm, self, win, msg, d1, d2),
            b"ChallengeMenuSystem" => cm::challenge_menu_system(wm, self, win, msg, d1, d2),
            // (#87)
            b"QuitMenuSystem" => qm::quit_menu_system(wm, self, win, msg, d1, d2),
            // (#148) the briefing history (`Diplomacy.wnd`)
            b"DiplomacySystem" => {
                let control_id = crate::client::window_manager::data_win(d1).map_or(0, |c| wm.tree.windows[c].id);
                self.diplomacy_with(wm, |h| crate::client::diplomacy::diplomacy_system(h, msg, control_id, None)).0
            }
            // (#151) the popup message (`InGamePopupMessage.wnd`): the focus wanted (`*(Bool *)mData2 = TRUE`)
            b"InGamePopupMessageSystem" => {
                let control_id = (msg == crate::client::window_manager::ggm::GBM_SELECTED).then(|| crate::client::window_manager::data_win(d1).map(|c| wm.tree.windows[c].id)).flatten();
                let mut focus = wm.focus_reply;
                let r = {
                    let mut h = ShellPopup { wm, gui: self };
                    let focus_arg = (msg == crate::client::window_manager::gwm::INPUT_FOCUS).then_some((d1 == 1, &mut focus));
                    crate::client::in_game_popup_message::in_game_popup_message_system(&mut h, msg, control_id, focus_arg)
                };
                wm.focus_reply = focus;
                r
            }
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
            b"OptionsMenuInput" => om::options_menu_input(wm, self, win, msg, d1, d2),
            // (#98)
            b"ReplayMenuInput" => rm::replay_menu_input(wm, self, win, msg, d1, d2),
            b"PopupReplayInput" => pr::popup_replay_input(wm, self, win, msg, d1, d2),
            b"ChallengeMenuInput" => cm::challenge_menu_input(wm, self, win, msg, d1, d2),
            // (#148)
            b"DiplomacyInput" => self.diplomacy_with(wm, |h| crate::client::diplomacy::diplomacy_input(h, msg, d1 as u8)).0,
            // (#151)
            b"InGamePopupMessageInput" => crate::client::in_game_popup_message::in_game_popup_message_input(&mut ShellPopup { wm, gui: self }, win, msg, d1 as u8, d2 as u8),
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
            // (#157)
            b"BattleHonorTooltip" => crate::client::battle_honors::battle_honor_tooltip(wm, self, win, mouse),
            b"playerTemplateComboBoxTooltip" => crate::client::lobby_utils::player_template_combo_box_tooltip(wm, self, win, mouse),
            b"playerTemplateListBoxTooltip" => crate::client::lobby_utils::player_template_list_box_tooltip(wm, self, win, mouse),
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
    fn draw(&mut self, wm: &GameWindowManager, func: &[u8], win: WinId, out: &mut Vec<crate::client::window_manager::DrawItem>) {
        // (#157) `W3DDrawMapPreview`: the window's `MapMetaData *`, `TheSupplyAndTechImageLocations`
        if func == b"W3DDrawMapPreview" {
            let extent = match &wm.ext[win].user {
                crate::client::window_manager::UserData::MapMetaData(Some(k)) => self.skirmish_data.map_cache.maps.get(k).map(|m| m.extent),
                _ => None,
            };
            let find = |n: &[u8]| self.find_mapped_image(n);
            crate::client::w3d_map_preview::w3d_draw_map_preview(wm, win, extent.as_ref(), &self.skirmish.supply_pos_list, &self.skirmish.tech_pos_list, &find, out);
        }
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
            // (#88)
            b"OptionsMenuInit" => om::options_menu_init(wm, self, layout),
            b"OptionsMenuUpdate" => om::options_menu_update(wm, self, layout),
            b"OptionsMenuShutdown" => om::options_menu_shutdown(wm, self, layout),
            // (#98)
            b"ReplayMenuInit" => rm::replay_menu_init(wm, self, layout),
            b"ReplayMenuUpdate" => rm::replay_menu_update(wm, self, layout),
            b"ReplayMenuShutdown" => rm::replay_menu_shutdown(wm, self, layout, user),
            b"PopupReplayInit" => pr::popup_replay_init(wm, self, layout),
            b"PopupReplayUpdate" => pr::popup_replay_update(wm, self, layout),
            b"PopupReplayShutdown" => pr::popup_replay_shutdown(wm, self, layout),
            b"ChallengeMenuInit" => cm::challenge_menu_init(wm, self, layout),
            b"ChallengeMenuUpdate" => cm::challenge_menu_update(wm, self, layout),
            b"ChallengeMenuShutdown" => cm::challenge_menu_shutdown(wm, self, layout, user),
            // (#151) the popup message's layout
            b"InGamePopupMessageInit" => crate::client::in_game_popup_message::in_game_popup_message_init(&mut ShellPopup { wm, gui: self }),
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
            // (#98)
            b"reallySaveReplay" => pr::really_save_replay(wm, self),
            b"reallyLoadReplay" => rm::really_load_replay(wm, self),
            b"deleteReplayFlag" => rm::delete_replay_flag(self),
            b"copyReplayFlag" => rm::copy_replay_flag(self),
            // (#87)
            b"exitQuitMenu" => qm::exit_quit_menu(wm, self),
            b"noExitQuitMenu" => qm::no_exit_quit_menu(self),
            b"surrenderQuitMenu" => qm::surrender_quit_menu(wm, self),
            b"restartMissionMenu" => qm::restart_mission_menu(wm, self),
            _ => self.untranslated.push(func.to_vec()),
        }
    }
    fn ex_message_box_callback(&mut self, wm: &mut GameWindowManager, func: &[u8], user_data: &crate::client::extended_message_box::ExUserData) -> crate::client::extended_message_box::MessageBoxReturn {
        use crate::client::extended_message_box::{ExUserData, MessageBoxReturn};
        // (#157) the insert-disc prompt's callbacks
        if func == sgo::CHECK_CD_CALLBACK.as_bytes() {
            let start = match user_data {
                ExUserData::Callback(n) => n.clone(),
                _ => Vec::new(),
            };
            return sgo::check_cd_callback(wm, self, |wm, h| match start.as_slice() {
                b"reallyDoStart" => sgo::really_do_start(wm, h),
                b"startNextCampaignGame" => ss::start_next_campaign_game(wm, h),
                // (`callback` NULL: nothing)
                _ => {}
            });
        }
        if func == sgo::CANCEL_START_NO_CD.as_bytes() {
            return sgo::cancel_start_because_of_no_cd(self);
        }
        if func == main_menu::CHECK_CD_CALLBACK.as_bytes() {
            let diff = if let ExUserData::Int(d) = user_data { *d } else { 0 };
            return main_menu::check_cd_callback(wm, self, diff);
        }
        if func == main_menu::CANCEL_START_NO_CD.as_bytes() {
            return main_menu::cancel_start_because_of_no_cd();
        }
        self.untranslated.push(func.to_vec());
        MessageBoxReturn::Close
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
    fn path_user_data(&self) -> Vec<u8> {
        self.user_data.path.clone()
    }
    fn user_data_read(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.user_data.user_data_read(path)
    }
    fn user_data_write(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        self.user_data.user_data_write(path, bytes);
        self.user_data_writes.push((path.to_vec(), bytes.to_vec()));
        true
    }
}

impl<W: WndFiles> ShellGui<W> {
    /// (#93) The preference file `leaf` of the user data folder as `UserPreferences::load` reads it (`None`: no file).
    pub fn pref_file(&mut self, leaf: &[u8]) -> Option<PreferenceMap> {
        let mut p = UserPreferences::new();
        p.load(&mut self.user_data, leaf).then_some(p.map)
    }

    /// (#93) The preference file `leaf` made from `map` as `UserPreferences::write` writes it.
    pub fn set_pref_file(&mut self, leaf: &[u8], map: &PreferenceMap) {
        let mut p = UserPreferences::new();
        p.load(&mut self.user_data, leaf);
        p.map = map.clone();
        p.write(&mut self.user_data);
    }
}

impl<W: WndFiles> OptionsHost for ShellGui<W> {
    fn options_menu(&mut self) -> &mut OptionsMenu {
        &mut self.options
    }
    fn global_data(&mut self) -> &mut GlobalData {
        &mut self.global_data
    }
    fn audio_settings(&self) -> AudioSettings {
        self.audio_settings.clone()
    }
    fn set_volume(&mut self, volume: f32, affect: u32) {
        self.requests.push(ShellRequest::SetVolume { volume, affect });
    }
    fn quitting(&self) -> bool {
        false
    }
    fn game_mode(&self) -> i32 {
        if self.env.in_shell_game {
            crate::common::recorder::game_mode::SHELL
        } else if self.env.in_game {
            crate::common::recorder::game_mode::SINGLE_PLAYER
        } else {
            crate::common::recorder::game_mode::NONE
        }
    }
    fn ip_addresses(&mut self) -> Vec<(Vec<u8>, u32)> {
        // (IPEnumeration: the client has no network)
        Vec::new()
    }
    fn registry_version(&mut self) -> u32 {
        self.untranslated.push(b"GetRegistryVersion".to_vec());
        0
    }
    fn registry_proxy(&mut self) -> Vec<u8> {
        self.untranslated.push(b"GetStringFromRegistry(Proxy)".to_vec());
        Vec::new()
    }
    fn set_proxy(&mut self, _proxy: &[u8]) {
        self.untranslated.push(b"SetStringInRegistry(Proxy), ghttpSetProxy".to_vec());
    }
    fn static_lod_level(&mut self) -> i32 {
        self.static_lod_level
    }
    fn set_static_lod_level(&mut self, level: i32) -> bool {
        // (the level kept; GameLODManager::setStaticLODLevel's settings are not applied: GameLOD.cpp)
        self.untranslated.push(b"GameLODManager::setStaticLODLevel".to_vec());
        let changed = self.static_lod_level != level;
        self.static_lod_level = level;
        changed
    }
    fn find_static_lod_level(&mut self) -> i32 {
        self.untranslated.push(b"GameLODManager::findStaticLODLevel".to_vec());
        crate::client::options_menu::STATIC_GAME_LOD_UNKNOWN
    }
    fn current_texture_reduction(&mut self) -> i32 {
        self.global_data.texture_reduction_factor
    }
    fn recommended_texture_reduction(&mut self) -> i32 {
        self.untranslated.push(b"GameLODManager::getRecommendedTextureReduction".to_vec());
        self.global_data.texture_reduction_factor
    }
    fn static_game_lod_level_name(&mut self, level: i32) -> Vec<u8> {
        // (GameLOD.cpp's StaticGameLODNames)
        match level {
            0 => b"Low".to_vec(),
            1 => b"Medium".to_vec(),
            2 => b"High".to_vec(),
            3 => b"Custom".to_vec(),
            _ => Vec::new(),
        }
    }
    fn adjust_lod(&mut self, _adj: i32) {
        self.untranslated.push(b"GameClient::adjustLOD".to_vec());
    }
    fn display_modes(&mut self) -> Vec<(i32, i32, i32)> {
        self.display_modes.clone()
    }
    fn display_settings(&mut self) -> DisplaySettings {
        DisplaySettings { x_res: self.global_data.x_resolution, y_res: self.global_data.y_resolution, bit_depth: 32, windowed: self.env.windowed }
    }
    fn set_display_mode(&mut self, _x: i32, _y: i32, _bits: i32, _windowed: bool) -> bool {
        self.untranslated.push(b"Display::setDisplayMode".to_vec());
        false
    }
    fn resolution_changed(&mut self, _wm: &mut GameWindowManager) {
        self.untranslated.push(b"OptionsMenu: the shell made anew after a display change".to_vec());
    }
    fn set_gamma(&mut self, _gamma: f32) {
        self.untranslated.push(b"Display::setGamma".to_vec());
    }
    fn draw_rmb_scroll_anchor(&mut self) -> &mut bool {
        &mut self.rmb_scroll_anchor.0
    }
    fn move_rmb_scroll_anchor(&mut self) -> &mut bool {
        &mut self.rmb_scroll_anchor.1
    }
    fn options_destroy_quit_menu(&mut self, wm: &mut GameWindowManager) {
        // (~~recorded as untranslated~~ **Corrected 2026-11-10 (#87):** translated)
        qm::destroy_quit_menu(wm, self);
    }
    fn do_resolution_dialog(&mut self, _wm: &mut GameWindowManager) {
        self.untranslated.push(b"DoResolutionDialog".to_vec());
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
    // (#157) the stats file in the owner's user data folder, as the other preference files
    fn grid_path_user_data(&self) -> Vec<u8> {
        MenuHost::path_user_data(self)
    }
    fn grid_user_data_read(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        MenuHost::user_data_read(self, path)
    }
    fn grid_user_data_write(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        MenuHost::user_data_write(self, path, bytes)
    }
}

impl<W: WndFiles> ScoreScreenHost for ShellGui<W> {}

impl<W: WndFiles> ReplayListHost for ShellGui<W> {
    fn replay_dir(&self) -> Vec<u8> {
        // (#98) `getPath_UserData()` (the user data folder's, empty in the client: a file opened by its name) and
        // `Replays\`
        let mut d = self.user_data.path.clone();
        d.extend_from_slice(b"Replays\\");
        d
    }
    fn replay_file_list(&mut self, dir: &[u8], search: &[u8]) -> FilenameList {
        self.replays.list(dir, search)
    }
    fn read_replay_header(&mut self, filename: &[u8]) -> Option<ReplayListing> {
        let dir = self.replay_dir();
        self.replays.header(&dir, filename)
    }
    fn replay_version(&self) -> ReplayVersion {
        // (#98) as the World records them: no version strings, version 0, the global data's CRCs
        ReplayVersion { exe_crc: self.global_data.exe_crc, ini_crc: self.global_data.ini_crc, ..Default::default() }
    }
    fn replay_file_exists(&mut self, path: &[u8]) -> bool {
        self.replays.find(path).is_some()
    }
    fn delete_replay_file(&mut self, path: &[u8]) -> Result<(), Vec<u16>> {
        let r = self.replays.delete(path)?;
        self.requests.push(r);
        Ok(())
    }
    fn copy_replay_file(&mut self, from: &[u8], to: &[u8]) -> Result<(), Vec<u16>> {
        let r = self.replays.copy(from, to)?;
        self.requests.push(r);
        Ok(())
    }
    fn desktop_path(&self) -> Vec<u8> {
        self.replays.desktop.clone()
    }
    fn replay_menu(&mut self) -> &mut ReplayMenu {
        &mut self.replay_menu
    }
}

impl<W: WndFiles> PopupReplayHost for ShellGui<W> {
    fn popup_replay(&mut self) -> &mut PopupReplay {
        &mut self.popup_replay
    }
}

impl<W: WndFiles> ChallengeHost for ShellGui<W> {
    fn challenge_menu(&mut self) -> &mut ChallengeMenu {
        &mut self.challenge
    }
}

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
    fn destroy_quit_menu(&mut self, wm: &mut GameWindowManager) {
        // (~~recorded as untranslated~~ **Corrected 2026-11-10 (#87):** translated)
        qm::destroy_quit_menu(wm, self);
    }
}

impl<W: WndFiles> QuitMenuHost for ShellGui<W> {
    fn quit_menu(&mut self) -> &mut QuitMenu {
        &mut self.quit_menu
    }
    fn hide_in_game_windows(&mut self, what: &'static str) {
        // (#148: `HideDiplomacy` and `ResetDiplomacy`, the briefing history's window, made by the owner on the window
        // manager after the quit menu's call)
        if what == "HideDiplomacy" || what == "ResetDiplomacy" {
            self.diplomacy_calls.push(what);
            return;
        }
        // (the cursor, ~~the diplomacy~~ (#148) the chat windows and the control bar's science purchase are not the shell's)
        if !self.untranslated.iter().any(|f| f == what.as_bytes()) {
            self.untranslated.push(what.as_bytes().to_vec());
        }
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
        // (#157: the map previews `getMapPreviewImage` added to the collection come after the collection's own)
        self.files.as_ref().and_then(|f| f.files.find_image(name)).or_else(|| self.map_preview_images.iter().find(|i| i.name == name).map(|i| i.name.clone()))
    }
    fn endurance_medal(&self, map: &[u8], state: i32) -> i32 {
        // (#157) `battleHonors->getEnduranceMedal(map, state)`: the stats file read (a copy of the folder: the read
        // changes nothing)
        let mut fs = self.user_data.clone();
        crate::client::battle_honors::SkirmishBattleHonors::new(&mut fs).get_endurance_medal(map, state)
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
    fn map_preview_from_file(&mut self, tga: &[u8], name: &[u8], temp_name: &[u8]) -> Option<Vec<u8>> {
        // ~~(#89) `copyFromBigToDir` and the new `Image`: not translated~~ **Corrected 2026-11-12, #157:** translated
        // (`crate::client::map_util::map_preview_from_file`) over the map files the owner gave
        // ([`SkirmishData::map_previews`]) and the shell's view of the user data folder
        let mut fs = MapPreviewFiles { previews: &self.skirmish_data.map_previews, user: &mut self.user_data };
        let image = crate::client::map_util::map_preview_from_file(&mut fs, tga, name, temp_name)?;
        let n = image.name.clone();
        self.map_preview_images.push(image);
        Some(n)
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
    fn is_preorder(&self) -> bool {
        self.preorder
    }
    fn untranslated(&mut self, what: &[u8]) {
        self.untranslated.push(what.to_vec());
    }
}

/// (#157) `getMapPreviewImage`'s files: the owner's map previews ([`SkirmishData::map_previews`], by their lower-case
/// paths: `TheFileSystem`'s archives match without case) and the shell's view of the user data folder, where the copy
/// is kept for the session (the client's folder keeps only preference files: the copy is not written to disk or OPFS).
pub struct MapPreviewFiles<'a> {
    pub previews: &'a std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
    pub user: &'a mut MemoryUserData,
}

impl crate::client::map_util::PreviewFiles for MapPreviewFiles<'_> {
    fn does_file_exist(&self, path: &[u8]) -> bool {
        self.previews.contains_key(&path.to_ascii_lowercase()) || self.user.files.contains_key(path)
    }
    fn read_file(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.previews.get(&path.to_ascii_lowercase()).cloned().or_else(|| self.user.files.get(path).cloned())
    }
    fn write_file(&mut self, path: &[u8], bytes: &[u8]) -> bool {
        self.user.files.insert(path.to_vec(), bytes.to_vec());
        true
    }
    fn path_user_data(&self) -> Vec<u8> {
        self.user.path.clone()
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
    /// text; the superweapon box at (20, 330); the starting money's combo box at (20, 360); `SubParent`. (#157) The map
/// window draws with `W3DDrawMapPreview`; the honours list `ListboxInfo` (four columns, length 100) at
/// (500, 320)-(780, 500), and the record's four numbers (`StaticTextStreakValue`, `...BestStreakValue`,
/// `...WinsValue`, `...LossesValue`) at x 300 from y 420, 25 apart.
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
        // (#157: the preview's draw callback, as the game's file names it)
        c += &win(f, "USER", "MapWindow", (500, 40, 700, 240), false, "    SYSTEMCALLBACK = \"PassSelectedButtonsToParentSystem\";\n    DRAWCALLBACK = \"W3DDrawMapPreview\";\n", &start_spots(f, 500, 40));
        // (#157) the battle honours: `ListboxInfo` (four columns) and the record's four numbers
        c += &win(
            f,
            "SCROLLLISTBOX",
            "ListboxInfo",
            (500, 320, 780, 500),
            false,
            "    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 4, COLUMNSWIDTH%: 25, COLUMNSWIDTH%: 25, COLUMNSWIDTH%: 25, COLUMNSWIDTH%: 25, FORCESELECT: 0;\n",
            "",
        );
        for (i, n) in ["StaticTextStreakValue", "StaticTextBestStreakValue", "StaticTextWinsValue", "StaticTextLossesValue"].iter().enumerate() {
            let y = 420 + 25 * i as i32;
            c += &static_text(f, n, (300, y, 480, y + 20));
        }
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
    /// `WinMapPreview` at (410, 120)-(610, 320) (#157: drawn with `W3DDrawMapPreview`) holding the eight start-spot buttons; *OK* at (410, 500) and *Back*
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
        c += &win(f, "USER", "WinMapPreview", (410, 120, 610, 320), false, "    DRAWCALLBACK = \"W3DDrawMapPreview\";\n", &start_spots(f, 410, 120));
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

    /// (#98) A synthetic `Menus/ReplayMenu.wnd` (no EA file): `ParentReplayMenu` (the system and input callbacks)
    /// with the replay list (four columns, 40/20/20/20 %) at (20, 60)-(560, 400), *Load*, *Delete*, *Copy* at x 610
    /// from y 110, *Back* at (20, 560), and `GadgetParent`.
    pub fn replay_menu_wnd() -> Vec<u8> {
        let f = "ReplayMenu.wnd";
        let mut c = String::new();
        c += &win(
            f,
            "SCROLLLISTBOX",
            "ListboxReplayFiles",
            (20, 60, 560, 400),
            false,
            "    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 4, COLUMNSWIDTH%: 40, COLUMNSWIDTH%: 20, COLUMNSWIDTH%: 20, COLUMNSWIDTH%: 20, FORCESELECT: 0;\n",
            "",
        );
        c += &buttons(f, 610, 110, &["ButtonLoadReplay", "ButtonDeleteReplay", "ButtonCopyReplay"]);
        c += &buttons(f, 20, 560, &["ButtonBack"]);
        c += &win(f, "USER", "GadgetParent", (0, 590, 10, 600), false, "", "");
        let parent = win(f, "USER", "ParentReplayMenu", (0, 0, 800, 600), false, "    SYSTEMCALLBACK = \"ReplayMenuSystem\";\n    INPUTCALLBACK = \"ReplayMenuInput\";\n", &c);
        layout_file("ReplayMenuInit", "ReplayMenuUpdate", "ReplayMenuShutdown", &parent)
    }

    /// (#98) A synthetic `Menus/PopupReplay.wnd` (no EA file): `PopupReplayMenu` (the system and input callbacks) at
    /// (100, 50)-(700, 550) with the games list (four columns) at (110, 60)-(500, 400), the name entry
    /// `TextEntryReplayName` at (110, 420)-(500, 440), `MenuButtonFrame` holding *Save* and *Back* at x 520 from y 110,
    /// and the *replay saved* window `PopupReplaySaved` at (250, 250)-(550, 300).
    pub fn popup_replay_wnd() -> Vec<u8> {
        let f = "PopupReplay.wnd";
        let mut c = String::new();
        c += &win(
            f,
            "SCROLLLISTBOX",
            "ListboxGames",
            (110, 60, 500, 400),
            false,
            "    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 4, COLUMNSWIDTH%: 40, COLUMNSWIDTH%: 20, COLUMNSWIDTH%: 20, COLUMNSWIDTH%: 20, FORCESELECT: 0;\n",
            "",
        );
        c += &win(f, "ENTRYFIELD", "TextEntryReplayName", (110, 420, 500, 440), false, "    TEXTENTRYDATA = MAXLEN: 64, SECRETTEXT: 0, NUMERICALONLY: 0, ALPHANUMERICALONLY: 0, ASCIIONLY: 0;\n", "");
        c += &frame(f, "MenuButtonFrame", (510, 100, 690, 200), false, &buttons(f, 520, 110, &["ButtonSave", "ButtonBack"]));
        c += &static_text(f, "PopupReplaySaved", (250, 250, 550, 300));
        let parent = win(f, "USER", "PopupReplayMenu", (100, 50, 700, 550), false, "    SYSTEMCALLBACK = \"PopupReplaySystem\";\n    INPUTCALLBACK = \"PopupReplayInput\";\n", &c);
        layout_file("PopupReplayInit", "PopupReplayUpdate", "PopupReplayShutdown", &parent)
    }

    /// (#98) A synthetic `Menus/ChallengeMenu.wnd` (no EA file): `ParentChallengeMenu` (the system and input
    /// callbacks) with the twelve general buttons `GeneralPosition0` .. `11` (check boxes, 40x40, three rows of four
    /// from (20, 40), 50 apart), *Play* (600, 560), *Back* (20, 560), `MainBackdrop`, `GadgetParent`, and
    /// `GeneralsBioParent` at (300, 40)-(780, 400) holding `BioPortrait` and the four bio lines (static texts at x 420
    /// from y 50, 30 apart).
    pub fn challenge_menu_wnd() -> Vec<u8> {
        let f = "ChallengeMenu.wnd";
        let mut c = String::new();
        for i in 0..12 {
            let (x, y) = (20 + 50 * (i % 4), 40 + 50 * (i / 4));
            c += &win(f, "CHECKBOX", &format!("GeneralPosition{i}"), (x, y, x + 40, y + 40), false, "", "");
        }
        c += &buttons(f, 600, 560, &["ButtonPlay"]);
        c += &buttons(f, 20, 560, &["ButtonBack"]);
        c += &win(f, "USER", "MainBackdrop", (0, 0, 10, 10), false, "", "");
        c += &win(f, "USER", "GadgetParent", (0, 590, 10, 600), false, "", "");
        let mut bio = win(f, "USER", "BioPortrait", (310, 50, 410, 150), false, "", "");
        for (i, n) in ["BioNameEntry", "BioDOBEntry", "BioBirthplaceEntry", "BioStrategyEntry"].iter().enumerate() {
            let y = 50 + 30 * i as i32;
            bio += &static_text(f, n, (420, y, 770, y + 20));
        }
        c += &win(f, "USER", "GeneralsBioParent", (300, 40, 780, 400), false, "", &bio);
        let parent = win(f, "USER", "ParentChallengeMenu", (0, 0, 800, 600), false, "    SYSTEMCALLBACK = \"ChallengeMenuSystem\";\n    INPUTCALLBACK = \"ChallengeMenuInput\";\n", &c);
        layout_file("ChallengeMenuInit", "ChallengeMenuUpdate", "ChallengeMenuShutdown", &parent)
    }

    /// (#87) A synthetic `Menus/QuitMenu.wnd`, or with `no_save` `Menus/QuitNoSave.wnd` (no EA file): the parent
    /// `QuitMenuParent` (hidden in the file: the window transitions show it; the system callback `QuitMenuSystem`) at
    /// (300, 150)-(500, 450), with *Exit*, *Restart*, *Return*, *Options* and (not in `QuitNoSave`) *Save/Load*, at
    /// x 310 from y 160, 25 apart. No layout callbacks (the lexicon has none for it).
    pub fn quit_menu_wnd(no_save: bool) -> Vec<u8> {
        let f = if no_save { "QuitNoSave.wnd" } else { "QuitMenu.wnd" };
        let names: &[&str] = if no_save { &["ButtonExit", "ButtonRestart", "ButtonReturn", "ButtonOptions"] } else { &["ButtonExit", "ButtonRestart", "ButtonReturn", "ButtonOptions", "ButtonSaveLoad"] };
        let parent = win(f, "USER", "QuitMenuParent", (300, 150, 500, 450), true, "    SYSTEMCALLBACK = \"QuitMenuSystem\";\n", &buttons(f, 310, 160, names));
        layout_file("[None]", "[None]", "[None]", &parent)
    }

    /// (#148) A `Diplomacy.wnd` (the briefing history's window, made by `ShowDiplomacy`): the parent with Diplomacy.cpp's
    /// callbacks, the solo parent with its list box, the in-game and buddies parents, the two radio buttons and the hide
    /// button. Test data, not EA's layout.
    pub fn diplomacy_wnd() -> Vec<u8> {
        let f = "Diplomacy.wnd";
        let list = "    LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 1, FORCESELECT: 0;\n";
        let solo = win(f, "USER", "SoloParent", (100, 100, 700, 400), false, "", &win(f, "SCROLLLISTBOX", "ListboxSolo", (120, 120, 680, 360), false, list, ""));
        let mut c = solo;
        c += &win(f, "USER", "InGameParent", (100, 100, 700, 400), false, "", "");
        c += &win(f, "USER", "BuddiesParent", (100, 100, 700, 400), false, "", "");
        c += &win(f, "RADIOBUTTON", "RadioButtonInGame", (110, 80, 200, 98), false, "    RADIOBUTTONDATA = GROUP: 1;\n", "");
        c += &win(f, "RADIOBUTTON", "RadioButtonBuddies", (210, 80, 300, 98), false, "    RADIOBUTTONDATA = GROUP: 1;\n", "");
        c += &buttons(f, 600, 410, &["ButtonHide"]);
        let parent = win(f, "USER", "DiplomacyParent", (90, 70, 710, 450), false, "    SYSTEMCALLBACK = \"DiplomacySystem\";\n    INPUTCALLBACK = \"DiplomacyInput\";\n", &c);
        layout_file("[None]", "[None]", "[None]", &parent)
    }

    /// (#151) An `InGamePopupMessage.wnd` (the popup message's window, made by `popupMessage`): the hidden parent with
    /// InGamePopupMessage.cpp's callbacks and its layout init, the static text and the OK button (60x20). Test data, not
    /// EA's layout.
    pub fn in_game_popup_message_wnd() -> Vec<u8> {
        let f = "InGamePopupMessage.wnd";
        let mut c = win(f, "STATICTEXT", "StaticTextMessage", (2, 2, 196, 40), false, "    STATICTEXTDATA = CENTERED: 0;\n", "");
        c += &win(f, "PUSHBUTTON", "ButtonOk", (138, 44, 198, 64), false, "", "");
        let parent = win(f, "USER", "InGamePopupMessageParent", (0, 0, 200, 66), true, "    SYSTEMCALLBACK = \"InGamePopupMessageSystem\";\n    INPUTCALLBACK = \"InGamePopupMessageInput\";\n", &c);
        layout_file("InGamePopupMessageInit", "[None]", "[None]", &parent)
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
        // (#98) the replay and challenge menus' fades, on their parents, not fire-once (their shutdowns reverse them)
        s += "WindowTransition ReplayMenuFade\n  Window\n    WinName = ReplayMenu.wnd:ParentReplayMenu\n    Style = FLASH\n    FrameDelay = 0\n  End\nEnd\n";
        // (#87) the quit menu's groups, on its parent: the shows (`QuitFull`, `QuitNoSave`) and the hides, reversed
        // (`QuitFullBack`, `QuitNoSaveBack`)
        for (g, f) in [("QuitFull", "QuitMenu"), ("QuitFullBack", "QuitMenu"), ("QuitNoSave", "QuitNoSave"), ("QuitNoSaveBack", "QuitNoSave")] {
            s += &format!("WindowTransition {g}\n  Window\n    WinName = {f}.wnd:QuitMenuParent\n    Style = FLASH\n    FrameDelay = 0\n  End\nEnd\n");
        }
        s += "WindowTransition ChallengeMenuFade\n  Window\n    WinName = ChallengeMenu.wnd:ParentChallengeMenu\n    Style = FLASH\n    FrameDelay = 0\n  End\nEnd\n";
        // (#89) the skirmish screen's fade, on its parent, not fire-once (its shutdown reverses it)
        s += "WindowTransition SkirmishGameOptionsMenuFade\n  Window\n    WinName = SkirmishGameOptionsMenu.wnd:SkirmishGameOptionsMenuParent\n    Style = FLASH\n    FrameDelay = 0\n  End\nEnd\n";
        for (g, w) in [("US", "US"), ("GLA", "GLA"), ("China", "China"), ("Training", "Training"), ("Skirmish", "Skirmish")] {
            s += &format!("WindowTransition MainMenuFaction{g}\n  Window\n    WinName = MainMenu.wnd:WinFaction{w}\n    Style = WINSCALEUP\n  End\nEnd\n");
        }
        s.into_bytes()
    }

    /// (#88) A synthetic `Menus/OptionsMenu.wnd` (no EA file), with every window OptionsMenu.cpp finds by name under
    /// `OptionsMenuParent` (the system and input callbacks): the four combo boxes and the two IP lists at x 20 from
    /// y 20, 25 apart; the check boxes at x 250 from y 20, 22 apart; the sliders (0 .. 100; the texture one 0 .. 2,
    /// the particle one 100 .. 5000) at x 500 from y 20, 25 apart; the two text entries, the version label;
    /// *Back*, *Defaults*, *Accept*, *Keyboard Options*, *Firewall Refresh* at y 560; and the advanced display window
    /// (hidden by the init) holding its controls and its *Accept* / *Back*.
    pub fn options_menu_wnd() -> Vec<u8> {
        let f = "OptionsMenu.wnd";
        let mut c = String::new();
        for (i, n) in ["ComboBoxIP", "ComboBoxOnlineIP", "ComboBoxAntiAliasing", "ComboBoxResolution", "ComboBoxDetail"].iter().enumerate() {
            let y = 20 + 25 * i as i32;
            c += &combo(f, n, (20, y, 200, y + 20));
        }
        let checks = ["CheckAlternateMouse", "Retaliation", "CheckDoubleClickAttackMove", "CheckLanguageFilter", "CheckSendDelay", "CheckBoxDrawAnchor", "CheckBoxMoveAnchor", "CheckBoxSaveCamera", "CheckBoxUseCamera"];
        for (i, n) in checks.iter().enumerate() {
            let y = 20 + 22 * i as i32;
            c += &win(f, "CHECKBOX", n, (250, y, 450, y + 20), false, "", "");
        }
        for (i, n) in ["SliderScrollSpeed", "SliderMusicVolume", "SliderSFXVolume", "SliderVoiceVolume", "SliderGamma"].iter().enumerate() {
            let y = 20 + 25 * i as i32;
            c += &win(f, "HORZSLIDER", n, (500, y, 700, y + 20), false, "    SLIDERDATA = MINVALUE: 0, MAXVALUE: 100;\n", "");
        }
        let entry = "    TEXTENTRYDATA = MAXLEN: 64, SECRETTEXT: 0, NUMERICALONLY: 0, ALPHANUMERICALONLY: 0, ASCIIONLY: 0;\n";
        c += &win(f, "ENTRYFIELD", "TextEntryHTTPProxy", (20, 200, 200, 220), false, entry, "");
        c += &win(f, "ENTRYFIELD", "TextEntryFirewallPortOverride", (20, 230, 200, 250), false, entry, "");
        c += &static_text(f, "LabelVersion", (20, 530, 300, 550));
        c += &buttons(f, 20, 560, &["ButtonBack"]);
        c += &buttons(f, 140, 560, &["ButtonDefaults"]);
        c += &buttons(f, 260, 560, &["ButtonAccept"]);
        c += &buttons(f, 380, 560, &["ButtonKeyboardOptions"]);
        c += &buttons(f, 500, 560, &["ButtonFirewallRefresh"]);
        let mut a = String::new();
        let adv = ["Check3DShadows", "Check2DShadows", "CheckCloudShadows", "CheckGroundLighting", "CheckSmoothWater", "CheckExtraAnimations", "CheckNoDynamicLOD", "CheckHeatEffects", "CheckUnlockFPS", "CheckBehindBuilding", "CheckShowProps"];
        for (i, n) in adv.iter().enumerate() {
            let y = 310 + 18 * i as i32;
            a += &win(f, "CHECKBOX", n, (210, y, 400, y + 16), false, "", "");
        }
        a += &win(f, "HORZSLIDER", "LowResSlider", (410, 310, 590, 330), false, "    SLIDERDATA = MINVALUE: 0, MAXVALUE: 2;\n", "");
        a += &win(f, "HORZSLIDER", "ParticleCapSlider", (410, 340, 590, 360), false, "    SLIDERDATA = MINVALUE: 100, MAXVALUE: 5000;\n", "");
        a += &buttons(f, 410, 470, &["ButtonAdvanceAccept"]);
        a += &buttons(f, 500, 470, &["ButtonAdvanceBack"]);
        c += &frame(f, "WinAdvancedDisplayOptions", (200, 300, 600, 500), false, &a);
        let parent = win(f, "USER", "OptionsMenuParent", (0, 0, 800, 600), false, "    SYSTEMCALLBACK = \"OptionsMenuSystem\";\n    INPUTCALLBACK = \"OptionsMenuInput\";\n", &c);
        layout_file("OptionsMenuInit", "OptionsMenuUpdate", "OptionsMenuShutdown", &parent)
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
            // (#88)
            ("OptionsMenu", options_menu_wnd()),
            // (#98)
            ("ReplayMenu", replay_menu_wnd()),
            ("PopupReplay", popup_replay_wnd()),
            ("ChallengeMenu", challenge_menu_wnd()),
            // (#87)
            ("QuitMenu", quit_menu_wnd(false)),
            ("QuitNoSave", quit_menu_wnd(true)),
        ];
        for n in ["BlankWindow", "CreditsMenu", "LanLobbyMenu", "KeyboardOptionsMenu"] {
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

    /// #157: the main menu's insert-disc prompt (`checkCDBeforeCampaign`, ExtendedMessageBox.cpp). Predicted: without
    /// the disc *Medium* opens the box (OK and Cancel shown, the title `GUI:InsertCDPrompt`), no start; *OK* still
    /// without it keeps the box; with the disc *OK* closes it and the campaign starts as from *Medium* (`StartCampaign
    /// { USA, 1 }` once the fade is done).
    #[test]
    fn the_main_menu_asks_for_the_disc() {
        let (mut wm, mut h) = booted();
        wm.win_process_key(&mut h, key::SPACE, key::STATE_DOWN);
        settle(&mut wm, &mut h);
        for b in ["ButtonSinglePlayer", "ButtonUSA"] {
            let b = w(&wm, b);
            click(&mut wm, &mut h, b);
            settle(&mut wm, &mut h);
        }
        h.first_cd_present = false;
        let newest = |wm: &GameWindowManager, name: &str| {
            let full = format!("MessageBox.wnd:{name}");
            (0..wm.tree.windows.len()).rev().find(|&i| wm.tree.windows[i].inst.decorated_name_string == full.as_bytes() && wm.tree.windows[i].status & crate::client::window_script::win_status::DESTROYED == 0).unwrap_or_else(|| panic!("{name}"))
        };
        let gone = |wm: &GameWindowManager, w: WinId| wm.tree.windows[w].status & crate::client::window_script::win_status::DESTROYED != 0;
        let b = w(&wm, "ButtonMedium");
        click(&mut wm, &mut h, b);
        let parent = newest(&wm, "MessageBoxParent");
        let title = newest(&wm, "StaticTextTitle");
        assert_eq!(String::from_utf16_lossy(&crate::client::gadget::static_text_get_text(&wm, title)), String::from_utf16_lossy(&h.text(b"GUI:InsertCDPrompt")));
        let ok = newest(&wm, "ButtonOk");
        click(&mut wm, &mut h, ok);
        assert!(!gone(&wm, parent) && !h.main_menu.start_game);
        h.first_cd_present = true;
        click(&mut wm, &mut h, ok);
        assert!(gone(&wm, parent) && h.main_menu.start_game);
        settle(&mut wm, &mut h);
        assert_eq!(h.requests, [ShellRequest::StartCampaign { campaign: b"USA".to_vec(), difficulty: 1 }]);
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
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
        assert!(h.ui_interacts.contains(&"ShellMainMenuExitPushed"));
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
    /// *Options* loads `Menus/OptionsMenu.wnd` once and runs its ~~(untranslated)~~ init (**Corrected 2026-11-10
    /// (#88):** translated; its *Back* closes it); *World Builder* cannot spawn, so
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
        assert!(!h.untranslated.contains(&b"OptionsMenuInit".to_vec()));
        // (#88) the options screen is modal over the menu; its Back destroys it
        let back = wm.tree.find_by_name(b"OptionsMenu.wnd:ButtonBack").expect("back");
        click(&mut wm, &mut h, back);
        assert!(wm.layouts[o].deleted && h.shell.options_layout.is_none());
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
        assert!(h.ui_interacts.contains(&"ShellMainMenuExitHighlighted"));
        wm.win_hide(w(&wm, "MapBorder"), false);
        let u = w(&wm, "ButtonUSA");
        let (x, y) = wm.tree.screen_position(u);
        wm.win_process_mouse_event(&mut h, gwm::MOUSE_POS, (x + 5, y + 5), (0, 0));
        assert!(h.ui_interacts.contains(&"ShellMainMenuExitUnhighlighted"));
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
