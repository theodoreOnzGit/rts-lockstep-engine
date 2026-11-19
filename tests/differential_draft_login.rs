#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLLoginMenu.cpp (obfuscate, GameSpyLoginPreferences,
//     isNickOkay, isAgeOkay)
//   GeneralsMD/Code/GameEngine/Source/Common/System/QuotedPrintable.cpp (the two ASCII functions)
//   GeneralsMD/Code/GameEngine/Source/Common/UserPreferences.cpp (load, as the base class of the login preferences)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::ui::wol_login_menu`'s pure logic against upstream's (#215).
//!
//! # Method
//!
//! `obfuscate`, `class GameSpyLoginPreferences`, `isNickOkay` and `isAgeOkay` are cut verbatim by build.rs into
//! `differential/wrap_draft_login.cpp` (which lists the cuts, the one edit to `obfuscate`, and the stand-ins: no file is touched,
//! the file's text is read with `fmemopen` and written with `open_memstream`; `GetDateFormat` gives the date the case sets), built
//! on the real AsciiString, the real `UserPreferences::load` and the real `QuotedPrintable.cpp` functions. Five tests, each from a
//! fixed seed, each comparing whole outputs:
//!
//! - `obfuscate_matches` (60,000 strings of 0 to 40 bytes: mostly the key's own bytes `1337Munkee` and letters, some bytes of
//!   0x80 and up, and a NUL in some: the C string ends there);
//! - `quoted_printable_matches` (the verified `crate::common::map_cache` functions the draft calls, 60,000 strings each way);
//! - `nick_okay_matches` (60,000 nicks of 0 to 6 UTF-16 units: legal IRC characters, other ASCII, any unit but 0);
//! - `age_okay_matches` (80,000: a system date and a birth date about 13 years earlier, the month and day as the user typed
//!   them: digits, one digit, `00`, 13, 32, a sign, white space, letters, and up to 18 digits; the year of 4 characters, or not),
//!   comparing the answer and the rewritten month and day;
//! - `login_preferences_match` (20,000 files of 0 to 10 lines `pass_`, `date_`, `nick_` and decoy keys, then 1 to 12 operations:
//!   the getters, `addLogin`, `forgetLogin`, `getEmails`, `write`), comparing load's answer and every operation's answer.
//!
//! Excluded: a value of more than 18 digits for `atoi` (glibc's `strtol` saturates at 64 bits; the draft wraps at 32 as the
//! game's CRT does); a NUL in a preference value (an `AsciiString` cannot hold one); `write()`'s side effect of inserting empty
//! `lastEmail`, `lastName` and `useProfiles` entries into C++'s map (the written text is compared); wide characters of 0 in a nick
//! (they end a string). `obfuscate`'s `*c = *c++ ^ *c2++` is undefined in C++; both sides take the left side's address before the
//! increment (the draft's documented reading), which needs the one mechanical edit in build.rs because g++ writes the next byte.
//! The preference file's `fname` is the full path here (upstream's `getPath_UserData() + fname`).
//!
//! # Predictions (written before the first run, 2026-11-18)
//!
//! Not 0. The draft's own `=XX` quoted-printable differs from upstream's `_XX` (read upstream's QuotedPrintable.cpp before the
//! run): expect every `pass_`, `date_` and `write` case with an escape to mismatch until the draft calls the verified functions;
//! `isAgeOkay`'s `{:02}` for a negative number ("-5": C's `%02.2d` gives `-05`, Rust `-5`) and its `atoi` overflow to 0; `obfuscate`
//! keeping bytes after a NUL. Everything else 0.
//!
//! # Results
//!
//! 2026-11-18. **Before any fix** (the draft as committed at acc964a): `obfuscate` 7,150 of
//! 60,000 mismatches (every case with a NUL: the draft kept the bytes after it, and the NUL, where the C string ends); `isAgeOkay`
//! 4,033 of 80,000 (a negative month or day, `-5` for `-05`; a value of 10 or more digits, 0 for the wrapped `atoi`; Unicode white
//! space); `GameSpyLoginPreferences` 8,488 of 20,000 (the draft's own quoted-printable used `=XX` where upstream's is `_XX`, so every
//! `pass_` and `date_` with an escape decoded wrongly and every `write()` of a password or date encoded wrongly); `isNickOkay` 0.
//! The two verified quoted-printable functions: 0 of 60,000 each way. **Prediction: met** (it named all four; it did not expect
//! `obfuscate`'s NUL case to be the whole of that function's misses, and did not name Unicode white space).
//! **Fixes:** `obfuscate` ends at the first NUL; the draft's `=XX` quoted-printable replaced by calls to the verified
//! `map_cache::quoted_printable_to_ascii_string` and `ascii_string_to_quoted_printable` (a duplicate removed); `atoi` wraps at 32 bits
//! and takes C's white space only; `%02.2d` written as `-05`; the subtractions in `isAgeOkay` wrap.
//! **After:** 0 mismatches in all five tests (`obfuscate` 60,000, `isNickOkay` 60,000, `isAgeOkay` 80,000, the preferences
//! 20,000 files, the quoted-printable pair 60,000 each), max ULP n/a. Planted bugs, mismatching cases (of 60,000 / 60,000 / 80,000 /
//! 20,000 by the unit): 12681 27,853 (and 1,229 in the preferences); 12682 22,045 (2,664); 12683 22,045 (2,664); 12684 31,292 (2,411);
//! 12685 18,118; 12686 8,591; 12687 6,543; 12688 354; 12690 760; 12691 3,420; 12692 265; 12693 158; 12694 957; 12695 429; 12696
//! 7,561; 12697 22,737; 12698 6,880; 12699 1,380; 12700 5,488; 12701 7,586; 12702 7,320; 12703 916; 12704 228; 12705 1,549; 12706
//! 5,884; 12708 2,732. **26 of 26 caught.** (12682 and 12683 both change only the byte at key index 9, hence the same counts.)
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose): 12681 `obfuscate` does not advance the key when the byte
//! equals the key byte; 12682 the key is 9 long; 12683 `1337Munkey`; 12684 a byte equal to the key byte is xored too (0); 12685
//! `isNickOkay` tests the first character; 12686 an empty nick is not okay; 12687 `-` alone is okay; 12688 `{` is not legal; 12690
//! `isAgeOkay` needs more than 14 years; 12691 12 years is not too young; 12692 the same month passes; 12693 the same day does not
//! pass; 12694 month 12 is refused; 12695 day 31 is refused; 12696 the year length is not checked; 12697 the month is not padded to two
//! digits; 12698 `load` does not de-obfuscate the password; 12699 `load` does not decode the date; 12700 `load` keeps empty nick
//! tokens; 12701 `write` does not obfuscate the password; 12702 `write` ends the nick list without a comma; 12703 a date of 8
//! or more characters is accepted; 12704 `addLogin` repeats a nick it has; 12705 `forgetLogin` keeps the date; 12706 `getEmails` lists
//! the password map; 12708 `write` does not quote the date.

mod support;

use rts_lockstep_engine::common::map_cache::{ascii_string_to_quoted_printable, quoted_printable_to_ascii_string};
use rts_lockstep_engine::drafts::ui::wol_login_menu::{is_age_okay, is_nick_okay, obfuscate, GameSpyLoginPreferences};
use std::collections::BTreeMap;
use std::os::raw::c_char;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_login_obfuscate(input: *const c_char, out: *mut c_char, cap: i32) -> i32;
    fn ref_login_qp(which: i32, input: *const c_char, out: *mut c_char, cap: i32) -> i32;
    fn ref_login_nick(units: *const u16, n: i32) -> i32;
    #[allow(clippy::too_many_arguments)]
    fn ref_login_age(month: *const c_char, day: *const c_char, year: *const c_char, sy: i32, sm: i32, sd: i32, omonth: *mut c_char, oday: *mut c_char, cap: i32) -> i32;
    fn ref_login_prefs(input: *const u8, len: i32, out: *mut u8, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(checks: Vec<Check>) {
    for c in checks {
        if planted() {
            c.report();
        } else {
            c.finish();
        }
    }
    if planted() {
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
}

fn cstr(b: &[u8]) -> Vec<u8> {
    let mut v: Vec<u8> = b.iter().copied().take_while(|&c| c != 0).collect();
    v.push(0);
    v
}

fn pick<'a, T>(g: &mut Gen, v: &'a [T]) -> &'a T {
    &v[g.below(v.len() as u32) as usize]
}

fn from_c(buf: &[u8], n: i32) -> Vec<u8> {
    buf[..n as usize].to_vec()
}

const KEY: &[u8] = b"1337Munkee";

fn obf_string(g: &mut Gen) -> Vec<u8> {
    let n = if g.below(6) == 0 { 10 + g.below(20) } else { g.below(14) } as usize;
    let style = g.below(4);
    let mut v: Vec<u8> = (0..n)
        .map(|i| match (style, g.below(10)) {
            (0, _) => KEY[i % KEY.len()],                                            // the key itself (every byte equal)
            (1, 0..=4) => KEY[g.below(KEY.len() as u32) as usize],                  // mostly key bytes
            (_, 0) => 0x80 + g.below(128) as u8,
            (_, 1) => b' ' + g.below(95) as u8,
            _ => KEY[g.below(KEY.len() as u32) as usize] ^ (g.below(2) as u8) ^ if g.below(3) == 0 { 0 } else { 1 },
        })
        .collect();
    if g.below(8) == 0 && !v.is_empty() {
        let at = g.below(v.len() as u32) as usize;
        v[at] = 0;
    }
    v
}

#[test]
fn obfuscate_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0B0F);
    let mut c = Check::new("obfuscate");
    let mut buf = vec![0 as c_char; 512];
    for _ in 0..60_000 {
        let s = obf_string(&mut g);
        let ci = cstr(&s);
        let n = unsafe { ref_login_obfuscate(ci.as_ptr() as *const c_char, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n >= 0);
        let cpp: Vec<u8> = buf[..n as usize].iter().map(|&x| x as u8).collect();
        c.eq(obfuscate(&s), cpp, || format!("input {s:?}"));
    }
    finish(vec![c]);
}

#[test]
fn quoted_printable_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0A91);
    let mut to = Check::new("AsciiStringToQuotedPrintable (verified fn)");
    let mut from = Check::new("QuotedPrintableToAsciiString (verified fn)");
    let mut buf = vec![0 as c_char; 4096];
    let mut run = |which: i32, c: &mut Check, s: Vec<u8>, rust: Vec<u8>| {
        let ci = cstr(&s);
        let n = unsafe { ref_login_qp(which, ci.as_ptr() as *const c_char, buf.as_mut_ptr() as *mut c_char, 4096) };
        assert!(n >= 0);
        let cpp: Vec<u8> = from_c(unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, 4096) }, n);
        c.eq(rust, cpp, || format!("input {s:?}"));
    };
    for _ in 0..60_000 {
        let n = if g.below(10) == 0 { 300 + g.below(500) } else { g.below(30) } as usize;
        let s: Vec<u8> = (0..n).map(|_| if g.below(4) == 0 { g.below(256) as u8 } else { b' ' + g.below(95) as u8 }).collect();
        let s: Vec<u8> = s.into_iter().take_while(|&c| c != 0).collect();
        let r = ascii_string_to_quoted_printable(&s);
        run(0, &mut to, s, r);
        let n = if g.below(10) == 0 { 300 + g.below(1000) } else { g.below(30) } as usize;
        let alpha = b"_____0123456789abcdefABCDEFgGzZ -.";
        let s: Vec<u8> = (0..n).map(|_| alpha[g.below(alpha.len() as u32) as usize]).collect();
        let r = quoted_printable_to_ascii_string(&s);
        run(1, &mut from, s, r);
    }
    finish(vec![to, from]);
}

#[test]
fn nick_okay_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_01C4);
    let mut c = Check::new("isNickOkay");
    let legal: Vec<u16> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789[]`_^{|}-".encode_utf16().collect();
    for _ in 0..60_000 {
        let n = g.below(7) as usize;
        let nick: Vec<u16> = (0..n)
            .map(|_| match g.below(6) {
                0 => *pick(&mut g, &legal),
                1 | 2 => *pick(&mut g, &legal),
                3 => 1 + g.below(127) as u16,   // any other ASCII, the neighbours of the legal ones included
                4 => ['@', '\\', '~', ' ', '(', ')', '.', '<'][g.below(8) as usize] as u16,
                _ => 1 + g.below(65535) as u16,
            })
            .collect();
        let nick = if g.below(8) == 0 && !nick.is_empty() { vec![b'-' as u16] } else { nick };
        let cpp = unsafe { ref_login_nick(nick.as_ptr(), nick.len() as i32) } != 0;
        c.eq(is_nick_okay(&nick), cpp, || format!("nick {nick:?}"));
    }
    finish(vec![c]);
}

/// A month or a day as typed.
fn typed_number(g: &mut Gen, lo: i32, hi: i32, exact: Option<i32>) -> Vec<u8> {
    match g.below(14) {
        0 => b"0".to_vec(),
        1 => b"00".to_vec(),
        2 => b"13".to_vec(),
        3 => b"32".to_vec(),
        4 => format!("-{}", g.below(40)).into_bytes(),
        5 => format!("+{}", g.below(40)).into_bytes(),
        6 => format!(" {}", g.below(40)).into_bytes(),
        7 => format!("{}x", g.below(40)).into_bytes(),
        8 => b"abc".to_vec(),
        9 => {
            let digits = 1 + g.below(18) as usize;
            let mut s = String::new();
            for _ in 0..digits {
                s.push((b'0' + g.below(10) as u8) as char);
            }
            s.into_bytes()
        }
        10 => {
            let t = if g.below(2) == 0 { exact.unwrap_or(lo) } else { lo };
            format!("{t}").into_bytes()
        }
        11 => {
            let t = exact.unwrap_or(hi);
            format!("{t:02}").into_bytes()
        }
        _ => format!("{}", lo + g.below((hi - lo + 1) as u32) as i32).into_bytes(),
    }
}

#[test]
fn age_okay_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_A9E0);
    let mut c = Check::new("isAgeOkay");
    let mut om = vec![0 as c_char; 64];
    let mut od = vec![0 as c_char; 64];
    for _ in 0..80_000 {
        let sy = 1990 + g.below(120) as i32;
        let sm = 1 + g.below(12) as i32;
        let sd = 1 + g.below(31) as i32;
        let age = [11, 12, 12, 13, 13, 13, 14, 14, 15, 5, 40][g.below(11) as usize];
        let year: Vec<u8> = match g.below(14) {
            0 => b"".to_vec(),
            1 => format!("{}", sy - age).chars().skip(1).collect::<String>().into_bytes(),
            2 => format!("{}0", sy - age).into_bytes(),
            3 => format!(" {}", (sy - age) % 1000).into_bytes(),
            4 => b"abcd".to_vec(),
            5 => format!("{:04}", g.below(10000)).into_bytes(),
            _ => format!("{}", sy - age).into_bytes(),
        };
        let month = if g.below(20) == 0 { Vec::new() } else { typed_number(&mut g, 1, 12, Some(sm)) };
        let day = if g.below(20) == 0 { Vec::new() } else { typed_number(&mut g, 1, 31, Some(sd)) };
        let (cm, cd, cy) = (cstr(&month), cstr(&day), cstr(&year));
        let r = unsafe { ref_login_age(cm.as_ptr() as *const c_char, cd.as_ptr() as *const c_char, cy.as_ptr() as *const c_char, sy, sm, sd, om.as_mut_ptr(), od.as_mut_ptr(), 64) } != 0;
        let s = |b: &[c_char]| -> Vec<u8> { b.iter().map(|&x| x as u8).take_while(|&x| x != 0).collect() };
        let (mut rm, mut rd) = (month.clone(), day.clone());
        let rr = is_age_okay(&mut rm, &mut rd, &year, (sy, sm, sd));
        c.eq((rr, rm, rd), (r, s(&om), s(&od)), || format!("month {:?} day {:?} year {:?} today {sy}-{sm}-{sd}", String::from_utf8_lossy(&month), String::from_utf8_lossy(&day), String::from_utf8_lossy(&year)));
    }
    finish(vec![c]);
}

// ---- the preferences ----

fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    v.extend_from_slice(&(s.len() as u32).to_le_bytes());
    v.extend_from_slice(s);
}

struct Rd<'a>(&'a [u8], usize);

fn text(g: &mut Gen, alphabet: &[u8], lo: u32, hi: u32) -> Vec<u8> {
    let n = lo + g.below(hi - lo + 1);
    (0..n).map(|_| alphabet[g.below(alphabet.len() as u32) as usize]).collect()
}

fn run_prefs(g: &mut Gen) -> (Vec<u8>, Vec<u8>) {
    let email_alpha = b"abcxyzABC019@._-";
    let pool: Vec<Vec<u8>> = (0..1 + g.below(5)).map(|_| { let lo = if g.below(8) == 0 { 0 } else { 1 }; text(g, email_alpha, lo, 10) }).collect();
    let nick_alpha = b"abcXYZ019_[]-";
    let mut entries: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
    for _ in 0..g.below(11) {
        let email = pick(g, &pool).clone();
        let (k, v): (Vec<u8>, Vec<u8>) = match g.below(9) {
            0..=2 => {
                // pass_: a quoted-printable value (underscores and hex digits)
                let raw = text(g, b"abc_0123456789ABCDEFxyz__", 1, 30);
                ([b"pass_".as_slice(), &email].concat(), raw)
            }
            3..=4 => {
                let v = if g.below(2) == 0 { text(g, b"0123456789", 8, 8) } else { text(g, b"_0123456789ABCabc", 1, 20) };
                ([b"date_".as_slice(), &email].concat(), v)
            }
            5..=6 => {
                let mut v = Vec::new();
                for _ in 0..g.below(4) {
                    if g.below(3) == 0 {
                        v.push(b',');
                    }
                    v.extend(text(g, nick_alpha, 1, 8));
                    v.push(b',');
                }
                if v.is_empty() {
                    v = text(g, nick_alpha, 1, 6);
                }
                if g.below(4) == 0 {
                    v.insert(0, b',');
                }
                ([b"nick_".as_slice(), &email].concat(), v)
            }
            7 => (pick(g, &[b"lastEmail".to_vec(), b"lastName".to_vec(), b"useProfiles".to_vec()]).clone(), text(g, b"abc019@.", 1, 10)),
            _ => ([pick(g, &[b"PASS_".to_vec(), b"Pass_".to_vec(), b"nick".to_vec(), b"date".to_vec(), b"xpass_".to_vec()]).as_slice(), &email].concat(), text(g, b"abc_019", 1, 8)),
        };
        entries.insert(k, v);
    }
    let has_file = g.below(8) != 0;
    let fname: Vec<u8> = if g.below(12) == 0 { Vec::new() } else { b"GameSpyLogin.ini".to_vec() };
    let mut file = Vec::new();
    for (k, v) in &entries {
        file.extend_from_slice(k);
        file.extend_from_slice(b" = ");
        file.extend_from_slice(v);
        file.push(b'\n');
    }
    let mut input = vec![u8::from(has_file)];
    put_str(&mut input, &fname);
    put_str(&mut input, &file);

    let mut p = GameSpyLoginPreferences::new();
    let mut out = vec![u8::from(p.load(&fname, if has_file { Some(entries.clone()) } else { None }))];

    let fresh = |g: &mut Gen| text(g, email_alpha, 0, 10);
    let nicks_pool: Vec<Vec<u8>> = (0..3).map(|_| text(g, nick_alpha, 1, 6)).collect();
    let nops = 1 + g.below(12);
    input.extend_from_slice(&nops.to_le_bytes());
    for _ in 0..nops {
        let email = if g.below(4) == 0 { fresh(g) } else { pick(g, &pool).clone() };
        match g.below(7) {
            0 => {
                input.push(0);
                put_str(&mut input, &email);
                put_str(&mut out, &p.get_password_for_email(&email));
            }
            1 => {
                input.push(1);
                put_str(&mut input, &email);
                let (m0, d0, y0) = (text(g, b"mdy", 0, 3), text(g, b"mdy", 0, 3), text(g, b"mdy", 0, 5));
                put_str(&mut input, &m0);
                put_str(&mut input, &d0);
                put_str(&mut input, &y0);
                let (mut m, mut d, mut y) = (m0, d0, y0);
                let full = p.get_date_for_email(&email, &mut m, &mut d, &mut y);
                for s in [&full, &m, &d, &y] {
                    put_str(&mut out, s);
                }
            }
            2 => {
                input.push(2);
                put_str(&mut input, &email);
                let l = p.get_nicks_for_email(&email);
                out.extend_from_slice(&(l.len() as u32).to_le_bytes());
                for n in &l {
                    put_str(&mut out, n);
                }
            }
            3 => {
                let nick = pick(g, &nicks_pool).clone();
                let pass = text(g, b"abcXYZ019 !\"#$%&'()*+-./:;<=>?@[\\]^_`{|}~\x80\xA9\xFF1337Munkee", 0, 14);
                let date = if g.below(2) == 0 { text(g, b"0123456789", 8, 8) } else { text(g, b"0123456789 /-.", 0, 12) };
                input.push(3);
                for s in [&email, &nick, &pass, &date] {
                    put_str(&mut input, s);
                }
                p.add_login(&email, &nick, &pass, &date);
            }
            4 => {
                input.push(4);
                put_str(&mut input, &email);
                p.forget_login(&email);
            }
            5 => {
                input.push(5);
                let l = p.get_emails();
                out.extend_from_slice(&(l.len() as u32).to_le_bytes());
                for n in &l {
                    put_str(&mut out, n);
                }
            }
            _ => {
                input.push(6);
                match p.write() {
                    Some(t) => {
                        out.push(1);
                        put_str(&mut out, &t);
                    }
                    None => {
                        out.push(0);
                        put_str(&mut out, b"");
                    }
                }
            }
        }
    }
    let _ = Rd(&[], 0);
    (input, out)
}

#[test]
fn login_preferences_match() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_9AEF);
    let mut c = Check::new("GameSpyLoginPreferences");
    let mut buf = vec![0u8; 1 << 20];
    for _ in 0..20_000 {
        let (input, rust) = run_prefs(&mut g);
        let n = unsafe { ref_login_prefs(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n >= 0, "buffer too small");
        let cpp = buf[..n as usize].to_vec();
        c.eq(rust, cpp, || format!("input {:?}", String::from_utf8_lossy(&input)));
    }
    finish(vec![c]);
}
