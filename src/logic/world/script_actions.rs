// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ScriptActions.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptActions.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: John Ahlquist, Nov. 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ScriptActions` (`TheScriptActions`): the execution of a map script's actions. `executeAction` dispatches on the action's type to
//! one of the `do*` actions, which order teams, units and players about, drive the camera, play sounds, create objects and teams, and
//! end the game. The world is reached through [`ScriptActionsHost`]: one method per upstream call (the script engine's lookups, the
//! terrain logic's waypoints, the view, the AI and its groups, the factories, players, teams, objects, the client and the audio); the
//! order and the conditions of the calls, and the values computed for them, are this file's.
//!
//! **Translated: all of `executeAction`**, in ten batches of upstream lines with their cases: A1, 96-1225 (victory and defeat, the
//! object panel flags, sounds, team and unit moves and states, reinforcements, the camera, object creation, attacks, the AI players'
//! build orders); A2, 1225-2300 (attack priorities, attitudes, repulsors, area and team attacks, transports, waypoint and skirmish
//! approach paths, guarding, hunting, construction and factories); A3, 2300-2870 (teams built and recruited, damage, deletion, wander
//! and panic, priorities, kills, texts and popups, the cinematic text, flashes, movies, sounds from units, speech and subtitles, asset
//! transfers, scoring, relations, radar events); A4, 2870-3475 (object and team radar events, radar, motion blur, map reveal and
//! shroud, recruitment, team merges, input, the border shroud, idling, ambient sounds, music, the teams' garrisons); A5, 3475-4165
//! (stealth, unmanned and booby-trapped units, the units' garrisons and exits, letterbox, black and white, skybox, weather, freezing
//! time, captions, audible distance, stopping distances, held units, the special power display, volumes, team transfers, money, timers
//! and counters, special power countdowns, firing at a waypoint); A6, 4165-4905 (the skirmish superweapon, firing at a unit, the radar
//! refresh, stops and disbands, the camera's tether and default, override relations, waypoint-following weapons, command button
//! abilities, sequential scripts, guarding and idling for a frame count); A7, 4905-5160 (water heights, border switches, forced
//! selection, destroying the contained, the radar, screen shakes, buildability, cave indexes, warehouse values, sound types and
//! volume overrides, topple directions); A8, 5160-5440 (moves to the nearest object, upgrades, the skirmish value attacks and command
//! buttons); A9, 5436-5900 (spinning, command buttons on the nearest targets, partial use, captures and teams from captured units);
//! A10, 5895-6405 and the rest of `executeAction` (camera modifiers, visual speed, background sounds, motion blur following, the FPS
//! limit, skill points, ranks and sciences, emoticons, object lists, permanent reveals, EVA, options, the camera's slave mode and
//! shakers, hulk lifetimes, facing, panel flags, command bar buttons, experience, supply-centre and tunnel guarding, difficulty
//! bonuses, the guardband, the unmanned deleted, victims, held trains). A type `executeAction` does not list does nothing, as
//! upstream's `default`. [`ActionError::NotTranslated`] is internal to the dispatch.
//!
//! # Upstream facts kept
//!
//! - `doVictory`, `doDefeat`, `doPlaySoundEffect` and `doPlaySoundEffectAt` read the local player unchecked
//!   ([`Undefined::NullLocalPlayer`] without one).
//! - The camera's times are `sec*1000` (a `Real`) passed to `Int` parameters: truncated, and undefined when out of range
//!   ([`Undefined::FloatToIntOutOfRange`]); the eases stay `Real`.
//! - `doCreateReinforcements`: the transport is positioned before it is checked; a transport that cannot contain the unit is never
//!   checked again; when a full transport is replaced, the new one's contain (or NULL) is used unchecked
//!   ([`Undefined::NullContainModule`]); the units of the team are placed along x, `2.25*j*radius` apart (`2.25` a `double`), and the
//!   next unit type starts `2*radius` (of the last made unit) along y.
//! - `doCreateObject` and `createUnitOnTeamAt` refuse a name that a living unit has, and give an existing (dead) unit's name over with
//!   `transferObjectName`.
//! - `updateTeamAttackPrioritySet` names the team's set only when it is not the default one (`getAttackInfo` never fails: an unknown
//!   name gives the default set, whose name is empty).
//! - `doLoadAllTransports` gives a unit with 0 transport slots 0 (the reinforcements give it "lots", `0x7fffff`).
//! - `doTeamEnterNamed` and `doTeamExitAll` use their AI group unchecked.
//! - `doTeamHuntWithCommandButton` reads the template's name of a member that lacks the button unchecked.
//! - `doTeamWander` and `doTeamPanic` stop at the first member with no waypoint on the path (`return`); `doTeamFlash` stops at the first
//!   member with no drawable (`break`).
//! - `doCameoFlash` uses its button unchecked; the flash counts are `30*seconds` (an `Int`: overflow is undefined) over 15.
//! - `doDisplayCinematicText` parses the font as upstream does: the name is the concatenation of the *rest of the buffer* after each
//!   character before the first space or dash; past the end of the string, or with no `:` after the name, it reads the uninitialised
//!   rest of its `char buf[256]` (and a font string of 256 characters or more overflows it); the size is the text after the `:` up to
//!   the next space (so `": 12"` is size 0).
//! - `getHumanPlayerMask` reads an uninitialised mask: `doRevealMapAtWaypoint` and `doShroudMapAtWaypoint` without a named player are
//!   undefined.
//! - `doMergeTeamIntoTeam` moves its first member twice.
//! - `doTeamGarrisonSpecificBuilding` tests `!(structure && nobody entered || the team's player entered)` (`&&` first), reading the
//!   team's player unchecked when the first half is false.
//! - `doTeamGarrisonNearestBuilding` walks the team once across all the buildings, nearest first; the free slots are
//!   `UnsignedInt` arithmetic read as an `Int`.
//! - `doSkirmishFireSpecialPowerAtMostCost`: in each team, members whose module is not ready are skipped and the first ready one fires
//!   (or not, when no target is found), ending that team's loop.
//! - `TEAM_GUARD_FOR_FRAMECOUNT` executes `doTeamIdleForFramecount` (`doTeamGuardForFramecount` is never called).
//! - `doSkirmishAttackNearestGroupWithValue` leaves its position uninitialised unless the comparison is `>=` or `>` and a group is
//!   found ([`Undefined::UninitialisedLocal`]); `doMoveTeamTowardsNearest` stops at the first member without an AI (`return`).
//! - `doForceObjectSelection` picks the member of the type with the lowest id and plays its sound for the local player unchecked.
//! - `CAMERA_ADD_SHAKER_AT` uses its waypoint unchecked ([`Undefined::NullLocation`]); the emoticon and hulk-lifetime durations are
//!   `(Int)(seconds*LOGICFRAMES_PER_SECOND)` (a negative hulk lifetime is `-1`, "no override").
//! - A kind-of bit outside the kinds (the command buttons on the nearest kind or building class) makes `std::bitset::set` throw; it is
//!   reported as [`Undefined::ArrayIndexOutOfRange`].
//!
//! Differential-verified in `tests/differential_script_actions.rs` (the file with two mechanical edits, on stand-ins): all 344 action
//! types, 40,000 programs, 719,958 operations, 0 mismatches; 29 planted bugs (3861-3889), all caught.
//!
//! `p1w13-sonnet`, layer 11, batches A1-A10, 2026-10-30.

use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::disabled_types::{DisabledType, DISABLED_HELD, DISABLED_UNMANNED};
use crate::common::game_type::ObjectId;
use crate::common::ini::{IniError, Undefined};
use crate::common::kind_of::{
    KindOfType, KINDOF_BLAST_CRATER, KINDOF_INFANTRY, KINDOF_MONEY_HACKER, KINDOF_NO_GARRISON, KINDOF_STRUCTURE, KINDOF_TRANSPORT,
};
use crate::common::partition_solver::{PartitionSolver, SolutionType};
use crate::logic::modules::plant;
use crate::logic::scripts::{action_type as at, Parameter, ScriptAction};

/// A player: its index.
pub type PlayerH = i32;
/// A team, as the host names it.
pub type TeamH = i32;
/// An object: its id.
pub type ObjH = ObjectId;
/// A waypoint, as the host names it.
pub type WaypointH = i32;
/// A thing template, as the host names it.
pub type TemplateH = i32;
/// A team prototype, as the host names it.
pub type ProtoH = i32;
/// An AI group, as the host names it.
pub type GroupH = i32;
/// A window, as the host names it.
pub type WindowH = i32;
/// A trigger area, as the host names it.
pub type TriggerH = i32;
/// A command button, as the host names it.
pub type ButtonH = i32;
/// A command set, as the host names it.
pub type CommandSetH = i32;
/// A special power template, as the host names it.
pub type SpecialPowerH = i32;

/// `CMD_FROM_SCRIPT`.
pub const CMD_FROM_SCRIPT: i32 = 1;
/// `LOCOMOTORSET_NORMAL`.
pub const LOCOMOTORSET_NORMAL: i32 = 0;
/// `NO_MAX_SHOTS_LIMIT`.
pub const NO_MAX_SHOTS_LIMIT: i32 = 0x7fff_ffff;
/// `View::LOCK_FOLLOW`.
pub const LOCK_FOLLOW: i32 = 0;
/// `View::LOCK_TETHER`.
pub const LOCK_TETHER: i32 = 1;
/// `SPECIAL_SNEAK_ATTACK`.
pub const SPECIAL_SNEAK_ATTACK: i32 = 45;
/// `PLAYERMASK_ALL`.
pub const PLAYERMASK_ALL: u32 = 0xffff;
/// `FT_VIEW_BW_FILTER`.
pub const FT_VIEW_BW_FILTER: i32 = 1;
/// `FM_VIEW_BW_BLACK_AND_WHITE`.
pub const FM_VIEW_BW_BLACK_AND_WHITE: i32 = 1;
/// `FOREVER`.
pub const FOREVER: u32 = 0x3fff_ffff;
/// `NEVER`.
pub const NEVER: u32 = 0;
/// `AudioAffect_Music`.
pub const AUDIO_AFFECT_MUSIC: i32 = 1;
/// `AudioAffect_Sound`.
pub const AUDIO_AFFECT_SOUND: i32 = 2;
/// `AudioAffect_Sound3D`.
pub const AUDIO_AFFECT_SOUND_3D: i32 = 4;
/// `AudioAffect_Speech`.
pub const AUDIO_AFFECT_SPEECH: i32 = 8;
/// `INVALID_ANGLE`.
pub const INVALID_ANGLE: f32 = -100.0;
/// `Parameter::GREATER_EQUAL`, `GREATER` (the comparisons `doSkirmishAttackNearestGroupWithValue` reads).
pub mod comparison {
    /// `GREATER_EQUAL`.
    pub const GREATER_EQUAL: i32 = 3;
    /// `GREATER`.
    pub const GREATER: i32 = 4;
}
/// `COMMAND_FIRED_BY_SCRIPT`.
pub const COMMAND_FIRED_BY_SCRIPT: u32 = 0x0004_0000;
/// `FM_VIEW_MB_END_PAN_ALPHA`.
pub const FM_VIEW_MB_END_PAN_ALPHA: i32 = 12;
/// `FM_VIEW_MB_PAN_ALPHA`.
pub const FM_VIEW_MB_PAN_ALPHA: i32 = 14;
/// `FT_NULL_FILTER`.
pub const FT_NULL_FILTER: i32 = 0;
/// `FT_VIEW_MOTION_BLUR_FILTER`.
pub const FT_VIEW_MOTION_BLUR_FILTER: i32 = 2;
/// The motion blur `FilterModes`.
pub mod filter_mode {
    /// `FM_VIEW_MB_IN_AND_OUT_ALPHA`.
    pub const IN_AND_OUT_ALPHA: i32 = 6;
    /// `FM_VIEW_MB_IN_AND_OUT_SATURATE`.
    pub const IN_AND_OUT_SATURATE: i32 = 7;
    /// `FM_VIEW_MB_IN_ALPHA`.
    pub const IN_ALPHA: i32 = 8;
    /// `FM_VIEW_MB_OUT_ALPHA`.
    pub const OUT_ALPHA: i32 = 9;
    /// `FM_VIEW_MB_IN_SATURATE`.
    pub const IN_SATURATE: i32 = 10;
    /// `FM_VIEW_MB_OUT_SATURATE`.
    pub const OUT_SATURATE: i32 = 11;
}
/// `AHSV_StopTheMusic`.
pub const AHSV_STOP_THE_MUSIC: i32 = 4;
/// `AHSV_StopTheMusicFade`.
pub const AHSV_STOP_THE_MUSIC_FADE: i32 = 5;
/// `LOCOMOTORSET_WANDER`.
pub const LOCOMOTORSET_WANDER: i32 = 3;
/// `LOCOMOTORSET_PANIC`.
pub const LOCOMOTORSET_PANIC: i32 = 4;
/// `DAMAGE_UNRESISTABLE`.
pub const DAMAGE_UNRESISTABLE: i32 = 11;
/// `DEATH_NORMAL`.
pub const DEATH_NORMAL: i32 = 0;
/// `LOGICFRAMES_PER_SECOND`.
pub const LOGICFRAMES_PER_SECOND: i32 = 30;
/// `DRAWABLE_FRAMES_PER_FLASH`.
pub const DRAWABLE_FRAMES_PER_FLASH: i32 = LOGICFRAMES_PER_SECOND / 2;
/// `SUBTITLE_DURATION` (milliseconds).
pub const SUBTITLE_DURATION: i32 = 8000;
/// `GUARDMODE_NORMAL`.
pub const GUARDMODE_NORMAL: i32 = 0;
/// `OBJECT_STATUS_REPULSOR`.
pub const OBJECT_STATUS_REPULSOR: i32 = 9;
/// `MAX_COMMANDS_PER_SET`.
pub const MAX_COMMANDS_PER_SET: i32 = 18;
/// `COMMAND_OPTION_NEED_OBJECT_TARGET` (`NEED_TARGET_ENEMY_OBJECT | NEED_TARGET_NEUTRAL_OBJECT | NEED_TARGET_ALLY_OBJECT`).
pub const COMMAND_OPTION_NEED_OBJECT_TARGET: u32 = 7;
/// The `GUICommandType`s `doTeamHuntWithCommandButton` hunts with.
pub mod gui_command {
    /// `GUI_COMMAND_FIRE_WEAPON`.
    pub const FIRE_WEAPON: i32 = 20;
    /// `GUI_COMMAND_SPECIAL_POWER`.
    pub const SPECIAL_POWER: i32 = 21;
    /// `GUI_COMMAND_SWITCH_WEAPON`.
    pub const SWITCH_WEAPON: i32 = 26;
    /// `GUICOMMANDMODE_HIJACK_VEHICLE`.
    pub const HIJACK_VEHICLE: i32 = 27;
    /// `GUICOMMANDMODE_CONVERT_TO_CARBOMB`.
    pub const CONVERT_TO_CARBOMB: i32 = 28;
    /// `GUICOMMANDMODE_SABOTAGE_BUILDING`.
    pub const SABOTAGE_BUILDING: i32 = 29;
}
/// `OBJECT_STATUS_SCRIPT_DISABLED` .. `OBJECT_STATUS_SCRIPT_TARGETABLE`.
pub mod script_status {
    /// `OBJECT_STATUS_SCRIPT_DISABLED`.
    pub const DISABLED: i32 = 0x01;
    /// `OBJECT_STATUS_SCRIPT_UNPOWERED`.
    pub const UNPOWERED: i32 = 0x02;
    /// `OBJECT_STATUS_SCRIPT_UNSELLABLE`.
    pub const UNSELLABLE: i32 = 0x04;
    /// `OBJECT_STATUS_SCRIPT_UNSTEALTHED`.
    pub const UNSTEALTHED: i32 = 0x08;
    /// `OBJECT_STATUS_SCRIPT_TARGETABLE`.
    pub const TARGETABLE: i32 = 0x10;
}
/// `TheObjectFlagsNames` (the object panel's flags, in order).
pub const OBJECT_FLAGS_NAMES: [&[u8]; 7] =
    [b"Enabled", b"Powered", b"Indestructible", b"Unsellable", b"Selectable", b"AI Recruitable", b"Player Targetable"];

/// What `executeAction` can fail with.
#[derive(Clone, Debug, PartialEq)]
pub enum ActionError {
    /// Upstream is undefined here.
    Undefined(IniError),
    /// The action's type belongs to a batch not yet translated.
    NotTranslated(i32),
}
impl From<IniError> for ActionError {
    fn from(e: IniError) -> Self {
        ActionError::Undefined(e)
    }
}

/// An `AudioEventRTS` as the actions make it.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioEvent {
    /// The event's name.
    pub name: Vec<u8>,
    /// `AudioEventRTS(name, position)`'s position.
    pub position: Option<Coord3D>,
    /// `setIsLogicalAudio`.
    pub logical: bool,
    /// `setPlayerIndex` (-1 when not set).
    pub player_index: i32,
    /// `setObjectID` (0 when not set).
    pub object: ObjectId,
    /// `setUninterruptable`.
    pub uninterruptable: bool,
    /// `setShouldFade`.
    pub should_fade: bool,
}

/// `TeamTemplateInfo`'s fields that the actions read.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TeamTemplateInfo {
    /// `m_startReinforceWaypoint`.
    pub start_reinforce_waypoint: Vec<u8>,
    /// `m_transportUnitType`.
    pub transport_unit_type: Vec<u8>,
    /// `m_unitsInfo[i]`: the thing's name and `maxUnits`, `m_numUnitsInfo` of them.
    pub units_info: Vec<(Vec<u8>, i32)>,
    /// `m_teamStartsFull`.
    pub team_starts_full: bool,
    /// `m_transportsExit`.
    pub transports_exit: bool,
}

/// The searches of the team command-button actions (batch A9), by their filters after the player's affiliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSearch {
    /// Valid targets of the button (`doTeamUseCommandButtonOnNearestEnemy`).
    Enemy,
    /// Garrisonable structures (`..OnNearestGarrisonedBuilding`).
    GarrisonedBuilding,
    /// The kind of thing (`..OnNearestKindof`).
    KindOf(i32),
    /// Structures (`..OnNearestBuilding`).
    Building,
    /// Structures of the kind (`..OnNearestBuildingClass`).
    BuildingClass(i32),
    /// The template, enemies or neutrals (`..OnNearestObjectType`).
    Template(TemplateH),
    /// Unmanned objects on the map (`doTeamCaptureNearestUnownedFactionUnit`).
    Unmanned,
}

/// The world, as `ScriptActions.cpp` reaches it. See the module docs.
pub trait ScriptActionsHost {
    // --- windows, the logic, the client's input ---
    /// `TheWindowManager->winCreateFromScript(name)`.
    fn sa_win_create_from_script(&mut self, name: &[u8]) -> WindowH;
    /// `TheWindowManager->winDestroy(window)`.
    fn sa_win_destroy(&mut self, w: WindowH);
    /// `TheGameLogic->closeWindows()`.
    fn sa_logic_close_windows(&mut self);
    /// `TheGameLogic->findObjectByID(id)`.
    fn sa_find_object_by_id(&self, id: ObjectId) -> Option<ObjH>;
    /// `TheInGameUI->setInputEnabled(enabled)`.
    fn sa_ui_set_input_enabled(&mut self, enabled: bool);
    /// `TheMouse->setVisibility(visible)`.
    fn sa_mouse_set_visibility(&mut self, visible: bool);
    /// `TheInGameUI->deselectAllDrawables()`.
    fn sa_ui_deselect_all_drawables(&mut self);
    /// `TheInGameUI->clearAttackMoveToMode()`.
    fn sa_ui_clear_attack_move_to_mode(&mut self);
    /// `TheInGameUI->setWaypointMode(on)`.
    fn sa_ui_set_waypoint_mode(&mut self, on: bool);
    /// `TheControlBar->deleteBuildTooltipLayout()`.
    fn sa_control_bar_delete_build_tooltip_layout(&mut self);
    /// `TheLookAtTranslator->resetModes()`.
    fn sa_look_at_reset_modes(&mut self);
    /// `TheCampaignManager` is not NULL.
    fn sa_has_campaign_manager(&self) -> bool;
    /// `TheCampaignManager->SetVictorious(victorious)`.
    fn sa_campaign_set_victorious(&mut self, victorious: bool);
    /// `TheVictoryConditions->amIObserver()`.
    fn sa_am_i_observer(&self) -> bool;
    /// `TheWritableGlobalData->m_scriptOverrideInfantryLightScale = setting`.
    fn sa_set_infantry_light_scale(&mut self, setting: f32);
    /// `TheGlobalData->m_timeOfDay == TIME_OF_DAY_NIGHT`.
    fn sa_is_night(&self) -> bool;

    // --- the script engine ---
    /// `startQuickEndGameTimer()`.
    fn sa_start_quick_end_game_timer(&mut self);
    /// `startEndGameTimer()`.
    fn sa_start_end_game_timer(&mut self);
    /// `startCloseWindowTimer()`.
    fn sa_start_close_window_timer(&mut self);
    /// `hasShownMPLocalDefeatWindow()`.
    fn sa_has_shown_mp_local_defeat_window(&self) -> bool;
    /// `markMPLocalDefeatWindowShown()`.
    fn sa_mark_mp_local_defeat_window_shown(&mut self);
    /// `AppendDebugMessage(message, pause)`.
    fn sa_append_debug_message(&mut self, message: &[u8], pause: bool);
    /// `getUnitNamed(name)`.
    fn sa_unit_named(&self, name: &[u8]) -> Option<ObjH>;
    /// `getTeamNamed(name)`.
    fn sa_team_named(&self, name: &[u8]) -> Option<TeamH>;
    /// `didUnitExist(name)`.
    fn sa_did_unit_exist(&self, name: &[u8]) -> bool;
    /// `transferObjectName(name, obj)`.
    fn sa_transfer_object_name(&mut self, name: &[u8], o: ObjH);
    /// `addObjectToCache(obj)`.
    fn sa_add_object_to_cache(&mut self, o: ObjH);
    /// `getCurrentPlayer()`.
    fn sa_current_player(&self) -> Option<PlayerH>;
    /// `getPlayerFromAsciiString(name)`.
    fn sa_player_from_name(&self, name: &[u8]) -> Option<PlayerH>;

    // --- players ---
    /// `ThePlayerList->getLocalPlayer()`.
    fn sa_local_player(&self) -> Option<PlayerH>;
    /// `isPlayerObserver()`.
    fn sa_player_is_observer(&self, p: PlayerH) -> bool;
    /// `buildSpecificBuilding(type)`.
    fn sa_player_build_specific_building(&mut self, p: PlayerH, building: &[u8]);
    /// `buildBySupplies(cash, type)`.
    fn sa_player_build_by_supplies(&mut self, p: PlayerH, cash: i32, building: &[u8]);
    /// `buildSpecificBuildingNearestTeam(type, team)`.
    fn sa_player_build_specific_building_nearest_team(&mut self, p: PlayerH, building: &[u8], t: TeamH);
    /// `buildUpgrade(upgrade)`.
    fn sa_player_build_upgrade(&mut self, p: PlayerH, upgrade: &[u8]);
    /// `buildBaseDefense(flank)`.
    fn sa_player_build_base_defense(&mut self, p: PlayerH, flank: bool);
    /// `buildBaseDefenseStructure(type, flank)`.
    fn sa_player_build_base_defense_structure(&mut self, p: PlayerH, building: &[u8], flank: bool);

    // --- the audio and the terrain logic ---
    /// `TheAudio->addAudioEvent(&event)`.
    fn sa_audio_add_event(&mut self, e: &AudioEvent);
    /// `TheTerrainLogic->getWaypointByName(name)`.
    fn sa_waypoint_by_name(&self, name: &[u8]) -> Option<WaypointH>;
    /// `TheTerrainLogic->getFirstWaypoint()`.
    fn sa_first_waypoint(&self) -> Option<WaypointH>;
    /// `getNext()`.
    fn sa_waypoint_next(&self, w: WaypointH) -> Option<WaypointH>;
    /// `getName()`.
    fn sa_waypoint_name(&self, w: WaypointH) -> Vec<u8>;
    /// `getLocation()`.
    fn sa_waypoint_location(&self, w: WaypointH) -> Coord3D;
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn sa_ground_height(&self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->createCraterInTerrain(obj)`.
    fn sa_create_crater_in_terrain(&mut self, o: ObjH);
    /// `TheAI->pathfinder()->addObjectToPathfindMap(obj)`.
    fn sa_pathfinder_add_object(&mut self, o: ObjH);
    /// `oversizeTheTerrain(amount)`.
    fn sa_oversize_the_terrain(&mut self, amount: i32);

    // --- the view ---
    /// `setCameraLock(id)`.
    fn sa_view_set_camera_lock(&mut self, id: ObjectId);
    /// `snapToCameraLock()`.
    fn sa_view_snap_to_camera_lock(&mut self);
    /// `setSnapMode(lockType, lockDist)`.
    fn sa_view_set_snap_mode(&mut self, lock_type: i32, dist: f32);
    /// `moveCameraTo(&pos, frames, shutter, orient, easeIn, easeOut)`.
    fn sa_view_move_camera_to(&mut self, p: &Coord3D, frames: i32, shutter: i32, orient: bool, ease_in: f32, ease_out: f32);
    /// `moveCameraAlongWaypointPath(way, frames, shutter, orient, easeIn, easeOut)`.
    fn sa_view_move_camera_along_waypoint_path(&mut self, w: WaypointH, frames: i32, shutter: i32, orient: bool, ease_in: f32, ease_out: f32);
    /// `zoomCamera(zoom, milliseconds, easeIn, easeOut)`.
    fn sa_view_zoom_camera(&mut self, zoom: f32, ms: i32, ease_in: f32, ease_out: f32);
    /// `pitchCamera(pitch, milliseconds, easeIn, easeOut)`.
    fn sa_view_pitch_camera(&mut self, pitch: f32, ms: i32, ease_in: f32, ease_out: f32);
    /// `rotateCamera(rotations, frames, easeIn, easeOut)`.
    fn sa_view_rotate_camera(&mut self, rotations: f32, frames: i32, ease_in: f32, ease_out: f32);
    /// `rotateCameraTowardObject(id, milliseconds, holdMilliseconds, easeIn, easeOut)`.
    fn sa_view_rotate_camera_toward_object(&mut self, id: ObjectId, ms: i32, hold: i32, ease_in: f32, ease_out: f32);
    /// `rotateCameraTowardPosition(&pos, milliseconds, easeIn, easeOut, reverse)`.
    fn sa_view_rotate_camera_toward_position(&mut self, p: &Coord3D, ms: i32, ease_in: f32, ease_out: f32, reverse: bool);
    /// `resetCamera(&pos, frames, easeIn, easeOut)`.
    fn sa_view_reset_camera(&mut self, p: &Coord3D, frames: i32, ease_in: f32, ease_out: f32);
    /// `scrollBy(&delta)`.
    fn sa_view_scroll_by(&mut self, d: &Coord2D);
    /// `cameraModLookToward(&pos)`.
    fn sa_view_camera_mod_look_toward(&mut self, p: &Coord3D);
    /// `cameraModFinalLookToward(&pos)`.
    fn sa_view_camera_mod_final_look_toward(&mut self, p: &Coord3D);
    /// `cameraModFinalMoveTo(&pos)`.
    fn sa_view_camera_mod_final_move_to(&mut self, p: &Coord3D);
    /// `cameraModFinalPitch(pitch, easeIn, easeOut)`.
    fn sa_view_camera_mod_final_pitch(&mut self, pitch: f32, ease_in: f32, ease_out: f32);
    /// `cameraModFinalZoom(zoom, easeIn, easeOut)`.
    fn sa_view_camera_mod_final_zoom(&mut self, zoom: f32, ease_in: f32, ease_out: f32);
    /// `TheGameClient`'s drawables, `firstDrawable` then `getNextDrawable`: each one's `isSelected()` and `getPosition()`.
    fn sa_drawables(&self) -> Vec<(bool, Coord3D)>;

    // --- teams and the team factory ---
    /// `damageTeamMembers(amount)`.
    fn sa_team_damage_members(&mut self, t: TeamH, amount: f32);
    /// `setState(state)`.
    fn sa_team_set_state(&mut self, t: TeamH, state: &[u8]);
    /// `setActive()`.
    fn sa_team_set_active(&mut self, t: TeamH);
    /// `getTeamAsAIGroup(group)`.
    fn sa_team_as_ai_group(&mut self, t: TeamH, g: GroupH);
    /// `iterate_TeamMemberList()`, in order.
    fn sa_team_members(&self, t: TeamH) -> Vec<ObjH>;
    /// `TheTeamFactory->findTeamPrototype(name)`.
    fn sa_find_team_prototype(&self, name: &[u8]) -> Option<ProtoH>;
    /// `getTemplateInfo()`.
    fn sa_proto_template_info(&self, p: ProtoH) -> TeamTemplateInfo;
    /// `TheTeamFactory->createInactiveTeam(name)`.
    fn sa_create_inactive_team(&mut self, name: &[u8]) -> Option<TeamH>;
    /// `TheTeamFactory->createTeam(name)`.
    fn sa_create_team(&mut self, name: &[u8]) -> Option<TeamH>;

    // --- the AI ---
    /// `TheAI->createGroup()`.
    fn sa_create_group(&mut self) -> Option<GroupH>;
    /// `groupMoveToPosition(&pos, addWaypoint, cmdSource)`.
    fn sa_group_move_to_position(&mut self, g: GroupH, p: &Coord3D, add_waypoint: bool, cmd: i32);
    /// `groupAttackTeam(team, maxShots, cmdSource)`.
    fn sa_group_attack_team(&mut self, g: GroupH, t: TeamH, max_shots: i32, cmd: i32);
    /// `getAIUpdateInterface()` is not NULL.
    fn sa_obj_has_ai(&self, o: ObjH) -> bool;
    /// `clearWaypointQueue()`.
    fn sa_ai_clear_waypoint_queue(&mut self, o: ObjH);
    /// `chooseLocomotorSet(set)`.
    fn sa_ai_choose_locomotor_set(&mut self, o: ObjH, set: i32);
    /// `aiMoveToPosition(&pos, cmdSource)`.
    fn sa_ai_move_to_position(&mut self, o: ObjH, p: &Coord3D, cmd: i32);
    /// `aiMoveToAndEvacuate(&pos, cmdSource)`.
    fn sa_ai_move_to_and_evacuate(&mut self, o: ObjH, p: &Coord3D, cmd: i32);
    /// `aiMoveToAndEvacuateAndExit(&pos, cmdSource)`.
    fn sa_ai_move_to_and_evacuate_and_exit(&mut self, o: ObjH, p: &Coord3D, cmd: i32);
    /// `aiForceAttackObject(victim, maxShots, cmdSource)`.
    fn sa_ai_force_attack_object(&mut self, o: ObjH, victim: ObjH, max_shots: i32, cmd: i32);
    /// `setIsRecruitable(recruitable)`.
    fn sa_ai_set_is_recruitable(&mut self, o: ObjH, recruitable: bool);

    // --- objects, templates and the thing factory ---
    /// `TheThingFactory->findTemplate(name)`.
    fn sa_find_template(&self, name: &[u8]) -> Option<TemplateH>;
    /// `TheThingFactory->newObject(template, team)`.
    fn sa_new_object(&mut self, t: TemplateH, team: Option<TeamH>) -> Option<ObjH>;
    /// `getTemplate()`.
    fn sa_obj_template(&self, o: ObjH) -> Option<TemplateH>;
    /// `isEquivalentTo(other)`.
    fn sa_template_is_equivalent_to(&self, a: TemplateH, b: Option<TemplateH>) -> bool;
    /// `isEffectivelyDead()`.
    fn sa_obj_is_effectively_dead(&self, o: ObjH) -> bool;
    /// `isKindOf(kind)`.
    fn sa_obj_is_kind_of(&self, o: ObjH, k: KindOfType) -> bool;
    /// `isDisabledByType(type)`.
    fn sa_obj_is_disabled_by_type(&self, o: ObjH, t: DisabledType) -> bool;
    /// `setName(name)`.
    fn sa_obj_set_name(&mut self, o: ObjH, name: &[u8]);
    /// `setPosition(&pos)`.
    fn sa_obj_set_position(&mut self, o: ObjH, p: &Coord3D);
    /// `setOrientation(angle)`.
    fn sa_obj_set_orientation(&mut self, o: ObjH, angle: f32);
    /// `getGeometryInfo().getMajorRadius()`.
    fn sa_obj_major_radius(&self, o: ObjH) -> f32;
    /// `getTransportSlotCount()`.
    fn sa_obj_transport_slot_count(&self, o: ObjH) -> i32;
    /// `getContainedBy()` is not NULL.
    fn sa_obj_is_contained(&self, o: ObjH) -> bool;
    /// `leaveGroup()`.
    fn sa_obj_leave_group(&mut self, o: ObjH);
    /// `setScriptStatus(bit, value)`.
    fn sa_obj_set_script_status(&mut self, o: ObjH, bit: i32, value: bool);
    /// `getBodyModule()` is not NULL.
    fn sa_obj_has_body(&self, o: ObjH) -> bool;
    /// `getBodyModule()->setIndestructible(value)`.
    fn sa_body_set_indestructible(&mut self, o: ObjH, value: bool);
    /// `isSelectable()`.
    fn sa_obj_is_selectable(&self, o: ObjH) -> bool;
    /// `setSelectable(value)`.
    fn sa_obj_set_selectable(&mut self, o: ObjH, value: bool);
    /// `getDrawable()` is not NULL.
    fn sa_obj_has_drawable(&self, o: ObjH) -> bool;
    /// `getDrawable()->enableAmbientSoundFromScript(enable)`.
    fn sa_drawable_enable_ambient_sound(&mut self, o: ObjH, enable: bool);
    /// `getContain()` is not NULL.
    fn sa_obj_has_contain(&self, o: ObjH) -> bool;
    /// `getContain()->getContainMax()` (as `TransportContain`).
    fn sa_contain_max(&self, o: ObjH) -> u32;
    /// `getContain()->isValidContainerFor(obj, checkCapacity)`.
    fn sa_contain_is_valid_container_for(&self, c: ObjH, o: ObjH, check_capacity: bool) -> bool;
    /// `getContain()->addToContain(obj)`.
    fn sa_contain_add(&mut self, c: ObjH, o: ObjH);
    /// `findUpdateModule("DeliverPayloadAIUpdate")` is not NULL.
    fn sa_obj_has_deliver_payload(&self, o: ObjH) -> bool;
    /// That module's `getPutInContainerTemplateViaModuleData()`.
    fn sa_deliver_payload_put_in_container_template(&self, o: ObjH) -> Option<TemplateH>;
    /// That module's `deliverPayloadViaModuleData(&pos)`.
    fn sa_deliver_payload(&mut self, o: ObjH, p: &Coord3D);

    // --- batch A2 ---
    /// `TheScriptEngine->getAttackInfo(name)`'s name (the default set's is empty).
    fn sa_attack_info(&self, name: &[u8]) -> Vec<u8>;
    /// `setAttackPriorityName(name)`.
    fn sa_team_set_attack_priority_name(&mut self, t: TeamH, name: &[u8]);
    /// `setAttackInfo(info)` (the info by its name).
    fn sa_ai_set_attack_info(&mut self, o: ObjH, info: &[u8]);
    /// `setTeamDelaySeconds(seconds)`.
    fn sa_player_set_team_delay_seconds(&mut self, p: PlayerH, seconds: i32);
    /// `setAttitude(attitude)`.
    fn sa_ai_set_attitude(&mut self, o: ObjH, attitude: i32);
    /// `AIGroup::setAttitude(attitude)`.
    fn sa_group_set_attitude(&mut self, g: GroupH, attitude: i32);
    /// `setStatus(MAKE_OBJECT_STATUS_MASK(status), value)`.
    fn sa_obj_set_status(&mut self, o: ObjH, status: i32, value: bool);
    /// `TheScriptEngine->getQualifiedTriggerAreaByName(name)`.
    fn sa_trigger_named(&self, name: &[u8]) -> Option<TriggerH>;
    /// `aiAttackArea(trigger, cmdSource)`.
    fn sa_ai_attack_area(&mut self, o: ObjH, trig: TriggerH, cmd: i32);
    /// `aiAttackTeam(team, maxShots, cmdSource)`.
    fn sa_ai_attack_team(&mut self, o: ObjH, t: TeamH, max_shots: i32, cmd: i32);
    /// `groupAttackArea(trigger, cmdSource)`.
    fn sa_group_attack_area(&mut self, g: GroupH, trig: TriggerH, cmd: i32);
    /// `groupAttackObject(victim, maxShots, cmdSource)`.
    fn sa_group_attack_object(&mut self, g: GroupH, victim: ObjH, max_shots: i32, cmd: i32);
    /// `aiEnter(target, cmdSource)`.
    fn sa_ai_enter(&mut self, o: ObjH, target: ObjH, cmd: i32);
    /// `groupEnter(target, cmdSource)`.
    fn sa_group_enter(&mut self, g: GroupH, target: ObjH, cmd: i32);
    /// `aiEvacuate(exposeStealthUnits, cmdSource)`.
    fn sa_ai_evacuate(&mut self, o: ObjH, expose: bool, cmd: i32);
    /// `groupEvacuate(cmdSource)`.
    fn sa_group_evacuate(&mut self, g: GroupH, cmd: i32);
    /// `getContain()->setEvacDisposition(disposition)`.
    fn sa_contain_set_evac_disposition(&mut self, o: ObjH, disposition: u32);
    /// `getPosition()`.
    fn sa_obj_position(&self, o: ObjH) -> Coord3D;
    /// `TheTerrainLogic->getClosestWaypointOnPath(&pos, label)`.
    fn sa_closest_waypoint_on_path(&self, p: &Coord3D, label: &[u8]) -> Option<WaypointH>;
    /// `aiFollowWaypointPath(way, cmdSource)` (`exact`: `aiFollowWaypointPathExact`).
    fn sa_ai_follow_waypoint_path(&mut self, o: ObjH, w: WaypointH, exact: bool, cmd: i32);
    /// `groupFollowWaypointPath`, `..Exact`, `..AsTeam`, `..AsTeamExact` (way, cmdSource).
    fn sa_group_follow_waypoint_path(&mut self, g: GroupH, w: WaypointH, as_team: bool, exact: bool, cmd: i32);
    /// `TheScriptEngine->getSkirmishEnemyPlayer()`.
    fn sa_skirmish_enemy_player(&self) -> Option<PlayerH>;
    /// `getMpStartIndex()`.
    fn sa_player_mp_start_index(&self, p: PlayerH) -> i32;
    /// `checkBridges(unit, way)`.
    fn sa_player_check_bridges(&mut self, p: PlayerH, o: ObjH, w: WaypointH);
    /// `aiGuardPosition(&pos, mode, cmdSource)`.
    fn sa_ai_guard_position(&mut self, o: ObjH, p: &Coord3D, mode: i32, cmd: i32);
    /// `groupGuardPosition(&pos, mode, cmdSource)`.
    fn sa_group_guard_position(&mut self, g: GroupH, p: &Coord3D, mode: i32, cmd: i32);
    /// `groupGuardObject(obj, mode, cmdSource)`.
    fn sa_group_guard_object(&mut self, g: GroupH, o: ObjH, mode: i32, cmd: i32);
    /// `groupGuardArea(trigger, mode, cmdSource)`.
    fn sa_group_guard_area(&mut self, g: GroupH, trig: TriggerH, mode: i32, cmd: i32);
    /// `aiHunt(cmdSource)`.
    fn sa_ai_hunt(&mut self, o: ObjH, cmd: i32);
    /// `groupHunt(cmdSource)`.
    fn sa_group_hunt(&mut self, g: GroupH, cmd: i32);
    /// `TheControlBar->findCommandButton(name)`.
    fn sa_find_command_button(&self, name: &[u8]) -> Option<ButtonH>;
    /// `getCommandType()`.
    fn sa_button_command_type(&self, b: ButtonH) -> i32;
    /// `getSpecialPowerTemplate()` is not NULL.
    fn sa_button_has_special_power(&self, b: ButtonH) -> bool;
    /// `getOptions()`.
    fn sa_button_options(&self, b: ButtonH) -> u32;
    /// `getCommandSetString()`.
    fn sa_obj_command_set_string(&self, o: ObjH) -> Vec<u8>;
    /// `TheControlBar->findCommandSet(name)`.
    fn sa_find_command_set(&self, name: &[u8]) -> Option<CommandSetH>;
    /// `getCommandButton(i)`.
    fn sa_command_set_button(&self, s: CommandSetH, i: i32) -> Option<ButtonH>;
    /// `getTemplate()->getName()`.
    fn sa_template_name(&self, t: TemplateH) -> Vec<u8>;
    /// `findUpdateModule("CommandButtonHuntUpdate")` is not NULL.
    fn sa_obj_has_command_button_hunt(&self, o: ObjH) -> bool;
    /// That module's `setCommandButton(ability)`.
    fn sa_command_button_hunt_set(&mut self, o: ObjH, ability: &[u8]);
    /// `setUnitsShouldHunt(hunt, cmdSource)`.
    fn sa_player_set_units_should_hunt(&mut self, p: PlayerH, hunt: bool, cmd: i32);
    /// `sellEverythingUnderTheSun()`.
    fn sa_player_sell_everything(&mut self, p: PlayerH);
    /// `setCanBuildBase(can)`.
    fn sa_player_set_can_build_base(&mut self, p: PlayerH, can: bool);
    /// `setCanBuildUnits(can)`.
    fn sa_player_set_can_build_units(&mut self, p: PlayerH, can: bool);
    /// `setObjectsEnabled(templateName, enable)`.
    fn sa_player_set_objects_enabled(&mut self, p: PlayerH, name: &[u8], enable: bool);
    /// `repairStructure(id)`.
    fn sa_player_repair_structure(&mut self, p: PlayerH, id: ObjectId);

    // --- batch A3 ---
    /// `TeamPrototype::getControllingPlayer()`.
    fn sa_proto_controlling_player(&self, p: ProtoH) -> Option<PlayerH>;
    /// `buildSpecificTeam(proto)`.
    fn sa_player_build_specific_team(&mut self, p: PlayerH, proto: ProtoH);
    /// `recruitSpecificTeam(proto, radius)`.
    fn sa_player_recruit_specific_team(&mut self, p: PlayerH, proto: ProtoH, radius: f32);
    /// `attemptDamage(&info)`: the source, damage type, death type, amount.
    fn sa_obj_attempt_damage(&mut self, o: ObjH, source: ObjectId, damage_type: i32, death_type: i32, amount: f32);
    /// `TheGameLogic->destroyObject(obj)`.
    fn sa_logic_destroy_object(&mut self, o: ObjH);
    /// `deleteTeam(ignoreDead)`.
    fn sa_team_delete(&mut self, t: TeamH, ignore_dead: bool);
    /// `aiWander(way, cmdSource)`.
    fn sa_ai_wander(&mut self, o: ObjH, w: WaypointH, cmd: i32);
    /// `aiWanderInPlace(cmdSource)`.
    fn sa_ai_wander_in_place(&mut self, o: ObjH, cmd: i32);
    /// `aiPanic(way, cmdSource)`.
    fn sa_ai_panic(&mut self, o: ObjH, w: WaypointH, cmd: i32);
    /// `Team::getPrototype()`.
    fn sa_team_prototype(&self, t: TeamH) -> Option<ProtoH>;
    /// `increaseAIPriorityForSuccess()` (`up`) or `decreaseAIPriorityForFailure()`.
    fn sa_proto_change_priority(&mut self, p: ProtoH, up: bool);
    /// `getTemplateInfo()->m_productionPriority`.
    fn sa_proto_production_priority(&self, p: ProtoH) -> i32;
    /// `kill()`.
    fn sa_obj_kill(&mut self, o: ObjH);
    /// `killTeam()`.
    fn sa_team_kill(&mut self, t: TeamH);
    /// `killPlayer()`.
    fn sa_player_kill(&mut self, p: PlayerH);
    /// `TheInGameUI->message(text)`.
    fn sa_ui_message(&mut self, text: &[u8]);
    /// `TheInGameUI->popupMessage(message, x, y, width, pause, pauseMusic)`.
    fn sa_ui_popup_message(&mut self, message: &[u8], x: i32, y: i32, width: i32, pause: bool, pause_music: bool);
    /// `TheGameText->fetch(label, &exists)`: the text (as bytes) and whether it exists.
    fn sa_game_text_fetch(&self, label: &[u8]) -> (Vec<u16>, bool);
    /// `TheDisplay->setCinematicText(text)`.
    fn sa_display_set_cinematic_text(&mut self, text: &[u8]);
    /// `TheFontLibrary->getFont(name, size, bold)` then `TheDisplay->setCinematicFont(font)`.
    fn sa_display_set_cinematic_font(&mut self, name: &[u8], size: i32, bold: bool);
    /// `TheGlobalLanguageData->adjustFontSize(size)`.
    fn sa_adjust_font_size(&self, size: i32) -> i32;
    /// `TheDisplay->setCinematicTextFrames(frames)`.
    fn sa_display_set_cinematic_text_frames(&mut self, frames: i32);
    /// `button->setFlashCount(count)`.
    fn sa_button_set_flash_count(&mut self, b: ButtonH, count: i32);
    /// `TheControlBar->setFlash(on)`.
    fn sa_control_bar_set_flash(&mut self, on: bool);
    /// `setCustomIndicatorColor(color)`.
    fn sa_obj_set_custom_indicator_color(&mut self, o: ObjH, c: i32);
    /// `getIndicatorColor()`.
    fn sa_obj_indicator_color(&self, o: ObjH) -> i32;
    /// `getNightIndicatorColor()`.
    fn sa_obj_night_indicator_color(&self, o: ObjH) -> i32;
    /// `getDrawable()->setFlashColor(color)`.
    fn sa_drawable_set_flash_color(&mut self, o: ObjH, c: i32);
    /// `getDrawable()->setFlashCount(count)`.
    fn sa_drawable_set_flash_count(&mut self, o: ObjH, count: i32);
    /// `getDrawable()->setIndicatorColor(color)`.
    fn sa_drawable_set_indicator_color(&mut self, o: ObjH, c: i32);
    /// `Team::hasAnyObjects()`.
    fn sa_team_has_any_objects(&self, t: TeamH) -> bool;
    /// `TheDisplay->playMovie(name)`.
    fn sa_display_play_movie(&mut self, name: &[u8]);
    /// `TheInGameUI->playMovie(name)`.
    fn sa_ui_play_movie(&mut self, name: &[u8]);
    /// `TheInGameUI->militarySubtitle(label, duration)`.
    fn sa_ui_military_subtitle(&mut self, label: &[u8], duration: i32);
    /// `transferAssetsFromThat(src)`.
    fn sa_player_transfer_assets_from(&mut self, dest: PlayerH, src: PlayerH);
    /// `getDefaultTeam()`.
    fn sa_player_default_team(&self, p: PlayerH) -> Option<TeamH>;
    /// `setTeam(team)`.
    fn sa_obj_set_team(&mut self, o: ObjH, t: Option<TeamH>);
    /// `updateUpgradeModules()`.
    fn sa_obj_update_upgrade_modules(&mut self, o: ObjH);
    /// `setListInScoreScreen(list)`.
    fn sa_player_set_list_in_score_screen(&mut self, p: PlayerH, list: bool);
    /// `TheGameLogic->enableScoring(score)`.
    fn sa_logic_enable_scoring(&mut self, score: bool);
    /// `setPlayerRelationship(other, relationship)`.
    fn sa_player_set_player_relationship(&mut self, p: PlayerH, other: PlayerH, r: i32);
    /// `TheRadar->createEvent(&pos, type)`.
    fn sa_radar_create_event(&mut self, p: &Coord3D, event_type: i32);

    // --- batch A4 ---
    /// `Team::hasAnyUnits()`.
    fn sa_team_has_any_units(&self, t: TeamH) -> bool;
    /// `getEstimateTeamPosition()`.
    fn sa_team_estimate_position(&self, t: TeamH) -> Option<Coord3D>;
    /// `TheRadar->hide(hide)`.
    fn sa_radar_hide(&mut self, hide: bool);
    /// `TheTacticalView->setViewFilter(filter)`.
    fn sa_view_set_view_filter(&mut self, filter: i32) -> bool;
    /// `TheTacticalView->setViewFilterMode(mode)`.
    fn sa_view_set_view_filter_mode(&mut self, mode: i32) -> bool;
    /// `TheTacticalView->setViewFilterPos(&pos)`.
    fn sa_view_set_view_filter_pos(&mut self, p: &Coord3D);
    /// `TheTacticalView->lookAt(&pos)`.
    fn sa_view_look_at(&mut self, p: &Coord3D);
    /// `ThePlayerList->getPlayerCount()`.
    fn sa_player_count(&self) -> i32;
    /// `ThePlayerList->getNthPlayer(i)`.
    fn sa_nth_player(&self, i: i32) -> Option<PlayerH>;
    /// `getPlayerType()` is `PLAYER_HUMAN`.
    fn sa_player_is_human(&self, p: PlayerH) -> bool;
    /// `getPlayerMask()`.
    fn sa_player_mask(&self, p: PlayerH) -> u32;
    /// `ThePartitionManager->doShroudReveal` (`cover` false) or `doShroudCover`, then the `undo..` of the same (`undo`).
    fn sa_partition_shroud(&mut self, cover: bool, undo: bool, x: f32, y: f32, r: f32, mask: u32);
    /// `revealMapForPlayer(i)`.
    fn sa_partition_reveal_map(&mut self, i: i32);
    /// `revealMapForPlayerPermanently(i)` (`undo`: `undoRevealMapForPlayerPermanently`).
    fn sa_partition_reveal_map_permanently(&mut self, i: i32, undo: bool);
    /// `shroudMapForPlayer(i)`.
    fn sa_partition_shroud_map(&mut self, i: i32);
    /// `setRecruitable(recruitable)`.
    fn sa_team_set_recruitable(&mut self, t: TeamH, recruitable: bool);
    /// `TheTeamFactory->findTeam(name)`.
    fn sa_find_team(&self, name: &[u8]) -> Option<TeamH>;
    /// `TheGlobalData->m_shroudAlpha` (`shroud`) or `m_clearAlpha`.
    fn sa_global_alpha(&self, shroud: bool) -> u8;
    /// `TheDisplay->setBorderShroudLevel(level)`.
    fn sa_display_set_border_shroud_level(&mut self, level: u8);
    /// `setUnitsShouldIdleOrResume(idle)`.
    fn sa_player_set_units_should_idle_or_resume(&mut self, p: PlayerH, idle: bool);
    /// `TheAudio->pauseAmbient(pause)`.
    fn sa_audio_pause_ambient(&mut self, pause: bool);
    /// `TheAudio->removeAudioEvent(handle)`.
    fn sa_audio_remove_event_handle(&mut self, handle: i32);
    /// `TheScriptEngine->setCurrentTrackName(name)`.
    fn sa_set_current_track_name(&mut self, name: &[u8]);
    /// `getContain()->getPlayerWhoEntered()`.
    fn sa_contain_player_who_entered(&self, o: ObjH) -> u32;
    /// `getContain()->getContainCount()`.
    fn sa_contain_count(&self, o: ObjH) -> u32;
    /// `Team::getControllingPlayer()`.
    fn sa_team_controlling_player(&self, t: TeamH) -> Option<PlayerH>;
    /// `getContain()->removeAllContained(exposeStealthUnits)`.
    fn sa_contain_remove_all_contained(&mut self, o: ObjH, expose: bool);
    /// `iterateObjectsInRange(leader, REALLY_FAR, FROM_CENTER_3D, {garrisonable by the player (or an internet center for a
    /// money hacker), same map status}, ITER_SORTED_NEAR_TO_FAR)`.
    fn sa_partition_garrison_candidates(&mut self, leader: ObjH, money_hacker: bool) -> Vec<ObjH>;
    /// `aiExit(NULL, cmdSource)`.
    fn sa_ai_exit(&mut self, o: ObjH, cmd: i32);

    // --- batch A5 ---
    /// `iterateObjectsInRange(unit, REALLY_FAR, FROM_CENTER_3D, {structure, same map status, internet center (a money hacker) or not
    /// one}, ITER_SORTED_NEAR_TO_FAR)`.
    fn sa_partition_unit_garrison_candidates(&mut self, unit: ObjH, money_hacker: bool) -> Vec<ObjH>;
    /// `getControllingPlayer()`.
    fn sa_obj_controlling_player(&self, o: ObjH) -> Option<PlayerH>;
    /// `getTeam()`.
    fn sa_obj_team(&self, o: ObjH) -> Option<TeamH>;
    /// `setDisabled(type)`.
    fn sa_obj_set_disabled(&mut self, o: ObjH, t: DisabledType);
    /// `setDisabledUntil(type, frame)`.
    fn sa_obj_set_disabled_until(&mut self, o: ObjH, t: DisabledType, frame: u32);
    /// `TheGameLogic->deselectObject(obj, mask, affectClient)`.
    fn sa_logic_deselect_object(&mut self, o: ObjH, mask: u32, affect_client: bool);
    /// `ThePlayerList->getNeutralPlayer()`.
    fn sa_neutral_player(&self) -> Option<PlayerH>;
    /// `findUpdateModule("StickyBombUpdate")` is not NULL.
    fn sa_obj_has_sticky_bomb_update(&self, o: ObjH) -> bool;
    /// `getGeometryInfo().makeRandomOffsetOnPerimeter(pos)`.
    fn sa_obj_random_offset_on_perimeter(&mut self, o: ObjH) -> Coord3D;
    /// `Matrix3D::Transform_Vector(*getTransformMatrix(), v)`.
    fn sa_obj_transform_vector(&self, o: ObjH, v: &Coord3D) -> Coord3D;
    /// The sticky bomb update's `initStickyBomb(target, NULL, &pos)`.
    fn sa_sticky_bomb_init(&mut self, bomb: ObjH, target: ObjH, pos: &Coord3D);
    /// `garrisonAllUnits(cmdSource)` (`garrison`) or `ungarrisonAllUnits(cmdSource)`.
    fn sa_player_garrison_all(&mut self, p: PlayerH, garrison: bool, cmd: i32);
    /// `HideControlBar(immediate)` (`hide`) or `ShowControlBar(immediate)`.
    fn sa_control_bar_show(&mut self, hide: bool, immediate: bool);
    /// `TheDisplay->enableLetterBox(enable)`.
    fn sa_display_enable_letter_box(&mut self, enable: bool);
    /// `TheTacticalView->setFadeParameters(frames, direction)`.
    fn sa_view_set_fade_parameters(&mut self, frames: i32, direction: i32);
    /// `TheTacticalView->getViewFilterType()`.
    fn sa_view_filter_type(&self) -> i32;
    /// `doSkyBoxSet(draw)`.
    fn sa_sky_box_set(&mut self, draw: bool);
    /// `TheSnowManager->setVisible(visible)`.
    fn sa_snow_set_visible(&mut self, visible: bool);
    /// `TheScriptEngine->doFreezeTime()` (`freeze`) or `doUnfreezeTime()`.
    fn sa_freeze_time(&mut self, freeze: bool);
    /// `getAIUpdateInterface()->getCurLocomotor()` is not NULL.
    fn sa_ai_has_cur_locomotor(&self, o: ObjH) -> bool;
    /// `getCurLocomotor()->setCloseEnoughDist(dist)`.
    fn sa_locomotor_set_close_enough_dist(&mut self, o: ObjH, dist: f32);
    /// `TheInGameUI->setSuperweaponDisplayEnabledByScript(enabled)`.
    fn sa_ui_set_superweapon_display_enabled(&mut self, enabled: bool);
    /// `TheInGameUI->hideObjectSuperweaponDisplayByScript(obj)` (`hide`) or `show..`.
    fn sa_ui_object_superweapon_display(&mut self, o: ObjH, hide: bool);
    /// `TheAudio->setVolume(volume, whichToAffect)`.
    fn sa_audio_set_volume(&mut self, volume: f32, affect: i32);
    /// `Team::setControllingPlayer(player)`.
    fn sa_team_set_controlling_player(&mut self, t: TeamH, p: PlayerH);
    /// `getMoney()->countMoney()`.
    fn sa_money_count(&self, p: PlayerH) -> u32;
    /// `getMoney()->withdraw(amount)`.
    fn sa_money_withdraw(&mut self, p: PlayerH, amount: u32);
    /// `getMoney()->deposit(amount)`.
    fn sa_money_deposit(&mut self, p: PlayerH, amount: u32);
    /// `TheInGameUI->addNamedTimer(name, text, countdown)`.
    fn sa_ui_add_named_timer(&mut self, name: &[u8], text: &[u16], countdown: bool);
    /// `TheInGameUI->removeNamedTimer(name)`.
    fn sa_ui_remove_named_timer(&mut self, name: &[u8]);
    /// `TheInGameUI->showNamedTimerDisplay(show)`.
    fn sa_ui_show_named_timer_display(&mut self, show: bool);
    /// `TheSpecialPowerStore->findSpecialPowerTemplate(name)`.
    fn sa_find_special_power(&self, name: &[u8]) -> Option<SpecialPowerH>;
    /// `getSpecialPowerModule(power)` is not NULL.
    fn sa_obj_has_special_power_module(&self, o: ObjH, power: SpecialPowerH) -> bool;
    /// That module's `pauseCountdown(pause)`.
    fn sa_spm_pause_countdown(&mut self, o: ObjH, power: SpecialPowerH, pause: bool);
    /// That module's `getReadyFrame()`.
    fn sa_spm_ready_frame(&self, o: ObjH, power: SpecialPowerH) -> u32;
    /// That module's `setReadyFrame(frame)`.
    fn sa_spm_set_ready_frame(&mut self, o: ObjH, power: SpecialPowerH, frame: u32);
    /// That module's `doSpecialPowerAtLocation(&pos, angle, options)`.
    fn sa_spm_do_at_location(&mut self, o: ObjH, power: SpecialPowerH, p: &Coord3D, angle: f32, options: u32);
    /// `TheGameLogic->getFrame()`.
    fn sa_frame(&self) -> u32;

    // --- batch A6 ---
    /// `getPlayerTeams()`: each prototype's instance list, in order.
    fn sa_player_teams(&self, p: PlayerH) -> Vec<Vec<TeamH>>;
    /// `getRadiusCursorRadius()`.
    fn sa_special_power_radius_cursor(&self, power: SpecialPowerH) -> f32;
    /// `getSpecialPowerType()`.
    fn sa_special_power_type(&self, power: SpecialPowerH) -> i32;
    /// The module's `isReady()`.
    fn sa_spm_is_ready(&self, o: ObjH, power: SpecialPowerH) -> bool;
    /// The module's `getReferenceThingTemplate()`.
    fn sa_spm_reference_template(&self, o: ObjH, power: SpecialPowerH) -> Option<TemplateH>;
    /// The module's `doSpecialPowerAtObject(target, options)`.
    fn sa_spm_do_at_object(&mut self, o: ObjH, power: SpecialPowerH, target: ObjH, options: u32);
    /// `computeSuperweaponTarget(power, &pos, playerIndex, weight)`.
    fn sa_player_compute_superweapon_target(&mut self, p: PlayerH, power: SpecialPowerH, index: i32, weight: f32) -> Option<Coord3D>;
    /// `calcClosestConstructionZoneLocation(template, &pos)`.
    fn sa_player_closest_construction_zone(&mut self, p: PlayerH, t: TemplateH) -> Option<Coord3D>;
    /// `getName()` of a command button.
    fn sa_button_name(&self, b: ButtonH) -> Vec<u8>;
    /// `doCommandButton(button, cmdSource)`.
    fn sa_obj_do_command_button(&mut self, o: ObjH, b: ButtonH, cmd: i32);
    /// `doCommandButtonAtObject(button, target, cmdSource)`.
    fn sa_obj_do_command_button_at_object(&mut self, o: ObjH, b: ButtonH, target: ObjH, cmd: i32);
    /// `doCommandButtonAtPosition(button, &pos, cmdSource)`.
    fn sa_obj_do_command_button_at_position(&mut self, o: ObjH, b: ButtonH, p: &Coord3D, cmd: i32);
    /// `doCommandButtonUsingWaypoints(button, way, cmdSource)`.
    fn sa_obj_do_command_button_using_waypoints(&mut self, o: ObjH, b: ButtonH, w: WaypointH, cmd: i32);
    /// `groupDoCommandButton(button, cmdSource)`.
    fn sa_group_do_command_button(&mut self, g: GroupH, b: ButtonH, cmd: i32);
    /// `groupDoCommandButtonAtObject(button, target, cmdSource)`.
    fn sa_group_do_command_button_at_object(&mut self, g: GroupH, b: ButtonH, target: ObjH, cmd: i32);
    /// `groupDoCommandButtonAtPosition(button, &pos, cmdSource)`.
    fn sa_group_do_command_button_at_position(&mut self, g: GroupH, b: ButtonH, p: &Coord3D, cmd: i32);
    /// `TheRadar->refreshTerrain(TheTerrainLogic)`.
    fn sa_radar_refresh_terrain(&mut self);
    /// `TheTacticalView->setDefaultView(pitch, angle, maxHeight)`.
    fn sa_view_set_default_view(&mut self, pitch: f32, angle: f32, max_height: f32);
    /// `aiIdle(cmdSource)`.
    fn sa_ai_idle(&mut self, o: ObjH, cmd: i32);
    /// `groupIdle(cmdSource)`.
    fn sa_group_idle(&mut self, g: GroupH, cmd: i32);
    /// `Team::getName()`.
    fn sa_team_name(&self, t: TeamH) -> Vec<u8>;
    /// `setOverrideTeamRelationship(otherTeamId, relation)`.
    fn sa_team_set_override_team_relationship(&mut self, t: TeamH, other: TeamH, r: i32);
    /// `removeOverrideTeamRelationship(otherTeamId)` (0: all).
    fn sa_team_remove_override_team_relationship(&mut self, t: TeamH, other: TeamH);
    /// `setOverridePlayerRelationship(playerIndex, relation)`.
    fn sa_team_set_override_player_relationship(&mut self, t: TeamH, p: PlayerH, r: i32);
    /// `removeOverridePlayerRelationship(playerIndex)` (0, NULL upstream: all... as the team reads it).
    fn sa_team_remove_override_player_relationship(&mut self, t: TeamH, p: PlayerH);
    /// `ThePlayerList->findPlayerWithNameKey(NAMEKEY(name))`.
    fn sa_player_with_name_key(&self, name: &[u8]) -> Option<PlayerH>;
    /// `setTeamRelationship(team, relation)`.
    fn sa_player_set_team_relationship(&mut self, p: PlayerH, t: TeamH, r: i32);
    /// `removeTeamRelationship(team)`.
    fn sa_player_remove_team_relationship(&mut self, p: PlayerH, t: TeamH);
    /// `TheScriptEngine->findScriptByName(name)` is not NULL.
    fn sa_find_script(&self, name: &[u8]) -> bool;
    /// `appendSequentialScript(seq)`: the team or object, the script's name, the times to loop.
    fn sa_append_sequential_script(&mut self, team: Option<TeamH>, object: ObjectId, script: &[u8], times_to_loop: i32);
    /// `removeAllSequentialScripts(obj)`.
    fn sa_remove_all_sequential_scripts_obj(&mut self, o: ObjH);
    /// `removeAllSequentialScripts(team)`.
    fn sa_remove_all_sequential_scripts_team(&mut self, t: TeamH);
    /// `setSequentialTimer(obj, frames)`.
    fn sa_set_sequential_timer_obj(&mut self, o: ObjH, frames: i32);
    /// `setSequentialTimer(team, frames)`.
    fn sa_set_sequential_timer_team(&mut self, t: TeamH, frames: i32);
    /// `findWaypointFollowingCapableWeapon()` is not NULL.
    fn sa_obj_has_waypoint_following_weapon(&self, o: ObjH) -> bool;
    /// That weapon's `forceFireWeapon(obj, &pos)`: the projectile.
    fn sa_weapon_force_fire(&mut self, o: ObjH, p: &Coord3D) -> Option<ObjH>;

    // --- batch A7 ---
    /// `TheTerrainLogic->getWaterHandleByName(name)`.
    fn sa_water_handle(&self, name: &[u8]) -> Option<i32>;
    /// `TheTerrainLogic->setWaterHeight(water, height, damage, forcePathfindUpdate)`.
    fn sa_set_water_height(&mut self, w: i32, height: f32, damage: f32, force: bool);
    /// `TheTerrainLogic->changeWaterHeightOverTime(water, height, time, damage)`.
    fn sa_change_water_height_over_time(&mut self, w: i32, height: f32, time: f32, damage: f32);
    /// `TheTerrainLogic->setActiveBoundary(border)`.
    fn sa_set_active_boundary(&mut self, border: i32);
    /// `ThePartitionManager->refreshShroudForLocalPlayer()`.
    fn sa_refresh_shroud_for_local_player(&mut self);
    /// `TheInGameUI->selectDrawable(obj->getDrawable())`.
    fn sa_ui_select_drawable(&mut self, o: ObjH);
    /// `TheTacticalView->getPosition(&pos)`.
    fn sa_view_position(&self) -> Coord3D;
    /// `TheTacticalView->shake(&pos, type)`.
    fn sa_view_shake(&mut self, p: &Coord3D, shake_type: i32);
    /// `getContain()->iterateContained(..)` (the contained objects, in order).
    fn sa_contained(&self, o: ObjH) -> Vec<ObjH>;
    /// `TheGameLogic->setBuildableStatusOverride(template, status)`.
    fn sa_logic_set_buildable_status_override(&mut self, t: TemplateH, status: i32);
    /// The first behavior module's `getCaveInterface()` that is not NULL.
    fn sa_obj_has_cave(&self, o: ObjH) -> bool;
    /// That cave's `tryToSetCaveIndex(index)`.
    fn sa_cave_try_set_index(&mut self, o: ObjH, index: i32);
    /// `findUpdateModule("SupplyWarehouseDockUpdate")` is not NULL.
    fn sa_obj_has_warehouse(&self, o: ObjH) -> bool;
    /// That module's `setCashValue(value)`.
    fn sa_warehouse_set_cash_value(&mut self, o: ObjH, value: i32);
    /// `TheAudio->setAudioEventEnabled(name, enable)`.
    fn sa_audio_set_event_enabled(&mut self, name: &[u8], enable: bool);
    /// `TheAudio->removeDisabledEvents()`.
    fn sa_audio_remove_disabled_events(&mut self);
    /// `TheAudio->removeAudioEvent(name)`.
    fn sa_audio_remove_event_named(&mut self, name: &[u8]);
    /// `TheAudio->setAudioEventVolumeOverride(name, volume)`.
    fn sa_audio_set_event_volume_override(&mut self, name: &[u8], volume: f32);
    /// `TheScriptEngine->setToppleDirection(name, &dir)`.
    fn sa_set_topple_direction(&mut self, name: &[u8], dir: &Coord3D);
    /// `TheRadar->forceOn(on)`.
    fn sa_radar_force_on(&mut self, on: bool);

    // --- batch A8 ---
    /// `ThePartitionManager->getClosestObject(&pos, REALLY_FAR, FROM_CENTER_2D, {the template, within the trigger, same map status
    /// as `obj`}, &dist)`: the object and the distance.
    fn sa_partition_closest_of_template(&self, p: &Coord3D, t: TemplateH, trig: TriggerH, obj: ObjH) -> Option<(ObjH, f32)>;
    /// `TheScriptEngine->getObjectTypes(name)`: the list's names.
    fn sa_object_types(&self, name: &[u8]) -> Option<Vec<Vec<u8>>>;
    /// `aiMoveToObject(target, cmdSource)` (`target` NULL: 0).
    fn sa_ai_move_to_object(&mut self, o: ObjH, target: Option<ObjH>, cmd: i32);
    /// `TheUpgradeCenter->findUpgrade(name)`.
    fn sa_find_upgrade(&self, name: &[u8]) -> Option<i32>;
    /// `giveUpgrade(upgrade)`.
    fn sa_obj_give_upgrade(&mut self, o: ObjH, upgrade: i32);
    /// `AIGroup::getCenter(&pos)` (`None`: an empty group, which leaves `pos` unset).
    fn sa_group_center(&self, g: GroupH) -> Option<Coord3D>;
    /// `ThePartitionManager->getNearestGroupWithValue(playerIndex, ALLOW_ENEMIES, VOT_CashValue, &src, value, greater, &out)`.
    fn sa_partition_nearest_group_with_value(&mut self, index: i32, src: &Coord3D, value: i32, greater: bool) -> Option<Coord3D>;
    /// `groupAttackMoveToPosition(&pos, maxShots, cmdSource)`.
    fn sa_group_attack_move_to_position(&mut self, g: GroupH, p: &Coord3D, max_shots: i32, cmd: i32);
    /// The button's `getSpecialPowerTemplate()->getID()` (`None`: no special power).
    fn sa_button_special_power_id(&self, b: ButtonH) -> Option<u32>;
    /// `AIGroup::getSpecialPowerSourceObject(id)`.
    fn sa_group_special_power_source(&self, g: GroupH, id: u32) -> Option<ObjH>;
    /// `AIGroup::getCommandButtonSourceObject(type)`.
    fn sa_group_command_button_source(&self, g: GroupH, command_type: i32) -> Option<ObjH>;
    /// `iterateObjectsInRange(&pos, range, FROM_CENTER_2D, {the player's enemies, valid targets of the button from `src`, same map
    /// status}, ITER_SORTED_EXPENSIVE_TO_CHEAP)`'s first object.
    fn sa_partition_most_valuable_target(&mut self, p: &Coord3D, range: f32) -> Option<ObjH>;

    // --- batch A9 ---
    /// `button->isValidToUseOn(src, target, NULL, cmdSource)`.
    fn sa_button_is_valid_to_use_on(&self, b: ButtonH, src: ObjH, target: Option<ObjH>, cmd: i32) -> bool;
    /// `ThePartitionManager->getClosestObject(&pos, REALLY_FAR, FROM_CENTER_2D, filters, &dist)` for the command-button searches: the
    /// filters after the player's affiliation, as `search` names them.
    fn sa_partition_closest_for_button(&self, p: &Coord3D, search: ButtonSearch) -> Option<(ObjH, f32)>;
    /// `groupDoCommandButtonAtObject(button, target, cmdSource)` with a target that may be NULL.
    fn sa_group_do_command_button_at_object_opt(&mut self, g: GroupH, b: ButtonH, target: Option<ObjH>, cmd: i32);

    // --- batch A10 ---
    /// `cameraModFreezeTime()` (`time`) or `cameraModFreezeAngle()`.
    fn sa_view_camera_mod_freeze(&mut self, time: bool);
    /// `cameraModFinalTimeMultiplier(m)`.
    fn sa_view_camera_mod_final_time_multiplier(&mut self, m: i32);
    /// `cameraModRollingAverage(n)`.
    fn sa_view_camera_mod_rolling_average(&mut self, n: i32);
    /// `setTimeMultiplier(m)`.
    fn sa_view_set_time_multiplier(&mut self, m: i32);
    /// `TheAudio->pauseAudio(affect)` (`pause`) or `resumeAudio(affect)`.
    fn sa_audio_pause(&mut self, pause: bool, affect: i32);
    /// `TheGlobalData->m_framesPerSecondLimit`.
    fn sa_global_fps_limit(&self) -> i32;
    /// `TheGameEngine->setFramesPerSecondLimit(fps)`.
    fn sa_engine_set_fps_limit(&mut self, fps: i32);
    /// `TheWritableGlobalData->m_useFpsLimit = use`.
    fn sa_set_use_fps_limit(&mut self, use_limit: bool);
    /// `addSkillPoints(delta)`.
    fn sa_player_add_skill_points(&mut self, p: PlayerH, delta: i32);
    /// `getRankLevel()`.
    fn sa_player_rank_level(&self, p: PlayerH) -> i32;
    /// `setRankLevel(level)`.
    fn sa_player_set_rank_level(&mut self, p: PlayerH, level: i32);
    /// `TheGameLogic->setRankLevelLimit(level)`.
    fn sa_logic_set_rank_level_limit(&mut self, level: i32);
    /// `TheScienceStore->getScienceFromInternalName(name)` (`None`: `SCIENCE_INVALID`).
    fn sa_science_from_name(&self, name: &[u8]) -> Option<i32>;
    /// `grantScience(science)`.
    fn sa_player_grant_science(&mut self, p: PlayerH, science: i32);
    /// `attemptToPurchaseScience(science)`.
    fn sa_player_purchase_science(&mut self, p: PlayerH, science: i32);
    /// `getScienceAvailabilityTypeFromString(name)` (`None`: `SCIENCE_AVAILABILITY_INVALID`).
    fn sa_player_science_availability_from_string(&self, p: PlayerH, name: &[u8]) -> Option<i32>;
    /// `setScienceAvailability(science, availability)`.
    fn sa_player_set_science_availability(&mut self, p: PlayerH, science: i32, availability: i32);
    /// `groupSetEmoticon(name, frames)`.
    fn sa_group_set_emoticon(&mut self, g: GroupH, name: &[u8], frames: i32);
    /// `getDrawable()->setEmoticon(name, frames)`.
    fn sa_drawable_set_emoticon(&mut self, o: ObjH, name: &[u8], frames: i32);
    /// `TheScriptEngine->doObjectTypeListMaintenance(list, type, add)`.
    fn sa_object_type_list_maintenance(&mut self, list: &[u8], object_type: &[u8], add: bool);
    /// `createNamedMapReveal(look, waypoint, radius, player)` then `doNamedMapReveal(look)`.
    fn sa_named_map_reveal(&mut self, look: &[u8], waypoint: &[u8], radius: f32, player: &[u8]);
    /// `undoNamedMapReveal(look)` then `removeNamedMapReveal(look)`.
    fn sa_undo_named_map_reveal(&mut self, look: &[u8]);
    /// `TheEva->setEvaEnabled(enabled)`.
    fn sa_eva_set_enabled(&mut self, enabled: bool);
    /// `TheGameLogic->setShowBehindBuildingMarkers(b)` (0), `setDrawIconUI(b)` (1) or `setShowDynamicLOD(b)` (2).
    fn sa_logic_set_option(&mut self, which: i32, enabled: bool);
    /// `TheGameLogic->setHulkMaxLifetimeOverride(frames)`.
    fn sa_logic_set_hulk_max_lifetime_override(&mut self, frames: i32);
    /// `cameraEnableSlaveMode(template, bone)` (`Some`) or `cameraDisableSlaveMode()`.
    fn sa_view_camera_slave_mode(&mut self, slave: Option<(&[u8], &[u8])>);
    /// `Add_Camera_Shake(pos, radius, duration, amplitude)`.
    fn sa_view_add_camera_shake(&mut self, p: &Coord3D, radius: f32, duration: f32, amplitude: f32);
    /// `aiFaceObject(target, cmdSource)`.
    fn sa_ai_face_object(&mut self, o: ObjH, target: ObjH, cmd: i32);
    /// `aiFacePosition(&pos, cmdSource)`.
    fn sa_ai_face_position(&mut self, o: ObjH, p: &Coord3D, cmd: i32);
    /// `friend_setSkillset(skillset)`.
    fn sa_player_set_skillset(&mut self, p: PlayerH, skillset: i32);
    /// `guardSupplyCenter(team, minSupplies)`.
    fn sa_player_guard_supply_center(&mut self, p: PlayerH, t: TeamH, supplies: i32);
    /// `aiGuardTunnelNetwork(mode, cmdSource)`.
    fn sa_ai_guard_tunnel_network(&mut self, o: ObjH, mode: i32, cmd: i32);
    /// `friend_getCommandSetString()` of a template.
    fn sa_template_command_set_string(&self, t: TemplateH) -> Vec<u8>;
    /// `TheGameLogic->setControlBarOverride(commandSet, slot, button)`.
    fn sa_logic_set_control_bar_override(&mut self, command_set: &[u8], slot: i32, b: Option<ButtonH>);
    /// `setSkillPointsModifier(m)`.
    fn sa_player_set_skill_points_modifier(&mut self, p: PlayerH, m: f32);
    /// `TheTacticalView->setGuardBandBias(&gb)`.
    fn sa_view_set_guard_band_bias(&mut self, x: f32, y: f32);
    /// Every object, `getFirstObject` then `getNextObject`.
    fn sa_all_objects(&self) -> Vec<ObjH>;
    /// `setReceivingDifficultyBonus(b)`.
    fn sa_obj_set_receiving_difficulty_bonus(&mut self, o: ObjH, b: bool);
    /// `TheScriptEngine->setObjectsShouldReceiveDifficultyBonus(b)`.
    fn sa_set_objects_should_receive_difficulty_bonus(&mut self, b: bool);
    /// `TheScriptEngine->setChooseVictimAlwaysUsesNormal(b)`.
    fn sa_set_choose_victim_always_uses_normal(&mut self, b: bool);
    /// `findUpdateModule("RailroadBehavior")` is not NULL.
    fn sa_obj_has_railroad(&self, o: ObjH) -> bool;
    /// That module's `setHeld(held)`.
    fn sa_railroad_set_held(&mut self, o: ObjH, held: bool);
}

type R<T> = Result<T, ActionError>;

/// `pAction->getParameter(i)`, dereferenced.
fn parm(a: &ScriptAction, i: i32) -> R<&Parameter> {
    a.get_parameter(i).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullParameter)))
}
fn ps(a: &ScriptAction, i: i32) -> R<Vec<u8>> {
    Ok(parm(a, i)?.string.clone())
}
fn pr(a: &ScriptAction, i: i32) -> R<f32> {
    Ok(parm(a, i)?.real)
}
fn pi(a: &ScriptAction, i: i32) -> R<i32> {
    Ok(parm(a, i)?.int)
}

/// `Real` to `Int` (truncation); a value an `Int` cannot hold is undefined.
fn real_to_int(v: f32) -> R<i32> {
    // (NaN is in no range)
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&v) {
        return Err(IniError::undefined(Undefined::FloatToIntOutOfRange).into());
    }
    Ok(v as i32)
}

/// `RGBColor::getAsInt()` (each channel times `255.0`, a `double`, truncated).
pub fn rgb_as_int(red: f32, green: f32, blue: f32) -> i32 {
    ((f64::from(red) * 255.0) as i32) << 16 | ((f64::from(green) * 255.0) as i32) << 8 | (f64::from(blue) * 255.0) as i32
}

/// `atoi`: optional blanks and sign, then digits; a value an `Int` cannot hold is undefined.
fn atoi(s: &[u8]) -> R<i32> {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || (9..=13).contains(&s[i])) {
        i += 1;
    }
    let neg = i < s.len() && s[i] == b'-';
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        i += 1;
    }
    let mut v: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v * 10 + i64::from(s[i] - b'0');
        if v > i64::from(i32::MAX) + 1 {
            return Err(IniError::undefined(Undefined::IntOutOfRange).into());
        }
        i += 1;
    }
    let v = if neg { -v } else { v };
    i32::try_from(v).map_err(|_| IniError::undefined(Undefined::IntOutOfRange).into())
}

/// `MAKE_KINDOF_MASK(bit)` of a script's `Int`: a bit outside the kinds makes `std::bitset::set` throw `std::out_of_range`, which is
/// not compared (reported as out of range).
fn kind_of_bit(k: i32) -> R<()> {
    if (0..crate::common::kind_of::KINDOF_COUNT).contains(&k) {
        Ok(())
    } else {
        Err(IniError::undefined(Undefined::ArrayIndexOutOfRange).into())
    }
}

fn null_local_player() -> ActionError {
    IniError::undefined(Undefined::NullLocalPlayer).into()
}

/// `class ScriptActions` (`m_messageWindow` is a static upstream: there is one `ScriptActions`).
#[derive(Clone, Debug, Default)]
pub struct ScriptActions {
    /// `m_suppressNewWindows`.
    pub suppress_new_windows: bool,
    /// `m_unnamedUnit` (the empty string).
    pub unnamed_unit: Vec<u8>,
    /// `m_messageWindow`.
    pub message_window: Option<WindowH>,
}

impl ScriptActions {
    /// `ScriptActions()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `init()`: `reset()`.
    pub fn init<H: ScriptActionsHost>(&mut self, h: &mut H) {
        self.reset(h);
    }

    /// `reset()`: new windows allowed, the victory or defeat window closed.
    pub fn reset<H: ScriptActionsHost>(&mut self, h: &mut H) {
        self.suppress_new_windows = false;
        self.close_windows(h, false);
    }

    /// `update()`: nothing.
    pub fn update(&mut self) {}

    /// `closeWindows(suppressNewWindows)`.
    pub fn close_windows<H: ScriptActionsHost>(&mut self, h: &mut H, suppress_new_windows: bool) {
        self.suppress_new_windows = suppress_new_windows;
        if let Some(w) = self.message_window.take() {
            h.sa_win_destroy(w);
        }
    }

    /// `doQuickVictory()`.
    pub fn do_quick_victory<H: ScriptActionsHost>(&mut self, h: &mut H) {
        self.close_windows(h, false);
        h.sa_logic_close_windows();
        self.do_disable_input(h);
        if h.sa_has_campaign_manager() {
            h.sa_campaign_set_victorious(true);
        }
        h.sa_start_quick_end_game_timer();
    }

    /// `doSetInfantryLightingOverride(setting)`.
    pub fn do_set_infantry_lighting_override<H: ScriptActionsHost>(&mut self, h: &mut H, setting: f32) {
        h.sa_set_infantry_light_scale(setting);
    }

    /// `doVictory()` and `doDefeat()` (`victorious`): the window (the observer's quit window for an observer, or after the local
    /// defeat window was shown), the campaign, the end game timer.
    fn do_victory_or_defeat<H: ScriptActionsHost>(&mut self, h: &mut H, victorious: bool) -> R<()> {
        self.close_windows(h, false);
        h.sa_logic_close_windows();
        self.do_disable_input(h);
        if !self.suppress_new_windows {
            let local = h.sa_local_player().ok_or_else(null_local_player)?;
            let show_observer_window = h.sa_player_is_observer(local) || (h.sa_has_shown_mp_local_defeat_window() && !plant(3866));
            let name: &[u8] = if show_observer_window {
                b"Menus/ObserverQuit.wnd"
            } else if victorious {
                b"Menus/Victorious.wnd"
            } else {
                b"Menus/Defeat.wnd"
            };
            self.message_window = Some(h.sa_win_create_from_script(name));
        }
        if h.sa_has_campaign_manager() {
            h.sa_campaign_set_victorious(victorious);
        }
        h.sa_start_end_game_timer();
        Ok(())
    }

    /// `doLocalDefeat()`.
    pub fn do_local_defeat<H: ScriptActionsHost>(&mut self, h: &mut H) {
        h.sa_mark_mp_local_defeat_window_shown();
        self.close_windows(h, false);
        h.sa_logic_close_windows();
        if !self.suppress_new_windows && !h.sa_am_i_observer() {
            self.message_window = Some(h.sa_win_create_from_script(b"Menus/LocalDefeat.wnd"));
        }
        if h.sa_has_campaign_manager() {
            h.sa_campaign_set_victorious(false);
        }
        h.sa_start_close_window_timer();
    }

    /// `changeObjectPanelFlagForSingleObject(obj, flag, newVal)`.
    pub fn change_object_panel_flag_for_single_object<H: ScriptActionsHost>(&mut self, h: &mut H, o: ObjH, flag: &[u8], new_val: bool) {
        let n = &OBJECT_FLAGS_NAMES;
        if flag == n[0] {
            h.sa_obj_set_script_status(o, script_status::DISABLED, !new_val);
        } else if flag == n[1] {
            h.sa_obj_set_script_status(o, script_status::UNPOWERED, !new_val);
        } else if flag == n[2] {
            if h.sa_obj_has_body(o) {
                h.sa_body_set_indestructible(o, new_val);
            }
        } else if flag == n[3] {
            h.sa_obj_set_script_status(o, script_status::UNSELLABLE, new_val != plant(3867));
        } else if flag == n[4] {
            if h.sa_obj_is_selectable(o) != new_val {
                h.sa_obj_set_selectable(o, new_val);
            }
        } else if flag == n[5] {
            if h.sa_obj_has_ai(o) {
                h.sa_ai_set_is_recruitable(o, new_val);
            }
        } else if flag == n[6] {
            h.sa_obj_set_script_status(o, script_status::TARGETABLE, new_val);
        }
    }

    /// `doPlaySoundEffect(sound)` and `doPlaySoundEffectAt(sound, waypoint)`: logical audio for the local player.
    fn do_play_sound_effect<H: ScriptActionsHost>(&mut self, h: &mut H, sound: &[u8], waypoint: Option<&[u8]>) -> R<()> {
        let position = match waypoint {
            None => None,
            Some(wp) => match h.sa_waypoint_by_name(wp) {
                Some(w) => Some(h.sa_waypoint_location(w)),
                None => return Ok(()),
            },
        };
        let local = h.sa_local_player().ok_or_else(null_local_player)?;
        let e = AudioEvent { name: sound.to_vec(), position, logical: true, player_index: local, object: 0, uninterruptable: false, should_fade: false };
        h.sa_audio_add_event(&e);
        Ok(())
    }

    /// `doEnableObjectSound(objectName, enable)`.
    pub fn do_enable_object_sound<H: ScriptActionsHost>(&mut self, h: &mut H, name: &[u8], enable: bool) {
        let Some(o) = h.sa_unit_named(name) else { return };
        if !h.sa_obj_has_drawable(o) {
            return;
        }
        h.sa_drawable_enable_ambient_sound(o, enable);
    }

    /// `doMoveToWaypoint(team, waypoint)`: the team as a group, moved to the waypoint.
    pub fn do_move_to_waypoint<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], waypoint: &[u8]) {
        let Some(t) = h.sa_team_named(team) else { return };
        let Some(g) = h.sa_create_group() else { return };
        h.sa_team_as_ai_group(t, g);
        if let Some(w) = h.sa_waypoint_by_name(waypoint) {
            let destination = h.sa_waypoint_location(w);
            h.sa_group_move_to_position(g, &destination, false, CMD_FROM_SCRIPT);
        }
    }

    /// `doNamedMoveToWaypoint(unit, waypoint)`.
    pub fn do_named_move_to_waypoint<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8], waypoint: &[u8]) {
        let Some(o) = h.sa_unit_named(unit) else { return };
        let Some(w) = h.sa_waypoint_by_name(waypoint) else { return };
        let destination = h.sa_waypoint_location(w);
        if !h.sa_obj_has_ai(o) {
            return;
        }
        h.sa_ai_clear_waypoint_queue(o);
        h.sa_obj_leave_group(o);
        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
        h.sa_ai_move_to_position(o, &destination, CMD_FROM_SCRIPT);
    }

    /// `doCameraFollowNamed(unit, snapToUnit)`.
    pub fn do_camera_follow_named<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8], snap: bool) {
        if let Some(o) = h.sa_unit_named(unit) {
            h.sa_view_set_camera_lock(o);
            if snap {
                h.sa_view_snap_to_camera_lock();
            }
            h.sa_view_set_snap_mode(LOCK_FOLLOW, 0.0);
        }
    }

    /// `doCreateReinforcements(team, waypoint)`. See the module docs.
    pub fn do_create_reinforcements<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], waypoint: &[u8]) -> R<()> {
        let proto = h.sa_find_team_prototype(team);
        let mut need_to_move = false;
        let Some(way) = h.sa_waypoint_by_name(waypoint) else { return Ok(()) };
        let destination = h.sa_waypoint_location(way);
        let Some(proto) = proto else { return Ok(()) };
        let info = h.sa_proto_template_info(proto);
        let mut origin = destination;
        if let Some(w) = h.sa_waypoint_by_name(&info.start_reinforce_waypoint) {
            origin = h.sa_waypoint_location(w);
            if origin.x != destination.x || origin.y != destination.y {
                need_to_move = true;
            }
        }
        let Some(the_team) = h.sa_create_inactive_team(team) else { return Ok(()) };
        let null_object = || ActionError::from(IniError::undefined(Undefined::NullObject));
        // the transport first, so that its paradrop capability is known
        let transport_template = h.sa_find_template(&info.transport_unit_type);
        let mut transport = None;
        if let Some(tt) = transport_template {
            let tr = h.sa_new_object(tt, Some(the_team)).ok_or_else(null_object)?;
            h.sa_obj_set_position(tr, &origin);
            h.sa_obj_set_orientation(tr, 0.0);
            transport = Some(tr);
        }
        let mut transport_count = 1i32;
        let dp = transport.filter(|t| h.sa_obj_has_deliver_payload(*t));
        let put_in_container_template = dp.and_then(|t| h.sa_deliver_payload_put_in_container_template(t));
        // the units of the team
        for (thing, max_units) in &info.units_info {
            let unit_template = h.sa_find_template(thing);
            let mut pos = origin;
            if let Some(ut) = unit_template {
                let mut obj = None;
                for j in 0..*max_units {
                    obj = h.sa_new_object(ut, Some(the_team));
                    if let Some(o) = obj {
                        // (2.25 is a double)
                        pos.x = (f64::from(origin.x) + 2.25 * f64::from(j) * f64::from(h.sa_obj_major_radius(o))) as f32;
                        pos.z = h.sa_ground_height(pos.x, pos.y);
                        h.sa_obj_set_position(o, &pos);
                        h.sa_obj_set_orientation(o, 0.0);
                    }
                }
                if let Some(o) = obj {
                    pos.y += if plant(3865) { 2.25 } else { 2.0 } * h.sa_obj_major_radius(o);
                }
            }
            origin.y = pos.y;
        }
        origin = destination;
        if info.team_starts_full {
            // load into the team's own transports
            let mut units = Vec::new();
            let mut transports = Vec::new();
            for o in h.sa_team_members(the_team) {
                if Some(o) == transport {
                    continue;
                }
                if h.sa_obj_is_kind_of(o, KINDOF_TRANSPORT) {
                    if h.sa_obj_has_contain(o) {
                        transports.push((o, h.sa_contain_max(o)));
                    }
                } else {
                    let mut slots = h.sa_obj_transport_slot_count(o);
                    if slots == 0 {
                        slots = 0x7f_ffff; // 0 means lots.
                    }
                    units.push((o, slots as u32));
                }
            }
            let mut partition = PartitionSolver::new(&units, &transports, SolutionType::PreferFastSolution);
            partition.solve();
            for (u, t) in partition.get_solution().clone() {
                let (Some(unit), Some(trans)) = (h.sa_find_object_by_id(u), h.sa_find_object_by_id(t)) else { continue };
                if h.sa_obj_has_contain(trans) {
                    h.sa_contain_add(trans, unit);
                }
            }
        }
        let mut contain = transport.filter(|t| h.sa_obj_has_contain(*t));
        if contain.is_some() {
            for o in h.sa_team_members(the_team) {
                let tr = transport.ok_or_else(null_object)?;
                let ot = h.sa_obj_template(o).ok_or_else(null_object)?;
                if h.sa_template_is_equivalent_to(ot, h.sa_obj_template(tr)) {
                    continue; // it's our transport.
                }
                if h.sa_obj_is_contained(o) {
                    continue;
                }
                let mut pos = origin;
                pos.x += transport_count as f32 * h.sa_obj_major_radius(tr);
                pos.z = h.sa_ground_height(pos.x, pos.y);
                let Some(c) = contain else { continue };
                if h.sa_contain_is_valid_container_for(c, o, false) {
                    let mut c = c;
                    let mut obj = o;
                    if !h.sa_contain_is_valid_container_for(c, obj, true) {
                        // full, try building another.
                        let tt = transport_template.ok_or_else(null_object)?;
                        let tr2 = h.sa_new_object(tt, Some(the_team)).ok_or_else(null_object)?;
                        h.sa_obj_set_position(tr2, &pos);
                        transport_count += 1;
                        h.sa_obj_set_orientation(tr2, 0.0);
                        transport = Some(tr2);
                        contain = h.sa_obj_has_contain(tr2).then_some(tr2);
                        c = contain.ok_or_else(|| ActionError::from(IniError::undefined(Undefined::NullContainModule)))?;
                    }
                    if let Some(pct) = put_in_container_template {
                        let container = h.sa_new_object(pct, Some(the_team)).ok_or_else(null_object)?;
                        h.sa_obj_set_position(container, &pos);
                        if h.sa_obj_has_contain(container) && h.sa_contain_is_valid_container_for(container, obj, true) {
                            h.sa_contain_add(container, obj);
                            obj = container;
                        }
                    }
                    h.sa_contain_add(c, obj);
                }
            }
        }
        if let Some(tr) = transport {
            for o in h.sa_team_members(the_team) {
                let has_ai = h.sa_obj_has_ai(o);
                let ot = h.sa_obj_template(o).ok_or_else(null_object)?;
                if h.sa_template_is_equivalent_to(ot, h.sa_obj_template(tr)) {
                    if let Some(d) = dp {
                        h.sa_deliver_payload(d, &destination);
                    } else if has_ai {
                        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
                        if info.transports_exit != plant(3861) {
                            h.sa_ai_move_to_and_evacuate_and_exit(o, &destination, CMD_FROM_SCRIPT);
                        } else {
                            h.sa_ai_move_to_and_evacuate(o, &destination, CMD_FROM_SCRIPT);
                        }
                    }
                } else if !h.sa_obj_is_disabled_by_type(o, DISABLED_HELD) && has_ai {
                    // units that are not transported move to the goal.
                    h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
                    h.sa_ai_move_to_position(o, &destination, CMD_FROM_SCRIPT);
                }
            }
        } else {
            h.sa_team_set_active(the_team);
            if need_to_move && !plant(3862) {
                let Some(g) = h.sa_create_group() else { return Ok(()) };
                h.sa_team_as_ai_group(the_team, g);
                h.sa_group_move_to_position(g, &destination, false, CMD_FROM_SCRIPT);
            }
        }
        Ok(())
    }

    /// The first waypoint (in the terrain logic's order) with the name.
    fn waypoint_named_in_order<H: ScriptActionsHost>(h: &H, name: &[u8]) -> Option<WaypointH> {
        let mut way = h.sa_first_waypoint();
        while let Some(w) = way {
            if h.sa_waypoint_name(w) == name {
                return Some(w);
            }
            way = h.sa_waypoint_next(w);
        }
        None
    }

    /// `doMoveCameraTo(waypoint, sec, cameraStutterSec, easeIn, easeOut)`.
    pub fn do_move_camera_to<H: ScriptActionsHost>(&mut self, h: &mut H, waypoint: &[u8], sec: f32, stutter: f32, ease_in: f32, ease_out: f32) -> R<()> {
        if let Some(w) = Self::waypoint_named_in_order(h, waypoint) {
            let destination = h.sa_waypoint_location(w);
            let ms = if plant(3877) { (sec * 1000.0).round() } else { sec * 1000.0 };
            h.sa_view_move_camera_to(&destination, real_to_int(ms)?, real_to_int(stutter * 1000.0)?, true, ease_in * 1000.0, ease_out * 1000.0);
        }
        Ok(())
    }

    /// `doOversizeTheTerrain(amount)`: the terrain oversized, and the view nudged there and back.
    pub fn do_oversize_the_terrain<H: ScriptActionsHost>(&mut self, h: &mut H, amount: i32) {
        h.sa_oversize_the_terrain(amount);
        h.sa_view_scroll_by(&Coord2D { x: 0.0001, y: 0.0001 });
        h.sa_view_scroll_by(&Coord2D { x: -0.0001, y: -0.0001 });
    }

    /// `doSetupCamera(waypoint, zoom, pitch, lookAtWaypoint)`.
    pub fn do_setup_camera<H: ScriptActionsHost>(&mut self, h: &mut H, waypoint: &[u8], zoom: f32, pitch: f32, look_at: &[u8]) {
        let Some(w) = h.sa_waypoint_by_name(waypoint) else { return };
        let pos = h.sa_waypoint_location(w);
        let Some(l) = h.sa_waypoint_by_name(look_at) else { return };
        let destination = h.sa_waypoint_location(l);
        h.sa_view_move_camera_to(&pos, 0, 0, true, 0.0, 0.0);
        h.sa_view_camera_mod_look_toward(&destination);
        h.sa_view_camera_mod_final_pitch(pitch, 0.0, 0.0);
        h.sa_view_camera_mod_final_zoom(zoom, 0.0, 0.0);
    }

    /// `doModCameraMoveToSelection()`: the camera's final position the mean of the selected drawables'.
    pub fn do_mod_camera_move_to_selection<H: ScriptActionsHost>(&mut self, h: &mut H) {
        let mut count = 0i32;
        let mut d = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        for (selected, pos) in h.sa_drawables() {
            if selected {
                d.x += pos.x;
                d.y += pos.y;
                d.z += pos.z;
                count += 1;
            }
        }
        if count != 0 {
            if !plant(3868) {
                d.z /= count as f32;
            }
            d.x /= count as f32;
            d.y /= count as f32;
            h.sa_view_camera_mod_final_move_to(&d);
        }
    }

    /// `doCreateObject(objectName, thingName, teamName, pos, angle)`.
    pub fn do_create_object<H: ScriptActionsHost>(&mut self, h: &mut H, name: &[u8], thing: &[u8], team: &[u8], pos: &Coord3D, angle: f32) {
        let mut old = None;
        if name != self.unnamed_unit.as_slice() {
            old = h.sa_unit_named(name);
            if let Some(o) = old {
                if !h.sa_obj_is_effectively_dead(o) {
                    let mut msg = b"WARNING - Object with name ".to_vec();
                    msg.extend_from_slice(name);
                    msg.extend_from_slice(b" already exists. Failed Create.");
                    h.sa_append_debug_message(&msg, false);
                    return;
                }
            }
        }
        let Some(t) = self.team_or_created(h, team) else { return };
        let Some(tmpl) = h.sa_find_template(thing) else { return };
        if let Some(o) = h.sa_new_object(tmpl, Some(t)) {
            if name != self.unnamed_unit.as_slice() {
                self.name_new_object(h, o, name, old.is_some());
            }
            h.sa_obj_set_orientation(o, angle);
            h.sa_obj_set_position(o, pos);
            if h.sa_obj_is_kind_of(o, KINDOF_BLAST_CRATER) {
                // since these footprints are permanent
                h.sa_create_crater_in_terrain(o);
                h.sa_pathfinder_add_object(o);
            }
        }
    }

    /// The team by name, created when it is missing; a debug message when there is none.
    fn team_or_created<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8]) -> Option<TeamH> {
        let mut t = h.sa_team_named(team);
        if t.is_none() {
            t = h.sa_create_team(team);
        }
        if t.is_none() {
            h.sa_append_debug_message(b"***WARNING - Team not found:***", false);
            h.sa_append_debug_message(team, true);
        }
        t
    }

    /// A new object's name: given over from the unit that had it (or that existed), else cached.
    fn name_new_object<H: ScriptActionsHost>(&mut self, h: &mut H, o: ObjH, name: &[u8], had_old: bool) {
        h.sa_obj_set_name(o, name);
        if had_old || (h.sa_did_unit_exist(name) && !plant(3869)) {
            h.sa_transfer_object_name(name, o);
        } else {
            h.sa_add_object_to_cache(o);
        }
    }

    /// `doAttack(attackerName, victimName)`: the attacking team as a group, attacking the victim team.
    pub fn do_attack<H: ScriptActionsHost>(&mut self, h: &mut H, attacker: &[u8], victim: &[u8]) {
        let a = h.sa_team_named(attacker);
        let v = h.sa_team_named(victim);
        let (Some(a), Some(v)) = (a, v) else { return };
        let Some(g) = h.sa_create_group() else { return };
        h.sa_team_as_ai_group(a, g);
        h.sa_group_attack_team(g, v, NO_MAX_SHOTS_LIMIT, CMD_FROM_SCRIPT);
    }

    /// `doNamedAttack(attackerName, victimName)`.
    pub fn do_named_attack<H: ScriptActionsHost>(&mut self, h: &mut H, attacker: &[u8], victim: &[u8]) {
        let a = h.sa_unit_named(attacker);
        let v = h.sa_unit_named(victim);
        let (Some(a), Some(v)) = (a, v) else { return };
        if h.sa_obj_has_ai(a) {
            h.sa_obj_leave_group(a);
            h.sa_ai_choose_locomotor_set(a, LOCOMOTORSET_NORMAL);
            h.sa_ai_force_attack_object(a, v, NO_MAX_SHOTS_LIMIT, CMD_FROM_SCRIPT);
        }
    }

    /// `createUnitOnTeamAt(unitName, objType, teamName, waypoint)`.
    pub fn create_unit_on_team_at<H: ScriptActionsHost>(&mut self, h: &mut H, name: &[u8], obj_type: &[u8], team: &[u8], waypoint: &[u8]) {
        let old = h.sa_unit_named(name);
        if let Some(o) = old {
            if !h.sa_obj_is_effectively_dead(o) {
                let mut msg = b"WARNING - Object with name ".to_vec();
                msg.extend_from_slice(name);
                msg.extend_from_slice(b" already exists. Failed Create.");
                h.sa_append_debug_message(&msg, false);
                return;
            }
        }
        let Some(t) = self.team_or_created(h, team) else { return };
        let Some(tmpl) = h.sa_find_template(obj_type) else { return };
        if let Some(o) = h.sa_new_object(tmpl, Some(t)) {
            if name != self.unnamed_unit.as_slice() {
                self.name_new_object(h, o, name, old.is_some());
            }
            if let Some(w) = h.sa_waypoint_by_name(waypoint) {
                let destination = h.sa_waypoint_location(w);
                h.sa_obj_set_position(o, &destination);
            }
        }
    }

    /// `doDisableInput()` (the release build's: always).
    pub fn do_disable_input<H: ScriptActionsHost>(&mut self, h: &mut H) {
        h.sa_ui_set_input_enabled(false);
        h.sa_mouse_set_visibility(false);
        h.sa_ui_deselect_all_drawables();
        h.sa_ui_clear_attack_move_to_mode();
        h.sa_ui_set_waypoint_mode(false);
        h.sa_control_bar_delete_build_tooltip_layout();
        h.sa_look_at_reset_modes();
    }

    // ------------------------------------------------------------------------------------------------------------------------
    // batch A2: lines 1225-2300
    // ------------------------------------------------------------------------------------------------------------------------

    /// `updateNamedAttackPrioritySet(unitName, set)`.
    pub fn update_named_attack_priority_set<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8], set: &[u8]) {
        let Some(o) = h.sa_unit_named(unit) else { return };
        if !h.sa_obj_has_ai(o) {
            return;
        }
        let info = h.sa_attack_info(set);
        h.sa_ai_set_attack_info(o, &info);
    }

    /// `updateTeamAttackPrioritySet(teamName, set)`: the team's name for the set (unless the default), each member's AI given it.
    pub fn update_team_attack_priority_set<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], set: &[u8]) {
        let Some(t) = h.sa_team_named(team) else { return };
        let info = h.sa_attack_info(set);
        if !info.is_empty() {
            h.sa_team_set_attack_priority_name(t, &info);
        }
        for o in h.sa_team_members(t) {
            if h.sa_obj_has_ai(o) {
                h.sa_ai_set_attack_info(o, &info);
            }
        }
    }

    /// The team as a new group (`TheAI->createGroup()`, `getTeamAsAIGroup`); `None` without a group.
    fn team_group<H: ScriptActionsHost>(h: &mut H, t: TeamH) -> Option<GroupH> {
        let g = h.sa_create_group()?;
        h.sa_team_as_ai_group(t, g);
        Some(g)
    }

    /// The unit's AI made ready for a scripted order (`leaveGroup`, `chooseLocomotorSet(LOCOMOTORSET_NORMAL)`): `false` without one.
    fn ready_unit<H: ScriptActionsHost>(h: &mut H, o: ObjH) -> bool {
        if !h.sa_obj_has_ai(o) {
            return false;
        }
        h.sa_obj_leave_group(o);
        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
        true
    }

    /// `doLoadAllTransports(teamName)`: the team's units partitioned among its transports, each told to enter its own.
    pub fn do_load_all_transports<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8]) {
        let Some(t) = h.sa_team_named(team) else { return };
        let mut units = Vec::new();
        let mut transports = Vec::new();
        for o in h.sa_team_members(t) {
            if h.sa_obj_is_kind_of(o, KINDOF_TRANSPORT) {
                if h.sa_obj_has_contain(o) {
                    transports.push((o, h.sa_contain_max(o)));
                }
            } else {
                // (no "0 means lots" here, unlike the reinforcements)
                let s = h.sa_obj_transport_slot_count(o);
                units.push((o, if s == 0 && plant(3870) { 0x7f_ffff } else { s as u32 }));
            }
        }
        let mut partition = PartitionSolver::new(&units, &transports, SolutionType::PreferFastSolution);
        partition.solve();
        for (u, tr) in partition.get_solution().clone() {
            let (Some(unit), Some(trans)) = (h.sa_find_object_by_id(u), h.sa_find_object_by_id(tr)) else { continue };
            if h.sa_obj_has_ai(unit) {
                h.sa_ai_choose_locomotor_set(unit, LOCOMOTORSET_NORMAL);
                h.sa_ai_enter(unit, trans, CMD_FROM_SCRIPT);
            }
        }
    }

    /// `doNamedFollowWaypoints` (`exact`: `..Exact`): the path's waypoint closest to the unit.
    pub fn do_named_follow_waypoints<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8], label: &[u8], exact: bool) {
        let Some(o) = h.sa_unit_named(unit) else { return };
        let pos = h.sa_obj_position(o);
        if !h.sa_obj_has_ai(o) {
            return;
        }
        let Some(w) = h.sa_closest_waypoint_on_path(&pos, label) else { return };
        h.sa_obj_leave_group(o);
        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
        h.sa_ai_follow_waypoint_path(o, w, exact, CMD_FROM_SCRIPT);
    }

    /// The team's group and its members' mean position (`None`: no group, or an empty team, after the group was made).
    fn team_group_center<H: ScriptActionsHost>(h: &mut H, t: TeamH) -> Option<(GroupH, Coord3D, Option<ObjH>)> {
        let g = Self::team_group(h, t)?;
        let mut count = 0i32;
        let mut pos = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        let mut first = None;
        for o in h.sa_team_members(t) {
            let p = h.sa_obj_position(o);
            pos.x += p.x;
            pos.y += p.y;
            pos.z += p.z;
            count += 1;
            if first.is_none() {
                first = Some(o);
            }
        }
        if count == 0 {
            return None; // empty team.
        }
        pos.x /= count as f32;
        pos.y /= count as f32;
        pos.z /= count as f32;
        Some((g, pos, first))
    }

    /// The skirmish approach path's label: the label and the enemy's start position, from 1.
    fn skirmish_path_label<H: ScriptActionsHost>(h: &H, label: &[u8]) -> Option<Vec<u8>> {
        let enemy = h.sa_skirmish_enemy_player()?;
        let mp = h.sa_player_mp_start_index(enemy).wrapping_add(1);
        let mut l = label.to_vec();
        l.extend_from_slice(mp.to_string().as_bytes());
        Some(l)
    }

    /// `doTeamFollowSkirmishApproachPath(teamName, label, asTeam)`.
    pub fn do_team_follow_skirmish_approach_path<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], label: &[u8], as_team: bool) {
        let Some(t) = h.sa_team_named(team) else { return };
        let Some((g, pos, first)) = Self::team_group_center(h, t) else { return };
        let Some(path) = Self::skirmish_path_label(h, label) else { return };
        let Some(w) = h.sa_closest_waypoint_on_path(&pos, &path) else { return };
        if let (Some(ai_player), Some(f)) = (h.sa_current_player(), first) {
            h.sa_player_check_bridges(ai_player, f, w);
        }
        h.sa_group_follow_waypoint_path(g, w, as_team, false, CMD_FROM_SCRIPT);
    }

    /// `doTeamMoveToSkirmishApproachPath(teamName, label)`.
    pub fn do_team_move_to_skirmish_approach_path<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], label: &[u8]) {
        let Some(t) = h.sa_team_named(team) else { return };
        let Some((g, pos, _)) = Self::team_group_center(h, t) else { return };
        let Some(path) = Self::skirmish_path_label(h, label) else { return };
        let Some(w) = h.sa_closest_waypoint_on_path(&pos, &path) else { return };
        let l = h.sa_waypoint_location(w);
        h.sa_group_move_to_position(g, &l, false, CMD_FROM_SCRIPT);
    }

    /// `doTeamFollowWaypoints` (`exact`: `..Exact`)`(teamName, label, asTeam)`.
    pub fn do_team_follow_waypoints<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], label: &[u8], as_team: bool, exact: bool) {
        let Some(t) = h.sa_team_named(team) else { return };
        let Some((g, pos, _)) = Self::team_group_center(h, t) else { return };
        let Some(w) = h.sa_closest_waypoint_on_path(&pos, label) else { return };
        h.sa_group_follow_waypoint_path(g, w, as_team, exact, CMD_FROM_SCRIPT);
    }

    /// `doTeamHuntWithCommandButton(teamName, ability)`. See the module docs.
    pub fn do_team_hunt_with_command_button<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], ability: &[u8]) -> R<()> {
        use gui_command::*;
        let Some(t) = h.sa_team_named(team) else { return Ok(()) };
        let Some(button) = h.sa_find_command_button(ability) else { return Ok(()) };
        let cannot = |h: &mut H| {
            let mut msg = b"ERROR-Team hunt with command button - cannot hunt with ability ".to_vec();
            msg.extend_from_slice(ability);
            h.sa_append_debug_message(&msg, false);
        };
        match h.sa_button_command_type(button) {
            SPECIAL_POWER => {
                if !h.sa_button_has_special_power(button) {
                    return Ok(());
                }
                if h.sa_button_options(button) & COMMAND_OPTION_NEED_OBJECT_TARGET == 0 || plant(3863) {
                    cannot(h);
                    return Ok(());
                }
            }
            SWITCH_WEAPON | FIRE_WEAPON | HIJACK_VEHICLE | CONVERT_TO_CARBOMB | SABOTAGE_BUILDING => {}
            _ => {
                cannot(h);
                return Ok(());
            }
        }
        for o in h.sa_team_members(t) {
            if !h.sa_obj_has_ai(o) {
                continue;
            }
            let mut found = false;
            if let Some(set) = h.sa_find_command_set(&h.sa_obj_command_set_string(o)) {
                for i in 0..MAX_COMMANDS_PER_SET {
                    if h.sa_command_set_button(set, i) == Some(button) {
                        found = true;
                        break;
                    }
                }
            }
            let template_name = |h: &H| -> R<Vec<u8>> {
                let tt = h.sa_obj_template(o).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullObject)))?;
                Ok(h.sa_template_name(tt))
            };
            if !found {
                let mut msg = b"Error - Team hunt with command button - unit type '".to_vec();
                msg.extend_from_slice(&template_name(h)?);
                msg.extend_from_slice(if plant(3871) { b"' is not valid for " } else { b"' is not valid for ability " });
                msg.extend_from_slice(ability);
                h.sa_append_debug_message(&msg, false);
                continue;
            }
            match h.sa_button_command_type(button) {
                FIRE_WEAPON | SWITCH_WEAPON | SPECIAL_POWER | HIJACK_VEHICLE | CONVERT_TO_CARBOMB | SABOTAGE_BUILDING => {
                    if h.sa_obj_has_command_button_hunt(o) {
                        h.sa_command_button_hunt_set(o, ability);
                    } else {
                        let mut msg = b"Error - Team hunt with command button - unit type '".to_vec();
                        msg.extend_from_slice(&template_name(h)?);
                        msg.extend_from_slice(b"' requires CommandButtonHuntUpdate in .ini definition to hunt with ");
                        msg.extend_from_slice(ability);
                        h.sa_append_debug_message(&msg, false);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The batch-A2 cases of `executeAction` (`None`: not one of them).
    fn execute_a2<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> Option<R<()>> {
        use at::*;
        let r = (|| -> R<()> {
            match a.action_type {
                TEAM_FOLLOW_WAYPOINTS | TEAM_FOLLOW_WAYPOINTS_EXACT => {
                    let (t, l, as_team) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    self.do_team_follow_waypoints(h, &t, &l, as_team != 0, a.action_type == TEAM_FOLLOW_WAYPOINTS_EXACT);
                }
                NAMED_FOLLOW_WAYPOINTS | NAMED_FOLLOW_WAYPOINTS_EXACT => {
                    let (u, l) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_named_follow_waypoints(h, &u, &l, a.action_type == NAMED_FOLLOW_WAYPOINTS_EXACT);
                }
                SKIRMISH_FOLLOW_APPROACH_PATH => {
                    let (t, l, as_team) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    self.do_team_follow_skirmish_approach_path(h, &t, &l, as_team != 0);
                }
                SKIRMISH_MOVE_TO_APPROACH_PATH => {
                    let (t, l) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_team_move_to_skirmish_approach_path(h, &t, &l);
                }
                NAMED_APPLY_ATTACK_PRIORITY_SET => {
                    let (u, s) = (ps(a, 0)?, ps(a, 1)?);
                    self.update_named_attack_priority_set(h, &u, &s);
                }
                TEAM_APPLY_ATTACK_PRIORITY_SET => {
                    let (t, s) = (ps(a, 0)?, ps(a, 1)?);
                    self.update_team_attack_priority_set(h, &t, &s);
                }
                SET_BASE_CONSTRUCTION_SPEED => {
                    let (p, d) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(p) = h.sa_player_from_name(&p) {
                        h.sa_player_set_team_delay_seconds(p, d);
                    }
                }
                NAMED_SET_ATTITUDE => {
                    let (u, att) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        if h.sa_obj_has_ai(o) {
                            h.sa_ai_set_attitude(o, att);
                        }
                    }
                }
                TEAM_SET_ATTITUDE => {
                    let (t, att) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(t) = h.sa_team_named(&t) {
                        if let Some(g) = Self::team_group(h, t) {
                            h.sa_group_set_attitude(g, att);
                        }
                    }
                }
                NAMED_SET_REPULSOR => {
                    let (u, v) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_obj_set_status(o, OBJECT_STATUS_REPULSOR, v != 0);
                    }
                }
                TEAM_SET_REPULSOR => {
                    let (t, v) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(t) = h.sa_team_named(&t) {
                        for o in h.sa_team_members(t) {
                            h.sa_obj_set_status(o, OBJECT_STATUS_REPULSOR, v != 0);
                        }
                    }
                }
                NAMED_ATTACK_AREA => {
                    let (u, area) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    let Some(trig) = h.sa_trigger_named(&area) else { return Ok(()) };
                    if Self::ready_unit(h, o) {
                        h.sa_ai_attack_area(o, trig, CMD_FROM_SCRIPT);
                    }
                }
                NAMED_ATTACK_TEAM => {
                    let (u, t) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    if Self::ready_unit(h, o) {
                        h.sa_ai_attack_team(o, t, NO_MAX_SHOTS_LIMIT, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_ATTACK_AREA => {
                    let (t, area) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let Some(g) = Self::team_group(h, t) else { return Ok(()) };
                    if let Some(trig) = h.sa_trigger_named(&area) {
                        h.sa_group_attack_area(g, trig, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_ATTACK_NAMED => {
                    let (t, u) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let Some(v) = h.sa_unit_named(&u) else { return Ok(()) };
                    if let Some(g) = Self::team_group(h, t) {
                        h.sa_group_attack_object(g, v, NO_MAX_SHOTS_LIMIT, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_LOAD_TRANSPORTS => {
                    let t = ps(a, 0)?;
                    self.do_load_all_transports(h, &t);
                }
                NAMED_ENTER_NAMED => {
                    let (u, d) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    let Some(tr) = h.sa_unit_named(&d) else { return Ok(()) };
                    if Self::ready_unit(h, o) {
                        h.sa_ai_enter(o, tr, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_ENTER_NAMED => {
                    let (t, d) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let Some(tr) = h.sa_unit_named(&d) else { return Ok(()) };
                    // (the group is not checked)
                    let g = Self::team_group(h, t).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullObject)))?;
                    h.sa_group_enter(g, tr, CMD_FROM_SCRIPT);
                }
                NAMED_EXIT_ALL => {
                    let u = ps(a, 0)?;
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if Self::ready_unit(h, o) {
                        h.sa_ai_evacuate(o, false, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_EXIT_ALL => {
                    let t = ps(a, 0)?;
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let g = Self::team_group(h, t).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullObject)))?;
                    h.sa_group_evacuate(g, CMD_FROM_SCRIPT);
                }
                NAMED_SET_EVAC_LEFT_OR_RIGHT => {
                    let (u, d) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        if h.sa_obj_has_contain(o) {
                            h.sa_contain_set_evac_disposition(o, d as u32);
                        }
                    }
                }
                NAMED_GUARD => {
                    let u = ps(a, 0)?;
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if !h.sa_obj_has_ai(o) {
                        return Ok(());
                    }
                    h.sa_obj_leave_group(o);
                    let p = h.sa_obj_position(o);
                    h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
                    h.sa_ai_guard_position(o, &p, GUARDMODE_NORMAL, CMD_FROM_SCRIPT);
                }
                TEAM_GUARD => {
                    let t = ps(a, 0)?;
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    for o in h.sa_team_members(t) {
                        if h.sa_obj_has_ai(o) {
                            let p = h.sa_obj_position(o);
                            h.sa_ai_guard_position(o, &p, GUARDMODE_NORMAL, CMD_FROM_SCRIPT);
                        }
                    }
                }
                TEAM_GUARD_POSITION => {
                    let (t, w) = (ps(a, 0)?, ps(a, 1)?);
                    let way = h.sa_waypoint_by_name(&w);
                    let (Some(t), Some(way)) = (h.sa_team_named(&t), way) else { return Ok(()) };
                    if let Some(g) = Self::team_group(h, t) {
                        let p = h.sa_waypoint_location(way);
                        h.sa_group_guard_position(g, &p, GUARDMODE_NORMAL, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_GUARD_OBJECT => {
                    let (t, u) = (ps(a, 0)?, ps(a, 1)?);
                    let o = h.sa_unit_named(&u);
                    let (Some(t), Some(o)) = (h.sa_team_named(&t), o) else { return Ok(()) };
                    if let Some(g) = Self::team_group(h, t) {
                        h.sa_group_guard_object(g, o, GUARDMODE_NORMAL, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_GUARD_AREA => {
                    let (t, area) = (ps(a, 0)?, ps(a, 1)?);
                    let trig = h.sa_trigger_named(&area);
                    let (Some(t), Some(trig)) = (h.sa_team_named(&t), trig) else { return Ok(()) };
                    if let Some(g) = Self::team_group(h, t) {
                        h.sa_group_guard_area(g, trig, GUARDMODE_NORMAL, CMD_FROM_SCRIPT);
                    }
                }
                NAMED_HUNT => {
                    let u = ps(a, 0)?;
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if h.sa_obj_has_ai(o) {
                        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
                        h.sa_ai_hunt(o, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_HUNT => {
                    let t = ps(a, 0)?;
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    if let Some(g) = Self::team_group(h, t) {
                        h.sa_group_hunt(g, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_HUNT_WITH_COMMAND_BUTTON => {
                    let (t, ab) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_team_hunt_with_command_button(h, &t, &ab)?;
                }
                PLAYER_HUNT | PLAYER_SELL_EVERYTHING | PLAYER_DISABLE_BASE_CONSTRUCTION | PLAYER_DISABLE_UNIT_CONSTRUCTION
                | PLAYER_ENABLE_BASE_CONSTRUCTION | PLAYER_ENABLE_UNIT_CONSTRUCTION => {
                    let p = ps(a, 0)?;
                    let Some(p) = h.sa_player_from_name(&p) else { return Ok(()) };
                    match a.action_type {
                        PLAYER_HUNT => h.sa_player_set_units_should_hunt(p, true, CMD_FROM_SCRIPT),
                        PLAYER_SELL_EVERYTHING => h.sa_player_sell_everything(p),
                        PLAYER_DISABLE_BASE_CONSTRUCTION => h.sa_player_set_can_build_base(p, false),
                        PLAYER_DISABLE_UNIT_CONSTRUCTION => h.sa_player_set_can_build_units(p, false),
                        PLAYER_ENABLE_BASE_CONSTRUCTION => h.sa_player_set_can_build_base(p, true),
                        _ => h.sa_player_set_can_build_units(p, true),
                    }
                }
                PLAYER_DISABLE_FACTORIES | PLAYER_ENABLE_FACTORIES => {
                    let (p, o) = (ps(a, 0)?, ps(a, 1)?);
                    if let Some(p) = h.sa_player_from_name(&p) {
                        h.sa_player_set_objects_enabled(p, &o, a.action_type == PLAYER_ENABLE_FACTORIES);
                    }
                }
                PLAYER_REPAIR_NAMED_STRUCTURE => {
                    let (p, s) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(p) = h.sa_player_from_name(&p) else { return Ok(()) };
                    if let Some(o) = h.sa_unit_named(&s) {
                        h.sa_player_repair_structure(p, o);
                    }
                }
                CAMERA_MOVE_HOME => {}
                _ => return Err(ActionError::NotTranslated(a.action_type)),
            }
            Ok(())
        })();
        match r {
            Err(ActionError::NotTranslated(_)) => None,
            r => Some(r),
        }
    }

    // ------------------------------------------------------------------------------------------------------------------------
    // batch A3: lines 2300-2870
    // ------------------------------------------------------------------------------------------------------------------------

    /// `doTeamWander`, `doTeamPanic` (`panic`): each member with an AI to the path's waypoint closest to it; a member with no
    /// waypoint on the path ends the loop (`return`, not `continue`).
    pub fn do_team_wander_or_panic<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], label: &[u8], panic: bool) {
        let Some(t) = h.sa_team_named(team) else { return };
        for o in h.sa_team_members(t) {
            if !h.sa_obj_has_ai(o) {
                continue;
            }
            let pos = h.sa_obj_position(o);
            let Some(w) = h.sa_closest_waypoint_on_path(&pos, label) else { return };
            if panic != plant(3864) {
                h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_PANIC);
                h.sa_ai_panic(o, w, CMD_FROM_SCRIPT);
            } else {
                h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_WANDER);
                h.sa_ai_wander(o, w, CMD_FROM_SCRIPT);
            }
        }
    }

    /// `doTeamIncreasePriority` (`up`) and `doTeamDecreasePriority`: the prototype's priority changed, and a debug message.
    pub fn do_team_change_priority<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], up: bool) {
        let Some(t) = h.sa_team_named(team) else { return };
        let Some(p) = h.sa_team_prototype(t) else { return };
        h.sa_proto_change_priority(p, up);
        let msg = format!(
            "Team '{}' priority {} to {} for {}.",
            String::from_utf8_lossy(team),
            if up { "increased" } else { "decreased" },
            h.sa_proto_production_priority(p),
            if up { "success" } else { "failure" }
        );
        h.sa_append_debug_message(msg.as_bytes(), false);
    }

    /// `doDisplayCinematicText(displayText, fontType, timeInSeconds)`: the text, then the font parsed from "Name - Size: N [Bold]"
    /// (see the module docs for upstream's parse, kept as it is).
    pub fn do_display_cinematic_text<H: ScriptActionsHost>(&mut self, h: &mut H, text: &[u8], font_type: &[u8], seconds: i32) -> R<()> {
        let undefined = |u: Undefined| ActionError::from(IniError::undefined(u));
        let (u, _) = h.sa_game_text_fetch(text);
        let a: Vec<u8> = u.iter().map(|c| *c as u8).collect();
        h.sa_display_set_cinematic_text(&a);
        // strcpy(buf[256], fontType)
        if font_type.len() >= 256 {
            return Err(undefined(Undefined::ArrayIndexOutOfRange));
        }
        let buf = font_type;
        let at = |c: usize| -> R<u8> {
            match c.cmp(&buf.len()) {
                std::cmp::Ordering::Less => Ok(buf[c]),
                std::cmp::Ordering::Equal => Ok(0),
                // past the terminator: the rest of the buffer is uninitialised
                std::cmp::Ordering::Greater => Err(undefined(Undefined::UninitialisedLocal)),
            }
        };
        let mut name = Vec::new();
        let mut c = 0usize;
        // for( c = buf; c != NULL; *c++ ) { if( *c != ' ' && *c++ != '-' ) fontName.concat(c); else break; }
        loop {
            let ch = at(c)?;
            if ch == b' ' {
                break;
            }
            c += 1;
            if ch == b'-' {
                break;
            }
            if c > buf.len() {
                return Err(undefined(Undefined::UninitialisedLocal));
            }
            name.extend_from_slice(&buf[c..]);
            c += 1;
        }
        // while( *c != ':' ) *c++;  *c++;
        while at(c)? != b':' {
            c += 1;
        }
        c += 1;
        let mut size_text = Vec::new();
        while at(c)? != 0 {
            let ch = at(c)?;
            if ch != b' ' {
                size_text.push(ch);
            } else {
                break;
            }
            c += 1;
        }
        let size = atoi(&size_text)?;
        let bold = if plant(3872) { font_type.windows(6).any(|w| w == b"[Bold]") || font_type.ends_with(b"Bold") } else { font_type.ends_with(b"[Bold]") };
        let adjusted = h.sa_adjust_font_size(size);
        h.sa_display_set_cinematic_font(&name, adjusted, bold);
        let frames = LOGICFRAMES_PER_SECOND.checked_mul(seconds).ok_or_else(|| undefined(Undefined::SignedIntOverflow))?;
        h.sa_display_set_cinematic_text_frames(frames);
        Ok(())
    }

    /// The flash count for a time in seconds (`LOGICFRAMES_PER_SECOND * t / DRAWABLE_FRAMES_PER_FLASH`).
    fn flash_count(seconds: i32) -> R<i32> {
        let frames = LOGICFRAMES_PER_SECOND.checked_mul(seconds).ok_or(ActionError::Undefined(IniError::undefined(Undefined::SignedIntOverflow)))?;
        Ok(frames / DRAWABLE_FRAMES_PER_FLASH)
    }

    /// `doCameoFlash(name, timeInSeconds)`: the button is used unchecked.
    pub fn do_cameo_flash<H: ScriptActionsHost>(&mut self, h: &mut H, name: &[u8], seconds: i32) -> R<()> {
        let button = h.sa_find_command_button(name);
        let mut count = Self::flash_count(seconds)?;
        if count % 2 == 1 {
            count += 1;
        }
        let b = button.ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullObject)))?;
        h.sa_button_set_flash_count(b, count);
        h.sa_control_bar_set_flash(true);
        Ok(())
    }

    /// `doNamedFlash(unitName, timeInSeconds, color)` (`color` `None`: the object's indicator color).
    pub fn do_named_flash<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8], seconds: i32, color: Option<i32>) -> R<()> {
        let Some(o) = h.sa_unit_named(unit) else { return Ok(()) };
        if !h.sa_obj_has_drawable(o) {
            return Ok(());
        }
        if seconds > 0 {
            let count = Self::flash_count(seconds)?;
            let flashy = color.unwrap_or_else(|| h.sa_obj_indicator_color(o));
            h.sa_drawable_set_flash_color(o, flashy);
            h.sa_drawable_set_flash_count(o, count);
        }
        Ok(())
    }

    /// `doTeamFlash(teamName, timeInSeconds, color)`: a member with no drawable ends the loop.
    pub fn do_team_flash<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], seconds: i32, color: Option<i32>) -> R<()> {
        let Some(t) = h.sa_team_named(team) else { return Ok(()) };
        if !h.sa_team_has_any_objects(t) {
            return Ok(());
        }
        for o in h.sa_team_members(t) {
            if !h.sa_obj_has_drawable(o) {
                break;
            }
            let count = Self::flash_count(seconds)?;
            let flashy = color.unwrap_or_else(|| h.sa_obj_indicator_color(o));
            h.sa_drawable_set_flash_color(o, flashy);
            h.sa_drawable_set_flash_count(o, count);
        }
        Ok(())
    }

    /// `doSpeechPlay(speechName, allowOverlap)`: the speech for the local player (unchecked), and its subtitle when the text has one.
    pub fn do_speech_play<H: ScriptActionsHost>(&mut self, h: &mut H, speech: &[u8], allow_overlap: bool) -> R<()> {
        let local = h.sa_local_player().ok_or_else(null_local_player)?;
        let e = AudioEvent { name: speech.to_vec(), position: None, logical: true, player_index: local, object: 0, uninterruptable: !allow_overlap, should_fade: false };
        h.sa_audio_add_event(&e);
        let mut label = b"DIALOGEVENT:".to_vec();
        label.extend_from_slice(speech);
        label.extend_from_slice(b"Subtitle");
        let (subtitle, found) = h.sa_game_text_fetch(&label);
        if (found || plant(3876)) && !subtitle.is_empty() && subtitle[0] != u16::from(b'*') {
            h.sa_ui_military_subtitle(&label, SUBTITLE_DURATION);
        }
        Ok(())
    }

    /// `doNamedTransferAssetsToPlayer(unitName, playerName)`: the unit onto the player's default team, then
    /// `updateTeamAndPlayerStuff`: its upgrades, and its drawable's indicator color (the night one at night).
    pub fn do_named_transfer_assets_to_player<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8], player: &[u8]) {
        let o = h.sa_unit_named(unit);
        let p = h.sa_player_from_name(player);
        let (Some(o), Some(p)) = (o, p) else { return };
        let Some(team) = h.sa_player_default_team(p) else { return };
        h.sa_obj_set_team(o, Some(team));
        Self::update_team_and_player_stuff(h, o);
    }

    /// The batch-A3 cases of `executeAction` (`None`: not one of them).
    fn execute_a3<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> Option<R<()>> {
        use at::*;
        let white = rgb_as_int(1.0, 1.0, 1.0);
        let r = (|| -> R<()> {
            match a.action_type {
                RECRUIT_TEAM | BUILD_TEAM => {
                    let team = ps(a, 0)?;
                    let radius = if a.action_type == RECRUIT_TEAM { pr(a, 1)? } else { 0.0 };
                    let Some(proto) = h.sa_find_team_prototype(&team) else { return Ok(()) };
                    let Some(p) = h.sa_proto_controlling_player(proto) else { return Ok(()) };
                    if a.action_type == RECRUIT_TEAM {
                        h.sa_player_recruit_specific_team(p, proto, radius);
                    } else {
                        h.sa_player_build_specific_team(p, proto);
                    }
                }
                NAMED_DAMAGE => {
                    let (u, amount) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_obj_attempt_damage(o, 0, DAMAGE_UNRESISTABLE, DEATH_NORMAL, amount as f32);
                    }
                }
                NAMED_DELETE => {
                    let u = ps(a, 0)?;
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_logic_destroy_object(o);
                    }
                }
                TEAM_DELETE | TEAM_DELETE_LIVING => {
                    let t = ps(a, 0)?;
                    if let Some(t) = h.sa_team_named(&t) {
                        h.sa_team_delete(t, a.action_type == TEAM_DELETE_LIVING);
                    }
                }
                TEAM_WANDER | TEAM_PANIC => {
                    let (t, l) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_team_wander_or_panic(h, &t, &l, a.action_type == TEAM_PANIC);
                }
                TEAM_WANDER_IN_PLACE => {
                    let t = ps(a, 0)?;
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    for o in h.sa_team_members(t) {
                        if h.sa_obj_has_ai(o) {
                            h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_WANDER);
                            h.sa_ai_wander_in_place(o, CMD_FROM_SCRIPT);
                        }
                    }
                }
                TEAM_INCREASE_PRIORITY | TEAM_DECREASE_PRIORITY => {
                    let t = ps(a, 0)?;
                    self.do_team_change_priority(h, &t, a.action_type == TEAM_INCREASE_PRIORITY);
                }
                NAMED_KILL => {
                    let u = ps(a, 0)?;
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_obj_kill(o);
                    }
                }
                TEAM_KILL => {
                    let t = ps(a, 0)?;
                    if let Some(t) = h.sa_team_named(&t) {
                        h.sa_team_kill(t);
                    }
                }
                PLAYER_KILL => {
                    let p = ps(a, 0)?;
                    if let Some(p) = h.sa_player_from_name(&p) {
                        h.sa_player_kill(p);
                    }
                }
                DISPLAY_TEXT => {
                    let s = ps(a, 0)?;
                    h.sa_ui_message(&s);
                }
                DISPLAY_CINEMATIC_TEXT => {
                    let (s, f, n) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    self.do_display_cinematic_text(h, &s, &f, n)?;
                }
                INGAME_POPUP_MESSAGE => {
                    let (m, x, y, w, pause) = (ps(a, 0)?, pi(a, 1)?, pi(a, 2)?, pi(a, 3)?, pi(a, 4)?);
                    h.sa_ui_popup_message(&m, x, y, w, pause != 0, false);
                }
                CAMEO_FLASH => {
                    let (n, s) = (ps(a, 0)?, pi(a, 1)?);
                    self.do_cameo_flash(h, &n, s)?;
                }
                NAMED_FLASH | NAMED_FLASH_WHITE => {
                    let (u, s) = (ps(a, 0)?, pi(a, 1)?);
                    let c = (a.action_type == NAMED_FLASH_WHITE).then_some(white);
                    self.do_named_flash(h, &u, s, c)?;
                }
                TEAM_FLASH | TEAM_FLASH_WHITE => {
                    let (t, s) = (ps(a, 0)?, pi(a, 1)?);
                    let c = (a.action_type == TEAM_FLASH_WHITE).then_some(white);
                    self.do_team_flash(h, &t, s, c)?;
                }
                NAMED_CUSTOM_COLOR => {
                    let (u, c) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_obj_set_custom_indicator_color(o, c);
                    }
                }
                MOVIE_PLAY_FULLSCREEN => {
                    let m = ps(a, 0)?;
                    h.sa_display_play_movie(&m);
                }
                MOVIE_PLAY_RADAR => {
                    let m = ps(a, 0)?;
                    h.sa_ui_play_movie(&m);
                }
                SOUND_PLAY_NAMED => {
                    let (s, u) = (ps(a, 0)?, ps(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        let e = AudioEvent { name: s, position: None, logical: true, player_index: -1, object: o, uninterruptable: false, should_fade: false };
                        h.sa_audio_add_event(&e);
                    }
                }
                SPEECH_PLAY => {
                    let (s, overlap) = (ps(a, 0)?, pi(a, 1)?);
                    self.do_speech_play(h, &s, overlap != 0)?;
                }
                PLAYER_TRANSFER_OWNERSHIP_PLAYER | PLAYER_RELATES_PLAYER => {
                    let (src, dst) = (ps(a, 0)?, ps(a, 1)?);
                    let rel = if a.action_type == PLAYER_RELATES_PLAYER { pi(a, 2)? } else { 0 };
                    let d = h.sa_player_from_name(&dst);
                    let s = h.sa_player_from_name(&src);
                    let (Some(d), Some(s)) = (d, s) else { return Ok(()) };
                    if a.action_type == PLAYER_RELATES_PLAYER {
                        h.sa_player_set_player_relationship(s, d, rel);
                    } else {
                        h.sa_player_transfer_assets_from(d, s);
                    }
                }
                NAMED_TRANSFER_OWNERSHIP_PLAYER => {
                    let (u, p) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_named_transfer_assets_to_player(h, &u, &p);
                }
                PLAYER_EXCLUDE_FROM_SCORE_SCREEN => {
                    let p = ps(a, 0)?;
                    if let Some(p) = h.sa_player_from_name(&p) {
                        h.sa_player_set_list_in_score_screen(p, false);
                    }
                }
                ENABLE_SCORING | DISABLE_SCORING => h.sa_logic_enable_scoring(a.action_type == ENABLE_SCORING),
                RADAR_CREATE_EVENT => {
                    let pos = parm(a, 0)?.get_coord3d();
                    let t = pi(a, 1)?;
                    h.sa_radar_create_event(&pos, t);
                }
                _ => return Err(ActionError::NotTranslated(a.action_type)),
            }
            Ok(())
        })();
        match r {
            Err(ActionError::NotTranslated(_)) => None,
            r => Some(r),
        }
    }

    // ------------------------------------------------------------------------------------------------------------------------
    // batch A4: lines 2870-3475
    // ------------------------------------------------------------------------------------------------------------------------

    /// `doCameraMotionBlurJump(waypointName, saturate)`: the motion blur filter in and out at the waypoint, else just the look.
    pub fn do_camera_motion_blur_jump<H: ScriptActionsHost>(&mut self, h: &mut H, waypoint: &[u8], saturate: bool) {
        let Some(w) = h.sa_waypoint_by_name(waypoint) else { return };
        let mut passed = false;
        let pos = h.sa_waypoint_location(w);
        if h.sa_view_set_view_filter(FT_VIEW_MOTION_BLUR_FILTER) {
            passed = true;
            let mode = if saturate { filter_mode::IN_AND_OUT_SATURATE } else { filter_mode::IN_AND_OUT_ALPHA };
            if !h.sa_view_set_view_filter_mode(mode) {
                h.sa_view_set_view_filter(FT_NULL_FILTER);
                passed = false;
            }
            if passed {
                h.sa_view_set_view_filter_pos(&pos);
            }
        }
        if !passed {
            h.sa_view_look_at(&pos);
        }
    }

    /// `doCameraMotionBlur(zoomIn, saturate)`.
    pub fn do_camera_motion_blur<H: ScriptActionsHost>(&mut self, h: &mut H, zoom_in: bool, saturate: bool) {
        if h.sa_view_set_view_filter(FT_VIEW_MOTION_BLUR_FILTER) {
            let mode = match (saturate, zoom_in) {
                (true, true) => filter_mode::IN_SATURATE,
                (true, false) => filter_mode::OUT_SATURATE,
                (false, true) => filter_mode::IN_ALPHA,
                (false, false) => filter_mode::OUT_ALPHA,
            };
            if !h.sa_view_set_view_filter_mode(mode) {
                h.sa_view_set_view_filter(FT_NULL_FILTER);
            }
        }
    }

    /// `doRevealMapAtWaypoint` (`cover` false) and `doShroudMapAtWaypoint`: the area revealed (or covered) and the reveal (or
    /// cover) undone at once, for the named player, else for `getHumanPlayerMask()`, which reads an uninitialised mask.
    pub fn do_shroud_at_waypoint<H: ScriptActionsHost>(&mut self, h: &mut H, waypoint: &[u8], radius: f32, player: &[u8], cover: bool) -> R<()> {
        let Some(w) = h.sa_waypoint_by_name(waypoint) else { return Ok(()) };
        let p = h.sa_player_from_name(player);
        let mask = match p {
            Some(p) if !player.is_empty() => h.sa_player_mask(p),
            // getHumanPlayerMask: `PlayerMaskType mask; mask &= ...` from an uninitialised value
            _ => return Err(IniError::undefined(Undefined::UninitialisedLocal).into()),
        };
        let l = h.sa_waypoint_location(w);
        h.sa_partition_shroud(cover, false, l.x, l.y, radius, mask);
        h.sa_partition_shroud(cover, true, l.x, l.y, radius, mask);
        Ok(())
    }

    /// The named player (`f(Some(p))`), or, with no name or no such player, each human player (`f(None)` is never called: the
    /// callback receives each index).
    fn for_player_or_humans<H: ScriptActionsHost>(h: &mut H, player: &[u8], mut f: impl FnMut(&mut H, i32)) {
        let p = h.sa_player_from_name(player);
        match p {
            Some(p) if !player.is_empty() => f(h, p),
            _ => {
                for i in 0..h.sa_player_count() {
                    if let Some(p) = h.sa_nth_player(i) {
                        if h.sa_player_is_human(p) {
                            f(h, i);
                        }
                    }
                }
            }
        }
    }

    /// `doMergeTeamIntoTeam(teamSrcName, teamDestName)`: each member onto the destination (the first one twice, as upstream's loop
    /// does), the source team deleted, the destination made active.
    pub fn do_merge_team_into_team<H: ScriptActionsHost>(&mut self, h: &mut H, src: &[u8], dest: &[u8]) {
        let s = h.sa_team_named(src);
        let mut d = h.sa_team_named(dest);
        if d.is_none() {
            d = h.sa_find_team(dest);
        }
        let (Some(s), Some(d)) = (s, d) else { return };
        let members = h.sa_team_members(s);
        let mut next = members.first().copied();
        let mut i = usize::from(plant(3873));
        while i < members.len() {
            let Some(o) = next else { break };
            next = Some(members[i]);
            i += 1;
            self.set_team_and_update(h, o, d);
        }
        if let Some(n) = next {
            self.set_team_and_update(h, n, d);
        }
        h.sa_team_delete(s, false);
        h.sa_team_set_active(d);
    }

    /// `obj->setTeam(team)` then `updateTeamAndPlayerStuff(obj)`.
    fn set_team_and_update<H: ScriptActionsHost>(&mut self, h: &mut H, o: ObjH, t: TeamH) {
        h.sa_obj_set_team(o, Some(t));
        Self::update_team_and_player_stuff(h, o);
    }

    /// `updateTeamAndPlayerStuff(obj)`: its upgrade modules, and its drawable's indicator color (the night one at night).
    fn update_team_and_player_stuff<H: ScriptActionsHost>(h: &mut H, o: ObjH) {
        h.sa_obj_update_upgrade_modules(o);
        if h.sa_obj_has_drawable(o) {
            let c = if h.sa_is_night() { h.sa_obj_night_indicator_color(o) } else { h.sa_obj_indicator_color(o) };
            h.sa_drawable_set_indicator_color(o, c);
        }
    }

    /// `doMusicTrackChange(newTrackName, fadeout, fadein)`: the music stopped, the track played for the local player (unchecked).
    pub fn do_music_track_change<H: ScriptActionsHost>(&mut self, h: &mut H, track: &[u8], fadeout: bool, fadein: bool) -> R<()> {
        h.sa_audio_remove_event_handle(if fadeout { AHSV_STOP_THE_MUSIC_FADE } else { AHSV_STOP_THE_MUSIC });
        let local = h.sa_local_player().ok_or_else(null_local_player)?;
        let e = AudioEvent { name: track.to_vec(), position: None, logical: false, player_index: local, object: 0, uninterruptable: false, should_fade: fadein };
        h.sa_audio_add_event(&e);
        h.sa_set_current_track_name(track);
        Ok(())
    }

    /// `doTeamGarrisonSpecificBuilding(teamName, buildingName)`. Upstream's test reads `!(structure && nobody entered || the team's
    /// player entered)`: `&&` binds first, so a building of the team's player is entered whatever it is, and the team's player is
    /// read (unchecked) only when the first half fails.
    pub fn do_team_garrison_specific_building<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], building: &[u8]) -> R<()> {
        let Some(t) = h.sa_team_named(team) else { return Ok(()) };
        let Some(b) = h.sa_unit_named(building) else { return Ok(()) };
        if !h.sa_obj_has_contain(b) {
            return Ok(());
        }
        let player = h.sa_contain_player_who_entered(b);
        let first = h.sa_obj_is_kind_of(b, KINDOF_STRUCTURE) && player == 0;
        let ok = first || {
            let p = h.sa_team_controlling_player(t).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullControllingPlayer)))?;
            player == h.sa_player_mask(p)
        };
        if !ok || (first && plant(3875)) {
            return Ok(());
        }
        let Some(g) = Self::team_group(h, t) else { return Ok(()) };
        h.sa_group_enter(g, b, CMD_FROM_SCRIPT);
        Ok(())
    }

    /// `doTeamGarrisonNearestBuilding(teamName)`: the buildings near the team's leader, nearest first, filled with the team's infantry
    /// in order (one walk of the team across all buildings).
    pub fn do_team_garrison_nearest_building<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8]) {
        let Some(t) = h.sa_team_named(team) else { return };
        let members = h.sa_team_members(t);
        let Some(&leader) = members.first() else { return };
        let hacker = h.sa_obj_is_kind_of(leader, KINDOF_MONEY_HACKER);
        let mut cur = 0usize;
        for b in h.sa_partition_garrison_candidates(leader, hacker) {
            if !h.sa_obj_has_contain(b) {
                continue;
            }
            // (UnsignedInt - UnsignedInt, as an Int)
            let slots = h.sa_contain_max(b).wrapping_sub(h.sa_contain_count(b)) as i32;
            let mut i = 0;
            while i < slots {
                let Some(&o) = members.get(cur) else { return };
                if h.sa_obj_has_ai(o) && h.sa_obj_is_kind_of(o, KINDOF_INFANTRY) && !h.sa_obj_is_kind_of(o, KINDOF_NO_GARRISON) {
                    h.sa_ai_enter(o, b, CMD_FROM_SCRIPT);
                    i += 1;
                } else if plant(3874) {
                    return;
                }
                cur += 1;
            }
        }
    }

    /// The batch-A4 cases of `executeAction` (`None`: not one of them).
    fn execute_a4<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> Option<R<()>> {
        use at::*;
        let r = (|| -> R<()> {
            match a.action_type {
                OBJECT_CREATE_RADAR_EVENT => {
                    let (u, ty) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        let p = h.sa_obj_position(o);
                        h.sa_radar_create_event(&p, ty);
                    }
                }
                TEAM_CREATE_RADAR_EVENT => {
                    let (t, ty) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    if !h.sa_team_has_any_units(t) {
                        return Ok(());
                    }
                    if let Some(p) = h.sa_team_estimate_position(t) {
                        h.sa_radar_create_event(&p, ty);
                    }
                }
                RADAR_DISABLE | RADAR_ENABLE => h.sa_radar_hide(a.action_type == RADAR_DISABLE),
                MAP_REVEAL_AT_WAYPOINT | MAP_SHROUD_AT_WAYPOINT => {
                    let (w, r, p) = (ps(a, 0)?, pr(a, 1)?, ps(a, 2)?);
                    self.do_shroud_at_waypoint(h, &w, r, &p, a.action_type == MAP_SHROUD_AT_WAYPOINT)?;
                }
                MAP_REVEAL_ALL | MAP_REVEAL_ALL_PERM | MAP_REVEAL_ALL_UNDO_PERM | MAP_SHROUD_ALL => {
                    let p = ps(a, 0)?;
                    let ty = a.action_type;
                    Self::for_player_or_humans(h, &p, |h, i| match ty {
                        MAP_REVEAL_ALL => h.sa_partition_reveal_map(i),
                        MAP_REVEAL_ALL_PERM => h.sa_partition_reveal_map_permanently(i, false),
                        MAP_REVEAL_ALL_UNDO_PERM => h.sa_partition_reveal_map_permanently(i, true),
                        _ => h.sa_partition_shroud_map(i),
                    });
                }
                TEAM_AVAILABLE_FOR_RECRUITMENT => {
                    let (t, v) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(t) = h.sa_team_named(&t) {
                        h.sa_team_set_recruitable(t, v != 0);
                    }
                }
                TEAM_COLLECT_NEARBY_FOR_TEAM => {
                    ps(a, 0)?;
                }
                TEAM_MERGE_INTO_TEAM => {
                    let (s, d) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_merge_team_into_team(h, &s, &d);
                }
                IDLE_ALL_UNITS | RESUME_SUPPLY_TRUCKING => {
                    let idle = a.action_type == IDLE_ALL_UNITS;
                    Self::for_player_or_humans(h, b"", |h, i| h.sa_player_set_units_should_idle_or_resume(i, idle));
                }
                DISABLE_INPUT => self.do_disable_input(h),
                ENABLE_INPUT => {
                    h.sa_ui_set_input_enabled(true);
                    h.sa_mouse_set_visibility(true);
                }
                DISABLE_BORDER_SHROUD | ENABLE_BORDER_SHROUD => {
                    let level = h.sa_global_alpha(a.action_type == ENABLE_BORDER_SHROUD);
                    h.sa_display_set_border_shroud_level(level);
                }
                SOUND_AMBIENT_PAUSE | SOUND_AMBIENT_RESUME => h.sa_audio_pause_ambient(a.action_type == SOUND_AMBIENT_PAUSE),
                MUSIC_SET_TRACK => {
                    let (n, fo, fi) = (ps(a, 0)?, pi(a, 1)?, pi(a, 2)?);
                    self.do_music_track_change(h, &n, fo != 0, fi != 0)?;
                }
                TEAM_GARRISON_SPECIFIC_BUILDING => {
                    let (t, b) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_team_garrison_specific_building(h, &t, &b)?;
                }
                EXIT_SPECIFIC_BUILDING => {
                    let b = ps(a, 0)?;
                    let Some(o) = h.sa_unit_named(&b) else { return Ok(()) };
                    if !h.sa_obj_is_kind_of(o, KINDOF_STRUCTURE) {
                        return Ok(());
                    }
                    if h.sa_obj_has_ai(o) {
                        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
                        h.sa_ai_evacuate(o, false, CMD_FROM_SCRIPT);
                        return Ok(());
                    }
                    if h.sa_obj_has_contain(o) {
                        h.sa_contain_remove_all_contained(o, false);
                    }
                }
                TEAM_GARRISON_NEAREST_BUILDING => {
                    let t = ps(a, 0)?;
                    self.do_team_garrison_nearest_building(h, &t);
                }
                TEAM_EXIT_ALL_BUILDINGS => {
                    let t = ps(a, 0)?;
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    for o in h.sa_team_members(t) {
                        if h.sa_obj_has_ai(o) {
                            h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
                            h.sa_ai_exit(o, CMD_FROM_SCRIPT);
                        }
                    }
                }
                CAMERA_MOTION_BLUR => {
                    let (z, s) = (pi(a, 0)?, pi(a, 1)?);
                    self.do_camera_motion_blur(h, z != 0, s != 0);
                }
                CAMERA_MOTION_BLUR_JUMP => {
                    let (w, s) = (ps(a, 0)?, pi(a, 1)?);
                    self.do_camera_motion_blur_jump(h, &w, s != 0);
                }
                _ => return Err(ActionError::NotTranslated(a.action_type)),
            }
            Ok(())
        })();
        match r {
            Err(ActionError::NotTranslated(_)) => None,
            r => Some(r),
        }
    }

    // ------------------------------------------------------------------------------------------------------------------------
    // batch A5: lines 3475-4165
    // ------------------------------------------------------------------------------------------------------------------------

    /// `doUnitGarrisonSpecificBuilding(unitName, buildingName)`: the same test as the team's, with the unit's player (unchecked).
    pub fn do_unit_garrison_specific_building<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8], building: &[u8]) -> R<()> {
        let Some(u) = h.sa_unit_named(unit) else { return Ok(()) };
        let Some(b) = h.sa_unit_named(building) else { return Ok(()) };
        if !h.sa_obj_has_contain(b) {
            return Ok(());
        }
        let player = h.sa_contain_player_who_entered(b);
        let first = h.sa_obj_is_kind_of(b, KINDOF_STRUCTURE) && player == 0;
        let ok = first || player == Self::unit_player_mask(h, u)?;
        if !ok || !h.sa_obj_has_ai(u) {
            return Ok(());
        }
        h.sa_ai_choose_locomotor_set(u, LOCOMOTORSET_NORMAL);
        h.sa_ai_enter(u, b, CMD_FROM_SCRIPT);
        Ok(())
    }

    /// `obj->getControllingPlayer()->getPlayerMask()` (unchecked).
    fn unit_player_mask<H: ScriptActionsHost>(h: &H, u: ObjH) -> R<u32> {
        let p = h.sa_obj_controlling_player(u).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullControllingPlayer)))?;
        Ok(h.sa_player_mask(p))
    }

    /// `doUnitGarrisonNearestBuilding(unitName)`: the nearest building nobody else holds.
    pub fn do_unit_garrison_nearest_building<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8]) -> R<()> {
        let Some(u) = h.sa_unit_named(unit) else { return Ok(()) };
        if !h.sa_obj_has_ai(u) {
            return Ok(());
        }
        let hacker = h.sa_obj_is_kind_of(u, KINDOF_MONEY_HACKER);
        for b in h.sa_partition_unit_garrison_candidates(u, hacker) {
            if !h.sa_obj_has_contain(b) {
                continue;
            }
            let player = h.sa_contain_player_who_entered(b);
            if !(player == 0 || player == Self::unit_player_mask(h, u)?) {
                continue;
            }
            h.sa_ai_choose_locomotor_set(u, LOCOMOTORSET_NORMAL);
            h.sa_ai_enter(u, b, CMD_FROM_SCRIPT);
            return Ok(());
        }
        Ok(())
    }

    /// `doNamedSetUnmanned` and each member of `doTeamSetUnmanned`: disabled unmanned, deselected, onto the neutral player's
    /// default team (the neutral player unchecked; no default team gives `setTeam(NULL)`).
    fn set_unmanned<H: ScriptActionsHost>(h: &mut H, o: ObjH) -> R<()> {
        h.sa_obj_set_disabled(o, DISABLED_UNMANNED);
        h.sa_logic_deselect_object(o, PLAYERMASK_ALL, !plant(3889));
        let n = h.sa_neutral_player().ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullObject)))?;
        let t = h.sa_player_default_team(n);
        h.sa_obj_set_team(o, t);
        Ok(())
    }

    /// `doNamedSetBoobytrapped` and each member of `doTeamSetBoobytrapped`: a booby trap of the template on the object's team, stuck
    /// at a random point of its perimeter.
    fn set_boobytrapped<H: ScriptActionsHost>(h: &mut H, thing: &[u8], o: ObjH) -> R<()> {
        let Some(tmpl) = h.sa_find_template(thing) else { return Ok(()) };
        let team = h.sa_obj_team(o);
        let Some(bomb) = h.sa_new_object(tmpl, team) else { return Ok(()) };
        if h.sa_obj_has_sticky_bomb_update(bomb) {
            let pos = h.sa_obj_random_offset_on_perimeter(o);
            let pos = h.sa_obj_transform_vector(o, &pos);
            h.sa_sticky_bomb_init(bomb, o, &pos);
        }
        Ok(())
    }

    /// The stopping distance of a unit's current locomotor (only from 0.5); `false`: no AI or no locomotor.
    fn set_stopping_distance<H: ScriptActionsHost>(h: &mut H, o: ObjH, d: f32) -> bool {
        if !h.sa_obj_has_ai(o) || !h.sa_ai_has_cur_locomotor(o) {
            return false;
        }
        if d >= 0.5 && !plant(3888) {
            h.sa_locomotor_set_close_enough_dist(o, d);
        }
        true
    }

    /// The named unit's module for the special power (`None`: no unit, no power, or no module).
    fn special_power_module<H: ScriptActionsHost>(h: &H, unit: &[u8], power: &[u8]) -> Option<(ObjH, SpecialPowerH)> {
        let o = h.sa_unit_named(unit);
        let p = h.sa_find_special_power(power);
        let (o, p) = (o?, p?);
        h.sa_obj_has_special_power_module(o, p).then_some((o, p))
    }

    /// The batch-A5 cases of `executeAction` (`None`: not one of them).
    fn execute_a5<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> Option<R<()>> {
        use at::*;
        let r = (|| -> R<()> {
            match a.action_type {
                NAMED_SET_STEALTH_ENABLED => {
                    let (u, e) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_obj_set_script_status(o, script_status::UNSTEALTHED, e == 0);
                    }
                }
                TEAM_SET_STEALTH_ENABLED => {
                    let (t, e) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    for o in h.sa_team_members(t) {
                        h.sa_obj_set_script_status(o, script_status::UNSTEALTHED, e == 0);
                    }
                }
                NAMED_SET_UNMANNED_STATUS => {
                    let u = ps(a, 0)?;
                    if let Some(o) = h.sa_unit_named(&u) {
                        Self::set_unmanned(h, o)?;
                    }
                }
                TEAM_SET_UNMANNED_STATUS => {
                    let t = ps(a, 0)?;
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    for o in h.sa_team_members(t) {
                        Self::set_unmanned(h, o)?;
                    }
                }
                NAMED_SET_BOOBYTRAPPED => {
                    let (thing, u) = (ps(a, 0)?, ps(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        Self::set_boobytrapped(h, &thing, o)?;
                    }
                }
                TEAM_SET_BOOBYTRAPPED => {
                    let (thing, t) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    for o in h.sa_team_members(t) {
                        Self::set_boobytrapped(h, &thing, o)?;
                    }
                }
                NAMED_GARRISON_SPECIFIC_BUILDING => {
                    let (u, b) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_unit_garrison_specific_building(h, &u, &b)?;
                }
                NAMED_GARRISON_NEAREST_BUILDING => {
                    let u = ps(a, 0)?;
                    self.do_unit_garrison_nearest_building(h, &u)?;
                }
                NAMED_EXIT_BUILDING => {
                    let u = ps(a, 0)?;
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if h.sa_obj_has_ai(o) {
                        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
                        h.sa_ai_exit(o, CMD_FROM_SCRIPT);
                    }
                }
                PLAYER_GARRISON_ALL_BUILDINGS | PLAYER_EXIT_ALL_BUILDINGS => {
                    let p = ps(a, 0)?;
                    if let Some(p) = h.sa_player_from_name(&p) {
                        h.sa_player_garrison_all(p, a.action_type == PLAYER_GARRISON_ALL_BUILDINGS, CMD_FROM_SCRIPT);
                    }
                }
                CAMERA_LETTERBOX_BEGIN => {
                    h.sa_control_bar_show(true, true);
                    h.sa_display_enable_letter_box(true);
                }
                CAMERA_LETTERBOX_END => {
                    h.sa_control_bar_show(false, false);
                    h.sa_display_enable_letter_box(false);
                }
                CAMERA_BW_MODE_BEGIN | CAMERA_BW_MODE_END => {
                    let frames = a.get_parameter(0).map(|p| p.int).unwrap_or(0);
                    if a.action_type == CAMERA_BW_MODE_BEGIN {
                        h.sa_view_set_view_filter_mode(FM_VIEW_BW_BLACK_AND_WHITE);
                        h.sa_view_set_view_filter(FT_VIEW_BW_FILTER);
                        h.sa_view_set_fade_parameters(frames, 1);
                    } else if h.sa_view_filter_type() == FT_VIEW_BW_FILTER {
                        h.sa_view_set_fade_parameters(frames, -1);
                    }
                }
                DRAW_SKYBOX_BEGIN | DRAW_SKYBOX_END => h.sa_sky_box_set(a.action_type == DRAW_SKYBOX_BEGIN),
                SHOW_WEATHER => {
                    let v = pi(a, 0)?;
                    h.sa_snow_set_visible(v != 0);
                }
                FREEZE_TIME | UNFREEZE_TIME => h.sa_freeze_time(a.action_type == FREEZE_TIME),
                SHOW_MILITARY_CAPTION => {
                    let (b, d) = (ps(a, 0)?, pi(a, 1)?);
                    h.sa_ui_military_subtitle(&b, d);
                }
                CAMERA_SET_AUDIBLE_DISTANCE => {
                    pr(a, 0)?;
                }
                SET_STOPPING_DISTANCE => {
                    let (t, d) = (ps(a, 0)?, pr(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    for o in h.sa_team_members(t) {
                        if !Self::set_stopping_distance(h, o, d) {
                            return Ok(()); // (`return`, not `continue`)
                        }
                    }
                }
                NAMED_SET_STOPPING_DISTANCE => {
                    let (u, d) = (ps(a, 0)?, pr(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        Self::set_stopping_distance(h, o, d);
                    }
                }
                NAMED_SET_HELD => {
                    let (u, held) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_obj_set_disabled_until(o, DISABLED_HELD, if held != 0 { FOREVER } else { NEVER });
                    }
                }
                DISABLE_SPECIAL_POWER_DISPLAY | ENABLE_SPECIAL_POWER_DISPLAY => {
                    h.sa_ui_set_superweapon_display_enabled(a.action_type == ENABLE_SPECIAL_POWER_DISPLAY)
                }
                NAMED_HIDE_SPECIAL_POWER_DISPLAY | NAMED_SHOW_SPECIAL_POWER_DISPLAY => {
                    let u = ps(a, 0)?;
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_ui_object_superweapon_display(o, a.action_type == NAMED_HIDE_SPECIAL_POWER_DISPLAY);
                    }
                }
                MUSIC_SET_VOLUME | SOUND_SET_VOLUME | SPEECH_SET_VOLUME => {
                    let affect = match a.action_type {
                        MUSIC_SET_VOLUME => AUDIO_AFFECT_MUSIC,
                        SOUND_SET_VOLUME => AUDIO_AFFECT_SOUND | AUDIO_AFFECT_SOUND_3D,
                        _ => AUDIO_AFFECT_SPEECH,
                    };
                    // (upstream's two comparisons: NaN and -0.0 pass through, as with `clamp`)
                    let v = (pr(a, 0)? / 100.0).clamp(0.0, 1.0);
                    h.sa_audio_set_volume(v, affect);
                }
                TEAM_TRANSFER_TO_PLAYER => {
                    let (t, p) = (ps(a, 0)?, ps(a, 1)?);
                    let t = h.sa_team_named(&t);
                    let p = h.sa_player_from_name(&p);
                    let (Some(t), Some(p)) = (t, p) else { return Ok(()) };
                    h.sa_team_set_controlling_player(t, p);
                    for o in h.sa_team_members(t) {
                        Self::update_team_and_player_stuff(h, o);
                    }
                }
                PLAYER_SET_MONEY | PLAYER_GIVE_MONEY => {
                    let (p, money) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(p) = h.sa_player_from_name(&p) else { return Ok(()) };
                    if a.action_type == PLAYER_SET_MONEY {
                        let all = h.sa_money_count(p);
                        h.sa_money_withdraw(p, all);
                        h.sa_money_deposit(p, money as u32);
                    } else if money < 0 {
                        let m = money.checked_neg().ok_or(ActionError::Undefined(IniError::undefined(Undefined::SignedIntOverflow)))?;
                        h.sa_money_withdraw(p, m as u32);
                    } else {
                        h.sa_money_deposit(p, money as u32);
                    }
                }
                DISPLAY_COUNTDOWN_TIMER | DISPLAY_COUNTER => {
                    let (n, text) = (ps(a, 0)?, ps(a, 1)?);
                    let (u, _) = h.sa_game_text_fetch(&text);
                    h.sa_ui_add_named_timer(&n, &u, a.action_type == DISPLAY_COUNTDOWN_TIMER);
                }
                HIDE_COUNTDOWN_TIMER | HIDE_COUNTER => {
                    let n = ps(a, 0)?;
                    h.sa_ui_remove_named_timer(&n);
                }
                DISABLE_COUNTDOWN_TIMER_DISPLAY | ENABLE_COUNTDOWN_TIMER_DISPLAY => {
                    h.sa_ui_show_named_timer_display(a.action_type == ENABLE_COUNTDOWN_TIMER_DISPLAY)
                }
                NAMED_STOP_SPECIAL_POWER_COUNTDOWN | NAMED_START_SPECIAL_POWER_COUNTDOWN => {
                    let (u, p) = (ps(a, 0)?, ps(a, 1)?);
                    if let Some((o, sp)) = Self::special_power_module(h, &u, &p) {
                        h.sa_spm_pause_countdown(o, sp, a.action_type == NAMED_STOP_SPECIAL_POWER_COUNTDOWN);
                    }
                }
                NAMED_SET_SPECIAL_POWER_COUNTDOWN | NAMED_ADD_SPECIAL_POWER_COUNTDOWN => {
                    let (u, p, secs) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    if let Some((o, sp)) = Self::special_power_module(h, &u, &p) {
                        let frames = LOGICFRAMES_PER_SECOND.checked_mul(secs).ok_or(ActionError::Undefined(IniError::undefined(Undefined::SignedIntOverflow)))?;
                        // (UnsignedInt + Int: unsigned arithmetic)
                        let base = if a.action_type == NAMED_SET_SPECIAL_POWER_COUNTDOWN { h.sa_frame() } else { h.sa_spm_ready_frame(o, sp) };
                        h.sa_spm_set_ready_frame(o, sp, base.wrapping_add(frames as u32));
                    }
                }
                NAMED_FIRE_SPECIAL_POWER_AT_WAYPOINT => {
                    let (u, p, w) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    if let Some((o, sp)) = Self::special_power_module(h, &u, &p) {
                        let Some(w) = h.sa_waypoint_by_name(&w) else { return Ok(()) };
                        let l = h.sa_waypoint_location(w);
                        h.sa_spm_do_at_location(o, sp, &l, INVALID_ANGLE, COMMAND_FIRED_BY_SCRIPT);
                    }
                }
                _ => return Err(ActionError::NotTranslated(a.action_type)),
            }
            Ok(())
        })();
        match r {
            Err(ActionError::NotTranslated(_)) => None,
            r => Some(r),
        }
    }

    // ------------------------------------------------------------------------------------------------------------------------
    // batch A6: lines 4165-4905
    // ------------------------------------------------------------------------------------------------------------------------

    /// `doSkirmishFireSpecialPowerAtMostCost(player, specialPower)`: in each team of the player, the first member whose module is
    /// ready fires at the target the player computes (for a sneak attack, the nearest construction zone to it); a member whose module
    /// is not ready is skipped, and the team's loop ends after the first ready one (`break`).
    pub fn do_skirmish_fire_special_power_at_most_cost<H: ScriptActionsHost>(&mut self, h: &mut H, player: &[u8], power: &[u8]) {
        let Some(enemy) = h.sa_skirmish_enemy_player() else { return };
        let Some(sp) = h.sa_find_special_power(power) else { return };
        let mut radius = 50.0f32;
        if h.sa_special_power_radius_cursor(sp) > radius {
            radius = h.sa_special_power_radius_cursor(sp);
        }
        let Some(p) = h.sa_player_from_name(player) else { return };
        for proto in h.sa_player_teams(p) {
            for t in proto {
                for o in h.sa_team_members(t) {
                    if !h.sa_obj_has_special_power_module(o, sp) {
                        continue;
                    }
                    if !h.sa_spm_is_ready(o, sp) {
                        continue;
                    }
                    let mut location = h.sa_player_compute_superweapon_target(p, sp, enemy, radius);
                    if location.is_some() && h.sa_special_power_type(sp) == SPECIAL_SNEAK_ATTACK {
                        if let Some(tt) = h.sa_spm_reference_template(o, sp) {
                            location = h.sa_player_closest_construction_zone(p, tt);
                        }
                    }
                    if let Some(l) = location {
                        if l.x * l.x + l.y * l.y + l.z * l.z > 0.0 {
                            h.sa_spm_do_at_location(o, sp, &l, INVALID_ANGLE, COMMAND_FIRED_BY_SCRIPT);
                        }
                    }
                    if plant(3878) {
                        continue;
                    }
                    break;
                }
            }
        }
    }

    /// The buttons of the unit's command set named `ability` (`None`: no command set).
    fn command_set_buttons_named<H: ScriptActionsHost>(h: &H, o: ObjH, ability: &[u8]) -> Vec<ButtonH> {
        let mut out = Vec::new();
        if let Some(set) = h.sa_find_command_set(&h.sa_obj_command_set_string(o)) {
            for i in 0..MAX_COMMANDS_PER_SET {
                if let Some(b) = h.sa_command_set_button(set, i) {
                    let n = h.sa_button_name(b);
                    if !n.is_empty() && n == ability {
                        out.push(b);
                    }
                }
            }
        }
        out
    }

    /// `doTeamStop(teamName, shouldDisband)`: the team idled; disbanded, its members recruitable and merged into its player's default
    /// team (the player and the team unchecked).
    pub fn do_team_stop<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], disband: bool) -> R<()> {
        let Some(t) = h.sa_team_named(team) else { return Ok(()) };
        let Some(g) = Self::team_group(h, t) else { return Ok(()) };
        h.sa_group_idle(g, CMD_FROM_SCRIPT);
        if disband {
            let p = h.sa_team_controlling_player(t).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullControllingPlayer)))?;
            let default = h.sa_player_default_team(p);
            for o in h.sa_team_members(t) {
                if h.sa_obj_has_ai(o) {
                    h.sa_ai_set_is_recruitable(o, !plant(3879));
                }
            }
            let d = default.ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullDefaultTeam)))?;
            let name = h.sa_team_name(d);
            self.do_merge_team_into_team(h, team, &name);
        }
        Ok(())
    }

    /// `doUnitStartSequentialScript` (`team` `None`) and `doTeamStartSequentialScript`.
    fn start_sequential_script<H: ScriptActionsHost>(&mut self, h: &mut H, name: &[u8], script: &[u8], loop_val: i32, team: bool) {
        if team {
            let Some(t) = h.sa_team_named(name) else { return };
            if !h.sa_find_script(script) {
                return;
            }
            let Some(g) = Self::team_group(h, t) else { return };
            h.sa_group_idle(g, CMD_FROM_SCRIPT);
            h.sa_append_sequential_script(Some(t), 0, script, loop_val);
        } else {
            let Some(o) = h.sa_unit_named(name) else { return };
            if !h.sa_find_script(script) {
                return;
            }
            h.sa_append_sequential_script(None, o, script, loop_val);
        }
    }

    /// `doNamedFireWeaponFollowingWaypointPath(unit, waypointPath)`: the projectile of its waypoint-following weapon sent along the path.
    pub fn do_named_fire_weapon_following_waypoint_path<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8], path: &[u8]) {
        let Some(u) = h.sa_unit_named(unit) else { return };
        let pos = h.sa_obj_position(u);
        let Some(w) = h.sa_closest_waypoint_on_path(&pos, path) else { return };
        if !h.sa_obj_has_waypoint_following_weapon(u) {
            return;
        }
        if let Some(pr) = h.sa_weapon_force_fire(u, &pos) {
            if !h.sa_obj_has_ai(pr) {
                return;
            }
            h.sa_obj_leave_group(pr);
            h.sa_ai_choose_locomotor_set(pr, LOCOMOTORSET_NORMAL);
            h.sa_ai_follow_waypoint_path(pr, w, plant(3880), CMD_FROM_SCRIPT);
        }
    }

    /// The batch-A6 cases of `executeAction` (`None`: not one of them).
    fn execute_a6<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> Option<R<()>> {
        use at::*;
        let r = (|| -> R<()> {
            match a.action_type {
                SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST => {
                    let (p, s) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_skirmish_fire_special_power_at_most_cost(h, &p, &s);
                }
                NAMED_FIRE_SPECIAL_POWER_AT_NAMED => {
                    let (u, s, tg) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    let o = h.sa_unit_named(&u);
                    let target = h.sa_unit_named(&tg);
                    let sp = h.sa_find_special_power(&s);
                    if let (Some(o), Some(sp), Some(target)) = (o, sp, target) {
                        if h.sa_obj_has_special_power_module(o, sp) {
                            h.sa_spm_do_at_object(o, sp, target, COMMAND_FIRED_BY_SCRIPT);
                        }
                    }
                }
                REFRESH_RADAR => h.sa_radar_refresh_terrain(),
                NAMED_STOP => {
                    let u = ps(a, 0)?;
                    if let Some(o) = h.sa_unit_named(&u) {
                        if h.sa_obj_has_ai(o) {
                            h.sa_ai_idle(o, CMD_FROM_SCRIPT);
                        }
                    }
                }
                TEAM_STOP | TEAM_STOP_AND_DISBAND => {
                    let t = ps(a, 0)?;
                    self.do_team_stop(h, &t, a.action_type == TEAM_STOP_AND_DISBAND)?;
                }
                CAMERA_TETHER_NAMED => {
                    let (u, snap, play) = (ps(a, 0)?, pi(a, 1)?, pr(a, 2)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_view_set_camera_lock(o);
                        if snap != 0 {
                            h.sa_view_snap_to_camera_lock();
                        }
                        h.sa_view_set_snap_mode(LOCK_TETHER, play);
                    }
                }
                CAMERA_STOP_TETHER_NAMED => h.sa_view_set_camera_lock(0),
                CAMERA_SET_DEFAULT => {
                    let (p, an, mh) = (pr(a, 0)?, pr(a, 1)?, pr(a, 2)?);
                    h.sa_view_set_default_view(p, an, mh);
                }
                TEAM_SET_OVERRIDE_RELATION_TO_TEAM | TEAM_REMOVE_OVERRIDE_RELATION_TO_TEAM => {
                    let (t, o) = (ps(a, 0)?, ps(a, 1)?);
                    let set = a.action_type == TEAM_SET_OVERRIDE_RELATION_TO_TEAM;
                    let r = if set { pi(a, 2)? } else { 0 };
                    let t = h.sa_team_named(&t);
                    let o = h.sa_team_named(&o);
                    if let (Some(t), Some(o)) = (t, o) {
                        if set {
                            h.sa_team_set_override_team_relationship(t, o, r);
                        } else {
                            h.sa_team_remove_override_team_relationship(t, o);
                        }
                    }
                }
                TEAM_REMOVE_ALL_OVERRIDE_RELATIONS => {
                    let t = ps(a, 0)?;
                    if let Some(t) = h.sa_team_named(&t) {
                        h.sa_team_remove_override_team_relationship(t, 0);
                        h.sa_team_remove_override_player_relationship(t, 0);
                    }
                }
                TEAM_SET_OVERRIDE_RELATION_TO_PLAYER | TEAM_REMOVE_OVERRIDE_RELATION_TO_PLAYER => {
                    let (t, p) = (ps(a, 0)?, ps(a, 1)?);
                    let set = a.action_type == TEAM_SET_OVERRIDE_RELATION_TO_PLAYER;
                    let r = if set { pi(a, 2)? } else { 0 };
                    let t = h.sa_team_named(&t);
                    let p = h.sa_player_with_name_key(&p);
                    if let (Some(t), Some(p)) = (t, p) {
                        if set {
                            h.sa_team_set_override_player_relationship(t, p, r);
                        } else {
                            h.sa_team_remove_override_player_relationship(t, p);
                        }
                    }
                }
                PLAYER_SET_OVERRIDE_RELATION_TO_TEAM | PLAYER_REMOVE_OVERRIDE_RELATION_TO_TEAM => {
                    let (p, t) = (ps(a, 0)?, ps(a, 1)?);
                    let set = a.action_type == PLAYER_SET_OVERRIDE_RELATION_TO_TEAM;
                    let r = if set { pi(a, 2)? } else { 0 };
                    let p = h.sa_player_with_name_key(&p);
                    let t = h.sa_team_named(&t);
                    if let (Some(p), Some(t)) = (p, t) {
                        if set {
                            h.sa_player_set_team_relationship(p, t, r);
                        } else {
                            h.sa_player_remove_team_relationship(p, t);
                        }
                    }
                }
                NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH => {
                    let (u, w) = (ps(a, 0)?, ps(a, 1)?);
                    self.do_named_fire_weapon_following_waypoint_path(h, &u, &w);
                }
                NAMED_USE_COMMANDBUTTON_ABILITY => {
                    let (u, ab) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    for b in Self::command_set_buttons_named(h, o, &ab) {
                        h.sa_obj_do_command_button(o, b, CMD_FROM_SCRIPT);
                    }
                }
                NAMED_USE_COMMANDBUTTON_ABILITY_ON_NAMED => {
                    let (u, ab, tg) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    let o = h.sa_unit_named(&u);
                    let target = h.sa_unit_named(&tg);
                    let (Some(o), Some(target)) = (o, target) else { return Ok(()) };
                    for b in Self::command_set_buttons_named(h, o, &ab) {
                        h.sa_obj_do_command_button_at_object(o, b, target, CMD_FROM_SCRIPT);
                    }
                }
                NAMED_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT => {
                    let (u, ab, w) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    let o = h.sa_unit_named(&u);
                    let w = h.sa_waypoint_by_name(&w);
                    let (Some(o), Some(w)) = (o, w) else { return Ok(()) };
                    let l = h.sa_waypoint_location(w);
                    for b in Self::command_set_buttons_named(h, o, &ab) {
                        h.sa_obj_do_command_button_at_position(o, b, &l, CMD_FROM_SCRIPT);
                    }
                }
                NAMED_USE_COMMANDBUTTON_ABILITY_USING_WAYPOINT_PATH => {
                    let (u, ab, path) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    let pos = h.sa_obj_position(o);
                    let Some(w) = h.sa_closest_waypoint_on_path(&pos, &path) else { return Ok(()) };
                    for b in Self::command_set_buttons_named(h, o, &ab) {
                        h.sa_obj_do_command_button_using_waypoints(o, b, w, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_USE_COMMANDBUTTON_ABILITY => {
                    let (t, ab) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let Some(b) = h.sa_find_command_button(&ab) else { return Ok(()) };
                    if let Some(g) = Self::team_group(h, t) {
                        h.sa_group_do_command_button(g, b, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_USE_COMMANDBUTTON_ABILITY_ON_NAMED => {
                    let (t, ab, tg) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let Some(target) = h.sa_unit_named(&tg) else { return Ok(()) };
                    let Some(b) = h.sa_find_command_button(&ab) else { return Ok(()) };
                    if let Some(g) = Self::team_group(h, t) {
                        h.sa_group_do_command_button_at_object(g, b, target, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT => {
                    let (t, ab, w) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let Some(w) = h.sa_waypoint_by_name(&w) else { return Ok(()) };
                    let Some(b) = h.sa_find_command_button(&ab) else { return Ok(()) };
                    if let Some(g) = Self::team_group(h, t) {
                        let l = h.sa_waypoint_location(w);
                        h.sa_group_do_command_button_at_position(g, b, &l, CMD_FROM_SCRIPT);
                    }
                }
                UNIT_EXECUTE_SEQUENTIAL_SCRIPT | TEAM_EXECUTE_SEQUENTIAL_SCRIPT => {
                    let (n, s) = (ps(a, 0)?, ps(a, 1)?);
                    self.start_sequential_script(h, &n, &s, 0, a.action_type == TEAM_EXECUTE_SEQUENTIAL_SCRIPT);
                }
                UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING | TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING => {
                    let (n, s, l) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    let l = l.checked_sub(1).ok_or(ActionError::Undefined(IniError::undefined(Undefined::SignedIntOverflow)))?;
                    self.start_sequential_script(h, &n, &s, l, a.action_type == TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING);
                }
                UNIT_STOP_SEQUENTIAL_SCRIPT => {
                    let u = ps(a, 0)?;
                    if let Some(o) = h.sa_unit_named(&u) {
                        h.sa_remove_all_sequential_scripts_obj(o);
                    }
                }
                TEAM_STOP_SEQUENTIAL_SCRIPT => {
                    let t = ps(a, 0)?;
                    if let Some(t) = h.sa_team_named(&t) {
                        h.sa_remove_all_sequential_scripts_team(t);
                    }
                }
                UNIT_GUARD_FOR_FRAMECOUNT | UNIT_IDLE_FOR_FRAMECOUNT => {
                    let (u, f) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if !h.sa_obj_has_ai(o) {
                        return Ok(());
                    }
                    if a.action_type == UNIT_GUARD_FOR_FRAMECOUNT {
                        let p = h.sa_obj_position(o);
                        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
                        h.sa_ai_guard_position(o, &p, GUARDMODE_NORMAL, CMD_FROM_SCRIPT);
                    } else {
                        h.sa_ai_idle(o, CMD_FROM_SCRIPT);
                    }
                    h.sa_set_sequential_timer_obj(o, f);
                }
                // (TEAM_GUARD_FOR_FRAMECOUNT executes doTeamIdleForFramecount upstream: doTeamGuardForFramecount is never called)
                TEAM_GUARD_FOR_FRAMECOUNT | TEAM_IDLE_FOR_FRAMECOUNT => {
                    let (t, f) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let Some(g) = Self::team_group(h, t) else { return Ok(()) };
                    h.sa_group_idle(g, CMD_FROM_SCRIPT);
                    h.sa_set_sequential_timer_team(t, f);
                }
                _ => return Err(ActionError::NotTranslated(a.action_type)),
            }
            Ok(())
        })();
        match r {
            Err(ActionError::NotTranslated(_)) => None,
            r => Some(r),
        }
    }

    // ------------------------------------------------------------------------------------------------------------------------
    // batch A7: lines 4905-5160
    // ------------------------------------------------------------------------------------------------------------------------

    /// `doBorderSwitch(border)`: the observer's ("ReplayObserver") permanent reveal undone and redone around the switch.
    pub fn do_border_switch<H: ScriptActionsHost>(&mut self, h: &mut H, border: i32) {
        let observer = h.sa_player_with_name_key(b"ReplayObserver");
        if let Some(o) = observer {
            h.sa_partition_reveal_map_permanently(o, true);
        }
        h.sa_set_active_boundary(border);
        if let Some(o) = observer {
            h.sa_partition_reveal_map_permanently(o, plant(3881));
        }
        h.sa_refresh_shroud_for_local_player();
    }

    /// `doForceObjectSelection(teamName, objectType, centerInView, audioToPlay)`: the team's member of the type with the lowest id
    /// selected, its sound played for the local player (unchecked), the camera moved to it.
    pub fn do_force_object_selection<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], object_type: &[u8], center: bool, audio: &[u8]) -> R<()> {
        let Some(t) = h.sa_team_named(team) else { return Ok(()) };
        let mut best: Option<ObjH> = None;
        let mut highest: Option<ObjH> = None;
        for o in h.sa_team_members(t) {
            if let Some(tt) = h.sa_obj_template(o) {
                if h.sa_template_name(tt) == object_type && best.is_none_or(|b| o < b) {
                    best = Some(o); // lower ID means its newer
                }
                if plant(3882) && h.sa_template_name(tt) == object_type && highest.is_none_or(|b| o > b) {
                    highest = Some(o);
                }
            }
        }
        let Some(b) = best.filter(|b| h.sa_obj_has_drawable(*b)) else { return Ok(()) };
        h.sa_ui_deselect_all_drawables();
        h.sa_ui_select_drawable(b);
        let local = h.sa_local_player().ok_or_else(null_local_player)?;
        let e = AudioEvent { name: audio.to_vec(), position: None, logical: false, player_index: local, object: 0, uninterruptable: false, should_fade: false };
        h.sa_audio_add_event(&e);
        if center {
            let p = h.sa_obj_position(highest.unwrap_or(b));
            h.sa_view_move_camera_to(&p, 0, 0, false, 0.0, 0.0);
        }
        Ok(())
    }

    /// The batch-A7 cases of `executeAction` (`None`: not one of them).
    fn execute_a7<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> Option<R<()>> {
        use at::*;
        let r = (|| -> R<()> {
            match a.action_type {
                WATER_CHANGE_HEIGHT => {
                    let (w, ht) = (ps(a, 0)?, pr(a, 1)?);
                    if let Some(w) = h.sa_water_handle(&w) {
                        h.sa_set_water_height(w, ht, 999_999.9, true);
                    }
                }
                WATER_CHANGE_HEIGHT_OVER_TIME => {
                    let (w, ht, time, dmg) = (ps(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?);
                    if let Some(w) = h.sa_water_handle(&w) {
                        h.sa_change_water_height_over_time(w, ht, time, dmg);
                    }
                }
                MAP_SWITCH_BORDER => {
                    let b = pi(a, 0)?;
                    self.do_border_switch(h, b);
                }
                OBJECT_FORCE_SELECT => {
                    let (t, ty, c, au) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?, ps(a, 3)?);
                    self.do_force_object_selection(h, &t, &ty, c != 0, &au)?;
                }
                UNIT_DESTROY_ALL_CONTAINED => {
                    let u = ps(a, 0)?;
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if !h.sa_obj_has_contain(o) || h.sa_contain_count(o) == 0 {
                        return Ok(());
                    }
                    for c in h.sa_contained(o) {
                        h.sa_obj_kill(c);
                    }
                }
                RADAR_FORCE_ENABLE | RADAR_REVERT_TO_NORMAL => h.sa_radar_force_on(a.action_type == RADAR_FORCE_ENABLE),
                SCREEN_SHAKE => {
                    let i = pi(a, 0)?;
                    let p = h.sa_view_position();
                    h.sa_view_shake(&p, i);
                }
                TECHTREE_MODIFY_BUILDABILITY_OBJECT => {
                    let (ty, s) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(t) = h.sa_find_template(&ty) {
                        h.sa_logic_set_buildable_status_override(t, s);
                    }
                }
                SET_CAVE_INDEX => {
                    let (c, i) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(o) = h.sa_unit_named(&c) else { return Ok(()) };
                    if h.sa_obj_has_cave(o) {
                        h.sa_cave_try_set_index(o, i);
                    }
                }
                WAREHOUSE_SET_VALUE => {
                    let (w, v) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(o) = h.sa_unit_named(&w) else { return Ok(()) };
                    if h.sa_obj_has_warehouse(o) {
                        h.sa_warehouse_set_cash_value(o, v);
                    }
                }
                SOUND_DISABLE_TYPE | SOUND_ENABLE_TYPE => {
                    let n = ps(a, 0)?;
                    h.sa_audio_set_event_enabled(&n, a.action_type == SOUND_ENABLE_TYPE);
                }
                SOUND_ENABLE_ALL => h.sa_audio_set_event_enabled(b"", true),
                SOUND_REMOVE_ALL_DISABLED => h.sa_audio_remove_disabled_events(),
                SOUND_REMOVE_TYPE => {
                    let n = ps(a, 0)?;
                    h.sa_audio_remove_event_named(&n);
                }
                AUDIO_OVERRIDE_VOLUME_TYPE => {
                    let (n, v) = (ps(a, 0)?, pr(a, 1)?);
                    h.sa_audio_set_event_volume_override(&n, v / 100.0);
                }
                AUDIO_RESTORE_VOLUME_TYPE => {
                    let n = ps(a, 0)?;
                    // (-100.0 is a double, converted to the Real parameter)
                    h.sa_audio_set_event_volume_override(&n, -100.0 / 100.0);
                }
                AUDIO_RESTORE_VOLUME_ALL_TYPE => h.sa_audio_set_event_volume_override(b"", -100.0 / 100.0),
                NAMED_SET_TOPPLE_DIRECTION => {
                    let dir = parm(a, 1)?.get_coord3d();
                    let n = ps(a, 0)?;
                    h.sa_set_topple_direction(&n, &dir);
                }
                _ => return Err(ActionError::NotTranslated(a.action_type)),
            }
            Ok(())
        })();
        match r {
            Err(ActionError::NotTranslated(_)) => None,
            r => Some(r),
        }
    }

    // ------------------------------------------------------------------------------------------------------------------------
    // batch A8: lines 5160-5440
    // ------------------------------------------------------------------------------------------------------------------------

    /// The nearest object of `object_type` (a template, or else an object type list: the nearest of each template in it) within the
    /// trigger: `Err(())` when the type is a template with no such object (the caller returns), `Ok(None)` when nothing was found
    /// through a list.
    fn nearest_of_type<H: ScriptActionsHost>(h: &H, p: &Coord3D, object_type: &[u8], trig: TriggerH, obj: ObjH) -> Result<Option<ObjH>, ()> {
        if let Some(t) = h.sa_find_template(object_type) {
            return match h.sa_partition_closest_of_template(p, t, trig, obj) {
                Some((o, _)) => Ok(Some(o)),
                None => Err(()),
            };
        }
        let mut best: Option<(ObjH, f32)> = None;
        if let Some(types) = h.sa_object_types(object_type) {
            for name in types {
                let Some(t) = h.sa_find_template(&name) else { continue };
                if let Some((o, d)) = h.sa_partition_closest_of_template(p, t, trig, obj) {
                    if best.is_none_or(|(_, bd)| d < bd) {
                        best = Some((o, d));
                    }
                }
            }
        }
        Ok(best.map(|b| b.0))
    }

    /// `doMoveUnitTowardsNearest(unitName, objectType, triggerName)`.
    pub fn do_move_unit_towards_nearest<H: ScriptActionsHost>(&mut self, h: &mut H, unit: &[u8], object_type: &[u8], trigger: &[u8]) {
        let Some(o) = h.sa_unit_named(unit) else { return };
        if !h.sa_obj_has_ai(o) {
            return;
        }
        let Some(trig) = h.sa_trigger_named(trigger) else { return };
        let pos = h.sa_obj_position(o);
        let Ok(best) = Self::nearest_of_type(h, &pos, object_type, trig, o) else { return };
        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
        h.sa_ai_move_to_object(o, best, CMD_FROM_SCRIPT);
    }

    /// `doMoveTeamTowardsNearest(teamName, objectType, triggerName)`: from the team's estimated position (the map status of its first
    /// member with an AI, or its last member); every member moves until one has no AI (`return`).
    pub fn do_move_team_towards_nearest<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], object_type: &[u8], trigger: &[u8]) -> R<()> {
        let Some(t) = h.sa_team_named(team) else { return Ok(()) };
        let Some(trig) = h.sa_trigger_named(trigger) else { return Ok(()) };
        let members = h.sa_team_members(t);
        let mut team_obj = None;
        for &o in &members {
            team_obj = Some(o);
            if h.sa_obj_has_ai(o) {
                break;
            }
        }
        let Some(team_obj) = team_obj else { return Ok(()) };
        let pos = h.sa_team_estimate_position(t).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullLocation)))?;
        let Ok(best) = Self::nearest_of_type(h, &pos, object_type, trig, team_obj) else { return Ok(()) };
        for o in members {
            if !h.sa_obj_has_ai(o) {
                if plant(3883) {
                    continue;
                }
                return Ok(());
            }
            h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
            h.sa_ai_move_to_object(o, best, CMD_FROM_SCRIPT);
        }
        Ok(())
    }

    /// `doSkirmishAttackNearestGroupWithValue(teamName, comparison, value)`: the team attack-moves to the nearest enemy group worth at
    /// least the value; the position is uninitialised (undefined) unless the comparison is `>=` or `>` and a group was found (and the
    /// search itself reads the group's centre, unset for an empty group).
    pub fn do_skirmish_attack_nearest_group_with_value<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], cmp: i32, value: i32) -> R<()> {
        let uninit = || ActionError::Undefined(IniError::undefined(Undefined::UninitialisedLocal));
        let Some(t) = h.sa_team_named(team) else { return Ok(()) };
        let g = Self::team_group(h, t).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullObject)))?;
        let Some(p) = h.sa_team_controlling_player(t) else { return Ok(()) };
        let center = h.sa_group_center(g);
        let mut loc = None;
        if cmp == comparison::GREATER_EQUAL || cmp == comparison::GREATER {
            let c = center.ok_or_else(uninit)?;
            loc = h.sa_partition_nearest_group_with_value(p, &c, value, true);
        }
        let l = loc.ok_or_else(uninit)?;
        h.sa_group_attack_move_to_position(g, &l, NO_MAX_SHOTS_LIMIT, CMD_FROM_SCRIPT);
        Ok(())
    }

    /// `doSkirmishCommandButtonOnMostValuable(teamName, ability, range, allTeamMembers)`: the group uses the button on the most valuable
    /// target in range of its centre (`allTeamMembers` is not read).
    pub fn do_skirmish_command_button_on_most_valuable<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], ability: &[u8], range: f32) -> R<()> {
        let Some(t) = h.sa_team_named(team) else { return Ok(()) };
        let g = Self::team_group(h, t).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullObject)))?;
        if h.sa_team_controlling_player(t).is_none() {
            return Ok(());
        }
        let Some(b) = h.sa_find_command_button(ability) else { return Ok(()) };
        let src = match h.sa_button_special_power_id(b) {
            Some(id) => h.sa_group_special_power_source(g, id),
            None => h.sa_group_command_button_source(g, h.sa_button_command_type(b)),
        };
        if src.is_none() {
            return Ok(());
        }
        let pos = h.sa_group_center(g).ok_or(ActionError::Undefined(IniError::undefined(Undefined::UninitialisedLocal)))?;
        if let Some(target) = h.sa_partition_most_valuable_target(&pos, if plant(3884) { range + 1.0 } else { range }) {
            h.sa_group_do_command_button_at_object(g, b, target, CMD_FROM_SCRIPT);
        }
        Ok(())
    }

    /// The batch-A8 cases of `executeAction` (`None`: not one of them).
    fn execute_a8<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> Option<R<()>> {
        use at::*;
        let r = (|| -> R<()> {
            match a.action_type {
                UNIT_MOVE_TOWARDS_NEAREST_OBJECT_TYPE => {
                    let (u, ty, tr) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    self.do_move_unit_towards_nearest(h, &u, &ty, &tr);
                }
                TEAM_MOVE_TOWARDS_NEAREST_OBJECT_TYPE => {
                    let (t, ty, tr) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    self.do_move_team_towards_nearest(h, &t, &ty, &tr)?;
                }
                NAMED_RECEIVE_UPGRADE => {
                    let (u, up) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if let Some(up) = h.sa_find_upgrade(&up) {
                        h.sa_obj_give_upgrade(o, up);
                    }
                }
                SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE => {
                    let (t, c, v) = (ps(a, 0)?, pi(a, 1)?, pi(a, 2)?);
                    self.do_skirmish_attack_nearest_group_with_value(h, &t, c, v)?;
                }
                SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT => {
                    let (t, ab, range) = (ps(a, 0)?, ps(a, 1)?, pr(a, 2)?);
                    pi(a, 3)?;
                    self.do_skirmish_command_button_on_most_valuable(h, &t, &ab, range)?;
                }
                _ => return Err(ActionError::NotTranslated(a.action_type)),
            }
            Ok(())
        })();
        match r {
            Err(ActionError::NotTranslated(_)) => None,
            r => Some(r),
        }
    }

    // ------------------------------------------------------------------------------------------------------------------------
    // batch A9: lines 5436-5900
    // ------------------------------------------------------------------------------------------------------------------------

    /// The team's group (unchecked), the button by name, and the group's source object for it (by its special power, else its
    /// command type): `None` when there is no team, no button or no source.
    fn team_button_source<H: ScriptActionsHost>(h: &mut H, team: &[u8], ability: &[u8]) -> R<Option<(TeamH, GroupH, ButtonH, ObjH)>> {
        let Some(t) = h.sa_team_named(team) else { return Ok(None) };
        let g = Self::team_group(h, t).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullObject)))?;
        let Some(b) = h.sa_find_command_button(ability) else { return Ok(None) };
        let src = match h.sa_button_special_power_id(b) {
            Some(id) => h.sa_group_special_power_source(g, id),
            None => h.sa_group_command_button_source(g, h.sa_button_command_type(b)),
        };
        Ok(src.map(|s| (t, g, b, s)))
    }

    /// The group's centre (its source object is a member, so it is not empty).
    fn group_center<H: ScriptActionsHost>(h: &H, g: GroupH) -> R<Coord3D> {
        h.sa_group_center(g).ok_or(ActionError::Undefined(IniError::undefined(Undefined::UninitialisedLocal)))
    }

    /// `doTeamUseCommandButtonOnNearest*`: the group uses the button on the nearest object the search finds.
    fn team_button_on_nearest<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], ability: &[u8], search: ButtonSearch) -> R<()> {
        let Some((_, g, b, _)) = Self::team_button_source(h, team, ability)? else { return Ok(()) };
        let pos = Self::group_center(h, g)?;
        let Some((o, _)) = h.sa_partition_closest_for_button(&pos, search) else { return Ok(()) };
        h.sa_group_do_command_button_at_object(g, b, o, CMD_FROM_SCRIPT);
        Ok(())
    }

    /// `doTeamUseCommandButtonOnNearestObjectType(teamName, ability, objectType)`: a template, or the nearest of each template of an
    /// object type list (the button is then used on NULL when none is found).
    pub fn do_team_use_command_button_on_nearest_object_type<H: ScriptActionsHost>(&mut self, h: &mut H, team: &[u8], ability: &[u8], object_type: &[u8]) -> R<()> {
        let Some((_, g, b, _)) = Self::team_button_source(h, team, ability)? else { return Ok(()) };
        let best;
        if let Some(t) = h.sa_find_template(object_type) {
            let pos = Self::group_center(h, g)?;
            let Some((o, _)) = h.sa_partition_closest_for_button(&pos, ButtonSearch::Template(t)) else { return Ok(()) };
            best = Some(o);
        } else {
            let mut found: Option<(ObjH, f32)> = None;
            if let Some(types) = h.sa_object_types(object_type) {
                let pos = Self::group_center(h, g)?;
                for name in types {
                    let Some(t) = h.sa_find_template(&name) else { continue };
                    if let Some((o, d)) = h.sa_partition_closest_for_button(&pos, ButtonSearch::Template(t)) {
                        if found.is_none_or(|(_, bd)| d < bd) {
                            found = Some((o, d));
                        }
                    }
                }
            }
            best = found.map(|f| f.0);
        }
        h.sa_group_do_command_button_at_object_opt(g, b, best, CMD_FROM_SCRIPT);
        Ok(())
    }

    /// `doTeamPartialUseCommandButton(percentage, teamName, ability)`: the first `percentage/100*n` of the members the button is
    /// valid for use it (the count a `Real` truncated to an `Int`).
    pub fn do_team_partial_use_command_button<H: ScriptActionsHost>(&mut self, h: &mut H, percentage: f32, team: &[u8], ability: &[u8]) -> R<()> {
        let Some(t) = h.sa_team_named(team) else { return Ok(()) };
        let Some(b) = h.sa_find_command_button(ability) else { return Ok(()) };
        let list: Vec<ObjH> = h.sa_team_members(t).into_iter().filter(|o| h.sa_button_is_valid_to_use_on(b, *o, None, CMD_FROM_SCRIPT)).collect();
        let n = real_to_int(percentage / 100.0 * list.len() as f32)?;
        for (count, o) in list.into_iter().enumerate() {
            if count as i32 >= n + i32::from(plant(3885)) {
                return Ok(());
            }
            h.sa_obj_do_command_button(o, b, CMD_FROM_SCRIPT);
        }
        Ok(())
    }

    /// The batch-A9 cases of `executeAction` (`None`: not one of them).
    fn execute_a9<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> Option<R<()>> {
        use at::*;
        let r = (|| -> R<()> {
            match a.action_type {
                TEAM_SPIN_FOR_FRAMECOUNT => {
                    let (t, f) = (ps(a, 0)?, pi(a, 1)?);
                    if let Some(t) = h.sa_team_named(&t) {
                        h.sa_set_sequential_timer_team(t, f);
                    }
                }
                TEAM_ALL_USE_COMMANDBUTTON_ON_NAMED => {
                    let (t, ab, u) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    let Some((_, g, b, src)) = Self::team_button_source(h, &t, &ab)? else { return Ok(()) };
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if h.sa_button_is_valid_to_use_on(b, src, Some(o), CMD_FROM_SCRIPT) {
                        h.sa_group_do_command_button_at_object(g, b, o, CMD_FROM_SCRIPT);
                    }
                }
                TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_UNIT => {
                    let (t, ab) = (ps(a, 0)?, ps(a, 1)?);
                    self.team_button_on_nearest(h, &t, &ab, ButtonSearch::Enemy)?;
                }
                TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_GARRISONED_BUILDING => {
                    let (t, ab) = (ps(a, 0)?, ps(a, 1)?);
                    self.team_button_on_nearest(h, &t, &ab, ButtonSearch::GarrisonedBuilding)?;
                }
                TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_KINDOF => {
                    let (t, ab, k) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    kind_of_bit(k)?;
                    self.team_button_on_nearest(h, &t, &ab, ButtonSearch::KindOf(k))?;
                }
                TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING => {
                    let (t, ab) = (ps(a, 0)?, ps(a, 1)?);
                    self.team_button_on_nearest(h, &t, &ab, ButtonSearch::Building)?;
                }
                TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING_CLASS => {
                    let (t, ab, k) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    kind_of_bit(k)?;
                    self.team_button_on_nearest(h, &t, &ab, ButtonSearch::BuildingClass(k))?;
                }
                TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_OBJECTTYPE => {
                    let (t, ab, ty) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    self.do_team_use_command_button_on_nearest_object_type(h, &t, &ab, &ty)?;
                }
                TEAM_PARTIAL_USE_COMMANDBUTTON => {
                    let (pc, t, ab) = (pr(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    self.do_team_partial_use_command_button(h, pc, &t, &ab)?;
                }
                TEAM_CAPTURE_NEAREST_UNOWNED_FACTION_UNIT => {
                    let t = ps(a, 0)?;
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let g = Self::team_group(h, t).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullObject)))?;
                    let pos = Self::group_center(h, g)?;
                    if let Some((o, _)) = h.sa_partition_closest_for_button(&pos, ButtonSearch::Unmanned) {
                        h.sa_group_enter(g, o, CMD_FROM_SCRIPT);
                    }
                }
                PLAYER_CREATE_TEAM_FROM_CAPTURED_UNITS => {
                    let (_p, _t) = (ps(a, 0)?, ps(a, 1)?);
                    // (doCreateTeamFromCapturedUnits only looks the team up)
                }
                _ => return Err(ActionError::NotTranslated(a.action_type)),
            }
            Ok(())
        })();
        match r {
            Err(ActionError::NotTranslated(_)) => None,
            r => Some(r),
        }
    }

    // ------------------------------------------------------------------------------------------------------------------------
    // batch A10: lines 5895-6405 and the rest of executeAction
    // ------------------------------------------------------------------------------------------------------------------------

    /// `doEnableOrDisableObjectDifficultyBonuses(enableBonuses)`: every object, then the script engine (the script engine calls it
    /// after a load).
    pub fn do_enable_or_disable_object_difficulty_bonuses<H: ScriptActionsHost>(&mut self, h: &mut H, enable: bool) {
        for o in h.sa_all_objects() {
            h.sa_obj_set_receiving_difficulty_bonus(o, enable);
        }
        h.sa_set_objects_should_receive_difficulty_bonus(enable != plant(3886));
    }

    /// The face actions: each unit with an AI cleared, out of its group, facing the object or the position.
    fn face<H: ScriptActionsHost>(h: &mut H, o: ObjH, target: Result<ObjH, Coord3D>) {
        if !h.sa_obj_has_ai(o) {
            return;
        }
        if !plant(3887) {
            h.sa_ai_clear_waypoint_queue(o);
        }
        h.sa_obj_leave_group(o);
        h.sa_ai_choose_locomotor_set(o, LOCOMOTORSET_NORMAL);
        match target {
            Ok(t) => h.sa_ai_face_object(o, t, CMD_FROM_SCRIPT),
            Err(p) => h.sa_ai_face_position(o, &p, CMD_FROM_SCRIPT),
        }
    }

    /// `(Int)(seconds * LOGICFRAMES_PER_SECOND)` (a `Real` truncated).
    fn frames_of(seconds: f32) -> R<i32> {
        real_to_int(seconds * LOGICFRAMES_PER_SECOND as f32)
    }

    /// The batch-A10 cases of `executeAction` (`None`: not one of them).
    fn execute_a10<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> Option<R<()>> {
        use at::*;
        let r = (|| -> R<()> {
            match a.action_type {
                CAMERA_MOD_FREEZE_TIME | CAMERA_MOD_FREEZE_ANGLE => h.sa_view_camera_mod_freeze(a.action_type == CAMERA_MOD_FREEZE_TIME),
                CAMERA_MOD_SET_FINAL_ZOOM | CAMERA_MOD_SET_FINAL_PITCH => {
                    let (v, e1, e2) = (pr(a, 0)?, pr(a, 1)?, pr(a, 2)?);
                    if a.action_type == CAMERA_MOD_SET_FINAL_ZOOM {
                        h.sa_view_camera_mod_final_zoom(v, e1, e2);
                    } else {
                        h.sa_view_camera_mod_final_pitch(v, e1, e2);
                    }
                }
                CAMERA_MOD_SET_FINAL_SPEED_MULTIPLIER => {
                    let m = pi(a, 0)?;
                    h.sa_view_camera_mod_final_time_multiplier(m);
                }
                CAMERA_MOD_SET_ROLLING_AVERAGE => {
                    let n = pi(a, 0)?;
                    h.sa_view_camera_mod_rolling_average(n);
                }
                SET_VISUAL_SPEED_MULTIPLIER => {
                    let m = pi(a, 0)?;
                    h.sa_view_set_time_multiplier(m);
                }
                SUSPEND_BACKGROUND_SOUNDS | RESUME_BACKGROUND_SOUNDS => h.sa_audio_pause(a.action_type == SUSPEND_BACKGROUND_SOUNDS, AUDIO_AFFECT_SOUND),
                CAMERA_MOTION_BLUR_FOLLOW => {
                    let m = pi(a, 0)?;
                    let mode = FM_VIEW_MB_PAN_ALPHA.checked_add(m).ok_or(ActionError::Undefined(IniError::undefined(Undefined::SignedIntOverflow)))?;
                    h.sa_view_set_view_filter_mode(mode);
                    h.sa_view_set_view_filter(FT_VIEW_MOTION_BLUR_FILTER);
                }
                CAMERA_MOTION_BLUR_END_FOLLOW => {
                    h.sa_view_set_view_filter_mode(FM_VIEW_MB_END_PAN_ALPHA);
                    h.sa_view_set_view_filter(FT_VIEW_MOTION_BLUR_FILTER);
                }
                SET_FPS_LIMIT => {
                    let f = pi(a, 0)?;
                    let fps = if f == 0 { h.sa_global_fps_limit() } else { pi(a, 0)? };
                    h.sa_engine_set_fps_limit(fps);
                    h.sa_set_use_fps_limit(true);
                }
                SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_ALL | SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_PARTIAL | TEAM_WAIT_FOR_NOT_CONTAINED_ALL
                | TEAM_WAIT_FOR_NOT_CONTAINED_PARTIAL => {}
                PLAYER_ADD_SKILLPOINTS | PLAYER_ADD_RANKLEVEL | PLAYER_SET_RANKLEVEL | PLAYER_SELECT_SKILLSET => {
                    let (p, n) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(p) = h.sa_player_from_name(&p) else { return Ok(()) };
                    let overflow = || ActionError::Undefined(IniError::undefined(Undefined::SignedIntOverflow));
                    match a.action_type {
                        PLAYER_ADD_SKILLPOINTS => h.sa_player_add_skill_points(p, n),
                        PLAYER_ADD_RANKLEVEL => {
                            let l = h.sa_player_rank_level(p).checked_add(n).ok_or_else(overflow)?;
                            h.sa_player_set_rank_level(p, l);
                        }
                        PLAYER_SET_RANKLEVEL => h.sa_player_set_rank_level(p, n),
                        _ => h.sa_player_set_skillset(p, n.checked_sub(1).ok_or_else(overflow)?),
                    }
                }
                PLAYER_SET_RANKLEVELLIMIT => {
                    let l = pi(a, 0)?;
                    h.sa_logic_set_rank_level_limit(l);
                }
                PLAYER_GRANT_SCIENCE | PLAYER_PURCHASE_SCIENCE => {
                    let (p, s) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(p) = h.sa_player_from_name(&p) else { return Ok(()) };
                    let Some(sci) = h.sa_science_from_name(&s) else { return Ok(()) };
                    if a.action_type == PLAYER_GRANT_SCIENCE {
                        h.sa_player_grant_science(p, sci);
                    } else {
                        h.sa_player_purchase_science(p, sci);
                    }
                }
                PLAYER_SCIENCE_AVAILABILITY => {
                    let (p, s, av) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                    let Some(p) = h.sa_player_from_name(&p) else { return Ok(()) };
                    let Some(t) = h.sa_player_science_availability_from_string(p, &av) else { return Ok(()) };
                    if let Some(sci) = h.sa_science_from_name(&s) {
                        h.sa_player_set_science_availability(p, sci, t);
                    }
                }
                TEAM_SET_EMOTICON => {
                    let (t, e, d) = (ps(a, 0)?, ps(a, 1)?, pr(a, 2)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let Some(g) = Self::team_group(h, t) else { return Ok(()) };
                    let frames = Self::frames_of(d)?;
                    h.sa_group_set_emoticon(g, &e, frames);
                }
                NAMED_SET_EMOTICON => {
                    let (u, e, d) = (ps(a, 0)?, ps(a, 1)?, pr(a, 2)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if h.sa_obj_has_drawable(o) {
                        let frames = Self::frames_of(d)?;
                        h.sa_drawable_set_emoticon(o, &e, frames);
                    }
                }
                OBJECTLIST_ADDOBJECTTYPE | OBJECTLIST_REMOVEOBJECTTYPE => {
                    let (l, t) = (ps(a, 0)?, ps(a, 1)?);
                    h.sa_object_type_list_maintenance(&l, &t, a.action_type == OBJECTLIST_ADDOBJECTTYPE);
                }
                MAP_REVEAL_PERMANENTLY_AT_WAYPOINT => {
                    let (w, r, p, look) = (ps(a, 0)?, pr(a, 1)?, ps(a, 2)?, ps(a, 3)?);
                    h.sa_named_map_reveal(&look, &w, r, &p);
                }
                MAP_UNDO_REVEAL_PERMANENTLY_AT_WAYPOINT => {
                    let look = ps(a, 0)?;
                    h.sa_undo_named_map_reveal(&look);
                }
                EVA_SET_ENABLED_DISABLED => {
                    let e = pi(a, 0)?;
                    h.sa_eva_set_enabled(e != 0);
                }
                OPTIONS_SET_OCCLUSION_MODE | OPTIONS_SET_DRAWICON_UI_MODE | OPTIONS_SET_PARTICLE_CAP_MODE => {
                    let e = pi(a, 0)?;
                    let which = match a.action_type {
                        OPTIONS_SET_OCCLUSION_MODE => 0,
                        OPTIONS_SET_DRAWICON_UI_MODE => 1,
                        _ => 2,
                    };
                    h.sa_logic_set_option(which, e != 0);
                }
                CAMERA_ENABLE_SLAVE_MODE => {
                    let (t, b) = (ps(a, 0)?, ps(a, 1)?);
                    h.sa_view_camera_slave_mode(Some((&t, &b)));
                }
                CAMERA_DISABLE_SLAVE_MODE => h.sa_view_camera_slave_mode(None),
                CAMERA_ADD_SHAKER_AT => {
                    let (w, amp, dur, radius) = (ps(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?);
                    // (the waypoint is used unchecked)
                    let w = h.sa_waypoint_by_name(&w).ok_or(ActionError::Undefined(IniError::undefined(Undefined::NullLocation)))?;
                    let p = h.sa_waypoint_location(w);
                    h.sa_view_add_camera_shake(&p, radius, dur, amp);
                }
                SCRIPTING_OVERRIDE_HULK_LIFETIME => {
                    let s = pr(a, 0)?;
                    let frames = if s < 0.0 { -1 } else { Self::frames_of(s)? };
                    h.sa_logic_set_hulk_max_lifetime_override(frames);
                }
                NAMED_FACE_NAMED | NAMED_FACE_WAYPOINT => {
                    let (u, f) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    let target = if a.action_type == NAMED_FACE_NAMED {
                        let Some(t) = h.sa_unit_named(&f) else { return Ok(()) };
                        Ok(t)
                    } else {
                        let Some(w) = h.sa_waypoint_by_name(&f) else { return Ok(()) };
                        Err(h.sa_waypoint_location(w))
                    };
                    Self::face(h, o, target);
                }
                TEAM_FACE_NAMED | TEAM_FACE_WAYPOINT => {
                    let (t, f) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    let target = if a.action_type == TEAM_FACE_NAMED {
                        let Some(o) = h.sa_unit_named(&f) else { return Ok(()) };
                        Ok(o)
                    } else {
                        let Some(w) = h.sa_waypoint_by_name(&f) else { return Ok(()) };
                        Err(h.sa_waypoint_location(w))
                    };
                    for o in h.sa_team_members(t) {
                        Self::face(h, o, target);
                    }
                }
                UNIT_AFFECT_OBJECT_PANEL_FLAGS => {
                    let (u, f, e) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    if let Some(o) = h.sa_unit_named(&u) {
                        self.change_object_panel_flag_for_single_object(h, o, &f, e != 0);
                    }
                }
                TEAM_AFFECT_OBJECT_PANEL_FLAGS => {
                    let (t, f, e) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    for o in h.sa_team_members(t) {
                        self.change_object_panel_flag_for_single_object(h, o, &f, e != 0);
                    }
                }
                COMMANDBAR_REMOVE_BUTTON_OBJECTTYPE => {
                    let (b, ty) = (ps(a, 0)?, ps(a, 1)?);
                    let Some(t) = h.sa_find_template(&ty) else { return Ok(()) };
                    let cs = h.sa_template_command_set_string(t);
                    let Some(set) = h.sa_find_command_set(&cs) else { return Ok(()) };
                    let slot = (0..MAX_COMMANDS_PER_SET).find(|i| h.sa_command_set_button(set, *i).is_some_and(|bt| h.sa_button_name(bt) == b));
                    if let Some(s) = slot {
                        h.sa_logic_set_control_bar_override(&cs, s, None);
                    }
                }
                COMMANDBAR_ADD_BUTTON_OBJECTTYPE_SLOT => {
                    let (b, ty, slot) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                    let Some(t) = h.sa_find_template(&ty) else { return Ok(()) };
                    let Some(bt) = h.sa_find_command_button(&b) else { return Ok(()) };
                    let Some(s) = slot.checked_sub(1) else { return Err(IniError::undefined(Undefined::SignedIntOverflow).into()) };
                    if !(0..MAX_COMMANDS_PER_SET).contains(&s) {
                        return Ok(());
                    }
                    let cs = h.sa_template_command_set_string(t);
                    h.sa_logic_set_control_bar_override(&cs, s, Some(bt));
                }
                PLAYER_AFFECT_RECEIVING_EXPERIENCE => {
                    let (p, m) = (ps(a, 0)?, pr(a, 1)?);
                    if let Some(p) = h.sa_player_from_name(&p) {
                        h.sa_player_set_skill_points_modifier(p, m);
                    }
                }
                TEAM_GUARD_SUPPLY_CENTER => {
                    let (t, s) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    if let Some(p) = h.sa_team_controlling_player(t) {
                        h.sa_player_guard_supply_center(p, t, s);
                    }
                }
                OBJECT_ALLOW_BONUSES => {
                    let e = pi(a, 0)?;
                    self.do_enable_or_disable_object_difficulty_bonuses(h, e != 0);
                }
                TEAM_GUARD_IN_TUNNEL_NETWORK => {
                    let t = ps(a, 0)?;
                    let Some(t) = h.sa_team_named(&t) else { return Ok(()) };
                    for o in h.sa_team_members(t) {
                        if h.sa_obj_has_ai(o) {
                            h.sa_ai_guard_tunnel_network(o, GUARDMODE_NORMAL, CMD_FROM_SCRIPT);
                        }
                    }
                }
                RESIZE_VIEW_GUARDBAND => {
                    let (x, y) = (pr(a, 0)?, pr(a, 1)?);
                    h.sa_view_set_guard_band_bias(x, y);
                }
                DELETE_ALL_UNMANNED => {
                    for o in h.sa_all_objects() {
                        if h.sa_obj_is_disabled_by_type(o, DISABLED_UNMANNED) {
                            h.sa_logic_destroy_object(o);
                        }
                    }
                }
                CHOOSE_VICTIM_ALWAYS_USES_NORMAL => {
                    let e = pi(a, 0)?;
                    h.sa_set_choose_victim_always_uses_normal(e != 0);
                }
                SET_TRAIN_HELD => {
                    let (u, held) = (ps(a, 0)?, pi(a, 1)?);
                    let Some(o) = h.sa_unit_named(&u) else { return Ok(()) };
                    if h.sa_obj_has_railroad(o) {
                        h.sa_railroad_set_held(o, held != 0);
                    }
                }
                _ => return Err(ActionError::NotTranslated(a.action_type)),
            }
            Ok(())
        })();
        match r {
            Err(ActionError::NotTranslated(_)) => None,
            r => Some(r),
        }
    }

    /// The player an AI build order of the current player goes to.
    fn current_player<H: ScriptActionsHost>(h: &H) -> Option<PlayerH> {
        h.sa_current_player()
    }

    /// `executeAction(pAction)`. See the module docs for what is translated.
    pub fn execute_action<H: ScriptActionsHost>(&mut self, h: &mut H, a: &ScriptAction) -> R<()> {
        use at::*;
        match a.action_type {
            DEBUG_MESSAGE_BOX => h.sa_append_debug_message(&ps(a, 0)?, true),
            DEBUG_STRING => h.sa_append_debug_message(&ps(a, 0)?, false),
            DAMAGE_MEMBERS_OF_TEAM => {
                let (team, amount) = (ps(a, 0)?, pr(a, 1)?);
                if let Some(t) = h.sa_team_named(&team) {
                    h.sa_team_damage_members(t, amount);
                }
            }
            MOVE_TEAM_TO => {
                let (team, way) = (ps(a, 0)?, ps(a, 1)?);
                self.do_move_to_waypoint(h, &team, &way);
            }
            MOVE_NAMED_UNIT_TO => {
                let (unit, way) = (ps(a, 0)?, ps(a, 1)?);
                self.do_named_move_to_waypoint(h, &unit, &way);
            }
            TEAM_SET_STATE => {
                let (team, state) = (ps(a, 0)?, ps(a, 1)?);
                if let Some(t) = h.sa_team_named(&team) {
                    h.sa_team_set_state(t, &state);
                }
            }
            CREATE_REINFORCEMENT_TEAM => {
                let (team, way) = (ps(a, 0)?, ps(a, 1)?);
                self.do_create_reinforcements(h, &team, &way)?;
            }
            SKIRMISH_BUILD_BUILDING => {
                let b = ps(a, 0)?;
                if let Some(p) = Self::current_player(h) {
                    h.sa_player_build_specific_building(p, &b);
                }
            }
            AI_PLAYER_BUILD_SUPPLY_CENTER => {
                let (player, building, cash) = (ps(a, 0)?, ps(a, 1)?, pi(a, 2)?);
                if let Some(p) = h.sa_player_from_name(&player) {
                    h.sa_player_build_by_supplies(p, cash, &building);
                }
            }
            AI_PLAYER_BUILD_TYPE_NEAREST_TEAM => {
                let (player, building, team) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                let p = h.sa_player_from_name(&player);
                let t = h.sa_team_named(&team);
                if let (Some(p), Some(t)) = (p, t) {
                    h.sa_player_build_specific_building_nearest_team(p, &building, t);
                }
            }
            AI_PLAYER_BUILD_UPGRADE => {
                let (player, upgrade) = (ps(a, 0)?, ps(a, 1)?);
                if let Some(p) = h.sa_player_from_name(&player) {
                    h.sa_player_build_upgrade(p, &upgrade);
                }
            }
            SKIRMISH_BUILD_BASE_DEFENSE_FRONT | SKIRMISH_BUILD_BASE_DEFENSE_FLANK => {
                if let Some(p) = Self::current_player(h) {
                    h.sa_player_build_base_defense(p, a.action_type == SKIRMISH_BUILD_BASE_DEFENSE_FLANK);
                }
            }
            SKIRMISH_BUILD_STRUCTURE_FRONT | SKIRMISH_BUILD_STRUCTURE_FLANK => {
                let b = ps(a, 0)?;
                if let Some(p) = Self::current_player(h) {
                    h.sa_player_build_base_defense_structure(p, &b, a.action_type == SKIRMISH_BUILD_STRUCTURE_FLANK);
                }
            }
            PLAY_SOUND_EFFECT => {
                let s = ps(a, 0)?;
                self.do_play_sound_effect(h, &s, None)?;
            }
            PLAY_SOUND_EFFECT_AT => {
                let (s, w) = (ps(a, 0)?, ps(a, 1)?);
                self.do_play_sound_effect(h, &s, Some(&w))?;
            }
            MOVE_CAMERA_TO => {
                let (w, s1, s2, e1, e2) = (ps(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?, pr(a, 4)?);
                self.do_move_camera_to(h, &w, s1, s2, e1, e2)?;
            }
            SETUP_CAMERA => {
                let (w, zoom, pitch, look) = (ps(a, 0)?, pr(a, 1)?, pr(a, 2)?, ps(a, 3)?);
                self.do_setup_camera(h, &w, zoom, pitch, &look);
            }
            ZOOM_CAMERA => {
                let (z, s, e1, e2) = (pr(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?);
                h.sa_view_zoom_camera(z, real_to_int(s * 1000.0)?, e1 * 1000.0, e2 * 1000.0);
            }
            PITCH_CAMERA => {
                let (p, s, e1, e2) = (pr(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?);
                h.sa_view_pitch_camera(p, real_to_int(s * 1000.0)?, e1 * 1000.0, e2 * 1000.0);
            }
            CAMERA_FOLLOW_NAMED => {
                let unit = ps(a, 0)?;
                let snap = a.get_parameter(1).is_some_and(|p| p.int != 0);
                self.do_camera_follow_named(h, &unit, snap);
            }
            CAMERA_STOP_FOLLOW => h.sa_view_set_camera_lock(0),
            OVERSIZE_TERRAIN => {
                let n = pi(a, 0)?;
                self.do_oversize_the_terrain(h, n);
            }
            CAMERA_MOD_LOOK_TOWARD | CAMERA_MOD_FINAL_LOOK_TOWARD => {
                let w = ps(a, 0)?;
                if let Some(w) = Self::waypoint_named_in_order(h, &w) {
                    let d = h.sa_waypoint_location(w);
                    if a.action_type == CAMERA_MOD_LOOK_TOWARD {
                        h.sa_view_camera_mod_look_toward(&d);
                    } else {
                        h.sa_view_camera_mod_final_look_toward(&d);
                    }
                }
            }
            MOVE_CAMERA_ALONG_WAYPOINT_PATH => {
                let (w, s1, s2, e1, e2) = (ps(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?, pr(a, 4)?);
                if let Some(w) = Self::waypoint_named_in_order(h, &w) {
                    h.sa_view_move_camera_along_waypoint_path(w, real_to_int(s1 * 1000.0)?, real_to_int(s2 * 1000.0)?, true, e1 * 1000.0, e2 * 1000.0);
                }
            }
            ROTATE_CAMERA => {
                let (r, s, e1, e2) = (pr(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?);
                h.sa_view_rotate_camera(r, real_to_int(s * 1000.0)?, e1 * 1000.0, e2 * 1000.0);
            }
            CAMERA_LOOK_TOWARD_OBJECT => {
                let (u, s, hold, e1, e2) = (ps(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?, pr(a, 4)?);
                if let Some(o) = h.sa_unit_named(&u) {
                    h.sa_view_rotate_camera_toward_object(o, real_to_int(s * 1000.0)?, real_to_int(hold * 1000.0)?, e1 * 1000.0, e2 * 1000.0);
                }
            }
            CAMERA_LOOK_TOWARD_WAYPOINT => {
                let (w, s, e1, e2, rev) = (ps(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?, pi(a, 4)?);
                if let Some(w) = h.sa_waypoint_by_name(&w) {
                    let p = h.sa_waypoint_location(w);
                    h.sa_view_rotate_camera_toward_position(&p, real_to_int(s * 1000.0)?, e1 * 1000.0, e2 * 1000.0, rev != 0);
                }
            }
            RESET_CAMERA => {
                let (w, s, e1, e2) = (ps(a, 0)?, pr(a, 1)?, pr(a, 2)?, pr(a, 3)?);
                if let Some(w) = Self::waypoint_named_in_order(h, &w) {
                    let d = h.sa_waypoint_location(w);
                    h.sa_view_reset_camera(&d, real_to_int(s * 1000.0)?, e1 * 1000.0, e2 * 1000.0);
                }
            }
            MOVE_CAMERA_TO_SELECTION => self.do_mod_camera_move_to_selection(h),
            SET_INFANTRY_LIGHTING_OVERRIDE => {
                let s = pr(a, 0)?;
                self.do_set_infantry_lighting_override(h, s);
            }
            RESET_INFANTRY_LIGHTING_OVERRIDE => self.do_set_infantry_lighting_override(h, -1.0),
            QUICKVICTORY => self.do_quick_victory(h),
            VICTORY => self.do_victory_or_defeat(h, true)?,
            DEFEAT => self.do_victory_or_defeat(h, false)?,
            LOCALDEFEAT => self.do_local_defeat(h),
            CREATE_OBJECT => {
                let pos = parm(a, 2)?.get_coord3d();
                let (thing, team, angle) = (ps(a, 0)?, ps(a, 1)?, pr(a, 3)?);
                let name = self.unnamed_unit.clone();
                self.do_create_object(h, &name, &thing, &team, &pos, angle);
            }
            UNIT_SPAWN_NAMED_LOCATION_ORIENTATION => {
                let pos = parm(a, 3)?.get_coord3d();
                let (name, thing, team, angle) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?, pr(a, 4)?);
                self.do_create_object(h, &name, &thing, &team, &pos, angle);
            }
            TEAM_ATTACK_TEAM => {
                let (x, y) = (ps(a, 0)?, ps(a, 1)?);
                self.do_attack(h, &x, &y);
            }
            NAMED_ATTACK_NAMED => {
                let (x, y) = (ps(a, 0)?, ps(a, 1)?);
                self.do_named_attack(h, &x, &y);
            }
            CREATE_NAMED_ON_TEAM_AT_WAYPOINT => {
                let (n, t, team, w) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?, ps(a, 3)?);
                self.create_unit_on_team_at(h, &n, &t, &team, &w);
            }
            CREATE_UNNAMED_ON_TEAM_AT_WAYPOINT => {
                let (t, team, w) = (ps(a, 0)?, ps(a, 1)?, ps(a, 2)?);
                self.create_unit_on_team_at(h, b"", &t, &team, &w);
            }
            DEBUG_CRASH_BOX => {}
            ENABLE_OBJECT_SOUND | DISABLE_OBJECT_SOUND => {
                let n = ps(a, 0)?;
                self.do_enable_object_sound(h, &n, a.action_type == ENABLE_OBJECT_SOUND);
            }
            _ => {
                if let Some(r) = self.execute_a2(h, a) {
                    return r;
                }
                if let Some(r) = self.execute_a3(h, a) {
                    return r;
                }
                if let Some(r) = self.execute_a4(h, a) {
                    return r;
                }
                if let Some(r) = self.execute_a5(h, a) {
                    return r;
                }
                if let Some(r) = self.execute_a6(h, a) {
                    return r;
                }
                if let Some(r) = self.execute_a7(h, a) {
                    return r;
                }
                if let Some(r) = self.execute_a8(h, a) {
                    return r;
                }
                if let Some(r) = self.execute_a9(h, a) {
                    return r;
                }
                if let Some(r) = self.execute_a10(h, a) {
                    return r;
                }
                // default: a type upstream has no case for (DEBUG_CRASH in debug builds) does nothing
            }
        }
        Ok(())
    }
}
