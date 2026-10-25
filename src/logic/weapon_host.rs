// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code). It names, for each method, the
// upstream call it replaces. The code it serves is translated from (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Weapon.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/WeaponSet.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! [`WeaponHost`]: everything `Weapon`, `WeaponTemplate::fireWeaponTemplate`, `dealDamageInternal` and `WeaponSet`
//! reach outside themselves: the objects (by id; `None` is upstream's `NULL`), the players of those objects, the
//! partition manager, the terrain, the game logic, the random numbers, the weapon store, the AI, the module
//! interfaces, and the client.
//!
//! # The shape
//!
//! The same shape as [`crate::logic::firing_tracker::FiringHost`]: the weapon code is plain functions and data
//! (`logic::weapon_fire`, `logic::weapon_set_state`), and the world behind it is a generic host. **Each method is
//! called at the point upstream makes the call it replaces, in upstream's order**, so a host that records its
//! calls sees upstream's order of effects. The production host is `World` (the objects, the partition manager,
//! the terrain, the players and teams); the differential test's host is a table (`ScenarioHost` in
//! `tests/differential_weapon_fire.rs`) answering from a configuration, which the C++ stand-ins answer from too.
//!
//! # Client calls
//!
//! The client is reached only by [`WeaponFx`] events and by the reads the simulation needs from client model
//! data (`barrel_count`, `projectile_launch_offset`): those are `Drawable` questions upstream, the answer comes
//! from model files that every peer loads alike, and logic uses them (the current barrel is lockstep state, the
//! launch position is where a projectile begins). `handleWeaponFireFX`'s return value (`handled`) only decides
//! whether the client falls back to playing the FX list at the drawable, which is the client's own business, so
//! the pair of calls is one event.
//!
//! # Pointers that alias an object's position
//!
//! Upstream passes `const Coord3D*` that often point into a live object (`victimObj->getPosition()`). The
//! translation reads the position when upstream first dereferences it and carries the value. This differs from
//! upstream only if the object moves between the read and a later dereference, which the calls in between (an
//! OCL, creating a projectile, launching it) do not do to the victim or the source.

use crate::common::base_type::{Coord3D, Region3D};
use crate::common::game_common::Relationship;
use crate::common::game_type::{ObjectId, PathfindLayer};
use crate::common::geometry::GeometryInfo;
use crate::common::kind_of::{KindOfMaskType, KindOfType};
use crate::common::object_status_types::ObjectStatusTypes;
use crate::common::random_value::RandomValues;
use crate::logic::damage::DamageInfo;
use crate::logic::damage::DamageInfoInput;
use crate::common::name_key_generator::NameKeyType;
use crate::logic::weapon::{FxName, WeaponError, WeaponStore};
use crate::logic::weapon_set::WeaponTemplateSet;
use crate::logic::weapon_set_flags::WeaponSetFlags;
use crate::logic::weapon_bonus::{WeaponBonus, WeaponBonusSet};
use crate::logic::weapon_types::WeaponStatus;
use crate::ww::matrix3d::Matrix3D;

/// What `DistanceCalculationType` the weapon code asks the partition manager for (`FROM_CENTER_2D` ...);
/// [`crate::logic::world::partition::DistanceCalculationType`] itself.
pub use crate::logic::world::partition::DistanceCalculationType;

pub use crate::common::game_common::{
    is_forced_attack, AbleToAttackType, CommandSourceType, ATTACK_CONTINUED_TARGET, ATTACK_CONTINUED_TARGET_FORCED, ATTACK_NEW_TARGET,
    ATTACK_NEW_TARGET_FORCED, ATTACK_TUNNEL_NETWORK_GUARD,
};

/// `enum CanAttackResult`, worst to best ("these must be done this way now").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum CanAttackResult {
    /// `ATTACKRESULT_NOT_POSSIBLE`: can't possibly attack the target.
    NotPossible = 0,
    /// `ATTACKRESULT_INVALID_SHOT`: not a clear shot.
    InvalidShot = 1,
    /// `ATTACKRESULT_POSSIBLE_AFTER_MOVING`: possible, after moving closer.
    PossibleAfterMoving = 2,
    /// `ATTACKRESULT_POSSIBLE`: possible now.
    Possible = 3,
}

/// A client effect a weapon asks for (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub enum WeaponFx {
    /// `fireWeaponTemplate`'s `Drawable::handleWeaponFireFX(wslot, barrel, fx, speed, recoilAngle, recoilDir,
    /// &targetPos, radius)`, and, when the drawable did not handle it and `fx` is not `NULL`,
    /// `FXList::doFXPos(fx, where, ...)` with `where` the target for a contact weapon and the drawable otherwise.
    /// Not sent for a stealthed unit the local player cannot see (upstream pretends it was handled).
    Fire {
        /// The firing object.
        source: ObjectId,
        /// `wslot`.
        weapon_slot: i32,
        /// `specificBarrelToUse`.
        barrel: i32,
        /// The `FXList` (`NULL` = `None`).
        fx: FxName,
        /// `getWeaponSpeed()`.
        speed: f32,
        /// `getWeaponRecoilAmount()`.
        recoil_amount: f32,
        /// The recoil direction (`atan2` of the shot, or 0).
        recoil_dir: f32,
        /// The target point.
        target: Coord3D,
        /// `getPrimaryDamageRadius(bonus)`.
        radius: f32,
        /// `isContactWeapon()`.
        contact_weapon: bool,
    },
    /// `FXList::doFXPos(fireFX, pos, transform, 0, pos, 0)` where a `DAMAGE_DISARM` weapon disarms or douses a mine.
    Disarm {
        /// The `FXList`.
        fx: FxName,
        /// The mine the FX is at (its position and transform).
        at: ObjectId,
    },
}

/// What `Drawable::getProjectileLaunchOffset` answers: the launch bone's transform and the turret pivots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaunchOffset {
    /// `attachTransform`.
    pub attach_transform: Matrix3D,
    /// `turretRotPos`.
    pub turret_rot_pos: Coord3D,
    /// `turretPitchPos`.
    pub turret_pitch_pos: Coord3D,
}

/// What a `ContainModuleInterface` of a victim says about its occupants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainFacts {
    /// `getContainCount()`.
    pub count: i32,
    /// `isGarrisonable()`.
    pub garrisonable: bool,
    /// `isImmuneToClearBuildingAttacks()`.
    pub immune_to_clear_building_attacks: bool,
}

/// An item of a container's occupant list that can attack (`getContainedItemsList`).
pub type ContainedItems = Vec<ObjectId>;

/// `WeaponTemplateSet*` (a pointer into a thing template's list of weapon sets): the thing template's id and the
/// set's position in that template's list. Two references are equal when upstream's pointers are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WeaponTemplateSetRef {
    /// The id of the thing template that owns the set.
    pub thing_template: u16,
    /// The position of the set in that template's `WeaponTemplateSet` list.
    pub index: usize,
}

/// What `Weapon::xfer` and `WeaponSet::xfer` look up on a load (or ask on a save): the thing factory, the thing
/// templates' weapon sets and the weapon store. A supertrait of [`WeaponHost`]; the object's own `xfer` can pass
/// anything that answers these.
pub trait WeaponXferLookups {
    /// `TheThingFactory->findTemplate(name)`: the template id, `None` for `NULL`.
    fn find_thing_template(&self, name: &[u8]) -> Option<u16>;
    /// `tt->getName()`.
    fn thing_template_name(&self, id: u16) -> Vec<u8>;
    /// `tt->findWeaponTemplateSet(flags)`: it fills the template's match cache, hence `&mut`.
    fn find_weapon_template_set_by_flags(&mut self, thing_template: u16, flags: &WeaponSetFlags) -> Option<WeaponTemplateSetRef>;
    /// The weapon template set `r` points at.
    fn weapon_template_set(&self, r: WeaponTemplateSetRef) -> &WeaponTemplateSet;
    /// The name and store position of every weapon template, in store order (what `findWeaponTemplate(name)`
    /// searches): needed to load a weapon, whose saved form names its template.
    fn weapon_template_names(&self) -> Vec<(Vec<u8>, usize)>;
    /// `TheWeaponStore->findWeaponTemplateByNameKey(key)` as a store position.
    fn weapon_template_index(&self, key: NameKeyType) -> Option<usize>;
    /// `getName()` of the weapon template at this store position.
    fn weapon_template_name(&self, index: usize) -> Vec<u8>;
}

/// Everything the weapon code reaches outside itself. See the module docs.
pub trait WeaponHost: WeaponXferLookups {
    // ---- TheGameLogic, TheGlobalData, TheWeaponStore, the random numbers --------------------------------------

    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `TheGameLogic->findObjectByID(id) != NULL`: a destroyed object is still found until the end of the frame.
    fn object_exists(&self, o: ObjectId) -> bool;
    /// `TheWeaponStore`.
    fn store(&self) -> &WeaponStore;
    /// `TheWeaponStore`, to change (`setDelayedDamage`, the delayed list, a template's historic damage).
    fn store_mut(&mut self) -> &mut WeaponStore;
    /// `GameLogicRandomValue(lo, hi)`.
    fn random_int(&mut self, lo: i32, hi: i32) -> i32;
    /// `GameLogicRandomValueReal(lo, hi)`.
    fn random_real(&mut self, lo: f32, hi: f32) -> f32;
    /// `template.getDelayBetweenShots(bonus)`: it draws from the logic random numbers when the delay is a range,
    /// so the template and the generator are borrowed together by the host.
    fn delay_between_shots(&mut self, template: usize, bonus: &WeaponBonus) -> i32;
    /// `TheGlobalData->m_weaponBonusSet` (`NULL` = `None`).
    fn global_weapon_bonus_set(&self) -> Option<&WeaponBonusSet>;
    /// `TheGlobalData->m_historicDamageLimit`.
    fn historic_damage_limit(&self) -> u32;
    /// The random numbers themselves, for a host that wants to hand them to a closure.
    fn rng(&mut self) -> &mut RandomValues;

    // ---- an Object's own state -----------------------------------------------------------------------------------

    /// `getPosition()`.
    fn position(&self, o: ObjectId) -> Coord3D;
    /// `getOrientation()`.
    fn orientation(&self, o: ObjectId) -> f32;
    /// `getTransformMatrix()`.
    fn transform_matrix(&self, o: ObjectId) -> Matrix3D;
    /// `getGeometryInfo()`.
    fn geometry(&self, o: ObjectId) -> GeometryInfo;
    /// `isKindOf(kind)`.
    fn is_kind_of(&self, o: ObjectId, kind: KindOfType) -> bool;
    /// `isKindOfMulti(mustBeSet, mustBeClear)`: every bit of `must_be_set` is set and no bit of `must_be_clear`.
    fn is_kind_of_multi(&self, o: ObjectId, must_be_set: &KindOfMaskType, must_be_clear: &KindOfMaskType) -> bool;
    /// `testStatus(status)`.
    fn test_status(&self, o: ObjectId, status: ObjectStatusTypes) -> bool;
    /// `testScriptStatusBit(bit)`.
    fn test_script_status_bit(&self, o: ObjectId, bit: u32) -> bool;
    /// `getVeterancyLevel()`.
    fn veterancy_level(&self, o: ObjectId) -> i32;
    /// `getDrawable() != NULL`.
    fn has_drawable(&self, o: ObjectId) -> bool;
    /// `isLocallyControlled()`.
    fn is_locally_controlled(&mut self, o: ObjectId) -> bool;
    /// `getWeaponBonusCondition()`.
    fn weapon_bonus_condition(&self, o: ObjectId) -> u32;
    /// `getContainedBy()`.
    fn contained_by(&self, o: ObjectId) -> Option<ObjectId>;
    /// `getContainedBy()->getContain()` when it `isWeaponBonusPassedToPassengers()`: its
    /// `getWeaponBonusPassedToPassengers()`. `None` when there is no container, no contain module, or it passes
    /// nothing.
    fn container_passed_weapon_bonus(&mut self, container: ObjectId) -> Option<u32>;
    /// `isReloadTimeShared()` (the weapon set's).
    fn is_reload_time_shared(&self, o: ObjectId) -> bool;
    /// `for each slot: weapon = getWeaponInWeaponSlot(slot); if (weapon) { weapon->setPossibleNextShotFrame(frame);
    /// weapon->setStatus(status); }` on `o`'s weapon set. The weapon calling this is skipped or included alike:
    /// it has just set the same two values on itself.
    fn apply_shared_reload(&mut self, o: ObjectId, next_shot_frame: u32, status: WeaponStatus);
    /// `getNumConsecutiveShotsFiredAtTarget(victim)`.
    fn num_consecutive_shots_fired_at_target(&mut self, o: ObjectId, victim: Option<ObjectId>) -> i32;
    /// `isAboveTerrain()`.
    fn is_above_terrain(&mut self, o: ObjectId) -> bool;
    /// `isAirborneTarget()`.
    fn is_airborne_target(&mut self, o: ObjectId) -> bool;
    /// `isSignificantlyAboveTerrain()`.
    fn is_significantly_above_terrain(&mut self, o: ObjectId) -> bool;
    /// `getLayer()`.
    fn layer(&self, o: ObjectId) -> PathfindLayer;
    /// `getProducerID()`.
    fn producer_id(&self, o: ObjectId) -> ObjectId;
    /// `a->getRelationship(b)`.
    fn relationship(&self, a: ObjectId, b: ObjectId) -> Relationship;
    /// `a->getTemplate()->isEquivalentTo(b->getTemplate())`.
    fn is_equivalent_template(&self, a: ObjectId, b: ObjectId) -> bool;
    /// `getTemplate()->getFenceWidth()`.
    fn fence_width(&self, o: ObjectId) -> f32;
    /// `isEffectivelyDead()`.
    fn is_effectively_dead(&self, o: ObjectId) -> bool;
    /// `isDestroyed()`.
    fn is_destroyed(&self, o: ObjectId) -> bool;
    /// `isAbleToAttack()`.
    fn is_able_to_attack(&mut self, o: ObjectId) -> bool;
    /// `o->getTeam()->getRelationship(player->getDefaultTeam())` for the player with this index.
    fn team_relationship_to_player_index(&self, o: ObjectId, player_index: i32) -> Relationship;
    /// `obj->getTemplate()->findWeaponTemplateSet(obj->getWeaponSetFlags())`: it fills the template's match cache,
    /// hence `&mut`. `None` for `NULL`.
    fn find_weapon_template_set(&mut self, o: ObjectId) -> Option<WeaponTemplateSetRef>;
    /// `estimateDamage(input)` of the body (the armor and the bonuses of the victim).
    fn estimate_damage(&mut self, o: ObjectId, input: &DamageInfoInput) -> f32;
    /// `attemptDamage(&info)`: the body's damage (it fills `info.out`).
    fn attempt_damage(&mut self, o: ObjectId, info: &mut DamageInfo);
    /// `TheGameLogic->destroyObject(o)`.
    fn destroy_object(&mut self, o: ObjectId);

    // ---- an Object's player ---------------------------------------------------------------------------------------

    /// `getControllingPlayer()`, as the player's index (identity only); `None` = `NULL`.
    fn controlling_player(&self, o: ObjectId) -> Option<i32>;
    /// `getControllingPlayer()->getPlayerMask()`; `None` when there is no controlling player.
    fn controlling_player_mask(&self, o: ObjectId) -> Option<crate::common::game_common::PlayerMaskType>;
    /// `getControllingPlayer()->getNumBattlePlansActive()`.
    fn controlling_player_battle_plans(&self, o: ObjectId) -> i32;
    /// `getControllingPlayer()->applyBattlePlanBonusesForObject(projectile)`.
    fn apply_battle_plan_bonuses(&mut self, source: ObjectId, projectile: ObjectId);
    /// `getControllingPlayer()->getAcademyStats()->recordMineCleared()`.
    fn record_mine_cleared(&mut self, o: ObjectId);
    /// `getControllingPlayer()->iterateObjects(...)`: the player's objects in the order it visits them; empty
    /// when there is no controlling player.
    fn controlling_player_objects(&self, o: ObjectId) -> Vec<ObjectId>;

    // ---- modules of an Object ---------------------------------------------------------------------------------------

    /// `getContain()` facts (`None` = no contain module).
    fn contain_facts(&mut self, o: ObjectId) -> Option<ContainFacts>;
    /// `getContain()->isEnclosingContainerFor(inner)`; `false` when there is no contain module.
    fn contain_is_enclosing_container_for(&mut self, container: ObjectId, inner: ObjectId) -> bool;
    /// `getContain()->isGarrisonable()`.
    fn contain_is_garrisonable(&mut self, container: ObjectId) -> bool;
    /// `getContain()->calcBestGarrisonPosition(&goalPos, &targetPos)`: the goal position, or `None`.
    fn contain_calc_best_garrison_position(&mut self, container: ObjectId, target_pos: &Coord3D) -> Option<Coord3D>;
    /// `getContain()->isPassengerAllowedToFire()`; `false` when there is no contain module.
    fn contain_is_passenger_allowed_to_fire(&mut self, o: ObjectId) -> bool;
    /// `getContain()->getContainedItemsList()` (`None` = the pointer is `NULL`).
    fn contain_items(&mut self, o: ObjectId) -> Option<ContainedItems>;
    /// `getContain()->getApparentControllingPlayer(observer)`: that player's index, `None` for `NULL`.
    fn contain_apparent_controlling_player(&mut self, o: ObjectId, observer: Option<i32>) -> Option<i32>;
    /// `getContain() != NULL`.
    fn has_contain(&mut self, o: ObjectId) -> bool;
    /// `getStealth()`'s `isDisguised()` and `getDisguisedPlayerIndex()`: `None` when there is no stealth update or
    /// it is not disguised.
    fn stealth_disguised_player_index(&mut self, o: ObjectId) -> Option<i32>;
    /// `ThePlayerList->getNthPlayer(index)` exists and the observer's `getRelationship(its default team)`:
    /// `None` if either player is missing.
    fn player_relationship_to_player(&self, observer: Option<i32>, other_index: i32) -> Option<Relationship>;
    /// `getSpawnBehaviorInterface()->getCanAnySlavesUseWeaponAgainstTarget(...)`; `None` = no spawn behaviour.
    fn spawn_slaves_can_use_weapon_against_target(
        &mut self,
        o: ObjectId,
        attack_type: AbleToAttackType,
        victim: Option<ObjectId>,
        pos: Option<Coord3D>,
        command_source: CommandSourceType,
    ) -> Option<CanAttackResult>;
    /// `garrisonedMember->getAbleToUseWeaponAgainstTarget(attackType, victim, pos, commandSource)`: another
    /// object's own weapon set is asked (the host dispatches to that object).
    fn member_able_to_use_weapon_against_target(
        &mut self,
        member: ObjectId,
        attack_type: AbleToAttackType,
        victim: Option<ObjectId>,
        pos: Option<Coord3D>,
        command_source: CommandSourceType,
    ) -> Result<CanAttackResult, WeaponError>;

    // ---- the AI of an Object -----------------------------------------------------------------------------------------

    /// `getAI()->getSneakyTargetingOffset(&offset)`: the offset, or `None` when there is no AI or it is not sneaky.
    fn ai_sneaky_targeting_offset(&mut self, o: ObjectId) -> Option<Coord3D>;
    /// `getAI()->getCurLocomotorSetType()`; `None` = no AI.
    fn ai_cur_locomotor_set_type(&mut self, o: ObjectId) -> Option<i32>;
    /// `getAI() && getAI()->isAircraftThatAdjustsDestination()`.
    fn ai_is_aircraft_that_adjusts_destination(&mut self, o: ObjectId) -> bool;
    /// `getAI()->getAssaultTransportAIInterface()->beginAssault(victim)`; nothing without the interface.
    fn ai_begin_assault(&mut self, o: ObjectId, victim: Option<ObjectId>);
    /// `getAIUpdateInterface()->getWhichTurretForWeaponSlot(wslot, &angle, &pitch)`: `None` without AI.
    fn ai_which_turret_for_weapon_slot(&mut self, o: ObjectId, wslot: i32) -> Option<(i32, f32, f32)>;
    /// `getAI()->isWeaponSlotOnTurretAndAimingAtTarget(slot, victim)`; `false` without AI.
    fn ai_is_weapon_slot_on_turret_and_aiming_at_target(&mut self, o: ObjectId, wslot: i32, victim: ObjectId) -> bool;
    /// `TheAI->pathfinder()->adjustTargetDestination(source, target, pos, weapon, &approachTargetPos)`.
    fn adjust_target_destination(
        &mut self,
        source: ObjectId,
        target: Option<ObjectId>,
        pos: Option<Coord3D>,
        template: usize,
        approach_target_pos: &mut Coord3D,
    );

    // ---- the partition manager and the terrain --------------------------------------------------------------------

    /// `ThePartitionManager->getDistanceSquared(a, b, calc)`.
    fn distance_squared_to_object(&self, a: ObjectId, b: ObjectId, calc: DistanceCalculationType) -> f32;
    /// `ThePartitionManager->getDistanceSquared(a, &pos, calc)`.
    fn distance_squared_to_pos(&self, a: ObjectId, pos: &Coord3D, calc: DistanceCalculationType) -> f32;
    /// `ThePartitionManager->getGoalDistanceSquared(a, &goal, b, calc)`.
    fn goal_distance_squared_to_object(&self, a: ObjectId, goal: &Coord3D, b: ObjectId, calc: DistanceCalculationType) -> f32;
    /// `ThePartitionManager->getGoalDistanceSquared(a, &goal, &pos, calc)`.
    fn goal_distance_squared_to_pos(&self, a: ObjectId, goal: &Coord3D, pos: &Coord3D, calc: DistanceCalculationType) -> f32;
    /// `ThePartitionManager->getVectorTo(a, b, calc, dir)`.
    fn vector_to_object(&self, a: ObjectId, b: ObjectId, calc: DistanceCalculationType) -> Coord3D;
    /// `ThePartitionManager->getVectorTo(a, &pos, calc, dir)`.
    fn vector_to_pos(&self, a: ObjectId, pos: &Coord3D, calc: DistanceCalculationType) -> Coord3D;
    /// `ThePartitionManager->iterateObjectsInRange(pos, radius, calc)` walked with `firstWithNumeric` /
    /// `nextWithNumeric`: each object with its distance squared, in the iterator's order.
    fn objects_in_range(&mut self, pos: &Coord3D, radius: f32, calc: DistanceCalculationType) -> Vec<(ObjectId, f32)>;
    /// `ThePartitionManager->iteratePotentialCollisions(source->getPosition(), source->getGeometryInfo(), 0)`.
    fn potential_collisions(&self, source: ObjectId) -> Vec<ObjectId>;
    /// `ThePartitionManager->isClearLineOfSightTerrain(NULL, from, NULL, to)`.
    fn is_clear_line_of_sight_terrain(&self, from: &Coord3D, to: &Coord3D) -> bool;
    /// `TheTerrainLogic->getBridgeAttackPoints(bridge, &info)`: the two attack points.
    fn bridge_attack_points(&self, bridge: ObjectId) -> (Coord3D, Coord3D);
    /// `TheTerrainLogic->getLayerHeight(x, y, layer)`.
    fn layer_height(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32;
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn ground_height(&mut self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->getExtent(&bounds)`.
    fn terrain_extent(&self) -> Region3D;

    // ---- the thing factory, projectiles and what they carry ----------------------------------------------------------

    /// `TheThingFactory->newObject(template, controllingPlayer->getDefaultTeam())` for the player of `source`:
    /// the new object's id. `template` is a thing template id.
    fn new_object_for_player_of(&mut self, template: u16, source: ObjectId) -> ObjectId;
    /// `projectile->setProducer(source)`.
    fn set_producer(&mut self, projectile: ObjectId, source: ObjectId);
    /// `findSpecialPowerCompletionDie() != NULL`.
    fn has_special_power_completion_die(&mut self, o: ObjectId) -> bool;
    /// `findSpecialPowerCompletionDie()->notifyScriptEngine()`.
    fn special_power_completion_die_notify_script_engine(&mut self, o: ObjectId);
    /// `findSpecialPowerCompletionDie()->setCreator(creator)`.
    fn special_power_completion_die_set_creator(&mut self, o: ObjectId, creator: ObjectId);
    /// Whether a behaviour module of `o` has a `ProjectileUpdateInterface`.
    fn has_projectile_update_interface(&mut self, o: ObjectId) -> bool;
    /// `pui->projectileLaunchAtObjectOrPosition(victim, &destination, source, wslot, barrel, weaponTemplate,
    /// exhaust)` on the first module of `projectile` that has the interface.
    #[allow(clippy::too_many_arguments)] // the arguments of upstream's `projectileLaunchAtObjectOrPosition`
    fn projectile_launch(
        &mut self,
        projectile: ObjectId,
        victim: Option<ObjectId>,
        destination: &Coord3D,
        source: ObjectId,
        wslot: i32,
        barrel: i32,
        template: usize,
        exhaust: &FxName,
    );
    /// `pui->projectileGetLauncherID()` on the first module of `o` with the interface; `None` when it has none.
    fn projectile_launcher_id(&mut self, o: ObjectId) -> Option<ObjectId>;
    /// `setPosition(&pos)`.
    fn set_position(&mut self, o: ObjectId, pos: &Coord3D);
    /// `hasCountermeasures()`.
    fn has_countermeasures(&mut self, o: ObjectId) -> bool;
    /// `reportMissileForCountermeasures(projectile)`.
    fn report_missile_for_countermeasures(&mut self, o: ObjectId, projectile: ObjectId);
    /// Some module of `o` has a `LandMineInterface`.
    fn has_land_mine_interface(&mut self, o: ObjectId) -> bool;
    /// `lmi->disarm()` on the first module of `o` with a `LandMineInterface`.
    fn disarm_land_mine(&mut self, o: ObjectId);
    /// `findUpdateModule(ProjectileStreamUpdate)`: `setPosition(pos)` and `addProjectile(source, projectile,
    /// victim, victimPos)`; `false` when the object has no such module.
    fn projectile_stream_add(
        &mut self,
        stream: ObjectId,
        source: ObjectId,
        projectile: ObjectId,
        victim: Option<ObjectId>,
        victim_pos: Option<Coord3D>,
    ) -> bool;
    /// `findUpdateModule(AssistedTargetingUpdate)` of `o`: `Some(isFreeToAssist())`, `None` without the module.
    fn assisted_targeting_is_free(&mut self, o: ObjectId) -> Option<bool>;
    /// `assistedTargetingUpdate->assistAttack(requesting, victim)`.
    fn assisted_targeting_assist_attack(&mut self, o: ObjectId, requesting: ObjectId, victim: ObjectId);
    /// `laser->getDrawable()->findClientUpdateModule(LaserUpdate)->initLaser(source, victim, &sourcePos, &pos,
    /// boneName)` on the laser object `laser` (a no-op when it has no drawable or no module).
    fn laser_init(&mut self, laser: ObjectId, source: ObjectId, victim: Option<ObjectId>, pos: &Coord3D, bone_name: &[u8]);
    /// `ObjectCreationList::create(ocl, primary, NULL)`.
    fn create_ocl(&mut self, ocl: u32, primary: ObjectId);

    // ---- the drawable (model data the simulation reads) ---------------------------------------------------------------

    /// `getDrawable()->getBarrelCount(wslot)`; `None` when the object has no drawable.
    fn barrel_count(&mut self, o: ObjectId, wslot: i32) -> Option<i32>;
    /// `getDrawable()->getProjectileLaunchOffset(wslot, barrel, &attach, turret, &rotPos, &pitchPos)`; `None`
    /// when there is no drawable or no such bone.
    fn projectile_launch_offset(&mut self, o: ObjectId, wslot: i32, barrel: i32, turret: i32) -> Option<LaunchOffset>;
    /// `convertBonePosToWorldPos(NULL, &boneTransform, NULL, &worldTransform)`.
    fn convert_bone_transform_to_world(&self, o: ObjectId, bone_transform: &Matrix3D) -> Matrix3D;
    /// A client effect.
    fn weapon_fx(&mut self, fx: WeaponFx);

    // ---- positioning a projectile for launch -------------------------------------------------------------------------------

    /// `projectile->getDrawable()->setDrawableHidden(false)`.
    fn unhide_drawable(&mut self, o: ObjectId);
    /// `projectile->setTransformMatrix(&m)`.
    fn set_transform_matrix(&mut self, o: ObjectId, m: &Matrix3D);
    /// `projectile->getExperienceTracker()->setExperienceSink(launcher)`.
    fn set_experience_sink(&mut self, o: ObjectId, sink: ObjectId);
    /// `launcherPhys->transferVelocityTo(missilePhys); missilePhys->setIgnoreCollisionsWith(launcher)` when both
    /// have a `PhysicsBehavior`.
    fn transfer_velocity_and_ignore_collisions(&mut self, launcher: ObjectId, projectile: ObjectId);
    /// A parking place on `o` (`ParkingPlaceBehaviorInterface`) that `hasReservedSpace(intended)`.
    fn parking_place_has_reserved_space(&mut self, o: ObjectId, intended: ObjectId) -> bool;
}
