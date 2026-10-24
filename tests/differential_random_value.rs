// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::random_value` against the original
//! `GameEngine/Source/Common/RandomValue.cpp` (commit 0a05454d), compiled
//! unchanged by `build.rs` (`--features differential`).
//!
//! Methodology: identical seeds and call sequences go to both sides; every
//! output word, every `f32` result (as bits) and the full 6-word states are
//! compared exactly. Pass criterion: zero mismatches.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::random_value::*;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen, N};

const DRAWS: usize = 1_000_000;
const SEEDS: [u32; 6] = [0, 1, 0x1234_5678, 0xdead_beef, 0x7fff_ffff, u32::MAX];

fn lock() -> std::sync::MutexGuard<'static, ()> {
    cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn mult_factor_matches() {
    let mut c = Check::new("theMultFactor");
    c.f32(MULT_FACTOR, unsafe { cpp::ref_mult_factor() }, || "constant".into());
    c.finish();
}

#[test]
fn seed_random_matches() {
    let mut c = Check::new("seedRandom");
    let mut g = Gen::new(1);
    let mut seeds: Vec<u32> = SEEDS.to_vec();
    seeds.extend((0..N).map(|_| g.u32()));
    for &s in &seeds {
        let mut r = [0u32; 6];
        let mut k = [0u32; 6];
        seed_random(s, &mut r);
        unsafe { cpp::ref_seed_random(s, k.as_mut_ptr()) };
        c.eq(r, k, || format!("seed {s:#x}"));
    }
    c.finish();
}

/// 1M consecutive raw draws from each of six seeds, state compared at the end.
#[test]
fn random_value_long_sequences_match() {
    let mut c = Check::new("randomValue (6 seeds x 1M draws)");
    for &s in &SEEDS {
        let mut r = [0u32; 6];
        let mut k = [0u32; 6];
        seed_random(s, &mut r);
        unsafe { cpp::ref_seed_random(s, k.as_mut_ptr()) };
        for i in 0..DRAWS {
            let a = random_value(&mut r);
            let b = unsafe { cpp::ref_random_value(k.as_mut_ptr()) };
            c.eq(a, b, || format!("seed {s:#x} draw {i}"));
        }
        c.eq(r, k, || format!("seed {s:#x} final state"));
    }
    c.finish();
}

/// One draw from 100k arbitrary states, weighted towards words of 0 and
/// 0xffffffff so the add-with-carry chain and the increment carry chain
/// (including upstream's ADC-macro carry quirk) are exercised.
#[test]
fn random_value_arbitrary_states_match() {
    let mut c = Check::new("randomValue (arbitrary states)");
    let mut g = Gen::new(2);
    for _ in 0..N {
        let mut st = [0u32; 6];
        for w in st.iter_mut() {
            *w = match g.below(4) {
                0 => 0,
                1 => u32::MAX,
                2 => u32::MAX - g.below(3),
                _ => g.u32(),
            };
        }
        let mut r = st;
        let mut k = st;
        let a = random_value(&mut r);
        let b = unsafe { cpp::ref_random_value(k.as_mut_ptr()) };
        c.eq((a, r), (b, k), || format!("state {st:08x?}"));
    }
    c.finish();
}

fn sync_states(rv: &RandomValues) {
    unsafe {
        cpp::ref_set_state(0, rv.logic.as_ptr());
        cpp::ref_set_state(1, rv.client.as_ptr());
        cpp::ref_set_state(2, rv.audio.as_ptr());
    }
}

fn cpp_state(which: i32) -> [u32; 6] {
    let mut s = [0u32; 6];
    unsafe { cpp::ref_get_state(which, s.as_mut_ptr()) };
    s
}

/// The public API from `InitRandom(seed)`: 1M interleaved draws per seed over
/// all three streams and both integer and real variants, with random ranges
/// (including lo > hi, lo == hi and the full i32 range), plus the seed and
/// seed-CRC accessors.
#[test]
fn api_sequences_match() {
    let _g = lock();
    let mut ci = Check::new("GetGame{Logic,Client,Audio}RandomValue");
    let mut cr = Check::new("GetGame{Logic,Client,Audio}RandomValueReal");
    let mut cs = Check::new("InitRandom/state/seed/seed CRC");
    let mut g = Gen::new(3);
    for &s in &SEEDS {
        let mut rv = RandomValues::new(s);
        unsafe { cpp::ref_init_random(s) };
        cs.eq(rv.get_game_logic_random_seed(), unsafe { cpp::ref_get_game_logic_random_seed() }, || format!("seed {s}"));
        cs.eq(rv.get_game_logic_random_seed_crc(), unsafe { cpp::ref_get_game_logic_random_seed_crc() }, || format!("crc {s}"));
        for i in 0..DRAWS / 2 {
            let which = g.below(3);
            if g.below(2) == 0 {
                let (lo, hi) = match g.below(8) {
                    0 => (g.i32(), g.i32()),
                    1 => (i32::MIN, i32::MAX),
                    2 => (i32::MAX, i32::MIN),
                    3 => {
                        let v = g.i32();
                        (v, v)
                    }
                    4 => (5, 3),
                    _ => {
                        let lo = (g.i32() % 1000) - 500;
                        (lo, lo + g.below(1000) as i32)
                    }
                };
                let (a, b) = unsafe {
                    match which {
                        0 => (rv.game_logic_random_value(lo, hi), cpp::ref_logic_value(lo, hi)),
                        1 => (rv.game_client_random_value(lo, hi), cpp::ref_client_value(lo, hi)),
                        _ => (rv.game_audio_random_value(lo, hi), cpp::ref_audio_value(lo, hi)),
                    }
                };
                ci.eq(a, b, || format!("seed {s} call {i} stream {which} ({lo}, {hi})"));
            } else {
                let (lo, hi) = match g.below(6) {
                    0 => (g.mixed(), g.mixed()),
                    1 => {
                        let v = g.finite();
                        (v, v)
                    }
                    2 => (1.0, 0.0),
                    _ => {
                        let lo = g.uniform(-1000.0, 1000.0);
                        (lo, lo + g.uniform(0.0, 500.0))
                    }
                };
                let (a, b) = unsafe {
                    match which {
                        0 => (rv.game_logic_random_value_real(lo, hi), cpp::ref_logic_value_real(lo, hi)),
                        1 => (rv.game_client_random_value_real(lo, hi), cpp::ref_client_value_real(lo, hi)),
                        _ => (rv.game_audio_random_value_real(lo, hi), cpp::ref_audio_value_real(lo, hi)),
                    }
                };
                cr.f32(a, b, || format!("seed {s} call {i} stream {which} ({lo:e}, {hi:e})"));
            }
        }
        cs.eq((rv.logic, rv.client, rv.audio), (cpp_state(0), cpp_state(1), cpp_state(2)), || format!("seed {s} final states"));
        cs.eq(rv.get_game_logic_random_seed_crc(), unsafe { cpp::ref_get_game_logic_random_seed_crc() }, || format!("crc after {s}"));
        let s2 = s.wrapping_mul(2_654_435_761);
        rv.init_game_logic_random(s2);
        unsafe { cpp::ref_init_game_logic_random(s2) };
        cs.eq((rv.logic, rv.client, rv.audio), (cpp_state(0), cpp_state(1), cpp_state(2)), || format!("InitGameLogicRandom {s2}"));
        cs.eq(rv.get_game_logic_random_seed(), unsafe { cpp::ref_get_game_logic_random_seed() }, || format!("seed2 {s2}"));
    }
    ci.finish();
    cr.finish();
    cs.finish();
}

/// `GameLogicRandomVariable` / `GameClientRandomVariable::getValue` for all six
/// distribution types (only CONSTANT and UNIFORM draw; the rest return 0).
#[test]
fn random_variables_match() {
    let _g = lock();
    let mut c = Check::new("Game{Logic,Client}RandomVariable::getValue");
    let mut g = Gen::new(4);
    let mut rv = RandomValues::new(99);
    unsafe { cpp::ref_init_random(99) };
    sync_states(&rv);
    let kinds = [
        DistributionType::Constant,
        DistributionType::Uniform,
        DistributionType::Gaussian,
        DistributionType::Triangular,
        DistributionType::LowBias,
        DistributionType::HighBias,
    ];
    for i in 0..N {
        let kind = kinds[g.below(6) as usize];
        let (lo, hi) = if g.below(3) == 0 {
            let v = g.finite();
            (v, v)
        } else {
            (g.mixed(), g.mixed())
        };
        if g.below(2) == 0 {
            let v = GameLogicRandomVariable::new(lo, hi, kind);
            let a = v.get_value(&mut rv);
            let b = unsafe { cpp::ref_logic_variable_get_value(kind as i32, lo, hi) };
            c.f32(a, b, || format!("logic var {i} {kind:?} ({lo:e},{hi:e})"));
        } else {
            let v = GameClientRandomVariable::new(lo, hi, kind);
            let a = v.get_value(&mut rv);
            let b = unsafe { cpp::ref_client_variable_get_value(kind as i32, lo, hi) };
            c.f32(a, b, || format!("client var {i} {kind:?} ({lo:e},{hi:e})"));
        }
    }
    c.eq((rv.logic, rv.client), (cpp_state(0), cpp_state(1)), || "final states".into());
    c.finish();
}
