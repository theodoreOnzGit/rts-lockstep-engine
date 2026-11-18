#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Keyboard.cpp (getFirstKey .. setKeyStateData, isShift, isCtrl, isAlt)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::keyboard::KeyboardState` against `Keyboard` (#215).
//!
//! # Method
//!
//! The nine accessors are cut verbatim (build.rs) into `differential/wrap_draft_keyboard.cpp`, with `KeyboardIO`, the key arrays and
//! the `KEY_STATE_*` enum cut verbatim from Keyboard.h and KeyDefs.h; the class is a stand-in holding only the members those cuts
//! use. 30,000 random op lists of 5 to 60 ops (set a key's status or state through the setters or raw, set the modifiers, query
//! every getter, `isShift`/`isCtrl`/`isAlt`, `getFirstKey` over a raw first key) run on both sides; every answer is compared.
//! Keys are the whole `UnsignedByte` range (clustered so that a key is revisited); modifiers and states are random 16-bit
//! values with single bits and the `KEY_STATE_*` combinations over-weighted; `bit` is a single bit, a combination, 0 or any
//! `Int`. `setKeyStatusData` takes only 0 and 1 (`StatusType`'s two values; others are not defined for the enum). The draft's
//! `KEY_STATE_*` constants are not compared separately: every modifier test would differ if one were wrong.
//!
//! # Prediction (before the first run, 2026-11-18)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-18: 30,000 op lists, **0 mismatches**, first run, no draft fix needed (prediction met). Planted bugs, 9 of 9 caught (mismatching lists
//! of 30,000): 12321 16,139; 12322 345; 12323 3,600; 12324 5,811; 12325 6,629; 12326 11,725; 12327 8,501; 12328 20,230; 12329 4,798.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 12321 `getKeyStateBit` needs every bit of `bit`; 12322 `setKeyStateData` ORs into the state; 12323 `isShift`
//! ignores `KEY_STATE_SHIFT2`; 12324 `isCtrl` ignores the right control; 12325 `isAlt` tests the right shift for the right alt;
//! 12326 `getKeyStatusData` reads the event array; 12327 `getKeySequenceData` cut to 16 bits; 12328 `getFirstKey` is the second
//! key; 12329 `setKeyStatusData` writes the next key.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::keyboard::KeyboardState;
use rts_lockstep_engine::drafts::sweep_common_client::keyboard::KeyboardIO;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_kb_run(input: *const i32, nin: i32, out: *mut i32, cap: i32) -> i32;
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

fn key(g: &mut Gen, hot: &[i32; 4]) -> i32 {
    if g.below(3) == 0 {
        g.below(256) as i32
    } else {
        hot[g.below(4) as usize]
    }
}

fn state16(g: &mut Gen) -> i32 {
    match g.below(5) {
        0 => 1 << g.below(11),
        1 => (g.below(0x800)) as i32,
        2 => 0,
        3 => 0xFFFF,
        _ => g.below(0x10000) as i32,
    }
}

fn ops(g: &mut Gen) -> Vec<i32> {
    let hot = [g.below(256) as i32, g.below(256) as i32, 0, 255];
    let mut v = Vec::new();
    for _ in 0..(5 + g.below(56)) {
        match g.below(12) {
            0 => v.extend_from_slice(&[0, key(g, &hot), g.below(2) as i32]),
            1 => v.extend_from_slice(&[1, key(g, &hot), g.below(256) as i32]),
            2 => v.extend_from_slice(&[2, key(g, &hot)]),
            3 => {
                let bit = match g.below(5) {
                    0 => 1 << g.below(16),
                    1 => state16(g),
                    2 => 0,
                    3 => g.i32(),
                    _ => 0x0010 | 0x0020 | 0x0400,
                };
                v.extend_from_slice(&[3, key(g, &hot), bit]);
            }
            4 => v.extend_from_slice(&[4, key(g, &hot)]),
            5 => {
                let m = state16(g);
                v.extend_from_slice(&[5, m]);
            }
            6 => v.push(6),
            7 => {
                let st = state16(g);
                v.extend_from_slice(&[7, key(g, &hot), st, g.i32(), g.below(2) as i32]);
            }
            8 => v.push(8),
            9 => {
                let st = state16(g);
                v.extend_from_slice(&[9, g.below(256) as i32, g.below(256) as i32, st, g.i32()]);
            }
            _ => v.push(6),
        }
    }
    v
}

fn run_rust(input: &[i32]) -> Vec<i32> {
    let mut k = KeyboardState::new(256);
    let mut o = Vec::new();
    let mut at = 0;
    while at < input.len() {
        let kind = input[at];
        at += 1;
        match kind {
            0 => {
                k.set_key_status_data(input[at] as u8, input[at + 1] as u8);
                at += 2;
            }
            1 => {
                k.set_key_state_data(input[at] as u8, input[at + 1] as u8);
                at += 2;
            }
            2 => {
                o.push(k.get_key_status_data(input[at] as u8) as i32);
                at += 1;
            }
            3 => {
                o.push(k.get_key_state_bit(input[at] as u8, input[at + 1]) as i32);
                at += 2;
            }
            4 => {
                o.push(k.get_key_sequence_data(input[at] as u8) as i32);
                at += 1;
            }
            5 => {
                k.modifiers = input[at] as u16;
                at += 1;
            }
            6 => {
                o.push(k.is_shift() as i32);
                o.push(k.is_ctrl() as i32);
                o.push(k.is_alt() as i32);
            }
            7 => {
                let s = &mut k.key_status[input[at] as u8 as usize];
                s.state = input[at + 1] as u16;
                s.sequence = input[at + 2] as u32;
                s.status = input[at + 3] as u8;
                at += 4;
            }
            8 => {
                let f: &KeyboardIO = k.get_first_key();
                o.extend_from_slice(&[f.key as i32, f.status as i32, f.state as i32, f.sequence as i32]);
            }
            _ => {
                k.keys[0] = KeyboardIO { key: input[at] as u8, status: input[at + 1] as u8, state: input[at + 2] as u16, sequence: input[at + 3] as u32 };
                at += 4;
            }
        }
    }
    o
}

#[test]
fn keyboard_op_lists() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_4B0A);
    let mut c = Check::new("Keyboard accessors and modifier tests");
    let mut buf = vec![0i32; 100_000];
    for case in 0..30_000 {
        let input = ops(&mut g);
        let rust = run_rust(&input);
        let n = unsafe { ref_kb_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n >= 0, "the output buffer is too small");
        let cpp = &buf[..n as usize];
        let first = rust.iter().zip(cpp.iter()).position(|(a, b)| a != b);
        c.eq(rust.len() == cpp.len() && first.is_none(), true, || format!("case {case}: first differing answer {first:?} (lengths {} and {})", rust.len(), cpp.len()));
    }
    finish(c);
}
