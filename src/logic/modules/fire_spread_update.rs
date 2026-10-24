// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FireSpreadUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireSpreadUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FireSpreadUpdate`: while its object is aflame, drops an ember object creation list and tries to set the closest flammable object in range alight, at a random
//! interval. `FlammableUpdate` starts it ([`ModuleCall::StartFireSpreading`]); it sleeps for ever while the object is not aflame.
//!
//! `ObjectCreationList::create(list, obj, NULL)` is [`CutCall::OclCreate`] (the list kept by name). `ThePartitionManager->getClosestObject(obj, range,
//! FROM_CENTER_3D, { PartitionFilterFlammable })` is the cut call [`World::partition_closest_candidates`]: the objects in range, nearest first, to which the
//! module applies its own filter (`findUpdateModule("FlammableUpdate")` and its `wouldIgnite()`) in order, the first allowed one being the closest. The candidate
//! `tryToIgnite()`s through [`ModuleCall::TryToIgnite`].
//!
//! # Upstream facts kept
//!
//! - The delay is `GameLogicRandomValue(min, max)` converted to `UnsignedInt` and raised to 1; the arguments are `UnsignedInt`s passed as `Int`.
//! - The ember list is created on every update while aflame, whether or not a range is set.
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_AFLAME;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class FireSpreadUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FireSpreadUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_oclEmbers`, by name (`None` for `NULL`).
    pub ocl_embers: Option<Vec<u8>>,
    /// `m_minSpreadTryDelayData`.
    pub min_spread_try_delay: u32,
    /// `m_maxSpreadTryDelayData`.
    pub max_spread_try_delay: u32,
    /// `m_spreadTryRange`.
    pub spread_try_range: f32,
}

/// `FireSpreadUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<FireSpreadUpdateModuleData>; 5] = [
    FieldParse::new(
        "OCLEmbers",
        |ini, d, _| {
            // INI::parseObjectCreationList: `None` is no list; any other name is looked up (here, when it is used)
            let token = ini.get_next_token(None)?;
            d.ocl_embers = (!token.eq_ignore_ascii_case(b"None")).then_some(token);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("MinSpreadDelay", |ini, d, _| { d.min_spread_try_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxSpreadDelay", |ini, d, _| { d.max_spread_try_delay = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SpreadTryRange", |ini, d, _| { d.spread_try_range = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl FireSpreadUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FireSpreadUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FireSpreadUpdateModuleData> {
        let mut data = FireSpreadUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FireSpreadUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class FireSpreadUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct FireSpreadUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getFireSpreadUpdateModuleData()`.
    data: Arc<FireSpreadUpdateModuleData>,
}

impl PartialEq for FireSpreadUpdate {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl FireSpreadUpdate {
    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<FireSpreadUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(FireSpreadUpdate { update, data })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getFireSpreadUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<FireSpreadUpdateModuleData> {
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

    /// The module's answer to one of `Object`'s calls: `startFireSpreading`.
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

    /// `calcNextSpreadDelay()`.
    fn calc_next_spread_delay(&self, world: &mut World) -> u32 {
        let d = &self.data;
        let mut delay = world.random.game_logic_random_value(d.min_spread_try_delay as i32, d.max_spread_try_delay as i32) as u32;
        if delay < 1 && !crate::logic::modules::plant(1203) {
            delay = 1;
        }
        delay
    }

    /// `PartitionFilterFlammable::allow(obj)`: it has a `FlammableUpdate` that would ignite.
    fn allow(world: &mut World, obj: ObjectId) -> bool {
        match world.find_module(obj, b"FlammableUpdate") {
            None => false,
            Some(idx) => world.module_call(obj, idx, ModuleCall::WouldIgnite).as_bool() || crate::logic::modules::plant(1204),
        }
    }
}

impl UpdateBehavior for FireSpreadUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let d = self.data.clone();
        let us = me.object;

        if !world.objects.find(us).is_some_and(|o| o.status.test(OBJECT_STATUS_AFLAME)) && !crate::logic::modules::plant(1201) {
            return UPDATE_SLEEP_FOREVER; // not on fire -- sleep forever
        }

        if let Some(ocl) = d.ocl_embers.clone().filter(|_| !crate::logic::modules::plant(1206)) {
            world.cut(CutCall::OclCreate { object: us, ocl });
        }

        if d.spread_try_range != 0.0 && !(crate::logic::modules::plant(1207) && d.spread_try_range < 0.0) {
            let candidates = world.partition_closest_candidates(us, d.spread_try_range);
            let object_to_light = candidates.into_iter().find(|&c| FireSpreadUpdate::allow(world, c));
            if let Some(target) = object_to_light {
                if let Some(idx) = world.find_module(target, b"FlammableUpdate") {
                    if !crate::logic::modules::plant(1205) {
                        world.module_call(target, idx, ModuleCall::TryToIgnite);
                    }
                }
            }
        }

        update_sleep(self.calc_next_spread_delay(world))
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::StartFireSpreading => {
                if !world.objects.find(me.object).is_some_and(|o| o.status.test(OBJECT_STATUS_AFLAME)) && !crate::logic::modules::plant(1202) {
                    return ModuleReply::Done; // sorry, must be on fire
                }
                let delay = self.calc_next_spread_delay(world);
                update_behavior::set_wake_frame(world, update_sleep(delay));
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FireSpreadUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FireSpreadUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for FireSpreadUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base (no fields of its own).
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

/// The factory's `NewModuleDataProc` for `FireSpreadUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FireSpreadUpdate(Arc::new(FireSpreadUpdateModuleData::friend_new_module_data(ini)?)))
}
