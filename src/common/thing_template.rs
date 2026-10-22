// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/ThingTemplate.h
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/ThingTemplate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ArmorSet.h (`ArmorTemplateSet`)
//   GeneralsMD/Code/GameEngine/Include/Common/Radar.h (`RadarPriorityType`)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Shadow.h (`ShadowType`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, April 2001 (ThingTemplate.h/.cpp)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ThingTemplate`: the "roadmap to creating things", one per `Object` (or
//! `ObjectReskin`) block of the game's INI. Everything a thing is made of:
//! its names, its kinds, geometry, vision and shroud ranges, build cost and
//! prerequisites, experience values, armor and weapon sets, the module lists
//! (behavior, draw and client update), the 36 sounds, and the editor and
//! shadow settings.
//!
//! This file has the template, the `Object` field table (**110 fields**,
//! [`OBJECT_FIELD_NAMES`]) and the `ObjectReskin` table (**10**,
//! [`RESKIN_FIELD_NAMES`]), the module lists with their add, remove, replace,
//! inheritable and override-by-like-kind rules, and the template's own logic.
//! The template store and the `Object` block handler are in
//! [`super::thing_factory`].
//!
//! # Modules go through `ModuleFactory`; an untranslated name is an error
//!
//! `Behavior = Name Tag` and `Body = Name Tag` ask [`ModuleFactory`] for the
//! name's interface mask (`findModuleInterfaceMask`) and for its data
//! (`newModuleDataFromINI`, which reads the module's sub-block, tags the
//! data and keeps it). A name the factory registers but whose class is not
//! translated yet ([`ModuleFactoryError::NotTranslated`]) stops the template
//! load with [`IniErrorKind::ModuleNotTranslated`]; it is never skipped, since
//! a skipped module would be a thing without its behavior. (Upstream cannot
//! fail there: every class exists. None is translated yet, so today every
//! `Behavior` and `Body` line is this error; layer 9 adds the classes.) A
//! name the factory has never registered is [`IniErrorKind::ModuleUnknown`]
//! (upstream gets a null `ModuleData` from `newModuleDataFromINI` and calls
//! `isAiModuleData()` on it).
//!
//! # Client fields are data
//!
//! - **`Draw` and `ClientUpdate`** are drawable modules. Their classes are
//!   the client's (`W3DModuleFactory` registers the draw modules; the game
//!   logic factory knows only four client update names, all untranslated)
//!   and nothing in logic interprets them. The name, tag and interface mask
//!   (`MODULEINTERFACE_DRAW` / `MODULEINTERFACE_CLIENT_UPDATE`) are kept, and
//!   the block's lines as [`ClientModuleData`]: field, arguments and nesting
//!   depth, each field as written. The reader needs one fact to find the
//!   block's `End`: the four fields that open a nested block
//!   (`DefaultConditionState`, `ConditionState`, `AliasConditionState` and
//!   `TransitionState` of `W3DModelDraw`; no other client module has one,
//!   checked in `GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw`
//!   and `GameClient/Drawable/Update`). Anything else on a line is not
//!   validated; that is the client's job. The factory's name keys are
//!   assigned as upstream assigns them (the decorated key of the name, then
//!   the key of the tag after the block is read).
//! - **Names that the client resolves later**: `SelectPortrait`,
//!   `ButtonImage`, `UpgradeCameo1`..`5`, `ShadowTexture` (names, as
//!   upstream), the 36 sounds (`DynamicAudioEventRTS`: the event name; the
//!   `getInfoForAudioEvent` call is the client's), `UnitSpecificSounds` (the
//!   event name of each entry) and `UnitSpecificFX` (the `FXList` name, or
//!   `None`; upstream keeps the pointer or null, here an unknown name is kept
//!   as a name), `DisplayName` (the translated label).
//! - `resolveNames` looks the images up in `TheMappedImageCollection` when
//!   there is one and clears the names; with no client it leaves them, which
//!   is what [`ThingFactory::resolve_names`](super::thing_factory::ThingFactory)
//!   does.
//!
//! # Translation choices
//!
//! - **Pointers.** `m_reskinnedFrom` and the identity that `this ==`
//!   comparisons need are [`ThingTemplate::node_id`]s, assigned by the
//!   factory to every template object it makes (a template and each of its
//!   override copies is one object upstream). The template's `m_templateID`
//!   is kept as it is. A prerequisite's unit is a template id.
//! - **`m_nextThingTemplate`** (the factory's linked list) is the factory's
//!   `Vec`.
//! - **Strings** are byte vectors. **Uninitialised members.** The
//!   constructor leaves `m_assetScale`, `m_buildable`, `m_isBridge`,
//!   `m_isForbidden`, `m_displayColor`, `m_editorSorting`, the two
//!   `...CopiedFromDefault` flags and the `m_info` of `ModuleInfo` to the
//!   memory pool, which zero-fills every allocation (`GameMemory.cpp`):
//!   they are 0 here.
//! - **`m_moduleParsingMode` is not restored by a throw.** `AddModule`,
//!   `RemoveModule`, `ReplaceModule`, `InheritableModule` and
//!   `OverrideableByLikeKind` set the mode, read, and restore it; if the read
//!   throws, the mode stays (and for `ReplaceModule` so do the two
//!   `m_moduleBeingReplaced...` strings). A later block that reads the same
//!   template, or a later field of an `AddModule` retry, sees it. Kept.
//! - **Debug-only code is not translated**: `addModuleInfo`'s check that tags
//!   are unique (`#if defined(_DEBUG)`), the duplicate-condition checks of
//!   the armor and weapon sets, the `validate()` checks (immobile structures,
//!   armor on structures) and `validateAudio()`.
//! - **`std::map<AsciiString, ...>`** (`PerUnitSoundMap`, `PerUnitFXMap`)
//!   are [`BTreeMap`]s ordered by bytes, as `strcmp` orders. `insert`
//!   keeps the first value of a key, as `std::map::insert` does.
//! - Template copies. `*newTemplate = *defaultT` (`ThingFactory::newTemplate`,
//!   `newOverride`) and `copyFrom` copy the whole object, including the
//!   `Overridable` base's `m_nextOverride` and `m_isOverride`; see
//!   [`super::thing_factory`] for what that means for the Rust.
//!
//! # Cut points
//!
//! Needing a `Player`, `TheGameLogic` or an `Object` (layers 7 and 8), not
//! translated: `getBuildFacilityTemplate(player)`, `calcTimeToBuild(player)`,
//! `calcCostToBuild(player)` (they ask the player's production changes, handicap,
//! energy and factory counts), and `getBuildable()`'s script override
//! (`TheGameLogic->findBuildableStatusOverride`; [`ThingTemplate::buildable`]
//! is the template's own value) and `getMaxSimultaneousOfType()`'s
//! superweapon restriction (a parameter). `friend_getAIModuleInfo()` and
//! the `Locomotor` field (`AIUpdateModuleData::parseLocomotorSet`) need
//! `AIUpdateModuleData` (layer 9): no AI module data exists yet, so the
//! field is `INI_INVALID_DATA`, which is also what upstream throws for a
//! template without an `AIUpdate` module ("Attempted to specify a locomotor
//! ... without an AIUpdate block"). `getDisplayColor`, the editor sorting, the
//! shadow and the radar priority are plain data. `ProductionPrerequisite`'s
//! player-dependent members are in [`super::production_prerequisite`].
//!
//! # Where upstream is undefined
//!
//! - `Shadow = +NAME` / `-NAME` (or no tokens): `parseBitString8` reads an
//!   uninitialised variable ([`Undefined::UninitialisedBitString8`]).
//! - Integers outside `Int` and the other [`Undefined`] cases of the INI
//!   field parsers.
//! - A template copied from one that has an override chain, and `reset()`
//!   deleting a template from the middle of the list: see
//!   [`super::thing_factory`].
//!
//! # Verified
//!
//! Against upstream `ThingTemplate.cpp` and `ThingFactory.cpp`, compiled
//! unchanged, by `tests/differential_thing_template.rs` (see there for the
//! method and the results).

use std::collections::BTreeMap;

use crate::common::audio_event_name::AudioEventName;
use crate::common::game_common::{VeterancyLevel, LEVEL_COUNT};
use crate::common::geometry::{
    parse_geometry_height, parse_geometry_is_small, parse_geometry_major_radius, parse_geometry_minor_radius, parse_geometry_type,
    GeometryInfo, GeometryType,
};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Undefined, UserData};
use crate::common::kind_of::{
    KindOfMaskType, KindOfType, KINDOF_AIRCRAFT, KINDOF_BALLISTIC_MISSILE, KINDOF_BOAT, KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER, KINDOF_CLEARED_BY_BUILD,
    KINDOF_DEFENSIVE_WALL, KINDOF_DRAWABLE_ONLY, KINDOF_IGNORED_IN_GUI, KINDOF_INERT, KINDOF_INFANTRY, KINDOF_LANDMARK_BRIDGE, KINDOF_MOB_NEXUS,
    KINDOF_OPTIMIZED_TREE, KINDOF_PORTABLE_STRUCTURE, KINDOF_SCORE, KINDOF_SHRUBBERY, KINDOF_STRUCTURE, KINDOF_SUPPLY_SOURCE, KINDOF_VEHICLE,
};
use crate::common::module::{
    ModuleDataKind, ModuleType, MODULEINTERFACE_BODY, MODULEINTERFACE_CLIENT_UPDATE, MODULEINTERFACE_DRAW,
};
use crate::common::module_factory::{ModuleFactory, ModuleFactoryError};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::production_prerequisite::ProductionPrerequisite;
use crate::common::science::{parse_and_translate_label, scan_science, GameTextFetch, ScienceStore};
use crate::common::sparse_match_finder::{SparseMatchFinder, SparseMatchable};
use crate::common::thing_sort::EDITOR_SORTING_NAMES;
use crate::common::unicode_string::UnicodeString;
use crate::logic::armor::ArmorStore;
use crate::logic::armor_set::{ArmorSetFlags, ArmorSetNames};
use crate::logic::weapon_set::{WeaponTemplateSet, WeaponTemplateTable};
use crate::common::damage_fx::DamageFxStore;

// ---------------------------------------------------------------------------
// Constants and small enums
// ---------------------------------------------------------------------------

/// `MAX_UPGRADE_CAMEO_UPGRADES`.
pub const MAX_UPGRADE_CAMEO_UPGRADES: usize = 5;

/// `LEVEL_COUNT` as an array length.
const LEVELS: usize = LEVEL_COUNT as usize;

/// `USE_EXP_VALUE_FOR_SKILL_VALUE`: "-1 means 'same value as experienceValues
/// for that level'" (the constant is -999).
pub const USE_EXP_VALUE_FOR_SKILL_VALUE: i32 = -999;

/// `enum ThingTemplateAudioType`: the index of each sound in the template's
/// array. (`TTAUDIO_voiceSurrender` exists only under `ALLOW_SURRENDER`,
/// which no build defines.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(usize)]
pub enum ThingTemplateAudioType {
    VoiceSelect = 0,
    VoiceGroupSelect,
    VoiceSelectElite,
    VoiceMove,
    VoiceAttack,
    VoiceEnter,
    VoiceFear,
    VoiceCreated,
    VoiceNearEnemy,
    VoiceTaskUnable,
    VoiceTaskComplete,
    VoiceMeetEnemy,
    SoundMoveStart,
    SoundMoveStartDamaged,
    SoundMoveLoop,
    SoundMoveLoopDamaged,
    SoundAmbient,
    SoundAmbientDamaged,
    SoundAmbientReallyDamaged,
    SoundAmbientRubble,
    SoundStealthOn,
    SoundStealthOff,
    SoundCreated,
    SoundOnDamaged,
    SoundOnReallyDamaged,
    SoundEnter,
    SoundExit,
    SoundPromotedVeteran,
    SoundPromotedElite,
    SoundPromotedHero,
    VoiceGarrison,
    SoundFalling,
    VoiceDefect,
    VoiceAttackSpecial,
    VoiceAttackAir,
    VoiceGuard,
}

/// `TTAUDIO_COUNT`.
pub const TTAUDIO_COUNT: usize = 36;

/// `BC_INVALID`.
pub const BC_INVALID: u8 = 0;
/// `BC_APPEARS_AT_RALLY_POINT`: "unit appears at rally point of its #1 prereq".
pub const BC_APPEARS_AT_RALLY_POINT: u8 = 1;
/// `BC_PLACED_BY_PLAYER`: "unit must be manually placed by player".
pub const BC_PLACED_BY_PLAYER: u8 = 2;
/// `BuildCompletionNames[]`, without the `NULL`.
pub const BUILD_COMPLETION_NAMES: [&str; 3] = ["INVALID", "APPEARS_AT_RALLY_POINT", "PLACED_BY_PLAYER"];

/// `BSTATUS_YES`.
pub const BSTATUS_YES: u8 = 0;
/// `BSTATUS_IGNORE_PREREQUISITES`.
pub const BSTATUS_IGNORE_PREREQUISITES: u8 = 1;
/// `BSTATUS_NO`.
pub const BSTATUS_NO: u8 = 2;
/// `BSTATUS_ONLY_BY_AI`.
pub const BSTATUS_ONLY_BY_AI: u8 = 3;
/// `BuildableStatusNames[]`, without the `NULL`.
pub const BUILDABLE_STATUS_NAMES: [&str; 4] = ["Yes", "Ignore_Prerequisites", "No", "Only_By_AI"];

/// `RADAR_PRIORITY_INVALID`: "a priority that has not been set".
pub const RADAR_PRIORITY_INVALID: u8 = 0;
/// `RadarPriorityNames[]` (Radar.h), without the `NULL`.
pub const RADAR_PRIORITY_NAMES: [&str; 5] = ["INVALID", "NOT_ON_RADAR", "STRUCTURE", "UNIT", "LOCAL_UNIT_ONLY"];

/// `SHADOW_NONE`.
pub const SHADOW_NONE: u8 = 0;
/// `TheShadowNames[]` (Shadow.h), without the `NULL`: bit `i` of a shadow
/// type is name `i` (`SHADOW_DECAL` is 1, `SHADOW_VOLUME` is 2, ...).
pub const SHADOW_NAMES: [&str; 7] = [
    "SHADOW_DECAL",
    "SHADOW_VOLUME",
    "SHADOW_PROJECTION",
    "SHADOW_DYNAMIC_PROJECTION",
    "SHADOW_DIRECTIONAL_PROJECTION",
    "SHADOW_ALPHA_DECAL",
    "SHADOW_ADDITIVE_DECAL",
];

/// `enum ModuleParseMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleParseMode {
    /// `MODULEPARSE_NORMAL`.
    Normal,
    /// `MODULEPARSE_ADD_REMOVE_REPLACE`.
    AddRemoveReplace,
    /// `MODULEPARSE_INHERITABLE`.
    Inheritable,
    /// `MODULEPARSE_OVERRIDEABLE_BY_LIKE_KIND`.
    OverrideableByLikeKind,
}

// ---------------------------------------------------------------------------
// Module lists
// ---------------------------------------------------------------------------

/// One line of a client module's block (see the module docs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientLine {
    /// How many nested blocks the line is inside (0 for the module's own).
    pub depth: u32,
    /// The field token as written.
    pub field: Vec<u8>,
    /// The tokens after it, as written.
    pub args: Vec<Vec<u8>>,
}

/// The block of a `Draw` or `ClientUpdate` module, kept and never interpreted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientModuleData {
    /// `m_moduleTagNameKey`, set by `newModuleDataFromINI` after the block is read.
    pub module_tag_name_key: NameKeyType,
    /// The lines of the block (nested blocks' lines follow their opener).
    pub lines: Vec<ClientLine>,
}

/// What a `ModuleInfo` entry holds as `const ModuleData* second`.
#[derive(Clone, Debug)]
pub enum ModuleInfoData {
    /// The data of a behavior or body module, made by `ModuleFactory`.
    Logic(ModuleDataKind),
    /// The block of a draw or client update module.
    Client(ClientModuleData),
}

impl ModuleInfoData {
    /// `isAiModuleData()`: only `AIUpdateModuleData` says yes.
    pub fn is_ai_module_data(&self) -> bool {
        match self {
            ModuleInfoData::Logic(d) => d.is_ai_module_data(),
            ModuleInfoData::Client(_) => false,
        }
    }
}

/// `ModuleInfo::Nugget`.
#[derive(Clone, Debug)]
pub struct Nugget {
    /// `first`: the module's class name.
    pub first: Vec<u8>,
    /// `m_moduleTag`.
    pub module_tag: Vec<u8>,
    /// `second`.
    pub second: ModuleInfoData,
    /// `interfaceMask`.
    pub interface_mask: i32,
    /// `copiedFromDefault`.
    pub copied_from_default: bool,
    /// `inheritable`.
    pub inheritable: bool,
    /// `overrideableByLikeKind`.
    pub overrideable_by_like_kind: bool,
}

/// `class ModuleInfo`: the modules of one kind (behavior, draw or client
/// update) of a template, in order.
#[derive(Clone, Debug, Default)]
pub struct ModuleInfo {
    /// `m_info`.
    info: Vec<Nugget>,
}

impl ModuleInfo {
    /// `ModuleInfo()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `getNuggetWithTag(tag)`: the first entry with this tag.
    pub fn get_nugget_with_tag(&self, tag: &[u8]) -> Option<&Nugget> {
        self.info.iter().find(|n| n.module_tag == tag)
    }

    /// `addModuleInfo(...)`, without the debug-only check that the tag is
    /// unique among the template's modules.
    pub fn add_module_info(
        &mut self,
        name: &[u8],
        module_tag: &[u8],
        data: ModuleInfoData,
        interface_mask: i32,
        inheritable: bool,
        overrideable_by_like_kind: bool,
    ) {
        self.info.push(Nugget {
            first: name.to_vec(),
            module_tag: module_tag.to_vec(),
            second: data,
            interface_mask,
            copied_from_default: false,
            inheritable,
            overrideable_by_like_kind,
        });
    }

    /// `getCount()`.
    pub fn get_count(&self) -> i32 {
        self.info.len() as i32
    }

    /// `getNthName(i)`: empty for an `i` outside the list.
    pub fn get_nth_name(&self, i: i32) -> &[u8] {
        self.nth(i).map_or(&[], |n| &n.first)
    }

    /// `getNthTag(i)`: empty for an `i` outside the list.
    pub fn get_nth_tag(&self, i: i32) -> &[u8] {
        self.nth(i).map_or(&[], |n| &n.module_tag)
    }

    /// `getNthData(i)`: `None` for an `i` outside the list.
    pub fn get_nth_data(&self, i: i32) -> Option<&ModuleInfoData> {
        self.nth(i).map(|n| &n.second)
    }

    /// The entries, in order.
    pub fn nuggets(&self) -> &[Nugget] {
        &self.info
    }

    fn nth(&self, i: i32) -> Option<&Nugget> {
        if i >= 0 && (i as usize) < self.info.len() {
            Some(&self.info[i as usize])
        } else {
            None
        }
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.info.clear();
    }

    /// `setCopiedFromDefault(v)`.
    pub fn set_copied_from_default(&mut self, v: bool) {
        for n in &mut self.info {
            n.copied_from_default = v;
        }
    }

    /// `clearModuleDataWithTag(tagToClear, clearedModuleNameOut)`: remove
    /// every entry with this tag; `cleared_module_name_out` is set to the
    /// name of the last one removed, and left alone if none is.
    pub fn clear_module_data_with_tag(&mut self, tag_to_clear: &[u8], cleared_module_name_out: &mut Vec<u8>) -> bool {
        let mut cleared = false;
        let mut i = 0;
        while i < self.info.len() {
            if self.info[i].module_tag == tag_to_clear {
                *cleared_module_name_out = self.info[i].first.clone();
                self.info.remove(i);
                cleared = true;
            } else {
                i += 1;
            }
        }
        cleared
    }

    /// `clearCopiedFromDefaultEntries(interfaceMask, newName, fullTemplate)`:
    /// when a template adds a module, it drops the modules it got from
    /// `DefaultThingTemplate` that have the same interface, except the ones
    /// marked inheritable (and `ModuleTag_DefaultAutoHealBehavior` unless the
    /// template is trainable), and except the ones marked overrideable by
    /// like kind unless the new module is the same class, the template is
    /// one of the kinds immune to the GPS scrambler, or it is not one of the
    /// candidates. `kindof` and `is_trainable` are the full template's.
    pub fn clear_copied_from_default_entries(
        &mut self,
        interface_mask: i32,
        new_name: &[u8],
        kindof: &KindOfMaskType,
        is_trainable: bool,
    ) -> bool {
        let immune_to_gps_scrambler: [KindOfType; 16] = [
            KINDOF_AIRCRAFT,
            KINDOF_SHRUBBERY,
            KINDOF_OPTIMIZED_TREE,
            KINDOF_STRUCTURE,
            KINDOF_DRAWABLE_ONLY,
            KINDOF_MOB_NEXUS,
            KINDOF_IGNORED_IN_GUI,
            KINDOF_CLEARED_BY_BUILD,
            KINDOF_DEFENSIVE_WALL,
            KINDOF_BALLISTIC_MISSILE,
            KINDOF_SUPPLY_SOURCE,
            KINDOF_BOAT,
            KINDOF_INERT,
            KINDOF_BRIDGE,
            KINDOF_LANDMARK_BRIDGE,
            KINDOF_BRIDGE_TOWER,
        ];
        let candidate_for_gps_scrambler: [KindOfType; 4] = [KINDOF_SCORE, KINDOF_VEHICLE, KINDOF_INFANTRY, KINDOF_PORTABLE_STRUCTURE];
        let disallowed = immune_to_gps_scrambler.iter().any(|&k| kindof.test(k));
        let candidate = candidate_for_gps_scrambler.iter().any(|&k| kindof.test(k));

        let mut ret = false;
        let mut i = 0;
        while i < self.info.len() {
            let n = &self.info[i];
            if (n.interface_mask & interface_mask) != 0 && n.copied_from_default {
                if n.inheritable {
                    if n.module_tag == b"ModuleTag_DefaultAutoHealBehavior" && !is_trainable {
                        // Don't inherit this module if it is entirely useless to us.
                        self.info.remove(i);
                        ret = true;
                    } else {
                        i += 1; // skip to the next nugget, 'cause we inherit this one
                    }
                } else if n.overrideable_by_like_kind {
                    if n.first == new_name || disallowed || !candidate {
                        // the INI author asked for one of the same class, or the
                        // special overrideables are not added to these kinds
                        self.info.remove(i);
                        ret = true;
                    } else {
                        i += 1; // no match, preserve the default instance for now
                    }
                } else {
                    // just dump this instance, one of the same interface has been added
                    self.info.remove(i);
                    ret = true;
                }
            } else {
                i += 1;
            }
        }
        ret
    }

    /// `clearAiModuleInfo()`: remove the entries whose data is AI module data.
    pub fn clear_ai_module_info(&mut self) -> bool {
        let before = self.info.len();
        self.info.retain(|n| !n.second.is_ai_module_data());
        self.info.len() != before
    }
}

// ---------------------------------------------------------------------------
// ArmorTemplateSet
// ---------------------------------------------------------------------------

/// `class ArmorTemplateSet` (ArmorSet.h): one `ArmorSet` block of an
/// `Object`: the conditions it applies under, an armor and a damage effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmorTemplateSet {
    /// `m_types`.
    types: ArmorSetFlags,
    /// `m_template`: the armor template, by name key in the `ArmorStore`.
    template: Option<NameKeyType>,
    /// `m_fx`: the damage effect, by name key in the `DamageFxStore`.
    fx: Option<NameKeyType>,
}

impl Default for ArmorTemplateSet {
    fn default() -> Self {
        ArmorTemplateSet::new()
    }
}

impl ArmorTemplateSet {
    /// `ArmorTemplateSet()`: `clear()`.
    pub fn new() -> Self {
        ArmorTemplateSet { types: ArmorSetFlags::new(), template: None, fx: None }
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.types.clear();
        self.template = None;
        self.fx = None;
    }

    /// `getArmorTemplate()`.
    pub fn get_armor_template(&self) -> Option<NameKeyType> {
        self.template
    }

    /// `getDamageFX()`.
    pub fn get_damage_fx(&self) -> Option<NameKeyType> {
        self.fx
    }

    /// `getNthConditionsYes(0)`.
    pub fn conditions(&self) -> &ArmorSetFlags {
        &self.types
    }

    // --- p1w13-sonnet ---
    /// An armor template set with these parts (what `parseArmorTemplateSet` leaves for the same values), for a program that builds a template without INI
    /// text, as the differential test of `ActiveBody` does.
    pub fn from_parts(types: ArmorSetFlags, template: Option<NameKeyType>, fx: Option<NameKeyType>) -> Self {
        ArmorTemplateSet { types, template, fx }
    }
    // --- end p1w13-sonnet ---

    /// `parseArmorTemplateSet(ini)` (ThingTemplate.cpp): `Conditions`, `Armor`
    /// and `DamageFX` until `End`.
    pub fn parse_armor_template_set(
        &mut self,
        ini: &mut Ini<'_>,
        names: &mut NameKeyGenerator,
        armors: &ArmorStore,
        damage_fx: &DamageFxStore,
    ) -> IniResult<()> {
        let mut fill = ArmorFill { set: self, names, armors, damage_fx };
        ini.init_from_ini(&mut fill, &armor_set_fields())
    }
}

impl SparseMatchable<ArmorSetNames> for ArmorTemplateSet {
    /// `getConditionsYesCount()`: one.
    fn conditions_yes_count(&self) -> i32 {
        1
    }

    /// `getNthConditionsYes(i)`: the conditions.
    fn nth_conditions_yes(&self, _i: i32) -> &ArmorSetFlags {
        &self.types
    }
}

struct ArmorFill<'a> {
    set: &'a mut ArmorTemplateSet,
    names: &'a mut NameKeyGenerator,
    armors: &'a ArmorStore,
    damage_fx: &'a DamageFxStore,
}

fn name_keys_exhausted<T>(_: T) -> IniError {
    IniError::undefined(Undefined::NameKeysExhausted)
}

fn f_armor_conditions(ini: &mut Ini<'_>, w: &mut ArmorFill<'_>, _: &UserData) -> IniResult<()> {
    w.set.types.parse_from_ini(ini)
}

/// `INI::parseArmorTemplate`: `None` (any case) is none, else
/// `TheArmorStore->findArmorTemplate(token)` (which assigns the name a key).
fn f_armor_template(ini: &mut Ini<'_>, w: &mut ArmorFill<'_>, _: &UserData) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    w.set.template = if token.eq_ignore_ascii_case(b"None") {
        None
    } else {
        w.armors.find_armor_template_key(w.names, &token).map_err(name_keys_exhausted)?
    };
    Ok(())
}

/// `INI::parseDamageFX`: `None` (any case) is none, else
/// `TheDamageFXStore->findDamageFX(token)`.
fn f_armor_damage_fx(ini: &mut Ini<'_>, w: &mut ArmorFill<'_>, _: &UserData) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    w.set.fx = if token.eq_ignore_ascii_case(b"None") {
        None
    } else {
        let key = w.names.try_name_to_key(&token).map_err(name_keys_exhausted)?;
        w.damage_fx.get(key).map(|_| key)
    };
    Ok(())
}

fn armor_set_fields<'a>() -> [FieldParse<ArmorFill<'a>>; 4] {
    [
        FieldParse::new("Conditions", f_armor_conditions, UserData::None),
        FieldParse::new("Armor", f_armor_template, UserData::None),
        FieldParse::new("DamageFX", f_armor_damage_fx, UserData::None),
        FieldParse::end(),
    ]
}

// ---------------------------------------------------------------------------
// The template
// ---------------------------------------------------------------------------

/// `class AudioArray`: the sounds of a template, `None` for "not set".
pub type AudioArray = [Option<AudioEventName>; TTAUDIO_COUNT];

/// `s_audioEventNoSound`: what a sound that is not set reads as.
static NO_SOUND: AudioEventName = AudioEventName { event_name: Vec::new() };

/// `class ThingTemplate`.
#[derive(Clone, Debug)]
pub struct ThingTemplate {
    // ---- Strings
    /// `m_displayName`: "UI display for onscreen display".
    display_name: UnicodeString,
    /// `m_nameString`: "name of this thing template".
    name_string: Vec<u8>,
    /// `m_defaultOwningSide`.
    default_owning_side: Vec<u8>,
    /// `m_commandSetString`.
    command_set_string: Vec<u8>,
    /// `m_selectedPortraitImageName`.
    selected_portrait_image_name: Vec<u8>,
    /// `m_buttonImageName`.
    button_image_name: Vec<u8>,
    /// `m_upgradeCameoUpgradeNames`.
    upgrade_cameo_upgrade_names: [Vec<u8>; MAX_UPGRADE_CAMEO_UPGRADES],
    /// `m_shadowTextureName`.
    shadow_texture_name: Vec<u8>,
    /// `m_moduleBeingReplacedName`: used only inside a `ReplaceModule` block.
    module_being_replaced_name: Vec<u8>,
    /// `m_moduleBeingReplacedTag`.
    module_being_replaced_tag: Vec<u8>,

    // ---- Misc larger-than-int things
    /// `m_geometryInfo`.
    geometry_info: GeometryInfo,
    /// `m_kindof`.
    kindof: KindOfMaskType,
    /// `m_audioarray`.
    audio_array: AudioArray,
    /// `m_behaviorModuleInfo`.
    behavior_module_info: ModuleInfo,
    /// `m_drawModuleInfo`.
    draw_module_info: ModuleInfo,
    /// `m_clientUpdateModuleInfo`.
    client_update_module_info: ModuleInfo,

    // ---- Misc arrays-of-things
    /// `m_skillPointValues`.
    skill_point_values: [i32; LEVELS],
    /// `m_experienceValues`.
    experience_values: [i32; LEVELS],
    /// `m_experienceRequired`.
    experience_required: [i32; LEVELS],

    // ---- STL-sized things
    /// `m_prereqInfo`.
    prereq_info: Vec<ProductionPrerequisite>,
    /// `m_buildVariations`.
    build_variations: Vec<Vec<u8>>,
    /// `m_weaponTemplateSets`.
    weapon_template_sets: Vec<WeaponTemplateSet>,
    /// `m_weaponTemplateSetFinder`.
    weapon_template_set_finder: SparseMatchFinder,
    /// `m_armorTemplateSets`.
    armor_template_sets: Vec<ArmorTemplateSet>,
    /// `m_armorTemplateSetFinder`.
    armor_template_set_finder: SparseMatchFinder,
    /// `m_perUnitSounds`.
    per_unit_sounds: BTreeMap<Vec<u8>, AudioEventName>,
    /// `m_perUnitFX`: the `FXList`'s name, `None` for the null pointer.
    per_unit_fx: BTreeMap<Vec<u8>, Option<Vec<u8>>>,

    // ---- Pointer-sized things
    /// This object's identity, which pointer comparisons use; see the module docs.
    node_id: u64,
    /// `m_reskinnedFrom`: the identity of the template this one was reskinned from.
    reskinned_from: Option<u64>,

    // ---- Real-sized things
    fence_width: f32,
    fence_x_offset: f32,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_reveal_to_all_range: f32,
    placement_view_angle: f32,
    factory_exit_width: f32,
    factory_extra_bib_width: f32,
    build_time: f32,
    asset_scale: f32,
    instance_scale_fuzziness: f32,
    shadow_size_x: f32,
    shadow_size_y: f32,
    shadow_offset_x: f32,
    shadow_offset_y: f32,

    // ---- Int-sized things
    energy_production: i32,
    energy_bonus: i32,
    display_color: i32,
    occlusion_delay: u32,
    max_simultaneous_link_key: NameKeyType,

    // ---- Short-sized things
    template_id: u16,
    build_cost: u16,
    refund_value: u16,
    threat_value: u16,
    max_simultaneous_of_type: u16,

    // ---- Bool-sized things
    max_simultaneous_determined_by_superweapon_restriction: bool,
    is_prerequisite: bool,
    is_bridge: bool,
    is_build_facility: bool,
    is_trainable: bool,
    enter_guard: bool,
    hijack_guard: bool,
    is_forbidden: bool,
    armor_copied_from_default: bool,
    weapons_copied_from_default: bool,

    // ---- Byte-sized things
    radar_priority: u8,
    transport_slot_count: i8,
    buildable: u8,
    build_completion: u8,
    editor_sorting: u8,
    structure_rubble_height: i8,
    shadow_type: u8,
    module_parsing_mode: ModuleParseMode,
    crusher_level: u8,
    crushable_level: u8,
}

impl ThingTemplate {
    /// `ThingTemplate()`: the constructor's values; `default_occlusion_delay`
    /// is `TheGlobalData->m_defaultOcclusionDelay`. See the module docs for
    /// the members the constructor leaves to the zero-filled pool.
    pub fn new(default_occlusion_delay: u32) -> Self {
        ThingTemplate {
            display_name: UnicodeString::new(),
            name_string: Vec::new(),
            default_owning_side: Vec::new(),
            command_set_string: Vec::new(),
            selected_portrait_image_name: Vec::new(),
            button_image_name: Vec::new(),
            upgrade_cameo_upgrade_names: Default::default(),
            shadow_texture_name: Vec::new(),
            module_being_replaced_name: Vec::new(),
            module_being_replaced_tag: Vec::new(),
            geometry_info: GeometryInfo::new(GeometryType::Sphere, false, 1.0, 1.0, 1.0),
            kindof: KindOfMaskType::new(),
            audio_array: std::array::from_fn(|_| None),
            behavior_module_info: ModuleInfo::new(),
            draw_module_info: ModuleInfo::new(),
            client_update_module_info: ModuleInfo::new(),
            // -1 means "same value as experienceValues for that level"
            skill_point_values: [USE_EXP_VALUE_FOR_SKILL_VALUE; LEVELS],
            experience_values: [0; LEVELS],
            experience_required: [0; LEVELS],
            prereq_info: Vec::new(),
            build_variations: Vec::new(),
            weapon_template_sets: Vec::new(),
            weapon_template_set_finder: SparseMatchFinder::new(),
            armor_template_sets: Vec::new(),
            armor_template_set_finder: SparseMatchFinder::new(),
            per_unit_sounds: BTreeMap::new(),
            per_unit_fx: BTreeMap::new(),
            node_id: 0,
            reskinned_from: None,
            fence_width: 0.0,
            fence_x_offset: 0.0,
            vision_range: 0.0,
            shroud_clearing_range: -1.0,
            shroud_reveal_to_all_range: -1.0,
            placement_view_angle: 0.0,
            factory_exit_width: 0.0,
            factory_extra_bib_width: 0.0,
            build_time: 1.0,
            asset_scale: 0.0,
            instance_scale_fuzziness: 0.0,
            shadow_size_x: 0.0,
            shadow_size_y: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            energy_production: 0,
            energy_bonus: 0,
            display_color: 0,
            occlusion_delay: default_occlusion_delay,
            max_simultaneous_link_key: NameKeyType::INVALID, // not linked
            template_id: 0,
            build_cost: 0,
            refund_value: 0,
            threat_value: 0,
            max_simultaneous_of_type: 0, // unlimited
            max_simultaneous_determined_by_superweapon_restriction: false,
            is_prerequisite: false,
            is_bridge: false,
            is_build_facility: false,
            is_trainable: false,
            enter_guard: false,
            hijack_guard: false,
            is_forbidden: false,
            armor_copied_from_default: false,
            weapons_copied_from_default: false,
            radar_priority: RADAR_PRIORITY_INVALID,
            transport_slot_count: 0,
            buildable: 0,
            build_completion: BC_APPEARS_AT_RALLY_POINT,
            editor_sorting: 0,
            structure_rubble_height: 0,
            shadow_type: SHADOW_NONE,
            module_parsing_mode: ModuleParseMode::Normal,
            crusher_level: 0,   // unspecified, this object is unable to crush anything
            crushable_level: 255, // unspecified, this object is unable to be crushed by anything
        }
    }

    /// `copyFrom(that)`: "copy the guts of that into this, but preserve this'
    /// name, id, and list-links" (and, here, this object's identity). Does
    /// nothing for `None`.
    pub fn copy_from(&mut self, that: Option<&ThingTemplate>) {
        let Some(that) = that else {
            return;
        };
        let id = self.template_id;
        let name = std::mem::take(&mut self.name_string);
        let node_id = self.node_id;
        *self = that.clone();
        self.template_id = id;
        self.name_string = name;
        self.node_id = node_id;
    }

    /// `setCopiedFromDefault()`: the armor and weapon sets, and every module,
    /// came from `DefaultThingTemplate`, so the first thing the new template
    /// says about them replaces them.
    pub fn set_copied_from_default(&mut self) {
        self.armor_copied_from_default = true;
        self.weapons_copied_from_default = true;
        self.behavior_module_info.set_copied_from_default(true);
        self.draw_module_info.set_copied_from_default(true);
        self.client_update_module_info.set_copied_from_default(true);
    }

    /// `validate()` as the shipped build runs it: a template with no shadow
    /// texture gets `shadow` (sphere and cylinder) or `shadows` (box). (The
    /// debug checks and `validateAudio()` are compiled out.)
    pub fn validate(&mut self) {
        if self.shadow_texture_name.is_empty() {
            // no texture given, pick a default
            match self.geometry_info.get_geom_type() {
                GeometryType::Sphere | GeometryType::Cylinder => self.shadow_texture_name = b"shadow".to_vec(),
                GeometryType::Box => self.shadow_texture_name = b"shadows".to_vec(),
            }
        }
    }

    // ---- identity (see the module docs)

    /// This object's identity (the pointer upstream compares).
    pub fn node_id(&self) -> u64 {
        self.node_id
    }

    /// Set by the factory when it makes the object.
    pub fn friend_set_node_id(&mut self, id: u64) {
        self.node_id = id;
    }

    /// `getTemplateID()`: "a unique identifier suitable for identifying this
    /// ThingTemplate on machines playing across the net".
    pub fn get_template_id(&self) -> u16 {
        self.template_id
    }

    /// `friend_setTemplateID(id)`.
    pub fn friend_set_template_id(&mut self, id: u16) {
        self.template_id = id;
    }

    /// `getName()`.
    pub fn get_name(&self) -> &[u8] {
        &self.name_string
    }

    /// `friend_setTemplateName(name)`.
    pub fn friend_set_template_name(&mut self, name: &[u8]) {
        self.name_string = name.to_vec();
    }

    /// `setReskinnedFrom(tt)`: the identity of the template reskinned from.
    pub fn set_reskinned_from(&mut self, tt: u64) {
        self.reskinned_from = Some(tt);
    }

    /// `m_reskinnedFrom`.
    pub fn reskinned_from(&self) -> Option<u64> {
        self.reskinned_from
    }

    // ---- getters

    /// `getDisplayColor()`.
    pub fn get_display_color(&self) -> i32 {
        self.display_color
    }

    /// `getEditorSorting()`.
    pub fn get_editor_sorting(&self) -> i32 {
        i32::from(self.editor_sorting)
    }

    /// `isKindOf(t)`.
    pub fn is_kind_of(&self, t: KindOfType) -> bool {
        self.kindof.test(t)
    }

    /// `isKindOfMulti(mustBeSet, mustBeClear)`.
    pub fn is_kind_of_multi(&self, must_be_set: &KindOfMaskType, must_be_clear: &KindOfMaskType) -> bool {
        self.kindof.test_set_and_clear(must_be_set, must_be_clear)
    }

    /// `isAnyKindOf(anyKindOf)`.
    pub fn is_any_kind_of(&self, any_kind_of: &KindOfMaskType) -> bool {
        self.kindof.any_intersection_with(any_kind_of)
    }

    /// `m_kindof`.
    pub fn kindof(&self) -> &KindOfMaskType {
        &self.kindof
    }

    /// `getDisplayName()`.
    pub fn get_display_name(&self) -> &UnicodeString {
        &self.display_name
    }

    /// `getDefaultRadarPriority()`.
    pub fn get_default_radar_priority(&self) -> u8 {
        self.radar_priority
    }

    /// `getRawTransportSlotCount()`.
    pub fn get_raw_transport_slot_count(&self) -> i32 {
        i32::from(self.transport_slot_count)
    }

    /// `getFenceWidth()`.
    pub fn get_fence_width(&self) -> f32 {
        self.fence_width
    }

    /// `getFenceXOffset()`.
    pub fn get_fence_x_offset(&self) -> f32 {
        self.fence_x_offset
    }

    /// `isBridge()`.
    pub fn is_bridge(&self) -> bool {
        self.is_bridge
    }

    /// `friend_calcVisionRange()`.
    pub fn friend_calc_vision_range(&self) -> f32 {
        self.vision_range
    }

    /// `friend_calcShroudClearingRange()`.
    pub fn friend_calc_shroud_clearing_range(&self) -> f32 {
        self.shroud_clearing_range
    }

    /// `getShroudRevealToAllRange()`.
    pub fn get_shroud_reveal_to_all_range(&self) -> f32 {
        self.shroud_reveal_to_all_range
    }

    /// `getShadowType()`.
    pub fn get_shadow_type(&self) -> u8 {
        self.shadow_type
    }

    /// `getShadowSizeX()`.
    pub fn get_shadow_size_x(&self) -> f32 {
        self.shadow_size_x
    }

    /// `getShadowSizeY()`.
    pub fn get_shadow_size_y(&self) -> f32 {
        self.shadow_size_y
    }

    /// `getShadowOffsetX()`.
    pub fn get_shadow_offset_x(&self) -> f32 {
        self.shadow_offset_x
    }

    /// `getShadowOffsetY()`.
    pub fn get_shadow_offset_y(&self) -> f32 {
        self.shadow_offset_y
    }

    /// `getShadowTextureName()`.
    pub fn get_shadow_texture_name(&self) -> &[u8] {
        &self.shadow_texture_name
    }

    /// `getOcclusionDelay()`.
    pub fn get_occlusion_delay(&self) -> u32 {
        self.occlusion_delay
    }

    /// `getBehaviorModuleInfo()`.
    pub fn get_behavior_module_info(&self) -> &ModuleInfo {
        &self.behavior_module_info
    }

    /// `getDrawModuleInfo()`.
    pub fn get_draw_module_info(&self) -> &ModuleInfo {
        &self.draw_module_info
    }

    /// `getClientUpdateModuleInfo()`.
    pub fn get_client_update_module_info(&self) -> &ModuleInfo {
        &self.client_update_module_info
    }

    /// The name `getSelectedPortraitImage()` looks up (a client cut point).
    pub fn selected_portrait_image_name(&self) -> &[u8] {
        &self.selected_portrait_image_name
    }

    /// The name `getButtonImage()` looks up (a client cut point).
    pub fn button_image_name(&self) -> &[u8] {
        &self.button_image_name
    }

    /// `getSkillPointValue(level)`: the skill points, or the experience value
    /// of that level where the template says "same".
    pub fn get_skill_point_value(&self, level: VeterancyLevel) -> i32 {
        let value = self.skill_point_values[level as usize];
        if value == USE_EXP_VALUE_FOR_SKILL_VALUE {
            return self.get_experience_value(level);
        }
        value
    }

    /// `m_skillPointValues[level]`, before the "same" substitution.
    pub fn raw_skill_point_value(&self, level: VeterancyLevel) -> i32 {
        self.skill_point_values[level as usize]
    }

    /// `getExperienceValue(level)`.
    pub fn get_experience_value(&self, level: VeterancyLevel) -> i32 {
        self.experience_values[level as usize]
    }

    /// `getExperienceRequired(level)`.
    pub fn get_experience_required(&self, level: VeterancyLevel) -> i32 {
        self.experience_required[level as usize]
    }

    /// `isTrainable()`.
    pub fn is_trainable(&self) -> bool {
        self.is_trainable
    }

    /// `isEnterGuard()`.
    pub fn is_enter_guard(&self) -> bool {
        self.enter_guard
    }

    /// `isHijackGuard()`.
    pub fn is_hijack_guard(&self) -> bool {
        self.hijack_guard
    }

    /// `getAudio(t)`: the sound, or `s_audioEventNoSound` (an event with no
    /// name) where it is not set. (`getVoiceSelect()` and the rest are this
    /// with a constant.)
    pub fn get_audio(&self, t: ThingTemplateAudioType) -> &AudioEventName {
        self.audio_array[t as usize].as_ref().unwrap_or(&NO_SOUND)
    }

    /// `hasAudio(t)`; `hasSoundAmbient()` is this with `SoundAmbient`.
    pub fn has_audio(&self, t: ThingTemplateAudioType) -> bool {
        self.audio_array[t as usize].is_some()
    }

    /// `getPerUnitSound(soundName)`: the entry's event, or the no-sound
    /// event for an empty or unknown name.
    pub fn get_per_unit_sound(&self, sound_name: &[u8]) -> &AudioEventName {
        if sound_name.is_empty() {
            return &NO_SOUND;
        }
        self.per_unit_sounds.get(sound_name).unwrap_or(&NO_SOUND)
    }

    /// `getAllPerUnitSounds()`.
    pub fn get_all_per_unit_sounds(&self) -> &BTreeMap<Vec<u8>, AudioEventName> {
        &self.per_unit_sounds
    }

    /// `getPerUnitFX(fxName)`: the `FXList`'s name; `None` for an empty or
    /// unknown name, and for an entry that is `None`.
    pub fn get_per_unit_fx(&self, fx_name: &[u8]) -> Option<&[u8]> {
        if fx_name.is_empty() {
            return None;
        }
        self.per_unit_fx.get(fx_name).and_then(|f| f.as_deref())
    }

    /// `m_perUnitFX`, entries with a null `FXList` included.
    pub fn per_unit_fx(&self) -> &BTreeMap<Vec<u8>, Option<Vec<u8>>> {
        &self.per_unit_fx
    }

    /// `getThreatValue()`.
    pub fn get_threat_value(&self) -> u32 {
        u32::from(self.threat_value)
    }

    /// `getMaxSimultaneousLinkKey()`.
    pub fn get_max_simultaneous_link_key(&self) -> NameKeyType {
        self.max_simultaneous_link_key
    }

    /// `getMaxSimultaneousOfType()`: the template's value, or, when it says
    /// `DeterminedBySuperweaponRestriction` and there is a game logic
    /// (`superweapon_restriction`, `TheGameLogic->getSuperweaponRestriction()`),
    /// that.
    pub fn get_max_simultaneous_of_type(&self, superweapon_restriction: Option<u32>) -> u32 {
        if self.max_simultaneous_determined_by_superweapon_restriction {
            if let Some(r) = superweapon_restriction {
                return r;
            }
        }
        u32::from(self.max_simultaneous_of_type)
    }

    /// Whether `MaxSimultaneousOfType` was `DeterminedBySuperweaponRestriction`.
    pub fn max_simultaneous_determined_by_superweapon_restriction(&self) -> bool {
        self.max_simultaneous_determined_by_superweapon_restriction
    }

    /// `friend_getCommandSetString()`.
    pub fn friend_get_command_set_string(&self) -> &[u8] {
        &self.command_set_string
    }

    /// `getBuildVariations()`.
    pub fn get_build_variations(&self) -> &[Vec<u8>] {
        &self.build_variations
    }

    /// `getAssetScale()`.
    pub fn get_asset_scale(&self) -> f32 {
        self.asset_scale
    }

    /// `getInstanceScaleFuzziness()`.
    pub fn get_instance_scale_fuzziness(&self) -> f32 {
        self.instance_scale_fuzziness
    }

    /// `getStructureRubbleHeight()`.
    pub fn get_structure_rubble_height(&self) -> f32 {
        f32::from(self.structure_rubble_height)
    }

    /// `getTemplateGeometryInfo()`.
    pub fn get_template_geometry_info(&self) -> &GeometryInfo {
        &self.geometry_info
    }

    /// `getEnergyProduction()`.
    pub fn get_energy_production(&self) -> i32 {
        self.energy_production
    }

    /// `getEnergyBonus()`.
    pub fn get_energy_bonus(&self) -> i32 {
        self.energy_bonus
    }

    /// `getRefundValue()`.
    pub fn get_refund_value(&self) -> i32 {
        i32::from(self.refund_value)
    }

    /// `getBuildCompletion()`.
    pub fn get_build_completion(&self) -> u8 {
        self.build_completion
    }

    /// The template's `m_buildable`; `getBuildable()` first asks the game
    /// logic for a script override (a cut point, see the module docs).
    pub fn buildable(&self) -> u8 {
        self.buildable
    }

    /// `getPrereqCount()`.
    pub fn get_prereq_count(&self) -> i32 {
        self.prereq_info.len() as i32
    }

    /// `getNthPrereq(i)`.
    pub fn get_nth_prereq(&self, i: usize) -> Option<&ProductionPrerequisite> {
        self.prereq_info.get(i)
    }

    /// `m_prereqInfo`.
    pub fn prereq_info(&self) -> &[ProductionPrerequisite] {
        &self.prereq_info
    }

    /// `m_prereqInfo`, for `resolveNames`.
    pub fn prereq_info_mut(&mut self) -> &mut Vec<ProductionPrerequisite> {
        &mut self.prereq_info
    }

    /// `isBuildableItem()`.
    pub fn is_buildable_item(&self) -> bool {
        self.build_cost != 0
    }

    /// `friend_getBuildCost()`.
    pub fn friend_get_build_cost(&self) -> i32 {
        i32::from(self.build_cost)
    }

    /// `getBuildTime()`: seconds.
    pub fn get_build_time(&self) -> f32 {
        self.build_time
    }

    /// `getDefaultOwningSide()`.
    pub fn get_default_owning_side(&self) -> &[u8] {
        &self.default_owning_side
    }

    /// `isBuildFacility()`.
    pub fn is_build_facility(&self) -> bool {
        self.is_build_facility
    }

    /// The flag `resolveNames` sets on the templates that other templates'
    /// prerequisites name (and on command centers).
    pub fn set_is_build_facility(&mut self, v: bool) {
        self.is_build_facility = v;
    }

    /// `getPlacementViewAngle()`.
    pub fn get_placement_view_angle(&self) -> f32 {
        self.placement_view_angle
    }

    /// `getFactoryExitWidth()`.
    pub fn get_factory_exit_width(&self) -> f32 {
        self.factory_exit_width
    }

    /// `getFactoryExtraBibWidth()`.
    pub fn get_factory_extra_bib_width(&self) -> f32 {
        self.factory_extra_bib_width
    }

    /// `isPrerequisite()`.
    pub fn is_prerequisite(&self) -> bool {
        self.is_prerequisite
    }

    /// `m_isForbidden`: "useful when overriding in <mapfile>.ini".
    pub fn is_forbidden(&self) -> bool {
        self.is_forbidden
    }

    /// `getCrushableLevel()`.
    pub fn get_crushable_level(&self) -> u8 {
        self.crushable_level
    }

    /// `getCrusherLevel()`.
    pub fn get_crusher_level(&self) -> u8 {
        self.crusher_level
    }

    /// `getUpgradeCameoName(n)`: empty for an `n` outside the five.
    pub fn get_upgrade_cameo_name(&self, n: usize) -> &[u8] {
        self.upgrade_cameo_upgrade_names.get(n).map_or(&[], |s| s)
    }

    /// `getWeaponTemplateSets()`.
    pub fn get_weapon_template_sets(&self) -> &[WeaponTemplateSet] {
        &self.weapon_template_sets
    }

    /// The armor template sets.
    pub fn armor_template_sets(&self) -> &[ArmorTemplateSet] {
        &self.armor_template_sets
    }

    /// `findArmorTemplateSet(t)`: the set that best matches the flags (the
    /// finder caches, so this takes `&mut self`).
    pub fn find_armor_template_set(&mut self, t: &ArmorSetFlags) -> Option<&ArmorTemplateSet> {
        let i = self.armor_template_set_finder.find_best_info(&self.armor_template_sets, t)?;
        self.armor_template_sets.get(i)
    }

    /// `findWeaponTemplateSet(t)`: the set that best matches the flags.
    pub fn find_weapon_template_set(&mut self, t: &crate::logic::weapon_set_flags::WeaponSetFlags) -> Option<&WeaponTemplateSet> {
        let i = self.weapon_template_set_finder.find_best_info(&self.weapon_template_sets, t)?;
        self.weapon_template_sets.get(i)
    }

    /// `canPossiblyHaveAnyWeapon()`: "true iff we have at least one weaponset
    /// that contains a weapon".
    pub fn can_possibly_have_any_weapon(&self) -> bool {
        self.weapon_template_sets.iter().any(|s| s.has_any_weapons())
    }

    /// `m_moduleParsingMode`.
    pub fn module_parsing_mode(&self) -> ModuleParseMode {
        self.module_parsing_mode
    }

    /// `m_moduleBeingReplacedName`.
    pub fn module_being_replaced_name(&self) -> &[u8] {
        &self.module_being_replaced_name
    }

    /// `m_moduleBeingReplacedTag`.
    pub fn module_being_replaced_tag(&self) -> &[u8] {
        &self.module_being_replaced_tag
    }

    /// `m_armorCopiedFromDefault`.
    pub fn armor_copied_from_default(&self) -> bool {
        self.armor_copied_from_default
    }

    /// `m_weaponsCopiedFromDefault`.
    pub fn weapons_copied_from_default(&self) -> bool {
        self.weapons_copied_from_default
    }

    /// `friend_getAIModuleInfo()`: whether the template has a module whose
    /// data is AI module data (the data itself is `AIUpdateModuleData`,
    /// layer 9).
    pub fn has_ai_module(&self) -> bool {
        self.behavior_module_info.nuggets().iter().any(|n| n.second.is_ai_module_data())
    }

    /// `removeModuleInfo(moduleToRemove, clearedModuleNameOut)`: remove the
    /// module with this tag from whichever list has it (the behavior list,
    /// the draw list and the client update list, each tried).
    fn remove_module_info(&mut self, module_to_remove: &[u8], cleared_module_name_out: &mut Vec<u8>) -> bool {
        let mut removed = false;
        if self.behavior_module_info.clear_module_data_with_tag(module_to_remove, cleared_module_name_out) {
            removed = true;
        }
        if self.draw_module_info.clear_module_data_with_tag(module_to_remove, cleared_module_name_out) {
            removed = true;
        }
        if self.client_update_module_info.clear_module_data_with_tag(module_to_remove, cleared_module_name_out) {
            removed = true;
        }
        removed
    }
}

// --- p1w6-object ---
/// The values of a template that `Object` (layer 8) reads, for building one
/// without INI (the tests of `logic::world` and the differential test of
/// `Object` use it; production code reads templates from INI). Every field
/// has the constructor's default in [`ObjectTemplateValues::default`].
#[derive(Clone, Debug)]
pub struct ObjectTemplateValues {
    /// `m_nameString`.
    pub name: Vec<u8>,
    /// `m_templateID`.
    pub template_id: u16,
    /// The `KindOf` bits set.
    pub kind_of: Vec<crate::common::kind_of::KindOfType>,
    /// `m_visionRange`.
    pub vision_range: f32,
    /// `m_shroudClearingRange` (-1 = use the vision range).
    pub shroud_clearing_range: f32,
    /// `m_shroudRevealToAllRange`.
    pub shroud_reveal_to_all_range: f32,
    /// `m_geometryInfo` (`None`: the constructor's).
    pub geometry: Option<GeometryInfo>,
    /// `m_energyProduction`.
    pub energy_production: i32,
    /// `m_energyBonus`.
    pub energy_bonus: i32,
    /// `m_buildCost`.
    pub build_cost: u16,
    /// `m_threatValue`.
    pub threat_value: u16,
    /// `m_crusherLevel`.
    pub crusher_level: u8,
    /// `m_crushableLevel`.
    pub crushable_level: u8,
    /// `m_transportSlotCount`.
    pub transport_slot_count: i8,
    /// `m_experienceValues`.
    pub experience_values: [i32; 4],
    /// `m_experienceRequired`.
    pub experience_required: [i32; 4],
    /// `m_isTrainable`.
    pub is_trainable: bool,
    /// `m_enterGuard`.
    pub enter_guard: bool,
    /// `m_occlusionDelay`.
    pub occlusion_delay: u32,
    /// `m_radarPriority`.
    pub radar_priority: u8,
    /// `m_commandSetString`.
    pub command_set_string: Vec<u8>,
    /// The behaviour modules' (class name, tag), in order.
    pub behavior_modules: Vec<(Vec<u8>, Vec<u8>)>,
    /// The weapon template sets: (the `WEAPONSET_*` conditions, has a weapon in the primary slot).
    pub weapon_sets: Vec<(Vec<i32>, bool)>,
    // --- p1w13-sonnet ---
    /// The armor template sets (`ArmorSet` blocks), in order.
    pub armor_sets: Vec<ArmorTemplateSet>,
    /// The sounds of the template (`VoiceFear`, `SoundOnDamaged`, ...): the audio type and the event's name.
    pub audio: Vec<(ThingTemplateAudioType, Vec<u8>)>,
    /// `m_structureRubbleHeight` (an `Int8`).
    pub structure_rubble_height: i8,
    // --- end p1w13-sonnet ---
}

impl Default for ObjectTemplateValues {
    fn default() -> Self {
        ObjectTemplateValues {
            name: Vec::new(),
            template_id: 0,
            kind_of: Vec::new(),
            vision_range: 0.0,
            shroud_clearing_range: -1.0,
            shroud_reveal_to_all_range: 0.0,
            geometry: None,
            energy_production: 0,
            energy_bonus: 0,
            build_cost: 0,
            threat_value: 0,
            crusher_level: 0,
            crushable_level: 0,
            transport_slot_count: 0,
            experience_values: [0; 4],
            experience_required: [0; 4],
            is_trainable: false,
            enter_guard: false,
            occlusion_delay: 0,
            radar_priority: RADAR_PRIORITY_INVALID,
            command_set_string: Vec::new(),
            behavior_modules: Vec::new(),
            weapon_sets: Vec::new(),
            // --- p1w13-sonnet ---
            armor_sets: Vec::new(),
            audio: Vec::new(),
            structure_rubble_height: 0,
            // --- end p1w13-sonnet ---
        }
    }
}

impl ThingTemplate {
    /// A template with these values and the constructor's defaults for the rest.
    pub fn from_object_values(v: &ObjectTemplateValues) -> ThingTemplate {
        let mut t = ThingTemplate::new(v.occlusion_delay);
        t.name_string = v.name.clone();
        t.template_id = v.template_id;
        t.kindof = KindOfMaskType::from_indices(&v.kind_of);
        t.vision_range = v.vision_range;
        t.shroud_clearing_range = v.shroud_clearing_range;
        t.shroud_reveal_to_all_range = v.shroud_reveal_to_all_range;
        if let Some(g) = &v.geometry {
            t.geometry_info = g.clone();
        }
        t.energy_production = v.energy_production;
        t.energy_bonus = v.energy_bonus;
        t.build_cost = v.build_cost;
        t.threat_value = v.threat_value;
        t.crusher_level = v.crusher_level;
        t.crushable_level = v.crushable_level;
        t.transport_slot_count = v.transport_slot_count;
        t.experience_values = v.experience_values;
        t.experience_required = v.experience_required;
        t.is_trainable = v.is_trainable;
        t.enter_guard = v.enter_guard;
        t.radar_priority = v.radar_priority;
        t.command_set_string = v.command_set_string.clone();
        for (name, tag) in &v.behavior_modules {
            t.behavior_module_info.add_module_info(name, tag, ModuleInfoData::Client(ClientModuleData::default()), 0, false, false);
        }
        t.weapon_template_sets = v
            .weapon_sets
            .iter()
            .map(|(flags, has)| WeaponTemplateSet::from_values(Some(v.template_id), crate::logic::weapon_set_flags::WeaponSetFlags::from_indices(flags), *has))
            .collect();
        // --- p1w13-sonnet ---
        t.armor_template_sets = v.armor_sets.clone();
        for (slot, name) in &v.audio {
            t.audio_array[*slot as usize].get_or_insert_with(AudioEventName::default).event_name = name.clone();
        }
        t.structure_rubble_height = v.structure_rubble_height;
        // --- end p1w13-sonnet ---
        t
    }
}
// --- end p1w6-object ---

// ---------------------------------------------------------------------------
// isEquivalentTo
// ---------------------------------------------------------------------------

/// `ThingTemplate::isEquivalentTo(tt)`, for two templates given with their
/// override chains: the same object, the same template (the same final
/// override), one reskinned from the other (or both from the same), or one a
/// build variation of the other (by name, without regard to case).
pub fn is_equivalent_to(this: &crate::common::overridable::Overridable<ThingTemplate>, tt: &crate::common::overridable::Overridable<ThingTemplate>) -> bool {
    let (a, b) = (&this.data, &tt.data);
    // sanity
    if a.node_id == b.node_id {
        return true;
    }
    if this.get_final_override().data.node_id == tt.get_final_override().data.node_id {
        return true;
    }
    // This reskinned from that?
    if a.reskinned_from == Some(b.node_id) {
        return true;
    }
    // That reskinned from this?
    if b.reskinned_from == Some(a.node_id) {
        return true;
    }
    // This reskinned from that reskinned from? (NULL possible if not reskinned)
    if a.reskinned_from.is_some() && a.reskinned_from == b.reskinned_from {
        return true;
    }
    // Is this thing a build variation of that thing or vice versa
    if a.build_variations.iter().any(|v| v.eq_ignore_ascii_case(&b.name_string)) {
        return true;
    }
    if b.build_variations.iter().any(|v| v.eq_ignore_ascii_case(&a.name_string)) {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Parsing: the environment and the fill
// ---------------------------------------------------------------------------

/// What the `Object` block reads besides the template: the systems that the
/// field parsers look names up in.
pub struct ThingParseEnv<'a> {
    /// `TheNameKeyGenerator`.
    pub names: &'a mut NameKeyGenerator,
    /// `TheModuleFactory`.
    pub modules: &'a mut ModuleFactory,
    /// `TheArmorStore`.
    pub armors: &'a ArmorStore,
    /// `TheDamageFXStore`.
    pub damage_fx: &'a DamageFxStore,
    /// `TheWeaponStore`, as far as `findWeaponTemplate` uses it.
    pub weapons: &'a WeaponTemplateTable,
    /// `TheScienceStore`.
    pub sciences: &'a ScienceStore,
    /// `TheGameText`.
    pub text: &'a dyn GameTextFetch,
}

impl<'a> ThingParseEnv<'a> {
    /// The same systems, borrowed for a shorter time.
    pub fn reborrow(&mut self) -> ThingParseEnv<'_> {
        ThingParseEnv {
            names: &mut *self.names,
            modules: &mut *self.modules,
            armors: self.armors,
            damage_fx: self.damage_fx,
            weapons: self.weapons,
            sciences: self.sciences,
            text: self.text,
        }
    }
}

/// What the fields of an `Object` block fill: the template and the systems.
pub(crate) struct ObjectFill<'a> {
    pub(crate) tt: &'a mut ThingTemplate,
    pub(crate) env: ThingParseEnv<'a>,
}

/// A field that is `INI::parseReal` into a `Real` member.
macro_rules! real_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
            w.tt.$member = ini.parse_real()?;
            Ok(())
        }
    };
}

/// A field that is `INI::parseBool` into a `Bool` member.
macro_rules! bool_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
            w.tt.$member = ini.parse_bool()?;
            Ok(())
        }
    };
}

/// A field that is `INI::parseAsciiString` into a string member.
macro_rules! ascii_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
            w.tt.$member = ini.parse_ascii_string()?;
            Ok(())
        }
    };
}

/// A field that is `INI::parseUnsignedByte` into a `Byte` member.
macro_rules! byte_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
            w.tt.$member = ini.parse_unsigned_byte()?;
            Ok(())
        }
    };
}

/// A field that is `INI::parseUnsignedByte` into a **`Byte`** (signed) member: `TransportSlotCount` and
/// `StructureRubbleHeight`. A value over 127 wraps to a negative one, as the C++ conversion does, and
/// the getters sign-extend it (found by the differential test: 133 reads back as -123).
macro_rules! signed_byte_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
            w.tt.$member = ini.parse_unsigned_byte()? as i8;
            Ok(())
        }
    };
}

/// A field that is `INI::parseUnsignedShort` into a `UnsignedShort` member.
macro_rules! short_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
            w.tt.$member = ini.parse_unsigned_short()?;
            Ok(())
        }
    };
}

/// A field that is `INI::parseInt` into an `Int` member.
macro_rules! int_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
            w.tt.$member = ini.parse_int()?;
            Ok(())
        }
    };
}

/// A field that is `INI::parseByteSizedIndexList` into a `Byte` member.
macro_rules! index_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, ud: &UserData) -> IniResult<()> {
            w.tt.$member = ini.parse_byte_sized_index_list(ud.names())?;
            Ok(())
        }
    };
}

real_field!(f_fence_width, fence_width);
real_field!(f_fence_x_offset, fence_x_offset);
bool_field!(f_is_bridge, is_bridge);
real_field!(f_vision_range, vision_range);
real_field!(f_shroud_clearing_range, shroud_clearing_range);
real_field!(f_shroud_reveal_to_all_range, shroud_reveal_to_all_range);
real_field!(f_factory_exit_width, factory_exit_width);
real_field!(f_factory_extra_bib_width, factory_extra_bib_width);
bool_field!(f_is_trainable, is_trainable);
bool_field!(f_enter_guard, enter_guard);
bool_field!(f_hijack_guard, hijack_guard);
ascii_field!(f_side, default_owning_side);
real_field!(f_build_time, build_time);
bool_field!(f_is_forbidden, is_forbidden);
bool_field!(f_is_prerequisite, is_prerequisite);
ascii_field!(f_command_set, command_set_string);
ascii_field!(f_select_portrait, selected_portrait_image_name);
ascii_field!(f_button_image, button_image_name);
real_field!(f_scale, asset_scale);
real_field!(f_shadow_size_x, shadow_size_x);
real_field!(f_shadow_size_y, shadow_size_y);
real_field!(f_shadow_offset_x, shadow_offset_x);
real_field!(f_shadow_offset_y, shadow_offset_y);
ascii_field!(f_shadow_texture, shadow_texture_name);
real_field!(f_instance_scale_fuzziness, instance_scale_fuzziness);
signed_byte_field!(f_transport_slot_count, transport_slot_count);
signed_byte_field!(f_structure_rubble_height, structure_rubble_height);
byte_field!(f_crusher_level, crusher_level);
byte_field!(f_crushable_level, crushable_level);
short_field!(f_build_cost, build_cost);
short_field!(f_refund_value, refund_value);
short_field!(f_threat_value, threat_value);
int_field!(f_energy_production, energy_production);
int_field!(f_energy_bonus, energy_bonus);
index_field!(f_radar_priority, radar_priority);
index_field!(f_buildable, buildable);
index_field!(f_build_completion, build_completion);
index_field!(f_editor_sorting, editor_sorting);

/// `DisplayName`: `INI::parseAndTranslateLabel`; the field is untouched if it throws.
fn f_display_name(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    w.tt.display_name = parse_and_translate_label(ini, w.env.text)?;
    Ok(())
}

/// `PlacementViewAngle`: `INI::parseAngleReal`.
fn f_placement_view_angle(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    w.tt.placement_view_angle = ini.parse_angle_real()?;
    Ok(())
}

/// `DisplayColor`: `INI::parseColorInt`.
fn f_display_color(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    w.tt.display_color = ini.parse_color_int()?;
    Ok(())
}

/// `KindOf`: `KindOfMaskType::parseFromINI`.
fn f_kindof(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    w.tt.kindof.parse_from_ini(ini)
}

/// `BuildVariations`: `INI::parseAsciiStringVector`.
fn f_build_variations(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    ini.parse_ascii_string_vector(&mut w.tt.build_variations)
}

/// `UpgradeCameo1` to `5`: the slot is the entry's `Mask`.
fn f_upgrade_cameo(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, ud: &UserData) -> IniResult<()> {
    w.tt.upgrade_cameo_upgrade_names[ud.mask() as usize] = ini.parse_ascii_string()?;
    Ok(())
}

/// `OcclusionDelay`: `INI::parseDurationUnsignedInt`.
fn f_occlusion_delay(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    w.tt.occlusion_delay = ini.parse_duration_unsigned_int()?;
    Ok(())
}

/// `Shadow`: `INI::parseBitString8` over `TheShadowNames`.
fn f_shadow(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    w.tt.shadow_type = ini.parse_bit_string8(&SHADOW_NAMES)?;
    Ok(())
}

/// `ThingTemplate::parseIntList`: exactly `LEVEL_COUNT` integers; each is
/// stored as it is read, so a bad one leaves the earlier ones set.
fn parse_int_list(ini: &mut Ini<'_>, out: &mut [i32; LEVELS]) -> IniResult<()> {
    for slot in out.iter_mut() {
        let token = ini.get_next_token(None)?;
        *slot = Ini::scan_int(&token)?;
    }
    Ok(())
}

fn f_skill_point_value(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    parse_int_list(ini, &mut w.tt.skill_point_values)
}

fn f_experience_value(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    parse_int_list(ini, &mut w.tt.experience_values)
}

fn f_experience_required(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    parse_int_list(ini, &mut w.tt.experience_required)
}

/// `ThingTemplate::parseMaxSimultaneous`.
fn f_max_simultaneous(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    // Most of the time, this is an UnsignedShort, but sometimes this is the keyword
    const DETERMINED_BY_SUPERWEAPON_KEYWORD: &[u8] = b"DeterminedBySuperweaponRestriction";
    let token = ini.get_next_token(None)?;
    if token.eq_ignore_ascii_case(DETERMINED_BY_SUPERWEAPON_KEYWORD) {
        w.tt.max_simultaneous_determined_by_superweapon_restriction = true;
        w.tt.max_simultaneous_of_type = 0;
    } else {
        // Copied from parseUnsignedShort
        let value = Ini::scan_int(&token)?;
        if !(0..=65535).contains(&value) {
            return Err(IniError::new(IniErrorKind::Bug));
        }
        w.tt.max_simultaneous_of_type = value as u16;
        w.tt.max_simultaneous_determined_by_superweapon_restriction = false;
    }
    Ok(())
}

/// `NameKeyGenerator::parseStringAsNameKeyType`.
fn f_max_simultaneous_link_key(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    w.tt.max_simultaneous_link_key = w.env.names.parse_string_as_name_key_type(&token).map_err(name_keys_exhausted)?;
    Ok(())
}

// ---- the geometry fields

fn f_geometry_type(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, ud: &UserData) -> IniResult<()> {
    parse_geometry_type(ini, &mut w.tt.geometry_info, ud)
}
fn f_geometry_major_radius(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, ud: &UserData) -> IniResult<()> {
    parse_geometry_major_radius(ini, &mut w.tt.geometry_info, ud)
}
fn f_geometry_minor_radius(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, ud: &UserData) -> IniResult<()> {
    parse_geometry_minor_radius(ini, &mut w.tt.geometry_info, ud)
}
fn f_geometry_height(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, ud: &UserData) -> IniResult<()> {
    parse_geometry_height(ini, &mut w.tt.geometry_info, ud)
}
fn f_geometry_is_small(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, ud: &UserData) -> IniResult<()> {
    parse_geometry_is_small(ini, &mut w.tt.geometry_info, ud)
}

// ---- the sounds

/// The 36 sounds: `INI::parseDynamicAudioEventRTS` into the entry's slot
/// (`Mask`). `NoSound` (any case) deletes the sound; anything else creates it
/// if it is not there and sets its event name.
fn f_audio(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, ud: &UserData) -> IniResult<()> {
    let slot = ud.mask() as usize;
    let token = ini.get_next_token(None)?;
    if token.eq_ignore_ascii_case(b"NoSound") {
        w.tt.audio_array[slot] = None;
    } else {
        w.tt.audio_array[slot].get_or_insert_with(AudioEventName::default).event_name = token;
    }
    Ok(())
}

/// What `UnitSpecificSounds` fills.
struct SoundMapFill<'a>(&'a mut BTreeMap<Vec<u8>, AudioEventName>);

/// `parseArbitrarySoundsIntoMap`: the field token is the key (kept if there
/// is one already), the next token the event name.
fn f_unit_sound_entry(ini: &mut Ini<'_>, w: &mut SoundMapFill<'_>, ud: &UserData) -> IniResult<()> {
    let name = match ud {
        UserData::Token(t) => t.clone(),
        _ => Vec::new(),
    };
    let token = ini.get_next_token(None)?;
    w.0.entry(name).or_insert(AudioEventName { event_name: token });
    Ok(())
}

/// `ThingTemplate::parsePerUnitSounds`: clear, then every field line is an entry.
fn f_unit_specific_sounds(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    w.tt.per_unit_sounds.clear();
    let table = [FieldParse::catch_all(f_unit_sound_entry), FieldParse::end()];
    let mut fill = SoundMapFill(&mut w.tt.per_unit_sounds);
    ini.init_from_ini(&mut fill, &table)
}

/// What `UnitSpecificFX` fills.
struct FxMapFill<'a> {
    map: &'a mut BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    names: &'a mut NameKeyGenerator,
}

/// `parseArbitraryFXIntoMap`: the field token is the key (kept if there is
/// one already), the next token the `FXList` (`None` is null). The lookup,
/// `TheFXListStore->findFXList(token)`, is the client's, but it calls
/// `NAMEKEY(token)` for every name but `None` (FXList.cpp line 854), which
/// assigns the name a key, so the key is assigned here.
fn f_unit_fx_entry(ini: &mut Ini<'_>, w: &mut FxMapFill<'_>, ud: &UserData) -> IniResult<()> {
    let name = match ud {
        UserData::Token(t) => t.clone(),
        _ => Vec::new(),
    };
    let token = ini.get_next_token(None)?;
    let fx = if token.eq_ignore_ascii_case(b"None") {
        None
    } else {
        w.names.try_name_to_key(&token).map_err(name_keys_exhausted)?;
        Some(token)
    };
    w.map.entry(name).or_insert(fx);
    Ok(())
}

/// `ThingTemplate::parsePerUnitFX`: clear, then every field line is an entry.
fn f_unit_specific_fx(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    w.tt.per_unit_fx.clear();
    let table = [FieldParse::catch_all(f_unit_fx_entry), FieldParse::end()];
    let mut fill = FxMapFill { map: &mut w.tt.per_unit_fx, names: &mut *w.env.names };
    ini.init_from_ini(&mut fill, &table)
}

// ---- armor and weapon sets

/// `ThingTemplate::parseArmorTemplateSet`: the first set after the copy from
/// the default replaces the default's sets (before the block is read, so a
/// block that fails still replaced them); the new set is added if the block
/// reads.
fn f_armor_set(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    if w.tt.armor_copied_from_default {
        w.tt.armor_copied_from_default = false;
        w.tt.armor_template_sets.clear();
    }
    let mut ws = ArmorTemplateSet::new();
    ws.parse_armor_template_set(ini, w.env.names, w.env.armors, w.env.damage_fx)?;
    w.tt.armor_template_sets.push(ws);
    w.tt.armor_template_set_finder.clear();
    Ok(())
}

/// `ThingTemplate::parseWeaponTemplateSet`, as the armor sets.
fn f_weapon_set(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    if w.tt.weapons_copied_from_default {
        w.tt.weapons_copied_from_default = false;
        w.tt.weapon_template_sets.clear();
    }
    let mut ws = WeaponTemplateSet::new();
    ws.parse_weapon_template_set(ini, w.tt.template_id, w.env.names, w.env.weapons)?;
    w.tt.weapon_template_sets.push(ws);
    w.tt.weapon_template_set_finder.clear();
    Ok(())
}

// ---- prerequisites

/// What the lines of a `Prerequisites` block fill.
struct PrereqFill<'a> {
    list: &'a mut Vec<ProductionPrerequisite>,
    names: &'a mut NameKeyGenerator,
    sciences: &'a ScienceStore,
}

/// `parsePrerequisiteUnit`: every token is a unit, each an alternative to the
/// one before it; the first is required (`INI_INVALID_DATA` with none).
fn f_prereq_unit(ini: &mut Ini<'_>, w: &mut PrereqFill<'_>, _: &UserData) -> IniResult<()> {
    let mut prereq = ProductionPrerequisite::new();
    let mut or_unit_with_previous = false;
    let mut token = Some(ini.get_next_token(None)?);
    while let Some(t) = token {
        prereq.add_unit_prereq(&t, or_unit_with_previous);
        or_unit_with_previous = true;
        token = ini.get_next_token_or_null(None);
    }
    w.list.push(prereq);
    Ok(())
}

/// `parsePrerequisiteScience`: one science, checked against the store.
fn f_prereq_science(ini: &mut Ini<'_>, w: &mut PrereqFill<'_>, _: &UserData) -> IniResult<()> {
    let mut prereq = ProductionPrerequisite::new();
    let token = ini.get_next_token(None)?;
    let sciences = w.sciences;
    prereq.add_science_prereq(scan_science(w.names, |st| sciences.is_valid_science(st), &token)?);
    w.list.push(prereq);
    Ok(())
}

/// `ThingTemplate::parsePrerequisites`: an override load starts the list
/// afresh; the block's `Object` and `Science` lines each add an entry.
fn f_prerequisites(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    if ini.get_load_type() == IniLoadType::CreateOverrides {
        w.tt.prereq_info.clear();
    }
    let table = [
        FieldParse::new("Object", f_prereq_unit, UserData::None),
        FieldParse::new("Science", f_prereq_science, UserData::None),
        FieldParse::end(),
    ];
    let mut fill = PrereqFill { list: &mut w.tt.prereq_info, names: &mut *w.env.names, sciences: w.env.sciences };
    ini.init_from_ini(&mut fill, &table)
}

// ---- modules

/// The four fields of a client module's block that open a nested block.
const CLIENT_NESTED_BLOCKS: [&str; 4] = ["DefaultConditionState", "ConditionState", "AliasConditionState", "TransitionState"];

struct ClientFill {
    lines: Vec<ClientLine>,
    depth: u32,
}

fn client_args(ini: &mut Ini<'_>) -> Vec<Vec<u8>> {
    let mut args = Vec::new();
    while let Some(t) = ini.get_next_token_or_null(None) {
        args.push(t);
    }
    args
}

/// Any field of a client module: kept as written.
fn f_client_line(ini: &mut Ini<'_>, w: &mut ClientFill, ud: &UserData) -> IniResult<()> {
    let field = match ud {
        UserData::Token(t) => t.clone(),
        _ => Vec::new(),
    };
    let args = client_args(ini);
    w.lines.push(ClientLine { depth: w.depth, field, args });
    Ok(())
}

/// A field that opens a nested block: kept, then its lines until its `End`.
fn f_client_nested(ini: &mut Ini<'_>, w: &mut ClientFill, ud: &UserData) -> IniResult<()> {
    let field = CLIENT_NESTED_BLOCKS[ud.mask() as usize].as_bytes().to_vec();
    let args = client_args(ini);
    w.lines.push(ClientLine { depth: w.depth, field, args });
    w.depth += 1;
    ini.init_from_ini(w, &client_fields())?;
    w.depth -= 1;
    Ok(())
}

fn client_fields() -> [FieldParse<ClientFill>; 6] {
    [
        FieldParse::new(CLIENT_NESTED_BLOCKS[0], f_client_nested, UserData::Mask(0)),
        FieldParse::new(CLIENT_NESTED_BLOCKS[1], f_client_nested, UserData::Mask(1)),
        FieldParse::new(CLIENT_NESTED_BLOCKS[2], f_client_nested, UserData::Mask(2)),
        FieldParse::new(CLIENT_NESTED_BLOCKS[3], f_client_nested, UserData::Mask(3)),
        FieldParse::catch_all(f_client_line),
        FieldParse::end(),
    ]
}

/// The block of a client module, up to its `End`.
fn read_client_module_block(ini: &mut Ini<'_>) -> IniResult<ClientModuleData> {
    let mut fill = ClientFill { lines: Vec::new(), depth: 0 };
    ini.init_from_ini(&mut fill, &client_fields())?;
    Ok(ClientModuleData { module_tag_name_key: NameKeyType::INVALID, lines: fill.lines })
}

/// How `parseModuleName` reports what `ModuleFactory` could not do.
fn module_error(e: ModuleFactoryError) -> IniError {
    match e {
        ModuleFactoryError::Ini(e) => e,
        ModuleFactoryError::NameKey(_) => IniError::undefined(Undefined::NameKeysExhausted),
        ModuleFactoryError::NotTranslated { name, .. } => IniError::new(IniErrorKind::ModuleNotTranslated(name)),
        ModuleFactoryError::UnknownModule { name, .. } => IniError::new(IniErrorKind::ModuleUnknown(name)),
        ModuleFactoryError::NotInShippedBuild { name } => IniError::new(IniErrorKind::ModuleNotInShippedBuild(name)),
        ModuleFactoryError::EmptyName => IniError::new(IniErrorKind::InvalidData),
        ModuleFactoryError::InterfaceMaskMismatch { .. } | ModuleFactoryError::ModuleDataShared { .. } => IniError::new(IniErrorKind::Bug),
    }
}

/// `Body` is the one `parseModuleName` user whose `userData` is not a
/// `ModuleType`: 999.
const BODY: u32 = 999;

/// `ThingTemplate::parseModuleName`: `Behavior`, `Body`, `Draw` and
/// `ClientUpdate`. The entry's `Mask` is the `ModuleType` (or 999).
fn f_module(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, ud: &UserData) -> IniResult<()> {
    let which = ud.mask();
    let token = ini.get_next_token(None)?;
    // get the tag string (it is now required)
    let module_tag = ini.get_next_token(None)?;

    let module_type = if which == BODY {
        ModuleType::Behavior
    } else {
        ModuleType::from_i32(which as i32).unwrap_or(ModuleType::Behavior)
    };

    // what interface(s) does this module support?
    let interface_mask = match module_type {
        ModuleType::Behavior => w.env.modules.find_module_interface_mask(w.env.names, &token, module_type).map_err(module_error)?,
        ModuleType::Draw | ModuleType::ClientUpdate => {
            // the client's modules: the key of the decorated name is assigned, as upstream's lookup does
            ModuleFactory::make_decorated_name_key(w.env.names, &token, module_type).map_err(name_keys_exhausted)?;
            if module_type == ModuleType::Draw {
                MODULEINTERFACE_DRAW
            } else {
                MODULEINTERFACE_CLIENT_UPDATE
            }
        }
    };
    if which == BODY {
        if interface_mask & MODULEINTERFACE_BODY == 0 {
            // Only Body allowed here
            return Err(IniError::new(IniErrorKind::InvalidData));
        }
    } else if interface_mask & MODULEINTERFACE_BODY != 0 {
        // No Body allowed here
        return Err(IniError::new(IniErrorKind::InvalidData));
    }

    // if we're overriding, we can totally skip over this block
    if ini.get_load_type() == IniLoadType::CreateOverrides {
        if w.tt.module_parsing_mode != ModuleParseMode::AddRemoveReplace {
            // You must use AddModule to add modules in override INI files.
            return Err(IniError::new(IniErrorKind::InvalidData));
        }
    } else {
        let kindof = w.tt.kindof;
        let trainable = w.tt.is_trainable;
        w.tt.behavior_module_info.clear_copied_from_default_entries(interface_mask, &token, &kindof, trainable);
        w.tt.draw_module_info.clear_copied_from_default_entries(interface_mask, &token, &kindof, trainable);
        w.tt.client_update_module_info.clear_copied_from_default_entries(interface_mask, &token, &kindof, trainable);
    }

    if w.tt.module_parsing_mode == ModuleParseMode::AddRemoveReplace
        && !w.tt.module_being_replaced_name.is_empty()
        && w.tt.module_being_replaced_name != token
    {
        // ReplaceModule must replace modules with another module of the same type
        return Err(IniError::new(IniErrorKind::InvalidData));
    }

    if w.tt.module_parsing_mode == ModuleParseMode::AddRemoveReplace
        && !w.tt.module_being_replaced_tag.is_empty()
        && w.tt.module_being_replaced_tag == module_tag
    {
        // ReplaceModule must specify a new, unique tag for the replaced module
        return Err(IniError::new(IniErrorKind::InvalidData));
    }

    let data = match module_type {
        ModuleType::Behavior => {
            let d = w
                .env
                .modules
                .new_module_data_from_ini(w.env.names, Some(ini), &token, module_type, &module_tag)
                .map_err(module_error)?;
            // an empty name gives no data; a name here is never empty
            let d = d.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
            ModuleInfoData::Logic(d)
        }
        ModuleType::Draw | ModuleType::ClientUpdate => {
            let mut block = read_client_module_block(ini)?;
            block.module_tag_name_key = w.env.names.try_name_to_key(&module_tag).map_err(name_keys_exhausted)?;
            ModuleInfoData::Client(block)
        }
    };

    let mi = match module_type {
        ModuleType::Behavior => &mut w.tt.behavior_module_info,
        ModuleType::Draw => &mut w.tt.draw_module_info,
        ModuleType::ClientUpdate => &mut w.tt.client_update_module_info,
    };
    if data.is_ai_module_data() {
        mi.clear_ai_module_info();
    }

    let inheritable = w.tt.module_parsing_mode == ModuleParseMode::Inheritable;
    let overrideable_by_like_kind = w.tt.module_parsing_mode == ModuleParseMode::OverrideableByLikeKind;
    let mi = match module_type {
        ModuleType::Behavior => &mut w.tt.behavior_module_info,
        ModuleType::Draw => &mut w.tt.draw_module_info,
        ModuleType::ClientUpdate => &mut w.tt.client_update_module_info,
    };
    mi.add_module_info(&token, &module_tag, data, interface_mask, inheritable, overrideable_by_like_kind);
    Ok(())
}

/// The shared shape of `AddModule`, `InheritableModule` and
/// `OverrideableByLikeKind`: refuse inside another, set the mode, read the
/// object's fields up to `End`, restore the mode (not if the read throws).
fn nested_module_block(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, mode: ModuleParseMode) -> IniResult<()> {
    let old_mode = w.tt.module_parsing_mode;
    if old_mode != ModuleParseMode::Normal {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    w.tt.module_parsing_mode = mode;
    ini.init_from_ini(w, &object_fields())?;
    w.tt.module_parsing_mode = old_mode;
    Ok(())
}

fn f_add_module(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    nested_module_block(ini, w, ModuleParseMode::AddRemoveReplace)
}

fn f_inheritable_module(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    nested_module_block(ini, w, ModuleParseMode::Inheritable)
}

fn f_overrideable_by_like_kind(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    nested_module_block(ini, w, ModuleParseMode::OverrideableByLikeKind)
}

/// `ThingTemplate::parseRemoveModule`: the mode is set before the tag is read
/// and is not restored if the tag is missing or finds no module.
fn f_remove_module(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    let old_mode = w.tt.module_parsing_mode;
    if old_mode != ModuleParseMode::Normal {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    w.tt.module_parsing_mode = ModuleParseMode::AddRemoveReplace;
    let mod_to_remove = ini.get_next_token(None)?;
    let mut removed_module_name = Vec::new();
    if !w.tt.remove_module_info(&mod_to_remove, &mut removed_module_name) {
        // RemoveModule %s was not found for %s. The game will crash now!
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    w.tt.module_parsing_mode = old_mode;
    Ok(())
}

/// `ThingTemplate::parseReplaceModule`: as `RemoveModule`, then the module
/// block that replaces it, which must be of the same class and have a new tag.
fn f_replace_module(ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    let old_mode = w.tt.module_parsing_mode;
    if old_mode != ModuleParseMode::Normal {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    w.tt.module_parsing_mode = ModuleParseMode::AddRemoveReplace;
    let mod_to_remove = ini.get_next_token(None)?;
    let mut removed_module_name = Vec::new();
    if !w.tt.remove_module_info(&mod_to_remove, &mut removed_module_name) {
        // ReplaceModule %s was not found for %s; cannot continue.
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    w.tt.module_being_replaced_name = removed_module_name;
    w.tt.module_being_replaced_tag = mod_to_remove;
    ini.init_from_ini(w, &object_fields())?;
    w.tt.module_being_replaced_name.clear();
    w.tt.module_being_replaced_tag.clear();
    w.tt.module_parsing_mode = old_mode;
    Ok(())
}

/// `Locomotor`: `AIUpdateModuleData::parseLocomotorSet`, a cut point (see the
/// module docs). With no AI module data (and none can exist yet) it throws
/// `INI_INVALID_DATA`, as upstream does for a template without an `AIUpdate`.
fn f_locomotor(_ini: &mut Ini<'_>, w: &mut ObjectFill<'_>, _: &UserData) -> IniResult<()> {
    if !w.tt.has_ai_module() {
        // Attempted to specify a locomotor for object without an AIUpdate block.
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    // An AI module's data is not translated: unreachable until layer 9.
    Err(IniError::new(IniErrorKind::Bug))
}

// ---------------------------------------------------------------------------
// The tables
// ---------------------------------------------------------------------------

/// The tokens of `s_objectFieldParseTable`, in order.
pub const OBJECT_FIELD_NAMES: [&str; 110] = [
    "DisplayName",
    "RadarPriority",
    "TransportSlotCount",
    "FenceWidth",
    "FenceXOffset",
    "IsBridge",
    "ArmorSet",
    "WeaponSet",
    "VisionRange",
    "ShroudClearingRange",
    "ShroudRevealToAllRange",
    "PlacementViewAngle",
    "FactoryExitWidth",
    "FactoryExtraBibWidth",
    "SkillPointValue",
    "ExperienceValue",
    "ExperienceRequired",
    "IsTrainable",
    "EnterGuard",
    "HijackGuard",
    "Side",
    "Prerequisites",
    "Buildable",
    "BuildCost",
    "BuildTime",
    "RefundValue",
    "BuildCompletion",
    "EnergyProduction",
    "EnergyBonus",
    "IsForbidden",
    "IsPrerequisite",
    "DisplayColor",
    "EditorSorting",
    "KindOf",
    "CommandSet",
    "BuildVariations",
    "Behavior",
    "Body",
    "Draw",
    "ClientUpdate",
    "SelectPortrait",
    "ButtonImage",
    "UpgradeCameo1",
    "UpgradeCameo2",
    "UpgradeCameo3",
    "UpgradeCameo4",
    "UpgradeCameo5",
    "VoiceSelect",
    "VoiceGroupSelect",
    "VoiceMove",
    "VoiceAttack",
    "VoiceEnter",
    "VoiceFear",
    "VoiceSelectElite",
    "VoiceCreated",
    "VoiceTaskUnable",
    "VoiceTaskComplete",
    "VoiceMeetEnemy",
    "VoiceGarrison",
    "VoiceDefect",
    "VoiceAttackSpecial",
    "VoiceAttackAir",
    "VoiceGuard",
    "SoundMoveStart",
    "SoundMoveStartDamaged",
    "SoundMoveLoop",
    "SoundMoveLoopDamaged",
    "SoundAmbient",
    "SoundAmbientDamaged",
    "SoundAmbientReallyDamaged",
    "SoundAmbientRubble",
    "SoundStealthOn",
    "SoundStealthOff",
    "SoundCreated",
    "SoundOnDamaged",
    "SoundOnReallyDamaged",
    "SoundEnter",
    "SoundExit",
    "SoundPromotedVeteran",
    "SoundPromotedElite",
    "SoundPromotedHero",
    "SoundFallingFromPlane",
    "UnitSpecificSounds",
    "UnitSpecificFX",
    "Scale",
    "Geometry",
    "GeometryMajorRadius",
    "GeometryMinorRadius",
    "GeometryHeight",
    "GeometryIsSmall",
    "Shadow",
    "ShadowSizeX",
    "ShadowSizeY",
    "ShadowOffsetX",
    "ShadowOffsetY",
    "ShadowTexture",
    "OcclusionDelay",
    "AddModule",
    "RemoveModule",
    "ReplaceModule",
    "InheritableModule",
    "OverrideableByLikeKind",
    "Locomotor",
    "InstanceScaleFuzziness",
    "StructureRubbleHeight",
    "ThreatValue",
    "MaxSimultaneousOfType",
    "MaxSimultaneousLinkKey",
    "CrusherLevel",
    "CrushableLevel",
];

/// The tokens of `s_objectReskinFieldParseTable`, in order.
pub const RESKIN_FIELD_NAMES: [&str; 10] = [
    "Draw",
    "Geometry",
    "GeometryMajorRadius",
    "GeometryMinorRadius",
    "GeometryHeight",
    "GeometryIsSmall",
    "FenceWidth",
    "FenceXOffset",
    "MaxSimultaneousOfType",
    "MaxSimultaneousLinkKey",
];

/// `s_objectFieldParseTable`, in upstream's order.
pub(crate) fn object_fields<'a>() -> [FieldParse<ObjectFill<'a>>; 111] {
    use ThingTemplateAudioType as A;
    let n = UserData::None;
    let audio = |t: A| UserData::Mask(t as u32);
    [
        FieldParse::new("DisplayName", f_display_name, n.clone()),
        FieldParse::new("RadarPriority", f_radar_priority, UserData::Names(&RADAR_PRIORITY_NAMES)),
        FieldParse::new("TransportSlotCount", f_transport_slot_count, n.clone()),
        FieldParse::new("FenceWidth", f_fence_width, n.clone()),
        FieldParse::new("FenceXOffset", f_fence_x_offset, n.clone()),
        FieldParse::new("IsBridge", f_is_bridge, n.clone()),
        FieldParse::new("ArmorSet", f_armor_set, n.clone()),
        FieldParse::new("WeaponSet", f_weapon_set, n.clone()),
        FieldParse::new("VisionRange", f_vision_range, n.clone()),
        FieldParse::new("ShroudClearingRange", f_shroud_clearing_range, n.clone()),
        FieldParse::new("ShroudRevealToAllRange", f_shroud_reveal_to_all_range, n.clone()),
        FieldParse::new("PlacementViewAngle", f_placement_view_angle, n.clone()),
        FieldParse::new("FactoryExitWidth", f_factory_exit_width, n.clone()),
        FieldParse::new("FactoryExtraBibWidth", f_factory_extra_bib_width, n.clone()),
        FieldParse::new("SkillPointValue", f_skill_point_value, n.clone()),
        FieldParse::new("ExperienceValue", f_experience_value, n.clone()),
        FieldParse::new("ExperienceRequired", f_experience_required, n.clone()),
        FieldParse::new("IsTrainable", f_is_trainable, n.clone()),
        FieldParse::new("EnterGuard", f_enter_guard, n.clone()),
        FieldParse::new("HijackGuard", f_hijack_guard, n.clone()),
        FieldParse::new("Side", f_side, n.clone()),
        FieldParse::new("Prerequisites", f_prerequisites, n.clone()),
        FieldParse::new("Buildable", f_buildable, UserData::Names(&BUILDABLE_STATUS_NAMES)),
        FieldParse::new("BuildCost", f_build_cost, n.clone()),
        FieldParse::new("BuildTime", f_build_time, n.clone()),
        FieldParse::new("RefundValue", f_refund_value, n.clone()),
        FieldParse::new("BuildCompletion", f_build_completion, UserData::Names(&BUILD_COMPLETION_NAMES)),
        FieldParse::new("EnergyProduction", f_energy_production, n.clone()),
        FieldParse::new("EnergyBonus", f_energy_bonus, n.clone()),
        FieldParse::new("IsForbidden", f_is_forbidden, n.clone()),
        FieldParse::new("IsPrerequisite", f_is_prerequisite, n.clone()),
        FieldParse::new("DisplayColor", f_display_color, n.clone()),
        FieldParse::new("EditorSorting", f_editor_sorting, UserData::Names(&EDITOR_SORTING_NAMES)),
        FieldParse::new("KindOf", f_kindof, n.clone()),
        FieldParse::new("CommandSet", f_command_set, n.clone()),
        FieldParse::new("BuildVariations", f_build_variations, n.clone()),
        FieldParse::new("Behavior", f_module, UserData::Mask(ModuleType::Behavior as u32)),
        FieldParse::new("Body", f_module, UserData::Mask(BODY)),
        FieldParse::new("Draw", f_module, UserData::Mask(ModuleType::Draw as u32)),
        FieldParse::new("ClientUpdate", f_module, UserData::Mask(ModuleType::ClientUpdate as u32)),
        FieldParse::new("SelectPortrait", f_select_portrait, n.clone()),
        FieldParse::new("ButtonImage", f_button_image, n.clone()),
        FieldParse::new("UpgradeCameo1", f_upgrade_cameo, UserData::Mask(0)),
        FieldParse::new("UpgradeCameo2", f_upgrade_cameo, UserData::Mask(1)),
        FieldParse::new("UpgradeCameo3", f_upgrade_cameo, UserData::Mask(2)),
        FieldParse::new("UpgradeCameo4", f_upgrade_cameo, UserData::Mask(3)),
        FieldParse::new("UpgradeCameo5", f_upgrade_cameo, UserData::Mask(4)),
        FieldParse::new("VoiceSelect", f_audio, audio(A::VoiceSelect)),
        FieldParse::new("VoiceGroupSelect", f_audio, audio(A::VoiceGroupSelect)),
        FieldParse::new("VoiceMove", f_audio, audio(A::VoiceMove)),
        FieldParse::new("VoiceAttack", f_audio, audio(A::VoiceAttack)),
        FieldParse::new("VoiceEnter", f_audio, audio(A::VoiceEnter)),
        FieldParse::new("VoiceFear", f_audio, audio(A::VoiceFear)),
        FieldParse::new("VoiceSelectElite", f_audio, audio(A::VoiceSelectElite)),
        FieldParse::new("VoiceCreated", f_audio, audio(A::VoiceCreated)),
        FieldParse::new("VoiceTaskUnable", f_audio, audio(A::VoiceTaskUnable)),
        FieldParse::new("VoiceTaskComplete", f_audio, audio(A::VoiceTaskComplete)),
        FieldParse::new("VoiceMeetEnemy", f_audio, audio(A::VoiceMeetEnemy)),
        FieldParse::new("VoiceGarrison", f_audio, audio(A::VoiceGarrison)),
        FieldParse::new("VoiceDefect", f_audio, audio(A::VoiceDefect)),
        FieldParse::new("VoiceAttackSpecial", f_audio, audio(A::VoiceAttackSpecial)),
        FieldParse::new("VoiceAttackAir", f_audio, audio(A::VoiceAttackAir)),
        FieldParse::new("VoiceGuard", f_audio, audio(A::VoiceGuard)),
        FieldParse::new("SoundMoveStart", f_audio, audio(A::SoundMoveStart)),
        FieldParse::new("SoundMoveStartDamaged", f_audio, audio(A::SoundMoveStartDamaged)),
        FieldParse::new("SoundMoveLoop", f_audio, audio(A::SoundMoveLoop)),
        FieldParse::new("SoundMoveLoopDamaged", f_audio, audio(A::SoundMoveLoopDamaged)),
        FieldParse::new("SoundAmbient", f_audio, audio(A::SoundAmbient)),
        FieldParse::new("SoundAmbientDamaged", f_audio, audio(A::SoundAmbientDamaged)),
        FieldParse::new("SoundAmbientReallyDamaged", f_audio, audio(A::SoundAmbientReallyDamaged)),
        FieldParse::new("SoundAmbientRubble", f_audio, audio(A::SoundAmbientRubble)),
        FieldParse::new("SoundStealthOn", f_audio, audio(A::SoundStealthOn)),
        FieldParse::new("SoundStealthOff", f_audio, audio(A::SoundStealthOff)),
        FieldParse::new("SoundCreated", f_audio, audio(A::SoundCreated)),
        FieldParse::new("SoundOnDamaged", f_audio, audio(A::SoundOnDamaged)),
        FieldParse::new("SoundOnReallyDamaged", f_audio, audio(A::SoundOnReallyDamaged)),
        FieldParse::new("SoundEnter", f_audio, audio(A::SoundEnter)),
        FieldParse::new("SoundExit", f_audio, audio(A::SoundExit)),
        FieldParse::new("SoundPromotedVeteran", f_audio, audio(A::SoundPromotedVeteran)),
        FieldParse::new("SoundPromotedElite", f_audio, audio(A::SoundPromotedElite)),
        FieldParse::new("SoundPromotedHero", f_audio, audio(A::SoundPromotedHero)),
        FieldParse::new("SoundFallingFromPlane", f_audio, audio(A::SoundFalling)),
        FieldParse::new("UnitSpecificSounds", f_unit_specific_sounds, n.clone()),
        FieldParse::new("UnitSpecificFX", f_unit_specific_fx, n.clone()),
        FieldParse::new("Scale", f_scale, n.clone()),
        FieldParse::new("Geometry", f_geometry_type, n.clone()),
        FieldParse::new("GeometryMajorRadius", f_geometry_major_radius, n.clone()),
        FieldParse::new("GeometryMinorRadius", f_geometry_minor_radius, n.clone()),
        FieldParse::new("GeometryHeight", f_geometry_height, n.clone()),
        FieldParse::new("GeometryIsSmall", f_geometry_is_small, n.clone()),
        FieldParse::new("Shadow", f_shadow, UserData::Names(&SHADOW_NAMES)),
        FieldParse::new("ShadowSizeX", f_shadow_size_x, n.clone()),
        FieldParse::new("ShadowSizeY", f_shadow_size_y, n.clone()),
        FieldParse::new("ShadowOffsetX", f_shadow_offset_x, n.clone()),
        FieldParse::new("ShadowOffsetY", f_shadow_offset_y, n.clone()),
        FieldParse::new("ShadowTexture", f_shadow_texture, n.clone()),
        FieldParse::new("OcclusionDelay", f_occlusion_delay, n.clone()),
        FieldParse::new("AddModule", f_add_module, n.clone()),
        FieldParse::new("RemoveModule", f_remove_module, n.clone()),
        FieldParse::new("ReplaceModule", f_replace_module, n.clone()),
        FieldParse::new("InheritableModule", f_inheritable_module, n.clone()),
        FieldParse::new("OverrideableByLikeKind", f_overrideable_by_like_kind, n.clone()),
        FieldParse::new("Locomotor", f_locomotor, n.clone()),
        FieldParse::new("InstanceScaleFuzziness", f_instance_scale_fuzziness, n.clone()),
        FieldParse::new("StructureRubbleHeight", f_structure_rubble_height, n.clone()),
        FieldParse::new("ThreatValue", f_threat_value, n.clone()),
        FieldParse::new("MaxSimultaneousOfType", f_max_simultaneous, n.clone()),
        FieldParse::new("MaxSimultaneousLinkKey", f_max_simultaneous_link_key, n.clone()),
        FieldParse::new("CrusherLevel", f_crusher_level, n.clone()),
        FieldParse::new("CrushableLevel", f_crushable_level, n),
        FieldParse::end(),
    ]
}

/// `s_objectReskinFieldParseTable`: "only things that affect the appearance
/// of a template".
pub(crate) fn reskin_fields<'a>() -> [FieldParse<ObjectFill<'a>>; 11] {
    let n = UserData::None;
    [
        FieldParse::new("Draw", f_module, UserData::Mask(ModuleType::Draw as u32)),
        FieldParse::new("Geometry", f_geometry_type, n.clone()),
        FieldParse::new("GeometryMajorRadius", f_geometry_major_radius, n.clone()),
        FieldParse::new("GeometryMinorRadius", f_geometry_minor_radius, n.clone()),
        FieldParse::new("GeometryHeight", f_geometry_height, n.clone()),
        FieldParse::new("GeometryIsSmall", f_geometry_is_small, n.clone()),
        FieldParse::new("FenceWidth", f_fence_width, n.clone()),
        FieldParse::new("FenceXOffset", f_fence_x_offset, n.clone()),
        FieldParse::new("MaxSimultaneousOfType", f_max_simultaneous, n.clone()),
        FieldParse::new("MaxSimultaneousLinkKey", f_max_simultaneous_link_key, n),
        FieldParse::end(),
    ]
}
