//! UNTESTED DRAFT (#202, #207): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANAPIhandlers.cpp
//!
//! The handlers for received [`LANMessage`]s: `impl LANAPI` continued. Each is called by `LANAPI::update` with the
//! message and the sender's address.

use crate::common::crc::Crc;
use crate::common::game_info::{
    game_info_to_ascii_string, parse_ascii_string_to_game_info, slot_state, GameSlot, PLAYERTEMPLATE_OBSERVER,
};
use crate::common::unicode_string::UnicodeString;

use super::lan_api::{lan_message_type, ChatType, LANAPI, LanApiHost, LANMessage, PendingActionType, ReturnType};
use super::lan_api::{G_LAN_PLAYER_NAME_LENGTH, G_MAX_SERIAL_LENGTH};
use super::lan_game_info::{parse_game_options_string, LANGameInfo, LANPlayer, LANSlotExtra};
use super::net_command_msg::{MAX_SLOTS, NETWORK_BASE_PORT_NUMBER};

const SLOTS: usize = MAX_SLOTS as usize;

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn widen(b: &[u8]) -> UnicodeString {
    let v: Vec<u16> = b.iter().map(|&c| c as u16).collect();
    UnicodeString::try_from_chars(&v).unwrap_or_default()
}

impl<H: LanApiHost> LANAPI<H> {
    /// Takes the player at `senderIP` out of the list or makes a new one, fills it from the message and puts it back
    /// (the body shared by `handleRequestLocations` and `handleLobbyAnnounce`), then reports the name.
    fn refresh_lobby_player(&mut self, msg: &LANMessage, sender_ip: u32) {
        let mut player = match self.lookup_player(sender_ip) {
            Some(i) => self.lobby_players.remove(i),
            None => {
                let mut p = LANPlayer::new();
                p.set_ip(sender_ip);
                p
            }
        };
        player.set_name(msg.name());
        player.set_host(widen(&msg.host_name()));
        player.set_login(widen(&msg.user_name()));
        player.set_last_heard(self.host.time_get_time());
        let (ip, name) = (player.get_ip(), player.get_name());
        self.add_player(player);
        self.host.on_name_change(ip, &name);
    }

    /// `handleRequestLocations` (LANAPIhandlers.cpp): "where is everybody?": answer with the lobby or (as host)
    /// the game announcement, and note the asker as a lobby player.
    pub fn handle_request_locations(&mut self, msg: &LANMessage, sender_ip: u32) {
        if self.in_lobby {
            let mut reply = LANMessage::new();
            self.fill_in_lan_message(&mut reply);
            reply.set_message_type(lan_message_type::LOBBY_ANNOUNCE);
            self.send_message(&reply, 0);
            self.last_resend_time = self.host.time_get_time();
        } else if let Some(g) = self.current_game {
            if self.games[g].get_ip(0) == self.local_ip {
                let mut reply = LANMessage::new();
                self.fill_in_lan_message(&mut reply);
                reply.set_message_type(lan_message_type::GAME_ANNOUNCE);
                let game_opts = self.generate_options();
                reply.set_game_info_options(&game_opts);
                let n = self.games[g].get_name();
                reply.set_game_name(n.str());
                reply.set_game_info_in_progress(self.games[g].info.in_progress);
                self.send_message(&reply, 0);
            }
        }
        self.refresh_lobby_player(msg, sender_ip);
    }

    /// The "find or make the announced game, parse its options" body of `handleGameAnnounce`. Returns the game's
    /// index, or None if its options did not parse (the game is then removed).
    fn announced_game(&mut self, msg: &LANMessage) -> Option<usize> {
        let name = msg.game_name();
        let idx = match self.lookup_game(&name) {
            Some(i) => i,
            None => {
                let mut game = LANGameInfo::new(&mut self.host);
                game.set_name(name);
                self.add_game(game)
            }
        };
        let options = msg.game_info_options();
        let in_progress = msg.game_info_in_progress();
        let direct = msg.game_info_is_direct_connect();
        let now = self.host.time_get_time();
        let success = parse_game_options_string(Some(&mut self.games[idx]), &options, &mut self.host);
        self.games[idx].info.in_progress = in_progress;
        self.games[idx].set_is_direct_connect(direct);
        self.games[idx].set_last_heard(now);
        if !success {
            self.remove_game(idx);
            return None;
        }
        Some(idx)
    }

    /// `handleGameAnnounce` (LANAPIhandlers.cpp): a game's announcement: ignored from myself or while I play;
    /// from the host I am direct-connecting to it triggers the join; otherwise it updates the game list.
    pub fn handle_game_announce(&mut self, msg: &LANMessage, sender_ip: u32) {
        if sender_ip == self.local_ip {
            return; // don't try to update own info
        } else if self.current_game.map_or(false, |g| self.games[g].info.in_progress) {
            return; // don't care about games if we're playing
        } else if sender_ip == self.direct_connect_remote_ip {
            if self.current_game.is_none() {
                if let Some(idx) = self.announced_game(msg) {
                    let remote = self.direct_connect_remote_ip;
                    self.request_game_join(Some(idx), remote);
                }
            }
        } else {
            let _ = self.announced_game(msg);
            let gl = self.games.clone();
            self.host.on_game_list(&gl);
        }
    }

    /// `handleLobbyAnnounce` (LANAPIhandlers.cpp).
    pub fn handle_lobby_announce(&mut self, msg: &LANMessage, sender_ip: u32) {
        self.refresh_lobby_player(msg, sender_ip);
    }

    /// `handleRequestGameInfo` (LANAPIhandlers.cpp): a direct-connect asker gets the game announcement.
    pub fn handle_request_game_info(&mut self, _msg: &LANMessage, sender_ip: u32) {
        if let Some(g) = self.current_game {
            if self.games[g].get_ip(0) == self.local_ip
                || (self.games[g].info.in_progress && self.host.network_is_packet_router())
            {
                let mut reply = LANMessage::new();
                self.fill_in_lan_message(&mut reply);
                reply.set_message_type(lan_message_type::GAME_ANNOUNCE);
                let game_opts = game_info_to_ascii_string(Some(&self.games[g].info), &self.host).unwrap_or_default();
                reply.set_game_info_options(&game_opts);
                let n = self.games[g].get_name();
                reply.set_game_name(n.str());
                reply.set_game_info_in_progress(self.games[g].info.in_progress);
                reply.set_game_info_is_direct_connect(self.games[g].get_is_direct_connect());
                self.send_message(&reply, sender_ip);
            }
        }
    }

    /// `handleRequestJoin` (LANAPIhandlers.cpp): the host decides: deny if the game started, the serial is already
    /// in the game, the name is taken, or no slot is free for the map's starting spots; else the first open slot
    /// goes to the joiner and the options are re-sent.
    pub fn handle_request_join(&mut self, msg: &LANMessage, sender_ip: u32) {
        let mut response_ip = sender_ip; // the player may or may not be known yet
        if msg.game_to_join_game_ip() != self.local_ip {
            return; // not us
        }
        let mut reply = LANMessage::new();
        self.fill_in_lan_message(&mut reply);

        let hosting = !self.in_lobby
            && self.current_game.map_or(false, |g| self.games[g].get_ip(0) == self.local_ip);
        if hosting {
            let g = self.current_game.unwrap_or(0);
            if self.games[g].info.in_progress {
                reply.set_message_type(lan_message_type::JOIN_DENY);
                reply.set_joined_slot_or_reason(ReturnType::GameStarted as i32);
                reply.set_joined_game_ip(self.local_ip);
                reply.set_joined_player_ip(sender_ip);
            } else {
                let mut can_join = true;
                // (the exe/ini CRC check is inside a comment upstream; `m_netMinPlayers` is only read)
                let _ = self.host.net_min_players();

                // a serial already in the game is a duplicate
                let mut player = 0usize;
                while can_join && player < SLOTS {
                    let mut s: Vec<u8> = Vec::new();
                    if player == 0 {
                        s = self.host.registry_serial();
                    } else if self.games[g].info.slots[player].is_human() {
                        s = self.games[g].extras[player].get_serial().str().to_vec();
                        if s.is_empty() {
                            s = b"<Munkee>".to_vec();
                        }
                    }
                    if !s.is_empty() {
                        let a: Vec<u8> = s.iter().take(G_MAX_SERIAL_LENGTH).copied().collect();
                        let b: Vec<u8> = msg.game_to_join_serial().iter().take(G_MAX_SERIAL_LENGTH).copied().collect();
                        if a == b {
                            reply.set_message_type(lan_message_type::JOIN_DENY);
                            reply.set_joined_slot_or_reason(ReturnType::SerialDupe as i32);
                            reply.set_joined_game_ip(self.local_ip);
                            reply.set_joined_player_ip(sender_ip);
                            can_join = false;
                            break;
                        }
                    }
                    player += 1;
                }

                // a name already in the game is a duplicate
                let joiner_name = msg.name();
                let mut player = 0usize;
                while can_join && player < SLOTS {
                    if self.games[g].info.slots[player].is_human()
                        && self.games[g].info.slots[player].name.as_slice() == joiner_name.str()
                    {
                        reply.set_message_type(lan_message_type::JOIN_DENY);
                        reply.set_joined_slot_or_reason(ReturnType::DuplicateName as i32);
                        reply.set_joined_game_ip(self.local_ip);
                        reply.set_joined_player_ip(sender_ip);
                        can_join = false;
                        break;
                    }
                    player += 1;
                }

                // occupied non-observer slots against the map's starting spots
                let mut num_players = 0;
                for p in 0..SLOTS {
                    let s = &self.games[g].info.slots[p];
                    if s.is_occupied() && s.player_template != PLAYERTEMPLATE_OBSERVER {
                        num_players += 1;
                    }
                }
                let mut num_starting_spots = MAX_SLOTS;
                if let Some(n) = self.host.map_num_players(&self.games[g].info.map_name) {
                    num_starting_spots = n;
                }

                let mut player = 0usize;
                if num_players < num_starting_spots {
                    while can_join && player < SLOTS {
                        if self.games[g].info.slots[player].is_open() {
                            reply.set_message_type(lan_message_type::JOIN_ACCEPT);
                            let gn = self.games[g].get_name();
                            reply.set_game_name(gn.str());
                            reply.set_joined_slot_or_reason(player as i32);
                            reply.set_joined_game_ip(self.local_ip);
                            reply.set_joined_player_ip(sender_ip);

                            let mut new_slot = GameSlot::new();
                            new_slot.set_state(slot_state::PLAYER, joiner_name.str(), 0, &mut self.host);
                            new_slot.ip = sender_ip;
                            new_slot.port = NETWORK_BASE_PORT_NUMBER as u16;
                            let mut extra = LANSlotExtra::new();
                            extra.set_last_heard(self.host.time_get_time());
                            extra.set_serial(
                                crate::common::ascii_string::AsciiString::try_from_bytes(&msg.game_to_join_serial())
                                    .unwrap_or_default(),
                            );
                            self.games[g].set_slot(player as i32, new_slot, extra);
                            self.host.on_player_join(player as i32, &joiner_name);
                            response_ip = 0;
                            break;
                        }
                        player += 1;
                    }
                }
                if can_join && player == SLOTS {
                    reply.set_message_type(lan_message_type::JOIN_DENY);
                    let gn = self.games[g].get_name();
                    reply.set_game_name(gn.str());
                    reply.set_joined_slot_or_reason(ReturnType::GameFull as i32);
                    reply.set_joined_game_ip(self.local_ip);
                    reply.set_joined_player_ip(sender_ip);
                }
            }
        } else {
            reply.set_message_type(lan_message_type::JOIN_DENY);
            reply.set_joined_slot_or_reason(ReturnType::GameGone as i32);
            reply.set_joined_game_ip(self.local_ip);
            reply.set_joined_player_ip(sender_ip);
        }
        self.send_message(&reply, response_ip);
        let opts = self.generate_options();
        self.request_game_options(&opts, true, 0);
    }

    /// `handleJoinAccept` (LANAPIhandlers.cpp): the host let me in: enter the game in the slot it gave me.
    pub fn handle_join_accept(&mut self, msg: &LANMessage, sender_ip: u32) {
        if msg.joined_player_ip() == self.local_ip && self.pending_action == PendingActionType::Join {
            self.current_game = self.lookup_game(&msg.game_name());
            match self.current_game {
                None => {
                    // DEBUG_ASSERTCRASH(false, "Could not find game to join!")
                    self.host.on_game_join(ReturnType::Unknown, None);
                }
                Some(g) => {
                    self.in_lobby = false;
                    let options = game_info_to_ascii_string(Some(&self.games[g].info), &self.host).unwrap_or_default();
                    self.games[g].info.enter_game(&mut self.host);
                    let _ = parse_ascii_string_to_game_info(Some(&mut self.games[g].info), &options, &mut self.host);
                    let pos = msg.joined_slot_or_reason();

                    let mut slot = GameSlot::new();
                    slot.set_state(slot_state::PLAYER, self.name.str(), 0, &mut self.host);
                    slot.ip = self.local_ip;
                    slot.port = NETWORK_BASE_PORT_NUMBER as u16;
                    let mut extra = LANSlotExtra::new();
                    extra.set_last_heard(0);
                    extra.set_login_ascii(&self.user_name);
                    extra.set_host_ascii(&self.host_name);
                    self.games[g].set_slot(pos, slot, extra);
                    self.games[g].extras[0].set_host(widen(&msg.host_name()));
                    self.games[g].extras[0].set_login(widen(&msg.user_name()));

                    // remember the host for the preferences' recent-IP list
                    let host_name = UnicodeString::try_from_chars(&self.games[g].info.slots[0].name).unwrap_or_default();
                    let mut entry = format!(
                        "{}.{}.{}.{}:",
                        sender_ip >> 24,
                        (sender_ip & 0xff0000) >> 16,
                        (sender_ip & 0xff00) >> 8,
                        sender_ip & 0xff
                    )
                    .into_bytes();
                    entry.extend_from_slice(&self.host.quoted_printable(&host_name));
                    self.host.remember_remote_ip(&entry);

                    let game = self.games[g].clone();
                    self.host.on_game_join(ReturnType::Ok, Some(&game));
                }
            }
            self.pending_action = PendingActionType::None;
            self.expiration = 0;
        }
    }

    /// `handleJoinDeny` (LANAPIhandlers.cpp).
    pub fn handle_join_deny(&mut self, msg: &LANMessage, _sender_ip: u32) {
        if msg.joined_player_ip() == self.local_ip && self.pending_action == PendingActionType::Join {
            let reason = ReturnType::from_i32(msg.joined_slot_or_reason());
            let game = self.lookup_game(&msg.game_name()).map(|i| self.games[i].clone());
            self.host.on_game_join(reason, game.as_ref());
            self.pending_action = PendingActionType::None;
            self.expiration = 0;
        }
    }

    /// `handleRequestGameLeave` (LANAPIhandlers.cpp): in game setup, the host leaving ends the game (I am back in the
    /// lobby), a joiner leaving frees their slot; in the lobby, the named game is dropped from the list.
    pub fn handle_request_game_leave(&mut self, msg: &LANMessage, sender_ip: u32) {
        if !self.in_lobby && self.current_game.map_or(false, |g| !self.games[g].info.in_progress) {
            let g = self.current_game.unwrap_or(0);
            for player in 0..SLOTS {
                if self.games[g].get_ip(player) == sender_ip {
                    if player == 0 {
                        self.host.on_host_leave();
                        self.remove_game(g);
                        self.current_game = None;
                        let mut lan_player = match self.lookup_player(self.local_ip) {
                            Some(i) => self.lobby_players.remove(i),
                            None => {
                                let mut p = LANPlayer::new();
                                p.set_ip(self.local_ip);
                                p
                            }
                        };
                        lan_player.set_name(self.name.clone());
                        lan_player.set_host_ascii(&self.host_name);
                        lan_player.set_login_ascii(&self.user_name);
                        lan_player.set_last_heard(self.host.time_get_time());
                        self.add_player(lan_player);
                    } else {
                        if self.am_i_host() {
                            let mut slot = GameSlot::new();
                            slot.set_state(slot_state::OPEN, &[], 0, &mut self.host);
                            self.games[g].set_slot(player as i32, slot, LANSlotExtra::new());
                        }
                        self.host.on_player_leave(&msg.name());
                        self.games[g].info.slots[player].set_state(slot_state::OPEN, &[], 0, &mut self.host);
                        self.games[g].reset_accepted(&mut self.host);
                        let opts = self.generate_options();
                        self.request_game_options(&opts, false, sender_ip);
                    }
                    break;
                }
            }
        } else if self.in_lobby {
            let name = msg.game_name();
            if let Some(i) = self.games.iter().position(|g| g.get_name() == name) {
                self.remove_game(i);
                let gl = self.games.clone();
                self.host.on_game_list(&gl);
            }
        }
    }

    /// `handleRequestLobbyLeave` (LANAPIhandlers.cpp).
    pub fn handle_request_lobby_leave(&mut self, _msg: &LANMessage, sender_ip: u32) {
        if self.in_lobby {
            if let Some(i) = self.lobby_players.iter().position(|p| p.get_ip() == sender_ip) {
                self.remove_player(i);
                let pl = self.lobby_players.clone();
                self.host.on_player_list(&pl);
            }
        }
    }

    /// `handleSetAccept` (LANAPIhandlers.cpp).
    pub fn handle_set_accept(&mut self, msg: &LANMessage, sender_ip: u32) {
        if !self.in_lobby && self.current_game.map_or(false, |g| !self.games[g].info.in_progress) {
            let g = self.current_game.unwrap_or(0);
            for player in 0..SLOTS {
                if self.games[g].get_ip(player) == sender_ip {
                    self.host.on_accept(sender_ip, msg.accept_is_accepted());
                    break;
                }
            }
        }
    }

    /// `handleHasMap` (LANAPIhandlers.cpp): only if the sender means my game's map (the CRC of its portable
    /// name).
    pub fn handle_has_map(&mut self, msg: &LANMessage, sender_ip: u32) {
        if !self.in_lobby {
            if let Some(g) = self.current_game {
                let portable = self.host.map_to_portable(&self.games[g].info.map_name);
                let mut crc = Crc::new();
                crc.compute_crc(&portable);
                if crc.get() != msg.map_status_map_crc() {
                    return;
                }
                for player in 0..SLOTS {
                    if self.games[g].get_ip(player) == sender_ip {
                        self.host.on_has_map(sender_ip, msg.map_status_has_map());
                        break;
                    }
                }
            }
        }
    }

    /// `handleChat` (LANAPIhandlers.cpp): lobby chat from a known player, or game chat from a slot of my game.
    pub fn handle_chat(&mut self, msg: &LANMessage, sender_ip: u32) {
        let format = ChatType::from_i32(msg.chat_type());
        if self.in_lobby {
            if let Some(i) = self.lookup_player(sender_ip) {
                let (pn, pip) = (self.lobby_players[i].get_name(), self.lobby_players[i].get_ip());
                self.host.on_chat(&pn, pip, &msg.chat_message(), format);
                let now = self.host.time_get_time();
                self.lobby_players[i].set_last_heard(now);
            }
        } else {
            if self.lookup_game(&msg.game_name()) != self.current_game {
                return;
            }
            if let Some(g) = self.current_game {
                for player in 0..SLOTS {
                    if self.games[g].get_ip(player) == sender_ip {
                        self.host.on_chat(&msg.name(), sender_ip, &msg.chat_message(), format);
                        break;
                    }
                }
            }
        }
    }

    /// `handleGameStart` (LANAPIhandlers.cpp): only from the host of my game in setup.
    pub fn handle_game_start(&mut self, _msg: &LANMessage, sender_ip: u32) {
        if !self.in_lobby {
            if let Some(g) = self.current_game {
                if self.games[g].get_ip(0) == sender_ip && !self.games[g].info.in_progress {
                    self.host.on_game_start();
                }
            }
        }
    }

    /// `handleGameStartTimer` (LANAPIhandlers.cpp).
    pub fn handle_game_start_timer(&mut self, msg: &LANMessage, sender_ip: u32) {
        if !self.in_lobby {
            if let Some(g) = self.current_game {
                if self.games[g].get_ip(0) == sender_ip && !self.games[g].info.in_progress {
                    self.host.on_game_start_timer(msg.start_timer_seconds());
                }
            }
        }
    }

    /// `handleGameOptions` (LANAPIhandlers.cpp).
    pub fn handle_game_options(&mut self, msg: &LANMessage, sender_ip: u32) {
        if !self.in_lobby && self.current_game.map_or(false, |g| !self.games[g].info.in_progress) {
            let g = self.current_game.unwrap_or(0);
            for player in 0..SLOTS {
                if self.games[g].get_ip(player) == sender_ip {
                    self.host.on_game_options(sender_ip, player as i32, &msg.game_options_options());
                    break;
                }
            }
        }
    }

    /// `handleInActive` (LANAPIhandlers.cpp): the host un-accepts a player who alt-tabbed out (not itself, not
    /// during the start countdown).
    pub fn handle_in_active(&mut self, msg: &LANMessage, sender_ip: u32) {
        let g = match self.current_game {
            Some(g) if !self.in_lobby && !self.games[g].info.in_progress => g,
            _ => return,
        };
        if !self.games[g].am_i_host(self.local_ip) {
            return;
        }
        let player_name = msg.name();
        let slot_num = self.games[g].get_slot_num(&player_name);
        if slot_num < 0 {
            return;
        }
        let su = slot_num as usize;
        if sender_ip != self.games[g].info.slots[su].ip {
            return;
        }
        if sender_ip == self.local_ip {
            return;
        }
        if self.game_start_time != 0 {
            return;
        }
        self.games[g].info.slots[su].un_accept();
        let options = self.generate_options();
        self.request_game_options(&options, false, 0);
        self.host.lan_update_slot_list();
    }
}

// keep the shared limits imported for the doc references above
#[allow(dead_code)]
const _USES: usize = G_LAN_PLAYER_NAME_LENGTH;
#[allow(dead_code)]
fn _utf16_used() -> Vec<u16> {
    utf16("")
}
