// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// how `Object` talks to the interfaces of its modules. Each variant names the
// upstream virtual call it replaces.

//! The calls `Object` makes through the interfaces of its modules
//! (`BehaviorModule::getDie()->onDie(..)`, `getBody()->attemptDamage(..)`, ...),
//! as one message type, so that a concrete module (layer 9) answers all of
//! them from one `match` and `Object` has one dispatch path to test.
//!
//! # Why messages and not 40 methods
//!
//! Upstream's `Object.cpp` reaches ten module interfaces (create, destroy,
//! die, damage, body, collide, contain, upgrade, special power, update) and
//! the base `BehaviorModule` virtuals; each is a pointer from
//! `BehaviorModule::getX()` that is `NULL` for a module without that
//! interface. Here:
//!
//! - **Which module has which interface** is the `interfaces` mask of its
//!   [`ModuleSlot`](crate::logic::world::object::ModuleSlot)
//!   (`MODULEINTERFACE_*`, the `getInterfaceMask()` of the class), so a loop
//!   over the modules skips the ones that answer `NULL` without calling them.
//! - **The call** is a [`ModuleCall`] sent to the module's body
//!   ([`UpdateBody::module_call`](crate::logic::world::update_body::UpdateBody::module_call))
//!   with the world, and the answer a [`ModuleReply`]. A body that does not
//!   handle a call answers [`ModuleReply::Unhandled`], which is upstream's
//!   empty virtual (`onDelete`, `onCapture`, ... do nothing in the base class).
//! - **The body is checked out while it runs**, as for updates
//!   (`docs/world-design.md`, "Re-entrancy"), and **lends itself** to its
//!   slot around every call that can reach its own object
//!   ([`World::lend_body`](crate::logic::world::game_logic::World::lend_body)).
//!   A call that reaches a module whose body is running (the module calls back
//!   into the object, which reaches the module again: upstream allows it
//!   because it is a plain recursive call) therefore **runs at once**, inside
//!   the outer call, on the live state, in upstream's order. A module that
//!   does not lend itself leaves its call undelivered; that is recorded in
//!   [`World::cuts`](crate::logic::world::World::cuts)`.reentrant_skips` as a
//!   tripwire (it must stay empty), and `reentrant_delivered` counts the calls
//!   that did run re-entrantly.
//!
//! `DamageInfo` crosses by value and comes back in the reply: the module
//! writes `out` (`actualDamageDealt`, `noEffect`...) and the caller reads it.

use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;
use crate::common::upgrade::UpgradeMaskType;
use crate::logic::damage::{DamageInfo, DamageInfoInput};

/// A module of an object: the object and the position in `Object::modules`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleHandle {
    /// The owner.
    pub object: ObjectId,
    /// Position in `Object::modules`.
    pub index: u16,
}

/// A call from `Object` to one of its modules' interfaces.
#[derive(Clone, Debug, PartialEq)]
pub enum ModuleCall {
    // ---- BehaviorModule virtuals, every module ----
    /// `onObjectCreated()` ("allow for inter-Module resolution").
    OnObjectCreated,
    /// `onDrawableBoundToObject()`.
    OnDrawableBoundToObject,
    /// `onDelete()` (`Object::onDestroy`).
    OnDelete,
    /// `onDisabledEdge(becomingDisabled)`.
    OnDisabledEdge { becoming_disabled: bool },
    /// `onCapture(oldOwner, newOwner)`, by player index.
    OnCapture { old_owner: i32, new_owner: i32 },
    // ---- CreateModuleInterface ----
    /// `onCreate()` (the loop of `ThingFactory::newObject`).
    OnCreate,
    // ---- DestroyModuleInterface ----
    /// `onDestroy()` (the loop of `GameLogic::destroyObject`).
    OnDestroy,
    // ---- DieModuleInterface ----
    /// `onDie(damageInfo)`.
    OnDie(DamageInfo),
    // ---- CollideModuleInterface ----
    /// `onCollide(other, loc, normal)`.
    OnCollide { other: ObjectId, loc: Option<Coord3D>, normal: Option<Coord3D> },
    /// `isSalvageCrateCollide()`; replies [`ModuleReply::Bool`].
    IsSalvageCrateCollide,
    // --- p1w11-sonnet ---
    /// `wouldLikeToCollideWith(other)` (`other` is `INVALID_ID` for `NULL`); replies [`ModuleReply::Bool`].
    WouldLikeToCollideWith { other: ObjectId },
    /// `isRailroad()`; replies [`ModuleReply::Bool`].
    IsRailroad,
    /// `isCarBombCrateCollide()`; replies [`ModuleReply::Bool`].
    IsCarBombCrateCollide,
    /// `isHijackedVehicleCrateCollide()`; replies [`ModuleReply::Bool`].
    IsHijackedVehicleCrateCollide,
    /// `isSabotageBuildingCrateCollide()`; replies [`ModuleReply::Bool`].
    IsSabotageBuildingCrateCollide,
    // --- end p1w11-sonnet ---
    // ---- UpgradeModuleInterface ----
    /// `isAlreadyUpgraded()`; replies [`ModuleReply::Bool`].
    IsAlreadyUpgraded,
    /// `attemptUpgrade(keyMask)`; replies [`ModuleReply::Bool`].
    AttemptUpgrade(UpgradeMaskType),
    /// `wouldUpgrade(keyMask)`; replies [`ModuleReply::Bool`].
    WouldUpgrade(UpgradeMaskType),
    /// `resetUpgrade(keyMask)`; replies [`ModuleReply::Bool`].
    ResetUpgrade(UpgradeMaskType),
    /// `isSubObjectsUpgrade()`; replies [`ModuleReply::Bool`].
    IsSubObjectsUpgrade,
    /// `forceRefreshUpgrade()`.
    ForceRefreshUpgrade,
    // ---- SpecialPowerModuleInterface ----
    /// `pauseCountdown(disabling)`.
    PauseCountdown { disabling: bool },
    /// `getSpecialPowerTemplate()->getSpecialPowerType()`; replies
    /// [`ModuleReply::Int`] if the module has a template, anything else if its
    /// template is `NULL`.
    GetSpecialPowerType,
    /// `getSpecialPowerTemplate()->isShortcutPower()`; replies [`ModuleReply::Bool`].
    IsShortcutPower,
    /// `isModuleForPower(template)`; the template by id. Replies [`ModuleReply::Bool`].
    IsModuleForPower { template: u32 },
    /// `doSpecialPower(commandOptions)`.
    DoSpecialPower { command_options: u32 },
    /// `doSpecialPowerAtObject(obj, commandOptions)`.
    DoSpecialPowerAtObject { target: ObjectId, command_options: u32 },
    /// `doSpecialPowerAtLocation(loc, angle, commandOptions)`.
    DoSpecialPowerAtLocation { loc: Coord3D, angle: f32, command_options: u32 },
    /// `doSpecialPowerUsingWaypoints(way, commandOptions)`, the waypoint by id.
    DoSpecialPowerUsingWaypoints { waypoint: u32, command_options: u32 },
    // ---- BodyModuleInterface ----
    /// `attemptDamage(info)`; replies [`ModuleReply::Damage`].
    AttemptDamage(DamageInfo),
    /// `attemptHealing(info)`; replies [`ModuleReply::Damage`].
    AttemptHealing(DamageInfo),
    /// `estimateDamage(input)`; replies [`ModuleReply::Real`].
    EstimateDamage(DamageInfoInput),
    /// `getMaxHealth()`; replies [`ModuleReply::Real`].
    GetMaxHealth,
    /// `getHealth()`; replies [`ModuleReply::Real`].
    GetHealth,
    /// `getDamageScalar()`; replies [`ModuleReply::Real`].
    GetDamageScalar,
    /// `getLastDamageInfo()->in.m_sourceID`; replies [`ModuleReply::Id`].
    GetLastDamageSource,
    /// `setArmorSetFlag(ast)`.
    SetArmorSetFlag(i32),
    /// `clearArmorSetFlag(ast)`.
    ClearArmorSetFlag(i32),
    /// `testArmorSetFlag(ast)`; replies [`ModuleReply::Bool`].
    TestArmorSetFlag(i32),
    /// `onVeterancyLevelChanged(oldLevel, newLevel, provideFeedback)`.
    OnVeterancyLevelChanged { old_level: i32, new_level: i32, provide_feedback: bool },
    /// `setMaxHealth(maxHealth)` (the default change type).
    SetMaxHealth(f32),
    // --- p1l9-upgrades ---
    /// `setMaxHealth(maxHealth, healthChangeType)` (`MaxHealthUpgrade`);
    /// the change type is `MaxHealthChangeType`.
    SetMaxHealthWithChange { max_health: f32, change_type: i32 },
    // --- end p1l9-upgrades ---
    /// `setInitialHealth(initialPercent)`.
    SetInitialHealth(i32),
    /// `setIndestructible(indestructible)`.
    SetIndestructible(bool),
    // ---- ContainModuleInterface ----
    /// `removeFromContain(obj)`.
    RemoveFromContain(ObjectId),
    /// `isEnclosingContainerFor(obj)`; replies [`ModuleReply::Bool`].
    IsEnclosingContainerFor(ObjectId),
    /// `isSpecialZeroSlotContainer()`; replies [`ModuleReply::Bool`].
    IsSpecialZeroSlotContainer,
    /// `getContainedItemsList()`, as ids in list order; replies [`ModuleReply::Ids`].
    GetContainedItems,
    /// `isPassengerAllowedToFire(id)`; replies [`ModuleReply::Bool`].
    IsPassengerAllowedToFire(ObjectId),
    /// `getContainCount()`; replies [`ModuleReply::Int`].
    GetContainCount,
    /// `containReactToTransformChange()`.
    ContainReactToTransformChange,
    /// `isGarrisonable()`; replies [`ModuleReply::Bool`].
    IsGarrisonable,
    /// `isKickOutOnCapture()`; replies [`ModuleReply::Bool`].
    IsKickOutOnCapture,
    /// `removeAllContained(exposeStealthUnits)`.
    RemoveAllContained { expose_stealth_units: bool },
    /// `friend_getRider()`; replies [`ModuleReply::Id`].
    GetRider,
    /// `getContainExitInterface()` is non-null; replies [`ModuleReply::Bool`].
    HasContainExitInterface,
    /// `iterateContained(func, userData, FALSE)` with `localIsHero`: whether a
    /// contained object is `KINDOF_HERO`; replies [`ModuleReply::Bool`].
    IsHeroInside,
    /// `getApparentControllingPlayer(observer)`; replies [`ModuleReply::Int`]
    /// (a player index, -1 for `NULL`).
    GetApparentControllingPlayer { observer: i32 },
    // ---- the other BehaviorModule interfaces Object.cpp asks for ----
    /// `getUpdateExitInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasUpdateExitInterface,
    /// `getProductionUpdateInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasProductionUpdateInterface,
    /// `getDockUpdateInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasDockUpdateInterface,
    /// `getSpawnBehaviorInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasSpawnBehaviorInterface,
    /// `getProjectileUpdateInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasProjectileUpdateInterface,
    /// `getCountermeasuresBehaviorInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasCountermeasuresInterface,
    /// `getParkingPlaceBehaviorInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasParkingPlaceInterface,
    /// `getParkingPlaceBehaviorInterface()->getLandingDeckHeightOffset()`;
    /// replies [`ModuleReply::Real`] (unhandled: no parking place).
    GetLandingDeckHeightOffset,
    /// `getParkingPlaceBehaviorInterface()->defectAllParkedUnits(newTeam, detectionTime)`,
    /// the team by id.
    DefectAllParkedUnits { new_team: u32, detection_time: u32 },
    /// `SpawnBehaviorInterface::orderSlavesDisabledUntil(type, frame)`.
    OrderSlavesDisabledUntil { disabled_type: i32, frame: u32 },
    /// `SpawnBehaviorInterface::orderSlavesToClearDisabled(type)`.
    OrderSlavesToClearDisabled { disabled_type: i32 },
    /// `SpawnBehaviorInterface::onSpawnDeath(deadID, damageInfo)`.
    OnSpawnDeath { dead: ObjectId, info: DamageInfo },
    /// `SpawnBehaviorInterface::canAnySlavesAttack()`; replies [`ModuleReply::Bool`].
    CanAnySlavesAttack,
    /// `RadarUpgrade::getIsDisableProof()`; replies [`ModuleReply::Bool`].
    GetRadarDisableProof,
    /// `OverchargeBehavior::isOverchargeActive()`; replies [`ModuleReply::Bool`].
    IsOverchargeActive,
    /// `AutoHealBehavior::undoUpgrade()`.
    UndoUpgrade,
    /// `ToppleUpdate::isAbleToBeToppled()`; replies [`ModuleReply::Bool`].
    IsAbleToBeToppled,
    /// `ToppleUpdate::applyTopplingForce(direction, speed, options)`.
    ApplyTopplingForce { direction: Option<Coord3D>, speed: f32, options: u32 },
    /// `StickyBombUpdate::detonate()`.
    Detonate,
    /// `ObjectDefectionHelper::startDefectionTimer(time, withDefectorFx)`.
    StartDefectionTimer { time: u32, with_defector_fx: bool },
    /// `ProductionUpdateInterface::cancelAndRefundAllProduction()`.
    CancelAndRefundAllProduction,
    /// `RebuildHoleBehaviorInterface::startRebuildProcess(template, spawnerID)`,
    /// the template by id.
    StartRebuildProcess { template: u16, spawner: ObjectId },
    /// `SlavedUpdateInterface::getSlaverID()`; replies [`ModuleReply::Id`]
    /// (unhandled: no slaved update interface).
    GetSlaverId,
    /// `CountermeasuresBehaviorInterface::isActive()`; replies [`ModuleReply::Bool`].
    CountermeasuresIsActive,
    /// `CountermeasuresBehaviorInterface::reportMissileForCountermeasures(missile)`.
    ReportMissileForCountermeasures { missile: ObjectId },
    /// `SpecialPowerUpdateInterface::doesSpecialPowerHaveOverridableDestinationActive()`;
    /// replies [`ModuleReply::Bool`] (unhandled: no such interface).
    DoesSpecialPowerHaveOverridableDestinationActive,
    /// `SpecialPowerUpdateInterface::doesSpecialPowerHaveOverridableDestination()`;
    /// replies [`ModuleReply::Bool`] (unhandled: no such interface).
    DoesSpecialPowerHaveOverridableDestination,
    /// `SpecialPowerUpdateInterface::isSpecialAbility()` and, if it is,
    /// `SpecialAbilityUpdate::getSpecialPowerType()`; replies
    /// [`ModuleReply::Int`] (anything else: not a special ability).
    GetSpecialAbilityType,
    /// `PhysicsBehavior::applyShock(force)`.
    PhysicsApplyShock(Coord3D),
    /// `PhysicsBehavior::applyRandomRotation()`.
    PhysicsApplyRandomRotation,
    /// `PhysicsBehavior::setStunned(stunned)`.
    PhysicsSetStunned(bool),
    /// `SubdualDamageHelper::notifySubdualDamage(amount)`.
    NotifySubdualDamage { amount: f32 },
    /// `StatusDamageHelper::doStatusDamage(status, duration)`.
    DoStatusDamage { status: i32, duration: f32 },
    /// `TempWeaponBonusHelper::doTempWeaponBonus(status, duration)`.
    DoTempWeaponBonus { status: i32, duration: u32 },
    /// `CountermeasuresBehaviorInterface::calculateCountermeasureToDivertTo(victim)`;
    /// replies [`ModuleReply::Id`].
    CalculateCountermeasureToDivertTo { victim: ObjectId },
    // --- p1w7-sonnet ---
    /// `DockUpdateInterface::isClearToApproach(docker)`; replies [`ModuleReply::Bool`].
    IsClearToApproach { docker: ObjectId },
    /// `SupplyTruckAIInterface::getPreferredDockID()`; replies [`ModuleReply::Id`]
    /// (unhandled: the AI is not a supply truck's, upstream's `NULL` interface).
    GetSupplyTruckPreferredDockId,
    /// `SupplyTruckAIInterface::getWarehouseScanDistance()`; replies [`ModuleReply::Real`].
    GetSupplyTruckWarehouseScanDistance,
    // --- end p1w7-sonnet ---
    // --- p1w10-sonnet ---
    /// `CreateModuleInterface::onBuildComplete()`: "This is called when you are a finished game object" (the
    /// loop in `BuildAssistant`, `GameLogic`, `DozerAIUpdate`, `ProductionUpdate` and `ReplaceObjectUpgrade`).
    OnBuildComplete,
    /// `SpecialPowerModuleInterface::onSpecialPowerCreation()` (`SpecialPowerCreate::onBuildComplete`'s loop).
    OnSpecialPowerCreation,
    /// `DamageModuleInterface::onDamage(damageInfo)` (the loop of `ActiveBody::attemptDamage`).
    OnDamage(DamageInfo),
    /// `LifetimeUpdate::setLifetimeRange(min, max)` / `DeletionUpdate::setLifetimeRange(min, max)` (frames): "the lifetime
    /// module of this object" is found with `findUpdateModule`.
    SetLifetimeRange { min_frames: u32, max_frames: u32 },
    /// `SmartBombTargetHomingUpdate::SetTargetPosition(pos)` (`DeliverPayloadAIUpdate`).
    SetSmartBombTargetPosition(Coord3D),
    // --- end p1w10-sonnet ---
    // --- p1w11-sonnet ---
    /// `PowerPlantUpdateInterface::extendRods(extend)` (`PowerPlantUpgrade::upgradeImplementation` asks every behaviour module of
    /// its object for `getPowerPlantUpdateInterface()`; a module without one answers [`ModuleReply::Unhandled`]).
    ExtendRods { extend: bool },
    /// `RadarUpdate::extendRadar()` (`RadarUpgrade::upgradeImplementation`, on the module `findUpdateModule("RadarUpdate")` finds).
    ExtendRadar,
    /// `loadPostProcess()` of a module that needs the world to run it (`PowerPlantUpgrade` re-adds its power bonus to the player on
    /// load): the world-level post-load pass of `docs/world-design.md` ("A World-aware loadPostProcess") sends it to the modules of an
    /// object in module order, after the world-free `Snapshot::load_post_process` of every module has run.
    LoadPostProcess,
    /// `SpecialPowerModuleInterface::getSpecialPowerTemplate()` as a template id (`SpecialPowerTemplate::getID`): replies
    /// [`ModuleReply::Int`] with the id, or anything else when the pointer is `NULL` (`UnpauseSpecialPowerUpgrade` compares it with its
    /// own pointer, `NULL` included).
    GetSpecialPowerTemplateId,
    /// `SpecialPowerCompletionDie::setCreator(creatorID)` (the object creation list, on the die `findSpecialPowerCompletionDie` finds).
    SetCreator { creator: ObjectId },
    // --- end p1w11-sonnet ---
}

/// What a module answers to a [`ModuleCall`].
#[derive(Clone, Debug, PartialEq)]
pub enum ModuleReply {
    /// The module has no behaviour for that call (upstream's empty virtual, or
    /// a base-class answer of `NULL`/`FALSE`/`0`).
    Unhandled,
    /// A call that returns nothing, handled.
    Done,
    /// A `Bool`.
    Bool(bool),
    /// An `Int` or an enum.
    Int(i32),
    /// A `Real`.
    Real(f32),
    /// An `ObjectID`.
    Id(ObjectId),
    /// A list of ids.
    Ids(Vec<ObjectId>),
    /// The `DamageInfo` after the call.
    Damage(DamageInfo),
}

impl ModuleReply {
    /// The answer as a `Bool`, `false` for anything else.
    pub fn as_bool(&self) -> bool {
        matches!(self, ModuleReply::Bool(true))
    }

    /// The answer as a `Real`, `default` for anything else.
    pub fn as_real(&self, default: f32) -> f32 {
        match self {
            ModuleReply::Real(r) => *r,
            _ => default,
        }
    }

    /// The answer as an id, `INVALID_ID` for anything else.
    pub fn as_id(&self) -> ObjectId {
        match self {
            ModuleReply::Id(i) => *i,
            _ => crate::common::game_type::INVALID_ID,
        }
    }

    /// The answer as an `Int`, `default` for anything else.
    pub fn as_int(&self, default: i32) -> i32 {
        match self {
            ModuleReply::Int(i) => *i,
            _ => default,
        }
    }
}
