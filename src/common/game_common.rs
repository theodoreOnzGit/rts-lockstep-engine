// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h (lines 70-108: the
//   frame-rate constants and unit conversions; lines 204-221 and 292-310: the
//   veterancy levels and their flag helpers)
//   GeneralsMD/Code/GameEngine/Source/Common/System/GameCommon.cpp (lines
//   33-40: TheVeterancyNames)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   GameCommon.cpp: John McDonald, Aug 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The parts of `GameCommon.h` that the INI field parsers use: the logic
//! frame rate, the unit conversions from per-second to per-frame values, and
//! the veterancy levels.
//!
//! The constants are computed in `f32`, as upstream's `const Real`
//! initialisers are (one rounding per operation).

use super::base_type::PI;

/// `LOGICFRAMES_PER_SECOND`.
pub const LOGICFRAMES_PER_SECOND: i32 = 30;
/// `MSEC_PER_SECOND`.
pub const MSEC_PER_SECOND: i32 = 1000;
/// `LOGICFRAMES_PER_MSEC_REAL = (Real)30 / (Real)1000`.
pub const LOGICFRAMES_PER_MSEC_REAL: f32 = LOGICFRAMES_PER_SECOND as f32 / MSEC_PER_SECOND as f32;
/// `MSEC_PER_LOGICFRAME_REAL = (Real)1000 / (Real)30`.
pub const MSEC_PER_LOGICFRAME_REAL: f32 = MSEC_PER_SECOND as f32 / LOGICFRAMES_PER_SECOND as f32;
/// `LOGICFRAMES_PER_SECONDS_REAL = (Real)30`.
pub const LOGICFRAMES_PER_SECONDS_REAL: f32 = LOGICFRAMES_PER_SECOND as f32;
/// `SECONDS_PER_LOGICFRAME_REAL = 1.0f / LOGICFRAMES_PER_SECONDS_REAL`.
pub const SECONDS_PER_LOGICFRAME_REAL: f32 = 1.0 / LOGICFRAMES_PER_SECONDS_REAL;

/// `ConvertDurationFromMsecsToFrames`: a `Real` number of frames (callers
/// usually take its ceiling).
pub fn convert_duration_from_msecs_to_frames(msec: f32) -> f32 {
    msec * LOGICFRAMES_PER_MSEC_REAL
}

/// `ConvertVelocityInSecsToFrames` ("this looks wrong, but is the correct
/// conversion factor": distance per second to distance per frame).
pub fn convert_velocity_in_secs_to_frames(dist_per_msec: f32) -> f32 {
    dist_per_msec * SECONDS_PER_LOGICFRAME_REAL
}

/// `ConvertAccelerationInSecsToFrames`.
pub fn convert_acceleration_in_secs_to_frames(dist_per_sec2: f32) -> f32 {
    const SEC_PER_LOGICFRAME_SQR: f32 = SECONDS_PER_LOGICFRAME_REAL * SECONDS_PER_LOGICFRAME_REAL;
    dist_per_sec2 * SEC_PER_LOGICFRAME_SQR
}

/// `ConvertAngularVelocityInDegreesPerSecToRadsPerFrame`.
pub fn convert_angular_velocity_in_degrees_per_sec_to_rads_per_frame(deg_per_sec: f32) -> f32 {
    const RADS_PER_DEGREE: f32 = PI / 180.0;
    deg_per_sec * (SECONDS_PER_LOGICFRAME_REAL * RADS_PER_DEGREE)
}

/// `enum VeterancyLevel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum VeterancyLevel {
    Regular = 0,
    Veteran = 1,
    Elite = 2,
    Heroic = 3,
}

/// `LEVEL_COUNT`.
pub const LEVEL_COUNT: i32 = 4;

/// `TheVeterancyNames[]` (GameCommon.cpp), without the `NULL` terminator.
pub const VETERANCY_NAMES: [&str; 4] = ["REGULAR", "VETERAN", "ELITE", "HEROIC"];

/// `typedef UnsignedInt VeterancyLevelFlags`.
pub type VeterancyLevelFlags = u32;
/// `VETERANCY_LEVEL_FLAGS_ALL`.
pub const VETERANCY_LEVEL_FLAGS_ALL: VeterancyLevelFlags = 0xffff_ffff;
/// `VETERANCY_LEVEL_FLAGS_NONE`.
pub const VETERANCY_LEVEL_FLAGS_NONE: VeterancyLevelFlags = 0x0000_0000;

/// The bit of `1UL << (dt - 1)`, the expression all three veterancy flag
/// helpers use. **Upstream bug:** for `LEVEL_REGULAR` (0) the shift count is
/// -1, which is undefined behaviour; the shipped 32-bit x86 build computes bit
/// 31 there (the `shl` masks its count to 5 bits), a 64-bit build bit 63
/// truncated to 0. `None` for that case (and for levels outside 1..=32).
fn veterancy_bit(dt: i32) -> Option<u32> {
    let shift = dt.checked_sub(1)?;
    if (0..32).contains(&shift) {
        Some(1u32 << shift)
    } else {
        None
    }
}

/// `getVeterancyLevelFlag`; `None` where upstream is undefined (see
/// [`set_veterancy_level_flag`]).
pub fn get_veterancy_level_flag(flags: VeterancyLevelFlags, dt: i32) -> Option<bool> {
    veterancy_bit(dt).map(|b| flags & b != 0)
}

/// `setVeterancyLevelFlag`: `flags | (1UL << (dt - 1))`. `None` for
/// `LEVEL_REGULAR`, where upstream shifts by -1 (undefined behaviour).
pub fn set_veterancy_level_flag(flags: VeterancyLevelFlags, dt: i32) -> Option<VeterancyLevelFlags> {
    veterancy_bit(dt).map(|b| flags | b)
}

/// `clearVeterancyLevelFlag`: `flags & ~(1UL << (dt - 1))`. `None` for
/// `LEVEL_REGULAR` (see [`set_veterancy_level_flag`]).
pub fn clear_veterancy_level_flag(flags: VeterancyLevelFlags, dt: i32) -> Option<VeterancyLevelFlags> {
    veterancy_bit(dt).map(|b| flags & !b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_are_single_precision() {
        assert_eq!(LOGICFRAMES_PER_MSEC_REAL, 30.0f32 / 1000.0f32);
        assert_eq!(convert_duration_from_msecs_to_frames(1000.0), 1000.0f32 * (30.0f32 / 1000.0f32));
        assert_eq!(SECONDS_PER_LOGICFRAME_REAL.to_bits(), (1.0f32 / 30.0f32).to_bits());
    }

    #[test]
    fn regular_has_no_defined_bit() {
        assert_eq!(set_veterancy_level_flag(0, VeterancyLevel::Regular as i32), None);
        assert_eq!(set_veterancy_level_flag(0, VeterancyLevel::Veteran as i32), Some(1));
        assert_eq!(clear_veterancy_level_flag(0xffff_ffff, VeterancyLevel::Heroic as i32), Some(0xffff_fffb));
        assert_eq!(get_veterancy_level_flag(4, VeterancyLevel::Heroic as i32), Some(true));
    }
}
