// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/BuildAssistant.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/BuildAssistant.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BuildAssistant` (`TheBuildAssistant`), wave 7, `p1w10-sonnet`: what it takes to place a structure or a line of walls
//! (the footprint samples, the objects in the way, the terrain and the shroud), to make a unit or a building at once,
//! to ask whether a builder may make something, and to sell a structure (the sell list and its countdown).
//!
//! # The shape
//!
//! The assistant's own data is [`BuildAssistant`] (the scratch array of line-build positions and the sell list); every
//! operation is a free function over a [`BuildAssistantHost`], the way `player_ops.rs` does `Player`. The host is what
//! `BuildAssistant.cpp` reaches and another unit owns: the objects and the thing templates, the player (as seven
//! forwarders to the verified `Player` functions), the terrain and its visual, the partition manager (its queries
//! answer in iteration order, which decides which failure is reported first), the pathfinder's cells, the AI, the
//! production, contain and parking-place interfaces, the control bar's command sets, the audio, and the game logic. The
//! layer-12 adapter answers them from the `World`.
//!
//! # Upstream quirks kept
//!
//! - **`isLocationClearOfObjects` tests `options == NO_ENEMY_OBJECT_OVERLAP`** (equality, not a bit test) for
//!   `onlyCheckEnemies`: the flag combined with any other makes the call check everything and return after the first loop
//!   only when it is the sole option.
//! - **`isLocationLegalToBuild` calls `isLocationClearOfObjects` twice** when both overlap flags are set (the second call
//!   repeats the first, and its feedback).
//! - **`isLocationLegalToBuild` dereferences `build`** (`build->isKindOf(...)`) whatever the options, although its comment
//!   says it may be null: the template is not optional here.
//! - **`canMakeUnit` compares `calcCostToBuild(player)` (an `Int`) with `countMoney()` (an `UnsignedInt`)**: a negative
//!   cost compares as a huge unsigned number and the player cannot afford it.
//! - **`moveObjectsForConstruction` computes its radius in `double`** (`sqrt(pow(a,2)+pow(b,2))` on `float`s promotes) and
//!   narrows to `float` before the `* 1.4f`.
//! - **A line build checks every tile's legality with `builderObject` and a `NULL` player**, so the skirmish-AI padding of
//!   `isLocationClearOfObjects` never applies to the tiles after the first.
//! - **`buildObjectNow` builds the first tile of a line like all the others**, and a failed tile does not stop the loop
//!   in `buildObjectLineNow` (only `buildTiledLocations` stops at an illegal position).
//! - **`m_buildPositions` is reallocated, not grown, when a line needs more tiles than it has** (the old contents are
//!   gone; nothing reads them).
//! - **The sell list is a `std::list` with `push_front`**: the newest sale is updated first.
//!
//! # Where upstream is undefined
//!
//! A builder without a controlling player passed to `isPossibleToMakeUnit`/`canMakeUnit` (the player is dereferenced),
//! and a player without a default team given to `buildObjectNow` (the new object gets a null team): [`BuildError::Undefined`],
//! and the differential never gives them to the C++.

use crate::common::base_type::{
    real_to_int, real_to_int_floor, real_to_unsigned_int, Coord2D, Coord3D, Region3D,
};
use crate::common::game_common::{CellShroudStatus, PlayerType, Relationship};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND};
use crate::common::geometry::{GeometryInfo, GeometryType};
use crate::common::kind_of::{
    KindOfType, KINDOF_ALWAYS_SELECTABLE, KINDOF_CANNOT_BUILD_NEAR_SUPPLIES,
    KINDOF_CLEARED_BY_BUILD, KINDOF_DOZER, KINDOF_IMMOBILE, KINDOF_INERT, KINDOF_LINEBUILD,
    KINDOF_MINE, KINDOF_SHRUBBERY, KINDOF_STRUCTURE,
};
use crate::common::model_state::{
    ModelConditionFlagType, MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED,
    MODELCONDITION_PARTIALLY_CONSTRUCTED, MODELCONDITION_SOLD,
};
use crate::common::object_status_types::{
    ObjectStatusTypes, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED,
    OBJECT_STATUS_IS_USING_ABILITY, OBJECT_STATUS_SOLD, OBJECT_STATUS_STEALTHED,
    OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_UNSELECTABLE,
};
use crate::common::xfer::{Xfer, XferMode, XferStatus};
use crate::logic::object_script_status_bits::{
    OBJECT_STATUS_SCRIPT_DISABLED, OBJECT_STATUS_SCRIPT_UNPOWERED,
};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

use super::ids::PlayerIndex;
use super::team_ops::TemplateRef;

/// `enum { CONSTRUCTION_COMPLETE = -1 }`.
pub const CONSTRUCTION_COMPLETE: i32 = -1;

/// `enum CanMakeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum CanMakeType {
    /// `CANMAKE_OK`.
    Ok = 0,
    /// `CANMAKE_NO_PREREQ`.
    NoPrereq = 1,
    /// `CANMAKE_NO_MONEY`.
    NoMoney = 2,
    /// `CANMAKE_FACTORY_IS_DISABLED`.
    FactoryIsDisabled = 3,
    /// `CANMAKE_QUEUE_FULL`: the production building has a full queue.
    QueueFull = 4,
    /// `CANMAKE_PARKING_PLACES_FULL`.
    ParkingPlacesFull = 5,
    /// `CANMAKE_MAXED_OUT_FOR_PLAYER`: the player has as many as it is allowed at once.
    MaxedOutForPlayer = 6,
}

/// `enum LegalBuildCode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum LegalBuildCode {
    /// `LBC_OK`.
    Ok = 0,
    /// `LBC_RESTRICTED_TERRAIN`.
    RestrictedTerrain = 1,
    /// `LBC_NOT_FLAT_ENOUGH`.
    NotFlatEnough = 2,
    /// `LBC_OBJECTS_IN_THE_WAY`.
    ObjectsInTheWay = 3,
    /// `LBC_NO_CLEAR_PATH`.
    NoClearPath = 4,
    /// `LBC_SHROUD`.
    Shroud = 5,
    /// `LBC_TOO_CLOSE_TO_SUPPLIES`.
    TooCloseToSupplies = 6,
    /// `LBC_GENERIC_FAILURE`.
    GenericFailure = 7,
}

/// `BuildAssistant::LocalLegalToBuildOptions`: check for basic terrain restrictions.
pub const TERRAIN_RESTRICTIONS: u32 = 0x0000_0001;
/// `CLEAR_PATH`: must be able to path find to the location.
pub const CLEAR_PATH: u32 = 0x0000_0002;
/// `NO_OBJECT_OVERLAP`: can't overlap enemy objects, or locally controlled objects that can't move out of the way.
pub const NO_OBJECT_OVERLAP: u32 = 0x0000_0004;
/// `USE_QUICK_PATHFIND`: use the quick pathfind method for `CLEAR_PATH`.
pub const USE_QUICK_PATHFIND: u32 = 0x0000_0008;
/// `SHROUD_REVEALED`: check that the shroud is revealed.
pub const SHROUD_REVEALED: u32 = 0x0000_0010;
/// `NO_ENEMY_OBJECT_OVERLAP`: can't overlap enemy objects only.
pub const NO_ENEMY_OBJECT_OVERLAP: u32 = 0x0000_0020;
/// `IGNORE_STEALTHED`: units that we can't see are legal to "build" on.
pub const IGNORE_STEALTHED: u32 = 0x0000_0040;
/// `FAIL_STEALTHED_WITHOUT_FEEDBACK`: with `IGNORE_STEALTHED`, fail without bib feedback.
pub const FAIL_STEALTHED_WITHOUT_FEEDBACK: u32 = 0x0000_0080;

/// `PATHFIND_CELL_SIZE` (`AIPathfind.h`).
pub const PATHFIND_CELL_SIZE: i32 = 10;
/// `PATHFIND_CELL_SIZE_F`.
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// `MAP_XY_FACTOR` (`MapObject.h`): how wide and tall each height map square is in world space.
pub const MAP_XY_FACTOR: f32 = 10.0;
/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: i32 = 30;
/// `FRAMES_TO_ALLOW_SCAFFOLD`.
const FRAMES_TO_ALLOW_SCAFFOLD: f32 = LOGICFRAMES_PER_SECOND as f32 * 1.5;
/// `TOTAL_FRAMES_TO_SELL_OBJECT`.
const TOTAL_FRAMES_TO_SELL_OBJECT: f32 = LOGICFRAMES_PER_SECOND as f32 * 3.0;

/// `GUICommandType`'s `GUI_COMMAND_DOZER_CONSTRUCT` (`ControlBar.h`).
pub const GUI_COMMAND_DOZER_CONSTRUCT: i32 = 1;
/// `GUI_COMMAND_UNIT_BUILD`.
pub const GUI_COMMAND_UNIT_BUILD: i32 = 3;
/// `MAX_COMMANDS_PER_SET` (`ControlBar.h`).
pub const MAX_COMMANDS_PER_SET: i32 = 18;

/// `PathfindCell::CellType` (`AIPathfind.h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PathfindCellType {
    /// `CELL_CLEAR`.
    Clear = 0,
    /// `CELL_WATER`.
    Water = 1,
    /// `CELL_CLIFF`.
    Cliff = 2,
    /// `CELL_RUBBLE`.
    Rubble = 3,
    /// `CELL_OBSTACLE`.
    Obstacle = 4,
    /// `CELL_BRIDGE_IMPASSABLE`.
    BridgeImpassable = 5,
    /// `CELL_IMPASSABLE`.
    Impassable = 6,
}

/// What `BuildAssistant` cannot continue after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// A null pointer dereferenced by upstream (undefined behaviour there); the text names it.
    Undefined(&'static str),
    /// What the xfer itself returned.
    Xfer(XferStatus),
}

impl BuildError {
    /// True for the errors that stand for undefined behaviour upstream, which a differential test must never send to the C++.
    pub fn is_undefined(&self) -> bool {
        matches!(self, BuildError::Undefined(_))
    }
}

impl From<XferStatus> for BuildError {
    fn from(s: XferStatus) -> BuildError {
        BuildError::Xfer(s)
    }
}

/// Results of the build assistant's operations.
pub type BResult<T> = Result<T, BuildError>;

/// The global data `BuildAssistant.cpp` reads (`TheGlobalData`), as one value.
#[derive(Clone, Copy, Debug)]
pub struct BuildGlobals {
    /// `m_maxLineBuildObjects`.
    pub max_line_build_objects: i32,
    /// `m_MinDistFromEdgeOfMapForBuild`.
    pub min_dist_from_edge_of_map_for_build: f32,
    /// `m_allowedHeightVariationForBuilding`.
    pub allowed_height_variation_for_building: f32,
    /// `m_SupplyBuildBorder`.
    pub supply_build_border: f32,
    /// `m_sellPercentage`.
    pub sell_percentage: f32,
}

/// `ObjectSellInfo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectSellInfo {
    /// `m_id`: the object being sold.
    pub id: ObjectId,
    /// `m_sellFrame`: the frame the sale started on.
    pub sell_frame: u32,
}

/// `BuildAssistant::TileBuildInfo`: the tiles used and (a copy of) the scratch array's positions.
#[derive(Clone, Debug, PartialEq)]
pub struct TileBuildInfo {
    /// `tilesUsed`.
    pub tiles_used: i32,
    /// `positions`: the first `tilesUsed` of them are meaningful.
    pub positions: Vec<Coord3D>,
}

/// `class BuildAssistant`'s own data.
#[derive(Clone, Debug, Default)]
pub struct BuildAssistant {
    /// `m_buildPositions`.
    pub build_positions: Vec<Coord3D>,
    /// `m_buildPositionSize`.
    pub build_position_size: i32,
    /// `m_sellList` (front first).
    pub sell_list: Vec<ObjectSellInfo>,
}

impl BuildAssistant {
    /// `BuildAssistant::BuildAssistant()`.
    pub fn new() -> BuildAssistant {
        BuildAssistant {
            build_positions: Vec::new(),
            build_position_size: 0,
            sell_list: Vec::new(),
        }
    }
}

/// What `BuildAssistant.cpp` asks of the rest of the world (see the module docs).
pub trait BuildAssistantHost {
    // ---- global state ---------------------------------------------------------------------------------------------
    /// `TheGlobalData`, the fields `BuildAssistant.cpp` reads.
    fn build_globals(&self) -> BuildGlobals;
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `GameLogicRandomValueReal(lo, hi)`.
    fn random_value_real(&mut self, lo: f32, hi: f32) -> f32;
    /// `TheGameLogic->findObjectByID(id)` (`None` for `NULL`).
    fn find_object_by_id(&self, id: ObjectId) -> Option<ObjectId>;
    /// `TheGameLogic->destroyObject(obj)`.
    fn destroy_object(&mut self, o: ObjectId);
    /// `TheGameLogic->deselectObject(obj, PLAYERMASK_ALL, TRUE)`.
    fn deselect_object(&mut self, o: ObjectId);
    /// The objects `TheGameLogic->getFirstObject()` / `getNextObject()` walk, in order.
    fn all_objects(&self) -> Vec<ObjectId>;
    // ---- objects --------------------------------------------------------------------------------------------------
    /// `obj->getTemplate()`.
    fn obj_template(&self, o: ObjectId) -> TemplateRef;
    /// `obj->isKindOf(k)`.
    fn obj_is_kind_of(&self, o: ObjectId, k: KindOfType) -> bool;
    /// `obj->testStatus(s)`.
    fn obj_test_status(&self, o: ObjectId, s: ObjectStatusTypes) -> bool;
    /// `obj->testScriptStatusBit(bit)`.
    fn obj_test_script_status_bit(&self, o: ObjectId, bit: u8) -> bool;
    /// `obj->isDisabled()`.
    fn obj_is_disabled(&self, o: ObjectId) -> bool;
    /// `obj->isEffectivelyDead()`.
    fn obj_is_effectively_dead(&self, o: ObjectId) -> bool;
    /// `obj->getGeometryInfo()` (a copy).
    fn obj_geometry(&self, o: ObjectId) -> GeometryInfo;
    /// `*obj->getPosition()`.
    fn obj_position(&self, o: ObjectId) -> Coord3D;
    /// `obj->getOrientation()`.
    fn obj_orientation(&self, o: ObjectId) -> f32;
    /// `obj->getControllingPlayer()`'s index (`None` for `NULL`).
    fn obj_controlling_player(&self, o: ObjectId) -> Option<PlayerIndex>;
    /// `a->getRelationship(b)`.
    fn obj_relationship(&self, a: ObjectId, b: ObjectId) -> Relationship;
    /// `obj->getAI() != NULL` (`getAIUpdateInterface()`).
    fn obj_has_ai(&self, o: ObjectId) -> bool;
    /// `obj->getAI()->isBusy()`.
    fn ai_is_busy(&self, o: ObjectId) -> bool;
    /// `ai->aiIdle(CMD_FROM_AI)`.
    fn ai_idle(&mut self, o: ObjectId);
    /// `ai->construct(what, pos, angle, owningPlayer, FALSE)`.
    fn ai_construct(
        &mut self,
        o: ObjectId,
        what: TemplateRef,
        pos: Coord3D,
        angle: f32,
        player: PlayerIndex,
    ) -> Option<ObjectId>;
    /// `ai->isQuickPathAvailable(&pos)`.
    fn ai_is_quick_path_available(&mut self, o: ObjectId, pos: Coord3D) -> bool;
    /// `ai->aiMoveToPositionEvenIfSleeping(&pos, CMD_FROM_AI)`.
    fn ai_move_to_position_even_if_sleeping(&mut self, o: ObjectId, pos: Coord3D);
    /// `obj->getProducerID()`.
    fn obj_producer_id(&self, o: ObjectId) -> ObjectId;
    /// `obj->getConstructionPercent()`.
    fn obj_construction_percent(&self, o: ObjectId) -> f32;
    /// `obj->setConstructionPercent(p)`.
    fn obj_set_construction_percent(&mut self, o: ObjectId, p: f32);
    /// `obj->clearAndSetModelConditionFlags(clear, set)`.
    fn obj_clear_and_set_model_condition(
        &mut self,
        o: ObjectId,
        clear: &[ModelConditionFlagType],
        set: &[ModelConditionFlagType],
    );
    /// `obj->setModelConditionFlags(set)`.
    fn obj_set_model_condition(&mut self, o: ObjectId, set: &[ModelConditionFlagType]);
    /// `obj->setStatus(mask)` of the given bits.
    fn obj_set_status(&mut self, o: ObjectId, bits: &[ObjectStatusTypes]);
    /// `obj->getDrawable() != NULL`.
    fn obj_has_drawable(&self, o: ObjectId) -> bool;
    /// `draw->setAnimationLoopDuration(frames)`.
    fn drawable_set_animation_loop_duration(&mut self, o: ObjectId, frames: u32);
    /// `obj->getContain() != NULL`.
    fn obj_has_contain(&self, o: ObjectId) -> bool;
    /// `contain->onSelling()`.
    fn contain_on_selling(&mut self, o: ObjectId);
    /// `obj->getProductionUpdateInterface() != NULL`.
    fn obj_has_production(&self, o: ObjectId) -> bool;
    /// `pui->getSpecialPowerConstructionCommandButton()->getThingTemplate()` (`None` without a button).
    fn production_special_power_construction_template(&self, o: ObjectId) -> Option<TemplateRef>;
    /// `pui->canQueueCreateUnit(what)`.
    fn production_can_queue_create_unit(&mut self, o: ObjectId, what: TemplateRef) -> CanMakeType;
    /// `pui->cancelAndRefundAllProduction()`.
    fn production_cancel_and_refund_all(&mut self, o: ObjectId);
    /// The index of the first behavior module of the object that has a `ParkingPlaceBehaviorInterface`.
    fn obj_first_parking_place(&self, o: ObjectId) -> Option<usize>;
    /// `pp->killAllParkedUnits()`.
    fn parking_kill_all_parked_units(&mut self, o: ObjectId, module: usize);
    /// How many behavior modules of the object have a `CreateModuleInterface`, in module order.
    fn obj_create_module_count(&self, o: ObjectId) -> usize;
    /// `create->onBuildComplete()` of the `n`th of them.
    fn create_module_on_build_complete(&mut self, o: ObjectId, n: usize);
    /// `obj->handlePartitionCellMaintenance()`.
    fn obj_handle_partition_cell_maintenance(&mut self, o: ObjectId);
    /// `obj->setProducer(producer)` (`None` for `NULL`).
    fn obj_set_producer(&mut self, o: ObjectId, producer: Option<ObjectId>);
    /// `obj->setPosition(&pos)`.
    fn obj_set_position(&mut self, o: ObjectId, pos: Coord3D);
    /// `obj->setOrientation(angle)`.
    fn obj_set_orientation(&mut self, o: ObjectId, angle: f32);
    /// `TheThingFactory->newObject(what, owner's default team, status)`; the bits are the starting status.
    fn thing_factory_new_object(
        &mut self,
        what: TemplateRef,
        owner: PlayerIndex,
        status: &[ObjectStatusTypes],
    ) -> ObjectId;
    /// `TheAI->pathfinder()->addObjectToPathfindMap(obj)`.
    fn pathfinder_add_object(&mut self, o: ObjectId);
    /// `obj->getCommandSetString()`'s command set, `TheControlBar->findCommandSet(...)`: `Some(set)` when it exists.
    fn find_command_set(&self, o: ObjectId) -> Option<u32>;
    /// `commandSet->getCommandButton(i)`: `Some(button)` when slot `i` holds one.
    fn command_set_button(&self, set: u32, i: i32) -> Option<u32>;
    /// `button->getCommandType()`.
    fn button_command_type(&self, button: u32) -> i32;
    /// `button->getThingTemplate()`.
    fn button_template(&self, button: u32) -> TemplateRef;
    // ---- thing templates ------------------------------------------------------------------------------------------
    /// `tmpl->getTemplateGeometryInfo()` (a copy).
    fn tmpl_geometry(&self, t: TemplateRef) -> GeometryInfo;
    /// `tmpl->isKindOf(k)`.
    fn tmpl_is_kind_of(&self, t: TemplateRef, k: KindOfType) -> bool;
    /// `tmpl->getFactoryExitWidth()`.
    fn tmpl_factory_exit_width(&self, t: TemplateRef) -> f32;
    /// `tmpl->getFactoryExtraBibWidth()`.
    fn tmpl_factory_extra_bib_width(&self, t: TemplateRef) -> f32;
    /// `tmpl->friend_calcVisionRange()`.
    fn tmpl_vision_range(&self, t: TemplateRef) -> f32;
    /// `tmpl->getRefundValue()`.
    fn tmpl_refund_value(&self, t: TemplateRef) -> i32;
    /// `tmpl->calcCostToBuild(player)`.
    fn tmpl_calc_cost_to_build(&self, t: TemplateRef, player: PlayerIndex) -> i32;
    /// `a->isEquivalentTo(b)`.
    fn tmpl_is_equivalent_to(&self, a: TemplateRef, b: TemplateRef) -> bool;
    /// `AudioEventRTS sound = *tmpl->getPerUnitSound("VoiceCreated"); sound.setObjectID(id); TheAudio->addAudioEvent(&sound)`.
    fn audio_add_voice_created(&mut self, t: TemplateRef, o: ObjectId);
    // ---- the player (forwarders to the verified `Player` functions) -----------------------------------------------
    /// `player->getPlayerType()`.
    fn player_type(&self, p: PlayerIndex) -> PlayerType;
    /// `player->isSkirmishAIPlayer()`.
    fn player_is_skirmish_ai(&self, p: PlayerIndex) -> bool;
    /// `player->getDefaultTeam() != NULL`.
    fn player_has_default_team(&self, p: PlayerIndex) -> bool;
    /// `player->getRelationship(them->getTeam())`.
    fn player_relationship_to_object(&self, p: PlayerIndex, them: ObjectId) -> Relationship;
    /// `player->canBuild(tmpl)`.
    fn player_can_build(&mut self, p: PlayerIndex, t: TemplateRef) -> bool;
    /// `player->canBuildMoreOfType(tmpl)`.
    fn player_can_build_more_of_type(&mut self, p: PlayerIndex, t: TemplateRef) -> bool;
    /// `player->getMoney()->countMoney()`.
    fn player_count_money(&self, p: PlayerIndex) -> u32;
    /// `player->getMoney()->deposit(amount)`.
    fn player_deposit(&mut self, p: PlayerIndex, amount: u32);
    /// `player->onStructureCreated(constructor, obj)`.
    fn player_on_structure_created(
        &mut self,
        p: PlayerIndex,
        constructor: Option<ObjectId>,
        o: ObjectId,
    );
    /// `player->onStructureConstructionComplete(constructor, obj, FALSE)`.
    fn player_on_structure_construction_complete(
        &mut self,
        p: PlayerIndex,
        constructor: Option<ObjectId>,
        o: ObjectId,
    );
    /// `player->onUnitCreated(constructor, obj)`.
    fn player_on_unit_created(
        &mut self,
        p: PlayerIndex,
        constructor: Option<ObjectId>,
        o: ObjectId,
    );
    // ---- terrain and the terrain visual ---------------------------------------------------------------------------
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn terrain_ground_height(&self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->getExtent(&region)`.
    fn terrain_extent(&self) -> Region3D;
    /// `TheTerrainLogic->getMaximumPathfindExtent(&region)`.
    fn terrain_maximum_pathfind_extent(&self) -> Region3D;
    /// `TheTerrainLogic->getLayerForDestination(&pos)`.
    fn terrain_layer_for_destination(&self, pos: Coord3D) -> PathfindLayer;
    /// `TheTerrainVisual->getTerrainTile(x, y)->getRestrictConstruction()` (`None` when there is no tile).
    fn terrain_tile_restricts_construction(&self, x: f32, y: f32) -> Option<bool>;
    /// `TheTerrainVisual->addFactionBib(obj, highlight, extra)`.
    fn terrain_visual_add_faction_bib(&mut self, o: ObjectId, highlight: bool, extra: f32);
    /// `TheTerrainVisual->removeTreesAndPropsForConstruction(pos, geom, angle)`.
    fn terrain_visual_remove_trees_and_props(
        &mut self,
        pos: Coord3D,
        geom: &GeometryInfo,
        angle: f32,
    );
    // ---- the partition manager and the pathfinder -----------------------------------------------------------------
    /// `ThePartitionManager->iteratePotentialCollisions(&pos, geom, angle)`, in iteration order.
    fn partition_iterate_potential_collisions(
        &mut self,
        pos: Coord3D,
        geom: &GeometryInfo,
        angle: f32,
    ) -> Vec<ObjectId>;
    /// `ThePartitionManager->iterateObjectsInRange(&pos, range, FROM_BOUNDINGSPHERE_2D, {AcceptByKindOf(STRUCTURE)})`, in
    /// iteration order.
    fn partition_structures_in_range(&mut self, pos: Coord3D, range: f32) -> Vec<ObjectId>;
    /// `ThePartitionManager->getClosestObject(&pos, range, FROM_BOUNDINGSPHERE_2D, {AcceptByKindOf(SUPPLY_SOURCE)})`.
    fn partition_closest_supply_source(&mut self, pos: Coord3D, range: f32) -> Option<ObjectId>;
    /// `ThePartitionManager->geomCollidesWithGeom(...)`.
    fn partition_geom_collides_with_geom(
        &self,
        pos1: Coord3D,
        geom1: &GeometryInfo,
        angle1: f32,
        pos2: Coord3D,
        geom2: &GeometryInfo,
        angle2: f32,
    ) -> bool;
    /// `ThePartitionManager->worldToCell(x, y, &cx, &cy)`.
    fn partition_world_to_cell(&self, x: f32, y: f32) -> (i32, i32);
    /// `ThePartitionManager->getShroudStatusForPlayer(playerIndex, x, y)`.
    fn partition_shroud_status_for_player(
        &self,
        player_index: i32,
        x: i32,
        y: i32,
    ) -> CellShroudStatus;
    /// `TheAI->pathfinder()->getCell(layer, x, y)->getType()` (`None` for `NULL`).
    fn pathfinder_cell_type(
        &self,
        layer: PathfindLayer,
        x: i32,
        y: i32,
    ) -> Option<PathfindCellType>;
}

// ---------------------------------------------------------------------------------------------------------------------
// the plain pieces
// ---------------------------------------------------------------------------------------------------------------------

/// `(Real)cos(x)` and `(Real)sin(x)` as MSVC compiles them: in `double`, narrowed.
fn cos_d(x: f32) -> f32 {
    <f64 as petir::Real>::cos(f64::from(x)) as f32
}
fn sin_d(x: f32) -> f32 {
    <f64 as petir::Real>::sin(f64::from(x)) as f32
}

/// `isDozer(obj)`.
fn is_dozer<H: BuildAssistantHost>(h: &H, obj: Option<ObjectId>) -> bool {
    match obj {
        None => false,
        Some(o) => h.obj_is_kind_of(o, KINDOF_DOZER),
    }
}

/// `BitTest( options, flag )`.
fn bit_test(options: u32, flag: u32) -> bool {
    options & flag != 0
}

/// `BuildAssistant::init()`.
pub fn build_assistant_init<H: BuildAssistantHost>(h: &H, ba: &mut BuildAssistant) {
    // allocate our array of positions that we use to assist ourselves when constructing a tiled array of locations to
    // build things like walls
    ba.build_position_size = h.build_globals().max_line_build_objects + plant(1) as i32;
    ba.build_positions = vec![Coord3D::default(); ba.build_position_size.max(0) as usize];
}

/// `BuildAssistant::reset()`: clear all our data from the sell list.
pub fn build_assistant_reset(ba: &mut BuildAssistant) {
    ba.sell_list.clear();
}

/// `BuildAssistant::update()`: remove any objects from the sell list whose sell time is up.
pub fn build_assistant_update<H: BuildAssistantHost>(h: &mut H, ba: &mut BuildAssistant) {
    let mut i = 0;
    while i < ba.sell_list.len() {
        let sell_info = ba.sell_list[i];
        // find the object
        let Some(obj) = h.find_object_by_id(sell_info.id) else {
            // if object is not found, remove it from the list immediately ... the object maybe was destroyed by other
            // means during the sell process
            if !plant(14) {
                ba.sell_list.remove(i);
            } else {
                i += 1;
            }
            continue;
        };
        // decrement the construction percent
        if h.frame().wrapping_sub(sell_info.sell_frame) as f32
            >= FRAMES_TO_ALLOW_SCAFFOLD + plant(2) as u32 as f32
        {
            let previous_construction_percent = h.obj_construction_percent(obj);
            // do the construction
            h.obj_set_construction_percent(
                obj,
                previous_construction_percent
                    - (100.0
                        / if plant(3) {
                            45.0
                        } else {
                            TOTAL_FRAMES_TO_SELL_OBJECT
                        }),
            );
            // did we cross the threshold from positive to negative ... if so, we will clear visual construction indicators
            if previous_construction_percent >= if plant(4) { 0.0 } else { f32::MIN_POSITIVE }
                && h.obj_construction_percent(obj) <= 0.0
            {
                h.obj_clear_and_set_model_condition(
                    obj,
                    &[
                        MODELCONDITION_PARTIALLY_CONSTRUCTED,
                        MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED,
                    ],
                    if plant(5) {
                        &[]
                    } else {
                        &[MODELCONDITION_SOLD]
                    },
                );
                // set the animation durations so that the regular build up loop animations can be done a bit faster for selling
                if h.obj_has_drawable(obj) && !plant(6) {
                    h.drawable_set_animation_loop_duration(
                        obj,
                        (TOTAL_FRAMES_TO_SELL_OBJECT / if plant(7) { 3.0 } else { 2.0 }) as u32,
                    );
                }
            }
        }
        // after we've reached zero ... the object has "sunk" back down into the ground ... but we need to keep the object
        // around for a little while longer so that the scaffold animation can finish up and sink itself back into the ground
        if h.obj_construction_percent(obj) <= if plant(8) { -49.0 } else { -50.0 } {
            // refund the money to the controlling player
            if let Some(player) = h.obj_controlling_player(obj) {
                let t = h.obj_template(obj);
                // 0 is the init, and means you have no override. We would be marked unsellable if someone wanted us to not sell
                let sell_value: u32 = if h.tmpl_refund_value(t) > if plant(9) { 100 } else { 0 } {
                    h.tmpl_refund_value(t) as u32
                } else {
                    real_to_unsigned_int(
                        h.tmpl_calc_cost_to_build(t, player) as f32
                            * if plant(10) {
                                1.0
                            } else {
                                h.build_globals().sell_percentage
                            },
                    )
                };
                if !plant(11) {
                    h.player_deposit(player, sell_value);
                }
            }
            // cancel any of the production items and refund to the controlling player
            if h.obj_has_production(obj) && !plant(12) {
                h.production_cancel_and_refund_all(obj);
            }
            // destroy the object
            if !plant(13) {
                h.destroy_object(obj);
            }
            // remove this object from the sell list
            ba.sell_list.remove(i);
            continue;
        }
        i += 1;
    }
}

/// `BuildAssistant::xferTheSellList(xfer)`.
pub fn build_assistant_xfer_the_sell_list(
    ba: &mut BuildAssistant,
    xfer: &mut dyn Xfer,
) -> BResult<()> {
    let mut count = ba.sell_list.len() as i32;
    xfer.xfer_int(&mut count)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        if !plant(15) {
            ba.sell_list.clear();
        }
        for _ in 0..count {
            // add this object to the list of objects being sold
            let mut info = ObjectSellInfo {
                id: INVALID_ID,
                sell_frame: 0,
            };
            xfer.xfer_object_id(&mut info.id)?;
            if !plant(16) {
                xfer.xfer_unsigned_int(&mut info.sell_frame)?;
            }
            if plant(17) {
                ba.sell_list.insert(0, info);
            } else {
                ba.sell_list.push(info);
            }
        }
    } else {
        for info in ba.sell_list.iter_mut() {
            xfer.xfer_object_id(&mut info.id)?;
            xfer.xfer_unsigned_int(&mut info.sell_frame)?;
        }
        // (upstream counts `count--` down to a DEBUG_ASSERTCRASH( count==0 ), which a release build does not compile)
    }
    Ok(())
}

/// `BuildAssistant::isLineBuildTemplate(tTemplate)`.
pub fn build_assistant_is_line_build_template<H: BuildAssistantHost>(
    h: &H,
    t: TemplateRef,
) -> bool {
    h.tmpl_is_kind_of(t, KINDOF_LINEBUILD) != plant(18)
}

/// `BuildAssistant::isRemovableForConstruction(obj)`: some objects are removed automatically when something is built on them.
pub fn build_assistant_is_removable_for_construction<H: BuildAssistantHost>(
    h: &H,
    obj: ObjectId,
) -> bool {
    if h.obj_is_kind_of(obj, KINDOF_INERT) && !plant(19) {
        // DEBUG_CRASH(("should not have gotten here.")); not compiled in a release build
        return false;
    }
    // all shrubbery can be removed
    if h.obj_is_kind_of(obj, KINDOF_SHRUBBERY) && !plant(20) {
        return true;
    }
    // anything with a kindof marked as cleared by build can be removed
    if h.obj_is_kind_of(obj, KINDOF_CLEARED_BY_BUILD) && !plant(21) {
        return true;
    }
    // Rubble, scrap & dead units can be cleared.
    h.obj_is_effectively_dead(obj) && !plant(22)
}

/// `BuildAssistant::clearRemovableForConstruction(whatToBuild, pos, angle)`.
fn clear_removable_for_construction<H: BuildAssistantHost>(
    h: &mut H,
    what: TemplateRef,
    pos: Coord3D,
    angle: f32,
) {
    let geom = h.tmpl_geometry(what);
    for them in h.partition_iterate_potential_collisions(pos, &geom, angle) {
        // UI feedback objects (always selectable) never get destroyed by construction, but also should never trip any of
        // the other side effects of placing them in isRemovableForConstruction()
        if build_assistant_is_removable_for_construction(h, them)
            && (!h.obj_is_kind_of(them, KINDOF_ALWAYS_SELECTABLE) || plant(23))
        {
            h.destroy_object(them);
        }
    }
    if !plant(24) {
        h.terrain_visual_remove_trees_and_props(pos, &geom, angle);
    }
}

/// `BuildAssistant::moveObjectsForConstruction(whatToBuild, pos, angle, playerToBuild)`: will move objects that can move out
/// of the way; false if there are objects that cannot be moved out of the way.
fn move_objects_for_construction<H: BuildAssistantHost>(
    h: &mut H,
    what: TemplateRef,
    pos: Coord3D,
    angle: f32,
    player_to_build: PlayerIndex,
) -> bool {
    let template_geom = h.tmpl_geometry(what);
    let mut gi = GeometryInfo::new(
        GeometryType::Box,
        false,
        10.0,
        template_geom.get_major_radius(),
        template_geom.get_major_radius(),
    );
    if template_geom.get_geom_type() == GeometryType::Box && !plant(25) {
        gi = template_geom;
    }
    let mut any_unmovables = false;
    // sqrt( pow(a, 2) + pow(b, 2) ): pow( float, int ) and sqrt are done in double, then narrowed
    let major = f64::from(gi.get_major_radius());
    let minor = f64::from(if plant(27) {
        gi.get_major_radius()
    } else {
        gi.get_minor_radius()
    });
    let mut radius: f32 = <f64 as petir::Real>::sqrt(
        <f64 as petir::Real>::powf(major, 2.0) + <f64 as petir::Real>::powf(minor, 2.0),
    ) as f32;
    radius *= if plant(26) { 1.5 } else { 1.4 }; // Fudge the distance,

    for them in h.partition_iterate_potential_collisions(pos, &gi, angle) {
        // ignore land mines, cluster mines and demo traps, since you can build on them; doing so will damage them during
        // construction, by the way
        if h.obj_is_kind_of(them, KINDOF_MINE) && !plant(28) {
            continue;
        }
        // same story for inert things: ie, radiation fields, pings, and projectile streams (!)...
        if h.obj_is_kind_of(them, KINDOF_INERT) && !plant(29) {
            continue;
        }
        // Skip KINDOF_ALWAYS_SELECTABLE and isRemovableForConstruction, because if it is RemovableForConstruction, it just
        // got destroyed, and won't actually be gone until the end of the frame. jba.
        if (!h.obj_is_kind_of(them, KINDOF_ALWAYS_SELECTABLE) || plant(30))
            && !build_assistant_is_removable_for_construction(h, them)
        {
            let rel = h.player_relationship_to_object(player_to_build, them);
            if rel == Relationship::Allies || (rel == Relationship::Neutral && !plant(31)) {
                // Pick a point outside of the construction and tell those fools to move there. Pick an arbitrary direction
                // and tell the unit to move there, at a distance greater than the object's radius.
                if h.obj_has_ai(them) {
                    // Vary the distance to move between one half the diameter of the building (roughly) and 1.5 times the
                    // diameter of the building
                    let varied_radius =
                        h.random_value_real(if plant(32) { 0.6 } else { 0.5 }, 1.5) * radius;
                    let dir = h.random_value_real(
                        -crate::common::base_type::PI,
                        if plant(33) {
                            0.0
                        } else {
                            crate::common::base_type::PI
                        },
                    );
                    let mut vec = Vector3::new(varied_radius, 0.0, 0.0);
                    vec.rotate_z(dir);
                    let dest_pos = Coord3D {
                        x: pos.x + vec.x,
                        y: pos.y + vec.y,
                        z: if plant(34) { 0.0 } else { pos.z + vec.z },
                    };
                    // note that this is an extra-special case... even if the unit's mood is "sleep" it still needs to move
                    // here. (units with an ai mood of "sleep" won't respond to any AI commands. this is a special case that
                    // ignores the "sleep" ai mood.) (srj)
                    h.ai_move_to_position_even_if_sleeping(them, dest_pos);
                } else if !plant(35) {
                    any_unmovables = true;
                }
            } else if !plant(36) {
                any_unmovables = true;
            }
        }
    }
    !any_unmovables
}

// ---------------------------------------------------------------------------------------------------------------------
// can it be made
// ---------------------------------------------------------------------------------------------------------------------

/// `BuildAssistant::isPossibleToMakeUnit(builder, whatToBuild)`: are all the requirements for making this unit (except
/// available cash) satisfied. `builder` and `whatToBuild` are never null here.
pub fn build_assistant_is_possible_to_make_unit<H: BuildAssistantHost>(
    h: &mut H,
    builder: ObjectId,
    what: TemplateRef,
) -> BResult<bool> {
    // get the command set for the producer object; if no command set we cannot build anything
    let Some(command_set) = h.find_command_set(builder) else {
        return Ok(false);
    };
    // scan the command set, we must find whatToBuild as one of the "build" commands available in the command set
    let mut found_command: Option<u32> = None;
    for i in 0..MAX_COMMANDS_PER_SET - plant(41) as i32 {
        if let Some(button) = h.command_set_button(command_set, i) {
            let ct = h.button_command_type(button);
            if (ct == GUI_COMMAND_UNIT_BUILD || (ct == GUI_COMMAND_DOZER_CONSTRUCT && !plant(38)))
                && (h.tmpl_is_equivalent_to(h.button_template(button), what) || plant(39))
            {
                found_command = if plant(37) && found_command.is_some() {
                    found_command
                } else {
                    Some(button)
                };
            }
        }
    }
    let Some(found) = found_command else {
        return Ok(false);
    };
    // make sure that the player can actually make this unit by checking prereqs and such
    let player = h
        .obj_controlling_player(builder)
        .ok_or(BuildError::Undefined(
            "isPossibleToMakeUnit: the builder has no controlling player",
        ))?;
    let t = h.button_template(found);
    if !h.player_can_build(player, t) && !plant(40) {
        return Ok(false);
    }
    Ok(true)
}

/// `BuildAssistant::canMakeUnit(builder, whatToBuild)`: are all the requirements for making this unit, and the cash.
pub fn build_assistant_can_make_unit<H: BuildAssistantHost>(
    h: &mut H,
    builder: ObjectId,
    what: TemplateRef,
) -> BResult<CanMakeType> {
    if (h.obj_test_script_status_bit(builder, OBJECT_STATUS_SCRIPT_DISABLED) && !plant(43))
        || (h.obj_test_script_status_bit(builder, OBJECT_STATUS_SCRIPT_UNPOWERED) && !plant(42))
    {
        return Ok(CanMakeType::FactoryIsDisabled);
    }
    let has_pu = h.obj_has_production(builder);
    // If our builder is actually constructing an object via a special power, then allow it if the templates match. It's
    // possible they won't match because a GLA command center could be in "place sneak attack" mode, and queue up a worker
    // in the meantime.
    if has_pu
        && (h.production_special_power_construction_template(builder) == Some(what)
            || (plant(44)
                && h.production_special_power_construction_template(builder)
                    .is_some()))
    {
        return Ok(CanMakeType::Ok);
    }
    let player = h.obj_controlling_player(builder);
    // make sure we're not maxed out for this type of unit. Warning: isPossibleToMakeUnit() now implicitly calls
    // canBuildMoreOfType(), so do this check first
    if let Some(p) = player {
        if !h.player_can_build_more_of_type(p, what) && !plant(45) {
            return Ok(CanMakeType::MaxedOutForPlayer);
        }
    }
    if !build_assistant_is_possible_to_make_unit(h, builder, what)? {
        return Ok(CanMakeType::NoPrereq);
    }
    if has_pu {
        let cmt = h.production_can_queue_create_unit(builder, what);
        if cmt != CanMakeType::Ok && !plant(46) {
            return Ok(cmt);
        }
    }
    // make sure we have enough money to build this
    let player = player.ok_or(BuildError::Undefined(
        "canMakeUnit: the builder has no controlling player",
    ))?;
    // calcCostToBuild() is an Int, countMoney() an UnsignedInt: the comparison is unsigned
    if (h.tmpl_calc_cost_to_build(what, player) as u32) >= h.player_count_money(player) {
        return Ok(CanMakeType::NoMoney);
    }
    Ok(CanMakeType::Ok)
}

// ---------------------------------------------------------------------------------------------------------------------
// the footprint
// ---------------------------------------------------------------------------------------------------------------------

/// `BuildAssistant::iterateFootprint(build, buildOrientation, worldPos, sampleResolution, func, funcUserData)`: call the
/// callback at each "sample point" across the footprint of where this object would be in the world. The user data of
/// upstream's `void*` is whatever the closure captures.
pub fn build_assistant_iterate_footprint<H: BuildAssistantHost, F: FnMut(&mut H, &Coord3D)>(
    h: &mut H,
    build: TemplateRef,
    build_orientation: f32,
    world_pos: Coord3D,
    sample_resolution: f32,
    mut func: F,
) {
    // create a transform matrix for the object position at the specified angle, we will iterate the object footprints in
    // "local object extent space" and transform those points using this matrix into the world coords for the object at
    // the real location and specified angle
    let mut transform = Matrix3D::default();
    transform.make_identity();
    transform.adjust_translation(&Vector3::new(world_pos.x, world_pos.y, world_pos.z));
    if !plant(50) {
        transform.rotate_z(build_orientation);
    }

    // get the bounding footprint rectangle for the geometry we're looking at
    let geom = h.tmpl_geometry(build);
    let (half_footprint_height, half_footprint_width);
    match geom.get_geom_type() {
        GeometryType::Box => {
            half_footprint_height = if plant(48) {
                geom.get_major_radius()
            } else {
                geom.get_minor_radius()
            };
            half_footprint_width = geom.get_major_radius();
        }
        GeometryType::Sphere | GeometryType::Cylinder => {
            half_footprint_height = if plant(49) {
                geom.get_major_radius()
            } else {
                geom.get_bounding_circle_radius()
            };
            half_footprint_width = geom.get_bounding_circle_radius();
        }
    }

    // start at a corner of the extent ... box geometries have a major radius down the X axis with the minor radius on the Y
    // axis. Note that we are allowing the sample process to go an "extra" sample resolution outside of the real extent here.
    // This is so that we can be sure to sample all of the footprint area, if our sample point is outside of the actual
    // extent footprint area because of this we snap it back to sample on the exact edge
    let mut y = -half_footprint_height;
    while y < half_footprint_height + sample_resolution - if plant(51) { 3.0 } else { 0.0 } {
        // snap it to the actual extent since we can go over by one sample resolution
        if y > half_footprint_height && !plant(52) {
            y = half_footprint_height;
        }
        let mut x = -half_footprint_width;
        while x < half_footprint_width + sample_resolution - if plant(53) { 3.0 } else { 0.0 } {
            // snap it to the actual extent since we can go over by one sample resolution
            if x > half_footprint_width && !plant(54) {
                x = half_footprint_width;
            }
            // transform to world
            let v = Vector3::new(
                x,
                y,
                if plant(55) {
                    0.0
                } else {
                    h.terrain_ground_height(x, y)
                },
            );
            let v = Matrix3D::transform_vector(&transform, &v);
            // for circular geometries we must actually be within the circle
            if matches!(
                geom.get_geom_type(),
                GeometryType::Sphere | GeometryType::Cylinder
            ) {
                let vector = Coord2D {
                    x: v.x - world_pos.x,
                    y: v.y - world_pos.y,
                };
                if vector.length() > half_footprint_width + if plant(56) { 1.0 } else { 0.0 } {
                    // could be height too, radius is all the same for circles
                    x += sample_resolution;
                    continue; // ignore this point
                }
            }
            // call the user callback
            let mut pos = Coord3D {
                x: v.x,
                y: v.y,
                z: 0.0,
            };
            pos.z = if plant(57) {
                v.z
            } else {
                h.terrain_ground_height(pos.x, pos.y)
            };
            func(h, &pos);
            x += sample_resolution;
        }
        y += sample_resolution;
    }
}

/// `SampleBuildData`.
struct SampleBuildData {
    map_region: Region3D,
    terrain_restricted: bool,
    hi_z: f32,
    lo_z: f32,
}

/// `checkSampleBuildLocation(samplePoint, userData)`: check the build conditions at the specified sample location point.
fn check_sample_build_location<H: BuildAssistantHost>(
    h: &H,
    sample_point: &Coord3D,
    sample_data: &mut SampleBuildData,
) {
    // get the terrain tile here; check for the restricts building flag
    if h.terrain_tile_restricts_construction(sample_point.x, sample_point.y) == Some(true)
        && !plant(58)
    {
        sample_data.terrain_restricted = true;
    }
    let cell_x = real_to_int_floor(sample_point.x / PATHFIND_CELL_SIZE as f32);
    let cell_y = real_to_int_floor(sample_point.y / PATHFIND_CELL_SIZE as f32);
    match h.pathfinder_cell_type(LAYER_GROUND, cell_x, cell_y) {
        None => sample_data.terrain_restricted = !plant(59),
        Some(ty) => {
            if (ty == PathfindCellType::Water && !plant(60))
                || ty == PathfindCellType::Cliff
                || (ty == PathfindCellType::Impassable && !plant(61))
            {
                sample_data.terrain_restricted = true;
            }
        }
    }
    // record the highest and lowest Z points from all the samples and do not allow building when the difference between
    // them is too great
    if sample_point.z < sample_data.lo_z && !plant(62) {
        sample_data.lo_z = sample_point.z;
    }
    if sample_point.z >= sample_data.hi_z + if plant(63) { 1.0 } else { 0.0 } {
        sample_data.hi_z = sample_point.z;
    }
    // too close to edge of map?
    let edge = h.build_globals().min_dist_from_edge_of_map_for_build;
    if edge > 0.0
        && (sample_point.x < sample_data.map_region.lo.x + if plant(64) { 0.0 } else { edge }
            || sample_point.x > sample_data.map_region.hi.x - if plant(65) { 0.0 } else { edge }
            || sample_point.y
                <= sample_data.map_region.lo.y + edge - if plant(66) { 1.0 } else { 0.0 }
            || sample_point.y
                > sample_data.map_region.hi.y - edge - if plant(67) { 1.0 } else { 0.0 })
    {
        sample_data.terrain_restricted = true;
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// legal to build
// ---------------------------------------------------------------------------------------------------------------------

/// `BuildAssistant::isLocationClearOfObjects(worldPos, build, angle, builderObject, options, thePlayer)`: check for objects
/// preventing building at this location.
pub fn build_assistant_is_location_clear_of_objects<H: BuildAssistantHost>(
    h: &mut H,
    world_pos: Coord3D,
    build: TemplateRef,
    angle: f32,
    builder_object: Option<ObjectId>,
    options: u32,
    the_player: Option<PlayerIndex>,
) -> LegalBuildCode {
    let build_geom = h.tmpl_geometry(build);
    let only_check_enemies = if plant(68) {
        bit_test(options, NO_ENEMY_OBJECT_OVERLAP)
    } else {
        options == NO_ENEMY_OBJECT_OVERLAP
    };
    for them in h.partition_iterate_potential_collisions(world_pos, &build_geom, angle) {
        let mut feedback_with_failure = true;
        let rel = match builder_object {
            Some(b) => h.obj_relationship(b, them),
            None => Relationship::Neutral,
        };
        // Kris: If the object is stealthed and we can't see it, pretend we can build there.
        if bit_test(options, IGNORE_STEALTHED)
            && (rel != Relationship::Allies || plant(69))
            && h.obj_test_status(them, OBJECT_STATUS_STEALTHED)
            && (!h.obj_test_status(them, OBJECT_STATUS_DETECTED) || plant(71))
            && !h.obj_test_status(them, OBJECT_STATUS_DISGUISED)
        {
            if bit_test(options, FAIL_STEALTHED_WITHOUT_FEEDBACK) && !plant(70) {
                feedback_with_failure = false; // We want to fail now but without feedback
            } else {
                continue;
            }
        }
        // Kris: Patch 1.01 - November 5, 2003: prevent busy units (black lotus hacking from being moved by trying to place a
        // building -- exploit).
        if rel == Relationship::Allies
            && (h.obj_test_status(them, OBJECT_STATUS_IS_USING_ABILITY) && !plant(73)
                || (h.obj_has_ai(them) && h.ai_is_busy(them) && !plant(72)))
        {
            return LegalBuildCode::ObjectsInTheWay;
        }
        // ignore any kind of class of objects that we will "remove" for building
        if build_assistant_is_removable_for_construction(h, them) && !plant(74) {
            continue;
        }
        // ignore land mines, cluster mines and demo traps, since you can build on them; doing so will damage them during
        // construction, by the way
        if h.obj_is_kind_of(them, KINDOF_MINE) && !plant(75) {
            continue;
        }
        // same story for inert things: ie, radiation fields, pings, and projectile streams (!)...
        if h.obj_is_kind_of(them, KINDOF_INERT) && !plant(76) {
            continue;
        }
        // an immobile object may obstruct our building depending on flags.
        if h.obj_is_kind_of(them, KINDOF_IMMOBILE) {
            if only_check_enemies
                && builder_object.is_some()
                && (rel != Relationship::Enemies || plant(78))
            {
                continue;
            }
            if feedback_with_failure || plant(81) {
                h.terrain_visual_add_faction_bib(them, true, 0.0);
                return LegalBuildCode::ObjectsInTheWay;
            }
            return LegalBuildCode::GenericFailure;
        }
        if h.obj_is_disabled(them) && !plant(79) {
            // Kris: If object is disabled, it can't move out of the way, thus you can't build there.
            if feedback_with_failure {
                h.terrain_visual_add_faction_bib(them, true, 0.0);
                return LegalBuildCode::ObjectsInTheWay;
            }
            return LegalBuildCode::GenericFailure;
        }
        // if this is an enemy object of the builder object (and therefore the thing that will be constructed) we can't build here
        if builder_object.is_some()
            && (rel == Relationship::Enemies || (plant(80) && rel == Relationship::Neutral))
        {
            if feedback_with_failure {
                h.terrain_visual_add_faction_bib(them, true, 0.0);
                return LegalBuildCode::ObjectsInTheWay;
            }
            return LegalBuildCode::GenericFailure;
        }
    }

    if only_check_enemies && !plant(82) {
        return LegalBuildCode::Ok;
    }
    // Check for overlapping exit areas.
    let range = 2.0
        * (build_geom.get_major_radius()
            + if plant(83) {
                build_geom.get_major_radius()
            } else {
                build_geom.get_minor_radius()
            });
    let structures = h.partition_structures_in_range(world_pos, range);

    let mut my_factory_exit_width = h.tmpl_factory_exit_width(build);
    let mut my_extra_width = h.tmpl_factory_extra_bib_width(build);

    if the_player.is_some_and(|p| h.player_is_skirmish_ai(p)) && !plant(84) {
        // Skirmish ai adds a little extra around the edges so it doesn't build itself into a corner.
        if my_extra_width < 3.0 * PATHFIND_CELL_SIZE_F {
            my_extra_width = 3.0 * PATHFIND_CELL_SIZE_F;
            my_factory_exit_width -= if plant(85) { 0.0 } else { my_extra_width };
            if my_factory_exit_width < 0.0 && !plant(86) {
                my_factory_exit_width = 0.0;
            }
        }
    }

    let mut check_my_exit = false;
    let mut my_exit_pos = Coord3D::default();
    let mut my_bounds = build_geom.clone();
    my_bounds.set_major_radius(
        my_bounds.get_major_radius() + if plant(87) { 0.0 } else { my_extra_width },
    );
    if my_bounds.get_geom_type() != GeometryType::Box {
        let m = my_bounds.get_major_radius();
        my_bounds.set(GeometryType::Box, false, 40.0, m, m);
    } else {
        my_bounds.set_minor_radius(
            my_bounds.get_minor_radius() + if plant(88) { 0.0 } else { my_extra_width },
        );
    }
    let mut my_geom = build_geom.clone();
    if my_geom.get_geom_type() != GeometryType::Box {
        let m = my_geom.get_major_radius();
        my_geom.set_minor_radius(m);
    }
    my_geom.set_major_radius(my_factory_exit_width / if plant(89) { 3.0 } else { 2.0 });
    if my_factory_exit_width > 0.0 {
        my_exit_pos = world_pos;
        check_my_exit = true;
        let c = cos_d(angle);
        let s = sin_d(angle);
        let offset = if plant(90) {
            build_geom.get_minor_radius()
        } else {
            build_geom.get_major_radius()
        } + my_factory_exit_width / 2.0;
        my_exit_pos.x += c * offset;
        my_exit_pos.y += if plant(91) { c } else { s } * offset;
    }

    for them in structures {
        let rel = match builder_object {
            Some(b) => h.obj_relationship(b, them),
            None => Relationship::Neutral,
        };
        // Kris: If the building is stealthed and we can't see it, pretend we can build there.
        if bit_test(options, IGNORE_STEALTHED)
            && (rel != Relationship::Allies || plant(92))
            && h.obj_test_status(them, OBJECT_STATUS_STEALTHED)
            && !h.obj_test_status(them, OBJECT_STATUS_DETECTED)
            && !h.obj_test_status(them, OBJECT_STATUS_DISGUISED)
        {
            continue;
        }
        // ignore any kind of class of objects that we will "remove" for building
        if build_assistant_is_removable_for_construction(h, them) && !plant(93) {
            continue;
        }
        let them_template = h.obj_template(them);
        let them_factory_exit_width = h.tmpl_factory_exit_width(them_template);
        let his_extra_width = h.tmpl_factory_extra_bib_width(them_template);

        let mut check_his_exit = false;
        let mut his_exit_pos = Coord3D::default();
        let them_geom = h.obj_geometry(them);
        let mut his_bounds = them_geom.clone();
        his_bounds.set_major_radius(
            his_bounds.get_major_radius()
                + if plant(95) {
                    my_extra_width
                } else {
                    his_extra_width
                },
        );
        if his_bounds.get_geom_type() != GeometryType::Box {
            let m = his_bounds.get_major_radius();
            his_bounds.set(GeometryType::Box, false, 40.0, m, m);
        } else {
            // (upstream adds MY extra width to HIS minor radius)
            his_bounds.set_minor_radius(
                his_bounds.get_minor_radius()
                    + if plant(94) {
                        his_extra_width
                    } else {
                        my_extra_width
                    },
            );
        }
        let mut his_geom = them_geom.clone();
        his_geom.set_major_radius(them_factory_exit_width / if plant(97) { 3.0 } else { 2.0 });
        if his_geom.get_geom_type() != GeometryType::Box {
            his_geom.set_minor_radius(if plant(96) {
                them_geom.get_minor_radius()
            } else {
                them_geom.get_major_radius()
            });
        }
        let them_pos = h.obj_position(them);
        let them_orientation = h.obj_orientation(them);
        if them_factory_exit_width > 0.0 {
            his_exit_pos = them_pos;
            check_his_exit = true;
            let c = cos_d(if plant(98) { angle } else { them_orientation });
            let s = sin_d(them_orientation);
            let offset = them_geom.get_major_radius()
                + them_factory_exit_width / if plant(99) { 3.0 } else { 2.0 };
            his_exit_pos.x += c * offset;
            his_exit_pos.y += s * offset;
        }
        if h.partition_geom_collides_with_geom(
            them_pos,
            &his_bounds,
            them_orientation,
            world_pos,
            if plant(100) { &my_geom } else { &my_bounds },
            angle,
        ) {
            h.terrain_visual_add_faction_bib(them, true, 0.0);
            return LegalBuildCode::ObjectsInTheWay;
        }
        if !check_my_exit
            && !check_his_exit
            && his_extra_width == 0.0
            && (my_extra_width == 0.0 || plant(101))
        {
            continue; // neither has extra exit space.
        }

        // an immobile object will obstruct our building no matter what team it's on
        if h.obj_is_kind_of(them, KINDOF_IMMOBILE) || plant(102) {
            // Check for overlap of my exit rectangle to his geom info.
            if check_my_exit
                && h.partition_geom_collides_with_geom(
                    them_pos,
                    &his_bounds,
                    them_orientation,
                    my_exit_pos,
                    if plant(103) { &my_bounds } else { &my_geom },
                    angle,
                )
            {
                h.terrain_visual_add_faction_bib(them, true, 0.0);
                return LegalBuildCode::ObjectsInTheWay;
            }
            // Check for overlap of his exit rectangle with my geom info
            if check_his_exit
                && h.partition_geom_collides_with_geom(
                    his_exit_pos,
                    if plant(104) { &his_bounds } else { &his_geom },
                    them_orientation,
                    world_pos,
                    &my_bounds,
                    angle,
                )
            {
                h.terrain_visual_add_faction_bib(them, true, 0.0);
                return LegalBuildCode::ObjectsInTheWay;
            }
            // Check both exit rectangles together.
            if (check_my_exit || (plant(105) && check_his_exit))
                && check_his_exit
                && h.partition_geom_collides_with_geom(
                    his_exit_pos,
                    &his_geom,
                    them_orientation,
                    my_exit_pos,
                    &my_geom,
                    angle,
                )
            {
                h.terrain_visual_add_faction_bib(them, true, 0.0);
                return LegalBuildCode::ObjectsInTheWay;
            }
        }
    }
    LegalBuildCode::Ok
}

/// `BuildAssistant::isLocationLegalToBuild(worldPos, build, angle, options, builderObject, player)`: query if we can build at
/// this location. `builderObject` is used for queries that require a pathfind check and is `None` if not required.
pub fn build_assistant_is_location_legal_to_build<H: BuildAssistantHost>(
    h: &mut H,
    world_pos: Coord3D,
    build: TemplateRef,
    angle: f32,
    options: u32,
    builder_object: Option<ObjectId>,
    player: Option<PlayerIndex>,
) -> LegalBuildCode {
    // You just can't never build off the map, regardless of options. jba.
    let map_extent = if plant(107) {
        h.terrain_extent()
    } else {
        h.terrain_maximum_pathfind_extent()
    };
    if !map_extent.is_in_region_no_z(&world_pos) && !plant(106) {
        return LegalBuildCode::RestrictedTerrain;
    }

    // check shroud level. This should be the first check, since returning other errors for shrouded areas could be used to
    // game the system
    if bit_test(options, SHROUD_REVEALED) && !plant(108) {
        let (x, y) = h.partition_world_to_cell(world_pos.x, world_pos.y);
        let mut player_index: i32 = -1;
        if let Some(p) = builder_object.and_then(|b| h.obj_controlling_player(b)) {
            player_index = if plant(109) { 0 } else { p };
        }
        if h.partition_shroud_status_for_player(player_index, x, y) != CellShroudStatus::Clear
            && !(plant(110)
                && h.partition_shroud_status_for_player(player_index, x, y)
                    == CellShroudStatus::Fogged)
        {
            return LegalBuildCode::Shroud;
        }
    }

    // if NO_OBJECT_OVERLAP is set, we are not allowed to construct 'build' if it would overlap any immobile objects, or an
    // enemy object. Friendly objects should politely "move out of the way" when you build something where they're standing
    if bit_test(options, NO_OBJECT_OVERLAP) && !plant(111) {
        let code = build_assistant_is_location_clear_of_objects(
            h,
            world_pos,
            build,
            angle,
            builder_object,
            options,
            player,
        );
        if code != LegalBuildCode::Ok {
            return code;
        }
    }
    // if NO_ENEMY_OBJECT_OVERLAP is set, we are not allowed to construct 'build' if it would overlap any enemy objects.
    // Friendly objects are ignored.
    if bit_test(options, NO_ENEMY_OBJECT_OVERLAP) && !plant(112) {
        let code = build_assistant_is_location_clear_of_objects(
            h,
            world_pos,
            build,
            angle,
            builder_object,
            options,
            player,
        );
        if code != LegalBuildCode::Ok {
            return code;
        }
    }

    let globals = h.build_globals();
    let build_geom = h.tmpl_geometry(build);
    if (h.tmpl_is_kind_of(build, KINDOF_CANNOT_BUILD_NEAR_SUPPLIES) || plant(113))
        && globals.supply_build_border > 0.0
    {
        // special case for supply centers: can't build too close to supply sources; see if there are any reasonably close by
        let range = build_geom.get_bounding_circle_radius()
            + globals.supply_build_border * if plant(114) { 1.0 } else { 2.0 };
        if let Some(too_close) = h.partition_closest_supply_source(world_pos, range) {
            // yep, see if we would collide with an expanded version
            let mut too_close_geom = h.obj_geometry(too_close);
            too_close_geom.expand_footprint(if plant(115) {
                0.0
            } else {
                globals.supply_build_border
            });
            let too_close_pos = h.obj_position(too_close);
            let too_close_orientation = h.obj_orientation(too_close);
            if h.partition_geom_collides_with_geom(
                world_pos,
                &build_geom,
                angle,
                too_close_pos,
                &too_close_geom,
                too_close_orientation,
            ) {
                h.terrain_visual_add_faction_bib(
                    too_close,
                    true,
                    if plant(116) {
                        0.0
                    } else {
                        globals.supply_build_border
                    },
                );
                return LegalBuildCode::TooCloseToSupplies;
            }
        }
    }

    // if clear path is requested check to see if the builder object can get there (unless it's a structure)
    if let Some(b) = builder_object {
        if bit_test(options, CLEAR_PATH) && (!h.obj_is_kind_of(b, KINDOF_IMMOBILE) || plant(117)) {
            // if there is no AI interface for this object, it cannot possible pass a clear path check since it will never be
            // able to move there
            if !h.obj_has_ai(b) && !plant(118) {
                return LegalBuildCode::NoClearPath;
            }
            if h.ai_is_quick_path_available(b, world_pos) == plant(119) {
                return LegalBuildCode::NoClearPath;
            }
        }
    }

    // check basic terrain restrictions
    if bit_test(options, TERRAIN_RESTRICTIONS) && !plant(120) {
        // we will take "samples" at this resolution across the footprint of where we are going to build the structure
        let sample_resolution = MAP_XY_FACTOR;
        // get the terrain extents
        let terrain_extent = h.terrain_extent();
        if h.terrain_layer_for_destination(world_pos) != LAYER_GROUND && !plant(121) {
            // we're on a bridge. This is somewhat restricted.
            return LegalBuildCode::RestrictedTerrain;
        }
        // check the footprint of where the structure would go to be clear of any non-buildable tiles and to make sure there
        // isn't a restricted tile and to make sure it's "flat" enough
        let mut sample_data = SampleBuildData {
            map_region: h.terrain_extent(),
            terrain_restricted: false,
            hi_z: if plant(122) {
                terrain_extent.hi.z
            } else {
                terrain_extent.lo.z
            }, // note we set hi point to lowest point
            lo_z: if plant(123) {
                terrain_extent.lo.z
            } else {
                terrain_extent.hi.z
            }, // note we set lo point to highest point
        };
        // quick check at triple res.
        build_assistant_iterate_footprint(
            h,
            build,
            angle,
            world_pos,
            if plant(124) { 2.0 } else { 3.0 } * sample_resolution,
            |hh, p| check_sample_build_location(hh, p, &mut sample_data),
        );
        if sample_data.terrain_restricted {
            return LegalBuildCode::RestrictedTerrain;
        }
        // check if the height across the whole footprint area is too varied (not flat enough)
        if sample_data.hi_z - sample_data.lo_z > globals.allowed_height_variation_for_building
            && !plant(125)
        {
            return LegalBuildCode::NotFlatEnough;
        }
        // careful check at full res.
        build_assistant_iterate_footprint(
            h,
            build,
            angle,
            world_pos,
            if plant(126) {
                3.0 * sample_resolution
            } else {
                sample_resolution
            },
            |hh, p| check_sample_build_location(hh, p, &mut sample_data),
        );
        if sample_data.terrain_restricted {
            return LegalBuildCode::RestrictedTerrain;
        }
        if sample_data.hi_z - sample_data.lo_z > globals.allowed_height_variation_for_building {
            return LegalBuildCode::NotFlatEnough;
        }
    }

    // we passed all the checks
    LegalBuildCode::Ok
}

/// `BuildAssistant::addBibs(worldPos, build)`: adds bibs to structures near to worldPos.
pub fn build_assistant_add_bibs<H: BuildAssistantHost>(
    h: &mut H,
    world_pos: Coord3D,
    build: TemplateRef,
) {
    let mut range = h.tmpl_vision_range(build);
    range += (if plant(127) { 2.0 } else { 3.0 }) * h.tmpl_geometry(build).get_major_radius();
    for them in h.partition_structures_in_range(world_pos, range) {
        // ignore any kind of class of objects that we will "remove" for building
        if build_assistant_is_removable_for_construction(h, them) && !plant(129) {
            continue;
        }
        if h.obj_is_kind_of(them, KINDOF_IMMOBILE) || plant(128) {
            h.terrain_visual_add_faction_bib(them, true, 0.0);
        }
    }
}

/// `BuildAssistant::buildTiledLocations(thingBeingTiled, angle, start, end, tilingSize, maxTiles, builderObject)`: fills the
/// scratch array of world positions starting at `start` and going to `end`; no more than `max_tiles` are considered and if
/// the position of any one of the objects in the chain is illegal the chain stops there.
#[allow(clippy::too_many_arguments)] // upstream's own parameter list
pub fn build_assistant_build_tiled_locations<H: BuildAssistantHost>(
    h: &mut H,
    ba: &mut BuildAssistant,
    thing_being_tiled: TemplateRef,
    angle: f32,
    start: Coord3D,
    end: Coord3D,
    tiling_size: f32,
    max_tiles: i32,
    builder_object: Option<ObjectId>,
) -> BResult<TileBuildInfo> {
    // we will fill out our own internal array of positions, it better be big enough to accomodate max tiles, if it's not
    // lets make it bigger!
    if max_tiles > ba.build_position_size + plant(130) as i32 {
        // delete the old array, create a new one
        ba.build_positions = vec![Coord3D::default(); max_tiles.max(0) as usize];
        ba.build_position_size = max_tiles;
    }
    // compute a vector from the start of the line in the world to the end
    let placement_vector = Coord3D {
        x: end.x - start.x,
        y: end.y - start.y,
        z: if plant(131) { end.z - start.z } else { 0.0 },
    };
    // get the length of the placement vector in the world, we'll use this to see how many objects we'll need to fill the
    // entire line
    let placement_length = placement_vector.length();
    // first see how many objects we're going to need to go from the placement line start to the placement line end, we are
    // guaranteed to have one thing (+1)
    let mut tiles_needed =
        real_to_int(placement_length / tiling_size) + if plant(132) { 0 } else { 1 };
    // we have a max that we are allowed to build to
    if tiles_needed > max_tiles + plant(133) as i32 {
        tiles_needed = max_tiles;
    }
    // we're guaranteed to have at least one object so lets take care of that position now (upstream writes `positions[ 0 ]`
    // of an array that `init` never allocated, or of one with no element: undefined)
    if ba.build_positions.is_empty() {
        return Err(BuildError::Undefined(
            "buildTiledLocations: the position array has no element",
        ));
    }
    ba.build_positions[0] = start;
    // march down each object and set the position based on its position in the line. note that we're skipping the first one
    // because we've already set it explicitly and are guaranteed to have at least one object
    let mut tiles_used = 1; // actual objects "tiled", we are guaranteed to have at least one
    let mut v = placement_vector;
    v.normalize();
    let mut i = 1;
    while i < tiles_needed {
        // compute position of object
        let x = v.x * (tiling_size * i as f32) + if plant(134) { end.x } else { start.x };
        let y = v.y * (tiling_size * i as f32) + start.y;
        let pos = Coord3D {
            x,
            y,
            z: if plant(135) {
                start.z
            } else {
                h.terrain_ground_height(x, y)
            },
        };
        // check for a legal position to be at and stop the tiling process if that becomes broken
        if build_assistant_is_location_legal_to_build(
            h,
            pos,
            thing_being_tiled,
            angle,
            USE_QUICK_PATHFIND
                | TERRAIN_RESTRICTIONS
                | if plant(137) { 0 } else { CLEAR_PATH }
                | NO_OBJECT_OVERLAP
                | if plant(136) { 0 } else { SHROUD_REVEALED },
            builder_object,
            if plant(138) { Some(0) } else { None },
        ) != LegalBuildCode::Ok
        {
            if !plant(139) {
                break;
            }
            i += 1;
            continue;
        }
        // save the position in the output array
        ba.build_positions[if plant(140) { 0 } else { i as usize }] = pos;
        // we have now actually used one more "tile"
        tiles_used += 1;
        i += 1;
    }
    // return a struct filled out with the actual tiles used and the array of locations
    Ok(TileBuildInfo {
        tiles_used,
        positions: ba.build_positions.clone(),
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// building and selling
// ---------------------------------------------------------------------------------------------------------------------

/// `BuildAssistant::buildObjectNow(constructorObject, what, pos, angle, owningPlayer)`: create an object from a build and put
/// it in the world now.
pub fn build_assistant_build_object_now<H: BuildAssistantHost>(
    h: &mut H,
    constructor_object: Option<ObjectId>,
    what: TemplateRef,
    pos: Coord3D,
    angle: f32,
    owning_player: PlayerIndex,
) -> BResult<Option<ObjectId>> {
    // Need to validate that we can make this in case someone fakes their CommandSet. A NULL constructor Object means a
    // script built building so let it slide.
    if let Some(c) = constructor_object {
        if !build_assistant_is_possible_to_make_unit(h, c, what)? && !plant(144) {
            return Ok(None);
        }
    }
    // clear out any objects from the building area that are "auto-clearable" when building
    if !plant(145) {
        clear_removable_for_construction(h, what, pos, angle);
    }

    if !move_objects_for_construction(h, what, pos, angle, owning_player) {
        // totally bogus. We tried to move our units out of the way, but they wouldn't. Chode-boys.
        if h.player_type(owning_player)
            == if plant(146) {
                PlayerType::Computer
            } else {
                PlayerType::Human
            }
        {
            return Ok(None); // ai gets to cheat. jba.
        }
    }

    // do the build
    if is_dozer(h, constructor_object)
        && (!build_assistant_is_line_build_template(h, what) || plant(147))
    {
        let c = constructor_object.expect("a dozer is an object");
        if h.obj_has_ai(c) {
            if !plant(148) {
                h.ai_idle(c); // stop any current behavior.
            }
            return Ok(h.ai_construct(c, what, pos, angle, owning_player));
        }
        return Ok(None);
    }
    // create the new object. We need to construct it with UnderConstruction set, since we are going to insta build it, but we
    // don't want to send double construction type events like power creation. onStructureConstructionComplete is called
    // below, so we need to simulate the proper object creation flow from the start. Be like Dozer.
    if !h.player_has_default_team(owning_player) {
        return Err(BuildError::Undefined(
            "buildObjectNow: the owning player has no default team",
        ));
    }
    let mut starting_status: Vec<ObjectStatusTypes> = Vec::new();
    if h.tmpl_is_kind_of(what, KINDOF_STRUCTURE) && !plant(149) {
        starting_status.push(OBJECT_STATUS_UNDER_CONSTRUCTION);
    }
    let obj = h.thing_factory_new_object(what, owning_player, &starting_status);
    h.obj_set_producer(obj, if plant(150) { None } else { constructor_object });

    // place on terrain surface
    let ground_pos = Coord3D {
        x: pos.x,
        y: pos.y,
        z: if plant(151) {
            pos.z
        } else {
            h.terrain_ground_height(pos.x, pos.y)
        },
    };
    h.obj_set_position(obj, ground_pos);
    h.obj_set_orientation(obj, angle);
    if !plant(152) {
        h.pathfinder_add_object(obj);
    }

    // notify the player that this thing has come into existence
    if h.obj_is_kind_of(obj, KINDOF_STRUCTURE) {
        // this is a cheat since this method is currently used by the solo AI to make buildings out of thin air
        if !plant(153) {
            h.player_on_structure_created(owning_player, constructor_object, obj);
        }
        if !plant(154) {
            h.player_on_structure_construction_complete(owning_player, constructor_object, obj);
        }
    } else {
        h.player_on_unit_created(
            owning_player,
            if plant(155) { None } else { constructor_object },
            obj,
        );
        // Call the voice created for the 1st object -- because it's possible to create multiple objects like redguards!
        let t = h.obj_template(obj);
        if !plant(156) {
            h.audio_add_voice_created(t, obj);
        }
    }

    // Now onCreates were called at the constructor. This magically created thing needs to be considered as Built for Game
    // specific stuff.
    for n in 0..h.obj_create_module_count(obj) {
        h.create_module_on_build_complete(obj, if plant(158) { 0 } else { n });
    }
    // Creation is another valid and essential time to call this. This building now Looks.
    if !plant(157) {
        h.obj_handle_partition_cell_maintenance(obj);
    }
    Ok(Some(obj))
}

/// `BuildAssistant::buildObjectLineNow(constructorObject, what, start, end, angle, owningPlayer)`: create a line of objects
/// end to end along the line defined in 3D space from start to end (walls).
#[allow(clippy::too_many_arguments)] // upstream's own parameter list
pub fn build_assistant_build_object_line_now<H: BuildAssistantHost>(
    h: &mut H,
    ba: &mut BuildAssistant,
    constructor_object: Option<ObjectId>,
    what: TemplateRef,
    start: Coord3D,
    end: Coord3D,
    angle: f32,
    owning_player: PlayerIndex,
) -> BResult<()> {
    // how big are each of our objects
    let object_size = h.tmpl_geometry(what).get_major_radius() * if plant(141) { 1.0 } else { 2.0 };
    // what is our max tiling length we can make
    let max_objects = h.build_globals().max_line_build_objects - plant(142) as i32;
    // build an array of locations that we want to build from start to end
    let tile_build_info = build_assistant_build_tiled_locations(
        h,
        ba,
        what,
        angle,
        start,
        end,
        object_size,
        max_objects,
        constructor_object,
    )?;
    // create an object at each position
    for i in 0..tile_build_info.tiles_used - plant(143) as i32 {
        build_assistant_build_object_now(
            h,
            constructor_object,
            what,
            tile_build_info.positions[i as usize],
            angle,
            owning_player,
        )?;
    }
    Ok(())
}

/// `BuildAssistant::sellObject(obj)`: start the process for selling the object in question.
pub fn build_assistant_sell_object<H: BuildAssistantHost>(
    h: &mut H,
    ba: &mut BuildAssistant,
    obj: ObjectId,
) {
    // we can only sell structures ... sanity check this
    if !h.obj_is_kind_of(obj, KINDOF_STRUCTURE) && !plant(159) {
        return;
    }
    // if object already has an entry in the sell list, we shouldn't try to sell it again
    if ba.sell_list.iter().any(|s| s.id == obj) && !plant(160) {
        return;
    }
    // set the construction percent of this object just below 100.0% so we can start counting down
    h.obj_set_construction_percent(obj, if plant(161) { 100.0 } else { 99.9 });

    // add this object to the list of objects being sold
    if plant(162) {
        ba.sell_list.push(ObjectSellInfo {
            id: obj,
            sell_frame: h.frame(),
        });
    } else {
        ba.sell_list.insert(
            0,
            ObjectSellInfo {
                id: obj,
                sell_frame: h.frame(),
            },
        );
    }

    // set the model condition in the drawable for this object that will show the buildup scaffold and adjust the model height
    // by construction percent
    h.obj_set_model_condition(
        obj,
        &[
            MODELCONDITION_PARTIALLY_CONSTRUCTED,
            if plant(163) {
                MODELCONDITION_SOLD
            } else {
                MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED
            },
        ],
    );

    // set this object as under de-construction (sold). It is still a legal target, since you get the money at the completion
    // of sale.
    h.obj_set_status(
        obj,
        &[
            OBJECT_STATUS_SOLD,
            if plant(164) {
                OBJECT_STATUS_UNDER_CONSTRUCTION
            } else {
                OBJECT_STATUS_UNSELECTABLE
            },
        ],
    );

    // for everybody, unselect them at this time. You can't just deselect a drawable. Selection is a logic property.
    if !plant(165) {
        h.deselect_object(obj);
    }

    // set the animation durations so that the regular build up loop animations can be done a bit faster for selling
    if h.obj_has_drawable(obj) && !plant(166) {
        h.drawable_set_animation_loop_duration(obj, (TOTAL_FRAMES_TO_SELL_OBJECT / 2.0) as u32);
    }

    // We also need to refund all production for the object at start-of-sell time
    if h.obj_has_production(obj) && !plant(167) {
        h.production_cancel_and_refund_all(obj);
    }

    // Tell the contain module so it can decide what to do.
    if h.obj_has_contain(obj) && !plant(168) {
        h.contain_on_selling(obj);
    }

    // Tell it to stop attacking or anything else it is doing
    if h.obj_has_ai(obj) && !plant(169) {
        h.ai_idle(obj);
    }

    // if it has parking places, kill anything parked there.
    if let Some(module) = h.obj_first_parking_place(obj) {
        if !plant(170) {
            h.parking_kill_all_parked_units(obj, module);
        }
    }

    // destroy any mines that are owned by this structure, right now. unfortunately, structures don't keep list of mines they
    // own, so we must do this the hard way :-( [fortunately, this doens't happen very often, so this is probably an
    // acceptable, if icky, solution.] (srj)
    for mine in h.all_objects() {
        if (h.obj_is_kind_of(mine, KINDOF_MINE)
            || plant(172) && h.obj_is_kind_of(mine, KINDOF_INERT))
            && (h.obj_producer_id(mine) == obj || plant(171))
        {
            h.destroy_object(mine);
        }
    }
}

/// (test-only) the planted bug that is switched on by `BA_PLANT`, for the differential tests. It exists only with the `differential`
/// feature: in every other build it is the constant `false`, so no environment variable can change the game's logic.
#[cfg(feature = "differential")]
#[doc(hidden)]
pub fn plant(k: u32) -> bool {
    use std::sync::OnceLock;
    static P: OnceLock<u32> = OnceLock::new();
    *P.get_or_init(|| {
        std::env::var("BA_PLANT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }) == k
}

/// Without the `differential` feature no bug is ever planted.
#[cfg(not(feature = "differential"))]
#[doc(hidden)]
#[inline(always)]
pub const fn plant(_k: u32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;

    fn sale(id: ObjectId, frame: u32) -> ObjectSellInfo {
        ObjectSellInfo {
            id,
            sell_frame: frame,
        }
    }

    /// The sell list saves as a count and then each sale's id and frame, in list order, and loads back the same; a file that stops
    /// short is an xfer error, which `?` turns into a `BuildError` that is not an undefined input.
    #[test]
    fn the_sell_list_round_trips_and_a_short_file_is_an_xfer_error() {
        let mut ba = BuildAssistant::new();
        ba.sell_list = vec![sale(7, 100), sale(3, 250)];
        let mut save = XferSave::new();
        save.open(b"ba").unwrap();
        build_assistant_xfer_the_sell_list(&mut ba, &mut save).unwrap();
        save.close().unwrap();
        let bytes = save.take_bytes();
        assert_eq!(bytes.len(), 4 + 2 * 8);

        let mut other = BuildAssistant::new();
        other.sell_list = vec![sale(1, 1)];
        let mut load = XferLoad::new(bytes.clone());
        load.open(b"ba").unwrap();
        build_assistant_xfer_the_sell_list(&mut other, &mut load).unwrap();
        assert_eq!(other.sell_list, ba.sell_list);

        let mut short = XferLoad::new(bytes[..bytes.len() - 3].to_vec());
        short.open(b"ba").unwrap();
        let err = build_assistant_xfer_the_sell_list(&mut other, &mut short).unwrap_err();
        assert!(matches!(err, BuildError::Xfer(_)));
        assert!(!err.is_undefined());
        assert!(BuildError::Undefined("x").is_undefined());
    }

    /// `reset` forgets the sales and keeps the position array.
    #[test]
    fn reset_clears_the_sell_list_only() {
        let mut ba = BuildAssistant::new();
        ba.build_positions = vec![Coord3D::default(); 3];
        ba.build_position_size = 3;
        ba.sell_list.push(sale(1, 1));
        build_assistant_reset(&mut ba);
        assert!(ba.sell_list.is_empty());
        assert_eq!(ba.build_position_size, 3);
        assert_eq!(ba.build_positions.len(), 3);
    }
}
