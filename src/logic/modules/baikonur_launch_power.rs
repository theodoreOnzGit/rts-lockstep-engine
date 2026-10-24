// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BaikonurLaunchPower.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/BaikonurLaunchPower.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BaikonurLaunchPower`: the Baikonur launch pad's power. `doSpecialPower` does the base's and then opens the pad's door
//! (`MODELCONDITION_DOOR_1_OPENING`); `doSpecialPowerAtLocation` does the base's and makes the `DetonationObject` on the object's team at the
//! location. A disabled object does nothing.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, UserData};
use crate::common::model_state::MODELCONDITION_DOOR_1_OPENING;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
use crate::logic::modules::special_power_body::{
    self, special_power_module_call, SpecialPowerBehavior,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::special_power_data_field_parse;

/// `class BaikonurLaunchPowerModuleData : public SpecialPowerModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BaikonurLaunchPowerModuleData {
    /// The `SpecialPowerModuleData` base.
    pub special_power: SpecialPowerModuleData,
    /// `m_detonationObject`: the thing to make at the target.
    pub detonation_object: Vec<u8>,
}

impl BaikonurLaunchPowerModuleData {
    /// `BaikonurLaunchPowerModuleData()`.
    pub fn new() -> Self {
        BaikonurLaunchPowerModuleData {
            special_power: SpecialPowerModuleData::new(),
            detonation_object: Vec::new(),
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the special power fields, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, BaikonurLaunchPowerModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<BaikonurLaunchPowerModuleData> {
        let mut data = BaikonurLaunchPowerModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                BaikonurLaunchPowerModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `SpecialPowerModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<BaikonurLaunchPowerModuleData>; 6] =
    special_power_data_field_parse!(BaikonurLaunchPowerModuleData, special_power);

/// `BaikonurLaunchPowerModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<BaikonurLaunchPowerModuleData>; 2] = [
    FieldParse::new(
        "DetonationObject",
        |ini, d, _| {
            d.detonation_object = ini.parse_ascii_string()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class BaikonurLaunchPower : public SpecialPowerModule`.
#[derive(Clone, Debug)]
pub struct BaikonurLaunchPower {
    /// The `SpecialPowerModule` base.
    power: SpecialPowerModule,
    /// `getBaikonurLaunchPowerModuleData()`.
    data: Arc<BaikonurLaunchPowerModuleData>,
}

impl PartialEq for BaikonurLaunchPower {
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

impl BaikonurLaunchPower {
    /// `BaikonurLaunchPower(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<BaikonurLaunchPowerModuleData>>,
    ) -> IniResult<BaikonurLaunchPower> {
        let data =
            data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let power = SpecialPowerModule::new(object, Some(Arc::new(data.special_power.clone())))?;
        Ok(BaikonurLaunchPower { power, data })
    }

    /// `SpecialPowerModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerModule::get_interface_mask()
    }

    /// The data, as `getBaikonurLaunchPowerModuleData()`.
    pub fn module_data(&self) -> &Arc<BaikonurLaunchPowerModuleData> {
        &self.data
    }

    /// The world half of the constructor, run once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        if let Err(e) = special_power_body::construct(self, world, me) {
            world.cuts.module_errors.push((me, e));
        }
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        special_power_module_call(self, world, me, call)
    }
}

impl SpecialPowerBehavior for BaikonurLaunchPower {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::BaikonurLaunchPower(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::BaikonurLaunchPower(m) => Some(m),
            _ => None,
        }
    }
    fn do_special_power(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        command_options: u32,
    ) -> Result<(), IniError> {
        if world
            .objects
            .find(me.object)
            .is_some_and(|o| o.is_disabled())
        {
            return Ok(());
        }
        special_power_body::base_do_special_power(self, world, me, command_options)?;
        if !crate::logic::modules::plant(811) {
            world.set_model_condition_state(me.object, MODELCONDITION_DOOR_1_OPENING);
        }
        Ok(())
    }
    fn do_special_power_at_location(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        loc: Coord3D,
        angle: f32,
        command_options: u32,
    ) -> Result<(), IniError> {
        if world
            .objects
            .find(me.object)
            .is_some_and(|o| o.is_disabled())
        {
            return Ok(());
        }
        special_power_body::base_do_special_power_at_location(
            self,
            world,
            me,
            loc,
            angle,
            command_options,
        )?;
        if let Some(template) = world.thing_factory_find_template(&self.data.detonation_object) {
            let team = world.get_team(me.object);
            if let Ok(detonation) =
                world.thing_factory_new_object(template, team, ObjectStatusMaskType::default())
            {
                world.set_position(detonation, &loc);
            }
        }
        Ok(())
    }
}

impl Snapshot for BaikonurLaunchPower {
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

/// The factory's `NewModuleDataProc` for `BaikonurLaunchPower`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::BaikonurLaunchPower(Arc::new(
        BaikonurLaunchPowerModuleData::friend_new_module_data(ini)?,
    )))
}
