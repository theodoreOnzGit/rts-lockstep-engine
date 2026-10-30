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
    // --- p1w13-sonnet --- (batch R3: RailedTransportContain)
    /// `DockUpdateInterface::isDockOpen()`; replies [`ModuleReply::Bool`].
    DockIsOpen,
    /// `DockUpdateInterface::setDockOpen(open)`.
    DockSetOpen(bool),
    /// `DockUpdateInterface::setDockCrippled(setting)` (`SupplyWarehouseCripplingBehavior`, batch R17).
    DockSetDockCrippled(bool),
    /// `StealthUpdate::receiveGrant(active, frames)` (batch R4).
    StealthReceiveGrant { active: bool, frames: u32 },
    /// `AIUpdateInterface::chooseLocomotorSet(set)` (batch R5).
    AiChooseLocomotorSet(i32),
    /// batch R8 (ParachuteContain): `AIUpdateInterface::aiHunt(cmdSource)`.
    AiHunt { command_source: i32 },
    /// `getCurLocomotor()->getPitchStiffness()`, `getRollStiffness()`, `getPitchDamping()`, `getRollDamping()` (0 to 3); replies [`ModuleReply::Real`].
    AiLocomotorSway(u8),
    /// `getCurLocomotor()->setUltraAccurate(on)`.
    AiLocomotorSetUltraAccurate(bool),
    /// batch R72: `ai->getCurLocomotor()->setAllowInvalidPosition(allow)` (SpectreGunshipUpdate; the locomotor checked first with
    /// [`ModuleCall::AiHasCurLocomotor`]).
    AiLocomotorSetAllowInvalidPosition(bool),
    /// `getCurLocomotor()->setCloseEnoughDist(dist)`.
    AiLocomotorSetCloseEnoughDist(f32),
    /// `getCurLocomotor()->setCloseEnoughDist3D(on)`.
    AiLocomotorSetCloseEnoughDist3D(bool),
    /// `PhysicsBehavior::setIsInFreeFall(on)`.
    PhysicsSetIsInFreeFall(bool),
    /// batch R11: `getCaveInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasCaveInterface,
    /// `CaveInterface::setOriginalTeam(team)` (`None` for `NULL`).
    SetOriginalTeam(Option<crate::logic::world::ids::TeamId>),
    /// `CaveInterface::tryToSetCaveIndex(newIndex)`.
    TryToSetCaveIndex(i32),
    /// batch R9: `MobNexusContain::tryToEvacuate(exposeStealthedUnits)`; replies [`ModuleReply::Bool`].
    TryToEvacuate { expose_stealthed_units: bool },
    /// `ExitInterface::useSpawnRallyPoint()`; replies [`ModuleReply::Bool`] (`FALSE` unless the module says otherwise, as upstream's default).
    UseSpawnRallyPoint,
    // --- end p1w13-sonnet ---
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
    /// `OverchargeBehaviorInterface::toggle()` (batch R24).
    OverchargeToggle,
    /// `OverchargeBehaviorInterface::enable(enable)` (batch R24).
    OverchargeEnable(bool),
    /// `getHordeUpdateInterface()->isTrueHordeMember()` (batch R25): [`ModuleReply::Bool`]; [`ModuleReply::Unhandled`] from a module with no
    /// horde interface.
    HordeIsTrueMember,
    /// `HordeUpdateInterface::isInHorde()` (batch R25).
    HordeIsInHorde,
    /// `HordeUpdateInterface::hasFlag()` (batch R25).
    HordeHasFlag,
    /// `HordeUpdateInterface::isAllowedNationalism()` (batch R25).
    HordeIsAllowedNationalism,
    /// `getAIUpdateInterface()->evaluateMoraleBonus()` (batch R25).
    AiEvaluateMoraleBonus,
    /// `AutoHealBehavior::undoUpgrade()`.
    UndoUpgrade,
    // --- p1w12-sonnet ---
    /// `AutoHealBehavior::stopHealing()` (`MinefieldBehavior`).
    StopHealing,
    /// `BodyModuleInterface::setAflame(setting)` (`FlammableUpdate`).
    SetAflame(bool),
    /// `FlammableUpdate::tryToIgnite()` (`FireSpreadUpdate`).
    TryToIgnite,
    /// `OCLUpdate::resetTimer()`.
    OclUpdateResetTimer,
    /// `OCLUpdate::getCountdownPercent()`; replies [`ModuleReply::Real`].
    OclUpdateGetCountdownPercent,
    /// `OCLUpdate::getRemainingFrames()`; replies [`ModuleReply::Int`].
    OclUpdateGetRemainingFrames,
    /// `FlammableUpdate::wouldIgnite()`; replies [`ModuleReply::Bool`].
    WouldIgnite,
    /// `FireSpreadUpdate::startFireSpreading()` (`FlammableUpdate`).
    StartFireSpreading,
    // --- end p1w12-sonnet ---
    // --- p1w13-sonnet ---
    /// `AIUpdateInterface::getGoalObject()`; replies [`ModuleReply::Id`] (`INVALID_ID` for `NULL`).
    AiGetGoalObject,
    /// batch R12 (GarrisonContain): `AIUpdateInterface::getCurrentVictim()`; replies [`ModuleReply::Id`] (`INVALID_ID` for `NULL`).
    AiGetCurrentVictim,
    /// batch R12: `AIUpdateInterface::getCurrentVictimPos()`; replies [`ModuleReply::Coord`], or [`ModuleReply::Unhandled`] for `NULL`.
    AiGetCurrentVictimPos,
    /// `SpecialAbilityUpdate::isActive()`; replies [`ModuleReply::Bool`].
    SpecialAbilityIsActive,
    /// `PhysicsBehavior::getVelocity()`; replies [`ModuleReply::Coord`].
    PhysicsGetVelocity,
    /// `ProneUpdate::goProne(damageInfo)` (`AIUpdateInterface::friend_...`: a unit that is damaged goes prone).
    ProneGoProne(DamageInfo),
    /// `FloatUpdate::setEnabled(enabled)` (`ObjectCreationList`'s `DeliverPayload`/`Debris` nuggets).
    FloatSetEnabled(bool),
    // ---- the rest of BodyModuleInterface (`ActiveBody`) ----
    /// `getDamageState()`; replies [`ModuleReply::Int`].
    GetDamageState,
    /// `setDamageState(newState)`.
    SetDamageState(i32),
    /// `getInitialHealth()`; replies [`ModuleReply::Real`].
    GetInitialHealth,
    /// `getPreviousHealth()`; replies [`ModuleReply::Real`].
    GetPreviousHealth,
    /// `internalChangeHealth(delta)`.
    InternalChangeHealth(f32),
    /// `evaluateVisualCondition()`.
    EvaluateVisualCondition,
    /// `updateBodyParticleSystems()`.
    UpdateBodyParticleSystems,
    /// `isSubdued()`; replies [`ModuleReply::Bool`].
    IsSubdued,
    /// `canBeSubdued()`; replies [`ModuleReply::Bool`].
    CanBeSubdued,
    /// `onSubdualChange(isNowSubdued)`.
    OnSubdualChange(bool),
    /// `getCurrentSubdualDamageAmount()`; replies [`ModuleReply::Real`].
    GetCurrentSubdualDamageAmount,
    /// `*getLastDamageInfo()`; replies [`ModuleReply::Damage`] (unhandled: the body has none).
    GetLastDamageInfo,
    /// `getLastDamageTimestamp()`; replies [`ModuleReply::Int`] (the frame as an `Int`).
    GetLastDamageTimestamp,
    /// `getLastHealingTimestamp()`; replies [`ModuleReply::Int`] (the frame as an `Int`).
    GetLastHealingTimestamp,
    /// `getClearableLastAttacker()`; replies [`ModuleReply::Id`].
    GetClearableLastAttacker,
    /// `clearLastAttacker()`.
    ClearLastAttacker,
    /// `getFrontCrushed()`; replies [`ModuleReply::Bool`].
    GetFrontCrushed,
    /// `getBackCrushed()`; replies [`ModuleReply::Bool`].
    GetBackCrushed,
    /// `setFrontCrushed(v)`.
    SetFrontCrushed(bool),
    /// `setBackCrushed(v)`.
    SetBackCrushed(bool),
    /// `isIndestructible()`; replies [`ModuleReply::Bool`].
    IsIndestructible,
    /// `applyDamageScalar(scalar)`.
    ApplyDamageScalar(f32),
    // ---- DamageModuleInterface ----
    /// `onHealing(damageInfo)` (the loop of `ActiveBody::attemptHealing`).
    OnHealing(DamageInfo),
    /// `onBodyDamageStateChange(damageInfo, oldState, newState)`.
    OnBodyDamageStateChange { info: DamageInfo, old_state: i32, new_state: i32 },
    // ---- ContainModuleInterface, the rest ----
    /// `isImmuneToClearBuildingAttacks()`; replies [`ModuleReply::Bool`].
    IsImmuneToClearBuildingAttacks,
    /// `isRiderChangeContain()`; replies [`ModuleReply::Bool`].
    IsRiderChangeContain,
    /// `orderAllPassengersToIdle(CMD_FROM_AI)`.
    OrderAllPassengersToIdle,
    /// `orderAllPassengersToHackInternet(CMD_FROM_AI)`.
    OrderAllPassengersToHackInternet,
    // ---- AIUpdateInterface ----
    /// `isMoving()`; replies [`ModuleReply::Bool`].
    AiIsMoving,
    /// `isIdle()`; replies [`ModuleReply::Bool`].
    AiIsIdle,
    /// `aiIdle(CMD_FROM_AI)`.
    AiIdle,
    /// `aiEvacuateInstantly(exposeStealthUnits, CMD_FROM_AI)`.
    AiEvacuateInstantly { expose_stealth_units: bool },
    /// `aiGuardRetaliate(victim, pos, NO_MAX_SHOTS_LIMIT, CMD_FROM_AI)`.
    AiGuardRetaliate { victim: ObjectId, pos: Coord3D },
    /// `StructureBody::setConstructorObject(obj)`: the id of the object (`INVALID_ID` is a null pointer).
    SetConstructorObject(ObjectId),
    /// `StructureBody::getConstructorObjectID()`.
    GetConstructorObjectId,
    // ---- OpenContain: the rest of the contain, collide and exit interfaces, and what the container asks of its riders ----
    /// `addToContain(obj)`.
    AddToContain(ObjectId),
    /// `addToContainList(obj)`.
    AddToContainList(ObjectId),
    /// `removeFromContain(obj, exposeStealthUnits)`.
    RemoveFromContainExposing { obj: ObjectId, expose_stealth_units: bool },
    /// `killAllContained()`.
    KillAllContained,
    /// `harmAndForceExitAllContained(info)`; replies [`ModuleReply::Damage`].
    HarmAndForceExitAllContained(DamageInfo),
    /// `isValidContainerFor(obj, checkCapacity)`; replies [`ModuleReply::Bool`].
    IsValidContainerFor { obj: ObjectId, check_capacity: bool },
    /// `onContaining(obj, wasSelected)`.
    OnContaining { obj: ObjectId, was_selected: bool },
    /// `onRemoving(obj)`.
    OnRemoving(ObjectId),
    /// `onSelling()`.
    OnSelling,
    /// `getContainMax()`; replies [`ModuleReply::Int`].
    GetContainMax,
    /// `orderAllPassengersToExit(commandSource, instantly)`.
    OrderAllPassengersToExit { command_source: i32, instantly: bool },
    /// `orderAllPassengersToIdle(commandSource)`.
    OrderAllPassengersToIdleFrom { command_source: i32 },
    /// `orderAllPassengersToHackInternet(commandSource)`.
    OrderAllPassengersToHackInternetFrom { command_source: i32 },
    /// `markAllPassengersDetected()`.
    MarkAllPassengersDetected,
    /// `setPassengerAllowedToFire(permission)`.
    SetPassengerAllowedToFire(bool),
    /// `setOverrideDestination(pos)`.
    SetOverrideDestination(Coord3D),
    /// `isDisplayedOnControlBar()`; replies [`ModuleReply::Bool`].
    IsDisplayedOnControlBar,
    /// `getExtraSlotsInUse()`; replies [`ModuleReply::Int`].
    GetExtraSlotsInUse,
    /// `getContainedItemsMass()`; replies [`ModuleReply::Real`].
    GetContainedItemsMass,
    /// `getStealthUnitsContained()`; replies [`ModuleReply::Int`].
    GetStealthUnitsContained,
    /// `getPlayerWhoEntered()`; replies [`ModuleReply::Int`] (the mask).
    GetPlayerWhoEntered,
    /// `processDamageToContained(percentDamage)`.
    ProcessDamageToContained(f32),
    /// `enableLoadSounds(enable)`.
    EnableLoadSounds(bool),
    /// `setEvacDisposition(disp)`.
    SetEvacDisposition(i32),
    /// `isWeaponBonusPassedToPassengers()`; replies [`ModuleReply::Bool`].
    IsWeaponBonusPassedToPassengers,
    /// `getWeaponBonusPassedToPassengers()`; replies [`ModuleReply::Int`] (the flags).
    GetWeaponBonusPassedToPassengers,
    /// `isBustable()`; replies [`ModuleReply::Bool`].
    IsBustable,
    /// `isHealContain()`; replies [`ModuleReply::Bool`].
    IsHealContain,
    /// `isTunnelContain()`; replies [`ModuleReply::Bool`].
    IsTunnelContain,
    /// `isSpecialOverlordStyleContainer()`; replies [`ModuleReply::Bool`].
    IsSpecialOverlordStyleContainer,
    /// `isAnyRiderAttacking()`; replies [`ModuleReply::Bool`].
    IsAnyRiderAttacking,
    /// `clientVisibleContainedFlashAsSelected()`.
    ClientVisibleContainedFlashAsSelected,
    /// `recalcApparentControllingPlayer()`.
    RecalcApparentControllingPlayer,
    /// `onObjectWantsToEnterOrExit(obj, wants)`, `wants` an `ObjectEnterExitType`.
    OnObjectWantsToEnterOrExit { obj: ObjectId, wants: i32 },
    /// `hasObjectsWantingToEnterOrExit()`; replies [`ModuleReply::Bool`].
    HasObjectsWantingToEnterOrExit,
    /// `isExitBusy()`; replies [`ModuleReply::Bool`].
    IsExitBusy,
    /// `exitObjectViaDoor(obj, door)`.
    ExitObjectViaDoor { obj: ObjectId, door: i32 },
    /// `exitObjectInAHurry(obj)`.
    ExitObjectInAHurry(ObjectId),
    /// `setRallyPoint(pos)`.
    SetRallyPoint(Coord3D),
    /// `getRallyPoint()`; replies [`ModuleReply::Coord`] ([`ModuleReply::Unhandled`]: `NULL`).
    GetRallyPoint,
    /// `getNaturalRallyPoint(rallyPoint, offset)`; replies [`ModuleReply::Coord`].
    GetNaturalRallyPoint,
    /// `scatterToNearbyPosition(rider)` (a protected member of the base class the derived containers call; the test reaches it through this).
    ScatterToNearbyPosition(ObjectId),
    /// `redeployOccupants()` (protected).
    RedeployOccupants,
    /// `putObjAtNextFirePoint(obj)` (protected).
    PutObjAtNextFirePoint(ObjectId),
    /// The objects `xfer` found in the contain list of a module that was loaded over one that was not empty ("a few rogue things come into existence preloaded with
    /// stuff"): upstream destroys them in the middle of the xfer, which has no world here, so the world destroys them (`TheGameLogic->destroyObject`) when it gets
    /// this; replies [`ModuleReply::Ids`] and empties the list.
    DrainXferDestroys,
    // ---- the AI, physics and stealth calls the contain modules make on their riders ----
    /// `aiIdle(commandSource)`.
    AiIdleFrom { command_source: i32 },
    /// `aiExit(container, commandSource)`.
    AiExit { container: ObjectId, command_source: i32 },
    /// `aiExitInstantly(container, commandSource)`.
    AiExitInstantly { container: ObjectId, command_source: i32 },
    /// `aiHackInternet(commandSource)`.
    AiHackInternet { command_source: i32 },
    /// `aiMoveToPosition(pos, commandSource)`.
    AiMoveToPosition { pos: Coord3D, command_source: i32 },
    // --- p1w13-sonnet --- (batch R81: SpecialAbilityUpdate)
    /// `aiMoveToObject(obj, commandSource)`.
    AiMoveToObject { target: ObjectId, command_source: i32 },
    /// `aiFaceObject(obj, commandSource)`.
    AiFaceObject { target: ObjectId, command_source: i32 },
    /// `aiFacePosition(pos, commandSource)`.
    AiFacePosition { pos: Coord3D, command_source: i32 },
    /// `SpecialAbilityUpdate::onExit(cleanup)`.
    SpecialAbilityOnExit { cleanup: bool },
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R13e: HijackerUpdate, set by ConvertToHijackedVehicleCrateCollide)
    /// `HijackerUpdate::setTargetObject(object)` (`INVALID_ID` for `NULL`).
    HijackerSetTargetObject(ObjectId),
    /// `HijackerUpdate::setIsInVehicle(i)`.
    HijackerSetIsInVehicle(bool),
    /// `HijackerUpdate::setUpdate(u)`.
    HijackerSetUpdate(bool),
    /// `getAIUpdateInterface()->getDozerAIInterface() != NULL` (`ConvertToHijackedVehicleCrateCollide`).
    AiGetDozerAIInterface,
    /// `getDozerAIInterface()->cancelTask(task)`.
    DozerCancelTask(i32),
    /// `getAI()->transferAttack(fromID, toID)` (`CreateObjectDie`, batch R14a).
    AiTransferAttack { from: ObjectId, to: ObjectId },
    /// `getAIUpdateInterface()->notifyCrate(crateID)` (`CreateCrateDie`, batch R14c).
    AiNotifyCrate(ObjectId),
    /// `getAIUpdateInterface()->isDoingGroundMovement()` (the production exits, batch R18); replies [`ModuleReply::Bool`].
    AiIsDoingGroundMovement,
    /// `getAIUpdateInterface()->aiGetHealed(healDepot, cmdSource)` (`AutoFindHealingUpdate`, batch R19).
    AiGetHealed { target: ObjectId, command_source: i32 },
    /// `getAIUpdateInterface()->aiEnter(obj, cmdSource)` (`PilotFindVehicleUpdate`, batch R19).
    AiEnter { target: ObjectId, command_source: i32 },
    /// `RadiusDecalUpdate::createRadiusDecal(tmpl, radius, pos)` (batch R20; `ObjectCreationList`'s attack nugget calls it).
    RadiusDecalCreate { template: crate::logic::radius_decal::RadiusDecalTemplate, radius: f32, pos: Coord3D },
    /// `RadiusDecalUpdate::killRadiusDecal()` (batch R20).
    RadiusDecalKill,
    /// `RadiusDecalUpdate::killWhenNoLongerAttacking(v)` (batch R20).
    RadiusDecalKillWhenNoLongerAttacking(bool),
    /// `getPhysics()->getTurning()` (`AnimationSteeringUpdate`, batch R20): replies [`ModuleReply::Int`], `TURN_NEGATIVE` -1, `TURN_NONE` 0,
    /// `TURN_POSITIVE` 1.
    PhysicsGetTurning,
    /// `getAIUpdateInterface()->aiFollowExitProductionPath(&path, ignoreObject, cmdSource)` (the production exits, batch R18).
    AiFollowExitProductionPath { path: Vec<Coord3D>, ignore_object: ObjectId, command_source: i32 },
    /// `ExitInterface::unreserveDoorForExit(door)` (batch R18).
    UnreserveDoorForExit(i32),
    /// `ExitInterface::exitObjectByBudding(newObj, budHost)` (batch R18; `INVALID_ID` for a `NULL` host).
    ExitObjectByBudding { obj: ObjectId, bud_host: ObjectId },
    /// `ExitInterface::getExitPosition(&pos)` (batch R18); replies [`ModuleReply::Coord`] (`Unhandled`: false).
    GetExitPosition,
    /// `ExitInterface::getNaturalRallyPoint(&pos, offset)` with the offset given (batch R18; [`ModuleCall::GetNaturalRallyPoint`] is `offset` true);
    /// replies [`ModuleReply::Coord`] (`Unhandled`: false).
    GetNaturalRallyPointWithOffset(bool),
    /// `getAIUpdateInterface()->construct(template, pos, angle, owner, isRebuild)` (`RebuildHoleBehavior`, batch R15): the template by id
    /// (`None`: `NULL`), the owner a player index; replies [`ModuleReply::Id`] (`INVALID_ID`: `NULL`).
    AiConstruct { template: Option<u16>, pos: Coord3D, angle: f32, owner: Option<crate::logic::world::ids::PlayerIndex>, is_rebuild: bool },
    /// `getAIUpdateInterface()->aiResumeConstruction(obj, cmdSource)` (batch R15).
    AiResumeConstruction { object: ObjectId, command_source: i32 },
    /// `StickyBombUpdate::getTargetObject()` (batch R15: `RebuildHoleBehavior::transferBombs`); replies [`ModuleReply::Id`] (`INVALID_ID`: `NULL`).
    StickyBombGetTargetObject,
    /// `StickyBombUpdate::setTargetObject(obj)` (batch R15).
    StickyBombSetTargetObject(ObjectId),
    // --- end p1w13-sonnet ---
    /// `ignoreObstacle(obstacle)` (`INVALID_ID` for `NULL`).
    AiIgnoreObstacle { obstacle: ObjectId },
    /// `getEnterTarget()`; replies [`ModuleReply::Id`].
    AiGetEnterTarget,
    /// `setIgnoreCollisionTime(frames)`.
    AiSetIgnoreCollisionTime { frames: i32 },
    /// `aiFollowPath(path, ignoreObject, commandSource)`.
    AiFollowPath { path: Vec<Coord3D>, ignore_object: ObjectId, command_source: i32 },
    /// `doQuickExit(path)`.
    AiDoQuickExit { path: Vec<Coord3D> },
    /// `PhysicsBehavior::getMass()`; replies [`ModuleReply::Real`].
    PhysicsGetMass,
    /// `PhysicsBehavior::getAllowToFall()`; replies [`ModuleReply::Bool`].
    PhysicsGetAllowToFall,
    // --- p1w13-sonnet --- (batch R2: TransportContain)
    /// `PhysicsBehavior::applyMotiveForce(force)`.
    PhysicsApplyMotiveForce(Coord3D),
    /// `PhysicsBehavior::getCenterOfMassOffset()`; replies [`ModuleReply::Real`].
    PhysicsGetCenterOfMassOffset,
    /// `PhysicsBehavior::setPitchRate(pitch)`.
    PhysicsSetPitchRate(f32),
    /// `AIUpdateInterface::setAttitude(attitude)` (`AttitudeType`).
    AiSetAttitude(i32),
    /// `AIUpdateInterface::wakeUpAndAttemptToTarget()`.
    AiWakeUpAndAttemptToTarget,
    /// `AIUpdateInterface::getAiFreeToExit(exiter)`; replies [`ModuleReply::Int`] (`AIFreeToExitType`).
    AiGetFreeToExit { exiter: ObjectId },
    /// `AIUpdateInterface::getCurLocomotor() != NULL`; replies [`ModuleReply::Bool`].
    AiHasCurLocomotor,
    /// batch R60: `AIUpdateInterface::getCurLocomotor()->getPreferredHeight()`; replies [`ModuleReply::Real`], anything else for no locomotor.
    AiGetCurLocomotorPreferredHeight,
    /// `ExitInterface::reserveDoorForExit(objType, specificObject)` of a contain module; replies [`ModuleReply::Int`] (`ExitDoorType`).
    ReserveDoorForExit { template: Option<u16>, specific: ObjectId },
    /// `killRidersWhoAreNotFreeToExit()` of a contain module (protected upstream; the differential test calls it).
    KillRidersWhoAreNotFreeToExit,
    /// `TransportContain::isSpecificRiderFreeToExit(obj)` (protected upstream; the differential test calls it); replies [`ModuleReply::Bool`], or
    /// `Unhandled` for a container that is no transport.
    IsSpecificRiderFreeToExit { obj: ObjectId },
    // --- end p1w13-sonnet ---
    /// `StealthUpdate::markAsDetected()`.
    StealthMarkAsDetected,
    // --- p1w13-sonnet --- (batch R26: StealthUpdate)
    /// `StealthUpdate::markAsDetected(numFrames)` with a count of frames.
    StealthMarkAsDetectedFor(u32),
    /// `StealthUpdate::disguiseAsObject(target)` (`INVALID_ID` for `NULL`).
    StealthDisguiseAsObject(ObjectId),
    /// `SpawnBehaviorInterface::areAllSlavesStealthed()`; replies [`ModuleReply::Bool`].
    SpawnAreAllSlavesStealthed,
    /// `SpawnBehaviorInterface::revealSlaves()`.
    SpawnRevealSlaves,
    /// `AIUpdateInterface::getLastCommandSource()`; replies [`ModuleReply::Int`] (a `CommandSourceType`).
    AiGetLastCommandSource,
    /// batch R59: `AssistedTargetingUpdate::isFreeToAssist()`; replies [`ModuleReply::Bool`].
    AssistedTargetingIsFreeToAssist,
    /// batch R59: `AssistedTargetingUpdate::assistAttack(requestingObject, victimObject)`.
    AssistedTargetingAssistAttack { requesting: ObjectId, victim: ObjectId },
    /// batch R58: `AIUpdateInterface::getGoalPosition()`; replies [`ModuleReply::Coord`].
    AiGetGoalPosition,
    /// batch R58: `AIUpdateInterface::getLocomotorDistanceToGoal()`; replies [`ModuleReply::Real`].
    AiGetLocomotorDistanceToGoal,
    /// batch R58: `AIUpdateInterface::isAttacking()`; replies [`ModuleReply::Bool`].
    AiIsAttacking,
    /// batch R58: `AIUpdateInterface::getNextMoodTarget(calledByAI, calledDuringIdle)`; replies [`ModuleReply::Id`].
    AiGetNextMoodTarget { called_by_ai: bool, called_during_idle: bool },
    /// batch R58: `SpawnBehaviorInterface::maySpawnSelfTaskAI(ratio)`; replies [`ModuleReply::Bool`].
    SpawnMaySpawnSelfTaskAi(f32),
    /// batch R57: `AIUpdateInterface::getPath()->getLastNode()->getPosition()`; replies [`ModuleReply::Coord`] (unhandled: no path).
    AiGetPathLastNodePosition,
    /// batch R57: `getCurLocomotor()->setUsePreciseZPos(on)`.
    AiLocomotorSetUsePreciseZPos(bool),
    /// batch R57: `AIUpdateInterface::aiGoProne(damageInfo, cmdSource)`.
    AiGoProne { info: DamageInfo, command_source: i32 },
    /// batch R57: `SlavedUpdateInterface::onEnslave(slaver)`.
    SlavedOnEnslave { slaver: ObjectId },
    /// batch R57: `SlavedUpdateInterface::onSlaverDamage(info)`.
    SlavedOnSlaverDamage(DamageInfo),
    /// batch R56: `AIUpdateInterface::isBusy()`; replies [`ModuleReply::Bool`].
    AiIsBusy,
    /// batch R56: `AIUpdateInterface::aiBusy(cmdSource)`.
    AiBusy { command_source: i32 },
    /// batch R56: `AIUpdateInterface::aiAttackObject(victim, maxShotsToFire, cmdSource)`.
    AiAttackObject { victim: ObjectId, max_shots: i32, command_source: i32 },
    /// batch R63: `AIUpdateInterface::aiAttackPosition(pos, maxShotsToFire, cmdSource)`, `None` for a `NULL` position.
    AiAttackPosition { pos: Option<Coord3D>, max_shots: i32, command_source: i32 },
    /// batch R63: `AIUpdateInterface::setTurretTargetPosition(turret, pos)`.
    AiSetTurretTargetPosition { turret: i32, pos: Coord3D },
    /// batch R63: `AIUpdateInterface::setTurretTargetObject(turret, target, isForceAttacking)`.
    AiSetTurretTargetObject { turret: i32, target: ObjectId, force_attacking: bool },
    /// batch R69: `ai->getAttackInfo()->getPriority(other->getTemplate())`; replies [`ModuleReply::Int`], [`ModuleReply::Unhandled`] for a
    /// `NULL` attack info.
    AiGetAttackPriority { other: ObjectId },
    /// batch R69: `CommandButtonHuntUpdate::setCommandButton(buttonName)` (`ScriptActions`' call).
    CommandButtonHuntSetCommandButton(Vec<u8>),
    /// batch R70: `AIUpdateInterface::aiFollowWaypointPath(waypoint, cmdSource)`, the waypoint by its id.
    AiFollowWaypointPath { waypoint: i32, command_source: i32 },
    /// batch R70: `AIUpdateInterface::setPathExtraDistance(dist)`.
    AiSetPathExtraDistance(f32),
    /// batch R71: `AIUpdateInterface::getWhichTurretForCurWeapon()`; replies [`ModuleReply::Int`] (`TURRET_INVALID`, -1: none).
    AiGetWhichTurretForCurWeapon,
    /// batch R71: `AIUpdateInterface::setTurretEnabled(turret, enabled)`.
    AiSetTurretEnabled { turret: i32, enabled: bool },
    /// batch R71: `AIUpdateInterface::recenterTurret(turret)`.
    AiRecenterTurret { turret: i32 },
    /// batch R71: `AIUpdateInterface::isTurretInNaturalPosition(turret)`; replies [`ModuleReply::Bool`].
    AiIsTurretInNaturalPosition { turret: i32 },
    /// batch R71: `SpecialPowerUpdateInterface::isActive()`; replies [`ModuleReply::Bool`].
    SpecialPowerUpdateIsActive,
    /// batch R71: `SpecialPowerUpdateInterface::getCommandOption()`; replies [`ModuleReply::Int`] (a `CommandOption`).
    SpecialPowerUpdateGetCommandOption,
    /// batch R71: `SpecialPowerUpdateInterface::isPowerCurrentlyInUse(command)`; replies [`ModuleReply::Bool`].
    IsPowerCurrentlyInUse,
    /// batch R71: `BattlePlanUpdate::getActiveBattlePlan()`; replies [`ModuleReply::Int`] (a `BattlePlanStatus`).
    BattlePlanGetActiveBattlePlan,
    /// `PhysicsBehavior::getVelocityMagnitude()`; replies [`ModuleReply::Real`].
    PhysicsGetVelocityMagnitude,
    /// `PhysicsBehavior::resetDynamicPhysics()`.
    PhysicsResetDynamicPhysics,
    /// batch R27: `StealthUpdate::getEnemyDetectionEvaEvent()`; replies [`ModuleReply::Int`] (an `EvaMessage`, -1 for `EVA_Invalid`).
    StealthGetEnemyDetectionEvaEvent,
    /// batch R27: `StealthUpdate::getOwnDetectionEvaEvent()`; replies [`ModuleReply::Int`].
    StealthGetOwnDetectionEvaEvent,
    /// batch R27: `StealthDetectorUpdate::setSDEnabled(enabled)` (`BattlePlanUpdate`).
    StealthDetectorSetEnabled(bool),
    /// batch R27: `StealthDetectorUpdate::isSDEnabled()`; replies [`ModuleReply::Bool`].
    StealthDetectorIsEnabled,
    // --- end p1w13-sonnet ---
    /// `SpawnBehaviorInterface::getClosestSlave(pos)` (`HiveStructureBody`); replies [`ModuleReply::Id`] (`INVALID_ID`: no slave).
    GetClosestSlave(Coord3D),
    /// `ContainModuleInterface::getClosestRider(pos)` (`HiveStructureBody`); replies [`ModuleReply::Id`] (`INVALID_ID`: no rider).
    GetClosestRider(Coord3D),
    // ---- SlowDeathBehavior and the physics, AI and slaved calls it makes ----
    /// `SlowDeathBehaviorInterface::beginSlowDeath(damageInfo)`.
    BeginSlowDeath(DamageInfo),
    /// `SlowDeathBehaviorInterface::getProbabilityModifier(damageInfo)`; replies [`ModuleReply::Int`] (a module that is no slow death behaviour replies
    /// [`ModuleReply::Unhandled`]).
    GetProbabilityModifier(DamageInfo),
    /// `SlowDeathBehaviorInterface::isDieApplicable(damageInfo)`; replies [`ModuleReply::Bool`] ([`ModuleReply::Unhandled`] for a module that is no slow death behaviour).
    IsDieApplicable(DamageInfo),
    /// `PhysicsBehavior::setAllowToFall(allow)`.
    PhysicsSetAllowToFall(bool),
    /// `PhysicsBehavior::applyForce(&force)`.
    PhysicsApplyForce(Coord3D),
    /// `PhysicsBehavior::setExtraBounciness(bounciness)`.
    PhysicsSetExtraBounciness(f32),
    /// `PhysicsBehavior::setExtraFriction(friction)`.
    PhysicsSetExtraFriction(f32),
    /// `PhysicsBehavior::setAllowBouncing(allow)`.
    PhysicsSetAllowBouncing(bool),
    /// `PhysicsBehavior::setAngles(yaw, pitch, roll)`.
    PhysicsSetAngles { yaw: f32, pitch: f32, roll: f32 },
    /// `PhysicsBehavior::getLastCollidee()`; replies [`ModuleReply::Id`].
    PhysicsGetLastCollidee,
    /// `AIUpdateInterface::isAiInDeadState()`; replies [`ModuleReply::Bool`].
    AiIsAiInDeadState,
    /// `AIUpdateInterface::markAsDead()`.
    AiMarkAsDead,
    /// batch R29: `BoneFXUpdate::stopAllBoneFX()` (`StructureCollapseUpdate::doCollapseDoneStuff`, on the object's `BoneFXUpdate`).
    BoneFxStopAll,
    /// batch R30: `BoneFXUpdate::changeBodyDamageState(oldState, newState)` (`BoneFXDamage::onBodyDamageStateChange`).
    BoneFxChangeBodyDamageState { old_state: i32, new_state: i32 },
    /// `SlavedUpdate::onSlaverDie(NULL)`.
    SlavedOnSlaverDie,
    // ---- the other interfaces ActiveBody asks for ----
    /// `ProjectileUpdateInterface::projectileNowJammed()`.
    ProjectileNowJammed,
    /// `getBridgeBehaviorInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasBridgeBehaviorInterface,
    /// `BridgeBehaviorInterface::getTowerID(BridgeTowerType)`; replies [`ModuleReply::Id`].
    GetBridgeTowerId { tower: i32 },
    /// batch R77: `BridgeBehaviorInterface::setTower(towerType, tower)` (`INVALID_ID`: `NULL`).
    BridgeSetTower { tower: i32, object: ObjectId },
    /// batch R77: `BridgeBehaviorInterface::createScaffolding()`.
    BridgeCreateScaffolding,
    /// batch R77: `BridgeBehaviorInterface::removeScaffolding()`.
    BridgeRemoveScaffolding,
    /// batch R77: `BridgeBehaviorInterface::isScaffoldInMotion()`; replies [`ModuleReply::Bool`].
    BridgeIsScaffoldInMotion,
    /// batch R77: `BridgeBehaviorInterface::isScaffoldPresent()`; replies [`ModuleReply::Bool`].
    BridgeIsScaffoldPresent,
    /// batch R78: `FiringTracker::shotFired(weaponFired, victimID)` (the weapon by its slot in the object's set; `INVALID_ID`: a ground shot).
    FiringTrackerShotFired { slot: i32, victim: ObjectId },
    /// batch R78: `FiringTracker::getNumConsecutiveShotsAtVictim(victim)` (`None`: `NULL`); replies [`ModuleReply::Int`].
    FiringTrackerGetNumConsecutiveShotsAtVictim(Option<ObjectId>),
    /// batch R78: `FiringTracker::getLastShotVictim()`; replies [`ModuleReply::Id`].
    FiringTrackerGetLastShotVictim,
    /// batch R79: `SupplyTruckAIInterface::getNumberBoxes()` (of an AI module); replies [`ModuleReply::Int`].
    SupplyTruckGetNumberBoxes,
    /// batch R79: `SupplyTruckAIInterface::isAvailableForSupplying()`; replies [`ModuleReply::Bool`].
    SupplyTruckIsAvailableForSupplying,
    /// batch R79: `DozerAIInterface::getCurrentTask()` (of an AI module); replies [`ModuleReply::Int`].
    DozerGetCurrentTask,
    /// batch R79: `DozerAIInterface::getTaskTarget(task)`; replies [`ModuleReply::Id`].
    DozerGetTaskTarget(i32),
    /// batch R79: `SpecialAbilityUpdate::getSpecialObjectCount()`; replies [`ModuleReply::Int`].
    SpecialAbilityGetSpecialObjectCount,
    /// batch R79: `SpecialAbilityUpdate::getSpecialObjectMax()`; replies [`ModuleReply::Int`].
    SpecialAbilityGetSpecialObjectMax,
    /// batch R79: `SpecialAbilityUpdate::findSpecialObjectWithProducerID(target) != NULL`; replies [`ModuleReply::Bool`].
    SpecialAbilityFindSpecialObjectWithProducerId(ObjectId),
    // --- end p1w13-sonnet ---
    /// `ToppleUpdate::isAbleToBeToppled()`; replies [`ModuleReply::Bool`].
    IsAbleToBeToppled,
    /// `ToppleUpdate::applyTopplingForce(direction, speed, options)`.
    ApplyTopplingForce { direction: Option<Coord3D>, speed: f32, options: u32 },
    // --- p1w13-sonnet --- (batch R35: `BridgeScaffoldBehaviorInterface`, which `BridgeBehavior` drives)
    /// `setPositions(createPos, riseToPos, buildPos)`.
    ScaffoldSetPositions { create_pos: Coord3D, rise_to_pos: Coord3D, build_pos: Coord3D },
    /// `setMotion(targetMotion)` (`ScaffoldTargetMotion` as its value).
    ScaffoldSetMotion(i32),
    /// `getCurrentMotion()`; replies [`ModuleReply::Int`].
    ScaffoldGetCurrentMotion,
    /// `reverseMotion()`.
    ScaffoldReverseMotion,
    /// `setLateralSpeed(lateralSpeed)`.
    ScaffoldSetLateralSpeed(f32),
    /// `setVerticalSpeed(verticalSpeed)`.
    ScaffoldSetVerticalSpeed(f32),
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R39: StickyBombUpdate and what it asks of LifetimeUpdate)
    /// `LifetimeUpdate::getDieFrame()`; replies [`ModuleReply::Int`] (the frame's bits).
    LifetimeGetDieFrame,
    /// `StickyBombUpdate::initStickyBomb(target, bomber, specificPos)` (an id that is no object is `NULL`).
    StickyBombInit { target: ObjectId, bomber: ObjectId, specific_pos: Option<Coord3D> },
    /// `StickyBombUpdate::isTimedBomb()`; replies [`ModuleReply::Bool`].
    StickyBombIsTimedBomb,
    /// `StickyBombUpdate::getDetonationFrame()`; replies [`ModuleReply::Int`] (the frame's bits).
    StickyBombGetDetonationFrame,
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R40: MinefieldBehavior's LandMineInterface and what it asks of an AI)
    /// `AIUpdateInterface::isClearingMines()`; replies [`ModuleReply::Bool`].
    AiIsClearingMines,
    /// `LandMineInterface::setScootParms(start, end)`.
    LandMineSetScootParms { start: Coord3D, end: Coord3D },
    /// `LandMineInterface::disarm()`.
    LandMineDisarm,
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R41)
    /// `GenerateMinefieldBehavior::setMinefieldTarget(pos)` (`None` for `NULL`).
    GenerateMinefieldSetTarget(Option<Coord3D>),
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R42: `ProjectileStreamUpdate`, which `Weapon::fireWeaponTemplate` drives)
    /// `addProjectile(sourceID, newID, victimID, victimPos)` (`INVALID_ID` for no victim, `None` for a `NULL` position).
    ProjectileStreamAddProjectile { source: ObjectId, new_id: ObjectId, victim: ObjectId, victim_pos: Option<Coord3D> },
    /// `setPosition(newPosition)`.
    ProjectileStreamSetPosition(Coord3D),
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R43: the rest of `DockUpdateInterface`, which the AI's dock state machine drives; a docker or drone of
    // `INVALID_ID` is `NULL`, a `position` of `None`/`false` is a `NULL` pointer)
    /// `reserveApproachPosition(docker, position, &index)`; replies [`ModuleReply::Approach`].
    DockReserveApproachPosition { docker: ObjectId, position: Option<Coord3D>, index: i32 },
    /// `advanceApproachPosition(docker, position, &index)`; replies [`ModuleReply::Approach`].
    DockAdvanceApproachPosition { docker: ObjectId, position: Option<Coord3D>, index: i32 },
    /// `isClearToEnter(docker)`; replies [`ModuleReply::Bool`].
    DockIsClearToEnter { docker: ObjectId },
    /// `isClearToAdvance(docker, dockerIndex)`; replies [`ModuleReply::Bool`].
    DockIsClearToAdvance { docker: ObjectId, index: i32 },
    /// `getEnterPosition(docker, position)`; replies [`ModuleReply::Coord`] (`Done` for a `NULL` position).
    DockGetEnterPosition { docker: ObjectId, position: bool },
    /// `getDockPosition(docker, position)`; as `DockGetEnterPosition`.
    DockGetDockPosition { docker: ObjectId, position: bool },
    /// `getExitPosition(docker, position)`; as `DockGetEnterPosition`.
    DockGetExitPosition { docker: ObjectId, position: bool },
    /// `onApproachReached(docker)`.
    DockOnApproachReached { docker: ObjectId },
    /// `onEnterReached(docker)`.
    DockOnEnterReached { docker: ObjectId },
    /// `onDockReached(docker)`.
    DockOnDockReached { docker: ObjectId },
    /// `onExitReached(docker)`.
    DockOnExitReached { docker: ObjectId },
    /// `cancelDock(docker)`.
    DockCancelDock { docker: ObjectId },
    /// `isAllowPassthroughType()`; replies [`ModuleReply::Bool`].
    DockIsAllowPassthroughType,
    /// `isRallyPointAfterDockType()`; replies [`ModuleReply::Bool`].
    DockIsRallyPointAfterDockType,
    /// `action(docker, drone)`; replies [`ModuleReply::Bool`].
    DockAction { docker: ObjectId, drone: ObjectId },
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R44: the supply docks)
    /// `getSupplyTruckAIInterface() != NULL` (of an AI module); replies [`ModuleReply::Bool`].
    HasSupplyTruckAiInterface,
    /// `SupplyTruckAIInterface::loseOneBox()`; replies [`ModuleReply::Bool`].
    SupplyTruckLoseOneBox,
    /// `SupplyTruckAIInterface::gainOneBox(remainingStock)`; replies [`ModuleReply::Bool`].
    SupplyTruckGainOneBox { remaining_stock: i32 },
    /// `SupplyTruckAIInterface::getUpgradedSupplyBoost()`; replies [`ModuleReply::Int`].
    SupplyTruckGetUpgradedSupplyBoost,
    /// `SupplyTruckAIInterface::setForceWantingState(v)`.
    SupplyTruckSetForceWantingState(bool),
    /// `StealthUpdate::isTemporaryGrant()`; replies [`ModuleReply::Bool`].
    StealthIsTemporaryGrant,
    /// `SupplyWarehouseDockUpdate::setCashValue(cashValue)`.
    SupplyWarehouseSetCashValue(i32),
    /// `SupplyWarehouseDockUpdate::getBoxesStored()`; replies [`ModuleReply::Int`].
    SupplyWarehouseGetBoxesStored,
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R46: JetSlowDeathBehavior)
    /// `getCurLocomotor()->setMaxLift(lift)`.
    AiLocomotorSetMaxLift(f32),
    /// `getCurLocomotor()->setMaxTurnRate(rate)`.
    AiLocomotorSetMaxTurnRate(f32),
    /// `PhysicsBehavior::setRollRate(rate)`.
    PhysicsSetRollRate(f32),
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R47: BattleBusSlowDeathBehavior)
    /// `PhysicsBehavior::clearAcceleration()`.
    PhysicsClearAcceleration,
    /// `PhysicsBehavior::scrubVelocity2D(desiredVelocity)`.
    PhysicsScrubVelocity2D(f32),
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R48: HelicopterSlowDeathBehavior)
    /// `getCurLocomotor()->setMaxBraking(braking)`.
    AiLocomotorSetMaxBraking(f32),
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R50: PhysicsBehavior, and what it asks of the other modules)
    /// `getAI()->getCurLocomotorSetType()`; replies [`ModuleReply::Int`] (a `LocomotorSetType`).
    AiGetCurLocomotorSetType,
    /// `getAIUpdateInterface()->getIgnoredObstacleID()`; replies [`ModuleReply::Id`].
    AiGetIgnoredObstacleId,
    /// `getAIUpdateInterface()->processCollision(physics, other)` (the physics is the object's own); replies [`ModuleReply::Bool`].
    AiProcessCollision { other: ObjectId },
    /// `getAIUpdateInterface()->hasLocomotorForSurface(surfaces)`; replies [`ModuleReply::Bool`].
    AiHasLocomotorForSurface(u32),
    /// `ProjectileUpdateInterface::projectileHandleCollision(other)` (`INVALID_ID` for `NULL`); replies [`ModuleReply::Bool`].
    ProjectileHandleCollision { other: ObjectId },
    /// `PhysicsBehavior::setYawRate(yaw)`.
    PhysicsSetYawRate(f32),
    /// `PhysicsBehavior::scrubVelocityZ(desiredVelocity)`.
    PhysicsScrubVelocityZ(f32),
    /// `PhysicsBehavior::addVelocityTo(&vel)`.
    PhysicsAddVelocityTo(Coord3D),
    /// `PhysicsBehavior::setTurning(turning)` (`TURN_NEGATIVE` -1, `TURN_NONE` 0, `TURN_POSITIVE` 1).
    PhysicsSetTurning(i32),
    /// `PhysicsBehavior::setIgnoreCollisionsWith(obj)` (`INVALID_ID` for `NULL`).
    PhysicsSetIgnoreCollisionsWith(ObjectId),
    /// `PhysicsBehavior::isIgnoringCollisionsWith(id)`; replies [`ModuleReply::Bool`].
    PhysicsIsIgnoringCollisionsWith(ObjectId),
    /// `PhysicsBehavior::setStickToGround(on)`.
    PhysicsSetStickToGround(bool),
    /// `PhysicsBehavior::setImmuneToFallingDamage(on)`.
    PhysicsSetImmuneToFallingDamage(bool),
    /// `PhysicsBehavior::setAllowAirborneFriction(on)`.
    PhysicsSetAllowAirborneFriction(bool),
    /// `PhysicsBehavior::setAllowCollideForce(on)`.
    PhysicsSetAllowCollideForce(bool),
    /// `PhysicsBehavior::setBounceSound(sound)` (the event's name; `None` for `NULL`).
    PhysicsSetBounceSound(Option<Vec<u8>>),
    /// `PhysicsBehavior::getIsStunned()`; replies [`ModuleReply::Bool`].
    PhysicsGetIsStunned,
    /// `PhysicsBehavior::getIsInFreeFall()`; replies [`ModuleReply::Bool`].
    PhysicsGetIsInFreeFall,
    /// `PhysicsBehavior::isMotive()`; replies [`ModuleReply::Bool`].
    PhysicsIsMotive,
    /// `PhysicsBehavior::getForwardSpeed2D()`; replies [`ModuleReply::Real`].
    PhysicsGetForwardSpeed2D,
    /// `PhysicsBehavior::getForwardSpeed3D()`; replies [`ModuleReply::Real`].
    PhysicsGetForwardSpeed3D,
    /// `PhysicsBehavior::getAcceleration()` (last frame's); replies [`ModuleReply::Coord`].
    PhysicsGetAcceleration,
    /// `PhysicsBehavior::getCurrentOverlap()`; replies [`ModuleReply::Id`].
    PhysicsGetCurrentOverlap,
    /// `PhysicsBehavior::getPreviousOverlap()`; replies [`ModuleReply::Id`].
    PhysicsGetPreviousOverlap,
    /// `PhysicsBehavior::addOverlap(obj)`.
    PhysicsAddOverlap(ObjectId),
    /// `PhysicsBehavior::setMass(mass)`.
    PhysicsSetMass(f32),
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R38: `BridgeTowerBehaviorInterface`, which `TerrainLogic`'s `Bridge::createTower` drives)
    /// `setBridge(bridge)` (the bridge's id; one that is no object is `setBridge(NULL)`).
    BridgeTowerSetBridge(ObjectId),
    /// `getBridgeID()`; replies [`ModuleReply::Id`].
    BridgeTowerGetBridgeId,
    /// `setTowerType(type)` (`BridgeTowerType` as its value).
    BridgeTowerSetTowerType(i32),
    // --- end p1w13-sonnet ---
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
    /// batch R72: `SpecialPowerUpdateInterface::setSpecialPowerOverridableDestination(loc)`.
    SetSpecialPowerOverridableDestination(Coord3D),
    /// batch R73: `getSlavedUpdateInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasSlavedUpdateInterface,
    /// batch R73: `SlavedUpdateInterface::isSelfTasking()`; replies [`ModuleReply::Bool`].
    SlavedIsSelfTasking,
    /// batch R73: `AIUpdateInterface::aiForceAttackObject(victim, maxShotsToFire, cmdSource)`.
    AiForceAttackObject { victim: ObjectId, max_shots: i32, command_source: i32 },
    /// batch R73: `SpawnBehaviorInterface::orderSlavesToAttackTarget(target, maxShotsToFire, cmdSource)`.
    SpawnOrderSlavesToAttackTarget { target: ObjectId, max_shots: i32, command_source: i32 },
    /// batch R73: `SpawnBehaviorInterface::orderSlavesToAttackPosition(pos, maxShotsToFire, cmdSource)`.
    SpawnOrderSlavesToAttackPosition { pos: Coord3D, max_shots: i32, command_source: i32 },
    /// batch R73: `SpawnBehaviorInterface::orderSlavesToGoIdle(cmdSource)`.
    SpawnOrderSlavesToGoIdle { command_source: i32 },
    /// batch R73: `SpawnBehaviorInterface::getCanAnySlavesAttackSpecificTarget(attackType, target, cmdSource)`; replies [`ModuleReply::Int`]
    /// (a `CanAttackResult`).
    SpawnGetCanAnySlavesAttackSpecificTarget { attack_type: i32, target: ObjectId, command_source: i32 },
    /// batch R73: `SpawnBehaviorInterface::giveSlavesStealthUpgrade(grantStealth)`.
    SpawnGiveSlavesStealthUpgrade(bool),
    /// batch R73: `SpawnBehaviorInterface::doSlavesHaveFreedom()`; replies [`ModuleReply::Bool`].
    SpawnDoSlavesHaveFreedom,
    /// batch R73: `SpawnBehavior::stopSpawning()`, `startSpawning()`.
    SpawnStopSpawning,
    SpawnStartSpawning,
    /// batch R73: `StealthUpdate::allowedToStealth(stealthOwner)`; replies [`ModuleReply::Bool`].
    StealthAllowedToStealth { owner: ObjectId },
    /// batch R74: `ProductionUpdateInterface::setHoldDoorOpen(exitDoor, holdIt)`.
    SetHoldDoorOpen { door: i32, hold: bool },
    /// batch R76: `ProductionUpdateInterface`: `canQueueCreateUnit(unitType)` and `canQueueUpgrade(upgrade)` (a `CanMakeType`),
    /// `requestUniqueUnitID()`, `queueUpgrade(upgrade)`, `cancelUpgrade(upgrade)`, `isUpgradeInQueue(upgrade)`, `countUnitTypeInQueue(unitType)`,
    /// `queueCreateUnit(unitType, productionID)`, `cancelUnitCreate(productionID)`, `cancelAllUnitsOfType(unitType)`, `getProductionCount()`
    /// and `setSpecialPowerConstructionCommandButton(button)` (by name). A template is its id, an upgrade its name key; `None` is `NULL`.
    ProductionCanQueueCreateUnit { template: u16 },
    ProductionCanQueueUpgrade { upgrade: Option<u32> },
    ProductionRequestUniqueUnitId,
    ProductionQueueUpgrade { upgrade: Option<u32> },
    ProductionCancelUpgrade { upgrade: Option<u32> },
    ProductionIsUpgradeInQueue { upgrade: Option<u32> },
    ProductionCountUnitTypeInQueue { template: Option<u16> },
    ProductionQueueCreateUnit { template: Option<u16>, production_id: i32 },
    ProductionCancelUnitCreate { production_id: i32 },
    ProductionCancelAllUnitsOfType { template: Option<u16> },
    ProductionGetCount,
    ProductionSetSpecialPowerConstructionCommandButton(Option<Vec<u8>>),
    /// batch R74: `JetAIUpdate::friend_getParkingOffset()`; replies [`ModuleReply::Real`].
    JetGetParkingOffset,
    /// batch R74: `JetAIUpdate::friend_isTakeoffOrLandingInProgress()`; replies [`ModuleReply::Bool`].
    JetIsTakeoffOrLandingInProgress,
    /// batch R74: `ParkingPlaceBehaviorInterface::shouldReserveDoorWhenQueued(thing)`; replies [`ModuleReply::Bool`].
    ParkingPlaceShouldReserveDoorWhenQueued { template: u16 },
    /// batch R74: `ParkingPlaceBehaviorInterface::hasAvailableSpaceFor(thing)`; replies [`ModuleReply::Bool`].
    ParkingPlaceHasAvailableSpaceFor { template: u16 },
    /// batch R74: `ParkingPlaceBehaviorInterface::getSpaceIndex(id)`; replies [`ModuleReply::Int`].
    ParkingPlaceGetSpaceIndex { id: ObjectId },
    /// batch R74: `ParkingPlaceBehaviorInterface::reserveSpace(id, parkingOffset, want_info ? &info : NULL)`; replies [`ModuleReply::PPInfo`].
    ParkingPlaceReserveSpace { id: ObjectId, parking_offset: f32, want_info: bool },
    /// batch R74: `ParkingPlaceBehaviorInterface::releaseSpace(id)`.
    ParkingPlaceReleaseSpace { id: ObjectId },
    /// batch R74: `ParkingPlaceBehaviorInterface::reserveRunway(id, forLanding)`; replies [`ModuleReply::Bool`].
    ParkingPlaceReserveRunway { id: ObjectId, for_landing: bool },
    /// batch R74: `ParkingPlaceBehaviorInterface::calcPPInfo(id, &info)` with `info` as given; replies [`ModuleReply::PPInfo`].
    ParkingPlaceCalcPPInfo { id: ObjectId, info: PPInfo },
    /// batch R74: `ParkingPlaceBehaviorInterface::releaseRunway(id)`.
    ParkingPlaceReleaseRunway { id: ObjectId },
    /// batch R74: `ParkingPlaceBehaviorInterface::getRunwayCount()`; replies [`ModuleReply::Int`].
    ParkingPlaceGetRunwayCount,
    /// batch R74: `ParkingPlaceBehaviorInterface::getRunwayReservation(r, type)`; replies [`ModuleReply::Id`].
    ParkingPlaceGetRunwayReservation { runway: i32, kind: i32 },
    /// batch R74: `ParkingPlaceBehaviorInterface::transferRunwayReservationToNextInLineForTakeoff(id)`.
    ParkingPlaceTransferRunwayReservation { id: ObjectId },
    /// batch R74: `ParkingPlaceBehaviorInterface::getApproachHeight()`; replies [`ModuleReply::Real`].
    ParkingPlaceGetApproachHeight,
    /// batch R74: `ParkingPlaceBehaviorInterface::setHealee(healee, add)`.
    ParkingPlaceSetHealee { id: ObjectId, add: bool },
    /// batch R74: `ParkingPlaceBehaviorInterface::killAllParkedUnits()`.
    ParkingPlaceKillAllParkedUnits,
    /// batch R74: `ParkingPlaceBehaviorInterface::calcBestParkingAssignment(id, &pos, ...)`; replies [`ModuleReply::Bool`].
    ParkingPlaceCalcBestParkingAssignment,
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
    /// batch R52: `CountermeasuresBehaviorInterface::reloadCountermeasures()`.
    ReloadCountermeasures,
    /// batch R52: `ProjectileUpdateInterface::setFramesTillCountermeasureDiversionOccurs(frames)`.
    ProjectileSetFramesTillCountermeasureDiversion(u32),
    /// batch R52: the receiving end of `PhysicsBehavior::transferVelocityTo(that)`: `that` gains the velocity and recomputes its speed.
    PhysicsTransferVelocityFrom(Coord3D),
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
    /// `getSubdualDamageHealRate()` of the body; replies [`ModuleReply::Int`] (`SubdualDamageHelper`).
    GetSubdualDamageHealRate,
    /// `getSubdualDamageHealAmount()` of the body; replies [`ModuleReply::Real`].
    GetSubdualDamageHealAmount,
    /// `hasAnySubdualDamage()` of the body; replies [`ModuleReply::Bool`].
    HasAnySubdualDamage,
    /// `SpecialPowerModuleInterface::isReady()`; replies [`ModuleReply::Bool`].
    IsReady,
    /// `CleanupHazardUpdate::setCleanupAreaParameters(loc, range)` (`CleanupAreaPower`).
    SetCleanupAreaParameters { loc: Coord3D, range: f32 },
    /// `SpyVisionUpdate::activateSpyVision(duration)` (`SpyVisionSpecialPower`).
    ActivateSpyVision { duration: u32 },
    /// `AutoDepositUpdate::awardInitialCaptureBonus(player)` (`Player::becomingTeamMember`), the player as an index (negative for `NULL`).
    AwardInitialCaptureBonus { player: i32 },
    /// `SpyVisionUpdate::setDisabledUntilFrame(frame)` (the sabotage crates).
    SetSpyVisionDisabledUntilFrame { frame: u32 },
    /// `getPercentReady()`; replies [`ModuleReply::Real`].
    GetPercentReady,
    /// `getReadyFrame() const`; replies [`ModuleReply::Int`] (the frame as an `Int`).
    GetReadyFrame,
    /// `getRequiredScience()` (the template's); replies [`ModuleReply::Int`].
    GetRequiredScience,
    /// `setReadyFrame(frame)`.
    SetReadyFrame { frame: u32 },
    /// `markSpecialPowerTriggered(location)` (`None` is `NULL`).
    MarkSpecialPowerTriggered { location: Option<Coord3D> },
    /// `startPowerRecharge()`.
    StartPowerRecharge,
    /// `isScriptOnly()`; replies [`ModuleReply::Bool`].
    IsScriptOnly,
    /// `SpecialPowerUpdateInterface::doesSpecialPowerUpdatePassScienceTest()`; replies [`ModuleReply::Bool`] (unhandled: no such interface).
    DoesSpecialPowerUpdatePassScienceTest,
    /// `SpecialPowerUpdateInterface::initiateIntentToDoSpecialPower(template, targetObj, targetPos, way, commandOptions)`, the template by id
    /// and the waypoint by id; replies [`ModuleReply::Bool`] (unhandled: no such interface).
    InitiateIntentToDoSpecialPower { template: u32, target_object: ObjectId, target_position: Option<Coord3D>, waypoint: u32, command_options: u32 },
    /// batch W1b: `ContainModuleInterface::calcBestGarrisonPosition(&goalPos, &targetPos)`; replies [`ModuleReply::Coord`] with the goal
    /// position, anything else for none.
    CalcBestGarrisonPosition { target_pos: Coord3D },
    /// batch W1b: `StealthUpdate::isDisguised()` and `getDisguisedPlayerIndex()`; replies [`ModuleReply::Int`] when disguised.
    StealthGetDisguisedPlayerIndex,
    /// batch W1b: `SpawnBehaviorInterface::getCanAnySlavesUseWeaponAgainstTarget(attackType, victim, pos, commandSource)`; replies
    /// [`ModuleReply::Int`] (`CanAttackResult`).
    SpawnCanAnySlavesUseWeaponAgainstTarget { attack_type: i32, victim: ObjectId, pos: Option<Coord3D>, command_source: i32 },
    /// batch W1b: `AIUpdateInterface::getSneakyTargetingOffset(&offset)`; replies [`ModuleReply::Coord`] when sneaky.
    AiGetSneakyTargetingOffset,
    /// batch W1b: `AIUpdateInterface::isAircraftThatAdjustsDestination()`; replies [`ModuleReply::Bool`].
    AiIsAircraftThatAdjustsDestination,
    /// batch W1b: `getAssaultTransportAIInterface()->beginAssault(victim)` (nothing without the interface).
    AiBeginAssault { victim: ObjectId },
    /// batch W1b: `getWhichTurretForWeaponSlot(slot, &angle, &pitch)`; replies [`ModuleReply::Turret`].
    AiGetWhichTurretForWeaponSlot { slot: i32 },
    /// batch W1b: `isWeaponSlotOnTurretAndAimingAtTarget(slot, victim)`; replies [`ModuleReply::Bool`].
    AiIsWeaponSlotOnTurretAndAimingAtTarget { slot: i32, victim: ObjectId },
    /// batch W1b: this module is a `SpecialPowerCompletionDie` (`findSpecialPowerCompletionDie`); replies [`ModuleReply::Bool`].
    IsSpecialPowerCompletionDie,
    /// batch W1b: `SpecialPowerCompletionDie::notifyScriptEngine()`.
    SpecialPowerCompletionDieNotifyScriptEngine,
    /// batch W1b: `ProjectileUpdateInterface::projectileLaunchAtObjectOrPosition(victim, &dest, launcher, wslot, barrel, template, exhaust)`.
    ProjectileLaunchAtObjectOrPosition {
        victim: ObjectId,
        destination: Coord3D,
        source: ObjectId,
        wslot: i32,
        barrel: i32,
        template: usize,
        exhaust: crate::logic::weapon::FxName,
    },
    /// batch W1b: `ProjectileUpdateInterface::projectileGetLauncherID()`; replies [`ModuleReply::Id`].
    ProjectileGetLauncherId,
    /// batch W1b: `getLandMineInterface() != NULL`; replies [`ModuleReply::Bool`].
    HasLandMineInterface,
    /// batch W1b: `ParkingPlaceBehaviorInterface::hasReservedSpace(id)`; replies [`ModuleReply::Bool`].
    ParkingPlaceHasReservedSpace { intended: ObjectId },
    /// batch R61: `RailedTransportDockUpdateInterface::isLoadingOrUnloading()`; replies [`ModuleReply::Bool`] (unhandled: no such interface).
    RailedTransportDockIsLoadingOrUnloading,
    /// batch R61: `RailedTransportDockUpdateInterface::unloadAll()`.
    RailedTransportDockUnloadAll,
    /// batch R61: `RailedTransportDockUpdateInterface::unloadSingleObject(obj)`.
    RailedTransportDockUnloadSingleObject(ObjectId),
    // --- end p1w11-sonnet ---
    // --- p1w13-sonnet --- (batch R80: RailroadBehavior; a train's cars call each other's)
    /// `PhysicsBehavior::getBounceSound()`; replies [`ModuleReply::Sound`] (the event's name, `None` for `NULL`).
    PhysicsGetBounceSound,
    /// `RailroadBehavior::getPulled(&info)`; replies [`ModuleReply::Bool`]: whether the car read `info` (it has a track), when the caller's
    /// `previousWaypoint` becomes its `currentWaypoint`.
    RailroadGetPulled(crate::logic::modules::railroad_behavior::PullInfo),
    /// `RailroadBehavior::hitchNewCarriagebyTemplate(locoID, list, iter, track)`: `next` the index of `iter` in `names`.
    RailroadHitchByTemplate {
        locomotive: ObjectId,
        names: Vec<Vec<u8>>,
        next: usize,
        track: std::sync::Arc<crate::logic::modules::railroad_behavior::TrainTrack>,
    },
    /// `RailroadBehavior::hitchNewCarriagebyProximity(locoID, track)`.
    RailroadHitchByProximity { locomotive: ObjectId, track: std::sync::Arc<crate::logic::modules::railroad_behavior::TrainTrack> },
    /// `RailroadBehavior::disembark()`.
    RailroadDisembark,
    /// `RailroadBehavior::makeAWallOutOfThisTrain(on)`.
    RailroadMakeAWallOutOfThisTrain(bool),
    /// `RailroadBehavior::destroyTheWholeTrainNow()`.
    RailroadDestroyTheWholeTrainNow,
    /// `RailroadBehavior::hasEverBeenHitched()`; replies [`ModuleReply::Bool`].
    RailroadHasEverBeenHitched,
    /// `RailroadBehavior::setHeld(held)`.
    RailroadSetHeld(bool),
    // --- end p1w13-sonnet ---
}

/// batch R74: `ParkingPlaceBehaviorInterface::PPInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PPInfo {
    pub parking_space: Coord3D,
    pub parking_orientation: f32,
    pub runway_prep: Coord3D,
    pub runway_start: Coord3D,
    pub runway_end: Coord3D,
    pub runway_exit: Coord3D,
    pub runway_landing_start: Coord3D,
    pub runway_landing_end: Coord3D,
    pub runway_approach: Coord3D,
    pub hangar_internal: Coord3D,
    pub runway_takeoff_dist: f32,
    pub hangar_internal_orient: f32,
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
    // --- p1w13-sonnet ---
    /// A `Coord3D` (a velocity).
    Coord(Coord3D),
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R43)
    /// `reserveApproachPosition` / `advanceApproachPosition`: the answer, and the position and index (the caller's when not written).
    Approach { ok: bool, position: Option<Coord3D>, index: i32 },
    /// batch W1b: `getWhichTurretForWeaponSlot(slot, &angle, &pitch)`: the turret and its two out-parameters.
    Turret { turret: i32, angle: f32, pitch: f32 },
    /// batch R74: `reserveSpace` / `calcPPInfo`: the answer, and the `PPInfo` written (`None`: the caller's left as it was).
    PPInfo { ok: bool, info: Option<PPInfo> },
    /// batch R80: an audio event's name (`None`: `NULL`).
    Sound(Option<Vec<u8>>),
    // --- end p1w13-sonnet ---
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

    /// The answer as a `Coord3D`, `None` for anything else (a `NULL` position). (p1w13-sonnet, batch R12)
    pub fn as_coord_opt(&self) -> Option<Coord3D> {
        match self {
            ModuleReply::Coord(c) => Some(*c),
            _ => None,
        }
    }

    /// The answer as a `Coord3D`, the zero vector for anything else.
    pub fn as_coord(&self) -> Coord3D {
        match self {
            ModuleReply::Coord(c) => *c,
            _ => Coord3D { x: 0.0, y: 0.0, z: 0.0 },
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
