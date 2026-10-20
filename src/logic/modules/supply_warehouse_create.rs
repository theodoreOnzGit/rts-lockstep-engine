// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/SupplyWarehouseCreate.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.


//! `SupplyWarehouseCreate`: when a supply warehouse is created, every player's
//! resource gathering manager records it. Warehouses are never built, so there
//! is no `onBuildComplete` work (upstream: none).
//!
//! The per-player loop runs from the last player down to the first, as
//! upstream does (`for (playerIndex = count - 1; playerIndex >= 0; ...)`). Its
//! body is the host's ([`CreateHost::resource_add_supply_warehouse`]), which
//! performs upstream's two `NULL` tests.

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::IniResult;
use crate::logic::module::create_module::{CreateModule, CreateModuleData};
use crate::logic::modules::create_host::CreateHost;

/// `class SupplyWarehouseCreate : public CreateModule`. It has no data of its own.
#[derive(Clone, Debug)]
pub struct SupplyWarehouseCreate {
    /// The `CreateModule` base.
    create: CreateModule,
}

impl SupplyWarehouseCreate {
    /// `SupplyWarehouseCreate(thing, moduleData)`.
    pub fn new(object: ObjectId, module_data: Option<Arc<CreateModuleData>>) -> IniResult<Self> {
        Ok(SupplyWarehouseCreate { create: CreateModule::new(object, module_data)? })
    }

    /// The `CreateModule` base.
    pub fn create(&self) -> &CreateModule {
        &self.create
    }

    /// `onCreate()`: `ThePlayerList == NULL` returns; otherwise every player,
    /// from the last to the first, records this object as a supply warehouse.
    pub fn on_create<H: CreateHost>(&mut self, host: &mut H) {
        let obj = self.create.behavior().object();
        let Some(count) = host.player_count() else {
            return;
        };
        for index in (0..count).rev() {
            host.resource_add_supply_warehouse(index, obj);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::modules::create_host::test_support::Recorder;

    #[test]
    fn warehouses_are_recorded_on_create_for_every_player() {
        let mut m = SupplyWarehouseCreate::new(4, Some(Arc::new(CreateModuleData::new()))).unwrap();
        let mut h = Recorder { players: Some(2), ..Default::default() };
        m.on_create(&mut h);
        assert_eq!(h.log, vec!["warehouse 1 4", "warehouse 0 4"]);
        h.players = None;
        h.log.clear();
        m.on_create(&mut h);
        assert!(h.log.is_empty());
    }
}
