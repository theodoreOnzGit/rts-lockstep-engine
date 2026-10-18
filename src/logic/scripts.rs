// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Scripts.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/Scripts.cpp
//   (and, for the struct it fills, GameLogic/ScriptEngine.h's TScriptListReadInfo)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Created: John Ahlquist, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The script data model of a map: [`ScriptList`] (one per player) holding
//! [`ScriptGroup`]s and [`Script`]s, each script an ORed list of
//! [`OrCondition`]s (each an AND list of [`Condition`]s) and two lists of
//! [`ScriptAction`]s (when the conditions hold, and when they do not), each
//! condition and action holding up to [`MAX_PARMS`] [`Parameter`]s; and
//! their parse from, and write to, the map's chunk file (the
//! `PlayerScriptsList` chunk, in the `SidesList` chunk of a map or on its
//! own in a `.scb` file). This is the data the script engine (layer 11)
//! runs; the engine is not here.
//!
//! # How the C++ shapes became Rust
//!
//! - **Linked lists are vectors.** `m_nextScript`, `m_nextGroup`,
//!   `m_nextOr`, `m_nextAndCondition` and `m_nextAction` chains become the
//!   `Vec` that holds them (`ScriptList::scripts`, `Script::conditions`,
//!   ...). A C++ function that works on "the head and everything after it"
//!   (`Condition::duplicate`, `OrCondition::duplicate`,
//!   `ScriptAction::duplicate`, the `Write*DataChunk` functions) is an
//!   associated function over a slice; one that works on "this node" is a
//!   method. Pointer arguments that name a list member
//!   (`deleteScript(pScr)`) are indices.
//! - **`m_parms` is `[Option<Parameter>; MAX_PARMS]`** with `m_numParms`
//!   kept separately and signed, because the parse leaves both in states a
//!   `Vec` would hide: slots after `m_numParms` that are stale or empty, a
//!   `m_numParms` that is negative, and (version 1 files) a count set to 3
//!   with empty slots below it.
//! - **Enumerations are `i32`**: the file's `readInt` is cast straight to
//!   `ParameterType`, `ConditionType` and `ScriptActionType`, and values
//!   past `NUM_ITEMS` are stored and written back unchanged (the template
//!   lookup clamps them, as `ScriptEngine::getConditionTemplate` does).
//! - **Process-wide state is explicit** ([`ScriptsState`]): the group
//!   numbering counter `ScriptList::m_curId`, and the lists the last parse
//!   left for [`ScriptList::get_read_scripts`] (`s_readLists`,
//!   `s_numInReadList`). `TheScriptEngine`'s template tables are a value
//!   ([`ScriptTemplates`]); the name keys come from the caller's
//!   [`NameKeyGenerator`] as in [`crate::formats::data_chunk`].
//! - **Parsers** are generic over the caller's context type `C`, which
//!   implements [`ScriptsContext`]; the C++ passes the object being filled
//!   as `userData` of each nested `parse`, here the object is the last one
//!   added to the list being read (`ScriptsState`'s `read_info` and
//!   `script_stack`), which is what each `userData` always was.
//!
//! # Cut points (what is not here)
//!
//! - **The script engine.** `ScriptEngine::init` builds the condition and
//!   action templates (their internal names, parameter types and editor
//!   text); this crate takes them as [`ScriptTemplates`] (layer 11 will
//!   produce them). Nothing here evaluates or executes a script.
//! - **Editor text:** `getUiText`, `getUiStrings` (all four classes),
//!   `Surfaces`, `ShakeIntensities`: strings for the WorldBuilder dialogs.
//! - **Save games:** the `Snapshot` methods (`crc`, `xfer`,
//!   `loadPostProcess`) of `ScriptList`, `ScriptGroup` and `Script` belong
//!   to the save/load layer.
//! - `SignalUIInteraction` and `TheShellHookNames` (the shell asks the
//!   engine to raise a flag): UI to engine, layer 11.
//!
//! # Where upstream is undefined
//!
//! The Rust returns [`MapUndefined`] instead (the tests count these and do
//! not give them to the C++): a `readInt` cast to an enumeration outside
//! its range (`ParameterType` 0..=63, `ConditionType` 0..=127,
//! `ScriptActionType` 0..=511); more than [`MAX_PARMS`] parameters, or the
//! version 1 repair indexing `m_parms` out of its bounds; a parameter slot
//! that is empty where a parameter is dereferenced (writing, duplicating a
//! condition other than the first of its list); a script whose children
//! failed to parse (`ParseScript` returns `NULL` and its caller stores it);
//! an `OBJECT_TYPE` string of 256 or more characters starting with
//! `Fundamentalist` (`strcpy` into `char[256]`); a `KIND_OF_PARAM` with an
//! empty string and an index outside `0..=KINDOF_COUNT` (`kindofNames[]`
//! unchecked); a pointer search for a node not in the list; a loop that
//! never ends (`validateSides` restarting its team-name check after
//! `removeTeam` refused to remove the last team).
//!
//! # Upstream quirks that are kept
//!
//! - A condition or action whose `m_internalNameKey` matches no template
//!   becomes `CONDITION_FALSE` / `NO_OP`, but its parameters are read by
//!   the count in the file and compared with the **last** template's count
//!   (the loop that searched for the name leaves `ct` on the last one).
//! - `PlayerScriptsList` is written with version 5 (`K_SCRIPTS_DATA_VERSION_1`
//!   follows `K_SCRIPT_CONDITION_VERSION_4 = 4` in an enum).
//! - The `KIND_OF_PARAM` read of `MISSILE`: the retry loop compares the
//!   string with itself, so it stops at index 0, and the outer loop goes
//!   on from index 1 until it meets `SMALL_MISSILE`'s real index.
//! - `WriteParameter` replaces the string of a `KIND_OF_PARAM` parameter
//!   with the name of its index, so writing changes the model.
//! - A failed parse (`ParseScriptsDataChunk` false) leaves the previous
//!   read lists' count in place with the lists gone.

use crate::common::base_type::Coord3D;
use crate::common::kind_of::KindOfMaskType;
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::formats::data_chunk::{ChunkError, ChunkResult, ChunkUndefined, DataChunkInfo, DataChunkInput};
use crate::formats::data_chunk_output::DataChunkOutput;

/// `MAX_PARMS`: parameters in a condition or an action.
pub const MAX_PARMS: usize = 12;
/// `MAX_PLAYER_COUNT` (GameCommon.h).
pub const MAX_PLAYER_COUNT: usize = 16;
/// `AT_END`: the position that always means the end of a list.
pub const AT_END: i32 = 0x00FF_FFFF;
/// `THIS_TEAM`.
pub const THIS_TEAM: &[u8] = b"<This Team>";
/// `THIS_PLAYER`.
pub const THIS_PLAYER: &[u8] = b"<This Player>";
/// `ERROR_BUG` (Common/Errors.h).
pub const ERROR_BUG: u32 = 0xdead_0001;

/// `K_SCRIPT_LIST_DATA_VERSION_1` and the other chunk versions of the enum
/// at the top of Scripts.cpp.
pub mod version {
    /// `K_SCRIPT_LIST_DATA_VERSION_1`.
    pub const SCRIPT_LIST_1: u16 = 1;
    /// `K_SCRIPT_GROUP_DATA_VERSION_2` (version 1 lacks `m_isGroupSubroutine`).
    pub const SCRIPT_GROUP_2: u16 = 2;
    /// `K_SCRIPT_DATA_VERSION_2` (version 1 lacks the delay).
    pub const SCRIPT_2: u16 = 2;
    /// `K_SCRIPT_OR_CONDITION_DATA_VERSION_1`.
    pub const SCRIPT_OR_CONDITION_1: u16 = 1;
    /// `K_SCRIPT_ACTION_VERSION_2` (has the internal name key).
    pub const SCRIPT_ACTION_2: u16 = 2;
    /// `K_SCRIPT_CONDITION_VERSION_2` (adds the surfaces-allowed parameter).
    pub const SCRIPT_CONDITION_2: u16 = 2;
    /// `K_SCRIPT_CONDITION_VERSION_4` (has the internal name key).
    pub const SCRIPT_CONDITION_4: u16 = 4;
    /// `K_SCRIPTS_DATA_VERSION_1`: the implicit successor of 4.
    pub const SCRIPTS_1: u16 = 5;
}

/// `Parameter::ParameterType`.
pub mod parameter_type {
    /// `INT`: an `Int`.
    pub const INT: i32 = 0;
    /// `REAL`.
    pub const REAL: i32 = 1;
    /// `SCRIPT`: a script's name.
    pub const SCRIPT: i32 = 2;
    /// `TEAM`.
    pub const TEAM: i32 = 3;
    /// `COUNTER`.
    pub const COUNTER: i32 = 4;
    /// `FLAG`.
    pub const FLAG: i32 = 5;
    /// `COMPARISON`.
    pub const COMPARISON: i32 = 6;
    /// `WAYPOINT`.
    pub const WAYPOINT: i32 = 7;
    /// `BOOLEAN`.
    pub const BOOLEAN: i32 = 8;
    /// `TRIGGER_AREA`.
    pub const TRIGGER_AREA: i32 = 9;
    /// `TEXT_STRING`.
    pub const TEXT_STRING: i32 = 10;
    /// `SIDE`: a player.
    pub const SIDE: i32 = 11;
    /// `SOUND`.
    pub const SOUND: i32 = 12;
    /// `SCRIPT_SUBROUTINE`.
    pub const SCRIPT_SUBROUTINE: i32 = 13;
    /// `UNIT`.
    pub const UNIT: i32 = 14;
    /// `OBJECT_TYPE`: a thing template's name.
    pub const OBJECT_TYPE: i32 = 15;
    /// `COORD3D`: the only type that stores a position.
    pub const COORD3D: i32 = 16;
    /// `ANGLE`.
    pub const ANGLE: i32 = 17;
    /// `TEAM_STATE`.
    pub const TEAM_STATE: i32 = 18;
    /// `RELATION`.
    pub const RELATION: i32 = 19;
    /// `AI_MOOD`.
    pub const AI_MOOD: i32 = 20;
    /// `DIALOG`.
    pub const DIALOG: i32 = 21;
    /// `MUSIC`.
    pub const MUSIC: i32 = 22;
    /// `MOVIE`.
    pub const MOVIE: i32 = 23;
    /// `WAYPOINT_PATH`.
    pub const WAYPOINT_PATH: i32 = 24;
    /// `LOCALIZED_TEXT`.
    pub const LOCALIZED_TEXT: i32 = 25;
    /// `BRIDGE`.
    pub const BRIDGE: i32 = 26;
    /// `KIND_OF_PARAM`: an index into the `KindOf` names, read from a name.
    pub const KIND_OF_PARAM: i32 = 27;
    /// `ATTACK_PRIORITY_SET`.
    pub const ATTACK_PRIORITY_SET: i32 = 28;
    /// `RADAR_EVENT_TYPE`.
    pub const RADAR_EVENT_TYPE: i32 = 29;
    /// `SPECIAL_POWER`.
    pub const SPECIAL_POWER: i32 = 30;
    /// `SCIENCE`.
    pub const SCIENCE: i32 = 31;
    /// `UPGRADE`.
    pub const UPGRADE: i32 = 32;
    /// `COMMANDBUTTON_ABILITY`.
    pub const COMMANDBUTTON_ABILITY: i32 = 33;
    /// `BOUNDARY`.
    pub const BOUNDARY: i32 = 34;
    /// `BUILDABLE`.
    pub const BUILDABLE: i32 = 35;
    /// `SURFACES_ALLOWED`.
    pub const SURFACES_ALLOWED: i32 = 36;
    /// `SHAKE_INTENSITY`.
    pub const SHAKE_INTENSITY: i32 = 37;
    /// `COMMAND_BUTTON`.
    pub const COMMAND_BUTTON: i32 = 38;
    /// `FONT_NAME`.
    pub const FONT_NAME: i32 = 39;
    /// `OBJECT_STATUS`: read from a name into a status mask.
    pub const OBJECT_STATUS: i32 = 40;
    /// `COMMANDBUTTON_ALL_ABILITIES`.
    pub const COMMANDBUTTON_ALL_ABILITIES: i32 = 41;
    /// `SKIRMISH_WAYPOINT_PATH`.
    pub const SKIRMISH_WAYPOINT_PATH: i32 = 42;
    /// `COLOR`.
    pub const COLOR: i32 = 43;
    /// `EMOTICON`.
    pub const EMOTICON: i32 = 44;
    /// `OBJECT_PANEL_FLAG`.
    pub const OBJECT_PANEL_FLAG: i32 = 45;
    /// `FACTION_NAME`.
    pub const FACTION_NAME: i32 = 46;
    /// `OBJECT_TYPE_LIST`.
    pub const OBJECT_TYPE_LIST: i32 = 47;
    /// `REVEALNAME`.
    pub const REVEALNAME: i32 = 48;
    /// `SCIENCE_AVAILABILITY`.
    pub const SCIENCE_AVAILABILITY: i32 = 49;
    /// `LEFT_OR_RIGHT`.
    pub const LEFT_OR_RIGHT: i32 = 50;
    /// `PERCENT`.
    pub const PERCENT: i32 = 51;
    /// `NUM_ITEMS`.
    pub const NUM_ITEMS: i32 = 52;
}

/// The `Condition::ConditionType` values this module names (the others are
/// the engine's); `NUM_ITEMS` sizes the template table.
pub mod condition_type {
    /// `CONDITION_FALSE`.
    pub const CONDITION_FALSE: i32 = 0;
    /// `TEAM_INSIDE_AREA_PARTIALLY`.
    pub const TEAM_INSIDE_AREA_PARTIALLY: i32 = 7;
    /// `TEAM_INSIDE_AREA_ENTIRELY`.
    pub const TEAM_INSIDE_AREA_ENTIRELY: i32 = 17;
    /// `TEAM_OUTSIDE_AREA_ENTIRELY`.
    pub const TEAM_OUTSIDE_AREA_ENTIRELY: i32 = 18;
    /// `TEAM_ENTERED_AREA_ENTIRELY`.
    pub const TEAM_ENTERED_AREA_ENTIRELY: i32 = 39;
    /// `TEAM_ENTERED_AREA_PARTIALLY`.
    pub const TEAM_ENTERED_AREA_PARTIALLY: i32 = 40;
    /// `TEAM_EXITED_AREA_ENTIRELY`.
    pub const TEAM_EXITED_AREA_ENTIRELY: i32 = 41;
    /// `TEAM_EXITED_AREA_PARTIALLY`.
    pub const TEAM_EXITED_AREA_PARTIALLY: i32 = 42;
    /// `SKIRMISH_SPECIAL_POWER_READY`.
    pub const SKIRMISH_SPECIAL_POWER_READY: i32 = 84;
    /// `NUM_ITEMS`.
    pub const NUM_ITEMS: i32 = 109;
}

/// The `ScriptAction::ScriptActionType` values this module names;
/// `NUM_ITEMS` sizes the template table.
pub mod action_type {
    /// `NO_OP`.
    pub const NO_OP: i32 = 5;
    /// `MOVE_CAMERA_TO`.
    pub const MOVE_CAMERA_TO: i32 = 14;
    /// `MOVE_CAMERA_ALONG_WAYPOINT_PATH`.
    pub const MOVE_CAMERA_ALONG_WAYPOINT_PATH: i32 = 17;
    /// `ROTATE_CAMERA`.
    pub const ROTATE_CAMERA: i32 = 18;
    /// `RESET_CAMERA`.
    pub const RESET_CAMERA: i32 = 19;
    /// `CAMERA_MOD_SET_FINAL_ZOOM`.
    pub const CAMERA_MOD_SET_FINAL_ZOOM: i32 = 26;
    /// `CAMERA_MOD_SET_FINAL_PITCH`.
    pub const CAMERA_MOD_SET_FINAL_PITCH: i32 = 27;
    /// `TEAM_FOLLOW_WAYPOINTS`.
    pub const TEAM_FOLLOW_WAYPOINTS: i32 = 36;
    /// `NAMED_SET_ATTITUDE`.
    pub const NAMED_SET_ATTITUDE: i32 = 45;
    /// `TEAM_SET_ATTITUDE`.
    pub const TEAM_SET_ATTITUDE: i32 = 46;
    /// `SPEECH_PLAY`.
    pub const SPEECH_PLAY: i32 = 83;
    /// `MAP_REVEAL_AT_WAYPOINT`.
    pub const MAP_REVEAL_AT_WAYPOINT: i32 = 90;
    /// `MAP_REVEAL_ALL`.
    pub const MAP_REVEAL_ALL: i32 = 102;
    /// `ZOOM_CAMERA`.
    pub const ZOOM_CAMERA: i32 = 117;
    /// `PITCH_CAMERA`.
    pub const PITCH_CAMERA: i32 = 118;
    /// `MAP_SHROUD_AT_WAYPOINT`.
    pub const MAP_SHROUD_AT_WAYPOINT: i32 = 145;
    /// `MAP_SHROUD_ALL`.
    pub const MAP_SHROUD_ALL: i32 = 146;
    /// `CAMERA_LOOK_TOWARD_OBJECT`.
    pub const CAMERA_LOOK_TOWARD_OBJECT: i32 = 181;
    /// `CAMERA_LOOK_TOWARD_WAYPOINT`.
    pub const CAMERA_LOOK_TOWARD_WAYPOINT: i32 = 206;
    /// `MAP_REVEAL_ALL_PERM`.
    pub const MAP_REVEAL_ALL_PERM: i32 = 229;
    /// `MAP_REVEAL_ALL_UNDO_PERM`.
    pub const MAP_REVEAL_ALL_UNDO_PERM: i32 = 230;
    /// `SKIRMISH_BUILD_BASE_DEFENSE_FRONT`.
    pub const SKIRMISH_BUILD_BASE_DEFENSE_FRONT: i32 = 250;
    /// `SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST`.
    pub const SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST: i32 = 251;
    /// `SKIRMISH_BUILD_BASE_DEFENSE_FLANK`.
    pub const SKIRMISH_BUILD_BASE_DEFENSE_FLANK: i32 = 254;
    /// `NUM_ITEMS`.
    pub const NUM_ITEMS: i32 = 344;
}

/// `TheObjectFlagsNames` (the object panel flags the actions can change).
pub const OBJECT_FLAGS_NAMES: [&str; 7] =
    ["Enabled", "Powered", "Indestructible", "Unsellable", "Selectable", "AI Recruitable", "Player Targetable"];

/// Where upstream is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapUndefined {
    /// Undefined in the chunk reader.
    Chunk(ChunkUndefined),
    /// `(ParameterType)readInt()` outside 0..=63.
    ParameterTypeOutOfRange,
    /// `(ConditionType)readInt()` outside 0..=127.
    ConditionTypeOutOfRange,
    /// `(ScriptActionType)readInt()` outside 0..=511.
    ActionTypeOutOfRange,
    /// `m_parms[i]` with `i` outside `0..MAX_PARMS`.
    ParameterIndexOutOfRange,
    /// A `NULL` `Parameter *` dereferenced.
    NullParameter,
    /// `ParseScript` returned `NULL` and the caller stored it.
    ScriptParseFailed,
    /// `strcpy` of a 256-character name into `char[256]`.
    NameTooLong,
    /// `kindofNames[m_int]` outside the table and its terminator.
    KindOfIndexOutOfRange,
    /// A list search for a node that is not in the list.
    NotInList,
    /// A parser that needs the object its caller was filling ran without one.
    NullUserData,
    /// A loop that upstream never leaves (`validateSides`' `goto` when the
    /// last team cannot be removed).
    NonTermination,
}

/// A failure of the map data code: upstream throws an `ErrorCode`, or is
/// undefined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapDataError {
    /// `throw ErrorCode`.
    Thrown(u32),
    /// Upstream is undefined.
    Undefined(MapUndefined),
}

impl From<ChunkError> for MapDataError {
    fn from(e: ChunkError) -> Self {
        match e {
            ChunkError::Thrown(c) => MapDataError::Thrown(c),
            ChunkError::Undefined(u) => MapDataError::Undefined(MapUndefined::Chunk(u)),
        }
    }
}

impl From<MapUndefined> for MapDataError {
    fn from(u: MapUndefined) -> Self {
        MapDataError::Undefined(u)
    }
}

impl std::fmt::Display for MapDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapDataError::Thrown(c) => write!(f, "map data error {c:#010x}"),
            MapDataError::Undefined(u) => write!(f, "map data outside what the original code defines: {u:?}"),
        }
    }
}

impl std::error::Error for MapDataError {}

/// Result of the map data functions.
pub type MapResult<T> = Result<T, MapDataError>;

/// The `ChunkError` a parser returns for `e`: a [`MapUndefined`] that is not
/// the chunk reader's is kept in the state (see [`ScriptsState::finish`]).
pub fn to_chunk_err(e: MapDataError, st: &mut ScriptsState) -> ChunkError {
    match e {
        MapDataError::Thrown(c) => ChunkError::Thrown(c),
        MapDataError::Undefined(MapUndefined::Chunk(u)) => ChunkError::Undefined(u),
        MapDataError::Undefined(u) => {
            st.undefined.get_or_insert(u);
            ChunkError::Undefined(ChunkUndefined::UninitialisedRead)
        }
    }
}

// ---------------------------------------------------------------------------
// Templates (the engine's tables, taken as data)
// ---------------------------------------------------------------------------

/// `class Template` as the parse and write see it: the internal name's key
/// and the parameter types (the editor text is not here).
#[derive(Clone, Debug, Default)]
pub struct ScriptTemplate {
    /// `m_internalNameKey`.
    pub internal_name_key: NameKeyType,
    /// `m_parameters[0..m_numParameters]`.
    pub parameters: Vec<i32>,
}

impl ScriptTemplate {
    /// `getNumParameters`.
    pub fn get_num_parameters(&self) -> i32 {
        self.parameters.len() as i32
    }

    /// `getParameterType(ndx)`: `INT` outside the range.
    pub fn get_parameter_type(&self, ndx: i32) -> i32 {
        if ndx >= 0 && (ndx as usize) < self.parameters.len() {
            self.parameters[ndx as usize]
        } else {
            parameter_type::INT
        }
    }
}

/// `TheScriptEngine`'s condition and action templates.
#[derive(Clone, Debug)]
pub struct ScriptTemplates {
    conditions: Vec<ScriptTemplate>,
    actions: Vec<ScriptTemplate>,
}

impl ScriptTemplates {
    /// The tables; `None` unless they hold exactly `Condition::NUM_ITEMS`
    /// conditions and `ScriptAction::NUM_ITEMS` actions, of at most
    /// [`MAX_PARMS`] parameters each.
    pub fn new(conditions: Vec<ScriptTemplate>, actions: Vec<ScriptTemplate>) -> Option<Self> {
        let ok = conditions.len() == condition_type::NUM_ITEMS as usize
            && actions.len() == action_type::NUM_ITEMS as usize
            && conditions.iter().chain(&actions).all(|t| t.parameters.len() <= MAX_PARMS);
        ok.then_some(ScriptTemplates { conditions, actions })
    }

    /// `getConditionTemplate(ndx)`: index 0 when out of range.
    pub fn get_condition_template(&self, ndx: i32) -> &ScriptTemplate {
        let ndx = if (0..condition_type::NUM_ITEMS).contains(&ndx) { ndx } else { 0 };
        &self.conditions[ndx as usize]
    }

    /// `getActionTemplate(ndx)`: index 0 when out of range.
    pub fn get_action_template(&self, ndx: i32) -> &ScriptTemplate {
        let ndx = if (0..action_type::NUM_ITEMS).contains(&ndx) { ndx } else { 0 };
        &self.actions[ndx as usize]
    }
}

// ---------------------------------------------------------------------------
// Parameter
// ---------------------------------------------------------------------------

/// `class Parameter`: one value of a condition or an action.
#[derive(Clone, Debug)]
pub struct Parameter {
    /// `m_paramType`: a [`parameter_type`] (any `i32` is kept).
    pub param_type: i32,
    /// `m_initialized`: set by `ReadParameter`.
    pub initialized: bool,
    /// `m_int`.
    pub int: i32,
    /// `m_real`.
    pub real: f32,
    /// `m_string`.
    pub string: Vec<u8>,
    /// `m_coord` (only a `COORD3D` parameter has one set).
    pub coord: Coord3D,
    /// `m_objectStatus` (set from the string of an `OBJECT_STATUS`).
    pub object_status: ObjectStatusMaskType,
}

impl Parameter {
    /// `Parameter(type, val)`.
    pub fn new(param_type: i32, val: i32) -> Self {
        Parameter {
            param_type,
            initialized: false,
            int: val,
            real: 0.0,
            string: Vec::new(),
            coord: Coord3D::default(),
            object_status: ObjectStatusMaskType::new(),
        }
    }

    /// `getParameterType`.
    pub fn get_parameter_type(&self) -> i32 {
        self.param_type
    }

    /// `getCoord3D`: zero unless the parameter is a `COORD3D`.
    pub fn get_coord3d(&self) -> Coord3D {
        if self.param_type == parameter_type::COORD3D {
            self.coord
        } else {
            Coord3D::default()
        }
    }

    /// `setCoord3D` / `friend_setCoord3D`: ignored unless a `COORD3D`.
    pub fn set_coord3d(&mut self, loc: Coord3D) {
        if self.param_type == parameter_type::COORD3D {
            self.coord = loc;
        }
    }

    /// `qualify(qualifier, playerTemplateName, newPlayerName)`: how a script
    /// is made for another player (the names it refers to get the qualifier;
    /// a `SIDE` that names the template player becomes the new player).
    pub fn qualify(&mut self, qualifier: &[u8], player_template_name: &[u8], new_player_name: &[u8]) {
        match self.param_type {
            parameter_type::SIDE => {
                let mut tmp = self.string.clone();
                tmp.extend_from_slice(qualifier);
                if tmp == player_template_name {
                    self.string = new_player_name.to_vec();
                }
            }
            parameter_type::TEAM => {
                if self.string != THIS_TEAM {
                    self.string.extend_from_slice(qualifier);
                }
            }
            parameter_type::SCRIPT | parameter_type::COUNTER | parameter_type::FLAG | parameter_type::SCRIPT_SUBROUTINE => {
                self.string.extend_from_slice(qualifier);
            }
            _ => {}
        }
    }

    /// `WriteParameter`. A `KIND_OF_PARAM` first has its string replaced by
    /// the name of its index (empty outside the table), as upstream does.
    pub fn write_parameter(&mut self, out: &mut DataChunkOutput) {
        out.write_int(self.param_type);
        if self.param_type == parameter_type::KIND_OF_PARAM {
            self.string = KindOfMaskType::get_name_from_single_bit(self.int).map_or_else(Vec::new, |n| n.as_bytes().to_vec());
        }
        if self.param_type == parameter_type::COORD3D {
            out.write_real(self.coord.x);
            out.write_real(self.coord.y);
            out.write_real(self.coord.z);
        } else {
            out.write_int(self.int);
            out.write_real(self.real);
            out.write_ascii_string(&self.string);
        }
    }

    /// `ReadParameter`: the type, then a position or an int, a real and a
    /// string; then the repairs of old files (`Fundamentalist` models are
    /// `GLA`, three obsolete capture-building upgrades are
    /// `Upgrade_InfantryCaptureBuilding`) and the conversion of the string
    /// of an `OBJECT_STATUS` to a mask and of a `KIND_OF_PARAM` to an index
    /// (or the reverse, when the string is empty).
    pub fn read_parameter<C>(file: &mut DataChunkInput<C>) -> MapResult<Parameter> {
        let t = file.read_int()?;
        if !(0..=63).contains(&t) {
            return Err(MapUndefined::ParameterTypeOutOfRange.into());
        }
        let mut p = Parameter::new(t, 0);
        p.initialized = true;
        if t == parameter_type::COORD3D {
            let x = file.read_real()?;
            let y = file.read_real()?;
            let z = file.read_real()?;
            p.set_coord3d(Coord3D { x, y, z });
        } else {
            p.int = file.read_int()?;
            p.real = file.read_real()?;
            p.string = file.read_ascii_string()?;
        }

        if t == parameter_type::OBJECT_TYPE && p.string.starts_with(b"Fundamentalist") {
            // char oldName[256]; strcpy(oldName, m_string)
            if p.string.len() > 255 {
                return Err(MapUndefined::NameTooLong.into());
            }
            let mut new = b"GLA".to_vec();
            new.extend_from_slice(&p.string[b"Fundamentalist".len()..]);
            p.string = new;
        }

        if t == parameter_type::UPGRADE
            && (p.string == b"Upgrade_AmericaRangerCaptureBuilding"
                || p.string == b"Upgrade_ChinaRedguardCaptureBuilding"
                || p.string == b"Upgrade_GLARebelCaptureBuilding")
        {
            p.string = b"Upgrade_InfantryCaptureBuilding".to_vec();
        }

        if t == parameter_type::OBJECT_STATUS {
            for (i, name) in ObjectStatusMaskType::get_bit_names().iter().enumerate() {
                if p.string.eq_ignore_ascii_case(name.as_bytes()) {
                    let mut m = ObjectStatusMaskType::new();
                    m.set(i as i32, 1);
                    p.object_status.set_mask(&m);
                    break;
                }
            }
        }

        if t == parameter_type::KIND_OF_PARAM {
            let names = KindOfMaskType::get_bit_names();
            if !p.string.is_empty() {
                p.read_kind_of_name(names)?;
            } else {
                // m_string = kindofNames[m_int]: the table ends with a NULL
                // (an empty string) at index KINDOF_COUNT
                match usize::try_from(p.int) {
                    Ok(i) if i < names.len() => p.string = names[i].as_bytes().to_vec(),
                    Ok(i) if i == names.len() => p.string = Vec::new(),
                    _ => return Err(MapUndefined::KindOfIndexOutOfRange.into()),
                }
            }
        }
        Ok(p)
    }

    /// The `KIND_OF_PARAM` branch for a non-empty string: look the name up
    /// (case-insensitively) with upstream's loop, including its handling of
    /// the retired `CRUSHER`, `CRUSHABLE`, `OVERLAPPABLE` and `MISSILE`.
    fn read_kind_of_name(&mut self, names: &[&str]) -> MapResult<()> {
        let eq = |s: &[u8], n: &str| s.eq_ignore_ascii_case(n.as_bytes());
        let mut found = false;
        let mut i = 0usize;
        while i < names.len() {
            if eq(&self.string, names[i]) {
                self.int = i as i32;
                found = true;
                break;
            }
            if eq(&self.string, "CRUSHER") || eq(&self.string, "CRUSHABLE") || eq(&self.string, "OVERLAPPABLE") {
                self.int = i as i32;
                found = true;
                break;
            } else if eq(&self.string, "MISSILE") {
                // MISSILE was split into SMALL_MISSILE and BALLISTIC_MISSILE
                self.string = b"SMALL_MISSILE".to_vec();
                // the retry loop compares m_string with the literal it was
                // just set to, not with kindofNames[i]: always equal at 0
                i = 0;
                if !names.is_empty() {
                    self.int = 0;
                    found = true;
                }
            }
            i += 1;
        }
        if !found {
            return Err(MapDataError::Thrown(ERROR_BUG));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Condition, OrCondition, ScriptAction
// ---------------------------------------------------------------------------

fn empty_parms() -> [Option<Parameter>; MAX_PARMS] {
    std::array::from_fn(|_| None)
}

/// A new parameter of each type the template lists (`setConditionType` and
/// `setActionType`): the slots up to the template's count, the rest empty.
fn parms_from_template(t: &ScriptTemplate) -> (i32, [Option<Parameter>; MAX_PARMS]) {
    let mut parms = empty_parms();
    for i in 0..t.get_num_parameters() {
        parms[i as usize] = Some(Parameter::new(t.get_parameter_type(i), 0));
    }
    (t.get_num_parameters(), parms)
}

/// Copy the parameters of `src` over those of `dst` as `duplicate` and
/// `duplicateAndQualify` do: `*pNew->m_parms[i] = *m_parms[i]`, qualified
/// when `q` is given.
fn copy_parm(dst: &mut Option<Parameter>, src: &Option<Parameter>, q: Option<(&[u8], &[u8], &[u8])>) -> MapResult<()> {
    let (Some(d), Some(s)) = (dst.as_mut(), src.as_ref()) else {
        return Err(MapUndefined::NullParameter.into());
    };
    *d = s.clone();
    if let Some((qualifier, tpl, new_name)) = q {
        d.qualify(qualifier, tpl, new_name);
    }
    Ok(())
}

/// `class Condition`: one test, with its parameters.
#[derive(Clone, Debug)]
pub struct Condition {
    /// `m_conditionType`: a `Condition::ConditionType` (any `i32` is kept).
    pub condition_type: i32,
    /// `m_numParms`.
    pub num_parms: i32,
    /// `m_parms`.
    pub parms: [Option<Parameter>; MAX_PARMS],
    /// `m_hasWarnings` (an `Int`; editor flag).
    pub has_warnings: i32,
    /// `m_customData`.
    pub custom_data: i32,
    /// `m_customFrame`.
    pub custom_frame: u32,
}

impl Condition {
    /// `Condition()`, the protected constructor for reading: `CONDITION_FALSE`
    /// without parameters.
    pub fn new_for_read() -> Self {
        Condition {
            condition_type: condition_type::CONDITION_FALSE,
            num_parms: 0,
            parms: empty_parms(),
            has_warnings: 0,
            custom_data: 0,
            custom_frame: 0,
        }
    }

    /// `Condition(type)`: with the parameters the template lists.
    pub fn new(condition_type: i32, templates: &ScriptTemplates) -> Self {
        let mut c = Condition::new_for_read();
        c.set_condition_type(condition_type, templates);
        c
    }

    /// `setConditionType(type)`: drop the parameters and create those the
    /// template lists.
    pub fn set_condition_type(&mut self, condition_type: i32, templates: &ScriptTemplates) {
        self.parms = empty_parms();
        self.condition_type = condition_type;
        let (n, parms) = parms_from_template(templates.get_condition_template(condition_type));
        self.num_parms = n;
        self.parms = parms;
    }

    /// `getParameter(ndx)`: `None` outside `0..m_numParms`.
    pub fn get_parameter(&self, ndx: i32) -> Option<&Parameter> {
        if ndx >= 0 && ndx < self.num_parms {
            self.parms[ndx as usize].as_ref()
        } else {
            None
        }
    }

    /// `duplicate()` of the head of an AND chain: a copy of every condition
    /// in `chain`, each made from its type's template with the parameters of
    /// the original copied over. (A condition after the first whose
    /// parameter count exceeds its type's template count dereferences an
    /// empty slot: [`MapUndefined::NullParameter`].)
    pub fn duplicate_chain(chain: &[Condition], templates: &ScriptTemplates) -> MapResult<Vec<Condition>> {
        Self::copy_chain(chain, templates, None)
    }

    /// `duplicateAndQualify(...)` of the head of an AND chain.
    pub fn duplicate_and_qualify_chain(
        chain: &[Condition],
        templates: &ScriptTemplates,
        qualifier: &[u8],
        player_template_name: &[u8],
        new_player_name: &[u8],
    ) -> MapResult<Vec<Condition>> {
        Self::copy_chain(chain, templates, Some((qualifier, player_template_name, new_player_name)))
    }

    fn copy_chain(chain: &[Condition], templates: &ScriptTemplates, q: Option<(&[u8], &[u8], &[u8])>) -> MapResult<Vec<Condition>> {
        let mut out = Vec::with_capacity(chain.len());
        for (k, src) in chain.iter().enumerate() {
            let mut new = Condition::new(src.condition_type, templates);
            // the head is copied up to both counts; the rest up to the
            // original's count (so a short template is an empty slot)
            let n = if k == 0 { src.num_parms.min(new.num_parms) } else { src.num_parms };
            for i in 0..n.max(0) {
                if i as usize >= MAX_PARMS {
                    return Err(MapUndefined::ParameterIndexOutOfRange.into());
                }
                copy_parm(&mut new.parms[i as usize], &src.parms[i as usize], q)?;
            }
            out.push(new);
        }
        Ok(out)
    }

    /// `WriteConditionDataChunk` for each condition of `chain` (the head and
    /// what follows it): a `Condition` chunk of version 4 with the type, the
    /// internal name key of its template, the parameter count and each
    /// parameter.
    pub fn write_condition_data_chunk(
        out: &mut DataChunkOutput,
        chain: &mut [Condition],
        templates: &ScriptTemplates,
        names: &NameKeyGenerator,
    ) -> MapResult<()> {
        for c in chain {
            out.open_data_chunk(b"Condition", version::SCRIPT_CONDITION_4);
            out.write_int(c.condition_type);
            out.write_name_key(templates.get_condition_template(c.condition_type).internal_name_key, names);
            out.write_int(c.num_parms);
            for i in 0..c.num_parms.max(0) {
                if i as usize >= MAX_PARMS {
                    return Err(MapUndefined::ParameterIndexOutOfRange.into());
                }
                c.parms[i as usize].as_mut().ok_or(MapUndefined::NullParameter)?.write_parameter(out);
            }
            out.close_data_chunk();
        }
        Ok(())
    }
}

/// `class OrCondition`: one OR clause, holding the conditions that are ANDed.
#[derive(Clone, Debug, Default)]
pub struct OrCondition {
    /// `m_firstAnd` and the chain after it.
    pub conditions: Vec<Condition>,
}

impl OrCondition {
    /// `duplicate()` of the head of an OR chain.
    pub fn duplicate_chain(chain: &[OrCondition], templates: &ScriptTemplates) -> MapResult<Vec<OrCondition>> {
        chain
            .iter()
            .map(|o| Ok(OrCondition { conditions: Condition::duplicate_chain(&o.conditions, templates)? }))
            .collect()
    }

    /// `duplicateAndQualify(...)` of the head of an OR chain.
    pub fn duplicate_and_qualify_chain(
        chain: &[OrCondition],
        templates: &ScriptTemplates,
        qualifier: &[u8],
        player_template_name: &[u8],
        new_player_name: &[u8],
    ) -> MapResult<Vec<OrCondition>> {
        chain
            .iter()
            .map(|o| {
                Ok(OrCondition {
                    conditions: Condition::duplicate_and_qualify_chain(&o.conditions, templates, qualifier, player_template_name, new_player_name)?,
                })
            })
            .collect()
    }

    /// `removeCondition(pCond)`: unlink the condition at `index` and return
    /// it. A position outside the list is [`MapUndefined::NotInList`] (the
    /// pointer search runs off the end).
    pub fn remove_condition(&mut self, index: usize) -> MapResult<Condition> {
        if index >= self.conditions.len() {
            return Err(MapUndefined::NotInList.into());
        }
        Ok(self.conditions.remove(index))
    }

    /// `deleteCondition(pCond)`.
    pub fn delete_condition(&mut self, index: usize) -> MapResult<()> {
        self.remove_condition(index).map(|_| ())
    }

    /// `findPreviousCondition(curCond)`: the index of the condition before
    /// the one at `cur`, `None` for the first.
    pub fn find_previous_condition(&self, cur: usize) -> MapResult<Option<usize>> {
        if cur >= self.conditions.len() {
            return Err(MapUndefined::NotInList.into());
        }
        Ok(cur.checked_sub(1))
    }

    /// `WriteOrConditionDataChunk` for each OR clause of `chain`.
    pub fn write_or_condition_data_chunk(
        out: &mut DataChunkOutput,
        chain: &mut [OrCondition],
        templates: &ScriptTemplates,
        names: &NameKeyGenerator,
    ) -> MapResult<()> {
        for o in chain {
            out.open_data_chunk(b"OrCondition", version::SCRIPT_OR_CONDITION_1);
            Condition::write_condition_data_chunk(out, &mut o.conditions, templates, names)?;
            out.close_data_chunk();
        }
        Ok(())
    }
}

/// `class ScriptAction`: one thing a script does, with its parameters.
#[derive(Clone, Debug)]
pub struct ScriptAction {
    /// `m_actionType`: a `ScriptAction::ScriptActionType` (any `i32` is kept).
    pub action_type: i32,
    /// `m_numParms`.
    pub num_parms: i32,
    /// `m_parms`.
    pub parms: [Option<Parameter>; MAX_PARMS],
    /// `m_hasWarnings` (editor flag).
    pub has_warnings: bool,
}

impl ScriptAction {
    /// `ScriptAction()`, the protected constructor for reading: `NO_OP`
    /// without parameters.
    pub fn new_for_read() -> Self {
        ScriptAction { action_type: action_type::NO_OP, num_parms: 0, parms: empty_parms(), has_warnings: false }
    }

    /// `ScriptAction(type)`: with the parameters the template lists.
    pub fn new(action_type: i32, templates: &ScriptTemplates) -> Self {
        let mut a = ScriptAction::new_for_read();
        a.set_action_type(action_type, templates);
        a
    }

    /// `setActionType(type)`.
    pub fn set_action_type(&mut self, action_type: i32, templates: &ScriptTemplates) {
        self.parms = empty_parms();
        self.action_type = action_type;
        let (n, parms) = parms_from_template(templates.get_action_template(action_type));
        self.num_parms = n;
        self.parms = parms;
    }

    /// `getParameter(ndx)`: `None` outside `0..m_numParms`.
    pub fn get_parameter(&self, ndx: i32) -> Option<&Parameter> {
        if ndx >= 0 && ndx < self.num_parms {
            self.parms[ndx as usize].as_ref()
        } else {
            None
        }
    }

    /// `duplicate()` of the head of an action chain: each action is made
    /// from its type's template and gets the parameters of the original
    /// where both have one.
    pub fn duplicate_chain(chain: &[ScriptAction], templates: &ScriptTemplates) -> Vec<ScriptAction> {
        Self::copy_chain(chain, templates, None)
    }

    /// `duplicateAndQualify(...)` of the head of an action chain.
    pub fn duplicate_and_qualify_chain(
        chain: &[ScriptAction],
        templates: &ScriptTemplates,
        qualifier: &[u8],
        player_template_name: &[u8],
        new_player_name: &[u8],
    ) -> Vec<ScriptAction> {
        Self::copy_chain(chain, templates, Some((qualifier, player_template_name, new_player_name)))
    }

    fn copy_chain(chain: &[ScriptAction], templates: &ScriptTemplates, q: Option<(&[u8], &[u8], &[u8])>) -> Vec<ScriptAction> {
        chain
            .iter()
            .map(|src| {
                let mut new = ScriptAction::new(src.action_type, templates);
                for i in 0..src.num_parms.clamp(0, MAX_PARMS as i32) as usize {
                    // `if (pNew->m_parms[i]) ...` (and, after the head, the source's)
                    if let (Some(d), Some(s)) = (new.parms[i].as_mut(), src.parms[i].as_ref()) {
                        *d = s.clone();
                        if let Some((qualifier, tpl, new_name)) = q {
                            d.qualify(qualifier, tpl, new_name);
                        }
                    }
                }
                new
            })
            .collect()
    }

    /// `WriteActionDataChunk` (`false_chain` false) and
    /// `WriteActionFalseDataChunk` (true) for each action of `chain`: a
    /// `ScriptAction` or `ScriptActionFalse` chunk of version 2.
    pub fn write_action_data_chunk(
        out: &mut DataChunkOutput,
        chain: &mut [ScriptAction],
        false_chain: bool,
        templates: &ScriptTemplates,
        names: &NameKeyGenerator,
    ) -> MapResult<()> {
        for a in chain {
            out.open_data_chunk(if false_chain { b"ScriptActionFalse" } else { b"ScriptAction" }, version::SCRIPT_ACTION_2);
            out.write_int(a.action_type);
            out.write_name_key(templates.get_action_template(a.action_type).internal_name_key, names);
            out.write_int(a.num_parms);
            for i in 0..a.num_parms.max(0) {
                if i as usize >= MAX_PARMS {
                    return Err(MapUndefined::ParameterIndexOutOfRange.into());
                }
                a.parms[i as usize].as_mut().ok_or(MapUndefined::NullParameter)?.write_parameter(out);
            }
            out.close_data_chunk();
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Script, ScriptGroup, ScriptList
// ---------------------------------------------------------------------------

/// `class Script`: conditions and actions, with the flags that say when it
/// applies.
#[derive(Clone, Debug)]
pub struct Script {
    /// `m_scriptName`.
    pub script_name: Vec<u8>,
    /// `m_comment`.
    pub comment: Vec<u8>,
    /// `m_conditionComment`.
    pub condition_comment: Vec<u8>,
    /// `m_actionComment`.
    pub action_comment: Vec<u8>,
    /// `m_delayEvaluationSeconds`.
    pub delay_evaluation_seconds: i32,
    /// `m_isActive`.
    pub is_active: bool,
    /// `m_isOneShot`.
    pub is_one_shot: bool,
    /// `m_isSubroutine`.
    pub is_subroutine: bool,
    /// `m_easy`.
    pub easy: bool,
    /// `m_normal`.
    pub normal: bool,
    /// `m_hard`.
    pub hard: bool,
    /// `m_condition` and the chain after it.
    pub conditions: Vec<OrCondition>,
    /// `m_action` and the chain after it.
    pub actions: Vec<ScriptAction>,
    /// `m_actionFalse` and the chain after it.
    pub actions_false: Vec<ScriptAction>,
    // Runtime fields - not saved or read.
    /// `m_frameToEvaluateAt`.
    pub frame_to_evaluate_at: u32,
    /// `m_hasWarnings` (editor flag).
    pub has_warnings: bool,
    /// `m_conditionTeamName` (the engine's).
    pub condition_team_name: Vec<u8>,
    /// `m_conditionTime`.
    pub condition_time: f32,
    /// `m_curTime`.
    pub cur_time: f32,
    /// `m_conditionExecutedCount`.
    pub condition_executed_count: i32,
}

impl Default for Script {
    fn default() -> Self {
        Self::new()
    }
}

impl Script {
    /// `Script()`: active, one-shot, for every difficulty, no delay.
    pub fn new() -> Self {
        Script {
            script_name: Vec::new(),
            comment: Vec::new(),
            condition_comment: Vec::new(),
            action_comment: Vec::new(),
            delay_evaluation_seconds: 0,
            is_active: true,
            is_one_shot: true,
            is_subroutine: false,
            easy: true,
            normal: true,
            hard: true,
            conditions: Vec::new(),
            actions: Vec::new(),
            actions_false: Vec::new(),
            frame_to_evaluate_at: 0,
            has_warnings: false,
            condition_team_name: Vec::new(),
            condition_time: 0.0,
            cur_time: 0.0,
            condition_executed_count: 0,
        }
    }

    /// `duplicate()`: the script with its conditions and actions copied, the
    /// runtime fields as for a new script.
    pub fn duplicate(&self, templates: &ScriptTemplates) -> MapResult<Script> {
        self.copy(templates, None)
    }

    /// `duplicateAndQualify(...)`: `duplicate()` with the qualifier added to
    /// the script's name and to the names its parameters refer to.
    pub fn duplicate_and_qualify(
        &self,
        templates: &ScriptTemplates,
        qualifier: &[u8],
        player_template_name: &[u8],
        new_player_name: &[u8],
    ) -> MapResult<Script> {
        self.copy(templates, Some((qualifier, player_template_name, new_player_name)))
    }

    fn copy(&self, templates: &ScriptTemplates, q: Option<(&[u8], &[u8], &[u8])>) -> MapResult<Script> {
        let mut n = Script::new();
        n.script_name = self.script_name.clone();
        if let Some((qualifier, ..)) = q {
            n.script_name.extend_from_slice(qualifier);
        }
        n.comment = self.comment.clone();
        n.condition_comment = self.condition_comment.clone();
        n.action_comment = self.action_comment.clone();
        n.is_active = self.is_active;
        n.is_one_shot = self.is_one_shot;
        n.is_subroutine = self.is_subroutine;
        n.easy = self.easy;
        n.normal = self.normal;
        n.hard = self.hard;
        n.delay_evaluation_seconds = self.delay_evaluation_seconds;
        match q {
            None => {
                n.conditions = OrCondition::duplicate_chain(&self.conditions, templates)?;
                n.actions = ScriptAction::duplicate_chain(&self.actions, templates);
                n.actions_false = ScriptAction::duplicate_chain(&self.actions_false, templates);
            }
            Some((qualifier, tpl, new_name)) => {
                n.conditions = OrCondition::duplicate_and_qualify_chain(&self.conditions, templates, qualifier, tpl, new_name)?;
                n.actions = ScriptAction::duplicate_and_qualify_chain(&self.actions, templates, qualifier, tpl, new_name);
                n.actions_false = ScriptAction::duplicate_and_qualify_chain(&self.actions_false, templates, qualifier, tpl, new_name);
            }
        }
        Ok(n)
    }

    /// `updateFrom(pSrc)`: take the data of `src`, whose conditions and
    /// actions are moved here and left empty.
    pub fn update_from(&mut self, src: &mut Script) {
        self.script_name = src.script_name.clone();
        self.comment = src.comment.clone();
        self.condition_comment = src.condition_comment.clone();
        self.action_comment = src.action_comment.clone();
        self.is_active = src.is_active;
        self.is_subroutine = src.is_subroutine;
        self.delay_evaluation_seconds = src.delay_evaluation_seconds;
        self.is_one_shot = src.is_one_shot;
        self.easy = src.easy;
        self.normal = src.normal;
        self.hard = src.hard;
        self.conditions = std::mem::take(&mut src.conditions);
        self.actions = std::mem::take(&mut src.actions);
        self.actions_false = std::mem::take(&mut src.actions_false);
    }

    /// `deleteOrCondition(pCond)`: remove the OR clause at `index`.
    pub fn delete_or_condition(&mut self, index: usize) -> MapResult<()> {
        remove_at(&mut self.conditions, index).map(|_| ())
    }

    /// `deleteAction(pAct)`: remove the action at `index`.
    pub fn delete_action(&mut self, index: usize) -> MapResult<()> {
        remove_at(&mut self.actions, index).map(|_| ())
    }

    /// `deleteFalseAction(pAct)`: remove the false action at `index`.
    pub fn delete_false_action(&mut self, index: usize) -> MapResult<()> {
        remove_at(&mut self.actions_false, index).map(|_| ())
    }

    /// `findPreviousOrCondition(curOr)`: the index of the clause before the
    /// one at `cur`, `None` for the first.
    pub fn find_previous_or_condition(&self, cur: usize) -> MapResult<Option<usize>> {
        if cur >= self.conditions.len() {
            return Err(MapUndefined::NotInList.into());
        }
        Ok(cur.checked_sub(1))
    }

    /// `WriteScriptDataChunk` for each script of `chain`: a `Script` chunk of
    /// version 2 holding the names, the flags, the delay, then the OR
    /// clauses, the actions and the false actions as chunks.
    pub fn write_script_data_chunk(
        out: &mut DataChunkOutput,
        chain: &mut [Script],
        templates: &ScriptTemplates,
        names: &NameKeyGenerator,
    ) -> MapResult<()> {
        for s in chain {
            out.open_data_chunk(b"Script", version::SCRIPT_2);
            out.write_ascii_string(&s.script_name);
            out.write_ascii_string(&s.comment);
            out.write_ascii_string(&s.condition_comment);
            out.write_ascii_string(&s.action_comment);
            out.write_byte(i8::from(s.is_active));
            out.write_byte(i8::from(s.is_one_shot));
            out.write_byte(i8::from(s.easy));
            out.write_byte(i8::from(s.normal));
            out.write_byte(i8::from(s.hard));
            out.write_byte(i8::from(s.is_subroutine));
            out.write_int(s.delay_evaluation_seconds);
            OrCondition::write_or_condition_data_chunk(out, &mut s.conditions, templates, names)?;
            ScriptAction::write_action_data_chunk(out, &mut s.actions, false, templates, names)?;
            ScriptAction::write_action_data_chunk(out, &mut s.actions_false, true, templates, names)?;
            out.close_data_chunk();
        }
        Ok(())
    }
}

/// Unlink the node at `index` of a list (the pointer searches of the
/// `delete*` functions, which run off the end for a node that is not there).
fn remove_at<T>(v: &mut Vec<T>, index: usize) -> MapResult<T> {
    if index >= v.len() {
        return Err(MapUndefined::NotInList.into());
    }
    Ok(v.remove(index))
}

/// Where `add*(p, ndx)` puts the node: the walk `while (ndx && pCur)` stops
/// after `ndx` nodes (index 0 for 0), at the end for a larger or negative
/// `ndx`.
fn insert_position(len: usize, ndx: i32) -> usize {
    match ndx {
        0 => 0,
        n if n > 0 => (n as usize).min(len),
        _ => len,
    }
}

/// `class ScriptGroup`: scripts under a name, switched on or off together.
#[derive(Clone, Debug)]
pub struct ScriptGroup {
    /// `m_groupName`.
    pub group_name: Vec<u8>,
    /// `m_isGroupActive`.
    pub is_group_active: bool,
    /// `m_isGroupSubroutine`.
    pub is_group_subroutine: bool,
    /// `m_hasWarnings` (editor flag).
    pub has_warnings: bool,
    /// `m_firstScript` and the chain after it.
    pub scripts: Vec<Script>,
}

impl ScriptGroup {
    /// `ScriptGroup()`: active, named `Script Group <n>` with `n` the next
    /// number of the counter `ScriptList::m_curId` (`cur_id`).
    pub fn new(cur_id: &mut i32) -> Self {
        ScriptGroup {
            group_name: format!("Script Group {}", ScriptList::get_next_id(cur_id)).into_bytes(),
            is_group_active: true,
            is_group_subroutine: false,
            has_warnings: false,
            scripts: Vec::new(),
        }
    }

    /// `duplicate()`: the group with its scripts copied (and a number of
    /// the counter used up by the new group's constructor).
    pub fn duplicate(&self, cur_id: &mut i32, templates: &ScriptTemplates) -> MapResult<ScriptGroup> {
        let mut n = ScriptGroup::new(cur_id);
        for s in &self.scripts {
            n.scripts.push(s.duplicate(templates)?);
        }
        n.group_name = self.group_name.clone();
        n.is_group_active = self.is_group_active;
        n.is_group_subroutine = self.is_group_subroutine;
        Ok(n)
    }

    /// `duplicateAndQualify(...)`.
    pub fn duplicate_and_qualify(
        &self,
        cur_id: &mut i32,
        templates: &ScriptTemplates,
        qualifier: &[u8],
        player_template_name: &[u8],
        new_player_name: &[u8],
    ) -> MapResult<ScriptGroup> {
        let mut n = ScriptGroup::new(cur_id);
        for s in &self.scripts {
            n.scripts.push(s.duplicate_and_qualify(templates, qualifier, player_template_name, new_player_name)?);
        }
        n.group_name = self.group_name.clone();
        n.group_name.extend_from_slice(qualifier);
        n.is_group_active = self.is_group_active;
        n.is_group_subroutine = self.is_group_subroutine;
        Ok(n)
    }

    /// `addScript(pScr, ndx)`: insert after `ndx` scripts (at the end for
    /// [`AT_END`]).
    pub fn add_script(&mut self, script: Script, ndx: i32) {
        let at = insert_position(self.scripts.len(), ndx);
        self.scripts.insert(at, script);
    }

    /// `deleteScript(pScr)`: remove the script at `index`.
    pub fn delete_script(&mut self, index: usize) -> MapResult<()> {
        remove_at(&mut self.scripts, index).map(|_| ())
    }

    /// `WriteGroupDataChunk` for each group of `chain`: a `ScriptGroup`
    /// chunk of version 2 (name, two flags, then its scripts as chunks).
    pub fn write_group_data_chunk(
        out: &mut DataChunkOutput,
        chain: &mut [ScriptGroup],
        templates: &ScriptTemplates,
        names: &NameKeyGenerator,
    ) -> MapResult<()> {
        for g in chain {
            out.open_data_chunk(b"ScriptGroup", version::SCRIPT_GROUP_2);
            out.write_ascii_string(&g.group_name);
            out.write_byte(i8::from(g.is_group_active));
            out.write_byte(i8::from(g.is_group_subroutine));
            Script::write_script_data_chunk(out, &mut g.scripts, templates, names)?;
            out.close_data_chunk();
        }
        Ok(())
    }
}

/// `class ScriptList`: the scripts and script groups of one player.
#[derive(Clone, Debug, Default)]
pub struct ScriptList {
    /// `m_firstGroup` and the chain after it.
    pub groups: Vec<ScriptGroup>,
    /// `m_firstScript` and the chain after it.
    pub scripts: Vec<Script>,
}

/// `TScriptListReadInfo`: what `ParseScriptsDataChunk` has read so far.
#[derive(Clone, Debug, Default)]
pub struct ScriptListReadInfo {
    /// `readLists[0..numLists]`.
    pub read_lists: Vec<ScriptList>,
}

/// The process-wide state of `Scripts.cpp` and of the parse in flight.
#[derive(Clone, Debug)]
pub struct ScriptsState {
    /// `ScriptList::m_curId`: the last group number handed out.
    pub cur_id: i32,
    /// `s_readLists`: what the last successful parse read (`None` for a
    /// list since taken or discarded).
    read_lists: [Option<ScriptList>; MAX_PLAYER_COUNT],
    /// `s_numInReadList`.
    num_in_read_list: i32,
    /// The `TScriptListReadInfo` that `ParseScriptsDataChunk` passes down.
    read_info: Option<ScriptListReadInfo>,
    /// The `Script`s being read (`ParseScript`'s `pScript`), innermost last.
    script_stack: Vec<Script>,
    /// The first [`MapUndefined`] a parser met (it can only return a
    /// `ChunkError`; the entry points turn this back into the error).
    pub undefined: Option<MapUndefined>,
}

impl Default for ScriptsState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptsState {
    /// The state of a process that has read no scripts.
    pub fn new() -> Self {
        ScriptsState {
            cur_id: 0,
            read_lists: std::array::from_fn(|_| None),
            num_in_read_list: 0,
            read_info: None,
            script_stack: Vec::new(),
            undefined: None,
        }
    }

    /// `s_numInReadList`.
    pub fn num_in_read_list(&self) -> i32 {
        self.num_in_read_list
    }

    /// Turn the result of a parse into the error the caller sees: a
    /// [`MapUndefined`] a parser stashed takes the place of the stand-in
    /// `ChunkError` it returned.
    pub fn finish<T>(&mut self, r: ChunkResult<T>) -> MapResult<T> {
        let u = self.undefined.take();
        match r {
            Ok(v) => Ok(v),
            Err(ChunkError::Undefined(_)) if u.is_some() => Err(MapDataError::Undefined(u.expect("checked"))),
            Err(e) => Err(e.into()),
        }
    }
}

/// What the parsers need from the caller's context type: the state, the
/// name keys and the templates.
pub trait ScriptsContext {
    /// The state of `Scripts.cpp`.
    fn scripts_state(&mut self) -> &mut ScriptsState;
    /// `TheNameKeyGenerator`.
    fn names(&mut self) -> &mut NameKeyGenerator;
    /// `TheScriptEngine`'s templates.
    fn templates(&self) -> &ScriptTemplates;
}

/// A [`ScriptsContext`] for reading scripts on their own (a `.scb` file).
#[derive(Clone, Debug)]
pub struct ScriptsCtx {
    /// The state.
    pub state: ScriptsState,
    /// The name keys.
    pub names: NameKeyGenerator,
    /// The templates.
    pub templates: ScriptTemplates,
}

impl ScriptsContext for ScriptsCtx {
    fn scripts_state(&mut self) -> &mut ScriptsState {
        &mut self.state
    }
    fn names(&mut self) -> &mut NameKeyGenerator {
        &mut self.names
    }
    fn templates(&self) -> &ScriptTemplates {
        &self.templates
    }
}

impl ScriptList {
    /// `ScriptList()`: no groups, no scripts.
    pub fn new() -> Self {
        ScriptList::default()
    }

    /// `getNextID()`: the counter, incremented first.
    pub fn get_next_id(cur_id: &mut i32) -> i32 {
        *cur_id = cur_id.wrapping_add(1);
        *cur_id
    }

    /// `duplicate()`: a deep copy.
    pub fn duplicate(&self, cur_id: &mut i32, templates: &ScriptTemplates) -> MapResult<ScriptList> {
        let mut n = ScriptList::new();
        for g in &self.groups {
            n.groups.push(g.duplicate(cur_id, templates)?);
        }
        for s in &self.scripts {
            n.scripts.push(s.duplicate(templates)?);
        }
        Ok(n)
    }

    /// `duplicateAndQualify(...)`: a deep copy with the qualifier added to
    /// names (how a script list is made for another player).
    pub fn duplicate_and_qualify(
        &self,
        cur_id: &mut i32,
        templates: &ScriptTemplates,
        qualifier: &[u8],
        player_template_name: &[u8],
        new_player_name: &[u8],
    ) -> MapResult<ScriptList> {
        let mut n = ScriptList::new();
        for g in &self.groups {
            n.groups.push(g.duplicate_and_qualify(cur_id, templates, qualifier, player_template_name, new_player_name)?);
        }
        for s in &self.scripts {
            n.scripts.push(s.duplicate_and_qualify(templates, qualifier, player_template_name, new_player_name)?);
        }
        Ok(n)
    }

    /// `discard()`: end the list but not what it holds, which is returned
    /// (the groups, then the scripts).
    pub fn discard(self) -> (Vec<ScriptGroup>, Vec<Script>) {
        (self.groups, self.scripts)
    }

    /// `addGroup(pGrp, ndx)`.
    pub fn add_group(&mut self, group: ScriptGroup, ndx: i32) {
        let at = insert_position(self.groups.len(), ndx);
        self.groups.insert(at, group);
    }

    /// `addScript(pScr, ndx)`.
    pub fn add_script(&mut self, script: Script, ndx: i32) {
        let at = insert_position(self.scripts.len(), ndx);
        self.scripts.insert(at, script);
    }

    /// `deleteScript(pScr)`: remove the script at `index`.
    pub fn delete_script(&mut self, index: usize) -> MapResult<()> {
        remove_at(&mut self.scripts, index).map(|_| ())
    }

    /// `deleteGroup(pGrp)`: remove the group at `index`.
    pub fn delete_group(&mut self, index: usize) -> MapResult<()> {
        remove_at(&mut self.groups, index).map(|_| ())
    }

    /// `getReadScripts(scriptLists)`: what the last parse read, once; the
    /// count is `s_numInReadList`, an entry is `None` if the list is gone.
    pub fn get_read_scripts(st: &mut ScriptsState) -> Vec<Option<ScriptList>> {
        let count = st.num_in_read_list;
        st.num_in_read_list = 0;
        let mut out = Vec::new();
        for i in 0..count.max(0) as usize {
            out.push(st.read_lists[i].take());
        }
        out
    }

    /// `WriteScriptListDataChunk`: the scripts, then the groups.
    pub fn write_script_list_data_chunk(
        &mut self,
        out: &mut DataChunkOutput,
        templates: &ScriptTemplates,
        names: &NameKeyGenerator,
    ) -> MapResult<()> {
        Script::write_script_data_chunk(out, &mut self.scripts, templates, names)?;
        ScriptGroup::write_group_data_chunk(out, &mut self.groups, templates, names)
    }

    /// `WriteScriptsDataChunk(chunkWriter, scriptLists, numLists)`: a
    /// `PlayerScriptsList` chunk holding one `ScriptList` chunk per list
    /// (empty for a missing list).
    pub fn write_scripts_data_chunk(
        out: &mut DataChunkOutput,
        script_lists: &mut [Option<&mut ScriptList>],
        templates: &ScriptTemplates,
        names: &NameKeyGenerator,
    ) -> MapResult<()> {
        out.open_data_chunk(b"PlayerScriptsList", version::SCRIPTS_1);
        for l in script_lists.iter_mut() {
            out.open_data_chunk(b"ScriptList", version::SCRIPT_LIST_1);
            if let Some(l) = l {
                l.write_script_list_data_chunk(out, templates, names)?;
            }
            out.close_data_chunk();
        }
        out.close_data_chunk();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The parsers
// ---------------------------------------------------------------------------

/// The chunk parser of a `PlayerScriptsList` chunk: `ParseScriptsDataChunk`.
/// Reads its `ScriptList` chunks into the state (see
/// [`ScriptList::get_read_scripts`]); false if there are more than
/// [`MAX_PLAYER_COUNT`].
pub fn parse_scripts_data_chunk<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> ChunkResult<bool> {
    file.register_parser(b"ScriptList", &info.label, parse_script_list_data_chunk::<C>);
    let st = ctx.scripts_state();
    // DEBUG_ASSERTCRASH(s_numInReadList==0) is compiled out; leftovers are dropped
    for i in 0..st.num_in_read_list.clamp(0, MAX_PLAYER_COUNT as i32) as usize {
        st.read_lists[i] = None;
    }
    let saved = st.read_info.replace(ScriptListReadInfo::default());
    let r = file.parse(ctx);
    let st = ctx.scripts_state();
    let read = std::mem::replace(&mut st.read_info, saved).unwrap_or_default();
    if r? {
        st.num_in_read_list = read.read_lists.len() as i32;
        for (i, l) in read.read_lists.into_iter().enumerate() {
            st.read_lists[i] = Some(l);
        }
        return Ok(true);
    }
    Ok(false)
}

/// `ParseScriptListDataChunk`: one player's list.
fn parse_script_list_data_chunk<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> ChunkResult<bool> {
    let st = ctx.scripts_state();
    let Some(ri) = st.read_info.as_mut() else {
        return Err(to_chunk_err(MapUndefined::NullUserData.into(), st));
    };
    if ri.read_lists.len() >= MAX_PLAYER_COUNT {
        return Ok(false);
    }
    ri.read_lists.push(ScriptList::new());
    file.register_parser(b"Script", &info.label, parse_script_from_list_data_chunk::<C>);
    file.register_parser(b"ScriptGroup", &info.label, parse_group_data_chunk::<C>);
    file.parse(ctx)
}

/// The list being read: the last one `ParseScriptListDataChunk` added.
fn current_list(st: &mut ScriptsState) -> Result<&mut ScriptList, MapDataError> {
    st.read_info.as_mut().and_then(|ri| ri.read_lists.last_mut()).ok_or(MapUndefined::NullUserData.into())
}

/// `ParseGroupDataChunk`.
fn parse_group_data_chunk<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> ChunkResult<bool> {
    let mut group = ScriptGroup::new(&mut ctx.scripts_state().cur_id);
    group.group_name = file.read_ascii_string()?;
    group.is_group_active = file.read_byte()? != 0;
    if info.version == version::SCRIPT_GROUP_2 {
        group.is_group_subroutine = file.read_byte()? != 0;
    }
    let st = ctx.scripts_state();
    match current_list(st) {
        Ok(l) => l.add_group(group, AT_END),
        Err(e) => return Err(to_chunk_err(e, st)),
    }
    file.register_parser(b"Script", &info.label, parse_script_from_group_data_chunk::<C>);
    file.parse(ctx)
}

/// `ParseScript`: the fields of a `Script` chunk, then its children. `None`
/// if the children failed to parse (upstream returns `NULL`).
fn parse_script<C: ScriptsContext>(file: &mut DataChunkInput<C>, version: u16, ctx: &mut C) -> ChunkResult<Option<Script>> {
    let mut s = Script::new();
    s.script_name = file.read_ascii_string()?;
    s.comment = file.read_ascii_string()?;
    s.condition_comment = file.read_ascii_string()?;
    s.action_comment = file.read_ascii_string()?;
    s.is_active = file.read_byte()? != 0;
    s.is_one_shot = file.read_byte()? != 0;
    s.easy = file.read_byte()? != 0;
    s.normal = file.read_byte()? != 0;
    s.hard = file.read_byte()? != 0;
    s.is_subroutine = file.read_byte()? != 0;
    if version >= self::version::SCRIPT_2 {
        s.delay_evaluation_seconds = file.read_int()?;
    }
    file.register_parser(b"OrCondition", b"Script", parse_or_condition_data_chunk::<C>);
    file.register_parser(b"ScriptAction", b"Script", parse_action_data_chunk::<C>);
    file.register_parser(b"ScriptActionFalse", b"Script", parse_action_false_data_chunk::<C>);
    let st = ctx.scripts_state();
    let depth = st.script_stack.len();
    st.script_stack.push(s);
    let r = file.parse(ctx);
    let s = ctx.scripts_state().script_stack.split_off(depth).pop();
    Ok(if r? { s } else { None })
}

/// `ParseScriptFromListDataChunk`.
fn parse_script_from_list_data_chunk<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> ChunkResult<bool> {
    let script = parse_script(file, info.version, ctx)?;
    let st = ctx.scripts_state();
    let Some(script) = script else {
        return Err(to_chunk_err(MapUndefined::ScriptParseFailed.into(), st));
    };
    match current_list(st) {
        Ok(l) => l.add_script(script, AT_END),
        Err(e) => return Err(to_chunk_err(e, st)),
    }
    Ok(true)
}

/// `ParseScriptFromGroupDataChunk`.
fn parse_script_from_group_data_chunk<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> ChunkResult<bool> {
    let script = parse_script(file, info.version, ctx)?;
    let st = ctx.scripts_state();
    let Some(script) = script else {
        return Err(to_chunk_err(MapUndefined::ScriptParseFailed.into(), st));
    };
    // the group being read: the last of the list being read
    match current_list(st).map(|l| l.groups.last_mut()) {
        Ok(Some(g)) => g.add_script(script, AT_END),
        Ok(None) => return Err(to_chunk_err(MapUndefined::NullUserData.into(), st)),
        Err(e) => return Err(to_chunk_err(e, st)),
    }
    Ok(true)
}

/// The script being read.
fn current_script(st: &mut ScriptsState) -> Result<&mut Script, MapDataError> {
    st.script_stack.last_mut().ok_or(MapUndefined::NullUserData.into())
}

/// `ParseOrConditionDataChunk`.
fn parse_or_condition_data_chunk<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> ChunkResult<bool> {
    let st = ctx.scripts_state();
    match current_script(st) {
        Ok(s) => s.conditions.push(OrCondition::default()),
        Err(e) => return Err(to_chunk_err(e, st)),
    }
    file.register_parser(b"Condition", &info.label, parse_condition_data_chunk::<C>);
    file.parse(ctx)
}

/// `ParseConditionDataChunk`.
fn parse_condition_data_chunk<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> ChunkResult<bool> {
    match read_condition(file, info, ctx) {
        Ok(()) => Ok(true),
        Err(e) => Err(to_chunk_err(e, ctx.scripts_state())),
    }
}

/// The body of `ParseConditionDataChunk`.
fn read_condition<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> MapResult<()> {
    let mut c = Condition::new_for_read();
    c.condition_type = file.read_int()?;
    if !(0..=127).contains(&c.condition_type) {
        return Err(MapUndefined::ConditionTypeOutOfRange.into());
    }
    // `ct`: the template of the type read, then of whatever the search for
    // the name key last looked at
    let mut ct = ctx.templates().get_condition_template(c.condition_type).clone();
    if info.version >= version::SCRIPT_CONDITION_4 {
        let key = file.read_name_key(ctx.names())?;
        let mut matched = ct.internal_name_key == key;
        if !matched {
            for i in 0..condition_type::NUM_ITEMS {
                ct = ctx.templates().get_condition_template(i).clone();
                if key == ct.internal_name_key {
                    matched = true;
                    c.condition_type = i;
                    break;
                }
            }
        }
        if !matched {
            c.condition_type = condition_type::CONDITION_FALSE;
            c.num_parms = 0;
        }
    }
    c.num_parms = file.read_int()?;
    for i in 0..c.num_parms.max(0) as usize {
        let p = Parameter::read_parameter(file)?;
        if i >= MAX_PARMS {
            return Err(MapUndefined::ParameterIndexOutOfRange.into());
        }
        c.parms[i] = Some(p);
    }

    if file.get_chunk_version() < version::SCRIPT_CONDITION_2 {
        // the conditions that gained a surfaces-allowed parameter in version 2
        for j in [
            condition_type::TEAM_INSIDE_AREA_PARTIALLY,
            condition_type::TEAM_INSIDE_AREA_ENTIRELY,
            condition_type::TEAM_OUTSIDE_AREA_ENTIRELY,
            condition_type::TEAM_ENTERED_AREA_PARTIALLY,
            condition_type::TEAM_ENTERED_AREA_ENTIRELY,
            condition_type::TEAM_EXITED_AREA_ENTIRELY,
            condition_type::TEAM_EXITED_AREA_PARTIALLY,
        ] {
            if c.condition_type == j {
                // m_parms[m_numParms] = ...; m_numParms = 3
                let at = usize::try_from(c.num_parms).ok().filter(|&n| n < MAX_PARMS).ok_or(MapUndefined::ParameterIndexOutOfRange)?;
                c.parms[at] = Some(Parameter::new(parameter_type::SURFACES_ALLOWED, 3));
                c.num_parms = 3;
            }
        }
    }
    // heal old files
    if c.condition_type == condition_type::SKIRMISH_SPECIAL_POWER_READY && c.num_parms == 1 {
        c.num_parms = 2;
        c.parms[1] = c.parms[0].take();
        let mut side = Parameter::new(parameter_type::SIDE, 0);
        side.string = THIS_PLAYER.to_vec();
        c.parms[0] = Some(side);
    }
    if ct.get_num_parameters() != c.num_parms {
        // Invalid script: make it false
        c.condition_type = condition_type::CONDITION_FALSE;
        c.num_parms = 0;
        c.parms = empty_parms();
    }
    let st = ctx.scripts_state();
    let or = current_script(st)?.conditions.last_mut().ok_or(MapUndefined::NullUserData)?;
    or.conditions.push(c);
    Ok(())
}

/// `ParseActionDataChunk`.
fn parse_action_data_chunk<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> ChunkResult<bool> {
    parse_action_into(file, info, ctx, false)
}

/// `ParseActionFalseDataChunk`.
fn parse_action_false_data_chunk<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> ChunkResult<bool> {
    parse_action_into(file, info, ctx, true)
}

fn parse_action_into<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C, false_list: bool) -> ChunkResult<bool> {
    let a = match read_action(file, info, ctx) {
        Ok(a) => a,
        Err(e) => return Err(to_chunk_err(e, ctx.scripts_state())),
    };
    let st = ctx.scripts_state();
    match current_script(st) {
        Ok(s) => {
            if false_list {
                s.actions_false.push(a);
            } else {
                s.actions.push(a);
            }
            Ok(true)
        }
        Err(e) => Err(to_chunk_err(e, st)),
    }
}

/// `ScriptAction::ParseAction`.
fn read_action<C: ScriptsContext>(file: &mut DataChunkInput<C>, info: &DataChunkInfo, ctx: &mut C) -> MapResult<ScriptAction> {
    use action_type as at;
    let mut a = ScriptAction::new_for_read();
    a.action_type = file.read_int()?;
    if !(0..=511).contains(&a.action_type) {
        return Err(MapUndefined::ActionTypeOutOfRange.into());
    }
    let mut t = ctx.templates().get_action_template(a.action_type).clone();
    if info.version >= version::SCRIPT_ACTION_2 {
        let key = file.read_name_key(ctx.names())?;
        let mut matched = t.internal_name_key == key;
        if !matched {
            for i in 0..action_type::NUM_ITEMS {
                t = ctx.templates().get_action_template(i).clone();
                if key == t.internal_name_key {
                    matched = true;
                    a.action_type = i;
                    break;
                }
            }
            if !matched {
                a.action_type = at::NO_OP;
                a.num_parms = 0;
            }
        }
    }
    a.num_parms = file.read_int()?;
    for i in 0..a.num_parms.max(0) as usize {
        let p = Parameter::read_parameter(file)?;
        if i >= MAX_PARMS {
            return Err(MapUndefined::ParameterIndexOutOfRange.into());
        }
        a.parms[i] = Some(p);
    }

    // heal old files
    let new_p = |t: i32, v: i32| Some(Parameter::new(t, v));
    match a.action_type {
        at::SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST => {
            if a.num_parms == 1 {
                a.num_parms = 2;
                a.parms[1] = a.parms[0].take();
                let mut side = Parameter::new(parameter_type::SIDE, 0);
                side.string = THIS_PLAYER.to_vec();
                a.parms[0] = Some(side);
            }
        }
        at::TEAM_FOLLOW_WAYPOINTS => {
            if a.num_parms == 2 {
                a.num_parms = 3;
                a.parms[2] = new_p(parameter_type::BOOLEAN, 1);
            }
        }
        at::SKIRMISH_BUILD_BASE_DEFENSE_FRONT => {
            if a.num_parms == 1 {
                let flank = a.parms[0].as_ref().ok_or(MapUndefined::NullParameter)?.int != 0;
                a.parms[0] = None;
                a.num_parms = 0;
                if flank {
                    a.action_type = at::SKIRMISH_BUILD_BASE_DEFENSE_FLANK;
                }
            }
        }
        at::NAMED_SET_ATTITUDE | at::TEAM_SET_ATTITUDE => {
            if a.num_parms >= 2 {
                let p1 = a.parms[1].as_ref().ok_or(MapUndefined::NullParameter)?;
                if p1.param_type == parameter_type::INT {
                    a.parms[1] = new_p(parameter_type::AI_MOOD, p1.int);
                }
            }
        }
        at::MAP_REVEAL_AT_WAYPOINT | at::MAP_SHROUD_AT_WAYPOINT => {
            if a.num_parms == 2 {
                a.num_parms = 3;
                a.parms[2] = new_p(parameter_type::SIDE, 0);
            }
        }
        at::MAP_REVEAL_ALL | at::MAP_REVEAL_ALL_PERM | at::MAP_REVEAL_ALL_UNDO_PERM | at::MAP_SHROUD_ALL => {
            if a.num_parms == 0 {
                a.num_parms = 1;
                a.parms[0] = new_p(parameter_type::SIDE, 0);
            }
        }
        at::SPEECH_PLAY => {
            if a.num_parms == 1 {
                a.num_parms = 2;
                // default it to TRUE
                a.parms[1] = new_p(parameter_type::BOOLEAN, 1);
            }
            // (upstream falls through into the next case, whose test is for
            // one parameter, which this case's repair has just ruled out)
        }
        at::CAMERA_MOD_SET_FINAL_ZOOM | at::CAMERA_MOD_SET_FINAL_PITCH => {
            if a.num_parms == 1 {
                a.num_parms = 3;
                a.parms[1] = new_p(parameter_type::PERCENT, 0);
                a.parms[2] = new_p(parameter_type::PERCENT, 0);
            }
        }
        at::MOVE_CAMERA_TO | at::MOVE_CAMERA_ALONG_WAYPOINT_PATH | at::CAMERA_LOOK_TOWARD_OBJECT => {
            if a.num_parms == 3 {
                a.num_parms = 5;
                a.parms[3] = new_p(parameter_type::REAL, 0);
                a.parms[4] = new_p(parameter_type::REAL, 0);
            }
        }
        at::RESET_CAMERA | at::ZOOM_CAMERA | at::PITCH_CAMERA | at::ROTATE_CAMERA => {
            if a.num_parms == 2 {
                a.num_parms = 4;
                a.parms[2] = new_p(parameter_type::REAL, 0);
                a.parms[3] = new_p(parameter_type::REAL, 0);
            }
        }
        at::CAMERA_LOOK_TOWARD_WAYPOINT => {
            if a.num_parms == 2 {
                a.num_parms = 5;
                a.parms[2] = new_p(parameter_type::REAL, 0);
                a.parms[3] = new_p(parameter_type::REAL, 0);
                a.parms[4] = new_p(parameter_type::BOOLEAN, 0);
            } else if a.num_parms == 4 {
                a.num_parms = 5;
                a.parms[4] = new_p(parameter_type::BOOLEAN, 0);
            }
        }
        _ => {}
    }

    if t.get_num_parameters() != a.num_parms {
        // Invalid script: make it a no-op
        a.action_type = at::NO_OP;
        a.num_parms = 0;
        a.parms = empty_parms();
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::data_chunk::CachedFileInputStream;

    /// A template table where every condition and action has a name of its
    /// own and the parameter types of `types(i)`.
    fn templates(names: &mut NameKeyGenerator, types: impl Fn(i32) -> Vec<i32>) -> ScriptTemplates {
        let conds = (0..condition_type::NUM_ITEMS)
            .map(|i| ScriptTemplate { internal_name_key: names.name_to_key(format!("Cond{i}").as_bytes()), parameters: types(i) })
            .collect();
        let acts = (0..action_type::NUM_ITEMS)
            .map(|i| ScriptTemplate { internal_name_key: names.name_to_key(format!("Act{i}").as_bytes()), parameters: types(i + 1000) })
            .collect();
        ScriptTemplates::new(conds, acts).expect("table sizes")
    }

    fn ctx() -> ScriptsCtx {
        let mut names = NameKeyGenerator::new();
        names.init();
        // Cond/Act 3: (Side, Int, String); Cond/Act 2000+: n/a
        let templates = templates(&mut names, |i| match i % 5 {
            0 => vec![],
            1 => vec![parameter_type::INT],
            2 => vec![parameter_type::SIDE, parameter_type::TEAM],
            3 => vec![parameter_type::SIDE, parameter_type::INT, parameter_type::SCRIPT],
            _ => vec![parameter_type::COORD3D],
        });
        ScriptsCtx { state: ScriptsState::new(), names, templates }
    }

    #[test]
    fn templates_clamp_and_check_sizes() {
        let c = ctx();
        assert_eq!(c.templates.get_condition_template(-1).internal_name_key, c.templates.get_condition_template(0).internal_name_key);
        assert_eq!(c.templates.get_action_template(344).internal_name_key, c.templates.get_action_template(0).internal_name_key);
        assert_eq!(c.templates.get_condition_template(3).get_parameter_type(1), parameter_type::INT);
        assert_eq!(c.templates.get_condition_template(3).get_parameter_type(7), parameter_type::INT);
        assert!(ScriptTemplates::new(vec![], vec![]).is_none());
    }

    #[test]
    fn group_numbering_and_insertion() {
        let mut id = 0;
        let a = ScriptGroup::new(&mut id);
        let b = ScriptGroup::new(&mut id);
        assert_eq!(a.group_name, b"Script Group 1");
        assert_eq!(b.group_name, b"Script Group 2");
        assert_eq!(id, 2);
        let mut l = ScriptList::new();
        let named = |n: &str| Script { script_name: n.as_bytes().to_vec(), ..Script::new() };
        l.add_script(named("a"), AT_END);
        l.add_script(named("b"), AT_END);
        l.add_script(named("c"), 0);
        l.add_script(named("d"), 2);
        l.add_script(named("e"), -5);
        let order: Vec<&[u8]> = l.scripts.iter().map(|s| &s.script_name[..]).collect();
        assert_eq!(order, [b"c" as &[u8], b"a", b"d", b"b", b"e"]);
        l.add_group(a, 7);
        l.add_group(b, 0);
        assert_eq!(l.groups[0].group_name, b"Script Group 2");
        assert!(l.delete_script(9).is_err());
        l.delete_script(1).unwrap();
        l.delete_group(0).unwrap();
        assert_eq!((l.scripts.len(), l.groups.len()), (4, 1));
        let (g, s) = l.discard();
        assert_eq!((g.len(), s.len()), (1, 4));
    }

    #[test]
    fn script_editing_helpers() {
        let c = ctx();
        let mut s = Script::new();
        s.conditions = vec![OrCondition::default(), OrCondition::default(), OrCondition::default()];
        s.conditions[1].conditions = vec![Condition::new(3, &c.templates), Condition::new(1, &c.templates)];
        s.actions = vec![ScriptAction::new(3, &c.templates), ScriptAction::new(4, &c.templates)];
        s.actions_false = vec![ScriptAction::new(3, &c.templates)];
        assert_eq!(s.find_previous_or_condition(0).unwrap(), None);
        assert_eq!(s.find_previous_or_condition(2).unwrap(), Some(1));
        assert!(s.find_previous_or_condition(3).is_err());
        assert_eq!(s.conditions[1].find_previous_condition(1).unwrap(), Some(0));
        let removed = s.conditions[1].remove_condition(0).unwrap();
        assert_eq!(removed.condition_type, 3);
        s.conditions[1].delete_condition(0).unwrap();
        assert!(s.conditions[1].delete_condition(0).is_err());
        s.delete_or_condition(0).unwrap();
        s.delete_action(1).unwrap();
        s.delete_false_action(0).unwrap();
        assert!(s.delete_action(1).is_err() && s.delete_false_action(0).is_err() && s.delete_or_condition(2).is_err());
        let mut src = Script::new();
        src.script_name = b"new".to_vec();
        src.is_active = false;
        src.actions = vec![ScriptAction::new(2, &c.templates)];
        s.update_from(&mut src);
        assert_eq!((s.script_name.as_slice(), s.is_active, s.actions.len(), s.conditions.len()), (&b"new"[..], false, 1, 0));
        assert!(src.actions.is_empty());
        assert_eq!(s.actions[0].get_parameter(1).map(|p| p.param_type), Some(parameter_type::TEAM));
        assert!(s.actions[0].get_parameter(2).is_none());
        // template 3 of the test table: (Side, Int, Script)
        let cond = Condition::new(3, &c.templates);
        assert_eq!(cond.get_parameter(0).map(|p| p.get_parameter_type()), Some(parameter_type::SIDE));
        assert!(cond.get_parameter(3).is_none() && cond.get_parameter(-1).is_none());
        let mut retyped = ScriptAction::new(2, &c.templates);
        retyped.set_action_type(3, &c.templates);
        assert_eq!(retyped.num_parms, 3, "action 3 has template 1003 % 5 = 3: three parameters");
    }

    #[test]
    fn qualify_by_parameter_type() {
        let mut p = Parameter::new(parameter_type::TEAM, 0);
        p.string = b"teamA".to_vec();
        p.qualify(b"_q", b"x", b"y");
        assert_eq!(p.string, b"teamA_q");
        p.string = THIS_TEAM.to_vec();
        p.qualify(b"_q", b"x", b"y");
        assert_eq!(p.string, THIS_TEAM);
        let mut s = Parameter::new(parameter_type::SIDE, 0);
        s.string = b"Plyr".to_vec();
        s.qualify(b"A", b"PlyrA", b"PlyrNew");
        assert_eq!(s.string, b"PlyrNew");
        s.qualify(b"A", b"PlyrA", b"PlyrNew");
        assert_eq!(s.string, b"PlyrNew", "PlyrNewA is not the template player");
        for t in [parameter_type::SCRIPT, parameter_type::COUNTER, parameter_type::FLAG, parameter_type::SCRIPT_SUBROUTINE] {
            let mut p = Parameter::new(t, 0);
            p.string = b"n".to_vec();
            p.qualify(b"!", b"", b"");
            assert_eq!(p.string, b"n!");
        }
        let mut other = Parameter::new(parameter_type::OBJECT_TYPE, 0);
        other.string = b"n".to_vec();
        other.qualify(b"!", b"", b"");
        assert_eq!(other.string, b"n");
        let mut c = Parameter::new(parameter_type::COORD3D, 0);
        c.set_coord3d(Coord3D { x: 1.0, y: 2.0, z: 3.0 });
        assert_eq!(c.get_coord3d().y, 2.0);
        let mut i = Parameter::new(parameter_type::INT, 0);
        i.set_coord3d(Coord3D { x: 1.0, y: 2.0, z: 3.0 });
        assert_eq!(i.get_coord3d(), Coord3D::default());
    }

    #[test]
    fn duplicate_copies_parameters_by_template() {
        let c = ctx();
        let mut cond = Condition::new(3, &c.templates);
        cond.parms[1].as_mut().unwrap().int = 77;
        cond.has_warnings = 4;
        let mut second = Condition::new(2, &c.templates);
        second.parms[0].as_mut().unwrap().string = b"P".to_vec();
        let d = Condition::duplicate_chain(&[cond.clone(), second], &c.templates).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].parms[1].as_ref().unwrap().int, 77);
        assert_eq!(d[0].has_warnings, 0, "editor flags are not copied");
        assert_eq!(d[1].parms[0].as_ref().unwrap().string, b"P");
        // a condition after the first with more parameters than its template
        let mut bad = Condition::new(3, &c.templates);
        bad.condition_type = 0; // template 0 has no parameters
        let e = Condition::duplicate_chain(&[cond, bad], &c.templates).unwrap_err();
        assert_eq!(e, MapDataError::Undefined(MapUndefined::NullParameter));
    }

    /// Chunk bytes for a parameter (type, int, real, string).
    fn parm(v: &mut Vec<u8>, t: i32, i: i32, r: f32, s: &[u8]) {
        v.extend_from_slice(&t.to_le_bytes());
        if t == parameter_type::COORD3D {
            for x in [1.0f32, 2.0, 3.0] {
                v.extend_from_slice(&x.to_le_bytes());
            }
            return;
        }
        v.extend_from_slice(&i.to_le_bytes());
        v.extend_from_slice(&r.to_le_bytes());
        v.extend_from_slice(&(s.len() as u16).to_le_bytes());
        v.extend_from_slice(s);
    }

    /// Read a `Parameter` straight from bytes.
    fn read_p(bytes: Vec<u8>) -> MapResult<Parameter> {
        let mut file: Vec<u8> = b"CkMp".to_vec();
        file.extend_from_slice(&0i32.to_le_bytes());
        file.extend(bytes);
        let mut input = DataChunkInput::<()>::new(CachedFileInputStream::new(file).unwrap()).unwrap();
        Parameter::read_parameter(&mut input)
    }

    #[test]
    fn read_parameter_repairs() {
        use parameter_type as pt;
        let mut b = Vec::new();
        parm(&mut b, pt::OBJECT_TYPE, 0, 0.0, b"FundamentalistInfantry");
        assert_eq!(read_p(b).unwrap().string, b"GLAInfantry");
        let mut b = Vec::new();
        parm(&mut b, pt::OBJECT_TYPE, 0, 0.0, &[b'F'; 300]);
        assert!(read_p(b).is_ok());
        let mut long = b"Fundamentalist".to_vec();
        long.extend(vec![b'x'; 300]);
        let mut b = Vec::new();
        parm(&mut b, pt::OBJECT_TYPE, 0, 0.0, &long);
        assert_eq!(read_p(b).unwrap_err(), MapDataError::Undefined(MapUndefined::NameTooLong));
        let mut b = Vec::new();
        parm(&mut b, pt::UPGRADE, 0, 0.0, b"Upgrade_ChinaRedguardCaptureBuilding");
        assert_eq!(read_p(b).unwrap().string, b"Upgrade_InfantryCaptureBuilding");
        let mut b = Vec::new();
        parm(&mut b, pt::OBJECT_STATUS, 0, 0.0, b"disguised");
        let p = read_p(b).unwrap();
        assert_eq!(p.object_status.count(), 1);
        let mut b = Vec::new();
        parm(&mut b, 64, 0, 0.0, b"");
        assert_eq!(read_p(b).unwrap_err(), MapDataError::Undefined(MapUndefined::ParameterTypeOutOfRange));
        let mut b = Vec::new();
        parm(&mut b, pt::COORD3D, 0, 0.0, b"");
        let p = read_p(b).unwrap();
        assert!(p.initialized && p.get_coord3d().z == 3.0);
    }

    #[test]
    fn read_parameter_kind_of() {
        use parameter_type as pt;
        let names = KindOfMaskType::get_bit_names();
        let small = names.iter().position(|n| *n == "SMALL_MISSILE").expect("name exists") as i32;
        for (s, want) in [
            ("selectable", 1),
            ("CRUSHER", 0),
            ("Crushable", 0),
            ("OVERLAPPABLE", 0),
            ("MISSILE", small),
        ] {
            let mut b = Vec::new();
            parm(&mut b, pt::KIND_OF_PARAM, 99, 0.0, s.as_bytes());
            assert_eq!(read_p(b).unwrap().int, want, "{s}");
        }
        let mut b = Vec::new();
        parm(&mut b, pt::KIND_OF_PARAM, 0, 0.0, b"NOT_A_KIND");
        assert_eq!(read_p(b).unwrap_err(), MapDataError::Thrown(ERROR_BUG));
        // empty string: the index names itself, the terminator is empty
        let mut b = Vec::new();
        parm(&mut b, pt::KIND_OF_PARAM, 2, 0.0, b"");
        assert_eq!(read_p(b).unwrap().string, names[2].as_bytes());
        let mut b = Vec::new();
        parm(&mut b, pt::KIND_OF_PARAM, names.len() as i32, 0.0, b"");
        assert_eq!(read_p(b).unwrap().string, b"");
        let mut b = Vec::new();
        parm(&mut b, pt::KIND_OF_PARAM, -1, 0.0, b"");
        assert_eq!(read_p(b).unwrap_err(), MapDataError::Undefined(MapUndefined::KindOfIndexOutOfRange));
    }

    #[test]
    fn write_parameter_replaces_the_kind_of_string() {
        let mut p = Parameter::new(parameter_type::KIND_OF_PARAM, 1);
        p.string = b"junk".to_vec();
        let mut out = DataChunkOutput::new();
        p.write_parameter(&mut out);
        assert_eq!(p.string, KindOfMaskType::get_bit_names()[1].as_bytes());
        let mut q = Parameter::new(parameter_type::KIND_OF_PARAM, 5000);
        q.string = b"junk".to_vec();
        q.write_parameter(&mut out);
        assert_eq!(q.string, b"");
        let mut c = Parameter::new(parameter_type::COORD3D, 0);
        c.set_coord3d(Coord3D { x: 1.0, y: 2.0, z: 3.0 });
        c.write_parameter(&mut out);
        assert!(!out.finish().is_empty());
    }

    /// A `.scb` file: one script list, one script with the given parts.
    fn scb(names: &[&str], chunks: Vec<u8>) -> Vec<u8> {
        let mut v = b"CkMp".to_vec();
        v.extend_from_slice(&(names.len() as i32).to_le_bytes());
        for (i, n) in names.iter().enumerate() {
            v.push(n.len() as u8);
            v.extend_from_slice(n.as_bytes());
            v.extend_from_slice(&(i as u32 + 1).to_le_bytes());
        }
        v.extend(chunks);
        v
    }

    fn chunk(id: u32, ver: u16, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&ver.to_le_bytes());
        v.extend_from_slice(&(body.len() as i32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    const SCB_NAMES: [&str; 7] = ["PlayerScriptsList", "ScriptList", "Script", "OrCondition", "Condition", "ScriptAction", "ScriptGroup"];

    fn script_body(tail: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        for s in ["name", "c1", "c2", "c3"] {
            b.extend_from_slice(&(s.len() as u16).to_le_bytes());
            b.extend_from_slice(s.as_bytes());
        }
        b.extend_from_slice(&[1, 0, 1, 1, 0, 1]);
        b.extend_from_slice(&9i32.to_le_bytes());
        b.extend_from_slice(tail);
        b
    }

    fn parse_scb(c: &mut ScriptsCtx, file: Vec<u8>) -> MapResult<bool> {
        let mut input = DataChunkInput::<ScriptsCtx>::new(CachedFileInputStream::new(file).unwrap())?;
        input.register_parser(b"PlayerScriptsList", b"", parse_scripts_data_chunk::<ScriptsCtx>);
        let r = input.parse(c);
        c.state.finish(r)
    }

    /// Version 1 conditions and old actions are repaired as upstream does,
    /// a bad count makes them false / no-ops, and the lists read can be
    /// taken once and written back.
    #[test]
    fn parse_repairs_old_files_and_round_trips() {
        let mut c = ctx();
        // condition type 7 (TEAM_INSIDE_AREA_PARTIALLY) of template 7 % 5 = 2: (Side, Team)
        let mut cond = 7i32.to_le_bytes().to_vec();
        cond.extend_from_slice(&2i32.to_le_bytes());
        parm(&mut cond, parameter_type::SIDE, 0, 0.0, b"s");
        parm(&mut cond, parameter_type::TEAM, 0, 0.0, b"t");
        // action type 6 = template 1006: 1006 % 5 = 1 -> one Int; written with two parameters: no-op
        let mut act = 6i32.to_le_bytes().to_vec();
        act.extend_from_slice(&2i32.to_le_bytes());
        parm(&mut act, parameter_type::INT, 1, 0.0, b"");
        parm(&mut act, parameter_type::INT, 2, 0.0, b"");
        let mut tail = chunk(4, 1, &chunk(5, 1, &cond));
        // Condition chunk (id 5) of version 1 inside OrCondition (id 4): needs the surfaces parameter
        // -> count 3 != template count 2, so false with no parameters
        tail.extend(chunk(6, 1, &act));
        let list = chunk(2, 1, &chunk(3, 2, &script_body(&tail)));
        let file = scb(&SCB_NAMES, chunk(1, 5, &list));
        assert!(parse_scb(&mut c, file).unwrap());
        assert_eq!(c.state.num_in_read_list(), 1);
        let mut lists = ScriptList::get_read_scripts(&mut c.state);
        assert_eq!(c.state.num_in_read_list(), 0);
        assert!(ScriptList::get_read_scripts(&mut c.state).is_empty());
        let list = lists[0].as_mut().expect("one list");
        assert_eq!(list.scripts.len(), 1);
        let s = &list.scripts[0];
        assert_eq!((&s.script_name[..], s.delay_evaluation_seconds, s.is_one_shot, s.easy, s.normal, s.hard, s.is_subroutine), (&b"name"[..], 9, false, true, true, false, true));
        let cd = &s.conditions[0].conditions[0];
        assert_eq!((cd.condition_type, cd.num_parms), (condition_type::CONDITION_FALSE, 0));
        assert_eq!(s.actions.len(), 1);
        assert_eq!((s.actions[0].action_type, s.actions[0].num_parms), (action_type::NO_OP, 0));
        // write it back
        let mut out = DataChunkOutput::new();
        let mut refs = [Some(list)];
        ScriptList::write_scripts_data_chunk(&mut out, &mut refs, &c.templates, &c.names).unwrap();
        let bytes = out.finish();
        assert_eq!(&bytes[..4], b"CkMp");
        let again = {
            let mut c2 = ctx();
            c2.names = c.names.clone();
            assert!(parse_scb(&mut c2, bytes).unwrap());
            ScriptList::get_read_scripts(&mut c2.state)
        };
        assert_eq!(again.len(), 1);
        let s2 = &again[0].as_ref().unwrap().scripts[0];
        assert_eq!(s2.script_name, b"name");
        assert_eq!(s2.conditions[0].conditions.len(), 1);
    }

    #[test]
    fn parse_matches_template_names_and_heals_actions() {
        let mut c = ctx();
        // version 4 condition naming Cond12 (template 12 % 5 = 2: Side, Team) but typed 3
        let key = c.names.name_to_key(b"Cond12");
        let mut names = SCB_NAMES.to_vec();
        names.push("Cond12");
        let mut cond = 3i32.to_le_bytes().to_vec();
        cond.extend_from_slice(&(((SCB_NAMES.len() as i32 + 1) << 8) | 3).to_le_bytes());
        cond.extend_from_slice(&2i32.to_le_bytes());
        parm(&mut cond, parameter_type::SIDE, 0, 0.0, b"s");
        parm(&mut cond, parameter_type::TEAM, 0, 0.0, b"t");
        // action 36 (TEAM_FOLLOW_WAYPOINTS), template 1036 % 5 = 1: wait, make it match: 3 params needed
        let mut c3 = ctx();
        c3.templates = {
            let mut n = NameKeyGenerator::new();
            n.init();
            templates(&mut n, |i| if i == 1036 { vec![parameter_type::SCRIPT, parameter_type::SCRIPT, parameter_type::BOOLEAN] } else { vec![] })
        };
        let mut act = 36i32.to_le_bytes().to_vec();
        act.extend_from_slice(&2i32.to_le_bytes());
        parm(&mut act, parameter_type::SCRIPT, 0, 0.0, b"a");
        parm(&mut act, parameter_type::SCRIPT, 0, 0.0, b"b");
        let tail = chunk(6, 1, &act);
        let list = chunk(2, 1, &chunk(3, 2, &script_body(&tail)));
        let file = scb(&SCB_NAMES, chunk(1, 5, &list));
        assert!(parse_scb(&mut c3, file).unwrap());
        let lists = ScriptList::get_read_scripts(&mut c3.state);
        let a = &lists[0].as_ref().unwrap().scripts[0].actions[0];
        assert_eq!((a.action_type, a.num_parms), (36, 3));
        let p = a.parms[2].as_ref().unwrap();
        assert_eq!((p.param_type, p.int), (parameter_type::BOOLEAN, 1));
        // the named condition is found by key
        let mut c4 = ctx();
        let mut c4names = c4.names.clone();
        assert_eq!(c4names.name_to_key(b"Cond12"), key);
        let tail = chunk(4, 1, &chunk(5, 4, &cond));
        let list = chunk(2, 1, &chunk(3, 2, &script_body(&tail)));
        let file = scb(&names, chunk(1, 5, &list));
        assert!(parse_scb(&mut c4, file).unwrap());
        let lists = ScriptList::get_read_scripts(&mut c4.state);
        let cd = &lists[0].as_ref().unwrap().scripts[0].conditions[0].conditions[0];
        assert_eq!((cd.condition_type, cd.num_parms), (12, 2));
    }

    #[test]
    fn too_many_lists_fail_and_leave_the_count() {
        let mut c = ctx();
        // (3 bytes of data: a chunk header ending the file is not parsed, upstream's `parse`
        // stops at the end of the file after opening a chunk)
        let lists: Vec<u8> = (0..17).flat_map(|_| chunk(2, 1, &[0; 3])).collect();
        let file = scb(&SCB_NAMES, chunk(1, 5, &lists));
        assert!(!parse_scb(&mut c, file).unwrap());
        assert_eq!(c.state.num_in_read_list(), 0);
        let lists: Vec<u8> = (0..16).flat_map(|_| chunk(2, 1, &[0; 3])).collect();
        let file = scb(&SCB_NAMES, chunk(1, 5, &lists));
        assert!(parse_scb(&mut c, file).unwrap());
        assert_eq!(c.state.num_in_read_list(), 16);
        // a second parse before taking them drops the leftovers; a failure then
        // keeps the count with the lists gone
        let lists: Vec<u8> = (0..17).flat_map(|_| chunk(2, 1, &[0; 3])).collect();
        let file = scb(&SCB_NAMES, chunk(1, 5, &lists));
        assert!(!parse_scb(&mut c, file).unwrap());
        assert_eq!(c.state.num_in_read_list(), 16);
        let taken = ScriptList::get_read_scripts(&mut c.state);
        assert_eq!((taken.len(), taken.iter().filter(|l| l.is_some()).count()), (16, 0));
    }

    #[test]
    fn groups_are_read_with_their_scripts() {
        let mut c = ctx();
        let mut grp = Vec::new();
        grp.extend_from_slice(&5u16.to_le_bytes());
        grp.extend_from_slice(b"Group");
        grp.extend_from_slice(&[0, 1]);
        grp.extend(chunk(3, 2, &script_body(&[])));
        let list = [chunk(7, 2, &grp), chunk(3, 2, &script_body(&[]))].concat();
        let file = scb(&SCB_NAMES, chunk(1, 5, &chunk(2, 1, &list)));
        assert!(parse_scb(&mut c, file).unwrap());
        assert_eq!(c.state.cur_id, 1, "a group's constructor takes a number");
        let lists = ScriptList::get_read_scripts(&mut c.state);
        let l = lists[0].as_ref().unwrap();
        assert_eq!((l.groups.len(), l.scripts.len()), (1, 1));
        let g = &l.groups[0];
        assert_eq!((&g.group_name[..], g.is_group_active, g.is_group_subroutine, g.scripts.len()), (&b"Group"[..], false, true, 1));
        // version 1 groups have no subroutine byte
        let mut grp = Vec::new();
        grp.extend_from_slice(&1u16.to_le_bytes());
        grp.extend_from_slice(b"G");
        grp.push(1);
        let file = scb(&SCB_NAMES, chunk(1, 5, &chunk(2, 1, &chunk(7, 1, &grp))));
        assert!(parse_scb(&mut c, file).unwrap());
        let lists = ScriptList::get_read_scripts(&mut c.state);
        assert!(!lists[0].as_ref().unwrap().groups[0].is_group_subroutine);
    }

    #[test]
    fn duplicates_and_qualifies_whole_lists() {
        let mut c = ctx();
        let mut sc = Script::new();
        sc.script_name = b"S".to_vec();
        sc.is_active = false;
        let mut cond = Condition::new(2, &c.templates);
        cond.parms[0].as_mut().unwrap().string = b"PlyrA".to_vec();
        cond.parms[1].as_mut().unwrap().string = b"team".to_vec();
        sc.conditions = vec![OrCondition { conditions: vec![cond] }];
        sc.actions = vec![ScriptAction::new(2, &c.templates)];
        sc.actions[0].parms[0].as_mut().unwrap().string = b"Plyr".to_vec();
        let mut g = ScriptGroup::new(&mut c.state.cur_id);
        g.group_name = b"G".to_vec();
        g.scripts.push(sc.clone());
        let list = ScriptList { groups: vec![g], scripts: vec![sc] };
        let mut ids = 10;
        let d = list.duplicate(&mut ids, &c.templates).unwrap();
        assert_eq!(ids, 11);
        assert_eq!(d.scripts[0].script_name, b"S");
        assert!(!d.scripts[0].is_active);
        let q = list.duplicate_and_qualify(&mut ids, &c.templates, b"_2", b"PlyrA", b"PlyrB").unwrap();
        assert_eq!(ids, 12);
        assert_eq!(q.groups[0].group_name, b"G_2");
        assert_eq!(q.scripts[0].script_name, b"S_2");
        let cc = &q.scripts[0].conditions[0].conditions[0];
        assert_eq!(cc.parms[0].as_ref().unwrap().string, b"PlyrA", "Plyr + _2 is not PlyrA");
        assert_eq!(cc.parms[1].as_ref().unwrap().string, b"team_2");
        let a = &q.scripts[0].actions[0];
        assert_eq!(a.parms[0].as_ref().unwrap().string, b"Plyr", "SIDE: Plyr + _2 != PlyrA");
        assert_eq!(OBJECT_FLAGS_NAMES[6], "Player Targetable");
    }
}
