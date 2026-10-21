// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpyVisionSpecialPower.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/SpyVisionSpecialPower.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpyVisionSpecialPower`: the spy satellite's power. `doSpecialPower` does the base's and then tells the object's `SpyVisionUpdate` to show the
//! enemy's view for `BaseDuration` plus `BonusDurationPerCaptured` for each unit the object holds, at most `MaxDuration`
//! ([`ModuleCall::ActivateSpyVision`]). A disabled object does nothing. The arithmetic is `UnsignedInt`: the count of contained items is converted
//! to unsigned for the product, which wraps.

use std::sync::Arc;

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

/// `class SpyVisionSpecialPowerModuleData : public SpecialPowerModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpyVisionSpecialPowerModuleData {
    /// The `SpecialPowerModuleData` base.
    pub special_power: SpecialPowerModuleData,
    /// `m_baseDurationInFrames`.
    pub base_duration_in_frames: u32,
    /// `m_bonusDurationPerCapturedInFrames`.
    pub bonus_duration_per_captured_in_frames: u32,
    /// `m_maxDurationInFrames`.
    pub max_duration_in_frames: u32,
}

impl SpyVisionSpecialPowerModuleData {
    /// `SpyVisionSpecialPowerModuleData()`.
    pub fn new() -> Self {
        SpyVisionSpecialPowerModuleData {
            special_power: SpecialPowerModuleData::new(),
            base_duration_in_frames: 0,
            bonus_duration_per_captured_in_frames: 0,
            max_duration_in_frames: 0,
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the special power fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SpyVisionSpecialPowerModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SpyVisionSpecialPowerModuleData> {
        let mut data = SpyVisionSpecialPowerModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SpyVisionSpecialPowerModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `SpecialPowerModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<SpyVisionSpecialPowerModuleData>; 6] = special_power_data_field_parse!(SpyVisionSpecialPowerModuleData, special_power);

/// `SpyVisionSpecialPowerModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<SpyVisionSpecialPowerModuleData>; 4] = [
    FieldParse::new("BaseDuration", |ini, d, _| { d.base_duration_in_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("BonusDurationPerCaptured", |ini, d, _| { d.bonus_duration_per_captured_in_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxDuration", |ini, d, _| { d.max_duration_in_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class SpyVisionSpecialPower : public SpecialPowerModule`.
#[derive(Clone, Debug)]
pub struct SpyVisionSpecialPower {
    /// The `SpecialPowerModule` base.
    power: SpecialPowerModule,
    /// `getSpyVisionSpecialPowerModuleData()`.
    data: Arc<SpyVisionSpecialPowerModuleData>,
}

impl PartialEq for SpyVisionSpecialPower {
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

impl SpyVisionSpecialPower {
    /// `SpyVisionSpecialPower(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<SpyVisionSpecialPowerModuleData>>) -> IniResult<SpyVisionSpecialPower> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let power = SpecialPowerModule::new(object, Some(Arc::new(data.special_power.clone())))?;
        Ok(SpyVisionSpecialPower { power, data })
    }

    /// `SpecialPowerModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerModule::get_interface_mask()
    }

    /// The data, as `getSpyVisionSpecialPowerModuleData()`.
    pub fn module_data(&self) -> &Arc<SpyVisionSpecialPowerModuleData> {
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

impl SpecialPowerBehavior for SpyVisionSpecialPower {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpyVisionSpecialPower(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpyVisionSpecialPower(m) => Some(m),
            _ => None,
        }
    }
    fn do_special_power(&mut self, world: &mut World, me: ModuleHandle, command_options: u32) -> Result<(), IniError> {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) {
            return Ok(());
        }
        special_power_body::base_do_special_power(self, world, me, command_options)?;
        let mut duration = self.data.base_duration_in_frames;
        if let Some(contain) = world.objects.find(me.object).and_then(|o| o.contain) {
            let count = match world.module_call(me.object, contain, ModuleCall::GetContainCount) {
                ModuleReply::Int(n) => n,
                _ => 0,
            };
            let count = if crate::logic::modules::plant(855) { 0 } else { count };
            duration = duration.wrapping_add((count as u32).wrapping_mul(self.data.bonus_duration_per_captured_in_frames));
            if duration > self.data.max_duration_in_frames && !crate::logic::modules::plant(821) {
                duration = self.data.max_duration_in_frames;
            }
        }
        if let Some(idx) = world.find_module(me.object, b"SpyVisionUpdate") {
            world.module_call(me.object, idx, ModuleCall::ActivateSpyVision { duration });
        }
        Ok(())
    }
}

impl Snapshot for SpyVisionSpecialPower {
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

/// The factory's `NewModuleDataProc` for `SpyVisionSpecialPower`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SpyVisionSpecialPower(Arc::new(SpyVisionSpecialPowerModuleData::friend_new_module_data(ini)?)))
}

