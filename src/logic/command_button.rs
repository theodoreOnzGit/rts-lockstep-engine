// SPDX-License-Identifier: GPL-3.0-or-later
//! Integration batch I19: `CommandButton` and `CommandSet`, the control bar's data the logic reads (ControlBar.h, ControlBar.cpp,
//! INICommandButton.cpp, INICommandSet.cpp).
//!
//! Upstream the control bar (a client subsystem) owns them, but the logic asks them: a command-button hunt, a script's
//! "execute command button", the AI's `findCommandSet(getCommandSetString())`, a special power's button. They are read in
//! `ControlBar::init` from `Data\INI\Default\CommandButton.ini`, `Data\INI\CommandButton.ini` and `Data\INI\CommandSet.ini`,
//! after the thing factory, the upgrade center, the special power store and the science store, whose entries a button names.
//!
//! **What is kept:** every field of the button's table (`s_commandButtonFieldParseTable`), with the client's (labels, images,
//! cursors, border, radius cursor, sound) as their names; `parseCommand` against `TheGuiCommandNames` (`INI_INVALID_DATA` for an
//! unknown command); `Options` as `parseBitString32` against `TheCommandOptionNames`, whose places are the bits (the unused bits keep
//! their placeholder names); the set's 18 slots, each a button found by name (`INI_INVALID_DATA` for an unknown one); the overrides
//! (`INI_LOAD_CREATE_OVERRIDES`: a copy of the final override, parsed over); a duplicate button is a debug crash upstream and is
//! parsed over the existing one, a duplicate set throws `INI_INVALID_DATA`. Built without `ALLOW_SURRENDER`, as the shipped game.
//! **Not kept:** `postProcessCommands`' `cacheButtonImage` (the client's image).
//!
//! `p1w13-sonnet`, integration batch I19, 2026-11-02.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, LookupListRec, UserData};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::science::{parse_science_vector, ScienceType, ScienceVec};
use crate::logic::modules::deliver_payload_data::WEAPON_SLOT_TYPE_NAMES_LOOKUP_LIST;

/// `MAX_COMMANDS_PER_SET`.
pub const MAX_COMMANDS_PER_SET: usize = 18;

/// `TheGuiCommandNames` (without `ALLOW_SURRENDER`): a `GUICommandType` is its place here.
pub const GUI_COMMAND_NAMES: [&str; 35] = [
    "NONE",
    "DOZER_CONSTRUCT",
    "DOZER_CONSTRUCT_CANCEL",
    "UNIT_BUILD",
    "CANCEL_UNIT_BUILD",
    "PLAYER_UPGRADE",
    "OBJECT_UPGRADE",
    "CANCEL_UPGRADE",
    "ATTACK_MOVE",
    "GUARD",
    "GUARD_WITHOUT_PURSUIT",
    "GUARD_FLYING_UNITS_ONLY",
    "STOP",
    "WAYPOINTS",
    "EXIT_CONTAINER",
    "EVACUATE",
    "EXECUTE_RAILED_TRANSPORT",
    "BEACON_DELETE",
    "SET_RALLY_POINT",
    "SELL",
    "FIRE_WEAPON",
    "SPECIAL_POWER",
    "PURCHASE_SCIENCE",
    "HACK_INTERNET",
    "TOGGLE_OVERCHARGE",
    "COMBATDROP",
    "SWITCH_WEAPON",
    "HIJACK_VEHICLE",
    "CONVERT_TO_CARBOMB",
    "SABOTAGE_BUILDING",
    "PLACE_BEACON",
    "SPECIAL_POWER_FROM_SHORTCUT",
    "SPECIAL_POWER_CONSTRUCT",
    "SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT",
    "SELECT_ALL_UNITS_OF_TYPE",
];

/// `TheCommandOptionNames` (without `ALLOW_SURRENDER`): bit `i` is the name in place `i`.
pub const COMMAND_OPTION_NAMES: [&str; 23] = [
    "NEED_TARGET_ENEMY_OBJECT",
    "NEED_TARGET_NEUTRAL_OBJECT",
    "NEED_TARGET_ALLY_OBJECT",
    "unused-reserved",
    "ALLOW_SHRUBBERY_TARGET",
    "NEED_TARGET_POS",
    "NEED_UPGRADE",
    "NEED_SPECIAL_POWER_SCIENCE",
    "OK_FOR_MULTI_SELECT",
    "CONTEXTMODE_COMMAND",
    "CHECK_LIKE",
    "ALLOW_MINE_TARGET",
    "ATTACK_OBJECTS_POSITION",
    "OPTION_ONE",
    "OPTION_TWO",
    "OPTION_THREE",
    "NOT_QUEUEABLE",
    "SINGLE_USE_COMMAND",
    "---DO-NOT-USE---",
    "SCRIPT_ONLY",
    "IGNORES_UNDERPOWERED",
    "USES_MINE_CLEARING_WEAPONSET",
    "CAN_USE_WAYPOINTS",
];

/// `CommandButtonMappedBorderTypeNames`.
pub static COMMAND_BUTTON_BORDER_NAMES: [LookupListRec; 5] = [
    LookupListRec { name: "NONE", value: 0 },
    LookupListRec { name: "BUILD", value: 1 },
    LookupListRec { name: "UPGRADE", value: 2 },
    LookupListRec { name: "ACTION", value: 3 },
    LookupListRec { name: "SYSTEM", value: 4 },
];

/// `TheRadiusCursorNames`.
pub const RADIUS_CURSOR_NAMES: [&str; 29] = [
    "NONE",
    "ATTACK_DAMAGE_AREA",
    "ATTACK_SCATTER_AREA",
    "ATTACK_CONTINUE_AREA",
    "GUARD_AREA",
    "EMERGENCY_REPAIR",
    "FRIENDLY_SPECIALPOWER",
    "OFFENSIVE_SPECIALPOWER",
    "SUPERWEAPON_SCATTER_AREA",
    "PARTICLECANNON",
    "CARPETBOMB",
    "DAISYCUTTER",
    "PARADROP",
    "SPYSATELLITE",
    "SPECTREGUNSHIP",
    "HELIX_NAPALM_BOMB",
    "NUCLEARMISSILE",
    "EMPPULSE",
    "ARTILLERYBARRAGE",
    "NAPALMSTRIKE",
    "CLUSTERMINES",
    "SCUDSTORM",
    "ANTHRAXBOMB",
    "AMBUSH",
    "RADAR",
    "SPYDRONE",
    "FRENZY",
    "CLEARMINES",
    "AMBULANCE",
];

/// `class CommandButton`.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandButton {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_command` (a `GUICommandType`).
    pub command: i32,
    /// `m_options` (`CommandOption` bits).
    pub options: u32,
    /// `m_thingTemplate`: the template's id (`None`: `NULL`).
    pub thing_template: Option<u16>,
    /// `m_upgradeTemplate`: the upgrade's name key (`None`: `NULL`).
    pub upgrade: Option<NameKeyType>,
    /// `m_weaponSlot`.
    pub weapon_slot: i32,
    /// `m_maxShotsToFire`.
    pub max_shots_to_fire: i32,
    /// `m_science`.
    pub science: ScienceVec,
    /// `m_specialPower`: the template's id (`None`: `NULL`).
    pub special_power: Option<u32>,
    /// `m_textLabel`, `m_descriptionLabel`, `m_purchasedLabel`, `m_conflictingLabel` (the client's).
    pub labels: [Vec<u8>; 4],
    /// `m_buttonImageName`, `m_cursorName`, `m_invalidCursorName` (the client's).
    pub images: [Vec<u8>; 3],
    /// `m_commandButtonBorder` (the client's).
    pub border: i32,
    /// `m_radiusCursor` (the client's).
    pub radius_cursor: i32,
    /// `m_unitSpecificSound`'s event name (the client's; empty: `NoSound`).
    pub unit_specific_sound: Vec<u8>,
    /// `markAsOverride()`.
    pub is_override: bool,
}

impl CommandButton {
    /// `CommandButton()`: no command, the primary slot, a "huge number" of shots.
    pub fn new(name: &[u8]) -> Self {
        CommandButton {
            name: name.to_vec(),
            command: 0,
            options: 0,
            thing_template: None,
            upgrade: None,
            weapon_slot: 0,
            max_shots_to_fire: 0x7fff_ffff,
            science: Vec::new(),
            special_power: None,
            labels: Default::default(),
            images: Default::default(),
            border: 0,
            radius_cursor: 0,
            unit_specific_sound: Vec::new(),
            is_override: false,
        }
    }
}

/// `class CommandSet`: its slots, each a button of the store (the button's chain and the override it was then).
#[derive(Clone, Debug, PartialEq)]
pub struct CommandSet {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_command[]`.
    pub commands: [Option<(usize, usize)>; MAX_COMMANDS_PER_SET],
    /// `markAsOverride()`.
    pub is_override: bool,
}

/// What a button's fields look up: the thing factory, the upgrade center, the special power store and the science store, by name.
#[derive(Clone, Debug, Default)]
pub struct CommandButtonLookups {
    /// `TheThingFactory->findTemplate(name)`'s id.
    pub templates: BTreeMap<Vec<u8>, u16>,
    /// `TheUpgradeCenter->findUpgrade(name)`'s name key.
    pub upgrades: BTreeMap<Vec<u8>, NameKeyType>,
    /// `TheSpecialPowerStore->findSpecialPowerTemplate(name)`'s id.
    pub special_powers: BTreeMap<Vec<u8>, u32>,
    /// The sciences `isValidScience` knows.
    pub sciences: Vec<ScienceType>,
}

/// The target of a button's fields: the button, the name keys (moved in for the parse) and the lookups.
pub struct ButtonParse {
    button: CommandButton,
    names: NameKeyGenerator,
    lookups: Arc<CommandButtonLookups>,
}

fn text(ini: &mut Ini<'_>) -> IniResult<Vec<u8>> {
    ini.get_next_token(None)
}

/// `s_commandButtonFieldParseTable`.
static BUTTON_FIELDS: [FieldParse<ButtonParse>; 18] = [
    FieldParse::new(
        "Command",
        |ini, p, _| {
            // `parseCommand`: the name's place in `TheGuiCommandNames`, compared without case; else `INI_INVALID_DATA`
            let token = ini.get_next_token(None)?;
            let i = GUI_COMMAND_NAMES.iter().position(|n| n.as_bytes().eq_ignore_ascii_case(&token)).ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
            p.button.command = i as i32;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("Options", |ini, p, _| ini.parse_bit_string32(&mut p.button.options, &COMMAND_OPTION_NAMES), UserData::None),
    FieldParse::new(
        "Object",
        |ini, p, _| {
            // `parseThingTemplate`: `None` is `NULL`; an unknown name is `NULL` too (a debug crash upstream)
            let token = text(ini)?;
            p.button.thing_template = if token.eq_ignore_ascii_case(b"None") { None } else { p.lookups.templates.get(&token).copied() };
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "Upgrade",
        |ini, p, _| {
            // `parseUpgradeTemplate`: `findUpgrade(name)`, `NULL` when there is none
            let token = text(ini)?;
            p.button.upgrade = p.lookups.upgrades.get(&token).copied();
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "WeaponSlot",
        |ini, p, _| {
            p.button.weapon_slot = ini.parse_lookup_list(&WEAPON_SLOT_TYPE_NAMES_LOOKUP_LIST)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "MaxShotsToFire",
        |ini, p, _| {
            p.button.max_shots_to_fire = ini.parse_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "Science",
        |ini, p, _| {
            let sciences = p.lookups.sciences.clone();
            parse_science_vector(ini, &mut p.names, |s| sciences.contains(&s), &mut p.button.science)
        },
        UserData::None,
    ),
    FieldParse::new(
        "SpecialPower",
        |ini, p, _| {
            // `parseSpecialPowerTemplate`: `findSpecialPowerTemplate(name)`, `NULL` when there is none
            let token = text(ini)?;
            p.button.special_power = p.lookups.special_powers.get(&token).copied();
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("TextLabel", |ini, p, _| text(ini).map(|t| p.button.labels[0] = t), UserData::None),
    FieldParse::new("DescriptLabel", |ini, p, _| text(ini).map(|t| p.button.labels[1] = t), UserData::None),
    FieldParse::new("PurchasedLabel", |ini, p, _| text(ini).map(|t| p.button.labels[2] = t), UserData::None),
    FieldParse::new("ConflictingLabel", |ini, p, _| text(ini).map(|t| p.button.labels[3] = t), UserData::None),
    FieldParse::new("ButtonImage", |ini, p, _| text(ini).map(|t| p.button.images[0] = t), UserData::None),
    FieldParse::new("CursorName", |ini, p, _| text(ini).map(|t| p.button.images[1] = t), UserData::None),
    FieldParse::new("InvalidCursorName", |ini, p, _| text(ini).map(|t| p.button.images[2] = t), UserData::None),
    FieldParse::new(
        "ButtonBorderType",
        |ini, p, _| {
            p.button.border = ini.parse_lookup_list(&COMMAND_BUTTON_BORDER_NAMES)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "RadiusCursorType",
        |ini, p, _| {
            p.button.radius_cursor = ini.parse_index_list(&RADIUS_CURSOR_NAMES)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "UnitSpecificSound",
        |ini, p, _| {
            // `parseAudioEventRTS`: `NoSound` leaves the event unnamed
            let token = text(ini)?;
            if !token.eq_ignore_ascii_case(b"NoSound") {
                p.button.unit_specific_sound = token;
            }
            Ok(())
        },
        UserData::None,
    ),
];

/// The target of a set's slots: the set and the buttons it names.
struct SetParse<'a> {
    set: CommandSet,
    store: &'a CommandButtonStore,
}

/// `CommandSet::parseCommandButton` for slot `index`: the button named, `INI_INVALID_DATA` for an unknown one.
fn set_slot(ini: &mut Ini<'_>, p: &mut SetParse<'_>, index: usize) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    let chain = p.store.find_command_button_chain(&token).ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
    p.set.commands[index] = Some((chain, p.store.buttons[chain].len() - 1));
    Ok(())
}

macro_rules! slot {
    ($n:literal, $i:literal) => {
        FieldParse::new($n, |ini, p: &mut SetParse<'_>, _| set_slot(ini, p, $i), UserData::None)
    };
}

/// `CommandSet`'s field table: slots `1` to `18`.
fn set_fields<'a>() -> [FieldParse<SetParse<'a>>; MAX_COMMANDS_PER_SET] {
    [
        slot!("1", 0),
        slot!("2", 1),
        slot!("3", 2),
        slot!("4", 3),
        slot!("5", 4),
        slot!("6", 5),
        slot!("7", 6),
        slot!("8", 7),
        slot!("9", 8),
        slot!("10", 9),
        slot!("11", 10),
        slot!("12", 11),
        slot!("13", 12),
        slot!("14", 13),
        slot!("15", 14),
        slot!("16", 15),
        slot!("17", 16),
        slot!("18", 17),
    ]
}

/// The control bar's buttons and sets: each name's chain of definitions, the original first, its overrides after.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandButtonStore {
    /// `m_commandButtons`, each a chain of overrides.
    pub buttons: Vec<Vec<CommandButton>>,
    /// `m_commandSets`, each a chain of overrides.
    pub sets: Vec<Vec<CommandSet>>,
}

impl CommandButtonStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// The chain of the button named `name` (`findNonConstCommandButton`: compared with case).
    pub fn find_command_button_chain(&self, name: &[u8]) -> Option<usize> {
        self.buttons.iter().position(|c| c[0].name.as_slice() == name)
    }

    /// `findCommandButton(name)`: its final override.
    pub fn find_command_button(&self, name: &[u8]) -> Option<&CommandButton> {
        self.find_command_button_chain(name).and_then(|c| self.buttons[c].last())
    }

    /// The chain of the set named `name` (`findNonConstCommandSet`).
    pub fn find_command_set_chain(&self, name: &[u8]) -> Option<usize> {
        self.sets.iter().position(|c| c[0].name.as_slice() == name)
    }

    /// `findCommandSet(name)`: its final override.
    pub fn find_command_set(&self, name: &[u8]) -> Option<&CommandSet> {
        self.find_command_set_chain(name).and_then(|c| self.sets[c].last())
    }

    /// The button a set's slot holds.
    pub fn slot_button(&self, slot: (usize, usize)) -> Option<&CommandButton> {
        self.buttons.get(slot.0).and_then(|c| c.get(slot.1))
    }

    /// `ControlBar::parseCommandButtonDefinition(ini)`: the name, then the button found (a duplicate outside an override load is a debug
    /// crash upstream and is parsed over), made, or overridden; then its fields. `names` is moved in for the parse and back after.
    pub fn parse_command_button_definition(&mut self, ini: &mut Ini<'_>, names: &mut NameKeyGenerator, lookups: &Arc<CommandButtonLookups>) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        let overrides = ini.get_load_type() == IniLoadType::CreateOverrides;
        let chain = match self.find_command_button_chain(&name) {
            None => {
                let mut b = CommandButton::new(&name);
                b.is_override = overrides;
                self.buttons.push(vec![b]);
                self.buttons.len() - 1
            }
            Some(c) => {
                if overrides {
                    // `newCommandButtonOverride`: a copy of the button, marked
                    let mut b = self.buttons[c].last().expect("a chain is never empty").clone();
                    b.is_override = true;
                    self.buttons[c].push(b);
                }
                c
            }
        };
        let button = self.buttons[chain].pop().expect("a chain is never empty");
        let mut p = ButtonParse { button, names: std::mem::take(names), lookups: lookups.clone() };
        let r = ini.init_from_ini(&mut p, &BUTTON_FIELDS);
        *names = p.names;
        self.buttons[chain].push(p.button);
        r
    }

    /// `ControlBar::parseCommandSetDefinition(ini)`: the name, then the set made, or overridden; a duplicate outside an override load
    /// throws `INI_INVALID_DATA`; then its slots.
    pub fn parse_command_set_definition(&mut self, ini: &mut Ini<'_>) -> IniResult<()> {
        let name = ini.get_next_token(None)?;
        let overrides = ini.get_load_type() == IniLoadType::CreateOverrides;
        let (chain, set) = match self.find_command_set_chain(&name) {
            None => (None, CommandSet { name: name.clone(), commands: [None; MAX_COMMANDS_PER_SET], is_override: overrides }),
            Some(_) if !overrides => return Err(IniError::new(IniErrorKind::InvalidData)),
            Some(c) => {
                let mut s = self.sets[c].last().expect("a chain is never empty").clone();
                s.is_override = true;
                (Some(c), s)
            }
        };
        let fields = set_fields();
        let mut p = SetParse { set, store: self };
        ini.init_from_ini(&mut p, &fields)?;
        let set = p.set;
        match chain {
            None => self.sets.push(vec![set]),
            Some(c) => self.sets[c].push(set),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::BlockParse;

    struct Ctx {
        store: CommandButtonStore,
        names: NameKeyGenerator,
        lookups: Arc<CommandButtonLookups>,
    }

    fn button(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store.parse_command_button_definition(ini, &mut c.names, &c.lookups)
    }
    fn set(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store.parse_command_set_definition(ini)
    }

    fn load(text: &str) -> (Ctx, IniResult<()>) {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut lookups = CommandButtonLookups::default();
        lookups.templates.insert(b"Tank".to_vec(), 7);
        lookups.special_powers.insert(b"SuperweaponX".to_vec(), 3);
        let mut c = Ctx { store: CommandButtonStore::new(), names, lookups: Arc::new(lookups) };
        static BLOCKS: [BlockParse<Ctx>; 2] = [BlockParse { token: "CommandButton", parse: button }, BlockParse { token: "CommandSet", parse: set }];
        let r = Ini::load(b"Data\\INI\\CommandButton.ini", text.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut c);
        (c, r)
    }

    /// Batch I19: the buttons' and sets' parse. Predicted from `s_commandButtonFieldParseTable`, `TheGuiCommandNames` and
    /// `TheCommandOptionNames`: `Command = HIJACK_VEHICLE` is 27 (no `ALLOW_SURRENDER`: no `POW_RETURN_TO_PRISON` before it);
    /// `Options = NEED_TARGET_ENEMY_OBJECT SCRIPT_ONLY` is bits 0 and 19 (`SCRIPT_ONLY` is in place 19, after the
    /// `---DO-NOT-USE---` placeholder at 18); `Object = Tank` is template 7; an unknown `SpecialPower` is `NULL`; an unset button
    /// keeps the primary slot and `0x7fffffff` shots; a set's slot `3` holds the button named; a set naming an unknown button, and a
    /// duplicate set, are `INI_INVALID_DATA`.
    #[test]
    fn buttons_and_sets_parse_as_upstream() {
        let text = "\
CommandButton Command_Hijack\n Command = HIJACK_VEHICLE\n Options = NEED_TARGET_ENEMY_OBJECT SCRIPT_ONLY\n Object = Tank\n SpecialPower = NoSuchPower\n TextLabel = CONTROLBAR:Hijack\nEnd\n\
CommandButton Command_Stop\n Command = STOP\nEnd\n\
CommandSet TankSet\n 3 = Command_Hijack\n 1 = Command_Stop\nEnd\n";
        let (c, r) = load(text);
        r.expect("the INI");
        let h = c.store.find_command_button(b"Command_Hijack").expect("the button");
        assert_eq!((h.command, h.options, h.thing_template, h.special_power), (27, 1 | (1 << 19), Some(7), None));
        assert_eq!(h.labels[0], b"CONTROLBAR:Hijack");
        let s = c.store.find_command_button(b"Command_Stop").expect("the button");
        assert_eq!((s.command, s.weapon_slot, s.max_shots_to_fire), (12, 0, 0x7fff_ffff));
        let set = c.store.find_command_set(b"TankSet").expect("the set");
        assert_eq!(c.store.slot_button(set.commands[2].expect("slot 3")).map(|b| b.name.clone()), Some(b"Command_Hijack".to_vec()));
        assert_eq!(c.store.slot_button(set.commands[0].expect("slot 1")).map(|b| b.name.clone()), Some(b"Command_Stop".to_vec()));
        assert!(set.commands[1].is_none());
        let (_, r) = load("CommandSet Bad\n 1 = NoSuchButton\nEnd\n");
        assert_eq!(r.err().map(|e| e.kind), Some(IniErrorKind::InvalidData));
        let (_, r) = load("CommandButton B\n Command = STOP\nEnd\nCommandSet S\n 1 = B\nEnd\nCommandSet S\n 1 = B\nEnd\n");
        assert_eq!(r.err().map(|e| e.kind), Some(IniErrorKind::InvalidData));
    }
}
