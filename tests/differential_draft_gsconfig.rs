#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/GSConfig.cpp
//   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/GSConfig.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The GameSpy config parser (GitHub #215, group netlogic): `src/drafts/network/gs_config.rs`, every function (the constructor, which is
//! the parser, and every getter).
//!
//! # Method
//!
//! `differential/wrap_draft_gsconfig.cpp` compiles upstream's `GSConfig.cpp` whole and unchanged. A case is a config text: lines drawn
//! from the eight sections (`<PingServers>`, `<PingDuration>`, `<QMMaps>`, `<Manglers>`, `<QMBot>`, `<VIP>`, `<NAT>`, `<Custom>`) with
//! their keys and values, section tags opened and closed in and out of order (nested, repeated, unclosed, with the wrong case),
//! lines with `\r`, tabs, runs of spaces, `=` and spaces as separators, unknown keys, keys without values, numbers that are negative,
//! zero, large or not numbers, and text outside any section (the leftover config). Both sides parse it and write every getter's
//! result (ping servers, timings, quick-match maps and bot id and channel, `getPointsForRank` from -2 to 12, `isPlayerVIP` from -1
//! to 40, `getManglerLocation` from -1 to 12 with its host and port, the leftover text, the NAT timings and the custom-match flag)
//! as a log of words, compared whole.
//!
//! The draft's `<QMMaps>` lookup is a closure that stands for `TheMapCache` and `TheGameState` (the game's map cache and the
//! real <-> portable path conversions). The C++ stand-ins (see the wrapper) make the path `Maps\<name>\<name>.map` pass through "P:"
//! and "R:" prefixes, lowercase it, and find it when the byte sum of the path is not a multiple of 3; the closure below does the same,
//! so that what is compared is the parser (the lowercasing of the line before the lookup, which lines reach the lookup, and what is
//! kept), not the map cache.
//!
//! Excluded inputs: bytes of 0x80 and up and NUL in the text (`isspace` of a negative `char` is undefined in C++, and a NUL ends
//! the C string), and lines of more than 700 bytes (`AsciiString::format` writes at most 2048).

mod support;

use rts_lockstep_engine::drafts::network::gs_config::GameSpyConfig;
use support::{Check, Gen};

extern "C" {
    fn gc_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 200_000];
    let n = unsafe { gc_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn emit_bytes(o: &mut Vec<u32>, b: &[u8]) {
    o.push(b.len() as u32);
    for c in b.chunks(4) {
        let mut v = 0u32;
        for (j, x) in c.iter().enumerate() {
            v |= (*x as u32) << (8 * j);
        }
        o.push(v);
    }
}

fn qm_lookup(line: &[u8]) -> Option<Vec<u8>> {
    let mut p = b"R:P:Maps\\".to_vec();
    p.extend_from_slice(line);
    p.push(b'\\');
    p.extend_from_slice(line);
    p.extend_from_slice(b".map");
    let p: Vec<u8> = p.iter().map(|c| c.to_ascii_lowercase()).collect();
    let sum: u32 = p.iter().map(|c| *c as u32).sum();
    if sum % 3 != 0 {
        Some(p)
    } else {
        None
    }
}

fn rust_run(text: &[u8]) -> Vec<u32> {
    let mut c = GameSpyConfig::new(text, &qm_lookup);
    let mut o = Vec::new();
    let ps = c.get_ping_servers();
    o.push(ps.len() as u32);
    for s in &ps {
        emit_bytes(&mut o, s.str());
    }
    o.push(c.get_num_ping_repetitions() as u32);
    o.push(c.get_ping_timeout_in_ms() as u32);
    o.push(c.get_ping_cutoff_good() as u32);
    o.push(c.get_ping_cutoff_bad() as u32);
    let qm = c.get_qm_maps();
    o.push(qm.len() as u32);
    for s in &qm {
        emit_bytes(&mut o, s.str());
    }
    o.push(c.get_qm_bot_id() as u32);
    o.push(c.get_qm_channel() as u32);
    c.set_qm_channel(7);
    o.push(c.get_qm_channel() as u32);
    for r in -2..=12 {
        o.push(c.get_points_for_rank(r) as u32);
    }
    for id in -1..=40 {
        o.push(c.is_player_vip(id) as u32);
    }
    for i in -1..=12 {
        match c.get_mangler_location(i) {
            Some((h, p)) => {
                o.push(1);
                emit_bytes(&mut o, h.str());
                o.push(p as u32);
            }
            None => o.push(0),
        }
    }
    emit_bytes(&mut o, c.get_leftover_config().str());
    o.push(c.restrict_games_to_lobby() as u32);
    o.push(c.get_time_between_retries() as u32);
    o.push(c.get_max_mangler_retries() as u32);
    o.push(c.get_retry_interval() as u32);
    o.push(c.get_keepalive_interval() as u32);
    o.push(c.get_port_timeout() as u32);
    o.push(c.get_round_timeout() as u32);
    o
}

const SECTIONS: [&str; 8] = ["PingServers", "PingDuration", "QMMaps", "Manglers", "QMBot", "VIP", "NAT", "Custom"];

fn number(g: &mut Gen) -> String {
    match g.below(8) {
        0 => "0".into(),
        1 => format!("-{}", g.below(500)),
        2 => format!("{}", g.below(30)),
        3 => format!("{}", g.below(100000)),
        4 => "abc".into(),
        5 => format!("{}x", g.below(50)),
        6 => format!("{}", g.u32()),
        _ => format!("{}", g.below(2000)),
    }
}

fn word(g: &mut Gen) -> String {
    let w = ["alpha", "Bravo", "ping.example", "MAP_one", "tournament", "x", "a_b", "Zed9", "host1", "Ladder"];
    w[g.below(w.len() as u32) as usize].to_string()
}

fn sep(g: &mut Gen) -> &'static str {
    ["=", " = ", " ", "  =  ", "=\t", "\t=\t", "==", " =="][g.below(8) as usize]
}

fn body_line(g: &mut Gen, sec: usize) -> String {
    let case = |g: &mut Gen, s: &str| -> String {
        match g.below(3) {
            0 => s.to_uppercase(),
            1 => s.to_string(),
            _ => {
                let mut t = s.to_string();
                if let Some(c) = t.get_mut(0..1) {
                    c.make_ascii_uppercase();
                }
                t
            }
        }
    };
    match sec {
        0 => word(g),
        1 => {
            let k = ["reps", "timeout", "low", "med", "other"][g.below(5) as usize];
            if g.below(8) == 0 {
                case(g, k)
            } else {
                format!("{}{}{}", case(g, k), sep(g), number(g))
            }
        }
        2 => word(g),
        3 => match g.below(6) {
            0 => format!("{}:{}", word(g), number(g)),
            1 => format!("{}", word(g)),
            2 => format!(":{}", number(g)),
            3 => format!("{}:", word(g)),
            4 => format!("  {} : {}  ", word(g), number(g)),
            _ => format!("{}:{}:{}", word(g), g.below(65536), word(g)),
        },
        4 => {
            let k = ["id", "ID", "name"][g.below(3) as usize];
            format!("{}{}{}", k, sep(g), number(g))
        }
        5 => number(g),
        6 => {
            let k = ["retryinterval", "manglerretries", "manglerinterval", "keepaliveinterval", "porttimeout", "roundtimeout", "bogus", "RetryInterval"]
                [g.below(8) as usize];
            if g.below(10) == 0 {
                k.to_string()
            } else {
                format!("{}{}{}", k, sep(g), number(g))
            }
        }
        _ => {
            let k = ["restricted", "Restricted", "other"][g.below(3) as usize];
            if g.below(8) == 0 {
                k.to_string()
            } else {
                format!("{}{}{}", k, sep(g), number(g))
            }
        }
    }
}

fn gen_config(g: &mut Gen) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let nsec = g.below(6);
    for _ in 0..nsec {
        let sec = g.below(8) as usize;
        let name = SECTIONS[sec];
        let tag = |g: &mut Gen, open: bool| -> String {
            let n = if g.below(12) == 0 { name.to_lowercase() } else { name.to_string() };
            let t = if open { format!("<{n}>") } else { format!("</{n}>") };
            match g.below(10) {
                0 => format!("  {t}"),
                1 => format!("{t}  "),
                2 => format!("{t}\r"),
                3 => format!("{t} x"),
                _ => t,
            }
        };
        lines.push(tag(g, true));
        for _ in 0..g.below(7) {
            if g.below(15) == 0 {
                // a stray tag of another section, inside this one
                let o = SECTIONS[g.below(8) as usize];
                lines.push(if g.below(2) == 0 { format!("<{o}>") } else { format!("</{o}>") });
            } else {
                lines.push(body_line(g, sec));
            }
            if g.below(10) == 0 {
                lines.push(String::new());
            }
        }
        if g.below(8) != 0 {
            lines.push(tag(g, false));
        }
        for _ in 0..g.below(3) {
            // text outside the sections: the leftover config
            lines.push(format!("{} {}", word(g), word(g)));
        }
    }
    let mut text = String::new();
    for l in &lines {
        text.push_str(l);
        text.push_str(["\n", "\n", "\r\n", "\n\n"][g.below(4) as usize]);
    }
    if g.below(6) == 0 {
        text.pop();
    }
    text.into_bytes()
}

#[test]
fn the_game_spy_config_parses_as_upstream() {
    let n = cases("NETLOGIC_GSCONFIG_CASES", 12000);
    let mut g = Gen::new(0x215_2001);
    let mut chk = Check::new("GameSpyConfig parse and getters");
    let (mut maps, mut vips, mut manglers, mut leftover) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..n {
        let text = gen_config(&mut g);
        let mut v = vec![text.len() as u32];
        for c in text.chunks(4) {
            let mut w = 0u32;
            for (j, x) in c.iter().enumerate() {
                w |= (*x as u32) << (8 * j);
            }
            v.push(w);
        }
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&v);
        let r = rust_run(&text);
        drop(_one);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            println!("gs config case {i}: first difference at word {k} (rust {} words, c++ {} words)\n{}", r.len(), c.len(), String::from_utf8_lossy(&text));
        }
        let s = String::from_utf8_lossy(&text);
        maps += s.contains("<QMMaps>") as u64;
        vips += s.contains("<VIP>") as u64;
        manglers += s.contains("<Manglers>") as u64;
        leftover += (c.len() > 300) as u64;
        chk.eq(r, c, || format!("gs config case {i}"));
    }
    println!("{n} configs ({maps} with QMMaps, {vips} with VIP, {manglers} with Manglers, {leftover} long logs)");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
