//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_disconnect.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/DisconnectWindow.cpp
//!     (the file's statics, `InitDisconnectWindow`, `ShowDisconnectWindow`, `HideDisconnectWindow`,
//!      `DisconnectControlInput`, `DisconnectControlSystem`)

use crate::client::gadget_list_box;
use crate::client::gadget_text_entry;
use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// The file's statics (`NAMEKEY_INVALID` is 0, windows `NULL`). `disconnectMenuLayout` is a layout index.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisconnectWindow {
    pub disconnect_menu_layout: Option<usize>,
    pub text_entry_id: i32,
    pub text_display_id: i32,
    pub text_entry_window: Option<WinId>,
    pub text_display_window: Option<WinId>,
    pub button_quit_id: i32,
    pub button_quit_window: Option<WinId>,
    pub button_vote_player_id: [i32; 7],
    pub button_vote_player_window: [Option<WinId>; 7],
}

/// What the disconnect window reaches outside the shell.
pub trait DisconnectWindowHost: ShellHost + crate::drafts::ui::disconnect_menu::DisconnectMenuHost {
    fn disconnect_window(&mut self) -> &mut DisconnectWindow;
}

/// `GameMakeColor(255,255,255,255)`.
const WHITE: i32 = 0xFFFF_FFFFu32 as i32;

/// `InitDisconnectWindow()` (DisconnectWindow.cpp:68).
fn init_disconnect_window<H: DisconnectWindowHost>(wm: &mut GameWindowManager, h: &mut H) {
    let text_entry_id = ShellHost::name_to_key(h, b"DisconnectScreen.wnd:TextEntry") as i32;
    let text_display_id = ShellHost::name_to_key(h, b"DisconnectScreen.wnd:ListboxTextDisplay") as i32;

    let text_entry_window = wm.win_get_window_from_id(None, text_entry_id);
    let text_display_window = wm.win_get_window_from_id(None, text_display_id);
    {
        let d = h.disconnect_window();
        d.text_entry_id = text_entry_id;
        d.text_display_id = text_display_id;
        d.text_entry_window = text_entry_window;
        d.text_display_window = text_display_window;
    }

    if let Some(w) = text_entry_window {
        gadget_text_entry::set_text(wm, h, Some(w), &[]);
        if !plant(21478) {
            wm.win_set_focus(h, Some(w));
        }
    }

    let button_quit_id = ShellHost::name_to_key(h, b"DisconnectScreen.wnd:ButtonQuitGame") as i32;
    let button_quit_window = wm.win_get_window_from_id(None, button_quit_id);
    h.disconnect_window().button_quit_id = button_quit_id;
    h.disconnect_window().button_quit_window = button_quit_window;

    for i in 0..7 {
        let id = ShellHost::name_to_key(h, format!("DisconnectScreen.wnd:ButtonKickPlayer{}", i + 1).as_bytes()) as i32;
        let win = wm.win_get_window_from_id(None, id);
        h.disconnect_window().button_vote_player_id[i] = id;
        h.disconnect_window().button_vote_player_window[i] = win;
    }
}

/// Both Show and Hide load `Menus/DisconnectScreen.wnd` if it is not there yet and init the statics.
fn ensure_layout<H: DisconnectWindowHost>(wm: &mut GameWindowManager, h: &mut H) -> Option<usize> {
    if h.disconnect_window().disconnect_menu_layout.is_none() {
        // load layout from disk
        let layout = h.create_layout(wm, b"Menus/DisconnectScreen.wnd");
        h.disconnect_window().disconnect_menu_layout = layout;
        // init it
        init_disconnect_window(wm, h);
    }
    h.disconnect_window().disconnect_menu_layout
}

/// `ShowDisconnectWindow()` (DisconnectWindow.cpp:99): in upstream the first call hides nothing and shows the layout,
/// later calls show it; the seven vote buttons and Quit are enabled, the layout brought forward, the list box reset and
/// given `GUI:InternetDisconnectionMenuBody1` in white.
pub fn show_disconnect_window<H: DisconnectWindowHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(layout) = ensure_layout(wm, h) else { return };
    wm.layout_hide(layout, false);

    let d = h.disconnect_window().clone();
    for w in d.button_vote_player_window.iter().take(if plant(21471) { 6 } else { 7 }).flatten() {
        wm.win_enable(*w, true);
    }
    if let Some(w) = d.button_quit_window {
        wm.win_enable(w, true);
    }
    if !plant(21472) {
        wm.layout_bring_forward(layout);
    }

    if let Some(display) = d.text_display_window {
        gadget_list_box::reset(wm, h, display);
        let text = crate::drafts::ui::disconnect_menu::DisconnectMenuHost::fetch_text(h, b"GUI:InternetDisconnectionMenuBody1");
        gadget_list_box::add_entry_text(wm, h, display, &text, WHITE, -1, -1, true);
    }
}

/// `HideDisconnectWindow()` (DisconnectWindow.cpp:140): loads the layout if needed (and inits), then hides it.
pub fn hide_disconnect_window<H: DisconnectWindowHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(layout) = ensure_layout(wm, h) else { return };
    wm.layout_hide(layout, plant(21477) != true);
}

/// `DisconnectControlInput(window, msg, mData1, mData2)`.
pub fn disconnect_control_input(_window: WinId, _msg: u32, _d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    MsgHandled::Ignored
}

/// `DisconnectControlSystem(window, msg, mData1, mData2)` (DisconnectWindow.cpp:182).
pub fn disconnect_control_system<H: DisconnectWindowHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.win(control).id;
            let d = h.disconnect_window().clone();

            if control_id == d.button_quit_id {
                crate::drafts::ui::disconnect_menu::quit_game(h);
                if let Some(w) = d.button_quit_window {
                    wm.win_enable(w, plant(21473));
                }
            } else {
                for i in 0..7 {
                    if control_id == d.button_vote_player_id[i] {
                        crate::drafts::ui::disconnect_menu::vote_for_player(h, i as i32 + i32::from(plant(21474)));
                        if let Some(w) = d.button_vote_player_window[i] {
                            wm.win_enable(w, false);
                        }
                        break;
                    }
                }
            }
        }
        ggm::GEM_EDIT_DONE => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.win(control).id;
            let d = h.disconnect_window().clone();

            // Take the user's input and echo it into the chat window as well as send it to the other clients
            if control_id == d.text_entry_id {
                // read the user's input
                let input = gadget_text_entry::get_text(wm, h, d.text_entry_window);
                // Clear the text entry line
                if !plant(21476) {
                    gadget_text_entry::set_text(wm, h, d.text_entry_window, &[]);
                }
                // Clean up the text (remove leading/trailing chars, etc)
                let input = if plant(21475) { input } else { crate::client::replay_menu::trim(&input) };
                // Echo the user's input to the chat window
                if !input.is_empty() || plant(21479) {
                    crate::drafts::ui::disconnect_menu::send_chat(h, &input);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
