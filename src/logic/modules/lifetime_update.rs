// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/LifetimeUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/LifetimeUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, December 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `LifetimeUpdate`: counts a random lifetime down and kills (not destroys)
//! the object when it is up. A hulk gets the script override instead, when
//! the game logic has one.
//!
//! # Differences from upstream
//!
//! - The random draw uses the logic generator passed in (`TheGameLogic`'s
//!   `GameLogicRandomValue` is the same generator, owned by the caller).
//! - The `TheGameLogic` and `Object` calls go through [`UpdateHost`]; the
//!   world implements it in layer 9's wiring.
//! - The wake frame of the constructor is a call to
//!   [`UpdateHost::awaken_update_module`], as upstream's `setWakeFrame`.
//!
//! `m_dieFrame` is set by every draw (the constructor and `setLifetimeRange`),
//! as upstream; `getDieFrame` reports the latest draw.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::ObjectId;
use crate::common::random_value::RandomValues;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
};
// --- p1w10-sonnet ---
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
// --- end p1w10-sonnet ---
use crate::logic::modules::UpdateHost;

/// `class LifetimeUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LifetimeUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_minFrames` (`MinLifetime`, a duration in frames).
    pub min_frames: u32,
    /// `m_maxFrames` (`MaxLifetime`, a duration in frames).
    pub max_frames: u32,
}

impl LifetimeUpdateModuleData {
    /// `LifetimeUpdateModuleData()`: both frames 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// `LifetimeUpdateModuleData::buildFieldParse(p)`: the base's (which is
    /// the behaviour base's), then `MinLifetime` and `MaxLifetime`.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, LifetimeUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFETIME_UPDATE_MODULE_DATA_FIELD_PARSE)
    }

    /// `LifetimeUpdate::friend_newModuleData(ini)`: new data, read with
    /// `initFromINIMultiProc` if there is an `ini`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<LifetimeUpdateModuleData> {
        let mut data = LifetimeUpdateModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, LifetimeUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `dataFieldParse` of `LifetimeUpdateModuleData::buildFieldParse`: both are
/// `INI::parseDurationUnsignedInt`.
pub static LIFETIME_UPDATE_MODULE_DATA_FIELD_PARSE: [FieldParse<LifetimeUpdateModuleData>; 3] = [
    FieldParse::new(
        "MinLifetime",
        |ini, d, _: &UserData| {
            d.min_frames = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "MaxLifetime",
        |ini, d, _: &UserData| {
            d.max_frames = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class LifetimeUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct LifetimeUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getLifetimeUpdateModuleData()`.
    data: Arc<LifetimeUpdateModuleData>,
    /// `m_dieFrame`: the frame the random lifetime ends on.
    die_frame: u32,
}

impl LifetimeUpdate {
    /// `LifetimeUpdate(thing, moduleData)`: draws the lifetime (the hulk
    /// override if the object is a hulk and the logic has one), then
    /// `setWakeFrame`. `INI_INVALID_DATA` for null data.
    pub fn new(
        object: ObjectId,
        module_data: Option<Arc<LifetimeUpdateModuleData>>,
        host: &mut impl UpdateHost,
        rng: &mut RandomValues,
    ) -> IniResult<Self> {
        let mut me = LifetimeUpdate::unconstructed(object, module_data)?;
        me.construct(host, rng);
        Ok(me)
    }

    // --- p1w10-sonnet ---
    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<LifetimeUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(LifetimeUpdate { update, data, die_frame: 0 })
    }

    /// The second half of the constructor: draws the lifetime (the hulk override if the object is a hulk and the logic has one),
    /// then `setWakeFrame`. The world runs it once the module is part of its object.
    pub fn construct(&mut self, host: &mut impl UpdateHost, rng: &mut RandomValues) {
        let data = self.data.clone();
        let delay = if host.is_kind_of_hulk() && host.hulk_max_lifetime_override() != -1 {
            let over = host.hulk_max_lifetime_override() as u32;
            self.calc_sleep_delay(host, rng, over, over)
        } else {
            self.calc_sleep_delay(host, rng, data.min_frames, data.max_frames)
        };
        host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), update_sleep(delay)));
    }
    // --- end p1w10-sonnet ---

    /// `setLifetimeRange(minFrames, maxFrames)`: a new random lifetime, then
    /// `setWakeFrame`.
    pub fn set_lifetime_range(&mut self, host: &mut impl UpdateHost, rng: &mut RandomValues, min_frames: u32, max_frames: u32) {
        let delay = self.calc_sleep_delay(host, rng, min_frames, max_frames);
        host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), update_sleep(delay)));
    }

    /// `getDieFrame()`.
    pub fn get_die_frame(&self) -> u32 {
        self.die_frame
    }

    /// `update()`: kill (not destroy) the object, then sleep forever.
    pub fn update(&mut self, host: &mut impl UpdateHost) -> UpdateSleepTime {
        host.kill_object();
        UPDATE_SLEEP_FOREVER
    }

    /// `calcSleepDelay(minFrames, maxFrames)`: a random delay in the range
    /// (at least 1), and `m_dieFrame` = the current frame plus it.
    fn calc_sleep_delay(&mut self, host: &impl UpdateHost, rng: &mut RandomValues, min_frames: u32, max_frames: u32) -> u32 {
        let mut delay = rng.game_logic_random_value(min_frames as i32, max_frames as i32) as u32;
        if delay < 1 {
            delay = 1;
        }
        self.die_frame = host.frame().wrapping_add(delay);
        delay
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getLifetimeUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<LifetimeUpdateModuleData> {
        &self.data
    }
}

impl Snapshot for LifetimeUpdate {
    /// `LifetimeUpdate::crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// Version 1: the base, then `m_dieFrame`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.update.xfer(xfer)?;
        xfer.xfer_unsigned_int(&mut self.die_frame)
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

// --- p1w10-sonnet ---
impl PartialEq for LifetimeUpdate {
    /// The same state: the same data object (upstream compares pointers) and the same `m_dieFrame`. (`UpdateBody` is compared in tests.)
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.die_frame == other.die_frame
    }
}

/// The factory's `NewModuleDataProc` for `LifetimeUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<crate::common::module::ModuleDataKind> {
    Ok(crate::common::module::ModuleDataKind::LifetimeUpdate(Arc::new(LifetimeUpdateModuleData::friend_new_module_data(ini)?)))
}

impl LifetimeUpdate {
    /// `update()` as the scheduler runs it: this module is the running one for the host calls, and it is lent to its slot for the
    /// duration, so that a call that reaches it from inside (`destroyObject` runs `onDelete` on every module of the object, this
    /// one included) is delivered at once on its live state, as upstream's plain recursion delivers it.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        let Some(h) = world.module_handle_of_update(me) else {
            return self.update(world);
        };
        let lent = crate::logic::world::update_body::UpdateBody::LifetimeUpdate(self.clone());
        let (r, back) = world.lend_body(h, lent, |w| w.in_update_module(me, |w| self.update(w)));
        if let Some(crate::logic::world::update_body::UpdateBody::LifetimeUpdate(b)) = back {
            *self = b;
        }
        r
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, _me: ModuleHandle) {
        let mut rng = std::mem::take(&mut world.random);
        self.construct(world, &mut rng);
        world.random = rng;
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, _me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::SetLifetimeRange { min_frames, max_frames } => {
                let mut rng = std::mem::take(&mut world.random);
                self.set_lifetime_range(world, &mut rng, min_frames, max_frames);
                world.random = rng;
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }

    /// The fields after the base: `m_dieFrame`.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.die_frame)
    }

    /// `xfer` with the scheduling half the slot holds (the live `UpdateModule`): the version, that half, then the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        update.xfer(xfer)?;
        self.xfer_fields(xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update.crc(xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update.load_post_process();
    }
}
// --- end p1w10-sonnet ---
