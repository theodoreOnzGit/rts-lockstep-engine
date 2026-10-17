// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h (lines 70-108: the
//   frame-rate constants and unit conversions; lines 110-198: player count
//   and mask, general types, difficulty, player type, shroud states, guard
//   modes, NEVER/FOREVER; lines 204-221 and 292-310: the veterancy levels and
//   their flag helpers; lines 225-287: command sources and attack types;
//   lines 475-510: turrets, normalizeAngle, stdAngleDiff, Relationship)
//   GeneralsMD/Code/GameEngine/Source/Common/System/GameCommon.cpp
//   (TheVeterancyNames, TheRelationshipNames, normalizeAngle)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   GameCommon.h: Steven Johnson, October 2001
//   GameCommon.cpp: John McDonald, Aug 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GameCommon.h`: the logic frame rate and the per-second to per-frame unit
//! conversions, the player limits and mask, the small game-wide enums
//! (difficulty, player type, shroud states, guard modes, veterancy, command
//! sources, attack types, turrets, relationships), and the angle helpers.
//!
//! The constants are computed in `f32`, as upstream's `const Real`
//! initialisers are (one rounding per operation).
//!
//! Not translated: the intrusive doubly-linked-list macros (`MAKE_DLINK_HEAD`,
//! `MAKE_DLINK`, `DLINK_ITERATOR`) and `BOGUSPTR`; the Rust keeps owned
//! containers and ids instead of pointers.
//!
//! **Upstream quirk:** `#if MAX_PLAYER_COUNT <= 16` tests an enumerator in the
//! preprocessor, where every identifier is 0, so the check always passes; it
//! happens to be true (16 <= 16), and `PlayerMaskType` is 16 bits.

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
/// helpers use, **as the shipped 32-bit x86 build computes it** (owner
/// decision, 2026-10-17). **Upstream bug:** for `LEVEL_REGULAR` (0) the shift
/// count is -1, undefined behaviour in C++. The shipped game's `shl` masks its
/// count to 5 bits, so `REGULAR` gets bit 31, consistently in get, set and
/// clear, and INI lines such as `-REGULAR` work in the original. This
/// reproduces that: the count is `(dt - 1) & 31`. A 64-bit build of the C++
/// gives a different answer, so the differential tests cannot compare this
/// case; `regular_is_bit_31_as_shipped` pins it instead.
fn veterancy_bit(dt: i32) -> u32 {
    1u32 << (dt.wrapping_sub(1) as u32 & 31)
}

/// `getVeterancyLevelFlag` (see
/// [`set_veterancy_level_flag`]).
pub fn get_veterancy_level_flag(flags: VeterancyLevelFlags, dt: i32) -> bool {
    flags & veterancy_bit(dt) != 0
}

/// `setVeterancyLevelFlag`: `flags | (1UL << (dt - 1))`; `LEVEL_REGULAR` is
/// bit 31, as shipped (see `veterancy_bit`).
pub fn set_veterancy_level_flag(flags: VeterancyLevelFlags, dt: i32) -> VeterancyLevelFlags {
    flags | veterancy_bit(dt)
}

/// `clearVeterancyLevelFlag`: `flags & ~(1UL << (dt - 1))`; `LEVEL_REGULAR`
/// is bit 31, as shipped (see `veterancy_bit`).
pub fn clear_veterancy_level_flag(flags: VeterancyLevelFlags, dt: i32) -> VeterancyLevelFlags {
    flags & !veterancy_bit(dt)
}

/// `MAX_PLAYER_COUNT`.
pub const MAX_PLAYER_COUNT: i32 = 16;
/// `typedef UnsignedShort PlayerMaskType`: one bit per player.
pub type PlayerMaskType = u16;
/// `PLAYERMASK_ALL`.
pub const PLAYERMASK_ALL: PlayerMaskType = 0xffff;
/// `PLAYERMASK_NONE`.
pub const PLAYERMASK_NONE: PlayerMaskType = 0x0;

/// `MAX_GLOBAL_GENERAL_TYPES`: playable General types, not the boss.
pub const MAX_GLOBAL_GENERAL_TYPES: i32 = 9;
/// `GLOBAL_GENERAL_BEGIN`: the first playable General's player template.
pub const GLOBAL_GENERAL_BEGIN: i32 = 5;
/// `GLOBAL_GENERAL_END`.
pub const GLOBAL_GENERAL_END: i32 = GLOBAL_GENERAL_BEGIN + MAX_GLOBAL_GENERAL_TYPES - 1;

/// `enum GameDifficulty`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum GameDifficulty {
    Easy = 0,
    Normal = 1,
    Hard = 2,
}
/// `DIFFICULTY_COUNT`.
pub const DIFFICULTY_COUNT: i32 = 3;

/// `enum PlayerType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum PlayerType {
    Human = 0,
    Computer = 1,
}
/// `PLAYERTYPE_COUNT`.
pub const PLAYERTYPE_COUNT: i32 = 2;

/// `enum CellShroudStatus`: the shroud state of one partition cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum CellShroudStatus {
    Clear = 0,
    Fogged = 1,
    Shrouded = 2,
}
/// `CELLSHROUD_COUNT`.
pub const CELLSHROUD_COUNT: i32 = 3;

/// `enum ObjectShroudStatus`: the shroud state of a whole object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum ObjectShroudStatus {
    Invalid = 0,
    Clear = 1,
    PartialClear = 2,
    Fogged = 3,
    Shrouded = 4,
    /// Recompute, but the previous status is valid (save/load).
    InvalidButPreviousValid = 5,
}
/// `OBJECTSHROUD_COUNT`.
pub const OBJECTSHROUD_COUNT: i32 = 6;

/// `enum GuardMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum GuardMode {
    Normal = 0,
    /// No pursuit out of the guard area.
    GuardWithoutPursuit = 1,
    /// Ignore non-flyers.
    GuardFlyingUnitsOnly = 2,
}

/// `NEVER` (a frame number).
pub const NEVER: u32 = 0;
/// `FOREVER`: `0x3fffffff`, so offsets can be added without overflow.
pub const FOREVER: u32 = 0x3fff_ffff;

/// `enum CommandSourceType`: who issued a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum CommandSourceType {
    FromPlayer = 0,
    FromScript = 1,
    FromAi = 2,
    /// A dozer attacking a mine (mines are not AI-attackable).
    FromDozer = 3,
    /// A selectable weapon's default case.
    DefaultSwitchWeapon = 4,
}

/// `enum AbleToAttackType`: a bit set (`_ATTACK_FORCED`, `_ATTACK_CONTINUED`,
/// `_ATTACK_TUNNELNETWORK_GUARD`); the named values are its combinations.
pub type AbleToAttackType = i32;
/// `_ATTACK_FORCED`.
pub const ATTACK_FORCED_BIT: AbleToAttackType = 0x01;
/// `_ATTACK_CONTINUED`.
pub const ATTACK_CONTINUED_BIT: AbleToAttackType = 0x02;
/// `_ATTACK_TUNNELNETWORK_GUARD`.
pub const ATTACK_TUNNELNETWORK_GUARD_BIT: AbleToAttackType = 0x04;
/// `ATTACK_NEW_TARGET`.
pub const ATTACK_NEW_TARGET: AbleToAttackType = 0;
/// `ATTACK_NEW_TARGET_FORCED`.
pub const ATTACK_NEW_TARGET_FORCED: AbleToAttackType = ATTACK_FORCED_BIT;
/// `ATTACK_CONTINUED_TARGET`.
pub const ATTACK_CONTINUED_TARGET: AbleToAttackType = ATTACK_CONTINUED_BIT;
/// `ATTACK_CONTINUED_TARGET_FORCED`.
pub const ATTACK_CONTINUED_TARGET_FORCED: AbleToAttackType = ATTACK_FORCED_BIT | ATTACK_CONTINUED_BIT;
/// `ATTACK_TUNNEL_NETWORK_GUARD`.
pub const ATTACK_TUNNEL_NETWORK_GUARD: AbleToAttackType = ATTACK_TUNNELNETWORK_GUARD_BIT;

/// `isForcedAttack(t)`.
pub fn is_forced_attack(t: AbleToAttackType) -> bool {
    t & ATTACK_FORCED_BIT != 0
}

/// `isContinuedAttack(t)`.
pub fn is_continued_attack(t: AbleToAttackType) -> bool {
    t & ATTACK_CONTINUED_BIT != 0
}

/// `enum WhichTurretType`.
pub type WhichTurretType = i32;
/// `TURRET_INVALID`.
pub const TURRET_INVALID: WhichTurretType = -1;
/// `TURRET_MAIN`.
pub const TURRET_MAIN: WhichTurretType = 0;
/// `TURRET_ALT`.
pub const TURRET_ALT: WhichTurretType = 1;
/// `MAX_TURRETS`.
pub const MAX_TURRETS: WhichTurretType = 2;

/// `enum Relationship` (keep in sync with [`RELATIONSHIP_NAMES`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum Relationship {
    Enemies = 0,
    Neutral = 1,
    Allies = 2,
}

/// `TheRelationshipNames[]` (GameCommon.cpp), without the `NULL` terminator.
pub const RELATIONSHIP_NAMES: [&str; 3] = ["ENEMIES", "NEUTRAL", "ALLIES"];

/// `normalizeAngle` would loop forever: the angle is infinite, or so large
/// that adding or subtracting `2*PI` no longer changes it in `float`.
/// Upstream hangs there (a side-effect-free infinite loop, which C++ also
/// makes undefined).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NormalizeAngleHangs;

/// `normalizeAngle(angle)`, checked (GameCommon.cpp): NaN gives 0 (upstream
/// asserts in debug builds and then returns 0); otherwise `2*PI` is
/// subtracted while `angle > PI` and added while `angle <= -PI`, one step at
/// a time as upstream does (the rounding of each step is part of the result).
pub fn try_normalize_angle(mut angle: f32) -> Result<f32, NormalizeAngleHangs> {
    if angle.is_nan() {
        return Ok(0.0);
    }
    const TWO_PI_EXPR: f32 = 2.0 * PI;
    while angle > PI {
        let next = angle - TWO_PI_EXPR;
        if next == angle {
            return Err(NormalizeAngleHangs);
        }
        angle = next;
    }
    while angle <= -PI {
        let next = angle + TWO_PI_EXPR;
        if next == angle {
            return Err(NormalizeAngleHangs);
        }
        angle = next;
    }
    Ok(angle)
}

/// `normalizeAngle(angle)`; panics where upstream would hang.
pub fn normalize_angle(angle: f32) -> f32 {
    match try_normalize_angle(angle) {
        Ok(a) => a,
        Err(NormalizeAngleHangs) => panic!("normalizeAngle({angle}) never terminates upstream"),
    }
}

/// `stdAngleDiff(a1, a2)`: `normalizeAngle(a1 - a2)`, checked.
pub fn try_std_angle_diff(a1: f32, a2: f32) -> Result<f32, NormalizeAngleHangs> {
    try_normalize_angle(a1 - a2)
}

/// `stdAngleDiff(a1, a2)`; panics where upstream would hang.
pub fn std_angle_diff(a1: f32, a2: f32) -> f32 {
    normalize_angle(a1 - a2)
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
    fn regular_is_bit_31_as_shipped() {
        // The shipped 32-bit build's `shl` masks a count of -1 to 31.
        let regular = VeterancyLevel::Regular as i32;
        assert_eq!(set_veterancy_level_flag(0, regular), 1 << 31);
        assert_eq!(clear_veterancy_level_flag(VETERANCY_LEVEL_FLAGS_ALL, regular), 0x7fff_ffff);
        assert!(get_veterancy_level_flag(1 << 31, regular));
        assert!(!get_veterancy_level_flag(0x7fff_ffff, regular));
        assert_eq!(set_veterancy_level_flag(0, VeterancyLevel::Veteran as i32), 1);
        assert_eq!(clear_veterancy_level_flag(0xffff_ffff, VeterancyLevel::Heroic as i32), 0xffff_fffb);
        assert!(get_veterancy_level_flag(4, VeterancyLevel::Heroic as i32));
    }

    #[test]
    fn normalize_angle_edges() {
        assert_eq!(try_normalize_angle(f32::NAN), Ok(0.0));
        assert_eq!(try_normalize_angle(PI), Ok(PI));
        let minus_pi = -PI;
        assert_eq!(try_normalize_angle(minus_pi), Ok(minus_pi + 2.0 * PI));
        assert_eq!(try_normalize_angle(f32::INFINITY), Err(NormalizeAngleHangs));
        assert_eq!(try_normalize_angle(-1.0e30), Err(NormalizeAngleHangs));
        assert!(try_normalize_angle(1.0e6).is_ok());
        assert!(is_forced_attack(ATTACK_CONTINUED_TARGET_FORCED));
        assert!(!is_continued_attack(ATTACK_NEW_TARGET_FORCED));
    }
}
