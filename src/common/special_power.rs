// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/SpecialPower.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/SpecialPower.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INISpecialPower.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp (lines 1378-1390:
//   `INI::parseScience`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, April 2002; Kris Morness, July 2002 (BitFlags)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpecialPowerTemplate` (the reload time, required science, timers and
//! sounds of a special power), `SpecialPowerStore` (every template read from
//! INI, in order, with its override chain), the `SpecialPower` INI block, and
//! the `SpecialPowerMaskType` helper functions of `SpecialPower.h`.
//!
//! # Translation choices
//!
//! - **Overrides.** A template is an `Overridable`
//!   ([`super::overridable`]): a later INI file loaded with
//!   `INI_LOAD_CREATE_OVERRIDES` adds a copy of the final definition to the
//!   end of its chain and parses over that, and every getter reads the
//!   **final override**, so all users see the newest values. The store's
//!   `reset` removes the overrides again. Here each template is an
//!   [`Overridable<SpecialPowerTemplateData>`], and a read-only view of one
//!   is [`SpecialPowerTemplateRef`].
//! - **The store** is the `std::vector` of the head of each chain, in the
//!   order the powers were first defined; ids count up from 1 and are never
//!   reused (a removed override-created template leaves a gap).
//! - **A new template** starts as a copy of the template named
//!   `DefaultSpecialPower` if there is one, whose id and name are then
//!   replaced.
//! - **`getAcademyClassificationType`** is the one getter that reads the
//!   template itself and not its final override (upstream does not use
//!   `getFO()` there); [`SpecialPowerTemplateRef::academy_classification_type`]
//!   keeps that.
//! - **The default of `m_academyClassificationType`**: the constructor does
//!   not set it. The engine's memory pools zero every block, so it is
//!   `ACT_NONE` (0), which is what the Rust starts with.
//!
//! # Cut points
//!
//! - **Sounds are names** ([`AudioEventName`]): `InitiateSound` and
//!   `InitiateAtLocationSound` keep the name as written; the client resolves
//!   it.
//! - **`RequiredScience`** goes through `INI::parseScience` /
//!   `ScienceStore::friend_lookupScience`, which turns the name into a
//!   `ScienceType` (its name key) and throws `INI_INVALID_DATA` for a name that
//!   is not a defined science. The science store is not here, so
//!   [`SpecialPowerStore::parse_special_power_definition`] takes a function
//!   saying which keys are sciences. `friend_lookupScience` calls `NAMEKEY`,
//!   which **assigns a key to a name it has not seen**, so the
//!   [`NameKeyGenerator`] is taken too.
//!
//! # Left out: methods that take an `Object` or a `Player`
//!
//! `SpecialPowerStore::canUseSpecialPower(Object*, template)` (the object's
//! module must be able to run the power, and the controlling player must have
//! the required science) moves to layer 7/8, with the special power modules
//! (`Object::getSpecialPowerModule`, `Player::hasScience`).
//!
//! # Where upstream is undefined or cannot be represented
//!
//! - [`Undefined::AliasedOverrideChain`]: a template made from
//!   `DefaultSpecialPower` copies the pointer to the default's next override
//!   (`*specialPower = *defaultTemplate` copies `Overridable` too), so the
//!   two templates share a node, which `reset` then deletes from the default
//!   while the copy still points to it (a dangling pointer). Owned chains
//!   cannot share a node, so a default that has an override chain is an error.
//! - [`Undefined::NameKeysExhausted`]: `NameKeyGenerator` past its last key.
//!
//! # Not translated here
//!
//! `INI::parseSpecialPowerTemplate` (the `SpecialPowerTemplate =` field of
//! modules and command buttons) belongs with its users; it is a name lookup
//! in the store ([`SpecialPowerStore::find_special_power_template`]).

use super::academy_classification::{AcademyClassificationType, ACADEMY_CLASSIFICATION_TYPE_NAMES};
use super::audio_event_name::AudioEventName;
use super::bit_flags::BitIndexOutOfRange;
use super::game_common::LOGICFRAMES_PER_SECOND;
use super::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Undefined, UserData,
};
use super::name_key_generator::{NameKeyGenerator, NameKeyType};
use super::overridable::Overridable;
use super::special_power_mask_type::{SpecialPowerMaskType, SPECIAL_POWER_NAMES};
use super::special_power_type::{SpecialPowerType, SPECIAL_INVALID};

// `enum ScienceType` and `SCIENCE_INVALID` (`Science.h`), from
// `common::science` (this module had a local stand-in before the two
// wave-2 branches were merged).
pub use super::science::{ScienceType, SCIENCE_INVALID};

/// `DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT`: `LOGICFRAMES_PER_SECOND * 10`.
pub const DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT: u32 =
    (LOGICFRAMES_PER_SECOND * 10) as u32;

/// `MAKE_SPECIALPOWER_MASK(k)`.
pub fn make_special_power_mask(
    k: SpecialPowerType,
) -> Result<SpecialPowerMaskType, BitIndexOutOfRange> {
    SpecialPowerMaskType::try_from_indices(&[k])
}

/// `TEST_SPECIALPOWERMASK(m, t)`.
pub fn test_special_power_mask(
    m: &SpecialPowerMaskType,
    t: SpecialPowerType,
) -> Result<bool, BitIndexOutOfRange> {
    m.try_test(t)
}

/// `TEST_SPECIALPOWERMASK_ANY(m, mask)`.
pub fn test_special_power_mask_any(m: &SpecialPowerMaskType, mask: &SpecialPowerMaskType) -> bool {
    m.any_intersection_with(mask)
}

/// `TEST_SPECIALPOWERMASK_MULTI(m, mustBeSet, mustBeClear)`.
pub fn test_special_power_mask_multi(
    m: &SpecialPowerMaskType,
    must_be_set: &SpecialPowerMaskType,
    must_be_clear: &SpecialPowerMaskType,
) -> bool {
    m.test_set_and_clear(must_be_set, must_be_clear)
}

/// `SPECIALPOWERMASK_ANY_SET(m)`.
pub fn special_power_mask_any_set(m: &SpecialPowerMaskType) -> bool {
    m.any()
}

/// `CLEAR_SPECIALPOWERMASK(m)`.
pub fn clear_special_power_mask(m: &mut SpecialPowerMaskType) {
    m.clear();
}

/// `SET_SPECIALPOWERMASK(m, t, val)` (`val` is 1 upstream's default).
pub fn set_special_power_mask(
    m: &mut SpecialPowerMaskType,
    t: SpecialPowerType,
    val: i32,
) -> Result<(), BitIndexOutOfRange> {
    m.try_set(t, val)
}

/// `SET_ALL_SPECIALPOWERMASK_BITS(m)`: clear, then flip.
pub fn set_all_special_power_mask_bits(m: &mut SpecialPowerMaskType) {
    m.clear();
    m.flip();
}

/// `FLIP_SPECIALPOWERMASK(m)`.
pub fn flip_special_power_mask(m: &mut SpecialPowerMaskType) {
    m.flip();
}

/// The fields of `class SpecialPowerTemplate` (everything but the
/// `Overridable` chain, which [`Overridable`] holds).
#[derive(Clone, Debug, PartialEq)]
pub struct SpecialPowerTemplateData {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_id`: "unique identifier".
    pub id: u32,
    /// `m_type`: "enum allowing for fast type checking for ability processing".
    pub special_power_type: SpecialPowerType,
    /// `m_reloadTime`: frames before the power can be used again.
    pub reload_time: u32,
    /// `m_requiredScience`: "science required (if any) to actually execute
    /// this power".
    pub required_science: ScienceType,
    /// `m_initiateSound`: "sound to play when initiated".
    pub initiate_sound: AudioEventName,
    /// `m_initiateAtLocationSound`: "sound to play at target location (if any)".
    pub initiate_at_location_sound: AudioEventName,
    /// `m_academyClassificationType`.
    pub academy_classification_type: AcademyClassificationType,
    /// `m_detectionTime`: frames before ex-comrades notice an infiltration.
    pub detection_time: u32,
    /// `m_viewObjectDuration`: "lifetime of a looking object we slap down so
    /// you can watch the effect".
    pub view_object_duration: u32,
    /// `m_viewObjectRange`.
    pub view_object_range: f32,
    /// `m_radiusCursorRadius`: "size of radius cursor, if any".
    pub radius_cursor_radius: f32,
    /// `m_publicTimer`: "display a countdown timer for this special power for
    /// all to see".
    pub public_timer: bool,
    /// `m_sharedNSync`: "shared between all of a player's command centers".
    pub shared_n_sync: bool,
    /// `m_shortcutPower`: "capable of being fired by the side panel".
    pub shortcut_power: bool,
}

impl Default for SpecialPowerTemplateData {
    /// `SpecialPowerTemplate()`.
    fn default() -> Self {
        SpecialPowerTemplateData {
            name: Vec::new(),
            id: 0,
            special_power_type: SPECIAL_INVALID,
            reload_time: 0,
            required_science: SCIENCE_INVALID,
            initiate_sound: AudioEventName::default(),
            initiate_at_location_sound: AudioEventName::default(),
            academy_classification_type: AcademyClassificationType::None,
            detection_time: DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT,
            view_object_duration: 0,
            view_object_range: 0.0,
            radius_cursor_radius: 0.0,
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
        }
    }
}

/// A `const SpecialPowerTemplate*`: the getters of `SpecialPowerTemplate`,
/// reading the final override of the chain.
#[derive(Clone, Copy, Debug)]
pub struct SpecialPowerTemplateRef<'a> {
    head: &'a Overridable<SpecialPowerTemplateData>,
}

impl<'a> SpecialPowerTemplateRef<'a> {
    /// `getFO()`: the data of the final override.
    fn fo(&self) -> &'a SpecialPowerTemplateData {
        &self.head.get_final_override().data
    }

    /// `getName()`.
    pub fn name(&self) -> &'a [u8] {
        &self.fo().name
    }
    /// `getID()`.
    pub fn id(&self) -> u32 {
        self.fo().id
    }
    /// `getSpecialPowerType()`.
    pub fn special_power_type(&self) -> SpecialPowerType {
        self.fo().special_power_type
    }
    /// `getReloadTime()`.
    pub fn reload_time(&self) -> u32 {
        self.fo().reload_time
    }
    /// `getRequiredScience()`.
    pub fn required_science(&self) -> ScienceType {
        self.fo().required_science
    }
    /// `getInitiateSound()`, as the event name.
    pub fn initiate_sound(&self) -> &'a AudioEventName {
        &self.fo().initiate_sound
    }
    /// `getInitiateAtTargetSound()`, as the event name.
    pub fn initiate_at_target_sound(&self) -> &'a AudioEventName {
        &self.fo().initiate_at_location_sound
    }
    /// `hasPublicTimer()`.
    pub fn has_public_timer(&self) -> bool {
        self.fo().public_timer
    }
    /// `isSharedNSync()`.
    pub fn is_shared_n_sync(&self) -> bool {
        self.fo().shared_n_sync
    }
    /// `getDetectionTime()`.
    pub fn detection_time(&self) -> u32 {
        self.fo().detection_time
    }
    /// `getViewObjectDuration()`.
    pub fn view_object_duration(&self) -> u32 {
        self.fo().view_object_duration
    }
    /// `getViewObjectRange()`.
    pub fn view_object_range(&self) -> f32 {
        self.fo().view_object_range
    }
    /// `getRadiusCursorRadius()`.
    pub fn radius_cursor_radius(&self) -> f32 {
        self.fo().radius_cursor_radius
    }
    /// `isShortcutPower()`.
    pub fn is_shortcut_power(&self) -> bool {
        self.fo().shortcut_power
    }
    /// `getAcademyClassificationType()`: read from this template itself, not
    /// its final override (see the module docs).
    pub fn academy_classification_type(&self) -> AcademyClassificationType {
        self.head.data.academy_classification_type
    }
    /// The chain this template heads (for inspection).
    pub fn chain(&self) -> &'a Overridable<SpecialPowerTemplateData> {
        self.head
    }
}

/// `class SpecialPowerStore` (`TheSpecialPowerStore`).
#[derive(Clone, Debug, Default)]
pub struct SpecialPowerStore {
    /// `m_specialPowerTemplates`: the head of each chain (boxed because
    /// `Overridable::delete_overrides` consumes a `Box`, as `reset` needs).
    #[allow(clippy::vec_box)]
    special_power_templates: Vec<Box<Overridable<SpecialPowerTemplateData>>>,
    /// `m_nextSpecialPowerID`.
    next_special_power_id: u32,
}

/// The state a `SpecialPower` block's field parsers work on.
struct FieldCtx<'a> {
    data: &'a mut SpecialPowerTemplateData,
    names: &'a mut NameKeyGenerator,
    is_valid_science: &'a dyn Fn(NameKeyType) -> bool,
}

impl FieldCtx<'_> {
    /// `INI::parseScience`: the next token through `scanScience`, i.e.
    /// `friend_lookupScience`: its name key if that is a defined science, else
    /// `INI_INVALID_DATA`.
    fn parse_science(ini: &mut Ini<'_>, c: &mut FieldCtx<'_>, _: &UserData) -> IniResult<()> {
        let token = ini.get_next_token(None)?;
        let key = c
            .names
            .try_name_to_key(&token)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        if !(c.is_valid_science)(key) {
            return Err(IniError::new(IniErrorKind::InvalidData));
        }
        c.data.required_science = ScienceType(key.0 as i32);
        Ok(())
    }

    fn parse_enum(ini: &mut Ini<'_>, c: &mut FieldCtx<'_>, user_data: &UserData) -> IniResult<()> {
        c.data.special_power_type = ini.parse_index_list(user_data.names())?;
        Ok(())
    }

    fn parse_academy(
        ini: &mut Ini<'_>,
        c: &mut FieldCtx<'_>,
        user_data: &UserData,
    ) -> IniResult<()> {
        let i = ini.parse_index_list(user_data.names())?;
        c.data.academy_classification_type =
            AcademyClassificationType::from_index(i).expect("index within the name list");
        Ok(())
    }
}

impl SpecialPowerStore {
    /// `SpecialPowerStore()`: no templates, the next id is 1.
    pub fn new() -> Self {
        Self::default()
    }

    /// `getNumSpecialPowers()` (for WorldBuilder): the number of templates.
    pub fn len(&self) -> usize {
        self.special_power_templates.len()
    }

    /// Whether there are no templates.
    pub fn is_empty(&self) -> bool {
        self.special_power_templates.is_empty()
    }

    /// `m_nextSpecialPowerID`: the id of the last template made.
    pub fn next_special_power_id(&self) -> u32 {
        self.next_special_power_id
    }

    /// The templates in store order.
    pub fn templates(&self) -> impl Iterator<Item = SpecialPowerTemplateRef<'_>> {
        self.special_power_templates
            .iter()
            .map(|b| SpecialPowerTemplateRef { head: b })
    }

    fn find_private_index(&self, name: &[u8]) -> Option<usize> {
        self.special_power_templates
            .iter()
            .position(|t| t.get_final_override().data.name == name)
    }

    /// `findSpecialPowerTemplate(name)`: the first template whose (final
    /// override's) name is `name`, which is compared exactly (case counts).
    pub fn find_special_power_template(&self, name: &[u8]) -> Option<SpecialPowerTemplateRef<'_>> {
        self.find_private_index(name)
            .map(|i| SpecialPowerTemplateRef {
                head: &self.special_power_templates[i],
            })
    }

    /// `findSpecialPowerTemplateByID(id)`.
    pub fn find_special_power_template_by_id(
        &self,
        id: u32,
    ) -> Option<SpecialPowerTemplateRef<'_>> {
        self.templates().find(|t| t.id() == id)
    }

    /// `getSpecialPowerTemplateByIndex(index)` (for WorldBuilder).
    pub fn get_special_power_template_by_index(
        &self,
        index: u32,
    ) -> Option<SpecialPowerTemplateRef<'_>> {
        self.special_power_templates
            .get(index as usize)
            .map(|b| SpecialPowerTemplateRef { head: b })
    }

    /// `reset()`: delete the overrides, and the templates that were made by an
    /// override load. Ids are not reset.
    pub fn reset(&mut self) {
        let templates = std::mem::take(&mut self.special_power_templates);
        self.special_power_templates = templates
            .into_iter()
            .filter_map(|t| t.delete_overrides())
            .collect();
    }

    /// A new template as `parseSpecialPowerDefinition` makes it: a copy of
    /// `DefaultSpecialPower` (all of `Overridable` too, so a default that is
    /// an override makes an override), named and numbered.
    fn new_from_default(
        &mut self,
        name: &[u8],
    ) -> Result<Box<Overridable<SpecialPowerTemplateData>>, IniError> {
        let mut new = match self.find_special_power_template(b"DefaultSpecialPower") {
            Some(default) => {
                if default.chain().get_next_override().is_some() {
                    return Err(IniError::undefined(Undefined::AliasedOverrideChain));
                }
                let mut copy = Overridable::new(default.chain().data.clone());
                if default.chain().is_override() {
                    copy.mark_as_override();
                }
                copy
            }
            None => Overridable::new(SpecialPowerTemplateData::default()),
        };
        // friend_setNameAndID(name, ++m_nextSpecialPowerID)
        self.next_special_power_id += 1;
        new.data.name = name.to_vec();
        new.data.id = self.next_special_power_id;
        Ok(Box::new(new))
    }

    /// `SpecialPowerStore::parseSpecialPowerDefinition(ini)` (reached through
    /// `INI::parseSpecialPowerDefinition`, the `SpecialPower` block): the
    /// power's name, then its fields until `End`.
    ///
    /// - In an ordinary load a name that exists is `INI_INVALID_DATA`, and a
    ///   new one gets a template.
    /// - In an override load (`INI_LOAD_CREATE_OVERRIDES`) a name that
    ///   exists gets a copy of its final definition added to its chain, and
    ///   a new one gets a template marked as an override.
    ///
    /// The fields are parsed into the new template or override (a template
    /// stays, as far as it was filled, if a line fails). `is_valid_science`
    /// says which name keys are defined sciences (see the module docs).
    pub fn parse_special_power_definition(
        &mut self,
        ini: &mut Ini<'_>,
        names: &mut NameKeyGenerator,
        is_valid_science: &dyn Fn(NameKeyType) -> bool,
    ) -> IniResult<()> {
        // read the name
        let name = ini.get_next_token(None)?;

        let existing = self.find_private_index(&name);

        let target: &mut Overridable<SpecialPowerTemplateData> =
            if ini.get_load_type() == IniLoadType::CreateOverrides {
                if let Some(i) = existing {
                    let child = self.special_power_templates[i].friend_get_final_override();
                    let mut copy = Overridable::new(child.data.clone());
                    copy.mark_as_override();
                    // the child is the final override: nothing is displaced
                    child.set_next_override(Some(Box::new(copy)));
                    self.special_power_templates[i].friend_get_final_override()
                } else {
                    let mut new = self.new_from_default(&name)?;
                    new.mark_as_override();
                    self.special_power_templates.push(new);
                    self.special_power_templates
                        .last_mut()
                        .expect("just pushed")
                }
            } else if existing.is_some() {
                return Err(IniError::new(IniErrorKind::InvalidData));
            } else {
                let new = self.new_from_default(&name)?;
                self.special_power_templates.push(new);
                self.special_power_templates
                    .last_mut()
                    .expect("just pushed")
            };

        // parse the ini definition
        let mut ctx = FieldCtx {
            data: &mut target.data,
            names,
            is_valid_science,
        };
        let fields = [
            FieldParse::new(
                "ReloadTime",
                |ini, c: &mut FieldCtx<'_>, _| {
                    c.data.reload_time = ini.parse_duration_unsigned_int()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new("RequiredScience", FieldCtx::parse_science, UserData::None),
            FieldParse::new(
                "InitiateSound",
                |ini, c: &mut FieldCtx<'_>, _| c.data.initiate_sound.parse(ini),
                UserData::None,
            ),
            FieldParse::new(
                "InitiateAtLocationSound",
                |ini, c: &mut FieldCtx<'_>, _| c.data.initiate_at_location_sound.parse(ini),
                UserData::None,
            ),
            FieldParse::new(
                "PublicTimer",
                |ini, c: &mut FieldCtx<'_>, _| {
                    c.data.public_timer = ini.parse_bool()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "Enum",
                FieldCtx::parse_enum,
                UserData::Names(&SPECIAL_POWER_NAMES),
            ),
            FieldParse::new(
                "DetectionTime",
                |ini, c: &mut FieldCtx<'_>, _| {
                    c.data.detection_time = ini.parse_duration_unsigned_int()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "SharedSyncedTimer",
                |ini, c: &mut FieldCtx<'_>, _| {
                    c.data.shared_n_sync = ini.parse_bool()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "ViewObjectDuration",
                |ini, c: &mut FieldCtx<'_>, _| {
                    c.data.view_object_duration = ini.parse_duration_unsigned_int()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "ViewObjectRange",
                |ini, c: &mut FieldCtx<'_>, _| {
                    c.data.view_object_range = ini.parse_real()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "RadiusCursorRadius",
                |ini, c: &mut FieldCtx<'_>, _| {
                    c.data.radius_cursor_radius = ini.parse_real()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "ShortcutPower",
                |ini, c: &mut FieldCtx<'_>, _| {
                    c.data.shortcut_power = ini.parse_bool()?;
                    Ok(())
                },
                UserData::None,
            ),
            FieldParse::new(
                "AcademyClassify",
                FieldCtx::parse_academy,
                UserData::Names(&ACADEMY_CLASSIFICATION_TYPE_NAMES),
            ),
            FieldParse::end(),
        ];
        ini.init_from_ini(&mut ctx, &fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::BlockParse;
    use crate::common::special_power_type::{SPECIAL_CASH_HACK, SPECIAL_DAISY_CUTTER};

    struct Ctx {
        store: SpecialPowerStore,
        names: NameKeyGenerator,
        sciences: Vec<NameKeyType>,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        let valid = c.sciences.clone();
        c.store
            .parse_special_power_definition(ini, &mut c.names, &move |k| valid.contains(&k))
    }

    static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse {
        token: "SpecialPower",
        parse: block,
    }];

    fn ctx() -> Ctx {
        let mut c = Ctx {
            store: SpecialPowerStore::new(),
            names: NameKeyGenerator::new(),
            sciences: Vec::new(),
        };
        c.names.init();
        let k = c.names.name_to_key(b"SCIENCE_Paradrop");
        c.sciences.push(k);
        c
    }

    fn load(c: &mut Ctx, kind: IniLoadType, text: &[u8]) -> IniResult<()> {
        Ini::load(b"sp.ini", text, kind, None, &BLOCKS, c)
    }

    #[test]
    fn fields_defaults_and_lookups() {
        let mut c = ctx();
        load(
            &mut c,
            IniLoadType::Overwrite,
            b"SpecialPower SuperweaponDaisyCutter\n Enum = SPECIAL_DAISY_CUTTER\n ReloadTime = 240000\n RequiredScience = SCIENCE_Paradrop\n \
              InitiateSound = DaisyCutterSound\n InitiateAtLocationSound = NoSound\n PublicTimer = Yes\n DetectionTime = 1000\n \
              SharedSyncedTimer = No\n ViewObjectDuration = 2000\n ViewObjectRange = 250.5\n RadiusCursorRadius = 170\n \
              ShortcutPower = Yes\n AcademyClassify = ACT_SUPERPOWER\nEnd\nSpecialPower Plain\nEnd\n",
        )
        .unwrap();
        assert_eq!(c.store.len(), 2);
        let p = c
            .store
            .find_special_power_template(b"SuperweaponDaisyCutter")
            .unwrap();
        assert_eq!((p.id(), p.special_power_type()), (1, SPECIAL_DAISY_CUTTER));
        assert_eq!(p.reload_time(), 7200);
        assert_eq!(p.required_science(), ScienceType(c.sciences[0].0 as i32));
        assert_eq!(p.initiate_sound().event_name, b"DaisyCutterSound");
        assert_eq!(p.initiate_at_target_sound().event_name, b"");
        assert!(p.has_public_timer() && !p.is_shared_n_sync() && p.is_shortcut_power());
        assert_eq!((p.detection_time(), p.view_object_duration()), (30, 60));
        assert_eq!(
            (p.view_object_range(), p.radius_cursor_radius()),
            (250.5, 170.0)
        );
        assert_eq!(
            p.academy_classification_type(),
            AcademyClassificationType::Superpower
        );
        // the constructor's defaults
        let d = c.store.find_special_power_template_by_id(2).unwrap();
        assert_eq!(d.name(), b"Plain");
        assert_eq!(
            (
                d.special_power_type(),
                d.required_science(),
                d.reload_time()
            ),
            (SPECIAL_INVALID, SCIENCE_INVALID, 0)
        );
        assert_eq!(
            d.detection_time(),
            DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT
        );
        assert_eq!(
            d.academy_classification_type(),
            AcademyClassificationType::None
        );
        assert!(c.store.find_special_power_template(b"plain").is_none());
        assert!(c.store.find_special_power_template_by_id(3).is_none());
        assert_eq!(
            c.store
                .get_special_power_template_by_index(1)
                .unwrap()
                .name(),
            b"Plain"
        );
        assert!(c.store.get_special_power_template_by_index(2).is_none());
    }

    #[test]
    fn default_special_power_is_copied_and_names_must_be_new() {
        let mut c = ctx();
        load(&mut c, IniLoadType::Overwrite, b"SpecialPower DefaultSpecialPower\n ReloadTime = 1000\n Enum = SPECIAL_CASH_HACK\nEnd\nSpecialPower B\n ShortcutPower = Yes\nEnd\n").unwrap();
        let b = c.store.find_special_power_template(b"B").unwrap();
        assert_eq!(
            (
                b.id(),
                b.reload_time(),
                b.special_power_type(),
                b.is_shortcut_power()
            ),
            (2, 30, SPECIAL_CASH_HACK, true)
        );
        // a repeated name in an ordinary load is INI_INVALID_DATA
        let r = load(&mut c, IniLoadType::Overwrite, b"SpecialPower B\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(c.store.len(), 2);
        assert_eq!(c.store.next_special_power_id(), 2);
    }

    #[test]
    fn overrides_are_read_through_and_reset_removes_them() {
        let mut c = ctx();
        load(&mut c, IniLoadType::Overwrite, b"SpecialPower A\n ReloadTime = 1000\n Enum = SPECIAL_CASH_HACK\n ShortcutPower = Yes\nEnd\n")
            .unwrap();
        load(
            &mut c,
            IniLoadType::CreateOverrides,
            b"SpecialPower A\n ReloadTime = 5000\n AcademyClassify = ACT_UPGRADE_RADAR\nEnd\nSpecialPower Map\n ReloadTime = 100\nEnd\n",
        )
        .unwrap();
        assert_eq!(c.store.len(), 2);
        let a = c.store.find_special_power_template(b"A").unwrap();
        // the final override: new reload time, everything else copied
        assert_eq!(
            (
                a.reload_time(),
                a.special_power_type(),
                a.is_shortcut_power(),
                a.id()
            ),
            (150, SPECIAL_CASH_HACK, true, 1)
        );
        // the head keeps its own academy type, which is what the getter reads
        assert_eq!(
            a.academy_classification_type(),
            AcademyClassificationType::None
        );
        assert_eq!(
            a.chain()
                .get_final_override()
                .data
                .academy_classification_type,
            AcademyClassificationType::UpgradeRadar
        );
        assert_eq!(
            c.store.find_special_power_template_by_id(2).unwrap().name(),
            b"Map"
        );
        // a second override goes to the end of the chain
        load(
            &mut c,
            IniLoadType::CreateOverrides,
            b"SpecialPower A\n ReloadTime = 7000\nEnd\n",
        )
        .unwrap();
        assert_eq!(
            c.store
                .find_special_power_template(b"A")
                .unwrap()
                .reload_time(),
            210
        );
        c.store.reset();
        assert_eq!(c.store.len(), 1);
        let a = c.store.find_special_power_template(b"A").unwrap();
        assert_eq!(a.reload_time(), 30);
        assert!(a.chain().get_next_override().is_none());
        // ids are not reused
        assert_eq!(c.store.next_special_power_id(), 2);
        load(&mut c, IniLoadType::Overwrite, b"SpecialPower C\nEnd\n").unwrap();
        assert_eq!(c.store.find_special_power_template(b"C").unwrap().id(), 3);
    }

    #[test]
    fn a_default_with_an_override_chain_cannot_be_copied() {
        let mut c = ctx();
        load(
            &mut c,
            IniLoadType::Overwrite,
            b"SpecialPower DefaultSpecialPower\nEnd\n",
        )
        .unwrap();
        load(
            &mut c,
            IniLoadType::CreateOverrides,
            b"SpecialPower DefaultSpecialPower\n ReloadTime = 1000\nEnd\n",
        )
        .unwrap();
        let e = load(&mut c, IniLoadType::Overwrite, b"SpecialPower X\nEnd\n").unwrap_err();
        assert!(e.is_undefined());
        assert_eq!(c.store.len(), 1);
        // nothing was numbered
        assert_eq!(c.store.next_special_power_id(), 1);
    }

    #[test]
    fn errors() {
        let mut c = ctx();
        for (text, kind) in [
            (
                &b"SpecialPower A\n Enum = SPECIAL_NOPE\nEnd\n"[..],
                IniErrorKind::InvalidData,
            ),
            (
                b"SpecialPower A\n RequiredScience = SCIENCE_Unknown\nEnd\n",
                IniErrorKind::InvalidData,
            ),
            (
                b"SpecialPower A\n AcademyClassify = ACT_X\nEnd\n",
                IniErrorKind::InvalidData,
            ),
            (
                b"SpecialPower A\n PublicTimer = Maybe\nEnd\n",
                IniErrorKind::InvalidData,
            ),
            (
                b"SpecialPower A\n InitiateSound =\nEnd\n",
                IniErrorKind::InvalidData,
            ),
            (
                b"SpecialPower A\n Foo = 1\nEnd\n",
                IniErrorKind::UnknownToken,
            ),
            (b"SpecialPower\n", IniErrorKind::InvalidData),
        ] {
            let mut c2 = ctx();
            assert_eq!(
                load(&mut c2, IniLoadType::Overwrite, text)
                    .unwrap_err()
                    .kind,
                kind,
                "{}",
                String::from_utf8_lossy(text)
            );
        }
        // a template whose line failed stays, with what was read before it
        let _ = load(
            &mut c,
            IniLoadType::Overwrite,
            b"SpecialPower A\n ReloadTime = 1000\n Foo = 1\nEnd\n",
        );
        assert_eq!(
            c.store
                .find_special_power_template(b"A")
                .unwrap()
                .reload_time(),
            30
        );
    }

    #[test]
    fn mask_helpers() {
        let a = make_special_power_mask(SPECIAL_CASH_HACK).unwrap();
        assert_eq!(test_special_power_mask(&a, SPECIAL_CASH_HACK), Ok(true));
        assert_eq!(test_special_power_mask(&a, 67), Err(BitIndexOutOfRange));
        let mut m = SpecialPowerMaskType::new();
        set_special_power_mask(&mut m, SPECIAL_DAISY_CUTTER, 1).unwrap();
        assert!(test_special_power_mask_any(
            &m,
            &make_special_power_mask(SPECIAL_DAISY_CUTTER).unwrap()
        ));
        assert!(test_special_power_mask_multi(
            &m,
            &make_special_power_mask(SPECIAL_DAISY_CUTTER).unwrap(),
            &a
        ));
        assert!(special_power_mask_any_set(&m));
        set_special_power_mask(&mut m, SPECIAL_DAISY_CUTTER, 0).unwrap();
        assert!(!special_power_mask_any_set(&m));
        set_all_special_power_mask_bits(&mut m);
        assert_eq!(m.count(), 67);
        flip_special_power_mask(&mut m);
        clear_special_power_mask(&mut m);
        assert!(!special_power_mask_any_set(&m));
        assert_eq!(make_special_power_mask(-1), Err(BitIndexOutOfRange));
    }
}
