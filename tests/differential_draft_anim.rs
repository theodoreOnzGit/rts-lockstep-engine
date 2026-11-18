// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215) of the W3D hierarchy, animation, quaternion and vector-processor drafts
//! (`src/drafts/sweep_ww3d/{htree,hrawanim,hcanim,quat,vp}.rs`) against upstream's compiled C++ (commit 0a05454d):
//! `HTreeClass::Alter_Avatar_HTree`, `Create_Morphed`, both `Create_Interpolated`; `HRawAnimClass` and
//! `HCompressedAnimClass` `Has_*`; `Quaternion::Normalize`, `Make_Closest`, `Randomize`; `VectorProcessorClass::CopyIndexed`
//! (six overloads) and `Prefetch`. The C++ is `differential/wrap_draft_htree.cpp` over the htree.cpp, hrawanim.cpp, hcanim.cpp
//! and quat.cpp the W3D tests already compile, plus a verbatim cut of vp.cpp (build.rs `drafts_anim`).
//!
//! # Method
//!
//! Hierarchies and animations are built in memory (tests/draft_anim_bytes; nothing of the game's files) as W3D chunks and
//! loaded by BOTH the real C++ loaders and the verified Rust ones. Hierarchy cases (`DRAFT_ANIM_CASES`, default 20,000, fixed
//! seed): 1 to 9 pivots, a pool of names that includes every name on `Alter_Avatar_HTree`'s flip list and near misses, four
//! hierarchies sharing names and parents with random translations and rotations, each updated to a random root with
//! `Base_Update` (so the pivots' `Transform` is not the base pose), then the four statics with random scales, weights and
//! interpolants; every pivot's parent, index, `BaseTransform` and `Transform` are compared as bits. `Has_*`: random
//! channel sets on 1 to 9 nodes, five answers per node. Quaternions: ordinary, zero, tiny, huge and non-finite
//! components. `Randomize`: libc `srand(seed)` before each side. `CopyIndexed`: six element types, counts from -2.
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! 0 mismatches in every function but those the documentation of a draft already flags; I did NOT expect any deviation in
//! the hierarchy statics (same `float` operations in the same order). All planted bugs caught, the least often being the
//! flip-list ones (a case needs a listed name with a parent).

mod draft_anim_bytes;
mod support;

use draft_anim_bytes::*;
use rts_lockstep_engine::drafts::sweep_ww3d::{hcanim, hrawanim, htree as dtree, quat as dquat, vp};
use rts_lockstep_engine::formats::w3d::hanim::{HCompressedAnim, HRawAnim};
use rts_lockstep_engine::formats::w3d::htree::{self, HTree};
use rts_lockstep_engine::ww::chunkio::{ChunkLoad, GameFile};
use rts_lockstep_engine::ww::quat::Quaternion;
use rts_lockstep_engine::ww::vector2::Vector2;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dah_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
    fn srand(seed: u32);
    fn rand() -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run_locked(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 200_000];
    let n = unsafe { ref_dah_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "the wrapper refused op {}", input[0]);
    out.truncate(n as usize);
    out
}

fn bits(f: f32) -> u32 {
    f.to_bits()
}

fn load_tree(bytes: &[u8]) -> HTree {
    let mut c = ChunkLoad::new(GameFile::new(bytes).unwrap());
    assert!(c.open_chunk().unwrap());
    htree::load_w3d(&mut c).unwrap().expect("a well-formed hierarchy loads")
}

fn dump(t: &HTree) -> Vec<u32> {
    let mut v = vec![t.num_pivots as u32, bits(t.scale_factor)];
    for p in &t.pivots {
        v.push(p.parent.unwrap_or(0xFFFF_FFFF));
        v.push(p.index as u32);
        v.extend(p.base_transform.to_array().iter().map(|x| bits(*x)));
        v.extend(p.transform.to_array().iter().map(|x| bits(*x)));
    }
    v
}

const FLIP: [&[u8]; 14] = [
    b" RIGHTFOREARM", b" RIGHTHAND", b" LEFTFOREARM", b" LEFTHAND", b"RIGHTINDEX", b"RIGHTFINGERS", b"RIGHTTHUMB", b"LEFTINDEX",
    b"LEFTFINGERS", b"LEFTTHUMB", b"RIGHTHAND", b"leftindex", b" RIGHTFOREARMX", b"LEFTTHUMB ",
];

fn pivot_names(g: &mut Gen, n: usize) -> Vec<Vec<u8>> {
    (0..n)
        .map(|i| if g.below(5) < 2 { FLIP[g.below(FLIP.len() as u32) as usize].to_vec() } else { format!("B{i}").into_bytes() })
        .collect()
}

fn unit_quat(g: &mut Gen) -> [f32; 4] {
    let mut q = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
    let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).max(1e-6);
    let s = 1.0 / petir::Real::sqrt(l);
    for x in q.iter_mut() {
        *x *= s;
    }
    q
}

fn random_matrix(g: &mut Gen) -> [f32; 12] {
    if g.below(10) < 3 {
        [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
    } else {
        let mut a = [0.0f32; 12];
        for x in a.iter_mut() {
            *x = g.uniform(-2.0, 2.0);
        }
        a
    }
}

/// Four hierarchies with the same names and parents; each is registered with both sides and updated to its own root.
fn make_trees(g: &mut Gen) -> Vec<HTree> {
    let n = 1 + g.below(9) as usize;
    let names = pivot_names(g, n);
    let parents: Vec<u32> = (0..n).map(|i| if i == 0 { 0xFFFF_FFFF } else { g.below(i as u32) }).collect();
    let mut inp = vec![0u32];
    run_locked(&inp);
    let mut trees = Vec::new();
    for k in 0..4 {
        let pivots: Vec<PivotSpec> = (0..n)
            .map(|i| PivotSpec {
                name: names[i].clone(),
                parent: parents[i],
                t: [g.uniform(-5.0, 5.0), g.uniform(-5.0, 5.0), g.uniform(-5.0, 5.0)],
                e: [0.0; 3],
                q: unit_quat(g),
            })
            .collect();
        let bytes = tree_chunk(format!("T{k}").as_bytes(), &pivots);
        inp = vec![1];
        words_blob(&bytes, &mut inp);
        let r = run_locked(&inp);
        assert_eq!(r[0], k as u32);
        let mut t = load_tree(&bytes);
        let root = random_matrix(g);
        let m = mat(root);
        t.base_update(&m).unwrap();
        inp = vec![4, k as u32];
        inp.extend(root.iter().map(|x| bits(*x)));
        run_locked(&inp);
        trees.push(t);
    }
    trees
}

fn scale_component(g: &mut Gen) -> f32 {
    match g.below(10) {
        0 => 0.0,
        1 => 1.0,
        2 => -g.uniform(0.2, 2.0),
        _ => g.uniform(0.2, 3.0),
    }
}

fn lerp_amount(g: &mut Gen) -> f32 {
    match g.below(8) {
        0 => 0.0,
        1 => 1.0,
        _ => g.uniform(-0.5, 1.5),
    }
}

#[test]
fn hierarchy_statics_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0001);
    let mut alter = Check::new("Alter_Avatar_HTree");
    let mut morphed = Check::new("Create_Morphed");
    let mut interp4 = Check::new("Create_Interpolated (bilinear)");
    let mut interp3 = Check::new("Create_Interpolated (two targets)");
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let (mut flips, mut zeroes) = (0u64, 0u64);
    for case in 0..cases() {
        let trees = make_trees(&mut g);
        let n = trees[0].num_pivots as usize;
        flips += trees[0].pivots.iter().filter(|p| p.parent.is_some() && FLIP[..10].iter().any(|f| p.name.starts_with(f) && p.name[f.len()..].iter().all(|&c| c == 0))).count() as u64;
        // Alter_Avatar_HTree
        let s = [scale_component(&mut g), scale_component(&mut g), scale_component(&mut g)];
        let t = g.below(4);
        let r = dtree::alter_avatar_htree(&trees[t as usize], &Vector3::new(s[0], s[1], s[2]));
        let c = run_locked(&[5, t, bits(s[0]), bits(s[1]), bits(s[2])]);
        assert_eq!(c[0], 4);
        let cd = run_locked(&[9, c[0]]);
        alter.eq(dump(&r), cd, || format!("case {case}: scale {s:?} tree {t} n {n}"));
        // Create_Morphed
        let k = 1 + g.below(4) as usize;
        let mut inp = vec![6, k as u32];
        let mut idx = Vec::new();
        let mut w = Vec::new();
        for _ in 0..k {
            let i = g.below(4);
            let wt = match g.below(6) {
                0 => 0.0,
                1 => 1.0,
                _ => g.uniform(0.0, 1.0),
            };
            inp.extend([i, bits(wt)]);
            idx.push(i as usize);
            w.push(wt);
        }
        let refs: Vec<&HTree> = idx.iter().map(|&i| &trees[i]).collect();
        let r = dtree::create_morphed(k as i32, &w, &refs);
        let c = run_locked(&inp);
        let cd = run_locked(&[9, c[0]]);
        morphed.eq(dump(&r), cd, || format!("case {case}: {k} sources {idx:?} weights {w:?}"));
        // Create_Interpolated (bilinear)
        let (la, lb) = (lerp_amount(&mut g), lerp_amount(&mut g));
        let r = dtree::create_interpolated_bilinear(&trees[0], &trees[1], &trees[2], &trees[3], la, lb);
        let c = run_locked(&[7, 0, 1, 2, 3, bits(la), bits(lb)]);
        let cd = run_locked(&[9, c[0]]);
        interp4.eq(dump(&r), cd, || format!("case {case}: lerp_a {la} lerp_b {lb}"));
        // Create_Interpolated (two targets)
        let (sa, sb) = match g.below(10) {
            0 => (0.0, 0.0),
            1 => (0.0, scale_component(&mut g)),
            2 => (scale_component(&mut g), 0.0),
            _ => (g.uniform(-1.5, 1.5), g.uniform(-1.5, 1.5)),
        };
        zeroes += u64::from(sa == 0.0 && sb == 0.0);
        let r = dtree::create_interpolated(&trees[0], &trees[1], &trees[2], sa, sb);
        let c = run_locked(&[8, 0, 1, 2, bits(sa), bits(sb)]);
        let cd = run_locked(&[9, c[0]]);
        interp3.eq(dump(&r), cd, || format!("case {case}: a_scale {sa} b_scale {sb}"));
    }
    println!("{} cases, {flips} listed flip-list pivots with a parent, {zeroes} both-zero interpolations", cases());
    let which = if planted == 0 { 0 } else { planted };
    let _ = which;
    if planted != 0 {
        let m = alter.report() + morphed.report() + interp4.report() + interp3.report();
        panic!("a planted run: {m} mismatches (see the MEASURED lines)");
    } else {
        let (a, b, c, d) = (alter, morphed, interp4, interp3);
        a.finish();
        b.finish();
        c.finish();
        d.finish();
    }
}

/// The five `Has_*` answers per node, on a random channel set.
#[test]
fn has_channel_queries_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0002);
    let mut raw_c = Check::new("HRawAnimClass::Has_*");
    let mut comp_c = Check::new("HCompressedAnimClass::Has_*");
    let mut present = [0u64; 2];
    for case in 0..cases() {
        let n = 1 + g.below(9) as usize;
        run_locked(&[0]);
        let pivots: Vec<PivotSpec> = (0..n)
            .map(|i| PivotSpec { name: format!("B{i}").into_bytes(), parent: if i == 0 { 0xFFFF_FFFF } else { g.below(i as u32) }, t: [0.0; 3], e: [0.0; 3], q: [0.0, 0.0, 0.0, 1.0] })
            .collect();
        let tb = tree_chunk(b"SK", &pivots);
        let mut inp = vec![1];
        words_blob(&tb, &mut inp);
        run_locked(&inp);
        let lookup = |name: &[u8; 16]| -> Option<i32> { if name == &name16(b"SK") { Some(n as i32) } else { None } };
        // raw
        let mut chans = Vec::new();
        let mut vis = Vec::new();
        for p in 0..n as u16 {
            for f in 0..7u16 {
                if g.below(2) == 0 {
                    chans.push(RawChan { flags: f, pivot: p, frames: 2 + g.below(3) as u16 });
                }
            }
            if g.below(2) == 0 {
                vis.push(p);
            }
        }
        let mut v = 0.5f32;
        let bytes = raw_anim_chunk(b"A", b"SK", 4, &chans, &vis, || { v += 0.25; v });
        let mut c = ChunkLoad::new(GameFile::new(&bytes).unwrap());
        assert!(c.open_chunk().unwrap());
        let a = HRawAnim::load_w3d(&mut c, &mut { lookup }).unwrap().expect("raw anim loads");
        inp = vec![2];
        words_blob(&bytes, &mut inp);
        assert_eq!(run_locked(&inp)[0], 0);
        for p in 0..n as i32 {
            let r = [
                hrawanim::has_x_translation(&a, p),
                hrawanim::has_y_translation(&a, p),
                hrawanim::has_z_translation(&a, p),
                hrawanim::has_rotation(&a, p),
                hrawanim::has_visibility(&a, p),
            ];
            present[0] += r.iter().filter(|b| **b).count() as u64;
            let cc = run_locked(&[10, 0, p as u32]);
            raw_c.eq(r.iter().map(|b| u32::from(*b)).collect::<Vec<_>>(), cc, || format!("case {case}: raw node {p}"));
        }
        // compressed
        let mut chans = Vec::new();
        let mut vis = Vec::new();
        for p in 0..n as u16 {
            for f in 7..11u8 {
                if g.below(2) == 0 {
                    chans.push((f, p, 2 + g.below(3)));
                }
            }
            if g.below(2) == 0 {
                vis.push(p);
            }
        }
        let bytes = comp_anim_chunk(b"C", b"SK", 8, &chans, &vis, || { v += 0.25; v });
        let mut c = ChunkLoad::new(GameFile::new(&bytes).unwrap());
        assert!(c.open_chunk().unwrap());
        let a = HCompressedAnim::load_w3d(&mut c, &mut { lookup }).unwrap().expect("compressed anim loads");
        inp = vec![3];
        words_blob(&bytes, &mut inp);
        assert_eq!(run_locked(&inp)[0], 0);
        for p in 0..n as i32 {
            let r = [
                hcanim::has_x_translation(&a, p),
                hcanim::has_y_translation(&a, p),
                hcanim::has_z_translation(&a, p),
                hcanim::has_rotation(&a, p),
                hcanim::has_visibility(&a, p),
            ];
            present[1] += r.iter().filter(|b| **b).count() as u64;
            let cc = run_locked(&[11, 0, p as u32]);
            comp_c.eq(r.iter().map(|b| u32::from(*b)).collect::<Vec<_>>(), cc, || format!("case {case}: compressed node {p}"));
        }
    }
    println!("channels present (of all asked): raw {}, compressed {}", present[0], present[1]);
    finish2(raw_c, comp_c);
}

fn finish2(a: Check, b: Check) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    if planted {
        let m = a.report() + b.report();
        panic!("a planted run: {m} mismatches (see the MEASURED lines)");
    }
    a.finish();
    b.finish();
}

fn quat_component(g: &mut Gen) -> f32 {
    match g.below(10) {
        0 => 0.0,
        1 => g.uniform(-1.0e-22, 1.0e-22),
        2 => g.mixed(),
        3 => g.uniform(-1.0e19, 1.0e19),
        _ => g.uniform(-1.0, 1.0),
    }
}

fn gen_quat(g: &mut Gen) -> [f32; 4] {
    if g.below(12) == 0 {
        [0.0; 4]
    } else {
        [quat_component(g), quat_component(g), quat_component(g), quat_component(g)]
    }
}

fn q(a: [f32; 4]) -> Quaternion {
    Quaternion::new(a[0], a[1], a[2], a[3])
}

fn qbits(q: &Quaternion) -> Vec<u32> {
    vec![bits(q.x), bits(q.y), bits(q.z), bits(q.w)]
}

#[test]
fn quaternion_normalize_closest_randomize_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0003);
    let mut norm = Check::new("Quaternion::Normalize");
    let mut closest = Check::new("Quaternion::Make_Closest");
    let mut random = Check::new("Quaternion::Randomize");
    for case in 0..cases() * 2 {
        let a = gen_quat(&mut g);
        let mut r = q(a);
        dquat::normalize(&mut r);
        let mut inp = vec![12];
        inp.extend(a.iter().map(|x| bits(*x)));
        let cc = run_locked(&inp);
        let (rb, cb) = (qbits(&r), cc);
        for k in 0..4 {
            norm.f32(f32::from_bits(rb[k]), f32::from_bits(cb[k]), || format!("case {case}: {a:?} component {k}"));
        }
        let to = if g.below(4) == 0 { [-a[0], -a[1], -a[2], -a[3]] } else { gen_quat(&mut g) };
        let mut r = q(a);
        dquat::make_closest(&mut r, &q(to));
        let mut inp = vec![13];
        inp.extend(a.iter().map(|x| bits(*x)));
        inp.extend(to.iter().map(|x| bits(*x)));
        let cb = run_locked(&inp);
        let rb = qbits(&r);
        for k in 0..4 {
            closest.f32(f32::from_bits(rb[k]), f32::from_bits(cb[k]), || format!("case {case}: {a:?} to {to:?} component {k}"));
        }
        if case % 2 == 0 {
            let seed = g.u32();
            let cb = run_locked(&[14, seed]);
            let mut r = q([9.0; 4]);
            unsafe { srand(seed) };
            dquat::randomize(&mut r, || unsafe { rand() });
            let rb = qbits(&r);
            for k in 0..4 {
                random.f32(f32::from_bits(rb[k]), f32::from_bits(cb[k]), || format!("case {case}: seed {seed} component {k}"));
            }
        }
    }
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    if planted {
        let m = norm.report() + closest.report() + random.report();
        panic!("a planted run: {m} mismatches (see the MEASURED lines)");
    }
    norm.finish();
    closest.finish();
    random.finish();
}

fn words_f32(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| bits(*x)).collect()
}

#[test]
fn copy_indexed_and_prefetch_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0004);
    let mut copy = Check::new("VectorProcessorClass::CopyIndexed");
    let mut counts = [0u64; 3];
    for case in 0..cases() {
        let ty = g.below(6);
        let width = [1usize, 2, 3, 4, 1, 1][ty as usize];
        let nsrc = 1 + g.below(12) as usize;
        let ndst = 1 + g.below(12) as usize;
        let count = g.below(ndst as u32 + 3) as i32 - 2;
        counts[(count.signum() + 1) as usize] += 1;
        let nidx = count.max(0) as usize + g.below(2) as usize;
        let rnd = |g: &mut Gen| -> u32 { if ty == 4 { g.below(256) } else { g.u32() } };
        let src: Vec<u32> = (0..nsrc * width).map(|_| rnd(&mut g)).collect();
        let dst: Vec<u32> = (0..ndst * width).map(|_| rnd(&mut g)).collect();
        let idx: Vec<u32> = (0..nidx).map(|_| g.below(nsrc as u32)).collect();
        let mut inp = vec![15, ty, nsrc as u32, ndst as u32, count as u32, nidx as u32];
        inp.extend(&src);
        inp.extend(&dst);
        inp.extend(&idx);
        let cc = run_locked(&inp);
        let mut d = dst.clone();
        let f = |w: &[u32]| -> Vec<f32> { w.iter().map(|b| f32::from_bits(*b)).collect() };
        let r: Vec<u32> = match ty {
            0 => {
                vp::copy_indexed_u32(&mut d, &src, &idx, count);
                d
            }
            1 => {
                let s: Vec<Vector2> = f(&src).chunks(2).map(|c| Vector2::new(c[0], c[1])).collect();
                let mut dv: Vec<Vector2> = f(&dst).chunks(2).map(|c| Vector2::new(c[0], c[1])).collect();
                vp::copy_indexed_vector2(&mut dv, &s, &idx, count);
                dv.iter().flat_map(|v| [bits(v.x), bits(v.y)]).collect()
            }
            2 => {
                let s: Vec<Vector3> = f(&src).chunks(3).map(|c| Vector3::new(c[0], c[1], c[2])).collect();
                let mut dv: Vec<Vector3> = f(&dst).chunks(3).map(|c| Vector3::new(c[0], c[1], c[2])).collect();
                vp::copy_indexed_vector3(&mut dv, &s, &idx, count);
                dv.iter().flat_map(|v| [bits(v.x), bits(v.y), bits(v.z)]).collect()
            }
            3 => {
                let s: Vec<Vector4> = f(&src).chunks(4).map(|c| Vector4::new(c[0], c[1], c[2], c[3])).collect();
                let mut dv: Vec<Vector4> = f(&dst).chunks(4).map(|c| Vector4::new(c[0], c[1], c[2], c[3])).collect();
                vp::copy_indexed_vector4(&mut dv, &s, &idx, count);
                dv.iter().flat_map(|v| [bits(v.x), bits(v.y), bits(v.z), bits(v.w)]).collect()
            }
            4 => {
                let s: Vec<u8> = src.iter().map(|x| *x as u8).collect();
                let mut db: Vec<u8> = dst.iter().map(|x| *x as u8).collect();
                vp::copy_indexed_u8(&mut db, &s, &idx, count);
                db.iter().map(|x| u32::from(*x)).collect()
            }
            _ => {
                let s = f(&src);
                let mut df = f(&dst);
                vp::copy_indexed_f32(&mut df, &s, &idx, count);
                words_f32(&df)
            }
        };
        copy.eq(r, cc, || format!("case {case}: type {ty} count {count} nsrc {nsrc} ndst {ndst}"));
    }
    // Prefetch: does nothing on either side
    vp::prefetch();
    assert_eq!(run_locked(&[16]).len(), 0);
    println!("counts: negative {}, zero {}, positive {}", counts[0], counts[1], counts[2]);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    if planted {
        copy.report();
        panic!("a planted run: see the MEASURED line");
    }
    copy.finish();
}
