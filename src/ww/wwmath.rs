// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/wwmath.h
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/wwmath.cpp (table init)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWMath, author Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class WWMath`: small float utilities used throughout the engine.
//!
//! # Which upstream branch is translated
//!
//! `wwmath.h` has two implementations of five functions, selected by
//! `#if defined(_MSC_VER) && defined(_M_IX86)`. The shipped Win32 game took
//! the MSVC branch; this translation follows the **portable** branch, which is
//! the one that can be compiled and checked bit for bit against the original
//! source on a modern compiler:
//!
//! | function | portable branch (translated) | MSVC x86 branch (shipped) |
//! |---|---|---|
//! | `Sin`, `Cos` | `sinf`, `cosf` (here: petir's `f32` `Real`) | x87 `fsin`, `fcos` |
//! | `Sqrt` | `(float)sqrt(val)` | x87 `fsqrt` |
//! | `Inv_Sqrt` | `1.0f / (float)sqrt(val)` | a hand-written x87 Newton iteration |
//! | `Float_To_Long` | `(long) f` (truncate) | `fld`/`fistp` (round to nearest) |
//!
//! `Sqrt` agrees between the two up to x87 exponent-range effects (both round
//! the exact root to 24 bits under the game's `_PC_24` mode). The other four
//! differ in their results, which matters only for the (separate, not
//! promised) goal of reproducing original replays; Rust-to-Rust lockstep is
//! unaffected. This is a recorded decision, open for review, not an accident.
//!
//! # Elementary functions: petir, in upstream's precision
//!
//! Every elementary function goes through the owner's `petir` crate
//! ([`petir::Real`], which forwards to the pure-Rust `libm` crate), never
//! `std`, so native and wasm builds compute identical bits. The precision is
//! upstream's, call by call:
//!
//! - upstream calls a C **double** function on a `float` (`(float)acos(val)`,
//!   `(float)asin(val)`, `static_cast<float>(atan(x))`,
//!   `static_cast<float>(atan2(y,x))`, `(float)sqrt(val)`, and `acos`/`asin`/
//!   `sin` in the `WWMath::Init` table loops): the argument is widened, the
//!   `f64` function is evaluated, and the result is rounded to `f32` once.
//!   Under the MSVC headers the shipped game was built with, C `acos(float)`
//!   is that double computation. (`g++`'s libstdc++ would instead pick a
//!   `float` overload; the differential build disables those overloads with a
//!   shim `math.h`, see `differential/shim/math.h`.)
//! - upstream calls an explicit **float** function (`sinf`, `cosf`, `ceilf`,
//!   `floorf`): petir's `f32` implementation. Computing it in `f64` and
//!   rounding would give a different function.
//!
//! For `sqrt` the two routes give the same bits (both are correctly rounded,
//! and double rounding through `f64` is exact for a square root); the
//! upstream form is kept anyway. `WWMath::Random_Float` (C `rand()`, not
//! deterministic across platforms) is not translated.

// Upstream's literal values, kept digit for digit (they are the spec, not
// approximations to be replaced by `std::f32::consts`).
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use petir::Real;
use std::sync::OnceLock;

pub const WWMATH_EPSILON: f32 = 0.0001;
/// `#define WWMATH_EPSILON2 WWMATH_EPSILON * WWMATH_EPSILON`.
pub const WWMATH_EPSILON2: f32 = WWMATH_EPSILON * WWMATH_EPSILON;
pub const WWMATH_PI: f32 = 3.141592654;
pub const WWMATH_FLOAT_MAX: f32 = f32::MAX;
pub const WWMATH_FLOAT_MIN: f32 = f32::MIN_POSITIVE;
pub const WWMATH_SQRT2: f32 = 1.414213562;
pub const WWMATH_SQRT3: f32 = 1.732050808;
pub const WWMATH_OOSQRT2: f32 = 0.707106781;
pub const WWMATH_OOSQRT3: f32 = 0.577350269;

pub const ARC_TABLE_SIZE: usize = 1024;
pub const SIN_TABLE_SIZE: usize = 1024;

/// `_FastAcosTable`, `_FastAsinTable`, `_FastSinTable`, `_FastInvSinTable`.
pub struct FastTables {
    pub acos: [f32; ARC_TABLE_SIZE],
    pub asin: [f32; ARC_TABLE_SIZE],
    pub sin: [f32; SIN_TABLE_SIZE],
    pub inv_sin: [f32; SIN_TABLE_SIZE],
}

impl FastTables {
    /// The table loops of `WWMath::Init()` (wwmath.cpp). Upstream calls the
    /// C double functions `acos`/`asin`/`sin` on a `float` and stores the
    /// result in a `float` table: widened, evaluated in `f64`, rounded once.
    pub fn compute() -> Self {
        let mut t = FastTables {
            acos: [0.0; ARC_TABLE_SIZE],
            asin: [0.0; ARC_TABLE_SIZE],
            sin: [0.0; SIN_TABLE_SIZE],
            inv_sin: [0.0; SIN_TABLE_SIZE],
        };
        let half = (ARC_TABLE_SIZE / 2) as i32;
        for a in 0..ARC_TABLE_SIZE {
            let cv = ((a as i32 - half) as f32) * (1.0f32 / (half as f32));
            t.acos[a] = Real::acos(f64::from(cv)) as f32;
            t.asin[a] = Real::asin(f64::from(cv)) as f32;
        }
        for a in 0..SIN_TABLE_SIZE {
            let cv = (a as f32) * 2.0f32 * WWMATH_PI / (SIN_TABLE_SIZE as f32);
            t.sin[a] = Real::sin(f64::from(cv)) as f32;
            if a > 0 {
                t.inv_sin[a] = 1.0f32 / t.sin[a];
            } else {
                t.inv_sin[a] = WWMATH_FLOAT_MAX;
            }
        }
        t
    }
}

/// The tables, computed on first use. (Upstream fills globals in
/// `WWMath::Init()`; they are a pure function of petir, so computing them
/// lazily cannot change any result.)
pub fn fast_tables() -> &'static FastTables {
    static TABLES: OnceLock<FastTables> = OnceLock::new();
    TABLES.get_or_init(FastTables::compute)
}

/// `WWMath::Fabs`: clears the sign bit.
#[inline]
pub fn fabs(val: f32) -> f32 {
    f32::from_bits(val.to_bits() & 0x7fff_ffff)
}

/// `WWMath::Float_To_Int_Chop`: truncate toward zero with integer bit tricks.
///
/// Upstream shifts by `31 - exponent`, which leaves `[0, 31]` for
/// `|f| >= 2^32` (and inf/NaN). x86 masks a shift count to 5 bits, and so do
/// `wrapping_shl`/`wrapping_shr`; the results for those inputs are the
/// hardware's, not meaningful values.
#[inline]
pub fn float_to_int_chop(f: f32) -> i32 {
    let a = f.to_bits() as i32;
    let sign = a >> 31;
    let mantissa = (a & ((1 << 23) - 1)) | (1 << 23);
    let exponent = ((a & 0x7fff_ffff) >> 23) - 127;
    let r = ((mantissa as u32) << 8).wrapping_shr((31 - exponent) as u32) as i32;
    ((r ^ sign).wrapping_sub(sign)) & !(exponent >> 31)
}

/// `WWMath::Float_To_Int_Floor`: floor with integer bit tricks.
#[inline]
pub fn float_to_int_floor(f: f32) -> i32 {
    let mut a = f.to_bits() as i32;
    let sign = a >> 31;
    a &= 0x7fff_ffff;
    let exponent = (a >> 23) - 127;
    let expsign = !(exponent >> 31);
    let imask = 1i32.wrapping_shl((31 - exponent) as u32).wrapping_sub(1);
    let mantissa = a & ((1 << 23) - 1);
    let mut r = (((mantissa | (1 << 23)) as u32) << 8).wrapping_shr((31 - exponent) as u32) as i32;
    let not_frac: i32 = if (mantissa << 8) & imask == 0 { 1 } else { 0 };
    r = ((r & expsign) ^ sign).wrapping_add((not_frac & (expsign ^ (a.wrapping_sub(1) >> 31))) & sign);
    r
}

/// `WWMath::Cos` (portable branch: `cosf`, petir's `f32` cosine).
#[inline]
pub fn cos(val: f32) -> f32 {
    Real::cos(val)
}

/// `WWMath::Sin` (portable branch: `sinf`, petir's `f32` sine).
#[inline]
pub fn sin(val: f32) -> f32 {
    Real::sin(val)
}

/// `WWMath::Sqrt` (portable branch: `(float)sqrt(val)`, the double `sqrt`).
#[inline]
pub fn sqrt(val: f32) -> f32 {
    Real::sqrt(f64::from(val)) as f32
}

/// `WWMath::Inv_Sqrt` (portable branch: `1.0f / (float)sqrt(val)`).
#[inline]
pub fn inv_sqrt(val: f32) -> f32 {
    1.0f32 / (Real::sqrt(f64::from(val)) as f32)
}

/// `WWMath::Float_To_Long(float)` (portable branch: `(long) f`).
///
/// `long` is 32 bits on the Win32 target. The C++ conversion is undefined for
/// NaN and for values whose truncation does not fit; Rust's `as` saturates
/// there (and gives 0 for NaN), which is deterministic on every target.
#[inline]
pub fn float_to_long(f: f32) -> i32 {
    f as i32
}

/// `WWMath::Float_To_Long(double)` (portable branch: `(long) f`).
#[inline]
pub fn float_to_long_f64(f: f64) -> i32 {
    f as i32
}

/// Upstream's `Float_To_Int_Floor`-based linear interpolation of a 1024-entry
/// sine table, shared by `Fast_Sin` and `Fast_Cos`.
#[inline]
fn sin_table_lerp(val: f32) -> f32 {
    let t = &fast_tables().sin;
    let idx0 = float_to_int_floor(val);
    let idx1 = idx0.wrapping_add(1);
    let frac = val - (idx0 as f32);
    let i0 = ((idx0 as u32) & (SIN_TABLE_SIZE as u32 - 1)) as usize;
    let i1 = ((idx1 as u32) & (SIN_TABLE_SIZE as u32 - 1)) as usize;
    (1.0f32 - frac) * t[i0] + frac * t[i1]
}

/// `float(SIN_TABLE_SIZE) / (2.0f * WWMATH_PI)`.
const SIN_TABLE_SCALE: f32 = (SIN_TABLE_SIZE as f32) / (2.0f32 * WWMATH_PI);

/// `WWMath::Fast_Sin`: table-based sine.
#[inline]
pub fn fast_sin(mut val: f32) -> f32 {
    val *= SIN_TABLE_SCALE;
    sin_table_lerp(val)
}

/// `WWMath::Fast_Inv_Sin` (the active `#else` branch: `1.0f / Fast_Sin(val)`).
#[inline]
pub fn fast_inv_sin(val: f32) -> f32 {
    1.0f32 / fast_sin(val)
}

/// `WWMath::Fast_Cos`: table-based cosine.
#[inline]
pub fn fast_cos(mut val: f32) -> f32 {
    val += WWMATH_PI * 0.5f32;
    val *= SIN_TABLE_SCALE;
    sin_table_lerp(val)
}

/// `WWMath::Fast_Inv_Cos` (the active `#else` branch: `1.0f / Fast_Cos(val)`).
#[inline]
pub fn fast_inv_cos(val: f32) -> f32 {
    1.0f32 / fast_cos(val)
}

/// Shared body of `Fast_Acos` and `Fast_Asin` after the `> 0.975` guard.
///
/// Upstream only `assert`s that the index is inside the table; for a NaN
/// input the index is garbage and the C++ reads out of bounds (undefined
/// behaviour). Here such an index panics instead of reading memory.
#[inline]
fn arc_table_lerp(table: &[f32; ARC_TABLE_SIZE], mut val: f32) -> f32 {
    val *= (ARC_TABLE_SIZE / 2) as f32;
    let mut idx0 = float_to_int_floor(val);
    let mut idx1 = idx0.wrapping_add(1);
    let frac = val - (idx0 as f32);
    idx0 = idx0.wrapping_add((ARC_TABLE_SIZE / 2) as i32);
    idx1 = idx1.wrapping_add((ARC_TABLE_SIZE / 2) as i32);
    let i0 = usize::try_from(idx0).expect("Fast_Acos/Fast_Asin index out of table (NaN input?)");
    let i1 = usize::try_from(idx1).expect("Fast_Acos/Fast_Asin index out of table (NaN input?)");
    (1.0f32 - frac) * table[i0] + frac * table[i1]
}

/// `WWMath::Fast_Acos`: table-based arc cosine, exact `acos` near +-1.
#[inline]
pub fn fast_acos(val: f32) -> f32 {
    if fabs(val) > 0.975f32 {
        return acos(val);
    }
    arc_table_lerp(&fast_tables().acos, val)
}

/// `WWMath::Acos`: `(float)acos(val)`, the double `acos`.
#[inline]
pub fn acos(val: f32) -> f32 {
    Real::acos(f64::from(val)) as f32
}

/// `WWMath::Fast_Asin`: table-based arc sine, exact `asin` near +-1.
#[inline]
pub fn fast_asin(val: f32) -> f32 {
    if fabs(val) > 0.975f32 {
        return asin(val);
    }
    arc_table_lerp(&fast_tables().asin, val)
}

/// `WWMath::Asin`: `(float)asin(val)`, the double `asin`.
#[inline]
pub fn asin(val: f32) -> f32 {
    Real::asin(f64::from(val)) as f32
}

/// `WWMath::Atan`: `static_cast<float>(atan(x))`, the double `atan`.
#[inline]
pub fn atan(x: f32) -> f32 {
    Real::atan(f64::from(x)) as f32
}

/// `WWMath::Atan2`: `static_cast<float>(atan2(y,x))`, the double `atan2`.
#[inline]
pub fn atan2(y: f32, x: f32) -> f32 {
    Real::atan2(f64::from(y), f64::from(x)) as f32
}

/// `WWMath::Sign`: +1, -1 or 0 (also 0 for NaN).
#[inline]
pub fn sign(val: f32) -> f32 {
    if val > 0.0f32 {
        return 1.0f32;
    }
    if val < 0.0f32 {
        return -1.0f32;
    }
    0.0f32
}

/// `WWMath::Ceil`: `ceilf`.
#[inline]
pub fn ceil(val: f32) -> f32 {
    Real::ceil(val)
}

/// `WWMath::Floor`: `floorf`.
#[inline]
pub fn floor(val: f32) -> f32 {
    Real::floor(val)
}

/// `WWMath::Fast_Is_Float_Positive`: sign bit clear (true for +0.0 and +NaN).
#[inline]
pub fn fast_is_float_positive(val: f32) -> bool {
    val.to_bits() & 0x8000_0000 == 0
}

/// `WWMath::Is_Power_Of_2`: `!((val)&val-1)`, which is also true for 0.
#[inline]
pub fn is_power_of_2(val: u32) -> bool {
    val & val.wrapping_sub(1) == 0
}

/// `WWMath::Clamp(float, min = 0, max = 1)`.
#[inline]
pub fn clamp(val: f32, min: f32, max: f32) -> f32 {
    if val < min {
        return min;
    }
    if val > max {
        return max;
    }
    val
}

/// `WWMath::Clamp(double, min = 0, max = 1)`.
#[inline]
pub fn clamp_f64(val: f64, min: f64, max: f64) -> f64 {
    if val < min {
        return min;
    }
    if val > max {
        return max;
    }
    val
}

/// `WWMath::Clamp_Int`.
#[inline]
pub fn clamp_int(val: i32, min_val: i32, max_val: i32) -> i32 {
    if val < min_val {
        return min_val;
    }
    if val > max_val {
        return max_val;
    }
    val
}

/// `WWMath::Wrap(float, min = 0, max = 1)`: one step of wrapping, then clamp.
#[inline]
pub fn wrap(mut val: f32, min: f32, max: f32) -> f32 {
    if val >= max {
        val -= max - min;
    }
    if val < min {
        val += max - min;
    }
    if val < min {
        val = min;
    }
    if val > max {
        val = max;
    }
    val
}

/// `WWMath::Wrap(double, min = 0, max = 1)`.
#[inline]
pub fn wrap_f64(mut val: f64, min: f64, max: f64) -> f64 {
    if val >= max {
        val -= max - min;
    }
    if val < min {
        val += max - min;
    }
    if val < min {
        val = min;
    }
    if val > max {
        val = max;
    }
    val
}

/// `WWMath::Min`: `a < b ? a : b` (returns `b` when either is NaN).
#[inline]
pub fn min(a: f32, b: f32) -> f32 {
    if a < b {
        return a;
    }
    b
}

/// `WWMath::Max`: `a > b ? a : b` (returns `b` when either is NaN).
#[inline]
pub fn max(a: f32, b: f32) -> f32 {
    if a > b {
        return a;
    }
    b
}

/// `WWMath::Float_As_Int`: the bit pattern.
#[inline]
pub fn float_as_int(f: f32) -> i32 {
    f.to_bits() as i32
}

/// `WWMath::Lerp(float, float, float)`: `a + (b - a)*lerp`.
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `WWMath::Lerp(double, double, float)`: the `float` factor is widened.
#[inline]
pub fn lerp_f64(a: f64, b: f64, t: f32) -> f64 {
    a + (b - a) * f64::from(t)
}

/// `WWMath::Unit_Float_To_Byte`: `(unsigned char)(f*255.0f)`.
///
/// The C++ conversion is undefined when `f*255` is outside `(-1, 256)`;
/// Rust's `as` saturates there, deterministically.
#[inline]
pub fn unit_float_to_byte(f: f32) -> u8 {
    (f * 255.0f32) as u8
}

/// `WWMath::Byte_To_Unit_Float`: `((float)byte) / 255.0f`.
#[inline]
pub fn byte_to_unit_float(byte: u8) -> f32 {
    f32::from(byte) / 255.0f32
}

/// `WWMath::Is_Valid_Float`: false when the exponent is all ones (inf or NaN).
#[inline]
pub fn is_valid_float(x: f32) -> bool {
    let exponent = (x.to_bits() & 0x7F80_0000) >> (32 - 9);
    exponent != 0xFF
}

/// `WWMath::Is_Valid_Double`: false when the exponent is all ones.
///
/// Upstream reads the high 32-bit word as `*((unsigned long *)&x + 1)`, which
/// is the high word only where `long` is 32 bits (Win32). This translation
/// takes the high word explicitly. (On an LP64 host the C++ reads past the
/// double, so this function has no compiled reference; see the unit test.)
#[inline]
pub fn is_valid_double(x: f64) -> bool {
    let hi = (x.to_bits() >> 32) as u32;
    let exponent = (hi & 0x7FF0_0000) >> (32 - 12);
    exponent != 0x7FF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_double_cases() {
        assert!(is_valid_double(0.0));
        assert!(is_valid_double(-1.0e300));
        assert!(is_valid_double(f64::MIN_POSITIVE / 2.0));
        assert!(!is_valid_double(f64::INFINITY));
        assert!(!is_valid_double(f64::NEG_INFINITY));
        assert!(!is_valid_double(f64::NAN));
    }

    #[test]
    fn small_cases() {
        assert_eq!(float_to_int_floor(-1.5), -2);
        assert_eq!(float_to_int_floor(-1.0), -1);
        assert_eq!(float_to_int_chop(-1.5), -1);
        assert!(is_power_of_2(0));
        assert!(!is_power_of_2(6));
        assert_eq!(fast_tables().inv_sin[0], f32::MAX);
        assert!((fast_sin(1.0) - 0.841_470_96).abs() < 1e-4);
    }
}
