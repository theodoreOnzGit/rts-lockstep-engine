// SPDX-License-Identifier: GPL-3.0-only
// Translation-owned file (this repository's own design, not upstream code).

//! What the partition manager reads from, and calls into, the rest of the
//! game: the two traits [`PartitionHost`] (everything the grid, the shroud
//! and the queries need) and [`PartitionFilterHost`] (the extra questions the
//! `PartitionFilter*` classes ask an object).
//!
//! Upstream reaches all of this through globals and `Object` pointers
//! (`ThePlayerList`, `TheTerrainLogic`, `TheGameLogic`, `TheGlobalData`,
//! `TheGhostObjectManager`, `TheAI`, `TheActionManager`, and the `Object`
//! methods). Objects are referenced by [`ObjectId`] and answered by a *host*,
//! because `Object` is translated in the same wave by another agent and
//! `Player`, `Team`, the pathfinder and the body/contain modules are later
//! layers. Every method names the upstream call it replaces. The traits are
//! used as generic bounds (static dispatch, no trait objects).
//!
//! The host's `&mut self` methods are the re-entrant callbacks
//! (`onPartitionCellChange`, `onCollide`, ghost creation, random numbers):
//! they receive the manager (`&mut PartitionManager`) because upstream's
//! callee calls straight back into `ThePartitionManager` (a collision handler
//! unregisters objects, a cell change looks and unlooks).

use super::collide::DistanceCalculationType;
use super::manager::PartitionManager;
use crate::common::base_type::{Coord3D, Region3D};
use crate::common::game_common::{
    AbleToAttackType, CommandSourceType, PlayerMaskType, Relationship,
};
use crate::common::geometry::GeometryInfo;
use crate::common::kind_of::{KindOfMaskType, KindOfType};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::logic::world::ids::{ObjectId, TeamId};

/// An object's position, orientation and shape: `getPosition()`,
/// `getOrientation()`, `getGeometryInfo()`.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectGeometry {
    /// `getPosition()`.
    pub position: Coord3D,
    /// `getOrientation()`.
    pub orientation: f32,
    /// `getGeometryInfo()`.
    pub geometry: GeometryInfo,
}

/// The data a `GhostObject` holds that the partition manager reads
/// (`getGeometryType`, `getGeometrySmall`, `getGeometryMajorRadius`,
/// `getGeometryMinorRadius`, `getParentPosition`, `getParentAngle`). The
/// ghost itself (its snapshots, its drawables) is client-side.
#[derive(Clone, Debug, PartialEq)]
pub struct GhostGeometry {
    /// Opaque handle the host gave the ghost; the manager only hands it back
    /// in its client events.
    pub handle: u32,
    /// `getParentPosition()`.
    pub position: Coord3D,
    /// `getParentAngle()`.
    pub angle: f32,
    /// Type, smallness and radii (`height` is not read by the manager).
    pub geometry: GeometryInfo,
}

/// `CanAttackResult` (`Object.h`): only the two "possible" answers matter to
/// `PartitionFilterPossibleToAttack`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanAttackResult {
    /// `ATTACKRESULT_NOT_POSSIBLE`.
    NotPossible,
    /// `ATTACKRESULT_POSSIBLE`.
    Possible,
    /// `ATTACKRESULT_POSSIBLE_AFTER_MOVING`.
    PossibleAfterMoving,
    /// Any other refusal upstream distinguishes (invalid shot, etc.).
    OtherRefusal,
}

/// What a stealthed object's `StealthUpdate` says (`PartitionFilterStealthedAndUndetected`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StealthFacts {
    /// `update->isDisguised()`.
    pub is_disguised: bool,
    /// `update->getDisguisedPlayerIndex()`.
    pub disguised_player_index: i32,
}

/// What a container's `ContainModuleInterface` says
/// (`PartitionFilterStealthedAndUndetected`, `RejectBuildings`,
/// `InsignificantBuildings`, `Garrisonable`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainFacts {
    /// `isGarrisonable()`.
    pub is_garrisonable: bool,
    /// `getContainCount()`.
    pub contain_count: i32,
    /// `getStealthUnitsContained()`.
    pub stealth_units_contained: i32,
    /// For the first contained item: `Some(detected)` where `detected` is
    /// `OBJECT_STATUS_DETECTED` on it; `None` when the list is empty or its
    /// first entry is null.
    pub first_contained_detected: Option<bool>,
}

/// The `Cell` types of the pathfinder grid `tryPosition` asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathfindCellType {
    /// `PathfindCell::CELL_CLEAR`.
    Clear,
    /// `PathfindCell::CELL_IMPASSABLE`.
    Impassable,
    /// Any other cell type.
    Other,
}

/// `PathfindLayerEnum` values `tryPosition` distinguishes (`LAYER_GROUND` and
/// the rest, as the host numbers them; `LAYER_GROUND` is 1 upstream).
pub type PathfindLayer = i32;
/// `LAYER_GROUND`.
pub const LAYER_GROUND: PathfindLayer = 1;

/// Everything the manager needs from the game. See the module docs.
pub trait PartitionHost {
    // ---- the clock and the global data -------------------------------------
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `TheGlobalData->m_partitionCellSize`.
    fn partition_cell_size(&self) -> f32;
    /// `TheGlobalData->m_unlookPersistDuration`.
    fn unlook_persist_duration(&self) -> u32;

    // ---- the terrain -------------------------------------------------------
    /// `TheTerrainLogic->getExtent(&extent)`; `None` when there is no
    /// `TheTerrainLogic` (the manager then stays empty).
    fn terrain_extent(&self) -> Option<Region3D>;
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn ground_height(&self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->isClearLineOfSight(pos, posOther)`.
    fn is_clear_line_of_sight(&self, pos: &Coord3D, pos_other: &Coord3D) -> bool;

    // ---- the players -------------------------------------------------------
    /// `ThePlayerList->getPlayerCount()`.
    fn player_count(&self) -> i32;
    /// `ThePlayerList->getNthPlayer(i)->getPlayerMask()`; `None` for a null player.
    fn player_mask(&self, index: i32) -> Option<PlayerMaskType>;
    /// `ThePlayerList->getLocalPlayer()->getPlayerIndex()`; `None` if there is
    /// no local player.
    fn local_player_index(&self) -> Option<i32>;
    /// `ThePlayerList->getPlayersWithRelationship(playerIndex, whichPlayerTypes)`.
    fn players_with_relationship(
        &self,
        player_index: i32,
        which_player_types: u32,
    ) -> PlayerMaskType;
    /// `ThePlayerList->getNthPlayer(playerIndex)->getRelationship(object->getTeam())`.
    fn player_relationship_to_object_team(
        &self,
        player_index: i32,
        object: ObjectId,
    ) -> Relationship;

    // ---- the objects -------------------------------------------------------
    /// Position, orientation and shape of an object; `None` if it does not exist.
    fn object_geometry(&self, id: ObjectId) -> Option<ObjectGeometry>;
    /// `obj->isKindOf(t)`.
    fn is_kind_of(&self, id: ObjectId, t: KindOfType) -> bool;
    /// `obj->getStatusBits()`.
    fn status_bits(&self, id: ObjectId) -> ObjectStatusMaskType;
    /// `obj->isDestroyed()`.
    fn is_destroyed(&self, id: ObjectId) -> bool;
    /// `obj->getUnitDirectionVector2D()`.
    fn unit_direction_vector_2d(&self, id: ObjectId) -> Coord3D;

    // ---- the ghost objects (client side; see the module docs of `manager`) ---
    /// The decision in `PartitionData::attachToObject` for a `KINDOF_IMMOBILE`
    /// object whose draw modules are not just `W3DDefaultDraw`:
    /// `TheGhostObjectManager->addGhostObject(object, this)`. `None` when no
    /// ghost is made (not immobile is decided by the manager; the draw-module
    /// test and the ghost manager's answer are the host's).
    fn add_ghost_object(&mut self, id: ObjectId) -> Option<GhostGeometry>;

    // ---- re-entrant callbacks ----------------------------------------------
    /// `obj->onPartitionCellChange()`: called by `updateCellsTouched` when the
    /// object's centre cell changed.
    fn on_partition_cell_change(&mut self, pm: &mut PartitionManager, id: ObjectId);
    /// `obj->onCollide(other, &loc, &normal)`.
    fn on_collide(
        &mut self,
        pm: &mut PartitionManager,
        id: ObjectId,
        other: ObjectId,
        loc: &Coord3D,
        normal: &Coord3D,
    );

    // ---- random numbers ----------------------------------------------------
    /// `GameLogicRandomValueReal(lo, hi)`.
    fn game_logic_random_value_real(&mut self, lo: f32, hi: f32) -> f32;

    // ---- the pathfinder (findPositionAround only) --------------------------
    /// `TheTerrainLogic->getMaximumPathfindExtent(&extent)`.
    fn maximum_pathfind_extent(&self) -> Region3D;
    /// `TheTerrainLogic->getHighestLayerForDestination(pos)`.
    fn highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayer;
    /// `TheTerrainLogic->getLayerHeight(x, y, layer)`.
    fn layer_height(&self, x: f32, y: f32, layer: PathfindLayer) -> f32;
    /// `TheTerrainLogic->isCliffCell(x, y)`.
    fn is_cliff_cell(&self, x: f32, y: f32) -> bool;
    /// `TheTerrainLogic->isUnderwater(x, y)`.
    fn is_underwater(&self, x: f32, y: f32) -> bool;
    /// `TheAI->pathfinder()->getCell(layer, cellX, cellY)->getType()`; `None`
    /// when the cell is null.
    fn pathfind_cell_type(
        &self,
        layer: PathfindLayer,
        cell_x: i32,
        cell_y: i32,
    ) -> Option<PathfindCellType>;
    /// `ai != NULL` for `options->sourceToPathToDest->getAIUpdateInterface()`,
    /// then `TheAI->pathfinder()->clientSafeQuickDoesPathExist(locomotorSet,
    /// source->getPosition(), &pos)`: `None` when the source has no AI
    /// update, else the answer.
    fn quick_does_path_exist(&self, source: ObjectId, to: &Coord3D) -> Option<bool>;
    /// `obj->getRelationship(other)`.
    fn relationship(&self, obj: ObjectId, other: ObjectId) -> Relationship;
}

/// The questions the partition filters ask an object. See the module docs.
pub trait PartitionFilterHost: PartitionHost {
    /// `obj->isUsingAirborneLocomotor()`.
    fn is_using_airborne_locomotor(&self, id: ObjectId) -> bool;
    /// `obj->getControllingPlayer()->getPlayerIndex()`; `None` for no controlling player.
    fn controlling_player(&self, id: ObjectId) -> Option<i32>;
    /// `obj->getTeam()` (by id; 0 = none).
    fn team(&self, id: ObjectId) -> TeamId;
    /// `squad->isOnSquad(obj)`.
    fn is_on_squad(&self, squad: i32, id: ObjectId) -> bool;
    /// `obj->isEffectivelyDead()`.
    fn is_effectively_dead(&self, id: ObjectId) -> bool;
    /// `obj->isOffMap()`.
    fn is_off_map(&self, id: ObjectId) -> bool;
    /// `TheAI && TheAI->pathfinder()->isViewBlockedByObstacle(obj, other)`.
    fn is_view_blocked_by_obstacle(&self, obj: ObjectId, other: ObjectId) -> bool;
    /// `obj->getAbleToAttackSpecificObject(type, other, source)`.
    fn able_to_attack_specific_object(
        &self,
        obj: ObjectId,
        attack_type: AbleToAttackType,
        other: ObjectId,
        source: CommandSourceType,
    ) -> CanAttackResult;
    /// `TheActionManager->canEnterObject(obj, other, source, DONT_CHECK_CAPACITY)`.
    fn can_enter_object(&self, obj: ObjectId, other: ObjectId, source: CommandSourceType) -> bool;
    /// `TheActionManager->canHijackVehicle(obj, other, source)`.
    fn can_hijack_vehicle(&self, obj: ObjectId, other: ObjectId, source: CommandSourceType)
        -> bool;
    /// `obj->getBodyModule()->getLastDamageInfo()->in.m_sourceID`, `None` when
    /// the object (or its body) is null.
    fn last_damage_source(&self, id: ObjectId) -> Option<ObjectId>;
    /// `obj->getKindOfMask()`: the template's kind mask, for `isKindOfMulti`.
    fn kind_of_mask(&self, id: ObjectId) -> KindOfMaskType;
    /// `obj->getTransformMatrix()->Get_X_Vector()`.
    fn transform_x_vector(&self, id: ObjectId) -> Coord3D;
    /// `obj->isStructure()`.
    fn is_structure(&self, id: ObjectId) -> bool;
    /// `obj->isNonFactionStructure()`.
    fn is_non_faction_structure(&self, id: ObjectId) -> bool;
    /// `obj->isAbleToAttack()`.
    fn is_able_to_attack(&self, id: ObjectId) -> bool;
    /// `obj->getContain()` as the facts the filters read; `None` if no container.
    fn contain_facts(&self, id: ObjectId) -> Option<ContainFacts>;
    /// `contain->getApparentControllingPlayer(player)`, as a player index;
    /// `None` when the container has none (or there is no container).
    fn apparent_controlling_player(
        &self,
        id: ObjectId,
        observer_player: Option<i32>,
    ) -> Option<i32>;
    /// `a->getRelationship(b->getDefaultTeam())` for two players.
    fn player_relationship_to_player(&self, a: i32, b: i32) -> Relationship;
    /// `m_self->getControllingPlayer()->getPlayerType() == PLAYER_COMPUTER`.
    fn controlling_player_is_computer(&self, id: ObjectId) -> bool;
    /// `obj->getStealth()`; `None` when the object has no `StealthUpdate`.
    fn stealth_facts(&self, id: ObjectId) -> Option<StealthFacts>;
    /// `m_obj->getTeam()->getRelationship(player->getDefaultTeam())`.
    fn team_relationship_to_player(&self, obj: ObjectId, player: i32) -> Relationship;
    /// `m_player->getRelationship(other->getTeam())` (player index).
    fn player_relationship_to_team_of(&self, player: i32, other: ObjectId) -> Relationship {
        self.player_relationship_to_object_team(player, other)
    }
    /// `m_tThing->isEquivalentTo(other->getTemplate())`, for a template named by `key`.
    fn template_is_equivalent_to(&self, template_key: i32, other: ObjectId) -> bool;
    /// `TheActionManager->canPlayerGarrison(player, other, source)`.
    fn can_player_garrison(
        &self,
        player: Option<i32>,
        other: ObjectId,
        source: CommandSourceType,
    ) -> bool;
    /// `other->isDisabledByType(DISABLED_UNMANNED)`.
    fn is_disabled_unmanned(&self, id: ObjectId) -> bool;
    /// `commandButton->isValidToUseOn(source, other, NULL, commandSource)`.
    fn command_button_valid_target(
        &self,
        button: i32,
        source: ObjectId,
        other: ObjectId,
        command_source: CommandSourceType,
    ) -> bool;
    /// `m_trigger->pointInTrigger(iPos)` for the trigger named by `trigger`.
    fn point_in_trigger(&self, trigger: i32, x: i32, y: i32) -> bool;
    /// `obj->getTemplate()->friend_getBuildCost()`, for the two cost sort orders.
    fn build_cost(&self, id: ObjectId) -> i32;
}

/// What a query needs to know about how distance is measured, kept here so the
/// host can name it in docs without importing the collide module.
pub type DistanceCalc = DistanceCalculationType;
