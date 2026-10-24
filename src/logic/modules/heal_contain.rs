// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/HealContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/HealContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   HealContain.cpp: Colin Day, July 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `HealContain`: a container that heals what goes in (an ambulance bay, a field hospital) and lets each rider out once it is fully healed.
//!
//! # Design
//!
//! As [`OpenContain`] for a class derived from it: the module holds its `OpenContain` by value and implements [`OpenContainVirtuals`], overriding
//! `update` and `isHealContain`; the data holds `OpenContainModuleData` by value and lifts its two field tables.
//!
//! # Upstream facts kept
//!
//! - `update` runs `OpenContain::update`, then heals every rider of its own list (`getContainList()`, the iterator advanced before the heal "which allows
//!   failure to not cause an infinite loop"), and returns `UPDATE_SLEEP_NONE` whatever the base asked for.
//! - `doHeal` heals through the rider's body directly (`body->attemptHealing`, not `Object::attemptHealing`), unchecked: a rider with no body is
//!   undefined ([`Undefined::NullBodyModule`]). A rider in for `TimeForFullHeal` frames or more (since `getContainedByFrame()`) gets its maximum
//!   health and is done; any other gets `maxHealth / TimeForFullHeal` (so a time of 0 heals fully at once).
//! - A rider that is done leaves through `reserveDoorForExit` and `exitObjectViaDoor` (the virtuals) when a door is free.
//!
//! `p1w13-sonnet`, batch R7, 2026-10-22.

use std::sync::Arc;

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData,
};
use crate::common::module::ModuleDataKind;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING, DEATH_NONE};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    UpdateModule, UpdateSleepTime, DOOR_NONE_AVAILABLE, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::open_contain::{
    self, out, OpenContain, OpenContainModuleData, OpenContainVirtuals,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class HealContainModuleData : public OpenContainModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct HealContainModuleData {
    /// The `OpenContainModuleData` base.
    pub open: OpenContainModuleData,
    /// `m_framesForFullHeal` (`TimeForFullHeal`): "time (in frames) something becomes fully healed".
    pub frames_for_full_heal: u32,
}

impl Default for HealContainModuleData {
    /// `HealContainModuleData()`: no time.
    fn default() -> Self {
        HealContainModuleData {
            open: OpenContainModuleData::default(),
            frames_for_full_heal: 0,
        }
    }
}

/// Lifts entry `k` of `OpenContainModuleData`'s table onto the `open` base.
fn lift_open(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut HealContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match open_contain::FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

/// Lifts entry `k` of the die mux table onto the `open` base.
fn lift_die(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut HealContainModuleData,
    u: &UserData,
) -> IniResult<()> {
    match open_contain::DIE_MUX_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.open, u),
        None => Ok(()),
    }
}

static LIFTED_OPEN: [FieldParse<HealContainModuleData>; 16] = [
    FieldParse::new(
        "ContainMax",
        |i, d, u| lift_open(0, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "EnterSound",
        |i, d, u| lift_open(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new("ExitSound", |i, d, u| lift_open(2, i, d, u), UserData::None),
    FieldParse::new(
        "DamagePercentToUnits",
        |i, d, u| lift_open(3, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "BurnedDeathToUnits",
        |i, d, u| lift_open(4, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowInsideKindOf",
        |i, d, u| lift_open(5, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ForbidInsideKindOf",
        |i, d, u| lift_open(6, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "PassengersAllowedToFire",
        |i, d, u| lift_open(7, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "PassengersInTurret",
        |i, d, u| lift_open(8, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "NumberOfExitPaths",
        |i, d, u| lift_open(9, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "DoorOpenTime",
        |i, d, u| lift_open(10, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "WeaponBonusPassedToPassengers",
        |i, d, u| lift_open(11, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowAlliesInside",
        |i, d, u| lift_open(12, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowEnemiesInside",
        |i, d, u| lift_open(13, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "AllowNeutralInside",
        |i, d, u| lift_open(14, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

static LIFTED_DIE: [FieldParse<HealContainModuleData>; 5] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die(0, i, d, u), UserData::None),
    FieldParse::new(
        "VeterancyLevels",
        |i, d, u| lift_die(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExemptStatus",
        |i, d, u| lift_die(2, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "RequiredStatus",
        |i, d, u| lift_die(3, i, d, u),
        UserData::None,
    ),
    FieldParse::end(),
];

/// `HealContainModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<HealContainModuleData>; 2] = [
    FieldParse::new(
        "TimeForFullHeal",
        |i, d, _| {
            d.frames_for_full_heal = i.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl HealContainModuleData {
    /// `HealContainModuleData::buildFieldParse(p)`: `OpenContainModuleData::buildFieldParse(p)`, then this class's table.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, HealContainModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LIFTED_OPEN)?;
        p.add(&LIFTED_DIE)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<HealContainModuleData> {
        let mut data = HealContainModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, HealContainModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `HealContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::HealContain(Arc::new(
        HealContainModuleData::friend_new_module_data(ini)?,
    )))
}

/// `class HealContain : public OpenContain`.
#[derive(Clone, Debug, PartialEq)]
pub struct HealContain {
    oc: OpenContain,
    /// `getHealContainModuleData()`.
    data: Arc<HealContainModuleData>,
}

/// Records that upstream is undefined here.
fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
    world.cuts.module_errors.push((me, IniError::undefined(u)));
}

impl HealContain {
    /// `getInterfaceMask()`: the base's.
    pub const fn get_interface_mask() -> i32 {
        OpenContain::get_interface_mask()
    }

    /// The constructor's first half: the base's (on a copy of its part of the data).
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<HealContainModuleData>>,
    ) -> IniResult<HealContain> {
        let data = module_data
            .ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let oc = OpenContain::unconstructed(object, Some(Arc::new(data.open.clone())))?;
        Ok(HealContain { oc, data })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        self.oc.update_module()
    }

    /// The module data.
    pub fn module_data(&self) -> &Arc<HealContainModuleData> {
        &self.data
    }

    /// The state, as words: the base's (no state of its own).
    pub fn state_words(&self) -> Vec<u32> {
        self.oc.state_words()
    }

    /// The constructor's second half: the base's.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.oc.world_construct(world, me);
    }

    /// `update()` as the scheduler runs it.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        open_contain::contain_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version (1), then the base's.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.oc.xfer_with(update, xfer)
    }

    /// `crc` with the scheduling half: "extend base class".
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.oc.crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half: "extend base class".
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.oc.load_post_process_with(update);
    }

    /// `doHeal(obj, framesForFullHeal)`: one frame's healing of one rider; `Some(true)` when it is done, `None` for a rider with no body (undefined).
    fn do_heal(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        obj: ObjectId,
        frames_for_full_heal: u32,
    ) -> Option<bool> {
        // setup the healing damageInfo structure with all but the amount
        let mut info = DamageInfo::default();
        info.input.damage_type = DAMAGE_HEALING;
        info.input.death_type = DEATH_NONE;
        info.input.source_id = me.object;
        let Some(body) = world.objects.find(obj).and_then(|o| o.body) else {
            undefined(world, me, Undefined::NullBodyModule);
            return None;
        };
        let max = out(self, world, me, |w| {
            w.module_call(obj, body, ModuleCall::GetMaxHealth)
        })
        .as_real(0.0);
        let since = world
            .frame()
            .wrapping_sub(world.objects.find(obj).map_or(0, |o| o.contained_by_frame));
        // "if we've been in here long enough ... set our health to max"
        if (since >= frames_for_full_heal) != crate::logic::modules::plant(1501) {
            info.input.amount = max;
            out(self, world, me, |w| {
                w.module_call(obj, body, ModuleCall::AttemptHealing(info))
            });
            Some(true)
        } else {
            // "give it a sliver of health as if it were at 0 health and would be fully healed at 'framesForFullHeal'"
            info.input.amount = if crate::logic::modules::plant(1502) {
                max
            } else {
                max / frames_for_full_heal as f32
            };
            out(self, world, me, |w| {
                w.module_call(obj, body, ModuleCall::AttemptHealing(info))
            });
            Some(false)
        }
    }
}

impl OpenContainVirtuals for HealContain {
    fn oc(&self) -> &OpenContain {
        &self.oc
    }
    fn oc_mut(&mut self) -> &mut OpenContain {
        &mut self.oc
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::HealContain(self)
    }
    fn from_body(b: UpdateBody) -> Option<Self> {
        match b {
            UpdateBody::HealContain(m) => Some(m),
            _ => None,
        }
    }

    /// `HealContain::update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        // "extending functionality"
        open_contain::base_update(self, world, me);
        let frames = self.data.frames_for_full_heal;
        // for each of our objects (`getContainList()`, the module's own), the iterator advanced first
        for obj in self.oc.contain_list().to_vec() {
            if obj == INVALID_ID {
                continue;
            }
            let Some(done) = self.do_heal(world, me, obj, frames) else {
                return UPDATE_SLEEP_NONE;
            };
            // "if we're done healing, we need to remove us from the healing container"
            if done && !crate::logic::modules::plant(1503) {
                let exit_door = self.reserve_door_for_exit(world, me, obj);
                if exit_door != DOOR_NONE_AVAILABLE {
                    self.exit_object_via_door(world, me, obj, exit_door);
                }
            }
        }
        if crate::logic::modules::plant(1504) {
            return crate::logic::module::update_module::UPDATE_SLEEP_FOREVER;
        }
        UPDATE_SLEEP_NONE
    }

    /// `isHealContain()`: `true`.
    fn is_heal_contain(&self) -> bool {
        !crate::logic::modules::plant(1505)
    }
}

impl UpdateBehavior for HealContain {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        self.oc.update_module()
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        OpenContainVirtuals::update(self, world, me)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::HealContain(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::HealContain(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        _call: ModuleCall,
    ) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

impl Snapshot for HealContain {
    /// `crc`: "extend base class".
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        Snapshot::crc(&mut self.oc, xfer)
    }

    /// `xfer` (version 1): the version, then `OpenContain::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        Snapshot::xfer(&mut self.oc, xfer)
    }

    /// `loadPostProcess`: "extend base class".
    fn load_post_process(&mut self) {
        Snapshot::load_post_process(&mut self.oc);
    }
}
