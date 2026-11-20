#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Thread/GameResultsThread.cpp and Include/GameNetwork/GameSpy/GameResultsThread.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/game_results_thread.rs` against the C++ (GitHub #215, group net4).
//!
//! # Method
//!
//! `differential/wrap_draft_gres.cpp` compiles upstream's `GameResultsQueue` (constructor, start/end threads, add and get requests and responses,
//! `areGameResultsBeingSent`) and the request and response types, cut verbatim by build.rs (`drafts_net4`). The mutex, the thread class and
//! `SubsystemInterface` are stand-ins listed in the wrapper. A program is 30 to 90 operations (opcodes at `ref_gres_run`); the thread flag right after
//! construction (upstream's constructor starts the threads) and every result are dumped word by word and compared.
//!
//! Not covered: `GameResultsThreadClass::Thread_Function` and `sendGameResults` (winsock name resolution and a TCP connection; the draft's
//! `GameResultsWorker` and `GameResultsHost`).

use rts_lockstep_engine::drafts::network::game_results_thread::*;
use std::os::raw::{c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_gres_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
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

fn text(g: &mut Gen, max: u32) -> Vec<u8> {
    let n = g.below(max + 1);
    (0..n).map(|_| (1 + g.below(254)) as u8).collect()
}

fn program(g: &mut Gen) -> (Vec<u32>, Vec<u32>) {
    let q = GameResultsQueue::new();
    let mut input = Vec::new();
    let mut out = vec![q.are_threads_running() as u32];
    let nops = 30 + g.below(61);
    input.push(nops);
    for _ in 0..nops {
        let op = g.below(8);
        input.push(op);
        out.push(0xA5A5A5A5);
        out.push(op);
        match op {
            0 => q.start_threads(),
            1 => q.end_threads(),
            2 => out.push(q.are_threads_running() as u32),
            3 => {
                let r = GameResultsRequest { hostname: text(g, 12), port: g.below(65536) as u16, results: text(g, 40) };
                put_str(&mut input, &r.hostname);
                input.push(r.port as u32);
                put_str(&mut input, &r.results);
                q.add_request(r);
            }
            4 => match q.get_request() {
                None => out.push(0),
                Some(r) => {
                    out.push(1);
                    put_str(&mut out, &r.hostname);
                    out.push(r.port as u32);
                    put_str(&mut out, &r.results);
                }
            },
            5 => {
                let r = GameResultsResponse { hostname: text(g, 12), port: g.below(65536) as u16, sent_ok: g.below(2) == 0 };
                put_str(&mut input, &r.hostname);
                input.push(r.port as u32);
                input.push(r.sent_ok as u32);
                q.add_response(r);
            }
            6 => match q.get_response() {
                None => out.push(0),
                Some(r) => {
                    out.push(1);
                    put_str(&mut out, &r.hostname);
                    out.push(r.port as u32);
                    out.push(r.sent_ok as u32);
                }
            },
            _ => out.push(q.are_game_results_being_sent() as u32),
        }
    }
    (input, out)
}

#[test]
fn game_results_queue_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("GRES_CASES", 20_000);
    let mut g = Gen::new(0x91a6_0002);
    let mut chk = Check::new("game results queue programs");
    for i in 0..n {
        let (input, r) = program(&mut g);
        let mut c = vec![0u32; 200_000];
        let k = unsafe { ref_gres_run(input.as_ptr(), input.len() as c_int, c.as_mut_ptr(), c.len() as c_int) };
        assert!(k >= 0, "the program dump does not fit");
        c.truncate(k as usize);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let op = r[..k].iter().filter(|&&x| x == 0xA5A5A5A5).count();
            println!("case {i}: operation {op}, first difference at word {k} (rust {} words, c++ {} words)", r.len(), c.len());
        }
        chk.eq(r == c, true, || format!("gres program {i}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
