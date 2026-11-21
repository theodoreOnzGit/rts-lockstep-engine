// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/w3d_motd.rs` (`motd_system`, the static `closeButtonID`) against upstream
//! `W3DMOTD.cpp`'s `MOTDSystem`, cut verbatim by build.rs (`drafts_ui3b`; `differential/wrap_draft_motd.cpp`, whose stand-ins are listed there).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). A scene of 1 to 5 top-level windows (the close button's id, as the C++ computes it with
//! NAMEKEY, or decoys; duplicates included; each hidden or not) and 1 to 12 messages to a random window: `GWM_CREATE`, `GWM_DESTROY`, `GBM_SELECTED`
//! with a random window of the scene as the control (so `window->winHide(!window->winIsHidden())` is toggled only when the control's id is the
//! close button's, and only after a `GWM_CREATE` has set the static), any other message code. After every message the return value, every
//! window's hidden and enabled bits and the static id are compared. EXCLUDED: `GBM_SELECTED` with a NULL control (upstream dereferences NULL).
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21091 | `GWM_CREATE` does not load the close button's id |
//! | 21092 | the window is always hidden, not toggled |
//! | 21093 | the close button's name is `MOTD.wnd:CloseMotd` |
//! | 21094 | the control is toggled, not the window |
//! | 21095 | `GWM_DESTROY` is reported ignored |
//! | 21096 | an unknown message is reported handled |
//! | 21097 | the id comparison is inverted |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences**, max ULP 0 (nothing is floating point here). No draft change was needed. Prediction held.
//! Planted bugs (of 20,000): 21091 13,873; 21092 3,760; 21093 13,873; 21094 4,315; 21095 13,819; 21096 13,619; 21097 17,456. **7 of 7 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::{gwm, win_data, GameWindowManager, GuiHost, MsgHandled};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::w3d_motd::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_motd_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Host {
    shell: Shell,
    motd: Motd,
}

impl GuiHost for Host {}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<usize> {
        None
    }
    fn create_from_script(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<WinId> {
        None
    }
    fn layout_callback(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _layout: usize, _user: Option<bool>) {}
    fn time_ms(&self) -> u32 {
        0
    }
    fn env(&self) -> ShellEnv {
        ShellEnv::default()
    }
    fn request(&mut self, _r: ShellRequest) {}
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        key_of(name) as u32
    }
}

impl MotdHost for Host {
    fn motd(&mut self) -> &mut Motd {
        &mut self.motd
    }
}

#[test]
fn motd_system_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_04);
    let mut chk = Check::new("W3DMOTD MOTDSystem");
    let ids = [key_of(b"MOTD.wnd:CloseMOTD"), key_of(b"MOTD.wnd:CloseMOTD"), 4001, 4002];
    for _ in 0..cases(20_000) {
        let nwin = 1 + g.below(5) as usize;
        let mut specs = random_specs(&mut g, nwin, &ids);
        for s in &mut specs {
            s.no_focus = false;
        }
        let (mut wm, order) = make_scene(&specs);
        let mut input = Vec::new();
        put_scene(&mut wm, &order, &specs, 0, &mut input);
        let mut host = Host { shell: Shell::new(), motd: Motd::default() };
        let steps = 1 + g.below(12);
        input.push(steps);
        let mut rust = Vec::new();
        for _ in 0..steps {
            let wi = g.below(nwin as u32);
            let msg = match g.below(5) {
                0 => gwm::CREATE,
                1 => gwm::DESTROY,
                2 | 3 => 16392,
                _ => g.below(40) + if g.below(2) == 0 { 16384 } else { 0 },
            };
            let (d1, d1c) = if msg == 16392 {
                let c = g.below(nwin as u32);
                (win_data(Some(order[c as usize])), c + 1)
            } else {
                let d = g.below(5);
                (d, d)
            };
            input.extend([wi, msg, d1c, 0]);
            let r = motd_system(&mut wm, &mut host, order[wi as usize], msg, d1, 0);
            rust.extend([100, u32::from(r == MsgHandled::Handled)]);
            put_state(&wm, &order, &mut rust);
            rust.push(host.motd.close_button_id as u32);
        }
        let mut buf = vec![0u32; 1 << 14];
        let len = unsafe { ref_ui3b_motd_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
