// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590): the layer-9 update modules of
// `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/`, one file per module.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Layer-9 update modules that need nothing outside the object: each file is
//! the module's data class (its INI fields) and its logic, written against
//! [`UpdateHost`], the cut point to the world.
//!
//! [`UpdateHost`] stands for the calls a module makes on `TheGameLogic` and
//! on its own `Object` (`getFrame`, `friend_awakenUpdateModule`,
//! `destroyObject`, `kill`, `isKindOf`). The world implements it when the
//! module is wired into the object list; the differential tests implement it
//! with the C++ stand-ins. Nothing here reaches the world directly.

pub mod base_regenerate_update;
pub mod deletion_update;
pub mod lifetime_update;
pub mod smart_bomb_target_homing_update;

/// The calls a layer-9 update module makes on the game logic and on its
/// object. Every method is one upstream call, in its upstream order.
pub trait UpdateHost {
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `getObject()->isKindOf(KINDOF_HULK)`.
    fn is_kind_of_hulk(&self) -> bool;
    /// `TheGameLogic->getHulkMaxLifetimeOverride()`.
    fn hulk_max_lifetime_override(&self) -> i32;
    /// `TheGameLogic->friend_awakenUpdateModule(getObject(), this, frame)`.
    fn awaken_update_module(&mut self, frame: u32);
    /// `getObject()->kill()`.
    fn kill_object(&mut self);
    /// `TheGameLogic->destroyObject(getObject())`.
    fn destroy_object(&mut self);
}
