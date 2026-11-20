// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of the draft `src/drafts/ww3d/shattersystem.rs` (`VertexClass`, `PolygonClass`,
//! `BSPClass`) against upstream `WW3D2/shattersystem.cpp`, the declarations and members cut VERBATIM (build.rs `drafts_ww3d6`,
//! differential/wrap_draft_w6_ss.cpp). Stand-ins: `MeshMatDescClass` (the two enumerators), `HTreeClass` (the three accessors the BSP builder calls);
//! `DX8Wrapper::Convert_Color` is the real transcription. `ShatterSystem::Init/Shutdown/Shatter_Mesh` (the draft's no-ops: upstream's pattern
//! list is empty in the shipped game) touch `WW3DAssetManager`/`DynamicMeshClass` and are not compared.
//! (The other draft `src/drafts/sweep_ww3d/shattersystem.rs`, `set_plane_from_transform`, is tested by `differential_draft_shattersystem.rs`.)
//!
//! # Method
//!
//! 20,000 cases per operation (fixed seed, `DRAFT_CASES` overrides): `VertexClass::Which_Side`, `Intersect_Plane` (so `Plane::Compute_Intersection`
//! and `Lerp`), `Lerp` with factors in and a little outside [0, 1]; `PolygonClass::Which_Side`, `Split`, `Compute_Plane`, `Is_Degenerate`,
//! `Salvage_Degenerate`; and `BSPClass` built from a scripted pivot tree (2 to 12 pivots, root bone 1 as `ShatterSystem::Init`) then
//! `Clip_Polygon` of 1 to 3 polygons into the 32 clip pools. Vertices sit on a half-integer grid (many exactly ON a plane, coincident vertices) or
//! at random; planes are axis-aligned or random. Compared: every field of every live vertex, the plane and material id, the counts, the return values.
//!
//! # Excluded inputs
//!
//! - Vertices whose `DCG`/`DIG`/texcoord slots at or beyond `PassCount` hold anything but the constructor value (upstream's assignment copies only
//!   the first `PassCount` passes and keeps the destination's; the draft copies all of them).
//! - Polygons of 24 vertices for `Split`/`Salvage_Degenerate` (upstream reads `Verts[24]`, outside the array).
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! Not 0. The draft clones whole polygons (`*front = self.clone()`) where upstream's `operator=` copies only `NumVerts` vertices, so a
//! stale slot read by `Salvage_Degenerate` (`Verts[i+1]` at `i == NumVerts-1`) may differ; and `Compute_Intersection` writes `*set_t` before it
//! range-checks, which the draft's `Option` may drop. Both are what I expect the run to show, otherwise 0.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::shattersystem::*;
use rts_lockstep_engine::ww::plane::PlaneClass;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_ss_vertex(input: *const u32, out: *mut u32) -> i32;
    fn ref_w6_ss_polygon(input: *const u32, out: *mut u32) -> i32;
    fn ref_w6_ss_bsp(input: *const u32, out: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn b(f: f32) -> u32 {
    f.to_bits()
}

fn coord(g: &mut Gen, grid: bool) -> f32 {
    if grid {
        (g.below(17) as f32 - 8.0) * 0.5
    } else {
        g.uniform(-5.0, 5.0)
    }
}

fn vertex(g: &mut Gen, pass_count: usize, grid: bool) -> VertexClass {
    let mut v = VertexClass::new();
    v.position = Vector3::new(coord(g, grid), coord(g, grid), coord(g, grid));
    v.normal = if g.below(4) == 0 { Vector3::new(0.0, 0.0, 1.0) } else { Vector3::new(g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)) };
    v.pass_count = pass_count;
    for ip in 0..pass_count {
        v.dcg[ip] = g.u32();
        v.dig[ip] = g.u32();
        for is in 0..2 {
            v.tex_coord[ip][is] = Vector2::new(g.uniform(-2.0, 2.0), g.uniform(-2.0, 2.0));
        }
    }
    v
}

fn vwords(v: &VertexClass) -> Vec<u32> {
    let mut w = vec![b(v.position.x), b(v.position.y), b(v.position.z), b(v.normal.x), b(v.normal.y), b(v.normal.z), v.pass_count as u32];
    w.extend(v.dcg);
    w.extend(v.dig);
    for ip in 0..4 {
        for is in 0..2 {
            w.extend([b(v.tex_coord[ip][is].x), b(v.tex_coord[ip][is].y)]);
        }
    }
    w
}

fn plane(g: &mut Gen) -> PlaneClass {
    let n = match g.below(5) {
        0 => Vector3::new(1.0, 0.0, 0.0),
        1 => Vector3::new(0.0, 1.0, 0.0),
        2 => Vector3::new(0.0, 0.0, 1.0),
        3 => Vector3::new(-1.0, 0.0, 0.0),
        _ => Vector3::new(g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)),
    };
    PlaneClass { n, d: if g.below(2) == 0 { (g.below(9) as f32 - 4.0) * 0.5 } else { g.uniform(-3.0, 3.0) } }
}

fn pwords(p: &PolygonClass, all: bool) -> Vec<u32> {
    let mut w = vec![p.material_id as u32, p.num_verts as u32, b(p.plane.n.x), b(p.plane.n.y), b(p.plane.n.z), b(p.plane.d)];
    let live = if all { 24 } else { p.num_verts };
    for v in &p.verts[..live] {
        w.extend(vwords(v));
    }
    w
}

fn polygon(g: &mut Gen, n: usize, pass_count: usize, grid: bool, garbage: bool) -> PolygonClass {
    let mut p = PolygonClass::new();
    p.material_id = g.below(5) as i32;
    p.num_verts = n;
    p.plane = plane(g);
    let base = vertex(g, pass_count, grid);
    for i in 0..n {
        p.verts[i] = match g.below(6) {
            // a repeated vertex: coincident neighbours for Is_Degenerate / Salvage_Degenerate
            0 if i > 0 => p.verts[i - 1],
            _ => {
                let mut v = vertex(g, pass_count, grid);
                if g.below(3) == 0 {
                    v.position.z = base.position.z; // flat-ish polygons
                }
                v
            }
        };
    }
    if garbage {
        for i in n..24 {
            p.verts[i] = vertex(g, pass_count, grid);
        }
    }
    p
}

fn input_polygon(p: &PolygonClass) -> Vec<u32> {
    pwords(p, true)
}

#[test]
fn shattersystem_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_03);
    let names = [
        "VertexClass::Which_Side", "VertexClass::Intersect_Plane", "VertexClass::Lerp", "PolygonClass::Which_Side", "PolygonClass::Split", "PolygonClass::Compute_Plane",
        "PolygonClass::Is_Degenerate", "PolygonClass::Salvage_Degenerate", "BSPClass (build + Clip_Polygon)",
    ];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    let mut out = vec![0u32; 200_000];
    for case in 0..support_sweep::cases() {
        let pc = g.below(5) as usize;
        let grid = g.below(2) == 0;
        // --- vertex operations
        {
            let pl = plane(&mut g);
            let (v0, v1) = (vertex(&mut g, pc, grid), vertex(&mut g, pc, grid));
            let alpha = match g.below(6) {
                0 => 0.0,
                1 => 1.0,
                2 => -0.00005,
                3 => 1.00005,
                _ => g.uniform(0.0, 1.0),
            };
            for op in 0..3u32 {
                let mut inp = vec![op, b(pl.n.x), b(pl.n.y), b(pl.n.z), b(pl.d)];
                inp.extend(vwords(&v0));
                inp.extend(vwords(&v1));
                inp.push(b(alpha));
                let n = unsafe { ref_w6_ss_vertex(inp.as_ptr(), out.as_mut_ptr()) } as usize;
                let cpp = out[..n].to_vec();
                let rust = match op {
                    0 => vec![v0.which_side(&pl) as u32],
                    1 => vwords(&VertexClass::intersect_plane(&v0, &v1, &pl)),
                    _ => vwords(&VertexClass::lerp(&v0, &v1, alpha)),
                };
                c[op as usize].eq(rust, cpp, || format!("case {case} op {op} plane {pl:?} v0 {v0:?} v1 {v1:?} alpha {alpha}"));
            }
        }
        // --- polygon operations
        {
            let n = 3 + g.below(9) as usize;
            let garbage = g.below(3) == 0;
            let poly = polygon(&mut g, n, pc, grid, garbage);
            let pl = plane(&mut g);
            for op in 0..5u32 {
                let mut inp = vec![op, b(pl.n.x), b(pl.n.y), b(pl.n.z), b(pl.d)];
                inp.extend(input_polygon(&poly));
                let k = unsafe { ref_w6_ss_polygon(inp.as_ptr(), out.as_mut_ptr()) } as usize;
                let cpp_all = out[..k].to_vec();
                // the C++ output holds every slot of each polygon; keep the live part of each
                let live = |w: &[u32], from: usize| -> (Vec<u32>, usize) {
                    let nv = w[from + 1] as usize;
                    let mut r = w[from..from + 6].to_vec();
                    r.extend_from_slice(&w[from + 6..from + 6 + 31 * nv]);
                    (r, from + 6 + 31 * 24)
                };
                let (rust, cpp) = match op {
                    0 => (vec![poly.which_side(&pl) as u32], cpp_all.clone()),
                    1 => {
                        let (mut f, mut bk) = (PolygonClass::new(), PolygonClass::new());
                        poly.split(&pl, &mut f, &mut bk);
                        let (cf, next) = live(&cpp_all, 0);
                        let (cb, _) = live(&cpp_all, next);
                        let mut r = pwords(&f, false);
                        r.extend(pwords(&bk, false));
                        let mut cc = cf;
                        cc.extend(cb);
                        (r, cc)
                    }
                    2 => {
                        let mut p = poly.clone();
                        p.compute_plane();
                        (pwords(&p, false), live(&cpp_all, 0).0)
                    }
                    3 | _ => {
                        let mut p = poly.clone();
                        let ret = if op == 3 { p.is_degenerate() } else { p.salvage_degenerate() };
                        let mut r = vec![ret as u32];
                        r.extend(pwords(&p, false));
                        let mut cc = vec![cpp_all[0]];
                        cc.extend(live(&cpp_all, 1).0);
                        (r, cc)
                    }
                };
                c[3 + op as usize].eq(rust, cpp, || format!("case {case} op {op} plane {pl:?} polygon {poly:?}"));
            }
        }
        // --- BSP
        {
            let np = 2 + g.below(11) as usize;
            let mut inp = vec![np as u32];
            let mut parents = vec![-1i32; np];
            let mut zs = Vec::new();
            for i in 0..np {
                parents[i] = if i == 0 { -1 } else if i == 1 { 0 } else { g.below(i as u32) as i32 };
                let mut m = [0.0f32; 12];
                for k in 0..12 {
                    m[k] = if k % 4 == 3 { (g.below(9) as f32 - 4.0) * 0.5 } else if g.below(2) == 0 { [0.0, 1.0, -1.0][g.below(3) as usize] } else { g.uniform(-1.0, 1.0) };
                }
                zs.push((Vector3::new(m[2], m[6], m[10]), Vector3::new(m[3], m[7], m[11])));
                inp.push(parents[i] as u32);
                inp.extend(m.iter().map(|x| b(*x)));
            }
            inp.push(1);
            let npolys = 1 + g.below(3) as usize;
            inp.push(npolys as u32);
            let mut polys = Vec::new();
            for _ in 0..npolys {
                let n = 3 + g.below(8) as usize;
                let p = polygon(&mut g, n, pc, grid, false);
                inp.extend(input_polygon(&p));
                polys.push(p);
            }
            let k = unsafe { ref_w6_ss_bsp(inp.as_ptr(), out.as_mut_ptr()) } as usize;
            let pf = |i: usize| parents[i];
            let tf = |i: usize| zs[i];
            let tree = ShatterHTree { num_pivots: np, parent_index: &pf, transform: &tf };
            let mut leaf = 0i32;
            let bsp = BSPClass::new(&tree, 1, &mut leaf);
            let mut pools: Vec<Vec<PolygonClass>> = vec![Vec::new(); 32];
            for p in &polys {
                bsp.clip_polygon(p, &mut pools);
            }
            let mut rust = vec![leaf as u32];
            for pool in &pools {
                rust.push(pool.len() as u32);
                for p in pool {
                    rust.extend(pwords(p, false));
                }
            }
            // the C++ output has all 24 slots per polygon: keep the live ones
            let mut cpp = vec![out[0]];
            let mut pos = 1;
            for _ in 0..32 {
                let cnt = out[pos] as usize;
                cpp.push(cnt as u32);
                pos += 1;
                for _ in 0..cnt {
                    let nv = out[pos + 1] as usize;
                    cpp.extend_from_slice(&out[pos..pos + 6 + 31 * nv]);
                    pos += 6 + 31 * 24;
                }
            }
            assert_eq!(pos, k, "bsp output length");
            c[8].eq(rust, cpp, || format!("case {case} pivots {np} parents {parents:?} polys {polys:?}"));
        }
    }
    for k in &c {
        for e in k.examples.iter().take(2) {
            println!("EXAMPLE {}: {}", k.name, e.chars().take(3000).collect::<String>());
        }
    }
    support_sweep::finish(c);
}
