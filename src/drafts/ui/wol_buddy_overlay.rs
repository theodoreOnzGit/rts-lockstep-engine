//! UNTESTED DRAFT (#202, #203): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLBuddyOverlay.cpp
//!     (the file's statics, `BuddyControls`, `InitBuddyControls`, `BuddyControlSystem`, `insertChat`,
//!      `updateBuddyInfo`, `HandleBuddyResponses`, `showNotificationBox`, `deleteNotificationBox`,
//!      `PopulateOldBuddyMessages`, `WOLBuddyOverlayInit/Shutdown/Update/Input/System`,
//!      `PopupBuddyNotificationSystem`, `WOLBuddyOverlayRCMenuInit`, `closeRightClickMenu`, `RequestBuddyAdd`,
//!      `WOLBuddyOverlayRCMenuSystem`, `setUnignoreText`, `refreshIgnoreList`)
//!
//! Not translated: the `DEBUG_LOG` lines and `DEBUG_ASSERTCRASH`es, the commented-out text-entry / add / delete /
//! double-click code. The GameSpy service is gone: `TheGameSpyInfo`, the buddy and persistent-storage queues,
//! `TheGameSpyGame`, `ThePlayerList` and the audio are the host's. The GameSpy SDK's `GPEnum` (gp.h) is not vendored
//! under `upstream/`, so only the three values this file names are modelled ([`GpStatus`]).
//!
//! Upstream dereferences `rcLayout->getFirstWindow()` without a null check after `winCreateLayout`; a layout that
//! fails to load returns early here instead.

use crate::client::gadget;
use crate::client::gadget_list_box as lb;
use crate::client::gadget_text_entry as te;
use crate::client::shell::ShellHost;
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::client::window_transitions as wt;
use crate::common::game_info::MAX_SLOTS;

/// `NOTIFICATION_EXPIRES`.
const NOTIFICATION_EXPIRES: u32 = 3000;
/// `MAX_BUDDY_CHAT_LEN`.
const MAX_BUDDY_CHAT_LEN: usize = 128;

/// `enum RCItemType` (PeerDefs.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RcItemType {
    #[default]
    ItemBuddy,
    ItemRequest,
    ItemNonBuddy,
    ItemNone,
}

impl RcItemType {
    /// The value stored in a list's item data.
    pub fn as_i32(self) -> i32 {
        match self {
            RcItemType::ItemBuddy => 0,
            RcItemType::ItemRequest => 1,
            RcItemType::ItemNonBuddy => 2,
            RcItemType::ItemNone => 3,
        }
    }
    /// `(RCItemType)(Int)data`.
    pub fn from_i32(v: i32) -> RcItemType {
        match v {
            0 => RcItemType::ItemBuddy,
            1 => RcItemType::ItemRequest,
            2 => RcItemType::ItemNonBuddy,
            _ => RcItemType::ItemNone,
        }
    }
}

/// `GameSpyRCMenuData`.
#[derive(Clone, Debug, Default)]
pub struct GameSpyRcMenuData {
    pub nick: Vec<u8>,
    pub id: i32,
    pub item_type: RcItemType,
}

/// The `GPEnum` values this file names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GpStatus {
    #[default]
    Offline,
    Online,
    RecvGameInvite,
    Other(i32),
}

/// `BuddyInfo` (PeerDefs.h).
#[derive(Clone, Debug, Default)]
pub struct BuddyInfo {
    pub id: i32,
    pub name: Vec<u8>,
    pub email: Vec<u8>,
    pub country_code: Vec<u8>,
    pub status: GpStatus,
    pub status_string: Vec<u16>,
    pub location_string: Vec<u16>,
}

/// `BuddyMessage` (PeerDefs.h).
#[derive(Clone, Debug, Default)]
pub struct BuddyMessage {
    pub timestamp: u32,
    pub sender_id: i32,
    pub sender_nick: Vec<u8>,
    pub recipient_id: i32,
    pub recipient_nick: Vec<u8>,
    pub message: Vec<u16>,
}

/// The `BuddyResponse` types `HandleBuddyResponses` acts on.
#[derive(Clone, Debug)]
pub enum BuddyResponseKind {
    /// `BUDDYRESPONSE_LOGIN`.
    Login,
    /// `BUDDYRESPONSE_DISCONNECT`.
    Disconnect,
    /// `BUDDYRESPONSE_MESSAGE`.
    Message { profile: i32, text: Vec<u16>, date: u32, nick: Vec<u8> },
    /// `BUDDYRESPONSE_REQUEST`.
    Request { profile: i32, country_code: Vec<u8>, email: Vec<u8>, nick: Vec<u8>, text: Vec<u16> },
    /// `BUDDYRESPONSE_STATUS`.
    Status { profile: i32, status: GpStatus, country_code: Vec<u8>, email: Vec<u8>, nick: Vec<u8>, status_string: Vec<u8>, location: Vec<u8> },
    /// Any other response type.
    Other,
}

/// `BuddyControls` and the `BUDDY_WINDOW_*` types of `InitBuddyControls`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuddyControls {
    pub listbox_chat: Option<WinId>,
    pub listbox_chat_id: i32,
    pub listbox_buddies: Option<WinId>,
    pub listbox_buddies_id: i32,
    pub text_entry_edit: Option<WinId>,
    pub text_entry_edit_id: i32,
    pub is_init: bool,
}

/// `BUDDY_RESETALL_CRAP`, `BUDDY_WINDOW_BUDDIES`, `BUDDY_WINDOW_DIPLOMACY`, `BUDDY_WINDOW_WELCOME_SCREEN`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuddyWindowType {
    ResetAll,
    Buddies,
    Diplomacy,
    WelcomeScreen,
}

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WolBuddyOverlay {
    pub parent_id: i32,
    pub button_hide_id: i32,
    pub button_add_buddy_id: i32,
    pub button_delete_buddy_id: i32,
    pub button_accept_buddy_id: i32,
    pub button_deny_buddy_id: i32,
    pub radio_button_buddies_id: i32,
    pub radio_button_ignore_id: i32,
    pub parent_buddies_id: i32,
    pub parent_ignore_id: i32,
    pub listbox_ignore_id: i32,
    pub button_notification_id: i32,
    pub parent: Option<WinId>,
    pub button_hide: Option<WinId>,
    pub button_add_buddy: Option<WinId>,
    pub button_delete_buddy: Option<WinId>,
    pub button_accept_buddy: Option<WinId>,
    pub button_deny_buddy: Option<WinId>,
    pub radio_button_buddies: Option<WinId>,
    pub radio_button_ignore: Option<WinId>,
    pub parent_buddies: Option<WinId>,
    pub parent_ignore: Option<WinId>,
    pub listbox_ignore: Option<WinId>,
    pub is_overlay_active: bool,
    pub rc_menu: Option<WinId>,
    pub notice_layout: Option<usize>,
    pub notice_expires: u32,
    pub last_notification_was_status: bool,
    pub num_online_in_notification: i32,
    pub buddy_controls: BuddyControls,
    // the right-click menu's ids (`buttonAddID` ... `buttonStatsID`)
    pub rc_button_add_id: i32,
    pub rc_button_delete_id: i32,
    pub rc_button_play_id: i32,
    pub rc_button_ignore_id: i32,
    pub rc_button_stats_id: i32,
}

/// What the buddy overlay reaches in the GameSpy layer and the engine.
pub trait WolBuddyOverlayHost: ShellHost {
    fn wol_buddy_overlay(&mut self) -> &mut WolBuddyOverlay;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `TheGameText->fetch(label, &exists)`.
    fn fetch_text_exists(&self, label: &[u8]) -> (Vec<u16>, bool);
    /// `format(fmt, arg)` with a `%ls` / `%hs` argument.
    fn format_text_arg(&self, fmt: &[u16], arg: &[u16]) -> Vec<u16>;
    /// `format(fmt, nick (%hs), text (%ls))`.
    fn format_nick_text(&self, fmt: &[u16], nick: &[u8], text: &[u16]) -> Vec<u16>;
    /// `GameSpyColor[GSCOLOR_*]`.
    fn gamespy_color_default(&self) -> i32;
    fn gamespy_color_player_self(&self) -> i32;
    fn gamespy_color_player_buddy(&self) -> i32;
    fn gamespy_color_player_ignored(&self) -> i32;
    /// `MultiByteToWideCharSingleLine(line)`.
    fn multi_byte_to_wide_char(&self, line: &[u8]) -> Vec<u16>;
    fn time_now(&self) -> u32;

    // TheGameSpyInfo
    fn has_gamespy_info(&self) -> bool;
    fn gamespy_local_profile_id(&self) -> i32;
    fn gamespy_local_base_name(&self) -> Vec<u8>;
    /// `getBuddyMap()` and `getBuddyRequestMap()` in key order; `find`, `[]=`, `erase`.
    fn buddy_map(&self) -> Vec<(i32, BuddyInfo)>;
    fn buddy_map_get(&self, id: i32) -> Option<BuddyInfo>;
    fn buddy_map_set(&mut self, info: BuddyInfo);
    fn buddy_map_erase(&mut self, id: i32);
    fn buddy_request_map(&self) -> Vec<(i32, BuddyInfo)>;
    fn buddy_request_map_get(&self, id: i32) -> Option<BuddyInfo>;
    fn buddy_request_map_set(&mut self, info: BuddyInfo);
    fn buddy_request_map_erase(&mut self, id: i32);
    /// `getBuddyMessages()`: the list, and `push_back`.
    fn buddy_messages(&self) -> Vec<BuddyMessage>;
    fn buddy_messages_push(&mut self, m: BuddyMessage);
    fn gamespy_is_buddy(&self, id: i32) -> bool;
    fn gamespy_is_saved_ignored(&self, id: i32) -> bool;
    fn gamespy_is_ignored(&self, nick: &[u8]) -> bool;
    fn gamespy_remove_from_saved_ignore_list(&mut self, id: i32);
    fn gamespy_add_to_saved_ignore_list(&mut self, id: i32, nick: &[u8]);
    fn gamespy_remove_from_ignore_list(&mut self, nick: &[u8]);
    fn gamespy_add_to_ignore_list(&mut self, nick: &[u8]);
    /// `returnSavedIgnoreList()` (id, nick) in key order; `returnIgnoreList()` in set order.
    fn gamespy_saved_ignore_list(&self) -> Vec<(i32, Vec<u8>)>;
    fn gamespy_ignore_list(&self) -> Vec<Vec<u8>>;
    /// `getGroupRoomList()->find(id)`: the room's `m_name`.
    fn gamespy_group_room_name(&self, id: i32) -> Option<Vec<u8>>;

    // buddy queue
    fn buddy_queue_exists(&self) -> bool;
    fn buddy_queue_is_connected(&self) -> bool;
    /// `getResponse(resp)`.
    fn buddy_queue_next_response(&mut self) -> Option<BuddyResponseKind>;
    /// `BUDDYREQUEST_MESSAGE` with `text` (already truncated) to `recipient`.
    fn buddy_request_message(&mut self, recipient: i32, text: &[u16]);
    /// `BUDDYREQUEST_ADDBUDDY`, `OKADD`, `DELBUDDY`, `DENYADD`.
    fn buddy_request_add_buddy(&mut self, id: i32, text: &[u16]);
    fn buddy_request_ok_add(&mut self, id: i32);
    fn buddy_request_del_buddy(&mut self, id: i32);
    fn buddy_request_deny_add(&mut self, id: i32);
    /// `PSREQUEST_READPLAYERSTATS`.
    fn ps_request_read_player_stats(&mut self, id: i32);

    // TheGameSpyGame, ThePlayerList
    /// `TheGameSpyGame && isInGame() && isGameInProgress() && !ThePlayerList->getLocalPlayer()->isPlayerActive()`.
    fn local_player_dead_in_gamespy_game(&self) -> bool;
    fn game_spy_game_slot_profile_id(&self, i: usize) -> i32;

    // other screens
    fn gamespy_close_overlay_buddy(&mut self, wm: &mut GameWindowManager);
    fn gamespy_open_overlay_buddy(&mut self, wm: &mut GameWindowManager);
    fn gamespy_close_overlay_player_info(&mut self, wm: &mut GameWindowManager);
    fn gamespy_open_overlay_player_info(&mut self, wm: &mut GameWindowManager);
    fn set_look_at_player(&mut self, profile_id: i32, nick: &[u8]);
    /// `PopulateLobbyPlayerListbox()`, `RefreshGameListBoxes()`.
    fn populate_lobby_player_listbox(&mut self, wm: &mut GameWindowManager);
    fn refresh_game_list_boxes(&mut self, wm: &mut GameWindowManager);
    /// `TheAudio->addAudioEvent(AudioEventRTS(name))` when `TheAudio` exists.
    fn audio_add_event(&mut self, name: &[u8]);
    /// `TheDisplay->getWidth()`, `getHeight()`.
    fn display_size(&self) -> (i32, i32);
    /// `rcMenu->winSetUserData(new GameSpyRCMenuData)`, and the take of it at the menu's button press:
    /// `winGetUserData()` then `delete` and `winSetUserData(NULL)`.
    fn win_set_rc_menu_data(&mut self, wm: &mut GameWindowManager, w: WinId, data: GameSpyRcMenuData);
    fn win_take_rc_menu_data(&mut self, wm: &mut GameWindowManager, w: WinId) -> Option<GameSpyRcMenuData>;
}

fn key_of<H: WolBuddyOverlayHost>(h: &mut H, n: &str) -> i32 {
    ShellHost::name_to_key(h, format!("WOLBuddyOverlay.wnd:{n}").as_bytes()) as i32
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn ascii_to_wide(s: &[u8]) -> Vec<u16> {
    s.iter().map(|&c| u16::from(c)).collect()
}

fn wide_lossy(s: &[u16]) -> String {
    String::from_utf16_lossy(s)
}

/// `UnicodeString::compareNoCase(ascii) == 0`.
fn eq_no_case(s: &[u16], other: &str) -> bool {
    wide_lossy(s).eq_ignore_ascii_case(other)
}

/// `InitBuddyControls(type)`.
pub fn init_buddy_controls<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, ty: BuddyWindowType) {
    if !h.has_gamespy_info() {
        h.wol_buddy_overlay().buddy_controls = BuddyControls::default();
        return;
    }
    match ty {
        BuddyWindowType::ResetAll => {
            h.wol_buddy_overlay().buddy_controls = BuddyControls::default();
        }
        BuddyWindowType::Buddies | BuddyWindowType::Diplomacy => {
            let prefix: &[u8] = if ty == BuddyWindowType::Buddies { b"WOLBuddyOverlay.wnd" } else { b"Diplomacy.wnd" };
            let name = |tail: &str| [prefix, b":", tail.as_bytes()].concat();
            let text_id = ShellHost::name_to_key(h, &name("TextEntryChat")) as i32;
            let text = wm.win_get_window_from_id(None, text_id);
            let buddies_id = ShellHost::name_to_key(h, &name("ListboxBuddies")) as i32;
            let chat_id = ShellHost::name_to_key(h, &name("ListboxBuddyChat")) as i32;
            let buddies = wm.win_get_window_from_id(None, buddies_id);
            let chat = wm.win_get_window_from_id(None, chat_id);
            te::set_text(wm, h, text, &[]);
            h.wol_buddy_overlay().buddy_controls = BuddyControls {
                listbox_chat: chat,
                listbox_chat_id: chat_id,
                listbox_buddies: buddies,
                listbox_buddies_id: buddies_id,
                text_entry_edit: text,
                text_entry_edit_id: text_id,
                is_init: true,
            };
        }
        BuddyWindowType::WelcomeScreen => {}
    }
}

/// Opens a right-click menu layout (`winCreateLayout`, `getFirstWindow`, `runInit`, `winBringToTop`, `winHide(FALSE)`).
fn open_rc_menu<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, file: &[u8]) -> Option<(usize, WinId)> {
    let rc_layout = ShellHost::create_layout(h, wm, file)?;
    let rc_menu = wm.layouts[rc_layout].windows.first().copied()?; // getFirstWindow()
    wm.layout_run(h, rc_layout, 0); // rcMenu->winGetLayout()->runInit()
    wm.win_bring_to_top(rc_menu);
    wm.win_hide(rc_menu, false);
    Some((rc_layout, rc_menu))
}

/// `BuddyControlSystem(window, msg, mData1, mData2)`.
pub fn buddy_control_system<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    let bc = h.wol_buddy_overlay().buddy_controls.clone();
    if !h.has_gamespy_info() || h.gamespy_local_profile_id() == 0 || !bc.is_init {
        return MsgHandled::Ignored;
    }

    match msg {
        ggm::GLM_RIGHT_CLICKED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;

            if control_id == bc.listbox_buddies_id {
                let (rc_pos, rc_mouse_x, rc_mouse_y) = wm.list_msg.right_click; // RightClickStruct
                if rc_pos < 0 {
                    return MsgHandled::Handled;
                }

                let profile_id = lb::get_item_data(wm, h, control, rc_pos, 0) as i32;
                let item_type = RcItemType::from_i32(lb::get_item_data(wm, h, control, rc_pos, 1) as i32);
                let nick = lb::get_text(wm, h, control, rc_pos, 0);

                lb::set_selected(wm, h, control, &[rc_pos]);
                let file: &[u8] = match item_type {
                    RcItemType::ItemBuddy => b"Menus/RCBuddiesMenu.wnd",
                    RcItemType::ItemRequest => b"Menus/RCBuddyRequestMenu.wnd",
                    _ => b"Menus/RCNonBuddiesMenu.wnd",
                };
                let Some((rc_layout, rc_menu)) = open_rc_menu(wm, h, file) else { return MsgHandled::Handled };
                h.wol_buddy_overlay().rc_menu = Some(rc_menu);

                let rc_size = wm.tree.windows[rc_menu].size;
                let mut rc_pos_x = rc_mouse_x;
                let mut rc_pos_y = rc_mouse_y;
                let (dw, dh) = h.display_size();
                if rc_mouse_x + rc_size.0 > dw {
                    rc_pos_x = dw - rc_size.0;
                }
                if rc_mouse_y + rc_size.1 > dh {
                    rc_pos_y = dh - rc_size.1;
                }
                wm.win_set_position(rc_menu, rc_pos_x, rc_pos_y);

                let rc_data = GameSpyRcMenuData { id: profile_id, nick: nick.iter().map(|&c| c as u8).collect(), item_type };
                set_unignore_text(wm, h, rc_layout, &rc_data.nick, rc_data.id);
                h.win_set_rc_menu_data(wm, rc_menu, rc_data);
                wm.win_set_lone_window(h, Some(rc_menu));
            } else {
                return MsgHandled::Ignored;
            }
        }
        ggm::GEM_EDIT_DONE => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            if control_id != bc.text_entry_edit_id {
                return MsgHandled::Ignored;
            }

            // see if someone's selected
            let selected = match bc.listbox_buddies {
                Some(l) => lb::get_selected(wm, h, l).first().copied().unwrap_or(-1),
                None => -1,
            };
            if selected >= 0 {
                let list = bc.listbox_buddies.expect("selected implies list");
                let selected_profile = lb::get_item_data(wm, h, list, selected, 0) as i32;
                let Some(recip) = h.buddy_map_get(selected_profile) else { return MsgHandled::Handled };

                if h.local_player_dead_in_gamespy_game() {
                    // I'm dead - gotta look for cheats.
                    for i in 0..MAX_SLOTS {
                        if h.game_spy_game_slot_profile_id(i) == selected_profile {
                            // can't send to someone in our game if we're dead/observing.  security breach and all that.  no seances for you.
                            if let Some(chat) = bc.listbox_chat {
                                let t = h.fetch_text(b"Buddy:CantTalkToIngameBuddy");
                                let c = h.gamespy_color_default();
                                lb::add_entry_text(wm, h, chat, &t, c, -1, -1, false);
                            }
                            return MsgHandled::Handled;
                        }
                    }
                }

                // read the user's input and clear the entry box
                let txt = te::get_text(wm, h, bc.text_entry_edit);
                te::set_text(wm, h, bc.text_entry_edit, &[]);
                let txt_input = crate::client::replay_menu::trim(&txt);
                if !txt_input.is_empty() {
                    // Send the message: wcsncpy(text, ..., MAX_BUDDY_CHAT_LEN); text[MAX_BUDDY_CHAT_LEN-1] = 0
                    let mut text = txt_input.clone();
                    text.truncate(MAX_BUDDY_CHAT_LEN - 1);
                    h.buddy_request_message(selected_profile, &text);

                    // save message for future incarnations of the buddy window
                    let message = BuddyMessage {
                        timestamp: h.time_now(),
                        sender_id: h.gamespy_local_profile_id(),
                        sender_nick: h.gamespy_local_base_name(),
                        recipient_id: selected_profile,
                        recipient_nick: recip.name.clone(),
                        message: text,
                    };
                    h.buddy_messages_push(message.clone());

                    // put message on screen
                    insert_chat(wm, h, &message);
                }
            } else {
                // nobody selected.  Prompt the user.
                if let Some(chat) = bc.listbox_chat {
                    let t = h.fetch_text(b"Buddy:SelectBuddyToChat");
                    let c = h.gamespy_color_default();
                    lb::add_entry_text(wm, h, chat, &t, c, -1, -1, false);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// `insertChat(msg)`.
fn insert_chat<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, msg: &BuddyMessage) {
    let Some(chat) = h.wol_buddy_overlay().buddy_controls.listbox_chat else { return };
    let local_sender = msg.sender_id == h.gamespy_local_profile_id();
    let time_str: Vec<u16> = Vec::new(); // the `_wctime` line is commented out upstream
    if local_sender {
        let base = ascii_to_wide(&h.gamespy_local_base_name());
        let mut s = utf16("[");
        s.extend(base);
        s.extend(utf16(" -> "));
        s.extend(ascii_to_wide(&msg.recipient_nick));
        s.extend(utf16("] "));
        s.extend(msg.message.iter().copied());
        let c = h.gamespy_color_player_self();
        let index = lb::add_entry_text(wm, h, chat, &s, c, -1, -1, false);
        lb::add_entry_text(wm, h, chat, &time_str, c, index, 1, false);
    } else if msg.sender_id == 0 {
        let s = msg.message.clone();
        let c = h.gamespy_color_default();
        let index = lb::add_entry_text(wm, h, chat, &s, c, -1, -1, false);
        lb::add_entry_text(wm, h, chat, &time_str, c, index, 1, false);
    } else {
        let mut s = utf16("[");
        s.extend(ascii_to_wide(&msg.sender_nick));
        s.extend(utf16("] "));
        s.extend(msg.message.iter().copied());
        let c = h.gamespy_color_player_buddy();
        let index = lb::add_entry_text(wm, h, chat, &s, c, -1, -1, false);
        lb::add_entry_text(wm, h, chat, &time_str, c, index, 1, false);
    }
}

/// `updateBuddyInfo()` (WOLBuddyOverlay.cpp:312).
pub fn update_buddy_info<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H) {
    let bc = h.wol_buddy_overlay().buddy_controls.clone();
    if !h.buddy_queue_is_connected() {
        if let Some(l) = bc.listbox_buddies {
            lb::reset(wm, h, l);
        }
        return;
    }

    if !bc.is_init {
        return;
    }
    let Some(list) = bc.listbox_buddies else { return };

    let mut selected;
    let mut selected_profile = 0;
    let visible_pos = lb::get_top_visible_entry(wm, list);

    selected = lb::get_selected(wm, h, list).first().copied().unwrap_or(-1);
    if selected >= 0 {
        selected_profile = lb::get_item_data(wm, h, list, selected, 0) as i32;
    }

    selected = -1;
    lb::reset(wm, h, list);

    // Add buddies
    for (profile_id, info) in h.buddy_map() {
        // insert name into box
        let format_str = ascii_to_wide(&info.name);
        let name_color = if h.gamespy_is_saved_ignored(profile_id) { h.gamespy_color_player_ignored() } else { h.gamespy_color_player_buddy() };
        let index = lb::add_entry_text(wm, h, list, &format_str, name_color, -1, -1, false);

        // insert status into box
        let marker = format!("Buddy:{}", wide_lossy(&info.status_string));
        let format_str: Vec<u16>;
        if eq_no_case(&info.status_string, "Offline") || eq_no_case(&info.status_string, "Online") || eq_no_case(&info.status_string, "Matching") {
            format_str = h.fetch_text(marker.as_bytes());
        } else if eq_no_case(&info.status_string, "Staging") || eq_no_case(&info.status_string, "Loading") || eq_no_case(&info.status_string, "Playing") {
            let fmt = h.fetch_text(marker.as_bytes());
            format_str = h.format_text_arg(&fmt, &info.location_string);
        } else if eq_no_case(&info.status_string, "Chatting") {
            let mut room_name: Vec<u16> = Vec::new();
            let room_id = wide_lossy(&info.location_string).trim().parse::<i32>().unwrap_or(0); // _wtoi
            if let Some(name) = h.gamespy_group_room_name(room_id) {
                let s = format!("GUI:{}", String::from_utf8_lossy(&name));
                room_name = h.fetch_text(s.as_bytes());
            }
            let fmt = h.fetch_text(marker.as_bytes());
            format_str = h.format_text_arg(&fmt, &room_name);
        } else {
            format_str = info.status_string.clone();
        }
        let c = h.gamespy_color_default();
        lb::add_entry_text(wm, h, list, &format_str, c, index, 1, false);
        lb::set_item_data(wm, h, list, profile_id as u32, index, 0);
        lb::set_item_data(wm, h, list, RcItemType::ItemBuddy.as_i32() as u32, index, 1);

        if profile_id == selected_profile {
            selected = index;
        }
    }

    // add requests
    for (profile_id, info) in h.buddy_request_map() {
        // insert name into box
        let format_str = ascii_to_wide(&info.name);
        let c = h.gamespy_color_default();
        let index = lb::add_entry_text(wm, h, list, &format_str, c, -1, -1, false);
        lb::set_item_data(wm, h, list, profile_id as u32, index, 0);

        // insert status into box
        let format_str = h.fetch_text(b"GUI:BuddyAddReq");
        lb::add_entry_text(wm, h, list, &format_str, c, index, 1, false);
        lb::set_item_data(wm, h, list, RcItemType::ItemRequest.as_i32() as u32, index, 1);

        if profile_id == selected_profile {
            selected = index;
        }
    }

    // select the same guy
    if selected >= 0 {
        lb::set_selected(wm, h, list, &[selected]);
    }

    // view the same spot
    lb::set_top_visible_entry(wm, h, list, visible_pos);
}

/// `HandleBuddyResponses()` (WOLBuddyOverlay.cpp:420).
pub fn handle_buddy_responses<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H) {
    // (`break` out of the switch continues to the notice check below: a labelled block)
    'handled: {
    if h.buddy_queue_exists() {
        if let Some(resp) = h.buddy_queue_next_response() {
            match resp {
                BuddyResponseKind::Login => {
                    delete_notification_box(wm, h);
                }
                BuddyResponseKind::Disconnect => {
                    {
                        let o = h.wol_buddy_overlay();
                        o.last_notification_was_status = false;
                        o.num_online_in_notification = 0;
                    }
                    let t = h.fetch_text(b"Buddy:MessageDisconnected");
                    show_notification_box(wm, h, &[], t);
                }
                BuddyResponseKind::Message { profile, text, date, nick: resp_nick } => {
                    if text == utf16("I have authorized your request to add me to your list") {
                        break 'handled;
                    }

                    if h.gamespy_is_saved_ignored(profile) {
                        break 'handled; // no buddy messages from ignored people
                    }

                    // save message for future incarnations of the buddy window
                    let mut message = BuddyMessage {
                        timestamp: date,
                        sender_id: profile,
                        recipient_id: h.gamespy_local_profile_id(),
                        recipient_nick: h.gamespy_local_base_name(),
                        message: text,
                        ..Default::default()
                    };
                    // insert status into box
                    let nick = match h.buddy_map_get(message.sender_id) {
                        Some(i) => i.name,
                        None => resp_nick,
                    };
                    message.sender_nick = nick.clone();
                    h.buddy_messages_push(message.clone());

                    // put message on screen
                    insert_chat(wm, h, &message);

                    // play audio notification
                    h.audio_add_event(b"GUIMessageReceived");

                    let mut snippet = message.message.clone();
                    while snippet.len() > 11 {
                        snippet.pop(); // removeLastChar
                    }
                    let fmt = h.fetch_text(b"Buddy:MessageNotification");
                    let s = h.format_nick_text(&fmt, &nick, &snippet);
                    {
                        let o = h.wol_buddy_overlay();
                        o.last_notification_was_status = false;
                        o.num_online_in_notification = 0;
                    }
                    show_notification_box(wm, h, &[], s);
                }
                BuddyResponseKind::Request { profile, country_code, email, nick, text } => {
                    // save request for future incarnations of the buddy window
                    let info = BuddyInfo {
                        country_code,
                        email,
                        name: nick,
                        id: profile,
                        status: GpStatus::Other(0), // (GPEnum)0
                        status_string: text,
                        location_string: Vec::new(),
                    };
                    h.buddy_request_map_set(info.clone());

                    // TODO: put request on screen
                    update_buddy_info(wm, h);
                    // insert status into box
                    {
                        let o = h.wol_buddy_overlay();
                        o.last_notification_was_status = false;
                        o.num_online_in_notification = 0;
                    }
                    let t = h.fetch_text(b"Buddy:AddNotification");
                    show_notification_box(wm, h, &info.name, t);
                }
                BuddyResponseKind::Status { profile, status: new_status, country_code, email, nick, status_string, location } => {
                    let previous = h.buddy_map_get(profile);
                    let seen_previously = previous.is_some();
                    let old_status = previous.map_or(GpStatus::Offline, |p| p.status);
                    let info = BuddyInfo {
                        country_code,
                        email,
                        name: nick,
                        id: profile,
                        status: new_status,
                        status_string: h.multi_byte_to_wide_char(&status_string),
                        location_string: h.multi_byte_to_wide_char(&location),
                    };
                    h.buddy_map_set(info.clone());

                    update_buddy_info(wm, h);
                    h.populate_lobby_player_listbox(wm);
                    h.refresh_game_list_boxes(wm);
                    if (new_status == GpStatus::Offline && seen_previously) || (new_status == GpStatus::Online && (old_status == GpStatus::Offline || !seen_previously)) {
                        // insert status into box
                        let marker = format!("Buddy:{}Notification", wide_lossy(&info.status_string));

                        h.wol_buddy_overlay().last_notification_was_status = true;
                        if new_status != GpStatus::Offline {
                            h.wol_buddy_overlay().num_online_in_notification += 1;
                        }

                        let t = h.fetch_text(marker.as_bytes());
                        show_notification_box(wm, h, &info.name, t);
                    } else if new_status == GpStatus::RecvGameInvite && !seen_previously {
                        h.wol_buddy_overlay().last_notification_was_status = true;
                        if new_status != GpStatus::Offline {
                            h.wol_buddy_overlay().num_online_in_notification += 1;
                        }

                        let t = h.fetch_text(b"Buddy:OnlineNotification");
                        show_notification_box(wm, h, &info.name, t);
                    }
                }
                BuddyResponseKind::Other => {}
            }
        }
    }
    // else: DEBUG_CRASH("No buddy message queue!")
    }
    let (layout, expires) = {
        let o = h.wol_buddy_overlay();
        (o.notice_layout, o.notice_expires)
    };
    if layout.is_some() && ShellHost::time_ms(h) > expires {
        delete_notification_box(wm, h);
    }
}

/// `showNotificationBox(nick, message)`.
pub fn show_notification_box<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, nick: &[u8], mut message: Vec<u16>) {
    // if(!GameSpyIsOverlayOpen(GSOVERLAY_BUDDY)) return;  (commented out upstream)
    if h.wol_buddy_overlay().notice_layout.is_none() {
        let l = ShellHost::create_layout(h, wm, b"Menus/PopupBuddyListNotification.wnd");
        h.wol_buddy_overlay().notice_layout = l;
    }
    let Some(layout) = h.wol_buddy_overlay().notice_layout else { return };
    wm.layout_hide(layout, false);
    if h.wol_buddy_overlay().button_notification_id == 0 {
        let id = ShellHost::name_to_key(h, b"PopupBuddyListNotification.wnd:ButtonNotification") as i32;
        h.wol_buddy_overlay().button_notification_id = id;
    }
    let id = h.wol_buddy_overlay().button_notification_id;
    let Some(win) = wm.win_get_window_from_id(None, id) else {
        delete_notification_box(wm, h);
        return;
    };

    let o = h.wol_buddy_overlay().clone();
    if o.last_notification_was_status && o.num_online_in_notification > 1 {
        message = h.fetch_text(b"Buddy:MultipleOnlineNotification");
    }

    if !nick.is_empty() {
        message = h.format_text_arg(&message, &ascii_to_wide(nick));
    }
    gadget::button_set_text(wm, h, Some(win), &message);
    //GadgetStaticTextSetText(win, message);
    let now = ShellHost::time_ms(h);
    h.wol_buddy_overlay().notice_expires = now.wrapping_add(NOTIFICATION_EXPIRES);
    wm.layout_bring_forward(layout);

    h.audio_add_event(b"GUICommunicatorIncoming");
}

/// `deleteNotificationBox()`.
pub fn delete_notification_box<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H) {
    {
        let o = h.wol_buddy_overlay();
        o.last_notification_was_status = false;
        o.num_online_in_notification = 0;
    }
    if let Some(l) = h.wol_buddy_overlay().notice_layout.take() {
        wm.layout_destroy_windows(l);
        wm.layouts[l].deleted = true; // deleteInstance()
    }
}

/// `PopulateOldBuddyMessages()`.
pub fn populate_old_buddy_messages<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H) {
    // show previous messages
    for message in h.buddy_messages() {
        insert_chat(wm, h, &message);
    }
}

/// `WOLBuddyOverlayInit(layout, userData)` (WOLBuddyOverlay.cpp:557).
pub fn wol_buddy_overlay_init<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let parent_id = key_of(h, "BuddyMenuParent");
    let hide_id = key_of(h, "ButtonHide");
    let add_id = key_of(h, "ButtonAdd");
    let delete_id = key_of(h, "ButtonDelete");
    let accept_id = key_of(h, "ButtonYes");
    let deny_id = key_of(h, "ButtonNo");
    let radio_buddies_id = key_of(h, "RadioButtonBuddies");
    let radio_ignore_id = key_of(h, "RadioButtonIgnore");
    let parent_buddies_id = key_of(h, "BuddiesParent");
    let parent_ignore_id = key_of(h, "IgnoreParent");
    let listbox_ignore_id = key_of(h, "ListboxIgnore");

    let parent = wm.win_get_window_from_id(None, parent_id);
    let find = |wm: &GameWindowManager, id: i32| wm.win_get_window_from_id(parent, id);
    let button_hide = find(wm, hide_id);
    let button_add_buddy = find(wm, add_id);
    let button_delete_buddy = find(wm, delete_id);
    let button_accept_buddy = find(wm, accept_id);
    let button_deny_buddy = find(wm, deny_id);
    let radio_button_buddies = find(wm, radio_buddies_id);
    let radio_button_ignore = find(wm, radio_ignore_id);
    let parent_buddies = find(wm, parent_buddies_id);
    let parent_ignore = find(wm, parent_ignore_id);
    let listbox_ignore = find(wm, listbox_ignore_id);

    {
        let o = h.wol_buddy_overlay();
        o.parent_id = parent_id;
        o.button_hide_id = hide_id;
        o.button_add_buddy_id = add_id;
        o.button_delete_buddy_id = delete_id;
        o.button_accept_buddy_id = accept_id;
        o.button_deny_buddy_id = deny_id;
        o.radio_button_buddies_id = radio_buddies_id;
        o.radio_button_ignore_id = radio_ignore_id;
        o.parent_buddies_id = parent_buddies_id;
        o.parent_ignore_id = parent_ignore_id;
        o.listbox_ignore_id = listbox_ignore_id;
        o.parent = parent;
        o.button_hide = button_hide;
        o.button_add_buddy = button_add_buddy;
        o.button_delete_buddy = button_delete_buddy;
        o.button_accept_buddy = button_accept_buddy;
        o.button_deny_buddy = button_deny_buddy;
        o.radio_button_buddies = radio_button_buddies;
        o.radio_button_ignore = radio_button_ignore;
        o.parent_buddies = parent_buddies;
        o.parent_ignore = parent_ignore;
        o.listbox_ignore = listbox_ignore;
    }

    init_buddy_controls(wm, h, BuddyWindowType::Buddies);

    if let Some(r) = radio_button_buddies {
        gadget::radio_set_selection(wm, h, r, false);
    }
    if let Some(p) = parent_buddies {
        wm.win_hide(p, false);
    }
    if let Some(p) = parent_ignore {
        wm.win_hide(p, true);
    }

    populate_old_buddy_messages(wm, h);

    // Show Menu
    wm.layout_hide(layout, false);

    // Set Keyboard to Main Parent
    wm.win_set_focus(h, parent);

    h.wol_buddy_overlay().is_overlay_active = true;
    update_buddy_info(wm, h);
}

/// `WOLBuddyOverlayShutdown(layout, userData)`.
pub fn wol_buddy_overlay_shutdown<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.wol_buddy_overlay().listbox_ignore = None;

    // hide menu
    wm.layout_hide(layout, true);

    // our shutdown is complete
    //TheShell->shutdownComplete( layout );

    h.wol_buddy_overlay().is_overlay_active = false;

    init_buddy_controls(wm, h, BuddyWindowType::ResetAll);
}

/// `WOLBuddyOverlayUpdate(layout, userData)`.
pub fn wol_buddy_overlay_update<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H) {
    if !h.buddy_queue_exists() || !h.buddy_queue_is_connected() {
        h.gamespy_close_overlay_buddy(wm);
    }
}

/// `WOLBuddyOverlayInput(window, msg, mData1, mData2)`.
pub fn wol_buddy_overlay_input<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        // send a simulated selected event to the parent window of the back/exit button
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            let o = h.wol_buddy_overlay().clone();
            wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(o.button_hide), o.button_hide_id as u32);
        }

        // don't let key fall through anywhere else
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `WOLBuddyOverlaySystem(window, msg, mData1, mData2)`.
pub fn wol_buddy_overlay_system<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if buddy_control_system(wm, h, window, msg, d1, d2) == MsgHandled::Handled {
        return MsgHandled::Handled;
    }
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GLM_RIGHT_CLICKED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let o = h.wol_buddy_overlay().clone();

            if control_id == o.listbox_ignore_id {
                let (rc_pos, rc_mouse_x, rc_mouse_y) = wm.list_msg.right_click; // RightClickStruct
                if rc_pos < 0 {
                    return MsgHandled::Handled;
                }

                let mut is_buddy = false;
                let mut is_request = false;
                let profile_id = lb::get_item_data(wm, h, control, rc_pos, 0) as i32;
                let nick = lb::get_text(wm, h, control, rc_pos, 0);
                if h.buddy_map_get(profile_id).is_some() {
                    is_buddy = true;
                } else if h.buddy_request_map_get(profile_id).is_some() {
                    is_request = true;
                }
                // else: neither buddy nor request

                lb::set_selected(wm, h, control, &[rc_pos]);
                let file: &[u8] = if is_buddy {
                    b"Menus/RCBuddiesMenu.wnd"
                } else if is_request {
                    b"Menus/RCBuddyRequestMenu.wnd"
                } else {
                    b"Menus/RCNonBuddiesMenu.wnd"
                };
                let Some((rc_layout, rc_menu)) = open_rc_menu(wm, h, file) else { return MsgHandled::Handled };
                h.wol_buddy_overlay().rc_menu = Some(rc_menu);

                wm.win_set_position(rc_menu, rc_mouse_x, rc_mouse_y);
                let rc_data = GameSpyRcMenuData {
                    id: profile_id,
                    nick: nick.iter().map(|&c| c as u8).collect(),
                    item_type: if is_buddy { RcItemType::ItemBuddy } else if is_request { RcItemType::ItemRequest } else { RcItemType::ItemNonBuddy },
                };
                set_unignore_text(wm, h, rc_layout, &rc_data.nick, rc_data.id);
                h.win_set_rc_menu_data(wm, rc_menu, rc_data);
                wm.win_set_lone_window(h, Some(rc_menu));
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let o = h.wol_buddy_overlay().clone();

            if control_id == o.button_hide_id {
                h.gamespy_close_overlay_buddy(wm);
            } else if control_id == o.radio_button_buddies_id {
                if let Some(p) = o.parent_buddies {
                    wm.win_hide(p, false);
                }
                if let Some(p) = o.parent_ignore {
                    wm.win_hide(p, true);
                }
            } else if control_id == o.radio_button_ignore_id {
                if let Some(p) = o.parent_buddies {
                    wm.win_hide(p, true);
                }
                if let Some(p) = o.parent_ignore {
                    wm.win_hide(p, false);
                }
                refresh_ignore_list(wm, h);
            } else if control_id == o.button_add_buddy_id || control_id == o.button_delete_buddy_id {
                // bodies are commented out upstream
            }
        }
        ggm::GLM_DOUBLE_CLICKED => {
            // body commented out upstream
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// `PopupBuddyNotificationSystem(window, msg, mData1, mData2)`.
pub fn popup_buddy_notification_system<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            if control_id == h.wol_buddy_overlay().button_notification_id {
                h.gamespy_open_overlay_buddy(wm);
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// `WOLBuddyOverlayRCMenuInit(layout, userData)`: ids are `<layout file name without "Menus/">:Button...`.
pub fn wol_buddy_overlay_rc_menu_init<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let filename = wm.layouts[layout].filename.clone();
    let base = String::from_utf8_lossy(filename.get(6..).unwrap_or(&[])).into_owned(); // getFilename().str()+6
    let add = ShellHost::name_to_key(h, format!("{base}:ButtonAdd").as_bytes()) as i32;
    let delete = ShellHost::name_to_key(h, format!("{base}:ButtonDelete").as_bytes()) as i32;
    let play = ShellHost::name_to_key(h, format!("{base}:ButtonPlay").as_bytes()) as i32;
    let ignore = ShellHost::name_to_key(h, format!("{base}:ButtonIgnore").as_bytes()) as i32;
    let stats = ShellHost::name_to_key(h, format!("{base}:ButtonStats").as_bytes()) as i32;
    let o = h.wol_buddy_overlay();
    o.rc_button_add_id = add;
    o.rc_button_delete_id = delete;
    o.rc_button_play_id = play;
    o.rc_button_ignore_id = ignore;
    o.rc_button_stats_id = stats;
}

/// `closeRightClickMenu(win)`.
fn close_right_click_menu(wm: &mut GameWindowManager, win: WinId) {
    if let Some(l) = wm.ext[win].layout {
        wm.layout_destroy_windows(l);
        wm.layouts[l].deleted = true; // deleteInstance()
    }
}

/// `RequestBuddyAdd(profileID, nick)`.
pub fn request_buddy_add<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, profile_id: i32, nick: &[u8]) {
    // request to add a buddy
    let mut text = h.fetch_text(b"GUI:BuddyAddReq");
    text.truncate(MAX_BUDDY_CHAT_LEN - 1);
    h.buddy_request_add_buddy(profile_id, &text);

    let (s, exists) = h.fetch_text_exists(b"Buddy:InviteSent");
    if !exists {
        // no string yet.  don't display.
        return;
    }

    // save message for future incarnations of the buddy window
    let fmt = h.fetch_text(b"Buddy:InviteSentToPlayer");
    let message = BuddyMessage {
        timestamp: h.time_now(),
        sender_id: 0,
        sender_nick: Vec::new(),
        recipient_id: h.gamespy_local_profile_id(),
        recipient_nick: h.gamespy_local_base_name(),
        message: h.format_text_arg(&fmt, &ascii_to_wide(nick)),
    };

    // insert status into box
    h.buddy_messages_push(message.clone());

    // put message on screen
    insert_chat(wm, h, &message);

    // play audio notification
    h.audio_add_event(b"GUIMessageReceived");

    {
        let o = h.wol_buddy_overlay();
        o.last_notification_was_status = false;
        o.num_online_in_notification = 0;
    }
    show_notification_box(wm, h, &[], s);
}

/// `WOLBuddyOverlayRCMenuSystem(window, msg, mData1, mData2)`.
pub fn wol_buddy_overlay_rc_menu_system<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {}
        gwm::DESTROY => {
            h.wol_buddy_overlay().rc_menu = None;
        }
        ggm::CLOSE => {
            close_right_click_menu(wm, window);
            //rcMenu = NULL;
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.tree.windows[control].id;
            let Some(rc_data) = h.win_take_rc_menu_data(wm, window) else { return MsgHandled::Handled };
            let profile_id = rc_data.id;
            let nick = rc_data.nick.clone();

            let mut is_buddy = false;
            let mut is_request = false;
            let is_game_spy_user = profile_id > 0;
            if rc_data.item_type == RcItemType::ItemBuddy {
                is_buddy = true;
            } else if rc_data.item_type == RcItemType::ItemRequest {
                is_request = true;
            }
            // (rcData deleted and winSetUserData(NULL) by win_take_rc_menu_data)

            let o = h.wol_buddy_overlay().clone();
            if control_id == o.rc_button_add_id {
                if !is_game_spy_user {
                    return MsgHandled::Handled;
                }
                if is_request {
                    // ok the request
                    h.buddy_request_ok_add(profile_id);

                    h.buddy_request_map_erase(profile_id);
                    // if the profile ID is not from a buddy and we're okaying his request, then
                    // request to add him to our list automatically CLH 2-18-03
                    if !h.gamespy_is_buddy(profile_id) {
                        request_buddy_add(wm, h, profile_id, &nick);
                    }
                    update_buddy_info(wm, h);
                } else if !is_buddy {
                    request_buddy_add(wm, h, profile_id, &nick);
                }
            } else if control_id == o.rc_button_delete_id {
                if !is_game_spy_user {
                    return MsgHandled::Handled;
                }
                if is_buddy {
                    // delete the buddy
                    h.buddy_request_del_buddy(profile_id);
                } else {
                    // delete the request
                    h.buddy_request_deny_add(profile_id);
                    h.buddy_request_map_erase(profile_id);
                }
                if is_buddy {
                    h.buddy_map_erase(profile_id);
                } else {
                    h.buddy_request_map_erase(profile_id);
                }
                update_buddy_info(wm, h);
                h.populate_lobby_player_listbox(wm);
            } else if control_id == o.rc_button_play_id {
                // buttonPlayID was pushed: nothing
            } else if control_id == o.rc_button_ignore_id {
                if is_game_spy_user {
                    if h.gamespy_is_saved_ignored(profile_id) {
                        h.gamespy_remove_from_saved_ignore_list(profile_id);
                    } else {
                        h.gamespy_add_to_saved_ignore_list(profile_id, &nick);
                    }
                } else if h.gamespy_is_ignored(&nick) {
                    h.gamespy_remove_from_ignore_list(&nick);
                } else {
                    h.gamespy_add_to_ignore_list(&nick);
                }
                update_buddy_info(wm, h);
                refresh_ignore_list(wm, h);
                // repopulate our player listboxes now
                h.populate_lobby_player_listbox(wm);
            } else if control_id == o.rc_button_stats_id {
                h.gamespy_close_overlay_player_info(wm);
                h.set_look_at_player(profile_id, &nick);
                h.gamespy_open_overlay_player_info(wm);
                h.ps_request_read_player_stats(profile_id);
            }
            close_right_click_menu(wm, window);
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// `setUnignoreText(layout, nick, id)`.
pub fn set_unignore_text<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, nick: &[u8], id: i32) {
    let filename = wm.layouts[layout].filename.clone();
    let base = String::from_utf8_lossy(filename.get(6..).unwrap_or(&[])).into_owned(); // getFilename().str()+6
    let key_id = ShellHost::name_to_key(h, format!("{base}:ButtonIgnore").as_bytes()) as i32;
    let first = wm.layouts[layout].windows.first().copied(); // getFirstWindow()
    if let Some(win) = wm.win_get_window_from_id(first, key_id) {
        if h.gamespy_is_saved_ignored(id) || h.gamespy_is_ignored(nick) {
            let t = h.fetch_text(b"GUI:Unignore");
            gadget::button_set_text(wm, h, Some(win), &t);
        }
    }
}

/// `refreshIgnoreList()`.
pub fn refresh_ignore_list<H: WolBuddyOverlayHost>(wm: &mut GameWindowManager, h: &mut H) {
    let Some(list) = h.wol_buddy_overlay().listbox_ignore else { return };
    let color = wt::game_make_color(255, 100, 100, 255);

    let temp_map = h.gamespy_saved_ignore_list();
    lb::reset(wm, h, list);
    for (id, nick) in temp_map {
        let name = ascii_to_wide(&nick);
        let pos = lb::add_entry_text(wm, h, list, &name, color, -1, -1, false);
        lb::set_item_data(wm, h, list, id as u32, pos, 0);
    }
    for a_name in h.gamespy_ignore_list() {
        let name = ascii_to_wide(&a_name);
        let pos = lb::add_entry_text(wm, h, list, &name, color, -1, -1, false);
        lb::set_item_data(wm, h, list, 0, pos, 0);
    }
}
