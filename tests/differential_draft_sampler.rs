#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the draft src/drafts/wwlib/sampler.rs against (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sampler.h, sampler.cpp, random.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! `SamplingClass` and its four samplers against the C++ (#215, group wwlib2).
//!
//! # Method
//!
//! `differential/wrap_draft_sampler.cpp` `#include`s sampler.cpp and random.cpp UNCHANGED (stand-ins and the -D renames that keep
//! its symbols apart from upstream_w3d_emit's copy of random.cpp are listed there). A case is a PROGRAM on one sampler: a kind
//! (Random, Regular, Stratified, QMC), 0 to 99 dimensions (0 to 12 mostly), 0 to 255 divisions (0, 1, 2 and 255 included), the global
//! `Random` reseeded with a random seed (0 is Random4Class's "seed 4375" case), then 1 to 40 operations: `Sample` (the whole target
//! vector is compared, floats by bits), `Reset`, and for QMC `Set_Offset` (0, 1, 2, `u32::MAX`, 2^31 and random offsets: an offset
//! above `i32::MAX` becomes a negative `int` in `RadInv`, where C++ and Rust agree on the wrap). Every sampled float is compared
//! bit for bit (a NaN against a NaN with other payload bits is not a mismatch: `Divisions` 1 gives 0/0 in the Regular sampler).
//!
//! # Excluded inputs
//!
//! QMC with 100 or more dimensions (upstream's `assert(Dimensions<100)` is compiled out and `primes[i]` then reads past its
//! 100 entries; the draft's `assert!` fires). The Random sampler ignores `Divisions`; the test still passes varied ones.
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! 0 mismatches: the draft's float arithmetic is written step by step as upstream's (`(float) index / (Divisions - 1.0f)`,
//! `(index + Random.Get_Float()) / (float) Divisions`, `RadInv`'s `float` sums). The Mersenne Twister is the verified port's
//! (src/ww/random.rs), so any difference would be the samplers'.
//!
//! # Results and planted bugs
//!
//! See the `DIFF` line of the run. `L9_PLANT=<id>`: 14880 `RadInv` summed in `double`; 14881 Regular divides by `Divisions`
//! instead of `Divisions - 1`; 14882 Stratified `index/Divisions + rand/Divisions`; 14883 the odometer carries at
//! `index == Divisions`; 14884 QMC does not advance its index; 14885 QMC `Reset` does not zero the index; 14886 Regular and
//! Stratified `Reset` do not zero the index.

mod support;

use rts_lockstep_engine::drafts::wwlib::sampler::SamplingClass;
use rts_lockstep_engine::ww::random::Random4;
use support::{Check, Gen};

extern "C" {
    fn ref_sampler_run(kind: i32, dims: u32, divs: i32, seed: u32, prog: *const u32, nprog: i32, out: *mut u32) -> i32;
}

const CASES: usize = 40_000;

fn cases(default: usize) -> usize {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(default)
}

fn plant_id() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

#[test]
fn samplers_match_the_cpp() {
    let mut g = Gen::new(0x215_5A3F);
    let mut c = Check::new("SamplingClass programs (Sample, Reset, Set_Offset)");
    let mut samples = 0u64;
    let mut bad_programs = 0u64;
    for _ in 0..cases(CASES) {
        let kind = g.below(4) as i32;
        let m0 = c.mismatches;
        let dims = if kind == 3 {
            if g.below(10) == 0 { g.below(100) } else { g.below(13) }
        } else if g.below(10) == 0 {
            g.below(30)
        } else {
            g.below(13)
        };
        let divs = match g.below(8) {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 255,
            4 => 254,
            _ => 3 + g.below(10) as i32,
        };
        let seed = if g.below(20) == 0 { 0 } else { g.u32() };
        let mut prog: Vec<u32> = Vec::new();
        for _ in 0..(1 + g.below(40)) {
            match g.below(10) {
                0 => prog.push(1),
                1 | 2 => {
                    prog.push(2);
                    prog.push([0, 1, 2, u32::MAX, 1 << 31, (1 << 31) - 1, g.u32(), g.below(1000)][g.below(8) as usize]);
                }
                _ => prog.push(0),
            }
        }
        // the Rust side
        let mut random = Random4::new(seed);
        let mut s = match kind {
            0 => SamplingClass::new_random(dims, divs as u8),
            1 => SamplingClass::new_regular(dims, divs as u8),
            2 => SamplingClass::new_stratified(dims, divs as u8),
            _ => SamplingClass::new_qmc(dims, divs as u8),
        };
        let mut rust: Vec<u32> = Vec::new();
        let mut i = 0;
        while i < prog.len() {
            match prog[i] {
                0 => {
                    let mut target = vec![0.0f32; dims as usize + 1];
                    s.sample(&mut target, &mut random);
                    rust.extend(target[..dims as usize].iter().map(|f| f.to_bits()));
                    i += 1;
                }
                1 => {
                    s.reset();
                    i += 1;
                }
                _ => {
                    s.set_offset(prog[i + 1]);
                    i += 2;
                }
            }
        }
        samples += (rust.len() / (dims.max(1) as usize)) as u64;
        let mut out = vec![0u32; prog.len() * dims as usize + 4];
        let n = unsafe { ref_sampler_run(kind, dims, divs, seed, prog.as_ptr(), prog.len() as i32, out.as_mut_ptr()) } as usize;
        assert_eq!(n, rust.len(), "kind {kind} dims {dims} divs {divs} prog {prog:?}");
        for (k, (r, cpp)) in rust.iter().zip(out[..n].iter()).enumerate() {
            c.f32(f32::from_bits(*r), f32::from_bits(*cpp), || format!("kind {kind} dims {dims} divs {divs} seed {seed} prog {prog:?} float {k}"));
        }
        if c.mismatches > m0 {
            bad_programs += 1;
        }
    }
    println!("samples about {samples}");
    if plant_id() != 0 {
        let bad = c.report();
        println!("PLANTED {} : {bad} mismatching floats in {bad_programs} programs", plant_id());
        assert!(bad > 0, "plant {} not caught", plant_id());
    } else {
        c.finish();
    }
}
