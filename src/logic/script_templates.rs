// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp (ScriptEngine::init, lines 587-5266)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The script engine's condition and action templates: what `ScriptEngine::init` writes into `m_conditionTemplates` and
//! `m_actionTemplates` (each type's internal name, editor name, editor strings and parameter types), and the name keys it gives
//! them, as the [`ScriptTemplates`] the script reader and writer take.
//!
//! The tables are data, extracted mechanically from `init`'s assignments (comments dropped, adjacent string literals joined, the
//! assignments applied in order, so a field set twice keeps its second value) and checked against the compiled `init` (see
//! below). An entry is what the template holds after `init`, on the memory `NEW ScriptEngine` gives it: the allocator zeroes it
//! (`DynamicMemoryAllocator::allocateBytesImplementation`), so a parameter slot `init` never sets is `0`, `Parameter::INT`, and
//! `Template()` gives the editor name `"UNUSED/(placeholder)/placeholder"` and no strings or parameters.
//!
//! # Upstream facts kept
//!
//! - Seven types are never set by `init` ([`UNSET`]): the conditions `OBSOLETE_SCRIPT_1`, `OBSOLETE_SCRIPT_2`,
//!   `DEFUNCT_PLAYER_SELECTED_GENERAL`, `DEFUNCT_PLAYER_SELECTED_GENERAL_FROM_NAMED`, `UNIT_COMPLETED_SEQUENTIAL_EXECUTION`,
//!   `TEAM_COMPLETED_SEQUENTIAL_EXECUTION`, and the action `NAMED_RECEIVE_UPGRADE`: an empty internal name, no parameters.
//! - `CAMERA_MOTION_BLUR_FOLLOW` declares one parameter and sets `m_parameters[1]`, so its one parameter is slot 0, `INT` by the
//!   zeroed memory (and its type is `INT` either way).
//! - `TEAM_ALL_USE_COMMANDBUTTON_ON_NAMED` sets three parameter types but declares two; `SKIRMISH_BUILD_BUILDING`,
//!   `SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST`, `TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_KINDOF` (whose fourth string is never set)
//!   and the condition `NAMED_INSIDE_AREA` set more editor strings than they declare; `VICTORY`, `DEFEAT`, `CAMERA_FADE_MULTIPLY`,
//!   `SET_BASE_CONSTRUCTION_SPEED` and `NAMED_INSIDE_AREA` set some fields twice.
//! - Every editor name ends in `[index]` (appended after the tables, [`TemplateText::ui_name`]), and the name keys are made in
//!   index order, the conditions first ([`script_templates`]).
//!
//! Not translated: the rest of `init` (the debug and particle editor DLLs, VTune, `TheScriptActions->init()` and
//! `TheScriptConditions->init()`, which their modules translate, and `reset()`, which the engine's own module will).
//!
//! Differential-verified in `tests/differential_script_templates.rs` against `init`'s template code compiled unchanged on the real
//! `Template` classes of `Scripts.h`: every field of the 453 templates.
//!
//! `p1w13-sonnet`, layer 11, batch E1, 2026-10-31.

use crate::common::name_key_generator::NameKeyGenerator;
use crate::logic::modules::plant;
use crate::logic::scripts::parameter_type as P;
use crate::logic::scripts::{action_type, condition_type, ScriptTemplate, ScriptTemplates, MAX_PARMS};

/// One `Template` after `init`.
#[derive(Clone, Copy, Debug)]
pub struct TemplateText {
    /// `m_uiName` before `init` appends the index.
    pub ui_name: &'static [u8],
    /// `m_internalName`.
    pub internal_name: &'static [u8],
    /// `m_numUiStrings`.
    pub num_ui_strings: i32,
    /// `m_uiStrings`, up to the last one set (the rest are empty).
    pub ui_strings: &'static [&'static [u8]],
    /// `m_numParameters`.
    pub num_parameters: i32,
    /// `m_parameters`, up to the last one set to other than `INT` (the rest are `INT`, the zeroed memory's or set so).
    pub parameters: &'static [i32],
}

/// A template `init` never sets: `Template()` on zeroed memory.
pub const UNSET: TemplateText =
    TemplateText { ui_name: b"UNUSED/(placeholder)/placeholder", internal_name: b"", num_ui_strings: 0, ui_strings: &[], num_parameters: 0, parameters: &[] };

impl TemplateText {
    /// `m_uiName` after `init`: with `[index]` appended.
    pub fn ui_name(&self, index: i32) -> Vec<u8> {
        let mut s = self.ui_name.to_vec();
        s.extend_from_slice(format!("[{}]", index + i32::from(plant(3891))).as_bytes());
        s
    }

    /// `m_uiStrings[i]`.
    pub fn ui_string(&self, i: usize) -> &'static [u8] {
        self.ui_strings.get(i).copied().unwrap_or(b"")
    }

    /// `m_parameters[i]` (`i < MAX_PARMS`).
    pub fn parameter(&self, i: usize) -> i32 {
        debug_assert!(i < MAX_PARMS);
        self.parameters.get(i).copied().unwrap_or(if plant(3893) { P::REAL } else { P::INT })
    }

    /// The template as the script reader sees it: the internal name's key and the declared parameters' types.
    fn script_template(&self, names: &mut NameKeyGenerator) -> ScriptTemplate {
        ScriptTemplate {
            internal_name_key: names.name_to_key(self.internal_name),
            parameters: if plant(3894) { self.parameters.to_vec() } else { (0..self.num_parameters as usize).map(|i| self.parameter(i)).collect() },
        }
    }
}

/// `init`'s last loops: each template's name key (`NAMEKEY(m_internalName)`), the conditions first, then the actions.
pub fn script_templates(names: &mut NameKeyGenerator) -> ScriptTemplates {
    let (conditions, actions): (Vec<ScriptTemplate>, Vec<ScriptTemplate>) = if plant(3892) {
        let a = ACTION_TEMPLATES.iter().map(|t| t.script_template(names)).collect();
        (CONDITION_TEMPLATES.iter().map(|t| t.script_template(names)).collect(), a)
    } else {
        let c = CONDITION_TEMPLATES.iter().map(|t| t.script_template(names)).collect();
        (c, ACTION_TEMPLATES.iter().map(|t| t.script_template(names)).collect())
    };
    ScriptTemplates::new(conditions, actions).expect("the tables have Condition::NUM_ITEMS and ScriptAction::NUM_ITEMS entries")
}

const _: () = assert!(CONDITION_TEMPLATES.len() == condition_type::NUM_ITEMS as usize);
const _: () = assert!(ACTION_TEMPLATES.len() == action_type::NUM_ITEMS as usize);

// Generated by the extraction described in the module doc: one entry per type, in index order.
/// `m_conditionTemplates` after `init`, by `ConditionType`.
pub static CONDITION_TEMPLATES: [TemplateText; 109] = [
    // 0
    TemplateText {
        ui_name: b"Scripting_/ False.",
        internal_name: b"CONDITION_FALSE",
        num_ui_strings: 1,
        ui_strings: &[
            b"False.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 1
    TemplateText {
        ui_name: b"Scripting_/ Counter compared to a value.",
        internal_name: b"COUNTER",
        num_ui_strings: 3,
        ui_strings: &[
            b"Counter ",
            b" IS ",
            b" ",
        ],
        num_parameters: 3,
        parameters: &[P::COUNTER, P::COMPARISON],
    },
    // 2
    TemplateText {
        ui_name: b"Scripting_/ Flag compared to a value.",
        internal_name: b"FLAG",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" IS ",
        ],
        num_parameters: 2,
        parameters: &[P::FLAG, P::BOOLEAN],
    },
    // 3
    TemplateText {
        ui_name: b"Scripting_/ True.",
        internal_name: b"CONDITION_TRUE",
        num_ui_strings: 1,
        ui_strings: &[
            b"True.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 4
    TemplateText {
        ui_name: b"Scripting_/ Timer expired.",
        internal_name: b"TIMER_EXPIRED",
        num_ui_strings: 2,
        ui_strings: &[
            b"Timer ",
            b" has expired.",
        ],
        num_parameters: 1,
        parameters: &[P::COUNTER],
    },
    // 5
    TemplateText {
        ui_name: b"Player_/ All destroyed.",
        internal_name: b"PLAYER_ALL_DESTROYED",
        num_ui_strings: 2,
        ui_strings: &[
            b"Everything belonging to  ",
            b" has been destroyed.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 6
    TemplateText {
        ui_name: b"Player_/ All factories destroyed.",
        internal_name: b"PLAYER_ALL_BUILDFACILITIES_DESTROYED",
        num_ui_strings: 2,
        ui_strings: &[
            b"All factories belonging to  ",
            b" have been destroyed.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 7
    TemplateText {
        ui_name: b"Team_/ Team has units in an area.",
        internal_name: b"TEAM_INSIDE_AREA_PARTIALLY",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" has one or more units in ",
            b" (",
            b").",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::TRIGGER_AREA, P::SURFACES_ALLOWED],
    },
    // 8
    TemplateText {
        ui_name: b"Team_/ Team is destroyed.",
        internal_name: b"TEAM_DESTROYED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been destroyed.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 9
    TemplateText {
        ui_name: b"Camera_/ Camera movement finished.",
        internal_name: b"CAMERA_MOVEMENT_FINISHED",
        num_ui_strings: 1,
        ui_strings: &[
            b"The camera movement has finished.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 10
    TemplateText {
        ui_name: b"Team_/ Team has units.",
        internal_name: b"TEAM_HAS_UNITS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has one or more units.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 11
    TemplateText {
        ui_name: b"Team_/ Team state is.",
        internal_name: b"TEAM_STATE_IS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" state IS ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::TEAM_STATE],
    },
    // 12
    TemplateText {
        ui_name: b"Team_/ Team state is not.",
        internal_name: b"TEAM_STATE_IS_NOT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" state IS NOT ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::TEAM_STATE],
    },
    // 13
    TemplateText {
        ui_name: b"Unit_/ Unit inside an area.",
        internal_name: b"NAMED_INSIDE_AREA",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is inside ",
            b" (",
            b").",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::TRIGGER_AREA],
    },
    // 14
    TemplateText {
        ui_name: b"Unit_/ Unit outside an area.",
        internal_name: b"NAMED_OUTSIDE_AREA",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is outside ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::TRIGGER_AREA],
    },
    // 15
    TemplateText {
        ui_name: b"Unit_/ Unit is destroyed.",
        internal_name: b"NAMED_DESTROYED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been destroyed.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 16
    TemplateText {
        ui_name: b"Unit_/ Unit exists and is alive.",
        internal_name: b"NAMED_NOT_DESTROYED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" exists and is alive.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 17
    TemplateText {
        ui_name: b"Team_/ Team completely inside an area.",
        internal_name: b"TEAM_INSIDE_AREA_ENTIRELY",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" is all inside ",
            b" (",
            b").",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::TRIGGER_AREA, P::SURFACES_ALLOWED],
    },
    // 18
    TemplateText {
        ui_name: b"Team_/ Team is completely outside an area.",
        internal_name: b"TEAM_OUTSIDE_AREA_ENTIRELY",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" is completely outside ",
            b" (",
            b").",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::TRIGGER_AREA, P::SURFACES_ALLOWED],
    },
    // 19
    TemplateText {
        ui_name: b"Unit_/ Unit is attacked by a specific unit type.",
        internal_name: b"NAMED_ATTACKED_BY_OBJECTTYPE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been attacked by a(n) ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::OBJECT_TYPE],
    },
    // 20
    TemplateText {
        ui_name: b"Team_/ Team is attacked by a specific unit type.",
        internal_name: b"TEAM_ATTACKED_BY_OBJECTTYPE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been attacked by a(n) ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::OBJECT_TYPE],
    },
    // 21
    TemplateText {
        ui_name: b"Unit_/ Unit has been attacked by a player.",
        internal_name: b"NAMED_ATTACKED_BY_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been attacked by ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::SIDE],
    },
    // 22
    TemplateText {
        ui_name: b"Team_/ Team has been attacked by a player.",
        internal_name: b"TEAM_ATTACKED_BY_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been attacked by ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::SIDE],
    },
    // 23
    TemplateText {
        ui_name: b"Player_/ Player has built an object type.",
        internal_name: b"BUILT_BY_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been built by ",
        ],
        num_parameters: 2,
        parameters: &[P::OBJECT_TYPE, P::SIDE],
    },
    // 24
    TemplateText {
        ui_name: b"Unit_/ Unit has been created.",
        internal_name: b"NAMED_CREATED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been created.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 25
    TemplateText {
        ui_name: b"Team_/ Team has been created.",
        internal_name: b"TEAM_CREATED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been created.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 26
    TemplateText {
        ui_name: b"Player_/ Player has (comparison) to a number of credits.",
        internal_name: b"PLAYER_HAS_CREDITS",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" is ",
            b" the number of credits possessed by ",
        ],
        num_parameters: 3,
        parameters: &[P::INT, P::COMPARISON, P::SIDE],
    },
    // 27
    TemplateText {
        ui_name: b"Player_/ Player has discovered a specific unit.",
        internal_name: b"NAMED_DISCOVERED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been discovered by ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::SIDE],
    },
    // 28
    TemplateText {
        ui_name: b"Player_/ Player has discovered a team.",
        internal_name: b"TEAM_DISCOVERED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been discovered by ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::SIDE],
    },
    // 29
    TemplateText {
        ui_name: b"Player_/ Player has attempted the mission a number of times.",
        internal_name: b"MISSION_ATTEMPTS",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" has attempted the mission ",
            b" ",
            b" times.",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::COMPARISON],
    },
    // 30
    TemplateText {
        ui_name: b"Player_/ Player owns the specific Unit.",
        internal_name: b"NAMED_OWNED_BY_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is owned by ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::SIDE],
    },
    // 31
    TemplateText {
        ui_name: b"Player_/ Player owns a specific team.",
        internal_name: b"TEAM_OWNED_BY_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is owned by ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::SIDE],
    },
    // 32
    TemplateText {
        ui_name: b"Player_/ Player currently owns N or fewer buildings.",
        internal_name: b"PLAYER_HAS_N_OR_FEWER_BUILDINGS",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" currently owns ",
            b" or fewer buildings.",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 33
    TemplateText {
        ui_name: b"Player_/ Player's base currently has power.",
        internal_name: b"PLAYER_HAS_POWER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" buildings are powered.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 34
    TemplateText {
        ui_name: b"Unit_/ Unit has reached the end of a specific waypoint path.",
        internal_name: b"NAMED_REACHED_WAYPOINTS_END",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has reached the end of ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::WAYPOINT_PATH],
    },
    // 35
    TemplateText {
        ui_name: b"Team_/ Team has reached the end of a specific waypoint path.",
        internal_name: b"TEAM_REACHED_WAYPOINTS_END",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has reached the end of ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::WAYPOINT_PATH],
    },
    // 36
    TemplateText {
        ui_name: b"Unit_/ Unit currently selected.",
        internal_name: b"NAMED_SELECTED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is currently selected.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 37
    TemplateText {
        ui_name: b"Unit_/ Unit enters an area.",
        internal_name: b"NAMED_ENTERED_AREA",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" enters ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::TRIGGER_AREA],
    },
    // 38
    TemplateText {
        ui_name: b"Unit_/ Unit exits an area.",
        internal_name: b"NAMED_EXITED_AREA",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" exits ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::TRIGGER_AREA],
    },
    // 39
    TemplateText {
        ui_name: b"Team_/ Team entirely enters an area.",
        internal_name: b"TEAM_ENTERED_AREA_ENTIRELY",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" all enter ",
            b" (",
            b").",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::TRIGGER_AREA, P::SURFACES_ALLOWED],
    },
    // 40
    TemplateText {
        ui_name: b"Team_/ One unit enters an area.",
        internal_name: b"TEAM_ENTERED_AREA_PARTIALLY",
        num_ui_strings: 4,
        ui_strings: &[
            b"One unit from ",
            b" enters ",
            b" (",
            b").",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::TRIGGER_AREA, P::SURFACES_ALLOWED],
    },
    // 41
    TemplateText {
        ui_name: b"Team_/ Team entirely exits an area.",
        internal_name: b"TEAM_EXITED_AREA_ENTIRELY",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" all exit ",
            b" (",
            b").",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::TRIGGER_AREA, P::SURFACES_ALLOWED],
    },
    // 42
    TemplateText {
        ui_name: b"Team_/ One unit exits an area.",
        internal_name: b"TEAM_EXITED_AREA_PARTIALLY",
        num_ui_strings: 4,
        ui_strings: &[
            b"One unit from ",
            b" exits ",
            b" (",
            b").",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::TRIGGER_AREA, P::SURFACES_ALLOWED],
    },
    // 43
    TemplateText {
        ui_name: b"Multiplayer_/ Multiplayer allied victory.",
        internal_name: b"MULTIPLAYER_ALLIED_VICTORY",
        num_ui_strings: 1,
        ui_strings: &[
            b"The multiplayer game has ended in victory for the local player and his allies.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 44
    TemplateText {
        ui_name: b"Multiplayer_/ Multiplayer allied defeat.",
        internal_name: b"MULTIPLAYER_ALLIED_DEFEAT",
        num_ui_strings: 1,
        ui_strings: &[
            b"The multiplayer game has ended in defeat for the local player and his allies.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 45
    TemplateText {
        ui_name: b"Multiplayer_/ Multiplayer local player defeat check.",
        internal_name: b"MULTIPLAYER_PLAYER_DEFEAT",
        num_ui_strings: 1,
        ui_strings: &[
            b"Everything belonging to the local player has been destroyed, but his allies may or may not have been defeated.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 46
    TemplateText {
        ui_name: b"Player_/ Player's base currently has no power.",
        internal_name: b"PLAYER_HAS_NO_POWER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" buildings are not powered.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 47
    TemplateText {
        ui_name: b"Multimedia_/ Video has completed playing.",
        internal_name: b"HAS_FINISHED_VIDEO",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has completed playing.",
        ],
        num_parameters: 1,
        parameters: &[P::MOVIE],
    },
    // 48
    TemplateText {
        ui_name: b"Multimedia_/ Speech has completed playing.",
        internal_name: b"HAS_FINISHED_SPEECH",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has completed playing.",
        ],
        num_parameters: 1,
        parameters: &[P::DIALOG],
    },
    // 49
    TemplateText {
        ui_name: b"Multimedia_/ Sound has completed playing.",
        internal_name: b"HAS_FINISHED_AUDIO",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has completed playing.",
        ],
        num_parameters: 1,
        parameters: &[P::SOUND],
    },
    // 50
    TemplateText {
        ui_name: b"Player_/ Player has entered a specific building.",
        internal_name: b"BUILDING_ENTERED_BY_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has entered building named ",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::UNIT],
    },
    // 51
    TemplateText {
        ui_name: b"Unit_/ Unit has sighted a(n) friendly/neutral/enemy unit belonging to a side.",
        internal_name: b"ENEMY_SIGHTED",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" sees a(n) ",
            b" unit belonging to ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::RELATION, P::SIDE],
    },
    // 52
    TemplateText {
        ui_name: b"Unit_/ Unit health % compared to a value.",
        internal_name: b"UNIT_HEALTH",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" Health IS ",
            b" ",
            b" percent.",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::COMPARISON],
    },
    // 53
    TemplateText {
        ui_name: b"Unit_/ Bridge is repaired.",
        internal_name: b"BRIDGE_REPAIRED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been repaired.",
        ],
        num_parameters: 1,
        parameters: &[P::BRIDGE],
    },
    // 54
    TemplateText {
        ui_name: b"Unit_/ Bridge is broken.",
        internal_name: b"BRIDGE_BROKEN",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been broken.",
        ],
        num_parameters: 1,
        parameters: &[P::BRIDGE],
    },
    // 55
    TemplateText {
        ui_name: b"Unit_/ Unit is dying.",
        internal_name: b"NAMED_DYING",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been killed, but still on screen.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 56
    TemplateText {
        ui_name: b"Unit_/ Unit is finished dying.",
        internal_name: b"NAMED_TOTALLY_DEAD",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has been killed, and is finished dying.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 57
    TemplateText {
        ui_name: b"Player_/ Player has (comparison) unit type.",
        internal_name: b"PLAYER_HAS_OBJECT_COMPARISON",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" has ",
            b" ",
            b" unit or structure of type ",
        ],
        num_parameters: 4,
        parameters: &[P::SIDE, P::COMPARISON, P::INT, P::OBJECT_TYPE],
    },
    // 58: OBSOLETE_SCRIPT_1 (not set by init)
    UNSET,
    // 59: OBSOLETE_SCRIPT_2 (not set by init)
    UNSET,
    // 60
    TemplateText {
        ui_name: b"Player_/ Player starts using a special power.",
        internal_name: b"PLAYER_TRIGGERED_SPECIAL_POWER",
        num_ui_strings: 3,
        ui_strings: &[
            b"Player ",
            b" starts using ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SPECIAL_POWER],
    },
    // 61
    TemplateText {
        ui_name: b"Player_/ Player completed using a special power.",
        internal_name: b"PLAYER_COMPLETED_SPECIAL_POWER",
        num_ui_strings: 3,
        ui_strings: &[
            b"Player ",
            b" completed using ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SPECIAL_POWER],
    },
    // 62
    TemplateText {
        ui_name: b"Player_/ Player is midway through using a special power.",
        internal_name: b"PLAYER_MIDWAY_SPECIAL_POWER",
        num_ui_strings: 3,
        ui_strings: &[
            b"Player ",
            b" is midway using ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SPECIAL_POWER],
    },
    // 63
    TemplateText {
        ui_name: b"Player_/ Player start using a special power from a named unit.",
        internal_name: b"PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED",
        num_ui_strings: 4,
        ui_strings: &[
            b"Player ",
            b" starts using ",
            b" from ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::SPECIAL_POWER, P::UNIT],
    },
    // 64
    TemplateText {
        ui_name: b"Player_/ Player completed using a special power from a named unit.",
        internal_name: b"PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED",
        num_ui_strings: 4,
        ui_strings: &[
            b"Player ",
            b" completed using ",
            b" from ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::SPECIAL_POWER, P::UNIT],
    },
    // 65
    TemplateText {
        ui_name: b"Player_/ Player is midway through using a special power from a named unit.",
        internal_name: b"PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED",
        num_ui_strings: 4,
        ui_strings: &[
            b"Player ",
            b" is midway using ",
            b" from ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::SPECIAL_POWER, P::UNIT],
    },
    // 66: DEFUNCT_PLAYER_SELECTED_GENERAL (not set by init)
    UNSET,
    // 67: DEFUNCT_PLAYER_SELECTED_GENERAL_FROM_NAMED (not set by init)
    UNSET,
    // 68
    TemplateText {
        ui_name: b"Player_/ Player built an upgrade.",
        internal_name: b"PLAYER_BUILT_UPGRADE",
        num_ui_strings: 3,
        ui_strings: &[
            b"Player ",
            b" built ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::UPGRADE],
    },
    // 69
    TemplateText {
        ui_name: b"Player_/ Player built an upgrade from a named unit.",
        internal_name: b"PLAYER_BUILT_UPGRADE_FROM_NAMED",
        num_ui_strings: 4,
        ui_strings: &[
            b"Player ",
            b" built ",
            b" from ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::UPGRADE, P::UNIT],
    },
    // 70
    TemplateText {
        ui_name: b"Player_/ Player destroyed N or more of an opponent's buildings.",
        internal_name: b"PLAYER_DESTROYED_N_BUILDINGS_PLAYER",
        num_ui_strings: 4,
        ui_strings: &[
            b"Player ",
            b" destroyed ",
            b" or more buildings owned by ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::INT, P::SIDE],
    },
    // 71: UNIT_COMPLETED_SEQUENTIAL_EXECUTION (not set by init)
    UNSET,
    // 72: TEAM_COMPLETED_SEQUENTIAL_EXECUTION (not set by init)
    UNSET,
    // 73
    TemplateText {
        ui_name: b"Player_/ Player has (comparison) unit type in an area.",
        internal_name: b"PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA",
        num_ui_strings: 5,
        ui_strings: &[
            b" ",
            b" has ",
            b" ",
            b" unit or structure of type ",
            b" in the ",
        ],
        num_parameters: 5,
        parameters: &[P::SIDE, P::COMPARISON, P::INT, P::OBJECT_TYPE, P::TRIGGER_AREA],
    },
    // 74
    TemplateText {
        ui_name: b"Player_/ Player has (comparison) kind of unit or structure in an area.",
        internal_name: b"PLAYER_HAS_COMPARISON_UNIT_KIND_IN_TRIGGER_AREA",
        num_ui_strings: 5,
        ui_strings: &[
            b" ",
            b" has ",
            b" ",
            b" unit or structure with ",
            b" in the ",
        ],
        num_parameters: 5,
        parameters: &[P::SIDE, P::COMPARISON, P::INT, P::KIND_OF_PARAM, P::TRIGGER_AREA],
    },
    // 75
    TemplateText {
        ui_name: b"Unit_/ Unit has emptied its contents.",
        internal_name: b"UNIT_EMPTIED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" emptied its contents.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 76
    TemplateText {
        ui_name: b"Unit_/ Unit has sighted a type of unit belonging to a side.",
        internal_name: b"TYPE_SIGHTED",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" sees a(n) ",
            b" belonging to ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::OBJECT_TYPE, P::SIDE],
    },
    // 77
    TemplateText {
        ui_name: b"Unit_/ A specific building is empty.",
        internal_name: b"NAMED_BUILDING_IS_EMPTY",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is empty.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 78
    TemplateText {
        ui_name: b"Player_/ Player currently owns N or fewer faction buildings.",
        internal_name: b"PLAYER_HAS_N_OR_FEWER_FACTION_BUILDINGS",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" currently owns ",
            b" or fewer faction buildings.",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 79
    TemplateText {
        ui_name: b"Unit_/ Unit has object status.",
        internal_name: b"UNIT_HAS_OBJECT_STATUS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::OBJECT_STATUS],
    },
    // 80
    TemplateText {
        ui_name: b"Team_/ Team has object status - all.",
        internal_name: b"TEAM_ALL_HAS_OBJECT_STATUS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::OBJECT_STATUS],
    },
    // 81
    TemplateText {
        ui_name: b"Team_/ Team has object status - partial.",
        internal_name: b"TEAM_SOME_HAVE_OBJECT_STATUS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::OBJECT_STATUS],
    },
    // 82
    TemplateText {
        ui_name: b"Player_/ Player has (comparison) percent power supply to consumption.",
        internal_name: b"PLAYER_POWER_COMPARE_PERCENT",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" has ",
            b" ",
            b" percent power supply ratio.",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::COMPARISON],
    },
    // 83
    TemplateText {
        ui_name: b"Player_/ Player has (comparison) kilowatts excess power supply.",
        internal_name: b"PLAYER_EXCESS_POWER_COMPARE_VALUE",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" has ",
            b" ",
            b" excess kilowatts power supply.",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::COMPARISON],
    },
    // 84
    TemplateText {
        ui_name: b"Skirmish_/ Player's special power is ready to fire.",
        internal_name: b"SKIRMISH_SPECIAL_POWER_READY",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" is ready to fire ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SPECIAL_POWER],
    },
    // 85
    TemplateText {
        ui_name: b"Skirmish Only_/ Player has total value in area.",
        internal_name: b"SKIRMISH_VALUE_IN_AREA",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" has ",
            b" ",
            b" within area ",
        ],
        num_parameters: 4,
        parameters: &[P::SIDE, P::COMPARISON, P::INT, P::TRIGGER_AREA],
    },
    // 86
    TemplateText {
        ui_name: b"Skirmish_/ Player is faction. - untested",
        internal_name: b"SKIRMISH_PLAYER_FACTION",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is ",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::FACTION_NAME],
    },
    // 87
    TemplateText {
        ui_name: b"Skirmish Only_/ Supplies are within specified distance.",
        internal_name: b"SKIRMISH_SUPPLIES_VALUE_WITHIN_DISTANCE",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" has supplies within ",
            b" of ",
            b" worth at least ",
        ],
        num_parameters: 4,
        parameters: &[P::SIDE, P::REAL, P::TRIGGER_AREA, P::REAL],
    },
    // 88
    TemplateText {
        ui_name: b"Skirmish Only_/ Tech building is within specified distance.",
        internal_name: b"SKIRMISH_TECH_BUILDING_WITHIN_DISTANCE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" has a tech building within ",
            b" of ",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::REAL, P::TRIGGER_AREA],
    },
    // 89
    TemplateText {
        ui_name: b"Skirmish_/ Command Ability is ready - all.",
        internal_name: b"SKIRMISH_COMMAND_BUTTON_READY_ALL",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b"'s ",
            b" are ready to use ",
            b" (all applicable members).",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 90
    TemplateText {
        ui_name: b"Skirmish_/ Command Ability is ready - partial",
        internal_name: b"SKIRMISH_COMMAND_BUTTON_READY_PARTIAL",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b"'s ",
            b" are ready to use ",
            b" (at least one member).",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 91
    TemplateText {
        ui_name: b"Skirmish_/ Unowned faction unit -- comparison.",
        internal_name: b"SKIRMISH_UNOWNED_FACTION_UNIT_EXISTS",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b". There are ",
            b" ",
            b" unowned faction units.",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::COMPARISON],
    },
    // 92
    TemplateText {
        ui_name: b"Skirmish_/ Player has prerequisites to build an object type.",
        internal_name: b"SKIRMISH_PLAYER_HAS_PREREQUISITE_TO_BUILD",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" can build ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::OBJECT_TYPE],
    },
    // 93
    TemplateText {
        ui_name: b"Skirmish_/ Player has garrisoned buildings -- comparison.",
        internal_name: b"SKIRMISH_PLAYER_HAS_COMPARISON_GARRISONED",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" has ",
            b" ",
            b" garrisoned buildings.",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::COMPARISON],
    },
    // 94
    TemplateText {
        ui_name: b"Skirmish_/ Player has captured units -- comparison",
        internal_name: b"SKIRMISH_PLAYER_HAS_COMPARISON_CAPTURED_UNITS",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" has captured ",
            b" ",
            b" units.",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::COMPARISON],
    },
    // 95
    TemplateText {
        ui_name: b"Skirmish_/ Area exists.",
        internal_name: b"SKIRMISH_NAMED_AREA_EXIST",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b". ",
            b" exists.",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::TRIGGER_AREA],
    },
    // 96
    TemplateText {
        ui_name: b"Skirmish_/ Player has units in an area",
        internal_name: b"SKIRMISH_PLAYER_HAS_UNITS_IN_AREA",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" has units in ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::TRIGGER_AREA],
    },
    // 97
    TemplateText {
        ui_name: b"Skirmish_/ Player has been attacked by player.",
        internal_name: b"SKIRMISH_PLAYER_HAS_BEEN_ATTACKED_BY_PLAYER",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" has been attacked by ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SIDE],
    },
    // 98
    TemplateText {
        ui_name: b"Skirmish_/ Player doesn't have units in an area.",
        internal_name: b"SKIRMISH_PLAYER_IS_OUTSIDE_AREA",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" has doesn't have units in ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::TRIGGER_AREA],
    },
    // 99
    TemplateText {
        ui_name: b"Skirmish_/ Player has discovered another player.",
        internal_name: b"SKIRMISH_PLAYER_HAS_DISCOVERED_PLAYER",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" has discovered ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SIDE],
    },
    // 100
    TemplateText {
        ui_name: b"Player_/ Player acquired a Science.",
        internal_name: b"PLAYER_ACQUIRED_SCIENCE",
        num_ui_strings: 3,
        ui_strings: &[
            b"Player ",
            b" acquired ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SCIENCE],
    },
    // 101
    TemplateText {
        ui_name: b"Player_/ Player has a certain number of Science Purchase Points available.",
        internal_name: b"PLAYER_HAS_SCIENCEPURCHASEPOINTS",
        num_ui_strings: 3,
        ui_strings: &[
            b"Player ",
            b" has at least ",
            b" Science Purchase Points available.",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 102
    TemplateText {
        ui_name: b"Player_/ Player can purchase a particular Science (has all prereqs & points).",
        internal_name: b"PLAYER_CAN_PURCHASE_SCIENCE",
        num_ui_strings: 3,
        ui_strings: &[
            b"Player ",
            b" can purchase ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SCIENCE],
    },
    // 103
    TemplateText {
        ui_name: b"Multimedia_/ Music track has completed some number of times.",
        internal_name: b"MUSIC_TRACK_HAS_COMPLETED",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" has completed at least ",
            b" times. (NOTE: This can only be used to start other music. USING THIS SCRIPT IN ANY OTHER WAY WILL CAUSE REPLAYS TO NOT WORK.)",
        ],
        num_parameters: 2,
        parameters: &[P::MUSIC],
    },
    // 104
    TemplateText {
        ui_name: b"Player_/ Player has lost an object of type.",
        internal_name: b"PLAYER_LOST_OBJECT_TYPE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" has lost an object of type ",
            b" (can be an object type list).",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::OBJECT_TYPE],
    },
    // 105
    TemplateText {
        ui_name: b"Skirmish_/ Supply source is safe.",
        internal_name: b"SUPPLY_SOURCE_SAFE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" closest supply src with at least ",
            b" available resources is SAFE from enemy influence.",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 106
    TemplateText {
        ui_name: b"Skirmish_/ Supply source is attacked.",
        internal_name: b"SUPPLY_SOURCE_ATTACKED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" supply source is under attack.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 107
    TemplateText {
        ui_name: b"Skirmish_/ Start position.",
        internal_name: b"START_POSITION_IS",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" starting position is ",
            b" .",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 108
    TemplateText {
        ui_name: b"Unit_/ Unit has free container slots.",
        internal_name: b"NAMED_HAS_FREE_CONTAINER_SLOTS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" has free container slots.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
];

/// `m_actionTemplates` after `init`, by `ScriptActionType`.
pub static ACTION_TEMPLATES: [TemplateText; 344] = [
    // 0
    TemplateText {
        ui_name: b"Scripting_/Debug/Display message and pause",
        internal_name: b"DEBUG_MESSAGE_BOX",
        num_ui_strings: 1,
        ui_strings: &[
            b"Show debug string and pause: ",
        ],
        num_parameters: 1,
        parameters: &[P::TEXT_STRING],
    },
    // 1
    TemplateText {
        ui_name: b"Scripting_/Flags/Set flag to value",
        internal_name: b"SET_FLAG",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set ",
            b" to ",
        ],
        num_parameters: 2,
        parameters: &[P::FLAG, P::BOOLEAN],
    },
    // 2
    TemplateText {
        ui_name: b"Scripting_/Counters/Set counter to a value",
        internal_name: b"SET_COUNTER",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set ",
            b" to ",
        ],
        num_parameters: 2,
        parameters: &[P::COUNTER],
    },
    // 3
    TemplateText {
        ui_name: b"Multiplayer_/ Announce victory.",
        internal_name: b"VICTORY",
        num_ui_strings: 1,
        ui_strings: &[
            b"Show 'Victorious' window and end game",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 4
    TemplateText {
        ui_name: b"Multiplayer_/ Announce defeat.",
        internal_name: b"DEFEAT",
        num_ui_strings: 1,
        ui_strings: &[
            b"Show 'Defeated' window and end game",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 5
    TemplateText {
        ui_name: b"Scripting_/Debug/Null operation.",
        internal_name: b"NO_OP",
        num_ui_strings: 1,
        ui_strings: &[
            b"Null operation. (Does nothing.)",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 6
    TemplateText {
        ui_name: b"Scripting_/Timer/Frame countdown timer -- set.",
        internal_name: b"SET_TIMER",
        num_ui_strings: 3,
        ui_strings: &[
            b"Set timer ",
            b" to expire in ",
            b" frames.",
        ],
        num_parameters: 2,
        parameters: &[P::COUNTER],
    },
    // 7
    TemplateText {
        ui_name: b"Multimedia_/Sound Effect/Play sound effect.",
        internal_name: b"PLAY_SOUND_EFFECT",
        num_ui_strings: 2,
        ui_strings: &[
            b"Play ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::SOUND],
    },
    // 8
    TemplateText {
        ui_name: b"Scripting_/Script/Enable Script.",
        internal_name: b"ENABLE_SCRIPT",
        num_ui_strings: 2,
        ui_strings: &[
            b"Enable ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::SCRIPT],
    },
    // 9
    TemplateText {
        ui_name: b"Scripting_/Script/Disable script.",
        internal_name: b"DISABLE_SCRIPT",
        num_ui_strings: 2,
        ui_strings: &[
            b"Disable ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::SCRIPT],
    },
    // 10
    TemplateText {
        ui_name: b"Scripting_/Script/Run subroutine script.",
        internal_name: b"CALL_SUBROUTINE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Run ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::SCRIPT_SUBROUTINE],
    },
    // 11
    TemplateText {
        ui_name: b"Multimedia_/Sound Effect/Play sound effect at waypoint.",
        internal_name: b"PLAY_SOUND_EFFECT_AT",
        num_ui_strings: 3,
        ui_strings: &[
            b"Play ",
            b" at ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SOUND, P::WAYPOINT],
    },
    // 12
    TemplateText {
        ui_name: b"Team_/Damage/Damage the members of a team.",
        internal_name: b"DAMAGE_MEMBERS_OF_TEAM",
        num_ui_strings: 3,
        ui_strings: &[
            b"Damage ",
            b", amount=",
            b" (-1==kill).",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::REAL],
    },
    // 13
    TemplateText {
        ui_name: b"Team_/Move/Set to move to a location.",
        internal_name: b"MOVE_TEAM_TO",
        num_ui_strings: 3,
        ui_strings: &[
            b"Move ",
            b" to ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::WAYPOINT],
    },
    // 14
    TemplateText {
        ui_name: b"Camera_/Move/Move the camera to a location.",
        internal_name: b"MOVE_CAMERA_TO",
        num_ui_strings: 6,
        ui_strings: &[
            b"Move camera to ",
            b" in ",
            b" seconds, camera shutter ",
            b" seconds, ease-in ",
            b" seconds, ease-out ",
            b" seconds.",
        ],
        num_parameters: 5,
        parameters: &[P::WAYPOINT, P::REAL, P::REAL, P::REAL, P::REAL],
    },
    // 15
    TemplateText {
        ui_name: b"Scripting_/Counters/Increment counter.",
        internal_name: b"INCREMENT_COUNTER",
        num_ui_strings: 2,
        ui_strings: &[
            b"Add ",
            b" to counter ",
        ],
        num_parameters: 2,
        parameters: &[P::INT, P::COUNTER],
    },
    // 16
    TemplateText {
        ui_name: b"Scripting_/Counters/Decrement counter.",
        internal_name: b"DECREMENT_COUNTER",
        num_ui_strings: 2,
        ui_strings: &[
            b"Subtract ",
            b" from counter ",
        ],
        num_parameters: 2,
        parameters: &[P::INT, P::COUNTER],
    },
    // 17
    TemplateText {
        ui_name: b"Camera_/Move/Move along a waypoint path.",
        internal_name: b"MOVE_CAMERA_ALONG_WAYPOINT_PATH",
        num_ui_strings: 6,
        ui_strings: &[
            b"Move along path starting with ",
            b" in ",
            b" seconds, camera shutter ",
            b" seconds, ease-in ",
            b" seconds, ease-out ",
            b" seconds.",
        ],
        num_parameters: 5,
        parameters: &[P::WAYPOINT, P::REAL, P::REAL, P::REAL, P::REAL],
    },
    // 18
    TemplateText {
        ui_name: b"Camera_/Rotate/ Rotate around the current viewpoint.",
        internal_name: b"ROTATE_CAMERA",
        num_ui_strings: 5,
        ui_strings: &[
            b"Rotate ",
            b" times, taking ",
            b" seconds, ease-in ",
            b" seconds, ease-out ",
            b" seconds.",
        ],
        num_parameters: 4,
        parameters: &[P::REAL, P::REAL, P::REAL, P::REAL],
    },
    // 19
    TemplateText {
        ui_name: b"Camera_/Move/ Reset to the default view.",
        internal_name: b"RESET_CAMERA",
        num_ui_strings: 5,
        ui_strings: &[
            b"Reset to ",
            b", taking ",
            b" seconds, ease-in ",
            b" seconds, ease-out ",
            b" seconds.",
        ],
        num_parameters: 4,
        parameters: &[P::WAYPOINT, P::REAL, P::REAL, P::REAL],
    },
    // 20
    TemplateText {
        ui_name: b"Scripting_/Timer/Seconds countdown timer -- set.",
        internal_name: b"SET_MILLISECOND_TIMER",
        num_ui_strings: 3,
        ui_strings: &[
            b"Set timer ",
            b" to expire in ",
            b" seconds.",
        ],
        num_parameters: 2,
        parameters: &[P::COUNTER, P::REAL],
    },
    // 21
    TemplateText {
        ui_name: b"Camera_/Move/Modify/ Freeze time during the camera movement.",
        internal_name: b"CAMERA_MOD_FREEZE_TIME",
        num_ui_strings: 1,
        ui_strings: &[
            b"Freeze time during the camera movement.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 22
    TemplateText {
        ui_name: b"{Compatibility}_/Multimedia/Modify visual game time.",
        internal_name: b"SET_VISUAL_SPEED_MULTIPLIER",
        num_ui_strings: 2,
        ui_strings: &[
            b"Make visual time ",
            b" time normal (1=normal, 2 = twice as fast, ...).",
        ],
        num_parameters: 1,
        parameters: &[],
    },
    // 23
    TemplateText {
        ui_name: b"Unit_/Spawn/Spawn object.",
        internal_name: b"CREATE_OBJECT",
        num_ui_strings: 5,
        ui_strings: &[
            b"Spawn object ",
            b" in ",
            b" at position (",
            b"), rotated ",
            b" .",
        ],
        num_parameters: 4,
        parameters: &[P::OBJECT_TYPE, P::TEAM, P::COORD3D, P::ANGLE],
    },
    // 24
    TemplateText {
        ui_name: b"Multimedia_/All Sounds/Suspend all sounds.",
        internal_name: b"SUSPEND_BACKGROUND_SOUNDS",
        num_ui_strings: 1,
        ui_strings: &[
            b"Suspend background sounds.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 25
    TemplateText {
        ui_name: b"Multimedia_/All Sounds/Resume all sounds.",
        internal_name: b"RESUME_BACKGROUND_SOUNDS",
        num_ui_strings: 1,
        ui_strings: &[
            b"Resume background sounds.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 26
    TemplateText {
        ui_name: b"Camera_/Move/Modify/Set Final zoom for camera movement.",
        internal_name: b"CAMERA_MOD_SET_FINAL_ZOOM",
        num_ui_strings: 4,
        ui_strings: &[
            b"Adjust zoom to ",
            b" (1.0==max height, 0.0==in the ground) ",
            b" ease-in ",
            b" ease-out.",
        ],
        num_parameters: 3,
        parameters: &[P::REAL, P::PERCENT, P::PERCENT],
    },
    // 27
    TemplateText {
        ui_name: b"Camera_/Move/Modify/Set Final pitch for camera movement.",
        internal_name: b"CAMERA_MOD_SET_FINAL_PITCH",
        num_ui_strings: 4,
        ui_strings: &[
            b"Adjust pitch to ",
            b" (1.0==default, 0.0==toward horizon, >1 = toward ground) ",
            b" ease-in ",
            b" ease-out.",
        ],
        num_parameters: 3,
        parameters: &[P::REAL, P::PERCENT, P::PERCENT],
    },
    // 28
    TemplateText {
        ui_name: b"Camera_/Move/Modify/ Freeze camera angle during the camera movement.",
        internal_name: b"CAMERA_MOD_FREEZE_ANGLE",
        num_ui_strings: 1,
        ui_strings: &[
            b"Freeze camera angle during the camera movement.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 29
    TemplateText {
        ui_name: b"{Compatibility}_/Camera/Modify/Final visual game time for camera movement.",
        internal_name: b"CAMERA_MOD_SET_FINAL_SPEED_MULTIPLIER",
        num_ui_strings: 2,
        ui_strings: &[
            b"Adjust game time to",
            b" times normal (1=normal, 2 = twice as fast, ...).",
        ],
        num_parameters: 1,
        parameters: &[],
    },
    // 30
    TemplateText {
        ui_name: b"Camera_/Move/Modify/ Number of frames to average movements.",
        internal_name: b"CAMERA_MOD_SET_ROLLING_AVERAGE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Average position and angle changes over",
            b" frames. (1=no smoothing, 5 = very smooth)",
        ],
        num_parameters: 1,
        parameters: &[],
    },
    // 31
    TemplateText {
        ui_name: b"{Compatibility}_/Camera/Modify/Move/ Final camera look toward point.",
        internal_name: b"CAMERA_MOD_FINAL_LOOK_TOWARD",
        num_ui_strings: 2,
        ui_strings: &[
            b"Look toward",
            b" at the end of the camera movement.",
        ],
        num_parameters: 1,
        parameters: &[P::WAYPOINT],
    },
    // 32
    TemplateText {
        ui_name: b"Camera_/Modify/Move/Camera look toward point while moving.",
        internal_name: b"CAMERA_MOD_LOOK_TOWARD",
        num_ui_strings: 2,
        ui_strings: &[
            b"Look toward",
            b" during the camera movement.",
        ],
        num_parameters: 1,
        parameters: &[P::WAYPOINT],
    },
    // 33
    TemplateText {
        ui_name: b"Team_/Attack/Set to attack -- another team.",
        internal_name: b"TEAM_ATTACK_TEAM",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begin attack on ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::TEAM],
    },
    // 34
    TemplateText {
        ui_name: b"Team_/ Spawn a reinforcement team.",
        internal_name: b"CREATE_REINFORCEMENT_TEAM",
        num_ui_strings: 3,
        ui_strings: &[
            b"Spawn an instance of ",
            b" at ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::WAYPOINT],
    },
    // 35
    TemplateText {
        ui_name: b"Camera_/Move/Modify/ End movement at selected unit.",
        internal_name: b"MOVE_CAMERA_TO_SELECTION",
        num_ui_strings: 1,
        ui_strings: &[
            b"End movement at selected unit.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 36
    TemplateText {
        ui_name: b"Team_/Move/Set to follow a waypoint path.",
        internal_name: b"TEAM_FOLLOW_WAYPOINTS",
        num_ui_strings: 3,
        ui_strings: &[
            b"Have ",
            b" follow ",
            b" , as a team is ",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::WAYPOINT_PATH, P::BOOLEAN],
    },
    // 37
    TemplateText {
        ui_name: b"Team_/Misc/Team custom state - set state.",
        internal_name: b"TEAM_SET_STATE",
        num_ui_strings: 3,
        ui_strings: &[
            b"Set ",
            b" to ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::TEAM_STATE],
    },
    // 38
    TemplateText {
        ui_name: b"Unit_/Move/Move a specific unit to a location.",
        internal_name: b"MOVE_NAMED_UNIT_TO",
        num_ui_strings: 3,
        ui_strings: &[
            b"Move ",
            b" to ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::WAYPOINT],
    },
    // 39
    TemplateText {
        ui_name: b"Unit_/Attack/Set unit to attack another unit.",
        internal_name: b"NAMED_ATTACK_NAMED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begin attack on ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::UNIT],
    },
    // 40
    TemplateText {
        ui_name: b"Unit_/Spawn/Spawn -- named unit on a team at a waypoint.",
        internal_name: b"CREATE_NAMED_ON_TEAM_AT_WAYPOINT",
        num_ui_strings: 4,
        ui_strings: &[
            b"Spawn ",
            b" of type ",
            b" on ",
            b" at waypoint ",
        ],
        num_parameters: 4,
        parameters: &[P::UNIT, P::OBJECT_TYPE, P::TEAM, P::WAYPOINT],
    },
    // 41
    TemplateText {
        ui_name: b"Unit_/Spawn/Spawn -- unnamed unit on a team at a waypoint.",
        internal_name: b"CREATE_UNNAMED_ON_TEAM_AT_WAYPOINT",
        num_ui_strings: 3,
        ui_strings: &[
            b"Spawn unit of type ",
            b" on ",
            b" at waypoint ",
        ],
        num_parameters: 3,
        parameters: &[P::OBJECT_TYPE, P::TEAM, P::WAYPOINT],
    },
    // 42
    TemplateText {
        ui_name: b"AttackPrioritySet_/Apply/Unit/Apply unit's attack priority set.",
        internal_name: b"NAMED_APPLY_ATTACK_PRIORITY_SET",
        num_ui_strings: 3,
        ui_strings: &[
            b"Have ",
            b" use ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::ATTACK_PRIORITY_SET],
    },
    // 43
    TemplateText {
        ui_name: b"AttackPrioritySet_/Apply/Team/Apply a team's attack priority set.",
        internal_name: b"TEAM_APPLY_ATTACK_PRIORITY_SET",
        num_ui_strings: 3,
        ui_strings: &[
            b"Have ",
            b" use ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::ATTACK_PRIORITY_SET],
    },
    // 44
    TemplateText {
        ui_name: b"Player_/AI/Set the delay between building teams.",
        internal_name: b"SET_BASE_CONSTRUCTION_SPEED",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" seconds between building teams.",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 45
    TemplateText {
        ui_name: b"Unit_/Mood/Set the general attitude of a specific unit.",
        internal_name: b"NAMED_SET_ATTITUDE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" changes his attitude to ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::AI_MOOD],
    },
    // 46
    TemplateText {
        ui_name: b"Team_/Mood/Set the general attitude of a team.",
        internal_name: b"TEAM_SET_ATTITUDE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" change their attitude to ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::AI_MOOD],
    },
    // 47
    TemplateText {
        ui_name: b"Unit_/Attack/Set a specific unit to attack a specific trigger area.",
        internal_name: b"NAMED_ATTACK_AREA",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" attacks anything in ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::TRIGGER_AREA],
    },
    // 48
    TemplateText {
        ui_name: b"Unit_/Attack/Set a specific unit to attack a team.",
        internal_name: b"NAMED_ATTACK_TEAM",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" attacks ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::TEAM],
    },
    // 49
    TemplateText {
        ui_name: b"Team_/Attack/Set to attack -- trigger area.",
        internal_name: b"TEAM_ATTACK_AREA",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" attack anything in ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::TRIGGER_AREA],
    },
    // 50
    TemplateText {
        ui_name: b"Team_/Attack/Set to attack -- specific unit.",
        internal_name: b"TEAM_ATTACK_NAMED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" attacks ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::UNIT],
    },
    // 51
    TemplateText {
        ui_name: b"Team_/Transport/Transport -- automatically load.",
        internal_name: b"TEAM_LOAD_TRANSPORTS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" load into transports.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 52
    TemplateText {
        ui_name: b"Unit_/Transport/Transport -- load unit into specific.",
        internal_name: b"NAMED_ENTER_NAMED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" loads into ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::UNIT],
    },
    // 53
    TemplateText {
        ui_name: b"Team_/Transport/Transport -- load team into specific.",
        internal_name: b"TEAM_ENTER_NAMED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" attempt to load into ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::UNIT],
    },
    // 54
    TemplateText {
        ui_name: b"Unit_/Transport/Transport -- unload units from specific.",
        internal_name: b"NAMED_EXIT_ALL",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" unloads.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 55
    TemplateText {
        ui_name: b"Team_/Transport/Transport -- unload team from all.",
        internal_name: b"TEAM_EXIT_ALL",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" unload.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 56
    TemplateText {
        ui_name: b"Unit_/Move/Set a specific unit to follow a waypoint path.",
        internal_name: b"NAMED_FOLLOW_WAYPOINTS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" follows waypoints, beginning at ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::WAYPOINT_PATH],
    },
    // 57
    TemplateText {
        ui_name: b"Unit_/Move/Set to guard.",
        internal_name: b"NAMED_GUARD",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begins guarding.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 58
    TemplateText {
        ui_name: b"Team_/Guard/Set to guard -- current location.",
        internal_name: b"TEAM_GUARD",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begins guarding.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 59
    TemplateText {
        ui_name: b"Unit_/Hunt/Set a specific unit to hunt.",
        internal_name: b"NAMED_HUNT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begins hunting.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 60
    TemplateText {
        ui_name: b"Team_/Hunt/Set to hunt.",
        internal_name: b"TEAM_HUNT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begins hunting.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 61
    TemplateText {
        ui_name: b"Player_/Set/Set a player to sell everything.",
        internal_name: b"PLAYER_SELL_EVERYTHING",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" sells everything.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 62
    TemplateText {
        ui_name: b"Player_/Build/Set a player to be unable to build buildings.",
        internal_name: b"PLAYER_DISABLE_BASE_CONSTRUCTION",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is unable to build buildings.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 63
    TemplateText {
        ui_name: b"Player_/Build/Set a player to be unable to build from a specific building.",
        internal_name: b"PLAYER_DISABLE_FACTORIES",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is unable to build from ",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::OBJECT_TYPE],
    },
    // 64
    TemplateText {
        ui_name: b"Player_/Build/Set a player to be unable to build units.",
        internal_name: b"PLAYER_DISABLE_UNIT_CONSTRUCTION",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is unable to build units.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 65
    TemplateText {
        ui_name: b"Player_/Build/Set a player to be able to build buildings.",
        internal_name: b"PLAYER_ENABLE_BASE_CONSTRUCTION",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is able to build buildings.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 66
    TemplateText {
        ui_name: b"Player_/Build/Set a player to be able to build from a specific building.",
        internal_name: b"PLAYER_ENABLE_FACTORIES",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is able to build from ",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::OBJECT_TYPE],
    },
    // 67
    TemplateText {
        ui_name: b"Player_/Build/Set a player to be able to build units.",
        internal_name: b"PLAYER_ENABLE_UNIT_CONSTRUCTION",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is able to build units.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 68
    TemplateText {
        ui_name: b"Camera_/Move/Move the camera to the home position.",
        internal_name: b"CAMERA_MOVE_HOME",
        num_ui_strings: 1,
        ui_strings: &[
            b"The camera moves to the home base.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 69
    TemplateText {
        ui_name: b"Team_/AI/Start building a team.",
        internal_name: b"BUILD_TEAM",
        num_ui_strings: 1,
        ui_strings: &[
            b"Start building team ",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 70
    TemplateText {
        ui_name: b"Unit_/Damage/Deal damage to a specific unit.",
        internal_name: b"NAMED_DAMAGE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" takes ",
            b" points of damage.",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT],
    },
    // 71
    TemplateText {
        ui_name: b"Unit_/Damage or Remove/Delete a specific unit.",
        internal_name: b"NAMED_DELETE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is removed from the world.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 72
    TemplateText {
        ui_name: b"Team_/Damage or Remove/Delete a team.",
        internal_name: b"TEAM_DELETE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is removed from the world.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 73
    TemplateText {
        ui_name: b"Unit_/Damage or Remove/Kill a specific unit.",
        internal_name: b"NAMED_KILL",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b"is dealt a lethal amount of damage.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 74
    TemplateText {
        ui_name: b"Team_/Damage or Remove/Kill an entire team.",
        internal_name: b"TEAM_KILL",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is dealt a lethal amount of damage.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 75
    TemplateText {
        ui_name: b"Player_/Damage or Remove/Kill a player.",
        internal_name: b"PLAYER_KILL",
        num_ui_strings: 2,
        ui_strings: &[
            b"All of ",
            b"'s buildings and units are dealt a lethal amount of damage.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 76
    TemplateText {
        ui_name: b"User_/String/Display a string.",
        internal_name: b"DISPLAY_TEXT",
        num_ui_strings: 2,
        ui_strings: &[
            b"Displays ",
            b" in the text log and message area.",
        ],
        num_parameters: 1,
        parameters: &[P::LOCALIZED_TEXT],
    },
    // 77
    TemplateText {
        ui_name: b"User_/Flash/Flash a cameo for a specified amount of time.",
        internal_name: b"CAMEO_FLASH",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" flashes for ",
            b" seconds.",
        ],
        num_parameters: 2,
        parameters: &[P::COMMAND_BUTTON],
    },
    // 78
    TemplateText {
        ui_name: b"User_/Flash/Flash a specific unit for a specified amount of time.",
        internal_name: b"NAMED_FLASH",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" flashes for ",
            b" seconds.",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT],
    },
    // 79
    TemplateText {
        ui_name: b"User_/Flash/Flash a team for a specified amount of time.",
        internal_name: b"TEAM_FLASH",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" flashes for ",
            b" seconds.",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM],
    },
    // 80
    TemplateText {
        ui_name: b"Multimedia_/Movie/Play a movie in fullscreen mode.",
        internal_name: b"MOVIE_PLAY_FULLSCREEN",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" plays fullscreen.",
        ],
        num_parameters: 1,
        parameters: &[P::MOVIE],
    },
    // 81
    TemplateText {
        ui_name: b"Multimedia_/Movie/Play a movie in the radar.",
        internal_name: b"MOVIE_PLAY_RADAR",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" plays in the radar window.",
        ],
        num_parameters: 1,
        parameters: &[P::MOVIE],
    },
    // 82
    TemplateText {
        ui_name: b"Multimedia_/Sound Effects/Play a sound as though coming from a specific unit.",
        internal_name: b"SOUND_PLAY_NAMED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" plays as though coming from ",
        ],
        num_parameters: 2,
        parameters: &[P::TEXT_STRING, P::UNIT],
    },
    // 83
    TemplateText {
        ui_name: b"Multimedia_/Sound Effects/Play a speech file.",
        internal_name: b"SPEECH_PLAY",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" plays, allowing overlap ",
            b" (true to allow, false to disallow).",
        ],
        num_parameters: 2,
        parameters: &[P::DIALOG, P::BOOLEAN],
    },
    // 84
    TemplateText {
        ui_name: b"Player_/Transfer/Transfer assets from one player to another player.",
        internal_name: b"PLAYER_TRANSFER_OWNERSHIP_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b"All assets of ",
            b" are transferred to ",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SIDE],
    },
    // 85
    TemplateText {
        ui_name: b"Player_/Transfer/Transfer a specific unit to the control of a player.",
        internal_name: b"NAMED_TRANSFER_OWNERSHIP_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is transferred to the command of ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::SIDE],
    },
    // 86
    TemplateText {
        ui_name: b"Player_/Alliances/Change how a player relates to another player.",
        internal_name: b"PLAYER_RELATES_PLAYER",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" considers ",
            b" to be ",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::SIDE, P::RELATION],
    },
    // 87
    TemplateText {
        ui_name: b"Radar_/Create Event/Create a radar event at a specified location.",
        internal_name: b"RADAR_CREATE_EVENT",
        num_ui_strings: 2,
        ui_strings: &[
            b"A radar event occurs at ",
            b" of type ",
        ],
        num_parameters: 2,
        parameters: &[P::COORD3D, P::RADAR_EVENT_TYPE],
    },
    // 88
    TemplateText {
        ui_name: b"Radar_/Control/Disable the radar.",
        internal_name: b"RADAR_DISABLE",
        num_ui_strings: 1,
        ui_strings: &[
            b"The radar is disabled.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 89
    TemplateText {
        ui_name: b"Radar_/Control/Enable the radar.",
        internal_name: b"RADAR_ENABLE",
        num_ui_strings: 1,
        ui_strings: &[
            b"The radar is enabled.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 90
    TemplateText {
        ui_name: b"Map_/Shroud or Reveal/Reveal map at waypoint -- fog.",
        internal_name: b"MAP_REVEAL_AT_WAYPOINT",
        num_ui_strings: 4,
        ui_strings: &[
            b"The map is revealed at ",
            b" with a radius of ",
            b" feet for ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::WAYPOINT, P::REAL, P::SIDE],
    },
    // 91
    TemplateText {
        ui_name: b"Team_/AI/Set whether members of a team can be recruited into another team.",
        internal_name: b"TEAM_AVAILABLE_FOR_RECRUITMENT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" sets their willingness to join teams to ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::BOOLEAN],
    },
    // 92
    TemplateText {
        ui_name: b"Team_/AI/Set to collect nearby units.",
        internal_name: b"TEAM_COLLECT_NEARBY_FOR_TEAM",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" attempts to collect nearby units for a team.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 93
    TemplateText {
        ui_name: b"Team_/Merge/Merge a team into another team.",
        internal_name: b"TEAM_MERGE_INTO_TEAM",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" merges onto ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::TEAM],
    },
    // 94
    TemplateText {
        ui_name: b"User_/Input/User input -- disable.",
        internal_name: b"DISABLE_INPUT",
        num_ui_strings: 1,
        ui_strings: &[
            b"Disable mouse and keyboard input.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 95
    TemplateText {
        ui_name: b"User_/Input/User input -- enable.",
        internal_name: b"ENABLE_INPUT",
        num_ui_strings: 1,
        ui_strings: &[
            b"Enable mouse and keyboard input.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 96
    TemplateText {
        ui_name: b"Player_/Hunt/Set all of a player's units to hunt.",
        internal_name: b"PLAYER_HUNT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begins hunting.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 97
    TemplateText {
        ui_name: b"Multimedia_/SoundEffects/Pause the ambient sounds.",
        internal_name: b"SOUND_AMBIENT_PAUSE",
        num_ui_strings: 1,
        ui_strings: &[
            b"Pause the ambient sounds.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 98
    TemplateText {
        ui_name: b"Multimedia_/SoundEffects/Resume the ambient sounds.",
        internal_name: b"SOUND_AMBIENT_RESUME",
        num_ui_strings: 1,
        ui_strings: &[
            b"Resume the ambient sounds.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 99
    TemplateText {
        ui_name: b"Multimedia_/Music/Play a music track.",
        internal_name: b"MUSIC_SET_TRACK",
        num_ui_strings: 4,
        ui_strings: &[
            b"Play ",
            b" using fadeout (",
            b") and fadein (",
            b").",
        ],
        num_parameters: 3,
        parameters: &[P::MUSIC, P::BOOLEAN, P::BOOLEAN],
    },
    // 100
    TemplateText {
        ui_name: b"Map_/Environment/Set wind sway amount and direction.",
        internal_name: b"SET_TREE_SWAY",
        num_ui_strings: 6,
        ui_strings: &[
            b"Set wind direction to ",
            b", amount to sway ",
            b", amount to lean with the wind ",
            b", frames to take to sway once ",
            b", randomness ",
            b"(0=lock step, 1=large random variation).",
        ],
        num_parameters: 5,
        parameters: &[P::ANGLE, P::ANGLE, P::ANGLE, P::INT, P::REAL],
    },
    // 101
    TemplateText {
        ui_name: b"Scripting_/Debug/Display string",
        internal_name: b"DEBUG_STRING",
        num_ui_strings: 1,
        ui_strings: &[
            b"Show debug string without pausing: ",
        ],
        num_parameters: 1,
        parameters: &[P::TEXT_STRING],
    },
    // 102
    TemplateText {
        ui_name: b"Map_/Shroud or Reveal/Reveal the entire map for a player.",
        internal_name: b"MAP_REVEAL_ALL",
        num_ui_strings: 2,
        ui_strings: &[
            b"The world is revealed for ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 103
    TemplateText {
        ui_name: b"Team_/Garrison/Garrison a specific building with a team.",
        internal_name: b"TEAM_GARRISON_SPECIFIC_BUILDING",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" enters into building named ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::UNIT],
    },
    // 104
    TemplateText {
        ui_name: b"Unit_/Garrison/Empty a specific building.",
        internal_name: b"EXIT_SPECIFIC_BUILDING",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" empties.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 105
    TemplateText {
        ui_name: b"Team_/Garrison/Garrison a nearby building with a team.",
        internal_name: b"TEAM_GARRISON_NEAREST_BUILDING",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" garrison a nearby building.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 106
    TemplateText {
        ui_name: b"Team_/Garrison/Exit all buildings a team is in.",
        internal_name: b"TEAM_EXIT_ALL_BUILDINGS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" exits all buildings.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 107
    TemplateText {
        ui_name: b"Unit_/Garrison/Garrison a specific building with a specific unit.",
        internal_name: b"NAMED_GARRISON_SPECIFIC_BUILDING",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" garrison building ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::UNIT],
    },
    // 108
    TemplateText {
        ui_name: b"Unit_/Garrison/Garrison a nearby building with a specific unit.",
        internal_name: b"NAMED_GARRISON_NEAREST_BUILDING",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" garrison a nearby building.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 109
    TemplateText {
        ui_name: b"Unit_/Garrison/Exit the building the unit is in.",
        internal_name: b"NAMED_EXIT_BUILDING",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" leaves the building.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 110
    TemplateText {
        ui_name: b"Player_/Garrison/Garrison as many buildings as player has units for.",
        internal_name: b"PLAYER_GARRISON_ALL_BUILDINGS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" garrison buildings.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 111
    TemplateText {
        ui_name: b"Player_/Garrison/All units leave their garrisons.",
        internal_name: b"PLAYER_EXIT_ALL_BUILDINGS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" evacuate.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 112
    TemplateText {
        ui_name: b"Team_/Move/Set to follow a waypoint path -- wander.",
        internal_name: b"TEAM_WANDER",
        num_ui_strings: 2,
        ui_strings: &[
            b"Have ",
            b" wander along ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::WAYPOINT_PATH],
    },
    // 113
    TemplateText {
        ui_name: b"Team_/Move/Set to follow a waypoint path -- panic.",
        internal_name: b"TEAM_PANIC",
        num_ui_strings: 2,
        ui_strings: &[
            b"Have ",
            b" move in panic along ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::WAYPOINT_PATH],
    },
    // 114
    TemplateText {
        ui_name: b"Camera_/Adjust/Set up the camera.",
        internal_name: b"SETUP_CAMERA",
        num_ui_strings: 5,
        ui_strings: &[
            b"Position camera at ",
            b", zoom = ",
            b"(0.0 to 1.0), pitch = ",
            b"(1.0==default), looking towards ",
            b".",
        ],
        num_parameters: 4,
        parameters: &[P::WAYPOINT, P::REAL, P::REAL, P::WAYPOINT],
    },
    // 115
    TemplateText {
        ui_name: b"Camera_/Letterbox/Start letterbox mode.",
        internal_name: b"CAMERA_LETTERBOX_BEGIN",
        num_ui_strings: 1,
        ui_strings: &[
            b"Start letterbox mode (hide UI, add border).",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 116
    TemplateText {
        ui_name: b"Camera_/ End letterbox mode.",
        internal_name: b"CAMERA_LETTERBOX_END",
        num_ui_strings: 1,
        ui_strings: &[
            b"End letterbox mode (show UI, remove border).",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 117
    TemplateText {
        ui_name: b"Camera_/Adjust/Change the camera zoom.",
        internal_name: b"ZOOM_CAMERA",
        num_ui_strings: 5,
        ui_strings: &[
            b"Change camera zoom to ",
            b" in ",
            b" seconds, ease-in ",
            b" seconds, ease-out ",
            b" seconds.",
        ],
        num_parameters: 4,
        parameters: &[P::REAL, P::REAL, P::REAL, P::REAL],
    },
    // 118
    TemplateText {
        ui_name: b"Camera_/Adjust/Change the camera pitch.",
        internal_name: b"PITCH_CAMERA",
        num_ui_strings: 5,
        ui_strings: &[
            b"Change camera pitch to ",
            b" in ",
            b" seconds, ease-in ",
            b" seconds, ease-out ",
            b" seconds.",
        ],
        num_parameters: 4,
        parameters: &[P::REAL, P::REAL, P::REAL, P::REAL],
    },
    // 119
    TemplateText {
        ui_name: b"Camera_/Move/Follow a specific unit.",
        internal_name: b"CAMERA_FOLLOW_NAMED",
        num_ui_strings: 3,
        ui_strings: &[
            b"Have the camera follow ",
            b".  Snap camera to object is ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::BOOLEAN],
    },
    // 120
    TemplateText {
        ui_name: b"Camera_/Terrain/Oversize the terrain.",
        internal_name: b"OVERSIZE_TERRAIN",
        num_ui_strings: 2,
        ui_strings: &[
            b"Oversize the terrain ",
            b" tiles on each side [0 = reset to normal].",
        ],
        num_parameters: 1,
        parameters: &[],
    },
    // 121
    TemplateText {
        ui_name: b"Camera_/Fade Effects/Fade using an add blend to white.",
        internal_name: b"CAMERA_FADE_ADD",
        num_ui_strings: 6,
        ui_strings: &[
            b"Fade (0-1) from ",
            b" to ",
            b" adding toward white. Take ",
            b" frames to increase, hold for ",
            b" fames, and decrease ",
            b" frames.",
        ],
        num_parameters: 5,
        parameters: &[P::REAL, P::REAL],
    },
    // 122
    TemplateText {
        ui_name: b"Camera_/Fade Effects/Fade using a subtractive blend to black.",
        internal_name: b"CAMERA_FADE_SUBTRACT",
        num_ui_strings: 6,
        ui_strings: &[
            b"Fade (0-1) from ",
            b" to ",
            b" subtracting toward black. Take ",
            b" frames to increase, hold for ",
            b" fames, and decrease ",
            b" frames.",
        ],
        num_parameters: 5,
        parameters: &[P::REAL, P::REAL],
    },
    // 123
    TemplateText {
        ui_name: b"Camera_/Fade Effects/Fade using a saturate blend.",
        internal_name: b"CAMERA_FADE_SATURATE",
        num_ui_strings: 6,
        ui_strings: &[
            b"Fade (0.5-1) from ",
            b" to ",
            b" increasing saturation. Take ",
            b" frames to increase, hold for ",
            b" fames, and decrease ",
            b" frames.",
        ],
        num_parameters: 5,
        parameters: &[P::REAL, P::REAL],
    },
    // 124
    TemplateText {
        ui_name: b"Camera_/Fade Effects/Fade using a multiply blend to black.",
        internal_name: b"CAMERA_FADE_MULTIPLY",
        num_ui_strings: 6,
        ui_strings: &[
            b"Fade (1-0) from ",
            b" to ",
            b" multiplying toward black. Take ",
            b" frames to increase, hold for ",
            b" fames, and decrease ",
            b" frames.",
        ],
        num_parameters: 5,
        parameters: &[P::REAL, P::REAL],
    },
    // 125
    TemplateText {
        ui_name: b"Camera_/Fade Effects/Start black & white mode.",
        internal_name: b"CAMERA_BW_MODE_BEGIN",
        num_ui_strings: 1,
        ui_strings: &[
            b"Frames to fade into black & white mode = ",
        ],
        num_parameters: 1,
        parameters: &[],
    },
    // 126
    TemplateText {
        ui_name: b"Camera_/Fade Effects/End black & white mode.",
        internal_name: b"CAMERA_BW_MODE_END",
        num_ui_strings: 1,
        ui_strings: &[
            b"Frames to fade into color mode = ",
        ],
        num_parameters: 1,
        parameters: &[],
    },
    // 127
    TemplateText {
        ui_name: b"Camera_/Skybox/Start skybox mode.",
        internal_name: b"DRAW_SKYBOX_BEGIN",
        num_ui_strings: 1,
        ui_strings: &[
            b"Start skybox mode (draw sky background).",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 128
    TemplateText {
        ui_name: b"Camera_/Skybox/End skybox mode.",
        internal_name: b"DRAW_SKYBOX_END",
        num_ui_strings: 1,
        ui_strings: &[
            b"End skybox mode (draw black background).",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 129
    TemplateText {
        ui_name: b"AttackPrioritySet_/Set/Modify a set's priority for a single unit type.",
        internal_name: b"SET_ATTACK_PRIORITY_THING",
        num_ui_strings: 3,
        ui_strings: &[
            b"For ",
            b" set the priority of object type ",
            b" to ",
        ],
        num_parameters: 3,
        parameters: &[P::ATTACK_PRIORITY_SET, P::OBJECT_TYPE],
    },
    // 130
    TemplateText {
        ui_name: b"AttackPrioritySet_/Set/Modify a set's priorities for all of a kind.",
        internal_name: b"SET_ATTACK_PRIORITY_KIND_OF",
        num_ui_strings: 3,
        ui_strings: &[
            b"For ",
            b" set the priority of object type ",
            b" to ",
        ],
        num_parameters: 3,
        parameters: &[P::ATTACK_PRIORITY_SET, P::KIND_OF_PARAM],
    },
    // 131
    TemplateText {
        ui_name: b"AttackPrioritySet_/Set/Specify the set's default priority.",
        internal_name: b"SET_DEFAULT_ATTACK_PRIORITY",
        num_ui_strings: 2,
        ui_strings: &[
            b"For ",
            b" set the default priority to ",
        ],
        num_parameters: 2,
        parameters: &[P::ATTACK_PRIORITY_SET],
    },
    // 132
    TemplateText {
        ui_name: b"Camera_/Move/Stop following any units.",
        internal_name: b"CAMERA_STOP_FOLLOW",
        num_ui_strings: 1,
        ui_strings: &[
            b"Stop following any units.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 133
    TemplateText {
        ui_name: b"Camera_/Fade Effects/Motion blur zoom.",
        internal_name: b"CAMERA_MOTION_BLUR",
        num_ui_strings: 2,
        ui_strings: &[
            b"Blur zoom, zoom in = ",
            b" (true=zoom in, false = zoom out), saturate colors = ",
        ],
        num_parameters: 2,
        parameters: &[P::BOOLEAN, P::BOOLEAN],
    },
    // 134
    TemplateText {
        ui_name: b"Camera_/Fade Effects/Motion blur zoom with jump cut.",
        internal_name: b"CAMERA_MOTION_BLUR_JUMP",
        num_ui_strings: 2,
        ui_strings: &[
            b"Blur zoom, zoom in at current location, zoom out at ",
            b", saturate colors = ",
        ],
        num_parameters: 2,
        parameters: &[P::WAYPOINT, P::BOOLEAN],
    },
    // 135
    TemplateText {
        ui_name: b"Camera_/Fade Effects/Start motion blur as the camera moves.",
        internal_name: b"CAMERA_MOTION_BLUR_FOLLOW",
        num_ui_strings: 2,
        ui_strings: &[
            b"Start motion blur as the camera moves, amount= ",
            b" (start with 30 and adjust up or down). ",
        ],
        num_parameters: 1,
        parameters: &[],
    },
    // 136
    TemplateText {
        ui_name: b"Camera_/Fade Effects/End motion blur as the camera moves.",
        internal_name: b"CAMERA_MOTION_BLUR_END_FOLLOW",
        num_ui_strings: 1,
        ui_strings: &[
            b"End motion blur as the camera moves.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 137
    TemplateText {
        ui_name: b"Scripting_/Time/Freeze time.",
        internal_name: b"FREEZE_TIME",
        num_ui_strings: 1,
        ui_strings: &[
            b"Freeze time.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 138
    TemplateText {
        ui_name: b"Scripting_/Time/Unfreeze time.",
        internal_name: b"UNFREEZE_TIME",
        num_ui_strings: 1,
        ui_strings: &[
            b"Unfreeze time.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 139
    TemplateText {
        ui_name: b"Scripting_/Briefing/Show military briefing caption.",
        internal_name: b"SHOW_MILITARY_CAPTION",
        num_ui_strings: 3,
        ui_strings: &[
            b"Show military briefing ",
            b" for ",
            b" milliseconds.",
        ],
        num_parameters: 2,
        parameters: &[P::TEXT_STRING],
    },
    // 140
    TemplateText {
        ui_name: b"Camera_/Sounds/Set the audible distance for camera-up shots.",
        internal_name: b"CAMERA_SET_AUDIBLE_DISTANCE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set the audible range during camera-up shots to ",
        ],
        num_parameters: 1,
        parameters: &[P::REAL],
    },
    // 141
    TemplateText {
        ui_name: b"Team_/Move/Set stopping distance for each unit's current locomotor.",
        internal_name: b"SET_STOPPING_DISTANCE",
        num_ui_strings: 3,
        ui_strings: &[
            b"Set stopping distances for ",
            b" to ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::REAL],
    },
    // 142
    TemplateText {
        ui_name: b"Unit_/Move/Set stopping distance for current locomotor.",
        internal_name: b"NAMED_SET_STOPPING_DISTANCE",
        num_ui_strings: 3,
        ui_strings: &[
            b"Set stopping distance for ",
            b" to ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::REAL],
    },
    // 143
    TemplateText {
        ui_name: b"Scripting_/ Set max frames per second.",
        internal_name: b"SET_FPS_LIMIT",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set max FPS to ",
            b".  (0 sets to default.)",
        ],
        num_parameters: 1,
        parameters: &[],
    },
    // 144
    TemplateText {
        ui_name: b"Multimedia_/ Set the current music volume.",
        internal_name: b"MUSIC_SET_VOLUME",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set the desired music volume to ",
            b"%. (0-100)",
        ],
        num_parameters: 1,
        parameters: &[P::REAL],
    },
    // 145
    TemplateText {
        ui_name: b"Map_/Shroud or Reveal/Shroud map at waypoint -- add fog.",
        internal_name: b"MAP_SHROUD_AT_WAYPOINT",
        num_ui_strings: 4,
        ui_strings: &[
            b"The map is shrouded at ",
            b" with a radius of ",
            b" feet for ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::WAYPOINT, P::REAL, P::SIDE],
    },
    // 146
    TemplateText {
        ui_name: b"Map_/Shroud or Reveal/Shroud the entire map for a player.",
        internal_name: b"MAP_SHROUD_ALL",
        num_ui_strings: 2,
        ui_strings: &[
            b"The world is shrouded for ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 147
    TemplateText {
        ui_name: b"Scripting_/Timer/Frame countdown timer -- set random.",
        internal_name: b"SET_RANDOM_TIMER",
        num_ui_strings: 4,
        ui_strings: &[
            b"Set timer ",
            b" to expire between ",
            b" and ",
            b" frames.",
        ],
        num_parameters: 3,
        parameters: &[P::COUNTER],
    },
    // 148
    TemplateText {
        ui_name: b"Scripting_/Timer/Seconds countdown timer -- set random.",
        internal_name: b"SET_RANDOM_MSEC_TIMER",
        num_ui_strings: 4,
        ui_strings: &[
            b"Set timer ",
            b" to expire between ",
            b" and ",
            b" seconds.",
        ],
        num_parameters: 3,
        parameters: &[P::COUNTER, P::REAL, P::REAL],
    },
    // 149
    TemplateText {
        ui_name: b"Scripting_/Timer/Timer -- stop.",
        internal_name: b"STOP_TIMER",
        num_ui_strings: 1,
        ui_strings: &[
            b"Stop timer ",
        ],
        num_parameters: 1,
        parameters: &[P::COUNTER],
    },
    // 150
    TemplateText {
        ui_name: b"Scripting_/Timer/Timer -- restart stopped.",
        internal_name: b"RESTART_TIMER",
        num_ui_strings: 1,
        ui_strings: &[
            b"Restart timer ",
        ],
        num_parameters: 1,
        parameters: &[P::COUNTER],
    },
    // 151
    TemplateText {
        ui_name: b"Scripting_/Timer/Seconds countdown timer -- add seconds.",
        internal_name: b"ADD_TO_MSEC_TIMER",
        num_ui_strings: 3,
        ui_strings: &[
            b"Add ",
            b" seconds to timer ",
            b" .",
        ],
        num_parameters: 2,
        parameters: &[P::REAL, P::COUNTER],
    },
    // 152
    TemplateText {
        ui_name: b"Scripting_/Timer/Seconds countdown timer -- subtract seconds.",
        internal_name: b"SUB_FROM_MSEC_TIMER",
        num_ui_strings: 3,
        ui_strings: &[
            b"Subtract ",
            b" seconds from timer ",
            b" .",
        ],
        num_parameters: 2,
        parameters: &[P::REAL, P::COUNTER],
    },
    // 153
    TemplateText {
        ui_name: b"Team_/ Transfer control of a team to a player.",
        internal_name: b"TEAM_TRANSFER_TO_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b"Control of ",
            b" transfers to ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::SIDE],
    },
    // 154
    TemplateText {
        ui_name: b"Player_/ Set player's money.",
        internal_name: b"PLAYER_SET_MONEY",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set ",
            b"'s money to $",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 155
    TemplateText {
        ui_name: b"Player_/ Gives/takes from player's money.",
        internal_name: b"PLAYER_GIVE_MONEY",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" gets $",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 156
    TemplateText {
        ui_name: b"Scripting_/ Special power countdown display -- disable.",
        internal_name: b"DISABLE_SPECIAL_POWER_DISPLAY",
        num_ui_strings: 1,
        ui_strings: &[
            b"Disables special power countdown display.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 157
    TemplateText {
        ui_name: b"Scripting_/ Special power countdown display -- enable.",
        internal_name: b"ENABLE_SPECIAL_POWER_DISPLAY",
        num_ui_strings: 1,
        ui_strings: &[
            b"Enables special power countdown display.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 158
    TemplateText {
        ui_name: b"Unit_/ Special power countdown timer -- hide.",
        internal_name: b"NAMED_HIDE_SPECIAL_POWER_DISPLAY",
        num_ui_strings: 2,
        ui_strings: &[
            b"Hides special power countdowns for ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 159
    TemplateText {
        ui_name: b"Unit_/ Special power countdown timer -- display.",
        internal_name: b"NAMED_SHOW_SPECIAL_POWER_DISPLAY",
        num_ui_strings: 2,
        ui_strings: &[
            b"Shows special power countdowns for ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 160
    TemplateText {
        ui_name: b"Scripting_/ Timer -- display an individual timer to the user.",
        internal_name: b"DISPLAY_COUNTDOWN_TIMER",
        num_ui_strings: 2,
        ui_strings: &[
            b"Show ",
            b" with text ",
        ],
        num_parameters: 2,
        parameters: &[P::COUNTER, P::LOCALIZED_TEXT],
    },
    // 161
    TemplateText {
        ui_name: b"Scripting_/ Timer -- hides an individual timer from the user.",
        internal_name: b"HIDE_COUNTDOWN_TIMER",
        num_ui_strings: 1,
        ui_strings: &[
            b"Hide ",
        ],
        num_parameters: 1,
        parameters: &[P::COUNTER],
    },
    // 162
    TemplateText {
        ui_name: b"Scripting_/ Timer -- display all timers to the user.",
        internal_name: b"ENABLE_COUNTDOWN_TIMER_DISPLAY",
        num_ui_strings: 1,
        ui_strings: &[
            b"Enables timer display.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 163
    TemplateText {
        ui_name: b"Scripting_/ Timer -- hide all timers from the user.",
        internal_name: b"DISABLE_COUNTDOWN_TIMER_DISPLAY",
        num_ui_strings: 1,
        ui_strings: &[
            b"Disables timer display.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 164
    TemplateText {
        ui_name: b"Unit_/ Special power countdown timer -- pause.",
        internal_name: b"NAMED_STOP_SPECIAL_POWER_COUNTDOWN",
        num_ui_strings: 3,
        ui_strings: &[
            b"Pause ",
            b"'s ",
            b" countdown.",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::SPECIAL_POWER],
    },
    // 165
    TemplateText {
        ui_name: b"Unit_/ Special power countdown timer -- resume.",
        internal_name: b"NAMED_START_SPECIAL_POWER_COUNTDOWN",
        num_ui_strings: 3,
        ui_strings: &[
            b"Resume ",
            b"'s ",
            b" countdown.",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::SPECIAL_POWER],
    },
    // 166
    TemplateText {
        ui_name: b"Unit_/ Special power countdown timer -- set.",
        internal_name: b"NAMED_SET_SPECIAL_POWER_COUNTDOWN",
        num_ui_strings: 4,
        ui_strings: &[
            b"Set ",
            b"'s ",
            b" to ",
            b" seconds.",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::SPECIAL_POWER],
    },
    // 167
    TemplateText {
        ui_name: b"Unit_/ Special power countdown timer -- add seconds.",
        internal_name: b"NAMED_ADD_SPECIAL_POWER_COUNTDOWN",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b"'s ",
            b" has ",
            b" seconds added to it.",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::SPECIAL_POWER],
    },
    // 168
    TemplateText {
        ui_name: b"Unit_/ Special power -- fire at location.",
        internal_name: b"NAMED_FIRE_SPECIAL_POWER_AT_WAYPOINT",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" fires ",
            b" at ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::SPECIAL_POWER, P::WAYPOINT],
    },
    // 169
    TemplateText {
        ui_name: b"Unit_/ Special power -- fire at unit.",
        internal_name: b"NAMED_FIRE_SPECIAL_POWER_AT_NAMED",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" fires ",
            b" at ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::SPECIAL_POWER, P::UNIT],
    },
    // 170
    TemplateText {
        ui_name: b"Scripting_/ Refresh radar terrain.",
        internal_name: b"REFRESH_RADAR",
        num_ui_strings: 1,
        ui_strings: &[
            b"Refresh radar terrain.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 171
    TemplateText {
        ui_name: b"Camera_/ Tether camera to a specific unit.",
        internal_name: b"CAMERA_TETHER_NAMED",
        num_ui_strings: 4,
        ui_strings: &[
            b"Have the camera tethered to ",
            b".  Snap camera to object is ",
            b".  Amount of play is ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::BOOLEAN, P::REAL],
    },
    // 172
    TemplateText {
        ui_name: b"Camera_/ Stop tether to any units.",
        internal_name: b"CAMERA_STOP_TETHER_NAMED",
        num_ui_strings: 1,
        ui_strings: &[
            b"Stop tether to any units.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 173
    TemplateText {
        ui_name: b"Camera_/ Set default camera.",
        internal_name: b"CAMERA_SET_DEFAULT",
        num_ui_strings: 4,
        ui_strings: &[
            b"Camera Pitch = ",
            b"(0.0==default), angle = ",
            b"(0.0 is N, 90.0 is W, etc), height = ",
            b"(1.0==default).",
        ],
        num_parameters: 3,
        parameters: &[P::REAL, P::REAL, P::REAL],
    },
    // 174
    TemplateText {
        ui_name: b"Unit_/ Set a specific unit to stop.",
        internal_name: b"NAMED_STOP",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" stops.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 175
    TemplateText {
        ui_name: b"Team_/ Set to stop.",
        internal_name: b"TEAM_STOP",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" stops.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 176
    TemplateText {
        ui_name: b"Team_/ Set to stop, then disband.",
        internal_name: b"TEAM_STOP_AND_DISBAND",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" stops, then disbands.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 177
    TemplateText {
        ui_name: b"Team_/Create/Recruit a team.",
        internal_name: b"RECRUIT_TEAM",
        num_ui_strings: 3,
        ui_strings: &[
            b"Recruit an instance of ",
            b", maximum recruiting distance (feet):",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::REAL],
    },
    // 178
    TemplateText {
        ui_name: b"Team_/ Override a team's relationship to another team.",
        internal_name: b"TEAM_SET_OVERRIDE_RELATION_TO_TEAM",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" considers ",
            b" to be ",
            b" (rather than using the the player relationship).",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::TEAM, P::RELATION],
    },
    // 179
    TemplateText {
        ui_name: b"Team_/ Remove an override to a team's relationship to another team.",
        internal_name: b"TEAM_REMOVE_OVERRIDE_RELATION_TO_TEAM",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" uses the player relationship to ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::TEAM],
    },
    // 180
    TemplateText {
        ui_name: b"Team_/ Remove all overrides to team's relationship to teams and/or players.",
        internal_name: b"TEAM_REMOVE_ALL_OVERRIDE_RELATIONS",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" uses the player relationship to all other teams and players.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 181
    TemplateText {
        ui_name: b"Camera (R)_/ Rotate toward unit.",
        internal_name: b"CAMERA_LOOK_TOWARD_OBJECT",
        num_ui_strings: 6,
        ui_strings: &[
            b"Rotate toward ",
            b", taking ",
            b" seconds and holding ",
            b" seconds, ease-in ",
            b" seconds, ease-out ",
            b" seconds.",
        ],
        num_parameters: 5,
        parameters: &[P::UNIT, P::REAL, P::REAL, P::REAL, P::REAL],
    },
    // 182
    TemplateText {
        ui_name: b"Unit_/ Fire waypoint-weapon following waypoint path.",
        internal_name: b"NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" fire waypoint-weapon following waypoints starting at ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::WAYPOINT_PATH],
    },
    // 183
    TemplateText {
        ui_name: b"Team_/ Override a team's relationship to another player.",
        internal_name: b"TEAM_SET_OVERRIDE_RELATION_TO_PLAYER",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" considers ",
            b" to be ",
            b" (rather than using the the player relationship).",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::SIDE, P::RELATION],
    },
    // 184
    TemplateText {
        ui_name: b"Team_/ Remove an override to a team's relationship to another player.",
        internal_name: b"TEAM_REMOVE_OVERRIDE_RELATION_TO_PLAYER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" uses the player relationship to ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::SIDE],
    },
    // 185
    TemplateText {
        ui_name: b"Player_/ Override a player's relationship to another team.",
        internal_name: b"PLAYER_SET_OVERRIDE_RELATION_TO_TEAM",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" considers ",
            b" to be ",
            b" (rather than using the the player relationship).",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::TEAM, P::RELATION],
    },
    // 186
    TemplateText {
        ui_name: b"Player_/ Remove an override to a player's relationship to another team.",
        internal_name: b"PLAYER_REMOVE_OVERRIDE_RELATION_TO_TEAM",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" uses the player relationship to ",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::TEAM],
    },
    // 187
    TemplateText {
        ui_name: b"Unit_/ Set a specific unit to execute a script sequentially.",
        internal_name: b"UNIT_EXECUTE_SEQUENTIAL_SCRIPT",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" executes ",
            b" sequentially.",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::SCRIPT],
    },
    // 188
    TemplateText {
        ui_name: b"Unit_/ Set a specific unit to execute a looping sequential script.",
        internal_name: b"UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" executes ",
            b" sequentially, ",
            b" times. (0=forever)",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::SCRIPT],
    },
    // 189
    TemplateText {
        ui_name: b"Unit_/ Set a specific unit to stop executing a sequential script.",
        internal_name: b"UNIT_STOP_SEQUENTIAL_SCRIPT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" stops executing.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 190
    TemplateText {
        ui_name: b"Team_/ Execute script sequentially -- start.",
        internal_name: b"TEAM_EXECUTE_SEQUENTIAL_SCRIPT",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" executes ",
            b" sequentially.",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::SCRIPT],
    },
    // 191
    TemplateText {
        ui_name: b"Team_/ Execute script sequentially -- looping.",
        internal_name: b"TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" executes ",
            b" sequentially, ",
            b" times. (0=forever)",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::SCRIPT],
    },
    // 192
    TemplateText {
        ui_name: b"Team_/ Execute script sequentially -- stop.",
        internal_name: b"TEAM_STOP_SEQUENTIAL_SCRIPT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" stops executing.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 193
    TemplateText {
        ui_name: b"Unit_/ Set to guard for some number of frames.",
        internal_name: b"UNIT_GUARD_FOR_FRAMECOUNT",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" guards for ",
            b" frames.",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT],
    },
    // 194
    TemplateText {
        ui_name: b"Unit_/ Set to idle for some number of frames.",
        internal_name: b"UNIT_IDLE_FOR_FRAMECOUNT",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" idles for ",
            b" frames.",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT],
    },
    // 195
    TemplateText {
        ui_name: b"Team_/ Set to guard -- number of frames.",
        internal_name: b"TEAM_GUARD_FOR_FRAMECOUNT",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" guards for ",
            b" frames.",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM],
    },
    // 196
    TemplateText {
        ui_name: b"Team_/ Set to idle for some number of frames.",
        internal_name: b"TEAM_IDLE_FOR_FRAMECOUNT",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" idles for ",
            b" frames.",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM],
    },
    // 197
    TemplateText {
        ui_name: b"Map_/ Adjust water height to a new level",
        internal_name: b"WATER_CHANGE_HEIGHT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" changes altitude to ",
        ],
        num_parameters: 2,
        parameters: &[P::TRIGGER_AREA, P::REAL],
    },
    // 198
    TemplateText {
        ui_name: b"Unit_/ Use commandbutton ability on an object.",
        internal_name: b"NAMED_USE_COMMANDBUTTON_ABILITY_ON_NAMED",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" use ",
            b" on ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::COMMANDBUTTON_ABILITY, P::UNIT],
    },
    // 199
    TemplateText {
        ui_name: b"Unit_/ Use commandbutton ability at a waypoint.",
        internal_name: b"NAMED_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" use ",
            b" at ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::COMMANDBUTTON_ABILITY, P::WAYPOINT],
    },
    // 200
    TemplateText {
        ui_name: b"Map_/ Adjust water height to a new level with damage over time",
        internal_name: b"WATER_CHANGE_HEIGHT_OVER_TIME",
        num_ui_strings: 5,
        ui_strings: &[
            b" ",
            b" changes altitude to ",
            b" in ",
            b" seconds doing ",
            b" dam_/sec.",
        ],
        num_parameters: 4,
        parameters: &[P::TRIGGER_AREA, P::REAL, P::REAL, P::REAL],
    },
    // 201
    TemplateText {
        ui_name: b"Map_/ Change the active boundary.",
        internal_name: b"MAP_SWITCH_BORDER",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" becomes the active border.",
        ],
        num_parameters: 1,
        parameters: &[P::BOUNDARY],
    },
    // 202
    TemplateText {
        ui_name: b"Team_/Guard/Set to guard -- location.",
        internal_name: b"TEAM_GUARD_POSITION",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begins guarding at ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::WAYPOINT],
    },
    // 203
    TemplateText {
        ui_name: b"Team_/Guard/Set to guard -- specific unit.",
        internal_name: b"TEAM_GUARD_OBJECT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begins guarding ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::UNIT],
    },
    // 204
    TemplateText {
        ui_name: b"Team_/Guard/Set to guard -- area.",
        internal_name: b"TEAM_GUARD_AREA",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begins guarding ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::TRIGGER_AREA],
    },
    // 205
    TemplateText {
        ui_name: b"Scripting_/ Select the first object type on a team.",
        internal_name: b"OBJECT_FORCE_SELECT",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" 's first ",
            b", centers in view (",
            b") while playing ",
        ],
        num_parameters: 4,
        parameters: &[P::TEAM, P::OBJECT_TYPE, P::BOOLEAN, P::DIALOG],
    },
    // 206
    TemplateText {
        ui_name: b"Camera (R)_/ Rotate to look at a waypoint.",
        internal_name: b"CAMERA_LOOK_TOWARD_WAYPOINT",
        num_ui_strings: 6,
        ui_strings: &[
            b"Rotate to look at ",
            b", taking ",
            b" seconds, ease-in ",
            b" seconds, ease-out ",
            b" seconds, reverse rotation ",
            b".",
        ],
        num_parameters: 5,
        parameters: &[P::WAYPOINT, P::REAL, P::REAL, P::REAL, P::BOOLEAN],
    },
    // 207
    TemplateText {
        ui_name: b"Unit_/ Kill all units contained within a specific transport or structure.",
        internal_name: b"UNIT_DESTROY_ALL_CONTAINED",
        num_ui_strings: 2,
        ui_strings: &[
            b"All units inside ",
            b" are killed.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 208
    TemplateText {
        ui_name: b"Radar_/ Force enable the radar.",
        internal_name: b"RADAR_FORCE_ENABLE",
        num_ui_strings: 1,
        ui_strings: &[
            b"The radar is now forced to be enabled.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 209
    TemplateText {
        ui_name: b"Radar_/ Revert radar to normal behavior.",
        internal_name: b"RADAR_REVERT_TO_NORMAL",
        num_ui_strings: 1,
        ui_strings: &[
            b"The radar is now reverting to its normal behavior.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 210
    TemplateText {
        ui_name: b"Camera_/ Shake Screen.",
        internal_name: b"SCREEN_SHAKE",
        num_ui_strings: 1,
        ui_strings: &[
            b"The screen will shake with ",
        ],
        num_parameters: 1,
        parameters: &[P::SHAKE_INTENSITY],
    },
    // 211
    TemplateText {
        ui_name: b"Map_/ Adjust the tech tree for a specific object type.",
        internal_name: b"TECHTREE_MODIFY_BUILDABILITY_OBJECT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" becomes ",
        ],
        num_parameters: 2,
        parameters: &[P::OBJECT_TYPE, P::BUILDABLE],
    },
    // 212
    TemplateText {
        ui_name: b"Unit_/ Set cash value of Warehouse.",
        internal_name: b"WAREHOUSE_SET_VALUE",
        num_ui_strings: 3,
        ui_strings: &[
            b"Warehouse named ",
            b" is set to having ",
            b" dollars worth of boxes. ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT],
    },
    // 213
    TemplateText {
        ui_name: b"Radar_/Create Event/Create  a radar event at a specific object.",
        internal_name: b"OBJECT_CREATE_RADAR_EVENT",
        num_ui_strings: 2,
        ui_strings: &[
            b"A radar event occurs at ",
            b" of type ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::RADAR_EVENT_TYPE],
    },
    // 214
    TemplateText {
        ui_name: b"Radar_/Create Event/Create  a radar event at a specific team.",
        internal_name: b"TEAM_CREATE_RADAR_EVENT",
        num_ui_strings: 2,
        ui_strings: &[
            b"A radar event occurs at ",
            b" of type ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::RADAR_EVENT_TYPE],
    },
    // 215
    TemplateText {
        ui_name: b"User_/String/Display a cinematic string.",
        internal_name: b"DISPLAY_CINEMATIC_TEXT",
        num_ui_strings: 4,
        ui_strings: &[
            b"Displays ",
            b" with font type ",
            b" in the bottom letterbox for ",
            b" seconds.",
        ],
        num_parameters: 3,
        parameters: &[P::LOCALIZED_TEXT, P::FONT_NAME],
    },
    // 216
    TemplateText {
        ui_name: b"{INTERNAL}_/Debug/Display a crash box (debug/internal builds only).",
        internal_name: b"DEBUG_CRASH_BOX",
        num_ui_strings: 1,
        ui_strings: &[
            b"Display a crash box with the text: ",
        ],
        num_parameters: 1,
        parameters: &[P::TEXT_STRING],
    },
    // 217
    TemplateText {
        ui_name: b"Multimedia_/ Sound Events -- disable type.",
        internal_name: b"SOUND_DISABLE_TYPE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is disabled.",
        ],
        num_parameters: 1,
        parameters: &[P::SOUND],
    },
    // 218
    TemplateText {
        ui_name: b"Multimedia_/ Sound Events -- enable type.",
        internal_name: b"SOUND_ENABLE_TYPE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is enabled.",
        ],
        num_parameters: 1,
        parameters: &[P::SOUND],
    },
    // 219
    TemplateText {
        ui_name: b"Multimedia_/ Sound Events -- enable all.",
        internal_name: b"SOUND_ENABLE_ALL",
        num_ui_strings: 1,
        ui_strings: &[
            b"Enable all sound events.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 220
    TemplateText {
        ui_name: b"Multimedia_/ Sound Events -- override volume -- type.",
        internal_name: b"AUDIO_OVERRIDE_VOLUME_TYPE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" play at ",
            b"% of full volume.",
        ],
        num_parameters: 2,
        parameters: &[P::SOUND, P::REAL],
    },
    // 221
    TemplateText {
        ui_name: b"Multimedia_/ Sound Events -- restore volume -- type.",
        internal_name: b"AUDIO_RESTORE_VOLUME_TYPE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" play at normal volume.",
        ],
        num_parameters: 1,
        parameters: &[P::SOUND],
    },
    // 222
    TemplateText {
        ui_name: b"Multimedia_/ Sound Events -- restore volume -- all.",
        internal_name: b"AUDIO_RESTORE_VOLUME_ALL_TYPE",
        num_ui_strings: 1,
        ui_strings: &[
            b"All sound events play at normal volume.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 223
    TemplateText {
        ui_name: b"User_/String/Display Popup Message Box.",
        internal_name: b"INGAME_POPUP_MESSAGE",
        num_ui_strings: 6,
        ui_strings: &[
            b"Displays ",
            b" at ",
            b" percent of the screen Width ",
            b" percent of the screen Height and a width of ",
            b" pixels and pauses the game (",
            b" )",
        ],
        num_parameters: 5,
        parameters: &[P::LOCALIZED_TEXT, P::INT, P::INT, P::INT, P::BOOLEAN],
    },
    // 224
    TemplateText {
        ui_name: b"Unit_/ Set Cave connectivity index.",
        internal_name: b"SET_CAVE_INDEX",
        num_ui_strings: 3,
        ui_strings: &[
            b"Cave named ",
            b" is set to being connected to all caves of index ",
            b", but only if both Cave listings have no occupants. ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT],
    },
    // 225
    TemplateText {
        ui_name: b"Unit_/Move/Set unit to be held in place, ignoring Physics, Locomotors, etc.",
        internal_name: b"NAMED_SET_HELD",
        num_ui_strings: 3,
        ui_strings: &[
            b"Set Held status for ",
            b" to ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::BOOLEAN],
    },
    // 226
    TemplateText {
        ui_name: b"Unit_/ Set topple direction.",
        internal_name: b"NAMED_SET_TOPPLE_DIRECTION",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" will topple towards ",
            b" if destroyed.",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::COORD3D],
    },
    // 227
    TemplateText {
        ui_name: b"Unit_/ Move unit towards the nearest object of a specific type.",
        internal_name: b"UNIT_MOVE_TOWARDS_NEAREST_OBJECT_TYPE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" will move towards the nearest ",
            b" within ",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::OBJECT_TYPE, P::TRIGGER_AREA],
    },
    // 228
    TemplateText {
        ui_name: b"Team_/ Move team towards the nearest object of a specific type.",
        internal_name: b"TEAM_MOVE_TOWARDS_NEAREST_OBJECT_TYPE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" will move towards the nearest ",
            b" within ",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::OBJECT_TYPE, P::TRIGGER_AREA],
    },
    // 229
    TemplateText {
        ui_name: b"Map_/Shroud or Reveal/Reveal the entire map permanently for a player.",
        internal_name: b"MAP_REVEAL_ALL_PERM",
        num_ui_strings: 2,
        ui_strings: &[
            b"The world is revealed permanently for ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 230
    TemplateText {
        ui_name: b"Map_/Shroud or Reveal/Un-Reveal the entire map permanently for a player.",
        internal_name: b"MAP_REVEAL_ALL_UNDO_PERM",
        num_ui_strings: 2,
        ui_strings: &[
            b"Undo the permanent reveal for ",
            b".  This will mess things up badly if called when there has been no permanent reveal.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 231
    TemplateText {
        ui_name: b"Unit_/Internal/Repulsor/Set the REPULSOR flag of a specific unit.",
        internal_name: b"NAMED_SET_REPULSOR",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" REPULSOR flag is ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::BOOLEAN],
    },
    // 232
    TemplateText {
        ui_name: b"Team_/Internal/Repulsor/Set the REPULSOR flag of a team.",
        internal_name: b"TEAM_SET_REPULSOR",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" REPULSOR flag is ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::BOOLEAN],
    },
    // 233
    TemplateText {
        ui_name: b"Team_/Move/Set to wander around current location.",
        internal_name: b"TEAM_WANDER_IN_PLACE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Have ",
            b" wander around it's current location.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 234
    TemplateText {
        ui_name: b"Team_/AI/Increase priority by Success Priority Increase amount.",
        internal_name: b"TEAM_INCREASE_PRIORITY",
        num_ui_strings: 2,
        ui_strings: &[
            b"Increase the AI priority for",
            b"  by its Success Priority Increase amount.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 235
    TemplateText {
        ui_name: b"Team_/AI/Reduce priority by Failure Priority Decrease amount.",
        internal_name: b"TEAM_DECREASE_PRIORITY",
        num_ui_strings: 2,
        ui_strings: &[
            b"Reduce the AI priority for",
            b"  by its Failure Priority Decrease amount.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 236
    TemplateText {
        ui_name: b"Scripting_/ Counter -- display an individual counter to the user.",
        internal_name: b"DISPLAY_COUNTER",
        num_ui_strings: 2,
        ui_strings: &[
            b"Show ",
            b" with text ",
        ],
        num_parameters: 2,
        parameters: &[P::COUNTER, P::LOCALIZED_TEXT],
    },
    // 237
    TemplateText {
        ui_name: b"Scripting_/ Counter -- hides an individual counter from the user.",
        internal_name: b"HIDE_COUNTER",
        num_ui_strings: 1,
        ui_strings: &[
            b"Hide ",
        ],
        num_parameters: 1,
        parameters: &[P::COUNTER],
    },
    // 238
    TemplateText {
        ui_name: b"Team_/ Use commandbutton ability on an object.",
        internal_name: b"TEAM_USE_COMMANDBUTTON_ABILITY_ON_NAMED",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" use ",
            b" on ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES, P::UNIT],
    },
    // 239
    TemplateText {
        ui_name: b"Team_/ Use commandbutton ability at a waypoint.",
        internal_name: b"TEAM_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" use ",
            b" at ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES, P::WAYPOINT],
    },
    // 240
    TemplateText {
        ui_name: b"Unit_/ Use commandbutton ability.",
        internal_name: b"NAMED_USE_COMMANDBUTTON_ABILITY",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" use ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::COMMANDBUTTON_ABILITY],
    },
    // 241
    TemplateText {
        ui_name: b"Team_/ Use commandbutton ability.",
        internal_name: b"TEAM_USE_COMMANDBUTTON_ABILITY",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" use ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 242
    TemplateText {
        ui_name: b"User_/Flash/Flash a specific unit white for a specified amount of time.",
        internal_name: b"NAMED_FLASH_WHITE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" flashes white for ",
            b" seconds.",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT],
    },
    // 243
    TemplateText {
        ui_name: b"User_/Flash/Flash a team white for a specified amount of time.",
        internal_name: b"TEAM_FLASH_WHITE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" flashes white for ",
            b" seconds.",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM],
    },
    // 244
    TemplateText {
        ui_name: b"Skirmish Only_/ Build a building.",
        internal_name: b"SKIRMISH_BUILD_BUILDING",
        num_ui_strings: 1,
        ui_strings: &[
            b"Build a building of type ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[P::OBJECT_TYPE],
    },
    // 245
    TemplateText {
        ui_name: b"Skirmish Only_/Move/Team follow approach path.",
        internal_name: b"SKIRMISH_FOLLOW_APPROACH_PATH",
        num_ui_strings: 3,
        ui_strings: &[
            b"Have ",
            b" approach the enemy using path ",
            b", as a team is ",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::SKIRMISH_WAYPOINT_PATH, P::BOOLEAN],
    },
    // 246
    TemplateText {
        ui_name: b"Scripting_/Idle or Restart/Idle all units for all players.",
        internal_name: b"IDLE_ALL_UNITS",
        num_ui_strings: 1,
        ui_strings: &[
            b"Idle all units for all players.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 247
    TemplateText {
        ui_name: b"Scripting_/Idle or Restart/All idle Supply Trucks attempt to resume supply routes.",
        internal_name: b"RESUME_SUPPLY_TRUCKING",
        num_ui_strings: 1,
        ui_strings: &[
            b"All idle Supply Trucks attempt to resume supply routes.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 248
    TemplateText {
        ui_name: b"User_/Flash/Set a specific unit to use a special indicator color.",
        internal_name: b"NAMED_CUSTOM_COLOR",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" uses the color ",
            b" .",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::COLOR],
    },
    // 249
    TemplateText {
        ui_name: b"Skirmish Only_/Move/Team move to approach path.",
        internal_name: b"SKIRMISH_MOVE_TO_APPROACH_PATH",
        num_ui_strings: 3,
        ui_strings: &[
            b"Have ",
            b" move to the start of enemy path ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::SKIRMISH_WAYPOINT_PATH],
    },
    // 250
    TemplateText {
        ui_name: b"Skirmish Only_/Build/Build base defense on front perimeter.",
        internal_name: b"SKIRMISH_BUILD_BASE_DEFENSE_FRONT",
        num_ui_strings: 1,
        ui_strings: &[
            b"Build one additional perimeter base defenses, on the front.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 251
    TemplateText {
        ui_name: b"Skirmish_/ Special power -- fire at enemy's highest cost area.",
        internal_name: b"SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" fire ",
            b" at enemy's most costly area.",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SPECIAL_POWER],
    },
    // 252: NAMED_RECEIVE_UPGRADE (not set by init)
    UNSET,
    // 253
    TemplateText {
        ui_name: b"Player_/ Repair named bridge or structure.",
        internal_name: b"PLAYER_REPAIR_NAMED_STRUCTURE",
        num_ui_strings: 3,
        ui_strings: &[
            b"Have ",
            b" repair ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::UNIT],
    },
    // 254
    TemplateText {
        ui_name: b"Skirmish Only_/Build/Build base defense on flank perimeter.",
        internal_name: b"SKIRMISH_BUILD_BASE_DEFENSE_FLANK",
        num_ui_strings: 1,
        ui_strings: &[
            b"Build one additional perimeter base defenses, on the flank.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 255
    TemplateText {
        ui_name: b"Skirmish Only_/Build/Build structure on front perimeter.",
        internal_name: b"SKIRMISH_BUILD_STRUCTURE_FRONT",
        num_ui_strings: 2,
        ui_strings: &[
            b"Build one additional ",
            b", on the front.",
        ],
        num_parameters: 1,
        parameters: &[P::OBJECT_TYPE],
    },
    // 256
    TemplateText {
        ui_name: b"Skirmish Only_/Build/Build structure on flank perimeter.",
        internal_name: b"SKIRMISH_BUILD_STRUCTURE_FLANK",
        num_ui_strings: 2,
        ui_strings: &[
            b"Build one additional ",
            b", on the flank.",
        ],
        num_parameters: 1,
        parameters: &[P::OBJECT_TYPE],
    },
    // 257
    TemplateText {
        ui_name: b"Skirmish_/ Team attacks nearest group matching value comparison.",
        internal_name: b"SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" attacks nearest group worth ",
            b" ",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::COMPARISON],
    },
    // 258
    TemplateText {
        ui_name: b"Skirmish_/ Team performs command ability on most valuable object.",
        internal_name: b"SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT",
        num_ui_strings: 5,
        ui_strings: &[
            b" ",
            b" performs ",
            b" on most expensive object within ",
            b" ",
            b" (true = all valid sources, false = first valid source).",
        ],
        num_parameters: 4,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES, P::REAL, P::BOOLEAN],
    },
    // 259
    TemplateText {
        ui_name: b"Skirmish_/ Delay a sequential script until the specified command ability is ready - all.",
        internal_name: b"SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_ALL",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" 's ",
            b" all wait until ",
            b" is ready.",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 260
    TemplateText {
        ui_name: b"Skirmish_/ Delay a sequential script until the specified command ability is ready - partial.",
        internal_name: b"SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_PARTIAL",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" 's ",
            b" wait until at least one member is ",
            b" ready.",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 261
    TemplateText {
        ui_name: b"Team_/ Set to continue current action for some number of frames.",
        internal_name: b"TEAM_SPIN_FOR_FRAMECOUNT",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" continue their current action for at least ",
            b" frames.",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM],
    },
    // 262
    TemplateText {
        ui_name: b"Team_/ Use command ability -- all -- named enemy",
        internal_name: b"TEAM_ALL_USE_COMMANDBUTTON_ON_NAMED",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" use ",
            b"  on ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES, P::UNIT],
    },
    // 263
    TemplateText {
        ui_name: b"Team_/ Use command ability -- all -- nearest enemy unit",
        internal_name: b"TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_UNIT",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" use ",
            b"  on nearest enemy unit.",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 264
    TemplateText {
        ui_name: b"Team_/ Use command ability -- all -- nearest enemy garrisoned building.",
        internal_name: b"TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_GARRISONED_BUILDING",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" use ",
            b"  on nearest enemy garrisoned building.",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 265
    TemplateText {
        ui_name: b"Team_/ Use command ability -- all -- nearest enemy object with kind of.",
        internal_name: b"TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_KINDOF",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" use ",
            b"  on nearest enemy with ",
            b"",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES, P::KIND_OF_PARAM],
    },
    // 266
    TemplateText {
        ui_name: b"Team_/ Use command ability -- all -- nearest enemy building.",
        internal_name: b"TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" use ",
            b"  on nearest enemy building.",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 267
    TemplateText {
        ui_name: b"Team_/ Use command ability -- all -- nearest enemy building kindof.",
        internal_name: b"TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING_CLASS",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" use ",
            b"  on nearest enemy building with ",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES, P::KIND_OF_PARAM],
    },
    // 268
    TemplateText {
        ui_name: b"Team_/ Use command ability -- all -- nearest object type.",
        internal_name: b"TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_OBJECTTYPE",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" use ",
            b" on nearest object of type ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES, P::OBJECT_TYPE],
    },
    // 269
    TemplateText {
        ui_name: b"Team_/ Use command ability -- partial -- self.",
        internal_name: b"TEAM_PARTIAL_USE_COMMANDBUTTON",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b"% of ",
            b" perform ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::REAL, P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 270
    TemplateText {
        ui_name: b"Team_/ Capture unowned faction unit -- nearest.",
        internal_name: b"TEAM_CAPTURE_NEAREST_UNOWNED_FACTION_UNIT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" capture the nearest unowned faction unit.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 271
    TemplateText {
        ui_name: b"Player_/ Create team from all captured units.",
        internal_name: b"PLAYER_CREATE_TEAM_FROM_CAPTURED_UNITS",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" creates a new ",
            b" from units it has captured. (There's nothing quite like being assaulted by your own captured units!)",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::TEAM],
    },
    // 272
    TemplateText {
        ui_name: b"Player_/Experience/Add or Subtract Skill Points.",
        internal_name: b"PLAYER_ADD_SKILLPOINTS",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" is given ",
            b" Skill Points.",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 273
    TemplateText {
        ui_name: b"Player_/Experience/Add or Subtract Rank Levels.",
        internal_name: b"PLAYER_ADD_RANKLEVEL",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" is given ",
            b" Rank Levels.",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 274
    TemplateText {
        ui_name: b"Player_/Experience/Set Rank Level.",
        internal_name: b"PLAYER_SET_RANKLEVEL",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" is given a Rank Level of ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 275
    TemplateText {
        ui_name: b"Map_/Experience/Set Rank Level Limit for current Map.",
        internal_name: b"PLAYER_SET_RANKLEVELLIMIT",
        num_ui_strings: 2,
        ui_strings: &[
            b"The current map is given a Rank Level Limit of ",
            b".",
        ],
        num_parameters: 1,
        parameters: &[],
    },
    // 276
    TemplateText {
        ui_name: b"Player_/Science/Grant a Science to a given Player (ignoring prerequisites).",
        internal_name: b"PLAYER_GRANT_SCIENCE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" is granted ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SCIENCE],
    },
    // 277
    TemplateText {
        ui_name: b"Player_/Science/Player attempts to purchase a Science.",
        internal_name: b"PLAYER_PURCHASE_SCIENCE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" attempts to purchase Science ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::SCIENCE],
    },
    // 278
    TemplateText {
        ui_name: b"Team_/Hunt/Set to hunt using commandbutton ability.",
        internal_name: b"TEAM_HUNT_WITH_COMMAND_BUTTON",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" begins hunting using ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::COMMANDBUTTON_ALL_ABILITIES],
    },
    // 279
    TemplateText {
        ui_name: b"Team_/ Delay a sequential script until the team is no longer contained - all",
        internal_name: b"TEAM_WAIT_FOR_NOT_CONTAINED_ALL",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" all delay until they are no longer contained.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 280
    TemplateText {
        ui_name: b"Team_/ Delay a sequential script until the team is no longer contained - partial",
        internal_name: b"TEAM_WAIT_FOR_NOT_CONTAINED_PARTIAL",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" delay until at least one of them is no longer contained.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 281
    TemplateText {
        ui_name: b"Team_/Move/Set to EXACTLY follow a waypoint path.",
        internal_name: b"TEAM_FOLLOW_WAYPOINTS_EXACT",
        num_ui_strings: 3,
        ui_strings: &[
            b"Have ",
            b" EXACTLY follow ",
            b" , as a team is ",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::WAYPOINT_PATH, P::BOOLEAN],
    },
    // 282
    TemplateText {
        ui_name: b"Unit_/Move/Set a specific unit to EXACTLY follow a waypoint path.",
        internal_name: b"NAMED_FOLLOW_WAYPOINTS_EXACT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" EXACTLY follows waypoints, beginning at ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::WAYPOINT_PATH],
    },
    // 283
    TemplateText {
        ui_name: b"Team_/ Set emoticon for duration (-1.0 permanent, otherwise duration in sec).",
        internal_name: b"TEAM_SET_EMOTICON",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" use ",
            b" emoticon for ",
            b" seconds.",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::EMOTICON, P::REAL],
    },
    // 284
    TemplateText {
        ui_name: b"Unit_/ Set emoticon for duration (-1.0 permanent, otherwise duration in sec).",
        internal_name: b"NAMED_SET_EMOTICON",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" use ",
            b" emoticon for ",
            b" seconds.",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::EMOTICON, P::REAL],
    },
    // 285
    TemplateText {
        ui_name: b"Player_/AI/AI player build near a supply source.",
        internal_name: b"AI_PLAYER_BUILD_SUPPLY_CENTER",
        num_ui_strings: 4,
        ui_strings: &[
            b"Have AI ",
            b" build a ",
            b" near a supply src with at least ",
            b" available resources.",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::OBJECT_TYPE],
    },
    // 286
    TemplateText {
        ui_name: b"Player_/AI/AI player build an upgrade.",
        internal_name: b"AI_PLAYER_BUILD_UPGRADE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Have AI ",
            b" build this upgrade: ",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::UPGRADE],
    },
    // 287
    TemplateText {
        ui_name: b"Scripting_/ Object Type List -- Add Object Type.",
        internal_name: b"OBJECTLIST_ADDOBJECTTYPE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" : add ",
        ],
        num_parameters: 2,
        parameters: &[P::OBJECT_TYPE_LIST, P::OBJECT_TYPE],
    },
    // 288
    TemplateText {
        ui_name: b"Scripting_/ Object Type List -- Remove Object Type.",
        internal_name: b"OBJECTLIST_REMOVEOBJECTTYPE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" : remove ",
        ],
        num_parameters: 2,
        parameters: &[P::OBJECT_TYPE_LIST, P::OBJECT_TYPE],
    },
    // 289
    TemplateText {
        ui_name: b"Map_/ Reveal map at waypoint -- permanently.",
        internal_name: b"MAP_REVEAL_PERMANENTLY_AT_WAYPOINT",
        num_ui_strings: 5,
        ui_strings: &[
            b"The map is permanently revealed at ",
            b" with a radius of ",
            b" for ",
            b". (Afterwards referred to as ",
            b").",
        ],
        num_parameters: 4,
        parameters: &[P::WAYPOINT, P::REAL, P::SIDE, P::REVEALNAME],
    },
    // 290
    TemplateText {
        ui_name: b"Map_/ Reveal map at waypoint -- undo permanently.",
        internal_name: b"MAP_UNDO_REVEAL_PERMANENTLY_AT_WAYPOINT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is undone.",
        ],
        num_parameters: 1,
        parameters: &[P::REVEALNAME],
    },
    // 291
    TemplateText {
        ui_name: b"Unit_/Status/Stealth set enabled or disabled.",
        internal_name: b"NAMED_SET_STEALTH_ENABLED",
        num_ui_strings: 3,
        ui_strings: &[
            b"Set ",
            b" stealth ability to ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::BOOLEAN],
    },
    // 292
    TemplateText {
        ui_name: b"Team_/Status/Stealth set enabled or disabled.",
        internal_name: b"TEAM_SET_STEALTH_ENABLED",
        num_ui_strings: 3,
        ui_strings: &[
            b"Set ",
            b" stealth ability to ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::BOOLEAN],
    },
    // 293
    TemplateText {
        ui_name: b"Scripting_/ Enable or Disable EVA.",
        internal_name: b"EVA_SET_ENABLED_DISABLED",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set EVA to be enabled ",
            b" (False to disable.)",
        ],
        num_parameters: 1,
        parameters: &[P::BOOLEAN],
    },
    // 294
    TemplateText {
        ui_name: b"Scripting_/ Enable or Disable Occlusion (Drawing Behind Buildings).",
        internal_name: b"OPTIONS_SET_OCCLUSION_MODE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set Occlusion to be enabled ",
            b" (False to disable.)",
        ],
        num_parameters: 1,
        parameters: &[P::BOOLEAN],
    },
    // 295
    TemplateText {
        ui_name: b"Multiplayer_/ Announce local defeat.",
        internal_name: b"LOCALDEFEAT",
        num_ui_strings: 1,
        ui_strings: &[
            b"Show 'Game Over' window",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 296
    TemplateText {
        ui_name: b"Scripting_/ Enable or Disable Draw-icon UI.",
        internal_name: b"OPTIONS_SET_DRAWICON_UI_MODE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set Draw-icon UI to be enabled ",
            b" (False to disable.)",
        ],
        num_parameters: 1,
        parameters: &[P::BOOLEAN],
    },
    // 297
    TemplateText {
        ui_name: b"Scripting_/ Enable or Disable Particle Cap.",
        internal_name: b"OPTIONS_SET_PARTICLE_CAP_MODE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set Particle Cap to be enabled ",
            b" (False to disable.)",
        ],
        num_parameters: 1,
        parameters: &[P::BOOLEAN],
    },
    // 298
    TemplateText {
        ui_name: b"Player_/Science/Set science availability.",
        internal_name: b"PLAYER_SCIENCE_AVAILABILITY",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" set ",
            b" availability to ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::SCIENCE, P::SCIENCE_AVAILABILITY],
    },
    // 299
    TemplateText {
        ui_name: b"Unit_/ Affect flags set on object panel.",
        internal_name: b"UNIT_AFFECT_OBJECT_PANEL_FLAGS",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" changes the value of flag ",
            b" to ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::OBJECT_PANEL_FLAG, P::BOOLEAN],
    },
    // 300
    TemplateText {
        ui_name: b"Team_/ Affect flags set on object panel - all.",
        internal_name: b"TEAM_AFFECT_OBJECT_PANEL_FLAGS",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" change the value of flag ",
            b" to ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::TEAM, P::OBJECT_PANEL_FLAG, P::BOOLEAN],
    },
    // 301
    TemplateText {
        ui_name: b"Player_/ Set the skillset for a computer player.",
        internal_name: b"PLAYER_SELECT_SKILLSET",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" uses skillset number ",
            b" (1-5).",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE],
    },
    // 302
    TemplateText {
        ui_name: b"Scripting_/ Hulk set override lifetime.",
        internal_name: b"SCRIPTING_OVERRIDE_HULK_LIFETIME",
        num_ui_strings: 2,
        ui_strings: &[
            b"Override hulk lifetime to ",
            b" seconds. Negative value reverts to normal behavior.",
        ],
        num_parameters: 1,
        parameters: &[P::REAL],
    },
    // 303
    TemplateText {
        ui_name: b"Unit_/ Set unit to face another unit.",
        internal_name: b"NAMED_FACE_NAMED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begin facing ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::UNIT],
    },
    // 304
    TemplateText {
        ui_name: b"Unit_/ Set unit to face a waypoint.",
        internal_name: b"NAMED_FACE_WAYPOINT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begin facing ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::WAYPOINT],
    },
    // 305
    TemplateText {
        ui_name: b"Team_/ Set team to face another unit.",
        internal_name: b"TEAM_FACE_NAMED",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begin facing ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::UNIT],
    },
    // 306
    TemplateText {
        ui_name: b"Team_/ Set team to face a waypoint.",
        internal_name: b"TEAM_FACE_WAYPOINT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" begin facing ",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM, P::WAYPOINT],
    },
    // 307
    TemplateText {
        ui_name: b"Scripting_/ Remove a command button from an object type.",
        internal_name: b"COMMANDBAR_REMOVE_BUTTON_OBJECTTYPE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" is removed from all objects of type ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::COMMAND_BUTTON, P::OBJECT_TYPE],
    },
    // 308
    TemplateText {
        ui_name: b"Scripting_/ Add a command button to an object type.",
        internal_name: b"COMMANDBAR_ADD_BUTTON_OBJECTTYPE_SLOT",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" is added to all objects of type ",
            b" in slot number ",
            b" (1-12).",
        ],
        num_parameters: 3,
        parameters: &[P::COMMAND_BUTTON, P::OBJECT_TYPE],
    },
    // 309
    TemplateText {
        ui_name: b"Unit_/ Spawn -- named unit on a team at a position with an orientation.",
        internal_name: b"UNIT_SPAWN_NAMED_LOCATION_ORIENTATION",
        num_ui_strings: 6,
        ui_strings: &[
            b"Spawn ",
            b" of type ",
            b" on team ",
            b" at position (",
            b"), rotated ",
            b" .",
        ],
        num_parameters: 5,
        parameters: &[P::UNIT, P::OBJECT_TYPE, P::TEAM, P::COORD3D, P::ANGLE],
    },
    // 310
    TemplateText {
        ui_name: b"Player_/ Change the modifier to generals experience that a player receives.",
        internal_name: b"PLAYER_AFFECT_RECEIVING_EXPERIENCE",
        num_ui_strings: 3,
        ui_strings: &[
            b" ",
            b" gains experience at ",
            b" times the usual rate (0.0 for no gain, 1.0 for normal rate)",
        ],
        num_parameters: 2,
        parameters: &[P::SIDE, P::REAL],
    },
    // 311
    TemplateText {
        ui_name: b"Player_/Score/Exclude this player from the score screen.",
        internal_name: b"PLAYER_EXCLUDE_FROM_SCORE_SCREEN",
        num_ui_strings: 2,
        ui_strings: &[
            b"Exclude ",
            b" from the score screen.",
        ],
        num_parameters: 1,
        parameters: &[P::SIDE],
    },
    // 312
    TemplateText {
        ui_name: b"Team_/Guard/Set to guard a supply source.",
        internal_name: b"TEAM_GUARD_SUPPLY_CENTER",
        num_ui_strings: 3,
        ui_strings: &[
            b"Have Team ",
            b" guard attacked or closest supply src with at least ",
            b" available resources",
        ],
        num_parameters: 2,
        parameters: &[P::TEAM],
    },
    // 313
    TemplateText {
        ui_name: b"Player_/Score/Turn on scoring.",
        internal_name: b"ENABLE_SCORING",
        num_ui_strings: 1,
        ui_strings: &[
            b"Turn on scoring.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 314
    TemplateText {
        ui_name: b"Player_/Score/Turn off scoring.",
        internal_name: b"DISABLE_SCORING",
        num_ui_strings: 1,
        ui_strings: &[
            b"Turn off scoring.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 315
    TemplateText {
        ui_name: b"Multimedia_/ Set the current sound volume.",
        internal_name: b"SOUND_SET_VOLUME",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set the desired sound volume to ",
            b"%. (0-100)",
        ],
        num_parameters: 1,
        parameters: &[P::REAL],
    },
    // 316
    TemplateText {
        ui_name: b"Multimedia_/ Set the current speech volume.",
        internal_name: b"SPEECH_SET_VOLUME",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set the desired speech volume to ",
            b"%. (0-100)",
        ],
        num_parameters: 1,
        parameters: &[P::REAL],
    },
    // 317
    TemplateText {
        ui_name: b"Map_/Shroud or Reveal/Border Shroud is turned off.",
        internal_name: b"DISABLE_BORDER_SHROUD",
        num_ui_strings: 1,
        ui_strings: &[
            b"Shroud off the map edges is turned off.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 318
    TemplateText {
        ui_name: b"Map_/Shroud or Reveal/Border Shroud is turned on.",
        internal_name: b"ENABLE_BORDER_SHROUD",
        num_ui_strings: 1,
        ui_strings: &[
            b"Shroud off the map edges is turned on.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 319
    TemplateText {
        ui_name: b"Map_/ Adjust Object Bonuses based on difficulty.",
        internal_name: b"OBJECT_ALLOW_BONUSES",
        num_ui_strings: 2,
        ui_strings: &[
            b"Enable Object Bonuses based on difficulty ",
            b" (true to enable, false to disable).",
        ],
        num_parameters: 1,
        parameters: &[P::BOOLEAN],
    },
    // 320
    TemplateText {
        ui_name: b"Multimedia_/ Sound Events -- remove all disabled.",
        internal_name: b"SOUND_REMOVE_ALL_DISABLED",
        num_ui_strings: 1,
        ui_strings: &[
            b"Remove all disabled sound events.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 321
    TemplateText {
        ui_name: b"Multimedia_/ Sound Events -- remove type.",
        internal_name: b"SOUND_REMOVE_TYPE",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" is removed.",
        ],
        num_parameters: 1,
        parameters: &[P::SOUND],
    },
    // 322
    TemplateText {
        ui_name: b"Team_/ Set to guard - from inside tunnel network.",
        internal_name: b"TEAM_GUARD_IN_TUNNEL_NETWORK",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" Enter and guard from tunnel network.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 323
    TemplateText {
        ui_name: b"User_/ Announce quick win",
        internal_name: b"QUICKVICTORY",
        num_ui_strings: 1,
        ui_strings: &[
            b"End game in victory immediately.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 324
    TemplateText {
        ui_name: b"Map_/Environment/Infantry Lighting - Set.",
        internal_name: b"SET_INFANTRY_LIGHTING_OVERRIDE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Set lighting percent on infantry to ",
            b" (0.0==min, 1.0==normal day, 2.0==max (which is normal night).)",
        ],
        num_parameters: 1,
        parameters: &[P::REAL],
    },
    // 325
    TemplateText {
        ui_name: b"Map_/Environment/Infantry Lighting - Reset.",
        internal_name: b"RESET_INFANTRY_LIGHTING_OVERRIDE",
        num_ui_strings: 1,
        ui_strings: &[
            b"Reset infantry lighting to the normal setting. 1.0 for the two day states, 2.0 for the two night states. (Look in GamesData.ini)",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 326
    TemplateText {
        ui_name: b"Team_/Damage or Remove/Delete a team, but ignore dead guys.",
        internal_name: b"TEAM_DELETE_LIVING",
        num_ui_strings: 2,
        ui_strings: &[
            b"Each living member of team ",
            b" is removed from the world.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 327
    TemplateText {
        ui_name: b"Map_/ Resize view guardband.",
        internal_name: b"RESIZE_VIEW_GUARDBAND",
        num_ui_strings: 3,
        ui_strings: &[
            b"Allow bigger objects to be perceived as onscreen near the edge (",
            b",",
            b") Width then height, in world units.",
        ],
        num_parameters: 2,
        parameters: &[P::REAL, P::REAL],
    },
    // 328
    TemplateText {
        ui_name: b"Scripting_/ Delete all unmanned (sniped) vehicles.",
        internal_name: b"DELETE_ALL_UNMANNED",
        num_ui_strings: 1,
        ui_strings: &[
            b"Delete all unmanned (sniped) vehicles.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 329
    TemplateText {
        ui_name: b"Map_/ Force ChooseVictim to ignore game difficulty and always use Normal setting.",
        internal_name: b"CHOOSE_VICTIM_ALWAYS_USES_NORMAL",
        num_ui_strings: 2,
        ui_strings: &[
            b"Force ChooseVictim to ignore game difficulty and always use Normal setting ",
            b" (true to enable, false to disable).",
        ],
        num_parameters: 1,
        parameters: &[P::BOOLEAN],
    },
    // 330
    TemplateText {
        ui_name: b"Camera_/Enable 3DSMax Camera Animation Playback mode.",
        internal_name: b"CAMERA_ENABLE_SLAVE_MODE",
        num_ui_strings: 2,
        ui_strings: &[
            b"Enable 3DSMax Camera playback of animation with thing name ",
            b" containing bone name ",
        ],
        num_parameters: 2,
        parameters: &[P::TEXT_STRING, P::TEXT_STRING],
    },
    // 331
    TemplateText {
        ui_name: b"Camera_/Disable 3DSMax Camera Animation Playback mode.",
        internal_name: b"CAMERA_DISABLE_SLAVE_MODE",
        num_ui_strings: 1,
        ui_strings: &[
            b"Disable camera playback mode.",
        ],
        num_parameters: 0,
        parameters: &[],
    },
    // 332
    TemplateText {
        ui_name: b"Camera_/Add Camera Shaker Effect at.",
        internal_name: b"CAMERA_ADD_SHAKER_AT",
        num_ui_strings: 4,
        ui_strings: &[
            b"Add Camera Shaker Effect at waypoint ",
            b" with Amplitude ",
            b" Duration (seconds) ",
            b" Radius.",
        ],
        num_parameters: 4,
        parameters: &[P::WAYPOINT, P::REAL, P::REAL, P::REAL],
    },
    // 333
    TemplateText {
        ui_name: b"Unit/ Set a train to stay at a station. TRUE = stay. FALSE = go-ahead.",
        internal_name: b"SET_TRAIN_HELD",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" sets its held status to ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::BOOLEAN],
    },
    // 334
    TemplateText {
        ui_name: b"Unit/ Set which side of a container (likely a train) you want the riders to exit on.",
        internal_name: b"NAMED_SET_EVAC_LEFT_OR_RIGHT",
        num_ui_strings: 2,
        ui_strings: &[
            b" ",
            b" will exit its riders on its ",
        ],
        num_parameters: 2,
        parameters: &[P::UNIT, P::LEFT_OR_RIGHT],
    },
    // 335
    TemplateText {
        ui_name: b"Multimedia_/Sound Effect/Enable object's ambient sound",
        internal_name: b"ENABLE_OBJECT_SOUND",
        num_ui_strings: 2,
        ui_strings: &[
            b"Enable (or trigger) ",
            b"'s ambient sound.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 336
    TemplateText {
        ui_name: b"Multimedia_/Sound Effect/Disable object's ambient sound",
        internal_name: b"DISABLE_OBJECT_SOUND",
        num_ui_strings: 2,
        ui_strings: &[
            b"Disable ",
            b"'s ambient sound.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 337
    TemplateText {
        ui_name: b"Unit_/ Use commandbutton ability using a waypoint path.",
        internal_name: b"NAMED_USE_COMMANDBUTTON_ABILITY_USING_WAYPOINT_PATH",
        num_ui_strings: 4,
        ui_strings: &[
            b" ",
            b" use ",
            b" to follow ",
            b" path.",
        ],
        num_parameters: 3,
        parameters: &[P::UNIT, P::COMMANDBUTTON_ABILITY, P::WAYPOINT_PATH],
    },
    // 338
    TemplateText {
        ui_name: b"Unit_/Status/Make unmanned.",
        internal_name: b"NAMED_SET_UNMANNED_STATUS",
        num_ui_strings: 2,
        ui_strings: &[
            b"Make ",
            b" unmanned.",
        ],
        num_parameters: 1,
        parameters: &[P::UNIT],
    },
    // 339
    TemplateText {
        ui_name: b"Team_/Status/Make unmanned.",
        internal_name: b"TEAM_SET_UNMANNED_STATUS",
        num_ui_strings: 2,
        ui_strings: &[
            b"Make ",
            b" unmanned.",
        ],
        num_parameters: 1,
        parameters: &[P::TEAM],
    },
    // 340
    TemplateText {
        ui_name: b"Unit_/Status/Add boobytrap.",
        internal_name: b"NAMED_SET_BOOBYTRAPPED",
        num_ui_strings: 3,
        ui_strings: &[
            b"Add boobytrap of type ",
            b" to ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::OBJECT_TYPE, P::UNIT],
    },
    // 341
    TemplateText {
        ui_name: b"Team_/Status/Add boobytrap.",
        internal_name: b"TEAM_SET_BOOBYTRAPPED",
        num_ui_strings: 3,
        ui_strings: &[
            b"Add boobytrap of type ",
            b" to team ",
            b".",
        ],
        num_parameters: 2,
        parameters: &[P::OBJECT_TYPE, P::TEAM],
    },
    // 342
    TemplateText {
        ui_name: b"Map/Environment/Show Weather.",
        internal_name: b"SHOW_WEATHER",
        num_ui_strings: 1,
        ui_strings: &[
            b"Show Weather = ",
        ],
        num_parameters: 1,
        parameters: &[P::BOOLEAN],
    },
    // 343
    TemplateText {
        ui_name: b"Player_/AI/AI player build nearest specified team.",
        internal_name: b"AI_PLAYER_BUILD_TYPE_NEAREST_TEAM",
        num_ui_strings: 4,
        ui_strings: &[
            b"Have AI ",
            b" build a ",
            b" nearest team ",
            b".",
        ],
        num_parameters: 3,
        parameters: &[P::SIDE, P::OBJECT_TYPE, P::TEAM],
    },
];
