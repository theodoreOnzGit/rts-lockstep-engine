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

// --- p1l9-helpers ---
pub mod object_helper;
pub mod object_smc_helper;
pub mod object_repulsor_helper;
pub mod object_weapon_status_helper;
pub mod status_damage_helper;
pub mod temp_weapon_bonus_helper;
pub mod destroy_die;
pub mod keep_object_die;
pub mod upgrade_die;
// --- end p1l9-helpers ---

// --- p1l9-example ---
pub mod command_set_upgrade;
// --- end p1l9-example ---

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
