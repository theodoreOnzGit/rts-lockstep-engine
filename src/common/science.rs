// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Science.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Science.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp
//     (INI::parseScience, INI::parseScienceVector, INI::scanScience,
//      INI::parseAndTranslateLabel)
//   GeneralsMD/Code/GameEngine/Source/Common/System/Xfer.cpp
//     (Xfer::xferScienceType, Xfer::xferScienceVec)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, Colin Day, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ScienceType`, `ScienceVec`, `ScienceInfo` and `ScienceStore`
//! (`TheScienceStore`), with the `Science` INI block, the science lookups,
//! the INI field parsers `parseScience` and `parseScienceVector`, and
//! `xferScienceType` / `xferScienceVec`.
//!
//! A science is identified by the **name key** of its internal name
//! (`ScienceType` is that key cast to an `int`), so the lookups that upstream
//! makes through `TheNameKeyGenerator` take the [`NameKeyGenerator`]:
//! `NAMEKEY(name)` **assigns a key to a name it has not seen**, and key values
//! are part of the lockstep state.
//!
//! # Translation choices
//!
//! - `ScienceStore::m_sciences` is a `vector<ScienceInfo*>` of the first
//!   definition of each science; a later INI file (`INI_LOAD_CREATE_OVERRIDES`)
//!   hangs a new `ScienceInfo` off it as its next override, and every reader
//!   goes to the final override. Here the vector holds
//!   `Box<Overridable<ScienceInfo>>` ([`crate::common::overridable`]).
//! - `ScienceInfo::m_name` and `m_description` are `UnicodeString`s that
//!   `parseAndTranslateLabel` fills from `TheGameText->fetch(label)`. The game
//!   text is not a global here: the block handler takes a [`GameTextFetch`],
//!   and keeps the `UnicodeString` it returns. (`formats::csf::GameText` is
//!   the CSF reader; it does not format the `MISSING:` text of an unknown
//!   label, so it does not implement the trait.) An empty translation throws
//!   `INI_INVALID_DATA`, as upstream.
//! - `INI::parseScienceVector` and `INI::parseScience` validate each name
//!   against `TheScienceStore` (`friend_lookupScience`). The functions here
//!   take the validity as a predicate ([`parse_science_vector`]), because
//!   while a `Science` block is read the store is being filled; the handler
//!   gives it the set of sciences known when the block started, which is what
//!   upstream sees: the science being defined has `m_science ==
//!   SCIENCE_INVALID` until its fields are read (so naming itself fails),
//!   while an override starts from a copy that already has its key.
//!   [`ScienceStore::parse_science_vector`] and
//!   [`ScienceStore::parse_science`] are the same against a finished store.
//! - A block that fails half-way **stays in the store**, as upstream: a new
//!   science keeps the fields read so far and `m_science == SCIENCE_INVALID`
//!   (so `isValidScience(SCIENCE_INVALID)` is then true, and the lookups of
//!   `SCIENCE_INVALID` find it); an override stays linked.
//! - `m_rootSciences` is "calced at runtime": `addRootSciences` appends to
//!   the copy the override started from, so roots of an earlier definition
//!   that the new prerequisites no longer lead to are **kept**.
//!
//! # Where upstream is undefined
//!
//! - `addRootSciences` recurses through the prerequisites, so a prerequisite
//!   cycle (possible only through an override: a first definition cannot name
//!   a science that does not exist yet) recurses forever.
//!   [`Undefined::ScienceCycle`].
//! - A name key past `0x7fffffff` ([`Undefined::NameKeysExhausted`]; in
//!   [`ScienceXferError::NameKeysExhausted`] for the xfers).
//!
//! # Left out: methods that take a `Player` (layer 7)
//!
//! `ScienceStore::playerHasPrereqsForScience(player, st)`,
//! `playerHasRootPrereqsForScience(player, st)` and
//! `getPurchasableSciences(player, purchasable, potentiallyPurchasable)` call
//! `Player::hasScience`. The data they read is public here
//! ([`ScienceStore::find_science_info`], [`ScienceStore::infos`],
//! [`ScienceInfo::prereq_sciences`], [`ScienceInfo::root_sciences`]), so the
//! `Player` layer can write them. `class Player;` itself is not needed.

use std::collections::BTreeSet;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Undefined, UserData};
use crate::common::name_key_generator::{NameKeyError, NameKeyGenerator, NameKeyType};
use crate::common::overridable::Overridable;
use crate::common::unicode_string::UnicodeString;
use crate::common::xfer::{Xfer, XferMode, XferStatus, XferVersion};

/// `enum ScienceType`: the name key of a science's internal name, cast to an
/// `int`. [`SCIENCE_INVALID`] is -1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScienceType(pub i32);

/// `SCIENCE_INVALID`.
pub const SCIENCE_INVALID: ScienceType = ScienceType(-1);

impl ScienceType {
    /// `(ScienceType)nameKey`.
    pub fn from_name_key(key: NameKeyType) -> Self {
        ScienceType(key.0 as i32)
    }

    /// `(NameKeyType)science`.
    pub fn to_name_key(self) -> NameKeyType {
        NameKeyType(self.0 as u32)
    }
}

/// `typedef std::vector<ScienceType> ScienceVec`.
pub type ScienceVec = Vec<ScienceType>;

/// `TheGameText->fetch(label)`, as `INI::parseAndTranslateLabel` uses it: the
/// translated text of a label.
pub trait GameTextFetch {
    /// The text of `label` (upstream's `MISSING:` text for an unknown one).
    fn fetch(&self, label: &[u8]) -> UnicodeString;
}

/// `INI::parseAndTranslateLabel`: one token, translated; an empty translation
/// is `INI_INVALID_DATA`. The caller stores the result (upstream leaves the
/// field untouched when it throws).
pub fn parse_and_translate_label(ini: &mut Ini<'_>, text: &dyn GameTextFetch) -> IniResult<UnicodeString> {
    let token = ini.get_next_token(None)?;
    let translated = text.fetch(&token);
    if translated.is_empty() {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    let mut s = UnicodeString::new();
    s.set_unicode(&translated);
    Ok(s)
}

/// `NAMEKEY(name)` as a science: assigns a key to a new name.
fn name_to_science(names: &mut NameKeyGenerator, name: &[u8]) -> IniResult<ScienceType> {
    names
        .try_name_to_key(name)
        .map(ScienceType::from_name_key)
        .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))
}

/// `INI::scanScience(token)` = `friend_lookupScience`: the science named
/// `token`, `INI_INVALID_DATA` if `is_valid` says it is not known. The name
/// is given a key first either way.
pub fn scan_science(names: &mut NameKeyGenerator, is_valid: impl Fn(ScienceType) -> bool, token: &[u8]) -> IniResult<ScienceType> {
    let st = name_to_science(names, token)?;
    if !is_valid(st) {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    Ok(st)
}

/// `INI::parseScienceVector`: clear `store`, then read names until the end of
/// the line; `None` (any case) clears it again and stops. A name that is not a
/// known science throws, leaving the names read so far in `store`.
pub fn parse_science_vector(
    ini: &mut Ini<'_>,
    names: &mut NameKeyGenerator,
    is_valid: impl Fn(ScienceType) -> bool,
    store: &mut ScienceVec,
) -> IniResult<()> {
    store.clear();
    while let Some(token) = ini.get_next_token_or_null(None) {
        if token.eq_ignore_ascii_case(b"None") {
            store.clear();
            return Ok(());
        }
        store.push(scan_science(names, &is_valid, &token)?);
    }
    Ok(())
}

/// `class ScienceInfo`: one science.
#[derive(Clone, Debug)]
pub struct ScienceInfo {
    /// `m_science`: `SCIENCE_INVALID` until the block that defines it has
    /// read its fields.
    science: ScienceType,
    /// `m_name`: the translated `DisplayName`.
    name: UnicodeString,
    /// `m_description`: the translated `Description`.
    description: UnicodeString,
    /// `m_rootSciences`: "calced at runtime, NOT read from INI".
    root_sciences: ScienceVec,
    /// `m_prereqSciences`.
    prereq_sciences: ScienceVec,
    /// `m_sciencePurchasePointCost`: 0 means "cannot be purchased".
    science_purchase_point_cost: i32,
    /// `m_grantable`.
    grantable: bool,
}

impl ScienceInfo {
    /// `ScienceInfo()`.
    fn new() -> Self {
        ScienceInfo {
            science: SCIENCE_INVALID,
            name: UnicodeString::new(),
            description: UnicodeString::new(),
            root_sciences: Vec::new(),
            prereq_sciences: Vec::new(),
            science_purchase_point_cost: 0,
            grantable: true,
        }
    }

    /// `m_science`.
    pub fn science(&self) -> ScienceType {
        self.science
    }
    /// `m_name`.
    pub fn name(&self) -> &UnicodeString {
        &self.name
    }
    /// `m_description`.
    pub fn description(&self) -> &UnicodeString {
        &self.description
    }
    /// `m_rootSciences`.
    pub fn root_sciences(&self) -> &[ScienceType] {
        &self.root_sciences
    }
    /// `m_prereqSciences`.
    pub fn prereq_sciences(&self) -> &[ScienceType] {
        &self.prereq_sciences
    }
    /// `m_sciencePurchasePointCost`.
    pub fn science_purchase_point_cost(&self) -> i32 {
        self.science_purchase_point_cost
    }
    /// `m_grantable`.
    pub fn grantable(&self) -> bool {
        self.grantable
    }
}

/// What the fields of a `Science` block fill: the `ScienceInfo`, and what the
/// field parsers read besides it.
struct ScienceFill<'a> {
    info: &'a mut ScienceInfo,
    names: &'a mut NameKeyGenerator,
    /// The sciences known when the block started.
    valid: &'a BTreeSet<ScienceType>,
    text: &'a dyn GameTextFetch,
}

fn f_prerequisite_sciences(ini: &mut Ini<'_>, w: &mut ScienceFill<'_>, _: &UserData) -> IniResult<()> {
    let valid = w.valid;
    parse_science_vector(ini, w.names, |s| valid.contains(&s), &mut w.info.prereq_sciences)
}
fn f_science_purchase_point_cost(ini: &mut Ini<'_>, w: &mut ScienceFill<'_>, _: &UserData) -> IniResult<()> {
    w.info.science_purchase_point_cost = ini.parse_int()?;
    Ok(())
}
fn f_is_grantable(ini: &mut Ini<'_>, w: &mut ScienceFill<'_>, _: &UserData) -> IniResult<()> {
    w.info.grantable = ini.parse_bool()?;
    Ok(())
}
fn f_display_name(ini: &mut Ini<'_>, w: &mut ScienceFill<'_>, _: &UserData) -> IniResult<()> {
    w.info.name = parse_and_translate_label(ini, w.text)?;
    Ok(())
}
fn f_description(ini: &mut Ini<'_>, w: &mut ScienceFill<'_>, _: &UserData) -> IniResult<()> {
    w.info.description = parse_and_translate_label(ini, w.text)?;
    Ok(())
}

/// `friend_parseScienceDefinition`'s `myFieldParse`.
fn science_fields<'a>() -> [FieldParse<ScienceFill<'a>>; 6] {
    [
        FieldParse::new("PrerequisiteSciences", f_prerequisite_sciences, UserData::None),
        FieldParse::new("SciencePurchasePointCost", f_science_purchase_point_cost, UserData::None),
        FieldParse::new("IsGrantable", f_is_grantable, UserData::None),
        FieldParse::new("DisplayName", f_display_name, UserData::None),
        FieldParse::new("Description", f_description, UserData::None),
        FieldParse::end(),
    ]
}

/// `class ScienceStore` (`TheScienceStore`).
// --- p1l9-upgrades ---
#[derive(Clone, Debug, Default)]
// --- end p1l9-upgrades ---
pub struct ScienceStore {
    /// `m_sciences`: the first definition of each science, with its chain of
    /// overrides.
    sciences: Vec<Overridable<ScienceInfo>>,
}

impl ScienceStore {
    /// `ScienceStore()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `init()`.
    pub fn init(&mut self) {
        self.sciences.clear();
    }

    /// `reset()`: delete the overrides marked by `INI_LOAD_CREATE_OVERRIDES`.
    /// A science that was itself created by an override file goes with them.
    pub fn reset(&mut self) {
        let old = std::mem::take(&mut self.sciences);
        for head in old {
            if let Some(kept) = Box::new(head).delete_overrides() {
                self.sciences.push(*kept);
            }
        }
    }

    /// The number of sciences (heads of override chains).
    pub fn len(&self) -> usize {
        self.sciences.len()
    }

    /// Whether the store has no science.
    pub fn is_empty(&self) -> bool {
        self.sciences.is_empty()
    }

    /// Every science's chain, first definition first, in store order.
    pub fn chains(&self) -> impl Iterator<Item = &Overridable<ScienceInfo>> {
        self.sciences.iter()
    }

    /// The final override of every science, in store order.
    pub fn infos(&self) -> impl Iterator<Item = &ScienceInfo> {
        self.sciences.iter().map(|h| &h.get_final_override().data)
    }

    /// `findScienceInfo(st)`: the first science whose final override has
    /// `st`, or `None`.
    pub fn find_science_info(&self, st: ScienceType) -> Option<&ScienceInfo> {
        self.infos().find(|i| i.science == st)
    }

    /// `isValidScience(st)`.
    pub fn is_valid_science(&self, st: ScienceType) -> bool {
        self.find_science_info(st).is_some()
    }

    /// `isScienceGrantable(st)`: false for an unknown science.
    pub fn is_science_grantable(&self, st: ScienceType) -> bool {
        self.find_science_info(st).is_some_and(|i| i.grantable)
    }

    /// `getNameAndDescription(st, name, description)`: `None` for an unknown
    /// science.
    pub fn get_name_and_description(&self, st: ScienceType) -> Option<(UnicodeString, UnicodeString)> {
        self.find_science_info(st).map(|i| (i.name.clone(), i.description.clone()))
    }

    /// `getSciencePurchaseCost(science)`: 0 for an unknown science.
    pub fn get_science_purchase_cost(&self, st: ScienceType) -> i32 {
        self.find_science_info(st).map_or(0, |i| i.science_purchase_point_cost)
    }

    /// `getScienceFromInternalName(name)`: `SCIENCE_INVALID` for the empty
    /// name, else the key of `name` (a new name gets a new key).
    pub fn get_science_from_internal_name(&self, names: &mut NameKeyGenerator, name: &[u8]) -> Result<ScienceType, NameKeyError> {
        science_from_internal_name(names, name)
    }

    /// `getInternalNameForScience(science)`: the empty string for
    /// `SCIENCE_INVALID` (and for a key the generator does not know).
    pub fn get_internal_name_for_science(&self, names: &NameKeyGenerator, science: ScienceType) -> Vec<u8> {
        internal_name_for_science(names, science)
    }

    /// `friend_getScienceNames()`: the internal name of every science, in
    /// store order.
    pub fn friend_get_science_names(&self, names: &NameKeyGenerator) -> Vec<Vec<u8>> {
        self.infos().map(|i| names.key_to_name(i.science.to_name_key()).to_vec()).collect()
    }

    /// `friend_lookupScience(scienceName)` (also `INI::scanScience`): the
    /// science named `science_name`, `INI_INVALID_DATA` if there is none.
    pub fn friend_lookup_science(&self, names: &mut NameKeyGenerator, science_name: &[u8]) -> IniResult<ScienceType> {
        scan_science(names, |s| self.is_valid_science(s), science_name)
    }

    /// `INI::parseScience`: one token, a known science.
    pub fn parse_science(&self, ini: &mut Ini<'_>, names: &mut NameKeyGenerator) -> IniResult<ScienceType> {
        let token = ini.get_next_token(None)?;
        self.friend_lookup_science(names, &token)
    }

    /// `INI::parseScienceVector`, against this store.
    pub fn parse_science_vector(&self, ini: &mut Ini<'_>, names: &mut NameKeyGenerator, store: &mut ScienceVec) -> IniResult<()> {
        parse_science_vector(ini, names, |s| self.is_valid_science(s), store)
    }

    /// `ScienceInfo::addRootSciences(v)` for `info`: a science with no
    /// prerequisites is a root (added once); any other adds the roots of its
    /// prerequisites. `path` is the chain of sciences being expanded: a
    /// prerequisite cycle recurses forever upstream.
    fn add_root_sciences(&self, info: &ScienceInfo, v: &mut ScienceVec, path: &mut Vec<*const ScienceInfo>) -> IniResult<()> {
        if info.prereq_sciences.is_empty() {
            if !v.contains(&info.science) {
                v.push(info.science);
            }
            return Ok(());
        }
        let me = info as *const ScienceInfo;
        if path.contains(&me) {
            return Err(IniError::undefined(Undefined::ScienceCycle));
        }
        path.push(me);
        for p in &info.prereq_sciences {
            if let Some(si) = self.find_science_info(*p) {
                self.add_root_sciences(si, v, path)?;
            }
        }
        path.pop();
        Ok(())
    }

    /// `friend_parseScienceDefinition(ini)` (the `Science` block, reached
    /// through `INI::parseScienceDefinition`): the science's name, then its
    /// fields until `End`. See the module docs for what a failing block
    /// leaves behind.
    ///
    /// - `INI_LOAD_CREATE_OVERRIDES`: a known name gets a new override
    ///   copied from the final override; an unknown one is added, marked as an
    ///   override.
    /// - Any other load: a known name is `INI_INVALID_DATA` ("duplicate
    ///   science").
    pub fn parse_science_definition(
        &mut self,
        ini: &mut Ini<'_>,
        names: &mut NameKeyGenerator,
        text: &dyn GameTextFetch,
    ) -> IniResult<()> {
        let c = ini.get_next_token(None)?;
        let st = name_to_science(names, &c)?;

        // "see if the science already exists": the heads, not the final overrides
        let existing = self.sciences.iter().position(|h| h.data.science == st);
        // what `friend_lookupScience` sees while the fields are read
        let valid: BTreeSet<ScienceType> = self.infos().map(|i| i.science).collect();

        let idx = if ini.get_load_type() == IniLoadType::CreateOverrides {
            match existing {
                None => {
                    let mut head = Overridable::new(ScienceInfo::new());
                    head.mark_as_override();
                    self.sciences.push(head);
                    self.sciences.len() - 1
                }
                Some(i) => {
                    let fin = self.sciences[i].friend_get_final_override();
                    let mut new_info = Overridable::new(fin.data.clone());
                    new_info.mark_as_override();
                    let _ = fin.set_next_override(Some(Box::new(new_info)));
                    i
                }
            }
        } else {
            if existing.is_some() {
                return Err(IniError::new(IniErrorKind::InvalidData));
            }
            self.sciences.push(Overridable::new(ScienceInfo::new()));
            self.sciences.len() - 1
        };

        {
            let target = &mut self.sciences[idx].friend_get_final_override().data;
            let mut fill = ScienceFill { info: target, names, valid: &valid, text };
            ini.init_from_ini(&mut fill, &science_fields())?;
            fill.info.science = st;
        }

        let info = &self.sciences[idx].get_final_override().data;
        let mut roots = info.root_sciences.clone();
        self.add_root_sciences(info, &mut roots, &mut Vec::new())?;
        self.sciences[idx].friend_get_final_override().data.root_sciences = roots;
        Ok(())
    }
}

/// `getScienceFromInternalName`.
fn science_from_internal_name(names: &mut NameKeyGenerator, name: &[u8]) -> Result<ScienceType, NameKeyError> {
    if name.is_empty() {
        return Ok(SCIENCE_INVALID);
    }
    names.try_name_to_key(name).map(ScienceType::from_name_key)
}

/// `getInternalNameForScience`.
fn internal_name_for_science(names: &NameKeyGenerator, science: ScienceType) -> Vec<u8> {
    if science == SCIENCE_INVALID {
        return Vec::new();
    }
    names.key_to_name(science.to_name_key()).to_vec()
}

/// Why `xferScienceType` / `xferScienceVec` failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScienceXferError {
    /// What upstream throws.
    Xfer(XferStatus),
    /// The name-key counter ran past `0x7fffffff`: undefined upstream.
    NameKeysExhausted,
}

impl From<XferStatus> for ScienceXferError {
    fn from(s: XferStatus) -> Self {
        ScienceXferError::Xfer(s)
    }
}

impl std::fmt::Display for ScienceXferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScienceXferError::Xfer(s) => write!(f, "{s}"),
            ScienceXferError::NameKeysExhausted => f.write_str("name keys exhausted (undefined upstream)"),
        }
    }
}

impl std::error::Error for ScienceXferError {}

/// `Xfer::xferScienceType`. Save: the science's internal name as an
/// `AsciiString`. Load: the name read, looked up with
/// `getScienceFromInternalName` (an unknown name gets a key); an empty name
/// is `XFER_UNKNOWN_STRING`. CRC: the 4 bytes of the `int`-sized enum.
pub fn xfer_science_type(xfer: &mut dyn Xfer, names: &mut NameKeyGenerator, science: &mut ScienceType) -> Result<(), ScienceXferError> {
    match xfer.get_xfer_mode() {
        XferMode::Save => {
            let mut name = internal_name_for_science(names, *science);
            xfer.xfer_ascii_string(&mut name)?;
            Ok(())
        }
        XferMode::Load => {
            let mut name = Vec::new();
            xfer.xfer_ascii_string(&mut name)?;
            *science = science_from_internal_name(names, &name).map_err(|_| ScienceXferError::NameKeysExhausted)?;
            if *science == SCIENCE_INVALID {
                return Err(XferStatus::UnknownString.into());
            }
            Ok(())
        }
        XferMode::Crc => {
            let mut b = science.0.to_le_bytes();
            xfer.xfer_implementation(&mut b)?;
            *science = ScienceType(i32::from_le_bytes(b));
            Ok(())
        }
        XferMode::Invalid => Err(XferStatus::ModeUnknown.into()),
    }
}

/// `Xfer::xferScienceVec` (version 1): an `UnsignedShort` count, then the
/// sciences. As upstream, the count is the size truncated to 16 bits while
/// a save writes every science; a load replaces the contents, keeping the
/// sciences read so far if one fails.
pub fn xfer_science_vec(xfer: &mut dyn Xfer, names: &mut NameKeyGenerator, science_vec: &mut ScienceVec) -> Result<(), ScienceXferError> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count = science_vec.len() as u16;
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save => {
            for s in science_vec.clone() {
                let mut s = s;
                xfer_science_type(xfer, names, &mut s)?;
            }
            Ok(())
        }
        XferMode::Load => {
            science_vec.clear();
            for _ in 0..count {
                let mut s = SCIENCE_INVALID;
                xfer_science_type(xfer, names, &mut s)?;
                science_vec.push(s);
            }
            Ok(())
        }
        XferMode::Crc => {
            for s in science_vec.iter() {
                let mut b = s.0.to_le_bytes();
                xfer.xfer_implementation(&mut b)?;
            }
            Ok(())
        }
        XferMode::Invalid => Err(XferStatus::ModeUnknown.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::BlockParse;
    use crate::common::xfer_crc::XferCrc;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;

    struct Text;
    impl GameTextFetch for Text {
        fn fetch(&self, label: &[u8]) -> UnicodeString {
            if label.starts_with(b"EMPTY") {
                return UnicodeString::new();
            }
            let u: Vec<u16> = label.iter().map(|&b| u16::from(b)).collect();
            UnicodeString::try_from_chars(&u).unwrap()
        }
    }

    struct Ctx {
        store: ScienceStore,
        names: NameKeyGenerator,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store.parse_science_definition(ini, &mut c.names, &Text)
    }
    static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse { token: "Science", parse: block }];

    fn ctx() -> Ctx {
        let mut names = NameKeyGenerator::new();
        names.reset();
        Ctx { store: ScienceStore::new(), names }
    }

    fn load(c: &mut Ctx, text: &[u8], t: IniLoadType) -> IniResult<()> {
        Ini::load(b"s.ini", text, t, None, &BLOCKS, c)
    }

    fn st(c: &mut Ctx, n: &str) -> ScienceType {
        ScienceType::from_name_key(c.names.name_to_key(n.as_bytes()))
    }

    const BASE: &[u8] = b"Science A\n DisplayName = NameA\n Description = DescA\n SciencePurchasePointCost = 3\nEnd\n\
Science B\n PrerequisiteSciences = A\n IsGrantable = No\nEnd\nScience C\n PrerequisiteSciences = A B\nEnd\n";

    #[test]
    fn definitions_lookups_and_roots() {
        let mut c = ctx();
        load(&mut c, BASE, IniLoadType::Overwrite).unwrap();
        let (a, b, cc) = (st(&mut c, "A"), st(&mut c, "B"), st(&mut c, "C"));
        assert!(c.store.is_valid_science(a) && !c.store.is_valid_science(SCIENCE_INVALID));
        assert_eq!(c.store.get_science_purchase_cost(a), 3);
        assert!(c.store.is_science_grantable(a) && !c.store.is_science_grantable(b));
        assert_eq!(c.store.find_science_info(cc).unwrap().root_sciences(), &[a]);
        let (n, d) = c.store.get_name_and_description(a).unwrap();
        assert_eq!((n.str().len(), d.str().len()), (5, 5));
        assert!(c.store.get_name_and_description(ScienceType(99)).is_none());
        assert_eq!(c.store.get_internal_name_for_science(&c.names, b), b"B");
        assert!(c.store.get_internal_name_for_science(&c.names, SCIENCE_INVALID).is_empty());
        assert_eq!(c.store.get_science_from_internal_name(&mut c.names, b"").unwrap(), SCIENCE_INVALID);
        assert_eq!(c.store.friend_lookup_science(&mut c.names, b"C").unwrap(), cc);
        assert_eq!(c.store.friend_lookup_science(&mut c.names, b"Z").unwrap_err().kind, IniErrorKind::InvalidData);
        assert_eq!(c.store.friend_get_science_names(&c.names), vec![b"A".to_vec(), b"B".to_vec(), b"C".to_vec()]);
        assert_eq!(c.store.len(), 3);
        assert_eq!(c.store.chains().count(), 3);
        assert!(!c.store.is_empty());
        // a duplicate is invalid data; a self-prerequisite of a first definition too
        assert_eq!(load(&mut c, b"Science A\nEnd\n", IniLoadType::Overwrite).unwrap_err().kind, IniErrorKind::InvalidData);
        let mut c2 = ctx();
        assert!(load(&mut c2, b"Science A\n PrerequisiteSciences = A\nEnd\n", IniLoadType::Overwrite).is_err());
        // the failed block stays, with SCIENCE_INVALID
        assert!(c2.store.is_valid_science(SCIENCE_INVALID));
        // an empty translation is invalid data
        let mut c3 = ctx();
        assert!(load(&mut c3, b"Science A\n DisplayName = EMPTY\nEnd\n", IniLoadType::Overwrite).is_err());
    }

    #[test]
    fn overrides_reset_and_cycles() {
        let mut c = ctx();
        load(&mut c, BASE, IniLoadType::Overwrite).unwrap();
        let (a, b) = (st(&mut c, "A"), st(&mut c, "B"));
        load(&mut c, b"Science A\n SciencePurchasePointCost = 9\nEnd\nScience New\nEnd\n", IniLoadType::CreateOverrides).unwrap();
        assert_eq!(c.store.get_science_purchase_cost(a), 9);
        assert_eq!(c.store.len(), 4);
        c.store.reset();
        assert_eq!(c.store.get_science_purchase_cost(a), 3);
        assert_eq!(c.store.len(), 3);
        // A needs B and B needs A: addRootSciences would recurse for ever
        let e = load(&mut c, b"Science A\n PrerequisiteSciences = B\nEnd\n", IniLoadType::CreateOverrides).unwrap_err();
        assert_eq!(e.kind, IniErrorKind::Undefined(Undefined::ScienceCycle));
        let _ = b;
        c.store.init();
        assert!(c.store.is_empty());
    }

    #[test]
    fn science_vector_and_scalar_parsers() {
        let mut c = ctx();
        load(&mut c, BASE, IniLoadType::Overwrite).unwrap();
        let (a, b) = (st(&mut c, "A"), st(&mut c, "B"));
        struct W {
            one: ScienceType,
            v: ScienceVec,
        }
        fn h(ini: &mut Ini<'_>, w: &mut (Ctx, W)) -> IniResult<()> {
            let (c, w) = (&mut w.0, &mut w.1);
            let line = ini.get_next_token(None)?;
            match line.as_slice() {
                b"one" => w.one = c.store.parse_science(ini, &mut c.names)?,
                _ => c.store.parse_science_vector(ini, &mut c.names, &mut w.v)?,
            }
            Ok(())
        }
        static T: [BlockParse<(Ctx, W)>; 1] = [BlockParse { token: "Science", parse: h }];
        let mut w = (c, W { one: SCIENCE_INVALID, v: vec![] });
        Ini::load(b"x", b"Science one B\nScience vec A B\n", IniLoadType::Overwrite, None, &T, &mut w).unwrap();
        assert_eq!((w.1.one, w.1.v.clone()), (b, vec![a, b]));
        Ini::load(b"x", b"Science vec A none B\n", IniLoadType::Overwrite, None, &T, &mut w).unwrap();
        assert!(w.1.v.is_empty());
        assert!(Ini::load(b"x", b"Science vec A Q\n", IniLoadType::Overwrite, None, &T, &mut w).is_err());
        assert_eq!(w.1.v, vec![a]);
    }

    #[test]
    fn xfers_round_trip() {
        let mut c = ctx();
        load(&mut c, BASE, IniLoadType::Overwrite).unwrap();
        let (a, b) = (st(&mut c, "A"), st(&mut c, "B"));
        let mut v = vec![a, b];
        let mut xs = XferSave::new();
        xs.open(b"s").unwrap();
        xfer_science_vec(&mut xs, &mut c.names, &mut v).unwrap();
        xs.close().unwrap();
        let mut back = vec![ScienceType(77)];
        let mut xl = XferLoad::new(xs.bytes().to_vec());
        xl.open(b"l").unwrap();
        xfer_science_vec(&mut xl, &mut c.names, &mut back).unwrap();
        assert_eq!(back, v);
        let mut one = a;
        let mut xs = XferSave::new();
        xs.open(b"s").unwrap();
        xfer_science_type(&mut xs, &mut c.names, &mut one).unwrap();
        let mut xl = XferLoad::new(vec![0]);
        xl.open(b"l").unwrap();
        let mut s = a;
        assert_eq!(xfer_science_type(&mut xl, &mut c.names, &mut s), Err(ScienceXferError::Xfer(XferStatus::UnknownString)));
        assert_eq!(s, SCIENCE_INVALID);
        let mut xc = XferCrc::new_without_nan_guard();
        xc.open(b"c").unwrap();
        xfer_science_vec(&mut xc, &mut c.names, &mut v).unwrap();
        assert_ne!(xc.get_crc(), 0);
        assert!(!format!("{}", ScienceXferError::NameKeysExhausted).is_empty());
    }
}
