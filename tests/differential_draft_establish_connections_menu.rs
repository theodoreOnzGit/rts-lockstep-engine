// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/establish_connections_menu.rs` (`init_menu`, `end_menu`, `abort_game`, `set_player_name`,
//! `set_player_status`, the three name tables) against upstream `EstablishConnectionsMenu.cpp` (all of its members) and NAT.h's `NATConnectionState`, cut verbatim by build.rs
//! (`drafts_ui3b`; `differential/wrap_draft_establish_connections_menu.cpp`, whose stand-ins and exclusion are listed there).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). A flat scene of 1 to 14 windows whose ids are the menu's player name and status controls (slots 0 to 6) or decoys
//! (duplicates and missing controls included); 1 to 10 steps of `initMenu`, `endMenu`, `setPlayerName(slot, name of 0 to 5 units)` and `setPlayerStatus(slot, state)` (every state of the
//! draft, the seven slots). The text a control is given arrives at the window's system callback (`GGM_SET_LABEL`) on the Rust side, logged with the control's id; the C++ logs it from
//! `GadgetStaticTextSetText`. `TheGameText->fetch` answers a function of the label, so a wrong label shows. The names in the tables are compared through the window ids they find.
//! The draft's `NetgearDelay` variant (commented out upstream) is sent to the C++ as `NATCONNECTIONSTATE_NOSTATE`: both take the unknown-state label. EXCLUDED: slot 7.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21171 | `Done` and `Failed` give each other's labels |
//! | 21172 | `WaitingForResponse` gives the mangled-port label |
//! | 21173 | the name goes to the status control |
//! | 21174 | the commented-out `NetgearDelay` branch is back (`GUI:NetgearDelay`) |
//! | 21175 | the name goes to the next slot's control |
//! | 21176 | `initMenu` hides the window |
//! | 21177 | `endMenu` does nothing |
//! | 21178 | `WaitingToBegin` gives the unknown label |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences** after the fixes noted here, max ULP 0 (nothing is floating point in the comparison, which is of integer words). No draft change was needed. Prediction held.
//! Planted bugs (of 20,000): 21171 3,730; 21172 1,973; 21173 12,469; 21174 2,032; 21175 11,198; 21176 9,742; 21177 9,714; 21178 2,016. **8 of 8 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::window_manager::{ggm, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::establish_connections_menu::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_ecm_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Host {
    log: Vec<u32>,
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, _func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
        if msg == ggm::SET_LABEL {
            self.log.extend([3, wm.tree.windows[win].id as u32, wm.msg_text.len() as u32]);
            self.log.extend(wm.msg_text.iter().map(|&u| u32::from(u)));
            return MsgHandled::Handled;
        }
        focus_system(&mut self.log, wm, win, msg, d1)
    }
}

impl EstablishConnectionsHost for Host {
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        key_of(name)
    }
    fn show_establish_connections_window(&mut self, _wm: &mut GameWindowManager) {
        self.log.push(1);
    }
    fn hide_establish_connections_window(&mut self, _wm: &mut GameWindowManager) {
        self.log.push(2);
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        vec![(fnv(label) & 0xFFFF) as u16, label.len() as u16]
    }
}

const STATES: [NatConnectionState; 7] = [
    NatConnectionState::NetgearDelay,
    NatConnectionState::WaitingToBegin,
    NatConnectionState::WaitingForManglerResponse,
    NatConnectionState::WaitingForMangledPort,
    NatConnectionState::WaitingForResponse,
    NatConnectionState::Done,
    NatConnectionState::Failed,
];

#[test]
fn establish_connections_menu_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_0C);
    let mut chk = Check::new("EstablishConnectionsMenu");
    let mut ids: Vec<i32> = Vec::new();
    for k in 0..7 {
        ids.push(key_of(PLAYER_NAME_CONTROL_NAMES[k].as_bytes()));
        ids.push(key_of(PLAYER_STATUS_CONTROL_NAMES[k].as_bytes()));
    }
    ids.extend([7001, 7002]);
    for _ in 0..cases(20_000) {
        let n = 1 + g.below(14) as usize;
        let ts = random_tree(&mut g, n, &ids, 0);
        let ts: Vec<TreeSpec> = ts.into_iter().map(|t| TreeSpec { parent: None, ..t }).collect();
        let (mut wm, scene) = make_tree_scene(&ts, b"unused");
        let mut input = scene;
        let mut host = Host { log: Vec::new() };
        let steps = 1 + g.below(10);
        input.push(steps);
        let mut rust = Vec::new();
        for _ in 0..steps {
            host.log.clear();
            match g.below(8) {
                0 => {
                    input.extend([0, 0, 0]);
                    init_menu(&mut wm, &mut host);
                }
                1 => {
                    input.extend([1, 0, 0]);
                    end_menu(&mut wm, &mut host);
                }
                2 | 3 | 4 => {
                    let slot = g.below(7);
                    let len = g.below(6);
                    let name: Vec<u16> = (0..len).map(|_| g.below(0x10000) as u16).collect();
                    input.extend([2, slot, len]);
                    input.extend(name.iter().map(|&u| u32::from(u)));
                    set_player_name(&mut wm, &mut host, slot as usize, &name);
                }
                _ => {
                    let slot = g.below(7);
                    let st = g.below(7);
                    input.extend([3, slot, st]);
                    set_player_status(&mut wm, &mut host, slot as usize, STATES[st as usize]);
                }
            }
            abort_game();
            rust.extend(host.log.iter().copied());
            rust.push(100);
            put_state_tree(&wm, n, &mut rust);
        }
        let mut buf = vec![0u32; 1 << 15];
        let len = unsafe { ref_ui3b_ecm_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
