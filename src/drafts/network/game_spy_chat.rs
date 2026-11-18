//! UNTESTED DRAFT (#202, #207, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpyChat.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpyChat.cpp
//!
//! # EXCLUDED FROM THE UPSTREAM BUILD
//!
//! `GameEngine.dsp` marks `GameSpyChat.cpp` `Exclude_From_Build` in every configuration of Zero Hour, and the file
//! cannot compile there: it includes `GameNetwork/GameSpy.h` (only in the original Generals tree) and uses
//! `TheGameSpyChat`, which no Zero Hour header declares. The live chat code is `GameSpyInfo::sendChat` /
//! `addChat` in `GameSpy/Chat.cpp` (not in this draft's list). It is drafted here because it is on the list, as a
//! record of the old design: the room/private send path, the style choice per message kind and the text window
//! fallback.
//!
//! `handleSlashCommands` has its whole body inside a `/* ... */` comment upstream and always returns false.
//! `handleUnicodeMessage` is declared `static handleUnicodeMessage(...)` (no return type, implicit int in C) and
//! has no return statement; here it returns nothing.

use super::peer_defs::GameSpyColors;

/// The Peer SDK calls the chat send path makes (`TheGameSpyChat->getPeer()`).
pub trait ChatHost {
    /// `TheGameSpyChat->getCurrentGroupRoomID() != 0`.
    fn in_group_room(&self) -> bool;
    /// `TheGameSpyChat->getLoginName()`.
    fn login_name(&self) -> Vec<u8>;
    /// `UnicodeStringToQuotedPrintable(message)`.
    fn to_quoted_printable(&self, message: &[u16]) -> Vec<u8>;
    /// `QuotedPrintableToUnicodeString(message)`.
    fn from_quoted_printable(&self, message: &[u8]) -> Vec<u16>;
    /// `peerMessageRoom(peer, room, text, ActionMessage/NormalMessage)`; `group_room` picks GroupRoom over
    /// StagingRoom.
    fn peer_message_room(&mut self, group_room: bool, text: &[u8], is_action: bool);
    /// `peerMessagePlayer(peer, "a,b,c", text, ActionMessage/NormalMessage)`.
    fn peer_message_player(&mut self, names: &[u8], text: &[u8], is_action: bool);
    /// `peerGetPlayerFlags(peer, nick, room, &flags); flags & PEER_FLAG_OP`.
    fn player_is_owner(&mut self, nick: &[u8], group_room: bool) -> bool;
    /// `TheLanguageFilter->filterLine(msg)`.
    fn language_filter_line(&self, msg: &mut Vec<u16>);
    /// `GadgetListBoxAddEntryText(window, message, GameSpyColor[color], -1, -1)` on the lobby chat box, else the
    /// game setup chat box; false if neither exists (nothing is added).
    fn add_chat_text(&mut self, message: &[u16], color: GameSpyColors) -> bool;
}

/// The player list box selection `GameSpySendChat` reads.
#[derive(Clone, Debug, Default)]
pub struct PlayerListSelection {
    /// `GadgetListBoxGetSelected`: selected row indices, -1 terminated.
    pub selections: Vec<i32>,
    /// `GadgetListBoxGetListLength`.
    pub length: i32,
    /// `GadgetListBoxGetText(listbox, row, 0)` as ASCII, for each selected row.
    pub names: Vec<Vec<u8>>,
}

/// `handleSlashCommands`: the body is commented out upstream, so no message is ever treated as a command.
pub fn handle_slash_commands(_message: &[u16], _is_action: bool) -> bool {
    false
}

/// `GameSpySendChat(message, isAction, playerListbox)`: trims the message; empty does nothing. Without a list box or
/// with no selection it is a public message to the current room (group room if in one, else staging) as
/// quoted-printable text. With selections it is a private message to "<me>,<selected>,..." (skipping myself,
/// stopping at the first -1) and returns true. Returns false otherwise (and when a slash command ate it).
pub fn game_spy_send_chat<H: ChatHost>(
    host: &mut H,
    message: &[u16],
    is_action: bool,
    player_listbox: Option<&PlayerListSelection>,
) -> bool {
    let group_room = host.in_group_room();
    // message.trim(): leading/trailing whitespace
    let is_ws = |c: u16| c == b' ' as u16 || (9..=13).contains(&c);
    let mut start = 0;
    let mut end = message.len();
    while start < end && is_ws(message[start]) {
        start += 1;
    }
    while end > start && is_ws(message[end - 1]) {
        end -= 1;
    }
    let message = &message[start..end];

    if message.is_empty() {
        return false;
    }
    // Check for slash commands
    if handle_slash_commands(message, is_action) {
        return false; // already handled
    }

    let list = match player_listbox {
        None => {
            // Public message
            let qp = host.to_quoted_printable(message);
            host.peer_message_room(group_room, &qp, is_action);
            return false;
        }
        Some(l) => l,
    };

    if list.selections.first().copied().unwrap_or(-1) == -1 {
        // Public message
        let qp = host.to_quoted_printable(message);
        host.peer_message_room(group_room, &qp, is_action);
        return false;
    }

    // Private message: construct a list
    let login = host.login_name();
    let mut names: Vec<u8> = login.clone();
    for i in 0..list.length.max(0) as usize {
        match list.selections.get(i).copied() {
            Some(sel) if sel != -1 => {
                let a_str = list.names.get(i).cloned().unwrap_or_default();
                if !a_str.eq_ignore_ascii_case(&login) {
                    names.push(b',');
                    names.extend_from_slice(&a_str);
                }
            }
            _ => break,
        }
    }

    if !names.is_empty() {
        let qp = host.to_quoted_printable(message);
        host.peer_message_player(&names, &qp, is_action);
    }
    true
}

/// `handleUnicodeMessage(nick, msg, isPublic, isAction)`: picks the colour from public/private, action/normal and
/// whether the sender is the room owner (op flag in the current room), runs the language filter and adds either
/// "<name> <msg>" (action) or "[<name>] <msg>".
pub fn handle_unicode_message<H: ChatHost>(host: &mut H, nick: &[u8], mut msg: Vec<u16>, is_public: bool, is_action: bool) {
    let group_room = host.in_group_room();
    let is_owner = host.player_is_owner(nick, group_room);

    let style = if is_public && is_action {
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

    let name: Vec<u16> = nick.iter().map(|&c| c as u16).collect();

    // filters language
    host.language_filter_line(&mut msg);

    let mut full_msg: Vec<u16> = Vec::new();
    if is_action {
        full_msg.extend_from_slice(&name);
        full_msg.push(b' ' as u16);
        full_msg.extend_from_slice(&msg);
    } else {
        full_msg.push(b'[' as u16);
        full_msg.extend_from_slice(&name);
        full_msg.push(b']' as u16);
        full_msg.push(b' ' as u16);
        full_msg.extend_from_slice(&msg);
    }
    host.add_chat_text(&full_msg, style);
}

/// `RoomMessageCallback`: a room message is public.
pub fn room_message_callback<H: ChatHost>(host: &mut H, nick: &[u8], message: &[u8], is_action: bool) {
    let msg = host.from_quoted_printable(message);
    handle_unicode_message(host, nick, msg, true, is_action);
}

/// `PlayerMessageCallback`: a player message is private.
pub fn player_message_callback<H: ChatHost>(host: &mut H, nick: &[u8], message: &[u8], is_action: bool) {
    let msg = host.from_quoted_printable(message);
    handle_unicode_message(host, nick, msg, false, is_action);
}

/// `GameSpyAddText(message, color)`: the lobby chat box if it exists, else the game setup chat box; nothing if
/// neither.
pub fn game_spy_add_text<H: ChatHost>(host: &mut H, message: &[u16], color: GameSpyColors) {
    host.add_chat_text(message, color);
}
