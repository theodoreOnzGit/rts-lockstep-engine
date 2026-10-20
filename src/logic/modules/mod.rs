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
        // --- p1l9-upgrades ---
        ModuleDataKind::WeaponSetUpgrade(d) => Some(made(
            weapon_set_upgrade::WeaponSetUpgrade::get_interface_mask(),
            weapon_set_upgrade::WeaponSetUpgrade::new(object, Some(d.clone())).map(UpdateBody::WeaponSetUpgrade),
        )),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        ModuleDataKind::WeaponBonusUpgrade(d) => Some(made(
            weapon_bonus_upgrade::WeaponBonusUpgrade::get_interface_mask(),
            weapon_bonus_upgrade::WeaponBonusUpgrade::new(object, Some(d.clone())).map(UpdateBody::WeaponBonusUpgrade),
        )),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        ModuleDataKind::ArmorUpgrade(d) => Some(made(
            armor_upgrade::ArmorUpgrade::get_interface_mask(),
            armor_upgrade::ArmorUpgrade::new(object, Some(d.clone())).map(UpdateBody::ArmorUpgrade),
        )),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        ModuleDataKind::ModelConditionUpgrade(d) => Some(made(
            model_condition_upgrade::ModelConditionUpgrade::get_interface_mask(),
            model_condition_upgrade::ModelConditionUpgrade::new(object, Some(d.clone())).map(UpdateBody::ModelConditionUpgrade),
        )),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        ModuleDataKind::ExperienceScalarUpgrade(d) => Some(made(
            experience_scalar_upgrade::ExperienceScalarUpgrade::get_interface_mask(),
            experience_scalar_upgrade::ExperienceScalarUpgrade::new(object, Some(d.clone())).map(UpdateBody::ExperienceScalarUpgrade),
        )),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        ModuleDataKind::ActiveShroudUpgrade(d) => Some(made(
            active_shroud_upgrade::ActiveShroudUpgrade::get_interface_mask(),
            active_shroud_upgrade::ActiveShroudUpgrade::new(object, Some(d.clone())).map(UpdateBody::ActiveShroudUpgrade),
        )),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        ModuleDataKind::MaxHealthUpgrade(d) => Some(made(
            max_health_upgrade::MaxHealthUpgrade::get_interface_mask(),
            max_health_upgrade::MaxHealthUpgrade::new(object, Some(d.clone())).map(UpdateBody::MaxHealthUpgrade),
        )),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        ModuleDataKind::GrantScienceUpgrade(d) => Some(made(
            grant_science_upgrade::GrantScienceUpgrade::get_interface_mask(),
            grant_science_upgrade::GrantScienceUpgrade::new(object, Some(d.clone())).map(UpdateBody::GrantScienceUpgrade),
        )),
        // --- end p1l9-upgrades ---
        _ => None,
    }
}

// --- p1l9-upgrades ---
pub mod weapon_set_upgrade;
pub mod weapon_bonus_upgrade;
pub mod armor_upgrade;
pub mod model_condition_upgrade;
pub mod experience_scalar_upgrade;
pub mod active_shroud_upgrade;
pub mod max_health_upgrade;
pub mod grant_science_upgrade;
// --- end p1l9-upgrades ---
