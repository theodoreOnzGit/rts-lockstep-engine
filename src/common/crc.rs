// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/crc.h
//   GeneralsMD/Code/GameEngine/Source/Common/crc.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Matthew D. Campbell, October 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The rotate-and-add checksum the engine uses for lockstep desync detection
//! (`XferCRC`, the logic random seed CRC, and so on).
//!
//! Upstream has two implementations of the same function: an out-of-line C++
//! loop for `_DEBUG` builds (`crc.cpp`) and an MSVC `_asm` block for release
//! builds (`crc.h`). Both compute, for every byte `b`:
//! `crc = (crc << 1) + b + (old crc >> 31)` modulo 2^32. The asm does it with
//! `shl ebx,1` (the shifted-out bit lands in the carry flag) followed by
//! `adc ebx,eax`. This translation is plain Rust; the differential tests check
//! it against both the compiled C++ path and a GNU-asm transcription of the
//! release instruction sequence.

/// `class CRC` from `crc.h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Crc {
    crc: u32,
}

impl Crc {
    /// `CRC()`: starts at zero.
    pub const fn new() -> Self {
        Crc { crc: 0 }
    }

    /// `CRC::addCRC` (crc.cpp, `_DEBUG` path): fold in one byte.
    #[inline]
    fn add_crc(&mut self, val: u8) {
        let hibit: u32 = if self.crc & 0x8000_0000 != 0 { 1 } else { 0 };
        self.crc <<= 1;
        self.crc = self.crc.wrapping_add(u32::from(val));
        self.crc = self.crc.wrapping_add(hibit);
    }

    /// `CRC::computeCRC(const void *buf, Int len)`: add a buffer into the
    /// running CRC. Upstream returns early for a null buffer or `len < 1`;
    /// an empty slice is the Rust equivalent.
    pub fn compute_crc(&mut self, buf: &[u8]) {
        if buf.is_empty() {
            return;
        }
        for &b in buf {
            self.add_crc(b);
        }
    }

    /// `CRC::clear()`.
    pub fn clear(&mut self) {
        self.crc = 0;
    }

    /// `CRC::get()`. (The `htonl` that upstream once applied is commented out
    /// there, so the value is returned as is.)
    pub const fn get(&self) -> u32 {
        self.crc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_computed_values() {
        let mut c = Crc::new();
        c.compute_crc(&[]);
        assert_eq!(c.get(), 0);
        c.compute_crc(&[1, 2, 3]);
        // ((1 << 1) + 2) << 1 + 3 = 11
        assert_eq!(c.get(), 11);
        let mut h = Crc { crc: 0x8000_0000 };
        h.compute_crc(&[0]);
        assert_eq!(h.get(), 1, "the bit shifted out re-enters at the bottom");
        h.clear();
        assert_eq!(h.get(), 0);
    }
}
