#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANAPIhandlers.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/lan_api_handlers.rs` against the C++ (GitHub #215, group lan5).
//!
//! # Method
//!
//! `differential/wrap_draft_lanhandlers.cpp` compiles LANAPIhandlers.cpp (all 16 handlers) verbatim, with LANAPI.cpp's `sendMessage`, `RequestGameJoin`,
//! `RequestGameOptions`, `fillInLANMessage`, the list functions and `AmIHost`, LANGameInfo.cpp's slot/game members and `ParseGameOptionsString`, and
//! QuotedPrintable.cpp's `UnicodeStringToQuotedPrintable`, all on the REAL `LANGameInfo`/`GameInfo` (the verified GameInfo test's build). What the cuts call
//! outside themselves (the transport, the callbacks, the preferences file, the map cache's stand-in, the clock, the registry serial, the GUI calls) is
//! logged; this file's `Host` answers and logs identically. A program is a configuration (clock, local IP, names, CRCs, serial, packet router) and
//! operations (listed at `ref_lh_run`): set up games and lobby players, set the LAN state, and deliver a received `LANMessage` to one of the 16 handlers.
//! After every operation the whole state (games with every slot field, players, the LAN API's fields) is dumped as text, preceded by the datagrams the
//! operation sent (address, port, length, type, a hash of the bytes) and the callbacks it made; the two texts are compared whole.
//!
//! Excluded inputs (both sides skip them): (1) a game announcement for the game I am in, from another IP, before it has started: the parse rewrites my own
//! slots, which changes `AmIHost()` in the middle of the handler, but the draft's `LanGameInfoHost` can only hold a snapshot of it (the host cannot see the LANAPI's
//! games): a design limit of the draft, reported on #215. (2) Chat types outside 0..=2 and join-deny reasons outside 0..=10 (upstream passes the wire value through to `OnChat`/`OnGameJoin`, the draft clamps it to an enumerator). The state "in the lobby with a current game" is not generated: upstream's `handleRequestGameLeave` would leave `m_currentGame` dangling there.
//! `RequestGameJoin` and `RequestGameOptions`, which the handlers call, are the draft's `lan_api.rs` functions, tested here through the handlers.
//! Not compared: the real `LANPreferences` file I/O (its write is logged), the socket (`Transport::update`).

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::crc::Crc;
use rts_lockstep_engine::common::game_info::*;
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::map_cache::unicode_string_to_quoted_printable;
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
    fn ref_lh_run(input: *const c_uint, n: c_int, out: *mut c_char, cap: c_int) -> c_int;
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

fn wide(u: &[u16]) -> String {
    let mut r = String::new();
    for &c in u {
        if (0x20..0x7f).contains(&c) && c != u16::from(b'\\') {
            r.push(c as u8 as char);
        } else {
            r.push_str(&format!("\\u{c:04x}"));
        }
    }
    r
}
fn bs(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}
fn swap_prefix(p: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let s = p.to_ascii_lowercase();
    if s.starts_with(from) {
        let mut r = to.to_vec();
        r.extend_from_slice(&s[from.len()..]);
        r
    } else {
        s
    }
}

// ---------------------------------------------------------------------------------------------------------------
// the host
// ---------------------------------------------------------------------------------------------------------------

struct Host {
    now: Cell<u32>,
    tick: u32,
    local_ip: Cell<u32>,
    exe_crc: u32,
    ini_crc: u32,
    serial: Vec<u8>,
    packet_router: bool,
    am_i_host: Cell<bool>,
    log: RefCell<String>,
}

impl Host {
    fn l(&self, s: String) {
        let mut g = self.log.borrow_mut();
        g.push_str(&s);
        g.push('\n');
    }
}

impl GameInfoHost for Host {
    fn game_text(&mut self, label: &str) -> Vec<u16> {
        "T:".bytes().chain(label.bytes()).map(u16::from).collect()
    }
    fn tick_count(&mut self) -> u32 {
        let t = self.tick;
        self.tick = self.tick.wrapping_add(7);
        t
    }
    fn default_starting_cash(&self) -> Money {
        Money::new()
    }
    fn net_crc_interval(&self) -> i32 {
        100
    }
    fn has_map_cache(&self) -> bool {
        true
    }
    fn map_cache_find(&self, lower: &[u8]) -> Option<MapMeta> {
        match lower {
            b"data\\alpha\\alpha.map" => Some(MapMeta { num_players: 4, crc: 0x1234 }),
            b"data\\beta\\beta.map" => Some(MapMeta { num_players: 2, crc: 0x77 }),
            _ => None,
        }
    }
    fn file_exists(&mut self, path: &[u8]) -> bool {
        let mut s = String::from("open ");
        s.push_str(&bs(path));
        self.l(s);
        let mut h = 12345u32;
        for &b in path {
            h = h.wrapping_mul(31).wrapping_add(u32::from(b));
        }
        (h >> 7) % 3 == 0
    }
    fn real_map_path_to_portable(&self, path: &[u8]) -> Vec<u8> {
        swap_prefix(path, b"data\\", b"maps\\")
    }
    fn portable_map_path_to_real(&self, path: &[u8]) -> Vec<u8> {
        swap_prefix(path, b"maps\\", b"data\\")
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
        Ok(0x123456)
    }
    fn player_template_count(&self) -> i32 {
        4
    }
    fn player_template_display_name(&self, n: i32) -> Result<Vec<u16>, IniError> {
        if !(0..4).contains(&n) {
            return Err(IniError::undefined(rts_lockstep_engine::common::ini::Undefined::NullObject));
        }
        Ok(format!("PT{n}").bytes().map(u16::from).collect())
    }
    fn record_income(&mut self, _player_index: i32) {}
}

impl LanGameInfoHost for Host {
    fn has_lan(&self) -> bool {
        true
    }
    fn lan_local_ip(&self) -> u32 {
        self.local_ip.get()
    }
    fn lan_reset_game_start_timer(&mut self) {
        self.l("lan resetGameStartTimer".into());
    }
    fn lan_my_game_is_this(&self) -> bool {
        true // the only caller is handleRequestGameLeave, on my game
    }
    fn lan_am_i_host(&self) -> bool {
        self.am_i_host.get()
    }
    fn lan_enable_start_button(&mut self, enabled: bool) {
        self.l(format!("lan enableStartButton {}", enabled as i32));
    }
    fn lan_request_has_map(&mut self) {
        self.l("lan requestHasMap".into());
    }
    fn lan_update_slot_list(&mut self) {
        self.l("lan updateSlotList".into());
    }
    fn lan_update_game_options(&mut self) {
        self.l("lan updateGameOptions".into());
    }
    fn time_get_time(&self) -> u32 {
        let t = self.now.get().wrapping_add(11);
        self.now.set(t);
        t
    }
}

impl LanApiHost for Host {
    fn on_game_list(&mut self, games: &[LANGameInfo]) {
        let s: String = games.iter().map(|g| format!("{}|", wide(g.get_name().str()))).collect();
        self.l(format!("cb gamelist {} [{}]", games.len(), s));
    }
    fn on_player_list(&mut self, players: &[LANPlayer]) {
        let s: String = players.iter().map(|p| format!("{}@{}|", wide(p.get_name().str()), p.get_ip())).collect();
        self.l(format!("cb playerlist {} [{}]", players.len(), s));
    }
    fn on_game_join(&mut self, ret: ReturnType, game: Option<&LANGameInfo>) {
        self.l(format!("cb gamejoin {} [{}]", ret as i32, game.map(|g| wide(g.get_name().str())).unwrap_or_else(|| "-".into())));
    }
    fn on_player_join(&mut self, slot: i32, player_name: &UnicodeString) {
        self.l(format!("cb playerjoin {} [{}]", slot, wide(player_name.str())));
    }
    fn on_host_leave(&mut self) {
        self.l("cb hostleave".into());
    }
    fn on_player_leave(&mut self, player: &UnicodeString) {
        self.l(format!("cb playerleave [{}]", wide(player.str())));
    }
    fn on_accept(&mut self, player_ip: u32, status: bool) {
        self.l(format!("cb accept {} {}", player_ip, status as i32));
    }
    fn on_has_map(&mut self, player_ip: u32, status: bool) {
        self.l(format!("cb hasmap {} {}", player_ip, status as i32));
    }
    fn on_chat(&mut self, player: &UnicodeString, ip: u32, message: &UnicodeString, format: ChatType) {
        self.l(format!("cb chat [{}] {} [{}] {}", wide(player.str()), ip, wide(message.str()), format as i32));
    }
    fn on_game_start(&mut self) {
        self.l("cb gamestart".into());
    }
    fn on_game_start_timer(&mut self, seconds: i32) {
        self.l(format!("cb gamestarttimer {seconds}"));
    }
    fn on_game_options(&mut self, player_ip: u32, player_slot: i32, options: &[u8]) {
        self.l(format!("cb gameoptions {} {} [{}]", player_ip, player_slot, bs(options)));
    }
    fn on_game_create(&mut self, _ret: ReturnType) {}
    fn on_name_change(&mut self, ip: u32, new_name: &UnicodeString) {
        self.l(format!("cb namechange {} [{}]", ip, wide(new_name.str())));
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
    fn map_num_players(&self, map: &[u8]) -> Option<i32> {
        self.map_cache_find(&map.to_ascii_lowercase()).map(|m| m.num_players)
    }
    fn map_to_portable(&self, map: &[u8]) -> Vec<u8> {
        swap_prefix(map, b"data\\", b"maps\\")
    }
    fn no_map_message(&mut self, _map: &[u8]) -> UnicodeString {
        UnicodeString::new()
    }
    fn preferred_map(&mut self) -> Vec<u8> {
        Vec::new()
    }
    fn remember_remote_ip(&mut self, entry: &[u8]) {
        self.l(format!("prefs write RemoteIP0 [{}]", bs(entry)));
    }
    fn quoted_printable(&self, name: &UnicodeString) -> Vec<u8> {
        unicode_string_to_quoted_printable(name.str())
    }
    fn read_local_motd(&mut self) -> Option<Vec<u8>> {
        None
    }
    fn get_tick_count(&self) -> u32 {
        0
    }
}

// ---------------------------------------------------------------------------------------------------------------
// programs
// ---------------------------------------------------------------------------------------------------------------

const LOCAL: u32 = 0xc0a8_0001;
const NAMES: &[&str] = &["Alice", "alice", "BOB", "Carol", "Dave", "", "Zed Q", "Hosty", "TwelveCharsX", "ThirteenChars"];
const GAMES: &[&str] = &["GameA", "gamea", "GameB", "Sixteen Chars Nm", "Z"];
const MAPS: &[&str] = &["Maps\\Alpha\\Alpha.map", "data\\alpha\\alpha.map", "Data\\Beta\\Beta.map", "data\\gamma\\g.map", "maps\\gamma\\g.map", ""];
const SERIALS: &[&str] = &["", "AAAA", "BBBB", "0123456789012345678901", "01234567890123456789012", "0123456789012345678901X", "<Munkee>"];
const ULOGINS: &[&str] = &["", "u", "h", "user"];
const BLOGINS: &[&[u8]] = &[b"", b"u", b"h", b"\xe9", b"\xff"];

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
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn pick<'a>(g: &mut Gen, l: &'a [&'a str]) -> &'a str {
    l[g.below(l.len() as u32) as usize]
}
fn ip(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => 0,
        1 | 2 | 3 => LOCAL,
        4 | 5 => LOCAL + 1 + g.below(3),
        6 => 0x0a00_0001,
        _ => g.u32(),
    }
}
fn other_ip(g: &mut Gen) -> u32 {
    LOCAL + 1 + g.below(4)
}

fn new_host(now: u32, local: u32, exe: u32, ini: u32, serial: Vec<u8>, pr: bool) -> Host {
    Host { now: Cell::new(now), tick: 0, local_ip: Cell::new(local), exe_crc: exe, ini_crc: ini, serial, packet_router: pr, am_i_host: Cell::new(false), log: RefCell::new(String::new()) }
}

/// An options string of a random game (what a host announces), sometimes damaged.
fn options_string(g: &mut Gen) -> Vec<u8> {
    let mut h = new_host(0, LOCAL, 0, 0, vec![], false);
    let mut gi = LANGameInfo::new(&mut h);
    gi.info.in_game = true;
    gi.info.map_name = pick(g, MAPS).as_bytes().to_vec();
    gi.info.seed = g.below(1000) as i32;
    for i in 0..8 {
        let st = g.below(7) as i32;
        let n = units(pick(g, NAMES));
        let ipv = ip(g);
        let mut s = GameSlot::new();
        s.set_state(st, &n, ipv, &mut h);
        if g.below(2) == 0 {
            s.set_accept();
        }
        s.set_map_availability(g.below(2) == 0);
        s.set_player_template(g.below(6) as i32 - 2);
        gi.info.slots[i] = s;
    }
    let Ok(mut o) = game_info_to_ascii_string(Some(&gi.info), &h) else { return Vec::new() };
    match g.below(8) {
        1 if !o.is_empty() => {
            let p = g.below(o.len() as u32) as usize;
            o.remove(p);
        }
        2 if !o.is_empty() => {
            let p = g.below(o.len() as u32) as usize;
            o.insert(p, b'x');
        }
        3 if !o.is_empty() => {
            let p = g.below(o.len() as u32) as usize;
            o.truncate(p);
        }
        4 => o.clear(),
        _ => {}
    }
    o
}

/// What the generator knows of a game it added (so that messages can name its players, its map and its serials).
#[derive(Clone)]
struct GameModel {
    name: &'static str,
    map: &'static str,
    slot_name: [&'static str; 8],
    slot_ip: [u32; 8],
    slot_serial: [&'static str; 8],
    human: [bool; 8],
}

fn flip_case(s: &str, g: &mut Gen) -> String {
    if g.below(2) == 0 {
        s.to_ascii_uppercase()
    } else {
        s.to_ascii_lowercase()
    }
}

fn message(g: &mut Gen, which: u32, models: &[GameModel]) -> (LANMessage, u32) {
    let mut m = LANMessage::new();
    m.set_message_type(which as i32);
    m.set_name(&units(pick(g, NAMES)));
    let mut sender = ip(g);
    let model = if models.is_empty() || g.below(4) == 0 { None } else { Some(&models[g.below(models.len() as u32) as usize]) };
    let human_slot = model.and_then(|md| {
        let hs: Vec<usize> = (0..8).filter(|&i| md.human[i]).collect();
        if hs.is_empty() { None } else { Some(hs[g.below(hs.len() as u32) as usize]) }
    });
    m.set_user_name(BLOGINS[g.below(BLOGINS.len() as u32) as usize]);
    m.set_host_name(BLOGINS[g.below(BLOGINS.len() as u32) as usize]);
    let gname = units(model.map_or_else(|| pick(g, GAMES), |md| md.name));
    if let (Some(md), Some(k)) = (model, human_slot) {
        if which == 15 || which == 7 || which == 4 || which == 11 {
            if g.below(4) != 0 {
                let nm = if which == 4 && g.below(2) == 0 { flip_case(md.slot_name[k], g) } else { md.slot_name[k].to_string() };
                m.set_name(&units(&nm));
            }
        }
        if which != 0 && which != 4 && g.below(5) != 0 {
            sender = md.slot_ip[k];
        }
        if which == 15 && g.below(4) != 0 {
            sender = md.slot_ip[k];
        }
    }
    match which {
        1 => {
            m.set_game_name(&gname);
            m.set_game_info_in_progress(g.below(4) == 0);
            m.set_game_info_is_direct_connect(g.below(3) == 0);
            let o = options_string(g);
            m.set_game_info_options(&o);
        }
        4 => {
            m.set_game_to_join_game_ip(if g.below(5) == 0 { ip(g) } else { LOCAL });
            m.set_game_to_join_exe_crc(g.u32());
            m.set_game_to_join_ini_crc(g.u32());
            let serial = match (model, human_slot) {
                (Some(md), Some(k)) if g.below(3) == 0 => md.slot_serial[k],
                _ => pick(g, SERIALS),
            };
            m.set_game_to_join_serial(serial.as_bytes());
        }
        5 | 6 => {
            m.set_game_name(&gname);
            m.set_joined_game_ip(ip(g));
            m.set_joined_player_ip(if g.below(5) == 0 { ip(g) } else { LOCAL });
            m.set_joined_slot_or_reason(if which == 5 { g.below(11) as i32 - 1 } else { g.below(11) as i32 });
        }
        7 => m.set_game_name(&gname),
        9 => m.set_accept_is_accepted(g.below(2) == 0),
        10 => {
            let map = model.map_or_else(|| pick(g, MAPS), |md| md.map);
            let portable = swap_prefix(map.as_bytes(), b"data\\", b"maps\\");
            let mut c = Crc::new();
            c.compute_crc(&portable);
            m.set_map_status_map_crc(if g.below(4) == 0 { g.u32() } else { c.get() });
            m.set_map_status_has_map(g.below(2) == 0);
        }
        11 => {
            m.set_game_name(&gname);
            m.set_chat_type(g.below(3) as i32);
            m.set_chat_message(&units(pick(g, NAMES)));
        }
        13 => m.set_start_timer_seconds(g.below(12) as i32 - 1),
        14 => {
            let o = options_string(g);
            m.set_game_options_options(&o);
        }
        _ => {}
    }
    (m, sender)
}

fn program(g: &mut Gen) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    v.push(g.below(100_000));
    v.push(LOCAL);
    push_units(&mut v, &units(pick(g, NAMES)));
    push_bytes(&mut v, BLOGINS[g.below(5) as usize]);
    push_bytes(&mut v, BLOGINS[g.below(5) as usize]);
    v.push(g.u32());
    v.push(g.u32());
    push_bytes(&mut v, pick(g, SERIALS).as_bytes());
    v.push(g.below(2));
    v.push(g.below(10_000));
    v.push(if g.below(4) == 0 { 0 } else { 0xFFFF_FFFF });
    let ngames = g.below(4);
    let mut models: Vec<GameModel> = Vec::new();
    for _ in 0..ngames {
        v.push(1);
        let gname = pick(g, GAMES);
        push_units(&mut v, &units(gname));
        v.push(u32::from(g.below(4) == 0));
        v.push(u32::from(g.below(5) == 0));
        v.push(u32::from(g.below(5) != 0));
        let map = if g.below(2) == 0 { ["data\\alpha\\alpha.map", "Data\\Beta\\Beta.map"][g.below(2) as usize] } else { pick(g, MAPS) };
        push_bytes(&mut v, map.as_bytes());
        v.push(g.below(1000));
        let host_mine = g.below(2) == 0;
        let mut md = GameModel { name: gname, map, slot_name: [""; 8], slot_ip: [0; 8], slot_serial: [""; 8], human: [false; 8] };
        let crowded = g.below(2) == 0;
        for i in 0..8u32 {
            let human = if i == 0 { g.below(8) != 0 } else { g.below(if crowded { 2 } else { 3 }) == 0 };
            let st = if human { 5 } else { [0, 0, 0, 1, 2, 3, 4][g.below(7) as usize] };
            v.push(st);
            let nm = pick(g, NAMES);
            push_units(&mut v, &units(nm));
            let ipv = if i == 0 && host_mine && human { LOCAL } else if g.below(3) == 0 { LOCAL } else { other_ip(g) };
            v.push(ipv);
            let sr = pick(g, SERIALS);
            push_bytes(&mut v, sr.as_bytes());
            md.slot_name[i as usize] = nm;
            md.slot_ip[i as usize] = ipv;
            md.slot_serial[i as usize] = sr;
            md.human[i as usize] = human;
            push_units(&mut v, &units(pick(g, ULOGINS)));
            push_units(&mut v, &units(pick(g, ULOGINS)));
            v.push(g.below(50_000));
            v.push(g.below(2) | (g.below(2) << 1) | ((if g.below(3) == 0 { 0 } else { g.below(8) }) << 4));
        }
        models.push(md);
    }
    let nplayers = g.below(4);
    for _ in 0..nplayers {
        v.push(2);
        push_units(&mut v, &units(pick(g, NAMES)));
        v.push(if g.below(3) == 0 { LOCAL } else { other_ip(g) });
        push_units(&mut v, &units(pick(g, ULOGINS)));
        push_units(&mut v, &units(pick(g, ULOGINS)));
        v.push(g.below(50_000));
    }
    let nops = 12 + g.below(30);
    for _ in 0..nops {
        match g.below(14) {
            0 => {
                // m_currentGame is NULL in the lobby and set outside it: a game I am in is never in the lobby state
                let k = g.below(ngames + 2).wrapping_sub(1);
                v.push(8);
                v.push(k);
                v.push(9);
                v.push(u32::from((k as i32) < 0));
            }
            1 => {
                v.push(12);
                push_units(&mut v, &units(pick(g, NAMES)));
            }
            2 => {
                v.push(10);
                v.push(if g.below(2) == 0 { 1 } else { g.below(4) });
            }
            3 => {
                v.push(15);
                v.push(g.below(5000));
            }
            4 => {
                v.push(16);
                v.push(if g.below(2) == 0 { 0 } else { g.below(5000) });
            }
            5 => {
                v.push(17);
                v.push(g.below(2));
            }
            6 => {
                v.push(13);
                v.push(g.below(500));
            }
            7 => {
                v.push(14);
                v.push(ip(g));
            }
            8 => {
                v.push(3);
                v.push(g.below(ngames + 1));
            }
            9 => {
                v.push(4);
                v.push(g.below(nplayers + 1));
            }
            _ => {
                let which = g.below(16);
                v.push(40);
                v.push(which);
                let (m, sender) = message(g, which, &models);
                v.push(sender);
                let mut b = m.as_bytes().to_vec();
                while b.len() % 4 != 0 {
                    b.push(0);
                }
                for c in b.chunks(4) {
                    v.push(u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
                }
            }
        }
    }
    v
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

fn dump(h: &mut Host, a: &LANAPI<Host>) {
    h.l(format!(
        "state: lobby {} cur {} pend {} exp {} gst {} gss {} lrt {} ip {} dcip {} name [{}] opt [{}] clock {}",
        a.in_lobby as i32,
        a.current_game.map_or(-1, |c| c as i32),
        a.pending_action as i32,
        a.expiration,
        a.game_start_time,
        a.game_start_seconds,
        a.last_resend_time,
        a.local_ip,
        a.direct_connect_remote_ip,
        wide(a.name.str()),
        bs(a.last_gameopt.str()),
        a.host.now.get()
    ));
    h.l(format!("games {}", a.games.len()));
    for (gi, g) in a.games.iter().enumerate() {
        let i = &g.info;
        h.l(format!(
            "game {gi}: in {} prog {} ip {} map [{}] crc {} seed {} last {} name [{}] direct {}",
            i.in_game as i32,
            i.in_progress as i32,
            i.local_ip,
            bs(&i.map_name),
            i.map_crc,
            i.seed,
            g.get_last_heard(),
            wide(g.get_name().str()),
            g.get_is_direct_connect() as i32
        ));
        for k in 0..8 {
            let s = &i.slots[k];
            let x = &g.extras[k];
            h.l(format!(
                " slot {k}: st {} acc {} map {} name [{}] ip {} port {} tmpl {} team {} col {} sp {} last {} serial [{}] login [{}] host [{}]",
                s.state,
                s.is_accepted as i32,
                s.has_map as i32,
                wide(&s.name),
                s.ip,
                s.port,
                s.player_template,
                s.team_number,
                s.color,
                s.start_pos,
                x.get_last_heard(),
                bs(x.get_serial().str()),
                wide(x.user_login().str()),
                wide(x.user_host().str())
            ));
        }
    }
    h.l(format!("players {}", a.lobby_players.len()));
    for p in &a.lobby_players {
        h.l(format!(" player [{}] login [{}] host [{}] last {} ip {}", wide(p.get_name().str()), wide(p.get_login().str()), wide(p.get_host().str()), p.get_last_heard(), p.get_ip()));
    }
}

/// The datagrams the draft queued, as the C++ stand-in logs them (the transport encrypts each message in place, so it is decrypted first).
fn drain_sent(a: &mut LANAPI<Host>, out: &mut String) {
    for m in a.transport.out_buffer.iter_mut() {
        if m.length != 0 {
            let n = m.length as usize;
            let mut image = Vec::with_capacity(6 + n);
            image.extend_from_slice(&m.header.crc.to_le_bytes());
            image.extend_from_slice(&m.header.magic.to_le_bytes());
            image.extend_from_slice(&m.data[..n]);
            let total = image.len();
            decrypt_buf(&mut image, total);
            let ty = i32::from_le_bytes([image[6], image[7], image[8], image[9]]);
            out.push_str(&format!("send ip {} port {} len {} type {} hash {}\n", m.addr, m.port, m.length, ty, hash_bytes(&image[6..])));
            m.length = 0;
        }
    }
}

fn rust_run(w: &[u32]) -> String {
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
    let mut a = LANAPI::new(new_host(now, local, exe, ini, serial, pr));
    a.local_ip = local;
    a.name = uni(&name);
    a.user_name = asc(&user);
    a.host_name = asc(&hostn);
    a.action_timeout = timeout;
    a.broadcast_addr = bcast;
    a.in_lobby = true;
    let mut out = String::new();
    let mut h0 = new_host(0, 0, 0, 0, vec![], false);
    let _ = &mut h0;
    // the initial dump goes through a scratch host (the dump only reads the API's fields)
    let mut scratch = new_host(0, 0, 0, 0, vec![], false);
    dump(&mut scratch, &a);
    out.push_str(&scratch.log.borrow());
    while rd.i < w.len() {
        let op = rd.next();
        a.host.l(format!("op {op}"));
        a.host.am_i_host.set(a.current_game.map_or(false, |c| a.games[c].get_ip(0) == a.local_ip));
        let mark = a.host.log.borrow().len();
        match op {
            1 => {
                let mut g = LANGameInfo::new(&mut a.host);
                g.set_name(uni(&rd.units()));
                g.set_is_direct_connect(rd.next() != 0);
                g.info.in_progress = rd.next() != 0;
                g.info.in_game = rd.next() != 0;
                g.info.map_name = rd.bytes();
                g.info.seed = rd.next() as i32;
                for i in 0..8 {
                    let st = rd.next();
                    let nm = rd.units();
                    let ipv = rd.next();
                    let serial = rd.bytes();
                    let login = rd.units();
                    let hostu = rd.units();
                    let lh = rd.next();
                    let flags = rd.next();
                    let mut s = GameSlot::new();
                    s.set_state((st % 7) as i32, &nm, ipv, &mut a.host);
                    let mut x = LANSlotExtra::new();
                    x.set_login(uni(&login));
                    x.set_host(uni(&hostu));
                    x.set_serial(asc(&serial));
                    x.set_last_heard(lh);
                    s.set_player_template(((flags >> 4) & 7) as i32 - 2);
                    if flags & 1 != 0 {
                        s.set_accept();
                    } else {
                        s.un_accept();
                    }
                    s.set_map_availability(flags & 2 != 0);
                    g.set_slot(i, s, x);
                }
                a.add_game(g);
            }
            2 => {
                let mut p = LANPlayer::new();
                p.set_name(uni(&rd.units()));
                p.set_ip(rd.next());
                p.set_login(uni(&rd.units()));
                p.set_host(uni(&rd.units()));
                p.set_last_heard(rd.next());
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
                a.host.local_ip.set(k);
            }
            12 => a.name = uni(&rd.units()),
            13 => {
                let dt = rd.next();
                a.host.now.set(a.host.now.get().wrapping_add(dt));
            }
            14 => a.direct_connect_remote_ip = rd.next(),
            15 => a.expiration = rd.next(),
            16 => a.game_start_time = rd.next(),
            17 => a.host.packet_router = rd.next() != 0,
            18 => a.user_name = asc(&rd.bytes()),
            19 => a.host_name = asc(&rd.bytes()),
            20 => a.host.serial = rd.bytes(),
            40 => {
                let which = rd.next();
                let sender = rd.next();
                let mut bytes = Vec::new();
                for _ in 0..LAN_MESSAGE_SIZE.div_ceil(4) {
                    bytes.extend_from_slice(&rd.next().to_le_bytes());
                }
                let m = LANMessage::from_bytes(&bytes[..LAN_MESSAGE_SIZE]);
                // excluded input (see the module doc)
                let skip = which == 1
                    && a.current_game.map_or(false, |c| !a.games[c].info.in_progress)
                    && sender != a.local_ip
                    && sender != a.direct_connect_remote_ip
                    && a.lookup_game(&m.game_name()) == a.current_game
                    && a.current_game.is_some();
                if skip {
                    a.host.l("skipped".into());
                } else {
                    match which {
                        0 => a.handle_request_locations(&m, sender),
                        1 => a.handle_game_announce(&m, sender),
                        2 => a.handle_lobby_announce(&m, sender),
                        3 => a.handle_request_game_info(&m, sender),
                        4 => a.handle_request_join(&m, sender),
                        5 => a.handle_join_accept(&m, sender),
                        6 => a.handle_join_deny(&m, sender),
                        7 => a.handle_request_game_leave(&m, sender),
                        8 => a.handle_request_lobby_leave(&m, sender),
                        9 => a.handle_set_accept(&m, sender),
                        10 => a.handle_has_map(&m, sender),
                        11 => a.handle_chat(&m, sender),
                        12 => a.handle_game_start(&m, sender),
                        13 => a.handle_game_start_timer(&m, sender),
                        14 => a.handle_game_options(&m, sender),
                        _ => a.handle_in_active(&m, sender),
                    }
                }
            }
            _ => {}
        }
        // the log of this operation: "op N", then the datagrams, then the callbacks
        let all = std::mem::take(&mut *a.host.log.borrow_mut());
        let (opline, ev) = all.split_at(mark);
        out.push_str(opline);
        drain_sent(&mut a, &mut out);
        out.push_str(ev);
        let mut s2 = new_host(0, 0, 0, 0, vec![], false);
        s2.now.set(a.host.now.get());
        dump(&mut s2, &a);
        out.push_str(&s2.log.borrow());
    }
    out
}

fn cpp_run(w: &[u32]) -> String {
    let mut out = vec![0u8; 16 << 20];
    let n = unsafe { ref_lh_run(w.as_ptr(), w.len() as c_int, out.as_mut_ptr() as *mut c_char, out.len() as c_int) } as usize;
    assert!(n <= out.len(), "log too long");
    out.truncate(n);
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn lan_handlers_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("LH_CASES", 20_000);
    let mut g = Gen::new(0x1a4a_5001);
    let mut chk = Check::new("lan_api_handlers programs");
    for i in 0..n {
        let w = program(&mut g);
        let r = rust_run(&w);
        let c = cpp_run(&w);
        if r != c && std::env::var("LH_DEBUG").is_ok() && chk.mismatches < 3 {
            let rl: Vec<&str> = r.lines().collect();
            let cl: Vec<&str> = c.lines().collect();
            let k = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
            println!("case {i}: first difference at line {k}");
            for j in k.saturating_sub(8)..(k + 3) {
                let mark = if rl.get(j) != cl.get(j) { ">>" } else { "  " };
                println!("{mark} rust {:?}\n{mark} c++  {:?}", rl.get(j), cl.get(j));
            }
        }
        chk.eq(r == c, true, || format!("lan handlers case {i}"));
    }
    println!("{n} programs");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
