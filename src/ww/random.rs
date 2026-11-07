// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/random.cpp, RANDOM.H
//   (Random3Class: constructor, operator(), Mix1, Mix2; Random4Class:
//   constructor, operator(), Get_Float)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWLib, by Joe Bostic; Random4Class by Hector Yee, after
//   Matsumoto and Nishimura's Mersenne Twister)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! WWLib's random number generators that the W3D particle code draws from
//! (GitHub #49): `Random3Class` (the `Vector3Randomizer`s' shared generator,
//! v3_rnd.cpp) and `Random4Class` (part_buf.cpp's `rand_gen`, which fills the
//! keyframe randomizer tables). Both are process globals upstream; here the
//! caller owns them ([`crate::formats::w3d::part_emt::ParticleRandom`]).
//!
//! The arithmetic is upstream's `int`/`unsigned int` arithmetic, wrapping (the
//! C++ is compiled with `-fwrapv` in the differential tests; MSVC wraps), with
//! `>>` on a signed `int` arithmetic, as MSVC and every x86 compiler do.

/// `Random3Class::Mix1`.
const MIX1: [u32; 20] = [
    0xbaa96887, 0x1e17d32c, 0x03bcdc3c, 0x0f33d1b2, 0x76a6491d, 0xc570d85d, 0xe382b1e3, 0x78db4362, 0x7439a9d4, 0x9cea8ac5,
    0x89537c5c, 0x2588f55d, 0x415b5e1d, 0x216e3d95, 0x85c662e7, 0x5e8ab368, 0x3ea5cc8c, 0xd26a0f74, 0xf3a9222b, 0x48aad7e4,
];
/// `Random3Class::Mix2`.
const MIX2: [u32; 20] = [
    0x4b0f3b58, 0xe874f0c3, 0x6955c5a6, 0x55a7ca46, 0x4d9a9d86, 0xfe28a195, 0xb1ca7865, 0x6b235751, 0x9a997a61, 0xaa6e95c8,
    0xaaa98ee1, 0x5af9154c, 0xfc8e2263, 0x390f5e8c, 0x58ffd802, 0xac0a5eba, 0xac4874f6, 0xa9df0913, 0x86be4c74, 0xed2c123b,
];

/// `class Random3Class`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Random3 {
    /// `Seed`.
    pub seed: i32,
    /// `Index`.
    pub index: i32,
}

impl Default for Random3 {
    /// `Random3Class()`: seeds 0 and 0.
    fn default() -> Self {
        Random3::new(0, 0)
    }
}

impl Random3 {
    /// `Random3Class(seed1, seed2)`.
    pub fn new(seed1: u32, seed2: u32) -> Self {
        Random3 { seed: seed1 as i32, index: seed2 as i32 }
    }

    /// `operator()()` (and `operator int()`).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        let mut loword = self.seed;
        let mut hiword = self.index;
        self.index = self.index.wrapping_add(1);
        for i in 0..4 {
            let hihold = hiword;
            let mut temp = hihold ^ MIX1[i] as i32;
            let itmpl = temp & 0xffff;
            let itmph = temp >> 16;
            temp = itmpl.wrapping_mul(itmpl).wrapping_add(!itmph.wrapping_mul(itmph));
            // (temp >> 16) | (temp << 16): an arithmetic shift, not a rotation (a negative temp keeps its sign bits)
            #[allow(clippy::manual_rotate)]
            {
                temp = (temp >> 16) | (temp << 16);
            }
            hiword = loword ^ (temp ^ MIX2[i] as i32).wrapping_add(itmpl.wrapping_mul(itmph));
            loword = hihold;
        }
        hiword
    }
}

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908b0df;
const UPPER_MASK: u32 = 0x80000000;
const LOWER_MASK: u32 = 0x7fffffff;
const TEMPERING_MASK_B: u32 = 0x9d2c5680;
const TEMPERING_MASK_C: u32 = 0xefc60000;

/// `class Random4Class` (the Mersenne Twister).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Random4 {
    /// `mt`.
    pub mt: Box<[u32; N]>,
    /// `mti`.
    pub mti: i32,
}

impl Default for Random4 {
    /// `Random4Class()`: seed 4357.
    fn default() -> Self {
        Random4::new(4357)
    }
}

impl Random4 {
    /// `Random4Class(seed)`: Knuth's line 25 (`69069 * x`); a seed of 0 is 4375.
    pub fn new(seed: u32) -> Self {
        let seed = if seed == 0 { 4375 } else { seed };
        let mut mt = Box::new([0u32; N]);
        mt[0] = seed;
        for i in 1..N {
            mt[i] = 69069u32.wrapping_mul(mt[i - 1]);
        }
        // mti is N after this (upstream's comment says N+1)
        Random4 { mt, mti: N as i32 }
    }

    /// `operator()()` (and `operator int()`): the tempered word, as an `int`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> i32 {
        const MAG01: [u32; 2] = [0, MATRIX_A];
        if self.mti >= N as i32 {
            // generate N words at one time
            let mt = &mut self.mt;
            let mut kk = 0;
            while kk < N - M {
                let y = (mt[kk] & UPPER_MASK) | (mt[kk + 1] & LOWER_MASK);
                mt[kk] = mt[kk + M] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
                kk += 1;
            }
            while kk < N - 1 {
                let y = (mt[kk] & UPPER_MASK) | (mt[kk + 1] & LOWER_MASK);
                mt[kk] = mt[kk + M - N] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
                kk += 1;
            }
            let y = (mt[N - 1] & UPPER_MASK) | (mt[0] & LOWER_MASK);
            mt[N - 1] = mt[M - 1] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
            self.mti = 0;
        }
        let mut y = self.mt[self.mti as usize];
        self.mti += 1;
        y ^= y >> 11;
        y ^= (y << 7) & TEMPERING_MASK_B;
        y ^= (y << 15) & TEMPERING_MASK_C;
        y ^= y >> 18;
        y as i32
    }

    /// `Get_Float()`: the word as unsigned, times 2^-32.
    pub fn get_float(&mut self) -> f32 {
        (self.next() as u32) as f32 * (1.0f32 / 4_294_967_296.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Predicted before the run: the Mersenne Twister seeded through Knuth's
    /// line 25 is not the reference MT19937 seeding, so no published vector
    /// applies; the checks are structural: the first refill happens on the
    /// first draw (mti starts at 624) and leaves mti 1; two generators of the
    /// same seed agree; seed 0 behaves as 4375. Random3's index advances by
    /// one per draw and seeds (s, i + 1) give the second number of (s, i)
    /// (upstream's "the second seed is the Nth index"). The bit-for-bit check
    /// against the C++ is the differential (tests/differential_w3d.rs,
    /// section 16).
    ///
    /// *Result (2026-11-07), first run:* held.
    #[test]
    fn generators_behave_as_written() {
        let mut a = Random4::new(4357);
        let mut b = Random4::default();
        assert_eq!(a.mti, 624);
        assert_eq!(a.next(), b.next());
        assert_eq!(a.mti, 1);
        assert_eq!(Random4::new(0), Random4::new(4375));
        let mut r = Random3::new(7, 3);
        let _ = r.next();
        let second = r.next();
        assert_eq!(r.index, 5);
        assert_eq!(Random3::new(7, 4).next(), second);
    }
}
