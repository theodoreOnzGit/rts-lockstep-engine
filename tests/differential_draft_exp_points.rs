// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/generals_exp_points.rs` (`generals_exp_points_input`,
//! `generals_exp_points_system`) against upstream `GeneralsExpPoints.cpp`'s `GeneralsExpPointsInput` and `GeneralsExpPointsSystem`, cut
//! verbatim by build.rs (`drafts_ui3b`; `differential/wrap_draft_exp_points.cpp`, whose stand-ins are listed there).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides) of 1 to 12 messages, each to the input or the system callback, with a message
//! code drawn from the window messages (0 to 30), the gadget messages (16,384 to 16,420), `GWM_USER`, and any value; the first datum from
//! the key codes (ESC, other keys, values with bits above the low byte, which upstream's `UnsignedByte key = mData1` truncates), 0, 1, 2
//! (for `GWM_INPUT_FOCUS`), or any value; for `GBM_SELECTED` to the system callback it is one of 2 to 5 windows (the exit button's id, as
//! the C++ computes it with NAMEKEY, or others, duplicates included). The second datum is the address of the Bool `*(Bool *)mData2` writes
//! (initial value random). With or without `TheInGameUI`. After every message the return value, the Bool, and the calls on the stand-ins
//! (`placeBuildAvailable`, `hidePurchaseScience`, `processContextSensitiveButtonClick` with the control's id and the message) are compared.
//! EXCLUDED: `GBM_SELECTED` with a NULL control (upstream dereferences NULL; the draft returns handled).
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches: the two functions are line for line. The likeliest miss is a message constant that the draft has different from the C++ enum.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21061 | the key is not truncated to a byte (`d1 == KEY_ESC`) |
//! | 21062 | `GWM_MOUSE_ENTERING` does not leave building placement |
//! | 21063 | a key other than Escape is reported ignored |
//! | 21064 | any key hides the screen |
//! | 21065 | `GWM_INPUT_FOCUS` says the window wants the focus |
//! | 21066 | the focus is refused for any non-zero first datum (`!= 0` for `== TRUE`) |
//! | 21067 | the exit button is tested inverted (the others hide the screen) |
//! | 21068 | `GGM_FOCUS_CHANGE` is ignored |
//! | 21069 | the button click is forwarded as `GBM_SELECTED_RIGHT` |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences**, max ULP 0 (nothing is floating point here). No draft change was needed. Prediction held.
//! Planted bugs (mismatching sequences of 20,000): 21061 1,284; 21062 4,559; 21063 3,263; 21064 3,263; 21065 656; 21066 697; 21067 2,098; 21068 2,116; 21069 990. **9 of 9 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::window_manager::{data_win, gwm, win_data, GameWindowManager, GuiHost, MsgHandled};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::generals_exp_points::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_exp_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Host {
    log: Vec<u32>,
    has_ui: bool,
}

impl GuiHost for Host {}

impl GeneralsExpPointsHost for Host {
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        key_of(name)
    }
    fn place_build_available_none(&mut self) {
        if self.has_ui {
            self.log.push(1);
        }
    }
    fn hide_purchase_science(&mut self, _wm: &mut GameWindowManager) {
        self.log.push(2);
    }
    fn process_context_sensitive_button_click(&mut self, wm: &mut GameWindowManager, control: WinId, msg: u32) {
        self.log.push(3);
        self.log.push(wm.win(control).id as u32);
        self.log.push(msg);
    }
}

fn msg_code(g: &mut Gen) -> u32 {
    match g.below(10) {
        0..=2 => g.below(31),
        3..=5 => 16384 + g.below(37),
        6 => gwm::MOUSE_ENTERING,
        7 => gwm::CHAR,
        8 => [gwm::INPUT_FOCUS, 16392, 16387, 32768][g.below(4) as usize],
        _ => g.u32(),
    }
}

fn datum(g: &mut Gen) -> u32 {
    match g.below(10) {
        0..=2 => 1,
        3 => 0,
        4 => 2,
        5 => 0x100 | 1,
        6 => u32::from(g.below(256) as u8),
        7 => g.below(0x2_0000) << 8 | 1,
        _ => g.u32(),
    }
}

#[test]
fn exp_points_callbacks_match_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_01);
    let mut chk = Check::new("GeneralsExpPoints Input / System");
    let exit_id = key_of(b"GeneralsExpPoints.wnd:ButtonExit");
    for _ in 0..cases(20_000) {
        let nwin = 2 + g.below(4) as usize;
        let specs: Vec<WinSpec> = (0..nwin).map(|i| WinSpec { id: if i == 0 || g.below(3) == 0 { exit_id } else { 100 + g.below(5) as i32 }, hidden: false, enabled: true, no_focus: false, extra: 0 }).collect();
        let (mut wm, order) = make_scene(&specs);
        let mut host = Host { log: Vec::new(), has_ui: true };
        let mut input: Vec<u32> = vec![nwin as u32];
        input.extend(specs.iter().map(|s| s.id as u32));
        let steps = 1 + g.below(12);
        input.push(steps);
        let mut rust: Vec<u32> = Vec::new();
        for _ in 0..steps {
            let kind = g.below(2);
            let msg = msg_code(&mut g);
            let is_win = kind == 1 && msg == 16392;
            let (d1, d1_idx) = if is_win {
                let i = g.below(nwin as u32);
                (win_data(Some(order[i as usize])), i + 1)
            } else {
                let d = datum(&mut g);
                (d, d)
            };
            let d2 = g.below(2);
            host.has_ui = g.below(4) != 0;
            input.extend([kind, msg, d1_idx, u32::from(is_win), d2, u32::from(host.has_ui)]);
            host.log.clear();
            wm.focus_reply = d2 != 0;
            let window = order[0];
            let r = if kind == 0 { generals_exp_points_input(&mut wm, &mut host, window, msg, d1, 0) } else { generals_exp_points_system(&mut wm, &mut host, window, msg, d1, 0) };
            let _ = data_win(0);
            rust.extend(host.log.iter().copied());
            rust.extend([100, u32::from(r == MsgHandled::Handled), u32::from(wm.focus_reply)]);
        }
        let mut buf = vec![0u32; 1 << 14];
        let len = unsafe { ref_ui3b_exp_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
