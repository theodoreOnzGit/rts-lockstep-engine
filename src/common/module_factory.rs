// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/ModuleFactory.h
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/ModuleFactory.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ModuleFactory` (`TheModuleFactory`): the registry that makes modules and
//! their INI data from the **names** used in object INI (`Behavior = AutoHealBehavior
//! ModuleTag_01`), and tells what interfaces each name's class has.
//!
//! # The table
//!
//! Upstream's `init()` makes 205 `addModule( Class )` calls, each of which
//! records a class's `friend_newModuleInstance`, `friend_newModuleData`,
//! `getModuleType()` and `getInterfaceMask()` under the **decorated name key**
//! `nameToKey('0' + type, name)`. [`REGISTRATIONS`] is those 205, in the order
//! `init()` makes them (the order matters: each registration assigns a name
//! key, and key values are lockstep state), measured from `ModuleFactory.cpp`
//! and compared with it by `tests/differential_module.rs`, which preprocesses
//! the unchanged `init()` body.
//!
//! - **Six are not compiled into the shipped game.** `POWTruckBehavior`,
//!   `PrisonBehavior`, `PropagandaCenterBehavior`, `PrisonDockUpdate` and
//!   `POWTruckAIUpdate` are under `#ifdef ALLOW_SURRENDER` and
//!   `DemoralizeSpecialPower` under `#ifdef ALLOW_DEMORALIZE`; neither macro is
//!   defined anywhere in the source. [`Registration::in_shipped_build`] says
//!   which; [`ModuleFactory::init`] registers only those (199 calls), and asking
//!   for one of the six is [`ModuleFactoryError::NotInShippedBuild`].
//! - **`WeaponBonusUpgrade` is registered twice** (the same key, the same
//!   values: the second call overwrites the first), so 199 calls make 198 map
//!   entries.
//! - **The type of a registration** is the class's `getModuleType()`:
//!   `MODULETYPE_BEHAVIOR` for everything derived from `BehaviorModule`, and
//!   `MODULETYPE_CLIENT_UPDATE` for the four derived from `ClientUpdateModule`:
//!   `AnimatedParticleSysBoneClientUpdate`, `SwayClientUpdate`,
//!   `BeaconClientUpdate` and **`LaserUpdate`**, which is listed among the
//!   update modules but is a client update module. The type is part of the
//!   key, so it decides whether `Behavior = LaserUpdate` finds it (it does not:
//!   a behavior line asks for type 0). Each class's base chain was followed
//!   through the headers once (2026-10-18, a throw-away script over
//!   `Include/`): 201 behavior, 4 client update. That is a reading of the
//!   headers, not a compile; the C++ reference cannot build the 205 classes.
//!
//! # Translated or not
//!
//! Whether a name has a Rust module is [`is_translated`]: a `match` over the
//! names whose class is a variant of [`Module`]. ~~**None yet**: this layer
//! holds only the bases, and the concrete modules arrive in layer 9, each
//! adding its arm there (its `friend_newModuleInstance`, its data constructor
//! and `getInterfaceMask()`).~~ **CORRECTED 2026-10-20**: layer 9 has begun.
//! A translated class's arm records its data constructor and
//! `getInterfaceMask()`; its `NewModuleProc` makes the **base** class the
//! factory's accessors answer for (an upgrade module's `UpgradeModule`), and
//! the concrete module, with its behaviour, is made for an object by
//! `logic::modules::new_module`. `CommandSetUpgrade` is the first. Looking up a name that is registered but not
//! translated is [`ModuleFactoryError::NotTranslated`]: an **error at the
//! time an object template asks for it, never a silent skip** (upstream
//! cannot fail there: every class exists). So until a name's arm exists, an
//! object INI using it cannot be loaded.
//!
//! # Translation choices
//!
//! - `m_moduleTemplateMap` is a `std::map` ordered by name key; here a
//!   `BTreeMap` of the same keys. `NewModuleProc` / `NewModuleDataProc` are
//!   function pointers, as upstream's (not trait objects): they return the
//!   [`Module`] / [`ModuleDataKind`] enum.
//! - Every lookup (`findModuleTemplate`, and so `newModule`,
//!   `newModuleDataFromINI`, `findModuleInterfaceMask`) calls `nameToKey` on
//!   the decorated name and so **assigns a key** to a name it has not seen,
//!   found or not: they take the [`NameKeyGenerator`].
//! - `newModuleDataFromINI` sets the new data's module tag
//!   (`NAMEKEY( moduleTag )`) before keeping it in `m_moduleDataList`; the
//!   data is created unshared, so setting the tag cannot fail
//!   ([`ModuleFactoryError::ModuleDataShared`] otherwise).
//! - `newModule`'s `#ifdef _DEBUG` block asserts that the registered
//!   `getInterfaceMask()` agrees with which `get...()` interface accessors the
//!   new module answers. That is compiled out of the shipped build; the Rust
//!   keeps it as an error ([`ModuleFactoryError::InterfaceMaskMismatch`])
//!   because it catches a layer-9 registration with a wrong mask, which would
//!   otherwise corrupt `findModuleInterfaceMask` silently.
//! - `reset()` and `update()` are empty upstream and not translated.
//! - The destructor's `delete` of the module data is `Drop`.
//!
//! # Where upstream returns `NULL`
//!
//! `newModule` for an empty name (a `DEBUG_CRASH`), for a name the table does
//! not know, and `newModuleDataFromINI` / `findModuleInterfaceMask` for a name
//! the table does not know, all return `NULL` / 0 in a release build. Here an
//! unknown name is [`ModuleFactoryError::UnknownModule`] (not a way the game
//! can run: object INI that names a module the game does not have is an INI
//! error in intent), an empty name is [`ModuleFactoryError::EmptyName`] for
//! `newModule` and `Ok(None)` / `Ok(0)` for the two that return quietly
//! upstream.

use std::collections::BTreeMap;

use super::ini::{Ini, IniError};
use super::module::{Module, ModuleDataKind, ModuleInterfaceType, ModuleType, ObjectId};
use super::name_key_generator::{NameKeyError, NameKeyGenerator, NameKeyType};
use super::xfer::{Snapshot, Xfer, XferResult, XferVersion};

/// `typedef Module *(*NewModuleProc)(Thing *thing, const ModuleData* moduleData)`.
pub type NewModuleProc = fn(ObjectId, Option<ModuleDataKind>) -> Result<Module, IniError>;

/// `typedef ModuleData* (*NewModuleDataProc)(INI* ini)`.
pub type NewModuleDataProc = fn(Option<&mut Ini<'_>>) -> Result<ModuleDataKind, IniError>;

/// One `addModule( Class )` call of `init()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registration {
    /// The class name (`#classname`), which object INI uses.
    pub name: &'static str,
    /// `classname::getModuleType()`.
    pub module_type: ModuleType,
    /// Whether the call is compiled into the shipped game (false under
    /// `#ifdef ALLOW_SURRENDER` / `ALLOW_DEMORALIZE`).
    pub in_shipped_build: bool,
}

/// The 205 `addModule` calls of `ModuleFactory::init`, in order (see the
/// module docs).
pub const REGISTRATIONS: [Registration; 205] = {
    const fn r(name: &'static str, module_type: ModuleType, in_shipped_build: bool) -> Registration {
        Registration { name, module_type, in_shipped_build }
    }
    [
        // behavior modules
        r("AutoHealBehavior", ModuleType::Behavior, true),
        r("GrantStealthBehavior", ModuleType::Behavior, true),
        r("NeutronBlastBehavior", ModuleType::Behavior, true),
        r("BridgeBehavior", ModuleType::Behavior, true),
        r("BridgeScaffoldBehavior", ModuleType::Behavior, true),
        r("BridgeTowerBehavior", ModuleType::Behavior, true),
        r("CountermeasuresBehavior", ModuleType::Behavior, true),
        r("DumbProjectileBehavior", ModuleType::Behavior, true),
        r("PhysicsBehavior", ModuleType::Behavior, true),
        r("InstantDeathBehavior", ModuleType::Behavior, true),
        r("SlowDeathBehavior", ModuleType::Behavior, true),
        r("HelicopterSlowDeathBehavior", ModuleType::Behavior, true),
        r("NeutronMissileSlowDeathBehavior", ModuleType::Behavior, true),
        r("CaveContain", ModuleType::Behavior, true),
        r("OpenContain", ModuleType::Behavior, true),
        r("OverchargeBehavior", ModuleType::Behavior, true),
        r("HealContain", ModuleType::Behavior, true),
        r("GarrisonContain", ModuleType::Behavior, true),
        r("InternetHackContain", ModuleType::Behavior, true),
        r("TransportContain", ModuleType::Behavior, true),
        r("RiderChangeContain", ModuleType::Behavior, true),
        r("RailedTransportContain", ModuleType::Behavior, true),
        r("MobNexusContain", ModuleType::Behavior, true),
        r("TunnelContain", ModuleType::Behavior, true),
        r("OverlordContain", ModuleType::Behavior, true),
        r("HelixContain", ModuleType::Behavior, true),
        r("ParachuteContain", ModuleType::Behavior, true),
        r("POWTruckBehavior", ModuleType::Behavior, false),
        r("PrisonBehavior", ModuleType::Behavior, false),
        r("PropagandaCenterBehavior", ModuleType::Behavior, false),
        r("PropagandaTowerBehavior", ModuleType::Behavior, true),
        r("BunkerBusterBehavior", ModuleType::Behavior, true),
        r("FireWeaponWhenDamagedBehavior", ModuleType::Behavior, true),
        r("FireWeaponWhenDeadBehavior", ModuleType::Behavior, true),
        r("GenerateMinefieldBehavior", ModuleType::Behavior, true),
        r("ParkingPlaceBehavior", ModuleType::Behavior, true),
        r("FlightDeckBehavior", ModuleType::Behavior, true),
        r("PoisonedBehavior", ModuleType::Behavior, true),
        r("RebuildHoleBehavior", ModuleType::Behavior, true),
        r("SupplyWarehouseCripplingBehavior", ModuleType::Behavior, true),
        r("TechBuildingBehavior", ModuleType::Behavior, true),
        r("MinefieldBehavior", ModuleType::Behavior, true),
        r("BattleBusSlowDeathBehavior", ModuleType::Behavior, true),
        r("JetSlowDeathBehavior", ModuleType::Behavior, true),
        r("RailroadBehavior", ModuleType::Behavior, true),
        r("SpawnBehavior", ModuleType::Behavior, true),
        // die modules
        r("DestroyDie", ModuleType::Behavior, true),
        r("FXListDie", ModuleType::Behavior, true),
        r("CrushDie", ModuleType::Behavior, true),
        r("DamDie", ModuleType::Behavior, true),
        r("CreateCrateDie", ModuleType::Behavior, true),
        r("CreateObjectDie", ModuleType::Behavior, true),
        r("EjectPilotDie", ModuleType::Behavior, true),
        r("SpecialPowerCompletionDie", ModuleType::Behavior, true),
        r("RebuildHoleExposeDie", ModuleType::Behavior, true),
        r("UpgradeDie", ModuleType::Behavior, true),
        r("KeepObjectDie", ModuleType::Behavior, true),
        // update modules
        r("AssistedTargetingUpdate", ModuleType::Behavior, true),
        r("AutoFindHealingUpdate", ModuleType::Behavior, true),
        r("BaseRegenerateUpdate", ModuleType::Behavior, true),
        r("StealthDetectorUpdate", ModuleType::Behavior, true),
        r("StealthUpdate", ModuleType::Behavior, true),
        r("DeletionUpdate", ModuleType::Behavior, true),
        r("SmartBombTargetHomingUpdate", ModuleType::Behavior, true),
        r("DynamicShroudClearingRangeUpdate", ModuleType::Behavior, true),
        r("DeployStyleAIUpdate", ModuleType::Behavior, true),
        r("AssaultTransportAIUpdate", ModuleType::Behavior, true),
        r("HordeUpdate", ModuleType::Behavior, true),
        r("ToppleUpdate", ModuleType::Behavior, true),
        r("EnemyNearUpdate", ModuleType::Behavior, true),
        r("LifetimeUpdate", ModuleType::Behavior, true),
        r("RadiusDecalUpdate", ModuleType::Behavior, true),
        r("EMPUpdate", ModuleType::Behavior, true),
        r("LeafletDropBehavior", ModuleType::Behavior, true),
        r("AutoDepositUpdate", ModuleType::Behavior, true),
        r("WeaponBonusUpdate", ModuleType::Behavior, true),
        r("MissileAIUpdate", ModuleType::Behavior, true),
        r("NeutronMissileUpdate", ModuleType::Behavior, true),
        r("FireSpreadUpdate", ModuleType::Behavior, true),
        r("FireWeaponUpdate", ModuleType::Behavior, true),
        r("FlammableUpdate", ModuleType::Behavior, true),
        r("FloatUpdate", ModuleType::Behavior, true),
        r("TensileFormationUpdate", ModuleType::Behavior, true),
        r("HeightDieUpdate", ModuleType::Behavior, true),
        r("ChinookAIUpdate", ModuleType::Behavior, true),
        r("JetAIUpdate", ModuleType::Behavior, true),
        r("AIUpdateInterface", ModuleType::Behavior, true),
        r("SupplyTruckAIUpdate", ModuleType::Behavior, true),
        r("DeliverPayloadAIUpdate", ModuleType::Behavior, true),
        r("HackInternetAIUpdate", ModuleType::Behavior, true),
        r("DynamicGeometryInfoUpdate", ModuleType::Behavior, true),
        r("FirestormDynamicGeometryInfoUpdate", ModuleType::Behavior, true),
        r("LaserUpdate", ModuleType::ClientUpdate, true),
        r("PointDefenseLaserUpdate", ModuleType::Behavior, true),
        r("CleanupHazardUpdate", ModuleType::Behavior, true),
        r("CommandButtonHuntUpdate", ModuleType::Behavior, true),
        r("PilotFindVehicleUpdate", ModuleType::Behavior, true),
        r("DemoTrapUpdate", ModuleType::Behavior, true),
        r("ParticleUplinkCannonUpdate", ModuleType::Behavior, true),
        r("SpectreGunshipUpdate", ModuleType::Behavior, true),
        r("SpectreGunshipDeploymentUpdate", ModuleType::Behavior, true),
        r("BaikonurLaunchPower", ModuleType::Behavior, true),
        r("BattlePlanUpdate", ModuleType::Behavior, true),
        r("ProjectileStreamUpdate", ModuleType::Behavior, true),
        r("QueueProductionExitUpdate", ModuleType::Behavior, true),
        r("RepairDockUpdate", ModuleType::Behavior, true),
        r("PrisonDockUpdate", ModuleType::Behavior, false),
        r("RailedTransportDockUpdate", ModuleType::Behavior, true),
        r("DefaultProductionExitUpdate", ModuleType::Behavior, true),
        r("SpawnPointProductionExitUpdate", ModuleType::Behavior, true),
        r("SpyVisionUpdate", ModuleType::Behavior, true),
        r("SlavedUpdate", ModuleType::Behavior, true),
        r("MobMemberSlavedUpdate", ModuleType::Behavior, true),
        r("OCLUpdate", ModuleType::Behavior, true),
        r("SpecialAbilityUpdate", ModuleType::Behavior, true),
        r("MissileLauncherBuildingUpdate", ModuleType::Behavior, true),
        r("SupplyCenterProductionExitUpdate", ModuleType::Behavior, true),
        r("SupplyCenterDockUpdate", ModuleType::Behavior, true),
        r("SupplyWarehouseDockUpdate", ModuleType::Behavior, true),
        r("DozerAIUpdate", ModuleType::Behavior, true),
        r("POWTruckAIUpdate", ModuleType::Behavior, false),
        r("RailedTransportAIUpdate", ModuleType::Behavior, true),
        r("ProductionUpdate", ModuleType::Behavior, true),
        r("ProneUpdate", ModuleType::Behavior, true),
        r("StickyBombUpdate", ModuleType::Behavior, true),
        r("FireOCLAfterWeaponCooldownUpdate", ModuleType::Behavior, true),
        r("HijackerUpdate", ModuleType::Behavior, true),
        r("StructureToppleUpdate", ModuleType::Behavior, true),
        r("StructureCollapseUpdate", ModuleType::Behavior, true),
        r("BoneFXUpdate", ModuleType::Behavior, true),
        r("RadarUpdate", ModuleType::Behavior, true),
        r("AnimationSteeringUpdate", ModuleType::Behavior, true),
        r("TransportAIUpdate", ModuleType::Behavior, true),
        r("WanderAIUpdate", ModuleType::Behavior, true),
        r("WaveGuideUpdate", ModuleType::Behavior, true),
        r("WorkerAIUpdate", ModuleType::Behavior, true),
        r("PowerPlantUpdate", ModuleType::Behavior, true),
        r("CheckpointUpdate", ModuleType::Behavior, true),
        // upgrade modules
        r("CostModifierUpgrade", ModuleType::Behavior, true),
        r("ActiveShroudUpgrade", ModuleType::Behavior, true),
        r("ArmorUpgrade", ModuleType::Behavior, true),
        r("CommandSetUpgrade", ModuleType::Behavior, true),
        r("GrantScienceUpgrade", ModuleType::Behavior, true),
        r("PassengersFireUpgrade", ModuleType::Behavior, true),
        r("StatusBitsUpgrade", ModuleType::Behavior, true),
        r("SubObjectsUpgrade", ModuleType::Behavior, true),
        r("StealthUpgrade", ModuleType::Behavior, true),
        r("RadarUpgrade", ModuleType::Behavior, true),
        r("PowerPlantUpgrade", ModuleType::Behavior, true),
        r("LocomotorSetUpgrade", ModuleType::Behavior, true),
        r("ObjectCreationUpgrade", ModuleType::Behavior, true),
        r("ReplaceObjectUpgrade", ModuleType::Behavior, true),
        r("ModelConditionUpgrade", ModuleType::Behavior, true),
        r("UnpauseSpecialPowerUpgrade", ModuleType::Behavior, true),
        r("WeaponBonusUpgrade", ModuleType::Behavior, true),
        r("WeaponSetUpgrade", ModuleType::Behavior, true),
        r("WeaponBonusUpgrade", ModuleType::Behavior, true),
        r("ExperienceScalarUpgrade", ModuleType::Behavior, true),
        r("MaxHealthUpgrade", ModuleType::Behavior, true),
        // create modules
        r("LockWeaponCreate", ModuleType::Behavior, true),
        r("PreorderCreate", ModuleType::Behavior, true),
        r("SupplyCenterCreate", ModuleType::Behavior, true),
        r("SupplyWarehouseCreate", ModuleType::Behavior, true),
        r("SpecialPowerCreate", ModuleType::Behavior, true),
        r("GrantUpgradeCreate", ModuleType::Behavior, true),
        r("VeterancyGainCreate", ModuleType::Behavior, true),
        // damage modules
        r("BoneFXDamage", ModuleType::Behavior, true),
        r("TransitionDamageFX", ModuleType::Behavior, true),
        // collide modules
        r("FireWeaponCollide", ModuleType::Behavior, true),
        r("SquishCollide", ModuleType::Behavior, true),
        r("HealCrateCollide", ModuleType::Behavior, true),
        r("MoneyCrateCollide", ModuleType::Behavior, true),
        r("ShroudCrateCollide", ModuleType::Behavior, true),
        r("UnitCrateCollide", ModuleType::Behavior, true),
        r("VeterancyCrateCollide", ModuleType::Behavior, true),
        r("ConvertToCarBombCrateCollide", ModuleType::Behavior, true),
        r("ConvertToHijackedVehicleCrateCollide", ModuleType::Behavior, true),
        r("SabotageCommandCenterCrateCollide", ModuleType::Behavior, true),
        r("SabotageFakeBuildingCrateCollide", ModuleType::Behavior, true),
        r("SabotageInternetCenterCrateCollide", ModuleType::Behavior, true),
        r("SabotageMilitaryFactoryCrateCollide", ModuleType::Behavior, true),
        r("SabotagePowerPlantCrateCollide", ModuleType::Behavior, true),
        r("SabotageSuperweaponCrateCollide", ModuleType::Behavior, true),
        r("SabotageSupplyCenterCrateCollide", ModuleType::Behavior, true),
        r("SabotageSupplyDropzoneCrateCollide", ModuleType::Behavior, true),
        r("SalvageCrateCollide", ModuleType::Behavior, true),
        // body modules
        r("InactiveBody", ModuleType::Behavior, true),
        r("ActiveBody", ModuleType::Behavior, true),
        r("HighlanderBody", ModuleType::Behavior, true),
        r("ImmortalBody", ModuleType::Behavior, true),
        r("StructureBody", ModuleType::Behavior, true),
        r("HiveStructureBody", ModuleType::Behavior, true),
        r("UndeadBody", ModuleType::Behavior, true),
        // special power modules
        r("CashHackSpecialPower", ModuleType::Behavior, true),
        r("DefectorSpecialPower", ModuleType::Behavior, true),
        r("DemoralizeSpecialPower", ModuleType::Behavior, false),
        r("OCLSpecialPower", ModuleType::Behavior, true),
        r("FireWeaponPower", ModuleType::Behavior, true),
        r("SpecialAbility", ModuleType::Behavior, true),
        r("SpyVisionSpecialPower", ModuleType::Behavior, true),
        r("CashBountyPower", ModuleType::Behavior, true),
        r("CleanupAreaPower", ModuleType::Behavior, true),
        // client update modules
        r("AnimatedParticleSysBoneClientUpdate", ModuleType::ClientUpdate, true),
        r("SwayClientUpdate", ModuleType::ClientUpdate, true),
        r("BeaconClientUpdate", ModuleType::ClientUpdate, true),
    ]
};

/// The number of registrations.
pub const NUM_REGISTRATIONS: usize = 205;

/// The module a name makes, once its class is a variant of [`Module`]: the
/// two constructor functions and `getInterfaceMask()` that `addModule`
/// records. ~~**No concrete module is translated yet**~~ **CORRECTED
/// 2026-10-20**: `CommandSetUpgrade` (the lead's layer-9 example) is the
/// first; layer 9 adds an arm per name.
pub fn translated_module(name: &str) -> Option<ModuleTemplate> {
    match name {
        // --- p1l9-example ---
        "CommandSetUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::command_set_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::command_set_upgrade::CommandSetUpgrade::get_interface_mask()),
        }),
        // --- end p1l9-example ---
        // --- p1l9-upgrades ---
        "WeaponSetUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::weapon_set_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::weapon_set_upgrade::WeaponSetUpgrade::get_interface_mask()),
        }),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        "WeaponBonusUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::weapon_bonus_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::weapon_bonus_upgrade::WeaponBonusUpgrade::get_interface_mask()),
        }),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        "ArmorUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::armor_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::armor_upgrade::ArmorUpgrade::get_interface_mask()),
        }),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        "ModelConditionUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::model_condition_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::model_condition_upgrade::ModelConditionUpgrade::get_interface_mask()),
        }),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        "ExperienceScalarUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::experience_scalar_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::experience_scalar_upgrade::ExperienceScalarUpgrade::get_interface_mask()),
        }),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        "ActiveShroudUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::active_shroud_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::active_shroud_upgrade::ActiveShroudUpgrade::get_interface_mask()),
        }),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        "MaxHealthUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::max_health_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::max_health_upgrade::MaxHealthUpgrade::get_interface_mask()),
        }),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        "GrantScienceUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::grant_science_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::grant_science_upgrade::GrantScienceUpgrade::get_interface_mask()),
        }),
        // --- end p1l9-upgrades ---
        // --- p1w10-sonnet ---
        "StatusBitsUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::status_bits_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::status_bits_upgrade::StatusBitsUpgrade::get_interface_mask()),
        }),
        "LocomotorSetUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::locomotor_set_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::locomotor_set_upgrade::LocomotorSetUpgrade::get_interface_mask()),
        }),
        "PassengersFireUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::passengers_fire_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::passengers_fire_upgrade::PassengersFireUpgrade::get_interface_mask()),
        }),
        "StealthUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::stealth_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::stealth_upgrade::StealthUpgrade::get_interface_mask()),
        }),
        "SubObjectsUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::sub_objects_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::sub_objects_upgrade::SubObjectsUpgrade::get_interface_mask()),
        }),
        "ObjectCreationUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::object_creation_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::object_creation_upgrade::ObjectCreationUpgrade::get_interface_mask()),
        }),
        // --- end p1w10-sonnet ---
        // --- p1w10-sonnet ---
        "PreorderCreate" => Some(ModuleTemplate {
            create_proc: Some(new_create_base),
            create_data_proc: Some(crate::logic::modules::create_host::new_plain_create_module_data),
            which_interfaces: Some(crate::logic::module::create_module::CreateModule::get_interface_mask()),
        }),
        "SpecialPowerCreate" => Some(ModuleTemplate {
            create_proc: Some(new_create_base),
            create_data_proc: Some(crate::logic::modules::create_host::new_plain_create_module_data),
            which_interfaces: Some(crate::logic::module::create_module::CreateModule::get_interface_mask()),
        }),
        "SupplyCenterCreate" => Some(ModuleTemplate {
            create_proc: Some(new_create_base),
            create_data_proc: Some(crate::logic::modules::create_host::new_plain_create_module_data),
            which_interfaces: Some(crate::logic::module::create_module::CreateModule::get_interface_mask()),
        }),
        "SupplyWarehouseCreate" => Some(ModuleTemplate {
            create_proc: Some(new_create_base),
            create_data_proc: Some(crate::logic::modules::create_host::new_plain_create_module_data),
            which_interfaces: Some(crate::logic::module::create_module::CreateModule::get_interface_mask()),
        }),
        "LockWeaponCreate" => Some(ModuleTemplate {
            create_proc: Some(new_create_base),
            create_data_proc: Some(crate::logic::modules::lock_weapon_create::new_module_data),
            which_interfaces: Some(crate::logic::module::create_module::CreateModule::get_interface_mask()),
        }),
        "VeterancyGainCreate" => Some(ModuleTemplate {
            create_proc: Some(new_create_base),
            create_data_proc: Some(crate::logic::modules::veterancy_gain_create::new_module_data),
            which_interfaces: Some(crate::logic::module::create_module::CreateModule::get_interface_mask()),
        }),
        "GrantUpgradeCreate" => Some(ModuleTemplate {
            create_proc: Some(new_create_base),
            create_data_proc: Some(crate::logic::modules::grant_upgrade_create::new_module_data),
            which_interfaces: Some(crate::logic::module::create_module::CreateModule::get_interface_mask()),
        }),
        "LifetimeUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::lifetime_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "DeletionUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::deletion_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "BaseRegenerateUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::base_regenerate_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask() | crate::common::module::MODULEINTERFACE_DAMAGE),
        }),
        "SmartBombTargetHomingUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::smart_bomb_target_homing_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "DestroyDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::new_plain_die_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "KeepObjectDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::new_plain_die_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "UpgradeDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::upgrade_die::new_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        // --- end p1w10-sonnet ---
        // --- p1w11-sonnet ---
        "CostModifierUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::cost_modifier_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::cost_modifier_upgrade::CostModifierUpgrade::get_interface_mask()),
        }),
        "PowerPlantUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::power_plant_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::power_plant_upgrade::PowerPlantUpgrade::get_interface_mask()),
        }),
        "RadarUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::radar_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::radar_upgrade::RadarUpgrade::get_interface_mask()),
        }),
        "UnpauseSpecialPowerUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::unpause_special_power_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::unpause_special_power_upgrade::UnpauseSpecialPowerUpgrade::get_interface_mask()),
        }),
        "ReplaceObjectUpgrade" => Some(ModuleTemplate {
            create_proc: Some(new_upgrade_base),
            create_data_proc: Some(crate::logic::modules::replace_object_upgrade::new_module_data),
            which_interfaces: Some(crate::logic::modules::replace_object_upgrade::ReplaceObjectUpgrade::get_interface_mask()),
        }),
        "DamDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::dam_die::new_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "SpecialPowerCompletionDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::special_power_completion_die::new_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "EjectPilotDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::eject_pilot_die::new_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "InstantDeathBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::instant_death_behavior::new_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "RebuildHoleExposeDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::rebuild_hole_expose_die::new_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "CreateCrateDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::create_crate_die::new_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "CrushDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::crush_die::new_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "CreateObjectDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::create_object_die::new_module_data),
            which_interfaces: Some(crate::logic::module::die_module::DieModule::get_interface_mask()),
        }),
        "PowerPlantUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::power_plant_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "RadarUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::radar_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "HealCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: Some(crate::logic::modules::heal_crate_collide::new_module_data),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "ShroudCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: Some(crate::logic::modules::shroud_crate_collide::new_module_data),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        // --- p1w13-sonnet --- (batch R13: the eight sabotage crates, one class)
        "SabotageCommandCenterCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: crate::logic::modules::sabotage_crate_collide::new_module_data_proc("SabotageCommandCenterCrateCollide"),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "SabotageFakeBuildingCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: crate::logic::modules::sabotage_crate_collide::new_module_data_proc("SabotageFakeBuildingCrateCollide"),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "SabotageInternetCenterCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: crate::logic::modules::sabotage_crate_collide::new_module_data_proc("SabotageInternetCenterCrateCollide"),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "SabotageMilitaryFactoryCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: crate::logic::modules::sabotage_crate_collide::new_module_data_proc("SabotageMilitaryFactoryCrateCollide"),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "SabotagePowerPlantCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: crate::logic::modules::sabotage_crate_collide::new_module_data_proc("SabotagePowerPlantCrateCollide"),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "SabotageSuperweaponCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: crate::logic::modules::sabotage_crate_collide::new_module_data_proc("SabotageSuperweaponCrateCollide"),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "SabotageSupplyCenterCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: crate::logic::modules::sabotage_crate_collide::new_module_data_proc("SabotageSupplyCenterCrateCollide"),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "SabotageSupplyDropzoneCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: crate::logic::modules::sabotage_crate_collide::new_module_data_proc("SabotageSupplyDropzoneCrateCollide"),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "UnitCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: Some(crate::logic::modules::unit_crate_collide::new_module_data),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "VeterancyCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: Some(crate::logic::modules::veterancy_crate_collide::new_module_data),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "SalvageCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: Some(crate::logic::modules::salvage_crate_collide::new_module_data),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "ConvertToCarBombCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: Some(crate::logic::modules::convert_to_car_bomb_crate_collide::new_module_data),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "ConvertToHijackedVehicleCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: Some(crate::logic::modules::convert_to_hijacked_vehicle_crate_collide::new_module_data),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        // --- end p1w13-sonnet ---
        "MoneyCrateCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: Some(crate::logic::modules::money_crate_collide::new_module_data),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "CleanupAreaPower" => Some(ModuleTemplate {
            create_proc: Some(new_special_power_base),
            create_data_proc: Some(crate::logic::modules::cleanup_area_power::new_module_data),
            which_interfaces: Some(crate::logic::module::special_power_module::SpecialPowerModule::get_interface_mask()),
        }),
        "BaikonurLaunchPower" => Some(ModuleTemplate {
            create_proc: Some(new_special_power_base),
            create_data_proc: Some(crate::logic::modules::baikonur_launch_power::new_module_data),
            which_interfaces: Some(crate::logic::module::special_power_module::SpecialPowerModule::get_interface_mask()),
        }),
        "SpyVisionSpecialPower" => Some(ModuleTemplate {
            create_proc: Some(new_special_power_base),
            create_data_proc: Some(crate::logic::modules::spy_vision_special_power::new_module_data),
            which_interfaces: Some(crate::logic::module::special_power_module::SpecialPowerModule::get_interface_mask()),
        }),
        "DefectorSpecialPower" => Some(ModuleTemplate {
            create_proc: Some(new_special_power_base),
            create_data_proc: Some(crate::logic::modules::defector_special_power::new_module_data),
            which_interfaces: Some(crate::logic::module::special_power_module::SpecialPowerModule::get_interface_mask()),
        }),
        "OCLSpecialPower" => Some(ModuleTemplate {
            create_proc: Some(new_special_power_base),
            create_data_proc: Some(crate::logic::modules::ocl_special_power::new_module_data),
            which_interfaces: Some(crate::logic::module::special_power_module::SpecialPowerModule::get_interface_mask()),
        }),
        "CashHackSpecialPower" => Some(ModuleTemplate {
            create_proc: Some(new_special_power_base),
            create_data_proc: Some(crate::logic::modules::cash_hack_special_power::new_module_data),
            which_interfaces: Some(crate::logic::module::special_power_module::SpecialPowerModule::get_interface_mask()),
        }),
        "CashBountyPower" => Some(ModuleTemplate {
            create_proc: Some(new_special_power_base),
            create_data_proc: Some(crate::logic::modules::cash_bounty_power::new_module_data),
            which_interfaces: Some(crate::logic::module::special_power_module::SpecialPowerModule::get_interface_mask()),
        }),
        "SpyVisionUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::spy_vision_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::spy_vision_update::SpyVisionUpdate::get_interface_mask()),
        }),
        "FXListDie" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::fx_list_die::new_module_data),
            which_interfaces: Some(crate::logic::modules::fx_list_die::FXListDie::get_interface_mask()),
        }),
        "FireWeaponWhenDeadBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_die_base),
            create_data_proc: Some(crate::logic::modules::fire_weapon_when_dead_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::fire_weapon_when_dead_behavior::FireWeaponWhenDeadBehavior::get_interface_mask()),
        }),
        "AutoDepositUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::auto_deposit_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        // --- end p1w11-sonnet ---
        // --- p1w12-sonnet ---
        "FireOCLAfterWeaponCooldownUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::fire_ocl_after_weapon_cooldown_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::fire_ocl_after_weapon_cooldown_update::FireOCLAfterWeaponCooldownUpdate::get_interface_mask()),
        }),
        "AutoHealBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::auto_heal_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::auto_heal_behavior::AutoHealBehavior::get_interface_mask()),
        }),
        "CheckpointUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::checkpoint_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "FlammableUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::flammable_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::flammable_update::FlammableUpdate::get_interface_mask()),
        }),
        "PoisonedBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::poisoned_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::poisoned_behavior::PoisonedBehavior::get_interface_mask()),
        }),
        "WeaponBonusUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::weapon_bonus_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::weapon_bonus_update::WeaponBonusUpdate::get_interface_mask()),
        }),
        "PilotFindVehicleUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::pilot_find_vehicle_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::pilot_find_vehicle_update::PilotFindVehicleUpdate::get_interface_mask()),
        }),
        "HordeUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::horde_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::horde_update::HordeUpdate::get_interface_mask()),
        }),
        "ToppleUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::topple_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::topple_update::ToppleUpdate::get_interface_mask()),
        }),
        "PropagandaTowerBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::propaganda_tower_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::propaganda_tower_behavior::PropagandaTowerBehavior::get_interface_mask()),
        }),
        "StickyBombUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::sticky_bomb_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::sticky_bomb_update::StickyBombUpdate::get_interface_mask()),
        }),
        "StructureToppleUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::structure_topple_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::structure_topple_update::StructureToppleUpdate::get_interface_mask()),
        }),
        "BridgeScaffoldBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::new_plain_update_module_data),
            which_interfaces: Some(crate::logic::modules::bridge_scaffold_behavior::BridgeScaffoldBehavior::get_interface_mask()),
        }),
        "NeutronBlastBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::neutron_blast_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::neutron_blast_behavior::NeutronBlastBehavior::get_interface_mask()),
        }),
        "TensileFormationUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::tensile_formation_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::tensile_formation_update::TensileFormationUpdate::get_interface_mask()),
        }),
        "LeafletDropBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::emp_update::new_leaflet_module_data),
            which_interfaces: Some(crate::logic::modules::emp_update::LeafletDropBehavior::get_interface_mask()),
        }),
        "EMPUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::emp_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::emp_update::EmpUpdate::get_interface_mask()),
        }),
        "TransitionDamageFX" => Some(ModuleTemplate {
            create_proc: Some(new_damage_base),
            create_data_proc: Some(crate::logic::modules::transition_damage_fx::new_module_data),
            which_interfaces: Some(crate::logic::modules::transition_damage_fx::TransitionDamageFx::get_interface_mask()),
        }),
        "BridgeTowerBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_behavior_base),
            create_data_proc: Some(crate::logic::modules::bridge_tower_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::bridge_tower_behavior::BridgeTowerBehavior::get_interface_mask()),
        }),
        "BoneFXDamage" => Some(ModuleTemplate {
            create_proc: Some(new_damage_base),
            create_data_proc: Some(crate::logic::modules::new_plain_damage_module_data),
            which_interfaces: Some(crate::logic::module::damage_module::DamageModule::get_interface_mask()),
        }),
        "BoneFXUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::bone_fx_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::bone_fx_update::BoneFxUpdate::get_interface_mask()),
        }),
        "StructureCollapseUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::structure_collapse_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::structure_collapse_update::StructureCollapseUpdate::get_interface_mask()),
        }),
        "StealthDetectorUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::stealth_detector_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::stealth_detector_update::StealthDetectorUpdate::get_interface_mask()),
        }),
        "StealthUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::stealth_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::stealth_update::StealthUpdate::get_interface_mask()),
        }),
        "OverchargeBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::overcharge_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::overcharge_behavior::OverchargeBehavior::get_interface_mask()),
        }),
        "GrantStealthBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::grant_stealth_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::grant_stealth_behavior::GrantStealthBehavior::get_interface_mask()),
        }),
        "DynamicGeometryInfoUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::dynamic_geometry_info_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::dynamic_geometry_info_update::DynamicGeometryInfoUpdate::get_interface_mask()),
        }),
        "FirestormDynamicGeometryInfoUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::firestorm_dynamic_geometry_info_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::firestorm_dynamic_geometry_info_update::FirestormDynamicGeometryInfoUpdate::get_interface_mask()),
        }),
        "AnimationSteeringUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::animation_steering_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::animation_steering_update::AnimationSteeringUpdate::get_interface_mask()),
        }),
        "RadiusDecalUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::radius_decal_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::radius_decal_update::RadiusDecalUpdate::get_interface_mask()),
        }),
        "AutoFindHealingUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::auto_find_healing_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::auto_find_healing_update::AutoFindHealingUpdate::get_interface_mask()),
        }),
        "QueueProductionExitUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::queue_production_exit_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::queue_production_exit_update::QueueProductionExitUpdate::get_interface_mask()),
        }),
        "DefaultProductionExitUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::default_production_exit_update::new_module_data),
            which_interfaces: Some(crate::logic::modules::default_production_exit_update::DefaultProductionExitUpdate::get_interface_mask()),
        }),
        "SupplyWarehouseCripplingBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::supply_warehouse_crippling_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::supply_warehouse_crippling_behavior::SupplyWarehouseCripplingBehavior::get_interface_mask()),
        }),
        "OCLUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::ocl_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        // --- end p1w12-sonnet ---
        // --- p1w13-sonnet ---
        "HeightDieUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::height_die_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "DynamicShroudClearingRangeUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::dynamic_shroud_clearing_range_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "SquishCollide" => Some(ModuleTemplate {
            create_proc: Some(new_collide_base),
            create_data_proc: Some(crate::logic::modules::squish_collide::new_module_data),
            which_interfaces: Some(crate::logic::module::collide_module::CollideModule::get_interface_mask()),
        }),
        "ProneUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::prone_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "EnemyNearUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::enemy_near_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "FloatUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::float_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "HijackerUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::hijacker_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "TechBuildingBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::tech_building_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::tech_building_behavior::TechBuildingBehavior::get_interface_mask()),
        }),
        "FireSpreadUpdate" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::fire_spread_update::new_module_data),
            which_interfaces: Some(crate::logic::module::update_module::UpdateModule::get_interface_mask()),
        }),
        "HighlanderBody" => Some(ModuleTemplate {
            create_proc: Some(new_body_base),
            create_data_proc: Some(crate::logic::modules::active_body::new_module_data),
            which_interfaces: Some(crate::logic::modules::derived_bodies::HighlanderBody::get_interface_mask()),
        }),
        "ImmortalBody" => Some(ModuleTemplate {
            create_proc: Some(new_body_base),
            create_data_proc: Some(crate::logic::modules::active_body::new_module_data),
            which_interfaces: Some(crate::logic::modules::derived_bodies::ImmortalBody::get_interface_mask()),
        }),
        "UndeadBody" => Some(ModuleTemplate {
            create_proc: Some(new_body_base),
            create_data_proc: Some(crate::logic::modules::derived_bodies::new_undead_module_data),
            which_interfaces: Some(crate::logic::modules::derived_bodies::UndeadBody::get_interface_mask()),
        }),
        "HiveStructureBody" => Some(ModuleTemplate {
            create_proc: Some(new_body_base),
            create_data_proc: Some(crate::logic::modules::derived_bodies::new_hive_module_data),
            which_interfaces: Some(crate::logic::modules::derived_bodies::HiveStructureBody::get_interface_mask()),
        }),
        "StructureBody" => Some(ModuleTemplate {
            create_proc: Some(new_body_base),
            create_data_proc: Some(crate::logic::modules::active_body::new_module_data),
            which_interfaces: Some(crate::logic::modules::derived_bodies::StructureBody::get_interface_mask()),
        }),
        "InactiveBody" => Some(ModuleTemplate {
            create_proc: Some(new_body_base),
            create_data_proc: Some(crate::logic::modules::inactive_body::new_module_data),
            which_interfaces: Some(crate::logic::modules::inactive_body::InactiveBody::get_interface_mask()),
        }),
        "ActiveBody" => Some(ModuleTemplate {
            create_proc: Some(new_body_base),
            create_data_proc: Some(crate::logic::modules::active_body::new_module_data),
            which_interfaces: Some(crate::logic::modules::active_body::ActiveBody::get_interface_mask()),
        }),
        "SlowDeathBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::slow_death_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::slow_death_behavior::SlowDeathBehavior::get_interface_mask()),
        }),
        "RebuildHoleBehavior" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::rebuild_hole_behavior::new_module_data),
            which_interfaces: Some(crate::logic::modules::rebuild_hole_behavior::RebuildHoleBehavior::get_interface_mask()),
        }),
        "OpenContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::open_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::open_contain::OpenContain::get_interface_mask()),
        }),
        "InternetHackContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::internet_hack_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::internet_hack_contain::InternetHackContain::get_interface_mask()),
        }),
        "RailedTransportContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::railed_transport_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::railed_transport_contain::RailedTransportContain::get_interface_mask()),
        }),
        "HelixContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::helix_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::helix_contain::HelixContain::get_interface_mask()),
        }),
        "RiderChangeContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::rider_change_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::rider_change_contain::RiderChangeContain::get_interface_mask()),
        }),
        "OverlordContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::overlord_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::overlord_contain::OverlordContain::get_interface_mask()),
        }),
        "HealContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::heal_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::heal_contain::HealContain::get_interface_mask()),
        }),
        "ParachuteContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::parachute_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::parachute_contain::ParachuteContain::get_interface_mask()),
        }),
        "MobNexusContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::mob_nexus_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::mob_nexus_contain::MobNexusContain::get_interface_mask()),
        }),
        "TunnelContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::tunnel_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::tunnel_contain::TunnelContain::get_interface_mask()),
        }),
        "CaveContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::cave_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::cave_contain::CaveContain::get_interface_mask()),
        }),
        "GarrisonContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::garrison_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::garrison_contain::GarrisonContain::get_interface_mask()),
        }),
        "TransportContain" => Some(ModuleTemplate {
            create_proc: Some(new_update_base),
            create_data_proc: Some(crate::logic::modules::transport_contain::new_module_data),
            which_interfaces: Some(crate::logic::modules::transport_contain::TransportContain::get_interface_mask()),
        }),
        // --- end p1w13-sonnet ---
        _ => None,
    }
}

// --- p1w10-sonnet ---
/// The `NewModuleProc` of a translated create module: its `CreateModule` base, made from the `CreateModuleData` base of its
/// data. As [`new_upgrade_base`] does for the upgrade modules, the factory answers what the base answers; the concrete
/// module is made for an object by `logic::modules::new_module`.
fn new_create_base(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
    let base = match data {
        Some(ModuleDataKind::Create(d)) => Some(d),
        Some(ModuleDataKind::GrantUpgradeCreate(d)) => Some(std::sync::Arc::new(d.create.clone())),
        Some(ModuleDataKind::LockWeaponCreate(d)) => Some(std::sync::Arc::new(d.create.clone())),
        Some(ModuleDataKind::VeterancyGainCreate(d)) => Some(std::sync::Arc::new(d.create.clone())),
        _ => None,
    };
    Ok(Module::Create(crate::logic::module::create_module::CreateModule::new(object, base)?))
}

/// The `NewModuleProc` of a translated update module: its `UpdateModule` base, made from the `UpdateModuleData` base of its
/// data (all four are `PHASE_NORMAL`). The concrete module is made for an object by `logic::modules::new_module`.
fn new_update_base(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
    let base = match data {
        Some(ModuleDataKind::Update(d)) => Some(d),
        Some(ModuleDataKind::LifetimeUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::DeletionUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::BaseRegenerateUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::SmartBombTargetHomingUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        // --- p1w11-sonnet ---
        Some(ModuleDataKind::PowerPlantUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::RadarUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::SpyVisionUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::AutoDepositUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::OpenContain(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::TransportContain(d)) => Some(std::sync::Arc::new(d.open.update.clone())),
        Some(ModuleDataKind::GarrisonContain(d)) => Some(std::sync::Arc::new(d.open.update.clone())),
        Some(ModuleDataKind::CaveContain(d)) => Some(std::sync::Arc::new(d.open.update.clone())),
        Some(ModuleDataKind::TunnelContain(d)) => Some(std::sync::Arc::new(d.open.update.clone())),
        Some(ModuleDataKind::MobNexusContain(d)) => Some(std::sync::Arc::new(d.open.update.clone())),
        Some(ModuleDataKind::ParachuteContain(d)) => Some(std::sync::Arc::new(d.open.update.clone())),
        Some(ModuleDataKind::HealContain(d)) => Some(std::sync::Arc::new(d.open.update.clone())),
        Some(ModuleDataKind::OverlordContain(d)) => Some(std::sync::Arc::new(d.transport.open.update.clone())),
        Some(ModuleDataKind::RiderChangeContain(d)) => Some(std::sync::Arc::new(d.transport.open.update.clone())),
        Some(ModuleDataKind::HelixContain(d)) => Some(std::sync::Arc::new(d.transport.open.update.clone())),
        Some(ModuleDataKind::RailedTransportContain(d)) => Some(std::sync::Arc::new(d.open.update.clone())),
        Some(ModuleDataKind::InternetHackContain(d)) => Some(std::sync::Arc::new(d.open.update.clone())),
        // --- end p1w11-sonnet ---
        // --- p1w12-sonnet ---
        Some(ModuleDataKind::FireOCLAfterWeaponCooldownUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::AutoHealBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::CheckpointUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::FlammableUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::PoisonedBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::WeaponBonusUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::PilotFindVehicleUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::HordeUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::ToppleUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::PropagandaTowerBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::StickyBombUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::StructureToppleUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::NeutronBlastBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::TensileFormationUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::LeafletDropBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::EmpUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::BoneFxUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::StructureCollapseUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::StealthDetectorUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::StealthUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::OverchargeBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::GrantStealthBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::DynamicGeometryInfoUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::FirestormDynamicGeometryInfoUpdate(d)) => Some(std::sync::Arc::new(d.geom.update.clone())),
        Some(ModuleDataKind::AnimationSteeringUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::RadiusDecalUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::AutoFindHealingUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::QueueProductionExitUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::DefaultProductionExitUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::SupplyWarehouseCripplingBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::OclUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        // --- end p1w12-sonnet ---
        // --- p1w13-sonnet ---
        Some(ModuleDataKind::HeightDieUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::DynamicShroudClearingRangeUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::ProneUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::EnemyNearUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::FloatUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::HijackerUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::TechBuildingBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::FireSpreadUpdate(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::SlowDeathBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        Some(ModuleDataKind::RebuildHoleBehavior(d)) => Some(std::sync::Arc::new(d.update.clone())),
        // --- end p1w13-sonnet ---
        _ => None,
    };
    Ok(Module::Update(crate::logic::module::update_module::UpdateModule::new(
        object,
        base,
        crate::logic::module::update_module::SleepyUpdatePhase::Normal,
    )?))
}

/// The `NewModuleProc` of a translated die module: its `DieModule` base, made from the `DieModuleData` base of its data.
fn new_die_base(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
    let base = match data {
        Some(ModuleDataKind::Die(d)) => Some(d),
        Some(ModuleDataKind::UpgradeDie(d)) => Some(std::sync::Arc::new(d.die.clone())),
        // --- p1w11-sonnet ---
        Some(ModuleDataKind::DamDie(d)) => Some(std::sync::Arc::new(d.die.clone())),
        Some(ModuleDataKind::SpecialPowerCompletionDie(d)) => Some(std::sync::Arc::new(d.die.clone())),
        Some(ModuleDataKind::EjectPilotDie(d)) => Some(std::sync::Arc::new(d.die.clone())),
        Some(ModuleDataKind::InstantDeathBehavior(d)) => Some(std::sync::Arc::new(d.die.clone())),
        Some(ModuleDataKind::RebuildHoleExposeDie(d)) => Some(std::sync::Arc::new(d.die.clone())),
        Some(ModuleDataKind::CreateCrateDie(d)) => Some(std::sync::Arc::new(d.die.clone())),
        Some(ModuleDataKind::CrushDie(d)) => Some(std::sync::Arc::new(d.die.clone())),
        Some(ModuleDataKind::CreateObjectDie(d)) => Some(std::sync::Arc::new(d.die.clone())),
        Some(ModuleDataKind::FXListDie(d)) => Some(std::sync::Arc::new(d.die.clone())),
        Some(ModuleDataKind::FireWeaponWhenDeadBehavior(d)) => Some(std::sync::Arc::new(d.die.clone())),
        // --- end p1w11-sonnet ---
        _ => None,
    };
    Ok(Module::Die(crate::logic::module::die_module::DieModule::new(object, base)?))
}
// --- end p1w10-sonnet ---

// --- p1w13-sonnet ---
/// batch R38: the `NewModuleProc` of a translated behaviour module whose data is `BehaviorModuleData` itself: its `BehaviorModule` base.
fn new_behavior_base(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
    let base = match data {
        Some(d @ ModuleDataKind::Behavior(_)) => Some(d),
        _ => None,
    };
    Ok(Module::Behavior(crate::logic::module::behavior_module::BehaviorModule::new(object, base)?))
}
// --- end p1w13-sonnet ---

// --- p1w13-sonnet ---
/// batch R30: the `NewModuleProc` of a translated damage module: its `DamageModule` base.
fn new_damage_base(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
    let base = match data {
        Some(ModuleDataKind::Damage(d)) => Some(d),
        Some(ModuleDataKind::TransitionDamageFx(d)) => Some(std::sync::Arc::new(d.damage.clone())),
        _ => None,
    };
    Ok(Module::Damage(crate::logic::module::damage_module::DamageModule::new(object, base)?))
}
// --- end p1w13-sonnet ---

// --- p1w11-sonnet ---
/// The `NewModuleProc` of a translated special power module: its `SpecialPowerModule` base, made from the `SpecialPowerModuleData` base of its data.
fn new_special_power_base(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
    let base = match data {
        Some(ModuleDataKind::SpecialPower(d)) => Some(d),
        // --- p1w11-sonnet ---
        Some(ModuleDataKind::CleanupAreaPower(d)) => Some(std::sync::Arc::new(d.special_power.clone())),
        Some(ModuleDataKind::BaikonurLaunchPower(d)) => Some(std::sync::Arc::new(d.special_power.clone())),
        Some(ModuleDataKind::SpyVisionSpecialPower(d)) => Some(std::sync::Arc::new(d.special_power.clone())),
        Some(ModuleDataKind::DefectorSpecialPower(d)) => Some(std::sync::Arc::new(d.special_power.clone())),
        Some(ModuleDataKind::OCLSpecialPower(d)) => Some(std::sync::Arc::new(d.special_power.clone())),
        Some(ModuleDataKind::CashHackSpecialPower(d)) => Some(std::sync::Arc::new(d.special_power.clone())),
        Some(ModuleDataKind::CashBountyPower(d)) => Some(std::sync::Arc::new(d.special_power.clone())),
        // --- end p1w11-sonnet ---
        _ => None,
    };
    Ok(Module::SpecialPower(crate::logic::module::special_power_module::SpecialPowerModule::new(object, base)?))
}

/// The `NewModuleProc` of a translated collide module: its `CollideModule` base, made from the `CollideModuleData` base of its data.
fn new_collide_base(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
    let base = match data {
        Some(ModuleDataKind::Collide(d)) => Some(d),
        // --- p1w11-sonnet ---
        Some(ModuleDataKind::HealCrateCollide(d)) => Some(std::sync::Arc::new(d.crate_data.collide.clone())),
        Some(ModuleDataKind::ShroudCrateCollide(d)) => Some(std::sync::Arc::new(d.crate_data.collide.clone())),
        Some(ModuleDataKind::MoneyCrateCollide(d)) => Some(std::sync::Arc::new(d.crate_data.collide.clone())),
        Some(ModuleDataKind::SabotageCrateCollide(d)) => Some(std::sync::Arc::new(d.crate_data.collide.clone())),
        Some(ModuleDataKind::UnitCrateCollide(d)) => Some(std::sync::Arc::new(d.crate_data.collide.clone())),
        Some(ModuleDataKind::VeterancyCrateCollide(d)) => Some(std::sync::Arc::new(d.crate_data.collide.clone())),
        Some(ModuleDataKind::SalvageCrateCollide(d)) => Some(std::sync::Arc::new(d.crate_data.collide.clone())),
        Some(ModuleDataKind::ConvertToCarBombCrateCollide(d)) => Some(std::sync::Arc::new(d.crate_data.collide.clone())),
        Some(ModuleDataKind::ConvertToHijackedVehicleCrateCollide(d)) => Some(std::sync::Arc::new(d.crate_data.collide.clone())),
        // --- end p1w11-sonnet ---
        _ => None,
    };
    Ok(Module::Collide(crate::logic::module::collide_module::CollideModule::new(object, base)?))
}
// --- end p1w11-sonnet ---
// --- p1w13-sonnet ---
/// The `NewModuleProc` of a translated body module: its `BodyModule` base, made from the `BodyModuleData` base of its data. The concrete module is made for
/// an object by `logic::modules::new_module`.
fn new_body_base(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
    let base = match data {
        Some(ModuleDataKind::Body(d)) => Some(d),
        // --- p1w13-sonnet ---
        Some(ModuleDataKind::ActiveBody(d)) => Some(std::sync::Arc::new(d.body.clone())),
        Some(ModuleDataKind::Module(d)) => Some(std::sync::Arc::new(crate::logic::module::body_module::BodyModuleData {
            behavior: crate::logic::module::behavior_module::BehaviorModuleData { module_data: (*d).clone() },
        })),
        // --- end p1w13-sonnet ---
        _ => None,
    };
    Ok(Module::Body(crate::logic::module::body_module::BodyModule::new(object, base)?))
}
// --- end p1w13-sonnet ---

/// The `NewModuleProc` of a translated upgrade module: its `UpgradeModule`
/// base, made from the `UpgradeModuleData` base of its data. The factory
/// answers what the base answers (the interface accessors); the concrete
/// module, with its behaviour, is made for an object by
/// `logic::modules::new_module`.
fn new_upgrade_base(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
    let base = match data {
        Some(ModuleDataKind::Upgrade(d)) => Some(d),
        // --- p1l9-example ---
        Some(ModuleDataKind::CommandSetUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1l9-example ---
        // --- p1l9-upgrades ---
        Some(ModuleDataKind::WeaponSetUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        Some(ModuleDataKind::WeaponBonusUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        Some(ModuleDataKind::ArmorUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        Some(ModuleDataKind::ModelConditionUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        Some(ModuleDataKind::ExperienceScalarUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        Some(ModuleDataKind::ActiveShroudUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        Some(ModuleDataKind::MaxHealthUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1l9-upgrades ---
        // --- p1l9-upgrades ---
        Some(ModuleDataKind::GrantScienceUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1l9-upgrades ---
        // --- p1w10-sonnet ---
        Some(ModuleDataKind::StatusBitsUpgrade(d)) => Some(d.upgrade.clone()),
        Some(ModuleDataKind::LocomotorSetUpgrade(d)) => Some(d.upgrade.clone()),
        Some(ModuleDataKind::PassengersFireUpgrade(d)) => Some(d.upgrade.clone()),
        Some(ModuleDataKind::StealthUpgrade(d)) => Some(d.upgrade.clone()),
        Some(ModuleDataKind::SubObjectsUpgrade(d)) => Some(d.upgrade.clone()),
        Some(ModuleDataKind::ObjectCreationUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1w10-sonnet ---
        // --- p1w11-sonnet ---
        Some(ModuleDataKind::CostModifierUpgrade(d)) => Some(d.upgrade.clone()),
        Some(ModuleDataKind::PowerPlantUpgrade(d)) => Some(d.upgrade.clone()),
        Some(ModuleDataKind::RadarUpgrade(d)) => Some(d.upgrade.clone()),
        Some(ModuleDataKind::UnpauseSpecialPowerUpgrade(d)) => Some(d.upgrade.clone()),
        Some(ModuleDataKind::ReplaceObjectUpgrade(d)) => Some(d.upgrade.clone()),
        // --- end p1w11-sonnet ---
        _ => None,
    };
    Ok(Module::Upgrade(crate::logic::module::upgrade_module::UpgradeModule::new(object, base)?))
}

/// Whether the class a registered name stands for is translated.
pub fn is_translated(name: &str) -> bool {
    translated_module(name).is_some()
}

/// `class ModuleTemplate`: what is recorded for one registration.
#[derive(Clone, Copy, Debug)]
pub struct ModuleTemplate {
    /// `m_createProc`; `None` while the class is not translated.
    pub create_proc: Option<NewModuleProc>,
    /// `m_createDataProc`; `None` while the class is not translated.
    pub create_data_proc: Option<NewModuleDataProc>,
    /// `m_whichInterfaces`: the class's `getInterfaceMask()`; `None` while the
    /// class is not translated (unknown without the class).
    pub which_interfaces: Option<ModuleInterfaceType>,
}

impl ModuleTemplate {
    /// The record of a registration that has no Rust class yet.
    pub const UNTRANSLATED: ModuleTemplate = ModuleTemplate { create_proc: None, create_data_proc: None, which_interfaces: None };

    /// Whether a class is behind it.
    pub fn is_translated(&self) -> bool {
        self.create_proc.is_some()
    }
}

/// Why a lookup in the factory failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleFactoryError {
    /// `newModule` for an empty name ("attempting to create module with empty name").
    EmptyName,
    /// A name (with the type asked for) that is not registered.
    UnknownModule { name: Vec<u8>, module_type: ModuleType },
    /// One of the six registrations that are not compiled into the shipped game.
    NotInShippedBuild { name: Vec<u8> },
    /// Registered upstream, but its class is not translated yet (see the module docs).
    NotTranslated { name: Vec<u8>, module_type: ModuleType },
    /// `newModule` made a module whose accessors disagree with the mask it was
    /// registered with (upstream's `_DEBUG` assertion).
    InterfaceMaskMismatch { name: Vec<u8>, registered: ModuleInterfaceType, actual: ModuleInterfaceType },
    /// The data a data constructor returned was already shared, so its tag
    /// could not be set (cannot happen with a constructor that returns a new one).
    ModuleDataShared { name: Vec<u8> },
    /// The constructor failed (reading the INI, or the module's own check).
    Ini(IniError),
    /// `NameKeyGenerator` ran out of keys.
    NameKey(NameKeyError),
}

impl std::fmt::Display for ModuleFactoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        match self {
            ModuleFactoryError::EmptyName => write!(f, "attempting to create module with empty name"),
            ModuleFactoryError::UnknownModule { name, module_type } => write!(f, "module name '{}' (type {:?}) not found", n(name), module_type),
            ModuleFactoryError::NotInShippedBuild { name } => {
                write!(f, "module '{}' is registered only under ALLOW_SURRENDER / ALLOW_DEMORALIZE, which the shipped game does not define", n(name))
            }
            ModuleFactoryError::NotTranslated { name, module_type } => {
                write!(f, "module '{}' (type {:?}) is registered upstream but not translated yet", n(name), module_type)
            }
            ModuleFactoryError::InterfaceMaskMismatch { name, registered, actual } => {
                write!(f, "getInterfaceMask bad for '{}': registered {registered:#x}, accessors answer {actual:#x}", n(name))
            }
            ModuleFactoryError::ModuleDataShared { name } => write!(f, "module data of '{}' is shared; cannot set its tag", n(name)),
            ModuleFactoryError::Ini(e) => e.fmt(f),
            ModuleFactoryError::NameKey(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ModuleFactoryError {}

impl From<IniError> for ModuleFactoryError {
    fn from(e: IniError) -> Self {
        ModuleFactoryError::Ini(e)
    }
}

impl From<NameKeyError> for ModuleFactoryError {
    fn from(e: NameKeyError) -> Self {
        ModuleFactoryError::NameKey(e)
    }
}

/// `class ModuleFactory` (`TheModuleFactory`).
#[derive(Clone, Debug, Default)]
pub struct ModuleFactory {
    /// `m_moduleTemplateMap`.
    module_template_map: BTreeMap<NameKeyType, ModuleTemplate>,
    /// `m_moduleDataList`: "const ModuleData*" kept for the snapshot.
    module_data_list: Vec<ModuleDataKind>,
}

impl ModuleFactory {
    /// `ModuleFactory()`: empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// `makeDecoratedNameKey(name, type)`: the name key of `'0' + type`
    /// followed by the name.
    pub fn make_decorated_name_key(names: &mut NameKeyGenerator, name: &[u8], module_type: ModuleType) -> Result<NameKeyType, NameKeyError> {
        let mut tmp = Vec::with_capacity(name.len() + 1);
        tmp.push(b'0' + module_type as u8);
        tmp.extend_from_slice(name);
        names.try_name_to_key(&tmp)
    }

    /// `addModuleInternal(proc, dataproc, type, name, whichIntf)`: record a
    /// registration under its decorated key (replacing one with the same key).
    pub fn add_module_internal(
        &mut self,
        names: &mut NameKeyGenerator,
        template: ModuleTemplate,
        module_type: ModuleType,
        name: &str,
    ) -> Result<(), NameKeyError> {
        let key = Self::make_decorated_name_key(names, name.as_bytes(), module_type)?;
        self.module_template_map.insert(key, template);
        Ok(())
    }

    /// `init()`: register every class, in [`REGISTRATIONS`] order: the 199
    /// compiled into the shipped game (a registration with no Rust class is
    /// recorded as [`ModuleTemplate::UNTRANSLATED`], so that its key is
    /// assigned as upstream's is). The six `#ifdef`'d ones make no key.
    pub fn init(&mut self, names: &mut NameKeyGenerator) -> Result<(), NameKeyError> {
        for r in REGISTRATIONS.iter().filter(|r| r.in_shipped_build) {
            let template = match translated_module(r.name) {
                Some(t) => t,
                None => ModuleTemplate::UNTRANSLATED,
            };
            self.add_module_internal(names, template, r.module_type, r.name)?;
        }
        Ok(())
    }

    /// The registration of a name (any type), if upstream has one.
    pub fn registration_of(name: &[u8]) -> Option<&'static Registration> {
        REGISTRATIONS.iter().find(|r| r.name.as_bytes() == name)
    }

    /// `findModuleTemplate(name, type)`: the record under the decorated key
    /// (assigning that key if new), or why there is none.
    pub fn find_module_template(
        &self,
        names: &mut NameKeyGenerator,
        name: &[u8],
        module_type: ModuleType,
    ) -> Result<ModuleTemplate, ModuleFactoryError> {
        let key = Self::make_decorated_name_key(names, name, module_type)?;
        match self.module_template_map.get(&key) {
            Some(t) if t.is_translated() => Ok(*t),
            Some(_) => Err(ModuleFactoryError::NotTranslated { name: name.to_vec(), module_type }),
            None => match Self::registration_of(name) {
                Some(r) if !r.in_shipped_build => Err(ModuleFactoryError::NotInShippedBuild { name: name.to_vec() }),
                _ => Err(ModuleFactoryError::UnknownModule { name: name.to_vec(), module_type }),
            },
        }
    }

    /// `newModule(thing, name, data, type)`: a new module of the class `name`
    /// stands for.
    pub fn new_module(
        &self,
        names: &mut NameKeyGenerator,
        object: ObjectId,
        name: &[u8],
        module_data: Option<ModuleDataKind>,
        module_type: ModuleType,
    ) -> Result<Module, ModuleFactoryError> {
        // sanity
        if name.is_empty() {
            return Err(ModuleFactoryError::EmptyName);
        }
        let mt = self.find_module_template(names, name, module_type)?;
        let create = mt.create_proc.ok_or_else(|| ModuleFactoryError::NotTranslated { name: name.to_vec(), module_type })?;
        let module = create(object, module_data)?;
        if module_type == ModuleType::Behavior {
            if let Some(registered) = mt.which_interfaces {
                let actual = module.interfaces_present();
                if registered != actual {
                    return Err(ModuleFactoryError::InterfaceMaskMismatch { name: name.to_vec(), registered, actual });
                }
            }
        }
        Ok(module)
    }

    /// `newModuleDataFromINI(ini, name, type, moduleTag)`: the data of the
    /// class `name` stands for, read from `ini` if there is one, tagged with
    /// the key of `module_tag` and kept in the factory's list. `Ok(None)` for
    /// an empty name, as upstream returns `NULL` there without complaint.
    pub fn new_module_data_from_ini(
        &mut self,
        names: &mut NameKeyGenerator,
        ini: Option<&mut Ini<'_>>,
        name: &[u8],
        module_type: ModuleType,
        module_tag: &[u8],
    ) -> Result<Option<ModuleDataKind>, ModuleFactoryError> {
        if name.is_empty() {
            return Ok(None);
        }
        let mt = self.find_module_template(names, name, module_type)?;
        let create_data = mt.create_data_proc.ok_or_else(|| ModuleFactoryError::NotTranslated { name: name.to_vec(), module_type })?;
        let mut md = create_data(ini)?;
        let tag = names.try_name_to_key(module_tag)?;
        if !md.set_module_tag_name_key(tag) {
            return Err(ModuleFactoryError::ModuleDataShared { name: name.to_vec() });
        }
        self.module_data_list.push(md.clone());
        Ok(Some(md))
    }

    /// `findModuleInterfaceMask(name, type)`: the registered
    /// `getInterfaceMask()`; 0 for an empty name.
    pub fn find_module_interface_mask(
        &self,
        names: &mut NameKeyGenerator,
        name: &[u8],
        module_type: ModuleType,
    ) -> Result<ModuleInterfaceType, ModuleFactoryError> {
        if name.is_empty() {
            return Ok(0);
        }
        let mt = self.find_module_template(names, name, module_type)?;
        mt.which_interfaces.ok_or_else(|| ModuleFactoryError::NotTranslated { name: name.to_vec(), module_type })
    }

    /// The number of records in the map (`m_moduleTemplateMap.size()`).
    pub fn template_count(&self) -> usize {
        self.module_template_map.len()
    }

    /// The data `newModuleDataFromINI` made, in order (`m_moduleDataList`).
    pub fn module_data_list(&self) -> &[ModuleDataKind] {
        &self.module_data_list
    }
}

impl Snapshot for ModuleFactory {
    /// `ModuleFactory::crc`: every kept data's `crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        for md in &self.module_data_list {
            md.crc(xfer)?;
        }
        Ok(())
    }

    /// `ModuleFactory::xfer` (version 1): the version, then every kept data's
    /// `xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        for md in &self.module_data_list {
            md.xfer(xfer)?;
        }
        Ok(())
    }

    /// `ModuleFactory::loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniErrorKind, IniLoadType, IniResult};
    use crate::common::module::{MODULEINTERFACE_DIE, MODULEINTERFACE_UPDATE};
    use crate::logic::module::test_support::save_bytes;
    use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData};
    use std::sync::Arc;

    fn fresh_names() -> NameKeyGenerator {
        let mut n = NameKeyGenerator::new();
        n.reset();
        n
    }

    #[test]
    fn the_table_has_205_registrations_199_in_the_shipped_build() {
        assert_eq!(REGISTRATIONS.len(), NUM_REGISTRATIONS);
        let not_shipped: Vec<&str> = REGISTRATIONS.iter().filter(|r| !r.in_shipped_build).map(|r| r.name).collect();
        assert_eq!(
            not_shipped,
            ["POWTruckBehavior", "PrisonBehavior", "PropagandaCenterBehavior", "PrisonDockUpdate", "POWTruckAIUpdate", "DemoralizeSpecialPower"]
        );
        assert_eq!(REGISTRATIONS.iter().filter(|r| r.in_shipped_build).count(), 199);
        // the order of init(): first, last, and a few in between
        assert_eq!(REGISTRATIONS[0].name, "AutoHealBehavior");
        assert_eq!(REGISTRATIONS[204].name, "BeaconClientUpdate");
        let at = |n: &str| REGISTRATIONS.iter().position(|r| r.name == n).unwrap();
        assert_eq!((at("SpawnBehavior"), at("DestroyDie"), at("AssistedTargetingUpdate")), (45, 46, 57));
        assert!(at("OpenContain") < at("POWTruckBehavior") && at("POWTruckBehavior") < at("PropagandaTowerBehavior"));
        assert!(at("OpenContain") < at("POWTruckBehavior") && at("POWTruckBehavior") < at("StickyBombUpdate"));
        // only WeaponBonusUpgrade is there twice
        let mut seen = std::collections::BTreeMap::new();
        for r in &REGISTRATIONS {
            *seen.entry(r.name).or_insert(0) += 1;
        }
        let twice: Vec<_> = seen.iter().filter(|(_, c)| **c > 1).collect();
        assert_eq!(twice, [(&"WeaponBonusUpgrade", &2)]);
        // the types: the four client update modules, LaserUpdate among the update modules
        let client: Vec<&str> = REGISTRATIONS.iter().filter(|r| r.module_type == ModuleType::ClientUpdate).map(|r| r.name).collect();
        assert_eq!(client, ["LaserUpdate", "AnimatedParticleSysBoneClientUpdate", "SwayClientUpdate", "BeaconClientUpdate"]);
        assert!(REGISTRATIONS.iter().all(|r| r.module_type != ModuleType::Draw));
    }

    // ~~no_concrete_module_is_translated_yet~~ CORRECTED 2026-10-20: layer 9 has begun.
    #[test]
    fn only_the_translated_modules_are_translated() {
        // --- p1l9-example ---
        let translated: Vec<&str> = REGISTRATIONS.iter().map(|r| r.name).filter(|n| is_translated(n)).collect();
        assert!(translated.contains(&"CommandSetUpgrade"));
        // every translated name is registered, and has all three parts
        for n in &translated {
            let t = translated_module(n).unwrap();
            assert!(t.create_proc.is_some() && t.create_data_proc.is_some() && t.which_interfaces.is_some(), "{n}");
        }
        // some registered module is always still untranslated until the port is complete
        assert!(REGISTRATIONS.iter().any(|r| !is_translated(r.name)));
        // --- end p1l9-example ---
        assert!(!ModuleTemplate::UNTRANSLATED.is_translated());
    }

    #[test]
    fn init_assigns_one_key_per_distinct_decorated_name_in_registration_order() {
        let mut names = fresh_names();
        let mut f = ModuleFactory::new();
        f.init(&mut names).unwrap();
        assert_eq!(f.template_count(), 198, "199 shipped calls, WeaponBonusUpgrade twice");
        // the keys are consecutive in order of first registration (looking a registered name up assigns nothing new)
        let mut expect: Option<u32> = None;
        let mut done = std::collections::BTreeSet::new();
        for r in REGISTRATIONS.iter().filter(|r| r.in_shipped_build) {
            let mut tmp = vec![b'0' + r.module_type as u8];
            tmp.extend_from_slice(r.name.as_bytes());
            let key = names.try_name_to_key(&tmp).unwrap().0;
            if done.insert(tmp) {
                if let Some(prev) = expect {
                    assert_eq!(key, prev + 1, "{}", r.name);
                }
                expect = Some(key);
            }
        }
        assert_eq!(done.len(), 198);
        // a name that is not registered, then a new name: the lookup assigned a key to it
        let before = names.try_name_to_key(b"Probe").unwrap().0;
        let _ = f.find_module_template(&mut names, b"NoSuchModule", ModuleType::Behavior);
        assert_eq!(names.try_name_to_key(b"Probe2").unwrap().0, before + 2);
    }

    #[test]
    fn looking_up_an_untranslated_or_unknown_name_is_an_error() {
        let mut names = fresh_names();
        let mut f = ModuleFactory::new();
        f.init(&mut names).unwrap();
        let name = |s: &str| s.as_bytes().to_vec();
        let find = |f: &ModuleFactory, names: &mut NameKeyGenerator, n: &str, t| f.find_module_template(names, n.as_bytes(), t).unwrap_err();
        assert_eq!(
            find(&f, &mut names, "SpectreGunshipUpdate", ModuleType::Behavior),
            ModuleFactoryError::NotTranslated { name: name("SpectreGunshipUpdate"), module_type: ModuleType::Behavior }
        );
        assert_eq!(
            find(&f, &mut names, "BeaconClientUpdate", ModuleType::ClientUpdate),
            ModuleFactoryError::NotTranslated { name: name("BeaconClientUpdate"), module_type: ModuleType::ClientUpdate }
        );
        // the right name with the wrong type is a different key: unknown
        assert_eq!(
            find(&f, &mut names, "LaserUpdate", ModuleType::Behavior),
            ModuleFactoryError::UnknownModule { name: name("LaserUpdate"), module_type: ModuleType::Behavior }
        );
        assert!(matches!(find(&f, &mut names, "LaserUpdate", ModuleType::ClientUpdate), ModuleFactoryError::NotTranslated { .. }));
        // the six that the shipped game does not register
        assert_eq!(find(&f, &mut names, "PrisonBehavior", ModuleType::Behavior), ModuleFactoryError::NotInShippedBuild { name: name("PrisonBehavior") });
        assert_eq!(find(&f, &mut names, "DemoralizeSpecialPower", ModuleType::Behavior), ModuleFactoryError::NotInShippedBuild { name: name("DemoralizeSpecialPower") });
        // a name upstream has never heard of (and case counts in a key)
        assert!(matches!(find(&f, &mut names, "NoSuchModule", ModuleType::Behavior), ModuleFactoryError::UnknownModule { .. }));
        assert!(matches!(find(&f, &mut names, "autohealbehavior", ModuleType::Behavior), ModuleFactoryError::NotTranslated { .. } | ModuleFactoryError::UnknownModule { .. }));
        // the three public entry points, for an untranslated name
        let pending = REGISTRATIONS.iter().find(|r| r.module_type == ModuleType::Behavior && !is_translated(r.name)).map(|r| r.name.as_bytes().to_vec()).unwrap();
        let pending = &pending[..];
        let e = f.new_module(&mut names, 1, pending, None, ModuleType::Behavior).unwrap_err();
        assert!(matches!(e, ModuleFactoryError::NotTranslated { .. }));
        let e = f.new_module_data_from_ini(&mut names, None, pending, ModuleType::Behavior, b"ModuleTag_01").unwrap_err();
        assert!(matches!(e, ModuleFactoryError::NotTranslated { .. }));
        let e = f.find_module_interface_mask(&mut names, pending, ModuleType::Behavior).unwrap_err();
        assert!(matches!(e, ModuleFactoryError::NotTranslated { .. }));
        assert!(f.module_data_list().is_empty());
        // an empty name
        assert_eq!(f.new_module(&mut names, 1, b"", None, ModuleType::Behavior).unwrap_err(), ModuleFactoryError::EmptyName);
        assert_eq!(f.find_module_interface_mask(&mut names, b"", ModuleType::Behavior), Ok(0));
        assert!(matches!(f.new_module_data_from_ini(&mut names, None, b"", ModuleType::Behavior, b"x"), Ok(None)));
    }

    fn make_update(object: ObjectId, data: Option<ModuleDataKind>) -> Result<Module, IniError> {
        let d = match data {
            Some(ModuleDataKind::Update(d)) => Some(d),
            _ => None,
        };
        Ok(Module::Update(UpdateModule::new(object, d, SleepyUpdatePhase::Normal)?))
    }
    fn make_update_data(ini: Option<&mut Ini<'_>>) -> Result<ModuleDataKind, IniError> {
        Ok(ModuleDataKind::Update(Arc::new(UpdateModuleData::friend_new_module_data(ini)?)))
    }
    fn test_template(mask: ModuleInterfaceType) -> ModuleTemplate {
        ModuleTemplate { create_proc: Some(make_update), create_data_proc: Some(make_update_data), which_interfaces: Some(mask) }
    }

    #[test]
    fn a_registered_class_makes_modules_data_and_masks() {
        let mut names = fresh_names();
        let mut f = ModuleFactory::new();
        f.init(&mut names).unwrap();
        f.add_module_internal(&mut names, test_template(MODULEINTERFACE_UPDATE), ModuleType::Behavior, "TestUpdate").unwrap();
        assert_eq!(f.template_count(), 199);
        assert_eq!(f.find_module_interface_mask(&mut names, b"TestUpdate", ModuleType::Behavior), Ok(MODULEINTERFACE_UPDATE));

        // the data from an ini block, tagged and kept
        struct C {
            f: ModuleFactory,
            names: NameKeyGenerator,
            out: Vec<Option<ModuleDataKind>>,
        }
        fn block(ini: &mut Ini<'_>, c: &mut C) -> IniResult<()> {
            let r = c.f.new_module_data_from_ini(&mut c.names, Some(ini), b"TestUpdate", ModuleType::Behavior, b"ModuleTag_Test");
            c.out.push(r.map_err(|e| match e {
                ModuleFactoryError::Ini(i) => i,
                other => panic!("{other}"),
            })?);
            Ok(())
        }
        let blocks = [BlockParse { token: "B", parse: block }];
        let mut c = C { f, names, out: Vec::new() };
        let r = Ini::load(b"f.ini", b"B\nEnd\nB\n Bad = 1\nEnd\n", IniLoadType::Overwrite, None, &blocks, &mut c);
        assert_eq!(r.unwrap_err().kind, IniErrorKind::UnknownToken);
        assert_eq!(c.out.len(), 1);
        let md = c.out[0].clone().unwrap();
        let tag = c.names.try_name_to_key(b"ModuleTag_Test").unwrap();
        assert_eq!(md.module_tag_name_key().unwrap(), tag);
        assert_eq!(c.f.module_data_list().len(), 1, "a data that failed to read is not kept");

        // the module
        let m = c.f.new_module(&mut c.names, 5, b"TestUpdate", Some(md.clone()), ModuleType::Behavior).unwrap();
        assert_eq!(m.interface_mask(), MODULEINTERFACE_UPDATE);
        assert_eq!(m.module_tag_name_key().unwrap(), tag);
        assert!(m.get_update().is_some());
        // a module without data: the constructor's INI_INVALID_DATA
        let e = c.f.new_module(&mut c.names, 5, b"TestUpdate", None, ModuleType::Behavior).unwrap_err();
        assert_eq!(e, ModuleFactoryError::Ini(IniError::new(IniErrorKind::InvalidData)));

        // the snapshot: a version, and nothing from data that has nothing to save
        assert_eq!(save_bytes(&mut c.f).unwrap(), [1]);
    }

    #[test]
    fn a_wrong_mask_is_caught_when_the_module_is_made() {
        let mut names = fresh_names();
        let mut f = ModuleFactory::new();
        f.add_module_internal(&mut names, test_template(MODULEINTERFACE_DIE), ModuleType::Behavior, "BadMask").unwrap();
        let data = ModuleDataKind::Update(Arc::new(UpdateModuleData::new()));
        let e = f.new_module(&mut names, 1, b"BadMask", Some(data), ModuleType::Behavior).unwrap_err();
        assert_eq!(e, ModuleFactoryError::InterfaceMaskMismatch { name: b"BadMask".to_vec(), registered: MODULEINTERFACE_DIE, actual: MODULEINTERFACE_UPDATE });
    }

    #[test]
    fn a_second_registration_of_a_key_replaces_the_first() {
        let mut names = fresh_names();
        let mut f = ModuleFactory::new();
        f.add_module_internal(&mut names, test_template(1), ModuleType::Behavior, "Twice").unwrap();
        f.add_module_internal(&mut names, test_template(2), ModuleType::Behavior, "Twice").unwrap();
        assert_eq!(f.template_count(), 1);
        assert_eq!(f.find_module_interface_mask(&mut names, b"Twice", ModuleType::Behavior), Ok(2));
        // another type is another key
        f.add_module_internal(&mut names, test_template(4), ModuleType::ClientUpdate, "Twice").unwrap();
        assert_eq!(f.template_count(), 2);
        assert_eq!(f.find_module_interface_mask(&mut names, b"Twice", ModuleType::ClientUpdate), Ok(4));
    }

    #[test]
    fn the_error_texts_name_the_module() {
        let e = ModuleFactoryError::NotTranslated { name: b"ActiveBody".to_vec(), module_type: ModuleType::Behavior };
        assert!(e.to_string().contains("ActiveBody") && e.to_string().contains("not translated"));
        assert!(ModuleFactoryError::UnknownModule { name: b"X".to_vec(), module_type: ModuleType::Behavior }.to_string().contains("not found"));
        assert!(ModuleFactoryError::EmptyName.to_string().contains("empty name"));
    }
}
