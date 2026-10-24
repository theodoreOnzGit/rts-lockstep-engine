// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RadiusDecalUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/RadiusDecalUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RadiusDecalUpdate`: holds the decal an attack draws on the ground (the object creation list's attack nugget creates it on the object it
//! makes, at the target, and asks for it to go when the object stops attacking). Asleep while there is no decal; awake every frame while there
//! is one, to throb it, until `killWhenNoLongerAttacking` is set and the object is no longer attacking.
//!
//! The module data has no fields (upstream's two are commented out). The decal is [`RadiusDecal`], decal 0 of the object's
//! [`ObjectClientEvent::GridDecal`](crate::logic::world::object_events::ObjectClientEvent::GridDecal) events. The calls are
//! [`ModuleCall::RadiusDecalCreate`], [`ModuleCall::RadiusDecalKill`] and [`ModuleCall::RadiusDecalKillWhenNoLongerAttacking`].

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{UpdateModule, UpdateModuleData, SleepyUpdatePhase, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::common::object_status_types::OBJECT_STATUS_IS_ATTACKING;
use crate::logic::radius_decal::{RadiusDecal, RadiusDecalTemplate};
use crate::common::base_type::Coord3D;

/// `class RadiusDecalUpdateModuleData : public UpdateModuleData`: no fields of its own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RadiusDecalUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
}

/// `RadiusDecalUpdateModuleData::buildFieldParse`'s own table: empty.
static FIELD_PARSE: [FieldParse<RadiusDecalUpdateModuleData>; 1] = [FieldParse::end()];

impl RadiusDecalUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, RadiusDecalUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<RadiusDecalUpdateModuleData> {
        let mut data = RadiusDecalUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, RadiusDecalUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class RadiusDecalUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct RadiusDecalUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getRadiusDecalUpdateModuleData()`.
    data: Arc<RadiusDecalUpdateModuleData>,
    /// `m_deliveryDecal`.
    delivery_decal: RadiusDecal,
    /// `m_killWhenNoLongerAttacking`.
    kill_when_no_longer_attacking: bool,
}

impl PartialEq for RadiusDecalUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same decal and flag.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.delivery_decal == other.delivery_decal
            && self.kill_when_no_longer_attacking == other.kill_when_no_longer_attacking
    }
}

impl RadiusDecalUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and an empty decal, the kill flag false.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<RadiusDecalUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(RadiusDecalUpdate { update, data, delivery_decal: RadiusDecal::default(), kill_when_no_longer_attacking: false })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getRadiusDecalUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<RadiusDecalUpdateModuleData> {
        &self.data
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

    /// `xfer` with the scheduling half the slot holds: the version, that half, the fields.
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

    /// The state the test compares: the decal is empty, the kill flag.
    pub fn state_words(&self) -> [u32; 2] {
        [u32::from(self.delivery_decal.is_empty()), u32::from(self.kill_when_no_longer_attacking)]
    }

    /// `createRadiusDecal(tmpl, radius, pos)`: the old decal cleared, the new one made for the controlling player, and awake while it is not
    /// empty.
    pub fn create_radius_decal(&mut self, world: &mut World, me: ObjectId, tmpl: &RadiusDecalTemplate, radius: f32, pos: Coord3D) {
        self.delivery_decal.clear(world, me, 0);
        let player = world.get_controlling_player(me);
        tmpl.create_radius_decal(world, (me, 0), pos, radius, player, &mut self.delivery_decal);
        let empty = self.delivery_decal.is_empty() != crate::logic::modules::plant(1872);
        update_behavior::set_wake_frame(world, if empty { UPDATE_SLEEP_FOREVER } else { UPDATE_SLEEP_NONE });
    }

    /// `killRadiusDecal()`: the decal cleared, asleep.
    pub fn kill_radius_decal(&mut self, world: &mut World, me: ObjectId) {
        self.delivery_decal.clear(world, me, 0);
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
}

impl UpdateBehavior for RadiusDecalUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let attacking = world.objects.find(me.object).is_some_and(|o| o.test_status(OBJECT_STATUS_IS_ATTACKING));
        if self.kill_when_no_longer_attacking && !attacking && !crate::logic::modules::plant(1873) {
            self.delivery_decal.clear(world, me.object, 0);
            return UPDATE_SLEEP_FOREVER;
        }
        self.delivery_decal.update(world, me.object, 0);
        UPDATE_SLEEP_NONE
    }
    /// The constructor: the decal cleared, asleep for ever.
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.delivery_decal.clear(world, me.object, 0);
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// The fields after the base: the decal (nothing saved; a load clears it), then the kill flag.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        self.delivery_decal.xfer_radius_decal(xfer.get_xfer_mode() == crate::common::xfer::XferMode::Load);
        if crate::logic::modules::plant(1874) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.kill_when_no_longer_attacking)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::RadiusDecalUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::RadiusDecalUpdate(m) => Some(m),
            _ => None,
        }
    }
    /// `createRadiusDecal`, `killRadiusDecal` and `killWhenNoLongerAttacking`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::RadiusDecalCreate { template, radius, pos } => {
                self.create_radius_decal(world, me.object, &template, radius, pos);
                ModuleReply::Done
            }
            ModuleCall::RadiusDecalKill => {
                self.kill_radius_decal(world, me.object);
                ModuleReply::Done
            }
            ModuleCall::RadiusDecalKillWhenNoLongerAttacking(v) => {
                self.kill_when_no_longer_attacking = v;
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for RadiusDecalUpdate {
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

/// The factory's `NewModuleDataProc` for `RadiusDecalUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RadiusDecalUpdate(Arc::new(RadiusDecalUpdateModuleData::friend_new_module_data(ini)?)))
}
