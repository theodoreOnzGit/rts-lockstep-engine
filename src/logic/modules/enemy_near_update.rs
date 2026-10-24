// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/EnemyNearUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/EnemyNearUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `EnemyNearUpdate`: every `ScanDelayTime` frames looks for the closest enemy the object can see (`TheAI->findClosestEnemy(obj, visionRange, CAN_SEE)`), and
//! gives its drawable `MODELCONDITION_ENEMYNEAR` while there is one. Never sleeps.
//!
//! `TheAI->findClosestEnemy` is layer 10's: [`World::ai_find_closest_enemy`] records a [`CutCall`] and answers from `cuts.ai_enemy_near`.
//!
//! # Upstream facts kept
//!
//! - The constructor adds `GameLogicRandomValue(0, ScanDelayTime)` to the scan delay, so the first scan is that many frames away.
//! - The scan runs when the delay is 0, then reloads the full delay (a scan every `ScanDelayTime + 1` frames).
//! - `getDrawable()` is tested for null before the model condition is changed.
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::model_state::MODELCONDITION_ENEMYNEAR;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `AI::CAN_SEE`.
pub const AI_CAN_SEE: u32 = 1;

/// `class EnemyNearUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct EnemyNearUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_enemyScanDelayTime` (frames; one second by default).
    pub enemy_scan_delay_time: u32,
}

impl Default for EnemyNearUpdateModuleData {
    /// `EnemyNearUpdateModuleData()`.
    fn default() -> Self {
        EnemyNearUpdateModuleData {
            update: UpdateModuleData::new(),
            enemy_scan_delay_time: LOGICFRAMES_PER_SECOND as u32,
        }
    }
}

/// `EnemyNearUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<EnemyNearUpdateModuleData>; 2] = [
    FieldParse::new(
        "ScanDelayTime",
        |ini, d, _| {
            d.enemy_scan_delay_time = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl EnemyNearUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, EnemyNearUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<EnemyNearUpdateModuleData> {
        let mut data = EnemyNearUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, EnemyNearUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class EnemyNearUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct EnemyNearUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getEnemyNearUpdateModuleData()`.
    data: Arc<EnemyNearUpdateModuleData>,
    /// `m_enemyScanDelay`.
    enemy_scan_delay: u32,
    /// `m_enemyNear`.
    enemy_near: bool,
}

impl PartialEq for EnemyNearUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.enemy_scan_delay == other.enemy_scan_delay
            && self.enemy_near == other.enemy_near
    }
}

impl EnemyNearUpdate {
    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the zeroed state.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<EnemyNearUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(EnemyNearUpdate {
            update,
            data,
            enemy_scan_delay: 0,
            enemy_near: false,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getEnemyNearUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<EnemyNearUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the scan delay and the enemy flag.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.enemy_scan_delay, u32::from(self.enemy_near)]
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

    /// `checkForEnemies()`: "look around us for enemies".
    fn check_for_enemies(&mut self, world: &mut World, me: ObjectId) {
        if self.enemy_scan_delay == 0 {
            self.enemy_scan_delay = if crate::logic::modules::plant(1171) {
                self.data.enemy_scan_delay_time.wrapping_sub(1)
            } else {
                self.data.enemy_scan_delay_time
            };
            let vision_range = world.get_vision_range(me);
            self.enemy_near = world.ai_find_closest_enemy(
                me,
                vision_range,
                if crate::logic::modules::plant(1172) {
                    0
                } else {
                    AI_CAN_SEE
                },
            );
        } else {
            self.enemy_scan_delay -= 1;
        }
    }
}

impl UpdateBehavior for EnemyNearUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let enemy_was_near = self.enemy_near;

        self.check_for_enemies(world, me.object);

        if self.enemy_near && !enemy_was_near {
            if world.has_drawable(me.object) {
                world.set_model_condition_state(me.object, MODELCONDITION_ENEMYNEAR);
            }
        } else if !self.enemy_near
            && enemy_was_near
            && world.has_drawable(me.object)
            && !crate::logic::modules::plant(1173)
        {
            world.clear_model_condition_state(me.object, MODELCONDITION_ENEMYNEAR);
        }
        UPDATE_SLEEP_NONE
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        let delay = self.data.enemy_scan_delay_time;
        let r = world.random.game_logic_random_value(0, delay as i32);
        self.enemy_scan_delay = self.enemy_scan_delay.wrapping_add(r as u32);
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.enemy_scan_delay)?;
        if crate::logic::modules::plant(1174) {
            return Ok(());
        }
        xfer.xfer_bool(&mut self.enemy_near)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::EnemyNearUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::EnemyNearUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for EnemyNearUpdate {
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

/// The factory's `NewModuleDataProc` for `EnemyNearUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::EnemyNearUpdate(Arc::new(
        EnemyNearUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
