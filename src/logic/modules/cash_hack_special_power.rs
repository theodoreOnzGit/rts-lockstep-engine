// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CashHackSpecialPower.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/CashHackSpecialPower.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CashHackSpecialPower`: the hacker's theft. At an object (not disabled, a target given), after the base's work, up to `MoneyAmount` of the
//! target's player's money (or the amount of the first `UpgradeMoneyAmount` whose science the object's player has) goes to the object's player,
//! counted as money earned, with a floating "+cash" above the object and "-cash" above the target. At a location it does nothing (not even the
//! base's).
//!
//! The money calls are cuts, as the sabotage crates' are ([`CutCall::PlayerMoneyWithdraw`](crate::logic::world::object_cuts::CutCall),
//! `PlayerMoneyDeposit`, `PlayerScoreAddMoneyEarned`, `InGameUiAddFloatingTextNamed`). Either player `NULL` is
//! [`Undefined::NullControllingPlayer`] (upstream reads both unchecked).
//!
//! # Deviation: the sciences of `UpgradeMoneyAmount`
//!
//! `INI::parseScience` needs `TheScienceStore`, which a module's data proc does not have: the science is kept as its name and looked up in
//! `world.statics.science_store` when the amount is found, as `CrateCollide` does its `PickupScience`. A name the store does not know is an
//! `INI_INVALID_DATA` throw upstream when the data is read; here it is [`Undefined::UnknownScience`] when it is used.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
use crate::logic::modules::special_power_body::{self, special_power_module_call, SpecialPowerBehavior};
use crate::logic::world::game_logic::World;
use crate::common::base_type::Coord3D;
use crate::common::ini::game_make_color;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::update_body::UpdateBody;
use crate::special_power_data_field_parse;

/// `CashHackSpecialPowerModuleData::Upgrades`: a science and the amount it steals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CashHackUpgrade {
    /// `m_science`, as its name (see the module docs).
    pub science: Vec<u8>,
    /// `m_amountToSteal`.
    pub amount_to_steal: i32,
}

/// `class CashHackSpecialPowerModuleData : public SpecialPowerModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CashHackSpecialPowerModuleData {
    /// The `SpecialPowerModuleData` base.
    pub special_power: SpecialPowerModuleData,
    /// `m_upgrades`, in the order read.
    pub upgrades: Vec<CashHackUpgrade>,
    /// `m_defaultAmountToSteal` (`MoneyAmount`).
    pub default_amount_to_steal: i32,
}

impl CashHackSpecialPowerModuleData {
    /// `CashHackSpecialPowerModuleData()`.
    pub fn new() -> Self {
        CashHackSpecialPowerModuleData { special_power: SpecialPowerModuleData::new(), upgrades: Vec::new(), default_amount_to_steal: 0 }
    }

    /// `buildFieldParse(p)`: the behaviour base, the special power fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CashHackSpecialPowerModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CashHackSpecialPowerModuleData> {
        let mut data = CashHackSpecialPowerModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CashHackSpecialPowerModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `SpecialPowerModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<CashHackSpecialPowerModuleData>; 6] = special_power_data_field_parse!(CashHackSpecialPowerModuleData, special_power);

/// `parseCashHackUpgradePair`: a science (`INI::parseScience`, kept as its name), then an `Int`, appended.
fn parse_cash_hack_upgrade_pair(ini: &mut Ini<'_>, d: &mut CashHackSpecialPowerModuleData, _ud: &UserData) -> IniResult<()> {
    let science = ini.get_next_token(None)?;
    let amount_to_steal = ini.parse_int()?;
    d.upgrades.push(CashHackUpgrade { science, amount_to_steal });
    Ok(())
}

/// `CashHackSpecialPowerModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<CashHackSpecialPowerModuleData>; 3] = [
    FieldParse::new("UpgradeMoneyAmount", parse_cash_hack_upgrade_pair, UserData::None),
    FieldParse::new("MoneyAmount", |ini, d, _| { d.default_amount_to_steal = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class CashHackSpecialPower : public SpecialPowerModule`.
#[derive(Clone, Debug)]
pub struct CashHackSpecialPower {
    /// The `SpecialPowerModule` base.
    power: SpecialPowerModule,
    /// `getCashHackSpecialPowerModuleData()`.
    data: Arc<CashHackSpecialPowerModuleData>,
}

impl PartialEq for CashHackSpecialPower {
    /// The same object, the same data object (upstream compares pointers) and the same base state.
    fn eq(&self, other: &Self) -> bool {
        self.power.behavior().object() == other.power.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.power.get_available_on_frame() == other.power.get_available_on_frame()
            && self.power.paused_count() == other.power.paused_count()
            && self.power.paused_on_frame() == other.power.paused_on_frame()
            && self.power.paused_percent().to_bits() == other.power.paused_percent().to_bits()
    }
}

impl CashHackSpecialPower {
    /// `CashHackSpecialPower(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<CashHackSpecialPowerModuleData>>) -> IniResult<CashHackSpecialPower> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let power = SpecialPowerModule::new(object, Some(Arc::new(data.special_power.clone())))?;
        Ok(CashHackSpecialPower { power, data })
    }

    /// `SpecialPowerModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerModule::get_interface_mask()
    }

    /// The data, as `getCashHackSpecialPowerModuleData()`.
    pub fn module_data(&self) -> &Arc<CashHackSpecialPowerModuleData> {
        &self.data
    }

    /// The world half of the constructor, run once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        if let Err(e) = special_power_body::construct(self, world, me) {
            world.cuts.module_errors.push((me, e));
        }
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        special_power_module_call(self, world, me, call)
    }

    /// `findAmountToSteal()`: the amount of the first upgrade whose science the controlling player has, else `MoneyAmount`.
    fn find_amount_to_steal(&self, world: &mut World, me: ObjectId) -> Result<i32, IniError> {
        if let Some(p) = world.get_controlling_player(me) {
            for up in &self.data.upgrades {
                let statics = world.statics.clone();
                let science = statics.science_store.friend_lookup_science(&mut world.names, &up.science).map_err(|_| IniError::undefined(Undefined::UnknownScience))?;
                if world.players.get_nth(p).is_some_and(|pl| pl.sciences.contains(&science)) && !crate::logic::modules::plant(1901) {
                    return Ok(up.amount_to_steal);
                }
            }
        }
        Ok(self.data.default_amount_to_steal)
    }
}

impl SpecialPowerBehavior for CashHackSpecialPower {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CashHackSpecialPower(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CashHackSpecialPower(m) => Some(m),
            _ => None,
        }
    }
    /// `doSpecialPowerAtLocation`: nothing (a disabled object returns first; either way, not the base's).
    fn do_special_power_at_location(&mut self, _world: &mut World, _me: ModuleHandle, _loc: Coord3D, _angle: f32, _command_options: u32) -> Result<(), IniError> {
        Ok(())
    }
    fn do_special_power_at_object(&mut self, world: &mut World, me: ModuleHandle, target: ObjectId, command_options: u32) -> Result<(), IniError> {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) {
            return Ok(());
        }
        if world.objects.find(target).is_none() {
            return Ok(());
        }
        special_power_body::base_do_special_power_at_object(self, world, me, target, command_options)?;
        let us = me.object;
        // `victim->getControllingPlayer()->getMoney()` and the object's, both unchecked
        let victim = world.get_controlling_player(target).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let thief = world.get_controlling_player(us).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let available = world.players.get_nth(victim).map_or(0, |p| p.money.count_money());
        let desired = self.find_amount_to_steal(world, us)? as u32;
        let cash = if crate::logic::modules::plant(1902) { desired } else { desired.min(available) };
        if cash > 0 {
            world.cut(CutCall::PlayerMoneyWithdraw { player: victim, amount: cash });
            world.cut(CutCall::PlayerMoneyDeposit { player: thief, amount: cash });
            world.cut(CutCall::PlayerScoreAddMoneyEarned { player: thief, amount: cash as i32 });
            // "add a little z to make it show up above the unit"
            let mut pos = world.objects.find(us).map_or(Coord3D::default(), |o| o.get_position());
            pos.z += 20.0;
            world.cut(CutCall::InGameUiAddFloatingTextNamed { text: b"GUI:AddCash".to_vec(), amount: cash as i32, position: pos, color: game_make_color(0, 255, 0, 255) });
            let mut pos = world.objects.find(target).map_or(Coord3D::default(), |o| o.get_position());
            pos.z += if crate::logic::modules::plant(1903) { 20.0 } else { 30.0 };
            world.cut(CutCall::InGameUiAddFloatingTextNamed { text: b"GUI:LoseCash".to_vec(), amount: cash as i32, position: pos, color: game_make_color(255, 0, 0, 255) });
        }
        Ok(())
    }
}

impl Snapshot for CashHackSpecialPower {
    /// `crc`: `SpecialPowerModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.power.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `SpecialPowerModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.power.xfer(xfer)
    }
    /// `loadPostProcess`: `SpecialPowerModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.power.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `CashHackSpecialPower`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CashHackSpecialPower(Arc::new(CashHackSpecialPowerModuleData::friend_new_module_data(ini)?)))
}
