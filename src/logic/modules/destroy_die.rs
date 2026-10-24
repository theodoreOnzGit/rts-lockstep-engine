// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/DestroyDie.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DestroyDie`: destroys its object when it dies (if the die mux applies).

use std::sync::Arc;

use super::{plant, DieHost};
// --- p1w10-sonnet ---
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
// --- end p1w10-sonnet ---
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::die_module::{DieModule, DieModuleData};

/// The data of a `DestroyDie` block: the die mux only.
pub type DestroyDieModuleData = DieModuleData;

/// `class DestroyDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct DestroyDie {
    die: DieModule,
}

impl DestroyDie {
    /// `DestroyDie(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<DieModuleData>>) -> IniResult<Self> {
        Ok(DestroyDie { die: DieModule::new(object, module_data)? })
    }

    /// `DestroyDie::onDie(damageInfo)`; `death_type` is `damageInfo->in.m_deathType`.
    pub fn on_die(&mut self, host: &mut impl DieHost, death_type: i32) {
        if !self.die.is_die_applicable(death_type, host.veterancy_level(), &host.status_bits()) && !plant(47) {
            return;
        }
        host.destroy_object();
    }

    /// The `DieModule` base.
    pub fn die_module(&self) -> &DieModule {
        &self.die
    }
}

impl Snapshot for DestroyDie {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.die.crc(xfer)
    }
    /// Version 1: the version, `DieModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.die.xfer(xfer)
    }
    fn load_post_process(&mut self) {
        self.die.load_post_process();
    }
}

// --- p1w10-sonnet ---
impl PartialEq for DestroyDie {
    /// The same object. (`UpdateBody` is compared in tests.)
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object()
    }
}

impl DestroyDie {
    /// The module's answer to one of `Object`'s calls: `onDie`. The module is lent to its slot around it, because
    /// `destroyObject` and the upgrade removal send calls to every module of the object, this one included.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(info) => {
                let death_type = info.input.death_type;
                lend_clone(world, me, self, UpdateBody::DestroyDie, un, |w, m| m.on_die(w, death_type));
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

/// The variant's module, if `b` is one.
fn un(b: UpdateBody) -> Option<DestroyDie> {
    match b {
        UpdateBody::DestroyDie(m) => Some(m),
        _ => None,
    }
}
// --- end p1w10-sonnet ---
