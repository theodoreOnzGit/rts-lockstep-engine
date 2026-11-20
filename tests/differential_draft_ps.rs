#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Thread/PersistentStorageThread.cpp and Include/GameNetwork/GameSpy/PersistentStorageThread.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/persistent_storage_thread.rs` against the C++ (GitHub #215, group net4).
//!
//! # Method
//!
//! `differential/wrap_draft_ps.cpp` compiles upstream's `PSRequest::PSRequest`, `PSPlayerStats` (constructor, `reset`, `incorporate`, the copy constructor),
//! `GameSpyPSMessageQueue` (the whole queue: requests, responses, thread flag, `trackPlayerStats`, `findPlayerStatsByID`, the local player and the
//! credentials), `parsePlayerKVPairs` and `formatPlayerKVPairs`, cut verbatim by build.rs (`drafts_net4`). The SDK thread, the mutex and the player-template
//! store are stand-ins listed in the wrapper. Three comparisons: 20,000 key/value texts parsed (random keys, general numbers, numeric forms such as
//! overflow, sign, whitespace, hex, exponents and `nan`, malformed backslash runs), 20,000 records formatted (with random template counts, values at
//! and above 2^31, strings around the 255-character cut), and 6,000 programs of 30 to 80 queue and record operations, every result dumped word by
//! word. The two uninitialised words of upstream's copy constructor (`QMwinsInARow`, `maxQMwinsInARow`) are written as 0 on both sides wherever a record
//! went through it.
//!
//! Not covered: the SDK-driven thread (`Thread_Function`, `tryConnect`, `tryLogin`, the persist callbacks), which is the draft's `PsThread` and needs the
//! GameSpy stats SDK (not in the repository).

use rts_lockstep_engine::drafts::network::persistent_storage_thread::*;
use std::os::raw::{c_char, c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_ps_parse(kv: *const c_char, len: c_int, out: *mut c_uint, cap: c_int) -> c_int;
    fn ref_ps_format(input: *const c_uint, template_count: c_int, out: *mut c_char, cap: c_int) -> c_int;
    fn ref_ps_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn ints(s: &PsPlayerStats) -> [i32; 21] {
    [
        s.id,
        s.locale,
        s.games_as_random,
        s.last_general,
        s.games_in_row_with_last_general,
        s.challenge_medals,
        s.battle_honors,
        s.qm_wins_in_a_row,
        s.max_qm_wins_in_a_row,
        s.wins_in_a_row,
        s.max_wins_in_a_row,
        s.losses_in_a_row,
        s.max_losses_in_a_row,
        s.discons_in_a_row,
        s.max_discons_in_a_row,
        s.desyncs_in_a_row,
        s.max_desyncs_in_a_row,
        s.built_particle_cannon,
        s.built_nuke,
        s.built_scud,
        s.last_ladder_port,
    ]
}

fn set_ints(s: &mut PsPlayerStats, v: [i32; 21]) {
    s.id = v[0];
    s.locale = v[1];
    s.games_as_random = v[2];
    s.last_general = v[3];
    s.games_in_row_with_last_general = v[4];
    s.challenge_medals = v[5];
    s.battle_honors = v[6];
    s.qm_wins_in_a_row = v[7];
    s.max_qm_wins_in_a_row = v[8];
    s.wins_in_a_row = v[9];
    s.max_wins_in_a_row = v[10];
    s.losses_in_a_row = v[11];
    s.max_losses_in_a_row = v[12];
    s.discons_in_a_row = v[13];
    s.max_discons_in_a_row = v[14];
    s.desyncs_in_a_row = v[15];
    s.max_desyncs_in_a_row = v[16];
    s.built_particle_cannon = v[17];
    s.built_nuke = v[18];
    s.built_scud = v[19];
    s.last_ladder_port = v[20];
}

fn maps(s: &PsPlayerStats) -> [&PerGeneralMap; 24] {
    [
        &s.wins, &s.losses, &s.games, &s.duration, &s.units_killed, &s.units_lost, &s.units_built, &s.buildings_killed, &s.buildings_lost, &s.buildings_built, &s.earnings,
        &s.tech_captured, &s.discons, &s.desyncs, &s.surrenders, &s.games_of_2p, &s.games_of_3p, &s.games_of_4p, &s.games_of_5p, &s.games_of_6p, &s.games_of_7p,
        &s.games_of_8p, &s.custom_games, &s.qm_games,
    ]
}

fn maps_mut(s: &mut PsPlayerStats) -> [&mut PerGeneralMap; 24] {
    [
        &mut s.wins, &mut s.losses, &mut s.games, &mut s.duration, &mut s.units_killed, &mut s.units_lost, &mut s.units_built, &mut s.buildings_killed,
        &mut s.buildings_lost, &mut s.buildings_built, &mut s.earnings, &mut s.tech_captured, &mut s.discons, &mut s.desyncs, &mut s.surrenders, &mut s.games_of_2p,
        &mut s.games_of_3p, &mut s.games_of_4p, &mut s.games_of_5p, &mut s.games_of_6p, &mut s.games_of_7p, &mut s.games_of_8p, &mut s.custom_games, &mut s.qm_games,
    ]
}

fn put_str(w: &mut Vec<u32>, s: &[u8]) {
    w.push(s.len() as u32);
    w.extend(s.iter().map(|&b| b as u32));
}

/// The stream of the wrapper (`dumpStats`/`readStats`); `zero_qm` writes 0 for the two uninitialised words.
fn encode(w: &mut Vec<u32>, s: &PsPlayerStats, zero_qm: bool) {
    for (i, v) in ints(s).iter().enumerate() {
        w.push(if zero_qm && (i == 7 || i == 8) { 0 } else { *v as u32 });
    }
    w.push(s.last_fps.to_bits());
    put_str(w, &s.options);
    put_str(w, &s.system_spec);
    put_str(w, &s.last_ladder_host);
    for m in maps(s) {
        w.push(m.len() as u32);
        for (k, v) in m.iter() {
            w.push(*k as u32);
            w.push(*v);
        }
    }
}

fn text(g: &mut Gen, max: u32) -> Vec<u8> {
    let n = if g.below(12) == 0 { 240 + g.below(40) } else { g.below(max + 1) };
    (0..n)
        .map(|_| {
            if g.below(30) == 0 {
                (128 + g.below(128)) as u8
            } else {
                let c = (32 + g.below(95)) as u8;
                if c == b'\\' {
                    b'/'
                } else {
                    c
                }
            }
        })
        .collect()
}

fn small_or_edge(g: &mut Gen) -> i32 {
    match g.below(10) {
        0 => -1,
        1 => 0,
        2 => i32::MAX,
        3 => i32::MIN,
        4 => g.i32(),
        _ => g.below(40) as i32,
    }
}

fn map_value(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => 0,
        1 => 0x8000_0000 + g.below(1000),
        2 => g.u32(),
        _ => g.below(500),
    }
}

fn gen_stats(g: &mut Gen) -> PsPlayerStats {
    let mut s = PsPlayerStats::new();
    let mut v = [0i32; 21];
    for x in v.iter_mut() {
        *x = small_or_edge(g);
    }
    set_ints(&mut s, v);
    s.last_fps = match g.below(8) {
        0 => 0.0,
        1 => -1.0,
        2 => f32::from_bits(g.u32()),
        3 => g.uniform(0.0, 100.0),
        4 => g.uniform(0.0, 1e-3),
        5 => g.uniform(0.0, 1e9),
        6 => (g.below(2_000_000) as f32) / 10.0,
        _ => g.below(120) as f32,
    };
    s.options = if g.below(2) == 0 { Vec::new() } else { text(g, 20) };
    s.system_spec = if g.below(2) == 0 { Vec::new() } else { text(g, 20) };
    s.last_ladder_host = if g.below(2) == 0 { Vec::new() } else { text(g, 20) };
    for m in maps_mut(&mut s) {
        if g.below(3) == 0 {
            continue;
        }
        for _ in 0..g.below(7) {
            let k = if g.below(20) == 0 { g.i32() } else { g.below(14) as i32 };
            m.insert(k, map_value(g));
        }
    }
    s
}

const KEYS: [&str; 52] = [
    "wins", "losses", "games", "duration", "unitsKilled", "unitsLost", "unitsBuilt", "buildingsKilled", "buildingsLost", "buildingsBuilt", "earnings", "techCaptured",
    "discons", "desyncs", "surrenders", "gamesOf2p", "gamesOf3p", "gamesOf4p", "gamesOf5p", "gamesOf6p", "gamesOf7p", "gamesOf8p", "customGames", "QMGames", "locale",
    "random", "options", "systemSpec", "fps", "lastGeneral", "genInRow", "builtNuke", "builtSCUD", "builtCannon", "challenge", "battle", "WinRow", "WinRowMax", "LossRow",
    "LossRowMax", "DSRow", "DSRowMax", "DCRow", "DCRowMax", "ladderPort", "ladderHost", "bogus", "", "1", "wins2x", "Wins", "fpsx",
];

fn number_text(g: &mut Gen) -> Vec<u8> {
    let forms: [&str; 38] = [
        "0x1p3", "0x1.8p1", "0XA", "1e", "1e+", "-INF", "infinity", "NaN", "0x", " +.5", "5.", "0x.8", "-nan", "1E2x",
        "", "0", "-0", "7", "-7", "+9", "  12", "\t-3", "12abc", "abc", "0x10", "1e3", "1.5", ".5", "-.5e1", "nan", "inf", "-inf", "1e400", "99999999999", "4294967296",
        "-2147483649", "123456789012345678901", "-123456789012345678901",
    ];
    match g.below(6) {
        0 => forms[g.below(38) as usize].as_bytes().to_vec(),
        1 => format!("{}", g.i32()).into_bytes(),
        2 => format!("{}", g.below(100)).into_bytes(),
        3 => format!("{}.{}", g.below(200), g.below(1000)).into_bytes(),
        4 => format!("{}e{}", g.below(100), g.below(12) as i32 - 6).into_bytes(),
        _ => {
            let n = 1 + g.below(25);
            (0..n).map(|_| b'0' + g.below(10) as u8).collect()
        }
    }
}

fn gen_kv(g: &mut Gen) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    if g.below(4) == 0 {
        // a formatted record, perhaps cut
        let s = gen_stats(g);
        let mut f = format_player_kv_pairs(&s, g.below(12) as i32);
        if g.below(3) == 0 && !f.is_empty() {
            let n = g.below(f.len() as u32) as usize;
            f.truncate(n);
        }
        return f;
    }
    for _ in 0..g.below(14) {
        match g.below(12) {
            0 => out.push(b'\\'),
            1 => out.extend_from_slice(b"\\\\"),
            2 => out.extend_from_slice(&text(g, 6)),
            _ => {
                out.push(b'\\');
                out.extend_from_slice(KEYS[g.below(KEYS.len() as u32) as usize].as_bytes());
                if g.below(3) > 0 {
                    let gen = match g.below(6) {
                        0 => format!("{}", g.u32()),
                        1 => format!("{}", g.below(1_000_000_000)),
                        _ => format!("{}", g.below(14)),
                    };
                    out.extend_from_slice(gen.as_bytes());
                }
                out.push(b'\\');
                if g.below(3) == 0 {
                    out.extend_from_slice(&text(g, 12));
                } else {
                    out.extend_from_slice(&number_text(g));
                }
            }
        }
    }
    if g.below(2) == 0 {
        out.push(b'\\');
    }
    out
}

fn cpp_parse(kv: &[u8]) -> Vec<u32> {
    let mut out = vec![0u32; 20_000];
    let n = unsafe { ref_ps_parse(kv.as_ptr() as *const c_char, kv.len() as c_int, out.as_mut_ptr(), out.len() as c_int) };
    assert!(n >= 0, "the parse dump does not fit");
    out.truncate(n as usize);
    out
}

#[test]
fn parse_player_kv_pairs_matches_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("PS_CASES", 20_000);
    let mut g = Gen::new(0x5eed_0001);
    let mut chk = Check::new("ps parsePlayerKVPairs");
    for i in 0..n {
        let kv = gen_kv(&mut g);
        let mut r = Vec::new();
        encode(&mut r, &parse_player_kv_pairs(&kv), false);
        let c = cpp_parse(&kv);
        if r != c {
            println!("case {i}: {:?}", String::from_utf8_lossy(&kv));
        }
        chk.eq(r == c, true, || format!("ps parse case {i}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}

#[test]
fn format_player_kv_pairs_matches_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("PS_CASES", 20_000);
    let mut g = Gen::new(0x5eed_0002);
    let mut chk = Check::new("ps formatPlayerKVPairs");
    for i in 0..n {
        let s = gen_stats(&mut g);
        let tc = g.below(15) as i32;
        let mut w = Vec::new();
        encode(&mut w, &s, false);
        let mut out = vec![0u8; 20_000];
        let k = unsafe { ref_ps_format(w.as_ptr(), tc, out.as_mut_ptr() as *mut c_char, out.len() as c_int) };
        assert!(k >= 0);
        out.truncate(k as usize);
        let r = format_player_kv_pairs(&s, tc);
        if r != out {
            println!("case {i}: rust {:?}\n        c++  {:?}", String::from_utf8_lossy(&r), String::from_utf8_lossy(&out));
        }
        chk.eq(r == out, true, || format!("ps format case {i}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}

/// A program: the words the wrapper reads, and the draft's results in the wrapper's dump format.
fn program(g: &mut Gen) -> (Vec<u32>, Vec<u32>) {
    let q = PsMessageQueue::new();
    let mut input: Vec<u32> = Vec::new();
    let mut out: Vec<u32> = Vec::new();
    let nops = 30 + g.below(51);
    input.push(nops);
    let mut ids: Vec<i32> = Vec::new();
    let pick_stats = |g: &mut Gen, ids: &[i32]| -> PsPlayerStats {
        let mut s = gen_stats(g);
        s.id = if !ids.is_empty() && g.below(2) == 0 { ids[g.below(ids.len() as u32) as usize] } else { g.below(6) as i32 };
        s
    };
    for _ in 0..nops {
        let op = g.below(17);
        input.push(op);
        out.push(0xA5A5A5A5);
        out.push(op);
        match op {
            0 => {
                let a = gen_stats(g);
                let b = gen_stats(g);
                encode(&mut input, &a, false);
                encode(&mut input, &b, false);
                let mut a2 = a.clone();
                a2.incorporate(&b);
                encode(&mut out, &a2, false);
            }
            1 => {
                let a = gen_stats(g);
                encode(&mut input, &a, false);
                encode(&mut out, &PsPlayerStats::copy_constructed(&a), true);
            }
            2 => {
                let s = pick_stats(g, &ids);
                ids.push(s.id);
                encode(&mut input, &s, false);
                q.track_player_stats(s);
            }
            3 => {
                let id = if !ids.is_empty() && g.below(3) > 0 { ids[g.below(ids.len() as u32) as usize] } else { g.below(8) as i32 };
                input.push(id as u32);
                encode(&mut out, &q.find_player_stats_by_id(id), true);
            }
            4 => {
                let mut r = PsRequest::new();
                let ty = g.below(5);
                r.request_type = [PsRequestType::ReadPlayerStats, PsRequestType::UpdatePlayerStats, PsRequestType::UpdatePlayerLocale, PsRequestType::ReadCdKeyStats, PsRequestType::SendGameResToGameSpy][ty as usize];
                r.player = gen_stats(g);
                r.cdkey = text(g, 8);
                r.nick = text(g, 8);
                r.password = text(g, 8);
                r.email = text(g, 8);
                r.results = text(g, 8);
                r.add_discon = g.below(2) == 0;
                r.add_desync = g.below(2) == 0;
                r.last_house = small_or_edge(g);
                input.push(ty);
                encode(&mut input, &r.player, false);
                for s in [&r.cdkey, &r.nick, &r.password, &r.email, &r.results] {
                    put_str(&mut input, s);
                }
                input.push(r.add_discon as u32);
                input.push(r.add_desync as u32);
                input.push(r.last_house as u32);
                q.add_request(r);
            }
            5 => match q.get_request() {
                None => out.push(0),
                Some(r) => {
                    out.push(1);
                    dump_request(&mut out, &r);
                }
            },
            6 => {
                let ty = g.below(3);
                let s = gen_stats(g);
                let pre = g.below(2) == 0;
                input.push(ty);
                encode(&mut input, &s, false);
                input.push(pre as u32);
                let mut r = PsResponse::new();
                r.response_type = [PsResponseType::PlayerStats, PsResponseType::CouldNotConnect, PsResponseType::Preorder][ty as usize];
                r.player = s;
                r.preorder = pre;
                q.add_response(r);
            }
            7 => match q.get_response() {
                None => out.push(0),
                Some(r) => {
                    out.push(1);
                    out.push(match r.response_type {
                        PsResponseType::PlayerStats => 0,
                        PsResponseType::CouldNotConnect => 1,
                        PsResponseType::Preorder => 2,
                    });
                    encode(&mut out, &r.player, true);
                    out.push(r.preorder as u32);
                }
            },
            8 => q.start_thread(),
            9 => q.end_thread(),
            10 => out.push(q.is_thread_running() as u32),
            11 => {
                let id = small_or_edge(g);
                input.push(id as u32);
                q.set_local_player_id(id);
            }
            12 => out.push(q.get_local_player_id() as u32),
            13 => {
                let (e, n, p) = (text(g, 8), text(g, 8), text(g, 8));
                for s in [&e, &n, &p] {
                    put_str(&mut input, s);
                }
                q.set_email(e);
                q.set_nick(n);
                q.set_password(p);
            }
            14 => {
                put_str(&mut out, &q.get_email());
                put_str(&mut out, &q.get_nick());
                put_str(&mut out, &q.get_password());
            }
            15 => dump_request(&mut out, &PsRequest::new()),
            _ => encode(&mut out, &PsPlayerStats::new(), false),
        }
    }
    (input, out)
}

fn dump_request(out: &mut Vec<u32>, r: &PsRequest) {
    out.push(match r.request_type {
        PsRequestType::ReadPlayerStats => 0,
        PsRequestType::UpdatePlayerStats => 1,
        PsRequestType::UpdatePlayerLocale => 2,
        PsRequestType::ReadCdKeyStats => 3,
        PsRequestType::SendGameResToGameSpy => 4,
    });
    encode(out, &r.player, true);
    for s in [&r.cdkey, &r.nick, &r.password, &r.email, &r.results] {
        put_str(out, s);
    }
    out.push(r.add_discon as u32);
    out.push(r.add_desync as u32);
    out.push(r.last_house as u32);
}

#[test]
fn queue_and_record_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("PS_PROGRAMS", 6_000);
    let mut g = Gen::new(0x5eed_0003);
    let mut chk = Check::new("ps queue and record programs");
    for i in 0..n {
        let (input, r) = program(&mut g);
        let mut c = vec![0u32; 400_000];
        let k = unsafe { ref_ps_run(input.as_ptr(), input.len() as c_int, c.as_mut_ptr(), c.len() as c_int) };
        assert!(k >= 0, "the program dump does not fit");
        c.truncate(k as usize);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let op = r[..k].iter().filter(|&&x| x == 0xA5A5A5A5).count();
            let marker = r[..k].iter().rposition(|&x| x == 0xA5A5A5A5).unwrap_or(0);
            println!("case {i}: operation {op} (opcode {}), first difference at word {k}, {} words into it (rust {} words, c++ {} words)", r[marker + 1], k - marker, r.len(), c.len());
        }
        chk.eq(r == c, true, || format!("ps program {i}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
