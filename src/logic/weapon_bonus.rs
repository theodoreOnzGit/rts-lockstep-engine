// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Weapon.h  (WeaponBonusConditionType,
//     TheWeaponBonusNames, WeaponBonus, TheWeaponBonusFieldNames, WeaponBonusSet)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp  (WeaponBonus::appendBonuses,
//     WeaponBonusSet::parseWeaponBonusSet, parseWeaponBonusSetPtr, appendBonuses)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WeaponBonus` (five multipliers) and `WeaponBonusSet` (one `WeaponBonus`
//! per bonus condition): the part of `Weapon.h` that `GlobalData` holds and
//! reads from INI (`WeaponBonus = <CONDITION> <FIELD> <percent>` in
//! `GameData`, and in a `Weapon` block).
//!
//! # Scope
//!
//! Only these classes. The rest of `Weapon.h`/`Weapon.cpp` (`WeaponTemplate`,
//! `WeaponStore`, `Weapon`) is a later layer, which should use this module
//! rather than a second copy. `WeaponTemplate::parseWeaponBonusSet` (the
//! `WeaponBonus =` line of a `Weapon` block, which fills the template's
//! `m_extraBonus`) is that layer's; it calls
//! [`WeaponBonusSet::parse_weapon_bonus_set`].
//!
//! # Translation choices
//!
//! - The bonus conditions are `i32` constants, and `WeaponBonusConditionFlags`
//!   keeps its `u32` type ([`crate::logic::weapon_types`]); bit `i` of the
//!   flags is condition `i`.
//! - `ALLOW_DEMORALIZE` is not defined upstream, so condition 7 is
//!   `DEMORALIZED_OBSOLETE`, as shipped.
//! - Upstream's `WeaponBonusSet` is a `MemoryPoolObject` that `GlobalData`
//!   holds by pointer; here `GlobalData` owns it by value.
//! - `appendBonuses(flags, bonus)` adds `m_bonus[i].m_field[f] - 1` for every
//!   set flag; the float arithmetic is the same single operations in the same
//!   order.

use crate::common::ini::{Ini, IniResult};

/// `WEAPONBONUSCONDITION_COUNT`.
pub const WEAPONBONUSCONDITION_COUNT: usize = 27;

/// `enum WeaponBonusConditionType`'s values, by name (`WEAPONBONUSCONDITION_`
/// prefix dropped): "these values are saved in save files, so you MUST NOT
/// REMOVE OR CHANGE existing values".
pub mod condition {
    /// `WEAPONBONUSCONDITION_INVALID`.
    pub const INVALID: i32 = -1;
    /// `WEAPONBONUSCONDITION_GARRISONED`.
    pub const GARRISONED: i32 = 0;
    /// `WEAPONBONUSCONDITION_HORDE`.
    pub const HORDE: i32 = 1;
    /// `WEAPONBONUSCONDITION_CONTINUOUS_FIRE_MEAN`.
    pub const CONTINUOUS_FIRE_MEAN: i32 = 2;
    /// `WEAPONBONUSCONDITION_CONTINUOUS_FIRE_FAST`.
    pub const CONTINUOUS_FIRE_FAST: i32 = 3;
    /// `WEAPONBONUSCONDITION_NATIONALISM`.
    pub const NATIONALISM: i32 = 4;
    /// `WEAPONBONUSCONDITION_PLAYER_UPGRADE`.
    pub const PLAYER_UPGRADE: i32 = 5;
    /// `WEAPONBONUSCONDITION_DRONE_SPOTTING`.
    pub const DRONE_SPOTTING: i32 = 6;
    /// `WEAPONBONUSCONDITION_DEMORALIZED_OBSOLETE` (`ALLOW_DEMORALIZE` is not defined).
    pub const DEMORALIZED_OBSOLETE: i32 = 7;
    /// `WEAPONBONUSCONDITION_ENTHUSIASTIC`.
    pub const ENTHUSIASTIC: i32 = 8;
    /// `WEAPONBONUSCONDITION_VETERAN`.
    pub const VETERAN: i32 = 9;
    /// `WEAPONBONUSCONDITION_ELITE`.
    pub const ELITE: i32 = 10;
    /// `WEAPONBONUSCONDITION_HERO`.
    pub const HERO: i32 = 11;
    /// `WEAPONBONUSCONDITION_BATTLEPLAN_BOMBARDMENT`.
    pub const BATTLEPLAN_BOMBARDMENT: i32 = 12;
    /// `WEAPONBONUSCONDITION_BATTLEPLAN_HOLDTHELINE`.
    pub const BATTLEPLAN_HOLDTHELINE: i32 = 13;
    /// `WEAPONBONUSCONDITION_BATTLEPLAN_SEARCHANDDESTROY`.
    pub const BATTLEPLAN_SEARCHANDDESTROY: i32 = 14;
    /// `WEAPONBONUSCONDITION_SUBLIMINAL`.
    pub const SUBLIMINAL: i32 = 15;
    /// `WEAPONBONUSCONDITION_SOLO_HUMAN_EASY`.
    pub const SOLO_HUMAN_EASY: i32 = 16;
    /// `WEAPONBONUSCONDITION_SOLO_HUMAN_NORMAL`.
    pub const SOLO_HUMAN_NORMAL: i32 = 17;
    /// `WEAPONBONUSCONDITION_SOLO_HUMAN_HARD`.
    pub const SOLO_HUMAN_HARD: i32 = 18;
    /// `WEAPONBONUSCONDITION_SOLO_AI_EASY`.
    pub const SOLO_AI_EASY: i32 = 19;
    /// `WEAPONBONUSCONDITION_SOLO_AI_NORMAL`.
    pub const SOLO_AI_NORMAL: i32 = 20;
    /// `WEAPONBONUSCONDITION_SOLO_AI_HARD`.
    pub const SOLO_AI_HARD: i32 = 21;
    /// `WEAPONBONUSCONDITION_TARGET_FAERIE_FIRE`.
    pub const TARGET_FAERIE_FIRE: i32 = 22;
    /// `WEAPONBONUSCONDITION_FANATICISM`.
    pub const FANATICISM: i32 = 23;
    /// `WEAPONBONUSCONDITION_FRENZY_ONE`.
    pub const FRENZY_ONE: i32 = 24;
    /// `WEAPONBONUSCONDITION_FRENZY_TWO`.
    pub const FRENZY_TWO: i32 = 25;
    /// `WEAPONBONUSCONDITION_FRENZY_THREE`.
    pub const FRENZY_THREE: i32 = 26;
}

/// `TheWeaponBonusNames[]` (the right-hand sides of `WeaponBonus = ...`),
/// without the `NULL` terminator.
pub const WEAPON_BONUS_NAMES: [&str; WEAPONBONUSCONDITION_COUNT] = [
    "GARRISONED",
    "HORDE",
    "CONTINUOUS_FIRE_MEAN",
    "CONTINUOUS_FIRE_FAST",
    "NATIONALISM",
    "PLAYER_UPGRADE",
    "DRONE_SPOTTING",
    "DEMORALIZED_OBSOLETE",
    "ENTHUSIASTIC",
    "VETERAN",
    "ELITE",
    "HERO",
    "BATTLEPLAN_BOMBARDMENT",
    "BATTLEPLAN_HOLDTHELINE",
    "BATTLEPLAN_SEARCHANDDESTROY",
    "SUBLIMINAL",
    "SOLO_HUMAN_EASY",
    "SOLO_HUMAN_NORMAL",
    "SOLO_HUMAN_HARD",
    "SOLO_AI_EASY",
    "SOLO_AI_NORMAL",
    "SOLO_AI_HARD",
    "TARGET_FAERIE_FIRE",
    "FANATICISM",
    "FRENZY_ONE",
    "FRENZY_TWO",
    "FRENZY_THREE",
];

/// `WeaponBonus::FIELD_COUNT`.
pub const WEAPON_BONUS_FIELD_COUNT: usize = 5;

/// `WeaponBonus::Field`: `DAMAGE`.
pub const FIELD_DAMAGE: usize = 0;
/// `WeaponBonus::RADIUS`.
pub const FIELD_RADIUS: usize = 1;
/// `WeaponBonus::RANGE`.
pub const FIELD_RANGE: usize = 2;
/// `WeaponBonus::RATE_OF_FIRE`.
pub const FIELD_RATE_OF_FIRE: usize = 3;
/// `WeaponBonus::PRE_ATTACK`.
pub const FIELD_PRE_ATTACK: usize = 4;

/// `TheWeaponBonusFieldNames[]`, without the `NULL` terminator.
pub const WEAPON_BONUS_FIELD_NAMES: [&str; WEAPON_BONUS_FIELD_COUNT] = ["DAMAGE", "RADIUS", "RANGE", "RATE_OF_FIRE", "PRE_ATTACK"];

/// `class WeaponBonus`: a multiplier for each of damage, radius, range, rate
/// of fire and pre-attack delay. 1 is no bonus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponBonus {
    /// `m_field`.
    field: [f32; WEAPON_BONUS_FIELD_COUNT],
}

impl Default for WeaponBonus {
    /// `WeaponBonus()`: `clear()`.
    fn default() -> Self {
        WeaponBonus { field: [1.0; WEAPON_BONUS_FIELD_COUNT] }
    }
}

impl WeaponBonus {
    /// `WeaponBonus()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `clear()`: every field back to 1.
    pub fn clear(&mut self) {
        self.field = [1.0; WEAPON_BONUS_FIELD_COUNT];
    }

    /// `getField(f)`; `f` is one of the `FIELD_*` constants.
    pub fn get_field(&self, f: usize) -> f32 {
        self.field[f]
    }

    /// `setField(f, v)`.
    pub fn set_field(&mut self, f: usize, v: f32) {
        self.field[f] = v;
    }

    /// `appendBonuses(bonus)`: adds this bonus, as a difference from 1, to
    /// `bonus`.
    pub fn append_bonuses(&self, bonus: &mut WeaponBonus) {
        for f in 0..WEAPON_BONUS_FIELD_COUNT {
            bonus.field[f] += self.field[f] - 1.0;
        }
    }
}

/// `class WeaponBonusSet`: the bonus of every condition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponBonusSet {
    /// `m_bonus`.
    bonus: [WeaponBonus; WEAPONBONUSCONDITION_COUNT],
}

impl Default for WeaponBonusSet {
    /// `WeaponBonusSet()`: every `WeaponBonus` constructed, so all 1.
    fn default() -> Self {
        WeaponBonusSet { bonus: [WeaponBonus::new(); WEAPONBONUSCONDITION_COUNT] }
    }
}

impl WeaponBonusSet {
    /// `WeaponBonusSet()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `m_bonus[condition]`, for inspection.
    pub fn bonus(&self, condition: usize) -> &WeaponBonus {
        &self.bonus[condition]
    }

    /// `appendBonuses(flags, bonus)`: for every condition whose bit is set in
    /// `flags`, appends that condition's bonus to `bonus`.
    pub fn append_bonuses(&self, flags: u32, bonus: &mut WeaponBonus) {
        if flags == 0 {
            return; // my, that was easy
        }
        for i in 0..WEAPONBONUSCONDITION_COUNT {
            if (flags & (1u32 << i)) == 0 {
                continue;
            }
            self.bonus[i].append_bonuses(bonus);
        }
    }

    /// `parseWeaponBonusSet(ini)`: `<CONDITION> <FIELD> <percent>` (three
    /// tokens, read before any is stored): sets that condition's field.
    pub fn parse_weapon_bonus_set(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        let wb = Ini::scan_index_list(&ini.get_next_token(None)?, &WEAPON_BONUS_NAMES)?;
        let wf = Ini::scan_index_list(&ini.get_next_token(None)?, &WEAPON_BONUS_FIELD_NAMES)?;
        let pct = Ini::scan_percent_to_real(&ini.get_next_token(None)?)?;
        self.bonus[wb as usize].set_field(wf as usize, pct);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, FieldParse, IniErrorKind, IniLoadType, UserData};

    fn parse_set(ini: &mut Ini<'_>, set: &mut WeaponBonusSet, _ud: &UserData) -> IniResult<()> {
        set.parse_weapon_bonus_set(ini)
    }
    static SET_FIELDS: [FieldParse<WeaponBonusSet>; 2] = [FieldParse::new("WeaponBonus", parse_set, UserData::None), FieldParse::end()];

    fn block(ini: &mut Ini<'_>, set: &mut WeaponBonusSet) -> IniResult<()> {
        ini.init_from_ini(set, &SET_FIELDS)
    }
    static BLOCKS: [BlockParse<WeaponBonusSet>; 1] = [BlockParse { token: "Bonus", parse: block }];

    fn load(text: &[u8]) -> (WeaponBonusSet, IniResult<()>) {
        let mut set = WeaponBonusSet::new();
        let r = Ini::load(b"w.ini", text, IniLoadType::Overwrite, None, &BLOCKS, &mut set);
        (set, r)
    }

    #[test]
    fn names_match_the_condition_constants() {
        assert_eq!(WEAPON_BONUS_NAMES[condition::GARRISONED as usize], "GARRISONED");
        assert_eq!(WEAPON_BONUS_NAMES[condition::DEMORALIZED_OBSOLETE as usize], "DEMORALIZED_OBSOLETE");
        assert_eq!(WEAPON_BONUS_NAMES[condition::FANATICISM as usize], "FANATICISM");
        assert_eq!(WEAPON_BONUS_NAMES[condition::FRENZY_THREE as usize], "FRENZY_THREE");
        assert_eq!(condition::FRENZY_THREE as usize + 1, WEAPONBONUSCONDITION_COUNT);
    }

    #[test]
    fn parse_sets_one_field_and_append_adds_the_difference() {
        let (set, r) = load(b"Bonus X\n WeaponBonus = HORDE DAMAGE 125%\n WeaponBonus = hero rate_of_fire 150%\n WeaponBonus = HORDE RANGE 90%\nEnd\n");
        r.unwrap();
        assert_eq!(set.bonus(condition::HORDE as usize).get_field(FIELD_DAMAGE), 1.25);
        assert_eq!(set.bonus(condition::HORDE as usize).get_field(FIELD_RANGE), 0.9);
        assert_eq!(set.bonus(condition::HORDE as usize).get_field(FIELD_RADIUS), 1.0);
        assert_eq!(set.bonus(condition::HERO as usize).get_field(FIELD_RATE_OF_FIRE), 1.5);
        let mut total = WeaponBonus::new();
        set.append_bonuses(0, &mut total);
        assert_eq!(total, WeaponBonus::new());
        set.append_bonuses((1 << condition::HORDE) | (1 << condition::HERO), &mut total);
        assert_eq!(total.get_field(FIELD_DAMAGE), 1.25);
        assert_eq!(total.get_field(FIELD_RATE_OF_FIRE), 1.5);
        assert_eq!(total.get_field(FIELD_RANGE), 0.9);
        total.clear();
        assert_eq!(total, WeaponBonus::default());
    }

    #[test]
    fn bad_names_and_missing_tokens_throw_before_storing() {
        let (set, r) = load(b"Bonus X\n WeaponBonus = NOPE DAMAGE 125%\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(set, WeaponBonusSet::new());
        let (_, r) = load(b"Bonus X\n WeaponBonus = HORDE NOPE 125%\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        let (set, r) = load(b"Bonus X\n WeaponBonus = HORDE DAMAGE\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(set, WeaponBonusSet::new());
    }
}
