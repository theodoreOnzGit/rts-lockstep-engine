// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code): what every concrete update module shares, the way
// `modules/upgrade_body.rs` and `modules/die_body.rs` do for the upgrade and the die modules.

//! What every concrete update module (a `class X : public UpdateModule`) shares: the scheduler's call of `update()`
//! ([`world_update`]), the world half of the constructor ([`world_construct`]), the three snapshot functions over the scheduling half the
//! slot holds ([`xfer_with`], [`crc_with`], [`load_post_process_with`]) and `setWakeFrame` ([`set_wake_frame`]).
//!
//! The module is lent to its slot around `update()` ([`world_update`]): an update module's effects (`destroyObject`, a new object, an
//! interface call to a sibling) can reach the object's modules, this one included, and upstream's plain recursion delivers the call at
//! once on the live state. The lend compares the module before and after (it implements `PartialEq` over its whole state), so that a
//! change `update()` makes is kept and a change a nested call made to the lent copy is kept too.
//!
//! `p1w11-sonnet`, 2026-10-21.

use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime};
use crate::logic::modules::UpdateHost;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// A concrete update module. A compile-time contract, used as a generic bound by the functions of this file.
pub trait UpdateBehavior: Clone + PartialEq + Sized {
    /// The version `xfer` writes (`XferVersion currentVersion`).
    const XFER_VERSION: XferVersion = 1;
    /// The `UpdateModule` base (the scheduling half) the module made for itself.
    fn update_base(&self) -> &UpdateModule;
    /// `update()`. The module is the running one for the host calls ([`UpdateHost`]).
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime;
    /// The part of the constructor that needs the world (a wake call, a random draw): the world runs it once the module is part of its object,
    /// in module order.
    fn construct(&mut self, _world: &mut World, _me: ModuleHandle) {}
    /// The fields `xfer` writes after the `UpdateModule` base; `version` is the one read or written.
    fn xfer_fields(&mut self, _xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        Ok(())
    }
    /// `loadPostProcess` after the base's.
    fn load_post_process_fields(&mut self) {}
    /// The module as the variant of [`UpdateBody`] it is.
    fn into_body(self) -> UpdateBody;
    /// The module back out of its variant; `None` for another variant.
    fn from_body(body: UpdateBody) -> Option<Self>;
    /// A call to the module's own interface (`extendRods`, `extendRadar`, ...): [`ModuleReply::Unhandled`] unless it overrides this.
    fn other_call(&mut self, _world: &mut World, _me: ModuleHandle, _call: ModuleCall) -> ModuleReply {
        ModuleReply::Unhandled
    }
}

/// `update()` as the scheduler runs it: this module is the running one for the host calls, and it is lent to its slot for the duration.
pub fn world_update<M: UpdateBehavior>(m: &mut M, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
    let Some(h) = world.module_handle_of_update(me) else {
        // a scheduler entry with no module: nothing to run
        return crate::logic::module::update_module::UPDATE_SLEEP_NONE;
    };
    let before = m.clone();
    let (r, back) = world.lend_body(h, m.clone().into_body(), |w| w.in_update_module(me, |w| m.update(w, h)));
    if *m == before {
        if let Some(b) = back.and_then(M::from_body) {
            *m = b;
        }
    }
    r
}

/// The constructor's second half, run by the world once the module is part of its object, as the running module.
pub fn world_construct<M: UpdateBehavior>(m: &mut M, world: &mut World, me: ModuleHandle) {
    m.construct(world, me);
}

/// The module's answer to one of `Object`'s calls.
pub fn update_module_call<M: UpdateBehavior>(m: &mut M, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
    m.other_call(world, me, call)
}

/// `xfer` with the scheduling half the slot holds (the live `UpdateModule`): the version, that half, then the fields.
pub fn xfer_with<M: UpdateBehavior>(m: &mut M, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current = M::XFER_VERSION;
    let mut version = current;
    xfer.xfer_version(&mut version, current)?;
    update.xfer(xfer)?;
    m.xfer_fields(xfer, version)
}

/// `xfer` with the module's own `UpdateModule` base.
pub fn xfer_own<M: UpdateBehavior>(m: &mut M, own: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
    xfer_with(m, own, xfer)
}

/// `crc` with the scheduling half the slot holds.
pub fn crc_with(update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
    update.crc(xfer)
}

/// `loadPostProcess` with the scheduling half the slot holds.
pub fn load_post_process_with<M: UpdateBehavior>(m: &mut M, update: &mut UpdateModule) {
    update.load_post_process();
    m.load_post_process_fields();
}

/// `setWakeFrame(getObject(), wakeDelay)`: `now + wakeDelay` to the sleepy list, for the module that is running.
pub fn set_wake_frame(world: &mut World, wake_delay: UpdateSleepTime) {
    let frame = UpdateModule::set_wake_frame(UpdateHost::frame(world), wake_delay);
    UpdateHost::awaken_update_module(world, frame);
}
