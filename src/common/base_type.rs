// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Include/Lib/BaseType.h (lines 175-245: the fast
//   float helpers and the REAL_TO_* macros)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Michael S. Booth, January 1995, September 2000
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The fast float-to-integer helpers behind `REAL_TO_INT`, `REAL_TO_INT_FLOOR`
//! and friends, which GameLogic uses widely (21 files in
//! `GameEngine/Source/GameLogic` use `REAL_TO_INT*`).
//!
//! Upstream these are MSVC `__asm` blocks with **no C++ fallback**. Each is
//! rewritten here in plain Rust from the instruction semantics, including the
//! quirks of the original sequences, which are kept on purpose:
//!
//! - [`fast_float_trunc`] maps every `|f| < 1` to `+0.0` (the sign is lost),
//!   and for exponents >= 32 (|f| >= 2^32, inf, NaN) the x86 `sar` masks its
//!   count to 5 bits, so the mask wraps (e.g. NaN becomes infinity);
//! - [`fast_float_floor`] subtracts `0.99999994f` from negatives before
//!   truncating, so a negative *integer* is pushed down by one
//!   (`fast_float_floor(-1.0) == -2.0`) whenever the subtraction rounds up to
//!   the next integer;
//! - [`fast_float2long_round`] is `fld`/`fistp` into a 32-bit `long`:
//!   round-to-nearest-even (the mode the game sets in `setFPMode`, `_RC_NEAR`),
//!   and the x87 "integer indefinite" `0x80000000` for NaN and out-of-range.
//!
//! The differential tests compare these against GNU-asm transcriptions of the
//! same instruction sequences (`differential/wrap_base_type.cpp`).

/// `almost1 = (126<<23)|0x7fffff`, the largest float below 1.0 (`0.99999994f`).
const ALMOST1: f32 = f32::from_bits((126 << 23) | 0x7f_ffff);

/// `__forceinline long fast_float2long_round(float f)`:
/// `fld [f]` / `fistp [i]` with the x87 in round-to-nearest mode.
///
/// Rounds half to even. NaN, and any value whose rounded result does not fit
/// a 32-bit signed integer, gives `i32::MIN` (`0x80000000`), as x87 `fistp`
/// does with the invalid-operation exception masked.
#[inline]
pub fn fast_float2long_round(f: f32) -> i32 {
    if f.is_nan() {
        return i32::MIN;
    }
    let r = libm::rintf(f);
    // Every f32 in [-2^31, 2^31) is exactly representable as f64, and 2^31
    // itself is the first value that does not fit.
    if (r as f64) < -2_147_483_648.0 || (r as f64) >= 2_147_483_648.0 {
        return i32::MIN;
    }
    r as i32
}

/// `__forceinline float fast_float_trunc(float f)`, from its asm:
///
/// ```text
/// mov ecx,[f] ; shr ecx,23 ; mov eax,0xff800000 ; xor ebx,ebx
/// sub cl,127  ; cmovc eax,ebx ; sar eax,cl ; and [f],eax
/// ```
#[inline]
pub fn fast_float_trunc(f: f32) -> f32 {
    let bits = f.to_bits();
    let ecx = bits >> 23; // sign bit lands in bit 8, exponent in bits 0-7
    let cl_before = ecx as u8; // the biased exponent
    let carry = cl_before < 127; // `sub cl,127` borrows
    let cl = cl_before.wrapping_sub(127);
    let eax: i32 = if carry { 0 } else { 0xff80_0000u32 as i32 };
    // `sar eax,cl`: x86 masks a 32-bit shift count to its low 5 bits.
    let mask = eax.wrapping_shr(u32::from(cl));
    f32::from_bits(bits & (mask as u32))
}

/// `__forceinline float fast_float_floor(float f)`.
#[inline]
pub fn fast_float_floor(mut f: f32) -> f32 {
    if f.to_bits() & 0x8000_0000 != 0 {
        f -= ALMOST1;
    }
    fast_float_trunc(f)
}

/// `__forceinline float fast_float_ceil(float f)`.
#[inline]
pub fn fast_float_ceil(mut f: f32) -> f32 {
    if f.to_bits() & 0x8000_0000 == 0 {
        f += ALMOST1;
    }
    fast_float_trunc(f)
}

/// `REAL_TO_INT(x)`: `(Int)(fast_float2long_round(fast_float_trunc(x)))`.
#[inline]
pub fn real_to_int(x: f32) -> i32 {
    fast_float2long_round(fast_float_trunc(x))
}

/// `REAL_TO_UNSIGNEDINT(x)`: the same, reinterpreted as unsigned.
#[inline]
pub fn real_to_unsigned_int(x: f32) -> u32 {
    fast_float2long_round(fast_float_trunc(x)) as u32
}

/// `REAL_TO_SHORT(x)`: truncated to 16 bits (modular, as the MSVC cast).
#[inline]
pub fn real_to_short(x: f32) -> i16 {
    fast_float2long_round(fast_float_trunc(x)) as i16
}

/// `REAL_TO_UNSIGNEDSHORT(x)`.
#[inline]
pub fn real_to_unsigned_short(x: f32) -> u16 {
    fast_float2long_round(fast_float_trunc(x)) as u16
}

/// `REAL_TO_BYTE(x)` and `REAL_TO_CHAR(x)`: `Byte` and `Char` are `char`,
/// which is signed on MSVC.
#[inline]
pub fn real_to_byte(x: f32) -> i8 {
    fast_float2long_round(fast_float_trunc(x)) as i8
}

/// `REAL_TO_UNSIGNEDBYTE(x)`.
#[inline]
pub fn real_to_unsigned_byte(x: f32) -> u8 {
    fast_float2long_round(fast_float_trunc(x)) as u8
}

/// `DOUBLE_TO_INT(x)`: the double is first converted to the `float`
/// parameter of `fast_float_trunc` (round to nearest).
#[inline]
pub fn double_to_int(x: f64) -> i32 {
    fast_float2long_round(fast_float_trunc(x as f32))
}

/// `DOUBLE_TO_REAL(x)`: `(Real)(x)`.
#[inline]
pub fn double_to_real(x: f64) -> f32 {
    x as f32
}

/// `INT_TO_REAL(x)`: `(Real)(x)`.
#[inline]
pub fn int_to_real(x: i32) -> f32 {
    x as f32
}

/// `REAL_TO_INT_CEIL(x)`: `fast_float2long_round(fast_float_ceil(x))`.
#[inline]
pub fn real_to_int_ceil(x: f32) -> i32 {
    fast_float2long_round(fast_float_ceil(x))
}

/// `REAL_TO_INT_FLOOR(x)`: `fast_float2long_round(fast_float_floor(x))`.
#[inline]
pub fn real_to_int_floor(x: f32) -> i32 {
    fast_float2long_round(fast_float_floor(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_quirks() {
        assert_eq!(fast_float_trunc(-0.5).to_bits(), 0.0f32.to_bits());
        assert_eq!(fast_float_trunc(-2.75), -2.0);
        assert_eq!(fast_float_floor(-1.0), -2.0, "upstream floor quirk on negative integers");
        assert_eq!(fast_float_floor(-1.5), -2.0);
        assert_eq!(fast_float_ceil(1.5), 2.0);
        assert_eq!(fast_float2long_round(2.5), 2);
        assert_eq!(fast_float2long_round(3.5), 4);
        assert_eq!(fast_float2long_round(f32::NAN), i32::MIN);
        assert_eq!(fast_float2long_round(3.0e9), i32::MIN);
        assert_eq!(real_to_int(-7.9), -7);
        assert_eq!(real_to_int_floor(-7.1), -8);
        assert_eq!(real_to_int_ceil(7.1), 8);
        assert!(fast_float_trunc(f32::NAN).is_infinite());
        assert_eq!(double_to_int(12.7), 12);
        assert_eq!(real_to_unsigned_byte(300.0), 44);
        assert_eq!(real_to_byte(200.0), -56);
        assert_eq!(real_to_short(40000.0), -25536);
        assert_eq!(real_to_unsigned_short(-1.0), 65535);
        assert_eq!(real_to_unsigned_int(-1.0), u32::MAX);
        assert_eq!(double_to_real(0.1), 0.1f32);
        assert_eq!(int_to_real(16_777_217), 16_777_216.0);
    }
}
