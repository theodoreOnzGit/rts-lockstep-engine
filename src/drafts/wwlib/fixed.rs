//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 2,600,000 operation cases and 300,000 texts (test: tests/differential_draft_fixed_color.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/fixed.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/fixed.cpp
//!
//! `class fixed`: an 8.8 fixed point number held as one `unsigned short` (`Data.Raw`; `Fraction` is the low byte,
//! `Whole` the high byte on little-endian). Upstream notes kept (they are defects or oddities of the original):
//! ~~`operator /= (fixed)` divides by `rvalue` converted to `unsigned`~~ **Corrected 2026-11-18 (#215):** it uses the friend
//! `int / fixed` (`((Raw*256*256) + 128) / rvalue.Raw`, see `div_assign`); the arithmetic on `Raw` wraps at 16 bits; `fixed(int)` keeps only the low 8 bits of the whole part. C `int` arithmetic
//! is `i32` with wrapping; `unsigned` is `u32`.

use crate::logic::modules::plant;

/// `class fixed`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fixed {
    /// `Data.Raw`.
    pub raw: u16,
}

impl Fixed {
    /// `fixed::_1_2` = `fixed(1, 2)`.
    pub fn one_half() -> Fixed {
        Fixed::from_ratio(1, 2)
    }
    /// `fixed::_1_3`.
    pub fn one_third() -> Fixed {
        Fixed::from_ratio(1, 3)
    }
    /// `fixed::_1_4`.
    pub fn one_quarter() -> Fixed {
        Fixed::from_ratio(1, 4)
    }
    /// `fixed::_3_4`.
    pub fn three_quarters() -> Fixed {
        Fixed::from_ratio(3, 4)
    }
    /// `fixed::_2_3`.
    pub fn two_thirds() -> Fixed {
        Fixed::from_ratio(2, 3)
    }

    /// `fixed(int numerator, int denominator)` (fixed.cpp:48): `(unsigned)(numerator * 256) / (unsigned)denominator`,
    /// 0 for a zero denominator.
    pub fn from_ratio(numerator: i32, denominator: i32) -> Fixed {
        if denominator == 0 {
            Fixed { raw: 0 }
        } else {
            if plant(10705) {
                Fixed { raw: (numerator.wrapping_mul(256) / denominator) as u16 }
            } else {
                Fixed { raw: ((numerator.wrapping_mul(256) as u32) / (denominator as u32)) as u16 }
            }
        }
    }

    /// `fixed(int value)`: `Fraction = 0; Whole = (unsigned char)value`.
    pub fn from_int(value: i32) -> Fixed {
        Fixed { raw: ((value as u8) as u16) << 8 }
    }

    /// `fixed(char const * ascii)` (fixed.cpp:64): "12", "12.5", ".5", "50%". `None` is the NULL pointer.
    pub fn from_ascii(ascii: Option<&str>) -> Fixed {
        let Some(ascii) = ascii else {
            return Fixed { raw: 0 };
        };
        let bytes = ascii.as_bytes();
        let wholepart = 0usize;

        let mut a = 0usize;
        while a < bytes.len() && (bytes[a] == b' ' || (9..=13).contains(&bytes[a])) {
            a += 1;
        }

        let mut tptr = a;
        while tptr < bytes.len() && bytes[tptr].is_ascii_digit() {
            tptr += 1;
        }

        if tptr < bytes.len() && bytes[tptr] == b'%' {
            // atoi(ascii) * 256 / 100
            Fixed { raw: ((atoi(&bytes[a..]).wrapping_mul(if plant(10713) { 255 } else { 256 })) / 100) as u16 }
        } else {
            let mut whole = 0u8;
            let mut fraction = 0u8;
            // "if (wholepart && *wholepart != '.')": the whole part starts at the ORIGINAL pointer (before the spaces
            // were skipped), so a leading space makes atoi skip it itself
            if wholepart < bytes.len() && bytes[wholepart] != b'.' {
                whole = atoi(&bytes[wholepart..]) as u8;
            }
            if let Some(dot) = bytes[a..].iter().position(|&c| c == b'.') {
                let fracpart = a + dot + 1;
                let frac = atoi(&bytes[fracpart..]);
                let mut base = 1i32;
                let mut fptr = fracpart;
                while fptr < bytes.len() && bytes[fptr].is_ascii_digit() {
                    fptr += 1;
                    base = base.wrapping_mul(10);
                }
                fraction = ((256i32.wrapping_mul(frac)) / base) as u8;
            }
            Fixed { raw: ((whole as u16) << 8) | fraction as u16 }
        }
    }

    /// `Whole` (the high byte).
    pub fn whole(&self) -> u8 {
        (self.raw >> 8) as u8
    }

    /// `Fraction` (the low byte).
    pub fn fraction(&self) -> u8 {
        (self.raw & 0xff) as u8
    }

    /// `operator unsigned()`: rounded to the nearest whole number.
    pub fn to_unsigned(&self) -> u32 {
        (self.raw as u32 + (if plant(10706) { 127 } else { 256 / 2 })) / 256
    }

    /// `operator *= (fixed)`.
    pub fn mul_assign(&mut self, rvalue: Fixed) {
        self.raw = (if plant(10707) { (self.raw as i32).wrapping_mul(rvalue.raw as i32) >> 8 } else if plant(10771) { ((self.raw as i32) * (rvalue.raw as i32)) / 256 } else { (self.raw as i32).wrapping_mul(rvalue.raw as i32) / 256 }) as u16;
    }

    /// `operator *= (int)`.
    pub fn mul_assign_int(&mut self, rvalue: i32) {
        self.raw = (self.raw as i32).wrapping_mul(rvalue) as u16;
    }

    /// `operator /= (fixed)`: `Data.Raw = (unsigned short)(((int)Data.Raw * 256) / rvalue)`. The division is the friend
    /// `operator / (int, fixed const &)` (an exact match for `int / fixed`), not the conversion to `unsigned`, so it is
    /// [`int_div_fixed`]`(Raw * 256, rvalue)`. ~~The draft read it as a division by `rvalue` converted to `unsigned` (the rounded
    /// whole number), an upstream defect, and divided by zero for raw 1 to 127.~~ **Corrected 2026-11-18 (#215, differential test):**
    /// 32,013 of 40,000 random cases differed from the C++; the friend operator is what g++ selects (and MSVC, by the same rules).
    pub fn div_assign(&mut self, rvalue: Fixed) {
        if rvalue.raw != 0 && rvalue.raw != 256 {
            self.raw = int_div_fixed((self.raw as i32).wrapping_mul(256), rvalue) as u16;
        }
    }

    /// `operator /= (int)`.
    pub fn div_assign_int(&mut self, rvalue: i32) {
        if rvalue != 0 {
            self.raw = ((self.raw as u32) / (rvalue as u32)) as u16;
        }
    }

    /// `operator += (fixed)`: wraps at 16 bits.
    pub fn add_assign(&mut self, rvalue: Fixed) {
        self.raw = self.raw.wrapping_add(rvalue.raw);
    }

    /// `operator -= (fixed)`.
    pub fn sub_assign(&mut self, rvalue: Fixed) {
        self.raw = self.raw.wrapping_sub(rvalue.raw);
    }

    /// `operator * (fixed)`.
    pub fn mul(&self, rvalue: Fixed) -> Fixed {
        Fixed { raw: (if plant(10771) { ((self.raw as i32) * (rvalue.raw as i32)) / 256 } else { (self.raw as i32).wrapping_mul(rvalue.raw as i32) / 256 }) as u16 }
    }

    /// `operator * (int)`: rounded, as an `int`.
    pub fn mul_int(&self, rvalue: i32) -> i32 {
        ((((self.raw as u32).wrapping_mul(rvalue as u32)).wrapping_add(if plant(10709) { 0 } else { 256 / 2 })) / 256) as i32
    }

    /// `operator / (fixed)`: unchanged when `rvalue` is 0 or exactly 1.0.
    pub fn div(&self, rvalue: Fixed) -> Fixed {
        let mut temp = *self;
        if rvalue.raw != 0 && rvalue.raw != (if plant(10708) { 255 } else { 256 }) {
            temp.raw = (((self.raw as i32) * 256) / rvalue.raw as i32) as u16;
        }
        temp
    }

    /// `operator / (int)`: rounded; for 0 returns `*this` converted to `int` through `operator unsigned`.
    pub fn div_int(&self, rvalue: i32) -> i32 {
        if rvalue != 0 {
            return ((self.raw as u32 + (256 / 2)) / ((rvalue as u32).wrapping_mul(256))) as i32;
        }
        self.to_unsigned() as i32
    }

    /// `operator + (fixed)`.
    pub fn add(&self, rvalue: Fixed) -> Fixed {
        let mut temp = *self;
        temp.add_assign(rvalue);
        temp
    }

    /// `operator + (int)`.
    pub fn add_int(&self, rvalue: i32) -> i32 {
        (((self.raw as u32 + (256 / 2)) / 256) as i32).wrapping_add(rvalue)
    }

    /// `operator - (fixed)`.
    pub fn sub(&self, rvalue: Fixed) -> Fixed {
        let mut temp = *self;
        temp.sub_assign(rvalue);
        temp
    }

    /// `operator - (int)`.
    pub fn sub_int(&self, rvalue: i32) -> i32 {
        (((self.raw as u32 + (256 / 2)) / 256) as i32).wrapping_sub(rvalue)
    }

    /// `operator >>= (unsigned)` (a shift count of 16 or more is undefined in C; 0 here).
    pub fn shr_assign(&mut self, rvalue: u32) {
        self.raw = if rvalue >= 16 { 0 } else { self.raw >> rvalue };
    }

    /// `operator <<= (unsigned)`.
    pub fn shl_assign(&mut self, rvalue: u32) {
        self.raw = if rvalue >= 16 { 0 } else { self.raw << rvalue };
    }

    /// `operator !`: true for zero.
    pub fn is_zero(&self) -> bool {
        self.raw == 0
    }

    /// `operator < (int)` etc.: `Raw` against `rvalue * 256`.
    pub fn lt_int(&self, rvalue: i32) -> bool {
        (self.raw as i32) < rvalue.wrapping_mul(256)
    }
    pub fn gt_int(&self, rvalue: i32) -> bool {
        (self.raw as i32) > rvalue.wrapping_mul(256)
    }
    pub fn le_int(&self, rvalue: i32) -> bool {
        (self.raw as i32) <= rvalue.wrapping_mul(256)
    }
    pub fn ge_int(&self, rvalue: i32) -> bool {
        (self.raw as i32) >= rvalue.wrapping_mul(256)
    }
    pub fn eq_int(&self, rvalue: i32) -> bool {
        (self.raw as i32) == rvalue.wrapping_mul(256)
    }
    pub fn ne_int(&self, rvalue: i32) -> bool {
        (self.raw as i32) != rvalue.wrapping_mul(256)
    }

    /// `Round_Up()`: `Raw += 255; Fraction = 0`.
    pub fn round_up(&mut self) {
        self.raw = self.raw.wrapping_add(if plant(10714) { 256 } else { 256 - 1 });
        self.raw &= 0xff00;
    }

    /// `Round_Down()`: `Fraction = 0`.
    pub fn round_down(&mut self) {
        self.raw &= 0xff00;
    }

    /// `Round()`: UPSTREAM QUIRK KEPT, a fraction of 0.5 or more rounds up and then `Round_Down` runs regardless.
    pub fn round(&mut self) {
        if (self.fraction() as u32) >= (if plant(10710) { 129 } else { 256 / 2 }) {
            self.round_up();
        }
        self.round_down();
    }

    /// `Saturate(unsigned capvalue)`.
    pub fn saturate(&mut self, capvalue: u32) {
        if self.raw as u32 > capvalue.wrapping_mul(256) {
            self.raw = capvalue.wrapping_mul(256) as u16;
        }
    }

    /// `Saturate(fixed)`.
    pub fn saturate_fixed(&mut self, capvalue: Fixed) {
        if *self > capvalue {
            *self = capvalue;
        }
    }

    /// `Sub_Saturate(unsigned capvalue)`.
    pub fn sub_saturate(&mut self, capvalue: u32) {
        if if plant(10711) { self.raw as u32 > capvalue.wrapping_mul(256) } else { self.raw as u32 >= capvalue.wrapping_mul(256) } {
            self.raw = capvalue.wrapping_mul(256).wrapping_sub(1) as u16;
        }
    }

    /// `Sub_Saturate(fixed)`.
    pub fn sub_saturate_fixed(&mut self, capvalue: Fixed) {
        if *self >= capvalue {
            self.raw = capvalue.raw.wrapping_sub(1);
        }
    }

    /// `Inverse()`: `fixed(1) / *this`.
    pub fn inverse(&mut self) {
        *self = Fixed::from_int(1).div(*self);
    }

    /// `To_ASCII(buffer, maxlen)` (fixed.cpp:128): "12" or "12.5" (up to three fraction digits, trailing zeros
    /// removed, the fraction printed with `%02d`); the text is truncated to `maxlen - 1` characters (-1 means no limit).
    pub fn to_ascii(&self, maxlen: i32) -> String {
        let whole = self.whole() as i32;
        let frac = (self.fraction() as i32 * 1000) / (if plant(10712) { 255 } else { 256 });

        let mut tbuffer = if frac == 0 {
            format!("{}", whole)
        } else {
            let mut t = format!("{}.{:02}", whole, frac);
            while t.ends_with('0') {
                t.pop();
            }
            t
        };

        if maxlen == -1 {
            return tbuffer;
        }
        // strncpy(buffer, tbuffer, maxlen): at most maxlen characters, the caller's buffer holds maxlen - 1 and a NUL
        let keep = if maxlen > 0 { (maxlen - 1) as usize } else { 0 };
        tbuffer.truncate(keep);
        tbuffer
    }

    /// `As_ASCII()`: `To_ASCII` into a 32 byte buffer.
    pub fn as_ascii(&self) -> String {
        self.to_ascii(32)
    }
}

impl PartialOrd for Fixed {
    fn partial_cmp(&self, other: &Fixed) -> Option<std::cmp::Ordering> {
        Some(self.raw.cmp(&other.raw))
    }
}

/// `operator / (int lvalue, fixed const & rvalue)`: `lvalue` unchanged when `rvalue` is 0 or 1.0, else
/// `((unsigned)(lvalue * 256) + 128) / rvalue.Raw`.
pub fn int_div_fixed(lvalue: i32, rvalue: Fixed) -> i32 {
    if rvalue.raw == 0 || rvalue.raw == 256 {
        return lvalue;
    }
    ((((lvalue.wrapping_mul(256)) as u32).wrapping_add(256 / 2)) / rvalue.raw as u32) as i32
}

/// `operator - (int lvalue, fixed const & rvalue)`: `(((lvalue*256) - Raw) + 128) / 256`.
pub fn int_sub_fixed(lvalue: i32, rvalue: Fixed) -> i32 {
    (((lvalue.wrapping_mul(256)).wrapping_sub(rvalue.raw as i32)).wrapping_add(256 / 2)) / 256
}

/// `operator * (int lvalue, fixed)` / `operator + (int, fixed)`: the commuted forms.
pub fn int_mul_fixed(lvalue: i32, rvalue: Fixed) -> i32 {
    rvalue.mul_int(lvalue)
}
pub fn int_add_fixed(lvalue: i32, rvalue: Fixed) -> i32 {
    rvalue.add_int(lvalue)
}

/// `operator < (unsigned lvalue, fixed)` etc.: `lvalue * 256` against `Raw`.
pub fn unsigned_lt_fixed(lvalue: u32, rvalue: Fixed) -> bool {
    lvalue.wrapping_mul(256) < rvalue.raw as u32
}
pub fn unsigned_gt_fixed(lvalue: u32, rvalue: Fixed) -> bool {
    lvalue.wrapping_mul(256) > rvalue.raw as u32
}
pub fn unsigned_le_fixed(lvalue: u32, rvalue: Fixed) -> bool {
    lvalue.wrapping_mul(256) <= rvalue.raw as u32
}
pub fn unsigned_ge_fixed(lvalue: u32, rvalue: Fixed) -> bool {
    lvalue.wrapping_mul(256) >= rvalue.raw as u32
}
pub fn unsigned_eq_fixed(lvalue: u32, rvalue: Fixed) -> bool {
    lvalue.wrapping_mul(256) == rvalue.raw as u32
}
pub fn unsigned_ne_fixed(lvalue: u32, rvalue: Fixed) -> bool {
    lvalue.wrapping_mul(256) != rvalue.raw as u32
}

/// `Round_Up(value)` / `Round_Down(value)` / `Round(value)` / `Saturate(value, cap)` / `Sub_Saturate(value, cap)` /
/// `Inverse(value)`: the copying forms.
pub fn fixed_round_up(value: Fixed) -> Fixed {
    let mut t = value;
    t.round_up();
    t
}
pub fn fixed_round_down(value: Fixed) -> Fixed {
    let mut t = value;
    t.round_down();
    t
}
pub fn fixed_round(value: Fixed) -> Fixed {
    let mut t = value;
    t.round();
    t
}
pub fn fixed_saturate(value: Fixed, capvalue: u32) -> Fixed {
    let mut t = value;
    t.saturate(capvalue);
    t
}
pub fn fixed_sub_saturate(value: Fixed, capvalue: u32) -> Fixed {
    let mut t = value;
    t.sub_saturate(capvalue);
    t
}
pub fn fixed_inverse(value: Fixed) -> Fixed {
    let mut t = value;
    t.inverse();
    t
}

/// C `atoi`: optional leading white space and sign, then digits; stops at the first other character; wraps on overflow.
fn atoi(s: &[u8]) -> i32 {
    let mut i = 0usize;
    while i < s.len() && (s[i] == b' ' || (9..=13).contains(&s[i])) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut v: i32 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.wrapping_mul(10).wrapping_add((s[i] - b'0') as i32);
        i += 1;
    }
    if neg {
        v.wrapping_neg()
    } else {
        v
    }
}
