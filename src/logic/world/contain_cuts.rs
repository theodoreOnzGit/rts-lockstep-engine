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
    /// `rider->kill()`.
    Kill { object: ObjectId },
    /// `rider->attemptDamage(info)`.
    AttemptDamage { object: ObjectId, info: DamageInfo },
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

    /// `rider->kill()` (`DAMAGE_UNRESISTABLE`, `DEATH_NORMAL`).
    pub fn contain_kill(&mut self, id: ObjectId) {
        self.contain_cut(ContainCall::Kill { object: id });
        if !self.cuts.contain.stub_object_calls {
            self.kill(id, crate::logic::damage::DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
        }
    }

    /// `object->attemptDamage(info)`; the info comes back as the body wrote it.
    pub fn contain_attempt_damage(&mut self, id: ObjectId, info: &mut DamageInfo) {
        self.contain_cut(ContainCall::AttemptDamage { object: id, info: info.clone() });
        if !self.cuts.contain.stub_object_calls {
            self.attempt_damage(id, info);
        }
    }

    /// `obj->setPosition(pos)`.
    pub fn contain_set_position(&mut self, id: ObjectId, pos: &Coord3D) {
        self.contain_cut(ContainCall::SetPosition { object: id, position: *pos });
        if !self.cuts.contain.stub_object_calls {
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
}
