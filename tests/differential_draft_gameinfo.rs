#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/LANGameInfo.cpp
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/StagingRoomGameInfo.cpp
//   GeneralsMD/Code/GameEngine/Include/GameNetwork/LANGameInfo.h, LANPlayer.h, GameSpy/StagingRoomGameInfo.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/lan_game_info.rs` and `staging_room_game_info.rs` against the C++ (GitHub #215, group gameinfo).
//!
//! # Method
//!
//! `differential/wrap_draft_gameinfo.cpp` compiles the unchanged headers and, cut verbatim by build.rs, LANGameInfo.cpp's
//! `LANGameSlot` and `LANGameInfo` members, `GenerateGameOptionsString`, `ParseGameOptionsString` (the one edit: a `.TheEmptyString`
//! typo MSVC 6 accepted becomes `::`) and StagingRoomGameInfo.cpp from `GameSpyGameSlot::GameSpyGameSlot` and `setPingString` to the end of
//! the file (the staging room's `startGame`, `launchGame`, both results packets), on top of the GameInfo.cpp build of the verified
//! GameInfo test. What the cuts call outside themselves (`TheLAN`, `TheGameSpyInfo`, `TheNAT`, `TheNetwork`, `TheGameLogic`, the player
//! list and score keepers, the victory conditions, the buddy queue, ...) are stand-ins listed in the wrapper's header: they answer from a
//! configuration of 14 words and log every call; this file's `Host` answers and logs identically. A program is a configuration and 6-word
//! operations (listed at `run_op` in the wrapper) on a fresh `GameSpyStagingRoom` and `LANGameInfo`; every operation logs a line, the
//! games are dumped field by field, and the two logs are compared whole. The GameInfo base (verified separately by
//! `differential_game_info.rs`) is the verified port's; the options string a LAN game parses is the staging room's own, mutated
//! (a character deleted, inserted, replaced, or the string cut).
//!
//! An operation the Rust reports as undefined (an `Err` from the verified GameInfo code: a byte of 0x80 or more through `tolower`, ...)
//! ends the Rust run there and the C++ runs only the operations before it. The draft's `parse_game_options_string` turns such an `Err` into
//! `false`, so the test checks the same parse on a copy first.
//!
//! Not compared: `WideCharStringToMultiByte` of the buddy location (the stand-in is UTF-8, the draft truncates to a byte; names here are
//! ASCII), `GetLocalChatConnectionAddress` and `GameSpyStartGame`/`GameSpyLaunchGame` (not translated; not in the cut).

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::game_info::*;
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::drafts::network::lan_game_info::*;
use rts_lockstep_engine::drafts::network::staging_room_game_info::*;
use std::os::raw::{c_char, c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_dgi_run(cfg: *const c_uint, ops: *const c_uint, n: c_int, strs: *const c_char, nstrs: c_int, out: *mut c_char, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

const POOL: &[&str] = &[
    "Alice", "alice", "BOB", "Carol", "Dave", "", "Zed Q", "host", "Hosty", "Maps\\Alpha\\Alpha.map", "maps\\beta\\beta.map",
    "data\\alpha\\alpha.map", "data\\other\\x.map", "Maps\\Gamma\\gamma.map", "x", "Eve", "eve", "Mallory", "Trent", "Wendy", "Zed", "a b",
    "Peggy", "Victor", "Oscar", "Sybil", "localhost", "ladder.example", "L1", "L2",
];

fn pool(i: u32) -> &'static [u8] {
    POOL[i as usize % POOL.len()].as_bytes()
}
fn uname(i: u32) -> Vec<u16> {
    pool(i).iter().map(|&b| u16::from(b)).collect()
}
fn uni(i: u32) -> UnicodeString {
    UnicodeString::try_from_chars(&uname(i)).unwrap()
}
fn asc(b: &[u8]) -> AsciiString {
    AsciiString::try_from_bytes(b).unwrap()
}

fn hash_bytes(s: &[u8], salt: u32) -> u32 {
    let mut h = salt.wrapping_mul(2654435761).wrapping_add(17);
    for &b in s {
        h = h.wrapping_mul(31).wrapping_add(u32::from(b));
    }
    h ^ (h >> 15)
}
fn hash_int(v: u32, salt: u32) -> u32 {
    let h = v.wrapping_add(0x9e37_79b9).wrapping_mul(2654435761).wrapping_add(salt.wrapping_mul(40503));
    h ^ (h >> 13)
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

/// The host: the configuration, and the log of the stand-ins' calls.
#[derive(Clone)]
struct Host {
    cfg: [u32; 14],
    tick: u32,
    time: std::cell::Cell<u32>,
    log: std::cell::RefCell<String>,
    nat: bool,
    network: bool,
    lan: bool,
    my_game: bool,
    pending: Vec<u8>,
    fps_limit: bool,
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
        self.lan
    }
    fn lan_local_ip(&self) -> u32 {
        self.cfg[0]
    }
    fn lan_reset_game_start_timer(&mut self) {
        self.l("lan resetGameStartTimer".into());
    }
    fn lan_my_game_is_this(&self) -> bool {
        self.my_game
    }
    fn lan_am_i_host(&self) -> bool {
        (self.cfg[1] >> 1) & 1 != 0
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
        let t = self.time.get().wrapping_add(11);
        self.time.set(t);
        t
    }
}

impl StagingRoomHost for Host {
    fn local_name(&self) -> AsciiString {
        asc(pool(self.cfg[2]))
    }
    fn internal_ip(&self) -> u32 {
        self.cfg[3]
    }
    fn local_profile_id(&self) -> i32 {
        self.cfg[4] as i32
    }
    fn find_player_info(&self, name: &AsciiString) -> Option<StagingPlayerInfo> {
        let n = name.str();
        if !POOL.iter().any(|p| p.as_bytes() == n) {
            return None;
        }
        let h = hash_bytes(n, self.cfg[13]);
        if h % 3 == 0 {
            return None;
        }
        Some(StagingPlayerInfo { profile_id: (h & 0xffff) as i32, locale: asc(format!("L{}", (h >> 3) & 7).as_bytes()), rank_points: ((h >> 5) & 0xfff) as i32, side: ((h >> 9) & 3) as i32 })
    }
    fn ping_value(&self, ping_str: &AsciiString) -> i32 {
        (hash_bytes(ping_str.str(), 9) % 500) as i32
    }
    fn did_player_preorder(&self, profile_id: i32) -> bool {
        (self.cfg[5] >> (profile_id & 31)) & 1 != 0
    }
    fn leave_staging_room(&mut self) -> bool {
        self.l("gamespyinfo leaveStagingRoom".into());
        true
    }
    fn nat_exists(&self) -> bool {
        self.nat
    }
    fn nat_delete(&mut self) {
        self.nat = false;
    }
    fn nat_create_and_establish(&mut self, local_slot: i32, local_ip: u32) {
        self.nat = true;
        self.l(format!("nat attachSlotList slots 8 local {local_slot} ip {local_ip}"));
        self.l("nat establishConnectionPaths".into());
    }
    fn nat_slot_port(&self, slot: i32) -> Option<u16> {
        if !self.nat {
            return None;
        }
        self.l(format!("nat getSlotPort {slot}"));
        Some(self.cfg[8].wrapping_add(slot.wrapping_mul(3) as u32) as u16)
    }
    fn network_exists(&self) -> bool {
        self.network
    }
    fn network_delete(&mut self) {
        self.network = false;
        self.l("network delete".into());
    }
    fn network_create(&mut self) {
        self.network = true;
        self.l("network create".into());
        self.l("network init".into());
    }
    fn network_set_local_address(&mut self, ip: u32, port: u16) {
        self.l(format!("network setLocalAddress {ip} {port}"));
    }
    fn network_attach_transport(&mut self, use_nat_transport: bool) {
        if use_nat_transport {
            self.l("nat getTransport".into());
            self.l("network attachTransport 1".into());
        } else {
            self.l("network initTransport".into());
        }
    }
    fn network_parse_user_list(&mut self) {
        self.l("network parseUserList".into());
    }
    fn game_logic_is_in_game(&self) -> bool {
        (self.cfg[1] >> 5) & 1 != 0
    }
    fn game_logic_clear_game_data(&mut self) {
        self.l("gamelogic clearGameData".into());
    }
    fn do_any_map_transfers(&mut self) -> bool {
        self.l("doAnyMapTransfers".into());
        (self.cfg[1] >> 6) & 1 != 0
    }
    fn map_cache_update_cache(&mut self) {
        self.l("mapcache updateCache".into());
    }
    fn map_cache_lacks_map(&mut self, map: &[u8]) -> bool {
        self.l(format!("mapcache findMap [{}]", bs(map)));
        (self.cfg[1] >> 7) & 1 == 0
    }
    fn message_box_and_pop_back_to_lobby(&mut self) {
        self.l("messagebox [T:GUI:Error] [T:GUI:CouldNotTransferMap]".into());
        self.l("popBackToLobby".into());
    }
    fn set_pending_file(&mut self, map: &[u8]) {
        self.pending = map.to_vec();
    }
    fn append_new_game_message_internet(&mut self) {
        self.l("message append 1".into());
        self.l("message appendIntegerArgument 5".into());
    }
    fn set_use_fps_limit_false(&mut self) {
        self.fps_limit = false;
    }
    fn init_game_logic_random(&mut self, seed: i32) {
        self.l(format!("initGameLogicRandom {seed}"));
    }
    fn buddy_set_status_loading(&mut self, location: &[u8]) {
        self.l(format!("buddy request 7 status 3 [Loading] [{}]", bs(location)));
    }
}

fn dump_room(h: &mut Host, what: &str, g: &GameSpyStagingRoom) {
    let i = &g.info;
    h.l(format!(
        "{what}: in {} prog {} ip {} map [{}] seed {} pre {} name [{}] id {} ping [{}] {} qm {} ladder [{}] {} pending [{}] fps {}",
        i.in_game as i32,
        i.in_progress as i32,
        i.local_ip,
        bs(&i.map_name),
        i.seed,
        i.preorder_mask,
        wide(g.get_game_name().str()),
        g.get_id(),
        bs(g.get_ping_string().str()),
        g.get_ping_as_int(),
        g.is_qm_game() as i32,
        bs(g.get_ladder_ip().str()),
        g.get_ladder_port(),
        bs(&h.pending.clone()),
        h.fps_limit as i32
    ));
    for k in 0..8 {
        let s = &i.slots[k];
        let x = &g.gs_slots[k];
        h.l(format!(
            " slot {k}: st {} acc {} map {} team {} name [{}] ip {} dis {} | login [{}] pid {} loc [{}] ping [{}] {} wl {} {} rank {} side {}",
            s.state,
            s.is_accepted as i32,
            s.has_map as i32,
            s.team_number,
            wide(&s.name),
            s.ip,
            s.disconnected as i32,
            bs(x.login_name.str()),
            x.profile_id,
            bs(x.locale.str()),
            bs(x.ping_str.str()),
            x.ping_int,
            x.wins,
            x.losses,
            x.rank_points,
            x.favorite_side
        ));
    }
}

fn dump_lan(h: &mut Host, what: &str, g: &LANGameInfo) {
    let i = &g.info;
    h.l(format!(
        "{what}: in {} prog {} ip {} map [{}] crc {} seed {} last {} name [{}] direct {}",
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
            " slot {k}: st {} acc {} map {} name [{}] ip {} last {} serial [{}] login [{}] host [{}]",
            s.state,
            s.is_accepted as i32,
            s.has_map as i32,
            wide(&s.name),
            s.ip,
            x.get_last_heard(),
            bs(x.get_serial().str()),
            wide(x.user_login().str()),
            wide(x.user_host().str())
        ));
    }
}

/// The program of the Rust side. Returns the log and how many operations ran (fewer than all when one is undefined).
fn rust_run(cfg: &[u32; 14], ops: &[[u32; 6]]) -> (String, usize) {
    let mut h = Host {
        cfg: *cfg,
        tick: 0,
        time: std::cell::Cell::new(0),
        log: std::cell::RefCell::new(String::new()),
        nat: false,
        network: false,
        lan: true,
        my_game: false,
        pending: Vec::new(),
        fps_limit: true,
    };
    let mut room = GameSpyStagingRoom::new(&mut h);
    let mut lan = LANGameInfo::new(&mut h);
    h.l("made".into());
    dump_room(&mut h, "room", &room);
    dump_lan(&mut h, "lan", &lan);
    let mut done = 0;
    for w in ops {
        let (a, b, c, d, e) = (w[1], w[2], w[3], w[4], w[5]);
        match w[0] {
            0 => {
                let k = (a % 8) as usize;
                let name = uname(c);
                room.info.slots[k].set_state((b % 7) as i32, &name, d, &mut h);
                let s = &mut room.info.slots[k];
                s.team_number = (e & 15) as i32 - 2;
                s.disconnected = e & 16 != 0;
                if e & 32 != 0 {
                    s.set_accept();
                } else {
                    s.un_accept();
                }
                s.set_map_availability(e & 64 != 0);
                h.l(format!("room setslot {}", a % 8));
            }
            1 => {
                match a % 5 {
                    0 => room.info.enter_game(&mut h),
                    1 => room.info.leave_game(&mut h),
                    2 => room.info.in_progress = b & 1 != 0,
                    3 => room.reset(&mut h),
                    _ => room.init(&mut h),
                }
                h.l(format!("room life {}", a % 5));
            }
            2 => {
                match a % 8 {
                    0 if room.info.in_game => h.l("skip setMap in game".into()),
                    0 => {
                        let mut probe = room.info.clone();
                        let mut hp = h.clone();
                        if probe.set_map(pool(b), &mut hp).is_err() {
                            break;
                        }
                        let _ = room.info.set_map(pool(b), &mut h);
                    }
                    1 => room.info.seed = b as i32,
                    2 => room.set_local_name(asc(pool(b))),
                    3 => {
                        room.set_ladder_ip(asc(pool(b)));
                        room.set_ladder_port(c as u16);
                    }
                    4 => room.mark_game_as_qm(),
                    5 => room.set_game_name(uni(b)),
                    6 => room.set_id(b as i32),
                    _ => room.info.local_ip = b,
                }
                h.l(format!("room set {}", a % 8));
            }
            3 => {
                let idx = if a % 12 == 11 { -1 } else { (a % 12) as i32 };
                let host_q = room.am_i_host();
                let local = room.get_local_slot_num(&h);
                let present = room.get_game_spy_slot(idx).is_some();
                h.l(format!(
                    "room q host {} local {} ping [{}] {} slot {}",
                    host_q as i32,
                    local,
                    bs(room.get_ping_string().str()),
                    room.get_ping_as_int(),
                    present as i32
                ));
            }
            4 => {
                let p = asc(pool(a));
                let v = h.ping_value(&p);
                room.set_ping_string(p, v);
                h.l("room ping set".into());
            }
            5 => {
                let k = (a % 8) as usize;
                let g = &mut room.gs_slots[k];
                g.login_name = asc(pool(b));
                g.profile_id = c as i32;
                g.locale = asc(pool(d));
                g.wins = (e & 255) as i32;
                g.losses = ((e >> 8) & 255) as i32;
                g.rank_points = ((e >> 16) & 255) as i32;
                g.favorite_side = (e >> 24) as i32;
                let p = asc(pool(b));
                let v = h.ping_value(&p);
                g.set_ping_string(p, v);
                h.l(format!("room gs slot {}", a % 8));
            }
            6 => {
                let mut input = StagingResultsInput {
                    end_frame: h.cfg[7] as i32,
                    saw_crc_mismatch: (h.cfg[1] >> 8) & 1 != 0,
                    average_fps: h.cfg[6] as i32,
                    portable_map_path: asc(&swap_prefix(&room.info.map_name, b"data\\", b"maps\\")),
                    players: Default::default(),
                };
                for k in 0..8usize {
                    if (h.cfg[9] >> k) & 1 != 0 {
                        let st = |q: u32| (hash_int(k as u32 * 16 + q, h.cfg[12]) & 0xffff) as i32;
                        let sides = ["America", "China", "GLA", "Boss"];
                        input.players[k] = Some(StagingPlayerResults {
                            has_achieved_victory: (h.cfg[10] >> k) & 1 != 0,
                            side: asc(sides[((h.cfg[11] >> (2 * k)) & 3) as usize].as_bytes()),
                            units_killed: st(0),
                            units_lost: st(1),
                            units_built: st(2),
                            buildings_killed: st(3),
                            buildings_lost: st(4),
                            buildings_built: st(5),
                            earnings: st(6),
                            tech_captured: st(7),
                            stats_locale: (hash_int(room.gs_slots[k].profile_id as u32, 5) % 1000) as i32,
                        });
                    }
                }
                let s1 = room.generate_game_spy_game_results_packet(&h, &input);
                h.l(format!("results gs [{}]", bs(s1.str())));
                let s2 = room.generate_ladder_game_results_packet(&h, &input);
                h.l(format!("results ladder [{}]", bs(s2.str())));
            }
            7 => {
                room.start_game(&mut h, a as i32);
                h.l("room start".into());
            }
            8 => {
                room.launch_game(&mut h);
                h.l("room launch".into());
            }
            9 => {
                room.reset_accepted();
                let f: String = (0..8).map(|k| if room.info.slots[k].is_accepted { '1' } else { '0' }).collect();
                h.l(format!("room resetAccepted {f}"));
            }
            10 => dump_room(&mut h, "room", &room),
            11 => {
                if a == 0 {
                    h.nat = false;
                }
                if a == 1 {
                    h.nat = true;
                }
                if b == 0 && h.network {
                    h.network = false;
                    h.l("network delete".into());
                }
                if b == 1 {
                    h.network = true;
                }
                h.l(format!("globals nat {} network {}", h.nat as i32, h.network as i32));
            }
            20 => {
                let mut s = GameSlot::new();
                s.set_state((b % 7) as i32, &uname(c), d, &mut h);
                let mut x = LANSlotExtra::new();
                x.set_login(uni(e & 255));
                x.set_host(uni((e >> 8) & 255));
                x.set_serial(asc(pool((e >> 16) & 255)));
                x.set_last_heard((e >> 24).wrapping_mul(1000));
                lan.set_slot(a as i32 - 1, s, x);
                h.l(format!("lan setslot {}", a as i32 - 1));
            }
            21 => {
                if !h.lan {
                    h.l("skip".into());
                    done += 1;
                    continue;
                }
                let ib = (b % 10) as i32 - 1;
                let ic = (c % 10) as i32 - 1;
                let li = h.cfg[0];
                let l1 = lan.get_local_slot_num(li);
                let l2 = lan.get_slot_num(&uni(a));
                let l3 = lan.am_i_host(li);
                let l4 = lan.get_host_ip();
                h.l(format!("lan q local {l1} slotnum {l2} host {} hostip {l4}", l3 as i32));
                let cu = if (0..8).contains(&ic) {
                    let k = ic as usize;
                    let slot = lan.info.slots[k].clone();
                    lan.extras[k].get_user(&slot).cloned()
                } else {
                    None
                };
                if (0..8).contains(&ib) {
                    let k = ib as usize;
                    let slot = lan.info.slots[k].clone();
                    let u = lan.extras[k].get_user(&slot).cloned();
                    let is_user = lan.extras[k].is_user_player(&slot, cu.as_ref());
                    let is_name = lan.extras[k].is_user_name(&slot, &uni(d));
                    let local = LANSlotExtra::is_local_player(&slot, li);
                    h.l(format!(
                        "lan q slot ip {} heard {} user {} [{}] ip {} heard {} login [{}] host [{}] isUser {} isName {} local {} serial [{}]",
                        slot.ip,
                        lan.extras[k].get_last_heard(),
                        u.is_some() as i32,
                        u.as_ref().map(|u| wide(u.get_name().str())).unwrap_or_default(),
                        u.as_ref().map(|u| u.get_ip()).unwrap_or(0),
                        u.as_ref().map(|u| u.get_last_heard()).unwrap_or(0),
                        u.as_ref().map(|u| wide(u.get_login().str())).unwrap_or_default(),
                        u.as_ref().map(|u| wide(u.get_host().str())).unwrap_or_default(),
                        is_user as i32,
                        is_name as i32,
                        local as i32,
                        bs(lan.extras[k].get_serial().str())
                    ));
                }
                let who = (b % 8) as usize;
                h.l(format!("lan q who {who} ip {} heard {} name [{}]", lan.get_ip(who), lan.get_player_last_heard(who), wide(lan.get_player_name(who).str())));
            }
            22 => {
                lan.reset_accepted(&mut h);
                let f: String = (0..8).map(|k| if lan.info.slots[k].is_accepted { '1' } else { '0' }).collect();
                h.l(format!("lan resetAccepted {f}"));
            }
            23 => {
                match a % 10 {
                    0 => lan.info.enter_game(&mut h),
                    1 => lan.info.leave_game(&mut h),
                    2 => {
                        let mut probe = lan.info.clone();
                        let mut hp = h.clone();
                        if probe.set_map(pool(b), &mut hp).is_err() {
                            break;
                        }
                        lan.set_map(pool(b), &mut h);
                    }
                    3 => lan.set_seed(b as i32),
                    4 => lan.set_name(uni(b)),
                    5 => lan.set_last_heard(b),
                    6 => lan.set_is_direct_connect(b & 1 != 0),
                    7 => lan.set_player_name((c % 8) as usize, &uni(b)),
                    8 => lan.set_player_last_heard((c % 8) as usize, b),
                    _ => lan.set_ip((c % 8) as usize, b),
                }
                h.l(format!("lan set {}", a % 10));
            }
            24 => {
                let Ok(mut o) = game_info_to_ascii_string(Some(&room.info), &h) else { break };
                match b % 6 {
                    1 => {
                        if !o.is_empty() {
                            let p = c as usize % o.len();
                            o.remove(p);
                        }
                    }
                    2 => {
                        if !o.is_empty() {
                            let p = c as usize % o.len();
                            o.insert(p, if d != 0 { d as u8 } else { b'x' });
                        }
                    }
                    3 => {
                        if !o.is_empty() {
                            let p = c as usize % o.len();
                            o[p] = if d != 0 { d as u8 } else { b'x' };
                        }
                    }
                    4 => {
                        if !o.is_empty() {
                            let p = c as usize % o.len();
                            o.truncate(p);
                        }
                    }
                    _ => {}
                }
                // the same parse on a copy: an Err here is undefined upstream (the draft would report false)
                let mut probe = lan.clone();
                let mut hp = h.clone();
                if parse_ascii_string_to_game_info(Some(&mut probe.info), &o, &mut hp).is_err() {
                    break;
                }
                let ok = parse_game_options_string(Some(&mut lan), &o, &mut h);
                h.l(format!("lan parse {} len {}", ok as i32, o.len()));
                dump_lan(&mut h, "lan after parse", &lan);
            }
            25 => {
                if !h.lan {
                    h.l("skip".into());
                } else {
                    let mine = if h.my_game { Some(&lan) } else { None };
                    if let Some(g) = mine {
                        if g.am_i_host(h.cfg[0]) && game_info_to_ascii_string(Some(&g.info), &h).is_err() {
                            break;
                        }
                    }
                    let o = generate_game_options_string(mine, &h);
                    h.l(format!("lan options [{}]", bs(&o)));
                }
            }
            26 => dump_lan(&mut h, "lan", &lan),
            27 => {
                h.my_game = a == 1;
                h.l(format!("lan mygame {}", (a == 1) as i32));
            }
            28 => {
                h.lan = a == 1;
                h.l(format!("lan thelan {}", (a == 1) as i32));
            }
            _ => h.l("op?".into()),
        }
        done += 1;
    }
    if h.network {
        h.l("network delete".into());
    }
    let log = h.log.borrow().clone();
    (log, done)
}


// ---------------------------------------------------------------------------------------------------------------
// the generator and the tests
// ---------------------------------------------------------------------------------------------------------------

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn pack_strs() -> Vec<u8> {
    let mut v = Vec::new();
    for s in POOL {
        v.extend_from_slice(&(s.len() as u16).to_le_bytes());
        v.extend_from_slice(s.as_bytes());
    }
    v
}

fn cpp_run(cfg: &[u32; 14], ops: &[[u32; 6]]) -> String {
    let mut cfg16 = [0u32; 16];
    cfg16[..14].copy_from_slice(cfg);
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let strs = pack_strs();
    let mut out = vec![0u8; 4_000_000];
    let n = unsafe { ref_dgi_run(cfg16.as_ptr(), flat.as_ptr(), ops.len() as c_int, strs.as_ptr() as *const c_char, POOL.len() as c_int, out.as_mut_ptr() as *mut c_char, out.len() as c_int) };
    assert!((n as usize) <= out.len(), "log too long");
    out.truncate(n as usize);
    String::from_utf8_lossy(&out).into_owned()
}

fn gen_cfg(g: &mut Gen) -> [u32; 14] {
    let ips = [0u32, 1, 0x7f00_0001, 0xc0a8_0001, 0xc0a8_0002, 0x0a00_0001];
    let mut flags = 0u32;
    for bit in [1, 5, 6, 7, 8] {
        if g.below(2) == 0 {
            flags |= 1 << bit;
        }
    }
    [
        ips[g.below(6) as usize],
        flags,
        [0, 1, 2, 3, 4, 7, 8, 15, 16, 25][g.below(10) as usize],
        ips[g.below(6) as usize],
        g.below(40),
        g.u32(),
        g.below(100),
        if g.below(3) == 0 { 0 } else { g.below(100000) },
        g.below(70000),
        if g.below(4) == 0 { 0 } else { g.u32() & 0xff },
        g.u32() & 0xff,
        g.u32() & 0xffff,
        g.u32(),
        g.u32(),
    ]
}

fn name_idx(g: &mut Gen) -> u32 {
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25][g.below(20) as usize]
}

fn gen_ops(g: &mut Gen, cfg: &[u32; 14]) -> Vec<[u32; 6]> {
    let mut ops: Vec<[u32; 6]> = Vec::new();
    let ip = |g: &mut Gen| match g.below(4) {
        0 => cfg[0],
        1 => cfg[3],
        2 => 0xc0a8_0001 + g.below(3),
        _ => g.u32(),
    };
    // setup: enter both games, some slots
    ops.push([1, 0, 0, 0, 0, 0]);
    ops.push([23, 0, 0, 0, 0, 0]);
    let n = 20 + g.below(60);
    for _ in 0..n {
        let w = match g.below(100) {
            0..=11 => [0, g.below(10), if g.below(3) == 0 { g.below(7) } else { 5 }, name_idx(g), ip(g), g.u32() & 0xff],
            12..=14 => [1, g.below(5), g.below(2), 0, 0, 0],
            15..=22 => [2, g.below(8), if g.below(2) == 0 { 9 + g.below(5) } else { name_idx(g) }, g.below(70000), 0, 0],
            23..=26 => [3, g.below(13), 0, 0, 0, 0],
            27..=28 => [4, name_idx(g), 0, 0, 0, 0],
            29..=36 => [5, g.below(8), name_idx(g), g.below(60), name_idx(g), g.u32()],
            37..=44 => [6, 0, 0, 0, 0, 0],
            45..=47 => [7, g.below(100), 0, 0, 0, 0],
            48..=51 => [8, 0, 0, 0, 0, 0],
            52..=53 => [9, 0, 0, 0, 0, 0],
            54..=55 => [10, 0, 0, 0, 0, 0],
            56..=58 => [11, g.below(3), g.below(3), 0, 0, 0],
            59..=69 => [20, g.below(10), if g.below(3) == 0 { g.below(7) } else { 5 }, name_idx(g), ip(g), (name_idx(g)) | (name_idx(g) << 8) | (name_idx(g) << 16) | (g.below(30) << 24)],
            70..=77 => [21, name_idx(g), g.below(10), g.below(10), name_idx(g), 0],
            78..=80 => [22, 0, 0, 0, 0, 0],
            81..=84 => [23, g.below(10), if g.below(2) == 0 { 9 + g.below(5) } else { g.u32() }, g.below(10), 0, 0],
            85..=90 => [24, 0, g.below(6), g.u32(), if g.below(3) == 0 { g.below(256) } else { b'~' as u32 }, 0],
            91..=93 => [25, 0, 0, 0, 0, 0],
            94 => [26, 0, 0, 0, 0, 0],
            95..=96 => [27, g.below(2), 0, 0, 0, 0],
            _ => [28, if g.below(3) == 0 { 0 } else { 1 }, 0, 0, 0, 0],
        };
        ops.push(w);
    }
    ops
}

#[test]
fn the_lan_and_staging_room_drafts_are_upstream_s() {
    let n = cases("DGI_CASES", 6000);
    let mut g = Gen::new(0x215_0a01);
    let mut chk = Check::new("lan + staging room: logs and dumps");
    let (mut total_ops, mut cut, mut lines) = (0u64, 0u64, 0u64);
    for i in 0..n {
        let cfg = gen_cfg(&mut g);
        let ops = gen_ops(&mut g, &cfg);
        let (r, done) = rust_run(&cfg, &ops);
        if done < ops.len() {
            cut += 1;
        }
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&cfg, &ops[..done]);
        drop(_one);
        total_ops += done as u64;
        lines += r.lines().count() as u64;
        if r != c {
            let k = r.lines().zip(c.lines()).position(|(a, b)| a != b).unwrap_or(0);
            println!("case {i}: first difference at line {k}:\n  rust {:?}\n  c++  {:?}", r.lines().nth(k), c.lines().nth(k));
        }
        chk.eq(r, c, || format!("case {i} (cfg {cfg:?}, {done} ops)"));
    }
    println!("{n} programs, {total_ops} operations, {lines} log lines, {cut} programs cut at an undefined operation");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
