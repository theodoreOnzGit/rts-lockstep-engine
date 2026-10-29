// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPlayer.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AIPlayer.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `AIPlayer`: the computer-controlled opponent (`p1w13-sonnet`, layer 10, batch A28): the base it builds from its player's build
//! list (with dozers, rebuilding the destroyed and resuming the abandoned), the teams it trains (a build queue of work orders at
//! its factories, recruiting where it can, then a ready queue until they gather), supply trucks for its supply centres, dozers,
//! bridge repairs, upgrades and sciences, and the superweapon targets and safety checks its scripts ask for.
//!
//! The world is the host's ([`AiPlayerHost`]): the player and the others, their build lists, the templates, objects, teams and
//! team prototypes, the script engine, the build assistant, the partition queries and the terrain. Objects, teams, team prototypes,
//! templates and upgrades are handles. The AI's data is passed in ([`AiDataEntry`], `TheAI->getAiData()`).
//!
//! The two intrusive queues (`TeamBuildQueue`, `TeamReadyQueue`) are vectors, head first. Upstream's "remove, then restart the
//! iteration" loops reset to the head and then advance, so the head is not looked at again in that pass: kept (see
//! [`AiPlayer::ai_pre_team_destroy`]). Deleting a [`TeamInQueue`] activates its team (`~TeamInQueue`).
//!
//! # Upstream facts kept
//!
//! - `onUnitProduced` reads `supplyTruck` unset when no work order took the unit, or the unit has no supply truck AI: it is false
//!   here, the value the reference build's zero-initialised locals give (`-ftrivial-auto-var-init=zero`).
//! - `buildStructureNow` and `buildStructureWithDozer` never set the rally point (`gotOffset;` is a statement that does nothing).
//! - `computeSuperweaponTarget`'s fine pass moves `y` by the `x` step (`(x-5)`); a pass whose every value is below `-1` leaves its
//!   best position unset (undefined: [`AiPlayer::compute_superweapon_target`] refuses it).
//! - `getPlayerStructureBounds` returns the bounds of the non-conservative structures (`objBounds`), the conservative ones only
//!   deciding whether any structure was seen.
//! - A work order's `m_required` is the pool's zero unless set; `WorkOrder()` does not set it.
//! - The four legal-position searches differ in one place: `buildBySupplies` stops at a legal top (right) cell; the others try
//!   the next bottom (left) cell over it, so such a cell is kept only when it ends its row (column).
//! - A dozer's AI, and a building's builder's, are reached unchecked (`updateBridgeRepair`, `processBaseBuilding`): the host
//!   answers for them as if they exist.

use crate::common::base_type::{Coord2D, Coord3D, Region2D};
use crate::common::game_common::{GameDifficulty, Relationship};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::*;
use crate::common::object_status_types::{OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::science::ScienceType;
use crate::common::special_power_type::{SpecialPowerType, SPECIAL_SNEAK_ATTACK};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::ai::ai_data::AiDataEntry;
use crate::logic::ai::path::PATHFIND_CELL_SIZE_F;
use crate::logic::modules::plant;
use crate::logic::sides_list::BuildListInfo;
use crate::logic::world::team::TeamTemplateInfo;

/// A template handle (`const ThingTemplate *`).
pub type TemplateRef = u32;
/// A team handle (`Team *`, its id); `0` is none.
pub type TeamRef = u32;
/// A team prototype handle (`TeamPrototype *`).
pub type ProtoRef = u32;
/// An upgrade handle (`const UpgradeTemplate *`).
pub type UpgradeRef = u32;
/// A player index.
pub type PlayerRef = i32;

/// `INVALID_SKILLSET_SELECTION`.
pub const INVALID_SKILLSET_SELECTION: i32 = -1;
/// `MAX_STRUCTURES_TO_REPAIR`.
pub const MAX_STRUCTURES_TO_REPAIR: usize = 2;
/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: i32 = 30;
/// `SUPPLY_CENTER_CLOSE_DIST`.
const SUPPLY_CENTER_CLOSE_DIST: f32 = 20.0 * PATHFIND_CELL_SIZE_F;
/// `BODY_PRISTINE`.
const BODY_PRISTINE: i32 = 0;

/// `BuildAssistant::isLocationLegalToBuild`'s option bits.
pub mod legal {
    pub const TERRAIN_RESTRICTIONS: u32 = 0x0000_0001;
    pub const CLEAR_PATH: u32 = 0x0000_0002;
    pub const NO_OBJECT_OVERLAP: u32 = 0x0000_0004;
    pub const NO_ENEMY_OBJECT_OVERLAP: u32 = 0x0000_0008;
}

/// A command the AI player gives a unit's AI.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerAiCommand {
    /// `aiDock(obj, CMD_FROM_PLAYER)`.
    Dock(ObjectId),
    /// `aiIdle(CMD_FROM_AI)`.
    Idle,
    /// `aiMoveToPosition(pos, CMD_FROM_AI)`.
    MoveToPosition(Coord3D),
    /// `aiResumeConstruction(obj, CMD_FROM_AI)`.
    ResumeConstruction(ObjectId),
    /// `aiRepair(obj, CMD_FROM_AI)`.
    Repair(ObjectId),
    /// `aiFollowExitProductionPath(&path, NULL, CMD_FROM_AI)`.
    FollowExitProductionPath(Vec<Coord3D>),
    /// `joinTeam()`.
    JoinTeam,
}

/// `PartitionManager::getClosestObject(pos, radius, FROM_BOUNDINGSPHERE_2D, filters)` with one of the AI player's filter sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClosestQuery {
    /// `queueSupplyTruck`: a supply source, not the player's, on the map.
    SupplySourceNotMine,
    /// `findSupplyCenter`: a cash generator of the player's, on the map.
    MyCashGenerator,
    /// `isLocationSafe`: an enemy (not allied or neutral), alive, not stealthed undetected, not an insignificant building, not a
    /// harvester or a dozer.
    Threat,
}

/// What the AI player asks of its world.
pub trait AiPlayerHost {
    // ---- the clock, the random values, the script engine, the globals
    fn frame(&mut self) -> u32;
    fn logic_random_value(&mut self, lo: i32, hi: i32) -> i32;
    /// `TheGlobalData->m_debugAI` (as a truth value), `m_baseValuePerSupplyBox`.
    fn debug_ai(&mut self) -> bool;
    fn base_value_per_supply_box(&mut self) -> i32;
    /// `TheScriptEngine->AppendDebugMessage(msg, false)`.
    fn append_debug_message(&mut self, msg: &[u8]);
    fn add_object_to_cache(&mut self, o: ObjectId);
    fn run_object_script(&mut self, script: &[u8], o: ObjectId);
    /// `findScriptByName(name)`: (it exists, it has an action).
    fn find_script(&mut self, name: &[u8]) -> (bool, bool);
    /// `friend_executeAction(script->getAction(), team)` for the script named.
    fn execute_action(&mut self, script: &[u8], team: TeamRef);
    fn clear_team_flags(&mut self);
    /// `getSkirmishEnemyPlayer()->getPlayerIndex()`.
    fn skirmish_enemy_player(&mut self) -> PlayerRef;
    // ---- players
    fn player_exists(&mut self, p: PlayerRef) -> bool;
    /// `getMoney()->countMoney()`.
    fn money(&mut self, p: PlayerRef) -> u32;
    fn can_build_units(&mut self, p: PlayerRef) -> bool;
    fn set_can_build_units(&mut self, p: PlayerRef, b: bool);
    fn can_build_base(&mut self, p: PlayerRef) -> bool;
    fn side(&mut self, p: PlayerRef) -> Vec<u8>;
    fn attacked_frame(&mut self, p: PlayerRef) -> u32;
    fn science_purchase_points(&mut self, p: PlayerRef) -> i32;
    fn is_capable_of_purchasing_science(&mut self, p: PlayerRef, s: ScienceType) -> bool;
    fn attempt_to_purchase_science(&mut self, p: PlayerRef, s: ScienceType) -> bool;
    fn science_internal_name(&mut self, s: ScienceType) -> Vec<u8>;
    /// `keyToName(getPlayerNameKey())`.
    fn player_name(&mut self, p: PlayerRef) -> Vec<u8>;
    fn has_upgrade_in_production(&mut self, p: PlayerRef, u: UpgradeRef) -> bool;
    fn has_upgrade_complete(&mut self, p: PlayerRef, u: UpgradeRef) -> bool;
    fn default_team(&mut self, p: PlayerRef) -> TeamRef;
    /// `getRelationship(team)` of the player towards the team.
    fn relationship_to_team(&mut self, p: PlayerRef, team: TeamRef) -> Relationship;
    /// `getPlayerTeams()`: the player's team prototypes, in order.
    fn player_teams(&mut self, p: PlayerRef) -> Vec<ProtoRef>;
    /// The player's build list (`getBuildList()` and its links), in order.
    fn build_list(&mut self, p: PlayerRef) -> &mut Vec<BuildListInfo>;
    /// `addToBuildList(obj)`: a new entry at the head (the object's id, template, position, angle; no rebuilds).
    fn add_to_build_list(&mut self, p: PlayerRef, o: ObjectId);
    /// `addToPriorityBuildList(name, pos, angle)`: a new priority entry at the head (one rebuild).
    fn add_to_priority_build_list(&mut self, p: PlayerRef, name: &[u8], loc: &Coord3D, angle: f32);
    // ---- templates
    fn find_template(&mut self, name: &[u8]) -> Option<TemplateRef>;
    fn first_template(&mut self) -> Option<TemplateRef>;
    fn next_template(&mut self, t: TemplateRef) -> Option<TemplateRef>;
    fn template_name(&mut self, t: TemplateRef) -> Vec<u8>;
    fn template_is_kind_of(&mut self, t: TemplateRef, k: KindOfType) -> bool;
    fn template_cost(&mut self, t: TemplateRef, p: PlayerRef) -> i32;
    fn template_equivalent(&mut self, a: TemplateRef, b: TemplateRef) -> bool;
    fn template_bounding_radius(&mut self, t: TemplateRef) -> f32;
    fn template_placement_angle(&mut self, t: TemplateRef) -> f32;
    // ---- objects
    /// `getFirstObject()` .. `getNextObject()`.
    fn objects(&mut self) -> Vec<ObjectId>;
    fn object_exists(&mut self, o: ObjectId) -> bool;
    fn object_template(&mut self, o: ObjectId) -> TemplateRef;
    fn object_is_kind_of(&mut self, o: ObjectId, k: KindOfType) -> bool;
    fn position(&mut self, o: ObjectId) -> Coord3D;
    fn set_position(&mut self, o: ObjectId, pos: &Coord3D);
    fn bounding_radius(&mut self, o: ObjectId) -> f32;
    fn controlling_player(&mut self, o: ObjectId) -> Option<PlayerRef>;
    fn object_team(&mut self, o: ObjectId) -> TeamRef;
    fn set_team(&mut self, o: ObjectId, team: TeamRef);
    fn status_test(&mut self, o: ObjectId, s: crate::common::object_status_types::ObjectStatusTypes) -> bool;
    /// `clearStatus(MAKE_OBJECT_STATUS_MASK2(UNDER_CONSTRUCTION, RECONSTRUCTING))`.
    fn clear_construction_status(&mut self, o: ObjectId);
    fn builder_id(&mut self, o: ObjectId) -> ObjectId;
    fn is_significantly_above_terrain(&mut self, o: ObjectId) -> bool;
    /// `updateObjValuesFromMapProperties` of the dictionary of a build list entry's name, script, health and unsellable flag.
    fn update_obj_values_from_map_properties(&mut self, o: ObjectId, name: &[u8], script: &[u8], health: i32, unsellable: bool);
    fn update_upgrade_modules(&mut self, o: ObjectId);
    // ---- the object's AI
    fn has_ai(&mut self, o: ObjectId) -> bool;
    fn ai_is_idle(&mut self, o: ObjectId) -> bool;
    fn ai_goal_position(&mut self, o: ObjectId) -> Coord3D;
    fn ai_command(&mut self, o: ObjectId, cmd: PlayerAiCommand);
    /// The supply truck AI: present, `getPreferredDockID`, `isCurrentlyFerryingSupplies`, `isForcedIntoWantingState`.
    fn has_supply_truck_ai(&mut self, o: ObjectId) -> bool;
    fn preferred_dock(&mut self, o: ObjectId) -> ObjectId;
    fn ferrying_supplies(&mut self, o: ObjectId) -> bool;
    fn forced_into_wanting_state(&mut self, o: ObjectId) -> bool;
    fn set_force_wanting_state(&mut self, o: ObjectId, b: bool);
    /// The dozer AI: present, `isAnyTaskPending`, `isTaskPending(DOZER_TASK_BUILD)`.
    fn has_dozer_ai(&mut self, o: ObjectId) -> bool;
    fn dozer_any_task_pending(&mut self, o: ObjectId) -> bool;
    fn dozer_build_task_pending(&mut self, o: ObjectId) -> bool;
    // ---- the object's modules
    fn has_production(&mut self, o: ObjectId) -> bool;
    fn production_count(&mut self, o: ObjectId) -> i32;
    /// `queueCreateUnit(thing, requestUniqueUnitID())`.
    fn queue_create_unit(&mut self, o: ObjectId, t: TemplateRef) -> bool;
    fn queue_upgrade(&mut self, o: ObjectId, u: UpgradeRef) -> bool;
    /// The body: `getDamageState()` (`None`: no body); its last damage: (`m_noEffect`, the timestamp) (`None`: none).
    fn damage_state(&mut self, o: ObjectId) -> Option<i32>;
    fn last_damage(&mut self, o: ObjectId) -> Option<(bool, u32)>;
    fn has_supply_center_dock(&mut self, o: ObjectId) -> bool;
    /// `SupplyWarehouseDockUpdate::getBoxesStored()` (`None`: no such module).
    fn warehouse_boxes(&mut self, o: ObjectId) -> Option<i32>;
    /// The rebuild hole behaviour: (`getSpawnerID`, `getReconstructedBuildingID`) (`None`: none).
    fn rebuild_hole(&mut self, o: ObjectId) -> Option<(ObjectId, ObjectId)>;
    /// The exit interface: `None` without one; else its natural rally point (`None`: it has none).
    fn exit_natural_rally_point(&mut self, o: ObjectId) -> Option<Option<Coord3D>>;
    /// The upgrade names of the command set's buttons that name one (`None`: no command set).
    fn command_set_upgrades(&mut self, o: ObjectId) -> Option<Vec<Vec<u8>>>;
    // ---- teams and prototypes
    fn team_proto(&mut self, team: TeamRef) -> ProtoRef;
    fn team_name(&mut self, team: TeamRef) -> Vec<u8>;
    fn team_members(&mut self, team: TeamRef) -> Vec<ObjectId>;
    fn team_has_any_units(&mut self, team: TeamRef) -> bool;
    fn team_has_any_objects(&mut self, team: TeamRef) -> bool;
    fn team_is_idle(&mut self, team: TeamRef) -> bool;
    fn team_set_active(&mut self, team: TeamRef);
    fn team_transfer_units_to(&mut self, team: TeamRef, to: TeamRef);
    fn team_delete(&mut self, team: TeamRef);
    fn team_try_to_recruit(&mut self, team: TeamRef, t: TemplateRef, origin: &Coord3D, radius: f32) -> Option<ObjectId>;
    fn team_count_objects_by_template(&mut self, team: TeamRef, t: TemplateRef) -> i32;
    fn team_estimate_position(&mut self, team: TeamRef) -> Option<Coord3D>;
    /// `TheAI->createGroup()`, `getTeamAsAIGroup`, `groupGuardPosition(pos, GUARDMODE_NORMAL, CMD_FROM_SCRIPT)`.
    fn team_guard_position(&mut self, team: TeamRef, pos: &Coord3D);
    fn proto_name(&mut self, proto: ProtoRef) -> Vec<u8>;
    fn proto_info(&mut self, proto: ProtoRef) -> TeamTemplateInfo;
    fn proto_is_singleton(&mut self, proto: ProtoRef) -> bool;
    fn proto_controlling_player(&mut self, proto: ProtoRef) -> PlayerRef;
    fn proto_instances(&mut self, proto: ProtoRef) -> Vec<TeamRef>;
    fn proto_evaluate_production_condition(&mut self, proto: ProtoRef) -> bool;
    /// `TheTeamFactory->findTeam(name)`, `createInactiveTeam(name)`.
    fn find_team(&mut self, name: &[u8]) -> Option<TeamRef>;
    fn create_inactive_team(&mut self, name: &[u8]) -> TeamRef;
    /// `TheTeamFactory->findTeamByID(id)` (`0`: none).
    fn team_by_id(&mut self, id: u32) -> TeamRef;
    // ---- the build assistant, the partition, the pathfinder, the terrain, the upgrades
    fn build_object_now(&mut self, builder: Option<ObjectId>, t: TemplateRef, pos: &Coord3D, angle: f32, p: PlayerRef) -> Option<ObjectId>;
    fn is_location_legal_to_build(&mut self, pos: &Coord3D, t: TemplateRef, angle: f32, options: u32, builder: Option<ObjectId>, p: PlayerRef) -> bool;
    fn is_possible_to_make_unit(&mut self, factory: ObjectId, t: TemplateRef) -> bool;
    /// `TheTerrainVisual->removeAllBibs()`.
    fn remove_all_bibs(&mut self);
    fn closest_object(&mut self, q: ClosestQuery, p: PlayerRef, center: &Coord3D, radius: f32) -> Option<ObjectId>;
    fn quick_does_path_exist(&mut self, o: ObjectId, to: &Coord3D) -> bool;
    fn adjust_to_possible_destination(&mut self, o: ObjectId, pos: &mut Coord3D);
    fn ground_height(&mut self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->getExtent()` in x and y.
    fn extent(&mut self) -> Region2D;
    fn find_upgrade(&mut self, name: &[u8]) -> Option<UpgradeRef>;
    fn upgrade_is_object_type(&mut self, u: UpgradeRef) -> bool;
    fn upgrade_name(&mut self, u: UpgradeRef) -> Vec<u8>;
    fn can_afford_upgrade(&mut self, p: PlayerRef, u: UpgradeRef) -> bool;
    // ---- batch A29: the skirmish AI's questions
    /// `getEnergy()->hasSufficientPower()`.
    fn has_sufficient_power(&mut self, p: PlayerRef) -> bool;
    /// `isDisabledByType(DISABLED_UNMANNED)`.
    fn is_disabled_unmanned(&mut self, o: ObjectId) -> bool;
    /// `setBuilder(NULL)`.
    fn clear_builder(&mut self, o: ObjectId);
    /// `TheBuildAssistant->canMakeUnit(builder, whatToBuild) == CANMAKE_OK` (none: `CANMAKE_NO_PREREQ`).
    fn can_make_unit(&mut self, builder: ObjectId, t: Option<TemplateRef>) -> bool;
    /// `ThePlayerList->getPlayerCount()`.
    fn player_count(&mut self) -> i32;
    /// `getPlayerType() == PLAYER_HUMAN`.
    fn player_is_human(&mut self, p: PlayerRef) -> bool;
    /// `hasAnyUnits()`, `hasAnyBuildFacility()`, `hasAnyObjects()`.
    fn player_has_any_units(&mut self, p: PlayerRef) -> bool;
    fn player_has_any_build_facility(&mut self, p: PlayerRef) -> bool;
    fn player_has_any_objects(&mut self, p: PlayerRef) -> bool;
    /// `isSkirmishAIPlayer()` and `getCurrentEnemy()` of a player other than this AI's.
    fn player_is_skirmish_ai(&mut self, p: PlayerRef) -> bool;
    fn player_current_enemy(&mut self, p: PlayerRef) -> Option<PlayerRef>;
    /// `getMpStartIndex()`.
    fn mp_start_index(&mut self, p: PlayerRef) -> i32;
    /// `TheTerrainLogic->getClosestWaypointOnPath(pos, label)`'s location (`None`: no such path).
    fn closest_waypoint_on_path(&mut self, pos: &Coord3D, label: &[u8]) -> Option<Coord3D>;
    /// `TheTerrainLogic->getMaximumPathfindExtent()` in x and y.
    fn maximum_pathfind_extent(&mut self) -> Region2D;
    /// `TheAI->pathfinder()->findBrokenBridge(unit's locomotor set, unit's position, to, &bridge)`.
    fn find_broken_bridge(&mut self, unit: ObjectId, to: &Coord3D) -> Option<ObjectId>;
    /// `onStructureUndone(obj)`, `removeObjectFromPathfindMap(obj)` and `destroyObject(obj)` of the player's start structure.
    fn undo_and_destroy_structure(&mut self, o: ObjectId);
}

/// `WorkOrder`: one unit type of a team to build, the factory building it, how many are built and needed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkOrder {
    pub thing: Option<TemplateRef>,
    pub factory_id: ObjectId,
    pub num_completed: i32,
    pub num_required: i32,
    pub required: bool,
    pub is_resource_gatherer: bool,
}

impl WorkOrder {
    /// `WorkOrder()`: one required, not a gatherer (`m_required` the pool's zero).
    pub fn new() -> Self {
        WorkOrder { thing: None, factory_id: INVALID_ID, num_completed: 0, num_required: 1, required: false, is_resource_gatherer: false }
    }

    /// `isWaitingToBuild()`: no factory has it and more are needed.
    pub fn is_waiting_to_build(&self) -> bool {
        self.factory_id == INVALID_ID && self.num_completed < self.num_required
    }

    /// `validateFactory(thisPlayer)`: forgotten when its factory is gone or no longer the player's.
    pub fn validate_factory<H: AiPlayerHost>(&mut self, h: &mut H, player: PlayerRef) {
        if self.factory_id == INVALID_ID {
            return;
        }
        if !h.object_exists(self.factory_id) {
            self.factory_id = INVALID_ID;
            return;
        }
        if h.controlling_player(self.factory_id) != Some(player) && !plant(3580) {
            self.factory_id = INVALID_ID;
        }
    }

    /// `xfer` (version 1): the template's name, the factory, the counts and the flags.
    pub fn xfer<H: AiPlayerHost>(&mut self, h: &mut H, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut name = match self.thing {
            Some(t) => h.template_name(t),
            None => Vec::new(),
        };
        xfer.xfer_ascii_string(&mut name)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.thing = h.find_template(&name);
        }
        xfer.xfer_object_id(&mut self.factory_id)?;
        xfer.xfer_int(&mut self.num_completed)?;
        xfer.xfer_int(&mut self.num_required)?;
        xfer.xfer_bool(&mut self.required)?;
        xfer.xfer_bool(&mut self.is_resource_gatherer)
    }
}

/// `TeamInQueue`: a team being built (its work orders) or gathering.
#[derive(Clone, Debug, PartialEq)]
pub struct TeamInQueue {
    /// `m_workOrders` and their links, in order.
    pub work_orders: Vec<WorkOrder>,
    pub priority_build: bool,
    /// `m_team` (`0`: none).
    pub team: TeamRef,
    pub frame_started: i32,
    pub sent_to_start_location: bool,
    pub stop_queueing: bool,
    pub reinforcement: bool,
    pub reinforcement_id: ObjectId,
}

impl Default for TeamInQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl TeamInQueue {
    /// `TeamInQueue()`.
    pub fn new() -> Self {
        TeamInQueue {
            work_orders: Vec::new(),
            priority_build: false,
            team: 0,
            frame_started: 0,
            sent_to_start_location: false,
            stop_queueing: false,
            reinforcement: false,
            reinforcement_id: INVALID_ID,
        }
    }

    /// `isAllBuilt()`: every order has as many as it needs.
    pub fn is_all_built(&self) -> bool {
        !self.work_orders.iter().any(|o| o.num_required > o.num_completed)
    }

    /// `isBuildTimeExpired()`: the team's prototype's initial idle frames have passed since it started (none: never).
    pub fn is_build_time_expired<H: AiPlayerHost>(&self, h: &mut H) -> bool {
        let proto = h.team_proto(self.team);
        let idle = h.proto_info(proto).initial_idle_frames;
        if idle < 1 {
            return false; // "Unlimited time."
        }
        // (an UnsignedInt frame against an Int sum)
        h.frame() > (self.frame_started.wrapping_add(idle)) as u32
    }

    /// `isMinimumBuilt()`: every required order has (counting one in a factory) as many as it needs.
    pub fn is_minimum_built(&self) -> bool {
        for o in &self.work_orders {
            let mut count = o.num_completed;
            if o.factory_id != INVALID_ID {
                count += 1; // "we have one building."
            }
            if o.num_required > count && o.required && !plant(3581) {
                return false;
            }
        }
        true
    }

    /// `includesADozer()`: an order for a dozer that is not a resource gatherer.
    pub fn includes_a_dozer<H: AiPlayerHost>(&self, h: &mut H) -> bool {
        for o in &self.work_orders {
            let t = o.thing.expect("a work order's template");
            if h.template_is_kind_of(t, KINDOF_DOZER) && !o.is_resource_gatherer {
                return true;
            }
        }
        false
    }

    /// `areBuildsComplete()`: no order is in a factory.
    pub fn are_builds_complete(&self) -> bool {
        self.work_orders.iter().all(|o| o.factory_id == INVALID_ID)
    }

    /// `disband()`: the units to the prototype's player's default team, the team forgotten; returns it when it is to be deleted
    /// (not a singleton's): the caller deletes it with [`AiPlayer::delete_world_team`] (upstream's `m_team->deleteInstance()`,
    /// whose destructor tells every player's AI first).
    #[must_use]
    pub fn disband<H: AiPlayerHost>(&mut self, h: &mut H) -> Option<TeamRef> {
        let proto = h.team_proto(self.team);
        let owner = h.proto_controlling_player(proto);
        let new_team = h.default_team(owner);
        let mut name = h.proto_name(proto);
        name.extend_from_slice(b" - team disbanded, build time expired.");
        h.append_debug_message(&name);
        let mut deleted = None;
        if self.team != new_team {
            h.team_transfer_units_to(self.team, new_team);
            if !h.proto_is_singleton(proto) {
                deleted = Some(self.team);
            }
            self.team = 0;
        }
        deleted
    }

    /// `~TeamInQueue()`: its team activated.
    pub fn delete<H: AiPlayerHost>(self, h: &mut H) {
        if self.team != 0 {
            h.team_set_active(self.team);
        }
    }

    /// `xfer` (version 1): the work orders, the flags, the team's id, the frame and the reinforcement.
    pub fn xfer<H: AiPlayerHost>(&mut self, h: &mut H, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut count = self.work_orders.len() as u16;
        xfer.xfer_unsigned_short(&mut count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for o in &mut self.work_orders {
                o.xfer(h, xfer)?;
            }
        } else {
            if !self.work_orders.is_empty() {
                return Err(XferStatus::ErrorUnknown);
            }
            for _ in 0..count {
                // (appended, then read)
                self.work_orders.push(WorkOrder::new());
                let last = self.work_orders.len() - 1;
                self.work_orders[last].xfer(h, xfer)?;
            }
        }
        xfer.xfer_bool(&mut self.priority_build)?;
        let mut team_id = self.team;
        let mut b = team_id.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        team_id = u32::from_le_bytes(b);
        if xfer.get_xfer_mode() == XferMode::Load {
            self.team = h.team_by_id(team_id);
        }
        xfer.xfer_int(&mut self.frame_started)?;
        xfer.xfer_bool(&mut self.sent_to_start_location)?;
        xfer.xfer_bool(&mut self.stop_queueing)?;
        xfer.xfer_bool(&mut self.reinforcement)?;
        xfer.xfer_object_id(&mut self.reinforcement_id)
    }
}

/// Why an AI player operation is undefined upstream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AiPlayerUndefined {
    /// `computeSuperweaponTarget`: no grid position valued above `-1`, so its best position is never set.
    SuperweaponBestPositionUnset,
}

/// `AIPlayer`.
#[derive(Clone, Debug, PartialEq)]
pub struct AiPlayer {
    /// `m_player`.
    pub player: PlayerRef,
    /// Whether this is an `AISkirmishPlayer` (`isSkirmishAI()`).
    pub skirmish: bool,
    /// `TeamBuildQueue`, `TeamReadyQueue`, head first.
    pub build_queue: Vec<TeamInQueue>,
    pub ready_queue: Vec<TeamInQueue>,
    pub ready_to_build_team: bool,
    pub ready_to_build_structure: bool,
    pub team_timer: i32,
    pub structure_timer: i32,
    pub team_seconds: i32,
    pub build_delay: i32,
    pub team_delay: i32,
    pub frame_last_building_built: i32,
    pub difficulty: GameDifficulty,
    pub skillset_selector: i32,
    pub base_center: Coord3D,
    pub base_center_set: bool,
    pub base_radius: f32,
    pub structures_to_repair: [ObjectId; MAX_STRUCTURES_TO_REPAIR],
    pub repair_dozer: ObjectId,
    pub repair_dozer_origin: Coord3D,
    pub structures_in_queue: i32,
    pub dozer_queued_for_repair: bool,
    pub dozer_is_repairing: bool,
    pub bridge_timer: i32,
    pub supply_source_attack_check_frame: u32,
    pub attacked_supply_center: ObjectId,
    pub cur_warehouse_id: ObjectId,
    /// An `AISkirmishPlayer`'s own members (batch A29): its overrides are taken where this is set.
    pub skirmish_state: Option<crate::logic::ai::ai_skirmish_player::SkirmishState>,
}

/// `(Int)f`, x86 (`cvttss2si`): the indefinite value out of range.
fn c_int(f: f32) -> i32 {
    if f.is_nan() || f64::from(f) < -2_147_483_648.0 || f64::from(f) >= 2_147_483_648.0 {
        return i32::MIN;
    }
    f as i32
}

fn c_sqrt(x: f32) -> f32 {
    use petir::Real;
    Real::sqrt(f64::from(x)) as f32
}

fn concat(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut v = a.to_vec();
    v.extend_from_slice(b);
    v
}

/// The legal-position search of `buildStructureWithDozer` (with its skirmish steps), `buildBySupplies`,
/// `calcClosestConstructionZoneLocation` and `buildSpecificBuildingNearestTeam`: rings of growing size around `pos`, their bottom
/// and top rows (x from left to right, the top tried after each bottom cell), then their left and right columns. A legal bottom
/// (left) cell ends the search; a legal top (right) cell ends it only in `buildBySupplies` (`second_ends`): in the others the
/// next bottom (left) cell is tried over it, so it counts only when it is a row's (column's) last. Returns the position and
/// whether it is legal (the last cell tried when none is).
#[allow(clippy::too_many_arguments)]
fn search_legal<H: AiPlayerHost>(
    h: &mut H,
    pos: &Coord3D,
    t: TemplateRef,
    angle: f32,
    limit: f32,
    skirmish: bool,
    builder: Option<ObjectId>,
    p: PlayerRef,
    second_ends: bool,
) -> (Coord3D, bool) {
    let opts = legal::CLEAR_PATH | legal::TERRAIN_RESTRICTIONS | legal::NO_OBJECT_OVERLAP;
    let mut valid = false;
    let mut new_pos = *pos;
    let mut pos_offset = 0.0f32;
    while pos_offset < limit {
        if skirmish {
            pos_offset += 2.0 * PATHFIND_CELL_SIZE_F;
        }
        let offset = pos_offset / 2.0;
        let y_pos = pos.y - offset;
        let mut x_pos = pos.x - offset;
        while x_pos <= pos.x + offset {
            if skirmish {
                x_pos += PATHFIND_CELL_SIZE_F;
            }
            new_pos.x = x_pos;
            new_pos.y = y_pos;
            valid = h.is_location_legal_to_build(&new_pos, t, angle, opts, builder, p);
            if valid {
                break;
            }
            new_pos.y = y_pos + pos_offset;
            valid = h.is_location_legal_to_build(&new_pos, t, angle, opts, builder, p);
            if valid && second_ends && !plant(3595) {
                break;
            }
            x_pos += PATHFIND_CELL_SIZE_F;
        }
        if valid {
            break;
        }
        let x_pos = pos.x - offset;
        let mut y_pos = pos.y - offset;
        while y_pos <= pos.y + offset {
            if skirmish {
                y_pos += PATHFIND_CELL_SIZE_F;
            }
            new_pos.x = x_pos;
            new_pos.y = y_pos;
            valid = h.is_location_legal_to_build(&new_pos, t, angle, opts, builder, p);
            if valid {
                break;
            }
            new_pos.x = x_pos + pos_offset;
            valid = h.is_location_legal_to_build(&new_pos, t, angle, opts, builder, p);
            if valid && second_ends {
                break;
            }
            y_pos += PATHFIND_CELL_SIZE_F;
        }
        if valid {
            break;
        }
        pos_offset += 2.0 * PATHFIND_CELL_SIZE_F;
    }
    (new_pos, valid)
}

/// The player's teams (each prototype's instances, in order).
fn player_team_instances<H: AiPlayerHost>(h: &mut H, p: PlayerRef) -> Vec<TeamRef> {
    let mut v = Vec::new();
    for proto in h.player_teams(p) {
        v.extend(h.proto_instances(proto));
    }
    v
}

impl AiPlayer {
    /// `AIPlayer(p)`: its timers at 2 (no building before frame 1), the player's unit building turned off, the script engine's
    /// difficulty, the AI data's team seconds.
    pub fn new<H: AiPlayerHost>(h: &mut H, player: PlayerRef, skirmish: bool, global_difficulty: GameDifficulty, data: &AiDataEntry) -> Self {
        let frame = h.frame();
        h.set_can_build_units(player, false); // "turn off ai production by default."
        AiPlayer {
            player,
            skirmish,
            build_queue: Vec::new(),
            ready_queue: Vec::new(),
            ready_to_build_team: false,
            ready_to_build_structure: false,
            team_timer: 2,
            structure_timer: 2,
            // ("m_teamSeconds = TheAI->getAiData()->m_teamSeconds": a Real into an Int)
            team_seconds: c_int(data.data.team_seconds),
            build_delay: 0,
            team_delay: 0,
            frame_last_building_built: frame as i32,
            difficulty: global_difficulty,
            skillset_selector: INVALID_SKILLSET_SELECTION,
            base_center: Coord3D::default(),
            base_center_set: false,
            base_radius: 0.0,
            structures_to_repair: [INVALID_ID; MAX_STRUCTURES_TO_REPAIR],
            repair_dozer: INVALID_ID,
            repair_dozer_origin: Coord3D::default(),
            structures_in_queue: 0,
            dozer_queued_for_repair: false,
            dozer_is_repairing: false,
            bridge_timer: 0,
            supply_source_attack_check_frame: 0,
            attacked_supply_center: INVALID_ID,
            cur_warehouse_id: INVALID_ID,
            skirmish_state: None,
        }
    }

    /// `isSkirmishAI()`.
    pub fn is_skirmish_ai(&self) -> bool {
        self.skirmish
    }

    /// `getAIDifficulty()`.
    pub fn get_ai_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    /// `getBaseCenter(&pos)`.
    pub fn get_base_center(&self) -> (Coord3D, bool) {
        (self.base_center, self.base_center_set)
    }

    /// Deletes a team in a queue (`~TeamInQueue`).
    fn delete_team<H: AiPlayerHost>(h: &mut H, t: TeamInQueue) {
        t.delete(h);
    }

    /// `onStructureProduced(factory, bldg)`: the build list entry building it gets its map properties, script and supply centre
    /// set up; else an entry whose rebuild hole spawned it takes it.
    pub fn on_structure_produced<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, bldg: ObjectId) {
        self.team_delay = 0;
        self.build_delay = 0;
        let p = self.player;
        for i in 0..h.build_list(p).len() {
            if h.build_list(p)[i].object_id != bldg {
                continue;
            }
            let info = h.build_list(p)[i].clone();
            h.build_list(p)[i].under_construction = false;
            h.update_obj_values_from_map_properties(bldg, &info.building_name, &info.script, info.health, info.unsellable);
            h.clear_construction_status(bldg);
            h.update_upgrade_modules(bldg);
            h.add_object_to_cache(bldg);
            h.run_object_script(&info.script, bldg);
            if h.debug_ai() {
                let t = h.object_template(bldg);
                let name = h.template_name(t);
                h.append_debug_message(&concat(&name, b" - Building completed."));
            }
            self.check_for_supply_center(h, data, i, bldg);
            return;
        }
        for i in 0..h.build_list(p).len() {
            let name = h.build_list(p)[i].template_name.clone();
            let Some(plan) = h.find_template(&name) else { continue };
            let bt = h.object_template(bldg);
            if !h.template_equivalent(plan, bt) {
                continue; // "not the same kind of building we're looking for."
            }
            let id = h.build_list(p)[i].object_id;
            if id != INVALID_ID && h.object_exists(id) && h.object_is_kind_of(id, KINDOF_REBUILD_HOLE) {
                if let Some((_, spawned)) = h.rebuild_hole(id) {
                    if bldg == spawned && !plant(3582) {
                        h.build_list(p)[i].object_id = bldg;
                        return;
                    }
                }
            }
        }
    }

    /// `checkForSupplyCenter(info, bldg)`: a supply centre wants its side's gatherers for the difficulty, plus one ("a freebie with
    /// the supply depots").
    pub fn check_for_supply_center<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, info: usize, bldg: ObjectId) {
        if !h.has_supply_center_dock(bldg) {
            return;
        }
        let p = self.player;
        h.build_list(p)[info].is_supply_building = true;
        let mut desired = 0;
        let side = h.side(p);
        for res in &data.lists.side_info {
            if res.side == side {
                match self.difficulty {
                    GameDifficulty::Easy => desired = res.easy,
                    GameDifficulty::Normal => desired = res.normal,
                    GameDifficulty::Hard => desired = res.hard,
                    #[allow(unreachable_patterns)]
                    _ => {}
                }
            }
        }
        let e = &mut h.build_list(p)[info];
        e.is_supply_building = true;
        e.current_gatherers = -1;
        e.desired_gatherers = desired + if plant(3583) { 0 } else { 1 };
    }

    /// The player's harvesters with a supply truck AI.
    fn harvesters<H: AiPlayerHost>(&self, h: &mut H) -> Vec<ObjectId> {
        let mut v = Vec::new();
        for team in player_team_instances(h, self.player) {
            for o in h.team_members(team) {
                if !h.object_is_kind_of(o, KINDOF_HARVESTER) || !h.has_ai(o) {
                    continue;
                }
                if h.has_supply_truck_ai(o) {
                    v.push(o);
                }
            }
        }
        v
    }

    /// `queueSupplyTruck()`: unless one is queued, each supply building with its gatherers counts them again (and docks the idle
    /// ones; while its supply source has cash and is not an enemy's); one short re-attaches a loose truck, or queues one at a
    /// factory that can make a harvester.
    pub fn queue_supply_truck<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        let truck_in_queue = self.build_queue.iter().any(|t| t.work_orders.iter().any(|o| o.is_resource_gatherer));
        if truck_in_queue {
            return; // "already building a supply truck."
        }
        let total_harvesters = self.harvesters(h).len() as i32;
        let p = self.player;
        let mut i = 0;
        while i < h.build_list(p).len() {
            let info = h.build_list(p)[i].clone();
            i += 1;
            let idx = i - 1;
            if !info.is_supply_building {
                continue;
            }
            let desired = info.desired_gatherers;
            let cur = info.current_gatherers;
            if cur >= desired {
                let center_id = info.object_id;
                if !h.object_exists(center_id) {
                    continue;
                }
                if h.object_is_kind_of(center_id, KINDOF_REBUILD_HOLE) {
                    continue; // "don't consider rebuild holes."
                }
                let center = h.position(center_id);
                let radius = SUPPLY_CENTER_CLOSE_DIST + h.bounding_radius(center_id);
                let Some(source) = h.closest_object(ClosestQuery::SupplySourceNotMine, p, &center, radius) else { continue };
                if let Some(boxes) = h.warehouse_boxes(source) {
                    let available = boxes.wrapping_mul(h.base_value_per_supply_box());
                    if available <= 0 {
                        continue;
                    }
                    let team = h.object_team(source);
                    if h.relationship_to_team(p, team) == Relationship::Enemies {
                        continue;
                    }
                }
                self.check_for_supply_center(h, data, idx, center_id);
                let mut cur_gatherers = 0;
                for o in self.harvesters(h) {
                    if h.preferred_dock(o) == center_id {
                        cur_gatherers += 1;
                        if !h.ferrying_supplies(o) {
                            h.ai_command(o, PlayerAiCommand::Dock(center_id));
                        }
                    }
                }
                h.build_list(p)[idx].current_gatherers = cur_gatherers;
            } else {
                // "See if we have any "loose" harvesters (cause my supply center got nuked.)"
                for o in self.harvesters(h) {
                    let dock = h.preferred_dock(o);
                    if h.object_exists(dock) {
                        continue;
                    }
                    if h.ferrying_supplies(o) || h.forced_into_wanting_state(o) {
                        let center_id = h.build_list(p)[idx].object_id;
                        if h.object_exists(center_id) {
                            h.build_list(p)[idx].current_gatherers += 1;
                            h.ai_command(o, PlayerAiCommand::Dock(center_id));
                            return;
                        }
                    }
                }
                if total_harvesters >= desired.wrapping_mul(3) {
                    continue; // "we got lotsa gatherers."
                }
                let can_build_units = h.can_build_units(p);
                h.set_can_build_units(p, true);
                let mut t = h.first_template();
                while let Some(tt) = t {
                    if h.template_is_kind_of(tt, KINDOF_HARVESTER) {
                        if let Some(factory) = Self::find_factory(h, p, tt, false) {
                            let order = WorkOrder { thing: Some(tt), factory_id: INVALID_ID, num_required: 1, required: true, is_resource_gatherer: true, ..WorkOrder::new() };
                            let team = TeamInQueue {
                                work_orders: vec![order],
                                priority_build: true,
                                frame_started: h.frame() as i32,
                                team: h.default_team(p),
                                ..TeamInQueue::new()
                            };
                            self.build_queue.insert(0, team);
                            let ft = h.object_template(factory);
                            let fname = h.template_name(ft);
                            h.append_debug_message(&concat(b"Supply truck - building one at the ", &fname));
                            self.team_delay = 0;
                            if h.build_list(p)[idx].current_gatherers == -1 {
                                self.build_queue[0].work_orders[0].factory_id = factory;
                                h.build_list(p)[idx].current_gatherers = 0;
                            } else {
                                let team_name = h.team_name(self.build_queue[0].team);
                                let busy_ok = self.build_queue[0].priority_build;
                                Self::start_training(h, p, &mut self.build_queue[0].work_orders[0], busy_ok, &team_name);
                            }
                            break;
                        }
                    }
                    t = h.next_template(tt);
                }
                h.set_can_build_units(p, can_build_units);
            }
        }
    }

    /// `clearTeamsInQueue()`: both queues emptied from the head, each team deleted.
    pub fn clear_teams_in_queue<H: AiPlayerHost>(&mut self, h: &mut H) {
        while !self.build_queue.is_empty() {
            let t = self.build_queue.remove(0);
            Self::delete_team(h, t);
        }
        while !self.ready_queue.is_empty() {
            let t = self.ready_queue.remove(0);
            Self::delete_team(h, t);
        }
    }

    /// The exit interface's rally point as both builders look at it (and then leave it: `gotOffset;` sets nothing).
    fn look_at_rally_point<H: AiPlayerHost>(h: &mut H, bldg: ObjectId) {
        let _ = h.exit_natural_rally_point(bldg);
    }

    /// `buildStructureNow(bldgPlan, info)`: built at once where the list says, with its map properties and script.
    pub fn build_structure_now<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, plan: TemplateRef, info: usize) -> Option<ObjectId> {
        let p = self.player;
        let e = h.build_list(p)[info].clone();
        let bldg = h.build_object_now(None, plan, &e.location, e.angle, p)?;
        h.update_obj_values_from_map_properties(bldg, &e.building_name, &e.script, e.health, e.unsellable);
        let frame = h.frame();
        {
            let e = &mut h.build_list(p)[info];
            e.object_id = bldg;
            e.object_timestamp = frame.wrapping_add(1); // "has to be non-zero, so just add 1."
        }
        h.clear_construction_status(bldg);
        h.update_upgrade_modules(bldg);
        if h.debug_ai() {
            let name = h.template_name(plan);
            h.append_debug_message(&concat(&name, b" - Building completed."));
        }
        h.add_object_to_cache(bldg);
        h.run_object_script(&e.script, bldg);
        self.check_for_supply_center(h, data, info, bldg);
        Self::look_at_rally_point(h, bldg);
        Some(bldg)
    }

    /// `buildStructureWithDozer(bldgPlan, info)`: a dozer (one is queued when there is none) builds it where the list says, or at
    /// the nearest legal cell (the skirmish AI searching farther), teleported there when it has no path; `None` without a dozer,
    /// the money or a legal place.
    pub fn build_structure_with_dozer<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, plan: TemplateRef, info: usize) -> Option<ObjectId> {
        let p = self.player;
        let e = h.build_list(p)[info].clone();
        let dozer = self.find_dozer(h, data, &e.location)?;
        if h.money(p) < h.template_cost(plan, p) as u32 {
            return None;
        }
        let mut pos = e.location;
        pos.z += h.ground_height(pos.x, pos.y);
        if !h.has_ai(dozer) {
            return None;
        }
        let angle = e.angle;
        if !h.is_location_legal_to_build(&pos, plan, angle, legal::NO_ENEMY_OBJECT_OVERLAP, Some(dozer), p) {
            h.remove_all_bibs(); // "isLocationLegalToBuild adds bib feedback, turn it off."
            return None;
        }
        let opts = legal::CLEAR_PATH | legal::TERRAIN_RESTRICTIONS | legal::NO_OBJECT_OVERLAP;
        if !h.is_location_legal_to_build(&pos, plan, angle, opts, Some(dozer), p) {
            let name = h.template_name(plan);
            h.append_debug_message(&concat(&name, b" - Dozer unable to place.  Attempting to adjust position."));
            let limit = if self.skirmish { 120.0 * PATHFIND_CELL_SIZE_F } else { 10.0 * PATHFIND_CELL_SIZE_F };
            let (new_pos, mut valid) = search_legal(h, &pos, plan, angle, limit, self.skirmish, Some(dozer), p, false);
            if valid {
                pos = new_pos;
            }
            if !valid {
                valid = h.is_location_legal_to_build(&pos, plan, angle, legal::NO_ENEMY_OBJECT_OVERLAP, Some(dozer), p);
                if !valid {
                    return None;
                }
            }
        }
        h.remove_all_bibs();
        if !h.quick_does_path_exist(dozer, &pos) {
            let name = h.template_name(plan);
            h.append_debug_message(&concat(&name, b" - Dozer unable to reach building.  Teleporting."));
            h.set_position(dozer, &pos);
        }
        let bldg = h.build_object_now(Some(dozer), plan, &pos, angle, p);
        if let Some(b) = bldg {
            Self::look_at_rally_point(h, b);
            let frame = h.frame();
            {
                let e = &mut h.build_list(p)[info];
                e.object_id = b;
                e.object_timestamp = frame.wrapping_add(1);
                e.under_construction = true;
            }
            if h.debug_ai() {
                let name = h.template_name(plan);
                h.append_debug_message(&concat(&name, b" - Building started."));
            }
        }
        h.remove_all_bibs();
        bldg
    }
}

impl AiPlayer {
    /// The structure timer after a building is started: the structure seconds, divided by the poor or wealthy rate.
    pub(crate) fn reset_structure_timer<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        let d = &data.data;
        self.structure_timer = c_int(d.structure_seconds * LOGICFRAMES_PER_SECOND as f32);
        let money = h.money(self.player);
        if money < d.resources_poor as u32 {
            self.structure_timer = c_int(self.structure_timer as f32 / d.structures_poor_mod);
        } else if money > d.resources_wealthy as u32 {
            self.structure_timer = c_int(self.structure_timer as f32 / d.structures_wealthy_mod);
        }
    }

    /// `processBaseBuilding()`: when ready, each build list entry: a vanished building is forgotten (its rebuild hole taken), a
    /// building under construction gets its dozer back to work, another player's is forgotten; after the rebuild delay the first
    /// missing buildable one is started with a dozer (and the structure timer reset).
    pub fn process_base_building<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if self.skirmish_state.is_some() {
            return self.skirmish_process_base_building(h, data);
        }
        if !self.ready_to_build_structure {
            return;
        }
        let p = self.player;
        let mut i = 0;
        while i < h.build_list(p).len() {
            let idx = i;
            i += 1;
            let name = h.build_list(p)[idx].template_name.clone();
            if name.is_empty() {
                continue;
            }
            let Some(plan) = h.find_template(&name) else { continue };
            let id = h.build_list(p)[idx].object_id;
            if id != INVALID_ID {
                if !h.object_exists(id) {
                    let frame = h.frame();
                    {
                        let e = &mut h.build_list(p)[idx];
                        e.object_id = INVALID_ID;
                        e.object_timestamp = frame.wrapping_add(1);
                    }
                    for obj in h.objects() {
                        if !h.object_is_kind_of(obj, KINDOF_REBUILD_HOLE) {
                            continue;
                        }
                        if let Some((spawner, _)) = h.rebuild_hole(obj) {
                            if id == spawner && !plant(3584) {
                                h.build_list(p)[idx].object_id = obj;
                            }
                        }
                    }
                } else if h.controlling_player(id) == Some(p) {
                    if h.status_test(id, OBJECT_STATUS_UNDER_CONSTRUCTION) {
                        let builder = h.builder_id(id);
                        if !h.object_exists(builder) {
                            let pos = h.position(id);
                            match self.find_dozer(h, data, &pos) {
                                Some(d) if h.has_ai(d) => h.ai_command(d, PlayerAiCommand::ResumeConstruction(id)),
                                _ => continue,
                            }
                        } else {
                            h.ai_command(builder, PlayerAiCommand::ResumeConstruction(id));
                        }
                    }
                } else {
                    let frame = h.frame();
                    let e = &mut h.build_list(p)[idx];
                    e.object_id = INVALID_ID;
                    e.object_timestamp = frame.wrapping_add(1);
                }
            }
            let (oid, ts) = (h.build_list(p)[idx].object_id, h.build_list(p)[idx].object_timestamp);
            if oid == INVALID_ID && ts > 0 {
                let delay = (data.data.rebuild_delay_seconds.wrapping_mul(LOGICFRAMES_PER_SECOND)) as u32;
                if ts.wrapping_add(delay) > h.frame() {
                    continue;
                }
                h.build_list(p)[idx].object_timestamp = 0; // "ready to build."
            }
            if h.build_list(p)[idx].is_buildable() {
                let oid = h.build_list(p)[idx].object_id;
                if !h.object_exists(oid) {
                    if let Some(b) = self.build_structure_with_dozer(h, data, plan, idx) {
                        {
                            let e = &mut h.build_list(p)[idx];
                            e.object_id = b;
                            e.decrement_num_rebuilds();
                        }
                        self.ready_to_build_structure = false;
                        self.reset_structure_timer(h, data);
                        self.frame_last_building_built = h.frame() as i32;
                        break;
                    }
                }
            }
        }
    }

    /// Removes the teams of `team` from a queue: restarting at the head after each removal, and so (as upstream's restart and then
    /// advance) not looking at the new head again in this pass.
    fn remove_team_from<H: AiPlayerHost>(h: &mut H, queue: &mut Vec<TeamInQueue>, team: TeamRef) {
        let mut i = 0;
        while i < queue.len() {
            if queue[i].team == team {
                let t = queue.remove(i);
                Self::delete_team(h, t);
                i = 1;
                continue;
            }
            i += 1;
        }
    }

    /// A team deleted (`Team::~Team`): this AI player's queued teams for it go first (`Player::preTeamDestroy`, its
    /// [`AiPlayer::ai_pre_team_destroy`]), then the world deletes it. (The other players' AIs are the host's to tell.)
    pub fn delete_world_team<H: AiPlayerHost>(&mut self, h: &mut H, team: TeamRef) {
        if !plant(3596) {
            self.ai_pre_team_destroy(h, team);
        }
        h.team_delete(team);
    }

    /// `aiPreTeamDestroy(deletedTeam)`: the queued teams for it deleted.
    pub fn ai_pre_team_destroy<H: AiPlayerHost>(&mut self, h: &mut H, team: TeamRef) {
        Self::remove_team_from(h, &mut self.build_queue, team);
        if !plant(3585) {
            Self::remove_team_from(h, &mut self.ready_queue, team);
        }
    }

    /// `guardSupplyCenter(team, minSupplies)`: the team guards the attacked (else the best) supply centre, on its side away from the
    /// enemy's structures.
    pub fn guard_supply_center<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, team: TeamRef, min_supplies: i32) {
        self.supply_source_attack_check_frame = 0; // "force check."
        let mut warehouse = None;
        if self.is_supply_source_attacked(h) && h.object_exists(self.attacked_supply_center) {
            warehouse = Some(self.attacked_supply_center);
        }
        if warehouse.is_none() {
            warehouse = self.find_supply_center(h, data, min_supplies);
        }
        let Some(w) = warehouse else { return };
        let mut location = h.position(w);
        let enemy = h.skirmish_enemy_player();
        let bounds = Self::get_player_structure_bounds(h, enemy, false);
        let mut offset = Coord3D { x: location.x - (bounds.lo.x + bounds.hi.x) * 0.5, y: location.y - (bounds.lo.y + bounds.hi.y) * 0.5, z: 0.0 };
        offset.normalize();
        let radius = h.bounding_radius(w) * 0.8;
        location.x -= offset.x * radius;
        location.y -= offset.y * radius;
        h.team_guard_position(team, &location);
    }

    /// `isSupplySourceAttacked()`: at most every 10 frames after the player was attacked, whether a cash generator, dozer or
    /// harvester of the player's took damage (with an effect) in the last 10 frames (it is remembered).
    pub fn is_supply_source_attacked<H: AiPlayerHost>(&mut self, h: &mut H) -> bool {
        const SCAN_RATE: u32 = 10;
        let cur_frame = h.frame();
        if cur_frame == 0 {
            self.supply_source_attack_check_frame = cur_frame + SCAN_RATE;
            return false; // "can't be attacked on first frame."
        }
        self.attacked_supply_center = INVALID_ID;
        if cur_frame < self.supply_source_attack_check_frame {
            return false;
        }
        if h.attacked_frame(self.player).wrapping_add(SCAN_RATE) < cur_frame {
            return false; // "haven't been attacked recently."
        }
        self.supply_source_attack_check_frame = cur_frame.wrapping_add(SCAN_RATE);
        for team in player_team_instances(h, self.player) {
            for o in h.team_members(team) {
                if !h.object_is_kind_of(o, KINDOF_CASH_GENERATOR) && !h.object_is_kind_of(o, KINDOF_DOZER) && !h.object_is_kind_of(o, KINDOF_HARVESTER) {
                    continue;
                }
                if h.damage_state(o).is_some() {
                    if let Some((no_effect, stamp)) = h.last_damage(o) {
                        if no_effect {
                            continue;
                        }
                        if stamp.wrapping_add(SCAN_RATE) > cur_frame && !plant(3586) {
                            self.attacked_supply_center = o;
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// `isSupplySourceSafe(minSupplies)`: no supply centre is safe; else its location is checked.
    pub fn is_supply_source_safe<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, min_supplies: i32) -> bool {
        let Some(w) = self.find_supply_center(h, data, min_supplies) else { return true };
        let pos = h.position(w);
        let t = h.object_template(w);
        self.is_location_safe(h, data, &pos, Some(t))
    }

    /// `isLocationSafe(pos, tthing)`: no threat within the supply centre safe radius (plus the template's radius).
    pub fn is_location_safe<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, pos: &Coord3D, t: Option<TemplateRef>) -> bool {
        let Some(t) = t else { return false };
        let radius = data.data.supply_center_safe_radius + h.template_bounding_radius(t);
        h.closest_object(ClosestQuery::Threat, self.player, pos, radius).is_none()
    }

    /// `onUnitProduced(factory, unit)`: the first work order at the factory for the unit takes it (its team, its way home, a supply
    /// truck's dock); a dozer goes to the bridge repair it was queued for, or wakes the base building.
    pub fn on_unit_produced<H: AiPlayerHost>(&mut self, h: &mut H, factory: Option<ObjectId>, unit: ObjectId) {
        let Some(factory) = factory else { return };
        let p = self.player;
        let mut found = false;
        // (read unset upstream when nothing sets it: the zero-initialised build's false)
        let mut supply_truck = false;
        let ut = h.object_template(unit);
        'teams: for qi in 0..self.build_queue.len() {
            for oi in 0..self.build_queue[qi].work_orders.len() {
                let o = &self.build_queue[qi].work_orders[oi];
                let thing = o.thing.expect("a work order's template");
                if !(o.factory_id == factory && o.num_completed < o.num_required && h.template_equivalent(ut, thing)) {
                    continue;
                }
                self.build_queue[qi].work_orders[oi].num_completed += 1;
                let team = self.build_queue[qi].team;
                if team != 0 {
                    h.set_team(unit, team);
                }
                if self.build_queue[qi].reinforcement {
                    self.build_queue[qi].reinforcement_id = unit;
                }
                let has_ai = h.has_ai(unit);
                let proto = h.team_proto(team);
                let info = h.proto_info(proto);
                if info.has_home_location && has_ai {
                    let path = vec![h.ai_goal_position(unit), info.home_location];
                    h.ai_command(unit, PlayerAiCommand::FollowExitProductionPath(path));
                }
                self.build_queue[qi].work_orders[oi].factory_id = INVALID_ID; // "no longer using this factory."
                if has_ai && h.has_supply_truck_ai(unit) {
                    supply_truck = self.build_queue[qi].work_orders[oi].is_resource_gatherer;
                    h.set_force_wanting_state(unit, supply_truck);
                    if supply_truck {
                        for i in 0..h.build_list(p).len() {
                            let e = h.build_list(p)[i].clone();
                            if e.is_supply_building && e.desired_gatherers > 0 && e.desired_gatherers > e.current_gatherers && h.object_exists(e.object_id) {
                                h.build_list(p)[i].current_gatherers += 1;
                                h.ai_command(unit, PlayerAiCommand::Dock(e.object_id));
                            }
                        }
                    }
                }
                found = true;
                break 'teams;
            }
        }
        let _ = found;
        if !supply_truck && h.object_is_kind_of(unit, KINDOF_DOZER) {
            if self.dozer_queued_for_repair {
                self.repair_dozer = unit;
                self.dozer_queued_for_repair = false;
            } else {
                self.build_delay = 0;
                self.structure_timer = 1;
            }
        }
        self.team_delay = 0; // "Cause the update queues & selection to happen immediately."
    }

    /// `computeSuperweaponTarget(power, &retPos, playerNdx, weaponRadius)`: over the target player's structure bounds (the map's
    /// without any), a grid of at most 10 by 10 cells walked from a random corner, then 11 by 11 finer steps around its best cell;
    /// the best value's position (the mean of the ties), on the ground. Whether any value was above `-1`.
    pub fn compute_superweapon_target<H: AiPlayerHost>(
        &mut self,
        h: &mut H,
        power: SpecialPowerType,
        player_ndx: PlayerRef,
        weapon_radius: f32,
    ) -> Result<(Coord3D, bool), AiPlayerUndefined> {
        if self.skirmish_state.is_some() {
            return self.skirmish_compute_superweapon_target(h, power, player_ndx, weapon_radius);
        }
        self.base_compute_superweapon_target(h, power, player_ndx, weapon_radius)
    }

    /// `AIPlayer::computeSuperweaponTarget`.
    pub(crate) fn base_compute_superweapon_target<H: AiPlayerHost>(
        &mut self,
        h: &mut H,
        power: SpecialPowerType,
        player_ndx: PlayerRef,
        mut weapon_radius: f32,
    ) -> Result<(Coord3D, bool), AiPlayerUndefined> {
        let sneak_attack = power == SPECIAL_SNEAK_ATTACK;
        let mut bounds = Self::get_player_structure_bounds(h, player_ndx, false);
        if bounds.hi.x == 0.0 && bounds.lo.x == 0.0 && bounds.hi.y == 0.0 && bounds.lo.y == 0.0 {
            bounds = h.extent();
        }
        if weapon_radius < 1.0 {
            weapon_radius = 1.0; // "sanity to avoid divide by 0."
        }
        bounds.lo.x += weapon_radius;
        bounds.hi.x -= weapon_radius;
        if bounds.hi.x < bounds.lo.x {
            let m = (bounds.hi.x + bounds.lo.x) / 2.0;
            bounds.hi.x = m;
            bounds.lo.x = m;
        }
        if bounds.hi.y < bounds.lo.y {
            let m = (bounds.hi.y + bounds.lo.y) / 2.0;
            bounds.hi.y = m;
            bounds.lo.y = m;
        }
        let width = bounds.hi.x - bounds.lo.x;
        let height = bounds.hi.y - bounds.lo.y;
        let mut x_count = crate::common::base_type::real_to_int_ceil(width / weapon_radius) + 1;
        let mut y_count = crate::common::base_type::real_to_int_ceil(height / weapon_radius) + 1;
        x_count = x_count.min(10);
        y_count = y_count.min(10);
        let mut cash = -1;
        let target_military_units = !sneak_attack;
        let (x_delta, y_delta, x_start, y_start) = match h.logic_random_value(1, 4) {
            1 => (1, 1, 0, 0),
            2 => (-1, 1, x_count, 0),
            3 => (1, -1, 0, y_count),
            _ => (-1, -1, x_count, y_count),
        };
        let mut best_pos: Option<Coord3D> = None;
        let mut x_index = x_start;
        for _ in 0..x_count {
            let mut y_index = y_start;
            for _ in 0..y_count {
                let pos = Coord3D {
                    x: bounds.lo.x + (width * x_index as f32) / x_count as f32,
                    y: bounds.lo.y + (height * y_index as f32) / y_count as f32,
                    z: 0.0,
                };
                let cur = Self::get_player_superweapon_value(h, &pos, player_ndx, 2.0 * weapon_radius, target_military_units);
                if cur > cash {
                    cash = cur;
                    best_pos = Some(pos);
                }
                y_index += y_delta;
            }
            x_index += x_delta;
        }
        let Some(best_pos) = best_pos else { return Err(AiPlayerUndefined::SuperweaponBestPositionUnset) };
        let mut very_best = Coord3D::default();
        cash = -1;
        let mut count = 0;
        let mut any = false;
        for x in 0..11 {
            for _y in 0..11 {
                // (upstream's: y moves by the x step too)
                let step = if plant(3587) { (_y - 5) as f32 } else { (x - 5) as f32 };
                let pos = Coord3D { x: best_pos.x + (x - 5) as f32 * (weapon_radius / 10.0), y: best_pos.y + step * (weapon_radius / 10.0), z: 0.0 };
                let cur = Self::get_player_superweapon_value(h, &pos, player_ndx, weapon_radius, target_military_units);
                if cur > cash {
                    cash = cur;
                    very_best = pos;
                    count = 1;
                    any = true;
                } else if cur == cash {
                    very_best.x += pos.x;
                    very_best.y += pos.y;
                    count += 1;
                }
            }
        }
        if !any {
            return Err(AiPlayerUndefined::SuperweaponBestPositionUnset);
        }
        if count > 1 {
            very_best.x /= count as f32;
            very_best.y /= count as f32;
        }
        very_best.z = h.ground_height(very_best.x, very_best.y);
        Ok((very_best, cash > -1))
    }

    /// `getPlayerSuperweaponValue(center, playerNdx, radius, includeMilitaryUnits)`: the build cost of the player's objects within
    /// the radius (at least 4 cells), weighted from 1 at the centre to 0.5 at the edge; command centres and superweapons a tenth
    /// (or for a sneak attack five times); for a sneak attack the base defences and the fighting units count five times against;
    /// aircraft in the air not counted.
    pub fn get_player_superweapon_value<H: AiPlayerHost>(h: &mut H, center: &Coord3D, player_ndx: PlayerRef, mut radius: f32, include_military_units: bool) -> i32 {
        if radius < 4.0 * PATHFIND_CELL_SIZE_F {
            radius = 4.0 * PATHFIND_CELL_SIZE_F;
        }
        let mut cash = 0.0f32;
        let rad_sqr = radius * radius;
        if !h.player_exists(player_ndx) {
            return 0;
        }
        for team in player_team_instances(h, player_ndx) {
            for o in h.team_members(team) {
                let mut apply_neg_value = false;
                if !include_military_units {
                    if h.object_is_kind_of(o, KINDOF_FS_BASE_DEFENSE) || h.object_is_kind_of(o, KINDOF_TECH_BASE_DEFENSE) {
                        apply_neg_value = true; // "Hostile structure"
                    } else if h.object_is_kind_of(o, KINDOF_VEHICLE) || h.object_is_kind_of(o, KINDOF_INFANTRY) {
                        // "Hostile unit."
                        apply_neg_value = !h.object_is_kind_of(o, KINDOF_DOZER) && !h.object_is_kind_of(o, KINDOF_HARVESTER);
                    }
                } else if h.object_is_kind_of(o, KINDOF_AIRCRAFT) && h.is_significantly_above_terrain(o) {
                    continue; // "Don't target flying aircraft."
                }
                let pos = h.position(o);
                let dx = center.x - pos.x;
                let dy = center.y - pos.y;
                if dx * dx + dy * dy < rad_sqr {
                    let dist = c_sqrt(dx * dx + dy * dy);
                    let factor = 1.0 - (dist / (2.0 * radius));
                    let t = h.object_template(o);
                    let mut value = h.template_cost(t, player_ndx) as f32;
                    if h.object_is_kind_of(o, KINDOF_COMMANDCENTER) {
                        if !include_military_units {
                            value *= 5.0;
                        } else {
                            value /= 10.0;
                        }
                    }
                    if h.object_is_kind_of(o, KINDOF_FS_SUPERWEAPON) {
                        if !include_military_units {
                            value *= 5.0;
                        } else {
                            value /= 10.0;
                        }
                    }
                    if apply_neg_value {
                        cash -= factor * value * 5.0;
                    } else {
                        cash += factor * value;
                    }
                }
            }
        }
        c_int(cash)
    }

    /// `startTraining(order, busyOK, teamName)`: a factory that can make it queues one; the order remembers the factory.
    pub fn start_training<H: AiPlayerHost>(h: &mut H, p: PlayerRef, order: &mut WorkOrder, busy_ok: bool, team_name: &[u8]) -> bool {
        let thing = order.thing.expect("a work order's template");
        let Some(factory) = Self::find_factory(h, p, thing, busy_ok) else { return false };
        if h.has_production(factory) && h.queue_create_unit(factory, thing) {
            order.factory_id = factory;
            if h.debug_ai() {
                let mut s = b"Queuing ".to_vec();
                s.extend(h.template_name(thing));
                s.extend_from_slice(b" for ");
                s.extend_from_slice(team_name);
                h.append_debug_message(&s);
            }
            return true;
        }
        false
    }

    /// `findFactory(thing, busyOK)`: the first idle factory of the build list that can make it (finished, not sold, the
    /// player's; one no longer the player's is forgotten); with `busy_ok` the last busy one when none is idle.
    pub fn find_factory<H: AiPlayerHost>(h: &mut H, p: PlayerRef, thing: TemplateRef, busy_ok: bool) -> Option<ObjectId> {
        let mut busy_factory = None;
        for i in 0..h.build_list(p).len() {
            let f = h.build_list(p)[i].object_id;
            if !h.object_exists(f) {
                continue;
            }
            if h.controlling_player(f) != Some(p) {
                h.build_list(p)[i].object_id = INVALID_ID;
                continue;
            }
            if h.status_test(f, OBJECT_STATUS_UNDER_CONSTRUCTION) || h.status_test(f, OBJECT_STATUS_SOLD) {
                continue;
            }
            if !h.has_production(f) {
                continue;
            }
            if !h.is_possible_to_make_unit(f, thing) {
                continue;
            }
            let busy = h.production_count(f) > 0;
            if !busy && !plant(3588) {
                return Some(f); // "found a not busy factory."
            }
            if busy_ok {
                busy_factory = Some(f);
            }
        }
        if busy_ok {
            return busy_factory;
        }
        None
    }

    /// `isPossibleToBuildTeam(proto, requireIdleFactory, &notEnoughMoney)`: a factory (busy or not) for every unit type, the
    /// money for the average team times the resources-to-build factor, and (if asked) an idle factory for one of them.
    pub fn is_possible_to_build_team<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, proto: ProtoRef, require_idle_factory: bool) -> (bool, bool) {
        let p = self.player;
        let mut any_idle = false;
        let mut cost: i32 = 0;
        let info = h.proto_info(proto);
        for i in 0..info.num_units_info.max(0) as usize {
            let u = &info.units_info[i];
            if let Some(thing) = h.find_template(&u.unit_thing_name) {
                let thing_cost = h.template_cost(thing, p);
                if Self::find_factory(h, p, thing, true).is_none() {
                    return (false, false);
                }
                if Self::find_factory(h, p, thing, false).is_some() {
                    any_idle = true;
                }
                // ("cost += thingCost * ((max+min)/2.0f)": an Int += a Real)
                cost = c_int(cost as f32 + thing_cost as f32 * ((u.max_units + u.min_units) as f32 / 2.0));
            }
        }
        cost = c_int(cost as f32 * data.data.team_resources_to_build);
        if h.money(p) < cost as u32 {
            return (false, true); // "too expensive"
        }
        if any_idle {
            return (true, false);
        }
        (!require_idle_factory, false)
    }
}

impl AiPlayer {
    /// `isAGoodIdeaToBuildTeam(proto)`: its production condition holds, fewer than its maximum exist, none is being built, and it
    /// can be built with an idle factory.
    pub fn is_a_good_idea_to_build_team<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, proto: ProtoRef) -> bool {
        if !h.proto_evaluate_production_condition(proto) {
            return false;
        }
        let info = h.proto_info(proto);
        let instances = h.proto_instances(proto).len() as i32;
        if instances >= info.max_instances {
            if h.debug_ai() {
                let name = h.proto_name(proto);
                let msg = format!("Team {} not chosen - {} already exist.", String::from_utf8_lossy(&name), instances);
                h.append_debug_message(msg.as_bytes());
            }
            return false; // "Max already built."
        }
        for qi in 0..self.build_queue.len() {
            let t = self.build_queue[qi].team;
            if h.team_proto(t) == proto {
                return false; // "currently building one of these."
            }
        }
        let (possible, need_money) = self.is_possible_to_build_team(h, data, proto, true);
        if !possible {
            if h.debug_ai() {
                let name = String::from_utf8_lossy(&h.proto_name(proto)).into_owned();
                let msg = if need_money {
                    format!("Team {name} not chosen - Not enough money.")
                } else {
                    format!("Team {name} not chosen - Factory/tech missing or busy.")
                };
                h.append_debug_message(msg.as_bytes());
            }
            return false;
        }
        true
    }

    /// `selectTeamToReinforce(minPriority)`: of the player's automatically reinforced prototypes above the priority (none being
    /// built), the last team with units short of one type an idle factory can make; it recruits one nearby, else trains one.
    pub fn select_team_to_reinforce<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, min_priority: i32) -> bool {
        let p = self.player;
        let mut cur_team: TeamRef = 0;
        let mut cur_priority = min_priority;
        let mut cur_thing: Option<TemplateRef> = None;
        for proto in h.player_teams(p) {
            let mut busy = false;
            for qi in 0..self.build_queue.len() {
                let t = self.build_queue[qi].team;
                if h.team_proto(t) == proto {
                    busy = true;
                }
            }
            if busy {
                continue;
            }
            let info = h.proto_info(proto);
            if !(info.automatically_reinforce && info.production_priority > cur_priority) {
                continue;
            }
            for team in h.proto_instances(proto) {
                if !h.team_has_any_units(team) {
                    continue; // "empty."
                }
                let tp = h.team_proto(team);
                let tinfo = h.proto_info(tp);
                for i in 0..tinfo.num_units_info.max(0) as usize {
                    let u = &tinfo.units_info[i];
                    if u.max_units < 1 {
                        continue;
                    }
                    let Some(thing) = h.find_template(&u.unit_thing_name) else { continue };
                    let count = h.team_count_objects_by_template(team, thing);
                    if count < u.max_units && Self::find_factory(h, p, thing, false).is_some() {
                        cur_team = team;
                        cur_priority = info.production_priority;
                        cur_thing = Some(thing);
                    }
                }
            }
        }
        let (true, Some(thing)) = (cur_team != 0, cur_thing) else { return false };
        // "We have something to build."
        let order = WorkOrder { thing: Some(thing), factory_id: INVALID_ID, num_required: 1, required: true, ..WorkOrder::new() };
        let tq = TeamInQueue { priority_build: false, reinforcement: true, work_orders: vec![order], frame_started: h.frame() as i32, team: cur_team, ..TeamInQueue::new() };
        self.build_queue.insert(0, tq);
        let proto = h.team_proto(cur_team);
        let mut msg = h.proto_name(proto);
        msg.extend_from_slice(b" - AutoReinforcing one ");
        msg.extend(h.template_name(thing));
        h.append_debug_message(&msg);
        let mut origin = h.proto_info(proto).home_location;
        if let Some(&first) = h.team_members(cur_team).first() {
            origin = h.position(first);
        }
        if let Some(unit) = h.team_try_to_recruit(cur_team, thing, &origin, data.data.max_recruit_distance) {
            self.build_queue[0].work_orders[0].num_completed = 1;
            let from = h.object_team(unit);
            let from_proto = h.team_proto(from);
            let mut s = b"Team '".to_vec();
            s.extend(h.proto_name(proto));
            s.extend_from_slice(b"' recruits ");
            s.extend(h.template_name(thing));
            s.extend_from_slice(b" from team '");
            s.extend(h.proto_name(from_proto));
            s.extend_from_slice(b"'");
            h.append_debug_message(&s);
            h.set_team(unit, cur_team);
            self.build_queue[0].reinforcement_id = unit;
            if h.has_ai(unit) {
                h.ai_command(unit, PlayerAiCommand::Idle);
            }
        } else {
            let name = h.team_name(cur_team);
            let busy_ok = self.build_queue[0].priority_build;
            Self::start_training(h, p, &mut self.build_queue[0].work_orders[0], busy_ok, &name);
        }
        self.team_delay = 0;
        true
    }

    /// `selectTeamToBuild()`: a reinforcement above the best candidate's priority first; else a random one of the highest
    /// priority candidates is built and the team timer reset (divided by the poor or wealthy rate).
    pub fn select_team_to_build<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) -> bool {
        const INVALID_PRI: i32 = -99999;
        let mut hi_pri = INVALID_PRI;
        let mut candidates1 = Vec::new();
        for proto in h.player_teams(self.player) {
            if self.is_a_good_idea_to_build_team(h, data, proto) {
                candidates1.push(proto);
                let pri = h.proto_info(proto).production_priority;
                if pri > hi_pri {
                    hi_pri = pri;
                }
            }
        }
        if self.select_team_to_reinforce(h, data, hi_pri) {
            return true;
        }
        if hi_pri == INVALID_PRI {
            return false;
        }
        if h.debug_ai() {
            h.append_debug_message(b"**AI** Selecting team to build");
        }
        let mut candidates = Vec::new();
        for &proto in &candidates1 {
            if h.proto_info(proto).production_priority == hi_pri {
                candidates.push(proto);
            }
        }
        let which = h.logic_random_value(0, candidates.len() as i32 - 1);
        let Some(&proto) = candidates.get(which as usize) else { return false };
        if !h.proto_info(proto).has_home_location && !self.skirmish {
            let mut s = b"Error : team '".to_vec();
            s.extend(h.proto_name(proto));
            s.extend_from_slice(b"' has no Home Position (or Origin).");
            h.append_debug_message(&s);
        }
        self.build_specific_ai_team(h, data, Some(proto), false);
        self.ready_to_build_team = false;
        self.team_timer = self.team_seconds.wrapping_mul(LOGICFRAMES_PER_SECOND);
        let money = h.money(self.player);
        if money < data.data.resources_poor as u32 {
            self.team_timer = c_int(self.team_timer as f32 / data.data.team_poor_mod);
        } else if money > data.data.resources_wealthy as u32 {
            self.team_timer = c_int(self.team_timer as f32 / data.data.team_wealthy_mod);
        }
        true
    }

    /// `buildSpecificAIBuilding(thingName)`: the solo AI does not ("Error : Solo ai doesn't support BuildSpecificBuilding.").
    pub fn build_specific_ai_building<H: AiPlayerHost>(&mut self, h: &mut H, thing_name: &[u8]) {
        if self.skirmish_state.is_some() {
            return self.skirmish_build_specific_ai_building(h, thing_name);
        }
        let mut s = b"Error : Solo ai doesn't support BuildSpecificBuilding. '".to_vec();
        s.extend_from_slice(thing_name);
        s.extend_from_slice(b"' not built.");
        h.append_debug_message(&s);
    }

    /// `buildUpgrade(upgrade)`: a player upgrade not in production or complete and affordable is queued at the first finished
    /// factory whose command set has its button.
    pub fn build_upgrade<H: AiPlayerHost>(&mut self, h: &mut H, upgrade: &[u8]) {
        let p = self.player;
        let Some(u) = h.find_upgrade(upgrade) else {
            let mut m = b"Upgrade ".to_vec();
            m.extend_from_slice(upgrade);
            m.extend_from_slice(b" does not exist.  Ignoring request.");
            h.append_debug_message(&m);
            return;
        };
        if h.upgrade_is_object_type(u) {
            let mut m = b"Player build upgrade: Upgrade ".to_vec();
            m.extend_from_slice(upgrade);
            m.extend_from_slice(b" is an object, not a player upgrade.  Ignoring request.");
            h.append_debug_message(&m);
            return;
        }
        let refusal: Option<&[u8]> = if h.has_upgrade_in_production(p, u) {
            Some(b" queued.  Ignoring request.")
        } else if h.has_upgrade_complete(p, u) {
            Some(b" completed.  Ignoring request.")
        } else {
            None
        };
        if let Some(tail) = refusal {
            let mut m = h.player_name(p);
            m.extend_from_slice(b" already has upgrade ");
            m.extend_from_slice(upgrade);
            m.extend_from_slice(tail);
            h.append_debug_message(&m);
            return;
        }
        if !h.can_afford_upgrade(p, u) {
            let mut m = h.player_name(p);
            m.extend_from_slice(b" lacks money to build upgrade ");
            m.extend_from_slice(upgrade);
            m.extend_from_slice(b" at this time.  Ignoring request.");
            h.append_debug_message(&m);
            return;
        }
        let uname = h.upgrade_name(u);
        for i in 0..h.build_list(p).len() {
            let f = h.build_list(p)[i].object_id;
            if !h.object_exists(f) {
                continue;
            }
            if h.status_test(f, OBJECT_STATUS_UNDER_CONSTRUCTION) || h.status_test(f, OBJECT_STATUS_SOLD) {
                continue;
            }
            let Some(buttons) = h.command_set_upgrades(f) else { continue };
            if !buttons.contains(&uname) {
                continue;
            }
            if !h.has_production(f) {
                continue;
            }
            if h.queue_upgrade(f, u) {
                let mut m = h.player_name(p);
                m.extend_from_slice(b" queues ");
                m.extend_from_slice(&uname);
                m.extend_from_slice(b" at ");
                let ft = h.object_template(f);
                m.extend(h.template_name(ft));
                h.append_debug_message(&m);
                return;
            }
        }
        let mut m = h.player_name(p);
        m.extend_from_slice(b" lacks factory to build upgrade ");
        m.extend_from_slice(upgrade);
        m.extend_from_slice(b" at this time.  Ignoring request.");
        h.append_debug_message(&m);
    }

    /// `buildBySupplies(minimumCash, thingName)`: by the best supply centre (for anything but a cash generator, the current
    /// warehouse when it still exists), on its side toward the base (else away from the enemy's structures), at the nearest legal
    /// cell, added to the priority build list.
    pub fn build_by_supplies<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, minimum_cash: i32, thing_name: &[u8]) {
        let p = self.player;
        let mut best = self.find_supply_center(h, data, minimum_cash);
        // (upstream reads the template's kind before checking that it exists)
        let t = h.find_template(thing_name).expect("buildBySupplies: a known template");
        if !h.template_is_kind_of(t, KINDOF_CASH_GENERATOR) && h.object_exists(self.cur_warehouse_id) {
            best = Some(self.cur_warehouse_id);
        }
        let Some(w) = best else { return };
        let mut location = h.position(w);
        let mut offset = Coord2D { x: location.x - self.base_center.x, y: location.y - self.base_center.y };
        offset.normalize();
        let mut radius = 3.0 * PATHFIND_CELL_SIZE_F;
        if !h.template_is_kind_of(t, KINDOF_CASH_GENERATOR) {
            let enemy = h.skirmish_enemy_player();
            let bounds = Self::get_player_structure_bounds(h, enemy, false);
            offset.x = location.x - (bounds.lo.x + bounds.hi.x) * 0.5;
            offset.y = location.y - (bounds.lo.y + bounds.hi.y) * 0.5;
            offset.normalize();
            radius = h.bounding_radius(w);
        }
        location.x -= offset.x * radius;
        location.y -= offset.y * radius;
        let angle = h.template_placement_angle(t);
        let mut valid = false;
        let mut new_pos = location;
        if !h.is_location_legal_to_build(&location, t, angle, legal::NO_OBJECT_OVERLAP, None, p) {
            let wl = h.position(w);
            let msg = format!(" {} - buildBySupplies unable to place near dock at ({:.2},{:.2}).  Attempting to adjust position.", String::from_utf8_lossy(&h.template_name(t)), wl.x, wl.y);
            h.append_debug_message(msg.as_bytes());
            (new_pos, valid) = search_legal(h, &location, t, angle, 2.0 * SUPPLY_CENTER_CLOSE_DIST, false, None, p, true);
        }
        if valid {
            location = new_pos;
        }
        h.remove_all_bibs();
        location.z = 0.0; // "All build list locations are ground relative."
        h.add_to_priority_build_list(p, thing_name, &location, angle);
        self.cur_warehouse_id = w;
    }

    /// `calcClosestConstructionZoneLocation(constructTemplate, &location)`: the nearest legal cell (from the location itself);
    /// zero and false when there is none.
    pub fn calc_closest_construction_zone_location<H: AiPlayerHost>(&mut self, h: &mut H, t: Option<TemplateRef>, location: &mut Coord3D) -> bool {
        let Some(t) = t else { return false };
        let angle = h.template_placement_angle(t);
        let mut valid = false;
        let mut new_pos = *location;
        if !h.is_location_legal_to_build(location, t, angle, legal::NO_OBJECT_OVERLAP, None, self.player) {
            let mut m = h.template_name(t);
            m.extend_from_slice(b" - calcClosestConstructionZoneLocation unable to place.  Attempting to adjust position.");
            h.append_debug_message(&m);
            (new_pos, valid) = search_legal(h, location, t, angle, 2.0 * SUPPLY_CENTER_CLOSE_DIST, false, None, self.player, false);
        }
        let success = if valid {
            *location = new_pos;
            true
        } else {
            *location = Coord3D::default();
            false
        };
        h.remove_all_bibs();
        success
    }

    /// `buildSpecificBuildingNearestTeam(thingName, team)`: at the nearest legal cell to the team's estimated position (none when
    /// that position is already legal), added to the priority build list.
    pub fn build_specific_building_nearest_team<H: AiPlayerHost>(&mut self, h: &mut H, thing_name: &[u8], team: TeamRef) {
        let Some(t) = h.find_template(thing_name) else { return };
        if team == 0 {
            return;
        }
        let Some(location) = h.team_estimate_position(team) else { return };
        let angle = h.template_placement_angle(t);
        let mut valid = false;
        let mut new_pos = location;
        if !h.is_location_legal_to_build(&location, t, angle, legal::NO_OBJECT_OVERLAP, None, self.player) {
            let mut m = h.template_name(t);
            m.extend_from_slice(b" - buildSpecificBuildingNearestTeam unable to place.  Attempting to adjust position.");
            h.append_debug_message(&m);
            (new_pos, valid) = search_legal(h, &location, t, angle, 2.0 * SUPPLY_CENTER_CLOSE_DIST, false, None, self.player, false);
        }
        if valid && !plant(3589) {
            new_pos.z = 0.0;
            h.add_to_priority_build_list(self.player, thing_name, &new_pos, angle);
        }
        h.remove_all_bibs();
    }

    /// `findSupplyCenter(minimumCash)`: the supply source structure nearest the base with at least the cash (halved until a source
    /// is found or it falls to 100), not an enemy's, without a cash generator of the player's near it, and (with an enemy) nearer
    /// the base than the enemy's structures by the 0.4 to 0.6 weighting.
    pub fn find_supply_center<H: AiPlayerHost>(&mut self, h: &mut H, _data: &AiDataEntry, mut minimum_cash: i32) -> Option<ObjectId> {
        let p = self.player;
        let mut best: Option<ObjectId> = None;
        let mut best_dist_sqr = 0.0f32;
        let mut enemy_center = Coord3D::default();
        let enemy = self.get_ai_enemy(h);
        if let Some(e) = enemy {
            let b = Self::get_player_structure_bounds(h, e, false);
            enemy_center = Coord3D { x: (b.lo.x + b.hi.x) * 0.5, y: (b.lo.y + b.hi.y) * 0.5, z: 0.0 };
        }
        loop {
            for obj in h.objects() {
                if !h.object_is_kind_of(obj, KINDOF_STRUCTURE) || !h.object_is_kind_of(obj, KINDOF_SUPPLY_SOURCE) {
                    continue;
                }
                let Some(boxes) = h.warehouse_boxes(obj) else { continue };
                let available = boxes.wrapping_mul(h.base_value_per_supply_box());
                if available < minimum_cash {
                    continue;
                }
                let team = h.object_team(obj);
                if h.relationship_to_team(p, team) == Relationship::Enemies {
                    continue;
                }
                let center = h.position(obj);
                let radius = SUPPLY_CENTER_CLOSE_DIST + h.bounding_radius(obj);
                if h.closest_object(ClosestQuery::MyCashGenerator, p, &center, radius).is_some() {
                    continue;
                }
                let pos = h.position(obj);
                let mut dx = pos.x - self.base_center.x;
                let mut dy = pos.y - self.base_center.y;
                let dist_sqr = dx * dx + dy * dy;
                if enemy.is_some() {
                    dx = pos.x - enemy_center.x;
                    dy = pos.y - enemy_center.y;
                    // ("distSqr*0.4 > (dx*dx+dy*dy)*0.6f": the left side in double)
                    if f64::from(dist_sqr) * 0.4 > f64::from((dx * dx + dy * dy) * 0.6) {
                        continue;
                    }
                }
                if best.is_none() || best_dist_sqr > dist_sqr {
                    best = Some(obj);
                    best_dist_sqr = dist_sqr;
                }
            }
            if best.is_some() {
                break;
            }
            minimum_cash /= 2;
            if minimum_cash <= 100 {
                break;
            }
        }
        best
    }

    /// `getAiEnemy()`: none for the solo AI; the skirmish AI's ([`AiPlayer::skirmish_get_ai_enemy`]).
    pub fn get_ai_enemy<H: AiPlayerHost>(&mut self, h: &mut H) -> Option<PlayerRef> {
        if self.skirmish_state.is_some() {
            return self.skirmish_get_ai_enemy(h);
        }
        None
    }

    /// `buildAIBaseDefense(flank)`, `buildAIBaseDefenseStructure(thingName, flank)`: the solo AI does not (the skirmish AI's:
    /// batch A29).
    pub fn build_ai_base_defense<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, flank: bool) {
        if self.skirmish_state.is_some() {
            return self.skirmish_build_ai_base_defense(h, data, flank);
        }
        h.append_debug_message(b"Error : Solo ai doesn't support buildAIBaseDefense. '");
    }
    pub fn build_ai_base_defense_structure<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, thing_name: &[u8], flank: bool) {
        if self.skirmish_state.is_some() {
            return self.skirmish_build_ai_base_defense_structure(h, data, thing_name, flank);
        }
        h.append_debug_message(b"Error : Solo ai doesn't support buildAIBaseDefenseStructure. '");
    }

    /// `repairStructure(structure)`: a damaged structure not yet queued joins the repair queue (of two).
    pub fn repair_structure<H: AiPlayerHost>(&mut self, h: &mut H, structure: ObjectId) {
        if !h.object_exists(structure) {
            return;
        }
        let Some(state) = h.damage_state(structure) else { return };
        if state == BODY_PRISTINE {
            return;
        }
        for i in 0..self.structures_in_queue.max(0) as usize {
            if self.structures_to_repair[i] == structure {
                return; // "Bridge already queued for repair."
            }
        }
        if self.structures_in_queue == MAX_STRUCTURES_TO_REPAIR as i32 {
            return; // "Structure repair queue is full"
        }
        self.structures_to_repair[self.structures_in_queue as usize] = structure;
        self.structures_in_queue += 1;
    }

    /// `selectSkillset(skillset)`.
    pub fn select_skillset(&mut self, skillset: i32) {
        self.skillset_selector = skillset;
    }

    /// `updateBridgeRepair()`: every second, the first structure of the repair queue (the vanished ones dropped) gets a dozer (one
    /// found, else queued); a repairing dozer done with a pristine structure moves on, or (the queue empty) goes back to the base.
    pub fn update_bridge_repair<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if self.structures_in_queue == 0 {
            return;
        }
        self.bridge_timer -= 1;
        if self.bridge_timer > 0 {
            return;
        }
        self.bridge_timer = LOGICFRAMES_PER_SECOND;
        let mut bridge: Option<ObjectId> = None;
        while bridge.is_none() && self.structures_in_queue > 0 {
            let id = self.structures_to_repair[0];
            if h.object_exists(id) {
                bridge = Some(id);
            } else {
                self.pop_repair();
            }
        }
        if self.structures_in_queue == 0 {
            return;
        }
        let bridge = bridge.expect("a structure to repair");
        let bridge_pos = h.position(bridge);
        let bridge_state = h.damage_state(bridge).expect("the structure's body");
        if self.repair_dozer == INVALID_ID {
            self.dozer_is_repairing = false;
            if self.dozer_queued_for_repair {
                return; // "we're waiting for one."
            }
            if let Some(d) = self.find_dozer(h, data, &bridge_pos) {
                self.repair_dozer = d;
                self.repair_dozer_origin = h.position(d);
                h.ai_command(d, PlayerAiCommand::Repair(bridge));
                self.dozer_is_repairing = true;
                return;
            }
            self.queue_dozer(h);
            self.dozer_queued_for_repair = true;
            return;
        }
        let dozer = self.repair_dozer;
        if !h.object_exists(dozer) {
            self.repair_dozer = INVALID_ID; // "we got killed."
            self.bridge_timer = 0;
            return;
        }
        if !h.has_dozer_ai(dozer) {
            return; // ("Unexpected - dozer doesn't have dozer interface.")
        }
        if self.dozer_is_repairing {
            if h.dozer_any_task_pending(dozer) {
                return;
            }
            if bridge_state == BODY_PRISTINE {
                self.pop_repair();
                self.dozer_is_repairing = false;
                if self.structures_in_queue == 0 {
                    let mut pos = self.base_center;
                    if !self.base_center_set {
                        pos = self.repair_dozer_origin;
                    }
                    h.adjust_to_possible_destination(dozer, &mut pos);
                    h.ai_command(dozer, PlayerAiCommand::MoveToPosition(pos));
                    return;
                }
            }
        }
        h.ai_command(dozer, PlayerAiCommand::Repair(bridge));
        self.dozer_is_repairing = true;
    }

    /// The repair queue's first structure dropped.
    fn pop_repair(&mut self) {
        let n = self.structures_in_queue.max(0) as usize;
        for i in 0..n.saturating_sub(1) {
            self.structures_to_repair[i] = self.structures_to_repair[i + 1];
        }
        self.structures_in_queue -= 1;
    }
}

impl AiPlayer {
    /// `buildSpecificAITeam(teamProto, priorityBuild)`: unless units cannot be built (or a priority singleton exists), work orders
    /// for the optional units (each type's max less min) and then the required ones, each at the head of the list; the team
    /// queued at the head (a priority build) or the tail, a new inactive team for it, and its production condition's actions run.
    pub fn build_specific_ai_team<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry, proto: Option<ProtoRef>, priority_build: bool) {
        let Some(proto) = proto else { return };
        let p = self.player;
        let name = h.proto_name(proto);
        if !h.can_build_units(p) {
            let mut s = b"Can't build team '".to_vec();
            s.extend_from_slice(&name);
            s.extend_from_slice(b"' because build units is disabled.");
            h.append_debug_message(&s);
            return;
        }
        if priority_build && h.proto_is_singleton(proto) {
            if let Some(t) = h.find_team(&name) {
                if h.team_has_any_objects(t) {
                    // (upstream's message names no team)
                    h.append_debug_message(b"Unable to build singleton team '' because team already exists.");
                    return;
                }
            }
        }
        let (possible, need_money) = self.is_possible_to_build_team(h, data, proto, false);
        if !possible {
            if need_money {
                let mut s = b"Note - queueing team '".to_vec();
                s.extend_from_slice(&name);
                s.extend_from_slice(b"' but there is enough money.");
                h.append_debug_message(&s);
            } else {
                let mut s = b"Unable to build team '".to_vec();
                s.extend_from_slice(&name);
                s.extend_from_slice(b"' because required factories/tech don't exist.");
                h.append_debug_message(&s);
                return;
            }
        }
        let info = h.proto_info(proto);
        let n = info.num_units_info.max(0) as usize;
        // (each new order at the head)
        let mut orders: Vec<WorkOrder> = Vec::new();
        for i in 0..n {
            if let Some(thing) = h.find_template(&info.units_info[i].unit_thing_name) {
                let count = info.units_info[i].max_units - info.units_info[i].min_units;
                if count > 0 {
                    orders.insert(0, WorkOrder { thing: Some(thing), factory_id: INVALID_ID, num_required: count, ..WorkOrder::new() });
                }
            }
        }
        for i in 0..n {
            if let Some(thing) = h.find_template(&info.units_info[i].unit_thing_name) {
                let count = info.units_info[i].min_units;
                let order = WorkOrder { thing: Some(thing), factory_id: INVALID_ID, num_required: count, required: !plant(3590), ..WorkOrder::new() };
                orders.insert(0, order);
            }
        }
        if orders.is_empty() {
            if h.debug_ai() {
                h.append_debug_message(&concat(&name, b" - contains 0 buildable units."));
            }
            return;
        }
        // "We have something to build."
        let mut team = TeamInQueue::new();
        let at = if priority_build {
            team.priority_build = true;
            0
        } else {
            team.priority_build = false;
            self.build_queue.len()
        };
        team.work_orders = orders;
        team.frame_started = h.frame() as i32;
        team.team = h.create_inactive_team(&name);
        self.build_queue.insert(at, team);
        h.append_debug_message(&concat(&name, b" - starting team build."));
        self.team_delay = 0;
        let t = self.build_queue[at].team;
        let tproto = h.team_proto(t);
        let tinfo = h.proto_info(tproto);
        if tinfo.execute_actions {
            let (exists, has_action) = h.find_script(&tinfo.production_condition);
            if exists && has_action {
                h.execute_action(&tinfo.production_condition, t);
            }
        }
    }

    /// `recruitSpecificAITeam(teamProto, recruitRadius)`: a new inactive team recruits up to each type's maximum from around the
    /// home location (each moving there); with any, it joins the ready queue, else it is deleted (not a singleton's).
    pub fn recruit_specific_ai_team<H: AiPlayerHost>(&mut self, h: &mut H, proto: Option<ProtoRef>, mut recruit_radius: f32) {
        if recruit_radius < 1.0 {
            recruit_radius = 99999.0;
        }
        let Some(proto) = proto else { return };
        let name = h.proto_name(proto);
        if h.proto_is_singleton(proto) {
            if let Some(t) = h.find_team(&name) {
                if h.team_has_any_objects(t) {
                    h.append_debug_message(b"Unable to recruit singleton team '' because team already exists.");
                    return;
                }
            }
        }
        let info = h.proto_info(proto);
        // (the skirmish AI's copy reports it for itself too)
        if !info.has_home_location && (!self.skirmish || self.skirmish_state.is_some()) {
            let mut s = b"Error : team '".to_vec();
            s.extend_from_slice(&name);
            s.extend_from_slice(b"' has no Home Position (or Origin).");
            h.append_debug_message(&s);
        }
        let the_team = h.create_inactive_team(&name);
        h.append_debug_message(&concat(&name, b" - Recruiting."));
        let mut recruited = 0;
        for i in 0..info.num_units_info.max(0) as usize {
            let Some(thing) = h.find_template(&info.units_info[i].unit_thing_name) else { continue };
            let mut count = info.units_info[i].max_units;
            while count > 0 {
                let Some(unit) = h.team_try_to_recruit(the_team, thing, &info.home_location, recruit_radius) else { break };
                recruited += 1;
                let from = h.object_team(unit);
                let from_proto = h.team_proto(from);
                let tp = h.team_proto(the_team);
                let mut s = b"Team '".to_vec();
                s.extend(h.proto_name(tp));
                s.extend_from_slice(b"' recruits ");
                s.extend(h.template_name(thing));
                s.extend_from_slice(b" from team '");
                s.extend(h.proto_name(from_proto));
                s.extend_from_slice(b"'");
                h.append_debug_message(&s);
                h.set_team(unit, the_team);
                if h.has_ai(unit) {
                    h.ai_command(unit, PlayerAiCommand::MoveToPosition(info.home_location));
                }
                count -= 1;
            }
        }
        if recruited > 0 && !plant(3591) {
            let tq = TeamInQueue { priority_build: false, frame_started: h.frame() as i32, team: the_team, ..TeamInQueue::new() };
            self.ready_queue.insert(0, tq);
            h.append_debug_message(&concat(&name, b" - Finished recruiting."));
        } else {
            let tp = h.team_proto(the_team);
            if !h.proto_is_singleton(tp) {
                self.delete_world_team(h, the_team);
            }
            h.append_debug_message(&concat(&name, b" - Recruited 0 units, disbanding."));
        }
    }

    /// `processTeamBuilding()`.
    pub fn process_team_building<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if self.select_team_to_build(h, data) {
            self.queue_units(h, data);
        }
    }

    /// `queueUnits()`: a supply truck if wanted; then each waiting work order recruits what it can (each sent home, or idled),
    /// and trains the rest (an order in a factory has its factory checked).
    pub fn queue_units<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        self.queue_supply_truck(h, data);
        let p = self.player;
        for qi in 0..self.build_queue.len() {
            for oi in 0..self.build_queue[qi].work_orders.len() {
                let team = self.build_queue[qi].team;
                let proto = h.team_proto(team);
                let info = h.proto_info(proto);
                let mut home = info.home_location;
                let has_home = if info.has_home_location {
                    true
                } else {
                    let (c, set) = self.get_base_center();
                    home = c;
                    set
                };
                while self.build_queue[qi].work_orders[oi].is_waiting_to_build() {
                    let thing = self.build_queue[qi].work_orders[oi].thing.expect("a work order's template");
                    let Some(unit) = h.team_try_to_recruit(team, thing, &home, data.data.max_recruit_distance) else { break };
                    self.build_queue[qi].work_orders[oi].num_completed += 1;
                    let from = h.object_team(unit);
                    let from_proto = h.team_proto(from);
                    let mut s = b"Team '".to_vec();
                    s.extend(h.proto_name(proto));
                    s.extend_from_slice(b"' recruits ");
                    s.extend(h.template_name(thing));
                    s.extend_from_slice(b" from team '");
                    s.extend(h.proto_name(from_proto));
                    s.extend_from_slice(b"'");
                    h.append_debug_message(&s);
                    h.set_team(unit, team);
                    // (upstream commands the unit's AI unchecked)
                    if has_home && !plant(3592) {
                        h.ai_command(unit, PlayerAiCommand::MoveToPosition(home));
                    } else {
                        h.ai_command(unit, PlayerAiCommand::Idle); // "stop, you've been recruited."
                    }
                }
                if self.build_queue[qi].work_orders[oi].is_waiting_to_build() {
                    let name = h.team_name(team);
                    let busy_ok = self.build_queue[qi].priority_build;
                    Self::start_training(h, p, &mut self.build_queue[qi].work_orders[oi], busy_ok, &name);
                } else {
                    self.build_queue[qi].work_orders[oi].validate_factory(h, p);
                }
            }
        }
    }

    /// `doBaseBuilding()`: the structure timer counts down to readiness; the build delay to the next base building pass (then two
    /// seconds, unless the pass set it).
    pub fn do_base_building<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if self.skirmish_state.is_some() {
            return self.skirmish_do_base_building(h, data);
        }
        if !h.can_build_base(self.player) {
            return;
        }
        if !self.ready_to_build_structure {
            self.structure_timer -= 1;
            if self.structure_timer <= 0 {
                self.ready_to_build_structure = true;
                self.build_delay = 0;
            }
        }
        self.build_delay -= 1;
        if self.build_delay < 1 {
            if self.ready_to_build_structure {
                self.process_base_building(h, data);
            }
            if self.build_delay < 1 {
                self.build_delay = 2 * LOGICFRAMES_PER_SECOND; // "check again in 2 seconds."
            }
        }
    }

    /// `checkReadyTeams()`: a ready team all idle (a reinforcement: its unit idle; with an action to run: any idle; or after a
    /// minute) leaves the queue: the reinforcement joins its team, a team is activated; restarting at the head after each.
    pub fn check_ready_teams<H: AiPlayerHost>(&mut self, h: &mut H) {
        let mut i = 0;
        while i < self.ready_queue.len() {
            let t = self.ready_queue[i].clone();
            let time_expired = (t.frame_started.wrapping_add(60 * LOGICFRAMES_PER_SECOND) as u32) < h.frame();
            let mut all_idle = true;
            let mut any_idle = false;
            if t.reinforcement {
                if h.object_exists(t.reinforcement_id) && h.has_ai(t.reinforcement_id) {
                    all_idle = h.ai_is_idle(t.reinforcement_id);
                    any_idle = all_idle;
                }
            } else {
                all_idle = h.team_is_idle(t.team);
                for o in h.team_members(t.team) {
                    if h.has_ai(o) && h.ai_is_idle(o) {
                        any_idle = true;
                    }
                }
            }
            let proto = h.team_proto(t.team);
            let info = h.proto_info(proto);
            if any_idle && info.execute_actions {
                let (exists, has_action) = h.find_script(&info.production_condition);
                if exists && has_action {
                    all_idle = true;
                }
            }
            if time_expired {
                all_idle = true;
            }
            if all_idle {
                if !t.sent_to_start_location {
                    self.ready_queue[i].sent_to_start_location = true;
                }
                let t = self.ready_queue.remove(i);
                if t.reinforcement {
                    if h.object_exists(t.reinforcement_id) && h.has_ai(t.reinforcement_id) {
                        h.ai_command(t.reinforcement_id, PlayerAiCommand::JoinTeam);
                    }
                } else {
                    h.team_set_active(t.team);
                    if self.skirmish {
                        h.clear_team_flags();
                    }
                    if h.debug_ai() {
                        let mut s = h.proto_name(proto);
                        s.extend_from_slice(b" - team activated.");
                        h.append_debug_message(&s);
                    }
                }
                Self::delete_team(h, t);
                // (restarted at the head, then advanced)
                i = 1;
                continue;
            }
            i += 1;
        }
    }

    /// `checkQueuedTeams()`: a team out of build time joins the ready queue when its minimum is built and nothing is in a
    /// factory (else it waits), or is disbanded; then a team all built joins the ready queue, and an unfinished one with an idle
    /// unit runs its production condition's actions. Each pass restarts at the head after a move.
    pub fn check_queued_teams<H: AiPlayerHost>(&mut self, h: &mut H) {
        let mut i = 0;
        while i < self.build_queue.len() {
            if self.build_queue[i].is_build_time_expired(h) {
                if self.build_queue[i].is_minimum_built() {
                    if self.build_queue[i].are_builds_complete() {
                        let t = self.build_queue.remove(i);
                        self.ready_queue.insert(0, t);
                    } else {
                        i += 1;
                        continue;
                    }
                } else {
                    let mut t = self.build_queue.remove(i);
                    if let Some(team) = t.disband(h) {
                        self.delete_world_team(h, team);
                    }
                    Self::delete_team(h, t);
                    if self.skirmish {
                        h.clear_team_flags();
                    }
                }
                i = 1;
                continue;
            }
            i += 1;
        }
        let mut i = 0;
        while i < self.build_queue.len() {
            if self.build_queue[i].is_all_built() && !plant(3593) {
                let t = self.build_queue.remove(i);
                self.ready_queue.insert(0, t);
                i = 1;
                continue;
            }
            let team = self.build_queue[i].team;
            let mut any_idle = false;
            for o in h.team_members(team) {
                if h.has_ai(o) && h.ai_is_idle(o) {
                    any_idle = true;
                }
            }
            if any_idle {
                let proto = h.team_proto(team);
                let info = h.proto_info(proto);
                if info.execute_actions {
                    let (exists, _) = h.find_script(&info.production_condition);
                    if exists {
                        h.execute_action(&info.production_condition, team);
                    }
                }
            }
            i += 1;
        }
    }

    /// `doTeamBuilding()`: the team timer counts down to readiness; every 5 seconds the queues are updated and, when ready, a team
    /// chosen.
    pub fn do_team_building<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if self.skirmish_state.is_some() {
            return self.skirmish_do_team_building(h, data);
        }
        if !h.can_build_units(self.player) {
            return;
        }
        if !self.ready_to_build_team {
            self.team_timer -= 1;
            if self.team_timer <= 0 {
                self.ready_to_build_team = true;
                self.team_delay = 0;
            }
        }
        self.team_delay -= 1;
        if self.team_delay < 1 {
            self.queue_units(h, data);
            if self.ready_to_build_team {
                self.process_team_building(h, data);
            }
            self.team_delay = 5 * LOGICFRAMES_PER_SECOND; // "check again in 5 seconds."
        }
    }

    /// `doUpgradesAndSkills()`: from frame 2, with science points, the side's skill set (chosen once: the solo AI the first, the
    /// skirmish AI a random one of those with skills) buys every science it can.
    pub fn do_upgrades_and_skills<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if h.frame() < 2 {
            return;
        }
        let p = self.player;
        if h.science_purchase_points(p) <= 0 {
            return;
        }
        let side = h.side(p);
        let Some(side_info) = data.lists.side_info.iter().find(|s| s.side == side) else { return };
        if self.skillset_selector == INVALID_SKILLSET_SELECTION {
            let sets = &side_info.skill_sets;
            let mut limit = 0;
            if sets[1].num_skills > 0 {
                limit = 1;
                if sets[2].num_skills > 0 {
                    limit = 2;
                    if sets[3].num_skills > 0 {
                        limit = 3;
                        if sets[4].num_skills > 0 {
                            limit = 4;
                        }
                    }
                }
            }
            self.skillset_selector = if self.skirmish { h.logic_random_value(0, limit) } else { 0 };
        }
        if h.science_purchase_points(p) > 0 {
            let which = match self.skillset_selector {
                1 => 1,
                2 => 2,
                3 => 3,
                4 => 4,
                _ => 0,
            };
            let set = side_info.skill_sets[which].clone();
            for i in 0..set.num_skills.max(0) as usize {
                let science = set.skills[i];
                if h.is_capable_of_purchasing_science(p, science) && h.attempt_to_purchase_science(p, science) {
                    let mut m = h.player_name(p);
                    m.extend_from_slice(b" purchases from SkillSet");
                    m.push(b'1'.wrapping_add(self.skillset_selector as u8));
                    m.push(b' ');
                    m.extend(h.science_internal_name(science));
                    m.extend_from_slice(b".");
                    h.append_debug_message(&m);
                }
            }
        }
    }

    /// `update()`: base building, the ready and queued teams, team building, upgrades and skills, bridge repairs.
    pub fn update<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        self.do_base_building(h, data);
        self.check_ready_teams(h);
        self.check_queued_teams(h);
        self.do_team_building(h, data);
        self.do_upgrades_and_skills(h, data);
        self.update_bridge_repair(h, data);
    }

    /// `newMap()`: the player's producing objects join its build list, the base's centre and radius are found, and the entries
    /// built initially are built now (the others get a rebuild for the build that will consume one).
    pub fn new_map<H: AiPlayerHost>(&mut self, h: &mut H, data: &AiDataEntry) {
        if self.skirmish_state.is_some() {
            return self.skirmish_new_map(h, data);
        }
        let p = self.player;
        // (the list's head as it was: `addToBuildList` prepends, so the entries added here are not walked below)
        let before = h.build_list(p).len();
        for obj in h.objects() {
            if h.controlling_player(obj) == Some(p) && h.has_production(obj) {
                h.add_to_build_list(p, obj);
            }
        }
        let added = h.build_list(p).len() - before;
        let (c, r) = self.compute_center_and_radius_of_base(h);
        self.base_center = c;
        self.base_radius = r;
        let mut i = added;
        while i < h.build_list(p).len() {
            let idx = i;
            i += 1;
            let name = h.build_list(p)[idx].template_name.clone();
            if name.is_empty() {
                continue;
            }
            let Some(plan) = h.find_template(&name) else { continue };
            if h.build_list(p)[idx].is_initially_built {
                self.build_structure_now(h, data, plan, idx);
            } else {
                h.build_list(p)[idx].increment_num_rebuilds();
            }
        }
    }

    /// `computeCenterAndRadiusOfBase(&center, &radius)`: the mean position of the build list's known templates, and the farthest
    /// corner (each offset widened by 0.4 of its template's radius).
    pub fn compute_center_and_radius_of_base<H: AiPlayerHost>(&mut self, h: &mut H) -> (Coord3D, f32) {
        let p = self.player;
        let mut total = Coord2D { x: 0.0, y: 0.0 };
        let mut num = 0;
        let list = h.build_list(p).clone();
        let mut plans = Vec::new();
        for e in &list {
            if e.template_name.is_empty() {
                plans.push(None);
                continue;
            }
            let plan = h.find_template(&e.template_name);
            plans.push(plan);
            if plan.is_none() {
                continue;
            }
            total.x += e.location.x;
            total.y += e.location.y;
            num += 1;
        }
        if num > 0 {
            total.x /= num as f32;
            total.y /= num as f32;
        }
        self.base_center_set = num > 0;
        let center = Coord3D { x: total.x, y: total.y, z: 0.0 };
        let mut max_rad_sqr = 0.0f32;
        for (e, plan) in list.iter().zip(plans) {
            let Some(plan) = plan else { continue };
            let mut dx = (e.location.x - center.x).abs();
            let mut dy = (e.location.y - center.y).abs();
            let r = h.template_bounding_radius(plan) * 0.4;
            dx += r;
            dy += r;
            let rad_sqr = dx * dx + dy * dy;
            if rad_sqr > max_rad_sqr {
                max_rad_sqr = rad_sqr;
            }
        }
        (center, c_sqrt(max_rad_sqr))
    }

    /// `dozerInQueue()`: a queued team includes a dozer.
    pub fn dozer_in_queue<H: AiPlayerHost>(&self, h: &mut H) -> bool {
        self.build_queue.iter().any(|t| t.includes_a_dozer(h))
    }

    /// `queueDozer()`: unless one is queued, the first dozer template any factory (busy or not) can make is queued at the head as
    /// a priority team of the default team.
    pub fn queue_dozer<H: AiPlayerHost>(&mut self, h: &mut H) {
        if self.dozer_in_queue(h) {
            return;
        }
        let p = self.player;
        let can_build_units = h.can_build_units(p);
        h.set_can_build_units(p, true);
        let mut t = h.first_template();
        while let Some(tt) = t {
            if h.template_is_kind_of(tt, KINDOF_DOZER) {
                if let Some(factory) = Self::find_factory(h, p, tt, true) {
                    let order = WorkOrder { thing: Some(tt), factory_id: INVALID_ID, num_required: 1, required: true, is_resource_gatherer: false, ..WorkOrder::new() };
                    let team = TeamInQueue { work_orders: vec![order], priority_build: true, frame_started: h.frame() as i32, team: h.default_team(p), ..TeamInQueue::new() };
                    self.build_queue.insert(0, team);
                    let ft = h.object_template(factory);
                    let fname = h.template_name(ft);
                    h.append_debug_message(&concat(b"DOZER - building one at the ", &fname));
                    self.team_delay = 0;
                    let name = h.team_name(self.build_queue[0].team);
                    Self::start_training(h, p, &mut self.build_queue[0].work_orders[0], true, &name);
                    break;
                }
            }
            t = h.next_template(tt);
        }
        h.set_can_build_units(p, can_build_units);
    }

    /// `findDozer(pos)`: of the player's dozers (not a truck busy with supplies, not the repair dozer, not building), the idle one
    /// nearest the position, else the first; a dozer is queued when the player has none.
    pub fn find_dozer<H: AiPlayerHost>(&mut self, h: &mut H, _data: &AiDataEntry, pos: &Coord3D) -> Option<ObjectId> {
        let p = self.player;
        let mut dozer: Option<ObjectId> = None;
        let mut need_dozer = true;
        let mut closest: Option<ObjectId> = None;
        let mut closest_dist_sqr = 0.0f32;
        for obj in h.objects() {
            if h.controlling_player(obj) != Some(p) || !h.object_is_kind_of(obj, KINDOF_DOZER) {
                continue;
            }
            if !h.has_ai(obj) || !h.has_dozer_ai(obj) {
                continue;
            }
            let pending = h.dozer_any_task_pending(obj);
            if !pending && h.has_supply_truck_ai(obj) && (h.ferrying_supplies(obj) || h.forced_into_wanting_state(obj)) {
                continue;
            }
            if obj == self.repair_dozer {
                continue; // "don't steal the repair dozer."
            }
            need_dozer = false; // "dozer exists, may be busy."
            if h.dozer_build_task_pending(obj) {
                continue; // "already building."
            }
            if !pending {
                dozer = Some(obj); // "prefer an idle dozer"
            }
            if dozer.is_none() {
                dozer = Some(obj); // "but we'll take one doing stuff."
            }
            if let Some(d) = dozer {
                if !pending {
                    let dp = h.position(d);
                    let dx = pos.x - dp.x;
                    let dy = pos.y - dp.y;
                    let dist_sqr = dx * dx + dy * dy;
                    if closest.is_none() || dist_sqr < closest_dist_sqr {
                        closest = Some(d);
                        closest_dist_sqr = dist_sqr;
                    }
                }
            }
        }
        if need_dozer && !plant(3594) {
            self.queue_dozer(h);
        }
        closest.or(dozer)
    }

    /// `getPlayerStructureBounds(&bounds, playerNdx, conservative)`: the box of the player's structures (all zero for none).
    pub fn get_player_structure_bounds<H: AiPlayerHost>(h: &mut H, player_ndx: PlayerRef, conservative: bool) -> Region2D {
        let mut bounds = Region2D::default();
        let mut obj_bounds = Region2D::default();
        let mut first_object = true;
        let mut first_structure = true;
        if !h.player_exists(player_ndx) {
            return bounds;
        }
        for team in player_team_instances(h, player_ndx) {
            for o in h.team_members(team) {
                if !h.object_is_kind_of(o, KINDOF_STRUCTURE) {
                    continue;
                }
                if conservative && h.object_is_kind_of(o, KINDOF_CONSERVATIVE_BUILDING) {
                    continue;
                }
                let pos = h.position(o);
                if first_object {
                    obj_bounds.lo.x = pos.x;
                    obj_bounds.hi.x = pos.x;
                    obj_bounds.lo.y = pos.y;
                    obj_bounds.hi.y = pos.y;
                    first_object = false;
                } else {
                    if obj_bounds.lo.x > pos.x {
                        obj_bounds.lo.x = pos.x;
                    }
                    if obj_bounds.lo.y > pos.y {
                        obj_bounds.lo.y = pos.y;
                    }
                    if obj_bounds.hi.x < pos.x {
                        obj_bounds.hi.x = pos.x;
                    }
                    if obj_bounds.hi.y < pos.y {
                        obj_bounds.hi.y = pos.y;
                    }
                }
                if first_structure {
                    bounds.lo.x = pos.x;
                    bounds.hi.x = pos.x;
                    bounds.lo.y = pos.y;
                    bounds.hi.y = pos.y;
                    first_structure = false;
                } else {
                    if bounds.lo.x > pos.x {
                        bounds.lo.x = pos.x;
                    }
                    if bounds.lo.y > pos.y {
                        bounds.lo.y = pos.y;
                    }
                    if bounds.hi.x < pos.x {
                        bounds.hi.x = pos.x;
                    }
                    if bounds.hi.y < pos.y {
                        bounds.hi.y = pos.y;
                    }
                }
            }
        }
        if !first_structure {
            bounds = obj_bounds;
        }
        bounds
    }

    /// `xfer(xfer)` (version 1): the two queues (a load reads into empty queues, each team prepended and the queue reversed), the
    /// player's index (checked on load), the timers and flags, the difficulty, the base and the repairs.
    pub fn xfer<H: AiPlayerHost>(&mut self, h: &mut H, xfer: &mut dyn Xfer) -> XferResult<()> {
        if self.skirmish_state.is_some() {
            return self.skirmish_xfer(h, xfer);
        }
        self.base_xfer(h, xfer)
    }

    /// `AIPlayer::xfer`.
    pub(crate) fn base_xfer<H: AiPlayerHost>(&mut self, h: &mut H, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        for ready in [false, true] {
            let queue_len = if ready { self.ready_queue.len() } else { self.build_queue.len() };
            let mut count = queue_len as u16;
            xfer.xfer_unsigned_short(&mut count)?;
            if xfer.get_xfer_mode() == XferMode::Save {
                let q = if ready { &mut self.ready_queue } else { &mut self.build_queue };
                for t in q.iter_mut() {
                    t.xfer(h, xfer)?;
                }
            } else {
                if queue_len != 0 {
                    return Err(XferStatus::ErrorUnknown);
                }
                for _ in 0..count {
                    let mut t = TeamInQueue::new();
                    let r = t.xfer(h, xfer);
                    let q = if ready { &mut self.ready_queue } else { &mut self.build_queue };
                    q.insert(0, t);
                    r?;
                }
                let q = if ready { &mut self.ready_queue } else { &mut self.build_queue };
                q.reverse();
            }
        }
        let mut b = self.player.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        if i32::from_le_bytes(b) != self.player {
            return Err(XferStatus::ErrorUnknown);
        }
        xfer.xfer_bool(&mut self.ready_to_build_team)?;
        xfer.xfer_bool(&mut self.ready_to_build_structure)?;
        xfer.xfer_int(&mut self.team_timer)?;
        xfer.xfer_int(&mut self.structure_timer)?;
        xfer.xfer_int(&mut self.build_delay)?;
        xfer.xfer_int(&mut self.team_delay)?;
        xfer.xfer_int(&mut self.team_seconds)?;
        xfer.xfer_object_id(&mut self.cur_warehouse_id)?;
        xfer.xfer_int(&mut self.frame_last_building_built)?;
        let mut d = (self.difficulty as i32).to_le_bytes();
        xfer.xfer_user(&mut d)?;
        self.difficulty = match i32::from_le_bytes(d) {
            0 => GameDifficulty::Easy,
            1 => GameDifficulty::Normal,
            2 => GameDifficulty::Hard,
            // (a value no difficulty has: refused)
            _ => return Err(XferStatus::ErrorUnknown),
        };
        xfer.xfer_int(&mut self.skillset_selector)?;
        xfer.xfer_coord3d(&mut self.base_center)?;
        xfer.xfer_bool(&mut self.base_center_set)?;
        xfer.xfer_real(&mut self.base_radius)?;
        let mut r = [0u8; 4 * MAX_STRUCTURES_TO_REPAIR];
        for (i, id) in self.structures_to_repair.iter().enumerate() {
            r[4 * i..4 * i + 4].copy_from_slice(&id.to_le_bytes());
        }
        xfer.xfer_user(&mut r)?;
        for i in 0..MAX_STRUCTURES_TO_REPAIR {
            self.structures_to_repair[i] = ObjectId::from_le_bytes([r[4 * i], r[4 * i + 1], r[4 * i + 2], r[4 * i + 3]]);
        }
        xfer.xfer_object_id(&mut self.repair_dozer)?;
        xfer.xfer_int(&mut self.structures_in_queue)?;
        xfer.xfer_bool(&mut self.dozer_queued_for_repair)?;
        xfer.xfer_bool(&mut self.dozer_is_repairing)?;
        xfer.xfer_int(&mut self.bridge_timer)
    }
}
