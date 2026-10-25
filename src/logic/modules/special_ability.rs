// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpecialAbility.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/SpecialAbility.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpecialAbility`: a special power module for a unit's own ability (the work is done by its `SpecialAbilityUpdate`, which the base's
//! intent reaches). Each of the three `doSpecialPower` calls does nothing for a disabled object, and the one at an object nothing with no
//! target; otherwise it does the base's. (The one at a location also returns on a `NULL` location, which the interface here cannot pass.) The
//! disabled tests repeat the base's own (`SpecialPowerModule.cpp` returns for a disabled object too), so they change nothing that can be seen.
//!
//! `p1w13-sonnet`, batch R51, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
use crate::logic::modules::special_power_body::{self, special_power_module_call, SpecialPowerBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::special_power_data_field_parse;

/// `class SpecialAbilityModuleData : public SpecialPowerModuleData` ("nothing" of its own).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpecialAbilityModuleData {
    /// The `SpecialPowerModuleData` base.
    pub special_power: SpecialPowerModuleData,
}

impl SpecialAbilityModuleData {
    /// `SpecialAbilityModuleData()`.
    pub fn new() -> Self {
        SpecialAbilityModuleData { special_power: SpecialPowerModuleData::new() }
    }

    /// `buildFieldParse(p)`: the behaviour base, then the special power fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SpecialAbilityModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SpecialAbilityModuleData> {
        let mut data = SpecialAbilityModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SpecialAbilityModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `SpecialPowerModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<SpecialAbilityModuleData>; 6] = special_power_data_field_parse!(SpecialAbilityModuleData, special_power);

/// `class SpecialAbility : public SpecialPowerModule`.
#[derive(Clone, Debug)]
pub struct SpecialAbility {
    /// The `SpecialPowerModule` base.
    power: SpecialPowerModule,
    /// `getSpecialAbilityModuleData()`.
    data: Arc<SpecialAbilityModuleData>,
}

impl PartialEq for SpecialAbility {
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

impl SpecialAbility {
    /// `SpecialAbility(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<SpecialAbilityModuleData>>) -> IniResult<SpecialAbility> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let power = SpecialPowerModule::new(object, Some(Arc::new(data.special_power.clone())))?;
        Ok(SpecialAbility { power, data })
    }

    /// `SpecialPowerModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerModule::get_interface_mask()
    }

    /// The data, as `getSpecialAbilityModuleData()`.
    pub fn module_data(&self) -> &Arc<SpecialAbilityModuleData> {
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
}

impl SpecialPowerBehavior for SpecialAbility {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpecialAbility(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpecialAbility(m) => Some(m),
            _ => None,
        }
    }
    fn do_special_power_at_location(&mut self, world: &mut World, me: ModuleHandle, loc: Coord3D, angle: f32, command_options: u32) -> Result<(), IniError> {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) {
            return Ok(());
        }
        if crate::logic::modules::plant(2213) {
            return special_power_body::base_do_special_power(self, world, me, command_options);
        }
        special_power_body::base_do_special_power_at_location(self, world, me, loc, angle, command_options)
    }
    fn do_special_power_at_object(&mut self, world: &mut World, me: ModuleHandle, target: ObjectId, command_options: u32) -> Result<(), IniError> {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) {
            return Ok(());
        }
        if world.objects.find(target).is_none() {
            return Ok(());
        }
        if crate::logic::modules::plant(2212) {
            return special_power_body::base_do_special_power(self, world, me, command_options);
        }
        if crate::logic::modules::plant(2214) {
            return special_power_body::base_do_special_power_at_object(self, world, me, me.object, command_options);
        }
        special_power_body::base_do_special_power_at_object(self, world, me, target, command_options)
    }
    fn do_special_power(&mut self, world: &mut World, me: ModuleHandle, command_options: u32) -> Result<(), IniError> {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) {
            return Ok(());
        }
        if crate::logic::modules::plant(2211) {
            special_power_body::base_do_special_power(self, world, me, command_options)?;
        }
        special_power_body::base_do_special_power(self, world, me, command_options)
    }
}

impl Snapshot for SpecialAbility {
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

/// The factory's `NewModuleDataProc` for `SpecialAbility`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SpecialAbility(Arc::new(SpecialAbilityModuleData::friend_new_module_data(ini)?)))
}

