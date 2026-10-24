// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AutoFindHealingUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AutoFindHealingUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `AutoFindHealingUpdate`: a computer player's unit that goes to be healed. A unit of a human player does nothing. Otherwise, every
//! `ScanRate` frames (it counts down a frame at a time and is never asleep), a unit with an AI and a body, no healthier than `NeverHeal` of its
//! maximum and idle, looks for the closest `HEAL_PAD` within `ScanRange` (2D, from the centre) and orders itself there (`aiGetHealed`).
//!
//! Upstream's `AlwaysHeal` test sits after a `return` and never runs: a busy unit is never sent, however hurt. The module data is
//! `ModuleData`-derived upstream; the `UpdateModuleData` base here has no fields and parses none, so it is the same.
//!
//! The scan is [`World::partition_iterate_objects_in_range`], the distances [`World::partition_get_distance_squared_center_2d`] and the order
//! [`ModuleCall::AiGetHealed`].

use std::sync::Arc;

use crate::common::game_common::PlayerType;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_HEAL_PAD;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class AutoFindHealingUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoFindHealingUpdateModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_scanFrames` (`ScanRate`).
    pub scan_frames: u32,
    /// `m_scanRange`.
    pub scan_range: f32,
    /// `m_neverHeal`.
    pub never_heal: f32,
    /// `m_alwaysHeal` (read, never used: see the module's doc).
    pub always_heal: f32,
}

impl Default for AutoFindHealingUpdateModuleData {
    /// `AutoFindHealingUpdateModuleData()`: no scan rate or range, `NeverHeal` 0.95, `AlwaysHeal` 0.25.
    fn default() -> Self {
        AutoFindHealingUpdateModuleData { update: UpdateModuleData::default(), scan_frames: 0, scan_range: 0.0, never_heal: 0.95, always_heal: 0.25 }
    }
}

/// `AutoFindHealingUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<AutoFindHealingUpdateModuleData>; 5] = [
    FieldParse::new("ScanRate", |ini, d, _| { d.scan_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScanRange", |ini, d, _| { d.scan_range = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("NeverHeal", |ini, d, _| { d.never_heal = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("AlwaysHeal", |ini, d, _| { d.always_heal = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl AutoFindHealingUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (no fields of its own besides the behaviour base's), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, AutoFindHealingUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<AutoFindHealingUpdateModuleData> {
        let mut data = AutoFindHealingUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, AutoFindHealingUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class AutoFindHealingUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct AutoFindHealingUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getAutoFindHealingUpdateModuleData()`.
    data: Arc<AutoFindHealingUpdateModuleData>,
    /// `m_nextScanFrames`: the frames left before the next scan.
    next_scan_frames: i32,
}

impl PartialEq for AutoFindHealingUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same frames to the next scan.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.next_scan_frames == other.next_scan_frames
    }
}

/// `obj->getControllingPlayer()->getPlayerType() == PLAYER_HUMAN`. The player is read unchecked upstream, so an object with none is recorded as
/// undefined ([`Undefined::NullControllingPlayer`]) and taken as not human. Shared with `PilotFindVehicleUpdate`.
pub(crate) fn controlled_by_human(world: &mut World, me: ModuleHandle) -> bool {
    match world.get_controlling_player(me.object) {
        Some(p) => world.players.get_nth(p).is_some_and(|pl| pl.player_type == PlayerType::Human),
        None => {
            world.cuts.module_errors.push((me, IniError::undefined(Undefined::NullControllingPlayer)));
            false
        }
    }
}

impl AutoFindHealingUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and
    /// `m_nextScanFrames` 0.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<AutoFindHealingUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(AutoFindHealingUpdate { update, data, next_scan_frames: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getAutoFindHealingUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<AutoFindHealingUpdateModuleData> {
        &self.data
    }

    /// The state the test compares: `m_nextScanFrames`.
    pub fn state_words(&self) -> [u32; 1] {
        [self.next_scan_frames as u32]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: nothing (it sets no wake frame).
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
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

    /// `scanClosestTarget()`: the closest `HEAL_PAD` of the scan (the first of equals), `None` for `NULL`.
    fn scan_closest_target(&self, world: &mut World, me: ObjectId) -> Option<ObjectId> {
        let mut best: Option<(ObjectId, f32)> = None;
        for other in world.partition_iterate_objects_in_range(me, self.data.scan_range) {
            let heal_pad = world.objects.find(other).is_some_and(|o| o.template().is_kind_of(KINDOF_HEAL_PAD));
            if !heal_pad && !crate::logic::modules::plant(1854) {
                continue;
            }
            let dist = world.partition_get_distance_squared_center_2d(me, other);
            match best {
                None => best = Some((other, dist)),
                Some((_, closest)) if dist < closest || (crate::logic::modules::plant(1852) && dist == closest) => best = Some((other, dist)),
                Some(_) => {}
            }
        }
        best.map(|(id, _)| id)
    }
}

impl UpdateBehavior for AutoFindHealingUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if controlled_by_human(world, me) {
            return UPDATE_SLEEP_NONE;
        }
        let d = self.data.clone();
        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            return UPDATE_SLEEP_NONE;
        }
        if !crate::logic::modules::plant(1853) {
            self.next_scan_frames = d.scan_frames as i32;
        }
        let Some(ai) = world.objects.find(me.object).and_then(|o| o.ai) else { return UPDATE_SLEEP_NONE };
        let Some((health, max_health)) = world.body_health(me.object) else { return UPDATE_SLEEP_NONE };
        if health > max_health * d.never_heal || (crate::logic::modules::plant(1851) && health == max_health * d.never_heal) {
            return UPDATE_SLEEP_NONE;
        }
        if !world.module_call(me.object, ai, ModuleCall::AiIsIdle).as_bool() {
            // upstream tests `AlwaysHeal` after this return, so never
            return UPDATE_SLEEP_NONE;
        }
        if let Some(heal_unit) = self.scan_closest_target(world, me.object) {
            world.module_call(me.object, ai, ModuleCall::AiGetHealed { target: heal_unit, command_source: CMD_FROM_AI });
        }
        UPDATE_SLEEP_NONE
    }
    /// The field after the base: `m_nextScanFrames`.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_int(&mut self.next_scan_frames)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::AutoFindHealingUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::AutoFindHealingUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for AutoFindHealingUpdate {
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

/// The factory's `NewModuleDataProc` for `AutoFindHealingUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::AutoFindHealingUpdate(Arc::new(AutoFindHealingUpdateModuleData::friend_new_module_data(ini)?)))
}
