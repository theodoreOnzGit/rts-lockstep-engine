// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215) of the `AABTreeClass` drafts (`src/drafts/sweep_ww3d/aabtree.rs`): both
//! `Generate_OBBox_APT_Recursive`, `Cast_Semi_Infinite_Axis_Aligned_Ray_Recursive` (and `_To_Polys`), `Cast_AABox_Recursive`
//! (and `_To_Polys`), `Cast_OBBox_Recursive` (and `_To_Polys`), `Intersect_OBBox_Recursive` (and `_With_Polys`) against
//! upstream's aabtree.cpp text, cut verbatim by build.rs (`drafts_anim`, differential/wrap_draft_aabtree.cpp).
//!
//! # Method
//!
//! The collision maths is not under test (it is the colmath group's), so both sides call an ORACLE for every collision
//! call: a SplitMix64 hash of (case seed, call number) decides each answer, and each call appends its arguments' bits to a
//! log (the node box, the triangle's three vertices and the triangle's normal `tri.N`, the ray's start and axes, `flags` in
//! and out, the polygon whose surface type is asked). The C++ stand-ins (wrapper) and this file's host
//! ([`Host`]) draw in the same order. Compared as one word stream: every call's arguments, the call order (so every skipped
//! or extra call), then the results: the active-polygon table, the ray's count and flags, the cast's return value,
//! `StartBad` and `SurfaceType`.
//!
//! Cases (default 20,000, fixed seed): a random tree of 1 to 15 nodes (leaves with 0 to 4 polygons, repeated polygon
//! indices), a mesh of 3 to 12 vertices (random and integer-grid, so triangles can be degenerate) and 1 to 8 polygons with
//! random plane equations, node boxes random or on an integer grid; each of the six operations once per case with its own
//! seed.
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! A MISS expected in every operation that passes a triangle to the host: the draft builds `tri.N` from the stored plane
//! (`tri_view`), while upstream (`OPTIMIZE_PLANEEQ_RAM` is 1 in meshgeometry.h) computes `tri.N` with `Compute_Normal()`
//! (the normalised cross product), and `Generate_OBBox_APT_Recursive`'s view-direction test reads it. The pure ray cast
//! (`op 3`) passes only the vertices and the plane, so it should match.

mod draft_anim_bytes;
mod support;

use draft_anim_bytes::*;
use rts_lockstep_engine::drafts::sweep_ww3d::aabtree::{AabTreeTestHost, AabTreeView, AABTREE_LEAF_FLAG};
use rts_lockstep_engine::drafts::sweep_ww3d::meshgeometry::{MeshGeometry, TriView};
use rts_lockstep_engine::formats::w3d::mesh::AabNode;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dab_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 400_000];
    let n = unsafe { ref_dab_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "wrapper refused");
    out.truncate(n as usize);
    out
}

fn bits(f: f32) -> u32 {
    f.to_bits()
}

/// The oracle and log of the wrapper's `CollisionMath` stand-ins, in Rust.
struct Host {
    seed: u64,
    calls: std::cell::Cell<u64>,
    log: std::cell::RefCell<Vec<u32>>,
    view: [f32; 3],
    apt: Vec<u32>,
    start_bad: bool,
    surface: u32,
}

impl Host {
    fn new(seed: u64, view: [f32; 3]) -> Self {
        Host { seed, calls: Default::default(), log: Default::default(), view, apt: Vec::new(), start_bad: false, surface: 0xDEAD_BEEF }
    }
    fn mix(&self) -> u64 {
        self.calls.set(self.calls.get() + 1);
        let mut z = self.seed.wrapping_add(self.calls.get().wrapping_mul(0x9E37_79B9_7F4A_7C15));
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn chance(&self, pct: u64) -> bool {
        (self.mix() >> 20) % 100 < pct
    }
    fn v(&self, a: [f32; 3]) {
        self.log.borrow_mut().extend(a.iter().map(|x| bits(*x)));
    }
    fn tri(&self, t: &TriView) {
        for p in t.v {
            self.v(p);
        }
        self.v(t.n);
    }
}

impl AabTreeTestHost for Host {
    fn box_intersects_node(&mut self, min: [f32; 3], max: [f32; 3]) -> bool {
        let r = self.chance(70);
        self.log.borrow_mut().push(1);
        self.v(min);
        self.v(max);
        self.log.borrow_mut().push(u32::from(r));
        r
    }
    fn box_intersects_tri(&mut self, tri: &TriView) -> bool {
        let r = self.chance(50);
        self.log.borrow_mut().push(2);
        self.tri(tri);
        self.log.borrow_mut().push(u32::from(r));
        r
    }
    fn apt_add(&mut self, poly_index: u32) {
        self.apt.push(poly_index);
    }
    fn view_vector(&self) -> [f32; 3] {
        self.view
    }
    fn cull(&mut self, min: [f32; 3], max: [f32; 3]) -> bool {
        let r = self.chance(25);
        self.log.borrow_mut().push(4);
        self.v(min);
        self.v(max);
        self.log.borrow_mut().push(u32::from(r));
        r
    }
    fn collide_aabox_tri(&mut self, tri: &TriView) -> bool {
        let r = self.chance(25);
        self.start_bad = self.chance(6);
        self.log.borrow_mut().push(5);
        self.tri(tri);
        self.log.borrow_mut().extend([u32::from(r), u32::from(self.start_bad)]);
        r
    }
    fn collide_obbox_tri(&mut self, tri: &TriView) -> bool {
        let r = self.chance(25);
        self.start_bad = self.chance(6);
        self.log.borrow_mut().push(6);
        self.tri(tri);
        self.v([0.0; 3]);
        self.log.borrow_mut().extend([u32::from(r), u32::from(self.start_bad)]);
        r
    }
    fn intersect_obbox_tri(&mut self, tri: &TriView) -> bool {
        let r = self.chance(50);
        self.log.borrow_mut().push(2);
        self.tri(tri);
        self.log.borrow_mut().push(u32::from(r));
        r
    }
    fn result_start_bad(&self) -> bool {
        self.start_bad
    }
    fn set_result_surface_type(&mut self, surface_type: u32) {
        self.surface = surface_type;
    }
    fn poly_surface_type(&self, poly: usize) -> u32 {
        let v = ((self.mix() >> 24) & 0xFF) as u32;
        self.log.borrow_mut().extend([9, poly as u32, v]);
        v
    }
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
    ) -> bool {
        self.log.borrow_mut().push(8);
        for p in tri.v {
            self.v(p);
        }
        self.log.borrow_mut().extend(tri_plane.iter().map(|x| bits(*x)));
        self.v(start_point);
        self.log.borrow_mut().extend([axis_r as u32, axis_1 as u32, axis_2 as u32, direction as u32, u32::from(*flags)]);
        let h = self.mix();
        let r = (h >> 20) % 100 < 50;
        *flags |= ((h >> 40) & 7) as u8;
        self.log.borrow_mut().extend([u32::from(r), u32::from(*flags)]);
        r
    }
}

fn grid_or_random(g: &mut Gen, lo: f32, hi: f32) -> f32 {
    if g.below(3) == 0 { g.below(7) as f32 - 3.0 } else { g.uniform(lo, hi) }
}

fn build(g: &mut Gen, nodes: &mut Vec<AabNode>, pis: &mut Vec<u32>, npolys: u32, depth: u32) -> u32 {
    let me = nodes.len();
    let mut min = [0.0f32; 3];
    let mut max = [0.0f32; 3];
    for k in 0..3 {
        min[k] = grid_or_random(g, -5.0, 3.0);
        max[k] = min[k] + if g.below(3) == 0 { g.below(5) as f32 } else { g.uniform(0.0, 6.0) };
    }
    nodes.push(AabNode { min, max, front_or_poly0: 0, back_or_poly_count: 0 });
    if depth >= 3 || g.below(3) == 0 {
        let count = g.below(5);
        let poly0 = pis.len() as u32;
        for _ in 0..count {
            pis.push(g.below(npolys));
        }
        nodes[me].front_or_poly0 = poly0 | AABTREE_LEAF_FLAG;
        nodes[me].back_or_poly_count = count;
    } else {
        let f = build(g, nodes, pis, npolys, depth + 1);
        let b = build(g, nodes, pis, npolys, depth + 1);
        nodes[me].front_or_poly0 = f;
        nodes[me].back_or_poly_count = b;
    }
    me as u32
}

#[test]
fn aabtree_casts_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0040);
    let names = ["Generate_APT(box)", "Generate_APT(box, viewdir)", "Cast_Semi_Infinite_Axis_Aligned_Ray", "Cast_AABox", "Cast_OBBox", "Intersect_OBBox"];
    let mut checks: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    let mut calls_total = 0u64;
    for case in 0..cases() {
        let npolys = 1 + g.below(8);
        let nv = 3 + g.below(10);
        let mut nodes = Vec::new();
        let mut pis = Vec::new();
        build(&mut g, &mut nodes, &mut pis, npolys, 0);
        let vertices: Vec<[f32; 3]> = (0..nv).map(|_| [grid_or_random(&mut g, -5.0, 5.0), grid_or_random(&mut g, -5.0, 5.0), grid_or_random(&mut g, -5.0, 5.0)]).collect();
        let polygons: Vec<[u32; 3]> = (0..npolys).map(|_| if g.below(12) == 0 { let a = g.below(nv); [a, a, a] } else { [g.below(nv), g.below(nv), g.below(nv)] }).collect();
        let planes: Vec<[f32; 4]> = (0..npolys).map(|_| [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-3.0, 3.0)]).collect();
        let mesh = MeshGeometry { vertices: vertices.clone(), planes: planes.clone(), polygons: polygons.clone(), has_cull_tree: true, user_text: None };
        let tree = AabTreeView { nodes: nodes.clone(), poly_indices: pis.clone() };
        let mut base = Vec::new();
        for n in &nodes {
            base.extend(n.min.iter().chain(n.max.iter()).map(|x| bits(*x)));
            base.extend([n.front_or_poly0, n.back_or_poly_count]);
        }
        let push_all = |base: &mut Vec<u32>| {
            base.push(pis.len() as u32);
            base.extend(&pis);
            base.push(vertices.len() as u32);
            base.extend(vertices.iter().flatten().map(|x| bits(*x)));
            base.push(polygons.len() as u32);
            base.extend(polygons.iter().flatten());
            base.push(planes.len() as u32);
            base.extend(planes.iter().flatten().map(|x| bits(*x)));
        };
        let mut head = Vec::new();
        head.push(nodes.len() as u32);
        head.extend(&base);
        push_all(&mut head);
        for op in 1..=6u32 {
            let seed = g.u64();
            let view = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
            let mut host = Host::new(seed, view);
            let mut inp = vec![op, (seed >> 32) as u32, seed as u32];
            inp.extend(&head);
            let mut res: Vec<u32> = Vec::new();
            match op {
                1 => {
                    tree.generate_obbox_apt_recursive(&mesh, 0, &mut host);
                    res.push(host.apt.len() as u32);
                    res.extend(&host.apt);
                }
                2 => {
                    inp.extend(view.iter().map(|x| bits(*x)));
                    tree.generate_obbox_apt_recursive_dir(&mesh, 0, &mut host);
                    res.push(host.apt.len() as u32);
                    res.extend(&host.apt);
                }
                3 => {
                    let start = [grid_or_random(&mut g, -5.0, 8.0), grid_or_random(&mut g, -5.0, 8.0), grid_or_random(&mut g, -5.0, 8.0)];
                    let axis_dir = g.below(6);
                    let flags = g.below(256) as u8;
                    inp.extend(start.iter().map(|x| bits(*x)));
                    inp.extend([axis_dir, u32::from(flags)]);
                    const AXIS_R: [usize; 6] = [0, 0, 1, 1, 2, 2];
                    const AXIS_1: [usize; 6] = [1, 1, 2, 2, 0, 0];
                    const AXIS_2: [usize; 6] = [2, 2, 0, 0, 1, 1];
                    const DIR: [usize; 6] = [1, 0, 1, 0, 1, 0];
                    let a = axis_dir as usize;
                    let mut f = flags;
                    let c = tree.cast_semi_infinite_axis_aligned_ray_recursive(&mesh, &mut host, 0, start, AXIS_R[a], AXIS_1[a], AXIS_2[a], DIR[a], &mut f);
                    res.extend([c as u32, u32::from(f)]);
                }
                4 | 5 => {
                    let b = if op == 4 { tree.cast_aabox_recursive(&mesh, &mut host, 0) } else { tree.cast_obbox_recursive(&mesh, &mut host, 0) };
                    res.extend([u32::from(b), u32::from(host.start_bad), host.surface]);
                }
                _ => {
                    let b = tree.intersect_obbox_recursive(&mesh, &mut host, 0);
                    res.push(u32::from(b));
                }
            }
            let mut want = host.log.borrow().clone();
            want.push(0xFFFF_FFFE);
            want.extend(res);
            calls_total += host.calls.get();
            let c = run(&inp);
            checks[op as usize - 1].eq(want, c, || format!("case {case} op {op}"));
        }
    }
    println!("{} cases, {calls_total} oracle draws", cases());
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    if planted {
        let m: u64 = checks.into_iter().map(|c| c.report()).sum();
        panic!("a planted run: {m} mismatches (see the MEASURED lines)");
    }
    for c in checks {
        c.finish();
    }
}
