#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/LanLobbyMenu.cpp (LANPreferences::getNumRemoteIPs, getRemoteIPEntry)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::lan_lobby_menu` against upstream's (#215).
//!
//! # Method
//!
//! The two functions, `QuotedPrintableToUnicodeString` and the base class `UserPreferences` (the verified wrapper's cut, reused) are cut
//! verbatim (build.rs, `drafts_guilogic4`) into `differential/wrap_draft_lanprefs.cpp`, built on upstream's own UnicodeString.cpp and
//! AsciiString.cpp (`-fshort-wchar`). 20,000 random preference maps: `NumRemoteIPs` absent or a number as text (a sign, blanks, 1 to 9 digits,
//! trailing junk, or none), and up to five `RemoteIP<n>` entries `ip:name`, with leading colons before the ip, colons inside the name, an
//! empty name, names that are quoted-printable (`_41`, even and odd numbers of bytes) or plain, and entries with other keys around them.
//! Both functions are called for every `RemoteIP` index 0..=6 (some absent) and the results compared (a number; the 16-bit units).
//!
//! Excluded, and why: entries whose first token is not followed by a `:` (no colon at all, or the only colons leading) and the empty entry.
//! Upstream's `asciientry.set(asciientry.str() + 1)` then reads one past the end of the string (and `nextToken` has cleared it, for an entry
//! of colons only): undefined behaviour, which the draft documents as an empty name. A `NumRemoteIPs` of more than 9 digits (`atoi` overflow).
//! Names that decode to an ODD number of bytes: upstream's `QuotedPrintableToUnicodeString` decodes into a `static unsigned short dest[1024]` it never
//! clears, so the last 16-bit unit's high byte is whatever an earlier call left there (the verified decoder's doc records this: "leftovers are not
//! modelled"); the output then depends on the history of calls, not on the entry.
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches: both are short and the quoted-printable decoder is verified elsewhere.
//!
//! # Results
//!
//! 2026-11-20: 20,000 maps, 20,000 `getNumRemoteIPs` and 140,000 `getRemoteIPEntry` comparisons, **0 mismatches** (prediction met, after two corrections to the test's
//! own inputs: the first run had 17,104 differences, all in names that decode to an odd number of bytes, where upstream's static buffer leaves the last unit's high
//! byte from an earlier call; those inputs are now excluded, as the module docs say). Planted bugs, 6 of 6 caught (mismatches): 15883 84,839 (of 140,000 entries);
//! 15884 3,957 (of 20,000 numbers); 15885 32,058; 15886 26,733; 15887 59,361; 15888 59,361.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 15883 `getRemoteIPEntry(i)` looks up `RemoteIP<i+1>`; 15884 an absent `NumRemoteIPs` is 1; 15885 the ip runs to the last colon;
//! 15886 leading colons are not skipped; 15887 the name's closing parenthesis is missing; 15888 the name is not decoded (widened as it is).

mod support;

use rts_lockstep_engine::client::user_preferences::UserPreferences;
use rts_lockstep_engine::drafts::sweep_gui_logic::lan_lobby_menu as lan;
use std::ffi::CString;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lan_prefs(n: i32, keys: *const *const std::os::raw::c_char, vals: *const *const std::os::raw::c_char, op: i32, i: i32, out: *mut u16, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn number(g: &mut Gen) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..g.below(3) {
        v.push([b' ', b'\t'][g.below(2) as usize]);
    }
    match g.below(4) {
        0 => v.push(b'-'),
        1 => v.push(b'+'),
        _ => {}
    }
    for _ in 0..g.below(10) {
        v.push(b'0' + g.below(10) as u8);
    }
    if g.below(4) == 0 {
        v.extend_from_slice(b"abc");
    }
    v
}

fn name(g: &mut Gen) -> Vec<u8> {
    let n = g.below(8);
    let mut v = Vec::new();
    let mut pieces = 0;
    for _ in 0..n {
        pieces += 1;
        match g.below(5) {
            0 | 1 => v.extend_from_slice(format!("_{:02X}", g.below(256)).as_bytes()),
            2 => v.extend_from_slice(format!("_{:02x}", g.below(256)).as_bytes()),
            3 => v.push(b':'),
            _ => v.push(b'a' + g.below(26) as u8),
        }
    }
    if pieces % 2 == 1 {
        v.push(b'a' + g.below(26) as u8); // an even number of decoded bytes (see the module docs)
    }
    if g.below(10) == 0 {
        v.push(b'_'); // a name ending in the magic character (decodes to no byte)
    }
    v
}

fn entry(g: &mut Gen) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..[0, 0, 0, 1, 2][g.below(5) as usize] {
        v.push(b':');
    }
    for _ in 0..1 + g.below(12) {
        v.push([b'0', b'1', b'9', b'.', b'a', b'z', b' '][g.below(7) as usize]);
    }
    v.push(b':');
    v.extend(name(g));
    v
}

#[test]
fn lan_preferences() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_1A17);
    let mut nums = Check::new("LANPreferences::getNumRemoteIPs");
    let mut ents = Check::new("LANPreferences::getRemoteIPEntry");
    let mut out = vec![0u16; 4096];
    for _ in 0..20_000 {
        let mut pairs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        if g.below(5) != 0 {
            pairs.push((b"NumRemoteIPs".to_vec(), number(&mut g)));
        }
        for i in 0..5 {
            if g.below(3) != 0 {
                pairs.push((format!("RemoteIP{i}").into_bytes(), entry(&mut g)));
            }
        }
        for k in 0..g.below(3) {
            pairs.push((format!("Other{k}").into_bytes(), b"x:y".to_vec()));
        }
        let mut prefs = UserPreferences::default();
        for (k, v) in &pairs {
            prefs.map.insert(k.clone(), v.clone());
        }
        let ckeys: Vec<CString> = pairs.iter().map(|(k, _)| CString::new(k.clone()).expect("no NUL")).collect();
        let cvals: Vec<CString> = pairs.iter().map(|(_, v)| CString::new(v.clone()).expect("no NUL")).collect();
        let kp: Vec<*const std::os::raw::c_char> = ckeys.iter().map(|c| c.as_ptr()).collect();
        let vp: Vec<*const std::os::raw::c_char> = cvals.iter().map(|c| c.as_ptr()).collect();
        let cpp_n = unsafe { ref_lan_prefs(pairs.len() as i32, kp.as_ptr(), vp.as_ptr(), 0, 0, out.as_mut_ptr(), out.len() as i32) };
        nums.eq(lan::get_num_remote_ips(&mut prefs), cpp_n, || format!("map {pairs:?}"));
        for i in 0..7 {
            let n = unsafe { ref_lan_prefs(pairs.len() as i32, kp.as_ptr(), vp.as_ptr(), 1, i, out.as_mut_ptr(), out.len() as i32) };
            assert!(n >= 0, "buffer");
            let cpp: Vec<u16> = out[..n as usize].to_vec();
            ents.eq(lan::get_remote_ip_entry(&prefs, i), cpp, || format!("index {i} map {pairs:?}"));
        }
    }
    if planted() {
        nums.report();
        ents.report();
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    nums.finish();
    ents.finish();
}
