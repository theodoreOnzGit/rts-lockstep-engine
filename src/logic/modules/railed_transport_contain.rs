// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RailedTransportContain.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Contain/RailedTransportContain.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   RailedTransportContain.cpp: Colin Day, August 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `RailedTransportContain`: the container of a train that runs on rails between docks. Riders may leave only while its dock is open, and the dock
//! opens again once the last rider is out.
//!
//! # Design
//!
//! As every class derived from `TransportContain` (see that module): the module holds its `TransportContain` by value as `tc`, overrides the
//! [`TransportContainVirtuals`] hooks of `onRemoving` and `exitObjectViaDoor` and `isSpecificRiderFreeToExit`, and takes the rest from
//! `derived_transport_contain_glue!`. The object's dock is `World::get_dock_update_interface`, asked with [`ModuleCall::DockIsOpen`] and
//! [`ModuleCall::DockSetOpen`].
//!
//! # Upstream facts kept
//!
//! - **`exitObjectViaDoor` never unloads anyone.** It calls `getRailedTransportDockUpdateInterface()` unqualified. The file defines a static helper of
//!   that name taking an `Object *`, but inside a member function, name lookup finds the class's own member first: `BehaviorModule`'s virtual
//!   `getRailedTransportDockUpdateInterface()`, which a contain module does not override, so it returns `NULL` and the function returns before
//!   `unloadSingleObject`. The static helper is never called. The translation keeps the early return (the trains still unload through
//!   `RailedTransportDockUpdate::unloadAll`, which is not this module's).
//! - `exitObjectByBudding` is a no-op, as `OpenContain`'s.
//! - `isSpecificRiderFreeToExit` replaces `TransportContain`'s entirely ("we're not extending the base class cause *we* handle it all"): free unless
//!   there is a dock and it is closed.
//! - The data is `TransportContainModuleData` (the class declares no data class of its own; `MAKE_STANDARD_MODULE_MACRO` with the inherited
//!   `friend_newModuleData`).
//!
//! `p1w13-sonnet`, batch R3, 2026-10-22.

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::{Ini, IniResult};
use crate::common::module::ModuleDataKind;
use crate::logic::module::update_module::ExitDoorType;
use crate::logic::modules::open_contain::{out, OpenContainVirtuals};
use crate::logic::modules::transport_contain::{
    derived_transport_contain_glue, tc_on_removing, transport_open_contain_virtuals, TransportContain, TransportContainModuleData, TransportContainVirtuals,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle};

/// The factory's `NewModuleDataProc` for `RailedTransportContain`: `TransportContain`'s data.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::RailedTransportContain(Arc::new(TransportContainModuleData::friend_new_module_data(ini)?)))
}

/// `class RailedTransportContain : public TransportContain`.
#[derive(Clone, Debug, PartialEq)]
pub struct RailedTransportContain {
    tc: TransportContain,
}

derived_transport_contain_glue!(RailedTransportContain, RailedTransportContain);
transport_open_contain_virtuals!(RailedTransportContain, RailedTransportContain);

impl TransportContainVirtuals for RailedTransportContain {
    fn tc(&self) -> &TransportContain {
        &self.tc
    }
    fn tc_mut(&mut self) -> &mut TransportContain {
        &mut self.tc
    }

    /// `RailedTransportContain::onRemoving(obj)`: the base's, then "once we have removed all our contents we are "open" for docking again".
    fn t_on_removing(&mut self, world: &mut World, me: ModuleHandle, rider: ObjectId) {
        // extend functionality
        tc_on_removing(self, world, me, rider);

        if self.get_contain_count(world) == 0 && !crate::logic::modules::plant(1443) {
            let us = me.object;
            out(self, world, me, |w| {
                if let Some(dock) = w.get_dock_update_interface(us) {
                    w.module_call(us, dock, ModuleCall::DockSetOpen(!crate::logic::modules::plant(1444)));
                }
            });
        }
    }

    /// `RailedTransportContain::isSpecificRiderFreeToExit(obj)`: "if we are in transit we cannot exit, when we're in transit, the dock is closed".
    fn is_specific_rider_free_to_exit(&mut self, world: &mut World, me: ModuleHandle, _obj: ObjectId) -> bool {
        let us = me.object;
        let closed = out(self, world, me, |w| match w.get_dock_update_interface(us) {
            Some(dock) => !w.module_call(us, dock, ModuleCall::DockIsOpen).as_bool(),
            None => false,
        });
        if closed != crate::logic::modules::plant(1445) {
            return false;
        }
        // "we can now exit, note we're not extending the base class cause *we* handle it all"
        true
    }

    /// `RailedTransportContain::exitObjectViaDoor(newObj, exitDoor)`: returns at once (see the module docs: the railed dock interface it asks for is
    /// the contain module's own, which is `NULL`).
    fn t_exit_object_via_door(&mut self, world: &mut World, me: ModuleHandle, exit_obj: ObjectId, exit_door: ExitDoorType) {
        if crate::logic::modules::plant(1446) {
            // the static helper the comment intends: the base's exit, as if there were a dock to hand the rider to
            crate::logic::modules::open_contain::base_exit_object_via_door(self, world, me, exit_obj, exit_door);
        }
    }
}
