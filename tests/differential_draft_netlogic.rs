#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/FrameDataManager.cpp, FrameMetrics.cpp
//   GeneralsMD/Code/GameEngine/Include/GameNetwork/FrameDataManager.h, FrameMetrics.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The network drafts' bookkeeping (GitHub #215, group netlogic) against the C++: `src/drafts/network/frame_data_manager.rs`
//! (every function) and `frame_metrics.rs` (every function).
//!
//! # Method
//!
//! `differential/wrap_draft_netlogic.cpp` compiles upstream's `FrameDataManager.cpp` (with `FrameData.cpp`,
//! `NetCommandList.cpp`, `NetCommandRef.cpp`, `NetCommandMsg.cpp` and `NetworkUtil.cpp`) unchanged and `FrameMetrics.cpp` in a copy
//! with one for-scope edit. Stand-ins: `TheDisplay->getAverageFPS()` (a settable value), the three history lengths of `TheGlobalData`,
//! `timeGetTime()` (a settable DWORD), and the two globals `FRAME_DATA_LENGTH` and `MAX_FRAMES_AHEAD`, set per case. A case is a random
//! sequence of operations; both sides write a log of words (every result, then the whole state after every operation) that is compared
//! whole.
//!
//! - **FrameDataManager** (op 1): a ring of 1 to 12 frames with `MAX_FRAMES_AHEAD` 1 to 12 (and the game's 258 and 128 in a tenth of
//!   the cases), local or remote; commands of five kinds with players, execution frames and ids drawn from small ranges (so that
//!   duplicates, ring wrap-around and frames equal modulo the ring all occur), `allCommandsReady` after counts that are equal, short,
//!   long and -1, `resetFrame` either way, `zeroFrames` from any start over any count (wrapping), quit frames.
//! - **FrameMetrics** (op 2): histories of 1 to 8 entries (and the game's 30, 200, 10), 1 to 8 frames ahead; `init` and `reset`, one
//!   to six `doPerFrameMetrics` calls a second apart or less (times that are equal, 999, 1000 and 1001 ms apart, and going backwards),
//!   frames anywhere, fps values that are integers, fractions, large and negative, `processLatencyResponse`, `addCushion` of -1, 0
//!   and larger values.
//!
//! Excluded inputs: a history length or `MAX_FRAMES_AHEAD` of 0 (a division by zero upstream), and a `timeGetTime()` above 2^32 (a
//! DWORD upstream; the draft takes the milliseconds as an `i64` from its caller).

mod support;

use rts_lockstep_engine::common::global_data::GlobalData;
use rts_lockstep_engine::drafts::network::frame_data::FrameDataReturnType;
use rts_lockstep_engine::drafts::network::frame_data_manager::FrameDataManager;
use rts_lockstep_engine::drafts::network::frame_metrics::FrameMetrics;
use rts_lockstep_engine::drafts::network::net_command_list::NetCommandList;
use rts_lockstep_engine::drafts::network::net_command_msg::{NetCommandMsg, NetCommandMsgPtr, NetCommandType, NetworkFrameLimits};
use support::{Check, Gen};

extern "C" {
    fn nl_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 2_000_000];
    let n = unsafe { nl_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn show(what: &str, i: u64, r: &[u32], c: &[u32]) {
    let k = r.iter().zip(c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
    let k0 = k.saturating_sub(8);
    println!(
        "{what} {i}: first difference at word {k} (rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}",
        r.len(),
        c.len(),
        &r[k0..(k + 8).min(r.len())],
        &c[k0..(k + 8).min(c.len())]
    );
}

struct Rd<'a> {
    w: &'a [u32],
    i: usize,
}

impl Rd<'_> {
    fn next(&mut self) -> u32 {
        let v = self.w.get(self.i).copied().unwrap_or(0);
        self.i += 1;
        v
    }
}

fn make_msg(r: &mut Rd) -> NetCommandMsg {
    let kind = r.next();
    let (player, frame, id) = (r.next(), r.next(), r.next());
    let ty = NetCommandType::from_i32(kind as i32).expect("kind");
    let mut m = match ty {
        NetCommandType::FrameInfo => {
            let mut m = NetCommandMsg::new_frame();
            m.set_command_count(r.next() as u16);
            m
        }
        NetCommandType::KeepAlive => NetCommandMsg::new_keep_alive(),
        NetCommandType::Progress => {
            let mut m = NetCommandMsg::new_progress();
            m.set_percentage(r.next() as u8);
            m
        }
        NetCommandType::PlayerLeave => {
            let mut m = NetCommandMsg::new_player_leave();
            m.set_leaving_player_id(r.next() as u8);
            m
        }
        NetCommandType::FrameResendRequest => {
            let mut m = NetCommandMsg::new_frame_resend_request();
            m.set_frame_to_resend(r.next());
            m
        }
        _ => NetCommandMsg::new(),
    };
    m.set_net_command_type(ty);
    m.set_player_id(player);
    m.set_execution_frame(frame);
    m.set_id(id as u16);
    m
}

fn pos_of(l: &NetCommandList, id: Option<rts_lockstep_engine::drafts::network::net_command_ref::NetCommandRefId>) -> u32 {
    let Some(id) = id else { return u32::MAX };
    let mut i = 0u32;
    let mut x = l.get_first_message();
    while let Some(c) = x {
        if c == id {
            return i;
        }
        i += 1;
        x = l.get(c).and_then(|r| r.get_next());
    }
    u32::MAX
}

fn summary(o: &mut Vec<u32>, l: &NetCommandList) {
    o.push(l.length() as u32);
    let mut x = l.get_first_message();
    while let Some(c) = x {
        let r = l.get(c).expect("ref");
        let m = r.get_command().read();
        o.push(m.get_net_command_type() as i32 as u32);
        o.push(m.get_player_id());
        o.push(m.get_execution_frame());
        o.push(m.get_id() as u32);
        o.push(r.get_relay() as u32);
        o.push(m.get_sort_number() as u32);
        x = r.get_next();
    }
    o.push(pos_of(l, l.get_last_message()));
}

// ---------------------------------------------------------------------------------------------------------------
// FrameDataManager
// ---------------------------------------------------------------------------------------------------------------

fn rust_fdm(input: &[u32]) -> Vec<u32> {
    let mut r = Rd { w: input, i: 1 };
    let mut o = Vec::new();
    let limits = NetworkFrameLimits { max_frames_ahead: r.next() as i32, min_runahead: 10, frame_data_length: r.next() as i32, frames_to_keep: 65 };
    let local = r.next() & 1 != 0;
    let mut m = FrameDataManager::new(local, limits);
    let nops = r.next();
    for _ in 0..nops {
        match r.next() {
            0 => m.init(),
            1 => m.reset(),
            2 => m.update(),
            3 => {
                let msg = make_msg(&mut r);
                m.add_net_command_msg(&NetCommandMsgPtr::new(msg));
            }
            4 => {
                let f = r.next();
                let flag = r.next() & 1 != 0;
                o.push(match m.all_commands_ready(f, flag) {
                    FrameDataReturnType::NotReady => 0,
                    FrameDataReturnType::Resend => 1,
                    FrameDataReturnType::Ready => 2,
                });
            }
            5 => {
                let f = r.next();
                match m.get_frame_command_list(f) {
                    Some(l) => {
                        o.push(1);
                        summary(&mut o, l);
                    }
                    None => o.push(0),
                }
            }
            6 => {
                let f = r.next();
                let adv = r.next() & 1 != 0;
                m.reset_frame(f, adv);
            }
            7 => {
                let f = r.next();
                o.push(m.get_command_count(f));
            }
            8 => {
                let f = r.next();
                let c = r.next();
                m.set_frame_command_count(f, c);
            }
            9 => {
                let f = r.next();
                o.push(m.get_frame_command_count(f));
            }
            10 => {
                let s = r.next();
                let n = r.next();
                m.zero_frames(s, n);
            }
            11 => m.destroy_game_messages(),
            12 => {
                let f = r.next();
                m.set_quit_frame(f);
            }
            _ => {}
        }
        o.push(m.get_quit_frame());
        o.push(m.get_is_quitting() as u32);
        for i in 0..limits.frame_data_length as u32 {
            o.push(m.get_frame_data_frame(i));
            o.push(m.get_command_count(i));
            o.push(m.get_frame_command_count(i));
            match m.get_frame_command_list(i) {
                Some(l) => {
                    o.push(1);
                    summary(&mut o, l);
                }
                None => o.push(0),
            }
        }
    }
    o
}

fn gen_spec(g: &mut Gen, v: &mut Vec<u32>, ring: u32) {
    let kind = [3u32, 9, 14, 5, 21][g.below(5) as usize];
    v.push(kind);
    v.push(g.below(3)); // player
    v.push(if g.below(4) == 0 { g.below(ring * 3 + 2) } else { g.below(ring + 2) }); // execution frame
    v.push(g.below(4)); // id
    match kind {
        3 | 14 | 5 | 21 => v.push(g.below(256)),
        _ => {}
    }
}

fn gen_fdm(g: &mut Gen) -> Vec<u32> {
    let (ahead, ring) = if g.below(10) == 0 { (128, 258) } else { (1 + g.below(12), 1 + g.below(12)) };
    let mut v = vec![1, ahead, ring, g.below(2)];
    let nops = 1 + g.below(40);
    v.push(nops);
    for k in 0..nops {
        let code = if k == 0 && g.below(2) == 0 { 0 } else { g.below(20) };
        let frame = |g: &mut Gen| if g.below(4) == 0 { g.u32() } else { g.below(ring * 3 + 2) };
        match code {
            0 => v.push(0),
            1 => v.push(1),
            2 => v.push(2),
            3..=8 => {
                v.push(3);
                gen_spec(g, &mut v, ring);
            }
            9..=10 => {
                v.push(4);
                let f = frame(g);
                v.push(f);
                v.push(g.below(2));
            }
            11 => {
                v.push(5);
                let f = frame(g);
                v.push(f);
            }
            12 => {
                v.push(6);
                let f = frame(g);
                v.push(f);
                v.push(g.below(2));
            }
            13 => {
                v.push(7);
                let f = frame(g);
                v.push(f);
            }
            14 => {
                v.push(8);
                let f = frame(g);
                v.push(f);
                v.push(if g.below(8) == 0 { u32::MAX } else { g.below(6) });
            }
            15 => {
                v.push(9);
                let f = frame(g);
                v.push(f);
            }
            16 => {
                v.push(10);
                let f = frame(g);
                v.push(f);
                v.push(g.below(ring * 2 + 2));
            }
            17 => v.push(11),
            18 => {
                v.push(12);
                let f = frame(g);
                v.push(f);
            }
            _ => {
                v.push(3);
                gen_spec(g, &mut v, ring);
            }
        }
    }
    v
}

#[test]
fn the_frame_data_manager_matches_upstream() {
    let n = cases("NETLOGIC_FDM_CASES", 6000);
    let mut g = Gen::new(0x215_1001);
    let mut chk = Check::new("FrameDataManager operations");
    let mut ready = [0u64; 3];
    for i in 0..n {
        let v = gen_fdm(&mut g);
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&v);
        let r = rust_fdm(&v);
        drop(_one);
        if r != c {
            show("frame data manager case", i, &r, &c);
        }
        chk.eq(r, c, || format!("frame data manager case {i}"));
        let _ = &mut ready;
    }
    println!("{n} frame data manager sequences");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// FrameMetrics
// ---------------------------------------------------------------------------------------------------------------

fn rust_metrics(input: &[u32]) -> Vec<u32> {
    let mut r = Rd { w: input, i: 1 };
    let mut o = Vec::new();
    let mut g = GlobalData::new();
    g.network_fps_history_length = r.next();
    g.network_latency_history_length = r.next();
    g.network_cushion_history_length = r.next();
    let ahead = r.next() as i32;
    let mut m = FrameMetrics::new(ahead, &g);
    let nops = r.next();
    for _ in 0..nops {
        match r.next() {
            0 => m.init(),
            1 => m.reset(),
            2 => {
                let t = r.next();
                let fps = f32::from_bits(r.next());
                let f = r.next();
                m.do_per_frame_metrics(f, t as i64, fps);
            }
            3 => {
                let t = r.next();
                let f = r.next();
                m.process_latency_response(f, t as i64);
            }
            4 => {
                let c = r.next() as i32;
                m.add_cushion(c);
            }
            _ => {}
        }
        m.dump_state(&mut o);
    }
    o
}

fn gen_metrics(g: &mut Gen) -> Vec<u32> {
    let (fps_len, lat_len, cush_len, ahead) = if g.below(10) == 0 {
        (30, 200, 10, 128)
    } else {
        (1 + g.below(8), 1 + g.below(8), 1 + g.below(8), 1 + g.below(8))
    };
    let mut v = vec![2, fps_len, lat_len, cush_len, ahead];
    let nops = 1 + g.below(30);
    v.push(nops);
    let mut now = g.below(5000);
    for k in 0..nops {
        let code = if k == 0 && g.below(4) != 0 { 0 } else { g.below(12) };
        match code {
            0 => v.push(0),
            1 => v.push(1),
            2..=5 => {
                v.push(2);
                now = match g.below(8) {
                    0 => now,
                    1 => now.wrapping_add(999),
                    2 => now.wrapping_add(1000),
                    3 => now.wrapping_add(1001),
                    4 => now.saturating_sub(g.below(3000)),
                    5 => now.wrapping_add(g.below(100000)),
                    _ => now.wrapping_add(g.below(1500)),
                } & 0x7fff_ffff;
                v.push(now);
                let fps = match g.below(6) {
                    0 => g.below(120) as f32,
                    1 => g.uniform(0.0, 120.0),
                    2 => g.uniform(-50.0, 5000.0),
                    3 => 30.0,
                    4 => g.uniform(0.0, 1.0e6),
                    _ => g.uniform(20.0, 90.0),
                };
                v.push(fps.to_bits());
                v.push(if g.below(3) == 0 { g.u32() } else { g.below(300) });
            }
            6..=8 => {
                v.push(3);
                now = now.wrapping_add(g.below(800)) & 0x7fff_ffff;
                v.push(now);
                v.push(if g.below(3) == 0 { g.u32() } else { g.below(300) });
            }
            _ => {
                v.push(4);
                v.push(match g.below(5) {
                    0 => u32::MAX,
                    1 => 0,
                    _ => g.below(40),
                });
            }
        }
    }
    v
}

#[test]
fn the_frame_metrics_match_upstream() {
    let n = cases("NETLOGIC_METRICS_CASES", 8000);
    let mut g = Gen::new(0x215_1002);
    let mut chk = Check::new("FrameMetrics operations");
    for i in 0..n {
        let v = gen_metrics(&mut g);
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&v);
        let r = rust_metrics(&v);
        drop(_one);
        if r != c {
            show("frame metrics case", i, &r, &c);
        }
        chk.eq(r, c, || format!("frame metrics case {i}"));
    }
    println!("{n} frame metrics sequences");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
