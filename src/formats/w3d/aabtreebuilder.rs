// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtreebuilder.h (AABTreeBuilderClass, its CullNodeStruct,
//     SplitChoiceStruct, SplitArraysStruct, OverlapType, MIN_POLYS_PER_NODE, SMALL_VERTEX, BIG_VERTEX)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtreebuilder.cpp (COINCIDENCE_EPSILON, line 69; Build_AABTree,
//     line 153; Build_Tree, line 284; Select_Splitting_Plane, line 375; Compute_Plane_Score, line 435; Which_Side, line
//     502; Split_Polys, line 567; Compute_Bounding_Box, line 626; Assign_Index, line 699; Node_Count, line 735;
//     Poly_Count, line 757; Node_Count_Recursive, line 774; Update_Min_Max, line 851)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtree.cpp (AABTreeClass::AABTreeClass(AABTreeBuilderClass *),
//     line 113; Build_Tree_Recursive, line 261) and aabtree.h (CullNodeStruct's Set_Front_Child, Set_Back_Child,
//     Set_Poly0, Set_Poly_Count, lines 330-352)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshgeometry.cpp (MeshGeometryClass::Generate_Culling_Tree, line
//     1548) and meshmdlio.cpp (MeshModelClass::Load_W3D's "If this mesh is collideable and no AABTree was in the file,
//     generate one now", lines 400-406; the same test in MeshGeometryClass::Load_W3D, meshgeometry.cpp:1683-1688)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aaplane.h (AAPlaneClass, its AxisEnum and Set)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D2 and WWMath, by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #190: `AABTreeBuilderClass`, which builds the AABTree a collidable mesh gets when its W3D file carried none
//! (`MeshGeometryClass::Generate_Culling_Tree`), and `AABTreeClass(AABTreeBuilderClass *)`, which turns the builder's
//! tree into the node and polygon-index arrays the cast reads ([`CullTree`], `crate::client::mesh_cast`).
//!
//! # As upstream does it
//!
//! - **When** ([`load_w3d_generates_cull_tree`]): `MeshModelClass::Load_W3D` (meshmdlio.cpp:400-406), after the
//!   chunks and the pre-3.0 bone fix-up and before `install_materials` and `post_process`, builds one when the mesh's
//!   `W3dAttributes` have a collision bit (`W3D_MESH_FLAG_COLLISION_TYPE_MASK`) and no AABTree chunk was read.
//!   `post_process` then drops it again for a skin (meshmdlio.cpp:1655), as it drops a loaded one.
//! - **`Build_AABTree`** ([`AabTreeBuilder::build_aabtree`], the `TriIndex` overload `Generate_Culling_Tree` calls):
//!   copies the mesh's polygons and vertices, `Build_Tree` from all the polygon indices in order, then
//!   `Compute_Bounding_Box` and `Assign_Index` from the root.
//! - **`Build_Tree`**: a node with `MIN_POLYS_PER_NODE` (4) polygons or fewer is a leaf. Otherwise
//!   `Select_Splitting_Plane` tries `MIN(50, polycount)` axis-aligned planes, each through one vertex (`rand() % 3`) of
//!   one polygon (`polyindices[rand() % polycount]`) on one axis (`rand() % 3`), and keeps the first with the lowest
//!   cost (`<`). `Compute_Plane_Score`: each polygon goes front (`Which_Side` `FRONT`, `ON` or `BOTH`) or back
//!   (`BACK`); the cost is the sum over the two sides of the box volume times the polygon count, the back box
//!   inflated by `WWMATH_EPSILON` on each side (the front box is not), and `FLT_MAX` when either side is empty. When no
//!   plane beat `FLT_MAX` (the counts then add to 0), the node is a leaf. Else `Split_Polys` partitions the indices,
//!   keeping their order, and the front then the back child are built.
//! - **`Which_Side`**: a vertex is `POS` when `point[axis] - Dist > COINCIDENCE_EPSILON` (0.001), `NEG` when below
//!   `-COINCIDENCE_EPSILON`; the triangle is `ON` if no vertex was either, `POS` if none was `NEG`, `NEG` if none was
//!   `POS`, else `BOTH`. (`ON` is or'ed in for every vertex, so `mask == ON` means "neither".)
//! - **`Compute_Bounding_Box`** (children first): from `WWMATH_FLOAT_MAX` and `-WWMATH_FLOAT_MAX`, the node's own
//!   polygons (only leaves have any), then the front and back children's boxes, all by `<` and `>` (not min/max).
//! - **`Assign_Index`**: preorder, front before back. **`AABTreeClass(builder)`** and `Build_Tree_Recursive`: node
//!   `Index` gets its box; an inner node `Set_Front_Child` and `Set_Back_Child`, a leaf `Set_Poly0(curpolyindex)`
//!   (with `AABTREE_LEAF_FLAG`) and `Set_Poly_Count`; the polygon indices are laid out in the same preorder.
//!
//! # `rand()`
//!
//! The split choice draws from the C runtime's `rand()`, three draws per plane tried. The game is built with MSVC,
//! whose `rand()` is a linear congruential generator on a per-thread `_holdrand` (`_holdrand = _holdrand * 214013 +
//! 2531011; return (_holdrand >> 16) & 0x7fff;`, the CRT's rand.c; not EA code) that starts at 1 and that `srand`
//! sets. [`CrtRand`] is that generator. Upstream's state when a mesh is loaded is whatever every earlier `rand()` call
//! in the process left (WWMath's `Random_Float`, `Quaternion`'s random, Snow.cpp, the shatter system, other meshes'
//! trees); nothing in the game seeds it except W3DVolumetricShadow's debug path. **That state is not reproduced here**
//! (#190): [`MeshModel::from_mesh`](super::meshmdl::MeshModel::from_mesh) builds each tree from a fresh [`CrtRand`]
//! at the CRT's initial state (1), so a mesh's tree is the one upstream builds when the mesh is the first thing to
//! draw from `rand()`. Another state gives another (equally valid) tree: the same hits, except where two polygons tie
//! on `t` or the AABTree cull rejects a ray on a node box's boundary (#149's measurement).
//!
//! # Where this differs from upstream
//!
//! - A polygon whose vertex index is past the vertex array (upstream reads past `Verts`, undefined) makes
//!   [`AabTreeBuilder::build_aabtree`] give no tree (the mesh is then cast at by brute force).
//! - `Build_Tree`, `Compute_Bounding_Box`, `Assign_Index` and `Build_Tree_Recursive` are recursive upstream; here they
//!   walk an explicit stack in the same order (a mesh of many polygons on one plane splits one polygon at a time).
//! - `AABTreeBuilderClass::Export` and `Build_W3D_AABTree_Recursive` (the tools' path to a W3D chunk) are not
//!   translated: the game builds `AABTreeClass` from the builder directly. The differential test builds both upstream
//!   and checks the chunk's arrays equal [`CullTree`]'s.

use crate::client::mesh_cast::AABTREE_LEAF_FLAG;
use crate::formats::w3d::meshmdl::CullTree;
use crate::formats::w3d::mesh::AabNode;
use crate::formats::w3d::w3d_file::{W3D_MESH_FLAG_COLLISION_TYPE_MASK, W3D_MESH_FLAG_COLLISION_TYPE_SHIFT};
use crate::logic::modules::plant;
use crate::ww::wwmath::{WWMATH_EPSILON, WWMATH_FLOAT_MAX};

/// `const float COINCIDENCE_EPSILON = 0.001f;` (aabtreebuilder.cpp:69).
pub const COINCIDENCE_EPSILON: f32 = 0.001;
/// `MIN_POLYS_PER_NODE` (aabtreebuilder.h).
pub const MIN_POLYS_PER_NODE: i32 = 4;
/// `SMALL_VERTEX` (aabtreebuilder.h), as the `float` `SplitChoiceStruct` gives its maxima.
const SMALL_VERTEX: f32 = -100000.0;
/// `BIG_VERTEX` (aabtreebuilder.h), as the `float` `SplitChoiceStruct` gives its minima.
const BIG_VERTEX: f32 = 100000.0;
/// `Select_Splitting_Plane`'s `const int NUM_TRYS = 50;` (aabtreebuilder.cpp:379).
const NUM_TRYS: i32 = 50;

/// The MSVC C runtime's `rand()` (rand.c; not EA code, see the module docs): `_holdrand = _holdrand * 214013L +
/// 2531011L; return (_holdrand >> 16) & 0x7fff;`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrtRand {
    /// `_holdrand`.
    pub holdrand: u32,
}

impl Default for CrtRand {
    /// The CRT's initial state: `_holdrand` is 1 until `srand`.
    fn default() -> Self {
        CrtRand { holdrand: 1 }
    }
}

impl CrtRand {
    /// `srand(seed)`.
    pub fn new(seed: u32) -> Self {
        CrtRand { holdrand: seed }
    }

    /// `rand()`: 0 to 32767.
    pub fn rand(&mut self) -> i32 {
        self.holdrand = self.holdrand.wrapping_mul(214013).wrapping_add(2531011);
        let shift = if plant(10371) { 15 } else { 16 };
        ((self.holdrand >> shift) & 0x7fff) as i32
    }
}

/// `AAPlaneClass` (aaplane.h): `Normal` (`XNORMAL` 0, `YNORMAL` 1, `ZNORMAL` 2) and `Dist`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AaPlane {
    pub normal: usize,
    pub dist: f32,
}

/// `AABTreeBuilderClass::OverlapType` (aabtreebuilder.h): `POS` (`FRONT`), `NEG` (`BACK`), `ON`, `BOTH`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlapType {
    Pos = 0x01,
    Neg = 0x02,
    On = 0x04,
    Both = 0x08,
}

/// The `switch` of `Compute_Plane_Score` and `Split_Polys` (aabtreebuilder.cpp:447-464, 592-603): `FRONT`, `ON` and
/// `BOTH` go front, `BACK` back. (Plants 10375 and 10380 change both switches alike, so the counts still agree.)
fn goes_front(side: OverlapType) -> bool {
    match side {
        OverlapType::Pos => true,
        OverlapType::On => !plant(10375),
        OverlapType::Both => !plant(10380),
        OverlapType::Neg => false,
    }
}

/// `AABTreeBuilderClass::SplitChoiceStruct`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitChoice {
    /// `Cost` ("try to minimize this!").
    pub cost: f32,
    pub front_count: i32,
    pub back_count: i32,
    pub bmin: [f32; 3],
    pub bmax: [f32; 3],
    pub fmin: [f32; 3],
    pub fmax: [f32; 3],
    pub plane: AaPlane,
}

impl Default for SplitChoice {
    /// `SplitChoiceStruct()`: `Cost(FLT_MAX)`, no polygons, the boxes inverted at `BIG_VERTEX`/`SMALL_VERTEX`, the plane
    /// `(XNORMAL, 0)`.
    fn default() -> Self {
        SplitChoice {
            cost: f32::MAX,
            front_count: 0,
            back_count: 0,
            bmin: [BIG_VERTEX; 3],
            bmax: [SMALL_VERTEX; 3],
            fmin: [BIG_VERTEX; 3],
            fmax: [SMALL_VERTEX; 3],
            plane: AaPlane { normal: 0, dist: 0.0 },
        }
    }
}

/// `AABTreeBuilderClass::CullNodeStruct`: `Front` and `Back` are indices into [`AabTreeBuilder::nodes`].
#[derive(Clone, Debug, PartialEq)]
pub struct BuilderNode {
    pub index: i32,
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub front: Option<usize>,
    pub back: Option<usize>,
    /// `PolyIndices` (`PolyCount` of them).
    pub poly_indices: Vec<i32>,
}

impl BuilderNode {
    /// `CullNodeStruct()`: all zero, no children, no polygons.
    fn new() -> Self {
        BuilderNode { index: 0, min: [0.0; 3], max: [0.0; 3], front: None, back: None, poly_indices: Vec::new() }
    }
}

/// `class AABTreeBuilderClass`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AabTreeBuilder {
    /// The nodes (`Root` is node 0 once built).
    pub nodes: Vec<BuilderNode>,
    /// `Polys` (`PolyCount` of them).
    pub polys: Vec<[u16; 3]>,
    /// `Verts` (`VertCount` of them).
    pub verts: Vec<[f32; 3]>,
}

impl AabTreeBuilder {
    /// `AABTreeBuilderClass::Build_AABTree(polycount, polys, vertcount, verts)` (aabtreebuilder.cpp:153), drawing the
    /// split planes from `rng`. `false` (nothing built) when a polygon's vertex index is past `verts` (undefined
    /// upstream).
    pub fn build_aabtree(&mut self, polys: &[[u16; 3]], verts: &[[f32; 3]], rng: &mut CrtRand) -> bool {
        // Reset(); "Copy the mesh data"
        self.nodes.clear();
        if polys.iter().any(|p| p.iter().any(|&v| usize::from(v) >= verts.len())) {
            return false;
        }
        self.polys = polys.to_vec();
        self.verts = verts.to_vec();
        // "First, create a list of all of the poly indices"
        let polyindices: Vec<i32> = (0..polys.len() as i32).collect();
        // Root = new CullNodeStruct; Build_Tree(Root, PolyCount, polyindices);
        self.nodes.push(BuilderNode::new());
        self.build_tree(0, polyindices, rng);
        // "fill in the remaining information needed in the tree"
        self.compute_bounding_box(0);
        self.assign_index(0, 0);
        true
    }

    /// `AABTreeBuilderClass::Build_Tree(node, polycount, polyindices)` (aabtreebuilder.cpp:284), the recursion as a stack
    /// in the same order (a node, then its whole front subtree, then its back one), so `rand()` is drawn in upstream's
    /// order.
    fn build_tree(&mut self, root: usize, polyindices: Vec<i32>, rng: &mut CrtRand) {
        let mut stack = vec![(root, polyindices)];
        while let Some((node, polyindices)) = stack.pop() {
            let polycount = polyindices.len() as i32;
            // "First, if there are only a few polys left, just terminate the tree"
            let few = if plant(10379) { polycount < MIN_POLYS_PER_NODE } else { polycount <= MIN_POLYS_PER_NODE };
            if few {
                self.nodes[node].poly_indices = polyindices;
                continue;
            }
            // "Try to find a suitable partitioning plane."
            let sc = self.select_splitting_plane(&polyindices, rng);
            // "If the algorithm could not separate any polys, just install the polys in this node and terminate."
            if sc.front_count + sc.back_count != polycount {
                self.nodes[node].poly_indices = polyindices;
                continue;
            }
            // "Ok, split the polys"
            let (front_polys, back_polys) = self.split_polys(&polyindices, &sc);
            let mut todo = Vec::with_capacity(2);
            // "Build a front tree if necessary."
            if !front_polys.is_empty() {
                self.nodes.push(BuilderNode::new());
                let f = self.nodes.len() - 1;
                self.nodes[node].front = Some(f);
                todo.push((f, front_polys));
            }
            // "Build a back tree if necessary."
            if !back_polys.is_empty() {
                self.nodes.push(BuilderNode::new());
                let b = self.nodes.len() - 1;
                self.nodes[node].back = Some(b);
                todo.push((b, back_polys));
            }
            // the front subtree is built (and draws its planes) before the back one
            while let Some(t) = todo.pop() {
                stack.push(t);
            }
        }
    }

    /// `AABTreeBuilderClass::Select_Splitting_Plane(polycount, polyindices)` (aabtreebuilder.cpp:375): "Try putting
    /// axis-aligned planes through some random vertices".
    pub fn select_splitting_plane(&self, polyindices: &[i32], rng: &mut CrtRand) -> SplitChoice {
        let polycount = polyindices.len() as i32;
        let mut best_plane_stats = SplitChoice::default();
        let trys_max = if plant(10373) { NUM_TRYS } else { NUM_TRYS.min(polycount) };
        for _trys in 0..trys_max {
            // "Select a random poly and vertex index;"
            let (poly_index, vert_index) = if plant(10374) {
                let vi = rng.rand() % 3;
                (polyindices[(rng.rand() % polycount) as usize], vi)
            } else {
                let pi = polyindices[(rng.rand() % polycount) as usize];
                (pi, rng.rand() % 3)
            };
            let vert = self.verts[usize::from(self.polys[poly_index as usize][vert_index as usize])];
            // "Select a random plane"
            let axis = (rng.rand() % 3) as usize;
            let plane = AaPlane { normal: axis, dist: vert[axis] };
            // "Get the score for this plane"
            let considered_plane_stats = self.compute_plane_score(polyindices, &plane);
            // (plant 10372 keeps a later plane of equal cost, but never a one-sided one: that would split forever)
            let tie = plant(10372) && considered_plane_stats.cost == best_plane_stats.cost && considered_plane_stats.cost < f32::MAX;
            let better = considered_plane_stats.cost < best_plane_stats.cost || tie;
            if better {
                best_plane_stats = considered_plane_stats;
            }
        }
        best_plane_stats
    }

    /// `AABTreeBuilderClass::Compute_Plane_Score(polycount, polyindices, plane)` (aabtreebuilder.cpp:435): "the volumes
    /// of the resulting two children volumes, the number of polys in each child volume".
    pub fn compute_plane_score(&self, polyindices: &[i32], plane: &AaPlane) -> SplitChoice {
        let mut sc = SplitChoice { plane: *plane, ..SplitChoice::default() };
        for &pi in polyindices {
            if goes_front(self.which_side(plane, pi)) {
                sc.front_count += 1;
                self.update_min_max(pi, &mut sc.fmin, &mut sc.fmax);
            } else {
                sc.back_count += 1;
                self.update_min_max(pi, &mut sc.bmin, &mut sc.bmax);
            }
        }
        // "Inflate the box a tiny amount so that we never get volumes of zero!"
        if !plant(10376) {
            for a in 0..3 {
                sc.bmin[a] -= WWMATH_EPSILON;
                sc.bmax[a] += WWMATH_EPSILON;
            }
        }
        if plant(10377) {
            for a in 0..3 {
                sc.fmin[a] -= WWMATH_EPSILON;
                sc.fmax[a] += WWMATH_EPSILON;
            }
        }
        // "Compute the cost."
        let back_cost = (sc.bmax[0] - sc.bmin[0]) * (sc.bmax[1] - sc.bmin[1]) * (sc.bmax[2] - sc.bmin[2]) * sc.back_count as f32;
        let front_cost = (sc.fmax[0] - sc.fmin[0]) * (sc.fmax[1] - sc.fmin[1]) * (sc.fmax[2] - sc.fmin[2]) * sc.front_count as f32;
        sc.cost = front_cost + back_cost;
        if sc.front_count == 0 || sc.back_count == 0 {
            sc.cost = f32::MAX;
        }
        sc
    }

    /// `AABTreeBuilderClass::Which_Side(plane, poly_index)` (aabtreebuilder.cpp:502).
    pub fn which_side(&self, plane: &AaPlane, poly_index: i32) -> OverlapType {
        // "Check each vertex to see if it is in front, behind or on the plane"
        let mut mask = 0u32;
        for vi in 0..3 {
            let point = self.verts[usize::from(self.polys[poly_index as usize][vi])];
            let delta = point[plane.normal] - plane.dist;
            let (pos, neg) = if plant(10378) {
                (delta >= COINCIDENCE_EPSILON, delta <= -COINCIDENCE_EPSILON)
            } else {
                (delta > COINCIDENCE_EPSILON, delta < -COINCIDENCE_EPSILON)
            };
            if pos {
                mask |= OverlapType::Pos as u32;
            }
            if neg {
                mask |= OverlapType::Neg as u32;
            }
            mask |= OverlapType::On as u32;
        }
        let (pos, neg, on) = (OverlapType::Pos as u32, OverlapType::Neg as u32, OverlapType::On as u32);
        // "If all verts were ON the plane, the triangle is ON the plane"
        if mask == on {
            return OverlapType::On;
        }
        // "If all verts were POS or ON, the triangle is POS (IN_FRONT)"
        if mask & !(pos | on) == 0 {
            return OverlapType::Pos;
        }
        // "If all verts were NEG or ON, the triangle is NEG (BEHIND)"
        if mask & !(neg | on) == 0 {
            return OverlapType::Neg;
        }
        // "Otherwise, the triangle spans the plane"
        OverlapType::Both
    }

    /// `AABTreeBuilderClass::Split_Polys(polycount, polyindices, sc, arrays)` (aabtreebuilder.cpp:567): the front and
    /// back arrays, each in the input's order.
    fn split_polys(&self, polyindices: &[i32], sc: &SplitChoice) -> (Vec<i32>, Vec<i32>) {
        let mut front = Vec::with_capacity(sc.front_count as usize);
        let mut back = Vec::with_capacity(sc.back_count as usize);
        for &pi in polyindices {
            if goes_front(self.which_side(&sc.plane, pi)) {
                front.push(pi);
            } else {
                back.push(pi);
            }
        }
        (front, back)
    }

    /// `AABTreeBuilderClass::Compute_Bounding_Box(node)` (aabtreebuilder.cpp:626), children first (a postorder stack).
    fn compute_bounding_box(&mut self, root: usize) {
        let mut order = Vec::new();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            order.push(n);
            if let Some(f) = self.nodes[n].front {
                stack.push(f);
            }
            if let Some(b) = self.nodes[n].back {
                stack.push(b);
            }
        }
        // every child comes after its parent in `order`: done in reverse, each node's children are done before it
        for &n in order.iter().rev() {
            // "compute bounding volume for the polys in this node"
            let really_big = WWMATH_FLOAT_MAX;
            let mut min = [really_big; 3];
            let mut max = [-really_big; 3];
            for &pi in &self.nodes[n].poly_indices {
                self.update_min_max(pi, &mut min, &mut max);
            }
            // "bound the polys in the front child node", then the back's
            let mut children = vec![self.nodes[n].front];
            if !plant(10381) {
                children.push(self.nodes[n].back);
            }
            for c in children.into_iter().flatten() {
                let (cmin, cmax) = (self.nodes[c].min, self.nodes[c].max);
                for a in 0..3 {
                    if cmin[a] < min[a] {
                        min[a] = cmin[a];
                    }
                    if cmax[a] > max[a] {
                        max[a] = cmax[a];
                    }
                }
            }
            self.nodes[n].min = min;
            self.nodes[n].max = max;
        }
    }

    /// `AABTreeBuilderClass::Assign_Index(node, index)` (aabtreebuilder.cpp:699): "assign a sequential index to each
    /// node", preorder, front before back. Returns the next index.
    fn assign_index(&mut self, root: usize, mut index: i32) -> i32 {
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            self.nodes[n].index = index;
            index += 1;
            let (f, b) = (self.nodes[n].front, self.nodes[n].back);
            let (first, second) = if plant(10383) { (b, f) } else { (f, b) };
            if let Some(s) = second {
                stack.push(s);
            }
            if let Some(s) = first {
                stack.push(s);
            }
        }
        index
    }

    /// `AABTreeBuilderClass::Node_Count()` (aabtreebuilder.cpp:735, `Node_Count_Recursive`): every node.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// `AABTreeBuilderClass::Poly_Count()` (aabtreebuilder.cpp:757): `PolyCount`.
    pub fn poly_count(&self) -> usize {
        self.polys.len()
    }

    /// `AABTreeBuilderClass::Update_Min_Max(poly_index, min, max)` (aabtreebuilder.cpp:851): each vertex, each axis,
    /// `if (point < min) min = point; if (point > max) max = point;`.
    fn update_min_max(&self, poly_index: i32, min: &mut [f32; 3], max: &mut [f32; 3]) {
        for vert_index in 0..3 {
            let point = self.verts[usize::from(self.polys[poly_index as usize][vert_index])];
            for a in 0..3 {
                let lower = point[a] < min[a];
                if lower {
                    min[a] = point[a];
                }
                if point[a] > max[a] && !(plant(10382) && lower) {
                    max[a] = point[a];
                }
            }
        }
    }

    /// `AABTreeClass::AABTreeClass(AABTreeBuilderClass * builder)` (aabtree.cpp:113) and `Build_Tree_Recursive`
    /// (aabtree.cpp:261): `Nodes[node->Index]` gets the node's box; an inner node `Set_Front_Child(Front->Index)` and
    /// `Set_Back_Child(Back->Index)`, a leaf `Set_Poly0(curpolyindex)` (with `AABTREE_LEAF_FLAG`) and
    /// `Set_Poly_Count(PolyCount)`; the polygon indices copied in the same preorder.
    pub fn to_cull_tree(&self) -> CullTree {
        let zero = AabNode { min: [0.0; 3], max: [0.0; 3], front_or_poly0: 0, back_or_poly_count: 0 };
        let mut nodes = vec![zero; self.node_count()];
        let mut poly_indices = vec![0u32; self.poly_count()];
        let mut curpolyindex = 0usize;
        let mut stack = if self.nodes.is_empty() { Vec::new() } else { vec![0usize] };
        while let Some(n) = stack.pop() {
            let node = &self.nodes[n];
            let newnode = &mut nodes[node.index as usize];
            newnode.min = node.min;
            newnode.max = node.max;
            // "If this is a non-leaf node, set up the child indices, otherwise set up the polygon indices"
            if let Some(f) = node.front {
                // WWASSERT(node->Back != NULL): a split always gives both
                newnode.front_or_poly0 = self.nodes[f].index as u32;
                newnode.back_or_poly_count = node.back.map_or(0, |b| self.nodes[b].index as u32);
                if plant(10384) {
                    std::mem::swap(&mut newnode.front_or_poly0, &mut newnode.back_or_poly_count);
                }
            } else {
                newnode.front_or_poly0 = curpolyindex as u32 | AABTREE_LEAF_FLAG;
                newnode.back_or_poly_count = node.poly_indices.len() as u32;
            }
            // "Copy the polygon indices for this node into our array"
            for &p in &node.poly_indices {
                poly_indices[curpolyindex] = p as u32;
                curpolyindex += 1;
            }
            // "Install the children": front, then back
            if let Some(b) = node.back {
                stack.push(b);
            }
            if let Some(f) = node.front {
                stack.push(f);
            }
        }
        CullTree { nodes, poly_indices }
    }
}

/// `MeshGeometryClass::Generate_Culling_Tree()` (meshgeometry.cpp:1548): `builder.Build_AABTree(PolyCount, Poly,
/// VertexCount, Vertex)`, `CullTree = NEW_REF(AABTreeClass, (&builder))`. `None` where [`AabTreeBuilder::build_aabtree`]
/// builds nothing.
pub fn generate_culling_tree(polys: &[[u16; 3]], verts: &[[f32; 3]], rng: &mut CrtRand) -> Option<CullTree> {
    let mut builder = AabTreeBuilder::default();
    builder.build_aabtree(polys, verts, rng).then(|| builder.to_cull_tree())
}

/// `MeshModelClass::Load_W3D`'s test (meshmdlio.cpp:400-406; `MeshGeometryClass::Load_W3D`'s, meshgeometry.cpp:1683,
/// is the same): "If this mesh is collideable and no AABTree was in the file, generate one now", `(((W3dAttributes &
/// W3D_MESH_FLAG_COLLISION_TYPE_MASK) >> W3D_MESH_FLAG_COLLISION_TYPE_SHIFT) != 0) && (CullTree == NULL)`.
pub fn load_w3d_generates_cull_tree(w3d_attributes: u32, has_cull_tree: bool) -> bool {
    let collides = if plant(10385) { true } else { (w3d_attributes & W3D_MESH_FLAG_COLLISION_TYPE_MASK) >> W3D_MESH_FLAG_COLLISION_TYPE_SHIFT != 0 };
    collides && !has_cull_tree
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CRT's first values from its initial state (1): 41, 18467, 6334, 26500 (MSVC's well-known sequence).
    #[test]
    fn crt_rand_sequence() {
        let mut r = CrtRand::default();
        assert_eq!([r.rand(), r.rand(), r.rand(), r.rand()], [41, 18467, 6334, 26500]);
    }

    /// Ten unit triangles along x, two to a cell: more than `MIN_POLYS_PER_NODE`, so the root splits; every polygon is
    /// in exactly one leaf, the root box bounds them all, and the indices are preorder.
    #[test]
    fn builds_a_tree_over_every_polygon() {
        let mut verts = Vec::new();
        let mut polys = Vec::new();
        for i in 0..10u16 {
            let x = f32::from(i) * 2.0;
            verts.extend([[x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x, 1.0, 0.0]]);
            polys.push([3 * i, 3 * i + 1, 3 * i + 2]);
        }
        let t = generate_culling_tree(&polys, &verts, &mut CrtRand::default()).expect("a tree");
        assert!(t.nodes.len() >= 3);
        assert_eq!(t.nodes[0].min, [0.0, 0.0, 0.0]);
        assert_eq!(t.nodes[0].max, [19.0, 1.0, 0.0]);
        assert_eq!(t.nodes[0].front_or_poly0, 1);
        let mut seen = t.poly_indices.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..10).collect::<Vec<u32>>());
        // an index past the vertices builds nothing
        assert!(generate_culling_tree(&[[0, 1, 30]], &verts, &mut CrtRand::default()).is_none());
        assert!(load_w3d_generates_cull_tree(0x0000_0010, false));
        assert!(!load_w3d_generates_cull_tree(0x0000_0010, true));
        assert!(!load_w3d_generates_cull_tree(0x0000_100F, false));
    }
}
