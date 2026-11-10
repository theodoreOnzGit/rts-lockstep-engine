// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GUIUtil.cpp (the file's static winInitialized,
//     EnableSlotListUpdates, AreSlotListUpdatesEnabled, EnableAcceptControls, ShowUnderlyingGUIElements,
//     PopulateColorComboBox, PopulatePlayerTemplateComboBox, PopulateTeamComboBox,
//     formatMoneyForStartingCashComboBox, PopulateStartingCashComboBox, UpdateSlotList)
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANAPICallbacks.cpp (acceptTrueColor, acceptFalseColor)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The slot list's shared GUI (GitHub #89): GUIUtil.cpp, which the skirmish, LAN and online game-options screens share
//! to fill their colour, faction, team and starting-money combo boxes and to keep the slot list's controls in step with
//! the game's slots.
//!
//! # As upstream reads it
//!
//! **`UpdateSlotList`** (when `EnableSlotListUpdates(TRUE)` allows it) enables each slot's controls (its own and, for
//! the host, the computer players'), writes a human's name into the player combo box or selects an AI's state, refills
//! the colour list with the colours no other slot holds, and selects each combo box's row whose item data is the slot's
//! colour, team and faction. Selecting a row sends `GCM_SELECTED` to the screen, whose handlers write the choice back
//! into the slot: upstream's screens rely on it. **`PopulateColorComboBox`** lists *???* (random) then the free
//! colours; **`PopulatePlayerTemplateComboBox`** *Random* then one entry per side of a faction that has a starting
//! building (not locked by the challenge, not new when the game is old-factions-only); **`PopulateTeamComboBox`** no
//! team then teams 1 to 4; **`PopulateStartingCashComboBox`** the starting-money choices, the game's selected.
//! **`ShowUnderlyingGUIElements`** hides or shows a screen's named and per-player gadgets (a map picker over them).
//!
//! # Upstream quirks that are kept
//!
//! - `PopulateStartingCashComboBox`: when the game's amount is not among the choices it is added, and its item data
//!   is read from the list's **end** iterator (`it->countMoney()` after the loop): undefined in C++, recorded as such.
//! - `UpdateSlotList` dereferences the game (`myGame->getMap()`) before its `if (myGame)`; `EnableAcceptControls` reads
//!   `getConstSlot(slotNum)` unchecked (a local slot of -1 is `NULL`): both recorded as undefined.
//! - `GadgetComboBoxGetLength` of a `NULL` combo box reads its user data: recorded as undefined.
//!
//! # Where this differs from upstream
//!
//! - `myGame` is the host's ([`GuiUtilHost::util_game`]), read anew at each use: the combo boxes' `GCM_SELECTED`
//!   handlers change the game while `UpdateSlotList` runs, as upstream's pointer sees.
//! - The data upstream reads from `TheMultiplayerSettings`, `ThePlayerTemplateStore`, `TheChallengeGenerals` and
//!   `TheMapCache` is a copy the client takes of its World's ([`SkirmishData`]): the shell never reads the World.
//!
//! # Representation
//!
//! Free functions over a [`GuiUtilHost`]; a screen's arrays of controls are `[Option<WinId>; MAX_SLOTS]` (`None`:
//! `NULL`). [`SkirmishData`] is the copy of the World's data, and the shell's `GameInfoHost`.

use std::collections::BTreeSet;

use crate::client::challenge_generals::ChallengeGenerals;
use crate::client::control_bar_popup::{format_wide, FormatArg};
use crate::client::gadget_combo_box as cb;
use crate::client::main_menu::MenuHost;
use crate::client::map_util::{would_map_transfer, MapUtilHost};
use crate::client::window_manager::GameWindowManager;
use crate::client::window_script::{win_status, WinId};
use crate::common::game_info::{GameInfo, GameInfoHost, GameSlot, MapMeta, MAX_SLOTS, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM};
use crate::common::game_state::{GameState, GameStateHost};
use crate::common::ini::{game_make_color, IniError, Undefined};
use crate::common::map_cache::MapCache;
use crate::common::money::Money;
use crate::common::multiplayer_settings::MultiplayerSettings;
use crate::common::science::GameTextFetch;
use crate::common::xfer::{Xfer, XferResult};
use crate::logic::world::game_data::TextTable;
use crate::logic::world::game_logic::World;

/// `acceptTrueColor` (LANAPICallbacks.cpp).
pub fn accept_true_color() -> i32 {
    game_make_color(0, 255, 0, 255)
}

/// `acceptFalseColor` (LANAPICallbacks.cpp).
pub fn accept_false_color() -> i32 {
    game_make_color(255, 0, 0, 255)
}

/// What the game-options screens read of a player template (`ThePlayerTemplateStore->getNthPlayerTemplate(n)`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TemplateView {
    /// `getName()`.
    pub name: Vec<u8>,
    /// `getSide()`.
    pub side: Vec<u8>,
    /// `getDisplayName()`.
    pub display_name: Vec<u16>,
    /// `getStartingBuilding()`.
    pub starting_building: Vec<u8>,
    /// `isOldFaction()`.
    pub old_faction: bool,
    /// (#98) `getMedallionNormal()`, `getMedallionSelected()`, `getMedallionHilite()` (the challenge menu's).
    pub medallion_normal: Vec<u8>,
    pub medallion_selected: Vec<u8>,
    pub medallion_hilite: Vec<u8>,
}

/// (#89) The World's data the skirmish screens read, copied by the client when it gives the shell its World
/// ([`SkirmishData::from_world`]): `TheMapCache`, `TheMultiplayerSettings`, `ThePlayerTemplateStore`,
/// `TheChallengeGenerals`, `TheGlobalData`'s FPS limit, starting cash and user data path, `TheGameText`. The shell's
/// `GameInfoHost` for `TheSkirmishGameInfo` ([`GameInfoHost`] below).
#[derive(Clone, Debug, Default)]
pub struct SkirmishData {
    pub map_cache: MapCache,
    pub user_data_path: Vec<u8>,
    pub settings: MultiplayerSettings,
    pub templates: Vec<TemplateView>,
    pub challenge: ChallengeGenerals,
    /// `TheGlobalData->m_framesPerSecondLimit`.
    pub frames_per_second_limit: i32,
    /// `TheGlobalData->m_defaultStartingCash`.
    pub default_starting_cash: Money,
    /// `NET_CRC_INTERVAL`.
    pub net_crc_interval: i32,
    pub text: TextTable,
    /// The files `TheFileSystem` has (lower case), for `GameInfo::setMap`'s contents mask.
    pub files: BTreeSet<Vec<u8>>,
    /// `GetTickCount()` (set by the host before a call that reads it).
    pub tick: u32,
}

impl SkirmishData {
    /// The copy of `w`'s data (read only: the World is not changed). `files` is what the file system holds (the
    /// client's archives; lower case).
    pub fn from_world(w: &World, files: BTreeSet<Vec<u8>>) -> SkirmishData {
        let g = w.statics.global_data.the_global_data();
        let templates = w
            .statics
            .player_template_store
            .templates()
            .iter()
            .map(|t| TemplateView {
                name: w.names.key_to_name(t.name_key()).to_vec(),
                side: t.side().to_vec(),
                display_name: t.display_name().str().to_vec(),
                starting_building: t.starting_building().to_vec(),
                old_faction: t.is_old_faction(),
                medallion_normal: t.medallion_normal().to_vec(),
                medallion_selected: t.medallion_selected().to_vec(),
                medallion_hilite: t.medallion_hilite().to_vec(),
            })
            .collect();
        SkirmishData {
            map_cache: w.statics.map_cache.clone(),
            user_data_path: g.map(|g| g.get_path_user_data().to_vec()).unwrap_or_default(),
            settings: w.statics.multiplayer_settings.clone(),
            templates,
            challenge: w.challenge_generals.clone(),
            frames_per_second_limit: g.map_or(30, |g| g.frames_per_second_limit),
            default_starting_cash: g.map(|g| g.default_starting_cash).unwrap_or_default(),
            net_crc_interval: GameInfoHost::net_crc_interval(w),
            text: w.statics.game_text.clone(),
            files,
            tick: 0,
        }
    }

    /// `TheGameText->fetch(label)`.
    pub fn fetch(&self, label: &[u8]) -> Vec<u16> {
        self.text.fetch(label).str().to_vec()
    }

    /// `ThePlayerTemplateStore->getPlayerTemplateCount()`.
    pub fn template_count(&self) -> i32 {
        self.templates.len() as i32
    }

    /// (#98) `ThePlayerTemplateStore->getTemplateNumByName(name)`: the first template whose name equals `name` without
    /// regard to case (as C strings), or -1 (`PlayerTemplateStore::template_num_by_name` over the copy).
    pub fn template_num_by_name(&self, name: &[u8]) -> i32 {
        let c = |s: &[u8]| s.iter().position(|&b| b == 0).map_or(s.len(), |i| i);
        let want = &name[..c(name)];
        self.templates.iter().position(|t| t.name[..c(&t.name)].eq_ignore_ascii_case(want)).map_or(-1, |i| i as i32)
    }

    /// `getNthPlayerTemplate(n)`.
    pub fn template(&self, n: i32) -> Option<&TemplateView> {
        usize::try_from(n).ok().and_then(|i| self.templates.get(i))
    }
}

/// `GameState`'s map path conversions read only `getPath_UserData()`: the rest of the host is never reached.
struct PathHost<'a>(&'a [u8]);

impl GameStateHost<()> for PathHost<'_> {
    fn xfer_block(&mut self, _block: (), _gs: &mut GameState<()>, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    fn load_post_process(&mut self, _block: ()) {}
    fn partition_update(&mut self) {}
    fn user_data_path(&self) -> Vec<u8> {
        self.0.to_vec()
    }
    fn map_name(&self) -> Vec<u8> {
        Vec::new()
    }
    fn local_time(&mut self) -> [u16; 8] {
        [0; 8]
    }
    fn world_dict_map_name(&self) -> Option<Vec<u8>> {
        None
    }
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)> {
        None
    }
    fn file_exists(&self, _path: &[u8]) -> bool {
        false
    }
}

impl SkirmishData {
    /// `TheGameState->realMapPathToPortableMapPath(path)`.
    pub fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        GameState::<()>::new().real_map_path_to_portable(path, &PathHost(&self.user_data_path)).unwrap_or_default()
    }
}

impl GameInfoHost for SkirmishData {
    fn game_text(&mut self, label: &str) -> Vec<u16> {
        self.fetch(label.as_bytes())
    }
    fn tick_count(&mut self) -> u32 {
        self.tick
    }
    fn default_starting_cash(&self) -> Money {
        self.default_starting_cash
    }
    fn net_crc_interval(&self) -> i32 {
        self.net_crc_interval
    }
    fn has_map_cache(&self) -> bool {
        true
    }
    fn map_cache_find(&self, lower_name: &[u8]) -> Option<MapMeta> {
        self.map_cache.maps.get(lower_name).map(|m| MapMeta { num_players: m.num_players, crc: m.crc })
    }
    fn file_exists(&mut self, path: &[u8]) -> bool {
        self.files.contains(&path.to_ascii_lowercase())
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        SkirmishData::real_map_path_to_portable(self, path)
    }
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8> {
        GameState::<()>::new().portable_map_path_to_real(path, &PathHost(&self.user_data_path)).unwrap_or_default()
    }
    fn has_multiplayer_settings(&self) -> bool {
        true
    }
    fn num_colors(&self) -> i32 {
        self.settings.get_num_colors()
    }
    fn show_random_player_template(&self) -> bool {
        self.settings.show_random_player_template()
    }
    fn show_random_color(&self) -> bool {
        self.settings.show_random_color()
    }
    fn show_random_start_pos(&self) -> bool {
        self.settings.show_random_start_pos()
    }
    fn observer_color(&self) -> Result<i32, IniError> {
        Ok(self.settings.color_at(PLAYERTEMPLATE_OBSERVER).map_or(-1, |c| c.color()))
    }
    fn player_template_count(&self) -> i32 {
        self.template_count()
    }
    fn player_template_display_name(&self, n: i32) -> Result<Vec<u16>, IniError> {
        self.template(n).map(|t| t.display_name.clone()).ok_or_else(|| IniError::undefined(Undefined::NullObject))
    }
    fn record_income(&mut self, _player_index: i32) {}
}

/// What GUIUtil.cpp reaches besides the window manager.
pub trait GuiUtilHost: MapUtilHost + MenuHost {
    /// GUIUtil.cpp's `static Bool winInitialized`.
    fn win_initialized(&mut self) -> &mut bool;
    /// The `myGame` the caller passes (`None`: `NULL`).
    fn util_game(&self) -> Option<&GameInfo>;
    /// The World's data ([`SkirmishData`]).
    fn util_data(&self) -> &SkirmishData;
}

/// `EnableSlotListUpdates(val)`.
pub fn enable_slot_list_updates<H: GuiUtilHost>(h: &mut H, val: bool) {
    *h.win_initialized() = val;
}

/// `AreSlotListUpdatesEnabled()`.
pub fn are_slot_list_updates_enabled<H: GuiUtilHost>(h: &mut H) -> bool {
    *h.win_initialized()
}

/// `GadgetComboBoxReset(combo)` (nothing for `NULL`).
pub fn combo_reset<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, c: Option<WinId>) {
    if let Some(c) = c {
        cb::reset(wm, h, c);
    }
}

/// `GadgetComboBoxAddEntry(combo, text, color)` (-1 for `NULL`).
pub fn combo_add_entry<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, c: Option<WinId>, text: &[u16], color: i32) -> i32 {
    c.map_or(-1, |c| cb::add_entry(wm, h, c, text, color))
}

/// `GadgetComboBoxSetItemData(combo, index, data)` (nothing for `NULL`).
pub fn combo_set_item_data<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, c: Option<WinId>, index: i32, data: u32) {
    if let Some(c) = c {
        cb::set_item_data(wm, h, c, index, data);
    }
}

/// `GadgetComboBoxGetItemData(combo, index)` (`NULL`, 0, for `NULL`).
pub fn combo_get_item_data<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, c: Option<WinId>, index: i32) -> u32 {
    c.map_or(0, |c| cb::get_item_data(wm, h, c, index))
}

/// `GadgetComboBoxSetSelectedPos(combo, index, dontHide)` (nothing for `NULL`).
pub fn combo_set_selected_pos<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, c: Option<WinId>, index: i32, dont_hide: bool) {
    if let Some(c) = c {
        cb::set_selected_pos(wm, h, c, index, dont_hide);
    }
}

/// `GadgetComboBoxGetSelectedPos(combo, &index)`: `initial` (the caller's `Int`) when `NULL` or unanswered.
pub fn combo_get_selected_pos<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, c: Option<WinId>, initial: i32) -> i32 {
    c.map_or(initial, |c| cb::get_selected_pos(wm, h, c, initial))
}

/// `GadgetComboBoxGetText(combo)` (empty for `NULL`).
pub fn combo_get_text<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, c: Option<WinId>) -> Vec<u16> {
    c.map_or_else(Vec::new, |c| cb::get_text(wm, h, c))
}

/// `GadgetComboBoxGetLength(combo)`: a `NULL` combo box's user data is read (undefined; 0 here).
pub fn combo_get_length(wm: &mut GameWindowManager, c: Option<WinId>) -> i32 {
    match c {
        Some(c) => cb::get_length(wm, c),
        None => {
            wm.set_undefined("GadgetComboBoxGetLength(NULL): the user data of NULL");
            0
        }
    }
}

/// The game's slot `i` as it is now (`myGame->getSlot(i)`), copied.
fn slot_now<H: GuiUtilHost>(h: &H, i: i32) -> Option<GameSlot> {
    h.util_game().and_then(|g| g.slot(i)).cloned()
}

/// `EnableAcceptControls(Enabled, myGame, comboPlayer, comboColor, comboPlayerTemplate, comboTeam, buttonAccept,
/// buttonStart, buttonMapStartPosition, slotNum)` (`slotNum` -1 by default: the local slot).
#[allow(clippy::too_many_arguments)]
pub fn enable_accept_controls<H: GuiUtilHost>(
    wm: &mut GameWindowManager,
    h: &mut H,
    enabled: bool,
    combo_color: &[Option<WinId>; MAX_SLOTS],
    combo_player_template: &[Option<WinId>; MAX_SLOTS],
    combo_team: &[Option<WinId>; MAX_SLOTS],
    button_start: Option<WinId>,
    button_map_start_position: &[Option<WinId>; MAX_SLOTS],
    slot_num: i32,
) {
    let Some(g) = h.util_game().cloned() else {
        wm.set_undefined("EnableAcceptControls: myGame is NULL");
        return;
    };
    let mut slot_num = slot_num;
    if slot_num == -1 || slot_num >= MAX_SLOTS as i32 {
        slot_num = g.local_slot_num();
    }
    let Some(s) = g.slot(slot_num) else {
        wm.set_undefined("EnableAcceptControls: getConstSlot(slotNum) is NULL (no local slot)");
        return;
    };
    let i = slot_num as usize;
    let is_observer = s.player_template == PLAYERTEMPLATE_OBSERVER;
    if !g.am_i_host() {
        if let Some(b) = button_start {
            wm.win_enable(b, enabled);
        }
    }
    if let Some(c) = combo_color[i] {
        if is_observer {
            cb::hide_list(wm, h, c);
        }
        wm.win_enable(c, enabled && !is_observer);
    }
    if let Some(c) = combo_player_template[i] {
        wm.win_enable(c, enabled);
    }
    if let Some(c) = combo_team[i] {
        if is_observer {
            cb::hide_list(wm, h, c);
        }
        wm.win_enable(c, enabled && !is_observer);
    }
    // (the game is read again: the hide-list messages above reach no handler of the game's, but the reads are
    // upstream's own, after them)
    let g = h.util_game().cloned().unwrap_or(g);
    let mut can_choose_start_spot = !is_observer;
    let mut k = 0;
    while k < MAX_SLOTS && !can_choose_start_spot && g.am_i_host() {
        if g.slots[k].is_ai() {
            can_choose_start_spot = true;
        }
        k += 1;
    }
    let local = g.local_slot_num();
    if slot_num == local {
        let has_map = g.slot(local).is_some_and(|s| s.has_map);
        for b in button_map_start_position.iter().flatten() {
            wm.win_enable(*b, if has_map { enabled && can_choose_start_spot } else { false });
        }
    }
}

/// `ShowUnderlyingGUIElements(show, layoutFilename, parentName, gadgetsToHide, perPlayerGadgetsToHide)`.
pub fn show_underlying_gui_elements<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, show: bool, layout_filename: &str, parent_name: &str, gadgets_to_hide: &[&str], per_player_gadgets_to_hide: &[&str]) {
    let parent_id = h.name_to_key(format!("{layout_filename}:{parent_name}").as_bytes()) as i32;
    let Some(parent) = wm.win_get_window_from_id(None, parent_id) else {
        // (`DEBUG_CRASH`: nothing in the release)
        return;
    };
    for g in gadgets_to_hide {
        let id = h.name_to_key(format!("{layout_filename}:{g}").as_bytes()) as i32;
        if let Some(w) = wm.win_get_window_from_id(Some(parent), id) {
            wm.win_hide(w, !show);
        }
    }
    for g in per_player_gadgets_to_hide {
        for player in 0..MAX_SLOTS {
            let id = h.name_to_key(format!("{layout_filename}:{g}{player}").as_bytes()) as i32;
            if let Some(w) = wm.win_get_window_from_id(Some(parent), id) {
                wm.win_hide(w, !show);
            }
        }
    }
}

/// `TheMultiplayerSettings->getColor(which)->getColor()` (`None`: `NULL`).
fn settings_color<H: GuiUtilHost>(h: &H, which: i32) -> Option<i32> {
    h.util_data().settings.color_at(which).map(|d| d.color())
}

/// `PopulateColorComboBox(comboBox, comboArray, myGame, isObserver)`.
pub fn populate_color_combo_box<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, combo_box: usize, combo_array: &[Option<WinId>; MAX_SLOTS], is_observer: bool) {
    let num_colors = h.util_data().settings.get_num_colors();
    let mut available = vec![true; num_colors.max(0) as usize];
    let Some(g) = h.util_game().cloned() else {
        wm.set_undefined("PopulateColorComboBox: myGame is NULL");
        return;
    };
    for (i, s) in g.slots.iter().enumerate() {
        if i != combo_box && s.color >= 0 && s.color < num_colors {
            available[s.color as usize] = false;
        }
    }
    let c = combo_array[combo_box];
    let was_observer = combo_get_length(wm, c) == 1;
    combo_reset(wm, h, c);
    let random = settings_color(h, PLAYERTEMPLATE_RANDOM).unwrap_or(0);
    let label: &[u8] = if is_observer { b"GUI:None" } else { b"GUI:???" };
    let text = h.fetch_text(label);
    let new_index = combo_add_entry(wm, h, c, &text, random);
    combo_set_item_data(wm, h, c, new_index, -1i32 as u32);
    if is_observer {
        combo_set_selected_pos(wm, h, c, 0, false);
        return;
    }
    for k in 0..num_colors {
        let Some(def) = h.util_data().settings.color_at(k).cloned() else { continue };
        if !available[k as usize] {
            continue;
        }
        let color_name = h.fetch_text(def.tooltip_name());
        let new_index = combo_add_entry(wm, h, c, &color_name, def.color());
        combo_set_item_data(wm, h, c, new_index, k as u32);
    }
    if was_observer {
        combo_set_selected_pos(wm, h, c, 0, false);
    }
}

/// `PopulatePlayerTemplateComboBox(comboBox, comboArray, myGame, allowObservers)`.
pub fn populate_player_template_combo_box<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, combo_box: usize, combo_array: &[Option<WinId>; MAX_SLOTS], allow_observers: bool) {
    let c = combo_array[combo_box];
    let num_player_templates = h.util_data().template_count();
    combo_reset(wm, h, c);
    let color = settings_color(h, PLAYERTEMPLATE_RANDOM).unwrap_or(0);
    let text = h.fetch_text(b"GUI:Random");
    let new_index = combo_add_entry(wm, h, c, &text, color);
    combo_set_item_data(wm, h, c, new_index, PLAYERTEMPLATE_RANDOM as u32);
    let old_factions_only = h.util_game().is_some_and(|g| g.old_factions_only);
    let mut seen_sides: BTreeSet<Vec<u8>> = BTreeSet::new();
    for k in 0..num_player_templates {
        let Some(fac) = h.util_data().template(k).cloned() else { continue };
        if fac.starting_building.is_empty() {
            continue;
        }
        if old_factions_only && !fac.old_faction {
            continue;
        }
        // "Prevent players from selecting the disabled Generals for use."
        let disallow_locked_generals = true;
        let starts_locked = h.util_data().challenge.get_general_by_template_name(&fac.name).is_some_and(|g| !g.is_starting_enabled());
        if disallow_locked_generals && starts_locked {
            continue;
        }
        let mut side = b"SIDE:".to_vec();
        side.extend_from_slice(&fac.side);
        if !seen_sides.insert(side.clone()) {
            continue;
        }
        let text = h.fetch_text(&side);
        let new_index = combo_add_entry(wm, h, c, &text, color);
        combo_set_item_data(wm, h, c, new_index, k as u32);
    }
    if allow_observers {
        let observer = settings_color(h, PLAYERTEMPLATE_OBSERVER).unwrap_or(0);
        let text = h.fetch_text(b"GUI:Observer");
        let new_index = combo_add_entry(wm, h, c, &text, observer);
        combo_set_item_data(wm, h, c, new_index, PLAYERTEMPLATE_OBSERVER as u32);
    }
    combo_set_selected_pos(wm, h, c, 0, false);
}

/// `PopulateTeamComboBox(comboBox, comboArray, myGame, isObserver)`.
pub fn populate_team_combo_box<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, combo_box: usize, combo_array: &[Option<WinId>; MAX_SLOTS], is_observer: bool) {
    let num_teams = (MAX_SLOTS / 2) as i32;
    let c = combo_array[combo_box];
    combo_reset(wm, h, c);
    let color = settings_color(h, PLAYERTEMPLATE_RANDOM).unwrap_or(0);
    let text = h.fetch_text(b"Team:0");
    let new_index = combo_add_entry(wm, h, c, &text, color);
    combo_set_item_data(wm, h, c, new_index, -1i32 as u32);
    if is_observer {
        combo_set_selected_pos(wm, h, c, 0, false);
        return;
    }
    for k in 0..num_teams {
        let team_name = h.fetch_text(format!("Team:{}", k + 1).as_bytes());
        let new_index = combo_add_entry(wm, h, c, &team_name, color);
        combo_set_item_data(wm, h, c, new_index, k as u32);
    }
    combo_set_selected_pos(wm, h, c, 0, false);
}

/// `formatMoneyForStartingCashComboBox(moneyAmount)`: `GUI:StartingMoneyFormat` with the amount.
fn format_money_for_starting_cash_combo_box<H: GuiUtilHost>(h: &H, amount: u32) -> Vec<u16> {
    format_wide(&h.fetch_text(b"GUI:StartingMoneyFormat"), &[FormatArg::Int(i64::from(amount))])
}

/// `PopulateStartingCashComboBox(comboBox, myGame)`.
pub fn populate_starting_cash_combo_box<H: GuiUtilHost>(wm: &mut GameWindowManager, h: &mut H, combo_box: Option<WinId>) {
    let Some(c) = combo_box else {
        // `comboBox->winGetEnabled()` on NULL (after a reset that does nothing)
        wm.set_undefined("PopulateStartingCashComboBox: the combo box is NULL (winGetEnabled)");
        return;
    };
    cb::reset(wm, h, c);
    let list: Vec<Money> = h.util_data().settings.get_starting_money_list().to_vec();
    let game_cash = h.util_game().map(|g| g.starting_cash);
    let Some(game_cash) = game_cash else {
        wm.set_undefined("PopulateStartingCashComboBox: myGame is NULL");
        return;
    };
    let mut current_selection_index = -1;
    let text_color = |wm: &GameWindowManager| {
        let inst = &wm.tree.windows[c].inst;
        if wm.tree.windows[c].status & win_status::ENABLED != 0 {
            inst.enabled_text.color
        } else {
            inst.disabled_text.color
        }
    };
    for m in &list {
        let text = format_money_for_starting_cash_combo_box(h, m.count_money());
        let color = text_color(wm);
        let new_index = cb::add_entry(wm, h, c, &text, color);
        cb::set_item_data(wm, h, c, new_index, m.count_money());
        if game_cash.amount_equal(m) {
            current_selection_index = new_index;
        }
    }
    if current_selection_index == -1 {
        // (`DEBUG_CRASH`: nothing in the release)
        let text = format_money_for_starting_cash_combo_box(h, game_cash.count_money());
        let color = text_color(wm);
        // (the new row's index is selected after the item data, which is not reached here)
        cb::add_entry(wm, h, c, &text, color);
        // `(void *)it->countMoney()` with `it` the list's end
        wm.set_undefined("PopulateStartingCashComboBox: the item data read from the list's end iterator");
        return;
    }
    cb::set_selected_pos(wm, h, c, current_selection_index, false);
}

/// `UpdateSlotList(myGame, comboPlayer, comboColor, comboPlayerTemplate, comboTeam, buttonAccept, buttonStart,
/// buttonMapStartPosition)` (`buttonAccept` `None`: `NULL`).
#[allow(clippy::too_many_arguments)]
pub fn update_slot_list<H: GuiUtilHost>(
    wm: &mut GameWindowManager,
    h: &mut H,
    combo_player: &[Option<WinId>; MAX_SLOTS],
    combo_color: &[Option<WinId>; MAX_SLOTS],
    combo_player_template: &[Option<WinId>; MAX_SLOTS],
    combo_team: &[Option<WinId>; MAX_SLOTS],
    button_accept: Option<&[Option<WinId>; MAX_SLOTS]>,
    button_start: Option<WinId>,
    button_map_start_position: &[Option<WinId>; MAX_SLOTS],
) {
    if !are_slot_list_updates_enabled(h) {
        return;
    }
    let Some(map) = h.util_game().map(|g| g.map_name.clone()) else {
        wm.set_undefined("UpdateSlotList: myGame->getMap() on NULL");
        return;
    };
    let map_data = h.map_cache().and_then(|c| c.find_map(&map)).map(|m| m.is_official);
    let will_transfer = match map_data {
        Some(official) => !official,
        None => would_map_transfer(h, &map),
    };
    let accept = |wm: &mut GameWindowManager, h: &mut H, en: bool, i: i32| enable_accept_controls(wm, h, en, combo_color, combo_player_template, combo_team, button_start, button_map_start_position, i);
    for i in 0..MAX_SLOTS {
        let ii = i as i32;
        let Some(g) = h.util_game().cloned() else { return };
        let slot = g.slot(ii).cloned();
        if g.am_i_host() && slot.as_ref().is_some_and(|s| s.is_ai()) {
            accept(wm, h, true, ii);
        } else if slot.is_some() && g.local_slot_num() == ii {
            let s = slot.as_ref().expect("slot");
            if s.is_accepted && !g.am_i_host() {
                accept(wm, h, false, -1);
            } else if s.has_map {
                accept(wm, h, true, -1);
            } else {
                accept(wm, h, will_transfer, -1);
            }
        } else if g.am_i_host() {
            accept(wm, h, false, ii);
        }
        let slot = slot_now(h, ii);
        if slot.as_ref().is_some_and(|s| s.is_human()) {
            let s = slot.as_ref().expect("slot");
            let new_name = s.name.clone();
            let old_name = combo_get_text(wm, h, combo_player[i]);
            if let Some(c) = combo_player[i] {
                if new_name != old_name {
                    cb::set_text(wm, h, c, &new_name);
                }
            }
            if let Some(b) = button_accept.and_then(|a| a[i]).filter(|_| i != 0) {
                wm.win_hide(b, false);
                let accepted = slot_now(h, ii).is_some_and(|s| s.is_accepted);
                let image = wm.tree.windows[b].status & win_status::IMAGE != 0;
                if accepted {
                    if image {
                        wm.win_enable(b, true);
                    } else {
                        wm.tree.windows[b].inst.enabled_draw_data[0].color = accept_true_color();
                    }
                } else if image {
                    wm.win_enable(b, false);
                } else {
                    wm.tree.windows[b].inst.enabled_draw_data[0].color = accept_false_color();
                }
            }
        } else {
            let Some(s) = slot.as_ref() else {
                wm.set_undefined("UpdateSlotList: slot->getState() on a NULL slot");
                return;
            };
            combo_set_selected_pos(wm, h, combo_player[i], s.state, true);
            if let Some(b) = button_accept.and_then(|a| a[i]) {
                wm.win_hide(b, true);
            }
        }
        if !h.util_game().is_some_and(|g| g.am_i_host()) {
            if let Some(c) = combo_player[i] {
                wm.win_enable(c, false);
            }
        }
        if let Some(c) = combo_color[i] {
            if wm.tree.windows[c].status & win_status::ENABLED != 0 {
                let observer = slot_now(h, ii).is_some_and(|s| s.player_template == PLAYERTEMPLATE_OBSERVER);
                populate_color_combo_box(wm, h, i, combo_color, observer);
            }
        }
        for (array, read) in [(combo_color, 0), (combo_team, 1), (combo_player_template, 2)] {
            if array[i].is_none() {
                continue;
            }
            let max = combo_get_length(wm, array[i]);
            for idx in 0..max {
                let v = combo_get_item_data(wm, h, array[i], idx) as i32;
                let Some(s) = slot_now(h, ii) else { break };
                let want = match read {
                    0 => s.color,
                    1 => s.team_number,
                    _ => s.player_template,
                };
                if v == want {
                    combo_set_selected_pos(wm, h, array[i], idx, true);
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #89: the accept colours are LANAPICallbacks.cpp's green and red (opaque).
    #[test]
    fn the_accept_colours_are_green_and_red() {
        assert_eq!(accept_true_color(), game_make_color(0, 255, 0, 255));
        assert_eq!(accept_false_color(), game_make_color(255, 0, 0, 255));
        assert_ne!(accept_true_color(), accept_false_color());
    }
}
