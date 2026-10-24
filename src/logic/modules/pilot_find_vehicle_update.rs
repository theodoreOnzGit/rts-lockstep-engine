// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PilotFindVehicleUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PilotFindVehicleUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PilotFindVehicleUpdate`: a computer player's pilot that looks for a vehicle to man. A pilot of a human player, or one with no AI, sleeps
//! for ever. Otherwise every `ScanRate` frames an idle pilot takes the nearest living vehicle of its own player on its map within `ScanRange`
//! (2D, from the centre) that has a body with at least `MinHealth` of its maximum and that one of the pilot's collide modules would like to
//! collide with, and enters it (`aiEnter`). With none, it goes once to the centre of its player AI's base, if the AI has one
//! (`aiMoveToPosition`), and not again until it has next found a vehicle.
//!
//! The module data is `ModuleData`-derived upstream; the `UpdateModuleData` base here has no fields and parses none, so it is the same.
//!
//! The scan is [`World::partition_iterate_vehicles_near_to_far`], the collide test [`ModuleCall::WouldLikeToCollideWith`] of each collide
//! module in module order, the base [`World::player_get_ai_base_center`].

use std::sync::Arc;

use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_COLLIDE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime,
    UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::auto_find_healing_update::controlled_by_human;
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class PilotFindVehicleUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct PilotFindVehicleUpdateModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_scanFrames` (`ScanRate`).
    pub scan_frames: u32,
    /// `m_scanRange`.
    pub scan_range: f32,
    /// `m_minHealth`.
    pub min_health: f32,
}

impl Default for PilotFindVehicleUpdateModuleData {
    /// `PilotFindVehicleUpdateModuleData()`: no scan rate or range, `MinHealth` 0.5.
    fn default() -> Self {
        PilotFindVehicleUpdateModuleData {
            update: UpdateModuleData::default(),
            scan_frames: 0,
            scan_range: 0.0,
            min_health: 0.5,
        }
    }
}

/// `PilotFindVehicleUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<PilotFindVehicleUpdateModuleData>; 4] = [
    FieldParse::new(
        "ScanRate",
        |ini, d, _| {
            d.scan_frames = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "ScanRange",
        |ini, d, _| {
            d.scan_range = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "MinHealth",
        |ini, d, _| {
            d.min_health = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl PilotFindVehicleUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (no fields of its own besides the behaviour base's), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, PilotFindVehicleUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<PilotFindVehicleUpdateModuleData> {
        let mut data = PilotFindVehicleUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                PilotFindVehicleUpdateModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class PilotFindVehicleUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct PilotFindVehicleUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getPilotFindVehicleUpdateModuleData()`.
    data: Arc<PilotFindVehicleUpdateModuleData>,
    /// `m_didMoveToBase`: whether the pilot was sent to its base since it last found a vehicle.
    did_move_to_base: bool,
}

impl PartialEq for PilotFindVehicleUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same base flag.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.did_move_to_base == other.did_move_to_base
    }
}

impl PilotFindVehicleUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and
    /// `m_didMoveToBase` false.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<PilotFindVehicleUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(PilotFindVehicleUpdate {
            update,
            data,
            did_move_to_base: false,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getPilotFindVehicleUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<PilotFindVehicleUpdateModuleData> {
        &self.data
    }

    /// The state the test compares: `m_didMoveToBase`.
    pub fn state_words(&self) -> [u32; 1] {
        [u32::from(self.did_move_to_base)]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: awake now.
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

    /// `xfer` with the scheduling half the slot holds: the version, that half, the field.
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

    /// `scanClosestTarget()`: the first vehicle of the scan (nearest first) with a body at least `MinHealth` of its maximum that a collide
    /// module of the pilot would like to collide with; `None` for `NULL`.
    fn scan_closest_target(&self, world: &mut World, me: ObjectId) -> Option<ObjectId> {
        for other in world.partition_iterate_vehicles_near_to_far(me, self.data.scan_range) {
            let Some((health, max_health)) = world.body_health(other) else {
                continue;
            };
            if health < max_health * self.data.min_health && !crate::logic::modules::plant(1861) {
                continue;
            }
            for collide in world.modules_with(me, MODULEINTERFACE_COLLIDE) {
                if world
                    .module_call(me, collide, ModuleCall::WouldLikeToCollideWith { other })
                    .as_bool()
                {
                    return Some(other);
                }
            }
        }
        None
    }
}

impl UpdateBehavior for PilotFindVehicleUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if controlled_by_human(world, me) {
            return if crate::logic::modules::plant(1863) {
                UPDATE_SLEEP_NONE
            } else {
                UPDATE_SLEEP_FOREVER
            };
        }
        let d = self.data.clone();
        let Some(ai) = world.objects.find(me.object).and_then(|o| o.ai) else {
            return UPDATE_SLEEP_FOREVER;
        };
        if !world
            .module_call(me.object, ai, ModuleCall::AiIsIdle)
            .as_bool()
        {
            return update_sleep(d.scan_frames);
        }
        if let Some(upgrade_unit) = self.scan_closest_target(world, me.object) {
            world.module_call(
                me.object,
                ai,
                ModuleCall::AiEnter {
                    target: upgrade_unit,
                    command_source: CMD_FROM_AI,
                },
            );
            if !crate::logic::modules::plant(1862) {
                self.did_move_to_base = false;
            }
        } else if !self.did_move_to_base {
            let base = world
                .get_controlling_player(me.object)
                .and_then(|p| world.player_get_ai_base_center(p));
            if let Some(pos) = base {
                world.module_call(
                    me.object,
                    ai,
                    ModuleCall::AiMoveToPosition {
                        pos,
                        command_source: CMD_FROM_AI,
                    },
                );
                self.did_move_to_base = true;
            }
        }
        update_sleep(d.scan_frames)
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
    /// The field after the base: `m_didMoveToBase`.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        if crate::logic::modules::plant(1864) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.did_move_to_base)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::PilotFindVehicleUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::PilotFindVehicleUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for PilotFindVehicleUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the field.
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

/// The factory's `NewModuleDataProc` for `PilotFindVehicleUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::PilotFindVehicleUpdate(Arc::new(
        PilotFindVehicleUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
