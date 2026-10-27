// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AIUpdate.h (the enums and constants before AIUpdateModuleData, 66-189)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The enums and constants of `AIUpdate.h` that the AI's parts share (`p1w13-sonnet`, layer 10, batch T1). Values as upstream's:
//! several are saved ("you MUST NOT REMOVE OR CHANGE existing values").

/// `FAST_AS_POSSIBLE`.
pub const FAST_AS_POSSIBLE: f32 = 999_999.0;

/// `enum LocomotorSetType` (saved).
pub type LocomotorSetType = i32;
pub const LOCOMOTORSET_INVALID: LocomotorSetType = -1;
pub const LOCOMOTORSET_NORMAL: LocomotorSetType = 0;
pub const LOCOMOTORSET_NORMAL_UPGRADED: LocomotorSetType = 1;
pub const LOCOMOTORSET_FREEFALL: LocomotorSetType = 2;
pub const LOCOMOTORSET_WANDER: LocomotorSetType = 3;
pub const LOCOMOTORSET_PANIC: LocomotorSetType = 4;
/// "set used for normally-airborne items while taxiing on ground"
pub const LOCOMOTORSET_TAXIING: LocomotorSetType = 5;
/// "set used for high-speed attacks"
pub const LOCOMOTORSET_SUPERSONIC: LocomotorSetType = 6;
/// "set used for abnormally slow (but not damaged) speeds"
pub const LOCOMOTORSET_SLUGGISH: LocomotorSetType = 7;
pub const LOCOMOTORSET_COUNT: LocomotorSetType = 8;
/// `TheLocomotorSetNames[]` (under `DEFINE_LOCOMOTORSET_NAMES`), without the `NULL`.
pub const LOCOMOTOR_SET_NAMES: [&str; 8] =
    ["SET_NORMAL", "SET_NORMAL_UPGRADED", "SET_FREEFALL", "SET_WANDER", "SET_PANIC", "SET_TAXIING", "SET_SUPERSONIC", "SET_SLUGGISH"];

/// `enum GuardTargetType`.
pub type GuardTargetType = i32;
pub const GUARDTARGET_LOCATION: GuardTargetType = 0;
pub const GUARDTARGET_OBJECT: GuardTargetType = 1;
pub const GUARDTARGET_AREA: GuardTargetType = 2;
pub const GUARDTARGET_NONE: GuardTargetType = 3;

/// `enum AutoAcquireStates` (bits).
pub const AAS_IDLE: u32 = 0x01;
pub const AAS_IDLE_STEALTHED: u32 = 0x02;
pub const AAS_IDLE_NO: u32 = 0x04;
pub const AAS_IDLE_NOT_WHILE_ATTACKING: u32 = 0x08;
pub const AAS_IDLE_ATTACK_BUILDINGS: u32 = 0x10;
/// `TheAutoAcquireEnemiesNames[]` (under `DEFINE_AUTOACQUIRE_NAMES`), without the `NULL`.
pub const AUTO_ACQUIRE_ENEMIES_NAMES: [&str; 5] = ["YES", "STEALTHED", "NO", "NOTWHILEATTACKING", "ATTACK_BUILDINGS"];

/// `enum MoodMatrixParameters` (bits).
pub const MM_CONTROLLER_PLAYER: u32 = 0x0000_0001;
pub const MM_CONTROLLER_AI: u32 = 0x0000_0002;
pub const MM_CONTROLLER_BITMASK: u32 = MM_CONTROLLER_PLAYER | MM_CONTROLLER_AI;
pub const MM_UNITTYPE_NONTURRETED: u32 = 0x0000_0010;
pub const MM_UNITTYPE_TURRETED: u32 = 0x0000_0020;
pub const MM_UNITTYPE_AIR: u32 = 0x0000_0040;
pub const MM_UNITTYPE_BITMASK: u32 = MM_UNITTYPE_NONTURRETED | MM_UNITTYPE_TURRETED | MM_UNITTYPE_AIR;
pub const MM_MOOD_SLEEP: u32 = 0x0000_0100;
pub const MM_MOOD_PASSIVE: u32 = 0x0000_0200;
pub const MM_MOOD_NORMAL: u32 = 0x0000_0400;
pub const MM_MOOD_ALERT: u32 = 0x0000_0800;
pub const MM_MOOD_AGGRESSIVE: u32 = 0x0000_1000;
pub const MM_MOOD_BITMASK: u32 = MM_MOOD_SLEEP | MM_MOOD_PASSIVE | MM_MOOD_NORMAL | MM_MOOD_ALERT | MM_MOOD_AGGRESSIVE;

/// `enum MoodMatrixAction`.
pub type MoodMatrixAction = i32;
pub const MM_ACTION_IDLE: MoodMatrixAction = 0;
pub const MM_ACTION_MOVE: MoodMatrixAction = 1;
pub const MM_ACTION_ATTACK: MoodMatrixAction = 2;
pub const MM_ACTION_ATTACK_MOVE: MoodMatrixAction = 3;

/// `enum MoodActionAdjustment` (bits).
pub const MAA_ACTION_OK: u32 = 0x0000_0001;
pub const MAA_ACTION_TO_IDLE: u32 = 0x0000_0002;
pub const MAA_ACTION_TO_ATTACK_MOVE: u32 = 0x0000_0004;
pub const MAA_ACTION_TO_BITMASK: u32 = MAA_ACTION_OK | MAA_ACTION_TO_IDLE | MAA_ACTION_TO_ATTACK_MOVE;
pub const MAA_AFFECT_RANGE_IGNORE_ALL: u32 = 0x0000_0010;
pub const MAA_AFFECT_RANGE_WAIT_FOR_ATTACK: u32 = 0x0000_0020;
pub const MAA_AFFECT_RANGE_ALERT: u32 = 0x0000_0040;
pub const MAA_AFFECT_RANGE_AGGRESSIVE: u32 = 0x0000_0080;
pub const MAA_AFFECT_RANGE_BITMASK: u32 =
    MAA_AFFECT_RANGE_IGNORE_ALL | MAA_AFFECT_RANGE_WAIT_FOR_ATTACK | MAA_AFFECT_RANGE_ALERT | MAA_AFFECT_RANGE_AGGRESSIVE;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_are_upstreams() {
        assert_eq!(MM_MOOD_BITMASK, 0x1f00);
        assert_eq!(MAA_AFFECT_RANGE_BITMASK, 0xf0);
        assert_eq!(MAA_ACTION_TO_BITMASK, 0x7);
        assert_eq!(LOCOMOTORSET_COUNT, 8);
    }
}
