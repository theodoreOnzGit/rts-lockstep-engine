#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/encrypt.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::encrypt::EncryptState::encrypt_string` against `EncryptString` (#215).
//!
//! # Method
//!
//! encrypt.cpp from `#define MAX_CHARS 65` to `#ifdef UNIT_TEST` is cut verbatim (build.rs) into
//! `differential/wrap_draft_encrypt.cpp`. 200,000 calls from a fixed seed run in ONE sequence on both sides, because the C++
//! keeps `Temp_Buffer` in a static: a string shorter than the one before it leaves the earlier string's bytes past its length,
//! and the draft's `EncryptState` is meant to reproduce that. Both start zeroed. Strings are 0 to 12 bytes (past 8 the length is
//! capped), made of the base alphabet, of any non-NUL byte (high bytes are negative `char`s), or of the odd and even edge bytes.
//! The 8 returned characters are compared.
//!
//! Excluded: strings with an embedded NUL (the C string ends there; the draft takes the slice as given up to its first NUL, the
//! same text).
//!
//! # Prediction (written before the first run, 2026-11-18)
//!
//! 0 mismatches over 200,000 calls (integer code; the draft already sign-extends and keeps the static buffer).
//!
//! # Results
//!
//! 2026-11-18: 200,000 calls, **0 mismatches** (prediction held). Planted bugs, 7 of 7 caught (of 200,000 calls): 11461 194,570; 11462 2; 11463 199,999;
//! 11464 200,000; 11465 195,413; 11466 196,662; 11467 107,960. (A first 11461, `length > 7` then `length = 8`, was an equivalent mutant and a first
//! 11462, the missing sign extension of `char`, too (only the low 6 bits are used); both were replaced.)
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose): 11461 length cap 7 not 8; 11462 only `Length` output characters
//! filled; 11463 `DnCnt` starts at `Length - 1`; 11464 the odd and even operations swapped; 11465 even byte shifted by 1;
//! 11466 `% 63` for `& 0x3F`; 11467 a fresh `Temp_Buffer` per call.

mod support;

use rts_lockstep_engine::drafts::logic_common::encrypt::EncryptState;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_encrypt_reset();
    fn ref_encrypt(string: *const std::os::raw::c_char, out: *mut u8);
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

fn string(g: &mut Gen) -> Vec<u8> {
    const BASE: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789./";
    let len = match g.below(4) {
        0 => g.below(13) as usize,
        1 => 4 + g.below(5) as usize,
        2 => 8,
        _ => g.below(10) as usize,
    };
    let kind = g.below(4);
    (0..len)
        .map(|_| match kind {
            0 => BASE[g.below(64) as usize],
            1 => 1 + g.below(255) as u8,
            2 => [1u8, 2, 3, 0x7F, 0x80, 0x81, 0xFE, 0xFF][g.below(8) as usize],
            _ => if g.below(2) == 0 { BASE[g.below(64) as usize] } else { 1 + g.below(255) as u8 },
        })
        .collect()
}

#[test]
fn encrypt_string_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0E1C);
    let mut st = EncryptState::new();
    unsafe { ref_encrypt_reset() };
    let mut c = Check::new("encrypt_string");
    for _ in 0..200_000 {
        let s = string(&mut g);
        let mut z = s.clone();
        z.push(0);
        let mut out = [0u8; 8];
        unsafe { ref_encrypt(z.as_ptr() as *const _, out.as_mut_ptr()) };
        let r = st.encrypt_string(&s);
        c.eq(r, out, || format!("{s:?}"));
    }
    finish(c);
}
