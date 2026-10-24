// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PowerPlantUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PowerPlantUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PowerPlantUpdate`: the control rods of a power plant, extended when `PowerPlantUpgrade` tells it to.
//!
//! `RodsExtendTime` is `INI::parseDurationUnsignedInt`. The constructor sleeps for ever. `extendRods(TRUE)` ([`ModuleCall::ExtendRods`], which
//! `PowerPlantUpgrade` sends to every module of its object that answers it) does nothing if the rods are out; else the drawable gets
//! `MODELCONDITION_POWER_PLANT_UPGRADING`, the rods are out and the module wakes after `RodsExtendTime` frames. `extendRods(FALSE)` retracts them
//! at once (both model conditions cleared in one call, `MAKE_MODELCONDITION_MASK2`) and sleeps for ever. `update()` swaps the upgrading
//! condition for `MODELCONDITION_POWER_PLANT_UPGRADED` and sleeps for ever. A module without a drawable skips every drawable call.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::common::model_state::{MODELCONDITION_POWER_PLANT_UPGRADED, MODELCONDITION_POWER_PLANT_UPGRADING};
use crate::logic::module::update_module::{update_sleep, UPDATE_SLEEP_FOREVER};
use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};

/// `class PowerPlantUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PowerPlantUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_rodsExtendTime` (frames): the time the rods take to be built.
    pub rods_extend_time: u32,
}

impl PowerPlantUpdateModuleData {
    /// `PowerPlantUpdateModuleData()`.
    pub fn new() -> Self {
        PowerPlantUpdateModuleData {
            update: UpdateModuleData::new(),
            rods_extend_time: 0,
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, PowerPlantUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<PowerPlantUpdateModuleData> {
        let mut data = PowerPlantUpdateModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, PowerPlantUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `PowerPlantUpdateModuleData::dataFieldParse`.
static FIELD_PARSE: [FieldParse<PowerPlantUpdateModuleData>; 2] = [
    FieldParse::new("RodsExtendTime", |ini, d, _| { d.rods_extend_time = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class PowerPlantUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct PowerPlantUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getPowerPlantUpdateModuleData()`.
    data: Arc<PowerPlantUpdateModuleData>,
    /// `m_extended`: TRUE when extend is all done.
    extended: bool,
}

impl PartialEq for PowerPlantUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.extended == other.extended
    }
}

impl PowerPlantUpdate {
    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<PowerPlantUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(PowerPlantUpdate { update, data, extended: false })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getPowerPlantUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<PowerPlantUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's.
    pub fn state_words(&self) -> Vec<u32> {
        vec![u32::from(self.extended)]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, that half, then the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `extendRods(extend)`.
    pub fn extend_rods(&mut self, world: &mut World, me: ModuleHandle, extend: bool) {
        if extend {
            if !self.extended || crate::logic::modules::plant(732) {
                // set the model condition for radar extension
                world.set_model_condition_state(me.object, MODELCONDITION_POWER_PLANT_UPGRADING);
                self.extended = true;
                update_behavior::set_wake_frame(world, update_sleep(self.data.rods_extend_time));
            }
        } else {
            // they de-extend instantly.
            if world.has_drawable(me.object) {
                // MAKE_MODELCONDITION_MASK2: a set of bits, ascending
                let (a, b) = (MODELCONDITION_POWER_PLANT_UPGRADING.min(MODELCONDITION_POWER_PLANT_UPGRADED), MODELCONDITION_POWER_PLANT_UPGRADING.max(MODELCONDITION_POWER_PLANT_UPGRADED));
                world.emit(ObjectClientEvent::DrawableModelCondition { object_id: me.object, op: ModelConditionOp::Clear(vec![a, b]) });
            }
            self.extended = false;
            if !crate::logic::modules::plant(736) {
                update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
            }
        }
    }
}

impl UpdateBehavior for PowerPlantUpdate {
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        // remove the extending condition and set the extened condition
        if !crate::logic::modules::plant(731) {
            world.clear_and_set_model_condition_state(me.object, MODELCONDITION_POWER_PLANT_UPGRADING, MODELCONDITION_POWER_PLANT_UPGRADED);
        }
        self.extended = true;
        UPDATE_SLEEP_FOREVER
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        if !crate::logic::modules::plant(738) {
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
        }
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        let _ = version;
        xfer.xfer_bool(&mut self.extended)?;
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::PowerPlantUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::PowerPlantUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::ExtendRods { extend } => {
                self.extend_rods(world, me, extend);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for PowerPlantUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `PowerPlantUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::PowerPlantUpdate(Arc::new(PowerPlantUpdateModuleData::friend_new_module_data(ini)?)))
}
