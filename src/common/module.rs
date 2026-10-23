// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Module.h
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/Module.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, September 2001 (Module.cpp: Steven Johnson)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Module` and `ModuleData`: what every module of an `Object` or `Drawable`
//! is, and the per-INI-block data every module shares.
//!
//! # The enum design (no trait objects)
//!
//! Upstream is a class hierarchy: `Module` -> `ObjectModule` ->
//! `BehaviorModule` -> `UpdateModule`, `UpgradeModule`, `DieModule`, ...,
//! each concrete module (`ActiveBody`, `LifetimeUpdate`, ...) deriving from
//! one or more of them, and `ModuleFactory` creating them by name through
//! function pointers. Here:
//!
//! - **[`Module`] is an enum with one variant per module class**, each
//!   holding that class's own state *and* the state of the base classes it
//!   derives from (as a field, `behavior: BehaviorModule`, and so on down to
//!   [`ModuleBase`], the members of upstream `Module` itself). Virtual calls
//!   are a `match` over the variants, so dispatch order is explicit.
//! - **[`ModuleDataKind`] is the same for the data:** one variant per
//!   `...ModuleData` class, each an `Arc` to the data that every module of
//!   that kind shares (upstream: `const ModuleData*`).
//! - **Only the bases and interfaces are variants so far.** `ObjectModule`,
//!   `DrawableModule`, `BehaviorModule`, `BodyModule`, `CollideModule`,
//!   `CreateModule`, `DamageModule`, `DestroyModule`, `DieModule`,
//!   `SpecialPowerModule`, `SpecialPowerUpdateModule`, `UpdateModule` and
//!   `UpgradeModule` are the variants; every *concrete* module (`ActiveBody`,
//!   `AutoHealBehavior`, ...) arrives in layer 9 as a new variant whose
//!   struct holds its base's struct, and adds an arm to each `match` below.
//!   `ContainModule.h` declares only an interface and the data typedef (the
//!   class is `OpenContain`, layer 9), so there is no `ContainModule`
//!   variant. The registry that creates modules by name is
//!   [`super::module_factory`].
//! - **Abstract classes are kept.** `Module`, `ObjectModule` and so on are
//!   abstract upstream (pure `getModuleNameKey`, `update`, ...), so a base
//!   variant is never built by the factory; it exists so that the logic that
//!   lives in the base (state, constructor checks, the `xfer` chain, default
//!   virtuals) has one place and one test.
//!
//! # `ModuleData`
//!
//! [`ModuleData`] is the base data: the **module tag**, a name key "unique
//! among all modules for an object instance", set by the factory right after
//! the data is read
//! ([`ModuleFactory::new_module_data_from_ini`](super::module_factory::ModuleFactory::new_module_data_from_ini)). Upstream's
//! constructor (`ModuleData() { }`) does not initialise it, so reading it
//! before it is set is [`Undefined::UninitialisedModuleTag`]. The virtual
//! `isAiModuleData`, `getAsW3DModelDrawModuleData`, `getAsW3DTreeDrawModuleData`
//! and `getMinimumRequiredGameLOD` are answered by [`ModuleDataKind`]; the two
//! `W3D` downcasts are client hacks ("ugh, hack (srj)") and are cut points
//! for the client (nothing in logic calls them).
//!
//! `Module::friend_newModuleData(ini)` reads the base data with
//! `ini->initFromINI(data, 0)` ("this is just so that an `end` token is
//! required"): a **null** parse table. `End` is accepted; any field line
//! dereferences the null table, which is [`Undefined::NullFieldParseTable`].
//!
//! # Left out (cut points, layer 8/7)
//!
//! - `ObjectModule(Thing*, ...)` / `DrawableModule(Thing*, ...)` convert the
//!   `Thing` with `AsObject` / `AsDrawable` and keep the pointer. The Rust
//!   keeps the [`ObjectId`] of the object (the `Object` is layer 8); a
//!   `Thing` that is not an `Object`, or is null, is only a
//!   `DEBUG_ASSERTCRASH` upstream (compiled out in the shipped build, the
//!   module then holds a null pointer) and cannot be expressed here. A
//!   `Drawable` has an id of its own that is not translated; the Rust keeps
//!   nothing for it.
//! - `ObjectModule::onCapture(Player*, Player*)` and the other virtual
//!   no-ops that take game objects (`onObjectCreated`, `onDrawableBoundToObject`,
//!   `preloadAssets(TimeOfDay)`, `onDelete`, `onDisabledEdge`): every one is
//!   an empty function in the bases, so nothing here calls them; the
//!   concrete modules of layer 9 override them, and then [`Module`] gets the
//!   dispatching methods.
//! - `BehaviorModuleInterface`'s twenty interface-acquisition virtuals
//!   (`getParkingPlaceBehaviorInterface`, `getAIUpdateInterface`, ...) return
//!   `NULL` in every base; they are added with the first concrete module that
//!   returns something else.
//! - `MAKE_STANDARD_MODULE_MACRO`'s `friend_newModuleInstance` and
//!   `getModuleNameKey` are per concrete class and arrive with them.
//!
//! # Where upstream is undefined
//!
//! - Reading the module tag before it is set:
//!   [`Undefined::UninitialisedModuleTag`].
//! - A field line in a base `ModuleData` block:
//!   [`Undefined::NullFieldParseTable`].
//!
//! # Verification
//!
//! `tests/differential_module.rs` compiles `Module.cpp` unchanged
//! (`differential/wrap_module.cpp`, see there) and compares `friend_newModuleData`
//! (what it accepts and throws), `ObjectModule`'s constructor check, the
//! interface-mask and module-type constants, and the `xfer` chains of the base
//! classes byte for byte.

use std::sync::Arc;

use super::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined};
use super::name_key_generator::NameKeyType;
use super::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};
use crate::logic::module::body_module::{BodyModule, BodyModuleData};
use crate::logic::module::collide_module::{CollideModule, CollideModuleData};
use crate::logic::module::create_module::{CreateModule, CreateModuleData};
use crate::logic::module::damage_module::{DamageModule, DamageModuleData};
use crate::logic::module::destroy_module::DestroyModule;
use crate::logic::module::die_module::{DieModule, DieModuleData};
use crate::logic::module::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
use crate::logic::module::special_power_update_module::SpecialPowerUpdateModule;
use crate::logic::module::update_module::{UpdateModule, UpdateModuleData};
use crate::logic::module::upgrade_module::{UpgradeModule, UpgradeModuleData};

/// `enum ModuleType`. (An `i32`, like the other enums that are saved: the
/// factory builds a name key from `'0' + type`.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i32)]
pub enum ModuleType {
    /// `MODULETYPE_BEHAVIOR`.
    Behavior = 0,
    // Upstream: "drawable module types - you should *NOT* remove drawable
    // module types, we write modules into save game files in buckets of
    // module types ... if you do remove one here you will have to update the
    // xfer code for a drawable. ALSO note that new drawable module types
    // should go at the end of the existing drawable modules".
    /// `MODULETYPE_DRAW`.
    Draw = 1,
    /// `MODULETYPE_CLIENT_UPDATE`.
    ClientUpdate = 2,
}

/// `NUM_MODULE_TYPES` ("keep this last!").
pub const NUM_MODULE_TYPES: i32 = 3;
/// `FIRST_DRAWABLE_MODULE_TYPE`.
pub const FIRST_DRAWABLE_MODULE_TYPE: ModuleType = ModuleType::Draw;
/// `LAST_DRAWABLE_MODULE_TYPE`.
pub const LAST_DRAWABLE_MODULE_TYPE: ModuleType = ModuleType::ClientUpdate;
/// `NUM_DRAWABLE_MODULE_TYPES`.
pub const NUM_DRAWABLE_MODULE_TYPES: i32 = LAST_DRAWABLE_MODULE_TYPE as i32 - FIRST_DRAWABLE_MODULE_TYPE as i32 + 1;

impl ModuleType {
    /// The `ModuleType` of an integer, if it is one of the three.
    pub fn from_i32(v: i32) -> Option<ModuleType> {
        match v {
            0 => Some(ModuleType::Behavior),
            1 => Some(ModuleType::Draw),
            2 => Some(ModuleType::ClientUpdate),
            _ => None,
        }
    }
}

/// `enum ModuleInterfaceType`: bits of the mask a module class reports
/// (`getInterfaceMask()`), which the factory keeps per registration.
pub type ModuleInterfaceType = i32;
/// `MODULEINTERFACE_UPDATE`.
pub const MODULEINTERFACE_UPDATE: ModuleInterfaceType = 0x0000_0001;
/// `MODULEINTERFACE_DIE`.
pub const MODULEINTERFACE_DIE: ModuleInterfaceType = 0x0000_0002;
/// `MODULEINTERFACE_DAMAGE`.
pub const MODULEINTERFACE_DAMAGE: ModuleInterfaceType = 0x0000_0004;
/// `MODULEINTERFACE_CREATE`.
pub const MODULEINTERFACE_CREATE: ModuleInterfaceType = 0x0000_0008;
/// `MODULEINTERFACE_COLLIDE`.
pub const MODULEINTERFACE_COLLIDE: ModuleInterfaceType = 0x0000_0010;
/// `MODULEINTERFACE_BODY`.
pub const MODULEINTERFACE_BODY: ModuleInterfaceType = 0x0000_0020;
/// `MODULEINTERFACE_CONTAIN`.
pub const MODULEINTERFACE_CONTAIN: ModuleInterfaceType = 0x0000_0040;
/// `MODULEINTERFACE_UPGRADE`.
pub const MODULEINTERFACE_UPGRADE: ModuleInterfaceType = 0x0000_0080;
/// `MODULEINTERFACE_SPECIAL_POWER`.
pub const MODULEINTERFACE_SPECIAL_POWER: ModuleInterfaceType = 0x0000_0100;
/// `MODULEINTERFACE_DESTROY`.
pub const MODULEINTERFACE_DESTROY: ModuleInterfaceType = 0x0000_0200;
/// `MODULEINTERFACE_DRAW`.
pub const MODULEINTERFACE_DRAW: ModuleInterfaceType = 0x0000_0400;
/// `MODULEINTERFACE_CLIENT_UPDATE`.
pub const MODULEINTERFACE_CLIENT_UPDATE: ModuleInterfaceType = 0x0000_0800;

/// The `StaticGameLODLevel` value `ModuleData::getMinimumRequiredGameLOD`
/// returns: `(StaticGameLODLevel)0`, i.e. `STATIC_GAME_LOD_LOW`.
pub const STATIC_GAME_LOD_LOW: i32 = 0;

/// `ObjectID` of the object a module belongs to (`ObjectModule::m_object`;
/// see the module docs).
pub use super::game_type::ObjectId;

/// `class ModuleData`: "base class for data-read-from-INI for modules".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleData {
    /// `m_moduleTagNameKey`; `None` while unset (uninitialised upstream).
    module_tag_name_key: Option<NameKeyType>,
}

impl ModuleData {
    /// `ModuleData()`: the tag is not set.
    pub fn new() -> Self {
        ModuleData { module_tag_name_key: None }
    }

    /// `setModuleTagNameKey(key)`.
    pub fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = Some(key);
    }

    /// `getModuleTagNameKey()`: [`Undefined::UninitialisedModuleTag`] if it
    /// was never set.
    pub fn module_tag_name_key(&self) -> IniResult<NameKeyType> {
        self.module_tag_name_key.ok_or_else(|| IniError::undefined(Undefined::UninitialisedModuleTag))
    }

    /// `isAiModuleData()`: false (`AIUpdateModuleData` overrides it).
    pub fn is_ai_module_data(&self) -> bool {
        false
    }

    /// `getMinimumRequiredGameLOD()`: `(StaticGameLODLevel)0`.
    pub fn get_minimum_required_game_lod(&self) -> i32 {
        STATIC_GAME_LOD_LOW
    }

    /// `ModuleData::buildFieldParse(p)`: "nothing", adds no table.
    pub fn build_field_parse<T>(_p: &mut MultiIniFieldParse<'_, T>) -> IniResult<()> {
        Ok(())
    }

    /// `Module::friend_newModuleData(ini)`: a new base `ModuleData`; if there
    /// is an `ini`, `ini->initFromINI(data, 0)` ("this is just so that an
    /// `end` token is required"). See the module docs for the null table.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleData> {
        let mut data = ModuleData::new();
        if let Some(ini) = ini {
            let no_fields: &[FieldParse<ModuleData>] = &[];
            ini.init_from_ini(&mut data, no_fields).map_err(null_table_dereference)?;
        }
        Ok(data)
    }
}

/// With a null table the `findFieldParse` of any field line dereferences it:
/// the Rust's `UnknownToken` (an empty table finds nothing) is that case.
pub(crate) fn null_table_dereference(e: IniError) -> IniError {
    if e.kind == IniErrorKind::UnknownToken && e.exception.is_none() {
        IniError::undefined(Undefined::NullFieldParseTable)
    } else {
        e
    }
}

impl Snapshot for ModuleData {
    /// `ModuleData::crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// `ModuleData::xfer`: nothing (no version either).
    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// `ModuleData::loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}

/// Every `...ModuleData` class that is a variant so far, shared by the
/// modules made from one INI block (upstream: the `const ModuleData*` of
/// `Module::m_moduleData`). Concrete modules' data classes arrive in layer 9
/// as variants.
#[derive(Clone, Debug)]
pub enum ModuleDataKind {
    /// `ModuleData` itself.
    Module(Arc<ModuleData>),
    /// `BehaviorModuleData`.
    Behavior(Arc<BehaviorModuleData>),
    /// `BodyModuleData`.
    Body(Arc<BodyModuleData>),
    /// `CollideModuleData`.
    Collide(Arc<CollideModuleData>),
    /// `CreateModuleData`.
    Create(Arc<CreateModuleData>),
    /// `DamageModuleData`.
    Damage(Arc<DamageModuleData>),
    /// `DieModuleData`.
    Die(Arc<DieModuleData>),
    /// `SpecialPowerModuleData`.
    SpecialPower(Arc<SpecialPowerModuleData>),
    /// `UpdateModuleData`.
    Update(Arc<UpdateModuleData>),
    /// `UpgradeModuleData`.
    Upgrade(Arc<UpgradeModuleData>),
    // --- p1l9-example ---
    /// `CommandSetUpgradeModuleData`.
    CommandSetUpgrade(Arc<crate::logic::modules::command_set_upgrade::CommandSetUpgradeModuleData>),
    // --- end p1l9-example ---
// --- p1l9-upgrades ---
    /// `WeaponSetUpgradeModuleData`.
    WeaponSetUpgrade(Arc<crate::logic::modules::weapon_set_upgrade::WeaponSetUpgradeModuleData>),
    /// `WeaponBonusUpgradeModuleData`.
    WeaponBonusUpgrade(Arc<crate::logic::modules::weapon_bonus_upgrade::WeaponBonusUpgradeModuleData>),
    /// `ArmorUpgradeModuleData`.
    ArmorUpgrade(Arc<crate::logic::modules::armor_upgrade::ArmorUpgradeModuleData>),
    /// `ModelConditionUpgradeModuleData`.
    ModelConditionUpgrade(Arc<crate::logic::modules::model_condition_upgrade::ModelConditionUpgradeModuleData>),
    /// `ExperienceScalarUpgradeModuleData`.
    ExperienceScalarUpgrade(Arc<crate::logic::modules::experience_scalar_upgrade::ExperienceScalarUpgradeModuleData>),
    /// `ActiveShroudUpgradeModuleData`.
    ActiveShroudUpgrade(Arc<crate::logic::modules::active_shroud_upgrade::ActiveShroudUpgradeModuleData>),
    /// `MaxHealthUpgradeModuleData`.
    MaxHealthUpgrade(Arc<crate::logic::modules::max_health_upgrade::MaxHealthUpgradeModuleData>),
    /// `GrantScienceUpgradeModuleData`.
    GrantScienceUpgrade(Arc<crate::logic::modules::grant_science_upgrade::GrantScienceUpgradeModuleData>),
    // --- p1w10-sonnet ---
    /// `StatusBitsUpgradeModuleData`.
    StatusBitsUpgrade(Arc<crate::logic::modules::status_bits_upgrade::StatusBitsUpgradeModuleData>),
    /// `LocomotorSetUpgradeModuleData`.
    LocomotorSetUpgrade(Arc<crate::logic::modules::locomotor_set_upgrade::LocomotorSetUpgradeModuleData>),
    /// `PassengersFireUpgradeModuleData`.
    PassengersFireUpgrade(Arc<crate::logic::modules::passengers_fire_upgrade::PassengersFireUpgradeModuleData>),
    /// `StealthUpgradeModuleData`.
    StealthUpgrade(Arc<crate::logic::modules::stealth_upgrade::StealthUpgradeModuleData>),
    /// `SubObjectsUpgradeModuleData`.
    SubObjectsUpgrade(Arc<crate::logic::modules::sub_objects_upgrade::SubObjectsUpgradeModuleData>),
    /// `ObjectCreationUpgradeModuleData`.
    ObjectCreationUpgrade(Arc<crate::logic::modules::object_creation_upgrade::ObjectCreationUpgradeModuleData>),
    // --- end p1w10-sonnet ---
    // --- p1w11-sonnet ---
    /// `CostModifierUpgradeModuleData`.
    CostModifierUpgrade(Arc<crate::logic::modules::cost_modifier_upgrade::CostModifierUpgradeModuleData>),
    /// `PowerPlantUpgradeModuleData`.
    PowerPlantUpgrade(Arc<crate::logic::modules::power_plant_upgrade::PowerPlantUpgradeModuleData>),
    /// `RadarUpgradeModuleData`.
    RadarUpgrade(Arc<crate::logic::modules::radar_upgrade::RadarUpgradeModuleData>),
    /// `UnpauseSpecialPowerUpgradeModuleData`.
    UnpauseSpecialPowerUpgrade(Arc<crate::logic::modules::unpause_special_power_upgrade::UnpauseSpecialPowerUpgradeModuleData>),
    /// `ReplaceObjectUpgradeModuleData`.
    ReplaceObjectUpgrade(Arc<crate::logic::modules::replace_object_upgrade::ReplaceObjectUpgradeModuleData>),
    /// `DamDieModuleData`.
    DamDie(Arc<crate::logic::modules::dam_die::DamDieModuleData>),
    /// `SpecialPowerCompletionDieModuleData`.
    SpecialPowerCompletionDie(Arc<crate::logic::modules::special_power_completion_die::SpecialPowerCompletionDieModuleData>),
    /// `EjectPilotDieModuleData`.
    EjectPilotDie(Arc<crate::logic::modules::eject_pilot_die::EjectPilotDieModuleData>),
    /// `PowerPlantUpdateModuleData`.
    PowerPlantUpdate(Arc<crate::logic::modules::power_plant_update::PowerPlantUpdateModuleData>),
    /// `RadarUpdateModuleData`.
    RadarUpdate(Arc<crate::logic::modules::radar_update::RadarUpdateModuleData>),
    /// `HealCrateCollideModuleData`.
    HealCrateCollide(Arc<crate::logic::modules::heal_crate_collide::HealCrateCollideModuleData>),
    /// `ShroudCrateCollideModuleData`.
    ShroudCrateCollide(Arc<crate::logic::modules::shroud_crate_collide::ShroudCrateCollideModuleData>),
    /// `MoneyCrateCollideModuleData`.
    MoneyCrateCollide(Arc<crate::logic::modules::money_crate_collide::MoneyCrateCollideModuleData>),
    /// The eight sabotage crates' data (batch R13).
    SabotageCrateCollide(Arc<crate::logic::modules::sabotage_crate_collide::SabotageCrateCollideModuleData>),
    /// `UnitCrateCollide`'s data (batch R13b).
    UnitCrateCollide(Arc<crate::logic::modules::unit_crate_collide::UnitCrateCollideModuleData>),
    /// `VeterancyCrateCollide`'s data (batch R13b).
    VeterancyCrateCollide(Arc<crate::logic::modules::veterancy_crate_collide::VeterancyCrateCollideModuleData>),
    /// `CleanupAreaPowerModuleData`.
    CleanupAreaPower(Arc<crate::logic::modules::cleanup_area_power::CleanupAreaPowerModuleData>),
    /// `BaikonurLaunchPowerModuleData`.
    BaikonurLaunchPower(Arc<crate::logic::modules::baikonur_launch_power::BaikonurLaunchPowerModuleData>),
    /// `SpyVisionSpecialPowerModuleData`.
    SpyVisionSpecialPower(Arc<crate::logic::modules::spy_vision_special_power::SpyVisionSpecialPowerModuleData>),
    /// `DefectorSpecialPowerModuleData`.
    DefectorSpecialPower(Arc<crate::logic::modules::defector_special_power::DefectorSpecialPowerModuleData>),
    /// `SpyVisionUpdateModuleData`.
    SpyVisionUpdate(Arc<crate::logic::modules::spy_vision_update::SpyVisionUpdateModuleData>),
    /// `FXListDieModuleData`.
    FXListDie(Arc<crate::logic::modules::fx_list_die::FXListDieModuleData>),
    /// `FireWeaponWhenDeadBehaviorModuleData`.
    FireWeaponWhenDeadBehavior(Arc<crate::logic::modules::fire_weapon_when_dead_behavior::FireWeaponWhenDeadBehaviorModuleData>),
    /// `AutoDepositUpdateModuleData`.
    AutoDepositUpdate(Arc<crate::logic::modules::auto_deposit_update::AutoDepositUpdateModuleData>),
    // --- end p1w11-sonnet ---
    // --- p1w12-sonnet ---
    /// `FireOCLAfterWeaponCooldownUpdateModuleData`.
    FireOCLAfterWeaponCooldownUpdate(Arc<crate::logic::modules::fire_ocl_after_weapon_cooldown_update::FireOCLAfterWeaponCooldownUpdateModuleData>),
    /// `AutoHealBehaviorModuleData`.
    AutoHealBehavior(Arc<crate::logic::modules::auto_heal_behavior::AutoHealBehaviorModuleData>),
    /// `CheckpointUpdateModuleData`.
    CheckpointUpdate(Arc<crate::logic::modules::checkpoint_update::CheckpointUpdateModuleData>),
    /// `FlammableUpdateModuleData`.
    FlammableUpdate(Arc<crate::logic::modules::flammable_update::FlammableUpdateModuleData>),
    /// `OclUpdateModuleData`.
    OclUpdate(Arc<crate::logic::modules::ocl_update::OclUpdateModuleData>),
    // --- end p1w12-sonnet ---
    // --- p1w13-sonnet ---
    /// `HeightDieUpdateModuleData`.
    HeightDieUpdate(Arc<crate::logic::modules::height_die_update::HeightDieUpdateModuleData>),
    /// `DynamicShroudClearingRangeUpdateModuleData`.
    DynamicShroudClearingRangeUpdate(Arc<crate::logic::modules::dynamic_shroud_clearing_range_update::DynamicShroudClearingRangeUpdateModuleData>),
    /// `ProneUpdateModuleData`.
    ProneUpdate(Arc<crate::logic::modules::prone_update::ProneUpdateModuleData>),
    /// `EnemyNearUpdateModuleData`.
    EnemyNearUpdate(Arc<crate::logic::modules::enemy_near_update::EnemyNearUpdateModuleData>),
    /// `FloatUpdateModuleData`.
    FloatUpdate(Arc<crate::logic::modules::float_update::FloatUpdateModuleData>),
    /// `TechBuildingBehaviorModuleData`.
    TechBuildingBehavior(Arc<crate::logic::modules::tech_building_behavior::TechBuildingBehaviorModuleData>),
    /// `FireSpreadUpdateModuleData`.
    FireSpreadUpdate(Arc<crate::logic::modules::fire_spread_update::FireSpreadUpdateModuleData>),
    /// `ActiveBodyModuleData`.
    ActiveBody(Arc<crate::logic::modules::active_body::ActiveBodyModuleData>),
    /// `SlowDeathBehaviorModuleData`.
    SlowDeathBehavior(Arc<crate::logic::modules::slow_death_behavior::SlowDeathBehaviorModuleData>),
    /// `OpenContainModuleData`.
    OpenContain(Arc<crate::logic::modules::open_contain::OpenContainModuleData>),
    /// `TransportContainModuleData`.
    TransportContain(Arc<crate::logic::modules::transport_contain::TransportContainModuleData>),
    /// `GarrisonContain`'s data.
    GarrisonContain(Arc<crate::logic::modules::garrison_contain::GarrisonContainModuleData>),
    /// `CaveContain`'s data.
    CaveContain(Arc<crate::logic::modules::cave_contain::CaveContainModuleData>),
    /// `TunnelContain`'s data.
    TunnelContain(Arc<crate::logic::modules::tunnel_contain::TunnelContainModuleData>),
    /// `MobNexusContain`'s data.
    MobNexusContain(Arc<crate::logic::modules::mob_nexus_contain::MobNexusContainModuleData>),
    /// `ParachuteContain`'s data.
    ParachuteContain(Arc<crate::logic::modules::parachute_contain::ParachuteContainModuleData>),
    /// `HealContain`'s data.
    HealContain(Arc<crate::logic::modules::heal_contain::HealContainModuleData>),
    /// `OverlordContain`'s data.
    OverlordContain(Arc<crate::logic::modules::overlord_contain::OverlordContainModuleData>),
    /// `RiderChangeContain`'s data.
    RiderChangeContain(Arc<crate::logic::modules::rider_change_contain::RiderChangeContainModuleData>),
    /// `HelixContain`'s data.
    HelixContain(Arc<crate::logic::modules::helix_contain::HelixContainModuleData>),
    /// `RailedTransportContain`'s data (`TransportContainModuleData`).
    RailedTransportContain(Arc<crate::logic::modules::transport_contain::TransportContainModuleData>),
    /// `InternetHackContain`'s data (`TransportContainModuleData`).
    InternetHackContain(Arc<crate::logic::modules::transport_contain::TransportContainModuleData>),
    /// `UndeadBodyModuleData`.
    UndeadBody(Arc<crate::logic::modules::derived_bodies::UndeadBodyModuleData>),
    /// `HiveStructureBodyModuleData`.
    HiveStructureBody(Arc<crate::logic::modules::derived_bodies::HiveStructureBodyModuleData>),
    // --- end p1w13-sonnet ---
// --- end p1l9-upgrades ---
    // --- p1w10-sonnet ---
    /// `GrantUpgradeCreateModuleData`.
    GrantUpgradeCreate(Arc<crate::logic::modules::grant_upgrade_create::GrantUpgradeCreateModuleData>),
    /// `LockWeaponCreateModuleData`.
    LockWeaponCreate(Arc<crate::logic::modules::lock_weapon_create::LockWeaponCreateModuleData>),
    /// `VeterancyGainCreateModuleData`.
    VeterancyGainCreate(Arc<crate::logic::modules::veterancy_gain_create::VeterancyGainCreateModuleData>),
    /// `LifetimeUpdateModuleData`.
    LifetimeUpdate(Arc<crate::logic::modules::lifetime_update::LifetimeUpdateModuleData>),
    /// `DeletionUpdateModuleData`.
    DeletionUpdate(Arc<crate::logic::modules::deletion_update::DeletionUpdateModuleData>),
    /// `BaseRegenerateUpdateModuleData`.
    BaseRegenerateUpdate(Arc<crate::logic::modules::base_regenerate_update::BaseRegenerateUpdateModuleData>),
    /// `SmartBombTargetHomingUpdateModuleData`.
    SmartBombTargetHomingUpdate(Arc<crate::logic::modules::smart_bomb_target_homing_update::SmartBombTargetHomingUpdateModuleData>),
    /// `UpgradeDieModuleData`.
    UpgradeDie(Arc<crate::logic::modules::upgrade_die::UpgradeDieModuleData>),
    // --- end p1w10-sonnet ---
}

impl ModuleDataKind {
    /// The `ModuleData` base of whichever class this is.
    pub fn module_data(&self) -> &ModuleData {
        match self {
            ModuleDataKind::Module(d) => d,
            ModuleDataKind::Behavior(d) => &d.module_data,
            ModuleDataKind::Body(d) => &d.behavior.module_data,
            ModuleDataKind::Collide(d) => &d.behavior.module_data,
            ModuleDataKind::Create(d) => &d.behavior.module_data,
            ModuleDataKind::Damage(d) => &d.behavior.module_data,
            ModuleDataKind::Die(d) => &d.behavior.module_data,
            ModuleDataKind::SpecialPower(d) => &d.behavior.module_data,
            ModuleDataKind::Update(d) => &d.behavior.module_data,
            ModuleDataKind::Upgrade(d) => &d.behavior.module_data,
            // --- p1l9-example ---
            ModuleDataKind::CommandSetUpgrade(d) => &d.upgrade.behavior.module_data,
            // --- end p1l9-example ---
// --- p1l9-upgrades ---
            ModuleDataKind::WeaponSetUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::WeaponBonusUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::ArmorUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::ModelConditionUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::ExperienceScalarUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::ActiveShroudUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::MaxHealthUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::GrantScienceUpgrade(d) => &d.upgrade.behavior.module_data,
            // --- p1w10-sonnet ---
            ModuleDataKind::StatusBitsUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::LocomotorSetUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::PassengersFireUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::StealthUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::SubObjectsUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::ObjectCreationUpgrade(d) => &d.upgrade.behavior.module_data,
            // --- end p1w10-sonnet ---
            // --- p1w11-sonnet ---
            ModuleDataKind::CostModifierUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::PowerPlantUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::RadarUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::UnpauseSpecialPowerUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::ReplaceObjectUpgrade(d) => &d.upgrade.behavior.module_data,
            ModuleDataKind::DamDie(d) => &d.die.behavior.module_data,
            ModuleDataKind::SpecialPowerCompletionDie(d) => &d.die.behavior.module_data,
            ModuleDataKind::EjectPilotDie(d) => &d.die.behavior.module_data,
            ModuleDataKind::PowerPlantUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::RadarUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::HealCrateCollide(d) => &d.crate_data.collide.behavior.module_data,
            ModuleDataKind::ShroudCrateCollide(d) => &d.crate_data.collide.behavior.module_data,
            ModuleDataKind::MoneyCrateCollide(d) => &d.crate_data.collide.behavior.module_data,
            ModuleDataKind::SabotageCrateCollide(d) => &d.crate_data.collide.behavior.module_data,
            ModuleDataKind::UnitCrateCollide(d) => &d.crate_data.collide.behavior.module_data,
            ModuleDataKind::VeterancyCrateCollide(d) => &d.crate_data.collide.behavior.module_data,
            ModuleDataKind::CleanupAreaPower(d) => &d.special_power.behavior.module_data,
            ModuleDataKind::BaikonurLaunchPower(d) => &d.special_power.behavior.module_data,
            ModuleDataKind::SpyVisionSpecialPower(d) => &d.special_power.behavior.module_data,
            ModuleDataKind::DefectorSpecialPower(d) => &d.special_power.behavior.module_data,
            ModuleDataKind::SpyVisionUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::FXListDie(d) => &d.die.behavior.module_data,
            ModuleDataKind::FireWeaponWhenDeadBehavior(d) => &d.die.behavior.module_data,
            ModuleDataKind::AutoDepositUpdate(d) => &d.update.behavior.module_data,
            // --- end p1w11-sonnet ---
            // --- p1w12-sonnet ---
            ModuleDataKind::FireOCLAfterWeaponCooldownUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::AutoHealBehavior(d) => &d.update.behavior.module_data,
            ModuleDataKind::CheckpointUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::FlammableUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::OclUpdate(d) => &d.update.behavior.module_data,
            // --- end p1w12-sonnet ---
            // --- p1w13-sonnet ---
            ModuleDataKind::HeightDieUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::DynamicShroudClearingRangeUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::ProneUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::EnemyNearUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::FloatUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::TechBuildingBehavior(d) => &d.update.behavior.module_data,
            ModuleDataKind::FireSpreadUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::ActiveBody(d) => &d.body.behavior.module_data,
            ModuleDataKind::SlowDeathBehavior(d) => &d.update.behavior.module_data,
            ModuleDataKind::OpenContain(d) => &d.update.behavior.module_data,
            ModuleDataKind::TransportContain(d) => &d.open.update.behavior.module_data,
            ModuleDataKind::GarrisonContain(d) => &d.open.update.behavior.module_data,
            ModuleDataKind::CaveContain(d) => &d.open.update.behavior.module_data,
            ModuleDataKind::TunnelContain(d) => &d.open.update.behavior.module_data,
            ModuleDataKind::MobNexusContain(d) => &d.open.update.behavior.module_data,
            ModuleDataKind::ParachuteContain(d) => &d.open.update.behavior.module_data,
            ModuleDataKind::HealContain(d) => &d.open.update.behavior.module_data,
            ModuleDataKind::OverlordContain(d) => &d.transport.open.update.behavior.module_data,
            ModuleDataKind::RiderChangeContain(d) => &d.transport.open.update.behavior.module_data,
            ModuleDataKind::HelixContain(d) => &d.transport.open.update.behavior.module_data,
            ModuleDataKind::RailedTransportContain(d) => &d.open.update.behavior.module_data,
            ModuleDataKind::InternetHackContain(d) => &d.open.update.behavior.module_data,
            ModuleDataKind::UndeadBody(d) => &d.active.body.behavior.module_data,
            ModuleDataKind::HiveStructureBody(d) => &d.active.body.behavior.module_data,
            // --- end p1w13-sonnet ---
// --- end p1l9-upgrades ---
            // --- p1w10-sonnet ---
            ModuleDataKind::GrantUpgradeCreate(d) => &d.create.behavior.module_data,
            ModuleDataKind::LockWeaponCreate(d) => &d.create.behavior.module_data,
            ModuleDataKind::VeterancyGainCreate(d) => &d.create.behavior.module_data,
            ModuleDataKind::LifetimeUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::DeletionUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::BaseRegenerateUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::SmartBombTargetHomingUpdate(d) => &d.update.behavior.module_data,
            ModuleDataKind::UpgradeDie(d) => &d.die.behavior.module_data,
            // --- end p1w10-sonnet ---
        }
    }

    /// `setModuleTagNameKey(key)`, for the factory, which sets it right after
    /// the data is read. `false` if the data is already shared (it never is
    /// at that point).
    pub fn set_module_tag_name_key(&mut self, key: NameKeyType) -> bool {
        fn set<T>(a: &mut Arc<T>, key: NameKeyType, get: fn(&mut T) -> &mut ModuleData) -> bool {
            match Arc::get_mut(a) {
                Some(d) => {
                    get(d).set_module_tag_name_key(key);
                    true
                }
                None => false,
            }
        }
        match self {
            ModuleDataKind::Module(d) => set(d, key, |d| d),
            ModuleDataKind::Behavior(d) => set(d, key, |d| &mut d.module_data),
            ModuleDataKind::Body(d) => set(d, key, |d| &mut d.behavior.module_data),
            ModuleDataKind::Collide(d) => set(d, key, |d| &mut d.behavior.module_data),
            ModuleDataKind::Create(d) => set(d, key, |d| &mut d.behavior.module_data),
            ModuleDataKind::Damage(d) => set(d, key, |d| &mut d.behavior.module_data),
            ModuleDataKind::Die(d) => set(d, key, |d| &mut d.behavior.module_data),
            ModuleDataKind::SpecialPower(d) => set(d, key, |d| &mut d.behavior.module_data),
            ModuleDataKind::Update(d) => set(d, key, |d| &mut d.behavior.module_data),
            ModuleDataKind::Upgrade(d) => set(d, key, |d| &mut d.behavior.module_data),
            // --- p1l9-example ---
            ModuleDataKind::CommandSetUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            // --- end p1l9-example ---
// --- p1l9-upgrades ---
            ModuleDataKind::WeaponSetUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::WeaponBonusUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::ArmorUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::ModelConditionUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::ExperienceScalarUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::ActiveShroudUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::MaxHealthUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::GrantScienceUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            // --- p1w10-sonnet ---
            ModuleDataKind::StatusBitsUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::LocomotorSetUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::PassengersFireUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::StealthUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::SubObjectsUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::ObjectCreationUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            // --- end p1w10-sonnet ---
            // --- p1w11-sonnet ---
            ModuleDataKind::CostModifierUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::PowerPlantUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::RadarUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::UnpauseSpecialPowerUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::ReplaceObjectUpgrade(d) => match Arc::get_mut(d) {
                Some(d) => set(&mut d.upgrade, key, |d| &mut d.behavior.module_data),
                None => false,
            },
            ModuleDataKind::DamDie(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.die.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::SpecialPowerCompletionDie(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.die.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::EjectPilotDie(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.die.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::PowerPlantUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::RadarUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::HealCrateCollide(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.crate_data.collide.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::ShroudCrateCollide(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.crate_data.collide.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::SabotageCrateCollide(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.crate_data.collide.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::UnitCrateCollide(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.crate_data.collide.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::VeterancyCrateCollide(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.crate_data.collide.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::MoneyCrateCollide(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.crate_data.collide.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::CleanupAreaPower(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.special_power.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::BaikonurLaunchPower(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.special_power.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::SpyVisionSpecialPower(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.special_power.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::DefectorSpecialPower(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.special_power.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::SpyVisionUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::FXListDie(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.die.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::FireWeaponWhenDeadBehavior(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.die.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::AutoDepositUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            // --- end p1w11-sonnet ---
            // --- p1w12-sonnet ---
            ModuleDataKind::FireOCLAfterWeaponCooldownUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::AutoHealBehavior(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::CheckpointUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::FlammableUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::OclUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            // --- end p1w12-sonnet ---
            // --- p1w13-sonnet ---
            ModuleDataKind::HeightDieUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::DynamicShroudClearingRangeUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::ProneUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::EnemyNearUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::FloatUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::TechBuildingBehavior(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::FireSpreadUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::ActiveBody(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.body.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::SlowDeathBehavior(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
                        ModuleDataKind::OpenContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::InternetHackContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::RailedTransportContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::HelixContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.transport.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::RiderChangeContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.transport.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::OverlordContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.transport.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::HealContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::ParachuteContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::MobNexusContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::TunnelContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::CaveContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::GarrisonContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::TransportContain(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.open.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::UndeadBody(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.active.body.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::HiveStructureBody(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.active.body.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            // --- end p1w13-sonnet ---
// --- end p1l9-upgrades ---
            // --- p1w10-sonnet ---
            ModuleDataKind::GrantUpgradeCreate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.create.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::LockWeaponCreate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.create.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::VeterancyGainCreate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.create.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::LifetimeUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::DeletionUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::BaseRegenerateUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::SmartBombTargetHomingUpdate(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.update.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            ModuleDataKind::UpgradeDie(d) => match Arc::get_mut(d) {
                Some(d) => {
                    d.die.behavior.module_data.set_module_tag_name_key(key);
                    true
                }
                None => false,
            },
            // --- end p1w10-sonnet ---
        }
    }

    /// `getModuleTagNameKey()`.
    pub fn module_tag_name_key(&self) -> IniResult<NameKeyType> {
        self.module_data().module_tag_name_key()
    }

    /// `isAiModuleData()`: false for every class so far.
    pub fn is_ai_module_data(&self) -> bool {
        self.module_data().is_ai_module_data()
    }

    /// `getMinimumRequiredGameLOD()`: `STATIC_GAME_LOD_LOW` for every class so
    /// far.
    pub fn get_minimum_required_game_lod(&self) -> i32 {
        self.module_data().get_minimum_required_game_lod()
    }

    /// `ModuleData::crc` of the class: nothing in any class so far (the
    /// concrete data classes of layer 9 override it).
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `ModuleData::xfer` of the class: nothing in any class so far.
    pub fn xfer(&self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `ModuleData::loadPostProcess` of the class: nothing in any class so
    /// far.
    pub fn load_post_process(&self) {}
}

/// The members of upstream `Module` itself: the module data pointer.
#[derive(Clone, Debug)]
pub struct ModuleBase {
    /// `m_moduleData`.
    module_data: ModuleDataKind,
}

impl ModuleBase {
    /// `Module(moduleData)`.
    pub fn new(module_data: ModuleDataKind) -> Self {
        ModuleBase { module_data }
    }

    /// `getModuleData()`.
    pub fn module_data(&self) -> &ModuleDataKind {
        &self.module_data
    }

    /// `getModuleTagNameKey()`: the data's tag.
    pub fn module_tag_name_key(&self) -> IniResult<NameKeyType> {
        self.module_data.module_tag_name_key()
    }
}

impl Snapshot for ModuleBase {
    /// `Module::crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// `Module::xfer` (version 1): the version only.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)
    }
    /// `Module::loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}

/// `class ObjectModule`: "module interface specific for Objects".
#[derive(Clone, Debug)]
pub struct ObjectModule {
    /// The `Module` base.
    module: ModuleBase,
    /// `m_object`: the object this module is a part of (see the module docs).
    object: ObjectId,
}

impl ObjectModule {
    /// `ObjectModule(thing, moduleData)`: `throw INI_INVALID_DATA` when
    /// `moduleData` is null ("module data may not be null").
    pub fn new(object: ObjectId, module_data: Option<ModuleDataKind>) -> IniResult<Self> {
        match module_data {
            None => Err(IniError::new(IniErrorKind::InvalidData)),
            Some(d) => Ok(ObjectModule { module: ModuleBase::new(d), object }),
        }
    }

    /// The `Module` base.
    pub fn module(&self) -> &ModuleBase {
        &self.module
    }

    /// `getObject()`, as the object's id.
    pub fn object(&self) -> ObjectId {
        self.object
    }
}

impl Snapshot for ObjectModule {
    /// `ObjectModule::crc`: `Module::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.module.crc(xfer)
    }
    /// `ObjectModule::xfer` (version 1): the version, then `Module::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.module.xfer(xfer)
    }
    /// `ObjectModule::loadPostProcess`: `Module::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.module.load_post_process();
    }
}

/// `class DrawableModule`: "module interface specific for Drawables".
#[derive(Clone, Debug)]
pub struct DrawableModule {
    /// The `Module` base.
    module: ModuleBase,
}

impl DrawableModule {
    /// `DrawableModule(thing, moduleData)`: `throw INI_INVALID_DATA` when
    /// `moduleData` is null.
    pub fn new(module_data: Option<ModuleDataKind>) -> IniResult<Self> {
        match module_data {
            None => Err(IniError::new(IniErrorKind::InvalidData)),
            Some(d) => Ok(DrawableModule { module: ModuleBase::new(d) }),
        }
    }

    /// The `Module` base.
    pub fn module(&self) -> &ModuleBase {
        &self.module
    }
}

impl Snapshot for DrawableModule {
    /// `DrawableModule::crc`: `Module::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.module.crc(xfer)
    }
    /// `DrawableModule::xfer` (version 1): the version, then `Module::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.module.xfer(xfer)
    }
    /// `DrawableModule::loadPostProcess`: `Module::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.module.load_post_process();
    }
}

/// A module: one variant per module class (see the module docs). The
/// polymorphic handle that upstream calls `Module*`.
#[derive(Clone, Debug)]
pub enum Module {
    /// `ObjectModule` (abstract upstream).
    Object(ObjectModule),
    /// `DrawableModule` (abstract upstream).
    Drawable(DrawableModule),
    /// `BehaviorModule` (abstract upstream).
    Behavior(BehaviorModule),
    /// `BodyModule` (abstract upstream).
    Body(BodyModule),
    /// `CollideModule` (abstract upstream).
    Collide(CollideModule),
    /// `CreateModule` (abstract upstream).
    Create(CreateModule),
    /// `DamageModule` (abstract upstream).
    Damage(DamageModule),
    /// `DestroyModule` (abstract upstream).
    Destroy(DestroyModule),
    /// `DieModule` (abstract upstream).
    Die(DieModule),
    /// `SpecialPowerModule` (concrete upstream, but never registered:
    /// `OCLSpecialPower` and the other powers derive from it).
    SpecialPower(SpecialPowerModule),
    /// `SpecialPowerUpdateModule` (abstract upstream).
    SpecialPowerUpdate(SpecialPowerUpdateModule),
    /// `UpdateModule` (abstract upstream).
    Update(UpdateModule),
    /// `UpgradeModule` (abstract upstream).
    Upgrade(UpgradeModule),
}

impl Module {
    /// The `Module` base of whichever class this is.
    pub fn module_base(&self) -> &ModuleBase {
        match self {
            Module::Object(m) => m.module(),
            Module::Drawable(m) => m.module(),
            Module::Behavior(m) => m.module_base(),
            Module::Body(m) => m.behavior().module_base(),
            Module::Collide(m) => m.behavior().module_base(),
            Module::Create(m) => m.behavior().module_base(),
            Module::Damage(m) => m.behavior().module_base(),
            Module::Destroy(m) => m.behavior().module_base(),
            Module::Die(m) => m.behavior().module_base(),
            Module::SpecialPower(m) => m.behavior().module_base(),
            Module::SpecialPowerUpdate(m) => m.update().behavior().module_base(),
            Module::Update(m) => m.behavior().module_base(),
            Module::Upgrade(m) => m.behavior().module_base(),
        }
    }

    /// `getModuleData()`.
    pub fn module_data(&self) -> &ModuleDataKind {
        self.module_base().module_data()
    }

    /// `getModuleTagNameKey()`.
    pub fn module_tag_name_key(&self) -> IniResult<NameKeyType> {
        self.module_base().module_tag_name_key()
    }

    /// `getModuleType()` (a static of the class): `MODULETYPE_BEHAVIOR` for
    /// every `BehaviorModule`; a `DrawableModule` has none of its own (the
    /// `DrawModule` that sets `MODULETYPE_DRAW` is client code), and
    /// `ObjectModule` has none.
    pub fn module_type(&self) -> Option<ModuleType> {
        match self {
            Module::Object(_) | Module::Drawable(_) => None,
            _ => Some(ModuleType::Behavior),
        }
    }

    /// `getInterfaceMask()` (a static of the class): `BehaviorModule`: 0,
    /// `BodyModule`: `MODULEINTERFACE_BODY`, and so on. `ObjectModule` and
    /// `DrawableModule` have none (0).
    pub fn interface_mask(&self) -> ModuleInterfaceType {
        match self {
            Module::Object(_) | Module::Drawable(_) | Module::Behavior(_) => 0,
            Module::Body(_) => MODULEINTERFACE_BODY,
            Module::Collide(_) => MODULEINTERFACE_COLLIDE,
            Module::Create(_) => MODULEINTERFACE_CREATE,
            Module::Damage(_) => MODULEINTERFACE_DAMAGE,
            Module::Destroy(_) => MODULEINTERFACE_DESTROY,
            Module::Die(_) => MODULEINTERFACE_DIE,
            Module::SpecialPower(_) => MODULEINTERFACE_SPECIAL_POWER,
            // SpecialPowerUpdateModule inherits UpdateModule's getInterfaceMask()
            Module::SpecialPowerUpdate(_) | Module::Update(_) => MODULEINTERFACE_UPDATE,
            Module::Upgrade(_) => MODULEINTERFACE_UPGRADE,
        }
    }

    /// `getBody()`: this module as a `BodyModuleInterface`, or `NULL`.
    pub fn get_body(&self) -> Option<&BodyModule> {
        match self {
            Module::Body(m) => Some(m),
            _ => None,
        }
    }

    /// `getCollide()`.
    pub fn get_collide(&self) -> Option<&CollideModule> {
        match self {
            Module::Collide(m) => Some(m),
            _ => None,
        }
    }

    /// `getContain()`: no variant implements `ContainModuleInterface` yet.
    pub fn get_contain(&self) -> Option<&Module> {
        None
    }

    /// `getCreate()`.
    pub fn get_create(&self) -> Option<&CreateModule> {
        match self {
            Module::Create(m) => Some(m),
            _ => None,
        }
    }

    /// `getDamage()`.
    pub fn get_damage(&self) -> Option<&DamageModule> {
        match self {
            Module::Damage(m) => Some(m),
            _ => None,
        }
    }

    /// `getDestroy()`.
    pub fn get_destroy(&self) -> Option<&DestroyModule> {
        match self {
            Module::Destroy(m) => Some(m),
            _ => None,
        }
    }

    /// `getDie()`.
    pub fn get_die(&self) -> Option<&DieModule> {
        match self {
            Module::Die(m) => Some(m),
            _ => None,
        }
    }

    /// `getSpecialPower()`.
    pub fn get_special_power(&self) -> Option<&SpecialPowerModule> {
        match self {
            Module::SpecialPower(m) => Some(m),
            _ => None,
        }
    }

    /// `getUpdate()`: an `UpdateModule`, and a `SpecialPowerUpdateModule`
    /// (which derives from it).
    pub fn get_update(&self) -> Option<&UpdateModule> {
        match self {
            Module::Update(m) => Some(m),
            Module::SpecialPowerUpdate(m) => Some(m.update()),
            _ => None,
        }
    }

    /// `getUpgrade()`.
    pub fn get_upgrade(&self) -> Option<&UpgradeModule> {
        match self {
            Module::Upgrade(m) => Some(m),
            _ => None,
        }
    }

    /// Which of the ten `BehaviorModule::get...()` interfaces this module
    /// returns non-null, as the mask the factory keeps for it (the
    /// `_DEBUG` check of `ModuleFactory::newModule` compares exactly this
    /// with the registered `getInterfaceMask()`; `MODULEINTERFACE_CONTAIN` is
    /// never set so far).
    pub fn interfaces_present(&self) -> ModuleInterfaceType {
        let mut mask = 0;
        let bits = [
            (self.get_body().is_some(), MODULEINTERFACE_BODY),
            (self.get_collide().is_some(), MODULEINTERFACE_COLLIDE),
            (self.get_contain().is_some(), MODULEINTERFACE_CONTAIN),
            (self.get_create().is_some(), MODULEINTERFACE_CREATE),
            (self.get_damage().is_some(), MODULEINTERFACE_DAMAGE),
            (self.get_destroy().is_some(), MODULEINTERFACE_DESTROY),
            (self.get_die().is_some(), MODULEINTERFACE_DIE),
            (self.get_special_power().is_some(), MODULEINTERFACE_SPECIAL_POWER),
            (self.get_update().is_some(), MODULEINTERFACE_UPDATE),
            (self.get_upgrade().is_some(), MODULEINTERFACE_UPGRADE),
        ];
        for (present, bit) in bits {
            if present {
                mask |= bit;
            }
        }
        mask
    }
}

impl Snapshot for Module {
    /// The `crc` of the class.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            Module::Object(m) => m.crc(xfer),
            Module::Drawable(m) => m.crc(xfer),
            Module::Behavior(m) => m.crc(xfer),
            Module::Body(m) => m.crc(xfer),
            Module::Collide(m) => m.crc(xfer),
            Module::Create(m) => m.crc(xfer),
            Module::Damage(m) => m.crc(xfer),
            Module::Destroy(m) => m.crc(xfer),
            Module::Die(m) => m.crc(xfer),
            Module::SpecialPower(m) => m.crc(xfer),
            Module::SpecialPowerUpdate(m) => m.crc(xfer),
            Module::Update(m) => m.crc(xfer),
            Module::Upgrade(m) => m.crc(xfer),
        }
    }
    /// The `xfer` of the class.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            Module::Object(m) => m.xfer(xfer),
            Module::Drawable(m) => m.xfer(xfer),
            Module::Behavior(m) => m.xfer(xfer),
            Module::Body(m) => m.xfer(xfer),
            Module::Collide(m) => m.xfer(xfer),
            Module::Create(m) => m.xfer(xfer),
            Module::Damage(m) => m.xfer(xfer),
            Module::Destroy(m) => m.xfer(xfer),
            Module::Die(m) => m.xfer(xfer),
            Module::SpecialPower(m) => m.xfer(xfer),
            Module::SpecialPowerUpdate(m) => m.xfer(xfer),
            Module::Update(m) => m.xfer(xfer),
            Module::Upgrade(m) => m.xfer(xfer),
        }
    }
    /// The `loadPostProcess` of the class.
    fn load_post_process(&mut self) {
        match self {
            Module::Object(m) => m.load_post_process(),
            Module::Drawable(m) => m.load_post_process(),
            Module::Behavior(m) => m.load_post_process(),
            Module::Body(m) => m.load_post_process(),
            Module::Collide(m) => m.load_post_process(),
            Module::Create(m) => m.load_post_process(),
            Module::Damage(m) => m.load_post_process(),
            Module::Destroy(m) => m.load_post_process(),
            Module::Die(m) => m.load_post_process(),
            Module::SpecialPower(m) => m.load_post_process(),
            Module::SpecialPowerUpdate(m) => m.load_post_process(),
            Module::Update(m) => m.load_post_process(),
            Module::Upgrade(m) => m.load_post_process(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::logic::module::test_support::{load_bytes, save_bytes};
    use crate::logic::module::behavior_module::BehaviorModuleData;
    use crate::logic::module::collide_module::CollideModuleData;
    use crate::logic::module::create_module::CreateModuleData;
    use crate::logic::module::damage_module::DamageModuleData;
    use crate::logic::module::die_module::DieModuleData;
    use crate::logic::module::special_power_module::SpecialPowerModuleData;
    use crate::logic::module::update_module::SleepyUpdatePhase;
    use crate::logic::module::upgrade_module::UpgradeModuleData;

    fn data_block(ini: &mut Ini<'_>, out: &mut Vec<IniResult<ModuleData>>) -> IniResult<()> {
        out.push(ModuleData::friend_new_module_data(Some(ini)));
        // (the block's own result is recorded; a failed block stops the load)
        match out.last() {
            Some(Err(e)) => Err(e.clone()),
            _ => Ok(()),
        }
    }
    static BLOCKS: [BlockParse<Vec<IniResult<ModuleData>>>; 1] = [BlockParse { token: "ModData", parse: data_block }];

    fn load(text: &[u8]) -> (Vec<IniResult<ModuleData>>, IniResult<()>) {
        let mut out = Vec::new();
        let r = Ini::load(b"m.ini", text, IniLoadType::Overwrite, None, &BLOCKS, &mut out);
        (out, r)
    }

    fn data() -> ModuleDataKind {
        ModuleDataKind::Behavior(Arc::new(BehaviorModuleData::new()))
    }

    #[test]
    fn constants_are_upstreams() {
        assert_eq!((ModuleType::Behavior as i32, ModuleType::Draw as i32, ModuleType::ClientUpdate as i32), (0, 1, 2));
        assert_eq!((NUM_MODULE_TYPES, NUM_DRAWABLE_MODULE_TYPES), (3, 2));
        assert_eq!(FIRST_DRAWABLE_MODULE_TYPE, ModuleType::Draw);
        assert_eq!(LAST_DRAWABLE_MODULE_TYPE, ModuleType::ClientUpdate);
        let bits = [
            MODULEINTERFACE_UPDATE,
            MODULEINTERFACE_DIE,
            MODULEINTERFACE_DAMAGE,
            MODULEINTERFACE_CREATE,
            MODULEINTERFACE_COLLIDE,
            MODULEINTERFACE_BODY,
            MODULEINTERFACE_CONTAIN,
            MODULEINTERFACE_UPGRADE,
            MODULEINTERFACE_SPECIAL_POWER,
            MODULEINTERFACE_DESTROY,
            MODULEINTERFACE_DRAW,
            MODULEINTERFACE_CLIENT_UPDATE,
        ];
        for (i, b) in bits.iter().enumerate() {
            assert_eq!(*b, 1 << i, "bit {i}");
        }
        assert_eq!(ModuleType::from_i32(2), Some(ModuleType::ClientUpdate));
        assert_eq!(ModuleType::from_i32(3), None);
        assert_eq!(ModuleType::from_i32(-1), None);
    }

    #[test]
    fn module_tag_is_unset_until_set() {
        let mut d = ModuleData::new();
        let e = d.module_tag_name_key().unwrap_err();
        assert_eq!(e.kind, IniErrorKind::Undefined(Undefined::UninitialisedModuleTag));
        d.set_module_tag_name_key(NameKeyType(77));
        assert_eq!(d.module_tag_name_key().unwrap(), NameKeyType(77));
        assert!(!d.is_ai_module_data());
        assert_eq!(d.get_minimum_required_game_lod(), 0);
    }

    #[test]
    fn base_module_data_block_accepts_only_end() {
        // End (any case) alone is fine; with no ini there is nothing to read
        let (out, r) = load(b"ModData\n  End\nModData\n  eNd\n");
        assert!(r.is_ok() && out.len() == 2 && out.iter().all(|o| o.is_ok()));
        assert!(ModuleData::friend_new_module_data(None).is_ok());
        // a field line meets the null parse table
        let (out, r) = load(b"ModData\n  Foo = 1\nEnd\n");
        let e = out[0].clone().unwrap_err();
        assert_eq!(e.kind, IniErrorKind::Undefined(Undefined::NullFieldParseTable));
        assert!(r.unwrap_err().is_undefined());
        // an unterminated block
        let (out, _) = load(b"ModData\n");
        assert_eq!(out[0].clone().unwrap_err().kind, IniErrorKind::MissingEndToken);
    }

    #[test]
    fn null_module_data_is_invalid_data() {
        let invalid = |e: IniError| e.kind == IniErrorKind::InvalidData;
        assert!(invalid(ObjectModule::new(1, None).unwrap_err()));
        assert!(invalid(DrawableModule::new(None).unwrap_err()));
        assert!(invalid(BehaviorModule::new(1, None).unwrap_err()));
        assert!(ObjectModule::new(1, Some(data())).is_ok());
    }

    #[test]
    fn data_kind_reaches_its_base_and_sets_the_tag_once() {
        let kinds = [
            ModuleDataKind::Module(Arc::new(ModuleData::new())),
            ModuleDataKind::Behavior(Arc::new(BehaviorModuleData::new())),
            ModuleDataKind::Body(Arc::new(Default::default())),
            ModuleDataKind::Collide(Arc::new(CollideModuleData::new())),
            ModuleDataKind::Create(Arc::new(CreateModuleData::new())),
            ModuleDataKind::Damage(Arc::new(DamageModuleData::new())),
            ModuleDataKind::Die(Arc::new(DieModuleData::new())),
            ModuleDataKind::SpecialPower(Arc::new(SpecialPowerModuleData::new())),
            ModuleDataKind::Update(Arc::new(Default::default())),
            ModuleDataKind::Upgrade(Arc::new(UpgradeModuleData::new())),
        ];
        for (i, mut k) in kinds.into_iter().enumerate() {
            assert!(k.module_tag_name_key().is_err(), "kind {i}");
            assert!(k.set_module_tag_name_key(NameKeyType(100 + i as u32)));
            assert_eq!(k.module_tag_name_key().unwrap(), NameKeyType(100 + i as u32));
            // a shared one cannot be tagged
            let shared = k.clone();
            assert!(!k.set_module_tag_name_key(NameKeyType(1)));
            assert_eq!(shared.module_tag_name_key().unwrap(), NameKeyType(100 + i as u32));
            assert!(!k.is_ai_module_data());
            assert_eq!(k.get_minimum_required_game_lod(), STATIC_GAME_LOD_LOW);
        }
    }

    /// Every variant holds its base chain: the xfer writes one version byte per
    /// class on the way down (UpdateModule: its own, BehaviorModule's,
    /// ObjectModule's, Module's), then the class's own members.
    #[test]
    fn xfer_chains_write_one_version_per_base() {
        use crate::logic::module::{
            body_module::BodyModule, collide_module::CollideModule, create_module::CreateModule, damage_module::DamageModule,
            destroy_module::DestroyModule, die_module::DieModule, special_power_module::SpecialPowerModule,
            special_power_update_module::SpecialPowerUpdateModule, update_module::UpdateModule, upgrade_module::UpgradeModule,
        };
        let bytes = |m: &mut Module| save_bytes(m).unwrap();
        let mut cases: Vec<(&str, Module, Vec<u8>)> = vec![
            ("Object", Module::Object(ObjectModule::new(5, Some(data())).unwrap()), vec![1, 1]),
            ("Drawable", Module::Drawable(DrawableModule::new(Some(data())).unwrap()), vec![1, 1]),
            ("Behavior", Module::Behavior(BehaviorModule::new(5, Some(data())).unwrap()), vec![1, 1, 1]),
            ("Body", Module::Body(BodyModule::new(5, Some(Arc::new(Default::default()))).unwrap()), vec![1, 1, 1, 1, 0, 0, 0x80, 0x3f]),
            ("Collide", Module::Collide(CollideModule::new(5, Some(Arc::new(Default::default()))).unwrap()), vec![1, 1, 1, 1]),
            ("Create", Module::Create(CreateModule::new(5, Some(Arc::new(Default::default()))).unwrap()), vec![1, 1, 1, 1, 1]),
            ("Damage", Module::Damage(DamageModule::new(5, Some(Arc::new(Default::default()))).unwrap()), vec![1, 1, 1, 1]),
            ("Destroy", Module::Destroy(DestroyModule::new(5, Some(data())).unwrap()), vec![1, 1, 1, 1]),
            ("Die", Module::Die(DieModule::new(5, Some(Arc::new(Default::default()))).unwrap()), vec![1, 1, 1, 1]),
            (
                "SpecialPower",
                Module::SpecialPower(SpecialPowerModule::new(5, Some(Arc::new(Default::default()))).unwrap()),
                vec![1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ),
            (
                "SpecialPowerUpdate",
                Module::SpecialPowerUpdate(SpecialPowerUpdateModule::new(5, Some(Arc::new(Default::default())), SleepyUpdatePhase::Normal).unwrap()),
                vec![1, 1, 1, 1, 1, 0, 0, 0, 0],
            ),
            ("Update", Module::Update(UpdateModule::new(5, Some(Arc::new(Default::default())), SleepyUpdatePhase::Normal).unwrap()), vec![1, 1, 1, 1, 0, 0, 0, 0]),
            ("Upgrade", Module::Upgrade(UpgradeModule::new(5, Some(Arc::new(UpgradeModuleData::new()))).unwrap()), vec![1, 1, 1, 1, 1, 0]),
        ];
        for (name, m, want) in cases.iter_mut() {
            assert_eq!(&bytes(m), want, "{name}");
            // and it loads back what it saved
            assert!(load_bytes(m, want.clone()).is_ok(), "{name} load");
        }
    }

    #[test]
    fn interface_masks_agree_with_the_accessors() {
        use crate::logic::module::{
            body_module::BodyModule, collide_module::CollideModule, create_module::CreateModule, damage_module::DamageModule,
            destroy_module::DestroyModule, die_module::DieModule, special_power_module::SpecialPowerModule,
            special_power_update_module::SpecialPowerUpdateModule, update_module::UpdateModule, upgrade_module::UpgradeModule,
        };
        let all = vec![
            Module::Object(ObjectModule::new(5, Some(data())).unwrap()),
            Module::Drawable(DrawableModule::new(Some(data())).unwrap()),
            Module::Behavior(BehaviorModule::new(5, Some(data())).unwrap()),
            Module::Body(BodyModule::new(5, Some(Arc::new(Default::default()))).unwrap()),
            Module::Collide(CollideModule::new(5, Some(Arc::new(Default::default()))).unwrap()),
            Module::Create(CreateModule::new(5, Some(Arc::new(Default::default()))).unwrap()),
            Module::Damage(DamageModule::new(5, Some(Arc::new(Default::default()))).unwrap()),
            Module::Destroy(DestroyModule::new(5, Some(data())).unwrap()),
            Module::Die(DieModule::new(5, Some(Arc::new(Default::default()))).unwrap()),
            Module::SpecialPower(SpecialPowerModule::new(5, Some(Arc::new(Default::default()))).unwrap()),
            Module::SpecialPowerUpdate(SpecialPowerUpdateModule::new(5, Some(Arc::new(Default::default())), SleepyUpdatePhase::Final).unwrap()),
            Module::Update(UpdateModule::new(5, Some(Arc::new(Default::default())), SleepyUpdatePhase::Initial).unwrap()),
            Module::Upgrade(UpgradeModule::new(5, Some(Arc::new(UpgradeModuleData::new()))).unwrap()),
        ];
        let want = [0, 0, 0, 0x20, 0x10, 0x08, 0x04, 0x200, 0x02, 0x100, 0x01, 0x01, 0x80];
        for (m, w) in all.iter().zip(want) {
            assert_eq!(m.interface_mask(), w);
            // an abstract base answers its own interface; the object/drawable bases none
            assert_eq!(m.interfaces_present(), w);
            assert!(m.get_contain().is_none());
            assert!(m.module_tag_name_key().is_err());
        }
        assert_eq!(all[0].module_type(), None);
        assert_eq!(all[1].module_type(), None);
        assert!(all[2..].iter().all(|m| m.module_type() == Some(ModuleType::Behavior)));
    }
}
