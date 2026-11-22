//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_game_info_window.rs);
//! not yet wired into the game.
//!
//! Corrected 2026-11-21 (#215, group ui4): the list box entries are added with `overwrite` TRUE (GadgetListBox.h's default; the draft had FALSE, which refuses the
//! image into a full list), and the leaf name of a map not in the cache is widened with its sign (`(WideChar)` of a signed `char`).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/GameInfoWindow.cpp
//!     (the file's statics, `CreateLANGameInfoWindow`, `DestroyGameInfoWindow`, `RefreshGameInfoWindow`,
//!      `HideGameInfoWindow`, `GameInfoWindowInit`, `GameInfoWindowSystem`)

use crate::client::gadget_list_box;
use crate::client::shell::ShellHost;
use crate::client::window_manager::{gwm, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// `MAX_SLOTS` (GameInfo.h).
pub const MAX_SLOTS: usize = 8;
/// `PLAYERTEMPLATE_OBSERVER` (PlayerTemplate.h).
pub const PLAYERTEMPLATE_OBSERVER: i32 = -2;

/// The file's statics (`NAMEKEY_INVALID` is 0).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameInfoWindow {
    pub parent: Option<WinId>,
    pub static_text_game_name: Option<WinId>,
    pub static_text_map_name: Option<WinId>,
    pub list_box_players: Option<WinId>,
    pub win_crates: Option<WinId>,
    pub win_super_weapons: Option<WinId>,
    pub win_free_for_all: Option<WinId>,
    pub parent_id: i32,
    pub static_text_game_name_id: i32,
    pub static_text_map_name_id: i32,
    pub list_box_players_id: i32,
    pub win_crates_id: i32,
    pub win_super_weapons_id: i32,
    pub win_free_for_all_id: i32,
    pub game_info_window_layout: Option<usize>,
}

/// `SlotState` values `RefreshGameInfoWindow` tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotState {
    EasyAi,
    MedAi,
    BrutalAi,
    Other,
}

/// One occupied slot as `RefreshGameInfoWindow` reads it.
#[derive(Clone, Debug)]
pub struct InfoSlot {
    pub is_ai: bool,
    pub is_human: bool,
    pub state: SlotState,
    /// `slot->getColor()`.
    pub color: i32,
    /// `slot->getName()`.
    pub name: Vec<u16>,
    /// `slot->getPlayerTemplate()`.
    pub player_template: i32,
}

/// What the game info window reaches outside the shell.
pub trait GameInfoWindowHost: ShellHost {
    fn game_info_window(&mut self) -> &mut GameInfoWindow;
    /// `layout->runInit()`.
    fn layout_run_init(&mut self, wm: &mut GameWindowManager, layout: usize);
    /// `((LANGameInfo *)gameInfo)->getPlayerName(0)`.
    fn game_info_player_name_0(&self) -> Vec<u16>;
    /// `gameInfo->getMap()` (an `AsciiString`).
    fn game_info_map(&self) -> Vec<u8>;
    /// `TheMapCache->find(lowercased map)`: the map's `m_displayName`.
    fn map_cache_display_name(&self, lower_map: &[u8]) -> Option<Vec<u16>>;
    /// `gameInfo->getSlot(i)`; `None` for no slot or one not `isOccupied()`.
    fn game_info_occupied_slot(&self, i: usize) -> Option<InfoSlot>;
    /// `TheMultiplayerSettings->getNumColors()` and `getColor(c)->getColor()`.
    fn multiplayer_num_colors(&self) -> i32;
    fn multiplayer_color(&self, c: i32) -> i32;
    /// `ThePlayerTemplateStore->getPlayerTemplateCount()`.
    fn player_template_count(&self) -> i32;
    /// `getNthPlayerTemplate(n)->getSideIconImage()`'s mapped image name.
    fn player_template_side_icon(&self, n: i32) -> Option<Vec<u8>>;
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
}

fn static_text<H: GameInfoWindowHost>(wm: &mut GameWindowManager, h: &mut H, w: Option<WinId>, text: &[u16]) {
    crate::client::gadget::static_text_set_text(wm, h, w, text);
}

/// `GameMakeColor(255,255,255,255)`.
const WHITE: i32 = 0xFFFF_FFFFu32 as i32;

/// `CreateLANGameInfoWindow(sizeAndPosWin)`.
pub fn create_lan_game_info_window<H: GameInfoWindowHost>(wm: &mut GameWindowManager, h: &mut H, size_and_pos_win: Option<WinId>) {
    if h.game_info_window().game_info_window_layout.is_none() {
        let layout = h.create_layout(wm, b"Menus/GameInfoWindow.wnd");
        h.game_info_window().game_info_window_layout = layout;
    }
    let Some(layout) = h.game_info_window().game_info_window_layout else { return };

    if !plant(21493) {
        h.layout_run_init(wm, layout);
    }
    if !plant(21494) {
        wm.layout_bring_forward(layout);
    }
    wm.layout_hide(layout, !plant(21495));

    let parent = h.game_info_window().parent;
    let (Some(parent), Some(size_win)) = (parent, size_and_pos_win) else { return };
    let (x, y) = if plant(21496) { let r = wm.win(size_win).region; (r.0, r.1) } else { wm.tree.screen_position(size_win) };
    wm.win_set_position(parent, x, y);

    let (width, height) = wm.win(size_win).size;
    if plant(21497) {
        wm.win_set_size(h, parent, height, width);
    } else {
        wm.win_set_size(h, parent, width, height);
    }
}

/// `DestroyGameInfoWindow()`.
pub fn destroy_game_info_window<H: GameInfoWindowHost>(wm: &mut GameWindowManager, h: &mut H) {
    let l = if plant(21508) { h.game_info_window().game_info_window_layout } else { h.game_info_window().game_info_window_layout.take() };
    if let Some(layout) = l {
        wm.layout_destroy_windows(layout);
        // `deleteInstance()`: the layout record is released by the host
    }
}

/// `RefreshGameInfoWindow(gameInfo, gameName)`: `has_game_info` is `gameInfo != NULL`. The `randomIcon` and
/// `observerIcon` are `GameinfoRANDOM` and `GameinfoOBSRVR`.
pub fn refresh_game_info_window<H: GameInfoWindowHost>(wm: &mut GameWindowManager, h: &mut H, has_game_info: bool, _game_name: &[u16]) {
    if h.game_info_window().game_info_window_layout.is_none() || !has_game_info {
        return;
    }
    let g = h.game_info_window().clone();
    let Some(parent) = g.parent else { return };

    wm.win_hide(parent, false);
    wm.win_bring_to_top(parent);

    // Set the game name
    let name0 = h.game_info_player_name_0();
    static_text(wm, h, g.static_text_game_name, &name0);
    // set the map name
    let mut ascii_map = h.game_info_map();
    if !plant(21498) {
        ascii_map.make_ascii_lowercase();
    }
    let map: Vec<u16> = match h.map_cache_display_name(&ascii_map) {
        Some(display) => display,
        None => {
            // can happen if the map will have to be transferred... so use the leaf name (srj)
            let original = h.game_info_map();
            let no_path = match (if plant(21499) { original.iter().position(|&c| c == b'\\') } else { original.iter().rposition(|&c| c == b'\\') }) {
                Some(i) => &original[i + 1..],
                None => &original[..],
            };
            // `UnicodeString::translate(const AsciiString &)`: `(WideChar)getCharAt(i)`, a signed `char` widened with its sign (0xE9 becomes 0xFFE9; the
            // differential test found the draft's `c as u16` wrong, #215)
            no_path.iter().map(|&c| c as i8 as i16 as u16).collect()
        }
    };
    static_text(wm, h, g.static_text_map_name, &map);

    // fill in the player list
    let Some(list) = g.list_box_players else { return };
    if !plant(21504) {
        gadget_list_box::reset(wm, h, list);
    }

    let num_colors = h.multiplayer_num_colors();
    for i in 0..MAX_SLOTS {
        let mut player_color = WHITE;
        // (upstream's `addedRow` is uninitialised for an AI slot in no easy/medium/brutal state: -1 here)
        let mut added_row: i32 = -1;
        let Some(slot) = h.game_info_occupied_slot(i) else { continue };
        let color = slot.color;
        if color > if plant(21500) { 0 } else { -1 } && color < num_colors {
            player_color = h.multiplayer_color(color);
        }
        if slot.is_ai {
            let label: Option<&[u8]> = match slot.state {
                SlotState::EasyAi => Some(b"GUI:EasyAI"),
                SlotState::MedAi => Some(if plant(21503) { b"GUI:EasyAI" } else { b"GUI:MediumAI" }),
                SlotState::BrutalAi => Some(b"GUI:HardAI"),
                SlotState::Other => None,
            };
            if let Some(label) = label {
                let text = h.fetch_text(label);
                added_row = gadget_list_box::add_entry_text(wm, h, list, &text, player_color, -1, 1, true); // (`overwrite` defaults to TRUE in GadgetListBox.h; the draft had FALSE)
            }
        } else if slot.is_human {
            added_row = gadget_list_box::add_entry_text(wm, h, list, &slot.name, player_color, -1, 1, true); // (`overwrite` defaults to TRUE in GadgetListBox.h; the draft had FALSE)
        }
        let player_template = slot.player_template;
        if player_template == if plant(21501) { -1 } else { PLAYERTEMPLATE_OBSERVER } {
            gadget_list_box::add_entry_image(wm, h, list, Some(b"GameinfoOBSRVR".to_vec()), added_row, 0, 22, 25, true, WHITE);
        } else if player_template < 0 || player_template > h.player_template_count() - i32::from(!plant(21502)) {
            // @todo (upstream): when we get art that shows player's side, draw the art instead of putting in text
            gadget_list_box::add_entry_image(wm, h, list, Some(b"GameinfoRANDOM".to_vec()), added_row, 0, 22, 25, true, WHITE);
        } else {
            let icon = h.player_template_side_icon(player_template);
            gadget_list_box::add_entry_image(wm, h, list, icon, added_row, 0, 22, 25, true, WHITE);
        }
    }
}

/// `HideGameInfoWindow(hide)`.
pub fn hide_game_info_window<H: GameInfoWindowHost>(wm: &mut GameWindowManager, h: &mut H, hide: bool) {
    if let Some(parent) = h.game_info_window().parent {
        wm.win_hide(parent, hide != plant(21505));
    }
}

/// `GameInfoWindowInit(layout, userData)`.
pub fn game_info_window_init<H: GameInfoWindowHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let key = |h: &mut H, n: &[u8]| ShellHost::name_to_key(h, n) as i32;
    let parent_id = key(h, b"GameInfoWindow.wnd:ParentGameInfo");
    let static_text_game_name_id = key(h, b"GameInfoWindow.wnd:StaticTextGameName");
    let static_text_map_name_id = key(h, b"GameInfoWindow.wnd:StaticTextMapName");
    let list_box_players_id = key(h, b"GameInfoWindow.wnd:ListBoxPlayers");
    let win_crates_id = key(h, b"GameInfoWindow.wnd:WinCrates");
    let win_super_weapons_id = key(h, b"GameInfoWindow.wnd:WinSuperWeapons");
    let win_free_for_all_id = key(h, b"GameInfoWindow.wnd:WinFreeForAll");

    let parent = wm.win_get_window_from_id(None, parent_id);
    let game_name = wm.win_get_window_from_id(parent, static_text_game_name_id);
    let map_name = wm.win_get_window_from_id(parent, static_text_map_name_id);
    let list = wm.win_get_window_from_id(parent, list_box_players_id);
    let crates = wm.win_get_window_from_id(parent, win_crates_id);
    let super_weapons = wm.win_get_window_from_id(parent, win_super_weapons_id);
    let free_for_all = wm.win_get_window_from_id(parent, win_free_for_all_id);
    *h.game_info_window() = GameInfoWindow {
        parent,
        static_text_game_name: game_name,
        static_text_map_name: map_name,
        list_box_players: list,
        win_crates: crates,
        win_super_weapons: super_weapons,
        win_free_for_all: free_for_all,
        parent_id,
        static_text_game_name_id,
        static_text_map_name_id,
        list_box_players_id,
        win_crates_id,
        win_super_weapons_id,
        win_free_for_all_id,
        game_info_window_layout: h.game_info_window().game_info_window_layout,
    };

    static_text(wm, h, game_name, &[]);
    if !plant(21506) {
        static_text(wm, h, map_name, &[]);
    }
    if let Some(l) = list {
        gadget_list_box::reset(wm, h, l);
    }
}

/// `GameInfoWindowSystem(window, msg, mData1, mData2)`.
pub fn game_info_window_system(wm: &mut GameWindowManager, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {}
        gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return if plant(21507) { MsgHandled::Ignored } else { MsgHandled::Handled };
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
