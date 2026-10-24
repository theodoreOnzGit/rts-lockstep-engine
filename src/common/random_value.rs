// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/RandomValue.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/RandomValue.h
//   GeneralsMD/Code/GameEngine/Include/GameLogic/LogicRandomValue.h
//   GeneralsMD/Code/GameEngine/Include/GameClient/ClientRandomValue.h
//   GeneralsMD/Code/GameEngine/Include/Common/AudioRandomValue.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Michael S. Booth, January 1998
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The engine's pseudo-random number generators.
//!
//! Upstream keeps three independent generator states as file-static globals:
//! **logic** (must stay identical on every lockstep peer), **client** (visual
//! effects, may differ per machine) and **audio**. Each is a 6-word
//! add-with-carry generator. The modification here is structural only: the
//! globals become fields of [`RandomValues`], so the caller owns the state
//! (one per game, no hidden globals, safe on wasm). The arithmetic is
//! unchanged, including two quirks:
//!
//! - upstream's `ADC` macro re-reads its first operand *after* assigning the
//!   sum, so in every step but the first the carry test is `sum < b` only.
//!   This is translated literally (see [`random_value`]). It turns out to be
//!   harmless: `sum < b` misses a real carry only when the incoming `ax` is
//!   `0xffffffff` with carry-in 1, and a wrapped 32-bit sum can never be
//!   `0xffffffff` unless both of its inputs were and the carry-in was 1,
//!   which the first (unaliased) step cannot produce. A mutation test that
//!   "fixed" the macro was indeed not detected by the differential tests;
//! - `GetGame*RandomValue(lo, hi)` computes `hi - lo + 1` in 32-bit wrapping
//!   arithmetic and returns `hi` when that is zero.
//!
//! `InitRandom(void)` (seeded from `time()`) is not translated: lockstep needs
//! the caller to supply the seed. The `DEBUG_RANDOM_*` logging is omitted.

use super::crc::Crc;

/// The initial seed words of all three generators (`theGame*Seed[6]`).
/// They equal `seed_random(0)`.
pub const INITIAL_SEED: [u32; 6] = [
    0xf22d0e56, 0x883126e9, 0xc624dd2f, 0x0702c49c, 0x9e353f7d, 0x6fdf3b64,
];

/// `theMultFactor = 1.0f / (powf(2, 8 * sizeof(UnsignedInt)) - 1.0f)`.
///
/// In `f32`, `2^32 - 1` rounds back to `2^32`, so this is exactly `2^-32`.
/// (Evaluated in double precision, as an x87 static initialiser at the default
/// 53-bit precision would, `1/(2^32-1)` still rounds to `2^-32` when stored.)
pub const MULT_FACTOR: f32 = 1.0f32 / (4_294_967_296.0f32 - 1.0f32);

/// Upstream's `ADC(SUM, A, B, C)` macro:
/// `SUM = (A) + (B) + (C); C = ((SUM < (A)) || (SUM < (B)))`.
///
/// `a_after` is the value the macro reads for `A` in the carry test, which is
/// the new `SUM` whenever `SUM` and `A` are the same variable (`ax`).
#[inline(always)]
fn adc(a: u32, b: u32, c: &mut u32, a_aliases_sum: bool) -> u32 {
    let sum = a.wrapping_add(b).wrapping_add(*c);
    let a_after = if a_aliases_sum { sum } else { a };
    *c = u32::from(sum < a_after || sum < b);
    sum
}

/// `static UnsignedInt randomValue(UnsignedInt *seed)`: advance a generator
/// state and return the next 32-bit value.
pub fn random_value(seed: &mut [u32; 6]) -> u32 {
    let mut c: u32 = 0;

    // ADC(ax, seed[5], seed[4], c): A is seed[5], not aliased with ax.
    let mut ax = adc(seed[5], seed[4], &mut c, false);
    seed[4] = ax;

    // ADC(ax, ax, seed[k], c): A is ax, so the carry test reads the new ax.
    ax = adc(ax, seed[3], &mut c, true);
    seed[3] = ax;

    ax = adc(ax, seed[2], &mut c, true);
    seed[2] = ax;

    ax = adc(ax, seed[1], &mut c, true);
    seed[1] = ax;

    ax = adc(ax, seed[0], &mut c, true);
    seed[0] = ax;

    // Increment seed array, bubbling up the carries.
    seed[5] = seed[5].wrapping_add(1);
    if seed[5] == 0 {
        seed[4] = seed[4].wrapping_add(1);
        if seed[4] == 0 {
            seed[3] = seed[3].wrapping_add(1);
            if seed[3] == 0 {
                seed[2] = seed[2].wrapping_add(1);
                if seed[2] == 0 {
                    seed[1] = seed[1].wrapping_add(1);
                    if seed[1] == 0 {
                        seed[0] = seed[0].wrapping_add(1);
                        ax = ax.wrapping_add(1);
                    }
                }
            }
        }
    }
    ax
}

/// `static void seedRandom(UnsignedInt SEED, UnsignedInt *seed)`.
pub fn seed_random(seed_value: u32, seed: &mut [u32; 6]) {
    let mut ax = seed_value;
    ax = ax.wrapping_add(0xf22d0e56);
    seed[0] = ax;
    ax = ax.wrapping_add(0x883126e9u32.wrapping_sub(0xf22d0e56));
    seed[1] = ax;
    ax = ax.wrapping_add(0xc624dd2fu32.wrapping_sub(0x883126e9));
    seed[2] = ax;
    ax = ax.wrapping_add(0x0702c49cu32.wrapping_sub(0xc624dd2f));
    seed[3] = ax;
    ax = ax.wrapping_add(0x9e353f7du32.wrapping_sub(0x0702c49c));
    seed[4] = ax;
    ax = ax.wrapping_add(0x6fdf3b64u32.wrapping_sub(0x9e353f7d));
    seed[5] = ax;
}

/// The integer draw shared by `GetGameLogicRandomValue`,
/// `GetGameClientRandomValue` and `GetGameAudioRandomValue`
/// (identical bodies upstream, differing only in the state they advance).
///
/// Returns a value in `[lo, hi]`. When `lo > hi` the 32-bit wrap of
/// `hi - lo + 1` makes the range wrap too, exactly as upstream.
pub fn random_int(seed: &mut [u32; 6], lo: i32, hi: i32) -> i32 {
    let delta: u32 = hi.wrapping_sub(lo).wrapping_add(1) as u32;
    if delta == 0 {
        return hi;
    }
    ((random_value(seed) % delta) as i32).wrapping_add(lo)
}

/// The real-valued draw shared by `GetGame{Logic,Client,Audio}RandomValueReal`.
///
/// Returns `hi` when `hi - lo <= 0` (a NaN difference does not take that
/// branch, as upstream). Operation order is upstream's:
/// `((Real)rand * theMultFactor) * delta + lo`.
pub fn random_real(seed: &mut [u32; 6], lo: f32, hi: f32) -> f32 {
    let delta = hi - lo;
    if delta <= 0.0f32 {
        return hi;
    }
    ((random_value(seed) as f32) * MULT_FACTOR) * delta + lo
}

/// The three generator states and the logic base seed, which upstream keeps
/// as file-static globals in `RandomValue.cpp`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RandomValues {
    /// `theGameLogicSeed`.
    pub logic: [u32; 6],
    /// `theGameClientSeed`.
    pub client: [u32; 6],
    /// `theGameAudioSeed`.
    pub audio: [u32; 6],
    /// `theGameLogicBaseSeed`.
    pub logic_base_seed: u32,
}

impl Default for RandomValues {
    /// The state before any `InitRandom` call: all three at [`INITIAL_SEED`],
    /// base seed 0.
    fn default() -> Self {
        RandomValues {
            logic: INITIAL_SEED,
            client: INITIAL_SEED,
            audio: INITIAL_SEED,
            logic_base_seed: 0,
        }
    }
}

impl RandomValues {
    /// A state as left by `InitRandom(seed)`.
    pub fn new(seed: u32) -> Self {
        let mut r = RandomValues::default();
        r.init_random(seed);
        r
    }

    /// `void InitRandom(UnsignedInt seed)`: seed all three generators.
    pub fn init_random(&mut self, seed: u32) {
        seed_random(seed, &mut self.audio);
        seed_random(seed, &mut self.client);
        seed_random(seed, &mut self.logic);
        self.logic_base_seed = seed;
    }

    /// `void InitGameLogicRandom(UnsignedInt seed)` (upstream's
    /// `DETERMINISTIC` debugging switch is not defined, so the seed is used).
    pub fn init_game_logic_random(&mut self, seed: u32) {
        seed_random(seed, &mut self.logic);
        self.logic_base_seed = seed;
    }

    /// `UnsignedInt GetGameLogicRandomSeed(void)`: the seed (used for replays).
    pub fn get_game_logic_random_seed(&self) -> u32 {
        self.logic_base_seed
    }

    /// `UnsignedInt GetGameLogicRandomSeedCRC(void)`: CRC of the 24 bytes of
    /// the logic state as laid out in memory on x86 (little-endian words).
    pub fn get_game_logic_random_seed_crc(&self) -> u32 {
        let mut bytes = [0u8; 24];
        for (i, w) in self.logic.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let mut c = Crc::new();
        c.compute_crc(&bytes);
        c.get()
    }

    /// `GetGameLogicRandomValue(lo, hi, file, line)` (macro `GameLogicRandomValue`).
    pub fn game_logic_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        random_int(&mut self.logic, lo, hi)
    }

    /// `GetGameClientRandomValue(lo, hi, file, line)`.
    pub fn game_client_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        random_int(&mut self.client, lo, hi)
    }

    /// `GetGameAudioRandomValue(lo, hi, file, line)`.
    pub fn game_audio_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        random_int(&mut self.audio, lo, hi)
    }

    /// `GetGameLogicRandomValueReal(lo, hi, file, line)`.
    pub fn game_logic_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        random_real(&mut self.logic, lo, hi)
    }

    /// `GetGameClientRandomValueReal(lo, hi, file, line)`.
    pub fn game_client_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        random_real(&mut self.client, lo, hi)
    }

    /// `GetGameAudioRandomValueReal(lo, hi, file, line)`.
    pub fn game_audio_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        random_real(&mut self.audio, lo, hi)
    }
}

/// `GameLogicRandomVariable::DistributionType` / `GameClientRandomVariable::DistributionType`
/// (the two upstream enums are identical).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DistributionType {
    Constant = 0,
    Uniform = 1,
    Gaussian = 2,
    Triangular = 3,
    LowBias = 4,
    HighBias = 5,
}

/// `DistributionTypeNames[]`, as upstream (used when parsing INI files).
pub const DISTRIBUTION_TYPE_NAMES: [&str; 6] = [
    "CONSTANT",
    "UNIFORM",
    "GAUSSIAN",
    "TRIANGULAR",
    "LOW_BIAS",
    "HIGH_BIAS",
];

/// Shared body of the two `RandomVariable::getValue` methods.
/// Only CONSTANT and UNIFORM are implemented upstream; the other types hit a
/// `DEBUG_CRASH` (a no-op in release) and return `0.0f`. A CONSTANT with
/// `low != high` falls through to UNIFORM, as upstream.
fn variable_value(kind: DistributionType, low: f32, high: f32, seed: &mut [u32; 6]) -> f32 {
    match kind {
        DistributionType::Constant if low == high => low,
        DistributionType::Constant | DistributionType::Uniform => random_real(seed, low, high),
        _ => 0.0f32,
    }
}

/// `class GameLogicRandomVariable`: a distribution drawn from the logic stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameLogicRandomVariable {
    pub distribution: DistributionType,
    pub low: f32,
    pub high: f32,
}

impl GameLogicRandomVariable {
    /// `setRange(low, high, type)` as a constructor.
    pub fn new(low: f32, high: f32, distribution: DistributionType) -> Self {
        GameLogicRandomVariable {
            distribution,
            low,
            high,
        }
    }

    /// `setRange(low, high, type = UNIFORM)`.
    pub fn set_range(&mut self, low: f32, high: f32, distribution: DistributionType) {
        self.low = low;
        self.high = high;
        self.distribution = distribution;
    }

    /// `getValue()`: draws from the **logic** stream.
    pub fn get_value(&self, rv: &mut RandomValues) -> f32 {
        variable_value(self.distribution, self.low, self.high, &mut rv.logic)
    }

    pub fn get_minimum_value(&self) -> f32 {
        self.low
    }
    pub fn get_maximum_value(&self) -> f32 {
        self.high
    }
    pub fn get_distribution_type(&self) -> DistributionType {
        self.distribution
    }
}

/// `class GameClientRandomVariable`: a distribution drawn from the client stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameClientRandomVariable {
    pub distribution: DistributionType,
    pub low: f32,
    pub high: f32,
}

impl GameClientRandomVariable {
    /// `setRange(low, high, type)` as a constructor.
    pub fn new(low: f32, high: f32, distribution: DistributionType) -> Self {
        GameClientRandomVariable {
            distribution,
            low,
            high,
        }
    }

    /// `setRange(low, high, type = UNIFORM)`.
    pub fn set_range(&mut self, low: f32, high: f32, distribution: DistributionType) {
        self.low = low;
        self.high = high;
        self.distribution = distribution;
    }

    /// `getValue()`: draws from the **client** stream.
    pub fn get_value(&self, rv: &mut RandomValues) -> f32 {
        variable_value(self.distribution, self.low, self.high, &mut rv.client)
    }

    pub fn get_minimum_value(&self) -> f32 {
        self.low
    }
    pub fn get_maximum_value(&self) -> f32 {
        self.high
    }
    pub fn get_distribution_type(&self) -> DistributionType {
        self.distribution
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_zero_is_the_initial_state() {
        let mut s = [0u32; 6];
        seed_random(0, &mut s);
        assert_eq!(s, INITIAL_SEED);
    }

    #[test]
    fn mult_factor_is_two_to_minus_32() {
        assert_eq!(MULT_FACTOR.to_bits(), (2.0f32).powi(-32).to_bits());
    }

    #[test]
    fn streams_are_independent() {
        let mut a = RandomValues::new(1234);
        let mut b = RandomValues::new(1234);
        for _ in 0..100 {
            a.game_client_random_value(0, 100);
        }
        assert_eq!(
            a.game_logic_random_value(0, 1000),
            b.game_logic_random_value(0, 1000)
        );
    }

    #[test]
    fn variable_types() {
        let mut rv = RandomValues::new(7);
        let c = GameLogicRandomVariable::new(2.5, 2.5, DistributionType::Constant);
        assert_eq!(c.get_value(&mut rv), 2.5);
        let g = GameClientRandomVariable::new(0.0, 1.0, DistributionType::Gaussian);
        assert_eq!(g.get_value(&mut rv), 0.0);
        assert_eq!(
            DISTRIBUTION_TYPE_NAMES[DistributionType::HighBias as usize],
            "HIGH_BIAS"
        );
    }
}
