// SPDX-License-Identifier: GPL-3.0-only
//! MEASUREMENT, not a gate: how often the pure-Rust `libm` crate (which the
//! translation uses for every transcendental, so that native and wasm agree)
//! differs from the host glibc (which the C++ reference would use by
//! default), on the functions upstream WWMath calls.
//!
//! This cannot be made to match bit for bit without replacing one library
//! with the other: neither is required to be correctly rounded, and they use
//! different algorithms. The comparison is exact and every differing pair is
//! counted; the tests print the counts and the maximum ULP distance and fail
//! only if nothing was compared. The gated translation tests bind the C++ to
//! the same `libm` (see `tests/support/mod.rs`), so this difference does not
//! leak into them.
//!
//! Consequence to remember: the translation will NOT reproduce glibc (or
//! x87 `fsin`/`fcos`) results for these functions; it reproduces the `libm`
//! crate's, identically on every platform.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen, N, SPECIALS};

fn inputs(seed: u64, unit: bool) -> Vec<f32> {
    let mut g = Gen::new(seed);
    let mut v: Vec<f32> = SPECIALS.to_vec();
    v.extend((0..N).map(|_| {
        if unit && g.below(2) == 0 {
            g.uniform(-1.0, 1.0)
        } else if g.below(2) == 0 {
            g.uniform(-10.0, 10.0)
        } else {
            g.mixed()
        }
    }));
    v
}

#[test]
fn measure_libm_crate_vs_glibc() {
    type Case = (&'static str, fn(f32) -> f32, i32, bool);
    let funcs: [Case; 5] = [
        ("sinf: libm crate vs glibc", libm::sinf, 0, false),
        ("cosf: libm crate vs glibc", libm::cosf, 1, false),
        ("acosf: libm crate vs glibc", libm::acosf, 2, true),
        ("asinf: libm crate vs glibc", libm::asinf, 3, true),
        ("atanf: libm crate vs glibc", libm::atanf, 4, false),
    ];
    let mut compared = 0;
    for (k, (name, f, which, unit)) in funcs.iter().enumerate() {
        let mut c = Check::new(name);
        for x in inputs(60 + k as u64, *unit) {
            c.f32(f(x), unsafe { cpp::ref_glibc_f1(*which, x) }, || format!("x = {x:e} ({:#010x})", x.to_bits()));
        }
        compared += c.compared;
        c.report();
    }
    let mut c = Check::new("atan2f: libm crate vs glibc");
    let mut g = Gen::new(66);
    for _ in 0..N {
        let (y, x) = if g.below(2) == 0 { (g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0)) } else { (g.mixed(), g.mixed()) };
        c.f32(libm::atan2f(y, x), unsafe { cpp::ref_glibc_atan2f(y, x) }, || format!("y = {y:e}, x = {x:e}"));
    }
    compared += c.compared;
    c.report();
    assert!(compared > 0);
}

/// The `WWMath::Init` tables evaluated with each library: how many of the
/// 4 x 1024 entries the choice of library changes.
#[test]
fn measure_fast_tables_libm_crate_vs_glibc() {
    let t = rts_lockstep_engine::ww::wwmath::fast_tables();
    let mut c = Check::new("WWMath::Init tables: libm crate vs glibc");
    let half = 512i32;
    for a in 0..1024usize {
        let cv = ((a as i32 - half) as f32) * (1.0f32 / (half as f32));
        unsafe {
            c.f32(t.acos[a], cpp::ref_glibc_f1(2, cv), || format!("acos[{a}]"));
            c.f32(t.asin[a], cpp::ref_glibc_f1(3, cv), || format!("asin[{a}]"));
        }
        let cv = (a as f32) * 2.0f32 * rts_lockstep_engine::ww::wwmath::WWMATH_PI / 1024.0f32;
        let s = unsafe { cpp::ref_glibc_f1(0, cv) };
        c.f32(t.sin[a], s, || format!("sin[{a}]"));
        if a > 0 {
            c.f32(t.inv_sin[a], 1.0f32 / s, || format!("inv_sin[{a}]"));
        }
    }
    assert!(c.compared > 0);
    c.report();
}
