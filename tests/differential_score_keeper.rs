// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::score_keeper` (`ScoreKeeper`)
//! against upstream `RTS/ScoreKeeper.cpp` (commit 0a05454d), compiled unchanged
//! in `differential/wrap_score_keeper.cpp` (see build.rs). Phase 1 layer 7
//! (p1w6-energy).
//!
//! # Methodology
//!
//! 1. **Random event sequences.** Each case sets up a table of 12 templates
//!    (names, kind-of bits, equivalence groups, and whether the factory knows
//!    the name) on both sides, then applies 1 to 60 random calls to a fresh
//!    ScoreKeeper: `reset` (to one of the 16 players), money spent and earned
//!    (ordinary, zero, and the `Int` limits, where the totals wrap), the five
//!    object events (`addObjectBuilt`, `removeObjectBuilt`,
//!    `addObjectCaptured`, `addObjectDestroyed`, `addObjectLost`) on objects of
//!    random templates, controllers, construction states and scoring flags, and
//!    `calculateScore`. After **every** call both sides are compared on the
//!    totals, the current score, the unit totals for three kind-of mask pairs,
//!    and `getTotalObjectsBuilt` for each template.
//! 2. **Xfer.** At the end of each case the ScoreKeeper is saved and compared
//!    byte for byte with upstream's file, except for the order of the entries
//!    of each per-template map: upstream orders them by the template's pointer
//!    (allocation order, which no game data determines), the port by name.
//!    Both files are canonicalised (each map's entries sorted by name) before
//!    the comparison, and the test checks that the canonical forms are equal.
//!    Then upstream's `XferLoad` of the Rust bytes and the Rust `XferLoad` of
//!    upstream's bytes must agree on success or failure (a name the factory does
//!    not know is a failure on both sides) and, on success, on the readout.
//!
//! **Planted bugs.** The event comparison also runs a Rust sequence in which
//! every `removeObjectBuilt` is applied as an `addObjectBuilt` (a call-site
//! slip); the xfer comparison a readout with the current score one too high.
//! Each must be caught.
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_score_keeper -- --nocapture` (the `DIFF` and `PLANTED` lines).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::kind_of::{
    KindOfMaskType, KINDOFMASK_NONE, KINDOF_AIRCRAFT, KINDOF_DOZER, KINDOF_INFANTRY, KINDOF_SCORE,
    KINDOF_SCORE_CREATE, KINDOF_SCORE_DESTROY, KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::score_keeper::{ScoreKeeper, ScoredObject};
use std::ffi::CString;
use support::{Check, Gen};

const NT: usize = 12;
const OUT_LEN: usize = 14 + NT;

/// One template of the case's table, as both sides know it.
#[derive(Clone, Debug)]
struct Tpl {
    name: Vec<u8>,
    kind: KindOfMaskType,
    group: i32,
    known: bool,
}

/// The kinds the table draws from: the ones the score rules test, plus some others.
const KIND_POOL: [i32; 9] = [
    KINDOF_STRUCTURE,
    KINDOF_INFANTRY,
    KINDOF_VEHICLE,
    KINDOF_DOZER,
    KINDOF_AIRCRAFT,
    KINDOF_SCORE,
    KINDOF_SCORE_CREATE,
    KINDOF_SCORE_DESTROY,
    0,
];

fn gen_table(g: &mut Gen) -> Vec<Tpl> {
    (0..NT)
        .map(|i| {
            let mut bits: Vec<i32> = Vec::new();
            for _ in 0..g.below(6) {
                let k = KIND_POOL[g.below(KIND_POOL.len() as u32) as usize];
                if k == 0 {
                    bits.push(g.below(90) as i32 + 8);
                } else {
                    bits.push(k);
                }
            }
            Tpl {
                name: format!("TPL{i:02}").into_bytes(),
                kind: KindOfMaskType::from_indices(&bits),
                group: g.below(4) as i32,
                known: i < NT - 2 || g.below(2) == 0,
            }
        })
        .collect()
}

/// The table, as the C++ setup takes it: NUL-terminated names, groups, known
/// flags, and the kind bits (-1 unused) of each template.
fn encode_table(t: &[Tpl]) -> (Vec<u8>, Vec<i32>, Vec<i32>, Vec<i32>) {
    let mut names = Vec::new();
    let mut groups = Vec::new();
    let mut known = Vec::new();
    let mut kinds = Vec::new();
    for tpl in t {
        names.extend_from_slice(&tpl.name);
        names.push(0);
        groups.push(tpl.group);
        known.push(i32::from(tpl.known));
        let mut bits: Vec<i32> = (0..tpl.kind.size()).filter(|&b| tpl.kind.test(b)).collect();
        bits.resize(6, -1);
        kinds.extend(bits);
    }
    (names, groups, known, kinds)
}

#[derive(Clone, Copy, Debug)]
enum Call {
    Reset(i32),
    Spent(i32),
    Earned(i32),
    Built {
        t: usize,
        c: i32,
        uc: bool,
        scoring: bool,
    },
    Unbuilt {
        t: usize,
        c: i32,
        uc: bool,
        scoring: bool,
    },
    Captured {
        t: usize,
        c: i32,
        uc: bool,
        scoring: bool,
    },
    Destroyed {
        t: usize,
        c: i32,
        uc: bool,
        scoring: bool,
    },
    Lost {
        t: usize,
        c: i32,
        uc: bool,
        scoring: bool,
    },
    Score,
}

impl Call {
    /// The C++ wrapper's `op` code, template, controller, construction, scoring and amount.
    fn encode(self) -> (i32, usize, i32, bool, bool, i32) {
        match self {
            Call::Reset(p) => (0, 0, 0, false, true, p),
            Call::Spent(m) => (1, 0, 0, false, true, m),
            Call::Earned(m) => (2, 0, 0, false, true, m),
            Call::Built { t, c, uc, scoring } => (3, t, c, uc, scoring, 0),
            Call::Unbuilt { t, c, uc, scoring } => (4, t, c, uc, scoring, 0),
            Call::Captured { t, c, uc, scoring } => (5, t, c, uc, scoring, 0),
            Call::Destroyed { t, c, uc, scoring } => (6, t, c, uc, scoring, 0),
            Call::Lost { t, c, uc, scoring } => (7, t, c, uc, scoring, 0),
            Call::Score => (8, 0, 0, false, true, 0),
        }
    }
}

fn gen_amount(g: &mut Gen) -> i32 {
    match g.below(10) {
        0 => i32::MAX,
        1 => i32::MIN,
        2 => 0,
        3..=5 => g.below(1000) as i32,
        _ => g.u32() as i32 >> g.below(31),
    }
}

fn gen_call(g: &mut Gen) -> Call {
    let t = g.below(NT as u32) as usize;
    let c = g.below(16) as i32;
    let uc = g.below(4) == 0;
    let scoring = g.below(10) != 0;
    match g.below(20) {
        0 => Call::Reset(g.below(16) as i32),
        1 => Call::Spent(gen_amount(g)),
        2 => Call::Earned(gen_amount(g)),
        3..=6 => Call::Built { t, c, uc, scoring },
        7..=8 => Call::Unbuilt { t, c, uc, scoring },
        9 => Call::Captured { t, c, uc, scoring },
        10..=13 => Call::Destroyed { t, c, uc, scoring },
        14..=16 => Call::Lost { t, c, uc, scoring },
        _ => Call::Score,
    }
}

/// The three mask pairs of the readout (`getTotalUnitsBuilt`).
fn mask_pairs() -> [(KindOfMaskType, KindOfMaskType); 3] {
    [
        (
            KindOfMaskType::from_indices(&[KINDOF_VEHICLE]),
            KindOfMaskType::from_indices(&[KINDOF_SCORE]),
        ),
        (
            KindOfMaskType::from_indices(&[KINDOF_STRUCTURE]),
            KindOfMaskType::from_indices(&[KINDOF_SCORE_DESTROY]),
        ),
        (
            KindOfMaskType::from_indices(&[KINDOF_INFANTRY]),
            KINDOFMASK_NONE,
        ),
    ]
}

/// The readout the C++ `readout` writes, from the Rust ScoreKeeper.
fn rust_readout(s: &ScoreKeeper, table: &[Tpl], current_score_bias: i32) -> Vec<u32> {
    let kind_of_of = |name: &[u8]| table.iter().find(|t| t.name == name).map(|t| t.kind);
    let mut out = vec![
        s.total_money_earned() as u32,
        s.total_money_spent() as u32,
        s.total_units_destroyed() as u32,
        s.total_units_built() as u32,
        s.total_units_lost() as u32,
        s.total_buildings_destroyed() as u32,
        s.total_buildings_built() as u32,
        s.total_buildings_lost() as u32,
        s.total_tech_buildings_captured() as u32,
        s.total_faction_buildings_captured() as u32,
        s.current_score().wrapping_add(current_score_bias) as u32,
    ];
    for (valid, invalid) in mask_pairs() {
        out.push(s.get_total_units_built(&valid, &invalid, kind_of_of) as u32);
    }
    for k in 0..NT {
        let group = table[k].group;
        let n = s.get_total_objects_built(|name| {
            table
                .iter()
                .find(|t| t.name == name)
                .is_some_and(|t| t.group == group)
        });
        out.push(n as u32);
    }
    out
}

/// The C++ side of one call, with its readout.
fn cpp_call(call: Call) -> Vec<u32> {
    let (op, t, c, uc, scoring, amount) = call.encode();
    let mut out = [0u32; OUT_LEN];
    unsafe {
        cpp::ref_score_op(
            op,
            t as i32,
            c,
            i32::from(uc),
            i32::from(scoring),
            amount,
            out.as_mut_ptr(),
        )
    };
    out.to_vec()
}

/// The Rust side of one call, with its readout. Planted: a removal is applied as an addition.
fn rust_call(s: &mut ScoreKeeper, table: &[Tpl], call: Call, planted: bool) -> Vec<u32> {
    let object = |t: usize, c: i32, uc: bool| ScoredObject {
        template_name: table[t].name.clone(),
        kind_of: table[t].kind,
        controller: c,
        under_construction: uc,
    };
    match call {
        Call::Reset(p) => s.reset(p),
        Call::Spent(m) => s.add_money_spent(m),
        Call::Earned(m) => s.add_money_earned(m),
        Call::Built { t, c, uc, scoring } => s.add_object_built(&object(t, c, uc), scoring),
        Call::Unbuilt { t, c, uc, scoring } => {
            if planted {
                s.add_object_built(&object(t, c, uc), scoring)
            } else {
                s.remove_object_built(&object(t, c, uc), scoring)
            }
        }
        Call::Captured { t, c, uc, scoring } => s.add_object_captured(&object(t, c, uc), scoring),
        Call::Destroyed { t, c, uc, scoring } => s.add_object_destroyed(&object(t, c, uc), scoring),
        Call::Lost { t, c, uc, scoring } => s.add_object_lost(&object(t, c, uc), scoring),
        Call::Score => {
            s.calculate_score();
        }
    }
    rust_readout(s, table, 0)
}

/// The C++ stand-ins keep the table and the ScoreKeeper in globals
/// (wrap_score_keeper.cpp), so the tests that call them run one at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn setup_cpp(table: &[Tpl]) {
    let (names, groups, known, kinds) = encode_table(table);
    // NUL-terminated names, one after another (the buffer ends with its own NUL)
    unsafe {
        cpp::ref_score_setup(
            names.as_ptr() as *const _,
            groups.as_ptr(),
            known.as_ptr(),
            kinds.as_ptr(),
        )
    };
}

/// Canonical form of an upstream-format ScoreKeeper file: each per-template
/// map's entries sorted by name (upstream orders them by pointer). Everything
/// else is copied as it is.
fn canonical(bytes: &[u8]) -> Vec<u8> {
    let mut at = 0usize;
    let mut out = Vec::new();
    let take = |at: &mut usize, n: usize| -> &[u8] {
        let s = &bytes[*at..*at + n];
        *at += n;
        s
    };
    // version byte, two money ints, 64 bytes, two ints, 64 bytes, six ints
    out.extend_from_slice(take(&mut at, 1 + 4 + 4 + 64 + 4 + 4 + 64 + 6 * 4));
    let map = |at: &mut usize, out: &mut Vec<u8>| {
        let version = take(at, 1);
        out.extend_from_slice(version);
        let size_bytes = take(at, 2);
        let size = u16::from_le_bytes([size_bytes[0], size_bytes[1]]);
        out.extend_from_slice(&size.to_le_bytes());
        let mut entries: Vec<(Vec<u8>, [u8; 4])> = Vec::new();
        for _ in 0..size {
            let len = take(at, 1)[0] as usize;
            let name = take(at, len).to_vec();
            let count: [u8; 4] = take(at, 4).try_into().expect("4 bytes");
            entries.push((name, count));
        }
        entries.sort();
        for (name, count) in entries {
            out.push(name.len() as u8);
            out.extend_from_slice(&name);
            out.extend_from_slice(&count);
        }
    };
    map(&mut at, &mut out); // objects built
    let destroyed_size = take(&mut at, 2);
    out.extend_from_slice(destroyed_size);
    for _ in 0..16 {
        map(&mut at, &mut out);
    }
    map(&mut at, &mut out); // objects lost
    map(&mut at, &mut out); // objects captured
    assert_eq!(at, bytes.len(), "the file is exactly the ScoreKeeper");
    out
}

/// Upstream's file reader for the tests: the ScoreKeeper's state after load is
/// read back through the readout; `Ok` only when the load did not fail.
fn rust_load(bytes: &[u8], table: &[Tpl]) -> Option<Vec<u32>> {
    let mut load = XferLoad::new(bytes.to_vec());
    load.open(b"score").expect("open");
    let mut s = ScoreKeeper::new();
    let mut known = |name: &[u8]| table.iter().any(|t| t.known && t.name == name);
    match s.xfer(&mut load, &mut known) {
        Ok(()) => Some(rust_readout(&s, table, 0)),
        Err(_) => None,
    }
}

fn temp_path(tag: &str, n: u32) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("p1w6-score-{}-{tag}-{n}.bin", std::process::id()))
}

#[test]
fn score_keeper_calls_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 3_000;
    let mut g = Gen::new(0x3064_1001);
    let mut ok = Check::new("ScoreKeeper calls (totals, score, queries)");
    let mut bug = Check::new("ScoreKeeper calls (planted: removal as add)");
    for case in 0..CASES {
        let table = gen_table(&mut g);
        setup_cpp(&table);
        unsafe { cpp::ref_score_fresh() };
        let mut s = ScoreKeeper::new();
        let mut p = ScoreKeeper::new();
        let mut trace: Vec<String> = Vec::new();
        for step in 0..1 + g.below(60) {
            let call = gen_call(&mut g);
            trace.push(format!("{call:?}"));
            let input = || format!("case {case} step {step}: {}", trace.join(", "));
            let cpp_out = cpp_call(call);
            ok.eq(
                rust_call(&mut s, &table, call, false),
                cpp_out.clone(),
                input,
            );
            bug.eq(rust_call(&mut p, &table, call, true), cpp_out, input);
        }
    }
    ok.finish();
    bug.finish_planted();
}

#[test]
fn score_keeper_xfer_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 1_000;
    let mut g = Gen::new(0x3064_1002);
    let mut ok = Check::new("ScoreKeeper xfer (save canonical, load, unknown names)");
    let mut bug = Check::new("ScoreKeeper xfer (planted: score off by one)");
    let mut unknown_loads = 0u32;
    for case in 0..CASES {
        let table = gen_table(&mut g);
        setup_cpp(&table);
        unsafe { cpp::ref_score_fresh() };
        let mut s = ScoreKeeper::new();
        for _ in 0..1 + g.below(60) {
            let call = gen_call(&mut g);
            let _ = cpp_call(call);
            let _ = rust_call(&mut s, &table, call, false);
        }
        let input = || format!("case {case}");

        // save
        let mut save = XferSave::new();
        save.open(b"score").expect("open");
        s.xfer(&mut save, &mut |_| true).expect("save");
        save.close().expect("close");
        let rust_bytes = save.take_bytes();
        let path = temp_path("save", case);
        let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
        let status = unsafe { cpp::ref_score_save(cpath.as_ptr()) };
        assert_eq!(status, 0, "upstream XferSave threw");
        let cpp_bytes = std::fs::read(&path).expect("upstream wrote the file");
        let _ = std::fs::remove_file(&path);
        ok.eq(canonical(&rust_bytes), canonical(&cpp_bytes), input);

        // load: upstream's XferLoad of the Rust file, and the Rust XferLoad of upstream's file
        let path2 = temp_path("load", case);
        std::fs::write(&path2, &rust_bytes).expect("write");
        let cpath2 = CString::new(path2.to_string_lossy().as_bytes()).expect("path");
        let mut out = [0u32; 1 + OUT_LEN];
        unsafe { cpp::ref_score_load(cpath2.as_ptr(), out.as_mut_ptr()) };
        let _ = std::fs::remove_file(&path2);
        let cpp_loaded = (out[0] == 0).then(|| out[1..].to_vec());
        unknown_loads += u32::from(cpp_loaded.is_none());
        let rust_loaded = rust_load(&rust_bytes, &table);
        ok.eq(rust_loaded.is_some(), cpp_loaded.is_some(), input);
        if let (Some(r), Some(c)) = (rust_loaded.clone(), cpp_loaded.clone()) {
            ok.eq(r.clone(), c.clone(), input);
            let mut planted = r;
            planted[10] = planted[10].wrapping_add(1);
            bug.eq(planted, c, input);
        }
        let rust_from_cpp = rust_load(&cpp_bytes, &table);
        ok.eq(rust_from_cpp.is_some(), cpp_loaded.is_some(), input);
        if let (Some(r), Some(c)) = (rust_from_cpp, cpp_loaded) {
            ok.eq(r, c, input);
        }
    }
    println!(
        "xfer loads refused by upstream (an unknown template name): {unknown_loads} of {CASES}"
    );
    ok.finish();
    bug.finish_planted();
}
