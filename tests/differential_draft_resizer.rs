#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarResizer.cpp
//   (findResizerWindow, newResizerWindow, sizeWindowsDefault, sizeWindowsAlt)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::control_bar_resizer` against `ControlBarResizer` (#215).
//!
//! # Method
//!
//! The two classes (ControlBarResizer.h), the four functions and `ResizerWindow`'s constructor (ControlBarResizer.cpp), and
//! `GameWindow::normalizeWindowRegion`, `winSetPosition`, `winGetPosition`, `winSetSize`, `winGetSize` (GameWindow.cpp) are cut
//! verbatim (build.rs) into `differential/wrap_draft_resizer.cpp`; the wrapper stands in for the window manager's lookup by name,
//! the name key generator, the display size and `winSendSystemMsg`. 30,000 programs of 3 to 40 ops from a fixed seed run on
//! both sides: 0 to 6 windows (names from `W0`..`W7`, so that two windows can share a name and a resizer entry can name no
//! window; positions and sizes from the whole `i32` range, negative sizes included), resizer entries created and searched by
//! name (a second entry of a name that exists, a name with no window), alternate positions and sizes (zero, negative, and
//! within +-2^20 so that the scaled values fit an `Int`), display sizes 0 to 8,192 (so that the scale `Real(width) / 800` takes
//! non-dyadic values), windows moved and resized between calls, and dumps of every window's region and size and every entry's
//! four coordinates.
//!
//! Excluded: alternate coordinates times the scale outside the range of `Int` (the `Real` to `Int` conversion is undefined in C++).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches: the draft reads `Real x = (Real)TheDisplay->getWidth() / 800` as f32 and truncates, as C++ does.
//!
//! # Results
//!
//! Run 2026-11-19: `ControlBarResizer programs` 30,000 compared, 0 mismatches (max ULP 0). Prediction hit. Plants 14281..14289: all 9
//! caught (697 to 17,202 of 30,000 programs each); baseline 0.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 14281 `x` scales by the display's height; 14282 `y` divides by 800; 14283 the size is set only when both
//! alternate extents are positive; 14284 products rounded, not truncated; 14285 the alternate size scales the alternate position;
//! 14286 `findResizerWindow` finds the last; 14287 the default size is the region's far corner; 14288 `sizeWindowsDefault` swaps
//! the position's coordinates; 14289 the products are taken in `f64`.

mod support;

use rts_lockstep_engine::client::window_manager::{GameWindowManager, NoGuiHost, SystemFunc};
use rts_lockstep_engine::client::window_script::WinInstanceData;
use rts_lockstep_engine::drafts::sweep_gui_logic::control_bar_resizer::ControlBarResizer;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_resizer_run(input: *const i32, nin: i32, out: *mut i32, cap: i32) -> i32;
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

fn coord(g: &mut Gen) -> i32 {
    match g.below(6) {
        0 => 0,
        1 => g.i32() >> g.below(16),
        2 => -(g.below(2000) as i32),
        _ => g.below(1600) as i32,
    }
}

fn alt(g: &mut Gen) -> i32 {
    match g.below(6) {
        0 => 0,
        1 => -(g.below(1 << 20) as i32),
        2 => g.below(1 << 20) as i32,
        3 => -(g.below(100) as i32),
        _ => g.below(1200) as i32,
    }
}

fn program(g: &mut Gen) -> Vec<i32> {
    let mut v = vec![g.below(8193) as i32, g.below(8193) as i32];
    let nw = g.below(7) as i32;
    v.push(nw);
    for _ in 0..nw {
        v.extend_from_slice(&[g.below(8) as i32, coord(g), coord(g), coord(g), coord(g)]);
    }
    let mut entries = 0;
    for _ in 0..(3 + g.below(38)) {
        match g.below(10) {
            0 | 1 => {
                v.extend_from_slice(&[0, g.below(8) as i32]);
                entries += 1; // (an entry only when its window exists; the index is looked up, so a guess is harmless)
            }
            2 => v.extend_from_slice(&[1, g.below(8) as i32]),
            3 | 4 => {
                if entries > 0 {
                    v.extend_from_slice(&[2, g.below(entries as u32) as i32, alt(g), alt(g), alt(g), alt(g)]);
                }
            }
            5 => v.push(3),
            6 => v.push(4),
            7 => {
                if nw > 0 {
                    v.extend_from_slice(&[5, g.below(nw as u32) as i32, coord(g), coord(g), coord(g), coord(g)]);
                }
            }
            8 => v.extend_from_slice(&[6, g.below(8193) as i32, g.below(8193) as i32]),
            _ => v.push(7),
        }
    }
    v.push(7);
    v
}

fn name(i: i32) -> Vec<u8> {
    format!("W{i}").into_bytes()
}

fn dump(wm: &GameWindowManager, r: &ControlBarResizer, wins: &[usize], out: &mut Vec<i32>) {
    for &w in wins {
        let g = wm.win(w);
        out.extend_from_slice(&[g.region.0, g.region.1, g.region.2, g.region.3, g.size.0, g.size.1]);
    }
    for e in &r.resizer_windows_list {
        out.extend_from_slice(&[e.default_pos.0, e.default_pos.1, e.default_size.0, e.default_size.1, e.alt_pos.0, e.alt_pos.1, e.alt_size.0, e.alt_size.1]);
    }
}

fn run_rust(p: &[i32]) -> Vec<i32> {
    let mut at = 0;
    let mut wm = GameWindowManager::new((p[0], p[1]));
    at += 2;
    let mut h = NoGuiHost;
    let nw = p[at] as usize;
    at += 1;
    let mut wins = Vec::new();
    for _ in 0..nw {
        let mut inst = WinInstanceData::new();
        inst.decorated_name_string = name(p[at]);
        let w = wm.win_create(&mut h, None, 0, p[at + 1], p[at + 2], p[at + 3], p[at + 4], SystemFunc::Default, Some(inst));
        wins.push(w);
        at += 5;
    }
    let mut r = ControlBarResizer::default();
    let mut out = Vec::new();
    while at < p.len() {
        let kind = p[at];
        at += 1;
        match kind {
            0 => {
                let res = r.new_resizer_window(&wm, &name(p[at]));
                at += 1;
                out.push(res.map_or(-1, |i| i as i32));
            }
            1 => {
                let res = r.find_resizer_window(&name(p[at]));
                at += 1;
                out.push(res.map_or(-1, |i| i as i32));
            }
            2 => {
                if let Some(e) = r.resizer_windows_list.get_mut(p[at] as usize) {
                    e.alt_pos = (p[at + 1], p[at + 2]);
                    e.alt_size = (p[at + 3], p[at + 4]);
                }
                at += 5;
            }
            3 => r.size_windows_default(&mut wm, &mut h),
            4 => r.size_windows_alt(&mut wm, &mut h),
            5 => {
                let w = wins[p[at] as usize];
                wm.win_set_position(w, p[at + 1], p[at + 2]);
                wm.win_set_size(&mut h, w, p[at + 3], p[at + 4]);
                at += 5;
            }
            6 => {
                wm.display = (p[at], p[at + 1]);
                at += 2;
            }
            _ => dump(&wm, &r, &wins, &mut out),
        }
    }
    out
}

#[test]
fn resizer_programs() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C0B2);
    let mut c = Check::new("ControlBarResizer programs");
    let mut buf = vec![0i32; 100_000];
    for case in 0..30_000 {
        let p = program(&mut g);
        let rust = run_rust(&p);
        let n = unsafe { ref_resizer_run(p.as_ptr(), p.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n >= 0, "the output buffer is too small");
        let cpp = &buf[..n as usize];
        let first = rust.iter().zip(cpp.iter()).position(|(a, b)| a != b);
        c.eq(rust.len() == cpp.len() && first.is_none(), true, || format!("case {case}: first differing word {first:?} (lengths {} and {})", rust.len(), cpp.len()));
    }
    finish(c);
}
