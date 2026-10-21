// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DefectorSpecialPower.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/DefectorSpecialPower.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DefectorSpecialPower`: turns a unit to the object's side. `doSpecialPowerAtObject` does the base's and then `defect`s the target to the object's
//! controlling player's default team for the template's detection time; `doSpecialPowerAtLocation` does nothing. A disabled object does nothing,
//! nor does a call with no target. (No default team is a `NULL` team passed to `defect`: [`Undefined::NullDefaultTeam`].)

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
use crate::logic::modules::special_power_body::{self, special_power_module_call, SpecialPowerBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::special_power_data_field_parse;
use crate::common::ini::Undefined;

/// `class DefectorSpecialPowerModuleData : public SpecialPowerModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DefectorSpecialPowerModuleData {
    /// The `SpecialPowerModuleData` base.
    pub special_power: SpecialPowerModuleData,
    /// `m_fatCursorRadius`.
    pub fat_cursor_radius: f32,
}

impl DefectorSpecialPowerModuleData {
    /// `DefectorSpecialPowerModuleData()`.
    pub fn new() -> Self {
        DefectorSpecialPowerModuleData {
            special_power: SpecialPowerModuleData::new(),
            fat_cursor_radius: 0.0,
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the special power fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, DefectorSpecialPowerModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<DefectorSpecialPowerModuleData> {
        let mut data = DefectorSpecialPowerModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, DefectorSpecialPowerModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `SpecialPowerModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<DefectorSpecialPowerModuleData>; 6] = special_power_data_field_parse!(DefectorSpecialPowerModuleData, special_power);

/// `DefectorSpecialPowerModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<DefectorSpecialPowerModuleData>; 2] = [
    FieldParse::new("FatCursorRadius", |ini, d, _| { d.fat_cursor_radius = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class DefectorSpecialPower : public SpecialPowerModule`.
#[derive(Clone, Debug)]
pub struct DefectorSpecialPower {
    /// The `SpecialPowerModule` base.
    power: SpecialPowerModule,
    /// `getDefectorSpecialPowerModuleData()`.
    data: Arc<DefectorSpecialPowerModuleData>,
}

impl PartialEq for DefectorSpecialPower {
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

impl DefectorSpecialPower {
    /// `DefectorSpecialPower(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<DefectorSpecialPowerModuleData>>) -> IniResult<DefectorSpecialPower> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let power = SpecialPowerModule::new(object, Some(Arc::new(data.special_power.clone())))?;
        Ok(DefectorSpecialPower { power, data })
    }

    /// `SpecialPowerModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerModule::get_interface_mask()
    }

    /// The data, as `getDefectorSpecialPowerModuleData()`.
    pub fn module_data(&self) -> &Arc<DefectorSpecialPowerModuleData> {
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

impl SpecialPowerBehavior for DefectorSpecialPower {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::DefectorSpecialPower(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::DefectorSpecialPower(m) => Some(m),
            _ => None,
        }
    }
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
        let time = special_power_body::template_detection_time(world, self.power()).ok_or_else(|| IniError::undefined(Undefined::NullSpecialPowerTemplate))?;
        let player = world.get_controlling_player(me.object).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let team = world.players.get_nth(player).and_then(|p| p.default_team).ok_or_else(|| IniError::undefined(Undefined::NullDefaultTeam))?;
        if !crate::logic::modules::plant(831) {
            world.defect(target, team, time);
        }
        Ok(())
    }
}

impl Snapshot for DefectorSpecialPower {
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

/// The factory's `NewModuleDataProc` for `DefectorSpecialPower`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::DefectorSpecialPower(Arc::new(DefectorSpecialPowerModuleData::friend_new_module_data(ini)?)))
}

