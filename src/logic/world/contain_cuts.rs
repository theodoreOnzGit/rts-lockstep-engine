// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the cut points of `OpenContain` and the containers built on it, so that the
// modules (`logic::modules::open_contain`, ...) can be translated line by line
// while the units they call are layers 10 to 12. Each method names the
// upstream call it replaces.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this file), with the additional terms in NOTICE.md.

//! The calls the contain modules make on units that are not translated, or that cascade, as [`World`] methods and the tables that answer them.
//!
//! # What is cut and why
//!
//! `OpenContain.cpp` reaches the partition manager (`registerObject`, `unRegisterObject`, `getDistanceSquared`), the pathfinder (`addObjectToPathfindMap`,
//! `removeObjectFromPathfindMap`, `removeUnitFromPathfindMap`, `updatePos`, `updateGoal`, `adjustToPossibleDestination`), the terrain (`getLayerHeight`,
//! `getLayerForDestination`), the drawable (`setDrawableHidden`, `isSelected`, `getModelConditionFlags`) and the bones of the art. The partition manager and
//! the pathfinder are layers 10 and 12 and not yet translated onto the world, the drawable is the client's, and the bones are the art's: each is a method
//! below, one line of the upstream call, with an answer from [`ContainCutTables`] where upstream reads something back.
//!
//! # The Object-level calls, and the explicit ablation
//!
//! `onContainedBy`, `onRemovedFrom`, `kill`, `attemptDamage`, `setPosition`, `setOrientation`, `setLayer`, `setTransformMatrix`, `leaveGroup`, `checkAndDetonateBoobyTrap`
//! and `TheGameLogic->destroyObject` are translated (`object_state`, `object_damage`, `object_transform`, ...) and **run for real by default** (`contain_*` below
//! call them). Each of them cascades (statuses, the radar, the partition cells, die modules), and every cascade has its own differential test; the test of
//! `OpenContain` therefore sets [`ContainCutTables::stub_object_calls`], which replaces each of these calls by a [`ContainCall`] record alone, plus the bookkeeping the
//! container reads back (`m_containedBy`). That is an explicit ablation, off by default, in the spirit of `BodyCutTables::stub_object_calls`.
//!
//! `p1w13-sonnet`, 2026-10-22.

use std::collections::{BTreeMap, BTreeSet};

use crate::common::base_type::Coord3D;
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID};
use crate::logic::damage::DamageInfo;
use crate::logic::world::game_logic::World;
use crate::logic::world::object_cuts::CutCall;
use crate::ww::matrix3d::Matrix3D;

/// The tables behind the cut points of the contain modules, and the switches of their tests. See the module docs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContainCutTables {
    /// The explicit ablation of the module docs: the Object-level calls are records (and the container's own bookkeeping) only.
    pub stub_object_calls: bool,
    /// `getSingleLogicalBonePosition(name, &pos, NULL)`: the bone's position by object and bone name (no entry: the bone does not exist and the call gives
    /// the object's position back).
    pub single_bones: BTreeMap<(ObjectId, Vec<u8>), Coord3D>,
    /// `getMultiLogicalBonePosition("FIREPOINT", max, NULL, transforms, TRUE)`: the transforms by object (at most `max` are used).
    pub fire_points: BTreeMap<ObjectId, Vec<Matrix3D>>,
    /// `getSingleLogicalBonePositionOnTurret(TURRET_MAIN, name, NULL, &matrix)`: the transform by object and bone name (no entry: not found).
    pub turret_bones: BTreeMap<(ObjectId, Vec<u8>), Matrix3D>,
    /// `adjustToPossibleDestination(obj, locomotorSet, &pos)`: the position it writes, by object (no entry: left as it is).
    pub adjusted_destination: BTreeMap<ObjectId, Coord3D>,
    /// `checkAndDetonateBoobyTrap(rider)`: the objects that are booby trapped (the call answers true, and the test's answers decide who is dead afterwards).
    pub booby_trapped: BTreeSet<ObjectId>,
    /// How deep the recursion of `addOrRemoveObjFromWorld` and `isPassengerAllowedToFire` is: a ring of containers recurses for ever upstream.
    pub recursion: u32,
    /// batch R8: the walk of `addOrRemoveObjFromWorld` met a ring of containers (it stops; cleared when the walk ends).
    pub ring_found: bool,
    /// batch R10: a query that reads `&World` met a tunnel whose object has no controlling player (`getControllingPlayer()->getTunnelSystem()` through
    /// `NULL`); the module records it after the call.
    pub null_player_read: std::cell::Cell<bool>,
    /// batch R6: how deep the reads of other objects' contain modules go (`open_contain::view_*`, through `&World`), and whether one went too deep.
    pub view_depth: std::cell::Cell<u32>,
    pub view_overflow: std::cell::Cell<bool>,
    // --- p1w13-sonnet --- (batch R2: TransportContain)
    /// `draw->getPristineBonePositions(name, 0, &pos, NULL, 1)`: the bone's pristine (art) position, by object and bone name (no entry: the bone is not in
    /// the art and the call finds none). Batch R8: a transport's exit bone was the world position here (`exit_bones`); it is now the art's position, which
    /// `convertBonePosToWorldPos` pushes through the object's transform as upstream does.
    pub pristine_bones: BTreeMap<(ObjectId, Vec<u8>), Coord3D>,
    /// `TheAI->pathfinder()->validMovementTerrain(layer, rider's locomotor, pos)`: the riders for which the terrain at the transport is valid.
    pub valid_movement_terrain: BTreeSet<ObjectId>,
    // --- end p1w13-sonnet ---
    // --- p1w13-sonnet --- (batch R8: ParachuteContain)
    /// `ThePartitionManager->findPositionAround(&pos, options, &pos)`: the position found (`None`: none is found and `pos` is left as it was).
    pub find_position: Option<Coord3D>,
    /// `TheAI->pathfinder()->getCell(layer, x, y)->getType()`, by the tile of the terrain tables the cell's indices fall in (`terrain_cell(x, y)` of
    /// the indices as reals); -1 is a `NULL` cell.
    pub cell_types: [i32; 8],
    /// `player->isSkirmishAIPlayer()`: the players whose AI is a skirmish AI.
    pub skirmish_ai_players: BTreeSet<crate::logic::world::ids::PlayerIndex>,
    // --- end p1w13-sonnet ---
}

/// What a contain module did to a unit that is not translated, or that is run for real but cascades. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub enum ContainCall {
    /// `ThePartitionManager->registerObject(obj)`.
    PartitionRegisterObject { object: ObjectId },
    /// `ThePartitionManager->unRegisterObject(obj)`.
    PartitionUnregisterObject { object: ObjectId },
    /// `ThePartitionManager->getDistanceSquared(obj, pos, FROM_CENTER_2D)`.
    PartitionGetDistanceSquared { object: ObjectId, position: Coord3D },
    /// `obj->getDrawable()->setDrawableHidden(hidden)`.
    DrawableSetHidden { object: ObjectId, hidden: bool },
    /// `TheAI->pathfinder()->addObjectToPathfindMap(obj)`.
    PathfindAddObject { object: ObjectId },
    /// `TheAI->pathfinder()->removeObjectFromPathfindMap(obj)`.
    PathfindRemoveObject { object: ObjectId },
    /// `TheAI->pathfinder()->removeUnitFromPathfindMap(obj)`.
    PathfindRemoveUnit { object: ObjectId },
    /// `TheAI->pathfinder()->updatePos(obj, pos)`.
    PathfindUpdatePos { object: ObjectId, position: Coord3D },
    /// `TheAI->pathfinder()->updateGoal(obj, pos, layer)`.
    PathfindUpdateGoal { object: ObjectId, position: Coord3D, layer: PathfindLayer },
    /// `TheAI->pathfinder()->adjustToPossibleDestination(obj, locomotorSet, &pos)`, `before` the position it was given.
    PathfindAdjustToPossibleDestination { object: ObjectId, before: Coord3D },
    /// `TheTerrainLogic->getLayerForDestination(pos)`.
    TerrainGetLayerForDestination { position: Coord3D },
    /// `obj->leaveGroup()`.
    LeaveGroup { object: ObjectId },
    /// `container->checkAndDetonateBoobyTrap(rider)`; `result` is the answer.
    CheckAndDetonateBoobyTrap { container: ObjectId, rider: ObjectId, result: bool },
    /// `rider->onContainedBy(container)`.
    OnContainedBy { rider: ObjectId, container: ObjectId },
    /// `rider->onRemovedFrom(container)`.
    OnRemovedFrom { rider: ObjectId, container: ObjectId },
    /// `rider->kill()`: the damage it does is the body's maximum health, `kill` true.
    Kill { object: ObjectId, amount: f32 },
    /// `rider->attemptDamage(info)`; `has_body` is whether the object has a body to give it to (`Object::attemptDamage` does nothing without one).
    AttemptDamage { object: ObjectId, info: DamageInfo, has_body: bool },
    /// `obj->setPosition(pos)`.
    SetPosition { object: ObjectId, position: Coord3D },
    /// `obj->setOrientation(angle)`.
    SetOrientation { object: ObjectId, angle: f32 },
    /// `obj->setLayer(layer)`.
    SetLayer { object: ObjectId, layer: PathfindLayer },
    /// `obj->setTransformMatrix(matrix)`.
    SetTransformMatrix { object: ObjectId, matrix: Matrix3D },
    /// `TheGameLogic->destroyObject(obj)`.
    DestroyObject { object: ObjectId },
    /// `getSingleLogicalBonePosition(name, &pos, NULL)`.
    GetSingleLogicalBonePosition { object: ObjectId, name: Vec<u8> },
    /// `getMultiLogicalBonePosition(prefix, max, NULL, transforms, TRUE)`.
    GetMultiLogicalBonePosition { object: ObjectId, prefix: Vec<u8>, max: i32 },
    /// `getSingleLogicalBonePositionOnTurret(TURRET_MAIN, name, NULL, &matrix)`.
    GetSingleLogicalBonePositionOnTurret { object: ObjectId, name: Vec<u8> },
    // --- p1w13-sonnet --- (batch R2: TransportContain)
    /// `obj->setDisabled(type)`.
    SetDisabled { object: ObjectId, disabled: i32 },
    /// `obj->clearDisabled(type)`.
    ClearDisabled { object: ObjectId, disabled: i32 },
    /// `TheThingFactory->newObject(template, team)` for a transport's initial payload.
    NewObject { template: u16, team: i32 },
    // batch R4 (HelixContain)
    /// `portable->setTeam(team)`.
    SetTeam { object: ObjectId, team: i32 },
    /// `portable->getBodyModule()->setDamageState(state)`.
    SetDamageState { object: ObjectId, state: i32 },
    /// `obj->getStealth()->receiveGrant(active, frames)` (the object has a stealth update).
    StealthReceiveGrant { object: ObjectId, active: bool, frames: u32 },
    // batch R5 (RiderChangeContain)
    /// `obj->getExperienceTracker()->setVeterancyLevel(level, FALSE)`.
    ExperienceSetVeterancyLevel { object: ObjectId, level: i32 },
    /// `obj->getExperienceTracker()->setExperienceAndLevel(experience, FALSE)`.
    ExperienceSetExperienceAndLevel { object: ObjectId, experience: i32 },
    /// `obj->kill(damageType, deathType)`: unresistable damage of the body's maximum health.
    KillWith { object: ObjectId, amount: f32, damage_type: i32, death_type: i32 },
    // batch R6 (OverlordContain)
    /// `obj->getExperienceTracker()->setExperienceSink(sink)`.
    ExperienceSetSink { object: ObjectId, sink: ObjectId },
    // batch R8 (ParachuteContain)
    /// `ThePartitionManager->findPositionAround(&pos, options, &pos)` (`minRadius`, `maxRadius`, `flags`; no relationship object).
    PartitionFindPositionAround { position: Coord3D, min_radius: f32, max_radius: f32, flags: i32 },
    /// `TheAI->pathfinder()->getCell(layer, x, y)`.
    PathfindGetCell { layer: PathfindLayer, x: i32, y: i32 },
    /// `obj->getDrawable()->setInstanceMatrix(matrix)` (`None` for `NULL`).
    DrawableSetInstanceMatrix { object: ObjectId, matrix: Option<Matrix3D> },
    // batch R10 (TunnelContain)
    /// `player->getAcademyStats()->recordUnitEnteredTunnelNetwork()`.
    AcademyUnitEnteredTunnelNetwork { player: i32 },
    /// `obj->handlePartitionCellMaintenance()`.
    HandlePartitionCellMaintenance { object: ObjectId },
    /// `obj->setSafeOcclusionFrame(frame)`.
    SetSafeOcclusionFrame { object: ObjectId, frame: u32 },
    // --- end p1w13-sonnet ---
}

impl World {
    fn contain_cut(&mut self, call: ContainCall) {
        self.cut(CutCall::Contain(call));
    }

    /// `ThePartitionManager->registerObject(obj)`.
    pub fn contain_partition_register_object(&mut self, id: ObjectId) {
        self.contain_cut(ContainCall::PartitionRegisterObject { object: id });
    }

    /// `ThePartitionManager->unRegisterObject(obj)`.
    pub fn contain_partition_unregister_object(&mut self, id: ObjectId) {
        self.contain_cut(ContainCall::PartitionUnregisterObject { object: id });
    }

    /// `ThePartitionManager->getDistanceSquared(obj, pos, FROM_CENTER_2D)`: the squared distance in the plane from the object's centre.
    pub fn contain_partition_get_distance_squared(&mut self, id: ObjectId, pos: &Coord3D) -> f32 {
        self.contain_cut(ContainCall::PartitionGetDistanceSquared { object: id, position: *pos });
        let p = self.objects.find(id).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position());
        let (dx, dy) = (p.x - pos.x, p.y - pos.y);
        dx * dx + dy * dy
    }

    /// `obj->getDrawable()->setDrawableHidden(hidden)`.
    pub fn contain_drawable_set_hidden(&mut self, id: ObjectId, hidden: bool) {
        self.contain_cut(ContainCall::DrawableSetHidden { object: id, hidden });
    }

    /// `TheAI->pathfinder()->addObjectToPathfindMap(obj)`.
    pub fn contain_pathfind_add_object(&mut self, id: ObjectId) {
        self.contain_cut(ContainCall::PathfindAddObject { object: id });
    }

    /// `TheAI->pathfinder()->removeObjectFromPathfindMap(obj)`.
    pub fn contain_pathfind_remove_object(&mut self, id: ObjectId) {
        self.contain_cut(ContainCall::PathfindRemoveObject { object: id });
    }

    /// `TheAI->pathfinder()->removeUnitFromPathfindMap(obj)`.
    pub fn contain_pathfind_remove_unit(&mut self, id: ObjectId) {
        self.contain_cut(ContainCall::PathfindRemoveUnit { object: id });
    }

    /// `TheAI->pathfinder()->updatePos(obj, pos)`.
    pub fn contain_pathfind_update_pos(&mut self, id: ObjectId, pos: &Coord3D) {
        self.contain_cut(ContainCall::PathfindUpdatePos { object: id, position: *pos });
    }

    /// `TheAI->pathfinder()->updateGoal(obj, pos, layer)`.
    pub fn contain_pathfind_update_goal(&mut self, id: ObjectId, pos: &Coord3D, layer: PathfindLayer) {
        self.contain_cut(ContainCall::PathfindUpdateGoal { object: id, position: *pos, layer });
    }

    /// `TheAI->pathfinder()->adjustToPossibleDestination(obj, locomotorSet, &pos)`: `pos` is written when the table has an answer for the object.
    pub fn contain_pathfind_adjust_to_possible_destination(&mut self, id: ObjectId, pos: &mut Coord3D) {
        self.contain_cut(ContainCall::PathfindAdjustToPossibleDestination { object: id, before: *pos });
        if let Some(p) = self.cuts.contain.adjusted_destination.get(&id) {
            *pos = *p;
        }
    }

    /// `TheTerrainLogic->getLayerForDestination(pos)`: the layer table of the terrain cuts.
    pub fn contain_terrain_get_layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        self.contain_cut(ContainCall::TerrainGetLayerForDestination { position: *pos });
        let cell = crate::logic::world::object_cuts::terrain_cell(pos.x, pos.y);
        self.cuts.terrain_layer[cell]
    }

    /// `obj->leaveGroup()`.
    pub fn contain_leave_group(&mut self, id: ObjectId) {
        self.contain_cut(ContainCall::LeaveGroup { object: id });
        if !self.cuts.contain.stub_object_calls {
            self.leave_group(id);
        }
    }

    /// `container->checkAndDetonateBoobyTrap(rider)`.
    pub fn contain_check_and_detonate_booby_trap(&mut self, container: ObjectId, rider: ObjectId) -> bool {
        let result = if self.cuts.contain.stub_object_calls { self.cuts.contain.booby_trapped.contains(&rider) } else { self.check_and_detonate_booby_trap(container, Some(rider)) };
        self.contain_cut(ContainCall::CheckAndDetonateBoobyTrap { container, rider, result });
        result
    }

    /// `rider->onContainedBy(container)`.
    pub fn contain_on_contained_by(&mut self, rider: ObjectId, container: ObjectId) {
        self.contain_cut(ContainCall::OnContainedBy { rider, container });
        if self.cuts.contain.stub_object_calls {
            // the bookkeeping the container reads back: who contains the rider
            let now = self.frame();
            if let Some(o) = self.objects.find_mut(rider) {
                o.contained_by = container;
                o.contained_by_frame = now;
            }
        } else {
            self.on_contained_by(rider, container);
        }
    }

    /// `rider->onRemovedFrom(container)`.
    pub fn contain_on_removed_from(&mut self, rider: ObjectId, container: ObjectId) {
        self.contain_cut(ContainCall::OnRemovedFrom { rider, container });
        if self.cuts.contain.stub_object_calls {
            if let Some(o) = self.objects.find_mut(rider) {
                o.contained_by = INVALID_ID;
                o.contained_by_frame = 0;
            }
        } else {
            self.on_removed_from(rider);
        }
    }

    /// `rider->kill()` (`DAMAGE_UNRESISTABLE`, `DEATH_NORMAL`): `Object::kill` takes the amount from `getBodyModule()->getMaxHealth()`, so `false` is a body that
    /// is `NULL` (a call through a null pointer upstream; nothing is done).
    pub fn contain_kill(&mut self, id: ObjectId) -> bool {
        let Some(body) = self.objects.find(id).and_then(|o| o.body) else { return false };
        let amount = self.module_call(id, body, crate::logic::world::module_call::ModuleCall::GetMaxHealth).as_real(0.0);
        self.contain_cut(ContainCall::Kill { object: id, amount });
        if !self.cuts.contain.stub_object_calls {
            self.kill(id, crate::logic::damage::DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
        }
        true
    }

    /// `object->attemptDamage(info)`; the info comes back as the body wrote it.
    pub fn contain_attempt_damage(&mut self, id: ObjectId, info: &mut DamageInfo) {
        let has_body = self.objects.find(id).is_some_and(|o| o.body.is_some());
        self.contain_cut(ContainCall::AttemptDamage { object: id, info: info.clone(), has_body });
        if !self.cuts.contain.stub_object_calls {
            self.attempt_damage(id, info);
        }
    }

    /// `obj->setPosition(pos)`.
    pub fn contain_set_position(&mut self, id: ObjectId, pos: &Coord3D) {
        self.contain_cut(ContainCall::SetPosition { object: id, position: *pos });
        if self.cuts.contain.stub_object_calls {
            // the bookkeeping the container reads back: where the object is (the thing's own place, none of `reactToTransformChange`'s cascade)
            let angle = match self.objects.find(id) {
                Some(o) => o.thing.orientation(),
                None => return,
            };
            if let Some(ctx) = self.setter_context(id, Some((angle, *pos))) {
                if let Some(o) = self.objects.find_mut(id) {
                    let _ = o.thing.set_position(&ctx, Some(pos));
                    o.position = *o.thing.position();
                }
            }
        } else {
            self.set_position(id, pos);
        }
    }

    /// `obj->setOrientation(angle)`.
    pub fn contain_set_orientation(&mut self, id: ObjectId, angle: f32) {
        self.contain_cut(ContainCall::SetOrientation { object: id, angle });
        if !self.cuts.contain.stub_object_calls {
            self.set_orientation(id, angle);
        }
    }

    /// `obj->setLayer(layer)`.
    pub fn contain_set_layer(&mut self, id: ObjectId, layer: PathfindLayer) {
        self.contain_cut(ContainCall::SetLayer { object: id, layer });
        if !self.cuts.contain.stub_object_calls {
            self.set_layer(id, layer);
        }
        // (the stub leaves the layer alone: the test's objects have the layers the scene gave them)
    }

    /// `obj->setTransformMatrix(matrix)`.
    pub fn contain_set_transform_matrix(&mut self, id: ObjectId, matrix: &Matrix3D) {
        self.contain_cut(ContainCall::SetTransformMatrix { object: id, matrix: *matrix });
        if !self.cuts.contain.stub_object_calls {
            self.set_transform_matrix(id, matrix);
        }
    }

    /// `TheGameLogic->destroyObject(obj)`.
    pub fn contain_destroy_object(&mut self, id: ObjectId) {
        self.contain_cut(ContainCall::DestroyObject { object: id });
        if !self.cuts.contain.stub_object_calls {
            self.destroy_object(id);
        }
    }

    /// `obj->getSingleLogicalBonePosition(name, &pos, NULL)`: the bone's position, or the object's own when there is no such bone.
    pub fn contain_get_single_logical_bone_position(&mut self, id: ObjectId, name: &[u8]) -> Coord3D {
        self.contain_cut(ContainCall::GetSingleLogicalBonePosition { object: id, name: name.to_vec() });
        match self.cuts.contain.single_bones.get(&(id, name.to_vec())) {
            Some(p) => *p,
            None => self.objects.find(id).map_or(Coord3D { x: 0.0, y: 0.0, z: 0.0 }, |o| o.get_position()),
        }
    }

    /// `obj->getMultiLogicalBonePosition(prefix, max, NULL, transforms, TRUE)`: the transforms found.
    pub fn contain_get_multi_logical_bone_position(&mut self, id: ObjectId, prefix: &[u8], max: i32) -> Vec<Matrix3D> {
        self.contain_cut(ContainCall::GetMultiLogicalBonePosition { object: id, prefix: prefix.to_vec(), max });
        let has_drawable = self.objects.find(id).is_some_and(|o| o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID);
        if !has_drawable {
            return Vec::new();
        }
        let mut found = self.cuts.contain.fire_points.get(&id).cloned().unwrap_or_default();
        found.truncate(max.max(0) as usize);
        found
    }

    /// `obj->getSingleLogicalBonePositionOnTurret(TURRET_MAIN, name, NULL, &matrix)`: the transform, `None` when the call gives false.
    pub fn contain_get_single_logical_bone_position_on_turret(&mut self, id: ObjectId, name: &[u8]) -> Option<Matrix3D> {
        self.contain_cut(ContainCall::GetSingleLogicalBonePositionOnTurret { object: id, name: name.to_vec() });
        self.cuts.contain.turret_bones.get(&(id, name.to_vec())).copied()
    }

    // --- p1w13-sonnet --- (batch R2: TransportContain)
    /// `rider->setDisabled(type)`. The stub records the call only (the test's stand-in logs it and keeps no bit).
    pub fn contain_set_disabled(&mut self, id: ObjectId, disabled: crate::common::disabled_types::DisabledType) {
        self.contain_cut(ContainCall::SetDisabled { object: id, disabled });
        if !self.cuts.contain.stub_object_calls {
            self.set_disabled(id, disabled);
        }
    }

    /// `rider->clearDisabled(type)`. The stub records the call and clears the bit (the bookkeeping the stand-in keeps).
    pub fn contain_clear_disabled(&mut self, id: ObjectId, disabled: crate::common::disabled_types::DisabledType) {
        self.contain_cut(ContainCall::ClearDisabled { object: id, disabled });
        if self.cuts.contain.stub_object_calls {
            if let Some(o) = self.objects.find_mut(id) {
                o.disabled.set(disabled, 0);
            }
        } else {
            self.clear_disabled(id, disabled);
        }
    }

    /// `getDrawable()->setModelConditionState(a)` (`set`) or `clearModelConditionState(a)`, if the object has a drawable.
    pub fn contain_drawable_model_condition(&mut self, id: ObjectId, a: i32, set: bool) {
        if set {
            self.set_model_condition_state(id, a);
        } else {
            self.clear_model_condition_state(id, a);
        }
    }

    /// The world position of a transport's exit bone, if it has a drawable and the art has the bone: `getPristineBonePositions` then
    /// `convertBonePosToWorldPos` (see [`ContainCutTables::pristine_bones`]).
    pub fn contain_exit_bone_world_position(&mut self, id: ObjectId, bone: &[u8]) -> Option<Coord3D> {
        let pos = self.contain_drawable_pristine_bone(id, bone)?;
        Some(self.contain_convert_bone_pos_to_world_pos(id, &pos))
    }

    // batch R8 (ParachuteContain)
    /// `obj->getDrawable()->getPristineBonePositions(name, 0, &pos, NULL, 1) == 1`: the bone's art position, `None` when the object has no drawable or
    /// the art has no such bone (see [`ContainCutTables::pristine_bones`]).
    pub fn contain_drawable_pristine_bone(&mut self, id: ObjectId, bone: &[u8]) -> Option<Coord3D> {
        if !self.has_drawable(id) {
            return None;
        }
        self.cuts.contain.pristine_bones.get(&(id, bone.to_vec())).copied()
    }

    /// `obj->convertBonePosToWorldPos(&bonePos, NULL, &worldPos, NULL)`: the position pushed through the object's transform (`Thing`'s, translated).
    pub fn contain_convert_bone_pos_to_world_pos(&self, id: ObjectId, bone: &Coord3D) -> Coord3D {
        self.objects.find(id).map_or(*bone, |o| o.thing.bone_position_to_world(bone))
    }

    /// `ThePartitionManager->findPositionAround(&pos, options, &pos)`: `pos` is written when a position is found (see
    /// [`ContainCutTables::find_position`]).
    pub fn contain_partition_find_position_around(&mut self, pos: &mut Coord3D, min_radius: f32, max_radius: f32, flags: i32) -> bool {
        self.contain_cut(ContainCall::PartitionFindPositionAround { position: *pos, min_radius, max_radius, flags });
        match self.cuts.contain.find_position {
            Some(p) => {
                *pos = p;
                true
            }
            None => false,
        }
    }

    /// `TheAI->pathfinder()->getCell(layer, x, y)`, then `getType()`: `None` for a `NULL` cell (see [`ContainCutTables::cell_types`]).
    pub fn contain_pathfind_cell_type(&mut self, layer: PathfindLayer, x: i32, y: i32) -> Option<i32> {
        self.contain_cut(ContainCall::PathfindGetCell { layer, x, y });
        let t = self.cuts.contain.cell_types[crate::logic::world::object_cuts::terrain_cell(x as f32, y as f32)];
        (t >= 0).then_some(t)
    }

    /// `player->isSkirmishAIPlayer()` (see [`ContainCutTables::skirmish_ai_players`]).
    pub fn contain_player_is_skirmish_ai(&self, player: crate::logic::world::ids::PlayerIndex) -> bool {
        self.cuts.contain.skirmish_ai_players.contains(&player)
    }

    // batch R10 (TunnelContain)
    /// `player->getAcademyStats()->recordUnitEnteredTunnelNetwork()`. The stub records the call only.
    pub fn contain_record_unit_entered_tunnel_network(&mut self, player: crate::logic::world::ids::PlayerIndex) {
        self.contain_cut(ContainCall::AcademyUnitEnteredTunnelNetwork { player });
        if !self.cuts.contain.stub_object_calls {
            if let Some(p) = self.players.get_nth_mut(player) {
                p.academy_stats.record_unit_entered_tunnel_network();
            }
        }
    }

    /// `obj->handlePartitionCellMaintenance()`. The stub records the call only.
    pub fn contain_handle_partition_cell_maintenance(&mut self, id: ObjectId) {
        self.contain_cut(ContainCall::HandlePartitionCellMaintenance { object: id });
        if !self.cuts.contain.stub_object_calls {
            self.handle_partition_cell_maintenance(id);
        }
    }

    /// `obj->setSafeOcclusionFrame(frame)`. The stub records the call only.
    pub fn contain_set_safe_occlusion_frame(&mut self, id: ObjectId, frame: u32) {
        self.contain_cut(ContainCall::SetSafeOcclusionFrame { object: id, frame });
        if !self.cuts.contain.stub_object_calls {
            if let Some(o) = self.objects.find_mut(id) {
                o.safe_occlusion_frame = frame;
            }
        }
    }

    /// `obj->getDrawable()->setInstanceMatrix(matrix)`: the client's; the caller checks the drawable.
    pub fn contain_drawable_set_instance_matrix(&mut self, id: ObjectId, matrix: Option<&Matrix3D>) {
        self.contain_cut(ContainCall::DrawableSetInstanceMatrix { object: id, matrix: matrix.copied() });
    }

    /// `TheAI->pathfinder()->validMovementTerrain(transport's layer, rider's locomotor, pos)` (see [`ContainCutTables::valid_movement_terrain`]).
    pub fn contain_valid_movement_terrain(&mut self, _transport: ObjectId, rider: ObjectId, _pos: &Coord3D) -> bool {
        self.cuts.contain.valid_movement_terrain.contains(&rider)
    }

    /// `TheThingFactory->newObject(template, team)` for a payload; `INVALID_ID` when the stub records it (the test's factory makes nothing).
    pub fn contain_new_payload_object(&mut self, template: u16, team: crate::logic::world::ids::TeamId, status: crate::common::object_status_types::ObjectStatusMaskType) -> ObjectId {
        self.contain_cut(ContainCall::NewObject { template, team: team as i32 });
        if self.cuts.contain.stub_object_calls {
            return INVALID_ID;
        }
        self.thing_factory_new_object(template, Some(team), status).unwrap_or(INVALID_ID)
    }

    // batch R4 (HelixContain)
    /// `obj->setTeam(team)`. The stub records the call only.
    pub fn contain_set_team(&mut self, id: ObjectId, team: crate::logic::world::ids::TeamId) {
        self.contain_cut(ContainCall::SetTeam { object: id, team: team as i32 });
        if !self.cuts.contain.stub_object_calls {
            self.set_team(id, Some(team));
        }
    }

    /// `obj->getBodyModule()->setDamageState(state)`: `false` when the object has no body (the call would be through `NULL`).
    pub fn contain_set_damage_state(&mut self, id: ObjectId, state: i32) -> bool {
        let Some(body) = self.objects.find(id).and_then(|o| o.body) else { return false };
        self.contain_cut(ContainCall::SetDamageState { object: id, state });
        if !self.cuts.contain.stub_object_calls {
            self.module_call(id, body, crate::logic::world::module_call::ModuleCall::SetDamageState(state));
        }
        true
    }

    /// `obj->getStealth()`, then `receiveGrant(active, frames)` if there is one.
    pub fn contain_stealth_receive_grant(&mut self, id: ObjectId, active: bool, frames: u32) {
        let Some(stealth) = self.objects.find(id).and_then(|o| o.stealth) else { return };
        self.contain_cut(ContainCall::StealthReceiveGrant { object: id, active, frames });
        if !self.cuts.contain.stub_object_calls {
            self.module_call(id, stealth, crate::logic::world::module_call::ModuleCall::StealthReceiveGrant { active, frames });
        }
    }

    // batch R5 (RiderChangeContain)
    /// `obj->getExperienceTracker()->setVeterancyLevel(level, FALSE)`. The stub records the call and sets the level (the bookkeeping the stand-in
    /// keeps); the real one is `ExperienceTracker`'s, with its feedback.
    pub fn contain_experience_set_veterancy_level(&mut self, id: ObjectId, level: i32) {
        self.contain_cut(ContainCall::ExperienceSetVeterancyLevel { object: id, level });
        let level = match level {
            1 => crate::common::game_common::VeterancyLevel::Veteran,
            2 => crate::common::game_common::VeterancyLevel::Elite,
            3 => crate::common::game_common::VeterancyLevel::Heroic,
            _ => crate::common::game_common::VeterancyLevel::Regular,
        };
        if self.cuts.contain.stub_object_calls {
            if let Some(o) = self.objects.find_mut(id) {
                o.experience.level = level;
            }
        } else {
            self.cut_experience_set_veterancy_level(id, level, false);
        }
    }

    /// `obj->getExperienceTracker()->setExperienceAndLevel(experience, FALSE)`. The stub records the call only.
    pub fn contain_experience_set_experience_and_level(&mut self, id: ObjectId, experience: i32) {
        self.contain_cut(ContainCall::ExperienceSetExperienceAndLevel { object: id, experience });
        if !self.cuts.contain.stub_object_calls {
            self.cut_experience_set_experience_and_level(id, experience, false);
        }
    }

    /// `obj->clearModelConditionFlags(flags)`, if the object has a drawable (one client event naming them all).
    pub fn contain_clear_model_condition_flags(&mut self, id: ObjectId, flags: &[i32]) {
        if self.has_drawable(id) {
            self.emit(crate::logic::world::object_events::ObjectClientEvent::DrawableModelCondition {
                object_id: id,
                op: crate::logic::world::object_events::ModelConditionOp::Clear(flags.to_vec()),
            });
        }
    }

    /// `obj->kill(damageType, deathType)`; `false` when the object has no body (the maximum health is read through `NULL`).
    pub fn contain_kill_with(&mut self, id: ObjectId, damage_type: i32, death_type: i32) -> bool {
        let Some(body) = self.objects.find(id).and_then(|o| o.body) else { return false };
        let amount = self.module_call(id, body, crate::logic::world::module_call::ModuleCall::GetMaxHealth).as_real(0.0);
        self.contain_cut(ContainCall::KillWith { object: id, amount, damage_type, death_type });
        if !self.cuts.contain.stub_object_calls {
            self.kill(id, damage_type, death_type);
        }
        true
    }

    // batch R6 (OverlordContain)
    /// `obj->getExperienceTracker()->setExperienceSink(sink)`: the object's experience goes to `sink` from now on. The stub records the call and keeps
    /// the field (the stand-in does the same).
    pub fn contain_experience_set_sink(&mut self, id: ObjectId, sink: ObjectId) {
        self.contain_cut(ContainCall::ExperienceSetSink { object: id, sink });
        if let Some(o) = self.objects.find_mut(id) {
            o.experience.sink = sink;
        }
    }
    // --- end p1w13-sonnet ---
}
