// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RadarUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/RadarUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RadarUpdate`: the radar dish of a building, extended when `RadarUpgrade` tells it to.
//!
//! `RadarExtendTime` is `INI::parseDurationReal` (frames, a real). `extendRadar()` ([`ModuleCall::ExtendRadar`], sent by `RadarUpgrade` to the
//! module `findUpdateModule("RadarUpdate")` finds) gives the drawable `MODELCONDITION_RADAR_EXTENDING`, sets `m_extendDoneFrame` to
//! `TheGameLogic->getFrame() + m_radarExtendTime` -- an `UnsignedInt` plus a `Real`, so a single-precision sum converted to `UnsignedInt` -- and
//! the radar active. `update()` does nothing until the extension is due (`frame > m_extendDoneFrame`), then swaps `MODELCONDITION_RADAR_EXTENDING` for
//! `MODELCONDITION_RADAR_UPGRADED`; it never sleeps (`UPDATE_SLEEP_NONE`). A sum outside the range of an `UnsignedInt` (a negative extend time that
//! outweighs the frame) converts undefinedly ([`Undefined::FloatToUnsignedOutOfRange`]).

use std::sync::Arc;

use crate::common::ini::Undefined;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::model_state::{MODELCONDITION_RADAR_EXTENDING, MODELCONDITION_RADAR_UPGRADED};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::UPDATE_SLEEP_NONE;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class RadarUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RadarUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_radarExtendTime` (frames, a real): the time the radar tower takes to build.
    pub radar_extend_time: f32,
}

impl RadarUpdateModuleData {
    /// `RadarUpdateModuleData()`.
    pub fn new() -> Self {
        RadarUpdateModuleData {
            update: UpdateModuleData::new(),
            radar_extend_time: 0.0,
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, RadarUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<RadarUpdateModuleData> {
        let mut data = RadarUpdateModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, RadarUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `RadarUpdateModuleData::dataFieldParse`.
static FIELD_PARSE: [FieldParse<RadarUpdateModuleData>; 2] = [
    FieldParse::new(
        "RadarExtendTime",
        |ini, d, _| {
            d.radar_extend_time = ini.parse_duration_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class RadarUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct RadarUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getRadarUpdateModuleData()`.
    data: Arc<RadarUpdateModuleData>,
    /// `m_extendDoneFrame`: the radar will be done extending on this frame.
    extend_done_frame: u32,
    /// `m_extendComplete`: TRUE when the extension is all done.
    extend_complete: bool,
    /// `m_radarActive`: TRUE when the radar is online and generating radar information.
    radar_active: bool,
}

impl PartialEq for RadarUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.extend_done_frame == other.extend_done_frame
            && self.extend_complete == other.extend_complete
            && self.radar_active == other.radar_active
    }
}

impl RadarUpdate {
    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<RadarUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(RadarUpdate {
            update,
            data,
            extend_done_frame: 0,
            extend_complete: false,
            radar_active: false,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getRadarUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<RadarUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.extend_done_frame,
            u32::from(self.extend_complete),
            u32::from(self.radar_active),
        ]
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
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
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

    /// `extendRadar()`. An error is upstream's undefined float-to-unsigned conversion.
    pub fn extend_radar(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        // set the model condition for radar extension
        if !crate::logic::modules::plant(737) {
            world.set_model_condition_state(me.object, MODELCONDITION_RADAR_EXTENDING);
        }
        // mark the frame that the extension will be done on: `UnsignedInt + Real` is a float sum
        let sum = if crate::logic::modules::plant(733) {
            world
                .frame()
                .wrapping_add(self.data.radar_extend_time as u32) as f32
        } else {
            world.frame() as f32 + self.data.radar_extend_time
        };
        if !(0.0..4_294_967_296.0).contains(&sum) {
            return Err(IniError::undefined(Undefined::FloatToUnsignedOutOfRange));
        }
        self.extend_done_frame = sum as u32;
        // Change this to make the radar active after extension...
        self.radar_active = !crate::logic::modules::plant(735);
        Ok(())
    }
}

impl UpdateBehavior for RadarUpdate {
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        // if no extend frame nothing to do
        if self.extend_done_frame == 0 {
            return UPDATE_SLEEP_NONE;
        }
        // check to see if our extension is already done
        if self.extend_complete {
            return UPDATE_SLEEP_NONE;
        }
        // see if it's time to stop the extension
        if world.frame() > self.extend_done_frame
            || (crate::logic::modules::plant(734) && world.frame() == self.extend_done_frame)
        {
            // mark extend as done
            self.extend_complete = true;
            self.extend_done_frame = 0; // just to be clean
                                        // remove the extending condition and set the extened condition
            world.clear_and_set_model_condition_state(
                me.object,
                MODELCONDITION_RADAR_EXTENDING,
                MODELCONDITION_RADAR_UPGRADED,
            );
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        let _ = version;
        xfer.xfer_unsigned_int(&mut self.extend_done_frame)?;
        xfer.xfer_bool(&mut self.extend_complete)?;
        xfer.xfer_bool(&mut self.radar_active)?;
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::RadarUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::RadarUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::ExtendRadar => {
                if let Err(e) = self.extend_radar(world, me) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for RadarUpdate {
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

/// The factory's `NewModuleDataProc` for `RadarUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RadarUpdate(Arc::new(
        RadarUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
