// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/WellKnownKeys.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Created: Steven Johnson, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WellKnownKeys.h`: the 127 `StaticNameKey`s (`TheKey_teamName`, ...) that
//! name the properties of map objects, teams, players and waypoints in map
//! `Dict`s. Upstream instantiates them (`INSTANTIATE_WELL_KNOWN_KEYS`) in
//! `GameEngineDevice/.../W3DDevice/GameClient/WorldHeightMap.cpp`, a device
//! file; here they are a table.
//!
//! Like every `StaticNameKey`, each gets its key from the generator **the
//! first time it is used**, so its value depends on when that is relative to
//! the other names the generator sees. [`WellKnownKeys`] keeps that lazy
//! behaviour; code that must agree with upstream's key values has to use the
//! keys at the same points upstream does.
//!
//! The constants are indices into [`WELL_KNOWN_KEY_NAMES`], named after
//! upstream's `TheKey_<name>` in upper snake case.

use super::name_key_generator::{NameKeyGenerator, NameKeyType, StaticNameKey};

/// The names, in `DEFINE_KEY` order (the `#NAME` of each).
pub const WELL_KNOWN_KEY_NAMES: [&str; 127] = [
    "teamName",
    "teamOwner",
    "teamIsSingleton",
    "teamHome",
    "teamUnitType1",
    "teamUnitMinCount1",
    "teamUnitMaxCount1",
    "teamUnitType2",
    "teamUnitMinCount2",
    "teamUnitMaxCount2",
    "teamUnitType3",
    "teamUnitMinCount3",
    "teamUnitMaxCount3",
    "teamUnitType4",
    "teamUnitMinCount4",
    "teamUnitMaxCount4",
    "teamUnitType5",
    "teamUnitMinCount5",
    "teamUnitMaxCount5",
    "teamUnitType6",
    "teamUnitMinCount6",
    "teamUnitMaxCount6",
    "teamUnitType7",
    "teamUnitMinCount7",
    "teamUnitMaxCount7",
    "teamOnCreateScript",
    "teamOnIdleScript",
    "teamInitialIdleFrames",
    "teamOnUnitDestroyedScript",
    "teamOnDestroyedScript",
    "teamDestroyedThreshold",
    "teamEnemySightedScript",
    "teamAllClearScript",
    "teamAutoReinforce",
    "teamIsAIRecruitable",
    "teamIsBaseDefense",
    "teamIsPerimeterDefense",
    "teamAggressiveness",
    "teamTransportsReturn",
    "teamAvoidThreats",
    "teamAttackCommonTarget",
    "teamMaxInstances",
    "teamDescription",
    "teamProductionCondition",
    "teamProductionPriority",
    "teamProductionPrioritySuccessIncrease",
    "teamProductionPriorityFailureDecrease",
    "teamTransport",
    "teamReinforcementOrigin",
    "teamStartsFull",
    "teamTransportsExit",
    "teamVeterancy",
    "teamExecutesActionsOnCreate",
    "teamGenericScriptHook",
    "originalOwner",
    "lightHeightAboveTerrain",
    "lightOuterRadius",
    "lightInnerRadius",
    "lightAmbientColor",
    "lightDiffuseColor",
    "waypointName",
    "waypointID",
    "waypointPathLabel1",
    "waypointPathLabel2",
    "waypointPathLabel3",
    "waypointPathBiDirectional",
    "objectName",
    "exportWithScript",
    "objectInitialHealth",
    "objectMaxHPs",
    "objectEnabled",
    "objectIndestructible",
    "objectUnsellable",
    "objectTargetable",
    "objectPowered",
    "objectScriptAttachment",
    "objectAggressiveness",
    "objectVisualRange",
    "objectShroudClearingDistance",
    "objectGroupNumber",
    "objectRecruitableAI",
    "objectSelectable",
    "objectVeterancy",
    "objectTime",
    "objectWeather",
    "objectRadius",
    "scorchType",
    "objectStoppingDistance",
    "objectLayer",
    "objectGrantUpgrade",
    "uniqueID",
    "objectSoundAmbient",
    "objectSoundAmbientCustomized",
    "objectSoundAmbientEnabled",
    "objectSoundAmbientLooping",
    "objectSoundAmbientLoopCount",
    "objectSoundAmbientMinVolume",
    "objectSoundAmbientVolume",
    "objectSoundAmbientMinRange",
    "objectSoundAmbientMaxRange",
    "objectSoundAmbientPriority",
    "playerName",
    "playerIsHuman",
    "playerIsSkirmish",
    "playerDisplayName",
    "playerFaction",
    "playerEnemies",
    "playerAllies",
    "playerStartMoney",
    "playerColor",
    "playerNightColor",
    "multiplayerStartIndex",
    "skirmishDifficulty",
    "multiplayerIsLocal",
    "playerIsPreorder",
    "weather",
    "mapName",
    "compression",
    "InitialCameraPosition",
    "Player_1_Start",
    "Player_2_Start",
    "Player_3_Start",
    "Player_4_Start",
    "Player_5_Start",
    "Player_6_Start",
    "Player_7_Start",
    "Player_8_Start",
];

/// `TheKey_teamName`.
pub const TEAM_NAME: usize = 0;
/// `TheKey_teamOwner`.
pub const TEAM_OWNER: usize = 1;
/// `TheKey_teamIsSingleton`.
pub const TEAM_IS_SINGLETON: usize = 2;
/// `TheKey_teamHome`.
pub const TEAM_HOME: usize = 3;
/// `TheKey_teamUnitType1`.
pub const TEAM_UNIT_TYPE1: usize = 4;
/// `TheKey_teamUnitMinCount1`.
pub const TEAM_UNIT_MIN_COUNT1: usize = 5;
/// `TheKey_teamUnitMaxCount1`.
pub const TEAM_UNIT_MAX_COUNT1: usize = 6;
/// `TheKey_teamUnitType2`.
pub const TEAM_UNIT_TYPE2: usize = 7;
/// `TheKey_teamUnitMinCount2`.
pub const TEAM_UNIT_MIN_COUNT2: usize = 8;
/// `TheKey_teamUnitMaxCount2`.
pub const TEAM_UNIT_MAX_COUNT2: usize = 9;
/// `TheKey_teamUnitType3`.
pub const TEAM_UNIT_TYPE3: usize = 10;
/// `TheKey_teamUnitMinCount3`.
pub const TEAM_UNIT_MIN_COUNT3: usize = 11;
/// `TheKey_teamUnitMaxCount3`.
pub const TEAM_UNIT_MAX_COUNT3: usize = 12;
/// `TheKey_teamUnitType4`.
pub const TEAM_UNIT_TYPE4: usize = 13;
/// `TheKey_teamUnitMinCount4`.
pub const TEAM_UNIT_MIN_COUNT4: usize = 14;
/// `TheKey_teamUnitMaxCount4`.
pub const TEAM_UNIT_MAX_COUNT4: usize = 15;
/// `TheKey_teamUnitType5`.
pub const TEAM_UNIT_TYPE5: usize = 16;
/// `TheKey_teamUnitMinCount5`.
pub const TEAM_UNIT_MIN_COUNT5: usize = 17;
/// `TheKey_teamUnitMaxCount5`.
pub const TEAM_UNIT_MAX_COUNT5: usize = 18;
/// `TheKey_teamUnitType6`.
pub const TEAM_UNIT_TYPE6: usize = 19;
/// `TheKey_teamUnitMinCount6`.
pub const TEAM_UNIT_MIN_COUNT6: usize = 20;
/// `TheKey_teamUnitMaxCount6`.
pub const TEAM_UNIT_MAX_COUNT6: usize = 21;
/// `TheKey_teamUnitType7`.
pub const TEAM_UNIT_TYPE7: usize = 22;
/// `TheKey_teamUnitMinCount7`.
pub const TEAM_UNIT_MIN_COUNT7: usize = 23;
/// `TheKey_teamUnitMaxCount7`.
pub const TEAM_UNIT_MAX_COUNT7: usize = 24;
/// `TheKey_teamOnCreateScript`.
pub const TEAM_ON_CREATE_SCRIPT: usize = 25;
/// `TheKey_teamOnIdleScript`.
pub const TEAM_ON_IDLE_SCRIPT: usize = 26;
/// `TheKey_teamInitialIdleFrames`.
pub const TEAM_INITIAL_IDLE_FRAMES: usize = 27;
/// `TheKey_teamOnUnitDestroyedScript`.
pub const TEAM_ON_UNIT_DESTROYED_SCRIPT: usize = 28;
/// `TheKey_teamOnDestroyedScript`.
pub const TEAM_ON_DESTROYED_SCRIPT: usize = 29;
/// `TheKey_teamDestroyedThreshold`.
pub const TEAM_DESTROYED_THRESHOLD: usize = 30;
/// `TheKey_teamEnemySightedScript`.
pub const TEAM_ENEMY_SIGHTED_SCRIPT: usize = 31;
/// `TheKey_teamAllClearScript`.
pub const TEAM_ALL_CLEAR_SCRIPT: usize = 32;
/// `TheKey_teamAutoReinforce`.
pub const TEAM_AUTO_REINFORCE: usize = 33;
/// `TheKey_teamIsAIRecruitable`.
pub const TEAM_IS_AIRECRUITABLE: usize = 34;
/// `TheKey_teamIsBaseDefense`.
pub const TEAM_IS_BASE_DEFENSE: usize = 35;
/// `TheKey_teamIsPerimeterDefense`.
pub const TEAM_IS_PERIMETER_DEFENSE: usize = 36;
/// `TheKey_teamAggressiveness`.
pub const TEAM_AGGRESSIVENESS: usize = 37;
/// `TheKey_teamTransportsReturn`.
pub const TEAM_TRANSPORTS_RETURN: usize = 38;
/// `TheKey_teamAvoidThreats`.
pub const TEAM_AVOID_THREATS: usize = 39;
/// `TheKey_teamAttackCommonTarget`.
pub const TEAM_ATTACK_COMMON_TARGET: usize = 40;
/// `TheKey_teamMaxInstances`.
pub const TEAM_MAX_INSTANCES: usize = 41;
/// `TheKey_teamDescription`.
pub const TEAM_DESCRIPTION: usize = 42;
/// `TheKey_teamProductionCondition`.
pub const TEAM_PRODUCTION_CONDITION: usize = 43;
/// `TheKey_teamProductionPriority`.
pub const TEAM_PRODUCTION_PRIORITY: usize = 44;
/// `TheKey_teamProductionPrioritySuccessIncrease`.
pub const TEAM_PRODUCTION_PRIORITY_SUCCESS_INCREASE: usize = 45;
/// `TheKey_teamProductionPriorityFailureDecrease`.
pub const TEAM_PRODUCTION_PRIORITY_FAILURE_DECREASE: usize = 46;
/// `TheKey_teamTransport`.
pub const TEAM_TRANSPORT: usize = 47;
/// `TheKey_teamReinforcementOrigin`.
pub const TEAM_REINFORCEMENT_ORIGIN: usize = 48;
/// `TheKey_teamStartsFull`.
pub const TEAM_STARTS_FULL: usize = 49;
/// `TheKey_teamTransportsExit`.
pub const TEAM_TRANSPORTS_EXIT: usize = 50;
/// `TheKey_teamVeterancy`.
pub const TEAM_VETERANCY: usize = 51;
/// `TheKey_teamExecutesActionsOnCreate`.
pub const TEAM_EXECUTES_ACTIONS_ON_CREATE: usize = 52;
/// `TheKey_teamGenericScriptHook`.
pub const TEAM_GENERIC_SCRIPT_HOOK: usize = 53;
/// `TheKey_originalOwner`.
pub const ORIGINAL_OWNER: usize = 54;
/// `TheKey_lightHeightAboveTerrain`.
pub const LIGHT_HEIGHT_ABOVE_TERRAIN: usize = 55;
/// `TheKey_lightOuterRadius`.
pub const LIGHT_OUTER_RADIUS: usize = 56;
/// `TheKey_lightInnerRadius`.
pub const LIGHT_INNER_RADIUS: usize = 57;
/// `TheKey_lightAmbientColor`.
pub const LIGHT_AMBIENT_COLOR: usize = 58;
/// `TheKey_lightDiffuseColor`.
pub const LIGHT_DIFFUSE_COLOR: usize = 59;
/// `TheKey_waypointName`.
pub const WAYPOINT_NAME: usize = 60;
/// `TheKey_waypointID`.
pub const WAYPOINT_ID: usize = 61;
/// `TheKey_waypointPathLabel1`.
pub const WAYPOINT_PATH_LABEL1: usize = 62;
/// `TheKey_waypointPathLabel2`.
pub const WAYPOINT_PATH_LABEL2: usize = 63;
/// `TheKey_waypointPathLabel3`.
pub const WAYPOINT_PATH_LABEL3: usize = 64;
/// `TheKey_waypointPathBiDirectional`.
pub const WAYPOINT_PATH_BI_DIRECTIONAL: usize = 65;
/// `TheKey_objectName`.
pub const OBJECT_NAME: usize = 66;
/// `TheKey_exportWithScript`.
pub const EXPORT_WITH_SCRIPT: usize = 67;
/// `TheKey_objectInitialHealth`.
pub const OBJECT_INITIAL_HEALTH: usize = 68;
/// `TheKey_objectMaxHPs`.
pub const OBJECT_MAX_HPS: usize = 69;
/// `TheKey_objectEnabled`.
pub const OBJECT_ENABLED: usize = 70;
/// `TheKey_objectIndestructible`.
pub const OBJECT_INDESTRUCTIBLE: usize = 71;
/// `TheKey_objectUnsellable`.
pub const OBJECT_UNSELLABLE: usize = 72;
/// `TheKey_objectTargetable`.
pub const OBJECT_TARGETABLE: usize = 73;
/// `TheKey_objectPowered`.
pub const OBJECT_POWERED: usize = 74;
/// `TheKey_objectScriptAttachment`.
pub const OBJECT_SCRIPT_ATTACHMENT: usize = 75;
/// `TheKey_objectAggressiveness`.
pub const OBJECT_AGGRESSIVENESS: usize = 76;
/// `TheKey_objectVisualRange`.
pub const OBJECT_VISUAL_RANGE: usize = 77;
/// `TheKey_objectShroudClearingDistance`.
pub const OBJECT_SHROUD_CLEARING_DISTANCE: usize = 78;
/// `TheKey_objectGroupNumber`.
pub const OBJECT_GROUP_NUMBER: usize = 79;
/// `TheKey_objectRecruitableAI`.
pub const OBJECT_RECRUITABLE_AI: usize = 80;
/// `TheKey_objectSelectable`.
pub const OBJECT_SELECTABLE: usize = 81;
/// `TheKey_objectVeterancy`.
pub const OBJECT_VETERANCY: usize = 82;
/// `TheKey_objectTime`.
pub const OBJECT_TIME: usize = 83;
/// `TheKey_objectWeather`.
pub const OBJECT_WEATHER: usize = 84;
/// `TheKey_objectRadius`.
pub const OBJECT_RADIUS: usize = 85;
/// `TheKey_scorchType`.
pub const SCORCH_TYPE: usize = 86;
/// `TheKey_objectStoppingDistance`.
pub const OBJECT_STOPPING_DISTANCE: usize = 87;
/// `TheKey_objectLayer`.
pub const OBJECT_LAYER: usize = 88;
/// `TheKey_objectGrantUpgrade`.
pub const OBJECT_GRANT_UPGRADE: usize = 89;
/// `TheKey_uniqueID`.
pub const UNIQUE_ID: usize = 90;
/// `TheKey_objectSoundAmbient`.
pub const OBJECT_SOUND_AMBIENT: usize = 91;
/// `TheKey_objectSoundAmbientCustomized`.
pub const OBJECT_SOUND_AMBIENT_CUSTOMIZED: usize = 92;
/// `TheKey_objectSoundAmbientEnabled`.
pub const OBJECT_SOUND_AMBIENT_ENABLED: usize = 93;
/// `TheKey_objectSoundAmbientLooping`.
pub const OBJECT_SOUND_AMBIENT_LOOPING: usize = 94;
/// `TheKey_objectSoundAmbientLoopCount`.
pub const OBJECT_SOUND_AMBIENT_LOOP_COUNT: usize = 95;
/// `TheKey_objectSoundAmbientMinVolume`.
pub const OBJECT_SOUND_AMBIENT_MIN_VOLUME: usize = 96;
/// `TheKey_objectSoundAmbientVolume`.
pub const OBJECT_SOUND_AMBIENT_VOLUME: usize = 97;
/// `TheKey_objectSoundAmbientMinRange`.
pub const OBJECT_SOUND_AMBIENT_MIN_RANGE: usize = 98;
/// `TheKey_objectSoundAmbientMaxRange`.
pub const OBJECT_SOUND_AMBIENT_MAX_RANGE: usize = 99;
/// `TheKey_objectSoundAmbientPriority`.
pub const OBJECT_SOUND_AMBIENT_PRIORITY: usize = 100;
/// `TheKey_playerName`.
pub const PLAYER_NAME: usize = 101;
/// `TheKey_playerIsHuman`.
pub const PLAYER_IS_HUMAN: usize = 102;
/// `TheKey_playerIsSkirmish`.
pub const PLAYER_IS_SKIRMISH: usize = 103;
/// `TheKey_playerDisplayName`.
pub const PLAYER_DISPLAY_NAME: usize = 104;
/// `TheKey_playerFaction`.
pub const PLAYER_FACTION: usize = 105;
/// `TheKey_playerEnemies`.
pub const PLAYER_ENEMIES: usize = 106;
/// `TheKey_playerAllies`.
pub const PLAYER_ALLIES: usize = 107;
/// `TheKey_playerStartMoney`.
pub const PLAYER_START_MONEY: usize = 108;
/// `TheKey_playerColor`.
pub const PLAYER_COLOR: usize = 109;
/// `TheKey_playerNightColor`.
pub const PLAYER_NIGHT_COLOR: usize = 110;
/// `TheKey_multiplayerStartIndex`.
pub const MULTIPLAYER_START_INDEX: usize = 111;
/// `TheKey_skirmishDifficulty`.
pub const SKIRMISH_DIFFICULTY: usize = 112;
/// `TheKey_multiplayerIsLocal`.
pub const MULTIPLAYER_IS_LOCAL: usize = 113;
/// `TheKey_playerIsPreorder`.
pub const PLAYER_IS_PREORDER: usize = 114;
/// `TheKey_weather`.
pub const WEATHER: usize = 115;
/// `TheKey_mapName`.
pub const MAP_NAME: usize = 116;
/// `TheKey_compression`.
pub const COMPRESSION: usize = 117;
/// `TheKey_InitialCameraPosition`.
pub const INITIAL_CAMERA_POSITION: usize = 118;
/// `TheKey_Player_1_Start`.
pub const PLAYER_1_START: usize = 119;
/// `TheKey_Player_2_Start`.
pub const PLAYER_2_START: usize = 120;
/// `TheKey_Player_3_Start`.
pub const PLAYER_3_START: usize = 121;
/// `TheKey_Player_4_Start`.
pub const PLAYER_4_START: usize = 122;
/// `TheKey_Player_5_Start`.
pub const PLAYER_5_START: usize = 123;
/// `TheKey_Player_6_Start`.
pub const PLAYER_6_START: usize = 124;
/// `TheKey_Player_7_Start`.
pub const PLAYER_7_START: usize = 125;
/// `TheKey_Player_8_Start`.
pub const PLAYER_8_START: usize = 126;

/// The keys, each looked up on first use (`StaticNameKey::key`).
#[derive(Clone, Debug)]
pub struct WellKnownKeys {
    keys: Vec<StaticNameKey>,
}

impl Default for WellKnownKeys {
    fn default() -> Self {
        Self::new()
    }
}

impl WellKnownKeys {
    /// All 127, none looked up yet.
    pub fn new() -> Self {
        WellKnownKeys { keys: WELL_KNOWN_KEY_NAMES.iter().map(|n| StaticNameKey::new(n.as_bytes())).collect() }
    }

    /// `TheKey_<name>.key()`: `which` is one of the constants of this module.
    pub fn key(&self, which: usize, gen: &mut NameKeyGenerator) -> NameKeyType {
        self.keys[which].key(gen)
    }
}
