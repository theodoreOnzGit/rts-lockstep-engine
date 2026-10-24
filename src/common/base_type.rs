// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Include/Lib/BaseType.h (lines 175-245: the fast
//   float helpers and the REAL_TO_* macros; lines 85-88: PI, TWO_PI;
//   lines 100-106: min/max; lines 142-167: sqr, clamp, sign, rad2deg,
//   deg2rad; lines 258-492: the derived types and their methods)
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
//!
//! The derived value types (`Coord2D`, `Coord3D`, `Region3D`, `RGBColor`,
//! ...) and the small templates (`sqr`, `clamp`, `sign`, `rad2deg`,
//! `deg2rad`) follow BaseType.h operation for operation; their differential
//! test compiles the header's own text (`differential/wrap_value_types.cpp`).
//!
//! # Where upstream is undefined
//!
//! The integer methods (`ICoord2D::length`, `IRegion2D::width`, ...) overflow
//! a signed `Int` for large coordinates, and `RGBColor::getAsInt` converts a
//! `double` to `Int` and shifts it, which is undefined for components outside
//! the representable range or below zero. The `try_*` methods return
//! [`BaseTypeError`] there; the plain ones panic with that error. On every
//! input where upstream is defined they return upstream's value.

/// `#define PI 3.14159265359f` (BaseType.h line 86).
#[allow(clippy::approx_constant, clippy::excessive_precision)]
pub const PI: f32 = 3.14159265359;
/// `#define TWO_PI 6.28318530718f` (BaseType.h line 87).
#[allow(clippy::approx_constant, clippy::excessive_precision)]
pub const TWO_PI: f32 = 6.28318530718;

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
    // x87 round-to-nearest is IEEE roundToIntegralTiesToEven, an exact
    // operation (no rounding error, identical bits on every target, like
    // `abs`). petir has no `rint`, so this is Rust's `f32::round_ties_even`.
    let r = f.round_ties_even();
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

// ---------------------------------------------------------------------------
// The small templates (BaseType.h lines 142-167) and min/max (lines 100-106).
// ---------------------------------------------------------------------------

/// Where BaseType.h's integer and conversion code is undefined in C++ (see
/// the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BaseTypeError {
    /// Signed `Int` arithmetic overflowed.
    SignedOverflow,
    /// A `double` converted to `Int` does not fit (or is NaN).
    FloatToIntOutOfRange,
    /// `<<` of a negative `Int`, or a result that does not fit 32 bits.
    ShiftUndefined,
}

impl std::fmt::Display for BaseTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let what = match self {
            BaseTypeError::SignedOverflow => "signed integer overflow (undefined behaviour upstream)",
            BaseTypeError::FloatToIntOutOfRange => "float-to-int conversion out of range (undefined behaviour upstream)",
            BaseTypeError::ShiftUndefined => "left shift of a negative value or past 32 bits (undefined behaviour upstream)",
        };
        f.write_str(what)
    }
}

fn ok_or_panic<T>(r: Result<T, BaseTypeError>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

/// `#define min(x,y) (((x)<(y)) ? (x) : (y))`: with a NaN operand it
/// returns `y`.
#[inline]
pub fn min<T: PartialOrd>(x: T, y: T) -> T {
    if x < y { x } else { y }
}

/// `#define max(x,y) (((x)>(y)) ? (x) : (y))`: with a NaN operand it
/// returns `y`.
#[inline]
pub fn max<T: PartialOrd>(x: T, y: T) -> T {
    if x > y { x } else { y }
}

/// `sqr(x)` for `Real`: `x*x`.
#[inline]
pub fn sqr(x: f32) -> f32 {
    x * x
}

/// `sqr(x)` for `Int`; `x*x` overflows (undefined) for `|x| > 46340`.
pub fn try_sqr_int(x: i32) -> Result<i32, BaseTypeError> {
    x.checked_mul(x).ok_or(BaseTypeError::SignedOverflow)
}

/// `clamp(lo, val, hi)`: `val < lo ? lo : (val > hi ? hi : val)`, so a NaN
/// `val` passes through and `lo > hi` returns `lo` for `val < lo`.
#[inline]
pub fn clamp<T: PartialOrd>(lo: T, val: T, hi: T) -> T {
    if val < lo {
        lo
    } else if val > hi {
        hi
    } else {
        val
    }
}

/// `sign(x)`: 1, -1 or 0 (also 0 for `-0.0` and NaN).
#[inline]
pub fn sign<T: PartialOrd + Default>(x: T) -> i32 {
    let zero = T::default();
    if x > zero {
        1
    } else if x < zero {
        -1
    } else {
        0
    }
}

/// `180/PI` as upstream's expression evaluates it: `int / float`, in `float`.
const RAD2DEG: f32 = 180.0 / PI;
/// `PI/180`, in `float`.
const DEG2RAD: f32 = PI / 180.0;

/// `rad2deg(rad)`: `rad * (180/PI)`.
#[inline]
pub fn rad2deg(rad: f32) -> f32 {
    rad * RAD2DEG
}

/// `deg2rad(rad)`: `rad * (PI/180)`.
#[inline]
pub fn deg2rad(rad: f32) -> f32 {
    rad * DEG2RAD
}

/// `(Real)sqrt(v)` of a `float` sum: C's double `sqrt`, rounded back once.
#[inline]
fn sqrt_via_double(v: f32) -> f32 {
    petir::Real::sqrt(f64::from(v)) as f32
}

/// `(Int)sqrt((double)sum)` of a non-negative `Int`: truncates.
#[inline]
fn int_sqrt_via_double(v: i32) -> i32 {
    // sqrt of an Int below 2^31 is below 46341, so the cast is defined.
    petir::Real::sqrt(f64::from(v)) as i32
}

// ---------------------------------------------------------------------------
// Derived types (BaseType.h lines 258-492): the data members in upstream's
// order, and their methods.
// ---------------------------------------------------------------------------

/// `struct RealRange { Real lo, hi; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RealRange {
    pub lo: f32,
    pub hi: f32,
}

/// `struct Coord2D { Real x, y; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

/// `struct ICoord2D { Int x, y; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

/// `struct Region2D { Coord2D lo, hi; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Region2D {
    pub lo: Coord2D,
    pub hi: Coord2D,
}

/// `struct IRegion2D { ICoord2D lo, hi; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

/// `struct Coord3D { Real x, y, z; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// `struct ICoord3D { Int x, y, z; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ICoord3D {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// `struct Region3D { Coord3D lo, hi; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

/// `struct IRegion3D { ICoord3D lo, hi; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IRegion3D {
    pub lo: ICoord3D,
    pub hi: ICoord3D,
}

/// `struct RGBColor { Real red, green, blue; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RGBColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

/// `struct RGBAColorReal { Real red, green, blue, alpha; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RGBAColorReal {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

/// `struct RGBAColorInt { UnsignedInt red, green, blue, alpha; }`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RGBAColorInt {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub alpha: u32,
}


impl RealRange {
    /// `combine(other)`: `lo = min(lo, other.lo); hi = max(hi, other.hi);`
    pub fn combine(&mut self, other: &RealRange) {
        self.lo = min(self.lo, other.lo);
        self.hi = max(self.hi, other.hi);
    }
}

impl Coord2D {
    /// `length()`: `(Real)sqrt(x*x + y*y)`, the sum in `float`.
    pub fn length(&self) -> f32 {
        sqrt_via_double(self.x * self.x + self.y * self.y)
    }

    /// `normalize()`: divides by the length unless it is 0.
    pub fn normalize(&mut self) {
        let len = self.length();
        if len != 0.0 {
            self.x /= len;
            self.y /= len;
        }
    }

    /// `toAngle()`: 0 for a zero vector, else `ACos(x/len)` (clamped to
    /// [-1, 1]), negated when `y < 0`. `ACos` is `Trig.cpp`'s
    /// ([`super::trig::acos`]).
    pub fn to_angle(&self) -> f32 {
        let len = self.length();
        if len == 0.0 {
            return 0.0;
        }
        // `if (c < -1.0f) c = -1.0f; else if (c > 1.0f) c = 1.0f;`: the same
        // branches as `clamp`.
        let c = clamp(-1.0, self.x / len, 1.0);
        if self.y < 0.0 {
            -super::trig::acos(c)
        } else {
            super::trig::acos(c)
        }
    }
}

/// `x*x + y*y (+ z*z)` in `Int`, checked.
fn int_sum_of_squares(v: &[i32]) -> Result<i32, BaseTypeError> {
    let mut acc: i32 = 0;
    for (i, &c) in v.iter().enumerate() {
        let sq = try_sqr_int(c)?;
        acc = if i == 0 { sq } else { acc.checked_add(sq).ok_or(BaseTypeError::SignedOverflow)? };
    }
    Ok(acc)
}

impl ICoord2D {
    /// `length()`: `(Int)sqrt((double)(x*x + y*y))`, checked for the
    /// `Int` overflow of the sum (undefined upstream).
    pub fn try_length(&self) -> Result<i32, BaseTypeError> {
        Ok(int_sqrt_via_double(int_sum_of_squares(&[self.x, self.y])?))
    }

    /// `length()`; panics where upstream is undefined.
    pub fn length(&self) -> i32 {
        ok_or_panic(self.try_length())
    }
}

impl Region2D {
    /// `width()`: `hi.x - lo.x`.
    pub fn width(&self) -> f32 {
        self.hi.x - self.lo.x
    }

    /// `height()`: `hi.y - lo.y`.
    pub fn height(&self) -> f32 {
        self.hi.y - self.lo.y
    }
}

fn int_sub(a: i32, b: i32) -> Result<i32, BaseTypeError> {
    a.checked_sub(b).ok_or(BaseTypeError::SignedOverflow)
}

impl IRegion2D {
    /// `width()`: `hi.x - lo.x`, checked.
    pub fn try_width(&self) -> Result<i32, BaseTypeError> {
        int_sub(self.hi.x, self.lo.x)
    }

    /// `height()`: `hi.y - lo.y`, checked.
    pub fn try_height(&self) -> Result<i32, BaseTypeError> {
        int_sub(self.hi.y, self.lo.y)
    }

    /// `width()`; panics where upstream is undefined.
    pub fn width(&self) -> i32 {
        ok_or_panic(self.try_width())
    }

    /// `height()`; panics where upstream is undefined.
    pub fn height(&self) -> i32 {
        ok_or_panic(self.try_height())
    }
}

impl Coord3D {
    /// `length()`: `(Real)sqrt(x*x + y*y + z*z)`, the sum in `float`, left to
    /// right.
    pub fn length(&self) -> f32 {
        sqrt_via_double(self.length_sqr())
    }

    /// `lengthSqr()`: `x*x + y*y + z*z`.
    pub fn length_sqr(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// `normalize()`: divides by the length unless it is 0.
    pub fn normalize(&mut self) {
        let len = self.length();
        if len != 0.0 {
            self.x /= len;
            self.y /= len;
            self.z /= len;
        }
    }

    /// `static crossProduct(a, b, r)`, returning `r`.
    pub fn cross_product(a: &Coord3D, b: &Coord3D) -> Coord3D {
        Coord3D {
            x: a.y * b.z - a.z * b.y,
            y: a.z * b.x - a.x * b.z,
            z: a.x * b.y - a.y * b.x,
        }
    }

    /// `zero()`.
    pub fn zero(&mut self) {
        *self = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
    }

    /// `add(a)`.
    pub fn add(&mut self, a: &Coord3D) {
        self.x += a.x;
        self.y += a.y;
        self.z += a.z;
    }

    /// `sub(a)`.
    pub fn sub(&mut self, a: &Coord3D) {
        self.x -= a.x;
        self.y -= a.y;
        self.z -= a.z;
    }

    /// `set(ax, ay, az)` (and `set(const Coord3D*)` is assignment).
    pub fn set(&mut self, ax: f32, ay: f32, az: f32) {
        *self = Coord3D { x: ax, y: ay, z: az };
    }

    /// `scale(s)`.
    pub fn scale(&mut self, scale: f32) {
        self.x *= scale;
        self.y *= scale;
        self.z *= scale;
    }

    /// `equals(r)` and `operator==`: component `==` (so `-0 == 0`, and a
    /// NaN component is never equal). The derived `PartialEq` is the same.
    pub fn equals(&self, r: &Coord3D) -> bool {
        self.x == r.x && self.y == r.y && self.z == r.z
    }
}

impl ICoord3D {
    /// `length()`: `(Int)sqrt((double)(x*x + y*y + z*z))`, checked for the
    /// `Int` overflow of the sum (undefined upstream).
    pub fn try_length(&self) -> Result<i32, BaseTypeError> {
        Ok(int_sqrt_via_double(int_sum_of_squares(&[self.x, self.y, self.z])?))
    }

    /// `length()`; panics where upstream is undefined.
    pub fn length(&self) -> i32 {
        ok_or_panic(self.try_length())
    }

    /// `zero()`.
    pub fn zero(&mut self) {
        *self = ICoord3D { x: 0, y: 0, z: 0 };
    }
}

impl Region3D {
    /// `width()`: `hi.x - lo.x`.
    pub fn width(&self) -> f32 {
        self.hi.x - self.lo.x
    }

    /// `height()`: `hi.y - lo.y`.
    pub fn height(&self) -> f32 {
        self.hi.y - self.lo.y
    }

    /// `depth()`: `hi.z - lo.z`.
    pub fn depth(&self) -> f32 {
        self.hi.z - self.lo.z
    }

    /// `zero()`.
    pub fn zero(&mut self) {
        self.lo.zero();
        self.hi.zero();
    }

    /// `isInRegionNoZ(query)`: strictly inside in x and y.
    pub fn is_in_region_no_z(&self, query: &Coord3D) -> bool {
        (self.lo.x < query.x) && (query.x < self.hi.x) && (self.lo.y < query.y) && (query.y < self.hi.y)
    }

    /// `isInRegionWithZ(query)`: strictly inside in x, y and z.
    pub fn is_in_region_with_z(&self, query: &Coord3D) -> bool {
        (self.lo.x < query.x)
            && (query.x < self.hi.x)
            && (self.lo.y < query.y)
            && (query.y < self.hi.y)
            && (self.lo.z < query.z)
            && (query.z < self.hi.z)
    }
}

impl IRegion3D {
    /// `width()`: `hi.x - lo.x`, checked.
    pub fn try_width(&self) -> Result<i32, BaseTypeError> {
        int_sub(self.hi.x, self.lo.x)
    }

    /// `height()`: `hi.y - lo.y`, checked.
    pub fn try_height(&self) -> Result<i32, BaseTypeError> {
        int_sub(self.hi.y, self.lo.y)
    }

    /// `depth()`: `hi.z - lo.z`, checked.
    pub fn try_depth(&self) -> Result<i32, BaseTypeError> {
        int_sub(self.hi.z, self.lo.z)
    }
}

/// `(Int)(c * 255.0) << shift`: the component is widened to `double`
/// (`255.0` is a double literal), multiplied, truncated to `Int`, shifted.
fn color_component(c: f32, shift: u32) -> Result<i32, BaseTypeError> {
    let v = f64::from(c) * 255.0;
    // Truncation is defined only when the result fits Int.
    if !(v > -2_147_483_649.0 && v < 2_147_483_648.0) {
        return Err(BaseTypeError::FloatToIntOutOfRange);
    }
    let i = v as i32;
    // C++17 [expr.shift]: E1 << E2 for signed E1 is defined only for E1 >= 0
    // with E1 * 2^E2 representable in the unsigned type; the value is then
    // converted to Int (modular, as on every target this runs on).
    if i < 0 || (i as u64) << shift > u64::from(u32::MAX) {
        return Err(BaseTypeError::ShiftUndefined);
    }
    Ok(((i as u32) << shift) as i32)
}

impl RGBColor {
    /// `getAsInt()`: `((Int)(red*255.0) << 16) | ((Int)(green*255.0) << 8) |
    /// (Int)(blue*255.0)`, checked where upstream is undefined.
    pub fn try_get_as_int(&self) -> Result<i32, BaseTypeError> {
        Ok(color_component(self.red, 16)? | color_component(self.green, 8)? | color_component(self.blue, 0)?)
    }

    /// `getAsInt()`; panics where upstream is undefined.
    pub fn get_as_int(&self) -> i32 {
        ok_or_panic(self.try_get_as_int())
    }

    /// `setFromInt(c)`: each byte divided by `255.0f` (the `>>` of a negative
    /// `Int` is arithmetic, then masked).
    pub fn set_from_int(&mut self, c: i32) {
        self.red = ((c >> 16) & 0xff) as f32 / 255.0;
        self.green = ((c >> 8) & 0xff) as f32 / 255.0;
        self.blue = (c & 0xff) as f32 / 255.0;
    }
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

    #[test]
    fn value_type_undefined_cases_are_errors() {
        assert_eq!(ICoord2D { x: 46341, y: 0 }.try_length(), Err(BaseTypeError::SignedOverflow));
        assert_eq!(ICoord2D { x: 46340, y: 0 }.try_length(), Ok(46340));
        assert_eq!(ICoord2D { x: 30000, y: 30000 }.try_length(), Ok(42426));
        assert_eq!(ICoord2D { x: 33000, y: 33000 }.try_length(), Err(BaseTypeError::SignedOverflow));
        assert_eq!(ICoord3D { x: 3, y: 4, z: 12 }.try_length(), Ok(13));
        let r = IRegion2D { lo: ICoord2D { x: i32::MIN, y: 0 }, hi: ICoord2D { x: 1, y: 0 } };
        assert_eq!(r.try_width(), Err(BaseTypeError::SignedOverflow));
        let red = |r: f32| RGBColor { red: r, green: 0.0, blue: 0.0 }.try_get_as_int();
        assert_eq!(red(1.0), Ok(0xff_0000));
        assert_eq!(red(-0.5), Err(BaseTypeError::ShiftUndefined));
        assert_eq!(red(-0.001), Ok(0), "(Int)(-0.255) truncates to 0");
        assert_eq!(red(f32::NAN), Err(BaseTypeError::FloatToIntOutOfRange));
        assert_eq!(red(65535.0 / 255.0), Ok(0xffff_0000u32 as i32), "fits the unsigned type");
        assert_eq!(red(65536.0 / 255.0), Err(BaseTypeError::ShiftUndefined));
        let mut c = RGBColor::default();
        c.set_from_int(-1);
        assert_eq!((c.red, c.green, c.blue), (1.0, 1.0, 1.0));
        assert_eq!(sign(-0.0f32), 0);
        assert_eq!(sign(f32::NAN), 0);
        assert!(clamp(0.0, f32::NAN, 1.0).is_nan());
        assert_eq!(min(f32::NAN, 1.0), 1.0);
    }
}
