// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/ActionManager.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/ActionManager.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ActionManager`: "queries about what objects can do in the world and to other objects. The purpose of having a
//! central place for this logic assists us in making these logical kind of queries in the user interface and allows
//! us to use the same code to validate commands as they come in over the network interface in order to do the real
//! action." Wave 7, slice C (`p1w8-sonnet`).
//!
//! # The shape
//!
//! `ActionManager` has no state (its `init`, `reset` and `update` are empty), so each query is a function over an
//! [`ActionHost`]: an [`ObjectWeaponHost`] that also answers what the queries ask of an object's body, AI, modules,
//! contain module, special powers, shroud and the terrain. Each host method is called where upstream makes the
//! call, in upstream's order, with the same short-circuiting. The weapon questions (`getAbleToAttackSpecificObject`,
//! `getWeaponInWeaponSlot`, `estimateWeaponDamage`, ...) go to the real `WeaponSet` through `object_weapons`.
//!
//! # Where upstream is undefined
//!
//! A null `BodyModuleInterface*`, `Team*` or `Player*` that upstream dereferences is [`WeaponError::NullObject`].
//!
//! # Verified
//!
//! Against upstream's own `ActionManager.cpp` (unchanged but for its includes) by
//! `tests/differential_weapon_fire.rs`, ops 48 to 70, on the weapon test's stand-ins. Methodology and results are in
//! that file's header.

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{DisabledType, DISABLED_SUBDUED, DISABLED_UNMANNED};
use crate::common::game_common::{CellShroudStatus, ObjectShroudStatus, Relationship};
use crate::common::game_type::{ObjectId, WEAPONSLOT_COUNT};
use crate::common::kind_of::{
    KINDOF_AIRCRAFT, KINDOF_BOAT, KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER, KINDOF_CAPTURABLE, KINDOF_CASH_GENERATOR, KINDOF_CLIFF_JUMPER,
    KINDOF_DOZER, KINDOF_DRONE, KINDOF_FS_AIRFIELD, KINDOF_FS_TECHNOLOGY, KINDOF_HEAL_PAD, KINDOF_IGNORED_IN_GUI, KINDOF_IMMOBILE,
    KINDOF_IMMUNE_TO_CAPTURE, KINDOF_INFANTRY, KINDOF_MOB_NEXUS, KINDOF_NO_GARRISON, KINDOF_REBUILD_HOLE, KINDOF_REJECT_UNMANNED, KINDOF_REPAIR_PAD,
    KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use crate::common::object_status_types::{
    OBJECT_STATUS_DECK_HEIGHT_OFFSET, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_SOLD, OBJECT_STATUS_STEALTHED,
    OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::common::special_power_type::*;
use crate::logic::damage::{DAMAGE_DISARM, DAMAGE_KILLPILOT};
use crate::logic::weapon::WeaponError;
use crate::logic::weapon_host::{AbleToAttackType, CanAttackResult, CommandSourceType, ATTACK_NEW_TARGET};
use crate::logic::world::object_weapons::{self as ow, ObjectWeaponHost};

type AResult<T> = Result<T, WeaponError>;

/// `enum CanEnterType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanEnterType {
    /// `CHECK_CAPACITY`.
    CheckCapacity,
    /// `DONT_CHECK_CAPACITY`.
    DontCheckCapacity,
    /// `COMBATDROP_INTO`.
    CombatDropInto,
}

/// A `SpecialPowerTemplate*`: its identity and its `getSpecialPowerType()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialPowerTemplateRef {
    /// The template's identity (what `getSpecialPowerModule(template)` matches).
    pub id: u32,
    /// `getSpecialPowerType()`.
    pub sp_type: SpecialPowerType,
}

/// The update modules `ActionManager` looks up by name key (`findUpdateModule(key)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamedUpdateModule {
    /// `NAMEKEY("SupplyWarehouseDockUpdate")`.
    SupplyWarehouseDock,
    /// `NAMEKEY("SupplyCenterDockUpdate")`.
    SupplyCenterDock,
    /// `NAMEKEY("RailedTransportDockUpdate")`.
    RailedTransportDock,
    /// `NAMEKEY("RailroadBehavior")`.
    Railroad,
}

/// `SupplyTruckAIInterface`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupplyTruckFacts {
    /// `getNumberBoxes()`.
    pub number_boxes: i32,
    /// `isAvailableForSupplying()`.
    pub available_for_supplying: bool,
}

/// `DOZER_TASK_BUILD`.
pub const DOZER_TASK_BUILD: i32 = 0;

/// `DozerAIInterface`'s `getCurrentTask()` and `getTaskTarget(DOZER_TASK_BUILD)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DozerFacts {
    /// `getCurrentTask()`.
    pub current_task: i32,
    /// `getTaskTarget(DOZER_TASK_BUILD)`.
    pub build_task_target: ObjectId,
}

/// `SpecialAbilityUpdate`'s counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialAbilityFacts {
    /// `getSpecialObjectCount()`.
    pub object_count: i32,
    /// `getSpecialObjectMax()`.
    pub object_max: i32,
}

/// What one `CollideModuleInterface` of an object says about another object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollideFacts {
    /// `wouldLikeToCollideWith(other)`.
    pub would_like: bool,
    /// `isCarBombCrateCollide()`.
    pub car_bomb: bool,
    /// `isHijackedVehicleCrateCollide()`.
    pub hijack: bool,
    /// `isSabotageBuildingCrateCollide()`.
    pub sabotage: bool,
}

/// What one `ParkingPlaceBehaviorInterface` of a destination says about an object that wants to land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParkingFacts {
    /// `hasReservedSpace(obj->getID())`.
    pub has_reserved_space: bool,
    /// `shouldReserveDoorWhenQueued(obj->getTemplate())`.
    pub should_reserve_door: bool,
    /// `hasAvailableSpaceFor(obj->getTemplate())`.
    pub has_available_space: bool,
}

/// What `ActionManager` asks beyond a weapon host. See the module docs.
pub trait ActionHost: ObjectWeaponHost {
    // ---- the object ---------------------------------------------------------------------------------------------
    /// `isMobile()`.
    fn is_mobile(&self, o: ObjectId) -> bool;
    /// `getCarrierDeckHeight()`.
    fn carrier_deck_height(&self, o: ObjectId) -> f32;
    /// `isDisabledByType(t)`.
    fn is_disabled_by_type(&self, o: ObjectId, t: DisabledType) -> bool;
    /// `getBodyModule()`: its `getHealth()` and `getMaxHealth()`; `None` = no body module.
    fn body_health(&self, o: ObjectId) -> Option<(f32, f32)>;
    /// `getTransportSlotCount()`.
    fn transport_slot_count(&self, o: ObjectId) -> i32;
    /// `isFactionStructure()`.
    fn is_faction_structure(&self, o: ObjectId) -> bool;
    /// `getShroudedStatus(playerIndex)`.
    fn object_shrouded_status(&self, o: ObjectId, player_index: i32) -> ObjectShroudStatus;
    /// `getBuilderID()`.
    fn builder_id(&self, o: ObjectId) -> ObjectId;
    /// `getControllingPlayer()->getPlayerType() == PLAYER_HUMAN`; `None` when there is no controlling player.
    fn controlling_player_is_human(&self, o: ObjectId) -> Option<bool>;
    /// `player->getRelationship(o->getTeam())` for the player with this index (`NEUTRAL` for an object with no
    /// team: `Player::getRelationship(NULL)` is `NEUTRAL`).
    fn player_relationship_to_team_of(&self, player_index: i32, o: ObjectId) -> Relationship;

    // ---- special powers -------------------------------------------------------------------------------------------
    /// `hasSpecialPower(type)`.
    fn has_special_power(&self, o: ObjectId, t: SpecialPowerType) -> bool;
    /// `findSpecialPowerModuleInterface(type)->getPercentReady()`; `None` = no such module.
    fn special_power_percent_ready_by_type(&self, o: ObjectId, t: SpecialPowerType) -> Option<f32>;
    /// `getSpecialPowerModule(template)->getPercentReady()`; `None` = no such module.
    fn special_power_percent_ready_by_template(&self, o: ObjectId, template: &SpecialPowerTemplateRef) -> Option<f32>;
    /// `findSpecialAbilityUpdate(type)`.
    fn special_ability_update(&self, o: ObjectId, t: SpecialPowerType) -> Option<SpecialAbilityFacts>;
    /// `findSpecialAbilityUpdate(type)->findSpecialObjectWithProducerID(target) != NULL`.
    fn special_ability_has_object_with_producer(&self, o: ObjectId, t: SpecialPowerType, target: ObjectId) -> bool;
    /// `findSpecialPowerWithOverridableDestinationActive(type) != NULL`.
    fn special_power_overridable_destination_active(&self, o: ObjectId, t: SpecialPowerType) -> bool;

    // ---- modules --------------------------------------------------------------------------------------------------
    /// `findUpdateModule(key) != NULL`.
    fn has_named_update_module(&self, o: ObjectId, which: NamedUpdateModule) -> bool;
    /// `SupplyWarehouseDockUpdate::getBoxesStored()`.
    fn warehouse_boxes_stored(&self, o: ObjectId) -> i32;
    /// `getAI() != NULL`.
    fn has_ai_interface(&self, o: ObjectId) -> bool;
    /// `getAI()->getSupplyTruckAIInterface()`.
    fn supply_truck_ai(&self, o: ObjectId) -> Option<SupplyTruckFacts>;
    /// `getAI()->getDozerAIInterface()`.
    fn dozer_ai(&self, o: ObjectId) -> Option<DozerFacts>;
    /// Whether some behaviour module of `o` has a `DockUpdateInterface`.
    fn has_dock_update_interface(&self, o: ObjectId) -> bool;
    /// Every behaviour module of `o` that has a `CollideModuleInterface`, in module order, asked about `other`.
    fn collide_facts(&self, o: ObjectId, other: ObjectId) -> Vec<CollideFacts>;
    /// Every behaviour module of `dest` that has a `ParkingPlaceBehaviorInterface`, in module order, asked about
    /// `obj`.
    fn parking_facts(&self, dest: ObjectId, obj: ObjectId) -> Vec<ParkingFacts>;
    /// `getContain()->isHealContain()`.
    fn contain_is_heal_contain(&self, o: ObjectId) -> bool;
    /// `getContain()->getStealthUnitsContained()`.
    fn contain_stealth_units_contained(&self, o: ObjectId) -> i32;
    /// `container->getContain()->isValidContainerFor(obj, checkCapacity)`.
    fn contain_is_valid_container_for(&self, container: ObjectId, obj: ObjectId, check_capacity: bool) -> bool;
    /// `getContain()->getClosestRider(pos)`.
    fn contain_closest_rider(&self, container: ObjectId, pos: &Coord3D) -> Option<ObjectId>;
    /// `getSpawnBehaviorInterface()->getClosestSlave(pos)`: `None` = no spawn behaviour.
    fn spawn_closest_slave(&self, o: ObjectId, pos: &Coord3D) -> Option<Option<ObjectId>>;

    // ---- the world ------------------------------------------------------------------------------------------------
    /// `ThePartitionManager->getShroudStatusForPlayer(playerIndex, loc)`.
    fn cell_shroud_status_for_player(&self, player_index: i32, loc: &Coord3D) -> CellShroudStatus;
    /// `TheTerrainLogic->isUnderwater(x, y)`.
    fn is_underwater(&self, x: f32, y: f32) -> bool;
}

/// `appearsToContainFriendlies(obj, otherObject)`: "check if the object is a container containing stealth units
/// tricking the player into thinking it isn't actually an enemy."
fn appears_to_contain_friendlies<H: ActionHost>(host: &H, obj: ObjectId, other_object: ObjectId) -> AResult<bool> {
    if host.has_contain(other_object) {
        let other_player = host.contain_apparent_controlling_player(other_object, host.controlling_player(obj));
        // the test "other player's relationship to the object's team" is wrong; we want to know how WE consider THEM,
        // not how THEY consider US
        if let Some(p) = other_player {
            // (`obj->getTeam()` is dereferenced: an object with no team is undefined upstream)
            host.controlling_player(obj).ok_or(WeaponError::NullObject)?;
            if host.team_relationship_to_player_index(obj, p) != Relationship::Enemies {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// `isObjectShroudedForAction(source, target, commandSource)`: the target is only shrouded for action if the asking
/// player is human, the asking impetus is not from a script and the target object is Fogged or worse.
fn is_object_shrouded_for_action<H: ActionHost>(host: &H, source: ObjectId, target: ObjectId, command_source: CommandSourceType) -> bool {
    if let (Some(human), Some(player)) = (host.controlling_player_is_human(source), host.controlling_player(source)) {
        if human && command_source != CommandSourceType::FromScript && host.object_shrouded_status(target, player) >= ObjectShroudStatus::Fogged {
            return true;
        }
    }
    false
}

/// A stealthed, undetected and undisguised target: "if the enemy unit is stealthed and not detected, then we can't
/// attack it!"
fn is_hidden_by_stealth<H: ActionHost>(host: &H, target: ObjectId) -> bool {
    host.test_status(target, OBJECT_STATUS_STEALTHED) && !host.test_status(target, OBJECT_STATUS_DETECTED) && !host.test_status(target, OBJECT_STATUS_DISGUISED)
}

/// `ActionManager::canGetRepairedAt(obj, repairDest, commandSource)`.
pub fn can_get_repaired_at<H: ActionHost>(host: &H, obj: Option<ObjectId>, repair_dest: Option<ObjectId>, command_source: CommandSourceType) -> AResult<bool> {
    // sanity
    let (Some(obj), Some(repair_dest)) = (obj, repair_dest) else { return Ok(false) };

    let r = host.relationship(obj, repair_dest);

    // only available by our allies
    if r != Relationship::Allies {
        return Ok(false);
    }

    // dead objects cannot be repaired
    if host.is_effectively_dead(obj) {
        return Ok(false);
    }

    // If I can't move, I can't get repaired
    if !host.is_mobile(obj) {
        return Ok(false);
    }

    // nothing can be done with things that are under construction
    if host.test_status(obj, OBJECT_STATUS_UNDER_CONSTRUCTION) || host.test_status(repair_dest, OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return Ok(false);
    }

    // Can't get repaired at something being sold
    if host.test_status(repair_dest, OBJECT_STATUS_SOLD) {
        return Ok(false);
    }

    // only vehicles can go get repaired at something
    if !host.is_kind_of(obj, KINDOF_VEHICLE) {
        return Ok(false);
    }

    // vehicles can only be repaired at something that is designated as a repair pad
    if host.is_kind_of(obj, KINDOF_AIRCRAFT) {
        // aircraft require an airfield.
        if !host.is_above_terrain(obj) || !host.is_kind_of(repair_dest, KINDOF_FS_AIRFIELD) {
            return Ok(false);
        }
    } else if !host.is_kind_of(repair_dest, KINDOF_REPAIR_PAD) {
        return Ok(false);
    }

    // if I am at full health, I can't get repair there
    let (health, max_health) = host.body_health(obj).ok_or(WeaponError::NullObject)?;
    if health == max_health {
        return Ok(false);
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, repair_dest, command_source) {
        return Ok(false);
    }

    // all is well, we can be repaired here
    Ok(true)
}

/// `ActionManager::canTransferSuppliesAt(obj, transferDest)`: note that "dest" is typically a building.
pub fn can_transfer_supplies_at<H: ActionHost>(host: &H, obj: Option<ObjectId>, transfer_dest: Option<ObjectId>) -> bool {
    // sanity
    let (Some(obj), Some(transfer_dest)) = (obj, transfer_dest) else { return false };

    if host.is_effectively_dead(transfer_dest) {
        return false;
    }

    // nothing can be done with things that are under construction
    if host.test_status(obj, OBJECT_STATUS_UNDER_CONSTRUCTION) || host.test_status(transfer_dest, OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return false;
    }

    // Can't transfer at something being sold
    if host.test_status(transfer_dest, OBJECT_STATUS_SOLD) {
        return false;
    }

    // I must be something with a Supply Transfering AI interface
    if !host.has_ai_interface(obj) {
        return false;
    }

    let Some(supply_truck) = host.supply_truck_ai(obj) else { return false };

    // If it is a warehouse, it must have boxes left and not be an enemy
    let warehouse_module = host.has_named_update_module(transfer_dest, NamedUpdateModule::SupplyWarehouseDock);
    if warehouse_module && (host.warehouse_boxes_stored(transfer_dest) == 0 || host.relationship(transfer_dest, obj) == Relationship::Enemies) {
        return false;
    }

    // if it is a supply center, I must have boxes, and must be controlled by the same player (not merely an ally...
    // otherwise you may find yourself funding your allies. ick.)
    let center_module = host.has_named_update_module(transfer_dest, NamedUpdateModule::SupplyCenterDock);
    if center_module && (supply_truck.number_boxes == 0 || host.controlling_player(transfer_dest) != host.controlling_player(obj)) {
        return false;
    }

    // if he is not a warehouse or a center, then shut the hell up
    if !warehouse_module && !center_module {
        return false;
    }

    // We do not check ClearToApproach, as it is a temporary failure that is handled in the state logic. This
    // function is for Legality, not conditionals. however, we DO check for the unit to be available (cf Chinook) (srj)
    if !supply_truck.available_for_supplying {
        return false;
    }

    // Fogged is okay for player, and anything is okay for AI.
    if let (Some(human), Some(player)) = (host.controlling_player_is_human(obj), host.controlling_player(obj)) {
        if human && host.object_shrouded_status(transfer_dest, player) == ObjectShroudStatus::Shrouded {
            return false;
        }
    }

    // all is well, we can transfer here
    true
}

/// `ActionManager::canDockAt(obj, dockDest, commandSource)`: can object 'obj' dock with object 'dockDest' for any
/// reason.
pub fn can_dock_at<H: ActionHost>(host: &H, obj: ObjectId, dock_dest: ObjectId, _command_source: CommandSourceType) -> bool {
    // look for a dock interface
    if !host.has_dock_update_interface(dock_dest) {
        return false; // no dock update interface, can't possibly dock
    }

    // transferring supplies is a valid docking action
    if can_transfer_supplies_at(host, Some(obj), Some(dock_dest)) {
        return true;
    }

    // units and infantry can dock with a railed transport
    if host.has_named_update_module(dock_dest, NamedUpdateModule::RailedTransportDock) && (host.is_kind_of(obj, KINDOF_VEHICLE) || host.is_kind_of(obj, KINDOF_INFANTRY)) {
        return true;
    }

    // cannot dock
    false
}

/// `ActionManager::canGetHealedAt(obj, healDest, commandSource)`.
pub fn can_get_healed_at<H: ActionHost>(host: &H, obj: Option<ObjectId>, heal_dest: Option<ObjectId>, command_source: CommandSourceType) -> bool {
    // sanity
    let (Some(obj), Some(heal_dest)) = (obj, heal_dest) else { return false };

    let r = host.relationship(obj, heal_dest);

    // only available by our allies
    if r != Relationship::Allies {
        return false;
    }

    // dead objects cannot be healed
    if host.is_effectively_dead(heal_dest) {
        return false;
    }

    // nothing can be done with things that are under construction
    if host.test_status(obj, OBJECT_STATUS_UNDER_CONSTRUCTION) || host.test_status(heal_dest, OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return false;
    }

    // Can't get healed at something being sold
    if host.test_status(heal_dest, OBJECT_STATUS_SOLD) {
        return false;
    }

    // only infantry can go get "healed" somewhere (vehicles get "repaired")
    if !host.is_kind_of(obj, KINDOF_INFANTRY) {
        return false;
    }

    // infantry can only be healed at something that is designated as a heal pad
    if !host.is_kind_of(heal_dest, KINDOF_HEAL_PAD) {
        return false;
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, heal_dest, command_source) {
        return false;
    }

    if let Some((health, max_health)) = host.body_health(obj) {
        if health == max_health {
            // No point in healing if you have full health!
            return false;
        }
    }

    // all is well, we can be healed here
    true
}

/// `ActionManager::canRepairObject(obj, objectToRepair, commandSource)`.
pub fn can_repair_object<H: ActionHost>(host: &H, obj: Option<ObjectId>, object_to_repair: Option<ObjectId>, command_source: CommandSourceType) -> AResult<bool> {
    // sanity
    let (Some(obj), Some(object_to_repair)) = (obj, object_to_repair) else { return Ok(false) };

    let r = host.relationship(obj, object_to_repair);

    // you can only repair allies, we ignore this restriction for bridges
    // srj sez: nope, allow neutral too, so civ bldgs can be repaired
    // GS repairing bridges cut 12/12/02 , so this check is just no to enemies
    if r == Relationship::Enemies {
        return Ok(false);
    }

    // can't repair dead things ... the exception is bridges and bridge towers, which can be destroyed and die, but
    // can be repaired to bring the bring "back to life"
    // GS and again, repairing bridges is cut, so this is just a dead check
    if host.is_effectively_dead(object_to_repair) {
        return Ok(false);
    }

    // GS So here's the ensuring that they can't be repaired
    if host.is_kind_of(object_to_repair, KINDOF_BRIDGE) || host.is_kind_of(object_to_repair, KINDOF_BRIDGE_TOWER) {
        return Ok(false);
    }

    // nothing can be done with things that are under construction
    if host.test_status(obj, OBJECT_STATUS_UNDER_CONSTRUCTION) || host.test_status(object_to_repair, OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return Ok(false);
    }

    // we cannot manually repair things that are regeneration holes
    if host.is_kind_of(object_to_repair, KINDOF_REBUILD_HOLE) {
        return Ok(false);
    }

    // only Dozers can go repair things
    if !host.is_kind_of(obj, KINDOF_DOZER) {
        return Ok(false);
    }

    // dozers can only repair buildings
    if !host.is_kind_of(object_to_repair, KINDOF_STRUCTURE) {
        return Ok(false);
    }

    // get the body module from the object to repair
    let (health, max_health) = host.body_health(object_to_repair).ok_or(WeaponError::NullObject)?;

    // buildings that are at full health cannot be repaired
    if health == max_health {
        return Ok(false);
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_repair, command_source) {
        return Ok(false);
    }

    if host.contained_by(obj).is_some() {
        // We can't heal things while in a transport (especially our own transport, you cheater)
        return Ok(false);
    }

    Ok(true)
}

/// `ActionManager::canResumeConstructionOf(obj, objectBeingConstructed, commandSource)`.
pub fn can_resume_construction_of<H: ActionHost>(
    host: &H,
    obj: Option<ObjectId>,
    object_being_constructed: Option<ObjectId>,
    command_source: CommandSourceType,
) -> bool {
    // sanity
    let (Some(obj), Some(object_being_constructed)) = (obj, object_being_constructed) else { return false };

    // only dozers or workers can resume construction of things
    if !host.is_kind_of(obj, KINDOF_DOZER) {
        return false;
    }

    let r = host.relationship(obj, object_being_constructed);

    // only available to our allies
    if r != Relationship::Allies {
        return false;
    }

    // if the objectBeingConstructed is not actually under construction we can't resume that!
    if !host.test_status(object_being_constructed, OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return false;
    }

    // dead things can do nothing
    if host.is_effectively_dead(obj) {
        return false;
    }

    // if the object being constructed is actively being built by another dozer we cannot add more effectiveness to
    // the construction so we'll say no (this might change for workers in the future)
    let builder = host.builder_id(object_being_constructed);
    if host.object_exists(builder) && host.has_ai_interface(builder) {
        if let Some(dozer) = host.dozer_ai(builder) {
            if dozer.current_task == DOZER_TASK_BUILD && dozer.build_task_target == object_being_constructed {
                return false;
            }
        }
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_being_constructed, command_source) {
        return false;
    }

    // all is well, the objectBeingConstructeds' builder object has gone away or is no longer building the object anymore
    true
}

/// `ActionManager::canEnterObject(obj, objectToEnter, commandSource, mode)`.
pub fn can_enter_object<H: ActionHost>(
    host: &H,
    obj: Option<ObjectId>,
    object_to_enter: Option<ObjectId>,
    command_source: CommandSourceType,
    mode: CanEnterType,
) -> AResult<bool> {
    // sanity
    let (Some(obj), Some(object_to_enter)) = (obj, object_to_enter) else { return Ok(false) };

    if obj == object_to_enter {
        // You can't contain yourself (crash fix for pow truck reselection)
        return Ok(false);
    }

    // can't enter dead things
    if host.is_effectively_dead(object_to_enter) {
        return Ok(false);
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_enter, command_source) {
        return Ok(false);
    }

    // nothing can be done with things that are under construction
    if host.test_status(obj, OBJECT_STATUS_UNDER_CONSTRUCTION) || host.test_status(object_to_enter, OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return Ok(false);
    }

    // Can't enter something being sold
    if host.test_status(object_to_enter, OBJECT_STATUS_SOLD) {
        return Ok(false);
    }
    if host.is_kind_of(obj, KINDOF_IGNORED_IN_GUI) // As in, Angry Mob Members, Cargo Planes
        || host.is_kind_of(obj, KINDOF_MOB_NEXUS)
        || host.is_kind_of(object_to_enter, KINDOF_IGNORED_IN_GUI)
    // As in Cargo Planes
    {
        return Ok(false);
    }

    if host.is_disabled_by_type(object_to_enter, DISABLED_SUBDUED) {
        return Ok(false); // a microwave tank has soldered the doors shut
    }

    if host.is_kind_of(obj, KINDOF_STRUCTURE) || host.is_kind_of(obj, KINDOF_IMMOBILE) {
        // Structures or immobiles can't garrison
        return Ok(false);
    }

    // Special case for unmanned vehicles. Any infantry unit can take over any unmanned vehicle!
    if host.is_kind_of(obj, KINDOF_INFANTRY) && host.is_disabled_by_type(object_to_enter, DISABLED_UNMANNED) && !host.is_kind_of(obj, KINDOF_REJECT_UNMANNED) {
        // But only if it's allowed to.
        return Ok(true);
    }

    // Special case for aircraft.
    if host.is_kind_of(obj, KINDOF_AIRCRAFT) && host.is_kind_of(object_to_enter, KINDOF_FS_AIRFIELD) {
        if host.test_status(obj, OBJECT_STATUS_DECK_HEIGHT_OFFSET) && host.carrier_deck_height(obj) >= host.position(obj).z {
            return Ok(false);
        }

        if !host.is_above_terrain(obj) {
            return Ok(false);
        }

        if host.controlling_player(obj) == host.controlling_player(object_to_enter) {
            // Kris -- added code to prevent aircraft from landing in any airstrips other than their own!

            // @todo srj -- this is horrible, but expedient.
            for pp in host.parking_facts(object_to_enter, obj) {
                if pp.has_reserved_space {
                    return Ok(true);
                }

                if pp.should_reserve_door && pp.has_available_space {
                    return Ok(true);
                }
            }
        }
        return Ok(false);
    }

    // first, see if we'd like to collide with 'other'
    for collide in host.collide_facts(obj, object_to_enter) {
        if collide.would_like {
            // I thought this was a little confusing that it would return TRUE here before getting to any of the
            // other checks. The key is that it usually doesn't return TRUE because most things aren't trying to
            // collide with objects. This is different for terrorist converting carbombs, and pilots entering
            // vehicles. In these cases, the vehicles don't have transport capacities, therefore returning true
            // here foregoes that checking later on.
            return Ok(true);
        }
    }

    // make sure our objectToEnter has a contain module.
    if !host.has_contain(object_to_enter) {
        return Ok(false);
    }

    if host.contain_is_heal_contain(object_to_enter) {
        // (upstream dereferences the body of obj without testing it)
        let (health, max_health) = host.body_health(obj).ok_or(WeaponError::NullObject)?;
        if health == max_health {
            // This container is only used for the purposes of healing and we cannot enter it with full health.
            // This is not a normal container.
            return Ok(false);
        }
    }

    if mode == CanEnterType::CombatDropInto {
        // we don't care about valid container-ness, but we DO care about faction structures... we aren't allowed to
        // combat drop into them
        if host.is_faction_structure(object_to_enter) {
            return Ok(false);
        }
    } else {
        let mut check_capacity = mode == CanEnterType::CheckCapacity;
        let contain_count = host.contain_facts(object_to_enter).map_or(0, |f| f.count);
        let stealth_contain_count = host.contain_stealth_units_contained(object_to_enter);
        let non_stealth_contain_count = contain_count - stealth_contain_count;

        // not ours... must do special checks.
        if host.controlling_player(object_to_enter) != host.controlling_player(obj) {
            // not empty... can't do it.
            if non_stealth_contain_count > 0 {
                return Ok(false);
            }

            // faction structure... can't do it.
            if host.is_faction_structure(object_to_enter) {
                return Ok(false);
            }

            // it's stealth-garrisoned... ignore check-cap and fall thru to normal isValid test.
            if stealth_contain_count > 0 && non_stealth_contain_count == 0 {
                check_capacity = false;
            }
        }

        // if our transport slot count is zero, we can't be transported. so punt.
        // @todo srj -- seems like we should check always (not just for checkCap), but scared to change now -- check later
        if check_capacity && host.transport_slot_count(obj) == 0 {
            return Ok(false);
        }

        // finally: make sure that objectToEnter is a valid container for obj
        if !host.contain_is_valid_container_for(object_to_enter, obj, check_capacity) {
            return Ok(false);
        }
    }

    Ok(true)
}

/// `ActionManager::getCanAttackObject(obj, objectToAttack, commandSource, attackType)`.
pub fn get_can_attack_object<H: ActionHost>(
    host: &H,
    obj: Option<ObjectId>,
    object_to_attack: Option<ObjectId>,
    command_source: CommandSourceType,
    attack_type: AbleToAttackType,
) -> AResult<CanAttackResult> {
    // sanity
    let (Some(obj), Some(object_to_attack)) = (obj, object_to_attack) else { return Ok(CanAttackResult::NotPossible) };
    if host.is_effectively_dead(obj) || host.is_effectively_dead(object_to_attack) || object_to_attack == obj {
        return Ok(CanAttackResult::NotPossible);
    }

    if !host.is_able_to_attack(obj) {
        return Ok(CanAttackResult::NotPossible);
    }

    // has any weapons that are capable of inflicting damage. Special damage types are rejected such as hack
    // weapons... others can be added.
    let mut result = ow::get_able_to_attack_specific_object(host, obj, attack_type, object_to_attack, command_source, -1)?;
    if result != CanAttackResult::NotPossible {
        // Kris: August 5, 2003
        // Fix a case where the Demo_GLAInfantryWorker is able to attack using his passive bomb upgrade. We don't
        // want him to allow a visible attack cursor for the player. This code never checked for
        // AutoChoosesSources logic, but now we will but only when the commandSource is FROM_PLAYER.
        if command_source == CommandSourceType::FromPlayer {
            // Check if it's got any weapons that can be used.
            let mut any_valid_weapon = false;
            for i in 0..WEAPONSLOT_COUNT {
                let cmd_source_mask = ow::get_weapon_in_weapon_slot_command_source_mask(host, obj, i as usize);
                if cmd_source_mask != 0 {
                    any_valid_weapon = true;
                    break;
                }
            }
            if !any_valid_weapon {
                return Ok(CanAttackResult::NotPossible);
            }
        }

        if result == CanAttackResult::InvalidShot && host.is_kind_of(obj, KINDOF_DOZER) {
            // For the case of dozers, we don't ever want to see an attack cursor unless it's valid on a mine.
            let set = host.weapon_set(obj);
            if let Some(weapon) = set.get_cur_weapon().filter(|_| set.has_any_weapon()) {
                if host.store().templates()[weapon.template()].damage_type == DAMAGE_DISARM {
                    return Ok(CanAttackResult::NotPossible);
                }
            }
        }
        return Ok(result);
    }

    // Special case code for stinger sites: Stinger sites have no weapons, instead -- their spawns are the weapons,
    // in this case the stinger soldiers.
    if host.is_kind_of(obj, KINDOF_SPAWNS_ARE_THE_WEAPONS) {
        // Look at the spawn behavior and evaluate them!
        let target_pos = host.position(object_to_attack);
        match host.spawn_closest_slave(obj, &target_pos) {
            Some(slave) => {
                // We found the spawn interface, now get the closest slave to the target.
                if let Some(slave) = slave {
                    result = ow::get_able_to_attack_specific_object(host, slave, attack_type, object_to_attack, command_source, -1)?;
                    if result != CanAttackResult::NotPossible {
                        return Ok(result);
                    }
                }
            }
            None => {
                if result == CanAttackResult::NotPossible {
                    // oh dear me. The wierd case of a garrisoncontainer being a KINDOF_SPAWNS_ARE_THE_WEAPONS... the
                    // AmericaBuildingFirebase
                    if host.has_contain(obj) {
                        if let Some(rider) = host.contain_closest_rider(obj, &target_pos) {
                            result = ow::get_able_to_attack_specific_object(host, rider, attack_type, object_to_attack, command_source, -1)?;
                            if result != CanAttackResult::NotPossible {
                                return Ok(result);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(CanAttackResult::NotPossible)
}

/// `ActionManager::canConvertObjectToCarBomb(obj, objectToConvert, commandSource)`.
pub fn can_convert_object_to_car_bomb<H: ActionHost>(
    host: &H,
    obj: Option<ObjectId>,
    object_to_convert: Option<ObjectId>,
    command_source: CommandSourceType,
) -> bool {
    // sanity
    let (Some(obj), Some(object_to_convert)) = (obj, object_to_convert) else { return false };

    if host.is_effectively_dead(object_to_convert) {
        return false;
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_convert, command_source) {
        return false;
    }

    // first, see if we'd like to collide with 'other'
    for collide in host.collide_facts(obj, object_to_convert) {
        if collide.would_like && collide.car_bomb {
            return true;
        }
    }
    false
}

/// `ActionManager::canHijackVehicle(obj, objectToHijack, commandSource)` (LORENZEN).
pub fn can_hijack_vehicle<H: ActionHost>(host: &H, obj: Option<ObjectId>, object_to_hijack: Option<ObjectId>, command_source: CommandSourceType) -> bool {
    // sanity
    let (Some(obj), Some(object_to_hijack)) = (obj, object_to_hijack) else { return false };

    // Make sure it's alive.
    if host.is_effectively_dead(object_to_hijack) {
        return false;
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_hijack, command_source) {
        return false;
    }

    let r = host.relationship(obj, object_to_hijack);
    // Only hijack enemy objects
    if r != Relationship::Enemies {
        return false;
    }

    // Make sure target is a vehicle.
    if !host.is_kind_of(object_to_hijack, KINDOF_VEHICLE) {
        return false;
    }

    // Kris -- Hijackers can no longer hijack any aircraft.
    if host.is_kind_of(object_to_hijack, KINDOF_AIRCRAFT) {
        return false;
    }

    // Can't hijack a drone type.
    if host.is_kind_of(object_to_hijack, KINDOF_DRONE) {
        return false;
    }

    // last, see if we'd like to collide with 'objectToHijack'
    for collide in host.collide_facts(obj, object_to_hijack) {
        if collide.would_like && collide.hijack {
            return true;
        }
    }

    false
}

/// `ActionManager::canSabotageBuilding(obj, objectToSabotage, commandSource)`.
pub fn can_sabotage_building<H: ActionHost>(host: &H, obj: Option<ObjectId>, object_to_sabotage: Option<ObjectId>, command_source: CommandSourceType) -> bool {
    // sanity
    let (Some(obj), Some(object_to_sabotage)) = (obj, object_to_sabotage) else { return false };

    // Make sure it's alive.
    if host.is_effectively_dead(object_to_sabotage) {
        return false;
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_sabotage, command_source) {
        return false;
    }

    let r = host.relationship(obj, object_to_sabotage);
    // Only sabotage enemy objects
    if r != Relationship::Enemies {
        return false;
    }

    // last, see if we'd like to collide with 'objectToSabotage'
    for collide in host.collide_facts(obj, object_to_sabotage) {
        if collide.would_like && collide.sabotage {
            return true;
        }
    }

    false
}

/// `ActionManager::canMakeObjectDefector(obj, objectToMakeDefector, commandSource)` (LORENZEN).
pub fn can_make_object_defector<H: ActionHost>(
    host: &H,
    obj: Option<ObjectId>,
    object_to_make_defector: Option<ObjectId>,
    command_source: CommandSourceType,
) -> bool {
    // sanity
    let (Some(obj), Some(object_to_make_defector)) = (obj, object_to_make_defector) else { return false };

    let r = host.relationship(obj, object_to_make_defector);

    // Only make defectors of enemy objects
    if r != Relationship::Enemies {
        return false;
    }

    // Make sure it's alive.
    if host.is_effectively_dead(object_to_make_defector) {
        return false;
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_make_defector, command_source) {
        return false;
    }

    true
}

/// `ActionManager::canCaptureBuilding(obj, objectToCapture, commandSource)`.
pub fn can_capture_building<H: ActionHost>(host: &H, obj: Option<ObjectId>, object_to_capture: Option<ObjectId>, command_source: CommandSourceType) -> AResult<bool> {
    // sanity
    let (Some(obj), Some(object_to_capture)) = (obj, object_to_capture) else { return Ok(false) };

    // Make sure our object has the capability of performing this special ability.
    let is_owner_black_lotus = host.has_special_power(obj, SPECIAL_BLACKLOTUS_CAPTURE_BUILDING);

    if !host.has_special_power(obj, SPECIAL_INFANTRY_CAPTURE_BUILDING) && !is_owner_black_lotus {
        return Ok(false);
    }

    if host.is_kind_of(object_to_capture, KINDOF_IMMUNE_TO_CAPTURE) {
        return Ok(false);
    }

    let mut sp_percent = host.special_power_percent_ready_by_type(obj, SPECIAL_INFANTRY_CAPTURE_BUILDING);
    if sp_percent.is_none() {
        sp_percent = host.special_power_percent_ready_by_type(obj, SPECIAL_BLACKLOTUS_CAPTURE_BUILDING);
    }
    let Some(percent) = sp_percent else { return Ok(false) };

    if percent < 1.0 {
        // Special not ready or non-existent.
        return Ok(false);
    }

    // can't capture dead things.
    if host.is_effectively_dead(object_to_capture) {
        return Ok(false);
    }

    // Make sure we are targeting a building!
    if !host.is_kind_of(object_to_capture, KINDOF_STRUCTURE) {
        return Ok(false);
    }

    // can't capture things that are under construction, or sold.
    if host.test_status(object_to_capture, OBJECT_STATUS_UNDER_CONSTRUCTION) || host.test_status(object_to_capture, OBJECT_STATUS_SOLD) {
        return Ok(false);
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_capture, command_source) {
        return Ok(false);
    }

    let r = host.relationship(obj, object_to_capture);

    // ensure that it's capturable, and not allied
    // exception: we can always capture enemy bldgs, regardless of kindof
    if !(r == Relationship::Enemies || (host.is_kind_of(object_to_capture, KINDOF_CAPTURABLE) && r != Relationship::Allies)) {
        return Ok(false);
    }

    // If the enemy unit is stealthed and not detected, then we can't capture it!
    if is_hidden_by_stealth(host, object_to_capture) {
        return Ok(false);
    }

    // if it's garrisoned already, we cannot capture it. (unless it's just stealth-garrisoned.)
    if let Some(contain) = host.contain_facts(object_to_capture) {
        if contain.garrisonable {
            let contain_count = contain.count;
            let stealth_contain_count = host.contain_stealth_units_contained(object_to_capture);
            let non_stealth_contain_count = contain_count - stealth_contain_count;
            if non_stealth_contain_count > 0 {
                return Ok(false);
            }
        }
    }

    // Also check if the object is a container containing stealth units, tricking the player into thinking it isn't
    // actually an enemy.
    if appears_to_contain_friendlies(host, obj, object_to_capture)? {
        return Ok(false);
    }

    Ok(true)
}

/// `ActionManager::canDisableVehicleViaHacking(obj, objectToHack, commandSource, checkSourceRequirements)`.
pub fn can_disable_vehicle_via_hacking<H: ActionHost>(
    host: &H,
    obj: Option<ObjectId>,
    object_to_hack: Option<ObjectId>,
    command_source: CommandSourceType,
    check_source_requirements: bool,
) -> AResult<bool> {
    // sanity
    let (Some(obj), Some(object_to_hack)) = (obj, object_to_hack) else { return Ok(false) };

    if check_source_requirements {
        // Make sure our object has the capability of performing this special ability.
        if !host.has_special_power(obj, SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK) {
            return Ok(false);
        }
    }

    let sp_percent = host.special_power_percent_ready_by_type(obj, SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK);
    if check_source_requirements {
        match sp_percent {
            Some(p) if p >= 1.0 => {}
            _ => {
                // Special not ready or non-existent.
                return Ok(false);
            }
        }
    }

    if host.is_effectively_dead(object_to_hack) {
        return Ok(false);
    }

    if host.is_kind_of(object_to_hack, KINDOF_AIRCRAFT) || host.is_airborne_target(object_to_hack) {
        return Ok(false);
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_hack, command_source) {
        return Ok(false);
    }

    let r = host.relationship(obj, object_to_hack);

    // Make sure object is an enemy
    if r == Relationship::Enemies {
        // Make sure we are targeting a building!
        if !host.is_kind_of(object_to_hack, KINDOF_VEHICLE) {
            return Ok(false);
        }

        // If the enemy unit is stealthed and not detected, then we can't attack it!
        if is_hidden_by_stealth(host, object_to_hack) {
            return Ok(false);
        }

        // Also check if the object is a container containing stealth units tricking the player into thinking it
        // isn't actually an enemy.
        if appears_to_contain_friendlies(host, obj, object_to_hack)? {
            return Ok(false);
        }

        return Ok(true);
    }
    Ok(false)
}

/// `ActionManager::canStealCashViaHacking(obj, objectToHack, commandSource)`.
pub fn can_steal_cash_via_hacking<H: ActionHost>(host: &H, obj: Option<ObjectId>, object_to_hack: Option<ObjectId>, command_source: CommandSourceType) -> AResult<bool> {
    // sanity
    let (Some(obj), Some(object_to_hack)) = (obj, object_to_hack) else { return Ok(false) };

    // Make sure our object has the capability of performing this special ability.
    if !host.has_special_power(obj, SPECIAL_BLACKLOTUS_STEAL_CASH_HACK) {
        return Ok(false);
    }

    match host.special_power_percent_ready_by_type(obj, SPECIAL_BLACKLOTUS_STEAL_CASH_HACK) {
        Some(p) if p >= 1.0 => {}
        _ => {
            // Special not ready or non-existent.
            return Ok(false);
        }
    }

    if host.is_effectively_dead(object_to_hack) {
        return Ok(false);
    }

    if host.test_status(object_to_hack, OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return Ok(false);
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_hack, command_source) {
        return Ok(false);
    }

    let r = host.relationship(obj, object_to_hack);

    // Make sure object is an enemy
    if r == Relationship::Enemies {
        // Make sure we are targeting something that contains cash!
        if !host.is_kind_of(object_to_hack, KINDOF_CASH_GENERATOR) {
            return Ok(false);
        }

        // Make sure object isn't under construction!
        if host.test_status(object_to_hack, OBJECT_STATUS_UNDER_CONSTRUCTION) {
            return Ok(false);
        }

        // Make sure the building is considered hackable (temp: using capturable)
        if !host.is_kind_of(object_to_hack, KINDOF_CAPTURABLE) || host.is_kind_of(object_to_hack, KINDOF_REBUILD_HOLE) {
            return Ok(false);
        }

        // If the enemy unit is stealthed and not detected, then we can't attack it!
        if is_hidden_by_stealth(host, object_to_hack) {
            return Ok(false);
        }

        // Also check if the object is a container containing stealth units tricking the player into thinking it
        // isn't actually an enemy.
        if appears_to_contain_friendlies(host, obj, object_to_hack)? {
            return Ok(false);
        }

        return Ok(true);
    }
    Ok(false)
}

/// `ActionManager::canDisableBuildingViaHacking(obj, objectToHack, commandSource)`.
pub fn can_disable_building_via_hacking<H: ActionHost>(
    host: &H,
    obj: Option<ObjectId>,
    object_to_hack: Option<ObjectId>,
    command_source: CommandSourceType,
) -> AResult<bool> {
    // sanity
    let (Some(obj), Some(object_to_hack)) = (obj, object_to_hack) else { return Ok(false) };

    // Make sure our object has the capability of performing this special ability.
    if !host.has_special_power(obj, SPECIAL_HACKER_DISABLE_BUILDING) {
        return Ok(false);
    }

    match host.special_power_percent_ready_by_type(obj, SPECIAL_HACKER_DISABLE_BUILDING) {
        Some(p) if p >= 1.0 => {}
        _ => {
            // Special not ready or non-existent.
            return Ok(false);
        }
    }

    if host.is_effectively_dead(object_to_hack) {
        return Ok(false);
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_hack, command_source) {
        return Ok(false);
    }

    let r = host.relationship(obj, object_to_hack);

    // Make sure object is an enemy
    if r != Relationship::Enemies {
        return Ok(false);
    }

    // Make sure we are targeting a building!
    if !host.is_kind_of(object_to_hack, KINDOF_STRUCTURE) {
        return Ok(false);
    }

    // Make sure the building is considered hackable (temp: using capturable)
    // An exception is any TechFactionBuilding that is not explicitly immune to capture
    if (!host.is_kind_of(object_to_hack, KINDOF_CAPTURABLE) || host.is_kind_of(object_to_hack, KINDOF_REBUILD_HOLE))
        && !(host.is_kind_of(object_to_hack, KINDOF_FS_TECHNOLOGY) && !host.is_kind_of(object_to_hack, KINDOF_IMMUNE_TO_CAPTURE))
    {
        return Ok(false);
    }

    if host.is_kind_of(object_to_hack, KINDOF_REBUILD_HOLE) || host.test_status(object_to_hack, OBJECT_STATUS_UNDER_CONSTRUCTION) {
        return Ok(false);
    }

    // If the enemy unit is stealthed and not detected, then we can't attack it!
    if is_hidden_by_stealth(host, object_to_hack) {
        return Ok(false);
    }

    // Also check if the object is a container containing stealth units tricking the player into thinking it isn't
    // actually an enemy.
    if appears_to_contain_friendlies(host, obj, object_to_hack)? {
        return Ok(false);
    }

    Ok(true)
}

/// `ActionManager::canBribeUnit`: always false.
pub fn can_bribe_unit(_obj: Option<ObjectId>, _object_to_bribe: Option<ObjectId>, _command_source: CommandSourceType) -> bool {
    false
}

/// `ActionManager::canCutBuildingPower`: always false.
pub fn can_cut_building_power(_obj: Option<ObjectId>, _building: Option<ObjectId>, _command_source: CommandSourceType) -> bool {
    false
}

/// `ActionManager::canSnipeVehicle(obj, objectToSnipe, commandSource)`.
pub fn can_snipe_vehicle<H: ActionHost>(host: &H, obj: Option<ObjectId>, object_to_snipe: Option<ObjectId>, command_source: CommandSourceType) -> bool {
    // Sanity check
    let (Some(obj), Some(object_to_snipe)) = (obj, object_to_snipe) else { return false };

    // Make sure it's alive.
    if host.is_effectively_dead(object_to_snipe) {
        return false;
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, object_to_snipe, command_source) {
        return false;
    }

    let r = host.relationship(obj, object_to_snipe);

    if r == Relationship::Enemies {
        // Make sure target is a vehicle.
        if !host.is_kind_of(object_to_snipe, KINDOF_VEHICLE) {
            return false;
        }

        // Can't be a drone type.
        if host.is_kind_of(object_to_snipe, KINDOF_DRONE) {
            return false;
        }

        // Make sure object is not flying
        if host.is_airborne_target(object_to_snipe) {
            return false;
        }

        // Make sure the vehicle is manned!
        if host.is_disabled_by_type(object_to_snipe, DISABLED_UNMANNED) {
            return false;
        }

        return true;
    }

    false
}

/// `isPointOnMap(testPos)`.
fn is_point_on_map<H: ActionHost>(host: &H, test_pos: &Coord3D) -> bool {
    let map_region = host.terrain_extent();
    map_region.is_in_region_no_z(test_pos)
}

/// `ActionManager::canDoSpecialPowerAtLocation(obj, loc, commandSource, spTemplate, objectInWay, commandOptions,
/// checkSourceRequirements)`.
// The eight parameters are upstream's (`canDoSpecialPowerAtLocation`, one overload).
#[allow(clippy::too_many_arguments)]
pub fn can_do_special_power_at_location<H: ActionHost>(
    host: &H,
    obj: ObjectId,
    loc: &Coord3D,
    _command_source: CommandSourceType,
    sp_template: &SpecialPowerTemplateRef,
    _object_in_way: Option<ObjectId>,
    _command_options: u32,
    check_source_requirements: bool,
) -> AResult<bool> {
    if check_source_requirements {
        // First check, if our object can do this special power.
        if !host.has_special_power(obj, sp_template.sp_type) {
            return Ok(false);
        }
    }

    let Some(percent) = host.special_power_percent_ready_by_template(obj, sp_template) else { return Ok(false) };
    if check_source_requirements && percent < 1.0 {
        // Not fully ready
        return Ok(false);
    }

    // First check terrain type, if it is cared about. Don't return a true, since there are more checks.
    match sp_template.sp_type {
        SPECIAL_PARADROP_AMERICA | INFA_SPECIAL_PARADROP_AMERICA | SPECIAL_CRATE_DROP | SPECIAL_TANK_PARADROP if host.is_underwater(loc.x, loc.y) => {
            return Ok(false);
        }
        _ => {}
    }

    // Last check is shroudedness, if it is cared about
    match sp_template.sp_type {
        SPECIAL_DAISY_CUTTER
        | AIRF_SPECIAL_DAISY_CUTTER
        | SPECIAL_PARADROP_AMERICA
        | SPECIAL_TANK_PARADROP
        | INFA_SPECIAL_PARADROP_AMERICA
        | SPECIAL_CARPET_BOMB
        | SPECIAL_CHINA_CARPET_BOMB
        | SPECIAL_LEAFLET_DROP
        | EARLY_SPECIAL_LEAFLET_DROP
        | EARLY_SPECIAL_CHINA_CARPET_BOMB
        | AIRF_SPECIAL_CARPET_BOMB
        | SUPR_SPECIAL_CRUISE_MISSILE
        | SPECIAL_CLUSTER_MINES
        | NUKE_SPECIAL_CLUSTER_MINES
        | SPECIAL_EMP_PULSE
        | SPECIAL_CRATE_DROP
        | SPECIAL_NAPALM_STRIKE
        | SPECIAL_BLACK_MARKET_NUKE
        | SPECIAL_ANTHRAX_BOMB
        | SPECIAL_TERROR_CELL
        | SPECIAL_AMBUSH
        | SPECIAL_NEUTRON_MISSILE
        | NUKE_SPECIAL_NEUTRON_MISSILE
        | SUPW_SPECIAL_NEUTRON_MISSILE
        | SPECIAL_SCUD_STORM
        | SPECIAL_A10_THUNDERBOLT_STRIKE
        | AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE
        | SPECIAL_SPECTRE_GUNSHIP
        | AIRF_SPECIAL_SPECTRE_GUNSHIP
        | SPECIAL_REPAIR_VEHICLES
        | EARLY_SPECIAL_REPAIR_VEHICLES
        | SPECIAL_GPS_SCRAMBLER
        | SLTH_SPECIAL_GPS_SCRAMBLER
        | SPECIAL_ARTILLERY_BARRAGE
        | SPECIAL_FRENZY
        | EARLY_SPECIAL_FRENZY
        | SPECIAL_PARTICLE_UPLINK_CANNON
        | SUPW_SPECIAL_PARTICLE_UPLINK_CANNON
        | LAZR_SPECIAL_PARTICLE_UPLINK_CANNON
        | SPECIAL_CLEANUP_AREA
        | SPECIAL_SNEAK_ATTACK
        | SPECIAL_BATTLESHIP_BOMBARDMENT => {
            // Don't allow "damaging" special powers in shrouded areas, but Fogged are okay.
            let player = host.controlling_player(obj).ok_or(WeaponError::NullObject)?;
            Ok(host.cell_shroud_status_for_player(player, loc) != CellShroudStatus::Shrouded)
        }

        SPECIAL_SPY_SATELLITE | SPECIAL_RADAR_VAN_SCAN | SPECIAL_SPY_DRONE | SPECIAL_HELIX_NAPALM_BOMB => {
            // These specials can be used anywhere!
            Ok(is_point_on_map(host, loc))
        }
        SPECIAL_LAUNCH_BAIKONUR_ROCKET => Ok(true),

        // These special powers require object targets!
        SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES
        | SPECIAL_HACKER_DISABLE_BUILDING
        | SPECIAL_TANKHUNTER_TNT_ATTACK
        | SPECIAL_BOOBY_TRAP
        | SPECIAL_CASH_HACK
        | SPECIAL_DEFECTOR
        | SPECIAL_BLACKLOTUS_CAPTURE_BUILDING
        | SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK
        | SPECIAL_BLACKLOTUS_STEAL_CASH_HACK
        | SPECIAL_INFANTRY_CAPTURE_BUILDING
        | SPECIAL_DETONATE_DIRTY_NUKE
        | SPECIAL_DISGUISE_AS_VEHICLE
        | SPECIAL_REMOTE_CHARGES
        | SPECIAL_TIMED_CHARGES
        | SPECIAL_CASH_BOUNTY
        | SPECIAL_CHANGE_BATTLE_PLANS => Ok(false),

        _ => Ok(false),
    }
}

/// `ActionManager::canDoSpecialPowerAtObject(obj, target, commandSource, spTemplate, commandOptions,
/// checkSourceRequirements)`.
pub fn can_do_special_power_at_object<H: ActionHost>(
    host: &H,
    obj: ObjectId,
    target: ObjectId,
    command_source: CommandSourceType,
    sp_template: &SpecialPowerTemplateRef,
    _command_options: u32,
    check_source_requirements: bool,
) -> AResult<bool> {
    if check_source_requirements {
        // First check, if our object can do this special power.
        if !host.has_special_power(obj, sp_template.sp_type) {
            return Ok(false);
        }
    }

    if host.is_effectively_dead(target) {
        return Ok(false);
    }

    let r = host.relationship(obj, target);

    let Some(percent) = host.special_power_percent_ready_by_template(obj, sp_template) else { return Ok(false) };
    if check_source_requirements && percent < 1.0 {
        // Not fully ready
        return Ok(false);
    }

    // if the target is in the shroud, we can't do anything
    if is_object_shrouded_for_action(host, obj, target, command_source) {
        return Ok(false);
    }

    match sp_template.sp_type {
        SPECIAL_CASH_BOUNTY => return Ok(false),

        SPECIAL_BATTLESHIP_BOMBARDMENT => {
            if host.relationship(obj, target) != Relationship::Allies {
                return Ok(true);
            }
            return Ok(false);
        }

        SPECIAL_TANKHUNTER_TNT_ATTACK => {
            if host.is_kind_of(target, KINDOF_STRUCTURE) || (host.is_kind_of(target, KINDOF_VEHICLE) && !host.is_kind_of(target, KINDOF_AIRCRAFT)) {
                return Ok(true);
            }
        }

        SPECIAL_BOOBY_TRAP => {
            // We can booby trap any building that is allied or neutral
            if host.is_kind_of(target, KINDOF_STRUCTURE) && (r == Relationship::Neutral || r == Relationship::Allies) {
                return Ok(true);
            }
            return Ok(false);
        }

        SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES => {
            // Can only use laser guided missiles on vehicles!
            if host.is_kind_of(target, KINDOF_VEHICLE) && r == Relationship::Enemies {
                return Ok(true);
            }
        }

        SPECIAL_HACKER_DISABLE_BUILDING => {
            // Can only disable buildings...
            if host.is_kind_of(target, KINDOF_STRUCTURE) && r == Relationship::Enemies {
                // Make sure the building is considered hackable (temp: using capturable)
                if !host.is_kind_of(target, KINDOF_CAPTURABLE) || host.is_kind_of(target, KINDOF_REBUILD_HOLE) {
                    return Ok(false);
                }
                return Ok(true);
            }
        }

        SPECIAL_INFANTRY_CAPTURE_BUILDING | SPECIAL_BLACKLOTUS_CAPTURE_BUILDING => {
            return can_capture_building(host, Some(obj), Some(target), command_source);
        }

        SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK => {
            return can_disable_vehicle_via_hacking(host, Some(obj), Some(target), command_source, false);
        }

        SPECIAL_BLACKLOTUS_STEAL_CASH_HACK => {
            return can_steal_cash_via_hacking(host, Some(obj), Some(target), command_source);
        }

        SPECIAL_CASH_HACK => {
            // Can only disable enemy supply centers.
            if host.is_kind_of(target, KINDOF_STRUCTURE) && r == Relationship::Enemies {
                // Make sure the building is considered hackable (temp: using capturable)
                if !host.is_kind_of(target, KINDOF_CAPTURABLE) || host.is_kind_of(target, KINDOF_REBUILD_HOLE) {
                    return Ok(false);
                }

                // Can't cash hack a building that's under construction.
                if host.test_status(target, OBJECT_STATUS_UNDER_CONSTRUCTION) {
                    return Ok(false);
                }

                if host.is_kind_of(target, KINDOF_CASH_GENERATOR) {
                    return Ok(true);
                }
            }
        }

        SPECIAL_DISGUISE_AS_VEHICLE => {
            if host.is_kind_of(target, KINDOF_VEHICLE)
                && !host.is_kind_of(target, KINDOF_AIRCRAFT)
                && !host.is_kind_of(target, KINDOF_BOAT)
                && !host.is_kind_of(target, KINDOF_CLIFF_JUMPER)
            {
                // Don't allow it to disguise as another bomb truck -- that's just plain dumb.
                // Don't allow it to disguise as a train -- they don't have KINDOF_TRAIN yet, but if added, please
                // change this code so it'll be faster!
                if !host.has_named_update_module(target, NamedUpdateModule::Railroad) {
                    return Ok(true);
                }
            }
        }

        SPECIAL_DEFECTOR => {
            // buildings do not defect
            if !host.is_kind_of(target, KINDOF_STRUCTURE) {
                // only attacking type things will defect (no dozers, supply trucks, workers...)
                // neutral or same-team units are worthless defectors
                if r == Relationship::Enemies {
                    return Ok(can_make_object_defector(host, Some(obj), Some(target), command_source));
                }
            }
        }

        // These special powers require locations, not objects!
        SPECIAL_DAISY_CUTTER
        | AIRF_SPECIAL_DAISY_CUTTER
        | SPECIAL_PARADROP_AMERICA
        | SPECIAL_TANK_PARADROP
        | INFA_SPECIAL_PARADROP_AMERICA
        | SPECIAL_CARPET_BOMB
        | SPECIAL_CHINA_CARPET_BOMB
        | SPECIAL_LEAFLET_DROP
        | EARLY_SPECIAL_LEAFLET_DROP
        | EARLY_SPECIAL_CHINA_CARPET_BOMB
        | AIRF_SPECIAL_CARPET_BOMB
        | SUPR_SPECIAL_CRUISE_MISSILE
        | SPECIAL_CLUSTER_MINES
        | NUKE_SPECIAL_CLUSTER_MINES
        | SPECIAL_EMP_PULSE
        | SPECIAL_CRATE_DROP
        | SPECIAL_NAPALM_STRIKE
        | SPECIAL_TERROR_CELL
        | SPECIAL_AMBUSH
        | SPECIAL_NEUTRON_MISSILE
        | NUKE_SPECIAL_NEUTRON_MISSILE
        | SUPW_SPECIAL_NEUTRON_MISSILE
        | SPECIAL_DETONATE_DIRTY_NUKE
        | SPECIAL_BLACK_MARKET_NUKE
        | SPECIAL_ANTHRAX_BOMB
        | SPECIAL_SPY_SATELLITE
        | SPECIAL_SPY_DRONE
        | SPECIAL_RADAR_VAN_SCAN
        | SPECIAL_SCUD_STORM
        | SPECIAL_A10_THUNDERBOLT_STRIKE
        | AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE
        | SPECIAL_SPECTRE_GUNSHIP
        | AIRF_SPECIAL_SPECTRE_GUNSHIP
        | SPECIAL_ARTILLERY_BARRAGE
        | SPECIAL_FRENZY
        | EARLY_SPECIAL_FRENZY
        | SPECIAL_REPAIR_VEHICLES
        | EARLY_SPECIAL_REPAIR_VEHICLES
        | SPECIAL_GPS_SCRAMBLER
        | SLTH_SPECIAL_GPS_SCRAMBLER
        | SPECIAL_PARTICLE_UPLINK_CANNON
        | SPECIAL_CHANGE_BATTLE_PLANS
        | SPECIAL_CLEANUP_AREA
        | SPECIAL_LAUNCH_BAIKONUR_ROCKET
        | SPECIAL_SNEAK_ATTACK => return Ok(false),

        SPECIAL_REMOTE_CHARGES | SPECIAL_TIMED_CHARGES | SPECIAL_HELIX_NAPALM_BOMB => {
            if host.is_effectively_dead(target) || host.is_kind_of(target, KINDOF_BRIDGE) || host.is_kind_of(target, KINDOF_BRIDGE_TOWER) {
                return Ok(false);
            }

            if host.is_kind_of(target, KINDOF_STRUCTURE) || host.is_kind_of(target, KINDOF_VEHICLE) {
                if let Some(sp_update) = host.special_ability_update(obj, sp_template.sp_type) {
                    // Make sure we have enough equipment to place an additional charge.
                    if sp_update.object_count < sp_update.object_max {
                        // Also restrict the unit from placing more than one charge on the same building. We
                        // accomplish this by having the stickybomb update store the target as the producer ID.
                        if host.special_ability_has_object_with_producer(obj, sp_template.sp_type, target) {
                            return Ok(false);
                        }

                        // HERE'S THE TOUGH CASE...
                        // We also don't want to allow a unit that can place timed charges on a building to be able
                        // to place remote charges (or vice-versa). So we're going to look for the other special
                        // ability update and reject if the other one has it planted...
                        let other_type = if sp_template.sp_type == SPECIAL_REMOTE_CHARGES {
                            Some(SPECIAL_TIMED_CHARGES)
                        } else if sp_template.sp_type == SPECIAL_TIMED_CHARGES {
                            Some(SPECIAL_REMOTE_CHARGES)
                        } else {
                            None
                        };
                        // `spUpdate` keeps the first update when the type is neither
                        let other_has = match other_type {
                            Some(t) => host.special_ability_update(obj, t).is_some() && host.special_ability_has_object_with_producer(obj, t, target),
                            None => host.special_ability_has_object_with_producer(obj, sp_template.sp_type, target),
                        };

                        // If we have a valid pointer at this point, we found the other special. Make sure it isn't
                        // planted on the same target.
                        if other_has {
                            return Ok(false);
                        }

                        return Ok(true);
                    }
                }
            }
        }

        _ => {}
    }
    Ok(false)
}

/// `ActionManager::canDoSpecialPower(obj, spTemplate, commandSource, commandOptions, checkSourceRequirements)`.
pub fn can_do_special_power<H: ActionHost>(
    host: &H,
    obj: ObjectId,
    sp_template: &SpecialPowerTemplateRef,
    _command_source: CommandSourceType,
    _command_options: u32,
    check_source_requirements: bool,
) -> bool {
    if check_source_requirements {
        // First check, if our object can do this special power.
        if !host.has_special_power(obj, sp_template.sp_type) {
            return false;
        }
    }

    let Some(percent) = host.special_power_percent_ready_by_template(obj, sp_template) else { return false };
    if check_source_requirements && percent < 1.0 {
        // Not fully ready
        return false;
    }

    match sp_template.sp_type {
        SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES
        | SPECIAL_TANKHUNTER_TNT_ATTACK
        | SPECIAL_BOOBY_TRAP
        | SPECIAL_DAISY_CUTTER
        | AIRF_SPECIAL_DAISY_CUTTER
        | SPECIAL_PARADROP_AMERICA
        | SPECIAL_TANK_PARADROP
        | INFA_SPECIAL_PARADROP_AMERICA
        | SPECIAL_CARPET_BOMB
        | SPECIAL_CHINA_CARPET_BOMB
        | SPECIAL_LEAFLET_DROP
        | EARLY_SPECIAL_LEAFLET_DROP
        | EARLY_SPECIAL_CHINA_CARPET_BOMB
        | AIRF_SPECIAL_CARPET_BOMB
        | SUPR_SPECIAL_CRUISE_MISSILE
        | SPECIAL_CLUSTER_MINES
        | NUKE_SPECIAL_CLUSTER_MINES
        | SPECIAL_NAPALM_STRIKE
        | SPECIAL_TERROR_CELL
        | SPECIAL_NEUTRON_MISSILE
        | NUKE_SPECIAL_NEUTRON_MISSILE
        | SUPW_SPECIAL_NEUTRON_MISSILE
        | SPECIAL_BLACK_MARKET_NUKE
        | SPECIAL_ANTHRAX_BOMB
        | SPECIAL_SPY_SATELLITE
        | SPECIAL_SPY_DRONE
        | SPECIAL_RADAR_VAN_SCAN
        | SPECIAL_TIMED_CHARGES
        | SPECIAL_SCUD_STORM
        | SPECIAL_A10_THUNDERBOLT_STRIKE
        | AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE
        | SPECIAL_SPECTRE_GUNSHIP
        | AIRF_SPECIAL_SPECTRE_GUNSHIP
        | SPECIAL_ARTILLERY_BARRAGE
        | SPECIAL_FRENZY
        | EARLY_SPECIAL_FRENZY
        | SPECIAL_DISGUISE_AS_VEHICLE
        | SPECIAL_REPAIR_VEHICLES
        | EARLY_SPECIAL_REPAIR_VEHICLES
        | SPECIAL_GPS_SCRAMBLER
        | SLTH_SPECIAL_GPS_SCRAMBLER
        | SPECIAL_PARTICLE_UPLINK_CANNON
        | SPECIAL_CASH_BOUNTY
        | SPECIAL_CLEANUP_AREA
        | SPECIAL_HELIX_NAPALM_BOMB
        | SPECIAL_SNEAK_ATTACK
        | SPECIAL_EMP_PULSE
        | SPECIAL_CASH_HACK => {
            // These all require object or location targets.
            false
        }

        SPECIAL_REMOTE_CHARGES
        | SPECIAL_CIA_INTELLIGENCE
        | SPECIAL_COMMUNICATIONS_DOWNLOAD
        | SPECIAL_DETONATE_DIRTY_NUKE
        | SPECIAL_CHANGE_BATTLE_PLANS
        | SPECIAL_LAUNCH_BAIKONUR_ROCKET => {
            // Detonate's any existing charges
            true
        }

        _ => false,
    }
}

/// `ActionManager::canFireWeaponAtLocation(obj, loc, commandSource, slot, objectInWay)`.
pub fn can_fire_weapon_at_location<H: ActionHost>(
    host: &H,
    obj: Option<ObjectId>,
    loc: Option<&Coord3D>,
    _command_source: CommandSourceType,
    slot: i32,
    _object_in_way: Option<ObjectId>,
) -> bool {
    // Sanity check
    let (Some(obj), Some(_)) = (obj, loc) else { return false };

    // Make sure we have the right weapon.
    host.weapon_set(obj).get_weapon_in_weapon_slot(slot).is_some()
}

/// `ActionManager::canFireWeaponAtObject(obj, target, commandSource, slot)`.
pub fn can_fire_weapon_at_object<H: ActionHost>(
    host: &H,
    obj: Option<ObjectId>,
    target: Option<ObjectId>,
    command_source: CommandSourceType,
    slot: i32,
) -> AResult<bool> {
    // Sanity check
    let (Some(obj), Some(target)) = (obj, target) else { return Ok(false) };

    // Make sure we have the right weapon.
    let Some(weapon) = host.weapon_set(obj).get_weapon_in_weapon_slot(slot) else { return Ok(false) };

    let mut sniper = false;
    if host.store().templates()[weapon.template()].damage_type == DAMAGE_KILLPILOT {
        if !can_snipe_vehicle(host, Some(obj), Some(target), command_source) {
            return Ok(false);
        }
        sniper = true;
    }

    let result = if sniper {
        ow::get_able_to_attack_specific_object(host, obj, ATTACK_NEW_TARGET, target, command_source, slot)?
    } else {
        ow::get_able_to_attack_specific_object(host, obj, ATTACK_NEW_TARGET, target, command_source, -1)?
    };

    if result == CanAttackResult::Possible || result == CanAttackResult::PossibleAfterMoving {
        return Ok(weapon.estimate_weapon_damage(host, obj, Some(target), None)? != 0.0);
    }
    Ok(false)
}

/// `ActionManager::canFireWeapon(obj, slot, commandSource)`.
pub fn can_fire_weapon<H: ActionHost>(host: &H, obj: Option<ObjectId>, slot: i32, _command_source: CommandSourceType) -> bool {
    // Sanity check
    let Some(obj) = obj else { return false };

    // Make sure we have the right weapon.
    host.weapon_set(obj).get_weapon_in_weapon_slot(slot).is_some()
}

/// `ActionManager::canGarrison(obj, target, commandSource)`.
pub fn can_garrison<H: ActionHost>(host: &H, obj: Option<ObjectId>, target: Option<ObjectId>, _command_source: CommandSourceType) -> AResult<bool> {
    let (Some(obj), Some(target)) = (obj, target) else { return Ok(false) };

    // The object was not an infantry, or is disallowed from being allowed to garrison stuff.
    if !host.is_kind_of(obj, KINDOF_INFANTRY) || host.is_kind_of(obj, KINDOF_NO_GARRISON) {
        return Ok(false);
    }

    if !host.is_kind_of(target, KINDOF_STRUCTURE) {
        return Ok(false);
    }

    if !host.has_contain(obj) {
        return Ok(false);
    }

    let Some(cmi) = host.contain_facts(target) else { return Ok(false) };

    if !cmi.garrisonable {
        return Ok(false);
    }

    if host.controlling_player(obj) == host.controlling_player(target) {
        return Ok(host.contain_is_valid_container_for(target, obj, true));
    }

    let player = host.controlling_player(obj).ok_or(WeaponError::NullObject)?;
    if host.player_relationship_to_team_of(player, target) == Relationship::Neutral {
        // needs to be empty if its not already ours.
        return Ok(cmi.count == 0 && host.contain_is_valid_container_for(target, obj, true));
    }

    Ok(false)
}

/// `ActionManager::canPlayerGarrison(player, target, commandSource)`.
pub fn can_player_garrison<H: ActionHost>(host: &H, player: Option<i32>, target: Option<ObjectId>, _command_source: CommandSourceType) -> AResult<bool> {
    let (Some(player), Some(target)) = (player, target) else { return Ok(false) };

    if host.is_effectively_dead(target) {
        return Ok(false);
    }

    if !host.is_kind_of(target, KINDOF_STRUCTURE) {
        return Ok(false);
    }

    let Some(cmi) = host.contain_facts(target) else { return Ok(false) };

    if !cmi.garrisonable {
        return Ok(false);
    }

    if host.controlling_player(target) == Some(player) {
        return Ok(true);
    }

    if host.player_relationship_to_team_of(player, target) == Relationship::Neutral {
        // needs to be empty if its not already ours.
        return Ok(cmi.count == 0);
    }

    Ok(false)
}

/// `ActionManager::canOverrideSpecialPowerDestination(obj, loc, spType, commandSource)`.
pub fn can_override_special_power_destination<H: ActionHost>(
    host: &H,
    obj: ObjectId,
    loc: &Coord3D,
    sp_type: SpecialPowerType,
    _command_source: CommandSourceType,
) -> AResult<bool> {
    if host.special_power_overridable_destination_active(obj, sp_type) {
        // But so long as it's not in the black areas of the map.
        let player = host.controlling_player(obj).ok_or(WeaponError::NullObject)?;
        return Ok(host.cell_shroud_status_for_player(player, loc) != CellShroudStatus::Shrouded);
    }
    Ok(false)
}
