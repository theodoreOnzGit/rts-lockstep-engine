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
    /// `PowerPlantUpdate`.
    PowerPlantUpdate(crate::logic::modules::power_plant_update::PowerPlantUpdate),
    /// `RadarUpdate`.
    RadarUpdate(crate::logic::modules::radar_update::RadarUpdate),
    /// `HealCrateCollide`.
    HealCrateCollide(crate::logic::modules::heal_crate_collide::HealCrateCollide),
    /// `ShroudCrateCollide`.
    ShroudCrateCollide(crate::logic::modules::shroud_crate_collide::ShroudCrateCollide),
    // --- end p1w11-sonnet ---
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
            UpdateBody::PowerPlantUpdate(m) => m.world_update(world, me),
            UpdateBody::RadarUpdate(m) => m.world_update(world, me),
            UpdateBody::HealCrateCollide(_) => UPDATE_SLEEP_FOREVER,
            UpdateBody::ShroudCrateCollide(_) => UPDATE_SLEEP_FOREVER,
// --- end p1w11-sonnet ---
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
            UpdateBody::PowerPlantUpdate(m) => m.module_call(world, me, call),
            UpdateBody::RadarUpdate(m) => m.module_call(world, me, call),
            UpdateBody::HealCrateCollide(m) => m.module_call(world, me, call),
            UpdateBody::ShroudCrateCollide(m) => m.module_call(world, me, call),
// --- end p1w11-sonnet ---
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
// --- end p1w11-sonnet ---
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
// --- end p1w11-sonnet ---
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
// --- end p1w11-sonnet ---
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
            UpdateBody::PowerPlantUpdate(m) => m.crc(xfer),
            UpdateBody::RadarUpdate(m) => m.crc(xfer),
            UpdateBody::HealCrateCollide(m) => m.crc(xfer),
            UpdateBody::ShroudCrateCollide(m) => m.crc(xfer),
// --- end p1w11-sonnet ---
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
            UpdateBody::PowerPlantUpdate(m) => m.xfer(xfer),
            UpdateBody::RadarUpdate(m) => m.xfer(xfer),
            UpdateBody::HealCrateCollide(m) => m.xfer(xfer),
            UpdateBody::ShroudCrateCollide(m) => m.xfer(xfer),
// --- end p1w11-sonnet ---
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
            UpdateBody::PowerPlantUpdate(m) => m.load_post_process(),
            UpdateBody::RadarUpdate(m) => m.load_post_process(),
            UpdateBody::HealCrateCollide(m) => m.load_post_process(),
            UpdateBody::ShroudCrateCollide(m) => m.load_post_process(),
// --- end p1w11-sonnet ---
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
// --- end p1w11-sonnet ---
            _ => {}
        }
    }
}
// --- end p1w10-sonnet ---
