#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32OSDisplay.cpp
//   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32GameEngine.cpp (update)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::win32_os_display` against `Win32OSDisplay.cpp` and `Win32GameEngine::update` (#215, group lc2).
//!
//! # Method
//!
//! `RTSFlagsToOSFlags`, `OSDisplayWarningBox` and `Win32GameEngine::update` are cut verbatim by build.rs into
//! `differential/wrap_draft_os_display.cpp` (stand-ins listed there: Windows' `MB_*` values, recording `MessageBox`, `Sleep`, `IsIconic`,
//! `serviceWindowsOS`, `TheLAN`, the audio manager). Three comparisons:
//!
//! 1. **Flags**: `RTSFlagsToOSFlags` for every `buttonFlags` in 0..=0xff and `otherFlags` in 0..=0xff (65,536 pairs), and 100,000 random 32-bit pairs.
//! 2. **Warning box** (20,000 cases): with and without `TheGameText`, the wide and the narrow box (the draft's host chooses; both are compared with the
//!    same expectation, the fetched text), random labels, random flags and the box's answer (`IDOK`, `IDCANCEL`, 0, -1, 7): the returned button, the flags
//!    handed to the box, both texts, and whether the box was called.
//! 3. **Engine update** (20,000 scenarios): window handle present or not, LAN present or not, active or not, the quitting / Internet-game / LAN-game
//!    flags and a script of `IsIconic` answers; compared: the ordered log of base update, `Sleep(5)`, `serviceWindowsOS`, the LAN's
//!    `setIsActive(active)` and `update`, the audio volume reset with `0x10`, and the number of `IsIconic` calls.
//!
//! The boxes, the message pump, the sleep and the audio manager are the host's and are not tested; the draft's ordering and short-circuit choices are.
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches** (the prediction held, no draft change): 165,536 flag pairs, 20,000 warning boxes, 20,000 engine updates. Planted bugs, 11 of 11 caught (flags / box / engine):
//! 16941 CANCEL maps to MB_OK 82,631 / 8,032; 16942 the error icon maps to the information icon 41,162 / 4,035; 16943 any answer but cancel is OK 0 / 6,417; 16944 prompt and message swapped 0 / 12,849;
//! 16945 IDOK reported as cancel 0 / 16,072; 16946 no `TheGameText` check 0 / 3,928; 16947 task-modal maps to system-modal 82,833 / 8,149; 16948 `Sleep(50)` 0 / 0 / 4,651; 16949 never breaks out for a
//! multiplayer game 1,592; 16950 audio re-woken when not iconic 11,501; 16951 no final `serviceWindowsOS` 20,000.

mod support;

use rts_lockstep_engine::drafts::logic_common::win32_os_display::*;
use std::cell::Cell;
use std::ffi::CString;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc2_osd_flags(button: u32, other: u32) -> u32;
    fn ref_lc2_osd_box(has_text: i32, unicode: i32, result: i32, p: *const i8, m: *const i8, button: u32, other: u32, out: *mut u32) -> i32;
    fn ref_lc2_ge_update(hwnd: i32, lan: i32, active: i32, quitting: i32, internet: i32, lan_game: i32, answers: *const i32, n: i32, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish_all(checks: Vec<Check>) {
    if planted() {
        for c in checks {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    } else {
        for c in checks {
            c.finish();
        }
    }
}

struct BoxHost {
    has_text: bool,
    result: i32,
    flags: u32,
    mesg: Vec<u16>,
    prompt: Vec<u16>,
    called: bool,
}

impl OsDisplayHost for BoxHost {
    fn has_game_text(&self) -> bool {
        self.has_text
    }
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        let mut v: Vec<u16> = vec![b'#' as u16];
        v.extend(label.iter().map(|&b| b as u16));
        v
    }
    fn message_box(&mut self, message: &[u16], prompt: &[u16], flags: u32) -> i32 {
        self.mesg = message.to_vec();
        self.prompt = prompt.to_vec();
        self.flags = flags;
        self.called = true;
        self.result
    }
}

fn text_sig(v: &[u16]) -> [u32; 2] {
    let mut s: u32 = 0;
    for (i, &c) in v.iter().enumerate() {
        s = s.wrapping_add((c as u32).wrapping_mul(i as u32 + 1));
    }
    [v.len() as u32, s]
}

struct EngineHost {
    hwnd: bool,
    lan: bool,
    active: bool,
    quitting: bool,
    internet: bool,
    lan_game: bool,
    answers: Vec<i32>,
    idx: Cell<usize>,
    calls: Cell<i32>,
    log: Vec<i32>,
}

impl Win32GameEngineHost for EngineHost {
    fn base_update(&mut self) {
        self.log.push(1);
    }
    fn is_iconic(&self) -> bool {
        // `ApplicationHWnd && ::IsIconic(ApplicationHWnd)`: no call without a window; the answers are the script, then false
        if !self.hwnd {
            return false;
        }
        self.calls.set(self.calls.get() + 1);
        let i = self.idx.get();
        if i < self.answers.len() {
            self.idx.set(i + 1);
            self.answers[i] != 0
        } else {
            false
        }
    }
    fn sleep_ms(&mut self, ms: u32) {
        self.log.extend([2, ms as i32]);
    }
    fn service_windows_os(&mut self) {
        self.log.push(3);
    }
    fn update_lan_if_present(&mut self) {
        if self.lan {
            self.log.extend([4, self.active as i32, 5]);
        }
    }
    fn must_keep_running(&self) -> bool {
        self.quitting || self.internet || self.lan_game
    }
    fn rewake_audio(&mut self) {
        self.log.extend([6, 0x10]);
    }
}

#[test]
fn os_display_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_05D);
    let mut cf = Check::new("RTSFlagsToOSFlags");
    let mut cb = Check::new("OSDisplayWarningBox");
    let mut ce = Check::new("Win32GameEngine::update");

    for b in 0..=0xffu32 {
        for o in 0..=0xffu32 {
            cf.eq(rts_flags_to_os_flags(b, o), unsafe { ref_lc2_osd_flags(b, o) }, || format!("button {b:#x} other {o:#x}"));
        }
    }
    for _ in 0..100_000 {
        let (b, o) = (g.u32(), g.u32());
        cf.eq(rts_flags_to_os_flags(b, o), unsafe { ref_lc2_osd_flags(b, o) }, || format!("button {b:#x} other {o:#x}"));
    }

    let labels: [&[u8]; 5] = [b"", b"GUI:Ok", b"x", b"A longer label \xe9\xff", b"MSG"];
    for case in 0..20_000 {
        let has_text = g.below(5) != 0;
        let unicode = g.below(2) as i32;
        let result = [1, 2, 0, -1, 7][g.below(5) as usize];
        let p = labels[g.below(5) as usize];
        let m = labels[g.below(5) as usize];
        let (b, o) = if g.below(2) == 0 { (g.below(4), g.below(0x40)) } else { (g.u32(), g.u32()) };
        let (cp, cm) = (CString::new(p).unwrap(), CString::new(m).unwrap());
        let mut cpp = [0u32; 7];
        unsafe { ref_lc2_osd_box(has_text as i32, unicode, result, cp.as_ptr(), cm.as_ptr(), b, o, cpp.as_mut_ptr()) };
        let mut host = BoxHost { has_text, result, flags: 0xFFFF_FFFF, mesg: vec![], prompt: vec![], called: false };
        let r = os_display_warning_box(&mut host, p, m, b, o);
        let mut rust = vec![r, host.flags];
        rust.extend(text_sig(&host.mesg));
        rust.extend(text_sig(&host.prompt));
        rust.push(host.called as u32);
        cb.eq(rust, cpp.to_vec(), || format!("case {case}: text {has_text} unicode {unicode} result {result} p {p:?} m {m:?} flags {b:#x} {o:#x}"));
    }

    for case in 0..20_000 {
        let (hwnd, lan, active) = (g.below(4) != 0, g.below(2) == 0, g.below(2) == 0);
        let (quitting, internet, lan_game) = (g.below(8) == 0, g.below(8) == 0, g.below(8) == 0);
        let answers: Vec<i32> = (0..g.below(7)).map(|_| (g.below(3) != 0) as i32).collect();
        let mut cpp = vec![0i32; 64];
        let len = unsafe { ref_lc2_ge_update(hwnd as i32, lan as i32, active as i32, quitting as i32, internet as i32, lan_game as i32, answers.as_ptr(), answers.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);
        let mut host = EngineHost { hwnd, lan, active, quitting, internet, lan_game, answers: answers.clone(), idx: Cell::new(0), calls: Cell::new(0), log: vec![] };
        win32_game_engine_update(&mut host);
        let mut rust = host.log.clone();
        rust.push(host.calls.get());
        ce.eq(rust, cpp, || format!("case {case}: hwnd {hwnd} lan {lan} active {active} quit {quitting} net {internet} lan_game {lan_game} answers {answers:?}"));
    }
    finish_all(vec![cf, cb, ce]);
}
