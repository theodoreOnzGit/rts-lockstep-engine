// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/popup_communicator.rs` (`popup_communicator_init`, `_shutdown`, `_update`, `_input`,
//! `_system`) against upstream `PopupCommunicator.cpp`, cut verbatim by build.rs (`drafts_ui3b`; `differential/wrap_draft_popup_communicator.cpp`, whose stand-ins, the
//! window manager's functions cut verbatim from GameWindowManager.cpp / GameWindow.cpp among them, and exclusions are listed there).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). A tree of 2 to 7 windows (ids: the popup's parent, its OK button, decoys; duplicates included; hidden, enabled,
//! no-focus bits random; a third of them have the popup's own system callback, the rest the test callback that wants the focus), all in one layout; 1 to 8 steps of
//! `PopupCommunicatorInit`, `PopupCommunicatorInput` (Escape with and without the key-up state, other keys, other messages) and `PopupCommunicatorSystem` (create, destroy,
//! `GWM_INPUT_FOCUS`, `GBM_SELECTED` with a random window of the tree, others). The Rust draft runs on the verified `GameWindowManager` (its focus, modal stack,
//! `win_send_system_msg` and layouts), the C++ on the ui3b mini manager built from upstream's own `winSetFocus`, `winSendSystemMsg`, `winSetModal`, `winGetWindowFromId` and
//! `winHide`. After every step the focus messages the windows were sent, the result, every window's status word, the focus, the modal stack and the statics are compared.
//! A sequence ends after the layout is destroyed. EXCLUDED (upstream dereferences NULL): Escape-up before an init that found the OK button, `GBM_SELECTED` to a window with no
//! layout or with a NULL control.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches. The likeliest miss: the Rust draft keeps the `Option`al `button_ok` where upstream's `(WindowMsgData)buttonOk` is the pointer (0 for NULL).
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21181 | the OK button is looked up from the head of the window list, not from the parent |
//! | 21182 | the popup does not go modal |
//! | 21183 | the OK button goes modal, not the parent |
//! | 21184 | Escape sends the simulated click on the key-down (the test of `KEY_STATE_UP` inverted) |
//! | 21185 | Escape is let fall through when the key is not up |
//! | 21186 | the simulated click names the popup as the control, not the OK button |
//! | 21187 | the popup says it does not want the keyboard focus |
//! | 21188 | any button destroys the layout |
//! | 21189 | `GWM_CREATE` is reported ignored |
//! | 21190 | an unknown message is reported handled |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences** after the fixes noted here, max ULP 0 (nothing is floating point in the comparison, which is of integer words). The first run did not finish: it crashed (SIGSEGV) in the C++, a harness input error (not a draft fault): `GWM_INPUT_FOCUS` was sent with its second datum as a plain number where upstream writes `*(Bool *)mData2`; the wrapper now passes the address of a Bool and the result carries it. Then **0 mismatches**. No draft change was needed.
//! Planted bugs (mismatching sequences of 20,000): 21181 5,830; 21182 8,055; 21183 10,385; 21184 1,160; 21185 11,561; 21186 323; 21187 4,228; 21188 7,669; 21189 5,072; 21190 4,595. **10 of 10 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::{gwm, win_data, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::popup_communicator::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_pc_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Host {
    log: Vec<u32>,
    shell: Shell,
    pc: PopupCommunicator,
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == b"popup" {
            popup_communicator_system(wm, self, win, msg, d1, d2)
        } else {
            focus_system(&mut self.log, wm, win, msg, d1)
        }
    }
}

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

impl PopupCommunicatorHost for Host {
    fn popup_communicator(&mut self) -> &mut PopupCommunicator {
        &mut self.pc
    }
}

#[test]
fn popup_communicator_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_09);
    let mut chk = Check::new("PopupCommunicator callbacks");
    let parent_key = key_of(b"PopupCommunicator.wnd:PopupCommunicator");
    let ok_key = key_of(b"PopupCommunicator.wnd:ButtonOk");
    let ids = [parent_key, ok_key, ok_key, 7001, 7002];
    for _ in 0..cases(20_000) {
        let n = 2 + g.below(6) as usize;
        let mut ts = random_tree(&mut g, n, &ids, 3);
        if g.below(3) != 0 {
            ts[0].spec.id = parent_key;
            ts[g.below(n as u32) as usize].spec.id = ok_key;
        }
        let (mut wm, scene) = make_tree_scene(&ts, b"popup");
        let members: Vec<usize> = (0..n).collect();
        make_layout(&mut wm, &members);
        let mut input = scene;
        input.push(n as u32);
        input.extend(members.iter().map(|&m| m as u32));
        let mut host = Host { log: Vec::new(), shell: Shell::new(), pc: PopupCommunicator::default() };
        let steps = 1 + g.below(8);
        let mut step_words: Vec<u32> = Vec::new();
        let mut nsteps = 0u32;
        let mut rust = Vec::new();
        // the reference model of what the C++ finds, for the exclusions (independent of the draft)
        let mut init_done = false;
        let mut ok_found = false;
        let mut done = false;
        for _ in 0..steps {
            if done {
                break;
            }
            nsteps += 1;
            let kind = g.below(5);
            let (kind, a, b, c, d) = match kind {
                0 => (0, 0, 0, 0, 0),
                1 | 2 => {
                    let key = match g.below(3) {
                        0 | 1 => 1u32,
                        _ => g.below(256),
                    };
                    let mut state = if g.below(2) == 0 { 1u32 } else { 2 };
                    state |= if g.below(4) == 0 { 0x100 } else { 0 };
                    if g.below(5) == 0 {
                        state = g.below(256);
                    }
                    let msg = if g.below(8) == 0 { g.below(30) } else { gwm::CHAR };
                    let k = if g.below(8) == 0 { key | g.below(0x1000) << 8 } else { key };
                    (1, g.below(n as u32), msg, k, state)
                }
                _ => {
                    let msg = match g.below(6) {
                        0 => gwm::CREATE,
                        1 => gwm::DESTROY,
                        2 => gwm::INPUT_FOCUS,
                        3 | 4 => 16392,
                        _ => g.below(40),
                    };
                    let w = g.below(n as u32);
                    let c = if msg == 16392 {
                        1 + g.below(n as u32)
                    } else if msg == gwm::INPUT_FOCUS {
                        g.below(3)
                    } else {
                        g.below(5)
                    };
                    (2, w, msg, c, g.below(2))
                }
            };
            // ESC with the key-up state sends a click on the OK button: not before an init that found it
            let esc_up = kind == 1 && b == gwm::CHAR && (c & 0xFF) == 1 && (d & 0xFF) & 1 != 0;
            let (kind, a, b, c, d) = if esc_up && !(init_done && ok_found) { (kind, a, b, c, d & !1) } else { (kind, a, b, c, d) };
            let esc_up = esc_up && init_done && ok_found;
            step_words.extend([kind, a, b, c, d]);
            host.log.clear();
            let w = a as usize;
            let rr: i32 = match kind {
                0 => {
                    popup_communicator_init(&mut wm, &mut host, 0);
                    -1
                }
                1 => i32::from(popup_communicator_input(&mut wm, &mut host, w, b, c, d) == MsgHandled::Handled),
                _ => {
                    let d1 = if b == 16392 { win_data(Some(c as usize - 1)) } else { c };
                    wm.focus_reply = d != 0;
                    let r = i32::from(popup_communicator_system(&mut wm, &mut host, w, b, d1, d) == MsgHandled::Handled);
                    if b == gwm::INPUT_FOCUS {
                        r * 2 + i32::from(wm.focus_reply)
                    } else {
                        r
                    }
                }
            };
            // the C++ comparison: the model of what upstream did
            if kind == 0 {
                init_done = true;
                let p = wm.win_get_window_from_id(None, parent_key);
                ok_found = wm.win_get_window_from_id(p, ok_key).is_some();
                // (the lookup is read after the draft's own: the verified manager has no side effect here)
            }
            rust.extend(host.log.iter().copied());
            rust.extend([100, rr as u32]);
            put_state_tree(&wm, n, &mut rust);
            let pc = &host.pc;
            rust.extend([pc.button_ok_id as u32, pc.parent.map_or(0, |p| p as u32 + 1), pc.button_ok.map_or(0, |p| p as u32 + 1)]);
            // a click on the OK button destroys the layout (the model of the C++, not the draft's state): the sequence ends there
            if kind == 2 && b == 16392 && init_done && ts[c as usize - 1].spec.id == ok_key {
                done = true;
            }
            if kind == 1 && esc_up && init_done && ok_found && ts[a as usize].unit_system {
                done = true;
            }
        }
        input.push(nsteps);
        input.extend(step_words);
        trace(&input);
        let mut buf = vec![0u32; 1 << 15];
        let len = unsafe { ref_ui3b_pc_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
