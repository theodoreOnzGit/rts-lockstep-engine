//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_disconnect.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/DisconnectMenu/DisconnectMenu.cpp
//!     (`DisconnectMenu`: init, attachDisconnectManager, showScreen, hideScreen, setPlayerName, setPlayerTimeoutTime,
//!      showPlayerControls, hidePlayerControls, showPacketRouterTimeout, hidePacketRouterTimeout,
//!      setPacketRouterTimeoutTime, sendChat, showChat, quitGame, removePlayer, voteForPlayer, updateVotes)

use crate::client::gadget_list_box;
use crate::client::window_manager::{GameWindowManager, GuiHost};
use crate::logic::modules::plant;

/// `DisconnectMenu::m_playerNameTextControlNames` (the NULL terminator is the slice's end).
pub const PLAYER_NAME_TEXT_CONTROL_NAMES: [&str; 7] = [
    "DisconnectScreen.wnd:StaticPlayer1Name",
    "DisconnectScreen.wnd:StaticPlayer2Name",
    "DisconnectScreen.wnd:StaticPlayer3Name",
    "DisconnectScreen.wnd:StaticPlayer4Name",
    "DisconnectScreen.wnd:StaticPlayer5Name",
    "DisconnectScreen.wnd:StaticPlayer6Name",
    "DisconnectScreen.wnd:StaticPlayer7Name",
];
/// `DisconnectMenu::m_playerTimeoutTextControlNames`.
pub const PLAYER_TIMEOUT_TEXT_CONTROL_NAMES: [&str; 7] = [
    "DisconnectScreen.wnd:StaticPlayer1Timeout",
    "DisconnectScreen.wnd:StaticPlayer2Timeout",
    "DisconnectScreen.wnd:StaticPlayer3Timeout",
    "DisconnectScreen.wnd:StaticPlayer4Timeout",
    "DisconnectScreen.wnd:StaticPlayer5Timeout",
    "DisconnectScreen.wnd:StaticPlayer6Timeout",
    "DisconnectScreen.wnd:StaticPlayer7Timeout",
];
/// `DisconnectMenu::m_playerVoteButtonControlNames`.
pub const PLAYER_VOTE_BUTTON_CONTROL_NAMES: [&str; 7] = [
    "DisconnectScreen.wnd:ButtonKickPlayer1",
    "DisconnectScreen.wnd:ButtonKickPlayer2",
    "DisconnectScreen.wnd:ButtonKickPlayer3",
    "DisconnectScreen.wnd:ButtonKickPlayer4",
    "DisconnectScreen.wnd:ButtonKickPlayer5",
    "DisconnectScreen.wnd:ButtonKickPlayer6",
    "DisconnectScreen.wnd:ButtonKickPlayer7",
];
/// `DisconnectMenu::m_playerVoteCountControlNames`.
pub const PLAYER_VOTE_COUNT_CONTROL_NAMES: [&str; 7] = [
    "DisconnectScreen.wnd:StaticPlayer1Votes",
    "DisconnectScreen.wnd:StaticPlayer2Votes",
    "DisconnectScreen.wnd:StaticPlayer3Votes",
    "DisconnectScreen.wnd:StaticPlayer4Votes",
    "DisconnectScreen.wnd:StaticPlayer5Votes",
    "DisconnectScreen.wnd:StaticPlayer6Votes",
    "DisconnectScreen.wnd:StaticPlayer7Votes",
];
pub const PACKET_ROUTER_TIMEOUT_CONTROL_NAME: &str = "DisconnectScreen.wnd:StaticPacketRouterTimeout";
pub const PACKET_ROUTER_TIMEOUT_LABEL_CONTROL_NAME: &str = "DisconnectScreen.wnd:StaticPacketRouterTimeoutLabel";
pub const TEXT_DISPLAY_CONTROL_NAME: &str = "DisconnectScreen.wnd:ListboxTextDisplay";

/// `chatNormalColor = GameMakeColor(255,0,0,255)` (ARGB, as `GameMakeColor` packs it).
pub const CHAT_NORMAL_COLOR: i32 = 0xFFFF_0000u32 as i32;

/// `enum DisconnectMenuStateType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisconnectMenuStateType {
    ScreenOn,
    ScreenOff,
}

/// `class DisconnectMenu`'s members. `m_disconnectManager` is the network's; the draft keeps whether one is attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisconnectMenu {
    pub disconnect_manager_attached: bool,
    pub menu_state: DisconnectMenuStateType,
}

impl DisconnectMenu {
    /// `DisconnectMenu::DisconnectMenu()`: `m_disconnectManager = NULL` (the state is set by `init`).
    pub fn new() -> Self {
        DisconnectMenu { disconnect_manager_attached: false, menu_state: DisconnectMenuStateType::ScreenOff }
    }
}

impl Default for DisconnectMenu {
    fn default() -> Self {
        Self::new()
    }
}

/// What the disconnect menu reaches outside the window manager.
pub trait DisconnectMenuHost: GuiHost {
    fn disconnect_menu(&mut self) -> &mut DisconnectMenu;
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `HideDisconnectWindow()`, `ShowDisconnectWindow()` (DisconnectWindow.cpp).
    fn hide_disconnect_window(&mut self, wm: &mut GameWindowManager);
    fn show_disconnect_window(&mut self, wm: &mut GameWindowManager);
    /// `HideDiplomacy()`, `HideInGameChat()`, `HideQuitMenu()`.
    fn hide_diplomacy(&mut self, wm: &mut GameWindowManager);
    fn hide_in_game_chat(&mut self, wm: &mut GameWindowManager);
    fn hide_quit_menu(&mut self, wm: &mut GameWindowManager);
    /// `TheNetwork->sendDisconnectChat(text)`, `quitGame()`, `voteForPlayerDisconnect(slot)`.
    fn network_send_disconnect_chat(&mut self, text: &[u16]);
    fn network_quit_game(&mut self);
    fn network_vote_for_player_disconnect(&mut self, slot: i32);
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `UnicodeString::format(fmt, name)` with the one `%ls`.
    fn format_with_name(&self, fmt: &[u16], name: &[u16]) -> Vec<u16>;
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn win_by_name<H: DisconnectMenuHost>(wm: &GameWindowManager, h: &mut H, name: &str) -> Option<usize> {
    let id = h.name_to_key(name.as_bytes());
    wm.win_get_window_from_id(None, id)
}

fn static_text<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H, w: usize, text: &[u16]) {
    crate::client::gadget::static_text_set_text(wm, h, Some(w), text);
}

/// `DisconnectMenu::init()`.
pub fn init<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.disconnect_menu().disconnect_manager_attached = false;
    h.hide_disconnect_window(wm);
    h.disconnect_menu().menu_state = if plant(21468) { DisconnectMenuStateType::ScreenOn } else { DisconnectMenuStateType::ScreenOff };
}

/// `DisconnectMenu::attachDisconnectManager(disconnectManager)`.
pub fn attach_disconnect_manager<H: DisconnectMenuHost>(h: &mut H, attached: bool) {
    h.disconnect_menu().disconnect_manager_attached = attached;
}

/// `DisconnectMenu::showScreen()`.
pub fn show_screen<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.hide_diplomacy(wm);
    h.hide_in_game_chat(wm);
    if !plant(21467) {
        h.hide_quit_menu(wm);
    }
    h.show_disconnect_window(wm);
    h.disconnect_menu().menu_state = DisconnectMenuStateType::ScreenOn;
}

/// `DisconnectMenu::hideScreen()`.
pub fn hide_screen<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.hide_disconnect_window(wm);
    h.disconnect_menu().menu_state = DisconnectMenuStateType::ScreenOff;
}

/// `DisconnectMenu::setPlayerName(playerNum, name)`.
pub fn set_player_name<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H, player_num: usize, name: &[u16]) {
    let control = win_by_name(wm, h, PLAYER_NAME_TEXT_CONTROL_NAMES[player_num]);
    if let Some(c) = control {
        if !name.is_empty() {
            static_text(wm, h, c, name);
            // showPlayerControls(playerNum);  (commented out upstream)
        }
    }

    let control = win_by_name(wm, h, PLAYER_TIMEOUT_TEXT_CONTROL_NAMES[player_num]);
    if let Some(c) = control {
        if !name.is_empty() || plant(21463) {
            static_text(wm, h, c, &[]);
        }
    }

    if !name.is_empty() != plant(21464) {
        show_player_controls(wm, h, player_num);
    } else {
        hide_player_controls(wm, h, player_num);
    }
}

/// `DisconnectMenu::setPlayerTimeoutTime(playerNum, newTime)`: `itoa(newTime, str, 10)`.
pub fn set_player_timeout_time<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H, player_num: usize, new_time: i64) {
    let control = win_by_name(wm, h, PLAYER_TIMEOUT_TEXT_CONTROL_NAMES[player_num]);
    if let Some(c) = control {
        static_text(wm, h, c, &utf16(&new_time.to_string()));
    }
}

/// `DisconnectMenu::showPlayerControls(slot)`.
pub fn show_player_controls<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H, slot: usize) {
    if let Some(c) = win_by_name(wm, h, PLAYER_NAME_TEXT_CONTROL_NAMES[slot]) {
        wm.win_hide(c, false);
    }
    if let Some(c) = win_by_name(wm, h, PLAYER_TIMEOUT_TEXT_CONTROL_NAMES[slot]) {
        wm.win_hide(c, false);
    }
    if let Some(c) = win_by_name(wm, h, PLAYER_VOTE_BUTTON_CONTROL_NAMES[slot]) {
        wm.win_hide(c, false);
        wm.win_enable(c, true);
    }
    if let Some(c) = win_by_name(wm, h, PLAYER_VOTE_COUNT_CONTROL_NAMES[slot]) {
        wm.win_hide(c, plant(21462));
    }
}

/// `DisconnectMenu::hidePlayerControls(slot)` (the vote button is hidden and, as upstream, also enabled).
pub fn hide_player_controls<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H, slot: usize) {
    if let Some(c) = win_by_name(wm, h, PLAYER_NAME_TEXT_CONTROL_NAMES[slot]) {
        wm.win_hide(c, true);
    }
    if let Some(c) = win_by_name(wm, h, PLAYER_TIMEOUT_TEXT_CONTROL_NAMES[slot]) {
        wm.win_hide(c, true);
    }
    if let Some(c) = win_by_name(wm, h, PLAYER_VOTE_BUTTON_CONTROL_NAMES[slot]) {
        wm.win_hide(c, true);
        if !plant(21461) {
            wm.win_enable(c, true);
        }
    }
    if let Some(c) = win_by_name(wm, h, PLAYER_VOTE_COUNT_CONTROL_NAMES[slot]) {
        wm.win_hide(c, true);
    }
}

/// `DisconnectMenu::showPacketRouterTimeout()`.
pub fn show_packet_router_timeout<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if let Some(c) = win_by_name(wm, h, PACKET_ROUTER_TIMEOUT_LABEL_CONTROL_NAME) {
        wm.win_hide(c, false);
    }
    if let Some(c) = win_by_name(wm, h, PACKET_ROUTER_TIMEOUT_CONTROL_NAME) {
        if !plant(21470) {
            static_text(wm, h, c, &[]); // start it off with a blank string.
        }
        wm.win_hide(c, false);
    }
}

/// `DisconnectMenu::hidePacketRouterTimeout()`.
pub fn hide_packet_router_timeout<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H) {
    if let Some(c) = win_by_name(wm, h, PACKET_ROUTER_TIMEOUT_LABEL_CONTROL_NAME) {
        wm.win_hide(c, true);
    }
    if let Some(c) = win_by_name(wm, h, PACKET_ROUTER_TIMEOUT_CONTROL_NAME) {
        wm.win_hide(c, true);
    }
}

/// `DisconnectMenu::setPacketRouterTimeoutTime(newTime)`.
pub fn set_packet_router_timeout_time<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H, new_time: i64) {
    if let Some(c) = win_by_name(wm, h, if plant(21469) { PACKET_ROUTER_TIMEOUT_LABEL_CONTROL_NAME } else { PACKET_ROUTER_TIMEOUT_CONTROL_NAME }) {
        static_text(wm, h, c, &utf16(&new_time.to_string()));
    }
}

/// `DisconnectMenu::sendChat(text)`.
pub fn send_chat<H: DisconnectMenuHost>(h: &mut H, text: &[u16]) {
    h.network_send_disconnect_chat(text);
}

/// `DisconnectMenu::showChat(text)`.
pub fn show_chat<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H, text: &[u16]) {
    if let Some(display) = win_by_name(wm, h, TEXT_DISPLAY_CONTROL_NAME) {
        gadget_list_box::add_entry_text(wm, h, display, text, CHAT_NORMAL_COLOR, -1, -1, true);
    }
}

/// `DisconnectMenu::quitGame()`.
pub fn quit_game<H: DisconnectMenuHost>(h: &mut H) {
    h.network_quit_game();
}

/// `DisconnectMenu::removePlayer(slot, playerName)`.
pub fn remove_player<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H, slot: usize, player_name: &[u16]) {
    hide_player_controls(wm, h, slot);

    let display = win_by_name(wm, h, TEXT_DISPLAY_CONTROL_NAME);
    let fmt = h.fetch_text(b"Network:PlayerLeftGame");
    let text = h.format_with_name(&fmt, player_name);

    if let Some(d) = display {
        gadget_list_box::add_entry_text(wm, h, d, &text, if plant(21466) { 0xFFFF_FFFFu32 as i32 } else { CHAT_NORMAL_COLOR }, -1, -1, true);
    }
}

/// `DisconnectMenu::voteForPlayer(slot)`: "Casting vote for disconnect slot %d" in the debug log, then the vote.
pub fn vote_for_player<H: DisconnectMenuHost>(h: &mut H, slot: i32) {
    h.network_vote_for_player_disconnect(slot);
}

/// `DisconnectMenu::updateVotes(slot, votes)`.
pub fn update_votes<H: DisconnectMenuHost>(wm: &mut GameWindowManager, h: &mut H, slot: usize, votes: i32) {
    if let Some(c) = win_by_name(wm, h, if plant(21465) { PLAYER_TIMEOUT_TEXT_CONTROL_NAMES[slot] } else { PLAYER_VOTE_COUNT_CONTROL_NAMES[slot] }) {
        static_text(wm, h, c, &utf16(&votes.to_string()));
    }
}
