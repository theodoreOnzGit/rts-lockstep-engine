// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/InternetHackContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/InternetHackContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `InternetHackContain`: China's Internet Center. A [`TransportContain`] whose riders, once inside, are told to hack the internet.
//!
//! # Design
//!
//! As every class derived from `TransportContain` (see that module): the module holds its `TransportContain` by value as `tc`, overrides the
//! [`TransportContainVirtuals`] hook of `onContaining`, and takes the rest from `derived_transport_contain_glue!`.
//!
//! # Upstream facts kept
//!
//! - `InternetHackContainModuleData` adds no field (its table is empty) and declares a `parseRiderInfo` it never defines or uses: the data is
//!   `TransportContainModuleData`'s ([`InternetHackContainModuleData`] is that type).
//! - `xfer` writes its version (1) and the base's; no state of its own.
//!
//! `p1w13-sonnet`, batch R3, 2026-10-22.

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::{Ini, IniResult};
use crate::common::module::ModuleDataKind;
use crate::logic::modules::open_contain::{out, CMD_FROM_AI};

/// `CMD_FROM_PLAYER` (`CommandSourceType`), for plant 1442.
const CMD_FROM_PLAYER: i32 = 0;
use crate::logic::modules::transport_contain::{
    derived_transport_contain_glue, tc_on_containing, transport_open_contain_virtuals, TransportContain, TransportContainModuleData,
    TransportContainVirtuals,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle};

/// `InternetHackContainModuleData`: `TransportContainModuleData` with an empty table of its own.
pub type InternetHackContainModuleData = TransportContainModuleData;

/// The factory's `NewModuleDataProc` for `InternetHackContain`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::InternetHackContain(Arc::new(TransportContainModuleData::friend_new_module_data(ini)?)))
}

/// `class InternetHackContain : public TransportContain`.
#[derive(Clone, Debug, PartialEq)]
pub struct InternetHackContain {
    tc: TransportContain,
}

derived_transport_contain_glue!(InternetHackContain, InternetHackContain);
transport_open_contain_virtuals!(InternetHackContain, InternetHackContain);

impl TransportContainVirtuals for InternetHackContain {
    fn tc(&self) -> &TransportContain {
        &self.tc
    }
    fn tc_mut(&mut self) -> &mut TransportContain {
        &mut self.tc
    }
    /// `InternetHackContain::onContaining(rider, wasSelected)`: the base's, then the rider's AI is told to hack.
    fn t_on_containing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId, was_selected: bool) {
        tc_on_containing(self, world, me, rider, was_selected);

        if crate::logic::modules::plant(1441) {
            return;
        }
        if let Some(ai) = world.objects.find(rider).and_then(|o| o.ai) {
            let command_source = if crate::logic::modules::plant(1442) { CMD_FROM_PLAYER } else { CMD_FROM_AI };
            out(self, world, me, |w| w.module_call(rider, ai, ModuleCall::AiHackInternet { command_source }));
        }
    }
}
