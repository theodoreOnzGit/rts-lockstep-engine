//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameLogic/W3DGhostObject.cpp
//!
//! Sweep 2 (#213): the fog-of-war "ghost" snapshots of `W3DGhostObject` that were absent from the port. Render objects are
//! [`GhostRenderObj`] handles; scene membership is what the caller's scene reports (`robj->Peek_Scene()`), so each function
//! returns the scene operations to apply, in order ([`GhostSceneOp`]). `DEBUG_FOG_MEMORY` is not defined: only the local
//! player's snapshots are handled where upstream has the `#ifdef` (checked at W3DGhostObject.cpp:469 and 1025).

/// Opaque `RenderObjClass *`.
pub type GhostRenderObj = u32;

/// `MAX_PLAYER_COUNT`.
pub const MAX_PLAYER_COUNT: usize = 16;

/// A scene operation the caller performs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GhostSceneOp {
    /// `((SimpleSceneClass *)W3DDisplay::m_3DScene)->Add_Render_Object(robj)`.
    AddRenderObject(GhostRenderObj),
    /// `robj->Remove()`.
    RemoveRenderObject(GhostRenderObj),
    /// `draw->setFullyObscuredByShroud(value)`.
    SetFullyObscuredByShroud(bool),
}

/// `W3DRenderObjectSnapshot` list (`m_next` chain) as a vector, head first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotList {
    pub robjs: Vec<GhostRenderObj>,
}

/// `W3DRenderObjectSnapshot::addToScene` (W3DGhostObject.cpp:156): `Add_Render_Object(m_robj)`.
pub fn snapshot_add_to_scene(robj: GhostRenderObj) -> GhostSceneOp {
    GhostSceneOp::AddRenderObject(robj)
}

/// The members of `W3DGhostObject` these functions use.
#[derive(Clone, Debug)]
pub struct W3DGhostObject {
    /// `m_parentObject != NULL`.
    pub has_parent_object: bool,
    /// `m_parentSnapshots[MAX_PLAYER_COUNT]`.
    pub parent_snapshots: Vec<Option<SnapshotList>>,
    /// `m_partitionData` exists (`getShroudStatus` reads through it).
    pub has_partition_data: bool,
}

impl Default for W3DGhostObject {
    fn default() -> Self {
        W3DGhostObject { has_parent_object: false, parent_snapshots: vec![None; MAX_PLAYER_COUNT], has_partition_data: false }
    }
}

/// A parent object's model draw modules, for the functions that act on them: the render object of each
/// `ObjectDrawInterface` draw module (None when it has none) and whether it is currently in a scene (`Peek_Scene()`).
#[derive(Clone, Copy, Debug)]
pub struct ParentRenderObj {
    pub robj: GhostRenderObj,
    pub in_scene: bool,
}

impl W3DGhostObject {
    /// `W3DGhostObject::removeParentObject` (W3DGhostObject.cpp:399): the parent drawable is flagged fully obscured and each
    /// of its model render objects is removed from the scene (upstream asserts each is in a scene). None without a parent.
    pub fn remove_parent_object(&self, parent_robjs: &[Option<ParentRenderObj>]) -> Vec<GhostSceneOp> {
        if !self.has_parent_object {
            return Vec::new();
        }
        let mut ops = vec![GhostSceneOp::SetFullyObscuredByShroud(true)];
        for p in parent_robjs.iter().flatten() {
            ops.push(GhostSceneOp::RemoveRenderObject(p.robj));
        }
        ops
    }

    /// `W3DGhostObject::restoreParentObject` (W3DGhostObject.cpp:434): the parent drawable is un-obscured and each model
    /// render object that is not in the scene is added back (one in the scene is left). `has_drawable` is
    /// `m_parentObject->getDrawable() != NULL` (the function returns without a drawable).
    pub fn restore_parent_object(&self, has_drawable: bool, parent_robjs: &[Option<ParentRenderObj>]) -> Vec<GhostSceneOp> {
        if !has_drawable {
            return Vec::new();
        }
        let mut ops = vec![GhostSceneOp::SetFullyObscuredByShroud(false)];
        for p in parent_robjs.iter().flatten() {
            if !p.in_scene {
                ops.push(GhostSceneOp::AddRenderObject(p.robj));
            }
        }
        ops
    }

    /// `W3DGhostObject::addToScene` (W3DGhostObject.cpp:560): the player unfogged the object, so every snapshot render
    /// object of that player is added to the scene, head first.
    pub fn add_to_scene(&self, player_index: usize) -> Vec<GhostSceneOp> {
        match &self.parent_snapshots[player_index] {
            Some(list) => list.robjs.iter().map(|r| GhostSceneOp::AddRenderObject(*r)).collect(),
            None => Vec::new(),
        }
    }

    /// `W3DGhostObject::freeAllSnapShots` (W3DGhostObject.cpp:469): for the local player, if there is a snapshot chain:
    /// remove it from the scene (`removeFromScene(playerIndex)`: the caller's, the ops are returned first), restore the
    /// parent object when there is one, and delete the chain. `remove_from_scene_ops` are the ops
    /// `removeFromScene(local)` yields; `restore_ops` those of `restore_parent_object`.
    pub fn free_all_snapshots(&mut self, local_player_index: usize, remove_from_scene_ops: Vec<GhostSceneOp>, restore_ops: Vec<GhostSceneOp>) -> Vec<GhostSceneOp> {
        let mut ops = Vec::new();
        if self.parent_snapshots[local_player_index].is_some() {
            ops.extend(remove_from_scene_ops);
            if self.has_parent_object {
                ops.extend(restore_ops);
            }
            self.parent_snapshots[local_player_index] = None; // delete the chain
        }
        ops
    }

    /// `W3DGhostObject::getShroudStatus` (W3DGhostObject.cpp:591): `m_partitionData->getShroudedStatus(playerIndex)`; the
    /// call has side effects on the partition data (it updates the snapshot), so the caller runs it; here, whether it can.
    pub fn get_shroud_status_available(&self) -> bool {
        self.has_partition_data
    }
}

/// `W3DGhostObjectManager::updateOrphanedObjects` (W3DGhostObject.cpp:1025): ghost objects whose parent is gone and that
/// hold no snapshot for the local player after refreshing their shroud status are dropped (unregistered from the
/// partition manager, partition data cleared, removed from the manager). `get_shroud_status(i)` runs `getShroudStatus` of the
/// i-th used module for the local player; returns the indexes (in `used_modules` order) to remove.
pub fn update_orphaned_objects(used_modules: &mut [W3DGhostObject], local_player: usize, get_shroud_status: &mut impl FnMut(usize)) -> Vec<usize> {
    let mut remove = Vec::new();
    for (i, m) in used_modules.iter_mut().enumerate() {
        if !m.has_parent_object {
            let mut num_stored_snapshots = 0;
            get_shroud_status(i);
            if m.parent_snapshots[local_player].is_some() {
                num_stored_snapshots += 1;
            }
            if num_stored_snapshots == 0 {
                m.has_partition_data = false;
                remove.push(i);
            }
        }
    }
    remove
}
