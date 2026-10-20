// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/SupplyCenterCreate.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.


//! `SupplyCenterCreate`: when a supply center is finished building, every
//! player's resource gathering manager records it. `onCreate` is empty.
//!
//! Upstream order: `shouldDoOnBuildComplete()`, the base's `onBuildComplete`
//! (clears the flag), `ThePlayerList == NULL` returns, then the per-player
//! loop from the last player to the first, through the host
//! ([`CreateHost::resource_add_supply_center`]).

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::IniResult;
use crate::logic::module::create_module::{CreateModule, CreateModuleData};
use crate::logic::modules::create_host::CreateHost;

/// `class SupplyCenterCreate : public CreateModule`. It has no data of its own.
#[derive(Clone, Debug)]
pub struct SupplyCenterCreate {
    /// The `CreateModule` base.
    create: CreateModule,
}

impl SupplyCenterCreate {
    /// `SupplyCenterCreate(thing, moduleData)`.
    pub fn new(object: ObjectId, module_data: Option<Arc<CreateModuleData>>) -> IniResult<Self> {
        Ok(SupplyCenterCreate { create: CreateModule::new(object, module_data)? })
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
        let obj = self.create.behavior().object();
        let Some(count) = host.player_count() else {
            return;
        };
        for index in (0..count).rev() {
            host.resource_add_supply_center(index, obj);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::modules::create_host::test_support::Recorder;

    #[test]
    fn centers_are_recorded_last_player_first_after_the_base() {
        let mut m = SupplyCenterCreate::new(9, Some(Arc::new(CreateModuleData::new()))).unwrap();
        let mut h = Recorder { players: Some(3), ..Default::default() };
        m.on_build_complete(&mut h);
        assert_eq!(h.log, vec!["center 2 9", "center 1 9", "center 0 9"]);
        assert!(!m.create().should_do_on_build_complete());
        h.log.clear();
        m.on_build_complete(&mut h);
        assert!(h.log.is_empty());
    }

    #[test]
    fn no_player_list_records_nothing_but_still_clears_the_flag() {
        let mut m = SupplyCenterCreate::new(9, Some(Arc::new(CreateModuleData::new()))).unwrap();
        let mut h = Recorder::default();
        m.on_build_complete(&mut h);
        assert!(h.log.is_empty());
        assert!(!m.create().should_do_on_build_complete());
    }
}
