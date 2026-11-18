// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dgeo) of the collision members of `MeshGeometryClass`
//! (`src/drafts/sweep_ww3d/meshgeometry.rs`): the three `Generate_Rigid_APT` overloads, `Generate_Skin_APT`,
//! `cast_semi_infinite_axis_aligned_ray`, `cast_aabox_identity` / `_z90` / `_z180` / `_z270`,
//! `intersect_obbox_brute_force`, `cast_ray_brute_force`, `cast_aabox_brute_force` and `cast_obbox_brute_force`, against
//! upstream's text cut VERBATIM from `WW3D2/meshgeometry.cpp` (build.rs `drafts_ww3dgeo`,
//! differential/wrap_draft_meshgeom.cpp). The triangle tests, the cull tree and the box transforms are scripted stand-ins on
//! both sides (CollisionMath's own maths is verified by the colmath tests; here the question is whether the draft calls them
//! with the right triangle, in the right order, and handles their results as upstream does).
//!
//! # Method
//!
//! 20,000 meshes (fixed seed, `DRAFT_CASES` overrides): 1 to 8 vertices (finite, with specials: 0, -0, inf, NaN), 0 to 8
//! polygons with random indices and random plane equations (10 % have an exactly zero normal, so `N . dir == 0` is reached),
//! with and without a cull tree; each of the 13 members is run on every mesh with random operands (view directions, start
//! points, translations, prefilled `CastResult` including `StartBad`, a prefilled `apt` and `flags`) and a random script
//! seed. The n-th stand-in call answers from a hash of (seed, n) (hit, StartBad, a normal, flag bits) identically on both
//! sides. Compared as words: the return value, the `apt`, the flags, the cast result, and the ordered log of every stand-in
//! call with the triangle (12 floats), plane, start point, axes it was given.
//!
//! In the draft the box transform (`newbox.Translate`, `Rotate`) and the choice between the cull tree and the brute force
//! belong to the host (`cast_aabox_on_copy`); the host here logs the translation and rotation exactly as the C++ stand-in
//! does and then runs the cull tree stand-in or the DRAFT's own `cast_aabox_brute_force`, as upstream calls its own.
//!
//! Excluded (upstream undefined or not drafted): `axis_dir` outside 0..6 (upstream indexes static tables unchecked);
//! `Get_User_Text` / `Set_User_Text` (a ref-counted share buffer, whose NULL argument upstream leaves dangling: the draft
//! documents the deviation); `TriIndex` is `Vector3i16` upstream and `u32` in the draft (indices here are below 8).
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! 0 mismatches. The translation is a line-by-line copy, so the likeliest miss is a table (`direction`, the rotation
//! codes) or the normal rotation signs.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::sweep_ww3d::meshgeometry::*;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dmg_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Host {
    seed: u32,
    call: u32,
    log: Vec<u32>,
    mesh: MeshGeometry,
}

impl Host {
    fn next_hash(&mut self) -> u32 {
        let mut h = (self.seed ^ self.call.wrapping_mul(0x9E3779B1)).wrapping_mul(2654435761);
        h ^= h >> 15;
        h = h.wrapping_mul(2246822519);
        h ^= h >> 13;
        self.call += 1;
        h
    }
    fn log_tri(&mut self, code: u32, t: &TriView) {
        self.log.push(code);
        for v in &t.v {
            self.log.extend(v.iter().map(|x| x.to_bits()));
        }
        self.log.extend(t.n.iter().map(|x| x.to_bits()));
    }
    fn script_result(h: u32, r: &mut CastResult) {
        if (h >> 1) % 5 == 0 {
            r.start_bad = true;
        }
        r.normal[0] = (((h >> 8) & 255) as i32 - 128) as f32;
        r.normal[1] = (((h >> 16) & 255) as i32 - 128) as f32;
        r.normal[2] = (((h >> 24) & 255) as i32 - 128) as f32;
    }
}

impl MeshCollisionHost for Host {
    fn cull_tree_generate_apt_box(&mut self, apt: &mut Vec<u32>) {
        self.log.push(310);
        let h = self.next_hash();
        for i in 0..h % 4 {
            apt.push((h >> 4) % 100 + i);
        }
    }
    fn cull_tree_generate_apt_box_dir(&mut self, viewdir: [f32; 3], apt: &mut Vec<u32>) {
        self.log.push(311);
        self.log.extend(viewdir.iter().map(|x| x.to_bits()));
        let h = self.next_hash();
        for i in 0..h % 4 {
            apt.push((h >> 4) % 100 + i);
        }
    }
    fn cull_tree_cast_semi_infinite_axis_aligned_ray(&mut self, start_point: [f32; 3], axis_dir: i32, flags: &mut u8) -> i32 {
        self.log.push(312);
        self.log.extend(start_point.iter().map(|x| x.to_bits()));
        self.log.push(axis_dir as u32);
        let h = self.next_hash();
        *flags = ((h >> 8) & 7) as u8;
        (h % 6) as i32
    }
    fn cull_tree_cast_aabox(&mut self, result: &mut CastResult) -> bool {
        self.log.push(313);
        let h = self.next_hash();
        let hit = h & 1 != 0;
        if hit {
            Host::script_result(h, result);
        }
        hit
    }
    fn obbox_intersects_tri(&mut self, tri: &TriView) -> bool {
        self.log_tri(300, tri);
        self.next_hash() & 1 != 0
    }
    fn collide_ray_tri(&mut self, tri: &TriView, result: &mut CastResult) -> bool {
        self.log_tri(301, tri);
        let h = self.next_hash();
        if h & 1 != 0 {
            Host::script_result(h, result);
        }
        h & 1 != 0
    }
    fn collide_aabox_tri(&mut self, tri: &TriView, result: &mut CastResult) -> bool {
        self.log_tri(302, tri);
        let h = self.next_hash();
        if h & 1 != 0 {
            Host::script_result(h, result);
        }
        h & 1 != 0
    }
    fn collide_obbox_tri(&mut self, tri: &TriView, result: &mut CastResult) -> bool {
        self.log_tri(303, tri);
        let h = self.next_hash();
        if h & 1 != 0 {
            Host::script_result(h, result);
        }
        h & 1 != 0
    }
    fn cast_semi_infinite_axis_aligned_ray_to_triangle(&mut self, tri: &TriView, plane: [f32; 4], start_point: [f32; 3], axis_r: usize, axis_1: usize, axis_2: usize, direction: i32, flags: &mut u8) -> bool {
        self.log.push(304);
        for v in &tri.v {
            self.log.extend(v.iter().map(|x| x.to_bits()));
        }
        self.log.extend(plane.iter().map(|x| x.to_bits()));
        self.log.extend(start_point.iter().map(|x| x.to_bits()));
        self.log.extend([axis_r as u32, axis_1 as u32, axis_2 as u32, direction as u32]);
        let h = self.next_hash();
        *flags |= ((h >> 8) & 3) as u8;
        h & 1 != 0
    }
    fn poly_surface_type(&self, poly: usize) -> u32 {
        ((poly * 7 + 3) % 32) as u32
    }
    fn cast_aabox_on_copy(&mut self, translation: [f32; 3], rotation: Option<BoxRotation>, use_cull_tree: bool, result: &mut CastResult) -> bool {
        self.log.push(320);
        self.log.extend(translation.iter().map(|x| x.to_bits()));
        if let Some(r) = rotation {
            self.log.push(321);
            self.log.push(match r {
                BoxRotation::Z90 => 1,
                BoxRotation::Z180 => 2,
                BoxRotation::Z270 => 3,
            });
        }
        if use_cull_tree {
            self.cull_tree_cast_aabox(result)
        } else {
            let m = self.mesh.clone();
            m.cast_aabox_brute_force(self, result)
        }
    }
}

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp 0", c.name, c.compared, c.mismatches, c.nan_only);
        bad += c.mismatches;
        if !planted {
            c.finish();
        }
    }
    if planted {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

fn coord(g: &mut Gen) -> f32 {
    match g.below(12) {
        0 => g.special(),
        1 => 0.0,
        _ => g.uniform(-10.0, 10.0),
    }
}

fn v3(g: &mut Gen) -> [f32; 3] {
    [coord(g), coord(g), coord(g)]
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn res_out(hit: bool, r: &CastResult) -> Vec<u32> {
    vec![u32::from(hit), u32::from(r.start_bad), r.normal[0].to_bits(), r.normal[1].to_bits(), r.normal[2].to_bits(), r.surface_type]
}

#[test]
fn mesh_collision_members_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0707);
    let names = [
        "Generate_Rigid_APT(view_dir)", "Generate_Rigid_APT(box)", "Generate_Rigid_APT(box, viewdir)", "Generate_Skin_APT",
        "cast_semi_infinite_axis_aligned_ray", "cast_aabox_identity", "cast_aabox_z90", "cast_aabox_z180", "cast_aabox_z270",
        "intersect_obbox_brute_force", "cast_ray_brute_force", "cast_aabox_brute_force", "cast_obbox_brute_force",
    ];
    let mut chk: Vec<Check> = names.iter().map(|n| Check::new(&format!("MeshGeometryClass::{n}"))).collect();
    let mut hits = [0u64; 13];
    for case in 0..cases() {
        let nv = 1 + g.below(8) as usize;
        let np = g.below(9) as usize;
        let mesh = MeshGeometry {
            vertices: (0..nv).map(|_| v3(&mut g)).collect(),
            planes: (0..np)
                .map(|_| {
                    if g.below(10) == 0 {
                        [0.0, 0.0, 0.0, coord(&mut g)]
                    } else {
                        [coord(&mut g), coord(&mut g), coord(&mut g), coord(&mut g)]
                    }
                })
                .collect(),
            polygons: (0..np).map(|_| [g.below(nv as u32), g.below(nv as u32), g.below(nv as u32)]).collect(),
            has_cull_tree: g.below(2) == 0,
            user_text: None,
        };
        let seed = g.u32();
        let mut header = vec![0, seed, u32::from(mesh.has_cull_tree), nv as u32];
        for v in &mesh.vertices {
            header.extend(bits(v));
        }
        header.push(np as u32);
        for i in 0..np {
            header.extend(mesh.polygons[i]);
            header.extend(bits(&mesh.planes[i]));
        }
        for op in 0..13usize {
            let mut host = Host { seed, call: 0, log: Vec::new(), mesh: mesh.clone() };
            let mut inp = header.clone();
            inp[0] = op as u32;
            let mut rust: Vec<u32> = Vec::new();
            let prefill_apt = |g: &mut Gen| -> Vec<u32> { (0..g.below(3)).map(|_| g.below(1000)).collect() };
            let prefill_res = |g: &mut Gen| CastResult { normal: [coord(g), coord(g), coord(g)], start_bad: g.below(4) == 0, surface_type: g.below(40) };
            let put_res = |inp: &mut Vec<u32>, r: &CastResult| {
                inp.push(u32::from(r.start_bad));
                inp.extend(bits(&r.normal));
                inp.push(r.surface_type);
            };
            match op {
                0 | 1 | 2 => {
                    let vd = v3(&mut g);
                    let mut apt = prefill_apt(&mut g);
                    if op != 1 {
                        inp.extend(bits(&vd));
                    }
                    inp.push(apt.len() as u32);
                    inp.extend(apt.iter().copied());
                    match op {
                        0 => mesh.generate_rigid_apt_dir(vd, &mut apt),
                        1 => mesh.generate_rigid_apt_box(&mut host, &mut apt),
                        _ => mesh.generate_rigid_apt_box_dir(&mut host, vd, &mut apt),
                    }
                    rust.push(apt.len() as u32);
                    rust.extend(apt.iter().copied());
                    hits[op] += apt.len() as u64;
                }
                3 => {
                    let world: Vec<[f32; 3]> = (0..nv).map(|_| v3(&mut g)).collect();
                    let mut apt = prefill_apt(&mut g);
                    for w in &world {
                        inp.extend(bits(w));
                    }
                    inp.push(apt.len() as u32);
                    inp.extend(apt.iter().copied());
                    mesh.generate_skin_apt(&mut host, &mut apt, &world);
                    rust.push(apt.len() as u32);
                    rust.extend(apt.iter().copied());
                    hits[op] += apt.len() as u64;
                }
                4 => {
                    let sp = v3(&mut g);
                    let axis = g.below(6) as i32;
                    let mut flags = g.below(256) as u8;
                    inp.extend(bits(&sp));
                    inp.extend([axis as u32, u32::from(flags)]);
                    let c = mesh.cast_semi_infinite_axis_aligned_ray(&mut host, sp, axis, &mut flags);
                    rust.extend([c as u32, u32::from(flags)]);
                    hits[op] += c as u64;
                }
                5..=8 => {
                    let tr = v3(&mut g);
                    let mut r = prefill_res(&mut g);
                    inp.extend(bits(&tr));
                    put_res(&mut inp, &r);
                    let hit = match op {
                        5 => mesh.cast_aabox_identity(&mut host, tr, &mut r),
                        6 => mesh.cast_aabox_z90(&mut host, tr, &mut r),
                        7 => mesh.cast_aabox_z180(&mut host, tr, &mut r),
                        _ => mesh.cast_aabox_z270(&mut host, tr, &mut r),
                    };
                    rust = res_out(hit, &r);
                    hits[op] += u64::from(hit);
                }
                9 => {
                    let hit = mesh.intersect_obbox_brute_force(&mut host);
                    rust.push(u32::from(hit));
                    hits[op] += u64::from(hit);
                }
                _ => {
                    let mut r = prefill_res(&mut g);
                    put_res(&mut inp, &r);
                    let hit = match op {
                        10 => mesh.cast_ray_brute_force(&mut host, &mut r),
                        11 => mesh.cast_aabox_brute_force(&mut host, &mut r),
                        _ => mesh.cast_obbox_brute_force(&mut host, &mut r),
                    };
                    rust = res_out(hit, &r);
                    hits[op] += u64::from(hit);
                }
            }
            rust.extend(host.log.iter().copied());
            let mut out = vec![0u32; 20_000];
            let n = unsafe { ref_dmg_run(inp.as_ptr(), inp.len() as i32, out.as_mut_ptr(), out.len() as i32) };
            assert!(n >= 0, "op {op}");
            out.truncate(n as usize);
            chk[op].eq(rust, out, || format!("case {case} op {op}: {} verts {} polys cull {}", nv, np, mesh.has_cull_tree));
        }
    }
    println!("positive results per op (hits, or apt entries, or ray count): {hits:?}");
    finish(chk);
}
