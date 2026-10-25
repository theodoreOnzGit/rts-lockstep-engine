// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RailedTransportDockUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/DockUpdate/RailedTransportDockUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, August 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RailedTransportDockUpdate`: the dock of a train (`RailedTransportContain`). A docker that comes within `ToleranceDistance` of the dock is
//! taken over: deselected, made unselectable and held, turned to face the dock, and pulled in over `PullInsideDuration` (moving), until it
//! is within 6 of the dock's centre, when its dock is cancelled, it idles and it is put inside. Unloading takes the first rider out at the
//! dock's position and angle, held, and pushes it to the dock's exit position on the ground over `PushOutsideDuration`; within 3 of it,
//! the rider idles, is released and made selectable again, walks to the dock's `DOCKWAITING07` bone, and the next one comes out
//! (`unloadAll`; `unloadSingleObject` unloads one, whichever object it is given).
//!
//! # Upstream facts kept
//!
//! - `update` runs `DockUpdate::update`, ignores its answer, pulls and pushes, and never sleeps.
//! - The durations divide the distances as `Real`s: a duration of 0 gives an infinite (or not-a-number) step, as upstream.
//! - `unloadSingleObject` ignores its object and unloads the first rider.
//! - The pushed rider is unloaded when `m_unloadingObjectID` is not 0; the next is started at once when it has gone.
//! - The bone is looked up as `DOCKWAITING07` (the art's names are case-blind).
//! - The xfer is version 1: the version, `DockUpdate`'s (its own version, the update base, its fields), then the five fields.
//!
//! # Where upstream is undefined
//!
//! Unloading with no contain module reads through `NULL` (`asOpenContain` of nothing: `Undefined::NullContainModule`); a dock with a drawable
//! but no `DOCKWAITING07` bone leaves the walk's position unset (`Undefined::UninitialisedLocal`). Both are recorded in
//! `world.cuts.module_errors`.
//!
//! `p1w13-sonnet`, batch R61, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_common::PLAYERMASK_ALL;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::MODELCONDITION_MOVING;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_UNSELECTABLE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::dock_update::{new_dock_update, DockUpdate, DockUpdateModuleData};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::partition::DistanceCalculationType;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `UNLOAD_ALL`.
pub const UNLOAD_ALL: i32 = -1;

/// `class RailedTransportDockUpdateModuleData : public DockUpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct RailedTransportDockUpdateModuleData {
    /// The base.
    pub dock: DockUpdateModuleData,
    /// `m_pullInsideDurationInFrames`, `m_pushOutsideDurationInFrames`.
    pub pull_inside_duration_in_frames: u32,
    pub push_outside_duration_in_frames: u32,
    /// `m_toleranceDistance`: "the maximum distance the docking unit must be within in order to cheat and dock".
    pub tolerance_distance: f32,
}

impl Default for RailedTransportDockUpdateModuleData {
    /// `RailedTransportDockUpdateModuleData()`.
    fn default() -> Self {
        RailedTransportDockUpdateModuleData {
            dock: DockUpdateModuleData::default(),
            pull_inside_duration_in_frames: 0,
            push_outside_duration_in_frames: 0,
            tolerance_distance: 50.0,
        }
    }
}

/// `DockUpdateModuleData::buildFieldParse`'s table.
static DOCK_FIELD_PARSE: [FieldParse<RailedTransportDockUpdateModuleData>; 3] = crate::dock_update_field_parse!(RailedTransportDockUpdateModuleData, dock);

/// `RailedTransportDockUpdateModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<RailedTransportDockUpdateModuleData>; 4] = [
    FieldParse::new("PullInsideDuration", |i, d, _| { d.pull_inside_duration_in_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("PushOutsideDuration", |i, d, _| { d.push_outside_duration_in_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ToleranceDistance", |i, d, _| { d.tolerance_distance = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl RailedTransportDockUpdateModuleData {
    /// `buildFieldParse(p)`: `DockUpdateModuleData`'s (the update base, then its fields), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, RailedTransportDockUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&DOCK_FIELD_PARSE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<RailedTransportDockUpdateModuleData> {
        let mut data = RailedTransportDockUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, RailedTransportDockUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RailedTransportDockUpdate(Arc::new(RailedTransportDockUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class RailedTransportDockUpdate : public DockUpdate, public RailedTransportDockUpdateInterface`.
#[derive(Clone, Debug)]
pub struct RailedTransportDockUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// The data.
    data: Arc<RailedTransportDockUpdateModuleData>,
    /// The `DockUpdate` base's state.
    dock: DockUpdate,
    /// `m_dockingObjectID`: "object docking with us".
    docking_object_id: ObjectId,
    /// `m_pullInsideDistancePerFrame`.
    pull_inside_distance_per_frame: f32,
    /// `m_unloadingObjectID`: "object that is currently unloading".
    unloading_object_id: ObjectId,
    /// `m_pushOutsideDistancePerFrame`.
    push_outside_distance_per_frame: f32,
    /// `m_unloadCount`: "count used to govern unloading 1 or all objects".
    unload_count: i32,
}

impl PartialEq for RailedTransportDockUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

/// The status mask of one bit.
fn status_mask(bit: i32) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::default();
    m.set(bit, 1);
    m
}

impl RailedTransportDockUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor, which touches no world (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<RailedTransportDockUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.dock.update.clone())), SleepyUpdatePhase::Normal)?;
        let dock = new_dock_update(Arc::new(data.dock.clone()))?;
        Ok(RailedTransportDockUpdate {
            update,
            data,
            dock,
            docking_object_id: INVALID_ID,
            pull_inside_distance_per_frame: 0.0,
            unloading_object_id: INVALID_ID,
            push_outside_distance_per_frame: 0.0,
            unload_count: UNLOAD_ALL,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The state the test compares: the dock's, then this class's five fields.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = self.dock.state_words();
        w.push(self.docking_object_id as u32);
        w.push(self.pull_inside_distance_per_frame.to_bits());
        w.push(self.unloading_object_id as u32);
        w.push(self.push_outside_distance_per_frame.to_bits());
        w.push(self.unload_count as u32);
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half (nothing).
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: this class's version, then `DockUpdate::xfer` (its version, the base, its fields),
    /// then this class's fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)?;
        let mut dock_version: XferVersion = 1;
        xfer.xfer_version(&mut dock_version, 1)?;
        update.xfer(xfer)?;
        self.dock.xfer_fields(xfer)?;
        xfer.xfer_object_id(&mut self.docking_object_id)?;
        xfer.xfer_real(&mut self.pull_inside_distance_per_frame)?;
        xfer.xfer_object_id(&mut self.unloading_object_id)?;
        xfer.xfer_real(&mut self.push_outside_distance_per_frame)?;
        if crate::logic::modules::plant(2317) {
            return Ok(());
        }
        xfer.xfer_int(&mut self.unload_count)
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    fn position(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map(|o| o.get_position()).unwrap_or_default()
    }

    /// `action(docker, drone)`: "Return TRUE to continue the docking process" (always, once there is a docker).
    fn action(&mut self, world: &mut World, us: ObjectId, docker: ObjectId) -> bool {
        if world.objects.find(docker).is_none() {
            return false;
        }
        if self.docking_object_id != docker {
            let docker_pos = Self::position(world, docker);
            let dock_pos = Self::position(world, us);
            let v = Coord3D { x: dock_pos.x - docker_pos.x, y: dock_pos.y - docker_pos.y, z: dock_pos.z - docker_pos.z };
            let mag = v.length();
            let tolerance = if crate::logic::modules::plant(2311) { self.data.tolerance_distance * 2.0 } else { self.data.tolerance_distance };
            // "Are we close enough to even be able to get sucked in?"
            if mag <= tolerance {
                self.docking_object_id = docker;
                world.game_logic_deselect_object(docker, PLAYERMASK_ALL, true);
                world.body_set_status(docker, status_mask(OBJECT_STATUS_UNSELECTABLE), true);
                world.body_set_disabled(docker, DISABLED_HELD);
                self.pull_inside_distance_per_frame = mag / self.data.pull_inside_duration_in_frames as f32;
                let angle_vector = Coord2D { x: dock_pos.x - docker_pos.x, y: dock_pos.y - docker_pos.y };
                world.contain_set_orientation(docker, angle_vector.to_angle());
            }
        }
        true
    }

    /// `isClearToEnter(docker)`: the base's, and the container (if any) must take the docker.
    fn is_clear_to_enter(&self, world: &mut World, us: ObjectId, docker: ObjectId) -> Result<bool, Undefined> {
        if !self.dock.is_clear_to_enter(world, docker)? {
            return Ok(false);
        }
        if let Some(c) = world.get_contain(us) {
            if !world.module_call(us, c, ModuleCall::IsValidContainerFor { obj: docker, check_capacity: true }).as_bool() && !crate::logic::modules::plant(2312) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `doPullInDocking()`.
    fn do_pull_in_docking(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        if self.docking_object_id == INVALID_ID {
            return Ok(());
        }
        let us = me.object;
        let docker = self.docking_object_id;
        if world.objects.find(docker).is_none() {
            self.docking_object_id = INVALID_ID;
            return Ok(());
        }
        let docker_pos = Self::position(world, docker);
        let dock_pos = Self::position(world, us);
        let mut v = Coord3D { x: dock_pos.x - docker_pos.x, y: dock_pos.y - docker_pos.y, z: dock_pos.z - docker_pos.z };
        v.normalize();
        v.x *= self.pull_inside_distance_per_frame;
        v.y *= self.pull_inside_distance_per_frame;
        v.x += docker_pos.x;
        v.y += docker_pos.y;
        // "keep Z height the same and just scoot along the ground"
        v.z = docker_pos.z;
        world.set_position(docker, &v);
        world.set_model_condition_state(docker, MODELCONDITION_MOVING);
        let dist_sq = world.partition_distance_squared(docker, us, DistanceCalculationType::FromCenter2D);
        let close_enough_distance: f32 = if crate::logic::modules::plant(2313) { 3.0 } else { 6.0 };
        if dist_sq <= close_enough_distance * close_enough_distance {
            world.clear_model_condition_state(docker, MODELCONDITION_MOVING);
            self.dock.cancel_dock(world, me, docker)?;
            if let Some(ai) = world.objects.find(docker).and_then(|o| o.ai) {
                world.module_call(docker, ai, ModuleCall::AiIdle);
            }
            if let Some(c) = world.get_contain(us) {
                world.module_call(us, c, ModuleCall::AddToContain(docker));
            }
            self.docking_object_id = INVALID_ID;
        }
        Ok(())
    }

    /// `doPushOutDocking()`.
    fn do_push_out_docking(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        if self.unloading_object_id == 0 {
            return Ok(());
        }
        let us = me.object;
        let unloader = self.unloading_object_id;
        if world.objects.find(unloader).is_none() {
            return self.unload_next(world, me);
        }
        let unloader_pos = Self::position(world, unloader);
        let mut dest = self.dock.get_path_position(world, me, unloader, true, 2)?.unwrap_or_default();
        dest.z = world.terrain_get_ground_height(dest.x, dest.y);
        let mut v = Coord3D { x: dest.x - unloader_pos.x, y: dest.y - unloader_pos.y, z: dest.z - unloader_pos.z };
        v.normalize();
        v.x *= self.push_outside_distance_per_frame;
        v.y *= self.push_outside_distance_per_frame;
        v.x += unloader_pos.x;
        v.y += unloader_pos.y;
        v.z = dest.z;
        world.set_position(unloader, &v);
        world.set_model_condition_state(unloader, MODELCONDITION_MOVING);
        let sqr = |x: f32| x * x;
        let dist_sq = sqr(dest.x - v.x) + sqr(dest.y - v.y) + sqr(dest.z - v.z);
        let close_enough_distance: f32 = 3.0;
        if dist_sq <= sqr(close_enough_distance) {
            world.clear_model_condition_state(unloader, MODELCONDITION_MOVING);
            let ai = world.objects.find(unloader).and_then(|o| o.ai);
            if let Some(ai) = ai {
                world.module_call(unloader, ai, ModuleCall::AiIdle);
            }
            if !crate::logic::modules::plant(2314) {
                world.body_clear_disabled(unloader, DISABLED_HELD);
            }
            world.body_set_status(unloader, status_mask(OBJECT_STATUS_UNSELECTABLE), false);
            // "tell the unloader to move to one of the dock positions and out of the way"
            if let Some(ai) = ai {
                if world.has_drawable(us) {
                    let bone = world.drawable_pristine_bones(us, b"DOCKWAITING07", 0, 1).first().copied().ok_or(Undefined::UninitialisedLocal)?;
                    let final_pos = world.contain_convert_bone_pos_to_world_pos(us, &bone);
                    world.module_call(unloader, ai, ModuleCall::AiMoveToPosition { pos: final_pos, command_source: CMD_FROM_AI });
                }
            }
            self.unload_next(world, me)?;
        }
        Ok(())
    }

    /// `unloadNext()`: "start the next object contained by us as unloading and coming out".
    fn unload_next(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let us = me.object;
        self.unloading_object_id = INVALID_ID;
        if self.unload_count == 0 {
            return Ok(());
        }
        // "better be an open container"
        let contain = world.get_contain(us).ok_or(Undefined::NullContainModule)?;
        let first = match world.module_call(us, contain, ModuleCall::GetContainedItems) {
            ModuleReply::Ids(ids) => ids.first().copied(),
            _ => None,
        };
        let Some(unloader) = first else { return Ok(()) };
        world.module_call(us, contain, ModuleCall::RemoveFromContain(unloader));
        let us_pos = Self::position(world, us);
        world.set_position(unloader, &us_pos);
        let angle = world.objects.find(us).map_or(0.0, |o| o.thing.orientation());
        world.contain_set_orientation(unloader, angle);
        world.body_set_disabled(unloader, DISABLED_HELD);
        // "that is where we came in at the DOCKEND point"
        let dock_position = self.dock.get_path_position(world, me, unloader, true, 2)?.unwrap_or_default();
        let unloader_pos = Self::position(world, unloader);
        let v = Coord3D { x: dock_position.x - unloader_pos.x, y: dock_position.y - unloader_pos.y, z: dock_position.z - unloader_pos.z };
        let mag = v.length();
        self.push_outside_distance_per_frame = mag / self.data.push_outside_duration_in_frames as f32;
        self.unloading_object_id = unloader;
        if self.unload_count != UNLOAD_ALL && !crate::logic::modules::plant(2315) {
            self.unload_count -= 1;
        }
        Ok(())
    }

    /// `unloadAll()`: "if we're already unloading, ignore this command and just allow us to finish".
    fn unload_all(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        if self.unloading_object_id != INVALID_ID && !crate::logic::modules::plant(2316) {
            return Ok(());
        }
        self.unload_count = UNLOAD_ALL;
        self.unload_next(world, me)
    }

    /// `unloadSingleObject(obj)`: one, the first (the object is not used).
    fn unload_single_object(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        self.unload_count = 1;
        self.unload_next(world, me)
    }

    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }
}

impl UpdateBehavior for RailedTransportDockUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// `update()`: the dock's (its answer ignored), the pull and the push; never asleep.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        self.dock.update(world, me);
        if let Err(u) = self.do_pull_in_docking(world, me) {
            Self::undefined(world, me, u);
        }
        if let Err(u) = self.do_push_out_docking(world, me) {
            Self::undefined(world, me, u);
        }
        UPDATE_SLEEP_NONE
    }
    /// The calls of `DockUpdateInterface` (with this class's action and entry test) and of `RailedTransportDockUpdateInterface`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let done = |r: Result<(), Undefined>, world: &mut World| {
            if let Err(u) = r {
                Self::undefined(world, me, u);
            }
            ModuleReply::Done
        };
        match call {
            ModuleCall::DockAction { docker, .. } => return ModuleReply::Bool(self.action(world, me.object, docker)),
            ModuleCall::DockIsClearToEnter { docker } => {
                return match self.is_clear_to_enter(world, me.object, docker) {
                    Ok(b) => ModuleReply::Bool(b),
                    Err(u) => {
                        Self::undefined(world, me, u);
                        ModuleReply::Bool(false)
                    }
                }
            }
            ModuleCall::RailedTransportDockIsLoadingOrUnloading => {
                return ModuleReply::Bool(self.unloading_object_id != INVALID_ID || self.docking_object_id != INVALID_ID);
            }
            ModuleCall::RailedTransportDockUnloadAll => {
                let r = self.unload_all(world, me);
                return done(r, world);
            }
            ModuleCall::RailedTransportDockUnloadSingleObject(_) => {
                let r = self.unload_single_object(world, me);
                return done(r, world);
            }
            _ => {}
        }
        match self.dock.dock_call(world, me, &call, false) {
            Some(Ok(reply)) => reply,
            Some(Err(u)) => {
                Self::undefined(world, me, u);
                ModuleReply::Done
            }
            None => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::RailedTransportDockUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::RailedTransportDockUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for RailedTransportDockUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: see [`RailedTransportDockUpdate::xfer_with`].
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = self.xfer_with(&mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = RailedTransportDockUpdateModuleData::default();
        assert_eq!((d.pull_inside_duration_in_frames, d.push_outside_duration_in_frames, d.tolerance_distance), (0, 0, 50.0));
        let m = RailedTransportDockUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        let w = m.state_words();
        assert_eq!(w[w.len() - 5..], [0, 0, 0, 0, u32::MAX], "nothing docking or unloading, unload all");
    }
}
