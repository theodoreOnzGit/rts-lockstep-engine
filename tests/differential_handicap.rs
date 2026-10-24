// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::handicap` (`Handicap`) against
//! upstream `RTS/Handicap.cpp` (commit 0a05454d), compiled unchanged in
//! `differential/wrap_handicap.cpp` (see build.rs). Phase 1 layer 4.
//!
//! # Methodology
//!
//! Random dictionaries of `HANDICAP_*` entries (the four keys `readFromDict`
//! looks up, plus misspelt and foreign keys, and some keys present twice in
//! the lookup), with random values (ordinary, zero, negative, NaN and
//! infinite). Each is loaded into a `Dict` through the name table on each
//! side, `readFromDict` runs on a default `Handicap`, and the four multipliers
//! `getHandicap(type, generic | structure)` are compared bit for bit. The
//! names `readFromDict` asks the name table for are compared with the names
//! upstream asks for, in order: the C++ stand-in logs them, the Rust side
//! checks them against the four fixed names (the Rust logs nothing).
//!
//! The C++ dictionary is a stand-in over the name table (the real `Dict`
//! lives under another name in its own build), and the thing template's
//! "is a structure" answer is a stand-in (`ThingTemplate::isKindOf`). What is
//! compared is the lookup: which names are built, and which values land in
//! which multiplier.
//!
//! **Planted bugs.** The comparison also runs against a variant that swaps the
//! build-cost and build-time rows (a plausible slip), which must be caught.
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_handicap -- --nocapture` (the `DIFF` and `PLANTED` lines).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::dict::Dict;
use rts_lockstep_engine::common::handicap::{Handicap, HandicapType};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const NAMES: &[&str] = &[
    "HANDICAP_BUILDCOST_GENERIC",
    "HANDICAP_BUILDCOST_BUILDINGS",
    "HANDICAP_BUILDTIME_GENERIC",
    "HANDICAP_BUILDTIME_BUILDINGS",
    "HANDICAP_BUILDCOST_Generic",
    "HANDICAP_FIREPOWER_GENERIC",
    "HANDICAP_BUILDTIME_BUILDING",
    "BUILDCOST_GENERIC",
    "HANDICAP_ARMOR_BUILDINGS",
];

/// The names `readFromDict` asks for, in upstream's order (type outer, kind inner).
const ASKED: [&str; 4] = [
    "HANDICAP_BUILDCOST_GENERIC",
    "HANDICAP_BUILDCOST_BUILDINGS",
    "HANDICAP_BUILDTIME_GENERIC",
    "HANDICAP_BUILDTIME_BUILDINGS",
];

fn gen_value(g: &mut Gen) -> f32 {
    match g.below(10) {
        0 => f32::NAN,
        1 => f32::INFINITY,
        2 => 0.0,
        3 => -g.uniform(0.0, 4.0),
        _ => g.uniform(0.0, 4.0),
    }
}

/// The Rust side: the dictionary built through the Rust name table, read,
/// and the four multipliers in the wrapper's order, with the swap planted.
fn rust_read(entries: &[(&str, f32)], planted: bool) -> Vec<u32> {
    let mut keys = NameKeyGenerator::new();
    let mut dict = Dict::new();
    for (name, value) in entries {
        let key = keys.name_to_key(name.as_bytes());
        dict.set_real(key, *value).expect("a small dictionary");
    }
    let mut h = Handicap::new();
    h.read_from_dict(&mut keys, &dict);
    let cost = |s: bool| h.get_handicap(HandicapType::BuildCost, s).to_bits();
    let time = |s: bool| h.get_handicap(HandicapType::BuildTime, s).to_bits();
    if planted {
        vec![time(false), cost(true), cost(false), time(true)]
    } else {
        vec![cost(false), cost(true), time(false), time(true)]
    }
}

fn cpp_read(entries: &[(&str, f32)]) -> (Vec<u32>, Vec<String>) {
    let mut names = Vec::new();
    let mut values = Vec::new();
    for (name, value) in entries {
        let mut b = name.as_bytes().to_vec();
        b.push(0);
        names.extend(b);
        values.push(*value);
    }
    let mut out = [0f32; 4];
    let mut log = vec![0u8; 4096];
    let mut log_len = 0i32;
    unsafe {
        cpp::ref_handicap_read(
            entries.len() as i32,
            names.as_ptr() as *const _,
            values.as_ptr(),
            out.as_mut_ptr(),
            log.as_mut_ptr() as *mut _,
            log.len() as i32,
            &mut log_len,
        )
    };
    let asked: Vec<String> = log[..log_len as usize]
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    (out.iter().map(|f| f.to_bits()).collect(), asked)
}

#[test]
fn handicap_read_from_dict_matches() {
    const CASES: u32 = 20_000;
    let mut g = Gen::new(0x4A9D_0001);
    let mut ok = Check::new("Handicap::readFromDict + getHandicap");
    let mut bug = Check::new("Handicap::readFromDict (planted)");
    let mut names_checked = 0u64;
    for case in 0..CASES {
        // a random dictionary: 0 to 8 entries, names may repeat
        let n = g.below(9) as usize;
        let entries: Vec<(&str, f32)> = (0..n)
            .map(|_| {
                (
                    NAMES[g.below(NAMES.len() as u32) as usize],
                    gen_value(&mut g),
                )
            })
            .collect();
        let input = || format!("case {case}: entries {entries:?}");
        let (cpp_bits, asked) = cpp_read(&entries);
        ok.eq(rust_read(&entries, false), cpp_bits.clone(), input);
        bug.eq(rust_read(&entries, true), cpp_bits, input);
        ok.eq(
            asked.iter().map(String::as_str).collect::<Vec<_>>(),
            ASKED.to_vec(),
            input,
        );
        names_checked += 1;
    }
    println!("handicap: {CASES} dictionaries; the names asked for checked in {names_checked}");
    ok.finish();
    bug.finish_planted();
}

#[test]
fn handicap_defaults_and_init_match() {
    // A default Handicap is 1.0 everywhere, and init() restores that after
    // readFromDict set values: the only path with no C++ stand-in is the
    // defaults, checked against the one value upstream's constructor sets.
    let h = Handicap::new();
    for t in [HandicapType::BuildCost, HandicapType::BuildTime] {
        assert_eq!(h.get_handicap(t, false).to_bits(), 1.0f32.to_bits());
        assert_eq!(h.get_handicap(t, true).to_bits(), 1.0f32.to_bits());
    }
    let mut keys = NameKeyGenerator::new();
    let mut dict = Dict::new();
    dict.set_real(keys.name_to_key(b"HANDICAP_BUILDCOST_BUILDINGS"), 0.5)
        .expect("set");
    let mut g = Handicap::new();
    g.read_from_dict(&mut keys, &dict);
    assert_eq!(
        g.get_handicap(HandicapType::BuildCost, true).to_bits(),
        0.5f32.to_bits()
    );
    assert_eq!(
        g.get_handicap(HandicapType::BuildCost, false).to_bits(),
        1.0f32.to_bits()
    );
    g.init();
    assert_eq!(
        g.get_handicap(HandicapType::BuildCost, true).to_bits(),
        1.0f32.to_bits()
    );
}
