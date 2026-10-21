// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code): what every module that mixes `UpgradeMux` in WITHOUT being an
// `UpgradeModule` shares (`SpyVisionUpdate`, `FXListDie`, ...), the way `modules/upgrade_body.rs` does for the upgrade modules.
//
// Translated from the behaviour of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/UpgradeModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/UpgradeModule.cpp   (class UpgradeMux)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The `UpgradeMux` calls of a module that is not an `UpgradeModule`: `isAlreadyUpgraded`, `attemptUpgrade`, `wouldUpgrade`, `resetUpgrade`,
//! `isSubObjectsUpgrade` and `forceRefreshUpgrade`, over the layer-5 [`UpgradeMux`] (the executed flag) and the mux data the module's data class
//! holds in an [`UpgradeModuleData`] (the field table macro `upgrade_module_data_field_parse!` reads into it).
//!
//! The steps of `attemptUpgrade` are run with the world in between, in `modules/upgrade_body.rs`'s order (see there for why): `wouldUpgrade`,
//! `performUpgradeFX`, `processUpgradeRemoval` (one name at a time, `removeUpgrade` before the next name is resolved), `upgradeImplementation`,
//! `setUpgradeExecuted(true)`.
//!
//! `p1w11-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::ini::IniError;
use crate::common::upgrade::UpgradeMaskType;
use crate::logic::module::upgrade_module::{find_named_upgrade, UpgradeModuleData, UpgradeMux};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::update_body::UpdateBody;

/// A module that has an `UpgradeMux` but is not an `UpgradeModule`. A compile-time contract, used as a generic bound by [`upgrade_mux_call`].
pub trait UpgradeMuxUser: Clone + Sized {
    /// The `UpgradeMux` base (the executed flag).
    fn mux(&self) -> &UpgradeMux;
    /// The `UpgradeMux` base, to change.
    fn mux_mut(&mut self) -> &mut UpgradeMux;
    /// The mux data (`m_upgradeMuxData`), inside the data object's `UpgradeModuleData`.
    fn mux_data(&self) -> &Arc<UpgradeModuleData>;
    /// `upgradeImplementation()`.
    fn upgrade_implementation(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError>;
    /// `isSubObjectsUpgrade()`.
    fn is_sub_objects_upgrade(&self) -> bool {
        false
    }
    /// The module as the variant of [`UpdateBody`] it is.
    fn mux_into_body(self) -> UpdateBody;
    /// The module back out of its variant; `None` for another variant.
    fn mux_from_body(body: UpdateBody) -> Option<Self>;
}

/// The answer of a mux user to one of `Object`'s upgrade calls; `None` for any other call (the module's own to answer).
pub fn upgrade_mux_call<M: UpgradeMuxUser>(m: &mut M, world: &mut World, me: ModuleHandle, call: ModuleCall) -> Option<ModuleReply> {
    Some(match call {
        ModuleCall::IsAlreadyUpgraded => ModuleReply::Bool(m.mux().is_already_upgraded()),
        ModuleCall::AttemptUpgrade(key_mask) => match attempt_upgrade(m, world, me, key_mask) {
            Ok(done) => ModuleReply::Bool(done),
            Err(e) => failed(world, me, e),
        },
        ModuleCall::WouldUpgrade(key_mask) => {
            let statics = world.statics.clone();
            let mut data = m.mux_data().mux_data_mut();
            match m.mux().would_upgrade(&mut data, &statics.upgrade_center, &mut world.names, key_mask) {
                Ok(would) => ModuleReply::Bool(would),
                Err(e) => {
                    drop(data);
                    failed(world, me, e)
                }
            }
        }
        ModuleCall::ResetUpgrade(key_mask) => {
            let statics = world.statics.clone();
            let data_holder = m.mux_data().clone();
            let mut data = data_holder.mux_data_mut();
            match m.mux_mut().reset_upgrade(&mut data, &statics.upgrade_center, &mut world.names, key_mask) {
                Ok(reset) => ModuleReply::Bool(reset),
                Err(e) => {
                    drop(data);
                    failed(world, me, e)
                }
            }
        }
        ModuleCall::IsSubObjectsUpgrade => ModuleReply::Bool(m.is_sub_objects_upgrade()),
        ModuleCall::ForceRefreshUpgrade => {
            // "*** DANGER! DANGER! Don't use this, unless you are forcing an already made upgrade to refresh!!"
            if m.mux().is_already_upgraded() {
                if let Err(e) = m.upgrade_implementation(world, me) {
                    return Some(failed(world, me, e));
                }
            }
            ModuleReply::Done
        }
        _ => return None,
    })
}

/// `UpgradeMux::attemptUpgrade(keyMask)` with `giveSelfUpgrade()` inline, step by step.
fn attempt_upgrade<M: UpgradeMuxUser>(m: &mut M, world: &mut World, me: ModuleHandle, key_mask: UpgradeMaskType) -> Result<bool, IniError> {
    let statics = world.statics.clone();
    let holder = m.mux_data().clone();
    // "If I have an activation condition, and I haven't activated, and this key matches my condition."
    {
        let mut data = holder.mux_data_mut();
        if !m.mux().would_upgrade(&mut data, &statics.upgrade_center, &mut world.names, key_mask)? {
            return Ok(false);
        }
    }
    // performUpgradeFX()
    let fx = holder.mux_data().fx_list_upgrade.clone();
    if let Some(fx) = fx {
        world.emit(ObjectClientEvent::FxListDoFxObj { fx, object_id: me.object });
    }
    // processUpgradeRemoval(): "need to execute removals first, to prevent both being on for a moment"
    let count = holder.mux_data().removal_upgrade_names.len();
    for i in 0..count {
        let name = holder.mux_data().removal_upgrade_names[i].clone();
        let t = find_named_upgrade(&statics.upgrade_center, &mut world.names, &name)?;
        let mask = t.upgrade_mask();
        if let Some(trace) = &mut world.cuts.trace {
            trace.push(format!("remove {}", String::from_utf8_lossy(t.upgrade_name())));
        }
        lend(m, world, me, |w| w.remove_upgrade(me.object, &mask));
    }
    m.upgrade_implementation(world, me)?;
    m.mux_mut().set_upgrade_executed(!crate::logic::modules::plant(871));
    Ok(true)
}

/// Runs `f` with the module lent to its slot ([`World::lend_body`]), so a call that reaches it from inside `f` runs at once on its live state, and
/// takes the state back afterwards.
pub fn lend<M: UpgradeMuxUser, R>(m: &mut M, world: &mut World, me: ModuleHandle, f: impl FnOnce(&mut World) -> R) -> R {
    let mine = std::mem::replace(m, m.clone());
    let (r, back) = world.lend_body(me, mine.mux_into_body(), f);
    if let Some(b) = back.and_then(M::mux_from_body) {
        *m = b;
    }
    r
}

/// Records an upstream throw and answers `FALSE` (see `modules/upgrade_body.rs`).
fn failed(world: &mut World, me: ModuleHandle, e: IniError) -> ModuleReply {
    world.cuts.module_errors.push((me, e));
    ModuleReply::Bool(false)
}
