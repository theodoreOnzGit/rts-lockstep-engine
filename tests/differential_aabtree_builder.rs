#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtreebuilder.h, aabtreebuilder.cpp (AABTreeBuilderClass)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/aabtree.cpp (AABTreeClass(AABTreeBuilderClass *),
//     Build_Tree_Recursive) and aabtree.h (CullNodeStruct's setters)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshgeometry.cpp (Generate_Culling_Tree) and meshmdlio.cpp
//     (MeshModelClass::Load_W3D's "If this mesh is collideable and no AABTree was in the file, generate one now")
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `formats::w3d::aabtreebuilder` (GitHub #190) against the C++, and casts through the trees each side built.
//!
//! # Method
//!
//! The C++ side is the cuts `differential/wrap_aabtree_builder.cpp` lists, verbatim, on the real `Vector3`,
//! `Vector3i16`, `AAPlaneClass` and `W3dMeshAABTreeNode`, with stand-ins for the mesh geometry (its arrays, its
//! `CullTree`, its `W3dAttributes`) and for MSVC's `rand()` (the CRT's LCG; glibc's is another generator). Both sides
//! get the same random mesh and `rand()` seed (1 in one case in eight, else random):
//!
//! - polygon counts 0 (1 in 50), 1 to 4 (a leaf at once), 5 to 40, 41 to 200, 201 to 1,500 and 1,501 to 5,000 (1 in 100);
//! - vertices uniform in a cube; on an integer grid (planes through shared vertices, polygons on split planes);
//!   coplanar (one axis constant); tiny (within `COINCIDENCE_EPSILON` of each other: everything `ON`); huge (1e8 to
//!   1e30: from 1e13 or so the volumes overflow to infinity, so no plane beats `FLT_MAX`); on a 0.001 lattice (`Which_Side`'s epsilon met
//!   exactly, give or take rounding); and in 1 case in 100 one vertex infinite or NaN;
//! - degenerate polygons (a repeated index), duplicated runs of polygons, and in 1 case in 20 a mesh of one polygon
//!   repeated (nothing separates: a leaf of all of them);
//! - `W3dAttributes` 0, a collision bit, or random; the file's tree present or not.
//!
//! Compared word for word: `Load_W3D`'s decision and the `rand()` state after it, `Generate_Culling_Tree`'s
//! `NodeCount`, `PolyCount`, every node (`Min`, `Max` bits, `FrontOrPoly0`, `BackOrPolyCount`), the `PolyIndices`, the
//! `rand()` state after it (so the number of draws), and the tools' export (`Build_W3D_AABTree_Recursive`'s nodes and
//! poly indices), which must be the same arrays. Then 0 to 12 ray casts per mesh (`MeshGeometryClass::Cast_Ray`, the
//! #149 cut, `ref_mesh_cast_seq`): the C++ through its tree and by brute force, the Rust through its tree; rays at a
//! polygon's inside, an edge, a vertex, a node box's corner, or anywhere.
//!
//! # Predictions (written before the first run, 2026-11-17)
//!
//! - 0 mismatches over 20,000 meshes: the same `float` subtractions, products (`((dx * dy) * dz) * count`) and sum in
//!   the same order, the same `<`/`>` comparisons (no `min`/`max`), the same draws.
//! - The export's arrays equal `AABTreeClass`'s on every mesh (the two recursions are the same walk).
//! - Casts: 0 mismatches. Tree versus brute force (C++ words): a handful of differences per thousand hits, like #149's
//!   (37 + 30 of 25,260 on test-built trees), from the cull at node boundaries; more on grid and lattice meshes.
//! - Plants 10371-10385 all caught. Expected weakest: 10372 (`<=` keeps a later plane of equal cost: needs exact ties,
//!   grid meshes), 10378 (`>=` at the epsilon: needs a delta of exactly 0.001f, the lattice meshes), 10381 (the back
//!   child's box left out: caught whenever the back child extends the front's).
//!
//! # Results (2026-11-17, `AABTREE_CASES=20000`, the default)
//!
//! 20,000 meshes (7,479 of them generate a tree by `Load_W3D`'s test; the built trees hold 956,750 nodes and 2,615,367
//! poly indices, the largest 3,393 nodes, 7,889 a lone leaf): **0 mismatches**, bit for bit (nodes, poly indices,
//! `rand()` states, decisions), on the first full run, as predicted. The export's arrays equal `AABTreeClass`'s on all
//! 20,000. Casts through the built trees: 116,413 casts on 17,947 meshes, **0 mismatches**.
//!
//! **Missed prediction (tree versus brute force).** Predicted "a handful per thousand hits"; measured on the C++'s
//! words, of 116,413 casts (56,342 hit by brute force): the generated tree **missed a hit 591 times** (1.0 % of hits),
//! **hit at another fraction 82 times**, brute force never missed one, and 4,119 differ only in the normal, contact
//! point or surface type (ties on `t`, mostly duplicated and coplanar polygons). By mesh kind (cube, grid, coplanar,
//! tiny, huge, lattice, mixed): 160, 466, 3,420, 57, 15, 299, 375. More than #149's rate, because one ray in five here
//! is aimed at a node box's corner. It is upstream's cull (both sides agree bit for bit): what the brute-force
//! fallback gave before #190 was not what upstream gives.
//!
//! **Missed design (plants).** The first versions of 10372 (`<=` on any cost), 10375 and 10380 (in one `switch` only)
//! and 10377 (a one-sided split not given `FLT_MAX`) made `Build_Tree` split a node into one child holding all its
//! polygons, forever (upstream's recursion would do the same); a first 10384 (leaves without `AABTREE_LEAF_FLAG`) made
//! the cast's recursion explode. Those runs were stopped and the plants redesigned as in the table.
//!
//! Plants, mismatching meshes of 20,000 (tree words / casts): 10371 11,712 / 1,234; 10372 627 / 5; 10373 14,879 /
//! 1,036; 10374 11,498 / 1,171; 10375 6,457 / 339; 10376 4,776 / 1,090; 10377 4,766 / 1,061; 10378 2,098 / 123;
//! 10379 11,473 / 206; 10380 12,049 / 2,531; 10381 11,495 / 5,615; 10382 10,276 / 1,986; 10383 12,111 / 0; 10384
//! 12,111 / 2,739; 10385 7,566 / 0 (the decision word only). **15 of 15 caught.** 10372 is the weakest, as predicted
//! (exact cost ties); 10378 at 2,098 is stronger than predicted (lattice meshes meet the epsilon often).
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 10371 | `rand()` shifts `_holdrand` by 15, not 16 |
//! | 10372 | `Select_Splitting_Plane` keeps a later plane of equal (finite) cost (`<=`) |
//! | 10373 | `Select_Splitting_Plane` tries 50 planes whatever the polygon count (no `MIN`) |
//! | 10374 | `Select_Splitting_Plane` draws the vertex before the polygon |
//! | 10375 | `Compute_Plane_Score` and `Split_Polys` send an `ON` polygon back |
//! | 10376 | `Compute_Plane_Score` does not inflate the back box |
//! | 10377 | `Compute_Plane_Score` inflates the front box too |
//! | 10378 | `Which_Side` compares the epsilon with `>=` and `<=` |
//! | 10379 | `Build_Tree` splits a node of exactly `MIN_POLYS_PER_NODE` (`<`) |
//! | 10380 | `Compute_Plane_Score` and `Split_Polys` send a spanning (`BOTH`) polygon back |
//! | 10381 | `Compute_Bounding_Box` leaves out the back child's box |
//! | 10382 | `Update_Min_Max` skips the max on an axis where the point was a new min |
//! | 10383 | `Assign_Index` numbers the back child before the front |
//! | 10384 | `Build_Tree_Recursive` swaps an inner node's front and back children |
//! | 10385 | `Load_W3D` generates a tree without a collision bit |

mod support;

use rts_lockstep_engine::client::mesh_cast::MeshGeometry;
use rts_lockstep_engine::client::pick_drawable::{coll_type, CastResult, LineSeg, RayTest};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::aabtreebuilder::{generate_culling_tree, load_w3d_generates_cull_tree, AabTreeBuilder, CrtRand};
use rts_lockstep_engine::formats::w3d::meshmdl::CullTree;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

const STEP: usize = 32;

fn rb(i: i32) -> f32 {
    f32::from_bits(i as u32)
}
fn bits(f: f32) -> i32 {
    f.to_bits() as i32
}

/// One generated mesh.
struct Case {
    verts: Vec<[f32; 3]>,
    polys: Vec<[u16; 3]>,
    attrs: u32,
    file_tree: bool,
    seed: u32,
    kind: u32,
}

impl Case {
    fn input(&self) -> Vec<i32> {
        let mut v = vec![self.verts.len() as i32, self.polys.len() as i32, self.attrs as i32, i32::from(self.file_tree), self.seed as i32];
        for p in &self.verts {
            v.extend(p.iter().map(|&f| bits(f)));
        }
        for p in &self.polys {
            v.extend(p.iter().map(|&i| i32::from(i)));
        }
        v
    }
}

fn push_tree(out: &mut Vec<i32>, t: &CullTree) {
    for n in &t.nodes {
        out.extend(n.min.iter().chain(n.max.iter()).map(|&f| bits(f)));
        out.extend([n.front_or_poly0 as i32, n.back_or_poly_count as i32]);
    }
    out.extend(t.poly_indices.iter().map(|&p| p as i32));
}

/// The Rust's words, in the wrapper's order; and its tree.
fn rust_words(c: &Case) -> (Vec<i32>, CullTree) {
    let mut out = Vec::new();
    let generated = load_w3d_generates_cull_tree(c.attrs, c.file_tree);
    let mut r = CrtRand::new(c.seed);
    if generated {
        let _ = generate_culling_tree(&c.polys, &c.verts, &mut r);
    }
    out.extend([i32::from(generated), r.holdrand as i32]);
    let mut r = CrtRand::new(c.seed);
    let tree = generate_culling_tree(&c.polys, &c.verts, &mut r).expect("indices in range");
    out.extend([tree.nodes.len() as i32, tree.poly_indices.len() as i32]);
    push_tree(&mut out, &tree);
    out.push(r.holdrand as i32);
    // the export: the same builder, the same walk
    let mut b = AabTreeBuilder::default();
    let mut r = CrtRand::new(c.seed);
    assert!(b.build_aabtree(&c.polys, &c.verts, &mut r));
    let w = b.to_cull_tree();
    out.extend([b.node_count() as i32, b.poly_count() as i32]);
    push_tree(&mut out, &w);
    (out, tree)
}

fn cpp_words(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 16];
    loop {
        let n = unsafe { cpp::ref_aabtree_builder(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
        if n <= out.len() {
            out.truncate(n);
            return out;
        }
        out = vec![0; n];
    }
}

/// The C++ tree out of its words (after the two decision words).
fn cpp_tree(w: &[i32]) -> Option<CullTree> {
    use rts_lockstep_engine::formats::w3d::mesh::AabNode;
    let (nn, np) = (*w.get(2)? as usize, *w.get(3)? as usize);
    let mut k = 4;
    let mut nodes = Vec::with_capacity(nn);
    for _ in 0..nn {
        let s = w.get(k..k + 8)?;
        nodes.push(AabNode { min: [rb(s[0]), rb(s[1]), rb(s[2])], max: [rb(s[3]), rb(s[4]), rb(s[5])], front_or_poly0: s[6] as u32, back_or_poly_count: s[7] as u32 });
        k += 8;
    }
    let poly_indices = w.get(k..k + np)?.iter().map(|&p| p as u32).collect();
    Some(CullTree { nodes, poly_indices })
}

fn grid(g: &mut Gen, n: u32) -> f32 {
    g.below(n) as f32 - (n / 2) as f32
}

fn gen_case(g: &mut Gen) -> Case {
    let np = match g.below(100) {
        0..=1 => 0,
        2..=16 => 1 + g.below(4) as usize,
        17..=66 => 5 + g.below(36) as usize,
        67..=91 => 41 + g.below(160) as usize,
        92..=98 => 201 + g.below(1300) as usize,
        _ => 1501 + g.below(3500) as usize,
    };
    let nv = (3 + g.below((2 * np as u32).clamp(1, 3000)) as usize).min(65535);
    let kind = g.below(7);
    let flat_axis = g.below(3) as usize;
    let scale = [1.0e8, 1.0e11, 1.0e12, 1.0e13, 1.0e18, 1.0e30][g.below(6) as usize] * g.uniform(0.5, 1.0);
    let mut verts: Vec<[f32; 3]> = (0..nv)
        .map(|_| {
            let mut p = match kind {
                0 => [g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0)],
                1 => [grid(g, 7), grid(g, 7), grid(g, 7)],
                2 => [g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0), g.uniform(-4.0, 4.0)],
                3 => [g.uniform(0.0, 0.0008), g.uniform(0.0, 0.0008), g.uniform(0.0, 0.0008)],
                4 => [g.uniform(-1.0, 1.0) * scale, g.uniform(-1.0, 1.0) * scale, g.uniform(-1.0, 1.0) * scale],
                5 => [grid(g, 9) * 0.001, grid(g, 9) * 0.001, grid(g, 9) * 0.001],
                _ => [grid(g, 5) * 0.5, g.uniform(-2.0, 2.0), grid(g, 5)],
            };
            if kind == 2 {
                p[flat_axis] = 1.25;
            }
            p
        })
        .collect();
    if g.below(100) == 0 {
        let i = g.below(nv as u32) as usize;
        verts[i][g.below(3) as usize] = match g.below(3) {
            0 => f32::INFINITY,
            1 => f32::NEG_INFINITY,
            _ => f32::NAN,
        };
    }
    let mut polys: Vec<[u16; 3]> = (0..np)
        .map(|_| {
            let a = g.below(nv as u32) as u16;
            let b = g.below(nv as u32) as u16;
            let c = if g.below(15) == 0 { a } else { g.below(nv as u32) as u16 };
            [a, b, c]
        })
        .collect();
    if np > 2 && g.below(4) == 0 {
        // a duplicated run
        let from = g.below(np as u32) as usize;
        let len = 1 + g.below((np - from) as u32) as usize;
        let to = g.below(np as u32) as usize;
        for i in 0..len.min(np - to) {
            polys[to + i] = polys[from + i];
        }
    }
    if np > 0 && g.below(20) == 0 {
        let p = polys[0];
        polys.iter_mut().for_each(|q| *q = p);
    }
    let attrs = match g.below(4) {
        0 => 0,
        1 => 0x10 << g.below(5),
        2 => g.u32() & !0xFF0,
        _ => g.u32(),
    };
    let seed = if g.below(8) == 0 { 1 } else { g.u32() };
    Case { verts, polys, attrs, file_tree: g.below(4) == 0, seed, kind }
}

/// `ref_mesh_cast_seq`'s mesh words for `c` with `tree`, then the kind-1 casts `rays` (each twice: tree, brute force).
fn cast_input(c: &Case, tree: &CullTree, rays: &[([f32; 3], [f32; 3], f32)]) -> Vec<i32> {
    let np = c.polys.len();
    let mut v = vec![c.verts.len() as i32, np as i32, tree.nodes.len() as i32, tree.poly_indices.len() as i32, 0, 0, 0, 0, 0];
    for p in &c.verts {
        v.extend(p.iter().map(|&f| bits(f)));
    }
    for p in &c.polys {
        v.extend(p.iter().map(|&i| i32::from(i)));
    }
    v.extend((0..np).map(|i| (i % 7) as i32));
    push_tree(&mut v, tree);
    for &(s, e, f) in rays {
        for with_tree in [1, 0] {
            let mut w = vec![0i32; STEP];
            w[0] = 1;
            for a in 0..3 {
                w[1 + a] = bits(s[a]);
                w[4 + a] = bits(e[a]);
            }
            w[7] = bits(f);
            w[8] = 0;
            w[9] = 1;
            w[10] = with_tree;
            v.extend(w);
        }
    }
    v
}

fn rust_casts(c: &Case, tree: &CullTree, rays: &[([f32; 3], [f32; 3], f32)]) -> Vec<i32> {
    let geom = MeshGeometry {
        flags: 0,
        vertices: c.verts.clone(),
        polys: c.polys.clone(),
        surface_types: (0..c.polys.len()).map(|i| (i % 7) as u8).collect(),
        cull_tree: Some(tree.clone()),
    };
    let mut out = Vec::new();
    for &(s, e, f) in rays {
        let res = CastResult { fraction: f, start_bad: false, compute_contact_point: true, ..CastResult::default() };
        let mut rt = RayTest::new(LineSeg::new(Vector3::new(s[0], s[1], s[2]), Vector3::new(e[0], e[1], e[2])), res, coll_type::ALL, false, false);
        let hit = geom.cast_ray(&mut rt);
        let r = &rt.result;
        out.extend([1, i32::from(hit), i32::from(r.start_bad), bits(r.fraction)]);
        out.extend([r.normal.x, r.normal.y, r.normal.z, r.contact_point.x, r.contact_point.y, r.contact_point.z].map(bits));
        out.push(r.surface_type as i32);
    }
    out
}

fn gen_rays(g: &mut Gen, c: &Case, tree: &CullTree) -> Vec<([f32; 3], [f32; 3], f32)> {
    if c.polys.is_empty() || c.verts.iter().any(|v| v.iter().any(|x| !x.is_finite())) {
        return Vec::new();
    }
    let n = g.below(13) as usize;
    (0..n)
        .map(|_| {
            let p = c.polys[g.below(c.polys.len() as u32) as usize];
            let v = |i: usize| c.verts[p[i] as usize];
            let target = match g.below(5) {
                0 => {
                    let (a, b) = (g.uniform(0.0, 1.0), g.uniform(0.0, 1.0));
                    let (a, b) = if a + b > 1.0 { (1.0 - a, 1.0 - b) } else { (a, b) };
                    let (p0, p1, p2) = (v(0), v(1), v(2));
                    [0, 1, 2].map(|k| p0[k] + a * (p1[k] - p0[k]) + b * (p2[k] - p0[k]))
                }
                1 => {
                    let (p0, p1) = (v(0), v(1));
                    [0, 1, 2].map(|k| (p0[k] + p1[k]) * 0.5)
                }
                2 => v(g.below(3) as usize),
                3 => {
                    let nd = &tree.nodes[g.below(tree.nodes.len() as u32) as usize];
                    [0, 1, 2].map(|k| if g.below(2) == 0 { nd.min[k] } else { nd.max[k] })
                }
                _ => {
                    let q = c.verts[g.below(c.verts.len() as u32) as usize];
                    [0, 1, 2].map(|k| q[k] * g.uniform(0.5, 1.5))
                }
            };
            let dir = match g.below(5) {
                0 => [0.0, 0.0, 1.0],
                1 => [1.0, 0.0, 0.0],
                _ => [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)],
            };
            let span = c.verts.iter().fold(1.0e-3f32, |m, q| q.iter().fold(m, |m, &x| if x.abs() > m { x.abs() } else { m }));
            let d = span * g.uniform(0.5, 3.0);
            let s = [0, 1, 2].map(|k| target[k] - dir[k] * d);
            let e = [0, 1, 2].map(|k| target[k] + dir[k] * d);
            (s, e, if g.below(5) == 0 { g.uniform(0.2, 1.0) } else { 1.0 })
        })
        .collect()
}

#[test]
fn aabtree_builder_matches_upstream() {
    let cases: u64 = std::env::var("AABTREE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x190_aab);
    let mut build = Check::new("AABTreeBuilderClass and Generate_Culling_Tree");
    let mut casts = Check::new("casts through the built trees");
    let (mut generated, mut leaves_only, mut nodes_total, mut polys_total, mut max_nodes) = (0u64, 0u64, 0u64, 0u64, 0usize);
    let (mut export_same, mut cast_count, mut hits, mut tree_missed, mut brute_missed, mut fraction_differs, mut tie_only) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut by_kind_diff = [0u64; 7];
    for i in 0..cases {
        let c = gen_case(&mut g);
        let input = c.input();
        let (r, rtree) = rust_words(&c);
        let w = cpp_words(&input);
        generated += u64::from(w[0] != 0);
        if r != w && !planted {
            let k = r.iter().zip(&w).position(|(a, b)| a != b).unwrap_or(r.len().min(w.len()));
            println!("mesh {i} (kind {}, {} polys) first difference at word {k} of {}/{}", c.kind, c.polys.len(), r.len(), w.len());
        }
        build.eq(&r, &w, || format!("mesh {i}"));
        let Some(ctree) = cpp_tree(&w) else { continue };
        // the export's arrays (after the tree's and the rand word) equal the tree's
        let tl = 4 + 8 * ctree.nodes.len() + ctree.poly_indices.len();
        export_same += u64::from(w.get(tl + 1..) == w.get(2..tl));
        nodes_total += ctree.nodes.len() as u64;
        polys_total += ctree.poly_indices.len() as u64;
        max_nodes = max_nodes.max(ctree.nodes.len());
        leaves_only += u64::from(ctree.nodes.len() == 1);
        let rays = gen_rays(&mut g, &c, &ctree);
        if rays.is_empty() {
            continue;
        }
        let ci = cast_input(&c, &ctree, &rays);
        let mut out = vec![0i32; rays.len() * 2 * 11 + 16];
        let n = unsafe { cpp::ref_mesh_cast_seq(ci.as_ptr(), ci.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
        out.truncate(n);
        let cpp_tree_words: Vec<i32> = out.chunks(11).step_by(2).flatten().copied().collect();
        let rc = rust_casts(&c, &rtree, &rays);
        casts.eq(&rc, &cpp_tree_words, || format!("mesh {i}"));
        for pair in out.chunks(22) {
            let (a, b) = (&pair[..11], &pair[11..]);
            cast_count += 1;
            hits += u64::from(b[1] != 0);
            match (a[1] != 0, b[1] != 0) {
                (false, true) => tree_missed += 1,
                (true, false) => brute_missed += 1,
                _ if a[3] != b[3] => fraction_differs += 1,
                _ if a != b => tie_only += 1,
                _ => continue,
            }
            by_kind_diff[c.kind as usize] += 1;
        }
    }
    println!("{cases} meshes: {generated} generated by Load_W3D's test; trees {nodes_total} nodes, {polys_total} poly indices, the largest {max_nodes} nodes, {leaves_only} a lone leaf; the export's arrays the tree's on {export_same}");
    println!("casts {cast_count} ({hits} hit by brute force): through the generated tree versus brute force, the tree missed a hit {tree_missed}, brute force missed one {brute_missed}, both hit at different fractions {fraction_differs}, only the normal, contact point or surface type differ {tie_only}; by mesh kind {by_kind_diff:?}");
    if planted {
        let a = build.report();
        let b = casts.report();
        println!("PLANT caught: {}", a + b > 0);
    } else {
        build.finish();
        casts.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}
