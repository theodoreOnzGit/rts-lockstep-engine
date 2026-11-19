//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 4,000 programs, the colour table and the field table (after a draft fix, 2026-11-19: `add_chat` widens a player name signed, as `UnicodeString::translate` does) (test: tests/differential_draft_chat.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Chat.cpp (GameSpyColor and its `OnlineChatColorDefinition`
//!   field table, GameSpyInfo::sendChat, both addChat, addText, registerTextWindow, unregisterTextWindow)
//!
//! Not `GameSpyChat.cpp`: that file is excluded from the upstream build and is drafted in
//! `crate::drafts::network::game_spy_chat`. This is the chat the game uses. `GameSpyColors` is
//! `crate::drafts::network::peer_defs::GameSpyColors`.

use crate::common::ini::game_make_color;
use crate::drafts::network::peer_defs::GameSpyColors;
use crate::logic::modules::plant;

/// `GSCOLOR_MAX`.
pub const GSCOLOR_MAX: usize = 27;

/// `Color GameSpyColor[GSCOLOR_MAX]` (Chat.cpp:107), the defaults `ChatColors.ini`'s `OnlineChatColorDefinition` may replace.
pub fn game_spy_color_defaults() -> [i32; GSCOLOR_MAX] {
    [
        game_make_color(255, 255, 255, 255), // GSCOLOR_DEFAULT
        game_make_color(255, 255, 0, 255),   // GSCOLOR_CURRENTROOM
        game_make_color(255, 255, 255, 255), // GSCOLOR_ROOM
        game_make_color(128, 128, if plant(15776) { 128 } else { 0 }, 255), // GSCOLOR_GAME
        game_make_color(128, 128, 128, 255), // GSCOLOR_GAME_FULL
        game_make_color(128, 128, 128, 255), // GSCOLOR_GAME_CRCMISMATCH
        game_make_color(255, 255, 255, 255), // GSCOLOR_PLAYER_NORMAL
        game_make_color(255, 0, 255, 255),   // GSCOLOR_PLAYER_OWNER
        game_make_color(255, 0, 128, 255),   // GSCOLOR_PLAYER_BUDDY
        game_make_color(255, 0, 0, 255),     // GSCOLOR_PLAYER_SELF
        game_make_color(128, 128, 128, 255), // GSCOLOR_PLAYER_IGNORED
        game_make_color(255, 255, 255, 255), // GSCOLOR_CHAT_NORMAL
        game_make_color(255, 128, 0, 255),   // GSCOLOR_CHAT_EMOTE
        game_make_color(255, 255, 0, 255),   // GSCOLOR_CHAT_OWNER
        game_make_color(128, 255, 0, 255),   // GSCOLOR_CHAT_OWNER_EMOTE
        game_make_color(0, 0, 255, 255),     // GSCOLOR_CHAT_PRIVATE
        game_make_color(0, 255, 255, 255),   // GSCOLOR_CHAT_PRIVATE_EMOTE
        game_make_color(255, 0, 255, 255),   // GSCOLOR_CHAT_PRIVATE_OWNER
        game_make_color(255, 128, 255, 255), // GSCOLOR_CHAT_PRIVATE_OWNER_EMOTE
        game_make_color(255, 0, 255, 255),   // GSCOLOR_CHAT_BUDDY
        game_make_color(255, 0, 128, 255),   // GSCOLOR_CHAT_SELF
        game_make_color(0, 255, 0, 255),     // GSCOLOR_ACCEPT_TRUE
        game_make_color(255, 0, 0, 255),     // GSCOLOR_ACCEPT_FALSE
        game_make_color(255, 255, 0, 255),   // GSCOLOR_MAP_SELECTED
        game_make_color(255, 255, 255, 255), // GSCOLOR_MAP_UNSELECTED
        game_make_color(255, 255, 255, 255), // GSCOLOR_MOTD
        game_make_color(255, 255, 0, 255),   // GSCOLOR_MOTD_HEADING
    ]
}

/// `GameSpyColorFieldParse` (Chat.cpp:50): the field names of `OnlineChatColorDefinition`, in the table's order, with
/// the colour each sets (`INI::parseColorInt` at `OFFSET(GSCOLOR_x)`).
pub const GAME_SPY_COLOR_FIELDS: [(&str, GameSpyColors); GSCOLOR_MAX] = [
    ("Default", GameSpyColors::Default),
    ("CurrentRoom", GameSpyColors::CurrentRoom),
    ("ChatRoom", GameSpyColors::Room),
    ("Game", GameSpyColors::Game),
    ("GameFull", GameSpyColors::GameFull),
    ("GameCRCMismatch", GameSpyColors::GameCrcMismatch),
    ("PlayerNormal", GameSpyColors::PlayerNormal),
    ("PlayerOwner", GameSpyColors::PlayerOwner),
    ("PlayerBuddy", GameSpyColors::PlayerBuddy),
    ("PlayerSelf", GameSpyColors::PlayerSelf),
    ("PlayerIgnored", GameSpyColors::PlayerIgnored),
    ("ChatNormal", GameSpyColors::ChatNormal),
    ("ChatEmote", GameSpyColors::ChatEmote),
    ("ChatOwner", GameSpyColors::ChatOwner),
    ("ChatOwnerEmote", GameSpyColors::ChatOwnerEmote),
    ("ChatPriv", GameSpyColors::ChatPrivate),
    ("ChatPrivEmote", GameSpyColors::ChatPrivateEmote),
    ("ChatPrivOwner", GameSpyColors::ChatPrivateOwner),
    ("ChatPrivOwnerEmote", GameSpyColors::ChatPrivateOwnerEmote),
    ("ChatBuddy", GameSpyColors::ChatBuddy),
    ("ChatSelf", GameSpyColors::ChatSelf),
    ("AcceptTrue", GameSpyColors::AcceptTrue),
    ("AcceptFalse", GameSpyColors::AcceptFalse),
    ("MapSelected", GameSpyColors::MapSelected),
    ("MapUnselected", GameSpyColors::MapUnselected),
    ("MOTD", GameSpyColors::Motd),
    ("MOTDHeading", GameSpyColors::MotdHeading),
];

/// What the chat asks of `TheGameSpyInfo` and the GUI.
pub trait GameSpyChatHost {
    /// `getCurrentGroupRoom()` is non-zero.
    fn in_group_room(&self) -> bool;
    /// `TheGameSpyPeerMessageQueue->addRequest(req)`: `PEERREQUEST_MESSAGEROOM` with `text` and `isAction`.
    fn queue_message_room(&mut self, text: &[u16], is_action: bool);
    /// `PEERREQUEST_MESSAGEPLAYER`: to the comma separated `nicks`.
    fn queue_message_player(&mut self, text: &[u16], nicks: &[u8], is_action: bool);
    /// `TheGameSpyInfo->getLocalName()`.
    fn local_name(&self) -> Vec<u8>;
    /// `GadgetListBoxGetListLength(playerListbox)`.
    fn listbox_length(&self, listbox: u32) -> i32;
    /// `GadgetListBoxGetSelected(playerListbox, &selections)`: the selected rows, `-1` ending (and when none).
    fn listbox_selected(&self, listbox: u32) -> Vec<i32>;
    /// `GadgetListBoxGetNumColumns(playerListbox)`.
    fn listbox_num_columns(&self, listbox: u32) -> i32;
    /// `GadgetListBoxGetText(listbox, row, column)`, translated to ASCII.
    fn listbox_text_ascii(&self, listbox: u32, row: i32, column: i32) -> Vec<u8>;
    /// `GadgetListBoxAddEntryText(win, message, color, -1, -1)`: the row index.
    fn listbox_add_entry_text(&mut self, win: u32, message: &[u16], color: i32) -> i32;
    /// `GadgetListBoxSetItemData(win, (void *)data, index)`.
    fn listbox_set_item_data(&mut self, win: u32, data: i32, index: i32);
    /// `isSavedIgnored(profileID)`.
    fn is_saved_ignored(&self, profile_id: i32) -> bool;
    /// `isIgnored(name)`.
    fn is_ignored(&self, name: &[u8]) -> bool;
    /// `getBuddyMap()->find(profileID) != end`.
    fn is_buddy(&self, profile_id: i32) -> bool;
    /// `m_disallowAsainText`, `m_disallowNonAsianText`.
    fn disallow_asian_text(&self) -> bool;
    fn disallow_non_asian_text(&self) -> bool;
    /// `TheAudio->addAudioEvent(AudioEventRTS(name))` (when `TheAudio` exists).
    fn audio_add_event(&mut self, name: &[u8]);
    /// `TheLanguageFilter->filterLine(msg)`.
    fn language_filter_line(&self, msg: &mut Vec<u16>);
    /// `TheGameSpyGame && isInGame() && isGameInProgress()`.
    fn game_in_progress(&self) -> bool;
    /// `TheInGameUI->message(message)`.
    fn in_game_ui_message(&mut self, message: &[u16]);
    /// `m_textWindows` (a `std::set<GameWindow *>`, ordered by pointer): the first, `None` when empty.
    fn first_text_window(&self) -> Option<u32>;
    /// `m_textWindows.insert(win)` / `erase(win)`.
    fn text_windows_insert(&mut self, win: u32);
    fn text_windows_erase(&mut self, win: u32);
    /// `GameSpyColor[style]`.
    fn game_spy_color(&self, style: GameSpyColors) -> i32;
    /// `PEER_FLAG_OP` is set in the player's flags.
    fn player_is_op(&self, flags: i32) -> bool;
}

/// A `PlayerInfo`'s fields that `addChat` reads.
#[derive(Clone, Debug, Default)]
pub struct ChatPlayer {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_profileID`.
    pub profile_id: i32,
    /// `m_flags`.
    pub flags: i32,
}

/// `GameSpyInfo::sendChat(message, isAction, playerListbox)` (Chat.cpp:132): a public message goes to the room unless it
/// repeats the previous (an action always goes); with players selected in the list box, a private message goes to them
/// (and the local player). Returns true when the message was private. `s_prev_msg` is the function's `static
/// UnicodeString s_prevMsg` ("stop spam before it happens").
pub fn send_chat<H: GameSpyChatHost>(
    h: &mut H,
    s_prev_msg: &mut Vec<u16>,
    message: &[u16],
    is_action: bool,
    player_listbox: Option<u32>,
) -> bool {
    // RoomType roomType = StagingRoom; if (getCurrentGroupRoom()) roomType = GroupRoom; (unused afterwards)
    let _ = h.in_group_room();
    // req.text = message.str(); message.trim();
    let text: Vec<u16> = message.to_vec();
    let trimmed = trim_wide(message);
    if !trimmed.is_empty() {
        match player_listbox {
            None => {
                // Public message
                if is_action || (trimmed != *s_prev_msg) == !plant(15765) {
                    // don't send duplicate messages
                    h.queue_message_room(&text, is_action);
                    if !plant(15767) {
                        *s_prev_msg = trimmed;
                    }
                }
                return false;
            }
            Some(listbox) => {
                let max_sel = h.listbox_length(listbox);
                let selections = h.listbox_selected(listbox);
                if selections.first().copied().unwrap_or(-1) == -1 {
                    // Public message
                    if is_action || trimmed != *s_prev_msg {
                        h.queue_message_room(&text, is_action);
                        *s_prev_msg = trimmed;
                    }
                    return false;
                }
                let local = h.local_name();
                let mut names: Vec<u8> = local.clone();
                for i in 0..(max_sel.max(0) as usize) {
                    let sel = selections.get(i).copied().unwrap_or(-1);
                    if sel != -1 {
                        let col = h.listbox_num_columns(listbox) - 1;
                        let a_str = h.listbox_text_ascii(listbox, sel, col);
                        // if (aStr.compareNoCase(getLocalName())): non-zero, i.e. different
                        if (if plant(15766) { a_str != local } else { !a_str.eq_ignore_ascii_case(&local) }) {
                            names.push(b',');
                            names.extend_from_slice(&a_str);
                        }
                    } else {
                        break;
                    }
                }
                if !names.is_empty() {
                    h.queue_message_player(&text, &names, is_action);
                }
                *s_prev_msg = trimmed;
                return true;
            }
        }
    }
    *s_prev_msg = trimmed;
    false
}

/// `UnicodeString::trim()`: removes white space from both ends.
fn trim_wide(s: &[u16]) -> Vec<u16> {
    let is_space = |c: &u16| *c == 0x20 || (0x09..=0x0d).contains(c);
    let start = s.iter().position(|c| !is_space(c)).unwrap_or(s.len());
    let end = s.iter().rposition(|c| !is_space(c)).map_or(start, |i| i + 1);
    s[start..end].to_vec()
}

/// `GameSpyInfo::addChat(nick, profileID, msg, isPublic, isAction, win)` (Chat.cpp:215): the player info map lookup by
/// nick; an unknown nick does nothing (the `else` is empty). `player` is the map's entry.
pub fn add_chat_by_nick<H: GameSpyChatHost>(
    h: &mut H,
    player: Option<&ChatPlayer>,
    msg: &[u16],
    is_public: bool,
    is_action: bool,
    win: Option<u32>,
) {
    if let Some(p) = player {
        add_chat(h, p, msg, is_public, is_action, win);
    }
}

/// `GameSpyInfo::addChat(p, msg, isPublic, isAction, win)` (Chat.cpp:225): a saved or session ignore drops the message;
/// another player's text is dropped if it is outside (or, with `m_disallowNonAsianText`, inside) Latin-1 by the
/// settings; a private message plays `GUIMessageReceived`; the colour is chosen buddy first, then public/private,
/// action and owner; the line is filtered and shown as `name text` (an action) or `[name] text`, and its list box item
/// data is the profile id.
pub fn add_chat<H: GameSpyChatHost>(h: &mut H, p: &ChatPlayer, msg: &[u16], is_public: bool, is_action: bool, win: Option<u32>) {
    if h.is_saved_ignored(p.profile_id) || h.is_ignored(&p.name) {
        return;
    }
    let is_owner = h.player_is_op(p.flags);
    let is_buddy = h.is_buddy(p.profile_id);
    let is_me = if plant(15768) { p.name.eq_ignore_ascii_case(&h.local_name()) } else { p.name == h.local_name() };
    if !is_me {
        if h.disallow_asian_text() {
            if msg.iter().any(|&c| if plant(15769) { c > 256 } else { c >= 256 }) {
                return;
            }
        } else if h.disallow_non_asian_text() {
            let has_unicode = msg.iter().any(|&c| c >= 256);
            if !has_unicode {
                return;
            }
        }
        if !is_public || plant(15771) {
            h.audio_add_event(b"GUIMessageReceived");
        }
    }
    let style = if plant(15770) && is_public && is_action {
        if is_owner { GameSpyColors::ChatOwnerEmote } else { GameSpyColors::ChatEmote }
    } else if is_buddy {
        GameSpyColors::ChatBuddy
    } else if is_public && is_action {
        if is_owner { GameSpyColors::ChatOwnerEmote } else { GameSpyColors::ChatEmote }
    } else if is_public {
        if is_owner { GameSpyColors::ChatOwner } else { GameSpyColors::ChatNormal }
    } else if is_action {
        if is_owner { GameSpyColors::ChatPrivateOwnerEmote } else { GameSpyColors::ChatPrivateEmote }
    } else if is_owner {
        GameSpyColors::ChatPrivateOwner
    } else {
        GameSpyColors::ChatPrivate
    };
    // name.translate(p.m_name): `UnicodeString::translate` widens each `char` as `(WideChar)stringSrc.getCharAt(i)`, and `char` is
    // signed (MSVC's default): a byte of 0x80 or more becomes 0xFF80..0xFFFF. ~~Zero extended (`c as u16`)~~ **Corrected 2026-11-19
    // (differential, #215, tests/differential_draft_chat.rs):** sign extended; the draft mismatched upstream in 739 of 4,000 programs.
    let name: Vec<u16> = p.name.iter().map(|&c| c as i8 as i16 as u16).collect();
    let mut msg = msg.to_vec();
    if !plant(15775) {
        h.language_filter_line(&mut msg);
    }
    let mut full_msg: Vec<u16> = Vec::new();
    if is_action {
        // L"%ls %ls"
        full_msg.extend_from_slice(&name);
        full_msg.push(b' ' as u16);
        full_msg.extend_from_slice(&msg);
    } else {
        // L"[%ls] %ls"
        full_msg.push(b'[' as u16);
        full_msg.extend_from_slice(&name);
        full_msg.push(b']' as u16);
        full_msg.push(b' ' as u16);
        full_msg.extend_from_slice(&msg);
    }
    if plant(15775) {
        h.language_filter_line(&mut full_msg);
    }
    let color = h.game_spy_color(style);
    let index = add_text(h, &full_msg, color, win);
    if index >= 0 {
        if let Some(w) = win {
            h.listbox_set_item_data(w, p.profile_id, index);
        }
    }
}

/// `GameSpyInfo::addText(message, c, win)` (Chat.cpp:314): a message during a game in progress plays
/// `GUIMessageReceived` and shows in the in-game UI; it is added to `win`, or to the first registered text window;
/// `-1` when there is none. The new row's item data is `-1`.
pub fn add_text<H: GameSpyChatHost>(h: &mut H, message: &[u16], c: i32, win: Option<u32>) -> i32 {
    if h.game_in_progress() || plant(15773) {
        h.audio_add_event(b"GUIMessageReceived");
        h.in_game_ui_message(message);
    }
    let win = match win {
        Some(w) => w,
        None => match if plant(15774) { None } else { h.first_text_window() } {
            Some(w) => w,
            None => return -1,
        },
    };
    let index = h.listbox_add_entry_text(win, message, c);
    if !plant(15772) {
        h.listbox_set_item_data(win, -1, index);
    }
    index
}

/// `GameSpyInfo::registerTextWindow(win)` (Chat.cpp:339).
pub fn register_text_window<H: GameSpyChatHost>(h: &mut H, win: u32) {
    h.text_windows_insert(win);
}

/// `GameSpyInfo::unregisterTextWindow(win)` (Chat.cpp:344).
pub fn unregister_text_window<H: GameSpyChatHost>(h: &mut H, win: u32) {
    h.text_windows_erase(win);
}
