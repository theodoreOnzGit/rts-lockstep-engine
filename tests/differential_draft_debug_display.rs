#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/DebugDisplay.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::debug_display::DebugDisplay` against `DebugDisplay` (#215).
//!
//! # Method
//!
//! `DebugDisplay::DebugDisplay()` to the end of `DebugDisplay.cpp` is cut verbatim (build.rs) into `differential/wrap_draft_debug_display.cpp`,
//! on the real `DebugDisplay.h` and a derived class whose `drawText` logs its call (the wrapper lists its stand-ins). A case is a random op
//! sequence (set the cursor, colour, margins; reset; `printf` of a text; the four getters) on a display of random width and height; both sides log
//! every `drawText` (x, y, the text) and the whole state after each op, and the logs are compared whole. 20,000 sequences from a fixed seed. The
//! texts are heavy in newlines (leading, trailing, doubled), spaces, and bytes above 127; `%` is just text (`printf("%s", text)`).
//!
//! Excluded inputs: texts of 5,120 bytes or more (upstream's static buffer; its `DEBUG_ASSERTCRASH` is compiled out of release and
//! `vsprintf` overruns); a NUL in the text (a C string ends there).
//!
//! # Prediction (written before the first run, 2026-11-19)
//!
//! 0 mismatches on the printf and the accessors; one failure to compile, not a mismatch: the draft's colour enum has only `White`, so the
//! colour ops cannot be expressed. The enum gets the header's six variants (`BLACK`, `YELLOW`, `RED`, `GREEN`, `BLUE`, `NUM_COLORS`).
//!
//! # Planted bugs and results: see the end of this comment, filled after the runs.

mod support;

use rts_lockstep_engine::drafts::logic_common::debug_display::{DebugDisplay, DebugDisplayColor, DebugDisplayDevice};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dd_run(width: i32, height: i32, ops: *const i32, nops: i32, out: *mut i32, cap: i32) -> i32;
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

struct Dev {
    log: Vec<i32>,
}

impl DebugDisplayDevice for Dev {
    fn draw_text(&mut self, x: i32, y: i32, text: &[u8]) {
        self.log.extend([100, x, y, text.len() as i32]);
        self.log.extend(text.iter().map(|&b| b as i32));
    }
}

fn color(i: i32) -> DebugDisplayColor {
    match i {
        0 => DebugDisplayColor::White,
        1 => DebugDisplayColor::Black,
        2 => DebugDisplayColor::Yellow,
        3 => DebugDisplayColor::Red,
        4 => DebugDisplayColor::Green,
        5 => DebugDisplayColor::Blue,
        _ => DebugDisplayColor::NumColors,
    }
}

fn state(log: &mut Vec<i32>, d: &DebugDisplay) {
    log.extend([d.text_color as i32, d.x_pos, d.y_pos, d.width, d.height, d.right_margin, d.left_margin]);
}

fn run_rust(width: i32, height: i32, ops: &[i32]) -> Vec<i32> {
    let mut dev = Dev { log: Vec::new() };
    let mut d = DebugDisplay::new();
    d.width = width;
    d.height = height;
    d.reset();
    let mut log = Vec::new();
    state(&mut log, &d);
    let mut p = 0;
    while p < ops.len() {
        let op = ops[p];
        p += 1;
        log.push(1000 + op);
        match op {
            0 => {
                d.set_cursor_pos(ops[p], ops[p + 1]);
                p += 2;
            }
            1 => {
                d.set_text_color(color(ops[p]));
                p += 1;
            }
            2 => {
                d.set_right_margin(ops[p]);
                p += 1;
            }
            3 => {
                d.set_left_margin(ops[p]);
                p += 1;
            }
            4 => d.reset(),
            5 => {
                let n = ops[p] as usize;
                p += 1;
                let text: Vec<u8> = ops[p..p + n].iter().map(|&w| w as u8).collect();
                p += n;
                d.printf(&mut dev, &text);
            }
            _ => log.extend([d.get_cursor_x_pos(), d.get_cursor_y_pos(), d.get_width(), d.get_height()]),
        }
        log.append(&mut dev.log);
        state(&mut log, &d);
    }
    log
}

fn gen_text(g: &mut Gen) -> Vec<i32> {
    let n = match g.below(5) {
        0 => 0,
        1 => g.below(4),
        _ => g.below(40),
    };
    let mut t = Vec::new();
    while t.len() < n as usize {
        match g.below(8) {
            0..=2 => t.push(b'\n' as i32),
            3 => t.push(b' ' as i32),
            4 => t.push(0x80 + g.below(128) as i32),
            5 => t.push(b'%' as i32),
            _ => t.push(b'a' as i32 + g.below(26) as i32),
        }
    }
    t
}

#[test]
fn debug_display_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_DD15);
    let mut c = Check::new("DebugDisplay");
    let mut drawn = 0u32;
    for case in 0..20_000 {
        let (width, height) = (g.below(200) as i32 - 20, g.below(100) as i32);
        let mut ops: Vec<i32> = Vec::new();
        for _ in 0..1 + g.below(14) {
            match g.below(12) {
                0 => ops.extend([0, g.below(60) as i32 - 10, g.below(60) as i32 - 10]),
                1 => ops.extend([1, g.below(6) as i32]),
                2 => ops.extend([2, g.below(80) as i32 - 20]),
                3 => ops.extend([3, g.below(80) as i32 - 20]),
                4 => ops.push(4),
                5 => ops.push(6),
                _ => {
                    let t = gen_text(&mut g);
                    ops.extend([5, t.len() as i32]);
                    ops.extend(t);
                }
            }
        }
        let mut out = vec![0i32; 20_000];
        let len = unsafe { ref_dd_run(width, height, ops.as_ptr(), ops.len() as i32, out.as_mut_ptr(), out.len() as i32) };
        assert!(len >= 0, "the log did not fit");
        out.truncate(len as usize);
        let rust = run_rust(width, height, &ops);
        if out.contains(&100) {
            drawn += 1;
        }
        let first = rust.iter().zip(out.iter()).position(|(a, b)| a != b).unwrap_or(rust.len().min(out.len()));
        c.eq(rust == out, true, || format!("case {case}: size {width}x{height} ops {ops:?}; first difference at word {first}: rust {:?} c++ {:?}", rust.get(first.saturating_sub(6)..(first + 4).min(rust.len())), out.get(first.saturating_sub(6)..(first + 4).min(out.len()))));
    }
    println!("(debug_display: {drawn} sequences drew text)");
    finish(c);
}
