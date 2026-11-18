//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 meshes per member (test: tests/differential_draft_meshgeom.rs);
//! not yet wired into the game.
//! Not covered: `get_user_text` and `set_user_text` (a ref-counted share buffer; upstream leaves a NULL argument dangling).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshgeometry.cpp
//!
//! The collision-side members of `MeshGeometryClass` that no earlier pass translated: the active-polygon-table
//! generators (`Generate_Rigid_APT`, `Generate_Skin_APT`), the axis-aligned ray cast, the AABox cast in its four Z
//! orientations, and the brute-force intersection / casts that run when the mesh has no `AABTreeClass`.
//! The triangle tests are `CollisionMath::Intersection_Test` / `Collide` (drafts in `wwlib::colmath*`) and the cull tree's
//! own casts (`aabtree.cpp`); they are the [`MeshCollisionHost`] contract, called with the triangle's three vertices and
//! its plane (`Get_Plane_Array`), in upstream's order.
//!
//! `COMPUTE_NORMALS` is not defined upstream, so `tri.N` is always the stored plane (the `#ifdef` branch is not kept).
//! Not drafted here because already translated elsewhere: `Cast_Ray_Brute_Force` is on the original list but its body
//! (the ray analogue of `cast_aabox_brute_force`) is [`MeshGeometry::cast_ray_brute_force`] below.

use crate::logic::modules::plant;

/// `TriRayCast flags` and the result fields the casts touch. `normal` is `CastResultStruct::Normal`.
#[derive(Clone, Debug, Default)]
pub struct CastResult {
    pub normal: [f32; 3],
    pub start_bad: bool,
    pub surface_type: u32,
}

/// The mesh data the casts read (`Get_Vertex_Array`, `Get_Plane_Array`, `Get_Polygon_Array`, `UserText`).
#[derive(Clone, Debug, Default)]
pub struct MeshGeometry {
    pub vertices: Vec<[f32; 3]>,
    /// `Get_Plane_Array`: `(nx, ny, nz, d)` per polygon.
    pub planes: Vec<[f32; 4]>,
    pub polygons: Vec<[u32; 3]>,
    pub has_cull_tree: bool,
    /// `UserText`: `None` is NULL.
    pub user_text: Option<String>,
}

/// A triangle for the collision tests: vertices and the plane normal (`TriClass::V[3]`, `N`).
#[derive(Clone, Copy, Debug)]
pub struct TriView {
    pub v: [[f32; 3]; 3],
    pub n: [f32; 3],
}

/// `AABoxCollisionTestClass::ROTATE_*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxRotation {
    Z90,
    Z180,
    Z270,
}

/// What the brute-force and cull-tree paths call. The cull-tree methods are only called when `has_cull_tree`.
pub trait MeshCollisionHost {
    /// `CullTree->Generate_APT(local_box, apt)`.
    fn cull_tree_generate_apt_box(&mut self, apt: &mut Vec<u32>);
    /// `CullTree->Generate_APT(local_box, viewdir, apt)`.
    fn cull_tree_generate_apt_box_dir(&mut self, viewdir: [f32; 3], apt: &mut Vec<u32>);
    /// `CullTree->Cast_Semi_Infinite_Axis_Aligned_Ray(start_point, axis_dir, flags)`.
    fn cull_tree_cast_semi_infinite_axis_aligned_ray(&mut self, start_point: [f32; 3], axis_dir: i32, flags: &mut u8) -> i32;
    /// `CullTree->Cast_AABox(newbox)`: the box test is already translated / rotated by the caller.
    fn cull_tree_cast_aabox(&mut self, result: &mut CastResult) -> bool;
    /// `CollisionMath::Intersection_Test(local_box, tri)` (OBBox vs triangle).
    fn obbox_intersects_tri(&mut self, tri: &TriView) -> bool;
    /// `CollisionMath::Collide(raytest.Ray, tri, result)`.
    fn collide_ray_tri(&mut self, tri: &TriView, result: &mut CastResult) -> bool;
    /// `CollisionMath::Collide(boxtest.Box, boxtest.Move, tri, result)` (AABox sweep).
    fn collide_aabox_tri(&mut self, tri: &TriView, result: &mut CastResult) -> bool;
    /// `CollisionMath::Collide(boxtest.Box, boxtest.Move, tri, Vector3(0,0,0), result)` (OBBox sweep).
    fn collide_obbox_tri(&mut self, tri: &TriView, result: &mut CastResult) -> bool;
    /// `Cast_Semi_Infinite_Axis_Aligned_Ray_To_Triangle(v0, v1, v2, plane, start, r, a1, a2, dir, flags)`.
    fn cast_semi_infinite_axis_aligned_ray_to_triangle(&mut self, tri: &TriView, plane: [f32; 4], start_point: [f32; 3], axis_r: usize, axis_1: usize, axis_2: usize, direction: i32, flags: &mut u8) -> bool;
    /// `Get_Poly_Surface_Type(poly)`.
    fn poly_surface_type(&self, poly: usize) -> u32;
    /// `newbox.Translate(translation); newbox.Rotate(rotation)` on a copy, then the cull-tree / brute-force cast; the host
    /// keeps `boxtest` and `newbox`. The boolean is the hit.
    fn cast_aabox_on_copy(&mut self, translation: [f32; 3], rotation: Option<BoxRotation>, use_cull_tree: bool, result: &mut CastResult) -> bool;
}

/// `TRI_RAYCAST_FLAG_NONE`.
pub const TRI_RAYCAST_FLAG_NONE: u8 = 0;

impl MeshGeometry {
    fn tri(&self, poly: usize) -> TriView {
        let p = self.polygons[poly];
        let n = self.planes[poly];
        TriView { v: [self.vertices[p[0] as usize], self.vertices[p[1] as usize], self.vertices[p[2] as usize]], n: [n[0], n[1], n[2]] }
    }

    /// `MeshGeometryClass::Get_User_Text` (meshgeometry.cpp:366).
    pub fn get_user_text(&self) -> Option<&str> {
        self.user_text.as_deref()
    }

    /// `MeshGeometryClass::Set_User_Text` (meshgeometry.cpp:387): replaces the text; a NULL argument releases the old one
    /// and, as upstream leaves it, does NOT clear the pointer (the old text stays). Here `None` keeps the old text.
    pub fn set_user_text(&mut self, usertext: Option<&str>) {
        if let Some(t) = usertext {
            self.user_text = Some(t.to_string());
        }
    }

    /// `MeshGeometryClass::Generate_Rigid_APT(view_dir, apt)` (meshgeometry.cpp:451): the polygons facing the viewer
    /// (`N . view_dir < 0`).
    pub fn generate_rigid_apt_dir(&self, view_dir: [f32; 3], apt: &mut Vec<u32>) {
        for poly_counter in 0..self.polygons.len() {
            let n = self.planes[poly_counter];
            let d = n[0] * view_dir[0] + n[1] * view_dir[1] + n[2] * view_dir[2];
            if if plant(11707) { d <= 0.0 } else { d < 0.0 } {
                apt.push(poly_counter as u32);
            }
        }
    }

    /// `MeshGeometryClass::Generate_Rigid_APT(local_box, apt)` (meshgeometry.cpp:486).
    pub fn generate_rigid_apt_box<H: MeshCollisionHost>(&self, host: &mut H, apt: &mut Vec<u32>) {
        if self.has_cull_tree != plant(11725) {
            host.cull_tree_generate_apt_box(apt);
        } else {
            for poly_counter in 0..self.polygons.len() {
                let tri = self.tri(poly_counter);
                if host.obbox_intersects_tri(&tri) {
                    apt.push(poly_counter as u32);
                }
            }
        }
    }

    /// `MeshGeometryClass::Generate_Rigid_APT(local_box, viewdir, apt)` (meshgeometry.cpp:525).
    pub fn generate_rigid_apt_box_dir<H: MeshCollisionHost>(&self, host: &mut H, viewdir: [f32; 3], apt: &mut Vec<u32>) {
        if self.has_cull_tree {
            host.cull_tree_generate_apt_box_dir(viewdir, apt);
        } else {
            for poly_counter in 0..self.polygons.len() {
                let n = self.planes[poly_counter];
                let d = n[0] * viewdir[0] + n[1] * viewdir[1] + n[2] * viewdir[2];
                if if plant(11708) { d > 0.0 } else { d < 0.0 } {
                    let tri = self.tri(poly_counter);
                    if host.obbox_intersects_tri(&tri) {
                        apt.push(poly_counter as u32);
                    }
                }
            }
        }
    }

    /// `MeshGeometryClass::Generate_Skin_APT` (meshgeometry.cpp:567): the skin's polygons use the deformed world vertices
    /// and a dummy normal (0, 0, 1) (the OBBox-vs-triangle test does not read the normal for this case).
    pub fn generate_skin_apt<H: MeshCollisionHost>(&self, host: &mut H, apt: &mut Vec<u32>, world_vertex_locs: &[[f32; 3]]) {
        for poly_counter in 0..self.polygons.len() {
            let p = self.polygons[poly_counter];
            let tri = TriView {
                v: if plant(11724) { [self.vertices[p[0] as usize], self.vertices[p[1] as usize], self.vertices[p[2] as usize]] } else { [world_vertex_locs[p[0] as usize], world_vertex_locs[p[1] as usize], world_vertex_locs[p[2] as usize]] },
                n: if plant(11709) { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] },
            };
            if host.obbox_intersects_tri(&tri) {
                apt.push(poly_counter as u32);
            }
        }
    }

    /// `MeshGeometryClass::cast_semi_infinite_axis_aligned_ray` (meshgeometry.cpp:833).
    pub fn cast_semi_infinite_axis_aligned_ray<H: MeshCollisionHost>(&self, host: &mut H, start_point: [f32; 3], axis_dir: i32, flags: &mut u8) -> i32 {
        let mut count = 0;
        if self.has_cull_tree {
            count = host.cull_tree_cast_semi_infinite_axis_aligned_ray(start_point, axis_dir, flags);
        } else {
            const AXIS_R: [usize; 6] = [0, 0, 1, 1, 2, 2];
            const AXIS_1: [usize; 6] = [1, 1, 2, 2, 0, 0];
            const AXIS_2: [usize; 6] = [2, 2, 0, 0, 1, 1];
            let direction: [i32; 6] = if plant(11710) { [1, 0, 1, 0, 1, 1] } else { [1, 0, 1, 0, 1, 0] };
            debug_assert!(axis_dir >= 0);
            debug_assert!(axis_dir < 6);
            if !plant(11711) {
                *flags = TRI_RAYCAST_FLAG_NONE;
            }
            for poly_counter in 0..self.polygons.len() {
                let tri = self.tri(poly_counter);
                let plane = self.planes[poly_counter];
                let a = axis_dir as usize;
                if host.cast_semi_infinite_axis_aligned_ray_to_triangle(&tri, plane, start_point, AXIS_R[a], AXIS_1[a], AXIS_2[a], direction[a], flags) {
                    count = if plant(11712) { 1 } else { count + 1 };
                }
            }
        }
        count
    }

    /// `MeshGeometryClass::cast_aabox_identity` (meshgeometry.cpp:896).
    pub fn cast_aabox_identity<H: MeshCollisionHost>(&self, host: &mut H, translation: [f32; 3], result: &mut CastResult) -> bool {
        host.cast_aabox_on_copy(translation, if plant(11717) { Some(BoxRotation::Z90) } else { None }, self.has_cull_tree, result)
    }

    /// `MeshGeometryClass::cast_aabox_z90` (meshgeometry.cpp:923): cast with the box rotated `ROTATE_Z270`, then rotate the
    /// hit normal back: `(x, y) <- (-y, x)`.
    pub fn cast_aabox_z90<H: MeshCollisionHost>(&self, host: &mut H, translation: [f32; 3], result: &mut CastResult) -> bool {
        let hit = host.cast_aabox_on_copy(translation, Some(if plant(11713) { BoxRotation::Z90 } else { BoxRotation::Z270 }), self.has_cull_tree, result);
        if hit {
            let tmp = result.normal[0];
            if plant(11714) {
                result.normal[0] = result.normal[1];
                result.normal[1] = -tmp;
            } else {
                result.normal[0] = -result.normal[1];
                result.normal[1] = tmp;
            }
        }
        hit
    }

    /// `MeshGeometryClass::cast_aabox_z180` (meshgeometry.cpp:962): `ROTATE_Z180`, normal `(x, y) <- (-x, -y)`.
    pub fn cast_aabox_z180<H: MeshCollisionHost>(&self, host: &mut H, translation: [f32; 3], result: &mut CastResult) -> bool {
        let hit = host.cast_aabox_on_copy(translation, Some(BoxRotation::Z180), self.has_cull_tree, result);
        if hit {
            result.normal[0] = -result.normal[0];
            if !plant(11715) {
                result.normal[1] = -result.normal[1];
            }
        }
        hit
    }

    /// `MeshGeometryClass::cast_aabox_z270` (meshgeometry.cpp:1000): `ROTATE_Z90`, normal `(x, y) <- (y, -x)`.
    pub fn cast_aabox_z270<H: MeshCollisionHost>(&self, host: &mut H, translation: [f32; 3], result: &mut CastResult) -> bool {
        let hit = host.cast_aabox_on_copy(translation, Some(BoxRotation::Z90), self.has_cull_tree, result);
        if hit {
            let tmp = result.normal[0];
            result.normal[0] = result.normal[1];
            result.normal[1] = if plant(11716) { tmp } else { -tmp };
        }
        hit
    }

    /// `MeshGeometryClass::intersect_obbox_brute_force` (meshgeometry.cpp:1041): first polygon the box touches.
    pub fn intersect_obbox_brute_force<H: MeshCollisionHost>(&self, host: &mut H) -> bool {
        let mut any = false;
        for srtri in 0..self.polygons.len() {
            let tri = self.tri(srtri);
            if host.obbox_intersects_tri(&tri) {
                if plant(11723) {
                    any = true;
                } else {
                    return true;
                }
            }
        }
        any
    }

    /// `MeshGeometryClass::cast_ray_brute_force`: every polygon is tested, a hit records the polygon's surface type, and
    /// `StartBad` ends the loop early with `true`.
    pub fn cast_ray_brute_force<H: MeshCollisionHost>(&self, host: &mut H, result: &mut CastResult) -> bool {
        let mut hit = false;
        for srtri in 0..self.polygons.len() {
            let tri = self.tri(srtri);
            if host.collide_ray_tri(&tri, result) {
                hit = true;
                if !plant(11718) {
                    result.surface_type = host.poly_surface_type(srtri);
                }
            }
            if plant(11718) {
                result.surface_type = host.poly_surface_type(srtri);
            }
            if result.start_bad && !plant(11719) {
                return true;
            }
        }
        hit
    }

    /// `MeshGeometryClass::cast_aabox_brute_force` (meshgeometry.cpp:1144): remembers the LAST polygon hit and takes its
    /// surface type after the loop; `StartBad` returns `true` at once (without a surface type).
    pub fn cast_aabox_brute_force<H: MeshCollisionHost>(&self, host: &mut H, result: &mut CastResult) -> bool {
        let mut polyhit: i64 = -1;
        for srtri in 0..self.polygons.len() {
            let tri = self.tri(srtri);
            if host.collide_aabox_tri(&tri, result) && !(plant(11720) && polyhit != -1) {
                polyhit = srtri as i64;
            }
            if result.start_bad {
                return !plant(11721);
            }
        }
        if polyhit != -1 {
            result.surface_type = host.poly_surface_type(if plant(11726) { self.polygons.len() - 1 } else { polyhit as usize });
            return true;
        }
        false
    }

    /// `MeshGeometryClass::cast_obbox_brute_force` (meshgeometry.cpp:1202): as the AABox version, with the OBBox sweep.
    pub fn cast_obbox_brute_force<H: MeshCollisionHost>(&self, host: &mut H, result: &mut CastResult) -> bool {
        let mut polyhit: i64 = -1;
        for srtri in 0..self.polygons.len() {
            let tri = self.tri(srtri);
            if if plant(11722) { host.collide_aabox_tri(&tri, result) } else { host.collide_obbox_tri(&tri, result) } {
                polyhit = srtri as i64;
            }
            if result.start_bad {
                return true;
            }
        }
        if polyhit != -1 {
            result.surface_type = host.poly_surface_type(polyhit as usize);
            return true;
        }
        false
    }
}
