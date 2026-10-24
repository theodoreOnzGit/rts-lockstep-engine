// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpawnPointProductionExitUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/SpawnPointProductionExitUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpawnPointProductionExitUpdate`: an exit with fixed places, one per `<SpawnPointBoneName>NN` bone of the art (at most 10). The bones
//! are read once, on the first door request, from the drawable (none without one: the door stays shut and the next request tries again)
//! and turned into world positions and angles. A door is free while some place has no live occupier; a new object takes the first free
//! place, at that place's layer height, facing the bone's way, joins the pathfinder's map and is held there (`DISABLED_HELD`). There is no
//! rally point.
//!
//! The exit interface is the module calls `ExitObjectViaDoor`, `ReserveDoorForExit`, `UnreserveDoorForExit`, `ExitObjectByBudding`,
//! `SetRallyPoint`, `GetRallyPoint` and `IsExitBusy`; the bones' transforms are `cuts.contain.pristine_bone_transforms`
//! ([`World::drawable_pristine_bone_transforms`]).
//!
//! # Upstream facts kept
//!
//! - `exitObjectViaDoor` with no free place returns after a `DEBUG_CRASH` (nothing in a release build); it does not revalidate the
//!   occupiers (`reserveDoorForExit` does).
//! - The xfer saves the occupiers (as 40 raw bytes) and nothing else: the bones are re-read only if they were never read; after a load the
//!   module keeps the places it had.
//! - `exitObjectViaDoor` and `initializeBonePositions` use loop variables after their loops (MSVC 6's scoping; `build.rs` declares them
//!   before the loops for the C++ build).
//!
//! `p1w13-sonnet`, batch R45, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, DOOR_1, DOOR_NONE_AVAILABLE, UPDATE_SLEEP_FOREVER};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `MAX_SPAWN_POINTS`: "Size the array that holds the bone positions".
pub const MAX_SPAWN_POINTS: usize = 10;

/// `class SpawnPointProductionExitUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpawnPointProductionExitUpdateModuleData {
    /// The base.
    pub update: UpdateModuleData,
    /// `m_spawnPointBoneNameData`.
    pub spawn_point_bone_name_data: Vec<u8>,
}

/// `SpawnPointProductionExitUpdateModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<SpawnPointProductionExitUpdateModuleData>; 2] = [
    FieldParse::new("SpawnPointBoneName", |ini, d, _| { d.spawn_point_bone_name_data = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl SpawnPointProductionExitUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's field.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SpawnPointProductionExitUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SpawnPointProductionExitUpdateModuleData> {
        let mut data = SpawnPointProductionExitUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SpawnPointProductionExitUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SpawnPointProductionExitUpdate(Arc::new(SpawnPointProductionExitUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class SpawnPointProductionExitUpdate : public UpdateModule, public ExitInterface`.
#[derive(Clone, Debug)]
pub struct SpawnPointProductionExitUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// The data.
    data: Arc<SpawnPointProductionExitUpdateModuleData>,
    /// `m_bonesInitialized`.
    bones_initialized: bool,
    /// `m_spawnPointCount`.
    spawn_point_count: i32,
    /// `m_worldCoordSpawnPoints`.
    world_coord_spawn_points: [Coord3D; MAX_SPAWN_POINTS],
    /// `m_worldAngleSpawnPoints`.
    world_angle_spawn_points: [f32; MAX_SPAWN_POINTS],
    /// `m_spawnPointOccupier`.
    spawn_point_occupier: [ObjectId; MAX_SPAWN_POINTS],
}

impl PartialEq for SpawnPointProductionExitUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl SpawnPointProductionExitUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor's first half (`INI_INVALID_DATA` for null data); the wake is [`UpdateBehavior::construct`]'s.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SpawnPointProductionExitUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(SpawnPointProductionExitUpdate {
            update,
            data,
            bones_initialized: false,
            spawn_point_count: 0,
            world_coord_spawn_points: [Coord3D::default(); MAX_SPAWN_POINTS],
            world_angle_spawn_points: [0.0; MAX_SPAWN_POINTS],
            spawn_point_occupier: [INVALID_ID; MAX_SPAWN_POINTS],
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The state the test compares: the flag, the count, then each place's position, angle and occupier.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w = vec![u32::from(self.bones_initialized), self.spawn_point_count as u32];
        for k in 0..MAX_SPAWN_POINTS {
            let p = &self.world_coord_spawn_points[k];
            w.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), self.world_angle_spawn_points[k].to_bits(), self.spawn_point_occupier[k] as u32]);
        }
        w
    }

    /// `initializeBonePositions()`: "Look up the bone positions and store them in world space coords".
    fn initialize_bone_positions(&mut self, world: &mut World, me: ObjectId) {
        if !world.has_drawable(me) {
            return;
        }
        let transforms = world.drawable_pristine_bone_transforms(me, &self.data.spawn_point_bone_name_data, 1, MAX_SPAWN_POINTS as i32);
        self.spawn_point_count = transforms.len() as i32;
        for (k, bone) in transforms.iter().enumerate() {
            let t = world.objects.find(me).map_or(*bone, |o| o.thing.bone_transform_to_world(bone));
            self.world_coord_spawn_points[k] = Coord3D { x: t.get_x_translation(), y: t.get_y_translation(), z: 0.0 }; // "set at creation time"
            self.world_angle_spawn_points[k] = if crate::logic::modules::plant(2155) { 0.0 } else { t.get_z_rotation() };
        }
        self.bones_initialized = true;
    }

    /// `revalidateOccupiers()`: "Do a lookup on all our ID's and clear the dead ones."
    fn revalidate_occupiers(&mut self, world: &World) {
        for k in 0..self.spawn_point_count as usize {
            let o = self.spawn_point_occupier[k];
            if o != INVALID_ID && world.objects.find(o).is_none() && !crate::logic::modules::plant(2156) {
                self.spawn_point_occupier[k] = INVALID_ID;
            }
        }
    }

    /// `reserveDoorForExit(objType, specificObject)`.
    fn reserve_door_for_exit(&mut self, world: &mut World, me: ObjectId) -> i32 {
        if !self.bones_initialized {
            self.initialize_bone_positions(world, me);
        }
        if !self.bones_initialized {
            return DOOR_NONE_AVAILABLE; // "Init failure"
        }
        self.revalidate_occupiers(world);
        if self.spawn_point_occupier[..self.spawn_point_count as usize].contains(&INVALID_ID) {
            DOOR_1
        } else {
            DOOR_NONE_AVAILABLE
        }
    }

    /// `exitObjectViaDoor(newObj, exitDoor)`.
    fn exit_object_via_door(&mut self, world: &mut World, me: ObjectId, new_obj: ObjectId) {
        if !self.bones_initialized {
            self.initialize_bone_positions(world, me);
        }
        let Some(layer) = world.objects.find(me).map(|o| o.layer) else { return };
        let count = self.spawn_point_count as usize;
        let Some(k) = self.spawn_point_occupier[..count].iter().position(|&o| o == INVALID_ID) else {
            // "A SpawnPoint exit thought it had room but then failed" (a DEBUG_CRASH)
            return;
        };
        let mut create_point = self.world_coord_spawn_points[k];
        create_point.z = if crate::logic::modules::plant(2159) {
            world.terrain_get_ground_height(create_point.x, create_point.y)
        } else {
            world.terrain_get_layer_height(create_point.x, create_point.y, layer)
        };
        let create_angle = self.world_angle_spawn_points[k];
        self.spawn_point_occupier[k] = new_obj;
        world.contain_set_position(new_obj, &create_point);
        world.contain_set_orientation(new_obj, create_angle);
        world.contain_set_layer(new_obj, layer);
        // "@todo This really should be automatically wrapped up in an actication sequence for objects in general"
        world.contain_pathfind_add_object(new_obj);
        if !crate::logic::modules::plant(2157) {
            world.contain_set_disabled(new_obj, DISABLED_HELD);
        }
    }
}

impl UpdateBehavior for SpawnPointProductionExitUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, _world: &mut World, _me: ModuleHandle) -> UpdateSleepTime {
        UPDATE_SLEEP_FOREVER
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// `xfer`'s fields after the base: the occupiers, as raw bytes.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        let mut raw = [0u8; 4 * MAX_SPAWN_POINTS];
        for (k, id) in self.spawn_point_occupier.iter().enumerate() {
            raw[4 * k..4 * k + 4].copy_from_slice(&(*id as u32).to_le_bytes());
        }
        xfer.xfer_user(&mut raw)?;
        for (k, id) in self.spawn_point_occupier.iter_mut().enumerate() {
            if k == 0 && crate::logic::modules::plant(2158) {
                continue;
            }
            *id = u32::from_le_bytes(raw[4 * k..4 * k + 4].try_into().expect("four bytes")) as ObjectId;
        }
        Ok(())
    }
    /// The exit interface.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::HasUpdateExitInterface => ModuleReply::Bool(true),
            ModuleCall::IsExitBusy => ModuleReply::Bool(false),
            ModuleCall::ReserveDoorForExit { .. } => ModuleReply::Int(self.reserve_door_for_exit(world, me.object)),
            ModuleCall::ExitObjectViaDoor { obj, .. } => {
                self.exit_object_via_door(world, me.object, obj);
                ModuleReply::Done
            }
            ModuleCall::UnreserveDoorForExit(_) | ModuleCall::ExitObjectByBudding { .. } | ModuleCall::SetRallyPoint(_) => ModuleReply::Done,
            ModuleCall::GetRallyPoint => ModuleReply::Unhandled,
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpawnPointProductionExitUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpawnPointProductionExitUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SpawnPointProductionExitUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the occupiers.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_places_start_empty() {
        let m = SpawnPointProductionExitUpdate::unconstructed(1, Some(Arc::new(SpawnPointProductionExitUpdateModuleData::default()))).unwrap();
        let w = m.state_words();
        assert_eq!((w[0], w[1], w[6]), (0, 0, INVALID_ID as u32));
    }
}
