// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/mesh.cpp (MeshClass::Cast_Ray, line 1218; MeshClass::Init's collision
//     type, lines 1168-1169)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshgeometry.cpp (MeshGeometryClass::Cast_Ray, line 650;
//     cast_ray_brute_force, line 1089; Compute_Plane_Equations, line 1260; Get_Plane_Array, line 1497)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtree.h (AABTreeClass::Cast_Ray, line 231; CullNodeStruct's
//     accessors, lines 301-328) and aabtree.cpp (Cast_Ray_Recursive, line 527; Cast_Ray_To_Polys, line 747)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/coltest.h (RayCollisionTestClass(raytest, tm), Cull, lines 160-171)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/lineseg.cpp (LineSegClass::Set(that, tm), line 58)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathline.cpp (CollisionMath::Collide(LineSegClass, TriClass),
//     line 192) and colmathline.h (Overlap_Test(min, max, LineSegClass), line 47)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathaabox.cpp (Overlap_Test(AABoxClass, LineSegClass), line 144)
//     and colmathaabox.h (Overlap_Test(AABoxClass, Vector3), line 66)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/aabox.h (AABoxClass::Init_Min_Max, line 339)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   (Westwood Studios, WW3D2 and WWMath)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #149: `MeshClass::Cast_Ray`, the ray cast at a mesh's **polygons** that `HLodClass::Cast_Ray` reaches for each
//! mesh model (`crate::client::pick_drawable`).
//!
//! # As upstream does it
//!
//! - **`MeshClass::Cast_Ray(raytest)`** (mesh.cpp:1218, [`MeshRenderObj::cast_ray`]): nothing unless the mesh's
//!   collision type has a bit of the test's, it is not alpha when the test checks translucency, not hidden (unless the
//!   test checks hidden ones), not hidden by the animation and the result is not `StartBad`. Then the ray is taken into
//!   the mesh's frame (`Get_Transform()`, turned to face the ray for an `ALIGNED` mesh or its start for an `ORIENTED`
//!   one by `Obj_Look_At`, inverted by `Get_Inverse`, [`LineSeg::transformed`]) and cast at the model; a hit's normal
//!   (and contact point) go back to the world.
//! - **`MeshGeometryClass::Cast_Ray`** (meshgeometry.cpp:650, [`MeshGeometry::cast_ray`]): through the AABTree when the
//!   mesh has one (`AABTreeClass::Cast_Ray`, [`cast_ray_tree`]: from node 0, a node the ray's box test culls
//!   (`Overlap_Test(min, max, line) == POS`, [`overlap_test_min_max_line`]) is skipped, a leaf casts at its polygons in
//!   `PolyIndices` order, an inner node at its front then its back child, all of them), else at every polygon in order
//!   (`cast_ray_brute_force`). Each polygon is a `TriClass` ([`TriClass`]) whose normal is computed from its vertices:
//!   the tree path by `TriClass::Compute_Normal` (`OPTIMIZE_PLANEEQ_RAM` is 1), the brute-force path by
//!   `Compute_Plane_Equations` (the same products in the same order), and `CollisionMath::Collide(line, tri, result)`
//!   ([`collide_line_tri`]) keeps the nearest hit (`t < Fraction`), with the hit polygon's surface type.
//! - **`Collide(LineSegClass, TriClass)`** (colmathline.cpp:192): the segment against the triangle's plane (parallel:
//!   no hit; `t` outside 0..1: no hit), then `TriClass::Contains_Point` on the plane the normal is most perpendicular
//!   to. Both faces collide: there is no backface test.
//!
//! # What the client gives (where this differs from upstream)
//!
//! - **The mesh's transform** is the client's: the sub-object's bone transform in the drawn model's live hierarchy (a
//!   lone mesh: the drawn root). A **skin** is cast at as upstream casts at it: its own vertices (the bone-space ones
//!   `get_deformed_vertices` moves) under its bone's transform, **not** its deformed vertices.
//! - ~~**No generated AABTree.** A collidable mesh the file gave no AABTree gets one upstream (`Generate_Culling_Tree`,
//!   `AABTreeBuilderClass`, not translated); here it is cast at by brute force. The hit is the same polygon unless two
//!   polygons tie on `t` (the first in the tree's order wins there, the first in the mesh's here).~~ **Corrected
//!   2026-11-17 (#190):** a collidable mesh the file gave no AABTree gets one, as upstream
//!   (`crate::formats::w3d::aabtreebuilder`, built by `MeshModel::from_mesh`), and is cast through it. **The tree's
//!   split planes** are drawn from the C runtime's `rand()`, whose state upstream is the process's; here each tree is
//!   built from the runtime's initial state, so where upstream's state differs its tree can too (the hit then differs
//!   only on ties on `t` and on rays the node boxes' cull rejects at a boundary).
//! - **An AABTree or polygon index out of range** (undefined behaviour upstream: a read past the array) is no hit here.
//! - `Is_Alpha` (`IS_ALPHA`, set by `MeshClass::Init` from the shaders) and `Is_Animation_Hidden` are the client's to
//!   give ([`MeshRenderObj::is_alpha`], [`MeshRenderObj::animation_hidden`]).

use std::sync::Arc;

use crate::client::pick_drawable::{coll_type, set_collision_type, CastResult, LineSeg, RayTest};
use crate::formats::w3d::mesh::mesh_flags;
use crate::formats::w3d::meshmdl::{CullTree, MeshModel};
use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::tri::TriClass;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `CollisionMath::OverlapType` (colmath.h:105).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlapType {
    /// `POS` (`OUTSIDE`, `FRONT`).
    Pos = 0x01,
    /// `NEG` (`INSIDE`, `BACK`).
    Neg = 0x02,
    /// `ON`.
    On = 0x04,
    /// `BOTH` (`OVERLAPPED`).
    Both = 0x08,
}

/// `CollisionMath::Overlap_Test(AABoxClass box, Vector3 point)` (colmathaabox.h:66): `POS` when the point is outside
/// the box on an axis (`Fabs(point - Center) > Extent`), else `NEG`.
pub fn overlap_test_aabox_point(center: &Vector3, extent: &Vector3, point: &Vector3) -> OverlapType {
    let outside = |d: f32, e: f32| if plant(10361) { wwmath::fabs(d) >= e } else { wwmath::fabs(d) > e };
    if outside(point.x - center.x, extent.x) {
        return OverlapType::Pos;
    }
    if outside(point.y - center.y, extent.y) {
        return OverlapType::Pos;
    }
    if outside(point.z - center.z, extent.z) {
        return OverlapType::Pos;
    }
    OverlapType::Neg
}

/// `CollisionMath::Overlap_Test(AABoxClass box, LineSegClass line)` (colmathaabox.cpp:144, the one compiled; the
/// alternate below it is `#if 0`): `INSIDE` when both ends are in the box, `OVERLAPPED` when one is, else the separating
/// axis test on the three axes and the three crosses of each axis with the line's direction.
pub fn overlap_test_aabox_line(center: &Vector3, extent: &Vector3, line: &LineSeg) -> OverlapType {
    // "If both endpoints are in, return INSIDE / If either was inside, return OVERLAPPED"
    let mut inside_point_count = 0;
    if overlap_test_aabox_point(center, extent, &line.p0) == OverlapType::Neg {
        inside_point_count += 1;
    }
    if overlap_test_aabox_point(center, extent, &line.p1) == OverlapType::Neg {
        inside_point_count += 1;
    }
    if inside_point_count == 2 {
        return OverlapType::Neg;
    } else if inside_point_count == 1 {
        return OverlapType::Both;
    }
    // "Here I'm using the separating axis theorem to test if the line-segment and the box can be separated by a plane."
    let dp0 = Vector3::subtract_vectors(&line.p0, center);
    // "Project box and line onto the x-axis" (then y, then z): "The projection of 'dp' is only counted if it points
    // towards the center of the box"
    for i in 0..3 {
        let box_proj = extent[i];
        let p0_proj = dp0[i];
        let dp_proj = line.dp[i];
        if p0_proj > 0.0f32 {
            if p0_proj > box_proj - wwmath::min(dp_proj, 0.0f32) {
                return OverlapType::Pos;
            }
        } else if -p0_proj > box_proj + wwmath::max(dp_proj, 0.0f32) {
            return OverlapType::Pos;
        }
    }
    if plant(10360) {
        return OverlapType::Both;
    }
    let dir = &line.dir;
    // "Project box and line onto (x cross line)": "(1,0,0) cross (x,y,z)"
    let axis = Vector3::new(0.0, -dir.z, dir.y);
    let box_proj = wwmath::fabs(axis.y * extent.y) + wwmath::fabs(axis.z * extent.z);
    let p0_proj = Vector3::dot_product(&axis, &dp0);
    if wwmath::fabs(p0_proj) > box_proj {
        return OverlapType::Pos;
    }
    // "Project box and line onto (y cross line)": "(0,1,0) cross (x,y,z)"
    let axis = Vector3::new(dir.z, 0.0, -dir.x);
    let box_proj = wwmath::fabs(axis.x * extent.x) + wwmath::fabs(axis.z * extent.z);
    let p0_proj = Vector3::dot_product(&axis, &dp0);
    if wwmath::fabs(p0_proj) > box_proj {
        return OverlapType::Pos;
    }
    // "Project box and line onto (z cross line)": "(0,0,1) cross (x,y,z)"
    let axis = Vector3::new(-dir.y, dir.x, 0.0);
    let box_proj = wwmath::fabs(axis.x * extent.x) + wwmath::fabs(axis.y * extent.y);
    let p0_proj = Vector3::dot_product(&axis, &dp0);
    if wwmath::fabs(p0_proj) > box_proj {
        return OverlapType::Pos;
    }
    OverlapType::Both
}

/// `CollisionMath::Overlap_Test(min, max, LineSegClass line)` (colmathline.h:47): the box `Init_Min_Max(min, max)`
/// (aabox.h:339: `Center = (max + min) * 0.5f`, `Extent = (max - min) * 0.5f`), then [`overlap_test_aabox_line`].
pub fn overlap_test_min_max_line(min: &Vector3, max: &Vector3, line: &LineSeg) -> OverlapType {
    let center = (*max + *min) * 0.5f32;
    let extent = (*max - *min) * 0.5f32;
    overlap_test_aabox_line(&center, &extent, line)
}

/// `CollisionMath::Collide(LineSegClass line, TriClass tri, CastResultStruct * res)` (colmathline.cpp:192).
pub fn collide_line_tri(line: &LineSeg, tri: &TriClass, res: &mut CastResult) -> bool {
    // "Compute intersection of the line with the plane of the polygon": PlaneClass(*tri.N, *tri.V[0])
    let plane_n = tri.n;
    let plane_d = Vector3::dot_product(&tri.n, &tri.v[0]);
    let den = Vector3::dot_product(&plane_n, &line.dp);
    // "If the denominator is zero, the ray is parallel to the plane"
    if den == 0.0f32 {
        return false;
    }
    let num = -(Vector3::dot_product(&plane_n, &line.p0) - plane_d);
    let t = num / den;
    // "If t is not between 0 and 1, the line containing the segment intersects the plane but the segment does not"
    if (t < 0.0f32) || (t > 1.0f32 && !plant(10356)) {
        return false;
    }
    let ipoint = line.p0 + t * line.dp;
    // "Check if this point is inside the triangle"
    if !tri.contains_point(&ipoint) {
        return false;
    }
    // "Ok, we hit the triangle, set the collision results"
    let nearer = if plant(10357) { t <= res.fraction } else { t < res.fraction };
    if nearer {
        res.fraction = t;
        res.normal = plane_n;
        if res.compute_contact_point {
            res.contact_point = line.p0 + res.fraction * line.dp;
        }
        return true;
    }
    false
}

impl LineSeg {
    /// `LineSegClass(that, tm)` / `LineSegClass::Set(that, tm)` (lineseg.cpp:58): both ends transformed
    /// (`Transform_Vector`), `DP = P1 - P0`, the direction rotated (`Rotate_Vector`), the length kept.
    pub fn transformed(that: &LineSeg, tm: &Matrix3D) -> LineSeg {
        let p0 = Matrix3D::transform_vector(tm, &that.p0);
        let p1 = Matrix3D::transform_vector(tm, &that.p1);
        let dp = p1 - p0;
        let dir = if plant(10362) {
            let mut d = dp;
            d.normalize();
            d
        } else {
            Matrix3D::rotate_vector_by(tm, &that.dir)
        };
        LineSeg { p0, p1, dp, dir, length: that.length }
    }
}

impl RayTest {
    /// `RayCollisionTestClass::Cull(min, max)` (coltest.h:168): whether the box is wholly outside the ray
    /// (`Overlap_Test(min, max, Ray) == POS`).
    pub fn cull(&self, min: &Vector3, max: &Vector3) -> bool {
        overlap_test_min_max_line(min, max, &self.ray) == OverlapType::Pos
    }
}

fn v3(a: [f32; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

/// `AABTREE_LEAF_FLAG` (aabtree.h:76).
pub const AABTREE_LEAF_FLAG: u32 = 0x8000_0000;

/// What `MeshGeometryClass` holds that its `Cast_Ray` reads: the vertices, the polygons, their surface types, the
/// AABTree and the flags (`ALIGNED`, `ORIENTED`).
#[derive(Clone, Debug, PartialEq)]
pub struct MeshGeometry {
    pub flags: u32,
    pub vertices: Vec<[f32; 3]>,
    pub polys: Vec<[u16; 3]>,
    /// `PolySurfaceType` (0 for a polygon past its end).
    pub surface_types: Vec<u8>,
    pub cull_tree: Option<CullTree>,
}

impl MeshGeometry {
    /// The geometry of a loaded model ([`MeshModel`]).
    pub fn from_model(m: &MeshModel) -> MeshGeometry {
        MeshGeometry { flags: m.flags, vertices: m.vertices.clone(), polys: m.polys.clone(), surface_types: m.surface_types.clone(), cull_tree: m.cull_tree.clone() }
    }

    /// `Get_Flag(f)`.
    pub fn get_flag(&self, f: u32) -> bool {
        self.flags & f != 0
    }

    /// `Get_Poly_Surface_Type(poly_index)` (meshgeometry.h:289).
    fn poly_surface_type(&self, poly_index: usize) -> u32 {
        u32::from(self.surface_types.get(poly_index).copied().unwrap_or(0))
    }

    /// The `TriClass` of polygon `i` (its vertices), `None` when an index is out of range.
    fn tri(&self, i: usize) -> Option<TriClass> {
        let p = self.polys.get(i)?;
        let v = |k: usize| self.vertices.get(usize::from(p[k])).copied().map(v3);
        Some(TriClass { n: Vector3::new(0.0, 0.0, 0.0), v: [v(0)?, v(1)?, v(2)?] })
    }

    /// `Compute_Plane_Equations`' normal of polygon `i` (meshgeometry.cpp:1260): `Subtract(v1, p0, &a)`,
    /// `Subtract(v2, p0, &b)`, `Cross_Product(a, b, &normal)`, `normal.Normalize()` (the plane's `D` is not read by the
    /// cast). What `Get_Plane_Array` gives the brute-force cast (with `OPTIMIZE_PLANEEQ_RAM` it recomputes them all on
    /// each call).
    fn plane_normal(tri: &TriClass) -> Vector3 {
        let a = Vector3::subtract_vectors(&tri.v[1], &tri.v[0]);
        let b = Vector3::subtract_vectors(&tri.v[2], &tri.v[0]);
        let mut normal = Vector3::cross_product(&a, &b);
        if plant(10369) {
            return normal;
        }
        normal.normalize();
        normal
    }

    /// `MeshGeometryClass::Cast_Ray(raytest)` (meshgeometry.cpp:650): "if (CullTree) hit = CullTree->Cast_Ray(raytest);
    /// else hit = cast_ray_brute_force(raytest);".
    pub fn cast_ray(&self, raytest: &mut RayTest) -> bool {
        match &self.cull_tree {
            Some(tree) => cast_ray_tree(tree, self, raytest),
            None => self.cast_ray_brute_force(raytest),
        }
    }

    /// `MeshGeometryClass::cast_ray_brute_force(raytest)` (meshgeometry.cpp:1089): "Loop over each polygon".
    pub fn cast_ray_brute_force(&self, raytest: &mut RayTest) -> bool {
        let mut hit = false;
        let first = usize::from(plant(10368));
        for srtri in first..self.polys.len() {
            let Some(mut tri) = self.tri(srtri) else { continue };
            tri.n = Self::plane_normal(&tri);
            if collide_line_tri(&raytest.ray, &tri, &mut raytest.result) {
                hit = true;
                raytest.result.surface_type = self.poly_surface_type(srtri);
            }
            if raytest.result.start_bad {
                return true;
            }
        }
        hit
    }
}

/// `AABTreeClass::Cast_Ray(raytest)` (aabtree.h:231): `Cast_Ray_Recursive(&(Nodes[0]), raytest)`. No node: no hit (an
/// empty tree reads past its array upstream).
pub fn cast_ray_tree(tree: &CullTree, mesh: &MeshGeometry, raytest: &mut RayTest) -> bool {
    if tree.nodes.is_empty() {
        return false;
    }
    cast_ray_recursive(tree, mesh, 0, raytest, 0)
}

/// `AABTreeClass::Cast_Ray_Recursive(node, raytest)` (aabtree.cpp:527): "Cull the collision test against the bounding
/// volume of this node"; a leaf casts at its polygons, an inner node at its front then its back child (`res = res |
/// ...`, both). `depth` stops a cyclic tree (a stack overflow upstream) after as many levels as it has nodes.
fn cast_ray_recursive(tree: &CullTree, mesh: &MeshGeometry, node: usize, raytest: &mut RayTest, depth: usize) -> bool {
    let Some(n) = tree.nodes.get(node) else { return false };
    if depth > tree.nodes.len() {
        return false;
    }
    if raytest.cull(&v3(n.min), &v3(n.max)) {
        return false;
    }
    let mut res = false;
    if n.front_or_poly0 & AABTREE_LEAF_FLAG != 0 {
        return cast_ray_to_polys(tree, mesh, (n.front_or_poly0 & !AABTREE_LEAF_FLAG) as usize, n.back_or_poly_count as usize, raytest);
    }
    // Get_Front_Child(), Get_Back_Child()
    res |= cast_ray_recursive(tree, mesh, n.front_or_poly0 as usize, raytest, depth + 1);
    if plant(10366) && res {
        return res;
    }
    res |= cast_ray_recursive(tree, mesh, n.back_or_poly_count as usize, raytest, depth + 1);
    res
}

/// `AABTreeClass::Cast_Ray_To_Polys(node, raytest)` (aabtree.cpp:747): "Simply loop through the polys in this node,
/// checking each for collision", each `TriClass::Compute_Normal`'d (`OPTIMIZE_PLANEEQ_RAM`); the surface type of the last
/// polygon hit.
fn cast_ray_to_polys(tree: &CullTree, mesh: &MeshGeometry, poly0: usize, polycount: usize, raytest: &mut RayTest) -> bool {
    if polycount > 0 {
        let mut polyhit: Option<usize> = None;
        for poly_counter in 0..polycount {
            let Some(&poly_index) = poly0.checked_add(poly_counter).and_then(|i| tree.poly_indices.get(i)) else { continue };
            let poly_index = poly_index as usize;
            let Some(mut tri) = mesh.tri(poly_index) else { continue };
            tri.compute_normal();
            if collide_line_tri(&raytest.ray, &tri, &mut raytest.result) && !(plant(10367) && polyhit.is_some()) {
                polyhit = Some(poly_index);
            }
            if raytest.result.start_bad {
                return true;
            }
        }
        if let Some(polyhit) = polyhit {
            raytest.result.surface_type = mesh.poly_surface_type(polyhit);
            return true;
        }
    }
    false
}

/// `MeshClass::Init`'s collision type (mesh.cpp:1168): `col_bits = (W3dAttributes & W3D_MESH_FLAG_COLLISION_TYPE_MASK)
/// >> W3D_MESH_FLAG_COLLISION_TYPE_SHIFT; Set_Collision_Type(col_bits << 1);` on the mesh's bits.
pub fn mesh_collision_type(bits: i32, w3d_attributes: u32) -> i32 {
    use crate::formats::w3d::w3d_file::{W3D_MESH_FLAG_COLLISION_TYPE_MASK, W3D_MESH_FLAG_COLLISION_TYPE_SHIFT};
    let col_bits = ((w3d_attributes & W3D_MESH_FLAG_COLLISION_TYPE_MASK) >> W3D_MESH_FLAG_COLLISION_TYPE_SHIFT) as i32;
    if plant(10370) {
        return col_bits << 1;
    }
    set_collision_type(bits, col_bits << 1)
}

/// A mesh model of an HLod (`MeshClass`), as its `Cast_Ray` reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshRenderObj {
    /// `Get_Collision_Type()` ([`mesh_collision_type`]).
    pub collision_type: i32,
    /// `Is_Alpha()` (`IS_ALPHA`).
    pub is_alpha: bool,
    /// `Is_Hidden()`.
    pub hidden: bool,
    /// `Is_Animation_Hidden()`.
    pub animation_hidden: bool,
    /// `Get_Transform()`.
    pub transform: Matrix3D,
    /// `Model`.
    pub model: Arc<MeshGeometry>,
}

impl MeshRenderObj {
    /// `MeshClass::Cast_Ray(raytest)` (mesh.cpp:1218). Whether it hit; the caller notes the object hit
    /// (`raytest.CollidedRenderObj = this`).
    pub fn cast_ray(&self, raytest: &mut RayTest) -> bool {
        if (self.collision_type & raytest.collision_type) == 0 {
            return false;
        }
        // "Modified for 'Generals' so we could select trees but filter out headlight beams, etc. -MW"
        if raytest.check_translucent && self.is_alpha && !plant(10363) {
            return false;
        }
        if self.hidden && !raytest.check_hidden {
            return false;
        }
        if self.animation_hidden {
            return false;
        }
        if raytest.result.start_bad {
            return false;
        }
        let mut world = self.transform;
        // "if aligned or oriented rotate the mesh so that it's aligned to the ray"
        if self.model.get_flag(mesh_flags::ALIGNED) {
            let mesh_position = world.get_translation();
            world.obj_look_at(&mesh_position, &(mesh_position - raytest.ray.dir), 0.0f32);
        } else if self.model.get_flag(mesh_flags::ORIENTED) {
            let mesh_position = world.get_translation();
            let target = if plant(10364) { raytest.ray.p1 } else { raytest.ray.p0 };
            world.obj_look_at(&mesh_position, &target, 0.0f32);
        }
        let world_to_obj = world.get_inverse();
        // RayCollisionTestClass objray(raytest, world_to_obj): the same result (`Result` is a pointer)
        let mut objray = RayTest { ray: LineSeg::transformed(&raytest.ray, &world_to_obj), ..*raytest };
        let hit = self.model.cast_ray(&mut objray);
        raytest.result = objray.result;
        // "transform result back into original coordinate system"
        if hit {
            if !plant(10365) {
                raytest.result.normal = Matrix3D::rotate_vector_by(&world, &raytest.result.normal);
            }
            if raytest.result.compute_contact_point {
                raytest.result.contact_point = Matrix3D::transform_vector(&world, &raytest.result.contact_point);
            }
        }
        hit
    }
}

/// A new mesh model with [`DEFAULT_BITS`](coll_type::ALL)' collision type made by `MeshClass::Init`
/// ([`mesh_collision_type`]).
pub fn mesh_render_obj(model: &MeshModel, transform: Matrix3D, hidden: bool) -> MeshRenderObj {
    MeshRenderObj {
        collision_type: mesh_collision_type(coll_type::ALL, model.w3d_attributes),
        is_alpha: false,
        hidden,
        animation_hidden: false,
        transform,
        model: Arc::new(MeshGeometry::from_model(model)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::mesh::AabNode;

    fn geom(tree: bool) -> MeshGeometry {
        // a unit square in z = 0 (two triangles) and a triangle at z = 5
        let vertices = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 5.0], [1.0, 0.0, 5.0], [0.0, 1.0, 5.0]];
        let polys = vec![[0, 1, 2], [0, 2, 3], [4, 5, 6]];
        let cull_tree = tree.then(|| CullTree {
            nodes: vec![
                AabNode { min: [0.0, 0.0, 0.0], max: [1.0, 1.0, 5.0], front_or_poly0: 1, back_or_poly_count: 2 },
                AabNode { min: [0.0, 0.0, 0.0], max: [1.0, 1.0, 0.0], front_or_poly0: AABTREE_LEAF_FLAG, back_or_poly_count: 2 },
                AabNode { min: [0.0, 0.0, 5.0], max: [1.0, 1.0, 5.0], front_or_poly0: AABTREE_LEAF_FLAG | 2, back_or_poly_count: 1 },
            ],
            poly_indices: vec![0, 1, 2],
        });
        MeshGeometry { flags: 0, vertices, polys, surface_types: vec![7, 8, 9], cull_tree }
    }

    /// #149, predicted before the run: a ray down the z axis from z = 10 to z = 0 through (0.25, 0.75) hits the triangle
    /// at z = 5 first (fraction 0.5, surface type 9), by the tree and by brute force alike; through ~~(0.75, 0.25),
    /// outside that triangle~~ (0.8, 0.3) it hits the square's polygon 0 (x >= y) at its end (fraction 1, surface type
    /// 7). **Missed twice:** (0.75, 0.25) lies on that triangle's hypotenuse (x + y = 1), which `Contains_Point` counts
    /// as inside, so it hit there (0.5, 9) as upstream would; and a hit at the segment's very end (t = 1) is no hit
    /// against a fresh result (`t < Fraction`, and `Fraction` starts at 1). The ray now runs on to z = -10 (fraction
    /// 0.5, surface type 7). A ray in
    /// the plane z = 0 is parallel (no hit); a ray from below (the back face) hits too; the mesh moved up 10 by its
    /// transform is hit at the same fraction by a ray moved with it, its normal (0, 0, 1); and the cull test rejects a
    /// box beside the ray, not one it crosses.
    #[test]
    fn mesh_cast_hits_polygons() {
        for tree in [false, true] {
            let g = geom(tree);
            let mut rt = RayTest::new(LineSeg::new(Vector3::new(0.25, 0.75, 10.0), Vector3::new(0.25, 0.75, 0.0)), CastResult::default(), coll_type::ALL, false, false);
            assert!(g.cast_ray(&mut rt));
            assert_eq!((rt.result.fraction, rt.result.surface_type), (0.5, 9));
            let mut rt = RayTest::new(LineSeg::new(Vector3::new(0.8, 0.3, 10.0), Vector3::new(0.8, 0.3, -10.0)), CastResult::default(), coll_type::ALL, false, false);
            assert!(g.cast_ray(&mut rt));
            assert_eq!((rt.result.fraction, rt.result.surface_type), (0.5, 7));
            let mut rt = RayTest::new(LineSeg::new(Vector3::new(-1.0, 0.5, 0.0), Vector3::new(2.0, 0.5, 0.0)), CastResult::default(), coll_type::ALL, false, false);
            assert!(!g.cast_ray(&mut rt));
            let mut rt = RayTest::new(LineSeg::new(Vector3::new(0.75, 0.25, -1.0), Vector3::new(0.75, 0.25, 1.0)), CastResult::default(), coll_type::ALL, false, false);
            assert!(g.cast_ray(&mut rt));
            assert_eq!(rt.result.fraction, 0.5);
            let m = MeshRenderObj { collision_type: coll_type::ALL, is_alpha: false, hidden: false, animation_hidden: false, transform: Matrix3D::from_translation(&Vector3::new(0.0, 0.0, 10.0)), model: Arc::new(g) };
            let mut rt = RayTest::new(LineSeg::new(Vector3::new(0.25, 0.75, 20.0), Vector3::new(0.25, 0.75, 10.0)), CastResult::default(), coll_type::ALL, false, false);
            assert!(m.cast_ray(&mut rt));
            assert_eq!(rt.result.fraction, 0.5);
            assert_eq!(rt.result.normal, Vector3::new(0.0, 0.0, 1.0));
        }
        let rt = RayTest::new(LineSeg::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)), CastResult::default(), coll_type::ALL, false, false);
        assert!(rt.cull(&Vector3::new(0.0, 2.0, 0.0), &Vector3::new(1.0, 3.0, 1.0)));
        assert!(!rt.cull(&Vector3::new(0.5, -1.0, -1.0), &Vector3::new(2.0, 1.0, 1.0)));
        assert_eq!(mesh_collision_type(coll_type::ALL, 0), coll_type::ALL);
    }
}
