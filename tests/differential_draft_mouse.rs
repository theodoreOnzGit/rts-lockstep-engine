#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Input/Mouse.cpp (moveMouse, updateMouseData, processMouseEvent, checkForDrag)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::mouse::MouseInput` against `Mouse` (#215).
//!
//! # Method
//!
//! The four functions are cut verbatim (build.rs) into `differential/wrap_draft_mouse.cpp`, with `MouseIO` and the button state
//! enum, the `Mouse` members they use and the `GameWindowMessage` enum cut verbatim from Mouse.h and GameWindow.h. The class is a
//! stand-in holding only those members (the real one needs the subsystem machinery and a constructor outside the cut), with a
//! scripted `getMouseEvent` (the device). 20,000 random scripts of 6 to 60 steps (setting `m_currMouse`, `m_prevMouse`, event
//! slots and the input frame; `moveMouse`; `checkForDrag`; `processMouseEvent` of a random slot, slot 0 often; `updateMouseData`
//! over a device script of 0 to 8 events, with lost-device results, and sometimes 250 to 300, half of those with no end marker, so that the array fills) run on both sides.
//! After every step the whole of `m_currMouse`, `m_prevMouse`, the counters, and (after an update) the event slots it filled are
//! compared. Regions are random, sometimes with min above max, and positions sometimes at the `Int` extremes. The GWM codes the
//! draft uses (`gwm::LEFT_DOWN` ..) are compared with the C++ enumerators.
//!
//! # Prediction (before the first run, 2026-11-18)
//!
//! One miss expected: `moveMouse`'s `+=` and `processMouseEvent`'s delta subtraction overflow at the `Int` extremes, which the
//! release build of this crate (overflow checks on, gh #761) panics on and the MSVC build wraps. Otherwise 0 mismatches.
//!
//! # Results
//!
//! 2026-11-18: 20,000 scripts, **0 mismatches**, first run, no draft fix needed; the 12 GWM codes equal the C++ enumerators.
//! **Prediction miss (recorded):** I expected a panic from `+=`/`-` overflow at the `Int` extremes. This repository's release profile has no
//! integer overflow checks (unlike the outram-park workspace's), so the draft wraps, as MSVC does and as the C++ built here (`-fwrapv`) does,
//! and the 8% of scripts with extreme coordinates match. In a build with overflow checks the draft WOULD panic there (a debug build does);
//! the draft is left as it is. First planted run: 12307 (array bound one short) was MISSED, because a device script of 250 or more events
//! always ended in `MOUSE_NONE` before the array filled; the generator now gives half of those scripts no end marker. Planted bugs, 10 of 10
//! caught after that (mismatching scripts of 20,000): 12301 1,362; 12304 19,007; 12305 7,546; 12306 14,611; 12307 752; 12308 685;
//! 12309 7,424; 12310 16,904; 12312 12,278; 12313 12,283; 12314 6,728.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 12301 the clamp checks min before max; 12304 `m_eventsThisFrame = index`; 12305 dead-input frame set only
//! for two events or more; 12306 a lost-device result is kept; 12307 the event array bound one short; 12308 `checkForDrag` for the
//! Down state only; 12309 `checkForDrag` on every event but the first; 12310 the wheel position not cleared; 12312 the button's
//! frame from the event, not the input frame; 12313 a double click stored as Down; 12314 a drag continues only after a down.

mod support;

use rts_lockstep_engine::client::window_manager::gwm;
use rts_lockstep_engine::drafts::sweep_common_client::mouse::{MouseButtonState, MouseDevice, MouseIO, MouseInput};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_gwm(k: i32) -> i32;
    fn ref_mouse_run(input: *const i32, nin: i32, out: *mut i32, cap: i32) -> i32;
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

fn state_of(v: i32) -> MouseButtonState {
    match v {
        1 => MouseButtonState::Down,
        2 => MouseButtonState::DoubleClick,
        _ => MouseButtonState::Up,
    }
}

fn io_of(w: &[i32]) -> MouseIO {
    MouseIO {
        pos: (w[0], w[1]),
        time: w[2] as u32,
        wheel_pos: w[3],
        delta_pos: (w[4], w[5]),
        left_state: state_of(w[6]),
        left_event: w[7] as u32,
        left_frame: w[8],
        right_state: state_of(w[9]),
        right_event: w[10] as u32,
        right_frame: w[11],
        middle_state: state_of(w[12]),
        middle_event: w[13] as u32,
        middle_frame: w[14],
    }
}

fn put_io(o: &mut Vec<i32>, m: &MouseIO) {
    o.extend_from_slice(&[
        m.pos.0, m.pos.1, m.time as i32, m.wheel_pos, m.delta_pos.0, m.delta_pos.1, m.left_state as i32, m.left_event as i32, m.left_frame,
        m.right_state as i32, m.right_event as i32, m.right_frame, m.middle_state as i32, m.middle_event as i32, m.middle_frame,
    ]);
}

struct Script {
    results: Vec<u8>,
    ios: Vec<MouseIO>,
    at: usize,
}

impl MouseDevice for Script {
    fn get_mouse_event(&mut self, result: &mut MouseIO, _flush: bool) -> u8 {
        if self.at >= self.results.len() {
            return 0;
        }
        *result = self.ios[self.at];
        self.at += 1;
        self.results[self.at - 1]
    }
}

/// The draft run on a script, producing the same words as `ref_mouse_run`.
fn run_rust(input: &[i32]) -> Vec<i32> {
    let mut m = MouseInput::new();
    m.min_x = input[0];
    m.max_x = input[1];
    m.min_y = input[2];
    m.max_y = input[3];
    m.input_moves_absolute = input[4] != 0;
    m.input_frame = input[5] as u32;
    m.dead_input_frame = input[6] as u32;
    let mut o = Vec::new();
    let mut at = 7;
    while at < input.len() {
        let kind = input[at];
        at += 1;
        let mut events = false;
        match kind {
            0 => {
                m.move_mouse(input[at], input[at + 1], input[at + 2]);
                at += 3;
            }
            1 => {
                m.process_mouse_event(input[at] as usize);
                at += 1;
            }
            2 => m.check_for_drag(),
            3 => {
                let n = input[at] as usize;
                at += 1;
                let mut s = Script { results: Vec::new(), ios: Vec::new(), at: 0 };
                for _ in 0..n {
                    s.results.push(input[at] as u8);
                    s.ios.push(io_of(&input[at + 1..at + 16]));
                    at += 16;
                }
                m.update_mouse_data(&mut s);
                events = true;
            }
            4 => {
                m.curr_mouse = io_of(&input[at..at + 15]);
                at += 15;
            }
            5 => {
                m.prev_mouse = io_of(&input[at..at + 15]);
                at += 15;
            }
            6 => {
                let idx = input[at] as usize;
                m.mouse_events[idx] = io_of(&input[at + 1..at + 16]);
                at += 16;
            }
            _ => {
                m.input_frame = input[at] as u32;
                at += 1;
            }
        }
        put_io(&mut o, &m.curr_mouse);
        put_io(&mut o, &m.prev_mouse);
        o.extend_from_slice(&[m.events_this_frame, m.dead_input_frame as i32, m.input_frame as i32]);
        if events {
            for i in 0..=(m.events_this_frame as usize) {
                put_io(&mut o, &m.mouse_events[i]);
            }
        }
    }
    o
}

fn coord(g: &mut Gen, extreme: bool) -> i32 {
    match g.below(12) {
        0 if extreme => [i32::MAX, i32::MIN, i32::MAX - 3, i32::MIN + 3, 1 << 30][g.below(5) as usize],
        1..=3 => g.below(900) as i32,
        4 => g.below(1200) as i32 - 600,
        5 => 0,
        _ => g.below(120) as i32 - 60,
    }
}

fn frame(g: &mut Gen) -> i32 {
    match g.below(6) {
        0 | 1 => 0,
        2 => g.i32(),
        _ => 1 + g.below(1000) as i32,
    }
}

fn event_code(g: &mut Gen, codes: &[i32; 12]) -> i32 {
    match g.below(8) {
        0 => 0,
        1 => g.below(40) as i32,
        _ => codes[g.below(12) as usize],
    }
}

fn io_words(g: &mut Gen, codes: &[i32; 12], extreme: bool) -> [i32; 15] {
    let mut w = [0i32; 15];
    w[0] = coord(g, extreme);
    w[1] = coord(g, extreme);
    w[2] = if g.below(3) == 0 { g.i32() } else { g.below(100000) as i32 };
    w[3] = match g.below(4) {
        0 => 0,
        1 => [120, -120][g.below(2) as usize],
        _ => g.below(400) as i32 - 200,
    };
    w[4] = g.below(50) as i32 - 25;
    w[5] = g.below(50) as i32 - 25;
    for b in 0..3 {
        w[6 + 3 * b] = g.below(3) as i32;
        w[7 + 3 * b] = event_code(g, codes);
        w[8 + 3 * b] = frame(g);
    }
    w
}

fn script(g: &mut Gen, codes: &[i32; 12]) -> Vec<i32> {
    let extreme = g.below(6) == 0;
    let mut v = Vec::new();
    let (mut lox, mut hix, mut loy, mut hiy) = (0, 799, 0, 599);
    match g.below(6) {
        0 => {
            lox = coord(g, false);
            hix = coord(g, false);
            loy = coord(g, false);
            hiy = coord(g, false);
        } // possibly min above max
        1 => {
            lox = -100;
            hix = 100;
            loy = -50;
            hiy = 50;
        }
        2 if extreme => {
            lox = i32::MIN;
            hix = i32::MAX;
            loy = i32::MIN;
            hiy = i32::MAX;
        }
        _ => {}
    }
    v.extend_from_slice(&[lox, hix, loy, hiy, g.below(2) as i32, frame(g), g.i32()]);
    for _ in 0..(6 + g.below(55)) {
        match g.below(14) {
            0..=2 => {
                v.extend_from_slice(&[0, coord(g, extreme), coord(g, extreme), g.below(2) as i32]);
            }
            3..=6 => {
                let idx = if g.below(2) == 0 { 0 } else { g.below(256) as i32 };
                v.extend_from_slice(&[1, idx]);
            }
            7 => v.push(2),
            8 | 9 => {
                let n = if g.below(12) == 0 { 250 + g.below(51) as usize } else { g.below(9) as usize };
                v.extend_from_slice(&[3, n as i32]);
                let no_end = n >= 250 && g.below(2) == 0; // a device with no MOUSE_NONE: the array fills
                for _ in 0..n {
                    let r = match g.below(8) {
                        0 => 0xFF, // MOUSE_LOST
                        1 if !no_end => 0x00, // MOUSE_NONE
                        2 => 0x80,
                        _ => 1,
                    };
                    v.push(r);
                    v.extend_from_slice(&io_words(g, codes, extreme));
                }
            }
            10 => {
                v.push(4);
                v.extend_from_slice(&io_words(g, codes, extreme));
            }
            11 => {
                v.push(5);
                v.extend_from_slice(&io_words(g, codes, extreme));
            }
            12 => {
                v.extend_from_slice(&[6, g.below(256) as i32]);
                v.extend_from_slice(&io_words(g, codes, extreme));
            }
            _ => v.extend_from_slice(&[7, frame(g)]),
        }
    }
    v
}

#[test]
fn gwm_codes_match_the_enum() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mine = [
        gwm::LEFT_DOWN, gwm::LEFT_UP, gwm::LEFT_DOUBLE_CLICK, gwm::LEFT_DRAG, gwm::MIDDLE_DOWN, gwm::MIDDLE_UP, gwm::MIDDLE_DOUBLE_CLICK,
        gwm::MIDDLE_DRAG, gwm::RIGHT_DOWN, gwm::RIGHT_UP, gwm::RIGHT_DOUBLE_CLICK, gwm::RIGHT_DRAG,
    ];
    let mut c = Check::new("GameWindowMessage mouse codes");
    for (k, m) in mine.iter().enumerate() {
        c.eq(*m as i32, unsafe { ref_gwm(k as i32) }, || format!("code {k}"));
    }
    c.finish();
}

#[test]
fn mouse_scripts() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut codes = [0i32; 12];
    for (k, c) in codes.iter_mut().enumerate() {
        *c = unsafe { ref_gwm(k as i32) };
    }
    let mut g = Gen::new(0x215_3045E);
    let mut c = Check::new("Mouse move/update/process/drag");
    let mut buf = vec![0i32; 4_000_000];
    for case in 0..20_000 {
        let input = script(&mut g, &codes);
        let rust = std::panic::catch_unwind(|| run_rust(&input));
        let n = unsafe { ref_mouse_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n >= 0, "the output buffer is too small");
        let cpp = &buf[..n as usize];
        match rust {
            Ok(r) => {
                let first = r.iter().zip(cpp.iter()).position(|(a, b)| a != b);
                c.eq(r.len() == cpp.len() && first.is_none(), true, || format!("case {case}: first differing word {first:?} (lengths {} and {})", r.len(), cpp.len()));
            }
            Err(_) => c.eq(false, true, || format!("case {case}: the draft panicked (an overflow?); script {:?}", &input[..input.len().min(40)])),
        }
    }
    finish(c);
}
