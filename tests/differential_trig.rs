// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::trig` against upstream
//! `GameEngine/Source/Common/System/Trig.cpp` and `Lib/trig.h`, and of
//! `common::quick_trig` against `Common/QuickTrig.h` / `QuickTrig.cpp`
//! (commit 0a05454d), compiled by g++ (see `differential/wrap_trig.cpp`,
//! `differential/wrap_quick_trig.cpp`).
//!
//! - Active (`DEFAULT_TRIG`) path: the compiled upstream `Sin`, `Cos`, `Tan`,
//!   `ACos`, `ASin`, whose `sinf`/`cosf`/`tanf`/`acosf`/`asinf` are bound to
//!   the same petir functions as the Rust (`tests/support/mod.rs`). All
//!   inputs, NaN and infinities included.
//! - Table path: the integer kernels (`intSin`, `intCos`, `intTan`,
//!   `intArcCos`) on arbitrary `i32`, both tables entry by entry, and the four
//!   `#else` expressions on the floats for which `(Int)(x * INT_ONE)` is
//!   defined in C++.
//! - QuickTrig: every function on the inputs where upstream reads inside its
//!   tables and terminates (the Rust `try_*` functions say which; the others
//!   are undefined or hang upstream), `QMag` on all inputs, both tables and
//!   both counts.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::{quick_trig as qt, trig};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen, N, SPECIALS};

fn inputs(seed: u64) -> Vec<f32> {
    let mut g = Gen::new(seed);
    let mut v: Vec<f32> = SPECIALS.to_vec();
    v.extend((0..N).map(|_| if g.below(2) == 0 { g.uniform(-10.0, 10.0) } else { g.mixed() }));
    v
}

#[test]
fn default_trig_matches() {
    type F = fn(f32) -> f32;
    let funcs: [(&str, F); 5] = [
        ("Trig Sin (DEFAULT_TRIG)", trig::sin),
        ("Trig Cos (DEFAULT_TRIG)", trig::cos),
        ("Trig Tan (DEFAULT_TRIG)", trig::tan),
        ("Trig ACos (DEFAULT_TRIG)", trig::acos),
        ("Trig ASin", trig::asin),
    ];
    for (op, (name, f)) in funcs.iter().enumerate() {
        let mut c = Check::new(name);
        for x in inputs(100 + op as u64) {
            c.f32(f(x), unsafe { cpp::ref_trig_default(op as i32, x) }, || format!("x = {x:e} ({:#010x})", x.to_bits()));
        }
        c.finish();
    }
}

#[test]
fn lookup_tables_match() {
    let mut c = Check::new("Trig sinLookup, arcCosLookup (all entries)");
    for i in 0..4096usize {
        c.eq(trig::sin_lookup(i), unsafe { cpp::ref_trig_lookup(0, i as i32) }, || format!("sinLookup[{i}]"));
    }
    for i in 0..8192usize {
        c.eq(trig::arc_cos_lookup(i), unsafe { cpp::ref_trig_lookup(1, i as i32) }, || format!("arcCosLookup[{i}]"));
    }
    c.finish();
}

/// Mostly near the table range, a share of arbitrary `i32` (the reduction
/// loops upstream then run up to ~83,000 times, so that share is kept small).
fn int_inputs(g: &mut Gen) -> i32 {
    match g.below(10) {
        0 => g.i32(),
        1 => [0, 1, -1, i32::MAX, i32::MIN, i32::MIN + 1, 6434, 25736, -25736, 12868, 4096, -4096, 8191, -8192][g.below(14) as usize],
        _ => g.i32() % 200_000,
    }
}

#[test]
fn integer_kernels_match() {
    let mut g = Gen::new(110);
    let mut cs = Check::new("Trig intSin");
    let mut cc = Check::new("Trig intCos");
    let mut ct = Check::new("Trig intTan");
    let mut ca = Check::new("Trig intArcCos");
    for _ in 0..N {
        let v = int_inputs(&mut g);
        let d = || format!("v = {v}");
        unsafe {
            cs.eq(trig::int_sin(v), cpp::ref_int_trig(0, v), d);
            cc.eq(trig::int_cos(v), cpp::ref_int_trig(1, v), d);
            ct.eq(trig::int_tan(v), cpp::ref_int_trig(2, v), d);
            let a = if g.below(2) == 0 { g.i32() % 10_000 } else { g.i32() };
            ca.eq(trig::int_arc_cos(a), cpp::ref_int_trig(3, a), || format!("c = {a}"));
        }
    }
    for c in [cs, cc, ct, ca] {
        c.finish();
    }
}

/// `(Int)((x) * INT_ONE)` is defined in C++ only when the truncated product
/// fits an `Int`.
fn fixed_defined(x: f32) -> bool {
    let p = x * 4096.0f32;
    p.is_finite() && (-2147483648.0..2147483648.0).contains(&p)
}

#[test]
fn table_trig_matches_in_defined_domain() {
    type F = fn(f32) -> f32;
    let funcs: [(&str, F); 4] = [
        ("Trig Sin (table #else branch)", trig::table::sin),
        ("Trig Cos (table #else branch)", trig::table::cos),
        ("Trig Tan (table #else branch)", trig::table::tan),
        ("Trig ACos (table #else branch)", trig::table::acos),
    ];
    for (op, (name, f)) in funcs.iter().enumerate() {
        let mut c = Check::new(name);
        let mut g = Gen::new(120 + op as u64);
        let mut xs: Vec<f32> = SPECIALS.to_vec();
        while xs.len() < N + SPECIALS.len() {
            xs.push(match g.below(4) {
                0 => g.uniform(-1.1, 1.1),
                1 => g.uniform(-50.0, 50.0),
                _ => {
                    // Keep |x| moderate: intSin's reduction loops run upstream.
                    let x = g.mixed();
                    if x.abs() < 1.0e4 { x } else { x * 1.0e-6 }
                }
            });
        }
        for x in xs.into_iter().filter(|&x| fixed_defined(x)) {
            c.f32(f(x), unsafe { cpp::ref_trig_table(op as i32, x) }, || format!("x = {x:e} ({:#010x})", x.to_bits()));
        }
        assert!(c.compared >= N as u64 / 2, "{}: too few inputs in the defined domain", name);
        c.finish();
    }
}

#[test]
fn quick_trig_tables_match() {
    let mut c = Check::new("QuickTrig tables and counts");
    for i in 0..129usize {
        unsafe {
            c.f32(qt::QUICK_SIN_TABLE[i], cpp::ref_qtrig_table(0, i as i32), || format!("TheQuickSinTable[{i}]"));
            c.f32(qt::QUICK_TAN_TABLE[i], cpp::ref_qtrig_table(1, i as i32), || format!("TheQuickTanTable[{i}]"));
        }
    }
    unsafe {
        c.f32(qt::QUICK_SIN_TABLE_COUNT, cpp::ref_qtrig_count(0), || "TheQuickSinTableCount".into());
        c.f32(qt::QUICK_TAN_TABLE_COUNT, cpp::ref_qtrig_count(1), || "TheQuickTanTableCount".into());
    }
    c.finish();
}

#[test]
fn quick_trig_matches_where_upstream_is_defined() {
    type F = fn(f32) -> Result<f32, qt::QuickTrigError>;
    fn qcsc(x: f32) -> Result<f32, qt::QuickTrigError> {
        qt::try_qsin(x).map(|_| qt::qcsc(x))
    }
    fn qsec(x: f32) -> Result<f32, qt::QuickTrigError> {
        qt::try_qcos(x).map(|_| qt::qsec(x))
    }
    fn qcot(x: f32) -> Result<f32, qt::QuickTrigError> {
        qt::try_qtan(x).map(|_| qt::qcot(x))
    }
    let funcs: [(&str, F); 6] = [
        ("QuickTrig QSin", qt::try_qsin),
        ("QuickTrig QCos", qt::try_qcos),
        ("QuickTrig QTan", qt::try_qtan),
        ("QuickTrig QCsc", qcsc),
        ("QuickTrig QSec", qsec),
        ("QuickTrig QCot", qcot),
    ];
    for (op, (name, f)) in funcs.iter().enumerate() {
        let mut c = Check::new(name);
        let mut g = Gen::new(130 + op as u64);
        let mut undefined = 0u64;
        let mut xs: Vec<f32> = SPECIALS.to_vec();
        while xs.len() < 2 * N {
            xs.push(match g.below(4) {
                0 => g.uniform(0.0, 1.0),
                1 => g.uniform(-7.0, 7.0),
                2 => g.uniform(-1000.0, 1000.0),
                _ => g.mixed(),
            });
        }
        for x in xs {
            // Bound the range reduction (it runs |x|/PI times on both sides).
            if x.abs() > 1.0e5 && !x.is_nan() {
                continue;
            }
            match f(x) {
                Ok(r) => c.f32(r, unsafe { cpp::ref_qtrig(op as i32, x) }, || format!("x = {x:e} ({:#010x})", x.to_bits())),
                Err(_) => undefined += 1,
            }
        }
        println!("    {name}: {undefined} inputs skipped as undefined upstream (out-of-table read)");
        assert!(c.compared >= N as u64 / 2, "{name}: too few defined inputs");
        c.finish();
    }
}

#[test]
fn qmag_matches() {
    let mut g = Gen::new(140);
    let mut c = Check::new("QuickTrig QMag");
    for _ in 0..N {
        let (x, y, z) = (g.geom(), g.geom(), g.geom());
        c.f32(qt::qmag(x, y, z), unsafe { cpp::ref_qmag(x, y, z) }, || format!("({x:e}, {y:e}, {z:e})"));
    }
    c.finish();
}
