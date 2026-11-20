#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/FileTransfer.cpp (doFileTransfer, DoAnyMapTransfers)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::file_transfer`'s map transfer (`do_any_map_transfers`, `do_file_transfer`) against upstream's (#215).
//!
//! # Method
//!
//! `doFileTransfer`, the path helpers and `DoAnyMapTransfers` are cut verbatim (build.rs, `drafts_guilogic4`) into
//! `differential/wrap_draft_ftxfer.cpp`, built on upstream's own AsciiString.cpp. The game, the network, the shell, the
//! load screen and the clock are stand-ins in both worlds: upstream's are in the wrapper, the draft's is [`Host`] below; each
//! writes **one text line per call** (`time 123`, `announce path 6`, `ls.progress 1 40 MapTransfer:Recieving`, ...) and the whole
//! logs are compared, with the return value. The calls' results are scripted, identically on both sides: the clock (a list of
//! `timeGetTime` values, then 60,000 ms more at each call), `areAllQueuesEmpty` and `getFileTransferProgress` (lists, then
//! true and 100).
//!
//! 20,000 random scenarios: host or not; which of slots 1..7 are human and which already have the map (a transfer needed for
//! about three quarters); the map contents mask 0..127; a map path with `\`, `.`, letters; the announce id; a start time
//! that is small, around 2^31 ms, around 2^32 ms (the `timeGetTime` wrap), or random, advanced by random steps between 0
//! and 200,000 ms, including exactly 60,000 and 120,000 (the timeout period: a clock reading equal to the deadline), so a timeout happens in many; 0 to 40 progress values biased towards 100.
//! A draft that loops past the 4,000th clock read is stopped (a runaway) and its log compared as it stands.
//!
//! Excluded: the load screen's own work (`MapTransferLoadScreen::init`, `processProgress` ...) and the network's: stand-ins.
//! `TheGameInfo = game` (a global) has no counterpart in the draft's host.
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! Mismatches expected, from reading the C++: `const Int timeoutPeriod`, `UnsignedInt startTime`, `Int now`: `now > startTime +
//! timeoutPeriod` is an UNSIGNED comparison in C++ (the `Int` is converted), `(startTime + timeoutPeriod - now)/1000` an unsigned
//! division; the draft compares and divides as `i32`. They differ only for clock values at or above 2^31 ms (24.8 days of
//! uptime), so the scenarios starting around 2^31 and 2^32 should show it, the others none.
//!
//! # Results
//!
//! 2026-11-20: first run, 20,000 scenarios, **1,146 mismatches** (prediction met: 0 of 4,981 scenarios starting below 2^20 ms, 573 of 5,106 starting around 2^31, 570 of
//! 4,976 around 2^32, 3 of 4,937 random starts): upstream's `now > startTime + timeoutPeriod` and `(startTime + timeoutPeriod - now)/1000` are UNSIGNED (`Int now` is
//! converted to the `UnsignedInt` startTime's type), the draft compared and divided as `i32`; the draft was fixed (file_transfer.rs); then **0 mismatches** over 20,000
//! scenarios (19,355 needed a transfer; the C++ timed out in 15,303 of them). Planted bugs, 9 of 9 caught (mismatches of 20,000): 15874 9,603; 15875 4,905; 15876 16,205;
//! 15877 9,603; 15878 3,762 (the first version of the generator caught it in 2: exact 60,000 and 120,000 ms steps were added); 15879 4,885; 15880 4,052; 15881 18,479; 15882 15,360.
//! One more finding, upstream's: near the 2^32 ms wrap the unsigned deadline wraps and the C++ gives up at once (that is the 570 of 4,976); it is the behaviour
//! the draft now has.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 15874 the mask bit is `1 << (i - 1)`; 15875 the progress loop starts at slot 0; 15876 the "Recieving" test is
//! `<= 100`; 15877 the host does not sleep; 15878 the timeout test is `>=`; 15879 the bit 8 test reads bit 4; 15880 the shell shows
//! again on success too; 15881 the percent is the maximum, not the minimum, of the slots'; 15882 the file is sent again even when
//! it was sent.

mod support;

use rts_lockstep_engine::drafts::sweep_gui_logic::file_transfer as ft;
use std::cell::{Cell, RefCell};
use std::ffi::CString;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_ftx_run(host: i32, humans: u32, has_map: u32, contents: i32, map: *const std::os::raw::c_char, announce_id: i32, times: *const u32, n_times: i32, queues: *const i32, n_queues: i32, progress: *const i32, n_progress: i32) -> i32;
    fn ref_ftx_log(out: *mut std::os::raw::c_char, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

struct Runaway;

struct Host {
    host: bool,
    humans: u32,
    has_map: u32,
    contents: i32,
    map: Vec<u8>,
    announce_id: u16,
    times: Vec<u32>,
    queues: Vec<i32>,
    progress: Vec<i32>,
    n_time: Cell<usize>,
    n_queue: Cell<usize>,
    n_progress: Cell<usize>,
    last_time: Cell<u32>,
    log: RefCell<String>,
}

impl Host {
    fn line(&self, s: String) {
        let mut l = self.log.borrow_mut();
        l.push_str(&s);
        l.push('\n');
    }
}

fn name(b: &[u8]) -> String {
    // the C++ prints a C string: up to the first NUL (none generated); bytes above 0x7f are printed raw, here lossily mapped the same way
    b.iter().map(|&c| c as char).collect()
}

impl ft::FileTransferHost for Host {
    fn am_i_host(&self) -> bool {
        self.host
    }
    fn slot_is_human(&self, i: usize) -> bool {
        (self.humans >> i) & 1 != 0
    }
    fn slot_has_map(&self, i: usize) -> bool {
        (self.has_map >> i) & 1 != 0
    }
    fn map_contents_mask(&self) -> i32 {
        self.contents
    }
    fn map(&self) -> Vec<u8> {
        self.map.clone()
    }
    fn sleep(&mut self, ms: u32) {
        self.line(format!("sleep {ms}"));
    }
    fn time_get_time(&self) -> u32 {
        let n = self.n_time.get();
        if n > 4000 {
            std::panic::panic_any(Runaway);
        }
        let t = if n < self.times.len() {
            self.n_time.set(n + 1);
            self.times[n]
        } else {
            self.n_time.set(n + 1);
            self.last_time.get().wrapping_add(60000)
        };
        self.last_time.set(t);
        self.line(format!("time {t}"));
        t
    }
    fn network_send_file_announce(&mut self, filename: &[u8], mask: u8) -> u16 {
        self.line(format!("announce {} {}", name(filename), mask));
        self.announce_id
    }
    fn network_are_all_queues_empty(&self) -> bool {
        let n = self.n_queue.get();
        let v = if n < self.queues.len() {
            self.n_queue.set(n + 1);
            self.queues[n]
        } else {
            1
        };
        self.line(format!("queues {v}"));
        v != 0
    }
    fn network_send_file(&mut self, filename: &[u8], mask: u8, command_id: u16) {
        self.line(format!("sendFile {} {} {}", name(filename), mask, command_id));
    }
    fn network_get_file_transfer_progress(&mut self, slot: usize, filename: &[u8]) -> i32 {
        let n = self.n_progress.get();
        let v = if n < self.progress.len() {
            self.n_progress.set(n + 1);
            self.progress[n]
        } else {
            100
        };
        self.line(format!("fp {slot} {} {v}", name(filename)));
        v
    }
    fn ls_set_current_filename(&mut self, filename: &[u8]) {
        self.line(format!("ls.filename {}", name(filename)));
    }
    fn ls_process_timeout(&mut self, seconds: i32) {
        self.line(format!("ls.timeout {seconds}"));
    }
    fn ls_update(&mut self, percent: i32) {
        self.line(format!("ls.update {percent}"));
    }
    fn ls_process_progress(&mut self, player_id: i32, percentage: i32, state_str: &[u8]) {
        self.line(format!("ls.progress {player_id} {percentage} {}", name(state_str)));
    }
    fn shell_hide(&mut self) {
        self.line("shell.hide".to_string());
    }
    fn shell_show(&mut self) {
        self.line("shell.show".to_string());
    }
    fn ls_create(&mut self) {
        self.line("ls.init".to_string());
    }
    fn ls_destroy(&mut self) {
        self.line("ls.delete".to_string());
    }
}

fn map_path(g: &mut Gen) -> Vec<u8> {
    let n = 1 + g.below(30) as usize;
    (0..n)
        .map(|_| match g.below(8) {
            0 | 1 => b'\\',
            2 => b'.',
            _ => b'a' + g.below(26) as u8,
        })
        .collect()
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}

#[test]
fn map_transfer() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_F7B1);
    let mut check = Check::new("DoAnyMapTransfers (results and call logs)");
    let mut by_start = [(0u64, 0u64); 4]; // (cases, mismatches) by start time family
    let mut transfers = 0u64;
    let mut timeouts = 0u64;
    let mut buf = vec![0 as std::os::raw::c_char; 1 << 20];
    for _ in 0..20_000 {
        let host = g.below(2) == 0;
        let mut humans = g.u32() & 0xFF;
        let mut has_map = g.u32() & 0xFF;
        if g.below(4) != 0 {
            // make sure a transfer is needed: slot 1 + k is human and lacks the map
            let k = 1 + g.below(7);
            humans |= 1 << k;
            has_map &= !(1 << k);
        }
        let contents = g.below(128) as i32;
        let map = map_path(&mut g);
        let announce_id = g.below(65536) as u16;
        let family = g.below(4) as usize;
        let mut t: u32 = match family {
            0 => g.below(1_000_000),
            1 => 0x7FFF_F000u32.wrapping_add(g.below(0x4000)),
            2 => 0xFFFF_F000u32.wrapping_add(g.below(0x4000)),
            _ => g.u32(),
        };
        let mut times = Vec::new();
        for _ in 0..40 {
            times.push(t);
            t = t.wrapping_add([0, 1, 30, 500, 5000, 59000, 60000, 120000, 121000, 200000][g.below(10) as usize]);
        }
        let queues: Vec<i32> = (0..g.below(6)).map(|_| g.below(2) as i32).collect();
        let progress: Vec<i32> = (0..g.below(41)).map(|_| if g.below(3) == 0 { 100 } else { g.below(101) as i32 }).collect();
        let cmap = CString::new(map.clone()).expect("no NUL");
        let cpp_ok = unsafe { ref_ftx_run(host as i32, humans, has_map, contents, cmap.as_ptr(), announce_id as i32, times.as_ptr(), times.len() as i32, queues.as_ptr(), queues.len() as i32, progress.as_ptr(), progress.len() as i32) };
        let n = unsafe { ref_ftx_log(buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n >= 0, "log buffer too small");
        let cpp_log: String = buf[..n as usize].iter().map(|&c| c as u8 as char).collect();
        let mut h = Host {
            host,
            humans,
            has_map,
            contents,
            map,
            announce_id,
            times: times.clone(),
            queues,
            progress,
            n_time: Cell::new(0),
            n_queue: Cell::new(0),
            n_progress: Cell::new(0),
            last_time: Cell::new(*times.last().expect("times")),
            log: RefCell::new(String::new()),
        };
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ft::do_any_map_transfers(&mut h)));
        let (rust_ok, runaway) = match r {
            Ok(v) => (v as i32, false),
            Err(e) if e.is::<Runaway>() => (-1, true),
            Err(e) => std::panic::resume_unwind(e),
        };
        let rust_log = h.log.borrow().clone();
        if cpp_log.contains("shell.hide") {
            transfers += 1;
        }
        if cpp_ok == 0 {
            timeouts += 1;
        }
        let before = check.mismatches;
        check.eq((rust_ok, rust_log.clone()), (cpp_ok, cpp_log.clone()), || {
            let at = rust_log.lines().zip(cpp_log.lines()).position(|(a, b)| a != b).unwrap_or(0);
            format!("family {family} start {:#x} host {host} humans {humans:#x} has {has_map:#x} contents {contents} runaway {runaway}; first differing line {at}: rust {:?} c++ {:?}", times[0], rust_log.lines().nth(at), cpp_log.lines().nth(at))
        });
        by_start[family].0 += 1;
        by_start[family].1 += check.mismatches - before;
    }
    println!("scenarios with a transfer {transfers}, that timed out (C++) {timeouts}; mismatches by start family [small, ~2^31, ~2^32, random]: {by_start:?}");
    finish(check);
}
