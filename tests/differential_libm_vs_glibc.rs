// SPDX-License-Identifier: GPL-3.0-only
//! MEASUREMENT, not a gate: how often petir's elementary functions (which the
//! translation uses for all maths, so that native and wasm agree; they forward
//! to the pure-Rust `libm` crate) differ from the host glibc (which the C++
//! reference would use by default), on the functions upstream calls.
//!
//! Both precisions are measured, because upstream uses both: explicit `float`
//! calls (`sinf`, `cosf`, ...) go to petir's `f32` `Real`, and C `double`
//! calls on `float` arguments (`(float)acos(val)`, the `WWMath::Init` tables)
//! go to petir's `f64` `Real` and are then rounded to `f32`. The `f64` rows
//! compare the final `f32` (what the engine stores), not the raw double.
//!
//! This cannot be made to match bit for bit without replacing one library
//! with the other: neither is required to be correctly rounded, and they use
//! different algorithms. The comparison is exact and every differing pair is
//! counted; the tests print the counts and the maximum ULP distance and fail
//! only if nothing was compared. The gated translation tests bind the C++ to
//! the same petir functions (see `tests/support/mod.rs`), so this difference
//! does not leak into them.
//!
//! Consequence to remember: the translation will NOT reproduce glibc (or
//! x87 `fsin`/`fcos`) results for these functions; it reproduces petir's
//! (the `libm` crate's), identically on every platform.
#![cfg(feature = "differential")]

mod support;

use petir::Real;
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
fn measure_petir_f32_vs_glibc() {
    type Case = (&'static str, fn(f32) -> f32, i32, bool);
    let funcs: [Case; 5] = [
        ("sinf: petir f32 vs glibc", <f32 as Real>::sin, 0, false),
        ("cosf: petir f32 vs glibc", <f32 as Real>::cos, 1, false),
        ("acosf: petir f32 vs glibc", <f32 as Real>::acos, 2, true),
        ("asinf: petir f32 vs glibc", <f32 as Real>::asin, 3, true),
        ("atanf: petir f32 vs glibc", <f32 as Real>::atan, 4, false),
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
    let mut c = Check::new("atan2f: petir f32 vs glibc");
    let mut g = Gen::new(66);
    for _ in 0..N {
        let (y, x) = if g.below(2) == 0 { (g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0)) } else { (g.mixed(), g.mixed()) };
        c.f32(Real::atan2(y, x), unsafe { cpp::ref_glibc_atan2f(y, x) }, || format!("y = {y:e}, x = {x:e}"));
    }
    compared += c.compared;
    c.report();
    assert!(compared > 0);
}

/// `(float)f(double(x))` with petir's `f64` and with glibc's double function.
#[test]
fn measure_petir_f64_vs_glibc_rounded_to_f32() {
    type Case = (&'static str, fn(f64) -> f64, i32, bool);
    let funcs: [Case; 5] = [
        ("(float)sin(double): petir f64 vs glibc", <f64 as Real>::sin, 0, false),
        ("(float)cos(double): petir f64 vs glibc", <f64 as Real>::cos, 1, false),
        ("(float)acos(double): petir f64 vs glibc", <f64 as Real>::acos, 2, true),
        ("(float)asin(double): petir f64 vs glibc", <f64 as Real>::asin, 3, true),
        ("(float)atan(double): petir f64 vs glibc", <f64 as Real>::atan, 4, false),
    ];
    let mut compared = 0;
    for (k, (name, f, which, unit)) in funcs.iter().enumerate() {
        let mut c = Check::new(name);
        for x in inputs(70 + k as u64, *unit) {
            let xd = f64::from(x);
            c.f32(f(xd) as f32, unsafe { cpp::ref_glibc_d1(*which, xd) } as f32, || format!("x = {x:e} ({:#010x})", x.to_bits()));
        }
        compared += c.compared;
        c.report();
    }
    let mut c = Check::new("(float)atan2(double): petir f64 vs glibc");
    let mut g = Gen::new(76);
    for _ in 0..N {
        let (y, x) = if g.below(2) == 0 { (g.uniform(-10.0, 10.0), g.uniform(-10.0, 10.0)) } else { (g.mixed(), g.mixed()) };
        let (yd, xd) = (f64::from(y), f64::from(x));
        c.f32(Real::atan2(yd, xd) as f32, unsafe { cpp::ref_glibc_atan2(yd, xd) } as f32, || format!("y = {y:e}, x = {x:e}"));
    }
    compared += c.compared;
    c.report();
    assert!(compared > 0);
}

/// The `WWMath::Init` tables (double `acos`/`asin`/`sin`, rounded to `f32`)
/// evaluated with each library: how many of the 4 x 1024 entries the choice
/// of library changes.
#[test]
fn measure_fast_tables_petir_vs_glibc() {
    let t = rts_lockstep_engine::ww::wwmath::fast_tables();
    let mut c = Check::new("WWMath::Init tables: petir vs glibc");
    let half = 512i32;
    for a in 0..1024usize {
        let cv = ((a as i32 - half) as f32) * (1.0f32 / (half as f32));
        unsafe {
            c.f32(t.acos[a], cpp::ref_glibc_d1(2, f64::from(cv)) as f32, || format!("acos[{a}]"));
            c.f32(t.asin[a], cpp::ref_glibc_d1(3, f64::from(cv)) as f32, || format!("asin[{a}]"));
        }
        let cv = (a as f32) * 2.0f32 * rts_lockstep_engine::ww::wwmath::WWMATH_PI / 1024.0f32;
        let s = unsafe { cpp::ref_glibc_d1(0, f64::from(cv)) } as f32;
        c.f32(t.sin[a], s, || format!("sin[{a}]"));
        if a > 0 {
            c.f32(t.inv_sin[a], 1.0f32 / s, || format!("inv_sin[{a}]"));
        }
    }
    assert!(c.compared > 0);
    c.report();
}
