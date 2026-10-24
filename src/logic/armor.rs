// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Armor.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Armor.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, March 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ArmorTemplate` (the damage multipliers of one kind of armor), `Armor`
//! (a handle to a template) and `ArmorStore` (every template read from INI,
//! by name key), with the `Armor` INI block handler.
//!
//! # Translation choices
//!
//! - `ArmorStore::m_armorTemplates` is a `hash_map` keyed by `NameKeyType`;
//!   here a `BTreeMap`. Nothing iterates it upstream, so the order is not
//!   observable.
//! - `Armor` holds a `const ArmorTemplate*` upstream. Here it holds the
//!   template's name key (an index into the store), and
//!   [`Armor::adjust_damage`] takes the store. The store never removes a
//!   template (INI refills it in place, and `Armor` sees the new values, as
//!   the pointer did); a key the store does not have is treated as no armor.
//! - `findArmorTemplate` calls `nameToKey`, which **assigns a key to a name
//!   it has not seen**, and key values are part of the lockstep state. So
//!   the lookups take the [`NameKeyGenerator`].
//! - The field parsers `INI::parseArmorTemplate` (for `ThingTemplate`'s and
//!   `ArmorUpgrade`'s `Armor =` lines) is not translated: it belongs with its
//!   users.
//!
//! # Where upstream is undefined
//!
//! - `parseArmorCoefficients` with a damage name that is not one of
//!   [`DAMAGE_TYPE_NAMES`] (and not `Default`) writes
//!   `m_damageCoefficient[-1]`: the Rust returns
//!   [`Undefined::DamageTypeOutOfRange`].
//! - `adjustDamage(t, ...)` with `t` outside `[0, DAMAGE_NUM_TYPES)` (other
//!   than the two unresistable types, which return before the lookup) reads
//!   out of bounds: [`DamageTypeOutOfRange`].
//!
//! # Left out: methods that take an `Object` or a `Player`
//!
//! None of `Armor.h`/`Armor.cpp` takes one. (`ArmorTemplateSet` and the
//! `Object`'s armor-set selection live in `ArmorSet.h`, `Object.cpp` and
//! `ActiveBody.cpp`, later layers.)

use std::collections::BTreeMap;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, Undefined, UserData};
use crate::common::name_key_generator::{NameKeyError, NameKeyGenerator, NameKeyType};
use crate::logic::damage::DamageTypeFlags;
use crate::logic::damage::{
    DamageType, DAMAGE_NUM_TYPES, DAMAGE_SUBDUAL_UNRESISTABLE, DAMAGE_UNRESISTABLE,
};

/// A `DamageType` outside `[0, DAMAGE_NUM_TYPES)` was used as an index:
/// undefined upstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageTypeOutOfRange;

impl std::fmt::Display for DamageTypeOutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DamageType out of range (an out-of-bounds index upstream)")
    }
}

impl std::error::Error for DamageTypeOutOfRange {}

/// `class ArmorTemplate`: a modifier to damage taken, per damage type, "to
/// simulate different materials, and to help make game balance easier to
/// adjust".
#[derive(Clone, Debug, PartialEq)]
pub struct ArmorTemplate {
    /// `m_damageCoefficient`.
    damage_coefficient: [f32; DAMAGE_NUM_TYPES],
}

impl Default for ArmorTemplate {
    /// `ArmorTemplate()`: `clear()`.
    fn default() -> Self {
        ArmorTemplate {
            damage_coefficient: [1.0; DAMAGE_NUM_TYPES],
        }
    }
}

impl ArmorTemplate {
    /// `ArmorTemplate()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `clear()`: every coefficient back to 1.
    pub fn clear(&mut self) {
        self.damage_coefficient = [1.0; DAMAGE_NUM_TYPES];
    }

    /// `m_damageCoefficient`, for inspection.
    pub fn coefficients(&self) -> &[f32; DAMAGE_NUM_TYPES] {
        &self.damage_coefficient
    }

    // --- p1w13-sonnet ---
    /// An armor template with these coefficients (what `parseArmorCoefficients` leaves for the same values: a program that builds a store without INI text,
    /// as the differential test of `ActiveBody` does, gives the coefficients themselves).
    pub fn from_coefficients(damage_coefficient: [f32; DAMAGE_NUM_TYPES]) -> Self {
        ArmorTemplate { damage_coefficient }
    }
    // --- end p1w13-sonnet ---

    /// `adjustDamage(t, damage)`: given a damage type and amount, "return
    /// the amount that should be dealt". The two unresistable types pass
    /// through; otherwise the amount is multiplied by the type's
    /// coefficient and floored at 0 (a NaN stays NaN, as in C++).
    pub fn adjust_damage(&self, t: DamageType, damage: f32) -> Result<f32, DamageTypeOutOfRange> {
        if t == DAMAGE_UNRESISTABLE || t == DAMAGE_SUBDUAL_UNRESISTABLE {
            return Ok(damage);
        }
        if t < 0 || t as usize >= DAMAGE_NUM_TYPES {
            return Err(DamageTypeOutOfRange);
        }
        let mut damage = damage * self.damage_coefficient[t as usize];
        if damage < 0.0 {
            damage = 0.0;
        }
        Ok(damage)
    }

    /// `parseArmorCoefficients`: `Armor = <DamageType | Default> <percent>`.
    /// Both tokens are read before either is checked. `Default` sets every
    /// coefficient.
    fn parse_armor_coefficients(
        ini: &mut Ini<'_>,
        this: &mut ArmorTemplate,
        _user_data: &UserData,
    ) -> IniResult<()> {
        let damage_name = ini.get_next_token(None)?;
        let pct = Ini::scan_percent_to_real(&ini.get_next_token(None)?)?;
        if damage_name.eq_ignore_ascii_case(b"Default") {
            this.damage_coefficient = [pct; DAMAGE_NUM_TYPES];
            return Ok(());
        }
        let dt = DamageTypeFlags::get_single_bit_from_name(&damage_name);
        if dt < 0 {
            // m_damageCoefficient[-1] = pct
            return Err(IniError::undefined(Undefined::DamageTypeOutOfRange));
        }
        this.damage_coefficient[dt as usize] = pct;
        Ok(())
    }
}

/// `ArmorStore::parseArmorDefinition`'s `myFieldParse`.
static ARMOR_FIELDS: [FieldParse<ArmorTemplate>; 2] = [
    FieldParse::new(
        "Armor",
        ArmorTemplate::parse_armor_coefficients,
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class Armor`: a handle to the kind of armor something has. `None` is no
/// armor (damage passes through).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Armor {
    /// `m_template`, as the template's name key.
    template: Option<NameKeyType>,
}

impl Armor {
    /// `Armor(tmpl)`.
    pub fn new(template: Option<NameKeyType>) -> Self {
        Armor { template }
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.template = None;
    }

    /// The template's key.
    pub fn template(&self) -> Option<NameKeyType> {
        self.template
    }

    /// `adjustDamage(t, damage)`: through the template if there is one in
    /// `store`, else `damage` unchanged.
    pub fn adjust_damage(
        &self,
        store: &ArmorStore,
        t: DamageType,
        damage: f32,
    ) -> Result<f32, DamageTypeOutOfRange> {
        match self.template.and_then(|k| store.get(k)) {
            Some(tmpl) => tmpl.adjust_damage(t, damage),
            None => Ok(damage),
        }
    }
}

/// `class ArmorStore` (`TheArmorStore`): "a convenient place for us to store
/// each Armor we read from INI together in a list".
#[derive(Clone, Debug, Default)]
pub struct ArmorStore {
    /// `m_armorTemplates` (a `hash_map` upstream, see the module docs).
    armor_templates: BTreeMap<NameKeyType, ArmorTemplate>,
}

impl ArmorStore {
    /// `ArmorStore()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// The template of a key.
    pub fn get(&self, key: NameKeyType) -> Option<&ArmorTemplate> {
        self.armor_templates.get(&key)
    }

    /// The number of templates.
    pub fn len(&self) -> usize {
        self.armor_templates.len()
    }

    /// Whether there are no templates.
    pub fn is_empty(&self) -> bool {
        self.armor_templates.is_empty()
    }

    /// Every template, by key.
    pub fn templates(&self) -> impl Iterator<Item = (NameKeyType, &ArmorTemplate)> {
        self.armor_templates.iter().map(|(k, t)| (*k, t))
    }

    /// `findArmorTemplate(name)`'s key: `nameToKey(name)` (which assigns a
    /// key to a new name), and whether the store has a template under it.
    pub fn find_armor_template_key(
        &self,
        names: &mut NameKeyGenerator,
        name: &[u8],
    ) -> Result<Option<NameKeyType>, NameKeyError> {
        let key = names.try_name_to_key(name)?;
        Ok(self.armor_templates.contains_key(&key).then_some(key))
    }

    /// `findArmorTemplate(name)`: "If no such Armor exists, return null."
    pub fn find_armor_template(
        &self,
        names: &mut NameKeyGenerator,
        name: &[u8],
    ) -> Result<Option<&ArmorTemplate>, NameKeyError> {
        let key = names.try_name_to_key(name)?;
        Ok(self.armor_templates.get(&key))
    }

    // --- p1w13-sonnet ---
    /// Puts `template` in the store under `key` (what `parseArmorDefinition` does for a new name), for a program that builds a store without INI text.
    pub fn insert_template(&mut self, key: NameKeyType, template: ArmorTemplate) {
        self.armor_templates.insert(key, template);
    }
    // --- end p1w13-sonnet ---

    /// `makeArmor(tmpl)`.
    pub fn make_armor(&self, template: Option<NameKeyType>) -> Armor {
        Armor::new(template)
    }

    /// `ArmorStore::parseArmorDefinition(ini)` (the `Armor` block, reached
    /// through `INI::parseArmorDefinition`): the template's name, then its
    /// `Armor =` lines until `End`. A new name gets a template, and a known
    /// one is cleared and refilled; the template stays in the store, as
    /// far as it was filled, if a line fails.
    pub fn parse_armor_definition(
        &mut self,
        ini: &mut Ini<'_>,
        names: &mut NameKeyGenerator,
    ) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        let key = names
            .try_name_to_key(&name)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        let armor_tmpl = self.armor_templates.entry(key).or_default();
        armor_tmpl.clear();
        ini.init_from_ini(armor_tmpl, &ARMOR_FIELDS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniErrorKind, IniLoadType};
    use crate::logic::damage::{DAMAGE_EXPLOSION, DAMAGE_FLAME, DAMAGE_SMALL_ARMS};

    struct Ctx {
        store: ArmorStore,
        names: NameKeyGenerator,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store.parse_armor_definition(ini, &mut c.names)
    }

    static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse {
        token: "Armor",
        parse: block,
    }];

    fn load(text: &[u8]) -> (Ctx, IniResult<()>) {
        let mut c = Ctx {
            store: ArmorStore::new(),
            names: NameKeyGenerator::new(),
        };
        c.names.init();
        let r = Ini::load(
            b"a.ini",
            text,
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut c,
        );
        (c, r)
    }

    #[test]
    fn default_then_override_and_adjust() {
        let (mut c, r) = load(b"Armor Tank\n Armor = Default 100%\n Armor = FLAME 50%\n Armor = SMALL_ARMS 0%\n Armor = UNRESISTABLE 0%\nEnd\n");
        r.unwrap();
        let tmpl = c
            .store
            .find_armor_template(&mut c.names, b"Tank")
            .unwrap()
            .unwrap()
            .clone();
        assert_eq!(tmpl.adjust_damage(DAMAGE_EXPLOSION, 10.0), Ok(10.0));
        assert_eq!(tmpl.adjust_damage(DAMAGE_FLAME, 10.0), Ok(5.0));
        assert_eq!(tmpl.adjust_damage(DAMAGE_SMALL_ARMS, 10.0), Ok(0.0));
        // the unresistable types skip the table
        assert_eq!(tmpl.adjust_damage(DAMAGE_UNRESISTABLE, 10.0), Ok(10.0));
        assert_eq!(
            tmpl.adjust_damage(DAMAGE_SUBDUAL_UNRESISTABLE, -3.0),
            Ok(-3.0)
        );
        // a negative result is floored at 0
        assert_eq!(tmpl.adjust_damage(DAMAGE_EXPLOSION, -3.0), Ok(0.0));
        assert_eq!(tmpl.adjust_damage(-1, 1.0), Err(DamageTypeOutOfRange));
        assert_eq!(
            tmpl.adjust_damage(DAMAGE_NUM_TYPES as i32, 1.0),
            Err(DamageTypeOutOfRange)
        );
        assert!(tmpl.adjust_damage(DAMAGE_FLAME, f32::NAN).unwrap().is_nan());
    }

    #[test]
    fn redefinition_clears_and_armor_handle_follows_the_store() {
        let (mut c, r) =
            load(b"Armor A\n Armor = FLAME 50%\nEnd\nArmor A\n Armor = CRUSH 25%\nEnd\n");
        r.unwrap();
        assert_eq!(c.store.len(), 1);
        let key = c.store.find_armor_template_key(&mut c.names, b"A").unwrap();
        let armor = c.store.make_armor(key);
        assert_eq!(armor.adjust_damage(&c.store, DAMAGE_FLAME, 8.0), Ok(8.0));
        assert_eq!(
            armor.adjust_damage(&c.store, crate::logic::damage::DAMAGE_CRUSH, 8.0),
            Ok(2.0)
        );
        assert_eq!(
            c.store.find_armor_template_key(&mut c.names, b"B").unwrap(),
            None
        );
        let mut none = armor;
        none.clear();
        assert_eq!(none.adjust_damage(&c.store, DAMAGE_FLAME, 8.0), Ok(8.0));
    }

    #[test]
    fn errors_and_undefined() {
        // an unknown damage name is an out-of-bounds write upstream
        let (c, r) = load(b"Armor A\n Armor = NOPE 50%\nEnd\n");
        assert!(r.unwrap_err().is_undefined());
        assert_eq!(c.store.len(), 1);
        // a bad percentage throws, wrapped with the line
        let (c, r) = load(b"Armor A\n Armor = FLAME x\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(c.store.len(), 1);
        // a missing percentage
        let (_, r) = load(b"Armor A\n Armor = FLAME\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        // an unknown field
        let (_, r) = load(b"Armor A\n Foo = 1\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::UnknownToken);
    }
}
