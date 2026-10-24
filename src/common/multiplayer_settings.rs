// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/MultiplayerSettings.h
//   GeneralsMD/Code/GameEngine/Source/Common/MultiplayerSettings.cpp
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIMultiplayer.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Matthew D. Campbell, January 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MultiplayerSettings` (`TheMultiplayerSettings`: the multiplayer defaults
//! of `Multiplayer.ini`), its `MultiplayerColorDefinition`s (the player colour
//! list), and the starting-money choices, with the `MultiplayerSettings`,
//! `MultiplayerColor` and `MultiplayerStartingMoneyChoice` INI blocks.
//!
//! # Translation choices
//!
//! - `m_colorList` is a `std::map<Int, MultiplayerColorDefinition>`: a
//!   `BTreeMap`, which iterates in the same key order.
//! - `getNumColors()` caches `m_colorList.size()` in `m_numColors`. The cache
//!   is set to the size on every insertion, so it always equals the size;
//!   [`MultiplayerSettings::get_num_colors`] returns the size.
//! - `m_initialCreditsMin` and `m_initialCreditsMax` are declared upstream
//!   and never read or written by any code in the tree, and the constructor
//!   leaves them unset. They are not translated.
//! - The `MultiplayerSettings` block creates `TheMultiplayerSettings` on
//!   first use upstream. Here the owner creates the settings once, with the
//!   constructor's values, and the block fills it; the colour and money blocks
//!   upstream would dereference a `NULL` singleton before that, which is
//!   undefined.
//! - [`MultiplayerSettings::get_default_starting_money`] asserts that a
//!   default was given (`DEBUG_ASSERTCRASH`, which is empty in the release
//!   configuration upstream); without one it returns the default `Money`, as
//!   upstream does.
//! - `setColor` takes `rgb.getAsInt()`, which is undefined for a negative
//!   component or one that does not fit an `Int` (see `common::base_type`).
//!   [`MultiplayerColorDefinition::set_color`] returns the error for such a
//!   value. `parseRGBColor` keeps components in 0..=255, so the INI blocks
//!   never reach it; the block handlers map it to [`Undefined::ColorToInt`]
//!   all the same.
//!
//! # Left out: methods that take an `Object` or a `Player`
//!
//! None. `MultiplayerSettings.h` takes none; its users that take a player
//! (`GameInfo`, the skirmish and LAN menus) are in later layers.

use std::collections::BTreeMap;

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::{BaseTypeError, RGBColor};
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, Undefined, UserData};
use crate::common::money::{parse_money_amount, Money};
use crate::common::subsystem_interface::SubsystemInterface;

/// `PLAYERTEMPLATE_RANDOM` (GameNetwork/GameInfo.h): the colour of a random
/// player.
pub const PLAYERTEMPLATE_RANDOM: i32 = -1;

/// `PLAYERTEMPLATE_OBSERVER` (GameNetwork/GameInfo.h): the observer's colour.
pub const PLAYERTEMPLATE_OBSERVER: i32 = -2;

/// `class MultiplayerColorDefinition`: one colour of the multiplayer colour
/// list, in its day and night forms.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiplayerColorDefinition {
    /// `m_tooltipName`: the colour's name in the GUI (a text-table name).
    tooltip_name: Vec<u8>,
    /// `m_rgbValue`.
    rgb_value: RGBColor,
    /// `m_color`: `rgb_value` as an `Int` with a full alpha byte.
    color: i32,
    /// `m_rgbValueNight`.
    rgb_value_night: RGBColor,
    /// `m_colorNight`.
    color_night: i32,
}

impl Default for MultiplayerColorDefinition {
    /// `MultiplayerColorDefinition()`: white (`0xFFFFFFFF`) in both forms.
    fn default() -> Self {
        let mut white = RGBColor::default();
        white.set_from_int(-1);
        MultiplayerColorDefinition {
            tooltip_name: Vec::new(),
            rgb_value: white,
            color: -1,
            rgb_value_night: white,
            color_night: -1,
        }
    }
}

impl MultiplayerColorDefinition {
    /// `MultiplayerColorDefinition()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `getTooltipName()`.
    pub fn tooltip_name(&self) -> &[u8] {
        &self.tooltip_name
    }

    /// `getRGBValue()`.
    pub fn rgb_value(&self) -> RGBColor {
        self.rgb_value
    }

    /// `getRGBNightValue()`.
    pub fn rgb_night_value(&self) -> RGBColor {
        self.rgb_value_night
    }

    /// `getColor()`.
    pub fn color(&self) -> i32 {
        self.color
    }

    /// `getNightColor()`.
    pub fn night_color(&self) -> i32 {
        self.color_night
    }

    /// `setColor(rgb)`: `m_color = rgb.getAsInt() | 0xFF << 24`. Undefined
    /// upstream where `getAsInt` is (see the module docs).
    pub fn set_color(&mut self, rgb: RGBColor) -> Result<(), BaseTypeError> {
        self.color = rgb.try_get_as_int()? | (0xFF_i32 << 24);
        Ok(())
    }

    /// `setNightColor(rgb)`: `m_colorNight = rgb.getAsInt() | 0xFF << 24`.
    pub fn set_night_color(&mut self, rgb: RGBColor) -> Result<(), BaseTypeError> {
        self.color_night = rgb.try_get_as_int()? | (0xFF_i32 << 24);
        Ok(())
    }
}

/// `MultiplayerStartingMoneyList`: the amounts offered in the starting-money
/// drop-down, in the order given.
pub type MultiplayerStartingMoneyList = Vec<Money>;

/// `class MultiplayerSettings`: the multiplayer defaults and the colour and
/// starting-money lists.
#[derive(Clone, Debug)]
pub struct MultiplayerSettings {
    /// `m_name` (the subsystem's name).
    name: AsciiString,
    /// `m_startCountdownTimerSeconds`.
    start_countdown_timer_seconds: i32,
    /// `m_maxBeaconsPerPlayer`.
    max_beacons_per_player: i32,
    /// `m_isShroudInMultiplayer`.
    is_shroud_in_multiplayer: bool,
    /// `m_showRandomPlayerTemplate`.
    show_random_player_template: bool,
    /// `m_showRandomStartPos`.
    show_random_start_pos: bool,
    /// `m_showRandomColor`.
    show_random_color: bool,
    /// `m_colorList`, by key (`std::map<Int, ...>`).
    color_list: BTreeMap<i32, MultiplayerColorDefinition>,
    /// `m_observerColor`.
    observer_color: MultiplayerColorDefinition,
    /// `m_randomColor`.
    random_color: MultiplayerColorDefinition,
    /// `m_startingMoneyList`.
    starting_money_list: MultiplayerStartingMoneyList,
    /// `m_defaultStartingMoney`.
    default_starting_money: Money,
    /// `m_gotDefaultStartingMoney`.
    got_default_starting_money: bool,
}

impl Default for MultiplayerSettings {
    /// `MultiplayerSettings()`.
    fn default() -> Self {
        MultiplayerSettings {
            name: AsciiString::default(),
            start_countdown_timer_seconds: 0,
            max_beacons_per_player: 3,
            is_shroud_in_multiplayer: true,
            show_random_player_template: true,
            show_random_start_pos: true,
            show_random_color: true,
            color_list: BTreeMap::new(),
            observer_color: MultiplayerColorDefinition::new(),
            random_color: MultiplayerColorDefinition::new(),
            starting_money_list: Vec::new(),
            default_starting_money: Money::new(),
            got_default_starting_money: false,
        }
    }
}

impl SubsystemInterface for MultiplayerSettings {
    fn name(&self) -> &AsciiString {
        &self.name
    }

    fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    /// `init() { }`.
    fn init(&mut self) {}

    /// `update() { }`.
    fn update(&mut self) {}

    /// `reset() { }`.
    fn reset(&mut self) {}
}

impl MultiplayerSettings {
    /// `MultiplayerSettings()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `getStartCountdownTimerSeconds()`.
    pub fn get_start_countdown_timer_seconds(&self) -> i32 {
        self.start_countdown_timer_seconds
    }

    /// `getMaxBeaconsPerPlayer()`.
    pub fn get_max_beacons_per_player(&self) -> i32 {
        self.max_beacons_per_player
    }

    /// `isShroudInMultiplayer()`.
    pub fn is_shroud_in_multiplayer(&self) -> bool {
        self.is_shroud_in_multiplayer
    }

    /// `showRandomPlayerTemplate()`.
    pub fn show_random_player_template(&self) -> bool {
        self.show_random_player_template
    }

    /// `showRandomStartPos()`.
    pub fn show_random_start_pos(&self) -> bool {
        self.show_random_start_pos
    }

    /// `showRandomColor()`.
    pub fn show_random_color(&self) -> bool {
        self.show_random_color
    }

    /// `getNumColors()`: the number of colours in the list (see the module
    /// docs on the upstream cache).
    pub fn get_num_colors(&self) -> i32 {
        self.color_list.len() as i32
    }

    /// `findMultiplayerColorDefinitionByName(name)`: the first colour (in key
    /// order) whose **tooltip name** is `name`. Upstream matches the tooltip,
    /// not the block's name.
    pub fn find_multiplayer_color_definition_by_name(&self, name: &[u8]) -> Option<&MultiplayerColorDefinition> {
        self.color_list.values().find(|d| d.tooltip_name == name)
    }

    /// `newMultiplayerColorDefinition(name)`: a new default colour at the
    /// next key, which is the list's size. Returns that key. (The name is
    /// unused upstream too; the tooltip is set by the block's own fields.)
    pub fn new_multiplayer_color_definition(&mut self, _name: &[u8]) -> i32 {
        let key = self.get_num_colors();
        self.color_list.insert(key, MultiplayerColorDefinition::new());
        key
    }

    /// `getColor(which)`: the random colour for `PLAYERTEMPLATE_RANDOM`, the
    /// observer's for `PLAYERTEMPLATE_OBSERVER`, else the colour at `which`
    /// if there is one. `None` otherwise.
    pub fn get_color(&mut self, which: i32) -> Option<&mut MultiplayerColorDefinition> {
        if which == PLAYERTEMPLATE_RANDOM {
            Some(&mut self.random_color)
        } else if which == PLAYERTEMPLATE_OBSERVER {
            Some(&mut self.observer_color)
        } else if which < 0 || which >= self.get_num_colors() {
            None
        } else {
            self.color_list.get_mut(&which)
        }
    }

    /// `getDefaultStartingMoney()`: the default amount, or the constructor's
    /// `Money` if no default was given (upstream asserts there is one).
    pub fn get_default_starting_money(&self) -> &Money {
        &self.default_starting_money
    }

    /// Whether a default starting money was given (`m_gotDefaultStartingMoney`).
    pub fn got_default_starting_money(&self) -> bool {
        self.got_default_starting_money
    }

    /// `getStartingMoneyList()`.
    pub fn get_starting_money_list(&self) -> &[Money] {
        &self.starting_money_list
    }

    /// `addStartingMoneyChoice(money, isDefault)`: appends the amount, and
    /// makes it the default if asked. A later default replaces an earlier one
    /// (upstream asserts that there is no second default; the assertion is
    /// empty in the release configuration).
    pub fn add_starting_money_choice(&mut self, money: Money, is_default: bool) {
        self.starting_money_list.push(money);
        if is_default {
            self.default_starting_money = money;
            self.got_default_starting_money = true;
        }
    }

    /// `INI::parseMultiplayerSettingsDefinition`: the six fields of the
    /// block, until `End`.
    pub fn parse_multiplayer_settings_definition(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        ini.init_from_ini(self, &MULTIPLAYER_SETTINGS_FIELDS)
    }

    /// `INI::parseMultiplayerColorDefinition`: the colour named by the block
    /// (an existing colour whose tooltip is that name, or a new one), its
    /// fields, then its `Int` forms of the two RGB values.
    pub fn parse_multiplayer_color_definition(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        let key = match self.color_key_by_tooltip(&name) {
            Some(key) => key,
            None => self.new_multiplayer_color_definition(&name),
        };
        let definition = self.color_list.entry(key).or_default();
        ini.init_from_ini(definition, &MULTIPLAYER_COLOR_FIELDS)?;
        let rgb = definition.rgb_value();
        definition.set_color(rgb).map_err(|_| IniError::undefined(Undefined::ColorToInt))?;
        let rgb_night = definition.rgb_night_value();
        definition.set_night_color(rgb_night).map_err(|_| IniError::undefined(Undefined::ColorToInt))?;
        Ok(())
    }

    /// The key of the first colour (in key order) whose tooltip is `name`.
    fn color_key_by_tooltip(&self, name: &[u8]) -> Option<i32> {
        self.color_list.iter().find(|(_, d)| d.tooltip_name == name).map(|(k, _)| *k)
    }

    /// `INI::parseMultiplayerStartingMoneyChoiceDefinition`: `Value` and
    /// `Default`, then the choice is added.
    pub fn parse_multiplayer_starting_money_choice_definition(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        let mut settings = StartingMoneySettings { money: Money::new(), is_default: false };
        ini.init_from_ini(&mut settings, &STARTING_MONEY_FIELDS)?;
        self.add_starting_money_choice(settings.money, settings.is_default);
        Ok(())
    }
}

/// The block's `MultiplayerSettings` fields (`m_multiplayerSettingsFieldParseTable`).
static MULTIPLAYER_SETTINGS_FIELDS: [FieldParse<MultiplayerSettings>; 7] = [
    FieldParse::new("StartCountdownTimer", parse_start_countdown_timer, UserData::None),
    FieldParse::new("MaxBeaconsPerPlayer", parse_max_beacons_per_player, UserData::None),
    FieldParse::new("UseShroud", parse_use_shroud, UserData::None),
    FieldParse::new("ShowRandomPlayerTemplate", parse_show_random_player_template, UserData::None),
    FieldParse::new("ShowRandomStartPos", parse_show_random_start_pos, UserData::None),
    FieldParse::new("ShowRandomColor", parse_show_random_color, UserData::None),
    FieldParse::end(),
];

fn parse_start_countdown_timer(ini: &mut Ini<'_>, s: &mut MultiplayerSettings, _ud: &UserData) -> IniResult<()> {
    s.start_countdown_timer_seconds = ini.parse_int()?;
    Ok(())
}

fn parse_max_beacons_per_player(ini: &mut Ini<'_>, s: &mut MultiplayerSettings, _ud: &UserData) -> IniResult<()> {
    s.max_beacons_per_player = ini.parse_int()?;
    Ok(())
}

fn parse_use_shroud(ini: &mut Ini<'_>, s: &mut MultiplayerSettings, _ud: &UserData) -> IniResult<()> {
    s.is_shroud_in_multiplayer = ini.parse_bool()?;
    Ok(())
}

fn parse_show_random_player_template(ini: &mut Ini<'_>, s: &mut MultiplayerSettings, _ud: &UserData) -> IniResult<()> {
    s.show_random_player_template = ini.parse_bool()?;
    Ok(())
}

fn parse_show_random_start_pos(ini: &mut Ini<'_>, s: &mut MultiplayerSettings, _ud: &UserData) -> IniResult<()> {
    s.show_random_start_pos = ini.parse_bool()?;
    Ok(())
}

fn parse_show_random_color(ini: &mut Ini<'_>, s: &mut MultiplayerSettings, _ud: &UserData) -> IniResult<()> {
    s.show_random_color = ini.parse_bool()?;
    Ok(())
}

/// `MultiplayerColorDefinition::m_colorFieldParseTable`.
static MULTIPLAYER_COLOR_FIELDS: [FieldParse<MultiplayerColorDefinition>; 4] = [
    FieldParse::new("TooltipName", parse_tooltip_name, UserData::None),
    FieldParse::new("RGBColor", parse_rgb_color, UserData::None),
    FieldParse::new("RGBNightColor", parse_rgb_night_color, UserData::None),
    FieldParse::end(),
];

fn parse_tooltip_name(ini: &mut Ini<'_>, c: &mut MultiplayerColorDefinition, _ud: &UserData) -> IniResult<()> {
    c.tooltip_name = ini.parse_ascii_string()?;
    Ok(())
}

fn parse_rgb_color(ini: &mut Ini<'_>, c: &mut MultiplayerColorDefinition, _ud: &UserData) -> IniResult<()> {
    c.rgb_value = ini.parse_rgb_color()?;
    Ok(())
}

fn parse_rgb_night_color(ini: &mut Ini<'_>, c: &mut MultiplayerColorDefinition, _ud: &UserData) -> IniResult<()> {
    c.rgb_value_night = ini.parse_rgb_color()?;
    Ok(())
}

/// The temporary store of `INI::parseMultiplayerStartingMoneyChoiceDefinition`.
struct StartingMoneySettings {
    money: Money,
    is_default: bool,
}

/// That function's `startingMoneyFieldParseTable`.
static STARTING_MONEY_FIELDS: [FieldParse<StartingMoneySettings>; 3] = [
    FieldParse::new("Value", parse_value, UserData::None),
    FieldParse::new("Default", parse_default, UserData::None),
    FieldParse::end(),
];

fn parse_value(ini: &mut Ini<'_>, s: &mut StartingMoneySettings, ud: &UserData) -> IniResult<()> {
    parse_money_amount(ini, &mut s.money, ud)
}

fn parse_default(ini: &mut Ini<'_>, s: &mut StartingMoneySettings, _ud: &UserData) -> IniResult<()> {
    s.is_default = ini.parse_bool()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniErrorKind, IniLoadType};

    struct Ctx {
        settings: MultiplayerSettings,
    }

    fn settings_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.settings.parse_multiplayer_settings_definition(ini)
    }
    fn color_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.settings.parse_multiplayer_color_definition(ini)
    }
    fn money_block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.settings.parse_multiplayer_starting_money_choice_definition(ini)
    }

    static BLOCKS: [BlockParse<Ctx>; 3] = [
        BlockParse { token: "MultiplayerSettings", parse: settings_block },
        BlockParse { token: "MultiplayerColor", parse: color_block },
        BlockParse { token: "MultiplayerStartingMoneyChoice", parse: money_block },
    ];

    fn load(text: &[u8]) -> (Ctx, IniResult<()>) {
        let mut c = Ctx { settings: MultiplayerSettings::new() };
        let r = Ini::load(b"m.ini", text, IniLoadType::Overwrite, None, &BLOCKS, &mut c);
        (c, r)
    }

    #[test]
    fn subsystem_interface_is_empty_and_the_name_is_kept() {
        let mut s = MultiplayerSettings::new();
        s.set_name(AsciiString::default());
        assert!(s.name().is_empty());
        s.init();
        s.update();
        s.reset();
        assert_eq!(s.get_num_colors(), 0, "reset changes nothing, as upstream");
    }

    #[test]
    fn constructor_defaults() {
        let s = MultiplayerSettings::new();
        assert_eq!(s.get_max_beacons_per_player(), 3);
        assert_eq!(s.get_start_countdown_timer_seconds(), 0);
        assert!(s.is_shroud_in_multiplayer() && s.show_random_color() && s.show_random_start_pos());
        assert!(s.show_random_player_template());
        assert_eq!(s.get_num_colors(), 0);
        assert!(s.get_starting_money_list().is_empty());
        assert_eq!(*s.get_default_starting_money(), Money::new());
        let def = MultiplayerColorDefinition::new();
        assert_eq!((def.color(), def.night_color()), (-1, -1));
        assert_eq!(def.rgb_value().red, 1.0);
    }

    #[test]
    fn settings_block_sets_its_six_fields() {
        let (c, r) = load(
            b"MultiplayerSettings\n StartCountdownTimer = 7\n MaxBeaconsPerPlayer = 5\n UseShroud = No\n\
              ShowRandomPlayerTemplate = No\n ShowRandomStartPos = No\n ShowRandomColor = No\nEnd\n",
        );
        r.unwrap();
        let s = &c.settings;
        assert_eq!(s.get_start_countdown_timer_seconds(), 7);
        assert_eq!(s.get_max_beacons_per_player(), 5);
        assert!(!s.is_shroud_in_multiplayer() && !s.show_random_player_template());
        assert!(!s.show_random_start_pos() && !s.show_random_color());
    }

    #[test]
    fn color_blocks_append_and_match_tooltips() {
        let (mut c, r) = load(
            b"MultiplayerColor Red\n TooltipName = COLOR:Red\n RGBColor = R:255 G:0 B:0\n RGBNightColor = R:128 G:0 B:0\nEnd\n\
              MultiplayerColor Blue\n TooltipName = COLOR:Blue\n RGBColor = R:0 G:0 B:255\nEnd\n\
              MultiplayerColor Red\n RGBColor = R:0 G:255 B:0\nEnd\n",
        );
        r.unwrap();
        let s = &mut c.settings;
        assert_eq!(s.get_num_colors(), 3, "a block whose name is not a tooltip makes a new colour");
        let red = s.get_color(0).unwrap().clone();
        assert_eq!(red.tooltip_name(), b"COLOR:Red");
        assert_eq!(red.color(), (0xFF << 24) | 0xFF_0000);
        assert_eq!(red.night_color(), (0xFF << 24) | 0x80_0000);
        assert_eq!(s.get_color(1).unwrap().color(), (0xFF << 24) | 0xFF);
        // the third block's RGBColor is kept by colour 2 (no tooltip matched "Red")
        assert_eq!(s.get_color(2).unwrap().color(), (0xFF << 24) | 0xFF00);
        assert_eq!(s.find_multiplayer_color_definition_by_name(b"COLOR:Blue").map(|d| d.color()), Some((0xFF << 24) | 0xFF));
        assert!(s.get_color(3).is_none() && s.get_color(-3).is_none());
        assert_eq!(s.get_color(PLAYERTEMPLATE_RANDOM).unwrap().color(), -1);
        assert_eq!(s.get_color(PLAYERTEMPLATE_OBSERVER).unwrap().color(), -1);
    }

    #[test]
    fn colour_components_are_bytes_and_set_color_checks_the_shift() {
        // parseRGBColor accepts 0..=255 only, so the INI cannot reach the undefined case
        let (_, r) = load(b"MultiplayerColor X\n RGBColor = R:-1 G:0 B:0\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        let (_, r) = load(b"MultiplayerColor X\n RGBColor = R:256 G:0 B:0\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        // the direct call: a negative component is undefined upstream (getAsInt)
        let mut def = MultiplayerColorDefinition::new();
        let negative = RGBColor { red: -1.0, green: 0.0, blue: 0.0 };
        assert!(def.set_color(negative).is_err());
        assert_eq!(def.color(), -1, "a failed set_color leaves the colour alone");
    }

    #[test]
    fn starting_money_choices_and_default() {
        let (c, r) = load(
            b"MultiplayerStartingMoneyChoice\n Value = 5000\nEnd\n\
              MultiplayerStartingMoneyChoice\n Value = 10000\n Default = Yes\nEnd\n\
              MultiplayerStartingMoneyChoice\n Value = 20000\nEnd\n",
        );
        r.unwrap();
        let s = &c.settings;
        let amounts: Vec<u32> = s.get_starting_money_list().iter().map(|m| m.count_money()).collect();
        assert_eq!(amounts, vec![5000, 10000, 20000]);
        assert_eq!(s.get_default_starting_money().count_money(), 10000);
        let (_, r) = load(b"MultiplayerStartingMoneyChoice\n Value = many\nEnd\n");
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
    }
}
