//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sampler.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/sampler.cpp
//!
//! Hector Yee's multidimensional samplers (6/11/01): random, regular hypergrid, stratified and quasi-Monte-Carlo
//! (Halton-Hammersley). The abstract `SamplingClass` with four subclasses is the [`SamplingClass`] enum. The file-scope
//! `Random4Class Random` is passed in (`crate::ww::random::Random4`), as the random draws are shared state upstream.
//! `RadInv` and the samplers do their float arithmetic in `float` (the `int` residue converts to float). The QMC
//! constructor `assert(Dimensions<100)` (the prime table has 100 entries).

use crate::ww::random::Random4;

/// "first 100 primes".
const PRIMES: [i32; 100] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193, 197, 199, 211, 223,
    227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281, 283, 293, 307, 311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379, 383, 389, 397, 401, 409, 419, 421, 431, 433, 439, 443, 449, 457, 461, 463, 467,
    479, 487, 491, 499, 503, 509, 521, 523, 541,
];

/// `RadInv(i, base)`: "returns the radical inverse of i in base b: write a number in base b and reverse it over the
/// decimal point".
fn rad_inv(mut i: i32, base: i32) -> f32 {
    let mut sum = 0.0f32;
    let mut power = 1.0f32 / base as f32;
    while i != 0 {
        let residue = i % base;
        i /= base;
        sum += residue as f32 * power;
        power /= base as f32;
    }
    sum
}

/// "add 1 and carry mod Divisions": the odometer shared by the regular and stratified samplers.
fn advance_index(index: &mut [u8], divisions: u8) {
    for i in 0..index.len() {
        index[i] = index[i].wrapping_add(1);
        if index[i] < divisions {
            break;
        }
        index[i] = 0;
    }
}

/// `class SamplingClass` and its subclasses; each `Sample` fills a vector of `Dimensions` values in 0..1.
#[derive(Clone, Debug)]
pub enum SamplingClass {
    /// `RandomSamplingClass`: uniform random (divisions ignored).
    Random { dimensions: u32, divisions: u8 },
    /// `RegularSamplingClass`: a regular hypergrid including 1.0.
    Regular { dimensions: u32, divisions: u8, index: Vec<u8> },
    /// `StratifiedSamplingClass`: a regular hypergrid with random perturbations.
    Stratified { dimensions: u32, divisions: u8, index: Vec<u8> },
    /// `QMCSamplingClass`: quasi-Monte-Carlo (divisions ignored).
    Qmc { dimensions: u32, divisions: u8, index: u32 },
}

impl SamplingClass {
    /// `RandomSamplingClass(dimensions, divisions = 0)`.
    pub fn new_random(dimensions: u32, divisions: u8) -> Self {
        SamplingClass::Random { dimensions, divisions }
    }

    /// `RegularSamplingClass(dimensions, divisions = 3)`: the index starts zeroed (`Reset()`).
    pub fn new_regular(dimensions: u32, divisions: u8) -> Self {
        SamplingClass::Regular { dimensions, divisions, index: vec![0; dimensions as usize] }
    }

    /// `StratifiedSamplingClass(dimensions, divisions = 3)`.
    pub fn new_stratified(dimensions: u32, divisions: u8) -> Self {
        SamplingClass::Stratified { dimensions, divisions, index: vec![0; dimensions as usize] }
    }

    /// `QMCSamplingClass(dimensions, divisions = 0)`: asserts `Dimensions < 100`.
    pub fn new_qmc(dimensions: u32, divisions: u8) -> Self {
        assert!(dimensions < 100);
        SamplingClass::Qmc { dimensions, divisions, index: 0 }
    }

    /// `Reset()`.
    pub fn reset(&mut self) {
        match self {
            SamplingClass::Random { .. } => {}
            SamplingClass::Regular { index, .. } | SamplingClass::Stratified { index, .. } => {
                for b in index.iter_mut() {
                    *b = 0;
                }
            }
            SamplingClass::Qmc { index, .. } => *index = 0,
        }
    }

    /// `QMCSamplingClass::Set_Offset(offset)` (other kinds: nothing).
    pub fn set_offset(&mut self, offset: u32) {
        if let SamplingClass::Qmc { index, .. } = self {
            *index = offset;
        }
    }

    /// `Sample(target)`: fills `target[0..Dimensions]` and advances the sampler's state. `random` is the shared
    /// `Random4Class Random`.
    pub fn sample(&mut self, target: &mut [f32], random: &mut Random4) {
        match self {
            SamplingClass::Random { dimensions, .. } => {
                for i in 0..*dimensions as usize {
                    target[i] = random.get_float();
                }
            }
            SamplingClass::Regular { dimensions, divisions, index } => {
                for i in 0..*dimensions as usize {
                    // minus one because we want to get 1.0f also
                    target[i] = index[i] as f32 / (*divisions as f32 - 1.0f32);
                }
                advance_index(index, *divisions);
            }
            SamplingClass::Stratified { dimensions, divisions, index } => {
                for i in 0..*dimensions as usize {
                    target[i] = (index[i] as f32 + random.get_float()) / *divisions as f32;
                }
                advance_index(index, *divisions);
            }
            SamplingClass::Qmc { dimensions, index, .. } => {
                for i in 0..*dimensions as usize {
                    target[i] = rad_inv(*index as i32, PRIMES[i]);
                }
                *index = index.wrapping_add(1);
            }
        }
    }
}
