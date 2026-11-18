//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases x 6 operations (test: tests/differential_draft_aabtree.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtree.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtree.h
//!
//! The `AABTreeClass` recursive casts and APT generators no earlier pass translated (the verified port has
//! `AABTreeClass::Cast_Ray` and its two helpers in `client::mesh_cast`, the loader in `formats::w3d::mesh`, and the
//! builder in `formats::w3d::aabtreebuilder`). The tree is [`AabTreeView`] (nodes are the port's
//! [`crate::formats::w3d::mesh::AabNode`]); the mesh is [`MeshGeometry`] of the `meshgeometry` draft in this directory.
//! Every collision-math call (`CollisionMath::Intersection_Test`, `Collide`, the test classes' `Cull`) is the
//! [`AabTreeTestHost`] contract, called with the same arguments and in upstream's order.
//!
//! `OPTIMIZE_PLANEEQ_RAM` is on upstream (`tri.N` is `Compute_Normal()`'d, not read from the plane array): [`tri_view`]
//! computes it.

use crate::logic::modules::plant;
use crate::drafts::sweep_ww3d::meshgeometry::{MeshGeometry, TriView};
use crate::formats::w3d::mesh::AabNode;
use crate::ww::vector3::Vector3;

/// `AABTREE_LEAF_FLAG` (aabtree.h:76).
pub const AABTREE_LEAF_FLAG: u32 = 0x8000_0000;

/// `AABTreeClass`'s `Nodes` and `PolyIndices`.
#[derive(Clone, Debug, Default)]
pub struct AabTreeView {
    pub nodes: Vec<AabNode>,
    pub poly_indices: Vec<u32>,
}

/// `CullNodeStruct::Is_Leaf` (aabtree.h:301).
pub fn node_is_leaf(node: &AabNode) -> bool {
    (node.front_or_poly0 & AABTREE_LEAF_FLAG) != 0
}

/// `CullNodeStruct::Get_Front_Child` (aabtree.h:306).
pub fn node_front_child(node: &AabNode) -> usize {
    node.front_or_poly0 as usize
}

/// `CullNodeStruct::Get_Back_Child` (aabtree.h:312).
pub fn node_back_child(node: &AabNode) -> usize {
    node.back_or_poly_count as usize
}

/// `CullNodeStruct::Get_Poly0` (aabtree.h:318).
pub fn node_poly0(node: &AabNode) -> usize {
    (node.front_or_poly0 & !AABTREE_LEAF_FLAG) as usize
}

/// `CullNodeStruct::Get_Poly_Count` (aabtree.h:324).
pub fn node_poly_count(node: &AabNode) -> usize {
    node.back_or_poly_count as usize
}

/// The triangle of `poly_index` as the casts build it: `tri.V[i] = &loc[polys[poly_index][i]]` and `tri.N`. With
/// `OPTIMIZE_PLANEEQ_RAM` on (meshgeometry.h:80) upstream does not read the plane array for `tri.N`: it calls
/// `tri.Compute_Normal()` (tri.h:67): `Cross_Product(V1 - V0, V2 - V0)` then `Normalize()`.
/// ~~`tri.N` is the stored plane~~ **Corrected 2026-11-18 (#215):** the differential test (tests/differential_draft_aabtree.rs)
/// found the stored plane passed where upstream passes the computed normal.
pub fn tri_view(mesh: &MeshGeometry, poly_index: usize) -> TriView {
    let p = mesh.polygons[poly_index];
    let v = [mesh.vertices[p[0] as usize], mesh.vertices[p[1] as usize], mesh.vertices[p[2] as usize]];
    let (v0, v1, v2) = (Vector3::new(v[0][0], v[0][1], v[0][2]), Vector3::new(v[1][0], v[1][1], v[1][2]), Vector3::new(v[2][0], v[2][1], v[2][2]));
    let mut n = Vector3::cross_product(&(v1 - v0), &(v2 - v0));
    n.normalize();
    TriView { v, n: [n.x, n.y, n.z] }
}

/// What the recursive tests call. One method per upstream collision call; the host holds the test object
/// (`OBBoxAPTContextStruct`, `RayCollisionTestClass`, `AABoxCollisionTestClass`, `OBBoxCollisionTestClass`,
/// `OBBoxIntersectionTestClass`) and its result.
pub trait AabTreeTestHost {
    /// `CollisionMath::Intersection_Test(context.Box, nodebox)` with `nodebox.Init_Min_Max(node->Min, node->Max)`.
    fn box_intersects_node(&mut self, min: [f32; 3], max: [f32; 3]) -> bool;
    /// `CollisionMath::Intersection_Test(context.Box, tri)`.
    fn box_intersects_tri(&mut self, tri: &TriView) -> bool;
    /// `context.APT.Add(poly_index)`.
    fn apt_add(&mut self, poly_index: u32);
    /// `context.ViewVector`.
    fn view_vector(&self) -> [f32; 3];
    /// `test.Cull(node->Min, node->Max)` of whichever test object this recursion carries.
    fn cull(&mut self, min: [f32; 3], max: [f32; 3]) -> bool;
    /// `CollisionMath::Collide(boxtest.Box, boxtest.Move, tri, boxtest.Result)` (AABox sweep).
    fn collide_aabox_tri(&mut self, tri: &TriView) -> bool;
    /// `CollisionMath::Collide(boxtest.Box, boxtest.Move, tri, Vector3(0,0,0), boxtest.Result)` (OBBox sweep).
    fn collide_obbox_tri(&mut self, tri: &TriView) -> bool;
    /// `CollisionMath::Intersection_Test(test.Box, tri)` for `OBBoxIntersectionTestClass`.
    fn intersect_obbox_tri(&mut self, tri: &TriView) -> bool;
    /// `Result->StartBad`.
    fn result_start_bad(&self) -> bool;
    /// `Result->SurfaceType = surface_type`.
    fn set_result_surface_type(&mut self, surface_type: u32);
    /// `Mesh->Get_Poly_Surface_Type(poly)`.
    fn poly_surface_type(&self, poly: usize) -> u32;
    /// `Cast_Semi_Infinite_Axis_Aligned_Ray_To_Triangle(v0, v1, v2, tri_plane, start_point, axis_r, axis_1, axis_2,
    /// direction, flags)`.
    fn cast_semi_infinite_axis_aligned_ray_to_triangle(
        &mut self,
        tri: &TriView,
        tri_plane: [f32; 4],
        start_point: [f32; 3],
        axis_r: usize,
        axis_1: usize,
        axis_2: usize,
        direction: usize,
        flags: &mut u8,
    ) -> bool;
}

impl AabTreeView {
    /// `AABTreeClass::Generate_OBBox_APT_Recursive(node, OBBoxAPTContextStruct & context)` (aabtree.cpp:359): "Test the
    /// volume against the bounding volume of this node. If it is outside, stop descending the tree." A leaf adds every
    /// polygon the box intersects; an inner node recurses front then back.
    pub fn generate_obbox_apt_recursive<H: AabTreeTestHost>(&self, mesh: &MeshGeometry, node: usize, host: &mut H) {
        let n = self.nodes[node];
        if !host.box_intersects_node(n.min, n.max) {
            return;
        }
        if node_is_leaf(&n) {
            let polycount = node_poly_count(&n);
            let poly0 = node_poly0(&n);
            if polycount > 0 {
                for poly_counter in 0..polycount {
                    let poly_index = self.poly_indices[poly0 + poly_counter] as usize;
                    let tri = tri_view(mesh, poly_index);
                    if host.box_intersects_tri(&tri) {
                        host.apt_add(poly_index as u32);
                    }
                }
            }
        } else {
            let (a, b) = if plant(11120) { (node_back_child(&n), node_front_child(&n)) } else { (node_front_child(&n), node_back_child(&n)) };
            self.generate_obbox_apt_recursive(mesh, a, host);
            self.generate_obbox_apt_recursive(mesh, b, host);
        }
    }

    /// `AABTreeClass::Generate_OBBox_APT_Recursive(node, OBBoxRayAPTContextStruct & context)` (aabtree.cpp:454): as above
    /// but only polygons facing against the view vector (`Dot_Product(*tri.N, context.ViewVector) < 0.0f`) are tested.
    pub fn generate_obbox_apt_recursive_dir<H: AabTreeTestHost>(&self, mesh: &MeshGeometry, node: usize, host: &mut H) {
        let n = self.nodes[node];
        if !host.box_intersects_node(n.min, n.max) {
            return;
        }
        if node_is_leaf(&n) {
            let polycount = node_poly_count(&n);
            let poly0 = node_poly0(&n);
            if polycount > 0 {
                let view = host.view_vector();
                for poly_counter in 0..polycount {
                    let poly_index = self.poly_indices[poly0 + poly_counter] as usize;
                    let tri = tri_view(mesh, poly_index);
                    // Vector3::Dot_Product(*tri.N, context.ViewVector)
                    let dot = tri.n[0] * view[0] + tri.n[1] * view[1] + tri.n[2] * view[2];
                    if (if plant(11121) { dot > 0.0 } else { dot < 0.0 }) {
                        if host.box_intersects_tri(&tri) {
                            host.apt_add(poly_index as u32);
                        }
                    }
                }
            }
        } else {
            self.generate_obbox_apt_recursive_dir(mesh, node_front_child(&n), host);
            self.generate_obbox_apt_recursive_dir(mesh, node_back_child(&n), host);
        }
    }

    /// `AABTreeClass::Cast_Semi_Infinite_Axis_Aligned_Ray_Recursive` (aabtree.cpp:571). `direction` is 0 (negative) or
    /// 1 (positive). The 2D test is against `axis_1` and `axis_2`, the 1D test is `start[direction] > bounds[direction]`
    /// with `bounds = { -Min[axis_r], Max[axis_r] }` and `start = { -start_point[axis_r], start_point[axis_r] }`.
    pub fn cast_semi_infinite_axis_aligned_ray_recursive<H: AabTreeTestHost>(
        &self,
        mesh: &MeshGeometry,
        host: &mut H,
        node: usize,
        start_point: [f32; 3],
        axis_r: usize,
        axis_1: usize,
        axis_2: usize,
        direction: usize,
        flags: &mut u8,
    ) -> i32 {
        let n = self.nodes[node];
        let bounds = [-n.min[axis_r], if plant(11129) { n.min[axis_r] } else { n.max[axis_r] }];
        let start = [-start_point[axis_r], start_point[axis_r]];
        if start_point[axis_1] < n.min[axis_1]
            || start_point[axis_2] < n.min[axis_2]
            || start_point[axis_1] > n.max[axis_1]
            || start_point[axis_2] > n.max[axis_2]
            || (if plant(11126) { start[direction] >= bounds[direction] } else { start[direction] > bounds[direction] })
        {
            return 0;
        }

        let mut count = 0;
        if node_is_leaf(&n) {
            return self.cast_semi_infinite_axis_aligned_ray_to_polys(mesh, host, node, start_point, axis_r, axis_1, axis_2, direction, flags);
        } else {
            count += self.cast_semi_infinite_axis_aligned_ray_recursive(mesh, host, node_front_child(&n), start_point, axis_r, axis_1, axis_2, direction, flags);
            count += self.cast_semi_infinite_axis_aligned_ray_recursive(mesh, host, node_back_child(&n), start_point, axis_r, axis_1, axis_2, direction, flags);
        }
        count
    }

    /// `AABTreeClass::Cast_AABox_Recursive` (aabtree.cpp:629): bail out when the node is culled; both children are
    /// visited (`res = res | ...`).
    pub fn cast_aabox_recursive<H: AabTreeTestHost>(&self, mesh: &MeshGeometry, host: &mut H, node: usize) -> bool {
        let n = self.nodes[node];
        if host.cull(n.min, n.max) {
            return false;
        }
        let mut res = false;
        if node_is_leaf(&n) {
            return self.cast_aabox_to_polys(mesh, host, node);
        } else {
            res = res | self.cast_aabox_recursive(mesh, host, node_front_child(&n));
            res = if plant(11122) { res || self.cast_aabox_recursive(mesh, host, node_back_child(&n)) } else { res | self.cast_aabox_recursive(mesh, host, node_back_child(&n)) };
        }
        res
    }

    /// `AABTreeClass::Cast_OBBox_Recursive` (aabtree.cpp:669).
    pub fn cast_obbox_recursive<H: AabTreeTestHost>(&self, mesh: &MeshGeometry, host: &mut H, node: usize) -> bool {
        let n = self.nodes[node];
        if host.cull(n.min, n.max) {
            return false;
        }
        let mut res = false;
        if node_is_leaf(&n) {
            return self.cast_obbox_to_polys(mesh, host, node);
        } else {
            res = res | self.cast_obbox_recursive(mesh, host, node_front_child(&n));
            res = res | self.cast_obbox_recursive(mesh, host, node_back_child(&n));
        }
        res
    }

    /// `AABTreeClass::Intersect_OBBox_Recursive` (aabtree.cpp:707).
    pub fn intersect_obbox_recursive<H: AabTreeTestHost>(&self, mesh: &MeshGeometry, host: &mut H, node: usize) -> bool {
        let n = self.nodes[node];
        if host.cull(n.min, n.max) {
            return false;
        }
        let mut res = false;
        if node_is_leaf(&n) {
            return self.intersect_obbox_with_polys(mesh, host, node);
        } else {
            res = res | self.intersect_obbox_recursive(mesh, host, node_front_child(&n));
            res = res | self.intersect_obbox_recursive(mesh, host, node_back_child(&n));
        }
        res
    }

    /// `AABTreeClass::Cast_Semi_Infinite_Axis_Aligned_Ray_To_Polys` (aabtree.cpp:817): the count of triangles the ray
    /// hits in this leaf (`count += (unsigned int)...To_Triangle(...)`).
    pub fn cast_semi_infinite_axis_aligned_ray_to_polys<H: AabTreeTestHost>(
        &self,
        mesh: &MeshGeometry,
        host: &mut H,
        node: usize,
        start_point: [f32; 3],
        axis_r: usize,
        axis_1: usize,
        axis_2: usize,
        direction: usize,
        flags: &mut u8,
    ) -> i32 {
        let n = self.nodes[node];
        let mut count = 0;
        if node_poly_count(&n) > 0 {
            let poly0 = node_poly0(&n);
            let polycount = node_poly_count(&n);
            for poly_counter in 0..polycount {
                let poly_index = self.poly_indices[poly0 + poly_counter] as usize;
                let tri = tri_view(mesh, poly_index);
                let tri_plane = mesh.planes[poly_index];
                // Since (int)true is defined as 1, and (int)false as 0:
                if host.cast_semi_infinite_axis_aligned_ray_to_triangle(&tri, tri_plane, start_point, axis_r, axis_1, axis_2, direction, flags) {
                    count += if plant(11127) { 2 } else { 1 };
                }
            }
        }
        count
    }

    /// `AABTreeClass::Cast_AABox_To_Polys` (aabtree.cpp:866): "Simply loop through the polys in this node, checking each
    /// for collision"; `StartBad` returns true at once; the surface type is that of the last polygon hit.
    pub fn cast_aabox_to_polys<H: AabTreeTestHost>(&self, mesh: &MeshGeometry, host: &mut H, node: usize) -> bool {
        let n = self.nodes[node];
        if node_poly_count(&n) > 0 {
            let mut polyhit: i64 = -1;
            let poly0 = node_poly0(&n);
            let polycount = node_poly_count(&n);
            for poly_counter in 0..polycount {
                let poly_index = self.poly_indices[poly0 + poly_counter] as usize;
                let tri = tri_view(mesh, poly_index);
                if host.collide_aabox_tri(&tri) {
                    if !plant(11125) || polyhit == -1 {
                        polyhit = poly_index as i64;
                    }
                }
                if host.result_start_bad() && !plant(11123) {
                    return true;
                }
            }
            if polyhit != -1 {
                let st = host.poly_surface_type(polyhit as usize);
                host.set_result_surface_type(st);
                return true;
            }
        }
        false
    }

    /// `AABTreeClass::Cast_OBBox_To_Polys` (aabtree.cpp:927): as [`Self::cast_aabox_to_polys`] with the OBBox sweep
    /// (`Collide(boxtest.Box, boxtest.Move, tri, Vector3(0,0,0), boxtest.Result)`).
    pub fn cast_obbox_to_polys<H: AabTreeTestHost>(&self, mesh: &MeshGeometry, host: &mut H, node: usize) -> bool {
        let n = self.nodes[node];
        let polycount = node_poly_count(&n);
        let poly0 = node_poly0(&n);
        if polycount > 0 {
            let mut polyhit: i64 = -1;
            for poly_counter in 0..polycount {
                let poly_index = self.poly_indices[poly0 + poly_counter] as usize;
                let tri = tri_view(mesh, poly_index);
                if host.collide_obbox_tri(&tri) {
                    polyhit = poly_index as i64;
                }
                if host.result_start_bad() && !plant(11124) {
                    return true;
                }
            }
            if polyhit != -1 {
                let st = host.poly_surface_type(polyhit as usize);
                host.set_result_surface_type(st);
                return true;
            }
        }
        false
    }

    /// `AABTreeClass::Intersect_OBBox_With_Polys` (aabtree.cpp:992, not on the sweep list but the leaf of
    /// `Intersect_OBBox_Recursive`): true at the first polygon the box intersects.
    pub fn intersect_obbox_with_polys<H: AabTreeTestHost>(&self, mesh: &MeshGeometry, host: &mut H, node: usize) -> bool {
        let n = self.nodes[node];
        let poly0 = node_poly0(&n);
        let polycount = node_poly_count(&n);
        if polycount > 0 {
            for poly_counter in 0..polycount {
                let poly_index = self.poly_indices[poly0 + poly_counter] as usize;
                let tri = tri_view(mesh, poly_index);
                if host.intersect_obbox_tri(&tri) && !plant(11130) {
                    return true;
                }
            }
        }
        false
    }
}
