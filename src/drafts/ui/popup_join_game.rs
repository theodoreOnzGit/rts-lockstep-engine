//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/PopupJoinGame.cpp
//!     (the file's statics, `PopupJoinGameInit`, `PopupJoinGameInput`, `PopupJoinGameSystem`, `joinGame`)
//!
//! The GameSpy service is gone: `TheGameSpyInfo` (the staging rooms) and `TheGameSpyPeerMessageQueue` are the
//! host's.

use crate::client::gadget;
use crate::client::gadget_text_entry as te;
use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, key, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PopupJoinGame {
    pub parent_popup_id: i32,
    pub text_entry_game_password_id: i32,
    pub button_cancel_id: i32,
    pub parent_popup: Option<WinId>,
    pub text_entry_game_password: Option<WinId>,
}

/// The staging room `TheGameSpyInfo->findStagingRoomByID(getCurrentStagingRoomID())` found.
#[derive(Clone, Debug, Default)]
pub struct StagingRoomInfo {
    pub id: i32,
    pub game_name: Vec<u16>,
}

/// What the join popup reaches in the GameSpy layer.
pub trait PopupJoinGameHost: ShellHost {
    fn popup_join_game(&mut self) -> &mut PopupJoinGame;
    /// `TheGameSpyInfo->findStagingRoomByID(TheGameSpyInfo->getCurrentStagingRoomID())`.
    fn current_staging_room(&self) -> Option<StagingRoomInfo>;
    /// `winSetModal(window)`.
    fn win_set_modal(&mut self, wm: &mut GameWindowManager, w: Option<WinId>);
    /// `GameSpyCloseOverlay(GSOVERLAY_GAMEPASSWORD)`.
    fn gamespy_close_overlay_game_password(&mut self, wm: &mut GameWindowManager);
    /// `SetLobbyAttemptHostJoin(FALSE)`.
    fn set_lobby_attempt_host_join_false(&mut self);
    /// `PeerRequest` of `PEERREQUEST_JOINSTAGINGROOM` (text = game name, stagingRoom.id, password), queued with
    /// `TheGameSpyPeerMessageQueue->addRequest`.
    fn peer_request_join_staging_room(&mut self, game_name: &[u16], room_id: i32, password: &[u8]);
}

/// `PopupJoinGameInit(layout, userData)`.
pub fn popup_join_game_init<H: PopupJoinGameHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    let parent_id = ShellHost::name_to_key(h, b"PopupJoinGame.wnd:ParentJoinPopUp") as i32;
    let parent = wm.win_get_window_from_id(None, parent_id);
    let entry_id = ShellHost::name_to_key(h, b"PopupJoinGame.wnd:TextEntryGamePassword") as i32;
    let entry = wm.win_get_window_from_id(parent, entry_id);
    te::set_text(wm, h, entry, &[]);

    let name_id = ShellHost::name_to_key(h, b"PopupJoinGame.wnd:StaticTextGameName") as i32;
    let static_text_game_name = wm.win_get_window_from_id(parent, name_id);
    gadget::static_text_set_text(wm, h, static_text_game_name, &[]);

    let cancel_id = ShellHost::name_to_key(h, b"PopupJoinGame.wnd:ButtonCancel") as i32;
    {
        let p = h.popup_join_game();
        p.parent_popup_id = parent_id;
        p.parent_popup = parent;
        p.text_entry_game_password_id = entry_id;
        p.text_entry_game_password = entry;
        p.button_cancel_id = cancel_id;
    }

    if let Some(room) = h.current_staging_room() {
        gadget::static_text_set_text(wm, h, static_text_game_name, &room.game_name);
    }

    wm.win_set_focus(h, parent);
    h.win_set_modal(wm, parent);
}

/// The shared "close the password overlay and give up" tail of Escape, *Cancel* and a missing room.
fn close_password_overlay<H: PopupJoinGameHost>(wm: &mut GameWindowManager, h: &mut H) {
    h.gamespy_close_overlay_game_password(wm);
    h.set_lobby_attempt_host_join_false();
    h.popup_join_game().parent_popup = None;
}

/// `PopupJoinGameInput(window, msg, mData1, mData2)`.
pub fn popup_join_game_input<H: PopupJoinGameHost>(wm: &mut GameWindowManager, h: &mut H, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            close_password_overlay(wm, h);
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `joinGame(password)` (PopupJoinGame.cpp:208).
fn join_game<H: PopupJoinGameHost>(wm: &mut GameWindowManager, h: &mut H, password: &[u8]) {
    let Some(room) = h.current_staging_room() else {
        close_password_overlay(wm, h);
        return;
    };

    h.peer_request_join_staging_room(&room.game_name, room.id, password);
    // DEBUG_LOG(("Attempting to join game %d(%ls) with password [%s]\n", ...))

    h.gamespy_close_overlay_game_password(wm);
    h.popup_join_game().parent_popup = None;
}

/// `PopupJoinGameSystem(window, msg, mData1, mData2)`.
pub fn popup_join_game_system<H: PopupJoinGameHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            if wm.tree.windows[control].id == h.popup_join_game().button_cancel_id {
                close_password_overlay(wm, h);
            }
        }
        gwm::INPUT_FOCUS => {
            // (break, not return: handled all the same)
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GEM_EDIT_DONE => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let p = h.popup_join_game().clone();
            if wm.tree.windows[control].id == p.text_entry_game_password_id {
                let mut txt = te::get_text(wm, h, p.text_entry_game_password);
                te::set_text(wm, h, p.text_entry_game_password, &[]);
                txt = crate::client::replay_menu::trim(&txt);
                if !txt.is_empty() {
                    let munkee: Vec<u8> = txt.iter().map(|&c| c as u8).collect(); // munkee.translate(txtInput)
                    join_game(wm, h, &munkee);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
