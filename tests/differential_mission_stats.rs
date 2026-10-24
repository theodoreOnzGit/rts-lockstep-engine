// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::mission_stats` (`MissionStats`)
//! against upstream `RTS/MissionStats.cpp` (commit 0a05454d), compiled unchanged
//! in `differential/wrap_mission_stats.cpp` (see build.rs). Phase 1 layer 7
//! (p1w6-energy).
//!
//! # Methodology
//!
//! Upstream has no method that counts, so the random events set a counter
//! directly (both sides: the four counters, the per-owner arrays at random
//! owners, with the `Int` limits among the values), and `init()` clears them.
//! After **every** event the 34 counters are compared. At the end of each case
//! the statistics are saved and the bytes compared with upstream's file (the
//! per-owner arrays go out as raw native integers, `xferUser`), and upstream's
//! `XferLoad` of the Rust bytes and the Rust `XferLoad` of upstream's bytes must
//! agree on all 34 counters.
//!
//! **Planted bugs.** The event comparison also runs a Rust sequence whose
//! counter writes go to the wrong owner's slot; the xfer comparison a readout
//! with the units-lost counter one too high. Each must be caught.
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_mission_stats -- --nocapture` (the `DIFF` and `PLANTED` lines).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::game_common::MAX_PLAYER_COUNT;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::mission_stats::MissionStats;
use std::ffi::CString;
use support::{Check, Gen};

const READOUT: usize = 2 * MAX_PLAYER_COUNT as usize + 2;

/// The counters in the wrapper's readout order.
fn readout(m: &MissionStats) -> Vec<u32> {
    let mut v: Vec<u32> = m.units_killed.iter().map(|&x| x as u32).collect();
    v.push(m.units_lost as u32);
    v.extend(m.buildings_killed.iter().map(|&x| x as u32));
    v.push(m.buildings_lost as u32);
    v
}

fn gen_value(g: &mut Gen) -> i32 {
    match g.below(8) {
        0 => i32::MIN,
        1 => i32::MAX,
        2 => 0,
        3 => -(g.below(100) as i32),
        _ => g.below(10_000) as i32,
    }
}

/// The C++ stand-ins keep the statistics in a global (wrap_mission_stats.cpp),
/// so the tests that call them run one at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn temp_path(tag: &str, n: u32) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("p1w6-mission-{}-{tag}-{n}.bin", std::process::id()))
}

#[test]
fn mission_stats_events_and_xfer_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 3_000;
    let mut g = Gen::new(0x3064_3001);
    let mut ok = Check::new("MissionStats counters (events, init, xfer)");
    let mut bug = Check::new("MissionStats counters (planted: owner slot)");
    for case in 0..CASES {
        let mut m = MissionStats::new();
        let mut planted = MissionStats::new();
        let mut out = [0u32; READOUT];
        unsafe { cpp::ref_mission_op(0, 0, 0, out.as_mut_ptr()) };
        let mut trace: Vec<String> = Vec::new();
        for step in 0..1 + g.below(40) {
            let op = if g.below(12) == 0 {
                0
            } else {
                1 + g.below(4) as i32
            };
            let idx = g.below(MAX_PLAYER_COUNT as u32) as i32;
            let value = gen_value(&mut g);
            trace.push(format!("op {op} [{idx}] = {value}"));
            let input = || format!("case {case} step {step}: {}", trace.join(", "));
            if op == 0 {
                m.init();
                planted.init();
            }
            match op {
                1 => m.units_killed[idx as usize] = value,
                2 => m.buildings_killed[idx as usize] = value,
                3 => m.units_lost = value,
                4 => m.buildings_lost = value,
                _ => {}
            }
            // the planted slip: the write lands in the next owner's slot
            match op {
                1 => planted.units_killed[((idx + 1) % MAX_PLAYER_COUNT) as usize] = value,
                2 => planted.buildings_killed[((idx + 1) % MAX_PLAYER_COUNT) as usize] = value,
                3 => planted.units_lost = value,
                4 => planted.buildings_lost = value,
                _ => {}
            }
            unsafe { cpp::ref_mission_op(op, idx, value, out.as_mut_ptr()) };
            ok.eq(readout(&m), out.to_vec(), input);
            bug.eq(readout(&planted), out.to_vec(), input);
        }

        let input = || format!("case {case}");
        let mut save = XferSave::new();
        save.open(b"mission").expect("open");
        m.xfer(&mut save).expect("save");
        save.close().expect("close");
        let rust_bytes = save.take_bytes();
        let path = temp_path("save", case);
        let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
        assert_eq!(
            unsafe { cpp::ref_mission_save(cpath.as_ptr()) },
            0,
            "upstream XferSave threw"
        );
        let cpp_bytes = std::fs::read(&path).expect("upstream wrote the file");
        let _ = std::fs::remove_file(&path);
        ok.eq(rust_bytes.clone(), cpp_bytes.clone(), input);

        // upstream's XferLoad of the Rust bytes
        let path2 = temp_path("load", case);
        std::fs::write(&path2, &rust_bytes).expect("write");
        let cpath2 = CString::new(path2.to_string_lossy().as_bytes()).expect("path");
        let mut loaded = [0u32; READOUT + 1];
        unsafe { cpp::ref_mission_load(cpath2.as_ptr(), loaded.as_mut_ptr()) };
        let _ = std::fs::remove_file(&path2);
        assert_eq!(loaded[0], 0, "upstream XferLoad threw");
        let mut back = MissionStats::new();
        let mut load = XferLoad::new(rust_bytes.clone());
        load.open(b"mission").expect("open");
        back.xfer(&mut load).expect("load");
        ok.eq(readout(&back), loaded[1..].to_vec(), input);
        bug.eq(
            readout(&back)
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    if i == MAX_PLAYER_COUNT as usize {
                        v + 1
                    } else {
                        v
                    }
                })
                .collect::<Vec<u32>>(),
            loaded[1..].to_vec(),
            input,
        );

        // the Rust XferLoad of upstream's bytes
        let mut back2 = MissionStats::new();
        let mut load2 = XferLoad::new(cpp_bytes.clone());
        load2.open(b"mission").expect("open");
        back2.xfer(&mut load2).expect("load");
        ok.eq(readout(&back2), readout(&m), input);
    }
    ok.finish();
    bug.finish_planted();
}
