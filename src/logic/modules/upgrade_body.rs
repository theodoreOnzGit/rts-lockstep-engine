// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/UpgradeModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/UpgradeModule.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! What every concrete upgrade module shares: the answers to `Object`'s
//! upgrade calls ([`upgrade_module_call`]), driven by the module's own
//! `upgradeImplementation` ([`UpgradeBehavior`]).
//!
//! # Why the mux steps are run here and not by `UpgradeMux::attempt_upgrade`
//!
//! The layer-5 [`UpgradeMux`](crate::logic::module::upgrade_module::UpgradeMux)
//! runs `attemptUpgrade` -> `giveSelfUpgrade` in one call that borrows the
//! upgrade store and the name-key generator for its whole length. A concrete
//! module's `upgradeImplementation` needs the world, and the world owns the
//! generator (`world.names`), so the two cannot be borrowed together. This
//! file runs the same steps in the same order, with the world in between:
//!
//! 1. `wouldUpgrade(keyMask)` (the layer-5 function, unchanged);
//! 2. `performUpgradeFX()`: [`ObjectClientEvent::FxListDoFxObj`];
//! 3. `processUpgradeRemoval()`: one `RemovesUpgrades` name at a time,
//!    resolved, then `obj->removeUpgrade(template)` **before the next name is
//!    resolved** (resolving can assign a name key, and `removeUpgrade` resets
//!    other modules, which can assign keys too; the order of key assignment is
//!    observable, so it is kept);
//! 4. `upgradeImplementation()`;
//! 5. `setUpgradeExecuted(true)`.
//!
//! The mux data's lock is never held across a call into the world (upstream
//! has no lock; a nested call reaching the same module data would otherwise
//! wait on itself).
//!
//! # Re-entrancy
//!
//! `removeUpgrade` calls `resetUpgrade` on every upgrade module of the
//! object, **this one included**. The module is checked out while it runs,
//! so the call is made inside [`lend`], which puts the module back in its
//! slot for the duration (`World::lend_body`), as upstream's plain recursion
//! reaches it. A concrete module's `upgrade_implementation` must do the same
//! around any call that can reach its own object's modules.
//!
//! # Errors
//!
//! Upstream throws `INI_INVALID_DATA` from inside the game loop when a mux
//! name is not an upgrade (`getUpgradeActivationMasks`,
//! `muxDataProcessUpgradeRemoval`); nothing catches it during a game. Here the
//! error is recorded in `world.cuts.module_errors` with the module, and the
//! call answers as if it had not run ([`ModuleReply::Bool`]`(false)`). A
//! shipped game never takes this path (its INI is valid); a test that does
//! sees the error instead of a crash.
//!
//! Lead reference module for layer 9, 2026-10-20.

use crate::common::ini::IniError;
use crate::common::upgrade::UpgradeMaskType;
use crate::logic::module::upgrade_module::{find_named_upgrade, UpgradeModule};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::update_body::UpdateBody;

/// A concrete upgrade module (a `class X : public UpgradeModule`). A
/// compile-time contract, used as a generic bound by [`upgrade_module_call`].
pub trait UpgradeBehavior: Clone + Sized {
    /// The `UpgradeModule` base (with the `UpgradeMux` base inside it).
    fn base(&self) -> &UpgradeModule;
    /// The `UpgradeModule` base, to change.
    fn base_mut(&mut self) -> &mut UpgradeModule;
    /// `upgradeImplementation()`: "here's the actual work of upgrading".
    fn upgrade_implementation(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
    ) -> Result<(), IniError>;
    /// `isSubObjectsUpgrade()` (pure virtual upstream).
    fn is_sub_objects_upgrade(&self) -> bool;
    // --- p1w11-sonnet ---
    /// `onDelete()` (`Object::onDestroy`'s loop): empty for a module that does not override it.
    fn on_delete(&mut self, _world: &mut World, _me: ModuleHandle) {}
    /// `onCapture(oldOwner, newOwner)` (`Object::onCapture`'s loop), the owners as player indices (a negative index is upstream's
    /// `NULL`): empty for a module that does not override it.
    fn on_capture(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        _old_owner: i32,
        _new_owner: i32,
    ) {
    }
    /// The part of `loadPostProcess()` that needs the world (see [`ModuleCall::LoadPostProcess`]): empty for a module that has none. The
    /// base's `UpgradeModule::loadPostProcess()` is the world-free `Snapshot::load_post_process`, which runs first.
    fn load_post_process_world(&mut self, _world: &mut World, _me: ModuleHandle) {}
    // --- end p1w11-sonnet ---
    /// The module as the variant of [`UpdateBody`] it is.
    fn into_body(self) -> UpdateBody;
    /// The module back out of its variant; `None` for another variant.
    fn from_body(body: UpdateBody) -> Option<Self>;
}

/// The answer of an upgrade module to one of `Object`'s calls. The calls an
/// upgrade module has no behaviour for (the `BehaviorModule` virtuals, which
/// are empty for it) answer [`ModuleReply::Unhandled`].
pub fn upgrade_module_call<M: UpgradeBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    call: ModuleCall,
) -> ModuleReply {
    match call {
        ModuleCall::IsAlreadyUpgraded => ModuleReply::Bool(m.base().is_already_upgraded()),
        ModuleCall::AttemptUpgrade(key_mask) => match attempt_upgrade(m, world, me, key_mask) {
            Ok(done) => ModuleReply::Bool(done),
            Err(e) => failed(world, me, e),
        },
        ModuleCall::WouldUpgrade(key_mask) => {
            let statics = world.statics.clone();
            match m
                .base()
                .would_upgrade(&statics.upgrade_center, &mut world.names, key_mask)
            {
                Ok(would) => ModuleReply::Bool(would),
                Err(e) => failed(world, me, e),
            }
        }
        ModuleCall::ResetUpgrade(key_mask) => {
            let statics = world.statics.clone();
            match m
                .base_mut()
                .reset_upgrade(&statics.upgrade_center, &mut world.names, key_mask)
            {
                Ok(reset) => ModuleReply::Bool(reset),
                Err(e) => failed(world, me, e),
            }
        }
        ModuleCall::IsSubObjectsUpgrade => ModuleReply::Bool(m.is_sub_objects_upgrade()),
        ModuleCall::ForceRefreshUpgrade => {
            // "*** DANGER! DANGER! Don't use this, unless you are forcing an already made upgrade to refresh!!"
            if m.base().is_already_upgraded() {
                if let Err(e) = m.upgrade_implementation(world, me) {
                    return failed(world, me, e);
                }
            }
            ModuleReply::Done
        }
        // --- p1w11-sonnet ---
        ModuleCall::OnDelete => {
            m.on_delete(world, me);
            ModuleReply::Done
        }
        ModuleCall::OnCapture {
            old_owner,
            new_owner,
        } => {
            m.on_capture(world, me, old_owner, new_owner);
            ModuleReply::Done
        }
        ModuleCall::LoadPostProcess => {
            m.load_post_process_world(world, me);
            ModuleReply::Done
        }
        // --- end p1w11-sonnet ---
        _ => ModuleReply::Unhandled,
    }
}

/// `UpgradeMux::attemptUpgrade(keyMask)` with `giveSelfUpgrade()` inline,
/// step by step (see the module docs).
fn attempt_upgrade<M: UpgradeBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    key_mask: UpgradeMaskType,
) -> Result<bool, IniError> {
    let statics = world.statics.clone();
    // "If I have an activation condition, and I haven't activated, and this key matches my condition."
    if !m
        .base()
        .would_upgrade(&statics.upgrade_center, &mut world.names, key_mask)?
    {
        return Ok(false);
    }
    // performUpgradeFX()
    let fx = m
        .base()
        .upgrade_module_data()
        .mux_data()
        .fx_list_upgrade
        .clone();
    if let Some(fx) = fx {
        world.emit(ObjectClientEvent::FxListDoFxObj {
            fx,
            object_id: me.object,
        });
    }
    // processUpgradeRemoval(): "need to execute removals first, to prevent both being on for a moment"
    let count = m
        .base()
        .upgrade_module_data()
        .mux_data()
        .removal_upgrade_names
        .len();
    for i in 0..count {
        let name = m
            .base()
            .upgrade_module_data()
            .mux_data()
            .removal_upgrade_names[i]
            .clone();
        let t = find_named_upgrade(&statics.upgrade_center, &mut world.names, &name)?;
        let mask = t.upgrade_mask();
        if let Some(trace) = &mut world.cuts.trace {
            trace.push(format!(
                "remove {}",
                String::from_utf8_lossy(t.upgrade_name())
            ));
        }
        lend(m, world, me, |w| w.remove_upgrade(me.object, &mask));
    }
    m.upgrade_implementation(world, me)?;
    m.base_mut().mux_mut().set_upgrade_executed(true);
    Ok(true)
}

/// Runs `f` with the module lent to its slot ([`World::lend_body`]), so a
/// call that reaches it from inside `f` runs at once on its live state, and
/// takes the state back afterwards.
pub fn lend<M: UpgradeBehavior, R>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    f: impl FnOnce(&mut World) -> R,
) -> R {
    let mine = std::mem::replace(m, m.clone());
    let (r, back) = world.lend_body(me, mine.into_body(), f);
    if let Some(b) = back.and_then(M::from_body) {
        *m = b;
    }
    r
}

/// Records an upstream throw (see the module docs) and answers `FALSE`.
fn failed(world: &mut World, me: ModuleHandle, e: IniError) -> ModuleReply {
    world.cuts.module_errors.push((me, e));
    ModuleReply::Bool(false)
}
