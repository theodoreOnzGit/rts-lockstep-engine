#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/mesh.cpp (MeshClass::Cast_Ray, MeshClass::Init's collision type)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshgeometry.cpp (Cast_Ray, cast_ray_brute_force, the planes)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtree.h, aabtree.cpp (Cast_Ray, Cast_Ray_Recursive,
//     Cast_Ray_To_Polys)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/tri.cpp (Contains_Point, Find_Dominant_Plane), lineseg.cpp
//     (LineSegClass::Set(that, tm)), colmathline.cpp (Collide(LineSegClass, TriClass)), colmathaabox.cpp
//     (Overlap_Test(AABoxClass, LineSegClass))
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::mesh_cast` and `ww::tri::TriClass` (GitHub #149) against the C++.
//!
//! # Method
//!
//! The C++ side is the cuts `differential/wrap_mesh_cast.cpp` lists, verbatim, on the real WWMath line segment, cast
//! result, triangle and collision headers, WW3D2's `RayCollisionTestClass`, and upstream_wwmath's `Matrix3D`
//! (`Obj_Look_At`, `Get_Inverse`), with stand-ins for the mesh, its geometry and its AABTree. Both sides get the same
//! random mesh: 3 to 24 vertices (uniform in a cube, or on an integer grid so that edges, vertices and planes are met
//! exactly), 1 to 20 triangles (some degenerate: a repeated index), random surface types, flags (sometimes `ALIGNED`,
//! `ORIENTED` or both), W3dAttributes, hidden, alpha and animation-hidden bits; two meshes in three get an AABTree,
//! built here by a median split (test scaffolding, not upstream's `AABTreeBuilderClass`), its leaves 1 to 4 polygons,
//! and in one tree in four some nodes' boxes are shrunk or moved so that the cull rejects polygons the ray crosses.
//! Then 1 to 30 steps of: `MeshClass::Cast_Ray` (a random transform: the identity, a rotation and translation, or a
//! random matrix; the ray aimed at a triangle's inside, an edge, a vertex, along its plane (parallel), from behind
//! (the back face), from inside the mesh's span, zero-length, or anywhere; a random starting result, sometimes
//! `StartBad`; random collision type and checks; with or without the tree), `MeshGeometryClass::Cast_Ray` (the same in
//! the mesh's frame, cast twice, by the tree and by brute force), `Collide(LineSegClass, TriClass)` (with
//! `Compute_Normal` or a given normal, unnormalised or random), `Contains_Point` and `Find_Dominant_Plane` (normals with
//! tied components), `Overlap_Test(min, max, LineSegClass)` (boxes about the ray's ends, inverted boxes), the segment's
//! transform and the mesh's collision type. Every output word is compared bit for bit. The tree-versus-brute-force
//! agreement (the hit and its fraction) is tallied from the C++'s words on trees with exact boxes.
//!
//! # Predictions (written before the first run, 2026-11-17)
//!
//! - 0 mismatches over 20,000 sequences: the same `float` operations in the same order (the plane's dot products,
//!   `num / den`, `Compute_Normal`'s cross product and `Normalize` through the portable `Inv_Sqrt`, the 2D crosses, the
//!   separating-axis products, `Transform_Vector`, `Get_Inverse` through the same `D3DXMatrixInverse` stand-in).
//! - Tree versus brute force (exact boxes): the same hit and fraction on every cast but ties on `t` (the normal or the
//!   surface type may differ there, never the fraction), so 0 differences in (hit, fraction).
//! - Every plant caught (see the table): 10356 by rays ending short of a triangle; 10357 by ties (the shared edges of
//!   grid meshes, rays ending on a triangle); 10358 by points on edges; 10359 by tied normals; 10360 by boxes the three
//!   axes do not separate; 10361 by ends on a box's face; 10362 by any non-orthonormal transform; 10363 by alpha meshes
//!   with translucency checked; 10364 by `ORIENTED` meshes; 10365 by rotated meshes hit; 10366 by trees whose front and
//!   back both hold polygons on the ray; 10367 by leaves with two polygons hit; 10368 by brute-force casts at polygon 0;
//!   10369 by every hit's normal; 10370 by the collision type of an attribute with no collision bits.
//!
//! # Results (2026-11-17, `MESH_CAST_CASES=20000`, the default)
//!
//! 20,000 sequences: 129,186 `MeshClass::Cast_Ray`s (15,312 hit), 103,918 `MeshGeometryClass::Cast_Ray`s (28,173 hit),
//! 25,691 triangle collides (1,072 true), 25,671 `Contains_Point`s (3,188 inside), 25,713 box-line overlaps, 25,897
//! segment transforms and 25,564 collision types: **0 mismatches**, bit for bit, on the first run, as predicted.
//!
//! **Missed predictions:**
//! - **Tree versus brute force.** Predicted 0 differences in (hit, fraction); measured, of 25,260 pairs on exact trees
//!   (pairs whose result was `StartBad` from the start left out: each path then stops after its first polygon, as
//!   upstream's do): the tree **missed a hit 37 times** and **found a farther hit 30 times**; brute force never missed
//!   one; 100 pairs differ only in the normal, contact point or surface type (ties on `t`). Both sides agree bit for bit
//!   on each, so this is upstream's AABTree, not the port. Hypothesis (not proven): the cull's float arithmetic
//!   (`Overlap_Test(AABox, LineSeg)`'s separating axes) rejects a ray that meets a polygon on its node box's boundary.
//!   Evidence: 43 of the 67 are on integer-grid meshes, whose boxes are often flat; aiming every ray at a triangle's
//!   inside (no edges or vertices) took the 67 down to 16, 13 of them on grid meshes.
//! - **Plant 10356** (`t > 1` not rejected) was **not caught** by the first generator (0 of 20,000): `t > 1` can only
//!   matter when the starting `Fraction` is above 1, which no fresh result has. The generator now also starts results
//!   above 1 (as the pick test's collides do), and the table below is from that run.
//!
//! Plants, mismatching sequences of 20,000: 10356 1,321; 10357 2,332; 10358 5,871; 10359 10,916; 10360 1,975; 10361
//! 277; 10362 10,347; 10363 1,030; 10364 855; 10365 7,698; 10366 2,835; 10367 668; 10368 2,501; 10369 13,184; 10370
//! 13,145. **15 of 15 caught.**
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 10356 | `Collide(line, tri)` does not reject `t > 1` |
//! | 10357 | `Collide(line, tri)` keeps a hit at `t == Fraction` (`<=`) |
//! | 10358 | `Contains_Point` counts a point on an edge's line as one side only (`cross > 0`) |
//! | 10359 | `find_dominant_plane` takes the later of tied components (`>=`) |
//! | 10360 | `Overlap_Test(AABox, LineSeg)` skips the three cross-axis tests |
//! | 10361 | `Overlap_Test(AABox, point)` counts a point on a face as outside (`>=`) |
//! | 10362 | `LineSegClass::Set(that, tm)` recomputes `Dir` from `DP` instead of rotating it |
//! | 10363 | `MeshClass::Cast_Ray` ignores `Is_Alpha` |
//! | 10364 | `MeshClass::Cast_Ray` turns an `ORIENTED` mesh to the ray's end, not its start |
//! | 10365 | `MeshClass::Cast_Ray` does not rotate the hit's normal back to the world |
//! | 10366 | `Cast_Ray_Recursive` skips the back child once the front one hit |
//! | 10367 | `Cast_Ray_To_Polys` reports the surface type of the first polygon hit, not the last |
//! | 10368 | `cast_ray_brute_force` skips polygon 0 |
//! | 10369 | the triangle normals are not normalised |
//! | 10370 | `MeshClass::Init`'s `Set_Collision_Type` does not force `COLL_TYPE_ALL` |

mod support;

use std::sync::Arc;

use rts_lockstep_engine::client::mesh_cast::*;
use rts_lockstep_engine::client::pick_drawable::{coll_type, CastResult, LineSeg, RayTest};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::mesh::AabNode;
use rts_lockstep_engine::formats::w3d::meshmdl::CullTree;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::tri::TriClass;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

const STEP: usize = 32;

fn rb(i: i32) -> f32 {
    f32::from_bits(i as u32)
}
fn bits(f: f32) -> i32 {
    f.to_bits() as i32
}
fn v3(w: &[i32]) -> Vector3 {
    Vector3::new(rb(w[0]), rb(w[1]), rb(w[2]))
}
fn push_v(out: &mut Vec<i32>, v: &Vector3) {
    out.extend([bits(v.x), bits(v.y), bits(v.z)]);
}
fn mat(w: &[i32]) -> Matrix3D {
    let mut a = [0.0f32; 12];
    for (i, x) in a.iter_mut().enumerate() {
        *x = rb(w[i]);
    }
    Matrix3D::from_array(a)
}
fn push_result(out: &mut Vec<i32>, r: &CastResult) {
    out.extend([i32::from(r.start_bad), bits(r.fraction)]);
    push_v(out, &r.normal);
    push_v(out, &r.contact_point);
    out.push(r.surface_type as i32);
}

struct Parsed {
    with_tree: Arc<MeshGeometry>,
    without_tree: Arc<MeshGeometry>,
    w3d_attributes: u32,
    hidden: bool,
    alpha: bool,
    anim_hidden: bool,
    steps: usize,
}

fn parse(input: &[i32]) -> Parsed {
    let (nv, np, nn, ni) = (input[0] as usize, input[1] as usize, input[2] as usize, input[3] as usize);
    let mut k = 9;
    let mut vertices = Vec::new();
    for _ in 0..nv {
        vertices.push([rb(input[k]), rb(input[k + 1]), rb(input[k + 2])]);
        k += 3;
    }
    let mut polys = Vec::new();
    for _ in 0..np {
        polys.push([input[k] as u16, input[k + 1] as u16, input[k + 2] as u16]);
        k += 3;
    }
    let surface_types: Vec<u8> = input[k..k + np].iter().map(|&s| s as u8).collect();
    k += np;
    let mut nodes = Vec::new();
    for _ in 0..nn {
        let w = &input[k..k + 8];
        nodes.push(AabNode { min: [rb(w[0]), rb(w[1]), rb(w[2])], max: [rb(w[3]), rb(w[4]), rb(w[5])], front_or_poly0: w[6] as u32, back_or_poly_count: w[7] as u32 });
        k += 8;
    }
    let poly_indices: Vec<u32> = input[k..k + ni].iter().map(|&i| i as u32).collect();
    k += ni;
    let without = MeshGeometry { flags: input[4] as u32, vertices, polys, surface_types, cull_tree: None };
    let mut with = without.clone();
    if nn > 0 {
        with.cull_tree = Some(CullTree { nodes, poly_indices });
    }
    Parsed { with_tree: Arc::new(with), without_tree: Arc::new(without), w3d_attributes: input[5] as u32, hidden: input[6] != 0, alpha: input[7] != 0, anim_hidden: input[8] != 0, steps: k }
}

fn tri_of(w: &[i32], n: Vector3) -> TriClass {
    TriClass { n, v: [v3(&w[0..3]), v3(&w[3..6]), v3(&w[6..9])] }
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    let p = parse(input);
    for s in input[p.steps..].chunks(STEP) {
        if s.len() < STEP {
            break;
        }
        let w = &s[1..];
        match s[0] {
            0 => {
                let model = if w[24] != 0 { p.with_tree.clone() } else { p.without_tree.clone() };
                let m = MeshRenderObj { collision_type: coll_type::ALL, is_alpha: p.alpha, hidden: p.hidden, animation_hidden: p.anim_hidden, transform: mat(&w[0..12]), model };
                let res = CastResult { fraction: rb(w[18]), start_bad: w[19] != 0, compute_contact_point: w[20] != 0, ..CastResult::default() };
                let mut rt = RayTest::new(LineSeg::new(v3(&w[12..15]), v3(&w[15..18])), res, w[21], w[22] != 0, w[23] != 0);
                let hit = m.cast_ray(&mut rt);
                out.extend([0, i32::from(hit)]);
                push_result(&mut out, &rt.result);
                out.push(i32::from(hit));
            }
            1 => {
                let g = if w[9] != 0 { &p.with_tree } else { &p.without_tree };
                let res = CastResult { fraction: rb(w[6]), start_bad: w[7] != 0, compute_contact_point: w[8] != 0, ..CastResult::default() };
                let mut rt = RayTest::new(LineSeg::new(v3(&w[0..3]), v3(&w[3..6])), res, coll_type::ALL, false, false);
                let hit = g.cast_ray(&mut rt);
                out.extend([1, i32::from(hit)]);
                push_result(&mut out, &rt.result);
            }
            2 => {
                let line = LineSeg::new(v3(&w[0..3]), v3(&w[3..6]));
                let mut tri = tri_of(&w[6..15], v3(&w[16..19]));
                if w[15] == 0 {
                    tri.compute_normal();
                }
                let mut res = CastResult { fraction: rb(w[19]), compute_contact_point: w[20] != 0, ..CastResult::default() };
                let r = collide_line_tri(&line, &tri, &mut res);
                out.extend([2, i32::from(r), bits(res.fraction)]);
                push_v(&mut out, &res.normal);
                push_v(&mut out, &res.contact_point);
            }
            3 => {
                let tri = tri_of(&w[0..9], v3(&w[9..12]));
                let (a1, a2) = tri.find_dominant_plane();
                out.extend([3, i32::from(tri.contains_point(&v3(&w[12..15]))), a1 as i32, a2 as i32]);
            }
            4 => {
                let line = LineSeg::new(v3(&w[6..9]), v3(&w[9..12]));
                out.extend([4, overlap_test_min_max_line(&v3(&w[0..3]), &v3(&w[3..6]), &line) as i32]);
            }
            5 => {
                let t = LineSeg::transformed(&LineSeg::new(v3(&w[12..15]), v3(&w[15..18])), &mat(&w[0..12]));
                out.push(5);
                for v in [t.p0, t.p1, t.dp, t.dir] {
                    push_v(&mut out, &v);
                }
                out.push(bits(t.length));
            }
            6 => out.extend([6, mesh_collision_type(w[0] & 0xFF, w[1] as u32)]),
            _ => {}
        }
    }
    out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 100_000];
    let n = unsafe { cpp::ref_mesh_cast_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

/// The generated mesh, kept for aiming rays.
struct GenMesh {
    vertices: Vec<[f32; 3]>,
    polys: Vec<[u16; 3]>,
    exact_tree: bool,
    grid: bool,
}

fn point(g: &mut Gen, r: f32) -> [f32; 3] {
    [g.uniform(-r, r), g.uniform(-r, r), g.uniform(-r, r)]
}

/// A median-split tree over `polys` (test scaffolding): appends nodes in preorder, the leaves' polygons to `indices`.
fn build_tree(g: &mut Gen, m: &GenMesh, polys: &mut [u32], nodes: &mut Vec<[i32; 8]>, indices: &mut Vec<u32>, leaf: usize, perturb: bool) {
    let me = nodes.len();
    nodes.push([0; 8]);
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for &p in polys.iter() {
        for &vi in &m.polys[p as usize] {
            let v = m.vertices[vi as usize];
            for a in 0..3 {
                lo[a] = lo[a].min(v[a]);
                hi[a] = hi[a].max(v[a]);
            }
        }
    }
    if perturb && g.below(3) == 0 {
        for a in 0..3 {
            let d = (hi[a] - lo[a]) * 0.4;
            lo[a] += g.uniform(-d, d);
            hi[a] += g.uniform(-d, d);
        }
    }
    let mut n = [bits(lo[0]), bits(lo[1]), bits(lo[2]), bits(hi[0]), bits(hi[1]), bits(hi[2]), 0, 0];
    if polys.len() <= leaf {
        n[6] = (indices.len() as u32 | AABTREE_LEAF_FLAG) as i32;
        n[7] = polys.len() as i32;
        indices.extend(polys.iter());
        nodes[me] = n;
        return;
    }
    let axis = (0..3).max_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b]))).unwrap_or(0);
    let centre = |p: u32| m.polys[p as usize].iter().map(|&vi| m.vertices[vi as usize][axis]).sum::<f32>();
    polys.sort_by(|&a, &b| centre(a).total_cmp(&centre(b)));
    let mid = polys.len() / 2;
    let (front, back) = polys.split_at_mut(mid);
    n[6] = nodes.len() as i32;
    build_tree(g, m, front, nodes, indices, leaf, perturb);
    n[7] = nodes.len() as i32;
    build_tree(g, m, back, nodes, indices, leaf, perturb);
    nodes[me] = n;
}

fn gen_mesh(g: &mut Gen) -> (Vec<i32>, GenMesh) {
    let nv = 3 + g.below(22) as usize;
    let grid = g.below(2) == 0;
    let vertices: Vec<[f32; 3]> = (0..nv)
        .map(|_| if grid { [g.below(7) as f32 - 3.0, g.below(7) as f32 - 3.0, g.below(7) as f32 - 3.0] } else { point(g, 4.0) })
        .collect();
    let np = 1 + g.below(20) as usize;
    let polys: Vec<[u16; 3]> = (0..np)
        .map(|_| {
            let a = g.below(nv as u32) as u16;
            let b = g.below(nv as u32) as u16;
            let c = if g.below(15) == 0 { a } else { g.below(nv as u32) as u16 };
            [a, b, c]
        })
        .collect();
    let flags: u32 = match g.below(10) {
        0 => 0x200,
        1 => 0x800,
        2 => 0xA00,
        3 => g.u32() & !0x200 & !0x800,
        _ => 0,
    };
    let attrs = if g.below(3) == 0 { g.u32() } else { 0 };
    let mut m = GenMesh { vertices, polys, exact_tree: false, grid };
    let (mut nodes, mut indices) = (Vec::new(), Vec::new());
    if g.below(3) != 0 {
        let perturb = g.below(4) == 0;
        let mut ps: Vec<u32> = (0..np as u32).collect();
        let leaf = 1 + g.below(4) as usize;
        build_tree(g, &m, &mut ps, &mut nodes, &mut indices, leaf, perturb);
        m.exact_tree = !perturb;
    }
    let mut v = vec![nv as i32, np as i32, nodes.len() as i32, indices.len() as i32, flags as i32, attrs as i32, i32::from(g.below(10) == 0), i32::from(g.below(6) == 0), i32::from(g.below(12) == 0)];
    for p in &m.vertices {
        v.extend(p.iter().map(|&f| bits(f)));
    }
    for p in &m.polys {
        v.extend(p.iter().map(|&i| i32::from(i)));
    }
    for _ in 0..np {
        v.push(g.below(256) as i32);
    }
    for n in &nodes {
        v.extend(n);
    }
    v.extend(indices.iter().map(|&i| i as i32));
    (v, m)
}

fn transform(g: &mut Gen) -> [f32; 12] {
    use rts_lockstep_engine::ww::wwmath::{cos, sin};
    match g.below(4) {
        0 => [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        1 => {
            let mut a = [0.0f32; 12];
            for x in a.iter_mut() {
                *x = g.uniform(-2.0, 2.0);
            }
            a
        }
        _ => {
            let (az, ax) = (g.uniform(-3.2, 3.2), g.uniform(-3.2, 3.2));
            let (s, c, s2, c2) = (sin(az), cos(az), sin(ax), cos(ax));
            let z = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
            let x = [[1.0, 0.0, 0.0], [0.0, c2, -s2], [0.0, s2, c2]];
            let mut r = [[0.0f32; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    r[i][j] = z[i][0] * x[0][j] + z[i][1] * x[1][j] + z[i][2] * x[2][j];
                }
            }
            let t = point(g, 50.0);
            [r[0][0], r[0][1], r[0][2], t[0], r[1][0], r[1][1], r[1][2], t[1], r[2][0], r[2][1], r[2][2], t[2]]
        }
    }
}

fn apply(t: &[f32; 12], p: [f32; 3]) -> [f32; 3] {
    let v = Matrix3D::transform_vector(&Matrix3D::from_array(*t), &Vector3::new(p[0], p[1], p[2]));
    [v.x, v.y, v.z]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A ray in the mesh's frame: (start, end).
fn ray(g: &mut Gen, m: &GenMesh) -> ([f32; 3], [f32; 3]) {
    let p = m.polys[g.below(m.polys.len() as u32) as usize];
    let v = |i: usize| m.vertices[p[i] as usize];
    let target = match g.below(5) {
        0 => {
            // inside
            let (a, b) = (g.uniform(0.0, 1.0), g.uniform(0.0, 1.0));
            let (a, b) = if a + b > 1.0 { (1.0 - a, 1.0 - b) } else { (a, b) };
            let (e1, e2) = (sub(v(1), v(0)), sub(v(2), v(0)));
            [v(0)[0] + a * e1[0] + b * e2[0], v(0)[1] + a * e1[1] + b * e2[1], v(0)[2] + a * e1[2] + b * e2[2]]
        }
        1 => {
            // on an edge (its midpoint, exact on a grid)
            let (i, j) = match g.below(3) {
                0 => (0, 1),
                1 => (1, 2),
                _ => (2, 0),
            };
            [(v(i)[0] + v(j)[0]) * 0.5, (v(i)[1] + v(j)[1]) * 0.5, (v(i)[2] + v(j)[2]) * 0.5]
        }
        2 => v(g.below(3) as usize),
        _ => point(g, 4.0),
    };
    let dir = match g.below(6) {
        // along the triangle's plane: parallel
        0 => sub(v(1), v(0)),
        1 => [0.0, 0.0, if g.below(2) == 0 { 1.0 } else { -1.0 }],
        2 => [1.0, 0.0, 0.0],
        _ => point(g, 1.0),
    };
    let d = g.uniform(0.5, 20.0);
    let start = match g.below(10) {
        0 => target,
        1 => point(g, 4.0),
        _ => [target[0] - dir[0] * d, target[1] - dir[1] * d, target[2] - dir[2] * d],
    };
    let f = match g.below(5) {
        0 => 1.0,
        1 => 0.0,
        2 => g.uniform(0.2, 1.0),
        _ => g.uniform(1.0, 4.0),
    };
    let end = if start == target { [start[0] + dir[0] * d, start[1] + dir[1] * d, start[2] + dir[2] * d] } else { [start[0] + (target[0] - start[0]) * f, start[1] + (target[1] - start[1]) * f, start[2] + (target[2] - start[2]) * f] };
    (start, end)
}

/// A starting `Fraction`: 1 (a fresh result), below 1 (an earlier hit), or above 1 (a caller's own, as
/// `differential_pick_drawable`'s collides use; the only way `t > 1` can matter, see plant 10356).
fn fraction(g: &mut Gen) -> f32 {
    match g.below(6) {
        0 => g.uniform(0.0, 1.0),
        1 => g.uniform(1.0, 4.0),
        _ => 1.0,
    }
}

fn normal(g: &mut Gen) -> [f32; 3] {
    match g.below(6) {
        0 => [1.0, 1.0, 0.0],
        1 => [-1.0, 1.0, 1.0],
        2 => [0.0, 0.0, 0.0],
        3 => [0.0, -2.0, 2.0],
        _ => point(g, 1.0),
    }
}

fn gen_case(g: &mut Gen) -> (Vec<i32>, GenMesh, Vec<usize>) {
    let (mut v, m) = gen_mesh(g);
    let mut pairs = Vec::new();
    let mut nsteps = 0usize;
    let steps = 1 + g.below(30);
    for _ in 0..steps {
        let mut s = [0i32; STEP];
        let kind = match g.below(12) {
            0..=4 => 0,
            5..=6 => 1,
            7 => 2,
            8 => 3,
            9 => 4,
            10 => 5,
            _ => 6,
        };
        s[0] = kind;
        let w = &mut s[1..];
        match kind {
            0 => {
                let t = transform(g);
                let (a, b) = ray(g, &m);
                let (a, b) = (apply(&t, a), apply(&t, b));
                for i in 0..12 {
                    w[i] = bits(t[i]);
                }
                for i in 0..3 {
                    w[12 + i] = bits(a[i]);
                    w[15 + i] = bits(b[i]);
                }
                w[18] = bits(fraction(g));
                w[19] = i32::from(g.below(20) == 0);
                w[20] = g.below(2) as i32;
                w[21] = match g.below(6) {
                    0 => g.u32() as i32,
                    1 => 0,
                    _ => coll_type::ALL,
                };
                w[22] = g.below(2) as i32;
                w[23] = g.below(2) as i32;
                w[24] = i32::from(g.below(4) != 0);
            }
            1 => {
                let (a, b) = ray(g, &m);
                for i in 0..3 {
                    w[i] = bits(a[i]);
                    w[3 + i] = bits(b[i]);
                }
                w[6] = bits(fraction(g));
                w[7] = i32::from(g.below(30) == 0);
                w[8] = g.below(2) as i32;
                // the same cast by the tree, then by brute force (below)
                w[9] = 1;
            }
            2 => {
                let p = m.polys[g.below(m.polys.len() as u32) as usize];
                let tri: Vec<f32> = if g.below(3) == 0 { (0..9).map(|_| g.uniform(-4.0, 4.0)).collect() } else { p.iter().flat_map(|&i| m.vertices[i as usize]).collect() };
                let (a, b) = ray(g, &m);
                for i in 0..3 {
                    w[i] = bits(a[i]);
                    w[3 + i] = bits(b[i]);
                }
                for i in 0..9 {
                    w[6 + i] = bits(tri[i]);
                }
                w[15] = g.below(2) as i32;
                let n = if g.below(2) == 0 { normal(g) } else { [g.uniform(-3.0, 3.0), g.uniform(-3.0, 3.0), g.uniform(-3.0, 3.0)] };
                for i in 0..3 {
                    w[16 + i] = bits(n[i]);
                }
                w[19] = bits(fraction(g));
                w[20] = g.below(2) as i32;
            }
            3 => {
                let p = m.polys[g.below(m.polys.len() as u32) as usize];
                for (k, &i) in p.iter().enumerate() {
                    for a in 0..3 {
                        w[3 * k + a] = bits(m.vertices[i as usize][a]);
                    }
                }
                let n = normal(g);
                let (a, _) = ray(g, &m);
                let q = if g.below(2) == 0 { m.vertices[p[0] as usize] } else { a };
                for i in 0..3 {
                    w[9 + i] = bits(n[i]);
                    w[12 + i] = bits(if g.below(3) == 0 { q[i] } else { a[i] });
                }
            }
            4 => {
                let (a, b) = ray(g, &m);
                let c = if g.below(2) == 0 { a } else { point(g, 4.0) };
                let e = [g.uniform(-0.5, 3.0), g.uniform(-0.5, 3.0), g.uniform(-0.5, 3.0)];
                let (lo, hi) = if g.below(4) == 0 { ([c[0], c[1], c[2]], [c[0] + e[0], c[1] + e[1], c[2] + e[2]]) } else { ([c[0] - e[0], c[1] - e[1], c[2] - e[2]], [c[0] + e[0], c[1] + e[1], c[2] + e[2]]) };
                for i in 0..3 {
                    w[i] = bits(lo[i]);
                    w[3 + i] = bits(hi[i]);
                    w[6 + i] = bits(a[i]);
                    w[9 + i] = bits(b[i]);
                }
            }
            5 => {
                let t = transform(g);
                let (a, b) = ray(g, &m);
                for i in 0..12 {
                    w[i] = bits(t[i]);
                }
                for i in 0..3 {
                    w[12 + i] = bits(a[i]);
                    w[15 + i] = bits(b[i]);
                }
            }
            _ => {
                w[0] = match g.below(3) {
                    0 => 0,
                    1 => coll_type::ALL,
                    _ => g.u32() as i32,
                };
                w[1] = match g.below(3) {
                    0 => 0,
                    1 => (g.below(256) << 4) as i32,
                    _ => g.u32() as i32,
                };
            }
        }
        if kind == 1 {
            v.extend(s);
            pairs.push(nsteps);
            nsteps += 1;
            s[10] = 0;
        }
        v.extend(s);
        nsteps += 1;
    }
    (v, m, pairs)
}

/// The output words of each step of `out` (the C++'s), by their kinds' widths.
fn step_words(out: &[i32]) -> Vec<&[i32]> {
    let mut k = 0;
    let mut steps = Vec::new();
    while k < out.len() {
        let n = match out[k] {
            0 => 12,
            1 => 11,
            2 => 9,
            3 => 4,
            5 => 14,
            _ => 2,
        };
        steps.push(&out[k..(k + n).min(out.len())]);
        k += n;
    }
    steps
}

#[test]
fn mesh_cast_matches_upstream() {
    let cases: u64 = std::env::var("MESH_CAST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x149_1218);
    let mut whole = Check::new("MeshClass::Cast_Ray and what it reaches");
    let mut counts = [0u64; 7];
    let (mut mesh_casts, mut mesh_hits, mut geom_hits, mut collides, mut contained) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut pairs_compared, mut tree_missed, mut brute_missed, mut fraction_differs, mut pairs_tie_only, mut on_grid) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let (input, m, pairs) = gen_case(&mut g);
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        let steps = step_words(&c);
        for s in &steps {
            counts[s[0] as usize % 7] += 1;
            match s[0] {
                0 => {
                    mesh_casts += 1;
                    mesh_hits += u64::from(s[1] != 0);
                }
                1 => geom_hits += u64::from(s[1] != 0),
                2 => collides += u64::from(s[1] != 0),
                3 => contained += u64::from(s[1] != 0),
                _ => {}
            }
        }
        if m.exact_tree {
            for &p in &pairs {
                if let (Some(a), Some(b)) = (steps.get(p), steps.get(p + 1)) {
                    // a result StartBad from the start stops each path after its first polygon tested: not compared
                    if a[2] != 0 || b[2] != 0 {
                        continue;
                    }
                    pairs_compared += 1;
                    match (a[1] != 0, b[1] != 0) {
                        (false, true) => {
                            tree_missed += 1;
                            on_grid += u64::from(m.grid);
                        }
                        (true, false) => brute_missed += 1,
                        _ if a[3] != b[3] => {
                            fraction_differs += 1;
                            on_grid += u64::from(m.grid);
                        }
                        _ if a != b => pairs_tie_only += 1,
                        _ => {}
                    }
                }
            }
        }
        if r != c && !planted {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(4);
            println!("sequence {i} first difference at word {k}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 8).min(r.len())], &c[k0.min(c.len())..(k + 8).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences: steps by kind {counts:?}; {mesh_casts} mesh casts ({mesh_hits} hit), {} geometry casts ({geom_hits} hit), {collides} triangle collides true, {contained} points contained", counts[1]);
    println!("tree versus brute force (exact boxes, not StartBad): {pairs_compared} pairs; the tree missed a hit {tree_missed}, brute force missed one {brute_missed}, both hit at different fractions {fraction_differs} (of these two, {on_grid} on grid meshes), only the normal, contact point or surface type differ {pairs_tie_only}");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
