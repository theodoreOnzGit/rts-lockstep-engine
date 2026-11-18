//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/EstablishConnectionsMenu/EstablishConnectionsMenu.cpp
//!     (`EstablishConnectionsMenu`: initMenu, endMenu, abortGame, setPlayerName, setPlayerStatus)

use crate::client::window_manager::{GameWindowManager, GuiHost};

/// `m_playerReadyControlNames` (unused by the .cpp's functions).
pub const PLAYER_READY_CONTROL_NAMES: [&str; 7] = [
    "EstablishConnectionsScreen.wnd:ButtonAccept1",
    "EstablishConnectionsScreen.wnd:ButtonAccept2",
    "EstablishConnectionsScreen.wnd:ButtonAccept3",
    "EstablishConnectionsScreen.wnd:ButtonAccept4",
    "EstablishConnectionsScreen.wnd:ButtonAccept5",
    "EstablishConnectionsScreen.wnd:ButtonAccept6",
    "EstablishConnectionsScreen.wnd:ButtonAccept7",
];
/// `m_playerNameControlNames`.
pub const PLAYER_NAME_CONTROL_NAMES: [&str; 7] = [
    "EstablishConnectionsScreen.wnd:StaticPlayer1Name",
    "EstablishConnectionsScreen.wnd:StaticPlayer2Name",
    "EstablishConnectionsScreen.wnd:StaticPlayer3Name",
    "EstablishConnectionsScreen.wnd:StaticPlayer4Name",
    "EstablishConnectionsScreen.wnd:StaticPlayer5Name",
    "EstablishConnectionsScreen.wnd:StaticPlayer6Name",
    "EstablishConnectionsScreen.wnd:StaticPlayer7Name",
];
/// `m_playerStatusControlNames`.
pub const PLAYER_STATUS_CONTROL_NAMES: [&str; 7] = [
    "EstablishConnectionsScreen.wnd:StaticPlayer1Status",
    "EstablishConnectionsScreen.wnd:StaticPlayer2Status",
    "EstablishConnectionsScreen.wnd:StaticPlayer3Status",
    "EstablishConnectionsScreen.wnd:StaticPlayer4Status",
    "EstablishConnectionsScreen.wnd:StaticPlayer5Status",
    "EstablishConnectionsScreen.wnd:StaticPlayer6Status",
    "EstablishConnectionsScreen.wnd:StaticPlayer7Status",
];

/// `enum NATConnectionState` (NAT.h), the members this file tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NatConnectionState {
    NetgearDelay,
    WaitingToBegin,
    WaitingForManglerResponse,
    WaitingForMangledPort,
    WaitingForResponse,
    Done,
    Failed,
}

/// What the menu reaches outside the window manager.
pub trait EstablishConnectionsHost: GuiHost {
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `ShowEstablishConnectionsWindow()`, `HideEstablishConnectionsWindow()` (EstablishConnectionsWindow.cpp).
    fn show_establish_connections_window(&mut self, wm: &mut GameWindowManager);
    fn hide_establish_connections_window(&mut self, wm: &mut GameWindowManager);
    /// `TheGameText->fetch(label)`.
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
}

/// `EstablishConnectionsMenu::initMenu()`.
pub fn init_menu<H: EstablishConnectionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.show_establish_connections_window(wm);
}

/// `EstablishConnectionsMenu::endMenu()`.
pub fn end_menu<H: EstablishConnectionsHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.hide_establish_connections_window(wm);
}

/// `EstablishConnectionsMenu::abortGame()`: empty upstream.
pub fn abort_game() {}

/// `EstablishConnectionsMenu::setPlayerName(slot, name)`: a missing control returns (the assert is compiled out).
pub fn set_player_name<H: EstablishConnectionsHost>(wm: &mut GameWindowManager, h: &mut H, slot: usize, name: &[u16]) {
    let control_id = h.name_to_key(PLAYER_NAME_CONTROL_NAMES[slot].as_bytes());
    let Some(control) = wm.win_get_window_from_id(None, control_id) else { return };
    crate::client::gadget::static_text_set_text(wm, h, Some(control), name);
}

/// `EstablishConnectionsMenu::setPlayerStatus(slot, state)`. `NATCONNECTIONSTATE_NETGEARDELAY` is commented out
/// upstream, so it falls to the unknown label.
pub fn set_player_status<H: EstablishConnectionsHost>(wm: &mut GameWindowManager, h: &mut H, slot: usize, state: NatConnectionState) {
    let control_id = h.name_to_key(PLAYER_STATUS_CONTROL_NAMES[slot].as_bytes());
    let Some(control) = wm.win_get_window_from_id(None, control_id) else { return };
    let label: &[u8] = match state {
        NatConnectionState::WaitingForManglerResponse => b"GUI:WaitingForManglerResponse",
        NatConnectionState::WaitingForMangledPort => b"GUI:WaitingForMangledPort",
        NatConnectionState::WaitingForResponse => b"GUI:WaitingForResponse",
        NatConnectionState::Done => b"GUI:ConnectionDone",
        NatConnectionState::Failed => b"GUI:ConnectionFailed",
        NatConnectionState::WaitingToBegin => b"GUI:WaitingToBeginConnection",
        NatConnectionState::NetgearDelay => b"GUI:UnknownConnectionState",
    };
    let text = h.fetch_text(label);
    crate::client::gadget::static_text_set_text(wm, h, Some(control), &text);
}
