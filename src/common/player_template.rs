// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/PlayerTemplate.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerTemplate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Westwood Studios Pacific, Steven Johnson, October 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PlayerTemplate` (one faction or side: its names, colour, starting money,
//! starting building and units, production discounts, intrinsic sciences,
//! command sets, screens and music), `PlayerTemplateStore`
//! (`ThePlayerTemplateStore`, every template read from INI) and the
//! `PlayerTemplate` INI block with its field parsers `parseStartMoney`,
//! `parseProductionCostChange`, `parseProductionTimeChange` and
//! `parseProductionVeterancyLevel`.
//!
//! # Translation choices
//!
//! - **Name keys.** A template is identified by the name key of its internal
//!   name, and the production maps are keyed by the name key of a thing
//!   template's name. `NAMEKEY(name)` **assigns a key to a name it has not
//!   seen**, and key values are lockstep state, so everything that upstream
//!   does through `TheNameKeyGenerator` takes the [`NameKeyGenerator`], in
//!   upstream's order of calls (`parseProductionCostChange` takes the key of
//!   the thing's name *before* it reads the percentage, so a bad percentage
//!   still leaves the name with a key).
//! - **Strings** are byte vectors, as in [`crate::common::terrain_types`]. A
//!   field that is not written by the INI stays the empty string.
//! - **The maps.** `ProductionChangeMap` and `ProductionVeterancyMap` are
//!   `std::map<NameKeyType, _>`; here ordered [`BTreeMap`]s, iterated in
//!   key order as upstream.
//! - **`Handicap` and `Money`.** `m_handicap` is only ever default-built
//!   (1.0 everywhere): no INI field writes it. `m_money` is written by
//!   `StartMoney`: `parseStartMoney` parses an `Int`, calls `Money::init()`
//!   and `Money::deposit(amount)` with the default `playSound = TRUE`. The
//!   `Int` is converted to the `UnsignedInt` parameter, so a negative amount
//!   wraps (`-1` is 4294967295). The deposit's cash sound goes to a
//!   [`ClientEvent`] queue that is dropped here (a template is not a player,
//!   and the sound is the client's business, see [`crate::common::money`]),
//!   and the `recordIncome()` call on `ThePlayerList->getNthPlayer(0)` is
//!   left out: `ThePlayerList` is created after the player templates are read
//!   (`GameEngine::init`, `initSubsystem(ThePlayerTemplateStore, ...)` before
//!   `initSubsystem(ThePlayerList, ...)`), so no player exists to record it.
//!   The amount is what is kept.
//! - **`IntrinsicSciences`** is `INI::parseScienceVector`, which checks every
//!   name against `TheScienceStore`: [`ScienceStore::parse_science_vector`]
//!   on the store the handler is given. The names read before a bad one stay
//!   in the template, as upstream.
//! - **`DisplayName`** is `INI::parseAndTranslateLabel`
//!   ([`parse_and_translate_label`]): the translation comes from the
//!   [`GameTextFetch`] the handler is given, and an empty one throws
//!   `INI_INVALID_DATA` with the field untouched.
//! - **`StartingUnit0` to `StartingUnit9`** write `m_startingUnits[i]`; the
//!   slot is the entry's [`UserData::Mask`] where upstream has
//!   `offsetof( PlayerTemplate, m_startingUnits[i] )`.
//! - **The block.** `PlayerTemplateStore::parsePlayerTemplateDefinition` reads
//!   the name, then `findPlayerTemplate(namekey)`; a template that is found is
//!   parsed **in place** and given `namekey` afterwards, one that is not is
//!   parsed as a new `PlayerTemplate` and appended. A block that fails leaves
//!   a found template with the fields read so far and its old name key, and
//!   leaves no trace of a new one.
//! - **`init()`** clears the templates; **`reset()`** keeps them ("we want to
//!   retain this info").
//!
//! # Upstream quirks, kept
//!
//! - **The old-map names.** `findPlayerTemplate` first maps the names of the
//!   old maps' sub-factions (`FactionAmericaChooseAGeneral`,
//!   `FactionAmericaTankCommand`, ... for America, China and the GLA, twelve
//!   in all) to the base faction. This applies to the INI block as well: a
//!   block `PlayerTemplate FactionAmericaTankCommand` finds the `FactionAmerica`
//!   template, updates it, **and renames it** to `FactionAmericaTankCommand`
//!   (`setNameKey(namekey)` is given the name as written, not the one found).
//!   Afterwards `FactionAmerica` no longer finds it, but the old-map names
//!   map to a name nothing has, and the lookup fails.
//! - **The fifteen keys are function statics.** `static NameKeyType a0 =
//!   NAMEKEY("FactionAmerica")` and its fourteen siblings are initialised the
//!   *first time `findPlayerTemplate` runs* (in the order `a0`..`a4`,
//!   `c0`..`c4`, `g0`..`g4`), and never again: if the name key generator is
//!   reset later, they keep the keys of the old numbering. A name key that
//!   the new numbering gives one of these names then maps wrongly, or not at
//!   all. [`PlayerTemplateStore`] keeps the fifteen keys the same way: once
//!   looked up, they live as long as the store, through [`init`](PlayerTemplateStore::init).
//!   Upstream's statics live as long as the process, so **one store models
//!   the process**; a second store would look its own keys up again.
//! - **`getTemplateNumByName`** compares with `compareNoCase` and returns -1
//!   for an unknown name (its `DEBUG_ASSERTCRASH(NULL, ...)` is compiled out
//!   in the shipped build).
//! - **`getAllSideStrings`** adds each side once to a list of its own, then
//!   appends that list: a side already in the caller's list is added again.
//!
//! # Cut points: images and sounds are names
//!
//! `getHeadWaterMarkImage`, `getFlagWaterMarkImage`, `getEnabledImage`,
//! `getSideIconImage` and `getGeneralImage` resolve a name through
//! `TheMappedImageCollection->findImageByName`, which is the client's
//! business. They are [`PlayerTemplate::head_water_mark_image_name`],
//! [`flag_water_mark_image_name`](PlayerTemplate::flag_water_mark_image_name),
//! [`enabled_image_name`](PlayerTemplate::enabled_image_name),
//! [`side_icon_image_name`](PlayerTemplate::side_icon_image_name) and
//! [`general_image_name`](PlayerTemplate::general_image_name): the name as
//! written. `LoadScreenImage`, `ScoreScreenImage`, the medallions, the music
//! tracks (`LoadScreenMusic`, `ScoreScreenMusic`) are names upstream too
//! (`AsciiString`), and `BeaconName` is the name of a thing template.
//!
//! # Left out: methods that take a `Player` or an `Object`
//!
//! None. No method of `PlayerTemplate` or `PlayerTemplateStore` takes a
//! `Player` or an `Object`: `PlayerTemplate.cpp` includes `Common/Player.h`
//! and never uses it. Layer 7 reads the getters (`Player::init` copies the
//! template's handicap, money, colour and sciences).
//!
//! # Where upstream is undefined
//!
//! - An integer outside `Int` in `StartMoney`,
//!   `SpecialPowerShortcutButtonCount` or `IntrinsicSciencePurchasePoints`
//!   (`sscanf` with `%d`, see [`crate::common::ini`]).
//! - A name key past `0x7fffffff` ([`Undefined::NameKeysExhausted`]).
//!
//! # Verified
//!
//! Against upstream `PlayerTemplate.cpp` and the real INI reader by
//! `tests/differential_player_template.rs` (see there for the method and the
//! results).

use std::collections::BTreeMap;

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::RGBColor;
use crate::common::game_common::{VeterancyLevel, VETERANCY_NAMES};
use crate::common::handicap::Handicap;
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, Undefined, UserData};
use crate::common::money::Money;
use crate::common::name_key_generator::{NameKeyError, NameKeyGenerator, NameKeyType};
use crate::common::science::{parse_and_translate_label, GameTextFetch, ScienceStore, ScienceVec};
use crate::common::subsystem_interface::SubsystemInterface;
use crate::common::unicode_string::UnicodeString;
use crate::logic::client_event::ClientEvent;

/// `MAX_MP_STARTING_UNITS`.
pub const MAX_MP_STARTING_UNITS: usize = 10;

/// `typedef std::map<NameKeyType, Real> ProductionChangeMap`.
pub type ProductionChangeMap = BTreeMap<NameKeyType, f32>;

/// `typedef std::map<NameKeyType, VeterancyLevel> ProductionVeterancyMap`.
pub type ProductionVeterancyMap = BTreeMap<NameKeyType, VeterancyLevel>;

/// The fifteen name keys that `findPlayerTemplate` keeps in function statics:
/// `a0`..`a4`, `c0`..`c4`, `g0`..`g4`.
pub const LEGACY_FACTION_KEY_NAMES: [&str; 15] = [
    "FactionAmerica",
    "FactionAmericaChooseAGeneral",
    "FactionAmericaTankCommand",
    "FactionAmericaSpecialForces",
    "FactionAmericaAirForce",
    "FactionChina",
    "FactionChinaChooseAGeneral",
    "FactionChinaRedArmy",
    "FactionChinaSpecialWeapons",
    "FactionChinaSecretPolice",
    "FactionGLA",
    "FactionGLAChooseAGeneral",
    "FactionGLATerrorCell",
    "FactionGLABiowarCommand",
    "FactionGLAWarlordCommand",
];

/// `(VeterancyLevel)INI::scanIndexList(token, TheVeterancyNames)`.
fn veterancy_from_index(i: i32) -> VeterancyLevel {
    match i {
        0 => VeterancyLevel::Regular,
        1 => VeterancyLevel::Veteran,
        2 => VeterancyLevel::Elite,
        _ => VeterancyLevel::Heroic,
    }
}

/// `strcmp(a, b) == 0` on two C strings: the bytes up to the first NUL.
fn c_str(s: &[u8]) -> &[u8] {
    s.split(|&b| b == 0).next().unwrap_or(&[])
}

/// `class PlayerTemplate`.
#[derive(Clone, Debug)]
pub struct PlayerTemplate {
    /// `m_nameKey`.
    name_key: NameKeyType,
    /// `m_displayName`: the translated `DisplayName`.
    display_name: UnicodeString,
    /// `m_side`.
    side: Vec<u8>,
    /// `m_baseSide`.
    base_side: Vec<u8>,
    /// `m_handicap`: "initial baseline for Player capabilities".
    handicap: Handicap,
    /// `m_money`: "starting credits, if any".
    money: Money,
    /// `m_preferredColor`.
    preferred_color: RGBColor,
    /// `m_startingBuilding`.
    starting_building: Vec<u8>,
    /// `m_startingUnits`.
    starting_units: [Vec<u8>; MAX_MP_STARTING_UNITS],
    /// `m_productionCostChanges`.
    production_cost_changes: ProductionChangeMap,
    /// `m_productionTimeChanges`.
    production_time_changes: ProductionChangeMap,
    /// `m_productionVeterancyLevels`.
    production_veterancy_levels: ProductionVeterancyMap,
    /// `m_intrinsicSciences`: "sciences that this Player will always know".
    intrinsic_sciences: ScienceVec,
    /// `m_purchaseScienceCommandSetRank1`.
    purchase_science_command_set_rank1: Vec<u8>,
    /// `m_purchaseScienceCommandSetRank3`.
    purchase_science_command_set_rank3: Vec<u8>,
    /// `m_purchaseScienceCommandSetRank8`.
    purchase_science_command_set_rank8: Vec<u8>,
    /// `m_specialPowerShortcutCommandSet`.
    special_power_shortcut_command_set: Vec<u8>,
    /// `m_specialPowerShortcutWinName`.
    special_power_shortcut_win_name: Vec<u8>,
    /// `m_specialPowerShortcutButtonCount`.
    special_power_shortcut_button_count: i32,
    /// `m_loadScreenMusic`.
    load_screen_music: Vec<u8>,
    /// `m_scoreScreenMusic`.
    score_screen_music: Vec<u8>,
    /// `m_tooltip`.
    tooltip: Vec<u8>,
    /// `m_observer`.
    observer: bool,
    /// `m_playableSide`.
    playable_side: bool,
    /// `m_oldFaction`: "Faction existed in the original Generals".
    old_faction: bool,
    /// `m_intrinsicSPP`.
    intrinsic_spp: i32,
    /// `m_scoreScreenImage`.
    score_screen_image: Vec<u8>,
    /// `m_loadScreenImage`.
    load_screen_image: Vec<u8>,
    /// `m_headWaterMark`.
    head_water_mark: Vec<u8>,
    /// `m_flagWaterMark`.
    flag_water_mark: Vec<u8>,
    /// `m_enabledImage`.
    enabled_image: Vec<u8>,
    /// `m_sideIconImage`.
    side_icon_image: Vec<u8>,
    /// `m_generalImage`.
    general_image: Vec<u8>,
    /// `m_beaconTemplate`: "ThingTemplate name for beacons".
    beacon_template: Vec<u8>,
    /// `m_strGeneralFeatures`.
    str_general_features: Vec<u8>,
    /// `m_strMedallionNormal`.
    str_medallion_normal: Vec<u8>,
    /// `m_strMedallionHilite`.
    str_medallion_hilite: Vec<u8>,
    /// `m_strMedallionSelected`.
    str_medallion_selected: Vec<u8>,
}

impl Default for PlayerTemplate {
    fn default() -> Self {
        PlayerTemplate::new()
    }
}

impl PlayerTemplate {
    /// `PlayerTemplate()`: no name key (`NAMEKEY_INVALID`), black, not an
    /// observer, not playable, not an old faction, no points, no strings.
    pub fn new() -> Self {
        PlayerTemplate {
            name_key: NameKeyType::INVALID,
            display_name: UnicodeString::new(),
            side: Vec::new(),
            base_side: Vec::new(),
            handicap: Handicap::new(),
            money: Money::new(),
            preferred_color: RGBColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            },
            starting_building: Vec::new(),
            starting_units: Default::default(),
            production_cost_changes: ProductionChangeMap::new(),
            production_time_changes: ProductionChangeMap::new(),
            production_veterancy_levels: ProductionVeterancyMap::new(),
            intrinsic_sciences: Vec::new(),
            purchase_science_command_set_rank1: Vec::new(),
            purchase_science_command_set_rank3: Vec::new(),
            purchase_science_command_set_rank8: Vec::new(),
            special_power_shortcut_command_set: Vec::new(),
            special_power_shortcut_win_name: Vec::new(),
            special_power_shortcut_button_count: 0,
            load_screen_music: Vec::new(),
            score_screen_music: Vec::new(),
            tooltip: Vec::new(),
            observer: false,
            playable_side: false,
            old_faction: false,
            intrinsic_spp: 0,
            score_screen_image: Vec::new(),
            load_screen_image: Vec::new(),
            head_water_mark: Vec::new(),
            flag_water_mark: Vec::new(),
            enabled_image: Vec::new(),
            side_icon_image: Vec::new(),
            general_image: Vec::new(),
            beacon_template: Vec::new(),
            str_general_features: Vec::new(),
            str_medallion_normal: Vec::new(),
            str_medallion_hilite: Vec::new(),
            str_medallion_selected: Vec::new(),
        }
    }

    /// `setNameKey(namekey)`.
    pub fn set_name_key(&mut self, name_key: NameKeyType) {
        self.name_key = name_key;
    }

    /// `getNameKey()`. (Its `DEBUG_ASSERTCRASH` that the key is valid is
    /// compiled out in the shipped build.)
    pub fn name_key(&self) -> NameKeyType {
        self.name_key
    }

    /// `getName()`: `KEYNAME(m_nameKey)`, empty for a key the generator does
    /// not know.
    pub fn name<'a>(&self, names: &'a NameKeyGenerator) -> &'a [u8] {
        names.key_to_name(self.name_key)
    }

    /// `getDisplayName()`.
    pub fn display_name(&self) -> &UnicodeString {
        &self.display_name
    }

    /// `getSide()`.
    pub fn side(&self) -> &[u8] {
        &self.side
    }

    /// `getBaseSide()`.
    pub fn base_side(&self) -> &[u8] {
        &self.base_side
    }

    /// `getHandicap()`: "return the tech tree for the player".
    pub fn handicap(&self) -> &Handicap {
        &self.handicap
    }

    /// `getMoney()`: "return the money for the player".
    pub fn money(&self) -> &Money {
        &self.money
    }

    /// `getPreferredColor()`.
    pub fn preferred_color(&self) -> &RGBColor {
        &self.preferred_color
    }

    /// `getStartingBuilding()`.
    pub fn starting_building(&self) -> &[u8] {
        &self.starting_building
    }

    /// `getStartingUnit(i)`: the empty string for an `i` outside
    /// `[0, MAX_MP_STARTING_UNITS)`.
    pub fn starting_unit(&self, i: i32) -> &[u8] {
        if i < 0 || i >= MAX_MP_STARTING_UNITS as i32 {
            return &[];
        }
        &self.starting_units[i as usize]
    }

    /// `getProductionCostChanges()`.
    pub fn production_cost_changes(&self) -> &ProductionChangeMap {
        &self.production_cost_changes
    }

    /// `getProductionTimeChanges()`.
    pub fn production_time_changes(&self) -> &ProductionChangeMap {
        &self.production_time_changes
    }

    /// `getProductionVeterancyLevels()`.
    pub fn production_veterancy_levels(&self) -> &ProductionVeterancyMap {
        &self.production_veterancy_levels
    }

    /// `isObserver()`.
    pub fn is_observer(&self) -> bool {
        self.observer
    }

    /// `isPlayableSide()`.
    pub fn is_playable_side(&self) -> bool {
        self.playable_side
    }

    /// `getScoreScreen()`: the name of the score screen image.
    pub fn score_screen(&self) -> &[u8] {
        &self.score_screen_image
    }

    /// `getLoadScreen()`: the name of the load screen image.
    pub fn load_screen(&self) -> &[u8] {
        &self.load_screen_image
    }

    /// `getBeaconTemplate()`.
    pub fn beacon_template(&self) -> &[u8] {
        &self.beacon_template
    }

    /// The name `getHeadWaterMarkImage()` looks up (a client cut point).
    pub fn head_water_mark_image_name(&self) -> &[u8] {
        &self.head_water_mark
    }

    /// The name `getFlagWaterMarkImage()` looks up (a client cut point).
    pub fn flag_water_mark_image_name(&self) -> &[u8] {
        &self.flag_water_mark
    }

    /// The name `getEnabledImage()` looks up (a client cut point).
    pub fn enabled_image_name(&self) -> &[u8] {
        &self.enabled_image
    }

    /// The name `getSideIconImage()` looks up (a client cut point).
    pub fn side_icon_image_name(&self) -> &[u8] {
        &self.side_icon_image
    }

    /// The name `getGeneralImage()` looks up (a client cut point).
    pub fn general_image_name(&self) -> &[u8] {
        &self.general_image
    }

    /// `getTooltip()`.
    pub fn tooltip(&self) -> &[u8] {
        &self.tooltip
    }

    /// `getGeneralFeatures()`.
    pub fn general_features(&self) -> &[u8] {
        &self.str_general_features
    }

    /// `getMedallionNormal()`.
    pub fn medallion_normal(&self) -> &[u8] {
        &self.str_medallion_normal
    }

    /// `getMedallionHilite()`.
    pub fn medallion_hilite(&self) -> &[u8] {
        &self.str_medallion_hilite
    }

    /// `getMedallionSelected()`.
    pub fn medallion_selected(&self) -> &[u8] {
        &self.str_medallion_selected
    }

    /// `getIntrinsicSciences()`.
    pub fn intrinsic_sciences(&self) -> &ScienceVec {
        &self.intrinsic_sciences
    }

    /// `getIntrinsicSciencePurchasePoints()`.
    pub fn intrinsic_science_purchase_points(&self) -> i32 {
        self.intrinsic_spp
    }

    /// `getPurchaseScienceCommandSetRank1()`.
    pub fn purchase_science_command_set_rank1(&self) -> &[u8] {
        &self.purchase_science_command_set_rank1
    }

    /// `getPurchaseScienceCommandSetRank3()`.
    pub fn purchase_science_command_set_rank3(&self) -> &[u8] {
        &self.purchase_science_command_set_rank3
    }

    /// `getPurchaseScienceCommandSetRank8()`.
    pub fn purchase_science_command_set_rank8(&self) -> &[u8] {
        &self.purchase_science_command_set_rank8
    }

    /// `getSpecialPowerShortcutCommandSet()`.
    pub fn special_power_shortcut_command_set(&self) -> &[u8] {
        &self.special_power_shortcut_command_set
    }

    /// `getSpecialPowerShortcutWinName()`.
    pub fn special_power_shortcut_win_name(&self) -> &[u8] {
        &self.special_power_shortcut_win_name
    }

    /// `getSpecialPowerShortcutButtonCount()`.
    pub fn special_power_shortcut_button_count(&self) -> i32 {
        self.special_power_shortcut_button_count
    }

    /// `getLoadScreenMusic()`.
    pub fn load_screen_music(&self) -> &[u8] {
        &self.load_screen_music
    }

    /// `getScoreScreenMusic()`.
    pub fn score_screen_music(&self) -> &[u8] {
        &self.score_screen_music
    }

    /// `isOldFaction()`.
    pub fn is_old_faction(&self) -> bool {
        self.old_faction
    }
}

// ---------------------------------------------------------------------------
// The field parsers
// ---------------------------------------------------------------------------

/// What the fields of a `PlayerTemplate` block fill: the template, and what
/// the field parsers read besides it.
struct PlayerTemplateFill<'a> {
    pt: &'a mut PlayerTemplate,
    names: &'a mut NameKeyGenerator,
    /// `TheScienceStore`, which `IntrinsicSciences` checks its names against.
    sciences: &'a ScienceStore,
    /// `TheGameText`.
    text: &'a dyn GameTextFetch,
}

/// A field that is `INI::parseAsciiString` into a string member.
macro_rules! ascii_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(
            ini: &mut Ini<'_>,
            w: &mut PlayerTemplateFill<'_>,
            _: &UserData,
        ) -> IniResult<()> {
            w.pt.$member = ini.parse_ascii_string()?;
            Ok(())
        }
    };
}

/// A field that is `INI::parseBool` into a `Bool` member.
macro_rules! bool_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(
            ini: &mut Ini<'_>,
            w: &mut PlayerTemplateFill<'_>,
            _: &UserData,
        ) -> IniResult<()> {
            w.pt.$member = ini.parse_bool()?;
            Ok(())
        }
    };
}

/// A field that is `INI::parseInt` into an `Int` member.
macro_rules! int_field {
    ($fn_name:ident, $member:ident) => {
        fn $fn_name(
            ini: &mut Ini<'_>,
            w: &mut PlayerTemplateFill<'_>,
            _: &UserData,
        ) -> IniResult<()> {
            w.pt.$member = ini.parse_int()?;
            Ok(())
        }
    };
}

ascii_field!(f_side, side);
ascii_field!(f_base_side, base_side);
ascii_field!(f_starting_building, starting_building);
ascii_field!(
    f_purchase_science_command_set_rank1,
    purchase_science_command_set_rank1
);
ascii_field!(
    f_purchase_science_command_set_rank3,
    purchase_science_command_set_rank3
);
ascii_field!(
    f_purchase_science_command_set_rank8,
    purchase_science_command_set_rank8
);
ascii_field!(
    f_special_power_shortcut_command_set,
    special_power_shortcut_command_set
);
ascii_field!(
    f_special_power_shortcut_win_name,
    special_power_shortcut_win_name
);
ascii_field!(f_score_screen_image, score_screen_image);
ascii_field!(f_load_screen_image, load_screen_image);
ascii_field!(f_load_screen_music, load_screen_music);
ascii_field!(f_score_screen_music, score_screen_music);
ascii_field!(f_head_water_mark, head_water_mark);
ascii_field!(f_flag_water_mark, flag_water_mark);
ascii_field!(f_enabled_image, enabled_image);
ascii_field!(f_side_icon_image, side_icon_image);
ascii_field!(f_general_image, general_image);
ascii_field!(f_beacon_name, beacon_template);
ascii_field!(f_army_tooltip, tooltip);
ascii_field!(f_features, str_general_features);
ascii_field!(f_medallion_regular, str_medallion_normal);
ascii_field!(f_medallion_hilite, str_medallion_hilite);
ascii_field!(f_medallion_select, str_medallion_selected);
bool_field!(f_playable_side, playable_side);
bool_field!(f_is_observer, observer);
bool_field!(f_old_faction, old_faction);
int_field!(
    f_special_power_shortcut_button_count,
    special_power_shortcut_button_count
);
int_field!(f_intrinsic_science_purchase_points, intrinsic_spp);

/// `StartingUnit0` to `StartingUnit9`: the slot is the entry's `Mask`.
fn f_starting_unit(
    ini: &mut Ini<'_>,
    w: &mut PlayerTemplateFill<'_>,
    ud: &UserData,
) -> IniResult<()> {
    w.pt.starting_units[ud.mask() as usize] = ini.parse_ascii_string()?;
    Ok(())
}

fn f_display_name(
    ini: &mut Ini<'_>,
    w: &mut PlayerTemplateFill<'_>,
    _: &UserData,
) -> IniResult<()> {
    w.pt.display_name = parse_and_translate_label(ini, w.text)?;
    Ok(())
}

/// `PlayerTemplate::parseStartMoney`: an `Int`, then `init()` and
/// `deposit(amount)` on the template's `Money`. See the module docs for the
/// sound and the income record.
fn f_start_money(ini: &mut Ini<'_>, w: &mut PlayerTemplateFill<'_>, _: &UserData) -> IniResult<()> {
    let money = ini.parse_int()?;
    w.pt.money.init();
    let mut sounds: Vec<ClientEvent> = Vec::new();
    let _record_income = w.pt.money.deposit(money as u32, true, &mut sounds);
    Ok(())
}

fn f_preferred_color(
    ini: &mut Ini<'_>,
    w: &mut PlayerTemplateFill<'_>,
    _: &UserData,
) -> IniResult<()> {
    w.pt.preferred_color = ini.parse_rgb_color()?;
    Ok(())
}

/// `NAMEKEY(ini->getNextToken())`.
fn next_token_key(ini: &mut Ini<'_>, names: &mut NameKeyGenerator) -> IniResult<NameKeyType> {
    let token = ini.get_next_token(None)?;
    names
        .try_name_to_key(&token)
        .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))
}

/// `PlayerTemplate::parseProductionCostChange`: a thing template's name, then
/// a percentage.
fn f_production_cost_change(
    ini: &mut Ini<'_>,
    w: &mut PlayerTemplateFill<'_>,
    _: &UserData,
) -> IniResult<()> {
    let build_template_key = next_token_key(ini, w.names)?;
    let percent_change = Ini::scan_percent_to_real(&ini.get_next_token(None)?)?;
    w.pt.production_cost_changes
        .insert(build_template_key, percent_change);
    Ok(())
}

/// `PlayerTemplate::parseProductionTimeChange`.
fn f_production_time_change(
    ini: &mut Ini<'_>,
    w: &mut PlayerTemplateFill<'_>,
    _: &UserData,
) -> IniResult<()> {
    let build_template_key = next_token_key(ini, w.names)?;
    let percent_change = Ini::scan_percent_to_real(&ini.get_next_token(None)?)?;
    w.pt.production_time_changes
        .insert(build_template_key, percent_change);
    Ok(())
}

/// `PlayerTemplate::parseProductionVeterancyLevel`: a thing template's name,
/// then a `TheVeterancyNames` entry.
fn f_production_veterancy_level(
    ini: &mut Ini<'_>,
    w: &mut PlayerTemplateFill<'_>,
    _: &UserData,
) -> IniResult<()> {
    let build_template_key = next_token_key(ini, w.names)?;
    let start_level = veterancy_from_index(Ini::scan_index_list(
        &ini.get_next_token(None)?,
        &VETERANCY_NAMES,
    )?);
    w.pt.production_veterancy_levels
        .insert(build_template_key, start_level);
    Ok(())
}

fn f_intrinsic_sciences(
    ini: &mut Ini<'_>,
    w: &mut PlayerTemplateFill<'_>,
    _: &UserData,
) -> IniResult<()> {
    w.sciences
        .parse_science_vector(ini, w.names, &mut w.pt.intrinsic_sciences)
}

/// The number of entries of `PlayerTemplate::getFieldParse()`, with the
/// terminator.
const FIELD_COUNT: usize = 46;

/// `PlayerTemplate::getFieldParse()`, in upstream's order.
fn player_template_fields<'a>() -> [FieldParse<PlayerTemplateFill<'a>>; FIELD_COUNT] {
    let n = UserData::None;
    [
        FieldParse::new("Side", f_side, n.clone()),
        FieldParse::new("BaseSide", f_base_side, n.clone()),
        FieldParse::new("PlayableSide", f_playable_side, n.clone()),
        FieldParse::new("DisplayName", f_display_name, n.clone()),
        FieldParse::new("StartMoney", f_start_money, n.clone()),
        FieldParse::new("PreferredColor", f_preferred_color, n.clone()),
        FieldParse::new("StartingBuilding", f_starting_building, n.clone()),
        FieldParse::new("StartingUnit0", f_starting_unit, UserData::Mask(0)),
        FieldParse::new("StartingUnit1", f_starting_unit, UserData::Mask(1)),
        FieldParse::new("StartingUnit2", f_starting_unit, UserData::Mask(2)),
        FieldParse::new("StartingUnit3", f_starting_unit, UserData::Mask(3)),
        FieldParse::new("StartingUnit4", f_starting_unit, UserData::Mask(4)),
        FieldParse::new("StartingUnit5", f_starting_unit, UserData::Mask(5)),
        FieldParse::new("StartingUnit6", f_starting_unit, UserData::Mask(6)),
        FieldParse::new("StartingUnit7", f_starting_unit, UserData::Mask(7)),
        FieldParse::new("StartingUnit8", f_starting_unit, UserData::Mask(8)),
        FieldParse::new("StartingUnit9", f_starting_unit, UserData::Mask(9)),
        FieldParse::new("ProductionCostChange", f_production_cost_change, n.clone()),
        FieldParse::new("ProductionTimeChange", f_production_time_change, n.clone()),
        FieldParse::new(
            "ProductionVeterancyLevel",
            f_production_veterancy_level,
            n.clone(),
        ),
        FieldParse::new("IntrinsicSciences", f_intrinsic_sciences, n.clone()),
        FieldParse::new(
            "PurchaseScienceCommandSetRank1",
            f_purchase_science_command_set_rank1,
            n.clone(),
        ),
        FieldParse::new(
            "PurchaseScienceCommandSetRank3",
            f_purchase_science_command_set_rank3,
            n.clone(),
        ),
        FieldParse::new(
            "PurchaseScienceCommandSetRank8",
            f_purchase_science_command_set_rank8,
            n.clone(),
        ),
        FieldParse::new(
            "SpecialPowerShortcutCommandSet",
            f_special_power_shortcut_command_set,
            n.clone(),
        ),
        FieldParse::new(
            "SpecialPowerShortcutWinName",
            f_special_power_shortcut_win_name,
            n.clone(),
        ),
        FieldParse::new(
            "SpecialPowerShortcutButtonCount",
            f_special_power_shortcut_button_count,
            n.clone(),
        ),
        FieldParse::new("IsObserver", f_is_observer, n.clone()),
        FieldParse::new("OldFaction", f_old_faction, n.clone()),
        FieldParse::new(
            "IntrinsicSciencePurchasePoints",
            f_intrinsic_science_purchase_points,
            n.clone(),
        ),
        FieldParse::new("ScoreScreenImage", f_score_screen_image, n.clone()),
        FieldParse::new("LoadScreenImage", f_load_screen_image, n.clone()),
        FieldParse::new("LoadScreenMusic", f_load_screen_music, n.clone()),
        FieldParse::new("ScoreScreenMusic", f_score_screen_music, n.clone()),
        FieldParse::new("HeadWaterMark", f_head_water_mark, n.clone()),
        FieldParse::new("FlagWaterMark", f_flag_water_mark, n.clone()),
        FieldParse::new("EnabledImage", f_enabled_image, n.clone()),
        FieldParse::new("SideIconImage", f_side_icon_image, n.clone()),
        FieldParse::new("GeneralImage", f_general_image, n.clone()),
        FieldParse::new("BeaconName", f_beacon_name, n.clone()),
        FieldParse::new("ArmyTooltip", f_army_tooltip, n.clone()),
        FieldParse::new("Features", f_features, n.clone()),
        FieldParse::new("MedallionRegular", f_medallion_regular, n.clone()),
        FieldParse::new("MedallionHilite", f_medallion_hilite, n.clone()),
        FieldParse::new("MedallionSelect", f_medallion_select, n),
        FieldParse::end(),
    ]
}

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

/// `class PlayerTemplateStore` (`ThePlayerTemplateStore`).
#[derive(Clone, Debug, Default)]
pub struct PlayerTemplateStore {
    /// `m_name` of the `SubsystemInterface`.
    name: AsciiString,
    /// `m_playerTemplates`.
    player_templates: Vec<PlayerTemplate>,
    /// `findPlayerTemplate`'s fifteen function statics (`a0`..`g4`), once
    /// looked up. See the module docs.
    legacy_keys: Option<[NameKeyType; 15]>,
}

impl SubsystemInterface for PlayerTemplateStore {
    fn name(&self) -> &AsciiString {
        &self.name
    }

    fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    /// `init()`: forget every template.
    fn init(&mut self) {
        self.player_templates.clear();
    }

    /// `reset()`: nothing ("don't reset this list here; we want to retain
    /// this info").
    fn reset(&mut self) {}

    /// `update()`: nothing.
    fn update(&mut self) {}
}

impl PlayerTemplateStore {
    /// `PlayerTemplateStore()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `getPlayerTemplateCount()`.
    pub fn player_template_count(&self) -> i32 {
        self.player_templates.len() as i32
    }

    /// Every template, in the order they were first defined.
    pub fn templates(&self) -> &[PlayerTemplate] {
        &self.player_templates
    }

    /// `getNthPlayerTemplate(i)`: `None` for an `i` outside the store.
    pub fn nth_player_template(&self, i: i32) -> Option<&PlayerTemplate> {
        if i >= 0 && (i as usize) < self.player_templates.len() {
            return Some(&self.player_templates[i as usize]);
        }
        None
    }

    /// The fifteen keys of `findPlayerTemplate`'s statics, looked up in
    /// upstream's order the first time and kept after that.
    fn legacy_keys(
        &mut self,
        names: &mut NameKeyGenerator,
    ) -> Result<[NameKeyType; 15], NameKeyError> {
        if let Some(keys) = self.legacy_keys {
            return Ok(keys);
        }
        let mut keys = [NameKeyType::INVALID; 15];
        for (slot, name) in keys.iter_mut().zip(LEGACY_FACTION_KEY_NAMES) {
            *slot = names.try_name_to_key(name.as_bytes())?;
        }
        self.legacy_keys = Some(keys);
        Ok(keys)
    }

    /// The fifteen keys, if `findPlayerTemplate` has run (so its statics are
    /// initialised).
    pub fn legacy_faction_keys(&self) -> Option<[NameKeyType; 15]> {
        self.legacy_keys
    }

    /// `findPlayerTemplate(namekey)`'s search, with the old-map names mapped
    /// to their base faction: the position of the first template with that
    /// key. (The first call looks the fifteen statics up, which can run out
    /// of name keys.)
    fn find_index(
        &mut self,
        names: &mut NameKeyGenerator,
        namekey: NameKeyType,
    ) -> Result<Option<usize>, NameKeyError> {
        let keys = self.legacy_keys(names)?;
        let mut namekey = namekey;
        // a1..a4 -> a0, else c1..c4 -> c0, else g1..g4 -> g0
        for group in 0..3 {
            let base = group * 5;
            if (1..=4).any(|k| namekey == keys[base + k]) {
                namekey = keys[base];
                break;
            }
        }
        Ok(self
            .player_templates
            .iter()
            .position(|t| t.name_key == namekey))
    }

    /// `findPlayerTemplate(namekey)`: `None` if there is no such template. An
    /// old map's sub-faction finds its base faction (see the module docs).
    pub fn find_player_template(
        &mut self,
        names: &mut NameKeyGenerator,
        namekey: NameKeyType,
    ) -> Result<Option<&PlayerTemplate>, NameKeyError> {
        let i = self.find_index(names, namekey)?;
        Ok(i.map(|i| &self.player_templates[i]))
    }

    /// `getTemplateNumByName(name)`: the position of the first template
    /// whose name equals `name` without regard to case, or -1.
    pub fn template_num_by_name(&self, names: &NameKeyGenerator, name: &[u8]) -> i32 {
        let want = c_str(name);
        for (num, t) in self.player_templates.iter().enumerate() {
            if c_str(t.name(names)).eq_ignore_ascii_case(want) {
                return num as i32;
            }
        }
        -1
    }

    /// `getAllSideStrings(outStringList)`: every side once (in order of
    /// first appearance) appended to `out`.
    pub fn all_side_strings(&self, out: &mut Vec<Vec<u8>>) {
        let mut tmp: Vec<Vec<u8>> = Vec::new();
        for i in 0..self.player_template_count() {
            let Some(pt) = self.nth_player_template(i) else {
                continue;
            };
            if !tmp.iter().any(|s| c_str(s) == c_str(pt.side())) {
                tmp.push(pt.side().to_vec());
            }
        }
        out.append(&mut tmp);
    }

    /// `PlayerTemplateStore::parsePlayerTemplateDefinition(ini)` (the
    /// `PlayerTemplate` block, reached through
    /// `INI::parsePlayerTemplateDefinition`): the template's name, then its
    /// fields until `End`. See the module docs for what a failing block
    /// leaves behind, and for the old-map names.
    pub fn parse_player_template_definition(
        &mut self,
        ini: &mut Ini<'_>,
        names: &mut NameKeyGenerator,
        sciences: &ScienceStore,
        text: &dyn GameTextFetch,
    ) -> IniResult<()> {
        let namekey = next_token_key(ini, names)?;
        let found = self
            .find_index(names, namekey)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        match found {
            Some(i) => {
                let pt = &mut self.player_templates[i];
                let mut fill = PlayerTemplateFill {
                    pt,
                    names,
                    sciences,
                    text,
                };
                ini.init_from_ini(&mut fill, &player_template_fields())?;
                fill.pt.set_name_key(namekey);
            }
            None => {
                let mut npt = PlayerTemplate::new();
                let mut fill = PlayerTemplateFill {
                    pt: &mut npt,
                    names,
                    sciences,
                    text,
                };
                ini.init_from_ini(&mut fill, &player_template_fields())?;
                npt.set_name_key(namekey);
                self.player_templates.push(npt);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniErrorKind, IniLoadType};
    use crate::common::science::ScienceType;

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
        store: PlayerTemplateStore,
        sciences: ScienceStore,
        names: NameKeyGenerator,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store
            .parse_player_template_definition(ini, &mut c.names, &c.sciences, &Text)
    }
    fn science_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.sciences
            .parse_science_definition(ini, &mut c.names, &Text)
    }
    static BLOCKS: [BlockParse<Ctx>; 2] = [
        BlockParse {
            token: "PlayerTemplate",
            parse: block,
        },
        BlockParse {
            token: "Science",
            parse: science_block,
        },
    ];

    fn ctx() -> Ctx {
        let mut names = NameKeyGenerator::new();
        names.reset();
        Ctx {
            store: PlayerTemplateStore::new(),
            sciences: ScienceStore::new(),
            names,
        }
    }

    fn load(c: &mut Ctx, text: &[u8]) -> IniResult<()> {
        Ini::load(b"p.ini", text, IniLoadType::Overwrite, None, &BLOCKS, c)
    }

    const AMERICA: &[u8] = b"Science SCIENCE_A\nEnd\n\
PlayerTemplate FactionAmerica\n Side = America\n BaseSide = USA\n PlayableSide = Yes\n DisplayName = NAME:America\n\
 StartMoney = 10000\n PreferredColor = R:255 G:0 B:51\n StartingBuilding = AmericaCommandCenter\n StartingUnit0 = AmericaDozer\n\
 StartingUnit9 = AmericaRanger\n ProductionCostChange = Tank -10%\n ProductionTimeChange = Tank 25%\n\
 ProductionVeterancyLevel = Tank ELITE\n IntrinsicSciences = SCIENCE_A\n PurchaseScienceCommandSetRank1 = Rank1\n\
 PurchaseScienceCommandSetRank3 = Rank3\n PurchaseScienceCommandSetRank8 = Rank8\n SpecialPowerShortcutCommandSet = Sc\n\
 SpecialPowerShortcutWinName = Win\n SpecialPowerShortcutButtonCount = 7\n IsObserver = No\n OldFaction = Yes\n\
 IntrinsicSciencePurchasePoints = 3\n ScoreScreenImage = Score\n LoadScreenImage = Load\n LoadScreenMusic = LM\n ScoreScreenMusic = SM\n\
 HeadWaterMark = Head\n FlagWaterMark = Flag\n EnabledImage = Enabled\n SideIconImage = Icon\n GeneralImage = General\n\
 BeaconName = Beacon\n ArmyTooltip = Tip\n Features = Feat\n MedallionRegular = M1\n MedallionHilite = M2\n MedallionSelect = M3\nEnd\n";

    #[test]
    fn every_field_is_read_and_every_getter_returns_it() {
        let mut c = ctx();
        load(&mut c, AMERICA).unwrap();
        assert_eq!(c.store.player_template_count(), 1);
        let t = c.store.nth_player_template(0).unwrap();
        assert_eq!(t.name(&c.names), b"FactionAmerica");
        assert_ne!(t.name_key(), NameKeyType::INVALID);
        assert_eq!(t.display_name().str().len(), "NAME:America".len());
        assert_eq!((t.side(), t.base_side()), (&b"America"[..], &b"USA"[..]));
        assert_eq!(t.money().count_money(), 10000);
        assert_eq!(
            t.handicap()
                .get_handicap(crate::common::handicap::HandicapType::BuildCost, true),
            1.0
        );
        assert_eq!(
            *t.preferred_color(),
            RGBColor {
                red: 1.0,
                green: 0.0,
                blue: 51.0 / 255.0
            }
        );
        assert_eq!(t.starting_building(), b"AmericaCommandCenter");
        assert_eq!(
            (t.starting_unit(0), t.starting_unit(9), t.starting_unit(5)),
            (&b"AmericaDozer"[..], &b"AmericaRanger"[..], &b""[..])
        );
        assert_eq!(
            (t.starting_unit(-1), t.starting_unit(10)),
            (&b""[..], &b""[..])
        );
        let tank = c.names.name_to_key(b"Tank");
        assert_eq!(
            t.production_cost_changes().get(&tank),
            Some(&(-10.0f32 / 100.0))
        );
        assert_eq!(
            t.production_time_changes().get(&tank),
            Some(&(25.0f32 / 100.0))
        );
        assert_eq!(
            t.production_veterancy_levels().get(&tank),
            Some(&VeterancyLevel::Elite)
        );
        let sci = ScienceType::from_name_key(c.names.name_to_key(b"SCIENCE_A"));
        assert_eq!(t.intrinsic_sciences(), &vec![sci]);
        assert_eq!(t.purchase_science_command_set_rank1(), b"Rank1");
        assert_eq!(t.purchase_science_command_set_rank3(), b"Rank3");
        assert_eq!(t.purchase_science_command_set_rank8(), b"Rank8");
        assert_eq!(
            (
                t.special_power_shortcut_command_set(),
                t.special_power_shortcut_win_name()
            ),
            (&b"Sc"[..], &b"Win"[..])
        );
        assert_eq!(t.special_power_shortcut_button_count(), 7);
        assert!(t.is_playable_side() && t.is_old_faction() && !t.is_observer());
        assert_eq!(t.intrinsic_science_purchase_points(), 3);
        assert_eq!(
            (t.score_screen(), t.load_screen()),
            (&b"Score"[..], &b"Load"[..])
        );
        assert_eq!(
            (t.load_screen_music(), t.score_screen_music()),
            (&b"LM"[..], &b"SM"[..])
        );
        assert_eq!(
            (
                t.head_water_mark_image_name(),
                t.flag_water_mark_image_name()
            ),
            (&b"Head"[..], &b"Flag"[..])
        );
        assert_eq!(
            (
                t.enabled_image_name(),
                t.side_icon_image_name(),
                t.general_image_name()
            ),
            (&b"Enabled"[..], &b"Icon"[..], &b"General"[..])
        );
        assert_eq!(
            (t.beacon_template(), t.tooltip(), t.general_features()),
            (&b"Beacon"[..], &b"Tip"[..], &b"Feat"[..])
        );
        assert_eq!(
            (
                t.medallion_normal(),
                t.medallion_hilite(),
                t.medallion_selected()
            ),
            (&b"M1"[..], &b"M2"[..], &b"M3"[..])
        );
        assert_eq!(PlayerTemplate::default().name_key(), NameKeyType::INVALID);
        assert_eq!(c.store.templates().len(), 1);
    }

    #[test]
    fn start_money_wraps_and_a_bad_value_keeps_the_old_one() {
        let mut c = ctx();
        load(&mut c, b"PlayerTemplate A\n StartMoney = -1\nEnd\n").unwrap();
        assert_eq!(c.store.templates()[0].money().count_money(), u32::MAX);
        // init() then deposit(): a second StartMoney replaces, it does not add
        load(&mut c, b"PlayerTemplate A\n StartMoney = 5\nEnd\n").unwrap();
        assert_eq!(c.store.templates()[0].money().count_money(), 5);
        let e = load(&mut c, b"PlayerTemplate A\n StartMoney = x\nEnd\n").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        assert_eq!(c.store.templates()[0].money().count_money(), 5);
        let e = load(
            &mut c,
            b"PlayerTemplate A\n StartMoney = 99999999999\nEnd\n",
        )
        .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::Undefined(Undefined::IntOutOfRange));
    }

    #[test]
    fn production_parsers_assign_the_key_before_the_value() {
        let mut c = ctx();
        let e = load(
            &mut c,
            b"PlayerTemplate A\n ProductionCostChange = Barracks nope\nEnd\n",
        )
        .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        // "Barracks" has a key although the block failed, and A is not stored
        assert_eq!(c.store.player_template_count(), 0);
        let next = c.names.name_to_key(b"Fresh");
        assert_eq!(
            next.0, 18,
            "A is 1, the fifteen old-map names 2..=16, Barracks 17, then the next name"
        );
        let e = load(
            &mut c,
            b"PlayerTemplate A\n ProductionVeterancyLevel = Tank GOD\nEnd\n",
        )
        .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        let e = load(
            &mut c,
            b"PlayerTemplate A\n ProductionTimeChange = Tank\nEnd\n",
        )
        .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        load(&mut c, b"PlayerTemplate A\n ProductionTimeChange = Tank 50\n ProductionTimeChange = Tank 75%\nEnd\n").unwrap();
        let t = &c.store.templates()[0];
        assert_eq!(t.production_time_changes().len(), 1);
        assert_eq!(
            *t.production_time_changes().values().next().unwrap(),
            0.75f32
        );
    }

    #[test]
    fn intrinsic_sciences_are_checked_and_kept_up_to_the_bad_name() {
        let mut c = ctx();
        load(&mut c, b"Science S1\nEnd\nScience S2\nEnd\nPlayerTemplate A\n IntrinsicSciences = S1 S2\nEnd\n").unwrap();
        assert_eq!(c.store.templates()[0].intrinsic_sciences().len(), 2);
        let e = load(
            &mut c,
            b"PlayerTemplate A\n IntrinsicSciences = S2 Nope S1\nEnd\n",
        )
        .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        assert_eq!(c.store.templates()[0].intrinsic_sciences().len(), 1);
        load(
            &mut c,
            b"PlayerTemplate A\n IntrinsicSciences = None\nEnd\n",
        )
        .unwrap();
        assert!(c.store.templates()[0].intrinsic_sciences().is_empty());
    }

    #[test]
    fn a_failing_display_name_leaves_the_field_and_a_failing_new_block_leaves_nothing() {
        let mut c = ctx();
        load(&mut c, b"PlayerTemplate A\n DisplayName = Nice\nEnd\n").unwrap();
        let e = load(
            &mut c,
            b"PlayerTemplate A\n Side = Changed\n DisplayName = EMPTY\nEnd\n",
        )
        .unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        let t = &c.store.templates()[0];
        assert_eq!(
            (t.side(), t.display_name().str().len()),
            (&b"Changed"[..], 4)
        );
        let e = load(&mut c, b"PlayerTemplate B\n Side = X\n Bogus = 1\nEnd\n").unwrap_err();
        assert_eq!(e.kind, IniErrorKind::UnknownToken);
        assert_eq!(c.store.player_template_count(), 1);
    }

    #[test]
    fn the_old_map_names_find_the_base_faction_and_rename_it() {
        let mut c = ctx();
        load(&mut c, b"PlayerTemplate FactionAmerica\n Side = America\nEnd\nPlayerTemplate FactionChina\n Side = China\nEnd\n").unwrap();
        let keys = c.store.legacy_faction_keys().expect("the first lookup ran");
        assert_eq!(keys[0], c.names.name_to_key(b"FactionAmerica"));
        let tank = c.names.name_to_key(b"FactionAmericaTankCommand");
        assert_eq!(keys[2], tank);
        // a lookup by an old name finds the base template
        let found = c
            .store
            .find_player_template(&mut c.names, tank)
            .unwrap()
            .unwrap();
        assert_eq!(found.side(), b"America");
        // the block updates it in place and renames it to the name as written
        load(
            &mut c,
            b"PlayerTemplate FactionAmericaTankCommand\n BaseSide = Tank\nEnd\n",
        )
        .unwrap();
        assert_eq!(c.store.player_template_count(), 2);
        assert_eq!(
            c.store.templates()[0].name(&c.names),
            b"FactionAmericaTankCommand"
        );
        assert_eq!(c.store.templates()[0].base_side(), b"Tank");
        let america = c.names.name_to_key(b"FactionAmerica");
        assert!(c
            .store
            .find_player_template(&mut c.names, america)
            .unwrap()
            .is_none());
        // the old name now maps to FactionAmerica, which nothing has
        assert!(c
            .store
            .find_player_template(&mut c.names, tank)
            .unwrap()
            .is_none());
        let china = c.names.name_to_key(b"FactionChinaRedArmy");
        assert_eq!(
            c.store
                .find_player_template(&mut c.names, china)
                .unwrap()
                .unwrap()
                .side(),
            b"China"
        );
    }

    #[test]
    fn the_fifteen_keys_are_looked_up_once_and_survive_a_reset_of_the_names() {
        let mut c = ctx();
        assert!(c.store.legacy_faction_keys().is_none());
        let first = c.names.name_to_key(b"Zeta");
        assert_eq!(first.0, 1);
        assert!(c
            .store
            .find_player_template(&mut c.names, first)
            .unwrap()
            .is_none());
        let keys = c.store.legacy_faction_keys().unwrap();
        let numbers: Vec<u32> = keys.iter().map(|k| k.0).collect();
        assert_eq!(
            numbers,
            (2..=16).collect::<Vec<u32>>(),
            "a0..g4 take the next fifteen keys, in order"
        );
        // a new numbering: the statics keep the old keys, so key 3 (a1's) now
        // reads as "the old name of the faction whose key is 2"
        c.names.reset();
        c.store.init();
        assert_eq!(c.names.name_to_key(b"First").0, 1);
        load(&mut c, b"PlayerTemplate Alpha\n Side = Found\nEnd\n").unwrap();
        assert_eq!(
            c.store.legacy_faction_keys(),
            Some(keys),
            "not looked up again"
        );
        assert_eq!(c.store.templates()[0].name_key().0, 2);
        let found = c
            .store
            .find_player_template(&mut c.names, NameKeyType(3))
            .unwrap()
            .unwrap();
        assert_eq!(found.side(), b"Found");
        // and FactionAmerica itself, now numbered 3, reads as a1 and finds it too
        let america = c.names.name_to_key(b"FactionAmerica");
        assert_eq!(america.0, 3);
        assert_eq!(
            c.store
                .find_player_template(&mut c.names, america)
                .unwrap()
                .unwrap()
                .side(),
            b"Found"
        );
    }

    #[test]
    fn lookups_by_number_and_name_and_the_sides() {
        let mut c = ctx();
        load(
            &mut c,
            b"PlayerTemplate One\n Side = America\nEnd\nPlayerTemplate Two\n Side = China\nEnd\n\
              PlayerTemplate Three\n Side = America\nEnd\nPlayerTemplate Four\nEnd\n",
        )
        .unwrap();
        assert_eq!(c.store.template_num_by_name(&c.names, b"three"), 2);
        assert_eq!(c.store.template_num_by_name(&c.names, b"THREE"), 2);
        assert_eq!(c.store.template_num_by_name(&c.names, b"Five"), -1);
        assert!(
            c.store.nth_player_template(-1).is_none() && c.store.nth_player_template(4).is_none()
        );
        let mut sides = vec![b"Existing".to_vec(), b"America".to_vec()];
        c.store.all_side_strings(&mut sides);
        // each side once, first appearance; a side already in the list is added again
        assert_eq!(
            sides,
            vec![
                b"Existing".to_vec(),
                b"America".to_vec(),
                b"America".to_vec(),
                b"China".to_vec(),
                b"".to_vec()
            ]
        );
    }

    #[test]
    fn the_subsystem_interface_init_clears_and_reset_keeps() {
        let mut c = ctx();
        load(&mut c, b"PlayerTemplate One\nEnd\n").unwrap();
        c.store.set_name(AsciiString::default());
        assert!(c.store.name().is_empty());
        c.store.update();
        c.store.reset();
        assert_eq!(c.store.player_template_count(), 1);
        c.store.init();
        assert_eq!(c.store.player_template_count(), 0);
    }
}
