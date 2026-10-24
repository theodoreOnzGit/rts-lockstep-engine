// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpecialPowerModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/SpecialPowerModule.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The concrete `SpecialPowerModule` itself: the base class upstream makes only through its derived classes (it is never registered with the module
//! factory). A test makes one directly; the translated powers wrap the same state and use the same functions
//! ([`special_power_body`](crate::logic::modules::special_power_body)).
//!
//! `p1w11-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::ini::{IniError, IniErrorKind, IniResult};
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult};
use crate::logic::module::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
use crate::logic::modules::special_power_body::{self, special_power_module_call, SpecialPowerBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;

/// `class SpecialPowerModule : public BehaviorModule, public SpecialPowerModuleInterface`, as the object holds it.
#[derive(Clone, Debug)]
pub struct SpecialPowerModuleImpl {
    /// The base: the state and the data.
    power: SpecialPowerModule,
}

impl PartialEq for SpecialPowerModuleImpl {
    /// The same object, the same data object and the same state (the real by bits).
    fn eq(&self, other: &Self) -> bool {
        self.power.behavior().object() == other.power.behavior().object()
            && Arc::ptr_eq(self.power.special_power_module_data(), other.power.special_power_module_data())
            && self.power.get_available_on_frame() == other.power.get_available_on_frame()
            && self.power.paused_count() == other.power.paused_count()
            && self.power.paused_on_frame() == other.power.paused_on_frame()
            && self.power.paused_percent().to_bits() == other.power.paused_percent().to_bits()
    }
}

impl SpecialPowerModuleImpl {
    /// `SpecialPowerModule(thing, moduleData)`, the part that touches no world; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<SpecialPowerModuleData>>) -> IniResult<Self> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        Ok(SpecialPowerModuleImpl { power: SpecialPowerModule::new(object, Some(data))? })
    }

    /// The module's own state as words, for the tests that compare it with upstream's.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.power.get_available_on_frame(), self.power.paused_count() as u32, self.power.paused_on_frame(), self.power.paused_percent().to_bits()]
    }

    /// The world half of the constructor, run once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        if let Err(e) = special_power_body::construct(self, world, me) {
            world.cuts.module_errors.push((me, e));
        }
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        special_power_module_call(self, world, me, call)
    }
}

impl SpecialPowerBehavior for SpecialPowerModuleImpl {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpecialPowerModule(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpecialPowerModule(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SpecialPowerModuleImpl {
    /// `crc`: `SpecialPowerModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.power.crc(xfer)
    }
    /// `xfer` (version 1): the version, `BehaviorModule::xfer`, the four fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.power.xfer(xfer)
    }
    /// `loadPostProcess`: `SpecialPowerModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.power.load_post_process();
    }
}
