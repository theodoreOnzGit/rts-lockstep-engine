#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/version.cpp (and Include/Common/version.h)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::version::Version` against `Version` (#215).
//!
//! # Method
//!
//! version.cpp from `Version *TheVersion = NULL;` to the end is cut verbatim (build.rs) into `differential/wrap_draft_version.cpp`,
//! built twice: as the shipped game is (no `_DEBUG`), and with `_DEBUG` defined after the headers (the draft's `debug_build`).
//! UnicodeString.cpp and AsciiString.cpp are upstream's own, unchanged. Outside the cut: `TheGameText->fetch` (a table the test fills
//! per case with format strings chosen to fit each label's arguments), and the C runtime's `_vsnwprintf` (a 16-bit stand-in for MSVC's
//! wide printf, for `%d`, `%c`, `%s` taking a WIDE string, and `%%`; `differential/shim_draft_text/text_prelude.h`). 100,000 cases
//! per build; each runs the constructor, optionally `setVersion` and `setShowFullVersion`, and compares `getVersionNumber`,
//! `showFullVersion` and the nine string getters (ASCII ones as bytes, Unicode ones as 16-bit units).
//!
//! The Unicode results rest on the `_vsnwprintf` stand-in, written for the same semantics as the draft's `format_wide`: the check
//! shows the draft and upstream's own code AGREE on which argument goes where, not that MSVC's printf behaves as both assume.
//! `_INTERNAL` (" Internal") is not modelled by the draft and not built here. `%c` with a build-user byte of 0x80 or more is not
//! compared (a negative `char` passed through a variadic call; the user is ASCII whenever the local build number, the only user
//! of `%c`, is nonzero).
//!
//! # Prediction (written before the first run, 2026-11-18)
//!
//! Misses predicted: (1) the draft's `Version::new()` leaves `m_showFullVersion` FALSE when `debug_build` is set (upstream TRUE).
//! (2) `translate`: upstream widens a `char` (signed) to a 16-bit `WideChar`, so a byte of 0x80 or more becomes 0xFF80 to 0xFFFF; the
//! draft zero-extends. Everything else: 0 mismatches.
//!
//! # Results
//!
//! 2026-11-18: 100,000 cases for each build, **0 mismatches** after three draft fixes. First run: 84,929 (release) and 84,974 (`_DEBUG`) of 100,000 differed,
//! in the Unicode getters (full version, build time, location, user, version) and, for `_DEBUG`, the ASCII version. **Prediction misses:** (1) `translate` widens a
//! signed `char` (0xFF80 to 0xFFFF for bytes of 0x80 or more), the draft zero-extended: predicted, fixed. (2) the `%c` of `Version:Format4` and the `Char` arguments:
//! not predicted. `_vsnwprintf` takes each argument as an `int` whatever the conversion is for, so the draft's `FormatArg` typing printed numbers where upstream
//! prints characters, and a `%c` of 0 ends the string (`set(buf)` takes a C string); `format_wide` now reads `%d` and `%c` as the next `int` and cuts at the first 0,
//! and the debug ASCII version cuts at a NUL the same way. (3) predicted and fixed: `Version::new()` did not set `m_showFullVersion` for a debug build. Planted bugs,
//! 12 of 12 caught (of 100,000 per build): 11490 73,535; 11491 100,000 (release only); 11492 35,250 (debug only); 11493 99,568; 11494 99,751 (release);
//! 11495 89,812; 11496 85,302; 11497 100,000 (release); 11498 94,678; 11500 9,884; 11501 100,000 (debug); 11502 33,232 (debug). (11499 is unused.)
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 11490 `<< 8`; 11491 the release `getAsciiVersion` with the build number; 11492 the user's characters swapped;
//! 11493 the Format3/Format4 choice inverted; 11494 `Version:Format3` in the release `getUnicodeVersion`; 11495 build time before
//! date; 11496 the two `%s` arguments swapped; 11497 `" Debug"` also in the release full version; 11498 the machine from the user;
//! 11500 the constructor's user and location swapped; 11501 no `" Debug"` in the debug `getUnicodeVersion`; 11502 `m_showFullVersion`
//! FALSE for a debug build.

mod support;

use rts_lockstep_engine::drafts::logic_common::version::Version;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_version_text(label: *const std::os::raw::c_char, units: *const u16);
    fn ref_version_case(use_default: i32, major: i32, minor: i32, build: i32, local: i32, user: *const std::os::raw::c_char, loc: *const std::os::raw::c_char, time: *const std::os::raw::c_char, date: *const std::os::raw::c_char, show: i32, out: *mut u16) -> i32;
    fn ref_version_dbg_text(label: *const std::os::raw::c_char, units: *const u16);
    fn ref_version_dbg_case(use_default: i32, major: i32, minor: i32, build: i32, local: i32, user: *const std::os::raw::c_char, loc: *const std::os::raw::c_char, time: *const std::os::raw::c_char, date: *const std::os::raw::c_char, show: i32, out: *mut u16) -> i32;
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

fn int(g: &mut Gen) -> i32 {
    match g.below(8) {
        0 => 0,
        1 => 1,
        2 => g.below(20) as i32 - 5,
        3 => g.below(70000) as i32,
        4 => i32::MAX - g.below(3) as i32,
        5 => i32::MIN + g.below(3) as i32,
        _ => g.i32(),
    }
}

fn text(g: &mut Gen, max: u32, ascii_only: bool) -> Vec<u8> {
    let n = g.below(max + 1) as usize;
    (0..n)
        .map(|_| {
            if ascii_only || g.below(4) != 0 {
                match g.below(4) {
                    0 => b'a' + g.below(26) as u8,
                    1 => b'A' + g.below(26) as u8,
                    2 => b'0' + g.below(10) as u8,
                    _ => [b' ', b':', b'/', b'-', b'.', b'%'][g.below(6) as usize],
                }
            } else {
                1 + g.below(255) as u8
            }
        })
        .collect()
}

const F2: [&str; 4] = ["%d.%d", "Version %d.%d!", "%d / %d", "v%d%%.%d"];
const F3: [&str; 4] = ["%d.%d.%d", "Ver %d.%d build %d", "%d-%d-%d", "%d %d %d%%"];
const F4: [&str; 4] = ["%d.%d.%d.%d%c%c", "%d.%d (%d) %d %c%c", "%c%c %d.%d.%d.%d", "%d%d%d%d%c%c"];
const BT: [&str; 3] = ["Built %s at %s", "%s %s", "%s|%s|"];
const BM: [&str; 3] = ["On %s", "%s", "Machine: %s!"];
const BU: [&str; 3] = ["By %s", "%s", "User: %s?"];

fn units(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

fn run(debug: bool, cases: u32, seed: u64, name: &str) {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(seed);
    let mut c = Check::new(name);
    let mut by_getter: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
    let labels = ["Version:Format2", "Version:Format3", "Version:Format4", "Version:BuildTime", "Version:BuildMachine", "Version:BuildUser"];
    let pools: [&[&str]; 6] = [&F2, &F3, &F4, &BT, &BM, &BU];
    for case in 0..cases {
        // the fetch table
        let mut table: Vec<(&str, Vec<u16>)> = Vec::new();
        for (l, pool) in labels.iter().zip(pools.iter()) {
            let f = if g.below(20) == 0 { "" } else { pool[g.below(pool.len() as u32) as usize] };
            let u = units(f);
            let label = std::ffi::CString::new(*l).unwrap();
            unsafe {
                if debug {
                    ref_version_dbg_text(label.as_ptr(), u.as_ptr())
                } else {
                    ref_version_text(label.as_ptr(), u.as_ptr())
                }
            }
            table.push((l, u[..u.len() - 1].to_vec()));
        }
        let use_default = g.below(10) == 0;
        let (major, minor, build) = (int(&mut g), int(&mut g), int(&mut g));
        let local = if g.below(2) == 0 { 0 } else { int(&mut g) };
        let user = text(&mut g, 12, local != 0);
        let (loc, time, date) = (text(&mut g, 16, false), text(&mut g, 16, false), text(&mut g, 16, false));
        let show = [-1, 0, 1][g.below(3) as usize];

        let cs = |b: &[u8]| std::ffi::CString::new(b.to_vec()).unwrap();
        let (cu, cl, ct, cd) = (cs(&user), cs(&loc), cs(&time), cs(&date));
        let mut out = vec![0u16; 4096];
        let n = unsafe {
            let f = if debug { ref_version_dbg_case } else { ref_version_case };
            f(use_default as i32, major, minor, build, local, cu.as_ptr(), cl.as_ptr(), ct.as_ptr(), cd.as_ptr(), show, out.as_mut_ptr()) as usize
        };
        out.truncate(n);

        let mut v = Version::with_debug_build(debug);
        if !use_default {
            v.set_version(major, minor, build, local, user.clone(), loc.clone(), time.clone(), date.clone());
        }
        if show >= 0 {
            v.set_show_full_version(show != 0);
        }
        let fetch = |label: &[u8]| -> Vec<u16> { table.iter().find(|(l, _)| l.as_bytes() == label).map(|(_, u)| u.clone()).unwrap_or_default() };
        let widen = |b: Vec<u8>| -> Vec<u16> { b.into_iter().map(|x| x as u16).collect() };
        let num = v.get_version_number();
        let mut rust: Vec<u16> = vec![(num & 0xFFFF) as u16, (num >> 16) as u16, v.show_full_version() as u16];
        let mut push = |s: Vec<u16>| {
            rust.push(s.len() as u16);
            rust.extend(s);
        };
        push(widen(v.get_ascii_version()));
        push(v.get_unicode_version(&fetch));
        push(v.get_full_unicode_version(&fetch));
        push(widen(v.get_ascii_build_time()));
        push(v.get_unicode_build_time(&fetch));
        push(widen(v.get_ascii_build_location()));
        push(v.get_unicode_build_location(&fetch));
        push(widen(v.get_ascii_build_user()));
        push(v.get_unicode_build_user(&fetch));
        if rust != out {
            let names = ["number", "show", "ascii version", "unicode version", "full unicode version", "ascii build time", "unicode build time", "ascii build location", "unicode build location", "ascii build user", "unicode build user"];
            let split = |v: &[u16]| -> Vec<Vec<u16>> {
                let mut r = vec![v[..2].to_vec(), v[2..3].to_vec()];
                let mut at = 3;
                while at < v.len() {
                    let n = v[at] as usize;
                    r.push(v[at + 1..(at + 1 + n).min(v.len())].to_vec());
                    at += 1 + n;
                }
                r
            };
            let (a, b) = (split(&rust), split(&out));
            for (i, name) in names.iter().enumerate() {
                if a.get(i) != b.get(i) {
                    *by_getter.entry(*name).or_insert(0u32) += 1;
                    if by_getter[*name] <= 2 {
                        let t = |x: Option<&Vec<u16>>| x.map(|w| w.iter().map(|&u| char::from_u32(u as u32).unwrap_or('?')).collect::<String>()).unwrap_or_default();
                        println!("  {name}: rust {:?} c++ {:?}   (inputs: {major}.{minor}.{build}.{local} user {user:?} loc {loc:?} time {time:?} date {date:?}; fetch {table:?})", t(a.get(i)), t(b.get(i)));
                    }
                }
            }
        }
        c.eq(rust, out, || format!("case {case}: default {use_default} {major}.{minor}.{build}.{local} user {user:?} loc {loc:?} time {time:?} date {date:?} show {show}; fetch {table:?}"));
    }
    println!("mismatching getters: {by_getter:?}");
    finish(c);
}

#[test]
fn version_release_matches() {
    run(false, 100_000, 0x215_0E51, "Version (shipped, release)");
}

#[test]
fn version_debug_matches() {
    run(true, 100_000, 0x215_0DE6, "Version (_DEBUG)");
}
