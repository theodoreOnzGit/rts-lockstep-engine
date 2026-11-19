#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameWindow.cpp (W3DGameWindow::getTextSize, setTextLoc)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::w3d_game_window` against upstream's (#215).
//!
//! # Method
//!
//! `getTextSize` and `setTextLoc` are cut verbatim (build.rs, drafts_guilogic3) into `differential/wrap_draft_w3dwin.cpp`; the sentence renderer's
//! `Get_Text_Extents` is a stand-in that returns the extents the test sets (the real one measures a font).
//!
//! - `getTextSize`: 20,000 cases with extents drawn from exact integers, halves, negative fractions (truncation toward zero, not floor), and uniform values
//!   in [-5000, 5000], and each of the four combinations of "wants width" and "wants height". Compared: both outputs, with a sentinel for an output not written.
//! - `setTextLoc`: 20,000 programs of 1 to 12 calls with coordinates from a small range (so that a call often repeats the position); the flag
//!   `m_newTextPos` is cleared after calls with an odd `x`, as `drawText` clears it. Compared: position and flag after each call.
//!
//! Excluded: extents that are NaN, infinite or beyond the `Int` range (the float to int conversion is undefined in C++; Rust saturates).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-19: 20,000 cases per function, **0 mismatches**, first run (prediction met). Planted bugs, 6 of 6 caught (mismatches of 20,000): 15758 8,917 (rounds, not truncates);
//! 15759 15,103 (width and height swapped); 15760 14,932 (both always written); 15761 2,452 (flag set on every call); 15762 8,008 (y change does not flag); 15763 8,097 (x change
//! does not flag).

mod support;

use rts_lockstep_engine::drafts::sweep_gui_logic::w3d_game_window as w;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w3d_size(x: i32, y: i32, want_w: i32, want_h: i32, out: *mut i32) -> i32;
    fn ref_w3d_loc(inp: *const i32, n: i32, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

struct R(f32, f32);

impl w::TextRenderer for R {
    fn get_text_extents(&self, _text: &[u8]) -> (f32, f32) {
        (self.0, self.1)
    }
}

fn extent(g: &mut Gen) -> f32 {
    match g.below(5) {
        0 => g.below(2000) as f32 - 1000.0,
        1 => g.below(2000) as f32 / 2.0 - 500.0,
        2 => -(g.below(100) as f32) - 0.5,
        3 => -(g.below(100) as f32) - g.uniform(0.0, 1.0),
        _ => g.uniform(-5000.0, 5000.0),
    }
}

#[test]
fn w3d_window() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_3D31);
    let mut size = Check::new("W3DGameWindow::getTextSize");
    let mut loc = Check::new("W3DGameWindow::setTextLoc");
    for i in 0..20_000 {
        let (x, y) = (extent(&mut g), extent(&mut g));
        let (ww, wh) = (g.below(2) == 0, g.below(2) == 0);
        let st = w::W3dGameWindowText::default();
        let (a, b) = st.get_text_size(&R(x, y), b"text", ww, wh);
        let want = vec![a.unwrap_or(-77777), b.unwrap_or(-77777)];
        let mut out = [0i32; 2];
        unsafe { ref_w3d_size(x.to_bits() as i32, y.to_bits() as i32, ww as i32, wh as i32, out.as_mut_ptr()) };
        size.eq(want, out.to_vec(), || format!("case {i}: extents {x} {y} want {ww} {wh}"));

        let n = 1 + g.below(12) as usize;
        let calls: Vec<i32> = (0..n * 2).map(|_| g.below(5) as i32 - 2).collect();
        let mut st = w::W3dGameWindowText::default();
        let mut want = Vec::new();
        for c in calls.chunks(2) {
            st.set_text_loc(c[0], c[1]);
            want.extend([st.text_pos.0, st.text_pos.1, st.new_text_pos as i32]);
            if c[0] & 1 != 0 {
                st.new_text_pos = false;
            }
        }
        let mut out = vec![0i32; 64];
        let k = unsafe { ref_w3d_loc(calls.as_ptr(), calls.len() as i32, out.as_mut_ptr()) };
        out.truncate(k as usize);
        loc.eq(want, out, || format!("case {i}: {calls:?}"));
    }
    if planted() {
        size.report();
        loc.report();
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    size.finish();
    loc.finish();
}
