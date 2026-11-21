// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/replay_controls.rs` (`replay_control_input`,
//! `replay_control_system`) against upstream `ReplayControls.cpp`'s `ReplayControlInput` and `ReplayControlSystem`, cut verbatim by build.rs
//! (`drafts_ui3b`; `differential/wrap_draft_replay_controls.cpp`).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides) of 1 to 16 messages to the input or the system callback; the code drawn from the window
//! messages (0 to 30), the gadget messages (16,384 to 16,420), `GWM_USER` and any value; the two data any value. The return value of each is compared.
//! Neither callback touches a window or a global, so the stand-ins are none.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches. The system callback handles only `GBM_SELECTED` (16,392); the input callback ignores everything.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21081 | the system callback also handles `GBM_SELECTED_RIGHT` |
//! | 21082 | the system callback handles every message |
//! | 21083 | the input callback handles `GWM_CHAR` |
//! | 21084 | the system callback ignores `GBM_SELECTED` |
//! | 21085 | the system callback handles `GBM_MOUSE_ENTERING` (the neighbouring gadget message, 16,390) |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences**, max ULP 0 (nothing is floating point here). No draft change was needed. Prediction held.
//! Planted bugs (of 20,000): 21081 735; 21082 18,526; 21083 4,762; 21084 4,634; 21085 759. **5 of 5 caught** (the unit is two functions of one line each, so five plants).
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::window_manager::{GameWindowManager, MsgHandled};
use rts_lockstep_engine::drafts::ui::replay_controls::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_replay_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

#[test]
fn replay_controls_match_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_05);
    let mut chk = Check::new("ReplayControls Input / System");
    let (mut wm, order) = make_scene(&[WinSpec { id: 1, hidden: false, enabled: true, no_focus: false, extra: 0 }]);
    let _: &GameWindowManager = &wm;
    for _ in 0..cases(20_000) {
        let n = 1 + g.below(16) as usize;
        let mut input = vec![n as u32];
        let mut rust = Vec::new();
        for _ in 0..n {
            let kind = g.below(2);
            let msg = match g.below(6) {
                0 | 1 => g.below(31),
                2 | 3 => 16384 + g.below(37),
                4 => [16392u32, 32768, 21][g.below(3) as usize],
                _ => g.u32(),
            };
            let (d1, d2) = (g.u32(), g.u32());
            input.extend([kind, msg, d1, d2]);
            let r = if kind == 0 { replay_control_input(order[0], msg, d1, d2) } else { replay_control_system(&mut wm, &mut NoHost, order[0], msg, d1, d2) };
            rust.push(u32::from(r == MsgHandled::Handled));
        }
        let mut buf = vec![0u32; 64];
        let len = unsafe { ref_ui3b_replay_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
