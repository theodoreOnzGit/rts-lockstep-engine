// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/PreorderCreate.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.


//! `PreorderCreate`: when a building is finished, the model condition
//! `MODELCONDITION_PREORDER` follows whether the controlling player pre-ordered
//! the game. Upstream's `onCreate` is empty and `onBuildComplete` neither checks
//! `shouldDoOnBuildComplete()` nor calls the base (no "extend" line), so the
//! base's flag is left as it is.
//!
//! Upstream dereferences `getControllingPlayer()` without a test: an object
//! with no controlling player is an undefined input, returned as
//! [`CreateError::NoControllingPlayer`].
//!
//! Verification: `tests/differential_create_modules.rs` (the unchanged
//! `PreorderCreate.cpp` against the stand-ins of `differential/shim_creates`).

// --- p1w10-sonnet ---
use crate::logic::modules::create_host::{create_module_eq, create_module_snapshot, report_create_error};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
// --- end p1w10-sonnet ---
use std::sync::Arc;

use crate::common::model_state::MODELCONDITION_PREORDER;
use crate::common::ini::IniResult;
use crate::logic::module::create_module::{CreateModule, CreateModuleData};
use crate::logic::modules::create_host::{CreateError, CreateHost};

/// `class PreorderCreate : public CreateModule`. It has no data of its own.
#[derive(Clone, Debug)]
pub struct PreorderCreate {
    /// The `CreateModule` base.
    create: CreateModule,
}

impl PreorderCreate {
    /// `PreorderCreate(thing, moduleData)`.
    pub fn new(object: crate::common::game_type::ObjectId, module_data: Option<Arc<CreateModuleData>>) -> IniResult<Self> {
        Ok(PreorderCreate { create: CreateModule::new(object, module_data)? })
    }

    /// The `CreateModule` base.
    pub fn create(&self) -> &CreateModule {
        &self.create
    }

    /// `onCreate()`: empty upstream.
    pub fn on_create(&mut self) {}

    /// `onBuildComplete()`: the preorder model condition follows the
    /// controlling player's `didPlayerPreorder()`. No base call, no
    /// `shouldDoOnBuildComplete()` test (both absent upstream).
    pub fn on_build_complete<H: CreateHost>(&mut self, host: &mut H) -> Result<(), CreateError> {
        let obj = self.create.behavior().object();
        let player = host.controlling_player(obj).ok_or(CreateError::NoControllingPlayer)?;
        if host.player_did_preorder(player) {
            host.set_model_condition_state(obj, MODELCONDITION_PREORDER);
        } else {
            host.clear_model_condition_state(obj, MODELCONDITION_PREORDER);
        }
        Ok(())
    }
}

// --- p1w10-sonnet ---
create_module_snapshot!(PreorderCreate);
create_module_eq!(PreorderCreate);

impl PreorderCreate {
    /// The module's answer to one of `Object`'s calls (`onCreate` and `onBuildComplete` of the create interface).
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let _ = me;
        match call {
            ModuleCall::OnCreate => {
                self.on_create();
                ModuleReply::Done
            }
            ModuleCall::OnBuildComplete => {
                let r = self.on_build_complete(world);
                report_create_error(world, me, r);
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

    #[test]
    fn preorder_sets_or_clears_the_condition_from_the_controller() {
        let mut m = PreorderCreate::new(7, Some(Arc::new(CreateModuleData::new()))).unwrap();
        let mut h = Recorder { controller: Some(1), preorder: true, ..Default::default() };
        m.on_build_complete(&mut h).unwrap();
        h.preorder = false;
        m.on_build_complete(&mut h).unwrap();
        assert_eq!(h.log, vec![format!("set 7 {MODELCONDITION_PREORDER}"), format!("clear 7 {MODELCONDITION_PREORDER}")]);
        // no base call: the flag is still set
        assert!(m.create().should_do_on_build_complete());
    }

    #[test]
    fn no_controlling_player_is_an_undefined_input() {
        let mut m = PreorderCreate::new(7, Some(Arc::new(CreateModuleData::new()))).unwrap();
        let mut h = Recorder::default();
        assert_eq!(m.on_build_complete(&mut h), Err(CreateError::NoControllingPlayer));
        assert!(h.log.is_empty());
    }
}
