// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `ww::wwmath` against upstream
//! `Libraries/Source/WWVegas/WWMath/wwmath.h` (commit 0a05454d), compiled by
//! g++ (so the portable, non-`_MSC_VER` branches, which are the ones
//! translated), and the table loops of `wwmath.cpp` (`WWMath::Init`).
//!
//! Elementary functions: the C++ calls to `sinf`, `cosf`, `sincosf`, `acosf`,
//! `asinf`, `atanf`, `atan2f` and to the double `acos`, `asin`, `atan`,
//! `atan2`, `sin`, `sqrt` are bound to the same petir functions the
//! translation uses (see `tests/support/mod.rs`; the shim `math.h` keeps g++
//! from resolving `acos(float)` to a `float` overload), so these gated tests
//! verify the translation itself. How far petir is from glibc is measured
//! separately in `tests/differential_libm_vs_glibc.rs`.
//!
//! Inputs: 100k per function from the mixed generator (NaN, inf, subnormals,
//! rounding boundaries) plus the special list. Domains are restricted only
//! where the C++ itself is undefined, as stated per test.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::wwmath::*;
use support::{Check, Gen, N, SPECIALS};

fn inputs(seed: u64) -> Vec<f32> {
    let mut g = Gen::new(seed);
    let mut v: Vec<f32> = SPECIALS.to_vec();
    v.extend((0..N).map(|_| g.mixed()));
    v
}

fn init_tables() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe { cpp::ref_wwmath_init_tables() });
}

macro_rules! unary_f32 {
    ($test:ident, $name:expr, $seed:expr, $rust:path, $cpp:path) => {
        #[test]
        fn $test() {
            init_tables();
            let mut c = Check::new($name);
            for x in inputs($seed) {
                c.f32($rust(x), unsafe { $cpp(x) }, || format!("x = {x:e} ({:#010x})", x.to_bits()));
            }
            c.finish();
        }
    };
}

unary_f32!(fabs_matches, "WWMath::Fabs", 30, fabs, cpp::ref_fabs);
unary_f32!(sqrt_matches, "WWMath::Sqrt", 31, sqrt, cpp::ref_sqrt);
unary_f32!(inv_sqrt_matches, "WWMath::Inv_Sqrt", 32, inv_sqrt, cpp::ref_inv_sqrt);
unary_f32!(sign_matches, "WWMath::Sign", 33, sign, cpp::ref_sign);
unary_f32!(ceil_matches, "WWMath::Ceil", 34, ceil, cpp::ref_ceil);
unary_f32!(floor_matches, "WWMath::Floor", 35, floor, cpp::ref_floor);
unary_f32!(sin_matches, "WWMath::Sin", 36, sin, cpp::ref_sin);
unary_f32!(cos_matches, "WWMath::Cos", 37, cos, cpp::ref_cos);
unary_f32!(acos_matches, "WWMath::Acos", 38, acos, cpp::ref_acos);
unary_f32!(asin_matches, "WWMath::Asin", 39, asin, cpp::ref_asin);
unary_f32!(atan_matches, "WWMath::Atan", 40, atan, cpp::ref_atan);
unary_f32!(fast_sin_matches, "WWMath::Fast_Sin", 41, fast_sin, cpp::ref_fast_sin);
unary_f32!(fast_cos_matches, "WWMath::Fast_Cos", 42, fast_cos, cpp::ref_fast_cos);
unary_f32!(fast_inv_sin_matches, "WWMath::Fast_Inv_Sin", 43, fast_inv_sin, cpp::ref_fast_inv_sin);
unary_f32!(fast_inv_cos_matches, "WWMath::Fast_Inv_Cos", 44, fast_inv_cos, cpp::ref_fast_inv_cos);

/// The four lookup tables built by `WWMath::Init`, all 4 x 1024 entries.
#[test]
fn fast_tables_match() {
    init_tables();
    let t = fast_tables();
    let mut c = Check::new("WWMath::Init tables (acos, asin, sin, 1/sin)");
    for i in 0..1024 {
        let ii = i as i32;
        unsafe {
            c.f32(t.acos[i], cpp::ref_fast_acos_table(ii), || format!("acos[{i}]"));
            c.f32(t.asin[i], cpp::ref_fast_asin_table(ii), || format!("asin[{i}]"));
            c.f32(t.sin[i], cpp::ref_fast_sin_table(ii), || format!("sin[{i}]"));
            c.f32(t.inv_sin[i], cpp::ref_fast_inv_sin_table(ii), || format!("inv_sin[{i}]"));
        }
    }
    c.finish();
}

/// `Fast_Acos` / `Fast_Asin`: NaN input is excluded because the C++ then
/// indexes the table out of bounds (undefined behaviour; Rust panics).
#[test]
fn fast_arc_functions_match() {
    init_tables();
    let mut ca = Check::new("WWMath::Fast_Acos (no NaN: UB upstream)");
    let mut cs = Check::new("WWMath::Fast_Asin (no NaN: UB upstream)");
    let mut g = Gen::new(45);
    let mut xs: Vec<f32> = SPECIALS.iter().copied().filter(|x| !x.is_nan()).collect();
    let mut accepted = 0;
    while accepted < N {
        let x = if g.below(2) == 0 { g.uniform(-1.0, 1.0) } else { g.mixed() };
        if !x.is_nan() {
            xs.push(x);
            accepted += 1;
        }
    }
    for x in xs {
        ca.f32(fast_acos(x), unsafe { cpp::ref_fast_acos(x) }, || format!("x = {x:e}"));
        cs.f32(fast_asin(x), unsafe { cpp::ref_fast_asin(x) }, || format!("x = {x:e}"));
    }
    ca.finish();
    cs.finish();
}

#[test]
fn float_to_int_bit_tricks_match() {
    let mut cc = Check::new("WWMath::Float_To_Int_Chop");
    let mut cf = Check::new("WWMath::Float_To_Int_Floor");
    let mut xs = inputs(46);
    let mut bits: u64 = 0;
    while bits <= u64::from(u32::MAX) {
        xs.push(f32::from_bits(bits as u32));
        bits += 4099;
    }
    for x in xs {
        let d = || format!("x = {x:e} ({:#010x})", x.to_bits());
        cc.eq(float_to_int_chop(x), unsafe { cpp::ref_float_to_int_chop(x) }, d);
        cf.eq(float_to_int_floor(x), unsafe { cpp::ref_float_to_int_floor(x) }, d);
    }
    cc.finish();
    cf.finish();
}

/// `Float_To_Long`: `(long) f` is undefined in C++ when the truncated value
/// does not fit the Win32 32-bit `long`, so only that domain is compared.
#[test]
fn float_to_long_matches_in_defined_domain() {
    let mut c = Check::new("WWMath::Float_To_Long(float) (|trunc| < 2^31)");
    let mut cd = Check::new("WWMath::Float_To_Long(double) (|trunc| < 2^31)");
    let mut g = Gen::new(47);
    let mut xs = SPECIALS.to_vec();
    while xs.len() < 2 * N {
        xs.push(if g.below(4) == 0 { g.uniform(-2.2e9, 2.2e9) } else { g.mixed() });
    }
    for x in xs {
        if x.is_finite() && x > -2147483904.0 && x < 2147483648.0 && c.compared < (N + SPECIALS.len()) as u64 {
            c.eq(i64::from(float_to_long(x)), unsafe { cpp::ref_float_to_long(x) }, || format!("x = {x:e}"));
        }
        let xd = f64::from(x) + g.uniform(-1.0, 1.0) as f64;
        if xd.is_finite() && xd > -2147483649.0 && xd < 2147483648.0 && cd.compared < (N + SPECIALS.len()) as u64 {
            cd.eq(i64::from(float_to_long_f64(xd)), unsafe { cpp::ref_float_to_long_d(xd) }, || format!("x = {xd:e}"));
        }
    }
    assert!(c.compared >= N as u64 && cd.compared >= N as u64);
    c.finish();
    cd.finish();
}

#[test]
fn small_utilities_match() {
    let mut g = Gen::new(48);
    let mut c_pos = Check::new("WWMath::Fast_Is_Float_Positive");
    let mut c_p2 = Check::new("WWMath::Is_Power_Of_2");
    let mut c_clamp = Check::new("WWMath::Clamp(float)");
    let mut c_clampd = Check::new("WWMath::Clamp(double)");
    let mut c_clampi = Check::new("WWMath::Clamp_Int");
    let mut c_wrap = Check::new("WWMath::Wrap(float)");
    let mut c_wrapd = Check::new("WWMath::Wrap(double)");
    let mut c_min = Check::new("WWMath::Min");
    let mut c_max = Check::new("WWMath::Max");
    let mut c_fai = Check::new("WWMath::Float_As_Int");
    let mut c_lerp = Check::new("WWMath::Lerp(float)");
    let mut c_lerpd = Check::new("WWMath::Lerp(double)");
    let mut c_b2f = Check::new("WWMath::Byte_To_Unit_Float");
    let mut c_f2b = Check::new("WWMath::Unit_Float_To_Byte (f*255 in (-1,256))");
    let mut c_valid = Check::new("WWMath::Is_Valid_Float");
    let mut c_atan2 = Check::new("WWMath::Atan2");
    for i in 0..N {
        let (a, b, x) = (g.mixed(), g.mixed(), g.mixed());
        let d = || format!("case {i}: a={a:e} b={b:e} x={x:e}");
        unsafe {
            c_pos.eq(fast_is_float_positive(x), cpp::ref_fast_is_float_positive(x) != 0, d);
            let u = if g.below(2) == 0 { 1u32 << g.below(32) } else { g.u32() };
            c_p2.eq(is_power_of_2(u), cpp::ref_is_power_of_2(u) != 0, || format!("u = {u}"));
            c_clamp.f32(clamp(x, a, b), cpp::ref_clamp(x, a, b), d);
            let (xd, ad, bd) = (f64::from(x) * 1.000000001, f64::from(a), f64::from(b));
            c_clampd.f64(clamp_f64(xd, ad, bd), cpp::ref_clamp_d(xd, ad, bd), d);
            let (vi, li, hi) = (g.i32(), g.i32() % 1000, g.i32() % 1000);
            c_clampi.eq(clamp_int(vi, li, hi), cpp::ref_clamp_int(vi, li, hi), || format!("{vi} {li} {hi}"));
            let (lo, hi2) = if g.below(2) == 0 { (0.0, 1.0) } else { (a, b) };
            let xw = if g.below(2) == 0 { g.uniform(-3.0, 3.0) } else { x };
            c_wrap.f32(wrap(xw, lo, hi2), cpp::ref_wrap(xw, lo, hi2), || format!("{xw:e} in [{lo:e},{hi2:e}]"));
            let (xwd, lod, hid) = (f64::from(xw) * 1.000000001, f64::from(lo), f64::from(hi2));
            c_wrapd.f64(wrap_f64(xwd, lod, hid), cpp::ref_wrap_d(xwd, lod, hid), d);
            c_min.f32(min(a, b), cpp::ref_min(a, b), d);
            c_max.f32(max(a, b), cpp::ref_max(a, b), d);
            c_fai.eq(float_as_int(x), cpp::ref_float_as_int(x), d);
            c_lerp.f32(lerp(a, b, x), cpp::ref_lerp(a, b, x), d);
            c_lerpd.f64(lerp_f64(f64::from(a) * 1.000000001, f64::from(b), x), cpp::ref_lerp_d(f64::from(a) * 1.000000001, f64::from(b), x), d);
            let byte = g.u32() as u8;
            c_b2f.f32(byte_to_unit_float(byte), cpp::ref_byte_to_unit_float(byte), || format!("byte {byte}"));
            // Two draws per case keep the accepted count above N.
            for _ in 0..2 {
                let f = if g.below(2) == 0 { g.uniform(-0.0039, 1.0039) } else { g.mixed() };
                if f.is_finite() && f * 255.0 > -1.0 && f * 255.0 < 256.0 {
                    c_f2b.eq(unit_float_to_byte(f), cpp::ref_unit_float_to_byte(f), || format!("f = {f:e}"));
                }
            }
            c_valid.eq(is_valid_float(x), cpp::ref_is_valid_float(x) != 0, d);
            c_atan2.f32(atan2(a, b), cpp::ref_atan2(a, b), d);
        }
    }
    for c in [c_pos, c_p2, c_clamp, c_clampd, c_clampi, c_wrap, c_wrapd, c_min, c_max, c_fai, c_lerp, c_lerpd, c_b2f, c_f2b, c_valid, c_atan2] {
        c.finish();
    }
}
