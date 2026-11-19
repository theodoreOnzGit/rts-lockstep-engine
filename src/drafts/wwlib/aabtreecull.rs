//! PARTLY DIFFERENTIAL-TESTED (#215): matches the upstream C++ (tests/differential_draft_cull.rs, 24,000 random programs, with the structure dumped after every operation).
//! NOT covered: `save`, `load`, `load_object_linkage`, `save_object_linkage` (chunk I/O) and the typed `get_*_collected_object` wrappers (the same as `peek_*`). Not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aabtreecull.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aabtreecull.cpp
//!
//! `AABTreeCullSystemClass`, `AABTreeNodeClass`, `AABTreeLinkClass`, `AABTreeIterator`. See `cullsys.rs` for how the
//! raw pointers became handles; the node tree is an arena (`nodes`, [`NodeId`] indices) because nodes point at their
//! parent and children. `IndexedNodes` maps a node's depth-first `Index` to its arena id. The `MemPool`
//! (`DEFINE_AUTO_POOL`) is Rust's allocator. The virtual `Load_Node_Contents` / `Save_Node_Contents` hooks are no-ops
//! (upstream's defaults); `TypedAABTreeCullSystemClass<T>` is this struct, objects being [`CullableHandle`]s. C's
//! `rand()` (split plane choice) is a caller closure; the same closure state reproduces upstream's tree.
//!
//! Upstream defects kept: `Select_Splitting_Plane_Brute_Force` builds planes 1..5 from `box.Center` where `box.Extent`
//! was meant (`Center.X - Center.Y`, `Center.Y + Center.Y`, ...); the `MIN(NUM_TRYS, objcount)` loop never leaves `trys`
//! below its limit, so the brute-force fallback runs whenever no random plane scored.

use crate::logic::modules::plant;
use super::aabox::{AABoxClass, MinMaxAABoxClass};
use super::aaplane::{AAPlaneClass, AxisEnum};
use super::chunk_save::ChunkSaveClass;
use super::colmath::OverlapType;
use super::colmathaabox::{overlap_test_aabox_aabox, overlap_test_aabox_sphere};
use super::colmathfrustum::{overlap_test_frustum_aabox, overlap_test_frustum_aabox_planes_passed};
use super::colmathobbox::overlap_test_obbox_aabox;
use super::colmathplane::overlap_test_aaplane_point;
use super::cullsys::{CullLinkClass, CullSystemClass, CullSystemId, CullableHandle};
use super::frustum::FrustumClass;
use super::obbox::OBBoxClass;
use super::sphere::SphereClass;
use crate::ww::chunkio::{ChunkIoResult, ChunkLoad};
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `const uint32 AABTREE_CURRENT_VERSION = 0x00010000;`
pub const AABTREE_CURRENT_VERSION: u32 = 0x00010000;

/// The chunk ids (aabtreecull.cpp:60): `AABTREE_CHUNK_VERSION = 1`, `AABTREE_CHUNK_AABNODE = 0x101` then
/// `_INFO` (0x102, obsolete), `_CONTENTS` (0x103), `_VARIABLES` (0x104); `AABTREE_CHUNK_NODE_INDEX = 0x200`;
/// micro chunks `AABTREE_VARIABLE_NODESTRUCT = 0`, `AABTREE_VARIABLE_USERDATA = 1`.
pub const AABTREE_CHUNK_VERSION: u32 = 0x00000001;
pub const AABTREE_CHUNK_AABNODE: u32 = 0x00000101;
pub const AABTREE_CHUNK_AABNODE_INFO: u32 = 0x00000102;
pub const AABTREE_CHUNK_AABNODE_CONTENTS: u32 = 0x00000103;
pub const AABTREE_CHUNK_AABNODE_VARIABLES: u32 = 0x00000104;
pub const AABTREE_CHUNK_NODE_INDEX: u32 = 0x00000200;
pub const AABTREE_VARIABLE_NODESTRUCT: u32 = 0x00;
pub const AABTREE_VARIABLE_USERDATA: u32 = 0x01;

/// `AABNODE_ATTRIBUTE_FRONT_CHILD`, `AABNODE_ATTRIBUTE_BACK_CHILD`.
pub const AABNODE_ATTRIBUTE_FRONT_CHILD: u32 = 0x00000001;
pub const AABNODE_ATTRIBUTE_BACK_CHILD: u32 = 0x00000002;

/// Index of a node in the system's arena.
pub type NodeId = usize;

/// `struct IOAABNodeStruct` (aabtreecull.cpp:95): centre, extent (`IOVector3Struct` = 3 floats), attributes: 28 bytes.
#[derive(Clone, Copy, Debug, Default)]
pub struct IOAABNodeStruct {
    pub center: [f32; 3],
    pub extent: [f32; 3],
    pub attributes: u32,
}

impl IOAABNodeStruct {
    /// Little-endian image of the struct, as `WRITE_MICRO_CHUNK` writes it.
    pub fn to_bytes(&self) -> [u8; 28] {
        let mut b = [0u8; 28];
        for i in 0..3 {
            b[i * 4..i * 4 + 4].copy_from_slice(&self.center[i].to_le_bytes());
            b[12 + i * 4..12 + i * 4 + 4].copy_from_slice(&self.extent[i].to_le_bytes());
        }
        b[24..28].copy_from_slice(&self.attributes.to_le_bytes());
        b
    }

    /// Inverse of [`IOAABNodeStruct::to_bytes`] (the `READ_MICRO_CHUNK` / `Read(&node_desc, sizeof)` image).
    pub fn from_bytes(b: &[u8; 28]) -> Self {
        let f = |i: usize| f32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        IOAABNodeStruct { center: [f(0), f(1), f(2)], extent: [f(3), f(4), f(5)], attributes: u32::from_le_bytes([b[24], b[25], b[26], b[27]]) }
    }
}

/// `class AABTreeLinkClass : public CullLinkClass`.
#[derive(Clone, Debug)]
pub struct AABTreeLinkClass {
    /// `CullLinkClass::System`.
    pub system: Option<CullSystemId>,
    /// `Node`: partition node containing this object.
    pub node: Option<NodeId>,
    /// `NextObject`: next object in the node.
    pub next_object: Option<CullableHandle>,
}

impl AABTreeLinkClass {
    /// `AABTreeLinkClass(system)`.
    pub fn new(system: CullSystemId) -> Self {
        AABTreeLinkClass { system: Some(system), node: None, next_object: None }
    }
}

/// `get_next_object(obj)`: `((AABTreeLinkClass *)obj->Get_Cull_Link())->NextObject`.
fn get_next_object(obj: &CullableHandle) -> Option<CullableHandle> {
    match obj.read().unwrap().get_cull_link() {
        Some(CullLinkClass::AABTree(l)) => l.next_object.clone(),
        _ => None,
    }
}

fn set_next_object(obj: &CullableHandle, next: Option<CullableHandle>) {
    if let Some(CullLinkClass::AABTree(l)) = obj.write().unwrap().get_cull_link_mut() {
        l.next_object = next;
    }
}

fn get_link_node(obj: &CullableHandle) -> Option<NodeId> {
    match obj.read().unwrap().get_cull_link() {
        Some(CullLinkClass::AABTree(l)) => l.node,
        _ => None,
    }
}

fn set_link_node(obj: &CullableHandle, node: Option<NodeId>) {
    if let Some(CullLinkClass::AABTree(l)) = obj.write().unwrap().get_cull_link_mut() {
        l.node = node;
    }
}

fn cull_box_of(obj: &CullableHandle) -> AABoxClass {
    *obj.read().unwrap().get_cull_box()
}

/// `AABTreeNodeClass::SplitChoiceStruct`.
#[derive(Clone, Copy, Debug)]
pub struct SplitChoiceStruct {
    pub cost: f32,
    pub front_count: i32,
    pub back_count: i32,
    pub front_box: MinMaxAABoxClass,
    pub back_box: MinMaxAABoxClass,
    pub plane: AAPlaneClass,
}

impl SplitChoiceStruct {
    /// `SplitChoiceStruct()`: `Cost(FLT_MAX), FrontCount(0), BackCount(0), Plane(XNORMAL, 0)`, both boxes `Init_Empty()`.
    pub fn new() -> Self {
        let mut front_box = MinMaxAABoxClass::default();
        let mut back_box = MinMaxAABoxClass::default();
        front_box.init_empty();
        back_box.init_empty();
        SplitChoiceStruct { cost: f32::MAX, front_count: 0, back_count: 0, front_box, back_box, plane: AAPlaneClass::new(AxisEnum::XNormal, 0.0) }
    }
}

impl Default for SplitChoiceStruct {
    fn default() -> Self {
        SplitChoiceStruct::new()
    }
}

/// `class AABTreeNodeClass`.
#[derive(Debug)]
pub struct AABTreeNodeClass {
    /// `Index`: depth-first index of this node.
    pub index: u32,
    /// `Box`: bounding box of the node.
    pub box_: AABoxClass,
    pub parent: Option<NodeId>,
    pub front: Option<NodeId>,
    pub back: Option<NodeId>,
    /// `Object`: head of the list of objects in this node.
    pub object: Option<CullableHandle>,
    /// `UserData`: 32bit field for the user, initialized to 0.
    pub user_data: u32,
}

impl AABTreeNodeClass {
    /// `AABTreeNodeClass()`: `Box(Vector3(0,0,0), Vector3(0,0,0))`.
    pub fn new() -> Self {
        AABTreeNodeClass { index: 0, box_: AABoxClass::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0)), parent: None, front: None, back: None, object: None, user_data: 0 }
    }

    /// `Compute_Volume()`: `Box.Volume()`.
    pub fn compute_volume(&self) -> f32 {
        self.box_.volume()
    }

    /// `Object_Count()`: walks the node's list.
    pub fn object_count(&self) -> i32 {
        let mut obj = self.object.clone();
        let mut count = 0;
        while let Some(o) = obj {
            count += 1;
            obj = get_next_object(&o);
        }
        count
    }

    /// `Peek_Object(index)`: asserts the node has an object and `count == index` at the end.
    pub fn peek_object(&self, index: i32) -> Option<CullableHandle> {
        let mut count = 0;
        let mut obj = self.object.clone();
        assert!(obj.is_some());
        while obj.is_some() && count != index {
            count += 1;
            obj = get_next_object(obj.as_ref().unwrap());
        }
        assert!(count == index);
        obj
    }

    /// `Add_Object(obj, update_bounds)` (aabtreecull.cpp:595) for the node whose arena id is `self_id`.
    pub fn add_object(&mut self, self_id: NodeId, obj: &CullableHandle, update_bounds: bool) {
        set_link_node(obj, Some(self_id));
        set_next_object(obj, self.object.take());
        self.object = Some(obj.clone());

        if update_bounds {
            // if this is the only object and we have no children, just copy the object's bounding box, otherwise, add
            // it to what we have
            if self.object_count() == 1 && self.front.is_none() && self.back.is_none() && !plant(11317) {
                self.box_ = cull_box_of(obj);
            } else {
                let b = cull_box_of(obj);
                self.box_.add_box(&b);
            }
        }
    }

    /// `Remove_Object(obj)` (aabtreecull.cpp:626): finds the object in the node's list and unlinks it (nothing happens if
    /// it is not there).
    pub fn remove_object(&mut self, obj: &CullableHandle) {
        let mut prevobj: Option<CullableHandle> = None;
        let mut curobj = self.object.clone();

        while let Some(cur) = curobj {
            let next = get_next_object(&cur);
            if std::sync::Arc::ptr_eq(&cur, obj) {
                // found the object, unlink it.
                match &prevobj {
                    Some(p) => set_next_object(p, next.clone()),
                    None => self.object = next.clone(),
                }
                set_next_object(&cur, None);
                set_link_node(&cur, None);
                return;
            }
            // move to the next object
            prevobj = Some(cur);
            curobj = next;
        }
    }
}

impl Default for AABTreeNodeClass {
    fn default() -> Self {
        AABTreeNodeClass::new()
    }
}

/// `AABTreeCullSystemClass::StatsStruct`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StatsStruct {
    pub node_count: i32,
    pub nodes_accepted: i32,
    pub nodes_trivially_accepted: i32,
    pub nodes_rejected: i32,
}

/// `class AABTreeCullSystemClass : public CullSystemClass`.
#[derive(Debug)]
pub struct AABTreeCullSystemClass {
    pub base: CullSystemClass,
    nodes: Vec<AABTreeNodeClass>,
    /// `RootNode`: root of the AAB-Tree (arena id).
    root_node: NodeId,
    /// `ObjectCount`: number of objects in the system.
    object_count: i32,
    /// `NodeCount`: number of nodes.
    node_count: i32,
    /// `IndexedNodes`: index access to the nodes (arena ids).
    indexed_nodes: Vec<NodeId>,
    stats: StatsStruct,
}

impl AABTreeCullSystemClass {
    /// `AABTreeCullSystemClass()`: a root node, then `Re_Index_Nodes()`.
    pub fn new() -> Self {
        let mut s = AABTreeCullSystemClass {
            base: CullSystemClass::new(),
            nodes: vec![AABTreeNodeClass::new()],
            root_node: 0,
            object_count: 0,
            node_count: 0,
            indexed_nodes: Vec::new(),
            stats: StatsStruct::default(),
        };
        s.re_index_nodes();
        s
    }

    /// (test support, #215; not upstream) The node arena and the root's id, for the differential test's structure dump.
    #[doc(hidden)]
    pub fn debug_arena(&self) -> (&[AABTreeNodeClass], NodeId) {
        (&self.nodes, self.root_node)
    }

    /// (test support, #215; not upstream) `NodeCount`.
    #[doc(hidden)]
    pub fn debug_node_count(&self) -> i32 {
        self.node_count
    }

    /// (test support, #215; not upstream) `IndexedNodes[node_index]->UserData = user_data`, as the C++ wrapper does.
    #[doc(hidden)]
    pub fn debug_set_user_data(&mut self, node_index: i32, user_data: u32) {
        if node_index >= 0 && node_index < self.node_count {
            let id = self.indexed_nodes[node_index as usize];
            self.nodes[id].user_data = user_data;
        }
    }

    /// `~AABTreeCullSystemClass()`: "Delete all links and release-ref all cullables": every node's objects removed.
    pub fn shutdown(&mut self) {
        for nidx in 0..self.node_count as usize {
            let id = self.indexed_nodes[nidx];
            while let Some(o) = self.nodes[id].object.clone() {
                self.remove_object_internal(&o);
            }
        }
    }

    /// `Add_Object_Internal(obj, node_index)` (aabtreecull.cpp:182). `node_index == -1` drops the object into the tree;
    /// otherwise straight into that node without touching its bounds. Upstream warns (debug) when the object is already
    /// in another culling system.
    pub fn add_object_internal(&mut self, obj: &CullableHandle, node_index: i32) {
        assert!(obj.read().unwrap().get_culling_system().is_none(), "AABTreeCullSystemClass::Add -- Object is already in another culling system!");

        let new_link = AABTreeLinkClass::new(self.base.id);
        obj.write().unwrap().set_cull_link(Some(CullLinkClass::AABTree(new_link)));

        if node_index == -1 {
            let root = self.root_node;
            self.add_object_recursive(root, obj);
        } else {
            assert!(node_index < self.node_count);
            let id = self.indexed_nodes[node_index as usize];
            self.nodes[id].add_object(id, obj, false);
            self.object_count += 1;
        }
        // obj->Add_Ref(): the node's list holds a handle.
    }

    /// `Remove_Object_Internal(obj)` (aabtreecull.cpp:213).
    pub fn remove_object_internal(&mut self, obj: &CullableHandle) {
        assert!(obj.read().unwrap().get_culling_system() == Some(self.base.id));
        let node = get_link_node(obj).expect("link has a node");
        self.nodes[node].remove_object(obj);

        obj.write().unwrap().set_culling_system(None);
        obj.write().unwrap().set_cull_link(None);

        if !plant(11318) {
            self.object_count -= 1;
        }
        assert!(self.object_count >= 0);
        // obj->Release_Ref(): the node's handle is gone.
    }

    /// `Update_Culling(obj)` (aabtreecull.cpp:249): unlink from its node, then drop it into the tree again.
    pub fn update_culling(&mut self, obj: &CullableHandle) {
        assert!(obj.read().unwrap().get_culling_system() == Some(self.base.id));
        let node = get_link_node(obj).expect("link has a node");
        self.nodes[node].remove_object(obj);

        // decrement the object counter, the node can't decrement it for us...
        if !plant(11319) {
            self.object_count -= 1;
        }

        // drop it into the tree again
        let root = self.root_node;
        self.add_object_recursive(root, obj);
    }

    /// `Collect_Objects(const Vector3 &)`.
    pub fn collect_objects_point(&mut self, point: &Vector3) {
        let root = self.root_node;
        self.collect_objects_recursive_point(root, point);
    }

    /// `Collect_Objects(const AABoxClass &)`.
    pub fn collect_objects_aabox(&mut self, box_: &AABoxClass) {
        let root = self.root_node;
        self.collect_objects_recursive_aabox(root, box_);
    }

    /// `Collect_Objects(const OBBoxClass &)`.
    pub fn collect_objects_obbox(&mut self, box_: &OBBoxClass) {
        let root = self.root_node;
        self.collect_objects_recursive_obbox(root, box_);
    }

    /// `Collect_Objects(const FrustumClass &)`: starts with `planes_passed = 0`.
    pub fn collect_objects_frustum(&mut self, frustum: &FrustumClass) {
        let root = self.root_node;
        self.collect_objects_recursive_frustum(root, frustum, 0);
    }

    /// `Collect_Objects(const SphereClass &)`.
    pub fn collect_objects_sphere(&mut self, sphere: &SphereClass) {
        let root = self.root_node;
        self.collect_objects_recursive_sphere(root, sphere);
    }

    /// `Partition_Node_Count()`.
    pub fn partition_node_count(&self) -> i32 {
        self.partition_node_count_recursive(self.root_node)
    }

    /// `Partition_Tree_Depth()`.
    pub fn partition_tree_depth(&self) -> i32 {
        let mut max_depth = 0;
        self.partition_tree_depth_recursive(self.root_node, 0, &mut max_depth);
        max_depth
    }

    /// `Object_Count()`.
    pub fn object_count(&self) -> i32 {
        self.object_count
    }

    fn partition_node_count_recursive(&self, node: NodeId) -> i32 {
        let mut curcount = 1;
        if let Some(f) = self.nodes[node].front {
            curcount += self.partition_node_count_recursive(f);
        }
        if let Some(b) = self.nodes[node].back {
            curcount += self.partition_node_count_recursive(b);
        }
        curcount
    }

    fn partition_tree_depth_recursive(&self, node: NodeId, cur_depth: i32, max_depth: &mut i32) {
        let cur_depth = cur_depth + 1;
        if cur_depth > *max_depth {
            *max_depth = cur_depth;
        }
        if let Some(f) = self.nodes[node].front {
            self.partition_tree_depth_recursive(f, cur_depth, max_depth);
        }
        if let Some(b) = self.nodes[node].back {
            self.partition_tree_depth_recursive(b, cur_depth, max_depth);
        }
    }

    /// `Add_Object_Recursive(node, obj)` (aabtreecull.cpp:300): into the smaller child that contains the object's box,
    /// else the bigger, else this node.
    fn add_object_recursive(&mut self, node: NodeId, obj: &CullableHandle) {
        // order the children in terms of size
        let mut big_child = self.nodes[node].front;
        let mut small_child = self.nodes[node].back;

        if let (Some(b), Some(s)) = (big_child, small_child) {
            if if plant(11315) { self.nodes[b].compute_volume() > self.nodes[s].compute_volume() } else { self.nodes[b].compute_volume() < self.nodes[s].compute_volume() } {
                big_child = Some(s);
                small_child = Some(b);
            }
        }

        let cull_box = cull_box_of(obj);

        // Can we fit in the smaller child?
        if let Some(s) = small_child {
            if self.nodes[s].box_.contains_box(&cull_box) && !plant(11316) {
                self.add_object_recursive(s, obj);
                return;
            }
        }

        // Can we fit in the bigger child?
        if let Some(b) = big_child {
            if self.nodes[b].box_.contains_box(&cull_box) {
                self.add_object_recursive(b, obj);
                return;
            }
        }

        // Ok, we have to fit in this node.
        self.nodes[node].add_object(node, obj, true);
        self.object_count += 1;
    }

    /// `Add_Loaded_Object(node, obj)` (aabtreecull.cpp:341).
    pub fn add_loaded_object(&mut self, node: NodeId, obj: &CullableHandle) {
        assert!(obj.read().unwrap().get_culling_system().is_none(), "AABTreeCullSystemClass::Add_Loaded_Object -- Object is already in another culling system!");

        let new_link = AABTreeLinkClass::new(self.base.id);
        obj.write().unwrap().set_cull_link(Some(CullLinkClass::AABTree(new_link)));

        self.nodes[node].add_object(node, obj, true);
        self.object_count += 1;
    }

    /// `Re_Partition()` (aabtreecull.cpp:358): every object moves to a fresh root, which is partitioned.
    /// `rand` is C's `rand()`.
    pub fn re_partition(&mut self, rand: &mut dyn FnMut() -> i32) {
        // Unlink everything from the old tree into a dummy node that becomes the new root.
        let mut old = std::mem::take(&mut self.nodes);
        let mut dummy = AABTreeNodeClass::new();
        Self::transfer_objects(&mut old, 0, &mut dummy);
        drop(old);
        self.nodes = vec![dummy];
        self.root_node = 0;

        self.partition(0, rand);
        self.re_index_nodes();
        self.reset_statistics();
    }

    /// `Re_Partition(bounds, boxes)` (aabtreecull.cpp:395): a tree built from the given boxes; the objects are then
    /// dropped back in with `Update_Culling`.
    pub fn re_partition_boxes(&mut self, bounds: &AABoxClass, boxes: &mut Vec<AABoxClass>, rand: &mut dyn FnMut() -> i32) {
        let mut old = std::mem::take(&mut self.nodes);
        let mut dummy = AABTreeNodeClass::new();
        Self::transfer_objects(&mut old, 0, &mut dummy);
        drop(old);

        self.nodes = vec![AABTreeNodeClass::new()];
        self.root_node = 0;
        self.partition_boxes(0, bounds, boxes, rand);

        self.re_index_nodes();
        self.reset_statistics();

        // Re-add the objects the dummy node holds; its box must hold nothing so `Update_Culling` relinks every one.
        dummy.box_.extent.set(0.0, 0.0, 0.0);
        while let Some(obj) = dummy.object.clone() {
            // The object is still linked to the dummy node, which is not in the arena: unlink it by hand, as
            // `Update_Culling` would via `node->Remove_Object`, then do its `ObjectCount--` and re-add.
            dummy.remove_object(&obj);
            self.object_count -= 1;
            let root = self.root_node;
            self.add_object_recursive(root, &obj);
        }

        // the root box covers everything again
        if !plant(11326) {
            self.nodes[0].box_.extent.set(f32::MAX, f32::MAX, f32::MAX);
        }
    }

    /// `Update_Bounding_Boxes()`.
    pub fn update_bounding_boxes(&mut self) {
        let root = self.root_node;
        self.update_bounding_boxes_recursive(root);
    }

    /// `Get_Bounding_Box()`: the root's box.
    pub fn get_bounding_box(&self) -> &AABoxClass {
        &self.nodes[self.root_node].box_
    }

    /// `Get_Node_Bounds(node_id, &set_bounds)`: the root's box for an out-of-range id.
    pub fn get_node_bounds(&self, node_id: i32) -> AABoxClass {
        if node_id >= 0 && node_id < self.node_count {
            self.nodes[self.indexed_nodes[node_id as usize]].box_
        } else {
            self.nodes[self.indexed_nodes[0]].box_
        }
    }

    /// `Reset_Statistics()`.
    pub fn reset_statistics(&mut self) {
        self.stats.node_count = self.node_count;
        self.stats.nodes_accepted = 0;
        self.stats.nodes_trivially_accepted = 0;
        self.stats.nodes_rejected = 0;
    }

    /// `Get_Statistics()`.
    pub fn get_statistics(&self) -> &StatsStruct {
        &self.stats
    }

    // NODE_ACCEPTED / NODE_TRIVIALLY_ACCEPTED / NODE_REJECTED count only under WWDEBUG; in release they are empty.

    /// `Collect_Objects_Recursive(node)`: everything under the node is in.
    fn collect_objects_recursive(&mut self, node: NodeId) {
        let mut obj = self.nodes[node].object.clone();
        while let Some(o) = obj {
            self.base.add_to_collection(&o);
            obj = get_next_object(&o);
        }

        // NODE_TRIVIALLY_ACCEPTED();
        if plant(11310) {
            if let Some(f) = self.nodes[node].front {
                self.collect_objects_recursive(f);
            }
            if let Some(b) = self.nodes[node].back {
                self.collect_objects_recursive(b);
            }
            return;
        }
        if let Some(b) = self.nodes[node].back {
            self.collect_objects_recursive(b);
        }
        if let Some(f) = self.nodes[node].front {
            self.collect_objects_recursive(f);
        }
    }

    fn collect_objects_recursive_point(&mut self, node: NodeId, point: &Vector3) {
        if !self.nodes[node].box_.contains_point(point) {
            // NODE_REJECTED();
            return;
        }
        // NODE_ACCEPTED();

        let mut obj = self.nodes[node].object.clone();
        while let Some(o) = obj {
            if plant(11311) || cull_box_of(&o).contains_point(point) {
                self.base.add_to_collection(&o);
            }
            obj = get_next_object(&o);
        }

        if let Some(b) = self.nodes[node].back {
            self.collect_objects_recursive_point(b, point);
        }
        if let Some(f) = self.nodes[node].front {
            self.collect_objects_recursive_point(f, point);
        }
    }

    fn collect_objects_recursive_aabox(&mut self, node: NodeId, box_: &AABoxClass) {
        let overlap = overlap_test_aabox_aabox(box_, &self.nodes[node].box_);
        if overlap == OverlapType::OUTSIDE {
            return;
        } else if overlap == OverlapType::INSIDE && !plant(11312) {
            self.collect_objects_recursive(node);
            return;
        }

        let mut obj = self.nodes[node].object.clone();
        while let Some(o) = obj {
            if overlap_test_aabox_aabox(box_, &cull_box_of(&o)) != OverlapType::OUTSIDE {
                self.base.add_to_collection(&o);
            }
            obj = get_next_object(&o);
        }

        if let Some(b) = self.nodes[node].back {
            self.collect_objects_recursive_aabox(b, box_);
        }
        if let Some(f) = self.nodes[node].front {
            self.collect_objects_recursive_aabox(f, box_);
        }
    }

    fn collect_objects_recursive_obbox(&mut self, node: NodeId, box_: &OBBoxClass) {
        let overlap = overlap_test_obbox_aabox(box_, &self.nodes[node].box_);
        if overlap == OverlapType::OUTSIDE {
            return;
        } else if overlap == OverlapType::INSIDE {
            self.collect_objects_recursive(node);
            return;
        }

        let mut obj = self.nodes[node].object.clone();
        while let Some(o) = obj {
            if overlap_test_obbox_aabox(box_, &cull_box_of(&o)) != OverlapType::OUTSIDE {
                self.base.add_to_collection(&o);
            }
            obj = get_next_object(&o);
        }

        if let Some(b) = self.nodes[node].back {
            self.collect_objects_recursive_obbox(b, box_);
        }
        if let Some(f) = self.nodes[node].front {
            self.collect_objects_recursive_obbox(f, box_);
        }
    }

    /// `Collect_Objects_Recursive(node, frustum, planes_passed)`: `planes_passed` is by value and updated by the node
    /// test, so each child starts from the parent's value after that test.
    fn collect_objects_recursive_frustum(&mut self, node: NodeId, frustum: &FrustumClass, planes_passed: i32) {
        let mut planes_passed = planes_passed;
        let overlap = overlap_test_frustum_aabox_planes_passed(frustum, &self.nodes[node].box_, &mut planes_passed);
        if overlap == OverlapType::OUTSIDE {
            return;
        } else if overlap == OverlapType::INSIDE {
            self.collect_objects_recursive(node);
            return;
        }

        let mut obj = self.nodes[node].object.clone();
        while let Some(o) = obj {
            if if plant(11313) { overlap_test_frustum_aabox(frustum, &cull_box_of(&o)) == OverlapType::INSIDE } else { overlap_test_frustum_aabox(frustum, &cull_box_of(&o)) != OverlapType::OUTSIDE } {
                self.base.add_to_collection(&o);
            }
            obj = get_next_object(&o);
        }

        if let Some(b) = self.nodes[node].back {
            self.collect_objects_recursive_frustum(b, frustum, planes_passed);
        }
        if let Some(f) = self.nodes[node].front {
            self.collect_objects_recursive_frustum(f, frustum, planes_passed);
        }
    }

    fn collect_objects_recursive_sphere(&mut self, node: NodeId, sphere: &SphereClass) {
        if overlap_test_aabox_sphere(&self.nodes[node].box_, sphere) == OverlapType::OUTSIDE {
            return;
        }

        let mut obj = self.nodes[node].object.clone();
        while let Some(o) = obj {
            if overlap_test_aabox_sphere(&(if plant(11314) { self.nodes[node].box_ } else { cull_box_of(&o) }), sphere) != OverlapType::OUTSIDE {
                self.base.add_to_collection(&o);
            }
            obj = get_next_object(&o);
        }

        if let Some(b) = self.nodes[node].back {
            self.collect_objects_recursive_sphere(b, sphere);
        }
        if let Some(f) = self.nodes[node].front {
            self.collect_objects_recursive_sphere(f, sphere);
        }
    }

    /// `Update_Bounding_Boxes_Recursive(node)`: the node's box grown to hold its children and its objects.
    fn update_bounding_boxes_recursive(&mut self, node: NodeId) {
        let mut minmaxbox = MinMaxAABoxClass::from_aabox(&self.nodes[node].box_);

        if let Some(f) = self.nodes[node].front {
            self.update_bounding_boxes_recursive(f);
            let fb = MinMaxAABoxClass::from_aabox(&self.nodes[f].box_);
            minmaxbox.add_min_max_box(&fb);
        }
        if let Some(b) = self.nodes[node].back {
            self.update_bounding_boxes_recursive(b);
            let bb = MinMaxAABoxClass::from_aabox(&self.nodes[b].box_);
            minmaxbox.add_min_max_box(&bb);
        }

        let mut obj = self.nodes[node].object.clone();
        while let Some(o) = obj {
            let ob = cull_box_of(&o);
            if !plant(11320) {
                minmaxbox.add_box(&ob);
            }
            obj = get_next_object(&o);
        }

        self.nodes[node].box_.init_min_max(&minmaxbox.min_corner, &minmaxbox.max_corner);
    }

    /// `Load(cload)` (aabtreecull.cpp:470): "Remove all objects from AAB-Culling system before loading!". A missing or
    /// obsolete version chunk returns with the tree emptied (upstream deleted the old root first).
    pub fn load(&mut self, cload: &mut ChunkLoad<'_>) -> ChunkIoResult<()> {
        assert!(self.object_count() == 0, "Remove all objects from AAB-Culling system before loading!");

        self.nodes = vec![AABTreeNodeClass::new()];
        self.root_node = 0;

        // The first chunk should be a version chunk
        cload.open_chunk()?;
        if cload.cur_chunk_id()? != AABTREE_CHUNK_VERSION {
            // WWDEBUG_SAY(("Attempting to read an obsolete AAB-Tree!"));
            cload.close_chunk()?;
            return Ok(());
        }

        // read in the version and verify that it is the current version
        let mut vb = [0u8; 4];
        cload.read_into(&mut vb)?;
        let version = u32::from_le_bytes(vb);
        if version != AABTREE_CURRENT_VERSION {
            // WWDEBUG_SAY(("Attempting to read an obsolete AAB-Tree!"));
            cload.close_chunk()?;
            return Ok(());
        }
        cload.close_chunk()?;

        // read in the tree
        let root = self.root_node;
        self.load_nodes(root, cload)?;

        // re-index all nodes
        self.re_index_nodes();

        // reset the statistics
        self.reset_statistics();
        Ok(())
    }

    /// `Load_Nodes(node, cload)` (aabtreecull.cpp:512): the node description (legacy `_INFO` chunk or the newer
    /// `_VARIABLES` micro chunks), the contents, then the front and back children.
    fn load_nodes(&mut self, node: NodeId, cload: &mut ChunkLoad<'_>) -> ChunkIoResult<()> {
        // Open the node description
        cload.open_chunk()?;
        assert!(cload.cur_chunk_id()? == AABTREE_CHUNK_AABNODE);

        let mut node_desc = IOAABNodeStruct::default();

        cload.open_chunk()?;
        if cload.cur_chunk_id()? == AABTREE_CHUNK_AABNODE_INFO {
            // Loading the legacy format...
            let mut b = [0u8; 28];
            cload.read_into(&mut b)?;
            node_desc = IOAABNodeStruct::from_bytes(&b);
        } else if cload.cur_chunk_id()? == AABTREE_CHUNK_AABNODE_VARIABLES {
            // Loading the new format, contains micro chunks...
            while cload.open_micro_chunk()? {
                match cload.cur_micro_chunk_id() {
                    AABTREE_VARIABLE_NODESTRUCT => {
                        let mut b = [0u8; 28];
                        cload.read_into(&mut b)?;
                        node_desc = IOAABNodeStruct::from_bytes(&b);
                    }
                    AABTREE_VARIABLE_USERDATA => {
                        let mut b = [0u8; 4];
                        cload.read_into(&mut b)?;
                        self.nodes[node].user_data = u32::from_le_bytes(b);
                    }
                    _ => {}
                }
                cload.close_micro_chunk()?;
            }
        }
        cload.close_chunk()?;

        // Initialize the node bounds.
        self.nodes[node].box_.center.x = node_desc.center[0];
        self.nodes[node].box_.center.y = node_desc.center[1];
        self.nodes[node].box_.center.z = node_desc.center[2];
        self.nodes[node].box_.extent.x = node_desc.extent[0];
        self.nodes[node].box_.extent.y = node_desc.extent[1];
        self.nodes[node].box_.extent.z = node_desc.extent[2];

        // Load the contents of the node (`Load_Node_Contents` is a no-op in the base class).
        cload.open_chunk()?;
        assert!(cload.cur_chunk_id()? == AABTREE_CHUNK_AABNODE_CONTENTS);
        cload.close_chunk()?;

        // Close the node description
        cload.close_chunk()?;

        // if we are supposed to have a front child, load it
        if node_desc.attributes & AABNODE_ATTRIBUTE_FRONT_CHILD != 0 {
            assert!(self.nodes[node].front.is_none());
            let child = self.new_child(node);
            self.nodes[node].front = Some(child);
            self.load_nodes(child, cload)?;
        }

        // if we have a back child, load it
        if node_desc.attributes & AABNODE_ATTRIBUTE_BACK_CHILD != 0 {
            assert!(self.nodes[node].back.is_none());
            let child = self.new_child(node);
            self.nodes[node].back = Some(child);
            self.load_nodes(child, cload)?;
        }
        Ok(())
    }

    /// A new node in the arena with `Parent = parent` (`new AABTreeNodeClass; node->Parent = this`).
    fn new_child(&mut self, parent: NodeId) -> NodeId {
        let id = self.nodes.len();
        let mut n = AABTreeNodeClass::new();
        n.parent = Some(parent);
        self.nodes.push(n);
        id
    }

    /// `Save(csave)`: the version chunk, then the nodes.
    pub fn save(&self, csave: &mut ChunkSaveClass) {
        csave.begin_chunk(AABTREE_CHUNK_VERSION);
        let version = AABTREE_CURRENT_VERSION;
        csave.write(&version.to_le_bytes());
        csave.end_chunk();

        self.save_nodes(self.root_node, csave);
    }

    /// `Save_Nodes(node, csave)`.
    fn save_nodes(&self, node: NodeId, csave: &mut ChunkSaveClass) {
        let n = &self.nodes[node];
        csave.begin_chunk(AABTREE_CHUNK_AABNODE);

        csave.begin_chunk(AABTREE_CHUNK_AABNODE_VARIABLES);

        let mut node_desc = IOAABNodeStruct::default();
        node_desc.center = [n.box_.center.x, n.box_.center.y, n.box_.center.z];
        node_desc.extent = [n.box_.extent.x, n.box_.extent.y, n.box_.extent.z];

        if n.front.is_some() {
            node_desc.attributes |= AABNODE_ATTRIBUTE_FRONT_CHILD;
        }
        if n.back.is_some() {
            node_desc.attributes |= AABNODE_ATTRIBUTE_BACK_CHILD;
        }

        csave.write_micro_chunk(AABTREE_VARIABLE_NODESTRUCT, &node_desc.to_bytes());
        csave.write_micro_chunk(AABTREE_VARIABLE_USERDATA, &n.user_data.to_le_bytes());
        csave.end_chunk();

        csave.begin_chunk(AABTREE_CHUNK_AABNODE_CONTENTS);
        // Save_Node_Contents(node, csave): a no-op in the base class
        csave.end_chunk();

        csave.end_chunk();

        if let Some(f) = n.front {
            self.save_nodes(f, csave);
        }
        if let Some(b) = n.back {
            self.save_nodes(b, csave);
        }
    }

    /// `Load_Object_Linkage(cload, obj)` (aabtreecull.cpp:696).
    pub fn load_object_linkage(&mut self, cload: &mut ChunkLoad<'_>, obj: &CullableHandle) -> ChunkIoResult<()> {
        cload.open_chunk()?;
        assert!(cload.cur_chunk_id()? == AABTREE_CHUNK_NODE_INDEX);
        let mut b = [0u8; 4];
        cload.read_into(&mut b)?;
        let index = u32::from_le_bytes(b);
        cload.close_chunk()?;

        self.add_object_internal(obj, index as i32);
        Ok(())
    }

    /// `Save_Object_Linkage(csave, obj)` (aabtreecull.cpp:712).
    pub fn save_object_linkage(&self, csave: &mut ChunkSaveClass, obj: &CullableHandle) {
        assert!(obj.read().unwrap().get_culling_system() == Some(self.base.id));
        let node = get_link_node(obj).expect("link has a node");
        let index = self.nodes[node].index;

        csave.begin_chunk(AABTREE_CHUNK_NODE_INDEX);
        csave.write(&index.to_le_bytes());
        csave.end_chunk();
    }

    /// `Re_Index_Nodes()`: "NodeCount > 0".
    fn re_index_nodes(&mut self) {
        self.node_count = self.partition_node_count();
        assert!(self.node_count > 0);

        self.indexed_nodes = vec![0; self.node_count as usize];
        let mut counter = 0;
        let root = self.root_node;
        self.re_index_nodes_recursive(root, &mut counter);
        assert!(counter == self.node_count);
    }

    fn re_index_nodes_recursive(&mut self, node: NodeId, counter: &mut i32) {
        self.nodes[node].index = *counter as u32;
        self.indexed_nodes[*counter as usize] = node;
        *counter += 1;

        if plant(11327) {
            if let Some(b) = self.nodes[node].back {
                self.re_index_nodes_recursive(b, counter);
            }
            if let Some(f) = self.nodes[node].front {
                self.re_index_nodes_recursive(f, counter);
            }
            return;
        }
        if let Some(f) = self.nodes[node].front {
            self.re_index_nodes_recursive(f, counter);
        }
        if let Some(b) = self.nodes[node].back {
            self.re_index_nodes_recursive(b, counter);
        }
    }

    /// `Compute_Bounding_Box()`: children first, then the node's own box.
    pub fn compute_bounding_box(&mut self, node: NodeId) {
        if let Some(f) = self.nodes[node].front {
            self.compute_bounding_box(f);
        }
        if let Some(b) = self.nodes[node].back {
            self.compute_bounding_box(b);
        }
        self.compute_local_bounding_box(node);
    }

    /// `Compute_Local_Bounding_Box()`: from the children's boxes and the node's objects'.
    pub fn compute_local_bounding_box(&mut self, node: NodeId) {
        let mut box_ = MinMaxAABoxClass::new(Vector3::new(f32::MAX, f32::MAX, f32::MAX), Vector3::new(-f32::MAX, -f32::MAX, -f32::MAX));

        if let Some(f) = self.nodes[node].front {
            let fb = self.nodes[f].box_;
            box_.add_box(&fb);
        }
        if let Some(b) = self.nodes[node].back {
            let bb = self.nodes[b].box_;
            box_.add_box(&bb);
        }

        let mut obj = self.nodes[node].object.clone();
        while let Some(o) = obj {
            box_.add_box(&cull_box_of(&o));
            obj = get_next_object(&o);
        }

        self.nodes[node].box_.init_min_max_box(&box_);
    }

    /// `AABTreeNodeClass::Transfer_Objects(dummy_node)` (aabtreecull.cpp:660) over the old arena: every object is
    /// unlinked and re-added (bounds updated) to `dummy_node`, which becomes arena id 0 of the new tree.
    fn transfer_objects(old: &mut Vec<AABTreeNodeClass>, node: NodeId, dummy_node: &mut AABTreeNodeClass) {
        // unlink all of our objects, relinking them to the dummy_node
        while let Some(obj) = old[node].object.clone() {
            old[node].remove_object(&obj);
            dummy_node.add_object(0, &obj, true);
        }

        // do the same with our children
        if let Some(f) = old[node].front {
            Self::transfer_objects(old, f, dummy_node);
        }
        if let Some(b) = old[node].back {
            Self::transfer_objects(old, b, dummy_node);
        }
    }

    /// `AABTreeNodeClass::Partition()` (aabtreecull.cpp:737): a leaf with more than two objects is split on a plane
    /// chosen by `Select_Splitting_Plane`, recursively.
    fn partition(&mut self, node: NodeId, rand: &mut dyn FnMut() -> i32) {
        // Don't partition if we only have 2 or less objects
        let obj_count = self.nodes[node].object_count();
        if if plant(11321) { obj_count < 2 } else { obj_count <= 2 } {
            return;
        }

        // Gather the cull boxes of the objects
        let mut boxes: Vec<AABoxClass> = Vec::with_capacity(obj_count as usize);
        let mut obj = self.nodes[node].object.clone();
        while let Some(o) = obj {
            boxes.push(cull_box_of(&o));
            obj = get_next_object(&o);
        }

        // Pick a splitting plane; no valid split ends the recursion
        let mut sc = SplitChoiceStruct::new();
        Self::select_splitting_plane(&mut sc, &boxes, rand);
        boxes.clear();

        if sc.cost == f32::MAX {
            return;
        }

        // Create the child nodes and split the objects between them
        let front_id = self.nodes.len();
        self.nodes.push(AABTreeNodeClass::new());
        let back_id = self.nodes.len();
        self.nodes.push(AABTreeNodeClass::new());
        self.split_objects(node, &sc, front_id, back_id);

        if self.nodes[front_id].object_count() > 0 {
            self.nodes[node].front = Some(front_id);
            self.nodes[front_id].parent = Some(node);
            self.partition(front_id, rand);
        }
        // (an empty child is deleted upstream; here its arena slot stays unreferenced)

        if self.nodes[back_id].object_count() > 0 {
            self.nodes[node].back = Some(back_id);
            self.nodes[back_id].parent = Some(node);
            self.partition(back_id, rand);
        }
    }

    /// `AABTreeNodeClass::Split_Objects(sc, front, back)` (aabtreecull.cpp:802): "assumes that this node is a leaf".
    fn split_objects(&mut self, node: NodeId, sc: &SplitChoiceStruct, front: NodeId, back: NodeId) {
        assert!(self.nodes[node].front.is_none());
        assert!(self.nodes[node].back.is_none());
        assert!(self.nodes[node].object_count() == sc.front_count + sc.back_count);

        let mut fcount = 0;
        let mut bcount = 0;

        // unlink all of our objects, relinking them to the appropriate node
        while let Some(obj) = self.nodes[node].object.clone() {
            // pull the object out of this node
            self.nodes[node].remove_object(&obj);

            // decide which node to add the object to, NOTE: we have to use the same convention as Compute_Score!
            let box_ = cull_box_of(&obj);
            if overlap_test_aaplane_point(&sc.plane, &box_.center) == OverlapType::FRONT {
                self.nodes[front].add_object(front, &obj, true);
                fcount += 1;
            } else {
                self.nodes[back].add_object(back, &obj, true);
                bcount += 1;
            }
        }

        // copy the bounding boxes
        self.nodes[front].box_ = AABoxClass::from_min_max_box(&sc.front_box);
        if !plant(11324) {
            self.nodes[front].box_.extent += Vector3::new(wwmath::WWMATH_EPSILON, wwmath::WWMATH_EPSILON, wwmath::WWMATH_EPSILON);
        }

        self.nodes[back].box_ = AABoxClass::from_min_max_box(&sc.back_box);
        self.nodes[back].box_.extent += Vector3::new(wwmath::WWMATH_EPSILON, wwmath::WWMATH_EPSILON, wwmath::WWMATH_EPSILON);

        // when we are all done, the counts should match.
        assert!(fcount == sc.front_count);
        assert!(bcount == sc.back_count);
    }

    /// `AABTreeNodeClass::Partition(bounds, boxes)` (aabtreecull.cpp:845): partitions a set of boxes (no objects).
    fn partition_boxes(&mut self, node: NodeId, bounds: &AABoxClass, boxes: &mut Vec<AABoxClass>, rand: &mut dyn FnMut() -> i32) {
        self.nodes[node].box_ = *bounds;

        // Don't partition if we only have 1 or less boxes
        if boxes.len() <= (if plant(11325) { 2 } else { 1 }) {
            return;
        }

        // Try to find a splitting plane
        let mut sc = SplitChoiceStruct::new();
        Self::select_splitting_plane(&mut sc, boxes, rand);

        if sc.cost == f32::MAX {
            return;
        }

        // Split the boxes between the two sides
        let mut frontboxes: Vec<AABoxClass> = Vec::with_capacity(sc.front_count as usize);
        let mut backboxes: Vec<AABoxClass> = Vec::with_capacity(sc.back_count as usize);
        Self::split_boxes(&sc, boxes, &mut frontboxes, &mut backboxes);
        boxes.clear();

        if !frontboxes.is_empty() {
            let id = self.new_child(node);
            self.nodes[node].front = Some(id);
            let fb = AABoxClass::from_min_max_box(&sc.front_box);
            self.partition_boxes(id, &fb, &mut frontboxes, rand);
        } else {
            self.nodes[node].front = None;
        }

        if !backboxes.is_empty() {
            let id = self.new_child(node);
            self.nodes[node].back = Some(id);
            let bb = AABoxClass::from_min_max_box(&sc.back_box);
            self.partition_boxes(id, &bb, &mut backboxes, rand);
        } else {
            self.nodes[node].back = None;
        }
    }

    /// `AABTreeNodeClass::Split_Boxes` (aabtreecull.cpp:897).
    fn split_boxes(sc: &SplitChoiceStruct, boxes: &Vec<AABoxClass>, frontboxes: &mut Vec<AABoxClass>, backboxes: &mut Vec<AABoxClass>) {
        assert!(boxes.len() as i32 == sc.front_count + sc.back_count);

        // copy each box in the input array into the appropriate output array
        for box_ in boxes.iter() {
            if overlap_test_aaplane_point(&sc.plane, &box_.center) == OverlapType::FRONT {
                frontboxes.push(*box_);
            } else {
                backboxes.push(*box_);
            }
        }

        // when we are all done, the counts should match.
        assert!(frontboxes.len() as i32 == sc.front_count);
        assert!(backboxes.len() as i32 == sc.back_count);
    }

    /// `AABTreeNodeClass::Select_Splitting_Plane` (aabtreecull.cpp:931): up to 300 random candidate planes through a
    /// random box's min or max face, scored with `Compute_Score`; the best is kept in `sc`. If none scored (every try had
    /// an empty side) the brute-force search runs.
    fn select_splitting_plane(sc: &mut SplitChoiceStruct, boxes: &[AABoxClass], rand: &mut dyn FnMut() -> i32) {
        const NUM_TRYS: i32 = 300;

        let objcount = boxes.len() as i32;
        let limit = std::cmp::min(NUM_TRYS, objcount);
        let mut trys = 0;
        while trys < limit {
            let mut test = SplitChoiceStruct::new();

            // select a random box and a random side of it
            let obj_index = (rand() % objcount) as usize;
            let box_ = &boxes[obj_index];

            match rand() % 6 {
                0 => test.plane.set(AxisEnum::XNormal, box_.center.x + box_.extent.x),
                1 => test.plane.set(AxisEnum::XNormal, if plant(11322) { box_.center.x + box_.extent.x } else { box_.center.x - box_.extent.x }),
                2 => test.plane.set(AxisEnum::YNormal, box_.center.y + box_.extent.y),
                3 => test.plane.set(AxisEnum::YNormal, box_.center.y - box_.extent.y),
                4 => test.plane.set(AxisEnum::ZNormal, box_.center.z + box_.extent.z),
                5 => test.plane.set(AxisEnum::ZNormal, box_.center.z - box_.extent.z),
                _ => {}
            }

            // evaluate it, keep the best
            Self::compute_score(&mut test, boxes);
            if test.cost < sc.cost {
                *sc = test;
            }
            trys += 1;
        }

        // If we didn't find a valid split, brute-force it
        if trys >= limit && sc.cost == f32::MAX {
            Self::select_splitting_plane_brute_force(sc, boxes);
        }
    }

    /// `Select_Splitting_Plane_Brute_Force` (aabtreecull.cpp:991): every box, six planes each. UPSTREAM DEFECT KEPT:
    /// plane indices 1..5 use `box.Center` where `box.Extent` was meant.
    fn select_splitting_plane_brute_force(sc: &mut SplitChoiceStruct, boxes: &[AABoxClass]) {
        for box_ in boxes.iter() {
            for plane_index in 0..6 {
                let mut test = SplitChoiceStruct::new();

                match plane_index % 6 {
                    0 => test.plane.set(AxisEnum::XNormal, box_.center.x + box_.extent.x),
                    1 => test.plane.set(AxisEnum::XNormal, if plant(11329) { box_.center.x - box_.extent.x } else { box_.center.x - box_.center.y }),
                    2 => test.plane.set(AxisEnum::YNormal, box_.center.y + box_.center.y),
                    3 => test.plane.set(AxisEnum::YNormal, box_.center.y - box_.center.y),
                    4 => test.plane.set(AxisEnum::ZNormal, box_.center.z + box_.center.z),
                    5 => test.plane.set(AxisEnum::ZNormal, box_.center.z - box_.center.z),
                    _ => {}
                }

                test.front_box.init_empty();
                test.back_box.init_empty();

                Self::compute_score(&mut test, boxes);
                if test.cost < sc.cost {
                    *sc = test;
                }
            }
        }
        // (upstream prints "Unable to split node!" under WWDEBUG when the cost is still FLT_MAX)
    }

    /// `AABTreeNodeClass::Compute_Score` (aabtreecull.cpp:1023): the cost is the sum of each side's box volume times its
    /// count; a split with an empty side costs `FLT_MAX`.
    fn compute_score(sc: &mut SplitChoiceStruct, boxes: &[AABoxClass]) {
        for box_ in boxes.iter() {
            if overlap_test_aaplane_point(&sc.plane, &box_.center) == OverlapType::FRONT {
                sc.front_count += 1;
                sc.front_box.add_box(box_);
            } else {
                sc.back_count += 1;
                sc.back_box.add_box(box_);
            }
        }

        let back_cost = if plant(11323) { sc.back_box.volume() } else { sc.back_box.volume() * sc.back_count as f32 };
        let front_cost = sc.front_box.volume() * sc.front_count as f32;

        sc.cost = front_cost + back_cost;

        if sc.front_count == 0 || sc.back_count == 0 {
            sc.cost = f32::MAX;
        }
    }
}

impl Default for AABTreeCullSystemClass {
    fn default() -> Self {
        AABTreeCullSystemClass::new()
    }
}

impl Drop for AABTreeCullSystemClass {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// `class AABTreeIterator`: walks the node tree by `Index`. Upstream holds the tree pointer; here every call takes the
/// tree (no lifetime parameters).
#[derive(Clone, Copy, Debug, Default)]
pub struct AABTreeIterator {
    cur_node_index: i32,
}

impl AABTreeIterator {
    /// `AABTreeIterator(tree)`: starts at the root.
    pub fn new() -> Self {
        AABTreeIterator { cur_node_index: 0 }
    }

    /// `Reset()`.
    pub fn reset(&mut self) {
        self.cur_node_index = 0;
    }

    /// `validate()`: "if out of range, back to the root".
    fn validate(&mut self, tree: &AABTreeCullSystemClass) {
        if self.cur_node_index < 0 || self.cur_node_index >= tree.node_count {
            self.cur_node_index = 0;
        }
    }

    fn node<'a>(&self, tree: &'a AABTreeCullSystemClass) -> &'a AABTreeNodeClass {
        &tree.nodes[tree.indexed_nodes[self.cur_node_index as usize]]
    }

    /// `Enter_Parent()`.
    pub fn enter_parent(&mut self, tree: &AABTreeCullSystemClass) -> bool {
        self.validate(tree);
        if self.cur_node_index != 0 && !(plant(11330) && self.cur_node_index == 1) {
            let parent = self.node(tree).parent.expect("non-root has a parent");
            self.cur_node_index = tree.nodes[parent].index as i32;
            return true;
        }
        false
    }

    /// `Enter_Sibling()`.
    pub fn enter_sibling(&mut self, tree: &AABTreeCullSystemClass) -> bool {
        self.validate(tree);
        if self.cur_node_index != 0 {
            let parent = self.node(tree).parent.expect("non-root has a parent");
            let parent_front = tree.nodes[parent].front;
            let parent_back = tree.nodes[parent].back;

            // "if the parent does not have both children, there is no sibling"
            let (pf, pb) = match (parent_front, parent_back) {
                (Some(f), Some(b)) => (f, b),
                _ => return false,
            };

            if tree.nodes[pf].index as i32 == self.cur_node_index {
                self.cur_node_index = tree.nodes[if plant(11328) { pf } else { pb }].index as i32;
                return true;
            }
            if tree.nodes[pb].index as i32 == self.cur_node_index {
                self.cur_node_index = tree.nodes[pf].index as i32;
                return true;
            }
        }
        false
    }

    /// `Has_Front_Child()`.
    pub fn has_front_child(&mut self, tree: &AABTreeCullSystemClass) -> bool {
        self.validate(tree);
        self.node(tree).front.is_some()
    }

    /// `Enter_Front_Child()`.
    pub fn enter_front_child(&mut self, tree: &AABTreeCullSystemClass) -> bool {
        self.validate(tree);
        if self.has_front_child(tree) {
            let f = self.node(tree).front.unwrap();
            self.cur_node_index = tree.nodes[f].index as i32;
            return true;
        }
        false
    }

    /// `Has_Back_Child()`.
    pub fn has_back_child(&mut self, tree: &AABTreeCullSystemClass) -> bool {
        self.validate(tree);
        self.node(tree).back.is_some()
    }

    /// `Enter_Back_Child()` (upstream does not call `validate()` first here).
    pub fn enter_back_child(&mut self, tree: &AABTreeCullSystemClass) -> bool {
        if self.has_back_child(tree) {
            let b = self.node(tree).back.unwrap();
            self.cur_node_index = tree.nodes[b].index as i32;
            return true;
        }
        false
    }

    /// `Get_Current_Node_Index()`.
    pub fn get_current_node_index(&self) -> i32 {
        self.cur_node_index
    }

    /// `Get_Current_Box(&set_box)`: `Tree->Get_Node_Bounds(CurNodeIndex, set_box)`.
    pub fn get_current_box(&self, tree: &AABTreeCullSystemClass) -> AABoxClass {
        tree.get_node_bounds(self.cur_node_index)
    }
}
