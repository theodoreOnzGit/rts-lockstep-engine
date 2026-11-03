// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp (`addObjectToPathfindMap`, `removeObjectFromPathfindMap`:
//   `classifyObjectFootprint(obj, insert)`; the calls `Pathfinder` makes out of itself, answered by the world)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I9a: `TheAI->pathfinder()` in the world ([`World::pathfinder`]), and the world as its host
//! (`LayerHost`, `PathfinderHost`, `MovementHost`, `SearchHost`, `AdjustHost`, `AttackHost`, `AlliesHost`).
//!
//! **Live with `cuts.live_pathfinder`** (off by default, so every caller's cut point and table answers as before):
//! `addObjectToPathfindMap` and `removeObjectFromPathfindMap` (the modules', the object's creation and deletion, a new map
//! boundary), `removeUnitFromPathfindMap`, `updatePos` and `removePos` (the object's position and layer), `updateGoal`,
//! `addWallPiece` (a new game, a load) and `removeWallPiece` (a destroyed wall piece), `newMap` (a new game, a load),
//! `addBridge`, `changeBridgeState` and `forceMapRecalculation` (the terrain), and the contain modules' `adjustDestination`,
//! `adjustToPossibleDestination`, `snapPosition` and `getCell`.
//!
//! **Checked out while it runs.** The pathfinder is taken out of the world while it runs ([`World::with_pathfinder`]) and
//! asks the world about objects and the terrain. Upstream it can be re-entered from inside: `getLayerHeight` and
//! `getLayerForDestination` ask `TheAI->pathfinder()->isPointOnWall`, and `updateLayer`'s `obj->setLayer` calls `removePos`
//! and `updatePos`. The first is answered from a copy of the wall layer ([`WallView`], `World::pf_wall`, taken each time
//! the pathfinder is put back; the walls change only while the pathfinder runs a call of its own). A call that changes
//! the pathfinder arriving while it is out is **deferred**: queued, logged as `Pathfinder::deferred`, and run as soon as
//! the pathfinder is back. `updateLayer` calls `setLayer` last, so its `removePos`/`updatePos` run in upstream's order. The
//! terrain's bridge calls are deferred the same way until the terrain is back in the world (`changeBridgeState` asks
//! about the bridges).
//!
//! **Cut points** (tables in [`PathfinderCuts`]): the units' AI (`AIUpdateInterface`, batch I9b): whether it is doing
//! ground movement or adjusts an aircraft's destination, its pathfind goal and current cells (which upstream keeps on the
//! AI), the obstacle it ignores, its locomotor set (`cuts.ai_valid_surfaces`), whether it can path through units, is
//! idle (`cuts.ai_idle`), moving, attacking or busy (`cuts.ai_busy`), the building it does not path around and a wings
//! locomotor, and `aiMoveAwayFromUnit` (logged). Also the weapon's attack distance and range (`getAttackDistance` and
//! `isGoalPosWithinAttackRange` take the weapon's host mutably), the terrain line of fire, and the partition manager's
//! closest tall building (none). **Corrected 2026-11-03 (batch I43):** with `cuts.live_ai`, these are the units' own AIs'
//! answers: whether it can path through units, its ignored obstacle, whether it is moving, attacking or busy (`moveAllies`), the
//! building it does not path around (a chinook's drop target), a wings locomotor, and `aiMoveAwayFromUnit` is given to the
//! ally's AI (it was only logged). The attack search's questions are live since batch I38. ~~Still tables: the AI facts the
//! pathfinder reads before it runs (ground movement, an aircraft that adjusts its destination, and the goal and current cells,
//! which the pathfinder writes to `ai_cells`, not the AI's own `pathfind_goal_cell` and `pathfind_cur_cell`)~~ **Corrected
//! 2026-11-03 (batch I44):** those facts are the AI's own with `live_ai`, and the pathfinder writes the cells to the AI's saved
//! fields. ~~Still tables: the downhill-only flag of an object with no AI, the slaver id, and `attackUsesLineOfSight` (the AI
//! data's default).~~ **Corrected 2026-11-03 (batch I45):** the slaver id (the slaved update's, with `live_ai`),
//! `attackUsesLineOfSight` (the AI data's, with `live_ai`) and the closest tall building (the partition's, with
//! `live_partition`) are live. Still a table: the downhill-only flag of an object with no AI.
//!
//! `p1w13-sonnet`, integration batch I9a, 2026-11-01.

use crate::logic::weapon_fire::Weapon;
use std::collections::{BTreeMap, BTreeSet};

use crate::common::base_type::{Coord3D, ICoord2D, Region2D, Region3D};
use crate::common::disabled_types::DISABLED_UNMANNED;
use crate::common::game_common::{PlayerType, Relationship};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND};
use crate::common::geometry::GeometryType;
use crate::common::kind_of::*;
use crate::logic::ai::layer::{BridgeHandle, LayerHost, WallPieceFacts};
use crate::logic::ai::movement::MovementHost;
use crate::logic::ai::pathfinder::{PathfinderHost, PfAiFacts, PfObjectFacts, Pathfinder, GEOMETRY_BOX, GEOMETRY_CYLINDER, GEOMETRY_SPHERE};
use crate::logic::ai::queue::{AiActivity, AlliesHost};
use crate::logic::ai::search::{PfLocomotorSet, SearchHost};
use crate::logic::ai::adjust::AdjustHost;
use crate::logic::ai::attack::AttackHost;
use crate::logic::damage::{DamageInfo, DAMAGE_FALLING, HUGE_DAMAGE_AMOUNT};
use crate::logic::module::body_module::{BodyDamageType, BODY_PRISTINE, BODY_RUBBLE};
use crate::logic::terrain_logic::{Bridge, DEATH_SPLATTED};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleHandle;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::update_body::UpdateBody;

/// A call that changes the pathfinder (see the module docs: deferred while the pathfinder, or for the bridge calls the
/// terrain, is out of the world).
#[derive(Clone, Debug, PartialEq)]
pub enum PfOp {
    /// `addObjectToPathfindMap(obj)`.
    AddObject(ObjectId),
    /// `removeObjectFromPathfindMap(obj)`.
    RemoveObject(ObjectId),
    /// `removeUnitFromPathfindMap(obj)`.
    RemoveUnit(ObjectId),
    /// `updatePos(obj, pos)`.
    UpdatePos(ObjectId, Coord3D),
    /// `removePos(obj)`.
    RemovePos(ObjectId),
    /// `updateGoal(obj, pos, layer)`.
    UpdateGoal(ObjectId, Coord3D, PathfindLayer),
    /// `addWallPiece(obj)`, `removeWallPiece(obj)`.
    AddWallPiece(ObjectId),
    RemoveWallPiece(ObjectId),
    /// `newMap()`.
    NewMap,
    /// `changeBridgeState(layer, repaired)`, `forceMapRecalculation()`.
    ChangeBridgeState(PathfindLayer, bool),
    ForceMapRecalculation,
    /// Integration batch I37: `createAWallFromMyFootprint(obj)` (true) and `removeWallFromMyFootprint(obj)` (false), upstream's
    /// `internal_classifyObjectFootprint(obj, insert)` (AIPathfind.h), with none of `classifyObjectFootprint`'s filters.
    FootprintWall(ObjectId, bool),
}

/// The pathfinder's answers from the units the world does not hold yet (see the module docs), and its deferred calls.
#[derive(Clone, Debug)]
pub struct PathfinderCuts {
    /// The AI's `isDoingGroundMovement()`, `isAircraftThatAdjustsDestination()`, `canPathThroughUnits()`, `isMoving()`,
    /// `isAttacking()`, and a wings locomotor (`LOCO_WINGS`), by object (none listed: false).
    pub ai_ground_movement: BTreeSet<ObjectId>,
    pub ai_aircraft_adjusts_destination: BTreeSet<ObjectId>,
    pub ai_can_path_through_units: BTreeSet<ObjectId>,
    pub ai_moving: BTreeSet<ObjectId>,
    pub ai_attacking: BTreeSet<ObjectId>,
    pub ai_wings: BTreeSet<ObjectId>,
    /// The locomotor set's `isDownhillOnly()` (its valid surfaces: `cuts.ai_valid_surfaces`).
    pub ai_downhill_only: BTreeSet<ObjectId>,
    /// The AI's pathfind goal cell and current cell (`AIUpdateInterface`'s constructor: (-1, -1) both), by object.
    pub ai_cells: BTreeMap<ObjectId, (ICoord2D, ICoord2D)>,
    /// The AI's `getIgnoredObstacleID()` and `getBuildingToNotPathAround()` (none listed: `INVALID_ID`).
    pub ai_ignored_obstacle: BTreeMap<ObjectId, ObjectId>,
    pub ai_building_to_not_path_around: BTreeMap<ObjectId, ObjectId>,
    /// `TheAI->getAiData()->m_attackUsesLineOfSight` (`TAiData`'s default: true).
    pub attack_uses_line_of_sight: bool,
    /// The weapon's `getAttackDistance` and `isGoalPosWithinAttackRange`, and the terrain line of fire, by attacker
    /// (none listed: 0, false, no weapon).
    pub attack_distance: BTreeMap<ObjectId, f32>,
    pub goal_within_attack_range: BTreeSet<ObjectId>,
    pub terrain_line_clear: BTreeMap<ObjectId, bool>,
    /// Integration batch I38: while a search that asks a weapon runs (`findAttackPath`, `adjustTargetDestination`, with `live_ai`),
    /// what it asks of the weapon and of the units, computed before it (the pathfinder's host is read-only); the tables above
    /// answer otherwise.
    pub attack_search: Option<AttackSearch>,
    /// The calls waiting for the pathfinder (or the terrain) to be back in the world.
    pub deferred: Vec<PfOp>,
    /// The bridge calls waiting for the terrain.
    pub deferred_bridge: Vec<PfOp>,
}

/// Integration batch I38: what a pathfinder search passed a weapon asks of it, computed with the world before the search: the weapon's
/// two squared ranges for `isGoalPosWithinAttackRange` ([`Weapon::goal_range_sqrs`]), its `getAttackDistance(obj, victim, victimPos)`,
/// and the units whose AI's `isIdle()` is true (the search asks it of the units in its way).
#[derive(Clone, Debug, PartialEq)]
pub struct AttackSearch {
    /// The searching object (the weapon's owner).
    pub obj: ObjectId,
    pub range_sqrs: (f32, f32),
    pub attack_distance: f32,
    pub idle: BTreeSet<ObjectId>,
}

impl Default for PathfinderCuts {
    fn default() -> Self {
        PathfinderCuts {
            ai_ground_movement: BTreeSet::new(),
            ai_aircraft_adjusts_destination: BTreeSet::new(),
            ai_can_path_through_units: BTreeSet::new(),
            ai_moving: BTreeSet::new(),
            ai_attacking: BTreeSet::new(),
            ai_wings: BTreeSet::new(),
            ai_downhill_only: BTreeSet::new(),
            ai_cells: BTreeMap::new(),
            ai_ignored_obstacle: BTreeMap::new(),
            ai_building_to_not_path_around: BTreeMap::new(),
            attack_uses_line_of_sight: true,
            attack_distance: BTreeMap::new(),
            goal_within_attack_range: BTreeSet::new(),
            terrain_line_clear: BTreeMap::new(),
            attack_search: None,
            deferred: Vec::new(),
            deferred_bridge: Vec::new(),
        }
    }
}

/// `AIUpdateInterface`'s constructor's pathfind cells.
const NO_CELL: ICoord2D = ICoord2D { x: -1, y: -1 };

impl World {
    /// Runs `f` with the pathfinder checked out of the world; then puts it back, takes its [`WallView`], and runs the
    /// calls deferred meanwhile. `None` when the pathfinder is already out (a re-entry; recorded in `team_live_errors`).
    pub fn with_pathfinder<T>(&mut self, f: impl FnOnce(&mut Pathfinder, &mut World) -> T) -> Option<T> {
        let Some(mut pf) = self.pathfinder.take() else {
            self.cuts.team_live_errors.push("Pathfinder: re-entered while it runs".into());
            return None;
        };
        let r = f(&mut pf, self);
        self.pf_wall = pf.wall_view();
        self.pathfinder = Some(pf);
        self.pathfinder_run_deferred();
        Some(r)
    }

    /// The pathfinder as the running AI asks it (`TheAI->pathfinder()`, batch I9c-1): `f` gets the AI's object's facts, its
    /// locomotor set and position; none when the pathfinder is not ready ([`World::pf_ready`]) or the object is gone.
    pub(crate) fn ai_pf<T>(&mut self, f: impl FnOnce(&mut Pathfinder, &World, &PfObjectFacts, PfLocomotorSet) -> T) -> Option<T> {
        self.pf_ready()?;
        let id = self.ai_self;
        let loco = self.ai_locomotor_set(id);
        let obj = self.pf_object_facts(id)?;
        self.with_pathfinder(|pf, w| f(pf, w, &obj, loco))
    }

    /// [`World::ai_pf`] for a query that changes the world (`moveAllies`, `removeGoal`).
    pub(crate) fn ai_pf_mut<T>(&mut self, f: impl FnOnce(&mut Pathfinder, &mut World, &PfObjectFacts, PfLocomotorSet) -> T) -> Option<T> {
        self.pf_ready()?;
        let id = self.ai_self;
        let loco = self.ai_locomotor_set(id);
        let obj = self.pf_object_facts(id)?;
        self.with_pathfinder(|pf, w| f(pf, w, &obj, loco))
    }

    /// `TheAI->update()` (batch I9c-1): `m_pathfinder->processPathfindQueue()`, then `ThePlayerList->UPDATE()`. The queue runs
    /// in its three parts ([`Pathfinder::process_pathfind_queue_begin`] and the rest), with the pathfinder back in the World
    /// while each queued unit's AI pathfinds (`doPathfind`, which asks the pathfinder for its path), as upstream's AI reaches
    /// `TheAI->pathfinder()` from inside the queue. The pathfinder runs only when it is ready ([`World::pf_ready`]).
    pub fn ai_update(&mut self) {
        if self.pf_ready().is_some() {
            let frame = self.frame();
            if self.with_pathfinder(|pf, w| pf.process_pathfind_queue_begin(w, frame)) == Some(true) {
                while let Some(Some((id, go))) = self.with_pathfinder(|pf, w| pf.process_pathfind_queue_next(w)) {
                    if go {
                        if let Some(index) = self.find_object(id).and_then(|o| o.ai) {
                            self.module_call(id, index, crate::logic::world::module_call::ModuleCall::AiDoPathfind);
                        }
                    }
                    self.with_pathfinder(|pf, _| pf.process_pathfind_queue_advance());
                }
            }
        }
        if let Err(e) = crate::logic::world::player_list::player_list_update(self) {
            self.cuts.team_live_errors.push(format!("ThePlayerList->update: {e:?}"));
        }
    }

    /// Runs the calls deferred while the pathfinder was out, oldest first (a call they defer in turn joins the queue).
    fn pathfinder_run_deferred(&mut self) {
        while self.pathfinder.is_some() && !self.cuts.pathfinder.deferred.is_empty() {
            let op = self.cuts.pathfinder.deferred.remove(0);
            self.pathfinder_op(op);
        }
    }

    /// Runs the terrain's bridge calls deferred while it was out (from `with_terrain`, once the terrain is back).
    pub(crate) fn pathfinder_run_deferred_bridge_calls(&mut self) {
        let ops = std::mem::take(&mut self.cuts.pathfinder.deferred_bridge);
        for op in ops {
            self.pathfinder_op(op);
        }
    }

    /// Integration batch I38: an [`AttackSearch`] for `obj` searching with `weapon` against `victim`: the weapon's ranges and attack
    /// distance (its bonuses on `obj`), and every other unit whose AI's (virtual) `isIdle()` is true.
    pub(crate) fn attack_search_for(&mut self, obj: ObjectId, weapon: &Weapon, victim: Option<ObjectId>) -> AttackSearch {
        let range_sqrs = weapon.goal_range_sqrs(self, obj);
        let attack_distance = weapon.get_attack_distance(self, obj, victim);
        let mut idle = BTreeSet::new();
        for id in self.objects.ids_in_order() {
            if id != obj && self.ai_ask(id, crate::logic::world::module_call::ModuleCall::AiIsIdle).is_some_and(|r| r.as_bool()) {
                idle.insert(id);
            }
        }
        AttackSearch { obj, range_sqrs, attack_distance, idle }
    }

    /// Runs `f` with `search` in place for the pathfinder's weapon questions (the one before it put back after).
    pub(crate) fn with_attack_search<T>(&mut self, search: AttackSearch, f: impl FnOnce(&mut World) -> T) -> T {
        let before = self.cuts.pathfinder.attack_search.replace(search);
        let r = f(self);
        self.cuts.pathfinder.attack_search = before;
        r
    }

    /// One call that changes the pathfinder, with `live_pathfinder`: run now, or deferred while the pathfinder is out.
    /// False without `live_pathfinder` (the caller's cut point answers).
    pub fn pathfinder_op(&mut self, op: PfOp) -> bool {
        if !self.cuts.live_pathfinder {
            return false;
        }
        if self.pathfinder.is_none() {
            self.cut(CutCall::Host { call: "Pathfinder::deferred", args: Vec::new(), text: format!("{op:?}").into_bytes() });
            self.cuts.pathfinder.deferred.push(op);
            return true;
        }
        let frame = self.frame();
        self.with_pathfinder(|pf, w| match op {
            PfOp::AddObject(id) => pf.classify_object_footprint(w, id, true, frame),
            PfOp::RemoveObject(id) => pf.classify_object_footprint(w, id, false, frame),
            PfOp::RemoveUnit(id) => pf.remove_unit_from_pathfind_map(w, id),
            PfOp::UpdatePos(id, pos) => pf.update_pos(w, id, &pos),
            PfOp::RemovePos(id) => pf.remove_pos(w, id),
            PfOp::UpdateGoal(id, pos, layer) => pf.update_goal(w, id, &pos, layer),
            PfOp::AddWallPiece(id) => pf.add_wall_piece(id),
            PfOp::RemoveWallPiece(id) => pf.remove_wall_piece(id),
            PfOp::NewMap => pf.new_map(w, frame),
            PfOp::ChangeBridgeState(layer, repaired) => pf.change_bridge_state(w, layer, repaired, frame),
            PfOp::ForceMapRecalculation => pf.force_map_recalculation(w),
            PfOp::FootprintWall(id, insert) => {
                if let Some(o) = w.object(id) {
                    pf.internal_classify_object_footprint(&o, insert, frame);
                }
            }
        });
        true
    }

    /// `TheAI->pathfinder()->addObjectToPathfindMap(obj)`: live, or the cut point.
    pub fn pathfinder_add_object_to_map(&mut self, id: ObjectId) {
        if !self.pathfinder_op(PfOp::AddObject(id)) {
            self.cut(CutCall::PathfinderAddObjectToPathfindMap { object: id });
        }
    }

    /// `TheAI->pathfinder()->removeObjectFromPathfindMap(obj)`: live, or the cut point.
    pub fn pathfinder_remove_object_from_map(&mut self, id: ObjectId) {
        if !self.pathfinder_op(PfOp::RemoveObject(id)) {
            self.cut(CutCall::PathfinderRemoveObjectFromPathfindMap { object: id });
        }
    }

    /// `TheAI->pathfinder()->updatePos(obj, obj->getPosition())`: live, or the cut point.
    pub fn pathfinder_update_pos(&mut self, id: ObjectId) {
        let pos = self.objects.find(id).map_or_else(Coord3D::default, |o| o.get_position());
        if !self.pathfinder_op(PfOp::UpdatePos(id, pos)) {
            self.cut(CutCall::PathfinderUpdatePos { object: id });
        }
    }

    /// `TheAI->pathfinder()->removePos(obj)`: live, or the cut point.
    pub fn pathfinder_remove_pos(&mut self, id: ObjectId) {
        if !self.pathfinder_op(PfOp::RemovePos(id)) {
            self.cut(CutCall::PathfinderRemovePos { object: id });
        }
    }

    /// `TheAI->pathfinder()->removeWallPiece(obj)`: live, or the cut point.
    pub fn pathfinder_remove_wall_piece(&mut self, id: ObjectId) {
        if !self.pathfinder_op(PfOp::RemoveWallPiece(id)) {
            self.cut(CutCall::PathfinderRemoveWallPiece { object: id });
        }
    }

    /// A bridge call from the terrain (deferred until the terrain is back; see the module docs). False without
    /// `live_pathfinder`.
    pub fn pathfinder_bridge_op(&mut self, op: PfOp) -> bool {
        if !self.cuts.live_pathfinder {
            return false;
        }
        self.cuts.pathfinder.deferred_bridge.push(op);
        true
    }

    /// A question to the pathfinder about object `id`: `f` with the pathfinder and the object's facts; `None` without
    /// `live_pathfinder`, for an object that does not exist, or while the pathfinder is out.
    pub(crate) fn pathfinder_ask<T>(&mut self, id: ObjectId, f: impl FnOnce(&mut Pathfinder, &World, &PfObjectFacts) -> T) -> Option<T> {
        if !self.cuts.live_pathfinder {
            return None;
        }
        let obj = self.pf_object_facts(id)?;
        self.with_pathfinder(|pf, w| f(pf, w, &obj))
    }

    /// `adjustDestination(obj, ai->getLocomotorSet(), &pos)`: `None` when the pathfinder does not answer (see
    /// [`World::pathfinder_ask`]).
    pub fn pathfinder_adjust_destination(&mut self, id: ObjectId, pos: &mut Coord3D) -> Option<bool> {
        let loco = self.ai_locomotor_set(id);
        let mut p = *pos;
        let r = self.pathfinder_ask(id, |pf, w, obj| pf.adjust_destination(w, obj, loco, &mut p, None))?;
        *pos = p;
        Some(r)
    }

    /// `adjustToPossibleDestination(obj, ai->getLocomotorSet(), &pos)`.
    pub fn pathfinder_adjust_to_possible_destination(&mut self, id: ObjectId, pos: &mut Coord3D) -> Option<bool> {
        let loco = self.ai_locomotor_set(id);
        let mut p = *pos;
        let r = self.pathfinder_ask(id, |pf, w, obj| pf.adjust_to_possible_destination(w, obj, loco, &mut p))?;
        *pos = p;
        Some(r)
    }

    /// `snapPosition(obj, &pos)`.
    pub fn pathfinder_snap_position(&mut self, id: ObjectId, pos: &mut Coord3D) -> Option<()> {
        let mut p = *pos;
        self.pathfinder_ask(id, |pf, w, obj| pf.snap_position(w, Some(obj), &mut p))?;
        *pos = p;
        Some(())
    }

    /// The world's pathfinder when it answers: with `live_pathfinder`, in the world (not running) and with its map made
    /// (`newMap`; upstream every unit's AI runs after it). `None` otherwise: the callers answer as their stand-ins did.
    pub fn pf_ready(&self) -> Option<&Pathfinder> {
        self.pathfinder.as_ref().filter(|pf| self.cuts.live_pathfinder && pf.is_map_ready)
    }

    /// `getCell(layer, x, y)`, then `getType()`: `Some(None)` for a `NULL` cell; `None` when the pathfinder does not answer
    /// ([`World::pf_ready`]).
    pub fn pathfinder_cell_type(&self, layer: PathfindLayer, x: i32, y: i32) -> Option<Option<i32>> {
        let pf = self.pf_ready()?;
        Some(pf.get_cell(layer, x, y).map(|c| i32::from(pf.store.cells[c as usize].get_type())))
    }

    /// The body's `getDamageState()`, read in place (a body that is out, or none: pristine; a probe: its answer).
    pub(crate) fn peek_damage_state(&self, id: ObjectId) -> BodyDamageType {
        let Some(index) = self.objects.find(id).and_then(|o| o.body) else { return BODY_PRISTINE };
        match self.peek_module_body(ModuleHandle { object: id, index }) {
            Some(UpdateBody::ActiveBody(b)) => b.get_damage_state(),
            Some(UpdateBody::HighlanderBody(b)) => b.active_body().get_damage_state(),
            Some(UpdateBody::ImmortalBody(b)) => b.active_body().get_damage_state(),
            Some(UpdateBody::StructureBody(b)) => b.active_body().get_damage_state(),
            Some(UpdateBody::UndeadBody(b)) => b.active_body().get_damage_state(),
            Some(UpdateBody::HiveStructureBody(b)) => b.active_body().get_damage_state(),
            Some(UpdateBody::ModuleProbe(p)) => p.answers.damage_state.unwrap_or(BODY_PRISTINE),
            _ => BODY_PRISTINE,
        }
    }

    /// The body's `getLastDamageInfo()`, read in place (`None`: no active body there, or it is out).
    pub(crate) fn peek_last_damage(&self, id: ObjectId) -> Option<&DamageInfo> {
        let index = self.objects.find(id).and_then(|o| o.body)?;
        match self.peek_module_body(ModuleHandle { object: id, index })? {
            UpdateBody::ActiveBody(b) => Some(b.get_last_damage_info()),
            UpdateBody::HighlanderBody(b) => Some(b.active_body().get_last_damage_info()),
            UpdateBody::ImmortalBody(b) => Some(b.active_body().get_last_damage_info()),
            UpdateBody::StructureBody(b) => Some(b.active_body().get_last_damage_info()),
            UpdateBody::UndeadBody(b) => Some(b.active_body().get_last_damage_info()),
            UpdateBody::HiveStructureBody(b) => Some(b.active_body().get_last_damage_info()),
            _ => None,
        }
    }

    /// What the pathfinder reads of object `id`.
    pub fn pf_object_facts(&self, id: ObjectId) -> Option<PfObjectFacts> {
        let o = self.objects.find(id)?;
        let g = &o.geometry_info;
        let t = o.template();
        // (batch I44: with `live_ai`, the AI's own: `isDoingGroundMovement()`, `isAircraftThatAdjustsDestination()` and its
        // pathfind goal and current cells, the fields it saves; they were the tables, so no unit did ground movement)
        let live = if self.cuts.live_ai && o.ai.is_some() {
            self.ai_peek(id).map(|a| {
                let unmanned_heli = o.is_disabled_by_type(DISABLED_UNMANNED) && o.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD);
                let held = o.is_disabled_by_type(crate::common::disabled_types::DISABLED_HELD);
                let falling = self.peek_height_above_terrain(id) > 0.0
                    && o.physics.is_some_and(|p| matches!(self.peek_module_body(ModuleHandle { object: id, index: p }), Some(UpdateBody::PhysicsBehavior(b)) if b.get_allow_to_fall()));
                PfAiFacts {
                    doing_ground_movement: a.is_doing_ground_movement_given(unmanned_heli, held, falling),
                    aircraft_adjusts_destination: a.is_aircraft_that_adjusts_destination(),
                    goal_cell: a.pathfind_goal_cell,
                    cur_cell: a.pathfind_cur_cell,
                }
            })
        } else {
            None
        };
        let ai = o.ai.map(|_| {
            if let Some(f) = live {
                return f;
            }
            let (goal_cell, cur_cell) = self.cuts.pathfinder.ai_cells.get(&id).copied().unwrap_or((NO_CELL, NO_CELL));
            PfAiFacts {
                doing_ground_movement: self.cuts.pathfinder.ai_ground_movement.contains(&id),
                aircraft_adjusts_destination: self.cuts.pathfinder.ai_aircraft_adjusts_destination.contains(&id),
                goal_cell,
                cur_cell,
            }
        });
        Some(PfObjectFacts {
            id,
            pos: o.get_position(),
            orientation: o.thing.orientation(),
            layer: o.layer,
            destination_layer: o.destination_layer,
            mine: o.is_kind_of(KINDOF_MINE),
            projectile: o.is_kind_of(KINDOF_PROJECTILE),
            bridge_tower: o.is_kind_of(KINDOF_BRIDGE_TOWER),
            defensive_wall: o.is_kind_of(KINDOF_DEFENSIVE_WALL),
            blast_crater: o.is_kind_of(KINDOF_BLAST_CRATER),
            walk_on_top_of_wall: o.is_kind_of(KINDOF_WALK_ON_TOP_OF_WALL),
            structure: o.is_kind_of(KINDOF_STRUCTURE),
            immobile: o.is_kind_of(KINDOF_IMMOBILE),
            produced_at_helipad: o.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD),
            can_see_through_structure: o.is_kind_of(KINDOF_CAN_SEE_THROUGH_STRUCTURE),
            aircraft_path_around: o.is_kind_of(KINDOF_AIRCRAFT_PATH_AROUND),
            attack_needs_line_of_sight: o.is_kind_of(KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT),
            aircraft: o.is_kind_of(KINDOF_AIRCRAFT),
            fs_airfield: o.is_kind_of(KINDOF_FS_AIRFIELD),
            unmanned: o.is_disabled_by_type(DISABLED_UNMANNED),
            rubble: self.peek_damage_state(id) == BODY_RUBBLE,
            fence_width: t.get_fence_width(),
            fence_x_offset: t.get_fence_x_offset(),
            mobile: self.is_mobile(id),
            geom_type: match g.get_geom_type() {
                GeometryType::Sphere => GEOMETRY_SPHERE,
                GeometryType::Cylinder => GEOMETRY_CYLINDER,
                GeometryType::Box => GEOMETRY_BOX,
            },
            major_radius: g.get_major_radius(),
            minor_radius: g.get_minor_radius(),
            small: g.get_is_small(),
            bounding_circle_radius: g.get_bounding_circle_radius(),
            height_above_terrain: self.peek_height_above_terrain(id),
            ai,
        })
    }

    /// The bridge whose object is `h` (the handle the world gives the pathfinder: upstream's `Bridge*`, kept as the
    /// bridge object's id because the terrain inserts bridges at the front of its list).
    fn pf_bridge(&self, h: BridgeHandle) -> Option<&Bridge> {
        self.terrain.bridges().iter().find(|b| b.info().bridge_object_id as BridgeHandle == h)
    }

    /// `getAIUpdateInterface()->getLocomotorSet()`: its valid surfaces (`cuts.ai_valid_surfaces`) and `isDownhillOnly()`.
    pub(crate) fn ai_locomotor_set(&self, id: ObjectId) -> PfLocomotorSet {
        // (batch I9c-1: the live AI's `getLocomotorSet()`; the cut tables for an object without one)
        if let Some(a) = self.ai_peek(id) {
            return PfLocomotorSet { valid_surfaces: a.locomotor_set.get_valid_surfaces() as _, downhill_only: a.locomotor_set.is_downhill_only() };
        }
        PfLocomotorSet {
            valid_surfaces: self.cuts.ai_valid_surfaces.get(&id).copied().unwrap_or(0) as _,
            downhill_only: self.cuts.pathfinder.ai_downhill_only.contains(&id),
        }
    }

    /// A pathfind cell for a unit whose AI is neither in its module nor lent (a call of its module is running with the AI
    /// unlent): reported, and kept in the table (batch I44). An object with no AI is not reported.
    fn pf_cell_out_of_reach(&mut self, id: ObjectId) {
        if self.objects.find(id).is_some_and(|o| o.ai.is_some()) {
            self.cuts.team_live_errors.push(format!("Pathfinder: the pathfind cell of {id}'s AI, which is out of reach"));
        }
    }

    fn pf_ai_cells_mut(&mut self, id: ObjectId) -> &mut (ICoord2D, ICoord2D) {
        self.cuts.pathfinder.ai_cells.entry(id).or_insert((NO_CELL, NO_CELL))
    }
}

impl LayerHost for World {
    fn bridge_bounds(&self, bridge: BridgeHandle) -> Region2D {
        self.pf_bridge(bridge).map(|b| *b.bounds()).unwrap_or_default()
    }
    fn bridge_ends(&self, bridge: BridgeHandle) -> (Coord3D, Coord3D) {
        self.pf_bridge(bridge).map(|b| (b.info().from, b.info().to)).unwrap_or_default()
    }
    fn bridge_object_id(&self, bridge: BridgeHandle) -> ObjectId {
        self.pf_bridge(bridge).map_or(INVALID_ID, |b| b.info().bridge_object_id)
    }
    fn is_point_on_bridge(&self, bridge: BridgeHandle, p: &Coord3D) -> bool {
        self.pf_bridge(bridge).is_some_and(|b| b.is_point_on_bridge(p))
    }
    fn is_cell_on_end(&self, bridge: BridgeHandle, cell: &Region2D) -> bool {
        self.pf_bridge(bridge).is_some_and(|b| b.is_cell_on_end(cell))
    }
    fn is_cell_on_side(&self, bridge: BridgeHandle, cell: &Region2D) -> bool {
        self.pf_bridge(bridge).is_some_and(|b| b.is_cell_on_side(cell))
    }
    fn is_cell_entry_point(&self, bridge: BridgeHandle, cell: &Region2D) -> bool {
        self.pf_bridge(bridge).is_some_and(|b| b.is_cell_entry_point(cell))
    }
    /// A vertical deck (an undefined input upstream) answers the ground height, as the world's `layer_height` does.
    fn bridge_height(&self, bridge: BridgeHandle, p: &Coord3D) -> f32 {
        match self.pf_bridge(bridge).map(|b| b.bridge_height(p)) {
            Some(Ok((h, _))) => h,
            _ => self.terrain.ground_height(p.x, p.y, false).0,
        }
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        self.terrain.ground_height(x, y, false).0
    }
    fn wall_piece(&self, id: ObjectId) -> Option<WallPieceFacts> {
        let o = self.objects.find(id)?;
        let (pos, orientation, g) = (o.get_position(), o.thing.orientation(), &o.geometry_info);
        Some(WallPieceFacts {
            pos,
            orientation,
            bounds_2d: g.get_2d_bounds(&pos, orientation),
            major_radius: g.get_major_radius(),
            minor_radius: g.get_minor_radius(),
            is_sphere: g.get_geom_type() == GeometryType::Sphere,
        })
    }
}

impl PathfinderHost for World {
    fn object(&self, id: ObjectId) -> Option<PfObjectFacts> {
        self.pf_object_facts(id)
    }
    fn objects(&self) -> Vec<ObjectId> {
        self.objects.ids_in_order()
    }
    fn set_layer(&mut self, id: ObjectId, layer: PathfindLayer) {
        World::set_layer(self, id, layer);
    }
    fn set_destination_layer(&mut self, id: ObjectId, layer: PathfindLayer) {
        World::set_destination_layer(self, id, layer);
    }
    fn set_pathfind_goal_cell(&mut self, id: ObjectId, cell: ICoord2D) {
        // (batch I44: the AI's own `setPathfindGoalCell` with `live_ai`; it was the table, so the AI's saved field, which the
        // collision code reads, stayed (-1, -1))
        if self.cuts.live_ai {
            if let Some(a) = self.ai_peek_mut(id) {
                a.pathfind_goal_cell = cell;
                return;
            }
            self.pf_cell_out_of_reach(id);
        }
        self.pf_ai_cells_mut(id).0 = cell;
    }
    fn set_cur_pathfind_cell(&mut self, id: ObjectId, cell: ICoord2D) {
        // (batch I44: the AI's own `setCurPathfindCell`, as the goal cell)
        if self.cuts.live_ai {
            if let Some(a) = self.ai_peek_mut(id) {
                a.pathfind_cur_cell = cell;
                return;
            }
            self.pf_cell_out_of_reach(id);
        }
        self.pf_ai_cells_mut(id).1 = cell;
    }
    fn splat(&mut self, id: ObjectId) {
        let mut info = DamageInfo::default();
        info.input.damage_type = DAMAGE_FALLING;
        info.input.death_type = DEATH_SPLATTED;
        info.input.source_id = id;
        info.input.amount = HUGE_DAMAGE_AMOUNT;
        self.attempt_damage(id, &mut info);
    }
    /// An undefined input (the terrain's) answers the cliff table, as the partition manager's host does.
    fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        match self.terrain.is_cliff_cell(x, y) {
            Ok(cliff) => cliff,
            Err(_) => self.cuts.terrain_cliff[crate::logic::world::object_cuts::terrain_cell(x, y)],
        }
    }
    fn is_underwater(&self, x: f32, y: f32) -> bool {
        self.terrain.is_underwater_uncounted(x, y).is_ok_and(|u| u.underwater)
    }
    fn maximum_pathfind_extent(&self) -> Region3D {
        self.terrain.maximum_pathfind_extent().unwrap_or_default()
    }
    fn object_interacts_with_bridge_layer(&self, id: ObjectId, layer: PathfindLayer) -> bool {
        let Some(v) = crate::logic::world::terrain_host_world::object_view(self, id) else { return false };
        self.terrain.object_interacts_with_bridge_layer_uncounted(self, &v, layer, false).unwrap_or(false)
    }
    fn object_interacts_with_bridge_end(&self, id: ObjectId, layer: PathfindLayer) -> bool {
        let Some(v) = crate::logic::world::terrain_host_world::object_view(self, id) else { return false };
        self.terrain.object_interacts_with_bridge_end_uncounted(&v, layer).unwrap_or(false)
    }
    fn wall_height(&self) -> f32 {
        self.cuts.wall_height
    }
}

impl MovementHost for World {
    fn ignored_obstacle_id(&self, id: ObjectId) -> ObjectId {
        // (batch I43: the AI's own `getIgnoredObstacleID()` with `live_ai`; it was always the table)
        if self.cuts.live_ai {
            return self.ai_peek(id).map_or(INVALID_ID, |a| a.get_ignored_obstacle_id());
        }
        self.cuts.pathfinder.ai_ignored_obstacle.get(&id).copied().unwrap_or(INVALID_ID)
    }
    fn are_allies(&self, obj: ObjectId, unit: ObjectId) -> bool {
        self.get_relationship(obj, unit) == Relationship::Allies
    }
    fn can_crush_or_squish(&self, obj: ObjectId, unit: ObjectId) -> bool {
        World::can_crush_or_squish(self, obj, Some(unit), crate::logic::world::object_damage::CrushSquishTestType::CrushOrSquish)
    }
    fn crusher_level(&self, id: ObjectId) -> i32 {
        i32::from(self.get_crusher_level(id))
    }
    fn crushable_level(&self, id: ObjectId) -> i32 {
        i32::from(self.get_crushable_level(id))
    }
    fn layer_height(&self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        crate::logic::world::object_transform::layer_height(&self.terrain, self.cuts.wall_height, &self.pf_wall, x, y, layer)
    }
    fn layer_for_destination(&self, pos: &Coord3D) -> PathfindLayer {
        self.terrain.layer_for_destination_uncounted(self, pos).unwrap_or(LAYER_GROUND)
    }
    fn is_significantly_above_terrain(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some() && self.peek_height_above_terrain(id) > (-(3 * 3)) as f32 * self.cuts.globals.gravity
    }
    fn attack_uses_line_of_sight(&self) -> bool {
        // (batch I45: `TheAI->getAiData()->m_attackUsesLineOfSight` with `live_ai`, as the AI reads it; it was always the table)
        if self.cuts.live_ai {
            return self.statics.ai_data.data.attack_uses_line_of_sight;
        }
        self.cuts.pathfinder.attack_uses_line_of_sight
    }
    fn weapon_terrain_line_clear(&self, attacker: ObjectId, attacker_pos: &Coord3D, victim: Option<ObjectId>, victim_pos: &Coord3D) -> Option<bool> {
        // (integration batch I38: with `live_ai`, the attacker's current weapon's `isClearGoalFiringLineOfSightTerrain`; it was the
        // table, empty in a game, so the terrain never blocked an attack view)
        if self.cuts.live_ai {
            let w = self.objects.find(attacker)?.weapon_set.get_cur_weapon()?;
            return Some(match victim {
                Some(v) => w.is_clear_goal_firing_line_of_sight_terrain(self, attacker, attacker_pos, v),
                None => w.is_clear_goal_firing_line_of_sight_terrain_pos(self, attacker, attacker_pos, victim_pos),
            });
        }
        self.cuts.pathfinder.terrain_line_clear.get(&attacker).copied()
    }
    /// `getSlaverID(o)` (`AIPathfind.cpp:9209`): the first behavior module with a slaved update interface answers (`SlavedUpdate`,
    /// `MobMemberSlavedUpdate`); none without one. ~~None until the slaved updates answer it (batch I9b).~~ **Corrected 2026-11-03
    /// (batch I45):** it was always none, so the pathfinder never treated a slave's slaver (a stinger site, a mob's nexus) as
    /// transparent. Live with `live_ai`; a module body out of its slot (its object updating) is not seen, and answers none.
    fn slaver_id(&self, id: ObjectId) -> ObjectId {
        if !self.cuts.live_ai {
            return INVALID_ID;
        }
        for index in self.all_modules(id) {
            use crate::logic::world::update_body::UpdateBody as B;
            match self.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: id, index }) {
                Some(B::SlavedUpdate(m)) => return m.slaver_id(),
                Some(B::MobMemberSlavedUpdate(m)) => return m.slaver_id(),
                _ => {}
            }
        }
        INVALID_ID
    }
    fn container_id(&self, id: ObjectId) -> ObjectId {
        self.objects.find(id).map_or(INVALID_ID, |o| o.contained_by)
    }
    fn is_infantry(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_INFANTRY))
    }
    /// `ThePartitionManager->getClosestObject(pos, radius, FROM_BOUNDINGSPHERE_2D, { AcceptByKindOf(AIRCRAFT_PATH_AROUND) })`
    /// (`circleClipsTallBuilding`, `AIPathfind.cpp:9548`): the world's partition manager with `live_partition`. ~~None (a cut point;
    /// batch I9b, with the aircraft's paths).~~ **Corrected 2026-11-03 (batch I45):** it was always none, so an aircraft's
    /// destination was never moved beside a tall building; none still without `live_partition`.
    fn closest_tall_building(&mut self, pos: &Coord3D, radius: f32) -> Option<ObjectId> {
        use crate::common::kind_of::{KindOfMaskType, KINDOF_AIRCRAFT_PATH_AROUND};
        use crate::logic::world::partition::filters::PartitionFilter as F;
        use crate::logic::world::partition::DistanceCalculationType;
        if !self.cuts.live_partition {
            return None;
        }
        let filters = [F::AcceptByKindOf { must_be_set: KindOfMaskType::from_indices(&[KINDOF_AIRCRAFT_PATH_AROUND]), must_be_clear: KindOfMaskType::new() }];
        let found = self.with_partition(|pm, w| pm.get_closest_object_at(w, pos, radius, DistanceCalculationType::FromBoundingSphere2D, &filters));
        found.object
    }
}

impl SearchHost for World {
    fn is_human(&self, id: ObjectId) -> bool {
        match self.get_controlling_player(id).and_then(|p| self.players.get_nth(p)) {
            Some(p) => p.player_type != PlayerType::Computer,
            None => true,
        }
    }
    fn can_path_through_units(&self, id: ObjectId) -> bool {
        // (batch I43: the AI's own `canPathThroughUnits()` with `live_ai`; it was always the table)
        if self.cuts.live_ai {
            return self.ai_peek(id).is_some_and(|a| a.can_path_through_units);
        }
        self.cuts.pathfinder.ai_can_path_through_units.contains(&id)
    }
    fn is_dozer(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_DOZER))
    }
    fn is_vehicle(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_VEHICLE))
    }
    fn are_enemies(&self, obj: ObjectId, other: ObjectId) -> bool {
        self.get_relationship(obj, other) == Relationship::Enemies
    }
    fn ground_height_at(&self, x: f32, y: f32) -> f32 {
        self.terrain.ground_height(x, y, false).0
    }
}

impl AdjustHost for World {
    fn goal_pos_within_attack_range(&self, obj: ObjectId, goal: &Coord3D, victim: Option<ObjectId>, victim_pos: &Coord3D) -> bool {
        // (integration batch I38: the searching weapon's test, with the ranges computed before the search)
        match &self.cuts.pathfinder.attack_search {
            Some(s) if s.obj == obj => Weapon::goal_pos_within_range_sqrs(self, obj, goal, victim, victim_pos, s.range_sqrs),
            _ => self.cuts.pathfinder.goal_within_attack_range.contains(&obj),
        }
    }
}

impl AttackHost for World {
    fn attack_distance(&self, obj: ObjectId, _victim: Option<ObjectId>, _victim_pos: &Coord3D) -> f32 {
        match &self.cuts.pathfinder.attack_search {
            Some(s) if s.obj == obj => s.attack_distance,
            _ => self.cuts.pathfinder.attack_distance.get(&obj).copied().unwrap_or(0.0),
        }
    }
    fn is_idle(&self, id: ObjectId) -> bool {
        if let Some(s) = &self.cuts.pathfinder.attack_search {
            return s.idle.contains(&id);
        }
        self.objects.find(id).is_some_and(|o| o.ai.is_some()) && self.cuts.ai_idle.contains(&id)
    }
    fn ai_locomotor_set(&self, id: ObjectId) -> PfLocomotorSet {
        World::ai_locomotor_set(self, id)
    }
}

impl AlliesHost for World {
    fn is_harvester(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_HARVESTER))
    }
    fn ai_activity(&mut self, id: ObjectId) -> Option<AiActivity> {
        let o = self.objects.find(id)?;
        o.ai?;
        let using_ability = o.test_status(crate::common::object_status_types::OBJECT_STATUS_IS_USING_ABILITY);
        // (batch I43: the AI's own `isMoving()`, `isAttacking()` and `isBusy()` with `live_ai`; they were always the tables, so no
        // ally was ever moving, attacking or busy, and `moveAllies` asked every ally in the way to move)
        if self.cuts.live_ai {
            use crate::logic::world::module_call::ModuleCall;
            let ask = |w: &mut World, call| w.ai_ask(id, call).is_some_and(|r| r.as_bool());
            let (moving, attacking, busy) = (ask(self, ModuleCall::AiIsMoving), ask(self, ModuleCall::AiIsAttacking), ask(self, ModuleCall::AiIsBusy));
            return Some(AiActivity { moving, attacking, busy, using_ability });
        }
        let o = self.objects.find(id)?;
        let c = &self.cuts.pathfinder;
        Some(AiActivity {
            moving: c.ai_moving.contains(&id),
            attacking: c.ai_attacking.contains(&id),
            busy: self.cuts.ai_busy.contains(&id),
            using_ability: o.test_status(crate::common::object_status_types::OBJECT_STATUS_IS_USING_ABILITY),
        })
    }
    fn ai_move_away_from_unit(&mut self, id: ObjectId, from: ObjectId) {
        // (batch I43: given to the ally's AI with `live_ai`, `aiMoveAwayFromUnit(from, CMD_FROM_AI)`, as the collision's; it was
        // only logged, so no ally in the way ever moved)
        if self.cuts.live_ai {
            use crate::logic::ai::ai_commands::{AiCommandParms, AICMD_MOVE_AWAY_FROM_UNIT};
            let mut parms = AiCommandParms::new(AICMD_MOVE_AWAY_FROM_UNIT, crate::logic::ai::ai_update_types::CMD_FROM_AI);
            parms.obj = from;
            // (the ally is never the AI that runs: `moveAllies` skips the mover itself)
            self.ai_ask(id, crate::logic::world::module_call::ModuleCall::AiDoCommandParms(parms));
            return;
        }
        self.cut(CutCall::Host { call: "AIUpdateInterface::aiMoveAwayFromUnit", args: vec![i64::from(id), i64::from(from)], text: Vec::new() });
    }
    fn building_to_not_path_around(&self, id: ObjectId) -> ObjectId {
        // (batch I43: with `live_ai`, the AI's own: `INVALID_ID` but for a chinook moving to or doing a combat drop, its goal object,
        // `chinook_ai_update::building_to_not_path_around`; it was always the table)
        if self.cuts.live_ai {
            use crate::logic::ai::chinook_ai_update::{DO_COMBAT_DROP, MOVE_TO_COMBAT_DROP};
            let (Some(a), Some(m)) = (self.ai_peek(id), self.ai_machine_query(id)) else { return INVALID_ID };
            if !matches!(a.subclass, crate::logic::ai::ai_update::AiSubclass::Chinook(_)) {
                return INVALID_ID;
            }
            let state = m.get_current_state_id();
            if state == MOVE_TO_COMBAT_DROP || state == DO_COMBAT_DROP {
                return self.find_object(m.goal_object_id()).map_or(INVALID_ID, |_| m.goal_object_id());
            }
            return INVALID_ID;
        }
        self.cuts.pathfinder.ai_building_to_not_path_around.get(&id).copied().unwrap_or(INVALID_ID)
    }
    fn has_wings_locomotor(&self, id: ObjectId) -> bool {
        // (batch I43: the AI's current locomotor's appearance with `live_ai`; it was always the table)
        if self.cuts.live_ai {
            return self.ai_peek(id).and_then(|a| a.get_cur_locomotor()).is_some_and(|l| l.get_appearance() == crate::logic::locomotor::LocomotorAppearance::LocoWings);
        }
        self.cuts.pathfinder.ai_wings.contains(&id)
    }
    fn terrain_extent(&self) -> Region3D {
        self.terrain.extent().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    //! The pathfinder itself is checked against the compiled AIPathfind.cpp by the `differential_pathfinder_*` tests with
    //! stand-in hosts; these check it in the world, answered by the world's objects and terrain.
    use super::*;
    use crate::common::object_status_types::ObjectStatusMaskType;
    use crate::logic::ai::cell::{CELL_CLEAR, CELL_OBSTACLE};
    use crate::logic::world::player_host_world::tests::world;

    fn make(w: &mut World, name: &[u8], x: f32, y: f32) -> ObjectId {
        let t = World::thing_factory_find_template(w, name).unwrap();
        let id = World::thing_factory_new_object(w, t, None, ObjectStatusMaskType::default()).unwrap();
        w.set_position(id, &Coord3D { x, y, z: 0.0 });
        id
    }

    fn cell(w: &World, x: i32, y: i32) -> i32 {
        w.pathfinder_cell_type(LAYER_GROUND, x, y).expect("live").expect("a cell")
    }

    /// A depot (an immobile structure, a 40 x 40 box) at (200, 200), then `newMap` on the flat 400 x 400 map: the map's
    /// cells are 0..=39 each way, `newMap` adds the depot, so the cell under it (20, 20) is an obstacle; clear once it is
    /// removed from the pathfind map, an obstacle again once it is added; (5, 5) stays clear. Without `live_pathfinder`
    /// the call is the cut point as before.
    ///
    /// Written first with the fixture's barracks (a structure without `KINDOF_IMMOBILE`), it measured clear: upstream's
    /// `isMobile()` is true for such an object and the pathfinder does not path around mobile objects. Real structures
    /// carry `IMMOBILE`; the fixture's depot does.
    #[test]
    fn an_object_is_classified_on_the_worlds_map() {
        let (mut w, _) = world();
        let b = make(&mut w, b"Depot", 200.0, 200.0);
        w.pathfinder_add_object_to_map(b);
        assert!(w.cuts.log.as_ref().unwrap().iter().any(|c| matches!(c, CutCall::PathfinderAddObjectToPathfindMap { object } if *object == b)));
        assert_eq!(w.pathfinder_cell_type(LAYER_GROUND, 20, 20), None);

        w.cuts.live_pathfinder = true;
        assert!(w.pathfinder_op(PfOp::NewMap));
        let pf = w.pathfinder.as_ref().unwrap();
        assert_eq!((pf.extent.lo.x, pf.extent.lo.y, pf.extent.hi.x, pf.extent.hi.y), (0, 0, 39, 39));
        // (`newMap` adds the objects there are: the barracks is on the map already)
        assert_eq!(cell(&w, 20, 20), i32::from(CELL_OBSTACLE));
        w.pathfinder_remove_object_from_map(b);
        assert_eq!((cell(&w, 20, 20), cell(&w, 5, 5)), (i32::from(CELL_CLEAR), i32::from(CELL_CLEAR)));
        w.pathfinder_add_object_to_map(b);
        assert_eq!((cell(&w, 20, 20), cell(&w, 5, 5)), (i32::from(CELL_OBSTACLE), i32::from(CELL_CLEAR)));
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }

    /// A call that changes the pathfinder while it is out of the world (upstream re-entering it) waits, logged, and runs as
    /// soon as the pathfinder is back.
    #[test]
    fn a_call_while_the_pathfinder_runs_is_deferred() {
        let (mut w, _) = world();
        w.cuts.live_pathfinder = true;
        w.pathfinder_op(PfOp::NewMap);
        let b = make(&mut w, b"Depot", 200.0, 200.0);
        let during = w.with_pathfinder(|pf, w| {
            w.pathfinder_add_object_to_map(b);
            pf.get_cell(LAYER_GROUND, 20, 20).map(|c| pf.store.cells[c as usize].get_type())
        });
        assert_eq!(during, Some(Some(CELL_CLEAR)));
        assert!(w.cuts.log.as_ref().unwrap().iter().any(|c| matches!(c, CutCall::Host { call: "Pathfinder::deferred", .. })));
        assert!(w.cuts.pathfinder.deferred.is_empty());
        assert_eq!(cell(&w, 20, 20), i32::from(CELL_OBSTACLE));
    }

    /// A wall piece (a 60 x 50 box at (100, 100)) and `newMap`: the wall layer covers it, and `isPointOnWall` answers true
    /// at the wall's centre and false away from it. The world's [`WallView`] answers as the pathfinder does at every point
    /// of a 5-unit grid over the map, and the wall layer's height is the wall height there.
    ///
    /// Written first with a 10-unit-thick wall, the centre measured off the wall: a wall layer cell is clear (on the wall)
    /// only when its four corners are on it, else `CELL_BRIDGE_IMPASSABLE` (its edge), so a wall one cell thick has no
    /// cell on it. Made 30 thick, it measured off the wall again: `classifyWallCells` then pinches a clear cell with any
    /// cell round it not clear, and a pinched clear cell becomes `CELL_CLIFF`, so a cell is on the wall only with its eight
    /// neighbours wholly on it too. The wall is 50 thick: rows 9 and 10 (and columns 8 to 11) are on it.
    #[test]
    fn the_wall_view_answers_as_the_pathfinder() {
        let (mut w, _) = world();
        w.cuts.live_pathfinder = true;
        w.cuts.wall_height = 20.0;
        let wall = make(&mut w, b"Wall", 100.0, 100.0);
        w.pathfinder_op(PfOp::AddWallPiece(wall));
        w.pathfinder_op(PfOp::NewMap);
        let pf = w.pathfinder.as_ref().unwrap();
        let centre = Coord3D { x: 100.0, y: 100.0, z: 0.0 };
        assert!(pf.is_point_on_wall(&centre));
        assert!(!pf.is_point_on_wall(&Coord3D { x: 300.0, y: 300.0, z: 0.0 }));
        let mut on = 0;
        for i in 0..80 {
            for j in 0..80 {
                let p = Coord3D { x: i as f32 * 5.0 + 2.5, y: j as f32 * 5.0 + 2.5, z: 0.0 };
                assert_eq!(w.pf_wall.is_point_on_wall(&p), pf.is_point_on_wall(&p), "{p:?}");
                on += usize::from(pf.is_point_on_wall(&p));
            }
        }
        assert!(on > 0);
        assert_eq!(MovementHost::layer_height(&w, 100.0, 100.0, crate::common::game_type::LAYER_WALL), 20.0);
        assert_eq!(MovementHost::layer_height(&w, 300.0, 300.0, crate::common::game_type::LAYER_WALL), 0.0);
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
