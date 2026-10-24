// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/WeaponSet.h (`WeaponTemplateSet`)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/WeaponSet.cpp (`WeaponTemplateSet`'s members)
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp (`INI::parseWeaponTemplate`)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AI.h (`TheCommandSourceMaskNames`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   WeaponSet.cpp: Steven Johnson, March 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WeaponTemplateSet`: one `WeaponSet` block of an `Object`: the conditions
//! under which it applies, the weapon of each of the three slots, which
//! command sources may auto-choose each slot, which kinds of target each slot
//! is preferred against, and two flags. A thing template holds a list of
//! these and picks the one that best matches an object's weapon-set flags.
//!
//! # What is translated
//!
//! Only `WeaponTemplateSet` (the template side of `WeaponSet.h`): its data,
//! `clear`, `hasAnyWeapons`, `testWeaponSetFlag`, the match-finder interface
//! and the `WeaponSet` INI block (`parseWeaponTemplateSet` with its four
//! field parsers). `WeaponSet` itself (an object's current weapons, their
//! locks, `xfer`, `weaponSetFromBlock`) needs `Object`, `Weapon` and
//! `Player` and belongs to layer 8.
//!
//! # Cut point: the weapon store
//!
//! `Weapon =` looks the name up in `TheWeaponStore` (`Weapon.cpp`, layer 8)
//! and keeps the `const WeaponTemplate*`. The Rust keeps the weapon's **name
//! key** and takes the set of keys the store holds as a [`WeaponTemplateTable`]
//! (the part of `WeaponStore` that `findWeaponTemplate` uses); a name that
//! the table does not hold is `None`, as upstream's `NULL` (its
//! `DEBUG_ASSERTCRASH` is compiled out of the shipped build). `findWeaponTemplate`
//! calls `nameToKey`, which **assigns a key to a name it has not seen**, and
//! `None` (any case) returns before it does.
//!
//! # Translation choices
//!
//! - `m_thingTemplate`, "needed for save/load", is the **id** of the thing
//!   template the set was parsed for ([`WeaponTemplateSet::thing_template`]);
//!   `WeaponSet::xfer` saves that template's name. Upstream never sets it in
//!   `clear()`, so a set that was not parsed has none.
//! - The match finder is [`crate::common::sparse_match_finder::SparseMatchFinder`]
//!   over [`WeaponSetFlags`]; a set has one "yes" mask, its conditions.
//!
//! # Verified
//!
//! Against upstream `WeaponSet.cpp`'s `WeaponTemplateSet` code (cut out of the
//! file verbatim by `build.rs`, since the rest of the file needs `Object`) by
//! `tests/differential_thing_template.rs`.

use std::collections::BTreeSet;

use crate::common::game_type::WEAPONSLOT_COUNT;
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, Undefined, UserData};
use crate::common::kind_of::KindOfMaskType;
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::sparse_match_finder::SparseMatchable;
use crate::logic::weapon_set_flags::{WeaponSetFlags, WeaponSetNames};
use crate::logic::weapon_types::WeaponSetType;

/// `WEAPONSLOT_COUNT` as an array length.
pub const SLOTS: usize = WEAPONSLOT_COUNT as usize;

/// `TheWeaponSlotTypeNames[]`, without the `NULL`.
pub const WEAPON_SLOT_TYPE_NAMES: [&str; SLOTS] = ["PRIMARY", "SECONDARY", "TERTIARY"];

/// `TheCommandSourceMaskNames[]` (AI.h), without the `NULL`.
pub const COMMAND_SOURCE_MASK_NAMES: [&str; 5] =
    ["FROM_PLAYER", "FROM_SCRIPT", "FROM_AI", "FROM_DOZER", "DEFAULT_SWITCH_WEAPON"];

/// The part of `TheWeaponStore` that `findWeaponTemplate` uses: which weapon
/// templates exist, by name key (a cut point, see the module docs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponTemplateTable {
    keys: BTreeSet<NameKeyType>,
}

impl WeaponTemplateTable {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a weapon template, by the name key of its name.
    pub fn add(&mut self, key: NameKeyType) {
        self.keys.insert(key);
    }

    /// Whether a weapon template has this key.
    pub fn contains(&self, key: NameKeyType) -> bool {
        self.keys.contains(&key)
    }

    /// `findWeaponTemplate(name)`: `None` for `None` (any case) without
    /// touching the name keys; otherwise `nameToKey(name)` and the template
    /// under it, if any.
    pub fn find_weapon_template(&self, names: &mut NameKeyGenerator, name: &[u8]) -> IniResult<Option<NameKeyType>> {
        if name.eq_ignore_ascii_case(b"None") {
            return Ok(None);
        }
        let key = names.try_name_to_key(name).map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        Ok(self.keys.contains(&key).then_some(key))
    }
}

/// `class WeaponTemplateSet`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponTemplateSet {
    /// `m_thingTemplate`: the id of the thing template the set was parsed
    /// for (`None` until it is).
    thing_template: Option<u16>,
    /// `m_types`: the conditions.
    types: WeaponSetFlags,
    /// `m_template`: the weapon of each slot, by name key.
    template: [Option<NameKeyType>; SLOTS],
    /// `m_autoChooseMask`: the command sources that may auto-choose each slot.
    auto_choose_mask: [u32; SLOTS],
    /// `m_preferredAgainst`.
    preferred_against: [KindOfMaskType; SLOTS],
    /// `m_isReloadTimeShared`.
    is_reload_time_shared: bool,
    /// `m_isWeaponLockSharedAcrossSets`: "a weapon set so similar that it is
    /// safe to hold locks across".
    is_weapon_lock_shared_across_sets: bool,
}

impl Default for WeaponTemplateSet {
    fn default() -> Self {
        WeaponTemplateSet::new()
    }
}

impl WeaponTemplateSet {
    /// `WeaponTemplateSet()`: `clear()`.
    pub fn new() -> Self {
        WeaponTemplateSet {
            thing_template: None,
            types: WeaponSetFlags::new(),
            template: [None; SLOTS],
            auto_choose_mask: [0xffff_ffff; SLOTS],
            preferred_against: [KindOfMaskType::new(); SLOTS],
            is_reload_time_shared: false,
            is_weapon_lock_shared_across_sets: false,
        }
    }

    // --- p1w6-object ---
    /// A set for tests and the differential test, built without INI: the
    /// conditions, and whether the primary slot has a weapon (a name key of
    /// 1; no weapon store is consulted).
    pub fn from_values(thing_template: Option<u16>, types: WeaponSetFlags, has_primary_weapon: bool) -> Self {
        let mut s = WeaponTemplateSet::new();
        s.thing_template = thing_template;
        s.types = types;
        if has_primary_weapon {
            s.template[0] = Some(NameKeyType(1));
        }
        s
    }
    // --- end p1w6-object ---

    // --- p1w8-sonnet ---
    /// A set built from its parts, without INI (a host that has no INI, the differential test): the thing template it
    /// belongs to, its conditions, the weapon of each slot (by name key), the command sources that may auto-choose each
    /// slot, what each slot is preferred against, and the two flags.
    pub fn from_parts(
        thing_template: Option<u16>,
        types: WeaponSetFlags,
        template: [Option<NameKeyType>; SLOTS],
        auto_choose_mask: [u32; SLOTS],
        preferred_against: [KindOfMaskType; SLOTS],
        is_reload_time_shared: bool,
        is_weapon_lock_shared_across_sets: bool,
    ) -> Self {
        WeaponTemplateSet { thing_template, types, template, auto_choose_mask, preferred_against, is_reload_time_shared, is_weapon_lock_shared_across_sets }
    }
    // --- end p1w8-sonnet ---

    /// `clear()`: no conditions, no weapons, "by default, allow autochoosing
    /// from any CommandSource" and "weapon isn't preferred against anything
    /// in particular". (`m_thingTemplate` is not touched.)
    pub fn clear(&mut self) {
        self.is_reload_time_shared = false;
        self.is_weapon_lock_shared_across_sets = false;
        self.types.clear();
        for i in 0..SLOTS {
            self.template[i] = None;
            self.auto_choose_mask[i] = 0xffff_ffff;
            self.preferred_against[i].clear();
        }
    }

    /// `friend_getThingTemplate()`: the id of the template the set is for.
    pub fn thing_template(&self) -> Option<u16> {
        self.thing_template
    }

    /// `friend_getWeaponSetFlags()`.
    pub fn weapon_set_flags(&self) -> &WeaponSetFlags {
        &self.types
    }

    /// `testWeaponSetFlag(wst)`.
    pub fn test_weapon_set_flag(&self, wst: WeaponSetType) -> bool {
        self.types.test(wst)
    }

    /// `isSharedReloadTime()`.
    pub fn is_shared_reload_time(&self) -> bool {
        self.is_reload_time_shared
    }

    /// `isWeaponLockSharedAcrossSets()`.
    pub fn is_weapon_lock_shared_across_sets(&self) -> bool {
        self.is_weapon_lock_shared_across_sets
    }

    /// `hasAnyWeapons()`.
    pub fn has_any_weapons(&self) -> bool {
        self.template.iter().any(|t| t.is_some())
    }

    /// `getNth(n)`: the weapon template of slot `n` (its name key).
    pub fn get_nth(&self, n: usize) -> Option<NameKeyType> {
        self.template[n]
    }

    /// `getNthCommandSourceMask(n)`.
    pub fn get_nth_command_source_mask(&self, n: usize) -> u32 {
        self.auto_choose_mask[n]
    }

    /// `getNthPreferredAgainstMask(n)`.
    pub fn get_nth_preferred_against_mask(&self, n: usize) -> &KindOfMaskType {
        &self.preferred_against[n]
    }

    /// `parseWeaponTemplateSet(ini, tt)`: the fields of the block until
    /// `End`, then `m_thingTemplate = tt` (so a block that fails leaves it
    /// as it was). `tt` is the id of the thing template being parsed.
    pub fn parse_weapon_template_set(
        &mut self,
        ini: &mut Ini<'_>,
        tt: u16,
        names: &mut NameKeyGenerator,
        weapons: &WeaponTemplateTable,
    ) -> IniResult<()> {
        let mut fill = WeaponFill { set: self, names, weapons };
        ini.init_from_ini(&mut fill, &weapon_set_fields())?;
        self.thing_template = Some(tt);
        Ok(())
    }
}

impl SparseMatchable<WeaponSetNames> for WeaponTemplateSet {
    /// `getConditionsYesCount()`: one.
    fn conditions_yes_count(&self) -> i32 {
        1
    }

    /// `getNthConditionsYes(i)`: the conditions.
    fn nth_conditions_yes(&self, _i: i32) -> &WeaponSetFlags {
        &self.types
    }
}

/// What the fields of a `WeaponSet` block fill.
struct WeaponFill<'a> {
    set: &'a mut WeaponTemplateSet,
    names: &'a mut NameKeyGenerator,
    weapons: &'a WeaponTemplateTable,
}

/// `INI::scanIndexList(ini->getNextToken(), TheWeaponSlotTypeNames)`.
fn next_slot(ini: &mut Ini<'_>) -> IniResult<usize> {
    let token = ini.get_next_token(None)?;
    Ok(Ini::scan_index_list(&token, &WEAPON_SLOT_TYPE_NAMES)? as usize)
}

fn f_conditions(ini: &mut Ini<'_>, w: &mut WeaponFill<'_>, _: &UserData) -> IniResult<()> {
    w.set.types.parse_from_ini(ini)
}

/// `WeaponTemplateSet::parseWeapon`: the slot, then `INI::parseWeaponTemplate`.
fn f_weapon(ini: &mut Ini<'_>, w: &mut WeaponFill<'_>, _: &UserData) -> IniResult<()> {
    let slot = next_slot(ini)?;
    let token = ini.get_next_token(None)?;
    w.set.template[slot] = w.weapons.find_weapon_template(w.names, &token)?;
    Ok(())
}

/// `WeaponTemplateSet::parseAutoChoose`: the slot, then `parseBitString32`
/// over the command source names, into that slot's mask.
fn f_auto_choose(ini: &mut Ini<'_>, w: &mut WeaponFill<'_>, _: &UserData) -> IniResult<()> {
    let slot = next_slot(ini)?;
    ini.parse_bit_string32(&mut w.set.auto_choose_mask[slot], &COMMAND_SOURCE_MASK_NAMES)
}

/// `WeaponTemplateSet::parsePreferredAgainst`: the slot, then a kind-of mask.
fn f_preferred_against(ini: &mut Ini<'_>, w: &mut WeaponFill<'_>, _: &UserData) -> IniResult<()> {
    let slot = next_slot(ini)?;
    w.set.preferred_against[slot].parse_from_ini(ini)
}

fn f_share_reload(ini: &mut Ini<'_>, w: &mut WeaponFill<'_>, _: &UserData) -> IniResult<()> {
    w.set.is_reload_time_shared = ini.parse_bool()?;
    Ok(())
}

fn f_lock_shared(ini: &mut Ini<'_>, w: &mut WeaponFill<'_>, _: &UserData) -> IniResult<()> {
    w.set.is_weapon_lock_shared_across_sets = ini.parse_bool()?;
    Ok(())
}

/// The field table of `WeaponTemplateSet::parseWeaponTemplateSet`.
fn weapon_set_fields<'a>() -> [FieldParse<WeaponFill<'a>>; 7] {
    [
        FieldParse::new("Conditions", f_conditions, UserData::None),
        FieldParse::new("Weapon", f_weapon, UserData::None),
        FieldParse::new("AutoChooseSources", f_auto_choose, UserData::None),
        FieldParse::new("PreferredAgainst", f_preferred_against, UserData::None),
        FieldParse::new("ShareWeaponReloadTime", f_share_reload, UserData::None),
        FieldParse::new("WeaponLockSharedAcrossSets", f_lock_shared, UserData::None),
        FieldParse::end(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniErrorKind, IniLoadType};
    use crate::common::kind_of::{KINDOF_INFANTRY, KINDOF_VEHICLE};
    use crate::common::sparse_match_finder::SparseMatchFinder;
    use crate::logic::weapon_types::{WEAPONSET_ELITE, WEAPONSET_VETERAN};

    struct Ctx {
        sets: Vec<WeaponTemplateSet>,
        names: NameKeyGenerator,
        weapons: WeaponTemplateTable,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        let mut ws = WeaponTemplateSet::new();
        ws.parse_weapon_template_set(ini, 9, &mut c.names, &c.weapons)?;
        c.sets.push(ws);
        Ok(())
    }

    static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse { token: "Set", parse: block }];

    fn load(text: &[u8]) -> (Ctx, IniResult<()>) {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut weapons = WeaponTemplateTable::new();
        for w in ["Rifle", "Cannon"] {
            let k = names.name_to_key(w.as_bytes());
            weapons.add(k);
        }
        let mut c = Ctx { sets: Vec::new(), names, weapons };
        let r = Ini::load(b"w.ini", text, IniLoadType::Overwrite, None, &BLOCKS, &mut c);
        (c, r)
    }

    #[test]
    fn a_block_fills_the_slots_masks_and_flags() {
        let (c, r) = load(
            b"Set\n Conditions = VETERAN ELITE\n Weapon = PRIMARY Rifle\n Weapon = SECONDARY None\n Weapon = TERTIARY Unknown\n \
AutoChooseSources = SECONDARY FROM_AI FROM_SCRIPT\n PreferredAgainst = PRIMARY INFANTRY VEHICLE\n \
ShareWeaponReloadTime = Yes\n WeaponLockSharedAcrossSets = Yes\nEnd\n",
        );
        r.unwrap();
        let s = &c.sets[0];
        assert!(s.test_weapon_set_flag(WEAPONSET_VETERAN) && s.test_weapon_set_flag(WEAPONSET_ELITE));
        assert_eq!(s.thing_template(), Some(9));
        assert!(s.get_nth(0).is_some());
        assert_eq!(s.get_nth(1), None, "None is no weapon");
        assert_eq!(s.get_nth(2), None, "a weapon the store does not hold is null");
        assert!(s.has_any_weapons());
        assert_eq!(s.get_nth_command_source_mask(0), 0xffff_ffff);
        assert_eq!(s.get_nth_command_source_mask(1), 0b110);
        assert!(s.get_nth_preferred_against_mask(0).test(KINDOF_INFANTRY) && s.get_nth_preferred_against_mask(0).test(KINDOF_VEHICLE));
        assert!(!s.get_nth_preferred_against_mask(1).any());
        assert!(s.is_shared_reload_time() && s.is_weapon_lock_shared_across_sets());
    }

    #[test]
    fn errors_leave_no_set_and_a_failed_weapon_keeps_the_names_it_made() {
        let (c, r) = load(b"Set\n Weapon = QUATERNARY Rifle\nEnd\n");
        // scanIndexList: a token that is not in the list is INI_INVALID_DATA (INI_INVALID_NAME_LIST is for an empty list)
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert!(c.sets.is_empty());
        let (_, r) = load(b"Set\n AutoChooseSources = PRIMARY FROM_MARS\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        let (_, r) = load(b"Set\n Frobnicate = 1\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::UnknownToken);
        // `None` does not assign a key; any other name does, found or not
        let mut names = NameKeyGenerator::new();
        names.reset();
        let table = WeaponTemplateTable::new();
        assert_eq!(table.find_weapon_template(&mut names, b"nOnE").unwrap(), None);
        assert_eq!(names.name_to_key(b"Next"), NameKeyType(1));
        assert_eq!(table.find_weapon_template(&mut names, b"Gone").unwrap(), None);
        assert_eq!(names.name_to_key(b"After"), NameKeyType(3));
    }

    #[test]
    fn the_best_match_prefers_the_most_conditions_in_common() {
        let (c, r) = load(
            b"Set\n Weapon = PRIMARY Rifle\nEnd\nSet\n Conditions = VETERAN\n Weapon = PRIMARY Cannon\nEnd\n\
Set\n Conditions = VETERAN ELITE\n Weapon = PRIMARY Rifle\nEnd\n",
        );
        r.unwrap();
        let mut finder = SparseMatchFinder::new();
        let mut flags = WeaponSetFlags::new();
        assert_eq!(finder.find_best_info(&c.sets, &flags), Some(0));
        flags.set(WEAPONSET_VETERAN, 1);
        assert_eq!(finder.find_best_info(&c.sets, &flags), Some(1));
        flags.set(WEAPONSET_ELITE, 1);
        assert_eq!(finder.find_best_info(&c.sets, &flags), Some(2));
    }

    #[test]
    fn clear_resets_everything_but_the_template() {
        let (c, r) = load(b"Set\n Weapon = PRIMARY Rifle\n AutoChooseSources = PRIMARY FROM_AI\n ShareWeaponReloadTime = Yes\nEnd\n");
        r.unwrap();
        let mut s = c.sets[0].clone();
        s.clear();
        assert!(!s.has_any_weapons() && !s.is_shared_reload_time());
        assert_eq!(s.get_nth_command_source_mask(0), 0xffff_ffff);
        assert_eq!(s.thing_template(), Some(9));
        assert!(WeaponTemplateSet::new().thing_template().is_none());
    }
}
