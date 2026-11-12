#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable/Update/SwayClientUpdate.cpp and its header
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable/Update/AnimatedParticleSysBoneClientUpdate.cpp and its header
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/LaserUpdate.cpp and its header
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable/Update/BeaconClientUpdate.cpp (xfer)
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/Module.cpp (Module::xfer, DrawableModule::xfer)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #165: the client update modules against the upstream text compiled verbatim
//! (`differential/wrap_client_update.cpp`, which lists the cuts, the stand-ins and the words).
//!
//! # Methodology
//!
//! - **`SwayClientUpdate`** ([`SwayClientUpdate`] through a test [`SwayHost`] that mirrors the C++ stand-ins, the
//!   client's random numbers the translated `RandomValues` against upstream's `RandomValue.cpp`, both seeded by
//!   `InitRandom(seed)`): 20,000 random sequences of 2 to 81 steps, the first a breeze in 19 of 20 (then `clientUpdate`
//!   60 %, a new breeze 8 %, visibility
//!   10 %, burned 3 %, `stopSway` 2 %, `loadPostProcess` 5 %, the drawable coming or going 2 %, the rest updates) from
//!   breezes of random directions, intensities and leans of -1 to 1, randomness 0 (a quarter of them) to 1, periods of
//!   -3 to 300 (0 one breeze in fifty, and the zero breeze of a sequence that starts without one: the step is infinite,
//!   and the matrix turns NaN; NaNs on both sides compare equal whatever their sign, as `Check::f32` counts them) and versions of -2 to 5. Compared after each step: the seven
//!   members, the drawable's instance matrix (12 floats as bits) and the count of `setInstanceMatrix` calls. A
//!   step undefined upstream (a new breeze version without a drawable: the C++ would read the matrix through `NULL`)
//!   ends the sequence on both sides with `[999]` (the wrapper checks the members before the call; the Rust's `Err`):
//!   what came before is compared, and the sequence is counted.
//! - **`AnimatedParticleSysBoneClientUpdate`**: 20,000 random sequences of 1 to 40 steps over 0 to 4 draw modules, each
//!   with or without an `ObjectDrawInterface` and answering true or false; compared: each
//!   `updateBonesForClientParticleSystems` call (which module) and `m_life` after each step.
//! - **`LaserUpdate`**: 20,000 random sequences of 1 to 40 steps (`clientUpdate` 40 %, `initLaser` 15 % with a parent or
//!   none, a target or none, a start and an end or none, a bone of three, size deltas of -40 to 40, now and then near
//!   the `Int` limits; `setDecayFrames` 5 %; the frame moving on 15 %; drawables appearing and going, moving, dying, the
//!   shroud, the manager losing its systems, `setDirty`, `getCurrentLaserRadius`, the local player, the destructor) on
//!   four objects (each with or without a drawable and a `MUZZLE` bone, at random positions, sometimes the same), muzzle
//!   and target systems named, misnamed or absent, punch-through scalars of 0 or -1 to 3, creation failing now and
//!   then. Compared: every template looked up, system made, moved and destroyed, the drawable moved or destroyed, each
//!   radius, and the module's members after each step. A step undefined upstream (no local player with a parent; no
//!   parent, a start, a target and no `endPos`) ends the sequence on both sides with `[999]`, as the sway's (counted).
//! - **The saves** (`xfer`, all four modules, `BeaconClientUpdate`'s too): 20,000 random module states per module
//!   saved by both through `XferSave` (the bytes and the status compared; a laser bone of 255 or more characters, one
//!   case in fifty, is `XFER_STRING_ERROR`), then 20,000 loads per module through `XferLoad` of the saved bytes as they
//!   are (half), cut short at a random length (a quarter), with the version byte set to 0 or 2 (an eighth) or with a
//!   random byte changed outside the `Bool`s (an eighth; a `Bool` byte other than 0 or 1 is undefined upstream), into a
//!   fresh module: the status and every member compared.
//!
//! # Predictions (written before the first run, `issue165_predictions.md`)
//!
//! 0 mismatches in all. Undefined: sway sequences a few per cent (2 % of steps take the drawable away and a sequence
//! often has a new breeze after it); laser sequences about 10 %.
//!
//! # Results (2026-11-12)
//!
//! - First runs, harness defects (not the translation's): the C++ load test's `delete` ran `~LaserUpdate()` into the
//!   words (fixed: its records are dropped); NaNs of opposite sign in the sway's matrix after a zero breeze period
//!   (compared as `Check::f32` does); a planted Rust that strayed (9604) made the C++ reach a `NULL` the unplanted Rust
//!   had excluded, and crashed: undefined steps are now checked by the wrapper before the call, ending both runs.
//! - **`SwayClientUpdate`:** 20,000 sequences, 826,163 steps, 178,571 matrices set; 3,551 ended undefined upstream
//!   (predicted 3-8 %: a miss, 17.8 %: a drawable taken away stays away until a 2 % step brings it back, and a new
//!   breeze arrives first); **0 mismatches** (216,905 float words NaN on both sides with different signs).
//! - **`AnimatedParticleSysBoneClientUpdate`:** 20,000 sequences, 413,080 steps, 224,718 draw modules asked, **0
//!   mismatches**.
//! - **`LaserUpdate`:** 20,000 sequences, 410,225 steps (8,827 systems made, 14,756 moved, 4,653 drawables destroyed);
//!   2,441 ended undefined upstream (predicted 8-15 %: 12.2 %); **0 mismatches**.
//! - **The saves:** 80,000 saves (sizes: beacon 11, sway 26, bones 3, laser 67 + the bone; 201 laser bones of 256
//!   characters refused, `XFER_STRING_ERROR`) and 79,799 loads (13,109 / 13,455 / 11,308 / 13,410 whole), **0
//!   mismatches**.
//!
//! Planted bugs, mismatching cases (of 20,000 sequences, or of the 159,799 save and load comparisons): 9601 16,589;
//! 9602 18,156; 9603 18,297; 9604 12,963; 9605 1,007; 9606 16,822; 9607 16,437; 9608 16,437; 9609 13,766; 9610
//! 36,935; 9611 19,774; 9612 7,841; 9613 34,709; 9614 36,266; 9621 3,021; 9622 2,769; 9623 399; 9624 919; 9625 26;
//! 9626 12,639; 9627 2,348; 9628 3,126; 9629 17,702. **23 of 23 caught.**
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9601 | `updateSway`'s `delta` is `randomness * 0.25` |
//! | 9602 | the phase step is `PI / period`, not `2 PI / period` |
//! | 9603 | the lean is the breeze's intensity |
//! | 9604 | an invisible drawable sways without a new breeze |
//! | 9605 | the phase wraps past `PI`, not `2 PI` |
//! | 9606 | `Sin` of the phase, not `Cos` |
//! | 9607 | the X rotation's sign is lost |
//! | 9608 | the Y rotation takes the direction's x |
//! | 9609 | `m_curAngle` keeps the change, not the target |
//! | 9610 | the sway's save swaps the angle and the step |
//! | 9611 | `m_life` counts by 2 |
//! | 9612 | the bone walk stops at a module that answers false |
//! | 9613 | the bones module saves version 2 |
//! | 9614 | the beacon's save swaps the system id and the pulse |
//! | 9621 | the destructor destroys the muzzle system twice, not the target's |
//! | 9622 | a bone's start takes the bone's y as its z |
//! | 9623 | the punch-through is measured from the end, not the start |
//! | 9624 | the widening's span is one frame longer |
//! | 9625 | a widening that reaches exactly 1 goes on |
//! | 9626 | an `endPos` given is ignored for the start |
//! | 9627 | the systems are made only when the parent is clear, not partly clear |
//! | 9628 | the target system goes to the start |
//! | 9629 | the laser's save swaps widening and decaying |

mod support;

use std::cell::RefCell;

use rts_lockstep_engine::client::animated_particle_sys_bone_client_update::{AnimatedParticleSysBoneClientUpdate, AnimatedParticleSysBoneHost};
use rts_lockstep_engine::client::beacon_client_update::{BeaconClientUpdate, BeaconClientUpdateModuleData};
use rts_lockstep_engine::client::laser_update::{LaserHost, LaserObject, LaserUpdate, LaserUpdateModuleData};
use rts_lockstep_engine::client::particle_sys::ParticleSystemId;
use rts_lockstep_engine::client::sway_client_update::{SwayClientUpdate, SwayHost};
use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D};
use rts_lockstep_engine::common::game_common::ObjectShroudStatus;
use rts_lockstep_engine::common::game_type::DrawableId;
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::xfer::{Xfer, XferStatus};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::script_engine::BreezeInfo;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

/// The wrapper's records are one global buffer: one test at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    c.finish();
}

fn cases() -> u64 {
    std::env::var("CLIENT_UPDATE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000)
}

fn b(f: f32) -> i32 {
    f.to_bits() as i32
}

fn r(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

fn fnv(s: &[u8]) -> i32 {
    let mut h: u32 = 2_166_136_261;
    for &c in s {
        h ^= u32::from(c);
        h = h.wrapping_mul(16_777_619);
    }
    h as i32
}

fn run_cpp(f: unsafe extern "C" fn(*const i32, i32, *mut i32, i32) -> i32, input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 18];
    let n = unsafe { f(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len());
    out.truncate(n as usize);
    out
}

fn report_first_difference(k: u64, rust: &[i32], cc: &[i32]) {
    if rust != cc {
        let at = rust.iter().zip(cc).position(|(a, b)| a != b).unwrap_or(rust.len().min(cc.len()));
        let lo = at.saturating_sub(4);
        println!("case {k}: at word {at}: rust {:?} c++ {:?}", &rust[lo.min(rust.len())..(at + 24).min(rust.len())], &cc[lo.min(cc.len())..(at + 24).min(cc.len())]);
    }
}

// ---------------------------------------------------------------------------
// SwayClientUpdate
// ---------------------------------------------------------------------------

struct SwayWorld {
    breeze: BreezeInfo,
    random: RandomValues,
    has_drawable: bool,
    visible: bool,
    burned: bool,
    m: Matrix3D,
    sets: i32,
}

impl SwayHost for SwayWorld {
    fn breeze_info(&self) -> BreezeInfo {
        self.breeze
    }
    fn game_client_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.random.game_client_random_value_real(lo, hi)
    }
    fn has_drawable(&self) -> bool {
        self.has_drawable
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
    fn instance_matrix(&self) -> Matrix3D {
        self.m
    }
    fn set_instance_matrix(&mut self, m: &Matrix3D) {
        self.m = *m;
        self.sets += 1;
    }
    fn object_burned(&self) -> bool {
        self.burned
    }
}

/// The Rust's words; a step undefined upstream ends them with `[999]` (the wrapper checks the same before the call).
fn sway_rust(input: &[i32]) -> Vec<i32> {
    let mut w = SwayWorld {
        breeze: BreezeInfo::default(),
        random: RandomValues::new(input[0] as u32),
        has_drawable: input[1] != 0,
        visible: input[2] != 0,
        burned: input[3] != 0,
        m: Matrix3D::IDENTITY,
        sets: 0,
    };
    let mut s = SwayClientUpdate::new();
    let mut out = Vec::new();
    for a in input[4..].chunks(8) {
        match a[0] {
            0 => {
                if s.client_update(&mut w).is_err() {
                    out.push(999);
                    return out;
                }
            }
            1 => {
                w.breeze.direction_vec = Coord2D { x: r(a[1]), y: r(a[2]) };
                w.breeze.intensity = r(a[3]);
                w.breeze.lean = r(a[4]);
                w.breeze.randomness = r(a[5]);
                w.breeze.breeze_period = a[6] as i16;
                w.breeze.breeze_version = a[7] as i16;
            }
            2 => w.visible = a[1] != 0,
            3 => w.burned = a[1] != 0,
            4 => s.stop_sway(),
            5 => s.load_post_process(&mut w),
            _ => w.has_drawable = a[1] != 0,
        }
        out.extend([100, b(s.cur_value), b(s.cur_angle), b(s.cur_delta), b(s.cur_angle_limit), b(s.lean_angle), i32::from(s.cur_version), i32::from(s.swaying)]);
        for row in &w.m.row {
            out.extend([b(row.x), b(row.y), b(row.z), b(row.w)]);
        }
        out.push(w.sets);
    }
    out
}

#[test]
fn sway_client_update_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x165_0001);
    let mut c = Check::new("SwayClientUpdate");
    let (mut steps, mut undefined, mut sways, mut stops) = (0u64, 0u64, 0u64, 0u64);
    for k in 0..cases() {
        let mut input = vec![g.u32() as i32, i32::from(g.below(20) != 0), i32::from(g.below(2) == 0), i32::from(g.below(40) == 0)];
        // the first step is a breeze nineteen times in twenty (the script engine's own is never all zero: its reset sets
        // a period of 150 frames); the twentieth sequence starts on the zero breeze (period 0: an infinite step)
        let first_breeze = g.below(20) != 0;
        for n in 0..2 + g.below(80) {
            let mut st = [0i32; 8];
            let pick = if n == 0 { if first_breeze { 60 } else { 0 } } else { g.below(100) };
            match pick {
                0..=59 => {}
                60..=67 => {
                    st[0] = 1;
                    st[1] = b(g.uniform(-1.0, 1.0));
                    st[2] = b(g.uniform(-1.0, 1.0));
                    st[3] = b(g.uniform(-1.0, 1.0));
                    st[4] = b(g.uniform(-1.0, 1.0));
                    st[5] = if g.below(4) == 0 { 0 } else { b(g.uniform(0.0, 1.0)) };
                    st[6] = if g.below(50) == 0 { 0 } else { g.below(304) as i32 - 3 };
                    st[7] = g.below(8) as i32 - 2;
                }
                68..=77 => st = [2, g.below(2) as i32, 0, 0, 0, 0, 0, 0],
                78..=80 => st = [3, i32::from(g.below(6) == 0), 0, 0, 0, 0, 0, 0],
                81 => st[0] = 4,
                83..=87 => st[0] = 5,
                88..=89 => st = [6, g.below(2) as i32, 0, 0, 0, 0, 0, 0],
                _ => {}
            }
            input.extend(st);
        }
        steps += ((input.len() - 4) / 8) as u64;
        let rust = sway_rust(&input);
        let mut cc = run_cpp(cpp::ref_sway_seq, &input);
        if cc.last() == Some(&999) {
            undefined += 1;
        }
        // a float word NaN on both sides compares as the support's `Check::f32` does: equal up to the NaN's sign and
        // payload (which operand's NaN an SSE operation passes on is the compiler's operand order, not upstream's)
        if rust.len() == cc.len() {
            for (i, (rw, cw)) in rust.iter().zip(cc.iter_mut()).enumerate() {
                let float = i + 1 < rust.len() && matches!(i % 21, 1..=5 | 8..=19);
                if float && rw != cw && r(*rw).is_nan() && r(*cw).is_nan() {
                    *cw = *rw;
                    c.nan_only += 1;
                }
            }
        }
        for rec in cc.chunks_exact(21) {
            if rec[7] == 0 {
                stops += 1;
            }
        }
        sways += cc.chunks_exact(21).last().map_or(0, |rec| rec[20] as u64);
        report_first_difference(k, &rust, &cc);
        c.eq(rust, cc, || format!("sequence {k}: {input:?}"));
    }
    println!("SwayClientUpdate: {} sequences, {steps} steps, {undefined} ended undefined upstream, {sways} matrices set, {stops} steps stopped", cases());
    finish(c);
}

// ---------------------------------------------------------------------------
// AnimatedParticleSysBoneClientUpdate
// ---------------------------------------------------------------------------

struct BonesWorld {
    has_drawable: bool,
    modules: Vec<(bool, bool)>,
    out: Vec<i32>,
}

impl AnimatedParticleSysBoneHost for BonesWorld {
    fn has_drawable(&self) -> bool {
        self.has_drawable
    }
    fn draw_module_count(&self) -> usize {
        self.modules.len()
    }
    fn has_object_draw_interface(&self, i: usize) -> bool {
        self.modules[i].0
    }
    fn update_bones_for_client_particle_systems(&mut self, i: usize) -> bool {
        self.out.extend([1, i as i32]);
        self.modules[i].1
    }
}

fn bones_rust(input: &[i32]) -> Vec<i32> {
    let n = input[1] as usize;
    let mut w = BonesWorld { has_drawable: input[0] != 0, modules: (0..n).map(|i| (input[2 + 2 * i] != 0, input[3 + 2 * i] != 0)).collect(), out: Vec::new() };
    let mut a = AnimatedParticleSysBoneClientUpdate::new();
    for s in input[2 + 2 * n..].chunks(3) {
        match s[0] {
            0 => a.client_update(&mut w),
            1 => w.modules[s[1] as usize].1 = s[2] != 0,
            _ => w.has_drawable = s[1] != 0,
        }
        w.out.extend([100, a.life as i32]);
    }
    w.out
}

#[test]
fn animated_particle_sys_bone_client_update_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x165_0002);
    let mut c = Check::new("AnimatedParticleSysBoneClientUpdate");
    let (mut steps, mut asked) = (0u64, 0u64);
    for k in 0..cases() {
        let n = g.below(5) as i32;
        let mut input = vec![i32::from(g.below(10) != 0), n];
        for _ in 0..n {
            input.extend([i32::from(g.below(3) != 0), g.below(2) as i32]);
        }
        for _ in 0..1 + g.below(40) {
            match g.below(10) {
                0..=6 => input.extend([0, 0, 0]),
                7 | 8 if n > 0 => input.extend([1, g.below(n as u32) as i32, g.below(2) as i32]),
                _ => input.extend([2, i32::from(g.below(4) != 0), 0]),
            }
        }
        steps += ((input.len() - 2 - 2 * n as usize) / 3) as u64;
        let rust = bones_rust(&input);
        let cc = run_cpp(cpp::ref_apsb_seq, &input);
        let mut i = 0;
        while i < cc.len() {
            if cc[i] == 1 {
                asked += 1;
            }
            i += 2;
        }
        report_first_difference(k, &rust, &cc);
        c.eq(rust, cc, || format!("sequence {k}: {input:?}"));
    }
    println!("AnimatedParticleSysBoneClientUpdate: {} sequences, {steps} steps, {asked} draw modules asked", cases());
    finish(c);
}

// ---------------------------------------------------------------------------
// LaserUpdate
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
struct Obj {
    has_drawable: bool,
    exists: bool,
    pos: Coord3D,
    dead: bool,
    shroud: i32,
    has_bone: bool,
}

struct LaserWorld {
    frame: u32,
    objs: [Obj; 5],
    local_player: i32,
    create_fails: bool,
    next_id: u32,
    lost: bool,
    destroyed: bool,
    laser_draw: bool,
    laser_width: f32,
    out: RefCell<Vec<i32>>,
}

impl LaserWorld {
    fn rec(&self, w: &[i32]) {
        self.out.borrow_mut().extend_from_slice(w);
    }
    fn object(&self, k: i32) -> Option<LaserObject> {
        if k == 0 {
            return None;
        }
        let o = &self.objs[k as usize];
        Some(LaserObject { id: k, drawable: o.has_drawable.then_some(10 + k), position: o.pos })
    }
}

fn shroud(i: i32) -> ObjectShroudStatus {
    match i {
        1 => ObjectShroudStatus::Clear,
        2 => ObjectShroudStatus::PartialClear,
        3 => ObjectShroudStatus::Fogged,
        4 => ObjectShroudStatus::Shrouded,
        5 => ObjectShroudStatus::InvalidButPreviousValid,
        _ => ObjectShroudStatus::Invalid,
    }
}

impl LaserHost for LaserWorld {
    fn logic_frame(&self) -> u32 {
        self.frame
    }
    fn has_drawable(&self) -> bool {
        true
    }
    fn drawable_position(&self, id: DrawableId) -> Option<Coord3D> {
        let k = id - 10;
        if !(1..=4).contains(&k) {
            return None;
        }
        let o = &self.objs[k as usize];
        (o.has_drawable && o.exists).then_some(o.pos)
    }
    fn drawable_bone_transform(&self, id: DrawableId, bone: &[u8]) -> Option<Matrix3D> {
        let o = &self.objs[(id - 10) as usize];
        if !o.has_bone || bone != b"MUZZLE" {
            return None;
        }
        let mut m = Matrix3D::IDENTITY;
        m.set_translation(&Vector3 { x: o.pos.x + 1.0, y: o.pos.y + 2.0, z: o.pos.z + 3.0 });
        Some(m)
    }
    fn drawable_object_effectively_dead(&self, id: DrawableId) -> bool {
        self.objs[(id - 10) as usize].dead
    }
    fn destroy_own_drawable(&mut self) {
        self.rec(&[6]);
        self.destroyed = true;
    }
    fn local_player_index(&self) -> Option<i32> {
        (self.local_player != 0).then_some(self.local_player - 1)
    }
    fn shrouded_status(&self, obj: &LaserObject, _: i32) -> ObjectShroudStatus {
        shroud(self.objs[obj.id as usize].shroud)
    }
    fn find_template(&self, name: &[u8]) -> bool {
        self.rec(&[1, fnv(name)]);
        name == b"Flare"
    }
    fn create_particle_system(&mut self, _: &[u8]) -> Option<ParticleSystemId> {
        if self.create_fails || self.next_id >= 63 {
            return None;
        }
        self.next_id += 1;
        self.rec(&[2, self.next_id as i32]);
        Some(self.next_id)
    }
    fn set_particle_system_position(&mut self, id: ParticleSystemId, pos: &Coord3D) {
        if !self.lost && id >= 1 && id <= self.next_id {
            self.rec(&[3, id as i32, b(pos.x), b(pos.y), b(pos.z)]);
        }
    }
    fn destroy_particle_system_by_id(&mut self, id: ParticleSystemId) {
        self.rec(&[4, id as i32]);
    }
    fn set_drawable_position(&mut self, pos: &Coord3D) {
        self.rec(&[5, b(pos.x), b(pos.y), b(pos.z)]);
    }
    fn laser_template_width(&self) -> Option<f32> {
        self.laser_draw.then_some(self.laser_width)
    }
}

fn laser_name(k: i32) -> Vec<u8> {
    match k {
        1 => b"Flare".to_vec(),
        2 => b"NoSuchSystem".to_vec(),
        _ => Vec::new(),
    }
}

fn laser_state(l: &LaserUpdate) -> [i32; 19] {
    [
        b(l.start_pos.x),
        b(l.start_pos.y),
        b(l.start_pos.z),
        b(l.end_pos.x),
        b(l.end_pos.y),
        b(l.end_pos.z),
        i32::from(l.dirty),
        l.particle_system_id as i32,
        l.target_particle_system_id as i32,
        i32::from(l.widening),
        i32::from(l.decaying),
        l.widen_start_frame as i32,
        l.widen_finish_frame as i32,
        b(l.current_width_scalar),
        l.decay_start_frame as i32,
        l.decay_finish_frame as i32,
        l.parent_id,
        l.target_id,
        fnv(&l.parent_bone_name),
    ]
}

/// The Rust's words; a step undefined upstream ends them with `[999]` (the wrapper checks the same before the call).
fn laser_rust(input: &[i32]) -> Vec<i32> {
    let mut w = LaserWorld {
        frame: input[0] as u32,
        objs: [Obj::default(); 5],
        local_player: input[4],
        create_fails: input[5] != 0,
        next_id: 0,
        lost: false,
        destroyed: false,
        laser_draw: false,
        laser_width: 0.0,
        out: RefCell::new(Vec::new()),
    };
    for k in 1..=4 {
        let o = &input[6 + 8 * (k - 1)..];
        w.objs[k] = Obj { has_drawable: o[0] != 0, exists: o[1] != 0, pos: Coord3D { x: r(o[2]), y: r(o[3]), z: r(o[4]) }, dead: o[5] != 0, shroud: o[6], has_bone: o[7] != 0 };
    }
    let d = LaserUpdateModuleData::default();
    w.rec(&[200, b(d.punch_through_scalar), d.particle_system_name.len() as i32, d.target_particle_system_name.len() as i32]);
    let data = LaserUpdateModuleData { particle_system_name: laser_name(input[1]), target_particle_system_name: laser_name(input[2]), punch_through_scalar: r(input[3]) };
    let mut l = Some(LaserUpdate::new(data));
    for a in input[38..].chunks(16) {
        let Some(m) = l.as_mut() else { break };
        if w.destroyed {
            break;
        }
        match a[0] {
            0 => m.client_update(&w),
            1 => {
                let parent = w.object(a[1]);
                let target = w.object(a[2]);
                let sp = Coord3D { x: r(a[4]), y: r(a[5]), z: r(a[6]) };
                let ep = Coord3D { x: r(a[8]), y: r(a[9]), z: r(a[10]) };
                let bone: &[u8] = [&b""[..], b"MUZZLE", b"NOBONE"][a[11] as usize];
                let before = w.out.borrow().len();
                if m.init_laser(&mut w, parent.as_ref(), target.as_ref(), (a[3] != 0).then_some(&sp), (a[7] != 0).then_some(&ep), bone, a[12]).is_err() {
                    // what the step did before upstream's `NULL` is not compared (the wrapper stops before the call)
                    w.out.borrow_mut().truncate(before);
                    w.rec(&[999]);
                    return w.out.into_inner();
                }
            }
            2 => m.set_decay_frames(a[1] as u32, &w),
            3 => w.frame = a[1] as u32,
            4 => w.objs[a[1] as usize].exists = a[2] != 0,
            5 => w.objs[a[1] as usize].pos = Coord3D { x: r(a[2]), y: r(a[3]), z: r(a[4]) },
            6 => w.objs[a[1] as usize].dead = a[2] != 0,
            7 => w.objs[a[1] as usize].shroud = a[2],
            8 => w.lost = true,
            9 => m.set_dirty(a[1] != 0),
            10 => {
                w.laser_draw = a[1] != 0;
                w.laser_width = r(a[2]);
                let radius = m.get_current_laser_radius(&w).expect("the laser's drawable is there");
                w.rec(&[9, b(radius)]);
            }
            11 => w.local_player = a[1],
            _ => {
                let mut m = l.take().expect("alive");
                m.destroy(&mut w);
                continue;
            }
        }
        let st = laser_state(l.as_ref().expect("alive"));
        w.rec(&[100]);
        w.rec(&st);
    }
    if let Some(mut m) = l {
        m.destroy(&mut w);
    }
    w.out.into_inner()
}

#[test]
fn laser_update_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x165_0003);
    let mut c = Check::new("LaserUpdate");
    let (mut steps, mut undefined, mut made, mut moved, mut destroyed, mut punched) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for k in 0..cases() {
        let mut frame = if g.below(8) == 0 { u32::MAX - g.below(100) } else { g.below(1000) };
        let scalar = if g.below(3) == 0 { 0.0 } else { g.uniform(-1.0, 3.0) };
        let mut input = vec![frame as i32, g.below(3) as i32, g.below(3) as i32, b(scalar), if g.below(30) == 0 { 0 } else { 1 + g.below(4) as i32 }, i32::from(g.below(15) == 0)];
        let pos = |g: &mut Gen| -> [i32; 3] { [b(g.uniform(-500.0, 500.0)), b(g.uniform(-500.0, 500.0)), b(g.uniform(0.0, 50.0))] };
        let shared = pos(&mut g);
        for _ in 1..=4 {
            let p = if g.below(6) == 0 { shared } else { pos(&mut g) };
            input.extend([i32::from(g.below(8) != 0), i32::from(g.below(6) != 0), p[0], p[1], p[2], i32::from(g.below(8) == 0), g.below(6) as i32, i32::from(g.below(2) == 0)]);
        }
        for _ in 0..1 + g.below(40) {
            let mut st = [0i32; 16];
            match g.below(100) {
                0..=39 => {}
                40..=54 => {
                    st[0] = 1;
                    st[1] = if g.below(3) == 0 { 0 } else { 1 + g.below(4) as i32 };
                    st[2] = if g.below(3) == 0 { 0 } else { 1 + g.below(4) as i32 };
                    st[3] = i32::from(g.below(6) != 0);
                    let p = pos(&mut g);
                    st[4..7].copy_from_slice(&p);
                    st[7] = i32::from(g.below(5) != 0);
                    let p = pos(&mut g);
                    st[8..11].copy_from_slice(&p);
                    st[11] = g.below(3) as i32;
                    st[12] = match g.below(12) {
                        0 => i32::MIN + g.below(3) as i32,
                        1 => i32::MAX - g.below(3) as i32,
                        2..=5 => 0,
                        _ => g.below(81) as i32 - 40,
                    };
                }
                55..=59 => st[..2].copy_from_slice(&[2, if g.below(10) == 0 { 0 } else { g.below(60) as i32 }]),
                60..=74 => {
                    frame = frame.wrapping_add(g.below(6));
                    st[..2].copy_from_slice(&[3, frame as i32]);
                }
                75..=78 => st[..3].copy_from_slice(&[4, 1 + g.below(4) as i32, g.below(2) as i32]),
                79..=83 => {
                    let p = pos(&mut g);
                    st[..5].copy_from_slice(&[5, 1 + g.below(4) as i32, p[0], p[1], p[2]]);
                }
                84..=86 => st[..3].copy_from_slice(&[6, 1 + g.below(4) as i32, g.below(2) as i32]),
                87..=89 => st[..3].copy_from_slice(&[7, 1 + g.below(4) as i32, g.below(6) as i32]),
                90 => st[0] = 8,
                91..=93 => st[..2].copy_from_slice(&[9, g.below(2) as i32]),
                94..=96 => st[..3].copy_from_slice(&[10, g.below(2) as i32, b(g.uniform(0.0, 20.0))]),
                97..=98 => st[..2].copy_from_slice(&[11, if g.below(4) == 0 { 0 } else { 1 + g.below(4) as i32 }]),
                _ => st[0] = 12,
            }
            input.extend(st);
        }
        steps += ((input.len() - 38) / 16) as u64;
        let rust = laser_rust(&input);
        let cc = run_cpp(cpp::ref_laser_seq, &input);
        if cc.last() == Some(&999) {
            undefined += 1;
        }
        // count the records (each starts with its tag; the members' record is 20 words)
        let mut i = 4;
        while i < cc.len() {
            let len = match cc[i] {
                1 => 2,
                2 => {
                    made += 1;
                    2
                }
                3 => {
                    moved += 1;
                    5
                }
                4 => 2,
                5 => 4,
                6 => {
                    destroyed += 1;
                    1
                }
                9 => 2,
                999 => 1,
                _ => 20,
            };
            i += len;
        }
        if r(input[3]) > 0.0 {
            punched += 1;
        }
        report_first_difference(k, &rust, &cc);
        c.eq(rust, cc, || format!("sequence {k}: {input:?}"));
    }
    println!(
        "LaserUpdate: {} sequences, {steps} steps, {undefined} ended undefined upstream: {made} systems made, {moved} systems moved, {destroyed} drawables destroyed, {punched} sequences with a punch-through scalar",
        cases()
    );
    finish(c);
}

// ---------------------------------------------------------------------------
// The four xfers
// ---------------------------------------------------------------------------

fn status(r: Result<(), XferStatus>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => e as i32 + 1,
    }
}

/// A module of `kind` (0 beacon, 1 sway, 2 bones, 3 laser) and its members as the wrapper's words.
#[derive(Clone, Debug)]
enum Any {
    Beacon(BeaconClientUpdate),
    Sway(SwayClientUpdate),
    Bones(AnimatedParticleSysBoneClientUpdate),
    Laser(LaserUpdate),
}

impl Any {
    fn fresh(kind: i32) -> Any {
        match kind {
            0 => Any::Beacon(BeaconClientUpdate::new(BeaconClientUpdateModuleData::default(), 7)),
            1 => Any::Sway(SwayClientUpdate::new()),
            2 => Any::Bones(AnimatedParticleSysBoneClientUpdate::new()),
            _ => Any::Laser(LaserUpdate::new(LaserUpdateModuleData::default())),
        }
    }
    fn xfer(&mut self, x: &mut dyn Xfer) -> Result<(), XferStatus> {
        match self {
            Any::Beacon(m) => m.xfer(x),
            Any::Sway(m) => m.xfer(x),
            Any::Bones(m) => m.xfer(x),
            Any::Laser(m) => m.xfer(x),
        }
    }
    fn words(&self) -> Vec<i32> {
        match self {
            Any::Beacon(m) => vec![m.particle_system_id as i32, m.last_radar_pulse as i32],
            Any::Sway(s) => vec![b(s.cur_value), b(s.cur_angle), b(s.cur_delta), b(s.cur_angle_limit), b(s.lean_angle), i32::from(s.cur_version), i32::from(s.swaying)],
            Any::Bones(m) => vec![m.life as i32],
            Any::Laser(l) => {
                let mut v = laser_state(l)[..18].to_vec();
                v.push(l.parent_bone_name.len() as i32);
                v.extend(l.parent_bone_name.iter().map(|&c| i32::from(c)));
                v
            }
        }
    }
    /// A random state.
    fn random(kind: i32, g: &mut Gen) -> Any {
        let f = |g: &mut Gen| if g.below(10) == 0 { g.mixed() } else { g.uniform(-10.0, 10.0) };
        match kind {
            0 => {
                let mut m = BeaconClientUpdate::new(BeaconClientUpdateModuleData::default(), 7);
                m.particle_system_id = g.u32();
                m.last_radar_pulse = g.u32();
                Any::Beacon(m)
            }
            1 => {
                let mut s = SwayClientUpdate::new();
                s.cur_value = f(g);
                s.cur_angle = f(g);
                s.cur_delta = f(g);
                s.cur_angle_limit = f(g);
                s.lean_angle = f(g);
                s.cur_version = g.u32() as i16;
                s.swaying = g.below(2) == 0;
                Any::Sway(s)
            }
            2 => {
                let mut m = AnimatedParticleSysBoneClientUpdate::new();
                m.life = g.u32();
                Any::Bones(m)
            }
            _ => {
                let mut l = LaserUpdate::new(LaserUpdateModuleData::default());
                l.start_pos = Coord3D { x: f(g), y: f(g), z: f(g) };
                l.end_pos = Coord3D { x: f(g), y: f(g), z: f(g) };
                l.dirty = g.below(2) == 0;
                l.particle_system_id = g.u32();
                l.target_particle_system_id = g.u32();
                l.widening = g.below(2) == 0;
                l.decaying = g.below(2) == 0;
                l.widen_start_frame = g.u32();
                l.widen_finish_frame = g.u32();
                l.current_width_scalar = f(g);
                l.decay_start_frame = g.u32();
                l.decay_finish_frame = g.u32();
                l.parent_id = g.i32();
                l.target_id = g.i32();
                let n = if g.below(50) == 0 { 255 + g.below(2) as usize } else { g.below(20) as usize };
                l.parent_bone_name = (0..n).map(|_| b'A' + g.below(26) as u8).collect();
                Any::Laser(l)
            }
        }
    }
    /// The byte offsets of its `Bool`s in a save.
    fn bools(kind: i32) -> &'static [usize] {
        match kind {
            1 => &[25],
            3 => &[27, 36, 37],
            _ => &[],
        }
    }
}

fn xfer_path(kind: &str) -> String {
    format!("{}/client_update_xfer_{kind}_{}.bin", std::env::temp_dir().display(), std::process::id())
}

/// The save of a state: (status, bytes) by the Rust, then by the C++.
fn save_both(kind: i32, m: &Any) -> ((i32, Vec<u8>), (i32, Vec<u8>)) {
    let mut rm = m.clone();
    let mut x = rts_lockstep_engine::common::xfer_save::XferSave::new();
    let st = match x.open(b"module") {
        Ok(()) => status(rm.xfer(&mut x)),
        Err(e) => e as i32 + 1,
    };
    let rust = (st, x.bytes().to_vec());
    let path = xfer_path("save");
    let cpath = std::ffi::CString::new(path.clone()).expect("path");
    let words = m.words();
    let mut out = vec![0i32; 4096];
    let n = unsafe { cpp::ref_cu_xfer(kind, 0, words.as_ptr(), words.len() as i32, cpath.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
    let bytes = std::fs::read(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    assert!(n >= 1);
    (rust, (out[0], bytes))
}

/// The load of `bytes` into a fresh module: [status, members] by the Rust, then by the C++.
fn load_both(kind: i32, bytes: &[u8]) -> (Vec<i32>, Vec<i32>) {
    let mut m = Any::fresh(kind);
    let mut x = rts_lockstep_engine::common::xfer_load::XferLoad::new(bytes.to_vec());
    let st = match x.open(b"module") {
        Ok(()) => status(m.xfer(&mut x)),
        Err(e) => e as i32 + 1,
    };
    let mut rust = vec![st];
    rust.extend(m.words());
    let path = xfer_path("load");
    std::fs::write(&path, bytes).expect("write the save");
    let cpath = std::ffi::CString::new(path.clone()).expect("path");
    let mut out = vec![0i32; 4096];
    let n = unsafe { cpp::ref_cu_xfer(kind, 1, std::ptr::null(), 0, cpath.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
    let _ = std::fs::remove_file(&path);
    out.truncate(n as usize);
    (rust, out)
}

#[test]
fn the_four_saves_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let names = ["BeaconClientUpdate", "SwayClientUpdate", "AnimatedParticleSysBoneClientUpdate", "LaserUpdate"];
    let mut c = Check::new("the client update modules' xfers");
    for kind in 0..4 {
        let mut g = Gen::new(0x165_0010 + kind as u64);
        let (mut saved, mut refused, mut loads_ok, mut loads_failed, mut sizes) = (0u64, 0u64, 0u64, 0u64, std::collections::BTreeSet::new());
        for k in 0..cases() {
            let m = Any::random(kind, &mut g);
            let (rust, cc) = save_both(kind, &m);
            if rust.0 == 0 {
                saved += 1;
                sizes.insert(rust.1.len());
            } else {
                refused += 1;
            }
            c.eq(rust.clone(), cc, || format!("{} save {k}: {m:?}", names[kind as usize]));
            if rust.0 != 0 {
                continue;
            }
            let mut bytes = rust.1;
            match g.below(8) {
                0..=3 => {}
                4 | 5 => {
                    let n = g.below(bytes.len() as u32) as usize;
                    bytes.truncate(n);
                }
                6 => bytes[0] = if g.below(2) == 0 { 0 } else { 2 },
                _ => {
                    let at = g.below(bytes.len() as u32) as usize;
                    if !Any::bools(kind).contains(&at) {
                        bytes[at] = g.below(256) as u8;
                    }
                }
            }
            let (rl, cl) = load_both(kind, &bytes);
            if rl[0] == 0 {
                loads_ok += 1;
            } else {
                loads_failed += 1;
            }
            if rl != cl {
                println!("{} load {k}: rust {rl:?} c++ {cl:?}", names[kind as usize]);
            }
            c.eq(rl, cl, || format!("{} load {k}: {bytes:?}", names[kind as usize]));
        }
        println!("{}: {} states, {saved} saved ({refused} refused), sizes {sizes:?}; loads {loads_ok} whole, {loads_failed} failing", names[kind as usize], cases());
    }
    finish(c);
}
