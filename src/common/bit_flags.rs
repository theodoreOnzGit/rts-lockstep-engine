// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/BitFlags.h
//   GeneralsMD/Code/GameEngine/Include/Common/BitFlagsIO.h (parse,
//   parseFromINI, parseSingleBitFromINI, xfer)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, March 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `template <size_t NUMBITS> class BitFlags`: a `std::bitset` with a name
//! table (`s_bitNameList`), and its INI parsing and xfer.
//!
//! Each instantiation is a [`BitFlags<T>`] whose marker type `T` implements
//! [`BitFlagNames`] (the bit count and the names): `KindOfMaskType`,
//! `ObjectStatusMaskType`, `DisabledMaskType`, `ModelConditionFlags`,
//! `ArmorSetFlags`, `WeaponSetFlags`, `SpecialPowerMaskType`. The bits are
//! stored in four `u64` words (256 bits; the largest instantiation has 117).
//!
//! # Where this follows the shipped 32-bit build
//!
//! **`xfer` in CRC mode** hashes `xferUser(this, sizeof(this))`: the size of
//! the *pointer*, not of the flags. Upstream bug. On the shipped Win32 build
//! that is 4 bytes, the first word of the `bitset`, i.e. **only bits 0-31
//! enter the lockstep CRC**; a 64-bit build hashes 8 bytes. This reproduces
//! the shipped 4 bytes (the low 32 bits, little-endian), following the
//! owner's decision for the other results that only the 32-bit build
//! defines (`DEATH_NORMAL` and `LEVEL_REGULAR` as bit 31). The differential
//! test checks them against the first 4 of the 8 bytes the 64-bit C++
//! reference hashes.
//!
//! # Where upstream throws
//!
//! `std::bitset::set` and `test` throw `std::out_of_range` for an index
//! outside `[0, NUMBITS)` (a negative `Int` converts to a huge `size_t`).
//! The `try_*` methods return [`BitIndexOutOfRange`] there; the plain ones
//! panic. `getNameFromSingleBit` returns `NULL` (`None`) instead.

use std::marker::PhantomData;

use super::ini::{Ini, IniError, IniErrorKind, IniResult};
use super::xfer::{Xfer, XferMode, XferResult, XferStatus, XferVersion};

/// `std::bitset::set`/`test` were given an index outside `[0, NUMBITS)`:
/// upstream throws `std::out_of_range`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BitIndexOutOfRange;

impl std::fmt::Display for BitIndexOutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BitFlags: bit index out of range (std::out_of_range upstream)")
    }
}

/// One instantiation of `BitFlags<NUMBITS>`: its size and `s_bitNameList`.
pub trait BitFlagNames {
    /// `NUMBITS`.
    const NUM_BITS: usize;
    /// `s_bitNameList`, without the `NULL` terminator.
    const NAMES: &'static [&'static str];
}

const WORDS: usize = 4;

/// `BitFlags<NUMBITS>`.
pub struct BitFlags<T: BitFlagNames> {
    bits: [u64; WORDS],
    _names: PhantomData<T>,
}

impl<T: BitFlagNames> Clone for BitFlags<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: BitFlagNames> Copy for BitFlags<T> {}
impl<T: BitFlagNames> PartialEq for BitFlags<T> {
    /// `operator==`.
    fn eq(&self, other: &Self) -> bool {
        self.bits == other.bits
    }
}
impl<T: BitFlagNames> Eq for BitFlags<T> {}
impl<T: BitFlagNames> std::hash::Hash for BitFlags<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.bits.hash(state);
    }
}
impl<T: BitFlagNames> Default for BitFlags<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: BitFlagNames> std::fmt::Debug for BitFlags<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let set: Vec<&str> = (0..T::NUM_BITS)
            .filter(|&i| self.bit(i))
            .map(|i| T::NAMES[i])
            .collect();
        write!(f, "BitFlags{set:?}")
    }
}

/// `strcmp`-style C string: up to the first NUL.
fn c_str(s: &[u8]) -> &[u8] {
    match s.iter().position(|&b| b == 0) {
        Some(n) => &s[..n],
        None => s,
    }
}

impl<T: BitFlagNames> BitFlags<T> {
    const CHECK: () = assert!(T::NUM_BITS <= WORDS * 64 && T::NAMES.len() == T::NUM_BITS);

    /// `BitFlags()`: all clear (named like upstream's `*_FLAGS_NONE`).
    pub const NONE: Self = Self::new();

    /// Every one of the `NUMBITS` bits set (upstream's `*_FLAGS_ALL`: clear,
    /// then `flip()`).
    pub const ALL: Self = {
        let mut bits = [0u64; WORDS];
        let mut w = 0;
        while w < WORDS {
            bits[w] = Self::word_mask(w);
            w += 1;
        }
        BitFlags {
            bits,
            _names: PhantomData,
        }
    };

    /// `BitFlags()`: all clear.
    pub const fn new() -> Self {
        #[allow(clippy::let_unit_value)]
        let _ = Self::CHECK;
        BitFlags {
            bits: [0; WORDS],
            _names: PhantomData,
        }
    }

    /// `BitFlags(kInit, idx1, ...)`: the listed bits set (upstream has
    /// overloads for 1-5 and 12 indices; this takes any number).
    pub fn try_from_indices(indices: &[i32]) -> Result<Self, BitIndexOutOfRange> {
        let mut f = Self::new();
        for &i in indices {
            f.try_set(i, 1)?;
        }
        Ok(f)
    }

    /// `BitFlags(kInit, idx1, ...)`; panics on an index out of range.
    pub fn from_indices(indices: &[i32]) -> Self {
        Self::try_from_indices(indices).unwrap_or_else(|e| panic!("{e}"))
    }

    /// The raw words, bit `i` in word `i / 64` (for tests and xfer).
    pub fn words(&self) -> [u64; WORDS] {
        self.bits
    }

    /// From raw words; bits at or above `NUMBITS` are dropped.
    pub fn from_words(mut words: [u64; WORDS]) -> Self {
        for (w, v) in words.iter_mut().enumerate() {
            *v &= Self::word_mask(w);
        }
        BitFlags {
            bits: words,
            _names: PhantomData,
        }
    }

    const fn word_mask(w: usize) -> u64 {
        let lo = w * 64;
        if T::NUM_BITS >= lo + 64 {
            u64::MAX
        } else if T::NUM_BITS <= lo {
            0
        } else {
            (1u64 << (T::NUM_BITS - lo)) - 1
        }
    }

    fn index(i: i32) -> Result<usize, BitIndexOutOfRange> {
        if i < 0 || i as usize >= T::NUM_BITS {
            Err(BitIndexOutOfRange)
        } else {
            Ok(i as usize)
        }
    }

    fn bit(&self, i: usize) -> bool {
        self.bits[i / 64] >> (i % 64) & 1 != 0
    }

    fn put(&mut self, i: usize, v: bool) {
        if v {
            self.bits[i / 64] |= 1u64 << (i % 64);
        } else {
            self.bits[i / 64] &= !(1u64 << (i % 64));
        }
    }

    fn map2(&self, that: &Self, f: impl Fn(u64, u64) -> u64) -> [u64; WORDS] {
        let mut r = [0u64; WORDS];
        for (w, v) in r.iter_mut().enumerate() {
            *v = f(self.bits[w], that.bits[w]) & Self::word_mask(w);
        }
        r
    }

    fn any_words(w: &[u64; WORDS]) -> bool {
        w.iter().any(|&x| x != 0)
    }

    /// `set(i, val)`: `std::bitset::set(i, val)`, `val` converted to `bool`.
    pub fn try_set(&mut self, i: i32, val: i32) -> Result<(), BitIndexOutOfRange> {
        let i = Self::index(i)?;
        self.put(i, val != 0);
        Ok(())
    }

    /// `set(i, val)`; panics on an index out of range.
    pub fn set(&mut self, i: i32, val: i32) {
        self.try_set(i, val).unwrap_or_else(|e| panic!("{e}"))
    }

    /// `test(i)`.
    pub fn try_test(&self, i: i32) -> Result<bool, BitIndexOutOfRange> {
        Ok(self.bit(Self::index(i)?))
    }

    /// `test(i)`; panics on an index out of range.
    pub fn test(&self, i: i32) -> bool {
        self.try_test(i).unwrap_or_else(|e| panic!("{e}"))
    }

    /// `testForAny(that)`: any bit set in both.
    pub fn test_for_any(&self, that: &Self) -> bool {
        Self::any_words(&self.map2(that, |a, b| a & b))
    }

    /// `testForAll(that)`: every bit of `that` is set here (true for an empty
    /// `that`; upstream asserts against that in debug builds only).
    pub fn test_for_all(&self, that: &Self) -> bool {
        !Self::any_words(&self.map2(that, |a, b| !a & b))
    }

    /// `testForNone(that)`: no bit of `that` is set here.
    pub fn test_for_none(&self, that: &Self) -> bool {
        !Self::any_words(&self.map2(that, |a, b| a & b))
    }

    /// `size()`: `NUMBITS`.
    pub fn size(&self) -> i32 {
        T::NUM_BITS as i32
    }

    /// `count()`: the number of bits set.
    pub fn count(&self) -> i32 {
        self.bits.iter().map(|w| w.count_ones()).sum::<u32>() as i32
    }

    /// `any()`.
    pub fn any(&self) -> bool {
        Self::any_words(&self.bits)
    }

    /// `flip()`: every one of the `NUMBITS` bits.
    pub fn flip(&mut self) {
        for w in 0..WORDS {
            self.bits[w] = !self.bits[w] & Self::word_mask(w);
        }
    }

    /// `clear()`: `std::bitset::reset()`.
    pub fn clear(&mut self) {
        self.bits = [0; WORDS];
    }

    /// `countIntersection(that)`.
    pub fn count_intersection(&self, that: &Self) -> i32 {
        self.map2(that, |a, b| a & b)
            .iter()
            .map(|w| w.count_ones())
            .sum::<u32>() as i32
    }

    /// `countInverseIntersection(that)`: bits of `that` not set here.
    pub fn count_inverse_intersection(&self, that: &Self) -> i32 {
        self.map2(that, |a, b| !a & b)
            .iter()
            .map(|w| w.count_ones())
            .sum::<u32>() as i32
    }

    /// `anyIntersectionWith(that)`.
    pub fn any_intersection_with(&self, that: &Self) -> bool {
        Self::any_words(&that.map2(self, |a, b| a & b))
    }

    /// `clear(clr)`: `m_bits &= ~clr.m_bits`.
    pub fn clear_mask(&mut self, clr: &Self) {
        self.bits = self.map2(clr, |a, b| a & !b);
    }

    /// `set(set)`: `m_bits |= set.m_bits`.
    pub fn set_mask(&mut self, set: &Self) {
        self.bits = self.map2(set, |a, b| a | b);
    }

    /// `clearAndSet(clr, set)`.
    pub fn clear_and_set(&mut self, clr: &Self, set: &Self) {
        self.clear_mask(clr);
        self.set_mask(set);
    }

    /// `testSetAndClear(mustBeSet, mustBeClear)`.
    pub fn test_set_and_clear(&self, must_be_set: &Self, must_be_clear: &Self) -> bool {
        if Self::any_words(&self.map2(must_be_clear, |a, b| a & b)) {
            return false;
        }
        !Self::any_words(&self.map2(must_be_set, |a, b| !a & b))
    }

    /// `getBitNames()`.
    pub fn get_bit_names() -> &'static [&'static str] {
        T::NAMES
    }

    /// `getNameFromSingleBit(i)`: `NULL` outside `[0, NUMBITS)`.
    pub fn get_name_from_single_bit(i: i32) -> Option<&'static str> {
        Self::index(i).ok().map(|i| T::NAMES[i])
    }

    /// `getSingleBitFromName(token)`: the first name equal to `token` under
    /// `stricmp` (ASCII case folding), or -1.
    pub fn get_single_bit_from_name(token: &[u8]) -> i32 {
        let token = c_str(token);
        T::NAMES
            .iter()
            .position(|n| n.as_bytes().eq_ignore_ascii_case(token))
            .map_or(-1, |i| i as i32)
    }

    /// `getBitNameIfSet(i)`: `test(i) ? s_bitNameList[i] : NULL`.
    pub fn try_get_bit_name_if_set(
        &self,
        i: i32,
    ) -> Result<Option<&'static str>, BitIndexOutOfRange> {
        Ok(if self.try_test(i)? {
            Some(T::NAMES[i as usize])
        } else {
            None
        })
    }

    /// `setBitByName(token)`: sets the named bit; false if there is no such
    /// name.
    pub fn set_bit_by_name(&mut self, token: &[u8]) -> bool {
        let i = Self::get_single_bit_from_name(token);
        if i >= 0 {
            self.put(i as usize, true);
            true
        } else {
            false
        }
    }

    /// `buildDescription(str)`: each set bit's name followed by `",\n"`,
    /// appended to `str`.
    pub fn build_description(&self, str: &mut Vec<u8>) {
        for i in 0..T::NUM_BITS {
            if self.bit(i) {
                str.extend_from_slice(T::NAMES[i].as_bytes());
                str.extend_from_slice(b",\n");
            }
        }
    }

    /// `parse(ini, str)` (BitFlagsIO.h): the rest of the INI line, a list of
    /// names (`A B C`: these bits only), or of `+A -B` edits to the current
    /// value, or `NONE` (clear, and the rest of the line is ignored).
    /// Mixing plain and `+`/`-` names, or `NONE` after any name, is
    /// `INI_INVALID_NAME_LIST`; an unknown name is `INI_INVALID_DATA`
    /// (`scanIndexList`). With no tokens the value is unchanged. `str`, if
    /// given, is cleared and receives the tokens read, space-separated, up to
    /// and including the one that stopped the parse.
    ///
    /// Like upstream, the flags are changed in place, so on an error the
    /// edits before it remain.
    pub fn parse(&mut self, ini: &mut Ini<'_>, mut str: Option<&mut Vec<u8>>) -> IniResult<()> {
        if let Some(s) = str.as_deref_mut() {
            s.clear();
        }
        let mut found_normal = false;
        let mut found_add_or_sub = false;
        while let Some(token) = ini.get_next_token_or_null(None) {
            if let Some(s) = str.as_deref_mut() {
                if !s.is_empty() {
                    s.push(b' ');
                }
                s.extend_from_slice(&token);
            }
            if token.eq_ignore_ascii_case(b"NONE") {
                if found_normal || found_add_or_sub {
                    return Err(IniError::new(IniErrorKind::InvalidNameList));
                }
                self.clear();
                break;
            }
            if token[0] == b'+' || token[0] == b'-' {
                if found_normal {
                    return Err(IniError::new(IniErrorKind::InvalidNameList));
                }
                let bit = Ini::scan_index_list(&token[1..], T::NAMES)?;
                self.put(bit as usize, token[0] == b'+');
                found_add_or_sub = true;
            } else {
                if found_add_or_sub {
                    return Err(IniError::new(IniErrorKind::InvalidNameList));
                }
                if !found_normal {
                    self.clear();
                }
                let bit = Ini::scan_index_list(&token, T::NAMES)?;
                self.put(bit as usize, true);
                found_normal = true;
            }
        }
        Ok(())
    }

    /// `parseFromINI` (the field parser): [`BitFlags::parse`] without a
    /// description string.
    pub fn parse_from_ini(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        self.parse(ini, None)
    }

    /// `parseSingleBitFromINI` (the field parser): the index of the next
    /// token's name (`INI_INVALID_DATA` if there is no token or no such
    /// name); upstream stores it as an `Int`.
    pub fn parse_single_bit_from_ini(ini: &mut Ini<'_>) -> IniResult<i32> {
        let token = ini.get_next_token(None)?;
        Ini::scan_index_list(&token, T::NAMES)
    }

    /// `xfer(xfer)` (version 1). Save: the number of bits set, then each set
    /// bit's name as an `AsciiString`, in bit order. Load: clears, then sets
    /// each name read (an unknown name is `XFER_READ_ERROR`). CRC: the low
    /// 32 bits as 4 little-endian bytes (as shipped, see the module docs).
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                let mut c = self.count();
                xfer.xfer_int(&mut c)?;
                for i in 0..T::NUM_BITS {
                    if self.bit(i) {
                        let mut name = T::NAMES[i].as_bytes().to_vec();
                        xfer.xfer_ascii_string(&mut name)?;
                    }
                }
                Ok(())
            }
            XferMode::Load => {
                self.clear();
                let mut c = 0i32;
                xfer.xfer_int(&mut c)?;
                let mut name = Vec::new();
                for _ in 0..c {
                    xfer.xfer_ascii_string(&mut name)?;
                    if !self.set_bit_by_name(&name) {
                        return Err(XferStatus::ReadError);
                    }
                }
                Ok(())
            }
            XferMode::Crc => {
                let mut low = (self.bits[0] as u32).to_le_bytes();
                xfer.xfer_user(&mut low)
            }
            XferMode::Invalid => Err(XferStatus::ModeUnknown),
        }
    }
}
