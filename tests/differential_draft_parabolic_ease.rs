#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/ParabolicEase.cpp (and Include/GameClient/ParabolicEase.h)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::parabolic_ease::ParabolicEase` against `ParabolicEase` (#215).
//!
//! # Method
//!
//! ParabolicEase.cpp from `namespace {` to the end of the file is cut verbatim (build.rs) into
//! `differential/wrap_draft_parabolic_ease.cpp`; the header is read unchanged (`private` read as `public`). 100,000 cases of
//! each: `setEaseTimes` (in/out times from the generator's mixed set: unit range, out of range, NaN, infinities), the two
//! constructors, and `operator()` on an ease derived from random times (the params hit `m_in`, `m_out`, 0, 1 exactly) and on
//! arbitrary member values. Every result is compared by bits (NaN payload only is counted apart).
//!
//! The C++ here is compiled by g++ for x86_64 (SSE, no excess precision); upstream's Windows build was x87-era MSVC, so the last
//! bit of the `double` mixing could differ there. The draft is meant to equal the SSE result.
//!
//! # Prediction (written before the first run, 2026-11-18)
//!
//! `set_ease_times` and `eval`: 0 mismatches. `new()`: 1 MISS predicted: the draft's `new()` sets both members to 0, and
//! the header's default constructor calls `setEaseTimes(0, 0)`, which gives `m_out = 1`.
//!
//! # Results
//!
//! 2026-11-18: 300,000 `operator()` cases, 200,000 `setEaseTimes`, 200,000 two-argument constructors and the default constructor: **0 mismatches**,
//! 113 NaN-payload-only. The predicted miss was real and was fixed before the first run, by reading the header again: the draft's `new()` set both
//! members to 0 (and said "the header's constructor sets both to 0"); the constructor calls `setEaseTimes(0, 0)`, so `m_out` is 1. `new()` now does the
//! same and `with_times` is the two-argument constructor; plant 11476 restores the old body and is caught (1 of 2). Planted bugs, 9 of 9 caught: 11468
//! 166,114 of 200,000 (setEaseTimes); 11469 83,838 of 200,000; 11470 26,196 of 300,000 (operator()); 11471 18,628; 11472 23,905; 11473 4,171; 11474
//! 23,067; 11475 15,839; 11476 1 of 2. (Two first versions of 11469 and 11470 were never inserted into the draft; the run showed 0 mismatches and they were
//! redone.)
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 11468 `m_out = easeOutTime` (no `1 -`); 11469 overlap test `>=`; 11470 the range check on one side only;
//! 11471 `v0 = 1 + (out - in)`; 11472 `param <= m_in`; 11473 the middle piece in `f32`; 11474 `m_out*m_out - param*param` in
//! `double`; 11475 `param < m_out`; 11476 `new()` as the draft first had it (both members 0).

mod support;

use rts_lockstep_engine::drafts::logic_common::parabolic_ease::ParabolicEase;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_ease_set(ease_in: f32, ease_out: f32, out: *mut f32);
    fn ref_ease_ctor(with_args: i32, ease_in: f32, ease_out: f32, out: *mut f32);
    fn ref_ease_eval(m_in: f32, m_out: f32, param: f32) -> f32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

fn time(g: &mut Gen) -> f32 {
    match g.below(8) {
        0..=3 => g.uniform(0.0, 1.0),
        4 => g.uniform(-0.5, 1.5),
        5 => [0.0f32, 1.0, 0.5, -0.0, 1.0000001, -1.0e-7, 0.99999994][g.below(7) as usize],
        _ => g.mixed(),
    }
}

#[test]
fn set_ease_times_and_constructors() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_EA5E);
    let mut c = Check::new("ParabolicEase::setEaseTimes");
    let mut k = Check::new("ParabolicEase(in, out)");
    for _ in 0..100_000 {
        let (a, b) = (time(&mut g), time(&mut g));
        let mut o = [0.0f32; 2];
        unsafe { ref_ease_set(a, b, o.as_mut_ptr()) };
        let mut e = ParabolicEase { m_in: 0.0, m_out: 0.0 };
        e.set_ease_times(a, b);
        c.f32(e.m_in, o[0], || format!("in {a:e} out {b:e}: m_in"));
        c.f32(e.m_out, o[1], || format!("in {a:e} out {b:e}: m_out"));
        let mut o2 = [0.0f32; 2];
        unsafe { ref_ease_ctor(1, a, b, o2.as_mut_ptr()) };
        let e2 = ParabolicEase::with_times(a, b);
        k.f32(e2.m_in, o2[0], || format!("in {a:e} out {b:e}: m_in"));
        k.f32(e2.m_out, o2[1], || format!("in {a:e} out {b:e}: m_out"));
    }
    let mut d = Check::new("ParabolicEase()");
    let mut o = [9.0f32; 2];
    unsafe { ref_ease_ctor(0, 0.0, 0.0, o.as_mut_ptr()) };
    let e = ParabolicEase::new();
    d.f32(e.m_in, o[0], || "default m_in".into());
    d.f32(e.m_out, o[1], || "default m_out".into());
    let d_misses = d.mismatches;
    let planted_run = planted();
    if planted_run {
        d.report();
        k.report();
        finish(c);
    } else {
        d.finish();
        k.finish();
        c.finish();
    }
    let _ = d_misses;
}

fn param_near(g: &mut Gen, m_in: f32, m_out: f32) -> f32 {
    match g.below(10) {
        0 => m_in,
        1 => m_out,
        2 => 0.0,
        3 => 1.0,
        4 => f32::from_bits(m_in.to_bits().wrapping_add(1)),
        5 => f32::from_bits(m_out.to_bits().wrapping_add(1)),
        6 => g.mixed(),
        _ => g.uniform(0.0, 1.0),
    }
}

#[test]
fn operator_call() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_E7A1);
    let mut c = Check::new("ParabolicEase::operator()");
    for i in 0..300_000 {
        let (m_in, m_out) = if i % 3 == 2 {
            (g.mixed(), g.mixed()) // arbitrary members
        } else {
            let (a, b) = (time(&mut g), time(&mut g));
            let mut o = [0.0f32; 2];
            unsafe { ref_ease_set(a, b, o.as_mut_ptr()) };
            (o[0], o[1])
        };
        let p = param_near(&mut g, m_in, m_out);
        let e = ParabolicEase { m_in, m_out };
        let cpp = unsafe { ref_ease_eval(m_in, m_out, p) };
        c.f32(e.eval(p), cpp, || format!("in {m_in:e} out {m_out:e} param {p:e}"));
    }
    finish(c);
}
