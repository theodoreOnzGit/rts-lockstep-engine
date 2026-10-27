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
//! The enums and constants of `AIUpdate.h` that the AI's parts share (`p1w13-sonnet`, layer 10, batch T1), with
//! `CommandSourceType` and `GuardMode` of `GameCommon.h` and `AttitudeType` of `AI.h` (batch A2), `AIStateType` of `AIStateMachine.h`
//! and the qualifiers and vision factors of `AI.h` (batch A3). Values as upstream's:
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

/// `enum CommandSourceType` (GameCommon.h): "the source of a command".
pub type CommandSourceType = i32;
pub const CMD_FROM_PLAYER: CommandSourceType = 0;
pub const CMD_FROM_SCRIPT: CommandSourceType = 1;
pub const CMD_FROM_AI: CommandSourceType = 2;
/// "Special rare command when the dozer originates a command to attack a mine."
pub const CMD_FROM_DOZER: CommandSourceType = 3;
/// "Special case: A weapon that can be chosen -- this is the default case (machine gun vs flashbang)."
pub const CMD_DEFAULT_SWITCH_WEAPON: CommandSourceType = 4;

/// `enum GuardMode` (GameCommon.h).
pub type GuardMode = i32;
pub const GUARDMODE_NORMAL: GuardMode = 0;
/// "no pursuit out of guard area"
pub const GUARDMODE_GUARD_WITHOUT_PURSUIT: GuardMode = 1;
/// "ignore nonflyers"
pub const GUARDMODE_GUARD_FLYING_UNITS_ONLY: GuardMode = 2;

/// `enum AttitudeType` (AI.h): "AI "attitude" behavior modifiers".
pub type AttitudeType = i32;
pub const AI_SLEEP: AttitudeType = -2;
pub const AI_PASSIVE: AttitudeType = -1;
pub const AI_NORMAL: AttitudeType = 0;
pub const AI_ALERT: AttitudeType = 1;
pub const AI_AGGRESSIVE: AttitudeType = 2;
pub const AI_INVALID: AttitudeType = 3;

/// `AI_VISIONFACTOR_*` (AI.h).
pub const AI_VISIONFACTOR_OWNERTYPE: i32 = 0x01;
pub const AI_VISIONFACTOR_MOOD: i32 = 0x02;
pub const AI_VISIONFACTOR_GUARDINNER: i32 = 0x04;

/// `AI::CAN_SEE` .. `AI::UNFOGGED`: `findClosestEnemy`'s qualifiers.
pub const AI_CAN_SEE: u32 = 1 << 0;
pub const AI_CAN_ATTACK: u32 = 1 << 1;
pub const AI_IGNORE_INSIGNIFICANT_BUILDINGS: u32 = 1 << 2;
pub const AI_ATTACK_BUILDINGS: u32 = 1 << 3;
pub const AI_WITHIN_ATTACK_RANGE: u32 = 1 << 4;
pub const AI_UNFOGGED: u32 = 1 << 5;

/// `enum CanAttackResult` (WeaponSet.h): "Worst scenario to best scenario".
pub type CanAttackResultValue = i32;
pub const ATTACKRESULT_NOT_POSSIBLE: CanAttackResultValue = 0;
pub const ATTACKRESULT_INVALID_SHOT: CanAttackResultValue = 1;
pub const ATTACKRESULT_POSSIBLE_AFTER_MOVING: CanAttackResultValue = 2;
pub const ATTACKRESULT_POSSIBLE: CanAttackResultValue = 3;

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

/// `enum AIStateType` (AIStateMachine.h): the AI state machine's state ids, in upstream's order.
pub type AiStateType = u32;
pub const AI_IDLE: AiStateType = 0;
pub const AI_MOVE_TO: AiStateType = 1;
pub const AI_FOLLOW_WAYPOINT_PATH_AS_TEAM: AiStateType = 2;
pub const AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS: AiStateType = 3;
pub const AI_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT: AiStateType = 4;
pub const AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS_EXACT: AiStateType = 5;
pub const AI_FOLLOW_PATH: AiStateType = 6;
pub const AI_FOLLOW_EXITPRODUCTION_PATH: AiStateType = 7;
pub const AI_WAIT: AiStateType = 8;
pub const AI_ATTACK_POSITION: AiStateType = 9;
pub const AI_ATTACK_OBJECT: AiStateType = 10;
pub const AI_FORCE_ATTACK_OBJECT: AiStateType = 11;
pub const AI_ATTACK_AND_FOLLOW_OBJECT: AiStateType = 12;
pub const AI_DEAD: AiStateType = 13;
pub const AI_DOCK: AiStateType = 14;
pub const AI_ENTER: AiStateType = 15;
pub const AI_GUARD: AiStateType = 16;
pub const AI_HUNT: AiStateType = 17;
pub const AI_WANDER: AiStateType = 18;
pub const AI_PANIC: AiStateType = 19;
pub const AI_ATTACK_SQUAD: AiStateType = 20;
pub const AI_GUARD_TUNNEL_NETWORK: AiStateType = 21;
pub const AI_GET_REPAIRED: AiStateType = 22;
pub const AI_MOVE_OUT_OF_THE_WAY: AiStateType = 23;
pub const AI_MOVE_AND_TIGHTEN: AiStateType = 24;
pub const AI_MOVE_AND_EVACUATE: AiStateType = 25;
pub const AI_MOVE_AND_EVACUATE_AND_EXIT: AiStateType = 26;
pub const AI_MOVE_AND_DELETE: AiStateType = 27;
pub const AI_ATTACK_AREA: AiStateType = 28;
pub const AI_HACK_INTERNET: AiStateType = 29;
pub const AI_ATTACK_MOVE_TO: AiStateType = 30;
pub const AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS: AiStateType = 31;
pub const AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM: AiStateType = 32;
pub const AI_FACE_OBJECT: AiStateType = 33;
pub const AI_FACE_POSITION: AiStateType = 34;
pub const AI_RAPPEL_INTO: AiStateType = 35;
pub const AI_COMBATDROP: AiStateType = 36;
pub const AI_EXIT: AiStateType = 37;
pub const AI_PICK_UP_CRATE: AiStateType = 38;
pub const AI_MOVE_AWAY_FROM_REPULSORS: AiStateType = 39;
pub const AI_WANDER_IN_PLACE: AiStateType = 40;
pub const AI_BUSY: AiStateType = 41;
pub const AI_EXIT_INSTANTLY: AiStateType = 42;
pub const AI_GUARD_RETALIATE: AiStateType = 43;
pub const NUM_AI_STATES: AiStateType = 44;

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
