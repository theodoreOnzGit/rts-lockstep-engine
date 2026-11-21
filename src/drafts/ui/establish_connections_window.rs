//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_establish_connections_window.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/EstablishConnectionsWindow.cpp
//!     (the file's statics and tables, `showGameSpyGameOptionsUnderlyingGUIElements`,
//!      `showGameSpyQMUnderlyingGUIElements`, `InitEstablishConnectionsDialog`, `ShowEstablishConnectionsWindow`,
//!      `HideEstablishConnectionsWindow`, `EstablishConnectionsControlInput`, `EstablishConnectionsControlSystem`)
//!
//! The file declares per-player name and status ids and windows it never assigns; they are left out.

use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// `layoutFilename`, `parentName`, `gadgetsToHide`, `perPlayerGadgetsToHide` (the NULL terminator is the slice's end).
pub const LAYOUT_FILENAME: &str = "GameSpyGameOptionsMenu.wnd";
pub const PARENT_NAME: &str = "GameSpyGameOptionsMenuParent";
pub const GADGETS_TO_HIDE: [&str; 10] = [
    "MapWindow",
    "StaticTextGameName",
    "StaticTextTeam",
    "StaticTextFaction",
    "StaticTextColor",
    "StaticTextPlayers",
    "TextEntryMapDisplay",
    "ButtonSelectMap",
    "ButtonStart",
    "StaticTextMapPreview",
];
pub const PER_PLAYER_GADGETS_TO_HIDE: [&str; 6] = ["ComboBoxTeam", "ComboBoxColor", "ComboBoxPlayerTemplate", "ComboBoxPlayer", "ButtonAccept", "GenericPing"];

/// `qmlayoutFilename`, `qmparentName`, `qmgadgetsToHide`, `qmperPlayerGadgetsToHide`.
pub const QM_LAYOUT_FILENAME: &str = "WOLQuickMatchMenu.wnd";
pub const QM_PARENT_NAME: &str = "WOLQuickMatchMenuParent";
pub const QM_GADGETS_TO_HIDE: [&str; 7] = ["StaticTextTitle", "ButtonBack", "ButtonOptions", "ButtonBuddies", "ButtonWiden", "ButtonStop", "ButtonStart"];
pub const QM_PER_PLAYER_GADGETS_TO_HIDE: [&str; 0] = [];

/// The file's statics that are assigned: `establishConnectionsLayout`, `buttonQuitID`, `buttonQuitWindow`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EstablishConnectionsWindow {
    pub establish_connections_layout: Option<usize>,
    pub button_quit_id: i32,
    pub button_quit_window: Option<WinId>,
}

/// What the window reaches outside the shell.
pub trait EstablishConnectionsWindowHost: ShellHost {
    fn establish_connections_window(&mut self) -> &mut EstablishConnectionsWindow;
    /// `TheGameSpyGame->isQMGame()`.
    fn is_qm_game(&self) -> bool;
    /// `ShowUnderlyingGUIElements(show, layoutFilename, parentName, gadgetsToHide, perPlayerGadgetsToHide)`
    /// ([`crate::client::gui_util::show_underlying_gui_elements`]), with the file's tables passed in.
    fn show_underlying_gui_elements(&mut self, wm: &mut GameWindowManager, show: bool, layout_filename: &str, parent_name: &str, gadgets_to_hide: &[&str], per_player_gadgets_to_hide: &[&str]);
    /// `TheEstablishConnectionsMenu->abortGame()`.
    fn establish_connections_abort_game(&mut self);
}

fn show_game_spy_game_options_underlying_gui_elements<H: EstablishConnectionsWindowHost>(wm: &mut GameWindowManager, h: &mut H, show: bool) {
    h.show_underlying_gui_elements(wm, show, LAYOUT_FILENAME, PARENT_NAME, &GADGETS_TO_HIDE, &PER_PLAYER_GADGETS_TO_HIDE);
}

fn show_game_spy_qm_underlying_gui_elements<H: EstablishConnectionsWindowHost>(wm: &mut GameWindowManager, h: &mut H, show: bool) {
    h.show_underlying_gui_elements(wm, show, QM_LAYOUT_FILENAME, QM_PARENT_NAME, &QM_GADGETS_TO_HIDE, &QM_PER_PLAYER_GADGETS_TO_HIDE);
}

/// `InitEstablishConnectionsDialog()`.
fn init_establish_connections_dialog<H: EstablishConnectionsWindowHost>(wm: &mut GameWindowManager, h: &mut H) {
    let id = ShellHost::name_to_key(h, if plant(21229) { b"EstablishConnectionsScreen.wnd:ButtonQuit1" } else { b"EstablishConnectionsScreen.wnd:ButtonQuit" }) as i32;
    let win = wm.win_get_window_from_id(None, id);
    h.establish_connections_window().button_quit_id = id;
    h.establish_connections_window().button_quit_window = win;
}

/// `ShowEstablishConnectionsWindow()`.
pub fn show_establish_connections_window<H: EstablishConnectionsWindowHost>(wm: &mut GameWindowManager, h: &mut H) {
    if h.establish_connections_window().establish_connections_layout.is_none() || plant(21226) {
        let layout = h.create_layout(wm, b"Menus/EstablishConnectionsScreen.wnd");
        h.establish_connections_window().establish_connections_layout = layout;
        init_establish_connections_dialog(wm, h);
    }
    let Some(layout) = h.establish_connections_window().establish_connections_layout else { return };
    wm.layout_hide(layout, plant(21221));
    // `winSetFocus(establishConnectionsLayout->getFirstWindow())`
    let first = wm.layouts[layout].windows.first().copied();
    wm.win_set_focus(h, first);
    if h.is_qm_game() == plant(21222) {
        show_game_spy_game_options_underlying_gui_elements(wm, h, plant(21223));
    } else {
        show_game_spy_qm_underlying_gui_elements(wm, h, plant(21223));
    }
}

/// `HideEstablishConnectionsWindow()`: returns when there is no layout; else destroys it (`destroyWindows`,
/// `deleteInstance`) and shows the underlying elements again.
pub fn hide_establish_connections_window<H: EstablishConnectionsWindowHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(layout) = h.establish_connections_window().establish_connections_layout else { return };
    if !plant(21225) {
        wm.layout_destroy_windows(layout);
    }
    // (`deleteInstance`: the layout record itself is released by the host)
    h.establish_connections_window().establish_connections_layout = None;
    if !h.is_qm_game() {
        show_game_spy_game_options_underlying_gui_elements(wm, h, !plant(21224));
    } else {
        show_game_spy_qm_underlying_gui_elements(wm, h, !plant(21224));
    }
}

/// `EstablishConnectionsControlInput(window, msg, mData1, mData2)`.
pub fn establish_connections_control_input(_window: WinId, _msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    MsgHandled::Ignored
}

/// `EstablishConnectionsControlSystem(window, msg, mData1, mData2)`: every message is reported handled.
pub fn establish_connections_control_system<H: EstablishConnectionsWindowHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    if msg == ggm::GBM_SELECTED {
        if let Some(control) = data_win(d1) {
            let control_id = wm.win(control).id;
            if control_id == h.establish_connections_window().button_quit_id || plant(21227) {
                h.establish_connections_abort_game();
            }
        }
    }
    if plant(21228) && msg != ggm::GBM_SELECTED {
        return MsgHandled::Ignored;
    }
    MsgHandled::Handled
}
