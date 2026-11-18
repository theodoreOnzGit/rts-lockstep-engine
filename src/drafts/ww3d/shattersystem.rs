//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shattersystem.cpp   (lines 1-600: the clipping geometry)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shattersystem.h
//!
//! Reachability note, the reason the second half is not drafted: `ShatterSystem::Init` has `#if 1  HTreeClass *htree =
//! NULL;` in place of `Get_HTree("ShatterPattern00")`, so in the shipped game `ShatterPatterns` stays empty and
//! `Shatter_Mesh` returns at its first line (`if (ShatterPatterns.Count() == 0) return;`). Translated: the clipping
//! classes it would use (`VertexClass`, `PolygonClass`, `BSPClass`), `Init` (as that no-op) and `Shutdown`.
//! NOT translated: the body of `Shatter_Mesh` after the early return (re-meshing the clipped leaf polygons into
//! `DynamicMeshClass` fragments), `Release_Fragments`, `Reset_Clip_Pools`, `Process_Clip_Pools`: unreachable in the
//! shipped game; left for the owner if patterns are ever enabled (#206 note).

use crate::ww::plane::PlaneClass;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

const MAX_PASSES: usize = 4;
const MAX_TEX_STAGES: usize = 2;

/// `BPT_FRONT` ... `BPT_COINCIDENCE_EPSILON` (shattersystem.cpp:46).
pub const BPT_FRONT: i32 = 0x01;
pub const BPT_BACK: i32 = 0x02;
pub const BPT_ON: i32 = 0x04;
pub const BPT_BOTH: i32 = 0x08;
pub const BPT_EPSILON: f32 = 0.0001f32;
pub const BPT_COINCIDENCE_EPSILON: f32 = 0.000001f32;
/// `PolygonClass::BPT_POLY_MAX_VERTS`.
pub const BPT_POLY_MAX_VERTS: usize = 24;
/// `MAX_MESH_FRAGMENTS`.
pub const MAX_MESH_FRAGMENTS: usize = 32;

/// `DX8Wrapper::Convert_Color(unsigned)`: packed ARGB to a Vector4 (r, g, b, a).
fn unpack(c: u32) -> [f32; 4] {
    crate::formats::w3d::mesh::dx8_unpack_color(c)
}

/// `class VertexClass` (shattersystem.cpp:100): a vertex being clipped. The copy constructor/assignment copy
/// `DCG`/`DIG` only for `ipass < that.PassCount`; the rest keep their value (Rust copies all of them, the
/// untouched ones being 0xffffffff either way unless the destination had other values: flagged).
#[derive(Clone, Copy, Debug)]
pub struct VertexClass {
    pub position: Vector3,
    pub normal: Vector3,
    pub pass_count: usize,
    pub dcg: [u32; MAX_PASSES],
    pub dig: [u32; MAX_PASSES],
    pub tex_coord: [[Vector2; MAX_TEX_STAGES]; MAX_PASSES],
}

impl VertexClass {
    /// `VertexClass::VertexClass()`: position 0, normal (0,0,1), colours 0xffffffff, UVs 0.
    pub fn new() -> Self {
        VertexClass {
            position: Vector3::new(0.0, 0.0, 0.0),
            normal: Vector3::new(0.0, 0.0, 1.0),
            pass_count: 0,
            dcg: [0xffff_ffff; MAX_PASSES],
            dig: [0xffff_ffff; MAX_PASSES],
            tex_coord: [[Vector2::new(0.0, 0.0); MAX_TEX_STAGES]; MAX_PASSES],
        }
    }

    /// `VertexClass::Lerp(v0, v1, lerp, res)` (shattersystem.cpp:158): positions and normals lerp (normal then
    /// normalised); UVs lerp; the colours are NOT written (see the note in the body).
    pub fn lerp(v0: &VertexClass, v1: &VertexClass, lerp: f32) -> VertexClass {
        debug_assert!(lerp >= -BPT_EPSILON);
        debug_assert!(lerp <= 1.0 + BPT_EPSILON);
        debug_assert!(v0.pass_count == v1.pass_count);
        let mut res = VertexClass::new();
        res.pass_count = v0.pass_count;
        res.position = Vector3::lerp(&v0.position, &v1.position, lerp);
        res.normal = Vector3::lerp(&v0.normal, &v1.normal, lerp);
        res.normal.normalize();
        for ipass in 0..v0.pass_count {
            // Upstream: `Vector4::Lerp(dcg_v0, dcg_v1, res->DCG[ipass])` (and the same for DIG). The third argument is the
            // lerp factor slot and the result is discarded, so `res->DCG/DIG` are never written: they keep their
            // constructor value 0xffffffff. Kept as is (a defect in unreachable code, see the module note); the
            // unpacked colours are computed and unused, as upstream computes them.
            let _unused = (unpack(v0.dcg[ipass]), unpack(v1.dcg[ipass]), unpack(v0.dig[ipass]), unpack(v1.dig[ipass]));
            for istage in 0..MAX_TEX_STAGES {
                res.tex_coord[ipass][istage] = Vector2::lerp(&v0.tex_coord[ipass][istage], &v1.tex_coord[ipass][istage], lerp);
            }
        }
        res
    }

    /// `VertexClass::Which_Side(plane)`: `BPT_FRONT`/`BPT_BACK`/`BPT_ON` within `BPT_EPSILON`.
    pub fn which_side(&self, plane: &PlaneClass) -> i32 {
        let mut d = Vector3::dot_product(&plane.n, &self.position);
        d -= plane.d;
        if d > BPT_EPSILON {
            return BPT_FRONT;
        }
        if d < -BPT_EPSILON {
            return BPT_BACK;
        }
        BPT_ON
    }

    /// `VertexClass::Intersect_Plane(p0, p1, plane, res)`: `Compute_Intersection` then `Lerp` (the alpha stays 0 when
    /// the segment is parallel, as `float alpha = 0.0f` upstream).
    pub fn intersect_plane(p0: &VertexClass, p1: &VertexClass, plane: &PlaneClass) -> VertexClass {
        let (_, t) = plane.compute_intersection(&p0.position, &p1.position);
        let alpha = t.unwrap_or(0.0);
        VertexClass::lerp(p0, p1, alpha)
    }
}

impl Default for VertexClass {
    fn default() -> Self {
        Self::new()
    }
}

/// `class PolygonClass` (shattersystem.cpp:128): a convex polygon of up to 24 vertices with a material id and plane.
#[derive(Clone, Debug)]
pub struct PolygonClass {
    pub material_id: i32,
    pub num_verts: usize,
    pub verts: Vec<VertexClass>,
    pub plane: PlaneClass,
}

impl PolygonClass {
    pub fn new() -> Self {
        PolygonClass { material_id: 0, num_verts: 0, verts: vec![VertexClass::new(); BPT_POLY_MAX_VERTS], plane: PlaneClass::default() }
    }

    /// `PolygonClass(const VertexClass *points, int num)`.
    pub fn from_points(points: &[VertexClass], num: usize) -> Self {
        let mut p = Self::new();
        p.num_verts = num;
        for i in 0..num {
            p.verts[i] = points[i];
        }
        p
    }

    pub fn get_vertex_count(&self) -> usize {
        self.num_verts
    }

    /// `PolygonClass::Compute_Plane` (shattersystem.cpp:325): Newell's method in `double`, the plane through the
    /// centroid.
    pub fn compute_plane(&mut self) {
        let (mut nx, mut ny, mut nz) = (0.0f64, 0.0f64, 0.0f64);
        let (mut ax, mut ay, mut az) = (0.0f64, 0.0f64, 0.0f64);
        for i in 0..self.num_verts {
            let j = (i + 1) % self.num_verts;
            let (pi, pj) = (self.verts[i].position, self.verts[j].position);
            nx += (pi.y - pj.y) as f64 * (pi.z + pj.z) as f64;
            ny += (pi.z - pj.z) as f64 * (pi.x + pj.x) as f64;
            nz += (pi.x - pj.x) as f64 * (pi.y + pj.y) as f64;
            ax += pi.x as f64;
            ay += pi.y as f64;
            az += pi.z as f64;
        }
        ax /= self.num_verts as f64;
        ay /= self.num_verts as f64;
        az /= self.num_verts as f64;
        // WWMath::Sqrt(double) -> float sqrt of the converted argument
        let len = wwmath::sqrt((nx * nx + ny * ny + nz * nz) as f32) as f64;
        nx /= len;
        ny /= len;
        nz /= len;
        let n = Vector3::new(nx as f32, ny as f32, nz as f32);
        let p = Vector3::new(ax as f32, ay as f32, az as f32);
        // PlaneClass::Set(normal, point)
        self.plane = PlaneClass { n, d: Vector3::dot_product(&n, &p) };
    }

    /// `PolygonClass::Which_Side(plane)`: `BPT_ON`, `BPT_FRONT`, `BPT_BACK` or `BPT_BOTH`.
    pub fn which_side(&self, plane: &PlaneClass) -> i32 {
        let mut side_mask = 0;
        for i in 0..self.num_verts {
            side_mask |= self.verts[i].which_side(plane);
        }
        if side_mask == BPT_ON {
            return BPT_ON;
        }
        if (side_mask & !(BPT_FRONT | BPT_ON)) == 0 {
            return BPT_FRONT;
        }
        if (side_mask & !(BPT_BACK | BPT_ON)) == 0 {
            return BPT_BACK;
        }
        BPT_BOTH
    }

    /// `PolygonClass::Split(plane, front, back)` (shattersystem.cpp:375): walks the loop from the first vertex not
    /// on the plane, emitting vertices into the front/back polygons and the plane intersections.
    pub fn split(&self, plane: &PlaneClass, front: &mut PolygonClass, back: &mut PolygonClass) {
        *front = self.clone();
        *back = self.clone();
        front.num_verts = 0;
        back.num_verts = 0;
        debug_assert!(self.which_side(plane) == BPT_BOTH);

        let n = self.num_verts;
        let mut side = BPT_ON;
        let mut i = 0usize;
        while i < n {
            side = self.verts[i].which_side(plane);
            if side != BPT_ON {
                break;
            }
            i += 1;
        }
        let mut iprev = i;
        let mut sideprev = side;
        let mut sidelastdefinite = 0;
        i = (i + 1) % n;

        for _j in 0..n {
            side = self.verts[i].which_side(plane);
            if sideprev == BPT_FRONT {
                if side == BPT_FRONT {
                    front.verts[front.num_verts] = self.verts[i];
                    front.num_verts += 1;
                } else if side == BPT_ON {
                    sidelastdefinite = BPT_FRONT;
                    front.verts[front.num_verts] = self.verts[i];
                    front.num_verts += 1;
                } else {
                    // side == BPT_BACK
                    let point = VertexClass::intersect_plane(&self.verts[iprev], &self.verts[i], plane);
                    front.verts[front.num_verts] = point;
                    front.num_verts += 1;
                    back.verts[back.num_verts] = point;
                    back.num_verts += 1;
                    back.verts[back.num_verts] = self.verts[i];
                    back.num_verts += 1;
                }
            } else if sideprev == BPT_BACK {
                if side == BPT_FRONT {
                    let point = VertexClass::intersect_plane(&self.verts[iprev], &self.verts[i], plane);
                    back.verts[back.num_verts] = point;
                    back.num_verts += 1;
                    front.verts[front.num_verts] = point;
                    front.num_verts += 1;
                    front.verts[front.num_verts] = self.verts[i];
                    front.num_verts += 1;
                } else if side == BPT_ON {
                    sidelastdefinite = BPT_BACK;
                    back.verts[back.num_verts] = self.verts[i];
                    back.num_verts += 1;
                } else {
                    back.verts[back.num_verts] = self.verts[i];
                    back.num_verts += 1;
                }
            } else if sideprev == BPT_ON {
                if side == BPT_FRONT {
                    if sidelastdefinite == BPT_BACK {
                        front.verts[front.num_verts] = self.verts[iprev];
                        front.num_verts += 1;
                    }
                    front.verts[front.num_verts] = self.verts[i];
                    front.num_verts += 1;
                } else if side == BPT_ON {
                    if sidelastdefinite == BPT_FRONT {
                        front.verts[front.num_verts] = self.verts[i];
                        front.num_verts += 1;
                    } else {
                        back.verts[back.num_verts] = self.verts[i];
                        back.num_verts += 1;
                    }
                } else {
                    // side == BPT_BACK
                    if sidelastdefinite == BPT_FRONT {
                        back.verts[back.num_verts] = self.verts[iprev];
                        back.num_verts += 1;
                    }
                    back.verts[back.num_verts] = self.verts[i];
                    back.num_verts += 1;
                }
            } else {
                debug_assert!(false, "PolygonClass::Split : invalid side");
            }
            sideprev = side;
            iprev = i;
            i = (i + 1) % n;
        }

        front.compute_plane();
        back.compute_plane();
        if front.is_degenerate() {
            front.salvage_degenerate();
        }
        if back.is_degenerate() {
            back.salvage_degenerate();
        }
    }

    /// `PolygonClass::Is_Degenerate` (shattersystem.cpp:480): fewer than three vertices, coincident vertices, or
    /// vertices off the polygon's own (recomputed) plane. May recompute the plane.
    pub fn is_degenerate(&mut self) -> bool {
        if self.num_verts <= 2 {
            return true;
        }
        for i in 0..self.num_verts {
            for j in (i + 1)..self.num_verts {
                let delta = (self.verts[i].position - self.verts[j].position).length();
                if delta < BPT_COINCIDENCE_EPSILON {
                    return true;
                }
            }
        }
        for i in 0..self.num_verts {
            let side = self.verts[i].which_side(&self.plane);
            if side != BPT_ON {
                self.compute_plane();
                if self.verts[i].which_side(&self.plane) != BPT_ON {
                    return true;
                }
            }
        }
        false
    }

    /// `PolygonClass::Salvage_Degenerate` (shattersystem.cpp:515): merges consecutive coincident vertices. The loop
    /// reads `Verts[i+1]` for `i == NumVerts-1` (the slot after the last, stale data); kept: the vector is sized to
    /// `BPT_POLY_MAX_VERTS` so the read is in range unless `NumVerts == 24`.
    pub fn salvage_degenerate(&mut self) -> bool {
        let mut i = 0usize;
        while i < self.num_verts {
            let next = if i + 1 < self.verts.len() { i + 1 } else { i };
            let delta = (self.verts[i].position - self.verts[next].position).length();
            if delta < BPT_COINCIDENCE_EPSILON {
                let mut j = i + 1;
                while j + 1 < self.num_verts {
                    self.verts[j] = self.verts[j + 1];
                    j += 1;
                }
                self.num_verts -= 1;
            } else {
                i += 1;
            }
        }
        !self.is_degenerate()
    }
}

impl Default for PolygonClass {
    fn default() -> Self {
        Self::new()
    }
}

/// One node of a shatter pattern's BSP (`BSPClass`, shattersystem.cpp:200): a plane, two children or two leaf
/// indices into the clip pools.
#[derive(Clone, Debug)]
pub struct BSPClass {
    pub plane: PlaneClass,
    pub front: Option<std::boxed::Box<BSPClass>>,
    pub back: Option<std::boxed::Box<BSPClass>>,
    pub front_leaf_index: i32,
    pub back_leaf_index: i32,
}

/// The slice of `HTreeClass` the BSP builder reads (htree.h), supplied by the caller.
pub struct ShatterHTree<'a> {
    /// `Num_Pivots()`.
    pub num_pivots: usize,
    /// `Get_Parent_Index(i)`.
    pub parent_index: &'a dyn Fn(usize) -> i32,
    /// `Get_Transform(i)`: the pivot's (z vector, translation).
    pub transform: &'a dyn Fn(usize) -> (Vector3, Vector3),
}

impl BSPClass {
    /// `BSPClass::BSPClass(tree, bone_index, leaf_index)` (shattersystem.cpp:540): the plane is the pivot's z axis
    /// through its origin; the child pivots are split front/back by the plane (the LAST one on each side wins),
    /// missing children become leaf indices handed out in order.
    pub fn new(tree: &ShatterHTree<'_>, bone_index: usize, leaf_index: &mut i32) -> Self {
        let (z, t) = (tree.transform)(bone_index);
        let plane = PlaneClass { n: z, d: Vector3::dot_product(&z, &t) };
        let mut node = BSPClass { plane, front: None, back: None, front_leaf_index: -1, back_leaf_index: -1 };

        let mut front: i32 = -1;
        let mut back: i32 = -1;
        for ibone in 0..tree.num_pivots {
            if (tree.parent_index)(ibone) == bone_index as i32 {
                let point = (tree.transform)(ibone).1;
                // PlaneClass::In_Front(point): dot(point, N) > D
                if Vector3::dot_product(&point, &node.plane.n) > node.plane.d {
                    front = ibone as i32;
                } else {
                    back = ibone as i32;
                }
            }
        }

        if front != -1 {
            node.front = Some(std::boxed::Box::new(BSPClass::new(tree, front as usize, leaf_index)));
        } else {
            node.front_leaf_index = *leaf_index;
            *leaf_index += 1;
        }
        if back != -1 {
            node.back = Some(std::boxed::Box::new(BSPClass::new(tree, back as usize, leaf_index)));
        } else {
            node.back_leaf_index = *leaf_index;
            *leaf_index += 1;
        }
        node
    }

    /// `BSPClass::Clip_Polygon(polygon)` (shattersystem.cpp:588): classify against the plane, split if needed, and
    /// recurse; polygons that reach a leaf (with 3+ vertices) are appended to `clip_pools[leaf]`.
    pub fn clip_polygon(&self, polygon: &PolygonClass, clip_pools: &mut Vec<Vec<PolygonClass>>) {
        let mut front_poly = PolygonClass::new();
        let mut back_poly = PolygonClass::new();
        front_poly.num_verts = 0;
        back_poly.num_verts = 0;

        match polygon.which_side(&self.plane) {
            BPT_FRONT | BPT_ON => front_poly = polygon.clone(),
            BPT_BACK => back_poly = polygon.clone(),
            _ => polygon.split(&self.plane, &mut front_poly, &mut back_poly),
        }

        match &self.front {
            None => {
                if front_poly.get_vertex_count() >= 3 {
                    clip_pools[self.front_leaf_index as usize].push(front_poly);
                }
            }
            Some(f) => {
                if front_poly.get_vertex_count() >= 3 {
                    f.clip_polygon(&front_poly, clip_pools);
                }
            }
        }
        match &self.back {
            None => {
                if back_poly.get_vertex_count() >= 3 {
                    clip_pools[self.back_leaf_index as usize].push(back_poly);
                }
            }
            Some(b) => {
                if back_poly.get_vertex_count() >= 3 {
                    b.clip_polygon(&back_poly, clip_pools);
                }
            }
        }
    }
}

/// The shatter system's state (the file-statics `ShatterPatterns`, `ClipPools`).
#[derive(Clone, Debug, Default)]
pub struct ShatterSystem {
    pub shatter_patterns: Vec<BSPClass>,
    pub clip_pools: Vec<Vec<PolygonClass>>,
}

impl ShatterSystem {
    /// `ShatterSystem::Init` (shattersystem.cpp:620): in the shipped game `htree = NULL` (`#if 1`), so no pattern is
    /// loaded and the system stays empty; the clip pools are sized to `MAX_MESH_FRAGMENTS`.
    pub fn init(&mut self) {
        self.clip_pools = vec![Vec::new(); MAX_MESH_FRAGMENTS];
    }

    /// `ShatterSystem::Shutdown`: drops the patterns (and the fragments).
    pub fn shutdown(&mut self) {
        self.shatter_patterns.clear();
        self.clip_pools.clear();
    }

    /// `ShatterSystem::Shatter_Mesh`'s first statement: `if (ShatterPatterns.Count() == 0) return;`. The rest of the
    /// function is not translated (see the module note).
    pub fn shatter_mesh_would_run(&self) -> bool {
        !self.shatter_patterns.is_empty()
    }
}
