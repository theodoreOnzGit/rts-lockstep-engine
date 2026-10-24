// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Upgrade.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/Upgrade.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIUpgrade.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/System/Xfer.cpp (lines 732-829:
//   `Xfer::xferUpgradeMask`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, March 2002 (Xfer.cpp: Colin Day, February 2002)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The upgrade system: `UpgradeTemplate` (what an upgrade costs and is
//! called), `UpgradeCenter` (every template read from INI, each with its own
//! bit of the 128-bit `UpgradeMaskType`), `Upgrade` (one instance of an
//! upgrade, with its status and xfer), the `Upgrade` INI block, and
//! `Xfer::xferUpgradeMask`.
//!
//! # Translation choices
//!
//! - **The template list.** Upstream links the templates into a doubly
//!   linked list with **new ones at the head**, and searches, iterates and
//!   (in `xferUpgradeMask`) saves in that order. Here a `Vec` in creation
//!   order, read backwards: [`UpgradeCenter::templates`] and
//!   [`UpgradeCenter::first_upgrade_template`] are newest first, as upstream.
//!   `unlinkUpgrade` is protected and nothing calls it, so it is not
//!   translated; templates are never removed.
//! - **Pointers.** `Upgrade` holds a `const UpgradeTemplate*` upstream, here
//!   the template's name key ([`Upgrade::template`], resolved through
//!   [`UpgradeCenter::find_upgrade_by_key`]), as `logic::armor::Armor` does.
//!   `findUpgrade(name)` calls `nameToKey`, which **assigns a key to a name it
//!   has not seen**, and key values are lockstep state, so every call that
//!   upstream makes that way takes the [`NameKeyGenerator`], in upstream's
//!   order of calls.
//! - **The mask bit.** `newUpgrade` gives each template the next bit of the
//!   mask. A 129th template makes `std::bitset::set(128)` throw
//!   `std::out_of_range` (the `DEBUG_ASSERTCRASH` after it is compiled out in
//!   the shipped build), which is [`UpgradeCenterError::MaskBit`]; the
//!   template that was being made is not linked and the counter stays at 128.
//!   Upstream defines no name list for `BitFlags<128>` (the xfer saves the
//!   templates' names instead), so [`UpgradeMaskType`]'s names are empty
//!   placeholders and are never looked up.
//! - **`DefaultUpgrade`.** A new template starts as a copy of the template
//!   named `DefaultUpgrade` if there is one (`*newUpgrade = *defaultUpgrade`).
//! - **The veterancy "upgrades".** [`UpgradeCenter::init`] makes three
//!   templates named `Upgrade_Veterancy_VETERAN`, `_ELITE` and `_HEROIC`
//!   (per-object, free), taking the first three mask bits when the INI has
//!   not yet defined any.
//!
//! # Cut points: images and sounds are names
//!
//! `ButtonImage` is kept as the **name as written**
//! ([`UpgradeTemplate::button_image_name`]); `cacheButtonImage` and
//! `getButtonImage`, which resolve it through `TheMappedImageCollection` and
//! then clear the name, belong to the client, as does `UpgradeCenter::reset`
//! (its only job is to call `cacheButtonImage`). `ResearchSound` and
//! `UnitSpecificSound` are [`AudioEventName`]s (see there).
//!
//! # Left out: methods that take a `Player`
//!
//! These belong to the layer that has players (7/8):
//! `UpgradeTemplate::calcTimeToBuild(Player*)` (in the shipped build it ignores
//! the player; the `buildsInstantly()` cheat exists only in debug builds) and
//! `calcCostToBuild(Player*)` (ignores it too, `@todo` handicaps), of which the
//! player-free results are [`UpgradeTemplate::time_to_build_frames`] and
//! [`UpgradeTemplate::cost_to_build`]; and
//! `UpgradeCenter::canAffordUpgrade(Player*, ...)` (money check and the
//! `GUI:NotEnoughMoneyToUpgrade` message). Granting, queueing and the
//! per-player upgrade lists (`Upgrade::friend_*` next/prev pointers) live in
//! `Player` and `Object`.
//!
//! # Where upstream is undefined
//!
//! - `calcTimeToBuild` converts `m_buildTime * 30` to `Int`: a result outside
//!   `Int` is [`Undefined::FloatToIntOutOfRange`].
//! - `NameKeyGenerator` past its last key: [`NameKeyError::KeysExhausted`].
//!
//! # `xferUpgradeMask` is verified by Rust tests only
//!
//! The Xfer build of the differential tests stands `TheUpgradeCenter` in with
//! functions that abort (`differential/wrap_xfer.cpp`), so the upstream
//! function cannot be run against this one yet. See [`xfer_upgrade_mask`].

use super::academy_classification::{AcademyClassificationType, ACADEMY_CLASSIFICATION_TYPE_NAMES};
use super::audio_event_name::AudioEventName;
use super::bit_flags::{BitFlagNames, BitFlags, BitIndexOutOfRange};
use super::game_common::{VeterancyLevel, LOGICFRAMES_PER_SECOND, VETERANCY_NAMES};
use super::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, Undefined, UserData};
use super::name_key_generator::{NameKeyError, NameKeyGenerator, NameKeyType};
use super::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};

/// `enum UpgradeStatusType`. (An `i32`, like the other enums that are saved:
/// `Upgrade::xfer` copies four raw bytes into `m_status`, which can then hold
/// a value outside the enumerators; the Rust keeps what was read.)
pub type UpgradeStatusType = i32;
/// `UPGRADE_STATUS_INVALID`.
pub const UPGRADE_STATUS_INVALID: UpgradeStatusType = 0;
/// `UPGRADE_STATUS_IN_PRODUCTION`.
pub const UPGRADE_STATUS_IN_PRODUCTION: UpgradeStatusType = 1;
/// `UPGRADE_STATUS_COMPLETE`.
pub const UPGRADE_STATUS_COMPLETE: UpgradeStatusType = 2;

/// `UPGRADE_MAX_COUNT`: "the maximum number of upgrades".
pub const UPGRADE_MAX_COUNT: usize = 128;

/// The (placeholder) names of `BitFlags<UPGRADE_MAX_COUNT>`; see the module
/// docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UpgradeMaskNames;
impl BitFlagNames for UpgradeMaskNames {
    const NUM_BITS: usize = UPGRADE_MAX_COUNT;
    const NAMES: &'static [&'static str] = &[""; UPGRADE_MAX_COUNT];
}

/// `typedef BitFlags<UPGRADE_MAX_COUNT> UpgradeMaskType`.
pub type UpgradeMaskType = BitFlags<UpgradeMaskNames>;

/// `MAKE_UPGRADE_MASK(k)` to `MAKE_UPGRADE_MASK5`: the listed bits set (an
/// index outside `[0, 128)` makes `std::bitset::set` throw).
pub fn make_upgrade_mask(indices: &[i32]) -> Result<UpgradeMaskType, BitIndexOutOfRange> {
    UpgradeMaskType::try_from_indices(indices)
}

/// `TEST_UPGRADE_MASK(m, index)`.
pub fn test_upgrade_mask(m: &UpgradeMaskType, index: i32) -> Result<bool, BitIndexOutOfRange> {
    m.try_test(index)
}

/// `TEST_UPGRADE_MASK_ANY(m, mask)`.
pub fn test_upgrade_mask_any(m: &UpgradeMaskType, mask: &UpgradeMaskType) -> bool {
    m.any_intersection_with(mask)
}

/// `TEST_UPGRADE_MASK_MULTI(m, mustBeSet, mustBeClear)`.
pub fn test_upgrade_mask_multi(
    m: &UpgradeMaskType,
    must_be_set: &UpgradeMaskType,
    must_be_clear: &UpgradeMaskType,
) -> bool {
    m.test_set_and_clear(must_be_set, must_be_clear)
}

/// `UPGRADE_MASK_ANY_SET(m)`.
pub fn upgrade_mask_any_set(m: &UpgradeMaskType) -> bool {
    m.any()
}

/// `CLEAR_UPGRADE_MASK(m)`.
pub fn clear_upgrade_mask(m: &mut UpgradeMaskType) {
    m.clear();
}

/// `SET_ALL_UPGRADE_MASK_BITS(m)`: clear, then flip.
pub fn set_all_upgrade_mask_bits(m: &mut UpgradeMaskType) {
    m.clear();
    m.flip();
}

/// `FLIP_UPGRADE_MASK(m)`.
pub fn flip_upgrade_mask(m: &mut UpgradeMaskType) {
    m.flip();
}

/// `class Upgrade`: one **instance** of an upgrade (its template, by name
/// key, and its status).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Upgrade {
    /// `m_template`, as the template's name key.
    template: NameKeyType,
    /// `m_status`.
    status: UpgradeStatusType,
}

impl Upgrade {
    /// `Upgrade(upgradeTemplate)`: status `UPGRADE_STATUS_INVALID`.
    pub fn new(template: NameKeyType) -> Self {
        Upgrade {
            template,
            status: UPGRADE_STATUS_INVALID,
        }
    }

    /// `getTemplate()`, as the template's key.
    pub fn template(&self) -> NameKeyType {
        self.template
    }

    /// `getStatus()`.
    pub fn status(&self) -> UpgradeStatusType {
        self.status
    }

    /// `setStatus(status)`.
    pub fn set_status(&mut self, status: UpgradeStatusType) {
        self.status = status;
    }
}

impl Snapshot for Upgrade {
    /// `crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer` (version 1): the status as four raw bytes (`xferUser`).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut b = self.status.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.status = UpgradeStatusType::from_le_bytes(b);
        Ok(())
    }

    /// `loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}

/// `enum UpgradeType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum UpgradeType {
    /// `UPGRADE_TYPE_PLAYER`: "upgrade applies to a player as a whole".
    #[default]
    Player = 0,
    /// `UPGRADE_TYPE_OBJECT`: "upgrade applies to an object instance only".
    Object = 1,
}

/// `TheUpgradeTypeNames[]`, without the `NULL` terminator.
pub const UPGRADE_TYPE_NAMES: [&str; 2] = ["PLAYER", "OBJECT"];

/// `class UpgradeTemplate`: a single upgrade template definition.
#[derive(Clone, Debug, PartialEq)]
pub struct UpgradeTemplate {
    /// `m_type`.
    type_: UpgradeType,
    /// `m_name`.
    name: Vec<u8>,
    /// `m_nameKey`.
    name_key: NameKeyType,
    /// `m_displayNameLabel`: "string manager label for UI display name".
    display_name_label: Vec<u8>,
    /// `m_buildTime`: seconds.
    build_time: f32,
    /// `m_cost`.
    cost: i32,
    /// `m_upgradeMask`: "unique bitmask for this upgrade template".
    upgrade_mask: UpgradeMaskType,
    /// `m_researchSound`.
    research_sound: AudioEventName,
    /// `m_unitSpecificSound`.
    unit_specific_sound: AudioEventName,
    /// `m_academyClassificationType`.
    academy_classification_type: AcademyClassificationType,
    /// `m_buttonImageName` (cut point: the client resolves it).
    button_image_name: Vec<u8>,
}

impl Default for UpgradeTemplate {
    /// `UpgradeTemplate()`: a player upgrade with no name, `NAMEKEY_INVALID`,
    /// no cost or build time and an empty mask.
    fn default() -> Self {
        UpgradeTemplate {
            type_: UpgradeType::Player,
            name: Vec::new(),
            name_key: NameKeyType::INVALID,
            display_name_label: Vec::new(),
            build_time: 0.0,
            cost: 0,
            upgrade_mask: UpgradeMaskType::new(),
            research_sound: AudioEventName::default(),
            unit_specific_sound: AudioEventName::default(),
            academy_classification_type: AcademyClassificationType::None,
            button_image_name: Vec::new(),
        }
    }
}

impl UpgradeTemplate {
    /// `UpgradeTemplate()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `getUpgradeName()`.
    pub fn upgrade_name(&self) -> &[u8] {
        &self.name
    }

    /// `getUpgradeNameKey()`.
    pub fn upgrade_name_key(&self) -> NameKeyType {
        self.name_key
    }

    /// `getDisplayNameLabel()`.
    pub fn display_name_label(&self) -> &[u8] {
        &self.display_name_label
    }

    /// `getUpgradeMask()`.
    pub fn upgrade_mask(&self) -> UpgradeMaskType {
        self.upgrade_mask
    }

    /// `getUpgradeType()`.
    pub fn upgrade_type(&self) -> UpgradeType {
        self.type_
    }

    /// `getResearchCompleteSound()`, as the event name.
    pub fn research_complete_sound(&self) -> &AudioEventName {
        &self.research_sound
    }

    /// `getUnitSpecificSound()`, as the event name.
    pub fn unit_specific_sound(&self) -> &AudioEventName {
        &self.unit_specific_sound
    }

    /// `getAcademyClassificationType()`.
    pub fn academy_classification_type(&self) -> AcademyClassificationType {
        self.academy_classification_type
    }

    /// `m_buttonImageName`: the image the client is to look up.
    pub fn button_image_name(&self) -> &[u8] {
        &self.button_image_name
    }

    /// `m_buildTime`, in seconds (private upstream, read by `calcTimeToBuild`).
    pub fn build_time(&self) -> f32 {
        self.build_time
    }

    /// `calcTimeToBuild(player)` in the shipped build, which ignores the
    /// player: `m_buildTime * LOGICFRAMES_PER_SECOND` in logic frames,
    /// converted to `Int` by truncation. See the module docs.
    pub fn time_to_build_frames(&self) -> Result<i32, IniError> {
        let frames = self.build_time * LOGICFRAMES_PER_SECOND as f32;
        if frames.is_nan() || !(-2147483648.0..2147483648.0).contains(&frames) {
            return Err(IniError::undefined(Undefined::FloatToIntOutOfRange));
        }
        Ok(frames as i32)
    }

    /// `calcCostToBuild(player)`, which ignores the player: `m_cost`.
    pub fn cost_to_build(&self) -> i32 {
        self.cost
    }

    /// `friend_makeVeterancyUpgrade(v)`: a free per-object upgrade named
    /// `Upgrade_Veterancy_<LEVEL>`. The mask is left alone.
    pub fn friend_make_veterancy_upgrade(
        &mut self,
        names: &mut NameKeyGenerator,
        v: VeterancyLevel,
    ) -> Result<(), NameKeyError> {
        self.type_ = UpgradeType::Object;
        self.name = vet_upgrade_name(v);
        self.name_key = names.try_name_to_key(&self.name)?;
        self.display_name_label.clear();
        self.build_time = 0.0;
        self.cost = 0;
        Ok(())
    }

    /// `parseUpgradeTemplate`'s field table is [`UPGRADE_FIELDS`]; the
    /// `Type` field.
    fn parse_type(
        ini: &mut Ini<'_>,
        this: &mut UpgradeTemplate,
        user_data: &UserData,
    ) -> IniResult<()> {
        let i = ini.parse_index_list(user_data.names())?;
        this.type_ = if i == 0 {
            UpgradeType::Player
        } else {
            UpgradeType::Object
        };
        Ok(())
    }

    /// The `AcademyClassify` field.
    fn parse_academy(
        ini: &mut Ini<'_>,
        this: &mut UpgradeTemplate,
        user_data: &UserData,
    ) -> IniResult<()> {
        let i = ini.parse_index_list(user_data.names())?;
        this.academy_classification_type =
            AcademyClassificationType::from_index(i).expect("index within the name list");
        Ok(())
    }
}

/// `getVetUpgradeName(v)`: `"Upgrade_Veterancy_" + TheVeterancyNames[v]`.
fn vet_upgrade_name(v: VeterancyLevel) -> Vec<u8> {
    let mut tmp = b"Upgrade_Veterancy_".to_vec();
    tmp.extend_from_slice(VETERANCY_NAMES[v as usize].as_bytes());
    tmp
}

/// `UpgradeTemplate::m_upgradeFieldParseTable`.
static UPGRADE_FIELDS: [FieldParse<UpgradeTemplate>; 9] = [
    FieldParse::new(
        "DisplayName",
        |ini, t, _| {
            t.display_name_label = ini.parse_ascii_string()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "Type",
        UpgradeTemplate::parse_type,
        UserData::Names(&UPGRADE_TYPE_NAMES),
    ),
    FieldParse::new(
        "BuildTime",
        |ini, t, _| {
            t.build_time = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "BuildCost",
        |ini, t, _| {
            t.cost = ini.parse_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ButtonImage",
        |ini, t, _| {
            t.button_image_name = ini.parse_ascii_string()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ResearchSound",
        |ini, t, _| t.research_sound.parse(ini),
        UserData::None,
    ),
    FieldParse::new(
        "UnitSpecificSound",
        |ini, t, _| t.unit_specific_sound.parse(ini),
        UserData::None,
    ),
    FieldParse::new(
        "AcademyClassify",
        UpgradeTemplate::parse_academy,
        UserData::Names(&ACADEMY_CLASSIFICATION_TYPE_NAMES),
    ),
    FieldParse::end(),
];

/// What can go wrong making an upgrade template: both are `throw`s or
/// undefined behaviour upstream (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeCenterError {
    /// `NameKeyGenerator` ran out of keys.
    NameKey(NameKeyError),
    /// The 129th template: `std::bitset::set(128)` throws.
    MaskBit(BitIndexOutOfRange),
}

impl std::fmt::Display for UpgradeCenterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpgradeCenterError::NameKey(e) => e.fmt(f),
            UpgradeCenterError::MaskBit(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for UpgradeCenterError {}

impl From<NameKeyError> for UpgradeCenterError {
    fn from(e: NameKeyError) -> Self {
        UpgradeCenterError::NameKey(e)
    }
}

impl From<UpgradeCenterError> for IniError {
    /// How the INI block reports it: a name-key overflow is undefined
    /// upstream, the mask overflow is the `std::out_of_range` it throws.
    fn from(e: UpgradeCenterError) -> Self {
        match e {
            UpgradeCenterError::NameKey(_) => IniError::undefined(Undefined::NameKeysExhausted),
            UpgradeCenterError::MaskBit(_) => IniError::new(IniErrorKind::BitIndexOutOfRange),
        }
    }
}

/// `class UpgradeCenter` (`TheUpgradeCenter`): "keeps some basic information
/// about the possible upgrades".
#[derive(Clone, Debug, Default)]
pub struct UpgradeCenter {
    /// `m_upgradeList`, oldest first (upstream's head is the last element).
    upgrade_list: Vec<UpgradeTemplate>,
    /// `m_nextTemplateMaskBit`.
    next_template_mask_bit: i32,
}

impl UpgradeCenter {
    /// `UpgradeCenter()`: no templates, the next mask bit is 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// `init()`: the three veterancy templates (there is "no upgrade" for
    /// `LEVEL_REGULAR`), each made by `newUpgrade("")` and then renamed.
    pub fn init(&mut self, names: &mut NameKeyGenerator) -> Result<(), UpgradeCenterError> {
        for level in [
            VeterancyLevel::Veteran,
            VeterancyLevel::Elite,
            VeterancyLevel::Heroic,
        ] {
            let i = self.new_upgrade(names, b"")?;
            self.upgrade_list[i].friend_make_veterancy_upgrade(names, level)?;
        }
        Ok(())
    }

    /// The number of templates.
    pub fn len(&self) -> usize {
        self.upgrade_list.len()
    }

    /// Whether there are no templates.
    pub fn is_empty(&self) -> bool {
        self.upgrade_list.is_empty()
    }

    /// `m_nextTemplateMaskBit`.
    pub fn next_template_mask_bit(&self) -> i32 {
        self.next_template_mask_bit
    }

    /// The templates in list order: the newest first, as upstream walks
    /// `firstUpgradeTemplate()` and `friend_getNext()`.
    pub fn templates(&self) -> impl Iterator<Item = &UpgradeTemplate> {
        self.upgrade_list.iter().rev()
    }

    /// `firstUpgradeTemplate()`: the newest template.
    pub fn first_upgrade_template(&self) -> Option<&UpgradeTemplate> {
        self.upgrade_list.last()
    }

    /// `findUpgradeByKey(key)`: the first template with the key.
    pub fn find_upgrade_by_key(&self, key: NameKeyType) -> Option<&UpgradeTemplate> {
        self.templates().find(|t| t.upgrade_name_key() == key)
    }

    /// `findNonConstUpgradeByKey(key)`, as an index into `upgrade_list`.
    fn find_index_by_key(&self, key: NameKeyType) -> Option<usize> {
        self.upgrade_list
            .iter()
            .rposition(|t| t.upgrade_name_key() == key)
    }

    /// `findUpgrade(name)`: `findUpgradeByKey(nameToKey(name))` (which assigns
    /// a key to a new name).
    pub fn find_upgrade(
        &self,
        names: &mut NameKeyGenerator,
        name: &[u8],
    ) -> Result<Option<&UpgradeTemplate>, NameKeyError> {
        let key = names.try_name_to_key(name)?;
        Ok(self.find_upgrade_by_key(key))
    }

    /// `findVeterancyUpgrade(level)`: the template named
    /// `Upgrade_Veterancy_<LEVEL>` (there is none for `LEVEL_REGULAR`).
    pub fn find_veterancy_upgrade(
        &self,
        names: &mut NameKeyGenerator,
        level: VeterancyLevel,
    ) -> Result<Option<&UpgradeTemplate>, NameKeyError> {
        self.find_upgrade(names, &vet_upgrade_name(level))
    }

    /// `newUpgrade(name)`: a template copied from `DefaultUpgrade` (looked up
    /// first: its name gets a key), named, given the next mask bit and
    /// linked at the head. Returns its index in the list order of storage
    /// (the last one).
    pub fn new_upgrade(
        &mut self,
        names: &mut NameKeyGenerator,
        name: &[u8],
    ) -> Result<usize, UpgradeCenterError> {
        // copy data from the default upgrade
        let mut new_upgrade = match self.find_upgrade(names, b"DefaultUpgrade")? {
            Some(default) => default.clone(),
            None => UpgradeTemplate::new(),
        };

        // assign name and starting data
        new_upgrade.name = name.to_vec();
        new_upgrade.name_key = names.try_name_to_key(name)?;

        // a unique bitmask for this template (the 129th throws)
        let mut new_mask = UpgradeMaskType::new();
        new_mask
            .try_set(self.next_template_mask_bit, 1)
            .map_err(UpgradeCenterError::MaskBit)?;
        self.next_template_mask_bit += 1;
        new_upgrade.upgrade_mask = new_mask;

        // link upgrade
        self.upgrade_list.push(new_upgrade);
        Ok(self.upgrade_list.len() - 1)
    }

    /// `getUpgradeNames()` ("for WorldBuilder only"): every template's name,
    /// newest first.
    pub fn get_upgrade_names(&self) -> Vec<Vec<u8>> {
        self.templates()
            .map(|t| t.upgrade_name().to_vec())
            .collect()
    }

    /// `UpgradeCenter::parseUpgradeDefinition(ini)` (reached through
    /// `INI::parseUpgradeDefinition`, the `Upgrade` block): the template's
    /// name, then its fields until `End`. A known name (by name key) is
    /// parsed over; a new one is made by [`UpgradeCenter::new_upgrade`]. A
    /// template stays as far as it was filled if a line fails.
    pub fn parse_upgrade_definition(
        &mut self,
        ini: &mut Ini<'_>,
        names: &mut NameKeyGenerator,
    ) -> IniResult<()> {
        // read the name
        let name = ini.get_next_token(None)?;

        // find existing item if present
        let key = names
            .try_name_to_key(&name)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        let index = match self.find_index_by_key(key) {
            Some(i) => i,
            // allocate a new item
            None => self.new_upgrade(names, &name)?,
        };

        // parse the ini definition
        ini.init_from_ini(&mut self.upgrade_list[index], &UPGRADE_FIELDS)
    }
}

/// What `Xfer::xferUpgradeMask` can fail with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XferUpgradeMaskError {
    /// The xfer failed, or a loaded name is no template (`XFER_UNKNOWN_STRING`).
    Xfer(XferStatus),
    /// `findUpgrade` ran `NameKeyGenerator` out of keys (undefined upstream).
    NameKeysExhausted,
}

impl From<XferStatus> for XferUpgradeMaskError {
    fn from(s: XferStatus) -> Self {
        XferUpgradeMaskError::Xfer(s)
    }
}

impl std::fmt::Display for XferUpgradeMaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XferUpgradeMaskError::Xfer(s) => write!(f, "xferUpgradeMask: {s:?}"),
            XferUpgradeMaskError::NameKeysExhausted => {
                f.write_str("xferUpgradeMask: name keys exhausted (undefined upstream)")
            }
        }
    }
}

impl std::error::Error for XferUpgradeMaskError {}

/// `Xfer::xferUpgradeMask(upgradeMaskData)` (version 1). The mask is saved as
/// the **names** of its upgrades, so that adding an upgrade to the INI (which
/// shifts the bits) does not break old files.
///
/// - **Save:** a `UnsignedShort` count of the templates whose whole mask is
///   set in `mask`, then those templates' names, in list order (newest
///   first).
/// - **Load:** the count, `mask` cleared, then each name read is looked up
///   (`findUpgrade`, which assigns a key to a new name) and its bit set;
///   a name that is no template is `XFER_UNKNOWN_STRING`.
/// - **CRC:** the 16 bytes of `sizeof(UpgradeMaskType)` (the shipped Win32
///   `std::bitset<128>`: four little-endian 32-bit words).
pub fn xfer_upgrade_mask(
    xfer: &mut dyn Xfer,
    mask: &mut UpgradeMaskType,
    center: &UpgradeCenter,
    names: &mut NameKeyGenerator,
) -> Result<(), XferUpgradeMaskError> {
    // this deserves a version number
    let current: XferVersion = 1;
    let mut version = current;
    xfer.xfer_version(&mut version, current)?;

    match xfer.get_xfer_mode() {
        XferMode::Save => {
            // count how many bits are set in the mask
            let mut count: u16 = 0;
            for t in center.templates() {
                if mask.test_for_all(&t.upgrade_mask()) {
                    count += 1;
                }
            }
            xfer.xfer_unsigned_short(&mut count)?;
            // write out the upgrades as strings
            for t in center.templates() {
                if mask.test_for_all(&t.upgrade_mask()) {
                    let mut name = t.upgrade_name().to_vec();
                    xfer.xfer_ascii_string(&mut name)?;
                }
            }
            Ok(())
        }
        XferMode::Load => {
            let mut count: u16 = 0;
            xfer.xfer_unsigned_short(&mut count)?;
            // zero the mask data
            mask.clear();
            let mut name = Vec::new();
            for _ in 0..count {
                xfer.xfer_ascii_string(&mut name)?;
                let template = center
                    .find_upgrade(names, &name)
                    .map_err(|_| XferUpgradeMaskError::NameKeysExhausted)?;
                match template {
                    Some(t) => mask.set_mask(&t.upgrade_mask()),
                    None => return Err(XferStatus::UnknownString.into()),
                }
            }
            Ok(())
        }
        XferMode::Crc => {
            let w = mask.words();
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&w[0].to_le_bytes());
            bytes[8..].copy_from_slice(&w[1].to_le_bytes());
            xfer.xfer_implementation(&mut bytes)?;
            Ok(())
        }
        XferMode::Invalid => Err(XferStatus::ModeUnknown.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::common::xfer_crc::XferCrc;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;

    struct Ctx {
        center: UpgradeCenter,
        names: NameKeyGenerator,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.center.parse_upgrade_definition(ini, &mut c.names)
    }

    static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse {
        token: "Upgrade",
        parse: block,
    }];

    fn ctx(init: bool) -> Ctx {
        let mut c = Ctx {
            center: UpgradeCenter::new(),
            names: NameKeyGenerator::new(),
        };
        c.names.init();
        if init {
            c.center.init(&mut c.names).unwrap();
        }
        c
    }

    fn load(init: bool, text: &[u8]) -> (Ctx, IniResult<()>) {
        let mut c = ctx(init);
        let r = Ini::load(
            b"u.ini",
            text,
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut c,
        );
        (c, r)
    }

    #[test]
    fn init_makes_the_three_veterancy_templates_newest_first() {
        let c = ctx(true);
        assert_eq!(c.center.len(), 3);
        assert_eq!(c.center.next_template_mask_bit(), 3);
        let names = c.center.get_upgrade_names();
        assert_eq!(
            names,
            vec![
                b"Upgrade_Veterancy_HEROIC".to_vec(),
                b"Upgrade_Veterancy_ELITE".to_vec(),
                b"Upgrade_Veterancy_VETERAN".to_vec()
            ]
        );
        let first = c.center.first_upgrade_template().unwrap();
        assert_eq!(first.upgrade_name(), b"Upgrade_Veterancy_HEROIC");
        assert_eq!(first.upgrade_type(), UpgradeType::Object);
        assert_eq!(first.upgrade_mask(), make_upgrade_mask(&[2]).unwrap());
        assert_eq!(first.time_to_build_frames(), Ok(0));
        assert_eq!(first.cost_to_build(), 0);
    }

    #[test]
    fn fields_default_template_and_redefinition() {
        let (mut c, r) = load(
            true,
            b"Upgrade DefaultUpgrade\n Type = OBJECT\n BuildCost = 7\n ResearchSound = Chime\n AcademyClassify = ACT_SUPERPOWER\nEnd\n\
              Upgrade Radar\n DisplayName = UPGRADE:Radar\n BuildTime = 2.5\n ButtonImage = SSRadar\n UnitSpecificSound = NoSound\nEnd\n\
              Upgrade Radar\n BuildCost = 99\nEnd\n",
        );
        r.unwrap();
        assert_eq!(c.center.len(), 5);
        assert_eq!(c.center.next_template_mask_bit(), 5);
        let radar = c
            .center
            .find_upgrade(&mut c.names, b"Radar")
            .unwrap()
            .unwrap()
            .clone();
        // copied from DefaultUpgrade, then overridden field by field
        assert_eq!(radar.upgrade_type(), UpgradeType::Object);
        assert_eq!(radar.cost_to_build(), 99);
        assert_eq!(radar.research_complete_sound().event_name, b"Chime");
        assert_eq!(radar.unit_specific_sound().event_name, b"");
        assert_eq!(
            radar.academy_classification_type(),
            AcademyClassificationType::Superpower
        );
        assert_eq!(radar.display_name_label(), b"UPGRADE:Radar");
        assert_eq!(radar.button_image_name(), b"SSRadar");
        assert_eq!(radar.time_to_build_frames(), Ok(75));
        // its own bit, not the default's
        assert_eq!(radar.upgrade_mask(), make_upgrade_mask(&[4]).unwrap());
        assert_eq!(c.center.find_upgrade(&mut c.names, b"Nope").unwrap(), None);
        assert!(c
            .center
            .find_veterancy_upgrade(&mut c.names, VeterancyLevel::Elite)
            .unwrap()
            .is_some());
        assert!(c
            .center
            .find_veterancy_upgrade(&mut c.names, VeterancyLevel::Regular)
            .unwrap()
            .is_none());
    }

    #[test]
    fn the_129th_template_throws_out_of_range() {
        let mut text = Vec::new();
        for i in 0..130 {
            text.extend_from_slice(format!("Upgrade U{i}\nEnd\n").as_bytes());
        }
        let (c, r) = load(true, &text);
        let e = r.unwrap_err();
        assert_eq!(e.kind, IniErrorKind::BitIndexOutOfRange);
        assert_eq!(c.center.len(), 128);
        assert_eq!(c.center.next_template_mask_bit(), 128);
        assert_eq!(
            c.center.first_upgrade_template().unwrap().upgrade_name(),
            b"U124"
        );
    }

    #[test]
    fn errors_leave_the_partial_template() {
        let (c, r) = load(false, b"Upgrade A\n BuildCost = 5\n Type = SIDEWAYS\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(c.center.len(), 1);
        assert_eq!(
            c.center.first_upgrade_template().unwrap().cost_to_build(),
            5
        );
        let (_, r) = load(false, b"Upgrade A\n Foo = 1\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::UnknownToken);
        let (_, r) = load(false, b"Upgrade\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
    }

    #[test]
    fn build_time_conversion_truncates_and_rejects_out_of_range() {
        let mut t = UpgradeTemplate::new();
        t.build_time = 1.99;
        assert_eq!(t.time_to_build_frames(), Ok(59));
        t.build_time = -1.99;
        assert_eq!(t.time_to_build_frames(), Ok(-59));
        t.build_time = 1.0e30;
        assert!(t.time_to_build_frames().unwrap_err().is_undefined());
        t.build_time = f32::NAN;
        assert!(t.time_to_build_frames().unwrap_err().is_undefined());
        assert!(t.build_time().is_nan());
    }

    #[test]
    fn mask_helpers() {
        let a = make_upgrade_mask(&[1, 5]).unwrap();
        let b = make_upgrade_mask(&[5, 100]).unwrap();
        assert_eq!(test_upgrade_mask(&a, 5), Ok(true));
        assert_eq!(test_upgrade_mask(&a, 128), Err(BitIndexOutOfRange));
        assert!(test_upgrade_mask_any(&a, &b));
        assert!(!test_upgrade_mask_multi(&a, &b, &UpgradeMaskType::new()));
        assert!(test_upgrade_mask_multi(
            &a,
            &make_upgrade_mask(&[1]).unwrap(),
            &make_upgrade_mask(&[2]).unwrap()
        ));
        assert!(upgrade_mask_any_set(&a));
        let mut m = a;
        clear_upgrade_mask(&mut m);
        assert!(!upgrade_mask_any_set(&m));
        set_all_upgrade_mask_bits(&mut m);
        assert_eq!(m.count(), 128);
        flip_upgrade_mask(&mut m);
        assert!(!upgrade_mask_any_set(&m));
        assert_eq!(make_upgrade_mask(&[128]), Err(BitIndexOutOfRange));
    }

    #[test]
    fn upgrade_instance_xfer_round_trips() {
        let mut u = Upgrade::new(NameKeyType(7));
        assert_eq!(u.status(), UPGRADE_STATUS_INVALID);
        assert_eq!(u.template(), NameKeyType(7));
        u.set_status(UPGRADE_STATUS_COMPLETE);
        let mut save = XferSave::new();
        save.open(b"t").unwrap();
        u.xfer(&mut save).unwrap();
        save.close().unwrap();
        let bytes = save.bytes().to_vec();
        assert_eq!(bytes, vec![1, 2, 0, 0, 0]);
        let mut v = Upgrade::new(NameKeyType(7));
        let mut load = XferLoad::new(bytes);
        load.open(b"t").unwrap();
        v.xfer(&mut load).unwrap();
        assert_eq!(v, u);
        // crc() and loadPostProcess() do nothing
        let mut crc = XferCrc::new_without_nan_guard();
        crc.open(b"t").unwrap();
        u.crc(&mut crc).unwrap();
        u.load_post_process();
        assert_eq!(crc.get_crc(), 0);
    }

    #[test]
    fn upgrade_mask_xfer_saves_names_and_loads_bits() {
        let (mut c, r) = load(true, b"Upgrade A\nEnd\nUpgrade B\nEnd\nUpgrade C\nEnd\n");
        r.unwrap();
        // A is bit 3, B bit 4, C bit 5; the veterancy ones are VETERAN 0, ELITE 1, HEROIC 2
        let mut mask = make_upgrade_mask(&[3, 5, 1]).unwrap();
        let mut save = XferSave::new();
        save.open(b"t").unwrap();
        xfer_upgrade_mask(&mut save, &mut mask, &c.center, &mut c.names).unwrap();
        save.close().unwrap();
        let bytes = save.bytes().to_vec();
        // version 1, count 3, then "C", "A", "Upgrade_Veterancy_ELITE" (newest first), each a length byte and text
        let mut want = vec![1u8, 3, 0];
        for n in [&b"C"[..], b"A", b"Upgrade_Veterancy_ELITE"] {
            want.push(n.len() as u8);
            want.extend_from_slice(n);
        }
        assert_eq!(bytes, want);
        let mut back = make_upgrade_mask(&[9]).unwrap();
        let mut load = XferLoad::new(bytes.clone());
        load.open(b"t").unwrap();
        xfer_upgrade_mask(&mut load, &mut back, &c.center, &mut c.names).unwrap();
        assert_eq!(back, make_upgrade_mask(&[1, 3, 5]).unwrap());
        // an unknown name
        let mut bad = vec![1u8, 1, 0, 3];
        bad.extend_from_slice(b"Zed");
        let mut load = XferLoad::new(bad);
        load.open(b"t").unwrap();
        let mut m = UpgradeMaskType::new();
        assert_eq!(
            xfer_upgrade_mask(&mut load, &mut m, &c.center, &mut c.names),
            Err(XferStatus::UnknownString.into())
        );
        // CRC hashes the 16 bytes of the mask
        let mut crc = XferCrc::new_without_nan_guard();
        crc.open(b"t").unwrap();
        xfer_upgrade_mask(&mut crc, &mut mask, &c.center, &mut c.names).unwrap();
        let mut crc2 = XferCrc::new_without_nan_guard();
        crc2.open(b"t").unwrap();
        let mut v: XferVersion = 1;
        crc2.xfer_version(&mut v, 1).unwrap();
        let w = mask.words();
        let mut raw = [0u8; 16];
        raw[..8].copy_from_slice(&w[0].to_le_bytes());
        raw[8..].copy_from_slice(&w[1].to_le_bytes());
        crc2.xfer_implementation(&mut raw).unwrap();
        assert_eq!(crc.get_crc(), crc2.get_crc());
    }
}
