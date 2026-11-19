// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dpure) of the draft `src/drafts/ww3d/meshbuild.rs` (`MeshBuilderClass`,
//! `VertClass`, `FaceClass`, `MeshStatsStruct`) against upstream `WW3D2/meshbuild.cpp`, compiled WHOLE
//! (differential/wrap_draft_meshbuild.cpp; one mechanical for-scope edit, build.rs `drafts_ww3dpure`). The real headers
//! (meshbuild.h, uarray.h, hashcalc.h) and the real libc `qsort` are used; the only stand-in is a `WorldInfoClass` subclass
//! (abstract upstream) whose `Get_Shared_Vertex_Normal` is a fixed arithmetic function, written identically on both sides.
//!
//! # Method
//!
//! 20,000 meshes per operation from a fixed seed (`DRAFT_CASES` overrides). A mesh is 1..=30 faces over a pool of 3..=14
//! points (random triples: shared edges, repeated points = degenerate faces, exact duplicates) or a grid of quads, with the
//! point templates carrying position, normal, id, bone, vertex material per pass, colours, alpha and UVs, and each face
//! vertex a copy that is sometimes altered (UV, vertex material, colour, id, smoothing group) so that welding, smoothing-group
//! sharing and UV splits all occur. Positions are sometimes nudged by less than, exactly, or just more than the `0.0001`
//! weld epsilon, and a share of the meshes are flat (zero extent on an axis, which makes upstream's hash insert divide by
//! zero). 1..=4 passes, every polygon-ordering channel (pass 0..3, stage 0..1), `compute_normals` on and off, a world info
//! that is absent, smoothing, or not smoothing, and face-array guesses and growths small enough to force `Grow_Face_Array`.
//! Operations (6): `Build_Mesh` (the whole chain: weld, `Remove_Degenerate_Faces`, normals, stats, texture sort,
//! `Sort_Vertices`, `Strip_Optimize_Mesh`, `Verify_Face_Normals`) compared on the counts, all 132 stats words, every vertex
//! and face field the builder defines, `Compute_Bounding_Box` and `Compute_Bounding_Sphere`; `Add_Face`;
//! `Verify_Face_Normals` before and after a normal is moved; `Compute_Face_Normals` after a vertex is moved;
//! `FaceClass::Compute_Plane` with `Is_Degenerate`; and the default `Reset` of both classes. Words are compared as bits
//! (a NaN against a NaN is counted as `NaN-payload-only`).
//!
//! # Excluded inputs
//!
//! - `SharedSmGroup` of a face's own three vertices: `VertClass::Reset` never sets it, so upstream holds the heap garbage of the
//!   allocation (the draft holds 0). It is written as 0 on both sides. (The vertex array's `SharedSmGroup` is always defined:
//!   set for a new master, propagated for the rest.)
//! - A mesh whose every face is degenerate: `Compute_Mesh_Stats` then reads `Faces[0]` of a zero-length array (undefined), where the
//!   draft panics; every generated mesh has at least one face whose positions are pairwise 0.2 or more apart.
//! - Non-finite positions (the hash casts `(int)NaN`, undefined), `face_count_growth_rate < 1`, a polygon-ordering stage >= 2
//!   (`Texture_Compare_Funcs` has two columns), and the order of faces that compare equal under `qsort`: the draft sorts
//!   stably, MSVC's `qsort` is not stable; glibc 2.39's is (a merge sort), which `ref_dpw_qsort_is_stable` verifies at the
//!   start of the run, failing loudly if a libc ever changes that.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches except: `Compute_Mesh_Stats` and `Strip_Optimize_Mesh` are the long functions I did not diff line by line, so I
//! expect one or two misses there (an index or a missing branch); everything else is straight code. One thing I will check:
//! `Strip_Optimize_Mesh` leaves every non-first polygon of a strip with default `Verts`, a fact the draft must reproduce.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::ww3d::meshbuild::*;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dpw_meshbuild(input: *const u32, out: *mut u32) -> i32;
    fn ref_dpw_qsort_is_stable() -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
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

fn fb(f: f32) -> u32 {
    f.to_bits()
}

fn put_vert_in(w: &mut Vec<u32>, v: &VertClass) {
    w.extend_from_slice(&[fb(v.position.x), fb(v.position.y), fb(v.position.z), fb(v.normal.x), fb(v.normal.y), fb(v.normal.z)]);
    w.extend_from_slice(&[v.sm_group as u32, v.id as u32, v.bone_index as u32, v.max_vert_col_index as u32]);
    for p in 0..MAX_PASSES {
        for s in 0..MAX_STAGES {
            w.push(fb(v.tex_coord[p][s].x));
            w.push(fb(v.tex_coord[p][s].y));
        }
    }
    for arr in [&v.diffuse_color, &v.specular_color, &v.diffuse_illumination] {
        for p in 0..MAX_PASSES {
            w.extend_from_slice(&[fb(arr[p].x), fb(arr[p].y), fb(arr[p].z)]);
        }
    }
    for p in 0..MAX_PASSES {
        w.push(fb(v.alpha[p]));
    }
    for p in 0..MAX_PASSES {
        w.push(v.vertex_material_index[p] as u32);
    }
    w.push(v.attribute0 as u32);
    w.push(v.attribute1 as u32);
}

fn put_vert_out(w: &mut Vec<u32>, v: &VertClass, shared: bool) {
    put_vert_in(w, v);
    w.push(if shared { v.shared_sm_group as u32 } else { 0 });
    w.push(v.unique_index as u32);
    w.push(v.shade_index as u32);
}

fn put_face_in(w: &mut Vec<u32>, f: &FaceClass) {
    for i in 0..3 {
        put_vert_in(w, &f.verts[i]);
    }
    w.extend_from_slice(&[f.sm_group as u32, f.index as u32, f.attributes as u32]);
    for p in 0..MAX_PASSES {
        for s in 0..MAX_STAGES {
            w.push(f.texture_index[p][s] as u32);
        }
    }
    for p in 0..MAX_PASSES {
        w.push(f.shader_index[p] as u32);
    }
    w.push(f.surface_type);
}

fn put_face_out(w: &mut Vec<u32>, f: &FaceClass) {
    for i in 0..3 {
        put_vert_out(w, &f.verts[i], false);
    }
    w.extend_from_slice(&[f.sm_group as u32, f.index as u32, f.attributes as u32]);
    for p in 0..MAX_PASSES {
        for s in 0..MAX_STAGES {
            w.push(f.texture_index[p][s] as u32);
        }
    }
    for p in 0..MAX_PASSES {
        w.push(f.shader_index[p] as u32);
    }
    w.push(f.surface_type);
    w.push(f.add_index as u32);
    for i in 0..3 {
        w.push(f.vert_idx[i] as u32);
    }
    w.extend_from_slice(&[fb(f.normal.x), fb(f.normal.y), fb(f.normal.z), fb(f.dist)]);
}

fn put_stats(w: &mut Vec<u32>, s: &MeshStatsStruct) {
    for p in 0..MAX_PASSES {
        for t in 0..MAX_STAGES {
            w.push(u32::from(s.has_texture[p][t]));
        }
    }
    for p in 0..MAX_PASSES {
        w.push(u32::from(s.has_shader[p]));
    }
    for p in 0..MAX_PASSES {
        w.push(u32::from(s.has_vertex_material[p]));
    }
    for p in 0..MAX_PASSES {
        for t in 0..MAX_STAGES {
            w.push(u32::from(s.has_per_poly_texture[p][t]));
        }
    }
    for p in 0..MAX_PASSES {
        w.push(u32::from(s.has_per_poly_shader[p]));
    }
    for p in 0..MAX_PASSES {
        w.push(u32::from(s.has_per_vertex_material[p]));
    }
    for p in 0..MAX_PASSES {
        w.push(u32::from(s.has_diffuse_color[p]));
    }
    for p in 0..MAX_PASSES {
        w.push(u32::from(s.has_specular_color[p]));
    }
    for p in 0..MAX_PASSES {
        w.push(u32::from(s.has_diffuse_illumination[p]));
    }
    for p in 0..MAX_PASSES {
        for t in 0..MAX_STAGES {
            w.push(u32::from(s.has_tex_coords[p][t]));
        }
    }
    w.extend_from_slice(&[s.uv_split_count as u32, s.strip_count as u32, s.max_strip_length as u32, fb(s.avg_strip_length)]);
}

/// The stand-in `WorldInfoClass::Get_Shared_Vertex_Normal` (the same expression as the wrapper's).
fn shared_normal(pos: &Vector3, smgroup: i32) -> Vector3 {
    Vector3::new(pos.x * 0.5f32 + smgroup as f32, -pos.y, pos.z + (smgroup as f32) * 0.25f32)
}

fn is_nan_word(a: u32) -> bool {
    (a & 0x7F80_0000) == 0x7F80_0000 && (a & 0x007F_FFFF) != 0
}

/// Compare two word lists, counting each word; a NaN against a NaN of another payload is counted apart.
fn cmp_words(c: &mut Check, rust: &[u32], cpp: &[u32], what: impl Fn() -> String) {
    c.compared += rust.len().max(cpp.len()) as u64;
    if rust.len() != cpp.len() {
        c.mismatches += 1;
        if c.examples.len() < 8 {
            c.examples.push(format!("{}: {} words, c++ {}", what(), rust.len(), cpp.len()));
        }
        return;
    }
    for (i, (&a, &b)) in rust.iter().zip(cpp.iter()).enumerate() {
        if a != b {
            if is_nan_word(a) && is_nan_word(b) {
                c.nan_only += 1;
                continue;
            }
            c.mismatches += 1;
            if c.examples.len() < 8 {
                c.examples.push(format!("{} word {i}: rust {a:#010x} != c++ {b:#010x}", what()));
            }
        }
    }
}

/// A mesh case: the faces and the builder settings.
struct Mesh {
    faces: Vec<FaceClass>,
    pass_count: i32,
    guess: i32,
    growth: i32,
    opass: usize,
    ostage: usize,
    compute_normals: bool,
    world_mode: u32,
}

fn sm(g: &mut Gen) -> i32 {
    [0, 1, 1, 2, 3, 4, 6, 8, 12][g.below(9) as usize]
}

fn make_mesh(g: &mut Gen) -> Mesh {
    let pass_count = 1 + g.below(4) as i32;
    let flat = g.below(8) == 0;
    let grid = g.below(4) == 0;
    // the point templates
    let (pool_n, w, h) = if grid {
        let w = 1 + g.below(4) as usize;
        let h = 1 + g.below(4) as usize;
        ((w + 1) * (h + 1), w, h)
    } else {
        (3 + g.below(12) as usize, 0, 0)
    };
    let mut pool: Vec<VertClass> = Vec::new();
    for k in 0..pool_n {
        let mut v = VertClass::new();
        let (x, y, z) = if grid {
            ((k % (w + 1)) as f32, (k / (w + 1)) as f32, if g.below(3) == 0 { 0.5 } else { 0.0 })
        } else {
            ((g.below(9) as f32) * 0.5, (g.below(9) as f32) * 0.5, (g.below(5) as f32) * 0.25)
        };
        v.position.set(if flat { 1.0 } else { x }, y, z);
        let nn = [Vector3::new(0.0, 0.0, 1.0), Vector3::new(0.0, 1.0, 0.0), Vector3::new(0.6, 0.0, 0.8)][g.below(3) as usize];
        v.normal = nn;
        v.id = if g.below(10) == 0 { 1 } else { 0 };
        v.bone_index = g.below(3) as i32;
        for p in 0..MAX_PASSES {
            v.vertex_material_index[p] = g.below(3) as i32 - 1;
            if g.below(4) == 0 {
                v.diffuse_color[p].set(0.5, 0.25, 1.0);
            }
            if g.below(6) == 0 {
                v.specular_color[p].set(0.0, 0.5, 0.5);
            }
            if g.below(8) == 0 {
                v.diffuse_illumination[p].set(0.1, 0.2, 0.3);
            }
            if g.below(6) == 0 {
                v.alpha[p] = 0.5;
            }
            for s in 0..MAX_STAGES {
                if s < 2 || g.below(10) == 0 {
                    let u = [0.0, 0.25, 0.5, 1.0][g.below(4) as usize];
                    let vv = [0.0, 0.5, 1.0][g.below(3) as usize];
                    v.tex_coord[p][s] = Vector2::new(u, vv);
                }
            }
        }
        v.max_vert_col_index = g.below(3) as i32;
        v.attribute0 = g.below(3) as i32;
        v.attribute1 = g.below(2) as i32;
        pool.push(v);
    }
    // the faces
    let mut tris: Vec<[usize; 3]> = Vec::new();
    if grid {
        for yy in 0..h {
            for xx in 0..w {
                let a = yy * (w + 1) + xx;
                let (b, c, d) = (a + 1, a + w + 1, a + w + 2);
                if g.below(2) == 0 {
                    tris.push([a, b, d]);
                    tris.push([a, d, c]);
                } else {
                    tris.push([a, b, c]);
                    tris.push([b, d, c]);
                }
            }
        }
        // a few random extras (duplicates, flipped)
        for _ in 0..g.below(3) {
            let t = tris[g.below(tris.len() as u32) as usize];
            tris.push(if g.below(2) == 0 { t } else { [t[0], t[2], t[1]] });
        }
        for i in (1..tris.len()).rev() {
            let j = g.below(i as u32 + 1) as usize;
            tris.swap(i, j);
        }
    } else {
        let n = 1 + g.below(30);
        for _ in 0..n {
            let a = g.below(pool_n as u32) as usize;
            let mut b = g.below(pool_n as u32) as usize;
            let mut c = g.below(pool_n as u32) as usize;
            if g.below(12) != 0 {
                while b == a {
                    b = g.below(pool_n as u32) as usize;
                }
                while c == a || c == b {
                    c = g.below(pool_n as u32) as usize;
                }
            }
            tris.push([a, b, c]);
        }
    }
    let mut faces = Vec::new();
    for (fi, t) in tris.iter().enumerate() {
        let mut f = FaceClass::new();
        f.sm_group = sm(g);
        f.index = fi as i32;
        f.attributes = g.below(3) as i32;
        f.surface_type = g.below(3);
        for p in 0..MAX_PASSES {
            f.shader_index[p] = g.below(3) as i32 - 1;
            for s in 0..MAX_STAGES {
                f.texture_index[p][s] = if s < 2 || g.below(4) == 0 { g.below(4) as i32 - 1 } else { -1 };
            }
        }
        for k in 0..3 {
            let mut v = pool[t[k]].clone();
            match g.below(8) {
                0 => v.tex_coord[0][0] = Vector2::new(0.75, 0.75),
                1 => v.vertex_material_index[0] = g.below(3) as i32,
                2 => v.diffuse_color[0].set(0.0, 1.0, 0.0),
                3 => v.id = 2,
                4 => {
                    // a nudge of the weld epsilon: smaller, equal, larger
                    let d = [0.00003f32, 0.0001f32, 0.00011f32][g.below(3) as usize];
                    v.position.x += d;
                }
                5 => v.alpha[0] = 0.75,
                _ => {}
            }
            f.verts[k] = v;
        }
        faces.push(f);
    }
    // At least one face must survive `Remove_Degenerate_Faces`: with none, upstream's `Compute_Mesh_Stats` reads `Faces[0]` of a
    // zero-length array (undefined). A face whose three positions are pairwise 0.2 or more apart cannot be welded or equal.
    let far = |f: &FaceClass| {
        (0..3).all(|i| {
            ((i + 1)..3).all(|j| {
                let d = f.verts[i].position - f.verts[j].position;
                d.length() >= 0.2
            })
        })
    };
    if !faces.iter().any(|f| far(f)) {
        let mut f = FaceClass::new();
        f.sm_group = 1;
        f.index = faces.len() as i32;
        f.verts[0].position.set(1.0, 0.0, 0.0);
        f.verts[1].position.set(1.0, 1.0, 0.0);
        f.verts[2].position.set(1.0, 0.0, 1.0);
        faces.push(f);
    }
    let guess = [0, 1, 2, 3, 5, 8, 255][g.below(7) as usize];
    let growth = 1 + g.below(4) as i32;
    Mesh {
        faces,
        pass_count,
        guess,
        growth,
        opass: g.below(4) as usize,
        ostage: g.below(2) as usize,
        compute_normals: g.below(2) == 0,
        world_mode: g.below(3),
    }
}

fn header(op: u32, m: &Mesh) -> Vec<u32> {
    let mut w = vec![op, m.pass_count as u32, m.guess as u32, m.growth as u32, m.opass as u32, m.ostage as u32, u32::from(m.compute_normals), m.world_mode, m.faces.len() as u32];
    for f in &m.faces {
        put_face_in(&mut w, f);
    }
    w
}

fn builder(m: &Mesh) -> MeshBuilderClass {
    let mut mb = MeshBuilderClass::new(m.pass_count, m.guess, m.growth);
    mb.set_polygon_ordering_channel(m.opass, m.ostage);
    if m.world_mode != 0 {
        mb.set_world_info(Some(WorldInfoStub { meshes_smoothed: m.world_mode == 1, shared_vertex_normal: Some(shared_normal) }));
    }
    mb
}

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 200_000];
    let n = unsafe { ref_dpw_meshbuild(input.as_ptr(), out.as_mut_ptr()) } as usize;
    out.truncate(n);
    out
}

#[test]
fn meshbuild_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(unsafe { ref_dpw_qsort_is_stable() }, 1, "this libc's qsort is not stable: the draft's stable sort cannot be compared tie for tie");
    let mut g = Gen::new(0x215_14_03);
    let mut c_build = Check::new("MeshBuilderClass::Build_Mesh (counts, stats, verts, faces, bounds)");
    let mut c_add = Check::new("MeshBuilderClass::Add_Face / Grow_Face_Array");
    let mut c_ver = Check::new("MeshBuilderClass::Verify_Face_Normals");
    let mut c_cfn = Check::new("MeshBuilderClass::Compute_Face_Normals");
    let mut c_plane = Check::new("FaceClass::Compute_Plane / Is_Degenerate");
    let mut c_reset = Check::new("VertClass::Reset / FaceClass::Reset");
    for case in 0..cases() {
        let m = make_mesh(&mut g);
        // Build_Mesh
        {
            let cpp = run(&header(0, &m));
            let mut mb = builder(&m);
            for f in &m.faces {
                mb.add_face(f);
            }
            mb.build_mesh(m.compute_normals);
            let mut w: Vec<u32> = vec![mb.get_vertex_count() as u32, mb.get_face_count() as u32];
            put_stats(&mut w, mb.get_mesh_stats());
            for i in 0..mb.get_vertex_count() as usize {
                put_vert_out(&mut w, mb.get_vertex(i), true);
            }
            for i in 0..mb.get_face_count() as usize {
                put_face_out(&mut w, mb.get_face(i));
            }
            let (mn, mx) = mb.compute_bounding_box();
            w.extend_from_slice(&[fb(mn.x), fb(mn.y), fb(mn.z), fb(mx.x), fb(mx.y), fb(mx.z)]);
            let (c, r) = mb.compute_bounding_sphere();
            w.extend_from_slice(&[fb(c.x), fb(c.y), fb(c.z), fb(r)]);
            cmp_words(&mut c_build, &w, &cpp, || format!("case {case}"));
            // Verify_Face_Normals before and after a face normal is moved
            let delta = [0.0f32, 0.5, 1e-8, -2.0][g.below(4) as usize];
            let mut inp = header(1, &m);
            inp.push(fb(delta));
            let cpp = run(&inp);
            let before = mb.verify_face_normals();
            if mb.get_face_count() > 0 {
                mb.get_face_mut(0).normal.x += delta;
            }
            let after = mb.verify_face_normals();
            cmp_words(&mut c_ver, &[u32::from(before), u32::from(after)], &cpp, || format!("case {case}"));
            // Compute_Face_Normals after a vertex is moved
            let delta = [0.0f32, 0.25, -1.0][g.below(3) as usize];
            let mut inp = header(2, &m);
            inp.push(fb(delta));
            let cpp = run(&inp);
            let mut mb = builder(&m);
            for f in &m.faces {
                mb.add_face(f);
            }
            mb.build_mesh(m.compute_normals);
            mb.get_vertex_mut(0).position.x += delta;
            mb.compute_face_normals();
            let mut w = vec![u32::from(mb.verify_face_normals())];
            for i in 0..mb.get_face_count() as usize {
                let f = mb.get_face(i);
                w.extend_from_slice(&[fb(f.normal.x), fb(f.normal.y), fb(f.normal.z), fb(f.dist)]);
            }
            cmp_words(&mut c_cfn, &w, &cpp, || format!("case {case}"));
        }
        // Add_Face
        {
            let cpp = run(&header(5, &m));
            let mut mb = builder(&m);
            let mut w: Vec<u32> = Vec::new();
            for f in &m.faces {
                w.push(mb.add_face(f) as u32);
            }
            // the faces as the builder holds them before processing (the draft's array is private; read through the result of a
            // second builder is not possible, so only the returned indices and the face-level fields it exposes are compared)
            w.truncate(m.faces.len());
            cmp_words(&mut c_add, &w, &cpp[..m.faces.len().min(cpp.len())], || format!("case {case}"));
        }
        // Compute_Plane / Is_Degenerate
        {
            let mut f = m.faces[0].clone();
            f.vert_idx = [g.below(4) as i32, g.below(4) as i32, g.below(4) as i32];
            let mut inp = vec![3u32];
            put_face_in(&mut inp, &f);
            inp.extend_from_slice(&[f.vert_idx[0] as u32, f.vert_idx[1] as u32, f.vert_idx[2] as u32]);
            let cpp = run(&inp);
            let d = f.is_degenerate();
            f.compute_plane();
            let w = [fb(f.normal.x), fb(f.normal.y), fb(f.normal.z), fb(f.dist), u32::from(d)];
            cmp_words(&mut c_plane, &w, &cpp, || format!("case {case}"));
        }
        // Reset
        if case == 0 {
            let cpp = run(&[4u32]);
            let mut w = Vec::new();
            put_vert_out(&mut w, &VertClass::new(), false);
            put_face_out(&mut w, &FaceClass::new());
            // the vertex's SharedSmGroup is garbage upstream (see the header): word 118 of the vertex block
            let mut cpp = cpp;
            cpp[118] = 0;
            w[118] = 0;
            // each of the face's three vertices likewise
            for k in 0..3 {
                let at = 121 + k * 121 + 118;
                cpp[at] = 0;
                w[at] = 0;
            }
            cmp_words(&mut c_reset, &w, &cpp, || "reset".to_string());
        }
    }
    finish(vec![c_build, c_add, c_ver, c_cfn, c_plane, c_reset]);
}
