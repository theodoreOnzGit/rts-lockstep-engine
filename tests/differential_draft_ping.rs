#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Thread/PingThread.cpp and Include/GameNetwork/GameSpy/PingThread.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/ping_thread.rs` against the C++ (GitHub #215, group net4).
//!
//! # Method
//!
//! `differential/wrap_draft_ping.cpp` compiles upstream's `Pinger` (every member from `startThreads` to `getPingString`) and the request and response types,
//! cut verbatim by build.rs (`drafts_net4`), on upstream's own `AsciiString.cpp`/`UnicodeString.cpp`. The mutex and the thread class are stand-ins listed
//! in the wrapper. A program is 30 to 90 operations (opcodes at `ref_ping_run`): start/end threads, add and get requests and responses, the in-progress
//! test, `getPing`, `clearPingMap`, `getPingString`; every result is dumped word by word and compared.
//!
//! Excluded input: `getPingString(0)`. Upstream divides by the timeout and traps; the draft answers 0 for every entry there (documented in the draft).
//! Not covered: `PingThreadClass::Thread_Function` and `doPing` (winsock name resolution and ICMP, the draft's `PingWorker` and `PingHost`).

use rts_lockstep_engine::drafts::network::ping_thread::*;
use std::os::raw::{c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_ping_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn put_str(w: &mut Vec<u32>, s: &[u8]) {
    w.push(s.len() as u32);
    w.extend(s.iter().map(|&b| b as u32));
}

fn host_name(g: &mut Gen) -> Vec<u8> {
    const POOL: [&str; 8] = ["a.example.net", "b.example.net", "10.0.0.1", "", "Zed", "zed", "\u{e9}host", "host-with-a-long-name.example.org"];
    if g.below(3) == 0 {
        let n = 1 + g.below(10);
        (0..n).map(|_| (1 + g.below(254)) as u8).collect()
    } else {
        POOL[g.below(POOL.len() as u32) as usize].as_bytes().to_vec()
    }
}

fn timeout(g: &mut Gen) -> i32 {
    match g.below(8) {
        0 => -1 - g.below(100) as i32,
        1 => 1,
        2 => 20_000_000 + g.below(1000) as i32,
        3 => i32::MAX,
        4 => i32::MIN,
        _ => 1 + g.below(3000) as i32,
    }
}

fn ping_value(g: &mut Gen) -> i32 {
    match g.below(6) {
        0 => -1,
        1 => 0,
        2 => g.i32(),
        _ => g.below(4000) as i32,
    }
}

fn program(g: &mut Gen) -> (Vec<u32>, Vec<u32>) {
    let p = Pinger::new();
    let mut input = Vec::new();
    let mut out = Vec::new();
    let nops = 30 + g.below(61);
    input.push(nops);
    for _ in 0..nops {
        let op = g.below(11);
        input.push(op);
        out.push(0xA5A5A5A5);
        out.push(op);
        match op {
            0 => p.start_threads(),
            1 => p.end_threads(),
            2 => out.push(p.are_threads_running() as u32),
            3 => {
                let r = PingRequest { hostname: host_name(g), repetitions: ping_value(g), timeout: timeout(g) };
                put_str(&mut input, &r.hostname);
                input.push(r.repetitions as u32);
                input.push(r.timeout as u32);
                p.add_request(r);
            }
            4 => match p.get_request() {
                None => out.push(0),
                Some(r) => {
                    out.push(1);
                    put_str(&mut out, &r.hostname);
                    out.push(r.repetitions as u32);
                    out.push(r.timeout as u32);
                }
            },
            5 => {
                let r = PingResponse { hostname: host_name(g), avg_ping: ping_value(g), repetitions: ping_value(g) };
                put_str(&mut input, &r.hostname);
                input.push(r.avg_ping as u32);
                input.push(r.repetitions as u32);
                p.add_response(r);
            }
            6 => match p.get_response() {
                None => out.push(0),
                Some(r) => {
                    out.push(1);
                    put_str(&mut out, &r.hostname);
                    out.push(r.avg_ping as u32);
                    out.push(r.repetitions as u32);
                }
            },
            7 => out.push(p.are_pings_in_progress() as u32),
            8 => {
                let h = host_name(g);
                put_str(&mut input, &h);
                out.push(p.get_ping(&h) as u32);
            }
            9 => p.clear_ping_map(),
            _ => {
                let t = timeout(g);
                input.push(t as u32);
                put_str(&mut out, &p.get_ping_string(t));
            }
        }
    }
    (input, out)
}

#[test]
fn ping_pool_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("PING_CASES", 20_000);
    let mut g = Gen::new(0x91a6_0001);
    let mut chk = Check::new("ping pool programs");
    for i in 0..n {
        let (input, r) = program(&mut g);
        let mut c = vec![0u32; 200_000];
        let k = unsafe { ref_ping_run(input.as_ptr(), input.len() as c_int, c.as_mut_ptr(), c.len() as c_int) };
        assert!(k >= 0, "the program dump does not fit");
        c.truncate(k as usize);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let op = r[..k].iter().filter(|&&x| x == 0xA5A5A5A5).count();
            println!("case {i}: operation {op}, first difference at word {k} (rust {} words, c++ {} words)", r.len(), c.len());
        }
        chk.eq(r == c, true, || format!("ping program {i}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
