// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ProneUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProneUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ProneUpdate`: infantry that goes to ground for a number of frames proportional to the damage it took (`AIUpdateInterface` sends `goProne`, which is
//! [`ModuleCall::ProneGoProne`]). While prone the drawable shows `MODELCONDITION_PRONE` and the object cannot attack (`OBJECT_STATUS_NO_ATTACK`). Never sleeps.
//!
//! # Upstream facts kept
//!
//! - The data is a plain `ModuleData` (`ProneUpdateModuleData : public ModuleData`); `UpdateModuleData` adds no field, so it is the same object.
//! - `m_proneFrames += damageTaken * ratio` is `Int = (Int)((Real)Int + Real)`; `Int damageTaken = damageInfo->out.m_actualDamageDealt` truncates.
//!   A value outside `Int` is undefined ([`Undefined::FloatToIntOutOfRange`]).
//! - Starting and stopping the effects dereference the drawable: an object with none is [`Undefined::NullDrawable`].
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::model_state::MODELCONDITION_PRONE;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_NO_ATTACK};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class ProneUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ProneUpdateModuleData {
    /// The `ModuleData` base (an `UpdateModuleData` adds no field).
    pub update: UpdateModuleData,
    /// `m_damageToFramesRatio`: conversion from damage dealt to number of frames we cower.
    pub damage_to_frames_ratio: f32,
}

impl Default for ProneUpdateModuleData {
    /// `ProneUpdateModuleData()`.
    fn default() -> Self {
        ProneUpdateModuleData {
            update: UpdateModuleData::new(),
            damage_to_frames_ratio: 1.0,
        }
    }
}

/// `ProneUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<ProneUpdateModuleData>; 2] = [
    FieldParse::new(
        "DamageToFramesRatio",
        |ini, d, _| {
            d.damage_to_frames_ratio = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl ProneUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (nothing), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, ProneUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ProneUpdateModuleData> {
        let mut data = ProneUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ProneUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `(Int)x`: undefined outside the range of `Int`.
fn real_to_int(x: f32) -> Result<i32, IniError> {
    if (-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        Ok(x as i32)
    } else {
        Err(IniError::undefined(Undefined::FloatToIntOutOfRange))
    }
}

/// `class ProneUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct ProneUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getProneUpdateModuleData()`.
    data: Arc<ProneUpdateModuleData>,
    /// `m_proneFrames`.
    prone_frames: i32,
}

impl PartialEq for ProneUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.prone_frames == other.prone_frames
    }
}

impl ProneUpdate {
    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<ProneUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(ProneUpdate {
            update,
            data,
            prone_frames: 0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getProneUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<ProneUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.prone_frames as u32]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls: `goProne`.
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

    /// `goProne(damageInfo)`. An error is upstream's undefined behaviour.
    pub fn go_prone(
        &mut self,
        world: &mut World,
        me: ObjectId,
        damage_info: &DamageInfo,
    ) -> Result<(), IniError> {
        let was_prone = self.prone_frames > 0;
        let damage_taken = real_to_int(damage_info.out.actual_damage_dealt)?;
        let ratio = if crate::logic::modules::plant(1161) {
            1.0
        } else {
            self.data.damage_to_frames_ratio
        };
        self.prone_frames = real_to_int(self.prone_frames as f32 + damage_taken as f32 * ratio)?;
        if (!was_prone || crate::logic::modules::plant(1164)) && self.prone_frames > 0 {
            self.start_prone_effects(world, me)?;
        }
        Ok(())
    }

    /// `startProneEffects()`.
    fn start_prone_effects(&self, world: &mut World, me: ObjectId) -> Result<(), IniError> {
        if !world.has_drawable(me) {
            return Err(IniError::undefined(Undefined::NullDrawable));
        }
        world.set_model_condition_state(me, MODELCONDITION_PRONE);
        let mut mask = ObjectStatusMaskType::default();
        mask.set(OBJECT_STATUS_NO_ATTACK, 1);
        world.set_status(me, mask, true);
        Ok(())
    }

    /// `stopProneEffects()`.
    fn stop_prone_effects(&self, world: &mut World, me: ObjectId) -> Result<(), IniError> {
        if !world.has_drawable(me) {
            return Err(IniError::undefined(Undefined::NullDrawable));
        }
        world.clear_model_condition_state(me, MODELCONDITION_PRONE);
        let mut mask = ObjectStatusMaskType::default();
        mask.set(OBJECT_STATUS_NO_ATTACK, 1);
        if !crate::logic::modules::plant(1163) {
            world.clear_status(me, mask);
        }
        Ok(())
    }
}

impl UpdateBehavior for ProneUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if self.prone_frames > 0 {
            self.prone_frames -= 1;
            if self.prone_frames == 0 || crate::logic::modules::plant(1162) {
                if let Err(e) = self.stop_prone_effects(world, me.object) {
                    world.cuts.module_errors.push((me, e));
                }
            }
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_int(&mut self.prone_frames)
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::ProneGoProne(info) => {
                if let Err(e) = self.go_prone(world, me.object, &info) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ProneUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ProneUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ProneUpdate {
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

/// The factory's `NewModuleDataProc` for `ProneUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ProneUpdate(Arc::new(
        ProneUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
