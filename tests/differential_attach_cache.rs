#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp
//   (W3DModelDrawModuleData::getAttachToDrawableBoneOffset) and its header (CACHE_ATTACH_BONE)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp (Drawable::getTransformMatrix, Drawable::draw's root,
//   applyPhysicsXform, setInstanceMatrix) and its header (PhysicsXformInfo)
//   (GitHub #173) GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp (ParticleSystem::update's
//   attachment block), GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp (Thing::setTransformMatrix),
//   GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h (ObjectShroudStatus)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #179 items 4 and 5, and (last test) #173 item 4, against the upstream text compiled verbatim (`differential/wrap_attach_cache.cpp`, which
//! lists the cuts, the stand-ins and the words).
//!
//! # Item 4: the attach-offset cache's first filler
//!
//! **What upstream does.** `W3DModelDrawModuleData::getAttachToDrawableBoneOffset(draw)` (W3DModelDraw.cpp:1146-1174,
//! `CACHE_ATTACH_BONE` defined at W3DModelDraw.h:278) keeps one cache in the module data, which is the template's and
//! shared by all its drawables (process-wide, `mutable`). The first call by any drawable of the template fills it from
//! that drawable's `getPristineBonePositions(bone, 0, NULL, &mtx, 1)`, in the state its flags select at that moment;
//! every later call answers the cached offset. It is asked by `adjustTransformMtx` (from `doDrawModule`, 2098, and
//! `reactToTransformChange`, 3658) and by `getProjectileLaunchOffset` (3363, the turret positions, which the logic
//! reads: Weapon.cpp:2971, Object.cpp:6041). In practice the first asker is the template's first drawable at its first
//! transform, right after `Object::friend_bindToDrawable` set its weapon-set, `NIGHT` and `SNOW` flags
//! (Object.cpp:2975-3006).
//!
//! **The cache feeds the logic** (upstream's own comment: "must use pristine bone here since the result is used by
//! logic"). The port has two caches: the client's (filled when each drawable is made, with no flags,
//! `Drawable::validate_attach_to_drawable_bone_offsets`) and the logic's (#178, filled at load with no flags,
//! `drawable_bones::fill_attach_to_drawable_bone_offsets`). Following upstream's first filler would make the logic's
//! offsets depend on which drawable asked first and in which state, so **no behaviour is changed** here: it is the
//! owner's decision (#179 item 4). This test measures the difference.
//!
//! **Methodology.** Each case: an `AttachToBoneInAnotherModule` bone (9 in 10 cases), four states' attach bones (state 0
//! is the state for no flags; each other state a copy of state 0's 1 in 4, else its own, found 4 in 5), then 1 to 16
//! steps over four drawables of the template: a drawable made and bound in a state (half of them state 0), which asks
//! the cache at its first transform; a drawable's state changed; a drawable asking (a draw or a launch offset). The C++
//! answers each ask with upstream's cache. The Rust gives three streams:
//! - **A** the port's `W3DModelDrawModuleData::get_attach_to_drawable_bone_offset` asked on upstream's schedule (the
//!   translation of the function; gated at 0 mismatches);
//! - **B** the port's client: filled when each drawable is made, with no flags; each ask reads
//!   `attach_to_drawable_bone_offset_cached` after the port's fill before a draw;
//! - **C** the port's logic: filled before the first step from the no-flags state; each ask reads it.
//!
//! B and C are measured against upstream, not gated. Every mismatching ask is classified: upstream's first filler was
//! in a state other than state 0 whose offset differs from state 0's (asserted for every mismatch).
//!
//! # Item 5: a drawable with no object
//!
//! `Drawable::getTransformMatrix` (Drawable.cpp:4297-4310) answers the object's transform, or without an object the
//! drawable's own (`Thing::getTransformMatrix`). The port's `ClientDrawables::draw_facts` used the identity there.
//! Translated as `client::drawable::get_transform_matrix`, read by `draw_facts`, and the draw's root by
//! `ClientDrawables::root_draw_facts` (both of `ClientDrawables::draw`'s loops).
//!
//! **Methodology.** One gate World and its first object. Each case: the object bound (7 in 10) and its transform, then
//! 1 to 12 steps: bind or unbind; the object's transform set (`World::set_transform_matrix`, the C++ given the
//! transform the World holds) and pushed to the drawable as `Object::reactToTransformChange` does; the drawable's own
//! transform set; `setInstanceMatrix`; the next draws' physics totals and two gates (held, ShowClientPhysics); a draw,
//! recorded as the root (`root_draw_facts(..).transform`); a `getTransformMatrix` (`draw_facts(..).drawable_transform`).
//! Every word compared as bits.
//!
//! # Predictions (written before the first run, `issue179i45_predictions.md`)
//!
//! A: 20,000 cases, 0 mismatches; plant 9753 caught. B == C at every ask; B and C against upstream: mismatches only
//! where upstream's first filler was in another state with another offset, 30-45 % of cases. Item 5: 20,000 cases, 0
//! mismatches, max ULP 0; plants 10016 (30-50 % of cases), 10017 (50-70 %), 10018 (30-50 %) caught.
//!
//! # Results (2026-11-14)
//!
//! - **First run:** A and item 5 passed; the measurement test panicked on its own `expect`: without an attach bone
//!   nothing fills the cache (a test defect, fixed: such asks take state 0, every answer NULL).
//! - **A:** 20,000 cases, 75,047 asks (67,677 non-NULL), **0 mismatches**, max ULP 0. Plant 9753 (a found bone filled
//!   as zero): 12,317 cases.
//! - **B and C (measured):** 20,000 cases, 76,531 asks. Both differ from upstream in **4,122 cases** (18,434 asks), and
//!   agree with each other at every ask. Every mismatch is classified: upstream's first filler was in another state
//!   with another offset. Upstream's first filler was in another state in 5,757 cases, 1,635 of them with state 0's
//!   offset. Largest offset distance 50.9. Prediction missed: 30-45 % predicted, 20.6 % measured (a tenth of the cases
//!   have no bone, and the first asker is the first drawable made, half of them in state 0).
//! - **Item 5:** 20,000 cases, 130,553 steps (6,914 draws and 4,718 `getTransformMatrix` without an object), **0
//!   mismatches**, max ULP 0. Plants: 10016 8,285 cases (41 %, predicted 30-50 %); 10017 3,641 (18 %, predicted 50-70 %:
//!   missed, only a quarter of the steps are draws or reads without an object); 10018 1,595 (8 %, predicted 30-50 %:
//!   missed, it needs a draw without an object after a non-identity instance matrix). 3 of 3 caught.

mod support;

use rts_lockstep_engine::client::drawable::{ClientDrawables, Drawable};
use rts_lockstep_engine::client::drawable_loco::{apply_physics_xform_gated, PhysicsXformGates, PhysicsXformInfo};
use rts_lockstep_engine::client::w3d_model_draw::{DrawModuleExtra, W3DModelDrawModuleData};
use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_ID};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

/// The wrapper's records are one global buffer: one test at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases() -> u64 {
    std::env::var("ATTACH_CACHE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn b(f: f32) -> i32 {
    f.to_bits() as i32
}

fn r(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

fn matrix(w: &[i32]) -> Matrix3D {
    let mut m = Matrix3D::IDENTITY;
    for row in 0..3 {
        for c in 0..4 {
            m.row[row][c] = r(w[row * 4 + c]);
        }
    }
    m
}

fn words(m: &Matrix3D) -> [i32; 12] {
    let mut w = [0i32; 12];
    for row in 0..3 {
        for c in 0..4 {
            w[row * 4 + c] = b(m.row[row][c]);
        }
    }
    w
}

/// A transform: a turn about z at a place in `lo..hi` (x, y) and `0..zh` (z); 1 in 5 its 3x3 entries any in -2..2.
fn transform(g: &mut Gen, lo: f32, hi: f32, zh: f32) -> [i32; 12] {
    let mut m = Matrix3D::IDENTITY;
    m.rotate_z(g.uniform(-3.2, 3.2));
    if g.below(5) == 0 {
        for row in 0..3 {
            for c in 0..3 {
                m.row[row][c] = g.uniform(-2.0, 2.0);
            }
        }
    }
    m.set_translation(&Vector3 { x: g.uniform(lo, hi), y: g.uniform(lo, hi), z: g.uniform(0.0, zh) });
    words(&m)
}

fn run_cpp(f: unsafe extern "C" fn(*const i32, i32, *mut i32, i32) -> i32, input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 16];
    let n = unsafe { f(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Item 4
// ---------------------------------------------------------------------------------------------------------------

const CACHE_HEADER: usize = 1 + 4 * 13;

fn cache_case(g: &mut Gen) -> Vec<i32> {
    let mut w = vec![i32::from(g.below(10) != 0)];
    let mut state0 = [0i32; 13];
    for s in 0..4 {
        let st: [i32; 13] = if s > 0 && g.below(4) == 0 {
            state0
        } else {
            let mut st = [0i32; 13];
            st[0] = i32::from(g.below(5) != 0);
            st[1..].copy_from_slice(&transform(g, -20.0, 20.0, 20.0));
            st
        };
        if s == 0 {
            state0 = st;
        }
        w.extend(st);
    }
    for _ in 0..1 + g.below(16) {
        let op = match g.below(10) {
            0..=2 => 0,
            3..=4 => 1,
            _ => 2,
        };
        let state = if g.below(2) == 0 { 0 } else { g.below(4) as i32 };
        w.extend([op, g.below(4) as i32, state]);
    }
    w
}

fn offset_words(out: &mut Vec<i32>, o: Option<Vector3>) {
    match o {
        Some(v) => out.extend([700, 1, b(v.x), b(v.y), b(v.z)]),
        None => out.extend([700, 0, 0, 0, 0]),
    }
}

/// The three Rust streams of a case (A, B, C; see the module docs) and, per ask, upstream's first filler's state (A's).
struct CacheRun {
    a: Vec<i32>,
    b: Vec<i32>,
    c: Vec<i32>,
    filler: Vec<Option<usize>>,
    /// Each state's offset as a fill from it would be (`None` without the bone).
    state_offsets: [Option<Vector3>; 4],
}

fn rust_cache(w: &[i32]) -> CacheRun {
    let data = || {
        let mut d = W3DModelDrawModuleData::new(b"W3DModelDraw", DrawModuleExtra::None);
        if w[0] != 0 {
            d.attach_to_drawable_bone = b"attachbone".to_vec();
        }
        d
    };
    let mut bones = [None; 4];
    for (s, bone) in bones.iter_mut().enumerate() {
        let st = &w[1 + s * 13..1 + (s + 1) * 13];
        *bone = (st[0] != 0).then(|| matrix(&st[1..]));
    }
    let bone = |s: usize| bones[s];
    let (da, db, dc) = (data(), data(), data());
    let state_offsets = std::array::from_fn(|s| data().get_attach_to_drawable_bone_offset(|| bone(s)));
    // C: the logic's cache, filled at load from the no-flags state
    dc.get_attach_to_drawable_bone_offset(|| bone(0));
    let (mut a, mut bb, mut c) = (Vec::new(), Vec::new(), Vec::new());
    let mut filler = Vec::new();
    let mut first: Option<usize> = None;
    let mut made = [false; 4];
    let mut state = [0usize; 4];
    for st in w[CACHE_HEADER..].chunks_exact(3) {
        let j = st[1] as usize % 4;
        let s = st[2] as usize % 4;
        match st[0] {
            1 => {
                if made[j] {
                    state[j] = s;
                }
                continue;
            }
            0 => {
                // B: the port's client fills when the drawable is made, before its flags (no flags)
                db.get_attach_to_drawable_bone_offset(|| bone(0));
                made[j] = true;
                state[j] = s;
            }
            _ if !made[j] => continue,
            _ => {}
        }
        // A: upstream's schedule, the port's function
        let sj = state[j];
        let oa = da.get_attach_to_drawable_bone_offset(|| {
            first.get_or_insert(sj);
            bone(sj)
        });
        offset_words(&mut a, oa);
        filler.push(first);
        // B: the port's fill before a draw (already full), then adjustTransformMtx's read
        db.get_attach_to_drawable_bone_offset(|| bone(sj));
        offset_words(&mut bb, db.attach_to_drawable_bone_offset_cached());
        // C
        offset_words(&mut c, dc.get_attach_to_drawable_bone_offset(|| bone(sj)));
    }
    CacheRun { a, b: bb, c, filler, state_offsets }
}

#[test]
fn the_attach_offset_cache_matches_upstream_on_upstreams_schedule() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("attach cache (A: upstream's schedule)");
    let mut g = Gen::new(0x179_4001);
    let (mut asks, mut found) = (0u64, 0u64);
    for k in 0..cases() {
        let w = cache_case(&mut g);
        let run = rust_cache(&w);
        let cc = run_cpp(cpp::ref_attach_cache_seq, &w);
        asks += (run.a.len() / 5) as u64;
        found += run.a.chunks_exact(5).filter(|x| x[1] == 1).count() as u64;
        check.eq(run.a, cc, || format!("case {k}"));
    }
    println!("attach cache A: {} cases, {asks} asks, {found} non-NULL", cases());
    if planted() {
        check.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    check.finish();
}

/// Measured, not gated: the port's two schedules (B, the client's; C, the logic's) against upstream's.
#[test]
fn the_ports_first_fillers_differ_from_upstreams_only_in_another_state() {
    if planted() {
        return;
    }
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x179_4002);
    let n = cases();
    let (mut asks, mut cases_b, mut asks_b, mut cases_c, mut asks_c, mut b_ne_c) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut filler_other, mut filler_other_equal, mut max_dist) = (0u64, 0u64, 0f64);
    for k in 0..n {
        let w = cache_case(&mut g);
        let run = rust_cache(&w);
        let cc = run_cpp(cpp::ref_attach_cache_seq, &w);
        assert_eq!(run.a.len(), cc.len(), "case {k}: the ask counts");
        let mut case_b = false;
        let mut case_c = false;
        let mut counted_filler = false;
        for (i, u) in cc.chunks_exact(5).enumerate() {
            asks += 1;
            let pb = &run.b[i * 5..i * 5 + 5];
            let pc = &run.c[i * 5..i * 5 + 5];
            b_ne_c += u64::from(pb != pc);
            // (without an attach bone nothing is filled: every answer is NULL, state 0 taken)
            let f = run.filler[i].unwrap_or(0);
            if f != 0 && !counted_filler {
                counted_filler = true;
                filler_other += 1;
                if run.state_offsets[f] == run.state_offsets[0] {
                    filler_other_equal += 1;
                }
            }
            for (p, case_x, asks_x) in [(pb, &mut case_b, &mut asks_b), (pc, &mut case_c, &mut asks_c)] {
                if p != u {
                    *case_x = true;
                    *asks_x += 1;
                    // the classification: upstream's first filler was in another state, with another offset
                    assert!(f != 0 && run.state_offsets[f] != run.state_offsets[0], "case {k} ask {i}: a mismatch not explained by the first filler's state");
                    let d = (0..3).map(|x| f64::from(r(p[2 + x])) - f64::from(r(u[2 + x]))).map(|x| x * x).sum::<f64>();
                    max_dist = max_dist.max(petir::Real::sqrt(d));
                }
            }
        }
        cases_b += u64::from(case_b);
        cases_c += u64::from(case_c);
    }
    println!(
        "MEASURED attach cache: {n} cases, {asks} asks; B (client) differs in {cases_b} cases / {asks_b} asks; C (logic) in {cases_c} cases / {asks_c} asks; B != C at {b_ne_c} asks; upstream's first filler in another state in {filler_other} cases ({filler_other_equal} of them with state 0's offset); largest offset distance {max_dist}"
    );
    assert_eq!(b_ne_c, 0, "the port's two caches agree");
}

// ---------------------------------------------------------------------------------------------------------------
// Item 5
// ---------------------------------------------------------------------------------------------------------------

const ROOT_HEADER: usize = 13;
const STEP: usize = 14;

fn root_case(g: &mut Gen) -> Vec<i32> {
    let mut w = vec![i32::from(g.below(10) < 7)];
    w.extend(transform(g, 10.0, 300.0, 30.0));
    for _ in 0..1 + g.below(12) {
        let mut s = [0i32; STEP];
        s[0] = match g.below(100) {
            0..=9 => 0,
            10..=29 => 1,
            30..=49 => 2,
            50..=64 => 3,
            65..=74 => 4,
            75..=89 => 5,
            _ => 6,
        };
        match s[0] {
            0 => s[1] = i32::from(g.below(2) == 0),
            1 => s[2..].copy_from_slice(&transform(g, 10.0, 300.0, 30.0)),
            2 => s[2..].copy_from_slice(&transform(g, -500.0, 500.0, 100.0)),
            3 => {
                s[1] = i32::from(g.below(4) != 0);
                let mut m = Matrix3D::IDENTITY;
                m.rotate_x(g.uniform(-0.5, 0.5));
                m.rotate_y(g.uniform(-0.5, 0.5));
                m.set_translation(&Vector3 { x: g.uniform(-3.0, 3.0), y: g.uniform(-3.0, 3.0), z: g.uniform(-3.0, 3.0) });
                s[2..].copy_from_slice(&words(&m));
            }
            4 => {
                s[1] = i32::from(g.below(2) == 0);
                for v in &mut s[2..5] {
                    *v = b(g.uniform(-0.5, 0.5));
                }
                s[5] = b(g.uniform(-2.0, 2.0));
                s[6] = i32::from(g.below(5) == 0);
                s[7] = i32::from(g.below(5) != 0);
            }
            _ => {}
        }
        w.extend(s);
    }
    w
}

/// The Rust's records of a case; the object's transforms in `w` are replaced by the ones the World holds.
fn rust_root(world: &mut World, obj: ObjectId, w: &mut [i32]) -> Vec<i32> {
    let mut out = Vec::new();
    let mut bound = w[0] != 0;
    world.set_transform_matrix(obj, &matrix(&w[1..13]));
    let held = words(world.find_object(obj).expect("the object").thing.transform_matrix());
    w[1..13].copy_from_slice(&held);
    let mut d = Drawable::bare(INVALID_ID, b"Case".to_vec());
    let (mut physics, mut held_gate, mut show) = (None, false, true);
    for s in w[ROOT_HEADER..].chunks_exact_mut(STEP) {
        let id = if bound { obj } else { INVALID_ID };
        match s[0] {
            0 => bound = s[1] != 0,
            1 => {
                if bound {
                    world.set_transform_matrix(obj, &matrix(&s[2..]));
                    let t = *world.find_object(obj).expect("the object").thing.transform_matrix();
                    s[2..].copy_from_slice(&words(&t));
                    // `Object::reactToTransformChange`'s push (Object.cpp:1771-1774)
                    d.saved.transform = t;
                }
            }
            2 => d.saved.transform = matrix(&s[2..]),
            3 => {
                let m = matrix(&s[2..]);
                d.set_instance_matrix(if s[1] != 0 { Some(&m) } else { None });
            }
            4 => {
                physics = (s[1] != 0).then_some(PhysicsXformInfo { total_pitch: r(s[2]), total_roll: r(s[3]), total_yaw: r(s[4]), total_z: r(s[5]) });
                held_gate = s[6] != 0;
                show = s[7] != 0;
            }
            5 => {
                let gates = PhysicsXformGates {
                    has_object: bound,
                    held: held_gate,
                    show_client_physics: show,
                    view_time_frozen: false,
                    view_camera_movement_finished: true,
                    time_frozen_debug: false,
                    time_frozen_script: false,
                };
                d.physics_xform = apply_physics_xform_gated(&gates, || physics);
                let f = ClientDrawables::root_draw_facts(world, id, &d, show);
                assert_eq!(f.has_object, bound);
                out.push(400);
                out.extend(words(&f.transform));
            }
            _ => {
                let f = ClientDrawables::draw_facts(world, id, &d.saved.transform, true, None);
                out.push(600);
                out.extend(words(&f.drawable_transform));
            }
        }
    }
    out
}

#[test]
fn a_drawables_transform_and_root_match_upstream_with_an_object_or_none() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = rts_lockstep_engine::logic::world::gate_scenario::gate_world();
    let obj = world.objects.ids_in_order()[0];
    let mut check = Check::new("getTransformMatrix and the draw root");
    let mut g = Gen::new(0x179_5001);
    let (mut steps, mut draws_without, mut gets_without, mut max_ulp) = (0u64, 0u64, 0u64, 0u64);
    for k in 0..cases() {
        let mut w = root_case(&mut g);
        let rust = rust_root(&mut world, obj, &mut w);
        let cc = run_cpp(cpp::ref_drawable_transform_seq, &w);
        steps += ((w.len() - ROOT_HEADER) / STEP) as u64;
        let mut bound = w[0] != 0;
        for s in w[ROOT_HEADER..].chunks_exact(STEP) {
            match s[0] {
                0 => bound = s[1] != 0,
                5 => draws_without += u64::from(!bound),
                6 => gets_without += u64::from(!bound),
                _ => {}
            }
        }
        if rust.len() == cc.len() {
            for (x, y) in rust.iter().zip(&cc) {
                if x != y && r(*x).is_finite() && r(*y).is_finite() {
                    max_ulp = max_ulp.max(support::ulp_distance(r(*x), r(*y)));
                }
            }
        }
        check.eq(rust, cc, || format!("case {k}"));
    }
    println!("drawable transform: {} cases, {steps} steps, {draws_without} draws and {gets_without} getTransformMatrix without an object; max ULP among differing words {max_ulp}", cases());
    if planted() {
        check.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    check.finish();
}

// ---------------------------------------------------------------------------------------------------------------
// #173 item 4: a particle system attached to a drawable, with an object or none
// ---------------------------------------------------------------------------------------------------------------

const PS_HEADER: usize = 13;

fn psys_case(g: &mut Gen) -> Vec<i32> {
    let mut w = vec![i32::from(g.below(2) == 0)];
    w.extend(transform(g, -300.0, 300.0, 30.0));
    for _ in 0..1 + g.below(16) {
        let mut s = [0i32; STEP];
        s[0] = match g.below(100) {
            0..=19 => 0,
            20..=29 => 1,
            30..=44 => 2,
            45..=54 => 3,
            55..=57 => 4,
            _ => 5,
        };
        match s[0] {
            0 => s[2..].copy_from_slice(&transform(g, -300.0, 300.0, 30.0)),
            1 | 3 => s[1] = i32::from(g.below(2) == 0),
            2 => s[2..].copy_from_slice(&transform(g, -20.0, 20.0, 20.0)),
            _ => {}
        }
        w.extend(s);
    }
    w
}

/// The Rust's records of a case (the port's `ParticleSystemManager::update` reading the drawable through
/// `WorldFxHost`); the object's transforms in `w` are replaced by the ones the World holds.
fn rust_psys(world: &mut World, obj: ObjectId, w: &mut [i32]) -> Vec<i32> {
    use rts_lockstep_engine::client::client_fx::WorldFxHost;
    use rts_lockstep_engine::client::drawable::LOGIC_DRAWABLE_ID_BASE;
    use rts_lockstep_engine::client::particle_sys::{ParticleHost, ParticleSettings, ParticleSystemManager};
    use rts_lockstep_engine::common::random_value::RandomValues;
    use std::collections::BTreeMap;
    let mut out = Vec::new();
    let object_backed = w[0] != 0;
    let mut d = Drawable::bare(if object_backed { obj } else { INVALID_ID }, b"Case".to_vec());
    if !object_backed {
        // (a logic-made drawable: its own id, as `ClientDrawables::new_logic_drawable` numbers it)
        d.saved.drawable_id = LOGIC_DRAWABLE_ID_BASE + 7;
    }
    let set = |world: &mut World, d: &mut Drawable, m: &mut [i32]| {
        if object_backed {
            world.set_transform_matrix(obj, &matrix(m));
            let t = *world.find_object(obj).expect("the object").thing.transform_matrix();
            m.copy_from_slice(&words(&t));
            // `Object::reactToTransformChange`'s push (Object.cpp:1771-1774)
            d.saved.transform = t;
        } else {
            d.saved.transform = matrix(m);
        }
    };
    set(world, &mut d, &mut w[1..13]);
    let mut manager = ParticleSystemManager::new();
    manager.load_text(b"x.ini", b"ParticleSystem PS\n  BurstCount = 0 0\nEnd\n").expect("the template");
    let mut random = RandomValues::new(0);
    let settings = ParticleSettings::default();
    let id = manager.create_particle_system(b"PS", true, &mut random, 0).expect("made");
    {
        let s = manager.find_particle_system_mut(id).expect("made");
        // (no emission: only the attachment is compared)
        s.stop();
        s.attach_to_drawable(d.particle_drawable_id());
    }
    let mut present = true;
    let mut frame = 0u32;
    for s in w[PS_HEADER..].chunks_exact_mut(STEP) {
        match s[0] {
            0 => set(world, &mut d, &mut s[2..]),
            1 => d.set_fully_obscured_by_shroud(s[1] != 0, None),
            2 => {
                if let Some(p) = manager.find_particle_system_mut(id) {
                    p.set_local_transform(&matrix(&s[2..]));
                }
            }
            3 => {
                if let Some(p) = manager.find_particle_system_mut(id) {
                    p.skip_parent_xfrm = s[1] != 0;
                }
            }
            4 => present = false,
            _ => {
                let Some(attached) = manager.find_particle_system(id).map(|p| p.attached_to_drawable) else {
                    out.extend([800, 2]);
                    continue;
                };
                let mut drawables = BTreeMap::new();
                let mut logic = BTreeMap::new();
                if present {
                    if object_backed {
                        drawables.insert(obj, d.clone());
                    } else {
                        // (kept by the logic's id, which is not the drawable's own)
                        logic.insert(1000, d.clone());
                    }
                }
                let host = WorldFxHost { world, drawables: &drawables, logic_drawables: &logic, local_player_index: None, sync_time: 0 };
                let shrouded = i32::from(attached != 0 && host.drawable(attached).is_some_and(|a| a.shrouded));
                frame += 1;
                manager.update(&host, &mut random, &settings, frame, frame);
                match manager.find_particle_system(id) {
                    None => out.extend([800, 1, shrouded]),
                    Some(p) => {
                        out.extend([800, 0, shrouded, i32::from(p.is_identity)]);
                        out.extend([b(p.pos.x), b(p.pos.y), b(p.pos.z), b(p.last_pos.x), b(p.last_pos.y), b(p.last_pos.z)]);
                        out.extend(words(&p.transform));
                    }
                }
            }
        }
    }
    out
}

/// GitHub #173 item 4: a particle system attached to a drawable reads its shroud, transform and position as
/// `ParticleSystem::update` reads them (ParticleSys.cpp:1874-1970, with `Drawable::getTransformMatrix` and
/// `Thing::setTransformMatrix`, compiled verbatim: `differential/wrap_attach_cache.cpp`, entry point 3), for an
/// object's drawable and for one of the logic's object-less drawables (`WorldFxHost::drawable`).
///
/// **Methodology.** One gate World and its first object. Each case: the drawable an object's (half) or an object-less
/// one of the logic's (kept by the logic's id 1000, its own id `LOGIC_DRAWABLE_ID_BASE + 7`), its first transform, a
/// stopped system of a template that emits nothing, attached with `Drawable::particle_drawable_id` (C++:
/// `attachToDrawable(draw)`); then 1 to 16 steps: the transform set (an object's through `World::set_transform_matrix`
/// and pushed, as `Object::reactToTransformChange` does; an object-less drawable's own) 20 %, the drawable's shroud
/// 10 %, the system's `setLocalTransform` 15 %, `setSkipParentXfrm` 10 %, the drawable destroyed 3 %, an update 42 %
/// (the port's whole `ParticleSystemManager::update`; the C++ the cut block): recorded the shroud, whether the system
/// was destroyed, `m_isIdentity`, `m_pos`, `m_lastPos` and `m_transform`. Every word compared as bits.
///
/// **Predictions** (`issue173_predictions.md`, written before the first run): 0 mismatches over 20,000 cases; plants
/// 10197-10202 caught (10200 in ~15 % of cases, the others ~40-45 %); 10196 not reached here.
///
/// **Results (2026-11-14, first run):** 20,000 cases, 61,379 updates of a live system (30,531 on an object-less
/// drawable, 10,562 shrouded), 3,547 systems destroyed with their drawable: **0 mismatches**, max ULP 0. Plants
/// (mismatching cases of 20,000): 10197 8,554; 10198 8,554; 10199 8,554; 10200 2,015; 10201 9,112; 10202 8,554; 10196
/// 0 here (not reached), caught by the lib test `a_logic_drawable_s_bone_particle_systems_follow_it`. 7 of 7 caught.
/// Prediction misses: 10200 (predicted ~15 %, measured 10 %); the others were 43-46 %, as predicted.
#[test]
fn a_particle_system_follows_its_drawable_with_an_object_or_none() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = rts_lockstep_engine::logic::world::gate_scenario::gate_world();
    let obj = world.objects.ids_in_order()[0];
    let mut check = Check::new("a particle system's attached drawable");
    let mut g = Gen::new(0x173_4001);
    let (mut updates, mut object_less_updates, mut destroyed, mut shrouded, mut max_ulp) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for k in 0..cases() {
        let mut w = psys_case(&mut g);
        let rust = rust_psys(&mut world, obj, &mut w);
        let cc = run_cpp(cpp::ref_psys_attach_seq, &w);
        {
            let mut i = 0;
            while i + 1 < cc.len() {
                match cc[i + 1] {
                    0 => {
                        updates += 1;
                        object_less_updates += u64::from(w[0] == 0);
                        shrouded += u64::from(cc[i + 2] != 0);
                        i += 22;
                    }
                    1 => {
                        destroyed += 1;
                        i += 3;
                    }
                    _ => i += 2,
                }
            }
        }
        if rust.len() == cc.len() {
            for (x, y) in rust.iter().zip(&cc) {
                if x != y && r(*x).is_finite() && r(*y).is_finite() {
                    max_ulp = max_ulp.max(support::ulp_distance(r(*x), r(*y)));
                }
            }
        }
        check.eq(rust, cc, || format!("case {k}"));
    }
    println!("particle attachment: {} cases, {updates} updates of a live system ({object_less_updates} on an object-less drawable, {shrouded} shrouded), {destroyed} destroyed with their drawable; max ULP among differing words {max_ulp}", cases());
    if planted() {
        check.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    check.finish();
}
