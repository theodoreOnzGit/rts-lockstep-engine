// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590): the concrete object
// modules under GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ (layer 9);
// each file names its own upstream source.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Layer 9: the concrete object modules (`Object/Behavior`, `Body`,
//! `Collide`, `Contain`, `Create`, `Damage`, `Destroy`, `Die`, `Helper`,
//! `SpecialPower`, `Update`, `Upgrade`).
//!
//! [`command_set_upgrade`] is the lead's worked example: its module docs list
//! every piece a translated module adds, and where. [`new_module`] is where
//! each one is made for an object: one arm per translated class.

use crate::common::ini::IniResult;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::logic::world::object_create::NewModule;
use crate::logic::world::update_body::UpdateBody;

pub mod upgrade_body;

// --- p1l9-example ---
pub mod command_set_upgrade;
// --- end p1l9-example ---

// --- p1l9-creates ---
pub mod create_host;
pub mod grant_upgrade_create;
pub mod lock_weapon_create;
pub mod preorder_create;
pub mod special_power_create;
pub mod supply_center_create;
pub mod supply_warehouse_create;
pub mod veterancy_gain_create;
// --- end p1l9-creates ---

/// The module for one module line of an object's template: the concrete
/// class `data` belongs to, made for `object` (upstream:
/// `TheModuleFactory->newModule(thing, name, data, type)`), with its tag.
/// `None` for data of a class that is not translated.
pub fn new_module(object: ObjectId, name: &[u8], tag: &[u8], data: &ModuleDataKind) -> Option<IniResult<NewModule>> {
    let made = |interfaces: i32, body: IniResult<UpdateBody>| -> IniResult<NewModule> {
        let mut m = NewModule::plain(name, tag);
        m.interfaces = interfaces;
        m.body = Some(body?);
        Ok(m)
    };
    match data {
        // --- p1l9-example ---
        ModuleDataKind::CommandSetUpgrade(d) => Some(made(
            command_set_upgrade::CommandSetUpgrade::get_interface_mask(),
            command_set_upgrade::CommandSetUpgrade::new(object, Some(d.clone())).map(UpdateBody::CommandSetUpgrade),
        )),
        // --- end p1l9-example ---
        _ => None,
    }
}

// --- p1l9-updates ---
// Layer-9 update modules that need nothing outside the object (lifetime_update, deletion_update,
// base_regenerate_update, smart_bomb_target_homing_update): each file is the module's data class and its logic,
// written against `UpdateHost` (and its extensions), the cut point to the world. Not yet wired into the world.

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

// --- end p1l9-updates ---
