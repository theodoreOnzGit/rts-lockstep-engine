// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/StatusDamageHelper.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StatusDamageHelper`: sets an object status for a number of frames
//! (`doStatusDamage`) and clears it when its sleep ends. Asleep until used.
//! Unlike most update modules it runs while the object is disabled
//! (`getDisabledTypesToProcess()` is `DISABLEDMASK_ALL`).

use std::sync::Arc;

use super::object_helper::{HelperHost, ObjectHelper};
use super::plant;
use crate::common::base_type::real_to_int_floor;
use crate::common::disabled_types::{disabledmask_all, DisabledMaskType};
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_NONE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
// --- p1w10-sonnet ---
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
// --- end p1w10-sonnet ---
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime,
    UPDATE_SLEEP_FOREVER,
};

/// `class StatusDamageHelper : public ObjectHelper`.
#[derive(Clone, Debug)]
pub struct StatusDamageHelper {
    helper: ObjectHelper,
    /// `m_statusToHeal` (`OBJECT_STATUS_NONE` when idle).
    pub status_to_heal: ObjectStatusTypes,
    /// `m_frameToHeal`.
    pub frame_to_heal: u32,
}

impl StatusDamageHelper {
    /// `StatusDamageHelper(thing, modData)`: the `ObjectHelper`, no status,
    /// frame 0, then `setWakeFrame(UPDATE_SLEEP_FOREVER)` (a second wake call
    /// after the base's).
    pub fn new(
        object: ObjectId,
        module_data: Option<Arc<UpdateModuleData>>,
        host: &mut impl HelperHost,
    ) -> IniResult<Self> {
        let mut me = StatusDamageHelper::unconstructed(object, module_data)?;
        me.construct(host);
        Ok(me)
    }

    /// The first half of the constructor, which touches no world: no status, frame 0.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<UpdateModuleData>>,
    ) -> IniResult<Self> {
        let mut helper =
            ObjectHelper::unconstructed(object, module_data, SleepyUpdatePhase::Normal)?;
        helper
            .update_module_mut()
            .set_disabled_types_to_process(disabledmask_all()); // p1w11-sonnet: the scheduler reads the arena's base
        Ok(StatusDamageHelper {
            helper,
            status_to_heal: OBJECT_STATUS_NONE,
            frame_to_heal: 0,
        })
    }

    /// The second half: the base's wake, then a second `setWakeFrame(UPDATE_SLEEP_FOREVER)`. The world runs it once the module
    /// is part of its object.
    pub fn construct(&mut self, host: &mut impl HelperHost) {
        self.helper.construct(host);
        if !plant(56) {
            host.awaken_update_module(UpdateModule::set_wake_frame(
                host.frame(),
                UPDATE_SLEEP_FOREVER,
            ));
        }
    }

    /// `getDisabledTypesToProcess()`: `DISABLEDMASK_ALL`.
    pub fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        if plant(40) {
            DisabledMaskType::new()
        } else {
            disabledmask_all()
        }
    }

    /// `update()`: "we are sleep driven, so seeing an update means our timer
    /// is ready implicitly": clear the status, sleep forever. (Upstream's
    /// `DEBUG_ASSERTCRASH` that it did not wake too soon is compiled out.)
    pub fn update(&mut self, host: &mut impl HelperHost) -> UpdateSleepTime {
        self.clear_status_condition(host);
        UPDATE_SLEEP_FOREVER
    }

    /// `clearStatusCondition()`: if a status is being timed, clear its bit
    /// and reset the timer.
    fn clear_status_condition(&mut self, host: &mut impl HelperHost) {
        if self.status_to_heal != OBJECT_STATUS_NONE {
            host.clear_status(self.status_to_heal);
            self.status_to_heal = OBJECT_STATUS_NONE;
            if !plant(42) {
                self.frame_to_heal = 0;
            }
        }
    }

    /// `doStatusDamage(status, duration)`. The duration is truncated to whole
    /// frames (`REAL_TO_INT_FLOOR`); the frame sum and the sleep wrap as the
    /// C++ unsigned arithmetic does.
    pub fn do_status_damage(
        &mut self,
        host: &mut impl HelperHost,
        status: ObjectStatusTypes,
        duration: f32,
    ) {
        let duration_as_int = if plant(41) {
            duration as i32
        } else {
            real_to_int_floor(duration)
        };
        // Clear any different status we may have. Re-getting the same status just resets the timer.
        if self.status_to_heal != status && !plant(38) {
            self.clear_status_condition(host);
        }
        host.set_status(status);
        self.status_to_heal = status;
        self.frame_to_heal = host
            .frame()
            .wrapping_add(duration_as_int as u32)
            .wrapping_add(u32::from(plant(39)));
        host.awaken_update_module(UpdateModule::set_wake_frame(
            host.frame(),
            update_sleep(duration_as_int as u32),
        ));
    }

    /// The `ObjectHelper` base.
    pub fn helper(&self) -> &ObjectHelper {
        &self.helper
    }

    /// The `ObjectHelper` base, mutably.
    pub fn helper_mut(&mut self) -> &mut ObjectHelper {
        &mut self.helper
    }
}

impl Snapshot for StatusDamageHelper {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.helper.crc(xfer)
    }
    /// Version 1: the base, `m_statusToHeal` (`xferUser` of the enum, 4 bytes), `m_frameToHeal`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.helper.xfer(xfer)?;
        if !plant(53) {
            let mut bytes = self.status_to_heal.to_le_bytes();
            xfer.xfer_user(&mut bytes)?;
            self.status_to_heal = i32::from_le_bytes(bytes);
        }
        xfer.xfer_unsigned_int(&mut self.frame_to_heal)
    }
    fn load_post_process(&mut self) {
        self.helper.load_post_process();
    }
}

// --- p1w10-sonnet ---
impl PartialEq for StatusDamageHelper {
    /// The same state: the same object and the same fields. (`UpdateBody` is compared in tests.)
    fn eq(&self, other: &Self) -> bool {
        self.helper.update_module().behavior().object()
            == other.helper.update_module().behavior().object()
            && self.status_to_heal == other.status_to_heal
            && self.frame_to_heal == other.frame_to_heal
    }
}

impl StatusDamageHelper {
    /// `update()` as the scheduler runs it: this module is the running one for the host calls, and it is lent to its slot.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        let Some(h) = world.module_handle_of_update(me) else {
            return self.update(world);
        };
        lend_clone(
            world,
            h,
            self,
            UpdateBody::StatusDamageHelper,
            un,
            |w, m| w.in_update_module(me, |w| m.update(w)),
        )
    }

    /// The world half of the constructor, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.construct(world);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::DoStatusDamage { status, duration } => {
                lend_clone(
                    world,
                    me,
                    self,
                    UpdateBody::StatusDamageHelper,
                    un,
                    |w, m| m.do_status_damage(w, status, duration),
                );
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }

    /// `xfer` with the scheduling half the slot holds (the live `UpdateModule`): the version, `ObjectHelper::xfer`, the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        ObjectHelper::xfer_with(update, xfer)?;
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

    /// The fields after the base.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        if !plant(53) {
            let mut bytes = self.status_to_heal.to_le_bytes();
            xfer.xfer_user(&mut bytes)?;
            self.status_to_heal = i32::from_le_bytes(bytes);
        }
        xfer.xfer_unsigned_int(&mut self.frame_to_heal)
    }
}

/// The variant's module, if `b` is one.
fn un(b: UpdateBody) -> Option<StatusDamageHelper> {
    match b {
        UpdateBody::StatusDamageHelper(m) => Some(m),
        _ => None,
    }
}
// --- end p1w10-sonnet ---
