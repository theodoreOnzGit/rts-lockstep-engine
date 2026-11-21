// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/control_bar_beacon.rs` (`populate_beacon`,
//! `update_context_beacon`, `beacon_window_input`) against upstream `ControlBarBeacon.cpp`'s `ControlBar::populateBeacon`,
//! `updateContextBeacon` and `BeaconWindowInput`, cut verbatim by build.rs (`drafts_ui3b`; `differential/wrap_draft_beacon.cpp`, whose
//! stand-ins are listed there).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). A scene of 1 to 7 top-level windows whose ids are the three beacon controls'
//! name keys or decoys (duplicates and missing ones included, so each `winGetWindowFromId` finds the first or nothing), each hidden or not,
//! enabled or not, willing to take the focus or not, and a random initial focus; 1 to 3 beacons (locally controlled or not, with a caption of
//! 0 to 5 units, 0 as the first unit included); 1 to 8 steps of `populateBeacon(beacon)` and `BeaconWindowInput(msg, d1)` (Escape, other
//! keys, key codes with bits above the low byte, other messages). The Rust draft runs on the verified `GameWindowManager` with the same
//! windows (system callback: the host's, which logs and accepts `GWM_INPUT_FOCUS`), the C++ on the stand-in windows. After every step the
//! calls logged (the portrait, the text entry's text with the window's hidden flag at that moment, the focus messages the window manager
//! sends, the deselect and its flag), the result, every window's hidden and enabled bits and the focused window are compared.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches: the draft is line for line; `updateContextBeacon` is empty on both sides.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21071 | a locally controlled beacon leaves the text entry hidden (`winHide(TRUE)`) |
//! | 21072 | the caption is set before the entry is shown |
//! | 21073 | the focus goes to the label (the static text), not the entry |
//! | 21074 | a beacon that is not ours hides only the entry, not the label and the button |
//! | 21075 | the entry's and the label's ids are looked up swapped (the first version swapped the label and the clear button, which are treated alike: an equivalent mutant, replaced) |
//! | 21076 | `BeaconWindowInput` acts on Escape for any message |
//! | 21077 | the deselect is made with `post = FALSE` |
//! | 21078 | the local test is inverted |
//! | 21079 | the clear button is left as it was when the beacon is ours |
//! | 21080 | a key other than Escape is handled |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences**, max ULP 0 (nothing is floating point here). No draft change was needed. Prediction held.
//! Planted bugs (of 20,000): 21071 6,477; 21072 2,790; 21073 6,121; 21074 6,504; 21075 7,482 (its first version, swapping the label and the clear button, showed 0: both are treated alike, an equivalent mutant, replaced); 21076 8,800; 21077 9,021; 21078 14,011; 21079 2,871; 21080 11,704. **10 of 10 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::window_manager::{gwm, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::control_bar_beacon::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_beacon_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Beacon {
    local: bool,
    caption: Vec<u16>,
}

struct Host {
    log: Vec<u32>,
    beacons: Vec<Beacon>,
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, _func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
        focus_system(&mut self.log, wm, win, msg, d1)
    }
}

impl BeaconHost for Host {
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        key_of(name)
    }
    fn set_portrait_by_object(&mut self, beacon: u32) {
        self.log.push(1);
        self.log.push(beacon);
    }
    fn is_locally_controlled(&self, beacon: u32) -> bool {
        self.beacons[beacon as usize].local
    }
    fn drawable_caption_text(&self, beacon: u32) -> Vec<u16> {
        self.beacons[beacon as usize].caption.clone()
    }
    fn gadget_text_entry_set_text(&mut self, wm: &mut GameWindowManager, w: WinId, text: &[u16]) {
        self.log.push(3);
        self.log.push(wm.tree.windows[w].id as u32);
        self.log.push(u32::from(wm.is_hidden(Some(w))));
        self.log.push(text.len() as u32);
        self.log.extend(text.iter().map(|&u| u32::from(u)));
    }
    fn deselect_all_drawables(&mut self, post_msg: bool) {
        self.log.push(4);
        self.log.push(u32::from(post_msg));
    }
}

#[test]
fn beacon_context_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_02);
    let mut chk = Check::new("ControlBar beacon (populate, input)");
    let ids = [key_of(b"ControlBar.wnd:EditBeaconText"), key_of(b"ControlBar.wnd:StaticTextBeaconLabel"), key_of(b"ControlBar.wnd:ButtonClearBeaconText"), 7001, 7002];
    for _ in 0..cases(20_000) {
        let nwin = 1 + g.below(7) as usize;
        let mut specs = random_specs(&mut g, nwin, &ids);
        // most scenes have all three
        if nwin >= 3 && g.below(4) != 0 {
            for k in 0..3 {
                specs[k].id = ids[k];
            }
            for i in (1..nwin).rev() {
                let j = g.below(i as u32 + 1) as usize;
                specs.swap(i, j);
            }
        }
        let (mut wm, order) = make_scene(&specs);
        let mut input = Vec::new();
        let focus = if g.below(3) == 0 { 1 + g.below(nwin as u32) } else { 0 };
        put_scene(&mut wm, &order, &specs, focus, &mut input);
        let nb = 1 + g.below(3) as usize;
        let mut host = Host { log: Vec::new(), beacons: Vec::new() };
        input.push(nb as u32);
        for _ in 0..nb {
            let len = g.below(6) as usize;
            let caption: Vec<u16> = (0..len).map(|_| if g.below(4) == 0 { 0 } else { g.below(0x10000) as u16 }).collect();
            let local = g.below(2) == 0;
            input.extend([u32::from(local), len as u32]);
            input.extend(caption.iter().map(|&u| u32::from(u)));
            host.beacons.push(Beacon { local, caption });
        }
        let steps = 1 + g.below(8);
        input.push(steps);
        let mut rust = Vec::new();
        for _ in 0..steps {
            let kind = g.below(3);
            host.log.clear();
            let (a, b, res);
            if kind == 0 {
                a = g.below(nb as u32);
                b = 0;
                populate_beacon(&mut wm, &mut host, a);
                res = 2;
                input.extend([0, a, b]);
            } else {
                a = match g.below(4) {
                    0 | 1 => gwm::CHAR,
                    2 => g.below(31),
                    _ => g.u32(),
                };
                b = match g.below(5) {
                    0 | 1 => 1,
                    2 => 0x100 | 1,
                    3 => g.below(256),
                    _ => g.u32(),
                };
                input.extend([1, a, b]);
                let r = beacon_window_input(&mut wm, &mut host, order[0], a, b, 0);
                res = u32::from(r == MsgHandled::Handled);
            }
            rust.extend(host.log.iter().copied());
            rust.extend([100, res]);
            put_state(&wm, &order, &mut rust);
        }
        update_context_beacon();
        let mut buf = vec![0u32; 1 << 14];
        let len = unsafe { ref_ui3b_beacon_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
