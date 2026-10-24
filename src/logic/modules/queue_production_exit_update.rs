// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/QueueProductionExitUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionExitUpdate/QueueProductionExitUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `QueueProductionExitUpdate`: a production exit that spaces its units out. As `DefaultProductionExitUpdate`, except: the create point is put on
//! the ground unless `AllowAirborneCreation` (a point off the ground is "in the air"); a unit made in the air takes the producer's velocity as a
//! push and a pitch rate from its centre of mass; the natural rally point is snapped by the pathfinder; with no rally point the unit is sent to the
//! natural rally point twice; after an exit the door stays shut for `ExitDelay` frames, except for the `InitialBurst` first units; budding puts the
//! unit at its host (or at the producer) and sends its AI to where it stands.
//!
//! # Upstream facts kept
//!
//! - `m_creationClearDistance` is saved and never set.
//! - The update counts the delay down every frame and never sleeps.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, DOOR_1,
    DOOR_NONE_AVAILABLE, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::production_exit::{
    exit_position, natural_rally_point, transform_point, RallyPoint,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `STARTING_PITCH_COEFF`.
const STARTING_PITCH_COEFF: f32 = 0.04;

/// `class QueueProductionExitUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueueProductionExitUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_unitCreatePoint`.
    pub unit_create_point: Coord3D,
    /// `m_naturalRallyPoint`.
    pub natural_rally_point: Coord3D,
    /// `m_exitDelayData`.
    pub exit_delay_data: u32,
    /// `m_allowAirborneCreationData`.
    pub allow_airborne_creation_data: bool,
    /// `m_initialBurst`.
    pub initial_burst: u32,
}

/// `QueueProductionExitUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<QueueProductionExitUpdateModuleData>; 6] = [
    FieldParse::new(
        "UnitCreatePoint",
        |ini, d, _| {
            d.unit_create_point = ini.parse_coord3d()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "NaturalRallyPoint",
        |ini, d, _| {
            d.natural_rally_point = ini.parse_coord3d()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ExitDelay",
        |ini, d, _| {
            d.exit_delay_data = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "AllowAirborneCreation",
        |ini, d, _| {
            d.allow_airborne_creation_data = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "InitialBurst",
        |ini, d, _| {
            d.initial_burst = ini.parse_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl QueueProductionExitUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, QueueProductionExitUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<QueueProductionExitUpdateModuleData> {
        let mut data = QueueProductionExitUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                QueueProductionExitUpdateModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class QueueProductionExitUpdate : public UpdateModule, public ExitInterface`.
#[derive(Clone, Debug)]
pub struct QueueProductionExitUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getQueueProductionExitUpdateModuleData()`.
    data: Arc<QueueProductionExitUpdateModuleData>,
    /// `m_currentDelay`: "When this hits zero, I can exit again".
    current_delay: u32,
    /// `m_rallyPoint`, `m_rallyPointExists`.
    rally: RallyPoint,
    /// `m_creationClearDistance` (never set).
    creation_clear_distance: f32,
    /// `m_currentBurstCount`: "how many times must I still override the delay timer".
    current_burst_count: u32,
}

impl PartialEq for QueueProductionExitUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.state_words() == other.state_words()
    }
}

impl QueueProductionExitUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data), no delay or rally point,
    /// the burst count `InitialBurst`.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<QueueProductionExitUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        let current_burst_count = data.initial_burst;
        Ok(QueueProductionExitUpdate {
            update,
            data,
            current_delay: 0,
            rally: RallyPoint::default(),
            creation_clear_distance: 0.0,
            current_burst_count,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<QueueProductionExitUpdateModuleData> {
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

    /// The module's state as words, for the tests that compare it with upstream's: the delay, the rally point and whether it exists, the clear
    /// distance, the burst count.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.current_delay,
            self.rally.point.x.to_bits(),
            self.rally.point.y.to_bits(),
            self.rally.point.z.to_bits(),
            u32::from(self.rally.exists),
            self.creation_clear_distance.to_bits(),
            self.current_burst_count,
        ]
    }

    /// `isFreeToExit()`.
    fn is_free_to_exit(&self) -> bool {
        let still_bursting = self.current_burst_count > 0;
        let still_delaying = self.current_delay != 0;
        if still_bursting && !crate::logic::modules::plant(1843) {
            return true;
        }
        !still_delaying
    }

    /// After an exit: the delay starts again, and one fewer unit to burst ("fewer and fewer units to burst").
    fn after_exit(&mut self) {
        self.current_delay = self.data.exit_delay_data;
        if self.current_burst_count != 0 {
            self.current_burst_count -= 1;
        }
    }

    /// `exitObjectViaDoor(newObj, exitDoor)`.
    fn exit_object_via_door(&mut self, world: &mut World, creation: ObjectId, new_obj: ObjectId) {
        let Some(exit_angle) = world.objects.find(creation).map(|o| o.thing.orientation()) else {
            return;
        };
        let d = self.data.clone();
        let mut loc = transform_point(world, creation, &d.unit_create_point);
        // "make sure the point is on the terrain"
        let creation_in_air = loc.z != world.terrain_get_ground_height(loc.x, loc.y);
        if creation_in_air && !d.allow_airborne_creation_data {
            loc.z = world.terrain_get_ground_height(loc.x, loc.y);
        }
        world.contain_set_position(new_obj, &loc);
        world.contain_set_orientation(new_obj, exit_angle);
        let new_physics = world.objects.find(new_obj).and_then(|o| o.physics);
        let my_physics = world.objects.find(creation).and_then(|o| o.physics);
        if let (Some(mine), true, Some(theirs)) = (my_physics, creation_in_air, new_physics) {
            let mut starting_force =
                match world.module_call(creation, mine, ModuleCall::PhysicsGetVelocity) {
                    ModuleReply::Coord(c) => c,
                    _ => Coord3D::default(),
                };
            let mass = world
                .module_call(new_obj, theirs, ModuleCall::PhysicsGetMass)
                .as_real(0.0);
            starting_force.x *= mass;
            starting_force.y *= mass;
            starting_force.z *= mass;
            world.module_call(
                new_obj,
                theirs,
                ModuleCall::PhysicsApplyMotiveForce(starting_force),
            );
            let pitch_rate = world
                .module_call(new_obj, theirs, ModuleCall::PhysicsGetCenterOfMassOffset)
                .as_real(0.0)
                * STARTING_PITCH_COEFF;
            world.module_call(new_obj, theirs, ModuleCall::PhysicsSetPitchRate(pitch_rate));
        }
        // "@todo This really should be automatically wrapped up in an actication sequence for objects in general"
        world.contain_pathfind_add_object(new_obj);
        let mut tmp = natural_rally_point(world, creation, &d.natural_rally_point, true);
        world.contain_pathfind_snap_position(new_obj, &mut tmp);
        let mut exit_path = vec![tmp];
        let ai = world.objects.find(new_obj).and_then(|o| o.ai);
        if self.rally.exists {
            tmp = self.rally.point;
            if let Some(ai) = ai {
                if world
                    .module_call(new_obj, ai, ModuleCall::AiIsDoingGroundMovement)
                    .as_bool()
                    && world.contain_pathfind_adjust_destination(new_obj, &mut tmp)
                {
                    exit_path.push(tmp);
                }
            }
        } else if !crate::logic::modules::plant(1844) {
            exit_path.push(tmp);
        }
        if let Some(ai) = ai {
            world.module_call(
                new_obj,
                ai,
                ModuleCall::AiFollowExitProductionPath {
                    path: exit_path,
                    ignore_object: creation,
                    command_source: CMD_FROM_AI,
                },
            );
        }
        self.after_exit();
    }

    /// `exitObjectByBudding(newObj, budHost)`.
    fn exit_object_by_budding(
        &mut self,
        world: &mut World,
        me: ObjectId,
        new_obj: ObjectId,
        bud_host: ObjectId,
    ) {
        if let Some((pos, angle, layer)) = world
            .objects
            .find(bud_host)
            .map(|o| (o.get_position(), o.thing.orientation(), o.layer))
        {
            // "object has provided a host for us"
            world.contain_set_position(new_obj, &pos);
            world.contain_set_orientation(new_obj, angle);
            world.contain_set_layer(new_obj, layer);
        } else {
            // "we need to just copy the host object"
            let (pos, angle) = world
                .objects
                .find(me)
                .map(|o| (o.get_position(), o.thing.orientation()))
                .unwrap_or_default();
            world.contain_set_position(new_obj, &pos);
            world.contain_set_orientation(new_obj, angle);
        }
        if let Some(ai) = world.objects.find(new_obj).and_then(|o| o.ai) {
            // "IOW, I cant stay here, if somebody else is here"
            let pos = world
                .objects
                .find(new_obj)
                .map(|o| o.get_position())
                .unwrap_or_default();
            world.module_call(
                new_obj,
                ai,
                ModuleCall::AiMoveToPosition {
                    pos,
                    command_source: CMD_FROM_AI,
                },
            );
        }
        self.after_exit();
    }
}

impl UpdateBehavior for QueueProductionExitUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, _world: &mut World, _me: ModuleHandle) -> UpdateSleepTime {
        if self.is_free_to_exit() {
            self.current_delay = 0;
            return UPDATE_SLEEP_NONE;
        }
        self.current_delay = self
            .current_delay
            .wrapping_sub(1 + u32::from(crate::logic::modules::plant(1845)));
        UPDATE_SLEEP_NONE
    }
    /// The fields after the base: the delay, the rally point and whether it exists, the clear distance, the burst count.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.current_delay)?;
        xfer.xfer_coord3d(&mut self.rally.point)?;
        xfer.xfer_bool(&mut self.rally.exists)?;
        xfer.xfer_real(&mut self.creation_clear_distance)?;
        xfer.xfer_unsigned_int(&mut self.current_burst_count)
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::HasUpdateExitInterface => ModuleReply::Bool(true),
            ModuleCall::IsExitBusy => ModuleReply::Bool(false),
            ModuleCall::ReserveDoorForExit { .. } => ModuleReply::Int(if self.is_free_to_exit() {
                DOOR_1
            } else {
                DOOR_NONE_AVAILABLE
            }),
            ModuleCall::ExitObjectViaDoor { obj, .. } => {
                self.exit_object_via_door(world, me.object, obj);
                ModuleReply::Done
            }
            ModuleCall::UnreserveDoorForExit(_) => ModuleReply::Done,
            ModuleCall::ExitObjectByBudding { obj, bud_host } => {
                self.exit_object_by_budding(world, me.object, obj, bud_host);
                ModuleReply::Done
            }
            ModuleCall::SetRallyPoint(p) => {
                self.rally.set(&p);
                ModuleReply::Done
            }
            ModuleCall::GetRallyPoint => self
                .rally
                .get()
                .map_or(ModuleReply::Unhandled, ModuleReply::Coord),
            ModuleCall::GetNaturalRallyPoint => ModuleReply::Coord(natural_rally_point(
                world,
                me.object,
                &self.data.natural_rally_point,
                true,
            )),
            ModuleCall::GetNaturalRallyPointWithOffset(offset) => ModuleReply::Coord(
                natural_rally_point(world, me.object, &self.data.natural_rally_point, offset),
            ),
            ModuleCall::GetExitPosition => {
                exit_position(world, me.object, &self.data.unit_create_point)
                    .map_or(ModuleReply::Unhandled, ModuleReply::Coord)
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::QueueProductionExitUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::QueueProductionExitUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for QueueProductionExitUpdate {
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

/// The factory's `NewModuleDataProc` for `QueueProductionExitUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::QueueProductionExitUpdate(Arc::new(
        QueueProductionExitUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
