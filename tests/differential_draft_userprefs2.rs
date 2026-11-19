#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/UserPreferences.cpp (QuickMatchPreferences, CustomMatchPreferences,
//   GameSpyMiscPreferences)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::user_preferences` against the online preference classes (#215).
//!
//! # Method
//!
//! The three classes (QuickmatchPreferences.h, CustomMatchPreferences.h, GameSpyMiscPreferences.h), their sections of
//! UserPreferences.cpp (constructors included) and the quoted-printable functions (QuotedPrintable.cpp) are cut verbatim
//! (build.rs) into `differential/wrap_draft_userprefs2.cpp`. The base class is the verified wrapper's cut
//! (`differential/wrap_user_prefs.cpp`, OUT_DIR/user_prefs_gen/user_prefs_cut.inc), reused, not cut again, and compiled here
//! under names of its own with the real AsciiString.cpp. The wrapper stands in for the globals the Custom class reads (see its
//! banner). 30,000 programs of 5 to 60 ops from a fixed seed run on both sides: the three objects' every accessor, with stored
//! values drawn from numbers (signs, spaces, `\v`, `\f`, overflow-free), words ("1", "Yes", "yes", "YES"), quoted-printable maps
//! and junk; keys written raw (with other case) so that a lookup meets a value no setter wrote; player template tables
//! (a NULL entry, an empty starting building, old and new factions), the game info (absent, old factions only, not), locked
//! generals, languages (Chinese and Korean in several cases), valid and official map masks. Every answer is compared, and each
//! program ends with a dump of the three maps. A further 300 programs build each class with a local profile id (0 to 20, and
//! -1, 99999) over a folder holding a marker file under the draft's `filename(profile)` and read whether the constructor loaded it.
//!
//! Excluded (undefined, or differing by platform, upstream): integers stored outside the range of `Int` (`atoi` overflows);
//! `StartingCash` text of more than 4,294,967,295 (the shipped 32-bit `strtoul` saturates, the host's 64-bit one wraps); a
//! stored `Map` decoding to 1,023 bytes or more.
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! Mismatches in `getStartingCash` for a signed value (`strtoul` accepts `+`/`-`; the draft read digits only) and in
//! `getPreferredMap` for a map name padded with `\v` (`trim` uses `isspace`; the draft used Rust's whitespace set); after fixing
//! those, 0.
//!
//! # Results
//!
//! Run 2026-11-19: file names 72 compared, 0 mismatches; accessors 30,000 programs compared. First run: 15,740 mismatches, all
//! one defect, a MISS against the prediction above (which expected `getStartingCash` and `getPreferredMap` only, and those had
//! been fixed beforehand): the draft's `isMapSelected` of a name with no key returned FALSE where upstream returns TRUE (the
//! plant 14321 had been written as `return plant(14321)`, so the unplanted branch was the wrong value). Fixed in the draft;
//! after: 0 mismatches, max ULP 0. (One test-generator slip was also fixed: an index past a 9-byte alphabet.) Plants
//! 14321..14337: all 17 caught (49 to 15,740 of 30,000 programs; 14336 caught by 24 of the 72 file-name cases); baseline 0.
//! Excluded inputs as above (`strtoul` saturates on the shipped 32-bit build, wraps on a 64-bit host).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 14321 `isMapSelected` of an unknown map is FALSE; 14322 the ladder port is cut to 8 bits; 14323 `MaxPoints`
//! defaults to 100; 14324 the colour bound is `>`; 14325 the chat slider allows 101; 14326 the observer template is kept;
//! 14327 `oldFactionsOnly` is ignored; 14328 a locked general is kept and an unlocked one dropped; 14329 `usesLongGameList` reads
//! the key; 14330 the Asian text default is inverted; 14331 the preferred map is not trimmed; 14332 an unofficial map is kept when
//! recording stats; 14333 `StartingCash` is written as unsigned; 14334 `getSuperweaponRestricted` is case-sensitive;
//! 14335 `MaxMessagesPerUpdate` defaults to 10; 14336 the quick match file is the custom one's; 14337 `strtoul` ignores a minus sign.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::user_preferences::*;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_up2_run(input: *const i32, nin: i32, dir: *const std::os::raw::c_char, out: *mut i32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

const KEYS: [&str; 24] = [
    "LastLadderAddr", "LastLadderPort", "MaxDisconnects", "MaxPoints", "MinPoints", "WaitTime", "NumPlayers", "MaxPing", "Color", "Side",
    "ChatSlider", "PlayerTemplate", "UseSystemMapDir", "UseLongGameList", "AllowObservers", "DisallowAsianText", "DisallowNonAsianText",
    "Map", "SuperweaponRestrict", "StartingCash", "LimitArmies", "UseStats", "Locale", "MaxMessagesPerUpdate",
];

fn put(v: &mut Vec<i32>, s: &[u8]) {
    v.push(s.len() as i32);
    v.extend(s.iter().map(|&b| b as i32));
}

fn key(g: &mut Gen) -> Vec<u8> {
    match g.below(12) {
        0 => b"QMResLock".to_vec(),
        1 => b"CachedStats".to_vec(),
        2 => KEYS[g.below(24) as usize].to_lowercase().into_bytes(),
        3 => format!("junk{}", g.below(3)).into_bytes(),
        _ => KEYS[g.below(24) as usize].as_bytes().to_vec(),
    }
}

fn number(g: &mut Gen, wide: bool) -> Vec<u8> {
    let n: i64 = match g.below(8) {
        0 => 0,
        1 => g.i32() as i64,
        2 => -(g.below(200) as i64),
        3 => g.below(70000) as i64,
        4 if wide => (g.u32()) as i64,
        4 => (g.i32()) as i64,
        _ => g.below(120) as i64,
    };
    let mut s = String::new();
    match g.below(8) {
        0 => s.push(' '),
        1 => s.push('\x0B'),
        2 => s.push_str(" \t"),
        3 => s.push('\x0C'),
        _ => {}
    }
    if n >= 0 && g.below(6) == 0 {
        s.push('+');
    }
    s.push_str(&n.to_string());
    match g.below(8) {
        0 => s.push_str("abc"),
        1 => s.push(' '),
        2 => s.push_str(".5"),
        3 => s.push('\x0B'),
        _ => {}
    }
    if g.below(15) == 0 {
        s = ["", "abc", "-", "+", "  ", "0x10", "1e3"][g.below(7) as usize].to_string();
    }
    s.into_bytes()
}

fn qp(g: &mut Gen, name: &[u8]) -> Vec<u8> {
    let mut o = Vec::new();
    for &b in name {
        if b.is_ascii_alphanumeric() && g.below(5) != 0 {
            o.push(b);
        } else {
            o.extend_from_slice(format!("_{:02X}", b).as_bytes());
        }
    }
    o
}

fn map_name(g: &mut Gen) -> Vec<u8> {
    let mut s = match g.below(6) {
        0 => format!("Map{}", 8 + g.below(3)).into_bytes(),
        _ => format!("Map{}", g.below(8)).into_bytes(),
    };
    match g.below(8) {
        0 => s.insert(0, b' '),
        1 => s.insert(0, 0x0B),
        2 => s.push(b' '),
        3 => s.push(0x0B),
        4 => s.extend_from_slice(b" \t\x0C"),
        _ => {}
    }
    s
}

fn value(g: &mut Gen, wide: bool) -> Vec<u8> {
    match g.below(9) {
        0 | 1 | 2 => number(g, wide),
        3 => ["1", "0", "Yes", "yes", "YES", "No", "no", "yEs", "1 ", " 1"][g.below(10) as usize].as_bytes().to_vec(),
        4 => {
            let m = map_name(g);
            qp(g, &m)
        }
        5 => {
            let m = map_name(g);
            m
        }
        6 => b"_".to_vec(),
        7 => {
            let n = g.below(30) as usize;
            (0..n).map(|_| b"ab_1F xY\x0B"[g.below(9) as usize]).collect()
        }
        _ => ["", "x", "10.0.0.1", "ladder.example"][g.below(4) as usize].as_bytes().to_vec(),
    }
}

fn int_val(g: &mut Gen) -> i32 {
    match g.below(6) {
        0 => g.i32(),
        1 => -1 - g.below(3) as i32,
        2 => 98 + g.below(6) as i32,
        _ => g.below(30) as i32,
    }
}

struct Tables {
    num_colors: i32,
    templates: Vec<(i32, bool, i32)>,
    game_info: i32,
    generals: Vec<(i32, bool)>,
    lang: usize,
    money: u32,
    default_map: Vec<u8>,
    valid: i32,
    official: i32,
}

const LANGS: [&str; 7] = ["english", "chinese", "korean", "CHINESE", "Korean", "", "koreanx"];

fn header(g: &mut Gen, v: &mut Vec<i32>) -> Tables {
    let t = Tables {
        num_colors: g.below(10) as i32,
        templates: (0..g.below(8)).map(|_| ([0, 0, 0, 1, 2][g.below(5) as usize], g.below(2) == 0, g.below(4) as i32)).collect(),
        game_info: g.below(4) as i32,
        generals: (0..g.below(5)).map(|_| (g.below(4) as i32, g.below(2) == 0)).collect(),
        lang: g.below(7) as usize,
        money: if g.below(3) == 0 { g.u32() } else { g.below(100000) },
        default_map: format!("Map{}", g.below(8)).into_bytes(),
        valid: g.below(256) as i32,
        official: g.below(256) as i32,
    };
    v.push(t.num_colors);
    v.push(t.templates.len() as i32);
    for &(k, o, n) in &t.templates {
        v.extend_from_slice(&[k, o as i32, n]);
    }
    v.push(t.game_info);
    v.push(t.generals.len() as i32);
    for &(n, e) in &t.generals {
        v.extend_from_slice(&[n, e as i32]);
    }
    v.push(t.lang as i32);
    v.push(t.money as i32);
    put(v, &t.default_map);
    v.push(t.valid);
    v.push(t.official);
    t
}

fn ops(g: &mut Gen, v: &mut Vec<i32>) {
    for _ in 0..(5 + g.below(56)) {
        match g.below(40) {
            0..=5 => {
                v.extend_from_slice(&[0, g.below(3) as i32]);
                let k = key(g);
                put(v, &k);
                let x = value(g, k == b"StartingCash");
                put(v, &x);
            }
            6 => {
                v.push(1);
                let m = map_name(g);
                put(v, &m);
                v.push(g.below(2) as i32);
            }
            7 => {
                v.push(2);
                let m = map_name(g);
                put(v, &m);
            }
            8 => {
                v.extend_from_slice(&[3, g.below(2) as i32]);
                let a = value(g, false);
                put(v, &a);
                v.push(match g.below(3) { 0 => g.i32(), _ => g.below(70000) as i32 });
            }
            9 => v.extend_from_slice(&[4, g.below(2) as i32]),
            10 => v.extend_from_slice(&[5, g.below(2) as i32]),
            11 => v.extend_from_slice(&[6, g.below(8) as i32, int_val(g)]),
            12..=14 => v.extend_from_slice(&[7, g.below(8) as i32]),
            15 => v.extend_from_slice(&[8, int_val(g)]),
            16 => v.push(9),
            17 => v.extend_from_slice(&[10, int_val(g)]),
            18 => v.push(11),
            19 => v.extend_from_slice(&[12, int_val(g)]),
            20 | 21 => v.push(13),
            22 => v.extend_from_slice(&[14, g.below(8) as i32, g.below(2) as i32]),
            23..=25 => v.extend_from_slice(&[15, g.below(8) as i32]),
            26 => {
                v.push(16);
                let m = map_name(g);
                put(v, &m);
            }
            27 | 28 => v.push(17),
            29 => v.extend_from_slice(&[18, if g.below(2) == 0 { g.u32() as i32 } else { g.below(5000) as i32 }]),
            30 | 31 => v.push(19),
            32 => v.extend_from_slice(&[20, int_val(g)]),
            33 => v.push(21),
            34 => {
                v.push(22);
                let x = value(g, false);
                put(v, &x);
            }
            35 => v.push(23),
            36 => v.push(24),
            37 => v.push(25),
            _ => v.push(26),
        }
    }
    v.push(26);
}

struct Host<'a>(&'a Tables);

impl MatchPrefsHost for Host<'_> {
    fn local_profile_id(&self) -> i32 {
        0
    }
    fn num_colors(&self) -> i32 {
        self.0.num_colors
    }
    fn player_template_count(&self) -> i32 {
        self.0.templates.len() as i32
    }
    fn nth_player_template(&self, n: i32) -> Option<(bool, bool, Vec<u8>)> {
        let &(k, old, name) = self.0.templates.get(usize::try_from(n).ok()?)?;
        if k == 2 {
            return None;
        }
        Some((k == 1, old, format!("Tpl{name}").into_bytes()))
    }
    fn old_factions_only(&self) -> bool {
        self.0.game_info == 1
    }
    fn general_starting_enabled(&self, template_name: &[u8]) -> Option<bool> {
        // the map keeps the last value written for a name
        self.0.generals.iter().rev().find(|&&(n, _)| format!("Tpl{n}").as_bytes() == template_name).map(|&(_, e)| e)
    }
    fn registry_language(&self) -> Vec<u8> {
        LANGS[self.0.lang].as_bytes().to_vec()
    }
    fn default_starting_money(&self) -> u32 {
        self.0.money
    }
    fn default_official_map(&self) -> Vec<u8> {
        self.0.default_map.clone()
    }
    fn is_valid_map(&self, name: &[u8]) -> bool {
        map_index(name).map_or(false, |k| self.0.valid >> k & 1 != 0)
    }
    fn is_official_map(&self, name: &[u8]) -> bool {
        map_index(name).map_or(false, |k| self.0.official >> k & 1 != 0)
    }
}

fn map_index(n: &[u8]) -> Option<u32> {
    if n.len() == 4 && &n[..3] == b"Map" && (b'0'..=b'7').contains(&n[3]) {
        Some((n[3] - b'0') as u32)
    } else {
        None
    }
}

fn dump_map(p: &rts_lockstep_engine::client::user_preferences::UserPreferences, o: &mut Vec<i32>) {
    o.push(p.map.len() as i32);
    for (k, v) in &p.map {
        put(o, k);
        put(o, v);
    }
}

fn run_rust(p: &[i32], t: &Tables) -> Vec<i32> {
    let host = Host(t);
    let mut at = 0;
    // skip the header: the caller passes the ops only
    let mut q = QuickMatchPreferences::default();
    let mut c = CustomMatchPreferences::default();
    let mut m = GameSpyMiscPreferences::default();
    let mut o = Vec::new();
    let rd = |at: &mut usize| -> Vec<u8> {
        let n = p[*at] as usize;
        *at += 1;
        let s: Vec<u8> = p[*at..*at + n].iter().map(|&x| x as u8).collect();
        *at += n;
        s
    };
    while at < p.len() {
        let kind = p[at];
        at += 1;
        match kind {
            0 => {
                let ob = p[at];
                at += 1;
                let k = rd(&mut at);
                let v = rd(&mut at);
                match ob {
                    0 => q.prefs.set_ascii_string(&k, &v),
                    1 => c.prefs.set_ascii_string(&k, &v),
                    _ => m.prefs.set_ascii_string(&k, &v),
                }
            }
            1 => {
                let s = rd(&mut at);
                q.set_map_selected(&s, p[at] != 0);
                at += 1;
            }
            2 => {
                let s = rd(&mut at);
                o.push(q.is_map_selected(&s) as i32);
            }
            3 => {
                let ob = p[at];
                at += 1;
                let s = rd(&mut at);
                let port = p[at] as u16;
                at += 1;
                if ob == 0 { q.set_last_ladder(&s, port) } else { c.set_last_ladder(&s, port) }
            }
            4 => {
                let ob = p[at];
                at += 1;
                let r = if ob == 0 { q.get_last_ladder_addr() } else { c.get_last_ladder_addr() };
                put(&mut o, &r);
            }
            5 => {
                let ob = p[at];
                at += 1;
                o.push(if ob == 0 { q.get_last_ladder_port() } else { c.get_last_ladder_port() } as i32);
            }
            6 => {
                let (w, v) = (p[at], p[at + 1]);
                at += 2;
                match w {
                    0 => q.set_max_disconnects(v),
                    1 => q.set_max_points(v),
                    2 => q.set_min_points(v),
                    3 => q.set_wait_time(v),
                    4 => q.set_num_players(v),
                    5 => q.set_max_ping(v),
                    6 => q.set_color(v),
                    _ => q.set_side(v),
                }
            }
            7 => {
                let w = p[at];
                at += 1;
                o.push(match w {
                    0 => q.get_max_disconnects(),
                    1 => q.get_max_points(),
                    2 => q.get_min_points(),
                    3 => q.get_wait_time(),
                    4 => q.get_num_players(),
                    5 => q.get_max_ping(),
                    6 => q.get_color(),
                    _ => q.get_side(),
                });
            }
            8 => {
                c.set_preferred_color(p[at]);
                at += 1;
            }
            9 => o.push(c.get_preferred_color(&host)),
            10 => {
                c.set_chat_size_slider(p[at]);
                at += 1;
            }
            11 => o.push(c.get_chat_size_slider()),
            12 => {
                c.set_preferred_faction(p[at]);
                at += 1;
            }
            13 => o.push(c.get_preferred_faction(&host)),
            14 => {
                let (w, v) = (p[at], p[at + 1] != 0);
                at += 2;
                match w {
                    0 => c.set_uses_system_map_dir(v),
                    1 => c.set_uses_long_game_list(v),
                    2 => c.set_allows_observer(v),
                    3 => c.set_disallow_asian_text(v),
                    4 => c.set_disallow_non_asian_text(v),
                    5 => c.set_superweapon_restricted(v),
                    6 => c.set_factions_limited(v),
                    _ => c.set_use_stats(v),
                }
            }
            15 => {
                let w = p[at];
                at += 1;
                o.push(match w {
                    0 => c.uses_system_map_dir(),
                    1 => c.uses_long_game_list(),
                    2 => c.allows_observers(),
                    3 => c.get_disallow_asian_text(&host),
                    4 => c.get_disallow_non_asian_text(),
                    5 => c.get_superweapon_restricted(),
                    6 => c.get_factions_limited(),
                    _ => c.get_use_stats(),
                } as i32);
            }
            16 => {
                let s = rd(&mut at);
                c.set_preferred_map(&s);
            }
            17 => {
                let r = c.get_preferred_map(&host);
                put(&mut o, &r);
            }
            18 => {
                c.set_starting_cash(p[at] as u32);
                at += 1;
            }
            19 => o.push(c.get_starting_cash(&host) as i32),
            20 => {
                m.set_locale(p[at]);
                at += 1;
            }
            21 => o.push(m.get_locale()),
            22 => {
                let s = rd(&mut at);
                m.set_cached_stats(&s);
            }
            23 => {
                let r = m.get_cached_stats();
                put(&mut o, &r);
            }
            24 => o.push(m.get_quick_match_res_locked() as i32),
            25 => o.push(m.get_max_messages_per_update()),
            _ => {
                dump_map(&q.prefs, &mut o);
                dump_map(&c.prefs, &mut o);
                dump_map(&m.prefs, &mut o);
            }
        }
    }
    o
}

fn cstr(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s).unwrap()
}

#[test]
fn online_preferences_programs() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("up2_{}/", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dirs = format!("{}/", dir.to_str().unwrap().trim_end_matches('/'));
    let cdir = cstr(&dirs);
    let mut g = Gen::new(0x215_0F2E);
    let mut c = Check::new("online preference accessors");
    let mut buf = vec![0i32; 200_000];
    for case in 0..30_000 {
        let mut head = Vec::new();
        let t = header(&mut g, &mut head);
        let mut body = Vec::new();
        ops(&mut g, &mut body);
        let rust = run_rust(&body, &t);
        let mut prog = head;
        prog.extend_from_slice(&body);
        let n = unsafe { ref_up2_run(prog.as_ptr(), prog.len() as i32, cdir.as_ptr(), buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n >= 0, "the output buffer is too small");
        let cpp = &buf[..n as usize];
        let first = rust.iter().zip(cpp.iter()).position(|(a, b)| a != b);
        c.eq(rust.len() == cpp.len() && first.is_none(), true, || format!("case {case}: first differing word {first:?} (lengths {} and {})", rust.len(), cpp.len()));
    }
    finish(c);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn online_preferences_file_names() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("up2f_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dirs = format!("{}/", dir.to_str().unwrap());
    let cdir = cstr(&dirs);
    let mut c = Check::new("online preference file names");
    let mut buf = vec![0i32; 1000];
    for profile in (0..=20).chain([-1, 99999, 2147483647]) {
        for kind in 0..3 {
            let name = match kind {
                0 => QuickMatchPreferences::filename(profile),
                1 => CustomMatchPreferences::filename(profile),
                _ => GameSpyMiscPreferences::filename(profile),
            };
            let path = format!("{}{}", dirs, String::from_utf8(name).unwrap());
            std::fs::write(&path, "Marker=yes\n").unwrap();
            let mut prog = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            // header: numColors, templates 0, gameInfo 3, generals 0, lang 0, money 0, default map "", masks 0 0
            prog.truncate(0);
            prog.extend_from_slice(&[0, 0, 3, 0, 0, 0, 0, 0, 0, 27, kind, profile]);
            let n = unsafe { ref_up2_run(prog.as_ptr(), prog.len() as i32, cdir.as_ptr(), buf.as_mut_ptr(), buf.len() as i32) };
            assert!(n >= 0);
            // [map size, marker string (len + bytes) twice]: the one marker file named by the draft is found
            c.eq(buf[0], 1, || format!("kind {kind} profile {profile}: the constructor did not load the file the draft names"));
            std::fs::remove_file(&path).unwrap();
        }
    }
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
    let _ = std::fs::remove_dir_all(&dir);
}
