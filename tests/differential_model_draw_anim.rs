#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp (doDrawModule,
//   setPauseAnimation, getCurrentAnimFraction, adjustAnimation, setCurAnimDurationInMsec, getCurAnimDistanceCovered,
//   adjustAnimSpeedToMovementSpeed, isAnimationComplete, handleClientRecoil, the bone particle systems, and setModelState's
//   head and tail) over GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/animobj.cpp (Animatable3DObjClass), htree.cpp and
//   hrawanim.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #181 item 7: `W3DModelDraw`'s draw sequence with an **animated** render object against the upstream text
//! compiled verbatim (`differential/wrap_model_draw_anim.cpp`, which lists the cuts, the stand-ins and the words).
//! `tests/differential_model_draw_particles.rs` covers the bone particle systems over models without animation, and
//! `tests/differential_w3d.rs` (ops 40-50) `Animatable3DObjClass` alone; this one covers their sequence.
//!
//! # What upstream does at one sync time
//!
//! `doDrawModule` (W3DModelDraw.cpp:2040-2112): `setPauseAnimation` (a pause re-sets the animation in `MANUAL`, keeping
//! the mode to resume in), the instance scale, then for a completed animation (`isAnimationComplete`: an HLOD's
//! `Is_Animation_Complete`) `adjustAnimation` again for an idle animation or `RESTART_ANIM_WHEN_COMPLETE` (client random
//! values: which animation, a random start frame, the speed factor), `adjustAnimSpeedToMovementSpeed`, the render
//! object's `Set_Transform` (the hierarchy no longer valid), then `recalcBonesForClientParticleSystems`: its
//! `Get_Bone_Transform` under `Matrix3D(true).Scale(getScale())` runs `Update_Sub_Object_Transforms`, whose
//! `Single_Anim_Progress` **moves the animation on** to the sync time before the pose is evaluated, and its restore
//! invalidates the hierarchy again; then (with `ParticlesAttachedToAnimatedBones`) `updateBonesForClientParticleSystems`,
//! whose first `Get_Bone_Transform` progresses again (by nothing at the same sync time) and evaluates the pose under the
//! render object's transform, the next ones reading it; then `handleClientRecoil`. The scene's `Render` later moves the
//! animation on again and evaluates the pose when the hierarchy is not valid.
//!
//! # Methodology
//!
//! Each case is a W3D file (one hierarchy `SKEL` of 2-6 pivots `B0`.., each under a random earlier pivot, with a random
//! translation in -2..2 and a unit rotation; then 1-4 raw animations `SKEL.A0`.. of 1-40 frames at 30 fps (15 or 60 one
//! in 8 each), with X, Y, Z and quaternion channels on random pivots and frame ranges), loaded by the REAL loaders on
//! the C++ side and by the port's on the Rust side; a drawable of a random id and scale (1, 0.5, 2, 1.005, 0.99 or any
//! in 0.5..2), with `ParticlesAttachedToAnimatedBones` (70 %) and `AnimationsRequirePower` (50 %); four particle system
//! templates with random slaves; 1-4 states, each a model (`M0`..`M2`, all on `SKEL`, or `None` 1 in 8), an
//! `AnimationMode` (any of the six), `Flags` (each of `RANDOMSTART`, `START_FRAME_FIRST`, `START_FRAME_LAST`,
//! `PRISTINE_BONE_POS_IN_FINAL_FRAME`, the four `MAINTAIN_FRAME_ACROSS_STATES*` and `RESTART_ANIM_WHEN_COMPLETE` 1 in 6),
//! an `AnimationSpeedFactorRange` (from 0.5 .. 2), 0-3 `Animation`/`IdleAnimation` lines (an animation of the file, or
//! one that is not there 1 in 12; a distance covered 0, 5, 12.5 or 30; a state's lines all idle 1 in 3, its mode then `ONCE` or
//! `ONCE_BACKWARDS`, as `parseConditionState` requires) and 0-3 `ParticleSysBone`s. The
//! sync time starts anywhere in 0..100,000 (1 in 20 near `u32::MAX`, to wrap). Then 1-60 steps: `setModelState` 8 %,
//! `doDrawModule` under a random world transform 25 %, the sync time moved on 20 % (0, 33, < 100, 100-3,100 or < 100,000
//! ms), the scene's `Render` 15 %, the drawable hidden or shown 3 %, `getShouldAnimate` 4 %, the object and its speed
//! 4 %, a bone captured and controlled (or released) 5 %, `reactToTransformChange` + `updateBonesForClientParticleSystems`
//! 4 %, a particle system dropped 2 %, the instance scale 2 %, the shroud 3 %, a `Get_Bone_Transform` query 5 %
//! (standing for another reader, as `handleWeaponFireFX`'s FX bone). Compared, every word as bits, after the
//! constructor and after each step: the module (`m_needRecalcBoneParticleSystems`, the shroud flag,
//! `m_whichAnimInCurState`, `m_pauseAnimation`, `m_animationMode`, the current state), the trackers, every particle
//! system (flags, slave, master, the local transform: where it is placed), the render object (motion mode, hierarchy
//! valid, the animation, `Frame`, `PrevFrame`, `LastSyncTime`, direction, speed multiplier, mode, complete, hidden,
//! transform, object scale, and **every pivot's transform**), and the client random state (6 words).
//!
//! Both sides run outside a logic time, as the port's client draws (`isValidTimeToCalcLogicStuff()` false): the pristine
//! bones, turrets and barrels are never valid, so no barrel fires and the recoil does nothing (gaps).
//!
//! # Predictions (`issue181i7_predictions.md`, before any run)
//!
//! 0 mismatches over 20,000 cases, max ULP 0, client random state equal everywhere, 0 NaN words; plants 9956-9975 all
//! caught here, each 200 - 15,000 of 20,000. (The NaN count was not kept: most words are integers, so it measured
//! nothing.)
//!
//! # Results (2026-11-13)
//!
//! - **First build:** 3 errors, the wrapper's (`DEBUG_ASSERTCRASH` undefined in the W3D setting; an ambiguous
//!   `RenderInfoClass`). Then the generator broke `parseConditionState`'s rules (idle and non-idle animations in one state;
//!   idle animations outside `ONCE`/`ONCE_BACKWARDS`): fixed in the generator.
//! - **First run** (300 cases): 232 differ, every one first in the client random state: the wrapper's particle system
//!   manager drew none of `ParticleSystem`'s constructor's three wind angles (ParticleSys.cpp:1142-1148), which the port
//!   draws. The wrapper's error: those lines are cut verbatim now.
//! - **Second run** (300): 59 differ: upstream makes the render object again on every state change outside a logic time
//!   (`turretNamesDiffer` is true while no state is `TURRETS_VALID`), as the port does; the wrapper's stand-in kept it
//!   when the model was the same. The wrapper's error: `turretNamesDiffer` is cut now.
//! - **Third run** (20,000): 88 differ, each a `setModelState` from a state with a maintain-frame flag while the render
//!   object played no animation: upstream's `getCurrentAnimFraction` reads its uninitialised `framenum` and
//!   `numFrames` there (`Peek_Animation_And_Info` answers NULL and is not tested). **Undefined upstream**, not a port
//!   defect (the port answers -1); both sides now end the case there ([700]). **A prediction miss** (I predicted 0).
//! - **Fourth run (gated):** 20,000 cases (1,704 ended where upstream is undefined), 573,806 steps, 143,926 draws (99,680
//!   with an animation playing), 55,365 particle systems made, 220,148 client random values drawn: **0 mismatches**, max
//!   ULP 0, the client random state equal after every step.
//! - **Plants**, mismatching cases of 20,000: 9956 13,839; 9957 13,001; 9958 3,598; 9959 4,145; 9960 472; 9961 2,533;
//!   9962 642; 9963 1,243; 9964 2,816; 9965 1,064; 9966 5,373; 9967 4,439; 9968 4,439; 9969 2,069; 9970 9,763; 9971
//!   6,445; 9972 8,988; 9973 13,304; 9974 268; 9975 1,493. **20 of 20 caught.**
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9956 | `adjustAnimation` does not draw the speed factor (the multiplier stays 1) |
//! | 9957 | `adjustAnimation` never draws which animation (always the first) |
//! | 9958 | `adjustAnimation` ignores `RANDOMSTART` |
//! | 9959 | `adjustAnimation` starts the backwards modes at frame 0 |
//! | 9960 | `adjustAnimation`'s maintained frame is `fraction * frames` (no `- 1`) |
//! | 9961 | `setPauseAnimation` does not keep the mode to resume in |
//! | 9962 | `setPauseAnimation` resumes in `LOOP` |
//! | 9963 | `nukeCurrentRender` does not clear `m_pauseAnimation` |
//! | 9964 | `doDrawModule` skips the completed-animation branch |
//! | 9965 | the idle switch passes no previous state (the same idle animation may come again) |
//! | 9966 | `doDrawModule` moves the bone particle systems before making them |
//! | 9967 | `adjustAnimSpeedToMovementSpeed` leaves out `MSEC_PER_LOGICFRAME_REAL` |
//! | 9968 | `setCurAnimDurationInMsec`'s multiplier inverted |
//! | 9969 | `Render` moves a `MANUAL` animation on |
//! | 9970 | `Single_Anim_Progress` does not keep the sync time (`LastSyncTime`) |
//! | 9971 | `updateBonesForClientParticleSystems` reads the pose without writing (no progress, no validation) |
//! | 9972 | `Set_Animation`'s `PrevFrame` is the new frame |
//! | 9973 | `adjustAnimation` draws the speed factor between max and min |
//! | 9974 | `getCurrentAnimFraction` divides by the frame count (no `- 1`) |
//! | 9975 | the completed branch restarts a non-idle animation without `RESTART_ANIM_WHEN_COMPLETE` |

mod support;

use std::sync::Arc;

use rts_lockstep_engine::client::particle_sys::ParticleSystemManager;
use rts_lockstep_engine::client::w3d_model_draw::{
    anim_mode_index, is_any_maintain_frame_flag_set, scale_render_obj_hierarchy, AnimData, AnimHandle, DrawEnv, DrawFacts, DrawParticles, Skeleton, StateRef, W3DModelDraw,
    W3DModelDrawModuleData, W3dAssetSource, THE_ANIM_MODE_NAMES,
};
use rts_lockstep_engine::common::model_state::{ModelConditionFlags, MODEL_CONDITION_NAMES};
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::thing_template::{ClientLine, ClientModuleData};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::assetmgr::HAnim;
use rts_lockstep_engine::formats::w3d::hanim::HRawAnim;
use rts_lockstep_engine::formats::w3d::htree::{self, HTree};
use rts_lockstep_engine::formats::w3d::render::get_frame_rate;
use rts_lockstep_engine::formats::w3d::w3d_file::*;
use rts_lockstep_engine::ww::chunkio::{ChunkLoad, GameFile};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use support::{Check, Gen};

/// The wrapper's records are one global buffer: one test at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases() -> u64 {
    std::env::var("MODEL_DRAW_ANIM_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn b(f: f32) -> i32 {
    f.to_bits() as i32
}

fn r(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

/// The wrapper's `BONE_NAMES` (the `ParticleSysBone` lines').
const BONE_NAMES: [&str; 7] = ["B1", "b2", "B3", "b4", "B5", "RootTransform", "nosuchbone"];
const MODEL_NAMES: [&str; 4] = ["M0", "M1", "M2", "None"];
const TEMPLATE_NAMES: [&str; 6] = ["PS0", "PS1", "PS2", "PS3", "None", "NoSuch"];
/// The states' condition bits (the first state is the default).
const STATE_BITS: [&str; 4] = ["", "DAMAGED", "RUBBLE", "GARRISONED"];
/// `ACBits` the cases set (not `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT`, 3: no object is under construction here).
const FLAG_BITS: [u32; 9] = [0, 1, 2, 4, 5, 6, 7, 8, 9];
/// `ACBitsNames`.
const AC_BITS_NAMES: [&str; 10] = [
    "RANDOMSTART",
    "START_FRAME_FIRST",
    "START_FRAME_LAST",
    "ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT",
    "PRISTINE_BONE_POS_IN_FINAL_FRAME",
    "MAINTAIN_FRAME_ACROSS_STATES",
    "RESTART_ANIM_WHEN_COMPLETE",
    "MAINTAIN_FRAME_ACROSS_STATES2",
    "MAINTAIN_FRAME_ACROSS_STATES3",
    "MAINTAIN_FRAME_ACROSS_STATES4",
];
/// Speed factors and distances that print and parse back exactly.
const SPEEDS: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
const DISTANCES: [f32; 4] = [0.0, 5.0, 12.5, 30.0];
/// The wrapper's index for an animation that is not in the file.
const MISSING_ANIM: i32 = 99;

const HEADER: usize = 6 + 4 + 1 + 4 * (5 + 1 + 3 * 3 + 1 + 3 * 2);
const STEP: usize = 14;

// ---- the W3D file ----

fn w32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn wf(v: &mut Vec<u8>, x: f32) {
    w32(v, x.to_bits());
}

fn hchunk(id: u32, body: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(body.len() + 8);
    w32(&mut v, id);
    w32(&mut v, body.len() as u32);
    v.extend_from_slice(body);
    v
}

fn name16(s: &[u8]) -> [u8; W3D_NAME_LEN] {
    let mut n = [0u8; W3D_NAME_LEN];
    n[..s.len()].copy_from_slice(s);
    n
}

/// A unit quaternion (normalised in `f32`).
fn quat(g: &mut Gen) -> [f32; 4] {
    let mut q = [0.0f32; 4];
    for v in q.iter_mut() {
        *v = g.uniform(-1.0, 1.0);
    }
    let l = petir::Real::sqrt(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]);
    if l < 1e-3 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    q.map(|v| v / l)
}

/// The file and its animations' frame counts.
fn gen_file(g: &mut Gen) -> (Vec<u8>, Vec<u32>) {
    let npiv = 2 + g.below(5);
    let mut f = Vec::new();
    // W3D_CHUNK_HIERARCHY: the header (version 4.1, name, pivot count, center) and the pivots
    let mut h = Vec::new();
    w32(&mut h, (4 << 16) | 1);
    h.extend_from_slice(&name16(b"SKEL"));
    w32(&mut h, npiv);
    for _ in 0..3 {
        wf(&mut h, 0.0);
    }
    let mut body = hchunk(W3D_CHUNK_HIERARCHY_HEADER, &h);
    let mut pb = Vec::new();
    for i in 0..npiv {
        pb.extend_from_slice(&name16(format!("B{i}").as_bytes()));
        w32(&mut pb, if i == 0 { 0xFFFF_FFFF } else { g.below(i) });
        for _ in 0..3 {
            wf(&mut pb, g.uniform(-2.0, 2.0));
        }
        for _ in 0..3 {
            wf(&mut pb, 0.0);
        }
        for v in quat(g) {
            wf(&mut pb, v);
        }
    }
    body.extend(hchunk(W3D_CHUNK_PIVOTS, &pb));
    f.extend(hchunk(W3D_CHUNK_HIERARCHY, &body));
    let mut frames = Vec::new();
    for a in 0..1 + g.below(4) {
        let nf = if g.below(10) == 0 { 1 } else { 2 + g.below(39) };
        frames.push(nf);
        let mut h = Vec::new();
        w32(&mut h, (4 << 16) | 1);
        h.extend_from_slice(&name16(format!("A{a}").as_bytes()));
        h.extend_from_slice(&name16(b"SKEL"));
        w32(&mut h, nf);
        w32(&mut h, match g.below(8) {
            0 => 15,
            1 => 60,
            _ => 30,
        });
        let mut body = hchunk(W3D_CHUNK_ANIMATION_HEADER, &h);
        for pivot in 0..npiv {
            for ty in [ANIM_CHANNEL_X, ANIM_CHANNEL_Y, ANIM_CHANNEL_Z, ANIM_CHANNEL_Q] {
                if g.below(2) != 0 {
                    continue;
                }
                let first = g.below(3).min(nf - 1) as u16;
                let last = first + g.below(nf - u32::from(first)) as u16;
                let vlen: u16 = if ty == ANIM_CHANNEL_Q { 4 } else { 1 };
                let mut c = Vec::new();
                for v in [first, last, vlen, ty as u16, pivot as u16, 0] {
                    c.extend_from_slice(&v.to_le_bytes());
                }
                for _ in first..=last {
                    if ty == ANIM_CHANNEL_Q {
                        for v in quat(g) {
                            wf(&mut c, v);
                        }
                    } else {
                        wf(&mut c, g.uniform(-3.0, 3.0));
                    }
                }
                body.extend(hchunk(W3D_CHUNK_ANIMATION_CHANNEL, &c));
            }
        }
        f.extend(hchunk(W3D_CHUNK_ANIMATION, &body));
    }
    (f, frames)
}

/// The port's load of the file: the hierarchy and the raw animations (the port's default `Float_To_Long`, Win32's).
fn rust_load(data: &[u8]) -> (HTree, Vec<HAnim>) {
    let mut c = ChunkLoad::new(GameFile::new(data).expect("a file"));
    let mut tree: Option<HTree> = None;
    let mut anims = Vec::new();
    while c.open_chunk().expect("a chunk") {
        match c.cur_chunk_id().expect("an id") {
            W3D_CHUNK_HIERARCHY => tree = htree::load_w3d(&mut c).expect("loads"),
            W3D_CHUNK_ANIMATION => {
                let t = tree.as_ref().map(|t| (t.name, t.num_pivots));
                let mut lookup = |n: &[u8; W3D_NAME_LEN]| t.filter(|(tn, _)| tn == n).map(|(_, p)| p);
                anims.push(HAnim::Raw(HRawAnim::load_w3d(&mut c, &mut lookup).expect("loads").expect("an animation")));
            }
            _ => panic!("only hierarchies and raw animations are made"),
        }
        c.close_chunk().expect("closes");
    }
    (tree.expect("a hierarchy"), anims)
}

// ---- the case ----

fn matrix(w: &[i32]) -> Matrix3D {
    let mut m = Matrix3D::IDENTITY;
    for row in 0..3 {
        for c in 0..4 {
            m.row[row][c] = r(w[row * 4 + c]);
        }
    }
    m
}

fn matrix_words(m: &Matrix3D) -> [i32; 12] {
    let mut w = [0i32; 12];
    for row in 0..3 {
        for c in 0..4 {
            w[row * 4 + c] = b(m.row[row][c]);
        }
    }
    w
}

/// A world transform: a turn about Z and a translation, or (1 in 4) any entries in -2..2 with a translation.
fn world(g: &mut Gen) -> [i32; 12] {
    let mut m = Matrix3D::IDENTITY;
    if g.below(4) == 0 {
        for row in 0..3 {
            for c in 0..3 {
                m.row[row][c] = g.uniform(-2.0, 2.0);
            }
        }
    } else {
        m.rotate_z(g.uniform(-4.0, 4.0));
    }
    m.row[0][3] = g.uniform(-500.0, 500.0);
    m.row[1][3] = g.uniform(-500.0, 500.0);
    m.row[2][3] = g.uniform(-20.0, 20.0);
    matrix_words(&m)
}

/// One case: the file and the wrapper's words.
fn random_case(g: &mut Gen) -> (Vec<u8>, Vec<i32>) {
    let (file, frames) = gen_file(g);
    let nanims = frames.len() as u32;
    let mut w = Vec::new();
    w.push(g.u32() as i32);
    w.push(if g.below(20) == 0 { u32::MAX - g.below(5000) } else { g.below(100_000) } as i32);
    w.push(1 + g.below(1000) as i32);
    w.push(b(match g.below(8) {
        0..=2 => 1.0,
        3 => 0.5,
        4 => 2.0,
        5 => [1.005f32, 0.99][g.below(2) as usize],
        _ => g.uniform(0.5, 2.0),
    }));
    w.push(i32::from(g.below(10) < 7));
    w.push(i32::from(g.below(2) == 0));
    for t in 0..4u32 {
        w.push(if t < 3 && g.below(3) == 0 { (t + 1 + g.below(3 - t)) as i32 } else { -1 });
    }
    w.push(1 + g.below(4) as i32);
    // parseConditionState's rules (W3DModelDraw.cpp, and the port's parse): a state's animations are all idle or all
    // not, and idle animations need `ONCE` or `ONCE_BACKWARDS`; a state without its own animations has the default
    // state's (and its idle flag)
    let mut default_idle = false;
    for si in 0..4 {
        w.push(if g.below(8) == 0 { 3 } else { g.below(3) as i32 });
        let na = if g.below(5) == 0 { 0 } else { 1 + g.below(3) as i32 };
        let idle = g.below(3) == 0;
        let effective_idle = if na > 0 { idle } else { si > 0 && default_idle };
        if si == 0 {
            default_idle = na > 0 && idle;
        }
        w.push(if effective_idle { [2, 5][g.below(2) as usize] } else { g.below(6) as i32 });
        let mut flags = 0i32;
        for bit in FLAG_BITS {
            if g.below(6) == 0 {
                flags |= 1 << bit;
            }
        }
        w.push(flags);
        w.push(b(SPEEDS[g.below(6) as usize]));
        w.push(b(SPEEDS[g.below(6) as usize]));
        w.push(na);
        for _ in 0..3 {
            w.push(if g.below(12) == 0 { MISSING_ANIM } else { g.below(nanims) as i32 });
            w.push(i32::from(idle));
            w.push(b(DISTANCES[g.below(4) as usize]));
        }
        w.push(g.below(4) as i32);
        for _ in 0..3 {
            w.push(g.below(7) as i32);
            w.push(g.below(6) as i32);
        }
    }
    assert_eq!(w.len(), HEADER);
    for _ in 0..1 + g.below(60) {
        let mut s = [0i32; STEP];
        let op = match g.below(100) {
            0..=7 => 0,
            8..=32 => 1,
            33..=52 => 2,
            53..=67 => 3,
            68..=70 => 4,
            71..=74 => 5,
            75..=78 => 6,
            79..=83 => 7,
            84..=87 => 8,
            88..=89 => 9,
            90..=91 => 10,
            92..=94 => 11,
            _ => 12,
        };
        s[0] = op;
        s[1] = match op {
            2 => match g.below(10) {
                0 => 0,
                1..=4 => 33,
                5..=7 => g.below(100) as i32,
                8 => 100 + g.below(3000) as i32,
                _ => g.below(100_000) as i32,
            },
            4 | 11 => i32::from(g.below(2) == 0),
            5 => i32::from(g.below(4) != 0),
            6 => i32::from(g.below(4) != 0),
            _ => g.below(1000) as i32,
        };
        match op {
            1 | 8 => s[2..].copy_from_slice(&world(g)),
            6 => s[2] = b(if g.below(4) == 0 { 0.0 } else { g.uniform(0.0, 3.0) }),
            7 => {
                // a turret's turn about Z, a pitch about Y, a recoil's shift along -X
                let mut m = Matrix3D::IDENTITY;
                match g.below(3) {
                    0 => m.rotate_z(g.uniform(-4.0, 4.0)),
                    1 => m.rotate_y(g.uniform(-1.5, 1.5)),
                    _ => m.translate_x(-g.uniform(0.0, 3.0)),
                }
                s[2..].copy_from_slice(&matrix_words(&m));
            }
            10 => s[2] = b([1.0f32, 0.5, 2.0, 1.005][g.below(4) as usize]),
            _ => {}
        }
        w.extend(s);
    }
    (file, w)
}

/// The test's asset manager: `M0`..`M2` (HLODs on the file's hierarchy) and the file's animations (each one shared
/// object, as upstream's reference-counted `HAnimClass`).
struct Assets {
    tree: HTree,
    anims: Vec<Arc<AnimData>>,
}

impl Assets {
    fn anim_index(&self, name: &[u8]) -> Option<usize> {
        self.anims.iter().position(|a| a.motion.name().eq_ignore_ascii_case(name))
    }
}

impl W3dAssetSource for Assets {
    fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
        MODEL_NAMES[..3].iter().any(|n| n.as_bytes().eq_ignore_ascii_case(name)).then_some(true)
    }
    fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle> {
        let i = self.anim_index(name)?;
        let a = &self.anims[i];
        Some(AnimHandle { name: a.motion.name().to_vec(), num_frames: a.motion.get_num_frames(), frame_rate: get_frame_rate(&a.motion), data: Some(a.clone()) })
    }
    /// The render object's hierarchy as `Create_Render_Obj(name, scale, color)` makes it (`scale_render_obj_hierarchy`),
    /// posed by `Base_Update` under `Matrix3D(true).Scale(scale)` (the pose is not asked outside a logic time).
    fn skeleton(&mut self, name: &[u8], _pose: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
        self.create_render_obj(name)?;
        let mut tree = self.tree.clone();
        scale_render_obj_hierarchy(&mut tree, scale);
        let mut root = Matrix3D::IDENTITY;
        root.scale(scale);
        tree.base_update(&root).expect("parents come first");
        Some(Skeleton { bones: Skeleton::bones_of(&tree), sub_objects: Vec::new(), scale_factor: tree.scale_factor })
    }
}

fn line(depth: u32, field: &str, args: &str) -> ClientLine {
    ClientLine { depth, field: field.as_bytes().to_vec(), args: args.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() }
}

fn flags(name: &str) -> ModelConditionFlags {
    if name.is_empty() {
        return ModelConditionFlags::new();
    }
    let i = MODEL_CONDITION_NAMES.iter().position(|n| *n == name).expect("a condition") as i32;
    ModelConditionFlags::from_indices(&[i])
}

fn push_matrix(out: &mut Vec<i32>, m: &Matrix3D) {
    out.extend(matrix_words(m));
}

/// The wrapper's `state()`.
fn state_words(out: &mut Vec<i32>, m: &W3DModelDraw, mgr: &ParticleSystemManager, states: &[usize], assets: &Assets, random: &RandomValues) {
    let si = match m.cur_state_ref() {
        Some(StateRef::Normal(i)) => states.iter().position(|s| s == i).map_or(-1, |p| p as i32),
        _ => -1,
    };
    out.extend([
        100,
        m.need_recalc_bone_particle_systems as i32,
        m.fully_obscured_by_shroud as i32,
        m.which_anim_in_cur_state(),
        m.pause_animation() as i32,
        anim_mode_index(m.animation_mode()),
        si,
        m.particle_system_ids.len() as i32,
    ]);
    for t in &m.particle_system_ids {
        out.extend([t.id as i32, t.bone_index]);
    }
    let mut l: Vec<_> = mgr.systems.iter().collect();
    l.sort_by_key(|s| s.id);
    out.push(l.len() as i32);
    for s in l {
        out.extend([
            s.id as i32,
            s.attached_to_drawable as i32,
            s.is_saveable as i32,
            s.is_stopped as i32,
            s.is_destroyed as i32,
            s.skip_parent_xfrm as i32,
            s.is_local_identity as i32,
            s.slave.unwrap_or(0) as i32,
            s.master.unwrap_or(0) as i32,
        ]);
        push_matrix(out, &s.local_transform);
    }
    match &m.render_object {
        None => out.push(0),
        Some(ro) => {
            out.extend([1, if ro.anim.is_some() { 2 } else { 1 }, ro.is_hierarchy_valid() as i32]);
            match &ro.anim {
                Some(a) => out.extend([
                    assets.anim_index(&a.motion.name).map_or(-1, |i| i as i32),
                    b(a.frame),
                    b(a.prev_frame),
                    a.last_sync_time as i32,
                    b(a.anim_direction),
                    b(a.frame_rate_multiplier),
                    anim_mode_index(a.anim_mode),
                    ro.is_animation_complete_hlod() as i32,
                ]),
                None => out.extend([-1, 0, 0, 0, 0, 0, 0, 0]),
            }
            out.push(ro.hidden as i32);
            push_matrix(out, &ro.transform);
            let pivots = ro.live_pivot_transforms();
            out.extend([b(ro.object_scale), pivots.len() as i32]);
            for p in &pivots {
                push_matrix(out, p);
            }
        }
    }
    out.extend(random.client.map(|x| x as i32));
}

thread_local! {
    /// Where each step's records end in the last Rust case (for the mismatch report).
    static STEP_ENDS: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The last Rust case's module block (for the mismatch report).
    static BLOCK: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// The systems the last Rust case made (its manager's last id).
static SYSTEMS_MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The cases ended at a `setModelState` that is undefined upstream (see `rust_case`).
static UNDEFINED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The client random values the last Rust case drew (the generator's counter word moved on).
static RANDOM_DRAWN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The last Rust case's draws with an animation playing (the render object in `SINGLE_ANIM` when `doDrawModule` ran).
static ANIMATED_DRAWS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The Rust's words for a case (the wrapper's records).
fn rust_case(file: &[u8], w: &[i32]) -> Vec<i32> {
    STEP_ENDS.with(|e| e.borrow_mut().clear());
    let (tree, anims) = rust_load(file);
    let mut assets = Assets { tree, anims: anims.into_iter().map(|a| Arc::new(AnimData { motion: Arc::new(a), poses: Vec::new() })).collect() };
    let mut k = 0;
    let mut next = || {
        k += 1;
        w[k - 1]
    };
    let seed = next() as u32;
    let mut now = next() as u32;
    let drawable_id = next();
    let mut scale = r(next());
    let attached = next() != 0;
    let require_power = next() != 0;
    let mut mgr = ParticleSystemManager::new();
    let mut slaves = [-1i32; 4];
    for s in &mut slaves {
        *s = next();
    }
    for (t, &sl) in slaves.iter().enumerate() {
        let tm = mgr.new_template(TEMPLATE_NAMES[t].as_bytes());
        tm.info.slave_system_name = if sl < 0 { Vec::new() } else { TEMPLATE_NAMES[sl as usize].as_bytes().to_vec() };
    }
    let nstates = next() as usize;
    let mut lines = vec![
        line(0, "ParticlesAttachedToAnimatedBones", if attached { "Yes" } else { "No" }),
        line(0, "AnimationsRequirePower", if require_power { "Yes" } else { "No" }),
    ];
    let names: Vec<String> = assets.anims.iter().map(|a| String::from_utf8(a.motion.name().to_vec()).expect("ASCII")).collect();
    for (si, bit) in STATE_BITS.iter().enumerate() {
        let model = next() as usize;
        let mode = next() as usize;
        let fl = next();
        let (lo, hi) = (r(next()), r(next()));
        let na = next() as usize;
        let mut anim_lines = Vec::new();
        for ai in 0..3 {
            let a = next();
            let idle = next() != 0;
            let dist = r(next());
            if ai < na {
                let name = if a == MISSING_ANIM { "SKEL.NOSUCH".to_string() } else { names[a as usize].clone() };
                anim_lines.push(line(1, if idle { "IdleAnimation" } else { "Animation" }, &format!("{name} {dist:?}")));
            }
        }
        let nb = next() as usize;
        let mut bones = Vec::new();
        for bi in 0..3 {
            let bone = next() as usize;
            let tmpl = next() as usize;
            if bi < nb {
                bones.push(format!("{} {}", BONE_NAMES[bone], TEMPLATE_NAMES[tmpl]));
            }
        }
        if si >= nstates {
            continue;
        }
        lines.push(if si == 0 { line(0, "DefaultConditionState", "") } else { line(0, "ConditionState", bit) });
        lines.push(line(1, "Model", MODEL_NAMES[model]));
        lines.push(line(1, "AnimationMode", THE_ANIM_MODE_NAMES[mode]));
        let set: Vec<&str> = (0..10).filter(|&i| fl & (1 << i) != 0).map(|i| AC_BITS_NAMES[i]).collect();
        lines.push(line(1, "Flags", &if set.is_empty() { "NONE".to_string() } else { set.join(" ") }));
        lines.push(line(1, "AnimationSpeedFactorRange", &format!("{lo:?} {hi:?}")));
        lines.extend(anim_lines);
        for bl in &bones {
            lines.push(line(1, "ParticleSysBone", bl));
        }
    }
    let block = ClientModuleData { module_tag_name_key: Default::default(), lines };
    BLOCK.with(|t| *t.borrow_mut() = String::from_utf8_lossy(&rts_lockstep_engine::client::w3d_model_draw::client_block_text(&block)).into_owned());
    let data = match W3DModelDrawModuleData::parse(b"W3DModelDraw", &block) {
        Ok(d) => Arc::new(d.expect("a model draw")),
        Err(e) => panic!("{e:?}: {}", String::from_utf8_lossy(&rts_lockstep_engine::client::w3d_model_draw::client_block_text(&block))),
    };
    let states: Vec<usize> = STATE_BITS[..nstates].iter().map(|bit| data.find_best_info(&flags(bit)).expect("a state")).collect();
    let mut random = RandomValues::new(seed);
    let counter0 = random.client[5];
    let mut animated_draws = 0u64;
    let mut out = Vec::new();
    let mut m = {
        let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: now, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
        W3DModelDraw::new_scaled(data.clone(), 0, false, false, scale, Arc::new(Vec::new()), &mut env).expect("an idle state")
    };
    m.drawable_id = drawable_id;
    let (mut should_animate, mut has_object, mut speed) = (true, false, 0.0f32);
    state_words(&mut out, &m, &mgr, &states, &assets, &random);
    for s in w[k..].chunks_exact(STEP) {
        match s[0] {
            0 => {
                let new_state = StateRef::Normal(states[(s[1] as u32 % nstates as u32) as usize]);
                // `getCurrentAnimFraction` reads uninitialised locals upstream when the render object plays no animation
                // (`Peek_Animation_And_Info` answers NULL) under a maintain-frame state: undefined, the case ends there
                let undefined = m.cur_state_ref().is_some_and(|c| *c != new_state)
                    && m.get_cur_state().is_some_and(|c| is_any_maintain_frame_flag_set(c.flags))
                    && m.render_object.as_ref().is_some_and(|ro| ro.anim.is_none());
                if undefined {
                    out.push(700);
                    UNDEFINED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
                let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: now, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
                m.set_model_state(new_state, &mut env);
            }
            1 => {
                let world = matrix(&s[2..]);
                let facts = DrawFacts {
                    should_animate_with_power: should_animate,
                    should_animate_without_power: should_animate,
                    speed: has_object.then_some(speed),
                    turrets: [None, None],
                    motion: None,
                    show_client_physics: true,
                    wheel_info: None,
                    transform: world,
                    drawable_transform: world,
                    construction: None,
                    frozen: false,
                    has_object,
                };
                animated_draws += u64::from(m.render_object.as_ref().is_some_and(|ro| ro.anim.is_some()));
                let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: now, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
                m.do_draw_module(&facts, &mut env);
            }
            2 => now = now.wrapping_add(s[1] as u32),
            3 => m.render(now),
            4 => m.set_hidden(s[1] != 0, Some(&mut mgr)),
            5 => should_animate = s[1] != 0,
            6 => {
                has_object = s[1] != 0;
                speed = r(s[2]);
            }
            7 => {
                if let Some(ro) = m.render_object.as_mut() {
                    let n = ro.skeleton.as_ref().map_or(0, |k| k.bones.len()) as u32;
                    let bone = (s[1] as u32 / 2 % n) as i32;
                    if s[1] & 1 != 0 {
                        ro.release_bone(bone);
                    } else {
                        ro.capture_bone(bone);
                        ro.control_bone(bone, &matrix(&s[2..]));
                    }
                }
            }
            8 => {
                m.react_to_transform_change(&matrix(&s[2..]), None);
                m.update_bones_for_client_particle_systems(Some(&mut mgr), now);
            }
            9 => {
                if !mgr.systems.is_empty() {
                    let mut ids: Vec<u32> = mgr.systems.iter().map(|x| x.id).collect();
                    ids.sort_unstable();
                    let id = ids[s[1] as usize % ids.len()];
                    // `~ParticleSystem`: unlinked from its slave and master, then unlisted
                    let i = mgr.systems.iter().position(|x| x.id == id).expect("listed");
                    if let Some(sl) = mgr.systems[i].slave.take() {
                        if let Some(j) = mgr.systems.iter().position(|x| x.id == sl) {
                            mgr.systems[j].master = None;
                        }
                    }
                    if let Some(ma) = mgr.systems[i].master.take() {
                        if let Some(j) = mgr.systems.iter().position(|x| x.id == ma) {
                            mgr.systems[j].slave = None;
                        }
                    }
                    mgr.systems.remove(i);
                }
            }
            10 => {
                scale = r(s[2]);
                m.scale = scale;
            }
            11 => m.set_fully_obscured_by_shroud(s[1] != 0, Some(&mut mgr)),
            12 => {
                if let Some(ro) = m.render_object.as_mut() {
                    let n = ro.skeleton.as_ref().map_or(0, |k| k.bones.len()) as u32;
                    let t = ro.get_bone_transform((s[1] as u32 % n) as i32, now);
                    out.push(600);
                    push_matrix(&mut out, &t);
                }
            }
            op => panic!("op {op}"),
        }
        state_words(&mut out, &m, &mgr, &states, &assets, &random);
        STEP_ENDS.with(|e| e.borrow_mut().push(out.len()));
    }
    SYSTEMS_MADE.store(u64::from(mgr.unique_system_id), std::sync::atomic::Ordering::Relaxed);
    RANDOM_DRAWN.store(u64::from(random.client[5].wrapping_sub(counter0)), std::sync::atomic::Ordering::Relaxed);
    ANIMATED_DRAWS.store(animated_draws, std::sync::atomic::Ordering::Relaxed);
    out
}

fn run_cpp(file: &[u8], input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 21];
    let n = unsafe { cpp::ref_model_draw_anim_seq(file.as_ptr() as *const _, file.len() as i32, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "the C++ loaders refused the file");
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

/// Every case's comparison; returns the check and (steps, draws, draws with an animation playing, particle systems
/// made, client random values drawn, max ULP among differing finite words).
fn compare(name: &str, seed: u64, n: u64) -> (Check, [u64; 6]) {
    let mut c = Check::new(name);
    let mut g = Gen::new(seed);
    let (mut steps, mut draws, mut animated, mut systems, mut drawn, mut max_ulp) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for k in 0..n {
        let (file, w) = random_case(&mut g);
        let rust = rust_case(&file, &w);
        let cc = run_cpp(&file, &w);
        // the steps run (a case ends early where upstream is undefined)
        let ran = STEP_ENDS.with(|e| e.borrow().len());
        steps += ran as u64;
        draws += w[HEADER..].chunks_exact(STEP).take(ran).filter(|s| s[0] == 1).count() as u64;
        systems += SYSTEMS_MADE.swap(0, std::sync::atomic::Ordering::Relaxed);
        drawn += RANDOM_DRAWN.swap(0, std::sync::atomic::Ordering::Relaxed);
        animated += ANIMATED_DRAWS.swap(0, std::sync::atomic::Ordering::Relaxed);
        if rust != cc {
            if rust.len() == cc.len() {
                for (a, bb) in rust.iter().zip(&cc) {
                    let (fa, fb) = (r(*a), r(*bb));
                    if a != bb && fa.is_finite() && fb.is_finite() {
                        max_ulp = max_ulp.max(support::ulp_distance(fa, fb));
                    }
                }
            }
            let at = rust.iter().zip(&cc).position(|(a, bb)| a != bb).unwrap_or(rust.len().min(cc.len()));
            if !planted() && c.mismatches < 4 {
                let step = STEP_ENDS.with(|e| e.borrow().iter().position(|&end| end > at));
                let ops: Vec<(i32, i32)> = w[HEADER..].chunks_exact(STEP).map(|s| (s[0], s[1])).collect();
                println!("case {k}: first difference in step {step:?} of {:?}", ops);
                if std::env::var("MODEL_DRAW_ANIM_DEBUG").is_ok() {
                    BLOCK.with(|t| println!("{}", t.borrow()));
                    let ends = STEP_ENDS.with(|e| e.borrow().clone());
                    let st = step.unwrap_or(0);
                    let from = if st == 0 { 0 } else { ends[st - 1] };
                    println!("rust step {st}: {:?}", &rust[from.min(rust.len())..ends[st].min(rust.len())]);
                    println!("c++  step {st}: {:?}", &cc[from.min(cc.len())..ends[st].min(cc.len())]);
                    if st > 0 {
                        let f0 = if st == 1 { 0 } else { ends[st - 2] };
                        println!("before: {:?}", &rust[f0..from]);
                    }
                }
                let lo = at.saturating_sub(8);
                println!(
                    "case {k}: at word {at} (lengths {} {}): rust {:?} c++ {:?}",
                    rust.len(),
                    cc.len(),
                    &rust[lo.min(rust.len())..(at + 16).min(rust.len())],
                    &cc[lo.min(cc.len())..(at + 16).min(cc.len())]
                );
            }
        }
        c.eq(rust, cc, || format!("case {k}"));
    }
    (c, [steps, draws, animated, systems, drawn, max_ulp])
}

/// The gated comparison: `MODEL_DRAW_ANIM_CASES` cases (20,000 by default), 0 mismatches.
#[test]
fn the_animated_draw_sequence_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (c, [steps, draws, animated, systems, drawn, max_ulp]) = compare("W3DModelDraw animated draw sequence", 0x181_0007, cases());
    println!(
        "W3DModelDraw animated: {} cases ({} ended where upstream is undefined), {steps} steps, {draws} draws ({animated} with an animation playing), {systems} particle systems made, {drawn} client random values drawn; max ULP among differing finite words {max_ulp}",
        cases(),
        UNDEFINED.load(std::sync::atomic::Ordering::Relaxed)
    );
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}
