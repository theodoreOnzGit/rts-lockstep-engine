#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANAPI.cpp and Include/GameNetwork/LANAPI.h, LANPlayer.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/lan_api.rs` against the C++ (GitHub #215, group network3).
//!
//! # Method
//!
//! `differential/wrap_draft_lanapi.cpp` compiles, cut verbatim by build.rs, LANAPI.cpp's `sendMessage`, `GetMessageTypeString`, the request
//! functions (all but `RequestGameCreate`), `fillInLANMessage`, the game and player lists (`add*`, `remove*`, `Lookup*`), `AmIHost` and `setIsActive`,
//! and LANAPI.h's `LANMessage` and its constants, on the real `LANPlayer.h` and upstream's own `UnicodeString.cpp`/`AsciiString.cpp`/`crc.cpp`.
//! The class `LANAPI`, the games, the transport and the callbacks are stand-ins listed in the wrapper. A program is a configuration (clock, local
//! IP, names, CRCs, serial) and 30 to 90 operations (opcodes at `ref_la_run`); after every operation the lists, the current game, the flags and
//! the timers are dumped, and every datagram queued and every callback is logged: the Rust side reads the draft's transport queue (decrypting the
//! messages `queue_send` encrypted) and its host logs the callbacks identically. Also the layout (`sizeof(LANMessage)`, every field's offset, the
//! constants) and `GetMessageTypeString`.
//!
//! Not compared: `RequestGameAnnounce` (it builds the options with `GameInfoToAsciiString`, which the verified game-info port tests), `RequestGameCreate`,
//! `init`, `reset`, `update`, `checkMOTD`, `SetLocalIP` (the transport binds a socket), `transport.update` (a socket write: not observable without
//! a socket), the `On*` handlers of `LANAPIhandlers.cpp` (another draft file) and `getErrorStringFromReturnType` (GUI text).
//! `RequestGameJoinDirectConnect` is skipped (by both sides) when the local name is longer than 12 characters: upstream's `wcsncpy(playerName, m_name.str(), m_name.getLength())`
//! overflows the 13-character field (a defect, reported on #210); the draft clamps.
//! `RequestChat` is given only the three real chat types (the draft's enum has no `LANCHAT_MAX`, which is a count, not a type).
//! `RequestHasMap` with no local slot: upstream indexes slot -1 (undefined); the stand-in and the draft both say "no map" there.

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::game_info::*;
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::drafts::network::lan_api::*;
use rts_lockstep_engine::drafts::network::lan_game_info::*;
use rts_lockstep_engine::drafts::network::transport::decrypt_buf;
use std::cell::{Cell, RefCell};
use std::os::raw::{c_char, c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_la_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
    fn ref_la_layout(out: *mut c_uint) -> c_int;
    fn ref_la_typestring(ty: c_uint, out: *mut c_char, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn fnv(h: u32, c: u8) -> u32 {
    (h ^ u32::from(c)).wrapping_mul(16777619)
}
fn hash_bytes(s: &[u8]) -> u32 {
    s.iter().fold(2166136261u32, |h, &c| fnv(h, c))
}
fn hash_str(s: &[u8]) -> u32 {
    let e = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    hash_bytes(&s[..e])
}
fn hash_units(s: &[u16]) -> u32 {
    s.iter().fold(2166136261u32, |h, &c| fnv(fnv(h, (c & 255) as u8), (c >> 8) as u8))
}

// ---------------------------------------------------------------------------------------------------------------
// the host
// ---------------------------------------------------------------------------------------------------------------

struct Host {
    now: Cell<u32>,
    local_ip: u32,
    exe_crc: u32,
    ini_crc: u32,
    serial: Vec<u8>,
    packet_router: bool,
    log: RefCell<Vec<u32>>,
}

impl Host {
    fn push(&self, v: u32) {
        self.log.borrow_mut().push(v);
    }
}

impl GameInfoHost for Host {
    fn game_text(&mut self, label: &str) -> Vec<u16> {
        label.bytes().map(u16::from).collect()
    }
    fn tick_count(&mut self) -> u32 {
        0
    }
    fn default_starting_cash(&self) -> Money {
        Money::new()
    }
    fn net_crc_interval(&self) -> i32 {
        100
    }
    fn has_map_cache(&self) -> bool {
        false
    }
    fn map_cache_find(&self, _lower: &[u8]) -> Option<MapMeta> {
        None
    }
    fn file_exists(&mut self, _path: &[u8]) -> bool {
        false
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }
    fn has_multiplayer_settings(&self) -> bool {
        true
    }
    fn num_colors(&self) -> i32 {
        8
    }
    fn show_random_player_template(&self) -> bool {
        false
    }
    fn show_random_color(&self) -> bool {
        false
    }
    fn show_random_start_pos(&self) -> bool {
        false
    }
    fn observer_color(&self) -> Result<i32, IniError> {
        Ok(0)
    }
    fn player_template_count(&self) -> i32 {
        4
    }
    fn player_template_display_name(&self, n: i32) -> Result<Vec<u16>, IniError> {
        Ok(format!("PT{n}").bytes().map(u16::from).collect())
    }
    fn record_income(&mut self, _player_index: i32) {}
}

impl LanGameInfoHost for Host {
    fn has_lan(&self) -> bool {
        true
    }
    fn lan_local_ip(&self) -> u32 {
        self.local_ip
    }
    fn lan_reset_game_start_timer(&mut self) {}
    fn lan_my_game_is_this(&self) -> bool {
        false
    }
    fn lan_am_i_host(&self) -> bool {
        false
    }
    fn lan_enable_start_button(&mut self, _enabled: bool) {}
    fn lan_request_has_map(&mut self) {}
    fn lan_update_slot_list(&mut self) {}
    fn lan_update_game_options(&mut self) {}
    fn time_get_time(&self) -> u32 {
        self.now.get()
    }
}

impl LanApiHost for Host {
    fn on_game_list(&mut self, _games: &[LANGameInfo]) {}
    fn on_player_list(&mut self, _players: &[LANPlayer]) {}
    fn on_game_join(&mut self, ret: ReturnType, game: Option<&LANGameInfo>) {
        self.push(92);
        self.push(ret as u32);
        self.push(u32::from(game.is_some()));
    }
    fn on_player_join(&mut self, _slot: i32, _player_name: &UnicodeString) {}
    fn on_host_leave(&mut self) {}
    fn on_player_leave(&mut self, player: &UnicodeString) {
        self.push(93);
        self.push(hash_units(player.str()));
    }
    fn on_accept(&mut self, _player_ip: u32, _status: bool) {}
    fn on_has_map(&mut self, _player_ip: u32, _status: bool) {}
    fn on_chat(&mut self, player: &UnicodeString, ip: u32, message: &UnicodeString, format: ChatType) {
        self.push(94);
        self.push(hash_units(player.str()));
        self.push(ip);
        self.push(hash_units(message.str()));
        self.push(format as u32);
    }
    fn on_game_start(&mut self) {
        self.push(95);
    }
    fn on_game_start_timer(&mut self, seconds: i32) {
        self.push(96);
        self.push(seconds as u32);
    }
    fn on_game_options(&mut self, player_ip: u32, player_slot: i32, options: &[u8]) {
        self.push(97);
        self.push(player_ip);
        self.push(player_slot as u32);
        self.push(hash_str(options));
    }
    fn on_game_create(&mut self, _ret: ReturnType) {}
    fn on_name_change(&mut self, ip: u32, new_name: &UnicodeString) {
        self.push(98);
        self.push(ip);
        self.push(hash_units(new_name.str()));
    }
    fn lan_button_pushed(&self) -> bool {
        false
    }
    fn lan_socket_error_detected(&self) -> bool {
        false
    }
    fn set_lan_socket_error_detected(&mut self) {}
    fn fetch_text(&mut self, _label: &str) -> Vec<u16> {
        Vec::new()
    }
    fn registry_serial(&mut self) -> Vec<u8> {
        self.serial.clone()
    }
    fn exe_crc(&self) -> u32 {
        self.exe_crc
    }
    fn ini_crc(&self) -> u32 {
        self.ini_crc
    }
    fn net_min_players(&self) -> i32 {
        1
    }
    fn user_name(&mut self) -> Option<Vec<u8>> {
        None
    }
    fn computer_name(&mut self) -> Option<Vec<u8>> {
        None
    }
    fn network_is_packet_router(&self) -> bool {
        self.packet_router
    }
    fn map_num_players(&self, _map: &[u8]) -> Option<i32> {
        None
    }
    fn map_to_portable(&self, map: &[u8]) -> Vec<u8> {
        let mut v = b"P:".to_vec();
        v.extend_from_slice(map);
        v
    }
    fn no_map_message(&mut self, map: &[u8]) -> UnicodeString {
        // the stand-in's fetch and format: "NOMAPT:%ls" when the map would transfer (an odd hash), else "NOMAP:%ls", with the leaf name (the name itself)
        let mut v: Vec<u16> = if hash_str(map) & 1 != 0 { "NOMAPT:".encode_utf16().collect() } else { "NOMAP:".encode_utf16().collect() };
        v.extend(map.iter().map(|&c| u16::from(c)));
        UnicodeString::try_from_chars(&v).unwrap()
    }
    fn preferred_map(&mut self) -> Vec<u8> {
        Vec::new()
    }
    fn remember_remote_ip(&mut self, _entry: &[u8]) {}
    fn quoted_printable(&self, _name: &UnicodeString) -> Vec<u8> {
        Vec::new()
    }
    fn read_local_motd(&mut self) -> Option<Vec<u8>> {
        None
    }
    fn get_tick_count(&self) -> u32 {
        0
    }
}

// ---------------------------------------------------------------------------------------------------------------
// the program
// ---------------------------------------------------------------------------------------------------------------

const NAMES: &[&str] = &[
    "Alpha", "alpha", "ALPHA", "Bravo", "bravo", "Charlie", "", "a", "A", "Zulu", "Echo", "echoo", "Delta Force 1", "Delta Force 2", "TwelveCharsXX", "ThirteenChars", "FifteenCharsXXXX", "SixteenCharsXXXXX",
    "SeventeenCharsXXXX", "An even longer game name than any limit", "x y",
];
const MAPS: &[&str] = &["Maps\\Alpha\\Alpha.map", "maps\\beta\\beta.map", "", "x", "Maps\\Gamma\\gamma.map", "d"];
const OPTS: &[&str] = &["", "a", "M=maps\\x.map;S=H,0,0,0,0,0,-1,0:", "short", "X"];

fn push_bytes(v: &mut Vec<u32>, b: &[u8]) {
    v.push(b.len() as u32);
    for c in b.chunks(4) {
        let mut w = 0u32;
        for (j, &x) in c.iter().enumerate() {
            w |= u32::from(x) << (8 * j);
        }
        v.push(w);
    }
}

fn push_units(v: &mut Vec<u32>, u: &[u16]) {
    v.push(u.len() as u32);
    for c in u.chunks(2) {
        let mut w = u32::from(c[0]);
        if c.len() > 1 {
            w |= u32::from(c[1]) << 16;
        }
        v.push(w);
    }
}

fn name_units(i: u32) -> Vec<u16> {
    NAMES[i as usize % NAMES.len()].encode_utf16().collect()
}

fn ip(g: &mut Gen) -> u32 {
    match g.below(6) {
        0 => 0,
        1 => 0x7f000001,
        2 => 0xc0a80001 + g.below(6),
        3 => 0x0a000001 + g.below(4),
        4 => g.u32(),
        _ => 0xc0a80001 + g.below(3),
    }
}

struct Prog {
    w: Vec<u32>,
}

fn program(g: &mut Gen) -> Vec<u32> {
    let mut p = Prog { w: Vec::new() };
    let v = &mut p.w;
    v.push(g.below(100_000)); // clock
    let local = match g.below(3) {
        0 => 0xc0a80001,
        1 => 0xc0a80001 + g.below(3),
        _ => ip(g),
    };
    v.push(local);
    push_units(v, &name_units(g.below(40)));
    let user = ["", "u", "user", "bob"][g.below(4) as usize];
    let host = ["", "h", "host", "machine"][g.below(4) as usize];
    push_bytes(v, user.as_bytes());
    push_bytes(v, host.as_bytes());
    v.push(g.u32());
    v.push(g.u32());
    let serial = ["", "SERIAL", "0123456789012345678901", "01234567890123456789012", "012345678901234567890123456"][g.below(5) as usize];
    push_bytes(v, serial.as_bytes());
    v.push(g.below(2));
    v.push(g.below(10_000));
    v.push(if g.below(4) == 0 { 0 } else { 0xFFFF_FFFF });
    let nops = 30 + g.below(61);
    for _ in 0..nops {
        let op = match g.below(10) {
            0..=2 => 1 + g.below(15),
            _ => 20 + g.below(19),
        };
        v.push(op);
        match op {
            1 => {
                push_units(v, &name_units(g.below(40)));
                v.push(g.below(3) / 2);
                v.push(g.below(2));
                push_bytes(v, MAPS[g.below(MAPS.len() as u32) as usize].as_bytes());
                for _ in 0..8 {
                    v.push(g.below(3).min(1));
                    v.push(ip(g));
                    v.push(g.below(2));
                }
            }
            2 => {
                push_units(v, &name_units(g.below(40)));
                v.push(ip(g));
            }
            3 | 4 | 6 => v.push(g.below(6)),
            5 | 26 | 30 => {
                push_units(v, &name_units(g.below(40)));
                if op == 26 {
                    v.push(g.below(3)); // LANCHAT_NORMAL..LANCHAT_SYSTEM; LANCHAT_MAX (3) is a count no caller passes, and the draft's enum cannot hold it
                }
            }
            7 | 11 | 14 | 22 => v.push(ip(g)),
            8 => v.push(g.below(7).wrapping_sub(1)),
            9 | 31 | 34 => v.push(g.below(2)),
            10 => v.push(g.below(4)),
            12 => push_units(v, &name_units(g.below(40))),
            13 => v.push(g.below(8000)),
            15 => v.push(g.below(100_000)),
            21 => {
                v.push(g.below(7).wrapping_sub(1));
                v.push(ip(g));
            }
            28 => v.push(g.below(5)),
            29 => {
                push_bytes(v, OPTS[g.below(OPTS.len() as u32) as usize].as_bytes());
                v.push(g.below(2));
                v.push(ip(g));
            }
            36 => v.push(g.below(20)),
            37 | 38 => push_bytes(v, ["", "u", "user", "toolong"][g.below(4) as usize].as_bytes()),
            _ => {}
        }
    }
    p.w
}

struct Rd<'a> {
    w: &'a [u32],
    i: usize,
}
impl Rd<'_> {
    fn next(&mut self) -> u32 {
        let v = self.w.get(self.i).copied().unwrap_or(0);
        self.i += 1;
        v
    }
    fn bytes(&mut self) -> Vec<u8> {
        let n = self.next() as usize;
        let mut b = vec![0u8; n];
        let mut i = 0;
        while i < n {
            let v = self.next();
            for j in 0..4 {
                if i + j < n {
                    b[i + j] = (v >> (8 * j)) as u8;
                }
            }
            i += 4;
        }
        b
    }
    fn units(&mut self) -> Vec<u16> {
        let n = self.next() as usize;
        let mut b = vec![0u16; n];
        let mut i = 0;
        while i < n {
            let v = self.next();
            b[i] = v as u16;
            if i + 1 < n {
                b[i + 1] = (v >> 16) as u16;
            }
            i += 2;
        }
        b
    }
}

fn uni(u: &[u16]) -> UnicodeString {
    UnicodeString::try_from_chars(u).unwrap()
}
fn asc(b: &[u8]) -> AsciiString {
    AsciiString::try_from_bytes(b).unwrap()
}
fn log_str(o: &mut Vec<u32>, s: &[u8]) {
    o.push(s.len() as u32);
    o.extend(s.iter().map(|&c| u32::from(c)));
}
fn log_units(o: &mut Vec<u32>, s: &[u16]) {
    o.push(s.len() as u32);
    o.extend(s.iter().map(|&c| u32::from(c)));
}

fn dump(o: &mut Vec<u32>, a: &LANAPI<Host>) {
    o.push(0xD0D0D0D0);
    o.push(a.games.len() as u32);
    for g in &a.games {
        log_units(o, g.get_name().str());
        o.push(u32::from(g.get_is_direct_connect()));
        o.push(u32::from(g.info.in_progress));
        log_str(o, &g.info.map_name);
        for i in 0..8 {
            o.push(u32::from(g.info.slots[i].is_human()));
            o.push(g.info.slots[i].ip);
            o.push(u32::from(g.info.slots[i].has_map));
        }
    }
    o.push(a.lobby_players.len() as u32);
    for p in &a.lobby_players {
        log_units(o, p.get_name().str());
        log_units(o, p.get_login().str());
        log_units(o, p.get_host().str());
        o.push(p.get_last_heard());
        o.push(p.get_ip());
    }
    o.push(a.current_game.map_or(-1i32, |c| c as i32) as u32);
    o.push(u32::from(a.in_lobby));
    o.push(a.pending_action as u32);
    o.push(a.expiration);
    o.push(a.game_start_time);
    o.push(a.game_start_seconds as u32);
    o.push(a.last_resend_time);
    log_str(o, a.last_gameopt.str());
    o.push(a.local_ip);
    o.push(a.direct_connect_remote_ip);
    o.push(u32::from(a.is_active));
    log_units(o, a.name.str());
}

/// The datagrams the draft queued, as the C++ stand-in logs them (the transport encrypts each message in place, so it is decrypted first).
fn drain_sent(a: &mut LANAPI<Host>, o: &mut Vec<u32>) {
    for m in a.transport.out_buffer.iter_mut() {
        if m.length != 0 {
            let n = m.length as usize;
            let mut image = Vec::with_capacity(6 + n);
            image.extend_from_slice(&m.header.crc.to_le_bytes());
            image.extend_from_slice(&m.header.magic.to_le_bytes());
            image.extend_from_slice(&m.data[..n]);
            let total = image.len();
            decrypt_buf(&mut image, total);
            o.extend([90, m.addr, u32::from(m.port), m.length as u32, hash_bytes(&image[6..])]);
            m.length = 0;
        }
    }
}

fn make_game(a: &mut LANAPI<Host>, name: &[u16], direct: bool, in_progress: bool, map: &[u8], slots: &[(bool, u32, bool)]) -> LANGameInfo {
    let mut g = LANGameInfo::new(&mut a.host);
    g.info.in_game = true;
    g.set_name(uni(name));
    g.set_is_direct_connect(direct);
    g.info.in_progress = in_progress;
    g.info.map_name = map.to_vec();
    for (i, &(human, ipv, has_map)) in slots.iter().enumerate() {
        g.info.slots[i].state = if human { slot_state::PLAYER } else { slot_state::OPEN };
        g.info.slots[i].ip = ipv;
        g.info.slots[i].has_map = has_map;
    }
    g
}

fn rust_run(w: &[u32]) -> Vec<u32> {
    let mut rd = Rd { w, i: 0 };
    let now = rd.next();
    let local = rd.next();
    let name = rd.units();
    let user = rd.bytes();
    let hostn = rd.bytes();
    let exe = rd.next();
    let ini = rd.next();
    let serial = rd.bytes();
    let pr = rd.next() != 0;
    let timeout = rd.next();
    let bcast = rd.next();
    let host = Host { now: Cell::new(now), local_ip: local, exe_crc: exe, ini_crc: ini, serial, packet_router: pr, log: RefCell::new(Vec::new()) };
    let mut a = LANAPI::new(host);
    a.local_ip = local;
    a.name = uni(&name);
    a.user_name = asc(&user);
    a.host_name = asc(&hostn);
    a.action_timeout = timeout;
    a.broadcast_addr = bcast;
    a.pending_action = PendingActionType::None;
    a.expiration = 0;
    a.in_lobby = true;
    a.game_start_time = 0;
    a.game_start_seconds = 0;
    a.last_resend_time = 0;
    a.direct_connect_remote_ip = 0;
    a.is_active = true;
    let mut o: Vec<u32> = Vec::new();
    dump(&mut o, &a);
    while rd.i < w.len() {
        let op = rd.next();
        o.push(0xA5A5A5A5);
        o.push(op);
        let mut res: Vec<u32> = Vec::new();
        match op {
            1 => {
                let n = rd.units();
                let direct = rd.next() != 0;
                let inprog = rd.next() != 0;
                let map = rd.bytes();
                let mut slots = vec![];
                for _ in 0..8 {
                    let h = rd.next() != 0;
                    let i = rd.next();
                    let m = rd.next() != 0;
                    slots.push((h, i, m));
                }
                let g = make_game(&mut a, &n, direct, inprog, &map, &slots);
                a.add_game(g);
            }
            2 => {
                let n = rd.units();
                let ipv = rd.next();
                let mut p = LANPlayer::new();
                p.set_name(uni(&n));
                p.set_ip(ipv);
                a.add_player(p);
            }
            3 => {
                let k = rd.next() as usize;
                if k < a.games.len() {
                    a.remove_game(k);
                }
            }
            4 => {
                let k = rd.next() as usize;
                if k < a.lobby_players.len() {
                    a.remove_player(k);
                }
            }
            5 => {
                let n = rd.units();
                res.push(a.lookup_game(&uni(&n)).map_or(-1i32, |i| i as i32) as u32);
            }
            6 => {
                let k = rd.next() as i32;
                res.push(a.lookup_game_by_list_offset(k).map_or(-1i32, |i| i as i32) as u32);
            }
            7 => {
                let k = rd.next();
                res.push(a.lookup_player(k).map_or(-1i32, |i| i as i32) as u32);
            }
            8 => {
                let k = rd.next() as i32;
                a.current_game = if k < 0 || k as usize >= a.games.len() { None } else { Some(k as usize) };
            }
            9 => a.in_lobby = rd.next() != 0,
            10 => {
                a.pending_action = match rd.next() {
                    0 => PendingActionType::None,
                    1 => PendingActionType::Join,
                    2 => PendingActionType::JoinDirectConnect,
                    _ => PendingActionType::Leave,
                }
            }
            11 => {
                let k = rd.next();
                a.local_ip = k;
                a.host.local_ip = k;
            }
            12 => {
                let n = rd.units();
                a.name = uni(&n);
            }
            13 => {
                let dt = rd.next();
                a.host.now.set(a.host.now.get().wrapping_add(dt));
            }
            14 => a.direct_connect_remote_ip = rd.next(),
            15 => a.expiration = rd.next(),
            20 => a.request_locations(),
            21 => {
                let k = rd.next() as i32;
                let ipv = rd.next();
                a.request_game_join(if k < 0 { None } else { Some(k as usize) }, ipv);
            }
            22 => {
                let ipv = rd.next();
                if a.name.get_length() <= 12 {
                    // upstream copies the whole name into PlayerInfo.playerName[13] (an overflow for longer names, #210): not compared
                    a.request_game_join_direct_connect(ipv);
                }
            }
            23 => a.request_game_leave(),
            24 => a.request_accept(),
            25 => a.request_has_map(),
            26 => {
                let m = rd.units();
                let f = ChatType::from_i32(rd.next() as i32);
                a.request_chat(uni(&m), f);
            }
            27 => a.request_game_start(),
            28 => {
                let s = rd.next() as i32;
                a.request_game_start_timer(s);
            }
            29 => {
                let opts = rd.bytes();
                let public = rd.next() != 0;
                let ipv = rd.next();
                a.request_game_options(&opts, public, ipv);
            }
            30 => {
                let n = rd.units();
                a.request_set_name(uni(&n));
            }
            31 => {
                let f = rd.next() != 0;
                a.request_lobby_leave(f);
            }
            32 => a.reset_game_start_timer(),
            33 => res.push(u32::from(a.am_i_host())),
            34 => {
                let v = rd.next() != 0;
                a.set_is_active(v);
            }
            35 => {
                let mut m = LANMessage::from_bytes(&[0x5A; LAN_MESSAGE_SIZE]);
                a.fill_in_lan_message(&mut m);
                res.push(hash_bytes(m.as_bytes()));
            }
            36 => {
                let t = rd.next();
                log_str(&mut res, get_message_type_string(t).as_bytes());
            }
            37 => a.user_name = asc(&rd.bytes()),
            38 => a.host_name = asc(&rd.bytes()),
            _ => {}
        }
        // order in the C++ log: the datagrams and callbacks in the order they happened. The draft queues datagrams in the transport and calls
        // the host's callbacks; the relative order within one operation is: datagrams first, then callbacks, except where a function calls a callback
        // before it sends (none does), so the datagrams go first here.
        drain_sent(&mut a, &mut o);
        o.append(&mut a.host.log.borrow_mut());
        o.extend(res);
        dump(&mut o, &a);
    }
    o
}

fn cpp_run(w: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 4_000_000];
    let n = unsafe { ref_la_run(w.as_ptr(), w.len() as c_int, out.as_mut_ptr(), out.len() as c_int) };
    assert!((n as usize) <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

#[test]
fn lanapi_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("LA_CASES", 15_000);
    let mut g = Gen::new(0x1a4a_0001);
    let mut chk = Check::new("lan_api programs");
    for i in 0..n {
        let w = program(&mut g);
        let r = rust_run(&w);
        let c = cpp_run(&w);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(14);
            let opn = r[..k].iter().filter(|&&x| x == 0xA5A5A5A5).count();
            println!("case {i}: operation {opn}, first difference at word {k} (rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}", r.len(), c.len(), &r[k0..(k + 14).min(r.len())], &c[k0..(k + 14).min(c.len())]);
            if std::env::var("LA_DEBUG").is_ok() {
                println!("program {:?}", w);
            }
        }
        chk.eq(r == c, true, || format!("lan_api case {i}"));
    }
    println!("{n} programs");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}

#[test]
fn lanmessage_layout_matches_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = [0u32; 128];
    let n = unsafe { ref_la_layout(out.as_mut_ptr()) } as usize;
    let u = |f: &dyn Fn(&mut LANMessage)| -> u32 {
        let mut m = LANMessage::new();
        f(&mut m);
        m.as_bytes().iter().position(|&b| b != 0).expect("a setter changed nothing") as u32
    };
    let w: [u16; 1] = [0x7e7e];
    let c: [u8; 1] = [0x7e];
    let rust: Vec<u32> = vec![
        LAN_MESSAGE_SIZE as u32,
        u(&|m| m.set_message_type(-1)),
        u(&|m| m.set_name(&w)),
        u(&|m| m.set_user_name(&c)),
        u(&|m| m.set_host_name(&c)),
        u(&|m| m.set_start_timer_seconds(-1)),
        u(&|m| m.set_game_name(&w)), // GameToLeave.gameName and the other game names share the offset of the union's start
        u(&|m| m.set_game_name(&w)),
        u(&|m| m.set_game_info_in_progress(true)),
        u(&|m| m.set_game_info_options(&c)),
        u(&|m| m.set_game_info_is_direct_connect(true)),
        u(&|m| m.set_player_info_ip(!0)),
        u(&|m| m.set_player_info_name(&w)),
        u(&|m| m.set_game_to_join_game_ip(!0)),
        u(&|m| m.set_game_to_join_exe_crc(!0)),
        u(&|m| m.set_game_to_join_ini_crc(!0)),
        u(&|m| m.set_game_to_join_serial(&c)),
        u(&|m| m.set_game_name(&w)),
        u(&|m| m.set_joined_game_ip(!0)),
        u(&|m| m.set_joined_player_ip(!0)),
        u(&|m| m.set_joined_slot_or_reason(-1)),
        u(&|m| m.set_game_name(&w)),
        u(&|m| m.set_joined_game_ip(!0)),
        u(&|m| m.set_joined_player_ip(!0)),
        u(&|m| m.set_joined_slot_or_reason(-1)),
        u(&|m| m.set_game_name(&w)),
        u(&|m| m.set_accept_is_accepted(true)),
        u(&|m| m.set_game_name(&w)),
        u(&|m| m.set_map_status_map_crc(!0)),
        u(&|m| m.set_map_status_has_map(true)),
        u(&|m| m.set_game_name(&w)),
        u(&|m| m.set_chat_type(-1)),
        u(&|m| m.set_chat_message(&w)),
        u(&|m| m.set_game_options_options(&c)),
    ];
    // the C++ list has, after the size, 33 offsets; the draft's accessors share the game name and some union fields, so only the offsets whose
    // accessor is distinct are compared one to one: the shared ones are compared with the C++ offset of the member they stand for
    let mut chk = Check::new("lan_api LANMessage layout");
    chk.eq(rust[0], out[0], || "sizeof(LANMessage)".into());
    // C++ order after the size: type, name, userName, hostName, StartTimer.seconds, GameToLeave.gameName, GameInfo.{gameName, inProgress, options, isDirectConnect},
    // PlayerInfo.{ip, playerName}, GameToJoin.{gameIP, exeCRC, iniCRC, serial}, GameJoined.{gameName, gameIP, playerIP, slotPosition},
    // GameNotJoined.{gameName, gameIP, playerIP, reason}, Accept.{gameName, isAccepted}, MapStatus.{gameName, mapCRC, hasMap}, Chat.{gameName, chatType, message},
    // GameOptions.options
    for i in 1..=33 {
        let cpp_off = out[i];
        let r_off = rust[i];
        chk.eq(r_off, cpp_off, || format!("field {i} offset"));
    }
    let tail = &out[34..n];
    let expect_tail: Vec<u32> = vec![
        lan_message_type::MAX as u32,
        G_LAN_PLAYER_NAME_LENGTH as u32,
        G_LAN_LOGIN_NAME_LENGTH as u32,
        G_LAN_HOST_NAME_LENGTH as u32,
        G_LAN_GAME_NAME_LENGTH as u32,
        G_LAN_GAME_NAME_RESERVED_LENGTH as u32,
        G_LAN_MAX_CHAT_LENGTH as u32,
        M_LAN_MAX_OPTIONS_LENGTH as u32,
        G_MAX_SERIAL_LENGTH as u32,
        11,
        3,
    ];
    chk.eq(tail.to_vec(), expect_tail, || "constants (MSG_MAX, name lengths, options length, serial length, RET_MAX, LANCHAT_MAX)".into());
    assert_eq!(LAN_MESSAGE_SIZE, out[0] as usize);
    chk.finish();
}

#[test]
fn message_type_strings_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut chk = Check::new("lan_api GetMessageTypeString");
    for t in (0..40u32).chain([0xFFFF_FFFF, 1000]) {
        let mut buf = [0 as c_char; 256];
        let n = unsafe { ref_la_typestring(t, buf.as_mut_ptr(), buf.len() as c_int) } as usize;
        let c: Vec<u8> = buf[..n].iter().map(|&x| x as u8).collect();
        chk.eq(get_message_type_string(t).into_bytes(), c, || format!("type {t}"));
    }
    chk.finish();
}
