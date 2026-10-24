// SPDX-License-Identifier: GPL-3.0-only
//! The behaviour half of an update module: an enum of the concrete modules.
//!
//! # Why an enum, and what goes in it
//!
//! Upstream's `UpdateModule::update()` is virtual: the scheduler calls
//! `u->update()` and the concrete class (`LifetimeUpdate`, `AIUpdate`, ...)
//! runs. No trait objects here, so the concrete modules are **variants of
//! [`UpdateBody`]**, and [`UpdateBody::update`] is a `match` that forwards to
//! the variant's own `update`. The layer-9 agent who translates a module adds
//! one variant and one arm (in a `// --- p1wN-name ---` block), and nothing
//! else in the world changes.
//!
//! Every variant's `update` has the same shape,
//! `fn update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime`:
//! it is **checked out** of its object while it runs, so `&mut self` and
//! `&mut World` do not alias, and it reaches its own object through `me` and
//! the world (`world.objects.find_mut(me.object)`), any other object through
//! its id, and the client through `world.events`.
//!
//! # [`ProbeUpdate`]
//!
//! The one variant that exists now is a scripted module with no upstream
//! counterpart. It is the executable form of the worked example in
//! `docs/world-design.md` and what the differential test drives: on each call
//! it performs a list of actions (destroy an object, wake another module,
//! create an object, deal damage through the cut point, emit a client event)
//! and returns a scripted sleep time. It is part of the library (not
//! `#[cfg(test)]`) because integration tests use it. Production code never
//! constructs one.

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::logic::client_event::ClientEvent;
use crate::logic::damage::DamageInfo;
use crate::logic::module::update_module::{
    UpdateModule, UpdateModuleData, UpdateSleepTime, SleepyUpdatePhase,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::object::{BehaviorSlot, Object};
use crate::logic::world::sleepy::UpdateHandle;
// --- p1w6-object ---
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::UPDATE_SLEEP_FOREVER;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::module_probe::ProbeModule;
// --- end p1w6-object ---

/// The concrete update modules. See the module docs.
// One value per module slot; the containers' variants (`OpenContain` holds its 32 fire-point matrices inline, as upstream) are much larger than
// most. Boxing them would add a heap indirection to every call into a container for a memory saving that does not matter at module counts.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateBody {
    /// The scripted module of the tests and of `docs/world-design.md`.
    Probe(ProbeUpdate),
    // --- p1w6-object ---
    /// A module with every interface and scripted answers (`module_probe.rs`),
    /// for the dispatch of `Object`. It has no update behaviour.
    ModuleProbe(ProbeModule),
    // --- end p1w6-object ---
    // --- p1l9-example ---
    /// `CommandSetUpgrade` (the lead's layer-9 example).
    CommandSetUpgrade(crate::logic::modules::command_set_upgrade::CommandSetUpgrade),
    // --- end p1l9-example ---
// --- p1l9-upgrades ---
    /// `WeaponSetUpgrade`.
    WeaponSetUpgrade(crate::logic::modules::weapon_set_upgrade::WeaponSetUpgrade),
    /// `WeaponBonusUpgrade`.
    WeaponBonusUpgrade(crate::logic::modules::weapon_bonus_upgrade::WeaponBonusUpgrade),
    /// `ArmorUpgrade`.
    ArmorUpgrade(crate::logic::modules::armor_upgrade::ArmorUpgrade),
    /// `ModelConditionUpgrade`.
    ModelConditionUpgrade(crate::logic::modules::model_condition_upgrade::ModelConditionUpgrade),
    /// `ExperienceScalarUpgrade`.
    ExperienceScalarUpgrade(crate::logic::modules::experience_scalar_upgrade::ExperienceScalarUpgrade),
    /// `ActiveShroudUpgrade`.
    ActiveShroudUpgrade(crate::logic::modules::active_shroud_upgrade::ActiveShroudUpgrade),
    /// `MaxHealthUpgrade`.
    MaxHealthUpgrade(crate::logic::modules::max_health_upgrade::MaxHealthUpgrade),
    /// `GrantScienceUpgrade`.
    GrantScienceUpgrade(crate::logic::modules::grant_science_upgrade::GrantScienceUpgrade),
    // --- p1w10-sonnet ---
    /// `StatusBitsUpgrade`.
    StatusBitsUpgrade(crate::logic::modules::status_bits_upgrade::StatusBitsUpgrade),
    /// `LocomotorSetUpgrade`.
    LocomotorSetUpgrade(crate::logic::modules::locomotor_set_upgrade::LocomotorSetUpgrade),
    /// `PassengersFireUpgrade`.
    PassengersFireUpgrade(crate::logic::modules::passengers_fire_upgrade::PassengersFireUpgrade),
    /// `StealthUpgrade`.
    StealthUpgrade(crate::logic::modules::stealth_upgrade::StealthUpgrade),
    /// `SubObjectsUpgrade`.
    SubObjectsUpgrade(crate::logic::modules::sub_objects_upgrade::SubObjectsUpgrade),
    /// `ObjectCreationUpgrade`.
    ObjectCreationUpgrade(crate::logic::modules::object_creation_upgrade::ObjectCreationUpgrade),
    // --- end p1w10-sonnet ---
// --- p1w10-sonnet ---
    /// `PreorderCreate`.
    PreorderCreate(crate::logic::modules::preorder_create::PreorderCreate),
    /// `SpecialPowerCreate`.
    SpecialPowerCreate(crate::logic::modules::special_power_create::SpecialPowerCreate),
    /// `SupplyCenterCreate`.
    SupplyCenterCreate(crate::logic::modules::supply_center_create::SupplyCenterCreate),
    /// `SupplyWarehouseCreate`.
    SupplyWarehouseCreate(crate::logic::modules::supply_warehouse_create::SupplyWarehouseCreate),
    /// `LockWeaponCreate`.
    LockWeaponCreate(crate::logic::modules::lock_weapon_create::LockWeaponCreate),
    /// `VeterancyGainCreate`.
    VeterancyGainCreate(crate::logic::modules::veterancy_gain_create::VeterancyGainCreate),
    /// `GrantUpgradeCreate`.
    GrantUpgradeCreate(crate::logic::modules::grant_upgrade_create::GrantUpgradeCreate),
// --- p1w10-sonnet (update modules) ---
    /// `LifetimeUpdate`.
    LifetimeUpdate(crate::logic::modules::lifetime_update::LifetimeUpdate),
    /// `DeletionUpdate`.
    DeletionUpdate(crate::logic::modules::deletion_update::DeletionUpdate),
    /// `BaseRegenerateUpdate`.
    BaseRegenerateUpdate(crate::logic::modules::base_regenerate_update::BaseRegenerateUpdate),
    /// `SmartBombTargetHomingUpdate`.
    SmartBombTargetHomingUpdate(crate::logic::modules::smart_bomb_target_homing_update::SmartBombTargetHomingUpdate),
// --- p1w10-sonnet (helpers and dies) ---
    /// `ObjectSMCHelper`.
    ObjectSMCHelper(crate::logic::modules::object_smc_helper::ObjectSMCHelper),
    /// `ObjectRepulsorHelper`.
    ObjectRepulsorHelper(crate::logic::modules::object_repulsor_helper::ObjectRepulsorHelper),
    /// `ObjectWeaponStatusHelper`.
    ObjectWeaponStatusHelper(crate::logic::modules::object_weapon_status_helper::ObjectWeaponStatusHelper),
    /// `StatusDamageHelper`.
    StatusDamageHelper(crate::logic::modules::status_damage_helper::StatusDamageHelper),
    /// `TempWeaponBonusHelper`.
    TempWeaponBonusHelper(crate::logic::modules::temp_weapon_bonus_helper::TempWeaponBonusHelper),
    /// `DestroyDie`.
    DestroyDie(crate::logic::modules::destroy_die::DestroyDie),
    /// `BoneFXDamage` (batch R30).
    BoneFxDamage(crate::logic::modules::bone_fx_update::BoneFxDamage),
    /// `TransitionDamageFX` (batch R31).
    TransitionDamageFx(crate::logic::modules::transition_damage_fx::TransitionDamageFx),
    /// `KeepObjectDie`.
    KeepObjectDie(crate::logic::modules::keep_object_die::KeepObjectDie),
    /// `UpgradeDie`.
    UpgradeDie(crate::logic::modules::upgrade_die::UpgradeDie),
// --- end p1w10-sonnet (helpers and dies) ---
// --- end p1w10-sonnet (update modules) ---
// --- end p1w10-sonnet ---
// --- end p1l9-upgrades ---
    // --- p1w11-sonnet ---
    /// `CostModifierUpgrade`.
    CostModifierUpgrade(crate::logic::modules::cost_modifier_upgrade::CostModifierUpgrade),
    /// `PowerPlantUpgrade`.
    PowerPlantUpgrade(crate::logic::modules::power_plant_upgrade::PowerPlantUpgrade),
    /// `RadarUpgrade`.
    RadarUpgrade(crate::logic::modules::radar_upgrade::RadarUpgrade),
    /// `UnpauseSpecialPowerUpgrade`.
    UnpauseSpecialPowerUpgrade(crate::logic::modules::unpause_special_power_upgrade::UnpauseSpecialPowerUpgrade),
    /// `ReplaceObjectUpgrade`.
    ReplaceObjectUpgrade(crate::logic::modules::replace_object_upgrade::ReplaceObjectUpgrade),
    /// `DamDie`.
    DamDie(crate::logic::modules::dam_die::DamDie),
    /// `SpecialPowerCompletionDie`.
    SpecialPowerCompletionDie(crate::logic::modules::special_power_completion_die::SpecialPowerCompletionDie),
    /// `EjectPilotDie`.
    EjectPilotDie(crate::logic::modules::eject_pilot_die::EjectPilotDie),
    /// `InstantDeathBehavior` (batch R16).
    InstantDeathBehavior(crate::logic::modules::instant_death_behavior::InstantDeathBehavior),
    /// `RebuildHoleExposeDie` (batch R15).
    RebuildHoleExposeDie(crate::logic::modules::rebuild_hole_expose_die::RebuildHoleExposeDie),
    /// `CreateCrateDie` (batch R14c).
    CreateCrateDie(crate::logic::modules::create_crate_die::CreateCrateDie),
    /// `CrushDie` (batch R14b).
    CrushDie(crate::logic::modules::crush_die::CrushDie),
    /// `CreateObjectDie` (batch R14a).
    CreateObjectDie(crate::logic::modules::create_object_die::CreateObjectDie),
    /// `PowerPlantUpdate`.
    PowerPlantUpdate(crate::logic::modules::power_plant_update::PowerPlantUpdate),
    /// `RadarUpdate`.
    RadarUpdate(crate::logic::modules::radar_update::RadarUpdate),
    /// `HealCrateCollide`.
    HealCrateCollide(crate::logic::modules::heal_crate_collide::HealCrateCollide),
    /// `ShroudCrateCollide`.
    ShroudCrateCollide(crate::logic::modules::shroud_crate_collide::ShroudCrateCollide),
    /// `MoneyCrateCollide`.
    MoneyCrateCollide(crate::logic::modules::money_crate_collide::MoneyCrateCollide),
    /// The eight sabotage crates (batch R13).
    SabotageCrateCollide(crate::logic::modules::sabotage_crate_collide::SabotageCrateCollide),
    /// `UnitCrateCollide` (batch R13b).
    UnitCrateCollide(crate::logic::modules::unit_crate_collide::UnitCrateCollide),
    /// `VeterancyCrateCollide` (batch R13b).
    VeterancyCrateCollide(crate::logic::modules::veterancy_crate_collide::VeterancyCrateCollide),
    /// `SalvageCrateCollide` (batch R13c).
    SalvageCrateCollide(crate::logic::modules::salvage_crate_collide::SalvageCrateCollide),
    /// `ConvertToCarBombCrateCollide` (batch R13d).
    ConvertToCarBombCrateCollide(crate::logic::modules::convert_to_car_bomb_crate_collide::ConvertToCarBombCrateCollide),
    /// `ConvertToHijackedVehicleCrateCollide` (batch R13e).
    ConvertToHijackedVehicleCrateCollide(crate::logic::modules::convert_to_hijacked_vehicle_crate_collide::ConvertToHijackedVehicleCrateCollide),
    /// `SubdualDamageHelper`.
    SubdualDamageHelper(crate::logic::modules::subdual_damage_helper::SubdualDamageHelper),
    /// `ObjectDefectionHelper`.
    ObjectDefectionHelper(crate::logic::modules::object_defection_helper::ObjectDefectionHelper),
    /// `CleanupAreaPower`.
    CleanupAreaPower(crate::logic::modules::cleanup_area_power::CleanupAreaPower),
    /// `BaikonurLaunchPower`.
    BaikonurLaunchPower(crate::logic::modules::baikonur_launch_power::BaikonurLaunchPower),
    /// `SpyVisionSpecialPower`.
    SpyVisionSpecialPower(crate::logic::modules::spy_vision_special_power::SpyVisionSpecialPower),
    /// `DefectorSpecialPower`.
    DefectorSpecialPower(crate::logic::modules::defector_special_power::DefectorSpecialPower),
    /// `OCLSpecialPower` (batch R22).
    OCLSpecialPower(crate::logic::modules::ocl_special_power::OCLSpecialPower),
    /// `CashHackSpecialPower` (batch R21).
    CashHackSpecialPower(crate::logic::modules::cash_hack_special_power::CashHackSpecialPower),
    /// `CashBountyPower` (batch R21).
    CashBountyPower(crate::logic::modules::cash_bounty_power::CashBountyPower),
    /// `SpecialPowerModule`.
    SpecialPowerModule(crate::logic::modules::special_power_module_impl::SpecialPowerModuleImpl),
    /// `SpyVisionUpdate`.
    SpyVisionUpdate(crate::logic::modules::spy_vision_update::SpyVisionUpdate),
    /// `FXListDie`.
    FXListDie(crate::logic::modules::fx_list_die::FXListDie),
    /// `FireWeaponWhenDeadBehavior`.
    FireWeaponWhenDeadBehavior(crate::logic::modules::fire_weapon_when_dead_behavior::FireWeaponWhenDeadBehavior),
    /// `AutoDepositUpdate`.
    AutoDepositUpdate(crate::logic::modules::auto_deposit_update::AutoDepositUpdate),
    // --- end p1w11-sonnet ---
    // --- p1w12-sonnet ---
    /// `FireOCLAfterWeaponCooldownUpdate`.
    FireOCLAfterWeaponCooldownUpdate(crate::logic::modules::fire_ocl_after_weapon_cooldown_update::FireOCLAfterWeaponCooldownUpdate),
    /// `AutoHealBehavior`.
    AutoHealBehavior(crate::logic::modules::auto_heal_behavior::AutoHealBehavior),
    /// `CheckpointUpdate`.
    CheckpointUpdate(crate::logic::modules::checkpoint_update::CheckpointUpdate),
    /// `FlammableUpdate`.
    FlammableUpdate(crate::logic::modules::flammable_update::FlammableUpdate),
    /// `PoisonedBehavior` (batch R16).
    PoisonedBehavior(crate::logic::modules::poisoned_behavior::PoisonedBehavior),
    /// `WeaponBonusUpdate` (batch R17).
    WeaponBonusUpdate(crate::logic::modules::weapon_bonus_update::WeaponBonusUpdate),
    /// `PilotFindVehicleUpdate` (batch R19).
    PilotFindVehicleUpdate(crate::logic::modules::pilot_find_vehicle_update::PilotFindVehicleUpdate),
    /// `HordeUpdate` (batch R25).
    HordeUpdate(crate::logic::modules::horde_update::HordeUpdate),
    /// `ToppleUpdate` (batch R28).
    ToppleUpdate(crate::logic::modules::topple_update::ToppleUpdate),
    /// `PropagandaTowerBehavior` (batch R37).
    PropagandaTowerBehavior(crate::logic::modules::propaganda_tower_behavior::PropagandaTowerBehavior),
    /// `StructureToppleUpdate` (batch R36).
    StructureToppleUpdate(crate::logic::modules::structure_topple_update::StructureToppleUpdate),
    /// `NeutronBlastBehavior` (batch R35).
    NeutronBlastBehavior(crate::logic::modules::neutron_blast_behavior::NeutronBlastBehavior),
    /// `BridgeScaffoldBehavior` (batch R35).
    BridgeScaffoldBehavior(crate::logic::modules::bridge_scaffold_behavior::BridgeScaffoldBehavior),
    /// `TensileFormationUpdate` (batch R34).
    TensileFormationUpdate(crate::logic::modules::tensile_formation_update::TensileFormationUpdate),
    /// `LeafletDropBehavior` (batch R33).
    LeafletDropBehavior(crate::logic::modules::emp_update::LeafletDropBehavior),
    /// `EmpUpdate` (batch R33).
    EmpUpdate(crate::logic::modules::emp_update::EmpUpdate),
    /// `BoneFxUpdate` (batch R30).
    BoneFxUpdate(crate::logic::modules::bone_fx_update::BoneFxUpdate),
    /// `StructureCollapseUpdate` (batch R29).
    StructureCollapseUpdate(crate::logic::modules::structure_collapse_update::StructureCollapseUpdate),
    /// `StealthDetectorUpdate` (batch R27).
    StealthDetectorUpdate(crate::logic::modules::stealth_detector_update::StealthDetectorUpdate),
    /// `StealthUpdate` (batch R26).
    StealthUpdate(crate::logic::modules::stealth_update::StealthUpdate),
    /// `OverchargeBehavior` (batch R24).
    OverchargeBehavior(crate::logic::modules::overcharge_behavior::OverchargeBehavior),
    /// `GrantStealthBehavior` (batch R23).
    GrantStealthBehavior(crate::logic::modules::grant_stealth_behavior::GrantStealthBehavior),
    /// `DynamicGeometryInfoUpdate` (batch R23).
    DynamicGeometryInfoUpdate(crate::logic::modules::dynamic_geometry_info_update::DynamicGeometryInfoUpdate),
    /// `FirestormDynamicGeometryInfoUpdate` (batch R32).
    FirestormDynamicGeometryInfoUpdate(crate::logic::modules::firestorm_dynamic_geometry_info_update::FirestormDynamicGeometryInfoUpdate),
    /// `AnimationSteeringUpdate` (batch R20).
    AnimationSteeringUpdate(crate::logic::modules::animation_steering_update::AnimationSteeringUpdate),
    /// `RadiusDecalUpdate` (batch R20).
    RadiusDecalUpdate(crate::logic::modules::radius_decal_update::RadiusDecalUpdate),
    /// `AutoFindHealingUpdate` (batch R19).
    AutoFindHealingUpdate(crate::logic::modules::auto_find_healing_update::AutoFindHealingUpdate),
    /// `QueueProductionExitUpdate` (batch R18).
    QueueProductionExitUpdate(crate::logic::modules::queue_production_exit_update::QueueProductionExitUpdate),
    /// `DefaultProductionExitUpdate` (batch R18).
    DefaultProductionExitUpdate(crate::logic::modules::default_production_exit_update::DefaultProductionExitUpdate),
    /// `SupplyWarehouseCripplingBehavior` (batch R17).
    SupplyWarehouseCripplingBehavior(crate::logic::modules::supply_warehouse_crippling_behavior::SupplyWarehouseCripplingBehavior),
    /// `OclUpdate`.
    OclUpdate(crate::logic::modules::ocl_update::OclUpdate),
    // --- end p1w12-sonnet ---
    // --- p1w13-sonnet ---
    /// `HeightDieUpdate`.
    HeightDieUpdate(crate::logic::modules::height_die_update::HeightDieUpdate),
    /// `DynamicShroudClearingRangeUpdate`.
    DynamicShroudClearingRangeUpdate(crate::logic::modules::dynamic_shroud_clearing_range_update::DynamicShroudClearingRangeUpdate),
    /// `SquishCollide`.
    SquishCollide(crate::logic::modules::squish_collide::SquishCollide),
    /// `ProneUpdate`.
    ProneUpdate(crate::logic::modules::prone_update::ProneUpdate),
    /// `EnemyNearUpdate`.
    EnemyNearUpdate(crate::logic::modules::enemy_near_update::EnemyNearUpdate),
    /// `FloatUpdate`.
    FloatUpdate(crate::logic::modules::float_update::FloatUpdate),
    /// `HijackerUpdate` (batch R13e).
    HijackerUpdate(crate::logic::modules::hijacker_update::HijackerUpdate),
    /// `TechBuildingBehavior`.
    TechBuildingBehavior(crate::logic::modules::tech_building_behavior::TechBuildingBehavior),
    /// `FireSpreadUpdate`.
    FireSpreadUpdate(crate::logic::modules::fire_spread_update::FireSpreadUpdate),
    /// `ActiveBody`.
    ActiveBody(crate::logic::modules::active_body::ActiveBody),
    /// `HighlanderBody`.
    HighlanderBody(crate::logic::modules::derived_bodies::HighlanderBody),
    /// `ImmortalBody`.
    ImmortalBody(crate::logic::modules::derived_bodies::ImmortalBody),
    /// `StructureBody`.
    StructureBody(crate::logic::modules::derived_bodies::StructureBody),
    /// `UndeadBody`.
    UndeadBody(crate::logic::modules::derived_bodies::UndeadBody),
    /// `HiveStructureBody`.
    HiveStructureBody(crate::logic::modules::derived_bodies::HiveStructureBody),
    /// `InactiveBody`.
    InactiveBody(crate::logic::modules::inactive_body::InactiveBody),
    /// `SlowDeathBehavior`.
    SlowDeathBehavior(crate::logic::modules::slow_death_behavior::SlowDeathBehavior),
    /// `RebuildHoleBehavior` (batch R15).
    RebuildHoleBehavior(crate::logic::modules::rebuild_hole_behavior::RebuildHoleBehavior),
    /// `OpenContain`.
    OpenContain(crate::logic::modules::open_contain::OpenContain),
    /// `TransportContain`.
    TransportContain(crate::logic::modules::transport_contain::TransportContain),
    /// `GarrisonContain`.
    GarrisonContain(crate::logic::modules::garrison_contain::GarrisonContain),
    /// `CaveContain`.
    CaveContain(crate::logic::modules::cave_contain::CaveContain),
    /// `TunnelContain`.
    TunnelContain(crate::logic::modules::tunnel_contain::TunnelContain),
    /// `MobNexusContain`.
    MobNexusContain(crate::logic::modules::mob_nexus_contain::MobNexusContain),
    /// `ParachuteContain`.
    ParachuteContain(crate::logic::modules::parachute_contain::ParachuteContain),
    /// `HealContain`.
    HealContain(crate::logic::modules::heal_contain::HealContain),
    /// `OverlordContain`.
    OverlordContain(crate::logic::modules::overlord_contain::OverlordContain),
    /// `RiderChangeContain`.
    RiderChangeContain(crate::logic::modules::rider_change_contain::RiderChangeContain),
    /// `HelixContain`.
    HelixContain(crate::logic::modules::helix_contain::HelixContain),
    /// `RailedTransportContain`.
    RailedTransportContain(crate::logic::modules::railed_transport_contain::RailedTransportContain),
    /// `InternetHackContain`.
    InternetHackContain(crate::logic::modules::internet_hack_contain::InternetHackContain),
    // --- end p1w13-sonnet ---
    // New variants go here, one per translated module.
}

impl UpdateBody {
    /// `UpdateModuleInterface::update()`: the concrete module's update.
    pub fn update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        match self {
            UpdateBody::Probe(p) => p.update(world, me),
            // --- p1w6-object ---
            UpdateBody::ModuleProbe(_) => UPDATE_SLEEP_FOREVER,
            // --- end p1w6-object ---
            // --- p1l9-example ---
            // not an update module: the scheduler never holds it
            UpdateBody::CommandSetUpgrade(_) => UPDATE_SLEEP_FOREVER,
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::WeaponBonusUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ArmorUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ModelConditionUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ExperienceScalarUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ActiveShroudUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::MaxHealthUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::GrantScienceUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::StatusBitsUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::LocomotorSetUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::PassengersFireUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::StealthUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SubObjectsUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ObjectCreationUpgrade(_) => UPDATE_SLEEP_FOREVER,
// --- p1w10-sonnet ---
            UpdateBody::PreorderCreate(_) => UPDATE_SLEEP_FOREVER,
// --- p1w10-sonnet (helpers and dies) ---
            UpdateBody::ObjectSMCHelper(m) => m.world_update(world, me),
            UpdateBody::ObjectRepulsorHelper(m) => m.world_update(world, me),
            UpdateBody::ObjectWeaponStatusHelper(m) => m.world_update(world, me),
            UpdateBody::StatusDamageHelper(m) => m.world_update(world, me),
            UpdateBody::TempWeaponBonusHelper(m) => m.world_update(world, me),
            UpdateBody::DestroyDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::BoneFxDamage(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::TransitionDamageFx(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::KeepObjectDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::UpgradeDie(_) => UPDATE_SLEEP_FOREVER,
// --- end p1w10-sonnet (helpers and dies) ---
// --- p1w10-sonnet (update modules) ---
            UpdateBody::LifetimeUpdate(m) => m.world_update(world, me),
            UpdateBody::DeletionUpdate(m) => m.world_update(world, me),
            UpdateBody::BaseRegenerateUpdate(m) => m.world_update(world, me),
            UpdateBody::SmartBombTargetHomingUpdate(m) => m.world_update(world, me),
// --- end p1w10-sonnet (update modules) ---
            UpdateBody::SpecialPowerCreate(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SupplyCenterCreate(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SupplyWarehouseCreate(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::LockWeaponCreate(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::VeterancyGainCreate(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::GrantUpgradeCreate(_) => UPDATE_SLEEP_FOREVER,
// --- end p1w10-sonnet ---
// --- end p1l9-upgrades ---
// --- p1w11-sonnet ---
            UpdateBody::CostModifierUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::PowerPlantUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::RadarUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::UnpauseSpecialPowerUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ReplaceObjectUpgrade(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::DamDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SpecialPowerCompletionDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::EjectPilotDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::InstantDeathBehavior(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::RebuildHoleExposeDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::CreateCrateDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::CrushDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::CreateObjectDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::PowerPlantUpdate(m) => m.world_update(world, me),
            UpdateBody::RadarUpdate(m) => m.world_update(world, me),
            UpdateBody::HealCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ShroudCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::MoneyCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SabotageCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::UnitCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::VeterancyCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SalvageCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ConvertToCarBombCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ConvertToHijackedVehicleCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SubdualDamageHelper(m) => m.world_update(world, me),
            UpdateBody::ObjectDefectionHelper(m) => m.world_update(world, me),
            UpdateBody::CleanupAreaPower(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::BaikonurLaunchPower(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SpyVisionSpecialPower(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::DefectorSpecialPower(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::OCLSpecialPower(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::CashHackSpecialPower(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::CashBountyPower(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SpecialPowerModule(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SpyVisionUpdate(m) => m.world_update(world, me),
            UpdateBody::FXListDie(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::FireWeaponWhenDeadBehavior(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::AutoDepositUpdate(m) => m.world_update(world, me),
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => m.world_update(world, me),
            UpdateBody::AutoHealBehavior(m) => m.world_update(world, me),
            UpdateBody::CheckpointUpdate(m) => m.world_update(world, me),
            UpdateBody::FlammableUpdate(m) => m.world_update(world, me),
            UpdateBody::PoisonedBehavior(m) => m.world_update(world, me),
            UpdateBody::WeaponBonusUpdate(m) => m.world_update(world, me),
            UpdateBody::PilotFindVehicleUpdate(m) => m.world_update(world, me),
            UpdateBody::HordeUpdate(m) => m.world_update(world, me),
            UpdateBody::ToppleUpdate(m) => m.world_update(world, me),
            UpdateBody::PropagandaTowerBehavior(m) => m.world_update(world, me),
            UpdateBody::StructureToppleUpdate(m) => m.world_update(world, me),
            UpdateBody::NeutronBlastBehavior(m) => m.world_update(world, me),
            UpdateBody::BridgeScaffoldBehavior(m) => m.world_update(world, me),
            UpdateBody::TensileFormationUpdate(m) => m.world_update(world, me),
            UpdateBody::LeafletDropBehavior(m) => m.world_update(world, me),
            UpdateBody::EmpUpdate(m) => m.world_update(world, me),
            UpdateBody::BoneFxUpdate(m) => m.world_update(world, me),
            UpdateBody::StructureCollapseUpdate(m) => m.world_update(world, me),
            UpdateBody::StealthDetectorUpdate(m) => m.world_update(world, me),
            UpdateBody::StealthUpdate(m) => m.world_update(world, me),
            UpdateBody::OverchargeBehavior(m) => m.world_update(world, me),
            UpdateBody::GrantStealthBehavior(m) => m.world_update(world, me),
            UpdateBody::DynamicGeometryInfoUpdate(m) => m.world_update(world, me),
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => m.world_update(world, me),
            UpdateBody::AnimationSteeringUpdate(m) => m.world_update(world, me),
            UpdateBody::RadiusDecalUpdate(m) => m.world_update(world, me),
            UpdateBody::AutoFindHealingUpdate(m) => m.world_update(world, me),
            UpdateBody::QueueProductionExitUpdate(m) => m.world_update(world, me),
            UpdateBody::DefaultProductionExitUpdate(m) => m.world_update(world, me),
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => m.world_update(world, me),
            UpdateBody::OclUpdate(m) => m.world_update(world, me),
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
            UpdateBody::HeightDieUpdate(m) => m.world_update(world, me),
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => m.world_update(world, me),
            UpdateBody::SquishCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ProneUpdate(m) => m.world_update(world, me),
            UpdateBody::EnemyNearUpdate(m) => m.world_update(world, me),
            UpdateBody::FloatUpdate(m) => m.world_update(world, me),
            UpdateBody::HijackerUpdate(m) => m.world_update(world, me),
            UpdateBody::TechBuildingBehavior(m) => m.world_update(world, me),
            UpdateBody::FireSpreadUpdate(m) => m.world_update(world, me),
            UpdateBody::ActiveBody(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::HighlanderBody(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ImmortalBody(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::StructureBody(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::UndeadBody(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::HiveStructureBody(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::InactiveBody(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::SlowDeathBehavior(m) => m.world_update(world, me),
            UpdateBody::RebuildHoleBehavior(m) => m.world_update(world, me),
            UpdateBody::OpenContain(m) => m.world_update(world, me),
            UpdateBody::TransportContain(m) => m.world_update(world, me),
            UpdateBody::GarrisonContain(m) => m.world_update(world, me),
            UpdateBody::CaveContain(m) => m.world_update(world, me),
            UpdateBody::TunnelContain(m) => m.world_update(world, me),
            UpdateBody::MobNexusContain(m) => m.world_update(world, me),
            UpdateBody::ParachuteContain(m) => m.world_update(world, me),
            UpdateBody::HealContain(m) => m.world_update(world, me),
            UpdateBody::OverlordContain(m) => m.world_update(world, me),
            UpdateBody::RiderChangeContain(m) => m.world_update(world, me),
            UpdateBody::HelixContain(m) => m.world_update(world, me),
            UpdateBody::RailedTransportContain(m) => m.world_update(world, me),
            UpdateBody::InternetHackContain(m) => m.world_update(world, me),
// --- end p1w13-sonnet ---
            // --- end p1l9-example ---
        }
    }

    // --- p1w6-object ---
    /// The interface calls of `Object` (see `module_call.rs`): the body's
    /// answer to `call`. A module answers [`ModuleReply::Unhandled`] to a call
    /// it has no behaviour for (upstream's empty virtual).
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match self {
            UpdateBody::Probe(_) => ModuleReply::Unhandled,
            UpdateBody::ModuleProbe(m) => m.handle(world, me, call),
            // --- p1l9-example ---
            UpdateBody::CommandSetUpgrade(m) => m.module_call(world, me, call),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::WeaponBonusUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ArmorUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ModelConditionUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ExperienceScalarUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ActiveShroudUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::MaxHealthUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::GrantScienceUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::StatusBitsUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::LocomotorSetUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::PassengersFireUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::StealthUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::SubObjectsUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ObjectCreationUpgrade(m) => m.module_call(world, me, call),
// --- p1w10-sonnet ---
            UpdateBody::PreorderCreate(m) => m.module_call(world, me, call),
// --- p1w10-sonnet (helpers and dies) ---
            UpdateBody::ObjectSMCHelper(m) => m.module_call(world, me, call),
            UpdateBody::ObjectRepulsorHelper(m) => m.module_call(world, me, call),
            UpdateBody::ObjectWeaponStatusHelper(m) => m.module_call(world, me, call),
            UpdateBody::StatusDamageHelper(m) => m.module_call(world, me, call),
            UpdateBody::TempWeaponBonusHelper(m) => m.module_call(world, me, call),
            UpdateBody::DestroyDie(m) => m.module_call(world, me, call),
            UpdateBody::BoneFxDamage(m) => m.module_call(world, me, call),
            UpdateBody::TransitionDamageFx(m) => m.module_call(world, me, call),
            UpdateBody::KeepObjectDie(m) => m.module_call(world, me, call),
            UpdateBody::UpgradeDie(m) => m.module_call(world, me, call),
// --- end p1w10-sonnet (helpers and dies) ---
// --- p1w10-sonnet (update modules) ---
            UpdateBody::LifetimeUpdate(m) => m.module_call(world, me, call),
            UpdateBody::DeletionUpdate(m) => m.module_call(world, me, call),
            UpdateBody::BaseRegenerateUpdate(m) => m.module_call(world, me, call),
            UpdateBody::SmartBombTargetHomingUpdate(m) => m.module_call(world, me, call),
// --- end p1w10-sonnet (update modules) ---
            UpdateBody::SpecialPowerCreate(m) => m.module_call(world, me, call),
            UpdateBody::SupplyCenterCreate(m) => m.module_call(world, me, call),
            UpdateBody::SupplyWarehouseCreate(m) => m.module_call(world, me, call),
            UpdateBody::LockWeaponCreate(m) => m.module_call(world, me, call),
            UpdateBody::VeterancyGainCreate(m) => m.module_call(world, me, call),
            UpdateBody::GrantUpgradeCreate(m) => m.module_call(world, me, call),
// --- end p1w10-sonnet ---
// --- end p1l9-upgrades ---
// --- p1w11-sonnet ---
            UpdateBody::CostModifierUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::PowerPlantUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::RadarUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::UnpauseSpecialPowerUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::ReplaceObjectUpgrade(m) => m.module_call(world, me, call),
            UpdateBody::DamDie(m) => m.module_call(world, me, call),
            UpdateBody::SpecialPowerCompletionDie(m) => m.module_call(world, me, call),
            UpdateBody::EjectPilotDie(m) => m.module_call(world, me, call),
            UpdateBody::InstantDeathBehavior(m) => m.module_call(world, me, call),
            UpdateBody::RebuildHoleExposeDie(m) => m.module_call(world, me, call),
            UpdateBody::CreateCrateDie(m) => m.module_call(world, me, call),
            UpdateBody::CrushDie(m) => m.module_call(world, me, call),
            UpdateBody::CreateObjectDie(m) => m.module_call(world, me, call),
            UpdateBody::PowerPlantUpdate(m) => m.module_call(world, me, call),
            UpdateBody::RadarUpdate(m) => m.module_call(world, me, call),
            UpdateBody::HealCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::ShroudCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::MoneyCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::SabotageCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::UnitCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::VeterancyCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::SalvageCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::ConvertToCarBombCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::ConvertToHijackedVehicleCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::SubdualDamageHelper(m) => m.module_call(world, me, call),
            UpdateBody::ObjectDefectionHelper(m) => m.module_call(world, me, call),
            UpdateBody::CleanupAreaPower(m) => m.module_call(world, me, call),
            UpdateBody::BaikonurLaunchPower(m) => m.module_call(world, me, call),
            UpdateBody::SpyVisionSpecialPower(m) => m.module_call(world, me, call),
            UpdateBody::DefectorSpecialPower(m) => m.module_call(world, me, call),
            UpdateBody::OCLSpecialPower(m) => m.module_call(world, me, call),
            UpdateBody::CashHackSpecialPower(m) => m.module_call(world, me, call),
            UpdateBody::CashBountyPower(m) => m.module_call(world, me, call),
            UpdateBody::SpecialPowerModule(m) => m.module_call(world, me, call),
            UpdateBody::SpyVisionUpdate(m) => m.module_call(world, me, call),
            UpdateBody::FXListDie(m) => m.module_call(world, me, call),
            UpdateBody::FireWeaponWhenDeadBehavior(m) => m.module_call(world, me, call),
            UpdateBody::AutoDepositUpdate(m) => m.module_call(world, me, call),
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => m.module_call(world, me, call),
            UpdateBody::AutoHealBehavior(m) => m.module_call(world, me, call),
            UpdateBody::CheckpointUpdate(m) => m.module_call(world, me, call),
            UpdateBody::FlammableUpdate(m) => m.module_call(world, me, call),
            UpdateBody::PoisonedBehavior(m) => m.module_call(world, me, call),
            UpdateBody::WeaponBonusUpdate(m) => m.module_call(world, me, call),
            UpdateBody::PilotFindVehicleUpdate(m) => m.module_call(world, me, call),
            UpdateBody::HordeUpdate(m) => m.module_call(world, me, call),
            UpdateBody::ToppleUpdate(m) => m.module_call(world, me, call),
            UpdateBody::PropagandaTowerBehavior(m) => m.module_call(world, me, call),
            UpdateBody::StructureToppleUpdate(m) => m.module_call(world, me, call),
            UpdateBody::NeutronBlastBehavior(m) => m.module_call(world, me, call),
            UpdateBody::BridgeScaffoldBehavior(m) => m.module_call(world, me, call),
            UpdateBody::TensileFormationUpdate(m) => m.module_call(world, me, call),
            UpdateBody::LeafletDropBehavior(m) => m.module_call(world, me, call),
            UpdateBody::EmpUpdate(m) => m.module_call(world, me, call),
            UpdateBody::BoneFxUpdate(m) => m.module_call(world, me, call),
            UpdateBody::StructureCollapseUpdate(m) => m.module_call(world, me, call),
            UpdateBody::StealthDetectorUpdate(m) => m.module_call(world, me, call),
            UpdateBody::StealthUpdate(m) => m.module_call(world, me, call),
            UpdateBody::OverchargeBehavior(m) => m.module_call(world, me, call),
            UpdateBody::GrantStealthBehavior(m) => m.module_call(world, me, call),
            UpdateBody::DynamicGeometryInfoUpdate(m) => m.module_call(world, me, call),
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => m.module_call(world, me, call),
            UpdateBody::AnimationSteeringUpdate(m) => m.module_call(world, me, call),
            UpdateBody::RadiusDecalUpdate(m) => m.module_call(world, me, call),
            UpdateBody::AutoFindHealingUpdate(m) => m.module_call(world, me, call),
            UpdateBody::QueueProductionExitUpdate(m) => m.module_call(world, me, call),
            UpdateBody::DefaultProductionExitUpdate(m) => m.module_call(world, me, call),
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => m.module_call(world, me, call),
            UpdateBody::OclUpdate(m) => m.module_call(world, me, call),
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
            UpdateBody::HeightDieUpdate(m) => m.module_call(world, me, call),
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => m.module_call(world, me, call),
            UpdateBody::SquishCollide(m) => m.module_call(world, me, call),
            UpdateBody::ProneUpdate(m) => m.module_call(world, me, call),
            UpdateBody::EnemyNearUpdate(m) => m.module_call(world, me, call),
            UpdateBody::FloatUpdate(m) => m.module_call(world, me, call),
            UpdateBody::HijackerUpdate(m) => m.module_call(world, me, call),
            UpdateBody::TechBuildingBehavior(m) => m.module_call(world, me, call),
            UpdateBody::FireSpreadUpdate(m) => m.module_call(world, me, call),
            UpdateBody::ActiveBody(m) => m.module_call(world, me, call),
            UpdateBody::HighlanderBody(m) => m.module_call(world, me, call),
            UpdateBody::ImmortalBody(m) => m.module_call(world, me, call),
            UpdateBody::StructureBody(m) => m.module_call(world, me, call),
            UpdateBody::UndeadBody(m) => m.module_call(world, me, call),
            UpdateBody::HiveStructureBody(m) => m.module_call(world, me, call),
            UpdateBody::InactiveBody(m) => m.module_call(world, me, call),
            UpdateBody::SlowDeathBehavior(m) => m.module_call(world, me, call),
            UpdateBody::RebuildHoleBehavior(m) => m.module_call(world, me, call),
            UpdateBody::OpenContain(m) => m.module_call(world, me, call),
            UpdateBody::TransportContain(m) => m.module_call(world, me, call),
            UpdateBody::GarrisonContain(m) => m.module_call(world, me, call),
            UpdateBody::CaveContain(m) => m.module_call(world, me, call),
            UpdateBody::TunnelContain(m) => m.module_call(world, me, call),
            UpdateBody::MobNexusContain(m) => m.module_call(world, me, call),
            UpdateBody::ParachuteContain(m) => m.module_call(world, me, call),
            UpdateBody::HealContain(m) => m.module_call(world, me, call),
            UpdateBody::OverlordContain(m) => m.module_call(world, me, call),
            UpdateBody::RiderChangeContain(m) => m.module_call(world, me, call),
            UpdateBody::HelixContain(m) => m.module_call(world, me, call),
            UpdateBody::RailedTransportContain(m) => m.module_call(world, me, call),
            UpdateBody::InternetHackContain(m) => m.module_call(world, me, call),
// --- end p1w13-sonnet ---
            // --- end p1l9-example ---
        }
    }
    // --- end p1w6-object ---
}

// --- p1w6-object ---
// --- p1l9-example ---
impl UpdateBody {
    /// Whether the concrete module saves its whole base chain itself, in
    /// upstream's order (every translated layer-9 module), rather than the
    /// slot saving the bases first (the probes). Upstream's
    /// `X::xfer` writes `X`'s version, then calls `UpdateModule::xfer` (the
    /// scheduling half) or `UpgradeModule::xfer`, then `X`'s fields; a slot
    /// that wrote the scheduling half before the module would put the bytes
    /// in the wrong order.
    pub fn owns_base_chain(&self) -> bool {
        !matches!(self, UpdateBody::Probe(_) | UpdateBody::ModuleProbe(_))
    }

    /// The whole chain's `xfer` for a module that [owns it](UpdateBody::owns_base_chain):
    /// `update` is the scheduling half (`UpdateModule`) for an update module,
    /// `None` for any other.
    pub fn xfer_chain(&mut self, update: Option<&mut UpdateModule>, xfer: &mut dyn Xfer) -> XferResult<()> {
        let _ = &update; // used by the update modules' arms
        match self {
            UpdateBody::CommandSetUpgrade(m) => m.xfer(xfer),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.xfer(xfer),
            UpdateBody::WeaponBonusUpgrade(m) => m.xfer(xfer),
            UpdateBody::ArmorUpgrade(m) => m.xfer(xfer),
            UpdateBody::ModelConditionUpgrade(m) => m.xfer(xfer),
            UpdateBody::ExperienceScalarUpgrade(m) => m.xfer(xfer),
            UpdateBody::ActiveShroudUpgrade(m) => m.xfer(xfer),
            UpdateBody::MaxHealthUpgrade(m) => m.xfer(xfer),
            UpdateBody::GrantScienceUpgrade(m) => m.xfer(xfer),
            UpdateBody::StatusBitsUpgrade(m) => m.xfer(xfer),
            UpdateBody::LocomotorSetUpgrade(m) => m.xfer(xfer),
            UpdateBody::PassengersFireUpgrade(m) => m.xfer(xfer),
            UpdateBody::StealthUpgrade(m) => m.xfer(xfer),
            UpdateBody::SubObjectsUpgrade(m) => m.xfer(xfer),
            UpdateBody::ObjectCreationUpgrade(m) => m.xfer(xfer),
// --- p1w10-sonnet ---
            UpdateBody::PreorderCreate(m) => m.xfer(xfer),
// --- p1w10-sonnet (helpers and dies) ---
            UpdateBody::ObjectSMCHelper(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::ObjectRepulsorHelper(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::ObjectWeaponStatusHelper(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::StatusDamageHelper(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::TempWeaponBonusHelper(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::DestroyDie(m) => m.xfer(xfer),
            UpdateBody::BoneFxDamage(m) => m.xfer(xfer),
            UpdateBody::TransitionDamageFx(m) => m.xfer(xfer),
            UpdateBody::KeepObjectDie(m) => m.xfer(xfer),
            UpdateBody::UpgradeDie(m) => m.xfer(xfer),
// --- end p1w10-sonnet (helpers and dies) ---
// --- p1w10-sonnet (update modules) ---
            UpdateBody::LifetimeUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::DeletionUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::BaseRegenerateUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::SmartBombTargetHomingUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
// --- end p1w10-sonnet (update modules) ---
            UpdateBody::SpecialPowerCreate(m) => m.xfer(xfer),
            UpdateBody::SupplyCenterCreate(m) => m.xfer(xfer),
            UpdateBody::SupplyWarehouseCreate(m) => m.xfer(xfer),
            UpdateBody::LockWeaponCreate(m) => m.xfer(xfer),
            UpdateBody::VeterancyGainCreate(m) => m.xfer(xfer),
            UpdateBody::GrantUpgradeCreate(m) => m.xfer(xfer),
// --- end p1w10-sonnet ---
// --- end p1l9-upgrades ---
// --- p1w11-sonnet ---
            UpdateBody::PowerPlantUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::RadarUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::SubdualDamageHelper(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::ObjectDefectionHelper(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::SpyVisionUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::AutoDepositUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::AutoHealBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::CheckpointUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::FlammableUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::PoisonedBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::WeaponBonusUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::PilotFindVehicleUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::HordeUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::ToppleUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::PropagandaTowerBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::StructureToppleUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::NeutronBlastBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::BridgeScaffoldBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::TensileFormationUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::LeafletDropBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::EmpUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::BoneFxUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::StructureCollapseUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::StealthDetectorUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::StealthUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::OverchargeBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::GrantStealthBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::DynamicGeometryInfoUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::AnimationSteeringUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::RadiusDecalUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::AutoFindHealingUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::QueueProductionExitUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::DefaultProductionExitUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::OclUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
            UpdateBody::HeightDieUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::ProneUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::EnemyNearUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::FloatUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::HijackerUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::TechBuildingBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::FireSpreadUpdate(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::SlowDeathBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::RebuildHoleBehavior(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::OpenContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::TransportContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::GarrisonContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::CaveContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::TunnelContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::MobNexusContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::ParachuteContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::HealContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::OverlordContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::RiderChangeContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::HelixContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::RailedTransportContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
            UpdateBody::InternetHackContain(m) => match update { Some(u) => m.xfer_with(u, xfer), None => m.xfer(xfer) },
// --- end p1w13-sonnet ---
            _ => self.xfer(xfer),
        }
    }

    /// The whole chain's `crc` (see [`UpdateBody::xfer_chain`]).
    pub fn crc_chain(&mut self, update: Option<&mut UpdateModule>, xfer: &mut dyn Xfer) -> XferResult<()> {
        let _ = &update;
        match self {
            UpdateBody::CommandSetUpgrade(m) => m.crc(xfer),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.crc(xfer),
            UpdateBody::WeaponBonusUpgrade(m) => m.crc(xfer),
            UpdateBody::ArmorUpgrade(m) => m.crc(xfer),
            UpdateBody::ModelConditionUpgrade(m) => m.crc(xfer),
            UpdateBody::ExperienceScalarUpgrade(m) => m.crc(xfer),
            UpdateBody::ActiveShroudUpgrade(m) => m.crc(xfer),
            UpdateBody::MaxHealthUpgrade(m) => m.crc(xfer),
            UpdateBody::GrantScienceUpgrade(m) => m.crc(xfer),
            UpdateBody::StatusBitsUpgrade(m) => m.crc(xfer),
            UpdateBody::LocomotorSetUpgrade(m) => m.crc(xfer),
            UpdateBody::PassengersFireUpgrade(m) => m.crc(xfer),
            UpdateBody::StealthUpgrade(m) => m.crc(xfer),
            UpdateBody::SubObjectsUpgrade(m) => m.crc(xfer),
            UpdateBody::ObjectCreationUpgrade(m) => m.crc(xfer),
// --- p1w10-sonnet ---
            UpdateBody::PreorderCreate(m) => m.crc(xfer),
// --- p1w10-sonnet (helpers and dies) ---
            UpdateBody::ObjectSMCHelper(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::ObjectRepulsorHelper(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::ObjectWeaponStatusHelper(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::StatusDamageHelper(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::TempWeaponBonusHelper(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::DestroyDie(m) => m.crc(xfer),
            UpdateBody::BoneFxDamage(m) => m.crc(xfer),
            UpdateBody::TransitionDamageFx(m) => m.crc(xfer),
            UpdateBody::KeepObjectDie(m) => m.crc(xfer),
            UpdateBody::UpgradeDie(m) => m.crc(xfer),
// --- end p1w10-sonnet (helpers and dies) ---
// --- p1w10-sonnet (update modules) ---
            UpdateBody::LifetimeUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::DeletionUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::BaseRegenerateUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::SmartBombTargetHomingUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
// --- end p1w10-sonnet (update modules) ---
            UpdateBody::SpecialPowerCreate(m) => m.crc(xfer),
            UpdateBody::SupplyCenterCreate(m) => m.crc(xfer),
            UpdateBody::SupplyWarehouseCreate(m) => m.crc(xfer),
            UpdateBody::LockWeaponCreate(m) => m.crc(xfer),
            UpdateBody::VeterancyGainCreate(m) => m.crc(xfer),
            UpdateBody::GrantUpgradeCreate(m) => m.crc(xfer),
// --- end p1w10-sonnet ---
// --- end p1l9-upgrades ---
// --- p1w11-sonnet ---
            UpdateBody::PowerPlantUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::RadarUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::SubdualDamageHelper(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::ObjectDefectionHelper(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::SpyVisionUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::AutoDepositUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::AutoHealBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::CheckpointUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::FlammableUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::PoisonedBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::WeaponBonusUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::PilotFindVehicleUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::HordeUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::ToppleUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::PropagandaTowerBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::StructureToppleUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::NeutronBlastBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::BridgeScaffoldBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::TensileFormationUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::LeafletDropBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::EmpUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::BoneFxUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::StructureCollapseUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::StealthDetectorUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::StealthUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::OverchargeBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::GrantStealthBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::DynamicGeometryInfoUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::AnimationSteeringUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::RadiusDecalUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::AutoFindHealingUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::QueueProductionExitUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::DefaultProductionExitUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::OclUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
            UpdateBody::HeightDieUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::ProneUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::EnemyNearUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::FloatUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::HijackerUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::TechBuildingBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::FireSpreadUpdate(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::SlowDeathBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::RebuildHoleBehavior(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::OpenContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::TransportContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::GarrisonContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::CaveContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::TunnelContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::MobNexusContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::ParachuteContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::HealContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::OverlordContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::RiderChangeContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::HelixContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::RailedTransportContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
            UpdateBody::InternetHackContain(m) => match update { Some(u) => m.crc_with(u, xfer), None => m.crc(xfer) },
// --- end p1w13-sonnet ---
            _ => self.crc(xfer),
        }
    }

    /// The whole chain's `loadPostProcess` (see [`UpdateBody::xfer_chain`]).
    pub fn load_post_process_chain(&mut self, update: Option<&mut UpdateModule>) {
        let _ = &update;
        match self {
            UpdateBody::CommandSetUpgrade(m) => m.load_post_process(),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.load_post_process(),
            UpdateBody::WeaponBonusUpgrade(m) => m.load_post_process(),
            UpdateBody::ArmorUpgrade(m) => m.load_post_process(),
            UpdateBody::ModelConditionUpgrade(m) => m.load_post_process(),
            UpdateBody::ExperienceScalarUpgrade(m) => m.load_post_process(),
            UpdateBody::ActiveShroudUpgrade(m) => m.load_post_process(),
            UpdateBody::MaxHealthUpgrade(m) => m.load_post_process(),
            UpdateBody::GrantScienceUpgrade(m) => m.load_post_process(),
            UpdateBody::StatusBitsUpgrade(m) => m.load_post_process(),
            UpdateBody::LocomotorSetUpgrade(m) => m.load_post_process(),
            UpdateBody::PassengersFireUpgrade(m) => m.load_post_process(),
            UpdateBody::StealthUpgrade(m) => m.load_post_process(),
            UpdateBody::SubObjectsUpgrade(m) => m.load_post_process(),
            UpdateBody::ObjectCreationUpgrade(m) => m.load_post_process(),
// --- p1w10-sonnet ---
            UpdateBody::PreorderCreate(m) => m.load_post_process(),
// --- p1w10-sonnet (helpers and dies) ---
            UpdateBody::ObjectSMCHelper(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::ObjectRepulsorHelper(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::ObjectWeaponStatusHelper(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::StatusDamageHelper(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::TempWeaponBonusHelper(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::DestroyDie(m) => m.load_post_process(),
            UpdateBody::BoneFxDamage(m) => m.load_post_process(),
            UpdateBody::TransitionDamageFx(m) => m.load_post_process(),
            UpdateBody::KeepObjectDie(m) => m.load_post_process(),
            UpdateBody::UpgradeDie(m) => m.load_post_process(),
// --- end p1w10-sonnet (helpers and dies) ---
// --- p1w10-sonnet (update modules) ---
            UpdateBody::LifetimeUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::DeletionUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::BaseRegenerateUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::SmartBombTargetHomingUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
// --- end p1w10-sonnet (update modules) ---
            UpdateBody::SpecialPowerCreate(m) => m.load_post_process(),
            UpdateBody::SupplyCenterCreate(m) => m.load_post_process(),
            UpdateBody::SupplyWarehouseCreate(m) => m.load_post_process(),
            UpdateBody::LockWeaponCreate(m) => m.load_post_process(),
            UpdateBody::VeterancyGainCreate(m) => m.load_post_process(),
            UpdateBody::GrantUpgradeCreate(m) => m.load_post_process(),
// --- end p1w10-sonnet ---
// --- end p1l9-upgrades ---
// --- p1w11-sonnet ---
            UpdateBody::PowerPlantUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::RadarUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::SubdualDamageHelper(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::ObjectDefectionHelper(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::SpyVisionUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::AutoDepositUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::AutoHealBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::CheckpointUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::FlammableUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::PoisonedBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::WeaponBonusUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::PilotFindVehicleUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::HordeUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::ToppleUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::PropagandaTowerBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::StructureToppleUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::NeutronBlastBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::BridgeScaffoldBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::TensileFormationUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::LeafletDropBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::EmpUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::BoneFxUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::StructureCollapseUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::StealthDetectorUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::StealthUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::OverchargeBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::GrantStealthBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::DynamicGeometryInfoUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::AnimationSteeringUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::RadiusDecalUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::AutoFindHealingUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::QueueProductionExitUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::DefaultProductionExitUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::OclUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
            UpdateBody::HeightDieUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::ProneUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::EnemyNearUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::FloatUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::HijackerUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::TechBuildingBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::FireSpreadUpdate(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::SlowDeathBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::RebuildHoleBehavior(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::OpenContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::TransportContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::GarrisonContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::CaveContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::TunnelContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::MobNexusContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::ParachuteContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::HealContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::OverlordContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::RiderChangeContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::HelixContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::RailedTransportContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
            UpdateBody::InternetHackContain(m) => match update { Some(u) => m.load_post_process_with(u), None => m.load_post_process() },
// --- end p1w13-sonnet ---
            _ => self.load_post_process(),
        }
    }
}
// --- end p1l9-example ---

impl Snapshot for UpdateBody {
    /// The `crc` of the concrete module (its own state; the bases are the slot's).
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            UpdateBody::Probe(p) => p.crc(xfer),
            UpdateBody::ModuleProbe(m) => m.crc(xfer),
            // --- p1l9-example ---
            UpdateBody::CommandSetUpgrade(m) => m.crc(xfer),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.crc(xfer),
            UpdateBody::WeaponBonusUpgrade(m) => m.crc(xfer),
            UpdateBody::ArmorUpgrade(m) => m.crc(xfer),
            UpdateBody::ModelConditionUpgrade(m) => m.crc(xfer),
            UpdateBody::ExperienceScalarUpgrade(m) => m.crc(xfer),
            UpdateBody::ActiveShroudUpgrade(m) => m.crc(xfer),
            UpdateBody::MaxHealthUpgrade(m) => m.crc(xfer),
            UpdateBody::GrantScienceUpgrade(m) => m.crc(xfer),
            UpdateBody::StatusBitsUpgrade(m) => m.crc(xfer),
            UpdateBody::LocomotorSetUpgrade(m) => m.crc(xfer),
            UpdateBody::PassengersFireUpgrade(m) => m.crc(xfer),
            UpdateBody::StealthUpgrade(m) => m.crc(xfer),
            UpdateBody::SubObjectsUpgrade(m) => m.crc(xfer),
            UpdateBody::ObjectCreationUpgrade(m) => m.crc(xfer),
// --- p1w10-sonnet ---
            UpdateBody::PreorderCreate(m) => m.crc(xfer),
// --- p1w10-sonnet (helpers and dies) ---
            UpdateBody::ObjectSMCHelper(m) => m.crc(xfer),
            UpdateBody::ObjectRepulsorHelper(m) => m.crc(xfer),
            UpdateBody::ObjectWeaponStatusHelper(m) => m.crc(xfer),
            UpdateBody::StatusDamageHelper(m) => m.crc(xfer),
            UpdateBody::TempWeaponBonusHelper(m) => m.crc(xfer),
            UpdateBody::DestroyDie(m) => m.crc(xfer),
            UpdateBody::BoneFxDamage(m) => m.crc(xfer),
            UpdateBody::TransitionDamageFx(m) => m.crc(xfer),
            UpdateBody::KeepObjectDie(m) => m.crc(xfer),
            UpdateBody::UpgradeDie(m) => m.crc(xfer),
// --- end p1w10-sonnet (helpers and dies) ---
// --- p1w10-sonnet (update modules) ---
            UpdateBody::LifetimeUpdate(m) => m.crc(xfer),
            UpdateBody::DeletionUpdate(m) => m.crc(xfer),
            UpdateBody::BaseRegenerateUpdate(m) => m.crc(xfer),
            UpdateBody::SmartBombTargetHomingUpdate(m) => m.crc(xfer),
// --- end p1w10-sonnet (update modules) ---
            UpdateBody::SpecialPowerCreate(m) => m.crc(xfer),
            UpdateBody::SupplyCenterCreate(m) => m.crc(xfer),
            UpdateBody::SupplyWarehouseCreate(m) => m.crc(xfer),
            UpdateBody::LockWeaponCreate(m) => m.crc(xfer),
            UpdateBody::VeterancyGainCreate(m) => m.crc(xfer),
            UpdateBody::GrantUpgradeCreate(m) => m.crc(xfer),
// --- end p1w10-sonnet ---
// --- end p1l9-upgrades ---
// --- p1w11-sonnet ---
            UpdateBody::CostModifierUpgrade(m) => m.crc(xfer),
            UpdateBody::PowerPlantUpgrade(m) => m.crc(xfer),
            UpdateBody::RadarUpgrade(m) => m.crc(xfer),
            UpdateBody::UnpauseSpecialPowerUpgrade(m) => m.crc(xfer),
            UpdateBody::ReplaceObjectUpgrade(m) => m.crc(xfer),
            UpdateBody::DamDie(m) => m.crc(xfer),
            UpdateBody::SpecialPowerCompletionDie(m) => m.crc(xfer),
            UpdateBody::EjectPilotDie(m) => m.crc(xfer),
            UpdateBody::InstantDeathBehavior(m) => m.crc(xfer),
            UpdateBody::RebuildHoleExposeDie(m) => m.crc(xfer),
            UpdateBody::CreateCrateDie(m) => m.crc(xfer),
            UpdateBody::CrushDie(m) => m.crc(xfer),
            UpdateBody::CreateObjectDie(m) => m.crc(xfer),
            UpdateBody::PowerPlantUpdate(m) => m.crc(xfer),
            UpdateBody::RadarUpdate(m) => m.crc(xfer),
            UpdateBody::HealCrateCollide(m) => m.crc(xfer),
            UpdateBody::ShroudCrateCollide(m) => m.crc(xfer),
            UpdateBody::MoneyCrateCollide(m) => m.crc(xfer),
            UpdateBody::SabotageCrateCollide(m) => m.crc(xfer),
            UpdateBody::UnitCrateCollide(m) => m.crc(xfer),
            UpdateBody::VeterancyCrateCollide(m) => m.crc(xfer),
            UpdateBody::SalvageCrateCollide(m) => m.crc(xfer),
            UpdateBody::ConvertToCarBombCrateCollide(m) => m.crc(xfer),
            UpdateBody::ConvertToHijackedVehicleCrateCollide(m) => m.crc(xfer),
            UpdateBody::SubdualDamageHelper(m) => m.crc(xfer),
            UpdateBody::ObjectDefectionHelper(m) => m.crc(xfer),
            UpdateBody::CleanupAreaPower(m) => m.crc(xfer),
            UpdateBody::BaikonurLaunchPower(m) => m.crc(xfer),
            UpdateBody::SpyVisionSpecialPower(m) => m.crc(xfer),
            UpdateBody::DefectorSpecialPower(m) => m.crc(xfer),
            UpdateBody::OCLSpecialPower(m) => m.crc(xfer),
            UpdateBody::CashHackSpecialPower(m) => m.crc(xfer),
            UpdateBody::CashBountyPower(m) => m.crc(xfer),
            UpdateBody::SpecialPowerModule(m) => m.crc(xfer),
            UpdateBody::SpyVisionUpdate(m) => m.crc(xfer),
            UpdateBody::FXListDie(m) => m.crc(xfer),
            UpdateBody::FireWeaponWhenDeadBehavior(m) => m.crc(xfer),
            UpdateBody::AutoDepositUpdate(m) => m.crc(xfer),
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => m.crc(xfer),
            UpdateBody::AutoHealBehavior(m) => m.crc(xfer),
            UpdateBody::CheckpointUpdate(m) => m.crc(xfer),
            UpdateBody::FlammableUpdate(m) => m.crc(xfer),
            UpdateBody::PoisonedBehavior(m) => m.crc(xfer),
            UpdateBody::WeaponBonusUpdate(m) => m.crc(xfer),
            UpdateBody::PilotFindVehicleUpdate(m) => m.crc(xfer),
            UpdateBody::HordeUpdate(m) => m.crc(xfer),
            UpdateBody::ToppleUpdate(m) => m.crc(xfer),
            UpdateBody::PropagandaTowerBehavior(m) => m.crc(xfer),
            UpdateBody::StructureToppleUpdate(m) => m.crc(xfer),
            UpdateBody::NeutronBlastBehavior(m) => m.crc(xfer),
            UpdateBody::BridgeScaffoldBehavior(m) => m.crc(xfer),
            UpdateBody::TensileFormationUpdate(m) => m.crc(xfer),
            UpdateBody::LeafletDropBehavior(m) => m.crc(xfer),
            UpdateBody::EmpUpdate(m) => m.crc(xfer),
            UpdateBody::BoneFxUpdate(m) => m.crc(xfer),
            UpdateBody::StructureCollapseUpdate(m) => m.crc(xfer),
            UpdateBody::StealthDetectorUpdate(m) => m.crc(xfer),
            UpdateBody::StealthUpdate(m) => m.crc(xfer),
            UpdateBody::OverchargeBehavior(m) => m.crc(xfer),
            UpdateBody::GrantStealthBehavior(m) => m.crc(xfer),
            UpdateBody::DynamicGeometryInfoUpdate(m) => m.crc(xfer),
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => m.crc(xfer),
            UpdateBody::AnimationSteeringUpdate(m) => m.crc(xfer),
            UpdateBody::RadiusDecalUpdate(m) => m.crc(xfer),
            UpdateBody::AutoFindHealingUpdate(m) => m.crc(xfer),
            UpdateBody::QueueProductionExitUpdate(m) => m.crc(xfer),
            UpdateBody::DefaultProductionExitUpdate(m) => m.crc(xfer),
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => m.crc(xfer),
            UpdateBody::OclUpdate(m) => m.crc(xfer),
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
            UpdateBody::HeightDieUpdate(m) => m.crc(xfer),
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => m.crc(xfer),
            UpdateBody::SquishCollide(m) => m.crc(xfer),
            UpdateBody::ProneUpdate(m) => m.crc(xfer),
            UpdateBody::EnemyNearUpdate(m) => m.crc(xfer),
            UpdateBody::FloatUpdate(m) => m.crc(xfer),
            UpdateBody::HijackerUpdate(m) => m.crc(xfer),
            UpdateBody::TechBuildingBehavior(m) => m.crc(xfer),
            UpdateBody::FireSpreadUpdate(m) => m.crc(xfer),
            UpdateBody::ActiveBody(m) => m.crc(xfer),
            UpdateBody::HighlanderBody(m) => m.crc(xfer),
            UpdateBody::ImmortalBody(m) => m.crc(xfer),
            UpdateBody::StructureBody(m) => m.crc(xfer),
            UpdateBody::UndeadBody(m) => m.crc(xfer),
            UpdateBody::HiveStructureBody(m) => m.crc(xfer),
            UpdateBody::InactiveBody(m) => m.crc(xfer),
            UpdateBody::SlowDeathBehavior(m) => m.crc(xfer),
            UpdateBody::RebuildHoleBehavior(m) => m.crc(xfer),
            UpdateBody::OpenContain(m) => m.crc(xfer),
            UpdateBody::TransportContain(m) => m.crc(xfer),
            UpdateBody::GarrisonContain(m) => m.crc(xfer),
            UpdateBody::CaveContain(m) => m.crc(xfer),
            UpdateBody::TunnelContain(m) => m.crc(xfer),
            UpdateBody::MobNexusContain(m) => m.crc(xfer),
            UpdateBody::ParachuteContain(m) => m.crc(xfer),
            UpdateBody::HealContain(m) => m.crc(xfer),
            UpdateBody::OverlordContain(m) => m.crc(xfer),
            UpdateBody::RiderChangeContain(m) => m.crc(xfer),
            UpdateBody::HelixContain(m) => m.crc(xfer),
            UpdateBody::RailedTransportContain(m) => m.crc(xfer),
            UpdateBody::InternetHackContain(m) => m.crc(xfer),
// --- end p1w13-sonnet ---
            // --- end p1l9-example ---
        }
    }
    /// The `xfer` of the concrete module.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            UpdateBody::Probe(p) => p.xfer(xfer),
            UpdateBody::ModuleProbe(m) => m.xfer(xfer),
            // --- p1l9-example ---
            UpdateBody::CommandSetUpgrade(m) => m.xfer(xfer),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.xfer(xfer),
            UpdateBody::WeaponBonusUpgrade(m) => m.xfer(xfer),
            UpdateBody::ArmorUpgrade(m) => m.xfer(xfer),
            UpdateBody::ModelConditionUpgrade(m) => m.xfer(xfer),
            UpdateBody::ExperienceScalarUpgrade(m) => m.xfer(xfer),
            UpdateBody::ActiveShroudUpgrade(m) => m.xfer(xfer),
            UpdateBody::MaxHealthUpgrade(m) => m.xfer(xfer),
            UpdateBody::GrantScienceUpgrade(m) => m.xfer(xfer),
            UpdateBody::StatusBitsUpgrade(m) => m.xfer(xfer),
            UpdateBody::LocomotorSetUpgrade(m) => m.xfer(xfer),
            UpdateBody::PassengersFireUpgrade(m) => m.xfer(xfer),
            UpdateBody::StealthUpgrade(m) => m.xfer(xfer),
            UpdateBody::SubObjectsUpgrade(m) => m.xfer(xfer),
            UpdateBody::ObjectCreationUpgrade(m) => m.xfer(xfer),
// --- p1w10-sonnet ---
            UpdateBody::PreorderCreate(m) => m.xfer(xfer),
// --- p1w10-sonnet (helpers and dies) ---
            UpdateBody::ObjectSMCHelper(m) => m.xfer(xfer),
            UpdateBody::ObjectRepulsorHelper(m) => m.xfer(xfer),
            UpdateBody::ObjectWeaponStatusHelper(m) => m.xfer(xfer),
            UpdateBody::StatusDamageHelper(m) => m.xfer(xfer),
            UpdateBody::TempWeaponBonusHelper(m) => m.xfer(xfer),
            UpdateBody::DestroyDie(m) => m.xfer(xfer),
            UpdateBody::BoneFxDamage(m) => m.xfer(xfer),
            UpdateBody::TransitionDamageFx(m) => m.xfer(xfer),
            UpdateBody::KeepObjectDie(m) => m.xfer(xfer),
            UpdateBody::UpgradeDie(m) => m.xfer(xfer),
// --- end p1w10-sonnet (helpers and dies) ---
// --- p1w10-sonnet (update modules) ---
            UpdateBody::LifetimeUpdate(m) => m.xfer(xfer),
            UpdateBody::DeletionUpdate(m) => m.xfer(xfer),
            UpdateBody::BaseRegenerateUpdate(m) => m.xfer(xfer),
            UpdateBody::SmartBombTargetHomingUpdate(m) => m.xfer(xfer),
// --- end p1w10-sonnet (update modules) ---
            UpdateBody::SpecialPowerCreate(m) => m.xfer(xfer),
            UpdateBody::SupplyCenterCreate(m) => m.xfer(xfer),
            UpdateBody::SupplyWarehouseCreate(m) => m.xfer(xfer),
            UpdateBody::LockWeaponCreate(m) => m.xfer(xfer),
            UpdateBody::VeterancyGainCreate(m) => m.xfer(xfer),
            UpdateBody::GrantUpgradeCreate(m) => m.xfer(xfer),
// --- end p1w10-sonnet ---
// --- end p1l9-upgrades ---
// --- p1w11-sonnet ---
            UpdateBody::CostModifierUpgrade(m) => m.xfer(xfer),
            UpdateBody::PowerPlantUpgrade(m) => m.xfer(xfer),
            UpdateBody::RadarUpgrade(m) => m.xfer(xfer),
            UpdateBody::UnpauseSpecialPowerUpgrade(m) => m.xfer(xfer),
            UpdateBody::ReplaceObjectUpgrade(m) => m.xfer(xfer),
            UpdateBody::DamDie(m) => m.xfer(xfer),
            UpdateBody::SpecialPowerCompletionDie(m) => m.xfer(xfer),
            UpdateBody::EjectPilotDie(m) => m.xfer(xfer),
            UpdateBody::InstantDeathBehavior(m) => m.xfer(xfer),
            UpdateBody::RebuildHoleExposeDie(m) => m.xfer(xfer),
            UpdateBody::CreateCrateDie(m) => m.xfer(xfer),
            UpdateBody::CrushDie(m) => m.xfer(xfer),
            UpdateBody::CreateObjectDie(m) => m.xfer(xfer),
            UpdateBody::PowerPlantUpdate(m) => m.xfer(xfer),
            UpdateBody::RadarUpdate(m) => m.xfer(xfer),
            UpdateBody::HealCrateCollide(m) => m.xfer(xfer),
            UpdateBody::ShroudCrateCollide(m) => m.xfer(xfer),
            UpdateBody::MoneyCrateCollide(m) => m.xfer(xfer),
            UpdateBody::SabotageCrateCollide(m) => m.xfer(xfer),
            UpdateBody::UnitCrateCollide(m) => m.xfer(xfer),
            UpdateBody::VeterancyCrateCollide(m) => m.xfer(xfer),
            UpdateBody::SalvageCrateCollide(m) => m.xfer(xfer),
            UpdateBody::ConvertToCarBombCrateCollide(m) => m.xfer(xfer),
            UpdateBody::ConvertToHijackedVehicleCrateCollide(m) => m.xfer(xfer),
            UpdateBody::SubdualDamageHelper(m) => m.xfer(xfer),
            UpdateBody::ObjectDefectionHelper(m) => m.xfer(xfer),
            UpdateBody::CleanupAreaPower(m) => m.xfer(xfer),
            UpdateBody::BaikonurLaunchPower(m) => m.xfer(xfer),
            UpdateBody::SpyVisionSpecialPower(m) => m.xfer(xfer),
            UpdateBody::DefectorSpecialPower(m) => m.xfer(xfer),
            UpdateBody::OCLSpecialPower(m) => m.xfer(xfer),
            UpdateBody::CashHackSpecialPower(m) => m.xfer(xfer),
            UpdateBody::CashBountyPower(m) => m.xfer(xfer),
            UpdateBody::SpecialPowerModule(m) => m.xfer(xfer),
            UpdateBody::SpyVisionUpdate(m) => m.xfer(xfer),
            UpdateBody::FXListDie(m) => m.xfer(xfer),
            UpdateBody::FireWeaponWhenDeadBehavior(m) => m.xfer(xfer),
            UpdateBody::AutoDepositUpdate(m) => m.xfer(xfer),
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => m.xfer(xfer),
            UpdateBody::AutoHealBehavior(m) => m.xfer(xfer),
            UpdateBody::CheckpointUpdate(m) => m.xfer(xfer),
            UpdateBody::FlammableUpdate(m) => m.xfer(xfer),
            UpdateBody::PoisonedBehavior(m) => m.xfer(xfer),
            UpdateBody::WeaponBonusUpdate(m) => m.xfer(xfer),
            UpdateBody::PilotFindVehicleUpdate(m) => m.xfer(xfer),
            UpdateBody::HordeUpdate(m) => m.xfer(xfer),
            UpdateBody::ToppleUpdate(m) => m.xfer(xfer),
            UpdateBody::PropagandaTowerBehavior(m) => m.xfer(xfer),
            UpdateBody::StructureToppleUpdate(m) => m.xfer(xfer),
            UpdateBody::NeutronBlastBehavior(m) => m.xfer(xfer),
            UpdateBody::BridgeScaffoldBehavior(m) => m.xfer(xfer),
            UpdateBody::TensileFormationUpdate(m) => m.xfer(xfer),
            UpdateBody::LeafletDropBehavior(m) => m.xfer(xfer),
            UpdateBody::EmpUpdate(m) => m.xfer(xfer),
            UpdateBody::BoneFxUpdate(m) => m.xfer(xfer),
            UpdateBody::StructureCollapseUpdate(m) => m.xfer(xfer),
            UpdateBody::StealthDetectorUpdate(m) => m.xfer(xfer),
            UpdateBody::StealthUpdate(m) => m.xfer(xfer),
            UpdateBody::OverchargeBehavior(m) => m.xfer(xfer),
            UpdateBody::GrantStealthBehavior(m) => m.xfer(xfer),
            UpdateBody::DynamicGeometryInfoUpdate(m) => m.xfer(xfer),
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => m.xfer(xfer),
            UpdateBody::AnimationSteeringUpdate(m) => m.xfer(xfer),
            UpdateBody::RadiusDecalUpdate(m) => m.xfer(xfer),
            UpdateBody::AutoFindHealingUpdate(m) => m.xfer(xfer),
            UpdateBody::QueueProductionExitUpdate(m) => m.xfer(xfer),
            UpdateBody::DefaultProductionExitUpdate(m) => m.xfer(xfer),
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => m.xfer(xfer),
            UpdateBody::OclUpdate(m) => m.xfer(xfer),
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
            UpdateBody::HeightDieUpdate(m) => m.xfer(xfer),
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => m.xfer(xfer),
            UpdateBody::SquishCollide(m) => m.xfer(xfer),
            UpdateBody::ProneUpdate(m) => m.xfer(xfer),
            UpdateBody::EnemyNearUpdate(m) => m.xfer(xfer),
            UpdateBody::FloatUpdate(m) => m.xfer(xfer),
            UpdateBody::HijackerUpdate(m) => m.xfer(xfer),
            UpdateBody::TechBuildingBehavior(m) => m.xfer(xfer),
            UpdateBody::FireSpreadUpdate(m) => m.xfer(xfer),
            UpdateBody::ActiveBody(m) => m.xfer(xfer),
            UpdateBody::HighlanderBody(m) => m.xfer(xfer),
            UpdateBody::ImmortalBody(m) => m.xfer(xfer),
            UpdateBody::StructureBody(m) => m.xfer(xfer),
            UpdateBody::UndeadBody(m) => m.xfer(xfer),
            UpdateBody::HiveStructureBody(m) => m.xfer(xfer),
            UpdateBody::InactiveBody(m) => m.xfer(xfer),
            UpdateBody::SlowDeathBehavior(m) => m.xfer(xfer),
            UpdateBody::RebuildHoleBehavior(m) => m.xfer(xfer),
            UpdateBody::OpenContain(m) => m.xfer(xfer),
            UpdateBody::TransportContain(m) => m.xfer(xfer),
            UpdateBody::GarrisonContain(m) => m.xfer(xfer),
            UpdateBody::CaveContain(m) => m.xfer(xfer),
            UpdateBody::TunnelContain(m) => m.xfer(xfer),
            UpdateBody::MobNexusContain(m) => m.xfer(xfer),
            UpdateBody::ParachuteContain(m) => m.xfer(xfer),
            UpdateBody::HealContain(m) => m.xfer(xfer),
            UpdateBody::OverlordContain(m) => m.xfer(xfer),
            UpdateBody::RiderChangeContain(m) => m.xfer(xfer),
            UpdateBody::HelixContain(m) => m.xfer(xfer),
            UpdateBody::RailedTransportContain(m) => m.xfer(xfer),
            UpdateBody::InternetHackContain(m) => m.xfer(xfer),
// --- end p1w13-sonnet ---
            // --- end p1l9-example ---
        }
    }
    /// The `loadPostProcess` of the concrete module.
    fn load_post_process(&mut self) {
        match self {
            UpdateBody::Probe(p) => p.load_post_process(),
            UpdateBody::ModuleProbe(m) => m.load_post_process(),
            // --- p1l9-example ---
            UpdateBody::CommandSetUpgrade(m) => m.load_post_process(),
// --- p1l9-upgrades ---
            UpdateBody::WeaponSetUpgrade(m) => m.load_post_process(),
            UpdateBody::WeaponBonusUpgrade(m) => m.load_post_process(),
            UpdateBody::ArmorUpgrade(m) => m.load_post_process(),
            UpdateBody::ModelConditionUpgrade(m) => m.load_post_process(),
            UpdateBody::ExperienceScalarUpgrade(m) => m.load_post_process(),
            UpdateBody::ActiveShroudUpgrade(m) => m.load_post_process(),
            UpdateBody::MaxHealthUpgrade(m) => m.load_post_process(),
            UpdateBody::GrantScienceUpgrade(m) => m.load_post_process(),
            UpdateBody::StatusBitsUpgrade(m) => m.load_post_process(),
            UpdateBody::LocomotorSetUpgrade(m) => m.load_post_process(),
            UpdateBody::PassengersFireUpgrade(m) => m.load_post_process(),
            UpdateBody::StealthUpgrade(m) => m.load_post_process(),
            UpdateBody::SubObjectsUpgrade(m) => m.load_post_process(),
            UpdateBody::ObjectCreationUpgrade(m) => m.load_post_process(),
// --- p1w10-sonnet ---
            UpdateBody::PreorderCreate(m) => m.load_post_process(),
// --- p1w10-sonnet (helpers and dies) ---
            UpdateBody::ObjectSMCHelper(m) => m.load_post_process(),
            UpdateBody::ObjectRepulsorHelper(m) => m.load_post_process(),
            UpdateBody::ObjectWeaponStatusHelper(m) => m.load_post_process(),
            UpdateBody::StatusDamageHelper(m) => m.load_post_process(),
            UpdateBody::TempWeaponBonusHelper(m) => m.load_post_process(),
            UpdateBody::DestroyDie(m) => m.load_post_process(),
            UpdateBody::BoneFxDamage(m) => m.load_post_process(),
            UpdateBody::TransitionDamageFx(m) => m.load_post_process(),
            UpdateBody::KeepObjectDie(m) => m.load_post_process(),
            UpdateBody::UpgradeDie(m) => m.load_post_process(),
// --- end p1w10-sonnet (helpers and dies) ---
// --- p1w10-sonnet (update modules) ---
            UpdateBody::LifetimeUpdate(m) => m.load_post_process(),
            UpdateBody::DeletionUpdate(m) => m.load_post_process(),
            UpdateBody::BaseRegenerateUpdate(m) => m.load_post_process(),
            UpdateBody::SmartBombTargetHomingUpdate(m) => m.load_post_process(),
// --- end p1w10-sonnet (update modules) ---
            UpdateBody::SpecialPowerCreate(m) => m.load_post_process(),
            UpdateBody::SupplyCenterCreate(m) => m.load_post_process(),
            UpdateBody::SupplyWarehouseCreate(m) => m.load_post_process(),
            UpdateBody::LockWeaponCreate(m) => m.load_post_process(),
            UpdateBody::VeterancyGainCreate(m) => m.load_post_process(),
            UpdateBody::GrantUpgradeCreate(m) => m.load_post_process(),
// --- end p1w10-sonnet ---
// --- end p1l9-upgrades ---
// --- p1w11-sonnet ---
            UpdateBody::CostModifierUpgrade(m) => m.load_post_process(),
            UpdateBody::PowerPlantUpgrade(m) => m.load_post_process(),
            UpdateBody::RadarUpgrade(m) => m.load_post_process(),
            UpdateBody::UnpauseSpecialPowerUpgrade(m) => m.load_post_process(),
            UpdateBody::ReplaceObjectUpgrade(m) => m.load_post_process(),
            UpdateBody::DamDie(m) => m.load_post_process(),
            UpdateBody::SpecialPowerCompletionDie(m) => m.load_post_process(),
            UpdateBody::EjectPilotDie(m) => m.load_post_process(),
            UpdateBody::InstantDeathBehavior(m) => m.load_post_process(),
            UpdateBody::RebuildHoleExposeDie(m) => m.load_post_process(),
            UpdateBody::CreateCrateDie(m) => m.load_post_process(),
            UpdateBody::CrushDie(m) => m.load_post_process(),
            UpdateBody::CreateObjectDie(m) => m.load_post_process(),
            UpdateBody::PowerPlantUpdate(m) => m.load_post_process(),
            UpdateBody::RadarUpdate(m) => m.load_post_process(),
            UpdateBody::HealCrateCollide(m) => m.load_post_process(),
            UpdateBody::ShroudCrateCollide(m) => m.load_post_process(),
            UpdateBody::MoneyCrateCollide(m) => m.load_post_process(),
            UpdateBody::SabotageCrateCollide(m) => m.load_post_process(),
            UpdateBody::UnitCrateCollide(m) => m.load_post_process(),
            UpdateBody::VeterancyCrateCollide(m) => m.load_post_process(),
            UpdateBody::SalvageCrateCollide(m) => m.load_post_process(),
            UpdateBody::ConvertToCarBombCrateCollide(m) => m.load_post_process(),
            UpdateBody::ConvertToHijackedVehicleCrateCollide(m) => m.load_post_process(),
            UpdateBody::SubdualDamageHelper(m) => m.load_post_process(),
            UpdateBody::ObjectDefectionHelper(m) => m.load_post_process(),
            UpdateBody::CleanupAreaPower(m) => m.load_post_process(),
            UpdateBody::BaikonurLaunchPower(m) => m.load_post_process(),
            UpdateBody::SpyVisionSpecialPower(m) => m.load_post_process(),
            UpdateBody::DefectorSpecialPower(m) => m.load_post_process(),
            UpdateBody::OCLSpecialPower(m) => m.load_post_process(),
            UpdateBody::CashHackSpecialPower(m) => m.load_post_process(),
            UpdateBody::CashBountyPower(m) => m.load_post_process(),
            UpdateBody::SpecialPowerModule(m) => m.load_post_process(),
            UpdateBody::SpyVisionUpdate(m) => m.load_post_process(),
            UpdateBody::FXListDie(m) => m.load_post_process(),
            UpdateBody::FireWeaponWhenDeadBehavior(m) => m.load_post_process(),
            UpdateBody::AutoDepositUpdate(m) => m.load_post_process(),
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => m.load_post_process(),
            UpdateBody::AutoHealBehavior(m) => m.load_post_process(),
            UpdateBody::CheckpointUpdate(m) => m.load_post_process(),
            UpdateBody::FlammableUpdate(m) => m.load_post_process(),
            UpdateBody::PoisonedBehavior(m) => m.load_post_process(),
            UpdateBody::WeaponBonusUpdate(m) => m.load_post_process(),
            UpdateBody::PilotFindVehicleUpdate(m) => m.load_post_process(),
            UpdateBody::HordeUpdate(m) => m.load_post_process(),
            UpdateBody::ToppleUpdate(m) => m.load_post_process(),
            UpdateBody::PropagandaTowerBehavior(m) => m.load_post_process(),
            UpdateBody::StructureToppleUpdate(m) => m.load_post_process(),
            UpdateBody::NeutronBlastBehavior(m) => m.load_post_process(),
            UpdateBody::BridgeScaffoldBehavior(m) => m.load_post_process(),
            UpdateBody::TensileFormationUpdate(m) => m.load_post_process(),
            UpdateBody::LeafletDropBehavior(m) => m.load_post_process(),
            UpdateBody::EmpUpdate(m) => m.load_post_process(),
            UpdateBody::BoneFxUpdate(m) => m.load_post_process(),
            UpdateBody::StructureCollapseUpdate(m) => m.load_post_process(),
            UpdateBody::StealthDetectorUpdate(m) => m.load_post_process(),
            UpdateBody::StealthUpdate(m) => m.load_post_process(),
            UpdateBody::OverchargeBehavior(m) => m.load_post_process(),
            UpdateBody::GrantStealthBehavior(m) => m.load_post_process(),
            UpdateBody::DynamicGeometryInfoUpdate(m) => m.load_post_process(),
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => m.load_post_process(),
            UpdateBody::AnimationSteeringUpdate(m) => m.load_post_process(),
            UpdateBody::RadiusDecalUpdate(m) => m.load_post_process(),
            UpdateBody::AutoFindHealingUpdate(m) => m.load_post_process(),
            UpdateBody::QueueProductionExitUpdate(m) => m.load_post_process(),
            UpdateBody::DefaultProductionExitUpdate(m) => m.load_post_process(),
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => m.load_post_process(),
            UpdateBody::OclUpdate(m) => m.load_post_process(),
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
            UpdateBody::HeightDieUpdate(m) => m.load_post_process(),
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => m.load_post_process(),
            UpdateBody::SquishCollide(m) => m.load_post_process(),
            UpdateBody::ProneUpdate(m) => m.load_post_process(),
            UpdateBody::EnemyNearUpdate(m) => m.load_post_process(),
            UpdateBody::FloatUpdate(m) => m.load_post_process(),
            UpdateBody::HijackerUpdate(m) => m.load_post_process(),
            UpdateBody::TechBuildingBehavior(m) => m.load_post_process(),
            UpdateBody::FireSpreadUpdate(m) => m.load_post_process(),
            UpdateBody::ActiveBody(m) => m.load_post_process(),
            UpdateBody::HighlanderBody(m) => m.load_post_process(),
            UpdateBody::ImmortalBody(m) => m.load_post_process(),
            UpdateBody::StructureBody(m) => m.load_post_process(),
            UpdateBody::UndeadBody(m) => m.load_post_process(),
            UpdateBody::HiveStructureBody(m) => m.load_post_process(),
            UpdateBody::InactiveBody(m) => m.load_post_process(),
            UpdateBody::SlowDeathBehavior(m) => m.load_post_process(),
            UpdateBody::RebuildHoleBehavior(m) => m.load_post_process(),
            UpdateBody::OpenContain(m) => m.load_post_process(),
            UpdateBody::TransportContain(m) => m.load_post_process(),
            UpdateBody::GarrisonContain(m) => m.load_post_process(),
            UpdateBody::CaveContain(m) => m.load_post_process(),
            UpdateBody::TunnelContain(m) => m.load_post_process(),
            UpdateBody::MobNexusContain(m) => m.load_post_process(),
            UpdateBody::ParachuteContain(m) => m.load_post_process(),
            UpdateBody::HealContain(m) => m.load_post_process(),
            UpdateBody::OverlordContain(m) => m.load_post_process(),
            UpdateBody::RiderChangeContain(m) => m.load_post_process(),
            UpdateBody::HelixContain(m) => m.load_post_process(),
            UpdateBody::RailedTransportContain(m) => m.load_post_process(),
            UpdateBody::InternetHackContain(m) => m.load_post_process(),
// --- end p1w13-sonnet ---
            // --- end p1l9-example ---
        }
    }
}

impl Snapshot for ProbeUpdate {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// Version 1: the number of calls so far.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_unsigned_int(&mut self.calls)
    }
    fn load_post_process(&mut self) {}
}
// --- end p1w6-object ---

/// What a [`ProbeUpdate`] spec describes for a module of a new object.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeModuleSpec {
    /// `getUpdatePhase()` of the module.
    pub phase: SleepyUpdatePhase,
    /// `m_nextCallFrameAndPhase`'s frame before registration (0 = unset,
    /// which `registerObject` turns into the current frame).
    pub initial_call_frame: u32,
    /// The module's script.
    pub script: Vec<ProbeStep>,
}

/// One action of a [`ProbeStep`].
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeAction {
    /// `TheGameLogic->destroyObject(findObjectByID(id))`.
    Destroy(ObjectId),
    /// `target->setWakeFrame(obj, delay)` as `UpdateModule` does it: the
    /// frame is `now + delay` ([`UpdateModule::set_wake_frame`]), then
    /// `TheGameLogic->friend_awakenUpdateModule(obj, u, frame)`. A delay of 0
    /// is a wake in the past once the module has run, and two modules that
    /// wake each other that way never let the frame end (as upstream).
    Awaken { target: UpdateHandle, delay: u32 },
    /// `newObject` with these modules, then `registerObject`.
    Spawn(Vec<ProbeModuleSpec>),
    /// `target->attemptDamage(&info)` with a damage of `amount`.
    Damage { target: ObjectId, amount: f32 },
    /// `TheAudio->addAudioEvent(...)` for this module's object.
    Sound(Vec<u8>),
}

/// One call of a [`ProbeUpdate`]: the actions, then the sleep it returns.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeStep {
    /// What the call does, in order.
    pub actions: Vec<ProbeAction>,
    /// The `UpdateSleepTime` the call returns (any number, as upstream's enum
    /// can hold: 0 and above `i32::MAX` included).
    pub sleep: u32,
}

/// A scripted update module. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeUpdate {
    /// The script; call `k` runs `script[k % len]` (an empty script sleeps
    /// one frame and does nothing).
    pub script: Vec<ProbeStep>,
    /// How many times `update` has been called.
    pub calls: u32,
}

impl ProbeUpdate {
    /// A module that runs `script` cyclically.
    pub fn new(script: Vec<ProbeStep>) -> ProbeUpdate {
        ProbeUpdate { script, calls: 0 }
    }

    /// The module's `update()`.
    pub fn update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        let step = if self.script.is_empty() {
            ProbeStep { actions: Vec::new(), sleep: 1 }
        } else {
            self.script[self.calls as usize % self.script.len()].clone()
        };
        self.calls = self.calls.wrapping_add(1);
        for action in &step.actions {
            match action {
                ProbeAction::Destroy(id) => world.destroy_object(*id),
                ProbeAction::Awaken { target, delay } => {
                    let when = UpdateModule::set_wake_frame(world.frame(), UpdateSleepTime(*delay));
                    // A failed wake is upstream's RELEASE_CRASH; the probe has no way to react.
                    let _ = world.awaken_update_module(target.object, *target, when);
                }
                ProbeAction::Spawn(specs) => {
                    let id = world.allocate_object_id();
                    // A refused registration is a bad id; the allocator cannot make one.
                    let _ = world.register_object(probe_object(id, specs));
                }
                ProbeAction::Damage { target, amount } => {
                    let mut info = DamageInfo::default();
                    info.input.source_id = me.object;
                    info.input.amount = *amount;
                    world.attempt_damage(*target, &mut info);
                }
                ProbeAction::Sound(name) => {
                    world.events.push(ClientEvent::Audio { event_name: name.clone(), object_id: me.object, player_index: -1 });
                }
            }
        }
        UpdateSleepTime(step.sleep)
    }
}

/// An object (not yet registered) with one [`ProbeUpdate`] per spec.
///
/// # Panics
///
/// Never in practice: `UpdateModule::new` refuses only absent module data.
pub fn probe_object(id: ObjectId, specs: &[ProbeModuleSpec]) -> Object {
    let mut obj = Object::new(id);
    for spec in specs {
        let data = Arc::new(UpdateModuleData::new());
        let mut update = UpdateModule::new(id, Some(data), spec.phase).expect("module data is present");
        if spec.initial_call_frame != 0 {
            update.friend_set_next_call_frame(spec.initial_call_frame);
        }
        obj.behaviors.push(BehaviorSlot { update, body: Some(UpdateBody::Probe(ProbeUpdate::new(spec.script.clone()))) });
    }
    obj
}

// --- p1w10-sonnet ---
impl UpdateBody {
    /// The part of a module's constructor that needs the world (a random draw, a wake call, a read of the global data), which
    /// the world runs once the module is part of its object, in module order (`World::construct_module`). Nothing for a module
    /// whose constructor touches no world.
    pub fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        match self {
            UpdateBody::LifetimeUpdate(m) => m.world_construct(world, me),
            UpdateBody::DeletionUpdate(m) => m.world_construct(world, me),
            UpdateBody::BaseRegenerateUpdate(m) => m.world_construct(world, me),
            UpdateBody::SmartBombTargetHomingUpdate(m) => m.world_construct(world, me),
            UpdateBody::ObjectSMCHelper(m) => m.world_construct(world, me),
            UpdateBody::ObjectRepulsorHelper(m) => m.world_construct(world, me),
            UpdateBody::ObjectWeaponStatusHelper(m) => m.world_construct(world, me),
            UpdateBody::StatusDamageHelper(m) => m.world_construct(world, me),
            UpdateBody::TempWeaponBonusHelper(m) => m.world_construct(world, me),
// --- p1w11-sonnet ---
            UpdateBody::PowerPlantUpdate(m) => m.world_construct(world, me),
            UpdateBody::RadarUpdate(m) => m.world_construct(world, me),
            UpdateBody::SubdualDamageHelper(m) => m.world_construct(world, me),
            UpdateBody::ObjectDefectionHelper(m) => m.world_construct(world, me),
            UpdateBody::CleanupAreaPower(m) => m.world_construct(world, me),
            UpdateBody::BaikonurLaunchPower(m) => m.world_construct(world, me),
            UpdateBody::SpyVisionSpecialPower(m) => m.world_construct(world, me),
            UpdateBody::DefectorSpecialPower(m) => m.world_construct(world, me),
            UpdateBody::OCLSpecialPower(m) => m.world_construct(world, me),
            UpdateBody::CashHackSpecialPower(m) => m.world_construct(world, me),
            UpdateBody::CashBountyPower(m) => m.world_construct(world, me),
            UpdateBody::SpecialPowerModule(m) => m.world_construct(world, me),
            UpdateBody::SpyVisionUpdate(m) => m.world_construct(world, me),
            UpdateBody::FXListDie(m) => m.world_construct(world, me),
            UpdateBody::FireWeaponWhenDeadBehavior(m) => m.world_construct(world, me),
            UpdateBody::AutoDepositUpdate(m) => m.world_construct(world, me),
// --- end p1w11-sonnet ---
// --- p1w12-sonnet ---
            UpdateBody::FireOCLAfterWeaponCooldownUpdate(m) => m.world_construct(world, me),
            UpdateBody::AutoHealBehavior(m) => m.world_construct(world, me),
            UpdateBody::CheckpointUpdate(m) => m.world_construct(world, me),
            UpdateBody::FlammableUpdate(m) => m.world_construct(world, me),
            UpdateBody::PoisonedBehavior(m) => m.world_construct(world, me),
            UpdateBody::WeaponBonusUpdate(m) => m.world_construct(world, me),
            UpdateBody::PilotFindVehicleUpdate(m) => m.world_construct(world, me),
            UpdateBody::HordeUpdate(m) => m.world_construct(world, me),
            UpdateBody::ToppleUpdate(m) => m.world_construct(world, me),
            UpdateBody::PropagandaTowerBehavior(m) => m.world_construct(world, me),
            UpdateBody::StructureToppleUpdate(m) => m.world_construct(world, me),
            UpdateBody::NeutronBlastBehavior(m) => m.world_construct(world, me),
            UpdateBody::BridgeScaffoldBehavior(m) => m.world_construct(world, me),
            UpdateBody::TensileFormationUpdate(m) => m.world_construct(world, me),
            UpdateBody::LeafletDropBehavior(m) => m.world_construct(world, me),
            UpdateBody::EmpUpdate(m) => m.world_construct(world, me),
            UpdateBody::BoneFxUpdate(m) => m.world_construct(world, me),
            UpdateBody::StructureCollapseUpdate(m) => m.world_construct(world, me),
            UpdateBody::StealthDetectorUpdate(m) => m.world_construct(world, me),
            UpdateBody::StealthUpdate(m) => m.world_construct(world, me),
            UpdateBody::OverchargeBehavior(m) => m.world_construct(world, me),
            UpdateBody::GrantStealthBehavior(m) => m.world_construct(world, me),
            UpdateBody::DynamicGeometryInfoUpdate(m) => m.world_construct(world, me),
            UpdateBody::FirestormDynamicGeometryInfoUpdate(m) => m.world_construct(world, me),
            UpdateBody::AnimationSteeringUpdate(m) => m.world_construct(world, me),
            UpdateBody::RadiusDecalUpdate(m) => m.world_construct(world, me),
            UpdateBody::AutoFindHealingUpdate(m) => m.world_construct(world, me),
            UpdateBody::QueueProductionExitUpdate(m) => m.world_construct(world, me),
            UpdateBody::DefaultProductionExitUpdate(m) => m.world_construct(world, me),
            UpdateBody::SupplyWarehouseCripplingBehavior(m) => m.world_construct(world, me),
            UpdateBody::OclUpdate(m) => m.world_construct(world, me),
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
            UpdateBody::HeightDieUpdate(m) => m.world_construct(world, me),
            UpdateBody::DynamicShroudClearingRangeUpdate(m) => m.world_construct(world, me),
            UpdateBody::ProneUpdate(m) => m.world_construct(world, me),
            UpdateBody::EnemyNearUpdate(m) => m.world_construct(world, me),
            UpdateBody::FloatUpdate(m) => m.world_construct(world, me),
            UpdateBody::HijackerUpdate(m) => m.world_construct(world, me),
            UpdateBody::TechBuildingBehavior(m) => m.world_construct(world, me),
            UpdateBody::FireSpreadUpdate(m) => m.world_construct(world, me),
            UpdateBody::ActiveBody(m) => m.world_construct(world, me),
            UpdateBody::HighlanderBody(m) => m.world_construct(world, me),
            UpdateBody::ImmortalBody(m) => m.world_construct(world, me),
            UpdateBody::StructureBody(m) => m.world_construct(world, me),
            UpdateBody::UndeadBody(m) => m.world_construct(world, me),
            UpdateBody::HiveStructureBody(m) => m.world_construct(world, me),
            UpdateBody::InactiveBody(m) => m.world_construct(world, me),
            UpdateBody::SlowDeathBehavior(m) => m.world_construct(world, me),
            UpdateBody::RebuildHoleBehavior(m) => m.world_construct(world, me),
            UpdateBody::OpenContain(m) => m.world_construct(world, me),
            UpdateBody::TransportContain(m) => m.world_construct(world, me),
            UpdateBody::GarrisonContain(m) => m.world_construct(world, me),
            UpdateBody::CaveContain(m) => m.world_construct(world, me),
            UpdateBody::TunnelContain(m) => m.world_construct(world, me),
            UpdateBody::MobNexusContain(m) => m.world_construct(world, me),
            UpdateBody::ParachuteContain(m) => m.world_construct(world, me),
            UpdateBody::HealContain(m) => m.world_construct(world, me),
            UpdateBody::OverlordContain(m) => m.world_construct(world, me),
            UpdateBody::RiderChangeContain(m) => m.world_construct(world, me),
            UpdateBody::HelixContain(m) => m.world_construct(world, me),
            UpdateBody::RailedTransportContain(m) => m.world_construct(world, me),
            UpdateBody::InternetHackContain(m) => m.world_construct(world, me),
// --- end p1w13-sonnet ---
            _ => {}
        }
    }
}
// --- end p1w10-sonnet ---
