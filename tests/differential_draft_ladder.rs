#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/LadderDefs.cpp
//   GeneralsMD/Code/GameEngine/Include/GameNetwork/GameSpy/LadderDefs.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The ladder list (GitHub #215, group netlogic): `src/drafts/network/ladder_defs.rs`, every function (`LadderInfo::new`,
//! `parse_ladder`, `LadderList::new`, `find_ladder`, `find_ladder_by_index`, the three getters, `load_local_ladders` and
//! `check_ladder`, the last two through `LadderList::new`).
//!
//! # Method
//!
//! `differential/wrap_draft_ladder.cpp` compiles upstream's `LadderDefs.cpp` whole and unchanged, with the real `AsciiString`. A case is a
//! world (a list of player templates with their sides, playable flags and NULL entries; the quick-match maps; the leftover config text;
//! a list of local ladder files; some texts for `parseLadder` alone; some `findLadder` probes) and both sides build the `LadderList` from
//! it and write every ladder of the three lists with all its fields, then each probe's `findLadder` result, `findLadderByIndex` from -4
//! to 8, and `parseLadder` of each standalone text, as a log of words compared whole.
//!
//! The stand-ins (the draft's `LadderHost` is the same on both sides): the map cache's directory is "Maps" and a map is found when
//! the byte sum of its path is not a multiple of 3 (and has `2 + sum % 7` players); the real <-> portable map path functions prefix
//! "P:" and "R:", then the path is lowercased; multi-byte to wide is one 16-bit unit per byte; a local ladder file is readable under
//! its lower-case name; the local ladder files are listed in the order given (upstream's `FilenameList` is a case-insensitive `std::set`,
//! whose ordering and de-duplication the host owes the draft).
//!
//! Excluded inputs: NUL and bytes of 0x80 and up in a text; a text with a line longer than 700 bytes.

mod support;

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::drafts::network::ladder_defs::{parse_ladder, LadderHost, LadderInfo, LadderList};
use support::{Check, Gen};

extern "C" {
    fn ld_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 1_000_000];
    let n = unsafe { ld_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
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

fn push_bytes(v: &mut Vec<u32>, b: &[u8]) {
    v.push(b.len() as u32);
    for c in b.chunks(4) {
        let mut w = 0u32;
        for (j, x) in c.iter().enumerate() {
            w |= (*x as u32) << (8 * j);
        }
        v.push(w);
    }
}

fn asc(b: &[u8]) -> AsciiString {
    AsciiString::try_from_bytes(b).unwrap()
}

struct Template {
    null: bool,
    side: String,
    playable: bool,
}

struct World {
    templates: Vec<Template>,
    qm_maps: Vec<String>,
    leftover: String,
    files: Vec<(String, String)>,
    standalone: Vec<String>,
    probes: Vec<(String, u32)>,
}

fn map_sum(p: &[u8]) -> u32 {
    p.iter().map(|c| *c as u32).sum()
}

struct Host<'a> {
    w: &'a World,
}

impl LadderHost for Host<'_> {
    fn all_side_strings(&self) -> Vec<AsciiString> {
        self.w.templates.iter().map(|t| asc(t.side.as_bytes())).collect()
    }
    fn playable_non_boss_sides(&self) -> Vec<AsciiString> {
        self.w.templates.iter().filter(|t| !t.null && t.playable && t.side != "Boss").map(|t| asc(t.side.as_bytes())).collect()
    }
    fn qm_maps(&self) -> Vec<AsciiString> {
        self.w.qm_maps.iter().map(|m| asc(m.as_bytes())).collect()
    }
    fn map_path_for(&self, map_name: &AsciiString) -> AsciiString {
        let n = map_name.str();
        let mut p = b"R:P:Maps\\".to_vec();
        p.extend_from_slice(n);
        p.push(b'\\');
        p.extend_from_slice(n);
        p.extend_from_slice(b".map");
        asc(&p.iter().map(|c| c.to_ascii_lowercase()).collect::<Vec<u8>>())
    }
    fn map_num_players(&self, map_path: &AsciiString) -> Option<i32> {
        let s = map_sum(map_path.str());
        if s % 3 == 0 {
            None
        } else {
            Some(2 + (s % 7) as i32)
        }
    }
    fn local_ladder_files(&self) -> Vec<AsciiString> {
        self.w.files.iter().map(|(n, _)| asc(n.as_bytes())).collect()
    }
    fn read_text_file(&self, name: &AsciiString) -> Vec<u8> {
        // the C++ table is keyed by the lower-case name of every listed file (the last of equal ones wins) and looked up by the exact name
        let mut found = None;
        for (n, c) in &self.w.files {
            if n.to_ascii_lowercase().as_bytes() == name.str() {
                found = Some(c.as_bytes().to_vec());
            }
        }
        found.unwrap_or_default()
    }
    fn multi_byte_to_wide_single_line(&self, s: &[u8]) -> Vec<u16> {
        s.iter().map(|c| *c as u16).collect()
    }
}

fn dump(o: &mut Vec<u32>, l: Option<&LadderInfo>) {
    let Some(l) = l else {
        o.push(0);
        return;
    };
    o.push(1);
    for u in [&l.name, &l.description, &l.location] {
        o.push(u.len() as u32);
        for c in u.iter() {
            o.push(*c as u32);
        }
    }
    o.push(l.players_per_team as u32);
    o.push(l.min_wins as u32);
    o.push(l.max_wins as u32);
    o.push(l.random_maps as u32);
    o.push(l.random_factions as u32);
    o.push(l.valid_qm as u32);
    o.push(l.valid_custom as u32);
    o.push(l.valid_maps.len() as u32);
    for m in &l.valid_maps {
        push_bytes(o, m.str());
    }
    o.push(l.valid_factions.len() as u32);
    for m in &l.valid_factions {
        push_bytes(o, m.str());
    }
    push_bytes(o, l.crypted_password.str());
    push_bytes(o, l.address.str());
    o.push(l.port as u32);
    push_bytes(o, l.homepage_url.str());
    o.push(l.submit_replay as u32);
    o.push(l.index as u32);
}

fn rust_run(w: &World) -> Vec<u32> {
    let host = Host { w };
    let list = LadderList::new(&host, &asc(w.leftover.as_bytes()));
    let mut o = Vec::new();
    for v in [list.get_standard_ladders(), list.get_special_ladders(), list.get_local_ladders()] {
        o.push(v.len() as u32);
        for l in v {
            dump(&mut o, Some(l));
        }
    }
    for (a, p) in &w.probes {
        dump(&mut o, list.find_ladder(&asc(a.as_bytes()), *p as u16));
    }
    for idx in -4..=8 {
        dump(&mut o, list.find_ladder_by_index(idx));
    }
    for t in &w.standalone {
        let l = parse_ladder(&host, &asc(t.as_bytes()));
        dump(&mut o, l.as_ref());
    }
    o
}

fn encode(w: &World) -> Vec<u32> {
    let mut v = vec![w.templates.len() as u32];
    for t in &w.templates {
        v.push(t.null as u32);
        push_bytes(&mut v, t.side.as_bytes());
        v.push(t.playable as u32);
    }
    v.push(w.qm_maps.len() as u32);
    for m in &w.qm_maps {
        push_bytes(&mut v, m.as_bytes());
    }
    push_bytes(&mut v, w.leftover.as_bytes());
    v.push(w.files.len() as u32);
    for (n, c) in &w.files {
        push_bytes(&mut v, n.as_bytes());
        push_bytes(&mut v, c.as_bytes());
    }
    v.push(w.standalone.len() as u32);
    for t in &w.standalone {
        push_bytes(&mut v, t.as_bytes());
    }
    v.push(w.probes.len() as u32);
    for (a, p) in &w.probes {
        push_bytes(&mut v, a.as_bytes());
        v.push(*p);
    }
    v
}

const SIDES: [&str; 6] = ["America", "China", "GLA", "Boss", "Observer", "Civilian"];
const MAPS: [&str; 7] = ["alpha", "Bravo", "charlie", "Delta", "echo", "foxtrot", "golf"];

fn pick<'a>(g: &mut Gen, v: &'a [&'a str]) -> &'a str {
    v[g.below(v.len() as u32) as usize]
}

fn full_path(m: &str) -> String {
    let p = format!("R:P:Maps\\{m}\\{m}.map");
    p.to_ascii_lowercase()
}

fn ladder_lines(g: &mut Gen, kind: u32, fixed: Option<(&str, u32)>) -> Vec<String> {
    let mut l = Vec::new();
    let host = ["ladder.example", "10.0.0.5", "a.b.c", "srv"][g.below(4) as usize];
    let port = [0u32, 1, 4000, 6500, 65535, 70000, 300][g.below(7) as usize];
    let (host, port) = fixed.unwrap_or((host, port));
    let home = ["http://x.y/z", "home", "", "h h"][g.below(4) as usize];
    l.push(match g.below(5) {
        0 => format!("<Ladder \"{host}\" {port} \"{home}\">"),
        1 => format!("<Ladder {host} {port} {home}>"),
        2 => format!("<Ladder {host} {port}>"),
        3 => format!("<Ladder  {host} {port} {home}>"),
        _ => format!("<Ladder {host}>"),
    });
    let nl = g.below(14);
    for _ in 0..nl {
        let line = match g.below(18) {
            0 => format!("Name {}", ["Tournament Ladder", "A", "A very long ladder name indeed, longer than twenty", "x y z"][g.below(4) as usize]),
            1 => format!("Desc {}", ["For experts", "d", ""][g.below(3) as usize]),
            2 => format!("Loc {}", ["Europe", "Asia"][g.below(2) as usize]),
            3 => format!("TeamSize {}", [1u32, 2, 3, 4, 5, 0, 9][g.below(7) as usize]),
            4 => format!("RandomMaps {}", g.below(3)),
            5 => format!("RandomFactions {}", g.below(3)),
            6 | 7 => format!("Faction {}", pick(g, &SIDES)),
            8 => format!("MinWins {}", g.below(30)),
            9 => format!("MaxWins {}", g.below(300)),
            10 => format!("CryptedPass {}", ["abc123", "x", "p w"][g.below(3) as usize]),
            11 | 12 | 13 => format!("Map {}", ["alpha", "Bravo", "charlie", "Delta", "echo", "foxtrot", "golf", "unknown", " alpha "][g.below(9) as usize]),
            14 => ["QM 1", "Custom 0", "Bogus line"][g.below(3) as usize].to_string(),
            15 => String::new(),
            16 => format!("Name{}", "x"),
            _ => format!("Faction {}", ["GLA", "America", "Nope"][g.below(3) as usize]),
        };
        let line = match g.below(12) {
            0 => format!("{line}\r"),
            1 => format!("  {line}  "),
            _ => line,
        };
        l.push(line);
    }
    if kind != 1 || g.below(4) != 0 {
        l.push(if g.below(20) == 0 { "</Ladder> ".to_string() } else { "</Ladder>".to_string() });
    }
    l
}

fn gen_world(g: &mut Gen) -> World {
    let mut templates = Vec::new();
    for _ in 0..g.below(8) {
        templates.push(Template { null: g.below(7) == 0, side: pick(g, &SIDES).to_string(), playable: g.below(4) != 0 });
    }
    let mut qm_maps = Vec::new();
    for m in MAPS.iter() {
        if g.below(3) != 0 {
            qm_maps.push(full_path(m));
        }
    }
    if g.below(10) == 0 {
        qm_maps.push("other\\map".to_string());
    }
    let shared = if g.below(2) == 0 { Some(("ladder.example", [4000u32, 6500, 300][g.below(3) as usize])) } else { None };
    let mut leftover = String::new();
    let nblocks = g.below(5);
    let mut texts = Vec::new();
    for _ in 0..nblocks {
        let special = g.below(2) == 0;
        let (open, close) = if special { ("<SpecialLadders>", "</SpecialLadders>") } else { ("<Ladders>", "</Ladders>") };
        let mut lines = vec![open.to_string()];
        for _ in 0..g.below(4) {
            let l = ladder_lines(g, 1, shared);
            texts.push(l.join("\n") + "\n");
            lines.extend(l);
        }
        if g.below(8) != 0 {
            lines.push(close.to_string());
        }
        if g.below(5) == 0 {
            lines.push(format!("noise {}", g.below(9)));
        }
        for l in lines {
            leftover.push_str(&l);
            leftover.push_str(["\n", "\n", "\r\n"][g.below(3) as usize]);
        }
    }
    let mut files = Vec::new();
    for i in 0..g.below(4) {
        let name = if g.below(2) == 0 { format!("Users\\X\\Ladders\\Lad{i}.INI") } else { format!("ladders\\lad{i}.ini") };
        let l = ladder_lines(g, 2, shared);
        let mut content = l.join("\n");
        if g.below(5) != 0 {
            content.push('\n');
        }
        if g.below(10) == 0 {
            content = String::new();
        }
        files.push((name, content));
    }
    let mut standalone = Vec::new();
    for _ in 0..g.below(3) {
        let l = ladder_lines(g, 1, None);
        standalone.push(l.join("\n") + "\n");
    }
    let mut probes = Vec::new();
    if let Some((h, p)) = shared {
        probes.push((h.to_string(), p));
    }
    for _ in 0..g.below(4) {
        probes.push((["ladder.example", "10.0.0.5", "a.b.c", "srv", "none"][g.below(5) as usize].to_string(), [0u32, 1, 4000, 6500, 65535, 300][g.below(6) as usize]));
    }
    let _ = texts;
    World { templates, qm_maps, leftover, files, standalone, probes }
}

#[test]
fn the_ladder_list_parses_as_upstream() {
    let n = cases("NETLOGIC_LADDER_CASES", 12000);
    let mut g = Gen::new(0x215_3001);
    let mut chk = Check::new("LadderList and parseLadder");
    let (mut std_n, mut spec_n, mut loc_n, mut found) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..n {
        let w = gen_world(&mut g);
        let v = encode(&w);
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&v);
        let r = rust_run(&w);
        drop(_one);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            println!(
                "ladder case {i}: first difference at word {k} (rust {} words, c++ {} words)\nleftover:\n{}\nfiles: {:?}\nstandalone: {:?}",
                r.len(),
                c.len(),
                w.leftover,
                w.files,
                w.standalone
            );
        }
        let host = Host { w: &w };
        let list = LadderList::new(&host, &asc(w.leftover.as_bytes()));
        std_n += list.get_standard_ladders().len() as u64;
        spec_n += list.get_special_ladders().len() as u64;
        loc_n += list.get_local_ladders().len() as u64;
        found += (1..9).filter(|k| list.find_ladder_by_index(*k).is_some()).count() as u64;
        chk.eq(r, c, || format!("ladder case {i}"));
    }
    println!("{n} worlds ({std_n} standard, {spec_n} special, {loc_n} local ladders; {found} found by index)");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
