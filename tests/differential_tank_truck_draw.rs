#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankDraw.cpp (doDrawModule,
//   updateTreadPositions)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTruckDraw.cpp (doDrawModule)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DTankTruckDraw.cpp (doDrawModule)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! GitHub #184 item 1: the tank, truck and tank truck draws' `doDrawModule`, their time freezes included
//! (`src/client/w3d_model_draw.rs`'s `tank_do_draw_module`, `src/client/w3d_truck_draw.rs`), against the upstream text
//! compiled verbatim (`differential/wrap_tank_truck_draw.cpp`, which lists the cuts, the stand-ins and the words).
//!
//! # Methodology
//!
//! 20,000 random sequences of 1 to 40 frames, each a fresh module: the kind (`W3DTankDraw`, `W3DTruckDraw`,
//! `W3DTankTruckDraw`, a third each), the bones its INI names (a 7-bit mask: front, rear, mid-front, mid-rear, mid-mid,
//! cab, trailer pairs; the tank truck has no mid-mid, cab or trailer), `TreadAnimationRate` (0 one in ten, else 0.5-6 a
//! second), the pivot and drive fractions, the cab, trailer, damping, powerslide and tire constants. The port's module is
//! made by `W3DModelDraw::new` from that INI block over one synthetic HLOD (13 pivots: root, cab, trailer and ten tires;
//! a cube on each, and three `TREADS*` cubes with a linear-offset mapper); the C++ is given the parsed constants. Each
//! frame: what the drawable has (no object 5 %, no physics 5 %), `ShowClientPhysics` (off 10 %), **the freeze's four
//! words** (the view frozen 30 %, its camera finished 50 %, the debug pause 15 %, the script's freeze 20 %: frozen in
//! about 45 % of frames), the speed (0 20 %, else 0-6), turning -1/0/1, motive 70 %, the locomotor speed (4, or 0 one in
//! twenty), a unit direction (the last one 30 %), moving backwards 20 %, a path 50 % with its angle, the wheel info 80 %
//! (four offsets, the angle, frames airborne). The port's frame is a [`DrawFacts`] with `frozen` =
//! [`PhysicsXformGates::frozen`] of those words and a transform translated by the frame number, so the base class's
//! draw shows as the render object's transform.
//!
//! Compared every frame, bit for bit: the base class's draws so far, the four wheel rotations, the cab and trailer
//! rotations, powersliding, the tread count and the first three treads' U offsets, the tank's last direction, each
//! of the 13 bones' capture and control (12 words), and the trucks' 12 bone indices.
//!
//! # Predictions (written before the first run, `issue184i1_predictions.md`)
//!
//! 0 mismatches for the freezes; ~40 % odds of a first-run harness mismatch elsewhere (bone matrices).
//!
//! # Results (2026-11-13)
//!
//! - **First build:** one error, the wrapper's (`W3DTruckDrawModuleData` not declared: a typedef added).
//! - **First run:** 72,126 mismatching words in 5,194 of 20,000 sequences, none in the freezes: 71,512 a middle
//!   tread's U after a pivot (the port wrapped its own offset; upstream's `offset_u`, declared before the loop, carries
//!   the previous tread's value) and 614 the tank truck's tread count in frames without an object (the port searched
//!   its bones and treads before upstream's `obj == NULL` return). Both port defects, fixed (`issue184i1_defects.md`).
//!   **Prediction miss:** I gave 40 % odds of a first-run mismatch in the bone matrices; there was none there, and the
//!   two defects I did not predict were elsewhere.
//! - **After the fixes:** **0 mismatches** over 20,000 sequences (6,542 tank, 6,673 truck, 6,785 tank truck), 409,893
//!   frames of which 172,809 frozen, 79,949,135 words compared (with the bone indices), max ULP 0.
//! - **Plants**, mismatching sequences of 20,000 (words): 9937 6,291 (481,245); 9938 5,836 (4,006,165); 9939 6,260
//!   (2,495,004); 9940 6,316 (124,196); 9941 13,003 (259,209); 9942 5,541 (2,228,712); 9943 5,396 (3,557,541); 9944
//!   2,435 (4,156); 9945 4,899 (71,512); 9946 326 (3,786); 9947 333 (3,240). **11 of 11 caught.** 9946 was not caught
//!   until the trucks' bone indices were added to the record (a search without an object changes no control).
//!
//! # Plants
//!
//! `L9_PLANT=<id>` (the run prints `PLANTED` and passes when caught):
//!
//! | id | planted bug |
//! |---|---|
//! | 9937 | the tank's freeze not tested |
//! | 9938 | the truck's freeze not tested |
//! | 9939 | the tank truck's freeze not tested |
//! | 9940 | the frozen tank still runs the base class's draw |
//! | 9941 | the truck's freeze tested before the base class's draw |
//! | 9942 | the tank truck's freeze stops only its treads |
//! | 9943 | the truck's freeze stops only its cab and trailer |
//! | 9944 | the tank made before the constructor's `setModelState` (its treads found at construction) |
//! | 9945 | a middle tread wraps its own offset, not the carried `offset_u` |
//! | 9946 | the truck searches its bones without an object |
//! | 9947 | the tank truck searches its bones and treads without an object |
//!
//! 9936 (`draw_facts` not reading the freeze) is the World read, caught by the lib test
//! `the_physics_gates_read_the_worlds_time_freezes` (it fails at the `draw_facts(..).frozen` assert), not here.
//!
//! # Not covered
//!
//! The effects, sounds and debris (the port does not make them: counted on the C++ side only); a render object
//! changed between draws (the port remakes it only through `setModelState`, whose `onRenderObjRecreated` runs the
//! searches itself); the police car; a drawable without an AI (upstream dereferences null).

mod support;

use std::sync::Arc;

use rts_lockstep_engine::client::drawable_loco::{PhysicsXformGates, TWheelInfo};
use rts_lockstep_engine::client::w3d_model_draw::{DrawEnv, DrawFacts, DrawModuleExtra, MotionFacts, W3DModelDraw, W3DModelDrawModuleData, W3dFiles};
use rts_lockstep_engine::client::w3d_truck_draw::TruckKind;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::file_system::{FileSystem, MemoryDevice};
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::thing_template::{ClientLine, ClientModuleData};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::w3d::assetmgr::{AssetManager, W3dFileFactory};
use rts_lockstep_engine::formats::w3d::synth::{hlod_w3d, htree_w3d, MeshSpec, PivotSpec};
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen};

const SEQUENCES: usize = 20_000;
const HEAD: usize = 11;
const IN_WORDS: usize = 22;
const OUT_WORDS: usize = 195;
const BONES: usize = 13;
const KINDS: [&str; 3] = ["W3DTankDraw", "W3DTruckDraw", "W3DTankTruckDraw"];
const PIVOTS: [&str; BONES] =
    ["ROOTTRANSFORM", "CAB", "TRAILER", "TIREFL", "TIREFR", "TIRERL", "TIRERR", "TIREMFL", "TIREMFR", "TIREMRL", "TIREMRR", "TIREMML", "TIREMMR"];

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn fb(v: f32) -> i32 {
    v.to_bits() as i32
}
fn bf(w: i32) -> f32 {
    f32::from_bits(w as u32)
}

/// The model `TTD`: the 13 pivots on the root (a cube each) and the treads TREADSL, TREADSR, TREADSX on the root, each
/// with a linear-offset mapper.
fn model() -> Vec<u8> {
    let ident = [0.0, 0.0, 0.0, 1.0];
    let pivots: Vec<PivotSpec> = PIVOTS
        .iter()
        .enumerate()
        .map(|(i, n)| PivotSpec { name: n.as_bytes().to_vec(), parent: if i == 0 { -1 } else { 0 }, translation: [i as f32 * 0.5, (i % 3) as f32, 0.25], rotation: ident })
        .collect();
    let mut f = htree_w3d(b"TTD", &pivots);
    let mut subs: Vec<(Vec<u8>, u32, u32)> = vec![(b"CHASSIS".to_vec(), 0, 0)];
    for (i, n) in PIVOTS.iter().enumerate().skip(1) {
        subs.push((n.as_bytes().to_vec(), i as u32, 0));
    }
    for t in ["TREADSL", "TREADSR", "TREADSX"] {
        subs.push((t.as_bytes().to_vec(), 0, 4));
    }
    let mut names = Vec::new();
    for (n, bone, mapping) in &subs {
        let mut m = MeshSpec::cuboid(b"TTD", n, [-0.5; 3], [0.5; 3]);
        m.stage0.texture = None;
        m.stage0.mapping = *mapping;
        f.extend(m.to_w3d());
        names.push(([b"TTD.".as_slice(), n.as_slice()].concat(), *bone));
    }
    let hl: Vec<(&[u8], u32)> = names.iter().map(|(n, b)| (n.as_slice(), *b)).collect();
    f.extend(hlod_w3d(b"TTD", b"TTD", &hl));
    f
}

/// One sequence's constants: the kind, the bone mask and the INI block's values.
struct Case {
    kind: usize,
    mask: i32,
    rate: f32,
    pivot: f32,
    drive: f32,
    cab: f32,
    trailer: f32,
    damping: f32,
    powerslide: f32,
    tire: f32,
}

fn case(g: &mut Gen) -> Case {
    Case {
        kind: g.below(3) as usize,
        mask: g.below(128) as i32,
        rate: if g.below(10) == 0 { 0.0 } else { g.uniform(0.5, 6.0) },
        pivot: g.uniform(0.0, 1.0),
        drive: g.uniform(0.0, 1.0),
        cab: g.uniform(-2.0, 2.0),
        trailer: g.uniform(-2.0, 2.0),
        damping: g.uniform(0.0, 1.0),
        powerslide: g.uniform(-1.0, 1.0),
        tire: g.uniform(0.0, 2.0),
    }
}

/// The INI block of `c`'s module (`Model TTD`, then its own fields).
fn block(c: &Case) -> ClientModuleData {
    let mut l: Vec<(u32, String, String)> = vec![(0, "DefaultConditionState".into(), String::new()), (1, "Model".into(), "TTD".into())];
    let mut push = |f: &str, a: String| l.push((0, f.into(), a));
    if c.kind == 0 || c.kind == 2 {
        push("TreadAnimationRate", format!("{}", c.rate));
        push("TreadDriveSpeedFraction", format!("{}", c.drive));
        if c.kind == 0 {
            push("TreadPivotSpeedFraction", format!("{}", c.pivot));
        }
    }
    if c.kind != 0 {
        let pairs: [(&str, &str, &str, &str); 5] = [
            ("LeftFrontTireBone", "TireFL", "RightFrontTireBone", "TireFR"),
            ("LeftRearTireBone", "TireRL", "RightRearTireBone", "TireRR"),
            ("MidLeftFrontTireBone", "TireMFL", "MidRightFrontTireBone", "TireMFR"),
            ("MidLeftRearTireBone", "TireMRL", "MidRightRearTireBone", "TireMRR"),
            ("MidLeftMidTireBone", "TireMML", "MidRightMidTireBone", "TireMMR"),
        ];
        let n = if c.kind == 1 { 5 } else { 4 };
        for (k, (lf, lb, rf, rb)) in pairs.iter().enumerate().take(n) {
            if c.mask & (1 << k) != 0 {
                push(lf, lb.to_string());
                push(rf, rb.to_string());
            }
        }
        push("TireRotationMultiplier", format!("{}", c.tire));
        push("PowerslideRotationAddition", format!("{}", c.powerslide));
        if c.kind == 1 {
            if c.mask & 32 != 0 {
                push("CabBone", "Cab".into());
                if c.mask & 64 != 0 {
                    push("TrailerBone", "Trailer".into());
                }
            }
            push("CabRotationMultiplier", format!("{}", c.cab));
            push("TrailerRotationMultiplier", format!("{}", c.trailer));
            push("RotationDamping", format!("{}", c.damping));
        }
    }
    ClientModuleData {
        module_tag_name_key: Default::default(),
        lines: l
            .iter()
            .map(|(d, f, a)| ClientLine { depth: *d, field: f.as_bytes().to_vec(), args: a.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() })
            .collect(),
    }
}

/// One frame's 22 words (see the wrapper).
fn frame(g: &mut Gen, last_dir: &mut (f32, f32)) -> Vec<i32> {
    let present = match g.below(100) {
        0..=4 => 0,
        5..=9 => 1,
        _ => 2,
    };
    let dir = if g.below(10) < 3 {
        *last_dir
    } else {
        let a = g.uniform(-3.2, 3.2);
        (rts_lockstep_engine::ww::wwmath::cos(a), rts_lockstep_engine::ww::wwmath::sin(a))
    };
    *last_dir = dir;
    vec![
        present,
        i32::from(g.below(10) != 0),
        i32::from(g.below(100) < 30),
        i32::from(g.below(2) == 0),
        i32::from(g.below(100) < 15),
        i32::from(g.below(100) < 20),
        fb(if g.below(5) == 0 { 0.0 } else { g.uniform(0.0, 6.0) }),
        g.below(3) as i32 - 1,
        i32::from(g.below(10) < 7),
        fb(if g.below(20) == 0 { 0.0 } else { 4.0 }),
        fb(dir.0),
        fb(dir.1),
        i32::from(g.below(5) == 0),
        i32::from(g.below(2) == 0),
        fb(g.uniform(-1.0, 1.0)),
        i32::from(g.below(5) != 0),
        fb(g.uniform(-0.5, 0.5)),
        fb(g.uniform(-0.5, 0.5)),
        fb(g.uniform(-0.5, 0.5)),
        fb(g.uniform(-0.5, 0.5)),
        fb(g.uniform(-0.5, 0.5)),
        g.below(10) as i32,
    ]
}

/// The gates of a frame's freeze words (the rest open).
fn gates(p: &[i32]) -> PhysicsXformGates {
    PhysicsXformGates {
        view_time_frozen: p[2] != 0,
        view_camera_movement_finished: p[3] != 0,
        time_frozen_debug: p[4] != 0,
        time_frozen_script: p[5] != 0,
        ..PhysicsXformGates::OPEN
    }
}

struct Rig {
    fs: FileSystem<MemoryDevice>,
    am: AssetManager,
    random: RandomValues,
}

/// The port: a fresh module of the case, then each frame's `doDrawModule`; the words the wrapper records.
fn port(rig: &mut Rig, c: &Case, frames: &[i32]) -> (Vec<i32>, [f32; 8]) {
    let d = Arc::new(W3DModelDrawModuleData::parse(KINDS[c.kind].as_bytes(), &block(c)).expect("parse").expect("a draw module"));
    let reals = {
        let r = |f: &rts_lockstep_engine::client::w3d_model_draw::TruckFields, k: &str| f.reals.get(k).copied().unwrap_or(0.0);
        match &d.extra {
            DrawModuleExtra::Tank(t) => [t.tread_animation_rate, t.tread_pivot_speed_fraction, t.tread_drive_speed_fraction, 0.0, 0.0, 0.0, 0.0, 0.0],
            DrawModuleExtra::Truck(f) => [0.0, 0.0, 0.0, r(f, "CabRotationMultiplier"), r(f, "TrailerRotationMultiplier"), r(f, "RotationDamping"), r(f, "PowerslideRotationAddition"), r(f, "TireRotationMultiplier")],
            DrawModuleExtra::TankTruck(f, t) => [t.tread_animation_rate, t.tread_pivot_speed_fraction, t.tread_drive_speed_fraction, 0.0, 0.0, 0.0, r(f, "PowerslideRotationAddition"), r(f, "TireRotationMultiplier")],
            _ => panic!("not a tank or truck draw"),
        }
    };
    let mut a = W3dFiles { assets: &mut rig.am, fs: &rig.fs };
    let mut env = DrawEnv { assets: &mut a, random: &mut rig.random, sync_time: 0, logic_time: true, particles: None };
    let mut m = W3DModelDraw::new(d, 0, false, false, &mut env).expect("a module");
    let mut out = Vec::new();
    let mut base = 0;
    for (k, p) in frames.chunks(IN_WORDS).enumerate() {
        let motion = (p[0] >= 2).then(|| MotionFacts {
            velocity_magnitude: bf(p[6]),
            turning: p[7],
            is_motive: p[8] != 0,
            cur_locomotor_speed: Some(bf(p[9])),
            unit_direction_2d: (bf(p[10]), bf(p[11])),
            velocity: Coord3D { x: bf(p[6]), y: 0.0, z: 0.0 },
            moving_backwards: p[12] != 0,
            angle_to_path_point: (p[13] != 0).then(|| bf(p[14])),
        });
        let wheel_info = (p[15] != 0).then(|| TWheelInfo {
            front_left_height_offset: bf(p[16]),
            front_right_height_offset: bf(p[17]),
            rear_left_height_offset: bf(p[18]),
            rear_right_height_offset: bf(p[19]),
            wheel_angle: bf(p[20]),
            frames_airborne_counter: 0,
            frames_airborne: p[21],
        });
        let mut transform = Matrix3D::IDENTITY;
        transform.set_translation(&Vector3::new((k + 1) as f32, 0.0, 0.0));
        let facts = DrawFacts {
            should_animate_with_power: true,
            should_animate_without_power: true,
            speed: motion.map(|m| m.velocity_magnitude),
            turrets: [None, None],
            motion,
            show_client_physics: p[1] != 0,
            wheel_info,
            transform,
            drawable_transform: transform,
            construction: None,
            frozen: gates(p).frozen(),
            has_object: p[0] >= 1,
        };
        let mut a = W3dFiles { assets: &mut rig.am, fs: &rig.fs };
        let mut env = DrawEnv { assets: &mut a, random: &mut rig.random, sync_time: 0, logic_time: false, particles: None };
        m.do_draw_module(&facts, &mut env);
        let r = m.render_object.as_ref().expect("a render object");
        if r.transform.row[0][3] == (k + 1) as f32 {
            base += 1;
        }
        let (mut rot, mut cab, mut slide, mut treads, mut last) = ([0.0f32; 4], [0.0f32; 2], false, Vec::new(), (0.0f32, 0.0f32));
        let mut bone_ids = [0i32; 12];
        if let Some(t) = &m.tank {
            treads = t.treads.iter().map(|x| x.custom_uv_offset[0]).collect();
            last = t.last_direction;
        }
        match &m.truck {
            Some(TruckKind::Truck(t)) | Some(TruckKind::PoliceCar(t, _)) => {
                rot = [t.front_wheel_rotation, t.rear_wheel_rotation, t.mid_front_wheel_rotation, t.mid_rear_wheel_rotation];
                cab = [t.cur_cab_rotation, t.cur_trailer_rotation];
                slide = t.is_powersliding;
                bone_ids = [
                    t.front_left_tire_bone, t.front_right_tire_bone, t.rear_left_tire_bone, t.rear_right_tire_bone, t.mid_front_left_tire_bone,
                    t.mid_front_right_tire_bone, t.mid_rear_left_tire_bone, t.mid_rear_right_tire_bone, t.mid_mid_left_tire_bone,
                    t.mid_mid_right_tire_bone, t.cab_bone, t.trailer_bone,
                ];
            }
            Some(TruckKind::TankTruck(t)) => {
                rot = [t.front_wheel_rotation, t.rear_wheel_rotation, t.mid_front_wheel_rotation, t.mid_rear_wheel_rotation];
                slide = t.is_powersliding;
                treads = t.treads.iter().map(|x| x.custom_uv_offset[0]).collect();
                bone_ids = [
                    t.front_left_tire_bone, t.front_right_tire_bone, t.rear_left_tire_bone, t.rear_right_tire_bone, t.mid_front_left_tire_bone,
                    t.mid_front_right_tire_bone, t.mid_rear_left_tire_bone, t.mid_rear_right_tire_bone, 0, 0, 0, 0,
                ];
            }
            None => {}
        }
        out.push(base);
        out.extend(rot.iter().map(|v| fb(*v)));
        out.extend([fb(cab[0]), fb(cab[1]), i32::from(slide), treads.len() as i32]);
        for i in 0..3 {
            out.push(fb(treads.get(i).copied().unwrap_or(0.0)));
        }
        out.extend([fb(last.0), fb(last.1)]);
        for b in 0..BONES as i32 {
            let c = r.bone_controls.get(&b).copied().unwrap_or_default();
            out.push(i32::from(c.captured));
            for row in 0..3 {
                for col in 0..4 {
                    out.push(fb(c.transform.row[row][col]));
                }
            }
        }
        out.extend(bone_ids);
    }
    (out, reals)
}

fn reference(input: &[i32], frames: usize) -> Vec<i32> {
    let cap = frames * OUT_WORDS;
    let mut out = vec![0i32; cap];
    let n = unsafe { cpp::ref_tank_truck_draw_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), cap as i32) };
    out.truncate(n.max(0) as usize);
    out
}

const FIELDS: [&str; 16] = [
    "base draws", "front rotation", "rear rotation", "mid-front rotation", "mid-rear rotation", "cab rotation", "trailer rotation",
    "powersliding", "tread count", "tread 0 U", "tread 1 U", "tread 2 U", "last direction x", "last direction y", "bones", "bone indices",
];

/// See the module docs.
#[test]
fn the_tank_and_truck_draws_match_upstream() {
    let _lock = cpp::GLOBAL_RNG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut d = MemoryDevice::new();
    d.insert(b"Art/W3D/ttd.w3d", model());
    let mut rig = Rig { fs: FileSystem::new(d), am: AssetManager::new_game(W3dFileFactory::default()), random: RandomValues::new(0) };
    let mut g = Gen::new(0x184_0001);
    let mut check = Check::new("tank and truck doDrawModule");
    let (mut frames_total, mut frozen, mut bad_seqs) = (0u64, 0u64, 0u64);
    let mut by_kind = [0u64; 3];
    let mut bad_fields = [0u64; 16];
    for case_no in 0..SEQUENCES {
        let c = case(&mut g);
        let n = 1 + g.below(40) as usize;
        let mut last = (1.0, 0.0);
        let mut words = Vec::with_capacity(n * IN_WORDS);
        for _ in 0..n {
            let f = frame(&mut g, &mut last);
            frozen += u64::from(gates(&f).frozen());
            words.extend(f);
        }
        frames_total += n as u64;
        by_kind[c.kind] += 1;
        let (r, reals) = port(&mut rig, &c, &words);
        let mut input = vec![c.kind as i32, c.mask];
        input.extend(reals.iter().map(|v| fb(*v)));
        input.push(n as i32);
        input.extend(&words);
        assert_eq!(input.len(), HEAD + n * IN_WORDS);
        let cr = reference(&input, n);
        let before = check.mismatches;
        check.eq(r.len(), cr.len(), || format!("case {case_no}: word count"));
        for (k, (a, b)) in r.iter().zip(&cr).enumerate() {
            let field = k % OUT_WORDS;
            let fr = k / OUT_WORDS;
            let m0 = check.mismatches;
            let slot = if field >= 183 { 15 } else { field.min(14) };
            let name = FIELDS[slot];
            if field == 0 || field == 7 || field == 8 || field >= 183 || (field >= 14 && (field - 14) % 13 == 0) {
                check.eq(*a, *b, || format!("case {case_no} ({}) frame {fr} {name} word {field}", KINDS[c.kind]));
            } else {
                check.f32(bf(*a), bf(*b), || format!("case {case_no} ({}) frame {fr} {name} word {field}", KINDS[c.kind]));
            }
            if check.mismatches > m0 {
                bad_fields[slot] += 1;
            }
        }
        if check.mismatches > before {
            bad_seqs += 1;
        }
    }
    println!("sequences {SEQUENCES} (tank {}, truck {}, tank truck {}), frames {frames_total}, frozen {frozen}", by_kind[0], by_kind[1], by_kind[2]);
    println!("mismatching sequences {bad_seqs}; words by field: {:?}", FIELDS.iter().zip(bad_fields).filter(|(_, n)| *n > 0).collect::<Vec<_>>());
    if planted() {
        check.finish_planted();
    } else {
        check.finish();
    }
}
