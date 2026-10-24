// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/SpecialPowerCreate.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpecialPowerCreate`: when a building is finished, every special power
//! module of the object starts its countdown (`onSpecialPowerCreation()`). The
//! loop itself is the host's ([`CreateHost::special_power_creation`]): it walks
//! the object's behaviour modules, which the host owns.
//!
//! Upstream order: `shouldDoOnBuildComplete()`, then the base's
//! `onBuildComplete` (clears the flag), then the loop.

// --- p1w10-sonnet ---
use crate::logic::modules::create_host::{create_module_eq, create_module_snapshot};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
// --- end p1w10-sonnet ---
use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::IniResult;
use crate::logic::module::create_module::{CreateModule, CreateModuleData};
use crate::logic::modules::create_host::CreateHost;

/// `class SpecialPowerCreate : public CreateModule`. It has no data of its own.
#[derive(Clone, Debug)]
pub struct SpecialPowerCreate {
    /// The `CreateModule` base.
    create: CreateModule,
}

impl SpecialPowerCreate {
    /// `SpecialPowerCreate(thing, moduleData)`.
    pub fn new(object: ObjectId, module_data: Option<Arc<CreateModuleData>>) -> IniResult<Self> {
        Ok(SpecialPowerCreate {
            create: CreateModule::new(object, module_data)?,
        })
    }

    /// The `CreateModule` base.
    pub fn create(&self) -> &CreateModule {
        &self.create
    }

    /// `onCreate()`: empty upstream.
    pub fn on_create(&mut self) {}

    /// `onBuildComplete()`.
    pub fn on_build_complete<H: CreateHost>(&mut self, host: &mut H) {
        if !self.create.should_do_on_build_complete() {
            return;
        }
        self.create.on_build_complete(); // extend
        host.special_power_creation(self.create.behavior().object());
    }
}

// --- p1w10-sonnet ---
create_module_snapshot!(SpecialPowerCreate);
create_module_eq!(SpecialPowerCreate);

impl SpecialPowerCreate {
    /// The module's answer to one of `Object`'s calls (`onCreate` and `onBuildComplete` of the create interface).
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        let _ = me;
        match call {
            ModuleCall::OnCreate => {
                self.on_create();
                ModuleReply::Done
            }
            ModuleCall::OnBuildComplete => {
                self.on_build_complete(world);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}
// --- end p1w10-sonnet ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::modules::create_host::test_support::Recorder;

    fn module() -> SpecialPowerCreate {
        SpecialPowerCreate::new(3, Some(Arc::new(CreateModuleData::new()))).unwrap()
    }

    #[test]
    fn countdown_starts_once_when_the_base_allows() {
        let mut m = module();
        let mut h = Recorder::default();
        m.on_build_complete(&mut h);
        assert_eq!(h.log, vec!["sp 3".to_string()]);
        m.on_build_complete(&mut h);
        assert_eq!(h.log.len(), 1, "the second build-complete does nothing");
    }
}
