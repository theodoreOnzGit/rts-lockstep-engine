// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectRepulsorHelper.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectRepulsorHelper`: clears `OBJECT_STATUS_REPULSOR` when woken, then sleeps until forcibly awakened.

use std::sync::Arc;

use super::plant;
use super::object_helper::{HelperHost, ObjectHelper};
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
// --- p1w10-sonnet ---
use crate::logic::module::update_module::UpdateModule;
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
// --- end p1w10-sonnet ---
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER};
use crate::common::object_status_types::OBJECT_STATUS_REPULSOR;

/// `class ObjectRepulsorHelper : public ObjectHelper`; it has no state of its own.
#[derive(Clone, Debug)]
pub struct ObjectRepulsorHelper {
    helper: ObjectHelper,
}

impl ObjectRepulsorHelper {
    /// `ObjectRepulsorHelper(thing, modData)`: the `ObjectHelper` (asleep), then nothing.
    pub fn new(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>, host: &mut impl HelperHost) -> IniResult<Self> {
        let mut me = ObjectRepulsorHelper::unconstructed(object, module_data)?;
        me.construct(host);
        Ok(me)
    }

    /// The first half of the constructor, which touches no world.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>) -> IniResult<Self> {
        Ok(ObjectRepulsorHelper { helper: ObjectHelper::unconstructed(object, module_data, SleepyUpdatePhase::Normal)? })
    }

    /// The second half: the base's wake. The world runs it once the module is part of its object.
    pub fn construct(&mut self, host: &mut impl HelperHost) {
        self.helper.construct(host);
    }

    /// `ObjectRepulsorHelper::update()`.
    pub fn update(&mut self, host: &mut impl HelperHost) -> UpdateSleepTime {
        host.clear_status(OBJECT_STATUS_REPULSOR + i32::from(plant(34)));
        UPDATE_SLEEP_FOREVER
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

impl Snapshot for ObjectRepulsorHelper {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.helper.crc(xfer)
    }
    /// Version 1: the version, `ObjectHelper::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.helper.xfer(xfer)
    }
    fn load_post_process(&mut self) {
        self.helper.load_post_process();
    }
}

// --- p1w10-sonnet ---
impl PartialEq for ObjectRepulsorHelper {
    /// The same state: the same object and the same fields. (`UpdateBody` is compared in tests.)
    fn eq(&self, other: &Self) -> bool {
        self.helper.update_module().behavior().object() == other.helper.update_module().behavior().object()
    }
}

impl ObjectRepulsorHelper {
    /// `update()` as the scheduler runs it: this module is the running one for the host calls, and it is lent to its slot.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        let Some(h) = world.module_handle_of_update(me) else {
            return self.update(world);
        };
        lend_clone(world, h, self, UpdateBody::ObjectRepulsorHelper, un, |w, m| w.in_update_module(me, |w| m.update(w)))
    }

    /// The world half of the constructor, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.construct(world);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, _world: &mut World, _me: ModuleHandle, _call: ModuleCall) -> ModuleReply {
        ModuleReply::Unhandled // (nothing is asked of it through the interfaces: it only updates)
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
        let _ = xfer;
        Ok(())
    }
}

/// The variant's module, if `b` is one.
fn un(b: UpdateBody) -> Option<ObjectRepulsorHelper> {
    match b {
        UpdateBody::ObjectRepulsorHelper(m) => Some(m),
        _ => None,
    }
}
// --- end p1w10-sonnet ---
