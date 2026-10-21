// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CheckpointUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CheckpointUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CheckpointUpdate`: a gate that opens for allies and stays shut for enemies. Every frame it looks for the closest enemy and the closest ally within
//! the object's vision range (with the geometry's minor radius stretched to its maximum, so that the gate does not oscillate), and, if the object has
//! a drawable, plays the opening or closing door animation when that changes and moves the geometry's minor radius by 0.333 towards open (0) or shut
//! (the maximum).
//!
//! `TheAI->findClosestEnemy(obj, visionRange, 0)` and `findClosestAlly` are layer 10's: [`World::ai_find_closest_enemy`] and
//! [`World::ai_find_closest_ally`] record a [`CutCall`] and answer from the tables `cuts.ai_enemy_near` and `cuts.ai_ally_near`.
//!
//! # Upstream facts kept
//!
//! - `if (m_enemyScanDelay == 0 || TRUE)`: the scan runs every frame, and the `else` branch that would count the delay down is dead code.
//! - The constructor adds `GameLogicRandomValue(0, ScanDelayTime)` to the scan delay, which nothing reads afterwards but `xfer`.
//! - The geometry is set (and the object re-registered with the partition manager, the drawable told) three times in a frame with a drawable and twice
//!   without one.
//!
//! `p1w12-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::model_state::{MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class CheckpointUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckpointUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_enemyScanDelayTime` (frames; one second by default).
    pub enemy_scan_delay_time: u32,
}

impl Default for CheckpointUpdateModuleData {
    /// `CheckpointUpdateModuleData()`.
    fn default() -> Self {
        CheckpointUpdateModuleData { update: UpdateModuleData::new(), enemy_scan_delay_time: LOGICFRAMES_PER_SECOND as u32 }
    }
}

/// `CheckpointUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<CheckpointUpdateModuleData>; 2] = [
    FieldParse::new("ScanDelayTime", |ini, d, _| { d.enemy_scan_delay_time = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl CheckpointUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CheckpointUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CheckpointUpdateModuleData> {
        let mut data = CheckpointUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CheckpointUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class CheckpointUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct CheckpointUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getCheckpointUpdateModuleData()`.
    data: Arc<CheckpointUpdateModuleData>,
    /// `m_enemyNear`.
    enemy_near: bool,
    /// `m_allyNear`.
    ally_near: bool,
    /// `m_maxMinorRadius`.
    max_minor_radius: f32,
    /// `m_enemyScanDelay`.
    enemy_scan_delay: u32,
}

impl PartialEq for CheckpointUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.enemy_near == other.enemy_near
            && self.ally_near == other.ally_near
            && self.max_minor_radius.to_bits() == other.max_minor_radius.to_bits()
            && self.enemy_scan_delay == other.enemy_scan_delay
    }
}

impl CheckpointUpdate {
    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the zeroed state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<CheckpointUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(CheckpointUpdate { update, data, enemy_near: false, ally_near: false, max_minor_radius: 0.0, enemy_scan_delay: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getCheckpointUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<CheckpointUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the enemy flag, the ally flag, the maximum minor radius (float
    /// bits) and the scan delay.
    pub fn state_words(&self) -> Vec<u32> {
        vec![u32::from(self.enemy_near), u32::from(self.ally_near), self.max_minor_radius.to_bits(), self.enemy_scan_delay]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls (none).
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

    /// `checkForAlliesAndEnemies()`: "look around us for enemies".
    fn check_for_allies_and_enemies(&mut self, world: &mut World, me: ObjectId) {
        // periodic enemy checks (`m_enemyScanDelay == 0 || TRUE`)
        self.enemy_scan_delay = self.data.enemy_scan_delay_time;

        // Kind of weird, but we have to set the geometry extents to the max extent before checking for nearest enemies and allies, or else the
        // stretch reaction to finding one will oscillate states open->closed->open...
        let Some(o) = world.objects.find(me) else { return };
        let mut geom = o.geometry_info.clone();
        let restore_special_radius = geom.get_minor_radius();
        geom.set_minor_radius(if crate::logic::modules::plant(961) { restore_special_radius } else { self.max_minor_radius });
        world.set_geometry_info(me, &geom);

        let vision_range = world.get_vision_range(me);
        self.enemy_near = world.ai_find_closest_enemy(me, vision_range, 0);
        self.ally_near = world.ai_find_closest_ally(me, vision_range, 0);

        // here we restore the radius so that other units can path past the open gate
        geom.set_minor_radius(restore_special_radius);
        if !crate::logic::modules::plant(965) {
            world.set_geometry_info(me, &geom);
        }
    }
}

impl UpdateBehavior for CheckpointUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// "Sit around until an enemy gets near."
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let was_an_ally = self.ally_near;
        let was_an_enemy = self.enemy_near;

        self.check_for_allies_and_enemies(world, me.object);

        let change = (was_an_ally != self.ally_near && !crate::logic::modules::plant(968)) || was_an_enemy != self.enemy_near;
        let open = !self.enemy_near && (self.ally_near || crate::logic::modules::plant(966));

        if world.has_drawable(me.object) {
            if change {
                if open {
                    // change the state of the art to open the door!
                    world.clear_and_set_model_condition_state(me.object, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING);
                } else if !crate::logic::modules::plant(962) {
                    // change the state of the art to close the door!
                    world.clear_and_set_model_condition_state(me.object, MODELCONDITION_DOOR_1_OPENING, MODELCONDITION_DOOR_1_CLOSING);
                }
            }

            let Some(o) = world.objects.find(me.object) else { return UPDATE_SLEEP_NONE };
            let mut geom = o.geometry_info.clone();

            // This method is more accidental than above, but it works for an unimportant thing like checkpoint
            let radius = geom.get_minor_radius();
            if open {
                if radius > 0.0 || (crate::logic::modules::plant(967) && radius == 0.0) {
                    geom.set_minor_radius(radius - if crate::logic::modules::plant(963) { 0.33 } else { 0.333 });
                }
            } else if radius < self.max_minor_radius {
                geom.set_minor_radius(radius + 0.333);
            }

            world.set_geometry_info(me.object, &geom);
        }
        UPDATE_SLEEP_NONE
    }
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.max_minor_radius = world.objects.find(me.object).map_or(0.0, |o| o.geometry_info.get_minor_radius());
        // bias a random amount so everyone doesn't spike at once
        let delay = self.data.enemy_scan_delay_time;
        let r = world.random.game_logic_random_value(0, delay as i32);
        self.enemy_scan_delay = self.enemy_scan_delay.wrapping_add(r as u32);
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_bool(&mut self.enemy_near)?;
        xfer.xfer_bool(&mut self.ally_near)?;
        xfer.xfer_real(&mut self.max_minor_radius)?;
        if crate::logic::modules::plant(964) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.enemy_scan_delay)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CheckpointUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CheckpointUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for CheckpointUpdate {
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

/// The factory's `NewModuleDataProc` for `CheckpointUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CheckpointUpdate(Arc::new(CheckpointUpdateModuleData::friend_new_module_data(ini)?)))
}
