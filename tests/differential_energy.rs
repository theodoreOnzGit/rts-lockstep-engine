// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::energy` (`Energy`) against
//! upstream `RTS/Energy.cpp` (commit 0a05454d), compiled unchanged in
//! `differential/wrap_energy.cpp` (see build.rs). Phase 1 layer 7 (p1w6-energy).
//!
//! # Methodology
//!
//! 1. **Random operation sequences.** Each case starts from a fresh Energy on
//!    both sides and applies 1 to 40 random operations: `init` (no owner, or
//!    one of the 16 players), `adjustPower`, `objectEnteringInfluence`,
//!    `objectLeavingInfluence`, `addPowerBonus`, `removePowerBonus` and
//!    `setPowerSabotagedTillFrame`. Each operation runs at a frame that moves
//!    forward at random, and sometimes jumps past or lands on a sabotage
//!    window. The deltas are small, zero, or at the `Int` limits (where the
//!    sums wrap). After **every** operation, both sides are compared on
//!    production, consumption, `hasSufficientPower`, the bits of
//!    `getEnergySupplyRatio`, the sabotage frame, the number of owner
//!    notifications, the brown-out flag each carried, and the owner's index.
//! 2. **Xfer.** The Energy at the end of each sequence is saved (`XferSave`)
//!    and the bytes compared with upstream's file; upstream's `XferLoad` of the
//!    same bytes and the Rust `XferLoad` of the upstream file must agree on the
//!    owner, the sabotage frame and the sums (which version 3 does not save).
//!    Files of version 1 and 2, written by hand with the same layout, are
//!    loaded by both sides: those versions do save the sums.
//!
//! **Planted bugs.** The operation comparison also runs a Rust variant that
//! sends the owner the opposite brown-out flag; the xfer comparison a variant
//! that loads the sabotage frame one too high. Each must be caught.
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_energy -- --nocapture` (the `DIFF` and `PLANTED` lines).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::energy::Energy;
use std::ffi::CString;
use support::{Check, Gen};

/// One operation on the Energy, as both sides take it.
#[derive(Clone, Copy, Debug)]
enum Op {
    Init(Option<i32>),
    Adjust { delta: i32, adding: bool },
    Entering(i32),
    Leaving(i32),
    AddBonus(i32),
    RemoveBonus(i32),
    Sabotage(u32),
}

impl Op {
    /// The C++ wrapper's `op` code and its two arguments.
    fn encode(self) -> (i32, i32, i32) {
        match self {
            Op::Init(owner) => (0, owner.unwrap_or(-1), 0),
            Op::Adjust { delta, adding } => (1, delta, i32::from(adding)),
            Op::Entering(p) => (2, p, 0),
            Op::Leaving(p) => (3, p, 0),
            Op::AddBonus(b) => (4, b, 0),
            Op::RemoveBonus(b) => (5, b, 0),
            Op::Sabotage(f) => (6, f as i32, 0),
        }
    }
}

fn gen_int(g: &mut Gen) -> i32 {
    match g.below(12) {
        0 => i32::MIN,
        1 => i32::MAX,
        2 => i32::MIN + 1 + g.below(5) as i32,
        3 => i32::MAX - g.below(5) as i32,
        4 => 0,
        5..=7 => g.below(20) as i32 - 10,
        _ => g.below(400) as i32 - 200,
    }
}

fn gen_op(g: &mut Gen, frame: u32, sabotage_at: u32) -> Op {
    match g.below(12) {
        0 => Op::Init(if g.below(4) == 0 { None } else { Some(g.below(16) as i32) }),
        1..=3 => Op::Adjust { delta: gen_int(g), adding: g.below(2) == 0 },
        4 => Op::Entering(gen_int(g)),
        5 => Op::Leaving(gen_int(g)),
        6 => Op::AddBonus(gen_int(g)),
        7 => Op::RemoveBonus(gen_int(g)),
        _ => Op::Sabotage(match g.below(3) {
            0 => sabotage_at,
            1 => frame.wrapping_add(g.below(50)),
            _ => g.u32() >> g.below(31),
        }),
    }
}

/// The Rust side of `ref_energy_op`, the eight values in its order. Planted:
/// the owner is told the opposite brown-out flag.
fn rust_op(e: &mut Energy, op: Op, frame: u32, planted: bool) -> Vec<u32> {
    let notice = match op {
        Op::Init(owner) => {
            e.init(owner);
            None
        }
        Op::Adjust { delta, adding } => e.adjust_power(frame, delta, adding),
        Op::Entering(p) => e.object_entering_influence(frame, p),
        Op::Leaving(p) => e.object_leaving_influence(frame, p),
        Op::AddBonus(b) => e.add_power_bonus(frame, b),
        Op::RemoveBonus(b) => e.remove_power_bonus(frame, b),
        Op::Sabotage(f) => {
            e.set_power_sabotaged_till_frame(f);
            None
        }
    };
    let (count, brown) = match notice {
        Some(n) => (1, u32::from(if planted { !n.brown_out } else { n.brown_out })),
        None => (0, 0),
    };
    vec![
        e.production(frame) as u32,
        e.consumption() as u32,
        u32::from(e.has_sufficient_power(frame)),
        e.energy_supply_ratio(frame).to_bits(),
        e.power_sabotaged_till_frame(),
        count,
        brown,
        e.owner().map_or(u32::MAX, |p| p as u32),
    ]
}

/// The C++ side of `ref_energy_op`.
fn cpp_op(op: Op, frame: u32) -> Vec<u32> {
    let (code, a, b) = op.encode();
    let mut out = [0u32; 8];
    unsafe { cpp::ref_energy_op(code, a, b, frame, out.as_mut_ptr()) };
    out.to_vec()
}

/// The C++ stand-ins keep the one Energy and the notification log in globals
/// (wrap_energy.cpp), so the tests that call them run one at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn energy_operations_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 4_000;
    let mut g = Gen::new(0x3064_0001);
    let mut ok = Check::new("Energy operations (sums, ratio, notices)");
    let mut bug = Check::new("Energy operations (planted: flipped brown-out)");
    for case in 0..CASES {
        unsafe { cpp::ref_energy_reset() };
        // `e` is the real comparison; `p` runs the same operations for the planted variant.
        let mut e = Energy::new();
        let mut p = Energy::new();
        let mut frame = g.below(1000);
        let sabotage_at = frame + g.below(200);
        let len = 1 + g.below(40);
        let mut trace: Vec<String> = Vec::new();
        for step in 0..len {
            frame = frame.wrapping_add(g.below(30));
            let op = gen_op(&mut g, frame, sabotage_at);
            trace.push(format!("{op:?} at {frame}"));
            let input = || format!("case {case} step {step}: {}", trace.join(", "));
            let cpp_out = cpp_op(op, frame);
            ok.eq(rust_op(&mut e, op, frame, false), cpp_out.clone(), input);
            bug.eq(rust_op(&mut p, op, frame, true), cpp_out, input);
        }
    }
    ok.finish();
    bug.finish_planted();
}

/// The file the C++ side and the Rust side both read: a temporary file.
fn temp_path(tag: &str, n: u32) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("p1w6-energy-{}-{tag}-{n}.bin", std::process::id()))
}

/// Writes a version 1 or 2 Energy file by hand: the version, the two sums
/// (version 1 only), the owner's index. No sabotage (version 3 only).
fn old_version_file(version: u8, production: i32, consumption: i32, owner: i32) -> Vec<u8> {
    let mut save = XferSave::new();
    save.open(b"energy").expect("open");
    let mut v = version;
    save.xfer_version(&mut v, 3).expect("version");
    if version < 2 {
        let (mut p, mut c) = (production, consumption);
        save.xfer_int(&mut p).expect("production");
        save.xfer_int(&mut c).expect("consumption");
    }
    let mut o = owner;
    save.xfer_int(&mut o).expect("owner");
    save.close().expect("close");
    save.take_bytes()
}

#[test]
fn energy_xfer_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 1_500;
    let mut g = Gen::new(0x3064_0002);
    let mut ok = Check::new("Energy xfer (save, load, versions 1-3)");
    let mut bug = Check::new("Energy xfer (planted: sabotage loaded + 1)");
    for case in 0..CASES {
        unsafe { cpp::ref_energy_reset() };
        let mut e = Energy::new();
        // Every saved Energy has an owner: upstream's save dereferences `m_owner`
        // (`m_owner->getPlayerIndex()`), so a NULL owner is not a state to save.
        let owner = Op::Init(Some(g.below(16) as i32));
        let _ = rust_op(&mut e, owner, 0, false);
        let _ = cpp_op(owner, 0);
        let mut frame = 0u32;
        for _ in 0..g.below(20) {
            frame = frame.wrapping_add(g.below(30));
            let op = match gen_op(&mut g, frame, frame + 5) {
                Op::Init(_) => Op::Init(Some(g.below(16) as i32)),
                other => other,
            };
            let _ = rust_op(&mut e, op, frame, false);
            let _ = cpp_op(op, frame);
        }
        let input = || format!("case {case}");

        // save: upstream's file against the Rust XferSave of the same state
        let mut save = XferSave::new();
        save.open(b"energy").expect("open");
        e.xfer(&mut save).expect("save");
        save.close().expect("close");
        let rust_bytes = save.take_bytes();
        let path = temp_path("save", case);
        let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
        assert_eq!(unsafe { cpp::ref_energy_save(cpath.as_ptr()) }, 0, "upstream XferSave threw");
        let cpp_bytes = std::fs::read(&path).expect("upstream wrote the file");
        let _ = std::fs::remove_file(&path);
        ok.eq(rust_bytes.clone(), cpp_bytes.clone(), input);

        // load: the Rust XferLoad of upstream's bytes, against upstream's XferLoad
        let load_both = |bytes: &[u8], tag: &str, n: u32| -> (Vec<u32>, Vec<u32>) {
            let path = temp_path(tag, n);
            std::fs::write(&path, bytes).expect("write");
            let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
            let mut out = [0u32; 5];
            unsafe { cpp::ref_energy_load(cpath.as_ptr(), out.as_mut_ptr()) };
            let _ = std::fs::remove_file(&path);
            let mut load = XferLoad::new(bytes.to_vec());
            load.open(b"energy").expect("open");
            let mut back = Energy::new();
            let status = back.xfer(&mut load);
            let rust = match status {
                Ok(()) => vec![
                    0,
                    back.owner().map_or(u32::MAX, |p| p as u32),
                    back.power_sabotaged_till_frame(),
                    back.production(0) as u32,
                    back.consumption() as u32,
                ],
                Err(_) => vec![1],
            };
            (rust, out.to_vec())
        };
        let (rust_loaded, cpp_loaded) = load_both(&cpp_bytes, "load", case);
        if rust_loaded[0] == 0 {
            ok.eq(rust_loaded.clone(), cpp_loaded.clone(), input);
            let mut planted = rust_loaded.clone();
            planted[2] = planted[2].wrapping_add(1);
            bug.eq(planted, cpp_loaded.clone(), input);
        } else {
            ok.eq(rust_loaded.clone(), cpp_loaded[..1].to_vec(), input);
        }

        // versions 1 and 2, written by hand, loaded by both sides
        let version = 1 + (case % 2) as u8;
        let production = gen_int(&mut g);
        let consumption = gen_int(&mut g);
        let owner = g.below(18) as i32 - 1;
        let old = old_version_file(version, production, consumption, owner);
        let (rust_old, cpp_old) = load_both(&old, "old", case);
        let rust_owner_ok = rust_old.clone();
        ok.eq(rust_owner_ok, cpp_old, || format!("case {case}: version {version}, sums {production}/{consumption}, owner {owner}"));
    }
    ok.finish();
    bug.finish_planted();
}
