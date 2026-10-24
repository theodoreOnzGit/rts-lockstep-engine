// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CashBountyPower.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/CashBountyPower.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CashBountyPower`: a general's promotion that pays a bounty on enemies killed. When the object is made with its controlling player already
//! having the power's required science, and whenever the special power is created, the player's cash bounty becomes `Bounty` if that is more
//! than it has. (Upstream's science-dependent `UpgradeBounty` list is `#ifdef NOT_IN_USE` and not compiled; so it is not here.)
//!
//! `onObjectCreated` and `onSpecialPowerCreation` are [`ModuleCall::OnObjectCreated`] and [`ModuleCall::OnSpecialPowerCreation`] (the base's
//! first). The required science reads the template, a `NULL` one being [`Undefined::NullSpecialPowerTemplate`]. `doSpecialPower` does nothing
//! (the header's `{ return; }`); the other three `doSpecialPower...` are the base's.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
use crate::logic::modules::special_power_body::{self, special_power_module_call, SpecialPowerBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::special_power_data_field_parse;

/// `class CashBountyPowerModuleData : public SpecialPowerModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CashBountyPowerModuleData {
    /// The `SpecialPowerModuleData` base.
    pub special_power: SpecialPowerModuleData,
    /// `m_defaultBounty` (`Bounty`, a percentage).
    pub default_bounty: f32,
}

impl CashBountyPowerModuleData {
    /// `CashBountyPowerModuleData()`.
    pub fn new() -> Self {
        CashBountyPowerModuleData { special_power: SpecialPowerModuleData::new(), default_bounty: 0.0 }
    }

    /// `buildFieldParse(p)`: the behaviour base, the special power fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CashBountyPowerModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CashBountyPowerModuleData> {
        let mut data = CashBountyPowerModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CashBountyPowerModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `SpecialPowerModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<CashBountyPowerModuleData>; 6] = special_power_data_field_parse!(CashBountyPowerModuleData, special_power);

/// `CashBountyPowerModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<CashBountyPowerModuleData>; 2] = [
    FieldParse::new("Bounty", |ini, d, _| { d.default_bounty = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class CashBountyPower : public SpecialPowerModule`.
#[derive(Clone, Debug)]
pub struct CashBountyPower {
    /// The `SpecialPowerModule` base.
    power: SpecialPowerModule,
    /// `getCashBountyPowerModuleData()`.
    data: Arc<CashBountyPowerModuleData>,
}

impl PartialEq for CashBountyPower {
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

impl CashBountyPower {
    /// `CashBountyPower(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<CashBountyPowerModuleData>>) -> IniResult<CashBountyPower> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let power = SpecialPowerModule::new(object, Some(Arc::new(data.special_power.clone())))?;
        Ok(CashBountyPower { power, data })
    }

    /// `SpecialPowerModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerModule::get_interface_mask()
    }

    /// The data, as `getCashBountyPowerModuleData()`.
    pub fn module_data(&self) -> &Arc<CashBountyPowerModuleData> {
        &self.data
    }

    /// The world half of the constructor, run once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        if let Err(e) = special_power_body::construct(self, world, me) {
            world.cuts.module_errors.push((me, e));
        }
    }

    /// The module's answer to one of `Object`'s calls: `onObjectCreated`, and `onSpecialPowerCreation` after the base's.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                if let Err(e) = self.on_object_created(world, me) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            ModuleCall::OnSpecialPowerCreation => {
                let r = special_power_module_call(self, world, me, ModuleCall::OnSpecialPowerCreation);
                if let Some(p) = world.get_controlling_player(me.object) {
                    self.raise_bounty(world, p);
                }
                r
            }
            other => special_power_module_call(self, world, me, other),
        }
    }

    /// `onObjectCreated()`: a controlling player that has the required science gets the bounty.
    fn on_object_created(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let Some(p) = world.get_controlling_player(me.object) else { return Ok(()) };
        let science = special_power_body::template_required_science(world, &self.power).ok_or_else(|| IniError::undefined(Undefined::NullSpecialPowerTemplate))?;
        if world.players.get_nth(p).is_some_and(|pl| pl.sciences.contains(&science)) != crate::logic::modules::plant(1891) {
            self.raise_bounty(world, p);
        }
        Ok(())
    }

    /// `findBounty()` ("NOT_IN_USE": the default), set as player `p`'s cash bounty if it is more.
    fn raise_bounty(&self, world: &mut World, p: crate::logic::world::ids::PlayerIndex) {
        let bounty = self.data.default_bounty;
        if let Some(pl) = world.players.get_nth_mut(p) {
            if bounty > pl.cash_bounty_percent || crate::logic::modules::plant(1892) {
                pl.cash_bounty_percent = bounty;
            }
        }
    }
}

impl SpecialPowerBehavior for CashBountyPower {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CashBountyPower(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CashBountyPower(m) => Some(m),
            _ => None,
        }
    }
    /// `doSpecialPower(commandOptions)`: `{ return; }` (the header's), so not the base's.
    fn do_special_power(&mut self, world: &mut World, me: ModuleHandle, command_options: u32) -> Result<(), IniError> {
        if crate::logic::modules::plant(1893) {
            return special_power_body::base_do_special_power(self, world, me, command_options);
        }
        Ok(())
    }
}

impl Snapshot for CashBountyPower {
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

/// The factory's `NewModuleDataProc` for `CashBountyPower`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CashBountyPower(Arc::new(CashBountyPowerModuleData::friend_new_module_data(ini)?)))
}
