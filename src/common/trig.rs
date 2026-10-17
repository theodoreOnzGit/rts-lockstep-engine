// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/Trig.cpp
//   GeneralsMD/Code/Libraries/Include/Lib/trig.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Authors: Michael S. Booth, March 1994 (Trig.cpp); Sondra Iverson,
//   March 1998 (trig.h); converted to Generals by Matthew D. Campbell,
//   February 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The game-logic trig functions `Sin`, `Cos`, `Tan`, `ACos`, `ASin`
//! declared in `Lib/trig.h` (about 120 call sites in `GameLogic`).
//!
//! # Two function sets, because upstream has two
//!
//! `Trig.cpp` writes `#define DEFAULT_TRIG` just above the functions, so in
//! the shipped build every one of them is the C `float` library call:
//! `Sin` = `sinf`, `Cos` = `cosf`, `Tan` = `tanf`, `ACos` = `acosf`,
//! `ASin` = `asinf`. That **active** path is the top-level set here
//! ([`sin`], [`cos`], [`tan`], [`acos`], [`asin`]), on petir's `f32` `Real`.
//!
//! The `#else` branches, which the shipped build does not compile, evaluate
//! the functions from two fixed-point tables (12 fractional bits) that
//! upstream ships as data. They are translated as the separate set
//! [`table`] (with the integer kernels [`int_sin`], [`int_cos`],
//! [`int_tan`], [`int_arc_cos`]), so a later decision to switch the logic to
//! the integer path (which needs no elementary-function library at all) is a
//! one-line change that is already verified. `ASin` has no table branch
//! upstream; [`table::asin`] is `asinf`, as upstream.
//!
//! # Float-to-int conversions
//!
//! The table path converts `x * 4096` to `Int` with a C cast, which is
//! undefined in C++ when the truncated value does not fit 32 bits (and for
//! NaN). Rust's `as` saturates there (NaN gives 0), deterministically on
//! every target. The differential tests compare only the defined domain.
//!
//! `REGENERATE_TRIG_TABLES` (code that rebuilt the tables with the C library)
//! is not translated: the tables are data, see [`super::trig_tables`].

use super::trig_tables::{ARC_COS_LOOKUP, SIN_LOOKUP};
use petir::Real;

/// `#define TWOPI 6.28318530718f`.
#[allow(clippy::approx_constant, clippy::excessive_precision)]
pub const TWOPI: f32 = 6.28318530718;
/// `#define DEG2RAD 0.0174532925199f` (defined, unused, upstream).
#[allow(clippy::excessive_precision)]
pub const DEG2RAD: f32 = 0.0174532925199;
/// `#define TRIG_RES 4096`: entries in the sine table.
pub const TRIG_RES: i32 = 4096;
/// `#define INT_ONE 4096`: 1.0 in 12-bit fixed point.
pub const INT_ONE: i32 = 4096;
/// `#define INT_TWOPI 25736`.
pub const INT_TWOPI: i32 = 25736;
/// `#define INT_THREEPIOVERTWO 19302` (defined, unused, upstream).
pub const INT_THREEPIOVERTWO: i32 = 19302;
/// `#define INT_PI 12868` (defined, unused, upstream).
pub const INT_PI: i32 = 12868;
/// `#define INT_HALFPI 6434`.
pub const INT_HALFPI: i32 = 6434;

/// `Real Sin(Real x)`, active `DEFAULT_TRIG` branch: `sinf(x)`.
#[inline]
pub fn sin(x: f32) -> f32 {
    Real::sin(x)
}

/// `Real Cos(Real x)`, active `DEFAULT_TRIG` branch: `cosf(x)`.
#[inline]
pub fn cos(x: f32) -> f32 {
    Real::cos(x)
}

/// `Real Tan(Real x)`, active `DEFAULT_TRIG` branch: `tanf(x)`.
#[inline]
pub fn tan(x: f32) -> f32 {
    Real::tan(x)
}

/// `Real ACos(Real x)`, active `DEFAULT_TRIG` branch: `acosf(x)`.
#[inline]
pub fn acos(x: f32) -> f32 {
    Real::acos(x)
}

/// `Real ASin(Real x)`: `asinf(x)` (upstream has no other branch).
#[inline]
pub fn asin(x: f32) -> f32 {
    Real::asin(x)
}

/// `static Int intArcCos(Int c)`: arc cosine of a 12-bit fixed-point value,
/// clamped to the table, as a fixed-point angle (`INT_TWOPI` = 2 pi).
///
/// `c += INT_ONE` can overflow for `c` near `i32::MAX`; it wraps (as on the
/// x86 target and under the reference build's `-fwrapv`), then clamps to 0.
#[inline]
pub fn int_arc_cos(mut c: i32) -> i32 {
    c = c.wrapping_add(INT_ONE);
    if c < 0 {
        c = 0;
    }
    if c >= 2 * INT_ONE {
        c = (2 * INT_ONE) - 1;
    }
    ARC_COS_LOOKUP[c as usize] as i32
}

/// `static Int intSin(Int angle)`: fixed-point sine of a fixed-point angle.
///
/// Upstream reduces the angle with two loops (`while (angle < 0) angle +=
/// INT_TWOPI; while (angle >= INT_TWOPI) angle -= INT_TWOPI;`). Their result
/// is exactly the Euclidean remainder, which is computed directly here (the
/// loops take up to ~83,000 iterations for large angles; neither overflows).
#[inline]
pub fn int_sin(angle: i32) -> i32 {
    let angle = angle.rem_euclid(INT_TWOPI);
    SIN_LOOKUP[((angle * TRIG_RES) / INT_TWOPI) as usize] as i32
}

/// `#define intCos(x) intSin( INT_HALFPI - (x) )`.
///
/// The subtraction overflows for `x` near `i32::MIN`; it wraps, as on the
/// x86 target and in the `-fwrapv` reference build.
#[inline]
pub fn int_cos(x: i32) -> i32 {
    int_sin(INT_HALFPI.wrapping_sub(x))
}

/// `static Int intTan(Int angle)`: `INT_ONE * s / c`, or `INT_MAX` /
/// `INT_MIN` when the cosine entry is 0 (by the sign of the sine).
#[inline]
pub fn int_tan(angle: i32) -> i32 {
    let s = int_sin(angle);
    let c = int_cos(angle);
    if c == 0 {
        if s > 0 {
            return i32::MAX;
        } else {
            return i32::MIN;
        }
    }
    INT_ONE * s / c
}

/// The table-based `#else` branches of `Trig.cpp` (not compiled in the
/// shipped build; see the module docs).
pub mod table {
    use super::*;

    /// `(Int)((x) * INT_ONE)`: float multiply, then C truncation (saturating
    /// here; undefined upstream outside the 32-bit range).
    #[inline]
    fn to_fixed(x: f32) -> i32 {
        (x * INT_ONE as f32) as i32
    }

    /// `Sin`, `#else` branch: `(Real)intSin((Int)((x) * INT_ONE)) / (Real)INT_ONE`.
    #[inline]
    pub fn sin(x: f32) -> f32 {
        int_sin(to_fixed(x)) as f32 / INT_ONE as f32
    }

    /// `Cos`, `#else` branch: `(Real)intCos((Int)((x) * INT_ONE)) / (Real)INT_ONE`.
    #[inline]
    pub fn cos(x: f32) -> f32 {
        int_cos(to_fixed(x)) as f32 / INT_ONE as f32
    }

    /// `Tan`, `#else` branch: `(Real)intTan((Int)((x) * INT_ONE)) / (Real)INT_ONE`.
    #[inline]
    pub fn tan(x: f32) -> f32 {
        int_tan(to_fixed(x)) as f32 / INT_ONE as f32
    }

    /// `ACos`, `#else` branch:
    /// `(Real)(intArcCos((Int)((x) * INT_ONE))) * TWOPI / (Real)INT_TWOPI`.
    #[inline]
    pub fn acos(x: f32) -> f32 {
        int_arc_cos(to_fixed(x)) as f32 * TWOPI / INT_TWOPI as f32
    }

    /// `ASin`: upstream has no table branch, so this is `asinf(x)` too.
    #[inline]
    pub fn asin(x: f32) -> f32 {
        super::asin(x)
    }
}

/// Entry `i` of upstream's `sinLookup` (for tests and inspection).
pub fn sin_lookup(i: usize) -> i32 {
    SIN_LOOKUP[i] as i32
}

/// Entry `i` of upstream's `arcCosLookup` (for tests and inspection).
pub fn arc_cos_lookup(i: usize) -> i32 {
    ARC_COS_LOOKUP[i] as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_point_values() {
        assert_eq!(int_sin(0), 0);
        // A quarter turn is entry 1024 of 4096. Upstream generated it as
        // `(Int)(sin(angle) * INT_ONE)` with a float angle, which truncates
        // to one below 1.0 in 12-bit fixed point.
        assert_eq!(int_sin(INT_HALFPI), sin_lookup(1024));
        assert_eq!(sin_lookup(1024), INT_ONE - 1);
        assert_eq!(int_sin(-INT_TWOPI), int_sin(0));
        assert_eq!(int_cos(0), int_sin(INT_HALFPI));
        assert_eq!(int_arc_cos(i32::MAX), arc_cos_lookup(0), "wraps, then clamps to 0");
        assert_eq!(int_arc_cos(i32::MIN), arc_cos_lookup(0));
        assert_eq!(int_arc_cos(1 << 20), arc_cos_lookup(8191));
        assert!((table::sin(1.0) - 0.841_470_96).abs() < 2e-3);
        assert!((table::acos(0.0) - std::f32::consts::FRAC_PI_2).abs() < 2e-3);
        assert_eq!(table::asin(0.5), asin(0.5));
    }
}
