// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215) of the `Animatable3DObjClass` drafts (`src/drafts/sweep_ww3d/animobj.rs`):
//! `Simple_Evaluate_Bone(bone, tm)`, `Simple_Evaluate_Bone(bone, frame, tm)` and `Set_HTree` against upstream's text, cut
//! verbatim by build.rs (`drafts_anim`, differential/wrap_draft_animobj.cpp), with the real hierarchy and raw animation.
//!
//! # Method
//!
//! Cases (default 20,000, fixed seed): a hierarchy of 1 to 8 pivots and a raw animation on random nodes, both built in memory
//! and loaded by the real C++ loaders and the verified Rust ones; an object in one of the five motion modes (`NONE`,
//! `BASE_POSE`, `SINGLE_ANIM`, `DOUBLE_ANIM`, `MULTIPLE_ANIM`) with a random transform and frame (the mode is
//! `ANIM_MODE_MANUAL`, so `Compute_Current_Frame` is the frame in `SINGLE_ANIM` and 0 otherwise); 1 to 6 evaluations of random
//! bones, either overload, each with a pre-filled matrix; then `Set_HTree` with a second hierarchy. Compared: the return
//! value, all 12 words of the matrix, the number of `Update_Sub_Object_Transforms` calls, and the final hierarchy. The
//! Rust host (this file) evaluates with the verified `HTree::simple_evaluate_pivot*` and `base_update`.
//!
//! Excluded (upstream dereferences NULL): the modes `DOUBLE_ANIM` and `MULTIPLE_ANIM` with no hierarchy; out-of-range bones.
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! 0 mismatches: the draft is the same branch structure as upstream's.

mod draft_anim_bytes;
mod support;

use draft_anim_bytes::*;
use rts_lockstep_engine::drafts::sweep_ww3d::animobj::{self as dao, Animatable3DState, AnimatableHost, MotionMode};
use rts_lockstep_engine::formats::w3d::assetmgr::HAnim;
use rts_lockstep_engine::formats::w3d::hanim::HRawAnim;
use rts_lockstep_engine::formats::w3d::hmorphanim::NoPoses;
use rts_lockstep_engine::formats::w3d::htree::{self, HTree};
use rts_lockstep_engine::ww::chunkio::{ChunkLoad, GameFile};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dao_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 20_000];
    let n = unsafe { ref_dao_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

fn bits(f: f32) -> u32 {
    f.to_bits()
}

struct Host {
    tree: Option<HTree>,
    anim: Option<HAnim>,
    frame: f32,
    mode: MotionMode,
    transform: Matrix3D,
    updates: u32,
}

impl AnimatableHost for Host {
    fn compute_current_frame(&self) -> f32 {
        if self.mode == MotionMode::SingleAnim { self.frame } else { 0.0 }
    }
    fn htree_simple_evaluate_pivot_anim(&self, boneindex: i32, frame: f32, obj_tm: &Matrix3D, tm: &mut Matrix3D) -> bool {
        let (ok, m) = self.tree.as_ref().unwrap().simple_evaluate_pivot_anim(self.anim.as_ref().unwrap(), &NoPoses, boneindex, frame, obj_tm).unwrap();
        *tm = m;
        ok
    }
    fn htree_simple_evaluate_pivot(&self, boneindex: i32, obj_tm: &Matrix3D, tm: &mut Matrix3D) -> bool {
        let (ok, m) = self.tree.as_ref().unwrap().simple_evaluate_pivot(boneindex, obj_tm).unwrap();
        *tm = m;
        ok
    }
    fn update_sub_object_transforms(&mut self) {
        self.updates += 1;
        let t = self.transform;
        if let Some(tree) = self.tree.as_mut() {
            tree.base_update(&t).unwrap();
        }
    }
    fn htree_get_transform(&self, boneindex: i32) -> Matrix3D {
        self.tree.as_ref().unwrap().get_transform(boneindex).unwrap()
    }
}

fn dump_tree(t: &Option<HTree>) -> Vec<u32> {
    match t {
        None => vec![0],
        Some(t) => {
            let mut v = vec![1, t.num_pivots as u32];
            for p in &t.pivots {
                v.extend(p.base_transform.to_array().iter().map(|x| bits(*x)));
                v.extend(p.transform.to_array().iter().map(|x| bits(*x)));
            }
            v
        }
    }
}

fn tree_bytes(g: &mut Gen, name: &[u8], n: usize, parents: &[u32]) -> Vec<u8> {
    let pivots: Vec<PivotSpec> = (0..n)
        .map(|i| {
            let mut q = [g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0), g.uniform(-1.0, 1.0)];
            let l = q.iter().map(|x| x * x).sum::<f32>().max(1e-6);
            let s = 1.0 / petir::Real::sqrt(l);
            q.iter_mut().for_each(|x| *x *= s);
            PivotSpec { name: format!("B{i}").into_bytes(), parent: parents[i], t: [g.uniform(-5.0, 5.0), g.uniform(-5.0, 5.0), g.uniform(-5.0, 5.0)], e: [0.0; 3], q }
        })
        .collect();
    tree_chunk(name, &pivots)
}

fn load_tree(bytes: &[u8]) -> HTree {
    let mut c = ChunkLoad::new(GameFile::new(bytes).unwrap());
    assert!(c.open_chunk().unwrap());
    htree::load_w3d(&mut c).unwrap().unwrap()
}

#[test]
fn simple_evaluate_bone_and_set_htree_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0030);
    let mut chk = Check::new("Animatable3DObjClass::Simple_Evaluate_Bone / Set_HTree");
    let mut by_mode = [0u64; 5];
    let mut updates = 0u64;
    for case in 0..cases() {
        run(&[0]);
        let n = 1 + g.below(8) as usize;
        let parents: Vec<u32> = (0..n).map(|i| if i == 0 { 0xFFFF_FFFF } else { g.below(i as u32) }).collect();
        let tb = tree_bytes(&mut g, b"SK", n, &parents);
        let tb2 = tree_bytes(&mut g, b"SK2", n, &parents);
        let mut inp = vec![1];
        words_blob(&tb, &mut inp);
        assert_eq!(run(&inp), vec![0]);
        inp = vec![1];
        words_blob(&tb2, &mut inp);
        assert_eq!(run(&inp), vec![1]);
        let tree = load_tree(&tb);
        let tree2 = load_tree(&tb2);
        let frames = 3 + g.below(5);
        let mut chans = Vec::new();
        for p in 0..n as u16 {
            for f in 0..7u16 {
                if g.below(3) == 0 {
                    chans.push(RawChan { flags: f, pivot: p, frames: frames as u16 });
                }
            }
        }
        let mut v = 0.1f32;
        let ab = raw_anim_chunk(b"A", b"SK", frames, &chans, &[], || {
            v += 0.37;
            if v > 1.0 {
                v -= 1.9;
            }
            v
        });
        inp = vec![2];
        words_blob(&ab, &mut inp);
        assert_eq!(run(&inp), vec![0]);
        let mut c = ChunkLoad::new(GameFile::new(&ab).unwrap());
        assert!(c.open_chunk().unwrap());
        let lookup = |name: &[u8; 16]| -> Option<i32> { if name == &name16(b"SK") { Some(n as i32) } else { None } };
        let anim = HRawAnim::load_w3d(&mut c, &mut { lookup }).unwrap().unwrap();

        let mode = g.below(5);
        by_mode[mode as usize] += 1;
        let no_tree = mode < 3 && g.below(12) == 0;
        let has_anim = mode == 2 || g.below(2) == 0;
        let frame = g.uniform(0.0, (frames - 1) as f32);
        let mut tm0 = [0.0f32; 12];
        for x in tm0.iter_mut() {
            *x = g.uniform(-2.0, 2.0);
        }
        let mut inp = vec![3, if no_tree { 0xFFFF_FFFF } else { 0 }, mode, if has_anim { 0 } else { 0xFFFF_FFFF }, bits(frame)];
        inp.extend(tm0.iter().map(|x| bits(*x)));
        run(&inp);
        let mm = [MotionMode::None, MotionMode::BasePose, MotionMode::SingleAnim, MotionMode::DoubleAnim, MotionMode::MultipleAnim][mode as usize];
        let transform = mat(tm0);
        let mut state = Animatable3DState { htree: if no_tree { None } else { Some(tree.clone()) }, cur_motion_mode: mm, transform };
        let mut host = Host { tree: if no_tree { None } else { Some(tree.clone()) }, anim: has_anim.then(|| HAnim::Raw(anim.clone())), frame, mode: mm, transform, updates: 0 };
        for step in 0..1 + g.below(6) {
            let bone = if no_tree { 0 } else { g.below(n as u32) };
            let with_frame = g.below(2) == 0;
            let f2 = g.uniform(0.0, (frames - 1) as f32);
            let mut pre = [0.0f32; 12];
            for x in pre.iter_mut() {
                *x = g.uniform(-9.0, 9.0);
            }
            let mut tm = mat(pre);
            let ret = if with_frame { dao::simple_evaluate_bone_frame(&state, &host, bone as i32, f2, &mut tm) } else { dao::simple_evaluate_bone(&state, &mut host, bone as i32, &mut tm) };
            let mut r = vec![u32::from(ret)];
            r.extend(tm.to_array().iter().map(|x| bits(*x)));
            r.push(host.updates);
            let mut inp = vec![if with_frame { 5 } else { 4 }, bone];
            if with_frame {
                inp.push(bits(f2));
            }
            inp.extend(pre.iter().map(|x| bits(*x)));
            let c = run(&inp);
            updates += u64::from(host.updates > 0);
            chk.eq(r, c, || format!("case {case} step {step}: mode {mode} bone {bone} frame {with_frame} no_tree {no_tree}"));
        }
        dao::set_htree(&mut state, &tree2);
        let c = run(&[6, 1]);
        chk.eq(dump_tree(&state.htree), c, || format!("case {case}: Set_HTree"));
    }
    println!("{} cases; objects per mode {by_mode:?}; {updates} cases that updated the hierarchy", cases());
    finish(chk);
}
