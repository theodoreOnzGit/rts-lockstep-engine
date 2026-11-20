#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/PeerDefs.cpp and Include/GameNetwork/GameSpy/PeerDefs.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/peer_defs.rs` against the C++ (GitHub #215, group network3).
//!
//! # Method
//!
//! `differential/wrap_draft_peerdefs.cpp` compiles upstream's `PeerDefs.cpp` members cut verbatim by build.rs (`AsciiComparator`,
//! `GameSpyInfo`'s group-room, player-info, ignore-list, preorder, staging-room-list, MOTD/config, ping-value and accessor members, and
//! `PlayerInfo::isIgnored`) and the type definitions cut from `PeerDefs.h`, on upstream's own `UnicodeString.cpp`/`AsciiString.cpp`.
//! The surroundings (config, game text, peer queue, message box, ignore preferences, staging rooms) are stand-ins listed in the wrapper;
//! this file's `Host` answers and logs identically. A case is a configuration (quick-match channel, ping timeout, text salt, the saved
//! ignore preferences) and 20 to 60 operations (opcodes at `ref_dpd_run`); after every operation the whole state is written out
//! (rooms with translated names, the case-insensitive player map in order, ignore lists, preorder probes, staging rooms, flags) and the two
//! logs are compared whole. Also `AsciiComparator` on pairs of strings and the `GameSpyColors` values.
//!
//! Not covered (needs the staging room's `GameInfo`, the GameSpy SDK, or the logic's globals): `GameSpyInfo::reset`,
//! `markAsStagingRoomHost`, `markAsStagingRoomJoiner`, `setGameOptions`, `updateAdditionalGameSpyDisconnections`,
//! `readAdditionalDisconnects`, `getCurrentStagingRoom` (only its None path is reachable without hosting), `SetUpGameSpy`,
//! `TearDownGameSpy` (threads, files, singletons).

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::game_info::*;
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::drafts::network::peer_defs::*;
use rts_lockstep_engine::drafts::network::peer_thread::{PeerMessageQueue, PeerRequestType};
use rts_lockstep_engine::drafts::network::persistent_storage_thread::PsMessageQueue;
use rts_lockstep_engine::drafts::network::staging_room_game_info::GameSpyStagingRoom;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::os::raw::{c_char, c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_dpd_cmp(a: *const c_char, b: *const c_char) -> c_int;
    fn ref_dpd_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
    fn ref_dpd_color_max() -> c_int;
    fn ref_dpd_color(i: c_int) -> c_int;
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
fn hash_str(s: &[u8]) -> u32 {
    s.iter().fold(2166136261u32, |h, &c| fnv(h, c))
}
fn hash_units(s: &[u16]) -> u32 {
    s.iter().fold(2166136261u32, |h, &c| fnv(fnv(h, (c & 255) as u8), (c >> 8) as u8))
}

struct Host {
    log: RefCell<Vec<u32>>,
    qm: Cell<u32>,
    timeout: u32,
    salt: u32,
    prefs: BTreeMap<i32, AsciiString>,
    queue: PeerMessageQueue,
    ps: PsMessageQueue,
}

impl Host {
    fn text(&self, label: &[u8]) -> (bool, Vec<u16>) {
        let h = hash_str(label) ^ self.salt;
        (h % 3 != 0, format!("T{}", h % 100000).bytes().map(u16::from).collect())
    }
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

impl PeerDefsHost for Host {
    fn game_text_fetch_exists(&self, label: &[u8]) -> (bool, Vec<u16>) {
        self.text(label)
    }
    fn gs_message_box_error(&mut self, text_label: &str) {
        let title = self.text(b"GUI:Error").1;
        let text = self.text(text_label.as_bytes()).1;
        self.push(50);
        self.push(hash_units(&title));
        self.push(hash_units(&text));
        self.push(0);
    }
    fn config_qm_channel(&self) -> i32 {
        self.qm.get() as i32
    }
    fn config_set_qm_channel(&mut self, id: i32) {
        self.push(30);
        self.push(id as u32);
        self.qm.set(id as u32);
    }
    fn config_ping_timeout_ms(&self) -> i32 {
        self.timeout as i32
    }
    fn peer_queue(&self) -> PeerMessageQueue {
        self.queue.clone()
    }
    fn ps_queue(&self) -> PsMessageQueue {
        self.ps.clone()
    }
    fn default_map(&mut self) -> Vec<u8> {
        Vec::new()
    }
    fn map_cache_update(&mut self) {}
    fn ignore_prefs_set_and_write(&mut self, nick: &[u8], profile_id: i32, ignore: bool) {
        self.push(60);
        self.push(hash_str(nick));
        self.push(profile_id as u32);
        self.push(u32::from(ignore));
        self.push(61);
    }
    fn ignore_prefs_get(&self) -> BTreeMap<i32, AsciiString> {
        self.prefs.clone()
    }
    fn additional_disconnects_from_user_file(&mut self, _profile_id: i32) -> i32 {
        0
    }
    fn misc_prefs_set_cached_stats_and_write(&mut self, _kv: &[u8]) {}
}

const NAMES: &[&str] = &[
    "Alice", "alice", "ALICE", "Bob", "bob", "", "a", "A", "Zed", "zed ", "Quickmatch", "quickmatch", "QuickMatch", "USA Lobby", "Room", "ROOM", "room2",
    "Al", "Alicf", "\u{e9}x", "GUI", "x y",
];
const PINGS: &[&str] = &[
    "", "00", "ff", "FF", "ffff", "0000", "a1b2c3", "A1B2C3", "7f7f7f7f", "0102030405060708", "zz", "g1", "1g", "  ", "+1", "-1", "0x", "x0", "fFfF", "80",
    "80ff", "ff80", "1", "abc", "ab", "AB", "cd", "7e", "81", "01", "10",
];

fn name(g: &mut Gen) -> Vec<u8> {
    NAMES[g.below(NAMES.len() as u32) as usize].bytes().collect()
}

fn ping(g: &mut Gen) -> Vec<u8> {
    match g.below(4) {
        0 | 1 => PINGS[g.below(PINGS.len() as u32) as usize].bytes().collect(),
        2 => {
            let n = 2 * g.below(5) as usize;
            let hex = b"0123456789abcdefABCDEF";
            (0..n).map(|_| hex[g.below(hex.len() as u32) as usize]).collect()
        }
        _ => {
            let n = 2 * (1 + g.below(3) as usize);
            let any = b"0123456789abcdefABCDEFgGzZ +-xX\t";
            (0..n).map(|_| any[g.below(any.len() as u32) as usize]).collect()
        }
    }
}

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

fn small(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => 0,
        1 => (-1i32) as u32,
        2 => 1,
        3 => g.below(40),
        4 => g.u32(),
        _ => g.below(6),
    }
}

fn id(g: &mut Gen) -> u32 {
    match g.below(6) {
        0 => 0,
        1 => (-1i32) as u32,
        _ => g.below(8),
    }
}

/// A random program (the words the wrapper reads) and the count of its operations.
fn program(g: &mut Gen) -> Vec<u32> {
    let mut v = vec![];
    v.push(match g.below(4) {
        0 => 0,
        _ => g.below(6),
    }); // qm channel
    v.push(match g.below(5) {
        0 => 0,
        1 => 1000,
        _ => 100 + g.below(900),
    }); // ping timeout
    v.push(g.u32()); // salt
    let np = g.below(4);
    v.push(np);
    for _ in 0..np {
        v.push(id(g));
        let n = name(g);
        push_bytes(&mut v, &n);
    }
    let nops = 20 + g.below(41);
    for _ in 0..nops {
        let op = 1 + g.below(44);
        v.push(op);
        match op {
            1 => {
                let p = ping(g);
                push_bytes(&mut v, &p);
            }
            2 => {
                let p = ping(g);
                push_bytes(&mut v, &p);
            }
            3 => {
                let n = name(g);
                push_bytes(&mut v, &n);
                v.push(if g.below(5) == 0 { 0 } else { id(g) });
                for _ in 0..4 {
                    v.push(small(g));
                }
            }
            4 | 20 | 13 | 14 | 17 | 18 | 23 | 24 | 25 | 27 | 39 | 40 => v.push(id(g)),
            7 => {
                let n = name(g);
                push_bytes(&mut v, &n);
                let l = name(g);
                push_bytes(&mut v, &l);
                for k in 0..7 {
                    v.push(if k == 2 { id(g) } else { small(g) });
                }
                let o = if g.below(2) == 0 { Vec::new() } else { name(g) };
                push_bytes(&mut v, &o);
            }
            8 | 9 | 10 | 11 | 30 | 32 => {
                let n = name(g);
                push_bytes(&mut v, &n);
            }
            12 => {
                v.push(id(g));
                let n = name(g);
                push_bytes(&mut v, &n);
            }
            16 => {
                let n = name(g);
                push_bytes(&mut v, &n);
                v.push(id(g));
            }
            19 => {
                v.push(g.u32());
                v.push(g.u32());
            }
            34 | 35 => v.push(g.below(3)),
            37 => v.push(small(g)),
            _ => {}
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
}

fn asc(b: &[u8]) -> AsciiString {
    AsciiString::try_from_bytes(b).unwrap()
}

fn log_str(o: &mut Vec<u32>, s: &[u8]) {
    o.push(s.len() as u32);
    o.extend(s.iter().map(|&c| u32::from(c)));
}

fn dump(o: &mut Vec<u32>, g: &mut GameSpyInfo, qm: u32) {
    o.push(0xD0D0D0D0);
    log_str(o, g.get_ping_string().str());
    let rooms: Vec<_> = g.get_group_room_list().iter().map(|(k, r)| (*k, r.clone())).collect();
    o.push(rooms.len() as u32);
    for (k, r) in &rooms {
        o.push(*k as u32);
        log_str(o, r.name.str());
        o.push(r.translated_name.len() as u32);
        o.extend(r.translated_name.iter().map(|&c| u32::from(c)));
        o.push(r.group_id as u32);
        o.push(r.num_waiting as u32);
        o.push(r.max_waiting as u32);
        o.push(r.num_games as u32);
        o.push(r.num_playing as u32);
    }
    o.push(u32::from(g.got_group_room_list()));
    o.push(g.get_current_group_room() as u32);
    let players: Vec<_> = g.get_player_info_map().iter().map(|(k, p)| (k.clone(), p.clone())).collect();
    o.push(players.len() as u32);
    for (k, p) in &players {
        log_str(o, k.str());
        log_str(o, p.name.str());
        log_str(o, p.locale.str());
        for v in [p.wins, p.losses, p.profile_id, p.flags, p.rank_points, p.side, p.preorder] {
            o.push(v as u32);
        }
    }
    let ig = g.return_ignore_list();
    o.push(ig.len() as u32);
    for n in &ig {
        log_str(o, n);
    }
    let sv = g.return_saved_ignore_list();
    o.push(sv.len() as u32);
    for (k, n) in &sv {
        o.push(*k as u32);
        log_str(o, n.str());
    }
    for id in -2..10 {
        o.push(u32::from(g.did_player_preorder(id)));
    }
    let sr: Vec<i32> = g.get_staging_room_list().iter().map(|(k, r)| {
        let _ = k;
        r.get_id()
    }).collect();
    let keys: Vec<i32> = g.get_staging_room_list().keys().copied().collect();
    o.push(keys.len() as u32);
    for (k, i) in keys.iter().zip(sr.iter()) {
        o.push(*k as u32);
        o.push(*i as u32);
    }
    o.push(u32::from(g.am_i_host()));
    o.push(g.get_current_staging_room_id() as u32);
    o.push(g.get_internal_ip());
    o.push(g.get_external_ip());
    log_str(o, g.get_motd().str());
    log_str(o, g.get_config().str());
    o.push(u32::from(g.get_disallow_asian_text()));
    o.push(u32::from(g.get_disallow_non_asian_text()));
    o.push(g.get_max_messages_per_update() as u32);
    let buddies: Vec<i32> = g.get_buddy_map().keys().copied().collect();
    o.push(buddies.len() as u32);
    o.extend(buddies.iter().map(|&b| b as u32));
    o.push(qm);
}

fn rust_run(w: &[u32]) -> Vec<u32> {
    let mut rd = Rd { w, i: 0 };
    let qm = rd.next();
    let timeout = rd.next();
    let salt = rd.next();
    let np = rd.next();
    let mut prefs = BTreeMap::new();
    for _ in 0..np {
        let id = rd.next() as i32;
        let n = rd.bytes();
        prefs.insert(id, asc(&n));
    }
    let mut host = Host { log: RefCell::new(Vec::new()), qm: Cell::new(qm), timeout, salt, prefs, queue: PeerMessageQueue::new(), ps: PsMessageQueue::new() };
    let mut g = GameSpyInfo::new(&mut host);
    let mut o: Vec<u32> = Vec::new();
    // the C++ log is one stream: host events are written as they happen, the state dumps and results between them
    let flush = |host: &Host, o: &mut Vec<u32>| {
        o.append(&mut host.log.borrow_mut());
    };
    dump(&mut o, &mut g, host.qm.get());
    while rd.i < w.len() {
        let op = rd.next();
        o.push(0xA5A5A5A5);
        o.push(op);
        let mut res: Vec<u32> = Vec::new();
        match op {
            1 => {
                let p = rd.bytes();
                g.set_ping_string(asc(&p));
            }
            2 => {
                let p = rd.bytes();
                let v = g.get_ping_value(&host, &asc(&p));
                res.push(v as u32);
            }
            3 => {
                let n = rd.bytes();
                let mut room = GameSpyGroupRoom::default();
                room.name = asc(&n);
                room.group_id = rd.next() as i32;
                room.num_waiting = rd.next() as i32;
                room.max_waiting = rd.next() as i32;
                room.num_games = rd.next() as i32;
                room.num_playing = rd.next() as i32;
                g.add_group_room(&mut host, room);
            }
            4 => g.join_group_room(&mut host, rd.next() as i32),
            5 => g.leave_group_room(&mut host),
            6 => g.join_best_group_room(&mut host),
            7 => {
                let mut pi = PlayerInfo::default();
                pi.name = asc(&rd.bytes());
                pi.locale = asc(&rd.bytes());
                pi.wins = rd.next() as i32;
                pi.losses = rd.next() as i32;
                pi.profile_id = rd.next() as i32;
                pi.flags = rd.next() as i32;
                pi.rank_points = rd.next() as i32;
                pi.side = rd.next() as i32;
                pi.preorder = rd.next() as i32;
                let old = asc(&rd.bytes());
                g.update_player_info(pi, &old);
            }
            8 => g.player_left_group_room(&asc(&rd.bytes())),
            9 => g.add_to_ignore_list(&asc(&rd.bytes())),
            10 => g.remove_from_ignore_list(&asc(&rd.bytes())),
            11 => res.push(u32::from(g.is_ignored(&asc(&rd.bytes())))),
            12 => {
                let id = rd.next() as i32;
                let n = asc(&rd.bytes());
                g.add_to_saved_ignore_list(&mut host, id, n);
            }
            13 => g.remove_from_saved_ignore_list(&mut host, rd.next() as i32),
            14 => res.push(u32::from(g.is_saved_ignored(rd.next() as i32))),
            15 => g.load_saved_ignore_list(&host),
            16 => {
                let mut pi = PlayerInfo::default();
                pi.name = asc(&rd.bytes());
                pi.profile_id = rd.next() as i32;
                res.push(u32::from(g.player_is_ignored(&pi)));
            }
            17 => g.mark_player_as_preorder(rd.next() as i32),
            18 => res.push(u32::from(g.did_player_preorder(rd.next() as i32))),
            19 => {
                let a = rd.next();
                let b = rd.next();
                g.set_local_ips(a, b);
            }
            20 => g.set_current_group_room(rd.next() as i32),
            21 => g.clear_group_room_list(),
            22 => g.saw_full_game_list(),
            23 | 24 | 25 => {
                let mut room = GameSpyStagingRoom::new(&mut host);
                room.set_id(rd.next() as i32);
                match op {
                    23 => g.add_staging_room(room),
                    24 => g.update_staging_room(room),
                    _ => g.remove_staging_room(&room),
                }
            }
            26 => res.push(u32::from(g.has_staging_room_list_changed())),
            27 => {
                let id = rd.next() as i32;
                match g.find_staging_room_by_id(id) {
                    Some(r) => {
                        res.push(1);
                        res.push(r.get_id() as u32);
                    }
                    None => res.push(0),
                }
            }
            28 => g.clear_staging_room_list(),
            29 => g.leave_staging_room(&mut host),
            30 => g.set_motd(asc(&rd.bytes())),
            31 => log_str(&mut res, g.get_motd().str()),
            32 => g.set_config(asc(&rd.bytes())),
            33 => log_str(&mut res, g.get_config().str()),
            34 => g.set_disallow_asian_text(rd.next() != 0),
            35 => g.set_disallow_non_asian_text(rd.next() != 0),
            36 => {
                res.push(u32::from(g.get_disallow_asian_text()));
                res.push(u32::from(g.get_disallow_non_asian_text()));
            }
            37 => g.set_max_messages_per_update(rd.next() as i32),
            38 => res.push(g.get_max_messages_per_update() as u32),
            39 => {
                let id = rd.next() as i32;
                g.get_buddy_map().insert(id, Default::default());
            }
            40 => res.push(u32::from(g.is_buddy(rd.next() as i32))),
            41 => res.push(g.get_additional_disconnects() as u32),
            42 => g.clear_additional_disconnects(),
            43 => res.push(u32::from(g.am_i_host())),
            44 => {
                res.push(g.get_current_group_room() as u32);
                res.push(u32::from(g.got_group_room_list()));
                res.push(g.get_internal_ip());
                res.push(g.get_external_ip());
                res.push(g.get_current_staging_room_id() as u32);
                log_str(&mut res, g.get_ping_string().str());
            }
            _ => {}
        }
        // queued peer requests: the C++ logs 40, type, id at the call
        while let Some(rq) = host.queue.get_request() {
            let t = match rq.peer_request_type {
                PeerRequestType::JoinGroupRoom => 1,
                PeerRequestType::LeaveGroupRoom => 2,
                PeerRequestType::LeaveStagingRoom => 3,
                _ => 99,
            };
            host.log.borrow_mut().extend([40, t, rq.group_room_id as u32]);
        }
        // the staging-room copies are logged by the C++ `cleanUpSlotPointers` stand-in; the draft's is empty: the C++ log of 70 is
        // reproduced here from the rooms added (addStagingRoom calls it once per add)
        if op == 23 || op == 24 {
            let id = w[rd.i - 1];
            host.log.borrow_mut().extend([70, id]);
        }
        flush(&host, &mut o);
        o.extend(res);
        dump(&mut o, &mut g, host.qm.get());
    }
    // clearStagingRoomList at the end in the C++ (frees the rooms): it logs nothing
    o
}

fn cpp_run(w: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 2_000_000];
    let n = unsafe { ref_dpd_run(w.as_ptr(), w.len() as c_int, out.as_mut_ptr(), out.len() as c_int) };
    assert!((n as usize) <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

#[test]
fn peerdefs_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("PD_CASES", 20_000);
    let mut g = Gen::new(0x5eed_0d0d);
    let mut chk = Check::new("peer_defs GameSpyInfo programs");
    for i in 0..n {
        let w = program(&mut g);
        let r = rust_run(&w);
        let c = cpp_run(&w);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(12);
            println!("case {i}: first difference at word {k} (rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}", r.len(), c.len(), &r[k0..(k + 12).min(r.len())], &c[k0..(k + 12).min(c.len())]);
            if std::env::var("PD_DEBUG").is_ok() {
                println!("program {:?}", w);
            }
        }
        chk.eq(r == c, true, || format!("peerdefs case {i}"));
    }
    println!("{n} programs");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}

#[test]
fn ascii_comparator_matches_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xc0de_0001);
    let mut chk = Check::new("peer_defs ascii_compare");
    let alphabet = b"abcABCzZ_ 09@[`{\x7f\x80\xe9\xc9";
    for i in 0..20_000 {
        let mk = |g: &mut Gen| -> Vec<u8> {
            let n = g.below(5) as usize;
            (0..n).map(|_| alphabet[g.below(alphabet.len() as u32) as usize]).collect()
        };
        let a = mk(&mut g);
        let b = if g.below(4) == 0 { a.iter().map(|c| c.to_ascii_uppercase()).collect() } else { mk(&mut g) };
        let ca = std::ffi::CString::new(a.clone()).unwrap();
        let cb = std::ffi::CString::new(b.clone()).unwrap();
        let c = unsafe { ref_dpd_cmp(ca.as_ptr(), cb.as_ptr()) } != 0;
        let r = ascii_compare(&a, &b) == std::cmp::Ordering::Less;
        chk.eq(r, c, || format!("case {i}: {a:?} < {b:?}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}

#[test]
fn game_spy_colors_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut chk = Check::new("peer_defs GameSpyColors");
    let max = unsafe { ref_dpd_color_max() };
    chk.eq(max, 27, || "GSCOLOR_MAX".into());
    use GameSpyColors::*;
    let all = [
        Default, CurrentRoom, Room, Game, GameFull, GameCrcMismatch, PlayerNormal, PlayerOwner, PlayerBuddy, PlayerSelf, PlayerIgnored, ChatNormal, ChatEmote,
        ChatOwner, ChatOwnerEmote, ChatPrivate, ChatPrivateEmote, ChatPrivateOwner, ChatPrivateOwnerEmote, ChatBuddy, ChatSelf, AcceptTrue, AcceptFalse,
        MapSelected, MapUnselected, Motd, MotdHeading,
    ];
    for (i, c) in all.iter().enumerate() {
        let r = *c as i32;
        let cpp = unsafe { ref_dpd_color(i as c_int) };
        chk.eq(r, cpp, || format!("colour {i}"));
    }
    chk.finish();
}
