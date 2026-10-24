// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/DamageFX.h
//   GeneralsMD/Code/GameEngine/Source/Common/DamageFX.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIDamageFX.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp (parseFXList: the
//   token handling only)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DamageFX` (how an object reacts, in sound and picture, to a type of
//! damage, per veterancy level and for "minor" and "major" damage),
//! `DamageFXStore` (every `DamageFX` read from INI, by name key) and the
//! `DamageFX` INI block handler.
//!
//! # Cut point: the `FXList`s are names
//!
//! Upstream stores `const FXList*` and resolves it while parsing
//! (`INI::parseFXList`: `TheFXListStore->findFXList(token)`). The client owns
//! the `FXList`s, so here a parsed `FXList` is the **name** as written, and
//! `None` (any case) is no effect, as `findFXList` returns `NULL` for it.
//! Nothing here resolves a name. (Upstream's `findFXList` also calls
//! `NAMEKEY(name)`, which assigns a name key to a name it has not seen; the
//! client side that resolves the name does that, not this module.)
//! `doDamageFX`, which runs the effect, belongs to the client.
//!
//! # Translation choices
//!
//! - `DamageFXStore::m_dfxmap` is a `hash_map`; here a `BTreeMap`. Nothing
//!   iterates it upstream.
//! - `getDamageFXList` and `getDamageFXThrottleTime` take the veterancy of
//!   the source `Object` (`LEVEL_REGULAR` for none). The Rust takes the
//!   [`VeterancyLevel`], and the caller (a later layer) reads it off the
//!   object: [`DamageFx::get_damage_fx_list`],
//!   [`DamageFx::get_damage_fx_throttle_time`].
//!
//! # Where upstream is undefined
//!
//! - A parser that is given a damage name that is not one of
//!   [`DAMAGE_TYPE_NAMES`] (and not `Default`) indexes `m_dfx[-1]` once the
//!   value has been read: the Rust returns
//!   [`Undefined::DamageTypeOutOfRange`] at that point (after the value's
//!   own errors, which upstream throws first).
//! - The getters with a damage type outside `[0, DAMAGE_NUM_TYPES)`:
//!   [`DamageTypeOutOfRange`].
//!
//! # Left out: methods that take an `Object`
//!
//! `doDamageFX(t, damageAmount, source, victim)` (calls the client's
//! `FXList::doFXObj`), and the `Object` forms of `getDamageFXList` and
//! `getDamageFXThrottleTime` (`source->getVeterancyLevel()`); see above for
//! the level-taking forms that stand in for them.

use std::collections::BTreeMap;

use crate::common::game_common::{VeterancyLevel, LEVEL_COUNT, VETERANCY_NAMES};
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, Undefined, UserData};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::logic::armor::DamageTypeOutOfRange;
use crate::logic::damage::{DamageTypeFlags, DAMAGE_NUM_TYPES};

/// `LEVEL_COUNT` as an array length.
const LEVELS: usize = LEVEL_COUNT as usize;

/// The name of an `FXList`, or none (see the module docs).
pub type FxListName = Option<Vec<u8>>;

/// `DamageFX::DFX`: the effects for one damage type at one veterancy level.
#[derive(Clone, Debug, PartialEq)]
pub struct Dfx {
    /// `m_amountForMajorFX`: "if damage done is >= this, use major fx".
    pub amount_for_major_fx: f32,
    /// `m_majorDamageFXList`.
    pub major_damage_fx_list: FxListName,
    /// `m_minorDamageFXList`.
    pub minor_damage_fx_list: FxListName,
    /// `m_damageFXThrottleTime`.
    pub damage_fx_throttle_time: u32,
}

impl Default for Dfx {
    /// `DFX()`: `clear()`.
    fn default() -> Self {
        Dfx { amount_for_major_fx: 0.0, major_damage_fx_list: None, minor_damage_fx_list: None, damage_fx_throttle_time: 0 }
    }
}

/// `class DamageFX`: shared between units; read-only once loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct DamageFx {
    /// `m_dfx[DAMAGE_NUM_TYPES][LEVEL_COUNT]`.
    dfx: [[Dfx; LEVELS]; DAMAGE_NUM_TYPES],
}

impl Default for DamageFx {
    /// `DamageFX()` leaves the array uninitialised ("not necessary", the
    /// store always `clear()`s); this is the cleared state.
    fn default() -> Self {
        DamageFx { dfx: std::array::from_fn(|_| std::array::from_fn(|_| Dfx::default())) }
    }
}

/// What `parseCommonStuff` found: the veterancy levels and damage types a
/// line applies to (`damage_first` is -1 for an unknown name).
struct Span {
    vet_first: usize,
    vet_last: usize,
    damage_first: i32,
    damage_last: i32,
}

impl DamageFx {
    /// `DamageFX()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// `m_dfx[t][level]`.
    pub fn dfx(&self, t: i32, level: VeterancyLevel) -> Result<&Dfx, DamageTypeOutOfRange> {
        if t < 0 || t as usize >= DAMAGE_NUM_TYPES {
            return Err(DamageTypeOutOfRange);
        }
        Ok(&self.dfx[t as usize][level as usize])
    }

    // --- p1w13-sonnet ---
    /// Replaces `m_dfx[t][level]` (what the parsers leave for the same values), for a program that builds a `DamageFX` without INI text. An error for a
    /// damage type outside the table.
    pub fn set_dfx(&mut self, t: i32, level: VeterancyLevel, dfx: Dfx) -> Result<(), DamageTypeOutOfRange> {
        if t < 0 || t as usize >= DAMAGE_NUM_TYPES {
            return Err(DamageTypeOutOfRange);
        }
        self.dfx[t as usize][level as usize] = dfx;
        Ok(())
    }
    // --- end p1w13-sonnet ---

    /// `getDamageFXThrottleTime(t, source)`, for the source's veterancy
    /// level (`LEVEL_REGULAR` when there is no source).
    pub fn get_damage_fx_throttle_time(&self, t: i32, level: VeterancyLevel) -> Result<u32, DamageTypeOutOfRange> {
        Ok(self.dfx(t, level)?.damage_fx_throttle_time)
    }

    /// `getDamageFXList(t, damageAmount, source)`, for the source's
    /// veterancy level. "If damage is zero, never do damage fx" (the
    /// battleship's faux weapon); otherwise the major effect from the
    /// threshold up, the minor one below it (a NaN amount is below).
    pub fn get_damage_fx_list(&self, t: i32, damage_amount: f32, level: VeterancyLevel) -> Result<Option<&[u8]>, DamageTypeOutOfRange> {
        if damage_amount == 0.0 {
            return Ok(None);
        }
        let dfx = self.dfx(t, level)?;
        let fx = if damage_amount >= dfx.amount_for_major_fx { &dfx.major_damage_fx_list } else { &dfx.minor_damage_fx_list };
        Ok(fx.as_deref())
    }

    /// `parseCommonStuff`: with `names` (`TheVeterancyNames`), a veterancy
    /// token and then a damage type token; without, every level.
    fn parse_common_stuff(ini: &mut Ini<'_>, names: &[&str]) -> IniResult<Span> {
        let (vet_first, vet_last) = if names.is_empty() {
            (0, LEVELS - 1)
        } else {
            let v = Ini::scan_index_list(&ini.get_next_token(None)?, names)? as usize;
            (v, v)
        };
        let damage_name = ini.get_next_token(None)?;
        let (damage_first, damage_last) = if damage_name.eq_ignore_ascii_case(b"Default") {
            (0, DAMAGE_NUM_TYPES as i32 - 1)
        } else {
            let d = DamageTypeFlags::get_single_bit_from_name(&damage_name);
            (d, d)
        };
        Ok(Span { vet_first, vet_last, damage_first, damage_last })
    }

    /// The loops over the span of a parser: `m_dfx[dt][v]` for each.
    fn for_span(&mut self, span: &Span, mut f: impl FnMut(&mut Dfx)) -> IniResult<()> {
        if span.damage_first < 0 {
            return Err(IniError::undefined(Undefined::DamageTypeOutOfRange));
        }
        for dt in span.damage_first..=span.damage_last {
            for v in span.vet_first..=span.vet_last {
                f(&mut self.dfx[dt as usize][v]);
            }
        }
        Ok(())
    }

    /// `parseAmount`: `AmountForMajorFX` / `VeterancyAmountForMajorFX`.
    fn parse_amount(ini: &mut Ini<'_>, this: &mut DamageFx, user_data: &UserData) -> IniResult<()> {
        let span = Self::parse_common_stuff(ini, user_data.names())?;
        let amt = Ini::scan_real(&ini.get_next_token(None)?)?;
        this.for_span(&span, |d| d.amount_for_major_fx = amt)
    }

    /// `INI::parseFXList`, as far as the token goes (see the module docs).
    fn parse_fx_list(ini: &mut Ini<'_>) -> IniResult<FxListName> {
        let token = ini.get_next_token(None)?;
        Ok(if token.eq_ignore_ascii_case(b"None") { None } else { Some(token) })
    }

    /// `parseMajorFXList`: `MajorFX` / `VeterancyMajorFX`.
    fn parse_major_fx_list(ini: &mut Ini<'_>, this: &mut DamageFx, user_data: &UserData) -> IniResult<()> {
        let span = Self::parse_common_stuff(ini, user_data.names())?;
        let fx = Self::parse_fx_list(ini)?;
        this.for_span(&span, |d| d.major_damage_fx_list = fx.clone())
    }

    /// `parseMinorFXList`: `MinorFX` / `VeterancyMinorFX`.
    fn parse_minor_fx_list(ini: &mut Ini<'_>, this: &mut DamageFx, user_data: &UserData) -> IniResult<()> {
        let span = Self::parse_common_stuff(ini, user_data.names())?;
        let fx = Self::parse_fx_list(ini)?;
        this.for_span(&span, |d| d.minor_damage_fx_list = fx.clone())
    }

    /// `parseTime`: `ThrottleTime` / `VeterancyThrottleTime`, a duration in
    /// frames.
    fn parse_time(ini: &mut Ini<'_>, this: &mut DamageFx, user_data: &UserData) -> IniResult<()> {
        let span = Self::parse_common_stuff(ini, user_data.names())?;
        let t = ini.parse_duration_unsigned_int()?;
        this.for_span(&span, |d| d.damage_fx_throttle_time = t)
    }
}

/// `DamageFX::getFieldParse()`'s `myFieldParse`.
static DAMAGE_FX_FIELDS: [FieldParse<DamageFx>; 9] = [
    FieldParse::new("AmountForMajorFX", DamageFx::parse_amount, UserData::None),
    FieldParse::new("MajorFX", DamageFx::parse_major_fx_list, UserData::None),
    FieldParse::new("MinorFX", DamageFx::parse_minor_fx_list, UserData::None),
    FieldParse::new("ThrottleTime", DamageFx::parse_time, UserData::None),
    FieldParse::new("VeterancyAmountForMajorFX", DamageFx::parse_amount, UserData::Names(&VETERANCY_NAMES)),
    FieldParse::new("VeterancyMajorFX", DamageFx::parse_major_fx_list, UserData::Names(&VETERANCY_NAMES)),
    FieldParse::new("VeterancyMinorFX", DamageFx::parse_minor_fx_list, UserData::Names(&VETERANCY_NAMES)),
    FieldParse::new("VeterancyThrottleTime", DamageFx::parse_time, UserData::Names(&VETERANCY_NAMES)),
    FieldParse::end(),
];

/// `class DamageFXStore` (`TheDamageFXStore`): "the 'store' used to hold all
/// the DamageFXs in existence".
#[derive(Clone, Debug, Default)]
pub struct DamageFxStore {
    /// `m_dfxmap` (a `hash_map` upstream, see the module docs).
    dfxmap: BTreeMap<NameKeyType, DamageFx>,
}

impl DamageFxStore {
    /// `DamageFXStore()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// The `DamageFX` of a key.
    pub fn get(&self, key: NameKeyType) -> Option<&DamageFx> {
        self.dfxmap.get(&key)
    }

    // --- p1w13-sonnet ---
    /// Puts `dfx` in the store under `key` (what `parseDamageFXDefinition` does for a new name), for a program that builds a store without INI text.
    pub fn insert(&mut self, key: NameKeyType, dfx: DamageFx) {
        self.dfxmap.insert(key, dfx);
    }
    // --- end p1w13-sonnet ---

    /// The number of `DamageFX`es.
    pub fn len(&self) -> usize {
        self.dfxmap.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.dfxmap.is_empty()
    }

    /// Every `DamageFX`, by key.
    pub fn damage_fxs(&self) -> impl Iterator<Item = (NameKeyType, &DamageFx)> {
        self.dfxmap.iter().map(|(k, d)| (*k, d))
    }

    /// `findDamageFX(name)`: `nameToKey(name)` (which assigns a key to a new
    /// name), and the `DamageFX` under it, or `None`.
    pub fn find_damage_fx(
        &self,
        names: &mut NameKeyGenerator,
        name: &[u8],
    ) -> Result<Option<&DamageFx>, crate::common::name_key_generator::NameKeyError> {
        let key = names.try_name_to_key(name)?;
        Ok(self.dfxmap.get(&key))
    }

    /// `init()`, `reset()` and `update()` do nothing upstream.
    pub fn init(&mut self) {}

    /// `DamageFXStore::parseDamageFXDefinition(ini)` (the `DamageFX` block,
    /// reached through `INI::parseDamageFXDefinition`): the name, then the
    /// fields until `End`. A known name is cleared and refilled; the entry
    /// stays in the store, as far as it was filled, if a line fails.
    pub fn parse_damage_fx_definition(&mut self, ini: &mut Ini<'_>, names: &mut NameKeyGenerator) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        let key = names
            .try_name_to_key(&name)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        let dfx = self.dfxmap.entry(key).or_default();
        dfx.clear();
        ini.init_from_ini(dfx, &DAMAGE_FX_FIELDS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniErrorKind, IniLoadType};
    use crate::logic::damage::{DAMAGE_EXPLOSION, DAMAGE_FLAME};

    struct Ctx {
        store: DamageFxStore,
        names: NameKeyGenerator,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store.parse_damage_fx_definition(ini, &mut c.names)
    }

    static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse { token: "DamageFX", parse: block }];

    fn load(text: &[u8]) -> (Ctx, IniResult<()>) {
        let mut c = Ctx { store: DamageFxStore::new(), names: NameKeyGenerator::new() };
        c.names.init();
        let r = Ini::load(b"d.ini", text, IniLoadType::Overwrite, None, &BLOCKS, &mut c);
        (c, r)
    }

    #[test]
    fn levels_default_and_selection() {
        let text = b"DamageFX Fx\n AmountForMajorFX = Default 10\n MajorFX = Default BigFX\n MinorFX = Default None\n \
            VeterancyMajorFX = ELITE FLAME EliteFX\n ThrottleTime = EXPLOSION 1000\nEnd\n";
        let (mut c, r) = load(text);
        r.unwrap();
        let fx = c.store.find_damage_fx(&mut c.names, b"Fx").unwrap().unwrap();
        // major from the threshold, minor (None) below it
        assert_eq!(fx.get_damage_fx_list(DAMAGE_EXPLOSION, 10.0, VeterancyLevel::Regular), Ok(Some(&b"BigFX"[..])));
        assert_eq!(fx.get_damage_fx_list(DAMAGE_EXPLOSION, 9.5, VeterancyLevel::Regular), Ok(None));
        // zero damage never has an effect
        assert_eq!(fx.get_damage_fx_list(DAMAGE_EXPLOSION, 0.0, VeterancyLevel::Regular), Ok(None));
        // the veterancy override
        assert_eq!(fx.get_damage_fx_list(DAMAGE_FLAME, 11.0, VeterancyLevel::Elite), Ok(Some(&b"EliteFX"[..])));
        assert_eq!(fx.get_damage_fx_list(DAMAGE_FLAME, 11.0, VeterancyLevel::Veteran), Ok(Some(&b"BigFX"[..])));
        assert_eq!(fx.get_damage_fx_throttle_time(DAMAGE_EXPLOSION, VeterancyLevel::Heroic), Ok(30));
        assert_eq!(fx.get_damage_fx_throttle_time(DAMAGE_FLAME, VeterancyLevel::Regular), Ok(0));
        assert_eq!(fx.get_damage_fx_list(99, 1.0, VeterancyLevel::Regular), Err(DamageTypeOutOfRange));
    }

    #[test]
    fn errors_and_undefined() {
        let (_, r) = load(b"DamageFX F\n MajorFX = NOPE Fx\nEnd\n");
        assert!(r.unwrap_err().is_undefined());
        // the value's own error comes first
        let (_, r) = load(b"DamageFX F\n AmountForMajorFX = NOPE x\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        let (_, r) = load(b"DamageFX F\n VeterancyMajorFX = BOGUS FLAME Fx\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        let (_, r) = load(b"DamageFX F\n Foo = 1\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::UnknownToken);
    }
}
