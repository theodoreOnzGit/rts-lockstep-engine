// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FloatUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FloatUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FloatUpdate`: keeps a floating object (a boat, debris) at the water's surface when enabled, and bobs its drawable. Never sleeps.
//!
//! `TheTerrainLogic->isUnderwater(x, y, &waterZ)` is layer 12's: [`World::terrain_is_underwater_water_z`] records a [`CutCall`] and answers from
//! `cuts.terrain_has_water` / `terrain_water`. The drawable's bob (`setInstanceMatrix` with a yaw and a pitch that depend on the frame) is the client's
//! maths on client data: [`ObjectClientEvent::DrawableFloatTilt`] carries the frame.
//!
//! # Upstream facts kept
//!
//! - When there is no water at the position `isUnderwater` leaves `waterZ` unassigned, and `FloatUpdate` copies it into the position anyway: an
//!   uninitialised read ([`Undefined::UninitialisedLocal`]).
//! - The constructor reads `m_enabled` from the module data through a plain cast.
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class FloatUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FloatUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_enabled`.
    pub enabled: bool,
}

/// `FloatUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<FloatUpdateModuleData>; 2] = [
    FieldParse::new("Enabled", |ini, d, _| { d.enabled = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl FloatUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FloatUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FloatUpdateModuleData> {
        let mut data = FloatUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FloatUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class FloatUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct FloatUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getFloatUpdateModuleData()`.
    data: Arc<FloatUpdateModuleData>,
    /// `m_enabled`.
    enabled: bool,
}

impl PartialEq for FloatUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.enabled == other.enabled
    }
}

impl FloatUpdate {
    /// The constructor: the base, the data (`INI_INVALID_DATA` for null data) and `m_enabled` from it.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<FloatUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        let enabled = data.enabled;
        Ok(FloatUpdate { update, data, enabled })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getFloatUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<FloatUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's.
    pub fn state_words(&self) -> Vec<u32> {
        vec![u32::from(self.enabled)]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls: `setEnabled`.
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

    /// The body of `update()`. An error is upstream's undefined behaviour.
    fn run(&mut self, world: &mut World, me: ObjectId) -> Result<(), IniError> {
        if self.enabled {
            let pos = world.objects.find(me).map_or_else(Coord3D::default, |o| o.get_position());

            // `isUnderwater` assigns `waterZ` only where there is water
            let water_z = world.terrain_is_underwater_water_z(pos.x, pos.y).ok_or_else(|| IniError::undefined(Undefined::UninitialisedLocal))?;
            let new_pos = Coord3D { x: pos.x, y: pos.y, z: if crate::logic::modules::plant(1182) { pos.z } else { water_z } };
            world.set_position(me, &new_pos);
        }

        if world.has_drawable(me) && !crate::logic::modules::plant(1183) {
            // the bob: yaw and pitch from the frame, on the drawable's instance matrix (the client's)
            let frame = world.frame();
            world.emit(ObjectClientEvent::DrawableFloatTilt { object_id: me, frame });
        }
        Ok(())
    }
}

impl UpdateBehavior for FloatUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(e) = self.run(world, me.object) {
            world.cuts.module_errors.push((me, e));
        }
        UPDATE_SLEEP_NONE
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        if crate::logic::modules::plant(1184) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.enabled)
    }
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::FloatSetEnabled(enabled) => {
                self.enabled = enabled && !crate::logic::modules::plant(1181);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FloatUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FloatUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for FloatUpdate {
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

/// The factory's `NewModuleDataProc` for `FloatUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FloatUpdate(Arc::new(FloatUpdateModuleData::friend_new_module_data(ini)?)))
}
