// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ScriptEngine.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp (lines 143-362 and 5270-9350)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ScriptEngine` (`TheScriptEngine`) at run time: each frame it counts the countdown timers down, evaluates every side's scripts
//! (the conditions it answers itself, flags, counters and timers, and the rest through `TheScriptConditions`) and executes the
//! actions of those that fire (its own: counters, flags, timers, fades, sway, attack priority sets, enabling, disabling and calling
//! scripts; the rest through `TheScriptActions`), and progresses the sequential scripts of units and teams. It keeps the named
//! objects' cache, the notifications the conditions ask about (completed videos, speech, audio, special powers, upgrades,
//! sciences, user interface hooks), the per-player object counts, the named map reveals, the topple directions, the object type
//! lists and the fade.
//!
//! The scripts themselves stay in the sides list (`TheSidesList->getSideInfo(i)->getScriptList()`): the host owns them and the
//! engine names a script by its place ([`ScriptRef`]). A condition handed to the host is evaluated on a copy that is then written
//! back: condition evaluation never runs scripts, so nothing reads the condition in between. The world is reached through
//! [`ScriptEngineHost`], one method per upstream call.
//!
//! # Upstream facts kept
//!
//! - A counter, flag or timer parameter's cached index is used unchecked (`m_counters[ndx]`): an index a map stores outside
//!   `0..256` is undefined ([`Undefined::ArrayIndexOutOfRange`]); with all 255 counters (or flags) taken, a new name gets index 0.
//! - A missing parameter is dereferenced ([`Undefined::NullParameter`]), as are `notifyOf*`'s player index (unchecked, unlike the
//!   `is*` questions) and `setToppleDirection`'s direction when the name is new.
//! - `evaluateFlag` is also true while a user interface hook of the flag's name was signalled this frame.
//! - `executeScript` with a condition team iterates the team's instances and does not clear a one-shot script whose false actions
//!   ran; without one it does.
//! - `setFade` with no frames to increase updates the fade once at once; `updateFades`' decrease divides by `frames + 1`.
//! - The sequential scripts: an instruction past the end ends the script (looping copies it to the end of its chain first);
//!   `SKIRMISH_WAIT_FOR_COMMANDBUTTON_*` and `TEAM_WAIT_FOR_NOT_CONTAINED_*` are evaluated, not executed; a team's AI group is made
//!   anew each frame (and again after an action). An action that adds a chain while they are progressed may reallocate the vector
//!   under the loop's iterator, and one that removes a chain at or before the loop's place invalidates it: both are undefined
//!   ([`Undefined::InvalidatedIterator`]); the capacity grows as `size + max(size, 1)` (MSVC's and libstdc++'s policy alike).
//! - Subroutines call each other without a limit: a call that does not return overflows the stack, taken here as
//!   [`MAX_CALL_DEPTH`] nested calls ([`Undefined::StackOverflow`]).
//! - `notifyOfTeamDestruction`, `getTeamNamed` and the others as written: `THIS_TEAM` is the calling team, else the condition team.
//!
//! Not translated: the script debugger and particle editor windows (`DebugWindow.dll`, `ParticleEditor.dll`, loaded only by
//! developer command line switches: without them `AppendDebugMessage`, `_appendMessage`, `_adjustVariable`,
//! `_updateFrameNumber` and `particleEditorUpdate` do nothing, and `isTimeFrozenDebug` and `isTimeFast` are false), VTune, the
//! profiling of `SPECIAL_SCRIPT_PROFILING` and `getStats` (internal builds), and the counter of `getTeamNamed`'s warnings (it only
//! limits debug messages). The text built for those messages (`getUiText`) has no effect and is not built.
//!
//! # Save and load (batch E4)
//!
//! [`ScriptEngine::xfer`] (version 5) and [`ScriptEngine::load_post_process`], with `SequentialScript::xfer`,
//! `AttackPriorityInfo::xfer` and the list helpers. Upstream facts kept: only the head of each sequential script chain is saved
//! (what follows it is lost on load); the attack priority map is saved in the order of its keys, template pointers upstream (the
//! host's handle order here); a count is an `UnsignedShort` (a longer list saves a wrapped count and every element); a named
//! object, team or template that is gone, a list that is not empty, or a count of players other than 16 throws `SC_INVALID_DATA`
//! (here [`XferStatus::ErrorUnknown`]), an unknown science `XFER_UNKNOWN_STRING`; a script that is gone loads as a NULL script,
//! and counter, flag and priority counts load unchecked: both are undefined at their first use, reported as [`LOAD_UNDEFINED`];
//! the named reveals load through one `NamedReveal`; a load with no fade starts the fade in from black.
//!
//! Differential-verified in `tests/differential_script_engine.rs` (the file with three mechanical edits about the debugger and
//! particle editor DLLs, on stand-ins): 20,000 programs, 330,759 operations (saves compared byte for byte, and loads), 0
//! mismatches; 19 planted bugs (3901-3926), all caught.
//!
//! `p1w13-sonnet`, layer 11, batches E2 and E4, 2026-10-31.

use std::collections::BTreeMap;

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::{real_to_int_ceil, real_to_unsigned_int, Coord2D, Coord3D, PI};
use crate::common::game_common::{convert_duration_from_msecs_to_frames, MSEC_PER_LOGICFRAME_REAL};
use crate::common::game_type::ObjectId;
use crate::common::ini::{IniError, Undefined};
use crate::common::name_key_generator::NameKeyType;
use crate::common::trig::{cos, sin};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::modules::plant;
use crate::logic::object_types::ObjectTypes;
use crate::logic::scripts::{action_type as at, condition_type as ct, parameter_type, Condition, Parameter, Script, ScriptAction, ScriptGroup, ScriptList};

/// A player: its index.
pub type PlayerH = i32;
/// A team, as the host names it.
pub type TeamH = i32;
/// A team prototype, as the host names it.
pub type ProtoH = i32;
/// An object: its id.
pub type ObjH = ObjectId;
/// A thing template, as the host names it.
pub type TemplateH = i32;
/// An AI group, as the host names it.
pub type GroupH = i32;
/// A trigger area, as the host names it.
pub type TriggerH = i32;

/// What the engine's operations return: an operation upstream does not define is an error.
pub type R<T> = Result<T, IniError>;

fn undefined(u: Undefined) -> IniError {
    IniError::undefined(u)
}

/// `MAX_COUNTERS`, `MAX_FLAGS`, `MAX_ATTACK_PRIORITIES`.
pub const MAX_COUNTERS: usize = 256;
/// `MAX_FLAGS`.
pub const MAX_FLAGS: usize = 256;
/// `MAX_ATTACK_PRIORITIES`.
pub const MAX_ATTACK_PRIORITIES: usize = 256;
/// `MAX_PLAYER_COUNT`.
pub const MAX_PLAYER_COUNT: usize = 16;
/// Subroutine calls nested this deep are taken as the end of the stack: a script that calls itself without end overflows it
/// upstream ([`Undefined::StackOverflow`]).
pub const MAX_CALL_DEPTH: u32 = 1000;
/// `MAX_SPIN_COUNT`.
const MAX_SPIN_COUNT: i32 = 20;
/// `FRAMES_TO_SHOW_WIN_LOSE_MESSAGE`.
pub const FRAMES_TO_SHOW_WIN_LOSE_MESSAGE: i32 = 120;
/// `FRAMES_TO_FADE_IN_AT_START`.
pub const FRAMES_TO_FADE_IN_AT_START: i32 = 33;
/// `ATTACK_PRIORITY_DEFAULT`.
pub const ATTACK_PRIORITY_DEFAULT: i32 = 1;
/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: i32 = 30;
/// `NONE_STRING`.
const NONE_STRING: &[u8] = b"<none>";
/// `INVALID_ID`.
pub const INVALID_ID: ObjectId = 0;

/// The names the engine reads specially (`Scripts.h`).
pub mod names {
    /// `LOCAL_PLAYER`.
    pub const LOCAL_PLAYER: &[u8] = b"<Local Player>";
    /// `THIS_PLAYER`.
    pub const THIS_PLAYER: &[u8] = b"<This Player>";
    /// `THIS_PLAYER_ENEMY`.
    pub const THIS_PLAYER_ENEMY: &[u8] = b"<This Player's Enemy>";
    /// `THE_PLAYER`.
    pub const THE_PLAYER: &[u8] = b"ThePlayer";
    /// `THIS_TEAM`.
    pub const THIS_TEAM: &[u8] = b"<This Team>";
    /// `TEAM_THE_PLAYER`.
    pub const TEAM_THE_PLAYER: &[u8] = b"teamThePlayer";
    /// `THIS_OBJECT`.
    pub const THIS_OBJECT: &[u8] = b"<This Object>";
    /// `MY_INNER_PERIMETER`.
    pub const MY_INNER_PERIMETER: &[u8] = b"[Skirmish]MyInnerPerimeter";
    /// `MY_OUTER_PERIMETER`.
    pub const MY_OUTER_PERIMETER: &[u8] = b"[Skirmish]MyOuterPerimeter";
    /// `ENEMY_INNER_PERIMETER`.
    pub const ENEMY_INNER_PERIMETER: &[u8] = b"[Skirmish]EnemyInnerPerimeter";
    /// `ENEMY_OUTER_PERIMETER`.
    pub const ENEMY_OUTER_PERIMETER: &[u8] = b"[Skirmish]EnemyOuterPerimeter";
    /// `INNER_PERIMETER`.
    pub const INNER_PERIMETER: &[u8] = b"InnerPerimeter";
    /// `OUTER_PERIMETER`.
    pub const OUTER_PERIMETER: &[u8] = b"OuterPerimeter";
}

/// `GameDifficulty`.
pub mod difficulty {
    /// `DIFFICULTY_EASY`.
    pub const EASY: i32 = 0;
    /// `DIFFICULTY_NORMAL`.
    pub const NORMAL: i32 = 1;
    /// `DIFFICULTY_HARD`.
    pub const HARD: i32 = 2;
}

/// `TFade` (an `Int` here: a load takes any value, as upstream's enum does).
pub mod fade {
    /// `FADE_NONE`.
    pub const NONE: i32 = 0;
    /// `FADE_SUBTRACT`.
    pub const SUBTRACT: i32 = 1;
    /// `FADE_ADD`.
    pub const ADD: i32 = 2;
    /// `FADE_SATURATE`.
    pub const SATURATE: i32 = 3;
    /// `FADE_MULTIPLY`.
    pub const MULTIPLY: i32 = 4;
}

/// `struct BreezeInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BreezeInfo {
    /// `m_direction`.
    pub direction: f32,
    /// `m_directionVec`.
    pub direction_vec: Coord2D,
    /// `m_intensity`.
    pub intensity: f32,
    /// `m_lean`.
    pub lean: f32,
    /// `m_randomness`.
    pub randomness: f32,
    /// `m_breezePeriod` (a `Short`).
    pub breeze_period: i16,
    /// `m_breezeVersion` (a `Short`).
    pub breeze_version: i16,
}

/// `struct NamedReveal`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NamedReveal {
    /// `m_revealName`.
    pub reveal_name: Vec<u8>,
    /// `m_waypointName`.
    pub waypoint_name: Vec<u8>,
    /// `m_radiusToReveal`.
    pub radius_to_reveal: f32,
    /// `m_playerName`.
    pub player_name: Vec<u8>,
}

/// `struct TCounter`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counter {
    /// `value`.
    pub value: i32,
    /// `name`.
    pub name: Vec<u8>,
    /// `isCountdownTimer`.
    pub is_countdown_timer: bool,
}

/// `struct TFlag`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flag {
    /// `value`.
    pub value: bool,
    /// `name`.
    pub name: Vec<u8>,
}

/// `class AttackPriorityInfo`: a named set of priorities by thing template (keyed by the template's final override), and a default.
#[derive(Clone, Debug, PartialEq)]
pub struct AttackPriorityInfo {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_defaultPriority`.
    pub default_priority: i32,
    /// `m_priorityMap` (`None`: not allocated).
    pub priority_map: Option<BTreeMap<TemplateH, i32>>,
}

impl Default for AttackPriorityInfo {
    fn default() -> Self {
        AttackPriorityInfo { name: Vec::new(), default_priority: ATTACK_PRIORITY_DEFAULT, priority_map: None }
    }
}

impl AttackPriorityInfo {
    /// `setPriority(tThing, priority)` (`tThing` already its final override).
    pub fn set_priority(&mut self, thing: TemplateH, priority: i32) {
        self.priority_map.get_or_insert_with(BTreeMap::new).insert(thing, priority);
    }

    /// `getPriority(tThing)` (`tThing` already its final override; `None`: NULL).
    pub fn get_priority(&self, thing: Option<TemplateH>) -> i32 {
        let Some(t) = thing else { return self.default_priority };
        self.priority_map.as_ref().and_then(|m| m.get(&t).copied()).unwrap_or(self.default_priority)
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        *self = AttackPriorityInfo::default();
    }
}

/// A script's place in the sides' script lists: side, group (`None`: the list's own scripts), index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptRef {
    /// The side.
    pub side: usize,
    /// The group in the side's list, or the list's own scripts.
    pub group: Option<usize>,
    /// The script's index.
    pub index: usize,
}

/// `class SequentialScript`, without its link: the chain it is in is a vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SequentialScript {
    /// `m_teamToExecOn`.
    pub team_to_exec_on: Option<TeamH>,
    /// `m_objectID`.
    pub object_id: ObjectId,
    /// `m_scriptToExecuteSequentially`.
    pub script: ScriptRef,
    /// `m_currentInstruction`.
    pub current_instruction: i32,
    /// `m_timesToLoop`.
    pub times_to_loop: i32,
    /// `m_framesToWait`.
    pub frames_to_wait: i32,
    /// `m_dontAdvanceInstruction`.
    pub dont_advance_instruction: bool,
}

impl SequentialScript {
    /// `SequentialScript()` for the script.
    pub fn new(script: ScriptRef) -> Self {
        SequentialScript {
            team_to_exec_on: None,
            object_id: INVALID_ID,
            script,
            current_instruction: -1,
            times_to_loop: 0,
            frames_to_wait: -1,
            dont_advance_instruction: false,
        }
    }
}

/// The world as `ScriptEngine.cpp` reaches it.
pub trait ScriptEngineHost {
    // --- the game engine, the frame, randomness ---
    /// `TheGameEngine->setFramesPerSecondLimit(TheGlobalData->m_framesPerSecondLimit)` (when both exist).
    fn se_restore_fps_limit(&mut self);
    /// `TheGameLogic->getFrame()`.
    fn se_frame(&self) -> u32;
    /// `GameLogicRandomValue(lo, hi)`.
    fn se_random_int(&mut self, lo: i32, hi: i32) -> i32;
    /// `TheMessageStream->appendMessage(GameMessage::MSG_CLEAR_GAME_DATA)`.
    fn se_append_clear_game_data(&mut self);
    /// `TheAudio->getAudioLengthMS(&AudioEventRTS(name))`.
    fn se_audio_length_ms(&mut self, name: &[u8]) -> f32;
    /// `TheCampaignManager->getCurrentCampaign() && ...->m_isChallengeCampaign`.
    fn se_is_challenge_campaign(&self) -> bool;
    /// `NAMEKEY(name)`.
    fn se_name_key(&mut self, name: &[u8]) -> NameKeyType;

    // --- TheScriptActions and TheScriptConditions ---
    /// `TheScriptActions->reset()`.
    fn se_actions_reset(&mut self);
    /// `TheScriptConditions->reset()`.
    fn se_conditions_reset(&mut self);
    /// `TheScriptActions->update()`.
    fn se_actions_update(&mut self);
    /// `TheScriptConditions->update()`.
    fn se_conditions_update(&mut self);
    /// `TheScriptActions->closeWindows(FALSE)`.
    fn se_actions_close_windows(&mut self);
    /// `TheScriptActions->executeAction(pAction)` (the action alone).
    fn se_execute_action(&mut self, engine: &mut ScriptEngine, action: &ScriptAction) -> R<()>;
    /// `TheScriptConditions->evaluateCondition(pCondition)`.
    fn se_evaluate_condition(&mut self, engine: &mut ScriptEngine, condition: &mut Condition) -> R<bool>;
    /// `TheScriptConditions->evaluateSkirmishCommandButtonIsReady(NULL, p1, p2, allReady)`.
    fn se_skirmish_command_button_is_ready(&mut self, engine: &mut ScriptEngine, p1: Option<&Parameter>, p2: Option<&Parameter>, all: bool) -> R<bool>;
    /// `TheScriptConditions->evaluateTeamIsContained(p0, allContained)`.
    fn se_team_is_contained(&mut self, engine: &mut ScriptEngine, p0: Option<&Parameter>, all: bool) -> R<bool>;

    // --- the sides' scripts ---
    /// `TheSidesList->getNumSides()`.
    fn se_num_sides(&self) -> i32;
    /// `TheSidesList->getSideInfo(i)->getScriptList()`.
    fn se_script_list(&self, side: usize) -> Option<&ScriptList>;
    /// The same, to change.
    fn se_script_list_mut(&mut self, side: usize) -> Option<&mut ScriptList>;
    /// `ScriptList::reset()`: the scripts read with the map deleted.
    fn se_reset_script_lists(&mut self);

    // --- players ---
    /// `ThePlayerList->getNthPlayer(i)`.
    fn se_nth_player(&self, i: i32) -> Option<PlayerH>;
    /// `ThePlayerList->getPlayerCount()`.
    fn se_player_count(&self) -> i32;
    /// `ThePlayerList->getLocalPlayer()`.
    fn se_local_player(&self) -> Option<PlayerH>;
    /// `ThePlayerList->findPlayerWithNameKey(key)`.
    fn se_find_player_with_name_key(&self, key: NameKeyType) -> Option<PlayerH>;
    /// `ThePlayerList->updateTeamStates()`.
    fn se_update_team_states(&mut self);
    /// `player->getCurrentEnemy()`.
    fn se_player_current_enemy(&self, p: PlayerH) -> Option<PlayerH>;
    /// `player->getPlayerType() == PLAYER_HUMAN`.
    fn se_player_is_human(&self, p: PlayerH) -> bool;
    /// `player->getPlayerNameKey()`.
    fn se_player_name_key(&self, p: PlayerH) -> NameKeyType;
    /// `player->getMpStartIndex()`.
    fn se_player_mp_start_index(&self, p: PlayerH) -> i32;
    /// `player->getPlayerMask()`.
    fn se_player_mask(&self, p: PlayerH) -> u32;
    /// `player->getPlayerDifficulty()`.
    fn se_player_difficulty(&self, p: PlayerH) -> i32;
    /// `player->isPlayerActive()`.
    fn se_player_is_active(&self, p: PlayerH) -> bool;
    /// `player->isSkirmishAIPlayer()`.
    fn se_player_is_skirmish_ai(&self, p: PlayerH) -> bool;
    /// `player->getDefaultTeam()`.
    fn se_player_default_team(&self, p: PlayerH) -> Option<TeamH>;

    // --- teams ---
    /// `TheTeamFactory->findTeamPrototype(name)`.
    fn se_find_team_prototype(&self, name: &[u8]) -> Option<ProtoH>;
    /// `proto->getIsSingleton()`.
    fn se_proto_is_singleton(&self, p: ProtoH) -> bool;
    /// `proto->getTemplateInfo()->m_maxInstances`.
    fn se_proto_max_instances(&self, p: ProtoH) -> i32;
    /// The prototype's instances, `iterate_TeamInstanceList()` order (`getFirstItemIn_TeamInstanceList()` first).
    fn se_proto_instances(&self, p: ProtoH) -> Vec<TeamH>;
    /// `team->isActive()`.
    fn se_team_is_active(&self, t: TeamH) -> bool;
    /// `team->getName()`.
    fn se_team_name(&self, t: TeamH) -> Vec<u8>;
    /// `team->getControllingPlayer()`.
    fn se_team_controlling_player(&self, t: TeamH) -> Option<PlayerH>;
    /// `TheAI->createGroup()` then `team->getTeamAsAIGroup(group)`.
    fn se_team_as_new_ai_group(&mut self, t: TeamH) -> GroupH;
    /// `group->isIdle()`.
    fn se_group_is_idle(&self, g: GroupH) -> bool;
    /// `group->isGroupAiDead()`.
    fn se_group_is_ai_dead(&self, g: GroupH) -> bool;

    // --- objects ---
    /// `TheGameLogic->findObjectByID(id)`.
    fn se_find_object(&self, id: ObjectId) -> Option<ObjH>;
    /// The objects from `TheGameLogic->getFirstObject()`, in `getNextObject()` order.
    fn se_objects(&self) -> Vec<ObjH>;
    /// `obj->getName()`.
    fn se_obj_name(&self, o: ObjH) -> Vec<u8>;
    /// `obj->setName(name)`.
    fn se_obj_set_name(&mut self, o: ObjH, name: &[u8]);
    /// `obj->hasCustomIndicatorColor()`.
    fn se_obj_has_custom_indicator_color(&self, o: ObjH) -> bool;
    /// `obj->getIndicatorColor()`.
    fn se_obj_indicator_color(&self, o: ObjH) -> i32;
    /// `obj->setCustomIndicatorColor(color)`.
    fn se_obj_set_custom_indicator_color(&mut self, o: ObjH, color: i32);
    /// `obj->removeCustomIndicatorColor()`.
    fn se_obj_remove_custom_indicator_color(&mut self, o: ObjH);
    /// `obj->getControllingPlayer()`.
    fn se_obj_controlling_player(&self, o: ObjH) -> Option<PlayerH>;
    /// `obj->getAIUpdateInterface() != NULL`.
    fn se_obj_has_ai(&self, o: ObjH) -> bool;
    /// `obj->getAIUpdateInterface()->isIdle()`.
    fn se_ai_is_idle(&self, o: ObjH) -> bool;
    /// `obj->isEffectivelyDead()`.
    fn se_obj_is_effectively_dead(&self, o: ObjH) -> bool;

    // --- terrain, partition, things ---
    /// `TheTerrainLogic->getTriggerAreaByName(name)`.
    fn se_trigger_area_by_name(&self, name: &[u8]) -> Option<TriggerH>;
    /// `TheTerrainLogic->getWaypointByName(name)`'s location.
    fn se_waypoint_location(&self, name: &[u8]) -> Option<Coord3D>;
    /// `ThePartitionManager->doShroudReveal(x, y, radius, mask)`.
    fn se_do_shroud_reveal(&mut self, x: f32, y: f32, radius: f32, mask: u32);
    /// `ThePartitionManager->undoShroudReveal(x, y, radius, mask)`.
    fn se_undo_shroud_reveal(&mut self, x: f32, y: f32, radius: f32, mask: u32);
    /// `TheThingFactory->findTemplate(name)`.
    fn se_find_template(&self, name: &[u8]) -> Option<TemplateH>;
    /// The templates from `TheThingFactory->firstTemplate()`, in `friend_getNextTemplate()` order.
    fn se_templates(&self) -> Vec<TemplateH>;
    /// `tmpl->isKindOf(kind)` (`kind` the script's `Int`, cast to `KindOfType`).
    fn se_template_is_kind_of(&self, t: TemplateH, kind: i32) -> R<bool>;
    /// `tmpl->getFinalOverride()`.
    fn se_template_final_override(&self, t: TemplateH) -> TemplateH;

    // --- save and load (batch E4) ---
    /// `TheTeamFactory->findTeamByID(id)` (`TeamH` is the team's id).
    fn se_team_exists(&self, id: TeamH) -> bool;
    /// `tmpl->getName()`.
    fn se_template_name(&self, t: TemplateH) -> Vec<u8>;
    /// `TheScienceStore->getInternalNameForScience(science)`.
    fn se_science_name(&self, science: i32) -> Vec<u8>;
    /// `TheScienceStore->getScienceFromInternalName(name)` (`None`: `SCIENCE_INVALID`).
    fn se_science_from_name(&self, name: &[u8]) -> Option<i32>;
    /// `TheScriptActions->doEnableOrDisableObjectDifficultyBonuses(enable)`.
    fn se_actions_enable_difficulty_bonuses(&mut self, enable: bool);
    /// `TheAudio->addAudioEvent(&event)` of an `AudioEventRTS(name)` for the player index.
    fn se_audio_add_event(&mut self, name: &[u8], player: i32);
}

/// `(Int)` of a `Real` (truncation): undefined out of range.
fn real_as_int(v: f32) -> R<i32> {
    let t = v.trunc();
    if t.is_nan() || t < i32::MIN as f32 || t >= 2147483648.0 {
        return Err(undefined(Undefined::FloatToIntOutOfRange));
    }
    Ok(t as i32)
}

/// An `AsciiString` of a script's string (one a map can hold).
fn ascii(s: &[u8]) -> AsciiString {
    AsciiString::try_from_bytes(s).expect("a script string fits an AsciiString")
}

/// `pCondition->getParameter(i)` / `pAction->getParameter(i)`, dereferenced.
fn parm(p: Option<&Parameter>) -> R<&Parameter> {
    p.ok_or_else(|| undefined(Undefined::NullParameter))
}

/// `m_counters[ndx]` / `m_flags[ndx]`: the index checked against the array.
fn slot(ndx: i32, len: usize) -> R<usize> {
    if ndx >= 0 && (ndx as usize) < len {
        Ok(ndx as usize)
    } else {
        Err(undefined(Undefined::ArrayIndexOutOfRange))
    }
}

/// Where an action or a condition is: a script and, in it, the or-term and the condition, or the action list and the action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemRef {
    /// `(or-term, condition)` of the script's conditions.
    Condition(ScriptRef, usize, usize),
    /// An action of the script's actions (`false`: its false actions).
    Action(ScriptRef, bool, usize),
}

/// `class ScriptEngine`'s state.
#[derive(Clone, Debug)]
pub struct ScriptEngine {
    /// `m_counters`.
    pub counters: Vec<Counter>,
    /// `m_numCounters`.
    pub num_counters: i32,
    /// `m_flags`.
    pub flags: Vec<Flag>,
    /// `m_numFlags`.
    pub num_flags: i32,
    /// `m_attackPriorityInfo`.
    pub attack_priority_info: Vec<AttackPriorityInfo>,
    /// `m_numAttackInfo`.
    pub num_attack_info: i32,
    /// `m_endGameTimer`.
    pub end_game_timer: i32,
    /// `m_closeWindowTimer`.
    pub close_window_timer: i32,
    /// `m_callingTeam`.
    pub calling_team: Option<TeamH>,
    /// `m_callingObject`.
    pub calling_object: Option<ObjH>,
    /// `m_conditionTeam`.
    pub condition_team: Option<TeamH>,
    /// `m_conditionObject`.
    pub condition_object: Option<ObjH>,
    /// `m_namedObjects`: each name and its object (`None`: it died).
    pub named_objects: Vec<(Vec<u8>, Option<ObjH>)>,
    /// `m_firstUpdate`.
    pub first_update: bool,
    /// `m_currentPlayer`.
    pub current_player: Option<PlayerH>,
    /// `m_skirmishHumanPlayer`.
    pub skirmish_human_player: Option<PlayerH>,
    /// `m_currentTrackName`.
    pub current_track_name: Vec<u8>,
    /// `m_fade`.
    pub fade: i32,
    /// `m_minFade`.
    pub min_fade: f32,
    /// `m_maxFade`.
    pub max_fade: f32,
    /// `m_curFadeValue`.
    pub cur_fade_value: f32,
    /// `m_curFadeFrame`.
    pub cur_fade_frame: i32,
    /// `m_fadeFramesIncrease`.
    pub fade_frames_increase: i32,
    /// `m_fadeFramesHold`.
    pub fade_frames_hold: i32,
    /// `m_fadeFramesDecrease`.
    pub fade_frames_decrease: i32,
    /// `m_frameObjectCountChanged`.
    pub frame_object_count_changed: u32,
    /// `m_objectCounts`.
    pub object_counts: Vec<BTreeMap<Vec<u8>, i32>>,
    /// `m_completedVideo`.
    pub completed_video: Vec<Vec<u8>>,
    /// `m_testingSpeech`.
    pub testing_speech: Vec<(Vec<u8>, u32)>,
    /// `m_testingAudio`.
    pub testing_audio: Vec<(Vec<u8>, u32)>,
    /// `m_uiInteractions`.
    pub ui_interactions: Vec<Vec<u8>>,
    /// `m_triggeredSpecialPowers`.
    pub triggered_special_powers: Vec<Vec<(Vec<u8>, ObjectId)>>,
    /// `m_midwaySpecialPowers`.
    pub midway_special_powers: Vec<Vec<(Vec<u8>, ObjectId)>>,
    /// `m_finishedSpecialPowers`.
    pub finished_special_powers: Vec<Vec<(Vec<u8>, ObjectId)>>,
    /// `m_completedUpgrades`.
    pub completed_upgrades: Vec<Vec<(Vec<u8>, ObjectId)>>,
    /// `m_acquiredSciences`.
    pub acquired_sciences: Vec<Vec<i32>>,
    /// `m_toppleDirections`.
    pub topple_directions: Vec<(Vec<u8>, Coord3D)>,
    /// `m_namedReveals`.
    pub named_reveals: Vec<NamedReveal>,
    /// `m_breezeInfo`.
    pub breeze_info: BreezeInfo,
    /// `m_gameDifficulty`.
    pub game_difficulty: i32,
    /// `m_freezeByScript`.
    pub freeze_by_script: bool,
    /// `m_allObjectTypeLists`.
    pub all_object_type_lists: Vec<ObjectTypes>,
    /// `m_objectsShouldReceiveDifficultyBonus`.
    pub objects_should_receive_difficulty_bonus: bool,
    /// `m_ChooseVictimAlwaysUsesNormal`.
    pub choose_victim_always_uses_normal: bool,
    /// `m_shownMPLocalDefeatWindow`.
    pub shown_mp_local_defeat_window: bool,
    /// `m_sequentialScripts`: each chain, its head first.
    pub sequential_scripts: Vec<Vec<SequentialScript>>,
    /// The capacity of `m_sequentialScripts`.
    pub sequential_capacity: usize,
    /// While the sequential scripts are progressed: the loop's place, and whether an action invalidated it.
    progressing: Option<(usize, bool)>,
    /// How many subroutine calls are in course.
    call_depth: u32,
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptEngine {
    /// `ScriptEngine()` (the members the constructor and their own constructors give, the rest zero: `NEW` zeroes the memory).
    pub fn new() -> Self {
        ScriptEngine {
            counters: vec![Counter::default(); MAX_COUNTERS],
            num_counters: 0,
            flags: vec![Flag::default(); MAX_FLAGS],
            num_flags: 0,
            attack_priority_info: vec![AttackPriorityInfo::default(); MAX_ATTACK_PRIORITIES],
            num_attack_info: 0,
            end_game_timer: 0,
            close_window_timer: 0,
            calling_team: None,
            calling_object: None,
            condition_team: None,
            condition_object: None,
            named_objects: Vec::new(),
            first_update: true,
            current_player: None,
            skirmish_human_player: None,
            current_track_name: Vec::new(),
            fade: fade::NONE,
            min_fade: 0.0,
            max_fade: 0.0,
            cur_fade_value: 0.0,
            cur_fade_frame: 0,
            fade_frames_increase: 0,
            fade_frames_hold: 0,
            fade_frames_decrease: 0,
            frame_object_count_changed: 0,
            object_counts: vec![BTreeMap::new(); MAX_PLAYER_COUNT],
            completed_video: Vec::new(),
            testing_speech: Vec::new(),
            testing_audio: Vec::new(),
            ui_interactions: Vec::new(),
            triggered_special_powers: vec![Vec::new(); MAX_PLAYER_COUNT],
            midway_special_powers: vec![Vec::new(); MAX_PLAYER_COUNT],
            finished_special_powers: vec![Vec::new(); MAX_PLAYER_COUNT],
            completed_upgrades: vec![Vec::new(); MAX_PLAYER_COUNT],
            acquired_sciences: vec![Vec::new(); MAX_PLAYER_COUNT],
            topple_directions: Vec::new(),
            named_reveals: Vec::new(),
            breeze_info: BreezeInfo::default(),
            game_difficulty: difficulty::NORMAL,
            freeze_by_script: false,
            all_object_type_lists: Vec::new(),
            objects_should_receive_difficulty_bonus: true,
            choose_victim_always_uses_normal: false,
            shown_mp_local_defeat_window: false,
            sequential_scripts: Vec::new(),
            sequential_capacity: 0,
            progressing: None,
            call_depth: 0,
        }
    }

    // --------------------------------------------------------------------------------------------------------------------
    // reset, newMap, update
    // --------------------------------------------------------------------------------------------------------------------

    /// `reset()`.
    pub fn reset<H: ScriptEngineHost>(&mut self, h: &mut H) {
        h.se_restore_fps_limit();
        h.se_actions_reset();
        h.se_conditions_reset();
        self.num_counters = 1;
        self.num_attack_info = 1;
        self.num_flags = 1;
        self.end_game_timer = -1;
        self.close_window_timer = -1;
        self.calling_team = None;
        self.calling_object = None;
        self.condition_team = None;
        self.condition_object = None;
        self.current_player = None;
        self.skirmish_human_player = None;
        self.frame_object_count_changed = 0;
        self.shown_mp_local_defeat_window = false;
        self.clear_counters_and_flags();
        self.breeze_info.direction = PI / 3.0;
        self.breeze_info.direction_vec.x = sin(self.breeze_info.direction);
        self.breeze_info.direction_vec.y = cos(self.breeze_info.direction);
        self.breeze_info.intensity = 0.07 * PI / 4.0;
        self.breeze_info.lean = 0.07 * PI / 4.0;
        self.breeze_info.breeze_period = (LOGICFRAMES_PER_SECOND * 5) as i16;
        self.breeze_info.randomness = 0.2;
        self.breeze_info.breeze_version = 0;
        self.freeze_by_script = false;
        self.objects_should_receive_difficulty_bonus = true;
        self.choose_victim_always_uses_normal = false;
        // every chain deleted (cleanupSequentialScript(it, TRUE)); the vector keeps its capacity
        self.sequential_scripts.clear();
        self.all_object_type_lists.clear();
        self.named_reveals.clear();
        self.named_objects.clear();
        self.clear_notifications();
        h.se_reset_script_lists();
        for a in &mut self.attack_priority_info {
            a.reset();
        }
        for c in &mut self.object_counts {
            c.clear();
        }
        self.topple_directions.clear();
    }

    fn clear_counters_and_flags(&mut self) {
        for c in &mut self.counters {
            c.value = 0;
            c.is_countdown_timer = false;
            c.name.clear();
        }
        for f in &mut self.flags {
            f.value = false;
            f.name.clear();
        }
    }

    fn clear_notifications(&mut self) {
        self.completed_video.clear();
        self.testing_speech.clear();
        self.testing_audio.clear();
        self.ui_interactions.clear();
        for i in 0..MAX_PLAYER_COUNT {
            self.triggered_special_powers[i].clear();
            self.midway_special_powers[i].clear();
            self.finished_special_powers[i].clear();
            self.acquired_sciences[i].clear();
            self.completed_upgrades[i].clear();
        }
    }

    /// `newMap()`: counters, flags, timers and notifications cleared, each script's condition team found, the fade in from black.
    pub fn new_map<H: ScriptEngineHost>(&mut self, h: &mut H) -> R<()> {
        self.num_counters = 1;
        self.num_flags = 1;
        self.clear_counters_and_flags();
        self.end_game_timer = -1;
        self.close_window_timer = -1;
        self.clear_notifications();
        for side in 0..h.se_num_sides().max(0) as usize {
            for r in Self::all_scripts(h, side) {
                self.check_conditions_for_team_names(h, r)?;
            }
        }
        self.first_update = true;
        self.fade = fade::MULTIPLY;
        self.cur_fade_frame = 0;
        self.min_fade = 1.0;
        self.max_fade = 0.0;
        self.fade_frames_increase = 0;
        self.fade_frames_hold = 0;
        self.fade_frames_decrease = FRAMES_TO_FADE_IN_AT_START;
        self.cur_fade_value = 0.0;
        Ok(())
    }

    /// Every script of the side's list, the list's own first, then each group's.
    fn all_scripts<H: ScriptEngineHost>(h: &H, side: usize) -> Vec<ScriptRef> {
        let mut out = Vec::new();
        if let Some(l) = h.se_script_list(side) {
            out.extend((0..l.scripts.len()).map(|index| ScriptRef { side, group: None, index }));
            for (g, grp) in l.groups.iter().enumerate() {
                out.extend((0..grp.scripts.len()).map(|index| ScriptRef { side, group: Some(g), index }));
            }
        }
        out
    }

    /// The script at the place.
    pub fn script<H: ScriptEngineHost>(h: &H, r: ScriptRef) -> Option<&Script> {
        let l = h.se_script_list(r.side)?;
        match r.group {
            None => l.scripts.get(r.index),
            Some(g) => l.groups.get(g)?.scripts.get(r.index),
        }
    }

    /// The script at the place, to change.
    pub fn script_mut<H: ScriptEngineHost>(h: &mut H, r: ScriptRef) -> Option<&mut Script> {
        let l = h.se_script_list_mut(r.side)?;
        match r.group {
            None => l.scripts.get_mut(r.index),
            Some(g) => l.groups.get_mut(g)?.scripts.get_mut(r.index),
        }
    }

    fn group<H: ScriptEngineHost>(h: &H, side: usize, g: usize) -> Option<&ScriptGroup> {
        h.se_script_list(side)?.groups.get(g)
    }

    /// `update()`.
    pub fn update<H: ScriptEngineHost>(&mut self, h: &mut H) -> R<()> {
        if self.first_update {
            self.create_named_cache(h);
            self.first_update = false;
        }
        if self.close_window_timer > 0 {
            self.close_window_timer -= 1;
            if self.close_window_timer < 1 {
                h.se_actions_close_windows();
            }
        }
        if self.end_game_timer > 0 {
            self.end_game_timer -= 1;
            if self.end_game_timer < 1 {
                h.se_append_clear_game_data();
            }
        }
        if self.fade != fade::NONE {
            self.update_fades();
        }
        if self.end_game_timer >= 0 {
            return Ok(());
        }
        h.se_actions_update();
        h.se_conditions_update();
        for i in 1..self.num_counters.max(1) as usize {
            let c = &mut self.counters[i];
            if c.is_countdown_timer && c.value >= 0 {
                c.value -= 1;
            }
        }
        for side in 0..h.se_num_sides().max(0) as usize {
            self.current_player = h.se_nth_player(side as i32);
            let Some(l) = h.se_script_list(side) else { continue };
            let n = l.scripts.len();
            self.execute_scripts_of(h, side, None, n)?;
            let mut g = 0;
            while let Some(grp) = Self::group(h, side, g) {
                let (active, sub, n) = (grp.is_group_active, grp.is_group_subroutine, grp.scripts.len());
                if active && !sub {
                    self.execute_scripts_of(h, side, Some(g), n)?;
                }
                g += 1;
            }
            self.current_player = None;
        }
        h.se_update_team_states();
        self.ui_interactions.clear();
        self.evaluate_and_progress_all_sequential_scripts(h)
    }

    /// `startQuickEndGameTimer()`.
    pub fn start_quick_end_game_timer(&mut self) {
        self.end_game_timer = 1;
    }

    /// `startEndGameTimer()`.
    pub fn start_end_game_timer(&mut self) {
        self.end_game_timer = FRAMES_TO_SHOW_WIN_LOSE_MESSAGE;
    }

    /// `startCloseWindowTimer()`.
    pub fn start_close_window_timer(&mut self) {
        self.close_window_timer = FRAMES_TO_SHOW_WIN_LOSE_MESSAGE;
    }

    /// `isGameEnding()`.
    pub fn is_game_ending(&self) -> bool {
        self.end_game_timer >= 0
    }

    /// `updateFades()`.
    pub fn update_fades(&mut self) {
        self.cur_fade_frame += 1;
        let mut fade = self.cur_fade_frame;
        if fade <= self.fade_frames_increase {
            let factor = self.cur_fade_frame as f32 / self.fade_frames_increase as f32;
            self.cur_fade_value = self.min_fade + factor * (self.max_fade - self.min_fade);
            return;
        }
        fade -= self.fade_frames_increase;
        if fade <= self.fade_frames_hold {
            self.cur_fade_value = self.max_fade;
            return;
        }
        fade -= self.fade_frames_hold;
        if fade <= self.fade_frames_decrease {
            let mut divisor = self.fade_frames_decrease.wrapping_add(i32::from(!plant(3914)));
            if divisor == 0 {
                divisor = 1;
            }
            let factor = fade as f32 / divisor as f32;
            self.cur_fade_value = self.max_fade + factor * (self.min_fade - self.max_fade);
            return;
        }
        self.fade = fade::NONE;
    }

    // --------------------------------------------------------------------------------------------------------------------
    // lookups
    // --------------------------------------------------------------------------------------------------------------------

    /// `getCurrentPlayer()`.
    pub fn get_current_player(&self) -> Option<PlayerH> {
        self.current_player
    }

    /// `clearFlag(name)`: the flags `name0` .. `name15` false.
    pub fn clear_flag(&mut self, name: &[u8]) {
        for j in 0..MAX_PLAYER_COUNT {
            let mut mod_name = name.to_vec();
            mod_name.extend_from_slice(j.to_string().as_bytes());
            for i in 1..self.num_flags.max(1) as usize {
                if self.flags[i].name == mod_name {
                    self.flags[i].value = false;
                }
            }
        }
    }

    /// `clearTeamFlags()`.
    pub fn clear_team_flags(&mut self) {
        for n in [
            &b"USA Team is Building"[..],
            b"USA Air Team Is Building",
            b"USA Inf Team Is Building",
            b"China Team is Building",
            b"China Air Team Is Building",
            b"China Inf Team Is Building",
            b"GLA Team is Building",
            b"GLA Inf Team is Building",
        ] {
            self.clear_flag(n);
        }
    }

    /// `getSkirmishEnemyPlayer()`: the current player's enemy, else the first human player (not `ThePlayer` in a challenge).
    pub fn get_skirmish_enemy_player<H: ScriptEngineHost>(&self, h: &mut H) -> Option<PlayerH> {
        let challenge = h.se_is_challenge_campaign();
        let cur = self.current_player?;
        let enemy = h.se_player_current_enemy(cur);
        if enemy.is_some() {
            return enemy;
        }
        for i in 0..h.se_player_count() {
            let Some(e) = h.se_nth_player(i) else { continue };
            if h.se_player_is_human(e) {
                if challenge && h.se_player_name_key(e) == h.se_name_key(names::THE_PLAYER) {
                    continue;
                }
                return Some(e);
            }
        }
        None
    }

    /// `getPlayerFromAsciiString(name)`.
    pub fn get_player_from_ascii_string<H: ScriptEngineHost>(&self, h: &mut H, s: &[u8]) -> Option<PlayerH> {
        let challenge = h.se_is_challenge_campaign();
        if s == names::LOCAL_PLAYER || (s == names::THE_PLAYER && challenge) {
            if plant(3911) {
                return None;
            }
            return h.se_local_player();
        }
        if s == names::THIS_PLAYER {
            return self.get_current_player();
        }
        if s == names::THIS_PLAYER_ENEMY {
            return self.get_skirmish_enemy_player(h);
        }
        let key = h.se_name_key(s);
        h.se_find_player_with_name_key(key)
    }

    /// `getObjectTypes(listName)` (the list's index).
    pub fn get_object_types(&self, list: &[u8]) -> Option<usize> {
        self.all_object_type_lists.iter().position(|t| t.list_name().str() == list)
    }

    /// `doObjectTypeListMaintenance(listName, type, add)`: an empty list is removed.
    pub fn do_object_type_list_maintenance(&mut self, list: &[u8], object_type: &[u8], add: bool) {
        let i = match self.get_object_types(list) {
            Some(i) => i,
            None => {
                self.all_object_type_lists.push(ObjectTypes::with_list_name(ascii(list)));
                self.all_object_type_lists.len() - 1
            }
        };
        let t = &mut self.all_object_type_lists[i];
        if add {
            t.add_object_type(&ascii(object_type));
        } else {
            t.remove_object_type(&ascii(object_type));
        }
        if t.list_size() == 0 {
            self.all_object_type_lists.remove(i);
        }
    }

    /// `getQualifiedTriggerAreaByName(name)`: the skirmish perimeters named for the current player's (or its enemy's) start.
    pub fn get_qualified_trigger_area_by_name<H: ScriptEngineHost>(&self, h: &H, name: &[u8]) -> Option<TriggerH> {
        let mut name = name.to_vec();
        if name == names::MY_INNER_PERIMETER || name == names::MY_OUTER_PERIMETER {
            let p = self.current_player?;
            let ndx = h.se_player_mp_start_index(p).wrapping_add(1);
            let base = if name == names::MY_INNER_PERIMETER { names::INNER_PERIMETER } else { names::OUTER_PERIMETER };
            name = [base, ndx.to_string().as_bytes()].concat();
        } else if name == names::ENEMY_INNER_PERIMETER || name == names::ENEMY_OUTER_PERIMETER {
            let mut ndx = -1;
            if let Some(p) = self.current_player {
                if let Some(e) = h.se_player_current_enemy(p) {
                    ndx = h.se_player_mp_start_index(e).wrapping_add(1);
                }
            }
            let base = if name == names::ENEMY_INNER_PERIMETER { names::INNER_PERIMETER } else { names::OUTER_PERIMETER };
            name = [base, ndx.to_string().as_bytes()].concat();
        }
        h.se_trigger_area_by_name(&name)
    }

    /// `getTeamNamed(name)`.
    pub fn get_team_named<H: ScriptEngineHost>(&self, h: &H, name: &[u8]) -> R<Option<TeamH>> {
        if name == names::TEAM_THE_PLAYER && h.se_is_challenge_campaign() {
            let p = h.se_local_player().ok_or_else(|| undefined(Undefined::NullLocalPlayer))?;
            return Ok(h.se_player_default_team(p));
        }
        if name == names::THIS_TEAM {
            return Ok(self.calling_team.or(self.condition_team));
        }
        if let Some(t) = self.calling_team {
            if h.se_team_name(t) == name {
                return Ok(Some(t));
            }
        }
        if let Some(t) = self.condition_team {
            if h.se_team_name(t) == name {
                return Ok(Some(t));
            }
        }
        let Some(proto) = h.se_find_team_prototype(name) else { return Ok(None) };
        let first = h.se_proto_instances(proto).first().copied();
        if h.se_proto_is_singleton(proto) {
            return Ok(first.filter(|t| plant(3905) || h.se_team_is_active(*t)));
        }
        Ok(first)
    }

    /// `getUnitNamed(name)`.
    pub fn get_unit_named(&self, name: &[u8]) -> Option<ObjH> {
        if name == names::THIS_OBJECT {
            return self.calling_object.or(self.condition_object);
        }
        self.named_objects.iter().find(|(n, _)| n == name).and_then(|(_, o)| *o)
    }

    /// `didUnitExist(name)`: the name was known and its object died.
    pub fn did_unit_exist(&self, name: &[u8]) -> bool {
        self.named_objects.iter().find(|(n, _)| n == name).is_some_and(|(_, o)| o.is_none())
    }

    /// `runScript(name, team)`: a subroutine (group or script) run for the team, as its player's.
    pub fn run_script<H: ScriptEngineHost>(&mut self, h: &mut H, name: &[u8], team: Option<TeamH>) -> R<()> {
        if name.is_empty() || name == NONE_STRING {
            return Ok(());
        }
        let sav_player = self.current_player;
        let sav_calling = self.calling_team;
        self.calling_team = team;
        self.condition_team = None;
        self.current_player = None;
        if let Some(t) = self.calling_team {
            self.current_player = h.se_team_controlling_player(t);
        }
        let r = self.call_subroutine_named(h, name);
        self.calling_team = sav_calling;
        self.current_player = sav_player;
        r
    }

    /// `runObjectScript(name, object)`.
    pub fn run_object_script<H: ScriptEngineHost>(&mut self, h: &mut H, name: &[u8], object: Option<ObjH>) -> R<()> {
        if name.is_empty() || name == NONE_STRING {
            return Ok(());
        }
        let sav = self.calling_object;
        self.calling_object = object;
        let r = self.call_subroutine_named(h, name);
        self.calling_object = sav;
        r
    }

    /// The body shared by `runScript`, `runObjectScript` and `callSubroutine`: an active subroutine group runs its scripts, a
    /// subroutine script runs.
    fn call_subroutine_named<H: ScriptEngineHost>(&mut self, h: &mut H, name: &[u8]) -> R<()> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(undefined(Undefined::StackOverflow));
        }
        self.call_depth += 1;
        let r = self.call_subroutine_body(h, name);
        self.call_depth -= 1;
        r
    }

    fn call_subroutine_body<H: ScriptEngineHost>(&mut self, h: &mut H, name: &[u8]) -> R<()> {
        if let Some((side, g)) = self.find_group(h, name) {
            let grp = Self::group(h, side, g).expect("found");
            if grp.is_group_subroutine && grp.is_group_active {
                let n = grp.scripts.len();
                self.execute_scripts_of(h, side, Some(g), n)?;
            }
        } else if let Some(r) = self.find_script(h, name) {
            if Self::script(h, r).expect("found").is_subroutine {
                self.execute_script(h, r)?;
            }
        }
        Ok(())
    }

    /// `allocateCounter(name)`.
    fn allocate_counter(&mut self, name: &[u8]) -> i32 {
        for i in 1..self.num_counters.max(1) {
            if self.counters[i as usize].name == name {
                return i;
            }
        }
        if (self.num_counters as usize) < MAX_COUNTERS {
            let i = self.num_counters;
            self.counters[i as usize].name = name.to_vec();
            self.num_counters += 1;
            return i;
        }
        0
    }

    /// `getCounter(name)`.
    pub fn get_counter(&self, name: &[u8]) -> Option<&Counter> {
        (1..self.num_counters.max(1) as usize).map(|i| &self.counters[i]).find(|c| c.name == name)
    }

    /// `createNamedMapReveal(revealName, waypointName, radius, playerName)`: not when the name is taken.
    pub fn create_named_map_reveal(&mut self, reveal: &[u8], waypoint: &[u8], radius: f32, player: &[u8]) {
        if self.named_reveals.iter().any(|r| r.reveal_name == reveal) {
            return;
        }
        self.named_reveals.push(NamedReveal { reveal_name: reveal.to_vec(), waypoint_name: waypoint.to_vec(), radius_to_reveal: radius, player_name: player.to_vec() });
    }

    fn named_reveal_args<H: ScriptEngineHost>(&self, h: &mut H, reveal: &[u8]) -> Option<(Coord3D, f32, u32)> {
        let r = self.named_reveals.iter().find(|r| r.reveal_name == reveal)?.clone();
        let pos = h.se_waypoint_location(&r.waypoint_name)?;
        let p = self.get_player_from_ascii_string(h, &r.player_name)?;
        Some((pos, r.radius_to_reveal, h.se_player_mask(p)))
    }

    /// `doNamedMapReveal(revealName)`.
    pub fn do_named_map_reveal<H: ScriptEngineHost>(&mut self, h: &mut H, reveal: &[u8]) {
        if let Some((pos, r, mask)) = self.named_reveal_args(h, reveal) {
            h.se_do_shroud_reveal(pos.x, pos.y, if plant(3916) { r * 2.0 } else { r }, mask);
        }
    }

    /// `undoNamedMapReveal(revealName)`.
    pub fn undo_named_map_reveal<H: ScriptEngineHost>(&mut self, h: &mut H, reveal: &[u8]) {
        if let Some((pos, r, mask)) = self.named_reveal_args(h, reveal) {
            h.se_undo_shroud_reveal(pos.x, pos.y, r, mask);
        }
    }

    /// `removeNamedMapReveal(revealName)`.
    pub fn remove_named_map_reveal(&mut self, reveal: &[u8]) {
        if let Some(i) = self.named_reveals.iter().position(|r| r.reveal_name == reveal && !plant(3917)) {
            self.named_reveals.remove(i);
        }
    }

    /// `allocateFlag(name)`.
    fn allocate_flag(&mut self, name: &[u8]) -> i32 {
        for i in 1..self.num_flags.max(1) {
            if self.flags[i as usize].name == name {
                return i;
            }
        }
        if (self.num_flags as usize) < MAX_FLAGS {
            let i = self.num_flags;
            self.flags[i as usize].name = name.to_vec();
            self.num_flags += 1;
            return i;
        }
        0
    }

    /// `findGroup(name)`: the first group of the name, side by side.
    fn find_group<H: ScriptEngineHost>(&self, h: &H, name: &[u8]) -> Option<(usize, usize)> {
        for side in 0..h.se_num_sides().max(0) as usize {
            let Some(l) = h.se_script_list(side) else { continue };
            if let Some(g) = l.groups.iter().position(|g| g.group_name == name) {
                return Some((side, g));
            }
        }
        None
    }

    /// `findScript(name)` (`findScriptByName`): side by side, the list's own scripts, then its groups'.
    pub fn find_script<H: ScriptEngineHost>(&self, h: &H, name: &[u8]) -> Option<ScriptRef> {
        (0..h.se_num_sides().max(0) as usize).flat_map(|side| Self::all_scripts(h, side)).find(|r| Self::script(h, *r).is_some_and(|s| s.script_name == name))
    }

    // --------------------------------------------------------------------------------------------------------------------
    // the engine's own conditions and actions
    // --------------------------------------------------------------------------------------------------------------------

    /// A counter parameter's index: the cached one, else the name's (allocated, and cached in the parameter).
    fn counter_index<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef, k: i32) -> R<usize> {
        let p = parm(Self::item_parameter(h, at, k))?;
        let mut ndx = p.int;
        if ndx == 0 {
            let name = p.string.clone();
            ndx = self.allocate_counter(&name);
            Self::item_parameter_mut(h, at, k).expect("present").int = ndx;
        }
        slot(ndx, MAX_COUNTERS)
    }

    /// A flag parameter's index, the same way.
    fn flag_index<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef, k: i32) -> R<usize> {
        let p = parm(Self::item_parameter(h, at, k))?;
        let mut ndx = p.int;
        if ndx == 0 {
            let name = p.string.clone();
            ndx = self.allocate_flag(&name);
            Self::item_parameter_mut(h, at, k).expect("present").int = ndx;
        }
        slot(ndx, MAX_FLAGS)
    }

    fn item_parameter<H: ScriptEngineHost>(h: &H, at: ItemRef, k: i32) -> Option<&Parameter> {
        match at {
            ItemRef::Condition(r, o, c) => Self::script(h, r)?.conditions.get(o)?.conditions.get(c)?.get_parameter(k),
            ItemRef::Action(r, f, a) => {
                let s = Self::script(h, r)?;
                if f { s.actions.get(a) } else { s.actions_false.get(a) }?.get_parameter(k)
            }
        }
    }

    fn item_parameter_mut<H: ScriptEngineHost>(h: &mut H, at: ItemRef, k: i32) -> Option<&mut Parameter> {
        if k < 0 {
            return None;
        }
        let parms = match at {
            ItemRef::Condition(r, o, c) => {
                let cond = Self::script_mut(h, r)?.conditions.get_mut(o)?.conditions.get_mut(c)?;
                if k >= cond.num_parms {
                    return None;
                }
                &mut cond.parms
            }
            ItemRef::Action(r, f, a) => {
                let s = Self::script_mut(h, r)?;
                let act = if f { s.actions.get_mut(a) } else { s.actions_false.get_mut(a) }?;
                if k >= act.num_parms {
                    return None;
                }
                &mut act.parms
            }
        };
        parms.get_mut(k as usize)?.as_mut()
    }

    fn pint<H: ScriptEngineHost>(h: &H, at: ItemRef, k: i32) -> R<i32> {
        Ok(parm(Self::item_parameter(h, at, k))?.int)
    }

    fn preal<H: ScriptEngineHost>(h: &H, at: ItemRef, k: i32) -> R<f32> {
        Ok(parm(Self::item_parameter(h, at, k))?.real)
    }

    fn pstr<H: ScriptEngineHost>(h: &H, at: ItemRef, k: i32) -> R<Vec<u8>> {
        Ok(parm(Self::item_parameter(h, at, k))?.string.clone())
    }

    /// `evaluateCounter(pCondition)`: the counter compared with the value.
    fn evaluate_counter<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<bool> {
        let ndx = self.counter_index(h, at, 0)?;
        let value = Self::pint(h, at, 2)?;
        let c = self.counters[ndx].value;
        Ok(match Self::pint(h, at, 1)? {
            0 => c < value,
            1 => c <= value,
            2 => c == value,
            3 => c >= value,
            4 => c > value,
            5 => c != value,
            _ => false,
        })
    }

    /// `setCounter(pAction)`.
    fn set_counter<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<()> {
        let ndx = self.counter_index(h, at, 0)?;
        self.counters[ndx].value = Self::pint(h, at, 1)?;
        Ok(())
    }

    /// `setFade(pAction)`.
    fn set_fade<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef, action_type: i32) -> R<()> {
        self.fade = match action_type {
            at::CAMERA_FADE_ADD => fade::ADD,
            at::CAMERA_FADE_SUBTRACT => fade::SUBTRACT,
            at::CAMERA_FADE_SATURATE => fade::SATURATE,
            at::CAMERA_FADE_MULTIPLY => fade::MULTIPLY,
            _ => {
                self.fade = fade::NONE;
                return Ok(());
            }
        };
        self.cur_fade_frame = 0;
        self.min_fade = Self::preal(h, at, 0)?;
        self.max_fade = Self::preal(h, at, 1)?;
        self.fade_frames_increase = Self::pint(h, at, 2)?;
        self.fade_frames_hold = Self::pint(h, at, 3)?;
        self.fade_frames_decrease = Self::pint(h, at, 4)?;
        self.cur_fade_value = self.min_fade;
        if self.fade_frames_increase == 0 && !plant(3901) {
            self.update_fades();
        }
        Ok(())
    }

    /// `setSway(pAction)`.
    fn set_sway<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<()> {
        self.breeze_info.breeze_version = self.breeze_info.breeze_version.wrapping_add(1);
        self.breeze_info.direction = Self::preal(h, at, 0)?;
        self.breeze_info.direction_vec.x = sin(self.breeze_info.direction);
        self.breeze_info.direction_vec.y = cos(self.breeze_info.direction);
        self.breeze_info.intensity = Self::preal(h, at, 1)?;
        self.breeze_info.lean = Self::preal(h, at, 2)?;
        self.breeze_info.breeze_period = Self::pint(h, at, 3)? as i16;
        if self.breeze_info.breeze_period < 1 {
            self.breeze_info.breeze_period = 1;
        }
        self.breeze_info.randomness = Self::preal(h, at, 4)?;
        Ok(())
    }

    /// `addCounter` (`sign` 1) and `subCounter` (`sign` -1): the value is the first parameter, the counter the second.
    fn add_counter<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef, add: bool) -> R<()> {
        let value = Self::pint(h, at, 0)?;
        let ndx = self.counter_index(h, at, 1)?;
        let c = &mut self.counters[ndx].value;
        *c = if add { c.checked_add(value) } else { c.checked_sub(value) }.ok_or_else(|| undefined(Undefined::SignedIntOverflow))?;
        Ok(())
    }

    /// `evaluateFlag(pCondition)`: the flag has the value, or a user interface hook of its name was signalled this frame.
    fn evaluate_flag<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<bool> {
        let ndx = self.flag_index(h, at, 0)?;
        let value = Self::pint(h, at, 1)? != 0;
        if value == self.flags[ndx].value {
            return Ok(true);
        }
        let name = Self::pstr(h, at, 0)?;
        Ok(self.ui_interactions.contains(&name))
    }

    /// `setFlag(pAction)`.
    fn set_flag<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<()> {
        let ndx = self.flag_index(h, at, 0)?;
        self.flags[ndx].value = Self::pint(h, at, 1)? != 0;
        Ok(())
    }

    /// `findAttackInfo(name, addIfNotFound)` (the index; 0 is the default set, never found by name).
    fn find_attack_info(&mut self, name: &[u8], add: bool) -> Option<usize> {
        if let Some(i) = (1..self.num_attack_info.max(1) as usize).find(|&i| self.attack_priority_info[i].name == name) {
            return Some(i);
        }
        if add && (self.num_attack_info as usize) < MAX_ATTACK_PRIORITIES {
            let i = self.num_attack_info as usize;
            self.attack_priority_info[i].name = name.to_vec();
            self.num_attack_info += 1;
            return Some(i);
        }
        None
    }

    /// `getDefaultAttackInfo()`.
    pub fn get_default_attack_info(&self) -> &AttackPriorityInfo {
        &self.attack_priority_info[0]
    }

    /// `getAttackInfo(name)`: the named set, else the default.
    pub fn get_attack_info(&self, name: &[u8]) -> &AttackPriorityInfo {
        (1..self.num_attack_info.max(1) as usize).map(|i| &self.attack_priority_info[i]).find(|a| a.name == name).unwrap_or(&self.attack_priority_info[0])
    }

    /// `setPriorityThing(pAction)`: the type, or each type of the list of that name, given the priority in the named set.
    fn set_priority_thing<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<()> {
        let arg = Self::pstr(h, at, 1)?;
        let names: Vec<Vec<u8>> = match self.get_object_types(&arg) {
            None => vec![arg],
            Some(i) => {
                let t = &self.all_object_type_lists[i];
                (0..t.list_size() as i32).map(|k| t.get_nth_in_list(k).str().to_vec()).collect()
            }
        };
        for n in names {
            let Some(t) = h.se_find_template(&n) else {
                Self::pstr(h, at, 0)?;
                return Ok(());
            };
            let set = Self::pstr(h, at, 0)?;
            let Some(i) = self.find_attack_info(&set, true) else { return Ok(()) };
            let t = if plant(3915) { t } else { h.se_template_final_override(t) };
            let pr = Self::pint(h, at, 2)?;
            self.attack_priority_info[i].set_priority(t, pr);
        }
        Ok(())
    }

    /// `setPriorityKind(pAction)`: every template of the kind given the priority in the named set.
    fn set_priority_kind<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<()> {
        let set = Self::pstr(h, at, 0)?;
        let Some(i) = self.find_attack_info(&set, true) else { return Ok(()) };
        let kind = Self::pint(h, at, 1)?;
        let pr = Self::pint(h, at, 2)?;
        for t in h.se_templates() {
            if h.se_template_is_kind_of(t, kind)? {
                let t = h.se_template_final_override(t);
                self.attack_priority_info[i].set_priority(t, pr);
            }
        }
        Ok(())
    }

    /// `setPriorityDefault(pAction)`.
    fn set_priority_default<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<()> {
        let set = Self::pstr(h, at, 0)?;
        let Some(i) = self.find_attack_info(&set, true) else { return Ok(()) };
        self.attack_priority_info[i].default_priority = Self::pint(h, at, 1)?;
        Ok(())
    }

    /// `getObjectCount(playerIndex, typeName)`: 0 for an inactive player (the player is read unchecked).
    pub fn get_object_count<H: ScriptEngineHost>(&self, h: &H, player: i32, name: &[u8]) -> R<i32> {
        let p = h.se_nth_player(player).ok_or_else(|| undefined(Undefined::NullObject))?;
        if !h.se_player_is_active(p) {
            return Ok(0);
        }
        let i = slot(player, MAX_PLAYER_COUNT)?;
        Ok(self.object_counts[i].get(name).copied().unwrap_or(0))
    }

    /// `setObjectCount(playerIndex, typeName, count)`.
    pub fn set_object_count(&mut self, player: i32, name: &[u8], count: i32) -> R<()> {
        let i = slot(player, MAX_PLAYER_COUNT)?;
        self.object_counts[i].insert(name.to_vec(), count.wrapping_add(i32::from(plant(3913))));
        Ok(())
    }

    /// `evaluateTimer(pCondition)`: started and down to 0 or -1.
    fn evaluate_timer<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<bool> {
        let ndx = self.counter_index(h, at, 0)?;
        let c = &self.counters[ndx];
        Ok(c.is_countdown_timer && c.value < 1)
    }

    /// `setTimer(pAction, millisecondTimer, random)`: a random timer's bounds go through `GameLogicRandomValue`, an `Int`
    /// function, millisecond ones too (truncated).
    fn set_timer<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef, msec: bool, random: bool) -> R<()> {
        let ndx = self.counter_index(h, at, 0)?;
        if msec {
            let mut value = Self::preal(h, at, 1)?;
            if random {
                let r = Self::preal(h, at, 2)?;
                let (lo, hi) = (real_as_int(value)?, real_as_int(r)?);
                value = h.se_random_int(lo, hi) as f32;
            }
            self.counters[ndx].value = real_to_int_ceil(convert_duration_from_msecs_to_frames(value * 1000.0));
        } else {
            let mut value = Self::pint(h, at, 1)?;
            if random {
                let r = Self::pint(h, at, 2)?;
                value = h.se_random_int(value, r);
            }
            self.counters[ndx].value = value;
        }
        self.counters[ndx].is_countdown_timer = true;
        Ok(())
    }

    /// `pauseTimer(pAction)`.
    fn pause_timer<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<()> {
        let ndx = self.counter_index(h, at, 0)?;
        self.counters[ndx].is_countdown_timer = false;
        Ok(())
    }

    /// `restartTimer(pAction)`: only a timer with time left.
    fn restart_timer<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<()> {
        let ndx = self.counter_index(h, at, 0)?;
        if self.counters[ndx].value > 0 {
            self.counters[ndx].is_countdown_timer = true;
        }
        Ok(())
    }

    /// `adjustTimer(pAction, millisecondTimer, add)`: the value is the first parameter, the timer the second.
    fn adjust_timer<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef, msec: bool, add: bool) -> R<()> {
        let ndx = self.counter_index(h, at, 1)?;
        let delta = if msec {
            let mut v = Self::preal(h, at, 0)?;
            if !add {
                v = -v;
            }
            real_to_int_ceil(convert_duration_from_msecs_to_frames(v * 1000.0))
        } else {
            let v = Self::pint(h, at, 0)?;
            if add { v } else { v.checked_neg().ok_or_else(|| undefined(Undefined::SignedIntOverflow))? }
        };
        let c = &mut self.counters[ndx].value;
        *c = c.checked_add(delta).ok_or_else(|| undefined(Undefined::SignedIntOverflow))?;
        Ok(())
    }

    /// `enableScript` (`true`: the group, then the script) and `disableScript` (the script, then the group).
    fn enable_script<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef, enable: bool) -> R<()> {
        let name = Self::pstr(h, at, 0)?;
        let g = self.find_group(h, &name);
        let s = self.find_script(h, &name);
        if let Some((side, g)) = g {
            h.se_script_list_mut(side).expect("found").groups[g].is_group_active = enable;
        }
        if let Some(r) = s {
            Self::script_mut(h, r).expect("found").is_active = enable;
        }
        Ok(())
    }

    // --------------------------------------------------------------------------------------------------------------------
    // notifications
    // --------------------------------------------------------------------------------------------------------------------

    /// `notifyOfObjectCreationOrDestruction()`.
    pub fn notify_of_object_creation_or_destruction<H: ScriptEngineHost>(&mut self, h: &H) {
        self.frame_object_count_changed = h.se_frame();
    }

    /// `notifyOfCompletedVideo(name)`.
    pub fn notify_of_completed_video(&mut self, name: &[u8]) {
        if plant(3907) {
            self.completed_video.insert(0, name.to_vec());
        } else {
            self.completed_video.push(name.to_vec());
        }
    }

    /// `notifyOfTriggeredSpecialPower`, `...Midway...`, `...Completed...`, `notifyOfCompletedUpgrade` (`which` 0..4): the player
    /// index is unchecked.
    pub fn notify_of_power(&mut self, which: usize, player: i32, name: &[u8], source: ObjectId) -> R<()> {
        let i = slot(player, MAX_PLAYER_COUNT)?;
        let list = match which {
            0 => &mut self.triggered_special_powers,
            1 => &mut self.midway_special_powers,
            2 => &mut self.finished_special_powers,
            _ => &mut self.completed_upgrades,
        };
        list[i].push((name.to_vec(), source));
        Ok(())
    }

    /// `notifyOfAcquiredScience(playerIndex, science)` (unchecked).
    pub fn notify_of_acquired_science(&mut self, player: i32, science: i32) -> R<()> {
        let i = slot(player, MAX_PLAYER_COUNT)?;
        self.acquired_sciences[i].push(science);
        Ok(())
    }

    /// `signalUIInteract(hookName)`: at the front.
    pub fn signal_ui_interact(&mut self, hook: &[u8]) {
        self.ui_interactions.insert(0, hook.to_vec());
    }

    /// `isVideoComplete(name, remove)`.
    pub fn is_video_complete(&mut self, name: &[u8], remove: bool) -> bool {
        let Some(i) = self.completed_video.iter().position(|v| v == name) else { return false };
        if remove {
            self.completed_video.remove(i);
        }
        true
    }

    /// `isSpeechComplete` (`audio` false) and `isAudioComplete`: the first ask records the frame the sound ends (its length in
    /// whole frames after now).
    pub fn is_sound_complete<H: ScriptEngineHost>(&mut self, h: &mut H, name: &[u8], remove: bool, audio: bool) -> bool {
        let i = match (if audio { &self.testing_audio } else { &self.testing_speech }).iter().position(|(n, _)| n == name) {
            Some(i) => i,
            None => {
                let len = h.se_audio_length_ms(name);
                let frames = real_to_unsigned_int(len / MSEC_PER_LOGICFRAME_REAL);
                let end = frames.wrapping_add(h.se_frame());
                let list = if audio { &mut self.testing_audio } else { &mut self.testing_speech };
                list.insert(0, (name.to_vec(), end));
                0
            }
        };
        let list = if audio { &mut self.testing_audio } else { &mut self.testing_speech };
        if h.se_frame() >= list[i].1 {
            if remove {
                list.remove(i);
            }
            return true;
        }
        false
    }

    /// `isSpecialPowerTriggered`, `...Midway`, `...Complete`, `isUpgradeComplete` (`which` 0..4): a player outside the range is
    /// false; `INVALID_ID` matches any source.
    pub fn is_power_done(&mut self, which: usize, player: i32, name: &[u8], remove: bool, source: ObjectId) -> bool {
        if !(0..MAX_PLAYER_COUNT as i32).contains(&player) {
            return false;
        }
        let list = match which {
            0 => &mut self.triggered_special_powers,
            1 => &mut self.midway_special_powers,
            2 => &mut self.finished_special_powers,
            _ => &mut self.completed_upgrades,
        };
        let l = &mut list[player as usize];
        let Some(i) = l.iter().position(|(n, s)| n == name && (source == INVALID_ID || source == *s || plant(3909))) else { return false };
        if remove {
            l.remove(i);
        }
        true
    }

    /// `isScienceAcquired(playerIndex, science, remove)`.
    pub fn is_science_acquired(&mut self, player: i32, science: i32, remove: bool) -> bool {
        if !(0..MAX_PLAYER_COUNT as i32).contains(&player) {
            return false;
        }
        let l = &mut self.acquired_sciences[player as usize];
        let Some(i) = l.iter().position(|s| *s == science) else { return false };
        if remove {
            l.remove(i);
        }
        true
    }

    /// `setToppleDirection(name, direction)`: changed, or removed (`None`); a new name goes at the front (its direction read
    /// unchecked).
    pub fn set_topple_direction(&mut self, name: &[u8], direction: Option<Coord3D>) -> R<()> {
        if name.is_empty() {
            return Ok(());
        }
        if let Some(i) = self.topple_directions.iter().position(|(n, _)| n == name) {
            match direction {
                Some(d) => self.topple_directions[i].1 = d,
                None => {
                    self.topple_directions.remove(i);
                }
            }
            return Ok(());
        }
        let d = direction.ok_or_else(|| undefined(Undefined::NullLocation))?;
        self.topple_directions.insert(0, (name.to_vec(), d));
        Ok(())
    }

    /// `adjustToppleDirection(object, direction)`: a named object's set direction, normalised.
    pub fn adjust_topple_direction<H: ScriptEngineHost>(&self, h: &H, o: ObjH, direction: &mut Coord3D) {
        let name = h.se_obj_name(o);
        if name.is_empty() {
            return;
        }
        if let Some((_, d)) = self.topple_directions.iter().find(|(n, _)| *n == name) {
            *direction = *d;
            direction.normalize();
        }
    }

    /// `adjustToppleDirection(object, Coord2D *direction)`.
    pub fn adjust_topple_direction_2d<H: ScriptEngineHost>(&self, h: &H, o: ObjH, direction: &mut Coord2D) {
        let mut d = Coord3D { x: direction.x, y: direction.y, z: 0.0 };
        self.adjust_topple_direction(h, o, &mut d);
        direction.x = d.x;
        direction.y = d.y;
    }

    // --------------------------------------------------------------------------------------------------------------------
    // the named objects
    // --------------------------------------------------------------------------------------------------------------------

    /// `addObjectToCache(obj)`: a named object takes a dead name's place, renames its own entry, or is added; a name a living
    /// object has is refused.
    pub fn add_object_to_cache<H: ScriptEngineHost>(&mut self, h: &H, o: ObjH) {
        let name = h.se_obj_name(o);
        if name.is_empty() {
            return;
        }
        for e in self.named_objects.iter_mut() {
            if e.0 == name {
                if e.1.is_none() {
                    e.1 = Some(o);
                }
                return;
            }
            if e.1 == Some(o) {
                e.0 = name;
                return;
            }
        }
        self.named_objects.push((name, Some(o)));
    }

    /// `removeObjectFromCache(obj)`: its entry kept, without the object.
    pub fn remove_object_from_cache(&mut self, o: ObjH) {
        if let Some(e) = self.named_objects.iter_mut().find(|e| e.1 == Some(o)) {
            e.1 = None;
        }
    }

    /// `transferObjectName(name, newObject)`: the new object named, and the name's entry pointed at it (with the old object's
    /// custom indicator colour, or none).
    pub fn transfer_object_name<H: ScriptEngineHost>(&mut self, h: &mut H, name: &[u8], o: ObjH) {
        if name.is_empty() {
            return;
        }
        if !h.se_obj_name(o).is_empty() {
            self.remove_object_from_cache(o);
        }
        h.se_obj_set_name(o, name);
        if let Some(e) = self.named_objects.iter_mut().find(|e| e.0 == name) {
            if let Some(old) = e.1 {
                if h.se_obj_has_custom_indicator_color(old) && !plant(3902) {
                    let c = h.se_obj_indicator_color(old);
                    h.se_obj_set_custom_indicator_color(o, c);
                } else {
                    h.se_obj_remove_custom_indicator_color(o);
                }
            }
            e.1 = Some(o);
        }
    }

    /// `notifyOfObjectDestruction(obj)`.
    pub fn notify_of_object_destruction<H: ScriptEngineHost>(&mut self, h: &H, o: ObjH) {
        if !h.se_obj_name(o).is_empty() {
            self.remove_object_from_cache(o);
        }
        if self.condition_object == Some(o) {
            self.condition_object = None;
        }
        if self.calling_object == Some(o) {
            self.calling_object = None;
        }
    }

    /// `createNamedCache()`: every named object, in the logic's order.
    pub fn create_named_cache<H: ScriptEngineHost>(&mut self, h: &H) {
        self.named_objects.clear();
        for o in h.se_objects() {
            let n = h.se_obj_name(o);
            if !n.is_empty() {
                self.named_objects.push((n, Some(o)));
            }
        }
    }

    // --------------------------------------------------------------------------------------------------------------------
    // evaluating and executing scripts
    // --------------------------------------------------------------------------------------------------------------------

    /// `checkConditionsForTeamNames(pScript)`: the frame to evaluate at (a random offset for a delayed script), and the condition
    /// team: the one non-singleton team the conditions name, else a singleton one.
    fn check_conditions_for_team_names<H: ScriptEngineHost>(&mut self, h: &mut H, r: ScriptRef) -> R<()> {
        let delay = Self::script(h, r).expect("script").delay_evaluation_seconds;
        let frame = if delay > 0 { h.se_random_int(0, 2 * LOGICFRAMES_PER_SECOND) as u32 } else { 0 };
        Self::script_mut(h, r).expect("script").frame_to_evaluate_at = frame;
        let mut singleton_name: Vec<u8> = Vec::new();
        let mut multi_name: Vec<u8> = Vec::new();
        let script = Self::script(h, r).expect("script");
        for or in &script.conditions {
            for c in &or.conditions {
                for i in 0..c.num_parms {
                    let p = parm(c.get_parameter(i))?;
                    if p.param_type != parameter_type::TEAM {
                        continue;
                    }
                    let Some(proto) = h.se_find_team_prototype(&p.string) else { continue };
                    let singleton = h.se_proto_is_singleton(proto) || h.se_proto_max_instances(proto) < 2;
                    if singleton {
                        singleton_name = p.string.clone();
                    } else if multi_name.is_empty() {
                        multi_name = p.string.clone();
                    }
                }
            }
        }
        let name = if multi_name.is_empty() { (!singleton_name.is_empty()).then_some(singleton_name) } else { Some(multi_name) };
        if let Some(n) = name {
            Self::script_mut(h, r).expect("script").condition_team_name = n;
        }
        Ok(())
    }

    /// `executeScripts(head)` over a side's own scripts (`group` `None`) or a group's, `n` of them: the subroutines skipped.
    fn execute_scripts_of<H: ScriptEngineHost>(&mut self, h: &mut H, side: usize, group: Option<usize>, n: usize) -> R<()> {
        for index in 0..n {
            let r = ScriptRef { side, group, index };
            let Some(s) = Self::script(h, r) else { break };
            if s.is_subroutine {
                continue;
            }
            self.execute_script(h, r)?;
        }
        Ok(())
    }

    /// `executeScript(pScript)`.
    pub fn execute_script<H: ScriptEngineHost>(&mut self, h: &mut H, r: ScriptRef) -> R<()> {
        Self::script_mut(h, r).expect("script").cur_time = 0.0;
        let s = Self::script(h, r).expect("script");
        if !s.is_active {
            return Ok(());
        }
        let mut diff = self.game_difficulty;
        if let Some(p) = self.current_player {
            diff = h.se_player_difficulty(p);
        }
        let s = Self::script(h, r).expect("script");
        let ok = match diff {
            difficulty::EASY => s.easy,
            difficulty::NORMAL => s.normal,
            difficulty::HARD => s.hard,
            _ => true,
        };
        if !ok {
            return Ok(());
        }
        if h.se_frame() < s.frame_to_evaluate_at {
            return Ok(());
        }
        let delay = s.delay_evaluation_seconds;
        if delay > 0 {
            let frames = delay.checked_mul(LOGICFRAMES_PER_SECOND).ok_or_else(|| undefined(Undefined::SignedIntOverflow))?;
            let f = h.se_frame().wrapping_add(frames as u32);
            Self::script_mut(h, r).expect("script").frame_to_evaluate_at = f;
        }
        let sav_condition_team = self.condition_team;
        let team_name = Self::script(h, r).expect("script").condition_team_name.clone();
        let proto = if team_name.is_empty() { None } else { h.se_find_team_prototype(&team_name) };
        let instances = proto.map(|p| h.se_proto_instances(p)).unwrap_or_default();
        if !instances.is_empty() {
            for t in instances {
                self.condition_team = Some(t);
                if self.evaluate_conditions(h, r, None, None)? {
                    if !Self::script(h, r).expect("script").actions.is_empty() {
                        self.execute_actions(h, r, true)?;
                    }
                    if Self::script(h, r).expect("script").is_one_shot {
                        Self::script_mut(h, r).expect("script").is_active = false;
                    }
                } else if !Self::script(h, r).expect("script").actions_false.is_empty() {
                    self.execute_actions(h, r, false)?;
                }
            }
        } else {
            self.condition_team = None;
            if self.evaluate_conditions(h, r, None, None)? {
                if !Self::script(h, r).expect("script").actions.is_empty() {
                    self.execute_actions(h, r, true)?;
                }
                if Self::script(h, r).expect("script").is_one_shot {
                    Self::script_mut(h, r).expect("script").is_active = false;
                }
            } else if !Self::script(h, r).expect("script").actions_false.is_empty() {
                self.execute_actions(h, r, false)?;
                if Self::script(h, r).expect("script").is_one_shot {
                    Self::script_mut(h, r).expect("script").is_active = false;
                }
            }
        }
        self.condition_team = sav_condition_team;
        Ok(())
    }

    /// `evaluateCondition(pCondition)`: false, true, counters, flags and timers here, the rest by `TheScriptConditions`.
    fn evaluate_condition<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<bool> {
        let ItemRef::Condition(r, o, c) = at else { unreachable!("a condition") };
        let ty = Self::script(h, r).expect("script").conditions[o].conditions[c].condition_type;
        match ty {
            ct::CONDITION_FALSE => Ok(false),
            ct::CONDITION_TRUE => Ok(true),
            ct::COUNTER => self.evaluate_counter(h, at),
            ct::FLAG => self.evaluate_flag(h, at),
            ct::TIMER_EXPIRED => self.evaluate_timer(h, at),
            _ => {
                let mut cond = Self::script(h, r).expect("script").conditions[o].conditions[c].clone();
                let v = h.se_evaluate_condition(self, &mut cond);
                if let Some(slot) = Self::script_mut(h, r).and_then(|s| s.conditions.get_mut(o)).and_then(|or| or.conditions.get_mut(c)) {
                    *slot = cond;
                }
                v
            }
        }
    }

    /// `evaluateConditions(pScript, thisTeam, player)`: the or-terms in order, each true when all its conditions are (stopping at
    /// the first false); an empty or-term is skipped.
    pub fn evaluate_conditions<H: ScriptEngineHost>(&mut self, h: &mut H, r: ScriptRef, this_team: Option<TeamH>, player: Option<PlayerH>) -> R<bool> {
        let sav_team = self.calling_team;
        let sav_player = self.current_player;
        self.calling_team = this_team;
        let mut player = player;
        if let Some(t) = this_team {
            player = h.se_team_controlling_player(t);
        }
        if player.is_none() {
            player = self.current_player;
        }
        self.current_player = player;
        let mut result = Ok(false);
        let n_or = Self::script(h, r).expect("script").conditions.len();
        'or: for o in 0..n_or {
            let n = Self::script(h, r).expect("script").conditions[o].conditions.len();
            if n == 0 {
                continue;
            }
            for c in 0..n {
                match self.evaluate_condition(h, ItemRef::Condition(r, o, c)) {
                    Ok(true) => {}
                    Ok(false) => continue 'or,
                    Err(e) => {
                        result = Err(e);
                        break 'or;
                    }
                }
            }
            result = Ok(true);
            break;
        }
        self.calling_team = sav_team;
        self.current_player = sav_player;
        result
    }

    /// `executeActions(head)` over the script's actions (`true`) or false actions.
    fn execute_actions<H: ScriptEngineHost>(&mut self, h: &mut H, r: ScriptRef, true_actions: bool) -> R<()> {
        let mut a = 0;
        loop {
            let s = Self::script(h, r).expect("script");
            let list = if true_actions { &s.actions } else { &s.actions_false };
            if a >= list.len() {
                return Ok(());
            }
            self.execute_action_at(h, ItemRef::Action(r, true_actions, a))?;
            a += 1;
        }
    }

    /// One action of `executeActions`.
    fn execute_action_at<H: ScriptEngineHost>(&mut self, h: &mut H, at: ItemRef) -> R<()> {
        let ItemRef::Action(r, f, a) = at else { unreachable!("an action") };
        let action_type = {
            let s = Self::script(h, r).expect("script");
            if f { &s.actions[a] } else { &s.actions_false[a] }.action_type
        };
        match action_type {
            at::SET_COUNTER => self.set_counter(h, at),
            at::SET_TREE_SWAY => self.set_sway(h, at),
            at::INCREMENT_COUNTER => self.add_counter(h, at, true),
            at::DECREMENT_COUNTER => self.add_counter(h, at, false),
            at::SET_FLAG => self.set_flag(h, at),
            at::STOP_TIMER => self.pause_timer(h, at),
            at::RESTART_TIMER => self.restart_timer(h, at),
            at::SET_TIMER => self.set_timer(h, at, false, false),
            at::SET_MILLISECOND_TIMER => self.set_timer(h, at, true, false),
            at::SET_RANDOM_TIMER => self.set_timer(h, at, false, true),
            at::SET_RANDOM_MSEC_TIMER => self.set_timer(h, at, true, true),
            at::ADD_TO_MSEC_TIMER => self.adjust_timer(h, at, true, true),
            at::SUB_FROM_MSEC_TIMER => self.adjust_timer(h, at, true, false),
            at::ENABLE_SCRIPT => self.enable_script(h, at, true),
            at::DISABLE_SCRIPT => self.enable_script(h, at, false),
            at::CALL_SUBROUTINE => {
                let name = Self::pstr(h, at, 0)?;
                self.call_subroutine_named(h, &name)
            }
            at::CAMERA_FADE_ADD | at::CAMERA_FADE_SUBTRACT | at::CAMERA_FADE_SATURATE | at::CAMERA_FADE_MULTIPLY => self.set_fade(h, at, action_type),
            at::SET_ATTACK_PRIORITY_THING => self.set_priority_thing(h, at),
            at::SET_ATTACK_PRIORITY_KIND_OF => self.set_priority_kind(h, at),
            at::SET_DEFAULT_ATTACK_PRIORITY => self.set_priority_default(h, at),
            at::NO_OP => Ok(()),
            _ => {
                let s = Self::script(h, r).expect("script");
                let action = if f { &s.actions[a] } else { &s.actions_false[a] }.clone();
                h.se_execute_action(self, &action)
            }
        }
    }

    /// `friend_executeAction(head, team)`: the script's actions executed for the team, as its player's.
    pub fn friend_execute_action<H: ScriptEngineHost>(&mut self, h: &mut H, r: ScriptRef, true_actions: bool, team: Option<TeamH>) -> R<()> {
        let sav_team = self.calling_team;
        let sav_player = self.current_player;
        self.calling_team = team;
        self.current_player = team.and_then(|t| h.se_team_controlling_player(t));
        let res = self.execute_actions(h, r, true_actions);
        self.calling_team = sav_team;
        self.current_player = sav_player;
        res
    }

    /// `removeObjectTypes(list)`.
    pub fn remove_object_types(&mut self, index: usize) {
        if index < self.all_object_type_lists.len() {
            self.all_object_type_lists.remove(index);
        }
    }

    /// `getActionTemplate` / `getConditionTemplate` are `logic::script_templates`'.
    ///
    /// `isTimeFrozenScript()`.
    pub fn is_time_frozen_script(&self) -> bool {
        self.freeze_by_script
    }

    /// `doFreezeTime()`.
    pub fn do_freeze_time(&mut self) {
        self.freeze_by_script = true;
    }

    /// `doUnfreezeTime()`.
    pub fn do_unfreeze_time(&mut self) {
        self.freeze_by_script = false;
    }

    /// `setGlobalDifficulty(difficulty)`.
    pub fn set_global_difficulty(&mut self, d: i32) {
        self.game_difficulty = d;
    }

    /// `hasShownMPLocalDefeatWindow()`.
    pub fn has_shown_mp_local_defeat_window(&self) -> bool {
        self.shown_mp_local_defeat_window
    }

    /// `markMPLocalDefeatWindowShown()`.
    pub fn mark_mp_local_defeat_window_shown(&mut self) {
        self.shown_mp_local_defeat_window = true;
    }

    // --------------------------------------------------------------------------------------------------------------------
    // sequential scripts
    // --------------------------------------------------------------------------------------------------------------------

    /// A `push_back` on `m_sequentialScripts`: a full vector reallocates (`size + max(size, 1)`), invalidating the iterator of a
    /// progress loop in course.
    fn push_chain(&mut self, chain: Vec<SequentialScript>) {
        if self.sequential_scripts.len() == self.sequential_capacity {
            self.sequential_capacity += self.sequential_capacity.max(1);
            if let Some((_, invalid)) = self.progressing.as_mut() {
                *invalid = true;
            }
        }
        self.sequential_scripts.push(chain);
    }

    /// An `erase` of chain `i`: at or before the progress loop's place, its iterator is invalidated.
    fn erase_chain(&mut self, i: usize) {
        if let Some((at, invalid)) = self.progressing.as_mut() {
            if i <= *at {
                *invalid = true;
            }
        }
        self.sequential_scripts.remove(i);
    }

    /// `appendSequentialScript(script)`: a copy (from its first instruction) at the end of the chain of the same object or team,
    /// else a new chain.
    pub fn append_sequential_script(&mut self, s: &SequentialScript) {
        let mut copy = *s;
        if !plant(3908) {
            copy.current_instruction = -1;
        }
        for chain in self.sequential_scripts.iter_mut() {
            let head = chain[0];
            if (s.object_id != INVALID_ID && s.object_id == head.object_id) || (s.team_to_exec_on.is_some() && s.team_to_exec_on == head.team_to_exec_on) {
                chain.push(copy);
                return;
            }
        }
        self.push_chain(vec![copy]);
    }

    /// `removeAllSequentialScripts(obj)`: every chain of the object, with what follows.
    pub fn remove_all_sequential_scripts_of_object(&mut self, id: ObjectId) {
        let mut i = 0;
        while i < self.sequential_scripts.len() {
            if self.sequential_scripts[i][0].object_id == id {
                self.erase_chain(i);
            } else {
                i += 1;
            }
        }
    }

    /// `notifyOfTeamDestruction(team)` (and `removeAllSequentialScripts(team)`): the team's chains removed; no longer the
    /// calling or condition team.
    pub fn notify_of_team_destruction(&mut self, team: TeamH) {
        let mut i = 0;
        while i < self.sequential_scripts.len() {
            if self.sequential_scripts[i][0].team_to_exec_on == Some(team) {
                self.erase_chain(i);
            } else {
                i += 1;
            }
        }
        if self.calling_team == Some(team) {
            self.calling_team = None;
        }
        if self.condition_team == Some(team) {
            self.condition_team = None;
        }
    }

    /// `setSequentialTimer(obj, frames)`: the first chain of the object.
    pub fn set_sequential_timer_object(&mut self, id: ObjectId, frames: i32) {
        if let Some(c) = self.sequential_scripts.iter_mut().find(|c| c[0].object_id == id) {
            c[0].frames_to_wait = frames;
        }
    }

    /// `setSequentialTimer(team, frames)`: the first chain of the team.
    pub fn set_sequential_timer_team(&mut self, team: TeamH, frames: i32) {
        if let Some(c) = self.sequential_scripts.iter_mut().find(|c| c[0].team_to_exec_on == Some(team)) {
            c[0].frames_to_wait = frames;
        }
    }

    /// `cleanupSequentialScript(it, cleanDanglers)`: the chain's head deleted (`false`), or the chain; an emptied chain erased.
    fn cleanup_sequential_script(&mut self, i: usize, clean_danglers: bool) {
        if clean_danglers {
            self.sequential_scripts[i].clear();
        } else {
            self.sequential_scripts[i].remove(0);
        }
        if self.sequential_scripts[i].is_empty() {
            self.sequential_scripts.remove(i);
        }
    }

    /// `evaluateAndProgressAllSequentialScripts()`.
    fn evaluate_and_progress_all_sequential_scripts<H: ScriptEngineHost>(&mut self, h: &mut H) -> R<()> {
        let mut it = 0usize;
        let mut last_it = usize::MAX; // end()
        let mut spin = 0;
        while it < self.sequential_scripts.len() {
            if it == last_it {
                spin += 1;
            } else {
                spin = 0;
            }
            if spin > MAX_SPIN_COUNT {
                it += 1;
                continue;
            }
            last_it = it;
            let mut advanced = false;
            let seq = self.sequential_scripts[it][0];
            let team = seq.team_to_exec_on;
            let obj = h.se_find_object(seq.object_id);
            if obj.is_none() && team.is_none() {
                self.cleanup_sequential_script(it, false);
                continue;
            }
            self.current_player = match (obj, team) {
                (Some(o), _) => h.se_obj_controlling_player(o),
                (None, Some(t)) => h.se_team_controlling_player(t),
                _ => None,
            };
            if let Some(p) = self.current_player {
                if !h.se_player_is_skirmish_ai(p) {
                    self.current_player = None;
                }
            }
            let ai = obj.filter(|o| h.se_obj_has_ai(*o));
            let mut group = team.map(|t| h.se_team_as_new_ai_group(t));
            if ai.is_none() && group.is_none() {
                it += 1;
                continue;
            }
            let idle = ai.is_some_and(|o| h.se_ai_is_idle(o)) || group.is_some_and(|g| h.se_group_is_idle(g));
            if (idle && seq.frames_to_wait < 1) || seq.frames_to_wait == 0 {
                {
                    let head = &mut self.sequential_scripts[it][0];
                    if head.dont_advance_instruction {
                        head.dont_advance_instruction = false;
                    } else {
                        head.current_instruction = head.current_instruction.wrapping_add(1);
                    }
                }
                let instruction = self.sequential_scripts[it][0].current_instruction;
                let n_actions = Self::script(h, seq.script).map_or(0, |s| s.actions.len());
                // `while (action && instruction) { --instruction; action = action->getNext(); }`: a negative instruction runs
                // off the end
                let action = (instruction >= 0 && (instruction as usize) < n_actions).then_some(instruction as usize);
                if let Some(a) = action {
                    self.condition_team = team;
                    self.condition_object = obj;
                    self.sequential_scripts[it][0].frames_to_wait = -1;
                    let at = ItemRef::Action(seq.script, true, a);
                    let act = Self::script(h, seq.script).expect("script").actions[a].clone();
                    self.progressing = Some((it, false));
                    let res = match act.action_type {
                        at::SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_ALL | at::SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_PARTIAL => {
                            let all = act.action_type == at::SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_ALL;
                            h.se_skirmish_command_button_is_ready(self, act.get_parameter(1), act.get_parameter(2), all).map(|ready| !ready)
                        }
                        at::TEAM_WAIT_FOR_NOT_CONTAINED_ALL | at::TEAM_WAIT_FOR_NOT_CONTAINED_PARTIAL => {
                            let all = act.action_type == at::TEAM_WAIT_FOR_NOT_CONTAINED_ALL;
                            h.se_team_is_contained(self, act.get_parameter(0), all)
                        }
                        _ => self.execute_action_at(h, at).map(|()| false),
                    };
                    let (_, invalid) = self.progressing.take().expect("set");
                    let wait = res?;
                    if invalid {
                        return Err(undefined(Undefined::InvalidatedIterator));
                    }
                    if wait {
                        self.sequential_scripts[it][0].dont_advance_instruction = true;
                    }
                    if self.sequential_scripts[it][0].dont_advance_instruction {
                        it += 1;
                        continue;
                    }
                    if ai.is_some_and(|o| h.se_ai_is_idle(o)) {
                        advanced = true;
                    } else if let Some(t) = team {
                        group = Some(h.se_team_as_new_ai_group(t));
                    }
                    if group.is_some_and(|g| h.se_group_is_idle(g)) {
                        advanced = true;
                    }
                    if advanced {
                        if obj.is_some_and(|o| h.se_obj_is_effectively_dead(o)) {
                            self.cleanup_sequential_script(it, true);
                            continue;
                        }
                        if group.is_some_and(|g| h.se_group_is_ai_dead(g)) {
                            self.cleanup_sequential_script(it, true);
                            continue;
                        }
                    }
                } else {
                    let head = &mut self.sequential_scripts[it][0];
                    if head.times_to_loop != 0 {
                        if head.times_to_loop != -1 {
                            head.times_to_loop -= 1;
                        }
                        head.frames_to_wait = -1;
                        let copy = *head;
                        self.progressing = Some((it, false));
                        self.append_sequential_script(&copy);
                        let (_, invalid) = self.progressing.take().expect("set");
                        if invalid {
                            return Err(undefined(Undefined::InvalidatedIterator));
                        }
                    }
                    self.cleanup_sequential_script(it, false);
                    advanced = true;
                }
            } else if seq.frames_to_wait > 0 {
                self.sequential_scripts[it][0].frames_to_wait -= 1;
            }
            if !advanced {
                it += 1;
            }
        }
        self.current_player = None;
        Ok(())
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// batch E4: save and load
// ------------------------------------------------------------------------------------------------------------------------

/// `throw SC_INVALID_DATA` (a `SaveCode`, not an `XferStatus`: `ErrorUnknown`, as `StateMachine` maps it).
const SC_INVALID_DATA: XferStatus = XferStatus::ErrorUnknown;

/// A load that upstream completes into a state it cannot use without undefined behaviour, reported as this status, which the
/// caller must take as such: a sequential script whose script the map no longer has (loaded as NULL, dereferenced at its first
/// use, progressing or saving it), or a counter, flag or attack priority count beyond its array (`m_numCounters` is a loop bound
/// over `m_counters`, and a negative one its index when a name is added).
pub const LOAD_UNDEFINED: XferStatus = XferStatus::InvalidParameters;

/// A nested `xferSnapshot`: the closure is the snapshot's `xfer`; its `crc` and `loadPostProcess` do nothing (as every snapshot
/// the engine nests: `SequentialScript`, `AttackPriorityInfo`, `ObjectTypes`).
struct Nested<F: FnMut(&mut dyn Xfer) -> XferResult<()>>(F);

impl<F: FnMut(&mut dyn Xfer) -> XferResult<()>> Snapshot for Nested<F> {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        (self.0)(xfer)
    }
    fn load_post_process(&mut self) {}
}

fn xfer_list_string(x: &mut dyn Xfer, list: &mut Vec<Vec<u8>>) -> XferResult<()> {
    let mut version: XferVersion = 1;
    x.xfer_version(&mut version, 1)?;
    let mut count = list.len() as u16;
    x.xfer_unsigned_short(&mut count)?;
    if x.get_xfer_mode() == XferMode::Save {
        for s in list.iter() {
            let mut s = s.clone();
            x.xfer_ascii_string(&mut s)?;
        }
    } else {
        if !list.is_empty() {
            return Err(SC_INVALID_DATA);
        }
        for _ in 0..count {
            let mut s = Vec::new();
            x.xfer_ascii_string(&mut s)?;
            list.push(s);
        }
    }
    Ok(())
}

fn xfer_list_string_u32(x: &mut dyn Xfer, list: &mut Vec<(Vec<u8>, u32)>) -> XferResult<()> {
    let mut version: XferVersion = 1;
    x.xfer_version(&mut version, 1)?;
    let mut count = list.len() as u16;
    x.xfer_unsigned_short(&mut count)?;
    if x.get_xfer_mode() == XferMode::Save {
        for (s, v) in list.iter() {
            let (mut s, mut v) = (s.clone(), *v);
            x.xfer_ascii_string(&mut s)?;
            x.xfer_unsigned_int(&mut v)?;
        }
    } else {
        if !list.is_empty() {
            return Err(SC_INVALID_DATA);
        }
        for _ in 0..count {
            let (mut s, mut v) = (Vec::new(), 0u32);
            x.xfer_ascii_string(&mut s)?;
            x.xfer_unsigned_int(&mut v)?;
            list.push((s, v));
        }
    }
    Ok(())
}

fn xfer_list_string_id(x: &mut dyn Xfer, list: &mut Vec<(Vec<u8>, ObjectId)>) -> XferResult<()> {
    let mut version: XferVersion = 1;
    x.xfer_version(&mut version, 1)?;
    let mut count = list.len() as u16;
    x.xfer_unsigned_short(&mut count)?;
    if x.get_xfer_mode() == XferMode::Save {
        for (s, v) in list.iter() {
            let (mut s, mut v) = (s.clone(), *v);
            x.xfer_ascii_string(&mut s)?;
            x.xfer_object_id(&mut v)?;
        }
    } else {
        if !list.is_empty() {
            return Err(SC_INVALID_DATA);
        }
        for _ in 0..count {
            let (mut s, mut v) = (Vec::new(), INVALID_ID);
            x.xfer_ascii_string(&mut s)?;
            x.xfer_object_id(&mut v)?;
            list.push((s, v));
        }
    }
    Ok(())
}

fn xfer_list_string_coord(x: &mut dyn Xfer, list: &mut Vec<(Vec<u8>, Coord3D)>) -> XferResult<()> {
    let mut version: XferVersion = 1;
    x.xfer_version(&mut version, 1)?;
    let mut count = list.len() as u16;
    x.xfer_unsigned_short(&mut count)?;
    if x.get_xfer_mode() == XferMode::Save {
        for (s, c) in list.iter() {
            let (mut s, mut c) = (s.clone(), *c);
            x.xfer_ascii_string(&mut s)?;
            x.xfer_coord3d(&mut c)?;
        }
    } else {
        if !list.is_empty() {
            return Err(SC_INVALID_DATA);
        }
        for _ in 0..count {
            let (mut s, mut c) = (Vec::new(), Coord3D::default());
            x.xfer_ascii_string(&mut s)?;
            x.xfer_coord3d(&mut c)?;
            list.push((s, c));
        }
    }
    Ok(())
}

impl ScriptEngine {
    /// `xferScienceVec`: a science by its internal name (an unknown one on load is `XFER_UNKNOWN_STRING`; CRC: version and
    /// count only, as upstream's branches).
    fn xfer_science_vec<H: ScriptEngineHost>(h: &H, x: &mut dyn Xfer, v: &mut Vec<i32>) -> XferResult<()> {
        let mut version: XferVersion = 1;
        x.xfer_version(&mut version, 1)?;
        let mut count = v.len() as u16;
        x.xfer_unsigned_short(&mut count)?;
        match x.get_xfer_mode() {
            XferMode::Save => {
                for s in v.iter() {
                    let mut name = h.se_science_name(*s);
                    x.xfer_ascii_string(&mut name)?;
                }
            }
            XferMode::Load => {
                v.clear();
                for _ in 0..count {
                    let mut name = Vec::new();
                    x.xfer_ascii_string(&mut name)?;
                    v.push(h.se_science_from_name(&name).ok_or(XferStatus::UnknownString)?);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// `SequentialScript::xfer`: team id, object id, the script's name (found again by name on load: one that is gone is
    /// [`LOAD_UNDEFINED`]), instruction, loops, wait, the flag.
    fn xfer_sequential<H: ScriptEngineHost>(&self, h: &H, x: &mut dyn Xfer, q: &mut SequentialScript) -> XferResult<()> {
        let mut version: XferVersion = 1;
        x.xfer_version(&mut version, 1)?;
        let mut team_id = q.team_to_exec_on.unwrap_or(0);
        let mut b = team_id.to_le_bytes();
        x.xfer_user(&mut b)?;
        team_id = i32::from_le_bytes(b);
        if x.get_xfer_mode() == XferMode::Load {
            q.team_to_exec_on = (team_id != 0 && h.se_team_exists(team_id)).then_some(team_id);
            if team_id != 0 && q.team_to_exec_on.is_none() {
                return Err(SC_INVALID_DATA);
            }
        }
        x.xfer_object_id(&mut q.object_id)?;
        if x.get_xfer_mode() == XferMode::Save {
            let mut name = Self::script(h, q.script).map(|s| s.script_name.clone()).unwrap_or_default();
            x.xfer_ascii_string(&mut name)?;
        } else {
            let mut name = Vec::new();
            x.xfer_ascii_string(&mut name)?;
            if x.get_xfer_mode() == XferMode::Load {
                q.script = self.find_script(h, &name).ok_or(LOAD_UNDEFINED)?;
            }
        }
        x.xfer_int(&mut q.current_instruction)?;
        x.xfer_int(&mut q.times_to_loop)?;
        x.xfer_int(&mut q.frames_to_wait)?;
        x.xfer_bool(&mut q.dont_advance_instruction)
    }

    /// `AttackPriorityInfo::xfer`: name, default, the map's count and each template by name with its priority.
    fn xfer_attack_info<H: ScriptEngineHost>(h: &H, x: &mut dyn Xfer, a: &mut AttackPriorityInfo) -> XferResult<()> {
        let mut version: XferVersion = 1;
        x.xfer_version(&mut version, 1)?;
        x.xfer_ascii_string(&mut a.name)?;
        x.xfer_int(&mut a.default_priority)?;
        let mut count = a.priority_map.as_ref().map_or(0, |m| m.len()) as u16;
        x.xfer_unsigned_short(&mut count)?;
        if x.get_xfer_mode() == XferMode::Save {
            for (t, p) in a.priority_map.iter().flatten() {
                let (mut name, mut p) = (h.se_template_name(*t), *p);
                x.xfer_ascii_string(&mut name)?;
                x.xfer_int(&mut p)?;
            }
        } else {
            for _ in 0..count {
                let mut name = Vec::new();
                x.xfer_ascii_string(&mut name)?;
                let t = h.se_find_template(&name).ok_or(SC_INVALID_DATA)?;
                let mut p = 0;
                x.xfer_int(&mut p)?;
                let t = if plant(3921) { t } else { h.se_template_final_override(t) };
                a.set_priority(t, p);
            }
        }
        Ok(())
    }

    /// `xfer` (version 5): the sequential scripts (each chain's head only: what follows a head is not saved), the counters, the
    /// flags, the attack priority sets, the timers, the named objects (by id), the fade, the notifications, the topple
    /// directions, the breeze, the difficulty, the freeze, the named reveals, the object type lists, the bonus flag, the music
    /// track, the victim flag; a load with no fade starts the fade in from black.
    pub fn xfer<H: ScriptEngineHost>(&mut self, h: &mut H, x: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 5;
        x.xfer_version(&mut version, 5)?;
        let mut count = self.sequential_scripts.len() as u16;
        x.xfer_unsigned_short(&mut count)?;
        if x.get_xfer_mode() == XferMode::Save {
            for i in 0..self.sequential_scripts.len() {
                let mut head = self.sequential_scripts[i][0];
                let me = &*self;
                x.xfer_snapshot(&mut Nested(|x: &mut dyn Xfer| me.xfer_sequential(h, x, &mut head)))?;
            }
        } else {
            if !self.sequential_scripts.is_empty() {
                return Err(SC_INVALID_DATA);
            }
            for _ in 0..count {
                // `newInstance(SequentialScript)` pushed, then filled
                let mut q = SequentialScript::new(ScriptRef { side: 0, group: None, index: 0 });
                let me = &*self;
                let r = x.xfer_snapshot(&mut Nested(|x: &mut dyn Xfer| me.xfer_sequential(h, x, &mut q)));
                self.push_chain(vec![q]);
                r?;
            }
        }
        let mut n = self.num_counters as u16;
        x.xfer_unsigned_short(&mut n)?;
        if usize::from(n) > MAX_COUNTERS {
            return Err(SC_INVALID_DATA);
        }
        for c in &mut self.counters[..usize::from(n)] {
            x.xfer_int(&mut c.value)?;
            x.xfer_ascii_string(&mut c.name)?;
            x.xfer_bool(&mut c.is_countdown_timer)?;
        }
        x.xfer_int(&mut self.num_counters)?;
        let mut n = self.num_flags as u16;
        x.xfer_unsigned_short(&mut n)?;
        if usize::from(n) > MAX_FLAGS {
            return Err(SC_INVALID_DATA);
        }
        for f in &mut self.flags[..usize::from(n)] {
            x.xfer_bool(&mut f.value)?;
            x.xfer_ascii_string(&mut f.name)?;
        }
        x.xfer_int(&mut self.num_flags)?;
        let mut n = self.num_attack_info as u16;
        x.xfer_unsigned_short(&mut n)?;
        if usize::from(n) > MAX_ATTACK_PRIORITIES {
            return Err(SC_INVALID_DATA);
        }
        for a in &mut self.attack_priority_info[..usize::from(n)] {
            let hh = &*h;
            x.xfer_snapshot(&mut Nested(|x: &mut dyn Xfer| Self::xfer_attack_info(hh, x, a)))?;
        }
        x.xfer_int(&mut self.num_attack_info)?;
        x.xfer_int(&mut self.end_game_timer)?;
        x.xfer_int(&mut self.close_window_timer)?;
        let mut n = self.named_objects.len() as u16;
        x.xfer_unsigned_short(&mut n)?;
        if x.get_xfer_mode() == XferMode::Save {
            for (name, o) in &self.named_objects {
                let (mut name, mut id) = (name.clone(), if plant(3923) { INVALID_ID } else { o.unwrap_or(INVALID_ID) });
                x.xfer_ascii_string(&mut name)?;
                x.xfer_object_id(&mut id)?;
            }
        } else {
            self.named_objects.clear();
            for _ in 0..n {
                let (mut name, mut id) = (Vec::new(), INVALID_ID);
                x.xfer_ascii_string(&mut name)?;
                x.xfer_object_id(&mut id)?;
                let o = h.se_find_object(id);
                if o.is_none() && id != INVALID_ID {
                    return Err(SC_INVALID_DATA);
                }
                self.named_objects.push((name, o));
            }
        }
        x.xfer_bool(&mut self.first_update)?;
        let mut f = self.fade.to_le_bytes();
        x.xfer_user(&mut f)?;
        self.fade = i32::from_le_bytes(f);
        x.xfer_real(&mut self.min_fade)?;
        x.xfer_real(&mut self.max_fade)?;
        x.xfer_real(&mut self.cur_fade_value)?;
        x.xfer_int(&mut self.cur_fade_frame)?;
        x.xfer_int(&mut self.fade_frames_increase)?;
        x.xfer_int(&mut self.fade_frames_hold)?;
        x.xfer_int(&mut self.fade_frames_decrease)?;
        xfer_list_string(x, &mut self.completed_video)?;
        xfer_list_string_u32(x, &mut self.testing_speech)?;
        xfer_list_string_u32(x, &mut self.testing_audio)?;
        xfer_list_string(x, &mut self.ui_interactions)?;
        for which in 0..4 {
            let mut n = MAX_PLAYER_COUNT as u16;
            x.xfer_unsigned_short(&mut n)?;
            if usize::from(n) != MAX_PLAYER_COUNT {
                return Err(SC_INVALID_DATA);
            }
            let lists = match which {
                0 => &mut self.triggered_special_powers,
                1 => &mut self.midway_special_powers,
                2 => &mut self.finished_special_powers,
                _ => &mut self.completed_upgrades,
            };
            for l in lists.iter_mut() {
                xfer_list_string_id(x, l)?;
            }
        }
        let mut n = MAX_PLAYER_COUNT as u16;
        x.xfer_unsigned_short(&mut n)?;
        if usize::from(n) != MAX_PLAYER_COUNT {
            return Err(SC_INVALID_DATA);
        }
        for v in self.acquired_sciences.iter_mut() {
            Self::xfer_science_vec(h, x, v)?;
        }
        xfer_list_string_coord(x, &mut self.topple_directions)?;
        let b = &mut self.breeze_info;
        x.xfer_real(&mut b.direction)?;
        x.xfer_real(&mut b.direction_vec.x)?;
        x.xfer_real(&mut b.direction_vec.y)?;
        x.xfer_real(&mut b.intensity)?;
        x.xfer_real(&mut b.lean)?;
        x.xfer_real(&mut b.randomness)?;
        x.xfer_short(&mut b.breeze_period)?;
        if plant(3925) {
            let mut v = b.breeze_version;
            x.xfer_short(&mut v)?;
        } else {
            x.xfer_short(&mut b.breeze_version)?;
        }
        let mut d = self.game_difficulty.to_le_bytes();
        x.xfer_user(&mut d)?;
        self.game_difficulty = i32::from_le_bytes(d);
        x.xfer_bool(&mut self.freeze_by_script)?;
        if version >= 2 {
            let mut n = self.named_reveals.len() as u16;
            x.xfer_unsigned_short(&mut n)?;
            if x.get_xfer_mode() == XferMode::Save {
                for r in self.named_reveals.iter_mut() {
                    x.xfer_ascii_string(&mut r.reveal_name)?;
                    x.xfer_ascii_string(&mut r.waypoint_name)?;
                    x.xfer_real(&mut r.radius_to_reveal)?;
                    x.xfer_ascii_string(&mut r.player_name)?;
                }
            } else {
                if !self.named_reveals.is_empty() {
                    return Err(SC_INVALID_DATA);
                }
                // one `NamedReveal` read into for all: a field a short read leaves keeps the previous reveal's
                let mut r = NamedReveal::default();
                for _ in 0..n {
                    x.xfer_ascii_string(&mut r.reveal_name)?;
                    x.xfer_ascii_string(&mut r.waypoint_name)?;
                    x.xfer_real(&mut r.radius_to_reveal)?;
                    x.xfer_ascii_string(&mut r.player_name)?;
                    let mut kept = r.clone();
                    if plant(3924) {
                        kept.radius_to_reveal *= 2.0;
                    }
                    self.named_reveals.push(kept);
                }
            }
            let mut n = self.all_object_type_lists.len() as u16;
            x.xfer_unsigned_short(&mut n)?;
            if x.get_xfer_mode() == XferMode::Save {
                for t in self.all_object_type_lists.iter_mut() {
                    x.xfer_snapshot(&mut Nested(|x: &mut dyn Xfer| t.xfer(x)))?;
                }
            } else {
                if !self.all_object_type_lists.is_empty() {
                    return Err(SC_INVALID_DATA);
                }
                for _ in 0..n {
                    let mut t = ObjectTypes::new();
                    x.xfer_snapshot(&mut Nested(|x: &mut dyn Xfer| t.xfer(x)))?;
                    self.all_object_type_lists.push(t);
                }
            }
        }
        if version >= 3 {
            x.xfer_bool(&mut self.objects_should_receive_difficulty_bonus)?;
        } else {
            self.objects_should_receive_difficulty_bonus = true;
        }
        if version >= 4 {
            x.xfer_ascii_string(&mut self.current_track_name)?;
        }
        if version >= 5 {
            x.xfer_bool(&mut self.choose_victim_always_uses_normal)?;
        } else {
            self.choose_victim_always_uses_normal = false;
        }
        if x.get_xfer_mode() == XferMode::Load
            && (!(0..=MAX_COUNTERS as i32).contains(&self.num_counters)
                || !(0..=MAX_FLAGS as i32).contains(&self.num_flags)
                || !(0..=MAX_ATTACK_PRIORITIES as i32).contains(&self.num_attack_info))
        {
            return Err(LOAD_UNDEFINED);
        }
        if x.get_xfer_mode() == XferMode::Load && self.fade == fade::NONE && !plant(3922) {
            self.fade = fade::MULTIPLY;
            self.cur_fade_frame = 0;
            self.min_fade = 1.0;
            self.max_fade = 0.0;
            self.fade_frames_increase = 0;
            self.fade_frames_hold = 0;
            self.fade_frames_decrease = FRAMES_TO_FADE_IN_AT_START;
            self.cur_fade_value = 0.0;
        }
        Ok(())
    }

    /// `loadPostProcess()`: the difficulty bonuses set again; the music track (if any) played for the local player (read
    /// unchecked).
    pub fn load_post_process<H: ScriptEngineHost>(&mut self, h: &mut H) -> R<()> {
        h.se_actions_enable_difficulty_bonuses(self.objects_should_receive_difficulty_bonus);
        if !self.current_track_name.is_empty() {
            let p = h.se_local_player().ok_or_else(|| undefined(Undefined::NullLocalPlayer))?;
            let name = self.current_track_name.clone();
            h.se_audio_add_event(&name, p + i32::from(plant(3926)));
        }
        Ok(())
    }
}
