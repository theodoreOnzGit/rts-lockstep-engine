// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CleanupAreaPower.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/CleanupAreaPower.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CleanupAreaPower`: a special power that sends the object's `CleanupHazardUpdate` to clean up an area. `doSpecialPowerAtLocation` hands the update the
//! location and `MaxMoveDistanceFromLocation` ([`ModuleCall::SetCleanupAreaParameters`]; an object with no such update is a `DEBUG_CRASH` upstream,
//! compiled out here) and does not call the base: no recharge, no sound, no script notice. A disabled object does nothing.

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
use crate::logic::world::module_call::ModuleCall as Call;

/// `class CleanupAreaPowerModuleData : public SpecialPowerModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CleanupAreaPowerModuleData {
    /// The `SpecialPowerModuleData` base.
    pub special_power: SpecialPowerModuleData,
    /// `m_cleanupMoveRange`.
    pub cleanup_move_range: f32,
}

impl CleanupAreaPowerModuleData {
    /// `CleanupAreaPowerModuleData()`.
    pub fn new() -> Self {
        CleanupAreaPowerModuleData {
            special_power: SpecialPowerModuleData::new(),
            cleanup_move_range: 0.0,
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the special power fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CleanupAreaPowerModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CleanupAreaPowerModuleData> {
        let mut data = CleanupAreaPowerModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CleanupAreaPowerModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `SpecialPowerModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<CleanupAreaPowerModuleData>; 6] = special_power_data_field_parse!(CleanupAreaPowerModuleData, special_power);

/// `CleanupAreaPowerModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<CleanupAreaPowerModuleData>; 2] = [
    FieldParse::new("MaxMoveDistanceFromLocation", |ini, d, _| { d.cleanup_move_range = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class CleanupAreaPower : public SpecialPowerModule`.
#[derive(Clone, Debug)]
pub struct CleanupAreaPower {
    /// The `SpecialPowerModule` base.
    power: SpecialPowerModule,
    /// `getCleanupAreaPowerModuleData()`.
    data: Arc<CleanupAreaPowerModuleData>,
}

impl PartialEq for CleanupAreaPower {
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

impl CleanupAreaPower {
    /// `CleanupAreaPower(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<CleanupAreaPowerModuleData>>) -> IniResult<CleanupAreaPower> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let power = SpecialPowerModule::new(object, Some(Arc::new(data.special_power.clone())))?;
        Ok(CleanupAreaPower { power, data })
    }

    /// `SpecialPowerModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerModule::get_interface_mask()
    }

    /// The data, as `getCleanupAreaPowerModuleData()`.
    pub fn module_data(&self) -> &Arc<CleanupAreaPowerModuleData> {
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

impl SpecialPowerBehavior for CleanupAreaPower {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CleanupAreaPower(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CleanupAreaPower(m) => Some(m),
            _ => None,
        }
    }
    fn do_special_power_at_location(&mut self, world: &mut World, me: ModuleHandle, loc: Coord3D, _angle: f32, _command_options: u32) -> Result<(), IniError> {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) {
            return Ok(());
        }
        if let Some(idx) = world.find_module(me.object, b"CleanupHazardUpdate") {
            let range = if crate::logic::modules::plant(801) { 0.0 } else { self.data.cleanup_move_range };
            world.module_call(me.object, idx, Call::SetCleanupAreaParameters { loc, range });
        }
        Ok(())
    }
}

impl Snapshot for CleanupAreaPower {
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

/// The factory's `NewModuleDataProc` for `CleanupAreaPower`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CleanupAreaPower(Arc::new(CleanupAreaPowerModuleData::friend_new_module_data(ini)?)))
}

