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
pub mod special_power_body;
pub mod special_power_module_impl;
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
pub mod instant_death_behavior;
pub mod rebuild_hole_expose_die;
pub mod create_crate_die;
pub mod crush_die;
pub mod create_object_die;
pub mod power_plant_update;
pub mod radar_update;
pub mod heal_crate_collide;
pub mod fire_weapon_collide;
pub mod shroud_crate_collide;
pub mod sabotage_crate_collide;
pub mod unit_crate_collide;
pub mod veterancy_crate_collide;
pub mod salvage_crate_collide;
pub mod convert_to_car_bomb_crate_collide;
pub mod convert_to_hijacked_vehicle_crate_collide;
pub mod money_crate_collide;
pub mod cleanup_area_power;
pub mod baikonur_launch_power;
pub mod spy_vision_special_power;
pub mod defector_special_power;
pub mod fire_weapon_power;
pub mod special_ability;
pub mod ocl_special_power;
pub mod cash_hack_special_power;
pub mod cash_bounty_power;
pub mod spy_vision_update;
pub mod upgrade_mux_user;
pub mod fx_list_die;
pub mod fire_weapon_when_dead_behavior;
pub mod bridge_tower_behavior;
pub mod auto_deposit_update;
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
pub mod fire_ocl_after_weapon_cooldown_update;
pub mod missile_launcher_building_update;
pub mod spectre_gunship_deployment_update;
pub mod spectre_gunship_update;
pub mod battle_plan_update;
pub mod countermeasures_behavior;
pub mod auto_heal_behavior;
pub mod fire_weapon_when_damaged_behavior;
pub mod checkpoint_update;
pub mod flammable_update;
pub mod poisoned_behavior;
pub mod production_exit;
pub mod weapon_bonus_update;
pub mod emp_update;
pub mod pilot_find_vehicle_update;
pub mod horde_update;
pub mod topple_update;
pub mod propaganda_tower_behavior;
pub mod dock_update;
pub mod repair_dock_update;
pub mod railed_transport_dock_update;
pub mod supply_center_dock_update;
pub mod supply_warehouse_dock_update;
pub mod projectile_stream_update;
pub mod generate_minefield_behavior;
pub mod spawn_behavior;
pub mod parking_place_behavior;
pub mod minefield_behavior;
pub mod sticky_bomb_update;
pub mod structure_topple_update;
pub mod neutron_blast_behavior;
pub mod demo_trap_update;
pub mod assisted_targeting_update;
pub mod slaved_update;
pub mod mob_member_slaved_update;
pub mod cleanup_hazard_update;
pub mod bunker_buster_behavior;
pub mod bridge_scaffold_behavior;
pub mod tensile_formation_update;
pub mod bone_fx_update;
pub mod transition_damage_fx;
pub mod structure_collapse_update;
pub mod stealth_detector_update;
pub mod stealth_update;
pub mod overcharge_behavior;
pub mod grant_stealth_behavior;
pub mod dynamic_geometry_info_update;
pub mod firestorm_dynamic_geometry_info_update;
pub mod animation_steering_update;
pub mod radius_decal_update;
pub mod auto_find_healing_update;
pub mod command_button_hunt_update;
pub mod queue_production_exit_update;
pub mod default_production_exit_update;
pub mod spawn_point_production_exit_update;
pub mod supply_center_production_exit_update;
pub mod supply_warehouse_crippling_behavior;
pub mod ocl_update;
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
pub mod height_die_update;
pub mod dynamic_shroud_clearing_range_update;
pub mod squish_collide;
pub mod prone_update;
pub mod enemy_near_update;
pub mod float_update;
pub mod wave_guide_update;
pub mod dumb_projectile_behavior;
pub mod neutron_missile_update;
pub mod point_defense_laser_update;
pub mod fire_weapon_update;
pub mod hijacker_update;
pub mod tech_building_behavior;
pub mod fire_spread_update;
pub mod active_body;
pub mod derived_bodies;
pub mod inactive_body;
pub mod slow_death_behavior;
pub mod jet_slow_death_behavior;
pub mod physics_behavior;
pub mod neutron_missile_slow_death_behavior;
pub mod helicopter_slow_death_behavior;
pub mod battle_bus_slow_death_behavior;
pub mod rebuild_hole_behavior;
pub mod open_contain;
pub mod transport_contain;
pub mod garrison_contain;
pub mod cave_contain;
pub mod tunnel_contain;
pub mod mob_nexus_contain;
pub mod parachute_contain;
pub mod heal_contain;
pub mod overlord_contain;
pub mod rider_change_contain;
pub mod helix_contain;
pub mod railed_transport_contain;
pub mod internet_hack_contain;
// --- end p1w13-sonnet ---
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
        // batch R31
        ModuleDataKind::TransitionDamageFx(d) => Some(made(
            transition_damage_fx::TransitionDamageFx::get_interface_mask(),
            transition_damage_fx::TransitionDamageFx::new(object, Some(d.clone())).map(UpdateBody::TransitionDamageFx),
        )),
        // batch R38: the behaviour module whose data is `BehaviorModuleData` itself (`MAKE_STANDARD_MODULE_MACRO`, the plain `ModuleData`)
        ModuleDataKind::Behavior(d) => match name {
            b"BridgeTowerBehavior" => Some(made(
                bridge_tower_behavior::BridgeTowerBehavior::get_interface_mask(),
                bridge_tower_behavior::BridgeTowerBehavior::new(object, Some(d.clone())).map(UpdateBody::BridgeTowerBehavior),
            )),
            _ => None,
        },
        // batch R30: the damage module whose data is `DamageModuleData` itself
        ModuleDataKind::Damage(d) => match name {
            b"BoneFXDamage" => Some(made(bone_fx_update::BoneFxDamage::get_interface_mask(), bone_fx_update::BoneFxDamage::new(object, Some(d.clone())).map(UpdateBody::BoneFxDamage))),
            _ => None,
        },
        // batch R35: the update module whose data is `UpdateModuleData` itself (`MAKE_STANDARD_MODULE_MACRO`, the plain `ModuleData`)
        ModuleDataKind::Update(d) => match name {
            b"BridgeScaffoldBehavior" => Some(made_update(
                bridge_scaffold_behavior::BridgeScaffoldBehavior::get_interface_mask(),
                bridge_scaffold_behavior::BridgeScaffoldBehavior::unconstructed(object, Some(d.clone()))
                    .map(|m| (m.update_module().clone(), UpdateBody::BridgeScaffoldBehavior(m))),
            )),
            // batch R42
            b"ProjectileStreamUpdate" => Some(made_update(
                projectile_stream_update::ProjectileStreamUpdate::get_interface_mask(),
                projectile_stream_update::ProjectileStreamUpdate::unconstructed(object, Some(d.clone()))
                    .map(|m| (m.update_module().clone(), UpdateBody::ProjectileStreamUpdate(m))),
            )),
            _ => None,
        },
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
        ModuleDataKind::InstantDeathBehavior(d) => Some(made(
            DieModule::get_interface_mask(),
            instant_death_behavior::InstantDeathBehavior::new(object, Some(d.clone())).map(UpdateBody::InstantDeathBehavior),
        )),
        ModuleDataKind::RebuildHoleExposeDie(d) => Some(made(
            DieModule::get_interface_mask(),
            rebuild_hole_expose_die::RebuildHoleExposeDie::new(object, Some(d.clone())).map(UpdateBody::RebuildHoleExposeDie),
        )),
        ModuleDataKind::CreateCrateDie(d) => Some(made(
            DieModule::get_interface_mask(),
            create_crate_die::CreateCrateDie::new(object, Some(d.clone())).map(UpdateBody::CreateCrateDie),
        )),
        ModuleDataKind::CrushDie(d) => Some(made(
            DieModule::get_interface_mask(),
            crush_die::CrushDie::new(object, Some(d.clone())).map(UpdateBody::CrushDie),
        )),
        ModuleDataKind::CreateObjectDie(d) => Some(made(
            DieModule::get_interface_mask(),
            create_object_die::CreateObjectDie::new(object, Some(d.clone())).map(UpdateBody::CreateObjectDie),
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
        ModuleDataKind::FireWeaponCollide(d) => Some(made(
            fire_weapon_collide::FireWeaponCollide::get_interface_mask(),
            fire_weapon_collide::FireWeaponCollide::new(object, Some(d.clone())).map(UpdateBody::FireWeaponCollide),
        )),
        ModuleDataKind::ShroudCrateCollide(d) => Some(made(
            shroud_crate_collide::ShroudCrateCollide::get_interface_mask(),
            shroud_crate_collide::ShroudCrateCollide::new(object, Some(d.clone())).map(UpdateBody::ShroudCrateCollide),
        )),
        ModuleDataKind::SabotageCrateCollide(d) => Some(made(
            sabotage_crate_collide::SabotageCrateCollide::get_interface_mask(),
            sabotage_crate_collide::SabotageCrateCollide::new(object, Some(d.clone())).map(UpdateBody::SabotageCrateCollide),
        )),
        ModuleDataKind::UnitCrateCollide(d) => Some(made(
            unit_crate_collide::UnitCrateCollide::get_interface_mask(),
            unit_crate_collide::UnitCrateCollide::new(object, Some(d.clone())).map(UpdateBody::UnitCrateCollide),
        )),
        ModuleDataKind::VeterancyCrateCollide(d) => Some(made(
            veterancy_crate_collide::VeterancyCrateCollide::get_interface_mask(),
            veterancy_crate_collide::VeterancyCrateCollide::new(object, Some(d.clone())).map(UpdateBody::VeterancyCrateCollide),
        )),
        ModuleDataKind::SalvageCrateCollide(d) => Some(made(
            salvage_crate_collide::SalvageCrateCollide::get_interface_mask(),
            salvage_crate_collide::SalvageCrateCollide::new(object, Some(d.clone())).map(UpdateBody::SalvageCrateCollide),
        )),
        ModuleDataKind::ConvertToCarBombCrateCollide(d) => Some(made(
            convert_to_car_bomb_crate_collide::ConvertToCarBombCrateCollide::get_interface_mask(),
            convert_to_car_bomb_crate_collide::ConvertToCarBombCrateCollide::new(object, Some(d.clone())).map(UpdateBody::ConvertToCarBombCrateCollide),
        )),
        ModuleDataKind::ConvertToHijackedVehicleCrateCollide(d) => Some(made(
            convert_to_hijacked_vehicle_crate_collide::ConvertToHijackedVehicleCrateCollide::get_interface_mask(),
            convert_to_hijacked_vehicle_crate_collide::ConvertToHijackedVehicleCrateCollide::new(object, Some(d.clone())).map(UpdateBody::ConvertToHijackedVehicleCrateCollide),
        )),
        ModuleDataKind::MoneyCrateCollide(d) => Some(made(
            money_crate_collide::MoneyCrateCollide::get_interface_mask(),
            money_crate_collide::MoneyCrateCollide::new(object, Some(d.clone())).map(UpdateBody::MoneyCrateCollide),
        )),
        ModuleDataKind::CleanupAreaPower(d) => Some(made(
            cleanup_area_power::CleanupAreaPower::get_interface_mask(),
            cleanup_area_power::CleanupAreaPower::new(object, Some(d.clone())).map(UpdateBody::CleanupAreaPower),
        )),
        ModuleDataKind::BaikonurLaunchPower(d) => Some(made(
            baikonur_launch_power::BaikonurLaunchPower::get_interface_mask(),
            baikonur_launch_power::BaikonurLaunchPower::new(object, Some(d.clone())).map(UpdateBody::BaikonurLaunchPower),
        )),
        ModuleDataKind::SpyVisionSpecialPower(d) => Some(made(
            spy_vision_special_power::SpyVisionSpecialPower::get_interface_mask(),
            spy_vision_special_power::SpyVisionSpecialPower::new(object, Some(d.clone())).map(UpdateBody::SpyVisionSpecialPower),
        )),
        ModuleDataKind::DefectorSpecialPower(d) => Some(made(
            defector_special_power::DefectorSpecialPower::get_interface_mask(),
            defector_special_power::DefectorSpecialPower::new(object, Some(d.clone())).map(UpdateBody::DefectorSpecialPower),
        )),
        ModuleDataKind::FireWeaponPower(d) => Some(made(
            fire_weapon_power::FireWeaponPower::get_interface_mask(),
            fire_weapon_power::FireWeaponPower::new(object, Some(d.clone())).map(UpdateBody::FireWeaponPower),
        )),
        ModuleDataKind::SpecialAbility(d) => Some(made(
            special_ability::SpecialAbility::get_interface_mask(),
            special_ability::SpecialAbility::new(object, Some(d.clone())).map(UpdateBody::SpecialAbility),
        )),
        ModuleDataKind::OCLSpecialPower(d) => Some(made(
            ocl_special_power::OCLSpecialPower::get_interface_mask(),
            ocl_special_power::OCLSpecialPower::new(object, Some(d.clone())).map(UpdateBody::OCLSpecialPower),
        )),
        ModuleDataKind::CashHackSpecialPower(d) => Some(made(
            cash_hack_special_power::CashHackSpecialPower::get_interface_mask(),
            cash_hack_special_power::CashHackSpecialPower::new(object, Some(d.clone())).map(UpdateBody::CashHackSpecialPower),
        )),
        ModuleDataKind::CashBountyPower(d) => Some(made(
            cash_bounty_power::CashBountyPower::get_interface_mask(),
            cash_bounty_power::CashBountyPower::new(object, Some(d.clone())).map(UpdateBody::CashBountyPower),
        )),
        ModuleDataKind::SpyVisionUpdate(d) => Some(made_update(
            spy_vision_update::SpyVisionUpdate::get_interface_mask(),
            spy_vision_update::SpyVisionUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SpyVisionUpdate(m))),
        )),
        ModuleDataKind::FXListDie(d) => Some(made(
            fx_list_die::FXListDie::get_interface_mask(),
            fx_list_die::FXListDie::new(object, Some(d.clone())).map(UpdateBody::FXListDie),
        )),
        ModuleDataKind::FireWeaponWhenDeadBehavior(d) => Some(made(
            fire_weapon_when_dead_behavior::FireWeaponWhenDeadBehavior::get_interface_mask(),
            fire_weapon_when_dead_behavior::FireWeaponWhenDeadBehavior::new(object, Some(d.clone())).map(UpdateBody::FireWeaponWhenDeadBehavior),
        )),
        ModuleDataKind::AutoDepositUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            auto_deposit_update::AutoDepositUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::AutoDepositUpdate(m))),
        )),
        // --- end p1w11-sonnet ---
        // --- p1w12-sonnet ---
        ModuleDataKind::FireOCLAfterWeaponCooldownUpdate(d) => Some(made_update(
            fire_ocl_after_weapon_cooldown_update::FireOCLAfterWeaponCooldownUpdate::get_interface_mask(),
            fire_ocl_after_weapon_cooldown_update::FireOCLAfterWeaponCooldownUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::FireOCLAfterWeaponCooldownUpdate(m))),
        )),
        ModuleDataKind::MissileLauncherBuildingUpdate(d) => Some(made_update(
            missile_launcher_building_update::MissileLauncherBuildingUpdate::get_interface_mask(),
            missile_launcher_building_update::MissileLauncherBuildingUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::MissileLauncherBuildingUpdate(m))),
        )),
        ModuleDataKind::SpectreGunshipDeploymentUpdate(d) => Some(made_update(
            spectre_gunship_deployment_update::SpectreGunshipDeploymentUpdate::get_interface_mask(),
            spectre_gunship_deployment_update::SpectreGunshipDeploymentUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SpectreGunshipDeploymentUpdate(m))),
        )),
        ModuleDataKind::SpectreGunshipUpdate(d) => Some(made_update(
            spectre_gunship_update::SpectreGunshipUpdate::get_interface_mask(),
            spectre_gunship_update::SpectreGunshipUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SpectreGunshipUpdate(m))),
        )),
        ModuleDataKind::BattlePlanUpdate(d) => Some(made_update(
            battle_plan_update::BattlePlanUpdate::get_interface_mask(),
            battle_plan_update::BattlePlanUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::BattlePlanUpdate(m))),
        )),
        ModuleDataKind::CountermeasuresBehavior(d) => Some(made_update(
            countermeasures_behavior::CountermeasuresBehavior::get_interface_mask(),
            countermeasures_behavior::CountermeasuresBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::CountermeasuresBehavior(m))),
        )),
        ModuleDataKind::AutoHealBehavior(d) => Some(made_update(
            auto_heal_behavior::AutoHealBehavior::get_interface_mask(),
            auto_heal_behavior::AutoHealBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::AutoHealBehavior(m))),
        )),
        ModuleDataKind::FireWeaponWhenDamagedBehavior(d) => Some(made_update(
            fire_weapon_when_damaged_behavior::FireWeaponWhenDamagedBehavior::get_interface_mask(),
            fire_weapon_when_damaged_behavior::FireWeaponWhenDamagedBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::FireWeaponWhenDamagedBehavior(m))),
        )),
        ModuleDataKind::CheckpointUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            checkpoint_update::CheckpointUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::CheckpointUpdate(m))),
        )),
        ModuleDataKind::FlammableUpdate(d) => Some(made_update(
            flammable_update::FlammableUpdate::get_interface_mask(),
            flammable_update::FlammableUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::FlammableUpdate(m))),
        )),
        ModuleDataKind::PoisonedBehavior(d) => Some(made_update(
            poisoned_behavior::PoisonedBehavior::get_interface_mask(),
            poisoned_behavior::PoisonedBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::PoisonedBehavior(m))),
        )),
        ModuleDataKind::WeaponBonusUpdate(d) => Some(made_update(
            weapon_bonus_update::WeaponBonusUpdate::get_interface_mask(),
            weapon_bonus_update::WeaponBonusUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::WeaponBonusUpdate(m))),
        )),
        ModuleDataKind::PilotFindVehicleUpdate(d) => Some(made_update(
            pilot_find_vehicle_update::PilotFindVehicleUpdate::get_interface_mask(),
            pilot_find_vehicle_update::PilotFindVehicleUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::PilotFindVehicleUpdate(m))),
        )),
        ModuleDataKind::HordeUpdate(d) => Some(made_update(
            horde_update::HordeUpdate::get_interface_mask(),
            horde_update::HordeUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::HordeUpdate(m))),
        )),
        ModuleDataKind::ToppleUpdate(d) => Some(made_update(
            topple_update::ToppleUpdate::get_interface_mask(),
            topple_update::ToppleUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::ToppleUpdate(m))),
        )),
        ModuleDataKind::PropagandaTowerBehavior(d) => Some(made_update(
            propaganda_tower_behavior::PropagandaTowerBehavior::get_interface_mask(),
            propaganda_tower_behavior::PropagandaTowerBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::PropagandaTowerBehavior(m))),
        )),
        ModuleDataKind::RepairDockUpdate(d) => Some(made_update(
            repair_dock_update::RepairDockUpdate::get_interface_mask(),
            repair_dock_update::RepairDockUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::RepairDockUpdate(m))),
        )),
        ModuleDataKind::RailedTransportDockUpdate(d) => Some(made_update(
            railed_transport_dock_update::RailedTransportDockUpdate::get_interface_mask(),
            railed_transport_dock_update::RailedTransportDockUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::RailedTransportDockUpdate(m))),
        )),
        ModuleDataKind::SupplyCenterDockUpdate(d) => Some(made_update(
            supply_center_dock_update::SupplyCenterDockUpdate::get_interface_mask(),
            supply_center_dock_update::SupplyCenterDockUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SupplyCenterDockUpdate(m))),
        )),
        ModuleDataKind::SupplyWarehouseDockUpdate(d) => Some(made_update(
            supply_warehouse_dock_update::SupplyWarehouseDockUpdate::get_interface_mask(),
            supply_warehouse_dock_update::SupplyWarehouseDockUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SupplyWarehouseDockUpdate(m))),
        )),
        ModuleDataKind::GenerateMinefieldBehavior(d) => Some(made_update(
            generate_minefield_behavior::GenerateMinefieldBehavior::get_interface_mask(),
            generate_minefield_behavior::GenerateMinefieldBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::GenerateMinefieldBehavior(m))),
        )),
        ModuleDataKind::SpawnBehavior(d) => Some(made_update(
            spawn_behavior::SpawnBehavior::get_interface_mask(),
            spawn_behavior::SpawnBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SpawnBehavior(m))),
        )),
        ModuleDataKind::ParkingPlaceBehavior(d) => Some(made_update(
            parking_place_behavior::ParkingPlaceBehavior::get_interface_mask(),
            parking_place_behavior::ParkingPlaceBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::ParkingPlaceBehavior(m))),
        )),
        ModuleDataKind::MinefieldBehavior(d) => Some(made_update(
            minefield_behavior::MinefieldBehavior::get_interface_mask(),
            minefield_behavior::MinefieldBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::MinefieldBehavior(m))),
        )),
        ModuleDataKind::StickyBombUpdate(d) => Some(made_update(
            sticky_bomb_update::StickyBombUpdate::get_interface_mask(),
            sticky_bomb_update::StickyBombUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::StickyBombUpdate(m))),
        )),
        ModuleDataKind::StructureToppleUpdate(d) => Some(made_update(
            structure_topple_update::StructureToppleUpdate::get_interface_mask(),
            structure_topple_update::StructureToppleUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::StructureToppleUpdate(m))),
        )),
        ModuleDataKind::NeutronBlastBehavior(d) => Some(made_update(
            neutron_blast_behavior::NeutronBlastBehavior::get_interface_mask(),
            neutron_blast_behavior::NeutronBlastBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::NeutronBlastBehavior(m))),
        )),
        ModuleDataKind::DemoTrapUpdate(d) => Some(made_update(
            demo_trap_update::DemoTrapUpdate::get_interface_mask(),
            demo_trap_update::DemoTrapUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::DemoTrapUpdate(m))),
        )),
        ModuleDataKind::AssistedTargetingUpdate(d) => Some(made_update(
            assisted_targeting_update::AssistedTargetingUpdate::get_interface_mask(),
            assisted_targeting_update::AssistedTargetingUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::AssistedTargetingUpdate(m))),
        )),
        ModuleDataKind::SlavedUpdate(d) => Some(made_update(
            slaved_update::SlavedUpdate::get_interface_mask(),
            slaved_update::SlavedUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SlavedUpdate(m))),
        )),
        ModuleDataKind::MobMemberSlavedUpdate(d) => Some(made_update(
            mob_member_slaved_update::MobMemberSlavedUpdate::get_interface_mask(),
            mob_member_slaved_update::MobMemberSlavedUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::MobMemberSlavedUpdate(m))),
        )),
        ModuleDataKind::CleanupHazardUpdate(d) => Some(made_update(
            cleanup_hazard_update::CleanupHazardUpdate::get_interface_mask(),
            cleanup_hazard_update::CleanupHazardUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::CleanupHazardUpdate(m))),
        )),
        ModuleDataKind::BunkerBusterBehavior(d) => Some(made_update(
            bunker_buster_behavior::BunkerBusterBehavior::get_interface_mask(),
            bunker_buster_behavior::BunkerBusterBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::BunkerBusterBehavior(m))),
        )),
        ModuleDataKind::TensileFormationUpdate(d) => Some(made_update(
            tensile_formation_update::TensileFormationUpdate::get_interface_mask(),
            tensile_formation_update::TensileFormationUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::TensileFormationUpdate(m))),
        )),
        ModuleDataKind::LeafletDropBehavior(d) => Some(made_update(
            emp_update::LeafletDropBehavior::get_interface_mask(),
            emp_update::LeafletDropBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::LeafletDropBehavior(m))),
        )),
        ModuleDataKind::EmpUpdate(d) => Some(made_update(
            emp_update::EmpUpdate::get_interface_mask(),
            emp_update::EmpUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::EmpUpdate(m))),
        )),
        ModuleDataKind::BoneFxUpdate(d) => Some(made_update(
            bone_fx_update::BoneFxUpdate::get_interface_mask(),
            bone_fx_update::BoneFxUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::BoneFxUpdate(m))),
        )),
        ModuleDataKind::StructureCollapseUpdate(d) => Some(made_update(
            structure_collapse_update::StructureCollapseUpdate::get_interface_mask(),
            structure_collapse_update::StructureCollapseUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::StructureCollapseUpdate(m))),
        )),
        ModuleDataKind::StealthDetectorUpdate(d) => Some(made_update(
            stealth_detector_update::StealthDetectorUpdate::get_interface_mask(),
            stealth_detector_update::StealthDetectorUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::StealthDetectorUpdate(m))),
        )),
        // `getStealth()` answers this module
        ModuleDataKind::StealthUpdate(d) => Some(
            made_update(
                stealth_update::StealthUpdate::get_interface_mask(),
                stealth_update::StealthUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::StealthUpdate(m))),
            )
            .map(|mut m| {
                m.is_stealth = true;
                m
            }),
        ),
        ModuleDataKind::OverchargeBehavior(d) => Some(made_update(
            overcharge_behavior::OverchargeBehavior::get_interface_mask(),
            overcharge_behavior::OverchargeBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::OverchargeBehavior(m))),
        )),
        ModuleDataKind::GrantStealthBehavior(d) => Some(made_update(
            grant_stealth_behavior::GrantStealthBehavior::get_interface_mask(),
            grant_stealth_behavior::GrantStealthBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::GrantStealthBehavior(m))),
        )),
        ModuleDataKind::DynamicGeometryInfoUpdate(d) => Some(made_update(
            dynamic_geometry_info_update::DynamicGeometryInfoUpdate::get_interface_mask(),
            dynamic_geometry_info_update::DynamicGeometryInfoUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::DynamicGeometryInfoUpdate(m))),
        )),
        ModuleDataKind::FirestormDynamicGeometryInfoUpdate(d) => Some(made_update(
            firestorm_dynamic_geometry_info_update::FirestormDynamicGeometryInfoUpdate::get_interface_mask(),
            firestorm_dynamic_geometry_info_update::FirestormDynamicGeometryInfoUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::FirestormDynamicGeometryInfoUpdate(m))),
        )),
        ModuleDataKind::AnimationSteeringUpdate(d) => Some(made_update(
            animation_steering_update::AnimationSteeringUpdate::get_interface_mask(),
            animation_steering_update::AnimationSteeringUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::AnimationSteeringUpdate(m))),
        )),
        ModuleDataKind::RadiusDecalUpdate(d) => Some(made_update(
            radius_decal_update::RadiusDecalUpdate::get_interface_mask(),
            radius_decal_update::RadiusDecalUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::RadiusDecalUpdate(m))),
        )),
        ModuleDataKind::AutoFindHealingUpdate(d) => Some(made_update(
            auto_find_healing_update::AutoFindHealingUpdate::get_interface_mask(),
            auto_find_healing_update::AutoFindHealingUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::AutoFindHealingUpdate(m))),
        )),
        ModuleDataKind::CommandButtonHuntUpdate(d) => Some(made_update(
            command_button_hunt_update::CommandButtonHuntUpdate::get_interface_mask(),
            command_button_hunt_update::CommandButtonHuntUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::CommandButtonHuntUpdate(m))),
        )),
        ModuleDataKind::QueueProductionExitUpdate(d) => Some(made_update(
            queue_production_exit_update::QueueProductionExitUpdate::get_interface_mask(),
            queue_production_exit_update::QueueProductionExitUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::QueueProductionExitUpdate(m))),
        )),
        ModuleDataKind::DefaultProductionExitUpdate(d) => Some(made_update(
            default_production_exit_update::DefaultProductionExitUpdate::get_interface_mask(),
            default_production_exit_update::DefaultProductionExitUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::DefaultProductionExitUpdate(m))),
        )),
        ModuleDataKind::SpawnPointProductionExitUpdate(d) => Some(made_update(
            spawn_point_production_exit_update::SpawnPointProductionExitUpdate::get_interface_mask(),
            spawn_point_production_exit_update::SpawnPointProductionExitUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SpawnPointProductionExitUpdate(m))),
        )),
        ModuleDataKind::SupplyCenterProductionExitUpdate(d) => Some(made_update(
            supply_center_production_exit_update::SupplyCenterProductionExitUpdate::get_interface_mask(),
            supply_center_production_exit_update::SupplyCenterProductionExitUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SupplyCenterProductionExitUpdate(m))),
        )),
        ModuleDataKind::SupplyWarehouseCripplingBehavior(d) => Some(made_update(
            supply_warehouse_crippling_behavior::SupplyWarehouseCripplingBehavior::get_interface_mask(),
            supply_warehouse_crippling_behavior::SupplyWarehouseCripplingBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SupplyWarehouseCripplingBehavior(m))),
        )),
        ModuleDataKind::OclUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            ocl_update::OclUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::OclUpdate(m))),
        )),
        // --- end p1w12-sonnet ---
        // --- p1w13-sonnet ---
        ModuleDataKind::HeightDieUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            height_die_update::HeightDieUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::HeightDieUpdate(m))),
        )),
        ModuleDataKind::DynamicShroudClearingRangeUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            dynamic_shroud_clearing_range_update::DynamicShroudClearingRangeUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::DynamicShroudClearingRangeUpdate(m))),
        )),
        ModuleDataKind::Collide(d) => match name {
            b"SquishCollide" => Some(made(crate::logic::module::collide_module::CollideModule::get_interface_mask(), squish_collide::SquishCollide::new(object, Some(d.clone())).map(UpdateBody::SquishCollide))),
            _ => None,
        },
        ModuleDataKind::ProneUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            prone_update::ProneUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::ProneUpdate(m))),
        )),
        ModuleDataKind::EnemyNearUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            enemy_near_update::EnemyNearUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::EnemyNearUpdate(m))),
        )),
        ModuleDataKind::FloatUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            float_update::FloatUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::FloatUpdate(m))),
        )),
        ModuleDataKind::WaveGuideUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            wave_guide_update::WaveGuideUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::WaveGuideUpdate(m))),
        )),
        ModuleDataKind::DumbProjectileBehavior(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            dumb_projectile_behavior::DumbProjectileBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::DumbProjectileBehavior(m))),
        )),
        ModuleDataKind::NeutronMissileUpdate(d) => Some(made_update(
            neutron_missile_update::NeutronMissileUpdate::get_interface_mask(),
            neutron_missile_update::NeutronMissileUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::NeutronMissileUpdate(m))),
        )),
        ModuleDataKind::PointDefenseLaserUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            point_defense_laser_update::PointDefenseLaserUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::PointDefenseLaserUpdate(m))),
        )),
        ModuleDataKind::FireWeaponUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            fire_weapon_update::FireWeaponUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::FireWeaponUpdate(m))),
        )),
        ModuleDataKind::HijackerUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            hijacker_update::HijackerUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::HijackerUpdate(m))),
        )),
        ModuleDataKind::TechBuildingBehavior(d) => Some(made_update(
            tech_building_behavior::TechBuildingBehavior::get_interface_mask(),
            tech_building_behavior::TechBuildingBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::TechBuildingBehavior(m))),
        )),
        ModuleDataKind::FireSpreadUpdate(d) => Some(made_update(
            UpdateModule::get_interface_mask(),
            fire_spread_update::FireSpreadUpdate::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::FireSpreadUpdate(m))),
        )),
        ModuleDataKind::Module(d) if name == b"InactiveBody" => Some(made(
            inactive_body::InactiveBody::get_interface_mask(),
            inactive_body::InactiveBody::new(object, Some(d.clone())).map(UpdateBody::InactiveBody),
        )),
        ModuleDataKind::ActiveBody(d) => Some(made(
            active_body::ActiveBody::get_interface_mask(),
            match name {
                b"HighlanderBody" => derived_bodies::HighlanderBody::new(object, Some(d.clone())).map(UpdateBody::HighlanderBody),
                b"ImmortalBody" => derived_bodies::ImmortalBody::new(object, Some(d.clone())).map(UpdateBody::ImmortalBody),
                b"StructureBody" => derived_bodies::StructureBody::new(object, Some(d.clone())).map(UpdateBody::StructureBody),
                _ => active_body::ActiveBody::new(object, Some(d.clone())).map(UpdateBody::ActiveBody),
            },
        )),
        ModuleDataKind::UndeadBody(d) => Some(made(
            active_body::ActiveBody::get_interface_mask(),
            derived_bodies::UndeadBody::new(object, Some(d.clone())).map(UpdateBody::UndeadBody),
        )),
        ModuleDataKind::HiveStructureBody(d) => Some(made(
            active_body::ActiveBody::get_interface_mask(),
            derived_bodies::HiveStructureBody::new(object, Some(d.clone())).map(UpdateBody::HiveStructureBody),
        )),
        ModuleDataKind::OpenContain(d) => Some(made_update(
            open_contain::OpenContain::get_interface_mask(),
            open_contain::OpenContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::OpenContain(m))),
        )),
        ModuleDataKind::InternetHackContain(d) => Some(made_update(
            internet_hack_contain::InternetHackContain::get_interface_mask(),
            internet_hack_contain::InternetHackContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::InternetHackContain(m))),
        )),
        ModuleDataKind::RailedTransportContain(d) => Some(made_update(
            railed_transport_contain::RailedTransportContain::get_interface_mask(),
            railed_transport_contain::RailedTransportContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::RailedTransportContain(m))),
        )),
        ModuleDataKind::HelixContain(d) => Some(made_update(
            helix_contain::HelixContain::get_interface_mask(),
            helix_contain::HelixContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::HelixContain(m))),
        )),
        ModuleDataKind::RiderChangeContain(d) => Some(made_update(
            rider_change_contain::RiderChangeContain::get_interface_mask(),
            rider_change_contain::RiderChangeContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::RiderChangeContain(m))),
        )),
        ModuleDataKind::OverlordContain(d) => Some(made_update(
            overlord_contain::OverlordContain::get_interface_mask(),
            overlord_contain::OverlordContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::OverlordContain(m))),
        )),
        ModuleDataKind::HealContain(d) => Some(made_update(
            heal_contain::HealContain::get_interface_mask(),
            heal_contain::HealContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::HealContain(m))),
        )),
        ModuleDataKind::ParachuteContain(d) => Some(made_update(
            parachute_contain::ParachuteContain::get_interface_mask(),
            parachute_contain::ParachuteContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::ParachuteContain(m))),
        )),
        ModuleDataKind::MobNexusContain(d) => Some(made_update(
            mob_nexus_contain::MobNexusContain::get_interface_mask(),
            mob_nexus_contain::MobNexusContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::MobNexusContain(m))),
        )),
        ModuleDataKind::TunnelContain(d) => Some(made_update(
            tunnel_contain::TunnelContain::get_interface_mask(),
            tunnel_contain::TunnelContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::TunnelContain(m))),
        )),
        ModuleDataKind::CaveContain(d) => Some(made_update(
            cave_contain::CaveContain::get_interface_mask(),
            cave_contain::CaveContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::CaveContain(m))),
        )),
        ModuleDataKind::GarrisonContain(d) => Some(made_update(
            garrison_contain::GarrisonContain::get_interface_mask(),
            garrison_contain::GarrisonContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::GarrisonContain(m))),
        )),
        ModuleDataKind::TransportContain(d) => Some(made_update(
            transport_contain::TransportContain::get_interface_mask(),
            transport_contain::TransportContain::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::TransportContain(m))),
        )),
        ModuleDataKind::SlowDeathBehavior(d) => Some(made_update(
            slow_death_behavior::SlowDeathBehavior::get_interface_mask(),
            slow_death_behavior::SlowDeathBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::SlowDeathBehavior(m))),
        )),
        ModuleDataKind::JetSlowDeathBehavior(d) => Some(made_update(
            jet_slow_death_behavior::JetSlowDeathBehavior::get_interface_mask(),
            jet_slow_death_behavior::JetSlowDeathBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::JetSlowDeathBehavior(m))),
        )),
        ModuleDataKind::PhysicsBehavior(d) => Some(made_update(
            physics_behavior::PhysicsBehavior::get_interface_mask(),
            physics_behavior::PhysicsBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::PhysicsBehavior(m))),
        )),
        ModuleDataKind::NeutronMissileSlowDeathBehavior(d) => Some(made_update(
            neutron_missile_slow_death_behavior::NeutronMissileSlowDeathBehavior::get_interface_mask(),
            neutron_missile_slow_death_behavior::NeutronMissileSlowDeathBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::NeutronMissileSlowDeathBehavior(m))),
        )),
        ModuleDataKind::HelicopterSlowDeathBehavior(d) => Some(made_update(
            helicopter_slow_death_behavior::HelicopterSlowDeathBehavior::get_interface_mask(),
            helicopter_slow_death_behavior::HelicopterSlowDeathBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::HelicopterSlowDeathBehavior(m))),
        )),
        ModuleDataKind::BattleBusSlowDeathBehavior(d) => Some(made_update(
            battle_bus_slow_death_behavior::BattleBusSlowDeathBehavior::get_interface_mask(),
            battle_bus_slow_death_behavior::BattleBusSlowDeathBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::BattleBusSlowDeathBehavior(m))),
        )),
        ModuleDataKind::RebuildHoleBehavior(d) => Some(made_update(
            rebuild_hole_behavior::RebuildHoleBehavior::get_interface_mask(),
            rebuild_hole_behavior::RebuildHoleBehavior::unconstructed(object, Some(d.clone())).map(|m| (m.update_module().clone(), UpdateBody::RebuildHoleBehavior(m))),
        )),
        // --- end p1w13-sonnet ---
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
/// (test-only) the planted bug that is switched on by `L9_PLANT`, for the differential tests. It exists only with the `differential`
/// feature: in every other build it is the constant `false`, so no environment variable can change the game's logic.
#[cfg(feature = "differential")]
#[doc(hidden)]
pub fn plant(k: u32) -> bool {
    use std::sync::OnceLock;
    static P: OnceLock<u32> = OnceLock::new();
    *P.get_or_init(|| std::env::var("L9_PLANT").ok().and_then(|s| s.parse().ok()).unwrap_or(0)) == k
}

/// Without the `differential` feature no bug is ever planted.
#[cfg(not(feature = "differential"))]
#[doc(hidden)]
#[inline(always)]
pub const fn plant(_k: u32) -> bool {
    false
}

/// The factory's `NewModuleDataProc` for the die modules whose data is `DieModuleData` itself (`DestroyDie`, `KeepObjectDie`).
/// batch R30: the factory's `NewModuleDataProc` for a damage module whose data is `DamageModuleData` (`BoneFXDamage`).
pub fn new_plain_damage_module_data(ini: Option<&mut crate::common::ini::Ini<'_>>) -> crate::common::ini::IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::Damage(std::sync::Arc::new(crate::logic::module::damage_module::DamageModuleData::friend_new_module_data(ini)?)))
}

/// batch R35: the `NewModuleDataProc` of an update module with no data class (`MAKE_STANDARD_MODULE_MACRO`: `Module::friend_newModuleData`, the
/// plain `ModuleData`, which `UpdateModuleData` adds nothing to).
pub fn new_plain_update_module_data(ini: Option<&mut crate::common::ini::Ini<'_>>) -> crate::common::ini::IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::Update(std::sync::Arc::new(crate::logic::module::update_module::UpdateModuleData::friend_new_module_data(ini)?)))
}

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
