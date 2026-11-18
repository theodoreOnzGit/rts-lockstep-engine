// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215) of the morph animation authoring drafts (`src/drafts/sweep_ww3d/hmorphanim.rs`):
//! `HMorphAnimClass::Create_New_Morph`, `Free_Morph`, `Resolve_Pivot_Channels`, `Insert_Morph_Key` and `Release_Keys`
//! (with `TimeCodedMorphKeysClass::Add_Key`) against upstream's text, cut verbatim by build.rs (`drafts_anim`,
//! differential/wrap_draft_morph.cpp) on the already-linked morph class.
//!
//! # Method
//!
//! Sequences (default 20,000, fixed seed) of 1 to 14 operations on one morph: create (1 to 3 channels, each pose a stand-in
//! with a frame rate, 1 to 8 pivots and a random node-motion mask; sometimes `anim == NULL`), free, insert a key (frames
//! from 0 to just under 2^31, including values above 2^24 that `float` cannot hold), release the keys. After every operation
//! the whole state is dumped and compared as words: counts, frame rate bits, the pivot channels, each pose's name and each
//! channel's keys and cached index. Inserts and releases are only issued while the arrays exist (upstream dereferences NULL
//! otherwise; the draft would not): after a successful create, not after a free or the `NULL` create.
//!
//! Excluded (upstream undefined): `Insert_Morph_Key` with `morph_frame` >= 2147483520, whose `float` is out of `int` range
//! (`WWMath::Max(uint32, int)` converts both to `float`, the result back to `int`).
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! A MISS expected in `Insert_Morph_Key`: upstream's `WWMath::Max` has only a `float` overload, so `FrameCount` is the `float`
//! maximum truncated to `int`: for a `morph_frame` above 2^24 that `float` rounds, and the draft's integer `max` does not.
//! Everything else 0 mismatches.

mod draft_anim_bytes;
mod support;

use draft_anim_bytes::*;
use rts_lockstep_engine::drafts::sweep_ww3d::hmorphanim::{self as dm, MorphPose};
use rts_lockstep_engine::formats::w3d::hmorphanim::HMorphAnim;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dmo_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 20_000];
    let n = unsafe { ref_dmo_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

struct Pose {
    rate: f32,
    present: Vec<bool>,
    name: Vec<u8>,
}

impl MorphPose for Pose {
    fn get_frame_rate(&self) -> f32 {
        self.rate
    }
    fn get_num_pivots(&self) -> i32 {
        self.present.len() as i32
    }
    fn is_node_motion_present(&self, pivot: i32) -> bool {
        pivot >= 0 && (pivot as usize) < self.present.len() && self.present[pivot as usize]
    }
    fn get_name(&self) -> Vec<u8> {
        self.name.clone()
    }
}

fn fresh() -> HMorphAnim {
    HMorphAnim {
        name: [0; 32],
        anim_name: [0; 16],
        hierarchy_name: [0; 16],
        frame_count: 0,
        frame_rate: 0.0,
        channel_count: 0,
        num_nodes: 0,
        channels: Vec::new(),
        pivot_channel: Vec::new(),
    }
}

fn dump(m: &HMorphAnim) -> Vec<u32> {
    let has_pose = !m.channels.is_empty();
    let mut v = vec![m.channel_count as u32, m.frame_count as u32, m.frame_rate.to_bits(), m.num_nodes as u32];
    v.extend([u32::from(has_pose), u32::from(has_pose), u32::from(!m.pivot_channel.is_empty())]);
    v.extend(m.pivot_channel.iter().copied());
    for c in 0..m.channel_count as usize {
        if let Some(ch) = m.channels.get(c) {
            let n = ch.pose_name.clone().unwrap_or_default();
            v.push(n.len() as u32);
            v.extend(n.iter().map(|&b| u32::from(b)));
            v.push(ch.keys.keys.len() as u32);
            for k in &ch.keys.keys {
                v.extend([k.morph_frame, k.pose_frame]);
            }
            v.push(*ch.keys.cached_idx.lock());
        }
    }
    v
}

fn frame(g: &mut Gen) -> u32 {
    match g.below(6) {
        0 => g.below(8),
        1 => g.below(60),
        2 => 16_777_216 + g.below(40),
        3 => g.below(2_147_483_000),
        4 => 33_554_432 + g.below(100),
        _ => g.below(1 << 20),
    }
}

#[test]
fn morph_authoring_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0010);
    let mut chk = Check::new("HMorphAnimClass authoring state");
    let (mut creates, mut inserts, mut big) = (0u64, 0u64, 0u64);
    for case in 0..cases() {
        run(&[0]);
        let mut m = fresh();
        let mut armed = false;
        for step in 0..1 + g.below(14) {
            let op = if !armed { [0, 0, 0, 0, 1][g.below(5) as usize] } else { [0, 1, 2, 2, 2, 2, 2, 3, 3][g.below(9) as usize] };
            let mut tag = String::new();
            match op {
                0 => {
                    creates += 1;
                    let channels = 1 + g.below(3) as i32;
                    let null = g.below(10) == 0;
                    let mut inp = vec![1, channels as u32, u32::from(null)];
                    let ret;
                    if null {
                        ret = dm::create_new_morph::<Pose>(&mut m, channels, None);
                        armed = false;
                    } else {
                        let poses: Vec<Pose> = (0..channels)
                            .map(|c| {
                                let n = 1 + g.below(8) as usize;
                                Pose {
                                    rate: if g.below(8) == 0 { 0.0 } else { g.uniform(1.0, 60.0) },
                                    present: (0..n).map(|_| g.below(2) == 0).collect(),
                                    name: format!("P{c}{}", g.below(100)).into_bytes(),
                                }
                            })
                            .collect();
                        for p in &poses {
                            inp.extend([p.rate.to_bits(), p.present.len() as u32, p.name.len() as u32]);
                            inp.extend(p.name.iter().map(|&b| u32::from(b)));
                            inp.extend(p.present.iter().map(|&b| u32::from(b)));
                        }
                        let refs: Vec<&Pose> = poses.iter().collect();
                        ret = dm::create_new_morph(&mut m, channels, Some(&refs));
                        armed = true;
                    }
                    let c = run(&inp);
                    chk.eq(vec![ret as u32], c, || format!("case {case} step {step}: create return"));
                    tag = format!("create {channels} null {null}");
                }
                1 => {
                    dm::free_morph(&mut m);
                    run(&[2]);
                    armed = false;
                    tag = "free".into();
                }
                2 => {
                    inserts += 1;
                    let ch = g.below(m.channel_count as u32);
                    let (mf, pf) = (frame(&mut g), g.u32());
                    big += u64::from(mf > 16_777_216);
                    dm::insert_morph_key(&mut m, ch as i32, mf, pf);
                    run(&[3, ch, mf, pf]);
                    tag = format!("insert ch {ch} morph {mf} pose {pf}");
                }
                _ => {
                    dm::release_keys(&mut m);
                    run(&[4]);
                    tag = "release".into();
                }
            }
            let c = run(&[5]);
            chk.eq(dump(&m), c, || format!("case {case} step {step} after {tag}"));
        }
    }
    println!("{} sequences: {creates} creates, {inserts} inserts ({big} with a frame above 2^24)", cases());
    finish(chk);
}
