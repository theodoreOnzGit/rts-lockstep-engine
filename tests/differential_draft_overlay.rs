#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpyOverlay.cpp and Include/GameNetwork/GameSpyOverlay.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/game_spy_overlay.rs` against the C++ (GitHub #215, group net4).
//!
//! # Method
//!
//! `differential/wrap_draft_overlay.cpp` compiles upstream's `GameSpyOverlay.cpp` (all of it from `deleteNotificationBox`'s declaration) and `enum GSOverlayType`,
//! cut verbatim by build.rs (`drafts_net4`), on upstream's own `AsciiString.cpp`/`UnicodeString.cpp`. The window manager, the layouts, the message boxes, the
//! game text, the buddy queue, the audio and the script hook are stand-ins listed in the wrapper that log what they are asked; this file's `Host` answers
//! and logs identically. A program is 30 to 100 operations (opcodes at `ref_ov_run`) on the tracker and the nine overlays, with the buddy thread's state
//! and whether the message box window comes back null as the configuration; after every operation the log and the state (`state_words`, a test accessor
//! added to the draft) are compared word by word.
//!
//! Not representable (stated, not tested): a callback that itself puts up a message box. Upstream's `messageBoxOK` calls the stored function and only
//! then clears the stored pointer, so a new box's callback is cleared; the draft's host receives `invoke_callback` with no way back into the tracker.

use rts_lockstep_engine::drafts::network::game_spy_overlay::*;
use std::os::raw::{c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_ov_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

struct Host {
    log: Vec<u32>,
    buddy_connected: bool,
    buddy_profile: i32,
    box_null: bool,
    /// (an OK button exists, a Cancel button exists) for the latest box put up, whoever asked for it; it keeps its handlers until another replaces them
    stored: (bool, bool),
}

impl OverlayHost for Host {
    fn create_message_box(&mut self, kind: MessageBoxKind, title: &[u16], message: &[u16]) -> bool {
        self.stored = (true, kind != MessageBoxKind::Ok);
        self.log.extend_from_slice(&[20, match kind {
            MessageBoxKind::Ok => 0,
            MessageBoxKind::OkCancel => 1,
            MessageBoxKind::YesNo => 2,
        }]);
        self.log.push(title.len() as u32);
        self.log.extend(title.iter().map(|&c| c as u32));
        self.log.push(message.len() as u32);
        self.log.extend(message.iter().map(|&c| c as u32));
        !self.box_null
    }
    fn destroy_message_box(&mut self) {
        self.log.push(14);
    }
    fn message_box_bring_to_top(&mut self) {
        self.log.push(19);
    }
    fn invoke_callback(&mut self, id: CallbackId) {
        self.log.extend_from_slice(&[25, id]);
    }
    fn game_text_fetch(&self, label: &str) -> Vec<u16> {
        label.bytes().map(|b| b as u16).collect()
    }
    fn buddy_is_connected(&self) -> bool {
        self.buddy_connected
    }
    fn buddy_local_profile_id(&self) -> i32 {
        self.buddy_profile
    }
    fn play_communicator_open_sound(&mut self) {
        self.log.push(21);
    }
    fn layout_create_and_show(&mut self, overlay: usize, wnd_file: &str) {
        self.log.push(13);
        self.log.push(wnd_file.len() as u32);
        self.log.extend(wnd_file.bytes().map(|b| b as u32));
        self.log.extend_from_slice(&[10, overlay as u32, 11, overlay as u32, 0, 12, overlay as u32]);
    }
    fn layout_show(&mut self, overlay: usize) {
        self.log.extend_from_slice(&[11, overlay as u32, 0, 12, overlay as u32]);
    }
    fn layout_bring_forward(&mut self, overlay: usize) {
        self.log.extend_from_slice(&[12, overlay as u32]);
    }
    fn layout_destroy(&mut self, overlay: usize) {
        self.log.extend_from_slice(&[15, overlay as u32, 16, overlay as u32, 17, overlay as u32]);
    }
    fn layout_run_update(&mut self, overlay: usize) {
        self.log.extend_from_slice(&[18, overlay as u32]);
    }
    fn signal_options_closed(&mut self) {
        self.log.push(22);
    }
    fn delete_notification_box(&mut self) {
        self.log.push(23);
    }
    fn buddy_try_reconnect(&mut self) {
        self.log.extend_from_slice(&[24, 1]);
    }
}

fn put_units(w: &mut Vec<u32>, s: &[u16]) {
    w.push(s.len() as u32);
    w.extend(s.iter().map(|&c| c as u32));
}

fn text(g: &mut Gen) -> Vec<u16> {
    (0..g.below(8)).map(|_| (0x20 + g.below(0x300)) as u16).collect()
}

fn cb(g: &mut Gen) -> (u32, Option<CallbackId>) {
    let k = g.below(5);
    (k, if k < 4 { Some(k) } else { None })
}

fn program(g: &mut Gen) -> (Vec<u32>, Vec<u32>) {
    let mut host = Host { log: Vec::new(), buddy_connected: false, buddy_profile: 0, box_null: false, stored: (false, false) };
    let mut o = GameSpyOverlays::new();
    let nops = 30 + g.below(71);
    let mut input = vec![nops];
    for _ in 0..nops {
        let op = g.below(16);
        input.push(op);
        host.log.extend_from_slice(&[0xA5A5A5A5, op]);
        match op {
            0 => o.clear_gs_message_boxes(&mut host),
            1 | 2 | 3 => {
                let (t, m) = (text(g), text(g));
                put_units(&mut input, &t);
                put_units(&mut input, &m);
                let (a, fa) = cb(g);
                input.push(a);
                if op == 1 {
                    o.gs_message_box_ok(&mut host, &t, &m, fa);
                } else {
                    let (b, fb) = cb(g);
                    input.push(b);
                    if op == 2 {
                        o.gs_message_box_ok_cancel(&mut host, &t, &m, fa, fb);
                    } else {
                        o.gs_message_box_yes_no(&mut host, &t, &m, fa, fb);
                    }
                }
            }
            4 => o.raise_gs_message_box(&mut host),
            5 | 6 | 7 | 8 => {
                let k = g.below(9);
                input.push(k);
                let ov = ALL_OVERLAYS[k as usize];
                match op {
                    5 => o.game_spy_open_overlay(&mut host, ov),
                    6 => o.game_spy_close_overlay(&mut host, ov),
                    7 => host.log.push(o.game_spy_is_overlay_open(ov) as u32),
                    _ => o.game_spy_toggle_overlay(&mut host, ov),
                }
            }
            9 => o.game_spy_close_all_overlays(&mut host),
            10 => o.game_spy_update_overlays(&mut host),
            11 => o.re_open_player_info(),
            12 => o.check_re_open_player_info(&mut host),
            13 => {
                if host.stored.0 {
                    host.log.extend_from_slice(&[71, 0]);
                    o.message_box_ok(&mut host);
                }
            }
            14 => {
                if host.stored.1 {
                    host.log.extend_from_slice(&[71, 1]);
                    o.message_box_cancel(&mut host);
                }
            }
            _ => {
                host.buddy_connected = g.below(2) == 0;
                host.buddy_profile = g.below(3) as i32;
                host.box_null = g.below(6) == 0;
                input.extend_from_slice(&[host.buddy_connected as u32, host.buddy_profile as u32, host.box_null as u32]);
            }
        }
        host.log.extend(o.state_words());
    }
    (input, host.log)
}

#[test]
fn game_spy_overlay_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("OV_CASES", 20_000);
    let mut g = Gen::new(0x0e01_0001);
    let mut chk = Check::new("game spy overlay programs");
    for i in 0..n {
        let (input, r) = program(&mut g);
        let mut c = vec![0u32; 400_000];
        let k = unsafe { ref_ov_run(input.as_ptr(), input.len() as c_int, c.as_mut_ptr(), c.len() as c_int) };
        assert!(k >= 0, "the program dump does not fit");
        c.truncate(k as usize);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let marker = r[..k].iter().rposition(|&x| x == 0xA5A5A5A5).unwrap_or(0);
            let opn = r[..k].iter().filter(|&&x| x == 0xA5A5A5A5).count();
            println!(
                "case {i}: operation {opn} (opcode {}), first difference at word {k} ({} into it; rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}",
                r[marker + 1],
                k - marker,
                r.len(),
                c.len(),
                &r[k.saturating_sub(6)..(k + 8).min(r.len())],
                &c[k.saturating_sub(6)..(k + 8).min(c.len())]
            );
        }
        chk.eq(r == c, true, || format!("overlay program {i}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
