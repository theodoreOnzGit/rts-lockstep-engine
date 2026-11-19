#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the draft src/drafts/wwlib/pointer_remap.rs against (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/pointerremap.h, pointerremap.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! `PointerRemapClass` against the C++ (#215, group wwlib2).
//!
//! # Method
//!
//! `differential/wrap_draft_ptrremap.cpp` `#include`s pointerremap.cpp UNCHANGED (release configuration: no `WWDEBUG`). The C++
//! registers pairs `(old pointer value, address of a RefCountClass object)`, requests plain and ref-counted remaps of locations
//! holding old values, calls `Process()` once and reports what each location holds afterwards (as the index of the object, -1 for
//! NULL) and every object's `Num_Refs()` (the ref-counted requests really `Add_Ref()`). The draft returns `(slot, Some(new))` or
//! `(slot, None)` per request, and the caller adds the reference; the test does that and compares the same words.
//! A case first registers and requests some STALE entries and calls `Reset()` (so a `Reset` that leaves a table behind shows).
//! Old values are drawn from small and large `usize`s (so the address order, not the value order of a small range, is used); about
//! half of the requests ask for a registered old pointer, the rest for an unregistered one (the "failed to re-map" path, which
//! resets the search position), with repeated values, NULL, and cases with no pair at all.
//!
//! # Excluded inputs
//!
//! Two pairs with the same old pointer: upstream sorts them with `qsort`, which is not stable, so which new pointer a request
//! finds is the C library's choice (the draft's stable sort picks the first registered). Registered old pointers here are
//! distinct. (Repeated request values are fine: the answer depends only on the value.) `WWASSERT`s are compiled out upstream; the
//! draft's `debug_assert!` is off in release, and requests with no pair (upstream's `WWASSERT(count > 0)`) are included.
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! 0 mismatches. The search-position reset on a failed remap is the place a translation slips, and upstream's choice not to
//! sort the pair table by new pointer is the other; both are in the plants.
//!
//! # Results and planted bugs
//!
//! See the `DIFF` line of the run. `L9_PLANT=<id>`: 14887 the search position moves one pair on after a failed remap; 14888 the scan
//! stops past the match (`<=`); 14889 the plain request table not sorted; 14890 the ref-counted requests answered from the plain
//! table; 14891 the pair table sorted by the NEW pointer; 14892 `Reset` leaves the ref-counted requests; 14893 a failed remap
//! leaves the old value instead of NULL.

mod support;

use rts_lockstep_engine::drafts::wwlib::pointer_remap::PointerRemapClass;
use support::{Check, Gen};

extern "C" {
    fn ref_ptrremap_run(input: *const u64, n: i32, out: *mut i64) -> i32;
}

const CASES: usize = 40_000;

fn cases(default: usize) -> usize {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(default)
}

fn plant_id() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

/// The "address" of object `k` in the Rust model.
fn obj_addr(k: usize) -> usize {
    0x7000_0000_0000 + k * 64
}

fn old_value(g: &mut Gen) -> u64 {
    match g.below(4) {
        0 => g.below(40) as u64,
        1 => g.below(4000) as u64 * 16,
        2 => g.u64() >> g.below(40),
        _ => 0xFFFF_FFFF_0000_0000 + g.below(1000) as u64,
    }
}

#[test]
fn pointer_remap_matches_the_cpp() {
    let mut g = Gen::new(0x215_9E3A);
    let mut c = Check::new("PointerRemapClass programs");
    let mut bad_programs = 0u64;
    let mut hits = 0u64;
    let mut misses = 0u64;
    for _ in 0..cases(CASES) {
        let k = if g.below(30) == 0 { 0 } else { 1 + g.below(8) as usize };
        // distinct old pointers for the pairs
        let np = if k == 0 { 0 } else { g.below(40) as usize };
        let mut olds: Vec<u64> = Vec::new();
        while olds.len() < np {
            let v = old_value(&mut g);
            if !olds.contains(&v) {
                olds.push(v);
            }
        }
        let pairs: Vec<(u64, usize)> = olds.iter().map(|&o| (o, g.below(k as u32) as usize)).collect();
        let request = |g: &mut Gen, olds: &[u64]| -> u64 {
            if !olds.is_empty() && g.below(2) == 0 {
                olds[g.below(olds.len() as u32) as usize]
            } else {
                old_value(g)
            }
        };
        let nq = g.below(25) as usize;
        let nr = g.below(25) as usize;
        let plain: Vec<u64> = (0..nq).map(|_| request(&mut g, &olds)).collect();
        let counted: Vec<u64> = (0..nr).map(|_| request(&mut g, &olds)).collect();
        let ns = if g.below(3) == 0 { 1 + g.below(5) as usize } else { 0 };
        let stale_p: Vec<u64> = (0..ns).map(|_| old_value(&mut g)).collect();
        let stale_c: Vec<u64> = (0..ns).map(|_| old_value(&mut g)).collect();

        // the C++
        let mut input: Vec<u64> = vec![k as u64, ns as u64];
        input.extend_from_slice(&stale_p);
        input.extend_from_slice(&stale_c);
        input.push(pairs.len() as u64);
        for (o, i) in &pairs {
            input.push(*o);
            input.push(*i as u64);
        }
        input.push(plain.len() as u64);
        input.extend_from_slice(&plain);
        input.push(counted.len() as u64);
        input.extend_from_slice(&counted);
        let mut out = vec![0i64; nq + nr + k + 4];
        let n = unsafe { ref_ptrremap_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr()) } as usize;

        // the draft
        let mut pr = PointerRemapClass::new();
        for s in 0..ns {
            pr.request_pointer_remap(6000 + s, stale_p[s] as usize, "stale", 0);
            pr.request_ref_counted_pointer_remap(5000 + s, stale_c[s] as usize, "stale", 0);
        }
        if k > 0 && ns > 0 {
            pr.register_pointer(0xdead, obj_addr(0));
        }
        pr.reset();
        for (o, i) in &pairs {
            pr.register_pointer(*o as usize, obj_addr(*i));
        }
        for (s, o) in plain.iter().enumerate() {
            pr.request_pointer_remap(s, *o as usize, "plain", s as i32);
        }
        for (s, o) in counted.iter().enumerate() {
            pr.request_ref_counted_pointer_remap(1000 + s, *o as usize, "counted", s as i32);
        }
        let (pres, cres) = pr.process();
        let mut rust = vec![0i64; nq + nr];
        let mut refs = vec![1i64; k];
        let idx_of = |v: Option<usize>| -> i64 {
            match v {
                None => -1,
                Some(a) => (0..k).find(|&o| obj_addr(o) == a).map(|o| o as i64).unwrap_or(-2),
            }
        };
        let m0 = c.mismatches;
        for (slot, newp) in pres {
            if slot >= nq {
                c.eq(0, 1, || format!("a stale plain request {slot} survived Reset"));
                continue;
            }
            rust[slot] = idx_of(newp);
        }
        for (slot, newp) in cres {
            if !(1000..1000 + nr).contains(&slot) {
                c.eq(0, 1, || format!("a stale ref-counted request {slot} survived Reset"));
                continue;
            }
            let r = idx_of(newp);
            rust[nq + slot - 1000] = r;
            if r >= 0 {
                refs[r as usize] += 1; // the caller's Add_Ref()
            }
        }
        // a failed remap in the draft is `None`; upstream leaves NULL: both -1. A found one is an object index.
        rust.extend(refs);
        hits += rust.iter().take(nq + nr).filter(|&&v| v >= 0).count() as u64;
        misses += rust.iter().take(nq + nr).filter(|&&v| v < 0).count() as u64;
        c.eq(n, rust.len(), || format!("lengths: input {input:?}"));
        if n == rust.len() {
            for (j, (r, cpp)) in rust.iter().zip(out[..n].iter()).enumerate() {
                c.eq(*r, *cpp, || format!("word {j}: pairs {pairs:?} plain {plain:?} counted {counted:?} stale {stale_p:?}/{stale_c:?} k {k}"));
            }
        }
        if c.mismatches > m0 {
            bad_programs += 1;
        }
    }
    println!("requests remapped {hits}, failed {misses}");
    if plant_id() != 0 {
        let bad = c.report();
        println!("PLANTED {} : {bad} mismatching words in {bad_programs} programs", plant_id());
        assert!(bad > 0, "plant {} not caught", plant_id());
    } else {
        c.finish();
    }
}
