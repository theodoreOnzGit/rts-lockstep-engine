// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/CrateSystem.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/CrateSystem.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INICrate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Graham Smallwood Feb 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CrateTemplate` (an INI `CrateData` block: when a crate is created, and
//! what it holds), `CrateSystem` (`TheCrateSystem`: every template, with its
//! INI overrides) and the `CrateData` block handler.
//!
//! # Translation choices
//!
//! - **Overrides.** Upstream's `CrateTemplate` derives from `Overridable`, so
//!   the template and its chain link are one object; the store's vector holds
//!   the chain heads. Here a store element is an [`Overridable<CrateTemplate>`]
//!   (the chain link, owning the newer overrides), and `OVERRIDE<T>`'s
//!   "final override" is [`Overridable::get_final_override`].
//! - **Copying a template** (`*ct = *defaultCT`, `*newOverride = *crateToOverride`)
//!   copies the whole object upstream, the `Overridable` part included. The
//!   copy's `m_isOverride` flag is therefore copied too (a new base template
//!   copied from a `DefaultCrate` override is itself an override, and the
//!   next `reset()` deletes it). That is reproduced: see
//!   [`CrateSystem::new_crate_template`]. The copy's next override is always
//!   empty, since the source is the final override.
//! - **`m_killerScience`.** Upstream's `ScienceType` is the name key of the
//!   science (`ScienceStore::friend_lookupScience` is `NAMEKEY(name)`), and
//!   `NAMEKEY` assigns a key to a new name, which is lockstep state. The field
//!   parser cannot reach the [`NameKeyGenerator`], so it keeps the token, and
//!   the block handler converts it to a key after the block's fields are read,
//!   in the same order of events. The science store's validity check (an
//!   unknown name is `INI_INVALID_DATA` upstream) is the `ScienceStore`'s,
//!   which is not translated: here an unknown name is accepted, as the
//!   differential stand-in accepts it.
//! - **`LEVEL_INVALID`** (the default veterancy) is `None`.
//! - `DEBUG_CRASH` for a duplicate name compiles to nothing in the release
//!   configuration upstream, so a duplicate block is merged into the existing
//!   template, as upstream does.
//! - `CrateSystem::reset` removes the overrides and keeps the base templates,
//!   in their order.
//!
//! # Left out: methods that take an `Object` or a `Player`
//!
//! None of `CrateSystem.h` or `CrateSystem.cpp` takes an `Object` or a
//! `Player`. The crate-creation code that does (`CreateCrateDie`,
//! `CrateCollide`, the `Player` checks for `OwnedByMaker` and the kill
//! conditions) is in the object-module layers.

use crate::common::ascii_string::AsciiString;
use crate::common::game_common::{VeterancyLevel, VETERANCY_NAMES};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Undefined, UserData};
use crate::common::kind_of::KindOfMaskType;
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::overridable::Overridable;
use crate::common::subsystem_interface::SubsystemInterface;

/// `struct crateCreationEntry`: one crate that a template may create, and
/// its chance (of the list's one-of-n choice).
#[derive(Clone, Debug, PartialEq)]
pub struct CrateCreationEntry {
    /// `crateName`: the crate's template name.
    pub crate_name: Vec<u8>,
    /// `crateChance`.
    pub crate_chance: f32,
}

/// `class CrateTemplate`: "a ini defined set of conditions plus a ThingTemplate
/// that is the Object containing the correct CrateCollide module".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CrateTemplate {
    /// `m_name`.
    name: Vec<u8>,
    /// `m_creationChance`: the chance that the crate is created at all.
    creation_chance: f32,
    /// `m_veterancyLevel`: the level of the unit that was killed (`None` is
    /// `LEVEL_INVALID`).
    veterancy_level: Option<VeterancyLevel>,
    /// `m_killedByTypeKindof`: the killer must have all these kinds.
    killed_by_type_kindof: KindOfMaskType,
    /// `m_killerScience`: the killer must have this science, as its name key
    /// (`None` is `SCIENCE_INVALID`).
    killer_science: Option<NameKeyType>,
    /// `m_possibleCrates`: "CreationChance is for this CrateData to succeed,
    /// this list controls one-of-n crates created on success".
    possible_crates: Vec<CrateCreationEntry>,
    /// `m_isOwnedByMaker`: "Design needs crates to be owned sometimes".
    is_owned_by_maker: bool,
    /// The science name of the `KillerScience` line being read, until the
    /// block handler converts it to a key (see the module docs). Always
    /// `None` outside that handler.
    pending_killer_science: Option<Vec<u8>>,
}

impl CrateTemplate {
    /// `getName()`.
    pub fn name(&self) -> &[u8] {
        &self.name
    }

    /// `m_creationChance`.
    pub fn creation_chance(&self) -> f32 {
        self.creation_chance
    }

    /// `m_veterancyLevel`; `None` is `LEVEL_INVALID`.
    pub fn veterancy_level(&self) -> Option<VeterancyLevel> {
        self.veterancy_level
    }

    /// `m_killedByTypeKindof`.
    pub fn killed_by_type_kindof(&self) -> &KindOfMaskType {
        &self.killed_by_type_kindof
    }

    /// `m_killerScience`, as the science's name key; `None` is `SCIENCE_INVALID`.
    pub fn killer_science(&self) -> Option<NameKeyType> {
        self.killer_science
    }

    /// `m_possibleCrates`.
    pub fn possible_crates(&self) -> &[CrateCreationEntry] {
        &self.possible_crates
    }

    /// `m_isOwnedByMaker`.
    pub fn is_owned_by_maker(&self) -> bool {
        self.is_owned_by_maker
    }
}

/// `class CrateSystem`: every crate template, and their overrides.
#[derive(Clone, Debug)]
pub struct CrateSystem {
    /// `m_name` (the subsystem's name).
    name: AsciiString,
    /// `m_crateTemplateVector`: the base template of each name, each the head
    /// of its override chain.
    crate_template_vector: Vec<Overridable<CrateTemplate>>,
}

impl Default for CrateSystem {
    /// `CrateSystem()`: no templates.
    fn default() -> Self {
        CrateSystem { name: AsciiString::default(), crate_template_vector: Vec::new() }
    }
}

impl SubsystemInterface for CrateSystem {
    fn name(&self) -> &AsciiString {
        &self.name
    }

    fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    /// `init()`: `reset()`.
    fn init(&mut self) {
        self.reset();
    }

    /// `update() { }`.
    fn update(&mut self) {}

    /// `reset()`: see [`CrateSystem::reset`].
    fn reset(&mut self) {
        CrateSystem::reset(self);
    }
}

impl CrateSystem {
    /// `CrateSystem()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// The position of the base template called `name`.
    fn index_of(&self, name: &[u8]) -> Option<usize> {
        self.crate_template_vector.iter().position(|o| o.data.name == name)
    }

    /// `findCrateTemplate(name)`: the newest definition of the template
    /// (`OVERRIDE<CrateTemplate>` reads the final override). `None` if there
    /// is no such template.
    pub fn find_crate_template(&self, name: &[u8]) -> Option<&CrateTemplate> {
        self.index_of(name).map(|i| &self.crate_template_vector[i].get_final_override().data)
    }

    /// `newCrateTemplate(name)`: a new base template, copied from the
    /// `DefaultCrate` template if there is one (its values, and its override
    /// flag, as upstream's whole-object copy), named `name`, appended.
    /// Returns its position, or `None` for an empty name (upstream's
    /// `NULL`).
    pub fn new_crate_template(&mut self, name: &[u8]) -> Option<usize> {
        if name.is_empty() {
            return None;
        }
        let mut node = Overridable::new(CrateTemplate::default());
        if let Some(i) = self.index_of(b"DefaultCrate") {
            let default = self.crate_template_vector[i].get_final_override();
            node.data = default.data.clone();
            if default.is_override() {
                node.mark_as_override();
            }
        }
        node.data.name = name.to_vec();
        self.crate_template_vector.push(node);
        Some(self.crate_template_vector.len() - 1)
    }

    /// `newCrateTemplateOverride(crateToOverride)` for the base template at
    /// `index`: a copy of its final override, marked as an override, hung
    /// off the final override. Upstream returns it; here the caller reads it
    /// through the chain ([`CrateSystem::friend_find_crate_template`]).
    pub fn new_crate_template_override(&mut self, index: usize) {
        let last = self.crate_template_vector[index].friend_get_final_override();
        let mut copy = Overridable::new(last.data.clone());
        copy.mark_as_override();
        last.set_next_override(Some(Box::new(copy)));
    }

    /// `m_crateTemplateVector`: each base template, with its override chain
    /// hanging off it ([`Overridable::get_next_override`]).
    pub fn chains(&self) -> &[Overridable<CrateTemplate>] {
        &self.crate_template_vector
    }

    /// `CrateSystem::parseCrateTemplateDefinition(ini)` (the `CrateData`
    /// block): the name, then the template (found, or new as an override or
    /// as a base), then its fields until `End`.
    ///
    /// A duplicate name outside an override load merges into the existing
    /// template (upstream's `DEBUG_CRASH` is empty here). The `names` are the
    /// [`NameKeyGenerator`] that `KillerScience` keys come from.
    pub fn parse_crate_template_definition(&mut self, ini: &mut Ini<'_>, names: &mut NameKeyGenerator) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        let overrides = ini.get_load_type() == IniLoadType::CreateOverrides;
        let index = match self.index_of(&name) {
            None => {
                let Some(i) = self.new_crate_template(&name) else {
                    // unreachable: a token is never empty
                    return Err(IniError::new(IniErrorKind::InvalidData));
                };
                if overrides {
                    self.crate_template_vector[i].mark_as_override();
                }
                i
            }
            Some(i) => {
                if overrides {
                    self.new_crate_template_override(i);
                }
                i
            }
        };
        let (result, science) = {
            let target = self.crate_template_vector[index].friend_get_final_override();
            let result = ini.init_from_ini(&mut target.data, &CRATE_TEMPLATE_FIELDS);
            (result, target.data.pending_killer_science.take())
        };
        if let Some(science) = science {
            let key = names
                .try_name_to_key(&science)
                .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
            self.crate_template_vector[index].friend_get_final_override().data.killer_science = Some(key);
        }
        result
    }

    /// `reset()`: "clean up overrides". Every override is removed, and a
    /// base template that is itself an override is removed with its chain;
    /// the base templates that remain keep their order.
    pub fn reset(&mut self) {
        let old = std::mem::take(&mut self.crate_template_vector);
        for node in old {
            if let Some(kept) = Box::new(node).delete_overrides() {
                self.crate_template_vector.push(*kept);
            }
        }
    }
}

/// `CrateTemplate::TheCrateTemplateFieldParseTable`.
static CRATE_TEMPLATE_FIELDS: [FieldParse<CrateTemplate>; 7] = [
    FieldParse::new("CreationChance", parse_creation_chance, UserData::None),
    FieldParse::new("VeterancyLevel", parse_veterancy_level, UserData::Names(&VETERANCY_NAMES)),
    FieldParse::new("KilledByType", parse_killed_by_type, UserData::None),
    FieldParse::new("CrateObject", parse_crate_creation_entry, UserData::None),
    FieldParse::new("KillerScience", parse_killer_science, UserData::None),
    FieldParse::new("OwnedByMaker", parse_owned_by_maker, UserData::None),
    FieldParse::end(),
];

fn parse_creation_chance(ini: &mut Ini<'_>, c: &mut CrateTemplate, _ud: &UserData) -> IniResult<()> {
    c.creation_chance = ini.parse_real()?;
    Ok(())
}

/// `parseIndexList` with `TheVeterancyNames`.
fn parse_veterancy_level(ini: &mut Ini<'_>, c: &mut CrateTemplate, ud: &UserData) -> IniResult<()> {
    let index = ini.parse_index_list(ud.names())?;
    c.veterancy_level = Some(match index {
        0 => VeterancyLevel::Regular,
        1 => VeterancyLevel::Veteran,
        2 => VeterancyLevel::Elite,
        3 => VeterancyLevel::Heroic,
        _ => return Err(IniError::new(IniErrorKind::InvalidData)),
    });
    Ok(())
}

fn parse_killed_by_type(ini: &mut Ini<'_>, c: &mut CrateTemplate, _ud: &UserData) -> IniResult<()> {
    c.killed_by_type_kindof.parse_from_ini(ini)
}

/// `CrateTemplate::parseCrateCreationEntry`: `CrateObject = <name> <chance>`.
/// The chance is read with `sscanf("%f")`.
fn parse_crate_creation_entry(ini: &mut Ini<'_>, c: &mut CrateTemplate, _ud: &UserData) -> IniResult<()> {
    let crate_name = ini.get_next_token(None)?;
    let token = ini.get_next_token(None)?;
    let crate_chance = Ini::scan_real(&token)?;
    c.possible_crates.push(CrateCreationEntry { crate_name, crate_chance });
    Ok(())
}

/// `INI::parseScience` (see the module docs): the name is kept until the
/// block handler converts it.
fn parse_killer_science(ini: &mut Ini<'_>, c: &mut CrateTemplate, _ud: &UserData) -> IniResult<()> {
    c.pending_killer_science = Some(ini.get_next_token(None)?);
    Ok(())
}

fn parse_owned_by_maker(ini: &mut Ini<'_>, c: &mut CrateTemplate, _ud: &UserData) -> IniResult<()> {
    c.is_owned_by_maker = ini.parse_bool()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};

    struct Ctx {
        crates: CrateSystem,
        names: NameKeyGenerator,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.crates.parse_crate_template_definition(ini, &mut c.names)
    }

    static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse { token: "CrateData", parse: block }];

    fn ctx() -> Ctx {
        let mut names = NameKeyGenerator::new();
        names.init();
        Ctx { crates: CrateSystem::new(), names }
    }

    fn load_into(c: &mut Ctx, load: IniLoadType, text: &[u8]) -> IniResult<()> {
        Ini::load(b"c.ini", text, load, None, &BLOCKS, c)
    }

    #[test]
    fn fields_are_read_and_the_default_is_copied() {
        let mut c = ctx();
        load_into(
            &mut c,
            IniLoadType::Overwrite,
            b"CrateData DefaultCrate\n CreationChance = 0.5\n VeterancyLevel = ELITE\n OwnedByMaker = Yes\nEnd\n\
              CrateData SalvageCrate\n CrateObject = SalvageMoney 0.25\n CrateObject = SalvageUnit 1\n KillerScience = SCIENCE_A\nEnd\n",
        )
        .unwrap();
        let s = c.crates.find_crate_template(b"SalvageCrate").unwrap();
        assert_eq!(s.creation_chance(), 0.5, "copied from DefaultCrate");
        assert_eq!(s.veterancy_level(), Some(VeterancyLevel::Elite));
        assert!(s.is_owned_by_maker());
        assert_eq!(s.possible_crates().len(), 2);
        assert_eq!(s.possible_crates()[0], CrateCreationEntry { crate_name: b"SalvageMoney".to_vec(), crate_chance: 0.25 });
        assert_eq!(s.killer_science(), Some(c.names.name_to_key(b"SCIENCE_A")));
        assert_eq!(c.crates.find_crate_template(b"salvagecrate"), None, "names are case-sensitive");
    }

    #[test]
    fn duplicate_base_merges_and_overrides_chain() {
        let mut c = ctx();
        load_into(&mut c, IniLoadType::Overwrite, b"CrateData A\n CreationChance = 0.1\nEnd\nCrateData A\n OwnedByMaker = Yes\nEnd\n").unwrap();
        let a = c.crates.find_crate_template(b"A").unwrap();
        assert_eq!((a.creation_chance(), a.is_owned_by_maker()), (0.1, true));
        assert_eq!(c.crates.crate_template_vector.len(), 1);
        load_into(&mut c, IniLoadType::CreateOverrides, b"CrateData A\n CreationChance = 0.9\nEnd\n").unwrap();
        assert_eq!(c.crates.find_crate_template(b"A").unwrap().creation_chance(), 0.9, "the override is the one read");
        assert_eq!(c.crates.crate_template_vector.len(), 1);
        assert!(c.crates.crate_template_vector[0].get_next_override().is_some());
        c.crates.reset();
        assert_eq!(c.crates.find_crate_template(b"A").unwrap().creation_chance(), 0.1, "reset removes the override");
    }

    #[test]
    fn subsystem_interface_init_resets_and_name_is_kept() {
        let mut c = CrateSystem::new();
        c.set_name(AsciiString::default());
        assert!(c.name().is_empty());
        c.update();
        let mut names = NameKeyGenerator::new();
        names.init();
        let mut ctx = Ctx { crates: c, names };
        load_into(&mut ctx, IniLoadType::CreateOverrides, b"CrateData Mod\n CreationChance = 0.3\nEnd\n").unwrap();
        ctx.crates.init();
        assert!(ctx.crates.find_crate_template(b"Mod").is_none(), "init is reset");
        ctx.crates.reset();
    }

    #[test]
    fn override_block_creating_a_new_template_is_removed_by_reset() {
        let mut c = ctx();
        load_into(&mut c, IniLoadType::CreateOverrides, b"CrateData Mod\n CreationChance = 0.3\nEnd\n").unwrap();
        assert!(c.crates.find_crate_template(b"Mod").is_some());
        c.crates.reset();
        assert!(c.crates.find_crate_template(b"Mod").is_none());
        assert!(c.crates.crate_template_vector.is_empty());
    }

    #[test]
    fn errors_are_reported_and_partial_fields_stay() {
        let mut c = ctx();
        let r = load_into(&mut c, IniLoadType::Overwrite, b"CrateData A\n CreationChance = 0.2\n CrateObject = X nope\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(c.crates.find_crate_template(b"A").unwrap().creation_chance(), 0.2);
        let r = load_into(&mut c, IniLoadType::Overwrite, b"CrateData B\n VeterancyLevel = LEGEND\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(c.crates.find_crate_template(b"B").unwrap().veterancy_level(), None);
        let r = load_into(&mut c, IniLoadType::Overwrite, b"CrateData C\n KillerScience\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(c.crates.find_crate_template(b"C").unwrap().killer_science(), None);
    }
}
