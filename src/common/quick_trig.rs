// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/QuickTrig.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/QuickTrig.cpp (the tables)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Mark Lorenzen (adapted by srj)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `QuickTrig.h`: coarse table-based `QSin`, `QCos`, `QTan`, `QCsc`, `QSec`,
//! `QCot` (129-entry tables, no interpolation) and the rough magnitude
//! estimate `QMag`. Upstream uses them only in client code (particles,
//! terrain, model drawing); `GameLogic` includes the header in one file
//! (`TensileFormationUpdate.cpp`) without calling it.
//!
//! # Where upstream reads outside its tables
//!
//! Upstream indexes the tables without a bounds check, and some ordinary
//! inputs reach past the end, which is undefined behaviour in C++:
//!
//! - `QSin(a)` reads index 129 of a 129-entry table whenever the reduced
//!   angle is exactly `PI/2`, so **`QSin(PI/2)` and `QCos(0.0)` read out of
//!   bounds**, and so does `QCos(x)` for every `|x|` small enough that
//!   `PI/2 - x` rounds to `PI/2` (below about `6e-8`); NaN gives index
//!   `i32::MIN`.
//! - `QTan(a)` indexes with `REAL_TO_INT(a * 129)`, so any `a < 0` or
//!   `a >= 1.0` (radians) is out of bounds.
//! - `QSin`'s `while (angle > PI) angle -= PI;` never ends for an angle where
//!   `angle - PI == angle` in `f32` (infinity, and finite `|a|` above about
//!   `2^25`).
//!
//! The `try_*` functions return a [`QuickTrigError`] in those cases; the plain
//! functions panic with that message rather than read memory or hang. On
//! every input where upstream is defined they return upstream's value (the
//! differential tests check this).

use super::base_type::{real_to_int, PI};

/// `#define QUARTER_CIRCLE (PI/2)`.
pub const QUARTER_CIRCLE: f32 = PI / 2.0;

/// `Real TheQuickSinTable[]` (129 entries).
pub static QUICK_SIN_TABLE: [f32; 129] = [
    0.00000, 0.01237, 0.02473, 0.03710, 0.04945, 0.06180, 0.07414, 0.08647, 0.09879, 0.11109,
    0.12337, 0.13563, 0.14788, 0.16010, 0.17229, 0.18446, 0.19661, 0.20872, 0.22080, 0.23284,
    0.24485, 0.25683, 0.26876, 0.28065, 0.29250, 0.30431, 0.31607, 0.32778, 0.33944, 0.35104,
    0.36260, 0.37410, 0.38554, 0.39692, 0.40824, 0.41950, 0.43070, 0.44183, 0.45289, 0.46388,
    0.47480, 0.48565, 0.49643, 0.50712, 0.51774, 0.52829, 0.53875, 0.54913, 0.55942, 0.56963,
    0.57975, 0.58978, 0.59973, 0.60958, 0.61934, 0.62900, 0.63857, 0.64804, 0.65741, 0.66668,
    0.67584, 0.68491, 0.69387, 0.70272, 0.71147, 0.72010, 0.72863, 0.73705, 0.74535, 0.75354,
    0.76161, 0.76957, 0.77741, 0.78513, 0.79273, 0.80020, 0.80756, 0.81479, 0.82190, 0.82888,
    0.83574, 0.84247, 0.84907, 0.85554, 0.86187, 0.86808, 0.87415, 0.88009, 0.88590, 0.89157,
    0.89710, 0.90250, 0.90775, 0.91287, 0.91785, 0.92269, 0.92739, 0.93195, 0.93636, 0.94063,
    0.94476, 0.94874, 0.95257, 0.95626, 0.95981, 0.96321, 0.96646, 0.96956, 0.97251, 0.97532,
    0.97798, 0.98048, 0.98284, 0.98505, 0.98710, 0.98901, 0.99076, 0.99236, 0.99381, 0.99511,
    0.99625, 0.99725, 0.99809, 0.99878, 0.99931, 0.99969, 0.99992, 1.00000, 0.99992,
];
/// `Real TheQuickTanTable[]` (129 entries).
pub static QUICK_TAN_TABLE: [f32; 129] = [
    0.00000, 0.00787, 0.01575, 0.02363, 0.03151, 0.03939, 0.04728, 0.05517, 0.06308, 0.07099,
    0.07890, 0.08683, 0.09477, 0.10272, 0.11068, 0.11866, 0.12666, 0.13466, 0.14269, 0.15073,
    0.15880, 0.16688, 0.17498, 0.18311, 0.19126, 0.19943, 0.20763, 0.21586, 0.22412, 0.23240,
    0.24071, 0.24906, 0.25744, 0.26585, 0.27430, 0.28279, 0.29131, 0.29987, 0.30847, 0.31712,
    0.32581, 0.33454, 0.34332, 0.35214, 0.36102, 0.36994, 0.37892, 0.38795, 0.39704, 0.40618,
    0.41539, 0.42465, 0.43398, 0.44337, 0.45282, 0.46234, 0.47194, 0.48160, 0.49134, 0.50115,
    0.51104, 0.52101, 0.53106, 0.54120, 0.55143, 0.56174, 0.57214, 0.58264, 0.59324, 0.60393,
    0.61473, 0.62563, 0.63664, 0.64777, 0.65900, 0.67035, 0.68183, 0.69342, 0.70515, 0.71700,
    0.72899, 0.74112, 0.75339, 0.76581, 0.77838, 0.79110, 0.80398, 0.81703, 0.83025, 0.84363,
    0.85720, 0.87096, 0.88490, 0.89904, 0.91338, 0.92793, 0.94269, 0.95767, 0.97288, 0.98833,
    1.00401, 1.01995, 1.03615, 1.05261, 1.06935, 1.08637, 1.10368, 1.12130, 1.13924, 1.15749,
    1.17609, 1.19503, 1.21433, 1.23400, 1.25406, 1.27452, 1.29540, 1.31670, 1.33845, 1.36067,
    1.38336, 1.40656, 1.43027, 1.45453, 1.47935, 1.50475, 1.53076, 1.55741, 1.58471,
];

/// `Real TheQuickSinTableCount = sizeof(TheQuickSinTable) / sizeof(Real)`
/// ("yes, Real. No, really.").
pub const QUICK_SIN_TABLE_COUNT: f32 = QUICK_SIN_TABLE.len() as f32;
/// `Real TheQuickTanTableCount`, likewise.
pub const QUICK_TAN_TABLE_COUNT: f32 = QUICK_TAN_TABLE.len() as f32;

/// Why a quick-trig call has no defined upstream result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickTrigError {
    /// The table index upstream would read is outside the 129-entry table.
    IndexOutOfTable(i32),
    /// `QSin`'s range-reduction loop would never terminate (the angle is
    /// infinite or so large that subtracting `PI` does not change it).
    AngleTooLarge,
}

impl std::fmt::Display for QuickTrigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuickTrigError::IndexOutOfTable(i) => write!(
                f,
                "QuickTrig: table index {i} is outside the 129-entry table (out-of-bounds read upstream)"
            ),
            QuickTrigError::AngleTooLarge => {
                write!(f, "QuickTrig: QSin range reduction never terminates for this angle (upstream hangs)")
            }
        }
    }
}

impl std::error::Error for QuickTrigError {}

fn lookup(table: &[f32; 129], index: i32) -> Result<f32, QuickTrigError> {
    usize::try_from(index)
        .ok()
        .and_then(|i| table.get(i).copied())
        .ok_or(QuickTrigError::IndexOutOfTable(index))
}

fn unwrap_or_panic(r: Result<f32, QuickTrigError>) -> f32 {
    match r {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

/// `QMag(x, y, z)`: rough magnitude (max + (mid + min) / 4), "may be off by
/// 10% or more". The swaps are upstream's, which do not fully sort.
pub fn qmag(x: f32, y: f32, z: f32) -> f32 {
    let mut s_temp_v;
    // `fabs` is exact, so it does not matter that upstream's is the double one.
    let mut s_max_v = x.abs();
    let mut s_med_v = y.abs();
    let mut s_min_v = z.abs();

    if s_max_v < s_med_v {
        s_temp_v = s_max_v;
        s_max_v = s_med_v;
        s_med_v = s_temp_v;
    }

    if s_max_v < s_min_v {
        s_temp_v = s_max_v;
        s_max_v = s_min_v;
        s_min_v = s_temp_v;
    }

    s_med_v += s_min_v;
    s_max_v += s_med_v * 0.25f32;

    s_max_v
}

/// `QSin(a)`, checked. See the module docs for the error cases.
pub fn try_qsin(a: f32) -> Result<f32, QuickTrigError> {
    let mut angle = a;
    let mut sgn: i32 = 1;

    if angle < 0.0 {
        sgn = -1;
        angle = -angle;
    }

    while angle > PI {
        let next = angle - PI;
        if next == angle {
            return Err(QuickTrigError::AngleTooLarge);
        }
        angle = next;
        sgn = -sgn;
    }

    if angle > PI / 2.0 {
        angle = PI - angle;
    }

    // Upstream scales by TheQuickTanTableCount here (both tables have 129).
    let index = real_to_int((angle / QUARTER_CIRCLE) * QUICK_TAN_TABLE_COUNT);
    let x = lookup(&QUICK_SIN_TABLE, index)?;

    Ok(x * sgn as f32)
}

/// `QSin(a)`. Panics where upstream is undefined (see the module docs).
pub fn qsin(a: f32) -> f32 {
    unwrap_or_panic(try_qsin(a))
}

/// `QCos(angle)`, checked: `QSin((QUARTER_CIRCLE) - angle)`.
pub fn try_qcos(angle: f32) -> Result<f32, QuickTrigError> {
    try_qsin(QUARTER_CIRCLE - angle)
}

/// `QCos(angle)`. Panics where upstream is undefined, including `QCos(0.0)`.
pub fn qcos(angle: f32) -> f32 {
    unwrap_or_panic(try_qcos(angle))
}

/// `QTan(angle)`, checked: `TheQuickTanTable[REAL_TO_INT(angle * TheQuickSinTableCount)]`.
pub fn try_qtan(angle: f32) -> Result<f32, QuickTrigError> {
    lookup(&QUICK_TAN_TABLE, real_to_int(angle * QUICK_SIN_TABLE_COUNT))
}

/// `QTan(angle)`. Panics unless `0 <= angle < 1` (upstream reads out of bounds).
pub fn qtan(angle: f32) -> f32 {
    unwrap_or_panic(try_qtan(angle))
}

/// `QCsc(angle)`: `1.0f / QSin(angle)`.
pub fn qcsc(angle: f32) -> f32 {
    1.0f32 / qsin(angle)
}

/// `QSec(angle)`: `1.0f / QCos(angle)`.
pub fn qsec(angle: f32) -> f32 {
    1.0f32 / qcos(angle)
}

/// `QCot(angle)`: `1.0f / QTan(angle)`.
pub fn qcot(angle: f32) -> f32 {
    1.0f32 / qtan(angle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_out_of_bounds_cases_are_reported() {
        assert_eq!(try_qcos(0.0), Err(QuickTrigError::IndexOutOfTable(129)));
        assert_eq!(try_qsin(f32::INFINITY), Err(QuickTrigError::AngleTooLarge));
        assert_eq!(
            try_qsin(f32::NAN),
            Err(QuickTrigError::IndexOutOfTable(i32::MIN))
        );
        assert_eq!(try_qtan(1.0), Err(QuickTrigError::IndexOutOfTable(129)));
        assert_eq!(try_qtan(-0.5), Err(QuickTrigError::IndexOutOfTable(-64)));
        assert_eq!(qsin(0.0), 0.0);
        assert_eq!(qmag(3.0, -4.0, 1.0), 4.0 + (3.0 + 1.0) * 0.25);
    }

    #[test]
    #[should_panic(expected = "outside the 129-entry table")]
    fn qcos_zero_panics() {
        qcos(0.0);
    }
}
