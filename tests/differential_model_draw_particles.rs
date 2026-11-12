#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp (the bone particle
//   systems: recalcBonesForClientParticleSystems, updateBonesForClientParticleSystems, stopClientParticleSystems,
//   doStartOrStopParticleSys, setHidden, setFullyObscuredByShroud, and the parts of the constructor, setModelState and
//   doDrawModule that reach them) and its header (ParticleSysTrackerType)
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp (ParticleSystem::setSaveable, start, stop,
//   destroy, setPosition, setLocalTransform, rotateLocalTransformZ, attachToDrawable)
//   (GitHub #172) W3DModelDraw.cpp (getCurrentWorldspaceClientBonePositions); W3DAssetManager.cpp (Create_Render_Obj's
//   scaling); GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htree.cpp (HTreeClass::Scale, Base_Update), htree.h
//   (Get_Transform), hlod.cpp (HLodClass::Scale), animobj.cpp (Get_Bone_Transform)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #171: `W3DModelDraw`'s bone particle systems against the upstream text compiled verbatim
//! (`differential/wrap_model_draw_particles.cpp`, which lists the cuts, the stand-ins and the words).
//!
//! # Methodology
//!
//! Each case makes a module (the port's whole [`W3DModelDraw`], parsed from a `Draw` block) of a drawable of a random id
//! and scale, with `ParticlesAttachedToAnimatedBones` on or off; four particle system templates `PS0`..`PS3`, each with a
//! later one as its slave or none; three models (HLOD or mesh, 0 to 6 bones named from a pool with case and duplicate
//! variations, each with a random pose: rotation-and-scale entries in -2..2, translation in -50..50, and (#172) a random
//! earlier pivot as its parent, so chains up to 5 deep) and the names `None`
//! (no model) and `Missing` (no such render object); one to four states, each with a model and up to three
//! `ParticleSysBone`s (a bone name from a pool that includes one no model has, and a template, `None` or an unknown
//! name). The asset source gives the port's skeleton as `W3dAssetSource::skeleton` promises it: ~~each bone under
//! `Matrix3D(true).Scale(scale)`, by WWMath's `Multiply`~~ **Corrected 2026-11-12 (#172):** the hierarchy scaled as
//! `Create_Render_Obj(name, scale, color)` scales it (the port's `scale_render_obj_hierarchy` and `HTree::scale`), posed
//! by the port's `Base_Update` under `Matrix3D(true).Scale(scale)`; the C++ render object is made the same way by the
//! cut upstream (`HTreeClass::Scale`, `Base_Update`, `Get_Transform`, the asset manager's `reallyscale`). Then 1 to 40
//! steps: `setModelState` 15 %, `doDrawModule` ~~35 %~~ 30 % (with a random world transform: ~~the C++ render object gets
//! `world * Scale(scale)`~~ (#172) the C++ render object gets `world`, as upstream's does (no template scale in its
//! transform), the port [`DrawFacts::transform`] = `world`), `updateBonesForClientParticleSystems` 15 % (the same), the
//! drawable hidden or shown 10 %, the shroud 10 %, a system dropped by the manager ~~8 %~~ 7 % (as its update deletes
//! one), the drawable's `NO_STATE_PARTICLES` status ~~7 %~~ 6 %, and (#172) `getCurrentWorldspaceClientBonePositions`
//! of a name from the bone pool under a random world 7 % (recorded: found, and the 12 words). ~~1 in 20 world
//! transforms of a case at scale 1 has entries from `Gen::finite` (huge, tiny and subnormal values, never NaN or
//! infinite: see the results).~~ (#172) The main test: 1 in 20 world transforms has entries from `Gen::finite` (huge,
//! tiny and subnormal) at any scale; the second test: 1 in 10 from `Gen::mixed` (infinities and NaN too). Compared, every
//! word as bits (NaN on both sides as equal), after the constructor and after each step: the module's
//! `m_needRecalcBoneParticleSystems`, `m_fullyObscuredByShroud`, its trackers (id, bone), and every system in the manager
//! in id order (attached drawable, saveable, stopped, destroyed, skip-parent, local identity, slave, master, local
//! transform).
//!
//! - ~~**The main test** draws the scale from the powers of two 0.25 .. 4, where the port's bone convention (the skeleton
//!   carries the scale: `world * (Scale * pose)`) and upstream's (`(world * Scale) * pose`) round the same.~~
//!   **Corrected 2026-11-12 (#172):** neither was upstream's. Upstream scales the render object's *hierarchy* (the
//!   pivots' translations, `HTreeClass::Scale`, when `|scale - 1| > 0.01`) and chains each bone from the render object's
//!   transform; the port now does the same (module docs of `client::w3d_model_draw`). **The main test** (gated, 20,000)
//!   draws the scale from the powers of two 0.25 .. 4 (40 %), the edge of `scale_epsilon` (0.9899 .. 1.0101, 20 %) and
//!   any in 0.5 .. 2 (40 %).
//! - ~~**The second test** (2,000 cases) draws any scale in 0.5 .. 2 and reports, not gates, how far the two conventions
//!   round apart.~~ **Corrected 2026-11-12 (#172): the second test** (gated, 20,000 cases) draws any scale in 0.25 .. 4,
//!   or from `Gen::mixed` (0, NaN, infinities) 1 in 10, with infinite and NaN world entries.
//!
//! # Predictions (written before the first run, `issue171_predictions.md`)
//!
//! Main: 0 mismatches, max ULP 0, no case undefined upstream. Second: mismatching words only in the local transforms
//! `updateBonesForClientParticleSystems` writes, max ULP <= 4. All 30 plants caught.
//!
//! # Results (2026-11-12)
//!
//! - First build: the wrapper declared its own `DrawableID` beside GameType.h's enum (removed; the wrapper's error).
//! - First run: 4,970 of 20,000 sequences mismatched: the wrapper's states did not start as a copy of the default state's
//!   `ParticleSysBone` list, as `parseConditionState` makes them (the port's parse does). The wrapper's error, fixed.
//! - Second run: 2 of 20,000 mismatched, both a world transform with an infinite entry (from `Gen::mixed`): upstream's
//!   `world * Scale(1)` makes `inf * 0 = NaN` in the row, the port's `world * (Scale * pose)` keeps the infinity. ~~Not a
//!   translation defect but the bone convention (module docs of `w3d_model_draw`), which no drawable's transform reaches;
//!   the mixed words are now finite (`Gen::finite`).~~ **Corrected 2026-11-12 (#172):** a translation defect (#172): the
//!   port's bone order was not upstream's, and the stand-in's `world * Scale(1)` was not upstream's either; with both
//!   fixed, infinite entries agree (second test). **A prediction miss** (I predicted 0 at scale 1).
//! - **Main, third run:** 20,000 sequences, 409,427 steps, 52,828 particle systems made; 0 NaN words; 0 undefined
//!   upstream (none generated: no template cycles, every bone index in range); **0 mismatches, max ULP 0**.
//! - ~~**Any scale (not gated):** 230 of 2,000 sequences differ, max ULP 32,768 (on words near zero after a
//!   cancellation), largest difference 3.8e-6 of max(|value|, 1): rounding only, every id, bone and flag word equal.
//!   **A prediction miss** on the size (I predicted max ULP <= 4; ULP is the wrong measure for words near zero).~~
//!   **Corrected 2026-11-12 (#172):** that measured the port against a stand-in that shared its mistaken picture of the
//!   scale; superseded below.
//!
//! # #172 (2026-11-12): predictions and results
//!
//! Predicted (`issue172_predictions.md`, before any change or run): both tests 0 mismatches over 20,000 cases; NaN on
//! both sides with differing payload bits 0; plants 9691-9698 caught (9697 by the lib test
//! `a_scaled_gun_s_bones_are_upstream_s`, which this test cannot reach); the 30 plants of #171 still caught.
//!
//! - **First run** (the port and the wrapper as described above): **main 0 mismatches** over 20,000 sequences (407,610
//!   steps, 50,486 systems made, 147 NaN words, max ULP 0); **any scale 0 mismatches** over 20,000 (409,774 steps,
//!   50,422 systems, 1,125 NaN words, max ULP 0). 204 words are NaN on both sides with different bits (counted and
//!   normalised as before): **a prediction miss** (I predicted 0; the NaN arise from NaN or infinite inputs on both
//!   sides, and their payload/sign bits are not specified by the arithmetic either side runs).
//! - Plants (main test, mismatching of 20,000): 9691 1,973; 9692 3,140; 9693 632; 9694 1,406; 9695 2,963; 9696 2,307;
//!   9698 1,449; 9697 0 here (not reachable), caught by the lib test. **8 of 8 caught.** The #171 plants re-run on
//!   the changed test: 30 of 30 caught (9661 18,834 ... 9690 1,381).
//!
//! Planted bugs, mismatching sequences of 20,000: 9661 19,070; 9662 1,708; 9663 2,894; 9664 1,358; 9665 1,466; 9666
//! 13,237; 9667 13,237; 9668 5,045; 9669 2,379; 9670 2,401; 9671 2,894; 9672 13,237; 9673 6,509; 9674 2,295; 9675 2,295;
//! 9676 2,295; 9677 1,534; 9678 3,957; 9679 7,426; 9680 6,397; 9681 3,751; 9682 7,426; 9683 4,065; 9684 2,588; 9685
//! 11,728; 9686 1,466; 9687 1,428; 9688 9,633; 9689 3,501; 9690 1,466. **30 of 30 caught.** (9665, 9686 and 9690 match
//! the same 1,466 sequences: a recalc's position is seen only where no update follows, i.e. without
//! `ParticlesAttachedToAnimatedBones`.)
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9661 | the recalc flag is never cleared |
//! | 9662 | the recalc ignores the drawable's `NO_STATE_PARTICLES` |
//! | 9663 | the recalc's bone index is always 0 |
//! | 9664 | the recalc does not turn the system by the bone's Z rotation |
//! | 9665 | the recalc swaps the bone's x and y |
//! | 9666 | the recalc does not attach the system to the drawable |
//! | 9667 | the recalc leaves the system saveable |
//! | 9668 | `setSaveable` does not reach the slave |
//! | 9669 | the recalc's hidden test ignores the shroud |
//! | 9670 | the recalc's hidden test ignores the drawable's hidden flag |
//! | 9671 | the tracker keeps bone 0 |
//! | 9672 | the recalc stops the systems of a drawable that is shown |
//! | 9673 | the update moves systems made at bone 0 |
//! | 9674 | the update does not skip the parent's transform |
//! | 9675 | the update does not set the local transform |
//! | 9676 | the update leaves the bone in model space |
//! | 9677 | the update moves every system to the first tracker's bone |
//! | 9678 | `doStartOrStopParticleSys` ignores the drawable's hidden flag |
//! | 9679 | `stopClientParticleSystems` does not clear the list |
//! | 9680 | `stopClientParticleSystems` stops, not destroys |
//! | 9681 | `setModelState` sets the flag despite `NO_STATE_PARTICLES` |
//! | 9682 | `setModelState` does not stop the old systems |
//! | 9683 | `doStartOrStopParticleSys` ignores the shroud |
//! | 9684 | `doStartOrStopParticleSys` never restarts a system |
//! | 9685 | `setFullyObscuredByShroud` does not keep the flag |
//! | 9686 | `doDrawModule` updates the bones without `ParticlesAttachedToAnimatedBones` |
//! | 9687 | `doDrawModule` never updates the bones |
//! | 9688 | `doDrawModule` recalcs only with a render object |
//! | 9689 | `setHidden` does not stop or start the systems |
//! | 9690 | the recalc's bone position has z 0 |
//! | 9691 | (#172) the update's bone is `world * (bone under Scale)` (the old convention) |
//! | 9692 | (#172) the render object's hierarchy is never scaled |
//! | 9693 | (#172) the hierarchy is scaled within `scale_epsilon` of 1 too |
//! | 9694 | (#172) the live bone is `root * (chain from the identity)` |
//! | 9695 | (#172) `HTreeClass::Scale` scales the pivots' rotations too |
//! | 9696 | (#172) `getCurrentWorldspaceClientBonePositions` is `world * (bone under Scale)` |
//! | 9697 | (#172) `handleWeaponFireFX`'s FX bone is the bone under Scale, not in the world (lib test) |
//! | 9698 | (#172) the live bone ignores its parents (every pivot under the root) |

mod support;

use std::sync::Arc;

use rts_lockstep_engine::client::particle_sys::ParticleSystemManager;
use rts_lockstep_engine::client::w3d_model_draw::{scale_render_obj_hierarchy, AnimHandle, DrawEnv, DrawFacts, DrawParticles, Skeleton, StateRef, W3DModelDraw, W3DModelDrawModuleData, W3dAssetSource};
use rts_lockstep_engine::formats::w3d::htree::{HTree, Pivot};
use rts_lockstep_engine::common::model_state::{ModelConditionFlags, MODEL_CONDITION_NAMES};
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::thing_template::{ClientLine, ClientModuleData};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use support::{Check, Gen};

/// The wrapper's records are one global buffer: one test at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases() -> u64 {
    std::env::var("MODEL_DRAW_PARTICLES_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn b(f: f32) -> i32 {
    f.to_bits() as i32
}

fn r(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

/// The wrapper's `BONE_NAMES` (the models' pivots) and `STATE_BONE_NAMES` (the `ParticleSysBone` lines').
const BONE_NAMES: [&str; 6] = ["RootTransform", "Bone01", "Smoke01", "FX_A", "Muzzle", "SMOKE01"];
const STATE_BONE_NAMES: [&str; 6] = ["RootTransform", "bone01", "SMOKE01", "fx_a", "Muzzle", "nosuchbone"];
const MODEL_NAMES: [&str; 5] = ["M0", "M1", "M2", "None", "Missing"];
const TEMPLATE_NAMES: [&str; 6] = ["PS0", "PS1", "PS2", "PS3", "None", "NoSuch"];
/// The states' condition bits (the first state is the default).
const STATE_BITS: [&str; 4] = ["", "DAMAGED", "RUBBLE", "GARRISONED"];

const HEADER: usize = 7 + 3 * (2 + 6 * 14) + 1 + 4 * (2 + 3 * 2);
const STEP: usize = 14;

/// A world entry that is not well scaled: `Gen::finite` (huge, tiny, subnormal), or (#172, the second test) `Gen::mixed`,
/// infinities and NaN included.
#[derive(Clone, Copy, PartialEq)]
enum Odd {
    None,
    Finite,
    Any,
}

fn matrix_words(g: &mut Gen, lo: f32, hi: f32, t: f32, odd: Odd) -> [i32; 12] {
    let mut w = [0i32; 12];
    for (k, v) in w.iter_mut().enumerate() {
        let f = if odd != Odd::None && g.below(3) == 0 {
            // ~~finite: an infinite world entry separates the two bone conventions too (`inf * 0` in upstream's
            // `world * Scale`, none in the port's), see the results~~ (#172) both sides now run the same chain: the
            // second test draws infinities and NaN too
            if odd == Odd::Any { g.mixed() } else { g.finite() }
        } else if k % 4 == 3 {
            g.uniform(-t, t)
        } else {
            g.uniform(lo, hi)
        };
        *v = b(f);
    }
    w
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

/// One case's words (the wrapper's layout). ~~`any_scale`: a scale in 0.5..2, else a power of two.~~ **Corrected
/// 2026-11-12 (#172):** `any_scale` false (the main test): a power of two (1 included) 40 %, a scale at
/// `scale_epsilon`'s edge (0.99 .. 1.0101) 20 %, any in 0.5..2 40 %, with 1 in 20 world transforms `Gen::finite`; true
/// (the second test): any in 0.25..4 90 %, `Gen::mixed` (0, NaN and infinities included) 10 %, with 1 in 10 world
/// transforms `Gen::mixed`.
fn random_case(g: &mut Gen, any_scale: bool) -> Vec<i32> {
    let mut w = Vec::new();
    w.push(1 + g.below(1000) as i32);
    let scale = if any_scale {
        if g.below(10) == 0 { g.mixed() } else { g.uniform(0.25, 4.0) }
    } else {
        match g.below(10) {
            0..=3 => [0.25f32, 0.5, 1.0, 1.0, 1.0, 2.0, 4.0][g.below(7) as usize],
            // `WWMath::Fabs(scale - 1) > 0.01`: 0.99 and 1.01 are not "really" scales in f32, 0.9899 and 1.0101 are
            4..=5 => [0.9899f32, 0.99, 0.995, 1.005, 1.0099, 1.01, 1.0101][g.below(7) as usize],
            _ => g.uniform(0.5, 2.0),
        }
    };
    w.push(b(scale));
    w.push((g.below(2) == 0) as i32);
    for t in 0..4u32 {
        w.push(if t < 3 && g.below(3) == 0 { (t + 1 + g.below(3 - t)) as i32 } else { -1 });
    }
    for _ in 0..3 {
        w.push((g.below(5) != 0) as i32);
        w.push(g.below(7) as i32);
        for k in 0..6 {
            w.push(if k == 0 { 0 } else { 1 + g.below(5) as i32 });
            // (#172) the pivot's parent: an earlier pivot (chains up to 5 deep); unused for pivot 0
            w.push(if k == 0 { 0 } else { g.below(k) as i32 });
            if g.below(10) == 0 {
                let mut p = [0i32; 12];
                for (i, v) in p.iter_mut().enumerate() {
                    *v = b(if i % 5 == 0 { 1.0 } else if i % 4 == 3 { g.uniform(-50.0, 50.0) } else { 0.0 });
                }
                w.extend(p);
            } else {
                w.extend(matrix_words(g, -2.0, 2.0, 50.0, Odd::None));
            }
        }
    }
    w.push(1 + g.below(4) as i32);
    for _ in 0..4 {
        w.push(g.below(5) as i32);
        w.push(g.below(4) as i32);
        for _ in 0..3 {
            w.push(g.below(6) as i32);
            w.push(g.below(6) as i32);
        }
    }
    assert_eq!(w.len(), HEADER);
    // ~~only at scale 1~~ (#172) at any scale
    for _ in 0..1 + g.below(40) {
        let mut s = [0i32; STEP];
        let op = match g.below(100) {
            0..=14 => 0,
            15..=44 => 1,
            45..=59 => 2,
            60..=69 => 3,
            70..=79 => 4,
            80..=86 => 5,
            87..=92 => 6,
            _ => 7,
        };
        s[0] = op;
        s[1] = match op {
            3 | 4 | 6 => (g.below(2) == 0) as i32,
            _ => g.below(1000) as i32,
        };
        if op == 1 || op == 2 || op == 7 {
            let odd = if any_scale {
                if g.below(10) == 0 { Odd::Any } else { Odd::None }
            } else if g.below(20) == 0 {
                Odd::Finite
            } else {
                Odd::None
            };
            let m = matrix_words(g, -2.0, 2.0, 1000.0, odd);
            s[2..].copy_from_slice(&m);
        }
        w.extend(s);
    }
    w
}

/// The test's asset manager: the case's three models.
struct Models {
    /// (hlod, bones (name, parent, pose)) of `M0`..`M2`.
    models: Vec<(bool, Vec<(&'static str, u32, Matrix3D)>)>,
}

impl Models {
    fn find(&self, name: &[u8]) -> Option<usize> {
        MODEL_NAMES[..3].iter().position(|n| n.as_bytes().eq_ignore_ascii_case(name))
    }
}

impl W3dAssetSource for Models {
    fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
        self.find(name).map(|i| self.models[i].0)
    }
    fn get_hanim(&mut self, _: &[u8]) -> Option<AnimHandle> {
        None
    }
    fn skeleton(&mut self, name: &[u8], _pose: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
        let i = self.find(name)?;
        let (hlod, bones) = &self.models[i];
        if !hlod {
            return Some(Skeleton::default());
        }
        if bones.is_empty() {
            // an HLOD without pivots: no hierarchy
            return Some(Skeleton::default());
        }
        // ~~each bone under `Matrix3D(true).Scale(scale)` (`W3dFiles::skeleton`'s root)~~ (#172) as `W3dFiles::skeleton`
        // makes it: the hierarchy scaled as `Create_Render_Obj(name, scale, color)` scales it (the port's
        // `scale_render_obj_hierarchy`, `HTree::scale`), posed by the port's `Base_Update` under
        // `Matrix3D(true).Scale(scale)`
        let pivots = bones
            .iter()
            .enumerate()
            .map(|(i, (n, parent, pose))| {
                let mut name = [0u8; 16];
                name[..n.len()].copy_from_slice(n.as_bytes());
                Pivot::new(name, if i == 0 { None } else { Some(*parent) }, *pose, i as i32)
            })
            .collect();
        let mut tree = HTree { name: [0; 16], num_pivots: bones.len() as i32, pivots, scale_factor: 1.0 };
        scale_render_obj_hierarchy(&mut tree, scale);
        let mut root = Matrix3D::IDENTITY;
        root.scale(scale);
        tree.base_update(&root).expect("parents come first");
        Some(Skeleton { bones: Skeleton::bones_of(&tree), sub_objects: Vec::new() })
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

fn state_words(out: &mut Vec<i32>, m: &W3DModelDraw, mgr: &ParticleSystemManager) {
    out.extend([100, m.need_recalc_bone_particle_systems as i32, m.fully_obscured_by_shroud as i32, m.particle_system_ids.len() as i32]);
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
        for row in 0..3 {
            for c in 0..4 {
                out.push(b(s.local_transform.row[row][c]));
            }
        }
    }
}

/// The Rust's words for a case (the wrapper's records).
fn rust_case(w: &[i32]) -> Vec<i32> {
    let mut k = 0;
    let mut next = || {
        k += 1;
        w[k - 1]
    };
    let drawable_id = next();
    let scale = r(next());
    let attached = next() != 0;
    let mut mgr = ParticleSystemManager::new();
    let mut slaves = [-1i32; 4];
    for s in &mut slaves {
        *s = next();
    }
    for (t, &sl) in slaves.iter().enumerate() {
        let tm = mgr.new_template(TEMPLATE_NAMES[t].as_bytes());
        tm.info.slave_system_name = if sl < 0 { Vec::new() } else { TEMPLATE_NAMES[sl as usize].as_bytes().to_vec() };
    }
    let mut models = Models { models: Vec::new() };
    for _ in 0..3 {
        let hlod = next() != 0;
        let n = next() as usize;
        let mut bones = Vec::new();
        for bi in 0..6 {
            let code = next() as usize;
            let parent = next() as u32;
            let mut p = [0i32; 12];
            for v in &mut p {
                *v = next();
            }
            if bi < n {
                bones.push((BONE_NAMES[code], parent, matrix(&p)));
            }
        }
        models.models.push((hlod, bones));
    }
    let nstates = next() as usize;
    let mut lines = vec![line(0, "ParticlesAttachedToAnimatedBones", if attached { "Yes" } else { "No" })];
    for (si, bit) in STATE_BITS.iter().enumerate() {
        let model = next() as usize;
        let nb = next() as usize;
        let mut bones = Vec::new();
        for bi in 0..3 {
            let bone = next() as usize;
            let tmpl = next() as usize;
            if bi < nb {
                bones.push(format!("{} {}", STATE_BONE_NAMES[bone], TEMPLATE_NAMES[tmpl]));
            }
        }
        if si >= nstates {
            continue;
        }
        lines.push(if si == 0 { line(0, "DefaultConditionState", "") } else { line(0, "ConditionState", bit) });
        lines.push(line(1, "Model", MODEL_NAMES[model]));
        for bl in &bones {
            lines.push(line(1, "ParticleSysBone", bl));
        }
    }
    let block = ClientModuleData { module_tag_name_key: Default::default(), lines };
    let data = Arc::new(W3DModelDrawModuleData::parse(b"W3DModelDraw", &block).expect("parses").expect("a model draw"));
    let states: Vec<usize> = STATE_BITS[..nstates].iter().map(|bit| data.find_best_info(&flags(bit)).expect("a state")).collect();
    let mut random = RandomValues::new(1);
    let mut out = Vec::new();
    let mut m = {
        let mut env = DrawEnv { assets: &mut models, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
        W3DModelDraw::new_scaled(data.clone(), 0, false, false, scale, Arc::new(Vec::new()), &mut env).expect("an idle state")
    };
    m.drawable_id = drawable_id;
    state_words(&mut out, &m, &mgr);
    for s in w[k..].chunks_exact(STEP) {
        match s[0] {
            0 => {
                let mut env = DrawEnv { assets: &mut models, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
                m.set_model_state(StateRef::Normal(states[s[1] as usize % nstates]), &mut env);
            }
            1 => {
                let facts = DrawFacts {
                    should_animate_with_power: true,
                    should_animate_without_power: true,
                    speed: None,
                    turrets: [None, None],
                    motion: None,
                    show_client_physics: true,
                    wheel_info: None,
                    transform: matrix(&s[2..]),
                };
                let mut env = DrawEnv { assets: &mut models, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 0 }) };
                m.do_draw_module(&facts, &mut env);
            }
            2 => {
                m.update_bones_for_client_particle_systems(&matrix(&s[2..]), Some(&mut mgr));
            }
            3 => m.set_hidden(s[1] != 0, Some(&mut mgr)),
            4 => m.set_fully_obscured_by_shroud(s[1] != 0, Some(&mut mgr)),
            5 => {
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
            6 => m.drawable_no_state_particles = s[1] != 0,
            _ => {
                // (#172) `getCurrentWorldspaceClientBonePositions(name, transform)` under the render object's transform
                match m.get_current_worldspace_client_bone_positions(STATE_BONE_NAMES[s[1] as usize % 6].as_bytes(), &matrix(&s[2..])) {
                    Some(t) => {
                        out.extend([200, 1]);
                        for row in 0..3 {
                            for c in 0..4 {
                                out.push(b(t.row[row][c]));
                            }
                        }
                    }
                    None => {
                        out.extend([200, 0]);
                        out.extend([0; 12]);
                    }
                }
            }
        }
        state_words(&mut out, &m, &mgr);
    }
    SYSTEMS_MADE.store(u64::from(mgr.unique_system_id), std::sync::atomic::Ordering::Relaxed);
    out
}

fn run_cpp(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 20];
    let n = unsafe { cpp::ref_model_draw_particles_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

/// The largest |rust - c++| / max(|rust|, |c++|, 1) over differing finite words (f64 bits).
static MAX_REL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The systems the last Rust case made (its manager's last id).
static SYSTEMS_MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Both runs of `n` cases; returns the check and (steps, systems made, NaN words, NaN-only differences, max ULP).
fn compare(name: &str, seed: u64, n: u64, any_scale: bool) -> (Check, [u64; 5]) {
    let mut c = Check::new(name);
    let mut g = Gen::new(seed);
    let (mut steps, mut systems, mut nan_words, mut nan_only, mut max_ulp) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for k in 0..n {
        let w = random_case(&mut g, any_scale);
        let mut rust = rust_case(&w);
        let mut cc = run_cpp(&w);
        if rust.len() == cc.len() {
            for (a, bb) in rust.iter_mut().zip(cc.iter_mut()) {
                if a != bb && r(*a).is_nan() && r(*bb).is_nan() {
                    nan_only += 1;
                    *a = 0x7FC0_0000;
                    *bb = 0x7FC0_0000;
                }
            }
        }
        steps += ((w.len() - HEADER) / STEP) as u64;
        systems += SYSTEMS_MADE.swap(0, std::sync::atomic::Ordering::Relaxed);
        nan_words += rust.iter().filter(|&&x| r(x).is_nan()).count() as u64;
        if rust != cc {
            if rust.len() == cc.len() {
                for (a, bb) in rust.iter().zip(&cc) {
                    let (fa, fb) = (r(*a), r(*bb));
                    if a != bb && fa.is_finite() && fb.is_finite() {
                        max_ulp = max_ulp.max(support::ulp_distance(fa, fb));
                        // the difference against the larger of the two magnitudes and 1 (a word near zero after a
                        // cancellation can be many ULP apart and still be a rounding)
                        let d = f64::from(fa) - f64::from(fb);
                        let scale = f64::from(fa).abs().max(f64::from(fb).abs()).max(1.0);
                        MAX_REL.store(f64::to_bits((d.abs() / scale).max(f64::from_bits(MAX_REL.load(std::sync::atomic::Ordering::Relaxed)))), std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
            let at = rust.iter().zip(&cc).position(|(a, bb)| a != bb).unwrap_or(rust.len().min(cc.len()));
            if !planted() && c.mismatches < 4 {
                let lo = at.saturating_sub(6);
                println!("case {k}: at word {at}: rust {:?} c++ {:?}", &rust[lo.min(rust.len())..(at + 16).min(rust.len())], &cc[lo.min(cc.len())..(at + 16).min(cc.len())]);
            }
        }
        c.eq(rust, cc, || format!("case {k}"));
    }
    (c, [steps, systems, nan_words, nan_only, max_ulp])
}

#[test]
fn the_bone_particle_systems_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (c, [steps, systems, nan_words, nan_only, max_ulp]) = compare("W3DModelDraw bone particle systems", 0x171_0001, cases(), false);
    println!(
        "W3DModelDraw particles: {} cases, {steps} steps, {systems} particle systems made, {nan_words} NaN words ({nan_only} NaN on both sides with different bits); 0 undefined upstream (none generated); max ULP among differing finite words {max_ulp}",
        cases()
    );
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}

/// ~~Not gated: the rounding of the port's bone convention at scales that are not powers of two (module docs).~~
/// **Corrected 2026-11-12 (#172):** gated at 0 mismatches: any scale (0.25..4, and `Gen::mixed`'s 0, NaN, infinities),
/// world transforms with infinite and NaN entries, 20,000 cases (`MODEL_DRAW_PARTICLES_ANY_CASES`).
#[test]
fn any_scale_matches_upstream() {
    if planted() {
        return;
    }
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let n = std::env::var("MODEL_DRAW_PARTICLES_ANY_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let (c, [steps, systems, nan_words, nan_only, max_ulp]) = compare("W3DModelDraw particles, any scale", 0x171_0002, n, true);
    let rel = f64::from_bits(MAX_REL.load(std::sync::atomic::Ordering::Relaxed));
    println!(
        "any scale: {n} cases, {steps} steps, {systems} particle systems made, {nan_words} NaN words ({nan_only} NaN on both sides with different bits); max ULP among differing finite words {max_ulp}, largest difference {rel:e} of max(|value|, 1)"
    );
    c.finish();
}
