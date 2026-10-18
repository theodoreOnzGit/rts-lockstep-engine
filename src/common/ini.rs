// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/INI.h
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/INIException.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   INI.h / INI.cpp: Colin Day, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The INI reader: line reading, `strtok` tokenising, block dispatch, the
//! field-parse-table mechanism, and the generic field parsers, reading from
//! an in-memory byte buffer (wasm has no file system).
//!
//! # The format, as upstream reads it
//!
//! A file is a sequence of lines (split at `\n`; at most 1028 bytes each,
//! longer lines are cut and the rest becomes the next line). On each line a
//! `;` ends the text (the comment is still read, then ignored), and every
//! byte 1-31 becomes a space. The first token of a line, split at
//! `" \n\r\t="`, names a block type, looked up case-sensitively in the block
//! table; the block handler reads the rest of the line (usually a name) and
//! then fields, one per line, until a line whose first token is `End` (any
//! case):
//!
//! ```text
//! Weapon RangerAdvancedCombatRifle   ; block type, name
//!   PrimaryDamage = 5.0              ; field token, value tokens
//!   DamageType    = SMALL_ARMS
//! End
//! ```
//!
//! Field tokens are looked up case-sensitively in parse tables
//! ([`FieldParse`]); each entry names a parser that pulls its value tokens
//! from the rest of the line. Upstream's tables hold byte offsets into the
//! object being filled and a C function pointer; here an entry holds a Rust
//! function that receives the object (`&mut T`) and stores into it, which
//! replaces both the offset and `MultiIniFieldParse`'s extra offsets.
//!
//! # What is left out
//!
//! - **The block handlers.** Upstream's `theTypeTable` maps 62 block types to
//!   handlers defined elsewhere, all of which build game objects (thing
//!   templates, weapons, audio events, ...): none is translated yet. The
//!   table's tokens are in [`TYPE_TABLE`] (with the upstream file of each
//!   handler); a caller passes its own [`BlockParse`] table to [`Ini::load`].
//! - **Field parsers that look names up in game systems not yet
//!   translated:** `parseAndTranslateLabel` (`TheGameText`),
//!   `parseMappedImage`, `parseAnim2DTemplate`, `parseAudioEventRTS`,
//!   `parseDynamicAudioEventRTS`, `parseThingTemplate`,
//!   `parseArmorTemplate`, `parseWeaponTemplate`, `parseFXList`,
//!   `parseParticleSystemTemplate`, `parseDamageFX`,
//!   `parseObjectCreationList`, `parseUpgradeTemplate`,
//!   `parseSpecialPowerTemplate`, `parseScience`, `parseScienceVector` and
//!   `scanScience`.
//!   (~~`parseScience`, `parseScienceVector` and `scanScience`~~ **translated in
//!   `common::science`** with `parseAndTranslateLabel`; p1w2-science.)
//! - `setFPMode()` at the start of `load` (x87 precision control; the Rust
//!   is plain `f32` already).
//! - `INI_LOAD_*` has no effect inside this file upstream either; it is kept
//!   for the handlers ([`Ini::get_load_type`]).
//!
//! # Numbers
//!
//! `scanInt`, `scanUnsignedInt` and `scanReal` are `sscanf` with `%d`, `%u`
//! and `%f`. They are reproduced: leading C white space, an optional sign,
//! the longest numeric prefix (`"12abc"` is 12, `"1.5.3"` is 1.5, `"1e"` is
//! 1, as glibc reads it), and `%u` negates in unsigned arithmetic (`"-1"` is
//! 4294967295). `%f` rounds correctly to `f32` (as glibc's `strtof`; the
//! original MSVC runtime of the shipped game went through `double`, so its
//! result can differ in the last bit; not verified here). Where the C
//! library makes the result undefined or platform-dependent the Rust returns
//! [`Undefined`] instead: integers out of range of the target type, a real
//! that overflows `f32`, and `inf`, `nan` and hexadecimal reals, which C99
//! libraries (glibc) accept and the C89 runtime the game was built with did
//! not.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! Every such case is an [`IniError`] with kind [`IniErrorKind::Undefined`]:
//! the number cases above; a bit index of 32 or more in a bit-string list
//! (`1 << 32`); `+NAME`/`-NAME` or an empty list in `parseBitString8`, which
//! reads an uninitialised variable; a lone `"` as the first token of
//! `parseQuotedAsciiString` (an uninitialised buffer); a duration over 65535 frames into an
//! `UnsignedShort`; an error message longer than its 1024-byte `sprintf`
//! buffer; `isspace` on a byte of 0x80 or more in `isDeclarationOfType` and
//! `isEndOfBlock`; and a file name shorter than the directory in
//! `loadDirectory`.
//!
//! # Upstream quirks that are kept
//!
//! - Over-long lines are cut at 1028 bytes and the remainder is read as the
//!   next line (and counted as one).
//! - The rest of a line after a NUL byte or `;` is ignored, but still read.
//! - `parseAsciiString` (`getNextAsciiString`) is "broken" on purpose (its
//!   own comment): for `"abc" x` it returns `abc"x`. `parseQuotedAsciiString`
//!   is the fixed version, which drops a one-byte second token.
//! - A parse-table entry with a token but no parse function ends the search
//!   in that table (the next table is tried); the terminating entry of a
//!   table, if it has a parse function, catches every unknown field and gets
//!   the field token as its user data.
//! - `parseBitString32` with `+`/`-` modifies the stored value; a plain name
//!   first clears it. `NONE` after any name is an error.
//! - `parseDamageTypeFlags` with an unknown `+NAME`/`-NAME` fails through
//!   `std::bitset::set(-1)` throwing `std::out_of_range`
//!   ([`IniErrorKind::BitIndexOutOfRange`]). (Strictly, converting -1 to the
//!   `DamageType` enum, whose values are 0..=38, gives an unspecified value
//!   in C++98 and is undefined since C++17; every compiler keeps -1.)
//! - **Upstream bug:** the message of the `INIException` for a failed field
//!   names the field through a pointer into the line buffer. When the field's
//!   parser read further lines before failing (a nested block, as many
//!   module parsers do), the buffer holds a later line by then, and the
//!   message names whatever bytes sit at that offset now (and overruns its
//!   1024-byte buffer if they are long: [`Undefined::MessageOverflow`]).
//! - `loadDirectory` ignores its `subdirs` argument (it always recurses) and
//!   loads files directly in the directory before those in subdirectories.
//! - `isDeclarationOfType` and `isEndOfBlock` require at least one byte after
//!   the block type (`"End"` alone is not an end of block).

use super::base_type::{Coord2D, Coord3D, ICoord2D, RGBAColorInt, RGBColor, PI};
use super::game_common::{
    clear_veterancy_level_flag, convert_acceleration_in_secs_to_frames,
    convert_angular_velocity_in_degrees_per_sec_to_rads_per_frame,
    convert_duration_from_msecs_to_frames, convert_velocity_in_secs_to_frames,
    set_veterancy_level_flag, VeterancyLevelFlags, VETERANCY_LEVEL_FLAGS_ALL,
    VETERANCY_LEVEL_FLAGS_NONE, VETERANCY_NAMES,
};
use super::random_value::{DistributionType, GameClientRandomVariable, DISTRIBUTION_TYPE_NAMES};
use super::xfer::{Xfer, XferStatus};
use crate::formats::big::FilenameList;
use crate::logic::damage::{
    clear_damage_type_flag, clear_death_type_flag, set_damage_type_flag, set_death_type_flag,
    DamageTypeFlags, DeathTypeFlags, DEATH_NAMES, DEATH_TYPE_FLAGS_ALL, DEATH_TYPE_FLAGS_NONE,
};
use petir::Real;

/// `INI_MAX_CHARS_PER_LINE`.
pub const INI_MAX_CHARS_PER_LINE: usize = 1028;
/// `m_seps`.
pub const SEPS: &[u8] = b" \n\r\t=";
/// `m_sepsPercent` (`" \n\r\t=%%"`: a plain string, so two `%`).
pub const SEPS_PERCENT: &[u8] = b" \n\r\t=%%";
/// `m_sepsColon`.
pub const SEPS_COLON: &[u8] = b" \n\r\t=:";
/// `m_sepsQuote` ("stop at \" = EOL").
pub const SEPS_QUOTE: &[u8] = b"\"\n=";
/// The separators of `parseSoundsList`.
pub const SEPS_SOUNDS: &[u8] = b" \t,=";
/// `m_blockEndToken` (compared with `stricmp`).
pub const BLOCK_END_TOKEN: &[u8] = b"END";
/// `ERROR_BUG` (Common/Errors.h).
pub const ERROR_BUG: u32 = 0xdead_0001;
/// `ERROR_BAD_INI`, the value of every `INI_*` status code.
pub const ERROR_BAD_INI: u32 = 0xdead_0006;
/// The size of the `char buff[1024]` the exception messages are formatted in.
pub const EXCEPTION_BUFFER: usize = 1024;
/// `MultiIniFieldParse::MAX_MULTI_FIELDS`.
pub const MAX_MULTI_FIELDS: usize = 16;

/// `enum INILoadType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum IniLoadType {
    Invalid = 0,
    Overwrite = 1,
    CreateOverrides = 2,
    Multifile = 3,
}

/// Where upstream's behaviour is undefined (see the module docs). The Rust
/// stops with an error there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Undefined {
    /// `%d` of a value outside `Int`.
    IntOutOfRange,
    /// `%u` of a magnitude above `UnsignedInt`.
    UnsignedOutOfRange,
    /// `%f` of a finite decimal that overflows `float`.
    RealOutOfRange,
    /// `%f` of `inf`, `nan` or a hexadecimal real (C99 libraries only).
    RealPlatformDependent,
    /// `1 << n` with `n >= 32` (a bit-string list of more than 32 names).
    ShiftOutOfRange,
    /// `parseBitString8` reads its uninitialised `tmp`.
    UninitialisedBitString8,
    /// `getNextQuotedAsciiString` uses its uninitialised buffer.
    UninitialisedQuoteBuffer,
    /// A float converted to an integer type that cannot hold it.
    FloatToIntOutOfRange,
    /// An exception message overruns its 1024-byte buffer.
    MessageOverflow,
    /// `isspace` of a negative `char` (a byte of 0x80 or more).
    IsspaceNonAscii,
    /// `loadDirectory`: a file name shorter than the directory name.
    FilenameShorterThanDirectory,
    // --- p1w1-armor ---
    /// A `DamageType` outside `[0, DAMAGE_NUM_TYPES)` indexes an array
    /// (`parseArmorCoefficients` and `DamageFX`'s parsers with an unknown
    /// damage name, whose index is -1).
    DamageTypeOutOfRange,
    /// `NameKeyGenerator` ran past its last key (`0x7fffffff`).
    NameKeysExhausted,
    // --- end p1w1-armor ---
    // --- p1w7-sonnet ---
    /// A missing token passed to `stricmp` (`WeaponTemplate::parseShotDelay`
    /// reads `getNextTokenOrNull`'s `NULL` when the line has nothing after the
    /// label).
    NullTokenString,
    // --- end p1w7-sonnet ---
    // --- p1w2-terrain ---
    /// `MultiplayerColorDefinition::setColor` takes `rgb.getAsInt()`, which
    /// is undefined for a negative component (`<<` of a negative `Int`) or a
    /// component that does not fit an `Int`. `parseRGBColor` keeps every
    /// component in 0..=255, so the INI cannot reach this; only a direct
    /// call with such an `RGBColor` can.
    ColorToInt,
    // --- end p1w2-terrain ---
    // --- p1w2-science ---
    /// `ScienceInfo::addRootSciences` recurses through the prerequisites of a
    /// cycle for ever (a stack overflow).
    ScienceCycle,
    // --- end p1w2-science ---
    // --- p1w2-upgrade ---
    /// A new `SpecialPowerTemplate` copied from `DefaultSpecialPower` shares
    /// the default's next-override pointer upstream, which `reset` frees
    /// from under it; owned override chains cannot share a node (see
    /// `common::special_power`).
    AliasedOverrideChain,
    // --- end p1w2-upgrade ---
    // --- p1w3-modules ---
    /// `ModuleData::m_moduleTagNameKey` is never set by the constructor
    /// (`ModuleData() { }`); reading it before `setModuleTagNameKey` reads an
    /// uninitialised `NameKeyType`.
    UninitialisedModuleTag,
    /// `Module::friend_newModuleData` runs `initFromINI(data, 0)`: a null
    /// parse table. Any field line reaches `findFieldParse(NULL, ...)`, which
    /// dereferences it (only `End` is defined).
    NullFieldParseTable,
    /// `UpgradeMuxData::getUpgradeActivationMasks` accepts an empty name or
    /// `None` that finds no upgrade and then calls
    /// `theTemplate->getUpgradeMask()` on the null template;
    /// `muxDataProcessUpgradeRemoval` does the same with the removal names,
    /// passing the null template to `Object::removeUpgrade`, whose first
    /// statement is `upgradeT->getUpgradeMask()`.
    NullUpgradeTemplate,
    /// `SpecialPowerModule::isReady` (with a controlling player) and the
    /// other members that ask `m_specialPowerTemplate->isSharedNSync()` run
    /// on a module whose data has no template (`SpecialPowerTemplate None`,
    /// or one that was not found).
    NullSpecialPowerTemplate,
    // --- end p1w3-modules ---
    // --- p1w4-thingtemplate ---
    /// `ThingFactory::reset` deletes a template that is not at the head of
    /// the template list (a map-only template created before a permanent
    /// one) and leaves the previous template's `m_nextThingTemplate` pointing
    /// at the freed object; the next walk of the list reads freed memory.
    DanglingTemplateLink,
    // --- end p1w4-thingtemplate ---
}

/// What failed, innermost cause first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IniErrorKind {
    /// `throw INI_INVALID_DATA`.
    InvalidData,
    /// `throw INI_UNKNOWN_TOKEN` (unknown block or field).
    UnknownToken,
    /// `throw INI_MISSING_END_TOKEN`.
    MissingEndToken,
    /// `throw INI_INVALID_NAME_LIST`.
    InvalidNameList,
    /// `throw INI_INVALID_DIRECTORY`.
    InvalidDirectory,
    /// `throw INI_CANT_OPEN_FILE`.
    CantOpenFile,
    /// `throw ERROR_BUG`.
    Bug,
    /// `std::bitset::set` threw `std::out_of_range`.
    BitIndexOutOfRange,
    /// The xfer of a line failed (cannot happen with `XferCRC`).
    Xfer(XferStatus),
    /// Upstream is undefined here.
    Undefined(Undefined),
    // --- p1w4-thingtemplate ---
    /// An object template names a behavior or body module that
    /// `ModuleFactory` registers but whose class is not translated yet
    /// (`ModuleFactoryError::NotTranslated`). Upstream cannot fail there
    /// (every class exists); the template load stops instead of skipping the
    /// module.
    ModuleNotTranslated(Vec<u8>),
    /// An object template names a module that `ModuleFactory` has never
    /// registered (`ModuleFactoryError::UnknownModule`; upstream returns a
    /// null `ModuleData` and dereferences it).
    ModuleUnknown(Vec<u8>),
    /// One of the six registrations that the shipped game does not compile.
    ModuleNotInShippedBuild(Vec<u8>),
    // --- end p1w4-thingtemplate ---
}

/// What upstream throws, as the caller of `INI::load` (or of a field parser)
/// would catch it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Thrown {
    /// An `INIException` with this message.
    Exception(Vec<u8>),
    /// An `INI_*` status code (anonymous enum, value `ERROR_BAD_INI`).
    Status(u32),
    /// An `ErrorCode` (`ERROR_BUG`).
    ErrorCode(u32),
    /// `std::out_of_range`.
    OutOfRange,
}

/// An INI error: the innermost cause and, when an `INIException` wrapped it,
/// the message of the outermost one (the inner messages are lost upstream).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IniError {
    pub kind: IniErrorKind,
    pub exception: Option<Vec<u8>>,
}

impl IniError {
    pub fn new(kind: IniErrorKind) -> Self {
        IniError { kind, exception: None }
    }
    pub fn undefined(u: Undefined) -> Self {
        IniError::new(IniErrorKind::Undefined(u))
    }
    /// Whether upstream's behaviour is undefined for this input.
    pub fn is_undefined(&self) -> bool {
        matches!(self.kind, IniErrorKind::Undefined(_))
    }
    /// What upstream throws (`None` where upstream is undefined or the
    /// failure is the Rust xfer's).
    pub fn thrown(&self) -> Option<Thrown> {
        if let IniErrorKind::Undefined(_) = self.kind {
            return None;
        }
        // --- p1w4-thingtemplate ---
        if let IniErrorKind::ModuleNotTranslated(_) | IniErrorKind::ModuleUnknown(_) | IniErrorKind::ModuleNotInShippedBuild(_) = self.kind {
            return None; // not something upstream throws, wrapped or not
        }
        // --- end p1w4-thingtemplate ---
        if let Some(m) = &self.exception {
            return Some(Thrown::Exception(m.clone()));
        }
        Some(match self.kind {
            IniErrorKind::Bug => Thrown::ErrorCode(ERROR_BUG),
            IniErrorKind::BitIndexOutOfRange => Thrown::OutOfRange,
            IniErrorKind::Xfer(_) => return None,
            IniErrorKind::ModuleNotTranslated(_) | IniErrorKind::ModuleUnknown(_) | IniErrorKind::ModuleNotInShippedBuild(_) => return None,
            _ => Thrown::Status(ERROR_BAD_INI),
        })
    }
    /// `catch (...) { sprintf(buff, ...); throw INIException(buff); }`.
    fn wrap(self, message: Vec<u8>) -> IniError {
        if self.is_undefined() {
            return self;
        }
        if message.len() + 1 > EXCEPTION_BUFFER {
            return IniError::undefined(Undefined::MessageOverflow);
        }
        IniError { kind: self.kind, exception: Some(message) }
    }
}

impl std::fmt::Display for IniError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.exception {
            Some(m) => write!(f, "{} ({:?})", String::from_utf8_lossy(m).trim_end(), self.kind),
            None => write!(f, "INI error {:?}", self.kind),
        }
    }
}

impl std::error::Error for IniError {}

/// Result of the INI functions.
pub type IniResult<T> = Result<T, IniError>;

fn err<T>(kind: IniErrorKind) -> IniResult<T> {
    Err(IniError::new(kind))
}

fn undefined<T>(u: Undefined) -> IniResult<T> {
    Err(IniError::undefined(u))
}

/// `struct LookupListRec`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LookupListRec {
    pub name: &'static str,
    pub value: i32,
}

/// The `const void* userData` of a parse-table entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserData {
    None,
    /// A `ConstCharPtrArray` (its `NULL` terminator is the slice end).
    Names(&'static [&'static str]),
    /// A `ConstLookupListRecArray`.
    Lookup(&'static [LookupListRec]),
    /// An integer (`parseBitInInt32`'s mask).
    Mask(u32),
    /// The field token, given to a table's catch-all entry.
    Token(Vec<u8>),
}

impl UserData {
    /// The name list; empty (`NULL` upstream) for anything else.
    pub fn names(&self) -> &'static [&'static str] {
        match self {
            UserData::Names(n) => n,
            _ => &[],
        }
    }
    /// The lookup list; empty for anything else.
    pub fn lookup(&self) -> &'static [LookupListRec] {
        match self {
            UserData::Lookup(l) => l,
            _ => &[],
        }
    }
    /// The mask; 0 for anything else.
    pub fn mask(&self) -> u32 {
        match self {
            UserData::Mask(m) => *m,
            _ => 0,
        }
    }
}

/// `INIFieldParseProc`: parse the field's value tokens into the object.
pub type FieldParseFn<T> = fn(&mut Ini<'_>, &mut T, &UserData) -> IniResult<()>;

/// `struct FieldParse`: one parse-table entry. A table is a slice; the first
/// entry with `token: None` ends it, and acts as the catch-all if it has a
/// parse function (see the module docs).
pub struct FieldParse<T> {
    pub token: Option<&'static str>,
    pub parse: Option<FieldParseFn<T>>,
    pub user_data: UserData,
}

impl<T> FieldParse<T> {
    /// An ordinary entry.
    pub const fn new(token: &'static str, parse: FieldParseFn<T>, user_data: UserData) -> Self {
        FieldParse { token: Some(token), parse: Some(parse), user_data }
    }
    /// An entry with a token and a `NULL` parse function.
    pub const fn without_parse(token: &'static str) -> Self {
        FieldParse { token: Some(token), parse: None, user_data: UserData::None }
    }
    /// A terminating entry that catches every unknown field.
    pub const fn catch_all(parse: FieldParseFn<T>) -> Self {
        FieldParse { token: None, parse: Some(parse), user_data: UserData::None }
    }
    /// The `{ 0, 0, 0, 0 }` terminator.
    pub const fn end() -> Self {
        FieldParse { token: None, parse: None, user_data: UserData::None }
    }
}

/// `class MultiIniFieldParse`: up to 16 tables searched in order.
pub struct MultiIniFieldParse<'t, T> {
    tables: Vec<&'t [FieldParse<T>]>,
}

impl<T> Default for MultiIniFieldParse<'_, T> {
    fn default() -> Self {
        MultiIniFieldParse { tables: Vec::new() }
    }
}

impl<'t, T> MultiIniFieldParse<'t, T> {
    pub fn new() -> Self {
        Self::default()
    }
    /// `add(f)`: `ERROR_BUG` past 16 tables. (Upstream's extra offset is
    /// replaced by the entries' functions choosing the store.)
    pub fn add(&mut self, f: &'t [FieldParse<T>]) -> IniResult<()> {
        if self.tables.len() < MAX_MULTI_FIELDS {
            self.tables.push(f);
            Ok(())
        } else {
            err(IniErrorKind::Bug)
        }
    }
    pub fn get_count(&self) -> usize {
        self.tables.len()
    }
    pub fn get_nth_field_parse(&self, i: usize) -> &'t [FieldParse<T>] {
        self.tables[i]
    }
}

/// `INIBlockParse`: a block-type handler.
pub type BlockParseFn<C> = fn(&mut Ini<'_>, &mut C) -> IniResult<()>;

/// `struct BlockParse`: one entry of the block-type table.
pub struct BlockParse<C> {
    pub token: &'static str,
    pub parse: BlockParseFn<C>,
}

/// Upstream's `theTypeTable`, in order: each block type's token, upstream
/// handler, and the file that defines the handler (`GameEngine/Source/...`).
/// None of the handlers is translated yet.
pub const TYPE_TABLE: [(&str, &str, &str); 62] = [
    ("AIData", "INI::parseAIDataDefinition", "Common/INI/INIAiData.cpp"),
    ("Animation", "INI::parseAnim2DDefinition", "Common/INI/INIAnimation.cpp"),
    ("Armor", "INI::parseArmorDefinition", "GameLogic/Object/Armor.cpp"),
    ("AudioEvent", "INI::parseAudioEventDefinition", "Common/INI/INIAudioEventInfo.cpp"),
    ("AudioSettings", "INI::parseAudioSettingsDefinition", "Common/Audio/GameAudio.cpp"),
    ("Bridge", "INI::parseTerrainBridgeDefinition", "Common/INI/INITerrainBridge.cpp"),
    ("Campaign", "INI::parseCampaignDefinition", "GameClient/System/CampaignManager.cpp"),
    ("ChallengeGenerals", "INI::parseChallengeModeDefinition", "GameClient/GUI/ChallengeGenerals.cpp"),
    ("CommandButton", "INI::parseCommandButtonDefinition", "Common/INI/INICommandButton.cpp"),
    ("CommandMap", "INI::parseMetaMapDefinition", "GameClient/MessageStream/MetaEvent.cpp"),
    ("CommandSet", "INI::parseCommandSetDefinition", "Common/INI/INICommandSet.cpp"),
    ("ControlBarScheme", "INI::parseControlBarSchemeDefinition", "Common/INI/INIControlBarScheme.cpp"),
    ("ControlBarResizer", "INI::parseControlBarResizerDefinition", "GameClient/GUI/ControlBar/ControlBarResizer.cpp"),
    ("CrateData", "INI::parseCrateTemplateDefinition", "Common/INI/INICrate.cpp"),
    ("Credits", "INI::parseCredits", "GameClient/Credits.cpp"),
    ("WindowTransition", "INI::parseWindowTransitions", "GameClient/GUI/GameWindowTransitions.cpp"),
    ("DamageFX", "INI::parseDamageFXDefinition", "Common/INI/INIDamageFX.cpp"),
    ("DialogEvent", "INI::parseDialogDefinition", "Common/INI/INIAudioEventInfo.cpp"),
    ("DrawGroupInfo", "INI::parseDrawGroupNumberDefinition", "Common/INI/INIDrawGroupInfo.cpp"),
    ("EvaEvent", "INI::parseEvaEvent", "GameClient/Eva.cpp"),
    ("FXList", "INI::parseFXListDefinition", "GameClient/FXList.cpp"),
    ("GameData", "INI::parseGameDataDefinition", "Common/INI/INIGameData.cpp"),
    ("InGameUI", "INI::parseInGameUIDefinition", "GameClient/InGameUI.cpp"),
    ("Locomotor", "INI::parseLocomotorTemplateDefinition", "GameLogic/Object/Locomotor.cpp"),
    ("Language", "INI::parseLanguageDefinition", "GameClient/GlobalLanguage.cpp"),
    ("MapCache", "INI::parseMapCacheDefinition", "Common/INI/INIMapCache.cpp"),
    ("MapData", "INI::parseMapDataDefinition", "Common/INI/INIMapData.cpp"),
    ("MappedImage", "INI::parseMappedImageDefinition", "Common/INI/INIMappedImage.cpp"),
    ("MiscAudio", "INI::parseMiscAudio", "Common/INI/INIMiscAudio.cpp"),
    ("Mouse", "INI::parseMouseDefinition", "GameClient/Input/Mouse.cpp"),
    ("MouseCursor", "INI::parseMouseCursorDefinition", "GameClient/Input/Mouse.cpp"),
    ("MultiplayerColor", "INI::parseMultiplayerColorDefinition", "Common/INI/INIMultiplayer.cpp"),
    ("MultiplayerStartingMoneyChoice", "INI::parseMultiplayerStartingMoneyChoiceDefinition", "Common/INI/INIMultiplayer.cpp"),
    ("OnlineChatColors", "INI::parseOnlineChatColorDefinition", "GameNetwork/GameSpy/Chat.cpp"),
    ("MultiplayerSettings", "INI::parseMultiplayerSettingsDefinition", "Common/INI/INIMultiplayer.cpp"),
    ("MusicTrack", "INI::parseMusicTrackDefinition", "Common/INI/INIAudioEventInfo.cpp"),
    ("Object", "INI::parseObjectDefinition", "Common/INI/INIObject.cpp"),
    ("ObjectCreationList", "INI::parseObjectCreationListDefinition", "GameLogic/Object/ObjectCreationList.cpp"),
    ("ObjectReskin", "INI::parseObjectReskinDefinition", "Common/INI/INIObject.cpp"),
    ("ParticleSystem", "INI::parseParticleSystemDefinition", "Common/INI/INIParticleSys.cpp"),
    ("PlayerTemplate", "INI::parsePlayerTemplateDefinition", "Common/RTS/PlayerTemplate.cpp"),
    ("Road", "INI::parseTerrainRoadDefinition", "Common/INI/INITerrainRoad.cpp"),
    ("Science", "INI::parseScienceDefinition", "Common/RTS/Science.cpp"),
    ("Rank", "INI::parseRankDefinition", "GameLogic/System/RankInfo.cpp"),
    ("SpecialPower", "INI::parseSpecialPowerDefinition", "Common/INI/INISpecialPower.cpp"),
    ("ShellMenuScheme", "INI::parseShellMenuSchemeDefinition", "GameClient/GUI/Shell/ShellMenuScheme.cpp"),
    ("Terrain", "INI::parseTerrainDefinition", "Common/INI/INITerrain.cpp"),
    ("Upgrade", "INI::parseUpgradeDefinition", "Common/INI/INIUpgrade.cpp"),
    ("Video", "INI::parseVideoDefinition", "Common/INI/INIVideo.cpp"),
    ("WaterSet", "INI::parseWaterSettingDefinition", "Common/INI/INIWater.cpp"),
    ("WaterTransparency", "INI::parseWaterTransparencyDefinition", "Common/INI/INIWater.cpp"),
    ("Weather", "INI::parseWeatherDefinition", "GameClient/Snow.cpp"),
    ("Weapon", "INI::parseWeaponTemplateDefinition", "Common/INI/INIWeapon.cpp"),
    ("WebpageURL", "INI::parseWebpageURLDefinition", "Common/INI/INIWebpageURL.cpp"),
    ("HeaderTemplate", "INI::parseHeaderTemplateDefinition", "GameClient/GUI/HeaderTemplate.cpp"),
    ("StaticGameLOD", "INI::parseStaticGameLODDefinition", "Common/GameLOD.cpp"),
    ("DynamicGameLOD", "INI::parseDynamicGameLODDefinition", "Common/GameLOD.cpp"),
    ("LODPreset", "INI::parseLODPreset", "Common/GameLOD.cpp"),
    ("BenchProfile", "INI::parseBenchProfile", "Common/GameLOD.cpp"),
    ("ReallyLowMHz", "parseReallyLowMHz", "Common/GameLOD.cpp"),
    ("ScriptAction", "ScriptEngine::parseScriptAction", "GameLogic/ScriptEngine/ScriptEngine.cpp"),
    ("ScriptCondition", "ScriptEngine::parseScriptCondition", "GameLogic/ScriptEngine/ScriptEngine.cpp"),
];

// ---------------------------------------------------------------------------
// C library helpers, with their C-locale meaning
// ---------------------------------------------------------------------------

/// `isspace` in the C locale, for a non-negative `char`.
fn is_c_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `stricmp(a, b) == 0` (ASCII case folding, as `_stricmp` in the C locale).
fn stricmp_eq(a: &[u8], b: &[u8]) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// The bytes of a C string: up to the first NUL.
fn c_str(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// `%d` / `%u` input: white space, sign, digits. `None` if no digit.
/// The magnitude saturates above `u64::MAX` (only its range matters).
fn scan_decimal(token: &[u8]) -> Option<(bool, u64)> {
    let mut i = 0;
    while i < token.len() && is_c_space(token[i]) {
        i += 1;
    }
    let mut neg = false;
    if i < token.len() && (token[i] == b'+' || token[i] == b'-') {
        neg = token[i] == b'-';
        i += 1;
    }
    let start = i;
    let mut mag: u64 = 0;
    while i < token.len() && token[i].is_ascii_digit() {
        mag = mag.saturating_mul(10).saturating_add(u64::from(token[i] - b'0'));
        i += 1;
    }
    if i == start {
        None
    } else {
        Some((neg, mag))
    }
}

/// `sscanf(token, "%f", &value)`: see the module docs.
fn scan_float(token: &[u8]) -> IniResult<f32> {
    let mut i = 0;
    while i < token.len() && is_c_space(token[i]) {
        i += 1;
    }
    let begin = i;
    if i < token.len() && (token[i] == b'+' || token[i] == b'-') {
        i += 1;
    }
    let rest = &token[i..];
    let starts = |p: &[u8]| rest.len() >= p.len() && rest[..p.len()].eq_ignore_ascii_case(p);
    if starts(b"inf") || starts(b"nan") || starts(b"0x") {
        return undefined(Undefined::RealPlatformDependent);
    }
    let mut digits = 0;
    while i < token.len() && token[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < token.len() && token[i] == b'.' {
        i += 1;
        while i < token.len() && token[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return err(IniErrorKind::InvalidData);
    }
    // An exponent counts only with at least one digit ("1e" is 1).
    if i < token.len() && (token[i] == b'e' || token[i] == b'E') {
        let mut j = i + 1;
        if j < token.len() && (token[j] == b'+' || token[j] == b'-') {
            j += 1;
        }
        if j < token.len() && token[j].is_ascii_digit() {
            while j < token.len() && token[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
        }
    }
    // The matched text is ASCII by construction.
    let text = std::str::from_utf8(&token[begin..i]).map_err(|_| IniError::new(IniErrorKind::InvalidData))?;
    let v: f32 = text.parse().map_err(|_| IniError::new(IniErrorKind::InvalidData))?;
    if v.is_infinite() {
        return undefined(Undefined::RealOutOfRange);
    }
    Ok(v)
}

/// `1 << bitIndex` on an `Int` (index 31 gives the sign bit, as every
/// compiler does; 32 or more is undefined).
fn int_bit(index: i32) -> IniResult<u32> {
    if (0..32).contains(&index) {
        Ok(1u32 << index)
    } else {
        undefined(Undefined::ShiftOutOfRange)
    }
}

/// `GameMakeColor(red, green, blue, alpha)` (GameClient/Color.h): the
/// `Color` (an `Int`) `alpha << 24 | red << 16 | green << 8 | blue`.
pub fn game_make_color(red: u8, green: u8, blue: u8, alpha: u8) -> i32 {
    ((u32::from(alpha) << 24) | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)) as i32
}

// ---------------------------------------------------------------------------
// The reader
// ---------------------------------------------------------------------------

/// `class INI`, while a file is being loaded (see [`Ini::load`]).
pub struct Ini<'x> {
    data: Vec<u8>,
    pos: usize,
    filename: Vec<u8>,
    load_type: IniLoadType,
    line_num: u32,
    buffer: Vec<u8>,
    /// `strtok`'s saved position in `buffer`.
    strtok_next: usize,
    end_of_file: bool,
    /// `s_xfer`: every line read is xfered into it (the INI CRC).
    xfer: Option<&'x mut dyn Xfer>,
}

impl<'x> Default for Ini<'x> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'x> Ini<'x> {
    /// `INI::INI()`.
    pub fn new() -> Self {
        Ini {
            data: Vec::new(),
            pos: 0,
            filename: b"None".to_vec(),
            load_type: IniLoadType::Invalid,
            line_num: 0,
            buffer: vec![0; INI_MAX_CHARS_PER_LINE + 1],
            strtok_next: 0,
            end_of_file: false,
            xfer: None,
        }
    }

    /// `getFilename`.
    pub fn get_filename(&self) -> &[u8] {
        &self.filename
    }
    /// `getLoadType`.
    pub fn get_load_type(&self) -> IniLoadType {
        self.load_type
    }
    /// `getLineNum`.
    pub fn get_line_num(&self) -> u32 {
        self.line_num
    }
    /// `getSeps`.
    pub fn get_seps(&self) -> &'static [u8] {
        SEPS
    }
    /// `getSepsPercent`.
    pub fn get_seps_percent(&self) -> &'static [u8] {
        SEPS_PERCENT
    }
    /// `getSepsColon`.
    pub fn get_seps_colon(&self) -> &'static [u8] {
        SEPS_COLON
    }
    /// `getSepsQuote`.
    pub fn get_seps_quote(&self) -> &'static [u8] {
        SEPS_QUOTE
    }
    /// `isEOF`.
    pub fn is_eof(&self) -> bool {
        self.end_of_file
    }
    /// The current line buffer, up to its first NUL (`m_buffer`; tokens
    /// already taken from it end in NULs, so this is the unread rest only
    /// before the first token is taken).
    pub fn current_line(&self) -> &[u8] {
        c_str(&self.buffer)
    }

    /// `INI::load(filename, loadType, pXfer)` on the file's bytes, with the
    /// block-type table `blocks` (upstream's is fixed, [`TYPE_TABLE`]) and a
    /// context the handlers fill.
    pub fn load<C>(
        filename: &[u8],
        data: &[u8],
        load_type: IniLoadType,
        xfer: Option<&'x mut dyn Xfer>,
        blocks: &[BlockParse<C>],
        ctx: &mut C,
    ) -> IniResult<()> {
        // prepFile: TheFileSystem->openFile(...)->convertToRAMFile().
        let mut ini = Ini::new();
        ini.data = data.to_vec();
        ini.filename = filename.to_vec();
        ini.load_type = load_type;
        ini.xfer = xfer;
        ini.load_lines(blocks, ctx)
        // unPrepFile: the reader is dropped.
    }

    fn load_lines<C>(&mut self, blocks: &[BlockParse<C>], ctx: &mut C) -> IniResult<()> {
        while !self.end_of_file {
            self.read_line()?;
            let current_line = c_str(&self.buffer).to_vec();
            if let Some(token) = self.strtok(true, SEPS) {
                match blocks.iter().find(|b| b.token.as_bytes() == token.as_slice()) {
                    Some(block) => {
                        if let Err(e) = (block.parse)(self, ctx) {
                            let mut m = b"Error parsing INI file '".to_vec();
                            m.extend_from_slice(&self.filename);
                            m.extend_from_slice(b"' (Line: '");
                            m.extend_from_slice(&current_line);
                            m.extend_from_slice(b"')\n");
                            return Err(e.wrap(m));
                        }
                    }
                    None => return err(IniErrorKind::UnknownToken),
                }
            }
        }
        Ok(())
    }

    /// `INI::loadDirectory(dirName, subdirs, loadType, pXfer)`, given the
    /// listing `TheFileSystem->getFileListInDirectory(dirName + "\\",
    /// "*.ini", list, TRUE)` and a way to open each listed file. Files
    /// directly in the directory load first, then those in subdirectories,
    /// each group in the list's order. `subdirs` is ignored, as upstream.
    #[allow(clippy::too_many_arguments)]
    pub fn load_directory<C>(
        dir_name: &[u8],
        _subdirs: bool,
        load_type: IniLoadType,
        mut xfer: Option<&mut dyn Xfer>,
        filename_list: &FilenameList,
        mut open: impl FnMut(&[u8]) -> Option<Vec<u8>>,
        blocks: &[BlockParse<C>],
        ctx: &mut C,
    ) -> IniResult<()> {
        if dir_name.is_empty() {
            return err(IniErrorKind::InvalidDirectory);
        }
        let dir_len = dir_name.len() + 1; // dirName.concat('\\')
        for in_subdir in [false, true] {
            for name in filename_list.iter() {
                if name.len() < dir_len {
                    return undefined(Undefined::FilenameShorterThanDirectory);
                }
                let tempname = &name[dir_len..];
                let nested = tempname.iter().any(|&c| c == b'\\' || c == b'/');
                if nested == in_subdir {
                    let data = open(name).ok_or_else(|| IniError::new(IniErrorKind::CantOpenFile))?;
                    let x: Option<&mut dyn Xfer> = match xfer {
                        Some(ref mut x) => Some(&mut **x),
                        None => None,
                    };
                    Ini::load(name, &data, load_type, x, blocks, ctx)?;
                }
            }
        }
        Ok(())
    }

    /// `INI::readLine`: the next line into the buffer (see the module docs),
    /// xfered into `s_xfer` if there is one.
    pub fn read_line(&mut self) -> IniResult<()> {
        if self.end_of_file {
            self.buffer[0] = 0;
        } else {
            let mut p = 0;
            while p != INI_MAX_CHARS_PER_LINE {
                if self.pos == self.data.len() {
                    self.end_of_file = true;
                    self.buffer[p] = 0;
                    break;
                }
                let mut c = self.data[self.pos];
                self.pos += 1;
                if c == b'\n' {
                    self.buffer[p] = 0;
                    break;
                }
                if c == b';' {
                    c = 0;
                } else if (1..32).contains(&c) {
                    c = b' ';
                }
                self.buffer[p] = c;
                p += 1;
            }
            self.buffer[p] = 0;
            self.line_num = self.line_num.wrapping_add(1);
        }
        if let Some(x) = self.xfer.as_mut() {
            let n = c_str(&self.buffer).len();
            x.xfer_user(&mut self.buffer[..n]).map_err(|s| IniError::new(IniErrorKind::Xfer(s)))?;
        }
        Ok(())
    }

    /// `strtok(start ? m_buffer : NULL, seps)` on the line buffer.
    fn strtok(&mut self, start: bool, seps: &[u8]) -> Option<Vec<u8>> {
        self.strtok_at(start, seps).map(|(_, t)| t)
    }

    /// [`Ini::strtok`], also giving the token's offset in the buffer (the
    /// C++ keeps the pointer, and reads through it later).
    fn strtok_at(&mut self, start: bool, seps: &[u8]) -> Option<(usize, Vec<u8>)> {
        let b = &mut self.buffer;
        let mut s = if start { 0 } else { self.strtok_next };
        while b[s] != 0 && seps.contains(&b[s]) {
            s += 1;
        }
        if b[s] == 0 {
            self.strtok_next = s;
            return None;
        }
        let begin = s;
        while b[s] != 0 && !seps.contains(&b[s]) {
            s += 1;
        }
        let token = b[begin..s].to_vec();
        if b[s] != 0 {
            b[s] = 0;
            self.strtok_next = s + 1;
        } else {
            self.strtok_next = s;
        }
        Some((begin, token))
    }

    /// Not an upstream function: puts `line` (up to its first NUL, at most
    /// `INI_MAX_CHARS_PER_LINE` bytes) in the line buffer and takes its first
    /// token with `strtok(m_buffer, seps)`, as `initFromINI` does with each
    /// field line before calling the field's parser. With it a field parser
    /// runs on one line (the BitFlags parsers, and their tests).
    pub fn begin_line(&mut self, line: &[u8]) -> Option<Vec<u8>> {
        let line = c_str(line);
        let n = line.len().min(INI_MAX_CHARS_PER_LINE);
        self.buffer.iter_mut().for_each(|b| *b = 0);
        self.buffer[..n].copy_from_slice(&line[..n]);
        self.strtok(true, SEPS)
    }

    /// `getNextToken(seps)`: the next token (default separators if `None`);
    /// `INI_INVALID_DATA` if there is none.
    pub fn get_next_token(&mut self, seps: Option<&[u8]>) -> IniResult<Vec<u8>> {
        self.strtok(false, seps.unwrap_or(SEPS)).ok_or_else(|| IniError::new(IniErrorKind::InvalidData))
    }

    /// `getNextTokenOrNull(seps)`.
    pub fn get_next_token_or_null(&mut self, seps: Option<&[u8]>) -> Option<Vec<u8>> {
        self.strtok(false, seps.unwrap_or(SEPS))
    }

    /// `getNextSubToken(expected)`: `Tag:value`, returns `value`.
    pub fn get_next_sub_token(&mut self, expected: &[u8]) -> IniResult<Vec<u8>> {
        let token = self.get_next_token(Some(SEPS_COLON))?;
        if !stricmp_eq(&token, expected) {
            return err(IniErrorKind::InvalidData);
        }
        self.get_next_token(Some(SEPS_COLON))
    }

    /// `getNextAsciiString` (the "broken" one, kept for existing data).
    pub fn get_next_ascii_string(&mut self) -> Vec<u8> {
        let Some(token) = self.get_next_token_or_null(None) else {
            return Vec::new();
        };
        if token[0] != b'"' {
            return token;
        }
        let mut buff = if token.len() > 1 { token[1..].to_vec() } else { Vec::new() };
        match self.get_next_token_or_null(Some(SEPS_QUOTE)) {
            Some(t) => {
                if t.len() > 1 && t[1] != b'\t' {
                    buff.push(b' ');
                }
                buff.extend_from_slice(&t);
            }
            None => {
                if buff.last() == Some(&b'"') {
                    buff.pop();
                }
            }
        }
        buff
    }

    /// `getNextQuotedAsciiString` (the fixed version).
    pub fn get_next_quoted_ascii_string(&mut self) -> IniResult<Vec<u8>> {
        let Some(token) = self.get_next_token_or_null(None) else {
            return Ok(Vec::new());
        };
        if token[0] != b'"' {
            return Ok(token);
        }
        let str_len = token.len();
        let mut buff: Option<Vec<u8>> = None;
        let mut done = false;
        if str_len > 1 {
            let mut b = token[1..].to_vec();
            if b[str_len - 2] == b'"' {
                b.truncate(str_len - 2);
                done = true;
            }
            buff = Some(b);
        }
        if !done {
            let t = self.get_next_token(Some(SEPS_QUOTE))?;
            let Some(mut b) = buff else {
                return undefined(Undefined::UninitialisedQuoteBuffer);
            };
            if t.len() > 1 && t[1] != b'\t' {
                b.push(b' ');
                b.extend_from_slice(&t);
            } else if b.last() == Some(&b'"') {
                b.pop();
            }
            return Ok(b);
        }
        Ok(buff.unwrap_or_default())
    }

    // -- the scanners (static upstream) ------------------------------------

    /// `scanInt`: `sscanf("%d")`.
    pub fn scan_int(token: &[u8]) -> IniResult<i32> {
        let (neg, mag) = scan_decimal(token).ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let v = if neg { -i128::from(mag) } else { i128::from(mag) };
        i32::try_from(v).or_else(|_| undefined(Undefined::IntOutOfRange))
    }

    /// `scanUnsignedInt`: `sscanf("%u")`, which negates in unsigned
    /// arithmetic (`"-1"` is 4294967295).
    pub fn scan_unsigned_int(token: &[u8]) -> IniResult<u32> {
        let (neg, mag) = scan_decimal(token).ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let m = u32::try_from(mag).or_else(|_| undefined(Undefined::UnsignedOutOfRange))?;
        Ok(if neg { m.wrapping_neg() } else { m })
    }

    /// `scanReal`: `sscanf("%f")`.
    pub fn scan_real(token: &[u8]) -> IniResult<f32> {
        scan_float(token)
    }

    /// `scanPercentToReal`: `sscanf("%f") / 100.0f`.
    pub fn scan_percent_to_real(token: &[u8]) -> IniResult<f32> {
        Ok(scan_float(token)? / 100.0)
    }

    /// `scanBool`: `yes` or `no`, any case.
    pub fn scan_bool(token: &[u8]) -> IniResult<bool> {
        if stricmp_eq(token, b"yes") {
            Ok(true)
        } else if stricmp_eq(token, b"no") {
            Ok(false)
        } else {
            err(IniErrorKind::InvalidData)
        }
    }

    /// `scanIndexList`: the index of `token` in `names` (`stricmp`).
    pub fn scan_index_list(token: &[u8], names: &[&str]) -> IniResult<i32> {
        if names.is_empty() {
            return err(IniErrorKind::InvalidNameList);
        }
        match names.iter().position(|n| stricmp_eq(n.as_bytes(), token)) {
            Some(i) => Ok(i as i32),
            None => err(IniErrorKind::InvalidData),
        }
    }

    /// `scanLookupList`: the value of `token` in `lookup` (`stricmp`).
    pub fn scan_lookup_list(token: &[u8], lookup: &[LookupListRec]) -> IniResult<i32> {
        if lookup.is_empty() {
            return err(IniErrorKind::InvalidNameList);
        }
        match lookup.iter().find(|r| stricmp_eq(r.name.as_bytes(), token)) {
            Some(r) => Ok(r.value),
            None => err(IniErrorKind::InvalidData),
        }
    }

    /// `isValidINIFilename`: ends in `ini` (any case), at least 3 bytes.
    pub fn is_valid_ini_filename(filename: &[u8]) -> bool {
        filename.len() >= 3 && filename[filename.len() - 3..].eq_ignore_ascii_case(b"ini")
    }

    /// `isDeclarationOfType(blockType, blockName, buffer)`: whether the line
    /// is `[ws] blockType [ws] blockName [ws]` (types and names compared
    /// case-insensitively; at least one byte must follow each).
    pub fn is_declaration_of_type(block_type: &[u8], block_name: &[u8], buffer: &[u8]) -> IniResult<bool> {
        if block_type.is_empty() || block_name.is_empty() {
            return Ok(false);
        }
        let buf = c_str(buffer);
        let mut ret = true;
        let mut t = skip_c_space(buf, 0)?;
        if buf.len() - t > block_type.len() {
            if !stricmp_eq(block_type, &buf[t..t + block_type.len()]) {
                ret = false;
            }
            t += block_type.len();
        } else {
            ret = false;
        }
        t = skip_c_space(buf, t)?;
        if buf.len() - t > block_name.len() {
            if !stricmp_eq(block_name, &buf[t..t + block_name.len()]) {
                ret = false;
            }
            t += block_name.len();
        } else {
            ret = false;
        }
        trailing_space(buf, t, ret)
    }

    /// `isEndOfBlock(buffer)`: whether the line is `[ws] End [ws]` with at
    /// least one byte after `End`.
    pub fn is_end_of_block(buffer: &[u8]) -> IniResult<bool> {
        const END: &[u8] = b"End";
        let buf = c_str(buffer);
        let mut ret = true;
        let mut t = skip_c_space(buf, 0)?;
        if buf.len() - t > END.len() {
            if !stricmp_eq(END, &buf[t..t + END.len()]) {
                ret = false;
            }
            t += END.len();
        } else {
            ret = false;
        }
        trailing_space(buf, t, ret)
    }

    // -- the parse-table mechanism ------------------------------------------

    /// `initFromINI(what, parseTable)`.
    pub fn init_from_ini<T>(&mut self, what: &mut T, parse_table: &[FieldParse<T>]) -> IniResult<()> {
        let mut p = MultiIniFieldParse::new();
        p.add(parse_table)?;
        self.init_from_ini_multi(what, &p)
    }

    /// `initFromINIMultiProc(what, proc)`.
    pub fn init_from_ini_multi_proc<'t, T: 't>(
        &mut self,
        what: &mut T,
        proc_: impl FnOnce(&mut MultiIniFieldParse<'t, T>) -> IniResult<()>,
    ) -> IniResult<()> {
        let mut p = MultiIniFieldParse::new();
        proc_(&mut p)?;
        self.init_from_ini_multi(what, &p)
    }

    /// `initFromINIMulti(what, parseTableList)`: read field lines into
    /// `what` until `End`.
    pub fn init_from_ini_multi<T>(&mut self, what: &mut T, list: &MultiIniFieldParse<'_, T>) -> IniResult<()> {
        let mut done = false;
        while !done {
            self.read_line()?;
            if let Some((field_at, field)) = self.strtok_at(true, SEPS) {
                if stricmp_eq(&field, BLOCK_END_TOKEN) {
                    done = true;
                } else {
                    let mut found = false;
                    for table in &list.tables {
                        if let Some((parse, user_data)) = find_field_parse(table, &field) {
                            if let Err(e) = parse(self, what, &user_data) {
                                let mut m = format!("[LINE: {} - FILE: '", self.line_num as i32).into_bytes();
                                m.extend_from_slice(&self.filename);
                                m.extend_from_slice(b"'] Error reading field '");
                                // `field` points into m_buffer: if the parser read
                                // more lines (a nested block), it now names
                                // whatever the current line holds there.
                                m.extend_from_slice(c_str(&self.buffer[field_at..]));
                                m.extend_from_slice(b"'\n");
                                return Err(e.wrap(m));
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return err(IniErrorKind::UnknownToken);
                    }
                }
            }
            if !done && self.end_of_file {
                return err(IniErrorKind::MissingEndToken);
            }
        }
        Ok(())
    }

    // -- the field parsers (they take the value tokens from the line) -------

    /// `parseUnsignedByte`: 0..=255.
    pub fn parse_unsigned_byte(&mut self) -> IniResult<u8> {
        let v = Self::scan_int(&self.get_next_token(None)?)?;
        if !(0..=255).contains(&v) {
            return err(IniErrorKind::Bug);
        }
        Ok(v as u8)
    }

    /// `parseShort`.
    pub fn parse_short(&mut self) -> IniResult<i16> {
        let v = Self::scan_int(&self.get_next_token(None)?)?;
        i16::try_from(v).or_else(|_| err(IniErrorKind::Bug))
    }

    /// `parseUnsignedShort`.
    pub fn parse_unsigned_short(&mut self) -> IniResult<u16> {
        let v = Self::scan_int(&self.get_next_token(None)?)?;
        u16::try_from(v).or_else(|_| err(IniErrorKind::Bug))
    }

    /// `parseInt`.
    pub fn parse_int(&mut self) -> IniResult<i32> {
        Self::scan_int(&self.get_next_token(None)?)
    }

    /// `parseUnsignedInt`.
    pub fn parse_unsigned_int(&mut self) -> IniResult<u32> {
        Self::scan_unsigned_int(&self.get_next_token(None)?)
    }

    /// `parseReal`.
    pub fn parse_real(&mut self) -> IniResult<f32> {
        Self::scan_real(&self.get_next_token(None)?)
    }

    /// `parsePositiveNonZeroReal`: `INI_INVALID_DATA` for `<= 0`.
    pub fn parse_positive_non_zero_real(&mut self) -> IniResult<f32> {
        let v = Self::scan_real(&self.get_next_token(None)?)?;
        if v <= 0.0 {
            return err(IniErrorKind::InvalidData);
        }
        Ok(v)
    }

    /// `parseAngleReal`: degrees to radians.
    pub fn parse_angle_real(&mut self) -> IniResult<f32> {
        const RADS_PER_DEGREE: f32 = PI / 180.0;
        Ok(Self::scan_real(&self.get_next_token(None)?)? * RADS_PER_DEGREE)
    }

    /// `parseAngularVelocityReal`: degrees per second to radians per frame.
    pub fn parse_angular_velocity_real(&mut self) -> IniResult<f32> {
        let v = Self::scan_real(&self.get_next_token(None)?)?;
        Ok(convert_angular_velocity_in_degrees_per_sec_to_rads_per_frame(v))
    }

    /// `parseBool`.
    pub fn parse_bool(&mut self) -> IniResult<bool> {
        Self::scan_bool(&self.get_next_token(None)?)
    }

    /// `parseBitInInt32`: set `mask` in `store` for `yes`, clear it for `no`.
    pub fn parse_bit_in_int32(&mut self, store: &mut u32, mask: u32) -> IniResult<()> {
        if Self::scan_bool(&self.get_next_token(None)?)? {
            *store |= mask;
        } else {
            *store &= !mask;
        }
        Ok(())
    }

    /// `parseAsciiString` (`getNextAsciiString`).
    pub fn parse_ascii_string(&mut self) -> IniResult<Vec<u8>> {
        Ok(self.get_next_ascii_string())
    }

    /// `parseQuotedAsciiString` (`getNextQuotedAsciiString`).
    pub fn parse_quoted_ascii_string(&mut self) -> IniResult<Vec<u8>> {
        self.get_next_quoted_ascii_string()
    }

    /// `parseAsciiStringVector`: the rest of the line's tokens, replacing.
    pub fn parse_ascii_string_vector(&mut self, store: &mut Vec<Vec<u8>>) -> IniResult<()> {
        store.clear();
        self.parse_ascii_string_vector_append(store)
    }

    /// `parseAsciiStringVectorAppend`: the rest of the line's tokens, added.
    pub fn parse_ascii_string_vector_append(&mut self, store: &mut Vec<Vec<u8>>) -> IniResult<()> {
        while let Some(t) = self.get_next_token_or_null(None) {
            store.push(t);
        }
        Ok(())
    }

    /// `parseSoundsList`: the rest of the line split at `" \t,="`.
    pub fn parse_sounds_list(&mut self, store: &mut Vec<Vec<u8>>) -> IniResult<()> {
        store.clear();
        while let Some(t) = self.get_next_token_or_null(Some(SEPS_SOUNDS)) {
            store.push(t);
        }
        Ok(())
    }

    /// `parsePercentToReal`: `"23%"` is 0.23.
    pub fn parse_percent_to_real(&mut self) -> IniResult<f32> {
        Self::scan_percent_to_real(&self.get_next_token(Some(SEPS_PERCENT))?)
    }

    /// The loop of `parseBitString32` on `*bits`, where `None` is
    /// `parseBitString8`'s uninitialised `tmp`.
    fn bit_string(&mut self, bits: &mut Option<u32>, names: &[&str]) -> IniResult<()> {
        if names.is_empty() {
            return err(IniErrorKind::InvalidNameList);
        }
        let mut found_normal = false;
        let mut found_add_or_sub = false;
        while let Some(token) = self.get_next_token_or_null(None) {
            if stricmp_eq(&token, b"NONE") {
                if found_normal || found_add_or_sub {
                    return err(IniErrorKind::InvalidNameList);
                }
                *bits = Some(0);
                break;
            }
            if token[0] == b'+' || token[0] == b'-' {
                if found_normal {
                    return err(IniErrorKind::InvalidNameList);
                }
                let bit = int_bit(Self::scan_index_list(&token[1..], names)?)?;
                let Some(b) = bits.as_mut() else {
                    return undefined(Undefined::UninitialisedBitString8);
                };
                if token[0] == b'+' {
                    *b |= bit;
                } else {
                    *b &= !bit;
                }
                found_add_or_sub = true;
            } else {
                if found_add_or_sub {
                    return err(IniErrorKind::InvalidNameList);
                }
                if !found_normal {
                    *bits = Some(0);
                }
                let bit = int_bit(Self::scan_index_list(&token, names)?)?;
                if let Some(b) = bits.as_mut() {
                    *b |= bit;
                }
                found_normal = true;
            }
        }
        Ok(())
    }

    /// `parseBitString32`: names from `names` as bits of `store`. A plain
    /// list replaces the value; `+NAME`/`-NAME` set and clear bits in it;
    /// `NONE` alone clears it.
    pub fn parse_bit_string32(&mut self, store: &mut u32, names: &[&str]) -> IniResult<()> {
        let mut bits = Some(*store);
        let r = self.bit_string(&mut bits, names);
        if let Some(b) = bits {
            *store = b;
        }
        r
    }

    /// `parseBitString8`: as [`Ini::parse_bit_string32`] into a temporary,
    /// which must fit 8 bits (`ERROR_BUG`). **Upstream bug:** the temporary
    /// is uninitialised, so `+`/`-` lists and empty lists are undefined.
    pub fn parse_bit_string8(&mut self, names: &[&str]) -> IniResult<u8> {
        let mut tmp = None;
        self.bit_string(&mut tmp, names)?;
        let Some(t) = tmp else {
            return undefined(Undefined::UninitialisedBitString8);
        };
        if t & 0xffff_ff00 != 0 {
            return err(IniErrorKind::Bug);
        }
        Ok(t as u8)
    }

    /// The `R:`, `G:`, `B:` (and optional `A:`) components of
    /// `parseRGBAColorInt` and `parseColorInt`, each 0..=255.
    fn color_components(&mut self) -> IniResult<[i32; 4]> {
        const NAMES: [&[u8]; 4] = [b"R", b"G", b"B", b"A"];
        let mut colors = [0i32; 4];
        for i in 0..4 {
            match self.get_next_token_or_null(Some(SEPS_COLON)) {
                None => {
                    if i < 3 {
                        return err(IniErrorKind::InvalidData);
                    }
                    colors[i] = 255;
                }
                Some(t) => {
                    if !stricmp_eq(&t, NAMES[i]) {
                        return err(IniErrorKind::InvalidData);
                    }
                    colors[i] = Self::scan_int(&self.get_next_token(Some(SEPS_COLON))?)?;
                }
            }
            if !(0..=255).contains(&colors[i]) {
                return err(IniErrorKind::InvalidData);
            }
        }
        Ok(colors)
    }

    /// `parseRGBColor`: `R:100 G:114 B:245` as fractions of 255.
    pub fn parse_rgb_color(&mut self) -> IniResult<RGBColor> {
        const NAMES: [&[u8]; 3] = [b"R", b"G", b"B"];
        let mut colors = [0i32; 3];
        for i in 0..3 {
            colors[i] = Self::scan_int(&self.get_next_sub_token(NAMES[i])?)?;
            if !(0..=255).contains(&colors[i]) {
                return err(IniErrorKind::InvalidData);
            }
        }
        Ok(RGBColor {
            red: colors[0] as f32 / 255.0,
            green: colors[1] as f32 / 255.0,
            blue: colors[2] as f32 / 255.0,
        })
    }

    /// `parseRGBAColorInt`: `R:100 G:114 B:245 [A:233]`, alpha 255 if absent.
    pub fn parse_rgba_color_int(&mut self) -> IniResult<RGBAColorInt> {
        let c = self.color_components()?;
        Ok(RGBAColorInt { red: c[0] as u32, green: c[1] as u32, blue: c[2] as u32, alpha: c[3] as u32 })
    }

    /// `parseColorInt`: as `parseRGBAColorInt`, packed by `GameMakeColor`.
    pub fn parse_color_int(&mut self) -> IniResult<i32> {
        let c = self.color_components()?;
        Ok(game_make_color(c[0] as u8, c[1] as u8, c[2] as u8, c[3] as u8))
    }

    /// `parseCoord3D`: `X:400 Y:-214.3 Z:8.6`.
    pub fn parse_coord3d(&mut self) -> IniResult<Coord3D> {
        let x = Self::scan_real(&self.get_next_sub_token(b"X")?)?;
        let y = Self::scan_real(&self.get_next_sub_token(b"Y")?)?;
        let z = Self::scan_real(&self.get_next_sub_token(b"Z")?)?;
        Ok(Coord3D { x, y, z })
    }

    /// `parseCoord2D`: `X:400 Y:-214.3`.
    pub fn parse_coord2d(&mut self) -> IniResult<Coord2D> {
        let x = Self::scan_real(&self.get_next_sub_token(b"X")?)?;
        let y = Self::scan_real(&self.get_next_sub_token(b"Y")?)?;
        Ok(Coord2D { x, y })
    }

    /// `parseICoord2D`: `X:400 Y:-214`.
    pub fn parse_icoord2d(&mut self) -> IniResult<ICoord2D> {
        let x = Self::scan_int(&self.get_next_sub_token(b"X")?)?;
        let y = Self::scan_int(&self.get_next_sub_token(b"Y")?)?;
        Ok(ICoord2D { x, y })
    }

    /// `parseIndexList`: the token's index in `names`.
    pub fn parse_index_list(&mut self, names: &[&str]) -> IniResult<i32> {
        Self::scan_index_list(&self.get_next_token(None)?, names)
    }

    /// `parseByteSizedIndexList`: as `parseIndexList`, 0..=255 (`ERROR_BUG`).
    pub fn parse_byte_sized_index_list(&mut self, names: &[&str]) -> IniResult<u8> {
        let v = Self::scan_index_list(&self.get_next_token(None)?, names)?;
        if !(0..=255).contains(&v) {
            return err(IniErrorKind::Bug);
        }
        Ok(v as u8)
    }

    /// `parseLookupList`: the token's value in `lookup`.
    pub fn parse_lookup_list(&mut self, lookup: &[LookupListRec]) -> IniResult<i32> {
        Self::scan_lookup_list(&self.get_next_token(None)?, lookup)
    }

    /// `parseGameClientRandomVariable`: `low high [distribution]`.
    pub fn parse_game_client_random_variable(&mut self, var: &mut GameClientRandomVariable) -> IniResult<()> {
        let low = Self::scan_real(&self.get_next_token(None)?)?;
        let high = Self::scan_real(&self.get_next_token(None)?)?;
        let mut kind = DistributionType::Uniform;
        if let Some(t) = self.get_next_token_or_null(None) {
            kind = match Self::scan_index_list(&t, &DISTRIBUTION_TYPE_NAMES)? {
                0 => DistributionType::Constant,
                1 => DistributionType::Uniform,
                2 => DistributionType::Gaussian,
                3 => DistributionType::Triangular,
                4 => DistributionType::LowBias,
                _ => DistributionType::HighBias,
            };
        }
        var.set_range(low, high, kind);
        Ok(())
    }

    /// `parseDurationReal`: milliseconds to frames.
    pub fn parse_duration_real(&mut self) -> IniResult<f32> {
        let v = Self::scan_real(&self.get_next_token(None)?)?;
        Ok(convert_duration_from_msecs_to_frames(v))
    }

    /// `parseDurationUnsignedInt`: milliseconds to whole frames, rounding up.
    pub fn parse_duration_unsigned_int(&mut self) -> IniResult<u32> {
        let v = Self::scan_unsigned_int(&self.get_next_token(None)?)?;
        // At most ceil(4294967296 * 0.03), always in range of UnsignedInt.
        Ok(Real::ceil(convert_duration_from_msecs_to_frames(v as f32)) as u32)
    }

    /// `parseDurationUnsignedShort`: milliseconds to whole frames, rounding
    /// up, into an `UnsignedShort` (undefined above 65535 frames).
    pub fn parse_duration_unsigned_short(&mut self) -> IniResult<u16> {
        let v = Self::scan_unsigned_int(&self.get_next_token(None)?)?;
        let frames = Real::ceil(convert_duration_from_msecs_to_frames(v as f32));
        if frames >= 65536.0 {
            return undefined(Undefined::FloatToIntOutOfRange);
        }
        Ok(frames as u16)
    }

    /// `parseVelocityReal`: per second to per frame.
    pub fn parse_velocity_real(&mut self) -> IniResult<f32> {
        let v = Self::scan_real(&self.get_next_token(None)?)?;
        Ok(convert_velocity_in_secs_to_frames(v))
    }

    /// `parseAccelerationReal`: per second squared to per frame squared.
    pub fn parse_acceleration_real(&mut self) -> IniResult<f32> {
        let v = Self::scan_real(&self.get_next_token(None)?)?;
        Ok(convert_acceleration_in_secs_to_frames(v))
    }

    /// `parseVeterancyLevelFlags`: `ALL`, `NONE`, `+LEVEL`, `-LEVEL`,
    /// starting from all; at least one token.
    pub fn parse_veterancy_level_flags(&mut self) -> IniResult<VeterancyLevelFlags> {
        let mut flags = VETERANCY_LEVEL_FLAGS_ALL;
        let mut token = Some(self.get_next_token(None)?);
        while let Some(t) = token {
            if stricmp_eq(&t, b"ALL") {
                flags = VETERANCY_LEVEL_FLAGS_ALL;
            } else if stricmp_eq(&t, b"NONE") {
                flags = VETERANCY_LEVEL_FLAGS_NONE;
            } else if t[0] == b'+' {
                let dt = Self::scan_index_list(&t[1..], &VETERANCY_NAMES)?;
                flags = set_veterancy_level_flag(flags, dt);
            } else if t[0] == b'-' {
                let dt = Self::scan_index_list(&t[1..], &VETERANCY_NAMES)?;
                flags = clear_veterancy_level_flag(flags, dt);
            } else {
                return err(IniErrorKind::UnknownToken);
            }
            token = self.get_next_token_or_null(None);
        }
        Ok(flags)
    }

    /// `parseDeathTypeFlags`: `ALL`, `NONE`, `+TYPE`, `-TYPE`, starting
    /// from all; at least one token.
    pub fn parse_death_type_flags(&mut self) -> IniResult<DeathTypeFlags> {
        let mut flags = DEATH_TYPE_FLAGS_ALL;
        let mut token = Some(self.get_next_token(None)?);
        while let Some(t) = token {
            if stricmp_eq(&t, b"ALL") {
                flags = DEATH_TYPE_FLAGS_ALL;
            } else if stricmp_eq(&t, b"NONE") {
                flags = DEATH_TYPE_FLAGS_NONE;
            } else if t[0] == b'+' {
                let dt = Self::scan_index_list(&t[1..], &DEATH_NAMES)?;
                flags = set_death_type_flag(flags, dt);
            } else if t[0] == b'-' {
                let dt = Self::scan_index_list(&t[1..], &DEATH_NAMES)?;
                flags = clear_death_type_flag(flags, dt);
            } else {
                return err(IniErrorKind::UnknownToken);
            }
            token = self.get_next_token_or_null(None);
        }
        Ok(flags)
    }

    /// `parseDamageTypeFlags`: `ALL`, `NONE`, `+TYPE`, `-TYPE`, starting
    /// from all; at least one token. An unknown type throws
    /// `std::out_of_range` (see the module docs).
    pub fn parse_damage_type_flags(&mut self) -> IniResult<DamageTypeFlags> {
        let oor = |_| IniError::new(IniErrorKind::BitIndexOutOfRange);
        let mut flags = DamageTypeFlags::NONE;
        flags.flip();
        let mut token = Some(self.get_next_token(None)?);
        while let Some(t) = token {
            if stricmp_eq(&t, b"ALL") {
                flags = DamageTypeFlags::NONE;
                flags.flip();
            } else if stricmp_eq(&t, b"NONE") {
                flags = DamageTypeFlags::NONE;
            } else if t[0] == b'+' {
                let dt = DamageTypeFlags::get_single_bit_from_name(&t[1..]);
                flags = set_damage_type_flag(flags, dt).map_err(oor)?;
            } else if t[0] == b'-' {
                let dt = DamageTypeFlags::get_single_bit_from_name(&t[1..]);
                flags = clear_damage_type_flag(flags, dt).map_err(oor)?;
            } else {
                return err(IniErrorKind::UnknownToken);
            }
            token = self.get_next_token_or_null(None);
        }
        Ok(flags)
    }
}

/// `while (isspace(*tempBuff)) ++tempBuff;` from `t`.
fn skip_c_space(buf: &[u8], mut t: usize) -> IniResult<usize> {
    while t < buf.len() {
        if buf[t] >= 0x80 {
            return undefined(Undefined::IsspaceNonAscii);
        }
        if !is_c_space(buf[t]) {
            break;
        }
        t += 1;
    }
    Ok(t)
}

/// `while (strlen(tempBuff)) { retVal = retVal && isspace(tempBuff[0]); ++tempBuff; }`
/// (`isspace` is only evaluated while `retVal` holds).
fn trailing_space(buf: &[u8], mut t: usize, mut ret: bool) -> IniResult<bool> {
    while t < buf.len() {
        if ret {
            if buf[t] >= 0x80 {
                return undefined(Undefined::IsspaceNonAscii);
            }
            ret = is_c_space(buf[t]);
        }
        t += 1;
    }
    Ok(ret)
}

/// `findFieldParse(parseTable, token, offset, userData)`.
fn find_field_parse<T>(table: &[FieldParse<T>], token: &[u8]) -> Option<(FieldParseFn<T>, UserData)> {
    for entry in table {
        match entry.token {
            Some(t) => {
                if t.as_bytes() == token {
                    return entry.parse.map(|p| (p, entry.user_data.clone()));
                }
            }
            None => return entry.parse.map(|p| (p, UserData::Token(token.to_vec()))),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::xfer_crc::XferCrc;

    #[derive(Default, Debug)]
    struct Rec {
        name: Vec<u8>,
        i: i32,
        r: f32,
        s: Vec<u8>,
        bits: u32,
        fields: Vec<Vec<u8>>,
    }

    static NAMES: [&str; 3] = ["ALPHA", "BETA", "GAMMA"];

    static FIELDS: [FieldParse<Rec>; 6] = [
        FieldParse::new("Int", |ini, r, _| { r.i = ini.parse_int()?; Ok(()) }, UserData::None),
        FieldParse::new("Real", |ini, r, _| { r.r = ini.parse_real()?; Ok(()) }, UserData::None),
        FieldParse::new("Str", |ini, r, _| { r.s = ini.parse_quoted_ascii_string()?; Ok(()) }, UserData::None),
        FieldParse::new("Bits", |ini, r, ud| ini.parse_bit_string32(&mut r.bits, ud.names()), UserData::Names(&NAMES)),
        FieldParse::without_parse("Dead"),
        FieldParse::catch_all(|_, r, ud| {
            if let UserData::Token(t) = ud {
                r.fields.push(t.clone());
            }
            Ok(())
        }),
    ];

    fn block(ini: &mut Ini<'_>, out: &mut Vec<Rec>) -> IniResult<()> {
        let mut r = Rec { name: ini.get_next_token(None)?, ..Rec::default() };
        ini.init_from_ini(&mut r, &FIELDS)?;
        out.push(r);
        Ok(())
    }

    static BLOCKS: [BlockParse<Vec<Rec>>; 1] = [BlockParse { token: "Thing", parse: block }];

    #[test]
    fn a_block_with_fields() {
        let text = b"; header\nThing Foo\n  Int = -12 ; c\n  Real = 2.5e1\n  Str = \"Hello  World\"\n  Bits = BETA gamma\n  Other = 1\nEND\n";
        let mut out = Vec::new();
        let mut crc = XferCrc::new();
        Ini::load(b"t.ini", text, IniLoadType::Overwrite, Some(&mut crc), &BLOCKS, &mut out).unwrap();
        assert_eq!(out.len(), 1);
        let r = &out[0];
        assert_eq!((r.name.as_slice(), r.i, r.r, r.s.as_slice(), r.bits), (&b"Foo"[..], -12, 25.0, &b"Hello  World"[..], 6));
        assert_eq!(r.fields, vec![b"Other".to_vec()]);
        assert_ne!(crc.get_crc(), 0);
    }

    #[test]
    fn errors_are_wrapped_as_upstream() {
        let mut out = Vec::new();
        let e = Ini::load(b"t.ini", b"Thing A\nInt = x\nEnd\n", IniLoadType::Overwrite, None, &BLOCKS, &mut out).unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        assert_eq!(e.thrown(), Some(Thrown::Exception(b"Error parsing INI file 't.ini' (Line: 'Thing A')\n".to_vec())));
        let e = Ini::load(b"t.ini", b"Thing A\nDead 1\nEnd\n", IniLoadType::Overwrite, None, &BLOCKS, &mut out).unwrap_err();
        assert_eq!(e.kind, IniErrorKind::UnknownToken);
        let e = Ini::load(b"t.ini", b"Nope\n", IniLoadType::Overwrite, None, &BLOCKS, &mut out).unwrap_err();
        assert_eq!(e.thrown(), Some(Thrown::Status(ERROR_BAD_INI)));
        let e = Ini::load(b"t.ini", b"Thing A\nInt 1", IniLoadType::Overwrite, None, &BLOCKS, &mut out).unwrap_err();
        assert_eq!(e.kind, IniErrorKind::MissingEndToken);
    }

    #[test]
    fn scanners() {
        assert_eq!(Ini::scan_int(b"12abc"), Ok(12));
        assert_eq!(Ini::scan_int(b"2147483648").unwrap_err().kind, IniErrorKind::Undefined(Undefined::IntOutOfRange));
        assert_eq!(Ini::scan_unsigned_int(b"-1"), Ok(u32::MAX));
        assert_eq!(Ini::scan_real(b"1e"), Ok(1.0));
        assert_eq!(Ini::scan_real(b"-.5x"), Ok(-0.5));
        assert_eq!(Ini::scan_real(b"."), Err(IniError::new(IniErrorKind::InvalidData)));
        assert!(Ini::scan_real(b"inf").unwrap_err().is_undefined());
        assert!(Ini::scan_real(b"1e39").unwrap_err().is_undefined());
        assert_eq!(Ini::scan_percent_to_real(b"50"), Ok(0.5));
        assert_eq!(Ini::is_end_of_block(b"  end  "), Ok(true));
        assert_eq!(Ini::is_end_of_block(b"End"), Ok(false));
        assert_eq!(Ini::is_declaration_of_type(b"Object", b"Tank", b" object tank "), Ok(true));
        assert!(Ini::is_valid_ini_filename(b"a.INI"));
    }
}
