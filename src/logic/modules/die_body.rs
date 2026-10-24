// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code): what every concrete die module shares, the way
// `modules/upgrade_body.rs` does for the upgrade modules.

//! What every concrete die module (a `class X : public DieModule`) shares: the answer to `Object`'s `onDie` call
//! ([`die_module_call`]) and the `isDieApplicable(damageInfo)` test every `onDie` begins with ([`DieBehavior::is_die_applicable`]).
//!
//! The module is lent to its slot around `onDie` ([`lend_clone`]): a die module's effects (`destroyObject`, an object creation list's new
//! object, a script notification) can reach the object's modules, this one included, and upstream's plain recursion delivers the call at once on
//! the live state.
//!
//! `p1w11-sonnet`, 2026-10-21.

use crate::logic::damage::DamageInfo;
use crate::logic::module::die_module::DieModule;
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;

/// A concrete die module. A compile-time contract, used as a generic bound by [`die_module_call`].
pub trait DieBehavior: Clone + PartialEq + Sized {
    /// The `DieModule` base.
    fn die(&self) -> &DieModule;
    /// `onDie(damageInfo)`: each module begins with `if (!isDieApplicable(damageInfo)) return;`.
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo);
    /// The module as the variant of [`UpdateBody`] it is.
    fn into_body(self) -> UpdateBody;
    /// The module back out of its variant; `None` for another variant.
    fn from_body(body: UpdateBody) -> Option<Self>;
    /// A call other than `onDie` (a die module's own interface, such as `setCreator`): [`ModuleReply::Unhandled`] unless it overrides this.
    fn other_call(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        _call: ModuleCall,
    ) -> ModuleReply {
        ModuleReply::Unhandled
    }
    /// `isDieApplicable(damageInfo)` (protected upstream): the die mux against the damage's death type and the object's veterancy level
    /// and status bits.
    fn is_die_applicable(&self, world: &World, me: ModuleHandle, info: &DamageInfo) -> bool {
        match world.objects.find(me.object) {
            Some(o) => self.die().is_die_applicable(
                info.input.death_type,
                o.get_veterancy_level() as i32,
                &o.status,
            ),
            None => false,
        }
    }
}

/// The answer of a die module to one of `Object`'s calls.
pub fn die_module_call<M: DieBehavior>(
    m: &mut M,
    world: &mut World,
    me: ModuleHandle,
    call: ModuleCall,
) -> ModuleReply {
    match call {
        ModuleCall::OnDie(info) => {
            lend_clone(world, me, m, M::into_body, M::from_body, |w, m| {
                m.on_die(w, me, &info)
            });
            ModuleReply::Done
        }
        other => m.other_call(world, me, other),
    }
}
