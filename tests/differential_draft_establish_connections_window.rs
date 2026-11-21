// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/establish_connections_window.rs` (`show_establish_connections_window`,
//! `hide_establish_connections_window`, `establish_connections_control_input`, `establish_connections_control_system`, the private
//! `showGameSpy*UnderlyingGUIElements` and `InitEstablishConnectionsDialog`, and the file's tables) against upstream `EstablishConnectionsWindow.cpp`, cut verbatim by build.rs
//! (`drafts_ui3b`; `differential/wrap_draft_establish_connections_window.cpp`, whose stand-ins, the window manager's functions cut verbatim from GameWindowManager.cpp /
//! GameWindow.cpp among them, and exclusions are listed there).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). A tree of 1 to 6 windows (ids: the quit button's, decoys; duplicates; random hidden, enabled, no-focus bits), a layout made
//! of a random subset on each `winCreateLayout`, a QM game or not; 1 to 10 steps of `ShowEstablishConnectionsWindow`, `HideEstablishConnectionsWindow`, the input callback, the system
//! callback (`GBM_SELECTED` with a random window, other messages) and a change of the QM flag. Every call on the stand-ins (the layout made, the underlying GUI elements shown or hidden
//! with the layout file, the parent name and each gadget name, the quit) is logged on both sides; after every step the result, every window's status word, the focus, the modal stack,
//! the quit button's id and window and whether the layout exists are compared. The Rust draft runs on the verified `GameWindowManager` with real layouts. The draft's tables
//! (`gadgetsToHide`, `perPlayerGadgetsToHide`, the QM tables) are compared through the logged names (the C++ ends them with NULL, the draft with the slice's end).
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21221 | the window is shown with its layout hidden |
//! | 21222 | the QM test is inverted at show |
//! | 21223 | the underlying GUI is shown (not hidden) when the window shows |
//! | 21224 | the underlying GUI is hidden (not shown) when the window hides |
//! | 21225 | hiding does not destroy the layout's windows |
//! | 21226 | showing makes the layout again every time |
//! | 21227 | any button aborts the game |
//! | 21228 | the system callback ignores messages other than `GBM_SELECTED` |
//! | 21229 | the quit button's name is `ButtonQuit1` |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences** after the fixes noted here, max ULP 0 (nothing is floating point in the comparison, which is of integer words). The first run showed 7,862 mismatches, then 414, then 235, then 0, all of them in the harness and none in the draft: the C++ stand-in logged the layout's destroyWindows / deleteInstance, which the draft leaves to its host and the Rust side cannot log (no longer logged: the state is compared); the stand-in `winDestroy` stopped after the first child (its loop read `m_next` after unlinking); and it kept a destroyed child's parent reference (upstream's `unlinkChildWindow` clears it). No draft change was needed.
//! Planted bugs (of 20,000): 21221 13,111; 21222 14,319; 21223 14,319; 21224 7,862; 21225 7,174; 21226 6,111; 21227 7,773; 21228 9,695; 21229 14,319. **9 of 9 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::{gwm, win_data, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::establish_connections_window::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_ecw_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Host {
    log: Vec<u32>,
    shell: Shell,
    ecw: EstablishConnectionsWindow,
    members: Vec<usize>,
    qm: bool,
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, _func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
        focus_system(&mut self.log, wm, win, msg, d1)
    }
}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        assert_eq!(filename, b"Menus/EstablishConnectionsScreen.wnd");
        self.log.push(912);
        Some(make_layout(wm, &self.members))
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

fn h(s: &str) -> u32 {
    fnv(s.as_bytes())
}

impl EstablishConnectionsWindowHost for Host {
    fn establish_connections_window(&mut self) -> &mut EstablishConnectionsWindow {
        &mut self.ecw
    }
    fn is_qm_game(&self) -> bool {
        self.qm
    }
    fn show_underlying_gui_elements(&mut self, _wm: &mut GameWindowManager, show: bool, layout_filename: &str, parent_name: &str, gadgets_to_hide: &[&str], per_player_gadgets_to_hide: &[&str]) {
        self.log.extend([10, u32::from(show), h(layout_filename), h(parent_name), gadgets_to_hide.len() as u32]);
        self.log.extend(gadgets_to_hide.iter().map(|s| h(s)));
        self.log.push(per_player_gadgets_to_hide.len() as u32);
        self.log.extend(per_player_gadgets_to_hide.iter().map(|s| h(s)));
    }
    fn establish_connections_abort_game(&mut self) {
        self.log.push(30);
    }
}

#[test]
fn establish_connections_window_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_0B);
    let mut chk = Check::new("EstablishConnectionsWindow");
    let quit_key = key_of(b"EstablishConnectionsScreen.wnd:ButtonQuit");
    let ids = [quit_key, quit_key, 7001, 7002];
    for _ in 0..cases(20_000) {
        let n = 1 + g.below(6) as usize;
        let ts = random_tree(&mut g, n, &ids, 0);
        let (mut wm, scene) = make_tree_scene(&ts, b"unused");
        let members: Vec<usize> = (0..n).filter(|_| g.below(3) != 0).collect();
        let mut input = scene;
        input.push(members.len() as u32);
        input.extend(members.iter().map(|&m| m as u32));
        let qm = g.below(2) == 0;
        input.push(u32::from(qm));
        let mut host = Host { log: Vec::new(), shell: Shell::new(), ecw: EstablishConnectionsWindow::default(), members, qm };
        let steps = 1 + g.below(10);
        input.push(steps);
        let mut rust = Vec::new();
        for _ in 0..steps {
            let (kind, a, b, c) = match g.below(8) {
                0 | 1 => (0, 0, 0, 0),
                2 | 3 => (1, 0, 0, 0),
                4 => (2, 0, g.below(40), g.u32()),
                5 | 6 => {
                    let msg = if g.below(2) == 0 { 16392 } else { g.below(40) };
                    let c = if msg == 16392 { 1 + g.below(n as u32) } else { g.below(5) };
                    (3, g.below(n as u32), msg, c)
                }
                _ => (4, g.below(2), 0, 0),
            };
            input.extend([kind, a, b, c]);
            host.log.clear();
            let mut res: i32 = -1;
            match kind {
                0 => show_establish_connections_window(&mut wm, &mut host),
                1 => hide_establish_connections_window(&mut wm, &mut host),
                2 => res = i32::from(establish_connections_control_input(0, b, c, 0) == MsgHandled::Handled),
                3 => {
                    let d1 = if b == 16392 { win_data(Some(c as usize - 1)) } else { c };
                    res = i32::from(establish_connections_control_system(&mut wm, &mut host, a as usize, b, d1, 0) == MsgHandled::Handled);
                }
                _ => host.qm = a != 0,
            }
            let _ = gwm::CHAR;
            rust.extend(host.log.iter().copied());
            rust.extend([100, res as u32]);
            put_state_tree(&wm, n, &mut rust);
            rust.extend([host.ecw.button_quit_id as u32, host.ecw.button_quit_window.map_or(0, |p| p as u32 + 1), u32::from(host.ecw.establish_connections_layout.is_some())]);
        }
        trace(&input);
        let mut buf = vec![0u32; 1 << 15];
        let len = unsafe { ref_ui3b_ecw_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
