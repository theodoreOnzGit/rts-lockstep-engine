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
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleHandle;
use crate::logic::world::object_create::NewModule;
use crate::common::module::MODULEINTERFACE_DAMAGE;
use crate::logic::module::create_module::CreateModule;
use crate::logic::module::die_module::DieModule;
use crate::logic::module::update_module::UpdateModule;
use crate::logic::world::update_body::UpdateBody;

pub mod crate_collide;
pub mod upgrade_pair;
pub mod subdual_damage_helper;
pub mod object_defection_helper;
pub mod update_behavior;
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

// --- p1w10-sonnet ---
pub mod deliver_payload_data;
pub mod status_bits_upgrade;
pub mod locomotor_set_upgrade;
pub mod passengers_fire_upgrade;
pub mod stealth_upgrade;
pub mod sub_objects_upgrade;
pub mod object_creation_upgrade;
// --- end p1w10-sonnet ---

// --- p1w11-sonnet ---
pub mod cost_modifier_upgrade;
pub mod power_plant_upgrade;
pub mod radar_upgrade;
pub mod unpause_special_power_upgrade;
pub mod die_body;
pub mod replace_object_upgrade;
pub mod dam_die;
pub mod special_power_completion_die;
pub mod eject_pilot_die;
pub mod power_plant_update;
pub mod radar_update;
pub mod heal_crate_collide;
pub mod shroud_crate_collide;
pub mod money_crate_collide;
// --- end p1w11-sonnet ---
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
    // --- p1w10-sonnet ---
    let made_update = |interfaces: i32, made: IniResult<(UpdateModule, UpdateBody)>| -> IniResult<NewModule> {
        let (update, body) = made?;
        let mut m = NewModule::plain(name, tag);
        m.interfaces = interfaces;
        m.update = Some(update);
        m.body = Some(body);
        Ok(m)
    };
    // --- end p1w10-sonnet ---
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
        // --- p1w10-sonnet ---
        ModuleDataKind::StatusBitsUpgrade(d) => Some(made(
            status_bits_upgrade::StatusBitsUpgrade::get_interface_mask(),
            status_bits_upgrade::StatusBitsUpgrade::new(object, Some(d.clone())).map(UpdateBody::StatusBitsUpgrade),
        )),
        ModuleDataKind::LocomotorSetUpgrade(d) => Some(made(
            locomotor_set_upgrade::LocomotorSetUpgrade::get_interface_mask(),
            locomotor_set_upgrade::LocomotorSetUpgrade::new(object, Some(d.clone())).map(UpdateBody::LocomotorSetUpgrade),
        )),
        ModuleDataKind::PassengersFireUpgrade(d) => Some(made(
            passengers_fire_upgrade::PassengersFireUpgrade::get_interface_mask(),
            passengers_fire_upgrade::PassengersFireUpgrade::new(object, Some(d.clone())).map(UpdateBody::PassengersFireUpgrade),
        )),
        ModuleDataKind::StealthUpgrade(d) => Some(made(
            stealth_upgrade::StealthUpgrade::get_interface_mask(),
            stealth_upgrade::StealthUpgrade::new(object, Some(d.clone())).map(UpdateBody::StealthUpgrade),
        )),
        ModuleDataKind::SubObjectsUpgrade(d) => Some(made(
            sub_objects_upgrade::SubObjectsUpgrade::get_interface_mask(),
            sub_objects_upgrade::SubObjectsUpgrade::new(object, Some(d.clone())).map(UpdateBody::SubObjectsUpgrade),
        )),
        ModuleDataKind::ObjectCreationUpgrade(d) => Some(made(
            object_creation_upgrade::ObjectCreationUpgrade::get_interface_mask(),
            object_creation_upgrade::ObjectCreationUpgrade::new(object, Some(d.clone())).map(UpdateBody::ObjectCreationUpgrade),
        )),
        // --- end p1w10-sonnet ---
        // --- p1w10-sonnet ---
        ModuleDataKind::GrantUpgradeCreate(d) => Some(made(
            CreateModule::get_interface_mask(),
            grant_upgrade_create::GrantUpgradeCreate::new(object, Some(d.clone())).map(UpdateBody::GrantUpgradeCreate),
        )),
        ModuleDataKind::LockWeaponCreate(d) => Some(made(
            CreateModule::get_interface_mask(),
            lock_weapon_create::LockWeaponCreate::new(object, Some(d.clone())).map(UpdateBody::LockWeaponCreate),
        )),
        ModuleDataKind::VeterancyGainCreate(d) => Some(made(
            CreateModule::get_interface_mask(),
            veterancy_gain_create::VeterancyGainCreate::new(object, Some(d.clone())).map(UpdateBody::VeterancyGainCreate),
        )),
        ModuleDataKind::LifetimeUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            lifetime_update::LifetimeUpdate::unconstructed(object, Some(d.clone()))
                .map(|m| (m.update_module().clone(), UpdateBody::LifetimeUpdate(m))),
        )),
        ModuleDataKind::DeletionUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            deletion_update::DeletionUpdate::unconstructed(object, Some(d.clone()))
                .map(|m| (m.update_module().clone(), UpdateBody::DeletionUpdate(m))),
        )),
        ModuleDataKind::BaseRegenerateUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask() | MODULEINTERFACE_DAMAGE,
            base_regenerate_update::BaseRegenerateUpdate::unconstructed(object, Some(d.clone()))
                .map(|m| (m.update_module().clone(), UpdateBody::BaseRegenerateUpdate(m))),
        )),
        ModuleDataKind::SmartBombTargetHomingUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            smart_bomb_target_homing_update::SmartBombTargetHomingUpdate::unconstructed(object, Some(d.clone()))
                .map(|m| (m.update_module().clone(), UpdateBody::SmartBombTargetHomingUpdate(m))),
        )),
        ModuleDataKind::UpgradeDie(d) => Some(made(
            DieModule::get_interface_mask(),
            upgrade_die::UpgradeDie::new(object, Some(d.clone())).map(UpdateBody::UpgradeDie),
        )),
        // the two die modules whose data is `DieModuleData` itself are told apart by their name
        ModuleDataKind::Die(d) => match name {
            b"DestroyDie" => Some(made(DieModule::get_interface_mask(), destroy_die::DestroyDie::new(object, Some(d.clone())).map(UpdateBody::DestroyDie))),
            b"KeepObjectDie" => Some(made(DieModule::get_interface_mask(), keep_object_die::KeepObjectDie::new(object, Some(d.clone())).map(UpdateBody::KeepObjectDie))),
            _ => None,
        },
        // the four create modules whose data is `CreateModuleData` itself are told apart by their name
        ModuleDataKind::Create(d) => match name {
            b"PreorderCreate" => Some(made(CreateModule::get_interface_mask(), preorder_create::PreorderCreate::new(object, Some(d.clone())).map(UpdateBody::PreorderCreate))),
            b"SpecialPowerCreate" => {
                Some(made(CreateModule::get_interface_mask(), special_power_create::SpecialPowerCreate::new(object, Some(d.clone())).map(UpdateBody::SpecialPowerCreate)))
            }
            b"SupplyCenterCreate" => {
                Some(made(CreateModule::get_interface_mask(), supply_center_create::SupplyCenterCreate::new(object, Some(d.clone())).map(UpdateBody::SupplyCenterCreate)))
            }
            b"SupplyWarehouseCreate" => Some(made(
                CreateModule::get_interface_mask(),
                supply_warehouse_create::SupplyWarehouseCreate::new(object, Some(d.clone())).map(UpdateBody::SupplyWarehouseCreate),
            )),
            _ => None,
        },
        // --- end p1w10-sonnet ---
        // --- p1w11-sonnet ---
        ModuleDataKind::CostModifierUpgrade(d) => Some(made(
            cost_modifier_upgrade::CostModifierUpgrade::get_interface_mask(),
            cost_modifier_upgrade::CostModifierUpgrade::new(object, Some(d.clone())).map(UpdateBody::CostModifierUpgrade),
        )),
        ModuleDataKind::PowerPlantUpgrade(d) => Some(made(
            power_plant_upgrade::PowerPlantUpgrade::get_interface_mask(),
            power_plant_upgrade::PowerPlantUpgrade::new(object, Some(d.clone())).map(UpdateBody::PowerPlantUpgrade),
        )),
        ModuleDataKind::RadarUpgrade(d) => Some(made(
            radar_upgrade::RadarUpgrade::get_interface_mask(),
            radar_upgrade::RadarUpgrade::new(object, Some(d.clone())).map(UpdateBody::RadarUpgrade),
        )),
        ModuleDataKind::UnpauseSpecialPowerUpgrade(d) => Some(made(
            unpause_special_power_upgrade::UnpauseSpecialPowerUpgrade::get_interface_mask(),
            unpause_special_power_upgrade::UnpauseSpecialPowerUpgrade::new(object, Some(d.clone())).map(UpdateBody::UnpauseSpecialPowerUpgrade),
        )),
        ModuleDataKind::ReplaceObjectUpgrade(d) => Some(made(
            replace_object_upgrade::ReplaceObjectUpgrade::get_interface_mask(),
            replace_object_upgrade::ReplaceObjectUpgrade::new(object, Some(d.clone())).map(UpdateBody::ReplaceObjectUpgrade),
        )),
        ModuleDataKind::DamDie(d) => Some(made(
            DieModule::get_interface_mask(),
            dam_die::DamDie::new(object, Some(d.clone())).map(UpdateBody::DamDie),
        )),
        ModuleDataKind::SpecialPowerCompletionDie(d) => Some(made(
            DieModule::get_interface_mask(),
            special_power_completion_die::SpecialPowerCompletionDie::new(object, Some(d.clone())).map(UpdateBody::SpecialPowerCompletionDie),
        )),
        ModuleDataKind::EjectPilotDie(d) => Some(made(
            DieModule::get_interface_mask(),
            eject_pilot_die::EjectPilotDie::new(object, Some(d.clone())).map(UpdateBody::EjectPilotDie),
        )),
        ModuleDataKind::PowerPlantUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            power_plant_update::PowerPlantUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::PowerPlantUpdate(m))),
        )),
        ModuleDataKind::RadarUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            radar_update::RadarUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::RadarUpdate(m))),
        )),
        ModuleDataKind::HealCrateCollide(d) => Some(made(
            heal_crate_collide::HealCrateCollide::get_interface_mask(),
            heal_crate_collide::HealCrateCollide::new(object, Some(d.clone())).map(UpdateBody::HealCrateCollide),
        )),
        ModuleDataKind::ShroudCrateCollide(d) => Some(made(
            shroud_crate_collide::ShroudCrateCollide::get_interface_mask(),
            shroud_crate_collide::ShroudCrateCollide::new(object, Some(d.clone())).map(UpdateBody::ShroudCrateCollide),
        )),
        ModuleDataKind::MoneyCrateCollide(d) => Some(made(
            money_crate_collide::MoneyCrateCollide::get_interface_mask(),
            money_crate_collide::MoneyCrateCollide::new(object, Some(d.clone())).map(UpdateBody::MoneyCrateCollide),
        )),
        // --- end p1w11-sonnet ---
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

// --- p1w10-sonnet ---
/// (test-only) the planted bug that is switched on by `L9_PLANT`, for the differential tests of the layer-9 batches.
#[doc(hidden)]
pub fn plant(k: u32) -> bool {
    use std::sync::OnceLock;
    static P: OnceLock<u32> = OnceLock::new();
    *P.get_or_init(|| std::env::var("L9_PLANT").ok().and_then(|s| s.parse().ok()).unwrap_or(0)) == k
}

/// The factory's `NewModuleDataProc` for the die modules whose data is `DieModuleData` itself (`DestroyDie`, `KeepObjectDie`).
pub fn new_plain_die_module_data(ini: Option<&mut crate::common::ini::Ini<'_>>) -> crate::common::ini::IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::Die(std::sync::Arc::new(crate::logic::module::die_module::DieModuleData::friend_new_module_data(ini)?)))
}

/// Runs `f` on `m` with a copy of it lent to its slot (`World::lend_body`), so that a call that reaches the module from inside `f`
/// (`destroyObject` sends `onDelete` to every module of the object, the running one included) is delivered at once, as upstream's
/// plain recursion delivers it. If `f` did not change `m`, the lent copy (which the nested calls may have changed) becomes its state.
pub fn lend_clone<M: Clone + PartialEq, R>(
    world: &mut World,
    me: ModuleHandle,
    m: &mut M,
    wrap: fn(M) -> UpdateBody,
    unwrap: fn(UpdateBody) -> Option<M>,
    f: impl FnOnce(&mut World, &mut M) -> R,
) -> R {
    let before = m.clone();
    let (r, back) = world.lend_body(me, wrap(m.clone()), |w| f(w, m));
    if *m == before {
        if let Some(b) = back.and_then(unwrap) {
            *m = b;
        }
    }
    r
}

/// The calls a layer-9 die module makes on its object and the game logic (the cut point to the world): the die mux reads the
/// object's veterancy level and status bits; `DestroyDie` destroys it; `UpgradeDie` looks up its producer's upgrade.
pub trait DieHost {
    /// `getObject()->getVeterancyLevel()`, as an integer.
    fn veterancy_level(&self) -> i32;
    /// `getObject()->getStatusBits()`.
    fn status_bits(&self) -> crate::common::object_status_types::ObjectStatusMaskType;
    /// `TheGameLogic->destroyObject(getObject())`.
    fn destroy_object(&mut self);
    /// `TheGameLogic->findObjectByID(getObject()->getProducerID()) != NULL`.
    fn producer_exists(&self) -> bool;
    /// `TheUpgradeCenter->findUpgrade(name) != NULL`.
    fn find_upgrade(&mut self, name: &[u8]) -> bool;
    /// `producer->hasUpgrade(upgrade)` for the upgrade of that name.
    fn producer_has_upgrade(&mut self, name: &[u8]) -> bool;
    /// `producer->removeUpgrade(upgrade)` for the upgrade of that name.
    fn producer_remove_upgrade(&mut self, name: &[u8]);
}
// --- end p1w10-sonnet ---
